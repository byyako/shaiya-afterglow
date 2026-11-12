# Client facts for the server

This page collects what the client `game.exe` (US build 2010-04-26, patch 108) does that matters to anyone writing or running a server for
it: corrections to earlier notes, request and reply layouts, what the client checks or filters itself, its region features, the SData
layouts, and answers to specific questions about scripted monsters, party instances, unions, monster HP and the duplicate-login message.
Each fact names the client address it comes from.

Status words: **confirmed** = read in a Ghidra decompile of `game.exe`; **confirmed (disasm)** = disassembly only, provisional;
*inferred*; **live** = needs a logged try. Sizes include the 2-byte opcode, exclude the length prefix. Messages are `sysmsg-uni.txt` IDs.

## In short

- **The client trusts the server for most rules** but checks a few things itself: equipment level, faction, class and gender; which items
  can go in a trade; personal-shop prices. A server cannot rely on those checks against a modified client.
- **Some answers must match the client exactly.** Lapis extraction must be answered with 33 bytes for this client (the original sends 34),
  a free skill reset can count down on screen with one extra byte, and save-point indexes must never be 0.
- **The client hides blocked players' whispers, trade, guild and "all" chat itself**, but nothing else: invitations from blocked players
  still arrive.
- **Monster health bars** can be corrected from the server with `0x0608`; the original never sends it.
- **Party instances** need nothing special from the client: it just follows the server's moves.

## Corrections to earlier notes

| Topic | Before | Now (confirmed) | Source |
|---|---|---|---|
| Warehouse Recall Rune (Item 100/54, Special 35; first written as 100/53, corrected from `Item.SData`) | sends nothing | **sends `0x050A`** like any item. The success reply `0x050A` (11 bytes) opens the warehouse window (dialog `0x67`, "Manage warehouse" 5299). The 5 minutes are **skill 228 "Warehouse Call" lv 1** (named by item bytes +32/+33; ability 39, 300 s): when the server adds it with `0x050D` (`u32, u16 228, u8 1`), the client sets its "warehouse anywhere" flag; while it lasts, using a rune again reopens the window locally (no packet); `0x050E` (buff end) closes a rune-opened window. | the client's action notes §1; `0x4c7d40`, `0x55ae90`, `0x4c0760`, `0x4c0e50`, `0x56c020` |
| NPC warehouse | — | NPC menu sends nothing; window `0x67` (maps 51–52: window `0x72`). Walking away closes it (rune-opened: stays). Contents only from `0x0711`. | `0x48c120`, `0x4c45a0`, `0x43c000` |
| `0x2704` / `0x2705` | sent from the ESC menu | **never sent by this client**: only when the client region is 4 (Japan). Region 6 (English) opens the exit dialog locally. Ignore both. | the client's action notes §4; `0x489ff0`, `0x48c120`, `0x550d20` |
| `0x0802` extract lapis | 4 or 8 bytes | **always 4 bytes** (`u8 bag, u8 slot`). The 8-byte form (extraction hammer + chosen gem) is behind client region feature 0, off for English. The server chooses the gem; extraction hammers (Special 58) cannot be used. | the client's action notes §1, §7; `0x493d30` |
| `0x1C02` | bag/slot | **`u8 type, u8 typeId`** of the selected result ("search this item") | the client's action notes §1; `0x459f60` |
| Block list in the client | whispers only | also **`0x1103`, `0x1104`, `0x1108` chat** (see below) | the client's action notes §3; `0x554260` |
| `0x0109` second byte | "the original never sends `0x0109`" | the original sends it; byte 2 (`maxGrow`) enables the Hard/Ultimate creation buttons | [Packet opportunities](packet-opportunities.md); `0x466be0` |

The original server answers lapis extraction with the 34-byte `0x0802`; this client reads 33 (see
[Compared with the server pages](#compared-with-the-server-pages) below). Read one byte off, lapis 1–5 still land right but the **gold
shown becomes `money × 256`** until the next money update, and a sixth lapis would be written at bag 255. Drop the `gemPos` byte for this
client. [Packet opportunities](packet-opportunities.md) also lists what the client supports that the original server never sends: item-mall
names and prices (`0xB106`), monster max HP (`0x0608`), the duplicate-login message (`0x0004`), and every region feature a client patch
could turn on (feature 0 = extraction hammers with a chosen gem); warehouse gold (`0x020D` both ways, messages 236/237) turned out to be
**dead on this client** ([below](#corrections-for-a-server)).

## Requests and replies

Confirmed unless marked.

- **`0x0208` stat up**, 14: `u16 STR, DEX, REC, INT, WIS, LUC` — matches [Character numbers](../original/stats.md#5-spending-stat-points-0x0208).
  Sent once, only if any > 0. The echo reply: client adds each and subtracts the sum from free points; no message. (`0x4f3990`,
  `0x585a20`, `0x54ef10`)
- **`0x0214` stat reset reply**: `u8 ok`, then `u16 points, STR, REC, INT, WIS, DEX, LUC` (different order from `0x0208`) → 5239 / 5240.
  After a success the client decrements its `0x021B` stat counter itself if it is non-zero (`0x54fe30`); `0x0215` likewise for the skill
  counter (`0x5528a0`). The client stores both `0x021B` bytes as counters (`0x550820` → `0x21e5588`/`0x21e5589`, read by the NPC menu
  `0x48c120`; see [the NPC-13 menu](#compared-with-the-server-pages)).
- **`0xF701` GM set status** `u8 attr, u32 value` (`0x554030`): 1 level, 2 gold, 3 stat points, 4 (*inferred* skill points), 5 STR,
  6 DEX, 7 REC, 8 INT, **9 LUC, 10 WIS**, 0x10 …
- **`0x050A` item use**, 4: `u8 bag, u8 slot`. Exceptions: type-100 Special 41 / 32 send **`0x0557`** 8 (`u8 bag, slot, u32 target`):
  41 only with a player selected (nothing sent otherwise), 32 = chosen party member. Specials 39, 44, 45, 50 open a window first; 50 (Exp
  Removal Pill) then sends `0x050A` from it. Reply `0x050A` 11 (`u32 charId, u8 bag, slot, type, typeId, count`): **bag 0 = failure**
  (227, or 488 "Battle rune … Blue grade" when the battle-rune flag is set). (`0x4c7d40`, `0x55ae90`)
- **`0x0801` link** 8: `u8 lapisBag, lapisSlot, itemBag, itemSlot, extraBag, extraSlot` (extra = Linking Hammer, Special 36, or 0/0).
  **`0x0805` enchant** 9: `u8 lapisianBag, lapisianSlot, materialBag, materialSlot, materialCount, equipBag, equipSlot` (PDB
  `TPC_LAPISIAN_ADD`; Lapisia = item type 95). Both, and `0x0554`, only while an NPC is selected; **no NPC id on the wire**.
- **Mail (`1Dxx`)**: the client never sends any; receive-only.

## Auction house (`1Cxx`)

All confirmed (the client's action notes §1 and §2). The full page is [Auction house](auction-house.md).

Requests use a second send routine `0x585520 send3(opcode, payload, len)`; the [packet catalogue](client-packets.generated.md) lists them.
Results go through the fallback to `0x55d490`.

Requests: `0x1C01` 39 search (`u8 classFilter 0/1/2, u8 minLv×10, u8 maxLv×10 (0xFF none), u8 category, u8 grade (0xFF any),
char name[32]`); `0x1C02` 4 (`u8 type, typeId`); `0x1C03` 3 (`u8` 1 prev / 2 next, *inferred*); `0x1C04`/`05` (my registrations / bids)
and `0x1C06`/`07` (items / gold to collect) 2 bytes, **each once per session**, then kept current by the results; `0x1C14` 2 interest list
(every time); `0x1C08` 14 register (`u8 bag, slot, count, period 0–3 = 7/24/72/168 h, u32 minBid, u32 buyNow`; client checks minBid ≤
3.8e9, buyNow ≤ 4e9, buyNow ≥ minBid, gold ≥ commission ≈ round(minBid×2e-6)×hours×5); `0x1C0B` 10 bid (`u32 id, u32 price`);
`0x1C0A`/`09`/`0C`/`0D`/`15`/`16` 6 (`u32 id`: buy now, cancel, collect item, collect gold, add/remove interest); `0x1C17` 2 clear interest
list. "Collect all" = the client sends the next `0x1C0C`/`0x1C0D` after each success.

Results (row sizes: search/interest/bid 48, registration 52, to-collect item 41, to-collect gold 53): `0x1C01`–`03`
`u8 hasPrev, u8 hasNext, u8 n, n×48`; `0x1C04`–`07` `u8 n, rows`; `0x1C08` `u8 r`; 0: `u8 bag, slot, count, u32 gold, 52-row` (7116 /
7117); `0x1C09` `u8 r`, 0: 41-row (7119 / 7120); `0x1C0A` `u8 r`, 0: `u32 gold, 41-row` (0 7111, 1 7107 closed, 2 7108 own item, 3 7102
gold); `0x1C0B` `u32 id, u8 r`, 0: `u32 gold, 48-row` (7106), 1 7107, 2 7108, 3 7109, 4 7102, 5: `u32 price` 7104; `0x1C0C` `u8 r`, 0:
`u32 id` + the `0x0205` `TP_ITEM_GET` fields (7121 / 7122); `0x1C0D` `u8 r`, 0: `u32 id, u32 gold` (chat 53 with the gain; 7123 / 7124);
notices `0x1C0E` 53-row 7130, `0x1C0F` 41-row 7131, `0x1C10` 11 (`u32 id, u32 price, u8 type, typeId, …`) 7132, `0x1C11` 41-row 7133,
`0x1C12` 53-row 7134, `0x1C13` `u8` 0 7135 / 1 7136; `0x1C15` `u8 r`, 0: 48-row (0 7125, 1 7126, 2 7127, 3 7128); `0x1C16`
`u32 id, u8 r`; `0x1C17` `u8 r`. The row layouts are on [Auction house](auction-house.md), and which fields the list draws on
[Auction house (server)](../original/auction.md#27-what-the-client-draws-of-a-list-row-client-research-queue-5-verified).

## Blocked players

The client (`0x554260`, block list `0x21e48e4`, by name, case-sensitive) **already hides** lines from blocked names in **`0x1102`
whisper, `0x1103` trade chat, `0x1104` guild chat, `0x1108` "all members" chat**; GM chat (`0xF102`–`F104`) passes. Not filtered by the
client: `0x1101` normal, `0x1105` party, `0x1107` shout, `0x1109`, `0x1111` zone, and **no invitation of any kind** (party, union, trade,
duel, guild, buddy, ride together — none of those handlers touches the list). So if blocking should cover invitations or the unfiltered
chat kinds, **only the server can do it** — neither the original server (see [Friends](../original/friends.md#7-the-block-list)) nor the
client does.

## Chat opcode names

The client shows `0x1103` as **"[Trade]<p> : <b>" (355)** and `0x1108` as **"[chatting all members]<p> : <b>" (481)** (`0x554260` types 1
and 3; the EP6 names agree: `0x1103` GameChatTrade). Each kind should be relayed under the opcode it came in with, with the client's
meaning for its scope (trade vs all).

## Server questions answered from the client

Confirmed; `game.exe` handler → logic.

- **`0x0109`** S→C 4 (`u8 country, u8 maxGrow`, `0x55cec0`): the client stores the faction (`0x21e4942`, `0x84fa1c`) and `maxGrow`
  (`0x21e4943`), which **enables the creation-mode buttons**: below 2 Hard is disabled, below 3 Ultimate (`0x466be0`). The original sends
  it (`CUser::PacketUserDBChar`; see the [game server](../original/ps_game.md#61-what-the-client-receives-after-the-handshake)).
- **`0xB103`** (`u8 billingType, u32 value`, `0x559930`): region 2 (China): `value` = seconds left, messages 813 (+834 under 3 h). Other
  regions: types 1–4 show 814 "individual package (expire date)", 815 "franchise PC room 1.5× EXP", 816, 863, with `value` as a packed date
  (year = v>>26 + 2000, month = v>>22 & 15, day = v>>17 & 31, then time). **Type 0 shows nothing**: send 0.
- **`0xB104`** (`u32` seconds, `0x559cf0`): "3 hour-free service (time left)" (813), 847 when under an hour.
- **`0x0404`** (`u32`, `0x54f020` → `0x4c1bb0`): the world clock in seconds, advanced locally each second (`0x48c120`); split as
  v / 7,257,600 (84-day period), then weeks (604,800), then 4-hour blocks (14,400; block / 6 = weekday), seconds in the block.
- **`0x0211`** (`u8 country < 2, u32 value, u32 ms`, `0x54cb90`): per-faction value and a countdown the client runs down itself
  (`0x482bb0`); a related path (`0x54caf0`) sets 600,000 ms, plays `ui_etainbless01.wav` and shows `event.tga` when the value reaches a
  threshold — *inferred*: the "blessing of the goddess" faction event (see [Player kills 2](../original/pvp-2.md#9-the-counts-and-grades)).
- **`0x0E04`** C→S = NPC clicked (`u32 npcId`); a `0x0E04` reply is ignored (see [World packets](client-world-packets.md)).

## Client region features

From the client's action notes §7. Behaviour that looks server-driven but is fixed in the client by region (`0x405cd0`, 63 features ×
region lists; region 6 = English): extraction-hammer UI (feature 0) off; item-name colour markers in AH notices (11) on; Movement Rune
party fallback (22) on; Teleportation Stone point count (32) off; exit dialog skip (59) off. Region 4 (Japan) also gates the
`0x2704/05` exit flow and a fixed enchant cost; region 3 (Vietnam) the shop-anywhere rule. The full list is on
[Packet opportunities](packet-opportunities.md#region-features-what-a-client-patch-would-unlock).

## SData layouts

The record structure of the client's SData files (see [Protocol notes](../original/protocol-notes.md#51-sdata)):

`Item.SData` (client patch 108, = `ps0108.patch`): `u32 100` banks; per bank `u32 count`; per record `u32 len + name`, `u32 len + desc`,
**81 fixed bytes** (Type +0, TypeID +1, ReqLevel u16 +4, faction +6, class flags +7..+12, Special +35, quality u16 +37, effects u16 +39..,
HP/SP/MP u16 +47/+49/+51, STR..LUC u16 +53..+63, Buy/Sell u32 +67/+71, stack +80); 16,045 records. `Skill.SData`: `u32 326` skills × 3
levels, records name, desc, **116 fixed bytes** (e.g. skill 228: ability 39 at +87, duration u16 at +59). The client's `Item.SData` is
compared with the server's item table on [Item.SData against the server](item-sdata-vs-defs.md).

## Compared with the server pages

Read against [Using items](../original/item-use.md), [Items that open a window](../original/item-windows.md),
[Character numbers](../original/stats.md), [Lapis and enchanting](../original/lapis.md), [Friends](../original/friends.md) and
[Movement, sitting, leaving](../original/world.md). **Agree, nothing to add:** `0x0208` order and echo; the `0x0805` answer (36 bytes, the
client reads all of it, `0x57ca50`); the `0x0225` list (`u8 n, n × 15`); `0x0228` shows 862 "failed to change your gender because you
have a gender-restricted item equipped" (`0x557c90`); the chat names. The rest:

| Topic | Server pages say | Client (status, address) | For a server |
|---|---|---|---|
| **`0x0802` extract answer** | 34 bytes, `TP_GEM_REMOVE`: `u8 success, bag, slot, gemPos, gemBag[6], gemSlot[6], gemTypeId[6], gemCount[6], u32 money` | **This client reads 33 bytes, without `gemPos`**: `u8 success, bag, slot, u8[6] × 4, u32 money`. The 34-byte form is read only when region feature 0 is on (off for English, as for the 8-byte request). confirmed (`0x57cb40`: `IsOn(0)` on the feature table `0x0222ea44` = `0x7013d4`, set by `0x40a6f0` after `0x405cd0`) | **Send 33 bytes (drop `gemPos`)**. The EP4.5 34-byte answer is read one byte off: `gemPos` (255) taken as the first gem's bag, the money short of its top byte. Field names after `slot` are the server's (*inferred* from the same order) |
| `0x0801` link answer | 20 bytes: … `u32 money, u8 hammerBag, u8 hammerSlot` | reads **18**: 12 × `u8`, `u32 money` (confirmed, `0x57c930`); the hammer is updated by its own `0x050A` | either size works (trailing bytes are ignored); 18 is the client's |
| **`0x021B` reset counters** | `u8 −m_byResetStatus, u8 1 − m_byResetSkill`: `00 01` while the free skill reset is unused | The two bytes are **"chances left" counts** shown by the **NPC type 13** menu (`0x48c120`, `DAT_00854eac` 13): option 2 "<v> chances left. Do you want to initialize your character's stats?" (5172, `v` = byte 0), then "do you want to initialize stat points?" (5169), yes → `0x0214`; option 3 the same with byte 1 (5173, 5170) → `0x0215`, but only when bags III and up are empty (else 5171, `0x4c2590`). **Display only: the client sends the request whatever the count.** confirmed | `00 01` shows "0 chances left" for stats and "1" for skills, which is right. A stat request still comes (0 chances): answer `0x0214 u8 0` (5240 "already used") |
| **`0x0215` answer, 3rd byte** | 5 bytes: `u8 1, u16 skill points` | the reader takes an **optional `u8` after the points** when the packet has one (`0x580740`); the handler lowers the skill counter only when that byte ≠ 0 (`0x5528a0`). `0x0214` success lowers the stat counter always (`0x54fe30`). confirmed | With 5 bytes the menu keeps saying "1 chances left" until the next login (original behaviour). **Optional:** answer a *free* skill reset with 6 bytes (`u8 1, u16 points, u8 1`) and the client counts it down; send 5 for the Skill Reset Stone |
| **Teleportation Stone window** (special 39) | the window is *inferred* to send `0x0557` with the index | **confirmed**: the window (`0x4dc6b0`) sends **`0x0557 (bag, slot, u32 index 1–4)`** to teleport and **`0x0220 (u8 index 1–4, u16 map, f32 x, y, z)`** with the player's own map and position to save. `0x0220` answer: 1 → 441 "Can't use in a dungeon.", 2 → 227, 0 → slot `index` stored (ignored when index > 4). confirmed | `0x0225` and `0x0220` are stored at `index − 1` with **no check for 0** (`0x55a280`, `0x55a0f0`): never send index 0, and never more than 4 entries in `0x0225` (the reader's buffer is 4 × 15 bytes on the stack, `0x581900`) |
| Teleportation Stone slots usable? | — | The window enables a slot's buttons only for slot < a count (`0x021e5b58`, confirmed (disasm)) whose only direct writes are behind region feature 32; **the stone works on this client** (reported by a player), so the count is set by a path not found. Unresolved, client-only | nothing |
| Body change `0x0226` (specials 44, 45) | any byte accepted | sent from the window `0x4630c0`: `u8 bag, slot` of the scroll, then the preview's two appearance indexes (window `+0x4c`, `+0x48`, each cycled within the model table of the race and sex), **size cycled 0–4**, **sex 0/1 by two buttons**; the window opens only on an item (239). confirmed | an unmodified client never sends sex > 1 or size > 4: a server can refuse anything else as coming from a modified client |
| `0x0207` exp | 10 bytes `u32, u32` | two forms by region feature 17, **on** here: two `u32` (`0x57a380`) | 10 bytes is right; a 6-byte `0x0207` would underflow |
| Login `0xA102` | — | 50 bytes (`char user[32], char password[16]`; feature 9 off), opcode `0xA106` instead when the caller's flag ≠ 0 (`0x5867c0`); feature 9 on would send a 33-byte password (67 bytes) | a live log matched |

Region-feature checks inside the packet readers and senders (`0x578000`–`0x58a000`) are only these three: the `0x0207` and `0x0802`
readers and the login sender (confirmed: every reference to `0x0222ea44` there; the union senders `0x585c20`–`0x585ff0` only log through
it). The `0x0802` request's form is chosen earlier, in the link window `0x493d30`.

## SData differences: what the client does with its own values

There is **no server-pack `Item.SData`/`Skill.SData`**: the original server ships only `NpcQuest.SData` and `KillStatus.SData` and reads
items and skills from the database; the server pack's bundled client (the original files' `Client/`, version 108) holds the same bytes as this
client. So [Item.SData against the server](item-sdata-vs-defs.md) and [Skill and Monster SData](skill-monster-sdata-vs-defs.md) are the
comparison; this answers their *inferred*/*live* rows from `game.exe` (the client's action notes §5). In memory the client keeps item and
skill records' fixed fields 8 bytes after their file offsets (item ReqLevel `+0xc`; skill ReqLevel `+0x10`, job flags `+0x13..+0x18`,
points `+0x1a`).

| Server page row | Client (status, address) | Players see |
|---|---|---|
| capes, mount stones: client price higher | NPC shop sends `0x0702` with **no money check** (confirmed, `0x4f7b80`, `0x4f8220`); the count dialog for stacks is capped by the client's stack size | the client's price in the window, the server's price charged. **Not a refusal** |
| level: rings/amulets 22/154–156, 23/153–155 (31/44/51), Echidna, Zion Cross; mount stones 42/1–4 (40) | every equip goes through `0x4c1590`: refused below the client's ReqLevel (311 "Unavailable level"), also faction 246, mode 247, class 248, gender items 5423 (confirmed) | **cannot equip** between the server's and the client's level |
| 44/89–91, 44/99 lower level in the client | item use refuses only above the client's level (227, `0x4c7d40`) | the client sends; the server's `0x050A` failure shows 227 |
| stack sizes (gold bars, group 100) | `0x0204` is sent for any other slot, no stack check (confirmed, `0x4e6250`) | the server's limits decide |
| Trap for Archers | the next-level arrow needs the job flag of the learnt level, the level and points of the next (confirmed, `0x4f2700`); the list holds only what `0x0108`/learn answers put there (`0x56c1d0`); the not-yet-learnt (grey) entries' source was not traced | an Archer who has Trap Lv1 cannot raise it; Lv1 itself: **live** |
| Pollution Zone points (client 4/4/5) | same arrow: off while points < the client's cost (confirmed) | a Fighter with exactly 3 points cannot learn it |
| Gruesome Attack, Enhance Wind Spin weapons | no weapon check in the send paths read (`0x4f12c0`, `0x4d4e10`, `0x43c000`; its gate `0x437fe0` is line of sight, 457/458) | *inferred*: the client sends, the server decides (**live**) |
| Kimuraku 835 max HP | `0x0601` takes the max from `Monster.SData`; **`0x0608` (14 bytes: `u32 id, u32 maxHP, u32 HP`) overwrites max and HP** (confirmed, `0x54eff0`). The original sends only `0x0304`/`0x0305` | **fixable from the server**: send `0x0608` with the database's max HP after `0x0601` (the original never does) |

## System notes

One page per system in the Client tab; facts marked confirmed / inferred with `game.exe` and `ps_game.exe` addresses as above. The line
under each page is the point most likely to bite.

1. **GM commands** ([GM opcodes and commands](gm-and-commands.md)): [what the client sends at each privilege](gm-and-commands.md#what-the-us-client-sends-at-each-privilege-110) lists privilege 1–10 (6–10:
   only `F1xx`, `F2xx`, `FA16`/`FA17`, which the original disconnects for). `F501`'s failure value is the opcode (all call sites).
   Deferred moves are `op u32 id, u16 map, f32 x, y, z`. `/cwatch` copies (`F502`) are **not handled by this client**; `/autocure` only
   works against `172.17.10.205`; `F202`/`F203` answers only reach the original's game log.
2. **Scripted monsters** ([Scripted monsters](scripted-monsters.md)): **no client-side boss logic**. `0x1109`
   (`u8 kind, u32 mobId, u8 len, text`) and `0x110A` (`u32 mobId, u16 sysmsgId`) are chat lines "`<mob name> : <text>`", not bubbles;
   `0x110B` plays a `Sound/` file at the mob. All three are dropped unless the mob is in view. Phases = `0x0601` swaps, `0x0608` (max HP),
   `0x0609`, `0x0610`.
3. **Personal shop** ([Personal shop](personal-shop.md)) and **auction house** ([Auction house](auction-house.md)): 12 shop slots; the
   client **ignores `0x2309`'s result byte** (any `0x2309` opens the visit window) and takes any `0x230D` result other than 0/2/3 as
   success; the server must enforce the market maps (Market Anywhere). Auction rows are the PDB's `TP_MARKET_ITEM` 48 / `SELLITEM` 52 /
   `ENDITEM` 41 / `ENDMONEY` 53 (layouts there); dates are packed `(year−2000)<<26 | month<<22 | day<<17 | hour<<12 | min<<6 | sec`; gold
   rows are collected by `dwMoneyID`.
4. **Item mall** ([Item mall](item-mall.md)): the product list and prices are `Cash.SData` (`0xB106` can rename/reprice by code); the
   client does not check points or bag space; `0x2602`/`0x2603` requests carry `char productCode[21]` (and a 21-byte name for gifts),
   **not** the `u32`s the catalogue shows; buy answer 0 = success with `bag/slot` per item. `0xB103` type 4 and `0xB105` print
   wrong/missing texts.
5. **Duels** ([Duels](duels.md)), **guild wars** ([Guild wars](guild-wars.md)), **obelisks** ([Obelisks and roads](obelisks.md)),
   **union parties** ([Union parties](union-parties.md)): duel order `0x2401` → `0x2402 1` → `0x2407` stake window → `0x2409` ready →
   `0x2403 (f32 x, z)` → 5 s → `0x2404` → `0x2405` (1 won, 2 lost, 3 draw) / `0x2406` (**0 disconnect, 1 area, 2 gave up, 3 attacked**:
   the catalogue was off by one); area radius 35. GvG: `0x2503 (f32 x, z, u32 enemyGuildId)` to every member of both guilds (the original
   sends `0x2503` and `0x2504` only to the members within 40 of the centre when the war is accepted, sends `0x2506` events 1 and 3
   only, and `0x2505` only 1 or 2; see [Guild wars](../original/guild-war.md#4-answer-0x2502)). `0x2104` (road opened) **never
   prints on this client**; use `0x2102` for link states (100–103). Union: see [Social packets](client-social-packets.md) (no `0x0C0F`;
   `0x0B0B` only adds).

## Answers to server questions

Four questions answered from `game.exe` decompiles (and `ps_game.exe` where named).

**1. Scripted monsters (`0x1109`, `0x110A`, `0x110B`).**

- **`0x1109` with the zero counted is fine** (confirmed, `0x57e650`): the handler reads `u8 kind, u32 id, u8 len` and copies `len` bytes
  into a 132-byte stack buffer whose first 128 bytes it zeroes first. So `len` counting a closing zero prints correctly, and so does the
  original's `len = strlen` with no zero (the buffer is already zeroed), for any `len` ≤ 127. **Keep `len` ≤ 128**: 129–132 without a
  zero run into unzeroed bytes, and **133–255 overrun the stack** (crash). A `len` larger than the bytes actually sent copies only what is
  there (no error). The line is then `sprintf("%s : %s", <Monster.SData name>, text)` into 260 bytes.
- **A trailing `"\n"`** is kept in the string and drawn by `DrawText` without `DT_SINGLELINE` (format `0x100`): an empty second line,
  nothing visible (*inferred* from the draw flags). Dropping it is the clean choice. A `"\n"` in the middle would push the rest onto a line
  of its own.
- **Where the lines show** (confirmed, `0x554260` → `0x41e2d0`, `0x4b7d00`, `0x4b8220`): kind 0 → chat type 0x0F, **chat window only**.
  Kind 1 → type 0x1E, **chat window and the screen-centre list `0x21e4e60`**: centred at mid-screen − 150 px, 17 px a line, pink
  (`0xFFFF80FF`), at most 5 lines (a 6th pushes out the oldest), each 10 s. `0x110A` uses type 0x1E too. The same list carries other
  notices (types 8, 0x18, 0x1B–0x1E, 0x20).
- **Sysmsg 6401–6404 exist** in `sysmsg-uni.txt`: 6401 "Do you think you can leave this place while you still breathe?", 6402 "Guards!
  Kill them!", 6403 "You can't win this fight.", 6404 "How dare you... I will kill all of you !!". An ID that does not exist prints
  "GetMsg Error <id>" as the text (confirmed, `0x41c180`).
- **`0x110B` takes a bare file name, no `Sound/` prefix** (confirmed): the name is joined as `data/sound/<name>` (`0x5955a0` `"%s/%s"`,
  directory `"data/sound"` set by `0x526f80` at sound manager `0x21edc20 + 0x888`), loaded once and cached by name, played as a 3-D sound
  at the mob. `Sound/x.wav` would look for `data/sound/Sound/x.wav` and play nothing.
- For boss phases: **a `0x0601` for a mob id the client already has is ignored entirely** (no model, size or HP change, `0x552380`). A
  phase that changes `wMOBID` must send `0x0602` first.

**2. Party-instance maps 65/66 (Cailum Sacra).** The client has **no instance logic**: it never sees `CreateType`, has no instance
packet, and no map-id case for 64–67 except map 64's loading picture (confirmed: every compare of the map id `0x84e6e0`). What it needs:

- **Map files**: the client has `world/65.wld` and `66.wld` (dungeon `B5_Dun_Caelumchakra_1F_final.dg`) and `67.wld` (2F). Their portal
  lists match the original `.svmap` files index for index (checked for maps 47, 65, 66, 67):
    - in: map **47 portal 6** (country 1) → map 65 (313, 29, 341); **portal 4** (country 2) → map 66 (same spot);
    - in 65/66: **portal 0** → map 67 "Cailum Sacra 2F" (65: 19, 27, 81.1; 66: 605.2, 27, 82.1); **portal 1** "Section 4", country
      **1001**, to the same map (87.5, 33, 180.2), noted in the client file as "opens when the mid-boss is killed"; **portal 2**, back to
      the 1F start (221, 31, 180);
    - map 67 is an ordinary dungeon (no `CreateType`); its portals 4 and 5 lead to map 0 (the two castles).

    `Map.ini` for 65/66: `CreateMinUser` 1, `EnterMaxUser` 7, `ExpireTime` 5, no `CreateTime` (always open); death: `RebirthMapPos` map 47
    (65: 125.58, 9, 454.08; 66: 571.37, 7, 194.08).

- **Entering** (confirmed, `0x443541`): the client sends `0x020A u8 portal` when the player stands in the portal's box (its own `.wld`
  box), with **no** level or country check, and again every 15 s while still inside. It then just follows the server's move (`0x020B`, as
  for any map change). Refusals `0x020A (u8 portal, u8 reason)` print (chat type 7, `0x555b60`): **4** → 8042 "only party members
  allowed", **9** → 6600 "Now party instance zone closed.", **10** → 6601 "Can't enter because of lack of party members.", **11** → 6602
  "You failed to enter.", 12 → 6603; 0–3 and 6–8 are guild-house and ranking texts; **5** opens the ranking-battle question 8043, whose
  yes sends **`0x0227 u8 portal`** (the portal the player stands in). Other values: silent. 6600–6602 are used nowhere else in the client.
- **`LuaUpdateInsZonePortal` sends nothing** (confirmed in `ps_game.exe`: `CMob::LuaUpdateInsZonePortal` 0x4b3520 →
  `CZone::UpdateInsZonePortalCountry` 0x417a10 only stores `(portal id > 1000, country)` in the zone's 10-entry table at `+0x218`;
  `GetInsZonePortalCountry` 0x4179e0 answers 3, nobody, for an id not stored; `CZone::BothMobRespawn`/`BothMobDestroy` also call it). The
  client always draws the portal and always asks; a closed "Section 4" just gets no answer (the original's way) and the client asks again
  every 15 s. Nothing needs to be sent when it opens.
- **Leaving**: portal 0 (to map 67), death (`RebirthMapPos`), or any move the server makes (expiry, a member leaving the party): the client
  has no rule of its own and follows the move packet.

**3. Union parties** (see [Union parties](../original/union.md)).

- **What sends `0x0B0F` / `0x0B10`** (confirmed, `0x48c120` at `0x4916f5`…`0x491836`): the union settings window's OK. Not in a union:
  `0x0B0E` (`u8 1, u8 autoJoin, u32 rule`), "Create Union". **In a union and its leader** (role 0): `0x0B0F u32` rule (the window's three
  buttons → 1, 2, 4) only when it differs from the current rule, and `0x0B10 u8` auto-join only when it differs. A sub-leader or member
  sends nothing. **So this client never sends `0x0B0E` for an existing union**; "Config Union" is `0x0B0F` / `0x0B10`.
- **Handlers**: `0x0B0F` (`0x5813e0` → `0x55ea30`) sets the rule and prints 6516 / 6517 / 6518 for 1 / 2 / 4 (other values stored,
  silent); `0x0B10` (`0x581410` → `0x55ea80`) sets auto-join and prints 6520 "Auto-join mode is activated." / 6521 "…deactivated.".
- **A joiner that gets only `0x0B0B` (as the original sends it: no own `0x0B0C`) is not in the union on the client** (confirmed, `0x561340` vs `0x5610c0`): `0x0B0B` fills the list,
  the leader, sub-leader, rule, auto-join and the joiner's group, but sets **no** in-union flag (`0x21e4928`), role (`0x21e485c`) or window
  flag and prints nothing. Only an own `0x0B0C` (or a `0x0B0E`) does: role 2, union window on (unless `CONFIG.INI` says FALSE), "Joined the
  party" (207). Without the flag the joiner sees no party chat (looked up in the empty party list), its settings window would send `0x0B0E`,
  and `/party` sends `0x0B09`. **Send the joiner its own `0x0B0C` as well**; either order works (the list is a map insert, `0x48bd70`: a slot
  already present is kept, not replaced); after the list puts 207 last.
- **`0x0B0E` received by an existing union** (confirmed, `0x560780`): the client clears its union list and **rebuilds it from its normal
  party list** (`0x21e4838`, the `0x0B01`/`0x0B02` members), slot = index there; not from the server's order and not from its union list.
  Members known only from `0x0B0C`/`0x0B0B` vanish from the window. It also needs the leader and the sub-leader slots among the rebuilt
  members, or it sets nothing else (no 6532, no union flags; this also hits a union created from a party whose sub-leader slot holds
  nobody). Since the client never asks for it, a server can simply not send `0x0B0E` to an existing union (and use `0x0B0F`/`0x0B10`).
- **`0x1105` shows for union members** (confirmed, `0x554bd0`): in a union the sender is looked up in the union list and the line is 353
  "[Party]<p> : <b>" (type 10, chat window); an unknown sender prints nothing.
- **`0xF10A`** (confirmed, `0x5812c0` → `0x554230`): `u32 id` is read and dropped (never shown or used), `u8 len` (≤ 255, 1024-byte zeroed
  buffer), text → 6606 "[Union] <b>" with type 0x20: **chat window and the screen-centre list** (pink, 10 s, as in 1). No name is added:
  put the speaker in the text if wanted.

**4. `0x0608` and `0x0004`.**

- **`0x0608`** (14: `u32 id, u32 maxHP, u32 HP`; confirmed, `0x57c2f0` → `0x54eff0`): looks the mob up and, if known, **sets** mob `+0x54`
  = max HP and `+0x58` = HP, the two values the target bar draws. Unknown id: dropped, so **send it after the `0x0601`** (the same flush is
  fine), to every player that gets that `0x0601`, including on re-entering view. A new mob gets max HP from `Monster.SData` and HP 0
  (`0x552380`, constructor `0x415670`), so **`0x0608` alone fills the empty bar** and also corrects the max (Kimuraku 835). Send it again
  whenever the absolute values should be reset (phases, `0x0610` drift).
- **`0x0004`** (3: `u8`; confirmed, `0x578ed0` → `0x54c860`): 0 → 10070 "disconnect due to using logged in ID.", anything else → 10071
  "disconnect due to system error". It shows as a Windows box "Message" (or the in-game box when network object `+0x370` is set), then the
  client marks itself closing and posts `WM_CLOSE` to its window: **it quits by itself**. Send it to the older connection and close that
  connection gracefully afterwards (flush, then shut down sending), so the 3 bytes are not lost to a reset. Nothing is needed in any order
  with `0x0601`.

## Answers of October 2026

Read in `game.exe` (decompiles and disassembly), October 20 and 21, 2026.

- **Move speeds and how often `0x0501` is sent**: the client's walk and run speeds by move index, the mounted speeds, and the send
  intervals (no minimum interval or distance) are on [The move speed check](../original/move-check.md#2a-the-clients-speeds).
- **Opening the item-mall and billing storages**: the point shop's store button and the Light's and Fury's Bank Tellers (1/179, 1/180);
  their lists are pushes ([Item mall](../original/item-mall.md#6-the-billing-storage-0xb101-0xb102)).
- **`0xB103`** on region 6 reads its value as a packed local date and prints only the day, once ([Item mall](../original/item-mall.md)).
- **Honour after `0x0230` is ignored**: the handler (slot `+0x58`) is `0x407660`, a bare `RET 4`; no other packet carries honour and the
  US EP4 character window has no honour line. confirmed
- **"Request transaction" to the other faction**: the player menu opens only on a player of **my faction, or on map 42 or 40**
  (`0x4420c7`), within 10; it then offers 5091 unless one side has a shop open. So for enemies the client offers a trade in the Auction
  House (map 42) and on map 40 only. confirmed (disasm)
- **Item wear without a packet**: the client lowers its own copies as the original server does: the weapon (slot 5) −1 on **my critical
  hit** (`0x4c1f80`, result byte 1), one armour piece per blow taken in the order shield, helmet, upper, gloves, lower, boots with a
  rotating index (`0x4c1fe0`, any result but 3), none while ability 36 (Eternal Endurance) is on. It does not check the shape (the
  server skips wear while hidden or transformed). The real value comes only with `0x0554` (repair) and item units (`0x0106`, `0x0204`,
  `0x0205`). confirmed
- **The reset counters of `0x021B`** are display numbers only, "<v> chances left" (5172 stats, 5173 skills) at NPC type 13; nothing is
  gated on them, and the client lowers each by 1 after a success. A skill reset also needs bags 3 to 5 empty (5171). confirmed
- **Hiding another player** (Stealth, invisible GMs) and **what `/kick` shows**: [GM opcodes and commands](gm-and-commands.md#what-hides-another-player).

## Corrections for a server

- **Warehouse gold `0x020D`: not reachable on this client.** The request and the answer handler exist, but the number dialog that sends
  `0x020D` (warehouse window `+0x720`) is **never created**: every store to that field writes 0 (confirmed, byte scan of `.text`;
  [Packet opportunities](packet-opportunities.md), opportunity 1). The original has no `0x020D` case either.
- **`0x2104` (road opened)**: dead on an unmodified client. The handler reads the wrong argument; it wants **link ids 1010, 1011, 1020,
  1021** (Jungle of Light, Desert of Light, Jungle of Fury, Desert of Fury: messages 5374, 5373, 5372, 5371). A 5-byte client patch is
  designed ([Obelisks and roads](obelisks.md)), not applied. If road portals are ever used and the patch ships, number the four roads that
  way and send `0x2104` only when a road opens.
- **`0xB106` product list** (`u8 n`, n × `char code[21], char name[51], u32 price`): renames/reprices `Cash.SData` products by code; the
  client asks once per scene set-up and nobody answers ([Item mall](item-mall.md); [Packet opportunities](packet-opportunities.md),
  opportunity 3).
- **`0xB103` type 0** for the billing line (shows nothing).
- **Item mall beyond `0x2601`**: buy `0x2602`/gift `0x2603` take a `char productCode[21]`; mall storage `0x2606`/`0x2607` (240 slots),
  billing storage `0xB101`/`0xB102` ([Item mall](item-mall.md)). The client checks neither points nor bag space.
- **Auction house `1Cxx`** and **mail `1Dxx`** (receive-only): [Auction house](auction-house.md) has the row layouts.
- **Region feature 0 (lapis extraction with a chosen gem)**: with that patch, the original's 34-byte answer to the 8-byte request is right.
  Other client patches worth considering ([Packet opportunities](packet-opportunities.md#region-features-what-a-client-patch-would-unlock)):
  24 on-screen login keyboard, 25/26 target HP % (pairs with `0x0608`), 46 auto-dismount (uses `0x0216`), 5, 8, 23, 43, 45, 62
  (client-only).

## Watch: HP, MP and SP above 65,535 (and current values above 32,767)

Found 2026-11-11 while testing the EP5.4 decomp client. Nothing is broken today; this is a limit to keep in mind when item, buff or
lapis stats are raised.

- **The wire is 16-bit for this client.** Max HP/MP/SP travel as `u16` in `0x0105` and `0x050B`, current values as `u16` in `0x0301`,
  `0x0302`, `0x0505`, `0x051F`, `0x0521` and the party and union packets (`0x0B01`/`0x0B04`/`0x0B0B`/`0x0B0C`, `0x0C01`/`0x0C03`/
  `0x0C08`/`0x0C0A`/`0x0C0C`/`0x0C0D`). On the original servers a value above 65,535 rolled over in the client (70,000 shows as 4,464);
  EP5 widened all of these fields to `u32` to fix that.
- **The server clamps instead of rolling over.** `crates/game-server/src/stats.rs` `Totals::max_health` clamps to 65,535 before sending,
  and current health is kept as `u16`, so a character whose gear adds up past 65,535 silently plays at 65,535. No wrong display, but
  the extra HP is lost.
- **The save is lower still: 32,767.** `crates/game-server/src/pg/character.rs` (`save_position`, `save_stat_reset`, `save_progress`)
  cut current HP/MP/SP to `i16::MAX` because `characters.hp`/`mp`/`sp` are `smallint` (`db/migrations/0005_characters.sql`). A
  character above 32,767 current HP is saved at 32,767 and logs back in below full. This can bite before the 65,535 limit does.
- **The way out (if stats ever need it):** 32-bit current and max health in the server and the database, and a per-session wire width.
  The 2010 client can only read 16 bits (unless `fixes.dll` patches those readers), so it keeps `u16`; the EP5.4 decomp client is
  being taught to read either width, chosen by the client version it sends at server select (`0x10042600` = 16-bit). The game
  server would need that version from the login server (the `0xA301` hop does not carry it).

## Open questions

- **Live** checks not done: `0x0802` with the 33-byte answer, the NPC-13 "chances left" counts, F501 failure codes, the word filter on
  region 6, Trap Lv1 for an Archer, Gruesome Attack with a sword.

## Corrections to the server pages

None yet. Findings here that contradict or extend a server analysis page are listed with the page and anchor, so the server side can correct that page.
