# Monster speech: who says each line

This page maps every line a monster can say in EP4.5 to the monsters that say it, and says when. It covers the lines of the AI scripts
(`Data/kr_string.DB`, ids 1 to 33) and the monster lines of the US client's system-message table, `sysmsg-uni.txt`: ids 6001 to 6031, which
are the same 31 lines in English, and 6401 to 6404, said by the Lua bosses of the Cailum Sacra instance. The AI files (`Data/AI/*.ai`,
`Data/mob_ai.ini`), the Lua scripts and the definitions were read; nothing was executed. How the scripts run is on [Scripted
monsters](scripted-mobs.md) and [Party instance zones](instances.md). The client's side of the speech packets is on the client's
[Scripted monsters](../client/scripted-monsters.md) page.

## In short

- **Only 35 of the client's 1,386 system messages are monster speech**: 6001 to 6031 and 6401 to 6404. The rest are the interface's own
  texts. Monster names are not in this table; they come from `Monster.SData`.
- **6001 to 6031 are the English of the AI scripts' 31 Korean lines**, in the same order: sysmsg 6000 + n is line n of `kr_string.DB`.
  But the original server never sends these ids. It sends the Korean text itself, which the US client shows as garbage. So in the
  original, **6001 to 6031 are never shown**.
- **A later client adds 100 lines, 6032 to 6131**, for EP5's returned Cryptic One; scripts found later say 78 of them (section 5).
- **6401 to 6404 are sent by id**, and so are shown in English: the Lua bosses Dios Exiel (2470 to 2472) say them in the Cailum Sacra
  instance.
- **The named monsters of maps 45 to 47** say one of two lines when a fight starts, picked at random (49 % / 51 %).
- **The Cryptic One bosses of maps 48 and 49** say an opening line, and some say more lines at set times in the fight.
- **Some lines have no speaker in the data**: 11, 12 and 13 (scripts no monster uses), 31 (the Haruhion and Freezing scripts, unused).
  Lines **32 and 33** are called by two bosses but are not in the file, so nothing is said.

Status words: **confirmed** = read in the files or the code (address given), **inferred** = follows from what was confirmed but is not
shown directly.

## 1. How speech reaches the client

Confirmed, `CMob::PAI_Say` 0x4b06c0, `PAI_Say2` 0x4b0b80, `CMob::LuaSay` 0x4b1d30, `LuaSayByIndex` 0x4b1ff0, `LuaSayByVoice`
0x4b2250 (the opcode each writes: 0x4b074c, 0x4b1d7b, 0x4b201f, 0x4b2274); the client handlers `0x57e650`, `0x57e6f0`, `0x57e740`
(`docs/client/scripted-monsters.md`).

There are three packets. The AI scripts can only send text from the server's table; Lua can send its own text, a sysmsg id or a sound:

| packet | sends | who uses it | what the client prints |
|---|---|---|---|
| **`0x1109`** | the **text**: `u8 kind, u32 monster id, u8 len, text` | the AI scripts' `Say` (kind 0, to the target or its party) and `Say2` (kind 1, to the players within the radius), with the text of the **server's** `data/kr_string.db` (the name in `ps_game`, 0x530164) | "`<monster name> : <text>`" |
| **`0x1109`** | the same | the Lua scripts' **`LuaSay(text, radius)`**, with the text written in the script | the same |
| **`0x110A`** | a **sysmsg id**: `u32 monster id, u16 id` | the Lua scripts' `LuaSayByIndex(id, radius)`; the text is the **client's** `sysmsg-uni.txt` line | "`<monster name> : <sysmsg line>`" |
| **`0x110B`** | a **sound**: `u32 monster id, char name[32]` | the Lua scripts' **`LuaSayByVoice(file, radius)`** | plays `data/sound/<name>` at the monster |

So a sysmsg line can only come from a Lua script's `LuaSayByIndex`; an AI script never shows one. The official scripts we have use
`LuaSayByIndex` throughout (the 2011 ones keep each line's Korean `LuaSay` above it as a comment), and **none of the scripts we hold
calls `LuaSayByVoice`** (EP4.5's eight, EP5.4's eight and the later set of section 5), so no monster sound sent by the server is known.
EP5.4's server reads `data/cn_string.DB` (Chinese) in place of `kr_string.db` (`docs/original/ep5.4.md`).

`Say` and `Say2` look the number up in `kr_string.DB` and send its text (`docs/original/scripted-mobs.md` section 8). The file is Korean,
code page 949, apart from line 2. The US client draws those bytes in its own code page, so the line comes out unreadable.

**Why 6001 to 6031 are the same lines** (inferred): the table has exactly 31 lines from 6001 to 6031, and line 6002 is "Li laka
Amon-Ra", the one line `kr_string.DB` has in Latin letters (its line 2). The others follow the Korean line by line, though some loosely
(line 26, "앱소션 쉴드 전개", "Absorption Shield, deploy", is 6026 "Raise the Apsosion Shield!").

**Nothing in the client uses them by itself** (confirmed): a search of `game.exe` finds no instruction with an id from 6001 to 6031 or
6401 to 6404, and no 6000 or 6400 base added to a number. `0x110A` is the only way to show them. The original's AI scripts use
`0x1109`, so the client's English lines 6001 to 6031 are never shown. They were translated and shipped but not wired up.

## 2. The lines and their speakers

The text is the US client's (`sysmsg-uni.txt`, 1,386 lines); the Korean and a closer English are in the appendix of [Scripted
monsters](scripted-mobs.md). "Opening" means the line a script says when a fight starts, at most once until the fight ends (its counter
`nCount` still 0; section 3).

| sysmsg | line | text (US client) | said by | when |
|---|---|---|---|---|
| 6001 | 1 | Smite the heathens with a hammer of iron! | `SorpNamed1` (AI 17) | opening, 49 % |
| 6002 | 2 | Li laka Amon-Ra | `SorpNamed1` (AI 17) | opening, 51 % |
| 6003 | 3 | I shall separate your muscle and bone! | `SorpNamed2` (AI 18) | opening, 49 % |
| 6004 | 4 | Unto the betrayers give a banquet of bloody death! | `SorpNamed2` (AI 18); `OrcNamed1` (23), `OrcNamed2` (24) | opening: 51 %; 49 %; 51 % |
| 6005 | 5 | Die, O foolish man! | `CrocoNamed1` (AI 20) | opening, 49 % |
| 6006 | 6 | Let's see the extent of your ability! | `CrocoNamed1` (AI 20) | opening, 51 % |
| 6007 | 7 | Time for you to die! | `CrocoNamed2` (AI 21) | opening, 49 % |
| 6008 | 8 | There are none who can avoid death! | `CrocoNamed2` (AI 21); `Alcarian` (30); `Kirhiross` (33) | opening, 51 %; opening; opening |
| 6009 | 9 | Each step only hastens your march toward death! | `CrocoNamed3` (AI 22); `SorpNamed2` (18); `OrcNamed3` (25) | opening, 49 %; at HP ≤ 45 % (section 3); opening, 51 % |
| 6010 | 10 | Hu huuuuu...it's been a long time since I've eaten meat. | `CrocoNamed3` (AI 22) | opening, 51 % |
| 6011 | 11 | The price for your previous offense shall be death. | `OrcNamed1` (AI 23) | opening, 51 % (no monster) |
| 6012 | 12 | Feel our pain~! | `OrcNamed2` (AI 24) | opening, 49 % (no monster) |
| 6013 | 13 | Rage, O storm of blood and madness! | `OrcNamed3` (AI 25) | opening, 49 % (no monster) |
| 6014 | 14 | For the glory of Helltooth! | `TrollNamed1` (AI 26); `HellTouthNamed1` (27) | opening, 51 % each |
| 6015 | 15 | I'm the commander of these forces! | `TrollNamed1` (AI 26) | opening, 49 % |
| 6016 | 16 | I..will...kill you. | `HellTouthNamed1` (AI 27) | opening, 49 % |
| 6017 | 17 | Welcome to the world of dreams... | `Spink1` (AI 14); `Deinos` (28) | opening, 49 %; opening |
| 6018 | 18 | You dare stand against a god? You must be a fool! | `Spink1` (AI 14); `Parridalis` (29) | opening, 51 %; opening |
| 6019 | 19 | You've no time to think in this battle! | `Alcarian` (AI 30) | at HP ≤ 10 %, once |
| 6020 | 20 | I shall be reborn in Gehena's fire. | `Alcarian` (AI 30) | 2 s after line 19, with the summon |
| 6021 | 21 | I was praising you for my own amusement, but I've grown bored of you now! | `ZinAlcaria` (AI 31) | opening |
| 6022 | 22 | Welcome to the festival of martyrs! | `Belizabeth` (AI 32) | opening |
| 6023 | 23 | I'm tired of dealing with insects.  I'll snap you like twigs. | `Belizabeth` (AI 32) | after 300 s of fighting: it starts to heal |
| 6024 | 24 | I shall be ensconced by maddening flame! | `Kirhiross` (AI 33) | element change 1 |
| 6025 | 25 | I am the handiwork of Etain herself, worthy of adoration! | `CrypticOne` (AI 34) | opening |
| 6026 | 26 | Raise the Apsosion Shield! | `CrypticOne` (AI 34) | every 240 s: hits heal it for 45 s |
| 6027 | 27 | The power of water will protect me | `Kirhiross` (AI 33) | element change 2 |
| 6028 | 28 | The firmness of land will protect me. | `Kirhiross` (AI 33) | element change 3 |
| 6029 | 29 | Curl around me, O wind~! | `Kirhiross` (AI 33) | element change 4 |
| 6030 | 30 | Everything will be as nothing.. | `Kirhiross` (AI 33) | element change 5 |
| 6031 | 31 | You have passed the line of no return. Come, worthless one! | `Haruhion` (AI 35), `Freezing` (36) | opening (no monster) |
| — | 32 | (not in the file: nothing said) | `CrypticOne` (AI 34) | when the 45 s of healing end |
| — | 33 | (not in the file: nothing said) | `Belizabeth` (AI 32) | when the 35 s of healing end |
| **6401** | — | Do you think you can leave this place while you still breathe? | **Dios Exiel 2471** (Lua 115) | the first time it is attacked |
| **6402** | — | Guards! Kill them! | **Dios Exiel 2471** (Lua 115) | in the fight, once, before it calls two floor bosses |
| **6403** | — | You can't win this fight. | **Dios Exiel 2470** (Lua 116) | at HP ≤ 50 %, once |
| **6404** | — | How dare you... I will kill all of you !! | **Dios Exiel 2472** (Lua 117) | at HP ≤ 30 %, once (again after it resets) |

The Lua lines and what follows them are in [Party instance zones](instances.md) section 16.

## 3. The scripts and their monsters

Confirmed, `Data/mob_ai.ini` (AI number → file), the `.ai` files (the `Say` and `Say2` tokens and the transitions into their states),
`original_defs.mobs` and the spawn areas and boss groups. Names are the US client's.

| AI | file | monsters | where |
|---|---|---|---|
| 14 | `Spink1` | Ides Sphinx Confirmist 1550, Ides Sphinx Absolute Ruler 1551; First to Forth Ridler 1610 to 1613 | maps 46; 47 |
| 17 | `SorpNamed1` | High Priest Canis Femin 1518, High Priest Iona Cragion 1522; Caliph Atemis Erucan 1559, Caliph Lacryma Apisus 1561, Prophet Kipella Turusicer 1563; Shultan Erpolrin Capsiwoo 1521 (not placed) | maps 45; 46 |
| 18 | `SorpNamed2` | Shultan Talat Alimoss 1519, High Priest Doorumnan 1520, Shultan Saladis Piper 1523; Champion Keratis 1560, Champion Hyodoras 1562 | maps 45; 46 |
| 20 | `CrocoNamed1` | Ssendo 1630, Coontam 1632 | map 47 |
| 21 | `CrocoNamed2` | Mukam 1629, Atan 1634 | map 47 |
| 22 | `CrocoNamed3` | Rakhan 1631, Lantul 1633 | map 47 |
| 26 | `TrollNamed1` | Zululu 1635, Lapalu 1636, Pantanu 1637 | map 47 |
| 27 | `HellTouthNamed1` | Viduras 1638, Brando 1639, Quantarus 1640, Heraqul 1641 | map 47 |
| 28 | `Deinos` | Deinos The Dream 1704, 1804 | boss groups, maps 48, 49 |
| 29 | `Parridalis` | Parrdalis 1701, 1801 | boss groups, maps 48, 49 |
| 30 | `Alcarian` | Alcarian The Flame 1714, 1814 | boss groups, maps 48, 49 (stage 1) |
| 31 | `ZinAlcaria` | Alcarian The Flame 1703, 1803 | the same groups, stage 2, and Alcarian's summon |
| 32 | `Belizabeth` | Blizabeth Eathory 1702, 1802 | boss groups, maps 48, 49 |
| 33 | `Kirhiross` | Kirhiross 1705, 1805; Nantarios 1706, 1806 | boss groups, maps 48, 49 |
| 34 | `CrypticOne` | Cryptic One 1700, 1800 | boss groups, maps 48, 49 (stage 1; stage 2 is Cryptic the Immortal 1716, 1816, AI 1) |
| 23 to 25 | `OrcNamed1` to `3` | none | |
| 35, 36 | `Haruhion`, `Freezing` | none on the server (the client's data gives 35 to Haruhion 1978 and 36 to Freezing Mirage 1977 and Cryptic the Immortal 1716, 1816: `docs/original/mob-ai.md` section 5) | |
| 115 to 117 | Lua `115.lua` to `117.lua` | Dios Exiel 2471, 2470, 2472 | Cailum Sacra (maps 65 to 67; 2470 and 2472 are made by the scripts) |

The boss groups of maps 48 and 49 are those of `Obelisk.ini` (`docs/original/obelisks.md` section 11); the maps are entered through the
portals the two dragon groups open.

**The opening line of the named monsters** (AI 14, 17, 18, 20 to 27; the same shape in all eleven files): the start state goes to `Idle`.
From `Idle`, when the counter `nCount` is 0 and the monster has just found a player (`IsFirstAttack`) or has been hit (`IsAttackMe`), it
goes to `SetSay`, which draws `rand() % 100`: **above 50** (49 of 100 values) the first line, otherwise (51 of 100) the second. Then the
fight. `nCount` rises at the script's first HP phase. When the fight ends (`IsBattle` 0) the machine goes back to its start; when the
monster has been home its variables are cleared (`CMob::ResetFSM`). So a monster that is pulled, left and pulled again before its first
phase says a line each time.

**The bosses** (AI 28 to 34) say their opening line the same way, without the draw. Then:

- **`SorpNamed2`** (AI 18), at HP ≤ 45 %: skill 95, 1.5 s later skill 40 with line 9.
- **`Alcarian`** (AI 30), at HP ≤ 10 %: line 19; 2 s later line 20, `SetSummonMob(1703, 1.0, 1, 4)` (one Alcarian The Flame of AI 31)
  and skill 254. Once.
- **`ZinAlcaria`** (AI 31) turns off every attack kind for its first 5 s (`SetEnableOnlyAttackType(5)`, which the original writes and
  never reads, so it can be hurt anyway: `docs/original/scripted-mobs.md` section 7), then says its opening line on contact.
- **`Belizabeth`** (AI 32), 300 s into the fight: line 23, recovery on every second; 1 s later its recovery ratio is 0.35 and its REC 0.55
  for 35 s; then both back to 1 and line 33 (missing). The 300 s count restarts.
- **`Kirhiross`** (AI 33), every 120 s of fighting (plus 1 s): it changes its defence element (`SerChangeRegist` 1, 2, 3, 4, 5 in turn)
  and says the line of that change: 24, 27, 28, 29, 30, then 24 again.
- **`CrypticOne`** (AI 34), every 240 s: line 26 and it counts the hits it takes; for 45 s each check adds 50,000 HP per hit counted
  (`AddEnergy`, `0x0610`); then line 32 (missing). Every 35 s it also targets the attacker with the most HP and uses skill 251.

## 4. The new server

- The AI scripts' lines go out as **`0x1109` with text**, and the text is this project's English (`world.ai_strings.english`; deviation 1
  of `docs/original/scripted-mobs.md` section 12), not the Korean.
- The Lua bosses' `LuaSayByIndex` sends **`0x110A`** with the sysmsg id, as the original does, so 6401 to 6404 show as above
  (`docs/original/instances.md` section 18).
- The client's own English for the AI lines, 6001 to 6031, would be available by sending `0x110A` with 6000 + the line number instead;
  that would use the translation the players' client already has. This server does not do it (its own English is closer to the Korean).

## 5. Later clients and episodes

A later client's system-message table (an English `usa.txt` of 2,355 lines, from a private server's client; not kept in the repository)
was compared with the 2010 client's, together with a community set of Lua scripts for EP5's returned Cryptic One
([Party instance zones](instances.md) section 19). Confirmed, the files, unless marked.

- **Nothing was removed or renumbered.** 970 lines are added and 610 reworded. **6001 to 6031 and 6401 to 6404 are unchanged**, so
  sections 2 and 3 hold for that client too.
- **6032 to 6131 are new monster lines.** The scripts say **78 of the 100** with `LuaSayByIndex`, each with the line's Korean kept above
  the call as a commented-out `LuaSay`, and every Korean line means what the English one says. The other 22 (6032 to 6037, 6054, 6055,
  6062, 6065 to 6068, 6075, 6076, 6088, 6110, 6113, 6117, 6119, 6120, 6129) have no speaker in any file we have. They are probably the
  lines of the EP5.4 Lua monsters whose scripts are still missing (121 to 124, 150, 180 to 185; inferred). 6065 and 6068 name "Fury
  Kirhiross".
- **Other monster lines in that client** belong to its server's own events. 10700 to 10706 are under the file's comment `골드몬스터`
  ("gold monster"): 10700 is the notice "Troublemaking goats have appeared in [<map>]! Take them to Santa Claus!", and 10701 to 10706 the
  goats' lines. 13850 to 13859 are notices of rabbit and zombie events, and 5632 and 5633 a zombie event's. **13860 to 13863, 13866 to
  13876 and 13896 to 13907 are boss speech** of that server's own content ("I AM THE CINDERWALKER!", "THE END......"); no script for them
  was given. 13864 and 13865 inside the first block are system lines.

| sysmsg | speaker (AI, monster; the script's header) | when |
|---|---|---|
| 6038 to 6043 | 126, 3028, the Frozen Man-eating Hydra | 6038 first contact; 6039 reset or home; 6040 in the fight; 6041 at HP ≤ 70 %, 6042 at ≤ 30 % (each before its guards); 6043 death |
| 6044 to 6052 | 127, 3019, Nantarios (lava zone) | 6044 first contact; 6045 death; 6046 to 6052 one each in the HP bands 98–95, 93–85, 80–70, 70–50, 50–40, 30–20 and ≤ 20 % |
| 6053, 6069 to 6074 | 133, 3043 (dimensional rift) | 6053 first contact; 6069 death; 6070 or 6073 when it calls its guards (by a random draw), 6071 and 6072 while they live, 6074 at each band's end (HP bands 90–70, 60–40, 30–10 %) |
| 6056 to 6061, 6063, 6064 | 129, 3005, the "Peri Queen" (crystal mine) | 6056 first contact; 6057 reset; 6058 death; 6059 with the first calls; 6060 when none of its summons is left, 6061 with each larger call; 6063 at ≤ 70 %, 6064 at ≤ 30 % |
| 6077 to 6086 | 135, 3041, the throne's boss 2 | 6077 first contact; 6078 reset; 6079 death; 6080 and 6081 at 95 %; 6082, then 6083 (after a random draw), at 85, 75, 65, 55 %; 6084 and 6085 at 50 % (then its 4 guards); 6086 at ≤ 2 % |
| 6078 | 145, 3042, Cryptic One's third form | reset (the same line as 135's) |
| 6087, 6089 to 6091 | 136 to 139, 3030 to 3033, the boss's guards | 6087 (3030) first contact; at ≤ 95 %: 6091 (3030 and 3033), 6089 (3031), 6090 (3032) |
| 6092 to 6100 | 141, 3023, "Delizabeth Etory" (otherworld garden) | 6092 first contact; 6093 reset; 6094 death; 6095 with its 3070 summons; 6096 to 6098 as its 3071 come; 6099 at every 5 % from 50 % to 15 %; 6100 at 10 % |
| 6101 to 6108 | 144, 3010, the "Bulldozer" (lava falls) | 6101 first contact; 6102 reset; 6105 death; 6103 with a fire attack (random); 6104 after it; 6107 at HP ≤ 90 %, on a timer, with skill 387 on a player drawn at random from its aggro list (`LuaGetNumberAggro`); 6106 with each 3069 from 65 % down; 6108 at 5 % |
| 6109, 6111, 6112, 6114, 6121, 6130, 6131 | 145, 3042, the third form's body | 6109 first contact; 6114 death; 6111 when its double appears (10 s in); 6112 every 30 s; 6130 then 6131 at every 10 % from 90 % to 20 %; 6121 at 10 % |
| 6115, 6116, 6118, 6122 | 146, 3072, the double | 6115 first contact; 6116 death; 6118 before it calls 2 × 3073; 6122 later in the fight |
| 6123 to 6128 | 147, 3040, the throne's boss 1 | 6123 first contact; 6124 reset; 6125 death; 6126, 6127 from 95 % at timed points; 6128 later |

**Are newer servers Lua-only?** The files cannot say. AI 0 to 9 are not files at all: they are code in `ps_game.exe` (`docs/original/mob-ai.md`),
and EP5.4 (2011) still ships the 180 AI files and `mob_ai.ini` of EP4.5. Its new monsters use Lua (AI 121 to 185), but its old ones keep
their AI numbers.
