# The character window, attack timing, race skills and the creation screen

This page answers, from the US `game.exe` (build 2010-04-26, patch 108), the client-side questions a game-balance change raises: what the
character window computes itself, what the client does before sending an attack or skill, how race and percentage skills are offered and
priced, and what the character-creation screen shows and accepts. It was read statically (Ghidra, read-only) together with the
client's decrypted `Skill.SData` and `Item.SData`; two cross-checks used the original `ps_game.exe` and its `ItemEnchant.ini`. Table
values are not reproduced; where one matters, a single number illustrates the mechanism. Status words: **confirmed** (read in code or
data), *inferred*, **live** (needs a game session). Addresses are in `game.exe` unless marked `ps_game`.

## In short

- **The character window computes attack, magic attack, defence and magic defence itself,** with the original server's per-point factors
  hard-coded. No packet carries those numbers, and hit, evasion and critical are not shown. A change to the formulas shows wrong numbers
  in the window until the client is patched.
- **Maxima (HP/MP/SP) and starting stats come from the server;** the client has no status table, and the creation screen shows no stats.
- **Crafted HP/MP/SP is multiplied by 100 in the client** (one byte, `0x64`), so a server that halves it needs a one-byte client patch.
- **The client has no wait of its own tied to the weapon:** its gates are the skill's cooldown, one outstanding skill request, and the
  cast state. The server's interval is the cadence players feel.
- **Skill costs are not checked before sending,** and a cost of exactly 1 is printed as "spend all".
- **The creation screen can grey out Hard and Ultimate but cannot be told to preselect or force one;** its answer has only three result
  codes, so a refused mode has no code of its own.
- **The original enchant tables are not the ones a copied server uses:** they live in a data file next to the original server, and the
  client's window uses the original formula.

## Short answers

| Question | Answer | What it means |
|---|---|---|
| Does the window compute combat numbers? | Yes: attack, magic attack, defence and magic defence, from stats, items, lapis, crafted stats, enchant, buffs and passives. Hit, evasion, critical are not shown. | Changing the attack or defence formulas or per-point constants needs a client or DLL patch (five float constants); changing hit, evasion, critical or PvP-only rules shows nothing wrong. |
| Where do maxima and base stats come from? | Maxima from the server (`0x0105`, `0x050B`). Base stats from the character list (`0x0101`) at world entry, then `0x0208`, level-up and `0xF701`. | Status rows, mode factors and points per stat are server-only. |
| Does the creation screen show stats? | No, and there is no starting-stat table. | Server-only. |
| What does `KillStatus.SData` do? | Drives the grade **tooltip (effect names, no values)**, the kill gauge, the 10-minute top-grade state, and the lapis link/extract rate shown (effect type 23). It never feeds the character window. | A server can change a faction's grade effects; the tooltip keeps listing the old effect names until the file is patched. Cosmetic. |
| Crafted HP/MP/SP | The client multiplies them by 100 (`0x4c47ba`, `IMUL EAX,EAX,0x64`). | Halving the effect needs `0x64` → `0x32` there, or the tooltip shows double. |
| Weapon-tied wait? | None. | See "Attack interval and casting". |
| Casters tied to staves? | No: every caster skill allows all 15 weapon kinds. | Nothing to change in the client. |
| Skill cost checks? | None before sending; cost 1 prints "spend all". | Server-only changes work; the tooltip and refusal text follow the client's own cost. |
| Can the creation screen force a mode? | `max_grow` only greys out Hard (< 2) and Ultimate (< 3); the mode starts unset. | "Ultimate only" needs the server to override, or a client patch. |

## The character window

### Where the numbers come from (confirmed)

The window (`0x4f3f80`) draws, top to bottom: name and level; job (sysmsg 5047–5058); the kill rank name (`0x6f35d0` thresholds against
the kill count, sysmsg 5389+); the mode (470–473, from `0x84fa1e`); the stat points left; the six stats; the HP, MP and SP bars; attack,
magic attack, defence and magic defence; the weapon's element; attack-speed and move-speed words; the four kill counters. The allocation
popup (`this+0x4c`) draws the six stats again with the points being added. Each stat is drawn as `base` or `base + bonus` ("%d + %d" /
"%d - %d"): bonus = item + lapis + crafted digits + buffs and passives. Bases: STR `0x84d690`, DEX `0x84d694`, REC `0x84d698`, INT
`0x84d692`, WIS `0x84d696`, LUC `0x84d69a`; the bonus fields are `0x84d6a4`–`0x84d6b8`. The bases come from the character-list slot at world
entry (`0x46b4eb`–`0x46b544`), then `0x0208` answers, the level-up handlers and `0xF701`. The equipment loop (`0x4f4800`–`0x4f4950`) walks
equipment slots 0–12 (`0x84e6f9`, stride `0x22`), taking each item's `Item.SData` record, craft name and six lapis (type 30; every slot
but the cloak, as on the original server).

| Line | Formula in the client | Constants |
|---|---|---|
| attack (melee weapon) | `ftol(STR × 1.3) + ftol(DEX × 0.2) + add_melee` + item attack + enchant; max adds the item spread | `0x6a67d8` = 1.3; `0x6a9be8` = −0.2 (subtracted) |
| attack (ranged weapon: item types 11, 13, 14, 59, 62, 63, 64, `0x4c0290`) | `STR + ftol(DEX × 0.2) + ftol(LUC × 0.3) + add_ranged` + item + enchant | `0x6a859c` = 0.2; `0x6a667c` = −0.3 (subtracted) |
| magic attack | `ftol(INT × 1.3) + ftol(WIS × 0.2) + add_magic` + item + enchant | 1.3, −0.2 |
| defence | `REC + add_def` + item defence (`+0x32`) | – |
| magic defence | `WIS + add_mdef` + item magic defence (`+0x34`) | – |

`add_*` are `0x84d6bc` (melee attack), `0x84d6c0` (ranged), `0x84d6c4` (magic), `0x84d6c8` (defence), `0x84d6cc` (ranged defence, never
drawn) and `0x84d6d0` (magic defence): the player object `0x84d668` + `0x54`…`+0x68`. They are filled only by the client's own buff
bookkeeping (`0x4c0760` add, `0x4c0e50` remove) from its `Skill.SData` copy: abilities 4 STR, 5 REC, 6 INT, 7 WIS, 8 DEX, 9 LUC, 23 melee
attack, 24 ranged attack, 25 magic attack, 26 defence, 27 ranged defence, 28 magic defence, for `TypeDetail` 4 and 82 (added), 62 (taken
off) and 91, when the server reports a passive (`0x0108`, `0x56c1d0`) or a buff (`0x010A`, the buff packets, `0x56c020`). They are zeroed
on map change (`0x46b7d8`, `0x42f2b9`).

**The window leaves out** (confirmed by absence in `0x4f3f80` and the add/remove functions; *inferred* that nothing else writes those
fields): the faction grade, equipment wear (no quality check; the original server halves below half quality), weapon- and
shield-mastery passives (not handled by `0x4c0760`), and armour enchant. Hit, evasion, critical, ranged defence and absorption are not
drawn at all.

**Enchant attack in the window** (`0x4c4920`): step `n` adds the sum over `k = 1..n` of `3 × max(1, ⌈k/3⌉) + 4`, so +1 adds 7 and the total
grows faster than linearly (weapons, enchant values 1–50 only). This is the original `ItemEnchant.ini` attack table, **not** a flat 7 a
step.

**`TypeDetail` 91 in the client** (`0x4c0760`): ability 1 is **subtracted**, abilities 2 and 3 are added (and the reverse on removal). The
original server adds all three for both 82 and 91 (`ps_game` `CUser::AddApplySkillBuff` `0x484cf0`, case `0x5b` → the `0x52` path;
`RemApplySkillBuff` `0x485130` the same). The only 91 rows have max HP in ability 1, which the window does not track, so nothing visible is
wrong. **Rule for new data: do not put a window stat (abilities 4–9, 23–28) in ability 1 of a `TypeDetail` 91 row**, or the window shows it
with the wrong sign.

**What a formula rebalance needs** (*inferred*): changing a per-point constant of attack needs the five floats patched. Two of them
(`0x6a9be8`, `0x6a667c`) are shared `.rdata` constants with possibly other readers, so a DLL patch should redirect the `FMUL` operands at
`0x4f616d`, `0x4f618d`, `0x4f61b5`, `0x4f61d3`, `0x4f62bd`, `0x4f62dd` to its own constants instead of overwriting them. Changing REC → defence
or WIS → magic defence (× 1) needs new code in the window. **Live:** equip a known weapon and compare the window with a model of the formulas
for the same build.

### Maxima, creation stats, `KillStatus.SData` (confirmed)

- **Maxima:** no status table exists in `game.exe` (searched as u16/u32/f32 arrays) or in the client data files (the loader `0x4248e0` reads
  only `Item`, `Skill`, `NpcSkill`, `Monster`, `NpcQuest`, `Cash`, `KillStatus`, `GuildHouse` SData). `0x0105` (`0x54cdc0`) sets the maxima
  `0x84d67c/7e/80` and clamps the current values; the bars draw the player object's `+0x130/134` (HP), `+0x138/13c` (MP), `+0x140/144`
  (SP). Nothing in the client multiplies by 1.1 or 1.5 or adds a per-stat amount.
- **Creation screen** (`0x464ab0`, `0x466be0`): race, job, face, hair, height, mode buttons and the mode texts (5034–5037), which **state the
  status and skill bonuses in words**; no stat numbers and no starting-stat table. Changing the points per level means changing those four
  texts, nothing else.
- **`KillStatus.SData`** (loaded at `0x4249e3` into `0x21d4fc8`): per faction (0, 1) up to 256 grades of `0x7c` bytes: a kill threshold and up to
  28 effect values (types −1 to 26). Readers: `0x54caf0` (the server's faction kill count is stored at `0x21e54a4[faction]`; reaching the top
  threshold starts a 600,000 ms state with `ui_etainbless01.wav` and `event.tga`); `0x482c40` (the grade tooltip: one line per non-zero effect,
  **the effect's name only**, sysmsg 5127–5153; no values); `0x483050` (the kill gauge, "%.1f%%" of the next threshold); `0x492f60` (the lapis
  link/extract window adds the grade's type 23 value to the rate it shows). The client never adds grade bonuses to the window. A server that
  gives one faction the other's effects works server-side; the tooltip lists the old effect names until the file is patched, and the
  thresholds (hence the gauge) stay right.
- **Crafted HP, MP, SP:** `0x4c4760` parses a 20-character craft name: digits at `i`, `i+1` → `10a + b`, and for `i` = 12, 14, 16 (HP, MP, SP)
  **× 100** (`0x4c47ba`, file offset `0xC47BA`, bytes `6B C0 64`). Users: the item tooltip `0x49fa00` (base HP in white, then the crafted part
  "+%d" in orange), the window (parsed and **discarded**: it shows no item HP), the enchant answer `0x558910` and the item name `0x45e520`
  (digits 18–19 only). If a server halves the crafted amount, the tooltip shows double until the byte becomes `0x32`; the companion DLL can
  write that at load (a byte-checked write; `fixes-dll.md`). Nothing else uses the factor.

## Attack interval and casting

### The client's own waits (confirmed)

Skill use from the quick bar (`0x4d4e10`) or the input loop (`0x43c000`) refuses without sending when:

1. the skill's cooldown is running (learned-skill record `+0x10` non-zero; message 215 "Recharge must be completed before next use"); the
   cooldown is `ResetTime` from the client's copy, restarted from the server's `0x0108` times and the results;
2. a skill request is outstanding: `0x21e493c = 1` after `0x0511`/`0x0517` is sent (`0x4404ed`, `0x440fa5`), cleared by the result handlers
   (`0x54d960`, `0x54de20`, `0x567720`, `0x569310`, `0x569b10`, `0x56b430`), the hit (`0x416cd0`) and a few resets;
3. the player state `+0xdc` is 5 or 6 (motion states from `0x0506`), 7, or **8 (casting**: set by the own `0x0510`/`0x0516`, `0x563c70`, for
   `ReadyTime × 250 ms`, and by the recall, `0x5671b0`).

Then come the line-of-sight test (`0x437fe0`, messages 457/458) and the range walk. There is **no timer based on the weapon's speed, no
global cooldown and no animation lock** for skills. Normal attacks (`0x0502`/`0x0503`) send one request per engagement (latches
`0x7050c8`/`0x7050cc`, cleared when the target leaves range or the action changes) and at least 250 ms apart (`0x70512c`); the server repeats
the blows.

**Animation playback** (`0x419f90` for players, the same formula at `0x56a7d4` for cast effects): index = the player's attack speed (`+0x274`,
from `0x051C`) − the equipped weapon's own speed (`Item.SData` record `+0x28`) + 5 (5 without a weapon); keys per second by index 0–9 run from
15 up to 45 (`0x6a666c` … `0x6a6648`; 30 at index 5); out of range (negative or above 9): 50; speed 255 (special states): 30. So the animation
speed reflects buffs relative to the weapon, not the weapon itself.

### Short sword against staff (from data; motion choice *inferred* from file names)

Player rigs have, per race and sex, one-hand attacks (`*_035–038_onattack*`), staff attacks (`*_060–061_sfattack*`), unarmed attacks
(`*_079–082_nfattack*`) and **one spell set** (`*_085 maready01` … `091–096 buready/buroop/buattack`) without weapon variants. A mage therefore
casts the same animation at the same speed with a short sword or a staff (both index 5 unbuffed). The player can send the next skill as soon as
the previous request is answered and no cast is running, before the animation ends (the outstanding-request flag is cleared by the result, not
by the animation). The vail caster rigs have **no one-hand attack set**, so a vail caster with a short sword uses another set for normal blows
(*inferred*; **live**). If a server makes staves faster and leaves the client's weapon speed unchanged, casters' animations play slightly
faster (index 7 instead of 5) but nothing breaks.

### Casters and staves (confirmed)

All 163 named caster skill rows allow all 15 weapon kinds; two (item skills) allow none. The only weapon-specific rows for casters are a
dagger power-up (weapon type 10) and **Staff Mastery** (320: weapon type 12, +1 attack speed from level 25, 6 points; levels 2 and 3 are empty
"ED" rows), the same in both copies of the data. With Staff Mastery a staff is one speed step faster, not two.

## Race and percentage skills

### Evasion, Bloody Karma, Battle Cry, Etain Bless (confirmed)

- **Offered:** the learnable list (`0x4cae70`) takes every skill whose level-1 row has `RequiredLevel ≠ 0`, `Country` matching the race (0
  human, 1 elf, 2 Light, 3 deatheater, 4 vail, 5 Fury, 6 all), the job flag, and `Grow` against the mode (easy: `Grow` 0; normal: 0, 1; hard: not
  3; ultimate: any). So every elf, of every job, gets Evasion (country 1); the race skills cost 0 points.
- **Cost display** (`0x49e500`): MP then SP; a value of exactly **1 prints "disposable MP: spend all"** (5320 + 5322), any other value
  "disposable MP: %d". Evasion and Etain Bless show "spend all" twice; Bloody Karma (0, 0) shows no cost. Their descriptions also say they
  sacrifice the remaining SP and MP.
- **Cost check:** none before sending (`0x4d4e10`, `0x43c000` compare no SP or MP). The only comparison is in the refusal message (`0x5622a0`,
  result 6): if the client's MP is below the client's MP cost it says "MP is not enough" (214 with `<hp>`), else if SP is below the cost "SP is
  not enough", else **nothing but the error sound**. With the client's cost 1, a server refusal for a new fixed cost shows no text until the
  client data is updated.
- **Buffs:** Evasion, Battle Cry and Berserker are `TypeDetail` 82 (abilities added; the window shows only stat, attack and defence types);
  Bloody Karma is 91 (max HP, melee and magic attack): the window adds its attack lines, the HP comes from the server. Icons and timers behave
  as any buff (`research-extension-features.md`).

### The level-60 skills 180 to 185 (confirmed)

Each of the six (one per job pair) has level 1 for hard and ultimate (`Grow` 2, 8 points), level 2 for ultimate only (`Grow` 3, 1 point), and
an empty "ED" level 3 with no job flag and level 0 (Etain Bless swaps the point costs: 1 then 8). The window (`0x4f1630`) enables "learn the next
level" when the next row exists (level ≤ 3), has the player's job flag, its `RequiredLevel` is non-zero and not above the level, and the
player has its points; **it does not check the next row's `Grow`**. So in hard mode the skill is listed (level 1 `Grow` 2) and **level 2 is
offered although it is ultimate-only**; the server must refuse it (*inferred*; keep the server's `Grow` check). Easy and normal do not list
the skill; level 3 is never offered. Client and server rows for 180–191 are identical (only the cast time of 186 differs).

The skill tooltip (`0x49e9a0`) shows name, cost, cast time ("cast time: %.1f seconds"), recharge time, range ("%dm"), "abnormal condition" when
the skill has a state, then **the description text**; **no damage number is computed**. The percentages are written in the descriptions (87, 92,
88, 87 % at level 2), so a changed value needs a changed description. For three of those skills the description says "current HP" for damage the
data applies to the maximum, so the text is wrong today.

## The creation screen and the mode

**What it offers** (confirmed). `0x0109` (`0x579780` → `0x55cec0`) stores the faction at `0x21e4942` and `max_grow` at `0x21e4943`. The screen
(`0x466be0`) builds four buttons from `create_mode_button.tga`: easy (`this+0x58e0`), normal (`+0x5db4`), hard (`+0x6288`), ultimate (`+0x675c`),
and **disables hard if `max_grow` < 2 and ultimate if < 3** (`0x467a97`–`0x467ab6`; the button's `+4` enabled byte; a disabled button ignores the
mouse, `0x514500`). Easy and normal are never disabled. The chosen mode (`this+0x893d`) is `0xFF` when the screen opens (`0x42a796`); a click sets
0–3 and shows the mode text (5034–5037); easy first asks "You can only attain Level 30 in Easy Mode. Are you sure…" (5382, when a region
feature test passes; the condition was not traced). Pressing create without a mode says "must select mode" (5045). `max_grow` values above 3
act as 3. **No value preselects a mode or hides easy and normal.** For "ultimate only" a DLL hook after the screen build (`0x467abb`) can clear the
`+4` byte of the easy, normal and hard buttons and set `this+0x893d = 3`; a byte patch of the two `CMP byte ptr [0x21e4943], 3/2` sites plus two more
disables also works. The mode texts can stay.

**Another mode than the one picked** (confirmed). The create request (`0x5857c0`) sends the pick; the answer handler (`0x54cd40`) only sets a
state from the result code; nothing of the pick is stored. The list handler (`0x54cbc0`) stores every `0x0101` slot as sent (mode at slot `+0x1b`).
At world entry (`0x46a4e0`) the game copies the slot's level, race, **mode** (`0x84fa1e`), hair, face, size, job, sex and stats; the mode is then
shown in the character window and read by the tooltip, the skill window (special skills are refused in easy, 10087), the skill list filter and
others. `0xF701` kind 0 (`0x554030`) overwrites it in game. A server that creates the character as ultimate whatever was picked therefore shows
ultimate in the game and (if the select screen draws the mode; **live**) in the list.

**Refusals** (confirmed). In the wait loop after the request (`0x464ab0`, `0x4653ea`–`0x465190`): result 0 (state 1) → back to the list; 1 (state
2) → "name already in use" (5046); 2 (state 3) → 5500 "If you disconnect in battle zone, you can not creat character."; **any other value is
ignored** and after 30 s "data receving error" closes the screen (state 5). There is no code for a refused mode, so refusing a lower mode can
only reuse code 1 or 2 (both misleading); creating as ultimate is the clean alternative. Before sending, the client itself refuses: no name or
banned words (5040–5043), no job (5044), no mode (5045).

## Findings outside the questions

**The original enchant tables are in a data file, not in `ps_game.exe`.** `ps_game` `CGameData::InitItemEnchant` (`0x4c7ab0`) opens
`data/ItemEnchant.ini` and, for steps 00–20, **overwrites** both the success-rate array (`g_LapisianEnchantPercentRate`, `0x542058`) and the add-value
array (`g_LapisianEnchantAddValue`, `0x542100`) with `GetPrivateProfileIntA` (default 0 for a missing key). The static `.data` values (a flat 7 a
step for weapons, 5 for defence) are only what is used without the file. Two copies exist in the original server files: the patch copy has
decreasing success rates and an attack table that is **exactly the client's own formula** (`0x4c4920`), so it is the intended one; the client-tools
copy is a private-server edit (99 % everywhere). A server that uses flat values gives less attack from +4 up than the window shows (by +20 the gap is
more than a factor of two), and any rate arithmetic based on the static values is far below the patch file's. *Inferred* side note:
`ps_game` `CGameData::ReItemEnchant` (`0x4c7bc0`, the reload command) seems to have an original bug: when a step's defence value differs from the
weapon value it **writes the weapon value into the defence slot** (rates and add values both).

Small notes: the client's ranged item types for the attack line are 11, 13, 14, 59, 62, 63, 64 (`0x4c0290`); level 2 descriptions of three of the
skills above say "current HP" for damage of the maximum (`DamageType` 4).

## Leftovers from the extension-features page

- **Quit and character-select timer** (confirmed; system menu `0x4ed990`, case at `0x4ef0db`). The menu's "quit" and "to character select"
  buttons do something only when the player is **sitting** (state `+0xdc` = 7), **on a mount** (`+0x288` / `+0x378`) or has the `+0xe4` flag (not
  identified). Then the client opens a countdown box (kind 9 for quit, 7 for select; `0x516770`), sets an **11.0 s** timer (`0x21e8b60`, shown
  capped at 10: "<v> seconds left till game over", 10083/10084) and **sends `0x010D` (quit) or `0x0107` (select) at once**. Standing up or moving
  closes the box (`0x4ef1a7`–`0x4ef218`) without sending anything (*inferred*: no cancel packet exists). The game ends only when the server answers
  `0x010D` (`0x54c900`: game state 5); the countdown is display only. Boxes 365/366 ("you can only quit after sitting down") are kinds 6/8 of
  the same box; their opener was not found in the menu.
- **Message 231 "Quest items cannot be traded"** (trade-window drop `0x4ffc50`, `0x4ffe2f`–`0x4ffe47`) is raised for items of **type 27, 28, 29 or 99**
  (collection, quest and letter items), after the "cannot be traded" flag check (443).
- **Merchant kinds 15 and 16** (confirmed): kind 15 is a single blacksmith NPC placed once, on map 42; kind 16 is a single NPC placed nowhere.
  So the compose and remake windows are reachable through that one NPC on map 42 only.
- **Map-id ceiling** (confirmed): no client table is indexed by the map id (`0x84e6e0`, u16). The world loader (`0x452270`) builds `<id>.wld`, `<id>.bmp`
  and `DATA\world\<id>.txt` from it; the name (`CPlayerData::GetMapName`, `0x4c9100`) is a switch over known ids and **prints the number** for any other
  (and logs); the ambient sound (`0x42ca20`) and the big-map setup (`0x4b9210`) fall back to a default or nothing. **Maps 100 to 200 refuse some item
  uses** with 227 (`0x4c7eba`). So a new map id works up to 65535 as long as its world files exist; it gets a numeric name and no big map unless
  the DLL adds them.
- **Quick bars** (confirmed; `0x010B` logic `0x567290`): records of 5 bytes `u8 bar, u8 slot, u8 bag, u16 slot/id`. Bars 0–9 go to `0x21e4944 + (bar × 10
  + slot) × 16`, but the table holds **50 entries (bars 0–4, 10 slots each)**: bars 5–9 or a slot above 9 **write over the next tables**. A server sends
  bar 0–4, slot 0–9. Bars 10–20 go to a second table of 6 a row (room for bars 10–13 only); bar 100 is a separate two-entry table tied to an
  inventory slot; a bar of 21 or more with bag 255 sets the shown page.
- **The client's in-water test** (confirmed; `0x419d6c`, `0x44e843`) uses a **fixed height**, not the `.wtr` file: the player swims (`+0xe0` = 1, motions
  6 and 7, `ch_mswim00x.wav`) when its height is −0.4 or below (`0x6a67c4`), and a swimmer's height is clamped to −0.4. So water is wherever the terrain
  lies below world height −0.4; for fishing spots, terrain below 0 can be marked from the client's `.wld` heights.

## Open

| Point | Cheapest way |
|---|---|
| the window against a server, end to end | **live:** one character with a +5 weapon, the window next to a model of the formulas |
| other writers of the `add_*` fields (mastery passives) | references to `0x84d668 + 0x54..0x68` through other base registers, or learn a weapon mastery and watch the window |
| vail casters' normal-blow set with a short sword | **live:** look |
| whether the select screen draws the mode | **live:** one look at the character list |
| a server refusing level 2 of 180–185 in hard mode | the server's own skill check, or a hard-mode level-60 character clicking level 2 |
| who opens the 365/366 "sit down first" boxes; the `+0xe4` flag | callers passing kind 6 or 8 to `0x516770` through a register |
