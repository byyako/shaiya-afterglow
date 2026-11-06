//! Players attacking players (`docs/original/pvp.md`): who is an enemy, normal blows on players, the damage record and the killer
//! of record at a death, and where a victim's items and gold go. Part of the zone: the callers hold the locks.

use super::*;
use crate::sync::Relock;

/// An entry of a player's damage record (`CDamage`): who hurt it (`cp::KILLER_PLAYER` or `cp::KILLER_MOB` and the id) and the HP
/// it took, as the blows came in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Harm {
    pub kind: u8,
    pub id: u32,
    pub total: u32,
}

/// Adds a blow's HP damage to the record (`CDamage::Add`): to the attacker's entry, or a new one at the end.
pub(super) fn record(harm: &mut Vec<Harm>, kind: u8, id: u32, damage: u16) {
    match harm.iter_mut().find(|h| h.kind == kind && h.id == id) {
        Some(h) => h.total = h.total.saturating_add(u32::from(damage)),
        None => harm.push(Harm { kind, id, total: u32::from(damage) }),
    }
}

/// The killer of record (`CDamage::GetMax`): the most damage, the earliest entry on a tie.
pub(super) fn top(harm: &[Harm]) -> Option<(u8, u32)> {
    let mut best: Option<&Harm> = None;
    for h in harm {
        if best.is_none_or(|b| h.total > b.total) {
            best = Some(h);
        }
    }
    best.map(|h| (h.kind, h.id))
}

/// `CUser::IsTeam` without duels and guild wars (section 1): the same player, a map with a war type, or the same country.
pub fn same_side(a: (u32, u8), b: (u32, u8), war_map: bool) -> bool {
    a.0 == b.0 || war_map || a.1 == b.1
}

/// Why two players are enemies (`CUser::IsTeam` false), for the rules (`docs/plans/ext-2-rules.md` section 1: duels and guild wars are
/// agreed to, so the rules do not rule them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Enmity {
    No,
    /// A duel partner while fighting, or the other side of a guild war.
    Agreed,
    /// Only the country rule (`same_side`) makes them enemies.
    Faction,
}

/// What may be done to a player: yes, no (the core says no), or no because a rule refuses (`docs/plans/ext-2-rules.md` section 5.3).
#[derive(Clone, Debug)]
pub enum Verdict {
    Yes,
    No,
    Ruled(crate::rules::Refusal),
}

impl Verdict {
    pub fn is_yes(&self) -> bool {
        matches!(self, Verdict::Yes)
    }
}

/// What the rules test of a player (`crate::rules::Who`).
pub(super) fn who(p: &Placed) -> crate::rules::Who {
    crate::rules::Who { level: p.occupant.vitals.guard.level, kills: p.occupant.shape.kills, country: p.occupant.shape.country }
}

/// Whether `me` may strike `target` as an enemy: the core's enmity first, then the rules for a faction enemy only (point 1).
pub(super) fn strikes(me: &Placed, target: &Placed, war_map: bool, rules: &crate::rules::RuleSet, map: u16) -> Verdict {
    match enmity(me, target, war_map) {
        Enmity::No => Verdict::No,
        Enmity::Agreed => Verdict::Yes,
        Enmity::Faction => rules.attack(map, &who(me), &who(target)).map_or(Verdict::Yes, Verdict::Ruled),
    }
}

/// Whether `me` may attack `target` (`0x0502` and the attack tick, sections 2 and 3): alive, not hidden (shape 13), not itself and
/// not of its side; a duel partner while fighting is always an enemy (`CUser::IsTeam`, `docs/original/duel.md` section 5), and so
/// are the two sides of a guild war (`docs/original/guild-war.md` section 5). Both are in the zone; everyone in it is fully in the
/// world. Enemies by faction only are asked of the rules (`docs/plans/ext-2-rules.md` point 1).
pub(super) fn attackable(me: &Placed, target: &Placed, war_map: bool, rules: &crate::rules::RuleSet, map: u16) -> Verdict {
    if area_victim(target) && target.form.shape != sk::SHAPE_HIDDEN { strikes(me, target, war_map, rules, map) } else { Verdict::No }
}

/// Whether `target` is `me`'s enemy and why (`CUser::IsTeam` false): not itself, and a duel partner while fighting, a guild war's
/// other side, or of the other side of the country rule (`same_side`).
pub(super) fn enmity(me: &Placed, target: &Placed, war_map: bool) -> Enmity {
    let (a, b) = ((me.occupant.id, me.occupant.shape.country), (target.occupant.id, target.occupant.shape.country));
    let war = || crate::guild_war::enemies((me.occupant.guild_id, me.war.as_ref()), (target.occupant.guild_id, target.war.as_ref()));
    if a.0 == b.0 {
        Enmity::No
    } else if super::duel::fighting(me, b.0) || war() {
        Enmity::Agreed
    } else if !same_side(a, b, war_map) {
        Enmity::Faction
    } else {
        Enmity::No
    }
}

/// Whether an area effect may hit `target` (`docs/original/thorns-fields-pvp.md` sections 1 and 2): alive, attackable and visible
/// (`+0x5775`, `+0x5774`). No shape test: a hidden player is hit, and revealed.
pub(super) fn area_victim(target: &Placed) -> bool {
    !target.dead && target.occupant.visible && target.occupant.pvp
}

/// What ends with a duel or a guild war for one player (`CUser::PvPModeEnd`, `PvPGModeEnd`): its debuffs go (`0x050E` each), its
/// damage record is cleared and it stops attacking (`ZoneEvent::DuelEnded`).
pub(super) fn mode_end(players: &mut HashMap<u32, Placed>, id: u32) {
    let Some(p) = players.get_mut(&id) else { return };
    p.harm.clear();
    let gone = p.occupant.buffs.cure(&[], true);
    for a in &gone {
        p.send(&sk::applied_end(a.key));
    }
    let _ = p.occupant.events.send(ZoneEvent::DuelEnded);
    if !gone.is_empty() {
        let _ = p.occupant.events.send(ZoneEvent::Buffs);
        reform(players, id, false);
    }
}

/// A player's death (`CUser::Death`, section 6) by `by` (the attacker of the killing blow): its killer of record decides who killed
/// it. The record goes; the body's looks, `0x0504` naming that killer to its view; a monster that struck the blow drops it. A player
/// killer of record in the zone gets the kill (`ZoneEvent::PlayerKill`), or each member of its party near it does, and the victim
/// is told what it loses to whom (`ZoneEvent::Died`), nothing with "Prevent Item Drop", and the experience percent it loses at rebirth
/// (`before`: its effects as they were before the death cleared the buffs). `skip`: a player in view who does not get `0x0504` here, as the caller puts it in order among its own
/// packets; the packet is returned.
///
/// Duels (`docs/original/duel.md` section 6.1): a death that is the partner's win (`duel::winner`; `flag` is `CUser::Death`'s, set by
/// the desperate counterattack of a duellist on its partner) hands the stakes over and counts the result instead of the player-kill
/// rules; with `flag` and a player of record none of those rules apply either. Either way nothing is lost at rebirth. Any other
/// death ends the victim's duel (`CUser::PvPChkLeaveByDeath`).
///
/// Guild wars (`docs/original/guild-war.md` section 6.2): a kill by an enemy in the war, or with `flag` by any player, is a defeat
/// without the player-kill rules (`PvPGetDeathType` 5) and nothing is lost at rebirth; every death of a player in a war takes it out
/// (`0x2506 3`), and a death of a request's officer cancels the request (deviation 3).
#[allow(clippy::too_many_arguments)]
pub(super) fn player_died(
    zones: &Zones,
    map: u16,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    victim: u32,
    by: (u8, u32),
    before: &crate::buffs::BuffEffects,
    skip: Option<u32>,
    flag: bool,
    now: Instant,
) -> Vec<u8> {
    let death = cp::death(victim, by.0, by.1);
    let Some(p) = players.get_mut(&victim) else { return death };
    // A guard's kill asks no record (`CUser::DeathByNPC`, `docs/original/guards.md` section 7).
    let recorded = if by.0 == cp::KILLER_NPC { None } else { top(&p.harm) };
    p.harm.clear();
    let level = p.occupant.vitals.guard.level;
    // `CMob::StatusAttackStop`: the monster drops its target; one with a machine looks for another at once and starts over only
    // without one (`docs/original/scripted-mobs.md` deviation 6).
    if by.0 == cp::KILLER_MOB {
        super::scripted::lose_target(mobs, by.1);
    }
    let (kind, killer) = recorded.unwrap_or(by);
    let mut loss = None;
    let mut counted = false;
    let mut pvp_raised = false;
    let winner = super::duel::winner(players, victim, (kind, killer), flag);
    let defeat = winner.is_none() && super::guild_war::defeat(players, victim, (kind, killer), flag);
    // `m_nDropExp` −1: a duel's death, a guild war's, or the flag with a player of record in the zone (the victim itself included).
    let flagged = flag && kind == cp::KILLER_PLAYER && (killer == victim || players.contains_key(&killer));
    let keep_exp = winner.is_some() || flagged || defeat;
    // Its buffs that end with death go, without 0x050E (`CUser::ClearApplySkillByDeath`, `docs/original/skills-buffs.md` section 16:
    // a death that costs no experience keeps the skills above id 200). The clients clear their own lists; only the looks go.
    if let Some(p) = players.get_mut(&victim)
        && !p.occupant.buffs.clear_on_death(keep_exp).is_empty()
    {
        let _ = p.occupant.events.send(ZoneEvent::Buffs);
    }
    reform(players, victim, true);
    if let Some(winner) = winner {
        super::duel::won(zones, map, players, winner, victim, now);
    } else if !flagged && !defeat && kind == cp::KILLER_PLAYER && killer != victim && players.contains_key(&killer) {
        loss = (!before.no_item_loss).then_some(killer);
        if recorded.is_some() {
            let last_hit = if by.0 == cp::KILLER_PLAYER { by.1 } else { 0 };
            let kill = |party| ZoneEvent::PlayerKill { victim, victim_level: level, last_hit, party };
            match zones.kill_share(players, killer) {
                None => {
                    let _ = players[&killer].occupant.events.send(kill(None));
                    counted = true;
                    zones.raise_pvp_kill(players, killer, victim, last_hit, &[killer]);
                }
                Some(share) => {
                    for m in share.near.iter().filter_map(|m| players.get(m)) {
                        let _ = m.occupant.events.send(kill(Some((share.size, share.union, share.max_level))));
                    }
                    counted = !share.near.is_empty();
                    if counted {
                        zones.raise_pvp_kill(players, killer, victim, last_hit, &share.near);
                    }
                }
            }
            pvp_raised = counted;
        }
    }
    // The scripts' `death`, after `pvp_kill` when there was one; queued here, run on the script thread (section 4.3).
    zones.raise_death(players, victim, kind, killer, pvp_raised);
    let p = &players[&victim];
    let death = cp::death(victim, kind, killer);
    for other in players.values().filter(|o| Some(o.occupant.id) != skip && in_view(o.cell, p.cell)) {
        other.send(&death);
    }
    let _ = p.occupant.events.send(ZoneEvent::Died {
        killer_kind: kind,
        killer,
        loss,
        counted,
        keep_exp,
        drop_exp: before.death_exp_percent(),
    });
    zones.end_duel(map, players, victim, super::duel::How::Interrupted, now);
    zones.war_died(players, victim);
    death
}

impl Zones {
    /// Whether `me` may attack the player `target` on `map` (`0x0502`, section 2), and when not whether a rule says so
    /// (`docs/plans/ext-2-rules.md` section 5.3): the refusal is for the whisper at the request.
    pub fn attack_verdict(&self, map: u16, me: u32, target: u32, war_map: bool) -> Verdict {
        let rules = self.rules();
        let maps = self.maps.guard();
        let Some(players) = maps.get(&map) else { return Verdict::No };
        let (Some(a), Some(t)) = (players.get(&me), players.get(&target)) else { return Verdict::No };
        if me == target { Verdict::No } else { attackable(a, t, war_map, &rules, map) }
    }

    /// Whether `me` may attack the player `target` on `map` (`0x0502`, section 2): `attack_verdict` is yes.
    pub fn may_attack_player(&self, map: u16, me: u32, target: u32, war_map: bool) -> bool {
        self.attack_verdict(map, me, target, war_map).is_yes()
    }

    /// One normal blow on a player (`CUser::AttackUser`, section 4): range, block, hit or miss, reflection, damage. The players
    /// within 60 of the attacker get `0x0502` (after the death packets of a kill) and `0x0515` when the blow came back; the
    /// attacker's copies go through its outbox too, behind a `0x0504` there, so `Struck` returns no packets.
    #[allow(clippy::too_many_arguments)]
    pub fn attack_player(
        &self,
        map: u16,
        target: u32,
        attack: &Attack,
        war_map: bool,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> AttackOutcome {
        // One snapshot of the rules for the blow (`docs/plans/ext-2-rules.md` section 8); a blow a rule now forbids is dropped silently.
        let rules = self.rules();
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let Some(players) = maps.get_mut(&map) else { return AttackOutcome::Gone };
        let mobs = all.entry(map).or_default();
        let me = attack.attacker;
        let (Some(a), Some(t)) = (players.get(&me), players.get(&target)) else { return AttackOutcome::Gone };
        if me == target || !attackable(a, t, war_map, &rules, map).is_yes() {
            return AttackOutcome::Gone;
        }
        let at = a.occupant.position;
        let gap = distance(at, t.occupant.position);
        if !combat::in_range(gap, attack.slack, 0, attack.range) {
            return AttackOutcome::OutOfRange;
        }
        let around = |players: &HashMap<u32, Placed>, packet: &[u8]| {
            for p in players.values().filter(|p| p.occupant.id == me || distance(p.occupant.position, at) <= ATTACK_RANGE) {
                p.send(packet);
            }
        };
        let t = players.get_mut(&target).expect("found above");
        // The target stands up (`CUser::StatusStand`).
        if t.occupant.shape.motion == w::MOTION_SIT {
            t.occupant.shape.motion = w::MOTION_STAND;
            let _ = t.occupant.events.send(ZoneEvent::StoodUp);
        }
        let block = t.occupant.buffs.block(attack.kind as u8, rand);
        if let Some(used_up) = block.used_up {
            t.send(&sk::applied_end(used_up.key));
            let _ = t.occupant.events.send(ZoneEvent::Buffs);
        }
        let (guard, defence_element) = (t.occupant.vitals.guard, t.occupant.vitals.guard_element);
        let close = gap <= CLOSE_RANGE;
        let blow = if block.blocked {
            combat::Blow { result: combat::RESULT_MISS, damage: 0 }
        } else {
            combat::pvp_strike(attack.fighter, &guard, attack.kind, close, attack.cross_road, defence_element, rand)
        };
        if blow.result == combat::RESULT_MISS {
            around(players, &cp::attack_user(blow.result, me, target, [0; 3]));
            return AttackOutcome::Struck { packets: Vec::new(), killed: false, critical: false };
        }
        // A hit breaks the target's cast, wakes it, ends its hiding and its ride (`CUser::StatusCastingStop`); a duel with someone
        // else ends (`CUser::PvPChkLeaveByAttack`).
        mob_skills::struck(players, target);
        super::duel::hit(self, map, players, target, Some(me), now);
        let t = &players[&target];
        let reflected = t.occupant.buffs.reflects(attack.kind, guard.luc, guard.wis, rand).is_some();
        let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
        let (shown, killed) = if reflected {
            ([0; 3], false)
        } else {
            mob_skills::hurt_player_by(&mut pass, mobs, players, (cp::KILLER_PLAYER, me), target, [blow.damage, 0, 0])
        };
        around(players, &cp::attack_user(blow.result, me, target, shown));
        if reflected {
            let (back, _) = mob_skills::hurt_player_by(&mut pass, mobs, players, (cp::KILLER_PLAYER, target), me, [blow.damage, 0, 0]);
            around(players, &cp::mirror_user(target, me, back));
        }
        // The attacker's weapon lapis on the target, then the target's armour lapis back (`docs/original/item-options.md` section 5).
        // Both through the queues, as the blow's own packets.
        let mine = item_options(&mut pass, mobs, players, me, SkillTarget::Player(target), Worn::Weapon, war_map, true);
        item_options(&mut pass, mobs, players, target, SkillTarget::Player(me), Worn::Armour, war_map, true);
        let critical = blow.result == combat::RESULT_CRITICAL || mine.criticals > 0;
        AttackOutcome::Struck { packets: Vec::new(), killed: killed || mine.killed, critical }
    }

    /// A player's kills changed (`CUser::KillPCTo`): `0x0522` to its view and in its looks from now on; its own copy is returned.
    pub fn set_kills(&self, map: u16, id: u32, kills: u32) -> Vec<u8> {
        let mut maps = self.maps.guard();
        let packet = cp::kill_info(id, kills);
        let Some(players) = maps.get_mut(&map) else { return packet };
        let Some(p) = players.get_mut(&id) else { return packet };
        p.occupant.shape.kills = kills;
        let cell = p.cell;
        for other in players.values().filter(|o| o.occupant.id != id && in_view(o.cell, cell)) {
            other.send(&packet);
        }
        packet
    }

    /// An item a victim lost (section 6.2): to the killer's party by its loot rule (rule 1: the first member near the killer from the
    /// turn gets it into its bags, `ZoneEvent::Loot`), otherwise on the ground at the victim's feet ± 0.5, the killer's, kind 1. The
    /// victim's own `0x0401` is returned.
    pub fn victim_item(
        &self,
        map: u16,
        victim: u32,
        killer: u32,
        loot: Loot,
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> Option<Vec<u8>> {
        let maps = self.maps.guard();
        let players = maps.get(&map)?;
        let position = drop_spot(players.get(&victim)?.occupant.position, rand);
        let mut party = 0;
        if let Some(share) = self.kill_share(players, killer) {
            party = share.party;
            let receiver = self.parties.guard().next_in_turn(share.party, |m| share.near.contains(&m));
            if let Some(r) = receiver.and_then(|r| players.get(&r)) {
                let _ = r.occupant.events.send(ZoneEvent::Loot { loot, position, owner: killer, party, dropped: Dropped::death(victim) });
                return None;
            }
        }
        let mut ground = self.ground.guard();
        self.place(
            ground.entry(map).or_default(),
            Some(players),
            loot,
            position,
            killer,
            party,
            now,
            gp::KIND_PLAYER_DROP,
            victim,
            Dropped::death(victim),
        )
    }

    /// Gold a victim lost (section 6.2): an equal part (at least 1) to each member of the killer's party near the killer
    /// (`ZoneEvent::Gold`), otherwise a gold item on the ground at the victim's feet ± 0.5, the killer's alone, kind 1. The victim's
    /// own `0x0401` is returned.
    pub fn victim_gold(
        &self,
        map: u16,
        victim: u32,
        killer: u32,
        amount: u32,
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> Option<Vec<u8>> {
        let maps = self.maps.guard();
        let players = maps.get(&map)?;
        let position = drop_spot(players.get(&victim)?.occupant.position, rand);
        if let Some(share) = self.kill_share(players, killer) {
            let part = crate::pvp::gold_part(amount, usize::from(share.size));
            for m in share.near.iter().filter_map(|m| players.get(m)) {
                let _ = m.occupant.events.send(ZoneEvent::Gold { amount: part, from_mob: false });
            }
            return None;
        }
        let mut ground = self.ground.guard();
        self.place(
            ground.entry(map).or_default(),
            Some(players),
            Loot::Gold(amount),
            position,
            killer,
            0,
            now,
            gp::KIND_PLAYER_DROP,
            victim,
            Dropped::death(victim),
        )
    }
}

/// `CUser::GetDropPosRand`: up to half a unit either way on x and z, y as it is.
pub(super) fn drop_spot(at: [f32; 3], rand: &mut impl FnMut() -> u32) -> [f32; 3] {
    let mut spread = || (rand() % 1000) as f32 / 1000.0 - 0.5;
    let (dx, dz) = (spread(), spread());
    [at[0] + dx, at[1], at[2] + dz]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_names_the_most_damage_first_on_a_tie() {
        let mut harm = Vec::new();
        assert_eq!(top(&harm), None);
        record(&mut harm, cp::KILLER_MOB, 9, 40);
        record(&mut harm, cp::KILLER_PLAYER, 2, 30);
        record(&mut harm, cp::KILLER_PLAYER, 2, 10);
        assert_eq!(top(&harm), Some((cp::KILLER_MOB, 9)), "40 each: the earlier entry");
        record(&mut harm, cp::KILLER_PLAYER, 2, 1);
        assert_eq!(top(&harm), Some((cp::KILLER_PLAYER, 2)));
        assert_eq!(harm.len(), 2);
    }

    #[test]
    fn sides_by_country_but_not_on_war_maps() {
        assert!(!same_side((1, 0), (2, 1), false));
        assert!(same_side((1, 0), (2, 0), false));
        assert!(same_side((1, 0), (2, 1), true));
        assert!(same_side((1, 0), (1, 1), false));
    }
}
