# Obelisks

This page covers the obelisks of the EP4.5 game server `ps_game.exe`: the obelisks of `Data/Obelisk.ini`, their
guards, who owns them, what a kill of one does (owner, packets, the faction kill count), when they come back, the reset at a faction's
top kill-count grade, what is saved, and the portal links ("roads") the file format allows. It was read in the Ghidra decompiler and the
disassembly with the PDB names and types: `CMap::LoadObelisk` 0x427300, `InitObelisk` 0x429a60, `UpdateRespawnObelisk` 0x429da0,
`ObeliskRespawn` 0x42a6f0, `ObeliskDestroy` 0x42a410, `ObeliskReset` 0x42aa40, `SendObeliskList` 0x42b040, `SendObeliskChange` 0x42b200,
`SendDBObeliskChange` 0x42b260, `SendDBObeliskList` 0x42b370, `UpdatePortalLink` 0x42a000, `EnableMoveByPortal` 0x4299b0,
`CWorld::SetKeepObelisk` 0x413bd0, and the callers: `CMob::Death` 0x4910f0, `CZone::UpdateMobDeath` 0x421a80, `CZone::MobRemoveByID`
0x41e910, `CZone::UpdateMobRespawn` 0x41d240, `CZone::MapInit` 0x41c8a0, `CZone::EnterUser` 0x415f80, `CWorld::ApplyKillCount` 0x4139c0,
`CWorldThread::UpdateKillCount` 0x407c30, `InitServer`. Layouts from the PDB: `stObeliskZone`, `stObelisk`, `stObeliskMob` (the file),
`stMapObelisk` (0x70), `stMapObeliskMob` (0x58), `stMobRespawn` (0x24), `stMapPortalLink` (0x84), `CMap` (`m_bObeliskResetSend`
`+0x1e4`, `m_nNumObelisk` `+0x1e8`, `m_pObelisk` `+0x1ec`). The client side is on the client's [Obelisks and roads](../client/obelisks.md)
page.

The same file's boss groups (`[bothmob_…]`) and the operator's removal of an obelisk or a boss (`/mera`) are [below](#1-the-file).
Not covered here: game logs (`0x0A02`, `0x0A03`, `0x0A04` to the log server; see [The game log](game-log.md)).

## In short

- **Obelisks are monsters that a faction can capture.** The shipped data has nine: seven on map 0 and one each on maps 18 and 30. Each
  is guarded by monsters of its owner's faction.
- **Killing an obelisk captures it** for the killer's faction. Its guards vanish, every player in the world is told, and about 3
  minutes later it comes back as the new owner's obelisk with the new owner's guards.
- **Captures move the faction kill count.** A capture adds 1366 to the capturing faction's count and takes the same from the faction
  that lost it.
- **Reset.** When a faction reaches the top kill-count grade, every obelisk goes back to its original owner and stays down for 10
  minutes.
- **Owners survive a restart**: they are saved in the database.
- **Roads (portals that open for the faction holding certain obelisks)** exist in the code, but the shipped file defines none. Its notes
  show they were meant for map 0's four exits to Stable Erde and Deep Desert.
- **The same file runs 25 boss chains**: the dragons of maps 19 and 20, the Cloron and Fantasma bosses, the Cryptic One's doors, the
  guild ranking battle's three bosses, and two more. Killing a chain's last boss opens its portal; on a plain map that opens it **for
  everyone on the server** until the chain starts again.

Status words: **confirmed** = read in the code (address given). Countries: the file writes **0 neutral, 1 light, 2 fury**; the server
keeps **0 light, 1 fury, 2 neutral** (`nCountry`, as a player's country); the packets carry **0 neutral, 1 light, 2 fury** again
(`stMapObelisk::GetCountry`: 0 → 1, 1 → 2, else 0).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names and types), read in the Ghidra decompiler (REA) and the disassembly (`tools/disasm.py`):
`CMap::LoadObelisk` 0x427300, `InitObelisk` 0x429a60, `UpdateRespawnObelisk` 0x429da0, `ObeliskRespawn` 0x42a6f0, `ObeliskDestroy`
0x42a410, `ObeliskReset` 0x42aa40, `SendObeliskList` 0x42b040, `SendObeliskChange` 0x42b200, `SendDBObeliskChange` 0x42b260,
`SendDBObeliskList` 0x42b370, `UpdatePortalLink` 0x42a000, `EnableMoveByPortal` 0x4299b0, `CWorld::SetKeepObelisk` 0x413bd0, and the
callers: `CMob::Death` 0x4910f0, `CZone::UpdateMobDeath` 0x421a80, `CZone::MobRemoveByID` 0x41e910, `CZone::UpdateMobRespawn`
0x41d240, `CZone::MapInit` 0x41c8a0, `CZone::EnterUser` 0x415f80, `CWorld::ApplyKillCount` 0x4139c0, `CWorldThread::UpdateKillCount`
0x407c30, `InitServer`. Layouts from the PDB: `stObeliskZone`, `stObelisk`, `stObeliskMob` (the file), `stMapObelisk` (0x70),
`stMapObeliskMob` (0x58), `stMobRespawn` (0x24), `stMapPortalLink` (0x84), `CMap` (`m_bObeliskResetSend` `+0x1e4`, `m_nNumObelisk`
`+0x1e8`, `m_pObelisk` `+0x1ec`). The client side is `docs/client/obelisks.md`.

Covered: the obelisks of `Data/Obelisk.ini`, their guards, who owns them, what a kill of one does (owner, packets, the faction kill
count), when they come back, the reset at a faction's top kill-count grade, what is saved; the boss spawns of the same file
(`[bothmob_…]`, 25 in the shipped file, section 11.3) and the operator's removal of an obelisk or a boss (`/mera`, `/mera t`, section
11.5), both built. **Not built**: portal links ("roads", sections 8 and 11.2; the shipped file defines none, and map 0's exits
stay as shipped by decision, section 11.7). The game log records `0x0A02` and `0x0A03` are written ([The game log](game-log.md)). **Section 11** lists
everything the loader reads, what a complete file holds, what ours lacks, and the decisions taken.

Countries: the file writes **0 neutral, 1 light, 2 fury**; the server keeps **0 light, 1 fury, 2 neutral** (`nCountry`, as a player's
country); the packets carry **0 neutral, 1 light, 2 fury** again (`stMapObelisk::GetCountry`: 0 → 1, 1 → 2, else 0).

## 1. The file

Confirmed, `CMap::LoadObelisk`, from `CWorld::InitMaps`.

`data/Obelisk.ini`, read line by line (`fgets`, 8191 bytes): leading spaces and tabs skipped; lines starting `//` or `--` skipped; the
rest split into tokens at any of ``␠ , = ( ) ] [ { } \t \r \n`` (`strtok`); numbers by `atol` / `atof` (a word reads as 0). Section
heads are matched without regard to case, and set the state:

- **`[zone_N]`**: state 1; a new zone, map **N**; no current obelisk.
- **`[Obelisk_K] = (x, y, z), (neutral, light, fury), c`**, only in state 1: a new obelisk of the zone: **base id = N × 65536 + K**,
  the position, the three monster ids, the base country `c` (file numbering). Room for 64 per zone.
- **`[Mob] = (x, y, z, radius), (neutral, light, fury), count`**, only in state 1 after an obelisk: a guard group of that obelisk. Room for 64.
- `[Portal_…` (state 3, section 8), `[bothmob_…` (state 4, scripted monsters): other states; `[light]`, `[fury]`, `[mob]`,
  `[RandomRespawn]` belong to those.

The shipped file: **map 0, obelisks 1 to 7** (1 to 5 neutral, 6 light, 7 fury; 16 to 24 guard groups each, many of them with
neutral id 0, so only there under a faction), **map 18, obelisk 1** and **map 30, obelisk 1** (neutral, 4 guard groups of 2). Its one `[Portal_` line is
the comment header `[Portal_그냥번호, (포탈있는 map번호, 맵에찍힌 포탈 id) ]`, left uncommented; it reads as portal 0 of map 0 with no
obelisks and is dropped (section 8). After the obelisks come **25 `[bothmob_…]` boss sections** (section 11.3). `ObeliskWar.ini`
(maps 102 to 108) is not read by this build.

**Setting up** (after reading), per zone whose map exists:

- Each obelisk (`stMapObelisk`): the country mapped (1 → 0 light, 2 → 1 fury, else 2 neutral) is the **base country**, the **country**
  and the **last destroyer** (`nCountryLastDestroy`); **`dwID`** = the next of a counter starting at **1** (`CMap::m_dwObeliskID`), in
  file order (so map 0's obelisks are 1 to 7, map 18's is 8, map 30's is 9); **the monster by country**: `nMob[0]` = the file's light id,
  `nMob[1]` fury, `nMob[2]` neutral. A monster counts only when its definition exists with `wID`, `wLevel` and `dwHP` not 0; id 0
  never counts. The three obelisk monsters get **`bObelisk`** (`stMOBInfo +0x7c`; in the data they already have it,
  `Attackplus3` ≠ 0: they never walk, `docs/original/mobs.md`).
- **An obelisk off its map** (x or z below 0 or not below the map size): no monster counts, it never appears (but it keeps its id and is
  listed). Otherwise it counts towards **`m_nNumTotalObelisk`** and gets its guard groups: centre, radius, **`nRadius2x` = 2 ×
  trunc(radius)**, count, the monster by country (light, fury, neutral as above).
- **What a capture is worth** (`m_nObeliskKCStatusAdd[2]`), when there is any obelisk: per faction **its top grade's kills ÷ the total
  + 1** (integer division; `docs/original/pvp-2.md` section 8). The shipped data: 12288 ÷ 9 + 1 = **1366** for both.

## 2. State (PDB, verified uses)

Per obelisk: `dwID`; `nCountry`; `nCountryLastDestroy` (saved and loaded; **`UpdatePortalLink` counts it**, section 11.2: a road
follows who last took an obelisk, not who owns it now; an earlier reading said nothing else reads it); `nBaseID`; `nBaseCountry`; `vPos`; the
three monsters; **its own respawn entry** (`stMobRespawn`: `nType` = the obelisk's index, `nCount` 1, `bObelisk` set, a current count
and a due time); its guard groups; **`pObeliskMob`**, the obelisk's monster while it stands. `dwResetTime` and `dwChangeTime` only pass
through the database (section 7). Per guard group: centre, radius, `nRadius2x`, count, the three monsters, **a respawn entry**
(`nCount` = the count, `pObelisk` = the group) and **the list of its guards** (`nCount` slots).

## 3. Spawning

Confirmed.

**At start** (`CZone::MapInit` → `InitObelisk`, maps of create type 0 only), each obelisk whose monster for its country counts:
current count 1, due 0; **its monster at `vPos`** (no check of the spot); then **each guard group whose monster for the obelisk's
country counts: current count = the group's count, due 0, and `count` tries at a spot each**: x = `(rand() % nRadius2x) − radius +
centre x`, z the same with a second `rand()`, y = the centre's; the spot is taken when `round(x)`, `round(z)` are on the map and not
blocked (`CBitArray::GetBit`); a blocked spot is not tried again. Each guard taken goes into the group's list. (Deviation 1.)

**The respawn pass** (`UpdateRespawnObelisk`, after the ordinary entries of `CZone::UpdateMobRespawn`, every pass), for each obelisk
whose monster for its current country counts:

- **No obelisk standing** (current count below 1): when its due time has come, **`ObeliskRespawn`**: as at start, the monster of the
  **current country** and **all its guards at once** (each group's current count set to its count, due 0).
- **The obelisk standing**: each guard group whose monster for the country counts, **below its count and due**: one guard at a spot as
  above (none when the spot is blocked); current count + 1, **due = now + that monster's respawn time**; into the list.

**Deaths and bodies** (`CMob::Death`, `CZone::UpdateMobDeath`):

- **The obelisk** dies: section 4 at once; its entry's **current count − 1 and due = never** (0xFFFFFFFF). When its body goes (as any
  body: 10 s, or 5 with no respawn time; `0x0602` to its view), **due = now + the dead monster's respawn time** if the due time is
  past or never (no hour's spread; the obelisks' respawn code is 3: **3 minutes**). So it comes back about 3 min 10 s after its fall,
  as **the new owner's monster, with the new owner's guards**.
- **A guard** dies: it leaves its group's list; when its body goes, as an ordinary monster (a full entry is due again after the respawn
  time, with an hour's spread from code 9 on; current count − 1). Guards respawn only while their obelisk stands.

## 4. A fall: `CMap::ObeliskDestroy(mob, country, index, quiet)`

Confirmed.

From **`CMob::Death`** (not quiet): the country is **the killer's** (the player of record, `docs/original/combat.md` section 6, when
in the zone); with no such player (a monster's or its own kill, the player gone) it is **the other side of the current owner**
(light → fury, fury → light, neutral stays neutral). From **`CZone::MobRemoveByID`** (the reset, section 5; quiet): the **base
country**. In this order:

1. **The obelisk's guards are removed at once** (`MobRemoveByID` for every guard in the lists: `0x0602` to its view, no death; its entry
   counts down, due again after its respawn time when it was full), the lists emptied; `pObeliskMob` cleared.
2. **The new country**: the one given, except that **neutral gives the base country** (a neutral-based obelisk stays neutral; a light or
   fury base never becomes neutral).
3. **`0x2102`** (`u32 dwID, u8 state`, 7 bytes, the packet country) **to every player of the map** (`CZone::SendAll`), unless during a
   reset (`m_bObeliskResetSend`).
4. Not quiet only:
   - **The kill count**, when the new country is a faction: **`AddKillCount(new, worth[new])`** (`docs/original/pvp-2.md` section 9;
     nothing while counting is off or either faction is in its top grade's ten minutes; it may reach the top, section 5); then, when
     the new faction is not (now) in its top grade's ten minutes and the old country was a faction, **`SubKillCount(old,
     worth[old])`** (section 6). A neutral fall changes no count; the same owner taking it back (a kill with no player) adds again.
   - **`0x2103`** (`u16` the fallen monster's id, `u8` old, `u8` new, packet countries, 6 bytes) **to every player in the world**
     (`CWorld::SendAll`); the client picks the message by the viewer's faction (`docs/client/obelisks.md`).
5. Game log `0x0A03` (built, [The game log](game-log.md)).
6. **When the country changed**: the last destroyer = the new country when a faction; **`0x0207` to ps_dbAgent** (section 7);
   `UpdatePortalLink` (section 8).

The killing itself goes on as for any monster: experience, drops, quest counts, `0x0606`.

## 5. The reset

Confirmed, `CMap::ObeliskReset`, `CWorldThread::UpdateKillCount`, `CWorld::ApplyKillCount`.

**When a faction reaches its top kill-count grade** by a kill or a capture (`AddKillCount`) or the Goddess Blessing
(`AddKillCountGrade`), `ApplyKillCount(country, true)` marks every world thread; on its next pass each thread resets the obelisks of
**every map of create type 0**. (`SetKillCount`, the top's ten minutes running out and `SubKillCount` pass `false`;
`CWorldThread::SetObeliskReset` and `CZone::MapObeliskReset` exist but nothing calls them.) For every obelisk of the map:

1. **Its monster, if standing, is removed** (`m_bObeliskResetSend` set around it): `MobRemoveByID` → section 4 quiet with the base
   country (the guards removed, no `0x2102`, no count, no `0x2103`; the last destroyer and `0x0207` when it changed); its entry counts
   down.
2. **Due = now + 10 minutes** (also for an obelisk already down: it comes back 10 min from the reset, not earlier).
3. **Country = the base country**; last destroyer = it when a faction; **`0x2102` to the map**; **`0x0207`** (always).

Then `UpdatePortalLink` once for the map.

## 6. `CWorld::SubKillCount(country, n)`

Confirmed, 0x413750.

Nothing while counting is off (`g_bDisableKillCount`) or either faction is in its top grade's ten minutes. Otherwise **n is cut to the
count** and taken off. Then, when the grade is above 0 and **the count is below the kills of the grade under the current one**
(`GetKCStatus(country, grade − 1)`): the grade = the highest whose kills are at most the count, the next count = the next grade's,
`ApplyKillCount(country, false)` (the faction's players' numbers again, `0x020F`), send time = now + 10 min. No save (the next 10-minute
save takes it). (Deviation 2.)

## 7. Saving

Confirmed, `SendDBObeliskChange`, `SendDBObeliskList`, `SetKeepObelisk`, `CClientToDBAgent::PacketSystemConnect` 0x0206.

`0x0207` to ps_dbAgent, 11 bytes: `u32 nBaseID`, `u8 (last destroyer + 1) × 16 | country`, `u32 dwChangeTime`. At start, before the
zones (`InitServer` waits up to 5 minutes for ps_dbAgent's ready), the server sends its list (`0x0206`, `u8 n`, n × 9 bytes in the same
form) and ps_dbAgent answers `0x0206` with the saved rows; **`SetKeepObelisk`** sets each obelisk by base id: country = low four bits,
last destroyer = high four bits − 1 (the country when that is below 0 or the country is a faction), `dwChangeTime` as saved,
`dwResetTime` = (86400 − the seconds since then) × 1000 when a change time was saved. So **the owners survive a restart**, and the
obelisks spawn as their saved owners'. Nothing in the game sets or reads the two times.

## 8. Portal links

Confirmed; not built; in full in section 11.2.

A `[Portal_<n>, (<map>, <portal>)]` section with `[light] = (map, Obelisk_K), …` and `[fury] = …` (up to four each) links a map
portal to obelisks; the link is kept only when its map and portal exist and at least one obelisk resolves. `UpdatePortalLink` opens a
link's portal to a faction when that faction holds all its obelisks (the portal's country 0 both, 1 light, 2 fury, 3 none), sends
`0x2102` with 100 + state and `0x2104`; `EnableMoveByPortal` refuses a portal move or a teleport to the portal's map when its
country is not the player's. **The shipped file makes no link**, so none of this ever runs: every move is allowed, the `0x2101` list
has no 100–103 rows, `0x2104` is never sent (and the client prints nothing for it).

## 9. Entering a map

Confirmed, `CZone::EnterUser` → `SendObeliskList`.

After the weather (`docs/original/world.md` section 5.2), **`0x2101`** to the player when the map has obelisks (or links): `u8 n`,
n × 13 bytes (`u32 dwID, u8 state, f32 x, f32 z`; the state in packet countries), every obelisk of the map, standing or not, in
order; 100 rows per packet at most.

## 10. The new server

Built as above, with these deviations (both approved):

1. **A guard group counts the guards really there.** In the original a respawning obelisk sets each group's count to full whether or
   not every spot was free (a blocked spot leaves the group short until a guard dies), and a guard's body still lying when its obelisk
   comes back counts the group down once more when it goes (one guard too many is spawned later). Here a group is full when its list
   is; a blocked spot is tried again on the next pass as for an ordinary respawn; a body only counts down the guards of the obelisk it
   belonged to.
2. **A lost capture lowers the grade when the count falls below the current grade's kills** (`SubKillCount`). In the original it
   compares with the grade under the current one, so a faction keeps its grade until its count drops a further grade (likely an
   off-by-one).

How it is built: `tools/world/import.py` reads the obelisks and guard groups into `world.obelisks` and `world.obelisk_guards`; the zone
keeps each map's obelisks with their owners, entries and guard lists, spawns them with the map's monsters, and on an obelisk's death does
section 4; what the server must do with it (the counts, the save, a reset at the top) goes back to the server through the zone's queue.
The owners are saved in `obelisk_owners` (per server, by map and obelisk number: country and last destroyer) and loaded before the
monsters spawn. The boss groups (section 11.3) and the operator's removal of an obelisk or a boss (section 11.5) are built as described
there. Not built: section 8 and 11.2 (no links in the shipped file; left as shipped by decision, 11.7).

## 11. A complete `Obelisk.ini` (verified unless marked)

Read for this section: `CMap::LoadObelisk` 0x427300 (decompiled in full), `UpdatePortalLink` 0x42a000 (disassembly),
`EnableMoveByPortal` 0x4299b0, `SendPortalLinkChange` 0x42b2c0, `SendObeliskList` 0x42b040, `SendDBPortalLinkList` 0x42b460,
`FindPortalLink` 0x42b500, `CWorld::SetKeepObelisk` 0x413bd0, `CWorld::SendDBObeliskList` 0x413b90, `CZone::MapInit` 0x41c8a0 (the copy
at 0x41c9a5), `MapInitObject` 0x41c5f0, `MapInitBothMob` 0x41b470, `UpdateBothMobRespawn` 0x41b870, `BothMobDestroy` 0x41bad0,
`BothMobRespawn` 0x41be50, `UpdateInsZonePortalCountry` 0x417a10, `CMob::Death` 0x4910f0, `CZone::UpdateMobDeath` 0x421a80,
`CZone::MobRemove` 0x41e100, `CZone::MobRemoveByID` 0x41e910. PDB layouts: `stPortalLink` (0x4c, the file), `stPortalLinkObelistk`
(sic), `stMapPortalLink` (0x84), `stMapPortalLinkObelisk` (0xc), `stMapPortal` (0x30), `stMapBoth` (0x26cc), `stMapBothMob` (0x12c),
`stMobRespawn` (0x24), `stInsZonePortal`. The binary names one file only, `data/Obelisk.ini` (no other "obelisk" string).

### 11.1 Every key the loader reads

Lines, comments, tokens and numbers as in section 1 (`strtok` on ``␠ , = ( ) ] [ { } \t \r \n``, `atol`/`atof`, a word reads 0;
heads matched without case). **Text after a value is not a comment**: `//` only works at the start of a line. Five states: 0 before
anything, 1 `[zone_…]`, 3 `[Portal_…]`, 4 `[bothmob_…]`. A line that is no head of its state is ignored (any Korean note line).

| head | state | form and what it sets |
|---|---|---|
| `[zone_N]` | → 1 | a new zone, map N (section 1) |
| `[Obelisk_K] = (x,y,z),(neutral,light,fury),c` | 1 | an obelisk (section 1). **K is only used for the base id** (map × 65536 + K); a link names an obelisk by its place in the zone (11.2) |
| `[Mob] = (x,y,z,r),(neutral,light,fury),n` | 1 | a guard group of the last obelisk (section 1) |
| `[Portal_n, (map, portal)]` | → 3 | a **portal link**: base id **n + 0x10000000**, the map, the portal's **id** (not its place: `stMapPortal::nID`, which a portal has only when its map file's country is above 1000, `docs/original/portals.md` section 1). All three numbers are needed, or the head starts state 3 without a new link and the following `[light]`/`[fury]` lines go to **the previous link** (bug). Room for 64 (`stPortalLink[64]`, not checked: a 65th overruns) |
| `[light] = (map, Obelisk_K), …` | 3 | up to **four** obelisks that open the link to Light. Each pair: a map, then a word that must start `Obelisk_` and be longer than 8 characters (`Obelisk_` + number); the first pair that is not so ends the list |
| `[fury] = (map, Obelisk_K), …` | 3 | the same for Fury (the parser also skips 7 characters, one past `[fury]`: harmless before `=`, `(` or a space) |
| `[bothmob_n, (map, stages, respawn, (pmap, portal))] = (x,y,z),(id1, …)` | → 4 | a **boss group** of map `map` (11.3): number n, **stages** (cut to 15), the **respawn time in seconds**, a portal (map, id), the boss's spot, and exactly `stages` monster ids, one per stage; with fewer ids the group is dropped. Room for 64 maps of 64 groups (not checked) |
| `[Mob] = (x,y,z,r),(id1, …, id stages+1), n` | 4 | a **follower group** of the last boss group: centre, radius, one monster **per stage** and **one more**, the "item" monster (0 = none for that stage), count **n** (cut to 16). At most **32** groups; a short line is dropped. `[Mob(…)]` as in the file's own comment does **not** match (`[mob]` must be followed by `]`) |
| `[RandomRespawn] = (min, max),(x,y,z),(x,y,z),(x,y,z)` | 4 | the last boss group respawns at a **random** time (**min** minutes + a random part below **max** minutes) at one of **three** spots, chosen each time |

**Set-up of the links** (after every zone's obelisks, in file order): a link is kept when its map exists, the map has a portal with
that **id**, and at least one of its eight pairs resolves: the pair's map exists and **K − 1** is an index into **that map's obelisks
in file order**, `0 ≤ K − 1 ≤ m_nNumObelisk` (**off by one**: K = count + 1 is accepted and points past the array). Unresolved pairs are
dropped and the rest packed. A kept link gets the next **`dwID`** of the obelisks' counter (section 1: after map 30's obelisk 9, the
first link is **10**); a link whose portal exists but whose pairs all fail uses up an id and is not kept. Both `bOpen` flags start
false; the portal's country stays what its map file says.

**Set-up of the boss groups**: per map with a group, `CMap +0x1c` (count) and `+0x20` (`stMapBoth[]`). Every monster id is looked up
(it counts only with `wID`, `wLevel`, `dwHP` not 0, as for obelisks); the portal resolves only when its id is **at least 1001** and the
map has a portal with it (else none, the group still runs); the boss's own respawn entry has `nType` = the group's index, `nCount` 1,
**`bBothMob`**; each follower group's entry has `pBothMob` = the group. **Every zone of the map copies the table** (`CZone::MapInit`:
`CZone +0x270`, `+0x274`), static and instance zones alike, and runs `MapInitBothMob` from `MapInitObject`.

### 11.2 Portal links ("roads") at run time

**`UpdatePortalLink`** runs after a fall that **changed** an obelisk's country (`ObeliskDestroy` step 6) and once after a reset
(`ObeliskReset`); never at start. It ignores which map called it and goes through **every link**:

1. **Counts**, per side: the link's obelisks whose **`nCountryLastDestroy`** is that side (0 Light, 1 Fury). So a road follows who
   **last took** each obelisk: after a reset a neutral obelisk keeps its last destroyer (section 5), a faction-based one goes back to its
   base faction.
2. **One side only**: a link with any Light obelisk is judged on Light alone; only a link with **no** Light obelisk is judged on Fury.
   (A link with both lists can never open for Fury.)
3. **Hysteresis**: the side **opens** when it counts **all** its obelisks (and was closed), **closes** when it counts **none** (and was
   open); in between nothing changes. The portal's country (`stMapPortal::nCountry`, the one `0x020A` checks: 0 everyone, 1 Light, 2
   Fury, anything else nobody) moves so:

   | event | country before → after |
   |---|---|
   | Light opens | 0 or 2 → **0**; anything else (1, 3, an id above 1000) → **1** |
   | Light closes | 0 or 2 → **2**; anything else → **3** |
   | Fury opens | 0 or 1 → **0**; anything else (2, 3, an id) → **2** |
   | Fury closes | 0 or 1 → **1**; anything else → **3** |

   So a portal whose start country is a faction (1 or 2) is opened **to the other faction too** by a link of that other side, and
   falls back to its own faction when the link closes; a portal whose map-file country is its id (above 1000) starts **shut to all**.
4. **Pairs**: when link *i* opens for Light, link ***i* + 1** is closed (its first judged side, if open); when link *i* opens for Fury,
   link ***i* − 1** is. Only for links 0 to 11 (a 12-slot table). So the file is meant to list a road's **Light link and Fury link next
   to each other, Light first**: when one faction takes the road, the other loses it (inferred intent; the code is verified).
5. **When a portal's country changed**: `SendPortalLinkChange`: **`0x0207`** to ps_dbAgent (`u32` base id, `u8` the portal's country,
   `u32` 0; 11 bytes), **`0x2102`** (`u32` the link's `dwID`, `u8` 100 + state) to the link's map's zone; then **`0x2104`** (`u16` **the
   portal's id**, `u8` old, `u8` new) to **every player in the world**. States on the wire: country 0 → **103**, 1 → **101**, 2 →
   **102**, anything else → **100**.

**Entering a map** (`SendObeliskList`): after the obelisks, one `0x2101` row per link **of that map**: `dwID`, the state as above, the
portal's x and z.

**Saving**: at start `CWorld::SendDBObeliskList` first sends the links (`SendDBPortalLinkList`: `0x0206` rows of base id, state **3**,
time 0), then each map's obelisks; ps_dbAgent's answer goes through `SetKeepObelisk`, which for a base id with bit 0x10000000 finds
the link and sets **the portal's country to the saved byte** and the flags (0 → both open, 1 → Light, 2 → Fury, else none). So roads
survive a restart; `UpdatePortalLink` is not run after loading.

**`EnableMoveByPortal(map, country)`** (moves by item, `docs/original/item-use.md` section 9: `CUser::ItemUse` six calls, the party
summon in `PacketMaininterface`, the due move in `UpdateResetPosition`): among the links whose **portal leads to that map**
(`nMapID`), the move is allowed when any has country 0 or the player's faction; when **no** link leads there, always allowed. A map
reached only through roads is therefore closed to recall runes, the teleportation stone and summons while its roads are shut to the
player's faction. The portal use itself (`0x020A`) checks the portal's country only (`docs/original/portals.md` section 2); NPC
gatekeepers check nothing of this.

**Client** (`docs/client/obelisks.md`): it keeps link rows with obelisk rows; draws 101–103 on the minimap; on maps 68–70 (not in
EP4.5) turns 101/102/103 into a status icon; blocks nothing. **`0x2104` prints nothing** on the US client (it tests the new state
against 1010–1021, which a byte cannot reach); with the one-instruction fix of that document it prints, for **portal ids 1010, 1011,
1020, 1021**, "a road to Jungle of Light / Desert of Light / Jungle of Fury / Desert of Fury will be opened" (5374, 5373, 5372, 5371)
**on every change**, closing included.

### 11.3 Boss spawns (`[bothmob_…]`) at run time

- **Zone set-up** (`MapInitBothMob`), each group whose first stage's monster counts: **a fixed group** spawns its first-stage boss at
  once at its spot (no check of the spot), then each follower group with a first-stage monster: `count` tries at a random spot as for
  obelisk guards (a blocked spot is skipped; the group still counts as full, as deviation 1 of section 10 describes for guards). **A
  random group** spawns nothing: due = now + **min** minutes + (random below **max × 60**) seconds.
- **Respawn pass** (`UpdateBothMobRespawn`, every pass, for the current stage): no boss standing and due → `BothMobRespawn`; the boss
  standing → each follower group below its count and due gets one follower at a random spot (blocked: none), due again after **that
  monster's** respawn time.
- **A boss dies** (`CMob::Death`, killer or not) or **is removed** (`MobRemove`, `MobRemoveByID`, so `/mera` too): **`BothMobDestroy`**:
  every follower of every group removed (no death), the stage + 1. **When it was the last stage**: stage back to 0; **the portal opens to
  everyone** (its country 0, and the zone's pair `UpdateInsZonePortalCountry(id, 0)`, which only counts on maps with a `CreateType`);
  every group with an **item monster** gets `count` of them at random spots (the chests; never respawned).
- **Its body goes** (`UpdateMobDeath`): due = now + the dead monster's respawn time (code table, `docs/original/mobs.md` section 1).
  Then, **when the chain just ended** (stage 0): a fixed group: due = now + **the header's respawn time + (rand % 120 − 60) minutes**
  (± 1 hour); a random group: as at set-up. A removed boss has no body: its due time stays now + its own respawn time.
- **`BothMobRespawn`**: at stage 0 first **the portal shuts** (country **3**, the zone's pair 3) and every follower and chest still
  there is removed; then the stage's boss (a random group: at one of its three spots, `rand() % 3`) and that stage's followers (each
  group full at once).
- **A follower dies**: it leaves its group's list; its body counts the group down as any monster (due after its respawn time when full,
  an hour's spread from code 9 on). **A restart starts every chain afresh**: fixed bosses stand again at once.

Since the portal's country is the map's (`stMapPortal`), on a static map **killing the last stage opens the portal for everyone on the
server until the chain restarts**; on a map with a `CreateType` the pair is per zone.

**How it is built** (`docs/plans/done/obelisk-bosses.md`): `tools/world/import.py` reads the groups into `world.boss_groups` and
`world.boss_followers` (the limits and the dropped short lines as 11.1); `World::set_boss_groups` sets them up (`boss.rs`: monsters that
count, the portal found by id in the portal's map, a group whose first stage does not count left out; "boss groups loaded" at start).
Every zone of the map gets the chains (`Zones::new_mob_map`, static maps and the instance, guild house and ranking zones alike); they
run in the respawn pass, the death and the body passes (`zone/boss.rs`). A static map's portal country is kept by the zone as an
override of the map file's (`Zones::portal_country`, read by the portal use on a map without a `CreateType`); an instance zone's goes
to the zone's pair. Deviation 7 (followers count the ones really there). Two details of the build, no change in what a player sees: a
chain is set up at its zone's first respawn pass (a random group's first wait starts then), and **the chests of a last fall come at the
next respawn pass** (half a second later at most) rather than inside the death. The instance pairs (`UpdateInsZonePortalCountry`, from
a boss or from a Lua script) are written from inside the monsters' lock and applied when the zone's instance table is next taken, which
every reader of a pair does first. Info logs "boss rose", "boss fell", "boss due".

### 11.4 What we have, and what a complete file holds

**The copies**: `PS_Game/Data/Obelisk.ini` and `ObeliskWar.ini` in the server pack, in `PSM_Client/Bin/Data`, and both again in
`New Files/OmegaFiles`: the four `Obelisk.ini` are **identical** (MD5 `3591d330…`), the four `ObeliskWar.ini` too
(`1d0c4b4c…`). **A later-episode copy** (an English-translated community file found later; not kept in
the repository) agrees with ours wherever they overlap: maps 0, 18 and 30's obelisks and guards and boss groups 001 to 025 are the
same values, and it **still has no road link** (the same uncommented format line and the same 1010–1021 note). It adds obelisks for
**map 70** (three, monsters 2780–2891) and boss groups 026–074 for **maps 70, 71, 72, 74, 105 and 108** (monsters 2785–3072; one
chain of three stages, 3040 → 3041 → 3042); none of those maps or monsters exist in our data (maps end at 67, monsters at 2746). Its
new lines carry an extra trailing number (`…,(3001),1`) this build would ignore, and a few typos (a missing `)` or `=`, a repeated
`[bothmob_071]`) that the `strtok` parsing happens to survive. Nothing in it changes this section's findings or proposal. **`ObeliskWar.ini`** has the same header and road note, no link and no
boss group; its zones are 102 (map 18's obelisk), 103 (map 30's), 104 (map 0's seven with all guards) and 105 and 108 (two each:
2382 "the incarnated Goddess of Light", base 1, and 2383, Fury's, base 2, 50 and 45 million HP). Not read by this build; maps above 67
are not in the data.

**The shipped `Obelisk.ini` holds**: the obelisks and guards of maps 0, 18, 30 (sections 1–7), complete; the header comment documenting
every key (11.1, with the limits "changemobcount ≤ 15, mobcount ≤ 16, [Mob] ≤ 32" and "portal ids from 1001"); **one** `[Portal_` line,
the format note itself, left uncommented (no link); a note listing the **road ids**: "Light: 1010 = Stable Erde 47, 1011 = Deep Desert
45; Fury: 1020 = Stable Erde 47, 1021 = Deep Desert 45; the Goddess's guidance portals exist from 1010 to 1050"; and **25 boss
groups**:

| group | map | stages (ids) | followers (stage: id × n) | chests | header respawn | portal it opens |
|---|---|---|---|---|---|---|
| 001 (the file: "the dragons' counterattack") | 19 | 1974 → 1977 | 2: 1980 ×2, 1981 ×2, 1987 ×4, 1984 ×2 (no definition: none), 1985 ×2, 1986 ×4 | 1972 ×16 | 7 days | map 19 portal 8 (1001) → **map 48** |
| 002 | 20 | 1975 → 1978 | 2: 1990 ×2, 1991 ×2, 1992 ×4, 1993 ×2, 1994 ×2, 1995 ×4 | 1972 ×16 | 7 days | map 20 portal 7 (1002) → **map 49** |
| 003 (Cloron) | 14 | 871 | — | — | 12 h | map 14 portal 1 (1003) → the inner hall (496.9, 464.9) |
| 004 (Cloron) | 14 | 872 | 1: 873 ×2, 874 ×2, 875 ×4 | 1970 ×10 | 3 days | (14, 1005): no such portal |
| 005 (Fantasma) | 17 | 901 | — | — | 12 h | map 17 portal 1 (1004) → the inner hall (387.0, 438.1) |
| 006 (Fantasma) | 17 | 902 | 1: 903 ×2, 904 ×2, 905 ×4 | 1971 ×11 | 3 days | (17, 1005): no such portal |
| 007–013 (Cryptic One, Light) | 48 | 1704; 1706; 1702; 1701; 1714 → 1703; 1705; 1700 → 1716 | — | — | 7 days each | map 48 portals 1001–1007 (the inner doors) |
| 014–020 (Cryptic One, Fury) | 49 | the same, 18xx | — | — | 7 days each | map 49 portals 1001–1007 |
| 021–023 (guild ranking) | 50 | 1855; 1853; 1851 | — | — | 7 days | the zone's pairs 1003, 1001, 1002 (map 50 portals 3, 4, 5) |
| 024 ("Oblivio random") | 64 | 2414 | — | — | random: 45 min + up to 60 h, 3 spots | (100, 2000): none |
| 025 (Cailum Sacra) | 67 | **2469** (its script then makes 2470 and 2472, `docs/original/instances.md` section 16) | — | — | 15 h | (100, 2000): none |

The first-stage bosses 1974, 1975, 1714 and 1700 have respawn code 14 (0 s), so the second stage stands about 5 s after the first
falls. **On our server none of these monsters appears** (no spawn area places them; checked in `world.mob_area_mobs`), and the
portals 1001–1007 of maps 14, 17, 19, 20, 48, 49 (static) and 50 stay shut to all: **maps 48 and 49 cannot be reached at all**
(their only ways in are map 19 portal 8 and map 20 portal 7), nor Cloron's and Fantasma's inner halls. (Earlier documents said the
file had no boss groups; corrected in `docs/original/instances.md` section 7 and `docs/original/guild-ranking.md` section 5.)

**What is missing for roads**: (a) `[Portal_…]` links with `[light]`/`[fury]` lists, and (b) **portals that carry the road ids**: no
map file has a portal with country 1010–1050 (the 23 portals above 1000 are all 1001–1007, the boss doors). The only ways into maps 45
(Deep Desert 1) and 47 (Stable Erde) are map 0's portals **9** (Light, → 47, light side (120.1, 453.1)), **8** (Light, → 45 (924.8,
866.1)), **7** (Fury, → 47, fury side (570.3, 189.5)) and **6** (Fury, → 45 (859.8, 307.9)), plain faction portals (countries 1, 1, 2,
2); map 46 is reached from 45. Those four match the road note (Light's two roads and Fury's two, to 47 and 45) and the client's four
road lines. So the roads were meant for **map 0's four exits**, and the build ships them unlinked.

### 11.5 The `/mera` bug (ours; fixed)

The original removes an obelisk's monster with the quiet fall: `/mera` (`CZone::MobRemove`) calls **`ObeliskDestroy(mob, its current
country, quiet)`**, `/mera t` and the reset (`CZone::MobRemoveByID`) **`ObeliskDestroy(mob, its base country, quiet)`**. Before that
the entry counts down with **due = now + the monster's respawn time** (3 minutes) when it was full. So the guards go at once, `0x2102`
goes to the map, no kill count and no `0x2103`; `/mera t` hands a captured obelisk back to its base (with the last destroyer, the save
and `UpdatePortalLink` when that changed it); it rises again **3 minutes later** for that owner with its guards. On a boss,
both removals run `BothMobDestroy` (11.3).

Ours (`zone/admin.rs` `kill_quietly`): the monster only gets `dead_until = now`. The obelisk stays `Phase::Standing(id)`; when the body
goes, `obelisk_body` waits for `Phase::Fallen` and does nothing; so **the obelisk never rises again** until a top-grade reset, its
guards are not removed and **keep respawning** around the empty spot (the `Standing` branch of `obelisk_respawn`), and `0x2101` still
lists it with its owner. A guard removed with `/mera` stays in its group's `alive` list (harmless: its body counts the group down).
**Fix** (the original, no deviation): `/mera` → `obelisk_fall(…, current country, quiet)`, `/mera t` → `obelisk_fall(…, base, quiet)`,
then `Phase::Due(now + respawn)`; the quiet fall sends `0x2102` to the map and, when the country changed (only `/mera t`), queues the
owner's save (and the road update once roads exist); a guard leaves its list.

**Fixed** as above (`zone/admin.rs` `remove_quietly`, `zone/obelisk.rs` `obelisk_removed`): the fall has three kinds, a kill (all of
section 4), a removal (`0x2102` and the owner's save when it changed) and the reset (nothing told or saved there); on a boss both
commands run `BothMobDestroy` with the boss marked removed (11.3).

### 11.6 A proposed file for our maps

Everything below in `[zone_…]` and `[bothmob_…]` is **the shipped data, unchanged** (sections 1–7, 11.3); everything marked **ours**
is new design. Two ways to use the four map-0 exits as roads (a design choice):

- **A. Roads to one's own fields** (the literal reading of the note): each exit becomes a road of its own faction, shut until that
  faction holds its obelisks. Then **maps 45, 46 and 47 are closed to a faction by default** (no other way in; `EnableMoveByPortal` also
  refuses recall runes, the teleportation stone and summons into 45 and 47), and they open only by capture: a large change to how
  the game plays now.
- **B. Invasion roads** (recommended): each exit **stays open to its own faction** and becomes a road **for the other faction**, which
  opens when the attackers hold the obelisks named for it. 11.2's table already does this for a portal whose country is a faction (Light
  opening a Fury portal: 2 → 0; closing: 0 → 2). Item moves are never refused (each map keeps an exit open to each faction). Holding the
  centre and the enemy's base obelisk lets a faction walk into the enemy's half of Stable Erde; holding the two outer corners too, into
  Deep Desert. The client's lines fit: opening portal 9 (id 1010) to Fury says "a road to Jungle of Light will be opened".

Proposed file, B (**ours**; links listed Light first in each pair, as 11.2 step 4 wants; comments on their own lines, 11.1):

```ini
// ours: map 0's exits as invasion roads. The portals' ids and start countries come from our import (deviation 3), not
// from 0.svmap: portal 7 = 1020 (start 2), portal 9 = 1010 (start 1), portal 6 = 1021 (start 2), portal 8 = 1011 (start 1).

// ours, road 1 (pair with road 2): Fury's exit to Stable Erde opens to Light
[Portal_1, (0, 1020)]
    [light] = (0, Obelisk_007), (0, Obelisk_005)
// ours, road 2: Light's exit to Stable Erde opens to Fury
[Portal_2, (0, 1010)]
    [fury] = (0, Obelisk_006), (0, Obelisk_005)
// ours, road 3 (pair with road 4): Fury's exit to Deep Desert opens to Light
[Portal_3, (0, 1021)]
    [light] = (0, Obelisk_007), (0, Obelisk_001), (0, Obelisk_004)
// ours, road 4: Light's exit to Deep Desert opens to Fury
[Portal_4, (0, 1011)]
    [fury] = (0, Obelisk_006), (0, Obelisk_001), (0, Obelisk_004)

// shipped, unchanged: [zone_0] obelisks 1-7 with their guards, [zone_18], [zone_30]
// shipped, unchanged: [bothmob_001] ... [bothmob_025] (11.4's table)
```

Map 0's obelisks for reference: 1 (155, 190) and 4 (1843, 1827) the two far corners, 2 (551, 631) and 3 (1535, 1321) the flanks, 5
(1114, 1112) the centre, 6 (622, 1721) Light's base by Light's exits, 7 (1759, 798) Fury's base by Fury's exits. Each road then
needs the enemy's base obelisk, so the road opens where the attackers already stand. Wire ids: the roads become rows **10 to 13** of
map 0's `0x2101` (after the nine obelisks); `0x2104` carries 1010/1011/1020/1021. **A** would be the same four links with the sides
swapped (road 1010 `[light]` with Light's own obelisks, and so on) and start countries left closed. No boss group of our own is
proposed: the shipped 25 cover the maps whose portals 1001–1007 need them.

### 11.7 Deviations

**Decided first**: design **B** (invasion roads); the file is **a full copy of `Obelisk.ini` in the repository**
(the shipped content unchanged, plus the links and the road portals' ids and start countries), read by the importer instead of the
shipped one; deviations **3 to 7 approved**; **8 not taken** (boss chains start afresh at each start, as the original).

3. **Road portal ids and start countries from our import** (needed by either design): map 0's portals 9, 8, 7, 6 get the ids 1010,
   1011, 1020, 1021 while keeping their start countries 1, 1, 2, 2 (B; A: start closed). The original can give a portal an id only
   through a map-file country above 1000, which also shuts it; we keep the shipped `0.svmap` and set this in our file.
4. **The road links themselves** (11.6): our design, not in any shipped file.
5. **`0x2104` only when a road opens to a faction** (the original sends it on every change; with the client fix it would print "will
   be opened" on closing too). `0x2102` stays on every change.
6. **Link checks at import**: K must name an obelisk of that map (K ≤ its count; the original accepts count + 1, past the array), a
   `[Portal_…]` head without its three numbers starts no link and takes no `[light]`/`[fury]` lines (the original gives them to the
   previous link), a link with both lists is refused with a warning (the original judges it on Light only), no 64-link limit and pairs
   for every link (the original's table: 12). None changes our file's behaviour.
7. **Boss followers count the ones really there** (as deviation 1 does for obelisk guards): a blocked spot is tried again on the next
   pass instead of leaving the group short.
8. ~~**Boss chains survive a restart**~~ (**not taken**): each chain starts afresh at each start, as the original, so a
   restart brings the fixed bosses back at once and shuts their doors.

**Revised later** (while reviewing the build plan, now `docs/plans/done/obelisk-bosses.md`): **map 0's exits stay as shipped**, each
open to its own faction only, with **no road links** (the reason: each exit leads into the other faction's safe area of map 45
or 47, where an invader could do nothing). So **design B, the repository copy of `Obelisk.ini` and deviations 3, 4, 5 and 6 are
withdrawn**: the importer keeps reading the shipped file, which links nothing, and the road code of 11.2 (`UpdatePortalLink`,
`0x2104`, saving the links, `EnableMoveByPortal` fed by links) is not built, exactly as the shipped file never runs it. Deviation 7
(boss followers) stands; 8 stays not taken. A link added to a file later would need 11.2 built first.

Still the original (no deviation): everything else of 11.2 and 11.3, including the hysteresis, the pairs, counting by last destroyer,
`EnableMoveByPortal`, saving the roads, the ± 1 hour and random boss times, and the `/mera` fall of 11.5.

## 12. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
and their `Obelisk.ini`, read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).
Function names in those builds are carried over from EP4.5 (matched by strings or the call graph, as those pages list).

### EP3

- The code is there: `CMap::LoadObelisk` (EP3 ps_game 0x422e90), `UpdatePortalLink` (0x425ad0) and `FindPortalLink` (0x426fa0); the
  portal-link functions and `SendPortalLinkChange` are identical or nearly so to EP4.5's (confirmed).
- `Obelisk.ini` has zones 0, 18 and 30 as in EP4.5, only six boss groups (maps 14, 17, 19, 20), and **four roads**: `[Portal_001…004,
  (0, 1049)]`, Light needing obelisks (1, 2, 5) or (1, 5, 6), Fury (3, 4, 5) or (4, 5, 7) (confirmed, the file). Comparing with EP3
  showed that the roads sections 1 and 8 infer from the file's notes were once written out, but all four name portal 1049, while EP3's
  map 0 has portals 1010, 1011, 1020 and 1021 (to maps 45 and 47, which EP3 lacks). EP3's loader (EP3 ps_game 0x422e90) parses the
  same heads, `[Portal_` included, so it reads the four roads (inferred from the shared parser). Where it sets them up was not located,
  so that it drops portal 1049 as section 11.1 does rests on the EP4.5 rule, and the roads most likely never worked in EP3 either
  (inferred). The two dragon groups name portal 1049 too.

### EP5.4

- **The same code** (confirmed for `SendObeliskList`, EP5.4 ps_game 0x4316e0, and `SendObeliskChange`, 0x431860; the rest inferred from
  matched constants): records of 0x70 bytes, `0x2101` with 13 bytes per obelisk flushed every 100, `0x2102` of 7 bytes. The loader
  (EP5.4 ps_game 0x42de40, matched by the string `[Mob]`) reads the new zone with no new keys.
- **Map 70's changes reach maps 68 and 69** (confirmed): after `0x2102` to its own map, a map-70 change goes to every player on 68 and
  69 (EP5.4 ps_game 0x4318b4, loop 0x419390); `SendPortalLinkChange` does the same (0x4319d6). A player entering a map with a `MapCurse`
  also gets map 70's `0x2101` (EP5.4 ps_game 0x41c891 to 0x41c8b9). In EP4.5 every obelisk packet is map-local.
- **`[zone_70]`** (confirmed, the file): three relic obelisks, neutral at the start, each with 49 guard groups (most of them far across
  the map); four bosses of level 73 with a 12-hour respawn whose portal `(100, 2000)` does not resolve. With 12 obelisks the capture
  value of section 1 becomes 12288 ÷ 12 + 1 = 1025 (inferred). One guard line per relic lacks its `)`, which the tokenizer ignores
  (inferred). The `bothmob` numbers are renumbered; `ObeliskWar.ini` is unchanged.
