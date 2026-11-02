//! Friends in the zone (`docs/original/friends.md`): what needs the maps and the parties (asking and answering on the same map, the
//! party search among the players of the map, leaving). Part of the zone: `friends` is taken after `maps` and `parties`, held briefly.
//! The bookkeeping and the friend packets are `crate::friends`'s.

use super::*;
use crate::backend::StoredFriend;
use crate::friends::{Friends, Player};
use crate::sync::Relock;

impl Zones {
    pub(super) fn friends(&self) -> std::sync::MutexGuard<'_, Friends> {
        self.friends.guard()
    }

    /// At start: the friends with their database writer.
    pub fn load_friends(&self, friends: Friends) {
        *self.friends() = friends;
    }

    /// The friend and block lists of the enter-world burst (section 6).
    pub fn friend_burst(&self, id: u32, friends: &[StoredFriend], blocks: &[StoredFriend]) -> Vec<Vec<u8>> {
        self.friends().burst(id, friends, blocks)
    }

    /// Entering the world, right after the burst (`CUser::InitFriend`).
    pub fn friend_connect(&self, id: u32, player: Player) {
        self.friends().connect(id, player);
    }

    /// Leaving the world (`CUser::ReleaseFriend`), before `Zones::leave`: a waiting request is refused to an asker on `map`.
    pub fn friend_leave_world(&self, map: u16, id: u32) {
        let maps = self.maps.guard();
        self.friends().disconnect(id, |asker| maps.get(&map).is_some_and(|players| players.contains_key(&asker)));
    }

    pub fn friend_level(&self, id: u32, level: u16) {
        self.friends().level_up(id, level);
    }

    /// Whether `by` has blocked `who` (deviation 5).
    pub fn blocked(&self, by: u32, who: u32) -> bool {
        self.friends().blocked(by, who)
    }

    /// `0x2202` (section 3): the one asked must be on the asker's map.
    pub fn friend_request(&self, map: u16, me: u32, name: &[u8], now: Instant) {
        let maps = self.maps.guard();
        self.friends().request(me, name, |id| maps.get(&map).is_some_and(|players| players.contains_key(&id)), now);
    }

    /// `0x2203` (section 4): the asker must be on the answerer's map.
    pub fn friend_answer(&self, map: u16, me: u32, yes: bool, now: Instant) {
        let maps = self.maps.guard();
        self.friends().answer(me, yes, |id| maps.get(&map).is_some_and(|players| players.contains_key(&id)), now);
    }

    pub fn friend_remove(&self, me: u32, id: u32) {
        self.friends().remove(me, id);
    }

    pub fn friend_memo(&self, me: u32, id: u32, memo: &[u8]) {
        self.friends().friend_memo(me, id, memo);
    }

    pub fn block(&self, me: u32, name: &[u8]) {
        self.friends().block(me, name);
    }

    pub fn unblock(&self, me: u32, id: u32) {
        self.friends().unblock(me, id);
    }

    pub fn block_memo(&self, me: u32, id: u32, memo: &[u8]) {
        self.friends().block_memo(me, id, memo);
    }

    /// `0x220C` (section 8): among the players on the map, registered unless in a party.
    pub fn party_search(&self, map: u16, me: u32) {
        let maps = self.maps.guard();
        let parties = self.parties.guard();
        let on_map: Vec<u32> = maps.get(&map).map(|players| players.keys().copied().collect()).unwrap_or_default();
        self.friends().party_search(me, parties.party_of(me).is_some(), &on_map);
    }
}
