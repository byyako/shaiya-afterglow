# New items: make types, craft stats and boxes

This page covers what the EP4.5 game server `ps_game.exe` gives a newly made item, wherever an item is made: its make
type and its craft name; the random craft stats of monster drops; and the boxes and pockets (`bySpecial` 42) with `Data/ItemCreate.ini`.
It was read in the Ghidra decompiler and the disassembly with the PDB names. The crafted numbers and the roll itself are also on
[Lapis and enchanting](lapis.md).

On other pages: the administrator's item command (`CUser::PacketAdminCmdB`, the other caller of `CUser::ItemCreate`;
[GM commands 1](gm-commands.md#72-items-f702-f703)), item mall and point shop items (`PacketBilling`, `PacketPoint`;
[Item mall](item-mall.md)), personal shops (`PacketMyShop`; [Personal shops](personal-shop.md)), the market
(`PacketMarket`; [Auction house](auction.md)), and server-wide drop limits (`wDrop`;
[Quest and sequence drops](quest-drops.md#5-server-wide-drop-limits)).

## In short

- **Every item remembers how it was made.** A one-letter *make type* says where it came from: `D` a monster's drop, `V` a sequence drop,
  `S` a box or a GM's item, `C` a remake, `B` bought from an NPC or the billing storage, `X` the item mall, `T` traded or from a personal
  shop, `U` a stack split on the ground, `G` lapis taken out, `Q` a quest reward (the full list: section 1). This matters later: bought items cannot be given new stats or remade, and quest
  rewards cannot be changed at all.
- **Dropped items come with random stats.** Craftable items from monsters and boxes get random bonuses (STR, DEX, HP and so on). Each
  bonus is the smallest of three random draws, so low values are much more common than high ones. Bought items get none.
- **The stats live in the "craft name".** A 20-character string of two-digit numbers: nine stats and the enchant level.
- **Boxes and pockets** roll a grade from a weighted list in `ItemCreate.ini`, then a random item of that grade. A grade with no items
  means the box is used up for nothing. If the bags are full, the box is kept.

Status words: **confirmed** = read in the code (address given). `CItem` and `stItemInfo` offsets as on [Dropping items](item-drop.md)
and [NPC shops](shop.md) (`m_pInfo` `+0x30`, `m_szCraftName[21]` `+0x4c`, `m_byMakeType` `+0x68`; `stItemInfo`: `shVG`
(`reqvg`) `+0x34`, `wReqWis` (`reqwis`) `+0x3e`, `byServer` (`server`) `+0x4a`, `byCount` `+0x4b`, `wQuality` `+0x4c`, `nRealType`
`+0xa0`).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). `CItem` and `stItemInfo` offsets as in `docs/original/item-drop.md` and `docs/original/shop.md` (`m_pInfo`
`+0x30`, `m_szCraftName[21]` `+0x4c`, `m_byMakeType` `+0x68`; `stItemInfo`: `shVG` (`reqvg`) `+0x34`, `wReqWis` (`reqwis`) `+0x3e`, `byServer`
(`server`) `+0x4a`, `byCount` `+0x4b`, `wQuality` `+0x4c`, `nRealType` `+0xa0`). The crafted numbers and the roll itself are in
`docs/original/lapis.md` sections 2 and 6.

Covered: what a new item's make type and craft name are, wherever the original makes one; the random craft stats of monster drops; the
boxes and pockets (`bySpecial` 42) with `Data/ItemCreate.ini`. **Not covered:** the administrator's item command (`CUser::PacketAdminCmdB`,
the other caller of `CUser::ItemCreate`; `docs/original/gm-commands.md` section 7: make type `'S'` with craft stats as for
boxes), item mall and point shop items (`PacketBilling`, `PacketPoint`; built since, with make type `'X'` and `'B'`:
`docs/original/item-mall.md` sections 3 and 6), personal shops (`PacketMyShop`) and the market
(`PacketMarket`), both built (`docs/original/personal-shop.md` section 7, `docs/original/auction.md` section 15); server-wide drop
limits (`wDrop`, as before).

## 1. Where items are made

Confirmed, the callers of `CItem::Init` 0x4b9f80 and of the two functions below.

`CItem::Init(type, type id, count, make time, make type)` sets the type, type id and count, the quality to the definition's `wQuality`, no
gems, and **clears only the craft name's first byte** (an empty name). Then, depending on where the item comes from:

**Every make type**, from every write of `m_byMakeType` (`+0x68`) in `ps_game` and every call of `CItem::Init` (confirmed: a scan of
the whole image for both; the other writes at `+0x68` are other structures). The letter is one byte; the database keeps it in the
`MakeType` column of `CharItems`, `UserStoredItems`, `GuildStoredItems` and the market tables, which the procedures' own comment calls
"획득방법", *how it was obtained*.

| make type | made by (address) | craft name |
|---|---|---|
| **`D`** (0x44) | a monster's drop (`CMob::ItemDrop` 0x4a665e), and the gold it drops (0x4a6489, an item of type 26 with no definition) | **`CreateCraftExpansion`**: random stats (section 3) |
| **`V`** (0x56) | a **sequence drop** (`CMob::ItemDrop` 0x4a66e7 writes it over the `D`; `docs/original/quest-drops.md` section 4) | as a monster drop |
| **`S`** (0x53) | a box or pocket (`CUser::ItemCreate` 0x45ef18, from `CUser::ItemUse`, section 5), and the GM's item command (`CUser::PacketAdminCmdB`, the other caller; `docs/original/gm-commands.md` section 7) | **`CreateCraftExpansion`** |
| **`C`** (0x43) | one item from three (`CUser::ItemRemake` 0x460a30) | **`CreateCraftExpansion`** (built, `docs/original/lapis.md` section 7) |
| **`B`** (0x42) | buying from a merchant (`CUser::PacketShop` 0x46b8c4, 0x46bb38), and taking an item from the billing storage (`CUser::PacketBilling` 0x457c90) | **`InitCraftExpansion`**: zeros (section 2) |
| **`X`** (0x58) | the item mall and point shop (`CUser::PacketPoint` 0x47a237, 0x479b5d; `docs/original/item-mall.md` sections 3 and 6) | `InitCraftExpansion` |
| **`T`** (0x54) | a trade's split stack (`CUser::ExchangeItemAdd` 0x46fce0; `docs/original/trade.md`), a duel stake taken from a stack (`CUser::ExchangePvP` 0x47cec4; `docs/original/duel.md` section 4), and buying from a personal shop (`CUser::PacketMyShop` 0x479356; `docs/original/personal-shop.md` section 7) | the giver's for a trade; none for a duel stake or a personal-shop purchase (the new server keeps the giver's: trade deviation 3) |
| **`U`** (0x55) | part of a stack put on the ground: dropped by the player (`CUser::ItemDrop` 0x45d150, `ItemDropByUser` 0x45a575; `docs/original/item-drop.md`), or a duel's stakes laid down (`CUser::ItemDropPvP` 0x47c9cd) | `InitCraftExpansion` (built; the new server gives them no name, section 6) |
| **`G`** (0x47) | lapis taken out of an item: one (`CUser::ItemGemRemovePos` 0x463989) or all (`ItemGemRemoveAll` 0x4626c9) | `InitCraftExpansion` |
| **`Q`** (0x51) | a quest reward (`CUser::QuestAddItem` 0x47e837) | empty (built) |
| (kept) | an item bought on the market keeps the registered item's own letter (`CUser::PacketMarket` 0x477d41); a stack split between bags keeps the stack's (`CUser::ItemBagToBag` 0x45becd); items loaded from the database keep theirs (`CUser::PacketUserDBSetInfo`, the guild bank and the market, `CClientToDBAgent::PacketGuild`, `PacketMarket`) | as stored |
| 0 | a new item object before it is filled (`CObjectMgr::CreateItem` 0x40a55f) | |

Items the GM tool (`PSGMTool.exe`) writes straight into the database get whatever letter the tool sends (`usp_Inven_Insert`,
`usp_Store_Insert`: `@MakeType varchar(5)`). The operators' statistics procedure `up_xu_shaiya_item_maketype_sel` (`PS_Statics`)
lists every **`V`** item in the bags and warehouses of three game databases, so the sequence drops were watched by their letter.

**Where the letter is checked** (confirmed: every compare of `+0x68` with a letter): only `CUser::ItemCompose` 0x460613 (`'Q'`),
`ItemRemake` 0x46098f, 0x4609ad (`'Q'`, `'B'`) and `ItemConsider` 0x4610d4 (`'Q'`, `'B'`). The game log's item block carries no make
type ([The game log](game-log.md) section 2).

The gold a monster drops is an item without a definition (`m_pInfo` 0): `CMob::ItemDrop` 0x4a645b calls `CreateCraftExpansion` on it too, which
returns at once.

The make type matters later: `'B'` items can't be given new stats by `0x0808` and can't be remade (`0x0807` result 3), and `'Q'` items can't be
changed by `0x0806` to `0x0808` (`docs/original/lapis.md` sections 6 and 7).

## 2. `CItem::InitCraftExpansion` (0x4ba280, verified)

For a **craftable** item (`byServer` ≠ 0 and `wReqWis` 1 to 99) the craft name becomes `"00000000000000000000"` (20 zeros: nine numbers and the
enchant level all 0). Any other item keeps the empty name.

## 3. `CItem::CreateCraftExpansion` (0x4ba2e0, verified)

Nothing for an item that is not craftable (as above; `wReqWis` is compared as 1 to 99 here too). Otherwise the same choice of numbers as
`ReGenerationCraftExpansion` (`docs/original/lapis.md` section 6, by `byServer`; the jump tables at 0x4ba62c and 0x4ba650 read):

- weapons (real types 1 to 15) and `CItem::IsAccessory` (real types **22, 23, 25**): the six stats; `byServer` 1 one at random, 2 to 4 that
  many different ones, 5 all but one, 6 or more all six;
- other items: the nine numbers; 1 one, 2 to 7 that many, 8 all but one, 9 all nine, more than 9 none.

Each chosen number is the **smallest of three** draws `rand() % (wReqWis + 1)`; the others 0. They are written as two digits each in the order
of `g_CraftNameExpansionIndex` (0x542040: characters 0, 2, …, 16, so STR, DEX, REC, INT, WIS, LUC, HP, MP, SP), and **characters 18 and 19
become `"00"`** with the name ending after them: a new item has no enchant level. The numbers are also kept in `m_wCraftExpansionAddValue`
(`+0x84`), what equipping adds (`docs/original/stats.md`).

So a dropped craftable item usually comes with some stats, and an item whose definition allows none (`byServer` above 9 for armour) still gets
a 20-zero name.

## 4. Monster drops

Confirmed, `CMob::ItemDrop` 0x4a65a0.

Each item a monster drops (`docs/original/combat.md` section 7) is made with `CItem::Init(type, type id, 1, now, 'D')` and then
`CreateCraftExpansion`, before it goes on the ground. The item keeps that name and make type on the ground and in the bag of whoever picks it up
(`0x0205` carries the craft name, `docs/original/combat.md` section 8). The draws come after the drop entry's own roll and the pick of the
item, before the next entry.

## 5. Boxes and pockets: `bySpecial` 42

Confirmed.

### 5.1 `Data/ItemCreate.ini` (`CItemDrop::InitItemCreate` 0x4be840)

Read once at start. Line by line (`fgets`, up to 8191 characters):

- lines of 0 or 1 character are skipped; leading spaces and tabs are skipped; lines starting with `//` or `--` are comments;
- **`[ItemCreate_n]`** (compared without case) starts set **n**, read with `strtok` on the delimiters `` ,=()[]{}`` plus tab, CR and LF
  and `atol`. n must be **1 to 255**; any other number makes the lines up to the next section count for nothing. Before a new section starts
  (and at the end of the file), the set being read is stored when it has a number and at least one line. A set number given twice replaces the
  earlier one.
- Any other line inside a valid set: the first two tokens, `atol`, are a **grade** and a **weight**. Kept only when the grade is **0 to 1024**
  and the weight **above 0**. A grade with **no items** (`CItemDrop`'s grade lists, as for monster drops) becomes grade **0**. A grade that is
  already in the set adds its weight to the earlier line (its place stays the earlier one's).

The set keeps its lines in file order with a running total of weights at each line, and the **total** of the set.

The weights are only weights: a line's chance is its weight over the set's total. (`docs/original/community-guide-notes.md` says that 0 to 100 is a
percentage and only larger numbers are weights; the code doesn't do that.)

The shipped file has sets 1 to 68, no set given twice. Several sets name grades that have no items (for example set 19, the Fortune Bag:
grades 1011 to 1015), and those lines become "nothing" (section 5.3).

### 5.2 Using the item (`CUser::ItemUse` 0x4664f5)

After the usual checks of `docs/original/item-use.md` (sections 1 to 3, the reuse group), for `bySpecial` 42 with `n` = `shVG` (`reqvg`, signed):

1. **`CItemDrop::EnableItemCreateByGrade(n)`** (0x4bf120): n 1 to 255 and set n has lines. Otherwise **the error answer**, the item kept.
2. **`GetItemCreateByGrade(n)`** (0x4bf0a0): with the set's total above 0, `r = rand()` once, and the first line in order with **`r % total <
   running total`** gives the grade. No line (a total of 0 cannot happen with the rules above) gives 0.
3. **Grade 0**: the item is **used up** (`ItemUseNSend`) and nothing else happens.
4. **`CItemDrop::GetItemPlz(grade)`** (0x4bddb0): grade above 1024 gives none. `CItemDrop::GetItem(grade)` (0x4bdd10) picks a random item of the
   grade (`rand() % count`), and gives it unless it has a server-wide limit (`wDrop` above 0) that is used up or waiting, in which case it gives
   none; then `GetItemPlz` picks **again** (`rand() % count`) and gives that item **without** looking at limits. No item (an empty grade) is
   the **error answer**, the box kept.
5. **`CUser::ItemCreate(item, 1)`** (section 5.3). It fails when no slot is empty: **the error answer**, the box kept.
6. Otherwise the box is **used up** (`ItemUseNSend`: `0x050A` to the view, `docs/original/item-use.md` section 3).

So `0x0205` for the new item comes **before** the box's `0x050A`.

### 5.3 `CUser::ItemCreate(item, count)` (0x45ede0)

`count` must be 1 to 1000 (it is 1 here). For each **empty** slot of bags 1 to `m_nMaxBag`, slots 0 to 23 in order, **without** looking at stacks
of the same item: a new item, count `min(count, byCount)`, the definition's quality, no gems, make time now, **make type `'S'`**, a new unique id,
`CreateCraftExpansion`; ps_dbAgent gets `0x0701` (a new item); a game log; and **`0x0205`** to the player, as for a pick-up of a whole item: `u8
bag, u8 slot, u8 type, u8 type id, u8 count, u16 quality, u8 gems[6]`, the craft name and its terminating zero (16 bytes with an empty name).
It returns success once `count` is all placed, and failure when no empty slot is left (any part already placed stays).

## 6. The new server

- **Craft stats on new items**: `lapis::new_craft_name(def, rand)` is section 3 (`roll_craft` with three draws and enchant level 0, a 20-zero
  name for every craftable item); `lapis::zero_craft_name(def)` is section 2. Both give an empty name for an item that is not craftable.
- **Monster drops** (`drops::roll`): each item drop now carries `kept` with make type `'D'` and that name, rolled right after the item is
  picked. The ground and pick-up already keep it (`docs/original/item-drop.md` section 5); the stored row gets the name and `'D'`, a new unique
  id and a making time when it is picked up (the original makes them at the drop; nothing tells the difference).
- **Buying** (`Inventory::buy`): a new item gets make type `'B'` and the zero name when craftable. Adding to a stack changes only the count, as
  before. Rows of items bought before this change keep their empty make type.
- **Boxes**: `world.item_create` holds `ItemCreate.ini` as it is (set, line order, grade, weight), imported by `tools/world/import.py`
  from the same `Data` folder; the game server applies section 5.1's rules when it loads it (`DropTables::load_boxes`, with the drop grade lists) and rolls
  with `DropTables::open_box`.
  `item_use::effect` gives `Effect::Box(reqvg)`, and the connection does 5.2 and 5.3: the new item (`NewItem` with `'S'` and the new name, in
  the first empty slot) and one box used are saved in **one transaction** (`ItemStore::trade`), then `0x0205`, then `0x050A` to the view.
- Server-wide limits are still not kept (as for monster drops): `GetItem` treats a limited item as unavailable, so the second pick of 5.2
  step 4 is taken, as the original does while a limit is waiting.
- Not changed: lapis taken out (`'G'`) and part of a stack on the ground (`'U'`) keep an empty name. In the original these are never
  craftable items (lapis) or are stackable (and stackable items are not craftable in the data: no `byCount` above 1 with `byServer` set and
  `wReqWis` 1 to 99 in `defs.items`).
- No deviations.
- Quest items and sequence items (make type `'D'` for the first, `'V'` for the second) dropped by monsters: `docs/original/quest-drops.md` (built).

## 7. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **No crafted values are read** (confirmed): EP3 has no craft-name parser like `CItem::CopyCraftExpassion` and its `ItemEquipmentAdd`
  (EP3 ps_game 0x43f590, matched by call graph) adds none. Whether EP3 rolls craft stats when an item is made was not established.
  The EP3 database still has `charitems.Craftname` (varchar(50), EP4.5 20) and `Maketype` (varchar(50), EP4.5 1) (confirmed, schema).
- **Boxes** (confirmed): `InitItemCreate` (EP3 ps_game 0x496730, matched by call graph) reads `ItemCreate.ini`; a rolled helmet or
  cloak (real type 16 or 24) gives the error answer and the box is kept (EP3 ps_game 0x44db7a to 0x44db8c). The EP3 file has 40 sets:
  37 equal EP4.5's, sets 1 and 2 have one more line (grade 142, weight 400) and set 38 other weights; sets 41 to 68 are EP4.5's
  additions (confirmed, file).

### EP5.4

- **Grades up to 3072** (confirmed): `CItemDrop::InitItemCreate` (EP5.4 ps_game 0x4d69f0, matched by its string) accepts grades up to
  0xc00 (EP5.4 ps_game 0x4d6d82) and `GetItemPlz` (EP5.4 ps_game 0x4d5f30) too; EP4.5 stops at 1024. Sets 1 to 68 of the file equal
  EP4.5's; sets 69 to 88 are new.
- **Multi-item boxes** (confirmed): `CItemDrop::InitRandomItemsCreate` (EP5.4 ps_game 0x4d6df0, named by its log string) reads
  `[RandomItemsCreate_n]` sections, n 1000 to 1255, each line a grade and a chance in per cent. A box of special 42 with `shVG` 1000 or
  more needs as many free bag places as the set has lines, then rolls every line separately and makes one item per success; the box is
  used up whatever came out (EP5.4 ps_game 0x473aaa to 0x473b6e). A fresh generator (a Mersenne Twister, EP5.4 ps_game
  0x4bd3e0) is made at every use (0x473b27) and seeded with `time(0) xor` the player's `m_dwUID` (`CUser+0x57f8`, a number counted up at
  each login, 0x4115db; 0x473b2e to 0x473b44); each line is rolled out of 1,000,000 against its chance (0x473b53 to 0x473b6e). So two
  boxes of one set opened by the same player in the same second roll the same (confirmed).
- **Cloaks are refused in both box kinds** (confirmed, EP5.4 ps_game 0x473bbd): the error answer, box kept.
- `CUser::ItemCreate` (EP5.4 ps_game 0x46bd10; name carried over, match method not recorded) still makes up to 1000 and sends the 57-byte `0x0701`; new
  `CItem` fields `+0x84` to `+0x9c` are cleared at creation (`+0x9c` holds the item-mall purchase number, [`item-mall.md`](item-mall.md)).
- Data (confirmed, file and rows): two boxes name sets the file does not have (1028, 1031), so they always fail (confirmed: a box of 1000 or more needs its set to
  have lines, else the error answer and the box is kept, EP5.4 ps_game 0x473ab6, 0x4d76bf).
