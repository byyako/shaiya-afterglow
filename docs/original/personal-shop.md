# Personal shops

This page covers player-run shops in the EP4.5 game server `ps_game.exe`: setting a shop up, placing items and prices,
opening and closing it, the sign over the seller, visiting, buying, and everything that ends a shop or a visit, with what an open shop
changes elsewhere (parties, buffs, item skills). It was read in the disassembly and the Ghidra decompiler with the PDB names; struct
sizes come from the PDB. The client's side (every `0x23xx` request and answer as `game.exe` reads them, its checks and messages) is on
the client's [personal shop](../client/personal-shop.md) page.

The game log messages (`0x0405`/`0x0406` to the game log server) are on [The game log](game-log.md), the auction house (`CUser::PacketMarket`,
`0x1Cxx`) on [Auction house](auction.md).

## In short

- **Sit, set up, open.** A player sits, opens the shop window, places up to 12 items from the bags with a price for one of each, gives
  the shop a title and opens it. A sign with the title appears over the seller.
- **The items stay in the bags.** The shop only remembers where each item is and its price, and reads the item from the bags at each
  sale. Nothing about a shop is saved.
- **Visitors.** Up to 16 players can look at a shop at once. Players of the other faction can only visit on map 42 (the Auction House
  map), where the seller pays a 5 % commission on each sale.
- **Standing up ends the shop.** So does taking a step, being hit, trading, using or moving an item, or leaving the map.
- **Weak spots in the original:** the server checks almost nothing when an item is placed (the client does that); the price of a
  purchase is a 32-bit product that can wrap ([fixed by the kit](community-fixes.md#myshop-price-wrap)); and a visitor who leaves the map stays stuck in the seller's visitor list, so sixteen of
  them can fill it for everyone.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the disassembly (`objdump`, names from `docs/original/symbols/ps_game.tsv`) and the
Ghidra decompile (REA). Client side: `docs/client/personal-shop.md` (every `0x23xx` request and answer as `game.exe` reads them, the
client's checks and messages). Struct sizes from the PDB (`docs/original/symbols/ps_game.packets.txt`). `CUser` offsets are the code's.

Covered: setting a shop up, placing items and prices, opening and closing it, the sign over the seller, visiting, buying, and everything
that ends a shop or a visit, with what an open shop changes elsewhere (parties, buffs, item skills).
**Not covered:** the game log messages (`0x0405`/`0x0406` to the game log server); the auction house (`CUser::PacketMarket`, `0x1Cxx`),
which is `docs/original/auction.md`.

## 1. The state

Confirmed, `stMyShop` at `CUser` +0x15DC, 352 bytes; `stMyShop::stMyShop` 0x478390, `Reset` 0x4783c0.

| field | offset in `stMyShop` | `CUser` | |
|---|---|---|---|
| owner `m_pUser` | +0x00 | +0x15DC | |
| status `nStatus` | +0x04 | +0x15E0 | **0** none, **1** setting up, **2** open |
| bag `byBag` | +0x08 | +0x15E4 | 12 × `int`; **0 = the shop slot is empty** |
| slot `bySlot` | +0x38 | +0x1614 | 12 × `int` |
| price `dwMoney` | +0x68 | +0x1644 | 12 × `u32`, the price **of one** |
| title length `nNameLen` | +0x98 | +0x1674 | |
| title `szName` | +0x9C | +0x1678 | `char[121]` |
| visitors `nNumUseChar`, `dwUseCharList` | +0x118, +0x11C | +0x16F4, +0x16F8 | at most **16** player ids |
| visiting `dwMyUseShop` | +0x15C | +0x1738 | the id of the seller whose shop **this** player is looking at; 0 none |

A shop slot holds only the bag, the slot and the price: **the item is read from the seller's bags each time** (`stMyShop::GetItem`
0x478420: shop slot below 12 with a bag set → the item object at that bag and slot, maybe none). Nothing of a shop is saved.

`stMyShop::Use` (0x478450), on the seller's shop with the visitor's id: **16 visitors already → 2**; the shop not open (status ≠ 2)
→ 0; the id already in the list → 0; otherwise it is added → 1. `stMyShop::Unuse` (0x4784a0) takes an id out (the last one moves into
its place). `stMyShop::Send` (0x478670) sends a packet to every listed visitor found **in the seller's zone**; `SendStopped`
(0x478720) does the same with **`0x230C`** and sets each one's visiting field to 0. Neither clears the list.

**`stMyShop::Ended`** (0x478520), the end of a shop from outside: only in status 1 or 2: status 0, `SendStopped`, the 12 bags cleared,
the visitor count 0, **`0x2302`** to the seller, **`0x0523 (seller id, 0)`** (7 bytes) to the seller's view (`CZone::SendView`).
`stMyShop::Stopped` (0x4785d0) and `stMyShop::MyUseShopUnuse` (0x4784e0) are never called.

## 2. The packets

Confirmed, `CUser::PacketMyShop` 0x478bd0.

**Every packet of the family first ends a trade** (`CUser::CancleActionExc`, `docs/original/trade.md` section 4, which also closes a duel
stakes window); it does not end the player's own shop. Opcodes `0x2301` to `0x230D` are handled; `0x2308`, `0x230B` and `0x230C` from the
client are ignored, as is anything outside the range (**nothing closes the connection**, unlike trades and duels).

Client → server:

| opcode | size | fields (PDB) |
|---|---|---|
| `0x2301` | 2 | — begin setting up |
| `0x2302` | 2 | — end (the setup window closes) |
| `0x2303` | 124 | `u8 len, char title[121]` (`TPC_MYSHOP_START`) open |
| `0x2304` | 2 | — close (back to setting up) |
| `0x2305` | 9 | `u8 bag, u8 slot, u8 shop slot, u32 price` (`TP_MYSHOP_ADD`) place an item |
| `0x2306` | 3 | `u8 shop slot` (`TP_MYSHOP_REMOVE`) take it off |
| `0x2307` | 7 | `u8 shop slot, u32 price` (`TP_MYSHOP_SETMONEY`) change a price (the client never sends it) |
| `0x2309` | 6 | `u32 seller id` (`TPC_MYSHOP_BUY_START`) visit |
| `0x230A` | 2 | — stop visiting |
| `0x230D` | 4 | `u8 shop slot, u8 count` (`TPC_MYSHOP_BUY_BUYITEM`) buy |

Server → client:

| opcode | size | fields | |
|---|---|---|---|
| `0x2301` | 2 | — (`TP_MYSHOP_BEGIN`) | the setup window opens, empty |
| `0x2302` | 2 | — (`TP_MYSHOP_END`) | the setup window closes |
| `0x2303` | 2 | — (`TP_MYSHOP_START`) | "Individual market has been opened" |
| `0x2304` | 2 | — (`TP_MYSHOP_STOP`) | "Individual market has been closed." |
| `0x2305` | 9 | the request echoed | the item is shown in that shop slot |
| `0x2306` | 3 | the request echoed | the shop slot is emptied |
| `0x2308` | 8 | `u8 shop slot, u8 count left, u32 money` (`TP_MYSHOP_SELLED`) | to the seller: sold; its money is now `money`; count left 0 empties the shop slot **and the bag slot**, otherwise the bag item has that count |
| `0x2309` | 7 | `u8 ok, u32 seller id` (`TP_MYSHOP_BUY_START`) | **the client ignores `ok`** and opens the visit window for any `0x2309` |
| `0x230A` | 2 | — (`TP_MYSHOP_BUY_END`) | the visit window closes |
| `0x230B` | 3 + 37 n | `u8 n`, n × (`u8 shop slot, u32 price, u8 type, type id, count, u16 quality, u8 gems[6], char craft_name[21]`) (`TP_MYSHOP_BUY_LIST`, `MYSHOP_UNIT`) | the shop's items |
| `0x230C` | 2 | — (`TP_MYSHOP_BUY_STOPPED`) | to a visitor: "Seller closed the market.", the window closes |
| `0x230D` | 3 or 43 | `u8 ok`; with 1: `u32 money, u8 shop slot, u8 count left, u8 bag, u8 slot, u8 type, type id, count, u16 quality, u8 gems[6], char craft_name[21]` (`TP_MYSHOP_BUY_BUYITEM`) | to the buyer: **0** "Can't purchase the item.", **2** "Insufficient gold", **3** "…exceeded the maximum amount of gold possible."; 1 bought (the client writes the item into `bag`/`slot` without checking) |
| `0x230E` | 4 | `u8 shop slot, u8 count left` (`TP_MYSHOP_BUY_SLOTSTAT`) | to a visitor: that slot's count (0 empties it) |
| `0x0523` | 7 or len + 9 | `u32 seller id, u8 open, u8 len, char title[]` (`TP_PC_MYSHOPINFO`, 129) | the sign over the seller; `open` 0 (7 bytes) takes it down |

## 3. Setting up, placing items, opening, closing

### 3.1 `0x2301` begin

Confirmed, 0x478c14.

Only while **sitting** (`m_byMotion` +0x1434 = 1, `docs/original/world.md` section 3); otherwise nothing. **No map check** (the client
offers "open market" only in the market maps: section 7, deviation 1).
- Status 0: status 1, `0x2301`.
- Status 1 or 2: status 0, `SendStopped` (every visitor gets `0x230C`), the bags cleared and the visitor count 0, then status 1 and
  `0x2301`: a fresh setup. **The sign is not taken down** when the shop was open (deviation 6).

### 3.2 `0x2302` end

Confirmed, 0x478c98.

Status 1 or 2: status 0, `SendStopped`, the bags cleared, the visitor count 0, `0x2302`. No motion check; **no `0x0523`** (the client
closes an open shop with `0x2304` first, which takes the sign down; deviation 6 for the rest).

### 3.3 `0x2305` place an item

Confirmed, 0x478e2b.

Only in status 1. In order, silently ignored unless: **`CUser::IsBag(bag, slot)`** (0x45b3d0: bag **at most the bag count** `+0x1304` and
slot below 24; **it does not look for an item**); **bag not 0** (not the equipment); shop slot below 12 and empty; **no shop slot holds
this bag and slot already**. Then bag, slot and price are stored and the request is echoed (`0x2305`, 9 bytes).

**Not checked on the server**: that an item is there, its count, whether it may be traded (`reqog`, quest items), the price. The client
checks before sending (`docs/client/personal-shop.md`, "Opening, setting up, closing"): not tradeable 443, the equipment 444, the shop slot used 445, the same
item placed 446, **count × price above 2³² − 1** 468.

### 3.4 `0x2306` take off, `0x2307` price

Confirmed, 0x478ec9, 0x478f1f.

Status 1, shop slot below 12 and filled: `0x2306` empties it and echoes the request; `0x2307` sets its price and answers nothing.

### 3.5 `0x2303` open

Confirmed, 0x478ce5.

Only in status 1. `len` is cut to **120**; the title is terminated at its byte 120 and copied up to its first zero; the length kept is
`len` (not the string's). Status 2; **`0x2303`** to the seller; **`0x0523 (seller id, 1, len, title)`** of **len + 9** bytes (the title
and its zero, one byte to spare) to the seller's view, **the seller included** (`CObject::SendView`). **No item needs to be placed** (the
client asks for one, dialog 0x11) and the title may be empty (the client asks, dialog 0x10).

### 3.6 `0x2304` close

Confirmed, 0x478dad.

Only in status 2: status 1; `0x2304`; `0x0523 (seller id, 0)` (7 bytes) to the view; `SendStopped`; the visitor count 0. The items stay
placed: the seller is setting up again.

### 3.7 The sign for players coming into view

Confirmed, `CZone::SendUserShape` 0x41fd00, from `CUser::PacketGetInfo` `0x0303`.

After the looks a player asks for with `0x0303` (`docs/original/world.md` section 5.5): when the shop shown is open and **the seller's map is
42 or the asker is of the seller's country**, the asker also gets the seller's `0x0523 (id, 1, len, title)` (len + 9 bytes). The
broadcasts of 3.5 and 3.6 go to the whole view, both countries.

## 4. Visiting

### 4.1 `0x2309` visit

Confirmed, 0x478f5e.

1. The visitor's visiting field must be 0; the seller is looked up **in the visitor's zone** (`CZone::FindUser`, the same map);
   **the visitor's map is 42 or both are of the same country** (`+0x12D`).
2. `stMyShop::Use` on the seller's shop: **1** → the visiting field = the seller's id, **`0x2309 (1, seller id)`**, then
   **`0x230B`** (`stMyShop::SendItemList` 0x4787e0: each shop slot 0 to 11 with a bag set and an item there, in slot order, with
   **the item's count now** and the slot's price). **2** (16 visitors) → `0x2309 (2, seller id)`.
3. Anything else → `0x2309 (0, seller id)`.

**No distance check** (the client leaves on its own beyond 11.0, `docs/client/personal-shop.md`, "Visiting and buying"); no check that the visitor
is not the seller (visiting oneself adds oneself to one's own list), that either is alive, or that the visitor has no shop of its own.

### 4.2 `0x230A` stop visiting

Confirmed, 0x479029.

The visiting field set and the seller found in the visitor's zone: the visitor is taken out of the seller's list, its field 0,
`0x230A`. **When the seller is not found, nothing happens and the field stays set**, so that player can visit no other shop.

## 5. Buying, `0x230D`

Confirmed, 0x479078.

Only while visiting, with count ≠ 0. In order, each failure answering **`0x230D` with only the result** (3 bytes):

| check | result |
|---|---|
| the seller found in the buyer's zone; its shop slot holds an item (`GetItem`; **the shop's status is not looked at**); the count asked at most the item's count | 0 |
| price = count × the slot's price, **a 32-bit product (`imul` at 0x4790f0; it wraps past 2³² − 1)**; the buyer's money below it | 2 |
| the seller's money + price would pass 2³² − 1 | 3 |
| the buyer has no empty slot (bags 1 to the bag count, slots 0 to 23, the first found) | 0 |

Then:
1. The buyer's money − price.
2. **On map 42** (the buyer's map, which is the seller's): a **commission** of **price × 0.05, truncated** (the x87 rounding mode is set
   to chop around the `fistp`; `0.05` is the double `0x3FA999999999999A`), so **the seller gets price − commission**; elsewhere the whole
   price. `CUser::ChkAddMoney` (0x478360) adds it, capped at 2³² − 1.
3. **The item**: when the count bought is all of it, the item object moves to the buyer's empty slot (its unique id, gems and craft name
   with it), the seller's bag slot and **the shop slot are emptied**; otherwise the seller's count goes down and **a new item** with the
   count bought is made for the buyer (`CObjectMgr::CreateItem`, `CItem::Init` with make type **'T'**, a new unique id, then
   `CItem::InitCraftExpansion`: as a split in a trade, `docs/original/trade.md` section 3, no gems, the craft name of zeros when
   craftable).
4. Database: the seller's money (`SendDBMoney`), the seller's item changed or removed (`0x0702`), the buyer's money, the buyer's new item
   (`0x0701`): **four separate messages**. Game logs `0x0406` (seller) and `0x0405` (buyer).
5. **`0x230D 1`** to the buyer: its money now, the shop slot, the count left in it (0 when all went), its bag and slot, then type, type id,
   **the count bought**, quality, gems and craft name **of the seller's item** (for a split, the original shows the buyer the seller's
   quality, gems and craft name, not the new item's).
6. **`0x2308 (shop slot, count left, seller's money now)`** to the seller (the client prints 412 "<p> is sold with <b>G.", and on map 42
   5357 "5% of item cost has been paid for commission.").
7. **`0x230E (shop slot, count left)`** to every visitor (`stMyShop::Send`), **the buyer included**.

## 6. What ends a shop or a visit

**`stMyShop::Ended`** (section 1) is called, besides by the shop packets above:
- right after every **`CUser::CancleActionExc`** of `docs/original/trade.md` section 4 except the shop's own: death (`CUser::Death`), moving
  an item to another place, picking up, dropping, using an item (`docs/original/item-use.md`), NPC shop buy and sell (after their checks),
  gems and lapis (`PacketGem`), quests (`PacketQuest`), the auction house (`PacketMarket`);
- on **every trade packet** (`CUser::PacketExchange` 0x46ec90, before its switch);
- **standing up**: `CUser::StatusStand` (0x48a100) on a sitting player — on every step (`0x0501`, even one the server then refuses),
  when hit by a player's blow or skill (`CUser::AttackUser`, `CUser::SkillOnce`) or a monster's (`CMob::Attack`, `CMob::AttackUser`) —
  and `CUser::ChangeMotion` (0x48a1a0) with motion 0 when sitting (`0x0506`);
- **a move carried out** (`CUser::UpdateResetPosition` 0x48d470: gatekeepers, recall runes, the movement rune, the summon stone, saved
  positions), before the move;
- **leaving the zone** (`CUser::LeaveZone` 0x44aee0: map change, logout, lost connection).

**Leaving the zone with a visit** (0x44aefa): when the visiting field is set and the seller is found in the zone, the field is set to 0
and **`stMyShop::Unuse` is called on the leaver's own shop with the leaver's own id**: the seller's list keeps the leaver. An original bug:
the seller's later `Send`/`SendStopped` skip the id while it is elsewhere (they look it up in the seller's zone) but reach it again when it
comes back, and **`Use` refuses it ("already in the list") until the shop is set up again**; sixteen such leavers fill the list for
everyone. (A visitor dying, standing up or trading keeps its visit.)

**What an open or set-up shop (status ≠ 0) changes elsewhere**:
- **Parties** (`CUser::PacketParty`): "busy" in `docs/original/party.md` sections 2 and 3: an inviter or target with a shop gets result **1**.
- **Buffs** (`CUser::BuffUser` 0x450c50): the target is not given `wType` 88 (hiding) or 90 (disguise) while its shop is set up or open
  (`docs/original/skills-buffs.md` section 4).
- **Item skills on a player** (`0x0559` kind 1, `CUser::ItemUse` 0x465923): refused when **the target** has a shop (the field is
  `+0x15E0`; `docs/original/item-use.md` called it a trade).

## 7. The new server

Built as above, with these deviations (all approved):

1. **The market maps**: `0x2301` is refused (nothing sent) outside the maps of `market_maps` (comma-separated, for example **35,
   36, 42**: the two capitals and the Auction House, where the client offers "open market"; empty allows every map, as the original, and
   is the default; the limit is an addition: `docs/server/deviations.md` section 1). The setting makes the rule `setting.market_maps`
   (a `deny` of `personal_shop` outside those maps, silent); a `[rules]` entry can close or open maps with more scope, and tells the
   player by a whisper (`docs/server/rules.md`). The
   client's own map gate is gone with the Market Anywhere patch (`docs/client/README.md`).
2. **Placing an item is checked as a trade offer is** (`docs/original/trade.md`, deviation 6): an item there, not a quest item (real type
   27) nor type 99, not bound (`reqog` not 0); and **count × price must fit in 32 bits** (what the client checks, 468). A `0x2307` price
   must fit too. The original lists an empty bag slot and leaves the rest to the client.
3. **The price of a purchase does not wrap**: count × price is computed in 64 bits and a total above 2³² − 1 is refused with `0x230D 0`.
   The original's 32-bit product lets a modified client list 2³¹ a piece and sell two for 0 (or any amount it likes); with deviation 2 it
   cannot be reached here at all.
4. **A visitor leaving the map or the world is taken out of the seller's list** (the original takes it out of its own; section 6). A
   seller leaving stops its visitors as the original does, so a visit never outlives its shop; `0x230A` with no shop found clears the
   visit and answers `0x230A` anyway.
5. **Visiting oneself is refused**, and **a refused visit gets no answer** (the original's `0x2309 0` or `2` opens an empty visit window
   on the client, which ignores `ok`).
6. **The sign comes down whenever an open shop ends** (`0x2301` again while open, `0x2302` while open): `0x0523 (id, 0)` to the view, as
   `Ended` does. The original leaves it up in those two cases.
7. **One database transaction per purchase**: both characters' money and the item, all or nothing (`store.exchange`, as trades). The
   seller's connection checks the item in its bags against what was listed (type, type id, enough of it) before it writes. **Part of a
   stack keeps the seller's quality, gems and craft name** with make type 'T' (trade deviation 3; the original's new item has none but
   shows the buyer the seller's).
8. The checks of a purchase run the buyer's first (the listing and the count, its money, an empty slot) and the seller's after (the item
   still in its bags, the gold limit). Only a purchase failing both an empty-slot and a gold-limit check answers differently (0, not 3).
9. **A purchase is taken under one lock, once** per seller: purchases of one shop are carried out one after the other by the seller's
   connection, each seeing the counts the last one left (the original serialises them with its single zone thread).

How it is built: the shop lives with the seller in the zone (`Placed`), which holds what each shop slot shows (bag, slot, price and the
item as it was when placed); the visit lives with the visitor. Every item action of the seller ends its shop (section 6), so what a shop
slot shows stays what is in the bag, except for the sales themselves, which update it. A purchase: the buyer's connection checks its side
and queues the request with the seller, then waits; the seller's connection (woken by `ZoneEvent::Sale`) checks its side, writes the
purchase, updates its bags, money and the shop slot, and sends `0x230D` to the buyer, `0x2308` to itself and `0x230E` to the visitors,
in the original's order; the buyer's connection then puts the item in its bags and takes the money off. A purchase not taken within 30 s
is withdrawn with `0x230D 0`.

10. **A title is checked like chat** (audit R21): one holding a byte below 0x20, or a colour marker (`<<NNNcCC`) whose length runs past the
    text (the marker that crashes clients, `docs/client/client-packets.md`), is refused as `0x2303` is refused outside a market map: the
    shop does not open and nothing is sent. The original shows any title to the whole view. Whether the sign goes through the chat text
    drawing is asked in `docs/testing/client-research-queue.md` entry 13 (b).

Game logs are not sent (none are built).
