# Gatekeepers: `0x020C` and moving a player to another place or map

This page covers gatekeeper NPCs in the EP4.5 game server `ps_game.exe`: their data, the teleport request, and how the
server moves a player to another place or map (the same move portals use). It comes from static analysis; nothing was executed.

## In short

- **A gatekeeper offers up to three destinations**, each with a name and a price (0 to 69,000 gold in the shipped data).
- **The player must be within 30 units** of the gatekeeper, of its faction (or the gatekeeper serves both), and able to pay.
- **The move cannot be interrupted** once accepted.
- **Bug in the original:** a player who has asked to log out pays and does not move.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's; the PDB's names are 4 bytes lower from
`m_pUserItem` on (see [NPC shops](shop.md)).

## 1. Data

Confirmed.

| what | where |
|---|---|
| an NPC | `CNpc`: `m_nNpcType` `+0x34` (2 = gatekeeper), `m_nNpcID` `+0x38`, `m_pInfo` `+0x50` (`stNpc*`) |
| `stNpc` (292 bytes) | `byType`, `wTypeID` `+0x02`, `nModel`, `nMoveDist`, `nMoveSpeed` (the three values `world.npcs` calls `value_a` to `value_c`), **`nCountry` `+0x10`** (0 Light, 1 Fury, 2 both, after the loader's conversion), `szName[256]`, quest links |
| `stNpcGateKeeper` (1120 bytes) | `stNpc`, then `pGateList`: 3 x `stGateLink` (276 bytes): `u16 map`, `SVector pos` `+0x04` (the loader adds 0.9 to y), `char name[256]` `+0x10`, `u32 cost` `+0x110` (= `world.npc_gates`) |
| a pending move | `m_nMovePositionType` (`+0x5818`), `m_dwMovePositionTime` (`+0x581c`, 0 = none), `m_nMoveMap` (`+0x5820`), `m_vMovePos` (`+0x5824`) |

With the shipped data: 207 destinations, 101 of them empty (no name), 12 on maps `Map.ini` does not list; costs 0 to 69,000.

## 2. The request

Confirmed, `CUser::PacketMaininterface` case `0x020C`, 0x468420.

Client → server, 7 bytes: `u32 npc object id, u8 destination` (0 to 2). The PDB struct `TP_USER_USEGATENPC` and the client agree.

**Failed** (answer below) when: the destination is above 2; the NPC is not in the player's zone (`CZone::FindNpc`, the same map); it is not a
gatekeeper (type 2) or has no definition; the horizontal distance from the player to the NPC is **30 or more** (`SVector::Distance2`); the gatekeeper's
country is not 2 and not the player's; the destination has no name; its cost is above the player's money; its map is not loaded (`CWorld::GetMap`);
its position is not on that map (`CMap::IsSafePos`).

Two special cases, not needed with the shipped data: a destination on **map 200** (none exist) goes to a guild ranking battle through ps_dbAgent
(`0x080C`) instead; on a **guild house map** (map type 6) guild rules apply (`CGuild`, `0x0D28`-style errors).

**Accepted:** unless a logout is pending (`m_nLogoutType`), the move is scheduled with `m_nMovePositionType` 1 and the time set to now; then, in all
cases, the cost is taken from the money (saved), logged (`0x0804`), and the player is answered. **Original bug:** with a logout pending, the money is
taken and nothing moves.

Server → client, `0x020C`, 7 bytes: `u8 result` (1 moved, 0 failed), `u32 money` (the money after paying, or as it is).

## 3. Carrying the move out

Confirmed, `CUser::UpdateResetPosition` 0x48d470, type 1.

On the player's next update after the scheduled time (here: right away), the player's personal shop is closed (`stMyShop::Ended`), and:

- **Same map** (`CZone::MoveUser` → `CZone::MoveUserLocal`): the position is checked (`IsSafePos`, off the map closes the connection), the cell
  changes as for a step and the view is swapped (`CZone::SendMoveUser`: `0x0202` to and from the players that leave the view, `0x0201`/`0x051C` for those
  that come into it, and the cells' other lists; **no `0x0501`**), then `0x020B` (`CObject::SendView`, 20 bytes: `u32 id, u16 map, f32 x, y, z`) goes to
  everyone in view, the player included.
- **Another map** (`CWorld::ZoneLeaveUserMove` → `ZoneLeaveUserMoveLocal`): the player leaves its zone (`CZone::LeaveUser`, `0x0202` to the view), the
  map and position are set, and `0x020B` with the new map goes to the player only. The player is **not** placed in the new zone yet: the client loads
  the map and sends `0x0201`, which places it as on entering the world (`docs/original/world.md` section 5.2).

`CUser::CancleMove` (0x48a010) does not cancel type 1: a gatekeeper move cannot be interrupted.

## 4. The new server

- Follows sections 2 and 3; the move is carried out right after the `0x020C` answer.
- **A pending logout makes the request fail** instead of taking the money without moving.
- The money is taken with the same write as shop purchases (`ItemStore::trade`), before the answer.
- Map 200 is left out (no shipped destination uses it). The guild house's rules are built (`docs/original/guild-house.md` section 9).

## 5. Open items

- The other `m_nMovePositionType` values: 0 return to the bind position (`CMap::GetBindPos`), 2 to 6 items and skills (town portal scrolls, recall,
  `0x0222` casting cancelled), and the delays they use.
- Portals (`0x020A`, `CUser` and `CMap` portal data in the svmaps).
