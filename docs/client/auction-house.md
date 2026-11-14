# Auction house (`1Cxx`): requests, answers, row layouts, client checks

This page covers the vault auction house of the client `game.exe` (US build 2010-04-26, patch 108): every request and answer, the row
layouts, the date format, the checks the client makes itself, and what each notice does to the client's lists. It was read from
`game.exe` (sub-dispatcher `0x55d490`, list adders `0x45acd0` / `0x45af50` / `0x45b090` / `0x45b1c0` / `0x45b2f0`, date code
`0x454920` / `0x454a30`) and the packet structs of the EP4.5 server's debug symbols (`ps_game.pdb`); the original server's date packing
comes from `CMarketMgr::GetResultEndDate` `0x438ec0` in `ps_game.exe`. Status words: **confirmed**, **confirmed (disasm)**,
[*inferred*](glossary.md#inferred), [**live**](glossary.md#live). Sizes include the [opcode](glossary.md#opcode).

## In short

- **The auction house lets players sell items to the highest bidder.** A seller sets a minimum bid, a buy-now price and a period of 7,
  24, 72 or 168 hours, and pays a commission up front.
- **Buyers search, bid or buy now**, and can keep an interest list.
- **Nothing is handed over directly.** Won items, unsold items and gold from sales all wait in two "to collect" lists until the player
  collects them.
- **The client loads each list once per session** and then keeps it up to date from the server's answers and notices, so every answer
  carries the full row the client will show.

### Using this page

- **If you write a server:** the request table says what the client checks before it sends and the answer table gives each row layout; the checklist is [What a server must do for this client](afterglow/server-checklist.md#auction-house).
- **To check a claim yourself:** start at the sub-dispatcher `0x55d490`; the requests are traced in [Client actions](client-actions.md#auction-house-requests-confirmed), and [Tools and method](tools-and-method.md) says how to open the binary.

## Summary

- Requests go out through the second send routine `0x585520`; answers arrive through the network fallback (`0x551030` → `0x55d490`),
  so `1Cxx` is not in the main dispatch table. confirmed
- **Rows** (PDB names, confirmed against the client's copies):
  - `TP_MARKET_ITEM` **48**: `u32 dwMarketID, u32 dwTenderMoney, u32 dwDirectMoney, u32 dwEndDate, u8 byType, u8 byTypeID,
    u16 wQuality, u8 byGem[6], u8 byCount, char szCraftName[21]` — search pages, bids, interest list.
  - `TP_MARKET_SELLITEM` **52**: `TP_MARKET_ITEM` + `u32 dwGuaranteeMoney` — my registrations.
  - `TP_MARKET_ENDITEM` **41**: `u32 dwMarketID, u8 byResultType, u32 dwEndDate, u8 byType, u8 byTypeID, u16 wQuality, u8 byGem[6],
    u8 byCount, char szCraftName[21]` — items to collect.
  - `TP_MARKET_ENDMONEY` **53**: `u32 dwMoneyID, u32 dwMarketID, u8 byResultType, u32 dwReturnMoney, u32 dwMoney, u32 dwEndDate`,
    then the same item fields as `ENDITEM` — gold to collect.
- The client keeps every row as a 60-byte entry: item fields at `+1…+0x20`, id at `+0x24` (`dwMarketID`, or **`dwMoneyID` for gold
  rows**: that is the id `0x1C0D` sends back), `+0x28` end date, `+0x2c` result type, `+0x30` tender / return money, `+0x34` direct /
  money, `+0x38` guarantee. **`TP_MARKET_ENDMONEY.dwMarketID` is not kept**: a gold row is identified only by its money id. confirmed
- **Dates** are packed local time: `(year − 2000) << 26 | month << 22 | day << 17 | hour << 12 | minute << 6 | second` (the original
  builds it from `localtime(now + 1,209,600 s)` for item-collection deadlines). The client shows the time left as "N min" / "%dH" /
  "%dD" against its clock corrected by the server time (`0x0114`, `0x21e8730`), nothing once it has passed; colour red under 2 h,
  orange from 2 to 12 h, yellow from 13 h (ARGB `0xFFFF0000` / `0xFFFF8000` / `0xFFFFFF00`). The client corrects an end date only by the
  `0x0114` offset, so the date it shows is the server's local time as sent. confirmed
- **A list is loaded once; the client has no refresh request after that.** A row it receives is added to its lists exactly as sent (all item
  fields, and for gold rows the money id). confirmed
- **Staff commands** on this house are `FA19` `/auctionrecall` and `FA1A` `/auctionsearch`
  ([GM opcodes and commands](gm-and-commands.md#faxx-gm-self-observation-bans-guilds-auction-packetadmincmde-level-5)).
- Packet row limits (PDB): search page 17, registrations 36, bids 40, items to collect 46, gold to collect 36, interest list 10.

## Requests (client → server), with the client's own checks

| op | size | fields | sent when / client checks |
|---|---|---|---|
| `0x1C01` | 39 | `u8 classFilter (0 both/neither faction box, 1 first, 2 second), u8 minLv/10, u8 maxLv/10 (0xFF none), u8 category, u8 grade (0xFF any), char name[32]` (`TPC_MARKET_SEARCH`) | search button (`0x459f60` case 6) |
| `0x1C02` | 4 | `u8 type, u8 typeId` (`TPC_MARKET_SAMEITEM`) | "search this item" on a selected result |
| `0x1C03` | 3 | `u8` 1 prev / 2 next (**confirmed (disasm)** plus the button picture, sender `0x459f60`, see [client-actions.md](client-actions.md); `TPC_MARKET_SEARCH_ACTION`) | page buttons |
| `0x1C04` / `0x1C05` | 2 | — | my registrations / my bids tab, **once per session** (flags `0x84cdbc` / `0x84cdbd`); later kept by results and notices |
| `0x1C06` / `0x1C07` | 2 | — | items / gold to collect tab, once per session (`0x84cdbe` / `0x84cdbf`) |
| `0x1C14` | 2 | — | interest list tab, every time |
| `0x1C08` | 14 | `u8 bag, u8 slot, u8 count, u8 period (0–3 = 7/24/72/168 h), u32 minBid, u32 buyNow` (`TPC_MARKET_ITEM_REGISTER`, `byMarketType` = period) | register: item chosen (7100), minBid > 0 (7112) and ≤ 3,800,000,000 (7139), buyNow > 0 (7115) and ≤ 4,000,000,000 (7113), buyNow not below minBid (7105), gold ≥ commission ≈ round(minBid × 2e-6) × hours × 5 (7102; the original server charges hours × ceil(minBid ÷ 500,000) × 5, **ref** [The auction house](../original/auction.md#52-the-fee-the-deposit)), count ≤ the stack |
| `0x1C09` | 6 | `u32 marketId` | cancel my auction, after 7118 (commission lost) |
| `0x1C0A` | 6 | `u32 marketId` | buy now, after 7110 |
| `0x1C0B` | 10 | `u32 marketId, u32 price` | bid |
| `0x1C0C` | 6 | `u32 marketId` | collect an item; "collect all" sends the next one after each success |
| `0x1C0D` | 6 | `u32 moneyId` | collect gold; "collect all" likewise |
| `0x1C15` / `0x1C16` | 6 | `u32 marketId` | add to / remove from the interest list |
| `0x1C17` | 2 | — | clear the interest list, after 7129 |

## Answers (server → client)

| op | size | fields (PDB) | client does |
|---|---|---|---|
| `0x1C01`–`0x1C03` | 5 + 48n | `u8 bPrevCursor, u8 bNextCursor, u8 n`, n × `TP_MARKET_ITEM` (`TP_MARKET_SEARCH`) | opens the window (dialog 0x75) if closed; the flags enable the page buttons; replaces the result list (a `0x1C03` after "previous" is inserted in reverse) |
| `0x1C04` | 3 + 52n | `u8 n`, n × `TP_MARKET_SELLITEM` | my registrations |
| `0x1C05` | 3 + 48n | `u8 n`, n × `TP_MARKET_ITEM` | my bids |
| `0x1C06` | 3 + 41n | `u8 n`, n × `TP_MARKET_ENDITEM` | items to collect |
| `0x1C07` | 3 + 53n | `u8 n`, n × `TP_MARKET_ENDMONEY` | gold to collect |
| `0x1C08` | 3 / 62 | `u8 result`; 0: `u8 bag, u8 slot, u8 count, u32 money, TP_MARKET_SELLITEM` | 0: gold = `money` (after commission), the bag stack loses `count` (all → cleared), the row is added to my registrations if that list was loaded, 7116; else 7117 |
| `0x1C09` | 3 / 44 | `u8 result`; 0: `TP_MARKET_ENDITEM` | 0: 7119, the row moves to items to collect; else 7120 |
| `0x1C0A` | 3 / 48 | `u8 result`; 0: `u32 money, TP_MARKET_ENDITEM` | 0: 7111, gold set, the row goes to items to collect; 1 7107 "closed"; 2 7108 "registered by you"; 3 7102 "not enough gold" |
| `0x1C0B` | 7 / 59 | `u32 marketId, u8 result`; 0: `u32 money, TP_MARKET_ITEM`; 5: `u32 minPrice` | 0: 7106, gold set, row in my bids; 1 7107; 2 7108; 3 7109 "bid already"; 4 7102; 5 7104 and the row's price ← `u32` |
| `0x1C0C` | 3 / 41 | `u8 result`; 0: `u32 marketId, u8 bag, u8 slot, u8 type, u8 typeId, u8 count, u16 quality, u8 gem[6], char craftName[21]` (`TP_MARKET_GET_ITEM`) | 0: the item goes into `bag/slot` (the `0x0205` logic, `0x55a670`), 7121, next "collect all" request; else 7122 |
| `0x1C0D` | 3 / 11 | `u8 result`; 0: `u32 moneyId, u32 money` (`TP_MARKET_GET_MONEY`) | 0: gold = `money`, chat 53 "Gained <b>G" with the difference, 7123; else 7124 |
| `0x1C0E` | 55 | `TP_MARKET_ENDMONEY` (`TP_MARKET_INFO_SELL_OK`) | my item sold: removed from my registrations (by `dwMarketID`), added to gold to collect if loaded, 7130 "<i>item has been sold successfully." |
| `0x1C0F` | 43 | `TP_MARKET_ENDITEM` (`INFO_SELL_FAIL`) | my auction expired: removed from my registrations, added to items to collect, 7131 |
| `0x1C10` | 13 | `u32 marketId, u32 tenderMoney, u8 type, u8 typeId, u8 count` (`INFO_TENDER`; the client reads 11 bytes, the count is unused) | someone bid on my item: my registration's tender price updated, 7132 "<i>item has been bid with <b> golds." |
| `0x1C11` | 43 | `TP_MARKET_ENDITEM` (`INFO_TENDER_OK`) | I won a bid: removed from my bids, added to items to collect, 7133 |
| `0x1C12` | 55 | `TP_MARKET_ENDMONEY` (`INFO_TENDER_FAIL`) | outbid / lost: removed from my bids, the refund added to gold to collect, 7134 |
| `0x1C13` | 3 | `u8 info` (`TP_MARKET_INFO`) | 0 7135 "Vault is preparing for auction open."; 1 7136 "…opened." (type 8) |
| `0x1C14` | 3 + 48n | `u8 n`, n × `TP_MARKET_ITEM` | interest list |
| `0x1C15` | 3 / 51 | `u8 result`; 0: `TP_MARKET_ITEM` | 0 7125 (row added); 1 7126 "sold"; 2 7127 "already registered"; 3 7128 "can't add anymore" |
| `0x1C16` | 7 | `u32 marketId, u8 result` | 0: row removed |
| `0x1C17` | 3 | `u8 result` | 0: list cleared |

The list updates above only touch lists the client has loaded (the once-per-session flags); a notice for an unloaded list only
prints its message. confirmed (`0x55d490`)

## In other episodes

- The server pack labelled EP3 has no auction house; the EP4.5 pack added it. **ref** ([what EP4.5 added](../original/ep3.md); episode summary on [Episode 4.5](episodes/ep4-5.md))

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#auction-house).

## Open

- `byResultType` values (ENDITEM / ENDMONEY) are not read by any display code found; meaning not traced.
- **Live**: the "N min" format string (`0x6a97e8`, not read).
