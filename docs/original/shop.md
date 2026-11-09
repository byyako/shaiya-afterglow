# NPC shops: buying (`0x0702`) and selling (`0x0703`)

This page covers buying from and selling to merchant NPCs in the EP4.5 game server `ps_game.exe` (built 2009-07-15): `CUser::PacketShop`
(0x46af10) and what it calls. It comes from static analysis; nothing was executed. The client's side agrees on every packet size (see
the [packet catalogue](../client/client-packets.generated.md)).

## In short

- **Buying** takes an item from the merchant's list by its position. Stackable items are added to an existing stack in the bags when
  they fit, otherwise to the first empty slot. The answer says "bought", "not enough money" or "no room".
- **Selling** works anywhere, since the request names no NPC. Quest items sell for nothing.
- **The merchant can be anywhere on the map.** The original only checks that the NPC is on the same map, not that the player is near it.
  The new server requires the merchant to be in view (section 4).
- **The price can wrap.** The price is the count times the item's price in 32 bits, so a large count of an expensive item could cost
  almost nothing (see [Known bugs and community fixes](community-fixes.md)). The new server computes it in 64 bits (section 4).

Status words: **confirmed** = read in the code (address given), as on [the game server page](ps_game.md).

**`CUser` offsets.** The PDB's `CUser` layout is 4 bytes lower than what the code uses from the item array on (`m_pUserItem` is at
`+0x1c0` in the PDB, `+0x1c4` in the code; `m_dwMoney` at `+0x140` agrees). The names below follow the PDB; the offsets are the code's.
Other pages refer to this shift.

## 1. Data the handler uses

Confirmed.

| what | where |
|---|---|
| the player's money | `m_dwMoney` (`+0x140`), u32 |
| the items | `m_pUserItem` (`+0x1c4`): 6 bags x 24 slots, `CItem*`, bag 0 the equipment |
| how many bags are usable | `m_nMaxBag` (`+0x1304`): **2** after `CUser::InitEquipment`; a passive skill raises it to a value from the skill definition (`CUser::AddSkillPassive`), removing it sets 2 again (`RemSkillPassive`) |
| an item definition | `CGameData::GetItemInfo(type, type id)` (0x402710): both 1 to 255 and a loaded definition. `stItemInfo`: `byType` `+0x24`, `byTypeID` `+0x25`, `byCount` `+0x4b` (stack limit), `wQuality[0]` `+0x4c`, `nBuy` `+0x80`, `nSell` `+0x84`, `nRealType` `+0xa0` |
| `nRealType` | set at load (`CGameData::LoadConstItem` → `GetItemRType` 0x4bdad0): the number (1 to 27) of the table in `LITEM_TYPE` (0x542188) that lists the item's type. Table 27 holds types **27, 28, 29** |
| a merchant's goods | `CNpcData::GetShopNpc(type id)` (0x466c30): merchants (NPC type 1) by type id, 1 to their count; `stNpcShop`: `nShopItemCount` `+0x128`, `pShopItem` `+0x12c`, pairs of `u8 type, u8 type id` (the merchant's list in the NPC data, in file order; in the new server `world.npc_shop_items`) |
| an NPC by object id | `CZone::FindNpc` (0x416d10): a lookup in the player's zone, i.e. **anywhere on the player's map; no distance check** |

A new item (`CItem::Init` 0x4b9f80) gets the type, type id and count, **quality = `wQuality[0]`** of the definition, no gems, no craft
name, the make time, **make type `'B'`**, and a new unique id (`SUniqueIDGenByServer::GenID`); then `CItem::InitCraftExpansion` gives a
craftable item the craft name of 20 zeros (see [New items and crafting](item-origins.md)).

## 2. Buying: `0x0702`

Confirmed. Client → server, 8 bytes: `u32 npc object id, u8 index` (position in the merchant's list, from 0) `, u8 count`.

Nothing is answered when (in this order): the NPC is not in the player's zone; it is not a merchant (NPC type 1); its shop is not found;
`index` is not below the shop's item count; the item definition does not exist; `count` is above the definition's stack limit or 0. (A
first check on `+0x142c == 1`, most likely a status such as dying, also ignores the packet.)

Then the player's trade and personal shop are cancelled (`CUser::CancleActionExc`, `stMyShop::Ended`) and the price is `count * nBuy`, a
32-bit multiplication (`imul` at 0x46b55a). (On a guild house map, map type 6, a guild discount from
`CGuild::GetGuildHouseShopBuyRate` applies instead, and a player without a guild gets `0x0D28` and nothing else; see
[Guild houses](guild-house.md#9-what-a-houses-npcs-do-differently).)

- **Not enough money** (`money < price`): result **1**.
- **Stack limit 1** (`byCount < 2`): the **first empty slot**, bags 1 to `m_nMaxBag` and slots 0 to 23 in that order, gets a new item
  with count 1.
- **Otherwise:** first a **stack to add to**: the first item in the same bag order with the same type and type id whose count plus
  `count` is at most the stack limit; its count grows. If there is none, the first empty slot gets a new item with `count`.
- **No room:** result **2**.

On success the money is lowered by the price (and saved, `CUser::SendDBMoney`), the item is saved (to ps_dbAgent: `0x0701` new item,
`0x0707` new count) and logged (`0x0403` to the game log).

Server → client, `0x0702`, 12 bytes: `u8 result, u8 bag, u8 slot, u8 type, u8 type id, u8 count, u32 money`.

| result | bag, slot | count | money |
|---|---|---|---|
| 0 bought | where the item is | the slot's count now | the money left |
| 1 not enough money | 0, 0 | the requested count | 0 |
| 2 no room | 0, 0 | 0 | 0 |

## 3. Selling: `0x0703`

Confirmed. Client → server, 5 bytes: `u8 bag, u8 slot, u8 count`.

Nothing is answered when: `bag` is 0 or above `m_nMaxBag`; `slot` is above 23; the slot is empty; `count` is above the item's count or
0. Then the trade and personal shop are cancelled as above.

The money is `count * nSell`, **none for real type 27** (types 27 to 29). If adding it would go past the u32 maximum, nothing happens and
nothing is answered. Otherwise the money grows (and is saved); the whole item is removed when `count` is all of it, otherwise its count
drops by `count`. Saved with ps_dbAgent `0x0702` (`u32 user, u8 bag, u8 slot, u8 count sold`), logged as `0x0404`.

Server → client, `0x0703`, 12 bytes: `u8 0, u8 bag, u8 slot, u8 type, u8 type id, u8 count left` (0 when sold out) `, u32 money`.

## 4. The new server (`crates/game-server/src/inventory.rs`, `server.rs`)

Follows sections 2 and 3, with these differences:

- **The merchant must be in the player's view** (the 5 x 5 cells), not just on the same map, so a modified client cannot buy from anywhere.
  Selling stays possible anywhere, as in the original: `0x0703` names no NPC.
- Prices are computed in 64 bits; a price that does not fit the money is "not enough money".
- Every change is written to PostgreSQL at once, money and item in one transaction (`characters.money`, `character_items`), before the
  reply. If the database refuses, the connection is closed rather than letting memory and storage disagree.
- `m_nMaxBag` is 2 until a bag passive skill is learnt ([Skills 1](skills.md), section 5).
- In a guild house the house's prices apply, or the refusal `0x0D28` ([Guild houses](guild-house.md), section 9). A purchase
  and a sale cancel the player's trade and personal shop (`Zones::cancel_actions`).
- A purchase and a sale are written to the game log as `0x0403` and `0x0404` ([The game log](game-log.md)).

## 5. Open questions

- The `+0x142c` state that ignores shop packets (likely dying or dead).
- What the original does with items in bags above `m_nMaxBag`.

## 6. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **The same buying and selling** (`PacketShop` EP3 ps_game 0x451810, matched by dispatch, `0x0702` and `0x0703`). The buy price is the
  same 32-bit product ([`npc-shop-price-wrap`](community-fixes.md#npc-shop-price-wrap); `imul` at EP3 ps_game 0x451e70, inferred to be
  the same path as EP4.5 0x46b55e).
- **The guild-house discount** also calls `CGuild::EnableGuildHouse` in EP3; EP4.5 does not call it there (confirmed at call level).
- Data (confirmed, `NpcQuest.SData`): EP4.5 adds 76 merchants, and 8 shared merchants differ (EP4.5 adds items of types 28, 44 and 100).

### EP5.4

- **The same program** (confirmed): `PacketShop` (EP5.4 ps_game 0x479220, matched by opcode switch table) finds the NPC anywhere in the
  zone (no distance), checks the same type, index and count, applies the guild-house rate, and computes the buy price as a 32-bit `imul`
  (EP5.4 ps_game 0x479871): the price wrap is not fixed. Selling multiplies `count × nSell` in each branch as EP4.5 does (EP5.4 ps_game
  0x479311 and on).
- The new item's unique id comes from EP5.4 ps_game 0x464840 instead of `SUniqueIDGenByServer::GenID`, and a function at EP5.4 ps_game
  0x468080 runs after each purchase; neither was read.
- **Placement** (confirmed): `CMap::InitNPC` (EP5.4 ps_game 0x42fb50) skips merchants of shop kind 7 or 8 (0x42fbbc to 0x42fbd1), so
  the eight capital cloak merchants of maps 35 and 36 are absent; why was not found.
- Data (confirmed, `NpcQuest.SData`): 44 merchants get 791 more entries, mostly consumables and level-38 to 48 jewellery and armour.
