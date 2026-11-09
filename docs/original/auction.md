# The auction house

This page covers the auction house (`0x1Cxx`) in the EP4.5 game server `ps_game.exe`: where it works, the auctions
and the two "to collect" lists the server keeps, searching and paging, the four lists, registering, cancelling, buying now, bidding,
collecting items and gold, the end of an auction, the interest list, the GM's `/auctionrecall` and `/auctionsearch`, and what the
original stores. It was read in the Ghidra decompiler and the disassembly with the PDB names; struct sizes and fields are the PDB's.
Nothing was executed.

The client's side (every request and answer as `game.exe` sends and reads them, the client's own checks, its messages and the row
layouts) is on the client's [Auction house](../client/auction-house.md) page and is not repeated here, except where the server adds to it or
disagrees. Not covered: the game log records (`0x0A08` to `0x0A0C`, `0x042A` to `0x042F`, the `TP_GAMELOG_*MARKET*` structs) and the
database agent's side of the stored procedures.

## In short

- **Only on map 42**, the Auction House. Every auction request, even the lists and collecting, does nothing anywhere else.
- **Registering** costs a deposit: the period's hours × 5 gold for every started 500,000 of the minimum price. **The deposit comes back
  only on a sale**; a cancel or an unsold end keeps it.
- **Periods** are 7, 24, 72 or 168 hours. The minimum price must be at least 500 and below the buy-now price. Quest items and bound
  items cannot be auctioned.
- **Bids** must be at least 5 % above the current price, so even the first bid must beat the minimum by 5 %. A bid does not extend the
  auction. A bid at or above the buy-now price gets no answer at all.
- **The seller pays 2 %** of the sale price.
- **Nothing goes straight into the bags.** Won items, unsold items and gold all wait in "to collect" lists for 14 days.
- **An auction ends up to a minute late**: the server checks once a minute.
- **The GM's `/auctionrecall`** disconnects the seller and gives the item to the GM, deposit lost.
- **Searches are newest first, 17 to a page**, and the item name filter is case-sensitive.

Status words: **confirmed** = read in the code (address given). `CUser` offsets used below: `+0x128` character id, `+0x140` gold,
`+0x188` name, `+0x1c4` the bags (`CItem*[6][24]`, bag 0 the equipment), `+0x1304` the number of bags, `+0x578c` the account id,
`+0x5d14` the interest list. `CItem`: `+0x30` the definition, `+0x38` unique id (u64), `+0x40` type, `+0x41` type id, `+0x42` count,
`+0x44` quality, `+0x46` gems[6], `+0x4c` craft name[21], `+0x64` made at, `+0x68` make type. `g_MarketMgr` (`CMarketMgr`) is at
0x3ed3338.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`tools/disasm.py`, names from
`docs/original/symbols/ps_game.tsv`); struct sizes and fields from the PDB (`docs/original/symbols/ps_game.packets.txt`). Client side:
`docs/client/auction-house.md` (every request and answer as `game.exe` sends and reads them, the client's own checks, its messages and
the row layouts), not repeated here except where the server's side adds to it or disagrees.

Covered: where the auction house works, the auctions and the two "to collect" lists the server keeps, searching and paging, the four
lists, registering, cancelling, buying now, bidding, collecting items and gold, the end of an auction, the interest list, the GM's
`/auctionrecall` and `/auctionsearch`, and what the original stores. **Not covered:** the game log records (`0x0A08` to `0x0A0C`,
`0x042A` to `0x042F`, the `TP_GAMELOG_*MARKET*` structs) and `ps_dbAgent`'s side of the stored procedures (their SQL bodies are in the
restored `PS_GameData`, not read; section 13).

`CUser` offsets used below: `+0x128` character id, `+0x140` gold, `+0x188` name, `+0x1c4` the bags (`CItem*[6][24]`, bag 0 the
equipment), `+0x1304` the number of bags, `+0x578c` the account id, `+0x5d14` the interest list. `CItem`: `+0x30` the definition,
`+0x38` unique id (u64), `+0x40` type, `+0x41` type id, `+0x42` count, `+0x44` quality, `+0x46` gems[6], `+0x4c` craft name[21], `+0x64`
made at, `+0x68` make type. `g_MarketMgr` (`CMarketMgr`) is at 0x3ed3338.

## 1. Where and when

Confirmed.

- **Map 42 only.** `CMap::LoadConfig` 0x426940 ends with `map +0x1e0 = (map id == 42)` (0x42706f: `cmp [esp+0x218],0x2a; sete`), the
  only writer. `CUser::PacketMarket` 0x477740 returns **without an answer** unless the player's zone's map has that flag, so every
  `0x1Cxx` request (searches, lists and collecting included) works only on map 42, the Auction House. The GM's `FA19`/`FA1A` check the
  same flag (section 12). `CServer::InitWorldThread` 0x406980 gives the zone of that map its own world thread.
- **Open only after loading.** `g_dwMarketOpen` (0x546f8c) starts 0; the database agent's `0x020F` ("Market End -----", after the
  market lists below, `CClientToDBAgent::PacketSystemConnect` 0x408c00) sets it to 1. While it is 0 **every** `0x1Cxx` request is
  answered `0x1C13` with `byInfo` **1** (`13 1C 01`, 0x477780) and nothing else happens. This is the only place that sends `0x1C13`;
  the client's note gives 0 "preparing" and 1 "opened" (`docs/client/auction-house.md`, "Answers"), so either the client's mapping or the
  original's byte is the other way round (open; the new server never sends it, section 15).
- `CZone::UpdateMarket` 0x4232f0 (the expiry pass for one zone) is never called; the pass runs from `CWorldThread::Update` (section 10).

## 2. The data

Confirmed.

Everything lives in `CMarketMgr`'s memory, filled once at start from the database agent (section 13) and written back message by
message. Nothing is read from the database while running.

### 2.1 An auction (`CMarket`, `CMarketMgr +0x124`: `map<market id, CMarket*>`)

| field | offset | |
|---|---|---|
| market id | +0x08 | `GenMarketID`: `InterlockedIncrement(+0x178)`, seeded from the agent's `0x0F05` |
| seller's character id | +0x0c | |
| the item | +0x10 | a `CItem` taken out of the seller's bags |
| end date | +0x14 | packed local time (2.5) of registration time + the period |
| minimum price `dwMinMoney` | +0x18 | |
| buy-now price `dwDirectMoney` | +0x1c | |
| period `byMarketType` | +0x20 | 0 to 3 = **7, 24, 72, 168 hours** (table 0x5318e8; above 3 is made 3) |
| deposit `dwGuaranteeMoney` | +0x24 | the registration fee (5.2) |
| top bidder's character id | +0x28 | 0 none |
| current price `dwTenderMoney` | +0x2c | **the minimum price until the first bid**, then the top bid |
| end tick | +0x30 | `GetTickCount()` + seconds until the end date × 1000 (0 when loaded already past its end) |
| seller's name | +0x34 | 21 bytes |
| top bidder's name | +0x49 | 21 bytes |

Besides that map, `+0x130` (`map<market id, market id>`, "all market list") keeps an entry for every auction made or loaded since the
start; an ended auction's entry is set to 0 (`SetEndIntheAllMarketList` 0x438f30), never removed. The search uses it to find its place
again when the auction it stopped at is gone (3.3). Per character: `+0x160` the ids it registered, `+0x16c` the ids it is the top
bidder of (`AddCharRegItem` / `AddCharTenderItem`, `DeleteChar…`); the lists skip ids that are no longer auctions.

### 2.2 An item to collect (`ResultItem`, 24 bytes; `+0x148`: `map<character id, list>`)

`+0x08` market id, `+0x0c` result type, `+0x10` the `CItem`, `+0x14` **collection deadline** (packed). Made by `AddCharResultItem`
0x439d60 (character, market id, result type, item, deadline); appended to the character's list.

### 2.3 Gold to collect (`ResultMoney`, 68 bytes; `+0x154`)

`+0x08` **money id** (`InterlockedIncrement(+0x17c)`, seeded from the agent's `0x0F02`; a counter apart from the market ids), `+0x0c`
market id, `+0x10` result type, `+0x14` money (the price paid), `+0x18` deposit, `+0x1c` **return money** (what collecting pays), `+0x20`
collection deadline, then the item's type, type id, quality, gems, count and craft name for display. Made by `AddCharResultMoney`
0x439eb0, which returns the new money id.

### 2.4 Result types (the `byResultType` of the rows)

| | item row | gold row |
|---|---|---|
| **0** | won or bought (the buyer's) | sold (the seller's) |
| **1** | returned unsold or cancelled (the seller's) | refund of an outbid or cancelled bid (the bidder's) |
| **2** | taken by a GM's `/auctionrecall` (the GM's) | — |

The client does not read them for display (`docs/client/auction-house.md`, Open; client research, section 2.7 below, covers the list row only).

### 2.7 What the client draws of a list row (client research, "Queue 5", verified)

`TP_MARKET_ITEM` (48 bytes, copied by 0x45acd0, drawn by 0x458d20 for the search, 0x456810 for my registrations and bids, details 0x45cd50):

| offset | field | use |
|---|---|---|
| +0 | u32 marketId | selection, bid, buy, interest; shown only under staff/debug flags |
| +4 | u32 tenderMoney | "Bid now :" (7041), gold |
| +8 | u32 directMoney | "Buy now :" (7042), gold |
| +12 | u32 endDate | remaining time (0x454a30; corrected by the 0x0114 clock offset; nothing shown once expired) |
| +16/+17 | u8 type, typeId | Item.SData lookup: name, name colour and icon |
| +18 | u16 quality | tooltip 0x49fa00 (mode 7) |
| +20 | u8 gems[6] | the same tooltip |
| +26 | u8 count | icon count |
| +27 | char craftName[21] | the same tooltip (craft stats, enchant) |

No seller or bidder name is in the row. Quality, gems and craft data are not columns but are shown in the hover tooltip, so they
matter. **Server: no change**; keep every field filled as the original does (the research found it unsafe to zero any field other than
type, type id and id). `byResultType` exists only in the 41/53-byte collection rows and was not part of that question.

### 2.5 Dates

Packed local time as everywhere (`shaiya_proto::world::pack_time`): `(year − 2000) << 26 | month << 22 | day << 17 | hour << 12 | minute
<< 6 | second` (the original computes `(year − 16)` with overflow; the same low 6 bits). **The collection deadline is always "now + 14
days"** (`CMarketMgr::GetResultEndDate` 0x438ec0: `localtime(time() + 1,209,600)`), taken when the row is made. Packed values are
compared as numbers.

### 2.6 The interest list (`CUser +0x5d14`, `list<market id>`)

Per character, **10 at most**, loaded with the character (`CUser::PacketUserDBSetInfo`, `TP_DB_INFO_CONCERN_MARKET`) and freed at
`CUser::ResetCharacter`; written back whole after each change (`CUser::SendConcernMarket` 0x484010: agent `0x061A`,
`TP_DB_SET_CONCERN_MARKET` with the account id and up to 10 ids; the table is per character, `MarketCharConcern`).

### 2.7 The search state (`SearchInfo`, `+0x13c`: `map<character id, SearchInfo*>`)

`+0x08` has-previous, `+0x09` has-next, `+0x0a` country filter, `+0x0b` lowest level, `+0x0c` highest level, `+0x0d` category, `+0x0e`
grade, `+0x0f` name (32 bytes), `+0x30` **mode** (0 a normal search, −1 a seller-name search, otherwise an item id `type × 1000 + type
id`), `+0x34` rows on the current page, `+0x38` previous-page cursor, `+0x3c` next-page cursor, `+0x40`/`+0x44` the page's highest and
lowest market ids (3.3). Made on a character's first search and kept; **freed when the character goes from map 42 to another map**
(`FreeCharSearchInfo` from `CUser::PacketMaininterface` 0x468ef5).

## 3. Searching

Confirmed.

### 3.1 `0x1C01` search (`SearchFirst` 0x43ad00)

`TPC_MARKET_SEARCH`: `u8 country, u8 level min, u8 level max, u8 category, u8 grade, char name[32]` (the server sets the name's last
byte to 0). A highest level of **0 becomes 255**. The search state gets these, mode 0, cursors and flags 0, then `SearchMarketFirst`.

**The filters** (mode 0), on the item's definition (`stItemInfo`):

- **Country** (`country` ≠ 0, and the item's type is not **30**, lapis, whose country column holds link flags): the item's `byCountry`
  (`defs.items.country`: 0 human, 1 elf, 2 Light, 3 deatheater, 4 vail, 5 Fury, 6 any): country **1 hides 3 to 5**, any other value
  **hides 0 to 2**; 6 always passes.
- **Level**: `level min ≤ (u8) wLevel ≤ level max` (`ReqLevel`, the low byte; shipped values are 0 to 99). What the client puts in these
  two bytes (the level, or a tenth of it) is not settled (`docs/client/auction-house.md`, "Requests", says "minLv/10"); live test.
- **Category** (≠ 0): equals the item's `byMarketItemType` (`stItemInfo +0xa4`), made at load from the item type by `GetItemMType`
  0x4bdb10 (the first of these lists, 1 to 25, holding the type; else 0):

  | category | item types | category | item types |
  |---|---|---|---|
  | 1 | 2, 4, 46, 48 | 14 | 20, 35, 70, 76, 85, 91 |
  | 2 | 1, 3, 45, 47 | 15 | 21, 36, 71, 77, 86, 92 |
  | 3 | 5, 49, 50 | 16 | 24, 39 |
  | 4 | 6, 51 | 17 | 19, 34, 69, 75, 84, 90 |
  | 5 | 7, 8, 53, 54, 55, 56 | 18 | 23, 38 |
  | 6 | 9, 15, 57, 65 | 19 | 22, 37 |
  | 7 | 10, 58 | 20 | 40, 41 |
  | 8 | 12, 60, 61 | 21 | 30 |
  | 9 | 13, 62, 63 | 22 | 95 |
  | 10 | 11, 14, 59, 64 | 23 | 25, 44 |
  | 11 | 16, 31, 66, 72, 81, 87 | 24 | 42 |
  | 12 | 17, 32, 67, 73, 82, 88 | 25 | 100 |
  | 13 | 18, 33, 68, 74, 83, 89 | | |

- **Grade** (≠ 255): equals the low byte of `wDex` (`ReqDex`, the name colour group: 0 normal … 8; the community notes call it "also used
  for auction sorting").
- **Name** (not empty): **`strstr(item name, name)`**, so a part of the definition's name, **case-sensitive**.

Mode −1 (a seller-name search, 12.2) matches `strcmp(seller's name, name) == 0`; an item id mode matches `dwItemID` (`type × 1000 +
type id`) and ignores every other filter.

### 3.2 The first page (`SearchMarketFirst` 0x43a070)

The auctions are walked **from the highest market id down** (newest first). Matching auctions fill the page; the first sets `+0x44`,
each sets `+0x44` to its id (so `+0x44` ends as the page's lowest id). When **17** are on the page and an 18th matches, its id becomes
the next cursor `+0x3c`, has-next is set and the walk stops. Then `+0x34` = rows, has-previous 0, and the answer is
**`0x1C01`** for mode 0, **`0x1C02`** otherwise: `u8 has-previous, u8 has-next, u8 n, n × TP_MARKET_ITEM` (5 + 48n; sent with n = 0
too). `TP_MARKET_ITEM` (48): market id, **current price** (`+0x2c`), buy-now price, **end date**, type, type id, quality, gems, count,
craft name.

### 3.3 `0x1C03` paging (`SearchSection` 0x43b060; `u8 action`)

Nothing happens without a search state. **2 next** (only with has-next, `SearchMarketNext` 0x43a680): the walk starts at the next
cursor; if that auction is gone, at the next lower id still an auction (through `+0x130`); none → no answer. Before walking, the
previous cursor `+0x38` = `+0x44` (the old page's lowest id), next cursor and has-next 0. It walks down as 3.2, the first row setting
`+0x40` (the highest id), each row `+0x44`; an 18th match sets the next cursor and has-next. has-previous = the old page had rows.
**1 previous** (only with has-previous, `SearchMarketPrev` 0x43a340) is the mirror: it starts at the previous cursor (or the next
higher id still an auction), sets the next cursor = `+0x40`, walks **up** (rows in rising order; the client inserts them reversed), the
first row setting `+0x44`, each `+0x40`; an 18th match sets the previous cursor and has-previous; has-next = the old page had rows.
**0 refresh**: without a previous cursor it is a new first page (3.2); otherwise (`SearchMarketRefresh` 0x43aa30, which needs
`+0x40` and `+0x44`) it sends the auctions from the page's lowest id (`+0x44`, or the next higher one) **up to and including the
previous cursor**, no 17 limit, flags and cursors unchanged. All three answer **`0x1C03`** with the `0x1C01` layout. Whether the client
ever sends 0 is not known.

### 3.4 `0x1C02` same item (`SearchFirstItemID` 0x43ae70)

`u8 type, u8 type id`: mode = `type × 1000 + type id`, cursors and flags 0, then 3.2 (answer `0x1C02`).

## 4. The lists

Confirmed.

All four are rebuilt from memory on each request and sent in **chunks**: every full chunk as its own packet, then one packet with the
rest (**always**, with n = 0 when nothing is left), so a client may get several.

| request | function | rows | chunk |
|---|---|---|---|
| `0x1C04` my auctions | `GetSellList` 0x43b0f0 | the character's registered ids still auctions: `TP_MARKET_SELLITEM` (the 48-byte row + deposit) | 36 |
| `0x1C05` my bids | `GetTenderList` 0x43b2d0 | the ids it is top bidder of, still auctions: `TP_MARKET_ITEM` | 40 |
| `0x1C06` items to collect | `GetEndItemList` 0x43b470 | its item rows with **deadline ≥ now**: `TP_MARKET_ENDITEM` (id, result type, deadline, item) | 46 |
| `0x1C07` gold to collect | `GetEndMoneyList` 0x43b5f0 | its gold rows with **deadline ≥ now**: `TP_MARKET_ENDMONEY` (money id, market id, result type, return money, money, deadline, item) | 36 |
| `0x1C14` interest list | `GetConcernList` 0x43ce70 | the listed ids still auctions, in list order: `TP_MARKET_ITEM` | (10 at most) |

Rows past their deadline stay in memory but are never listed again (and an item row can't be collected, 9.1).

## 5. Registering, `0x1C08`

Confirmed, 0x477857.

`TPC_MARKET_ITEM_REGISTER`: `u8 bag, u8 slot, u8 count, u8 period, u32 min, u32 buy-now`. A period above 3 is made 3.

### 5.1 Checks

**No answer** when: bag 0; not `CUser::IsBag(bag, slot)` (bag ≤ the number of bags, slot < 24); no item there; `count` above the item's
count; `count` 0. **`0x1C08` result 1** (3 bytes; client 7117) when: **min < 500**; **min ≥ buy-now**; **buy-now > 4,000,000,000**;
**min > 3,800,000,000**; the fee (5.2) above the gold; the item's `bySpecial` is 63 (none shipped); its real type is **27** (quest
items); its **`byOG`** (`ReqOg`) is not 0 (1,013 items: bound items can't be auctioned).

### 5.2 The fee (the deposit)

**`fee = hours × ceil(min ÷ 500,000) × 5`** with hours 7, 24, 72 or 168: so 35 to 840 gold for any minimum up to 500,000, then that much
more per started 500,000. Taken from the gold now; it becomes the auction's deposit, **paid back only on a sale** (10, 7) and lost on a
cancel or an unsold end.

### 5.3 What happens

1. The trade ends (`CancleActionExc`), the character's personal shop is closed (`stMyShop::Ended`) and the item leaves a trade or stakes
   window (`ExchangeItemRemove`).
2. End date = packed `localtime(time() + hours × 3600)`; end tick from it. The auction gets a new market id, the seller, the prices,
   period, fee, **current price = min**, no bidder, the seller's name.
3. **The whole stack** (`count` = the item's count): the item object itself goes to the auction (unique id kept) and the bag slot is
   emptied. **Part of a stack**: a **new** item (new unique id, `CItem::Init(type, type id, count, made now, the old make type)`: the
   definition's quality, no gems; then `InitCraftExpansion`, `docs/original/item-origins.md` sections 1 and 2) goes to the auction and
   the stack loses `count`. Either way the agent gets `0x0702` (bag, slot, count gone).
4. Gold −= fee (agent: `SendDBMoney`); `RegisterItem` 0x43b790 adds the id to the seller's list and sends the agent `0x0F06`
   (`TP_DB_MARKET_ITEM_REGISTER`: the auction and the item).
5. Answer **`0x1C08`** result 0 (62 bytes): `u8 0, u8 bag, u8 slot, u8 count, u32 gold after, TP_MARKET_SELLITEM` (the new auction:
   id, current price = min, buy-now, end date, the item with **count = `count`**, deposit). The client takes `count` off that slot.

## 6. Cancelling, `0x1C09`

Confirmed, `UnRegisterItem` 0x43b900; `u32 market id`.

Not an auction, or not the requester's → **`0x1C09` result 1** (3 bytes; 7120). Otherwise, with deadline = now + 14 days:

1. The id leaves the seller's list; an **item row** (result type **1**) for the seller; answer **`0x1C09` result 0** (44 bytes) with
   that `TP_MARKET_ENDITEM`. **The deposit is not returned.**
2. With a top bidder: the id leaves its list; a **gold row** for it (result type 1, money 0, deposit 0, **return = its bid**); if it is
   online, **`0x1C12`** (55: that `TP_MARKET_ENDMONEY`).
3. Agent `0x0F07` (`TP_DB_MARKET_ITEM_UNREGISTER`: market id, deadline, the bidder's money id or 0). The auction is freed.

Allowed at any time before the end, bids or not.

## 7. Buying now, `0x1C0A`

Confirmed, `DirectBuy` 0x43bd70; `u32 market id`.

In this order: not an auction → **1** (7107 "closed"); gold below the buy-now price → **3** (7102); the requester is the seller → **2**
(7108). Errors are answered `0x1C0A` with the result (3 bytes). Otherwise, deadline = now + 14 days:

1. Gold −= buy-now price. An **item row** (result type 0) for the buyer; answer **`0x1C0A` result 0** (48: `u8 0, u32 gold after,
   TP_MARKET_ENDITEM`).
2. The seller: `fee2 = trunc(price × 0.02)`, **return = deposit − fee2 + price**; a **gold row** (result type 0, money = price,
   deposit, return); if online, **`0x1C0E`** (55, `TP_MARKET_INFO_SELL_OK`: that `TP_MARKET_ENDMONEY`).
3. A previous top bidder: refund row (result type 1, return = its bid) and `0x1C12` if online, as 6.2. **The top bidder may buy now
   itself**: it pays the full price and gets its bid back as a gold row.
4. Agent `0x0F08` (market id, buyer, the seller's money id, the bidder's money id or 0, deadline). The auction is freed.

`trunc(x × 0.02)`: x87 multiply by the double `0x3F947AE147AE147B` (= Rust's `0.02_f64`) at the 53-bit precision MSVC sets, then
`fistp` with the control word set to chop (0x43bf0a `or ah,0xc`).

## 8. Bidding, `0x1C0B`

Confirmed, `Tender` 0x43c4c0; `u32 market id, u32 price`.

In this order: not an auction → **1**; the requester is the seller → **2**; it is already the top bidder → **3** (7109); gold below the
price → **4** (7102): these are answered **`0x1C0B`** `u32 market id, u8 result` (7 bytes). Then the **next minimum** `m =
trunc(current price × 1.05)` (`_ftol2` of the double `0x3FF0CCCCCCCCCCCD`, Rust's `1.05_f64`): price below `m` → **5**, answered
`u32 market id, u8 5, u32 m` (11 bytes; the client shows 7104 and puts `m` in the row). Price **≥ the buy-now price → 6, no answer at
all**. So even the first bid must reach `trunc(min × 1.05)`.

Otherwise:

1. Gold −= price. A previous top bidder gets its refund row (result type 1, return = its bid) and `0x1C12` if online (6.2).
2. The auction's top bidder, its name and the current price become the requester's; the id goes on the requester's list.
3. The seller, if online: **`0x1C10`** (13: market id, price, type, type id, count).
4. Answer **`0x1C0B`** result 0 (59): `u32 market id, u8 0, u32 gold after, TP_MARKET_ITEM` (current price = the bid, the auction's
   **end date**).
5. Agent `0x0F09` (`TP_DB_MARKET_TENDER`: market id, bidder, price, the refund's money id, deadline, bidder's name).

A bid does not extend the auction.

## 9. Collecting

Confirmed.

### 9.1 An item, `0x1C0C` (`CharGetItem` 0x43cc00, `CUser::MarketItemGet` 0x45e9c0; `u32 market id`)

The character's item row with that market id; **its deadline before now → refused**; the item goes into the **first empty place of bags
1 to the number of bags, slot 0 to 23 in order**; no place → refused. Refused → `0x1C0C` result 1 (3 bytes; 7122). Done: the agent gets
`0x0701` (the item in that place) and `0x0F0A` (market id, character); the row is freed; answer **`0x1C0C` result 0** (41,
`TP_MARKET_GET_ITEM`: market id, bag, slot, type, type id, count, quality, gems, craft name). The item keeps everything it had (unique
id included). No weight or other check.

### 9.2 Gold, `0x1C0D` (`CharGetMoney` 0x43cce0; `u32 money id`)

The character's gold row with that **money id**; refused when `gold + return` would pass `u32` (`return ≤ 0xFFFFFFFF − gold`); **no
deadline check**. Refused → result 1 (3 bytes; 7124). Done: gold += return; `SendDBMoney`; answer **`0x1C0D` result 0** (11: `u8 0,
u32 money id, u32 gold after`); agent `0x0F0B` (money id, character); the row is freed.

So an unlisted gold row past its deadline can still be collected by a client that remembers its id (one still showing it from before the
deadline passed); an item row can't.

## 10. The end of an auction

Confirmed, `CMarketMgr::CheckMarketItemEndDate` 0x439240.

`CWorldThread::Update` 0x407390, for the zones of its thread, **every 60 s** (`+0xac`, next = now + 60,000 ms): a zone of map 42 with the
market open calls the check with `GetTickCount()`; it walks **all** auctions (from the lowest id) and ends every one whose end tick has
passed. So an auction ends up to a minute after its end date. For each, with deadline = now + 14 days:

- **No bid**: the id leaves the seller's list; an **item row** (result type 1) for the seller; seller online → **`0x1C0F`** (43,
  `TP_MARKET_INFO_SELL_FAIL`: `TP_MARKET_ENDITEM`). **The deposit is lost.**
- **A bid**: the id leaves both lists; an **item row** (result type 0) for the top bidder, which online gets **`0x1C11`** (43,
  `INFO_TENDER_OK`); a **gold row** for the seller (result type 0, money = the bid, **return = deposit − trunc(bid × 0.02) + bid**),
  which online gets **`0x1C0E`** (55, `INFO_SELL_OK`).
- Agent `0x0F0C` (`TP_DB_MARKET_EXPIRE`: market id, deadline, the seller's money id or 0); the auction is freed and its `+0x130` entry
  set to 0.

"Online" is `CWorld` finding the character (`FindUser`, any map); the notice is sent to that connection.

## 11. The interest list

Confirmed.

- **`0x1C15` add** (`AddConcertMarket` 0x43cfd0; `u32 market id`): not an auction → **1** (7126 "sold"). The list is made if the
  character has none; ids in it that are no longer auctions are **removed now**; the id already in it → **2** (7127); 10 already → **3**
  (7128). Errors: `0x1C15` with the result (3 bytes). Otherwise answer **`0x1C15` result 0** (51: `u8 0, TP_MARKET_ITEM`), the id is
  appended, and the list is written back (2.6).
- **`0x1C16` remove** (`u32 market id`): with a list, the id is removed (if there) and the list written back; answer `0x1C16` `u32 market
  id, u8 r` (7) with **r = 0 when the character has a list** (even if the id was not in it), 1 when it never had one.
- **`0x1C17` clear**: likewise, `0x1C17` `u8 r` (3).
- **`0x1C14`**: section 4.

## 12. The GM commands

Confirmed, `CUser::PacketAdminCmdE` 0x474967, 0x474a1b.

Both need the GM on map 42 (else `F501` with the opcode).

### 12.1 `FA19` `/auctionrecall`: `char name[21], u32 market id` (`CMarketMgr::RetrievalItem` 0x43d210)

The name equal to the GM's own → `F501 FA19`. **A seller of that name who is online is disconnected** (`SConnection::Close(9)`). Then:
**market id 0** = every auction whose seller's name equals `name`; otherwise that auction if its seller's name equals `name`. For each:
the id leaves the seller's list; an **item row with result type 2 for the GM** (not the seller); a top bidder gets its refund row and
`0x1C12` (6.2); agent `0x0F0D` (`TP_DB_MARKET_RETRIEVAL`: market id, deadline, the refund's money id, the GM's character id); the auction
is freed. The GM gets **`0x1C06`** packets (the 4 layout, chunks of 46) with exactly the recalled rows, then `F501 0`. Nothing recalled
→ `F501 FA19` (for "all", no auction of that seller). The seller is not told and **its deposit is lost**.

### 12.2 `FA1A` `/auctionsearch`: `char name[21]` (`SearchCharName` 0x43af50)

A search in mode −1 (3.1) for that seller's name: the GM gets the first page as **`0x1C02`** and can page with `0x1C03`; then `F501 0`.

## 13. What the original stores (facts only)

The database agent loads at start (`CClientToDBAgent::PacketMarket` 0x4091b0): `0x0F01` auctions (`TP_DB_MARKET_LIST`, 14 per packet;
the end tick from the end date, 0 if past), `0x0F02` the highest money id, `0x0F03` item rows (`TP_DB_CHAR_RESULTITEM_LIST`), `0x0F04`
gold rows (`TP_DB_CHAR_RESULTMONY_LIST`), `0x0F05` the highest market id, then `0x020F` opens the market. While running the game server
sends `0x0F06` register, `0x0F07` cancel, `0x0F08` buy now, `0x0F09` bid, `0x0F0A` item collected, `0x0F0B` gold collected, `0x0F0C`
end, `0x0F0D` GM recall, plus the ordinary `0x0701`/`0x0702` item and money messages and `0x061A` for the interest list. The
`PS_GameData` tables are `Market` (an auction, with `Del`), `MarketItems` (its item), `MarketCharResultItems`, `MarketCharResultMoney`
(each with a `_DelLog`), `MarketCharConcern` (per character); procedures `usp_Market_*` and `usp_*_ConcernMarket_*`. Whether the
procedures ever delete rows past their deadline was not read (the bodies are in the restored database, `docs/original/database-notes.md`).

Each step changes memory first and tells the agent afterwards; a crash between the two loses or repeats the step.

## 14. Original oddities

1. The 14-day deadline is enforced for items (9.1) but not for gold (9.2), and expired rows stay in memory forever.
2. A bid at or above the buy-now price gets no answer (8, result 6); the client has no message for it.
3. The first bid must be 5 % above the minimum price; the minimum itself is never accepted.
4. The top bidder may buy now (7): it pays the full price and gets its bid back to collect.
5. A cancel or an unsold end keeps the deposit; a sale returns it.
6. The name search is case-sensitive (`strstr`).
7. `/auctionrecall` disconnects the seller and gives the item to the GM, not back to the seller.
8. `0x1C16`/`0x1C17` report success whenever the character has a list, whether or not anything changed.
9. The end tick is `GetTickCount()`-based (wraps after 49.7 days of uptime: auctions then end late or at once).

## 15. The new server

- **Data** (migration **0025**): `market_auctions` (one row per auction: id, server, seller and name, end time, prices, period, deposit,
  top bidder and name, current price, and the item's columns as `character_items` has them, unique id kept), `market_result_items`
  (item rows: market id, character, result type, deadline, the item's columns), `market_result_money` (gold rows: money id, market id,
  character, result type, money, deposit, return, deadline, the item's display columns) and `market_interest` (character, position,
  market id). Market and money ids come from two sequences (in place of the agent's highest ids). Times are `timestamptz`, packed to the
  server's local time when sent (as `0x0114`).
- **At start** the game server loads its server's auctions and the rows of its characters into a shared `Market` (as the original keeps
  them in memory) before it accepts players, so `0x1C13` is never needed and never sent.
- **Every change is one database transaction** (gold, bag places, auction and rows together) made before the answer is sent and before
  memory changes; changes run one at a time (a turn lock, as the guild bank's). A crash can't lose or repeat half a step.
- **Map 42 only**, as the original (`AUCTION_MAP`). The search state lives in the connection and is dropped when it leaves map 42.
- **The end pass** runs every 60 s on a timer task (not tied to a map's zone); end times are wall-clock times, not ticks (oddity 9).
- **Rows past their deadline** are removed by the same pass (deviation 1).
- Notices (`0x1C0E`, `0x1C0F`, `0x1C10`, `0x1C11`, `0x1C12`) go to the character wherever it is on this server.
- Arithmetic as the original: fee, `trunc(price × 0.02)` and `trunc(current × 1.05)` in `f64` with the same constants, `u32` limits.
- The interest list is loaded with the character and saved on each change, like the original.
- `FA19` and `FA1A` (`docs/original/gm-commands.md` 8.8) as section 12, with deviation 3 (the deposit back) for `FA19`.
- Game log records: written (`game_log::market_register`, `market_buy` and the others; [The game log](game-log.md)).

Built as above (`crates/shaiya-proto/src/market.rs`, `crates/game-server/src/market.rs` for the rules, `server/market.rs` for the
connection and the end pass, migration `0025_market.sql`), with these small choices:

- **The name filter** reads the item names from `defs.items.itemname`, loaded with the market at start (the item definitions the server
  keeps have no names).
- **The store write** is one `MarketWrite` per change (`MarketStore::market`): the gold, the bag place taken, the item row placed and a
  list of ops. Ending an auction is one op that moves its item, every column and the uid kept, into the item row of whoever gets it and
  deletes the auction (cancel, buy now, the end, the recall all end that way); the money ids come back in op order. A part of a stack
  registered becomes a new row (a new uid, made now, the bag row's make type).
- **`0x1C03` 0 (refresh) without a previous cursor** is a new first page, so it answers `0x1C01` (or `0x1C02`) as `SearchMarketFirst` does;
  with one, `0x1C03`. The refresh with a cursor but no page (`+0x40` or `+0x44` 0) sends nothing.
- **"Has an interest list"** (the `0x1C16`/`0x1C17` result 1 case) means a list loaded with entries or one made by an add since entering.
- **Notices** use the zones' `send_to_char` (any map, a lingering character included). A notice to the requester itself (the top bidder
  buying now gets `0x1C12` for its own bid) comes after its answer.
- **`/auctionrecall`**: an online seller (found by name, any case) is disconnected through its account with no packet (as `/akick` with
  nothing to send) before the recall; the recall is one write for all its auctions. The seller's deposit row (deviation 3) has money 0,
  deposit and return the deposit.
- **The end pass** takes the turn, ends each auction past its end in its own write (a failed one is logged and tried again a minute
  later), then deletes every row whose deadline is before now in one write and logs what was lost. Rows of a deleted character wait for
  their deadline like any other.
- A registration first ends the trade (which also closes a duel's stakes window) and the personal shop, so the item cannot be in either.

## 16. Deviations (all approved and built)

1. **Approved. The 14-day deadline applies to gold too, and expired rows are deleted.** A gold row past its deadline can't be collected
   (the original pays it if a client still shows it, oddity 1); the end pass deletes item and gold rows past their deadline (logged with
   what was lost) instead of keeping them in memory and in the database for ever. A player sees no difference from the original, which
   also never lists them again. Built (`Market::collect_money`, `Market::purge`).
2. **Approved. The name search ignores ASCII case** ("sword" finds "Long Sword"); the original's `strstr` is case-sensitive (oddity 6).
   Built (`market::matches`).
3. **Approved as changed. `/auctionrecall` returns the seller's deposit.** Everything else is the original's (12.1): the
   item goes to the GM's items to collect (result type 2), an online seller is disconnected, a top bidder is refunded, the GM gets the
   `0x1C06` rows and `F501 0`. Added: for each recalled auction the seller gets a gold row (result type 1, money 0, return = the
   deposit), so only the item is taken. The original keeps the deposit too (oddity 7). Built (`Market::recall`).
4. **A bid or a buy-now past the auction's end is refused as closed** (result 1, as for an auction that is not there; audit R20). The
    original ends auctions in the pass every 60 s and takes bids and buy-nows until the pass runs, up to a minute after the end; here
    the end time itself closes the auction (`Market::buy`, `Market::bid`), so a refused or failed end write ("tried again next pass")
    no longer keeps an auction open for bids.
5. **A refresh of the page lists at most 18 auctions** (audit R22): the path with a cursor takes every matching auction from the page's
    lowest id up, which can hold more than a page once new listings arrived; it is cut at `PAGE + 1` rows so the answer always fits a packet.

## 17. Other episodes

The EP3 builds of `ps_game.exe` (2008-01-17) and `ps_dbAgent.exe` (2008-01-04) and the EP5.4 builds (2011-09-14, `ps_game` modified
after linking) were compared with the EP4.5 builds of this note, read statically like the rest of the note; overview in
[`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

**There is no auction house** (confirmed): no `0x1C01` to `0x1C17` in the dispatch, no `CMarketMgr` or `Market*` strings, no market
group (`0x0F`) and no market procedures in `ps_dbAgent`, and no `Market*` tables in the database. It is an EP4.5 addition.

### EP5.4

- **The same rules** (confirmed): `PacketMarket` (EP5.4 ps_game 0x4862b0, matched by opcode switch table) keeps the map flag, set only
  for map 42 (EP5.4 ps_game 0x42db4b); the "market loaded" gate; the minimum price of 500 (EP5.4 ps_game 0x4863fb); the deposit per
  started 500,000 (0x48642a); the period index clamped to 3; and bids of `trunc(current × 1.05)` (1.05 at EP5.4 ps_game 0x5741c0).
  Every case was compared: search, page, the four lists (36/40/46/36 rows of 52/48/41/53 bytes, EP5.4 ps_game 0x442334 to 0x44298c), cancel, buy now, bid, collecting and the interest list (10 at most, EP5.4 ps_game 0x4445db) give the same results in the same order, and the end pass the same rows and DB messages (confirmed, token diff of the 37 matched `CMarketMgr` functions and `PacketMarket`). Only the game-log records are 13 bytes longer (the EP5.4 log header).
- **Times are 64-bit** (confirmed): the end time is `hours × 3600` on a 64-bit `time` (EP5.4 ps_game 0x4864b1); the packed dates on the
  wire keep their size (inferred).
- The database agent's market group `0x0F06` to `0x0F0D` is unchanged (confirmed). The war server of EP5.4 has no auction house
  ([`gamewar.md`](gamewar.md)). The "remote auction house scroll" (special 61) is in the EP5.4 data, but no code reads that special
  (inferred unused).
