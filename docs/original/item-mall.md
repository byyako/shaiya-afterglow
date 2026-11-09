# Item mall, points, gifts and premium

This page covers the item mall in the EP4.5 game server `ps_game.exe` and its database agent `ps_dbAgent.exe`: the
product list, the account's points, buying (`0x2602`), gifts (`0x2603`, `0x2604`), the point refresh (`0x2605`), the item-mall storage
(`0x2606`, `0x2607`), the billing storage (`0xB101`, `0xB102`), premium accounts (`0xB103`, `CUser::BillingApply`) and what premium
changes. It was read in the Ghidra decompiler and the disassembly with the PDB names, together with the stored procedures and tables of
the original databases (`PS_GameData`, `PS_UserData`, `PS_GameDefs`, `PS_Billing`). Nothing was executed. Sizes include the opcode.

The client side is on the client's [Item mall](../client/item-mall.md) page. Not covered: the game log records these write
(`TP_GAMELOG_ITEM_GET_POINTITEM` `0x0410`, `…_GET_STOREDPOINTITEM` `0x0411`, `…_BILLINGGET` `0x040D`), `0xB104` / `0xB105` (never sent
by this server) and `0xB106` (never answered).

## In short

- **Points belong to the account**, shared by every game server. Nothing in the game adds points: an outside billing page was meant to.
- **Buying** puts the product's items into empty bag places only (nothing is stacked onto items already there). There must be enough
  empty places for the whole product.
- **Gifts** go to a character by name on the same server, of either faction, but not to one's own account. The items go into the
  recipient account's **mall storage**, which the recipient opens from the mall's store button.
- **Gifts never worked in the shipped setup**: the points for a gift are taken through an outside billing server that is not part of
  the shipped databases, so every gift fails with "Can't send the present."
- **A second storage, the billing storage**, is filled by an outside web shop and opened at the Bank Tellers.
- **Premium accounts** come from the billing service. Type 1 opens the warehouse's second half; types 2 and 3 also stop equipment
  wearing, give ×1.5 experience (×2 with an EXP stone), and make lapis linking 1.5 times as likely. Premium lasts until the character
  leaves the world.
- **Points are not taken safely**: the game takes them from its own copy and the database a moment later, which goes wrong in some setups.
  The details are held back.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (the PDB's plus 4 from the map pointer on).

## How it was read

From `ps_game.exe` and `ps_dbAgent.exe` (EP4.5, with their PDB names), read in the Ghidra decompile (REA) and the disassembly
(`objdump`, names from `docs/original/symbols/ps_game.tsv` and `ps_dbAgent.tsv`); the stored procedures and tables of the restored original
databases (`PS_GameData`, `PS_UserData`, `PS_GameDefs`, `PS_Billing` in the `shaiya-afterglow-mssql` container); packet layouts from the PDB
(`docs/original/symbols/ps_game.packets.txt`). Client side: `docs/client/item-mall.md`, `docs/client/CLIENT-FACTS.md` (`0xB103`) and
`docs/client/skill-monster-sdata-vs-defs.md` ("The item mall"; `Cash.SData` against `defs.product_list`). Nothing was executed. `CUser` offsets
are the code's (`docs/original/small-items.md`). Sizes include the opcode.

Covered: the product list, the account's points, buying (`0x2602`), gifts (`0x2603`, `0x2604`), the point refresh (`0x2605`), the
item-mall storage (`0x2606`, `0x2607`), the billing storage (`0xB101`, `0xB102`), premium accounts (`0xB103`, `CUser::BillingApply`) and
what premium changes. **Not covered:** the game log records these write (`TP_GAMELOG_ITEM_GET_POINTITEM` `0x0410`,
`…_GET_STOREDPOINTITEM` `0x0411`, `…_BILLINGGET` `0x040D`: written, see [The game log](game-log.md)), `0xB104`/`0xB105` (never sent by this server) and
`0xB106` (never answered).

## 1. The product list

Confirmed.

- `CGameData::LoadProductList` 0x4c7870 runs `EXEC usp_Read_ProductList` (`PS_GameDefs`) at start: per row the product code (21), 24 ×
  (`u32 item id, u8 count`) and `u32 BuyCost`; a row whose first item id is 0 is skipped. They go into a map by code (`stProductInfo`:
  code `+0`, item ids `+0x18`, counts `+0x78`, price `+0x90`); `CGameData::GetProductInfo` 0x479770 looks a code up (exact,
  case-sensitive).
- An item id is `type × 1000 + type id`; `CGameData::GetItemInfo` finds the definition. A product row's entry is used when its item id
  and count are both non-zero.
- `defs.product_list` holds the 84 rows (copied from `PS_GameDefs`, `docs/original/database-notes.md`): 107 entries in all, at most 15 per
  product, every item defined, no count above its item's stack size; prices 25 to 3,999. The client's `Cash.SData` has the same 84 and
  one more, "Phukey Long Cape" (`B2_HpoLM4801`), which no server row sells (section 9, oddity 7).
- The client shows the mall from `Cash.SData` alone; the server only checks a code the client sends (`docs/client/item-mall.md`).

## 2. Points

Confirmed.

- **Points belong to the account**, across every game server: `PS_UserData.dbo.Users_Master.Point` (the shared user database).
  `usp_Read_User_CashPoint_NCash` reads them (a negative value is set to 0 and read as 0).
- ps_dbAgent reads them with the character's data and sends **`0x0E01`** (`TP_DB_POINT_LOG_LIST`: `u32 user, u32 points, u8 n`, n ×
  `DB_POINTLOG_UNIT`); `CUser::PacketUserDBPoint` 0x46e880 keeps the points at `+0x5a20` and the rows at `+0x58e0` (10 × 32 bytes).
  The rows are **this character's** last 10 purchases and gifts sent: `usp_Read_Chars_PointLog_R` = `SELECT TOP 10 ProductCode, UseDate,
  UsePoint FROM PointLog WHERE CharID = @CharID ORDER BY UseDate DESC`.
- **`0x2601`** at world entry (`CUser::SendPointLog` 0x483ed0, in `PacketUserDBChar` after the quick slots and before `0x0114`):
  `u32 points, u8 n`, n × (`char code[21], u32 buy time, u32 points used`) = **7 + 29n**. The time is the server's packed local time
  (`GetSystemTime` 0x4cb8e0, the format of `docs/client/auction-house.md`).
- **`0x2605`** (2 bytes, the client's refresh) in `CUser::PacketPoint` 0x4797c0: nothing while a gift is waiting for the database
  (`+0x5a24`, section 4); otherwise, **once in 40 s** (`+0x5a28` = now + 40,000 ms) it asks ps_dbAgent (`0x0E06` → `ReloadPoint` →
  `usp_Read_User_CashPoint_NCash` → `0x0E06 u32 user, u32 points` back → the points replaced, `0x2605` sent, unless a gift is waiting);
  within the 40 s it answers at once with the points it holds. **`0x2605`** = `u32 points` (6 bytes).
- **Charging**: nothing in the game adds points. The shipped `usp_Optisp_Charge_ShaiyaPoint` (`Point += @Point`, a row in
  `UserPointChargeLog`) is for an outside billing page; a running game sees the new total at the next refresh. ps_dbAgent also handles
  `0x0E08` (`u32 user, u32 points`: `usp_Save_User_GetPoint`, then `0x0E08` to the game, which adds the points and sends `0x2605`), but
  **no code in `ps_game.exe` sends `0x0E08`**: another service would have.

## 3. Buying: `0x2602`

Confirmed, `CUser::PacketPoint` 0x4797c0.

Request: `TPC_POINT_BUY` (23 bytes): `char code[21]` (the server writes a NUL at its last byte). Answer `TP_POINT_BUY` with `u8 result`
first; failures are 3 bytes. In order:

1. `g_bDisableShop` (a setting): 2 → nothing sent; 1 → result **1**.
2. A gift waiting for the database (`+0x5a24`) → result **4** (the client shows nothing).
3. Unknown code → result **1** (client: 239 "No item located on that spot").
4. Points (`+0x5a20`, as held) below the price → result **2** (823 "Insufficient points.").
5. **The places needed**: for each entry (item id and count non-zero, a definition with a stack size `byCount` `+0x4b` above 0), one
   place if the count is at most the stack size, else `count / size` plus one more for a remainder; entries without a definition or
   with stack size 0 are **skipped**.
6. **The empty places**: bags 1 to the character's bag count, slots 0 to 23 in order, empty slots only (nothing is stacked onto an item
   already there). Fewer than needed, or none needed at all (every entry skipped) → result **3** (238 "Inventory is full").
7. **The items**, entry by entry in the product's order, each split into pieces of at most the stack size, each piece in the next empty
   place: `CObjectMgr::CreateItem`; type and type id from the definition, the count, **the definition's quality**, no gems, the make time
   now, **make type `'X'`** (0x58), `CItem::InitCraftExpansion` (zeros when craftable, `docs/original/item-origins.md`), a new unique id.
8. **The points are taken at once** in memory (`InterlockedExchangeAdd(+0x5a20, −price)`), and ps_dbAgent gets **`0x0E02`**
   (`TP_DB_POINT_ITEM_BUY`: `u32 user, char code[21], u32 price, u32 time, u8 'X', u8 n`, n × `TP_DB_POINTITEM_UNIT` (uid, item id,
   bag, slot, type, type id, count, quality, gems, craft name)).
9. **Answer 0**: `u8 0, u32 points left, TP_POINTLOG_UNIT (code, time, price), u8 n`, n × `TP_POINTITEM_UNIT` (`u8 bag, u8 slot, u8 type,
   u8 type id, u8 count`) = **37 + 5n** (`docs/client/item-mall.md` says 38 + 5n; the PDB struct and the send length say 37). The
   client writes each item at its place with its definition's quality and no gems (it moves and merges nothing itself), adds the log row
   and shows 824 "Purchased <b>.".
10. A game log record per item (`0x0410`; built, [The game log](game-log.md)).

**In ps_dbAgent** (`CUser::PacketUserPoint` 0x406ca0, case `0x0E02`; `DBCharacter::SaveBuyPointItem` 0x41ed10): the items go into its
copy of the bags (a place already taken there is logged as `[POINT ERROR]` and the connection is dropped), the log row into the first of
10 pending rows; then it posts database job **0x12** at once (0x406fe2; `OnDispatchDB` runs job *n* as case *n* − 2, here
`GetPointItemCount` → `CUser::SaveBuyPointItem`), and the character saves (`SaveCharacter`, `SaveCharacterTick`, the exit saves) retry
any rows still pending. In one transaction: `Usp_Save_Char_Item_Add_E` per item, then `usp_Save_User_BuyPointItems_NCash
(user, character, price, code, date)`, which takes the points from `Users_Master` (`usp_Update_UserPoint`: **−1 when they would go
below 0**) and inserts the `PointLog` row (use type 1). On failure the transaction is rolled back, the items are removed from
ps_dbAgent's copy only, and `CUser::SaveBuyPointItem` (0x404f00) posts job **9** (`SaveExitCharacter`, the character dropped on the
agent's side) or exits the user: `ps_game` is not told (section 9, oddity 1).

## 4. Gifts: `0x2603`, `0x2604`

Confirmed.

Request `TPC_POINT_GIFT_SEND` (44 bytes): `char code[21], char target name[21]` (NULs written at both ends). The answer is **always 57
bytes**: `u8 result, char target name[21], u32 points, TP_POINTLOG_UNIT`; the client reads the rest only on 0.

In `CUser::PacketPoint`:
1. `g_bDisableShop` 2 → nothing; 1 → **6**.
2. Unknown code → **6**; points below the price → **10** ("Insufficient points."); an empty name → **1** ("The character doesn't exist.").
3. **The request lock** `+0x5a24` set (compare-and-swap 0 → 1); already set → **9** ("Previous request in process…").
4. Every used entry must have a definition with a stack size above 0, else **6** — **with the lock left set** (section 9, oddity 2).
5. ps_dbAgent gets **`0x0E03`** (`u32 user, char target[21], char code[21], u32 price, u32 time`, 56 bytes). No points are taken yet.

In ps_dbAgent (`DBCharacter::SaveGiftPointItem` 0x41f150), one transaction:
- `usp_Save_User_GiftPointItems_NCash`: the target is `SELECT UserUID FROM Chars WHERE CharName = @TargetCharName AND Del = 0` (any
  faction, this world only) — none → **1**; the buyer's own account → **2**; the target's mall storage (`UserStoredPointItems`, 240
  slots per account and world) already full → **3**; then **one storage row per product entry** (item id and count as in the
  product, not split by stack size) in the lowest free slots — no free slot → **4**; insert error → **5**; a product without a first
  entry → **6**; a `PointLog` row for the buyer (use type 2, with `TargetName`) — error → **8**. The new rows are returned.
- `usp_Save_User_NCash`: takes the points through the outside billing server (`[PS_NCASH].[Billcrux30]`; its answers mapped to 10
  insufficient, 1 no user, 6 no product, 7 other). (That linked server is not in the shipped databases: gifts always failed with 7 there.)
- The target account loaded in ps_dbAgent (connected to this world): the rows go into its copy of the storage; otherwise
  **`usp_Save_User_GiftPointItemNotify`** adds a `PointGiftNotify` row (account, code, sender's name, date).
- `0x0E03` back (`TP_DB_POINT_ITEM_GIFT_SEND_RES`: `u32 user, u8 result, u32 target user, char target[21], char code[21], u32 price,
  u32 time, u8 n`, n × `u8 slot, u8 type, u8 type id, u8 count`; type and type id are the item id's thousands and remainder); a failed
  database step sends **5**.

Back in the game (`PacketUserDBPoint` case `0x0E03`): on 0 the price is taken from the held points and, **if the target account is in
this world** (`CWorld::FindUserBill`: its connection, in the world or at the character screen), its storage copy gets the rows and it
receives **`0x2604`** (`TP_POINT_GIFT_RECV`: `char sender[21]` — the buyer's character name, `char code[21], u8 n`, n × `u8 slot, u8 type,
u8 type id, u8 count` = **45 + 4n**; the client: 831 "Received <b> from <p>." and the rows into its storage window). Then the buyer
gets `0x2603` with the result, the target name, the points and the log row (code, time, price) — 827 "Sent <b> to <p>." on 0; 1 → 347;
3, 4 → 828 "Recipient's inventory is full."; 5, 6 → 239; 9 → 830; 10 → 823; others (2, 7, 8) → 829 "Can't send the present." — and the
lock is cleared.

**Offline notices**: at the next character load ps_dbAgent reads and deletes the account's `PointGiftNotify` rows
(`DBCharacter::LoadUserGiftNotify` 0x41f870, `usp_Read_User_GiftPointItemNotify`, oldest first, at most 20) and sends `0x0E07`; the game
sends one **`0x2604` with n = 0** per row (sender, code: 45 bytes; the client: 832 "You got a new present.").

## 5. The item-mall storage: `0x2606`, `0x2607`

Confirmed.

- 240 slots per account and world (`UserStoredPointItems`: account, slot, item id, count, buy date), in the game at `+0x5a2c` (240 ×
  `u8 type, u8 type id, u8 count`), filled from ps_dbAgent's `0x0E04` at character load.
- **`0x2606`** at world entry, right after `0x2601` (`CUser::SendCharacterStoredPointItem` 0x483f80), **only when the storage is not
  empty**: `u8 n`, n × `u8 slot, u8 type, u8 type id, u8 count` (slots in order) = 3 + 4n.
- **`0x2607`** request (3 bytes: `u8 slot`): slot below 240, a stored row there (count non-zero) and its item defined; the **first empty
  place** in bags 1 to the bag count (slots 0 to 23): a new item (`CItem::Init`: type, type id, **the stored count as it is**, the
  definition's quality, make time now, make type **`'X'`**; `InitCraftExpansion`; a new unique id), the storage slot cleared; ps_dbAgent
  `0x0E05` (`TP_DB_POINT_STORED_ITEMGET`, 58 bytes) → `usp_Save_User_StoredPointItems` (the bag row inserted and the storage row deleted in
  one transaction); a game log record (`0x0411`); answer **`0x2607 u8 slot, u8 bag, u8 slot`** (5 bytes). Any failure (bad slot, empty,
  no definition, no empty place) → `0x2607 slot, 0, 0`, which the client takes as "nothing".

## 6. The billing storage: `0xB101`, `0xB102`

Confirmed.

The same kind of storage, filled from outside: `PS_Billing.dbo.Users_Product` (account, slot, item id, count, product code, order number,
verify code, buy date; one database for every world), which an outside web shop fills (`usp_Insert_Product_User_E`).

- Read with the character list (`DBCharacter::LoadBillingItem` 0x41ead0, `usp_Read_Char_Product_Item_E`), kept at `+0x7c4` (240 × 3
  bytes, as section 5).
- **`0xB101`** in the enter burst (`CUser::SendCharacterData` 0x483140, between `0x0105` and `0x0106`), only when not empty: `u8 n`, n × `u8 slot, u8 type,
  u8 type id, u8 count` = 3 + 4n.
- **`0xB102`** request (3 bytes: `u8 slot`; `CUser::PacketBilling` 0x457b40, every other `0xB1xx` closes the connection with reason 9):
  as `0x2607`, with **make type `'B'`** (0x42); ps_dbAgent `0x0710` → `usp_Save_Char_Product_Item_E` (the bag row, a `ProductLog` row with
  the order, the `Users_Product` row deleted); game log `0x040D`; answer `0xB102 u8 slot, u8 bag, u8 slot`, failure `slot, 0, 0`.

**Opening the windows** (client research, `docs/client/research-movement-storages-auction.md` "Queue 2", verified in `game.exe`):

- **The item-mall storage**: open the point shop and click its **store button** (`data/interface/ins_market_store_button.tga`, near
  left+25, top+401 of the mall window; the label is to be seen live). The client closes the mall and opens the storage with the inventory
  beside it.
- **The billing storage**: click a **type-1 merchant whose NpcQuest.SData extra byte is 9**, then its main menu action. In the client's
  data these are **1/179 "Light's Bank Teller"** and **1/180 "Fury's Bank Teller"** (the extra byte is the merchant-only byte after type
  and id, not the NPC type 9). Place them from the server's existing map placements.
- **Opening sends nothing.** `0x2606` and `0xB101` are receive-only pushes and do not open a window; the server already sends both at
  world entry (section 8), which is what the windows need. The NPC click is the usual `0x0E04`.
- **Taking an item is a drag** from the storage into the inventory: `0x2607` / `0xB102` with `u8 storageSlot` (3 bytes). The client sends
  no destination; the server picks the bag and place and answers in the 5-byte reply (sections 5 and 6).

## 7. Premium accounts

Confirmed.

- The **billing type** `m_nUserBillingType` (`+0x5854`) comes from the session server's user record in the login answer
  (`docs/original/ps_game.md` section 3.2, `byBillingType`; billing is turned off in the shipped `ps_session` configuration), with a
  remaining time `+0x5858` (0 from the session server; `CWorld::BillingRemainTime` 0x40c440 can set it from the manager connection).
- **`0xB103`** right after the handshake reply (`CUser::SendBillingInfo` 0x457ad0): `u8 type, u32 value` (7 bytes). The US client (region
  6): 1 → 814 "You are using individual package. (expire date : <b>)", 2 → 815 (the franchise PC room, 1.5× experience), 3 → 816, 4 →
  863 (missing from this client: "GetMsg Error 863"), 0 → nothing; `value` is shown as a **packed date** (`docs/client/CLIENT-FACTS.md`).
  Client reader 0x581d20 / logic 0x559930 (client research, "Queue 4"): region 6 reads `value` as the **packed local date**
  `(year-2000)<<26 | month<<22 | day<<17 | hour<<12 | minute<<6 | second`, takes **only year, month and day** (shown `YYYY-MM-DD`),
  prints the line **once** on receipt (chat category 7), keeps the type (0x21e55a4) and does not keep the expiry or count down. Types:
  **1** → 814 with the date, **2** → 815 without a date (1.5× EXP), **3** → 816 with the date, **4** → 863 missing from this client, 0
  or other → no line. (Region 2, China, would read seconds remaining; not this client.) `0xB104` is a separate free-service packet.
  The server's `0xB103` (`server.rs`, `billing_info`) sends the type and `packed_from_unix(premium_until)`, the same layout (the
  server's `year - 16` masked to 6 bits is the client's `year - 2000` to 2063); no end or type 0 sends 0. One thing to know: type 1 or 3
  with no `premium_until` sends value 0, which the client would show as a zero date, so give those an end date (`mall premium <user>
  <type> <days>`).
- **`CUser::BillingApply`** 0x457a70, called once in `PacketUserDBChar` when the character's data arrives (before the stats are
  computed): **type 1** → `m_bSkillBank2X` + 1 (the warehouse's second half); **types 2 and 3** → `m_bSkillNDQuality` + 1 (equipment does
  not wear), `m_bSkillGetExpUP15X` + 1, `m_bSkillBank2X` + 1. Other types: nothing. The flags are counters, so an item buff with the
  same flag ending (`−1`) leaves premium's in place.
- **`CUser::BillingRemove`** 0x457aa0 (the reverse) has **no caller**: premium lasts until the character leaves the world, whatever the
  remaining time does.
- What the flags do (`docs/original/item-flags.md`): `ExpUP15X` gives **× 1.5** experience alone and **× 2** with an EXP stone (`CUser::
  AddExpFromUser`; + 0.2 with a charm; quest rewards never), `NDQuality` = no wear (`docs/original/repair.md`), `Bank2X` = warehouse
  slots 120 to 239 (`docs/original/warehouse.md`).
- The only direct reader of the type besides those: **`CUser::ItemGemAdd`** (0x46165a): with **type 2 or 3** the lapis link rate is
  **`trunc(rate × 1.5)`** (f32), after the Linking Hammer and before the guild house bonus and the clamp to 1–50
  (`docs/original/lapis.md`).
- Nothing else reads it (`CUser::EnterWorld` only copies it into the game log record).

## 8. Order in the enter burst

Confirmed, `PacketUserDBChar` 0x46d5e3 onwards.

`0x0104` … `0x0105`, **`0xB101`** (if any, inside `SendCharacterData`), `0x0106` … `0x0230`, **`0x2601`**, **`0x2606`** (if any), the
guild house lists, `0x0114`, `0x021B` … (`docs/original/enter-world.md` items 5, 17 and 18). `0xB103` came earlier, after the handshake.

`0xB106`, which the US client sends once after its scene set-up, arrives before `0x0201` (seen live: "not handled in the world yet …
entered=false"); in that state the original ignores every `0xB1xx` (`docs/original/ps_game.md` section 5.4). In the world any `0xB1xx`
but `0xB102`, and any `0x26xx` but the four requests, closes the connection (reason 9).

## 9. Original oddities

1. **Points are spent twice over**: a purchase takes them from the game's held copy and writes them later; the database takes them only
   then, and two game servers (each with its own copy of the account's points) can both sell against the same total. When the later
   write fails (the points are short in the database), ps_dbAgent rolls back, drops the items from its own copy and then the character
   itself on its side (job 9, `SaveExitCharacter`), without a word to the game: the player keeps the items and plays on in a session
   whose later saves have no character to go to. Not patchable in `ps_game` alone (`docs/original/server-patch-notes.md`, "What pass 2 does not
   fix"); the write itself is queued at once, so within one world and with no outside point changes the window is the database queue's
   delay.
2. **A gift of a product with an undefined item leaves the request lock set** (section 4, step 4): until the character leaves the world,
   every purchase answers 4 (nothing shown), every gift 9, and `0x2605` is ignored.
3. **Gifts never worked in the shipped setup**: the points are taken by a linked billing server that is not there (result 7, "Can't
   send the present.").
4. The gift answer's log row and the purchase's carry the game's own time; the `PointLog` row is written with ps_dbAgent's clock.
5. A gift whose recipient is at the character screen sends it `0x2604` there, and writes no notice for later.
6. A stored row keeps the product's count even above the item's stack size, and taking it makes one item with that count (no product in
   the shipped list does this). A product needing more than 24 places would overrun the 24-row answer (none does: at most 15).
7. "Phukey Long Cape" is in the client's mall but not in the server's list: buying it answers 1.
8. A purchase checks nothing about the character's state (dead, trading, in a shop, anywhere on any map).

## 10. The new server

Before this was built `0x2601` was sent with 0 points and no log, `0xB103` with type 0, and everything else was ignored.

To build, as the original unless a deviation says otherwise:

- **Data** (migration 0028): `accounts.points` (`integer`, ≥ 0), `accounts.premium` (`smallint` 0–3) and `accounts.premium_until`
  (`timestamptz`, NULL = no end); `point_log` (account, character, kind 1 purchase / 2 gift, product code, points, target name, at);
  `point_items` (the item-mall storage: account, server, slot 0–239, item id, count, at); `point_gift_notices` (account, server, product
  code, sender name, at); `billing_items` (the billing storage: account, slot 0–239, item id, count, product code, at — one per account,
  every server, as `PS_Billing`). The product list from `defs.product_list` at start (warnings for undefined items and counts above a
  stack).
- **Packets**: `0x2601` with the points and this character's last 10 log rows; `0x2605` (40 s as the original, from the database);
  `0x2602`, `0x2603`/`0x2604`, `0x2606`/`0x2607`, `0xB101`/`0xB102`; `0xB103` with the account's premium.
- **Premium**: types 1 to 3 as section 7 (warehouse half; no wear, × 1.5 experience — × 2 with a stone, as `docs/original/item-flags.md`
  deviation 1 already says — and lapis × 1.5 for 2 and 3), applied at each world entry.
- **Operator tool** (`login-server` crate, next to `create_account`): `points <user> show | add <n> | set <n>`, `premium <user> <type>
  [<days>]` (0 ends it), `deliver <user> <product code>` (into the billing storage). A change to points notifies the game servers
  (`pg_notify('account_points', …)`, as the duplicate-login kick), which send `0x2605` to the account's player in the world.
- **Setting** `item_mall` (default on; off = the original's `g_bDisableShop` 1: purchases answer 1, gifts 6).
- Game log records: purchases and storage takes are written (`mall_buy`, `mall_stored_take`, `billing_take`; [The game log](game-log.md)).
- Live test: a new checklist section (purchase, a full bag, short points, a gift to an online and an offline character, the storage
  takes, premium 1 and 2 with the warehouse, wear, experience and a lapis link).

## 11. Deviations (approved; built, section 13)

1. **One transaction for each purchase, gift and storage take**, with the points checked and taken in the database (`UPDATE … WHERE
   points >= price`): short points answer 2 (purchase) or 10 (gift) and nothing is given (oddity 1). The answers are the original's.
2. **The gift lock is not left set** (oddity 2): an undefined item answers 6 and the character can go on buying.
3. **Gifts take the points from the account directly** (oddity 3: no outside billing server).
4. **Premium comes from the account** (`accounts.premium`, `premium_until`), set by the operator tool, not from a billing service
   through the session server. `0xB103` sends the type and the end date packed (the client's "expire date"; no end = type alone with
   value 0). Premium is read at each world entry and lasts until the character leaves the world, as the original (no removal); an
   ended premium (`premium_until` passed) is type 0.
5. **Points are charged by the operator tool**, and a player in the world sees the new total at once (`0x2605`); the original left that
   to an outside page and the next refresh.
6. **Offline gift notices are sent after the enter burst**, and a recipient not in the world (offline or at the character screen) gets a
   notice instead of a `0x2604` it cannot use (oddity 5).
7. **Stored counts above an item's stack size are split** into several storage rows when the gift is stored, and a product needing more
   than 24 places answers 3 (oddity 6; the shipped list has neither, so this changes nothing today).
8. **The billing storage is filled by the operator tool** (`deliver`), as a delivery box (the original's web shop and its order and
   verify codes are not built).

Kept as the original: no state check on purchases (oddity 8), 40 s refreshes, `0xB106` not answered (and ignored, as before the zone
entry; other unknown `0x26xx`/`0xB1xx` stay logged and ignored as every unhandled packet is now), gifts to any faction on the same
server but not to one's own account, the product without a server row answering 1.

## 12. Open

- The windows' opening is known (section 6), to be confirmed live: the store button, and the Light's/Fury's Bank Teller.
- A gift taken while the item-mall storage window is open, and the billing storage delivered while in the world (seen at the next
  world entry here).
- What message 816 (premium type 3) says; types 2 has no date by design (section 7).
- Whether the client shows `0x2604` n = 0 notices correctly when they come right after the burst.

## 13. As built

- **Data**: migration `0028_item_mall.sql` (`accounts.points`, `premium`, `premium_until`; `point_log`, `point_items`,
  `point_gift_notices`, `billing_items`). `pg::load_products` reads `defs.product_list` at start; the warnings of
  `mall::Products::problems` go to the log (none with the shipped list: 84 products).
- **Rules** (`crates/game-server/src/mall.rs`): `Products`, `plan_buy` (pieces by stack size into the empty places, result 3 with none, too
  few or more than 24), `gift_rows` (6 for an unusable item; split rows, deviation 7), `new_item` (the definition's quality, make type
  `'X'` or `'B'`, zero craft stats when craftable).
- **Store** (`backend::PointStore`, PostgreSQL and memory): `points`, `buy` (points, items and the log row in one transaction), `gift` (the
  recipient by name in any case on this server, own account refused, the recipient account locked while its free slots are read, the
  points, the rows, the log), `take_stored` (the row out and the item in together, for either storage), `add_gift_notice`,
  `take_gift_notices`. `CharacterStore::load` brings `PlayCharacter::mall` (points, premium, the character's last 10 log rows, both
  storages); `SessionGate::claim` brings the premium for `0xB103`.
- **Connection** (`server/mall.rs`): `0x2602`, `0x2603`/`0x2604`, `0x2605` (the database once in 40 s), `0x2607`, `0xB102` in the world;
  the burst sends `0xB101`, `0x2601` with the log and `0x2606`; gift notices after the burst; `OnlineUsers` keeps each account's events
  while it is in the world and a `MallInbox` (gifts that came, points changed) told by `ZoneEvent::Mall`; gifts not told when the
  character leaves (or arrive while it stays after its connection ended) become notices.
- **Premium**: `PlayerBuffs::premium` (the type at world entry) in `PlayerBuffs::effects`: type 1 the warehouse's second half, 2 and 3 also
  no wear and `exp_premium` (× 1.5, × 2 with a stone); `lapis::link` takes `premium` (× 1.5 after the hammer). `0xB103` sends the type
  and the packed end date.
- **Operator tool** `mall` (`crates/login-server/src/bin/mall.rs`): `show`, `points add|take|set` (with `pg_notify('account_points')`,
  which the game server's listener turns into `0x2605` for a character in the world), `premium <type> [<days>]`, `deliver <code>`.
- **Setting** `item_mall` (default on).
- Tests: `point` (sizes and layouts), `mall` (the list, purchases, gifts), `buffs` (premium flags), `lapis` (the premium rate), `enter`
  (burst order), flow tests (a purchase, refusals and refreshes; gifts online and offline, takes, notices at entry; premium 1 with the
  warehouse and the billing storage) and a PostgreSQL test (purchase, gift, refusals, loading, takes, notices, the shipped list).
- Live test: `docs/testing/live-test-checklist.md` section 44.

## 14. Other episodes

The EP3 builds of `ps_game.exe` (2008-01-17) and `ps_dbAgent.exe` (2008-01-04) and the EP5.4 builds (2011-09-14, `ps_game` modified
after linking) were compared with the EP4.5 builds of this note, read statically like the rest of the note; overview in
[`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **The same four requests** (confirmed): `PacketPoint` (EP3 ps_game 0x45eb60, matched by dispatch) has `0x2602`, `0x2603`, `0x2605` and
  `0x2607`; the database agent's point group is `0x0E02` to `0x0E09` (EP4.5 to `0x0E0A`).
- **EP3 answers `0xB106`** (confirmed): in state 3 it sends the product list (EP3 ps_game 0x441950 → sender EP3 ps_game 0x49b440), rows
  of 76 bytes (length `count × 76 + 3`, EP3 ps_game 0x49b456), and the console word `/initpl` reloads the list. Comparing with EP3
  showed that EP4.5 dropped an answer the earlier build gave; "never answered" holds for EP4.5 only.
- **No billing info `0xB103`** after the handshake (confirmed: the constant is not in the EP3 image).
- **Points go through `*_PayLetterT` procedures** (confirmed, `ps_dbAgent` strings), which are not in the EP3 database backup. EP3's
  `PS_Billing` has 3 tables; its delivery procedure `usp_Insert_Product_User_E`, encrypted in EP4.5, is in clear text: at most 240 slots
  per user, the product read from `ProductList` through a linked server, one `Users_Product` row per item, all in one transaction.

### EP5.4

- **The product list is sent** (confirmed): `CGameData::LoadProductList` (EP5.4 ps_game 0x4dbd00, matched by its string) reads a 51st
  column `ProductName` and builds up to 50 ready `0xB106` pages of **25 rows** (EP5.4 ps_game 0x4dbf34). They answer the one `0xB106`
  sent in state 3 and are pushed to every player after `/initpl`, which also reopens a mall an operator had closed (EP5.4 ps_game
  0x409e23). The EP5.4 client copies at most 64 rows without checking the count.

| offset | `0xB106` S→C (3 + 76n) |
|---|---|
| 0 | u16 opcode |
| 2 | u8 n |
| 3 + 76i | char code[21] |
| +0x15 | char name[51] |
| +0x48 | u32 price |

- **Points come from a billing service through ps_session** (confirmed, `PacketPoint` EP5.4 ps_game 0x487f20, matched by opcode switch
  table): the refresh asks the session (`0x1B01`) only while it reports billing up; a purchase takes a per-character lock (the flag that
  EP4.5 only reads, EP5.4 ps_game 0x488183) and a purchase number stored in each item (`CItem+0x9c`) and sent to the session (`0x1B02`).
  A refused payment takes the items back by that number (client `0x0206`, agent `0x0702`) and answers result 2; a gift asks the session
  first (`0x1B03`), and a refused gift clears the target's storage rows (new client `0x2609`). Storage rows grow to 8 bytes, under a lock.
- **Oddity 1** (section 9) is answered in part: items are still given before the points are confirmed, but a refusal now takes them
  back (confirmed). **Oddity 2** is still there and the stuck lock now blocks purchases too (confirmed, EP5.4 ps_game 0x48896d).
- `/disablegift` (`g_bDisableShop` 3) stops gifts only (confirmed, EP5.4 ps_game 0x409c38).
- The shipped setup has no PayLetter section, so refresh and gifts stay silent (inferred); the database agent's four new procedures (`usp_Save_User_BuyPointItems2` and three more) are in no
  database backup (confirmed by search).
- Data (confirmed, rows): `ProductList` 84 → 249 codes, 24 in common and all repriced, prices up to 50,000.
