//! Party instance zones (`docs/original/instances.md`) and guild house zones (`docs/original/guild-house.md` section 7): one zone
//! per party (or guild) and map, who is on the way to it, its portal countries and when an empty one goes. Pure; the zone keeps it (`Zones` `instances`) and owns what is in the zones.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::world::{map_of, zone_key};

/// `0x020A` refusals for a party instance map (section 3).
pub const REFUSED_NO_PARTY: u8 = 4;
pub const REFUSED_CLOSED: u8 = 9;
pub const REFUSED_FEW_MEMBERS: u8 = 10;
pub const REFUSED_FULL: u8 = 11;

/// The pairs a zone keeps for its instance portals (`CZone +0x218`).
const PORTAL_PAIRS: usize = 10;
/// An instance portal's country when the zone has not set it: nobody passes (`CZone::GetInsZonePortalCountry`).
pub const PORTAL_NOBODY: u32 = 3;
/// Portal ids above this are instance portals (section 5).
pub const PORTAL_ID_FLOOR: u32 = 1000;

/// Whose zone it is: a party's (`CParty +0x08`) or a guild's (its id, a house map).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    Party(u32),
    Guild(u32),
}

/// One instance zone.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub owner: Owner,
    /// The players on their way in (booked when they used the portal, until they are placed or leave the world).
    pub booked: HashSet<u32>,
    /// Empty: when it goes (`CZone +0x20`); `None` while someone is in it or on the way.
    pub empty_until: Option<Instant>,
    /// `(portal id, country)`, id 0 = unused.
    portals: [(u32, u32); PORTAL_PAIRS],
}

impl Instance {
    fn new(owner: Owner) -> Self {
        Self { owner, booked: HashSet::new(), empty_until: None, portals: [(0, 0); PORTAL_PAIRS] }
    }

    /// An instance portal's country here (`CZone::GetInsZonePortalCountry`): the pair's, or nobody.
    pub fn portal_country(&self, id: u32) -> u32 {
        self.portals.iter().find(|&&(pid, _)| pid == id).map_or(PORTAL_NOBODY, |&(_, country)| country)
    }

    /// Sets an instance portal's country (`CZone::UpdateInsZonePortalCountry`): the pair with that id, else the first unused one;
    /// with all ten used by others, nothing. Ids up to 1000 are not instance portals.
    pub fn set_portal(&mut self, id: u32, country: u32) {
        if id <= PORTAL_ID_FLOOR {
            return;
        }
        let slot = self.portals.iter().position(|&(pid, _)| pid == id).or_else(|| self.portals.iter().position(|&(pid, _)| pid == 0));
        if let Some(slot) = slot {
            self.portals[slot] = (id, country);
        }
    }
}

/// Every instance zone by zone key, and each owner's zone of a map.
#[derive(Debug, Default)]
pub struct Instances {
    zones: HashMap<u16, Instance>,
    by_owner: HashMap<(u16, Owner), u16>,
}

impl Instances {
    /// The owner's zone of a map (`CWorld::FindPartyInsZone`'s look-up).
    pub fn find(&self, map: u16, owner: Owner) -> Option<u16> {
        self.by_owner.get(&(map_of(map), owner)).copied()
    }

    pub fn get(&self, zone: u16) -> Option<&Instance> {
        self.zones.get(&zone)
    }

    pub fn get_mut(&mut self, zone: u16) -> Option<&mut Instance> {
        self.zones.get_mut(&zone)
    }

    /// A new zone of the map for the owner: the lowest free instance number (1 to 255); `None` when all are taken.
    pub fn create(&mut self, map: u16, owner: Owner) -> Option<u16> {
        let map = map_of(map);
        let zone = (1..=u8::MAX).map(|n| zone_key(map, n)).find(|zone| !self.zones.contains_key(zone))?;
        self.zones.insert(zone, Instance::new(owner));
        self.by_owner.insert((map, owner), zone);
        Some(zone)
    }

    /// Books a player into a zone (and out of any other).
    pub fn book(&mut self, zone: u16, player: u32) {
        self.unbook(player);
        if let Some(instance) = self.zones.get_mut(&zone) {
            instance.booked.insert(player);
            instance.empty_until = None;
        }
    }

    /// The player is placed somewhere or has left the world: no longer on the way anywhere.
    pub fn unbook(&mut self, player: u32) {
        for instance in self.zones.values_mut() {
            instance.booked.remove(&player);
        }
    }

    /// Takes a zone out (`CWorldThread::Update`'s release): its party's zone of the map is gone.
    pub fn release(&mut self, zone: u16) -> Option<Instance> {
        let instance = self.zones.remove(&zone)?;
        self.by_owner.remove(&(map_of(zone), instance.owner));
        Some(instance)
    }

    /// The one-second expiry pass (section 6): a zone with no players (`occupied`) and nobody on the way gets its time, now +
    /// `expire` of its map, the first time it is found empty; once that time has passed it is released. A zone with someone in it
    /// or on the way loses its time, so it starts afresh each time the zone empties (deviation 1; the original sets it once).
    /// Returns the zones released.
    pub fn expire(&mut self, now: Instant, occupied: impl Fn(u16) -> bool, expire: impl Fn(u16) -> Duration) -> Vec<u16> {
        let mut gone = Vec::new();
        for (&zone, instance) in &mut self.zones {
            if occupied(zone) || !instance.booked.is_empty() {
                instance.empty_until = None;
            } else {
                match instance.empty_until {
                    None => instance.empty_until = Some(now + expire(zone)),
                    Some(until) if until <= now => gone.push(zone),
                    Some(_) => {}
                }
            }
        }
        gone.sort_unstable();
        for &zone in &gone {
            self.release(zone);
        }
        gone
    }

    /// How many instance zones there are.
    /// The zones now open by kind of owner: parties' and guilds' (houses).
    pub fn counts(&self) -> (usize, usize) {
        let party = self.zones.values().filter(|z| matches!(z.owner, Owner::Party(_))).count();
        (party, self.zones.len() - party)
    }

    pub fn len(&self) -> usize {
        self.zones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }

    /// The players on the way into a zone, whom `CWorld::FindPartyInsZone` counts with those in it against `EnterMaxUser`.
    pub fn booked_count(&self, zone: u16) -> usize {
        self.zones.get(&zone).map_or(0, |i| i.booked.len())
    }
}

/// Whether a player may go into a party instance map (section 3), in the original's order: closed (9), no party (4), the party's
/// zone full (11: `full` when it exists, is not the player's own zone and its players plus those on the way reach `EnterMaxUser`), too
/// few members (10).
pub fn entry_refusal(open: bool, party_members: Option<usize>, full: bool, min_users: u32) -> Option<u8> {
    if !open {
        return Some(REFUSED_CLOSED);
    }
    let Some(members) = party_members else { return Some(REFUSED_NO_PARTY) };
    if full {
        return Some(REFUSED_FULL);
    }
    (members < min_users as usize).then_some(REFUSED_FEW_MEMBERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_take_the_lowest_free_number_and_are_found_by_party() {
        let mut all = Instances::default();
        assert_eq!(all.create(65, Owner::Party(7)), Some(65 | 1 << 8));
        assert_eq!(all.create(65, Owner::Party(8)), Some(65 | 2 << 8));
        assert_eq!(all.create(66, Owner::Party(7)), Some(66 | 1 << 8));
        assert_eq!(
            (all.find(65, Owner::Party(7)), all.find(65 | 2 << 8, Owner::Party(8)), all.find(65, Owner::Party(9))),
            (Some(65 | 1 << 8), Some(65 | 2 << 8), None)
        );
        assert!(all.release(65 | 1 << 8).is_some());
        assert_eq!(all.find(65, Owner::Party(7)), None);
        assert_eq!(all.create(65, Owner::Party(9)), Some(65 | 1 << 8), "a released number is free again");
    }

    #[test]
    fn portal_countries_default_to_nobody_and_keep_ten_pairs() {
        let mut zone = Instance::new(Owner::Party(1));
        assert_eq!(zone.portal_country(1001), PORTAL_NOBODY);
        zone.set_portal(1001, 0);
        assert_eq!(zone.portal_country(1001), 0);
        zone.set_portal(1001, 2);
        assert_eq!(zone.portal_country(1001), 2, "the pair changes");
        zone.set_portal(900, 0);
        assert_eq!(zone.portal_country(900), PORTAL_NOBODY, "not an instance portal");
        for id in 1002..1011 {
            zone.set_portal(id, 1);
        }
        zone.set_portal(1011, 1);
        assert_eq!((zone.portal_country(1010), zone.portal_country(1011)), (1, PORTAL_NOBODY), "ten pairs at most");
    }

    #[test]
    fn an_empty_zone_goes_after_its_time_which_restarts_when_someone_comes() {
        let mut all = Instances::default();
        let zone = all.create(65, Owner::Party(1)).unwrap();
        let t0 = Instant::now();
        let expire = |_| Duration::from_secs(330);
        assert!(all.expire(t0, |_| false, expire).is_empty());
        assert_eq!(all.get(zone).unwrap().empty_until, Some(t0 + Duration::from_secs(330)));
        assert!(all.expire(t0 + Duration::from_secs(100), |_| true, expire).is_empty(), "someone came back");
        assert_eq!(all.get(zone).unwrap().empty_until, None);
        let t1 = t0 + Duration::from_secs(400);
        assert!(all.expire(t1, |_| false, expire).is_empty(), "the time starts afresh");
        all.book(zone, 5);
        assert!(all.expire(t1 + Duration::from_secs(1000), |_| false, expire).is_empty(), "booked");
        all.unbook(5);
        assert!(all.expire(t1 + Duration::from_secs(1000), |_| false, expire).is_empty());
        assert_eq!(all.expire(t1 + Duration::from_secs(1330), |_| false, expire), vec![zone]);
        assert_eq!(all.find(65, Owner::Party(1)), None);
    }

    #[test]
    fn refusals_come_in_the_original_order() {
        assert_eq!(entry_refusal(false, None, true, 18), Some(REFUSED_CLOSED));
        assert_eq!(entry_refusal(true, None, true, 18), Some(REFUSED_NO_PARTY));
        assert_eq!(entry_refusal(true, Some(1), true, 18), Some(REFUSED_FULL));
        assert_eq!(entry_refusal(true, Some(17), false, 18), Some(REFUSED_FEW_MEMBERS));
        assert_eq!(entry_refusal(true, Some(1), false, 1), None);
    }
}
