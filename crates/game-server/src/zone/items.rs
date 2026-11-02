//! Using items in the zone (`docs/original/item-use.md`): what needs the maps and the parties (HP, SP and MP added, cures and
//! protections, the buff items' refusals, the target resurrection rune, where party members are, the summons waiting for an answer).
//! The decisions are `crate::item_use`'s.

use super::*;
use crate::item_use::{self, Cure};
use crate::sync::Relock;

/// A party member as a move by item needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub id: u32,
    pub map: u16,
    pub position: [f32; 3],
    pub level: u16,
    pub dead: bool,
    pub guild_id: u32,
}

impl Zones {
    /// HP, SP and MP added, each capped at its maximum (`CUser::SendRecoverAdd`, section 6): when something changed, `0x0505` to the
    /// players within 60; returns it for the player. `amounts` are HP, SP, MP.
    pub fn recover_add(&self, map: u16, id: u32, amounts: [u16; 3]) -> Option<Vec<u8>> {
        self.recover_with(map, id, |_| amounts)
    }

    /// The same with percentages of the maxima (special 17).
    pub fn recover_percent(&self, map: u16, id: u32, percent: [u16; 3]) -> Option<Vec<u8>> {
        // Vitals are HP, MP, SP.
        self.recover_with(map, id, |max| {
            [item_use::percent_of(percent[0], max[0]), item_use::percent_of(percent[1], max[2]), item_use::percent_of(percent[2], max[1])]
        })
    }

    fn recover_with(&self, map: u16, id: u32, amounts: impl FnOnce([u16; 3]) -> [u16; 3]) -> Option<Vec<u8>> {
        let mut maps = self.maps.guard();
        let players = maps.get_mut(&map)?;
        let me = players.get_mut(&id)?;
        let amounts = amounts(me.occupant.vitals.max);
        let [hp, sp, mp] = heal_player(me, amounts);
        if [hp, sp, mp] == [0; 3] {
            return None;
        }
        let at = me.occupant.position;
        let packet = cp::recover(id, [hp, mp, sp].map(|v| v.min(i16::MAX as u16) as i16));
        for other in players.values().filter(|p| p.occupant.id != id && distance(p.occupant.position, at) <= RECOVER_RANGE) {
            other.send(&packet);
        }
        Some(packet)
    }

    /// A cure item (`CUser::ItemUseRemState`, section 5): the applied skills it removes go (`0x050E` each, returned for the player),
    /// the shape and numbers follow, and its protections are set.
    pub fn cure(&self, map: u16, id: u32, cure: &Cure) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(me) = players.get_mut(&id) else { return Vec::new() };
        let buffs = &mut me.occupant.buffs;
        buffs.block_dying |= cure.block_dying;
        buffs.block_death |= cure.block_death;
        let gone = buffs.cure(cure.states, cure.all_debuffs);
        if gone.is_empty() {
            return Vec::new();
        }
        let _ = me.occupant.events.send(ZoneEvent::Buffs);
        reform(players, id, false);
        gone.iter().map(|a| sk::applied_end(a.key)).collect()
    }

    /// A stat or skill reset clears the cure items' protections (`CUser::SkillClearAll`).
    pub fn clear_protections(&self, map: u16, id: u32) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.occupant.buffs.block_dying = false;
            p.occupant.buffs.block_death = false;
        }
    }

    /// Whether a buff item's skill is refused for the player (`CUser::UseItemSkill`, section 7).
    pub fn item_skill_refused(&self, map: u16, id: u32, def: &SkillDef) -> bool {
        let maps = self.maps.guard();
        maps.get(&map).and_then(|players| players.get(&id)).is_none_or(|p| p.occupant.buffs.item_skill_refused(def))
    }

    /// The Target Resurrection Rune (section 10): another player in the zone, dead, of the user's country, comes back to life where
    /// the user stands (as the Resurrection skill, `ZoneEvent::Resurrected`). False when it cannot.
    pub fn resurrect_by_item(&self, map: u16, user: u32, target: u32) -> bool {
        let maps = self.maps.guard();
        let Some(players) = maps.get(&map) else { return false };
        let (Some(me), Some(them)) = (players.get(&user), players.get(&target)) else { return false };
        if user == target || !them.dead || them.occupant.shape.country != me.occupant.shape.country {
            return false;
        }
        them.occupant.events.send(ZoneEvent::Resurrected { position: me.occupant.position, caster: user, skill: 0 }).is_ok()
    }

    /// The members of the player's party, wherever they are in the world; `None` without a party.
    pub fn party_members_placed(&self, id: u32) -> Option<Vec<Member>> {
        let maps = self.maps.guard();
        let members = self.parties.guard().party_of(id)?.members.clone();
        let placed = members
            .into_iter()
            .filter_map(|m| {
                maps.iter().find_map(|(&map, players)| {
                    players.get(&m).map(|p| Member {
                        id: m,
                        map,
                        position: p.occupant.position,
                        level: p.occupant.vitals.guard.level,
                        dead: p.dead,
                        guild_id: p.occupant.guild_id,
                    })
                })
            })
            .collect();
        Some(placed)
    }

    /// A player anywhere in the world (`CWorld::FindUserZoneMove`).
    pub fn locate(&self, id: u32) -> Option<Member> {
        let maps = self.maps.guard();
        maps.iter().find_map(|(&map, players)| {
            players.get(&id).map(|p| Member {
                id,
                map,
                position: p.occupant.position,
                level: p.occupant.vitals.guard.level,
                dead: p.dead,
                guild_id: p.occupant.guild_id,
            })
        })
    }

    /// A summon asks `member` (`m_dwPartyCallReqID`, `m_dwTimeOverPartyCall`): `0x0223` and 35 s to answer. False when a summon
    /// is already waiting for it or it has left.
    pub fn ask_party_call(&self, member: &Member, summoner: u32, now: Instant) -> bool {
        let maps = self.maps.guard();
        let Some(p) = maps.get(&member.map).and_then(|players| players.get(&member.id)) else { return false };
        let mut calls = self.party_calls.guard();
        if calls.contains_key(&member.id) {
            return false;
        }
        let until = now + item_use::PARTY_CALL_WAIT;
        calls.insert(member.id, (summoner, until));
        p.send(&shaiya_proto::item::party_call(summoner));
        let _ = p.occupant.events.send(ZoneEvent::PartyCalled { until });
        true
    }

    /// The summon waiting for `member`, taken: its summoner. `expired_by` takes it only when it has run out by then.
    pub fn take_party_call(&self, member: u32, expired_by: Option<Instant>) -> Option<u32> {
        let mut calls = self.party_calls.guard();
        let &(summoner, until) = calls.get(&member)?;
        if expired_by.is_some_and(|now| now < until) {
            return None;
        }
        calls.remove(&member);
        Some(summoner)
    }

    /// Leaving the world: a summon waiting for the player is dropped.
    pub fn forget_party_call(&self, member: u32) {
        self.party_calls.guard().remove(&member);
    }
}
