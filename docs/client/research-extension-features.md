# What the client allows for extension features

This page collects what the US `game.exe` (build 2010-04-26, patch 108) does and does not allow for server-side and data-side extensions:
input ranges, the colour-marker parser, ground items, map pictures, small monsters as pets, channelled actions, faction rules, ids and
ceilings, what a signed patch can carry, effects, motions, banners, and what a DLL overlay would need. It was read statically
(Ghidra, read-only). Points that still need a game session or more reading are listed at the end and in `re-open-items.md`.
Status words: **confirmed** (read in the code or data), *inferred*, **live** (needs a test). Packet handlers are network-vtable functions
(`0x6A5C20` + slot), readers are `0x57xxxx`.

## In short

- **Check what you send:** hair, face and size are bounded by lists; **race above 3 or sex above 1 are not checked** and index tables out of
  bounds; shop-sign and chat text must stay inside fixed buffers.
- **Text in names, signs, balloons and chat goes through a `<<NNNcCC` colour-marker parser** in two places, so any text a server relays
  must be kept free of markers (the one crash the site writes out, with its filter, is in `client-packets.md`).
- **A ground item sent to only some players is fine:** the client keeps only items it was told about, and an unsolicited pick-up answer
  looks exactly like a real one.
- **Small monsters can serve as pets, scenery or campfires** with data only: scale classes are fixed, "size 0" makes a monster unclickable,
  and max HP must be at least 1.
- **The client scrambles the other faction's chat itself** and gates its player menu by faction; almost every other cross-faction rule is
  the server's.
- **No combat level rule exists in the client;** levels only colour names and gate item use, equipping and a few NPCs.
- **New effects, skill visuals and UI windows need the companion DLL;** new items, skills, monsters and buff icons are data.

## Input ranges

**Hair, face, size** (confirmed). Creation (`0x0102`, sender `0x5857c0`, built at `0x46547e` in `0x464ab0`): `u8 slot, race, mode, hair,
face, height, job, sex, name`. The screen cycles hair 0–4, face 0–4 and height 0–4 (wraps at `0x46670a`–`0x4667d5`) for every race and
sex; height starts at 2; race and sex come from the faction buttons (Light: race 0/1, Fury: race 2/3). Appearance change (`0x0226`, 8
bytes, sender `0x5861e0`, window `0x4630c0`): `u8 bag, slot, hair, face, size, sex`; hair and face cycle over the record count of the part
list `data/Character/<race>/<prefix>_hair.MLT` / `_face.MLT` (tables `0x21f4ff4`/`0x21f4fb4`, loader `0x540970`): **14 hair records (0–13)
and 8 face records (0–7)** for all 4 races × 2 sexes × 2 rigs (32 files checked); size 0–4; race cannot change.

What a viewer does with other values: `0x0303` (logic `0x54d370`), the `0x0226` answer (`0x557cb0`) and the list (`0x54cbc0`, slot < 5 only)
store everything unchecked, then `0x415ec0`/`0x415310` build the model. Hair or face out of range is **ignored safely** (`0x415310` checks
`0 <= idx < count` and a null entry). Size above 4 gives scale 1.0 (switch `0x413d29`: 0.96, 0.98, 1.00, 1.02, 1.04). **Race above 3 or sex
above 1 are not checked**: they index the part and skeleton tables (`0x21f4cac`, `0x21f4eac`, `0x21f5aa8`, stride `race*2+sex`) out of
bounds; *inferred* wrong model or crash. A server therefore sends race 0–3, sex 0–1, hair 0–13 (creation 0–4), face 0–7 (creation 0–4),
size 0–4. Whether a `0x0303` with race 4 crashes is a **live** question; a test would crash viewers, so keep the strict server check.

**Shop sign** (confirmed). `0x0523` (reader `0x5807c0`) reads `u8 len` and then `len` bytes into a 124-byte stack buffer: an oversized
length overwrites the return address and **crashes every viewer**. The logic `0x559080` copies the text to player `+0x2f4` (128 bytes), so
the text needs a NUL within 124 bytes; send a length of 121 or less (the original caps the title at 120 plus NUL). The sign is drawn by
`0x41b1e0` → `0x5373b0`, which parses colour markers (below). Typing uses the shared text input `0x21ea51c` with a maximum of 60 characters
(`0x51f440(0x3c, …)` at `0x4de939`) and no character filter; `0x2303` sends `u8 strlen+1, text`.

## Where the colour marker is parsed

Only two functions parse `<<NNNcCC` and copy NNN bytes (every `atol` caller that also tests for `'c'` was checked, **confirmed**):

1. **Chat window draw `0x46f4c0`** (vtable `0x6a6b50`) copies into a 272-byte stack buffer. `CC` indexes a 10-colour table unchecked (a wrong
   colour only). Every chat-log line goes through it: chat, system lines (`0x41e8f0`), notices (`0x41e2d0`).
2. **Over-head text textures `0x5373b0`** copy into a 260-byte stack buffer. They draw player names (`+0xf0`, from the 21 bytes of `0x0303`),
   **guild names** (`+0x2c4`, 25 bytes), **chat balloons** (`0x410460`, 5 s, for `0x1101`/`0x1107`/`0xF101`), shop signs, monster names, the
   target name and ground-item labels (`0x433c90`).

An over-long marker therefore crashes viewers from a chat line, a name, a guild name or a sign; **names and relayed text must be kept
marker-free by the server.** `0x41bec0` only carries an 8-byte marker to the next wrapped line (no copy). Not parsed (plain draw, *inferred*
because their callers never reach the two functions above): the friend and block memo column, the guild window's remark and names, mall
and auction texts. Notices `0xF106` (sysmsg 356, chat type 8) and `0xF906`–`0xF90B` (`ui_news001.wav` + 356, type `0x18`) are chat-log lines,
so they are parsed (`0x5533a0`, `0x553de0`); no separate banner exists for them.

Everything is a C string: readers copy `len` bytes, handlers use `%s`/`strcpy`, the parser uses `strlen`, so text after the first NUL is never
seen. Reader buffer sizes a server must stay within (confirmed): `0x1101`/`0x1105`/`0xF101`/`0xF105` 132 bytes, `0x2206`/`0x220B` memo 51,
`0xF106` 2004 (then `strcpy` into a 1544-byte global at `0x702478`: keep it under about 1500), `0xF906`–`0xF90B` 1032, `0x0D20` fixed 65,
`0x0523` 124.

## Ground items

All **confirmed**. `0x0401` (logic `0x55acb0`): `u32 id, u8 kind, u8 type, u8 typeId, u8 count, f32 x, y, z`. There is **no owner, party or
faction field**; `kind` is only the fall height (0, 0.1, 1, 2). An unknown id is created, a known id ignored; y = 0 means terrain height
(`0x44e8f0`). The client keeps only items it was sent, so **a `0x0401` that only some players receive is fine**.

Pick-up: a click (target kind 1) or the pick-up key (`0x21e546c = 4`, `0x43c55c`) picks the **nearest known item within 30.0** (`0x6a6648`),
walks to it and sends **`0x0205 u32 id` at 2-D distance 1.0 or less** (`0x43f8e1` case 1). There is no owner check; a client only ever
requests items it was shown. The answer `0x0205` (`0x55a670`): error `u8 0, u8 code` → a message (0 "This doesn't belong to you" …, 3
silent); success writes the item to its bag and slot, plays `ch_pikup001.wav` and prints 52 "Attained <i>" (gold 53). There is **no pending
request check**: an unsolicited `0x0205` is shown the same way (this is how a pet's pick-up can look like the player's). It does not remove
the ground item: only `0x0402` (`0x557690`) or leaving view does, and a refused pick-up leaves the item visible.

Slot writes without "Attained": `0x0204` move results (`0x557500`…), `0x0206`, quest reward `0x0903` (own messages), shop `0x0702` (prints 52
too), mall moves `0x2607`/`0xB102`; *inferred* `0x0204` is the silent way to fill a slot. The ground object keeps only `type`, position and fall
height (`0x55ade3`–`0x55adf9`); the model is entry `+8` (0) of the type's ground list (`0x428040`, tables `0x21f5cec + type*0x1c`), so
*inferred* every item of a type looks the same on the ground, and a different model per type is a data change in its `.ITM` list. No flag
makes an item unclickable or unpickable; that would need the DLL.

The item **44/246** is a Korean memorial "offer flowers" object: with region feature 48 on, dropping it inside a type-5 area of
`TacticsZone.zon` asks "헌화를 하시겠습니까?" (literal at `0x6ad3c8`; `0x4e6b20`–`0x4e6bbd`). Feature 48 is off here and the zone file has no
type-5 area. With feature 48 a counter allows 100 such objects and then removes the oldest (`0x55ad40`–`0x55adaa`); otherwise there is no cap.

## Map pictures

The map board `0x4b9210` loads `data/interface/MiniMap/<id>.tga` for 24 ids (maps 68, 69 and 70 have no file and no `.wld`); the corner
minimap `0x4be1c0` loads the same file for the current map. All are 512 × 512 (24- or 32-bit bottom-up TGA). Placement: the board
(`0x4baa5a`) uses px = x / mapSize × 512 and py = (mapSize − z) / mapSize × 512; the minimap (`0x4bee6d`) uses u = x / mapSize and
v = −z / mapSize. `mapSize` is world `+0x1400e4` (`0x845284`), the i32 after `FLD` in `world/<id>.wld` (1024 or 2048; map 70 is forced
to 1600). Map names come from `0x4c9100` (`CPlayerData::GetMapName`, map id → sysmsg); ids without a case (37–41, 64–67) show the number.
Maps 102–108 are all "Battle Zone", and their pictures repeat others (102, 105 and 108 equal 18; 103 equals 30). **Confirmed.**

## Small monsters as pets, scenery and campfires

`Monster.SData` in memory (loader `0x460e20`, `0x2c` bytes per record): `+0` name, `+4` model u16, `+6` level, `+8` ai, `+0xC` hp, `+0x10` the byte
a data viewer calls "Day", `+0x11` "Size", `+0x14/+0x18` normal time and step, `+0x1C/+0x20` chase time and step, `+0x21..` attacks, `+0x28`
quest item. **Confirmed:**

- **Draw scale is the "Day" byte, not "Size"** (`0x4322e0`): 0→0.6, 1→1.0, 2→1.3, 3→1.6, 4→2.0, 5→5.0, 6→0.4, other 1.0 (it also scales the
  `.MON` attachments, `0x432910`). Scale classes are discrete; a per-monster scale needs the DLL (redirect the scale call `0x40e910` inside
  `0x4322e0`). **Live:** a record with Day 6 spawned in game.
- **Count:** u16 in the file (2,474 now; at most 65,535). `0x461210` returns null for `wMOBID >= count` (no model, no name). Records may share
  a model (the model index into `monster.MON` is also bounds-checked, `0x432e80`).
- **Picking** (`0x43b920`) is a camera-facing quad with **width = Size** (±0.5 × Size) and height = scale × model height + a constant.
  **Size 0 makes a monster impossible to click or target**; dead monsters are skipped.
- **Any clickable monster is attacked:** action case 3 walks into range (Size − 0.3 + weapon range) and sends `0x0503`, with no AI, faction or
  level test. Name and target colour depend on the level difference only (`0x4c21a0`). A "friendly" monster is one with Size 0.
- **Name** is the record name (`0x461210`), drawn by `0x5373b0`; no packet gives a per-object name (only the DLL could, by hooking the name
  source in `0x447d30` at `0x44c2fd`).
- **Target bar** (`0x4f8e50`) shows min(HP, max) / max (`FIDIV` at `0x4f93b5`): **a max HP of 0 divides by zero** (*inferred*: a garbage
  bar). The maximum comes from the record `+0xC` at spawn and from `0x0608`; give pets HP of at least 1 and send `0x0608`.
- **Speed** (`0x431aa0`): units per second = (Step + adj) / (Time × 0.001) − 0.3; walk uses the normal values, run (`byMotion ≠ 0`) the chase
  values; `adj` from `0x0609` byMoveSpeed 0..4 = −4, −2, 0, +2, +4. No cap. A target 20.0 or more away makes the monster **snap** there.
  Animations: moving uses walk and run slots 0 and 1; arrival slot 6.

**Campfires and scenery** (archive listing and `monster.MON`, **confirmed**): spawnable models include treasure boxes (MON 83–85), fire poles with
flames already attached (MON 422, 426, 430, each with `monster.EFT` scenes 189–191), the water, wind and earth poles beside them, old guard
stones (MON 71–82) and Xmas trees (MON 416–418) that no row uses, and wood sprites (MON 24, 395–399). Fire emitters in `monster.EFT` are scenes
40/41/42 and 189–191, and 72/73/78/81/82 for the elementals. *Inferred* cheapest campfire: a new `Monster.SData` row with model 422 (or a new
`.MON` record with a small `.3DC` and attachments to scenes 40–42), Size 0, a small Day scale class, spawned with `0x0601`: data only.
World-only models (fire pots, braziers, tents, chairs, fishing nets, fish-house buildings) are placed by `.wld` files and are not spawnable;
converting one to a `.3DC` would make it spawnable (not tried). Every race and sex has a sitting animation (`*_012_sit.ANI`, action 12),
played by motion 1 of `0x0506`.

## Channelled actions, buff icons, chat colours

- **Cast bar** (globals `0x21e4e34` on, `0x21e4e38/40` ms, `0x21e4e3c` start; drawn over the local player by `0x4b8a60`, colour `0xff0000ff`,
  **no label**). Started by `0x0510`/`0x0516` (own cast: `Skill.SData` ReadyTime × 250 ms, `0x563c70`), **`0x0221`** (recall, fixed 5000 ms,
  sound `ef_in_atrbu01.wav`, `0x5671b0`) and the mount summon (client-local 3000 or 5000 ms, `0x4d4fa9`). It ends by itself at time-out
  (`0x4b8710`), or on `0x0222` (`0x55a170`, message by byte), `0x0216`, `0x0551`, death `0x0504`, monster death `0x0606` or own movement
  (`0x441cce`). A cast of arbitrary length without a real skill needs a data-patched skill row (ReadyTime up to 63.75 s) or the DLL.
- **Buff icons** (`0x4b8a60`, proven in play in patch 109 with a custom skill: icon, name, text, timer): 32 px icons every 40 px, **8 per row,
  unlimited rows**, from x = width/2 − 130, y = 60; kind 3 is a debuff flash; the tooltip counts down in d/h/m/s. A right-click on some buffs
  (detail `0x58/0x5a/0x5e/0x5f`) sends `0x0511 u8 0xFF, u32 key` (a cancel request) that a server can use as a button.
- **Chat categories → colour** (`0x41bc60`): 0 orange; 1/2/0x12/0x15 red; 3 gold; 4 green; 5/0x17 magenta; 6 cyan; 7 lime; 9 normal chat
  grey-white; 0xA party; 0xB whisper; 0xC guild; 0xD trade; 0xE shout; 0xF monster speech; 0x10 GM; 0x1E pink; 0x1F zone; 0x20 purple.

## Faction rules

- **The client scrambles the other faction's chat itself:** normal chat (`0x1101`, `0x554bd0`, balloon too), shout (`0x1107`, `0x553150`) and
  shop signs (except on map 42) go through `0x41cb20`: letters → `~`, double-byte characters → sysmsg 5092–5095. The scramble level
  comes from skill 3 "Interpretation" (`0x21e4e1d`, set by `0x0108`/`0x0209`): 0 all, 1 two-thirds, 2 about 40 %, 3 none. **Staff
  (`0x84d66c`) see clear text.** Whisper, trade, guild, all-members and zone chat are not scrambled; a combined chat needs the server to send
  GM opcodes or the DLL to skip `0x41cb20`.
- **Attacking players** (`0x43c000` case 2, sends `0x0502`) is allowed only against the **other faction**, the duel partner (`0x7045b4 == 2`)
  or a guild-war enemy (`0x7045d0 == 2`); **region feature 27** (`0x44055f`) removes the test.
- **The right-click player menu** opens only for the same faction or on maps 42 and 40, within 10 (60 between two guild members of different
  guilds, `0x47c610`); the test is in `0x43c000` (`0x4420c7`–`0x442177`, see `client-actions.md`). Inside it: not in a union, the fixed menu
  `0x47c700` — the **party invite is the only row with its own faction test** (`+0x276` equal); trade, view market, duel, ride together,
  inspect and whisper rows have none; the guild-war request (`0x2501`) needs my guild rank 1–4 (`0x21e54e2`), a target in another guild, and
  no duel, war or shop. In a union, the list menu `0x47d0d0`: "Union invitation" (6513) has no faction test and sends `0x0B14`
  (`0x491050`), printing "Invited <p>" only when the target is of my faction. The whisper row fills the name only for my faction or a
  party member. Inspect works only within 3.0 (`0x6a67f8`), else 233.
- **Chat commands and windows have no faction test:** `/party <name>` → `0x0B09` (`0x478210`) or `0x0B18` (`0x4781e6`), `/join` → `0x0B11`
  (`0x478174`), `/sign up for guild` on the selected player → `0x0D05` (`0x477c2e`), a friend request by typed name → `0x2202`
  (`0x4e0a07`), a whisper by typed name → `0x1102` (`0x46d328`). The incoming party, trade, duel, buddy and guild-invite handlers read no
  faction either. **Every cross-faction rule except the menu gate is the server's;** to allow cross-faction menus, the menu gate is the one
  test to lift (a DLL change; the party-invite row has its own).
- **GM status:** `0x84d668` privilege, `0x84d66c = privilege < 11` (`gm-and-commands.md`); it also unscrambles chat and shows HP numbers on the
  target bar. **Notices** `0xF106`, `0xF906`–`0xF90B` ("[Notice] <b>") and `0xFA07` ("[Warning]") take free text from any player.

## Levels and what the client blocks

**No level rule exists in combat:** `0x43c000` never reads the level `0x84d678`. The level only colours names (`0x4c21a0`, target level − mine:
10 or more grey, 8–9 magenta, 6–7 red, 4–5 orange, 2–3 yellow, −1..1 green, −3..−2 blue, −5..−4 light blue, −6 or less white). Other
level checks (**confirmed**): item use above the record's ReqLevel → 227 and nothing is sent (`0x4c7de1`); equipping → 311 (`0x4c1590`); the
movement rune to a party member (`0x4c7510`) needs my level at most 35 on maps 6/8, 15 on map 18, 30 on maps 30–32 (else 803); a mantle NPC
(kind 24 or 39) refuses level under 15 (5358, `0x47daee`); the PvP reward NPC (type 4, option 2) refuses level under 16 or a clear flag
`0x84fa1e` (dialog `0x6c`, 5346/5347, `0x506530`), else sends `0x0218` (`0x48d13f`); trade chat (`0x1103`) has level and cooldown gates only
for regions other than 6 (`0x477ef5`, `0x477f40`); the skill window compares each skill's required level (`0x4f1aba`, `0x4f2aba`; the learn
button disabling is *inferred*).

What the client blocks itself (nothing is sent when it fails):

- **Equipping** (`0x4c1590`, from `0x4d4e10`, `0x4e5bb0`, `0x4e6250`; 229 "Cannot equip <i>"): level (311); race by the Country field `+0xe`
  (0 race 0, 1 race 1, 2 races 0–1, 3 race 3, 4 race 2, 5 races 2–3; 246); mode by field `+0x15` (Easy only 0, Normal 0–1, Hard all but 3,
  Ultimate all; 247); class flags `+0xf..+0x14` (248); gender by Special 64 (sex 0 only) or 65 (sex 1 only) (5423).
- **Trade window, personal shop and drop:** an item whose record field `+0x24` is non-zero → 443 "cannot be traded" (`0x4ffdfe`, `0x4de772`)
  and 454 "This item cannot be dropped." (`0x4e6aea`, `0x4e6c8d`); 834 items have it. 231 "Quest items cannot be traded" also appears at
  `0x4fffad` (the condition was not traced). Equipped items cannot go into a shop (444).
- **Skills on a target:** no faction test (`0x4404db`, `0x440f93`: range plus line check `0x437fe0` only); the quick bar refuses while sitting,
  standing up or dead (states 5–8) and while the skill recharges (215) (`0x4d4e10`).
- **Maps:** portals are a box test (`0x524c60`) that sends `0x020A` once; the client tests no level or faction itself (the `.wld` portal records
  carry them, *inferred* unused on this path).
- **Town timers:** `/return` prints 251 (10 s) and sends `0x0556` at once; the quit dialog's timer (366 "You can only quit while sitting") was
  not traced.

## Items, ids and ceilings

- **Egg-like items:** the tooltip (`0x49fa00`) shows the record's name and description (static per type and typeId) and the requirement lines;
  **per-instance data is used only for equipment** (quality and six gem bytes feed stat lines, the 21-byte craft name feeds the bonus lines).
  For types 25, 30, 38, 41, 44, 95 and 100 (24 and 39 for gems) the same function clears the craft-name pointer and skips gems and quality, so
  a type-100 item shows nothing the server sets per item except the stack count. One egg type therefore cannot say "which monster": use one
  item record per monster (255 typeIds per bank, banks fixed at 100), or have the DLL add a tooltip line from the craft name (*inferred* small:
  the string travels in every item unit, `0x0106`, `0x0204`, `0x0205`, `0x0711`). The item name is the record name only (`0x460cd0` → `+0`).
- **Item use** (`0x050A u8 bag, slot`) carries **no target** (`0x4c7d40`); skills carry it (`0x0511`/`0x0517 u8 number, u32 target`), so a "capture"
  must be a skill. There is no cap on monsters in view (`0x44e990` inserts by id).

| What | Width on the wire | Client ceiling (confirmed) | Note |
|---|---|---|---|
| item type | u8 | `1 ≤ type ≤` bank count (`0x460cd0`; 100 now) | more banks need a bigger file (the count is read from it) |
| item typeId | u8 | `1 ≤ typeId ≤` records in the bank (255 now) | the one-byte chat item link and every item unit use u8 |
| skill id / level | u16 / u8 | `1 ≤ id ≤` count (`0x461ba0`; 326 now), level 1–3 only | `0x0108`, `0x050D`, `0x0511` carry u16 ids |
| monster id | u16 | `< count` (2,474 now; `0x461210`); model `<` `monster.MON` records (515) | |
| NPC type / typeId | u8 / u16 | types 3–16: `typeId ≤` that table's count (`0x479320`); type 1 (`0x4792e0`, 252 merchants) and 2 (`0x479300`) own tables | unknown → null record |
| quest id | u16 | `1 ≤ id ≤` count (`0x479360`) | quest texts and rewards are client data |
| buff icons | — | no cap (8 per row, unlimited rows) | |
| effect sets | — | 200 sets (ids 2–209), names hard-coded in `0x4248e0` | new effect files need the DLL |
| sysmsg | u16 id | any id in `sysmsg-uni.txt`; a missing id prints "GetMsg Error <id>" | 1,386 ids used, highest 10123 |
| map id | u16 | `world/<id>.wld` must exist; minimap and name tables cover 0–70 and 102–108 | not traced further |
| quick slots | u8 bar, u8 slot | 10 slots per bar (`0x4d4e10`, index `bar*10 + slot`) | bar count *inferred* from the window |

**What a signed patch can carry** (the updater, `updater.md`): archive files of any kind (the 108 archive holds dds, ani, 3dc, smod, wav, tga,
3de, eft, 3do, pc, mlt, wld, vani, dg, txt, bmp, mani, jpg, itm, alt, mon, sdata and a few others), removals through `remove.txt`, and loose
files anywhere under the client folder (`game.exe`, DLLs, text, configs); paths that escape the folder are refused. **The limit is the
client, not the updater:** a file is only used if something names it. Models need a list record (`.MLT` parts, `.ITM` items, `.MON` monsters
and NPCs) and an SData row pointing at it; textures and sounds are named by those files or by code; **effect files are named by code** (200
hard-coded names), so a new `.EFT` name needs the DLL, while replacing an existing `.EFT`'s content works by data alone. New icons are picked
by index from the item record (`+0x1a`, `0x51d370`, *inferred*), so new icon files follow the existing naming. The run-time names are listed
in `cleanup.md`.

## What a server can drive in the UI

- **A list the server fills:** `0x0701 u8 n, n × (u8 type, u8 typeId)` **replaces the open shop's goods list** (up to 256 rows, `0x557d40`);
  the buy request `0x0702` carries the row index. It is the only stock window whose rows the server picks at run time; the shop itself opens
  from a merchant NPC's menu (data), not from a packet.
- **Yes/no dialogs opened by a packet**, each with fixed text and one variable name: party (`0x0B02`, 204), union (`0x0B14`, 10007), guild
  invite (`0x0D05`, 8034), trade (`0x0A01`), duel (`0x2401`); also, *inferred*, ride together (`0x021D`) and the party-member summon (818,
  answered by `0x0224`).
- **Reward picker:** `0x0907 u16 questId` opens that quest's reward choice (`NpcQuest.SData`); answer `0x0907`.
- **NPC talk box:** text, buttons and sub-menus come from `NpcQuest.SData` (`0x48c120`, lookups `0x479320`, `0x4792e0`); no packet puts text
  or buttons into it (*inferred* from the menu code reading only data).
- **Free text** is chat lines and the screen-centre lines (below). Unknown opcodes are **silently ignored** (the fallback `+0x600` slot
  `0x551030` handles only `0x0114`, `0x1Cxx`, `0x1Dxx`).
- **Crafting windows:** link lapis `0x0801`, extract `0x0802` (4 bytes) and enchant `0x0805` each send bag and slot pairs and show a fixed
  success or failure line from the answer's first byte. Compose, remake and remake 2 (`0x0806`/`0x0807`/`0x0808`; windows `0x4a7c70`,
  `0x4a5cf0`, `0x4a4870`) open from a chooser (dialog `0x78`) that a **merchant NPC whose kind byte (`NpcQuest +4`) is 15 or 16** opens instead
  of its shop (`0x48cf3d`; kinds 9, 100 and 117 open dialogs `0x6e`, `0x73`, `0x75`; others the shop `0x65`). Answers: `0x0806` failure
  5502; `0x0807` 0 success, 2 → 5511, 3 → 5513; `0x0808` failure 5503. *Inferred:* remake is the cheapest "put items in, server answers"
  window (`0x0807` carries several bag and slot pairs).

## Effects, motions, banners and markers

- **Effect sets are hard-coded:** `0x4248e0` registers 200 `.EFT` names into a table (`0x21f6148 + id*300`, ids 2–209 with gaps; `Duel.EFT` and
  `Guild.EFT` load elsewhere, `0x4213d0`). **Skill effects come from code, not `Skill.SData`:** the cast effect is a switch on skill id with
  default set 37 (`0x5623a0`); hit and field effects are switches with default "none" (`0x562660`, `0x562d10`). A new skill row shows the
  generic cast effect and no hit effect; choosing an effect for a new or reused skill id needs the DLL (redirect those three calls), and
  field skills' looks are tied to their ids. Packets that play effects: `0x0510`/`0x0516` (cast), `0x0511`–`0x051B` and `0x060B`–`0x060E`
  (hits), recall `0x0221`, item use `0x050A`, guild-house reset `0x0D38`. **No packet takes an effect id.** Buffs (`0x050D`) play nothing
  persistent; persistent effects exist only as `.MON` attachments (`{i32, i32 scene}`, emitters of `monster.EFT`), and placed map effects
  are static `.wld` records.
- **Motions** (`0x54e3f0`): `0x0506 u32 id, u8 motion` acts on any player in view, including the local one, unless that player has `+0x288`
  set (*inferred* riding or transformed), except motion 2. 0 stands up and 1 sits down (the server can force both); 2, 3, 10 and 11 act on
  other players only; 116–125 are the ten emotes with chat lines 5228–5237 (the action played is the paired id); any other value is ignored.
  An emote *inferably* ends when its animation ends.
- **Rested experience** has no display: the only EXP-bonus texts are chat lines (66, 815 and 834). A rested state can be a buff icon with its own
  name and text (a new skill row, proven in patch 109) plus a line. **New text:** the markers `<p>`, `<t>`, `<b>`, `<v>`, `<i>`, `<s>` are filled
  from fixed buffers, not `%s`/`%d`; a new id is shown only where code asks for it, except **`0x110A u32 mobId, u16 msgId`**, which prints any
  id (a monster must be in view). Large id ranges in the file are unused (for example 489–799, 1313–1999, 5516–6000); 863 is asked for by
  `0xB103` type 4, so keep it free or give it a text.
- **Screen-centre text** (pink, mid-screen − 150 px, at most 5 lines, 10 s each; chat type `0x1E`): `0x1108` all-members chat ("[chatting
  all members]<p> : <b>", 481, `0x5544e1`) takes **free text with any sender name and no monster**, but is dropped for players who blocked
  that name; `0x1109` with kind ≠ 0 and `0x110A` need a monster in view. Sound to everyone: `0xF906`–`0xF90B` (one fixed sound) and `0x110B` (any
  `data/sound` file at a monster's position). Countdowns: the duel's 6 s (`0x2403`, client-run) and `0x0211` (per-faction value with a client
  countdown, `packet-opportunities.md`); there is no general countdown packet.
- **Map markers:** the obelisk list `0x2101 u8 n, n × (u32 id, u8 state, f32 x, f32 z)` and `0x2102 u32 id, u8 state` draw entries with state
  0–2 on the minimap at any x and z of the current map (not on maps 68, 69, 105, 108; `0x4ba850`); *inferred* an event position can be marked
  with a fake id (it looks like an obelisk). Party members come from `0x0C07`; quest marks and NPC dots from the `.wld` NPC list and quest
  data, no packet.
- **Boss presentation and death:** nothing marks a boss (name colour is the level difference, size the Day scale class). Death `0x0606`
  (`0x551c50`) flags the monster dead; with HP below 1 it plays the death animation (slot 5), the `.MON` die sound if present (`0x5280c0`) and an
  `Attack.EFT` effect (`0x431200`); the kill line is 3 "<t> died" when `byKillerType` is 1 and the killer is me, 4 "<p> killed <t>" for a
  party member (`0x431970`). No corpse timer was found: removal is `0x0602`/`0x0604`.
- **Water and ground type:** each field `.wld` names one `.wtr` (`Entity/Water/World.wtr`, `B1_Water.wtr`); the file starts with `f32 64.0` (*inferred*
  the water plane), a colour and caustic texture names (`0x59a7e0`); the client's in-water test was not found. The only ground-kind data is the
  per-corner terrain texture with its footstep sound name (`formats.md`) and **`data/world/TacticsZone.zon`** (loader `0x439270`, feature 33, on
  here): 35 areas, each `u8, u8 type, u8 sub, 4 corners, y range, u16 map, name`; type 1 safe zone (10 on maps 45–47), type 2 dungeon rooms of
  maps 65–67, type 100 a placeholder; `0x438650` returns the type at a position. No packet reports a terrain or water click.

## A DLL overlay: design facts

The companion DLL already replaces device vtable slots (Reset 16, Present 17, BeginStateBlock 60, EndStateBlock 61, draws 81–84), handles
device loss in Reset and changes the window style (`graphics.md`, `fixes-dll.md`); an overlay would draw in Present before the original and
release its own resources in Reset. `game.exe` imports `DirectInput8Create`, `GetAsyncKeyState`, `RegisterClass*` and `ImmGetContext`;
*inferred* input arrives through both the window procedure and DirectInput, so an overlay must subclass the window procedure and also stop
the game reading input while it has focus. The game draws text with GDI fonts into textures (`0x5373b0`) and uses no `D3DXCreateFont`, so an
overlay brings its own font. A new packet handler is a case in the fallback wrapper that `operator-listen.md` already uses; no handshake
exists, so the cheapest *inferred* way for a client to announce itself is one new client-to-server opcode sent after `0x0201`, which a server
that logs unknown opcodes can read (the original closes the connection). A public project (Spelunkern's Shaiya-Core) shows the same
approach: it subclasses the window procedure, hooks Present and Reset, and draws Dear ImGui with the Win32 and DX9 back ends.

## Open

| Point | Cheapest way to settle |
|---|---|
| live look of a scaled monster (Day 6) | patch one record's Day byte to 6 and spawn it |
| monster with max HP 0 | spawn a record with HP 0 without `0x0608`; look at the target bar |
| race above 3 | not worth a live test (crashes viewers); keep the server check |
| quit-dialog timer (366) | callers of message-box kind 8 (`0x516770`) and the system-menu exit path |
| condition of 231 (quest items in trade) | read `0x4ffc50` around `0x4fff93` |
| campfire look | spawn the "Flame Relicpole" row, then a Size 0, small-scale copy |
| ground model per item | drop two different items of one type; compare |
| the client's in-water test | readers of the `.wtr` height after `0x59a7e0`, or walk into a lake and watch for `Cwater.EFT` |
| which NPCs have merchant kind 15 or 16 | parse the merchant records (`+4`) |
| map-id ceiling, quick-bar count | read the map tables indexed by `0x84e6e0` and the quick-bar window |
| `0x1108` screen-centre look | send one with a test name and check the centre line |

The wider list of settled and unsettled client questions is in `re-open-items.md`.
