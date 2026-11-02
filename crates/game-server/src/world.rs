//! The maps this server runs and the NPCs placed on them, read from the `world` schema at start-up
//! (`docs/original/maps-npcs.md`). Which placements become NPCs follows the original's `CMap::InitNPC`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use shaiya_proto::world::NpcEnter;

use crate::mobs::SpawnArea;

/// NPC type the original never places from a map file (`CMap::InitNPC`): the gambling house's, which `GAME_MINI_GAME` places
/// (`docs/original/mini-game.md` decision 1).
pub const GAMBLING_NPC_TYPE: u8 = 5;
/// The largest height (either way) `is_safe_pos` accepts; real terrain is within a few hundred.
pub const MAX_HEIGHT: f32 = 10_000.0;

/// One NPC placement of a map file, as stored in `world.map_npcs`, with its first position if it has any.
#[derive(Clone, Debug)]
pub struct MapNpcRow {
    pub map: u16,
    pub kind: i16,
    pub type_id: i16,
    /// x, y, z and the direction in radians, as in the file.
    pub first: Option<[f32; 4]>,
    /// Every position in order, the first included (`stMapNpc::pPos`); two or more make a walk (`docs/original/guards.md` section 3).
    pub walk: Vec<[f32; 3]>,
}

/// What an NPC definition says of moving and fighting (`stNpc`, `docs/original/guards.md` section 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NpcDef {
    /// `nMoveDist`: a step's length.
    pub move_dist: u32,
    /// `nMoveSpeed`: milliseconds per walking step (a quarter of it per running step).
    pub move_speed: u32,
    /// 0 Light, 1 Fury, 2 both.
    pub country: u8,
}

/// A gatekeeper's destination (`stGateLink`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub map: u16,
    /// With the 0.9 the original adds to y when loading.
    pub position: [f32; 3],
    pub cost: u32,
}

/// A gatekeeper's country and its three destinations; a destination without a name is `None` (the original refuses it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GateKeeper {
    /// 0 Light, 1 Fury, 2 both.
    pub country: u8,
    pub gates: [Option<Gate>; 3],
}

/// A portal of a map file (`stMapPortal`, `docs/original/portals.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Portal {
    pub position: [f32; 3],
    /// 0 everyone, 1 Light, 2 Fury; above 1000 an instance zone portal.
    pub country: u32,
    /// Both 0: no level limit.
    pub level_min: u32,
    pub level_max: u32,
    pub to_map: u32,
    pub to: [f32; 3],
}

impl Portal {
    /// The portal's id (`stMapPortal::nID`): its map-file country when above 1000 (`docs/original/portals.md` section 1).
    pub fn id(&self) -> Option<u32> {
        (self.country > crate::instance::PORTAL_ID_FLOOR).then_some(self.country)
    }
}

/// A bind area of a map file (`stMapBind`): where players come back to life when the map has no rebirth point.
#[derive(Clone, Debug, PartialEq)]
pub struct BindArea {
    /// 1 Light, 2 Fury, 0 or 3 both.
    pub country: u32,
    /// `x1, y1, z1, x2, y2, z2`.
    pub boxes: Vec<[f32; 6]>,
}

/// A map's rebirth points per faction (Light, Fury): map and position as in `Map.ini`.
pub type RebirthPoints = [Option<(u16, [f32; 3])>; 2];

/// `Map.ini` `CreateType` values the server acts on.
pub mod create_type {
    pub const PARTY: u8 = 3;
    pub const GUILD: u8 = 4;
    pub const GUILD_RANKING: u8 = 5;
    pub const GUILD_HOUSE: u8 = 6;
}

/// The highest map id the server runs: zone keys keep the map in their low byte.
pub const MAX_MAP_ID: u16 = 255;

/// The map of a zone key (`docs/original/instances.md` section 18): a static map's zone is its map id, an instance zone of a map is
/// `map | instance << 8` with instance 1 to 255. Every look-up of map data takes the map of the key it is given.
pub fn map_of(zone: u16) -> u16 {
    zone & MAX_MAP_ID
}

/// The zone key of a map's instance (0: the static zone).
pub fn zone_key(map: u16, instance: u8) -> u16 {
    map | u16::from(instance) << 8
}

/// A party instance map's settings (`Map.ini`, `docs/original/instances.md` sections 1 and 2).
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceRules {
    /// `CreateTime` as `(day, hour, hours)`, day 0 Sunday; empty: always open.
    pub open_times: Vec<[u16; 3]>,
    /// `CreateMinUser`: the party's members needed to enter.
    pub min_users: u32,
    /// `EnterMaxUser`: the players a zone takes, those on their way included.
    pub max_users: u32,
    /// How long an empty zone lasts: `ExpireTime` minutes × 60000 + 30000 ms.
    pub expire: Duration,
}

impl Default for InstanceRules {
    fn default() -> Self {
        Self { open_times: Vec::new(), min_users: 0, max_users: 30, expire: Duration::from_millis(30_000) }
    }
}

impl InstanceRules {
    /// The rules from the `world.maps` columns.
    pub fn new(open_times: &[u16], min_users: u32, max_users: u32, expire_minutes: u32) -> Self {
        Self {
            open_times: open_times.as_chunks::<3>().0.iter().take(7).copied().collect(),
            min_users,
            max_users,
            expire: Duration::from_millis(u64::from(expire_minutes) * 60_000 + 30_000),
        }
    }

    /// Whether the zone is open (`CMap::IsPartyInsZoneOpenTime`) on a weekday (0 Sunday) at an hour of the server's local time: on
    /// an entry's day from its hour for its hours, an entry running past midnight also on the next day (Saturday's next is Sunday)
    /// before the hour it ends.
    pub fn is_open(&self, weekday: u16, hour: u16) -> bool {
        self.open_times.is_empty()
            || self.open_times.iter().any(|&[day, from, hours]| {
                let end = from.wrapping_add(hours);
                let next = if end > 24 { if day == 6 { 0 } else { day + 1 } } else { day };
                if weekday == day { from <= hour && hour < end } else { weekday == next && end > 24 && hour < end - 24 }
            })
    }
}

#[derive(Debug, Default)]
pub struct World {
    /// Each map's side; positions must stay inside `0 <= x, z < size`.
    sizes: HashMap<u16, u32>,
    npcs: HashMap<u16, Vec<NpcEnter>>,
    /// The positions of each NPC that walks (two or more), by object id.
    npc_walks: HashMap<u32, Vec<[f32; 3]>>,
    /// The NPC definitions' moving numbers and countries by type and type id.
    npc_defs: HashMap<(u8, u16), NpcDef>,
    /// Each merchant's goods (`world.npc_shop_items`) by its type id: item type and type id, in the list's order.
    shops: HashMap<u16, Vec<(u8, u8)>>,
    /// Gatekeepers by type id.
    gatekeepers: HashMap<u16, GateKeeper>,
    /// Each map's portals in file order, and its `CreateType` (0 when absent).
    portals: HashMap<u16, Vec<Portal>>,
    create_types: HashMap<u16, u8>,
    /// The dungeons: `MapType = D` in `Map.ini` (`CMap::m_nMapType` 1; anything else is a field, 0).
    dungeons: std::collections::HashSet<u16>,
    /// The maps with a war type (`Map.ini` `WarType = P`, `CMap::m_nWarType` 1): nobody is an enemy there outside duels and guild
    /// wars (`docs/original/pvp.md` section 1).
    war_maps: std::collections::HashSet<u16>,
    /// Each map's blocked cells (one bit per unit square, least significant bit first) and monster spawn areas.
    blocked: HashMap<u16, Vec<u8>>,
    spawn_areas: HashMap<u16, Vec<SpawnArea>>,
    /// Each map's rebirth points and bind areas.
    rebirth: HashMap<u16, RebirthPoints>,
    binds: HashMap<u16, Vec<BindArea>>,
    /// The obelisks of `Data/Obelisk.ini` as set up (`docs/original/obelisks.md` section 1).
    obelisks: crate::obelisk::Obelisks,
    /// The boss groups of the same file (section 11.1).
    boss_groups: crate::boss::BossGroups,
    /// The scripted monsters' machines and lines (`docs/original/scripted-mobs.md`).
    mob_ai: crate::mob_ai::Library,
    /// The party instance maps' settings (`docs/original/instances.md` section 1).
    instance_rules: HashMap<u16, InstanceRules>,
    /// The Lua monsters' scripts (section 9).
    mob_lua: crate::mob_lua::Scripts,
    /// `Data/GuildHouse.ini` (`docs/original/guild-house.md` section 2).
    guild_house: crate::guild_house::HouseRules,
    /// Each merchant's shop type (`stNpc +0x124`) by type id.
    shop_types: HashMap<u16, u8>,
}

impl World {
    /// The maps with their sizes, the NPC definitions that exist (type, type id), and the placements in the order the
    /// original creates them (by map, then file order). Object ids are numbered from 1 in that order, as the original's
    /// object counter does.
    pub fn build(
        sizes: impl IntoIterator<Item = (u16, u32)>,
        defined: &HashSet<(u8, u16)>,
        placements: impl IntoIterator<Item = MapNpcRow>,
    ) -> Self {
        Self::build_with(sizes, defined, placements, false)
    }

    /// `build`, with the gambling NPCs (type 5) placed too when `gambling` is set. They are numbered in placement order like the
    /// others, so the object ids of the NPCs after them shift by one each; nothing keeps NPC object ids across restarts.
    pub fn build_with(
        sizes: impl IntoIterator<Item = (u16, u32)>,
        defined: &HashSet<(u8, u16)>,
        placements: impl IntoIterator<Item = MapNpcRow>,
        gambling: bool,
    ) -> Self {
        let mut world = Self { sizes: sizes.into_iter().collect(), npcs: HashMap::new(), ..Default::default() };
        let mut next_id = 1;
        for row in placements {
            let (Ok(kind), Ok(type_id)) = (u8::try_from(row.kind), u16::try_from(row.type_id)) else { continue };
            let Some([x, y, z, radians]) = row.first else { continue };
            if (kind == GAMBLING_NPC_TYPE && !gambling) || !defined.contains(&(kind, type_id)) || !world.is_safe_pos(row.map, [x, y, z]) {
                continue;
            }
            world.npcs.entry(row.map).or_default().push(NpcEnter {
                object_id: next_id,
                kind,
                type_id,
                position: [x, y, z],
                dir: degrees(radians),
            });
            if row.walk.len() >= 2 {
                world.npc_walks.insert(next_id, row.walk);
            }
            next_id += 1;
        }
        world
    }

    /// Whether the map exists and the position is on it (`CMap::IsSafePos`; unlike the original, NaN is refused, and so is a height
    /// outside `-MAX_HEIGHT..=MAX_HEIGHT`, `docs/server/audit.md` R21).
    pub fn is_safe_pos(&self, map: u16, position: [f32; 3]) -> bool {
        let Some(&size) = self.sizes.get(&map_of(map)) else { return false };
        let on_map = |v: f32| (0.0..size as f32).contains(&v);
        on_map(position[0]) && on_map(position[2]) && (-MAX_HEIGHT..=MAX_HEIGHT).contains(&position[1])
    }

    pub fn map_count(&self) -> usize {
        self.sizes.len()
    }

    /// The NPCs placed on each map.
    pub fn npcs(&self) -> &HashMap<u16, Vec<NpcEnter>> {
        &self.npcs
    }

    /// An NPC's positions when it walks.
    pub fn npc_walk(&self, object_id: u32) -> Option<&[[f32; 3]]> {
        self.npc_walks.get(&object_id).map(Vec::as_slice)
    }

    pub fn set_npc_defs(&mut self, defs: HashMap<(u8, u16), NpcDef>) {
        self.npc_defs = defs;
    }

    /// An NPC definition's moving numbers and country (all 0 when unknown).
    pub fn npc_def(&self, kind: u8, type_id: u16) -> NpcDef {
        self.npc_defs.get(&(kind, type_id)).copied().unwrap_or_default()
    }

    /// Whether the NPC definition exists (`CNpcData::GetNpc` finds it).
    pub fn has_npc(&self, kind: u8, type_id: u16) -> bool {
        self.npc_defs.contains_key(&(kind, type_id))
    }

    pub fn set_shops(&mut self, shops: HashMap<u16, Vec<(u8, u8)>>) {
        self.shops = shops;
    }

    pub fn set_gatekeepers(&mut self, gatekeepers: HashMap<u16, GateKeeper>) {
        self.gatekeepers = gatekeepers;
    }

    pub fn gatekeeper(&self, type_id: u16) -> Option<&GateKeeper> {
        self.gatekeepers.get(&type_id)
    }

    pub fn set_guild_house(&mut self, rules: crate::guild_house::HouseRules) {
        self.guild_house = rules;
    }

    pub fn guild_house(&self) -> &crate::guild_house::HouseRules {
        &self.guild_house
    }

    pub fn set_shop_types(&mut self, types: HashMap<u16, u8>) {
        self.shop_types = types;
    }

    /// A merchant's shop type (1 weapons, 2 armour, 3 general goods, 4 potions); 0 when unknown.
    pub fn shop_type(&self, merchant_type_id: u16) -> u8 {
        self.shop_types.get(&merchant_type_id).copied().unwrap_or(0)
    }

    pub fn set_portals(&mut self, portals: HashMap<u16, Vec<Portal>>, create_types: HashMap<u16, u8>) {
        self.portals = portals;
        self.create_types = create_types;
    }

    /// A map's portal by its position in the file's list (`CMap::GetPortal`).
    pub fn portal(&self, map: u16, index: u8) -> Option<&Portal> {
        self.portals.get(&map_of(map))?.get(usize::from(index))
    }

    pub fn set_mob_data(&mut self, blocked: HashMap<u16, Vec<u8>>, spawn_areas: HashMap<u16, Vec<SpawnArea>>) {
        self.blocked = blocked;
        self.spawn_areas = spawn_areas;
    }

    /// Whether the unit square at whole-number `x`, `z` is blocked (`CBitArray::GetBit`); off the map counts as blocked.
    pub fn is_blocked(&self, map: u16, x: i32, z: i32) -> bool {
        let map = map_of(map);
        let Some(&size) = self.sizes.get(&map) else { return true };
        let size = size as i32;
        if !(0..size).contains(&x) || !(0..size).contains(&z) {
            return true;
        }
        let bit = (z * size + x) as usize;
        self.blocked.get(&map).and_then(|bits| bits.get(bit / 8)).is_some_and(|byte| byte >> (bit % 8) & 1 == 1)
    }

    /// Whether a monster can go straight from one whole-number spot to another (`CMap::EnableLine`, `docs/original/mobs.md`
    /// section 6): both on the map, and every unit square after the start, the end included, free.
    pub fn is_line_free(&self, map: u16, from: (i32, i32), to: (i32, i32)) -> bool {
        self.line_end(map, from, to).is_ok()
    }

    /// The straight line walk of `CMap::EnableLine`: steps go along the longer axis (z on a tie); the other coordinate gains
    /// the slope each step and is truncated. `Err` with the last free cell before the first blocked one (the start when the
    /// first step is blocked or a spot is off the map).
    pub fn line_end(&self, map: u16, from: (i32, i32), to: (i32, i32)) -> Result<(), (i32, i32)> {
        let Some(&size) = self.sizes.get(&map_of(map)) else { return Err(from) };
        if ![from.0, from.1, to.0, to.1].into_iter().all(|v| (0..size as i32).contains(&v)) {
            return Err(from);
        }
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let (sx, sz) = (if dx < 0 { -1 } else { 1 }, if dz < 0 { -1 } else { 1 });
        let (long, short) = if dx.abs() > dz.abs() { (dx.abs(), dz.abs()) } else { (dz.abs(), dx.abs()) };
        let x_major = dx.abs() > dz.abs();
        let slope = short as f32 / long as f32 * if x_major { sz } else { sx } as f32;
        let (mut along, mut across) = if x_major { (from.0 + sx, from.1 as f32 + slope) } else { (from.1 + sz, from.0 as f32 + slope) };
        let cell = |along: i32, across: f32| if x_major { (along, across as i32) } else { (across as i32, along) };
        for _ in 0..long {
            if self.is_blocked(map, cell(along, across).0, cell(along, across).1) {
                let step = if x_major { sx } else { sz };
                return Err(cell(along - step, across - slope));
            }
            along += if x_major { sx } else { sz };
            across += slope;
        }
        Ok(())
    }

    /// The file's obelisks on the maps that exist (`CMap::LoadObelisk`).
    pub fn set_obelisks(&mut self, file: &[crate::obelisk::FileObelisk], defs: &crate::mobs::MobDefs) {
        self.obelisks = crate::obelisk::place(file, |map| self.sizes.get(&map).copied(), defs);
    }

    /// The file's boss groups on the maps that exist (`CMap::LoadObelisk`), their portals found by id in the portal's map.
    pub fn set_boss_groups(&mut self, file: &[crate::boss::FileBossGroup], defs: &crate::mobs::MobDefs) {
        let portal_place = |map: u16, id: u32| {
            let place = self.portals.get(&map)?.iter().position(|p| p.id() == Some(id))?;
            u8::try_from(place).ok()
        };
        self.boss_groups = crate::boss::place(file, |map| self.sizes.contains_key(&map), portal_place, defs);
    }

    pub fn boss_groups(&self) -> &crate::boss::BossGroups {
        &self.boss_groups
    }

    pub fn set_mob_ai(&mut self, library: crate::mob_ai::Library) {
        self.mob_ai = library;
    }

    pub fn mob_ai(&self) -> &crate::mob_ai::Library {
        &self.mob_ai
    }

    pub fn set_mob_lua(&mut self, scripts: crate::mob_lua::Scripts) {
        self.mob_lua = scripts;
    }

    pub fn mob_lua(&self) -> &crate::mob_lua::Scripts {
        &self.mob_lua
    }

    pub fn obelisks(&self) -> &crate::obelisk::Obelisks {
        &self.obelisks
    }

    pub fn spawn_areas(&self) -> &HashMap<u16, Vec<SpawnArea>> {
        &self.spawn_areas
    }

    /// The spawn areas of a zone's map.
    pub fn spawn_areas_of(&self, zone: u16) -> &[SpawnArea] {
        self.spawn_areas.get(&map_of(zone)).map_or(&[], Vec::as_slice)
    }

    pub fn create_type(&self, map: u16) -> u8 {
        self.create_types.get(&map_of(map)).copied().unwrap_or(0)
    }

    pub fn set_instance_rules(&mut self, rules: HashMap<u16, InstanceRules>) {
        self.instance_rules = rules;
    }

    /// A party instance map's settings (the defaults for a map without a row).
    pub fn instance_rules(&self, map: u16) -> InstanceRules {
        self.instance_rules.get(&map_of(map)).cloned().unwrap_or_default()
    }

    /// The ranking battle's map (the first map of `CreateType` R, `CWorld::CheckGuildRankStatus`).
    pub fn ranking_map(&self) -> Option<u16> {
        self.create_types.iter().filter(|&(_, &kind)| kind == create_type::GUILD_RANKING).map(|(&map, _)| map).min()
    }

    /// The ranking battle's open times (map 50's `CreateTime`, or the test setting in their place).
    pub fn ranking_times(&self) -> Vec<[u16; 3]> {
        self.ranking_map().map(|map| self.instance_rules(map).open_times).unwrap_or_default()
    }

    /// Replaces the ranking battle map's open times (`GAME_RANKING_TIMES`).
    pub fn set_ranking_times(&mut self, times: Vec<[u16; 3]>) {
        if let Some(map) = self.ranking_map() {
            self.instance_rules.entry(map).or_default().open_times = times;
        }
    }

    pub fn set_dungeons(&mut self, dungeons: impl IntoIterator<Item = u16>) {
        self.dungeons = dungeons.into_iter().collect();
    }

    /// Whether the map is a dungeon (`m_nMapType` 1): no saved positions there (`docs/original/item-windows.md` section 1).
    pub fn is_dungeon(&self, map: u16) -> bool {
        self.dungeons.contains(&map_of(map))
    }

    pub fn set_war_maps(&mut self, maps: impl IntoIterator<Item = u16>) {
        self.war_maps = maps.into_iter().collect();
    }

    /// Whether the map has a war type: players of the two countries are not enemies there (`CUser::IsTeam`).
    pub fn is_war_map(&self, map: u16) -> bool {
        self.war_maps.contains(&map_of(map))
    }

    pub fn set_rebirth(&mut self, rebirth: HashMap<u16, RebirthPoints>, binds: HashMap<u16, Vec<BindArea>>) {
        self.rebirth = rebirth;
        self.binds = binds;
    }

    /// Where a player of `country` (0 Light, 1 Fury) at `position` on `map` comes back to life (`CMap::GetBindPos`,
    /// `docs/original/mob-fight.md` section 8): the map's rebirth point for the country when it is on its map (y + 0.9), else a
    /// random spot of the nearest bind area of the country, else where the player is. `map` may be a zone key: the answer stays
    /// in that zone unless the rebirth point leads elsewhere.
    pub fn bind_pos(&self, map: u16, country: u8, position: [f32; 3], rand: &mut impl FnMut() -> u32) -> (u16, [f32; 3]) {
        let point = self.rebirth.get(&map_of(map)).and_then(|points| points[usize::from(country.min(1))]);
        if let Some((to_map, [x, y, z])) = point
            && self.is_safe_pos(to_map, [x, y, z])
        {
            return (to_map, [x, y + 0.9, z]);
        }
        let Some(areas) = self.binds.get(&map_of(map)).filter(|a| !a.is_empty()) else { return (map, position) };
        let own = if country == 0 { 1 } else { 2 };
        let mut best = (9999.0f32, 0);
        for (i, area) in areas.iter().enumerate() {
            let Some(first) = area.boxes.first() else { continue };
            if !(area.country == own || area.country == 3 || area.country == 0) {
                continue;
            }
            let d = ((position[0] - first[0]).powi(2) + (position[2] - first[2]).powi(2)).sqrt();
            if d < best.0 {
                best = (d, i);
            }
        }
        let area = &areas[best.1];
        if area.boxes.is_empty() {
            return (map, position);
        }
        let b = area.boxes[rand() as usize % area.boxes.len()];
        // Whole-number offsets; a box without width counts as 1 here (the original would divide by zero).
        let spread = |from: f32, to: f32| ((to - from) as i32).max(1) as u32;
        let z = b[2] + (rand() % spread(b[2], b[5])) as f32;
        let x = b[0] + (rand() % spread(b[0], b[3])) as f32;
        (map, [x, b[1] + 0.9, z])
    }

    /// A merchant's goods by its type id (`CNpcData::GetShopNpc`).
    pub fn shop(&self, merchant_type_id: u16) -> Option<&[(u8, u8)]> {
        self.shops.get(&merchant_type_id).map(Vec::as_slice)
    }
}

/// One map as monsters see it when they chase (`crate::fight::Terrain`).
pub struct MapTerrain<'a> {
    pub world: &'a World,
    pub map: u16,
}

impl crate::fight::Terrain for MapTerrain<'_> {
    fn line(&self, from: (i32, i32), to: (i32, i32)) -> Result<(), (i32, i32)> {
        self.world.line_end(self.map, from, to)
    }

    fn blocked(&self, at: (i32, i32)) -> bool {
        self.world.is_blocked(self.map, at.0, at.1)
    }
}

/// A map file's direction in degrees as the original keeps it: `-radians * 57.29578`, plus 360 when negative (the loader),
/// truncated to a whole number (`CMap::InitNPC`).
pub fn degrees(radians: f32) -> u16 {
    let mut d = -radians * 57.29578;
    if d < 0.0 {
        d += 360.0;
    }
    d as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(map: u16, kind: i16, type_id: i16, first: Option<[f32; 4]>) -> MapNpcRow {
        MapNpcRow { map, kind, type_id, first, walk: Vec::new() }
    }

    #[test]
    fn instance_maps_open_by_weekday_and_hour() {
        // Map 64: Monday to Saturday, 21:00 to 22:00.
        let map64 = InstanceRules::new(&[1, 21, 1, 2, 21, 1, 3, 21, 1, 4, 21, 1, 5, 21, 1, 6, 21, 1], 18, 30, 2);
        assert_eq!(map64.expire, Duration::from_millis(150_000));
        assert!(map64.is_open(1, 21) && map64.is_open(6, 21));
        assert!(!map64.is_open(0, 21) && !map64.is_open(1, 20) && !map64.is_open(1, 22));
        assert!(InstanceRules::new(&[], 1, 7, 5).is_open(0, 3), "no CreateTime: always open");
        // Saturday 23:00 for 3 hours runs into Sunday until 02:00.
        let late = InstanceRules::new(&[6, 23, 3], 0, 30, 0);
        assert!(late.is_open(6, 23) && late.is_open(0, 0) && late.is_open(0, 1));
        assert!(!late.is_open(0, 2) && !late.is_open(6, 22) && !late.is_open(1, 0));
        assert_eq!(late.expire, Duration::from_millis(30_000), "ExpireTime 0 still waits 30 s");
    }

    #[test]
    fn zone_keys_keep_the_map_in_their_low_byte() {
        let mut world = World::build([(65, 1024)], &HashSet::new(), []);
        world.set_portals(HashMap::new(), [(65, create_type::PARTY)].into());
        let zone = zone_key(65, 3);
        assert_eq!((zone, map_of(zone), map_of(65)), (65 | 3 << 8, 65, 65));
        assert!(world.is_safe_pos(zone, [10.0, 0.0, 10.0]) && !world.is_safe_pos(zone, [1024.0, 0.0, 10.0]));
        assert_eq!(world.create_type(zone), create_type::PARTY);
    }

    #[test]
    fn directions_are_truncated_degrees() {
        assert_eq!(degrees(0.0), 0);
        assert_eq!(degrees(-std::f32::consts::FRAC_PI_2), 90);
        // 1.0 rad is -57.29578 degrees, so 302.70422, truncated.
        assert_eq!(degrees(1.0), 302);
        assert_eq!(degrees(-1.0), 57);
    }

    #[test]
    fn placements_follow_the_original_rules() {
        let defined = HashSet::from([(1, 1), (5, 1), (8, 2)]);
        let at = |x, z| Some([x, 10.0, z, 0.0]);
        let world = World::build(
            [(1, 2048), (2, 256)],
            &defined,
            [
                row(1, 1, 1, at(100.0, 200.0)),
                row(1, 5, 1, at(100.0, 200.0)), // type 5 is not placed unless gambling is on
                row(1, 1, 9, at(100.0, 200.0)), // no such NPC
                row(1, 8, 2, None),             // no position
                row(2, 8, 2, at(300.0, 10.0)),  // off the map
                row(3, 1, 1, at(10.0, 10.0)),   // a map this server does not run
                row(2, 8, 2, at(255.5, 0.0)),
                row(2, 99, 1, at(1.0, 1.0)), // a type that does not fit
            ],
        );
        let ids = |map| world.npcs().get(&map).map(|n| n.iter().map(|n| (n.object_id, n.kind, n.type_id)).collect::<Vec<_>>());
        assert_eq!(ids(1), Some(vec![(1, 1, 1)]));
        assert_eq!(ids(2), Some(vec![(2, 8, 2)]), "ids count only the NPCs created");
        assert_eq!(ids(3), None);
        // With the gambling house on, the type-5 placement is made (`docs/original/mini-game.md` decision 1), and the ids after it shift.
        let with = World::build_with(
            [(1, 2048)],
            &defined,
            [row(1, 1, 1, at(100.0, 200.0)), row(1, 5, 1, at(120.0, 200.0)), row(1, 8, 2, at(140.0, 200.0))],
            true,
        );
        let kinds = with.npcs().get(&1).map(|n| n.iter().map(|n| (n.object_id, n.kind)).collect::<Vec<_>>());
        assert_eq!(kinds, Some(vec![(1, 1), (2, 5), (3, 8)]));
    }

    #[test]
    fn a_blocked_line_ends_at_its_last_free_cell() {
        let mut world = World::build([(1, 16)], &HashSet::new(), []);
        // x = 5 blocked on rows 0 to 3.
        let mut bits = vec![0u8; 16 * 16 / 8];
        for z in 0..4 {
            let bit = z * 16 + 5;
            bits[bit / 8] |= 1 << (bit % 8);
        }
        world.set_mob_data([(1, bits)].into(), HashMap::new());
        assert_eq!(world.line_end(1, (1, 1), (9, 1)), Err((4, 1)));
        assert_eq!(world.line_end(1, (4, 2), (9, 2)), Err((4, 2)), "the first step is blocked: the start");
        assert_eq!(world.line_end(1, (1, 5), (9, 5)), Ok(()));
        assert_eq!(world.line_end(1, (1, 1), (20, 1)), Err((1, 1)), "off the map");
    }

    #[test]
    fn players_come_back_at_rebirth_points_or_bind_areas() {
        let mut world = World::build([(1, 2048), (2, 1024), (3, 512)], &HashSet::new(), []);
        let fury_area = BindArea { country: 2, boxes: vec![[100.0, 5.0, 200.0, 110.5, 6.0, 220.0]] };
        let light_area = BindArea { country: 1, boxes: vec![[900.0, 7.0, 900.0, 904.0, 7.0, 908.0]] };
        world.set_rebirth(
            [(1, [Some((2, [10.0, 1.0, 20.0])), None]), (3, [Some((9, [1.0, 1.0, 1.0])), None])].into(),
            [(1, vec![light_area.clone(), fury_area.clone()]), (3, vec![light_area])].into(),
        );
        let mut rolls = [0, 7, 13].into_iter();
        let mut rand = || rolls.next().unwrap();
        assert_eq!(world.bind_pos(1, 0, [0.0; 3], &mut rand), (2, [10.0, 1.9, 20.0]), "Light: the rebirth point, y + 0.9");
        // Fury on map 1: no point, the Fury area; box 0, z = 200 + 7 % 20, x = 100 + 13 % 10.
        assert_eq!(world.bind_pos(1, 1, [0.0; 3], &mut rand), (1, [103.0, 5.9, 207.0]));
        // Map 3: the point is on a map that does not exist; the only area is Light's, used for Fury too (area 0).
        let mut rolls = [0, 1, 2].into_iter();
        assert_eq!(world.bind_pos(3, 1, [5.0; 3], &mut || rolls.next().unwrap()), (3, [902.0, 7.9, 901.0]));
        assert_eq!(world.bind_pos(2, 0, [5.0; 3], &mut || 0), (2, [5.0; 3]), "nothing: where the player is");
    }

    #[test]
    fn safe_positions_stay_inside_the_map() {
        let world = World::build([(1, 256)], &HashSet::new(), []);
        assert!(world.is_safe_pos(1, [0.0, -50.0, 255.9]));
        assert!(world.is_safe_pos(1, [1.0, 10_000.0, 1.0]) && world.is_safe_pos(1, [1.0, -10_000.0, 1.0]));
        assert!(!world.is_safe_pos(1, [1.0, 10_000.5, 1.0]) && !world.is_safe_pos(1, [1.0, 1e38, 1.0]), "a huge height");
        assert!(
            !world.is_safe_pos(1, [1.0, f32::NAN, 1.0]) && !world.is_safe_pos(1, [1.0, f32::INFINITY, 1.0]),
            "a height that is no number"
        );
        assert!(!world.is_safe_pos(1, [256.0, 0.0, 1.0]));
        assert!(!world.is_safe_pos(1, [-0.1, 0.0, 1.0]));
        assert!(!world.is_safe_pos(1, [f32::NAN, 0.0, 1.0]));
        assert!(!world.is_safe_pos(2, [1.0, 0.0, 1.0]), "unknown map");
    }

    #[test]
    fn blocked_cells_are_bits_from_the_map_file() {
        let mut world = World::build([(1, 16)], &HashSet::new(), []);
        // Row 1 (z = 1) starts at bit 16: byte 2. Bit 3 of it is x = 3.
        let mut bits = vec![0u8; 16 * 16 / 8];
        bits[2] = 0b0000_1000;
        world.set_mob_data([(1, bits)].into(), HashMap::new());
        assert!(world.is_blocked(1, 3, 1));
        assert!(!world.is_blocked(1, 2, 1));
        assert!(!world.is_blocked(1, 3, 0));
        assert!(world.is_blocked(1, 16, 0), "off the map");
        assert!(world.is_blocked(2, 0, 0), "unknown map");
    }

    #[test]
    fn lines_check_every_square_after_the_start() {
        let mut world = World::build([(1, 16)], &HashSet::new(), []);
        // Only x = 3, z = 1 is blocked.
        let mut bits = vec![0u8; 16 * 16 / 8];
        bits[2] = 0b0000_1000;
        world.set_mob_data([(1, bits)].into(), HashMap::new());
        assert!(!world.is_line_free(1, (0, 1), (6, 1)), "x is the longer axis");
        assert!(!world.is_line_free(1, (3, 0), (3, 2)), "z is the longer axis");
        assert!(!world.is_line_free(1, (5, 0), (1, 2)), "backwards, x 3 is on z 0 + 2 x 0.5");
        assert!(world.is_line_free(1, (0, 0), (6, 0)));
        assert!(world.is_line_free(1, (3, 1), (3, 4)), "the start is not checked");
        assert!(world.is_line_free(1, (3, 1), (3, 1)));
        assert!(!world.is_line_free(1, (0, 0), (16, 0)), "off the map");
        assert!(!world.is_line_free(2, (0, 0), (1, 0)), "unknown map");
    }
}
