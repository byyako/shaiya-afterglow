# Monsters: spawning, respawning, the view and the info queries

This page covers monsters in the EP4.5 game server `ps_game.exe` up to the point where they fight: their definitions,
spawn areas, respawning, how they appear to players, the client's info queries, and walking about. It comes from static analysis; nothing
was executed. The client's side is on [World packets](../client/client-world-packets.md) ("Mobs"). Fighting is on
[Monsters fighting back](mob-fight.md).

## In short

- **Respawn times come from a code** hidden in an unused attack column of the monster table: from 5 seconds up to 7 days. Most monsters
  come back within 3 minutes; bosses take hours or days.
- **Monsters spawn in boxes** drawn in the map files. Each box tops up one monster per entry per respawn time, at a random spot that is
  not blocked.
- **Idle monsters wander**: every 14 to 44 seconds they take a short step in a random direction, never more than 10 units from home. A
  monster pulled more than 10 units away walks home.
- **Faction.** Most monsters are neutral; 42 each belong to Light and to Fury (the goddesses and the like) and only the other side can
  attack them.

Status words: **confirmed** = read in the code (address given).

## 1. Monster definitions

Confirmed, `CGameData::LoadConstMob` 0x4c1720, `stMOBInfo` 268 bytes.

Read from `usp_Read_Mobs_R` (the `Mobs` table, `defs.mobs`, selected in table order) into `stMOBInfo`. Each attack entry is bound column by
column as `byType` (`AttackType`), `dwTime`, `byRange`, `wDamage` (`Attack`), `wDamageAdd` (`AttackPlus`), `byAttrib`, `bySpecial`, `byOK`
(`AttackOk`, at entry `+0x02`), so the third entry's last two bytes are **`+0xb6` = `Attackattrib3` and `+0xb7` = `Attackspecial3`**.
Fields used here: `wID`, `wLevel`, `dwHP` (`+0x30`), and these, which are not columns of their own but taken from the third attack entry:

- **`dwRespawnTime` (`+0x6c`)**: `Attackspecial3` (`+0xb7`) as a code, 0 to 15 (above 15 counts as 0):

| code | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| respawn | 5 s | 30 s | 1 min | 3 min | 5 min | 10 min | 15 min | 30 min | 1 h | 4 h | 12 h | 24 h | 72 h | 2 h | 0 | 7 days |

  (jump table 0x4c2078; any other value 15 s). Shipped (all 2,747 definitions): code 0 for 1,293, 1 for 171, 2 for 567, 3 for 426, 4 for 40,
  5 for 12, 6 for 5, 7 for 29, 8 for 44, 9 for 10, 10 for 24, 11 for 30, 12 for 54, 14 for 6, 15 for 36 (the level-69 "immortal" boss: 7 days).
- **`nCountry` (`+0x78`)**: `Attackattrib3` 1 → 0 (light), 2 → 1 (fury), anything else → 2 (both); 42 monsters each are light and fury (the
  goddesses and the like).
- **`dwDefenseAddSH` (`+0x70`)** = `Attacktime3`, **`nRecoverValue` (`+0x74`)** = `Attack3`, **`bObelisk` (`+0x7c`)** = `Attackplus3` ≠ 0.

(Correction: an earlier version of this document named `Attackok3` as the respawn code; the binding order above shows it is `Attackspecial3`.)

## 2. Spawn areas

Confirmed, `CMap::LoadMap` 0x425780 and `CZone::MapInit` 0x41c8a0.

Each area of the map file (`docs/original/maps-npcs.md`) becomes an `stMapMob`: the box (`x1, y1, z1, x2, y2, z2` as in the file),
**`nDeltaX = trunc(x2 - x1)`, `nDeltaZ = trunc(z2 - z1)`** (at least 1), and its entries as `stMobRespawn`: `nType` (monster id), `nCount`
(how many), `nCurrCount` (0), `dwRespawnTime` (0), `pMobInfo`: the definition, or none when the id is out of range or the definition has id 0,
HP 0 or level 0 (such entries never spawn). The zone keeps the areas that have entries.

## 3. Respawning

Confirmed, `CZone::UpdateMobRespawn` 0x41d240, every zone update with the current tick.

For every entry with a definition, `nCurrCount < nCount` and `dwRespawnTime <= now`: a position **`x = x1 + rand() % nDeltaX`,
`z = z1 + rand() % nDeltaZ`** (whole numbers), `y = y1`. If it is on the map and its cell is not blocked (the map file's bitmap), a monster
is created there: a new object id from the monsters' own counter (the object manager's `+0x08`; NPCs use `+0x0c`), current HP = `dwHP`,
attack speed 3, move speed 2 (`CMob::Init` 0x490040), `dwRespawnTime = now + the definition's respawn time`, `nCurrCount + 1`; then it
enters the zone. Otherwise nothing happens and the entry tries again on the next update. So **an area fills one monster per entry per
respawn time** (at start-up too: everything is due at time 0).

## 4. In the view

Confirmed.

- **Entering** (`CZone::EnterMob` 0x416860): the monster goes into its cell, and everyone in view gets **`0x0601`** (`TP_MOB_ENTER`, 17 bytes):
  `u32 id, u8 byNew` (1 here), `u16 monster id, f32 x, f32 z`.
- **A player's view** (`CCell::SendCellInfoEnter`): per cell, after the players and the `0x0405` list, each living monster as `0x0601` with
  `byNew` 0; leaving the view (`SendCellInfoLeave`), **`0x0602`** (`u32 id`), after the players and `0x0406`, before the NPCs.

## 5. The info queries

Confirmed, `CUser::PacketGetInfo` 0x469cf0.

Client → server, 6 bytes each: `u32 id`. No answer when the id is not in the player's zone.

| request | answer |
|---|---|
| `0x0302` a player's HP | `0x0302`, 10 bytes: `u32 id, u16 max HP, u16 HP` |
| `0x0304` a monster | `0x0603` (15 bytes: `u32 id, u8 1, f32 x, f32 z`, its position), then `0x0304`, 12 bytes: `u32 id, u32 HP, u8 attack speed, u8 move speed` |
| `0x0305` a monster's HP | `0x0305`, 10 bytes: `u32 id, u32 HP` |

Monster speeds (`CMob::GetAttackSpeed`/`GetMoveSpeed`): attack clamped to 0..6, move to 0..4, 255 while stunned, asleep or the like.

## 6. Walking about

Confirmed, `CZone::UpdateMobIdle` 0x421a00, `CMob::UpdateIdle` 0x4a7e60, `CZone::MoveMob` 0x4169d0.

The zone update (0x4075a0) runs the monsters' attack pass every 500 ms and, every 1000 ms, `UpdateMobIdle`: `CMob::UpdateIdle(now)` for
every monster, then `UpdateMobDeath`, then `UpdateMobRespawn` (section 3). Times are `GetTickCount` milliseconds.

**At creation** (`CMob::Init`): state 0 (idle), next move time `now + rand() % 60000` (the C library's `rand()` gives 0 to 32767, so
at most 32.8 s), next new-target time `now + 5000`. The home position
(`m_vPosRespawn`) is where it entered the zone (`CZone::EnterMob`).

**Scripted monsters:** a definition whose `byAI` (column `AI`) is 10 to 99 gets a state machine from `Data/mob_ai.ini` and `Data/AI/*`
(`CAIFactory::SetFSM`; on success the state is 100), 100 and above a Lua one (`CMob::InitLuaFSM`). Those run their scripts instead of what
follows: `docs/original/scripted-mobs.md` (93 of the shipped definitions: 65 of them in spawn areas, on maps 45 to 47, and 16 bosses in the
boss groups of maps 48 and 49). What the numbers 0 to 9 change
in the code below and in the fight (aggressive, calls for help, standing still) is on [Monster AI 0 to 9](mob-ai.md).

**`CMob::UpdateIdle(now)`** for the others, only in state 0 (idle):

1. Looking for a target (`CMob::Targeting`, aggressive monsters): when the next new-target time is set and not yet reached it is skipped,
   otherwise the time is cleared and `Targeting` runs; when it finds someone the update ends (fighting: not analysed yet).
2. Obelisk monsters (`bObelisk`, set for the obelisk guards of `Obelisk.ini`) never walk. Nothing more happens before the next move time.
3. Away from home: with a chase path left and more than 0.5 from home, or more than **10** from home (horizontal distance,
   `SVector::Distance2`), the state becomes 3 (going home, `CMob::ReturnHome`, with fighting) and the update ends.
4. A monster that cannot walk — `byAI` 4, move speed 255 (stopped, stunned or asleep), `byChaseStep` 0 or `byNormalStep` 0 — only sets its
   next move time to **`now + 10000 + rand() % 30000`**.
5. Otherwise one step:
   - `a = rand() % 10000` (degrees), `r = (rand() % 500) / 100` (whole number, 0 to 4);
   - length **`L = g_ConstMobMoveSpeed[move speed] + byNormalStep + r`**, with `g_ConstMobMoveSpeed` = `{-4, -2, 0, 2, 4}` (0x542538) for
     move speeds 0 to 4 (the speed clamped to 0..4; a new monster has 2, so `L = byNormalStep + r`);
   - the vector `(0, 0, L)` turned about y by `a × π / 180` (`SVector::RotateY`: `x' = x cos a - z sin a`, `z' = z cos a + x sin a`), so
     the target is **`x - L sin a`, `y`, `z + L cos a`** from the current position;
   - the step is taken only when the target is **less than 10 from home** and the straight line to it is free (`CMap::EnableLine` 0x42ab80,
     below); then `CZone::MoveMob(target, motion 0)`;
   - in every case the next move time becomes **`now + 10000 + dwNormalTime + rand() % 30000`** (column `NormalTime`; 4000 for most
     monsters, so a step every 14 to 44 seconds).

**`CMap::EnableLine(x1, z1, x2, z2)`**, all four the truncated coordinates: false when any is outside `0 ≤ v < map size`. Otherwise a line
walk over the block bitmap (`CMap::IsBlock`, the map file's bitmap as in section 3): along the longer axis (z when `|dx| = |dz|`) `n =
max(|dx|, |dz|)` steps of one unit (`sx`, `sz` = the signs, +1 for 0); the other coordinate starts at its start value plus the slope
`|d_other| / |d_long| × s_other` and gains the slope each step, and is truncated for the lookup. Each of the `n` cells after the start, the
end included, must be free; the first blocked one makes it false. (With `dx = dz = 0` there is nothing to check: true.)

**`CZone::MoveMob(x, y, z, motion)`:** false, and no move, when the position is not on the map (`CMap::IsSafePos`). When the monster leaves
its cell's widened bounds (`CCell::IsInnerCellView`, as for players) and the cell `(trunc(x / 40), trunc(z / 40))` is a different one inside
the grid, it changes cells (`CZone::SendMoveMob`): the players in cells of the old view that are not in the new view get `0x0602`, those
in cells of the new view not in the old one `0x0601` with `byNew` 0 (both before the position changes, so with the old position). Then
the position is set and **`0x0603`** (`TP_MOB_MOVE`, 15 bytes: `u32 id, u8 motion, f32 x, f32 z`; motion 0 = walk) goes to every player in
the 5 × 5 view of the (new) cell (`CZone::PSendView` → `CCell::PSendAll`, no distance limit).

## 7. The new server

- Monster definitions from `defs.mobs` with the respawn code of section 1; spawn areas from `world.mob_areas` and `world.mob_area_mobs`; the
  blocked bitmap from `world.maps`.
- A pass every second for every map (`crates/game-server/src/mobs.rs`): walking (section 6) and then respawning (section 3); monsters in
  the zone's cells, sent as in section 4; the queries of section 5.
- Scripted monsters: `docs/original/scripted-mobs.md` section 12; the Lua ones (`AI` 100 and above): `docs/original/instances.md` section 18.
- Players attack, kill and get experience from monsters (`docs/original/combat.md`); monsters fight back, chase, look for targets and go
  home (`docs/original/mob-fight.md`).
- Built: monster skills (`docs/original/mob-skills.md` section 11), "both" monsters (country 2 never matches a player's, so they fight
  both sides: `Zones::attackable`, `zone::targetable`), instance zones (`docs/original/instances.md` section 18).

## 8. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
and their monster data, read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).
Function names in those builds are carried over from EP4.5 (matched by strings, constants or the call graph, as those pages list).

### EP3

- `CGameData::LoadConstMob` (EP3 ps_game 0x4991f0) reads the same 35 columns into a 228-byte record (EP4.5 268); the added field was not
  identified (confirmed).
- `CMob::SendSpeed` (EP3 ps_game 0x4745f0) leaves the monster id out of the speed packet, as EP4.5 does (confirmed).
- Spawning, respawning, the view and the info queries were not compared.

### EP5.4

- `LoadConstMob` (EP5.4 ps_game 0x4d9da0): a quest item id above 3000 now maps to item type 99 ([`quest-drops.md`](quest-drops.md))
  (confirmed, EP5.4 ps_game 0x4da047). `CMob::SendSpeed` (EP5.4 ps_game 0x4a1050) still has no id (confirmed).
- **Data** (confirmed, `PS_GameDefs`): 341 new monster ids (2747 to 3087, up to level 75) and 223 placeholders filled in; **no EP4.5
  monster with HP changed any column**. Maps 73 and 75 to 79 spawn ids above 3087, which section 1 never spawns (inferred).
- **Item summons**: `MobCreate.ini` (new, loaded by EP5.4 ps_game 0x4dcf10, named by its own error strings) defines weighted monster
  groups per set; an item with `bySpecial` 73 summons set `shVG` through the picker EP5.4 ps_game 0x4727d0 (maps below 200, an optional
  map list) (confirmed), and a new operator command `0xF70F` uses the same sets (inferred from the strings). The shipped sets name
  monsters without HP or missing sets, so nothing spawns from them (inferred). How the summon places the monsters was not followed.
- A player can subscribe to a monster target's buffs (`0x0309`, [`skills-buffs.md`](skills-buffs.md)).
