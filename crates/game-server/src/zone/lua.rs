//! Lua monsters in the zone (`docs/original/instances.md` sections 9 to 15): each one's script and stack machine, its turn in the
//! one-second pass (section 12), its fight (section 13), the hooks (section 14) and the functions its script calls (section 15).
//! Events from outside the pass (a player's hit, a death) wait in `MobMap::lua_events` for it. Part of the zone: the callers hold
//! `maps` and `mobs`.

use super::*;
use crate::mob_lua::{self as ml, Callback, LuaFields, LuaMob};
use crate::sync::Relock;
use mlua::{IntoLuaMulti, Lua, MultiValue, Value as LuaValue};
use shaiya_proto::chat;

/// `SetReTarget` gives up after this long without a target.
const GIVE_UP_AFTER: Duration = Duration::from_millis(3000);
/// Away from home: on a chase path and further than 0.5, or further than 10 (`WhileIdle`); home again when closer (`WhileReturnHome`).
const ON_PATH: f32 = 0.5;
const AWAY: f32 = 10.0;
/// `ChaseExp`'s full steps wait this much less than the chase time.
const CHASE_EX_QUICKER: Duration = Duration::from_millis(400);
/// The cells around a spot that `LuaDeleteMob`, `LuaGetMobCountByType` and `LuaAttackOrderSurroundMob` look at, and how close a
/// monster must be to be deleted.
const AROUND_CELLS: i32 = 5;
const DELETE_RANGE: f32 = 100.0;
/// A hook running: `LuaFixedMove` is ignored inside one (`+0x2b4`).
const IN_ATTACKED: u8 = 1;
const IN_ATTACKABLE: u8 = 2;
const IN_NORMAL_RESET: u8 = 3;
const IN_DEATH: u8 = 4;
const IN_RETURN_HOME: u8 = 5;
/// Events handled in one pass at most, against a script that keeps raising them.
const MAX_EVENTS: usize = 1000;

/// A Lua monster's status (`CMob::GetStatus`): the top of its stack; `None` for other monsters.
pub(super) fn lua_status(mobs: &MobMap, id: u32) -> Option<u8> {
    mobs.mobs.get(&id).filter(|m| m.lua)?;
    Some(mobs.lua.get(&id).map_or(ml::IDLE, |l| l.stack.top()))
}

/// Whether a player's blow or skill misses the monster (`CUser::AttackSuccess`): going home (status 3, a Lua monster's too) or
/// unbeatable (`LuaSetUnBeatable`).
pub(super) fn shielded(mobs: &MobMap, mob: &Mob) -> bool {
    mob.state == MobState::Home || mob.unbeatable || lua_status(mobs, mob.id) == Some(ml::HOME)
}

/// `CMob::SetTargetEx` (section 13): in battle with the player, no give-up time, and unless dead each skill entry's first ready time
/// (entry 0 and skills without reuse time at once).
fn set_target_ex(mobs: &mut MobMap, id: u32, player: u32, def: &MobDef, now: Instant, rand: &mut impl FnMut() -> u32) {
    let dead = lua_status(mobs, id) == Some(ml::DEAD);
    if let Some(l) = mobs.lua.get_mut(&id) {
        l.fields.battle = true;
        l.fields.give_up_at = None;
    }
    let Some(mob) = mobs.mobs.get_mut(&id) else { return };
    mob.state = MobState::Lua(Some(player));
    if dead {
        return;
    }
    for (i, entry) in def.attacks.iter().enumerate() {
        let Some(info) = entry.info.filter(|_| entry.ok != 0) else { continue };
        let reset = u32::from(info.reset_time) * 1000;
        mob.entry_ready[i] = (i != 0 && reset != 0).then(|| now + Duration::from_millis(u64::from(rand() % reset)));
    }
}

/// `CMob::SetTarget` for a Lua monster (section 11): only idle, or moving other than a type-1 move, and the player alive:
/// `SetTargetEx`, then event 1 for the pass.
pub(super) fn lua_set_target(
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    id: u32,
    player: u32,
    def: &MobDef,
    now: Instant,
    rand: &mut impl FnMut() -> u32,
) {
    let status = lua_status(mobs, id).unwrap_or(ml::IDLE);
    let move_type = mobs.lua.get(&id).map_or(0, |l| l.fields.move_type);
    let free = status == ml::IDLE || (status == ml::MOVING && move_type != 1);
    if !free || players.get(&player).is_none_or(|p| p.dead) || mobs.mobs.get(&id).is_none_or(|m| m.dead_until.is_some()) {
        return;
    }
    set_target_ex(mobs, id, player, def, now, rand);
    mobs.lua_events.push((id, ml::EV_ATTACKED, player));
}

/// The players of the damage record a Lua monster may take (`GetMaxAggroEx`, `LuaGetMinAggroEx`): in the zone, of another country,
/// alive, not hidden, within 45 of it, with their aggro, in the record's order.
fn aggro_candidates(mobs: &MobMap, players: &HashMap<u32, Placed>, id: u32, def: &MobDef) -> Vec<(u32, u32)> {
    let Some(mob) = mobs.mobs.get(&id) else { return Vec::new() };
    let Some(record) = mobs.damage.get(&id) else { return Vec::new() };
    record
        .iter()
        .filter_map(|h| Some((h.player()?, h.aggro)))
        .filter(|(player, _)| {
            players
                .get(player)
                .is_some_and(|p| targetable(p, def) && p.form.shape == 0 && distance(p.occupant.position, mob.position) <= fight::LEASH)
        })
        .collect()
}

/// The candidate with the most aggro, the first of equals (`CMob::GetMaxAggroEx`).
fn max_aggro(candidates: &[(u32, u32)]) -> Option<u32> {
    let mut best: Option<(u32, u32)> = None;
    for &(player, aggro) in candidates {
        if best.is_none_or(|(_, most)| aggro > most) {
            best = Some((player, aggro));
        }
    }
    best.map(|(player, _)| player)
}

/// Damage that did not kill (`CMob::ReTargetByAggroEx`, section 11): one more attack counted; once the term is up, the player with
/// the most aggro becomes the target.
pub(super) fn lua_damaged(mobs: &mut MobMap, players: &HashMap<u32, Placed>, id: u32, def: &MobDef, now: Instant) {
    let candidates = aggro_candidates(mobs, players, id, def);
    let Some(l) = mobs.lua.get_mut(&id) else { return };
    l.fields.attacked = l.fields.attacked.wrapping_add(1);
    if now < l.fields.next_retarget {
        return;
    }
    l.fields.next_retarget = now + l.fields.retarget_term;
    if let Some(top) = max_aggro(&candidates) {
        l.fields.battle = true;
        if let Some(m) = mobs.mobs.get_mut(&id) {
            m.state = MobState::Lua(Some(top));
        }
    }
}

/// A Lua monster killed (`CMob::Damage` at 0 HP): event 3 for the pass.
pub(super) fn lua_killed(mobs: &mut MobMap, id: u32) {
    if mobs.mobs.get(&id).is_some_and(|m| m.lua) {
        mobs.lua_events.push((id, ml::EV_DEATH, 0));
    }
}

/// A Lua number as `lua_tointeger` reads it (0 for anything else); fractions rounded to nearest, as the x87 does.
fn int(args: &[LuaValue], i: usize) -> i64 {
    match args.get(i) {
        Some(LuaValue::Integer(n)) => *n,
        Some(LuaValue::Number(n)) if n.is_finite() => n.round_ties_even() as i64,
        Some(LuaValue::String(s)) => s.to_str().ok().and_then(|s| s.trim().parse::<f64>().ok()).map_or(0, |n| n.round_ties_even() as i64),
        _ => 0,
    }
}

/// A Lua number as `lua_tonumber` reads it.
fn num(args: &[LuaValue], i: usize) -> f64 {
    match args.get(i) {
        Some(LuaValue::Integer(n)) => *n as f64,
        Some(LuaValue::Number(n)) => *n,
        Some(LuaValue::String(s)) => s.to_str().ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0),
        _ => 0.0,
    }
}

/// A Lua string's bytes as `lua_tolstring` gives them (numbers as text); empty for anything else.
fn bytes(args: &[LuaValue], i: usize) -> Vec<u8> {
    match args.get(i) {
        Some(LuaValue::String(s)) => s.as_bytes().to_vec(),
        Some(LuaValue::Integer(n)) => n.to_string().into_bytes(),
        Some(LuaValue::Number(n)) => n.to_string().into_bytes(),
        _ => Vec::new(),
    }
}

/// One zone's Lua monsters in a pass.
struct LuaRun<'a, 'b, R> {
    pass: &'a mut Pass<'b, R>,
    mobs: &'a mut MobMap,
    players: &'a mut HashMap<u32, Placed>,
    zone: u16,
    world: &'a World,
    terrain: &'a MapTerrain<'a>,
    /// `GetTickCount` for the hooks.
    clock: u32,
    /// A hook is running: events raised now wait for it to end, so hooks never run inside each other.
    in_hook: bool,
}

impl<R: FnMut() -> u32> LuaRun<'_, '_, R> {
    fn now(&self) -> Instant {
        self.pass.now
    }

    fn def(&self, id: u32) -> Option<&MobDef> {
        self.pass.defs.get(self.mobs.mobs.get(&id)?.mob_id)
    }

    fn fields(&mut self, id: u32) -> Option<&mut LuaFields> {
        self.mobs.lua.get_mut(&id).map(|l| &mut l.fields)
    }

    fn status(&self, id: u32) -> u8 {
        lua_status(self.mobs, id).unwrap_or(ml::DEAD)
    }

    fn target(&self, id: u32) -> Option<u32> {
        match self.mobs.mobs.get(&id)?.state {
            MobState::Lua(target) => target,
            _ => None,
        }
    }

    /// Gives every Lua monster of the zone without one its script and stack, and calls its `Init` (section 9). A monster with no
    /// script, or one that does not load, keeps the stack machine without hooks, as in the original.
    fn load_new(&mut self) {
        let mut new: Vec<u32> = self.mobs.mobs.values().filter(|m| m.lua && !self.mobs.lua.contains_key(&m.id)).map(|m| m.id).collect();
        new.sort_unstable();
        for id in new {
            let Some(ai) = self.def(id).map(|d| d.ai) else { continue };
            let lua = match self.world.mob_lua().0.get(&ai) {
                None => {
                    tracing::debug!(mob = id, ai, "Lua monster without a script");
                    None
                }
                Some(source) => match ml::load(source, &format!("{ai}.lua"), id) {
                    Ok(lua) => Some(lua),
                    Err(error) => {
                        tracing::warn!(mob = id, ai, %error, "Lua script did not load; the monster has no hooks");
                        None
                    }
                },
            };
            self.mobs.lua.insert(id, LuaMob { lua, stack: Default::default(), fields: LuaFields::new(self.now()) });
            self.hook(id, "Init", (), 0);
        }
    }

    /// Runs a hook of the monster's script (section 14) with its functions available; not a function: skipped; an error: logged,
    /// only-Lua-attack cleared (`LuaPrintError`).
    fn hook(&mut self, id: u32, name: &str, args: impl IntoLuaMulti, context: u8) {
        let Some(lua) = self.mobs.lua.get(&id).and_then(|l| l.lua.clone()) else { return };
        if let Some(f) = self.fields(id) {
            f.context = context;
        }
        self.in_hook = true;
        ml::reset_budget(&lua);
        let result = lua.scope(|scope| {
            let api = scope.create_function_mut(|lua, (name, args): (String, MultiValue)| self.call(id, &name, args, lua))?;
            lua.globals().set("__lua_mob", api)?;
            match lua.globals().get::<LuaValue>(name)? {
                LuaValue::Function(f) => f.call::<()>(args),
                _ => {
                    tracing::debug!(mob = id, hook = name, "not a function");
                    Ok(())
                }
            }
        });
        self.in_hook = false;
        let _ = lua.globals().set("__lua_mob", LuaValue::Nil);
        if let Err(error) = &result {
            tracing::warn!(mob = id, hook = name, %error, "Lua error");
        }
        if let Some(f) = self.fields(id) {
            f.context = 0;
            if result.is_err() {
                f.only_lua_attack = false;
            }
        }
    }

    /// An event for the monster: handled at once, or after the running hook.
    fn raise(&mut self, id: u32, event: u8, arg: u32) {
        if self.in_hook {
            self.mobs.lua_events.push((id, event, arg));
        } else {
            self.dispatch(id, event, arg);
        }
    }

    /// `DispatchEvent` (section 10): the top state's transition for the event, its callback first, then its change. An event the
    /// state has none for changes nothing (`StackFSMError`).
    fn dispatch(&mut self, id: u32, event: u8, arg: u32) {
        if self.mobs.mobs.get(&id).is_none_or(|m| !m.lua) {
            return;
        }
        let Some(top) = self.mobs.lua.get(&id).map(|l| l.stack.top()) else { return };
        let Some((callback, change)) = ml::transition(top, event) else {
            tracing::debug!(mob = id, state = top, event, "FSM error");
            return;
        };
        let clock = self.clock;
        let attacked = self.mobs.lua.get(&id).map_or(0, |l| l.fields.attacked);
        match callback {
            Callback::None => {}
            Callback::Attackable => self.hook(id, "OnAttackable", (clock, arg), IN_ATTACKABLE),
            Callback::Attacked => self.hook(id, "OnAttacked", (clock, arg), IN_ATTACKED),
            Callback::NormalReset => {
                if let Some(f) = self.fields(id) {
                    f.only_lua_attack = false;
                    f.give_up_at = None;
                }
                if let Some(m) = self.mobs.mobs.get_mut(&id) {
                    m.entry_used = 0;
                }
                self.hook(id, "OnNormalReset", clock, IN_NORMAL_RESET);
            }
            Callback::Death => self.hook(id, "OnDeath", (clock, attacked), IN_DEATH),
            Callback::ReturnHome => self.hook(id, "OnReturnHome", (clock, attacked), IN_RETURN_HOME),
            Callback::MoveEnd => {
                if let Some(f) = self.fields(id) {
                    f.move_type = 0;
                }
            }
        }
        if let Some(l) = self.mobs.lua.get_mut(&id) {
            l.stack.apply(change);
        }
    }

    /// The new monsters' scripts and the waiting events, in order, until none are left.
    fn drain(&mut self) {
        for _ in 0..MAX_EVENTS {
            self.load_new();
            if self.mobs.lua_events.is_empty() {
                return;
            }
            let (id, event, arg) = self.mobs.lua_events.remove(0);
            self.dispatch(id, event, arg);
        }
        tracing::warn!(zone = self.zone, left = self.mobs.lua_events.len(), "Lua events left for the next pass");
    }

    /// The top state's update (`StackStateMachine::Poll`, section 12).
    fn poll(&mut self, id: u32) {
        if self.mobs.mobs.get(&id).is_none_or(|m| m.dead_until.is_some()) {
            return;
        }
        match self.status(id) {
            ml::IDLE => self.while_idle(id),
            ml::COMBAT => self.while_combat(id),
            ml::HOME => self.while_return_home(id),
            ml::MOVING => self.while_move(id),
            _ => {}
        }
    }

    /// `CMob::FindTargetEx` (section 13): the first player in the 5 × 5 cells around (rows of z, then x) who is alive, not hidden or
    /// in a shop, within the monster's chase range and within 45 of its home; it gets `SetTargetEx`. No country check.
    fn find_target_ex(&mut self, id: u32) -> Option<u32> {
        let mob = *self.mobs.mobs.get(&id)?;
        let def = *self.def(id)?;
        let mut found: Option<((i32, i32), u32)> = None;
        for p in self.players.values() {
            if !in_view(p.cell, mob.cell)
                || p.dead
                || p.form.shape != 0
                || p.shop.is_some()
                || distance(p.occupant.position, mob.position) > f32::from(def.chase_range)
                || distance(p.occupant.position, mob.home) > fight::LEASH
            {
                continue;
            }
            let order = (p.cell.1, p.cell.0);
            if found.is_none_or(|(best, other)| order < best || (order == best && p.occupant.id < other)) {
                found = Some((order, p.occupant.id));
            }
        }
        let (_, player) = found?;
        set_target_ex(self.mobs, id, player, &def, self.now(), &mut *self.pass.rand);
        Some(player)
    }

    /// `CMob::WhileIdle`: a player found: event 0; else, its walk time come, away from home: event 4; else unless it holds its
    /// position, the ordinary idle step.
    fn while_idle(&mut self, id: u32) {
        if let Some(player) = self.find_target_ex(id) {
            return self.raise(id, ml::EV_ATTACKABLE, player);
        }
        let (Some(mob), Some(def)) = (self.mobs.mobs.get(&id).copied(), self.def(id).copied()) else { return };
        let now = self.now();
        if def.obelisk || mob.next_move > now {
            return;
        }
        let from_home = distance(mob.position, mob.home);
        let trail = self.mobs.trails.get(&id).is_some_and(|t| !t.is_empty());
        if (trail && from_home > ON_PATH) || from_home > AWAY {
            return self.raise(id, ml::EV_RETURN_HOME, 0);
        }
        if self.fields(id).is_some_and(|f| f.hold) {
            return;
        }
        let Some((step, wait)) = mobs::idle_step(&mob, &def, self.mobs.speeds(id).1, &mut *self.pass.rand) else { return };
        self.mobs.mobs.get_mut(&id).expect("listed").next_move = now + wait;
        let Some(to) = step else { return };
        let line = |p: [f32; 3]| (p[0] as i32, p[2] as i32);
        if distance(to, mob.home) < mobs::HOME_RANGE
            && self.world.is_line_free(self.zone, line(mob.position), line(to))
            && self.world.is_safe_pos(self.zone, to)
        {
            walk_mob(self.mobs, Some(self.players), id, to, 0);
        }
    }

    /// The HP percent the hooks get: `round(HP / max × 100)`.
    fn hp_percent(&self, id: u32) -> u32 {
        let Some(mob) = self.mobs.mobs.get(&id) else { return 0 };
        let max = self.def(id).map_or(0, |d| d.hp);
        if max == 0 { 0 } else { (f64::from(mob.hp) / f64::from(max) * 100.0).round_ties_even() as u32 }
    }

    /// `CMob::WhileCombat`: nothing asleep or stunned; with only-Lua-attack, or before its attack time, the hook; at its attack time
    /// the fight (`AttackAIExp`), and a new target when that fails.
    fn while_combat(&mut self, id: u32) {
        let effects = self.mobs.effects(id);
        if effects.sleep > 0 || effects.stun > 0 {
            return;
        }
        let Some(mob) = self.mobs.mobs.get(&id).copied() else { return };
        let only_lua = self.fields(id).is_some_and(|f| f.only_lua_attack);
        if only_lua || self.now() < mob.next_attack {
            let (clock, hp, attacked) = (self.clock, self.hp_percent(id), self.fields(id).map_or(0, |f| f.attacked));
            return self.hook(id, "WhileCombat", (clock, hp, attacked), 0);
        }
        let alive = self.target(id).and_then(|t| self.players.get(&t)).is_some_and(|p| !p.dead);
        if !alive || !self.attack_ai_ex(id) {
            self.set_retarget(id);
        }
    }

    /// `CMob::SetReTarget`: the first time without a target, a look around and a give-up time 3 s on; once past it, the monster
    /// gives up (`ResetAttack`, the record cleared, event 2); before it, another look.
    fn set_retarget(&mut self, id: u32) {
        let now = self.now();
        match self.fields(id).and_then(|f| f.give_up_at) {
            None => {
                if self.find_target_ex(id).is_none()
                    && let Some(f) = self.fields(id)
                {
                    f.give_up_at = Some(now + GIVE_UP_AFTER);
                }
            }
            Some(at) if now > at => {
                self.reset_attack(id);
                if let Some(f) = self.fields(id) {
                    f.battle = false;
                }
                self.raise(id, ml::EV_NORMAL_RESET, 0);
            }
            Some(_) => {
                self.find_target_ex(id);
            }
        }
    }

    /// `CMob::ResetAttack` and the paths and record cleared: HP, SP and MP full, attack times cleared, the debuffs gone, `0x0305`
    /// and `0x0609` within 60.
    fn reset_attack(&mut self, id: u32) {
        let Some(def) = self.def(id).copied() else { return };
        let Some(m) = self.mobs.mobs.get_mut(&id) else { return };
        (m.hp, m.sp, m.mp) = (def.hp, def.sp, def.mp);
        m.entry_ready = [None; 3];
        m.fair = true;
        m.state = MobState::Lua(None);
        let at = m.position;
        self.mobs.damage.remove(&id);
        self.mobs.trails.remove(&id);
        self.mobs.paths.remove(&id);
        self.mobs.debuffs.remove(&id);
        send_around(Some(self.players), at, RECOVER_RANGE, &w::mob_hp(id, def.hp));
        send_around(Some(self.players), at, RECOVER_RANGE, &cp::mob_speed(id, mobs::ATTACK_SPEED, mobs::MOVE_SPEED));
    }

    /// `ChaseExp` giving up (section 13): out of battle, the path cleared, a walk home as a move of type 3 (event 5), or event 7 when a
    /// move is already going on.
    fn give_up_chase(&mut self, id: u32) {
        let Some(home) = self.mobs.mobs.get(&id).map(|m| m.home) else { return };
        self.mobs.trails.remove(&id);
        self.mobs.paths.remove(&id);
        let Some(f) = self.fields(id) else { return };
        f.battle = false;
        f.move_to = [home[0], 0.0, home[2]];
        let event = if f.move_type == 0 {
            f.move_type = ml::MOVE_HOME;
            ml::EV_MOVE
        } else {
            ml::EV_MOVE_AGAIN
        };
        self.raise(id, event, 0);
    }

    /// A chase step towards the target or the attack in reach (`ChaseExp`): `Some(true)` in reach, `Some(false)` stepping or waiting,
    /// `None` given up. Full steps wait 400 ms less than the chase time (`docs/original/pathing.md` section 5).
    fn chase_ex(&mut self, id: u32, at: [f32; 3], range: u8) -> Option<bool> {
        let mob = *self.mobs.mobs.get(&id)?;
        let def = *self.def(id)?;
        let (speed, now) = (self.mobs.speeds(id).1, self.now());
        let full_wait = def.chase_time.saturating_sub(CHASE_EX_QUICKER);
        let path = self.mobs.paths.entry(id).or_default();
        let chase = fight::chase(mob.position, mob.home, at, &def, range, speed, mob.next_chase, now, self.terrain, path, full_wait);
        if !matches!(chase, Chase::Wait | Chase::Step { .. }) {
            stop_moving(self.mobs, id);
        }
        match chase {
            Chase::InRange => Some(true),
            Chase::Wait => Some(false),
            Chase::GoHome | Chase::Drop | Chase::Unfair => {
                self.give_up_chase(id);
                None
            }
            Chase::Step { to, next, trail } => {
                fight::add_trail(self.mobs.trails.entry(id).or_default(), trail, self.terrain);
                let to = to.filter(|to| self.world.is_safe_pos(self.zone, *to));
                let moving = match self.target(id) {
                    Some(target) => Moving::Chase { target, range, wait: full_wait },
                    None => Moving::None,
                };
                let next = paced_next(self.mobs, id, &def, to, 1, next, now, self.pass.zones.smooth_chase(), moving);
                self.mobs.mobs.get_mut(&id).expect("listed").next_chase = next;
                if let Some(to) = to {
                    walk_mob(self.mobs, Some(self.players), id, to, 1);
                }
                Some(false)
            }
        }
    }

    /// A normal blow with an entry (`CMob::Attack`), its ready time after the attack speed and its time (not entry 0).
    fn strike(&mut self, id: u32, target: u32, entry: usize) {
        let Some(def) = self.def(id).copied() else { return };
        let attack = def.attacks[entry];
        let speed = self.mobs.speeds(id).0;
        mob_strike(self.pass, self.mobs, self.players, id, target, &def, &attack);
        let now = self.now();
        if entry != 0
            && let Some(m) = self.mobs.mobs.get_mut(&id)
        {
            m.entry_ready[entry] = Some(fight::entry_wait(now, speed, &attack));
        }
    }

    /// A skill on the target (`CMob::UseSkill`), the entry's ready time after the skill's reuse time when it is an entry's.
    fn skill(&mut self, id: u32, target: u32, entry: Option<usize>, s: &SkillDef) {
        let Some(def) = self.def(id).copied() else { return };
        mob_skills::use_skill(self.pass, self.mobs, self.players, id, target, &def, entry.unwrap_or(0), s);
        if let Some(i) = entry
            && let Some(m) = self.mobs.mobs.get_mut(&id)
        {
            m.entry_ready[i] = Some(self.pass.now + Duration::from_secs(u64::from(s.reset_time)));
        }
    }

    /// Whether the monster fights where it stands: `AI` 4, no chase step, an obelisk, or holding its position.
    fn stays(&mut self, id: u32, def: &MobDef) -> bool {
        !fight::walks(def) || self.fields(id).is_some_and(|f| f.hold)
    }

    /// `CMob::AttackAIExp` (section 13): `false` when the target cannot be fought.
    fn attack_ai_ex(&mut self, id: u32) -> bool {
        let (Some(mob), Some(def)) = (self.mobs.mobs.get(&id).copied(), self.def(id).copied()) else { return false };
        let Some(target) = self.target(id) else { return false };
        let Some(at) = self.players.get(&target).filter(|p| targetable(p, &def) && p.form.shape == 0).map(|p| p.occupant.position) else {
            return false;
        };
        let now = self.now();
        for i in [2, 1, 0] {
            let entry = def.attacks[i];
            if mob.entry_ready[i].is_some_and(|t| now < t) || !fight::entry_usable(&entry, mob.hp, def.hp) {
                continue;
            }
            if let Some(m) = self.mobs.mobs.get_mut(&id) {
                m.entry_used = i as u8;
            }
            if let Some(s) = entry.info
                && s.type_effect == skills::HEAL
            {
                self.skill(id, target, Some(i), &s);
                return true;
            }
            let range = entry.info.map_or(entry.range, |s| s.attack_range);
            if self.stays(id, &def) {
                if !fight::stationary_reach(mob.position, at, &def, range) {
                    if let Some(s) = entry.info
                        && let Some(m) = self.mobs.mobs.get_mut(&id)
                    {
                        m.entry_ready[i] = Some(now + Duration::from_secs(u64::from(s.reset_time)));
                    }
                    return false;
                }
            } else {
                match self.chase_ex(id, at, range) {
                    Some(true) => {}
                    Some(false) => return true,
                    None => {
                        if let Some(s) = entry.info
                            && let Some(m) = self.mobs.mobs.get_mut(&id)
                        {
                            m.entry_ready[i] = Some(now + Duration::from_secs(u64::from(s.reset_time)));
                        }
                        return true;
                    }
                }
            }
            match entry.info {
                Some(s) => self.skill(id, target, Some(i), &s),
                None => self.strike(id, target, i),
            }
            return true;
        }
        let speed = self.mobs.speeds(id).0;
        if let Some(m) = self.mobs.mobs.get_mut(&id) {
            m.next_attack = fight::idle_wait(now, speed);
        }
        true
    }

    /// The end of a move (`WhileMove`): event 6, home where it stands, the path cleared; then a walk home resets the monster, any
    /// other move calls `OnMoveEnd`.
    fn move_ended(&mut self, id: u32, move_type: u8) {
        self.raise(id, ml::EV_MOVE_END, 0);
        if let Some(m) = self.mobs.mobs.get_mut(&id) {
            m.home = m.position;
        }
        self.mobs.trails.remove(&id);
        self.mobs.paths.remove(&id);
        if move_type == ml::MOVE_HOME {
            self.reset_attack(id);
        } else {
            let clock = self.clock;
            self.hook(id, "OnMoveEnd", clock, 0);
        }
    }

    /// `CMob::WhileMove` (section 12, `docs/original/pathing.md` section 5): types 2 and 3 look for a player first; then walking
    /// steps towards the move's spot until within `5 + rand() % 5` (0.5 on a chase path), round a blocked line by the chase path or a
    /// searched way, or the end when there is no way.
    fn while_move(&mut self, id: u32) {
        let Some(f) = self.fields(id).copied() else { return };
        if matches!(f.move_type, 2 | 3)
            && let Some(player) = self.find_target_ex(id)
        {
            return self.raise(id, ml::EV_ATTACKABLE, player);
        }
        let (Some(mob), Some(def)) = (self.mobs.mobs.get(&id).copied(), self.def(id).copied()) else { return };
        let (speed, now) = (self.mobs.speeds(id).1, self.now());
        let (trail, path) = (self.mobs.trails.entry(id).or_default(), self.mobs.paths.entry(id).or_default());
        let rand = &mut *self.pass.rand;
        match fight::moving(mob.position, f.move_to, &def, trail, path, speed, f.next_step, now, self.terrain, rand) {
            Homeward::Wait => {}
            Homeward::Step { to, next } => self.walk(id, &def, to, next, 0, Moving::Walk { spot: f.move_to }),
            Homeward::Arrived | Homeward::Settle | Homeward::Idle => {
                stop_moving(self.mobs, id);
                self.move_ended(id, f.move_type);
            }
        }
    }

    /// `CMob::WhileReturnHome` (section 12): going home as `CMob::ReturnHome` does; home: reset and event 2; no way or home blocked:
    /// event 2 (the home where it stands when there is no way).
    fn while_return_home(&mut self, id: u32) {
        let Some(next_step) = self.fields(id).map(|f| f.next_step) else { return };
        let (Some(mob), Some(def)) = (self.mobs.mobs.get(&id).copied(), self.def(id).copied()) else { return };
        let (speed, now) = (self.mobs.speeds(id).1, self.now());
        let (trail, path) = (self.mobs.trails.entry(id).or_default(), self.mobs.paths.entry(id).or_default());
        let homeward = fight::homeward(mob.position, mob.home, &def, trail, path, speed, next_step, now, self.terrain);
        if !matches!(homeward, Homeward::Wait | Homeward::Step { .. }) {
            stop_moving(self.mobs, id);
        }
        match homeward {
            Homeward::Wait => {}
            Homeward::Step { to, next } => self.walk(id, &def, to, next, 1, Moving::Home),
            Homeward::Arrived => {
                self.reset_attack(id);
                self.raise(id, ml::EV_NORMAL_RESET, 0);
            }
            Homeward::Settle => {
                if let Some(m) = self.mobs.mobs.get_mut(&id) {
                    m.home = m.position;
                }
                self.raise(id, ml::EV_NORMAL_RESET, 0);
            }
            Homeward::Idle => self.raise(id, ml::EV_NORMAL_RESET, 0),
        }
    }

    /// A move's step: the next step time (paced to the client with smoothing, `docs/original/chase-smoothing.md` deviations 1
    /// and 4), then the step unless the spot is off the map.
    fn walk(&mut self, id: u32, def: &MobDef, to: [f32; 3], next: Instant, motion: u8, moving: Moving) {
        let to = Some(to).filter(|to| self.world.is_safe_pos(self.zone, *to));
        let next = paced_next(self.mobs, id, def, to, motion, next, self.now(), self.pass.zones.smooth_chase(), moving);
        if let Some(f) = self.fields(id) {
            f.next_step = next;
        }
        if let Some(to) = to {
            walk_mob(self.mobs, Some(self.players), id, to, motion);
        }
    }

    /// The players in the cells from `trunc((pos − radius) / 40)` to `trunc((pos + radius) / 40)` within `radius` of the monster.
    fn send_by_radius(&self, id: u32, radius: f32, packet: &[u8]) {
        let Some(mob) = self.mobs.mobs.get(&id) else { return };
        let cell = |v: f32| (v / CELL) as i32;
        let (x0, x1) = (cell(mob.position[0] - radius).max(0), cell(mob.position[0] + radius));
        let (z0, z1) = (cell(mob.position[2] - radius).max(0), cell(mob.position[2] + radius));
        for p in self.players.values() {
            if (x0..=x1).contains(&p.cell.0) && (z0..=z1).contains(&p.cell.1) && distance(p.occupant.position, mob.position) <= radius {
                p.send(packet);
            }
        }
    }

    /// A spot from a function's `(x, z)`: both 0 is the monster's own spot and cell, otherwise `(x, own y, z)` and its cell.
    fn spot(&self, id: u32, x: f64, z: f64) -> Option<([f32; 3], (i32, i32))> {
        let mob = self.mobs.mobs.get(&id)?;
        if x == 0.0 && z == 0.0 {
            return Some((mob.position, mob.cell));
        }
        let (x, z) = (x as f32, z as f32);
        Some(([x, mob.position[1], z], ((x / CELL) as i32, (z / CELL) as i32)))
    }

    /// A definition a function may create or name: with an id, HP and a level.
    fn usable_def(&self, mob_id: i64) -> Option<MobDef> {
        u16::try_from(mob_id).ok().and_then(|id| self.pass.defs.get(id)).filter(|d| d.id != 0 && d.hp != 0 && d.level != 0).copied()
    }

    /// `CZone::MobGenEx` (`LuaCreateMob`, section 15): new monsters that never respawn, each at the spot ± 10 or exactly there; the
    /// new id when exactly one was made.
    fn mob_gen_ex(&mut self, id: u32, mob_id: i64, count: i64, x: f64, z: f64) -> u32 {
        let Some(def) = self.usable_def(mob_id) else { return 0 };
        let Some((spot, _)) = self.spot(id, x, z) else { return 0 };
        let count = count.max(1);
        let (made, last) = super::admin::generate(
            self.pass.zones,
            self.world,
            self.zone,
            self.mobs,
            self.players,
            &def,
            spot,
            count,
            self.pass.now,
            &mut *self.pass.rand,
        );
        if made == 1 { last } else { 0 }
    }

    /// `CZone::MobRemove2` (`LuaDeleteMob`, section 15): every living monster of that id within 100 of the spot, in the cells ± 5
    /// around, dies quietly: removed at the next body pass with `0x0602`, nothing gained; a Lua one gets event 3.
    fn mob_remove2(&mut self, id: u32, mob_id: i64, x: f64, z: f64) {
        let Some(def) = self.usable_def(mob_id) else { return };
        let Some((spot, cell)) = self.spot(id, x, z) else { return };
        let mut gone = Vec::new();
        for cz in cell.1 - AROUND_CELLS..=cell.1 + AROUND_CELLS {
            for cx in cell.0 - AROUND_CELLS..=cell.0 + AROUND_CELLS {
                for &other in self.mobs.cells.get(&(cx, cz)).into_iter().flatten() {
                    let Some(m) = self.mobs.mobs.get(&other) else { continue };
                    if m.mob_id == def.id && m.dead_until.is_none() && distance(m.position, spot) <= DELETE_RANGE {
                        gone.push(other);
                    }
                }
            }
        }
        let now = self.now();
        for other in gone {
            // A boss group's boss is taken away (`MobRemove2` → `BothMobDestroy`, `docs/original/obelisks.md` section 11.3).
            if let Some(m) = self.mobs.mobs.get(&other).copied() {
                self.pass.zones.boss_death(self.zone, Some(self.players), self.mobs, &m, true);
            }
            if let Some(m) = self.mobs.mobs.get_mut(&other) {
                m.dead_until = Some(now);
            }
            if self.mobs.mobs.get(&other).is_some_and(|m| m.lua) {
                self.raise(other, ml::EV_DEATH, 0);
            }
        }
    }

    /// `LuaAttackAI(skill, char id)` (section 15): a normal blow (skill 0, only when entry 0 has no skill) or the skill, on the
    /// player or the current target, at once when in reach, after a chase otherwise; no reuse time, no HP rule.
    fn attack_ai(&mut self, id: u32, skill: i64, player: i64) {
        let Some(def) = self.def(id).copied() else { return };
        let target = if player == 0 { self.target(id) } else { u32::try_from(player).ok() };
        let at =
            target.and_then(|t| self.players.get(&t)).filter(|p| targetable(p, &def) && p.form.shape == 0).map(|p| p.occupant.position);
        let (Some(target), Some(at)) = (target, at) else { return self.set_retarget(id) };
        let s = if skill == 0 {
            if def.attacks[0].info.is_some() {
                return;
            }
            if let Some(m) = self.mobs.mobs.get_mut(&id) {
                m.entry_used = 0;
            }
            None
        } else {
            let Some(s) = u16::try_from(skill).ok().and_then(|id| self.pass.zones.item_skills.get().and_then(|t| t.get(&id)).copied())
            else {
                return;
            };
            Some(s)
        };
        let Some(mob) = self.mobs.mobs.get(&id).copied() else { return };
        let range = s.map_or(def.attacks[0].range, |s| s.attack_range);
        if let Some(s) = s.filter(|s| s.type_effect == skills::HEAL) {
            return self.skill(id, target, None, &s);
        }
        let in_reach = if self.stays(id, &def) {
            if !fight::stationary_reach(mob.position, at, &def, range) {
                return self.set_retarget(id);
            }
            true
        } else {
            self.chase_ex(id, at, range) == Some(true)
        };
        if in_reach {
            match s {
                Some(s) => self.skill(id, target, None, &s),
                None => mob_strike(self.pass, self.mobs, self.players, id, target, &def, &def.attacks[0]),
            }
        }
    }

    /// `LuaFixedMove(type, x, z)` (section 15): not inside a transition hook; a free spot on the map; not dead or going home. The move
    /// type and spot are set, then event 5 (no move going on) or 7.
    fn fixed_move(&mut self, id: u32, kind: i64, x: f64, z: f64) {
        let status = self.status(id);
        let (cx, cz) = (x.round_ties_even() as i32, z.round_ties_even() as i32);
        let free = self.world.is_safe_pos(self.zone, [cx as f32, 0.0, cz as f32]) && !self.world.is_blocked(self.zone, cx, cz);
        let Some(f) = self.fields(id) else { return };
        if f.context != 0 || !free || status == ml::DEAD || status == ml::HOME {
            return;
        }
        let before = f.move_type;
        f.move_to = [x as f32, 0.0, z as f32];
        f.move_type = kind as u8;
        self.raise(id, if before == 0 { ml::EV_MOVE } else { ml::EV_MOVE_AGAIN }, 0);
    }

    /// `LuaRecover(kind, amount)`: kind 0 adds the amount, other kinds a percentage of the maximum; capped; `0x0610` within 60.
    fn recover(&mut self, id: u32, kind: i64, amount: f64) {
        let Some(max) = self.def(id).map(|d| d.hp) else { return };
        let Some(m) = self.mobs.mobs.get_mut(&id) else { return };
        if amount <= 0.0 || m.hp == max {
            return;
        }
        let add = if kind == 0 {
            amount.min(f64::from(max)) as u32
        } else {
            (f64::from(max) * amount.min(100.0) / 100.0).round_ties_even() as u32
        };
        let after = m.hp.saturating_add(add).min(max);
        let gained = after - m.hp;
        m.hp = after;
        let at = m.position;
        if gained > 0 {
            self.pass.zones.mob_record(self.pass.map, m, gained, crate::game_log::MobAction::Heal, None, None);
            send_around(Some(self.players), at, RECOVER_RANGE, &cp::mob_recover(id, gained as i32));
        }
    }

    /// The monsters in the cells ± 5 around a spot's cell.
    fn around(&self, cell: (i32, i32)) -> Vec<u32> {
        let mut ids = Vec::new();
        for cz in cell.1 - AROUND_CELLS..=cell.1 + AROUND_CELLS {
            for cx in cell.0 - AROUND_CELLS..=cell.0 + AROUND_CELLS {
                ids.extend(self.mobs.cells.get(&(cx, cz)).into_iter().flatten().copied());
            }
        }
        ids
    }

    /// `LuaAttackOrderSurroundMob(char id, x, z)`: the other idle or fighting monsters around that have the player within their chase
    /// range take it as target.
    fn attack_order(&mut self, id: u32, player: i64, x: f64, z: f64) {
        if self.players.is_empty() {
            return;
        }
        let player = if player == 0 { self.target(id) } else { u32::try_from(player).ok() };
        let Some((player, at)) = player.and_then(|p| self.players.get(&p).filter(|p| !p.dead).map(|q| (p, q.occupant.position))) else {
            return;
        };
        let Some((_, cell)) = self.spot(id, x, z) else { return };
        let now = self.now();
        for other in self.around(cell) {
            let Some(m) = self.mobs.mobs.get(&other).copied() else { continue };
            let Some(def) = self.def(other).copied() else { continue };
            let free = match lua_status(self.mobs, other) {
                Some(status) => status <= ml::COMBAT,
                None => matches!(m.state, MobState::Idle | MobState::Fighting(_)) && m.dead_until.is_none(),
            };
            if other == id || !free || distance(m.position, at) > f32::from(def.chase_range) {
                continue;
            }
            if m.lua {
                set_target_ex(self.mobs, other, player, &def, now, &mut *self.pass.rand);
            } else if let Some(m) = self.mobs.mobs.get_mut(&other) {
                take_target(m, &def, player, now, &mut *self.pass.rand);
            }
        }
    }

    /// `LuaRecallUser(job, radius, map, x, y, z)`: the players in the view by radius of that job (inferred), alive and not hidden,
    /// are moved there (± 2 in x and z when free), the monster's spot when `(x, y, z)` are 0.
    #[allow(clippy::too_many_arguments)]
    fn recall(&mut self, id: u32, job: i64, radius: f64, map: i64, x: f64, y: f64, z: f64) {
        let Ok(map) = u16::try_from(map) else { return };
        let Some(mob) = self.mobs.mobs.get(&id).copied() else { return };
        let (cx, cz) = (x.round_ties_even() as i32, z.round_ties_even() as i32);
        if !self.world.is_safe_pos(map, [cx as f32, 0.0, cz as f32]) || self.world.is_blocked(map, cx, cz) {
            return;
        }
        let to = if x == 0.0 && y == 0.0 && z == 0.0 { mob.position } else { [x as f32, y as f32, z as f32] };
        let radius = radius as f32;
        let cell = |v: f32| (v / CELL) as i32;
        let (x0, x1) = (cell(mob.position[0] - radius).max(0), cell(mob.position[0] + radius));
        let (z0, z1) = (cell(mob.position[2] - radius).max(0), cell(mob.position[2] + radius));
        let mut chosen = Vec::new();
        for p in self.players.values() {
            if (x0..=x1).contains(&p.cell.0)
                && (z0..=z1).contains(&p.cell.1)
                && !p.dead
                && p.form.shape == 0
                && i64::from(p.occupant.shape.job) == job
                && distance(p.occupant.position, mob.position) <= radius
            {
                chosen.push(p.occupant.id);
            }
        }
        // A map change goes to the map's static zone, a move on this map stays in this zone.
        let zone = if crate::world::map_of(self.zone) == map { self.zone } else { map };
        for player in chosen {
            let dx = ((self.pass.rand)() % 4) as f32 - 2.0;
            let dz = ((self.pass.rand)() % 4) as f32 - 2.0;
            let spread = [to[0] + dx, to[1], to[2] + dz];
            let free = self.world.is_safe_pos(zone, spread)
                && !self.world.is_blocked(zone, spread[0].round_ties_even() as i32, spread[2].round_ties_even() as i32);
            let position = if free { spread } else { to };
            if let Some(p) = self.players.get(&player) {
                let _ = p.occupant.events.send(ZoneEvent::Recalled { zone, position });
            }
        }
    }

    /// The functions of section 15, by name, with `lua_tointeger` / `lua_tonumber` arguments.
    fn call(&mut self, id: u32, name: &str, args: MultiValue, lua: &Lua) -> mlua::Result<MultiValue> {
        let a: Vec<LuaValue> = args.into_iter().collect();
        let Some(def) = self.def(id).copied() else { return MultiValue::new().into_lua_multi(lua) };
        let Some(mut fields) = self.fields(id).copied() else { return MultiValue::new().into_lua_multi(lua) };
        match name {
            "LuaSay" => {
                if let Some(packet) = chat::monster_say_lua(id, &bytes(&a, 0)) {
                    self.send_by_radius(id, num(&a, 1) as f32, &packet);
                }
            }
            "LuaSayByIndex" => self.send_by_radius(id, num(&a, 1) as f32, &chat::monster_say_index(id, int(&a, 0) as u16)),
            "LuaSayByVoice" => self.send_by_radius(id, num(&a, 1) as f32, &chat::monster_voice(id, &bytes(&a, 0))),
            "LuaGetName" => {
                let name = u32::try_from(int(&a, 0)).ok().and_then(|p| self.players.get(&p)).map(|p| p.occupant.shape.name.clone());
                let name = lua.create_string(name.unwrap_or_default())?;
                return name.into_lua_multi(lua);
            }
            "LuaCreateMob" => return self.mob_gen_ex(id, int(&a, 0), int(&a, 1), num(&a, 2), num(&a, 3)).into_lua_multi(lua),
            "LuaDeleteMob" => self.mob_remove2(id, int(&a, 0), num(&a, 2), num(&a, 3)),
            "LuaSetTarget" => {
                if let Some(m) = self.mobs.mobs.get_mut(&id) {
                    m.state = MobState::Lua(u32::try_from(int(&a, 0)).ok());
                }
            }
            "LuaSetReTargetingTerm" => {
                let term = Duration::from_millis((int(&a, 0).max(0) as u64).saturating_mul(1000));
                if term != fields.retarget_term && term >= Duration::from_millis(1000) {
                    if term > fields.retarget_term {
                        fields.next_retarget += term - fields.retarget_term;
                    } else {
                        fields.next_retarget =
                            fields.next_retarget.checked_sub(fields.retarget_term - term).unwrap_or(fields.next_retarget);
                    }
                    fields.retarget_term = term;
                }
            }
            "LuaSetOnlyLuaAttack" => fields.only_lua_attack = int(&a, 0) != 0,
            "LuaSetHoldPosion" => fields.hold = int(&a, 0) != 0,
            "LuaSetUnBeatable" => {
                if let Some(m) = self.mobs.mobs.get_mut(&id) {
                    m.unbeatable = int(&a, 0) != 0;
                }
            }
            "LuaResetAggro" => {
                for h in self.mobs.damage.get_mut(&id).into_iter().flatten() {
                    h.aggro = 0;
                }
            }
            "LuaGetMinAggro" => {
                let candidates = aggro_candidates(self.mobs, self.players, id, &def);
                let mut least: Option<(u32, u32)> = None;
                for &(player, aggro) in &candidates {
                    if least.is_none_or(|(_, l)| aggro < l) {
                        least = Some((player, aggro));
                    }
                }
                return least.map_or(0, |(p, _)| p).into_lua_multi(lua);
            }
            "LuaGetNumberAggro" => {
                let n = int(&a, 0);
                let candidates = aggro_candidates(self.mobs, self.players, id, &def);
                let found = match n {
                    ..=0 => None,
                    1 => max_aggro(&candidates),
                    _ => {
                        // The n-th highest (deviation 3; the original answers the second for every n of 2 and more).
                        let mut sorted = candidates.clone();
                        sorted.sort_by_key(|&(_, aggro)| std::cmp::Reverse(aggro));
                        sorted.get(n as usize - 1).map(|&(p, _)| p)
                    }
                };
                return found.unwrap_or(0).into_lua_multi(lua);
            }
            "LuaGetMobCountByType" => {
                let Some((_, cell)) = self.spot(id, num(&a, 1), num(&a, 2)) else { return 0.into_lua_multi(lua) };
                let wanted = int(&a, 0);
                let count = self
                    .around(cell)
                    .into_iter()
                    .filter(|&other| other != id)
                    .filter_map(|other| self.mobs.mobs.get(&other))
                    .filter(|m| m.dead_until.is_none() && (wanted == 0 || i64::from(m.mob_id) == wanted))
                    .count();
                return (count as i64).into_lua_multi(lua);
            }
            "LuaGetMobHP" => {
                let other = u32::try_from(int(&a, 0)).unwrap_or(0);
                let mob = if other == 0 { Some(id) } else { Some(other).filter(|o| self.mobs.mobs.contains_key(o)) };
                let percent = mob
                    .and_then(|m| Some((self.mobs.mobs.get(&m)?.hp, self.def(m)?.hp)))
                    .map_or(0.0, |(hp, max)| if max == 0 { 0.0 } else { f64::from(hp) / f64::from(max) * 100.0 });
                return percent.into_lua_multi(lua);
            }
            "LuaGetMobPos" => {
                let at = u32::try_from(int(&a, 0)).ok().and_then(|m| self.mobs.mobs.get(&m)).map_or([0.0; 3], |m| m.position);
                return (f64::from(at[0]), f64::from(at[2])).into_lua_multi(lua);
            }
            "LuaRecover" => self.recover(id, int(&a, 0), num(&a, 1)),
            "LuaAttackAI" => self.attack_ai(id, int(&a, 0), int(&a, 1)),
            "LuaFixedMove" => self.fixed_move(id, int(&a, 0), num(&a, 1), num(&a, 2)),
            "LuaAttackOrderSurroundMob" => self.attack_order(id, int(&a, 0), num(&a, 1), num(&a, 2)),
            "LuaRecallUser" => self.recall(id, int(&a, 0), num(&a, 1), int(&a, 2), num(&a, 3), num(&a, 4), num(&a, 5)),
            "LuaUpdateInsZonePortal" => {
                // Only on a map with a CreateType (`CZone::UpdateInsZonePortalCountry`); nothing is sent.
                if self.world.create_type(self.zone) != 0 {
                    self.pass.zones.set_instance_portal(self.zone, int(&a, 0).max(0) as u32, int(&a, 1).max(0) as u32);
                }
            }
            _ => return Err(mlua::Error::runtime(format!("no function {name}"))),
        }
        // The setters change the monster's fields.
        if matches!(name, "LuaSetReTargetingTerm" | "LuaSetOnlyLuaAttack" | "LuaSetHoldPosion")
            && let Some(f) = self.fields(id)
        {
            *f = fields;
        }
        MultiValue::new().into_lua_multi(lua)
    }
}

impl Zones {
    /// The Lua monsters' second (`CMob::UpdateIdle` for a Lua monster, section 10): per zone, the new monsters' scripts and the
    /// waiting events first, then each living Lua monster's top state, the events it raised after it. Not in the 500 ms attack pass.
    pub fn lua(&self, now: Instant, world: &World, defs: &MobDefs, loot: (&DropTables, &ItemDefs), rand: &mut impl FnMut() -> u32) {
        let mut maps = self.maps();
        let mut all = self.mobs.guard();
        let mut nobody = HashMap::new();
        let clock = crate::mob_ai::tick(now);
        for (&zone, mobs) in all.iter_mut() {
            if mobs.lua_events.is_empty() && !mobs.mobs.values().any(|m| m.lua) {
                continue;
            }
            let players = maps.get_mut(&zone).unwrap_or(&mut nobody);
            let terrain = MapTerrain { world, map: zone };
            let mut pass = Pass { zones: self, map: zone, defs, loot, now, rand: &mut *rand };
            let mut run = LuaRun { pass: &mut pass, mobs, players, zone, world, terrain: &terrain, clock, in_hook: false };
            run.drain();
            let mut ids: Vec<u32> = run.mobs.mobs.values().filter(|m| m.lua && m.dead_until.is_none()).map(|m| m.id).collect();
            ids.sort_unstable();
            for id in ids {
                run.poll(id);
                run.drain();
            }
        }
    }
}
