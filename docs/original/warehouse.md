# The warehouse: `0x0711` and the moves of `0x0204` with bag 100

This page covers the warehouse (the bank) in the EP4.5 game server `ps_game.exe`: what it holds, the three moves (bag
to warehouse, warehouse to bag, warehouse to warehouse), the fee, the second half of the warehouse, and `0x0711`. It was read in the
Ghidra decompiler and the disassembly with the PDB names. The client side is on [World packets](../client/client-world-packets.md)
(`0x0204`, `0x0711`) and in the [client facts](../client/CLIENT-FACTS.md) (the NPC warehouse and the Warehouse Recall Rune).

On other pages: the guild warehouse (bag 255, `CGuild::ItemBagToGuildBank` and the others; [Guild bank](guild-bank.md)) and the
premium accounts that can also set the warehouse flags ([Item mall](item-mall.md#7-premium-accounts)). Not covered: bag 200 (the client knows it, the server has no handler for it).

## In short

- **One warehouse per account.** All 240 slots are shared by every character of the account on that game server.
- **Putting in is free; taking out costs 5 %** of the stack's sell price, except for the guild bars. Equipped items and quest items
  cannot go in directly.
- **The second half** (slots 120 to 239) can only be filled while a Double Warehouse buff runs. Items there can still be taken out
  after it ends.
- **No money in the warehouse.** Warehouse money is loaded and sent, but nothing in this build can change it.
- **No keeper check.** The server never checks that the player is at a warehouse keeper or has the Warehouse Recall buff; only the
  client limits where the window opens.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's from `m_dwBankMoney` on;
see [Entering the world](enter-world.md)); `stItemInfo` as on [NPC shops](shop.md) (`byType` `+0x24`, `byCount`
`+0x4b`, `nSell` `+0x84`, `nRealType` `+0xa0`).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/client-world-packets.md` (`0x0204`, `0x0711`) and `docs/client/CLIENT-FACTS.md` (the
NPC warehouse and the Warehouse Recall Rune). `CUser` offsets are the code's (4 above the PDB's from `m_dwBankMoney` on,
`docs/original/enter-world.md`); `stItemInfo` as in `docs/original/shop.md` (`byType` `+0x24`, `byCount` `+0x4b`, `nSell` `+0x84`, `nRealType`
`+0xa0`).

Covered: what the warehouse holds, the three moves (bag to warehouse, warehouse to bag, warehouse to warehouse), the fee, the second half of
the warehouse, and `0x0711`. **Not covered:** the guild warehouse (bag 255, `CGuild::ItemBagToGuildBank` and the others: `docs/original/guild-bank.md`, built),
the premium "billing" items that can also set the warehouse flags, and bag 200 (the client knows it, the server has no handler for it).

## 1. What the warehouse is

Confirmed.

- **240 slots** (`CUser::m_pUserBank` `+0x404`, `CItem*[240]`), bag number **100** in the packets. `CUser::IsBank(bag, slot)` (0x45b3f0):
  bag 100 and slot below 240.
- It belongs to the **account** on its game server: ps_dbAgent loads it by user (`TP_DB_INFO_BANK`: `u32 dwUserID`, `DB_BANK_UNIT[50]` per
  packet) and saves each move by user (`TP_DB_ITEM_BAGTOBANK` and the others carry `dwUserID`, no character id); the original tables are
  `UserStoredItems` and `UserStoredMoney` in `PS_GameData`, which is one database per game world. Every character of the account sees the
  same warehouse.
- A bank item keeps all of an item: `DB_BANK_UNIT` = `u64 uid, u8 slot, u8 type, u8 type id, u16 quality, u8 gems[6], u8 count, u32 made at,
  u8 make type, char craft name[21]`.
- **Warehouse money** (`m_dwBankMoney`, code `+0x148`) is loaded with the character (`TP_DB_INFO_CHAR` `+0x1f`), sent in `0x0711` and written
  to the game log on leaving (`CUser::LeaveWorld`), and **nothing in this build changes it** (no other reads or writes of `+0x148` in the
  `CUser` code). The deposit and withdrawal of money that later episodes have does not exist here.

## 2. The contents: `0x0711`

Confirmed, `CUser::SendCharacterBank` 0x483410.

Server → client, `TP_CHAR_BANK`: `u16 0x0711, u32 bank money, u8 count`, then `count` × `TP_BANK_UNIT` (33 bytes): `u8 slot, u8 type,
u8 type id, u16 quality, u8 gems[6], u8 count, char craft name[21]` (note: count **after** the gems, unlike `0x0106`). Slots in order 0 to 239,
**50 per packet** (a full packet is 1657 bytes, the last one `7 + 33n`), nothing at all when the warehouse is empty.

Sent once, in the enter-world burst right after the items (`0x0106`; `docs/original/enter-world.md` section 5, row 7). It is not sent again
when the window opens: the client opens the window from the NPC menu or the rune without a packet and shows what `0x0711` and the later
`0x0204` answers left it.

## 3. The request: `0x0204`

Confirmed, `CUser::PacketMaininterface` 0x468475.

`u8 from bag, u8 from slot, u8 to bag, u8 to slot` as for bag moves (`docs/original/items.md` section 1): ignored while the status is 1 or the
shape is 100, or when both places are the same; then the trade is cancelled (`CancleActionExc`) and the personal shop closed. Then:

| from | to | further checks (otherwise nothing, no answer) | handler |
|---|---|---|---|
| warehouse | warehouse | a target slot **120 or above** needs `m_bSkillBank2X` (`+0x58cc`) | `ItemBankToBank` |
| warehouse | bag (`IsBag`) | the bag slot is **empty** | `ItemBankToBag` |
| bag | warehouse | a target slot 120 or above needs `m_bSkillBank2X`; the warehouse slot is **empty** | `ItemBagToBank` |
| bag | bag | | `ItemBagToBag` |
| other | | | the guild warehouse (`docs/original/guild-bank.md` section 5, built there) |

**Nothing checks that the player is at a warehouse NPC or has the Warehouse Recall buff** (`m_bSkillEnableBank`): the client only offers the
window there, but a modified client can move items in and out of the warehouse anywhere. The source slot is never checked against 120, so
items in the second half can be taken out after the buff ends.

`m_bSkillBank2X` is set by a buff with ability **43** (the Double Warehouse item; `docs/original/item-use.md` section 7), and
`m_bSkillEnableBank` by ability **39** (the Warehouse Recall Rune, skill 228).

## 4. Bag to warehouse

Confirmed, `CUser::ItemBagToBank` 0x45c4e0.

Nothing happens (no answer) when the bag is 0 (the equipment: equipped items cannot go straight in), the slot is empty, or the item's real
type is **27** (item types 27 to 29: quest and similar items).

Otherwise the item leaves the trade window if it was offered (`ExchangeItemRemove`) and goes, whole, into the warehouse slot (the dispatcher
made sure it is empty; the handler's swap branch is never reached). ps_dbAgent `0x0704` (`TP_DB_ITEM_BAGTOBANK`), game log `0x040B`. No fee.

Answer **`0x0204`, 70 bytes** (`TP_ITEM_BAGTOBAG`): the bag place as it is now (empty), then the warehouse place (bag 100) with the item.

## 5. Warehouse to bag

Confirmed, `CUser::ItemBankToBag` 0x45bf30.

Nothing happens when the warehouse slot is empty or the bag is 0 (no equipping straight from the warehouse).

**The fee**: nothing for item type **94** (`byType` `'^'`: the guild bars, Vault Guild Copperbar and the others, sold for up to 100,000,000);
otherwise `count × nSell` (32-bit), converted to a `float`, multiplied by the `float` constant `0.05f` (0x53a4b0, 0.0500000007…) in the x87 unit
and **truncated** (`fistp` with the rounding control set to chop): in effect 5 % of the stack's sell price, rounded down. Nothing happens when the
player's money is below it. (The largest `count × sell` in the item data is 937,200,000, so the 32-bit product never wraps; above 2^24 the
`float` rounds the product to a multiple of a few units first, which can move the fee by one.)

Then the item goes into the bag slot (empty, by the dispatcher; the swap branch is again not reached), the money is lowered by the fee and saved
(`SendDBMoney`, ps_dbAgent `0x060B`), ps_dbAgent `0x0705` (`TP_DB_ITEM_BANKTOBAG`), game log `0x040C`.

Answer **`0x0204`, 74 bytes** (`TP_ITEM_BANKTOBAG`): the warehouse place (bag 100, now empty), the bag place with the item, then **`u32` the
money now**. The client sets its money from it (`docs/client/client-world-packets.md`, `0x0204`).

## 6. Warehouse to warehouse

Confirmed, `CUser::ItemBankToBank` 0x45ca30.

Nothing happens when the source slot is empty. To an empty slot the item moves. To a taken slot: when both items stack (stack limit above 1)
and are the same type and type id, they combine like bag items (`docs/original/items.md` section 2): all into the target when the sum fits
the stack limit (the source is emptied), otherwise the target holds the limit and the source the rest; otherwise the two swap. ps_dbAgent
`0x0706`. Answer `0x0204`, 70 bytes: the source place, then the target place, both bag 100.

## 7. The client (from `docs/client`)

- The warehouse window opens from a warehouse keeper's menu (window `0x67`; maps 51 and 52 use window `0x72`) and closes when the player walks
  away; or from the Warehouse Recall Rune, whose `0x050A` answer opens it and whose buff (skill 228, ability 39, 300 s) lets the rune reopen it
  without a packet while it lasts; the buff's end (`0x050E`) closes a rune-opened window.
- `0x0204` answers with bag 100 rewrite the window's slots; with source 100 and a target outside the warehouse the client expects the 74-byte
  form with the money.

## 8. The new server

- Storage: `bank_items` (migration 0014), keyed by **account and game server** and slot (0 to 239), with the item columns of `character_items`
  and its `uid` (moving an item between a bag and the warehouse keeps its uid). Warehouse money is not stored: it is always 0, as nothing can
  change it.
- Loaded with the character on entering the world (`PlayCharacter::bank`) and sent as `0x0711` right after `0x0106`, 50 per packet, nothing
  when empty, money 0.
- The connection keeps the warehouse with the bags (`Inventory`); the moves of sections 3 to 6 are decided there and saved, money and slots, in
  one transaction (`ItemStore::trade`, where bag 100 means the account's warehouse) before the answer. The checks, fee, combining and answers
  are as above. Trades end on any move, as for bag moves.
- The buff flags: an applied buff with ability 39 or 43 (value not 0) gives "warehouse anywhere" or the second half (`BuffEffects`).
- **Deviation 1 (approved):** a warehouse move (any of sections 4 to 6) needs a **warehouse keeper (NPC type 6) in the player's view** or the
  **Warehouse Recall buff** (ability 39); otherwise it is ignored like the original's other refusals (no answer). The original checks neither,
  so a modified client can use the warehouse anywhere. NPC type 6 is the warehouse keeper by the NPC data's grouping
  (`docs/original/maps-npcs.md`; 39 such NPCs on 14 maps, e.g. map 1 at (491, 1785), (1560, 1811), (1046, 821)); ps_game itself never looks at
  the type for this.
