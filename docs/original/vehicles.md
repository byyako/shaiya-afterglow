# Mounts: `0x0216` and the two-seat ride `0x021C` to `0x021E`

This page covers mounts in the EP4.5 game server `ps_game.exe`: summoning and getting off, what riding prevents, the
rider's shape, and the two-seat mounts that carry a passenger. It was read in the Ghidra decompiler and the disassembly with the PDB
names. The client side is in the [packet catalogue](../client/client-packets.generated.md) (the client handles `0x0216`, `0x0217`,
`0x021C` to `0x021E` and sends `0x0216` (2 bytes), `0x021C` (2), `0x021D` (6), `0x021E` (3)).

## In short

- **The mount is an item** worn in equipment slot 13. Summoning takes 3 or 5 seconds of animation, standing still; a step or a hit
  cancels it. After getting off, the next summon has to wait 10 or 20 seconds.
- **Riding is for travel.** A rider cannot attack or use skills, and any hit throws the rider off. Mounts disappear on entering a
  dungeon.
- **Rider looks.** While riding, the character is drawn as the mount: its shape number depends on the mount and, for some mounts, on
  whether the player is in hard or ultimate mode.
- **Two-seat mounts** can carry a passenger of the same faction, who must accept within 30 seconds. If either is hit, both get off.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's here):
`m_nVehicleStatus` `+0x1460`, `m_dwNextVehicleTime` `+0x1464`, `m_nVehicleType` `+0x1468`, `m_nVehicleTypeAdd` `+0x146c`,
`m_dwVehicle2CharID` `+0x1470`, `m_dwVehicle2ReqID` `+0x1474`, `m_dwTimeOverVehicle2` `+0x1478`; the mount is the item in equipment slot
**13** (item type 42, 32 mounts in the data).

**Vehicle status**: **0** on foot, **1** summoning, **2** riding, **3** riding behind someone (the passenger of a two-seat mount).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/client-packets.generated.md` (the client handles `0x0216`, `0x0217`, `0x021C` to
`0x021E` and sends `0x0216` (2 bytes), `0x021C` (2), `0x021D` (6), `0x021E` (3)). `CUser` offsets are the code's (4 above the PDB's here):
`m_nVehicleStatus` `+0x1460`, `m_dwNextVehicleTime` `+0x1464`, `m_nVehicleType` `+0x1468`, `m_nVehicleTypeAdd` `+0x146c`,
`m_dwVehicle2CharID` `+0x1470`, `m_dwVehicle2ReqID` `+0x1474`, `m_dwTimeOverVehicle2` `+0x1478`; the mount is the item in equipment slot
**13** (item type 42, 32 mounts in the data).

**Vehicle status**: **0** on foot, **1** summoning, **2** riding, **3** riding behind someone (the passenger of a two-seat mount).

## 1. Summoning and getting off: `0x0216`

Confirmed, `CUser::PacketMaininterface` 0x46940b.

Client → server, 2 bytes (no payload). Answers are **`0x0216`** (4 bytes, `TP_USEVEHICLE`: `u8 success, u8 status`) to the player only.

- A passenger (status 3) is ignored.
- No mount equipped, the player not standing (`m_byMotion` not 0, e.g. sitting) or its shape not 0 (hidden, transformed): **`0x0216` 0, 1**.
- Otherwise the attack stops (`StatusAttackStop`), then by status:
  - **0**: while `m_dwNextVehicleTime` is not yet past: **`0x0216` 0, 0** (too soon). Otherwise a pending two-seat invitation the player
    received is dropped (its driver gets **`0x021E` 2**), status **1**, `m_dwNextVehicleTime` = now + **3 s** when the mount's `grow`
    (`stItemInfo` `+0x30`) is 3, else **5 s**, and **`0x0217`** (6 bytes, `TP_USEVEHICLEREADY`: `u32 player id`) to the players in view:
    the summoning animation. No `0x0216` yet.
  - **1** (summoning again): the summon is cancelled (`m_dwNextVehicleTime` = 0).
  - **2** (riding): getting off (`CUser::CancleActionVehicle`, section 4).
  - then status **0**, **`0x0216` 1, 0** and the shape to the view (`CUser::SendShape`, `0x051D` with the shape of section 3).

**The summon completes** on the player's update (`CUser::UpdateRebirth` 0x48d240) once status is 1 and `m_dwNextVehicleTime` is reached:

- the mount no longer equipped, or the shape 10, 11 or 12: status 0, `m_dwNextVehicleTime` = 0, **`0x0216` 0, 1**;
- otherwise: `m_nVehicleType` = **15** when the player's mode (`grow`) is at least 2 (hard, ultimate) **and** the mount's `grow` is at least 2,
  else **14**; `m_nVehicleTypeAdd` = the mount's `range` + 7 when `range` ≥ 2, else `range` × 2; `m_dwNextVehicleTime` = now + the mount's
  `reqig` × 1000 ms (10 or 20 s in the data: the time before the next summon); status **2**; **`0x0216` 1, 1**, then the shape to the view.

**Cancelled summons** (status 1 → 0, `m_dwNextVehicleTime` = 0, **nothing sent**): an accepted step (`0x0501`), a blow that reaches the
player (`CUser::StatusCastingStop`), leaving the zone.

## 2. While riding

Confirmed.

- **Refused** while the status is not 0 (summoning, riding or behind someone): normal attacks on players and monsters (`0x0502`, `0x0503`,
  ignored), skills (`0x0511`, `0x0517`, ignored) and item skills (`0x0559`, the item error). Items used with `0x050A` and `0x0557` are not
  refused. A duel cannot be requested or accepted (`CUser::PacketPVP`; `docs/original/duel.md` section 7). Evolution (skill
  127) cannot aim at someone not on foot (`CUser::ChkTargetUser`).
- **Speed**: the move speed check (`stUserMoveChk`, `docs/original/world.md` section 2) uses **8.0** while riding and 10007 (in effect none)
  behind someone; the 35-unit step check stays. (Built since with the check, `docs/original/move-check.md`.)
- **Moving the mount** out of slot 13 (or another item into it) while summoning or riding: the summon is cancelled or the player gets off as in
  section 1 (status 0, **`0x0216` 1, 0**, the shape); not for a passenger.
- **A blow that reaches the player** (`CUser::StatusCastingStop`, called on every hit): summoning stops silently; a rider gets off
  (`CancleActionVehicle`, status 0, the shape to the view, **no `0x0216`**); a passenger gets off (status 0, `0x021C` with 0 to its view) and
  **its driver gets off too** (status 0, the driver's shape to its view).
- **Entering a dungeon** (`m_nMapType` 1, `CZone::EnterUser`) while riding: status 0, silently.

## 3. The shape

Confirmed, `CUser::GetShape` 0x48a300, `SendShape` 0x482590, `SendShapeMe` 0x482510.

While riding (status 2) the player's shape is **`m_nVehicleType + m_nVehicleTypeAdd`** (14 to 27 with the mounts in the data); otherwise
`m_byShape` (hidden, transformed) or 13 for invisible staff. It goes out as `0x051D` (`u32 id, u8 shape`) to the view on each change, to
players the rider comes into view of after its `0x0201` (`CCell::SendCellInfoEnter`, `CZone::SendMoveUser`) and in the burst-time
`SendShapeMe`.

## 4. Two on one mount: `0x021D`, `0x021E`, `0x021C`

Confirmed.

**Asking** (`0x021D` client → server, 6 bytes: `u32 target id`, 0x4695cd): the asker must be riding a **type 15** mount with nobody behind;
the target must be in the zone, on foot (status 0), without a pending invitation, and of the **same country**. Then each one's
`m_dwVehicle2ReqID` names the other, the asker's deadline is now + **30 s** (`m_dwTimeOverVehicle2`), and the target gets **`0x021D`** (6
bytes: `u32 asker id`). Any failure: **`0x021E` 2** to the asker.

**Answering** (`0x021E` client → server, 3 bytes: `u8 answer`, 0x469695): only on foot with a pending invitation, whose asker is in the zone
and still asking this player. The answer goes to the asker as it came (**`0x021E` answer**). **0 = yes**: each one's `m_dwVehicle2CharID`
names the other, the target's status becomes **3** and **`0x021C`** (10 bytes, `TP_VEHICLE_USEVEHICLE2`: `u32 passenger id, u32 driver id`)
goes to the passenger's view. Either way both invitations are cleared.

**Getting off behind** (`0x021C` client → server, 2 bytes, 0x46958a): only with status 3: the driver's partner is cleared (it rides on), the
passenger's status 0 and **`0x021C`** (`passenger id, 0`) to its view.

**The invitation ends** after 30 s (`CUser::UpdateTimeover`: both cleared, nothing sent), when the asker gets off, and when either leaves the
zone (`CUser::LeaveZone`: an asker's target is cleared silently; a target's asker, still riding, gets **`0x021E` 2**).

**The pair splits**:
- when the driver gets off (`CancleActionVehicle` 0x44b2c0): the passenger's status 0 and `0x021C` (`passenger id, 0`) to the passenger's
  view; the driver's pending invitation cleared;
- on a teleport within the map (`CZone::MoveUser`) or a map change (`CWorld::ZoneLeaveUserMove`) of either: the passenger's status 0 and its
  `0x021C` with 0; the driver keeps riding;
- when either leaves the world (`CUser::LeaveWorld` → `CancleActionVehicle` or `CancleActionVehicle2`).

A passenger's `0x0201` (`TP_USER_ENTER`, `docs/original/enter-world.md` section 4) carries its driver's id in its last field.

## 5. The new server

- The zone keeps each player's vehicle status, mount shape, partner and invitation (`zone/vehicle.rs`), so the other players' packets and
  the hit, warp and leave rules of sections 2 and 4 happen where the players are; the connection keeps the summon's due time and the next
  allowed summon (`m_dwNextVehicleTime`) and asks the zone for the status before attacks, skills and item skills.
- Riding survives a map change (the connection carries the mount shape into the next zone) except into a dungeon, as in the original; a
  summon does not.
- As the original; the move speed check (section 2) is built (`docs/original/move-check.md`). Staff invisibility (shape 13) and duels are
  built (`docs/original/gm-commands.md` section 13, `zone::vehicle` `Placed::shape_packet`; `docs/original/duel.md` section 7).
- No deviations.

## 6. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

The mount requests exist (`0x0216` and `0x021C` to `0x021E` in the dispatch table, confirmed); their bodies were not compared.

### EP5.4

- **Summoning and getting off are unchanged** (confirmed): `0x0216` (EP5.4 ps_game 0x477052) is the same code; only the vehicle status
  moved (`CUser+0x147c`).
- **The rider's shape is unchanged** (confirmed): type 14, or 15 for a hard or ultimate player on a hard-or-better mount, plus `range` + 7
  or `range` × 2 (EP5.4 ps_game 0x49db8a to 0x49dbc9, in the function matched to `CUser::UpdateRebirth` by this identical block).
- **48 mounts** in the data (EP4.5 32), with `range` up to 16, so shapes up to 38 (EP4.5 27); the server passes the number on unchecked,
  and whether the client draws it is not known (inferred; not answerable here: it needs the EP5.4 client, which is not among the files
  read).
- **A new buff kind 97 dismounts** (confirmed): it clears the vehicle status, gets the passenger off and sets shape 103 (EP5.4 ps_game
  0x494645 to 0x4946d0).
- The new move check allows 8.0 for a rider (EP5.4 ps_game 0x4785aa to 0x47862f) (confirmed). The two-seat requests were not compared.
