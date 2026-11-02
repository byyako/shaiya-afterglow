//! The monster helpers of the zone's passes: entering the view and walking, finding and taking targets, the fight turn, the
//! normal attack and the way home.

use super::*;

/// `0x0601` of a monster (`new` when it has just appeared), then its `0x0608` (`docs/original/client-extras.md` deviation 1).
pub(super) fn mob_enter(mob: &Mob, new: bool) -> [Vec<u8>; 2] {
    [w::mob_enter(mob.id, new, mob.mob_id, mob.position[0], mob.position[2]), w::mob_max_hp(mob.id, mob.max_hp, mob.hp)]
}

/// The monsters in the cells of `view` that are not in the view of `except` (all of them without one).
pub(super) fn mobs_in_view(mobs: Option<&MobMap>, view: (i32, i32), except: Option<(i32, i32)>) -> Vec<Mob> {
    let Some(mobs) = mobs else { return Vec::new() };
    view_cells(view)
        .filter(|&cell| except.is_none_or(|other| !in_view(other, cell)))
        .filter_map(|cell| mobs.cells.get(&cell))
        .flatten()
        .filter_map(|id| mobs.mobs.get(id).copied())
        // Bodies are not sent (`CCell::SendCellInfoEnter`).
        .filter(|mob| mob.dead_until.is_none())
        .collect()
}

/// Moves a monster (`CZone::MoveMob`; the caller has checked the spot is on the map): a monster changing cells leaves and enters
/// the views around (`0x0602`/`0x0601`, still at the old spot), then everyone in the view of its cell gets `0x0603` with the
/// motion (0 walk, 1 run).
pub(super) fn walk_mob(mobs: &mut MobMap, players: Option<&HashMap<u32, Placed>>, id: u32, to: [f32; 3], motion: u8) {
    let Some(mob) = mobs.mobs.get_mut(&id) else { return };
    let old = *mob;
    mob.position = to;
    // A monster keeps its cell until it is clear of the cell's margin (`CCell::IsInnerCellView`).
    let cell = if keeps_cell(old.cell, to) { old.cell } else { cell_of(to) };
    if cell != old.cell {
        mobs.change_cell(id, cell);
        for p in players.into_iter().flat_map(|p| p.values()) {
            match (in_view(p.cell, old.cell), in_view(p.cell, cell)) {
                (true, false) => p.send(&w::mob_leave(id)),
                (false, true) => mob_enter(&old, false).iter().for_each(|packet| p.send(packet)),
                _ => {}
            }
        }
    }
    let packet = w::mob_move(id, motion, to[0], to[2]);
    for p in players.into_iter().flat_map(|p| p.values()).filter(|p| in_view(p.cell, cell)) {
        p.send(&packet);
    }
}

/// The next step time after a step to `to` (`None`: not sent): the original's `next`, or with smoothing the client's arrival there
/// less a tick (`docs/original/chase-smoothing.md` deviations 1 and 4; walking `motion` 0 by `NormalStep`/`NormalTime`, running by
/// `ChaseStep`/`ChaseTime`). Keeps the arrival time and what the move tick may go on with (`moving`, only for a sent, paced step).
/// Called before the move.
#[allow(clippy::too_many_arguments)]
pub(super) fn paced_next(
    mobs: &mut MobMap,
    id: u32,
    def: &MobDef,
    to: Option<[f32; 3]>,
    motion: u8,
    next: Instant,
    now: Instant,
    smooth: bool,
    moving: Moving,
) -> Instant {
    let speed = mobs.speeds(id).1;
    let Some(m) = mobs.mobs.get_mut(&id) else { return next };
    m.moving = Moving::None;
    let Some(to) = to.filter(|_| smooth) else { return next };
    let (step, time) = if motion == 0 { (def.normal_step, def.normal_time) } else { (def.chase_step, def.chase_time) };
    let Some(v) = pacing::mob_speed(mobs::step_length(i32::from(speed), step), time) else { return next };
    let (eta, paced) = pacing::paced(now, m.client_eta, distance(m.position, to), v);
    m.client_eta = Some(eta);
    m.moving = moving;
    paced
}

/// Nothing for the move tick to go on with.
pub(super) fn stop_moving(mobs: &mut MobMap, id: u32) {
    if let Some(m) = mobs.mobs.get_mut(&id) {
        m.moving = Moving::None;
    }
}

/// Whether a monster may take this player as a target: alive and of another country (`CUser::EnableTargeting`, the country
/// check of `CMob::AttackAI`), and not an invisible or untouchable staff member (`EnableTargeting`).
pub(super) fn targetable(p: &Placed, def: &MobDef) -> bool {
    !p.dead && p.occupant.visible && p.occupant.attackable && u32::from(p.occupant.shape.country) != u32::from(def.country)
}

/// An aggressive monster's look around (`CMob::FindTarget`): the first player in the 3 x 3 cells around it (rows of z, then x)
/// within its chase range, not hidden or transformed. Players of the monster's own country are passed over here (the original
/// takes them and drops them on its next attack turn).
pub(super) fn find_target(mob: &Mob, def: &MobDef, players: &HashMap<u32, Placed>) -> Option<u32> {
    let mut found: Option<((i32, i32), u32)> = None;
    for p in players.values() {
        if !near_cells(p.cell, mob.cell)
            || !targetable(p, def)
            || p.form.shape != 0
            || distance(p.occupant.position, mob.position) > f32::from(def.chase_range)
        {
            continue;
        }
        let order = (p.cell.1, p.cell.0);
        if found.is_none_or(|(best, id)| order < best || (order == best && p.occupant.id < id)) {
            found = Some((order, p.occupant.id));
        }
    }
    found.map(|(_, id)| id)
}

/// A monster taking a target (`CMob::SetTarget`): fighting it, its next move now, and the first ready time of each skill entry
/// (`docs/original/mob-skills.md` section 2): none for entry 0 or a skill without reuse time, else a random share of the reuse time.
pub(super) fn take_target(mob: &mut Mob, def: &MobDef, player: u32, now: Instant, rand: &mut impl FnMut() -> u32) {
    mob.state = MobState::Fighting(player);
    mob.next_move = now;
    for (i, entry) in def.attacks.iter().enumerate() {
        let Some(info) = entry.info.filter(|_| entry.ok != 0) else { continue };
        let reset = u32::from(info.reset_time) * 1000;
        mob.entry_ready[i] = (i != 0 && reset != 0).then(|| now + Duration::from_millis(u64::from(rand() % reset)));
    }
}

/// `CMob::SetTarget` (`docs/original/mob-fight.md` section 2): an idle monster fights the player; with `help`, it calls the
/// monsters around by its `AI` (2 the same monster, 3 aggressive ones, 5 any).
#[allow(clippy::too_many_arguments)]
pub(super) fn set_target(
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    id: u32,
    player: u32,
    help: bool,
    defs: &MobDefs,
    now: Instant,
    rand: &mut impl FnMut() -> u32,
) {
    let Some(p) = players.get(&player) else { return };
    let Some(mob) = mobs.mobs.get_mut(&id) else { return };
    let Some(def) = defs.get(mob.mob_id) else { return };
    // A Lua monster: `SetTargetEx` and event 1, only when idle or moving (`docs/original/instances.md` section 11).
    if mob.lua {
        return lua::lua_set_target(mobs, players, id, player, def, now, rand);
    }
    // A monster running its machine takes the target and stays in it, with no call for help; each skill entry, the first
    // included, gets its first ready time (`docs/original/scripted-mobs.md` section 7).
    if let MobState::Scripted(_) = mob.state {
        if mob.dead_until.is_none() {
            mob.state = MobState::Scripted(Some(player));
            for (i, entry) in def.attacks.iter().enumerate() {
                let Some(info) = entry.info.filter(|_| entry.ok != 0) else { continue };
                let reset = u32::from(info.reset_time) * 1000;
                mob.entry_ready[i] = (reset != 0).then(|| now + Duration::from_millis(u64::from(rand() % reset)));
            }
        }
        return;
    }
    if mob.state != MobState::Idle || mob.dead_until.is_some() || p.dead {
        return;
    }
    take_target(mob, def, player, now, rand);
    let caller = *mob;
    if !help || !matches!(def.ai, 2 | 3 | 5) || !targetable(p, def) {
        return;
    }
    let mut helpers = Vec::new();
    for dz in -NEAR_CELLS..=NEAR_CELLS {
        for dx in -NEAR_CELLS..=NEAR_CELLS {
            let Some(ids) = mobs.cells.get(&(caller.cell.0 + dx, caller.cell.1 + dz)) else { continue };
            for other in ids.iter().filter_map(|other| mobs.mobs.get(other)) {
                let Some(other_def) = defs.get(other.mob_id) else { continue };
                let fits = match def.ai {
                    2 => other.mob_id == caller.mob_id,
                    3 => matches!(other_def.ai, 1 | 3),
                    _ => true,
                };
                if fits
                    && other.id != id
                    && other.state == MobState::Idle
                    && other.dead_until.is_none()
                    && other_def.ai < crate::mob_ai::AI_LUA
                    && distance(other.position, caller.position) <= f32::from(def.chase_range)
                {
                    helpers.push(other.id);
                }
            }
        }
    }
    for helper in helpers {
        if let Some(m) = mobs.mobs.get_mut(&helper)
            && let Some(def) = defs.get(m.mob_id)
        {
            take_target(m, def, player, now, rand);
        }
    }
}

/// `CMob::ReTargetByAggro`: at most every 5 s, the player with the most aggro becomes the target when it is another player,
/// on the map, targetable and within 45.
pub(super) fn retarget(mobs: &mut MobMap, players: &HashMap<u32, Placed>, id: u32, def: &MobDef, now: Instant) {
    let top = mobs.top_aggro(id);
    let Some(mob) = mobs.mobs.get_mut(&id) else { return };
    if mob.next_retarget.is_some_and(|t| t > now) {
        return;
    }
    mob.next_retarget = Some(now + fight::RETARGET_EVERY);
    let Some(top) = top else { return };
    if scripted::target_of(mob.state) == Some(top) {
        return;
    }
    if let Some(p) = players.get(&top)
        && targetable(p, def)
        && distance(p.occupant.position, mob.position) <= fight::LEASH
    {
        // A monster running its machine keeps it (`docs/original/scripted-mobs.md` deviation 4; the original puts it in state 1).
        mob.state = match mob.state {
            MobState::Scripted(_) => MobState::Scripted(Some(top)),
            _ => MobState::Fighting(top),
        };
        mob.next_move = now;
    }
}

/// A sleeping monster wakes up (`CMob::RemApplySkillSleep`): its sleep debuffs go without their end; `0x0609` within 60 when its
/// speeds change.
pub(super) fn wake(players: &HashMap<u32, Placed>, mobs: &mut MobMap, id: u32) {
    let before = mobs.speeds(id);
    let Some(list) = mobs.debuffs.get_mut(&id) else { return };
    list.retain(|d| d.def.kind != buffs::SLEEP);
    let after = mobs.speeds(id);
    if after != before
        && let Some(m) = mobs.mobs.get(&id)
    {
        send_around(Some(players), m.position, SPEED_RANGE, &cp::mob_speed(id, after.0, after.1));
    }
}

/// A fighting monster's turn (`CMob::UpdateAttack`, `CMob::AttackAI`): the first usable attack entry from 2 down to 0, a chase
/// or the attack; without one, a wait. Waits use the monster's attack speed with its debuffs.
#[allow(clippy::too_many_arguments)]
pub(super) fn fight_turn<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    target: u32,
    def: &MobDef,
    terrain: &MapTerrain,
    world: &World,
) {
    let now = pass.now;
    let drop_target = |mobs: &mut MobMap| scripted::lose_target(mobs, id);
    let Some(p) = players.get(&target).filter(|p| targetable(p, def)) else { return drop_target(mobs) };
    let at = p.occupant.position;
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let speeds = mobs.speeds(id);
    for i in [2, 1, 0] {
        let entry = def.attacks[i];
        if mob.entry_ready[i].is_some_and(|t| now < t) {
            continue;
        }
        // `byOK` 6 runs `ReTargetByAggro` only when taunted (`docs/original/mob-skills.md` section 2).
        let aggro_first = match entry.ok {
            6 => match mob_skills::ok_six(mobs, players, id, def) {
                Some(taunted) => taunted,
                None => continue,
            },
            _ if fight::entry_usable(&entry, mob.hp, def.hp) => true,
            _ => continue,
        };
        if aggro_first {
            retarget(mobs, players, id, def, now);
        }
        if let Some(m) = mobs.mobs.get_mut(&id) {
            m.entry_used = i as u8;
        }
        if let Some(info) = entry.info {
            return mob_skills::skill_turn(pass, mobs, players, id, target, def, i, &info, terrain, world);
        }
        let ready = |mobs: &mut MobMap| {
            if i != 0
                && let Some(m) = mobs.mobs.get_mut(&id)
            {
                m.entry_ready[i] = Some(fight::entry_wait(now, speeds.0, &entry));
            }
        };
        if !fight::walks(def) {
            if fight::stationary_reach(mob.position, at, def, entry.range) {
                mob_strike(pass, mobs, players, id, target, def, &entry);
                ready(mobs);
            } else {
                drop_target(mobs);
            }
            return;
        }
        let path = mobs.paths.entry(id).or_default();
        let chase =
            fight::chase(mob.position, mob.home, at, def, entry.range, speeds.1, mob.next_chase, now, terrain, path, def.chase_time);
        if !matches!(chase, Chase::Wait | Chase::Step { .. }) {
            stop_moving(mobs, id);
        }
        let m = mobs.mobs.get_mut(&id).expect("listed");
        match chase {
            Chase::InRange => {
                mob_strike(pass, mobs, players, id, target, def, &entry);
                ready(mobs);
            }
            Chase::Wait => {}
            Chase::GoHome => m.state = MobState::Home,
            Chase::Drop => scripted::become_idle(mobs, id),
            Chase::Unfair => {
                m.fair = false;
                m.state = MobState::Home;
            }
            Chase::Step { to, next, trail } => {
                fight::add_trail(mobs.trails.entry(id).or_default(), trail, terrain);
                let to = to.filter(|to| world.is_safe_pos(terrain.map, *to));
                let moving = Moving::Chase { target, range: entry.range, wait: def.chase_time };
                let next = paced_next(mobs, id, def, to, 1, next, now, pass.zones.smooth_chase(), moving);
                mobs.mobs.get_mut(&id).expect("listed").next_chase = next;
                if let Some(to) = to {
                    walk_mob(mobs, Some(players), id, to, 1);
                }
            }
        }
        return;
    }
    if let Some(m) = mobs.mobs.get_mut(&id) {
        m.next_attack = fight::idle_wait(now, speeds.0);
    }
}

/// A monster's normal attack on a player (`CMob::Attack`, `docs/original/mob-fight.md` section 6, with the applied skills of
/// `docs/original/skills-buffs.md` section 10): nothing with its melee disabled; the player stands up; a block is a miss; a hit
/// may be reflected onto the monster (`0x051B`); give-up SP/MP pays first; a player killed dies (`0x0504` to its view, its
/// buffs that end with death gone) and the monster drops it; a player left alive may get its low-HP passive. Then `0x0605`
/// within 60 of the monster.
pub(super) fn mob_strike<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    target: u32,
    def: &MobDef,
    entry: &mobs::MobAttack,
) {
    let now = pass.now;
    let effects = mobs.effects(id);
    if effects.disabled[0] != 0 {
        return;
    }
    let flags = mobs.script_flags(id).unwrap_or_default();
    let Some(mob) = mobs.mobs.get_mut(&id) else { return };
    mob.next_attack = fight::next_attack(now, effects.speeds().0, def, flags.attack_speed);
    let at = mob.position;
    let Some(p) = players.get_mut(&target) else { return };
    if p.occupant.shape.motion == w::MOTION_SIT {
        p.occupant.shape.motion = w::MOTION_STAND;
        let _ = p.occupant.events.send(ZoneEvent::StoodUp);
    }
    let block = p.occupant.buffs.block(0, pass.rand);
    if let Some(used_up) = block.used_up {
        p.send(&sk::applied_end(used_up.key));
        let _ = p.occupant.events.send(ZoneEvent::Buffs);
    }
    let guard = p.occupant.vitals.guard;
    let blow = if block.blocked {
        combat::Blow { result: combat::RESULT_MISS, damage: 0 }
    } else {
        let fighter = combat::monster_with(def, &effects);
        combat::mob_strike(&fighter, entry.damage, entry.damage_add, flags.strength, &guard, p.occupant.vitals.guard_element, pass.rand)
    };
    if blow.result == combat::RESULT_MISS {
        send_around(Some(players), at, ATTACK_RANGE, &cp::mob_attack_values(blow.result, id, target, [0; 3]));
        return;
    }
    // A hit breaks a cast, wakes and ends hiding (`CUser::StatusCastingStop`, `docs/original/mob-skills.md` section 9); a duel ends
    // (`CUser::PvPChkLeaveByAttack`).
    mob_skills::struck(players, target);
    duel::hit(pass.zones, pass.map, players, target, None, pass.now);
    let p = &players[&target];
    let reflected = p.occupant.buffs.reflects(0, guard.luc, guard.wis, pass.rand).is_some_and(|skill| !def.resist_skills.contains(&skill));
    let shown_damage = if reflected { [0; 3] } else { mob_skills::hurt_player(pass, mobs, players, id, target, [blow.damage, 0, 0]).0 };
    send_around(Some(players), at, ATTACK_RANGE, &cp::mob_attack_values(blow.result, id, target, shown_damage));
    if reflected {
        mob_skills::reflect_onto(pass, mobs, players, id, target, [blow.damage, 0, 0]);
    }
    // The player's armour lapis back at the monster (`docs/original/item-options.md` section 5).
    item_options(pass, mobs, players, target, SkillTarget::Mob(id), Worn::Armour, false, true);
}

/// A monster's turn going home (`CMob::ReturnHome`, section 5); at home it is reset (`CMob::ResetAttack`): HP full, attack
/// times and the damage record cleared, `0x0305` and `0x0609` within 60.
#[allow(clippy::too_many_arguments)]
pub(super) fn go_home(
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    id: u32,
    def: &MobDef,
    now: Instant,
    terrain: &MapTerrain,
    world: &World,
    smooth: bool,
) {
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let move_speed = mobs.speeds(id).1;
    let (trail, path) = (mobs.trails.entry(id).or_default(), mobs.paths.entry(id).or_default());
    let homeward = fight::homeward(mob.position, mob.home, def, trail, path, move_speed, mob.next_chase, now, terrain);
    if !matches!(homeward, Homeward::Wait | Homeward::Step { .. }) {
        stop_moving(mobs, id);
    }
    match homeward {
        Homeward::Wait => {}
        Homeward::Arrived => {
            let m = mobs.mobs.get_mut(&id).expect("listed");
            m.state = MobState::Idle;
            (m.hp, m.sp, m.mp) = (def.hp, def.sp, def.mp);
            m.entry_ready = [None; 3];
            m.fair = true;
            mobs.damage.remove(&id);
            mobs.debuffs.remove(&id);
            scripted::become_idle(mobs, id);
            send_around(Some(players), mob.position, RECOVER_RANGE, &w::mob_hp(id, def.hp));
            send_around(Some(players), mob.position, RECOVER_RANGE, &cp::mob_speed(id, mobs::ATTACK_SPEED, mobs::MOVE_SPEED));
        }
        Homeward::Step { to, next } => {
            let to = Some(to).filter(|to| world.is_safe_pos(terrain.map, *to));
            let next = paced_next(mobs, id, def, to, 1, next, now, smooth, Moving::Home);
            mobs.mobs.get_mut(&id).expect("listed").next_chase = next;
            if let Some(to) = to {
                walk_mob(mobs, Some(players), id, to, 1);
            }
        }
        Homeward::Settle => {
            let m = mobs.mobs.get_mut(&id).expect("listed");
            m.home = m.position;
            scripted::become_idle(mobs, id);
        }
        Homeward::Idle => scripted::become_idle(mobs, id),
    }
}
