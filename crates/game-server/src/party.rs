//! Parties (`docs/original/party.md`) and unions (`docs/original/union.md`): who is in which party, its leader, the loot turn,
//! pending invitations, a union's slots, sub-leader and settings, and the experience shares. Bookkeeping only: the zone sends the
//! packets.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A normal party's size (`CParty::EnterNormalParty`).
pub const MAX_MEMBERS: usize = 7;
/// A union's size and slots (`CParty::EnterRaidParty`, `CParty::GetEmptySlot`).
pub const MAX_UNION_MEMBERS: usize = 30;
/// Slots to a group (`CParty::GetGroup`).
pub const GROUP_SIZE: u8 = 6;
/// Loot rules (`m_nItemDivType`, `docs/original/union.md` section 7): in turn, random, in turn on the ground, the leader.
pub const RULE_IN_TURN: u32 = 1;
pub const RULE_RANDOM: u32 = 2;
pub const RULE_IN_TURN_GROUND: u32 = 3;
pub const RULE_LEADER: u32 = 4;
/// How long an invitation waits for its answer (`m_dwTimeOverParty`).
pub const INVITE_TIME: Duration = Duration::from_secs(30);
/// Members this far from the killer share its kill: experience, loot and gold (`EnableApplyRange`, `EnableApplyRangeItem`).
pub const SHARE_RANGE: f32 = 100.0;
/// Members this far from the one who got an item hear of it (`0x0B08`, `CParty::Send` with a distance).
pub const LOOT_NOTICE_RANGE: f32 = 100.0;

/// Each member's share of a kill's experience by party size (0x5423a8, section 8).
const SHARE: [f64; MAX_MEMBERS + 1] = [1.0, 1.0, 1.0, 0.73, 0.645, 0.56, 0.475, 0.39];
/// The same by union size (`m_RaidPartyExp` 0x5423e8, `docs/original/union.md` section 8): one step behind the party's.
const UNION_SHARE: [f64; MAX_UNION_MEMBERS + 1] = [
    1.0, 1.0, 0.73, 0.645, 0.56, 0.475, 0.39, 0.305, 0.27, 0.251, 0.235, 0.219, 0.21, 0.17, 0.14, 0.1, 0.06, 0.03, 0.01, 0.01, 0.01, 0.01,
    0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01,
];

/// What a member levels below the party's highest level keeps (`CParty::GetPartyExpByLevel`).
pub fn gap(below: i32) -> f64 {
    match below {
        ..8 => 1.0,
        8..16 => 0.8,
        16..32 => 0.6,
        32..48 => 0.4,
        _ => 0.2,
    }
}

/// A member's experience from a party or union kill (`CMob::Death`): `base` is `CMob::GetExp` at the member's own level.
pub fn member_exp(base: u32, members: usize, union: bool, max_level: u16, level: u16) -> u32 {
    let table: &[f64] = if union { &UNION_SHARE } else { &SHARE };
    let share = table.get(members).copied().unwrap_or(1.0);
    let base = (f64::from(base) * share) as u32;
    (f64::from(base) * gap(i32::from(max_level) - i32::from(level))) as u32
}

/// A party's gold share (`CMob::ItemDrop`): the amount split over all members, at least 1.
pub fn gold_share(amount: u32, members: usize) -> u32 {
    (amount / members.max(1) as u32).max(1)
}

/// Whether a kill's gain counts twice: a normal party of 7 (`CUser::AddExpFromUser`; never a union).
pub fn full_party(members: usize, union: bool) -> bool {
    !union && members == MAX_MEMBERS
}

/// What the other members were last shown of a member (the party pass, section 13).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    /// HP, SP, MP and their maxima.
    pub health: [u16; 3],
    pub max: [u16; 3],
    /// Applied skills (id, level), those kept through death left out.
    pub buffs: Vec<(u16, u8)>,
    pub dead: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Party {
    pub id: u32,
    /// In joining order (the slots).
    pub members: Vec<u32>,
    /// The leader's slot (`m_nBothIndex`).
    pub leader: usize,
    /// Whose turn it is for loot (`m_nItemDivSeq`).
    pub turn: usize,
    /// The highest member level (`m_nMaxUserLevel`), kept by the zone.
    pub max_level: u16,
    pub shown: HashMap<u32, Shown>,
    /// A union's slots and settings; `None` for a normal party.
    pub union: Option<Union>,
}

/// What makes a party a union (`docs/original/union.md` section 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Union {
    /// Each member's slot (0 to 29; the group is slot / 6), in the members' order (`m_pParty[].Index`).
    pub slots: Vec<u8>,
    /// The sub-leader's index in the members (`m_nSubBothIndex`).
    pub sub: usize,
    pub rule: u32,
    pub auto_join: bool,
}

/// What a union's leader or sub-leader change came to (`SetRaidPartyBoth`, `LeaveRaidParty`).
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderChange {
    pub members: Vec<u32>,
    pub old: u32,
    pub new: u32,
    /// The new sub-leader, when the new leader was the sub-leader.
    pub sub: Option<u32>,
}

impl Party {
    pub fn leader_id(&self) -> u32 {
        self.members[self.leader]
    }

    pub fn is_full(&self) -> bool {
        self.members.len() >= if self.union.is_some() { MAX_UNION_MEMBERS } else { MAX_MEMBERS }
    }

    pub fn is_union(&self) -> bool {
        self.union.is_some()
    }

    /// The union's sub-leader.
    pub fn sub_leader_id(&self) -> Option<u32> {
        self.union.as_ref().map(|u| self.members[u.sub])
    }

    /// Whether the player leads the party or is the union's sub-leader (`IsPartyBoth`, `IsPartySubBoth`).
    pub fn leads(&self, id: u32) -> bool {
        self.leader_id() == id || self.sub_leader_id() == Some(id)
    }

    /// A union member's slot.
    pub fn slot_of(&self, id: u32) -> Option<u8> {
        let u = self.union.as_ref()?;
        Some(u.slots[self.members.iter().position(|&m| m == id)?])
    }

    /// A union member's group (`CParty::GetGroup`): `None` outside a union, for a non-member and for a slot of 30 or more.
    pub fn group_of(&self, id: u32) -> Option<u8> {
        self.slot_of(id).filter(|&s| usize::from(s) < MAX_UNION_MEMBERS).map(|s| s / GROUP_SIZE)
    }

    /// The members with their slots, in the members' order.
    pub fn slotted(&self) -> Vec<(u8, u32)> {
        let Some(u) = &self.union else { return Vec::new() };
        u.slots.iter().copied().zip(self.members.iter().copied()).collect()
    }

    /// The union's settings as `0x0B0B` and `0x0B0E` carry them.
    pub fn settings(&self) -> Option<shaiya_proto::party::UnionSettings> {
        let u = self.union.as_ref()?;
        Some(shaiya_proto::party::UnionSettings {
            leader_slot: u.slots[self.leader],
            sub_leader_slot: u.slots[u.sub],
            rule: u.rule,
            auto_join: u.auto_join,
        })
    }

    /// The first member that does not lead: a union's new sub-leader (`LeaveRaidParty`, `SetRaidPartyBoth`).
    fn first_not_leader(&self) -> usize {
        (0..self.members.len()).find(|&i| i != self.leader).unwrap_or(0)
    }

    /// The members other than `id`.
    pub fn others(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        self.members.iter().copied().filter(move |&m| m != id)
    }

    /// Takes the member at `slot` out; the loot turn, the leader and a union's sub-leader follow (`CParty::LeaveNormalParty`,
    /// `CParty::LeaveRaidParty`). A party's leader leaving hands over to slot 0; a union's to the sub-leader, and a new sub-leader is
    /// the first member that does not lead. Returns whether the leader and whether the sub-leader changed.
    fn remove_slot(&mut self, slot: usize) -> (bool, bool) {
        let gone = self.members.remove(slot);
        self.shown.remove(&gone);
        // As the original: the turn goes down only when it pointed past the slot after the one leaving.
        if slot + 1 < self.turn {
            self.turn -= 1;
        }
        let leader_left = slot == self.leader;
        if slot < self.leader {
            self.leader -= 1;
        }
        let Some(u) = &mut self.union else {
            if leader_left {
                self.leader = 0;
            }
            return (leader_left, false);
        };
        u.slots.remove(slot);
        let sub_left = slot == u.sub;
        if slot < u.sub {
            u.sub -= 1;
        }
        if self.members.len() < 2 {
            return (leader_left, sub_left);
        }
        if leader_left {
            self.leader = u.sub;
        }
        if leader_left || sub_left {
            let sub = self.first_not_leader();
            self.union.as_mut().expect("a union").sub = sub;
        }
        (leader_left, leader_left || sub_left)
    }
}

/// What leaving or being kicked did (`LeaveNormalParty`, `ExpelUser`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Departure {
    /// Every member before the departure, the one leaving included: they get `0x0B05` (or `0x0B06`) with its id.
    pub told: Vec<u32>,
    /// The new leader, when the leader left: the remaining members get `0x0B07`.
    pub new_leader: Option<u32>,
    /// Who is still in the party (none when it ended).
    pub remaining: Vec<u32>,
    /// The last member, who left too because it was alone (it gets `0x0B05` with its own id).
    pub last: Option<u32>,
    /// A union's new sub-leader: the remaining members get `0x0B13`.
    pub new_sub: Option<u32>,
    /// Whether it was a union (`0x0B0D`, `0x0B1A`, `0x0B17`).
    pub union: bool,
}

/// Every party of the server and the pending invitations.
#[derive(Debug, Default)]
pub struct Parties {
    parties: HashMap<u32, Party>,
    of: HashMap<u32, u32>,
    /// The invited player → its inviter and the deadline (`m_dwPartyReqID`, `m_dwTimeOverParty`).
    invites: HashMap<u32, (u32, Instant)>,
    last_id: u32,
}

impl Parties {
    pub fn party_of(&self, player: u32) -> Option<&Party> {
        self.parties.get(self.of.get(&player)?)
    }

    pub fn party_of_mut(&mut self, player: u32) -> Option<&mut Party> {
        self.parties.get_mut(self.of.get(&player)?)
    }

    pub fn get(&self, id: u32) -> Option<&Party> {
        self.parties.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Party> {
        self.parties.get_mut(&id)
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Party> {
        self.parties.values_mut()
    }

    /// `CUser::GetPartyType`: 0 no party, 1 member, 2 leader.
    pub fn party_type(&self, player: u32) -> u8 {
        match self.party_of(player) {
            None => shaiya_proto::party::TYPE_NONE,
            Some(p) if p.leader_id() == player => shaiya_proto::party::TYPE_LEADER,
            Some(_) => shaiya_proto::party::TYPE_MEMBER,
        }
    }

    /// Whether the player is in a party or has an invitation pending (section 2 step 5).
    pub fn engaged(&self, player: u32) -> bool {
        self.of.contains_key(&player) || self.invites.contains_key(&player)
    }

    pub fn invite(&mut self, inviter: u32, target: u32, now: Instant) {
        self.invites.insert(target, (inviter, now + INVITE_TIME));
    }

    /// The pending invitation of `target`, taken (section 3).
    pub fn take_invite(&mut self, target: u32) -> Option<u32> {
        self.invites.remove(&target).map(|(inviter, _)| inviter)
    }

    /// Drops invitations past their deadline, silently (`CUser::UpdateTimeover`).
    pub fn expire(&mut self, now: Instant) -> usize {
        let before = self.invites.len();
        self.invites.retain(|_, &mut (_, due)| due > now);
        before - self.invites.len()
    }

    /// Forgets the invitations a player sent or got (it left the world).
    pub fn forget_invites(&mut self, player: u32) {
        self.invites.retain(|&target, &mut (inviter, _)| target != player && inviter != player);
    }

    /// The inviter's party, created with the inviter in it when it has none (`CPartyMgr::CreateParty`: always a normal party);
    /// `None` when the inviter leads none (it is in a party but not its leader, nor, for a union's answer `0x0B15`, its sub-leader)
    /// or the party is full. Returns the party id and whether it is new.
    pub fn party_for(&mut self, inviter: u32, sub_leader_too: bool) -> Option<(u32, bool)> {
        if let Some(p) = self.party_of(inviter) {
            let leads = p.leader_id() == inviter || (sub_leader_too && p.sub_leader_id() == Some(inviter));
            return (leads && !p.is_full()).then_some((p.id, false));
        }
        self.last_id += 1;
        let id = self.last_id;
        let party = Party { id, members: vec![inviter], leader: 0, turn: 0, max_level: 0, shown: HashMap::new(), union: None };
        self.parties.insert(id, party);
        self.of.insert(inviter, id);
        Some((id, true))
    }

    /// Adds a player at the next slot (`CParty::EnterNormalParty`), or in a union with the lowest free slot
    /// (`CParty::EnterRaidParty`); false when the party is full or gone.
    pub fn join(&mut self, party: u32, player: u32) -> bool {
        let Some(p) = self.parties.get_mut(&party) else { return false };
        if p.is_full() || self.of.contains_key(&player) {
            return false;
        }
        if let Some(u) = &mut p.union {
            let Some(slot) = (0..MAX_UNION_MEMBERS as u8).find(|s| !u.slots.contains(s)) else { return false };
            u.slots.push(slot);
        }
        p.members.push(player);
        self.of.insert(player, party);
        true
    }

    /// `0x0B0E` (`docs/original/union.md` section 2): the requester leads a party that is not a union yet; the slots in joining
    /// order, the sub-leader as the original picks it (the first index that is neither the leader's nor 1, else 1). Refused for a
    /// `bRaidType` of 0, a rule not 1 to 4 and a union already (deviations 1, 2 and 8). Returns the members and the settings.
    pub fn make_union(
        &mut self,
        requester: u32,
        union: u8,
        auto_join: bool,
        rule: u32,
    ) -> Option<(Vec<u32>, shaiya_proto::party::UnionSettings)> {
        let p = self.party_of_mut(requester)?;
        if p.leader_id() != requester || p.union.is_some() || union == 0 || !(RULE_IN_TURN..=RULE_LEADER).contains(&rule) {
            return None;
        }
        let slots = (0..p.members.len() as u8).collect();
        let sub = (0..p.members.len()).find(|&i| i != p.leader && i != 1).unwrap_or(1);
        p.union = Some(Union { slots, sub, rule, auto_join });
        Some((p.members.clone(), p.settings().expect("a union")))
    }

    /// The union the requester leads, for its settings (`0x0B0F`, `0x0B10`, `0x0B16`, `0x0B17`, `0x0B13`, `0x0B1A`).
    fn led_union(&mut self, requester: u32) -> Option<&mut Party> {
        self.party_of_mut(requester).filter(|p| p.is_union() && p.leader_id() == requester)
    }

    /// `0x0B0F`: the loot rule, 1 to 4 only (deviation 2). Returns the members.
    pub fn set_union_rule(&mut self, requester: u32, rule: u32) -> Option<Vec<u32>> {
        let p = self.led_union(requester).filter(|_| (RULE_IN_TURN..=RULE_LEADER).contains(&rule))?;
        p.union.as_mut().expect("a union").rule = rule;
        Some(p.members.clone())
    }

    /// `0x0B10`: auto-join. Returns the members.
    pub fn set_auto_join(&mut self, requester: u32, on: bool) -> Option<Vec<u32>> {
        let p = self.led_union(requester)?;
        p.union.as_mut().expect("a union").auto_join = on;
        Some(p.members.clone())
    }

    /// `0x0B17` (`CParty::SetRaidPartyBoth`): another member becomes leader; when it was the sub-leader, the first member that does
    /// not lead becomes sub-leader.
    pub fn set_union_leader(&mut self, requester: u32, target: u32) -> Option<LeaderChange> {
        let p = self.led_union(requester)?;
        let index = p.members.iter().position(|&m| m == target).filter(|&i| i != p.leader)?;
        p.leader = index;
        let mut sub = None;
        if p.union.as_ref().is_some_and(|u| u.sub == index) {
            let first = p.first_not_leader();
            p.union.as_mut().expect("a union").sub = first;
            sub = Some(p.members[first]);
        }
        Some(LeaderChange { members: p.members.clone(), old: requester, new: target, sub })
    }

    /// `0x0B13` (`CParty::SetRaidPartySubBoth`): another member than the leader becomes sub-leader. Returns the members and the
    /// sub-leader, unchanged when the target is not such a member (the original answers then too).
    pub fn set_sub_leader(&mut self, requester: u32, target: u32) -> Option<(Vec<u32>, u32)> {
        let p = self.led_union(requester)?;
        if let Some(index) = p.members.iter().position(|&m| m == target).filter(|&i| i != p.leader) {
            p.union.as_mut().expect("a union").sub = index;
        }
        Some((p.members.clone(), p.sub_leader_id().expect("a union")))
    }

    /// `0x0B12` (`CParty::ChangeUser`) by the leader or the sub-leader: the members at the two slots swap, or the one there moves to
    /// the other slot. Slots 0 to 29 only, and `None` when nothing moved (deviation 3). Returns the members and the settings.
    pub fn move_in_union(&mut self, requester: u32, from: u32, to: u32) -> Option<(Vec<u32>, shaiya_proto::party::UnionSettings)> {
        let p = self.party_of_mut(requester).filter(|p| p.is_union() && p.leads(requester))?;
        let (from, to) = (u8::try_from(from).ok()?, u8::try_from(to).ok()?);
        if usize::from(from.max(to)) >= MAX_UNION_MEMBERS || from == to {
            return None;
        }
        let u = p.union.as_mut().expect("a union");
        let (a, b) = (u.slots.iter().position(|&s| s == from), u.slots.iter().position(|&s| s == to));
        match (a, b) {
            (None, None) => return None,
            (Some(a), b) => {
                u.slots[a] = to;
                if let Some(b) = b {
                    u.slots[b] = from;
                }
            }
            (None, Some(b)) => u.slots[b] = from,
        }
        Some((p.members.clone(), p.settings().expect("a union")))
    }

    /// `0x0B16` (`CParty::BreakParty`): the union and its party are gone. Returns who was in it.
    pub fn dismantle(&mut self, requester: u32) -> Option<Vec<u32>> {
        let id = self.led_union(requester)?.id;
        let p = self.parties.remove(&id).expect("found");
        for m in &p.members {
            self.of.remove(m);
        }
        Some(p.members)
    }

    /// `0x0B11` `/join` into the union of `target`'s party (`CParty::AutoPartyJoin`): the party id, or the `0x0B19` code: 12 without
    /// a party, 13 without auto-join (a normal party has none), 14 when full. The caller looked the target up (12 when not found
    /// or of the other country) and checked that the player is in no party.
    pub fn auto_join(&mut self, player: u32, target: u32) -> Result<u32, u32> {
        use shaiya_proto::party as pp;
        let p = self.party_of(target).ok_or(pp::UNION_NOT_FOUND)?;
        if !p.union.as_ref().is_some_and(|u| u.auto_join) {
            return Err(pp::UNION_NO_AUTO_JOIN);
        }
        if p.is_full() {
            return Err(pp::UNION_FULL);
        }
        let id = p.id;
        if self.join(id, player) { Ok(id) } else { Err(pp::UNION_FULL) }
    }

    /// `CParty::LeaveNormalParty`: `None` when the player is in no party.
    pub fn leave(&mut self, player: u32) -> Option<Departure> {
        self.depart(player)
    }

    /// `CParty::ExpelUser` by the leader, with the leader index and the loot turn kept right (deviation, section 4); `None` unless
    /// `leader` leads a party with `target` in it, `target` being another player. In a union, `ExpelRaidPartyUser`, which lets
    /// the leader kick itself (`docs/original/union.md` section 5).
    pub fn kick(&mut self, leader: u32, target: u32) -> Option<Departure> {
        let p = self.party_of(leader)?;
        if p.leader_id() != leader || (target == leader && !p.is_union()) || !p.members.contains(&target) {
            return None;
        }
        self.depart(target)
    }

    fn depart(&mut self, player: u32) -> Option<Departure> {
        let party_id = self.of.remove(&player)?;
        let p = self.parties.get_mut(&party_id).expect("a member's party exists");
        let slot = p.members.iter().position(|&m| m == player).expect("a member is listed");
        let told = p.members.clone();
        let (leader_changed, sub_changed) = p.remove_slot(slot);
        let mut out = Departure { told, union: p.is_union(), ..Default::default() };
        match p.members.len() {
            0 => {
                self.parties.remove(&party_id);
            }
            1 => {
                // Alone: the last member leaves too.
                let last = p.members[0];
                self.of.remove(&last);
                self.parties.remove(&party_id);
                out.last = Some(last);
            }
            _ => {
                if leader_changed {
                    out.new_leader = Some(p.leader_id());
                }
                if sub_changed {
                    out.new_sub = p.sub_leader_id();
                }
                out.remaining = p.members.clone();
            }
        }
        Some(out)
    }

    /// `CParty::SetPartyBoth`: the requester must lead a party with `target` at another slot. Returns the party's members, the old
    /// and the new leader.
    pub fn set_leader(&mut self, requester: u32, target: u32) -> Option<(Vec<u32>, u32, u32)> {
        let p = self.party_of_mut(requester)?;
        if p.leader_id() != requester {
            return None;
        }
        let slot = p.members.iter().position(|&m| m == target).filter(|&s| s != p.leader)?;
        p.leader = slot;
        Some((p.members.clone(), requester, target))
    }

    /// Loot rule 2 (`CMob::ItemDrop` 0x4a65a0): a member drawn out of all of them.
    pub fn random_member(&self, party: u32, roll: u32) -> Option<u32> {
        let p = self.parties.get(&party)?;
        p.members.get(roll as usize % p.members.len()).copied()
    }

    /// Loot rule 1, in turn (`CMob::ItemDrop` 0x4a65a0): starting at the turn, the first member `eligible` gets it and the turn
    /// moves past it.
    pub fn next_in_turn(&mut self, party: u32, mut eligible: impl FnMut(u32) -> bool) -> Option<u32> {
        let p = self.parties.get_mut(&party)?;
        let count = p.members.len();
        for _ in 0..count {
            if p.turn >= count {
                p.turn = 0;
            }
            let member = p.members[p.turn];
            p.turn += 1;
            if eligible(member) {
                return Some(member);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party_of(parties: &Parties, id: u32) -> Vec<u32> {
        parties.party_of(id).map(|p| p.members.clone()).unwrap_or_default()
    }

    /// A party of `members`, the first its leader.
    fn made(members: &[u32]) -> Parties {
        let mut parties = Parties::default();
        let (id, _) = parties.party_for(members[0], false).unwrap();
        for &m in &members[1..] {
            assert!(parties.join(id, m));
        }
        parties
    }

    #[test]
    fn experience_shares_follow_the_tables() {
        assert_eq!(member_exp(1000, 2, false, 30, 30), 1000);
        assert_eq!(member_exp(1000, 3, false, 30, 30), 730);
        assert_eq!(member_exp(1000, 7, false, 30, 30), 390);
        assert_eq!(member_exp(1000, 2, false, 30, 22), 800, "8 levels below the highest");
        assert_eq!(member_exp(1000, 2, false, 30, 23), 1000);
        assert_eq!(member_exp(1000, 2, false, 60, 12), 200, "48 levels below");
        assert_eq!(member_exp(999, 3, false, 30, 14), 437, "both steps truncate: 999 × 0.73 = 729, × 0.6 = 437");
        assert_eq!(gold_share(100, 3), 33);
        assert_eq!(gold_share(2, 3), 1, "at least 1");
    }

    #[test]
    fn invitations_create_the_party_and_fill_it() {
        let mut parties = Parties::default();
        let now = Instant::now();
        parties.invite(1, 2, now);
        assert!(parties.engaged(2));
        assert_eq!(parties.take_invite(2), Some(1));
        assert_eq!(parties.party_for(1, false), Some((1, true)));
        assert!(parties.join(1, 2));
        assert_eq!(party_of(&parties, 2), [1, 2]);
        assert_eq!(parties.party_type(1), shaiya_proto::party::TYPE_LEADER);
        assert_eq!(parties.party_type(2), shaiya_proto::party::TYPE_MEMBER);
        assert_eq!(parties.party_type(3), shaiya_proto::party::TYPE_NONE);
        assert_eq!(parties.party_for(2, false), None, "a member who does not lead cannot bring anyone");
        for m in 3..=7 {
            assert!(parties.join(1, m));
        }
        assert_eq!(parties.party_for(1, false), None, "full");
        assert!(!parties.join(1, 8));
    }

    #[test]
    fn invitations_expire_after_30_seconds() {
        let mut parties = Parties::default();
        let now = Instant::now();
        parties.invite(1, 2, now);
        assert_eq!(parties.expire(now + Duration::from_secs(29)), 0);
        assert_eq!(parties.expire(now + INVITE_TIME), 1);
        assert_eq!(parties.take_invite(2), None);
    }

    #[test]
    fn the_leader_leaving_hands_over_to_slot_zero() {
        let mut parties = made(&[1, 2, 3]);
        parties.set_leader(1, 3).unwrap();
        let out = parties.leave(3).unwrap();
        assert_eq!(out.told, [1, 2, 3]);
        assert_eq!(out.new_leader, Some(1));
        assert_eq!(out.remaining, [1, 2]);
        assert_eq!(parties.party_type(1), shaiya_proto::party::TYPE_LEADER);
    }

    #[test]
    fn a_member_before_the_leader_leaving_keeps_the_leader() {
        let mut parties = made(&[1, 2, 3]);
        parties.set_leader(1, 3).unwrap();
        let out = parties.leave(1).unwrap();
        assert_eq!(out.new_leader, None);
        assert_eq!(parties.party_of(2).unwrap().leader_id(), 3);
    }

    #[test]
    fn the_last_member_leaves_with_the_one_before() {
        let mut parties = made(&[1, 2]);
        let out = parties.leave(1).unwrap();
        assert_eq!(out.told, [1, 2]);
        assert_eq!(out.last, Some(2));
        assert!(out.remaining.is_empty());
        assert!(parties.party_of(2).is_none());
        assert!(parties.leave(2).is_none());
    }

    #[test]
    fn kicking_keeps_the_leader_right() {
        let mut parties = made(&[1, 2, 3, 4]);
        parties.set_leader(1, 3).unwrap();
        assert!(parties.kick(1, 2).is_none(), "only the leader kicks");
        assert!(parties.kick(3, 3).is_none(), "not oneself");
        let out = parties.kick(3, 1).unwrap();
        assert_eq!(out.told, [1, 2, 3, 4]);
        assert_eq!(out.new_leader, None);
        assert_eq!(parties.party_of(2).unwrap().leader_id(), 3, "the original would make 4 leader here");
    }

    #[test]
    fn leadership_goes_to_another_member_only() {
        let mut parties = made(&[1, 2]);
        assert!(parties.set_leader(2, 1).is_none(), "not the leader");
        assert!(parties.set_leader(1, 1).is_none(), "already leader");
        assert!(parties.set_leader(1, 9).is_none(), "not a member");
        assert_eq!(parties.set_leader(1, 2), Some((vec![1, 2], 1, 2)));
        assert_eq!(parties.party_type(2), shaiya_proto::party::TYPE_LEADER);
    }

    /// A union of `members`, the first its leader, made with `rule`.
    fn union_of(members: &[u32], rule: u32) -> Parties {
        let mut parties = made(members);
        parties.make_union(members[0], 1, false, rule).unwrap();
        parties
    }

    #[test]
    fn a_union_picks_its_sub_leader_as_the_original() {
        let parties = union_of(&[1, 2, 3], RULE_IN_TURN);
        let p = parties.party_of(1).unwrap();
        assert_eq!(p.sub_leader_id(), Some(3), "led from index 0 with 3 members: index 2, not 1");
        assert_eq!(p.slotted(), [(0, 1), (1, 2), (2, 3)]);
        assert_eq!(union_of(&[1, 2], RULE_IN_TURN).party_of(1).unwrap().sub_leader_id(), Some(2), "2 members: index 1");
        let mut parties = made(&[1, 2, 3]);
        parties.set_leader(1, 2).unwrap();
        parties.make_union(2, 1, true, RULE_RANDOM).unwrap();
        assert_eq!(parties.party_of(1).unwrap().sub_leader_id(), Some(1), "led from index 1: index 0");
    }

    #[test]
    fn making_a_union_is_the_leaders_with_a_known_rule() {
        let mut parties = made(&[1, 2, 3]);
        assert!(parties.make_union(2, 1, false, RULE_IN_TURN).is_none(), "not the leader");
        assert!(parties.make_union(1, 0, false, RULE_IN_TURN).is_none(), "bRaidType 0");
        assert!(parties.make_union(1, 1, false, 0).is_none(), "rule 0 (deviation 2)");
        assert!(parties.make_union(1, 1, false, 5).is_none(), "rule 5 (deviation 2)");
        let (members, settings) = parties.make_union(1, 7, true, RULE_LEADER).unwrap();
        assert_eq!(members, [1, 2, 3]);
        assert_eq!(settings, shaiya_proto::party::UnionSettings { leader_slot: 0, sub_leader_slot: 2, rule: 4, auto_join: true });
        assert!(parties.party_of(1).unwrap().is_union(), "any non-zero bRaidType (deviation 1)");
    }

    #[test]
    fn an_existing_union_is_not_set_up_again() {
        let mut parties = union_of(&[1, 2, 3, 4], RULE_IN_TURN);
        parties.move_in_union(1, 1, 9).unwrap();
        assert!(parties.make_union(1, 1, false, RULE_RANDOM).is_none(), "deviation 8");
        let p = parties.party_of(1).unwrap();
        assert_eq!(p.slotted(), [(0, 1), (9, 2), (2, 3), (3, 4)], "the groups kept");
        assert_eq!(p.settings().unwrap().rule, RULE_IN_TURN);
    }

    #[test]
    fn union_members_take_the_lowest_free_slot_up_to_30() {
        let mut parties = union_of(&[1, 2, 3], RULE_IN_TURN);
        let id = parties.party_of(1).unwrap().id;
        parties.move_in_union(1, 1, 7).unwrap();
        assert!(parties.join(id, 4));
        assert_eq!(parties.party_of(4).unwrap().slot_of(4), Some(1), "slot 1 is free again");
        for m in 5..=30 {
            assert!(parties.join(id, m));
        }
        assert!(!parties.join(id, 31), "30 at most");
        assert!(parties.party_of(1).unwrap().is_full());
        assert_eq!(parties.party_for(1, false), None);
    }

    #[test]
    fn a_union_leader_leaving_hands_over_to_the_sub_leader() {
        let mut parties = union_of(&[1, 2, 3, 4], RULE_IN_TURN);
        let out = parties.leave(1).unwrap();
        assert!(out.union);
        assert_eq!(out.told, [1, 2, 3, 4]);
        assert_eq!(out.new_leader, Some(3), "the sub-leader");
        assert_eq!(out.new_sub, Some(2), "the first member that does not lead");
        let p = parties.party_of(2).unwrap();
        assert_eq!((p.leader_id(), p.sub_leader_id()), (3, Some(2)));
        assert_eq!(p.slotted(), [(1, 2), (2, 3), (3, 4)], "everyone keeps its slot");
    }

    #[test]
    fn a_union_sub_leader_leaving_is_replaced() {
        let mut parties = union_of(&[1, 2, 3, 4], RULE_IN_TURN);
        let out = parties.kick(1, 3).unwrap();
        assert_eq!(out.new_leader, None);
        assert_eq!(out.new_sub, Some(2));
        let out = parties.leave(4).unwrap();
        assert_eq!(out.new_sub, None, "another member: no change");
        let out = parties.leave(2).unwrap();
        assert_eq!(out.last, Some(1), "alone: the last leaves too");
        assert!(parties.party_of(1).is_none());
    }

    #[test]
    fn a_union_leader_may_kick_itself() {
        let mut parties = union_of(&[1, 2, 3], RULE_IN_TURN);
        let out = parties.kick(1, 1).unwrap();
        assert_eq!(out.new_leader, Some(3));
        assert!(made(&[1, 2]).kick(1, 1).is_none(), "not in a normal party");
    }

    #[test]
    fn group_moves_stay_within_the_slots() {
        let mut parties = union_of(&[1, 2, 3], RULE_IN_TURN);
        assert!(parties.move_in_union(2, 0, 6).is_none(), "a member who does not lead");
        assert!(parties.move_in_union(1, 0, 30).is_none(), "slot 30 (deviation 3)");
        assert!(parties.move_in_union(1, 10, 11).is_none(), "nothing there: nothing sent (deviation 3)");
        assert!(parties.move_in_union(1, 1, 1).is_none());
        let (_, settings) = parties.move_in_union(3, 0, 6).unwrap();
        assert_eq!(settings.leader_slot, 6, "the sub-leader moves the leader to group 1");
        parties.move_in_union(1, 6, 1).unwrap();
        let p = parties.party_of(1).unwrap();
        assert_eq!(p.slotted(), [(1, 1), (6, 2), (2, 3)], "swapped");
        assert_eq!((p.group_of(1), p.group_of(2), p.group_of(9)), (Some(0), Some(1), None));
    }

    #[test]
    fn the_union_leader_hands_over_and_names_the_sub_leader() {
        let mut parties = union_of(&[1, 2, 3], RULE_IN_TURN);
        assert!(parties.set_union_leader(2, 1).is_none(), "not the leader");
        assert!(made(&[1, 2]).set_union_leader(1, 2).is_none(), "not a union");
        let change = parties.set_union_leader(1, 3).unwrap();
        assert_eq!((change.old, change.new, change.sub), (1, 3, Some(1)), "the sub-leader led: the first other member is sub");
        assert_eq!(parties.set_sub_leader(3, 2), Some((vec![1, 2, 3], 2)));
        assert_eq!(parties.set_sub_leader(3, 3), Some((vec![1, 2, 3], 2)), "the leader itself: unchanged, still answered");
        assert_eq!(parties.party_type(3), shaiya_proto::party::TYPE_LEADER);
        assert_eq!(parties.party_type(2), shaiya_proto::party::TYPE_MEMBER, "a sub-leader is a member");
    }

    #[test]
    fn auto_join_and_dismantling() {
        use shaiya_proto::party as pp;
        let mut parties = union_of(&[1, 2], RULE_IN_TURN);
        assert_eq!(parties.auto_join(5, 9), Err(pp::UNION_NOT_FOUND), "no party");
        assert_eq!(parties.auto_join(5, 2), Err(pp::UNION_NO_AUTO_JOIN));
        parties.set_auto_join(1, true).unwrap();
        assert!(parties.set_auto_join(2, true).is_none(), "the leader's");
        assert!(parties.auto_join(5, 2).is_ok());
        assert_eq!(parties.party_of(5).unwrap().slot_of(5), Some(2));
        assert_eq!(made(&[7, 8]).auto_join(5, 7), Err(pp::UNION_NO_AUTO_JOIN), "a normal party");
        assert!(parties.set_union_rule(1, 9).is_none());
        assert_eq!(parties.set_union_rule(1, RULE_RANDOM), Some(vec![1, 2, 5]));
        assert!(parties.dismantle(2).is_none(), "the leader's");
        assert_eq!(parties.dismantle(1), Some(vec![1, 2, 5]));
        assert!(parties.party_of(5).is_none() && parties.party_of(1).is_none(), "the party ends too");
    }

    #[test]
    fn a_union_shares_by_its_own_table_without_the_bonus() {
        assert_eq!(member_exp(1000, 2, true, 30, 30), 730, "one step behind the party's 1.0");
        assert_eq!(member_exp(1000, 7, true, 30, 30), 305);
        assert_eq!(member_exp(1000, 18, true, 30, 30), 10);
        assert_eq!(member_exp(1000, 30, true, 30, 30), 10);
        assert!(full_party(7, false));
        assert!(!full_party(7, true));
        assert!(!full_party(6, false));
        let parties = union_of(&[1, 2, 3], RULE_IN_TURN);
        let id = parties.party_of(1).unwrap().id;
        assert_eq!(parties.random_member(id, 4), Some(2));
    }

    #[test]
    fn loot_goes_round_the_eligible_members() {
        let mut parties = made(&[1, 2, 3]);
        let id = parties.party_of(1).unwrap().id;
        let all = |_| true;
        assert_eq!(parties.next_in_turn(id, all), Some(1));
        assert_eq!(parties.next_in_turn(id, all), Some(2));
        assert_eq!(parties.next_in_turn(id, |m| m != 3), Some(1), "3 is out of range: skipped");
        assert_eq!(parties.next_in_turn(id, all), Some(2));
        assert_eq!(parties.next_in_turn(id, |_| false), None);
    }
}
