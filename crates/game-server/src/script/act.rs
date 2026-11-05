//! The reading and acting half of the `game` table and the player handle (`docs/plans/ext-5-lua.md` sections 7.1 and 7.4): who is
//! online and what they are, messages and notices with their text rules and rate limits. The registration half is `api.rs`.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mlua::{Lua, MultiValue, Table, Value};

use super::api::{Shared, script_error};
use super::event::PlayerRef;
use super::world::{PlayerView, ScriptWorld};

/// The most players `game.online()` returns (section 7.1).
pub const ONLINE_MAX: usize = 2000;
/// The longest text of each kind, bytes (section 7.4).
pub const MESSAGE_LINE: usize = 120;
pub const MESSAGE_MAX: usize = 1200;
pub const NOTICE_TO_MAX: usize = 127;
pub const NOTICE_ALL_MAX: usize = 1999;
pub const NOTICE_ZONE_MAX: usize = 127;
pub const BANNER_MAX: usize = 127;
/// Rate limits (section 7.4): notices to many and banners per minute for the extension, messages and notices per second per player.
const MANY_PER_MINUTE: usize = 20;
const MESSAGES_PER_SECOND: u32 = 5;
const ORDERS_PER_SECOND: u32 = 50;

const PLAYER_META: &str = "script.player_meta";

/// What the extension has used of its rates.
#[derive(Default)]
pub struct Rates {
    many: VecDeque<Instant>,
    players: HashMap<u32, (Instant, u32)>,
    orders: HashMap<u32, (Instant, u32)>,
}

impl Rates {
    /// One more notice to many or banner; false when the minute's twenty are used.
    fn many(&mut self, now: Instant) -> bool {
        while self.many.front().is_some_and(|t| now.saturating_duration_since(*t) >= Duration::from_secs(60)) {
            self.many.pop_front();
        }
        if self.many.len() >= MANY_PER_MINUTE {
            return false;
        }
        self.many.push_back(now);
        true
    }

    /// One more order for `id`; false when the second's fifty are used.
    fn order(&mut self, id: u32, now: Instant) -> bool {
        if self.orders.len() > 2000 {
            self.orders.retain(|_, (t, _)| now.saturating_duration_since(*t) < Duration::from_secs(1));
        }
        let slot = self.orders.entry(id).or_insert((now, 0));
        if now.saturating_duration_since(slot.0) >= Duration::from_secs(1) {
            *slot = (now, 0);
        }
        if slot.1 >= ORDERS_PER_SECOND {
            return false;
        }
        slot.1 += 1;
        true
    }

    /// One more message or notice to `id`; false when the second's five are used.
    fn player(&mut self, id: u32, now: Instant) -> bool {
        if self.players.len() > 2000 {
            self.players.retain(|_, (t, _)| now.saturating_duration_since(*t) < Duration::from_secs(1));
        }
        let slot = self.players.entry(id).or_insert((now, 0));
        if now.saturating_duration_since(slot.0) >= Duration::from_secs(1) {
            *slot = (now, 0);
        }
        if slot.1 >= MESSAGES_PER_SECOND {
            return false;
        }
        slot.1 += 1;
        true
    }
}

/// A text a script sends to players: bytes 0x20 to 0x7E (others become `?`), at most `max` bytes, markup the clients can show.
fn clean(lua: &Lua, what: &str, value: &Value, max: usize) -> mlua::Result<String> {
    let bytes: Vec<u8> = match value {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Integer(i) => i.to_string().into_bytes(),
        Value::Number(n) => n.to_string().into_bytes(),
        other => return Err(script_error(lua, format!("{what}: the text is a string (got {})", other.type_name()))),
    };
    if bytes.len() > max {
        return Err(script_error(lua, format!("{what}: the text is {} bytes, at most {max}", bytes.len())));
    }
    let text: String = bytes.iter().map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '?' }).collect();
    if !shaiya_proto::chat::markup_is_safe(text.as_bytes()) {
        return Err(script_error(lua, format!("{what}: the text has a colour marker (<<…) that would crash the clients")));
    }
    Ok(text)
}

/// `text` cut into pieces of at most `max` bytes (the text is ASCII).
fn pieces(text: &str, max: usize) -> Vec<&str> {
    text.as_bytes().chunks(max).map(|c| std::str::from_utf8(c).unwrap_or("")).collect()
}

fn id_of(this: &Table) -> mlua::Result<u32> {
    this.raw_get::<u32>("id")
}

/// A player handle: `id` and `name` fields and the methods of section 7.1.
pub fn player_handle(lua: &Lua, p: &PlayerRef) -> mlua::Result<Table> {
    let handle = lua.create_table()?;
    handle.raw_set("id", p.id)?;
    handle.raw_set("name", p.name.as_str())?;
    handle.set_metatable(Some(lua.named_registry_value::<Table>(PLAYER_META)?))?;
    Ok(handle)
}

fn handle_of(lua: &Lua, view: &PlayerView) -> mlua::Result<Table> {
    player_handle(lua, &PlayerRef { id: view.id, name: view.name.clone() })
}

fn handles(lua: &Lua, views: &[PlayerView]) -> mlua::Result<Table> {
    let list = lua.create_table()?;
    for (i, v) in views.iter().enumerate() {
        list.raw_set(i + 1, handle_of(lua, v)?)?;
    }
    Ok(list)
}

/// The arguments of an event as the handlers get them (section 5).
pub fn event_args(lua: &Lua, world: &dyn ScriptWorld, payload: &super::event::Payload) -> mlua::Result<MultiValue> {
    use super::event::{Killer, Payload};
    let list = |ps: &[PlayerRef]| -> mlua::Result<Table> {
        let t = lua.create_table()?;
        for (i, p) in ps.iter().enumerate() {
            t.raw_set(i + 1, player_handle(lua, p)?)?;
        }
        Ok(t)
    };
    let mob = |m: &super::event::MobInfo| -> mlua::Result<Table> {
        let t = lua.create_table()?;
        t.raw_set("id", m.id)?;
        t.raw_set("uid", m.uid)?;
        // The zone does not know names; the world does.
        let name = if m.name.is_empty() { world.mob_name(m.id).unwrap_or_default() } else { m.name.clone() };
        t.raw_set("name", name)?;
        t.raw_set("level", m.level)?;
        t.raw_set("map", m.map)?;
        t.raw_set("x", m.x)?;
        t.raw_set("z", m.z)?;
        t.raw_set("boss", m.boss)?;
        t.raw_set("spawned", m.spawned.as_deref())?;
        Ok(t)
    };
    let h = |p: &PlayerRef| player_handle(lua, p).map(Value::Table);
    let values: Vec<Value> = match payload {
        Payload::Login { p } | Payload::Logout { p } => vec![h(p)?],
        Payload::MapEnter { p, map } | Payload::MapLeave { p, map } => vec![h(p)?, Value::Integer((*map).into())],
        Payload::Kill { p, mob: m, near } => vec![h(p)?, Value::Table(mob(m)?), Value::Table(list(near)?)],
        Payload::PvpKill { killer, victim, last_hit, near } => {
            vec![h(killer)?, h(victim)?, last_hit.as_ref().map(h).transpose()?.unwrap_or(Value::Nil), Value::Table(list(near)?)]
        }
        Payload::Death { victim, killer, pvp } => {
            let (by, who) = match killer {
                Killer::Player(p) => ("player", h(p)?),
                Killer::Monster(uid) => ("monster", Value::Integer((*uid).into())),
                Killer::Npc(uid) => ("npc", Value::Integer((*uid).into())),
            };
            vec![h(victim)?, Value::String(lua.create_string(by)?), who, Value::Boolean(*pvp)]
        }
    };
    Ok(MultiValue::from_iter(values))
}

/// The reads, messages and notices (sections 7.1 and 7.4), the player handle's metatable.
pub fn install(lua: &Lua, shared: &Arc<Shared>, game: &Table) -> mlua::Result<()> {
    let world = shared.world.clone();
    let methods = lua.create_table()?;

    let w = world.clone();
    game.set("player", lua.create_function(move |lua, id: Value| match id {
        Value::Integer(_) | Value::Number(_) => {
            let id = match id {
                Value::Integer(i) => i as f64,
                Value::Number(n) => n,
                _ => unreachable!(),
            };
            if id < 0.0 || id > f64::from(u32::MAX) || id.fract() != 0.0 {
                return Ok(Value::Nil);
            }
            match w.player(id as u32) {
                Some(v) => Ok(Value::Table(handle_of(lua, &v)?)),
                None => Ok(Value::Nil),
            }
        }
        _ => Err(script_error(lua, "game.player: the argument is a character id")),
    })?)?;
    let w = world.clone();
    game.set("find", lua.create_function(move |lua, name: Value| {
        let Value::String(name) = name else { return Err(script_error(lua, "game.find: the argument is a name")) };
        match w.find(&name.to_string_lossy()) {
            Some(v) => Ok(Value::Table(handle_of(lua, &v)?)),
            None => Ok(Value::Nil),
        }
    })?)?;
    let w = world.clone();
    game.set("online", lua.create_function(move |lua, ()| handles(lua, &w.online(ONLINE_MAX)))?)?;
    let w = world.clone();
    game.set("online_count", lua.create_function(move |_, ()| Ok(w.online_count()))?)?;
    let w = world.clone();
    game.set("item_name", lua.create_function(move |_, (t, i): (u8, u8)| Ok(w.item_name(t, i)))?)?;
    let w = world.clone();
    game.set("mob_name", lua.create_function(move |_, id: u16| Ok(w.mob_name(id)))?)?;
    let w = world.clone();
    game.set("commands", lua.create_function(move |lua, ()| {
        let list = lua.create_table()?;
        for (i, (word, help, staff)) in w.commands().into_iter().enumerate() {
            let t = lua.create_table()?;
            t.raw_set("word", word)?;
            t.raw_set("help", help)?;
            t.raw_set("staff", staff)?;
            list.raw_set(i + 1, t)?;
        }
        Ok(list)
    })?)?;

    // Notices to many and banners: twenty a minute for the extension.
    let (s, w) = (shared.clone(), world.clone());
    game.set("notice_all", lua.create_function(move |lua, text: Value| {
        let text = clean(lua, "game.notice_all", &text, NOTICE_ALL_MAX)?;
        many_allowed(lua, &s, "game.notice_all")?;
        w.notice_all(&text);
        Ok(())
    })?)?;
    let (s, w) = (shared.clone(), world.clone());
    game.set("notice_faction", lua.create_function(move |lua, (faction, text): (Value, Value)| {
        let country = match &faction {
            Value::String(f) if f.as_bytes() == b"light" => 0u8,
            Value::String(f) if f.as_bytes() == b"fury" => 1u8,
            _ => return Err(script_error(lua, "game.notice_faction: the faction is \"light\" or \"fury\"")),
        };
        let text = clean(lua, "game.notice_faction", &text, NOTICE_ZONE_MAX)?;
        many_allowed(lua, &s, "game.notice_faction")?;
        w.notice_faction(country, &text);
        Ok(())
    })?)?;
    let (s, w) = (shared.clone(), world.clone());
    game.set("notice_map", lua.create_function(move |lua, (map, text): (u16, Value)| {
        let text = clean(lua, "game.notice_map", &text, NOTICE_ZONE_MAX)?;
        many_allowed(lua, &s, "game.notice_map")?;
        w.notice_map(map, &text);
        Ok(())
    })?)?;
    let (s, w) = (shared.clone(), world.clone());
    game.set("banner", lua.create_function(move |lua, (text, name): (Value, Option<Value>)| {
        let text = clean(lua, "game.banner", &text, BANNER_MAX)?;
        let name = match name {
            Some(n) => clean(lua, "game.banner", &n, 20)?,
            None => "Notice".to_owned(),
        };
        many_allowed(lua, &s, "game.banner")?;
        w.banner(&name, &text);
        Ok(())
    })?)?;

    // Monsters (section 7.4): at most 50 a call and 300 alive per extension; they live `lifetime` seconds.
    let (s, w) = (shared.clone(), world.clone());
    game.set("spawn", lua.create_function(move |lua, (mob, count, map, x, z, opts): (Value, Value, Value, Value, Value, Option<Table>)| {
        let num = |what: &str, v: &Value, low: f64, high: f64| -> mlua::Result<f64> {
            let n = match v {
                Value::Integer(i) => *i as f64,
                Value::Number(n) if n.is_finite() => *n,
                _ => return Err(script_error(lua, format!("game.spawn {what}: a number is wanted"))),
            };
            if n < low || n > high {
                return Err(script_error(lua, format!("game.spawn {what}: from {low} to {high} is wanted (got {n})")));
            }
            Ok(n)
        };
        let mob_id = num("mob id", &mob, 1.0, 65535.0)? as u16;
        let count = num("count", &count, 1.0, crate::zone::SPAWNS_PER_CALL as f64)? as usize;
        let map = num("map", &map, 0.0, 65535.0)? as u16;
        let (x, z) = (num("x", &x, -1.0e6, 1.0e6)? as f32, num("z", &z, -1.0e6, 1.0e6)? as f32);
        let Some(opts) = opts else { return Err(script_error(lua, "game.spawn: the options table with lifetime (10 to 3600 seconds) is required")) };
        let (mut radius, mut lifetime, mut exp, mut drops) = (10u32, None, true, true);
        for pair in opts.pairs::<Value, Value>() {
            let (key, value) = pair?;
            match (&key, &value) {
                (Value::String(k), v) if k.as_bytes() == b"radius" => radius = num("radius", v, 0.0, 50.0)? as u32,
                (Value::String(k), v) if k.as_bytes() == b"lifetime" => lifetime = Some(num("lifetime", v, 10.0, 3600.0)? as u32),
                (Value::String(k), Value::Boolean(b)) if k.as_bytes() == b"exp" => exp = *b,
                (Value::String(k), Value::Boolean(b)) if k.as_bytes() == b"drops" => drops = *b,
                _ => return Err(script_error(lua, "game.spawn: the options are radius, lifetime (required), exp and drops")),
            }
        }
        let Some(lifetime) = lifetime else { return Err(script_error(lua, "game.spawn: lifetime (10 to 3600 seconds) is required")) };
        let ids = w.spawn(&s.name, mob_id, count, map, x, z, radius, lifetime, exp, drops);
        let list = lua.create_table()?;
        for (i, id) in ids.iter().enumerate() {
            list.raw_set(i + 1, *id)?;
        }
        Ok(list)
    })?)?;
    let uids = |lua: &Lua, what: &str, list: &Table| -> mlua::Result<Vec<u32>> {
        let mut out = Vec::new();
        for v in list.sequence_values::<Value>() {
            match v? {
                Value::Integer(i) if i >= 0 => out.push(i as u32),
                Value::Number(n) if n >= 0.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX) => out.push(n as u32),
                _ => return Err(script_error(lua, format!("{what}: a list of the ids game.spawn gave"))),
            }
        }
        Ok(out)
    };
    let (s, w) = (shared.clone(), world.clone());
    game.set("despawn", lua.create_function(move |lua, list: Table| Ok(w.despawn(&s.name, &uids(lua, "game.despawn", &list)?)))?)?;
    let (s, w) = (shared.clone(), world.clone());
    game.set("alive", lua.create_function(move |lua, list: Table| Ok(w.alive(&s.name, &uids(lua, "game.alive", &list)?)))?)?;

    // The player handle.
    let reads = |name: &str, f: fn(&Lua, &PlayerView) -> mlua::Result<MultiValue>| -> mlua::Result<()> {
        let w = world.clone();
        methods.set(name, lua.create_function(move |lua, this: Table| match w.player(id_of(&this)?) {
            Some(view) => f(lua, &view),
            None => Ok(MultiValue::from_iter([Value::Nil])),
        })?)
    };
    reads("level", |_, v| Ok(MultiValue::from_iter([Value::Integer(v.level.into())])))?;
    reads("faction", |lua, v| Ok(MultiValue::from_iter([Value::String(lua.create_string(if v.country == 0 { "light" } else { "fury" })?)])))?;
    reads("map", |_, v| Ok(MultiValue::from_iter([Value::Integer(v.map.into())])))?;
    reads("position", |_, v| Ok(MultiValue::from_iter(v.position.map(|c| Value::Number(f64::from(c))))))?;
    reads("kills", |_, v| Ok(MultiValue::from_iter([Value::Integer(v.kills.into())])))?;
    reads("job", |_, v| Ok(MultiValue::from_iter([Value::Integer(v.job.into())])))?;
    reads("staff", |_, v| Ok(MultiValue::from_iter([Value::Integer(v.staff.into())])))?;
    reads("guild", |_, v| Ok(MultiValue::from_iter([v.guild.map_or(Value::Nil, |g| Value::Integer(g.into()))])))?;
    let w = world.clone();
    methods.set("online", lua.create_function(move |_, this: Table| Ok(w.player(id_of(&this)?).is_some()))?)?;
    let w = world.clone();
    methods.set("party", lua.create_function(move |lua, this: Table| {
        let id = id_of(&this)?;
        let members: Vec<PlayerView> = w.party(id).into_iter().filter_map(|m| w.player(m)).collect();
        if members.is_empty() {
            return handles(lua, &[]);
        }
        handles(lua, &members)
    })?)?;
    // There is no client handshake yet (client research entry 23): no feature is known.
    methods.set("has_feature", lua.create_function(|_, (_this, _name): (Table, Value)| Ok(false))?)?;

    let (s, w) = (shared.clone(), world.clone());
    methods.set("message", lua.create_function(move |lua, (this, text): (Table, Value)| {
        let text = clean(lua, "p:message", &text, MESSAGE_MAX)?;
        let id = id_of(&this)?;
        player_allowed(lua, &s, id, "p:message")?;
        if w.player(id).is_some() {
            for piece in pieces(&text, MESSAGE_LINE) {
                w.whisper(id, piece);
            }
        }
        Ok(())
    })?)?;
    let (s, w) = (shared.clone(), world.clone());
    methods.set("notice", lua.create_function(move |lua, (this, text): (Table, Value)| {
        let text = clean(lua, "p:notice", &text, NOTICE_TO_MAX)?;
        let id = id_of(&this)?;
        player_allowed(lua, &s, id, "p:notice")?;
        if w.player(id).is_some() {
            w.notice_to(id, &text);
        }
        Ok(())
    })?)?;

    // Stored values (section 7.5): a character's own while it is loaded (from the login to the end of its `logout` handlers).
    let s = shared.clone();
    methods.set("get", lua.create_function(move |lua, (this, key): (Table, String)| {
        match s.store.get(&s.name, id_of(&this)?, &key) {
            Some(v) => super::store::to_lua(lua, &v),
            None => Ok(Value::Nil),
        }
    })?)?;
    let s = shared.clone();
    methods.set("set", lua.create_function(move |lua, (this, key, value): (Table, String, Value)| {
        let v = super::store::from_lua(&value).map_err(|e| script_error(lua, format!("p:set({key:?}): {e}")))?;
        s.store.set(&s.name, id_of(&this)?, &key, v).map_err(|e| script_error(lua, format!("p:set({key:?}): {e}")))
    })?)?;
    let s = shared.clone();
    game.set("get", lua.create_function(move |lua, key: String| match s.store.get_global(&s.name, &key) {
        Some(v) => super::store::to_lua(lua, &v),
        None => Ok(Value::Nil),
    })?)?;
    let s = shared.clone();
    game.set("set", lua.create_function(move |lua, (key, value): (String, Value)| {
        let v = super::store::from_lua(&value).map_err(|e| script_error(lua, format!("game.set({key:?}): {e}")))?;
        s.store.set_global(&s.name, &key, v).map_err(|e| script_error(lua, format!("game.set({key:?}): {e}")))
    })?)?;

    // Orders (section 4.4): left for the player's own connection; the callback hears how it came out, later, on the script thread.
    use super::event::{Order, Outcome, Queued};
    let whole = |lua: &Lua, what: &str, v: &Value, low: f64, high: f64| -> mlua::Result<f64> {
        let n = match v {
            Value::Integer(i) => *i as f64,
            Value::Number(n) => *n,
            _ => return Err(script_error(lua, format!("{what}: a number is wanted"))),
        };
        if n.fract() != 0.0 || n < low || n > high {
            return Err(script_error(lua, format!("{what}: a whole number from {low} to {high} is wanted (got {n})")));
        }
        Ok(n)
    };
    let queue = |lua: &Lua, shared: &Shared, this: &Table, what: &str, order: Order, callback: Option<Value>, failed: Outcome| -> mlua::Result<()> {
        let id = id_of(this)?;
        if !shared.rates.lock().unwrap_or_else(|e| e.into_inner()).order(id, Instant::now()) {
            return Err(script_error(lua, format!("{what}: rate limit, at most {ORDERS_PER_SECOND} orders a second for one player")));
        }
        let callback = match callback {
            None | Some(Value::Nil) => None,
            Some(Value::Function(f)) => {
                let cb = shared.next_callback.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                super::api::keep_callback(lua, cb, f)?;
                Some(cb)
            }
            Some(other) => return Err(script_error(lua, format!("{what}: the callback is a function (got {})", other.type_name()))),
        };
        let queued = Queued { extension: shared.name.clone(), callback, order };
        if !shared.world.order(id, queued) {
            // Not online here: answered at once, but behind the handler that asked (never inside it).
            if let Some(cb) = callback {
                shared.world.answer(&shared.name, cb, failed);
            }
        }
        Ok(())
    };
    let queue = Arc::new(queue);
    let (s, q) = (shared.clone(), queue.clone());
    methods.set("give_item", lua.create_function(move |lua, (this, t, i, n, cb): (Table, Value, Value, Value, Option<Value>)| {
        let order = Order::GiveItem {
            item_type: whole(lua, "p:give_item type", &t, 1.0, 255.0)? as u8,
            type_id: whole(lua, "p:give_item type_id", &i, 1.0, 255.0)? as u8,
            count: whole(lua, "p:give_item count", &n, 1.0, 255.0)? as u8,
        };
        q(lua, &s, &this, "p:give_item", order, cb, Outcome::Count(0))
    })?)?;
    let (s, q) = (shared.clone(), queue.clone());
    methods.set("take_item", lua.create_function(move |lua, (this, t, i, n, cb): (Table, Value, Value, Value, Option<Value>)| {
        let order = Order::TakeItem {
            item_type: whole(lua, "p:take_item type", &t, 1.0, 255.0)? as u8,
            type_id: whole(lua, "p:take_item type_id", &i, 1.0, 255.0)? as u8,
            count: whole(lua, "p:take_item count", &n, 1.0, 255.0)? as u8,
        };
        q(lua, &s, &this, "p:take_item", order, cb, Outcome::Done(false))
    })?)?;
    let (s, q) = (shared.clone(), queue.clone());
    methods.set("give_gold", lua.create_function(move |lua, (this, n, cb): (Table, Value, Option<Value>)| {
        let order = Order::GiveGold(whole(lua, "p:give_gold", &n, 1.0, 2_147_483_647.0)? as u32);
        q(lua, &s, &this, "p:give_gold", order, cb, Outcome::Count(0))
    })?)?;
    let (s, q) = (shared.clone(), queue.clone());
    methods.set("take_gold", lua.create_function(move |lua, (this, n, cb): (Table, Value, Option<Value>)| {
        let order = Order::TakeGold(whole(lua, "p:take_gold", &n, 1.0, 2_147_483_647.0)? as u32);
        q(lua, &s, &this, "p:take_gold", order, cb, Outcome::Done(false))
    })?)?;
    let (s, q) = (shared.clone(), queue.clone());
    methods.set("add_buff", lua.create_function(move |lua, (this, skill, level, cb): (Table, Value, Value, Option<Value>)| {
        let order = Order::AddBuff {
            skill: whole(lua, "p:add_buff skill", &skill, 1.0, 65535.0)? as u16,
            level: whole(lua, "p:add_buff level", &level, 1.0, 255.0)? as u8,
        };
        q(lua, &s, &this, "p:add_buff", order, cb, Outcome::Done(false))
    })?)?;
    let (s, q) = (shared.clone(), queue.clone());
    methods.set("teleport", lua.create_function(move |lua, (this, map, x, y, z, cb): (Table, Value, Value, Value, Value, Option<Value>)| {
        let coord = |v: &Value, what: &str| -> mlua::Result<f32> {
            match v {
                Value::Integer(i) => Ok(*i as f32),
                Value::Number(n) if n.is_finite() => Ok(*n as f32),
                _ => Err(script_error(lua, format!("p:teleport {what}: a number is wanted"))),
            }
        };
        let order = Order::Teleport {
            map: whole(lua, "p:teleport map", &map, 0.0, 65535.0)? as u16,
            position: [coord(&x, "x")?, coord(&y, "y")?, coord(&z, "z")?],
        };
        q(lua, &s, &this, "p:teleport", order, cb, Outcome::Done(false))
    })?)?;

    let meta = lua.create_table()?;
    meta.raw_set("__index", methods)?;
    meta.raw_set(
        "__eq",
        lua.create_function(|_, (a, b): (Table, Table)| Ok(matches!((a.raw_get::<u32>("id"), b.raw_get::<u32>("id")), (Ok(x), Ok(y)) if x == y)))?,
    )?;
    meta.raw_set("__tostring", lua.create_function(|_, this: Table| Ok(this.raw_get::<String>("name").unwrap_or_default()))?)?;
    meta.raw_set("__metatable", false)?;
    lua.set_named_registry_value(PLAYER_META, meta)?;
    Ok(())
}

fn many_allowed(lua: &Lua, shared: &Shared, what: &str) -> mlua::Result<()> {
    if shared.rates.lock().unwrap_or_else(|e| e.into_inner()).many(Instant::now()) {
        Ok(())
    } else {
        Err(script_error(lua, format!("{what}: rate limit, at most {MANY_PER_MINUTE} notices and banners a minute")))
    }
}

fn player_allowed(lua: &Lua, shared: &Shared, id: u32, what: &str) -> mlua::Result<()> {
    if shared.rates.lock().unwrap_or_else(|e| e.into_inner()).player(id, Instant::now()) {
        Ok(())
    } else {
        Err(script_error(lua, format!("{what}: rate limit, at most {MESSAGES_PER_SECOND} messages and notices a second to one player")))
    }
}

/// The type the API keeps its rates in (a field of [`Shared`]).
pub type RateCell = Mutex<Rates>;

/// The world a state acts on, when `check` or a unit test gives none.
pub fn null_world() -> Arc<dyn ScriptWorld> {
    Arc::new(super::world::NullWorld)
}
