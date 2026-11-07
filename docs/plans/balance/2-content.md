# Balance analysis, part 2: what exists

The inventory of what players can have and fight: every item, lapis, Lapisia, enchant step, skill and monster, where each comes from,
what it is worth against its level, where the factions differ and where the client's copy differs, with the model's numbers where they
help. **Analysis only**: nothing in the game, the database (read-only queries), the server code or the client was changed, and nothing
here is a decision. Part 1 (`docs/plans/balance/1-formulas.md`) explains the formulas and the model; part 3
(`docs/plans/balance/3-classes.md`) compares the classes.

**Status words** as in part 1: **verified** = read in the server's code or the data as the server loads it (the query or
`file::function` is named); **modelled** = an output of `tools/balance/` (expected values, see its README); **inferred** = a reasonable
reading not checked in code or play. **Names**: the client's English names (`client.items`, `client.skills`) where the client has one;
otherwise the `defs` name, said so; monsters by id (their `defs` names are Chinese and the client's `Monster.SData` is not joined here;
no name is guessed).

**The tables**: `tools/balance/content.py` builds the CSV tables in `data/balance/` (git-ignored; about four minutes, `--refresh` rereads
the database, `--quick` leaves out the model's numbers); `tools/balance/content_report.py` prints this document's tables from them. This
document keeps what a reader needs; the CSVs have every row:

| CSV | what |
|---|---|
| `items.csv` | every weapon, armour piece, shield, accessory and cloak (14,435 rows): limits, numbers, crafted stats, enchanting, price, drop grade, origins, its value against its peers |
| `item_brackets.csv` | per kind and level: count, median, lowest and highest value |
| `item_pairs.csv`, `item_duplicates.csv` | each Light item with its Fury counterpart and what differs; items equal in every number |
| `item_skills.csv` | items used from the bags that apply a skill |
| `lapis.csv`, `tools.csv`, `enchant.csv` | lapis; Lapisia, hammers, charms, runes, enchant materials, boxes; the enchant steps |
| `skills.csv`, `class_skills_15/30/60.csv` | every learnable skill row; each class's attack skills at 15, 30, 60 with the model's damage |
| `mobs.csv` | every monster: numbers, attack per second, experience per HP, placement, the default fighter's and mage's fight |
| `origins.csv` | every way of getting every item (123,091 rows), with its chance |
| `client_items.csv`, `client_skills.csv` | where the client's copy differs, for items and skills a player can get or learn |

## 0. The maintainer's goals (asked 2026-10-26)

| question | answer |
|---|---|
| PvE or PvP? | **Both equally.** |
| Which levels matter? | **The level cap (60)** most; also **15 and 30**, the levels of the two smaller PvP maps. |
| Which modes? | **Mostly ultimate** on private servers, a mix on the original server; ultimate used to be gated behind a level. The maintainer wants to discuss **why keep easy, normal and hard at all if everyone plays ultimate** (open, for part 3). |
| How far from the original? | **A rebalance**, and **lay out what could or should be reworked**. |
| Faction differences? | **Decide after the analysis**: show where the factions differ today first. |
| Lapis and enchanting? | **Fully in scope**: strength, success rates and costs. |

## 1. What stands out (details in the sections named)

1. **Half the equipment cannot be had** (section 3.1): of 9,720 usable named rows, 5,201 are obtainable in play; 2,130 have no origin at
   all, 1,780 come only from the mall (almost all through its three Item Pockets: operator points) and 609 only from sequence items.
   The best obtainable weapon of most classes at 60 is a **hard-mode quest reward**.
2. **No set bonuses exist in this build** (verified, section 3.11): an EP6 feature.
3. **Armour is worth its HP, MP and SP, not its defence** (section 3.4): the best obtainable level-60 set gives defence 93 against
   monster attacks of 961, but 1,813 HP. **Crafted stats can outweigh the item** (section 3.5): one crafted HP number on a level-60
   piece is up to 1,300 HP, more than the piece's own; the roll cannot be redone (the Recreation Rune has no origin).
4. **Enchanting past about +6 (weapon) or +7 (armour) is out of reach without the mall** (section 4.3, with the rates of the original
   patch's `ItemEnchant.ini`, which the server now loads): the only Lapisia players can get are **dangerous** (a failure destroys the
   item), they drop only at levels 59 to 63 in Cailum Sacra, and a weapon at +6 takes about 150 of them and 41 weapons, at +7 about
   1,500 and 413 weapons (3.7 billion). Armour is cheaper: +7 takes 146 Lapisia and 28 pieces. The safe Lapisia, the Recreation Rune and
   the Lucky Charm's enchant use are not in play.
5. **The strongest lapis are not in the game** (section 4.1): the state lapis 110 to 139, among them the 90 % and 100 % HP lapis 136
   and 138 that part 1 worried about, have no origin (grades 38 to 40, 98, 99 that nothing drops). The obtainable top is grade 37
   (+30 a stat, +32 attack, absorption 75) at a **1 % link rate**; the Lv7 debuff lapis (grade 36) link at 4 %, better than the
   weaker Lv6 (grade 37) at 1 %.
6. **The Light and Fury items are mirrored in 3,020 pairs and differ in 1,174** (section 3.8). The differences are systematic: Light
   swords have more attack and less spread than the Fury axes (same mean), **Fury shields have double the magic defence**, the job
   flags of one-handed swords and double swords differ, and two **Fury-only quest bracelets (all stats +2, HP 300, SP 200) have Light
   counterparts worth a seventh** of them.
7. **Percentage skills meet a wall in PvE** (section 5.5): the level-60 skills 182 Syeeric Touch and 183 Pantera Vision (state 8) are
   resisted by 1,424 of 1,436 placed monsters; 180 and 184 by 491 (270 of the 472 of level 51 and up). They are PvP
   skills. In PvP the mage's **Flame Shock Lv2 takes more than the mirrored mage's HP in one use** with its damage over time (modelled,
   section 5.3).
8. **Every class has attack skills that are never worth casting** at 60 (section 5.4): 2 to 11 per class are matched by another skill
   in every way, and 1 to 4 are below the normal blow on both monsters and players (the counts leave area and debuff effects aside).
9. **Monsters step at level 51** (section 6): median defence 47 → 422, attack per second 101 → 361, experience per HP 0.29 → 0.82, and
   the part 1 default builds die before killing most of them. From 59 on, monsters have 200,000 to 400,000 HP and are party or boss
   content.
10. **Guild-house merchants change nothing in the item supply** (section 2.4): 304 items are sold there, 299 also come from elsewhere;
    the five that do not are an Etin and four potions. Reaching the house maps needs a ranked guild with a bought, opened house.
11. **A merchant's faction is not checked when buying** (verified, `server.rs::buy`): the "Light" merchants of the shared PvP maps 18
    and 30 (the Exp Removal Pills, +1 attack speed) serve both factions.
12. **The client's copy differs in 132 obtainable equipment and lapis rows and 30 skill rows** (section 3.10): a change of numbers needs
    a client data patch to show.

## 2. Where items come from

### 2.1 The rules (verified; `tools/balance/data.py::origins`, checked against `drops.rs`, `docs/original/item-origins.md`, `docs/original/quest-drops.md`, `docs/original/item-mall.md`)

| kind | rule | chance | source |
|---|---|---|---|
| shop | a merchant (NPC type 1) that a map places, by its goods list | 1 | `world.npc_shop_items` joined with `world.map_npcs` (query `merchants`) |
| drop | a **placed** monster's drop entries: spawn areas, boss groups and their followers, obelisks and their guards, and the Lua-summoned Cailum Sacra bosses (2464 to 2467, 2470, 2472, 2473: `data.SCRIPT_SUMMONED`, `docs/original/instances.md` section 8) | entries 1 to 6 in %, 7 to 9 in millionths, ÷ the items of the grade (`drops::roll`); limited items (`drop` > 0) take a share but never come | `defs.mob_items`, `defs.items.grade` |
| sequence | every `sequence`-th monster made, by the level tens of placed monsters with money (`DropTables::sequence_item`) | 1 / sequence ÷ the grade's items | `world.sequence_groups` |
| box | an item of special 42 opens set `reqvg` of `world.item_create` (`DropTables::open_box`), counted when the box itself has an origin (repeated until nothing changes) | the line's weight share ÷ the grade's items | `world.item_create` |
| mall box | a box whose only origins are the mall | as box | |
| quest | a reward of a quest that can be started (`QuestStartRequest`, `docs/original/quest.md` section 5): its **starting NPC placed** (start types 0 to 2) and, for types 2 to 4, its **start item obtainable**; start items come from quests and boxes too, so quests and boxes are opened together until nothing changes | 1 | `world.quest_results`, `world.quests` |
| start | starting items | 1 | `defs.base_items_defs` |
| mall | an item of a product (points come only from the operator: nothing in play gives them) | 1 | `defs.product_list` |

An item is **obtainable** when it has an origin other than the mall, a mall box or sequence items (`content.py::origin_summary`;
`model.RARE`: one monster in 16,001 to 30,000,001 made is not a source a player can plan on). The gear picker of part 1 uses the same
rule.

### 2.2 How much each kind gives (verified, `origins.csv`)

| kind | origins (rows) | items |
|---|---|---|
| drop | 105,149 | 4,491 |
| mall box | 11,377 | 5,097 |
| shop | 2,832 | 695 |
| box | 1,787 | 1,284 |
| sequence | 1,065 | 870 |
| quest | 760 | 605 |
| mall | 107 | 92 |
| start | 14 | 14 |

The mall's reach is almost all its three **Item Pockets** (100/78 Normal, 100/79 Hard, 100/80 Ultimate; 249, 299 and 399 points; sets 1
to 3 of `world.item_create`): **2,671, 3,931 and 4,605 different items**. The largest boxes in play are 44/118 Angel's Chest (a quest
reward, 643 items), 44/201 and 44/200 Treasure Chest of Dragon (boss followers of levels 58 to 60, 555 and 388 items).

### 2.3 Three corrections to part 1's first cut (verified)

1. **117 of the 537 quests with item rewards start at NPCs no map places** (type 7: 305 and 299, 31 and 28 quests, the event NPCs of
   Santa, Chuseok and Rudolph; the recruiters 451 to 454; the explorers 536 to 538; 20 more NPCs with one to three each): their
   rewards are not obtainable.
2. **The old box rule** marked items of mall-only boxes as plain "box"; they are now "mall box" and do not count as obtainable.
3. **Quests that start from an item** (start types 2 to 4) counted as open whatever the item; now they need the start item (and type 2
   its NPC). **19 more quests** are closed by this (start type 2: the event chains 1533 to 1551, whose start items come only from
   quests of unplaced event NPCs). With 1 and 3, **401 of the 537 quests with item rewards can be started**; 55 items are rewarded only
   by the 136 closed ones (52 of them now have no origin at all, 2 are mall only).

The gear picker takes slightly different items, so **part 1's sample tables (section 7.3) were rerun and replaced** (e.g. the level-15
fighter's HP 1,653 → 1,485, the defender's mitigation at 15 0.90 → 0.68).

### 2.4 Guild-house merchants (verified data; `docs/original/guild-house.md` sections 2, 7, 9)

NPCs 196 to 225 stand on maps 50 to 54: the house maps 51 (Light) and 52 (Fury; `CreateType` H), the halls 53 and 54 (ordinary maps)
and the ranking battle map 50 (`CreateType` R). They count as shops (the rule above), which changes almost nothing (query over
`origins.csv`, merchants 196 to 225):

- **304 items** are sold there; **299 also drop or are sold elsewhere** (the weapons and armour of x/22, x/42, x/62, x/7, x/17, x/27,
  …, the grade-31 to 33 lapis 30/1 to 30/101 of the general-goods merchants 220 to 225). **Only five come from there alone**: 43/2
  [10] Etin (merchant 219, map 50) and four potions 44/99, 44/102, 44/105, 44/108 (Untouchable potion Lv1, Continual Healing, Stamina
  and Mana Potion Lv1; merchants 203 and 214, the level-0 potion merchants of the houses).
- **What reaching them needs.** The house maps: a portal on map 35 or 53 (Light) or 36 or 54 (Fury), or the Guild House Teleport
  Stone, passing only for a guild ranked 1
  to 30 that **bought the house (50,000,000)** and whose house **opened at a ranking** (`byUseHouse` 1). Inside, a merchant of a higher
  level is locked until the guild upgrades it with Etin; general goods need a ranking of 30 or better, potions 6 or better
  (`guild_house::House::shop_rate`); the guild's discount is 5 to 20 % (potions 10 to 40 %). The halls 53 and 54 (merchants 205, 216,
  potions, no guild check) are reached **only from the house or the ranking battle**: no portal leads in from outside, and the two
  gatekeepers with the 9,900 trip to the halls (44 and 45 in `world.npc_gates`) are placed on no map (query `world.map_npcs`). The
  ranking battle map 50 (merchant 219, Etin) is reached only through the halls' portals, on Sunday 21:00 to 22:00.
- **So on a fresh server the circle is closed** (inferred from the data): the ranking needs battle points, the battle needs the halls,
  the halls need the house or the battle. Whether the original placed gatekeepers 44 and 45 somewhere this data lacks is not checked
  (part 3 note; the live tests reached map 53 by operator moves).

### 2.5 Merchants' sides (verified, `tools/world/import.py` NPC rows, `server.rs::buy`)

`world.npcs.country` is the NPC file's country turned 1 → 0 Light, 2 → 1 Fury, other → 2 both. **Buying does not look at it**: any
player who reaches a merchant may buy. A merchant's side matters only where it stands in one faction's own town; on shared maps (18,
30, the halls of the other side when reached) it is a label. Example: merchants 191 to 194 (country Light) sell the Exp Removal Pills
(44/213, 44/214: attack speed +1, magic attack +65 / +205 for 300 s) on maps 18 and 30, the level-15 and level-30 PvP maps, to both
factions.

## 3. Items

### 3.1 Counts (verified, `items.csv`)

14,435 equipment rows (real types 1 to 25); 4,706 are placeholders (no name, `???`, `...`, `??`, `…`); of the **9,729 named, 9,720 are
usable** (some class, race and mode fits, level 60 or less) and **5,201 obtainable**:

| kind | named | obtainable | mall or mall box only | sequence (and mall) only | no origin |
|---|---|---|---|---|---|
| weapons (15 kinds) | 6,847 | 3,609 | 1,486 | 531 | 1,221 |
| helmet, upper, lower, gloves, boots | 2,010 | 1,085 | 186 | 48 | 691 |
| shield | 512 | 262 | 102 | 30 | 118 |
| ring, amulet, bracelet | 180 | 119 | 0 | 0 | 61 |
| cloak | 180 | 126 | 6 | 0 | 48 |
| **all** | **9,729** | **5,201** | **1,780** | **609** | **2,139** |

(Per kind: `content_report.py --section counts`.) By the lowest mode that may wear them:
easy 3,400 usable rows (1,802 obtainable), normal 3,506 (2,231), hard 1,918 (649), **ultimate 896 (519)**.

### 3.2 Value per kind and level (verified data; `item_brackets.csv`)

The main number: weapons their **mean attack** (`attack + (spread − 1) / 2`); armour and shields **defence + magic defence + stat
points**; accessories and cloaks **stat points + effects** (stat points: a stat point 1, 5 HP/MP/SP 1; `content.py::main_value`).
Median over the obtainable, usable items of the band (how many):

| kind | 11-15 | 26-30 | 46-50 | 56-60 | per level at 60 |
|---|---|---|---|---|---|
| one-handed sword / axe | 33 (8) | 74 (15) | 134 / 135 (29) | 176 (12) | 2.9 |
| two-handed sword / axe | 46 (8) | - | 178 (35) | 225 (12) | 3.8 |
| double sword | 36 (16) | 80 (30) | 146 (70) | 190 (24) | 3.2 |
| spear | 56 (16) | 95 (30) | 214 (70) | 272 (26) | 4.5 |
| one-handed mace | 40 (16) | 88 (30) | 160 (58) | 222 (26) | 3.7 |
| two-handed hammer | 49 (16) | 108 (30) | 196 (70) | 255 (24) | 4.3 |
| short blade / claw | 32 (8) | - | 125 (35) | 164 (25) | 2.7 |
| short sword | 24 (8) | 54 (15) | 94 (44) | 138 (26) | 2.3 |
| javelin / crossbow | 58 (8) | 98 (15) | 220 (35) | 280 (13) | 4.7 |
| bow | 36 (16) | - | 138 (70) | 175 (24) | 2.9 |
| staff | 30 (14) | 60 (26) | 120 (66) | 174 (26) | 2.9 |
| upper armour | 42 (12) | - | 162 (30) | 212 (42) | 3.5 |
| lower armour | - | - | - | 164 (42) | 2.7 |
| helmet | - | 22 (24) | - | 41 (42) | 0.7 |
| gloves / boots | 21 (12) / - | 40 / 57 (24) | 72 (36) / - | 82 (51-55) / 111 (42) | 1.4 / 1.9 |
| shield | 60 (12) | - | 223 (54) | 281 (24) | 4.7 |

The weapon's attack speed weighs as much as its attack: the javelin and crossbow (speed 4, 2,250 ms) have the most attack a second at 60
(135 a second against 95 to 121 for the others; `items.csv` `attack_per_s`). Accessories have medians of 3 to 18: their worth is in a
few quest rows (section 3.7) and in crafted stats and lapis.

### 3.3 Best obtainable weapon per class at 15, 30, 60 (verified data, ranked by mean attack; `content_report.py --section weapons`)

Ultimate wears every mode's items, so the best of any mode up to the level:

| class (Light / Fury) | 15 | 30 | 60 |
|---|---|---|---|
| fighter / warrior | Pike / Glaive (spear, L14, easy) 56 | Stormrage / Lethe (hammer, L30, easy) 108 | **Mellan Skia** (spear, L60, hard, quest) 298, all stats +15 |
| defender / guardian | Heavy Hammer / Double Pounder (hammer, L12) 49 | Stormrage / Lethe 108 | Giant Hammer / Masher Claw (hammer, L59, hard) 264, STR 40 DEX 8 |
| ranger / assassin | Blade Ace (short blade, L13) 32 | Fatal Shear / Rapid Fire (L25, normal, quest) 70 / 69, DEX 20 LUC 10 | Buse Nuikterion (claw) 180 / Heartseeker (short blade) 178 (L60, hard, quest) |
| archer / hunter | Lightning Bow (crossbow) / Piercing Javelin (L14) 58 | Hresvelgr / Spectrum Javelin (L26) 98 | Death Foshubil (crossbow) / Artraiya Adein (javelin) (L60, hard, quest) 304 |
| mage / pagan | Cross Staff / Black Bird (staff, L11) 30 | Wind Storm / Isobaric (staff, L25, normal, quest) 74 | Draconic Staff / Ele Shutolz (staff, L60, hard, quest) 190 |
| priest / oracle | Cross Staff / Black Bird 30 | Blind Devotion / Endure Flame (quest) 74 | Tiamat / Pearl Staff (staff, L59, hard) 174 |

- **At 60 the best are hard-mode quest rewards**; easy and normal characters cannot wear them.
- **Unobtainable rows above the best**: at 15 a staff of 36 (mage, priest); at 30 a hammer of 114 (fighter, warrior) and a crossbow /
  javelin of 122 (archer, hunter); at 60 the priest's staff 12/213 Zelen Wandern (190, no origin; the oracle has no such row) and the
  Fury hammer 8/203 Draco Cranio (290, no origin; the defender has no such row). The mage's and pagan's level-60 staves (190) are quest
  rewards; the priest's and oracle's best obtainable are drops of 174.
- The archer's and hunter's ranged weapons have the highest attack at every level; the ranger's and assassin's the lowest (part 1: its
  DEX and skill multipliers make up for it).

### 3.4 Best obtainable armour set per class (verified data; the best-valued helmet, upper, lower, gloves and boots)

| level | classes | pieces | defence | magic defence | HP | MP | SP | stat points | crafted (expected / highest) |
|---|---|---|---|---|---|---|---|---|---|
| 15 | fighter, defender (and Fury) | 4 | 14 | 7 | 382 | 0 | 86 | 98 | 103 / 870 |
| 15 | ranger, archer | 4 | 11 | 11 | 239 | 88 | 129 | 95 | 103 / 870 |
| 15 | mage, priest | 4 | 7 | 16 | 142 | 291 | 0 | 91 | 103 / 870 |
| 30 | fighter, defender | 5 | 34 | 15 | 898 | 0 | 201 | 246 | 191 / 2,058 |
| 30 | ranger, archer | 5 | 24 | 24 | 552 | 215 | 301 | 240 | 191 / 2,058 |
| 30 | mage, priest | 5 | 15 | 37 | 335 | 674 | 0 | 228 | 191 / 2,058 |
| 60 | fighter, defender | 5 | **93** | 46 | **1,813** | 0 | 440 | 499 | 321 / 3,780 |
| 60 | ranger, archer | 5 | 67 | 67 | 1,103 | 437 | 644 | 485 | 321 / 3,780 |
| 60 | mage, priest | 5 | 46 | 98 | 688 | 1,388 | 0 | 463 | 321 / 3,780 |

Mirrored classes get the same numbers (the pieces differ by name). Armour comes in three families by job flags (the fighter's and
defender's, the ranger's and archer's, the mage's and priest's), not per class. **At 60 the set's 93 defence against monster attacks of
961 to 1,040** (part 1 section 7.3) cuts little; its HP is a fifth of the character's. No helmet is obtainable below level 30.

### 3.5 Crafted stats and the enchant effect (verified rule `lapis::roll_craft`, `docs/original/item-origins.md` section 3; modelled values)

A craftable item gets up to nine numbers when it drops (weapons and accessories: the six stats; armour and cloaks: the six stats and HP,
MP, SP), each the **smallest of three draws of 0 to `reqwis`**; a crafted HP, MP or SP point is **100**. Obtainable craftable items:
4,956. Typical ranges (`items.csv` `craft_numbers`, `craft_max`):

| | how many numbers | top of each | highest | expected |
|---|---|---|---|---|
| armour, level 11-15 | 5 of 9 | 4 | | |
| armour, level 26-30 | 3 or 4 of 9 | 6 or 8 | | |
| armour, level 56-60 | 3 of 9 | 13 | three HP/MP/SP: **1,300 each** (one piece) | about 64 stat points a piece |
| weapons, level 56-60 | 3 of 6 stats | 23 (26 for 12 rows) | 69 stat points | about 17 |

So **a lucky level-60 armour piece holds up to 1,300 HP from one number**, and a set up to 6,500, more than the class's ultimate base HP
at 60 (part 1 section 3.1: 6,019 for the fighter); the chance of a 13 is 1 in 2,744 per number. **The Recreation Rune (100/171), which
rerolls with two draws, has no origin**, so the roll an item drops with is the roll it keeps.

**The enchant effect** (verified, `lapis::enchant_bonus` with the table from `Data/ItemEnchant.ini`): armour **+5 absorption a step**
(a flat cut off every blow); weapon **+7 attack a step to +3, then 10 or 13, up to 16 and more a step** (+4 = 31, +5 = 41, +7 = 64,
+10 = 106, +20 = 311). The built-in tables the server used before 2026-11-06 gave 7 a step all the way. 4,944 obtainable items are enchantable (craftable with a material). What the steps cost is section 4.3.

### 3.6 Item skills (verified, `item_use::effect`; `item_skills.csv`)

203 items used from the bags apply a skill (181 abilities, 9 heal over time, 8 transformations, 4 blocks, 1 attack); **46 are
obtainable**. The strongest obtainable:

| item | skill | effect | time | origin |
|---|---|---|---|---|
| 44/91 Parrdalis of Claw | 270/1 | **critical +300** (a critical every blow, part 1 section 4.2) | 600 s | drop, monsters 1715 and 1815 (level 62, maps 48 and 49), 1 in 10 |
| (44/153 Yummy Chocolate!, the same skill) | 270/1 | critical +300 | 600 s | **not obtainable**: the repeatable event quests 1541, 1551 start from items no open quest gives (section 2.3) |
| 44/90 Parrdalis of Heart, 44/89 of Wing | 269/1, 268/1 | attack speed +1, move speed +1 | 600 s | the same monsters |
| 100/61 Clean Hit Nostrum XT | 242/1 | melee and ranged attack +65 | 3,600 s | quests 1166 / 1192, levels 29 to 60, hard and up |
| 100/62 Defense Nostrum XT | 243/1 | melee, ranged, magic defence +50 | 3,600 s | quests 1164 / 1194 |
| 100/9 Weaponmastery Nostrum | 211/1 | attack speed +1 | 3,600 s | the same quests; mall (98 points) |
| 44/214 Large Exp Removal Pill (44/213 Small) | 273/2 (273/1) | attack speed +1, magic attack +205 (+65) | 300 s | merchants 193, 194 on map 30 (191, 192 on map 18), 19,800 (9,900) |
| 44/112, 44/115 (beers, champagnes) | 197/2, 198/2 | max HP +500 and critical +5; melee hit and defence +15 | 60 s | merchants, 43,600 |

The Nostrum quests (1164, 1166, 1192, 1194) are not repeatable (`world.quests.repeat_count` 0), so their items come once per
character; Angel's Chest (quest 1435) is repeatable. The Parrdalis of Claw, a 1-in-10 drop at level 62, is the outlier of this list.

### 3.7 Outliers (verified data; an obtainable item at 1.5 times its peers' median or above, or 0.67 or below; peers: the same kind within 3 levels)

| item | kind, level, mode | value | peers' median | origin |
|---|---|---|---|---|
| 40/202 Dignity Loop, 40/203 Fineness Loop (Fury) | bracelet, 38, hard | 132 (all stats +2, HP 300, SP 200 …) | 14 | quests 1011, 1015 (Fury, levels 40 to 60) |
| 22/31 to 22/34 Freon, Heartbeat, Will, Shooting Star Ring | ring, 55, normal | 18 | 5 | drop |
| 40/5, 40/15 Safe / Wise Loop Lv5 | bracelet, 58, easy | 15 | 6 | merchants |
| 23/201 Compass of Kuluny | amulet, 34, easy | 28 | 13 | quest (Light) |
| 22/35 Noble, 22/41 Worship, 22/49 Heroic Ring (and the amulets and loops of the same names, levels 53 to 58) | accessories | **0** | 5 to 18 | drop, merchants |
| 22/2 Mystic Ring Lv2, 22/12 Craft Ring Lv2, 22/21 Innate Twin-Ring Lv1 | ring, 20 | 5 or 6 | 40 | merchants, quests |

Weapons and armour have no outliers by this measure (6 low staves, 12 high shields at most: the Fury shields' double magic defence,
section 3.8). The zero-valued accessories have no numbers of their own and are not craftable, only one or two lapis rooms (verified
data; prices up to 8.5 million): lapis carriers. The full list: `items.csv` columns `ratio`, `outlier`.

### 3.8 Light and Fury items (verified data; `item_pairs.csv`, `content.py::pair_rows`)

Each Light-only item (race rule 0 to 2) is paired with a Fury-only item (3 to 5) of the mirrored kind (Light one- and two-handed swords
and crossbows against Fury one- and two-handed axes and javelins), the same job flags, level and mode:

| status | pairs | both obtainable |
|---|---|---|
| mirrored (every number equal) | 3,020 | 1,627 |
| differs | 1,174 | 595 |
| no Fury counterpart | 77 | |
| no Light counterpart | 64 | |

What differs, systematically:

| kind | difference | pairs |
|---|---|---|
| two-handed sword / axe | Light more attack (sum 37,132 / 32,821), Fury more spread (4,336 / 14,118): **about the same mean, Light steadier** | 314 |
| one-handed sword / axe | the same (27,147 / 24,229 attack; 3,368 / 10,361 spread) | 308 |
| one-handed sword / axe | **job flags: the Light sword allows the archer, the Fury axe does not** | 308 |
| double sword | Light more attack, Fury more spread; **the Fury double sword allows the hunter, the Light one not the archer** | 287, 285 |
| shield | **Fury magic defence double** (812 / 1,555 summed) | 149 |
| two-handed sword / axe, double sword, hammer | Fury more STR (and REC, DEX on hammers), Light DEX | 20 to 35 |
| gloves | Light WIS, Fury INT | 28 |
| crossbow / javelin | Light STR only, Fury STR + LUC halves | 9 |
| cloak | small stat swaps | 3 to 9 |

The unpaired rows are mostly quest rewards whose counterpart is of another kind or level (e.g. the Fury claw Rapid Fire against the
Light short blade Fatal Shear; inferred from the names and levels). The one with weight: **the Fury bracelets Dignity and Fineness Loop
(value 132)** against the Light quest bracelets Arciel, Sinseal and Heliotrope Loop (levels 38 to 42, value 18). Together with the
Fury kill-count grades (part 1 section 6.3) these are the faction differences in what players have.

### 3.9 Duplicates and dead rows (verified data)

- **Duplicates**: 121 groups of named items equal in every number but their key and name (310 rows), 28 groups with an obtainable member;
  mostly claws (30 groups), short blades (18), helmets (16) (`item_duplicates.csv`).
- **Dead rows**: 4,706 placeholder rows; 2,139 named rows with no origin; 9 named rows no class, race and mode can wear (helmets);
  the 55 rewards of closed quests (section 2.3).

### 3.10 The client's copy (verified, `client_items.csv`, `client_skills.csv`; `docs/client/item-sdata-vs-defs.md`, `skill-monster-sdata-vs-defs.md`)

- **Items**: 492 equipment and lapis rows differ from `client.items`, **132 obtainable**: MP 39, buy and sell prices 36, attack and
  spread 24, lapis room 13, HP 12, SP 12, STR 10, LUC 8, DEX 6, the material count 6, the equip level 5, a few others. The tooltip shows
  the client's numbers; the server's apply.
- **Skills**: the 30 rows of the earlier comparison and Pollution Zone's `typeshow` 4 / 3: Potential Nimble's third ability (client
  +8 to +25, server none), Slow, Rapid Shot Lv3's added damage, Gruesome Attack's weapon list, the priest's cast times (Healing,
  Healing Prayer, Recovery: the client shows longer), Trap's class, Enhance Wind Spin's spear, Pollution Zone's points, weapons and
  keep time, Dark Panic's state.

### 3.11 Set bonuses (verified: there are none)

Nothing in `crates/`, `tools/` or `docs/` handles item sets, `ps_game.exe` has no symbol for them, and the EP4 client has no
`SetItem.SData`; sets are an EP6 feature (Parsec's `src/Parsec/Shaiya/SetItem/` (github.com/matigramirez/Parsec),
shaiya-episode-6's `sdev/include/shaiya/Synergy.cpp` (github.com/kurtekat/shaiya-episode-6)).

## 4. Lapis and the enchant economy

### 4.1 Lapis (verified data, `lapis.csv`; rates `lapis::base_rate`, `lapis::link`)

255 lapis rows, 233 named, **164 obtainable**. The link rate at 60 is `trunc(289 − 8 × grade + KC + level × 0.05)`, 1 to 50 (part 1
section 4.9); a Linking Hammer multiplies it by 110 to 450 %, premium by 1.5, a guild house smith adds 1 or 2 (section 2.4).

| grade | rate at 60 (with the 450 % hammer) | price a try | named / obtainable | what (obtainable) |
|---|---|---|---|---|
| 30 | 50 | 1,000 | 7 / 0 | |
| 31, 32, 33, 34 | 44, 36, 28, 20 | 4,095 to 41,280 | 58 / 58 | the six stats +3 to +7, HP/SP/MP, attack, spread, defence, magic defence (Lv1 to Lv3) |
| 35 | 12 (50) | 137,900 | 39 / 38 | Lv4: a stat +10, HP 240, SP 240, MP 280, attack or spread +16, defence +16, magic defence +50; **the 24 "Dual" lapis** (two stats +9/+8, or a stat and HP/SP/MP 175 to 195 or defence 12, magic defence 36) |
| 36 | 4 (18) | 365,000 | 34 / 28 | Lv5: a stat +20, HP 360, attack +24, magic defence 75; **Absorption Lapis Lv5 (50, dangerous)**; **the Lv7 debuff lapis 164 to 176** |
| 37 | 1 (4) | 480,000 | 40 / 40 | Lv6: a stat +30, HP 500, MP 600, attack +32, defence +32, magic defence 100; **Absorption Lapis Lv6 (75, dangerous)**; the 12 jewel lapis (two stats +7/+6, dangerous); **the Lv6 debuff lapis 151 to 163** |
| 38, 39, 40 | 1 | 627,000 to 1,040,000 | 47 / 0 | Max lapis (a stat +45), the state lapis 110 to 135 |
| 98, 99 | 1 | 0 | 8 / 0 | the state lapis 136 to 139 (90 %, 100 %) |

- **The 90 % / 100 % HP lapis (136 Dying, 138 Death) and every state lapis 110 to 135 have no origin**: their drop grades are 38 to 40,
  98, 99, which no monster entry, box or merchant gives (verified query over `defs.mob_items` and `world.item_create`). Part 1 section
  4.5's concern holds only for operator-made items.
- **The debuff lapis that are in play** (weapon only): Lv6 (151 to 163, grade 37) stun, stop, blind, silence, slow, fear, curse, poison,
  disease, confusion, jinx, sleep at **7 or 8 %** a landed hit for 3 to 60 s; **Lv7 (164 to 176, grade 36) the same at 10 to 13 % and
  longer, yet a grade lower, so 4 % to link against 1 %**. They drop from monsters 1259 (level 63, map 58; 1 in 13 for Lv7, 1 in 5 for
  Lv6) and 1252 (level 60, map 57).
- **Absorption**: Lv5 (50) always drops from boss 2414 (level 62, map 64); Lv6 (75) twice a kill from 2472 (level 64, the summoned
  final-boss form, map 67). Both dangerous. Five pieces with Lv6 would take **375 off every blow**, which against PvP physical damage
  (attack minus defence, part 1 section 4.5) is a large share. At 1 % to link, a dangerous lapis destroys the piece 99 times in 100
  without a Lucky Charm (mall only).
- **What the strongest obtainable stat lapis give**: a grade-37 Craft Lapis (STR +30) on a weapon is 39 melee attack, as much as a weapon at +4.8
  (+4 = 31, +5 = 41); at 1 % a try, about 100 tries and 48 million a success (modelled arithmetic).
- **Origins**: grades 31 to 34 drop everywhere (the only merchants with lapis are the guild-house general goods, section 2.4); grade 35 drops at levels 50 to 55
  (monster 738, 1 in 7; Dual lapis from 1500, 1501, 1600 at 1 in 120 to 240); grade 36 from 1188 (level 58, 1 in 28); grade 37 from
  2412 (level 62, map 64, 1 in 15 to 17) and the jewel lapis from 1540 and 1546 (levels 56, 58, 1 in 120).

### 4.2 Lapisia, hammers, charms, runes (verified data, `tools.csv`)

| item | what | origin |
|---|---|---|
| 95/1 Weapon Lapisia (grade 96), 95/6 Armor Lapisia (grade 40) | enchant, **dangerous** (`reqvg` 1) | drop: Cailum Sacra floor bosses 2464 to 2467 (level 63, summoned, map 65) **1 in 10**; instance monsters 2462, 2463 (level 61) 1 in 40; 2460, 2461 (level 59) 1 in 200 (maps 65, 66); sequence: 1 in 32,002 monsters made |
| 95/2 Power Lapisia of Syeeric, 95/7 Defense Lapisia of Syeeric | enchant, **safe** | **none** |
| 95/3, 95/8 (GM, `???`) | enchant | none |
| 100/49 Linking Hammer (200 %) | link rate × 2 | mall |
| 100/193 to 100/199 Linking Hammer Lv1 to Lv7 (110 to 450 %) | link rate × 1.1 to 4.5 | mall boxes (Lv2, Lv3 also an in-play box) |
| 44/193 to 44/199 (the same hammers as type 44) | | none |
| 100/139 Lucky Charm | keeps the item on a failed dangerous link | mall |
| 100/140 Small, 100/141 Large Extraction Hammer (30 %, 90 %) | lapis extraction | mall |
| 100/171 Recreation Rune | rerolls crafted stats (two draws) | **none** |
| 100/82 Operator's Exclusive (2000 %) | link at 100 % | none |

**Enchant materials** (the `reqluc` item each enchant try uses): 59 items, all obtainable, 6 sold (among them 25/123 Magic Crystal,
25/127 Spirit Essence, 25/128 Eternal Essence at 100,000); the commonest: 25/150 Spirit Lost (758 items), 25/142 Nightmare Core
(735), 25/200 Magic Alchemy Powder (721), 25/157 Ghost Crystal (612), 25/137 Spirit Core (611), 25/156 Soul Crystal (601), all drops
at 1 in 6 to 20.

### 4.3 Enchant cost (verified rule `Enchant::succeeds`; modelled arithmetic, `enchant.csv`)

**Revised 2026-11-06.** The first version of this section used the server's built-in tables (weapon 70 % at +1 falling to 1 in a million
from +12, +7 a step; armour 99 % to 1 in a million from +15). The server now reads the rates and the attack from `Data/ItemEnchant.ini`
as the original does (`docs/plans/enchant-table.md`), and `content.py --only enchant` reads the same `world.item_enchant` table, so the
table below changed in every row: **the rates are higher at every step** (weapon +1 is 90 %, +3 70 %, +5 40 %; armour +4 80 %, +7 20 %),
**the weapon's attack grows faster after +3** (+4 = 31, +7 = 64, +10 = 106) and **no step is "never"** to +20 (the last rates are 0.02 %
from +14). The steps to +3 weapon and +4 armour, which the builds use, give the same 21 attack and 20 absorption as before; only what
they cost fell (about 20 to 5 dangerous Lapisia for a weapon +3, 14 to 5 for armour +4).

Success when the material count (1) + a draw below 1,000,000 is below the step's rate. With the **dangerous** Lapisia players can get, a
failure destroys the item, so a step is reached only by succeeding at every step from 0 with one item: tries `T(n) = (T(n-1) + 1) /
p(n-1)`, items used `1 / (p(0) x ... x p(n-1))`. A weapon try costs 2,500,000 (grade 96), an armour try 1,040,000 (grade 40), the
material extra.

| to step | weapon: attack, chance | safe tries from 0 | **dangerous Lapisia / weapons used** | money (dangerous) | armour: absorption, chance | safe tries | **dangerous Lapisia / pieces used** |
|---|---|---|---|---|---|---|---|
| 1 | +7, 90 % | 1 | 1 / 1 | 2.8 million | +5, 99 % | 1 | 1 / 1 |
| 2 | +14, 80 % | 2 | 3 / 1 | 6.6 million | +10, 98 % | 2 | 2 / 1 |
| 3 | +21, 70 % | 4 | 5 / 2 | 13 million | +15, 97 % | 3 | 3 / 1 |
| 4 | +31, 60 % | 5 | 10 / 3 | 26 million | +20, 80 % | 4 | 5 / 1 |
| 5 | +41, 40 % | 8 | 28 / 8 | 71 million | +25, 60 % | 6 | **10 / 2** |
| 6 | +51, 20 % | 13 | **147 / 41** | 370 million | +30, 40 % | 8 | **28 / 6** |
| 7 | +64, 10 % | 23 | **1,476 / 413** | 3.7 billion | +35, 20 % | 13 | **146 / 28** |
| 8 | +77, 8 % | 35 | 18,467 / 5,167 | 46 billion | +40, 10 % | 23 | 1,473 / 277 |
| 10 | +106, 1 % | 152 | 3.1 x 10^7 | | +50, 6 % | 53 | 307,092 / 57,652 |
| 12 | +138, 0.2 % | 819 | 2.6 x 10^12 | | +60, 0.6 % | 319 | 5.1 x 10^9 |
| 14 | +176, 0.06 % | 3,490 | 4.3 x 10^18 | | +70, 0.1 % | 1,821 | 2.6 x 10^15 |

(The chances are 1 in a million lower than printed because of the material.) Steps 15 to 20 stay at 0.02 % (weapon +15 is 195 attack,
+20 311; armour +20 is 100).

- **In play, a weapon tops out at +5 or +6 and armour at +6 or +7**: +5 on a weapon takes 28 Lapisia (that drop at 1 in 10 from four
  floor bosses of one instance) and 8 copies of the weapon, +6 takes 147 and 41, +7 takes 1,476 and 413 and 3.7 billion in Lapisia
  alone, which is the edge of what a character can hold (4.29 billion). The first version said +4 was the edge and +5 out of reach
  (228 Lapisia at +4); with the original's file the edge moved up about two steps. The builds in part 3 keep weapon +3 and armour +4,
  which now cost little (5 Lapisia each); whether players would go further was not modelled.
- **The safe Lapisia would make +7 a matter of 23 tries** (57 million) and +10 of 152 (380 million) (part 1 section 4.9). They are
  not in play.
- The material counts as a hindrance, not a help (more material lowers the chance: part 1 section 4.9); with one material per try the
  numbers above are the best case.

## 5. Skills

### 5.1 By class (verified data, `skills.csv`; rows of levels 1 to 3 the class may learn)

| class (Light = Fury) | attack | debuff | buff | heal | passive | field | one-off | rows |
|---|---|---|---|---|---|---|---|---|
| fighter / warrior | 37 | 20 | 24 | 0 | 41 | 0 | 3 | 125 |
| defender / guardian | 9 | 42 | 37 | 9 | 33 | 0 | 3 | 133 |
| ranger / assassin | 22 | 31 | 34 | 0 | 26 | 3 | 4 | 120 |
| archer / hunter | 33 | 31 | 29 | 0 | 26 | 3 | 3 | 125 |
| mage / pagan | 40 | 42 | 15 | 3 | 19 | 3 | 3 | 125 |
| priest / oracle | 21 | 23 | 35 | 27 | 19 | 0 | 6 | 131 |

**Mirrored classes share every skill row except the race skills** (part 1 section 6.2: Berserker, Evasion, Battle Cry, Bloody Karma
and the hard-mode race skills 101, 110, 128, 129).

### 5.2 Skills with no use (verified, `skills.csv`; `SkillDef::not_built`)

| skill | why |
|---|---|
| 3 Interpretation (levels 1 to 3) | passive kind 5, language: nothing in the server reads it |
| 125 Soul Burn | no class may learn it, and refused as not built |
| 140 Expanding, 169 Dragnet, 179 Ghost Panic | no class may learn them (no job flag) |
| level 3 of 15, 17, 19, 20, 21, 22 (the weapon masteries), 175 to 185 (the level-60 skills) | no job flag on the third level |
| levels 2 and 3 of 100 Scouting, 173 Spy Bug | no job flag |

The level-60 skills 180 to 185 thus have two levels in play: level 1 from hard, level 2 ultimate only (part 1 section 5.1).

### 5.3 The attack skills per class at 15, 30 and 60 (modelled; `class_skills_15/30/60.csv`)

Each class's default build of part 1 (ultimate, the picker's gear, a weapon the skill allows), every attack skill it may learn at its
highest level. **"A use" counts a skill's whole damage over time** (all its ticks), so "per second" (a use's damage over its cast and
interval) overstates damage-over-time skills against the rotation of part 1 (the mage's Flame Shock: 2,339 here, 1,144 in the
rotation). On the player: the other faction's mirrored class, default build.

| class | 15: normal / best on monster (/s) | 15: best on player (share of HP a use) | 30: normal / best on monster | 30: best on player | 60: normal / best on monster | 60: best on player |
|---|---|---|---|---|---|---|
| fighter | 67 / Double Damage Lv2 118 | Double Damage Lv2 (23 %) | 191 / Heavy Swat Lv3 397 | Heavy Swat Lv3 (23 %) | 163 / Enhance Wind Spin Lv3 624 | **Regina Power Lv2 (61 %)** |
| defender | 59 / Heavy Swat Lv1 97 | Heavy Swat Lv1 (14 %) | 168 / Heavy Swat Lv3 392 | Heavy Swat Lv3 (21 %) | 129 / Behimoth Range Lv3 531 | Behimoth Range Lv3 (18 %) |
| ranger | 118 / Aggravation Lv1 146 | Aggravation Lv1 (19 %) | 267 / Eviscerate Lv1 534 | Eviscerate Lv1 (31 %) | 265 / Slasher Lv3 920 | **Syeeric Touch Lv2 (64 %)** |
| archer | 54 / Nailed Shot Lv1 125 | Nailed Shot Lv1 (27 %) | 128 / Nailed Shot Lv2 202 | Bull's Eye Lv2 (22 %) | 106 / Viper Sting Shot Lv3 1,086 | Viper Sting Shot Lv3 (42 %) |
| mage | 6 / Sonic Bolt Lv2 146 | Sonic Bolt Lv2 (41 %) | 22 / Sustain Doom Lv1 806 | **Sustain Doom Lv1 (86 %)** | 1 / Flame Shock Lv2 2,339 | **Flame Shock Lv2 (114 %)** |
| priest | 5 / Hailstorm Lv1 141 | Magic Spot Lv1 (30 %) | 21 / Hailstorm Lv2 383 | Hailstorm Lv2 (32 %) | 1 / Surgy Shock Lv2 774 | Surgy Shock Lv2 (34 %) |

The Fury classes are within a few per cent (different picked items and races; the pagan's Flame Shock Lv2 takes 117 % of the mage's
HP). How many attack skills each class has at 15 / 30 / 60: fighter 3 / 5 / 15, defender 2 / 4 / 6, ranger 4 / 6 / 15, archer 4 / 5 /
13, mage 9 / 15 / 25, priest 6 / 9 / 11.

- **The defender and the priest have the fewest and weakest**: 6 and 11 attack skills at 60, the best taking 18 % and 34 % of a mirror
  a use. Their roles are support (part 3 counts the buffs and heals).
- **The mage and pagan one-shot a mirror at 60 and nearly at 30** with damage-over-time skills (modelled; the model does not count
  dispels, potions or healing during the ticks).
- **The level-60 percentage skills decide PvP for four classes** (61 % to 64 % a use at 70 % success; section 5.5).

### 5.4 Attack skills at 60 that are never worth it (modelled; `class_skills_60.csv` `below_normal_*`, `dominated_by`)

"Matched in every way": another of the class's skills on the same weapon deals at least as much on the monster and on the player, with
no longer cast and interval, cooldown, SP or MP. Area, debuff and lasting effects are **not** counted, so this is a list to look at,
not of useless skills.

| class | below the normal blow on both | matched in every way (examples) |
|---|---|---|
| fighter | 1 (56 Stinger Lv3) | 6: 118 Dragon Slayer Lv3, 158 Pollution Zone Lv3, 174 Hell Lord Lv2, 301 Bastard Swing Lv3, 302 Deadly Shock Lv2 (by Deadly Strike, Double Damage, Heavy Swat, Enhance Wind Spin) |
| defender | 0 | 0 |
| ranger | 4 (61 Stun Crash, 79 Venom Crash, 104 Halt Kick, 170 Mana Decay) | 9: 94 Gruesome Attack, 161 Fantom Assault, 176 Terrible Attack, 309 Shadow Attack, 311 Eviscerate, 312 Dark Panic, 313 Gruesome Zone (by Slasher) |
| archer | 3 (45 Piercing Shot, 122 Sustain Shot, 170 Mana Decay) | 5: 92 Absolute Attack, 163 Bloody Shower, 177 Nuke Shot (by Rapid Shot, Viper Sting Shot) |
| mage | 2 (60 Mana Slug, 155 Stamina Slug) | 11: 69 Earthquake, 83 Earth Spikes, 96 Confusion, 164 to 166 the Choirs, 184 Kamain Breath, 322 Storm Shock, 323 Earth Shock (by Flame Shock, Meteor, Hellfire) |
| priest | 2 (60 Mana Slug, 155 Stamina Slug) | 2 |

(The Fury classes the same. Skills at 0 on the monster do not hurt monster 1547: resisted (section 5.5) or MP and SP damage (Mana
Decay, the Slugs); inferred from the data, not traced per skill.)

### 5.5 Percentage skills against monster resistances (verified data: `defs.mobs` resist states and skills of the 1,436 placed monsters)

| skill (levels) | classes | damage | success | cooldown | resisted by (of 1,436 placed) | of the 472 of level 51+ |
|---|---|---|---|---|---|---|
| 41 Protection Charge (1-3) | defender | 5-10 % of current | roll | 30 s | 401 | 235 |
| 87 Eraser (1-3) | fighter | 1-2 × own current HP | 65 % | 600 s | 474 | 253 |
| 92 Absolute Attack (1-3) | archer | 10-20 % of max | roll | 300 s | 401 | 235 |
| 94 Gruesome Attack, 313 Gruesome Zone | ranger | 20-30 % | roll | 90 s | 401 | 235 |
| 113 Trap (1-3) | ranger, archer | 15-35 % of current | 100 % | 180 s | 401 | 235 |
| 118 Dragon Slayer (1-3) | fighter | 3-7 % of current | roll | 600 s | 32 | 32 |
| 170 Mana Decay (1-3) | ranger, archer | 50-90 % of max **MP** | roll | 600 s | 0 | 0 |
| 176 Terrible Attack (1-2) | ranger | 35-48 % of current | roll | 900 s | 401 | 235 |
| **180 Regina Power** Lv2 | fighter | 87 % of max | 70 % | 1,200 s | 491 | 270 |
| **182 Syeeric Touch** Lv1 / Lv2 | ranger | 46 % current / 92 % max | 70 % | 720 / 480 s | **1,424** (state 8) | **472** |
| **183 Pantera Vision** Lv1 / Lv2 | archer | 44 % current / 88 % max | 70 % | 900 / 600 s | **1,424** (state 8) | **472** |
| **184 Kamain Breath** Lv1 / Lv2 | mage | 43 % current / 87 % max | 70 % | 1,500 / 1,200 s | 491 | 270 |

(Each line holds for the Fury mirror.) **State 7** (resisted by 401) and **state 8** (by 1,424) are the near-death and death states of
the lapis 136 to 139 too. So 182 and 183 work only on players and on the 12 placed monsters without state 8; 180 and 184 on about 200
of the 472 cap-level monsters.

## 6. Monsters as the yardstick (verified data, `mobs.csv`; modelled fights)

Placed monsters with experience, by level band (kinds, not spawn counts); medians over the kinds that are not bosses. "Party": the
default fighter and mage of the level (part 1's builds) die before killing it even with potions (modelled).

| levels | kinds | of them party | boss | HP | defence | magic defence | DEX | attack/s | exp per HP | fighter kills / dies (s) | mage kills / dies (s) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1-5 | 24 | 0 | 0 | 186 | 12 | 13 | 12 | 12 | 0.23 | 5 / 111 | 4 / 49 |
| 11-15 | 75 | 0 | 0 | 647 | 18 | 18 | 25 | 27 | 0.22 | 10 / 82 | 5 / 36 |
| 26-30 | 97 | 7 | 0 | 1,816 | 31 | 27 | 46 | 62 | 0.24 | 4 / 96 | 6 / 31 |
| 41-45 | 155 | 10 | 0 | 3,422 | 44 | 35 | 68 | 95 | 0.29 | 8 / 100 | 5 / 33 |
| 46-50 | 161 | 36 | 0 | 3,773 | 47 | 44 | 72 | 101 | 0.29 | 10 / 103 | 8 / 34 |
| **51-55** | 185 | **133** | 0 | **8,522** | **422** | **253** | 156 | **361** | **0.82** | 28 / **21** | 14 / **9** |
| 56-58 | 134 | 120 | 0 | 9,520 | 456 | 281 | 167 | 390 | 0.82 | 28 / 18 | 15 / 8 |
| 59-60 | 40 | 35 | 2 | 274,363 | 250 | 202 | 262 | 526 | 0.07 | never / 12 | 329 / 6 |
| 61-64 | 66 | 37 | 26 | 301,624 | 210 | 201 | 285 | 501 | 0.03 | never / 13 | 594 / 6 |

- **The step at 51** is the whole of the endgame's difficulty: up to 50, monsters grow smoothly and a default build kills them in 4 to
  10 s while lasting 30 to 130 s; from 51, defence jumps ninefold, attack fourfold, experience per HP threefold, and the default builds
  die first. The monsters of 51 to 58 are on maps 43 to 47 (the level-50+ fields) and map 0.
- **59 to 64** are party and boss content (maps 64, 48, 49, 58, 65, 67; the Cailum Sacra instance; map 50, the ranking battle) with
  experience per HP 0.03 to 0.07. The level-59 grind of part 1 (15,700 kills) is against these.
- **The yardstick monsters of this document** are part 1's: 2125 (level 15), 2183 (30), 1547 (58, the most placed of maps 45 to 47).

## 7. What could be reworked (options only; no decision)

Grouped by what they touch; each names where the change would live (part 1 section 2).

**Supply (data: origins, drops, shops, the mall)**

- Give the safe Lapisia, the Recreation Rune and the Lucky Charm in-play origins (drops, quests, a currency), or keep them out and lower
  the enchant rates' cliff instead.
- Decide what the Item Pockets are for: they hold most items in the game; on an alpha with no mall points they are dead weight, or the
  pool for a new in-play box.
- Place the event NPCs of the 136 closed quests (or drop their rewards); decide whether gatekeepers 44 and 45 (the halls) should be
  placed, which would open the ranking battle and the halls' merchants to everyone.
- Remove or keep the 4,706 placeholder rows and the 121 duplicate groups (a data cleanup; the client patch would follow).

**Items (data `defs.items` and a client data patch)**

- Bring the top of each slot down from hard-mode quests to every mode, or keep mode-gated gear as the reason for ultimate.
- Rebalance armour between defence and HP (defence is a flat subtraction against monster attacks of 1,000 at 60), and the crafted HP,
  MP, SP factor of 100 (a single number can outweigh the item).
- Mirror the factions' items exactly (attack and spread, shield magic defence, job flags, the Fury quest bracelets), or keep and
  document the differences as faction flavour.
- Fill the accessory slots: most rings, amulets and bracelets are worth 3 to 18; a few quest rows are worth ten times that.

**Lapis and enchanting (data and server constants `lapis.rs`)**

- The enchant rates by step (`ENCHANT_RATES`) and the material rule (more material lowers the chance), the dangerous flag of the
  in-play Lapisia, where Lapisia drop.
- The link rate formula (grade 37 at 1 %), the Lv6 / Lv7 debuff lapis grades (the stronger one links better), the absorption lapis'
  size (75 a piece) and their danger.
- Whether the state lapis (110 to 139) stay out of play; if any come in, the 90 % / 100 % ones are the question.

**Skills (data `defs.skills` and a client data patch)**

- The level-60 percentage skills: their size (87 % to 92 % of max HP), success (70 %), cooldowns, and whether state 8 should be resisted
  by nearly every monster.
- The skills never worth casting (section 5.4): raise them, merge them, or leave them as flavour.
- The mage's damage-over-time skills against players (more than a mirror's HP a use).
- The defender's and priest's attack skills, if they should solo (part 3).
- The unlearnable level-3 rows (masteries, 175 to 185) and the dead skills (Interpretation, Soul Burn, 140, 169, 179).

**Monsters (data `defs.mobs`, the data editor)**

- The step at 51 (defence, attack, HP), and whether the cap monsters are meant for parties only; the experience per HP from 59 on.
- Monster resistances to the percentage and death states.

**Modes and factions (server code and data)**

- Why keep easy, normal and hard: they gate items (896 rows ultimate only; the top weapons hard), skills (62 rows ultimate only) and
  the experience curve (part 1 section 5); if everyone plays ultimate, those gates are what the other modes would lose or keep.
- The Fury kill-count grades and quest bracelets against Light's (part 1 section 6.3, section 3.8 here).

## 8. What cannot be settled without play

- **What players really get**: drop rates are known, but how often a party clears Cailum Sacra (the only Lapisia), which monsters
  people farm at 51 to 58, and how many 1-in-120 Dual lapis a week brings.
- **Crafted stats in practice**: how good the rolls on worn items are after a few weeks (the smallest of three draws).
- **Whether players accept enchant +5 / +6 (weapon) and +6 / +7 (armour) as the ceiling** (with the original's rates), or quit for it.
- **The percentage skills in PvP**: how often 70 % lands decide a fight, against healers, potions and reflection.
- **The guild houses and the ranking battle**: whether a guild can ever start the circle of section 2.4 without operator help.
- **Whether the client's tooltips mislead** players in the 132 items and 30 skills that differ.
