# Portals: `0x020A`

This page covers portals in the EP4.5 game server `ps_game.exe`: the portal data in the map files and `Map.ini`, and
what happens when a player steps through one. It comes from static analysis; nothing was executed.

## In short

- **Portals are defined in the map files**: where each stands, where it leads, which faction may use it, and a level range.
- **Stepping through** works within 40 units of the portal, for the right faction and level. Portals into party, guild and guild-house
  maps have extra conditions (a party of the right size, a guild, the map open).
- **A successful step gets no answer of its own**: the player is simply moved, exactly as by a gatekeeper.

Status words: **confirmed** = read in the code (address given).

## 1. Data

Confirmed.

**In the map files** (`CMap::LoadMap` 0x425780, after the NPCs; `tools/svmap.py`): `u32 n`, then n records of 36 bytes, read into
`stMapPortal` (48 bytes):

| file | `stMapPortal` |
|---|---|
| `f32 x, y, z` | `vPos` `+0x04`: where the portal stands |
| `u32 country` | `nCountry` `+0x14`, copied to `nBaseCountry` `+0x10`; when above 1000 it is also the portal's `nID` `+0x00` (otherwise `nID` = -1) |
| `u16 level min`, `u16 level max` | `nLevelMin` `+0x18`, `nLevelMax` `+0x1c` |
| `u32 map` | `nMapID` `+0x20` |
| `f32 x, y, z` | `fMoveX`, `fMoveY`, `fMoveZ` `+0x24`: where it leads |

The section after the portals holds the map's **bind areas** (`stMapBind`: `u32 count, u32 country, u32 attr`, then `count` boxes of 24 bytes),
used to find where a player respawns (`CMap::GetBindPos`).

**In `Map.ini`**, `CreateType` (`CMap::LoadConfig` 0x426a98) becomes `m_nCreateType` (`CMap` `+0x1a4`): S 1, F 2, **P 3** (party instance zone),
**G 4** (guild), **R 5** (guild ranking battle), **H 6** (guild house), anything else 0. Shipped: map 50 R, maps 51 and 52 H, maps 64 to 66 P.

Shipped portals: 265 in all files, 255 on the 68 maps `Map.ini` lists; every one of those leads to a position on a listed map, none to map 200.
Countries: 0 (18), 1 (95), 2 (129), above 1000 (23); all but one have a level range.

## 2. The request

Confirmed, `CUser::PacketMaininterface` case `0x020A`, shared with `0x0227`, 0x468420.

Client → server, 3 bytes: `u8 portal`, the portal's position in the map file's list (`CMap::GetPortal` 0x4298b0 is an index into the array). The PDB
struct `TPC_USER_USEPORTAL` is 4 bytes because the reply uses it too.

Nothing is answered (in this order) when: the player's status is 1 (most likely dying); the portal does not exist; the player is **40 or more** from
it (`SVector::Distance2`, x and z); the destination is map 200 (that goes to the guild ranking battle through ps_dbAgent `0x080C` instead); the
country does not allow the player; the player's level is outside `nLevelMin` to `nLevelMax` (when either is not 0); the destination map is not loaded.

The country is `nCountry`, unless the player's map has a `CreateType` and the portal's `nID` is above 1000; then it comes from the instance zone
(`CZone::GetInsZonePortalCountry`). Country 0 lets everyone through, 1 only Light, 2 only Fury, anything else nobody.

Then by the destination map's `CreateType`:

| type | the original | answer when refused: `0x020A`, 4 bytes, `u8 portal, u8 reason` |
|---|---|---|
| 3 party instance zone | the zone must be open (`CMap::IsPartyInsZoneOpenTime`), the player in a party whose zone (if it exists yet) has room (`CWorld::FindPartyInsZone`), the party big enough (`CreateMinUser`); `docs/original/instances.md` section 3 | 9 closed, 4 no party, 11 the party's zone full (corrected: not "no zone"), 10 too few members |
| 4 guild | needs a guild | 0 no guild |
| 5 guild ranking | needs a guild and the ranking map open (`CMap::IsGuildRankMapOpenTime`), then the player must be on an R map already or not have entered this battle (`+0x54c4`, `docs/original/guild-ranking.md` section 4) | 0 no guild, 1 closed, 2 already took part |
| 6 guild house | needs a guild with a house that can be entered | 0 no guild, 3, 6, 7, 8 (house states) |
| other | — | — |

When allowed: leaving an auction-house map for another map frees the player's auction search (`CMarketMgr::FreeCharSearchInfo`), and, unless a
logout is pending, the move is scheduled exactly as for a gatekeeper (`m_nMovePositionType` 1, now, `nMapID`, `fMove*`) and carried out the same
way (`docs/original/gatekeeper.md` section 3). **A successful use has no `0x020A` answer**; the client gets the `0x020B` of the move. If the
destination is off its map, the connection is closed when the player arrives (`CZone::MoveUserLocal` or `CZone::EnterUser`).

## 3. The new server

- Follows section 2 for maps without a `CreateType`.
- Party instance zones and instance zone portals (country above 1000): `docs/original/instances.md` section 18. Guild houses (H): `docs/original/guild-house.md` section 7. The guild ranking battle (R): `docs/original/guild-ranking.md` section 4. Maps G refuse with 0.
- A destination off its map is ignored instead of closing the connection on arrival.
- Leaving map 42 drops the auction search (`docs/original/auction.md` section 15).

## 4. Open items

- `0x0227` (handled by the same code), instance zones, guild houses and the guild ranking battle.
- Bind areas and respawning (`CMap::GetBindPos`, `m_nMovePositionType` 0).
