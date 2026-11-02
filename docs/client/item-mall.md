# Item mall (`26xx`, `B1xx`) and points: what the client needs to show prices and buy

This page covers the item mall ("Point shop") of the client `game.exe` (US build 2010-04-26, patch 108): where the product list and
prices come from, the point balance, the purchase and gift flows, the two storages, and the billing packets. It was read from
`game.exe` (handlers on the network object's vtable slots `+0x54c`…`+0x588`) and the packet structs of the EP4.5 server's debug symbols
(`ps_game.pdb`). Status words: **confirmed**, **confirmed (disasm)**, *inferred*, **live**. Sizes include the opcode.

## In short

- **The mall's products and prices are in the client's own data file** `Cash.SData`. The server can rename and reprice them, but not
  add new ones, and the original server never does.
- **Points come only from the server**: the balance at world entry, on refresh, and after each purchase or gift.
- **The client checks nothing before buying.** It asks the player to confirm and sends the product code; the server decides whether
  there are enough points and bag space.
- **Bought items go straight into the bags**; gifts go to a separate mall storage of 240 slots, which the player empties later.

## Summary

- **The product list is client data**: the mall window is filled from `Cash.SData` (85 products: index, tab, icon, price, 24 ×
  (item id, count), name, code, description; see [Skill and Monster SData](skill-monster-sdata-vs-defs.md#the-item-mall-cashsdata-against-productlist)). Entry `+0x88` is the tab (1–4; anything above 4
  goes to tab 4), at most 300 products per tab; a fifth tab (`ins_market_past.tga`) shows the purchase history (*inferred*).
  confirmed (`0x4cd7f0`)
- **`0xB106` can rename and reprice** products already in `Cash.SData` (match by code, case-sensitive); it cannot add products. The
  client asks for it once after a scene set-up (`0xB106` 2, region feature 10). The original never answers. confirmed (`0x55a000`,
  [Packet opportunities](packet-opportunities.md))
- **Points** come only from the server: `0x2601` at world entry, `0x2605` any time, and
  the remaining points in each buy/gift answer. The region-6 mall window does **not** ask for points when it opens (only regions 5
  and 9 send `0x2605` there); `0x2605` is also sent when the game window becomes active again in the world (`0x40977b`, *inferred*:
  after the "charge" web page). confirmed
- **The client does not check points or bag space before buying**: it asks "Do you want to purchase <b>?" (822) and sends the
  product code. The server decides. confirmed (`0x4ce550`)
- "Charge" button: region 6 starts Internet Explorer with `http://www.aeriagames.com/user?destination=https://billing.aeriagames.com/index.asp`
  (CreateProcess). Harmless offline; no packet. confirmed

## Requests (client → server)

| op | size | fields (PDB) | sent when |
|---|---|---|---|
| `0x2602` | 23 | `char szProductCode[21]` (`TPC_POINT_BUY`) | buy: product selected, confirm 822 accepted (`0x4ce810`). **The catalogue's `u32` reading of this packet is wrong: it is the code** |
| `0x2603` | 44 | `char szProductCode[21], char szTargetCharName[21]` (`TPC_POINT_GIFT_SEND`) | gift: 825 "Please enter the recipient's name." → name typed → confirm 826 "Do you want to send <b> to <p>?" (`0x4ce5eb`) |
| `0x2605` | 2 | — | point refresh (see summary) |
| `0x2607` | 3 | `u8 byNpcSlot` (`TPC_POINT_STORED_ITEMGET`) | mall-storage window (`0x4956b4`): take the item in that storage slot (the window picks the target bag slot itself) |
| `0xB102` | 3 | `u8 byNpcSlot` (`TPC_BILLING_ITEMGET`) | billing-storage window (`0x45f8c9`): same for the billing storage |
| `0xB106` | 2 | — | product names/prices request (above) |

## Answers (server → client)

| op | size | fields (PDB) | client does |
|---|---|---|---|
| `0x2601` | 7 + 29n | `u32 dwCashPoint, u8 n`, n × `TP_POINTLOG_UNIT` (`char szProductCode[21], u32 dwBuyTime, u32 dwUsePoint`) (`TP_POINT_AND_LOG`, ≤ 10 rows) | points = `dwCashPoint` (`0x21e558c`); the rows are appended to the purchase history |
| `0x2602` | 3 / 38 + 5n | `u8 bySuccess`; 0: `u32 dwRemainPoint, TP_POINTLOG_UNIT Log, u8 n`, n × `TP_POINTITEM_UNIT` (`u8 bag, u8 slot, u8 type, u8 typeId, u8 count`) (`TP_POINT_BUY`, ≤ 24 items) | **0 = success**: 824 "Purchased <b>." (product name from `Cash.SData` by code), log row added, points = `dwRemainPoint`, each item written into `bag/slot` (quality = the item's maximum from `Item.SData` `+0x2c`, no gems, no craft name). 1 → 239 "No item located on that spot"; 2 → 823 "Insufficient points."; 3 → 238 "Inventory is full"; others silent |
| `0x2603` | 3 / 57 | `u8 bySuccess`; 0: `char szTargetCharName[21], u32 dwRemainPoint, TP_POINTLOG_UNIT Log` (`TP_POINT_GIFT_SEND`) | 0: 827 "Sent <b> to <p>.", log row, points. 1 → 347 "The character doesn't exist."; 3/4 → 828 "Recipient's inventory is full."; 5/6 → 239; 9 → 830 "Previous request in process…"; 10 → 823 "Insufficient points."; others → 829 "Can't send the present." |
| `0x2604` | 45 + 4n | `char szSendCharName[21], char szProductCode[21], u8 n`, n × `POINT_STORED_ITEMUNIT` (`u8 slot, u8 type, u8 typeId, u8 count`) (`TP_POINT_GIFT_RECV`) | gift received: n > 0 → 831 "Received <b> from <p>.", n = 0 → 832 "You got a new present." (and a flag `0x21e5b9d`); each unit is stored in the mall storage at `slot` (< 240) |
| `0x2605` | 6 | `u32 dwCashPoint` (`TP_POINT_RELOAD`) | points |
| `0x2606` | 3 + 4n | `u8 n`, n × (`u8 slot, u8 type, u8 typeId, u8 count`) (`TP_POINT_STORED_ITEMLIST`, ≤ 240) | fills the mall storage (`0x21e5882`, 240 × 3 bytes); slots ≥ 240 ignored |
| `0x2607` | 5 | `u8 byNpcSlot, u8 bag, u8 slot` (`TP_POINT_STORED_ITEMGET`) | moves the stored item to `bag/slot` (quality from `Item.SData`), clears the storage slot. `bag == 0` or slot ≥ 240 → nothing (the only failure form) |
| `0x2609` | 3 + n | `u8 n`, n × `u8 slot` | clears those storage slots. Client-only (the original never sends it) |
| `0xB101` | 3 + 4n | `u8 n`, n × (`u8 slot, type, typeId, count`) (`TP_BILLING_ITEMLIST`, ≤ 240) | fills the billing storage (`0x21e55b2`) |
| `0xB102` | 5 | `u8 byNpcSlot, u8 bag, u8 slot` (`TP_BILLING_ITEMGET`) | as `0x2607`, from the billing storage |
| `0xB103` | 7 | `u8 byServiceType, u32 dwExpireTime` (`TP_BILLING_SERVICETYPE`) | region 6: stores the type; 1 → 814 "You are using individual package. (expire date : <b>)", 2 → 815 (PC room 1.5× exp), 3 → 816, 4 → sysmsg 863, **which this client lacks** ("GetMsg Error 863"); 0 → nothing. `<b>` is the expiry as `YYYY-MM-DD` from the packed date (see [Auction house](auction-house.md#summary)) |
| `0xB104` | 6 | `u32` | "3 hour-free service" — region feature 28, off: no effect |
| `0xB105` | 6 | `u32 dwLogOffTime` (seconds) | prints sysmsg 840 with `mm : ss`; **840 is a lapis-extraction text in this sysmsg**, so the line is wrong on this client. Client-only, never sent by the original: do not send |
| `0xB106` | 3 + 76n | `u8 n`, n × (`char code[21], char name[51], u32 price`) (`TP_BILLING_PRODUCTLIST`, ≤ 128) | for each code already in the mall list: name (≤ 50 characters) and price replaced. Client-only |

Purchase history rows (`TP_POINTLOG_UNIT`) are kept as `code, buyTime, usePoint`; `buyTime` is a packed date (*inferred*, as the
auction dates).

## For a server: a minimal mall

1. At world entry: `0x2601` with the account's points and, if the storage is used, `0x2606`.
2. On `0x2602`: look the code up in the product table (the client's `Cash.SData` codes; the original database's `ProductList` has 84 of the 85),
   check points and free bag slots, put the items in, then `0x2602 0` with the remaining points, a log row and one
   `TP_POINTITEM_UNIT` per item (the client only writes those slots; it does not move or merge anything itself).
3. Optional: answer `0xB106` with the database's names/prices for codes the client has, so price changes need no client patch.
4. Gifts (`0x2603`) put the items in the recipient's mall storage and send them `0x2604`; storage takes are `0x2607`.

## Open

- **Live**: what the fifth tab shows exactly; whether `0x2605` is sent on window activation; the gift flag `0x21e5b9d`'s icon.
- The billing-storage and mall-storage windows: how they are opened (NPC or button) was not traced.
