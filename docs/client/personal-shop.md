# Personal shop (`23xx`): requests, answers, client checks

This page covers the player "market" (personal shop) of the client `game.exe` (US build 2010-04-26, patch 108): every request and
answer, the fields the client reads, result codes with their messages, what the client checks before sending, and the order the
original EP4.5 server `ps_game.exe` uses (`CUser::PacketMyShop` `0x478bd0`, `stMyShop::*`). Struct names come from the server's debug
symbols (`ps_game.pdb`). Status words: **confirmed** (decompiled), **confirmed (disasm)**, [*inferred*](glossary.md#inferred), [**live**](glossary.md#live). Sizes include the
2-byte opcode. Item fields "item" = `u8 type, u8 typeId, u8 count, u16 quality, u8 gem[6], char craftName[21]` unless the order is
given.

## In short

- **A player can open a shop of 12 slots** in a market map, give it a title, and other players can walk up, look and buy.
- **The client shows changes only when the server echoes them**: an item placed or removed appears once the server confirms it.
- **The map rule is the client's**: only the market maps offer "open market", and a common client patch (Market Anywhere) removes that
  check. The original server does not check the map itself, so once the client check is gone only a server check remains.
- **On the Bootleggery map (42)** the seller pays a 5 % commission, and players of both factions can visit each other's shops.

### Using this page

- **If you write a server:** the request and answer tables give the fields and the result codes the client words; the checklist is [What a server must do for this client](afterglow/server-checklist.md#personal-shop).
- **If you patch or mod the client:** the market-map rule is a client check that the Market Anywhere patch removes; see [the patch kit](client-patch-kit.md#market-anywhere-personal-shop-outside-the-market-opt-in-gameplay).
- **To check a claim yourself:** start at `CUser::PacketMyShop` `0x478bd0` on the server side and the store at `0x21e51d8` in the client, using [Tools and method](tools-and-method.md).

## Summary

- A shop has **12 slots** (client store `0x21e51d8`, 12 × 12 bytes; the original's `stMyShop` checks `shopSlot < 12`). confirmed
- The client keeps the shop in step only from the server's echoes: placing (`0x2305`) and removing (`0x2306`) are shown when the
  echo arrives, not when the player drags. confirmed (`0x5591a0`, `0x5591e0`)
- **Map rule**: the client only offers "open market" in the market maps (map 42 "Bootleggery", 35/36 by faction) unless the
  Market Anywhere patch is applied (at `0x441F11`); the original's `0x2301` does not check the map itself (it checks
  `CUser +0x1434 == 1`, *inferred*: the player sits in the shop motion). Once that gate is removed, nothing in the client limits where a shop opens. confirmed (client),
  *inferred* (server field)
- Visiting needs the same faction **or** map 42 (original). confirmed
- The **`0x2309` answer's result byte is ignored by the client**: any `0x2309` opens the visit window (dialog `0x0E`), so a
  `0x2309` sent for a refused visit still opens an empty window. confirmed (`0x5593a0`)
- **`0x230D` answers**: 0, 2 and 3 are the refusals the client words (413, 303, 469); **any other value counts as a purchase**, and the
  client then writes the item into the answer's `bag/slot` **without checking that the slot is free**. The original sends 0 (also for no
  free bag slot), 2 (buyer's gold short) or 3 (seller's gold would overflow) for refusals. confirmed (client), refusal values from the
  original
- **`0x2305` and `0x2306` are shown only when echoed**: the setup window does not change until the echo arrives. confirmed
- **Sign and closing**: the original sends the sign `0x0523` to the players in view on open and close, and `0x230C` to visitors when the
  seller closes, cancels or leaves; the client prints 447 and closes the visit window on `0x230C`. confirmed (client), original as listed below
- Commission: on map 42 the original takes 5 % from the seller (price × 0.05, truncated, **ref** [Personal shops](../original/personal-shop.md#5-buying-0x230d)) and the
  client prints 5357. confirmed (map check both sides)

## Opening, setting up, closing (seller)

| step | client → server | sent from / client checks | server → client | client does |
|---|---|---|---|---|
| open the setup | `0x2301` 2 | player menu "market" (`0x48f67a`, in `0x48c120`): only when the player is in the action state the menu tests and no setup is pending (`0x21e5448 == 0`); then `0x21e5448 = 1` | `0x2301` 2 (`TP_MYSHOP_BEGIN`) | clears the 12 slots and the title (player `+0x2f4`, 128 bytes), setup state `0x21e5448 = 2` (the setup window opens) |
| place an item | `0x2305` 9: `u8 bag, u8 slot, u8 shopSlot, u32 price` (`TP_MYSHOP_ADD`) | setup window `0x4ddf90` after the price dialog. Checks: item not tradeable (Item.SData `+0x24`) → 443 "Not a tradeable item"; from the equipment (bag 0) → 444 "Can't register equipped item."; shop slot used → 445; the same bag/slot already placed → 446; `count × price > 0xFFFFFFFF` → 468 "Exceeded max. gold possessable." | `0x2305` 9 (echo of the request) | stores bag, slot, price in that shop slot |
| remove an item | `0x2306` 3: `u8 shopSlot` | setup window | `0x2306` 3 (echo) | clears that shop slot |
| change a price | — (the client never sends `0x2307`) | — | `0x2307` 7: `u8 shopSlot, u32 price` (`TP_MYSHOP_SETMONEY`) | sets the slot's price (the original never sends it: client-only) |
| open the shop | `0x2303` 3 + len: `u8 len, char title[len]` (len includes the NUL; PDB `TPC_MYSHOP_START` 124 = 121-byte title) | "open" button: empty title → dialog 0x10 (enter a title); no item placed → dialog 0x11; else sends | `0x2303` 2 (`TP_MYSHOP_START`) | "Individual market has been opened" (409), the window switches to selling (`0x21e5454`) |
| close the shop | `0x2304` 2 | "close" button | `0x2304` 2 (`TP_MYSHOP_STOP`) | 410 "Individual market has been closed.", back to setup |
| cancel the setup | `0x2304` then `0x2302` 2 | "cancel" / window closed | `0x2302` 2 (`TP_MYSHOP_END`) | `0x21e544c = 1`: the setup window closes |

Original (`PacketMyShop`): `0x2301` toggles: state 0 → 1 (setup) and answers `0x2301`; already 1 or 2 → resets, `SendStopped` to
visitors, answers `0x2301` again. `0x2305`/`0x2306`/`0x2307` only in setup (state 1). `0x2303` only in setup: caps the title at 120
bytes, state 2, answers `0x2303` and sends **`0x0523`** to the players in view (`u32 sellerId, u8 1, u8 len, title`: the sign over
the seller). `0x2304` only when open: state 1, answers `0x2304`, `0x0523 (id, 0)` to the view, `0x230C` to every visitor.
`0x2302` in state 1 or 2: state 0, `0x230C` to visitors, answers `0x2302`. confirmed

## Visiting and buying

| step | client → server | client checks | server → client | client does |
|---|---|---|---|---|
| visit | `0x2309` 6: `u32 sellerId` (`TPC_MYSHOP_BUY_START`) | player menu "view market" (`0x490a7c`), only with no dialog open | `0x2309` 7: `u8 ok, u32 sellerId` (`TP_MYSHOP_BUY_START`) | **ignores `ok`**: opens dialog 0x0E for `sellerId` and resets the "left" flag (`0x5593a0`) |
| item list | — | — | `0x230B` 3 + 37n: `u8 n`, n × (`u8 shopSlot, u32 price, item`) (`TP_MYSHOP_BUY_LIST`, `MYSHOP_UNIT`) | replaces the visited-shop store (`0x21e5268`, 12 × 40 bytes) |
| buy | `0x230D` 4: `u8 shopSlot, u8 count` (`TPC_MYSHOP_BUY_BUYITEM`) | visit window `0x4ded80`: a stack of 1 is bought as 1; a larger stack asks for the count first; then a confirm box | `0x230D` 43 (`TP_MYSHOP_BUY_BUYITEM`): `u8 ok`; ok = 1: `u32 money, u8 shopSlot, u8 shopCountLeft, u8 bag, u8 slot, u8 type, u8 typeId, u8 count, u16 quality, u8 gem[6], char craftName[21]` | 0 → 413 "Can't purchase the item."; 2 → 303 "Insufficient gold"; 3 → 469 "…exceeded the maximum amount of gold possible."; **any other value is taken as success** (fields are only read for 1): gold = `money`, 415 "Purchased <p> with <b>G." (b = count × price), the shop slot's count = `shopCountLeft` (0 empties it), the item put in `bag/slot` |
| a slot changed | — | — | `0x230E` 4: `u8 shopSlot, u8 count` (`TP_MYSHOP_BUY_SLOTSTAT`) | that slot's count (0 empties it) |
| leave | `0x230A` 2 | "close" button, **or automatically** when the seller is no longer in view or is farther than **11.0** units (`0x6a6794`) | `0x230A` 2 (`TP_MYSHOP_BUY_END`) | `0x21e5460 = 1`: the window closes |
| the seller closed | — | — | `0x230C` 2 (`TP_MYSHOP_BUY_STOPPED`) | 447 "Seller closed the market.", the window closes |

Seller side of a sale: `0x2308` 8 `u8 shopSlot, u8 countLeft, u32 money` (`TP_MYSHOP_SELLED`) → gold = `money`; 412 "<p> is sold
with <b>G." with b = (count before − countLeft) × price, and on map 42 also 5357 "5% of item cost has been paid for commission."
(on map 42 the client adds `trunc(unitPrice × −0.05)` to the amount shown, **confirmed** (`0x559220`@`0x5592b9`: float `−0.05` at `0x6a9b9c`, truncated toward zero by `0x5b6d48`): the term uses the **unit** price once, not the count × price, so for a stack of more than 1 the shown amount is not the 5 % of the total that the server takes); countLeft 0 clears the shop slot **and the bag slot**, else the
bag item's count = countLeft. confirmed (`0x559220`)

Original order for a buy (`0x230D`, confirmed): needs `count != 0`, the buyer visiting, the seller found, the item still in the
slot with `count ≤` its count; else `0x230D 0` (also when the buyer has no free bag slot). Price = count × slot price; buyer's gold
< price → `0x230D 2`; seller's gold would overflow → `0x230D 3`. Success: buyer's gold −= price, the seller gets the price (− 5 % on
map 42, `CUser::ChkAddMoney`), the item (or a split stack) moves into the buyer's first free slot (bags 1…), DB and game-log
records, then **`0x230D 1 …` to the buyer, `0x2308` to the seller, `0x230E` to every visitor**. Visit (`0x2309`): the buyer must not
be visiting already, the seller in the same zone, and (same faction or map 42); `stMyShop::Use` 1 → `0x2309 (1, id)` then `0x230B`;
2 → `0x2309 (2, id)` (*inferred*: shop full of visitors); fail → `0x2309 (0, id)`.

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#personal-shop).

## Open

- **Live**: the sign `0x0523` text placement; what `stMyShop::Use` result 2 means; the shop motion the original's `+0x1434` tests.
