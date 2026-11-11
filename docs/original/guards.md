# Walking NPCs and guards

This page covers NPCs that walk and guards that fight in the EP4.5 game server `ps_game.exe`: which NPCs walk and how,
the user count that stops them, and guards (NPC type 8) finding, chasing, killing and going back, with the A* search and chase path they
share with monsters (see [Monster paths](pathing.md)). It was read in the disassembly with the PDB names and types (Ghidra has no
PDB names for these): `CNpc::CNpc` 0x4bbd80, `Reset` 0x4bbe30, `UpdateIdle` 0x4bbe80 and `UpdateAttack` 0x4bbe60 (both inlined into their
callers), `UpdateMove` 0x4bbeb0, `UpdateGuard` 0x4bc0a0, `UpdateAttackAI` 0x4bc100, `FindTarget` 0x4bc180, `SetTarget` 0x4bc750 /
0x4bc790, `StatusAttackStop` 0x4bc800, `Attack` 0x4bc820 (a player) and 0x4bc930 (a monster), `Chase` 0x4bca00, `ReturnHome` 0x4bcf30;
`CZone::UpdateNpcAttack` 0x422390, `CZone::UpdateNpcIdle` 0x422400, the zone update 0x4075a0, `CZone::MoveNpc` 0x416f30,
`CZone::SendMoveNpc` 0x41faa0, `CMap::InitNPC` 0x429130, `CNpcData::Load` 0x4c9410, `CUser::PacketNpc` 0x467510, `CUser::PacketPC`
0x46a230 (the move), `CUser::LeaveZone` 0x44aee0, `CUser::Damage` 0x44c120, `CUser::DeathByNPC` 0x458fe0, `CMob::Damage` 0x490cf0,
`CMob::Death` 0x4910f0. Layouts from the PDB: `CNpc` (0x98), `stNpc` (0x124), `stMapNpc` (0x14), `stTarget`, `TP_NPC_MOVE`,
`TP_NPC_ATTACKMOB`, `TP_NPC_ATTACKPC`. The client side is on [World packets](../client/client-world-packets.md) (`0x0E03`, `0x0E05`, `0x0E06`).

## In short

- **Any NPC with more than one position walks** back and forth along them, 16 units every 8 seconds, pausing 18 to 28 seconds at each
  stop. Most walkers are guards, but some ordinary NPCs and animals walk too.
- **Clicking an NPC holds it still** while you talk to it; it walks on once you move more than 10 units away or leave.
- **Guards attack enemies on sight**: players of the other faction and monsters of another country, within 20 units of the guard and 30
  of its post. They chase, kill with **one blow**, and walk back to their post.
- **A guard's kill gives nothing**: the dead player loses experience as usual but no one gets the kill; a monster killed by a guard gives
  no experience or drops.
- **A guard's blow on a player goes out under the packet named for monsters**, and the other way round; the client reads them the same
  way, so both show correctly.
- **A guard walking back to its post attacks no one**, so a player can follow it home unharmed. Guards also keep the height they set
  out at, whatever the ground does under them.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.

## 1. The data

Confirmed.

**`stNpc`** (one per NPC definition of `NpcQuest.SData`, `docs/original/maps-npcs.md` section 3): the head's four `u32` after the type
id are, in order, **`nModel` `+0x04`, `nMoveDist` `+0x08`, `nMoveSpeed` `+0x0c`, `nCountry` `+0x10`**. `CNpcData::Load` keeps the
country as **0 Light (file 1), 1 Fury (file 2), 2 both (anything else)**, the values of `world.npcs.country`; `value_a`, `value_b`,
`value_c` of `world.npcs` are `nModel`, `nMoveDist`, `nMoveSpeed`. In the shipped file nearly every NPC has **`nMoveDist` 16,
`nMoveSpeed` 8000** (one merchant 15/8000, one 16/80000); 23 have 0/0, among them guard 102 (10 placements on map 20, three with a path).

**`stMapNpc`** (one per placement of the map file): type, type id, the position count, the positions (`SVector`), the directions.
**Every NPC** `CMap::InitNPC` creates keeps its placement (`m_pPosInfo` `+0x54`), so **every NPC with two or more positions walks**
(section 3), not only guards: on the 68 maps of `Map.ini`, 229 guards, 49 type 9, 10 type 7, 4 type 10, one each of types 1 and 3 (and
the map-50 merchant on a guild map). A guard whose first position is a blocked square (the log line "npc guard move block") gets
**`m_dwDisableTime` = `0xFFFFFFFF`** and is never updated; no shipped guard is (two type-7 NPCs on maps 48 and 49 stand on blocked
squares, which only matters for guards).

**`CNpc`**: `m_nNpcType` `+0x34`, `m_nNpcID` `+0x38`, `m_nAngle` `+0x3c` (whole degrees, never changed after creation),
**`m_nNumUsing` `+0x40`** (section 2), **`m_bDir` `+0x44`** (true: forwards along the positions), **`m_nPosNext` `+0x48`** (the
position walked to; 0 at creation), **`m_dwNextMoveTime` `+0x4c`**, `m_pInfo` `+0x50`, `m_pPosInfo` `+0x54`, **`m_nStatus` `+0x58`**
(0 idle, 1 fighting, 3 going back; 2 is never overwritten by the code below, and nothing was found that sets it), **`m_Target`
`+0x5c`** (type 1 player, 2 monster, and the id), **`m_dwNextChaseTime` `+0x64`**, **`m_dwDisableTime` `+0x68`**, `m_AStar` `+0x6c`
(the path, `docs/original/pathing.md` section 2), `m_ChasePath` `+0x94` (the chase path). At creation and `Reset` all of it is 0 but
`m_bDir` true.

## 2. The user count

Confirmed, `CUser::PacketNpc`, `CUser::PacketPC`, `CUser::LeaveZone`.

**Client `0x0E04`** (6 bytes: `u32 npc id`), sent when the player clicks an NPC: another opcode in that handler closes the connection
(reason 9); the NPC is looked up **on the player's map** (`CZone::FindNpc`); not found: nothing. When it differs from the player's
current NPC (`CUser+0x1564`), the old one's `m_nNumUsing` goes down by one, the new one's up by one, and it becomes the current NPC.
Nothing is answered (the client ignores a `0x0E04` reply, `docs/client`).

**The count goes down** and the current NPC is cleared: after each accepted **move** (`0x0501`), when the player is **more than 10**
(horizontal) from it; when the player **leaves the zone**. An NPC with `m_nNumUsing` above 0 does not walk (section 3); a guard still
fights.

## 3. Walking

Confirmed, `CNpc::UpdateMove(now)`.

Nothing when `m_nNumUsing > 0`, before `m_dwNextMoveTime`, without a placement, or with fewer than two positions. Then, with `p` the
position `m_nPosNext` and the NPC's position `q`:

1. `v = (p.x − trunc(q.x), 0, p.z − trunc(q.z))`, its length `L` (horizontal) and direction `n` (`SVector::Normalize`, the zero
   vector left as it is).
2. **`L ≤ nMoveDist`**: `CZone::MoveNpc` **onto `p`** (x, y, z of the position, motion 0). When the move is made (`p` on the map), the
   next position: forwards `m_nPosNext + 1`, at the end (`= count`) **`count − 2`** and backwards; backwards `m_nPosNext − 1`, below 0
   **1** and forwards (so the NPC goes back and forth along its positions). In either case **`m_dwNextMoveTime = now + rand() % 10000 +
   nMoveSpeed + 10000`** (18 to 28 s with the shipped values).
3. **`L > nMoveDist`**: `MoveNpc(q + n × nMoveDist, motion 0)` (y unchanged), **`m_dwNextMoveTime = now + nMoveSpeed`** (16 units every
   8 s).

At creation `m_nPosNext` is 0 and the NPC stands on position 0, so its first update is step 2: a move onto where it stands, then
position 1 after the wait.

**`CZone::MoveNpc(x, y, z, motion)`**: false, and nothing else, when the spot is not on the map (`CMap::IsSafePos`). When the spot is
outside the NPC's cell's widened bounds (`CCell::IsInnerCellView`) and the cell **of the NPC's current position** (before the move;
`CZone::MoveMob` takes the new one) is another cell inside the grid, it changes cells (`CZone::SendMoveNpc`): the players in cells of the
old view that are not in the new one get **`0x0E02`**, those in cells of the new view not in the old one **`0x0E01`** (current
position, `m_nAngle`). Then the position is set and **`0x0E03`** (`TP_NPC_MOVE`, 19 bytes: `u32 id, u8 motion, f32 x, y, z`) goes to
every player in the 5 × 5 view of its cell (no distance limit).

## 4. Who updates when

Confirmed, the zone update 0x4075a0.

Times are `GetTickCount` milliseconds. Per zone:

- **every 500 ms**, after the monsters' attack pass, **`UpdateNpcAttack`**: every guard (type 8) with a placement and `now ≥
  m_dwDisableTime`: **`UpdateGuard(now)`**;
- **every 3000 ms**, after the monsters' idle pass, **`UpdateNpcIdle`**: every NPC with a placement and `now ≥ m_dwDisableTime`: a
  guard **`UpdateGuard(now)`**, the others **`UpdateMove(now)`**.

**`UpdateGuard(now)`** by `m_nStatus`: **0** `FindTarget`, and when it finds one `UpdateAttackAI`, else `UpdateMove(now)`; **1**
`UpdateAttackAI(now)`; **3** `ReturnHome(now, position m_nPosNext)`; 2 nothing.

## 5. Finding a target

Confirmed, `CNpc::FindTarget`.

1. When the square under the guard (`trunc(x)`, `trunc(z)`) is off the map or blocked: **none**.
2. The 3 × 3 cells around its cell (clamped to the grid), **rows of z, then x**; in each cell **the players first, then the monsters**,
   each list in its order. The first that qualifies is taken.
   - **A player**: not of the guard's country (`CUser` country byte `+0x12d` **≠** `nCountry`: a guard of country 2 would take everyone;
     the shipped guards are 248 Light, 339 Fury), alive (`m_nStatusLife` ≠ 1), no shape (`m_byShape`), sees monsters
     (`m_bNotSeeMOB` 0), visible and attackable as staff (`stAdminInfo`), **within 20** of the guard and **within 30 of the guard's
     position `m_nPosNext`** (horizontal), and **not taken by a guard in the last 5 s** (`now ≥ CUser+0x5850`).
   - **A monster**: of another country than the guard's (`stMob` country `+0x78` ≠ `nCountry`; a guard of country 2 takes any), not
     dead (`CMob::GetStatus` ≠ 2), within 20 of the guard and 30 of its position, and not taken in the last 5 s (`now ≥ CMob+0x2b0`).
3. **Found** (`SetTarget`): when the guard is already fighting (status 1) **nothing changes** (its target stays); else status 1 (unless
   2), the target (1 and the player's id, or 2 and the monster's), and **`now + 5000`** on the one taken (`+0x5850` / `+0x2b0`). True.

## 6. The attack turn

Confirmed, `CNpc::UpdateAttackAI`.

The target is looked up in the zone by id (`CZone::FindUser` / `FindMob`, anywhere on the map). Not there: status 0 and no target
(unless 2). There: **`Chase(now, target position, range 2)`**; when it says "in range", **`Attack`**. A target of type 0 does nothing.

**`CNpc::Attack(now, player)`**:

1. A dead player (`m_nStatusLife` 1): straight to step 4 as a success.
2. Otherwise `CUser::StatusCastingStop` (the hit's usual effects: shapes, sleep, casting, `docs/original/skills-special.md`), the duel
   rule of `docs/original/duel.md` section 6.5 (`PvPChkLeaveByAttack`), then **`CUser::Damage(3, HP)`** with **the player's whole HP**:
   - nothing (false) when dead or with HP 0;
   - with **absorption** (`m_nReduceDamage`, `+0x12fc`, `docs/original/lapis.md` section 2) and the damage at least half the max HP
     (`maxHP / 2`, an integer division): the damage **− 1**, and **`0x0525` with the absorption** (`u16`, the whole value, not the 1);
   - armour wear (`QualityByDefense`), the damage recorded (`CDamage::Add`);
   - HP ≤ damage: HP 0, the party's HP view, **true**; else HP − damage, the party's HP view, the low-HP passive, false.
   **True**: **`CUser::DeathByNPC(3, guard id)`** (section 7), then status 0 and no target (unless 2). False: the attack failed.
3. **`0x0E05`** (`TP_NPC_ATTACKMOB`, 13 bytes: `u8 1, u32 guard id, u32 player id, u16 HP`, the HP as it was before the blow) to the
   players **within 60** of the guard, success or not.
4. On success **`FindTarget`**; none: status 3 (unless 2).
5. When `now ≥ m_dwDisableTime`: **`m_dwDisableTime = now + 1000`** on success, **`now + 500`** on failure.

So a guard kills with one blow. The one exception is a player with absorption at half HP or more, who keeps 1 HP and dies to the next
blow 500 ms later.

**`CNpc::Attack(now, monster)`**: a dead monster (`GetStatus` 2) skips to step 3. Otherwise the monster's fair-combat flag
(`CDamageMob` `+0x1f0`) is cleared and **`CMob::Damage(3, guard id, HP)`** with its whole HP. On a kill **`CMob::Death(3, guard id)`**:
with fair combat cleared (and no obelisk or `bothmob` flags) **only `0x0606`** (`u32 monster id, u8 3, u32 guard id`) to the monster's
view. No experience, drops or quest counts. Then status 0 and no target (unless 2). **`0x0E06`** (`TP_NPC_ATTACKPC`: `u8 1, u32 guard
id, u32 monster id, u16 HP`) within 60. Step 3: **`FindTarget`**; none: status 3 (unless 2). `m_dwDisableTime = now + 500` when due.

Note the packets: **the original sends `0x0E05` (`TP_NPC_ATTACKMOB`) for a player and `0x0E06` (`TP_NPC_ATTACKPC`) for a monster**,
against the PDB's names, and **the client reads them the same way** (`game.exe` disassembly: the `0x0E05` handler
0x54f890 looks its target up among the players (0x447880) and lowers its HP `+0x130`, the `0x0E06` handler 0x54f9e0 among the monsters
(0x447900); with the guard in view both store the target and damage on the guard, `+0xb8` 1 for a player, 0 for a monster, and apply
them when its attack lands; an unknown guard's blow applies at once). `docs/client/client-world-packets.md` had them swapped.

## 7. Death by a guard

Confirmed, `CUser::DeathByNPC`.

As `CUser::Death` (`docs/original/mob-fight.md` section 8) with these differences: the experience to lose is set from the
death-protection items only (5, or 0/1 with them), **without the damage record**: the record is reset and never asked, so **no player
gets a kill**, whoever hit the player before; it does **not** end trades, personal shops or shared mounts; **`0x0504`** names killer
kind **3** and the guard's object id. The duel and guild-war rules of a death apply (`PvPChkLeaveByDeath`). (Its game-log line looks
the NPC up with the arguments swapped and gets no name.)

## 8. Chasing

Confirmed, `CNpc::Chase(now, target, range)`.

1. Before `m_dwNextChaseTime`: false.
2. The square under the guard off the map or blocked: status 0, no target (unless 2), false.
3. **`d` = horizontal distance to the target; `range + 1 > d`** (`d < 3`): **in range**: the path ended, true.
4. The target **more than 30 from the guard's position `m_nPosNext`**, or `d > 30`: status 3 (unless 2), target type 0 (the id stays),
   the path ended, false.
5. **Without a path**, `m` the guard's truncated cell, `t` the target's: `CMap::EnableLine(m, t)`:
   - **free**: the cell walked to is `t` (**no path is set**: each turn aims again);
   - **blocked, `t` blocked**: the last free cell **is `m`**: status 3, target type 0; otherwise **`StatusAttackStop`** (status 0, no
     target, unless 2); the path ended either way, false;
   - **blocked, `t` free**: **the search** `AStar::FindPath(m, t)` (`docs/original/pathing.md` section 1); no way: status 3 (unless 2),
     target type 0, the path ended, **`m_dwDisableTime = now + 3000`**, false.
6. **With a path**: the cell walked to is its current cell.
7. With `p` that cell: `v = p − m` (as floats, y 0), `n` its direction, **`g = |v| − 1`**, the step **`s = nMoveDist`**, and
   **`ChasePathList::AddPath(m)`** (the chase path, as monsters keep it) in every case.
   - **`g ≥ s`**: `MoveNpc(position + n × s, motion 1)`, **`m_dwNextChaseTime = now + nMoveSpeed / 4`** (an integer division, 2000 ms
     with the shipped values); the path stays.
   - **else**: `MoveNpc(position + v − n × 0.3, motion 1)`; when it is made, **the guard's position is then set to `position + v`**
     (0.3 further on, sent to no one); `m_dwNextChaseTime = trunc(now + g / s × (nMoveSpeed / 4))` (now or earlier when `g < 0`);
     **`CursorNext`** (the path's next cell; after the last none).
   False either way.

Unlike a monster's chase there is no size or attack range in the steps, and no separate rule for a searched way's turns. The
height is never worked out: `v` has y 0, so a chasing (or returning, section 9) guard keeps the y it had when it set out, and the client
draws it there (`0x0E03` carries x, y, z), whatever the ground or a bridge does under it. While going back (status 3) it looks for no
target (section 4), so a player may follow it home unharmed until it is within 1.5 of its position (seen in play; the new server looks on the way, deviation 7).

## 9. Going back

Confirmed, `CNpc::ReturnHome(now, home)`.

"Home" is **the guard's position `m_nPosNext`**, not where it was created.

1. Before `m_dwNextChaseTime`: false.
2. **Within 1.5** (horizontal): status 0 (unless 2), the path and the chase path ended, true (the walk of section 3 goes on).
3. **Without a path**, `EnableLine(m, h)` with `h` home's truncated cell:
   - **free**: the cell walked to is `h` (no path set);
   - **blocked, `h` blocked**: status 0 (unless 2), both paths ended, false (whatever the last free cell);
   - **blocked, `h` free, a chase path left**: its newest cell is taken off (`GetNextPath`) and becomes the path (`SetPath`);
   - **blocked, `h` free, no chase path**: the search `FindPath(m, h)`; no way: status 0 (unless 2), both paths ended,
     **`m_dwDisableTime = now + 3000`**, false.
4. **With a path**: its current cell.
5. With `p` that cell, `v = p − m`, `n`, **`L = |v|`**, `s = nMoveDist`:
   - **`L ≥ s`**: `MoveNpc(position + n × s, motion 1)`, `m_dwNextChaseTime = now + nMoveSpeed / 4`;
   - **else**: `MoveNpc(position + v, motion 1)` (y kept), `m_dwNextChaseTime = trunc(now + L / s × (nMoveSpeed / 4))`,
     **`CursorNext`**.
   False.

## 10. The new server

- **Data**: `world.npcs` `value_b` / `value_c` as the move distance and time, `country` as the guard's; every position of each
  placement (`world.map_npc_positions`) for the walk. Each zone's NPCs live in `Zones` (`zone/npc.rs`): position, cell, the walk (next
  position, direction, next move), the user count, and for guards status, target, next chase, disable time, the path and the chase
  path. A party or guild zone gets its own when it is made, as `CZone::MapInit` does for every zone, with new object ids from the NPCs'
  counter, and they go with the zone.
- **`crate::guard`**: the pure parts: the walk (section 3), the chase (section 8) and going back (section 9), with the A* and the chase
  path of `crate::fight`.
- **The passes**: `Zones::npc_attack` after the monsters' attack pass every 500 ms, `Zones::npc_idle` every 3 s (`server::mob_updater`).
- **Targets**: players as `CMob::FindTarget` takes them (alive, no shape), in id order within a cell; monsters in their cell's order.
  The 5-second lock is kept per player and per monster.
- **Kills**: a player through the zone's usual death (`zone::pvp::player_died`) with killer kind 3 and no killer of record; a monster
  through the monster's death with `0x0606` kind 3 and nothing else (an obelisk monster still does what its death does).
- **`0x0E04`**: the user count of section 2; leaving the zone and moving more than 10 away release the NPC.
- **Deviations** (1, 2, 4 and 6 approved; 7 requested; 3 and 5 withdrawn):
  1. **An NPC changes cells by its new position** (`CZone::MoveNpc` takes the cell of the old one, so the NPC's cell lags one move
     behind and players who come into its real cell's view may not see it).
  2. **An NPC with move distance 0 does not move**: the original makes a zero-length move, and sends `0x0E03` for it, every turn
     (guard 102, three placements with a path on map 20, every 500 ms; its chase the same).
  3. *Withdrawn*: the blows were sent the other way round (`0x0E06` for a player), on a wrong reading of the client. The
     client then never applied the killing blow: the player stayed at full HP, dead to the server, and could neither move nor log out.
     Now as the original: `0x0E05` for a player, `0x0E06` for a monster.
  4. **A death by a guard ends trades, personal shops and shared mounts** like any other death (`DeathByNPC` leaves them).
  5. *Withdrawn*: NPCs of party and guild zones were going to stand still because the zones shared their map's NPCs; each zone now
     has its own, as in the original, so they walk too.
  6. **A guard whose player target is dead drops it and looks again at once.** The original keeps the dead player as its target and only
     asks `FindTarget` whether anyone else is near, so while someone else is near it keeps "attacking" the body and ignores them
     until the dead player leaves.
  7. **A guard going back looks for targets too** (status 3: `FindTarget` first, as in status 0; a target found ends the walk home and
     the chase starts from where it is). The original only walks home, so a player could follow it there unharmed (seen in
     play). The 5-second lock and the 20 / 30 limits of section 5 still apply.

## 11. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by the call graph and constants, as those pages list).

### EP3

- `CNpc::FindTarget` (EP3 ps_game 0x494270) lacks the two `now + 5000` stores and four state tests of EP4.5's (confirmed: no `0x1388`
  there), so the 5-second "one guard per target" lock of section 5 is new in EP4.5 (confirmed: EP3's `FindTarget`, 0x494270 to
  0x494756, reads no clock, and no `0x1388` appears anywhere from 0x489161 to 0x499940 of EP3 ps_game, the whole guard code).
- `CNpc::Chase` (EP3 ps_game 0x4949a0) and `CNpc::ReturnHome` (0x494ed0) are nearly identical to EP4.5's (confirmed).

### EP5.4

No differences in the code. `CNpc::FindTarget` (EP5.4 ps_game 0x4d4050, matched by its floats 20 and 30 and the 5000 ms, the only such
function) and `CNpc::UpdateAttackAI` (0x4d3fa0) have the same constants (confirmed). In the data, map 62's blocked-cell bitmap is empty,
so guards there are not kept out of walls (inferred from [`maps-npcs.md`](maps-npcs.md)).
