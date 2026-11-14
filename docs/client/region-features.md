# Region features

A region feature is one entry in the client's built-in table of switches that decide, per game region, whether a part of the game is on or off. This page lists every switch in the US EP4.5 `game.exe` (built 2010-04-26, patch 108): which code asks for it, what it changes, and what it would need to be useful. It is for anyone who patches the client or wants to know why a feature of a later episode is missing here.

## In short

- **The client carries features it does not use.** One table (the [region features](glossary.md#region-feature)), built at start-up, says which of the numbered features are on for the player's region. This build is set to region 6 (English).
- **Switched off does not mean absent.** For many features the code is all there and only the switch is off. Flipping it with a client patch would add things like selectable lapis extraction, a target HP percentage, auto-dismount when a skill is used, and quick-slot swapping.
- **Others lack art files.** The world map, bump-mapped water, the skill tree and a few more need pictures or configuration files the client does not ship, so they would not work if switched on.
- **Nothing here was patched.** Every line is read from the code; whether a feature behaves as written in play is marked **live** where nobody has tried it.
- **A server can pair with some of them.** What a server would add for a given feature is on the [Shaiya Afterglow tab](afterglow/opportunities.md#region-feature-patches-a-server-can-pair-with).

### Using this page

- **If you patch or mod the client:** read the first table for what each switch does and what it needs. How to patch the table or one call site is in [Patch kit for game.exe](client-patch-kit.md); the companion DLL that carries run-time changes is in [The companion DLL](fixes-dll.md).
- **If you write a server:** the features that need a server packet are paired with it on [Opportunities the client already supports](afterglow/opportunities.md#region-feature-patches-a-server-can-pair-with).
- **To check a claim yourself:** start from the addresses in the "callers" column; the method is in [Tools and method](tools-and-method.md).

## How the table works

`IsOn(n)` is virtual slot `+4` of the feature object (`0x405c60`, vtable `0x6a5614`; object at `0x7013d4`, copy `0x0222ea44`). The table
below lists **every call through either pointer** (265 + 16 reads, 260 + 3 `IsOn` calls; disassembly scan of each read, then the decompile
around it; bounded: an indirect call through another copy of the pointer would be missed). On for region 6: 3, 4, 10, 11, 13–17, 19, 22,
30, 31, 33, 34, 37–40, 42, 54, 57, 61 (the [client's action notes](client-actions.md), §7). No callers: 2, 18, 20, 33.

A patch can either add region 6 to a feature's list in the constructor `0x405cd0` (one feature, every caller) or force a single call
site. **Nothing was patched.** "needs" = what the server or the archive must supply for the change to be useful.

## Features that are off for this client (candidates)

| n | callers (function@call) | flipping it on would | needs | value / risk |
|---|---|---|---|---|
| **0** | link window `0x493d30`@`0x494ab3`, @`0x493f95`; `0x0802` reader `0x57cb40` | **selectable lapis extraction** (below) | server: the 8-byte `0x0802` request and the 34-byte answer, which the original already handles and sends | **high**; every extraction then uses the 8-byte form |
| 1 | model render `0x428860`@`0x429118`, @`0x429212` | weapon-effect render pass without the video option (*inferred*) | — | low, graphics |
| 5 | `0x464ab0`@`0x464b23`, @`0x46680a` | confirmed: character creation, **Easy mode** button: asks first, "You can only attain Level 30 in Easy Mode. Are you sure…?" (5382), and selects Easy (description 5034) only after OK; off: selected at once | — | small; safe |
| 6 | warehouse `0x4fbd00`, `0x4fb8c0` | **limits the warehouse to 40 slots** unless "Double Warehouse" (812) | — | **negative**: do not flip |
| 7 | `0x4c1f80` (from the hit handlers `0x0502`/`0x0503`/`0x0511`/`0x0512`/`0x0517`/`0x0518`) | confirmed (disasm): when the local player lands a hit, the client's own copy of the **weapon's durability** (Quality of equipment slot 5, `0x84e7a6`) drops by 1 on normal hits *and* criticals (`bySuccess` 0–1); off: criticals only (1). Only Special-25 items refill it (none in patch-108 `Item.SData`) | the server's own durability updates | low; display only |
| 8 | death window `0x4c8bf0`@`0x4c90b6` | "You do not possess a Resurrection Rune." (838) on the revive window when you have none | — | small hint; safe |
| 9 | login sender `0x5867c0` | 33-byte password field (`0xA102` 67 bytes) | login server must accept it | none |
| 12 | `0x48aa00`, `0x48c120`@`0x48c13d`, `0x51e720` | confirmed: **windowed mode only** (`[VIDEO] FULLSCREEN=FALSE`): a 32-px title bar (`window.tga`, set up by `0x4fea00`) slides down when the cursor reaches the top edge, with minimize (`SC_MINIMIZE`), maximize/restore and a button that sends `0x0506` motion 1 (sit); full screen: the cursor is clamped to the screen height (`0x51e720`) | **`window_button.tga` missing** from both archives (buttons invisible) | low |
| 21 | resource loaders `0x435050`, `0x452270`, `0x5404d0`, `0x540600`, `0x543b00`, `0x570480`–`0x570e90` | confirmed: engine only: **ref-counted preloading**: each texture/model/effect is loaded as the map's resource list is read (`0x52ad00`, `0x53ed50`, `0x543ab0`: count 0 → load) and released at unload (`0x52ad30`, `0x53e820`, `0x543190`); off: map load takes the `0x444ca0` path (`0x452270`) | — | engine; load time / memory, needs testing |
| 23 | buff list `0x4b8220`@`0x4b82ec` | **"Continuous Resurrect Rune is left <b> minutes."** (861) for buffs whose skill record byte `+100` is 46 (`.`) | — | small; safe |
| 24 | login screen `0x4b1f70`, `0x4b2cb0`, `0x4b3470`, `0x4b61b0` | confirmed: an **on-screen keyboard** under the login box at ≥ 1024×768 (`0x505660`, layout `1234567890-=\`, Backspace, `qwerty…`, Shift via `0x505540`; `keyboard.tga`, `0x504fa0`): clicking keys types into the login fields, its Enter submits the login (saves `[LOGIN] ID`/`NOLTOSITE`, then `0x4b5060`, or `0xA105` when the feature-29 step is armed) | `keyboard.tga` in both archives | small (anti-keylogger); safe; needs a live look |
| **25, 26** | target window `0x4f8e50`@`0x4f9554`, @`0x4f957b` | **target HP percentage** under the target bar ("%.2f%%", `HP / max × 100`; 26 = always, 25 = only when `0x431a40` allows: it is true when the target mob's monster id (`+0x2c`) is 2382 or 2383, the two "Descended Goddess" monsters of `Monster.SData`, **confirmed** `0x431a40`) | `0x0608` (opportunity 2), or the bar's max is the file's | **high**, cheap; pairs with `0x0608` |
| **27** | action loop `0x43c000`@`0x44055f` | clicking any player in range **attacks** (sends `0x0502`) without the faction / duel / guild-war check | server must decide who may be hit (same-faction PvP zones) | medium; the server already refuses illegal attacks |
| 28 | `0xB104` handler `0x559cf0` | "3 hour-free service (time left)" (813/847) | `0xB104` | none |
| 29 | login flow `0x4b1a10`, `0x4b5060` | confirmed: a **second login step** (security card / code): `0xA104` (`char[15]` label, `u8 n`, n × `u16` cell numbers; `0x55c7b0`) arms it, the player types n numbers, `0xA105` sends `u8 n` + the bytes (`0x586870`), its answer (`0x55c720`) 1 = continue, 0 = wrong (10120 for tries 1–2), 2 = 10019. On, a successful login waits for that step when armed, and login result 4 shows 10122 "account is blocked" / 10019 / 10120 instead of 10025 | login server `0xA104`/`0xA105` | none; harmless without `0xA104` except the result-4 text |
| 32 | item use `0x4c7d40`@`0x4c7f6a` | Teleportation Stone: number of usable save points from the item (`+0x1a`) | — | the stone already works (the client's action notes, §1) |
| 35 | options `0x4ec7f0`, render `0x53b8a0`, `0x53c3a0` | graphics option slider with 4 steps instead of 2, and the matching render states | — | low |
| 36 | `0x4444d0`, `0x447d30` | confirmed: **bump-mapped reflective water** (shader `BumpWater_R.fx`, technique `BumpReflect`, `0x52a430`; needs vertex/pixel shader 2.0) with `data/Effect/wave.dds` and `data/sky/skycube.bmp` (`0x4444d0`); drawn by `0x447d30`@`0x44a0da` unless Left Shift is held | **`BumpWater_R.fx`, `wave.dds`, `skycube.bmp` missing** from both archives | not usable |
| 41 | map board `0x4b9f90`, `0x4ba470`, `0x4bb4f0`, `0x4bcde0` | confirmed: a **world map** mode in the map board ("Teos Epeiros", 5424): buttons `wm_button_m`/`_w` switch map ↔ world (`wm_map.tga`, `wm_letter.tga`); 19 regions from `data/WorldMap.cfg` (`count`, `WorldMap<n>`: `Country`, `MapNum`, `Click_left/top/right/bottom`, `ImgPos_x/y`; `0x4bc1a0`) highlight on hover (`wm_e<map>`/`wm_f<map>.tga`); clicking one shows that map's `MiniMap/<map>.tga` and name (`0x4b9960`), the other faction's maps show `wm_warning.tga` | **`WorldMap.cfg` and every `wm_*.tga` missing** from both archives (the 22 `MiniMap` images are there); without the cfg the clicks do nothing | medium if the art and cfg are made |
| 43 | quick bar `0x4d4b30` | confirmed: a **brand-new character** (level 1, `0x84d684` 0 (*inferred* exp), every quick slot empty) gets two default quick slots: basic actions 5 and 4 (bag 101; the T and Z key actions) | — | small; safe |
| 44 | text `0x41e2d0`, chat `0x46f4c0` | confirmed: chat and wrapped text use the alternate font `+0x3a4` (`0x52ef90`, `0x530990`), which `0x5315c0` creates only for region 7 and the later European branches (17-px Arial); **region 6 never creates it**, so nothing changes | — | none |
| **45** | options `0x4e8740`, `0x4e8af0`, `0x4eb450`, `0x4ebac0`, `0x4ed990` | an options checkbox **"lock the skill window"** (`[VIDEO] SKILLWNDLOCK`) | — | small QoL, safe |
| **46** | quick slots `0x4d4e10`, skill bar `0x4db300`, skill window `0x4f12c0`, resume `0x416ba0` | **auto-dismount**: using a skill or quick slot while riding sends `0x0216` (get off) and queues the action (`0x21e8988`–`0x21e8994`); **confirmed** that the queue is replayed after the answer: `0x416ba0` (the entity's dismount clean-up, run when its state becomes 0) calls `0x4c49c0` when feature 46 is on, which re-runs the queued quick slot, skill-bar slot or skill-window skill and clears the pending flag `0x21e8988` (kind `0x21e898c`: 0 quick slot, 1 skill bar, 2 skill window) | the original already answers `0x0216` | **medium-high** QoL |
| 47 | lists `0x4dca00`, `0x4dd0d0` | a list shown newest-first (index reversed) | — | low |
| 48 | `0x427f50`, `0x428430`, inventory `0x4e6250`, ground items `0x55acb0` | item 44/246 dropped as a special ground model (`drop_item004.3DO/.DDS`), allowed to be dropped, ≤ 99 kept | **`drop_item004` missing** from both archives | not usable |
| 49 | `0x49d760`, `0x49d9b0` | an icon outline texture (`interface/icon/Icon_stroke.dds`) | **missing** | not usable |
| 50 | country select `0x47b0a0` | `select_country_bg3.jpg` and the two factions' sides swapped | **missing** | not usable |
| 51 | skill window `0x4f0ff0`, `0x4f1630`, `0x4f2700` | a **skill tree** button (`skilltree_.tga`) and window (`0x424100`) for one job (`0x84fa22 == 2`) | **`skilltree_.tga` missing**; window contents unknown | not usable without art |
| 52 | login screen `0x4b1f70`, `0x4b2cb0`, `0x4b5060`, `0x4b61b0` | two extra checkboxes (`[LOGIN] NOLTOSITE`) | — | none |
| 53 | `0x4be6b0` | a slider/scroll value moves the other way | — | low |
| 55 | `Item.SData` loader `0x460500` | confirmed: **doubles every weapon's max durability** (Quality, file offset +37 → record `+0x2c`) at load; types 1–15 and 45–65 are exactly the weapon banks (1200/1700 → 2400/3400) | server must use the same doubled values | **do not flip**: client and server durability would disagree |
| 56 | `0x482c40` | a list width of 400 instead of 200 | — | low |
| 58 | `0x4c2bb0` (22 sites) | confirmed: the built-in **guild-house NPC upgrade table** (36 × 16 bytes at `0x199abc0`; per NPC (description 8011 Gate keeper, 8012 weapon merchant, …) and level 0–3) **doubles both costs** (`+8`, `+0xa`: e.g. Gate keeper 1000/1500/2000 + 200/500/800 → 2000/3000/4000 + 400/1000/1600; `+8` = the Etin cost of an upgrade and `+0xa` the keep Etin: *inferred*, same numbers as the server's built-in table, **ref** [Guild houses](../original/guild-house.md#2-guildhouseini)) | — | low: display and client pre-check only (`0x0D25` carries no cost; the server charges) |
| 59 | system menu `0x489ff0`, `0x48c120` | skips the exit/character-select dialog 0x12 | — | low |
| 60 | debug text `0x52fe20` | the coordinate overlay text replaced by a centred string | — | none |
| **62** | quick slots `0x4d43c0`, `0x4d5540` | **dragging a quick slot onto another swaps them** (confirmed `0x4d43c0`; off: move and clear) | — | small QoL, safe |


## Features that are on (flipping them off would remove)

| n | what it gates (callers) |
|---|---|
| 3 | item-mall window layout (`ins_market.tga`, `ins_market_past.tga` widths) (`0x4cd7f0`, `0x4ce550`, `0x4cf0c0`) |
| 4 | weapon effects with the option (`0x428860`), video options entries (`0x4e8740`, `0x4e8af0`, `0x4e9730`, `0x4eb450`, `0x4ebac0`, `0x4ed990`) |
| 10 | sends `0xB106` after a scene set-up (`0x4248e0`) |
| 11 | colour/link markers on item names in messages and the AH (`0x49fa00`, `0x4f77c0`, `0x46f4c0`, `0x55a670`, `0x55d490`, `0x55f2e0`) |
| 13 | chat: a sixth tab and the `~ ` prefix / key for chat type 5 (`0x46f4c0`, `0x475e00`) |
| 14, 15 | login and character-select scene (`login2.wld`, camera, panels) (`0x447d30`, `0x4b17f0`–`0x4b61b0`, `0x504bc0`) |
| 16 | map loading bitmap (`0x452270`) |
| 17 | `0x0207` with two `u32` (`0x57a380`) |
| 19 | render states (`0x428040`, `0x428860`, `0x447d30`, `0x468c00`) |
| 22 | Movement Rune falls back to the party window's member (`0x4c7c00`, `0x4c7d40`) |
| 30 | chat input commands and a bar (`0x475e00`, `0x5010f0`, `0x502750`, `0x502aa0`, `0x503f90`, `0x504400`) |
| 31 | a list walk in `0x4c5470` |
| 34 | loading-screen images (`0x4b0cc0`) |
| 37 | the auction house (35 sites: AH window `0x456810`–`0x45d535`, tooltips `0x49fa00`, `0x4af1f0`, `0x4b0020`, `0x4e7580`, `0x4ff810`, `0x4ffff0`) |
| 38 | weather sound and sky (`Rain.wav`; `0x42ca20`, `0x4354f0`, `0x4516f0`, `0x447d30`, `0x539b40`–`0x53a040`) |
| 39 | an options default (`0x4eaa10`) |
| 40 | the "[Selected Item]" block in tooltips (`0x49fa00`, `0x4a3570`) |
| 42 | `0x41cee0` |
| 54 | start-up (`0x40a6f0`) |
| 57 | a UI size (200) (`0x49d760`) |
| 61 | list sorting from `SORT_LIST.INI` (`0x4dd620`, `0x4dca00`, `0x4dd0d0`) |

Region checks in packet code (the readers/senders and their handlers): `0x0207` reader (17), `0x0802` reader (0), login sender (9), the
`0xB106` request (10, in a scene function) and the `0xB104` handler (28). The link window chooses the `0x0802` request form (0).


## Feature 0 in full (selectable lapis extraction)

Everything feature 0 controls (confirmed; the three call sites above, plus the code only reachable through them):

1. **Which extra items the link window accepts** (`0x493d30`@`0x493f95`, item record `+0x2a` = Special). Off: Special 36 (Linking Hammer)
   and 69 as the "extra" item of a link. On: 36 → mode 1 (link with hammer), 69 → mode 1 with a flag (`+0x242c`, used by the rate
   display), **58 → mode 2 (extraction with a chosen gem)**.
2. **The extraction request** (`0x494ab3`). Off: always `0x0802` **4 bytes** (`u8 bag, slot`, `0x586540`). On: always the **8-byte form**
   (`0x586580`): mode 2 → `u8 bag, slot, 1, gemPos (0-based), hammerBag, hammerSlot`; any other mode → `u8 bag, slot, 0, 0, 0, 0`. Both
   on-forms are sent only while an NPC is selected (the trailing NPC-id argument is not on the wire).
3. **Choosing the gem** (mode 2 only, `0x493360`/`0x492f60`): clicking one of the item's gems selects it (`+0x2428` = 1–6); the other gems
   are drawn dimmed (`0xff333333`), and the success-rate text is computed for the chosen gem only.
4. **The answer** (`0x57cb40`). Off: 33 bytes → `0x558510` (vt `+0x1c8`), which clears **all six** gems of the item and places each lapis
   whose count is non-zero. On: 34 bytes with `gemPos` → `0x558700` (vt `+0x1c4`): `gemPos` 255 clears all, otherwise **only that gem**;
   then the same placement. Messages 57/58, `ui_rapis001/002.wav`.

Assets: the link window loads `interface/rapis.tga`, `rapis_button.tga`, `rapis_button2.tga` unconditionally (`0x492aa0`); all three are in
`data.saf` (and the original archive). The gem choice adds **no texture** of its own: it reuses the item icons with a tint. Nothing is
missing.

Items with **Special 58** in this client's `Item.SData` (patch 108): **100/140 Small Extraction Hammer**, **100/141 Large Extraction
hammer** (both stack 255, price 0, "extract Lapis with a fair / an excellent success rate"), **44/237 GM Extraction Hammer** and
**100/204** (Korean name, GM) (stack 10). Special 36: the Linking Hammers 100/49, 100/195–199, 100/193–194, 44/193–199, Operator's
Exclusive 100/82. Special 69: none.


Server side (**ref** [Lapis and enchanting](../original/lapis.md#4-extraction-0x0802)): the original handles the 8-byte form
(`ItemGemRemovePos`: Extraction Hammer = real type 26, special 58, one used up, success when `rand() % 100 < reqvg`) and always answers 34
bytes. So **with feature 0 on, the client and the original server agree as they are**; with it off, this client reads one byte fewer than the
original sends (33 against 34; see [The extraction answer](packet-opportunities.md#the-extraction-answer)). Risk of the patch: none known beyond
the request form changing for all extractions; needs one **live** try.

## Open

- **Live** (nothing was run): feature 0 flipped (8-byte request, 34-byte answer, the gem choice); every other feature's visible effect.
- Every off feature has an effect line. The ones worth a patch with no new art: **24** (on-screen login keyboard) and the small QoL ones
  (5, 8, 23, 43, 45, 46, 62). **41** (world map) needs `WorldMap.cfg` and the `wm_*.tga` set authored. Live checks not done for any of them.
- Not settled: which of feature 12's buttons sits where (one sends sit, `0x0506` motion 1); feature 43's `0x84d684` (inferred exp); whether feature 43's default slots reach the server.
