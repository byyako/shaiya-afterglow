# Moving items and equipping: `0x0204`

This page covers how the EP4.5 game server `ps_game.exe` moves an item between bags, stacks items, and equips or
unequips one, and the rules for who may wear what. It comes from static analysis; nothing was executed.

## In short

- **One packet does it all.** Dragging an item, whether to another bag slot, onto the character, or onto a matching stack, sends the same
  request. The server answers with both slots as they now are, even when nothing changed.
- **Stacks combine** when both items are the same and stack: everything goes onto the target if it fits, otherwise the target is filled
  and the rest stays behind.
- **Equipping has rules.** The character must be at least the item's level; the item must fit the slot, the character's race, class and
  mode, and sometimes its sex. A weapon and a shield together need a one-handed weapon.
- **Other players see the change**: equipping something in the visible slots (helmet to cloak) is shown to everyone around.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (see [NPC shops](shop.md) on the
PDB's 4-byte shift); `stItemInfo` (an item definition) and `CItem` (an item a character holds) as on that page.

## 1. The request

Confirmed, `CUser::PacketMaininterface` case `0x0204`.

Client → server, 6 bytes (`TPC_ITEM_BAGTOBAG`): `u8 from bag, u8 from slot, u8 to bag, u8 to slot`. Ignored when the player's status is 1, its shape
is 100 (`+0x1354`), or both places are the same. Then the trade and personal shop are cancelled and the request goes by the kinds of bag:

| from → to | handler |
|---|---|
| bag → bag | `CUser::ItemBagToBag` (0x45b690), section 2 |
| bag ↔ warehouse, warehouse → warehouse (bag 100, slots below 240, `CUser::IsBank`) | `ItemBagToBank`, `ItemBankToBag`, `ItemBankToBank` (warehouse NPC needed) |
| guild warehouse | `CGuild::ItemBagToGuildBank` and the others (a guild needed) |

A **bag** (`CUser::IsBag`) is any bag from 0 to `m_nMaxBag` with a slot below 24; bag 0 is the equipment.

## 2. Bag to bag

Confirmed, `CUser::ItemBagToBag`.

Nothing happens when the source slot is empty.

- **From the equipment** (bag 0, slot at most 13) to an inventory bag (not 0): to an empty slot, the item is unequipped; to a taken slot, the two
  are swapped if the other item may be equipped in that equipment slot (`EnableEquipment`), otherwise nothing changes.
- **Inventory to inventory**: if the target holds the same item and both stack (`CItem::EnableCombine`: both stack limits above 1, same type and type
  id), they are combined: everything into the target when it fits the stack limit (the source is emptied), otherwise the target holds the stack limit
  and the source keeps the rest. Otherwise the two slots are swapped (an empty target just receives the item).
- **To the equipment** (to bag 0, slot at most 13): if `EnableEquipment` allows the item there, it is equipped, swapping with what was equipped.

Saved with ps_dbAgent `0x0703`. Answer `0x0204` (`TP_ITEM_BAGTOBAG`, 70 bytes): for the source and then the target place, `u8 bag, u8 slot, u8 type,
u8 type id, u8 count, u16 quality, u8 gems[6], char craft name[21]`, as the slot now is (all zero when empty). The answer is sent even when nothing
changed (a refused equip answers the unchanged slots).

**Equipment changes** also: recompute the stats (`CUser::ItemEquipmentAdd`/`Rem`: attack and defence attributes, `0x051C` speed to the view, max
HP/SP/MP, item skills), and for slots 0 to 7 send **`0x0507`** to everyone in view (`CUser::SendEquipment`, `TP_PC_EQUIPMENT`): `u32 char id, u8 slot,
u8 type, u8 type id, u8 enchant level` (from the craft name), and for the cloak (slot 7) the 6 badge bytes (the cloak's gems): 10 or 16 bytes.
Slot 13 (the mount) stops riding.

## 3. Who may equip what

Confirmed, `CUser::EnableEquipment` 0x45b450.

All of these must hold:

- the item's level (`wLevel`) is at most the character's;
- the item's real type fits the slot:

  | slot | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9, 10 | 11, 12 | 13 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | real type | 16 | 17 | 18 | 20 | 21 | 1 to 15 (weapons) | 19 | 24 | 23 | 22 | 25 | item type 42 |
  | | helmet | upper armour | lower armour | gloves | shoes | weapon | shield | cloak | necklace | rings | bracelets | mount |

  A weapon with a shield equipped, or a shield with a weapon equipped, needs the weapon to be one-handed: real type 1, 3, 7 or 9.
- the item's country (`byCountry`) fits the character's race (`m_byFamily`: 0 human, 1 elf, 2 vail, 3 deatheater): 0 human, 1 elf, 2 human or elf,
  3 deatheater, 4 vail, 5 vail or deatheater, anything else everyone;
- the item allows the character's class (`byJob[job]` not 0);
- the item's mode (`byGrow`) fits the character's (`m_byGrow`): an easy character only easy (0) items, normal easy or normal (0, 1), hard anything
  but ultimate (3), ultimate anything;
- `bySpecial` 64 items only for sex 0, 65 only for sex 1.

## 4. The new server

- Bag to bag as in section 2, with the rules of section 3; the change is saved to PostgreSQL in one transaction before the answer.
- `0x0507` goes to the players in view and the looks other players get (`0x0303`) follow the equipment.
- Not yet: the stats that equipment changes (attack, defence, speed, max HP/SP/MP, item skills), the warehouse and guild warehouse moves, the mount
  slot's riding, the equipment quality bookkeeping (`0x070C` to ps_dbAgent). (Since built: stats `docs/original/stats.md`, the warehouse
  `docs/original/warehouse.md`, riding `docs/original/vehicles.md`, quality `docs/original/repair.md`, the guild warehouse
  `docs/original/guild-bank.md`.)

## 5. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **Moving** (confirmed): the `0x0204` case (EP3 ps_game 0x44f2c3) and `ItemBagToBag` (EP3 ps_game 0x4454c0, matched by call order, 46
  of 46 calls) work as in EP4.5, except that EP4.5 also ignores the move in shape 100 (EP4.5 0x468482) and EP3 does not.
- **Equipping** (confirmed): `EnableEquipment` (EP3 ps_game 0x4452a0, matched by call order) lacks EP4.5's check of the sex-only items,
  specials 64 and 65 (EP4.5 0x45b5f5 to 0x45b613); EP3 has no such items.
- The warehouse's second part starts at slot 40 ([`warehouse.md`](warehouse.md)).

### EP5.4

- **The same rules** (confirmed): `0x0204` branches by bag kind as in EP4.5 (EP5.4 ps_game 0x4760d3), and the `PacketMaininterface` case
  set `0x0204` to `0x0227` is identical. `EnableEquipment` (EP5.4 ps_game 0x468370, matched by call position and an identical compare
  chain) has the same 14 slots, real types per slot, and country, class, mode and sex rules; `ItemBagToBag` (EP5.4 ps_game 0x4685a0,
  matched by the branch arguments) keeps the slot limits 13 and 14 (0x4685e6, 0x46862b).
- **A new freeze during a server transfer** (confirmed for the gate, inferred for its purpose): `0x0204` is ignored while `CUser+0x5880`
  is set (EP5.4 ps_game 0x4760ed). The flag is set by the special-59 item (the move to the war server) with a 5-second timer, and the
  same test guards item use and trades.
- `CUser` offsets moved: bag items `+0x1c0`, bag count `+0x130c`, warehouse `+0x400` (confirmed by the same compares).
