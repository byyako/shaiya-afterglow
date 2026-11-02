//! Boss groups (`docs/original/obelisks.md` sections 11.1 and 11.3): the `[bothmob_…]` groups of `Data/Obelisk.ini` as the original
//! sets them up, and a group's chain of bosses in a zone: its stages, its followers, its chests and when the next boss is due. Pure;
//! the zone keeps the chains of each zone and their monsters (`zone/boss.rs`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::mobs::MobDefs;
use crate::obelisk::{Guards, Phase};

/// A portal id below this is no boss door (`CMap::LoadObelisk`: "portal ids from 1001").
pub const PORTAL_ID_MIN: u32 = 1001;
/// The portal's country while the chain runs (`BothMobRespawn`: nobody passes) and once its last stage fell (everyone).
pub const PORTAL_SHUT: u32 = 3;
pub const PORTAL_OPEN: u32 = 0;

/// A `[bothmob_…]` line with its `[Mob]` and `[RandomRespawn]` lines (`world.boss_groups`, `world.boss_followers`), as written.
#[derive(Clone, Debug, PartialEq)]
pub struct FileBossGroup {
    pub number: u32,
    pub map: u16,
    pub respawn_secs: u32,
    /// The portal's map and id.
    pub portal: (u16, u32),
    pub position: [f32; 3],
    /// One per stage.
    pub mobs: Vec<u16>,
    /// Min and max minutes and the three spots.
    pub random: Option<(u32, u32, [[f32; 3]; 3])>,
    pub followers: Vec<FileFollowers>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileFollowers {
    pub centre: [f32; 3],
    pub radius: f32,
    /// One per stage, then the chest monster (0: none).
    pub mobs: Vec<u16>,
    pub count: u32,
}

/// A follower group (`stMapBothMob`'s `[Mob]` line).
#[derive(Clone, Debug, PartialEq)]
pub struct Followers {
    pub centre: [f32; 3],
    pub radius: f32,
    /// `nRadius2x`: twice the radius cut to a whole number.
    pub spread: u32,
    pub count: u32,
    /// The follower for each stage; `None` when there is none or it does not count.
    pub mobs: Vec<Option<u16>>,
    /// The chest monster, which comes `count` times when the last stage falls.
    pub chest: Option<u16>,
}

impl Followers {
    /// A follower's or a chest's spot: as an obelisk guard's (`crate::obelisk::GuardGroup::spot`).
    pub fn spot(&self, r1: u32, r2: u32) -> [f32; 3] {
        crate::obelisk::GuardGroup { centre: self.centre, radius: self.radius, spread: self.spread, count: 0, mobs: [None; 3] }.spot(r1, r2)
    }
}

/// A random group's times and spots (`[RandomRespawn]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Random {
    pub min_minutes: u32,
    pub max: u32,
    pub spots: [[f32; 3]; 3],
}

/// A boss group (`stMapBoth`) as set up from the file.
#[derive(Clone, Debug, PartialEq)]
pub struct BossGroup {
    pub number: u32,
    /// The boss of each stage; `None` when it does not count. The first always counts (a group whose first does not never runs).
    pub stages: Vec<Option<u16>>,
    /// The header's respawn time, the base of a fixed group's wait after its last stage.
    pub respawn: Duration,
    /// The portal it opens: its map, its place in that map's list and its id.
    pub portal: Option<(u16, u8, u32)>,
    pub position: [f32; 3],
    pub random: Option<Random>,
    pub followers: Vec<Followers>,
}

impl BossGroup {
    /// A random group's wait (`MapInitBothMob`, `UpdateMobDeath`): min minutes and a random part below max × 60 seconds.
    pub fn random_wait(random: &Random, r: u32) -> Duration {
        let spread = random.max.saturating_mul(60);
        let extra = if spread == 0 { 0 } else { r % spread };
        Duration::from_secs(u64::from(random.min_minutes) * 60 + u64::from(extra))
    }

    /// The wait after the last stage fell (`CZone::UpdateMobDeath`): a fixed group the header's time ± up to an hour (`rand % 120 −
    /// 60` minutes, never below 0), a random group its random wait.
    pub fn chain_wait(&self, r: u32) -> Duration {
        match &self.random {
            Some(random) => Self::random_wait(random, r),
            None => {
                let minutes = i64::from(r % 120) - 60;
                Duration::from_secs((self.respawn.as_secs() as i64 + minutes * 60).max(0) as u64)
            }
        }
    }

    /// The boss's spot: a random group's one of three (`rand() % 3`), else the line's.
    pub fn spot(&self, r: u32) -> [f32; 3] {
        self.random.map_or(self.position, |random| random.spots[(r % 3) as usize])
    }
}

/// The boss groups of the maps that have them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BossGroups {
    pub maps: HashMap<u16, Vec<BossGroup>>,
}

impl BossGroups {
    pub fn of(&self, map: u16) -> &[BossGroup] {
        self.maps.get(&map).map_or(&[], Vec::as_slice)
    }
}

/// `CMap::LoadObelisk` after reading: the groups of maps that exist (`has_map`), every monster looked up (it counts with an id, a
/// level and HP, as for obelisks); the portal resolves when its id is at least 1001 and its map has a portal with that id
/// (`portal_place` gives its place), else the group has none and still runs. A group whose first stage does not count never runs
/// and is left out.
pub fn place(
    file: &[FileBossGroup],
    has_map: impl Fn(u16) -> bool,
    portal_place: impl Fn(u16, u32) -> Option<u8>,
    defs: &MobDefs,
) -> BossGroups {
    let counts = |id: u16| defs.get(id).filter(|d| d.id != 0 && d.level != 0 && d.hp != 0).map(|d| d.id);
    let mut out = BossGroups::default();
    for g in file.iter().filter(|g| has_map(g.map)) {
        let stages: Vec<_> = g.mobs.iter().map(|&id| counts(id)).collect();
        if stages.first().copied().flatten().is_none() {
            continue;
        }
        let (pmap, id) = g.portal;
        let portal = (id >= PORTAL_ID_MIN).then(|| portal_place(pmap, id).map(|place| (pmap, place, id))).flatten();
        let followers = g
            .followers
            .iter()
            .map(|f| Followers {
                centre: f.centre,
                radius: f.radius,
                spread: 2 * (f.radius.max(0.0) as u32),
                count: f.count,
                mobs: (0..stages.len()).map(|s| f.mobs.get(s).copied().and_then(counts)).collect(),
                chest: f.mobs.get(stages.len()).copied().and_then(counts),
            })
            .collect();
        out.maps.entry(g.map).or_default().push(BossGroup {
            number: g.number,
            stages,
            respawn: Duration::from_secs(u64::from(g.respawn_secs)),
            portal,
            position: g.position,
            random: g.random.map(|(min_minutes, max, spots)| Random { min_minutes, max, spots }),
            followers,
        });
    }
    out
}

/// A boss group's chain in a zone (`stMapBoth` in `CZone`): the stage, whether its boss stands, the follower groups (deviation 7:
/// a group counts the followers of this stage really there, as obelisk guards) and the chests of the last fall.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    pub def: BossGroup,
    pub stage: usize,
    pub phase: Phase,
    /// The boss was taken away (`/mera`, `LuaDeleteMob`) rather than killed: its body only waits its own respawn time.
    pub removed: bool,
    pub groups: Vec<Guards>,
    pub chests: Vec<u32>,
    /// The last stage fell and its chests are still to come (the next respawn pass places them).
    pub chests_due: bool,
    /// The first respawn pass of the zone has set it up (`start`).
    pub started: bool,
}

/// What a fall did (`BothMobDestroy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destroy {
    /// The followers to take off the map.
    pub followers: Vec<u32>,
    /// It was the last stage: the chain starts over, the portal opens, the chests come.
    pub ended: bool,
}

impl Chain {
    /// At stage 0, not yet set up.
    pub fn new(def: BossGroup) -> Self {
        let groups = vec![Guards::default(); def.followers.len()];
        Self { def, stage: 0, phase: Phase::Due(None), removed: false, groups, chests: Vec::new(), chests_due: false, started: false }
    }

    /// The zone's set-up (`MapInitBothMob`), at its first respawn pass: a fixed group is due at once, a random group after its random
    /// wait.
    pub fn start(&mut self, now: Instant, rand: &mut impl FnMut() -> u32) {
        if let Some(random) = &self.def.random {
            self.phase = Phase::Due(Some(now + BossGroup::random_wait(random, rand())));
        }
        self.started = true;
    }

    /// The current stage's boss, if it counts.
    pub fn boss(&self) -> Option<u16> {
        self.def.stages.get(self.stage).copied().flatten()
    }

    /// A follower group's monster for the current stage.
    pub fn follower(&self, group: usize) -> Option<u16> {
        self.def.followers.get(group)?.mobs.get(self.stage).copied().flatten()
    }

    /// A new rising: every follower group starts again, empty, due at once.
    pub fn rise(&mut self) {
        for g in &mut self.groups {
            *g = Guards { generation: g.generation.wrapping_add(1), ..Default::default() };
        }
    }

    /// `BothMobDestroy`: every follower goes, the stage goes up; past the last, back to stage 0 and the chain ended.
    pub fn destroy(&mut self) -> Destroy {
        let mut followers = Vec::new();
        for g in &mut self.groups {
            followers.append(&mut g.alive);
            *g = Guards { generation: g.generation.wrapping_add(1), ..Default::default() };
        }
        self.stage += 1;
        let ended = self.stage >= self.def.stages.len();
        if ended {
            self.stage = 0;
            self.chests_due = true;
        }
        Destroy { followers, ended }
    }

    /// The boss's body went (`CZone::UpdateMobDeath`): due after the monster's own respawn time; after the last stage of a boss that
    /// was killed, the chain's wait instead. Returns the wait.
    pub fn body_gone(&mut self, own: Duration, now: Instant, rand: &mut impl FnMut() -> u32) -> Duration {
        let wait = if self.stage == 0 && !self.removed { self.def.chain_wait(rand()) } else { own };
        self.phase = Phase::Due(Some(now + wait));
        self.removed = false;
        wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobs::MobDef;

    fn defs() -> MobDefs {
        let def = |id| MobDef { id, level: 60, hp: 1000, ..Default::default() };
        MobDefs::new([def(1974), def(1977), def(1980), def(1972), MobDef { level: 0, hp: 0, ..def(1984) }, def(2414)])
    }

    fn dragons() -> FileBossGroup {
        let line = |mobs: Vec<u16>| FileFollowers { centre: [230.0, 1.0, 1031.0], radius: 5.0, mobs, count: 1 };
        FileBossGroup {
            number: 1,
            map: 19,
            respawn_secs: 604_800,
            portal: (19, 1001),
            position: [229.43, 1.0, 1031.1],
            mobs: vec![1974, 1977],
            random: None,
            followers: vec![line(vec![0, 1980, 0]), line(vec![0, 1984, 0]), FileFollowers { count: 16, ..line(vec![0, 0, 1972]) }],
        }
    }

    fn set_up(file: &[FileBossGroup]) -> BossGroups {
        place(file, |m| m != 5, |map, id| (map == 19 && id == 1001).then_some(8), &defs())
    }

    #[test]
    fn groups_are_set_up_as_the_original_does() {
        let mut low = dragons();
        low.portal = (19, 1000);
        let mut missing = dragons();
        missing.portal = (19, 1005);
        let mut first = dragons();
        first.mobs = vec![1984, 1977];
        let mut off = dragons();
        off.map = 5;
        let groups = set_up(&[dragons(), low, missing, first, off]);
        let map19 = groups.of(19);
        assert_eq!(map19.len(), 3, "the first stage does not count, the map does not exist: not run");
        assert_eq!(map19[0].portal, Some((19, 8, 1001)));
        assert_eq!((map19[1].portal, map19[2].portal), (None, None), "below 1001, no portal with that id");
        let f = &map19[0].followers;
        assert_eq!((f[0].mobs.clone(), f[0].chest, f[0].spread), (vec![None, Some(1980)], None, 10));
        assert_eq!(f[1].mobs, [None, None], "1984 has no level or HP");
        assert_eq!((f[2].chest, f[2].count), (Some(1972), 16));
        assert!(groups.of(5).is_empty());
    }

    #[test]
    fn a_two_stage_chain() {
        let now = Instant::now();
        let def = set_up(&[dragons()]).of(19)[0].clone();
        let mut chain = Chain::new(def);
        chain.start(now, &mut || 0);
        assert_eq!((chain.phase, chain.boss()), (Phase::Due(None), Some(1974)), "a fixed group is due at once");
        chain.phase = Phase::Standing(7);
        chain.groups[0].alive.push(9);
        let first = chain.destroy();
        assert_eq!(first, Destroy { followers: vec![9], ended: false });
        assert_eq!((chain.stage, chain.boss(), chain.follower(0)), (1, Some(1977), Some(1980)));
        chain.body_gone(Duration::ZERO, now, &mut || 0);
        assert_eq!(chain.phase, Phase::Due(Some(now)), "1974's own respawn time (code 14: 0 s)");
        chain.groups[0].alive.push(10);
        let generation = chain.groups[0].generation;
        let last = chain.destroy();
        assert_eq!(last, Destroy { followers: vec![10], ended: true });
        assert_eq!((chain.stage, chain.chests_due), (0, true));
        assert_ne!(chain.groups[0].generation, generation, "older bodies no longer count");
        chain.body_gone(Duration::from_secs(7200), now, &mut || 90);
        assert_eq!(chain.phase, Phase::Due(Some(now + Duration::from_secs(604_800 + 30 * 60))), "the header's time + 30 min");
    }

    #[test]
    fn waits() {
        let now = Instant::now();
        let mut def = set_up(&[dragons()]).of(19)[0].clone();
        assert_eq!(def.chain_wait(0), Duration::from_secs(604_800 - 3600), "an hour less");
        assert_eq!(def.chain_wait(119), Duration::from_secs(604_800 + 59 * 60));
        def.respawn = Duration::from_secs(600);
        assert_eq!(def.chain_wait(0), Duration::ZERO, "never below now");
        let random = Random { min_minutes: 45, max: 3600, spots: [[1.0; 3], [2.0; 3], [3.0; 3]] };
        def.random = Some(random);
        assert_eq!(def.chain_wait(216_000 + 7), Duration::from_secs(45 * 60 + 7), "below max × 60 s");
        assert_eq!([def.spot(3), def.spot(4), def.spot(5)], [[1.0; 3], [2.0; 3], [3.0; 3]]);
        let mut chain = Chain::new(def.clone());
        chain.start(now, &mut || 100);
        assert_eq!(chain.phase, Phase::Due(Some(now + Duration::from_secs(45 * 60 + 100))), "a random group waits at set-up");
        let mut chain = Chain::new(BossGroup { random: None, ..def });
        chain.removed = true;
        chain.body_gone(Duration::from_secs(300), now, &mut || 0);
        assert_eq!((chain.phase, chain.removed), (Phase::Due(Some(now + Duration::from_secs(300))), false), "removed: its own time only");
    }
}
