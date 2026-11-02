# Client actions, result messages, block list, SData

This page answers, from `game.exe` (US build 2010-04-26, patch 108), the questions a server author asks about the client: what it sends
for actions a server may not build yet, what it shows for each result code, whether it filters blocked players itself, what sends the
unidentified opcodes, and how the client's tables compare with a server's data. Sizes include the 2-byte opcode and exclude the length
prefix; messages are `sysmsg-uni.txt` ids. Corrections and the shortest answers are collected in [client facts](CLIENT-FACTS.md); this
page has the evidence.

## In short

- **Most actions are plain requests** with a few bytes (stat points `0x0208`, item use `0x050A`, drop `0x0206`, repair `0x0554`, gems
  `0x0801`/`0x0802`/`0x0805`, shops `0x23xx`, duels `0x2401`, auction `0x1Cxx`); section 1 lists each with where it is sent.
- **Result codes become fixed messages.** Section 2 maps each reply's code to the message id and text the client shows.
- **The client hides blocked players' chat itself** (whisper, trade, guild and "all members" chat) **but not their invitations**.
- **Several opcodes the client never sends** in this region (`0x2704`, `0x2705`, the 8-byte `0x0802`); a server can ignore them.
- **Item and skill data in the client differ from a server's database in places players can see** (cost, level, points checks);
  section 5 says which differences matter.

Status words:
- **confirmed**: read in a Ghidra decompile of this `game.exe` (SHA-256 `5f045bcb…01a6`, 10,778 functions). Sections 0, 1, 3 and 4 and the
  auction house were re-read this way a second time;
- **confirmed (disasm)**: read by disassembly only, not re-read in a decompile; treat as provisional;
- **ref**: read in the 2012 Hex-Rays listing of a later US client and matched to this one by the same message ids and constants;
- *inferred*: indirect evidence; **live**: needs one logged try in game to settle.

Corrections made by the second pass (the first was wrong): the warehouse rune **does** send `0x050A`; `0x2704`/`0x2705` are **never
sent by this (English) client**; `0x0802`'s 8-byte form is **never sent** by it either; `0x1C02` carries an item **type/type id**, not
a bag/slot; the auction result handler shows ~30 messages (not 3); the block list also hides **trade, guild and "all members" chat**,
not only whispers. Details in each section.

## 0. Catalogue gap: a second send function

`client-packets.generated.md` used to list callers of `send(buf, len)` `0x585640` only. There is a second one,
**`0x585520 send3(u16 opcode, const void *payload, u32 len)`** (confirmed): it logs through the same packet dumper, writes
`u16 len+4, u16 opcode, payload`, encrypts (`0x4010a0`) and sends exactly like `0x585640`. Its 22 call sites are all
**auction-house requests** (`0x1C01`–`0x1C17`, section 1): 20 in the AH window (`0x458430`, `0x459b60`, `0x459f60`, `0x45a950`) and 2
inside the AH result handler `0x55d490` itself (the "receive all" / "withdraw all" loops). The catalogue now scans it too
(`client-packets.generated.md`, "Client → server").
No mail (`1Dxx`) request is sent through either function (confirmed: no `0x1D00`–`0x1D2F` immediate used as an opcode anywhere; the
only `0x1Dxx` constant is the fallback dispatcher's family check). Mail is receive-only in this client.

Also: several EP4 senders are **called with an extra trailing argument** (the selected NPC id `DAT_021e5ba8`) that the sender
ignores (`0x0554`, `0x0801`, `0x0802` 8-byte form, `0x0805`; confirmed): the EP4 wire format has no NPC id, so the server must find
the NPC itself (or not check). The client sends these only while an NPC is selected.

## 1. Requests for the unbuilt actions

| Action | Request (wire) | Where it is sent | Notes |
|---|---|---|---|
| Spend stat points | `0x0208` 14: `u16 STR, DEX, REC, INT, WIS, LUC` | status window `0x4f3990` → sender `0x585a20` | confirmed. Sent once with all six pending values, only if any is > 0. The window rows are STR, REC, INT, WIS, DEX, LUC (pending slots `+0x3c..+0x46`); the sender puts them on the wire as slot 0, 4, 1, 2, 3, 5. Names: the reply handler `0x54ef10` adds wire field 3 to the stat the window turns into defence and field 5 to the one it turns into magic resistance (`0x4f3f80`), i.e. REC and WIS; the other four follow the window rows and match the server's `TP_USER_STATUSUP` order (original server, `../analysis-stats.md`). Reply `0x0208` (same 6 × u16, the server echoes the request): the client **adds** each value and **subtracts their sum from the free stat points** (`0x84d6dc`); no message, no result code. |
| Drink a potion / read a scroll | `0x050A` 4: `u8 bag, u8 slot` | inventory/quick slot use `0x4c7d40` → sender `0x5861a0` | confirmed. Every usable item ends in `0x050A`, with three exceptions: type-100 items with Special 41 or 32 send `0x0557` (next row); Special 39 (Teleportation Stone), 44 (Appearance Change), 45 (Sex Change), 50 (Exp Removal Pill) open a window first (dialog modes `0x11`, `0x14`, `0x15`, `0x16`) and the item is used from that window (`0x050A` from the `0x16` confirm, `0x48c120`); Special 35 below. Items with a cast time are *inferred* to be sent later from the action state machine (`0x43c000`, case 5, a `0x050A` call). |
| Use a rune on a target | `0x0557` 8: `u8 bag, u8 slot, u32 targetId` | `0x4c7d40` → sender `0x5894f0` | confirmed. Only type-100 items: Special 41 (Target Resurrection Rune) with a **player** selected (`DAT_0084fa18 == 2`), target = the selected id; nothing is sent otherwise. Special 32 (Movement Rune): target = the chosen party member; with no choice, the party window's selected member (needs region feature 22, on for this client, section 7); refused with 227 when that member is me or in another group of the union. |
| Teleportation Stone (Special 39) | teleport: `0x0557` 8 with `target` = **slot index 1–4**; save: `0x0220` 17: `u8 index 1–4, u16 map, f32 x, y, z` | window (dialog `0x11`) `0x4dc6b0` → `0x5894f0` / `0x589690` | confirmed. Saves the player's own map and position. Answer `0x0220` 18 (`u8 result, u8 index, u16 map, f32 x, y, z`, handler `0x55a0f0`): 1 "Can't use in a dungeon." (441), 2 227, 0 stores the slot (ignored when index > 4). The list `0x0225` (`u8 n, n × 15`, reader `0x581900` with a 60-byte stack buffer, handler `0x55a280`) is stored at `index − 1`; neither handler checks index 0. The slot buttons are enabled only for slot < `0x021e5b58` (confirmed (disasm), `0x4dc704`); the only direct writes found are in the region-feature-32 branch of item use (`0x4c7f6a`), yet **the stone has always worked on this client** (reported working in play), so the count is set by a path not found here (an indirect write, or feature 32 is on for region 6 and the rebuilt region list is wrong). Unresolved; nothing for the server. |
| Appearance / Sex Change (Specials 44, 45) | `0x0226` 8: `u8 bag, u8 slot, u8 a, u8 b, u8 size, u8 sex` | window (dialogs `0x14`/`0x15`) `0x4630c0` → `0x5861e0` | confirmed. `a`, `b` = the preview's two appearance indexes (window `+0x4c`, `+0x48`; server `TPC_BODYCHG` names them hair, face), each cycled within the model table of the race and sex; size cycled 0–4; sex 0/1 by two buttons. Opening needs an item in the slot (239). `0x0228` → 862 (gender-restricted item equipped). |
| Drop an item | `0x0206` 5: `u8 bag, u8 slot, u8 count` | `0x4e6434` (stack: count from the number dialog), `0x4e6486` (single: count 1 after yes/no) | confirmed (disasm) |
| Repair | `0x0554` 4: `u8 bag, u8 slot` | blacksmith repair window `0x4d9294`/`0x4d9343`/`0x4d965a` | confirmed (disasm). Sent only while an NPC is selected; NPC id not on the wire. |
| Open the warehouse | NPC: **nothing**. Warehouse Recall Rune: **`0x050A`** | NPC menu `0x48c120`; rune `0x4c7d40` | confirmed. **NPC** (keeper menu option 2): opens the window locally, dialog mode `0x67` (in maps 51–52 mode `0x72`, a second window class `0x49d430`); walking away from the NPC closes it (`0x43c000`). **Rune** (Item 100/53, Special 35, "access the Warehouse for five minutes"): sends `0x050A` like any item. The success reply `0x050A` (count updated) opens the window (mode `0x67`, title 5299 "Manage warehouse", `0x55ae90`); it stays open when walking. The rune's 5 minutes are a **buff**: Item 100/53 names skill **228 "Warehouse Call"** level 1 (item bytes +32/+33), whose ability type is 39 and duration 300 s. When the server adds it (`0x050D`: `u32, u16 skill 228, u8 level 1`), the client sets its "warehouse anywhere" flag (`0x4c0760`); **while the buff lasts, using a rune again only reopens the window locally, no packet** (refused with 227 on maps 100–200); when the buff ends (`0x050E`) the client closes a rune-opened window (`0x4c0e50`). Contents always come from `0x0711` (`TP_CHAR_BANK`); moves are `0x0204` with the warehouse bag. |
| Link a lapis | `0x0801` 8: `u8 lapisBag, lapisSlot, itemBag, itemSlot, extraBag, extraSlot` | link window `0x493d30` → sender `0x586370` | confirmed. `extra` = an optional Special 36 (Linking Hammer) or 69 item, else 0/0. Sent only while an NPC is selected. Before the window opens, the blacksmith's lapis option sends `0x0555` (section 4). |
| Extract lapis | `0x0802` 4: `u8 bag, u8 slot` | link window `0x493d30` → sender `0x586540` | confirmed. This client **always sends the 4-byte form**: the 8-byte form (`u8 bag, slot, u8 mode, u8 gemPos (0-based), extraBag, extraSlot`, sender `0x586580`, mode 1 = an extraction hammer (Special 58) with a chosen gem, 0 = none) is behind region feature 0, which is off for English clients (section 7). So extraction hammers cannot be used and the server picks the gem. The client first checks for a free inventory slot ("Insufficient inventory for a transaction", 241). |
| Enchant (lapisia) | `0x0805` 9: `u8 lapisianBag, lapisianSlot, materialBag, materialSlot, materialCount, equipBag, equipSlot` | enchant window `0x4d7520` → sender `0x5863e0` | confirmed, names from the PDB `TPC_LAPISIAN_ADD` (9 ✓). Window: equipment first, then the material (count asked when the stack is > 1), then a type-95 Lapisia. Sent only while an NPC is selected and when gold ≥ the window's cost (Japan: fixed 600,000 / 1,300,000). |
| Open a personal shop | `0x2301` 2 → (`0x2305` 9: `u8, u8, u8, u32 price` per item; `0x2306` 3: `u8` remove) → `0x2303`: `u8 len, char title[len]` (len includes NUL) | market button `0x48f67a`; setup window `0x4de13c`/`0x4de2a9` | confirmed (disasm). Close: `0x2304` 2; cancel setup: `0x2302` 2. Visiting: `0x2309` 6 `u32 sellerId` (player menu "view market"), buy `0x230D` 4 `u8 slot, u8 count`, leave `0x230A` 2. Opening is only offered in the market maps unless Market Anywhere is patched (`patches.md`). |
| Auction house | `0x1C01`–`0x1C17` through `send3` | AH window `0x458000`–`0x45e000` | confirmed; table below. |
| Request a duel | `0x2401` 6: `u32 targetId` | player menu `0x490aaf` | confirmed (disasm). Answer `0x2402` 3: `u8` **1 = accept, 0 = refuse** (opposite of party/trade); sent 0 automatically when busy (`0x5503cb`). Give up: `/defeat` (5335) → `0x2406` 2. Stakes use the trade window: `0x2409`–`0x240C`. |
| Mount / dismount | `0x0216` 2 (no payload) | ride key/inventory `0x4d4fb3`, `0x4d500b`, `0x4db39f`, `0x4e5d23`; unequipping slot 13 while riding `0x4e6883` | confirmed (disasm). Uses the "Summon Stone" (item type 42) in equipment slot 13; cast time 3 s or 5 s set locally. Reply `0x0216` 4: `u8 ok, u8 on`: ok 0 → "failed to get on Ride." (5245); on 1 → "got on Ride." (5243); on 0 → "got off Ride." (5244). Riding together: `0x021D` 6 `u32` (player menu request), answer `0x021E` 3 `u8`. |

### Auction-house requests (confirmed)

All go through `send3` (`0x585520`). "id" = the auction id the server gave the row (row `+0x24`).

| op | size | payload | sent when |
|---|---|---|---|
| `0x1C01` | 39 | `u8 classFilter, u8 minLevel×10, u8 maxLevel×10 (0xFF = no maximum), u8 category, u8 grade, char name[32]` | Search (`0x459f60` case 6). classFilter: 0 = both or neither faction checkbox, 1 = only the first, 2 = only the second. grade: the window's choice, 6 → 7 and 7 → 0xFF (any). Name copied with its NUL. |
| `0x1C02` | 4 | `u8 type, u8 typeId` | "search this item" on a selected result row (row bytes 1–2 are the item's Type/TypeID) |
| `0x1C03` | 3 | `u8 dir`: **1 = previous page, 2 = next page** (confirmed: the sender `0x459f60` sends 1 for its action 7 and 2 for its action 8; *inferred*: that these are the previous and next buttons, from the reverse-order insertion of a 1-page reply) | page buttons |
| `0x1C04`, `0x1C05` | 2 | — | my registrations / my bids list (tab "progress"); each **once per session** (flags `0x84cdbc`/`0x84cdbd`), later kept up to date by the results below |
| `0x1C06`, `0x1C07` | 2 | — | items to collect / gold to collect (tab "completion"); once per session (`0x84cdbe`/`0x84cdbf`) |
| `0x1C14` | 2 | — | interest list (tab 3), every time the tab opens |
| `0x1C08` | 14 | `u8 bag, u8 slot, u8 count, u8 period, u32 minBid, u32 buyNow` | register an item. period index 0–3 = **7 h, 24 h, 72 h, 168 h** (table `0x6a98e8`; default 2). Client checks first: an item chosen (7100), minBid > 0 (7112) and ≤ 3,800,000,000 (7139), buyNow > 0 (7115) and ≤ 4,000,000,000 (7113), buyNow not below minBid (7105), gold ≥ the commission (7102), count ≤ the stack. Commission shown = `round(minBid × 0.000002) × hours × 5` (*inferred* rounding). |
| `0x1C0B` | 10 | `u32 id, u32 price` | bid |
| `0x1C0A` | 6 | `u32 id` | buy now, after "Do you want to process buy now option for <i> with <b>golds?" (7110) |
| `0x1C09` | 6 | `u32 id` | cancel my auction, after 7118 (commission is lost) |
| `0x1C0C` | 6 | `u32 id` | collect an item; "collect all" sends the next one after each success |
| `0x1C0D` | 6 | `u32 id` | collect gold; "collect all" likewise |
| `0x1C15` | 6 | `u32 id` | add to the interest list |
| `0x1C16` | 6 | `u32 id` | remove from the interest list |
| `0x1C17` | 2 | — | clear the interest list, after 7129 |

## 2. Result codes → messages

### Friends and block list (`22xx`) — confirmed (disasm)

| op S→C | fields | client |
|---|---|---|
| `0x2202` | `char name[21]` | buddy request from name: dialog mode 0x0B |
| `0x2203` | `u8 result` (to the requester) | 0 "Buddy registration has been rejected." (399); 1 "…accepted." (400); 2 "User is not in the same area." (401); other: nothing |
| `0x2204` | `u32 id, u8 job, char name[]` | adds the friend (marked online) |
| `0x2207` | `u32 id, u8 online` | friend online flag |
| `0x220E` | `u8 ok` | non-zero "Succeeded in blocking." (402); 0 "Failed to block." (403) |
| `0x220C` | `u8 ok` | party search: non-zero 404 "Registered on Party Search", 0 405 "Failed to register…" |

Client → server answer to a buddy request: `0x2203` 3 `u8` **1 = accept, 0 = refuse** (refuse also prints 5175).

### Item use (`0x050A`, 11: `u32 charId, u8 bag, u8 slot, u8 type, u8 typeId, u8 count`) — confirmed

Handler `0x55ae90`, for the local character only (`charId` = mine): **`bag` = 0 means failure**: `ui_error001.wav` and "Cannot
use the item due to its limitation" (227), or "Battle rune can be used only by Blue grade." (488) when the battle-rune flag
`0x21e8998` is set (items 100/177–180). Otherwise the slot's count becomes `count` (0 empties it), the item's effect runs from its
`Item.SData` values, its cooldown group (`+0x25`) starts, and a Special 35 item opens the warehouse (above). Client-side refusals
before sending (no packet): 227 (level above the player's, cooldown running, restricted state for items with no HP/SP/MP
effect), 806 "Can use after closing other active windows.", 5171 "Must empty all inventory slots from III on" (Special 33, Skill
Reset Stone).

### Stat points (`0x0208`, `0x0214`) — confirmed
`0x0208`: no result code and no message (table above). Stat reset item reply **`0x0214`: `u8 ok`, then if ok: `u16 freePoints,
u16 STR, REC, INT, WIS, DEX, LUC`** (handler `0x54fe30`; note the order differs from `0x0208`) → 5239 "stat points initialized."
/ 5240 "already used. Can't use again."; `0x0215` is the skill counterpart: `u8 ok`, then if ok `u16 skillPoints` and an
**optional** `u8` (read only when the packet has it, `0x580740`) → 5241 / 5242.
The free-reset counters (confirmed): `0x021B` (`u8 stat, u8 skill`, `0x550820`) are **"chances left" counts** shown by the NPC type 13
menu (`0x48c120`): option 2 → "<v> chances left. Do you want to initialize your character's stats?" (5172, v = the stat byte) → "do you
want to initialize stat points?" (5169) → `0x0214`; option 3 → 5173 → 5170 → `0x0215`, only with bags III and up empty (else 5171,
`0x4c2590`). The count is only displayed: the request goes out at 0 chances too. A successful `0x0214` lowers the stat count (if not 0);
a successful `0x0215` lowers the skill count only when its optional byte is non-zero (`0x5528a0`), so the 5-byte original answer leaves
"1 chances left" on screen until the next `0x021B`. The GM set-status packet `0xF701` (`u8 attr, u32
value`, `0x554030`) uses 5 = STR, 6 = DEX, 7 = REC, 8 = INT, 9 = LUC, 10 = WIS, 3 = free stat points, 1 = level, 2 = gold.

### Gems (`08xx`) — confirmed (disasm)

| op S→C | result field | messages |
|---|---|---|
| `0x0801` link (18: 12 × u8, u32 money) | `u8` #1 | ≠0 "Succeeded in linking <i> with <r>" (55); 0 "Failed to link…" (56). Then: item (#5 bag, #6 slot) gem position #7 ← lapis #8; lapis stack (#2,#3) count ← #4 (0 = gone); if #9 ≠ 0 slot (#9,#10) ← lapis type 30 id #11 × #12; money ← u32 |
| `0x0802` extract | reader `0x57cb40`: **33 bytes here** (`u8 success, bag, slot, u8[6] × 4, u32 money`); with region feature 0 on, 34 (a `u8` gem position after the slot, as the EP4.5 server's `TP_GEM_REMOVE`). confirmed | handlers 0x558510 / 0x558700: success 57 "Succeeded in extracting lapis from <i>", failure 58 |
| `0x0805` enchant | `u8` #1 | ≠0 "<i> [<v>]level Upgrade Completed." (67); 0 "…Upgrade failed." (68); level read from the craft name (> 50 = armour, minus 50) |
| `0x0806` compose | | failure 5502 "This item is not for reconstruction." |
| `0x0807` remake | `u8` | 0 success; 2 "It includes the Reward Item from the Quest." (5511); 3 "It includes the Item shop's item." (5513); other: failure sound only |
| `0x0808` remake 2 | | failure 5503 "You can't reward with this item." |
| `0x0555` | `u16` | **the "Blessing of Goddess" modifier** used by the client to compute the lapis-link success rate (stored at `0x84d6d6`; rate = base − value/10). Sent by the client from the blacksmith's lapis option (`0x48c120`, NPC menu case 3 option 2, which also sets dialog mode 0x66) before the link window opens; the confirm text is 10011 ("…<v>%% of success…"). |

### Duels (`24xx`) — confirmed (disasm; matches ref)

`0x2401` (10: `u32 from, u32 to`) → "<p> requested battle to <t>." (5325), duel dialog. `0x2402` (7: `u8 result, u32 id`):
0 "<p> rejected the battle." (5326), 1 nothing, 2 "can't request the battle to <p>." (5327), 3 "the battle is canceled as the
opponent didn't respond." (5328; nothing if `id` is me), 4 "impossible to battle as Debuff skill is on <p>." (5334).
`0x2406` (7: `u8 reason, u32 id`, `reason` 1-based): 1 "<p> disconnected." (5313) / 5314 if me; 2 out of area (5314 me / 5336 other);
3 win: "<p> won at the battle." (5310), plus 5315 "<p> admitted the failure." when the loser gave up; 4 "canceled as you've been
attacked" (5329). Each plays `ui_error001.wav` on failure.

### Personal shop (`23xx`) — confirmed (disasm)

`0x2301` (no fields): enter setup mode. `0x2303`: "Individual market has been opened" (409). `0x2304`: "…closed." (410).
`0x2305` (`u8, u8, u8, u32`): item placed in a shop slot. `0x2306` (`u8`): slot cleared. `0x2308` (`u8 slot, u8 left, u32 money`):
an item sold, "<p> is sold with <b>G." (412), plus "5% of item cost has been paid for commission." (5357) on map 42 (the market).
`0x2309` (`u8, u32`): visit dialog (mode 0x0E). `0x230D` buy result `u8`: 0 "Can't purchase the item." (413), 1 success
"Purchased <p> with <b>G." (415, then the item fields follow), 2 "Insufficient gold" (303), 3 469 (seller at max gold).
`0x230E` (`u8 slot, u8 count`): a visited shop slot's count.

### NPC shop (`07xx`)
Already in `client-world-packets.md` (`0x0702`: 0 ok, 1 303, 2 238).

### Auction house (`1Cxx`) — confirmed

Handled by `0x55d490` (the fallback `0x551030` passes every `0x1Cxx`). Window messages (7100–7129) go to the AH window's message
box (`0x454b90`); notices (7130–7136, 53) to chat. Record sizes: search/interest row 48 bytes, my registration 52, my bid 48,
item to collect 41, gold to collect 53 (contents not traced field by field; the client reads Type/TypeID at bytes 1–2 and the
auction id at `+0x24` of its copy).

| op S→C | fields | client |
|---|---|---|
| `0x1C01`, `0x1C02` | `u8 hasPrev, u8 hasNext, u8 n, n × 48` | search results (opens the AH window, dialog 0x75, if closed); the two flags enable the page buttons |
| `0x1C03` | same | a page; inserted in reverse order after a "previous" request |
| `0x1C04` / `0x1C05` / `0x1C06` / `0x1C07` | `u8 n, n × 52 / 48 / 41 / 53` | the four lists (registrations, bids, items to collect, gold to collect) |
| `0x1C08` register | `u8 result`; 0: `u8 bag, u8 slot, u8 count, u32 gold, 52-byte row` | 0 "Registration for your auction is complete." (7116): **gold = the u32** (after commission), the stack loses `count` (all → slot cleared), row added to my registrations; else 7117 |
| `0x1C09` cancel | `u8 result`; 0: 41-byte row | 0 7119 "The Auction for this item is canceled." (row moves to items to collect); else 7120 |
| `0x1C0A` buy now | `u8 result`; 0: `u32 gold, 41-byte row` | 0 7111 (gold set, row to items to collect); 1 7107 "…has been closed."; 2 7108 "…registered by you."; 3 7102 "You don't have enough golds." |
| `0x1C0B` bid | `u32 id, u8 result`; 0: `u32 gold, 48-byte row`; 5: `u32 price` | 0 7106 "Bidding Successful"; 1 7107; 2 7108; 3 7109 "…bid already."; 4 7102; 5 7104 "You can't bid lower price than minimum bidding price." (row price ← u32) |
| `0x1C0C` collect item | `u8 result`; 0: `u32 id, u8 bag, u8 slot, u8 type, u8 typeId, u8 count, u16 quality, u8 gem[6], char craftName[21]` | 0: item added by the `0x0205` logic (`0x55a670`; the fields after `id` are `TP_ITEM_GET`'s), 7121; else 7122 |
| `0x1C0D` collect gold | `u8 result`; 0: `u32 id, u32 gold` | 0: gold set, chat 53 "Gained <b>G" with the difference, 7123; else 7124 |
| `0x1C0E` / `0x1C12` | 53-byte row | notices "<i>item has been sold successfully." (7130) / "<i>item sale has been failed." (7134) |
| `0x1C0F` / `0x1C11` | 41-byte row | "<i>item auction has been canceled due to expiration." (7131) / "<i>item has been sold." (7133) |
| `0x1C10` | 11 bytes: `u32 id, u32 price, u8 type, u8 typeId, …` | "<i>item has been bid with <b> golds." (7132); updates my registration's price |
| `0x1C13` | `u8` | 0 "Vault is preparing for auction open." (7135); 1 "Vault Auction has been opened." (7136) |
| `0x1C14` | `u8 n, n × 48` | interest list |
| `0x1C15` | `u8 result`; 0: 48-byte row | 0 7125 "registered on your interest list"; 1 7126 "This product has been sold."; 2 7127 "already registered"; 3 7128 "can't add … anymore" |
| `0x1C16` | `u32 id, u8 result` | 0: row removed |
| `0x1C17` | `u8 result` | 0: interest list cleared |

Item names in these notices get the `<<NNNcCC` colour marker only when region feature 11 is on (it is, section 7).

## 3. Blocked players: client-side filtering

- **Chat: yes, the client hides blocked names itself** (confirmed). The chat handler `0x554260` (network slot `+0x2cc`) compares
  the sender's name with every entry of the block list (vector at `0x21e48e4`, names at entry `+4`, case-sensitive) and drops the
  line when it matches, unless the packet is a GM one. It is used by **`0x1102` whisper, `0x1103` trade chat ("[Trade]", 355),
  `0x1104` guild chat (370), `0x1108` "all members" chat (481)**; `0xF102`–`0xF104` (GM) pass the GM flag and are never
  dropped. Not filtered: `0x1101` normal, `0x1105` party, `0x1107` shout, `0x1109`, `0x1111` zone (other handlers, or id-based).
  The "refuse whispers" option (`0x701a20`) also drops all incoming whispers.
- **Invitations: no.** The only users of the block-list vector are the block-list window (`0x4e0ba0`, `0x4e1240`, `0x4e2110`),
  the block memo `0x220B` (`0x555980`, through the lookup `0x4c73b0`) and the chat handler (confirmed: every reference to
  `0x21e48e4`/`0x21e48e8` and every call of `0x4c73b0`). None is on the party (`0x55e820`), trade (`0x558b30`), duel
  (`0x550250`), buddy-request (`0x555790`) or guild-invitation path.
- Consequence: **deviation 5 is needed for invitations** (party, union, trade, duel, guild, buddy, ride-together); for the four
  chat types above it is a backstop (the client's filter is by name and depends on the list the server sent).

## 4. What sends the unidentified opcodes

| op | size | UI action | reply |
|---|---|---|---|
| `0x0307` | 6 `u32 id` | player menu "Inspect" (5421), only within 3 m, else "Too far away from an opponent" (233) — `0x490c7b` | `0x0307` `TP_USER_ITEMS` (equipment list). confirmed (disasm) |
| `0x0555` | 2 | blacksmith NPC menu → lapis linking (`0x48c120`, NPC menu case 3 option 2; sets dialog mode 0x66) | `0x0555` `u16` Goddess-blessing rate modifier. confirmed |
| `0x0556` | 2 | chat commands `/return` (5076) / `/town`, and two buttons (`0x4d52a7`, `0x4db66c`, *inferred* the death/return window); prints "Will be moved to the nearest town in 10 seconds…" (251) | — confirmed (disasm) |
| `0x0F01` | 2 | **Priest** NPC menu option 2 (`0x48d0f2`; NPC type 14 per ref) | `0x0F01` `u8 ok, u16, u16, u8, u8, u8`: ok 1 stores four ratings (warrior / leader / righteousness / teamwork — window labels, Korean) and opens dialog mode 0x69; else "Insufficient gold" (303). confirmed (disasm) |
| `0x0F02` | 2 | Priest result window (`SHOW_MESSAGE_PRIEST`) confirm, `0x4d09e2`/`0x4d0b21` | `0x0F02` `u8`: 1 shows the reading; else 303. *Inferred*: a paid character "reading" by the Priest. confirmed (disasm) |
| `0x1201` | `4 + n`: `u16 n, char text[n]` | chat command `/request GM` (5077) + text; prints "Successfully applied" (252) | — (a GM call / petition). confirmed (disasm) |
| `0x2704` | 2 | **only when the region is 4 (Japan)**: system menu first entry (`0x489ff0`) | `0x2704` `u8` (`0x550d20`): 1 opens dialog 0x12, else "cannot connect to server" (10074). confirmed |
| `0x2705` | 2 | **Japan only**: the window dialog 0x12 opens there (`0x513790`) returns 1 (`0x48c120` case 0x12) | — confirmed |

`0x2704`/`0x2705` therefore never come from this client: with region 6 (English, the build default) the system menu's first
entry opens dialog 0x12 (the exit/character-select choice) directly, and that dialog sends nothing of its own (region feature 59,
off here, would skip it). The server can ignore both. *Inferred*: in Japan it was an exit-time window (a survey or notice) that
the server had to allow first.

Others met on the way: `0x0224` 3 `u8` answers a party-member summon ("Do you want to accept <p>'s party member summon?", 818);
`0x021C` 2 is sent once by a key binding (`0x43d349`); `0x020D` 7 `u8 flag, u32 amount` comes from a number dialog (`0x49d261`,
`0x4fd581`; warehouse gold, but the dialog is never created, so it is never sent: `packet-opportunities.md` opportunity 1). confirmed (disasm)

## 5. Client Item.SData / Skill.SData vs the server

**The original server ships no `Item.SData` or `Skill.SData`.** Its pack carries only `NpcQuest.SData` and `KillStatus.SData`;
`ps_game` reads items and skills from its database. The client's copies are byte-identical to the ones in the original server pack's
bundled 108 client. A server that stores items and skills in its own database can compare them with the client's files (see
`item-sdata-vs-defs.md` and `skill-monster-sdata-vs-defs.md`); this section answers, from `game.exe`, what the client does with its own
values, and so which of those differences players see.

Record layouts in memory: the client keeps item and skill records with the fixed fields 8 bytes further on than in the file
(item ReqLevel file +4 → memory `+0xc`; skill ReqLevel file +8 → `+0x10`, job flags +11..+16 → `+0x13..+0x18`, skill points +18 →
`+0x1a`; checked on Pollution Zone and Trap). Not every item field follows that rule (stack count file +80 → memory `+0x59`).

| Difference (server doc) | What the client does (status, address) | In game |
|---|---|---|
| Capes 24/x, 39/x and mount stones 42/x cost less on the server than the client shows | the NPC shop sends `0x0702` **without any money check**: single items at once, stacks after a count dialog whose maximum is the client's stack size (confirmed, `0x4f7b80`, `0x4f8220`) | the shop shows the client's price, the buy works with the server's price; the gold shown afterwards is the server's (`0x0702` answer) |
| Level requirements: rings/amulets 22/154–156, 23/153–155 (client 31/44/51, server 22), Echidna, Zion Cross; mount stones 42/1–4 (client 40, server 25) | equipping (drag or right-click, any equipment slot including the mount slot 13) goes through `0x4c1590`: **refused below the client's ReqLevel** ("Unavailable level", 311), also faction (246), mode (247), class (248), gender items 64/65 (5423) (confirmed) | players between the server's and the client's level **cannot equip** those items, although the server would allow it |
| Item use with a lower client level (44/89–91 client 0, 44/99 client 1) | item use refuses only above the client's ReqLevel (227, `0x4c7d40`, confirmed) | the client sends; the server answers the `0x050A` failure (bag 0 → 227) |
| Stack sizes (gold bars 50/10 vs 100; group 100 1 vs 255) | inventory moves send `0x0204` for any other slot with **no stack check** (confirmed, `0x4e6250`); the counts shown are the server's answers | the server's limits decide; only the shop's count dialog is capped by the client's stack |
| Trap 113 (client: Ranger only; server also Archer) | the skill window's "next level" arrow needs the job flag of the learnt level's record (`+0x13..+0x18` by job `0x0084fa22`), the level (`+0x10`) and the points (`+0x1a`) of the next level (confirmed, `0x4f2700`, drawn by `0x4f1630`). The window lists only the entries of the client's skill list `0x021e47dc`, filled from the server's `0x0108`/learn answers (`0x56c1d0`); how not-yet-learnt (grey, number `0xFF`) entries get there was not traced | an archer who has Trap cannot raise it to Lv2/3 with this client; whether Lv1 is offered: **live** |
| Pollution Zone 158 (client 4/4/5 points, server 3/3/4) | same check: the arrow stays off while points < the client's cost (confirmed) | a Fighter with exactly 3 points cannot learn it in this client |
| Gruesome Attack 94, Enhance Wind Spin 156 (client allows fewer weapon types) | no weapon-type check was found in the skill send paths read (skill window `0x4f12c0`, quick slots `0x4d4e10`, the action loop `0x43c000`; its pre-send gate `0x437fe0` is a line-of-sight test, 457/458). Bounded search: other paths not read | *inferred*: the client sends, the server decides; **live** |
| Healing, Healing Prayer, Recovery cast times | not traced (the cast bar most likely uses the client's record) | *inferred*: longer bar than the server's cast |
| Kimuraku 835 max HP (client 115,438, server 1,115,438) | `0x0601` takes the max from `Monster.SData` (`0x552380`); **`0x0608` (`u32 id, u32 maxHP, u32 HP`) overwrites it** (`0x54eff0`: mob `+0x54` max, `+0x58` HP; confirmed). A server that sends HP only through `0x0304`/`0x0305` leaves the file's maximum in place | the bar is wrong only because the max comes from the file; sending `0x0608` with the database's max after `0x0601` (and on `0x0304`) fixes it without a client change |
| Prices, tooltips, potion amounts, skill bonuses | display only (the server applies its own values) | tooltips differ |

A comparison tool (the client repository's `tools/sdata_diff.py`) diffs two client-format files record by record.

Layouts (confirmed, both parse to EOF): `Item.SData` = `u32 100` banks, each `u32 count` + records (name, description, **81 fixed
bytes**), 16,045 records; `Skill.SData` = `u32 326` skills × 3 levels, records (name, description, **116 fixed bytes**). Item field
names come from the client loader (`0x460500`, file offset → memory offset) and sample records: Type, TypeID, ReqLevel (+4),
faction (+6), the six class flags (+7..+12), Special (+35, drives the item-use switch), durability (+37), effect values (+39..),
HP/SP/MP (+47/+49/+51, used for potions), Buy/Sell price (+67/+71), stack count (+80); the rest are reported by offset. File bytes
+14/+15 are never stored by the client.

## 7. Region features (confirmed)

Several behaviours depend on a per-region feature table, not on the server. The object at `0x7013d4` (built by `0x405cd0`, vtable
`0x6a5614`) holds 63 features, each with a list of region numbers; `IsOn(n)` (`0x405c60`) is true when the client's region
(`0x6f1558`) is in feature n's list, or when the list has 0 and the login address is the internal `172.17.10.205`. Regions
(`0x408750`, from `CONFIG.INI [LOGIN] VERSION`): 1 Korea, 2 China, 3 Vietnam, 4 Japan, 5 Taiwan, **6 English (the default,
this client)**, 7 Germany, 8 Singapore/Malaysia, 9 Hong Kong, 10 France, 11 Russia, 12 Turkey, 13 Spain, 14 Brazil, 15 Latin
America, 16 Poland, 17 Italy. The lists were rebuilt from the constructor's stack stores (the decompiler drops them).

On for region 6: features 3, 4, 10, 11, 13–17, 19, 22, 30, 31, 33, 34, 37–40, 42, 54, 57, 61. The ones met so far:

| feature | regions | what it gates | here |
|---|---|---|---|
| 0 | 0–3 | lapis extraction with a chosen gem / extraction hammer (8-byte `0x0802`); hammer kinds in the link window | off |
| 11 | many, incl. 6 | colour/link markers on item names in AH notices | on |
| 22 | many, incl. 6 | Movement Rune falls back to the party window's selected member | on |
| 32 | 0, 1, 4 | Teleportation Stone (Special 39): sets the number of usable save slots from the item (`0x4c7f6a`). The stone works on this client, so either this list misses region 6 or the count is set elsewhere (section 1) | off? |
| 59 | 8 | skips the exit dialog 0x12 | off |

Japan (4) also has its own code paths outside the table (exit flow `0x2704`/`0x2705`, fixed enchant cost); Vietnam (3) allowed
opening a personal shop anywhere (`patches.md`, Market Anywhere).

Every caller of every feature (260 calls) and what turning each off-feature on would do: `packet-opportunities.md`
("Region features"), with feature 0 traced in full.

## 8. More answers

Honour (`0x0230` is ignored), trading with the other faction (the player menu opens only on my faction or on maps 42 and 40), item wear
without a packet, and the `0x021B` reset counters (display numbers only) are answered, with addresses, in
[client facts](CLIENT-FACTS.md) ("Answers of October 2026"). The auction row layouts are in `auction-house.md`; the GM questions in
`gm-and-commands.md`.
