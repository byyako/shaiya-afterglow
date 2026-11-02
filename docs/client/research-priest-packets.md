# The Priest packets 0x0F01 and 0x0F02

This page answers which NPC sends `0x0F01` and `0x0F02` and what the client does around them. It was read statically in the US `game.exe`
(build 2010-04-26, patch 108) with Ghidra, plus raw byte scans of the file and the client's `NpcQuest.SData`, `PriestTalk.SData`,
`sysmsg-uni.txt` and `CONFIG.INI`. Status words: **confirmed** (read in the code or data), **ref** (the same in the 2012 Hex-Rays listing of a
later client), *inferred*, **live** (needs a test). Packet logic is in network-vtable functions (`0x6A5C20` + slot); readers are at `0x57xxxx`.
The open points are also tracked in `re-open-items.md`.

## In short

- **The sender is NPC type 14, not type 4.** Type 4 is the "Excellent Warrior manage" NPC; its button sends `0x0218`, not a Priest packet.
- **The client has no type-14 NPC.** `NpcQuest.SData` has 13 type sections and the loader fills types 3–13, so in normal play nothing in
  this client can send `0x0F01`.
- **`0x0F02` is a paid "reading":** 10 gold, sent from the window that a `0x0F01` reply with `u8 1` opens, one of four buttons, after a
  confirm box that names the price. The client requires 10 gold or more; the original server needs more than 10.
- **The text that window would show is never loaded.** The lookup code and an empty table exist, but nothing fills the table, so the lookup
  reads through a null pointer: a `0x0F02` reply with `u8 1` should crash the client (*inferred*, strong; **live** to confirm).
- **For a server:** no player of this client can reach the Priest legitimately. Never answer `0x0F02` with `u8 1`.

## Who calls the two senders (confirmed)

- The sender `0x586d50` (`0x0F01`, 2 bytes) has **one caller**: `0x48d0f2` in the NPC menu function `0x48c120` (dialog mode 100), `switch
  (npcType)` **case 14**, menu result 2 (the type button, text 5296 "decided to consult"). There is no gold or distance check at the send;
  after any menu result the menu closes, so one click sends once.
- The sender `0x586d80` (`0x0F02`, 2 bytes) has **two callers**, both in `0x4d0920`, the click handler of the Priest window (dialog `0x69`,
  vtable `0x6acb34` slot 1): `0x4d0b21` (direct) and `0x4d09e2` (the confirm box answered OK).
- The opcodes `0xf01`/`0xf02` appear as immediates only in those two senders and in the receive dispatcher `0x582e10` (`0x583de2`,
  `0x583eaa`). No item, chat command or other window sends them.
- The menu's type byte comes from the clicked NPC object (`+0x60`, set by `0x0E01` `byType`), copied to `0x854eac` at `0x44161b` when the
  walk-to-NPC finishes in `0x43c000`.
- **NPC type 4** (`case 4`, result 2, `0x48d15b`) sends **`0x0218`** (`0x586db0`) and, when the kill count `0x84d678` is below 16 or flag
  `0x84fa1e` is 0, also opens dialog `0x6c`.
- Menu labels (`0x4fe0b0`): button 1 is always 5293 "talk"; button 2 by type: 1 "trade" 5294, 2 "decided to move" 5300, 3 "repair weapon"
  5295, **4 "Excellent Warrior manage" 5303**, 5 "Betting" 5302, 6 "Manage warehouse" 5299, 11 "Manage guild" 5301, 13 "Skill manage" 5298,
  **14 "decided to consult" 5296**, others "OK" 5304.

Every path to `0x0F01` needs an NPC of type 14 in `0x0E01`. The record lookup `0x479320` accepts types 3–16 but only types 3–13 are filled
(loader `0x4796c0`, a loop over 11 types at `0x479dfc`–`0x479fe9`; counts start at 0 in the constructor `0x4c4be0`), and the NPC-creation
helpers (`0x433770` name and model, `0x434bc0` marks, `0x5254e0` minimap) switch on types 1–13 only. A type-14 NPC sent by a server would be
created with an empty name and default model, and its menu gets no record, so `0x4fe0b0` is never called and the buttons get no labels.
Whether such a menu can still be clicked to send `0x0F01` is **live**. The second route to the Priest window is an unsolicited `0x0F01 u8 1`
(see below).

## The type-4 NPCs as the client knows them (confirmed)

`NpcQuest.SData` holds exactly two type-4 records, the same two a server places: 4/1 **Macpallan** (welcome text "I supposed you are here about
a reward?", model 23, faction 1) and 4/2 **Preedon** ("Did you come here for some sort of compensation?", model 32, faction 2). Their button
2 is "Excellent Warrior manage" and sends `0x0218`. There are no type 14, 15 or 16 records at all: the file has 13 type sections. The Korean
title 공로관리관 ("merit manager") fits a kill-reward NPC, not a priest.

## The window and its costs (confirmed)

- **`0x0F01`** has no price shown anywhere and no gold check in the client.
- **The Priest window (dialog `0x69`)**, built by `0x4d01d0`, has the background `data/interface/Priest.TGA` (256×512), four option buttons
  whose labels are **Korean strings hard-coded in `game.exe`** (not in sysmsg, so the US client has no English for them), two sheet buttons
  확인 "OK" (`0x6aca9c`) and 취소 "Cancel" (`0x6ab31c`), a text box with a scroll bar, and a close box. The four buttons ask for a reading of:

| Button | Korean | English | Table | Value used |
|---|---|---|---|---|
| 1 | 전사로서의 자질 | qualities as a warrior | 0 | **HG**, low byte (`0x84d6d4`) |
| 2 | 수장으로서의 자질 | qualities as a leader | 2 | **CG** (`0x84d6d8`) |
| 3 | 비겁함과 정당함의 판별 | telling cowardice from righteousness | 1 | **VG**, low byte (`0x84d6d6`) |
| 4 | 동료들과의 협동 | cooperation with companions | 3 | **OG** (`0x84d6d9`) |

- **`0x0F02`** is a click on one of the four buttons (`0x4d0920`). The client requires **gold of at least 10** (`CMP [0x84d670], 0xa; JC` at
  `0x4d0ab1`). Below 10 nothing happens and no message appears. Otherwise it remembers the button (`+0x31cd`, 0–3) and, if `CONFIG.INI`
  `[INTERFACE] SHOW_MESSAGE_PRIEST` is missing, empty or `TRUE` (`0x4d06f0`), opens a confirm box (`0x516770` case 5) with **363** ("Will only
  have 10 gold for performing sacrificial rite to godness", Light, `0x21e4942` = 0) or **364** ("Will let you know your tendency if you give
  10 gold.", Fury); OK sends `0x0F02`. The box has a "don't show again" check that writes the key (`0x515b90`); any other value sends at
  once. **The chosen button is not sent,** so a server cannot know which of the four readings was asked. (The key's file is `CONFIG.INI`:
  the same path variable `0x7013e0` also gets `[USER] SHOW_INTERFACE`, *inferred*.)
- The client's gate is **10 or more** while the original server needs **more than 10**; with exactly 10 gold the client sends and the server
  answers `u8 0` → 303. **Each click sends one `0x0F02`** with no lock while waiting, so several clicks are several charges.
- **The `0x0F02` reply** (`0x54f760`, slot `0x2ac`) with `u8 1` only sets the flag `0x21e4938` = 1; any other value shows 303 "Insufficient
  gold" (`0x41e8f0(0x11, 303)`). The window's per-frame handler then calls `0x4d0770`: it copies the button's Korean title into the window and
  asks `0x479390(category, value, country)` (ECX = the NPC data object `0x854f1c`) for the text to show.

## The text that is never loaded (confirmed; crash *inferred*)

`PriestTalk.SData` is the intended source, but nothing loads it. `0x479390` reads four slots `{count, entries}` at `+0x198008C` of the NPC
data object; each entry is 12 bytes `{u8 threshold, char *light, char *fury}`. The file has exactly that shape: 4 sections, each `u32 count`
then `count × {u8 threshold, u32 len + Light text, u32 len + Fury text}` (cp949), 86 entries in all, ending exactly at the file size, with
section topics that match the categories (0 warrior, 1 cowardice, 2 leader, 3 cooperation). But **no code fills the slots**: the only
references to `+0x198008C`/`+0x1980090` (and the absolute `0x21d4fa8`…) are the lookup, the constructor (which zeroes them, `0x543150`) and the
destructor, and `game.exe` has no "PriestTalk" or "Talk" string. The 2012 client (**ref**) has the same reads and no loader either.

With count 0 and a null pointer, `0x479390` returns `*(NULL + 4 + country*4)` for every category and value: an access violation at address 4
or 8. *Inferred* (static, strong): **a `0x0F02 u8 1` reply crashes the client** as soon as the Priest window processes it. How an entry *would*
be picked, for completeness: table 0 (HG): value 0 → entry 0, otherwise the last entry from entry 1 on whose threshold is at least the value
(thresholds 210, 181 … 2, 1; a rank, low is better; above 210 falls back to entry 0); tables 1–3: walk while the threshold is at most the
value, result = that count − 1, but a count of 1 stays 1 (an off-by-one); Light or Fury text by the player's country. Never reachable.

## What HG, VG, CG, OG and IG do

- **No number is ever drawn.** The window's draw function `0x4d0b60` draws the buttons, the chosen title and the text box only; nothing
  formats HG, VG, CG, OG or IG.
- The numbers are **selectors** for the unloaded texts, as u8: HG and VG are cut to their **low byte** (`MOV AL,[0x84d6d4]` at `0x4d079d`,
  `MOV CL,[0x84d6d6]` at `0x4d0819`), so HG 300 reads as 44 and a **negative VG reads as 256 + VG** (−1 → 255); CG and OG are bytes. **IG is
  stored (`0x84d6da`) and never read.**
- The reply handler `0x54f700` (slot `0x2a8`) stores all five in the character block, the same variables the GM status packet `0xF701` sets
  as types 11–15 (`0x554030`): HG `0x84d6d4`, VG `0x84d6d6`, CG `0x84d6d8`, OG `0x84d6d9`, IG `0x84d6da`. **VG's variable is also the
  "Blessing of Goddess" lapis value** that `0x0555` writes (`0x5580a0`) and the lapis window reads as signed (`0x492f60`), so a Priest
  reading overwrites it until the next `0x0555`.
- The four button texts are Korean bytes in the US client's font; what a player actually sees is **live**.

## Client-side checks

- **Walking away:** every dialog mode of 100 or more (the NPC menu and the Priest window included) closes when the player is **more than 3.0
  units** from the NPC whose id is in `0x850a60` (`0x4433df`–`0x443521`, constant `0x6a67f8` = 3.0f); the check is skipped when that NPC is
  not in the client's list (`0x4479a0` returns 0).
- **Map change:** the zone-loading path in `0x42cc10` sets the dialog mode to 0 (`0x42f14d`, after the loading screen); that it runs on every
  teleport is *inferred*. **Death:** `0x48c120` closes every dialog when the player's state byte `+0x279` is 2 (`0x48c16e`); that 2 means
  dead is *inferred* (**live**). **Escape** (key test `0x426760`) closes the confirm box if open, else the window (*inferred*).
- **Window closed:** `0x0F02` cannot be sent, because its callers run only while the mode is `0x69` and the window is visible. The client
  never checks the NPC's type, the map or the distance when sending `0x0F02`, only the window's existence (and its 3.0 walk-away close).

## Replies the client did not ask for

- **`0x0F01 u8 1`** (`0x54f700`) stores the five values and sets the dialog mode to `0x69` **unconditionally**; it is the only place that
  opens that window (a single `MOV [0x850a5c], 0x69` in the file). It replaces whatever dialog was open, and the window appears **anywhere**:
  if the last clicked NPC is not in view, the walk-away close never fires, so the window stays until the player closes it, dies or changes
  map, and its buttons send `0x0F02` from anywhere.
- **`0x0F01 u8 0`** or **`0x0F02 u8 0`** show 303 "Insufficient gold" in every state.
- **`0x0F02 u8 1`** sets the pending flag `0x21e4938`; nothing is visible while the window is closed, and closing, death or a map change do
  **not** clear the flag. The next time the window is open, `0x4d0770` runs the text lookup → the crash above.

## For a server

1. A "Priest NPC in view" check would have to require an NPC of **type 14**; none exists in the client data or in the original world, so it
   equals "always refuse". Type 4 must not be the check: its button sends `0x0218`.
2. Never answer `0x0F02` with `u8 1` to this client (crash, *inferred*). Answering `u8 0` shows a misleading "Insufficient gold"; ignoring
   the request is silent and safe, because the client waits for nothing.
3. If the original behaviour is kept, note the gold mismatch (client 10 or more, server more than 10) and that each click is a separate
   charge.

## Open

| Point | Cheapest check |
|---|---|
| A `0x0F02 u8 1` crashes the client | **live**, on a test client with a debug hook that opens the window with a `0x0F01 u8 1`: click button 1 with at least 10 gold, press OK, answer `0x0F02 u8 1`; expected an access violation at address 4 or 8 |
| What a US player sees on the four Korean buttons | the same run, a screenshot before clicking |
| Whether a type-14 NPC is clickable and its unlabelled menu sends `0x0F01` | spawn one next to a test character, click it, press the second button, look for `0x0F01` in the packet log |
| State `+0x279` = 2 is death; Escape closes the window | the same window-open run |
| Exactly 10 gold: client sends, server answers `u8 0` → 303 | the same run with 10 gold |
