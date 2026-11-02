//! Obelisks (`docs/original/obelisks.md`): the obelisks of `Data/Obelisk.ini` as the original sets them up, their guards' spots,
//! the countries, an obelisk's state and what a capture does to the kill counts. Pure; the zone keeps each map's obelisks and
//! their monsters.

use std::time::{Duration, Instant};

use crate::kc::{Changes, KillCounts};
use crate::mobs::MobDefs;

/// Countries as the server keeps them (a player's): light, fury, and neutral for an obelisk nobody holds.
pub const LIGHT: u8 = 0;
pub const FURY: u8 = 1;
pub const NEUTRAL: u8 = 2;
/// How long the obelisks stay down after a reset (`CMap::ObeliskReset`: 600000 ms).
pub const RESET_DELAY: Duration = Duration::from_secs(600);

/// Who holds an obelisk, as saved (`obelisk_owners`; the original's `0x0207` to ps_dbAgent): by map and number, the owner and the
/// last faction that took it (`nCountryLastDestroy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub map: u16,
    pub number: u16,
    pub country: u8,
    pub last: u8,
}

/// An `[Obelisk_K]` line of the file with its `[Mob]` lines (`world.obelisks`, `world.obelisk_guards`), as written.
#[derive(Clone, Debug, PartialEq)]
pub struct FileObelisk {
    pub map: u16,
    pub number: u16,
    pub position: [f32; 3],
    /// The file's order: neutral, light, fury.
    pub mobs: [u16; 3],
    /// The file's numbering: 0 neutral, 1 light, 2 fury.
    pub country: u8,
    pub guards: Vec<FileGuards>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileGuards {
    pub centre: [f32; 3],
    pub radius: f32,
    /// The file's order: neutral, light, fury.
    pub mobs: [u16; 3],
    pub count: u32,
}

/// A guard group (`stMapObeliskMob`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardGroup {
    pub centre: [f32; 3],
    pub radius: f32,
    /// `nRadius2x`: twice the radius cut to a whole number.
    pub spread: u32,
    pub count: u32,
    /// The guard for each owner (light, fury, neutral); `None` when that monster does not count.
    pub mobs: [Option<u16>; 3],
}

impl GuardGroup {
    /// A guard's spot (`CMap::ObeliskRespawn`, `UpdateRespawnObelisk`): `rand() % nRadius2x − radius + centre` on x, then on z with
    /// the second roll; the centre's height. (A radius below 1 would divide by 0 in the original; here it puts the guard at
    /// `centre − radius`.)
    pub fn spot(&self, r1: u32, r2: u32) -> [f32; 3] {
        let spread = self.spread.max(1);
        let [x, y, z] = self.centre;
        [(r1 % spread) as f32 - self.radius + x, y, (r2 % spread) as f32 - self.radius + z]
    }
}

/// An obelisk (`stMapObelisk`) as set up from the file.
#[derive(Clone, Debug, PartialEq)]
pub struct ObeliskDef {
    /// `dwID`: what the client knows it by.
    pub id: u32,
    pub number: u16,
    pub position: [f32; 3],
    /// The base country (light, fury or neutral).
    pub base: u8,
    /// The obelisk's monster for each owner (light, fury, neutral); `None` when it does not count.
    pub mobs: [Option<u16>; 3],
    pub guards: Vec<GuardGroup>,
}

/// The obelisks of the maps that have them and how many count towards a capture's worth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obelisks {
    pub maps: std::collections::HashMap<u16, Vec<ObeliskDef>>,
    /// `CMap::m_nNumTotalObelisk`: the obelisks on their maps.
    pub total: usize,
}

/// The file's country (0 neutral, 1 light, 2 fury) as the server keeps it.
pub fn from_file(country: u8) -> u8 {
    match country {
        1 => LIGHT,
        2 => FURY,
        _ => NEUTRAL,
    }
}

/// A country as the packets carry it (`stMapObelisk::GetCountry`): 0 neutral, 1 light, 2 fury.
pub fn packet_country(country: u8) -> u8 {
    match country {
        LIGHT => 1,
        FURY => 2,
        _ => 0,
    }
}

/// The owner after a fall to `country` (`CMap::ObeliskDestroy`): neutral gives the base country.
pub fn new_owner(country: u8, base: u8) -> u8 {
    if country == NEUTRAL { base } else { country }
}

/// Who takes an obelisk that fell with no player to credit (`CMob::Death`): the other side of its owner; neutral stays neutral.
pub fn other_side(owner: u8) -> u8 {
    match owner {
        LIGHT => FURY,
        FURY => LIGHT,
        _ => NEUTRAL,
    }
}

/// `CMap::LoadObelisk` after reading: the obelisks of maps that exist (`size_of` gives a map's size), numbered from 1 in file order;
/// a monster counts when its definition has an id, a level and HP; an obelisk off its map keeps its id but no monster, and only the
/// others count towards the total and get their guards. File order: neutral, light, fury; kept by owner: light, fury, neutral.
pub fn place(file: &[FileObelisk], size_of: impl Fn(u16) -> Option<u32>, defs: &MobDefs) -> Obelisks {
    let counts = |id: u16| defs.get(id).filter(|d| d.id != 0 && d.level != 0 && d.hp != 0).map(|d| d.id);
    let by_owner = |m: [u16; 3]| [counts(m[1]), counts(m[2]), counts(m[0])];
    let mut out = Obelisks::default();
    let mut next_id = 1;
    for o in file {
        let Some(size) = size_of(o.map) else { continue };
        let on_map = |v: f32| (0.0..size as f32).contains(&v);
        let placed = on_map(o.position[0]) && on_map(o.position[2]);
        let guards = if placed {
            o.guards
                .iter()
                .map(|g| GuardGroup {
                    centre: g.centre,
                    radius: g.radius,
                    spread: 2 * (g.radius.max(0.0) as u32),
                    count: g.count,
                    mobs: by_owner(g.mobs),
                })
                .collect()
        } else {
            Vec::new()
        };
        out.total += usize::from(placed);
        out.maps.entry(o.map).or_default().push(ObeliskDef {
            id: next_id,
            number: o.number,
            position: o.position,
            base: from_file(o.country),
            mobs: if placed { by_owner(o.mobs) } else { [None; 3] },
            guards,
        });
        next_id += 1;
    }
    out
}

/// Whether an obelisk stands, lies dead or is waiting to rise (`stMapObelisk`'s respawn entry and `pObeliskMob`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Down; it rises when the time has come (`None`: now).
    Due(Option<Instant>),
    /// Its monster stands.
    Standing(u32),
    /// Its monster died and the body still lies: no time yet (the original's 0xFFFFFFFF); set when the body goes.
    Fallen(u32),
}

/// A guard group's guards (`stMapObeliskMob`: its list and respawn entry). `current` counts the guards of this rising there, alive
/// or lying dead (deviation 1); `generation` changes at each rising and fall, so older bodies no longer count.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Guards {
    pub alive: Vec<u32>,
    pub current: u32,
    pub due: Option<Instant>,
    pub generation: u32,
}

/// An obelisk on its map with its owner.
#[derive(Clone, Debug, PartialEq)]
pub struct Obelisk {
    pub def: ObeliskDef,
    pub country: u8,
    /// `nCountryLastDestroy`.
    pub last: u8,
    pub phase: Phase,
    pub groups: Vec<Guards>,
}

/// What a fall did (`CMap::ObeliskDestroy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fall {
    /// The guards to take off the map.
    pub guards: Vec<u32>,
    pub from: u8,
    pub to: u8,
}

impl Obelisk {
    /// At the base country, waiting to rise at once.
    pub fn new(def: ObeliskDef) -> Self {
        let groups = vec![Guards::default(); def.guards.len()];
        Self { country: def.base, last: def.base, phase: Phase::Due(None), groups, def }
    }

    /// Its monster for its owner, if that counts.
    pub fn monster(&self) -> Option<u16> {
        self.def.mobs[usize::from(self.country.min(NEUTRAL))]
    }

    /// A guard group's monster for the owner.
    pub fn guard(&self, group: usize) -> Option<u16> {
        self.def.guards.get(group)?.mobs[usize::from(self.country.min(NEUTRAL))]
    }

    /// Its `0x2101` row.
    pub fn row(&self) -> shaiya_proto::obelisk::Row {
        let [x, _, z] = self.def.position;
        shaiya_proto::obelisk::Row { id: self.def.id, state: packet_country(self.country), x, z }
    }

    pub fn owner(&self, map: u16) -> Owner {
        Owner { map, number: self.def.number, country: self.country, last: self.last }
    }

    /// A new rising (`ObeliskRespawn`): every group starts again, empty, its guards due at once.
    pub fn rise(&mut self) {
        for g in &mut self.groups {
            *g = Guards { generation: g.generation.wrapping_add(1), ..Default::default() };
        }
    }

    /// The fall to `country` (`ObeliskDestroy` steps 1, 2 and 6): every guard goes; the owner is `country`, neutral giving the
    /// base; a new faction owner is the last destroyer.
    pub fn fall(&mut self, country: u8) -> Fall {
        let mut guards = Vec::new();
        for g in &mut self.groups {
            guards.append(&mut g.alive);
            *g = Guards { generation: g.generation.wrapping_add(1), ..Default::default() };
        }
        let from = self.country;
        self.country = new_owner(country, self.def.base);
        if self.country != from && self.country != NEUTRAL {
            self.last = self.country;
        }
        Fall { guards, from, to: self.country }
    }

    /// The reset (`ObeliskReset` steps 2 and 3): down for ten minutes, back to the base country.
    pub fn reset(&mut self, now: Instant) {
        self.phase = Phase::Due(Some(now + RESET_DELAY));
        self.country = self.def.base;
        if self.country != NEUTRAL {
            self.last = self.country;
        }
    }
}

/// A capture's kill counts (`ObeliskDestroy` step 4): when the taker is a faction, its worth is added; then, unless that put the
/// taker in its top grade's ten minutes, the loser's worth is taken off it if it was a faction.
pub fn capture(k: &mut KillCounts, from: u8, to: u8, worth: [u32; 2], now: Instant) -> Changes {
    let mut changes = Changes::default();
    if to == NEUTRAL {
        return changes;
    }
    let added = worth[usize::from(to.min(FURY))];
    if added != 0 {
        changes.merge(k.add(to, added, now));
    }
    if (added == 0 || !k.in_top(to)) && from != NEUTRAL {
        let lost = worth[usize::from(from.min(FURY))];
        if lost != 0 {
            changes.merge(k.sub(from, lost, now));
        }
    }
    changes
}

/// What the zone asks the server to do for the obelisks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// An obelisk fell to a kill (`0x2103` to the world and the kill counts); the monster, the owner before and after.
    Fell { mob_id: u16, from: u8, to: u8 },
    /// An owner to save.
    Owner(Owner),
}

/// What a capture adds to the taker's kill count and takes off the loser's (`CMap::m_nObeliskKCStatusAdd`): each faction's top
/// grade's kills ÷ the obelisks + 1; nothing without obelisks.
pub fn worth(grades: &crate::kc::Grades, total: usize) -> [u32; 2] {
    if total == 0 {
        return [0; 2];
    }
    std::array::from_fn(|f| grades.get(f, grades.top(f)).kills / total as u32 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobs::MobDef;

    fn defs() -> MobDefs {
        let def = |id| MobDef { id, level: 50, hp: 1000, ..Default::default() };
        MobDefs::new([def(52), def(252), def(152), def(77), def(69), def(73), MobDef { hp: 0, ..def(9) }])
    }

    fn file(map: u16, number: u16, x: f32) -> FileObelisk {
        FileObelisk {
            map,
            number,
            position: [x, 1.0, 10.0],
            mobs: [52, 252, 152],
            country: 0,
            guards: vec![FileGuards { centre: [5.0, 1.0, 5.0], radius: 3.7, mobs: [77, 69, 9], count: 2 }],
        }
    }

    #[test]
    fn obelisks_are_set_up_as_the_original_does() {
        let rows = [file(0, 1, 10.0), file(5, 1, 10.0), file(0, 2, 100.0), file(18, 1, 10.0)];
        let o = place(&rows, |m| [(0, 64), (18, 1024)].into_iter().find(|&(id, _)| id == m).map(|(_, s)| s), &defs());
        let map0 = &o.maps[&0];
        assert_eq!(map0.iter().map(|d| (d.number, d.id)).collect::<Vec<_>>(), [(1, 1), (2, 2)], "map 5 does not exist: no id");
        assert_eq!(o.maps[&18][0].id, 3);
        assert_eq!(map0[0].mobs, [Some(252), Some(152), Some(52)], "by owner: light, fury, neutral");
        assert_eq!(map0[0].base, NEUTRAL);
        let g = map0[0].guards[0];
        assert_eq!((g.spread, g.mobs), (6, [Some(69), None, Some(77)]), "2 × trunc(3.7); no HP does not count");
        assert_eq!(map0[1].mobs, [None; 3], "off its map");
        assert!(map0[1].guards.is_empty());
        assert_eq!(o.total, 2);
    }

    #[test]
    fn guards_stand_within_their_radius() {
        let g = GuardGroup { centre: [100.0, 7.0, 200.0], radius: 3.5, spread: 6, count: 1, mobs: [None; 3] };
        assert_eq!(g.spot(0, 5), [96.5, 7.0, 201.5]);
        assert_eq!(g.spot(13, 6), [97.5, 7.0, 196.5]);
    }

    #[test]
    fn countries_and_owners() {
        assert_eq!([from_file(0), from_file(1), from_file(2)], [NEUTRAL, LIGHT, FURY]);
        assert_eq!([packet_country(LIGHT), packet_country(FURY), packet_country(NEUTRAL)], [1, 2, 0]);
        assert_eq!(new_owner(NEUTRAL, FURY), FURY, "a fury base never becomes neutral");
        assert_eq!(new_owner(LIGHT, NEUTRAL), LIGHT);
        assert_eq!([other_side(LIGHT), other_side(FURY), other_side(NEUTRAL)], [FURY, LIGHT, NEUTRAL]);
    }

    #[test]
    fn a_capture_is_worth_the_top_grade_over_the_obelisks() {
        let row = |faction, kills| crate::kc::Row { faction, kills, bonuses: [(0, 0); 6] };
        let grades = crate::kc::Grades::new(&[row(0, 150), row(0, 12288), row(1, 12000)]);
        assert_eq!(worth(&grades, 9), [12288 / 9 + 1, 12000 / 9 + 1]);
        assert_eq!(worth(&grades, 0), [0, 0]);
    }
}
