# Maps and NPCs: the server's map files, NpcQuest.SData and NPCs in the view

This page covers the data files the EP4.5 game server `ps_game.exe` loads for its maps and NPCs — `Map.ini`, the
`.svmap` map files and the NPC part of `NpcQuest.SData` — how the NPCs are created, and how they are shown to players. It comes from static
analysis of the server and of the data files shipped with it in `PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/Data/`; nothing was
executed.

## In short

- **68 maps** (0 to 67) are loaded, each from its own `.svmap` file. A map file holds the map's size, which squares are blocked, the
  boxes where monsters spawn, where NPCs stand, the portals, and the areas where dead players come back.
- **NPCs come from `NpcQuest.SData`**, sorted into 13 kinds: merchants, gatekeepers, blacksmiths, warehouse keepers, guards and so on.
  1,724 of them stand on the 68 maps.
- **The server checks positions only against the map's edges.** It does not check blocked squares for players; those only keep guards
  from walking into walls.
- **NPCs appear like players and monsters**: when they come into a player's view (the 5 × 5 cells around it) and disappear when they
  leave it.

Status words: **confirmed** = read in the code (address given).

## 1. Files

Confirmed.

| file | read by | content |
|---|---|---|
| `Data/Map.ini` | `CMap::LoadConfig`, `CMap::LoadGameMapConfig` | `[BASE]`: `MapCount = 68` and the four start positions; `[SET_ZONE_n]` per map: `MapType` (F field, D dungeon), `MapName` (another map file; `-1` = default), `RebirthMapPos`, `RebirthMapPos1`/`2` (respawn per faction), weather (`WeatherState` 0 none, 1 rain, 2 snow; rate, power, durations) |
| `Data/Map/<id>.svmap` | `CMap::Load` (0x4256f0) → `CMap::LoadMap` (0x425780) | the server's map: size, blocked cells, monster areas, NPC placements, portals, regions (section 2) |
| `Data/NpcQuest.SData` | `CNpcData::Load` (0x4c9410), then the quest loader | the NPC definitions (section 3), then the quests |
| `Data/Obelisk.ini`, `GuildHouse.ini`, `MapWar.ini`, `mob_ai.ini`, `AI/`, `Lua/` | | obelisks, guild houses, the war server's maps, monster AI (not read yet) |

The game server loads maps 0 to 67 (`MapCount`), each from `data/map/<id>.svmap` unless `MapName` names another file; no map in `Map.ini` does.
Files 100 to 108 exist but are not referred to by `Map.ini` (likely for `ps_gamewar.exe`, not checked). Map names (community list, maps 0 to 64) are
in the configuration guide: 0 D-Water Borderland, 1 Erina, 2 Reikeuseu, 18 Proelium Frontier, 19 Willieoseu, 20 Keuraijen, 35 Apulune, 36 Iris, 42 Auction House, ...

## 2. `.svmap` layout

Confirmed, : `tools/svmap.py` reads all 77 files to the last byte.

Little-endian, in this order:

| part | layout | notes |
|---|---|---|
| size | `u32` | the map's side: 2048 for the big fields, 1024, or smaller for dungeons and rooms |
| blocked cells | `ceil(size * size / 8)` bytes | one bit per unit square, least significant bit first (`0x41b3a0`); all zero on the open fields, mostly set in dungeons. Only used to keep guards from walking into walls |
| A | `u32 n`, n x { `f32 x, y, z`, `u32`, `u32` } | empty in every file |
| B | `u32 n`, n x { `f32 x, y, z` } | a few points on some maps (not identified) |
| monster areas | `u32 n`, n x { `f32 x1, y1, z1, x2, y2, z2`; `u32 m`; m x { `u32 mob id, u32 count` } } | the spawn boxes; mob ids are checked against the monster table (`Mobs`) |
| NPCs | `u32 n`, n x { `u32 type, u32 type id, u32 m`; m x { `f32 x, y, z, f32 direction` } } | the direction is in radians; the loader turns it into degrees (`-d * 57.29578`, plus 360 when negative) |
| portals | `u32 n`, n x 36 bytes { `f32 x, y, z`, `u32 country`, `u16 level min, max`, `u32 map`, `f32 x, y, z` } | read into `stMapPortal` (`docs/original/portals.md`) |
| bind areas | `u32 n`, n x { `u32 m, u32 country, u32 attr`; m x `f32 x1, y1, z1, x2, y2, z2` (`SBoxAxis`) } | `stMapBind`: where players respawn (`CMap::GetBindPos`, `docs/original/mob-fight.md` section 8); country 1 light, 2 fury, 0 or 3 both |
| regions | `u32 n`, n x { `f32 x1, y1, z1, x2, y2, z2`, `u32`, `u8`, `u8`, `u16` } | named areas (not read further) |

Counts per map from `tools/svmap.py`: map 1 has 835 monster areas and 286 NPC entries; map 2 708 and 236; the capitals 35 and 36 about 136 NPCs and no
monsters; dungeons have monsters and portals but no NPCs.

**Position check.** `CMap::IsSafePos` (0x415360) only checks `0 <= x < size` and `0 <= z < size`. Entering a zone (`CZone::EnterUser`) and every move
(`CZone::MoveUserByPacket`) use it; a position outside closes the connection with reason 9. Its x87 comparisons let NaN through; the new server refuses
NaN.

## 3. `NpcQuest.SData`, the NPC part

Confirmed, : `tools/npcquest.py`, 1,379 NPCs, every record of the type of its list.

In this order:
1. `u32 n` merchants (type 1), each: the head with an extra `u8` after the id; `u32 m`, m x { `u8 item type, u8 item type id` } (the shop); the two lists.
2. `u32 n` gatekeepers (type 2), each: the head; 3 x { `u16 map, f32 x, y, z, u32 len, name[len], u32 cost` } (destinations; `y + 0.9` at load); the lists.
3. For types 3 to 13: `u32 n`, n x { the head; the lists }.

Head: `u8 type, u16 type id, [u8 merchant only], u32, u32, u32, u32 country` (1 Light, 2 Fury, other both), `u32 len, name[len], u32 len, text[len]`.
Lists: `u32 n, n x u16; u32 m, m x u16` (most likely quest ids given and taken). The server copy has Korean names; the client has its own copy.

Count per type in this file: 1: 301, 2: 69, 3: 50, 4: 2, 5: 8, 6: 39, 7: 662, 8: 130, 9: 20, 10: 83, 11: 6, 12: 7, 13: 2. Community names for the types
(not verified here): 1 merchant, 2 gatekeeper, 3 blacksmith, 4 pet, 5 gambling, 6 warehouse, 7 ordinary, 8 guard, 9 animal, 10 apprentice, 11 guild
master, 12 dead NPC, 13 combat commander.

`CNpcData::GetNpc` (0x41d640) looks an NPC up by type (1 to 13) and **index**: type id `i` is the `i`-th record of that list (no id comparison).

## 4. Creating the map's NPCs

Confirmed, `CMap::InitNPC` 0x429130.

For each NPC entry of the map file, in order: skipped if its type is 5 (the new server places them when `mini_game` is on, the default: the gambling house,
`docs/original/mini-game.md`), if `GetNpc` finds nothing, if it has no position, or if the first position fails
`IsSafePos`. Otherwise **one NPC at the first position** with the first direction in degrees, **truncated** to a whole number (`fistp` with the
rounding mode set to chop, 0x4292f1), a new object id from the object manager's counter (`InterlockedIncrement`, 0x429288; kept when the heap reuses an
object), type and type id from the entry. The other positions are the NPC's walk (every NPC with two or more walks, `docs/original/guards.md`); a guard's first position is
also checked against the blocked cells (a blocked guard is never updated; logged "npc guard move block"). `CZone::NpcCreate` (used by a staff command) does the same for one NPC with direction 0.

With the shipped files: **1,964 of the 2,028 entries** of all 77 files create an NPC (map 1: 284, map 2: 233, map 0: 122, the capitals about 131
each); the others are type 5 or ids the NPC file does not have. On the 68 maps `Map.ini` lists, **1,724 of 1,774** (6 of type 5, 44 undefined ids, none
off the map), the number the new game server logs at start-up.

## 5. An NPC in the view: `0x0E01`

Confirmed, `CZone::EnterNpc` 0x416d70, `CCell::SendCellInfoEnter` 0x423f9d.

23 bytes: `u32 object id, u8 npc type, u16 npc type id, f32 x, y, z, u16 direction` (degrees). Sent to the view when an NPC is placed, and to a player for
every NPC in the cells it enters or that come into its view. The cell's other lists send `0x0601` (monsters: `u32 id, u8 0, u16 mob id, f32 x, f32 z`,
17 bytes, not sent when dead), `0x0401` (22 bytes, `u32 id, u8 0, 3 bytes, f32 x, y, z`; most likely ground items), `0x0403` and `0x0405`
(not identified). Leaving the view (`CCell::SendCellInfoLeave` 0x424130), 6 bytes each (`u32 id`), per list in this order: `0x0202` players,
`0x0406` (the `0x0405` list), `0x0602` monsters, **`0x0E02` NPCs**, `0x0402` (the `0x0401` list).

## 6. Where the new server gets this data (and what it does with it)

`tools/world/import.py <PS_Game Data folder>` reads `Map.ini`, the svmaps and `NpcQuest.SData` into the PostgreSQL schema `world`
(`db/world/schema.sql`); the game server reads only that schema. Rows can be edited directly; running the import again (for example
after editing an svmap with a community tool) replaces the whole schema. Like `defs`, it is proprietary game data and not a migration.

At start-up the game server (`crates/game-server/src/world.rs`) applies section 4's rules to the rows and numbers the NPCs from 1 in map and file order.
Players get `0x0E01` for the NPCs in their 5 x 5 cells on entering the zone (after all the players rather than cell by cell as the original, before the weather) and, when they change cells, `0x0E02`
for the NPCs that left the view and `0x0E01` for those that came into it. The client agrees on both sizes (23 and 6 bytes, `docs/client`).

## 7. Open items

- Walking NPCs, guards, `0x0E03`/`0x0E05`/`0x0E06` and the client's `0x0E04`: `docs/original/guards.md`. Regions and weather timing
  are still open (shops, gatekeepers, quests, monsters, portals and obelisks have their own documents).

## 8. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
and their map files, `Map.ini` and `NpcQuest.SData`, read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md)
and [`ep5.4.md`](ep5.4.md). Function names in those builds are carried over from EP4.5 (matched by strings or the call graph, as those
pages list).

### EP3

- `MapCount = 45`: maps 0 to 44. 30 map files are byte-identical to EP4.5's; the others mostly have fewer NPCs and portals, and map 42
  is a different map (another declared size; 8 NPCs, no portals) (confirmed, the files).
- `CWorld::InitMaps` (EP3 ps_game 0x411570) is the same code with a 0x1c4-byte map object (EP4.5 0x1f0); `LoadMap` (EP3 ps_game
  0x421490), `LoadGameMapConfig`, `IsSafePos` and `IsBlock` are identical or nearly so (confirmed).
- `NpcQuest.SData` parses with the EP4.5 layout (text in Shift-JIS): 1,116 NPCs against 1,379 (confirmed). `CMap::InitNPC` was not
  compared.

### EP5.4

- **Maps** (confirmed, files and `Map.ini`): `MapCount` 68 → 80, new maps 68 to 79 (the level 60 to 75 fields, a shared relic field 70,
  dungeons), and files 80, 98 and 99 that lie past the count and are not loaded (inferred). Every file parses with the EP4.5 layout. Map
  60 is rebuilt as a small town (86 NPCs, no monsters, no portal); map 62's blocked-cell bitmap is empty. Maps 73 and 75 to 79 spawn
  monster ids that have no definition.
- **No fixed map limit**: `InitMaps` (EP5.4 ps_game 0x41ab20) allocates `MapCount` maps, now 0x208 bytes each (confirmed).
- **`Map.ini`**: the same keys in the same order (`LoadConfig`, EP5.4 ps_game 0x42d3e0), plus `MapCurse` (a country, a skill and two
  bytes: that country's players get the skill on entering, cannot save a position there and are moved off the map at login) and
  `[DEFAULT_SET_ZONE] ReBirthAddSkillID`/`LV` (a skill after a full rebirth) (confirmed). No shipped file sets either. The "map 42 is
  the auction house" test becomes a per-map flag set for map 42 (EP5.4 ps_game 0x42db4b), same behaviour.
- **NPCs**: `CMap::InitNPC` (EP5.4 ps_game 0x42fb50) no longer skips type 5, so the eight gambling NPCs (maps 1, 2, 19, 20, 28, 29, 35,
  36) are placed; it skips merchants whose shop kind is 7 or 8 instead (EP5.4 ps_game 0x42fbbc to 0x42fbd1), the eight cloak sellers of
  maps 35 and 36 (confirmed; why, not found). `NpcQuest.SData` has 1,524 NPCs (145 new, text in GBK).
