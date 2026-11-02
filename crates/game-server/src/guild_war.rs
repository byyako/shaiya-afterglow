//! Guild wars (`docs/original/guild-war.md`): each guild's side of a war and each player's part in one. Bookkeeping only: the zone
//! keeps the wars by guild and a part on each player in one, and sends the packets.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::zone::Outbox;

/// How long a request waits for its answer (`CUser::PacketPVPGuild`, 10000 ms).
pub const REQUEST_TIME: Duration = Duration::from_secs(10);
/// The countdown before the fight (5000 ms).
pub const COUNTDOWN: Duration = Duration::from_secs(5);
/// A member closer than this to the centre takes part (`CGuild::PvPUserSet`); one further away leaves (`PvPChkLeaveByMove`), x and z.
pub const AREA: f32 = 40.0;
/// The highest rank (the largest number) that may ask or answer (`m_nGuildUserLevel` ≤ 4).
pub const MAX_RANK: u8 = 4;
/// Officers this far apart (x and z) cannot start a war: one of them would be outside the area (deviation 1).
pub const OFFICERS_APART: f32 = 2.0 * AREA;

/// Where a guild is in a war (`CGuild::m_nPvPStatus` 1 to 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// It asked another guild (1).
    Asking,
    /// Another guild asked it (2).
    Asked,
    /// The five seconds before the fight (3).
    Countdown,
    /// Fighting (4).
    Fighting,
}

/// One guild's side of a war.
#[derive(Clone, Debug)]
pub struct War {
    pub stage: Stage,
    /// Its player who asked or was asked (`m_dwPvPGuildBoss`) and the other guild's (`m_dwPvPMaster`).
    pub officer: u32,
    pub other_officer: u32,
    pub enemy: u32,
    pub map: u16,
    pub centre: [f32; 3],
    /// The request's or the countdown's end; the pair's timer, on both sides.
    pub deadline: Option<Instant>,
    /// Its players in the war (`m_nPvPUserCount`), and every player who took part, for the result (`m_bPvPJoinUser`).
    pub fighters: Vec<u32>,
    pub joined: Vec<(u32, Outbox)>,
}

impl War {
    pub fn requesting(&self) -> bool {
        matches!(self.stage, Stage::Asking | Stage::Asked)
    }

    pub fn due(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|d| d <= now)
    }
}

/// A player's part in a war (`m_dwPvPGuildID`, `m_vPvPGPos`, `m_bPvPGuildReady`). `ready` false: the officer of a request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub enemy: u32,
    pub centre: [f32; 3],
    pub ready: bool,
}

/// The wars, by guild.
#[derive(Debug, Default)]
pub struct Wars(HashMap<u32, War>);

impl Wars {
    pub fn get(&self, guild: u32) -> Option<&War> {
        self.0.get(&guild)
    }

    pub fn get_mut(&mut self, guild: u32) -> Option<&mut War> {
        self.0.get_mut(&guild)
    }

    pub fn at_war(&self, guild: u32) -> bool {
        self.0.contains_key(&guild)
    }

    /// A request (`0x2501`, section 3): the requester's guild asks, the target's is asked; 10 s for the answer.
    pub fn request(&mut self, map: u16, (guild, me): (u32, u32), (other, target): (u32, u32), now: Instant) {
        let side = |stage, officer, other_officer, enemy| War {
            stage,
            officer,
            other_officer,
            enemy,
            map,
            centre: [0.0; 3],
            deadline: Some(now + REQUEST_TIME),
            fighters: Vec::new(),
            joined: Vec::new(),
        };
        self.0.insert(guild, side(Stage::Asking, me, target, other));
        self.0.insert(other, side(Stage::Asked, target, me, guild));
    }

    /// Both sides of the war `guild` is in, gone.
    pub fn end(&mut self, guild: u32) -> Option<(War, Option<War>)> {
        let war = self.0.remove(&guild)?;
        let other = self.0.remove(&war.enemy);
        Some((war, other))
    }

    /// The guilds whose timer has run out, one of each pair.
    pub fn due(&self, now: Instant) -> Vec<u32> {
        let mut due: Vec<u32> =
            self.0.iter().filter(|&(&g, w)| w.due(now) && (g < w.enemy || !self.0.contains_key(&w.enemy))).map(|(&g, _)| g).collect();
        due.sort_unstable();
        due
    }
}

fn gap(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Closer than 40 to the centre, x and z: takes part (`CGuild::PvPUserSet`).
pub fn in_area(centre: [f32; 3], at: [f32; 3]) -> bool {
    gap(centre, at) < AREA
}

/// More than 40 from the centre, x and z: out of the war (`CUser::PvPChkLeaveByMove`).
pub fn out_of_area(centre: [f32; 3], at: [f32; 3]) -> bool {
    gap(centre, at) > AREA
}

/// Two officers too far apart to start a war (deviation 1).
pub fn too_far_apart(a: [f32; 3], b: [f32; 3]) -> bool {
    gap(a, b) >= OFFICERS_APART
}

/// A member who may ask or answer: ranks 1 to 4.
pub fn may_lead(rank: u8) -> bool {
    (1..=MAX_RANK).contains(&rank)
}

/// `CUser::IsTeam`'s guild war rule: both in guilds, each the other's enemy guild, both in the war.
pub fn enemies(a: (u32, Option<&Part>), b: (u32, Option<&Part>)) -> bool {
    match (a, b) {
        ((ga, Some(pa)), (gb, Some(pb))) => ga != 0 && gb != 0 && pa.enemy == gb && pb.enemy == ga && pa.ready && pb.ready,
        _ => false,
    }
}

/// `CUser::IsTeamUsable`'s guild war rule: party help passes unless the two enemy guilds differ and one of them is set (0 for none).
pub fn may_help(a: u32, b: u32) -> bool {
    a == b || (a == 0 && b == 0)
}

/// `CUser::IsCloneTargetingUser`'s guild war rule: each the other's enemy guild (both in guilds, the war's stage aside).
pub fn foes(a: (u32, Option<&Part>), b: (u32, Option<&Part>)) -> bool {
    match (a, b) {
        ((ga, Some(pa)), (gb, Some(pb))) => ga != 0 && gb != 0 && pa.enemy == gb && pb.enemy == ga,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(enemy: u32, ready: bool) -> Part {
        Part { enemy, centre: [0.0; 3], ready }
    }

    #[test]
    fn the_area() {
        let c = [100.0, 5.0, 100.0];
        assert!(in_area(c, [139.9, 50.0, 100.0]), "y does not count");
        assert!(!in_area(c, [140.0, 5.0, 100.0]), "40 itself is outside when joining");
        assert!(!out_of_area(c, [140.0, 5.0, 100.0]), "…and not out when moving");
        assert!(out_of_area(c, [130.0, 5.0, 130.0]));
        assert!(too_far_apart([0.0; 3], [80.0, 0.0, 0.0]));
        assert!(!too_far_apart([0.0; 3], [79.9, 0.0, 0.0]));
    }

    #[test]
    fn who_may_lead() {
        assert!(may_lead(1) && may_lead(4));
        assert!(!may_lead(0) && !may_lead(5) && !may_lead(9));
    }

    #[test]
    fn enemies_need_both_in_the_war() {
        let (a, b) = (part(20, true), part(10, true));
        assert!(enemies((10, Some(&a)), (20, Some(&b))));
        assert!(!enemies((10, Some(&a)), (20, Some(&part(10, false)))), "a request's officer is nobody's enemy yet");
        assert!(foes((10, Some(&a)), (20, Some(&part(10, false)))));
        assert!(!enemies((10, Some(&a)), (30, Some(&b))), "another guild");
        assert!(!enemies((10, Some(&a)), (20, None)));
        assert!(!enemies((0, Some(&part(0, true))), (0, Some(&part(0, true)))));
    }

    #[test]
    fn party_help_stays_within_a_side() {
        assert!(may_help(0, 0));
        assert!(may_help(20, 20), "the same side of one war");
        assert!(!may_help(20, 0) && !may_help(0, 20));
        assert!(!may_help(20, 10), "the two sides");
    }

    #[test]
    fn a_request_and_its_timer() {
        let now = Instant::now();
        let mut wars = Wars::default();
        wars.request(1, (10, 1), (20, 2), now);
        assert_eq!(wars.get(10).map(|w| (w.stage, w.officer, w.other_officer, w.enemy)), Some((Stage::Asking, 1, 2, 20)));
        assert_eq!(wars.get(20).map(|w| (w.stage, w.officer, w.other_officer, w.enemy)), Some((Stage::Asked, 2, 1, 10)));
        assert!(wars.due(now).is_empty());
        assert_eq!(wars.due(now + REQUEST_TIME), vec![10], "one of the pair");
        let (war, other) = wars.end(20).expect("at war");
        assert_eq!((war.enemy, other.map(|o| o.enemy)), (10, Some(20)));
        assert!(!wars.at_war(10) && !wars.at_war(20));
    }
}
