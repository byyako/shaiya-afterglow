# Guild houses

This page covers guild houses in the EP4.5 game server `ps_game.exe`: the guild's house state, buying a house,
returning Etin, the house NPCs' levels and their upgrades, the upkeep (keep Etin), going into a house (portal and teleport stone) and
its zone, and what a house's NPCs do differently (shops, gatekeepers, the smith's repair and lapis linking). It was read in the Ghidra
decompiler and the disassembly with the PDB names and types. Data: `Data/GuildHouse.ini`, `Data/Map.ini` (maps 50 to 52) and the map
files of 51 and 52. Nothing was executed.

Read: `CUser::PacketGuild` 0x4764a0 (cases `0x0D23`, `0x0D25`, `0x0D34`, `0x0D35`), `CGuild::EnableGuildHouse` 0x42f0c0,
`CGuild::SendNpcLvList` 0x42f0e0, `CGuild::NpcUpgrade` 0x430640, `CGuild::CalculateKeepEtin` 0x430a50,
`CGuild::GetGuildHouseShopBuyRate` 0x430ac0, `GetGuildHouseUseGateRate` 0x430c90, `GetGuildHouseRepairRate` 0x430d60,
`GetGuildHouseRapiceMixDecreRate` 0x430dd0, `GetGuildHouseRapiceMixPercentRate` 0x430e40, `CGuildMgr::InitGuildHouseIniFile` 0x4319a0
(and its tables 0x541b10 to 0x541d50, 0x5471e0), `CGuildMgr::UpdateGuildPointRank` 0x432b70, `CWorldSendThread::Run` 0x406f90,
`CWorld::GetZoneByMapID` 0x413df0 (case 6), `CUser::PacketMaininterface` 0x468420 (`0x020A` case 6, `0x020C`), `CUser::PacketShop`
0x46af10, `CUser::ItemRepair` 0x464340, `CUser::ItemGemAdd` 0x461310, `CUser::ItemUse` 0x4655e0 (special 56), `CUser::PacketUserDBChar`
0x46cf60, `CGuild::JoinOK` 0x42e330, `CGuild::JoinUser` 0x42e7a0. The client side is on [Social packets](../client/client-social-packets.md)
(`0x0D23` to `0x0D38`).

The ranking battle and the ranking update are on [Guild ranking battle](guild-ranking.md); the guild bank on
[Guild bank](guild-bank.md).

## In short

- **A guild needs a ranking (1 to 30) and a bought house** to use it. The guild master buys the house for 50,000,000 gold.
- **Buying does not open the house.** It opens at the next weekly ranking, which also takes the first upkeep in Etin.
- **Etin** (four kinds of item, worth 1, 10, 100 or 1000) is returned to the guild by its members. It pays for NPC upgrades and the
  weekly upkeep. A guild short of Etin at a ranking has its house closed until the next one.
- **The house NPCs have levels.** The master and officers upgrade them one level at a time with Etin; each upgrade raises the weekly
  upkeep for some NPCs. Higher-level merchants and gatekeepers are locked until the guild has their level.
- **House NPCs serve the guild for less**: merchants sell cheaper, gatekeepers are free, the blacksmith repairs cheaper and makes lapis
  linking cheaper and, at higher levels, more likely to succeed. Some need a ranking of 14 or better (6 or better for potions).
- **Each guild gets its own copy of the house map**, entered by a portal or the Guild House Teleport Stone.
- **Buying the house and returning Etin do not check the NPC**: both work from anywhere.

Status words: **confirmed** = read in the code (address given).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names and types), read with the decompiler (REA) and the disassembly (objdump):
`CUser::PacketGuild` 0x4764a0 (cases `0x0D23`, `0x0D25`, `0x0D34`, `0x0D35`), `CGuild::EnableGuildHouse` 0x42f0c0,
`CGuild::SendNpcLvList` 0x42f0e0, `CGuild::NpcUpgrade` 0x430640, `CGuild::CalculateKeepEtin` 0x430a50, `CGuild::GetGuildHouseShopBuyRate`
0x430ac0, `GetGuildHouseUseGateRate` 0x430c90, `GetGuildHouseRepairRate` 0x430d60, `GetGuildHouseRapiceMixDecreRate` 0x430dd0,
`GetGuildHouseRapiceMixPercentRate` 0x430e40, `CGuildMgr::InitGuildHouseIniFile` 0x4319a0 (and its tables 0x541b10 to 0x541d50,
0x5471e0), `CGuildMgr::UpdateGuildPointRank` 0x432b70, `CWorldSendThread::Run` 0x406f90, `CWorld::CheckGuildRankStatus` 0x4143e0,
`CMap::IsGuildRankMapOpenTime` 0x428d20, `CWorld::GetZoneByMapID` 0x413df0 (case 6), `CUser::PacketMaininterface` 0x468420 (`0x020A`
case 6, `0x020C`), `CUser::PacketShop` 0x46af10, `CUser::ItemRepair` 0x464340, `CUser::ItemGemAdd` 0x461310, `CUser::ItemUse` 0x4655e0
(special 56), `CUser::PacketUserDBChar` 0x46cf60, `CGuild::JoinOK` 0x42e330, `CGuild::JoinUser` 0x42e7a0. Layouts from the PDB:
`DB_GUILD_LIST_UNIT`, `DB_GUILD_UPDATERANK`, `TP_DB_GUILD_NPC_UNIT`, `TPC_GUILD_*`, `TP_DB_GUILD_*`. Data: `Data/GuildHouse.ini`,
`Data/Map.ini` (maps 50 to 52), the map files of 51 and 52. Nothing was executed. The client side is
`docs/client/client-social-packets.md` (`0x0D23` to `0x0D38`) and `docs/client/CLIENT-FACTS.md` (the `0x020A` refusals).

Covered: the guild's house state, buying a house, returning Etin, the house NPCs' levels and their upgrades, the upkeep (keep Etin),
going into a house (portal and teleport stone) and its zone, and what a house's NPCs do differently (shops, gatekeepers, the smith's
repair and lapis mixing). **Not covered** (later): the ranking battle and the ranking update of section 8 are part 2 (built,
`docs/original/guild-ranking.md`); the guild bank (`0x0D29`, `0x0D31`, `0x0D32`, bag 255, the house warehouse NPC's window `0x72`) is part 3
(`docs/original/guild-bank.md`, built); `CreateType` G; the operator commands (`FA12`, `FA14`, `FA15`; built,
`docs/original/gm-commands.md` sections 8.3 and 8.6).

## 1. The guild's state

Confirmed, `DB_GUILD_LIST_UNIT`, `CGuild`.

| `CGuild` | PDB field | meaning |
|---|---|---|
| `+0x40` | `byCountry` | 0 Light, 1 Fury (`> 1` is read as 1 by the rate functions) |
| `+0x44` | `dwGuildPoint` | the points of the last ranking |
| `+0x48` | `byRank` | **ranking** 1 to 30, **31 = unranked** (`GuildDetails.Rank` default 31) |
| `+0x4C` | `dwEtin` | the guild's Etin |
| `+0x50` | `dwKeepEtin` | the upkeep: what the ranking takes each time (section 6) |
| `+0x54` | `byUseHouse` | **0** not open yet, **1** open, **2** closed for want of Etin (section 8) |
| `+0x55` | `byBuyHouse` | 1 once the house is bought |
| `+0x47c` | (8 × `u16 key, u16 level`) | the NPC levels (section 4); a slot's place is its `byNumber` |
| `+0x9c` | `EtinReturnCnt` | Etin returns since the last ranking (section 3) |

`CGuild::EnableGuildHouse`: **ranking below 31, `byUseHouse` 1 and `byBuyHouse` set**. Database: `GuildDetails` (`UseHouse`,
`BuyHouse`, `Rank` 31, `Etin`, `EtinReturnCnt`, `KeepEtin` 2000, `Remark`) and `GuildNpcLv` (`GuildID`, `GNpcType`, `NpcLevel`,
`Number`, `CreateTime`, `Del`).

**At login and on joining** (`PacketUserDBChar` 0x46d659, `JoinOK`, `JoinUser`): after the member list, the guild bank (`0x0D29`,
part 3) and **`0x0D24`** (`SendNpcLvList`): `u8 count`, then per used slot in slot order `u16 key, u8 level, u8 slot` (3 + 4n bytes),
**not sent when no slot is used**.

## 2. `GuildHouse.ini`

Confirmed, `CGuildMgr::InitGuildHouseIniFile`.

Read with `GetPrivateProfileInt` / `String` into tables that hold built-in values first; a missing key keeps the built-in one. Number
lists are cut at commas (`atoi` for whole numbers, `atof` for rates), up to the count below.

| section, key | count | table | built in | shipped |
|---|---|---|---|---|
| `DefaultSet` `HouseBuyMoney` | 1 | `g_GuildHouseBuyMoney` | 50,000,000 | 50,000,000 |
| `DefaultSet` `HouseKeepEtin` | 1 | `g_GuildHouseDefaultKeepEtin` | 2,000 | 2,000 |
| `GateKeeper` `KeepEtin` / `UpgradeEtin` | 3 / 3 | key 2 | 200, 500, 800 / 1000, 1500, 2000 | the same |
| `GateKeeper` `MoveDecreRate` | 3 | gate rates, levels 0–3 | 0, 0, 0, **0** (level 3 is never read from the file) | 0.0, 0.0, 0.0 |
| `ShopDefault` `BuyDecreRate` | 4 | shop rates, levels 0–3 | 0.99, 0.98, 0.95, 0.90 | 0.95, 0.90, 0.85, 0.80 |
| `ShopDefault` `BuyPotionDecreRate` | 4 | potion rates, levels 0–3 | 0.90, 0.80, 0.70, 0.60 | the same |
| `ShopWeapon` `UpgradeEtin` | 3 | key `0x0101` | 100, 200, 500 | the same |
| `ShopDefen` `UpgradeEtin` | 3 | key `0x0201` | 100, 200, 500 | the same |
| `General` `UpgradeEtin` | 3 | key `0x0301` | 50, 100, 200 | **150, 200, 400** |
| `ShopPosion` `UpgradeEtin` / `KeepEtin` | 3 / 3 | key `0x0401` | 2000, 5000, 10000 / 50, 200, 500 | (none: built in) |
| `Bank` `KeepEtin` / `UpgradeEtin` | 5 / 5 | key 6 | 100, 200, 500, 800, 1000 / 200, 500, 1000, 2000, 5000 | the same |
| `Bank` `PermitSlot` | 6 | bank slots, levels 0–5 | 40, 80, 120, 160, 200, 240 | the same |
| `Smith` `KeepEtin` / `UpgradeEtin` | 5 / 5 | key 3 | 100, 200, 500, 800, 1000 / 500, 1000, 2000, 10000, 15000 | keep **…, 800, 1500** |
| `Smith` `RepairDecreRate` | 6 | repair rates, levels 0–5 | 0.99, 0.95, 0.95, 0.90, 0.90, 0.90 | the same |
| `Smith` `RapiceMixDecreRate` | 6 | mix price rates, levels 0–5 | 1.0, 1.0, 0.95, 0.95, 0.95, 0.90 | **0.90, 0.80, 0.70, 0.60, 0.50, 0.10** |
| `Smith` `RapiceMixPercentRate` | 6 | mix bonus, levels 0–5 | 0, 0, 0, 0, 1, 2 | **0, 0, 1, 1, 2, 2** |
| `NpcLevel_Light` / `NpcLevel_Dark` `GateKeeper1..3`, `ShopWeapon1..3`, `ShopDefen1..3`, `ShopGeneral1..3` | 1 each | the NPC lists (below) | Light 37–39, 196–198, 200–202, 200–202; Fury 41–43, 207–209, … | below |

**The NPC keys**: 2 gatekeeper, 3 blacksmith, 6 warehouse (bank), and the merchants `shop type << 8 | 1`: `0x0101` weapons, `0x0201`
armour, `0x0301` general goods, `0x0401` potions (the merchant's shop type is `stNpc +0x124`, `world.npcs.merchant_byte`).

**The NPC lists** (`0x541cb0`): per country (Light, Fury), per kind (0 gatekeeper, 1 weapons, 2 armour, 3 general goods) and level 1 to
3, three NPC type ids, of which the file sets the first (the other two stay 0). Shipped:

| | gatekeeper | weapons | armour | general goods |
|---|---|---|---|---|
| Light (map 51) | 37, 38, 39 | 196, 198, 197 | 200, 201, 202 | 220, 221, 222 |
| Fury (map 52) | 41, 42, 43 | 207, 209, 208 | 210, 212, 213 | 223, 224, 225 |

**The house maps** carry every level's NPC at once (map 51: merchants 195–204 and 220–222, gatekeepers 36–39, blacksmith 35, warehouse
27, NPCs of types 10 and 11; map 52: 206–215, 223–225, 40–43, 38, 28, …). A listed NPC is **locked** until the guild has its level;
the unlisted ones (195, 199, 203, 204, 36 on map 51) are the level-0 NPCs everyone may use.

## 3. Returning Etin: `0x0D35`

Confirmed, `PacketGuild` case `0x0D35`.

Client → server: `u32 npc id` (**not looked at**). Ignored without a guild. Otherwise every item of **type 43** in bags 1 to the
player's bag count, slots 0 to 23, is taken out of the bag (each is removed: database `0x0702`, `0x0415` to the game log), and its Etin
is **count × 1, 10, 100 or 1000** for `special` 52, 53, 54, 55 (the four shipped Etin, 43/1 to 43/4; another special adds 0). The
player gets **`0x0D35`**: `u8 count`, then per item `u8 bag, u8 slot` in that order (3 + 2n bytes; with nothing, `0x0D35 00`). If the
sum is above 0: the guild's Etin grows by it, **its Etin return count by 1**, the guild's online members get **`0x0D23`** (`u32
Etin`, `CGuild::SendEtin`), and the database is told (`0x0B12`: guild, Etin, the sum, the character, the return count).

## 4. NPC levels and upgrades: `0x0D25`

Confirmed, `PacketGuild` case `0x0D25`, `CGuild::NpcUpgrade`.

Client → server: `u16 key, u8 level` (5 bytes). Ignored without a guild or below rank 2 (rank 1 or 2 only). The answer `0x0D25`
(7 bytes: `u8 result, u16 key, u8 level, u8 slot`) goes to the player unless the upgrade succeeds. In order:

1. **Result 3** when the guild's ranking is 31, its `byUseHouse` is not 1, it has not bought the house, or `level` is 0 or above 5.
2. By key: `0x0101`, `0x0201` and 2: **3** for a level above 3. **3** (blacksmith): **4** when the ranking is above **14**. 6 (warehouse)
   and `0x0301`: **4** when the ranking is above 30 (never, after step 1). `0x0401`: **4** above **6**, then **3** for a level above 3.
   Any other key: **3**.
3. The Etin to spend is **Etin − keep Etin**: **1** when that is below 1. The cost is the key's `UpgradeEtin` for `level`: **1** when
   it is above what can be spent; **3** when it is 0 (no such level).
4. The guild's slot of that key: **2** unless `level` is its level + 1 (one step at a time). No slot: **3** unless `level` is 1, then
   the first empty slot takes `(key, 1)` (with all eight used, the request is lost: nothing more happens and nothing is answered).
5. Done: the slot's level is set, **every online member gets `0x0D25 (0, key, level, slot)`**, the keep Etin is worked out again
   (section 6), the Etin goes down by the cost, **the requester** gets `0x0D23` with the new Etin, and the database is told (`0x0B0F`:
   guild, key, level, slot, Etin, keep Etin).

## 5. Buying the house: `0x0D34`

Confirmed, `PacketGuild` case `0x0D34`.

Client → server: `u32 npc id` (**not looked at**). The answer is `0x0D34` (7 bytes: `u8 result, u32 money`, the player's money):

- **3** without a guild or below the master (rank 1);
- **1** when the guild has bought it already;
- **2** when the player has less than `HouseBuyMoney`;
- **0**: bought (`byBuyHouse` 1), the money goes down by `HouseBuyMoney` (saved), the keep Etin becomes `HouseKeepEtin` (2000), the
  database is told (`0x0B10`: guild, keep Etin) and the game log (`0x0B08`). Nothing else changes: **`byUseHouse` stays as it was**
  (0 for a guild never ranked with a house), so the house opens at the next ranking (section 8). The client's 4 ("ranking
  requirement", 8030) is never sent.

## 6. The keep Etin

Confirmed, `CGuild::CalculateKeepEtin`.

**`HouseKeepEtin` + the `KeepEtin` of each used slot's key at its level** (the gatekeeper at level 2 adds 500; the armour and weapon
merchants and general goods add nothing). Worked out after each upgrade; a purchase sets it to `HouseKeepEtin` without looking at the
slots. `0x0D33` (`u32 keep Etin`, `CGuild::SendHouseKeepEtin`) is never sent (no caller), and the client ignores it.

## 7. Going in

Confirmed, `0x020A` case 6, `GetZoneByMapID` case 6, `ItemUse` special 56.

**A portal to a map of `CreateType` H** (6; maps 51 and 52, from map 35 portal 2 and map 53 portal 2 to 51 (country 1), map 36 portal 2
and map 54 portal 2 to 52 (country 2)), after the usual portal checks (`docs/original/portals.md` section 2), answers `0x020A u8 portal,
u8 reason` and nothing else:

| check, in order | reason | client |
|---|---|---|
| no guild | 0 | guild text |
| ranking 31 | 3 | "Your Rank isn't high enough." kind |
| house not bought | 6 | |
| `byUseHouse` 0 | 7 | |
| `byUseHouse` 2 | 8 | |
| `byUseHouse` 1 | the move (no answer) | |

(The client prints 0–3 and 6–8 from its guild-house texts, `docs/client/CLIENT-FACTS.md`.)

**The zone** is found when the player arrives: key = **the guild's id**, when the guild exists and `EnableGuildHouse` holds; otherwise
there is no zone and the connection is closed. A guild's zone is made from the map as any zone is (its own NPCs; the house maps have no
monsters) and released as party zones are (`ExpireTime` 2: 2.5 minutes after it is empty, `docs/original/instances.md` section 6).
Counted apart from party zones. **Nothing moves a player out** when the guild loses its rank or its house while they are inside.

**The Guild House Teleport Stone** (special 56, `g_ItemSpecialMoveGuildHouse`): Light **map 51 (491.49, 41.84, 326.29)**, Fury **map 52
(482.37, 41.84, 321.91)**, with the checks of the other moves by item (`docs/original/item-use.md` section 9: `ChkMoveMap`, the portal
country), then: the map must be of type 6, the player in a guild that passes `EnableGuildHouse`, no move pending. Then as the other
stones: the move 5 s later (type 3), `0x0221` to those within 60; the stone is used up when the move happens.

Logging in or coming back to life on a house map: the map's `RebirthMapPos` (51: map 53 (50.43, 7.09, 86.57); 52: map 54 (97.33, 5.89,
42.03)), as for every map with a `CreateType` (`docs/original/instances.md` section 6).

## 8. The ranking update

Confirmed, ; completed in `docs/original/guild-ranking.md` sections 3 and 8.

`CWorldSendThread::Run` asks every 60 s whether the first map of `CreateType` R (map 50, `CreateTime (0, 21, 1)`: Sunday 21:00–22:00,
local time; only its first **two** time entries are read) is open; **30 minutes after it closes**, `CGuildMgr::UpdateGuildPointRank`:

- each guild's score is **battle points + min(Etin returns + 25 × a third counter, battle points / 2)** (`+0x98`, `+0x9c`, `+0xa0`;
  0 with no battle points), the three counters go back to 0, and the score becomes `dwGuildPoint`;
- a score of 0: ranking 31, `byUseHouse` 0;
- per country, highest score first, places 1 to 30 get that ranking; one with a house: **Etin below the keep Etin → `byUseHouse` 2;
  otherwise the keep Etin is taken, `byUseHouse` 1** and its online members get `0x0D23`. Places 31 and on: ranking 31, `byUseHouse` 0;
- the database is told (`0x0B16`, 30 guilds a packet: id, points, Etin, ranking, `byUseHouse`) and each country's players get `0x0D37`
  (the new top 30).

So **a house opens, and its upkeep is paid, only at a ranking**, and a guild that earned no points in the battle loses its rank and its
house's opening. The battle points come from the ranking battle (`CGuild::SendRankingZone`, monster drops in the ranking zone).

## 9. What a house's NPCs do differently

Confirmed.

Only on a map of type 6, and only by the player's own guild:

- **Buying** (`0x0702`, `docs/original/shop.md` section 2): no guild → `0x0D28 (1, 30)`. Otherwise `GetGuildHouseShopBuyRate(NPC type
  id, shop type)`: the level of key `shop type << 8 | 1` (0 without a slot, at most 3; a higher stored level is written back as 3), the
  rate is the potion rate (shop type 4) or the shop rate at that level. Then, by shop type: **1, 2** (weapons, armour): the NPC is
  locked when it is in a higher level's list (level + 1 to 3) of the guild's country → `0x0D28 (2, that level)`; **3** (general goods):
  ranking above 30 → `0x0D28 (1, 30)`, else locked as for 1 and 2; **4** (potions): ranking above 6 → `0x0D28 (1, 6)`, no lists. Allowed:
  the price is **trunc(`nBuy` × rate) × count** (the x87 is set to truncate, as for every rate below). The rest as everywhere.
- **Gatekeepers** (`0x020C`): no guild → `0x0D28 (1, 30)`; a gatekeeper of a higher level than the guild's (key 2, lists as above, kind
  0) → `0x0D28 (2, that level)`; otherwise trunc(cost × the gate rate) (all 0 shipped: **free**). The move as usual.
- **Repair** (`0x0554`): no guild, or ranking above **14** → `0x0D28 (1, 14)`; otherwise trunc(price × the repair rate) at the
  blacksmith's level (key 3, at most 5), at least 1 (one item: its truncated price; everything: the summed price).
- **Linking lapis** (`0x0801`, `docs/original/lapis.md` section 3): no guild or ranking above 14 → `0x0D28 (1, 14)` (before the money
  check); the price `P` becomes trunc(`P` × the mix price rate) at the blacksmith's level; and, when the guild passes
  `EnableGuildHouse`, the rate `R` + the mix bonus at that level, after the hammer and before the 1 to 50 limit.
- **Extracting lapis** (`0x0802`, both forms, `ItemGemRemoveAll` 0x462107, `ItemGemRemovePos` 0x4634e0): no guild or ranking above 14
  → `0x0D28 (1, 14)`; the fee becomes trunc(fee × the mix price rate). (No bonus.)

These checks do not ask whether the guild's house is open: inside the house it is the player's own guild's (the zone key), but an
operator moved in by `FA12` would also be served.

## 10. Original oddities

- **`0x0D34` and `0x0D35` do not check the NPC**: a house can be bought, and Etin returned, anywhere.
- **A purchase does not open the house**: the guild waits until the next ranking (which also takes the first upkeep).
- **A full slot table loses the upgrade request silently** (step 4 of section 4).
- **The keep Etin of a purchase ignores the NPC levels** (it is set to `HouseKeepEtin`; an earlier house's levels stay, but a guild
  buys once).
- **`GetGuildHouseShopBuyRate` writes a level above 3 back as 3** into the guild's slot.
- **An upgrade's new Etin goes to the requester only**, while an Etin return tells the whole guild.

## 11. The new server

- **Data**: `tools/world/import.py` reads `GuildHouse.ini` into **`world.guild_house`** (one row: buy money, keep Etin, the rate lists,
  the bank slots) and **`world.guild_house_npcs`** (key, keep list, upgrade list) and **`world.guild_house_levels`** (country, kind,
  level, NPC type id), with the built-in values of section 2 for missing keys.
- **Database** (`db/migrations/0019_guild_house.sql`): `guilds` gains `ranking` (31), `points`, `etin`, `etin_returns`,
  `keep_etin` (2000), `use_house` (0), `buy_house` (false); **`guild_npc_levels`** (`guild_id`, `npc_key`, `level`, `slot`), one row per
  used slot.
- **In memory** (`crate::guild`): each guild keeps its ranking, points, Etin, Etin returns, keep Etin, house state and eight slots;
  `crate::guild_house` holds the pure rules (sections 3 to 6 and 9: the upgrade's answer, the keep Etin, the rates and locks);
  `server/guild_house.rs` the requests and the house NPCs' rates for the shop, gatekeeper, repair and lapis code.
- **Merchants' shop types**: `world.npcs.merchant_byte` of type-1 NPCs.
- **The teleport stone** (special 56): its checks at use; the guild's zone is found or made when the move is due.
- **Packets**: `0x0D23` with the guild's Etin; `0x0D24` at login and on joining; `0x0D25`, `0x0D34`, `0x0D35` as sections 3 to 5;
  `0x0D28` for the house NPCs.
- **House zones**: the instance zones of `docs/original/instances.md` section 18, owned by a guild instead of a party (zone key
  `map | n << 8`, the guild's zone of the map looked up by guild id). The portal (section 7's order) and the teleport stone **book**
  the player into the guild's zone, made then if needed, as party zones are; arrival places the player in the booked zone. A guild that
  is deleted while its zone lives keeps the zone until it empties.
- **Ranking update**: built with part 2 (`docs/original/guild-ranking.md`). The guild bank is part 3, built (spec
  `docs/original/guild-bank.md`, build plan `docs/plans/done/guild-bank.md`). The operator commands are built with the GM commands
  (`docs/original/gm-commands.md` sections 8.3 and 8.6, deviations 8 and 10; `server/admin.rs`): `FA12` into a guild's house or ranking
  zone (`Zones::gm_zone`), `FA14` the ranking (saved), `FA15` the points.
- **Deviations** (all approved):
  1. ~~**No ranking update until the ranking battle exists**~~ (approved; done: part 2 is built, `docs/original/guild-ranking.md`; the ranking
     update runs after each battle). Before it, rankings and house states stayed as stored.
  2. **`0x0D34` and `0x0D35` need the NPC**: an NPC in the player's view, as for buying (the original takes the id and ignores it).
     The client sends them from an NPC's menu.
  3. **An upgrade sends the new Etin (`0x0D23`) to every online member**, as an Etin return does, not just to the requester.
  4. **A full slot table answers 3** ("failed to upgrade.") instead of nothing (none of the 7 shipped keys can fill eight slots, so
     this never shows).
  5. **Nobody is disconnected for a house that stopped qualifying on the way**: a portal user is booked into the guild's zone when
     the portal is used and arrives there (as party zones, `docs/original/instances.md` deviation 2); the teleport stone looks the
     zone up when its move is due and, if the guild no longer qualifies, fails as a stone whose item is gone (`0x0222` with 0). The
     original looks the zone up on arrival and closes the connection when there is none.
  6. **The blacksmith's `0x0D28` comes before the request's own checks** for linking and extraction (the original sends it only for a
     request it would otherwise carry out); an invalid request in a house without the ranking gets `0x0D28` instead of nothing.
  7. **The Etin return and the house purchase are each one database transaction** (audit R16). The return takes the Etin items out of
     the bags and adds their sum to the guild's Etin (to 32 bits) with one more return, and only while the player is still a member (a
     join request is not membership) of a live guild; the purchase takes `HouseBuyMoney` from the master's money and sets the house flag
     and the keep Etin, and only while the player is the guild's master and the house is not bought. The guild in memory is changed after
     the commit and nothing is queued for the writer. A return with no guild takes nothing out of the bags; an Etin item worth 0
     (a special outside 52 to 55) stays in the bags instead of being destroyed for nothing. Both start by ending a trade, a duel stakes
     window and a personal shop (audit R3). A player who left or was kicked while the request was on its way, or a guild disbanded
     meanwhile, gets no answer to the return and "not the master" (`0x0D34 3`) for the purchase; nothing is taken.
