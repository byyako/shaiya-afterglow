//! Monsters using skills (`docs/original/mob-skills.md`): the skill entries of the attack pass, attacks, debuffs and heals,
//! self-destruct, and what a monster's hit does to the player (`CUser::Damage`, `CUser::StatusCastingStop`). Part of the zone: the
//! callers hold both locks.

use super::*;

/// The damage record's entry for a monster that killed itself (`wType` 23): never a player.
pub(super) const SELF_DESTRUCT: u32 = u32::MAX;

/// `byOK` 6 (section 2): `Some(true)` when the monster is taunted, `Some(false)` when its damage record holds a player of another
/// country who is in the zone and alive (`CMob::FindAttackUserMinHP`; its `SetTarget` changes nothing in a fight), `None` otherwise.
pub(super) fn ok_six(mobs: &MobMap, players: &HashMap<u32, Placed>, id: u32, def: &MobDef) -> Option<bool> {
    if mobs.debuffs.get(&id).is_some_and(|list| list.iter().any(|d| d.def.kind == buffs::TAUNT)) {
        return Some(true);
    }
    let record = mobs.damage.get(&id)?;
    record.iter().any(|h| h.player().and_then(|id| players.get(&id)).is_some_and(|p| targetable(p, def))).then_some(false)
}

/// A packet for the players within 60 of the monster and always for the player it names (`CZone::PSendView` with its extra id).
fn send_hit(players: &HashMap<u32, Placed>, at: [f32; 3], named: u32, packet: &[u8]) {
    for p in players.values().filter(|p| p.occupant.id == named || distance(p.occupant.position, at) <= ATTACK_RANGE) {
        p.send(packet);
    }
}

/// A monster's hit lands (`CUser::StatusCastingStop`, section 9): the player's hiding or transformation (but Evolution) ends, it
/// wakes up, and its connection breaks a cast.
pub(super) fn struck(players: &mut HashMap<u32, Placed>, id: u32) {
    shape_and_sleep_end(players, id);
    // Summoning stops, riders get off (`docs/original/vehicles.md` section 2).
    super::vehicle::casting_stop(players, id);
    if let Some(p) = players.get(&id) {
        let _ = p.occupant.events.send(ZoneEvent::Struck);
    }
}

/// `CUser::RemApplySkillToMOB` and `CUser::RemApplySkillSleep`: the first hiding or transformation ends (Evolution excepted), and
/// every sleep debuff goes without its end; `0x050E` for each.
pub(super) fn shape_and_sleep_end(players: &mut HashMap<u32, Placed>, id: u32) {
    let Some(p) = players.get_mut(&id) else { return };
    let mut ended = Vec::new();
    if p.form.shape != 0 {
        ended.extend(p.occupant.buffs.end_shape_for_attack());
    }
    ended.extend(p.occupant.buffs.wake());
    if ended.is_empty() {
        return;
    }
    for a in &ended {
        p.send(&sk::applied_end(a.key));
    }
    let _ = p.occupant.events.send(ZoneEvent::Buffs);
    reform(players, id, false);
}

/// `CUser::Damage(2, monster, damage)` (`docs/original/mob-fight.md` section 6 step 5, `docs/original/mob-skills.md` section 5):
/// nothing for a dead player or one without HP; SP and MP lose their parts; give-up SP/MP pays the HP damage first; HP at or below the
/// damage: the player dies (its buffs that end with death go, the monster drops it, `0x0504` to its view); otherwise the low-HP
/// passive. Returns the HP, SP, MP taken and whether the player died.
pub(super) fn hurt_player<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    mob: u32,
    victim: u32,
    damage: [u16; 3],
) -> ([u16; 3], bool) {
    hurt_player_by(pass, mobs, players, (cp::KILLER_MOB, mob), victim, damage)
}

/// `CUser::Damage(kind, attacker, damage)` by a monster or a player (`by`: `cp::KILLER_MOB` or `cp::KILLER_PLAYER` and its id):
/// the HP damage goes into the victim's damage record first (`CDamage::Add`), and a death is the killer of record's
/// (`docs/original/pvp.md` sections 5 and 6).
pub(super) fn hurt_player_by<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    by: (u8, u32),
    victim: u32,
    damage: [u16; 3],
) -> ([u16; 3], bool) {
    let (shown, death) = hurt_player_skip(pass, mobs, players, by, victim, damage, None);
    (shown, death.is_some())
}

/// `hurt_player_by` where the player `skip` does not get the death's `0x0504` (its caller puts it among its own packets); a death
/// returns that packet.
pub(super) fn hurt_player_skip<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    by: (u8, u32),
    victim: u32,
    damage: [u16; 3],
    skip: Option<u32>,
) -> ([u16; 3], Option<Vec<u8>>) {
    let (now, zones) = (pass.now, pass.zones);
    let Some(p) = players.get_mut(&victim) else { return ([0; 3], None) };
    if p.dead || p.occupant.vitals.health[0] == 0 {
        return ([0; 3], None);
    }
    super::pvp::record(&mut p.harm, by.0, by.1, damage[0]);
    // `0x0A04` (`CMob::SendLogBothMob`): the monster's attack, naming the player.
    if by.0 == cp::KILLER_MOB
        && let Some(mob) = mobs.mobs.get(&by.1)
    {
        let name = String::from_utf8_lossy(&p.occupant.shape.name).trim_end_matches('\0').to_owned();
        zones.mob_record(pass.map, mob, u32::from(damage[0]), crate::game_log::MobAction::Attack, Some(&name), None);
    }
    let effects = p.occupant.buffs.effects();
    // The armour wears first (`CUser::QualityByDefense`).
    if effects.shape == 0 && !effects.no_wear {
        let _ = p.occupant.events.send(ZoneEvent::Worn);
    }
    let give_up = effects.give_up;
    // Health is HP, MP, SP; damage HP, SP, MP.
    let v = &mut p.occupant.vitals;
    v.health[2] = v.health[2].saturating_sub(damage[1]);
    v.health[1] = v.health[1].saturating_sub(damage[2]);
    let [hp, sp_paid, mp_paid] = buffs::give_up(give_up, damage[0], &mut v.health);
    // Absorption from lapis and armour enchants, at least 1 left; `0x0525` tells the player (`docs/original/lapis.md` section 2).
    let (hp, absorbed) = combat::absorb(hp, v.absorption);
    if absorbed > 0 {
        p.send(&cp::absorbed(absorbed));
    }
    let v = &mut p.occupant.vitals;
    let shown = [hp, damage[1].saturating_add(sp_paid), damage[2].saturating_add(mp_paid)];
    let mut changed = false;
    let died = hp > 0 && v.health[0] <= hp;
    if died {
        v.health = [0; 3];
        p.dead = true;
        p.attacking = false;
    } else {
        v.health[0] -= hp;
        let (hp, max) = (v.health[0], v.max[0]);
        if let Some(a) = p.occupant.buffs.low_hp_check(hp, max, now, &mut || zones.new_key()) {
            p.send(&sk::applied(a.key, a.def.id, a.def.level));
            changed = true;
        }
    }
    if changed {
        let _ = p.occupant.events.send(ZoneEvent::Buffs);
    }
    let death = died.then(|| super::pvp::player_died(zones, pass.map, mobs, players, victim, by, &effects, skip, false, now));
    (shown, death)
}

/// A reflected hit (`CMob::Damage` with the player as attacker, `CMob::Death` when it kills): the monster takes it, the player gets
/// its kill packets, `0x051B` within 60 of the monster.
pub(super) fn reflect_onto<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    mob: u32,
    player: u32,
    damage: [u16; 3],
) {
    let Some(at) = mobs.mobs.get(&mob).map(|m| m.position) else { return };
    let hit = Hit { attacker: player, damage: damage.map(u32::from), aggro: 0, direct: false };
    let mut mine = Vec::new();
    pass.zones.damage_mob(pass.map, players, mobs, mob, &hit, pass.defs, pass.loot, pass.now, pass.rand, &mut mine);
    if let Some(p) = players.get(&player) {
        for packet in mine {
            p.send(&packet);
        }
    }
    send_around(Some(players), at, ATTACK_RANGE, &sk::reflect(player, mob, damage));
}

/// The cast announcement (`CMob::SendUseSkill`): `0x060B` result 4 naming the player (or nobody).
fn announce(players: &HashMap<u32, Placed>, at: [f32; 3], mob: u32, named: u32, entry: u8, s: &SkillDef) {
    send_hit(players, at, named, &sk::mob_skill(sk::RESULT_ANNOUNCE, mob, named, entry, s.id, s.level, [0; 3]));
}

/// The players an area skill reaches (section 3): in the 5 x 5 cells around `cell`, alive, of another country than the monster,
/// within `range` of `from`; in id order.
fn victims(players: &HashMap<u32, Placed>, def: &MobDef, cell: (i32, i32), from: [f32; 3], range: u8) -> Vec<u32> {
    let mut ids: Vec<u32> = players
        .values()
        .filter(|p| in_view(p.cell, cell) && targetable(p, def) && distance(p.occupant.position, from) <= f32::from(range))
        .map(|p| p.occupant.id)
        .collect();
    ids.sort_unstable();
    ids
}

/// A skill entry's turn (`CMob::AttackAI`, section 2): a heal at once; another skill after a chase into its reach (a monster that
/// does not walk drops a target out of reach). The entry's ready time is the skill's reuse time from now.
#[allow(clippy::too_many_arguments)]
pub(super) fn skill_turn<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    target: u32,
    def: &MobDef,
    entry: usize,
    s: &SkillDef,
    terrain: &MapTerrain,
    world: &World,
) {
    let now = pass.now;
    let ready = now + Duration::from_secs(u64::from(s.reset_time));
    let set_ready = |mobs: &mut MobMap| {
        if let Some(m) = mobs.mobs.get_mut(&id) {
            m.entry_ready[entry] = Some(ready);
        }
    };
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let Some(at) = players.get(&target).map(|p| p.occupant.position) else { return };
    if s.type_effect == skills::HEAL {
        use_skill(pass, mobs, players, id, target, def, entry, s);
        return set_ready(mobs);
    }
    if !fight::walks(def) {
        if fight::stationary_reach(mob.position, at, def, s.attack_range) {
            use_skill(pass, mobs, players, id, target, def, entry, s);
        } else {
            super::scripted::lose_target(mobs, id);
        }
        return set_ready(mobs);
    }
    let move_speed = mobs.speeds(id).1;
    let path = mobs.paths.entry(id).or_default();
    let chase =
        fight::chase(mob.position, mob.home, at, def, s.attack_range, move_speed, mob.next_chase, now, terrain, path, def.chase_time);
    if !matches!(chase, Chase::Wait | Chase::Step { .. }) {
        stop_moving(mobs, id);
    }
    match chase {
        Chase::InRange => {
            use_skill(pass, mobs, players, id, target, def, entry, s);
            set_ready(mobs);
        }
        Chase::Wait => {}
        Chase::GoHome => {
            mobs.mobs.get_mut(&id).expect("listed").state = MobState::Home;
            set_ready(mobs);
        }
        Chase::Drop => {
            super::scripted::become_idle(mobs, id);
            set_ready(mobs);
        }
        Chase::Unfair => {
            let m = mobs.mobs.get_mut(&id).expect("listed");
            m.fair = false;
            m.state = MobState::Home;
            set_ready(mobs);
        }
        Chase::Step { to, next, trail } => {
            fight::add_trail(mobs.trails.entry(id).or_default(), trail, terrain);
            let to = to.filter(|to| world.is_safe_pos(terrain.map, *to));
            let moving = Moving::Chase { target, range: s.attack_range, wait: def.chase_time };
            let next = paced_next(mobs, id, def, to, 1, next, now, pass.zones.smooth_chase(), moving);
            mobs.mobs.get_mut(&id).expect("listed").next_chase = next;
            if let Some(to) = to {
                walk_mob(mobs, Some(players), id, to, 1);
            }
        }
    }
}

/// `CMob::UseSkill` (section 3): nothing with the skill's attack kind disabled; an attack, a debuff or a heal; then the next turn
/// at `now + speed + 2500`.
#[allow(clippy::too_many_arguments)]
pub(super) fn use_skill<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    target: u32,
    def: &MobDef,
    entry: usize,
    s: &SkillDef,
) {
    let effects = mobs.effects(id);
    if effects.disabled.get(usize::from(s.attack_kind)).is_some_and(|&d| d != 0) {
        return;
    }
    let entry = entry as u8;
    match s.type_effect {
        skills::ATTACK => attack(pass, mobs, players, id, target, def, entry, s),
        skills::DEBUFF => debuff(pass, mobs, players, id, target, def, entry, s),
        skills::HEAL => heal(pass, mobs, players, id, def, entry, s),
        _ => {}
    }
    if let Some(m) = mobs.mobs.get_mut(&id) {
        m.next_attack = fight::idle_wait(pass.now, effects.speeds().0);
    }
}

/// Whether the monster is still alive (a reflection can kill it in the middle of a skill).
fn alive(mobs: &MobMap, id: u32) -> bool {
    mobs.mobs.get(&id).is_some_and(|m| m.dead_until.is_none() && m.hp > 0)
}

/// `CMob::SkillAttack` (section 3) by target type and `wType`.
#[allow(clippy::too_many_arguments)]
fn attack<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    target: u32,
    def: &MobDef,
    entry: u8,
    s: &SkillDef,
) {
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let multi = |pass: &mut Pass<'_, R>, mobs: &mut MobMap, players: &mut HashMap<u32, Placed>, victim: u32| {
        for _ in 0..s.multi_attack {
            hit_player(pass, mobs, players, id, victim, def, s, entry, false);
            if players.get(&victim).is_none_or(|p| p.dead) || !alive(mobs, id) {
                break;
            }
        }
    };
    match s.target_type {
        3 => match s.kind {
            21 | 24..=27 => hit_player(pass, mobs, players, id, target, def, s, entry, true),
            22 => {
                announce(players, mob.position, id, target, entry, s);
                multi(pass, mobs, players, target);
            }
            23 => {
                hit_player(pass, mobs, players, id, target, def, s, entry, true);
                self_destruct(pass, mobs, players, id);
            }
            _ => {}
        },
        6 | 7 => {
            let (named, cell, from) = match s.target_type {
                7 => match players.get(&target) {
                    Some(p) => (target, p.cell, p.occupant.position),
                    None => return,
                },
                _ => (0, mob.cell, mob.position),
            };
            announce(players, mob.position, id, named, entry, s);
            for victim in victims(players, def, cell, from, s.apply_range) {
                if !alive(mobs, id) {
                    break;
                }
                match s.kind {
                    21 | 23..=26 => hit_player(pass, mobs, players, id, victim, def, s, entry, false),
                    22 => multi(pass, mobs, players, victim),
                    _ => {}
                }
            }
            if s.kind == 23 {
                self_destruct(pass, mobs, players, id);
            }
        }
        _ => {}
    }
}

/// `CMob::SkillDebuff` (section 3): one hit on the target, or one on each player around the monster (6) or around the target with
/// the cells taken around the monster (7, the original's mix).
#[allow(clippy::too_many_arguments)]
fn debuff<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    target: u32,
    def: &MobDef,
    entry: u8,
    s: &SkillDef,
) {
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let (named, from) = match s.target_type {
        3 => return hit_player(pass, mobs, players, id, target, def, s, entry, true),
        6 => (0, mob.position),
        7 => match players.get(&target) {
            Some(p) => (target, p.occupant.position),
            None => return,
        },
        _ => return,
    };
    announce(players, mob.position, id, named, entry, s);
    for victim in victims(players, def, mob.cell, from, s.apply_range) {
        if !alive(mobs, id) {
            break;
        }
        hit_player(pass, mobs, players, id, victim, def, s, entry, false);
    }
}

/// `CMob::SkillHeal` (section 6): the monster itself (target type 2), or every monster of its side around it, itself included.
#[allow(clippy::too_many_arguments)]
fn heal<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    def: &MobDef,
    entry: u8,
    s: &SkillDef,
) {
    let Some(caster) = mobs.mobs.get(&id).copied() else { return };
    if s.target_type == 2 {
        return heal_mob(pass, mobs, players, id, def, id, entry, s, true);
    }
    announce(players, caster.position, id, 0, entry, s);
    let mut ids: Vec<u32> = mobs
        .mobs
        .values()
        .filter(|m| in_view(m.cell, caster.cell) && m.dead_until.is_none() && m.hp > 0)
        .filter(|m| distance(m.position, caster.position) <= f32::from(s.apply_range))
        .filter(|m| pass.defs.get(m.mob_id).is_some_and(|d| def.country == 2 || d.country == def.country))
        .map(|m| m.id)
        .collect();
    ids.sort_unstable();
    for healed in ids {
        heal_mob(pass, mobs, players, id, def, healed, entry, s, false);
    }
}

/// `CMob::HealUser` (section 6): `wType` 41 heals `wHeal` (HP plus the caster's WIS) up to the definition's maxima, a full HP clears
/// the damage record; 42 takes the debuffs off. `0x060B` (itself) or `0x060D` (another), then the healed monster's HP as `0x0305`
/// within 60 (deviation, so health bars stay right).
#[allow(clippy::too_many_arguments)]
fn heal_mob<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    caster: u32,
    caster_def: &MobDef,
    healed: u32,
    entry: u8,
    s: &SkillDef,
    single: bool,
) {
    let Some(m) = mobs.mobs.get(&healed).copied() else { return };
    let Some(def) = pass.defs.get(m.mob_id) else { return };
    let mut amounts = [0u16; 3];
    match s.kind {
        skills::HEAL_HEALTH => {
            let hp = if s.heal[0] != 0 { u32::from(s.heal[0]) + u32::from(caster_def.wis) } else { 0 };
            let hp = hp.min(def.hp.saturating_sub(m.hp));
            let sp = s.heal[1].min(def.sp.saturating_sub(m.sp));
            let mp = s.heal[2].min(def.mp.saturating_sub(m.mp));
            let mob = mobs.mobs.get_mut(&healed).expect("listed");
            mob.hp += hp;
            mob.sp += sp;
            mob.mp += mp;
            pass.zones.mob_record(pass.map, mob, hp, crate::game_log::MobAction::Heal, None, None);
            if hp > 0 && mob.hp == def.hp {
                mobs.damage.remove(&healed);
            }
            amounts = [hp.min(u32::from(u16::MAX)) as u16, sp, mp];
        }
        skills::DISPEL => {
            let before = mobs.speeds(healed);
            mobs.debuffs.remove(&healed);
            let after = mobs.speeds(healed);
            if before != after {
                send_around(Some(players), m.position, SPEED_RANGE, &cp::mob_speed(healed, after.0, after.1));
            }
        }
        _ => return,
    }
    if single {
        let at = mobs.mobs.get(&caster).map_or(m.position, |c| c.position);
        send_around(Some(players), at, ATTACK_RANGE, &sk::mob_skill(combat::RESULT_HIT, caster, 0, entry, s.id, s.level, amounts));
    } else {
        send_around(Some(players), m.position, ATTACK_RANGE, &sk::mob_area(combat::RESULT_HIT, healed, 0, s.id, s.level, amounts));
    }
    if let Some(hp) = mobs.mobs.get(&healed).map(|m| m.hp) {
        send_around(Some(players), m.position, ATTACK_RANGE, &w::mob_hp(healed, hp));
    }
}

/// Self-destruct (`wType` 23, section 4): the monster records its remaining HP as damage it did to itself and dies; the record's top
/// damage dealer gets the kill, nobody when that is the monster itself.
fn self_destruct<R: FnMut() -> u32>(pass: &mut Pass<'_, R>, mobs: &mut MobMap, players: &HashMap<u32, Placed>, id: u32) {
    let Some(m) = mobs.mobs.get(&id).copied().filter(|_| alive(mobs, id)) else { return };
    let hit = Hit { attacker: SELF_DESTRUCT, damage: [m.hp, 0, 0], aggro: 0, direct: false };
    let mut nobody = Vec::new();
    pass.zones.damage_mob(pass.map, players, mobs, id, &hit, pass.defs, pass.loot, pass.now, pass.rand, &mut nobody);
}

/// One skill hit on a player (`CMob::AttackUser`, section 5). `single` picks `0x060B` (with the attack entry) or `0x060D`.
#[allow(clippy::too_many_arguments)]
fn hit_player<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    victim: u32,
    def: &MobDef,
    s: &SkillDef,
    entry: u8,
    single: bool,
) {
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let at = mob.position;
    let fighter = combat::monster_with(def, &mobs.effects(id));
    let kind = usize::from(s.attack_kind).min(combat::MAGIC);
    let packet = |result: u8, values: [u16; 3]| {
        if single {
            sk::mob_skill(result, id, victim, entry, s.id, s.level, values)
        } else {
            sk::mob_area(result, id, victim, s.id, s.level, values)
        }
    };
    let Some(p) = players.get_mut(&victim) else { return };
    if p.occupant.shape.motion == w::MOTION_SIT {
        p.occupant.shape.motion = w::MOTION_STAND;
        let _ = p.occupant.events.send(ZoneEvent::StoodUp);
    }
    let block = p.occupant.buffs.block(s.attack_kind, pass.rand);
    if let Some(used_up) = block.used_up {
        p.send(&sk::applied_end(used_up.key));
        let _ = p.occupant.events.send(ZoneEvent::Buffs);
    }
    let guard = p.occupant.vitals.guard;
    // A cure item's protection against state 7 or 8 makes it miss, used up (`docs/original/item-use.md` section 5).
    let protected = !block.blocked && p.occupant.buffs.protected(s.state_type);
    // Then a lapis that blocks the state (`m_pStateBlock`).
    let protected = protected || (!block.blocked && s.state_type < 16 && p.occupant.vitals.state_blocks & (1 << s.state_type) != 0);
    if block.blocked || protected || !combat::mob_skill_hits(&fighter, def, &guard, s, pass.rand) {
        send_hit(players, at, victim, &packet(combat::RESULT_MISS, [0; 3]));
        return;
    }
    struck(players, victim);
    super::duel::hit(pass.zones, pass.map, players, victim, None, pass.now);
    let p = &players[&victim];
    let reflected =
        p.occupant.buffs.reflects(kind, guard.luc, guard.wis, pass.rand).is_some_and(|skill| !def.resist_skills.contains(&skill));
    let v = p.occupant.vitals;
    let numbers = combat::MobSkillHit {
        mob: &fighter,
        entry_damage: def.attacks[0].damage,
        entry_damage_add: def.attacks[0].damage_add,
        mob_health: [mob.hp, u32::from(mob.sp), u32::from(mob.mp)],
        player: &guard,
        defence_element: v.guard_element,
        health: [v.health[0], v.health[2], v.health[1]],
        max: [v.max[0], v.max[2], v.max[1]],
    };
    let blow = combat::mob_skill_damage(&numbers, s, pass.rand);
    let (shown, mut died) = if reflected { ([0; 3], false) } else { hurt_player(pass, mobs, players, id, victim, blow.damage) };
    send_hit(players, at, victim, &packet(blow.result, shown));
    // The additional damage, also after a reflection.
    if s.add_damage != [0; 3] && !died {
        let (added, dead) = hurt_player(pass, mobs, players, id, victim, s.add_damage);
        died = dead;
        send_hit(players, at, victim, &sk::mob_add(id, victim, s.id, s.level, added));
    }
    if died {
        return;
    }
    if reflected {
        reflect_onto(pass, mobs, players, id, victim, blow.damage);
    }
    if s.type_effect == skills::DEBUFF {
        debuff_player(pass, players, victim, id, s);
    }
    // The player's armour lapis back at the monster (`docs/original/item-options.md` section 5).
    item_options(pass, mobs, players, victim, SkillTarget::Mob(id), Worn::Armour, false, true);
}

/// A monster's debuff on a player (`CUser::AddApplySkill(skill, 2, monster)`, part 3 section 3): what it pushed off ends (`0x050E`),
/// `0x050D`, and the shape and numbers follow.
fn debuff_player<R: FnMut() -> u32>(pass: &mut Pass<'_, R>, players: &mut HashMap<u32, Placed>, victim: u32, mob: u32, s: &SkillDef) {
    if buffs::refused_on_map(pass.map, s) {
        return;
    }
    let (zones, now) = (pass.zones, pass.now);
    let Some(p) = players.get_mut(&victim) else { return };
    let applying = p.occupant.buffs.apply(s, mob, now, &mut || zones.new_key());
    for a in &applying.ended {
        p.send(&sk::applied_end(a.key));
    }
    if let Some(key) = applying.set {
        p.occupant.buffs.mark_attacker(key, false);
        p.send(&sk::applied(key, s.id, s.level));
    }
    let _ = p.occupant.events.send(ZoneEvent::Buffs);
    reform(players, victim, false);
}

/// A damage-over-time payment or the death sentence on a player (`CUser::DamageByKeepSkill`, section 8): when the entry has an
/// attacker and the player reflects the skill's attack kind, nothing is taken unless the monster resists the reflecting skill;
/// otherwise as a hit (death by the entry's attacker). `0x050F` within 60. A player's debuff reflected goes back to that player
/// when it is in the zone, with `0x0515` (`docs/original/pvp-2.md` section 7). `attacker`: `cp::KILLER_PLAYER` or `cp::KILLER_MOB`
/// and the id, 0 for none. Returns whether the player was hit and lives.
pub(super) fn keep_damage<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    victim: u32,
    def: &SkillDef,
    attacker: (u8, u32),
    damage: [u16; 3],
) -> bool {
    let Some(p) = players.get(&victim) else { return false };
    let guard = p.occupant.vitals.guard;
    let kind = usize::from(def.attack_kind).min(combat::MAGIC);
    let reflected = if attacker.1 != 0 { p.occupant.buffs.reflects(kind, guard.luc, guard.wis, pass.rand) } else { None };
    let at = p.occupant.position;
    if attacker.0 == cp::KILLER_PLAYER && reflected.is_some() {
        let cell = p.cell;
        let nothing = sk::keep(sk::OP_KEEP_USER, victim, def.id, def.level, [0; 3]);
        players.values().filter(|o| in_view(o.cell, cell)).for_each(|o| o.send(&nothing));
        if players.contains_key(&attacker.1) {
            let (back, _) = hurt_player_by(pass, mobs, players, (cp::KILLER_PLAYER, victim), attacker.1, damage);
            let mirror = cp::mirror_user(victim, attacker.1, back);
            players.values().filter(|o| in_view(o.cell, cell)).for_each(|o| o.send(&mirror));
        }
        return false;
    }
    let damage = match reflected {
        Some(skill)
            if !mobs.mobs.get(&attacker.1).and_then(|m| pass.defs.get(m.mob_id)).is_some_and(|d| d.resist_skills.contains(&skill)) =>
        {
            [0; 3]
        }
        _ => damage,
    };
    let by = if attacker.0 == cp::KILLER_PLAYER { attacker } else { (cp::KILLER_MOB, attacker.1) };
    let (shown, died) = hurt_player_by(pass, mobs, players, by, victim, damage);
    send_around(Some(players), at, RECOVER_RANGE, &sk::keep(sk::OP_KEEP_USER, victim, def.id, def.level, shown));
    damage != [0; 3] && !died
}

/// What a debuff ending by its time takes from a player (section 8): the rest of a damage over time, or all its HP for the death
/// sentence. `health` is HP, MP, SP; the damage HP, SP, MP.
pub(super) fn end_damage(a: &Applied, health: [u16; 3], max: [u16; 3], now: Instant) -> Option<[u16; 3]> {
    if a.def.type_effect != skills::DEBUFF {
        return None;
    }
    match a.def.kind {
        buffs::DAMAGE_OVER_TIME => Some(over_time(&a.def, health, max, now.saturating_duration_since(a.started))),
        buffs::DEATH_SENTENCE => Some([health[0], 0, 0]),
        _ => None,
    }
}

/// `CUser::TimeDamage(elapsed)` with the monster version's multiplication (deviation: the original adds `elapsed / 3000`).
pub(super) fn over_time(def: &SkillDef, health: [u16; 3], max: [u16; 3], elapsed: Duration) -> [u16; 3] {
    let order = |v: [u16; 3]| [u32::from(v[0]), u32::from(v[2]), u32::from(v[1])];
    buffs::time_damage(def, order(health), order(max), elapsed)
}
