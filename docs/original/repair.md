# Equipment wear and repair: quality, `0x0554`

This page covers equipment quality in the EP4.5 game server `ps_game.exe`: what quality does (the three wear
levels), how equipment wears, how wear is saved, and repairs at an NPC and with the Repair Hammer. It was read in the Ghidra decompiler
and the disassembly with the PDB names. The client side of `0x0554` is in the [packet catalogue](../client/client-packets.generated.md).

On other pages: the guild house's repair rate (maps of create type 6; [Guild houses](guild-house.md#9-what-a-houses-npcs-do-differently))
and the weapon repair item (special 25; no such item in the data; [Small features](small-items.md#10-item-special-25-skipped)).

## In short

- **Quality is durability.** Every piece of equipment has a quality number. Above half of its full quality it gives its whole bonus;
  between a tenth and a half, half the bonus; below a tenth, nothing. Accessories always give their full bonus.
- **What wears.** The weapon loses one point on each of your critical hits. Armour loses one point each time you are hit, one piece at
  a time in a fixed order: shield, helmet, upper armour, gloves, lower armour, boots. Nothing wears while hidden or transformed, or
  while an Eternal Endurance buff runs.
- **The client is not told.** No packet goes out when an item wears.
- **Repairing** at a blacksmith costs a share of the item's buy price for the lost quality. "Repair all" costs at least 1 even when
  nothing needs it. The Repair Hammer repairs everything worn for free.
- **No blacksmith check.** The server never checks that a blacksmith is near; only the client limits repairs to one.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's in this region; see
[Entering the world](enter-world.md)); `stItemInfo` and `CItem` as on [Dropping items](item-drop.md).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/client-packets.generated.md` (`0x0554`). `CUser` offsets are the code's (4 above the PDB's
in this region, `docs/original/enter-world.md`); `stItemInfo` and `CItem` as in `docs/original/item-drop.md`.

Covered: what quality does (the three wear levels), how equipment wears, how wear is saved, and repairs at an NPC and with the Repair Hammer.
**Not covered:** the guild house's repair rate (maps of create type 6; `docs/original/guild-house.md` section 9), wear in player
fights (`docs/original/pvp.md` section 9 and `docs/original/pvp-2.md`), the weapon repair item (special 25; no such item in the data).

## 1. Quality and wear levels

Confirmed, `CGameData::LoadConstItem` 0x4c6420, `CItem::GetQualityLevel` 0x4540a0.

Each item definition keeps three qualities and three values of each effect, made at load from the columns `quality` (Q) and `effect1` to
`effect4` (E): `wQuality = (Q, trunc(Q × 0.5f), trunc(Q × 0.1f))` and `wEffectN = (E, trunc(E × 0.5f), 0)` (the products in the x87 at
double precision, then `_ftol2`). An item's **wear level** is **0** while its quality is above `wQuality[1]`, **1** while above
`wQuality[2]`, else **2**; the player keeps the level of each equipped slot (`+0x19d + slot`).

What an equipped item adds by level (`CUser::ItemEquipmentAttackAddQ` 0x456620 and `RemQ` 0x4566d0): the **weapon** (slot 5) its attack
`wEffect1[level]` and attack spread `wEffect2[level]`; **armour** (slots 0 to 7 but 5, and 13) its defence `wEffect3[level]` and magic
defence `wEffect4[level]`; **accessories** (slots 8 to 12) all four at level 0 whatever their quality. So an item worn down to half its quality
gives half, and below a tenth nothing. (This part is already in `docs/original/combat.md` section 4 and built in `combat::gear`.)

## 2. Wearing

Confirmed.

Neither wear sends anything to the client. Both do nothing while the player's shape (`m_byShape`, code `+0x1354`) is not 0 (hidden or
transformed) or while it has **Eternal Endurance** (`m_bSkillNDQuality`, code `+0x58b8`: a buff with ability **36**,
`docs/original/item-use.md` section 7).

- **`CUser::QualityDec(item, 1)`** (0x45b350): nothing for an empty slot or an item at quality 0 (returns false). Otherwise quality − 1 (not
  below 0); when the wear level changes, the slot's effects come off at the old level and on at the new one and the numbers are worked out
  again (`CUser::SetAttack`). Returns true.
- **The weapon** (`CUser::QualityByAttack` 0x45b260): `QualityDec` on slot 5. Called by `CUser::AttackMob` after the damage when the blow
  was a **critical** (`CUser::AttackDamage` returns 1 only for a critical): the normal attack (0x44ceff) and each skill hit on a monster
  (`AttackMob` with a skill, 0x44ff6c and 0x450354, which area and multiple hits use too); also in player fights (`AttackUser`). A plain hit
  or a miss wears nothing. (`docs/original/combat.md` section 3 step 4 said "a hit"; it is a critical hit.)
- **The armour** (`CUser::QualityByDefense` 0x45b290): at the start of **`CUser::Damage`** (both forms, 0x44c120 and 0x44c1e0), after the
  checks that the player is alive with HP above 0, so on every blow that reaches the player: monsters' normal attacks and skills, their damage
  over time (`CUser::ChkProtect`), traps, guard NPCs, and player attacks. It wears **one** piece, in the fixed order **shield (6), helmet
  (0), upper armour (1), gloves (3), lower armour (2), boots (4)**, starting at the player's index (`m_nQualityDec`, code `+0x144c`, 0 at
  first): from that index to the end of the order, the first piece that `QualityDec` accepts is worn and the index moves past it (back to
  0 after boots); when none to the end is accepted the index goes back to 0 and nothing wears this time (the pieces before the index are
  not tried until the next blow).

## 3. Saving wear

Confirmed.

Wear stays in memory. The player keeps the last saved quality of each equipment slot (code `+0x1aa`, 13 × `u16`):

- when an equipped item leaves its slot (moves, drops, gems, …) with a quality that differs, ps_dbAgent **`0x070C`**
  (`TP_DB_ITEM_SET_QUALITY`: `u32 char id, u8 slot, u16 quality`) saves it first;
- the qualities that differ are sent together as **`0x070D`** (`TP_DB_ITEM_SET_QUALITY_LIST`, up to 13 × `u8 slot, u16 quality`) by
  `CUser::SendDBAgentCharGetInfo`, which runs on leaving the world (`CWorld::LeaveUser`, `LogoutUser`) and on some other occasions.

A server stop loses the wear since the last save.

## 4. The request: `0x0554`

Confirmed, `CUser::PacketPC` 0x46ac4d.

Client → server, 4 bytes (`TPC_ITEM_REPAIR`): `u8 bag, u8 slot`; **255, 255** = everything equipped. Ignored while dead (`+0x142c` = 1).
Then the trade is cancelled (`CancleActionExc`), the personal shop closed (`stMyShop::Ended`), and `CUser::ItemRepair(bag, slot)` (0x464340)
runs. **Nothing checks for an NPC**: the client sends `0x0554` only while an NPC is selected (`docs/client/CLIENT-FACTS.md`; no NPC id
on the wire), but the server repairs anywhere.

In a guild house map (create type 6) the guild's repair rate (`CGuild::GetGuildHouseRepairRate`) multiplies the price, and without a guild
house of rank 14 or below the request is refused with `0x0D28` (`u8 1, u8 14`); elsewhere the rate is 1. (Built: `docs/original/guild-house.md` section 9.)

## 5. The price and the repair

Confirmed, `CUser::ItemRepair`.

An item's **full quality** is its definition's `wQuality[0]` plus, for each of its six gems that is a known lapis (item type 30, type id =
the gem), that lapis' `wQuality[0]`.

The **price of one item**, all rounding toward zero (the x87 control word is set to truncate):
`trunc(f64(f32(full − quality)) / 400 × f64(f32(f32(nBuy) / 30)))`, with `nBuy` the definition's buy price (unsigned).

- **One item** (`bag` 0 to 5; slot below 13 in bag 0, below 24 elsewhere; any bag up to 5, the bag count is not checked): nothing when the
  slot is empty or its quality is already at least full. Price as above, then `trunc(price × rate)`, **at least 1**; nothing (no answer)
  when the money is short. Otherwise money − price, the quality set to full, and for an equipped item a new wear level is put into effect as in
  section 2. Saved with ps_dbAgent **`0x070A`** (`TP_DB_ITEM_REPAIR`: `u32 char id, u8 bag, u8 slot, u16 quality, u32 money`). Answer
  **`0x0554`** (`TP_ITEM_REPAIR`, 8 bytes: `u8 bag, u8 slot, u32 money`).
- **Everything** (`bag` and `slot` 255; any other slot with bag 255 is ignored): for each item equipped in slots **0 to 12** (not the mount),
  `total = trunc(f64(f32(total)) + price of the item)` in slot order (an item at full quality adds 0). Then `trunc(total × rate)`, **at least
  1**: repairing when nothing is worn **costs 1**. Nothing when the money is short. Otherwise money − price, each item whose quality is not
  its full quality gets it (and a new wear level as above); ps_dbAgent **`0x070B`** (`TP_DB_ITEM_REPAIR_ALL`: money and the changed slots)
  and **`0x0554`** with bag and slot 255.

A flaw: after one item's repair, the saved-quality copy of **slot 6** (the shield) is set to the new quality, whatever slot was repaired (the
index is the gem loop's 6). The shield's next change of quality may then not be saved, or be saved needlessly.

**The Repair Hammer** (item special 40, `docs/original/item-use.md` section 10) does the "everything" repair **for free**: every item in slots 0
to 12 back to its full quality, `0x070B`, and `0x0554` (255, 255, money).

## 6. The new server

- **Wear**: a critical blow on a monster by a normal attack or a skill (`AttackOutcome::Struck::critical`, `SkillOutcome::criticals`) wears
  the weapon; every blow that reaches the player through `hurt_player` (monsters' attacks, skills and damage over time) sends
  `ZoneEvent::Worn` and the connection wears the next armour piece in the order of section 2 (`repair::Wear`). Neither while the shape is not
  0 or with an ability-36 buff. A changed wear level works the numbers out again (`renumber`).
- **Saving**: the connection keeps the equipment slots with unsaved wear; their qualities are written (`SlotChange::SetQuality`) in the same
  transaction as any move that touches the equipment, with a repair, and on leaving the world.
- **`0x0554`**: as sections 4 and 5, money and qualities in one transaction (`ItemStore::trade`), rate 1 (a guild house's blacksmith rate on
  its map, `docs/original/guild-house.md` section 9). The Repair Hammer repairs too.
- **Deviation 1 (approved):** a repair needs a **blacksmith (NPC type 3) in the player's view**; otherwise the request is ignored like the
  original's other refusals (no answer). The original checks nothing, so a modified client can repair anywhere. (Blacksmiths: 3 on map 1, 39
  in all on 18 maps.) The Repair Hammer needs none.
- Not reproduced: the shield's saved-quality flaw of section 5 (each slot's quality is saved as it is).
- The client gets no packet when an item wears, and lowers its own copies the same way (confirmed in `game.exe`): the weapon −1 on
  its own critical hit (0x4c1f80, the decrement at 0x4c1fcf), one armour piece per blow taken in a rotating order (0x4c1fe0, from
  0x4c2078), none while ability 36 runs. It does not check the shape, where this server and the original skip wear, so a hidden or
  transformed player's client shows lower qualities (and a higher repair price) than the server holds until the next `0x0106` or
  repair answer.

## 7. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

**The same repair** (confirmed): `ItemRepair` (EP3 ps_game 0x44ba50, matched by dispatch, `0x0554`) has the same structure (12 of 12
calls), and the shield-slot save of [`repair-shield-slot`](community-fixes.md#repair-shield-slot) is there (EP3 ps_game 0x44c0c8, as
EP4.5 0x4649d3).

### EP5.4

**The same repair** (confirmed): `ItemRepair` (EP5.4 ps_game 0x471b00, matched as the only call of the `0x0554` case after the two
cancel calls) differs from EP4.5 only in moved offsets (every immediate and compared field compared). The shield-slot flaw is still
there: the saved quality is written at the loop's final index 6 (EP5.4 ps_game 0x47208e).
