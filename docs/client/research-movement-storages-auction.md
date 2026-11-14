# Movement speeds, storages, billing and auction rows

This page collects four client findings read from `game.exe` (US build 2010-04-26, patch 108): how fast the client moves
and how often it reports its position (`0x0501`), how the two storages open, what the billing packet `0xB103` shows, and which
auction-row fields the client displays. It ends with the status of the client-side fixes this work touched. Fresh Ghidra decompiles were
read with objdump and direct reads of the constants as a cross-check. Status words: **confirmed** (decompiled), **confirmed (disasm)**
(instructions or data bytes only), *inferred*, **live** (needs a game session).

## In short

- **Forward speed is a small table:** walk 1.5–3.5 and run 3.0–7.0 world units per second by move index 0–4, and 7.0/8.0 on a mount.
  Strafing and backing up use other coefficients, so a combined diagonal can exceed the forward-only table.
- **`0x0501` (an [opcode](glossary.md#opcode)) has no universal send interval or distance.** The client sends on timers of 250, 500 or 1000 ms by situation and directly on
  corrections and warps.
- **Opening either storage sends nothing.** The server must push the list when the window opens; taking an item is a drag that sends
  `0x2607` or `0xB102` with only the storage slot.
- **`0xB103` carries a packed date,** not seconds; the client prints it once and does not run a countdown for region 6.
- **Every field of an auction row (48 bytes) is meaningful,** even those the list does not print: quality, gems and craft data feed the
  tooltip.

### Using this page

- **If you write a server:** the speed table, the storage-open behaviour and the `0xB103` date format tell you what the client expects; "Opening the two storages" matters most.
- **If you patch or mod the client:** "Client-side fixes touched by this work" lists what was changed and what state it is in ([fixes-dll.md](fixes-dll.md)).
- **To check a claim yourself:** the functions and constants are named in each section ([tools-and-method.md](tools-and-method.md)).

## Movement speeds and `0x0501`

Incoming `0x051C` logic (`0x54f260`) stores the move index at player `+0x275` and in the local global `0x21e4941`. The player update
(`0x418c00`, especially `0x41932c`..`0x41955e`) and the local mover (`0x43c000`, `0x43c357`..`0x43c3e2`) select these forward speeds in
world units per second (**confirmed**):

| Move index | Walk | Run |
|---|---:|---:|
| 0 | 1.5 | 3.0 |
| 1 | 2.0 | 4.0 |
| 2 | 2.5 | 5.0 |
| 3 | 3.0 | 6.0 |
| 4 | 3.5 | 7.0 |

The base is 5.0, index adjustments are −2/−1/0/+1/+2, and walking multiplies by 0.5. The local base global `0x701940` is initialised at
`0x4087a7` to 5.0; its other writes are developer movement controls in `0x43a530`, not a server speed setting. Run/walk is player
`+0x168` (non-zero = run). Index 255 gives zero in the local mover before mounted overrides. The floats at `0x6a62d8`/`0x6a67ac`/
`0x6a6638` are exactly 1/2/0.5 (**confirmed (disasm)**).

**Mounted:** the mover overrides the forward run speed to **7.0 when byte `+0x378` is non-zero and not 2**, and **8.0 when it is 2**;
walking is half (3.5/4.0). These are client mount flags, not a server's vehicle-status numbers. The shape handler `0x4c17a0` calls
`0x416950` with flag 1 for shapes 14 and 16 and flag 2 for 15, 17, 24–27, 29 and 30; the setter writes `+0x378` at `0x416970`. A flat
riding allowance of 8 therefore covers both forward speeds. A passenger follows its driver.

**Other directions:** the keyboard branches in `0x43c000` and the remote update `0x418c00` use lateral coefficients 0.5, 1, 1.5, 2, 2.5 and
backward coefficients 0.8, 1.3, 1.8, 2.3, 2.8; mounted lateral/backward is 2. The mover adds direction contributions before collision
resolution (`0x4383a0`). *Inferred*: combined forward and strafe input can exceed the forward-only table (about 5.22 at index 2); the
forward table is not a hard maximum for every input or terrain. A server that tightens its speed check should calibrate **live** first:
sustained diagonals, slopes, mount variants, lag and frame caps.

**Time scaling** is in seconds: `0x405850(6)` returns the performance-counter delta over the frequency (a `timeGetTime` fallback /1000),
assigned to `0x701b14` at `0x40be5f`; the mover multiplies velocity by it.

**`0x0501` send behaviour (confirmed):**

- The sender `0x586060` builds 17 bytes: opcode, u8 movement flags (`0x80` and `0x40` from the sender's two boolean arguments), u16
  direction, f32 x/y/z. It checks that x is finite, nothing about distance or send interval; callers own the timing.
- The ordinary movement helper `0x437050`, in movement mode 1, sends when the unsigned elapsed time is **strictly more than 1000 ms**,
  then sets its last tick `0x705098`. The stop/transition path sends immediately when the previously-moving flag is set. It is called
  by the scene update `0x42cc10`.
- Action, turn and change paths in `0x43c000` use **strictly more than 250 ms** against `0x705128` (for example `0x44034a` and
  `0x44038f`; helper `0x43a460` is also called for dirty movement flags). A landing or vertical-change path uses **more than 500 ms**
  before `0x43a460`.
- Corrections and warps also call `0x586060` directly (`0x553550`, `0x5536a0`, `0x553cb0`, `0x54f330`). There is **no universal minimum
  interval or positive minimum distance between packets**. The 0.005 displacement accumulator in the mover concerns simulation and
  collision input, not packet distance. Equal-position packets and bursts must stay acceptable to a server.

## Opening the two storages

All **confirmed**, from the UI dispatcher `0x48c120` and the actual sender references:

- **Item-mall storage:** open the point shop and click its store/storage button (texture `data/interface/ins_market_store_button.tga`).
  It is button object `mall+0x602c`, initialised in `0x4cd7f0` at `0x4cdc9f`/`0x4ce1ea`; placement is about left+25, top+401 in the
  English layout (128×32). The callback `0x4ce550` (`0x4cf04d`..`0x4cf064`) returns 3 for that button; the dispatcher at `0x48ef24`
  selects mode `0x13`, closes the mall, and mode `0x13` creates storage `0x495d20` with the inventory alongside (UI `+0x27c`).
- **Billing storage:** click a **type-1 merchant whose `NpcQuest.SData` extra byte is 9**, then choose its main service action (menu
  return 2). `0x48ce56`..`0x48ce7f` looks the merchant up with `0x4792e0`, tests record byte `+4 == 9` and selects mode `0x6e`, whose
  window constructor is `0x460060` (UI `+0x278`). In the client's plaintext SData the two records are 1/179 "Light's Bank Teller" and
  1/180 "Fury's Bank Teller". The extra byte is the merchant-only u8 after type and id, not NPC type 9.
- **Opening either sends no storage request.** `0x2606` and `0xB101` are receive-only list pushes and do not open the window; the
  constructors and mode-switch branches issue no network send (NPC clicking still sends its usual `0x0E04`). A server pushes both
  lists at entry (and updates when appropriate) rather than waiting for an open-window request.
- **Taking an item** is a drag from storage into the inventory: callbacks `0x4955f0` (at `0x4956b4`) and `0x45f800` (at `0x45f8c9`) call
  `0x5895a0`/`0x589540`, which send `0x2607`/`0xB102` with a `u8 storageSlot`, 3 bytes in all. The server chooses the target bag and slot
  and returns it in the 5-byte answer; neither request carries a destination.

**Live:** the store button, the faction's teller, then dragging one delivered item.

## Billing information `0xB103`

Reader `0x581d20` → logic `0x559930` (network vtable `+0x554`), **confirmed**. Seven bytes: `u16 0xB103, u8 billingType, u32 value`. For
region 6, `value` is a **packed local calendar date**, not Unix seconds, milliseconds or a duration:
`(year−2000)<<26 | month<<22 | day<<17 | hour<<12 | minute<<6 | second`. The handler extracts year, month and day only for the visible
`YYYY-MM-DD`, binds it to `<b>` and prints it in **chat category 7** once on receipt. It keeps the type at `0x21e55a4`; it does **not**
keep the expiry or start a countdown for region 6.

| Type | Message |
|---|---|
| 0 / outside 1..4 | no line (type still stored) |
| 1 | 814, individual package, includes expiry date |
| 2 | 815, franchise PC room, 1.5× EXP; no date |
| 3 | 816, combined individual/PC room, includes expiry date |
| 4 | 863, missing in the patch-108 `sysmsg-uni.txt` ("GetMsg Error 863") |

Region 2 (China) instead stores **seconds remaining** at `0x21e55a8`, divides by 86400/3600/60 and prints 813 (plus 834 under three
hours) in category 8; that countdown is irrelevant to region 6. A server sends type 0 for no package, and 1 or 3 only when that service
is meant. `0xB104` is a separate free-service packet. **Live:** the displayed line and date against the account's expiry.

## Auction rows: which fields are displayed

**Confirmed**, from the row copier `0x45acd0` (`TP_MARKET_ITEM`), the search renderer `0x458d20` (main dispatcher `0x45a660`, tab 0),
`0x456810` (my registrations and bids) and the selected-item detail `0x45cd50`. The layouts in `auction-house.md` stand. Row offsets
(48 bytes, excluding the opcode):

| Row offset | Field | Where used |
|---|---|---|
| +0 | u32 marketId | row selection, bid, buy, interest operations; shown as a "MarketID" line only under staff/debug flags |
| +4 | u32 tenderMoney | drawn as "Bid now : " (7041), formatted gold; initial/current bid |
| +8 | u32 directMoney | drawn as "Buy now : " (7042), formatted gold; buy-now confirmation |
| +12 | u32 endDate | drawn as remaining time by `0x454a30`; packed date, corrected against the server clock offset from `0x0114` |
| +16/+17 | u8 type/typeId | `Item.SData` lookup `0x460cd0`: item name, name colour/grade and icon |
| +18 | u16 quality | copied to entry `+4`, read by tooltip `0x49fa00`, mode 7 |
| +20 | u8 gems[6] | copied to entry `+6..+11`, read by that tooltip |
| +26 | u8 count | copied to entry `+3`; icon/count renderer `0x49dad0` |
| +27 | char craftName[21] | copied to entry `+12..+32`, read by tooltip `0x49fa00`, mode 7 (craft stats / enchant data) |

The 60-byte internal row keeps id at `+0x24`, date `+0x28`, tender `+0x30`, direct `+0x34`. Hovering sets the selected row at auction
`+0x193c`; `0x45a660` calls `0x4a3570` with tooltip mode 7 → `0x49fa00`, which reads quality, all six gems and the craft name. Quality,
gems and craft data are therefore meaningful although the compact list prints no columns for them. No seller or bidder name is in the
48-byte row. It is not safe for a server to zero any field except type, typeId and id.

The date formatter `0x454a30` shows minutes, hours or days remaining and no text once expired (`0x6a97e8` is its minute format).
`byResultType` exists only in the 41- and 53-byte collection rows; its meaning is outside this question. A server should keep all
fields populated as originally built.

## Client-side fixes touched by this work

- **GM question/request garbling:** the labels in `/UQGET` were CP949 Korean literals, not corrupt payloads. The companion DLL
  replaces the 16 labels at `0x6ab288`..`0x6ab350` with English by default (`fixes-dll.md`). No evidence supports changing the whole
  game's font or code page, or enabling feature 44 (region 6 does not create its alternate font). A garbled user-written question body,
  if seen, needs the actual bytes checked. **Live:** visible labels and body in the question window.
- **`0x2104` road messages:** implemented, both as a 5-byte byte patch (`0F B6 44 24 10` → `0F B7 44 24 08` at file offset `0x14FCD5`, VA
  `0x54fcd5`; see `client-patch-kit.md`, patch `road-messages`) and in the DLL (`road_messages=1` by default). **Confirmed (disasm)**
  against the original instruction. A server using link ids 1010/1011/1020/1021 sends the 6-byte `0x2104 u16 linkId, u8 old, u8 new`
  only for a road opening; `0x2102` stays the state update.
- **Region features:** the DLL's default set is 0, 5, 8, 23, 26, 43, 45, 46 and 62 (`fixes-dll.md`, `packet-opportunities.md`). Feature
  0 needs **8-byte `0x0802` requests and 34-byte replies**, including extraction without a hammer (stock: 4 and 33). Feature 26 needs
  `0x0608` for authoritative monster maxima; 46 uses `0x0216` and still needs its queued-action live check; 25 is redundant with the
  always-on 26. Feature 24 (on-screen login keyboard) stays optional. Not recommended: **6** (reduces warehouse capacity), **7/55** (change
  wear and durability semantics), **27** (changes friendly target attacks), **28/29** (need unused billing/login flows), **36/41/48–51**
  (need missing assets or configuration), **32** (unnecessary while the Teleportation Stone works). These recommendations come from the
  feature inventory, not from executed gameplay tests.

## Open

- `0x0513` buff and field hits: not examined; to be looked at only if a live test shows a wrong display.
- All **live** items above.
