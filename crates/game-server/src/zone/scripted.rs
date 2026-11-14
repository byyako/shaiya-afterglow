//! Scripted monsters in the zone (`docs/original/scripted-mobs.md`): each machine's turn in the one-second pass (section 3), the
//! functions (section 5) and handlers (section 6) acting on the zone, and the hand-backs to the machine (section 7). Part of the
//! zone: the callers hold both locks.

use super::*;
use crate::mob_ai::{self, Func, Handler, Machine, Value};
use crate::sync::Relock;
use shaiya_proto::chat;

/// `0x1109` kinds: to the target (`PAI_Say`), to the players around (`PAI_Say2`).
const SAY_TARGET: u8 = 0;
const SAY_AROUND: u8 = 1;
/// `ArrivalChek`: closer than this to the selected position.
const ARRIVED: f32 = 2.0;
/// The home limits of the handlers: `MoveRandom`, and `MoveRandomRun` and `MoveAtSelectedPosition`.
const WALK_LIMIT: f32 = 10.0;
const RUN_LIMIT: f32 = 20.0;
/// `IsExistUserAttackPoint`'s wait (`m_dwNextTargetingTime`).
const AGGRO_LOOK_EVERY: Duration = Duration::from_secs(5);

/// 3-D distance (`SVector::Distance`).
fn distance3(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The cells within `radius` of `cell` as the original counts them (`radius / 40` rounded to nearest), rows of z, then x.
fn cells_within(cell: (i32, i32), radius: f32) -> impl Iterator<Item = (i32, i32)> {
    let n = (radius / CELL).round_ties_even() as i32;
    (cell.1 - n..=cell.1 + n).flat_map(move |z| (cell.0 - n..=cell.0 + n).map(move |x| (x, z)))
}

/// The target a monster's state names: fighting it, or a machine's target.
pub(super) fn target_of(state: MobState) -> Option<u32> {
    match state {
        MobState::Fighting(t) | MobState::Scripted(Some(t)) | MobState::Lua(Some(t)) => Some(t),
        _ => None,
    }
}

/// Where an ordinary monster becomes idle (the chase giving up, back home): a monster with a machine starts it over
/// (`CMob::ResetFSM`, section 7), at its next turn or at once when that turn is running.
pub(super) fn become_idle(mobs: &mut MobMap, id: u32) {
    let Some(m) = mobs.mobs.get_mut(&id) else { return };
    if !m.scripted {
        m.state = MobState::Idle;
        return;
    }
    m.state = MobState::Scripted(None);
    if let Some(s) = mobs.scripts.get_mut(&id) {
        s.restart = true;
    }
}

/// A monster loses its target (it died, left, or cannot be fought): an ordinary one becomes idle; one with a machine runs it
/// without a target, and its `Battle` looks for another player and starts over only without one (deviation 6).
pub(super) fn lose_target(mobs: &mut MobMap, id: u32) {
    let Some(m) = mobs.mobs.get_mut(&id) else { return };
    m.state = if m.scripted { MobState::Scripted(None) } else { MobState::Idle };
}

/// Whether a player's blow or skill of this attack kind (0 melee, 1 ranged, 2 magic) can reach the monster: not when its script
/// turned that kind off (`SetEnableOnlyAttackType`, deviation 5; the original never reads the setting).
pub(super) fn takes_attack_kind(mobs: &MobMap, id: u32, kind: u8) -> bool {
    mobs.script_flags(id).is_none_or(|f| f.attack_kinds.get(usize::from(kind)).copied().unwrap_or(true))
}

/// A player's attack on a monster with a machine sets its flags (section 7): a hit always, a skill's miss only with battle type 0.
pub(super) fn mark_attacked(mobs: &mut MobMap, id: u32, miss: bool) {
    let Some(s) = mobs.scripts.get_mut(&id) else { return };
    if miss && s.flags.battle_type != 0 {
        return;
    }
    s.flags.attacked = true;
    s.flags.battle = true;
    s.flags.at_moment = true;
}

/// A monster's numbers as a player's attack meets them, with what its machine changed: the melee defence (`SetRec`) and the
/// defence element (`SerChangeRegist`; none under `wType` 68).
pub(super) fn scripted_fighter(mut f: Fighter, def: &MobDef, flags: Option<mob_ai::Flags>, no_element: bool) -> Fighter {
    let Some(flags) = flags else { return f };
    let add = f.kinds[0].defence - i32::from(def.defense);
    f.kinds[0].defence = (f32::from(def.defense) * flags.defence) as i32 + add;
    if let Some(element) = flags.element
        && !no_element
    {
        f.element = element;
    }
    f
}

/// One monster's turn of its machine.
struct Turn<'a, 'b, R> {
    pass: &'a mut Pass<'b, R>,
    mobs: &'a mut MobMap,
    players: &'a mut HashMap<u32, Placed>,
    id: u32,
    def: &'a MobDef,
    machine: &'a Machine,
    lines: &'a HashMap<i32, Vec<u8>>,
    terrain: &'a MapTerrain<'a>,
    world: &'a World,
    vars: Vec<Value>,
    state: usize,
    clock: u32,
    /// `0x1109` for party members on other maps, sent after the pass.
    elsewhere: &'a mut Vec<(u32, Vec<u8>)>,
}

impl<R: FnMut() -> u32> mob_ai::Host for Turn<'_, '_, R> {
    fn vars(&mut self) -> &mut Vec<Value> {
        &mut self.vars
    }

    fn call(&mut self, func: Func, args: &[Value]) -> Value {
        let arg = |i: usize| args.get(i).copied().unwrap_or_default();
        let Some(mob) = self.mob() else { return Value::void() };
        match func {
            Func::GetCurrentTime => return Value::dword(self.clock),
            Func::GetRandInt => return Value::int(((self.pass.rand)() % 100) as i32),
            Func::GetEnergyPercent => return Value::float(mob.hp as f32 / self.def.hp as f32),
            Func::IsAttackMe => return Value::int(i32::from(self.flags().attacked)),
            Func::IsBattle => return Value::int(i32::from(self.flags().battle)),
            Func::IsAttackMeAtMoment => {
                let f = self.flags_mut();
                let was = std::mem::take(&mut f.at_moment);
                return Value::int(i32::from(was));
            }
            Func::IsFirstAttack => return Value::int(i32::from(self.find_and_target().is_some())),
            Func::IsEndBattle => {
                // The original prints a debug line and leaves the result's type unset (section 10).
                tracing::debug!(mob = self.id, "IsEndBattle");
            }
            Func::GetCurrentUser => return Value::dword(target_of(mob.state).unwrap_or(0)),
            Func::SelectUser => {
                let player = arg(0).d;
                if player != 0 && self.players.contains_key(&player) {
                    set_target(self.mobs, self.players, self.id, player, true, self.pass.defs, self.pass.now, &mut *self.pass.rand);
                }
            }
            Func::IsExistUserAttackPoint => return Value::dword(self.top_aggro_player()),
            Func::GetCurrentMaxHpUser => return Value::dword(self.healthiest_attacker()),
            Func::GetDistanceFromExit => return Value::float(distance(mob.position, self.flags().exit)),
            Func::ArrivalChek => return Value::int(i32::from(distance3(mob.position, self.flags().selected) < ARRIVED)),
            Func::GetAttackCount => {
                let f = self.flags_mut();
                f.count = f.count.max(0);
                return Value::int(f.count);
            }
            Func::Say => self.say(arg(0).n, None),
            Func::Say2 => self.say(arg(0).n, Some(arg(1).n as f32)),
            Func::UseMobSkill => self.use_skill(arg(0).n),
            Func::AttackUser => self.attack_command(arg(0).d, arg(1).f),
            Func::SuchFrendlyandRun => self.find_quiet_friend(arg(0).f),
            Func::SetDrMove => {
                let speed = self.move_speed();
                let angle = ((self.pass.rand)() % 360) as f32;
                let length = mobs::step_length(speed, self.def.normal_step) as f32 + arg(0).f;
                let (dx, dz) = mobs::turned(length, angle);
                // Deviation 3: from the monster (the original keeps the bare offset, near the map's corner).
                self.flags_mut().selected = [mob.position[0] + dx, 0.0, mob.position[2] + dz];
            }
            Func::SetRunawayPosition => self.flags_mut().exit = mob.position,
            Func::SetStr => self.flags_mut().strength = arg(0).f,
            Func::SetRec => self.flags_mut().defence = arg(0).f,
            Func::SetAttackSpeed => self.flags_mut().attack_speed = arg(0).f,
            Func::SetHpChage => self.flags_mut().recover_ratio = arg(0).f,
            Func::SetRecoveryTime => self.flags_mut().recover_time = arg(0).n as u32,
            Func::SetEnableRecovery => self.flags_mut().recover = arg(0).n == 1,
            Func::SerChangeRegist => self.flags_mut().element = Some(arg(0).n as u8),
            Func::AddEnergy => self.add_energy(arg(0).n),
            Func::SetSummonMob => self.summon(arg(0).n, arg(1).f, arg(2).n, arg(3).n),
            Func::SetBattleType => self.flags_mut().battle_type = arg(0).n,
            // `m_bAttackEnableType`, which the new server reads (deviation 5).
            Func::SetEnableOnlyAttackType => self.flags_mut().attack_kinds = mob_ai::attack_kinds(arg(0).n),
            Func::SetEnableATAll => self.flags_mut().attack_kinds = [true; 4],
            Func::BeginCheckAttackCount => self.flags_mut().counting = true,
            Func::EndCheckAttackCount => self.flags_mut().counting = false,
            Func::ResetAttackCount => self.flags_mut().count = 0,
        }
        Value::void()
    }
}

impl<R: FnMut() -> u32> Turn<'_, '_, R> {
    fn mob(&self) -> Option<Mob> {
        self.mobs.mobs.get(&self.id).copied()
    }

    fn alive(&self) -> bool {
        self.mobs.mobs.get(&self.id).is_some_and(|m| m.dead_until.is_none())
    }

    fn flags(&self) -> mob_ai::Flags {
        self.mobs.script_flags(self.id).unwrap_or_default()
    }

    fn flags_mut(&mut self) -> &mut mob_ai::Flags {
        &mut self.mobs.scripts.entry(self.id).or_default().flags
    }

    /// `m_nMoveSpeed` as the handlers read it: the base speed with the debuffs, without the "cannot move" of a stop.
    fn move_speed(&self) -> i32 {
        i32::from(mobs::MOVE_SPEED) + self.mobs.effects(self.id).move_speed
    }

    /// The machine's turn (section 3): a pending start-over, the transitions, then the current state's update.
    fn run(&mut self) {
        if std::mem::take(&mut self.mobs.scripts.entry(self.id).or_default().restart) {
            self.restart();
        }
        if !self.alive() {
            return;
        }
        let from = self.state;
        let machine = self.machine;
        for t in &machine.states[from].transitions {
            let true_tests: usize = t.conditions.iter().map(|c| mob_ai::evaluate(c, self)).sum();
            if true_tests == t.tests && t.target != self.state {
                self.exit(from);
                // The original sets the current state after the new state's enter, so a start-over there is overruled.
                self.enter(t.target);
                self.state = t.target;
                break;
            }
            if !self.alive() {
                return;
            }
        }
        if !self.alive() {
            return;
        }
        let state = &machine.states[self.state];
        for c in &state.on_update {
            mob_ai::evaluate(c, self);
        }
        self.handle(state.handlers[1]);
    }

    /// `CMob::ResetFSM`: no target, not attacked or in battle, the variables at 0, the start state entered.
    fn restart(&mut self) {
        if let Some(m) = self.mobs.mobs.get_mut(&self.id) {
            m.state = MobState::Scripted(None);
        }
        let f = self.flags_mut();
        f.attacked = false;
        f.battle = false;
        f.attack_kinds = [true; 4];
        self.vars = self.machine.fresh_vars();
        self.state = self.machine.start;
        self.enter(self.machine.start);
    }

    fn enter(&mut self, state: usize) {
        let machine = self.machine;
        let s = &machine.states[state];
        for c in &s.on_enter {
            mob_ai::evaluate(c, self);
        }
        self.handle(s.handlers[0]);
    }

    fn exit(&mut self, state: usize) {
        let machine = self.machine;
        let s = &machine.states[state];
        for c in &s.on_exit {
            mob_ai::evaluate(c, self);
        }
        self.handle(s.handlers[2]);
    }

    fn handle(&mut self, handler: Option<Handler>) {
        if !self.alive() {
            return;
        }
        match handler {
            Some(Handler::Battle) => self.battle(),
            Some(Handler::MoveRandom) => self.move_random(false),
            Some(Handler::MoveRandomRun) => self.move_random(true),
            Some(Handler::MoveAtSelectedPosition) => self.move_to_selected(),
            Some(Handler::ResetUser) => {
                let f = self.flags_mut();
                f.attacked = false;
                f.battle = false;
                if let Some(m) = self.mobs.mobs.get_mut(&self.id) {
                    m.state = MobState::Scripted(None);
                }
            }
            Some(Handler::MoveRunAway | Handler::Debug) | None => {}
        }
    }

    /// `CMob::FindTarget`: the first player around within the chase range becomes the target (`SetTarget(player, 1)`).
    fn find_and_target(&mut self) -> Option<u32> {
        let mob = self.mob()?;
        let player = find_target(&mob, self.def, self.players)?;
        set_target(self.mobs, self.players, self.id, player, true, self.pass.defs, self.pass.now, &mut *self.pass.rand);
        Some(player)
    }

    /// Looks for another target, else starts over (`onBattle`, `PAI_Attack`).
    fn another_target_or_restart(&mut self) {
        if self.find_and_target().is_none() {
            self.restart();
        }
    }

    /// `Battle` (`CMob::onBattle`, `PAI_Attack`): at its attack time, not asleep or stunned, against a target on the map, alive and
    /// of another country, the ordinary fight turn (`AttackAI`); without one, another target or a start-over.
    fn battle(&mut self) {
        let Some(mob) = self.mob() else { return };
        let effects = self.mobs.effects(self.id);
        if mob.next_attack > self.pass.now || effects.sleep > 0 || effects.stun > 0 {
            return;
        }
        let MobState::Scripted(target) = mob.state else { return };
        let Some(target) = target.filter(|t| self.players.get(t).is_some_and(|p| targetable(p, self.def))) else {
            return self.another_target_or_restart();
        };
        fight_turn(self.pass, self.mobs, self.players, self.id, target, self.def, self.terrain, self.world);
        // A hand-back during the turn (the chase gave up) starts over at once; a kill leaves it looking for another target.
        if std::mem::take(&mut self.mobs.scripts.entry(self.id).or_default().restart) {
            return self.restart();
        }
        if self.alive() && self.mob().is_some_and(|m| m.state == MobState::Scripted(None)) {
            self.another_target_or_restart();
        }
    }

    /// `MoveRandom` / `MoveRandomRun` (`CMob::onMoveRandom`, `onMoveRandomRun`): a random step every turn, walking within 10 of
    /// home, running within 20; out of battle and away from home it goes home instead.
    fn move_random(&mut self, run: bool) {
        let Some(mob) = self.mob() else { return };
        let angle = ((self.pass.rand)() % 10_000) as f32;
        let extra = if run { 0 } else { ((self.pass.rand)() % 500 / 100) as i32 };
        let length = (mobs::step_length(self.move_speed(), self.def.normal_step) + extra) as f32;
        let (dx, dz) = mobs::turned(length, angle);
        let to = [mob.position[0] + dx, mob.position[1], mob.position[2] + dz];
        let limit = if run { RUN_LIMIT } else { WALK_LIMIT };
        if self.away_from_home(&mob, limit) {
            return;
        }
        let line = |p: [f32; 3]| (p[0] as i32, p[2] as i32);
        if distance(to, mob.home) < limit
            && self.world.is_line_free(self.terrain.map, line(mob.position), line(to))
            && self.world.is_safe_pos(self.terrain.map, to)
        {
            walk_mob(self.mobs, Some(self.players), self.id, to, u8::from(run));
        }
    }

    /// Out of battle and away from home (a chase path and more than 0.5, or more than `limit`): it goes home (state 3).
    fn away_from_home(&mut self, mob: &Mob, limit: f32) -> bool {
        if self.flags().battle {
            return false;
        }
        let from_home = distance(mob.position, mob.home);
        let trail = self.mobs.trails.get(&self.id).is_some_and(|t| !t.is_empty());
        if (trail && from_home > 0.5) || from_home > limit {
            if let Some(m) = self.mobs.mobs.get_mut(&self.id) {
                m.state = MobState::Home;
            }
            return true;
        }
        false
    }

    /// `MoveAtSelectedPosition` (`CMob::onMoveAtSelectedPosition`): one run to the selected position when it is within 20 of home
    /// and the line is free; otherwise the selected position becomes its own.
    fn move_to_selected(&mut self) {
        let Some(mob) = self.mob() else { return };
        if self.away_from_home(&mob, RUN_LIMIT) {
            return;
        }
        let f = self.flags_mut();
        f.selected[1] = 0.0;
        let selected = f.selected;
        let line = |p: [f32; 3]| (p[0] as i32, p[2] as i32);
        let free = self.world.is_line_free(self.terrain.map, line(mob.position), line(selected));
        if distance(selected, mob.home) >= RUN_LIMIT || !free {
            self.flags_mut().selected = mob.position;
            return;
        }
        if self.world.is_safe_pos(self.terrain.map, selected) {
            walk_mob(self.mobs, Some(self.players), self.id, selected, 1);
        }
    }

    /// `PAI_Say` / `PAI_Say2` (section 8): line `id` as `0x1109`, to the target or its party, or to the players within `radius`.
    fn say(&mut self, id: i32, radius: Option<f32>) {
        let Some(mob) = self.mob() else { return };
        let Some(text) = self.lines.get(&id) else { return };
        if text.is_empty() || text.len() >= chat::MAX_TEXT || !chat::markup_is_safe(text) {
            return;
        }
        let Some(radius) = radius else {
            let packet = chat::monster_say(SAY_TARGET, self.id, text);
            let Some(target) = target_of(mob.state).filter(|t| self.players.contains_key(t)) else { return };
            let members = self.pass.zones.parties.guard().party_of(target).map(|p| p.members.clone());
            for member in members.unwrap_or_else(|| vec![target]) {
                match self.players.get(&member) {
                    Some(p) => p.send(&packet),
                    None => self.elsewhere.push((member, packet.clone())),
                }
            }
            return;
        };
        let packet = chat::monster_say(SAY_AROUND, self.id, text);
        let cells: Vec<(i32, i32)> = cells_within(mob.cell, radius).collect();
        for p in self.players.values().filter(|p| cells.contains(&p.cell) && distance(p.occupant.position, mob.position) <= radius) {
            p.send(&packet);
        }
    }

    /// `UseMobSkill` (`PAI_AttackSkill`): the level-100 row of the skill on the target, no range check. An unknown id does nothing
    /// (the original crashes).
    fn use_skill(&mut self, skill: i32) {
        let Some(mob) = self.mob() else { return };
        let Some(target) = target_of(mob.state).filter(|t| self.players.contains_key(t)) else { return };
        let Some(s) = u16::try_from(skill).ok().and_then(|id| self.pass.zones.item_skills.get().and_then(|t| t.get(&id)).copied()) else {
            return;
        };
        let effects = self.mobs.effects(self.id);
        if effects.sleep > 0 || effects.stun > 0 {
            return;
        }
        mob_skills::use_skill(self.pass, self.mobs, self.players, self.id, target, self.def, usize::from(mob.entry_used), &s);
    }

    /// `AttackUser` (`PAI_AttackCommand`): every other monster within `radius` (3-D) takes the player as target.
    fn attack_command(&mut self, player: u32, radius: f32) {
        let Some(mob) = self.mob() else { return };
        if player == 0 || !self.players.contains_key(&player) {
            return;
        }
        let ids: Vec<u32> = cells_within(mob.cell, radius)
            .filter_map(|c| self.mobs.cells.get(&c))
            .flat_map(|ids| ids.iter().copied())
            .filter(|&other| other != self.id && self.mobs.mobs.get(&other).is_some_and(|m| distance3(m.position, mob.position) <= radius))
            .collect();
        for other in ids {
            set_target(self.mobs, self.players, other, player, true, self.pass.defs, self.pass.now, &mut *self.pass.rand);
        }
    }

    /// `SuchFrendlyandRun` (`PAI_FindNonBattleMob`): the first other monster within `radius` (3-D) that is not fighting gives the
    /// selected position; none, its own.
    fn find_quiet_friend(&mut self, radius: f32) {
        let Some(mob) = self.mob() else { return };
        let found = cells_within(mob.cell, radius)
            .filter_map(|c| self.mobs.cells.get(&c))
            .flat_map(|ids| ids.iter().copied())
            .filter(|&other| other != self.id)
            .filter_map(|other| self.mobs.mobs.get(&other).copied())
            .find(|m| {
                distance3(m.position, mob.position) < radius
                    && match self.mobs.script_flags(m.id) {
                        Some(flags) => !flags.battle,
                        None => m.state == MobState::Idle && m.dead_until.is_none(),
                    }
            });
        self.flags_mut().selected = found.map_or(mob.position, |m| m.position);
    }

    /// `IsExistUserAttackPoint` (`CMob::GetMaxAttackUserID`): at most every 5 s, the top-aggro player when it is not the target.
    fn top_aggro_player(&mut self) -> u32 {
        let now = self.pass.now;
        let top = self.mobs.top_aggro(self.id);
        let Some(m) = self.mobs.mobs.get_mut(&self.id) else { return 0 };
        if m.next_retarget.is_some_and(|t| t > now) {
            return 0;
        }
        if let Some(top) = top.filter(|&t| Some(t) != target_of(m.state)) {
            return top;
        }
        m.next_retarget = Some(now + AGGRO_LOOK_EVERY);
        0
    }

    /// `GetCurrentMaxHpUser` (`CMob::FindAttackUserMaxHP`): of the players in its damage record who are here, of another country
    /// and alive, the one with the most HP (the first of equals).
    fn healthiest_attacker(&self) -> u32 {
        let Some(record) = self.mobs.damage.get(&self.id) else { return 0 };
        let mut best: Option<(u16, u32)> = None;
        for id in record.iter().filter_map(|h| h.player()) {
            let Some(p) = self.players.get(&id).filter(|p| targetable(p, self.def)) else { continue };
            let hp = p.occupant.vitals.health[0];
            if best.is_none_or(|(most, _)| hp > most) {
                best = Some((hp, id));
            }
        }
        best.map_or(0, |(_, id)| id)
    }

    /// `AddEnergy` (`PAI_AddEnergy`): HP up to the maximum; `0x0610` within 60 with what was added.
    fn add_energy(&mut self, amount: i32) {
        let max = self.def.hp;
        let Some(m) = self.mobs.mobs.get_mut(&self.id) else { return };
        let after = (i64::from(m.hp) + i64::from(amount)).clamp(0, i64::from(max));
        let gained = after - i64::from(m.hp);
        m.hp = after as u32;
        if gained > 0 {
            self.pass.zones.mob_record(self.pass.map, m, gained as u32, crate::game_log::MobAction::Heal, None, None);
            send_around(Some(self.players), m.position, RECOVER_RANGE, &cp::mob_recover(self.id, gained as i32));
        }
    }

    /// `SetSummonMob` (`PAI_CreateMob`, `CZone::MobGen`, section 9): `count` tries at random spots within `radius`, each a new
    /// monster that never respawns, taking only the attack `kinds` (deviation 5).
    fn summon(&mut self, mob_id: i32, radius: f32, count: i32, kinds: i32) {
        let Some(mob) = self.mob() else { return };
        let Some(def) = u16::try_from(mob_id).ok().and_then(|id| self.pass.defs.get(id)).filter(|d| d.hp != 0 && d.level != 0) else {
            return;
        };
        let spread = (2.0 * radius) as i32;
        if spread <= 0 {
            return;
        }
        let map = self.terrain.map;
        for _ in 0..count.max(0) {
            let x = ((self.pass.rand)() % spread as u32) as f32 - radius + mob.position[0];
            let z = ((self.pass.rand)() % spread as u32) as f32 - radius + mob.position[2];
            let at = [x, mob.position[1], z];
            let (cx, cz) = (x.round_ties_even() as i32, z.round_ties_even() as i32);
            if !self.world.is_safe_pos(map, at) || self.world.is_blocked(map, cx, cz) {
                continue;
            }
            let new = self.pass.zones.new_mob(def, at, usize::MAX, mobs::Origin::Area, self.pass.now, &mut *self.pass.rand);
            self.mobs.insert(new);
            if kinds != 4 {
                self.mobs.scripts.entry(new.id).or_default().flags.attack_kinds = mob_ai::attack_kinds(kinds);
            }
            let packets = mob_enter(&new, true);
            for p in self.players.values().filter(|p| in_view(p.cell, new.cell)) {
                packets.iter().for_each(|packet| p.send(packet));
            }
        }
    }
}

impl Zones {
    /// The scripted monsters' second (`CMob::UpdateIdle` for `m_bUsePAIS`, section 3): each one running its machine takes its turn;
    /// one going home takes a `ReturnHome` step (as the attack pass also does).
    pub fn scripts(&self, now: Instant, world: &World, defs: &MobDefs, loot: (&DropTables, &ItemDefs), rand: &mut impl FnMut() -> u32) {
        let library = world.mob_ai();
        if library.machines.is_empty() {
            return;
        }
        let mut maps = self.maps();
        let mut all = self.mobs.guard();
        let mut nobody = HashMap::new();
        let mut elsewhere = Vec::new();
        let clock = mob_ai::tick(now);
        for (&map, mobs) in all.iter_mut() {
            let mut ids: Vec<u32> = mobs.mobs.values().filter(|m| m.scripted && m.dead_until.is_none()).map(|m| m.id).collect();
            if ids.is_empty() {
                continue;
            }
            ids.sort_unstable();
            let players = maps.get_mut(&map).unwrap_or(&mut nobody);
            let terrain = MapTerrain { world, map };
            let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
            for id in ids {
                let Some(mob) = mobs.mobs.get(&id).copied() else { continue };
                let Some(def) = defs.get(mob.mob_id) else { continue };
                let Some(machine) = library.machines.get(&def.ai) else { continue };
                match mob.state {
                    MobState::Scripted(_) => {
                        let script = mobs.scripts.entry(id).or_default();
                        let (vars, state) = (std::mem::take(&mut script.vars), script.state);
                        let mut turn = Turn {
                            pass: &mut pass,
                            mobs,
                            players,
                            id,
                            def,
                            machine,
                            lines: &library.lines,
                            terrain: &terrain,
                            world,
                            vars,
                            state,
                            clock,
                            elsewhere: &mut elsewhere,
                        };
                        turn.run();
                        let (vars, state) = (turn.vars, turn.state);
                        if let Some(script) = mobs.scripts.get_mut(&id) {
                            script.vars = vars;
                            script.state = state;
                        }
                    }
                    MobState::Home => go_home(mobs, players, id, def, now, &terrain, world, self.smooth_chase()),
                    MobState::Idle | MobState::Fighting(_) | MobState::Lua(_) => {}
                }
            }
        }
        for (member, packet) in elsewhere {
            if let Some(p) = maps.values().find_map(|players| players.get(&member)) {
                p.send(&packet);
            }
        }
    }
}
