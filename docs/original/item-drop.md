# Dropping items on the ground: `0x0206`

This page covers how the EP4.5 game server `ps_game.exe` lets a player drop an item from the bags: the request, the
three ways an item leaves the bag, where it lands, who may pick it up, and the answers. It was read in the Ghidra decompiler and the
disassembly, with the names from the server's PDB. The client's side of `0x0206` and `0x0401` is on
[World packets](../client/client-world-packets.md).

On other pages: the items a player loses on death (`CUser::ItemDropByUserDeath`; [Player kills 1](pvp.md#62-what-the-victim-loses))
and the duel stakes window (`0x24xx`, `ItemDropPvP`, `ExchangeItemRemove`'s second half; [Duels](duel.md)).

## In short

- **Equipped items cannot be dropped**, and neither can quest items. Dropping takes an item from the bags only.
- **Part of a stack can be dropped.** The rest stays in the slot, and the dropped part becomes a new item. A whole item keeps everything
  it had: its gems, craft stats and history.
- **The item lies at your feet for 60 seconds.** For the first 30 seconds only the player who dropped it can pick it up; after that,
  anyone can.
- **The ground is not saved.** An item on the ground lives only in the server's memory: if the server stops, it is gone.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (see
[Entering the world](enter-world.md)); `stItemInfo` as on [NPC shops](shop.md) (`byCount` `+0x4b`, `nRealType`
`+0xa0`); `CItem` from the PDB: `m_pInfo` `+0x30`, `m_qwUniqueID` `+0x38`, `m_byType` `+0x40`, `m_byTypeID` `+0x41`, `m_byCount`
`+0x42`, `m_wQuality` `+0x44`, `m_byGem[6]` `+0x46`, `m_szCraftName[21]` `+0x4c`, `m_dwMakeTime` `+0x64`, `m_byMakeType` `+0x68`,
`m_dwRemoveTime` `+0x6c`, `m_dwUserID` `+0x70`, `m_dwPartyID` `+0x74`, `m_byDropType` `+0x78`, `m_dwDropID` `+0x7c`.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/client-world-packets.md` (`0x0206`, `0x0401`). `CUser` offsets are the code's
(`docs/original/enter-world.md`); `stItemInfo` as in `docs/original/shop.md` (`byCount` `+0x4b`, `nRealType` `+0xa0`); `CItem` from the PDB:
`m_pInfo` `+0x30`, `m_qwUniqueID` `+0x38`, `m_byType` `+0x40`, `m_byTypeID` `+0x41`, `m_byCount` `+0x42`, `m_wQuality` `+0x44`, `m_byGem[6]`
`+0x46`, `m_szCraftName[21]` `+0x4c`, `m_dwMakeTime` `+0x64`, `m_byMakeType` `+0x68`, `m_dwRemoveTime` `+0x6c`, `m_dwUserID` `+0x70`,
`m_dwPartyID` `+0x74`, `m_byDropType` `+0x78`, `m_dwDropID` `+0x7c`.

Covered: the request, the three ways an item leaves the bag, where it lands, who may pick it up, and the answers. **Not covered:** the items a
player loses on death (`CUser::ItemDropByUserDeath`, `ItemDropPvP`) and the duel stakes window (`0x24xx`, `ExchangeItemRemove`'s second half);
both built (`docs/original/pvp.md` section 6.2, `docs/original/duel.md` sections 4 and 7).

## 1. The request

Confirmed, `CUser::PacketMaininterface` 0x4686f7.

Client → server, 5 bytes: `u8 bag, u8 slot, u8 count`. Ignored, with no answer, when:

- the player is dead (`+0x142c` = 1);
- `bag` is **0** (the equipment can never be dropped; `ItemDrop`'s own bag-0 branch, which unequips first, is unreachable from this packet);
- `count` is 0;
- the place is not a bag (`CUser::IsBag`: bag up to `m_nMaxBag`, slot below 24).

Then the trade is cancelled (`CUser::CancleActionExc`, `docs/original/trade.md` section 4) and the personal shop closed (`stMyShop::Ended`),
and `CUser::ItemDrop(bag, slot, count)` (0x45cd10) runs.

## 2. The drop

Confirmed, `CUser::ItemDrop`.

Nothing happens, with no answer, when the slot is empty or the item's real type is **27** (quest items and the like, `nRealType` `+0xa0`).
Otherwise one of three things:

1. **An unstackable item** (definition `byCount` 1, and the item's count 1): the whole item goes, **whatever `count` says**.
2. **`count` above what the slot holds**: nothing happens, no answer.
3. **`count` equal to what the slot holds**: the whole item goes.
4. **Less than the slot holds**: the slot keeps the rest (`m_byCount -= count`), and a **new item** (`CObjectMgr::CreateItem`) goes on the
   ground: same type and type id, `count`, a new unique id (`SUniqueIDGenByServer::GenID`), quality **0**, gems **0**, the craft name as the
   new object has it (not set here), made **now** (`GetSystemTime`), make type **`'U'`** (0x55). (If no item object can be made, nothing
   happens but the `0x0206` answer of the unchanged slot; that cannot happen here.)

An item that goes whole is the **same item** on the ground: its unique id, quality, gems, craft name, making time and make type stay with it.
Before it leaves, it is taken out of the trade window if it was offered (`CUser::ExchangeItemRemove`, `0x0A07`); the trade was already
cancelled in section 1, so this finds nothing.

Each way is saved with ps_dbAgent **`0x0702`** (`TP_DB_ITEM_DROP`, 9 bytes: `u32 dwUserID` from code `+0x578c`, `u8 bag, u8 slot, u8 count
dropped`; 1 for an unstackable item) and logged to the game log (`0x0402`, `CUser::SetGameLogMain`, with the item's unique id, type, type id,
gems, craft name and the place).

## 3. On the ground

Confirmed, `CZone::EnterItem` 0x417320 with kind 1.

The item lands at the player's **position** (x, y and z, unlike a monster's drop, which is randomised and put at y = 0), with:

- **owner** (`m_dwUserID`) = the player, **no party** (`m_dwPartyID` 0), drop type 2, drop id = the player;
- removal at **now + 60 s** (`m_dwRemoveTime`).

`0x0401` (22 bytes, `docs/original/combat.md` section 7) of **kind 1** goes to everyone in the 5 × 5 view, the player too; the client lets kind 1
fall from 0.1 high. (`EnterItem` refuses a position outside the map's cells, and the item would be lost; a player is never there.)

Who may pick it up is the rule of `CZone::GetPickItem` (`docs/original/combat.md` section 8): someone other than the owner gets error 0 ("This
doesn't belong to you") **for the first 30 s**; after that anyone may. The owner may take it back at once. Picking it up keeps the item as it is
(the `0x0205` answer carries its quality, gems and craft name, and the database gets the same unique id and making time).

## 4. The answer

Confirmed.

After `0x0401`, **`0x0206`** to the player (7 bytes, `TP_ITEM_DROP`): `u8 bag, u8 slot, u8 type, u8 type id, u8 count` of **what is left in
the slot**, the last three 0 when it is empty. The client rewrites that slot.

## 5. The new server

- `0x0206` → the checks of section 1 (dead, bag 0, count 0, not a bag), the trade ends, then the decision of section 2 is made on the
  `Inventory` (`Inventory::drop`), saved, and only then put on the ground.
- Saving: `ItemStore::drop_item` takes the row off the character in one statement: the whole item leaves `character_items` and comes back
  with everything a new row would not have (gems, craft name, make type, making time, unique id: `Kept`); part of a stack lowers the count, and
  the part dropped is a new item (make type `'U'`, no unique id yet). Both check that the slot still holds what the server thinks it holds.
- The ground keeps that with the loot (`Loot::Item { kept }`); a pick-up into an empty slot inserts it again as it was (same unique id and
  making time), and stacks of it only change counts. Monster drops keep nothing (`kept: None`) and are new items when picked up, as before.
- Kind 1, owner = the dropper, no party, 60 s, 30 s for the owner alone: the existing ground rules.
- As in the original, an item on the ground lives only in memory: a server stop loses it (it has already left the database).
- No deviations.
