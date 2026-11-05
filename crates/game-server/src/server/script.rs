//! The server's side of the extensions' scripts (`docs/plans/ext-5-lua.md`): the world the API acts on (`ContextWorld`), loading and
//! starting the scripts, the `!word` commands, the events the connection raises (login, logout, zone entry and exit), the console's
//! `scripts`. The scripts themselves run on their own thread (`crate::script::runner`) or, for a point, on the asking task; nothing here
//! runs one while a lock is held (section 4.3).

use std::sync::Weak;

use super::*;
use crate::script::event::{Job, Order, Outcome, Payload, PlayerRef};
use crate::script::sources::{Resolution, ScriptSources, resolve_options};
use crate::script::world::{PlayerView, ScriptWorld};
use crate::script::{Event, ExtInput, LoadContext, ScriptSet};

/// A null-terminated chat text.
fn zeroed(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

/// The world of the scripts on this server. It holds the context weakly: the scripts are the context's, not the other way round.
pub struct ContextWorld<S> {
    ctx: Weak<Context<S>>,
}

impl<S: Backend> ScriptWorld for ContextWorld<S> {
    fn player(&self, id: u32) -> Option<PlayerView> {
        self.ctx.upgrade()?.zones.script_view(id)
    }

    fn find(&self, name: &str) -> Option<PlayerView> {
        self.ctx.upgrade()?.zones.script_find(name)
    }

    fn online(&self, max: usize) -> Vec<PlayerView> {
        self.ctx.upgrade().map(|c| c.zones.script_online(max)).unwrap_or_default()
    }

    fn online_count(&self) -> usize {
        self.ctx.upgrade().map_or(0, |c| c.zones.script_online_count())
    }

    fn party(&self, id: u32) -> Vec<u32> {
        self.ctx.upgrade().map(|c| c.zones.script_party(id)).unwrap_or_default()
    }

    fn whisper(&self, id: u32, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_to_char(id, &chat::whisper(false, SERVER_NAME, &zeroed(text)));
        }
    }

    fn notice_to(&self, id: u32, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_to_char(id, &ap::notice(crate::admin::NoticeKind::To.opcode(), &zeroed(text)));
        }
    }

    fn notice_all(&self, text: &str) {
        if let (Some(c), Some(packet)) = (self.ctx.upgrade(), ap::world_notice(text)) {
            c.zones.send_world(&packet);
        }
    }

    fn notice_faction(&self, country: u8, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_country(country, None, &ap::notice(crate::admin::NoticeKind::Country.opcode(), &zeroed(text)));
        }
    }

    fn notice_map(&self, map: u16, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_zone(map, &ap::notice(crate::admin::NoticeKind::Zone.opcode(), &zeroed(text)));
        }
    }

    fn banner(&self, name: &str, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_all(&chat::by_name(chat::OP_ALL_SERVER, name.as_bytes(), &zeroed(text)));
        }
    }

    fn item_name(&self, item_type: u8, type_id: u8) -> Option<String> {
        let c = self.ctx.upgrade()?;
        c.names.items.get(&(item_type, type_id)).cloned()
    }

    fn mob_name(&self, mob_id: u16) -> Option<String> {
        let c = self.ctx.upgrade()?;
        c.names.mobs.get(&mob_id).cloned()
    }

    fn tell_staff(&self, text: &str) {
        let Some(c) = self.ctx.upgrade() else { return };
        for id in c.zones.script_staff_upto(c.config.console_level) {
            for piece in text.as_bytes().chunks(120) {
                c.zones.send_to_char(id, &chat::whisper(false, SERVER_NAME, &zeroed(&String::from_utf8_lossy(piece))));
            }
        }
    }

    fn spawn(&self, extension: &str, mob_id: u16, count: usize, map: u16, x: f32, z: f32, radius: u32, lifetime: u32, exp: bool, drops: bool) -> Vec<u32> {
        let Some(c) = self.ctx.upgrade() else { return Vec::new() };
        if !c.data.world.is_safe_pos(map, [x, 0.0, z]) {
            return Vec::new();
        }
        let live = c.data.live();
        let mut rng = rand::rng();
        use rand::Rng;
        // The height of the nearest player on the map (the API gives x and z only), else 0.
        let y = c
            .zones
            .players_of(map)
            .into_iter()
            .map(|(_, _, _, at)| at)
            .min_by(|a, b| crate::zone::distance(*a, [x, a[1], z]).total_cmp(&crate::zone::distance(*b, [x, b[1], z])))
            .map_or(0.0, |at| at[1]);
        c.zones.ext_spawn(
            extension,
            &c.data.world,
            &live.mobs,
            mob_id,
            count,
            map,
            [x, y, z],
            radius,
            Duration::from_secs(u64::from(lifetime)),
            (exp, drops),
            Instant::now(),
            &mut || rng.random::<u32>() & 0x7fff,
        )
    }

    fn despawn(&self, extension: &str, uids: &[u32]) -> usize {
        self.ctx.upgrade().map_or(0, |c| c.zones.ext_despawn(extension, uids, Instant::now()))
    }

    fn alive(&self, extension: &str, uids: &[u32]) -> usize {
        self.ctx.upgrade().map_or(0, |c| c.zones.ext_alive(extension, uids))
    }

    fn spawn_count(&self, extension: &str) -> usize {
        self.ctx.upgrade().map_or(0, |c| c.zones.ext_spawn_count(extension))
    }

    fn order(&self, id: u32, queued: crate::script::event::Queued) -> bool {
        self.ctx.upgrade().is_some_and(|c| c.zones.script_order(id, queued))
    }

    fn answer(&self, extension: &str, callback: u64, outcome: crate::script::event::Outcome) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.script_job(Job::OrderDone { extension: extension.to_owned(), callback, outcome });
        }
    }

    fn commands(&self) -> Vec<(String, String, u8)> {
        let Some(c) = self.ctx.upgrade() else { return Vec::new() };
        let set = c.zones.ext_scripts();
        let mut out: Vec<(String, String, u8)> = set
            .extensions()
            .iter()
            .filter(|e| !e.is_disabled())
            .flat_map(|e| e.commands().into_iter().map(|(word, meta)| (word, meta.help, meta.staff)))
            .collect();
        out.sort();
        out
    }
}

impl<S: Backend> Context<S> {
    /// Binds the scripts' world to this context; the context must live in an `Arc` (called by `start_scripts`).
    pub fn attach_scripts(self: &Arc<Self>) {
        let _ = self.script_me.set(Arc::downgrade(self));
    }

    /// The world the scripts act on; a world with nobody in it until `attach_scripts` ran.
    pub fn script_world(&self) -> Arc<dyn ScriptWorld> {
        match self.script_me.get() {
            Some(weak) => Arc::new(ContextWorld { ctx: weak.clone() }),
            None => Arc::new(crate::script::world::NullWorld),
        }
    }

    /// Loads the scripts of `sources` with the options resolved in `resolution` (section 9.1): every enabled extension in a state of its
    /// own, an extension that does not load listed in the set's `failed`. CPU work: it runs the scripts' top levels.
    pub fn load_script_set(&self, sources: &ScriptSources, resolution: &Resolution) -> ScriptSet {
        let inputs: Vec<ExtInput> = resolution.inputs(sources);
        let weak = self.script_me.get().cloned();
        let exists: Arc<dyn Fn(u8, u8) -> bool + Send + Sync> = match weak {
            Some(weak) => Arc::new(move |t, i| weak.upgrade().is_some_and(|c| c.data.items.get(t, i).is_some())),
            None => Arc::new(|_, _| true),
        };
        // The extensions' own values come in with the scripts, the writes not sent yet over them (section 7.6).
        let store = self.zones.script_store();
        store.retain_globals(&sources.extensions.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
        for e in &sources.extensions {
            store.set_globals(&e.name, e.globals.clone());
        }
        let base = LoadContext { item_exists: Some(exists), world: Some(self.script_world()), store: Some(store), ..LoadContext::default() };
        ScriptSet::load(&inputs, base, self.config.script_memory_mb)
    }

    /// Puts `set` in force and starts the script thread (once): the start of the server (`main`) and the tests call it.
    pub fn start_scripts(self: &Arc<Self>, set: ScriptSet) {
        self.attach_scripts();
        self.zones.start_script_thread();
        self.zones.set_ext_scripts(set);
    }
}

/// The scripts of the database and the file, loaded for the start of the server (section 9.1): an error in an option stops the start
/// (`Err`), an extension that does not load runs without scripts (a line in the log).
pub fn load_at_start<S: Backend>(ctx: &Arc<Context<S>>, sources: &ScriptSources) -> anyhow::Result<ScriptSet> {
    ctx.attach_scripts();
    let resolution = resolve_options(sources)?;
    for warning in &resolution.warnings {
        tracing::warn!("{warning}");
    }
    let set = ctx.load_script_set(sources, &resolution);
    for (name, why) in set.failed() {
        tracing::error!(extension = %name, "scripts not loaded: {why}");
    }
    for ext in set.extensions() {
        let s = ext.summary();
        tracing::info!(extension = %ext.name, handlers = s.handlers(), timers = s.timers, "scripts loaded");
    }
    Ok(set)
}

// ------------------------------------------------------------------------------------------------------------ the commands

/// `!word args` (`docs/plans/ext-5-lua.md` section 7.2): after an optional 8-byte colour marker (`<<NNNcCC`, which the client adds with
/// a chat colour), a `!`, a word of lower case letters, digits and `_` (any case is read as lower case), then a space or the end. The
/// text is cut at its first zero. `! name text` (a space after the `!`) is the client's whisper prefix, not a command.
pub fn parse_command(text: &[u8]) -> Option<(String, String)> {
    let text = text.split(|&b| b == 0).next().unwrap_or(text);
    let rest = match text.strip_prefix(b"<<") {
        Some(_) => {
            let marker = text.get(..8)?;
            let digits = |b: &[u8]| !b.is_empty() && b.iter().all(u8::is_ascii_digit);
            if !(digits(&marker[2..5]) && marker[5] == b'c' && digits(&marker[6..8])) {
                return None;
            }
            &text[8..]
        }
        None => text,
    };
    let rest = rest.strip_prefix(b"!")?;
    let end = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
    let word = &rest[..end];
    if word.is_empty() || !word.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_') {
        return None;
    }
    let word = String::from_utf8_lossy(word).to_ascii_lowercase();
    let args = String::from_utf8_lossy(&rest[end..]).trim().to_owned();
    Some((word, args))
}

/// Whether the command registered for staff levels `1..=staff` (0: everyone) may be used by a player whose staff level is `level` (0 for
/// a player, 1 to 10 for staff).
pub fn may_use(staff: u8, level: u8) -> bool {
    staff == 0 || (1..=staff).contains(&level)
}

/// A chat line that is a command a script registered: queued for the extension, and `true` (the line is not chat). Anything else,
/// an unregistered word, a staff command typed by a player, a disabled extension's, stays chat (`false`).
pub(super) fn command<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, text: &[u8]) -> bool {
    let set = ctx.zones.ext_scripts();
    if !set.has_commands() || !text.contains(&b'!') {
        return false;
    }
    let Some((word, args)) = parse_command(text) else { return false };
    let Some((ext, meta)) = set.command(&word) else { return false };
    let level = if state.staff { state.staff_level } else { 0 };
    if ext.is_disabled() || !may_use(meta.staff, level) {
        return false;
    }
    tracing::debug!(id = character.id, extension = %ext.name, word, "script command");
    ctx.zones.script_job(Job::Command { extension: ext.name.clone(), word, p: PlayerRef { id: character.id, name: character.name.clone() }, args });
    true
}

// ------------------------------------------------------------------------------------------------------------ the events

/// What the connection tells the scripts about itself: the character, which of its events were raised.
#[derive(Debug, Default)]
pub(super) struct Track {
    pub id: u32,
    pub name: String,
    /// `login` was raised (and `logout` is due).
    pub login: bool,
    /// The zone `map_enter` was raised for (and `map_leave` is due).
    pub map: Option<u16>,
    /// The generation of the stored values loaded for this login (they leave the cache after `logout`).
    pub session: Option<u64>,
}

impl Track {
    fn player(&self) -> PlayerRef {
        PlayerRef { id: self.id, name: self.name.clone() }
    }
}

/// The first zone entry of a login: the character's stored values come in before `login` is queued (one query), for the extensions
/// that are loaded. Without scripts nothing is read.
pub(super) async fn load_values<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    if state.script.session.is_some() || state.script.login {
        return;
    }
    let set = ctx.zones.ext_scripts();
    if set.extensions().is_empty() {
        return;
    }
    match ctx.store.script_values(state.script.id).await {
        Ok(rows) => {
            let rows = rows.into_iter().filter(|(e, _, _)| set.extensions().iter().any(|x| x.name == *e)).collect();
            state.script.session = Some(ctx.zones.script_store().begin_char(state.script.id, rows));
        }
        // Without them a `set` would write over what the database has: the player's values stay unloaded (reads nil, sets fail).
        Err(error) => tracing::warn!(%error, id = state.script.id, "could not load the character's script values"),
    }
}

/// The character entered a zone: `login` the first time, `map_enter` every time (section 5).
pub(super) fn entered<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    let map = state.map;
    if !state.script.login {
        state.script.login = true;
        let level = if state.staff { Some(state.staff_level) } else { None };
        ctx.zones.set_script_staff(state.script.id, level);
        ctx.zones.raise(Event::Login, || Payload::Login { p: state.script.player() });
    }
    state.script.map = Some(map);
    ctx.zones.raise(Event::MapEnter, || Payload::MapEnter { p: state.script.player(), map });
}

/// The character left a zone (`leave_zone`).
pub(super) fn left_zone<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    if let Some(map) = state.script.map.take() {
        ctx.zones.raise(Event::MapLeave, || Payload::MapLeave { p: state.script.player(), map });
    }
}

/// The character is out of the world: `map_leave` for the zone it was in, then `logout` (once, only after a `login`).
pub(super) fn gone<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    left_zone(ctx, state);
    if std::mem::take(&mut state.script.login) {
        ctx.zones.set_script_staff(state.script.id, None);
        ctx.zones.raise(Event::Logout, || Payload::Logout { p: state.script.player() });
    }
    // Orders still waiting are answered "no" (section 4.4).
    for queued in ctx.zones.take_script_orders(state.script.id) {
        if let Some(callback) = queued.callback {
            let outcome = match queued.order {
                Order::GiveItem { .. } | Order::GiveGold(_) => Outcome::Count(0),
                _ => Outcome::Done(false),
            };
            ctx.zones.script_job(Job::OrderDone { extension: queued.extension, callback, outcome });
        }
    }
    // The stored values leave the cache behind the `logout` handlers (the writes stay queued).
    if let Some(session) = state.script.session.take() {
        ctx.zones.script_forget(state.script.id, session);
    }
}

// ------------------------------------------------------------------------------------------------------ the writer

/// The writes the scripts queued, sent in one transaction; put back (and warned about) when the database refuses. Called every 2 seconds
/// by the writer task and at shutdown.
pub async fn flush_script_values<S: Backend>(ctx: &Context<S>) -> bool {
    let store = ctx.zones.script_store();
    let batch = store.take_pending();
    if batch.is_empty() {
        return true;
    }
    match ctx.store.write_script_values(&batch).await {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(%error, writes = batch.len(), "could not write the scripts' stored values; they are tried again");
            store.requeue(batch);
            false
        }
    }
}

/// The writer task (section 7.6): every 2 seconds the values the scripts set since are written.
pub async fn script_writer<S: Backend>(ctx: Arc<Context<S>>) {
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        flush_script_values(&ctx).await;
    }
}

// ------------------------------------------------------------------------------------------------------------ the console

/// `scripts`: one line per extension with scripts, `<name> <state> <handlers> timers <n> spawns <n> faults <n>`.
pub(super) fn answer<S>(ctx: &Context<S>) -> Vec<String> {
    let set = ctx.zones.ext_scripts();
    if set.is_empty() {
        return vec!["no scripts".to_owned()];
    }
    let mut out = Vec::new();
    for ext in set.extensions() {
        let s = ext.summary();
        let state = match ext.disabled_reason() {
            Some(reason) => format!("disabled ({reason})"),
            None => "running".to_owned(),
        };
        out.push(cut_line(format!("{} {state} {} handlers timers {} spawns {} faults {}", ext.name, s.handlers(), s.timers, ext.spawn_count(), ext.fault_count())));
    }
    for (name, why) in set.failed() {
        out.push(cut_line(format!("{name} not loaded ({why})")));
    }
    out
}

/// A console line cut to the whisper width.
fn cut_line(mut line: String) -> String {
    const WIDTH: usize = 120;
    if line.len() > WIDTH {
        let mut end = WIDTH - 1;
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        line.truncate(end);
        line.push('…');
    }
    line
}

// ------------------------------------------------------------------------------------------------------------ the orders

/// The orders scripts left for this connection's character, carried out with the code the GM orders and the item paths use
/// (section 4.4); each result goes back to the callback as a job, so no script is ever re-entered from here.
pub(super) async fn carry_out<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
) -> Result<()> {
    for queued in ctx.zones.take_script_orders(character.id) {
        let outcome = one(conn, ctx, player, character, state, &queued.order).await?;
        tracing::debug!(id = character.id, extension = %queued.extension, order = ?queued.order, ?outcome, "script order");
        if let Some(callback) = queued.callback {
            ctx.zones.script_job(Job::OrderDone { extension: queued.extension, callback, outcome });
        }
    }
    Ok(())
}

async fn one<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    order: &Order,
) -> Result<Outcome> {
    let id = character.id;
    // Nothing is done for a character that is leaving (a logout under way).
    if state.logout.is_some() {
        return Ok(match order {
            Order::GiveItem { .. } | Order::GiveGold(_) => Outcome::Count(0),
            _ => Outcome::Done(false),
        });
    }
    Ok(match *order {
        Order::GiveItem { item_type, type_id, count } => {
            let Some(def) = ctx.data.items.get(item_type, type_id).copied() else { return Ok(Outcome::Count(0)) };
            Outcome::Count(u32::from(admin::give_items(conn, ctx, character, state, &def, count).await?))
        }
        Order::TakeItem { item_type, type_id, count } => {
            // All or nothing, from the bags (not the warehouse, not what is worn).
            let mut units: Vec<_> = state
                .inventory
                .items()
                .filter(|i| i.item_type == item_type && i.type_id == type_id && i.bag != 0 && state.inventory.is_bag((i.bag, i.slot)))
                .map(|i| (i.bag, i.slot, i.count))
                .collect();
            units.sort();
            if units.iter().map(|u| u32::from(u.2)).sum::<u32>() < u32::from(count) {
                return Ok(Outcome::Done(false));
            }
            let mut left = count;
            let mut changes = Vec::new();
            for (bag, slot, have) in units {
                if left == 0 {
                    break;
                }
                let take = left.min(have);
                changes.push(if take == have { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: have - take } });
                left -= take;
            }
            let Some(rows) = ctx.store.trade(id, 0, &changes).await? else { bail!("the database refused taking {changes:?}") };
            record::removed(ctx, character, state, &changes, &rows);
            for change in &changes {
                state.inventory.apply(change);
            }
            super::send_item_changes(conn, state, &changes, false).await?;
            Outcome::Done(true)
        }
        Order::GiveGold(n) => {
            // At most 2^32 - 1 in hand; the rest is not given.
            let given = n.min(u32::MAX - state.money);
            if given == 0 {
                return Ok(Outcome::Count(0));
            }
            if ctx.store.trade(id, i64::from(given), &[]).await?.is_none() {
                return Ok(Outcome::Count(0));
            }
            state.money += given;
            conn.send(w::set_money(state.money)).await?;
            Outcome::Count(given)
        }
        Order::TakeGold(n) => {
            if state.money < n || ctx.store.trade(id, -i64::from(n), &[]).await?.is_none() {
                return Ok(Outcome::Done(false));
            }
            state.money -= n;
            conn.send(w::set_money(state.money)).await?;
            Outcome::Done(true)
        }
        Order::AddBuff { skill, level } => {
            let Some(def) = ctx.data.skills.get(skill, level).copied() else { return Ok(Outcome::Done(false)) };
            if def.type_effect != skills::BUFF || state.dead || ctx.zones.item_skill_refused(state.map, id, &def) {
                return Ok(Outcome::Done(false));
            }
            super::items::buff_self(conn, ctx, character, state, player.faction.map_or(0, |f| f.country), &def).await?;
            Outcome::Done(true)
        }
        Order::Teleport { map, position } => {
            // As a GM's summon: not while dead, entering or a move under way; only to a map and spot the world has.
            if state.dead || state.item_move.is_some() || !state.entered || !ctx.data.world.is_safe_pos(map, position) {
                return Ok(Outcome::Done(false));
            }
            super::move_player(conn, ctx, state, id, map, position, None).await?;
            Outcome::Done(true)
        }
    })
}

// ------------------------------------------------------------------------------------------------------------ the points
// (`docs/plans/ext-5-lua.md` section 6). Each runs on the asking connection's task with no lock of the zones, the context or the
// connection held: the callers are `async` functions of the connection owning `state`, and say so where they ask.

use crate::rules::Refusal;
use crate::script::{Point, act};
use mlua::{Lua, MultiValue, Value};

/// The character as a script handle.
pub(super) fn me(character: &PlayCharacter) -> PlayerRef {
    PlayerRef { id: character.id, name: character.name.clone() }
}

fn handle(lua: &Lua, p: &PlayerRef) -> mlua::Result<Value> {
    act::player_handle(lua, p).map(Value::Table)
}

fn text(lua: &Lua, bytes: &[u8]) -> mlua::Result<Value> {
    let bytes = bytes.split(|&b| b == 0).next().unwrap_or(bytes);
    Ok(Value::String(lua.create_string(bytes)?))
}

fn refuse<S>(ctx: &Context<S>, point: Point, id: u32, map: u16, build: &dyn Fn(&Lua) -> mlua::Result<MultiValue>) -> Option<Refusal> {
    let set = ctx.zones.ext_scripts();
    if set.for_point(point).is_empty() {
        return None;
    }
    let refusal = set.refuse(point, build);
    if let Some(r) = &refusal {
        tracing::debug!(id, map, point = point.name(), script = &*r.name, "refused by a script");
    }
    refusal
}

/// `trade`: the requester asks `target` (`tp::OP_REQUEST`, asked after `Zones::trade_check` and before the request).
pub(super) fn trade_refusal<S>(ctx: &Context<S>, asker: &PlayCharacter, map: u16, target: u32) -> Option<Refusal> {
    if ctx.zones.ext_scripts().for_point(Point::Trade).is_empty() {
        return None;
    }
    let asked = PlayerRef { id: target, name: ctx.zones.name_of(target)? };
    let me = me(asker);
    refuse(ctx, Point::Trade, asker.id, map, &|lua| Ok(MultiValue::from_iter([handle(lua, &me)?, handle(lua, &asked)?])))
}

/// `personal_shop`: the character sits and may set up a shop here by the core and the rules.
pub(super) fn shop_refusal<S>(ctx: &Context<S>, character: &PlayCharacter, map: u16) -> Option<Refusal> {
    let me = me(character);
    refuse(ctx, Point::PersonalShop, character.id, map, &|lua| Ok(MultiValue::from_iter([handle(lua, &me)?])))
}

/// `trade_chat` and `chat`: the line, as the player typed it (`chat` has the channel and a whisper's target too).
pub(super) fn chat_refusal<S>(ctx: &Context<S>, point: Point, character: &PlayCharacter, map: u16, channel: &str, to: Option<&[u8]>, line: &[u8]) -> Option<Refusal> {
    let me = me(character);
    refuse(ctx, point, character.id, map, &|lua| {
        let mut args = vec![handle(lua, &me)?];
        if point == Point::Chat {
            args.push(Value::String(lua.create_string(channel)?));
        }
        args.push(text(lua, line)?);
        if point == Point::Chat {
            args.push(match to {
                Some(name) => text(lua, name)?,
                None => Value::Nil,
            });
        }
        Ok(MultiValue::from_iter(args))
    })
}

/// `exp`: the factor the scripts give the gain (after the rules), 1 when none is asked.
pub(super) fn exp_factor<S>(ctx: &Context<S>, character: &PlayCharacter, map: u16, gain: u32, source: crate::rules::ExpSource) -> f64 {
    let set = ctx.zones.ext_scripts();
    if set.for_point(Point::Exp).is_empty() {
        return 1.0;
    }
    let me = me(character);
    let source = match source {
        crate::rules::ExpSource::Mob => "mob",
        crate::rules::ExpSource::Party => "party",
        crate::rules::ExpSource::Quest => "quest",
        crate::rules::ExpSource::Pvp => "pvp",
    };
    let factor = set.factor(Point::Exp, f64::from(gain), &|lua, so_far| {
        Ok(MultiValue::from_iter([handle(lua, &me)?, Value::Number(so_far.floor()), Value::String(lua.create_string(source)?)]))
    });
    if factor != 1.0 {
        tracing::debug!(id = character.id, map, point = "exp", extension = "scripts", before = gain, factor, "changed by scripts");
    }
    factor
}

/// `pvp_kills`: the multiplier the scripts give the counted kill (`times` is what the rules gave), 1 when none is asked.
pub(super) fn kills_times<S>(ctx: &Context<S>, character: &PlayCharacter, map: u16, times: u32) -> u32 {
    let set = ctx.zones.ext_scripts();
    if set.for_point(Point::PvpKills).is_empty() {
        return 1;
    }
    let me = me(character);
    let factor = set.times(times, &|lua, so_far| Ok(MultiValue::from_iter([handle(lua, &me)?, Value::Integer(i64::from(so_far))])));
    if factor != 1 {
        tracing::debug!(id = character.id, map, point = "pvp_kills", before = times, after = times.saturating_mul(factor), "changed by scripts");
    }
    factor
}

/// `pvp_death`: whether the scripts let the death count.
pub(super) fn death_counts<S>(ctx: &Context<S>, character: &PlayCharacter, map: u16) -> bool {
    let set = ctx.zones.ext_scripts();
    if set.for_point(Point::PvpDeath).is_empty() {
        return true;
    }
    let me = me(character);
    let counts = set.death_counts(&|lua| Ok(MultiValue::from_iter([handle(lua, &me)?])));
    if !counts {
        tracing::debug!(id = character.id, map, point = "pvp_death", "not counted by scripts");
    }
    counts
}

/// `enchant`: the jump after the scripts (`jump` is what the rules gave); `item` is the enchanted item's type, type id and step.
pub(super) fn enchant_jump<S>(ctx: &Context<S>, character: &PlayCharacter, map: u16, item: (u8, u8, u8), jump: u8) -> u8 {
    let set = ctx.zones.ext_scripts();
    if set.for_point(Point::Enchant).is_empty() {
        return jump;
    }
    let me = me(character);
    let after = set.jump(jump, &|lua, so_far| {
        let table = lua.create_table()?;
        table.raw_set("type", item.0)?;
        table.raw_set("type_id", item.1)?;
        table.raw_set("step", item.2)?;
        Ok(MultiValue::from_iter([handle(lua, &me)?, Value::Table(table), Value::Integer(i64::from(so_far))]))
    });
    if after != jump {
        tracing::debug!(id = character.id, map, point = "enchant", before = jump, after, "changed by scripts");
    }
    after
}

/// `item_use`: the extension that took the item over answers (section 6.1). `Core` for an item no script registered.
pub(super) fn item_use<S>(ctx: &Context<S>, character: &PlayCharacter, item: &crate::backend::ItemUnit, bag: u8, slot: u8) -> (crate::script::points::ItemUse, Option<Refusal>) {
    use crate::script::points::ItemUse;
    let set = ctx.zones.ext_scripts();
    let Some(owner) = set.item_owner(item.item_type, item.type_id) else { return (ItemUse::Core, None) };
    let me = me(character);
    let answer = owner.item_use(item.item_type, item.type_id, &|lua| {
        let table = lua.create_table()?;
        table.raw_set("type", item.item_type)?;
        table.raw_set("type_id", item.type_id)?;
        table.raw_set("bag", bag)?;
        table.raw_set("slot", slot)?;
        table.raw_set("count", item.count)?;
        Ok(MultiValue::from_iter([handle(lua, &me)?, Value::Table(table)]))
    });
    let refusal = match &answer {
        ItemUse::Refused(text) => Some(Refusal { rule: owner.item_refusal_id(), name: std::sync::Arc::from(format!("{}/item_use", owner.name)), text: text.clone() }),
        _ => None,
    };
    (answer, refusal)
}

/// Whether a script owns the item (so the core's refusal tests wait for it).
pub(super) fn owns_item<S>(ctx: &Context<S>, item_type: u8, type_id: u8) -> bool {
    ctx.zones.ext_scripts().item_owner(item_type, type_id).is_some()
}
