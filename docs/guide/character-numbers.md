# Your character's numbers

How the original game (the EP4.5 server and the US client) turns a character's class, race, level, mode, stat points and equipment
into the numbers that fight: maximum HP, SP and MP, hit, evasion, attack, defence, critical, speeds and recovery. The next pages use
these numbers: [Hitting and being hit](hitting.md) and [Elements, resistances and defences](elements-defences.md).

## In short

- **Six stats.** STR is melee and ranged attack. DEX is hit, evasion, a little attack and SP. REC is HP and defence. INT is magic
  attack. WIS is magic defence, MP and heals. LUC is critical hits and a little evasion.
- **Points.** Each level gives 1 point to your class's own stat and 5, 7 or 9 free points (easy and normal, hard, ultimate). You can spend
  them on any stat at any time; nothing in the game needs a stat to equip.
- **HP, SP and MP** come from a table by class and level, raised by 10 % in hard mode and 50 % in ultimate, plus 5 for every point of REC
  (HP), DEX (SP) and WIS (MP), plus what your equipment adds.
- **Attack is mostly your weapon and STR (or INT).** A level adds 1 hit and 1 evasion and the table's HP, SP and MP, nothing else.
- **Your equipment counts fully** until it wears down: below half its quality a weapon or armour piece gives half, below a tenth nothing.
  Accessories and lapis never wear.
- **REC does not make you recover faster.** Recovery is the same for everyone: a little every 5 seconds standing, much more sitting,
  nothing while you fight.
- **The character window works its numbers out itself** and leaves some things out (your faction's kill-count bonus, wear, mastery
  skills, armour enchants), so what it shows is not always what the server uses.

## 1. Words this guide uses

| word | meaning |
|---|---|
| **stats** | STR, DEX, REC, INT, WIS, LUC. **Base stats** are the character's own (start, levels, points spent); **totals** add equipment, lapis, buffs and the faction bonus. The formulas use the totals |
| **attack kind** | every blow is **melee**, **ranged** or **magic**. A normal blow is ranged with a javelin, bow or crossbow (item types 11, 13 and 14) and melee with any other weapon; normal blows are never magic. A skill names its own kind. Each kind has its own hit, evasion, attack and defence |
| **attack** and **damage** | *attack* is a number on your character sheet; *damage* is what a blow takes off. Damage is attack after the target's defence, the level gap, the element and the dice |
| **defence** | in this guide "defence" alone means the melee and ranged defence; **magic defence** is the magic kind's |
| **percentage point** | critical, hit and resist chances are whole percentages; "+5 critical" means 5 percentage points, not 5 % of the chance |
| **level gap** | the target's level minus yours (positive when the target is higher) |
| **trunc** | cut the fraction off (6019.5 becomes 6019); the game rounds this way almost everywhere |
| **a roll** | a random whole number: "a roll of 1 to LUC" is any whole number from 1 to the LUC total, each equally likely |

Where a rule comes from: every formula here is the original server's (`ps_game.exe`), or the client's for the character window, and
the reference note linked under each section gives the address it was read at. The tables of numbers (status rows, items, monsters,
skills, the kill-count grades) are the original's data.

## 2. The six stats

What one point does, everywhere it counts (base and item points alike):

| stat | each point adds | it does **not** |
|---|---|---|
| **STR** | melee attack **+1.3**, ranged attack **+1** | anything for magic; it is not needed to equip anything |
| **DEX** | melee and ranged **hit +0.5** and **evasion +0.5**, melee and ranged attack **+0.2**, **max SP +5** | add magic hit or evasion |
| **REC** | **max HP +5**, melee and ranged **defence +1**; monsters' grudge (aggro, below) | speed up recovery |
| **INT** | magic attack **+1.3**; against players it also shrinks the cut their magic defence makes ([Hitting and being hit](hitting.md#7-players-against-players), section 7) | anything for melee or ranged |
| **WIS** | magic attack **+0.2**, **magic defence +1**, **max MP +5**, every heal you cast **+2 HP**; the magic resist rolls compare both sides' WIS | much for damage (0.2 a point) |
| **LUC** | **critical +0.19**, melee and ranged **evasion +0.125**, ranged attack **+0.3**; every landed blow adds a small luck bonus (a roll of 1 to LUC, × 0.1); each point lowers the critical chance of anyone hitting you by 0.034 | change what monsters drop |

- **Aggro**: every blow you land adds your REC × 10 to that monster's grudge list, and a monster may switch to whoever has the most every
  5 seconds; a taunt adds its own amount plus REC × 10 ([Monsters fighting back](../original/mob-fight.md), section 2).
- **Drops**: the monster's drop chances read nothing of the killer but who it is: no stat, level, buff or faction bonus changes them
  ([Fighting monsters](../original/combat.md), section 7).
- **Magic hit and magic evasion have no stat behind them at all**: only buffs, passive skills and the faction bonus add to them.

Source: `CUser::SetAttack` and `CUser::ItemEquipmentAdd` in [Fighting monsters](../original/combat.md) section 4 and
[Character numbers](../original/stats.md) sections 1 and 2; heals in [Skills 2](../original/skills-use.md) section 10.

## 3. Starting stats, levels and points

Each class starts with its own stats; the race adds a little. In each faction every class belongs to one race (the Light side is human
or elf, the Fury side deatheater or vail), so a class and its race always go together.

| class (Light / Fury) | STR | DEX | REC | INT | WIS | LUC | **+1 a level to** |
|---|---|---|---|---|---|---|---|
| fighter / warrior | 14 | 12 | 9 | 8 | 7 | 11 | STR |
| defender / guardian | 10 | 9 | 12 | 10 | 10 | 10 | REC |
| ranger / assassin | 10 | 15 | 9 | 7 | 8 | 12 | DEX |
| archer / hunter | 11 | 10 | 10 | 7 | 10 | 13 | LUC |
| mage / pagan | 7 | 9 | 9 | 15 | 12 | 9 | INT |
| priest / oracle | 8 | 9 | 10 | 12 | 14 | 8 | WIS |

| race | its classes | stats | maxima |
|---|---|---|---|
| human | fighter, defender, priest | LUC +4 | SP +10 |
| elf | ranger, archer, mage | DEX +4 | MP +10 |
| deatheater | warrior, guardian, hunter | STR +2, REC +2 | HP +10 |
| vail | assassin, pagan, oracle | INT +2, WIS +2 | SP +5, MP +5 |

**Every level** adds 1 to the class's stat (the last column: 59 by level 60), fills HP, SP and MP, and gives free points:

| | easy | normal | hard | ultimate |
|---|---|---|---|---|
| free stat points a level | 5 | 5 | 7 | 9 |
| skill points a level | 3 | 3 | 4 | 5 |
| stat points by level 60 | (easy stops at 30: 145) | 295 | 413 | 531 |
| skill points by level 60 | (at 30: 92) | 182 | 241 | 300 |

A new character starts with 5 skill points, so the skill totals are the level's share plus 5.

The class stat is not always the stat its damage uses: the archer's goes to LUC (ranged attack uses STR fully and LUC only × 0.3), the
priest's to WIS, the defender's to REC.

**Spending points** works at any time in the world (fighting, trading, transformed), in any mix. It raises the stat and the matching
maximum at once but **not** your current HP, SP or MP. A request for more points than you have changes nothing.

**A Stat Reset Stone** gives back every point: base stats to the start, plus the class stat's levels, plus the race, and 5, 7 or 9 points a
level; a Skill Reset Stone removes every skill and gives 3, 4 or 5 points a level plus 5.

Source: starting stats and races in [The game server](../original/ps_game.md) section 7.2 (character creation); levels in
[Fighting monsters](../original/combat.md) section 5 (`CUser::LevelUpStatus`); spending in [Character numbers](../original/stats.md)
section 5; resets in [Using items](../original/item-use.md) section 8.

## 4. HP, SP and MP

```
maximum = trunc(status row × mode factor) + race bonus + 5 × the stat (REC for HP, DEX for SP, WIS for MP)
          + the items' HP/SP/MP + 100 × crafted HP/SP/MP + lapis + buffs and passives + faction bonus
mode factor: easy and normal 1, hard 1.1, ultimate 1.5
```

The status row is the class's at your level (normal mode; Light and Fury share the rows):

| class | HP at 1 / 15 / 30 / 45 / 60 | SP at 1 / 15 / 30 / 45 / 60 | MP at 1 / 15 / 30 / 45 / 60 |
|---|---|---|---|
| fighter, warrior | 205 / 699 / 1,511 / 2,616 / 4,013 | 125 / 252 / 411 / 596 / 804 | 50 / 73 / 121 / 190 / 283 |
| defender, guardian | 250 / 783 / 1,659 / 2,850 / 4,356 | 65 / 107 / 162 / 222 / 297 | 65 / 106 / 184 / 298 / 447 |
| ranger, assassin | 170 / 602 / 1,313 / 2,282 / 3,508 | 150 / 363 / 632 / 941 / 1,290 | 60 / 83 / 133 / 205 / 302 |
| archer, hunter | 175 / 574 / 1,229 / 2,119 / 3,244 | 135 / 312 / 534 / 789 / 1,078 | 70 / 148 / 294 / 504 / 778 |
| mage, pagan | 150 / 393 / 795 / 1,344 / 2,039 | 60 / 97 / 142 / 197 / 257 | 170 / 471 / 1,018 / 1,798 / 2,804 |
| priest, oracle | 160 / 443 / 910 / 1,546 / 2,350 | 70 / 112 / 168 / 230 / 305 | 150 / 414 / 889 / 1,567 / 2,445 |

The whole table, every level, is in the original server's data (`defs.status_defs` once imported); the client has no copy and shows
the maxima the server sends.

- **Ultimate's 1.5 applies only to the table's part**, not to the 5 a point or to items: a level-60 ultimate fighter's table HP is
  trunc(4,013 × 1.5) = 6,019, against 4,013 in normal mode.
- **5 a point** counts for base points and item points alike: an item with REC +2 is worth 10 HP more than its listed HP.
- A **crafted** HP, SP or MP number (the hidden bonuses of craftable items) is worth 100 a point.

Source: `CUser::InitEquipment` and `CUser::ItemEquipmentAdd` in [Character numbers](../original/stats.md) sections 1 and 2.

## 5. The numbers that fight

For each attack kind the character has a hit, an evasion, an attack and a defence, and one critical for all three. "Add" is what buffs,
passive skills and the faction bonus give that kind; "item attack" is the weapon's attack plus everything else on you that adds attack
(lapis, the enchant, accessories, weapon mastery); L is your level.

| kind | hit | evasion | attack | defence |
|---|---|---|---|---|
| melee | add + L + DEX × 0.5 | add + L + DEX × 0.5 + LUC × 0.125 | STR × 1.3 + DEX × 0.2 + item attack + add | REC + item defence + add |
| ranged | as melee | as melee | STR + DEX × 0.2 + LUC × 0.3 + item attack + add | as melee |
| magic | add | add | INT × 1.3 + WIS × 0.2 + item attack + add | WIS + item magic defence + add |

```
critical = LUC × 0.19 + add                       (each line is cut to a whole number at the end)
```

- **The item attack is shared by all three kinds**: a staff's attack counts for your spells *and* your melee number, a bow's for its
  melee number too.
- **Each blow also rolls the weapon's spread**: a roll of 0 to (spread − 1) is added to the attack (the weapon's second number, and
  lapis that add to it).
- Monsters have the same four numbers, from their own data: melee hit DEX × 0.5 + level, melee evasion DEX × 0.5 + level / 2 +
  LUC × 0.125 (ranged evasion with the whole level), magic attack WIS + level, critical LUC × 0.19; their magic hit and evasion are 0
  unless a skill changes them.

Source: `CUser::SetAttack` and `CMob::SetAttack`, [Fighting monsters](../original/combat.md) section 4.

## 6. What equipment adds

| slot | adds |
|---|---|
| **weapon** | its stats and HP/SP/MP, attack and spread, range, attack speed; the weapon-mastery passives add attack for their weapon types, the weapon-speed passives attack speed |
| **armour** (helmet, upper armour, lower armour, gloves, boots) | its stats and HP/SP/MP, defence and magic defence; the **upper armour's element** is your defending element ([Elements](elements-defences.md#2-elements)) |
| **shield** | as armour; your **shield-mastery** passives add their defence for anything in this slot |
| **cloak** | its stats and HP/SP/MP, defence and magic defence; its lapis sockets hold its badge, so lapis add nothing there |
| **accessories** (amulet, two rings, two bracelets) | stats, HP/SP/MP, range, attack and move speed, and all four of attack, spread, defence and magic defence, **always at full value** |
| **lapis** (any slot but the cloak) | their stats, HP/SP/MP, attack, spread, defence, magic defence, range, speeds, **absorption** (a flat cut of every blow, [Absorption](elements-defences.md#6-absorption)), elements, and outside the weapon a block of one state ([Resisting debuffs](elements-defences.md#5-resisting-debuffs-and-states)) |

- **A shield has no block value.** It is a defence item; blocking comes only from skills ([Blocks](elements-defences.md#7-blocks-reflection-and-giving-up-sp-or-mp)).
- **Wear**: a weapon's attack and spread and an armour piece's defence and magic defence count fully while the item's quality is above
  half its full quality, half below that, and **nothing below a tenth**. Stats and HP/SP/MP do not wear. A critical blow you land wears
  your weapon; blows you take wear your armour ([Wear and repair](../original/repair.md)).
- **Crafted stats**: craftable items carry up to nine hidden numbers in their craft name: STR, DEX, REC, INT, WIS, LUC as points (with
  their 5 HP, SP or MP a point) and HP, MP, SP at **100 a point** ([New items and crafting](../original/item-origins.md)).

### Enchanting

An enchant step (+1 to +20, with a Lapisia and the item's material at a blacksmith) adds **attack to a weapon** and **absorption to an
armour piece**, nothing else. How much, and how likely each step is, comes from a data file the original server reads when it starts
(`Data/ItemEnchant.ini`). With the file of the original server patch:

| step | +1 | +2 | +3 | +4 | +5 | +6 | +7 | +8 | +9 | +10 | +11 | +12 | +13 | +14 | +15 | +16 | +17 | +18 | +19 | +20 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| weapon attack | 7 | 14 | 21 | 31 | 41 | 51 | 64 | 77 | 90 | 106 | 122 | 138 | 157 | 176 | 195 | 217 | 239 | 261 | 286 | 311 |
| armour absorption | 5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 | 60 | 65 | 70 | 75 | 80 | 85 | 90 | 95 | 100 |
| weapon: chance to reach it | 90 % | 80 % | 70 % | 60 % | 40 % | 20 % | 10 % | 8 % | 6 % | 1 % | 0.6 % | 0.2 % | 0.1 % | 0.06 % | 0.02 % | 0.02 % | 0.02 % | 0.02 % | 0.02 % | 0.02 % |
| armour: chance to reach it | 99 % | 98 % | 97 % | 80 % | 60 % | 40 % | 20 % | 10 % | 8 % | 6 % | 1 % | 0.6 % | 0.2 % | 0.1 % | 0.06 % | 0.02 % | 0.02 % | 0.02 % | 0.02 % | 0.02 % |

- The weapon row grows faster than a flat amount: each group of three steps adds 3 more a step than the one before (7, 7, 7, then 10,
  10, 10, then 13, ...).
- **More material lowers the chance a little**: the roll is "material count + a number below a million, under the step's rate", so each
  piece of material used takes one millionth off. One piece is the best choice.
- A failure keeps the step; a **dangerous** Lapisia destroys the item when it fails. Each try costs one Lapisia, the material and the
  Lapisia's price.
- Without the file, the server falls back to tables built into it: a flat 7 attack and 5 absorption a step, and much lower chances
  (70 % for +1, 1 % for +6, one in a million for +13 and above). Since the material always counts against the roll, a chance of one in
  a million can never come up: with these tables a weapon stops at +12 and armour at +14. The original server ships with the file, so
  the values above are the original's.
- **The client agrees with the file at every step**: its character window and its weapon tooltip add exactly the attack row above,
  and its armour tooltip shows "Absorption" as 5 a step (section 12). The client works these out itself and reads them from no file, so a
  server using other values shows players numbers the client does not.

Source: [Lapis and enchanting](../original/lapis.md) sections 2 and 5 (`CGameData::InitItemEnchant`, `CUser::ItemLapisianAdd`).

## 7. Buffs, debuffs and passive skills

Everything a buff, a debuff or a passive skill does to your numbers is one of these **abilities** (a buff adds them while it lasts, a
debuff takes them off, a passive adds them for as long as you know the skill):

| ability | changes | ability | changes |
|---|---|---|---|
| 1, 2, 3 | max HP, MP, SP | 4, 6, 9 | STR, INT, LUC |
| 5 | REC (and max HP 5 a point) | 7 | WIS (and max MP 5 a point) |
| 8 | DEX (and max SP 5 a point) | 10, 11, 12 | sitting recovery, % of max HP, SP, MP |
| 13, 14 | skill SP and MP costs, − % | 15 | attack range |
| 16 | attack speed | 17 | move speed |
| 18 | critical | 19 | cooldowns of 500 s or less removed |
| 20, 21, 22 | hit: melee, ranged, magic | 23, 24, 25 | attack: melee, ranged, magic |
| 26, 27, 28 | defence: melee, ranged, magic | 29, 30, 31 | evasion: melee, ranged, magic |
| 32, 33, 34 | melee, ranged, magic attacks disabled | | |

A debuff never lowers your maxima (abilities 1 to 3 do nothing in a debuff). Other skills work outside these numbers: blocks, reflection,
giving up SP or MP, element buffs ([Elements, resistances and defences](elements-defences.md)), heals over time, transformations.

**Low-HP passives** (one per class pair) start when a blow leaves you at or below their share of your maximum HP, last 30 s, and can start
again only 180 s after the last time:

| skill (class) | starts at HP | gives (levels 1 / 2 / 3) |
|---|---|---|
| Potential Power (fighter, warrior) | 10 / 15 / 20 % | attack speed +3, melee attack +30 / +75 / +120 |
| Potential Avoid (defender, guardian) | 10 / 15 / 20 % | melee and ranged evasion +60 / +75 / +90 |
| Potential Nimble (ranger, assassin) | 10 / 15 / 20 % | magic defence +30 / +50 / +80, critical +90 / +120 / +250 |
| Potential Viable (archer, hunter) | 8 / 10 / 12 % | move speed +2, critical +8 / +15 / +25 |
| Potential Motive (mage, pagan, priest, oracle) | 10 / 17 / 25 % | every skill with a cooldown of 500 s or less has none while it lasts |

**Transformed** (a transformation skill or item): your weapon and armour's attack, spread, defence and magic defence are replaced by
the monster's, or by nothing for a disguise; stats, maxima and speeds still come from your equipment.

Source: [Skills 1](../original/skills.md) sections 5 and 6 (`CUser::SetSkillAbility`), [Skills 3](../original/skills-buffs.md)
sections 5 and 11, [Skills 4](../original/skills-special.md) section 5.4.

## 8. Speeds

**Attack speed** = the weapon's + accessories' + lapis' + weapon-speed passives + buffs, held between 0 and 9. Every action, a normal
blow or a skill, waits this long before the next:

| attack speed | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| wait | 4.0 s | 3.0 s | 2.75 s | 2.5 s | 2.25 s | 2.0 s | 1.75 s | 1.5 s | 1.25 s | 1.0 s |

Weapons carry 2 to 7 by themselves. **Move speed** starts at 2 for everyone and accessories, lapis and buffs move it between 0 and 4;
a stunned, sleeping or held character has neither. A skill with a cast time casts first (its ready time × 0.25 s) and then waits as above
([Hitting and being hit](hitting.md#2-the-timeline)).

Source: [Fighting monsters](../original/combat.md) section 2 (`g_ConstUserAttackSpeed`), [Character numbers](../original/stats.md)
sections 1 and 3.

## 9. Recovery

Every 5 seconds, for HP, SP and MP alike:

| you are | you get back |
|---|---|
| standing | **4** |
| sitting | **5 % of the maximum + 28** (+ the sitting-recovery % of buffs, abilities 10 to 12) |
| fighting (attacking, or until you stop) | **nothing** (only the faction bonus's fight recovery) |

The faction kill-count bonus adds a share of the maximum to the standing and sitting amounts. **No stat changes recovery**: every class
sits back to full in about 20 ticks (100 s), and SP and MP spent in a fight come back only from potions, skills or the faction bonus.

Source: `CUser::UpdateRecover` and the starting amounts of `CUser::InitEquipment`, [Monsters fighting back](../original/mob-fight.md)
section 9.

## 10. Modes

| | easy | normal | hard | ultimate |
|---|---|---|---|---|
| top level | 30 | 60 | 60 | 60 |
| HP, SP and MP of the status table | × 1 | × 1 | × 1.1 | × 1.5 |
| stat / skill points a level | 5 / 3 | 5 / 3 | 7 / 4 | 9 / 5 |
| skills and items | easy's only | up to normal's | up to hard's | all |
| experience to level 60 | | 284.7 million | 1,125 million | 1,405 million |
| death | 5 % of the level's experience (from level 11) | the same | the same | **the character is deleted**, unless a resurrection skill or item brings it back (a death by a player only costs experience) |

Each skill and item names the lowest mode that may use it; the strongest level-60 skills of four classes are ultimate's only.

Source: [Fighting monsters](../original/combat.md) section 5, [Monsters fighting back](../original/mob-fight.md) section 8,
[Moving and equipping](../original/items.md) section 3; the experience totals from the original's experience table.

## 11. The faction kill-count bonus

Each faction counts its players' kills of the other faction. At 150, 300, ... up to 12,288 kills it reaches a new grade (30 in all), and
every player of the faction gets the grade's bonuses. When a faction reaches the top grade, the other faction's count goes back to 0;
the top lasts 10 minutes, then that faction too starts again at grade 0. What the grades add, built up (each grade keeps the ones below):

| bonus | Light | Fury |
|---|---|---|
| max HP % | none | +6 % from grade 3, +9 % from 13 |
| max MP and SP % | +5 % from 27 | +6 % from 3, +9 % from 13, +11 % from 27 |
| ranged defence % | +2 % from 14 | +7 % from 14 |
| magic defence | +2 from 14 | +7 from 14 |
| melee defence % | +2 % from 15 | +2 % from 15 |
| recovery, % of the maximum | 5 %, then 10 % | 5 to 6 %, then 10 % |
| fight recovery of SP and MP | 3 % from 26 | 3 % from 26 |
| all six stats | +10 from 29 | +10 from 29 |
| critical +30, every evasion +30 | grade 30 only | grade 30 only |
| lapis link and extraction chance | +1 to +5 | the same |
| experience lost at death | − 50 % to − 100 % | the same |

So **the two factions' grades are not mirrored**: at the same kill count Fury's give more HP, MP, SP and defence.

Two quirks of the original: the melee and ranged defence percentages count only after a grade *changes* while you are online
(entering the world or a stat reset drops them until the next change); and entering the world gives the magic defence bonus as magic
defence and the DEX bonus as max MP, while a grade change gives them as magic evasion and max SP.

Source: [Player kills 2](../original/pvp-2.md) sections 8 to 10 (`CUser::InitKCStatus`, `CUser::UpdateKCStatus`); the values are the
original's `KillStatus.SData`.

## 12. What the character window shows

The client's character window **computes** attack, magic attack, defence and magic defence itself, with the same factors as the server;
no packet carries them. HP, SP and MP come from the server.

| the window | the server |
|---|---|
| attack = trunc(STR × 1.3) + trunc(DEX × 0.2) + items + lapis + enchant + buffs (the top of the range adds the spread); ranged weapons STR + trunc(DEX × 0.2) + trunc(LUC × 0.3) + ... | the same, but the fractions are added before cutting, so the server's number can be **1 higher** |
| enchant attack from its own formula, the same as the original's file at every step (+3 = 21, +4 = 31, +10 = 106, +20 = 311); the weapon's tooltip adds the same | the file's values |
| no armour enchant in the window; the armour's **tooltip** shows "Absorption" 5 × the step | the file's values (5 a step) |
| no faction bonus, no wear, no weapon or shield mastery, no armour enchant | all of these |
| no hit, evasion, critical, ranged defence or absorption | used in every blow |

So the window is right for a fresh, unworn set without mastery skills or the faction bonus, and shows too little otherwise (or too much
for a worn item). Source: [Character window](../client/research-balance.md), "The character window" (US `game.exe`). The enchant lines
were checked again in `game.exe` for this guide: the formula at 0x4c4920 (its factor 1/3 at 0x6ac568) gives 0, 7, 14, 21, 31 ... 311 for
steps 0 to 20; the window calls it at 0x4f48f4 and the weapon tooltip at 0x4a0cb0 for enchant values 1 to 50; the armour tooltip
computes 5 × (value − 50) at 0x4a2830 and prints it with message 485, "Absorption". The only "block" messages, 402 and 403, are printed
by the friends block list's answer (0x5559b0).

## 13. Worked example A: a level-60 ultimate fighter

A human fighter, ultimate mode, level 60. **Base stats**: STR 403, DEX 162, REC 60, INT 8, WIS 7, LUC 15 (the start 14 / 12 / 9 / 8 / 7 / 11,
LUC + 4 for the human, STR + 59 for the levels, and the 531 free points as STR + 330, DEX + 150, REC + 51). **Equipment** (real rows of the
original's item data), new, without crafted stats, no skills or buffs:

| slot | item | adds |
|---|---|---|
| weapon | **Hallowed Spear** (level 59, hard mode and up), **+3**, with **Assault Lapis LV4** (attack + 16) and **Craft Lapis LV4** (STR + 10) | attack 246, spread 54, attack speed 2, STR 40, DEX 8 |
| helmet, upper armour, lower armour, gloves, boots | the **Daynorth** set (the fighter's: Helmet, Armor, Pants, Gauntlet, Boots) | defence 7 + 35 + 25 + 12 + 16 = 95, magic defence 4 + 17 + 12 + 6 + 8 = 47, HP 110 + 650 + 491 + 239 + 335 = 1,825, SP 26 + 156 + 121 + 65 + 87 = 455, each piece STR 4, DEX 1, REC 2 |
| in the upper armour | **Life Lapis LV4** | HP + 240 |

**Totals**: STR 403 + 40 + 5 × 4 + 10 = **473**; DEX 162 + 8 + 5 × 1 = **175**; REC 60 + 5 × 2 = **70**; INT **8**; WIS **7**; LUC **15**.

**Maxima**:

```
HP = trunc(4,013 × 1.5) + 5 × 70 + 1,825 + 240            = 6,019 + 350 + 1,825 + 240 = 8,434
SP = trunc(804 × 1.5) + 10 (human) + 5 × 175 + 455         = 1,206 + 10 + 875 + 455   = 2,546
MP = trunc(283 × 1.5) + 5 × 7                              = 424 + 35                 = 459
```

**The fighting numbers** (item attack = 246 + 16 + 21 for +3 = **283**):

```
melee hit      = 60 + 175 × 0.5                        = 147.5  → 147
melee evasion  = 60 + 175 × 0.5 + 15 × 0.125           = 149.375 → 149
melee attack   = 473 × 1.3 + 175 × 0.2 + 283           = 614.9 + 35 + 283 = 932.9 → 932   (+ a roll of 0 to 53 each blow)
ranged attack  = 473 + 175 × 0.2 + 15 × 0.3 + 283      = 795.5 → 795
magic attack   = 8 × 1.3 + 7 × 0.2 + 283               = 294.8 → 294
defence        = 70 + 95                               = 165     (melee and ranged)
magic defence  = 7 + 47                                = 54
critical       = 15 × 0.19                             = 2.85 → 2
attack speed 2: one action every 2.75 s; move speed 2
```

**The character window** shows attack 614 + 35 + 283 = 932 (here the same: the two fractions, 0.9 and 0, add to less than 1), with
the spread on top for the higher figure; defence 165, magic attack 294, magic defence 54. Worn below half quality, the server would use 123 + 16 + 21
item attack and half the armour's defence while the window kept showing 932.

Checked with the balance model (`tools/balance/`, which follows the server's code): the same maxima and numbers.

The next page takes this fighter into a fight: [Hitting and being hit](hitting.md#9-worked-examples).

## Shaiya Afterglow

The server follows the original for everything on this page, with these differences:

- **The faction bonus is worked out the same way every time**: the defence percentages always count, the DEX bonus always goes to max SP
  and the magic defence bonus always to magic defence (the original's two quirks of section 11 are fixed).
- Spending points that would take a base stat above 32,767 is refused (the database's limit; the original would wrap at 65,536, out of
  reach in play).
- **Its enchant values are the built-in tables** (a flat 7 attack and 5 absorption a step, the low chances), not the original's file:
  from +4 a weapon has less attack than the original gives and than the window shows, and weapons stop at +12, armour at +14. This
  difference was found while writing this guide; the server is to load the original's file instead (planned, not built yet).

The full list: [Deviations](../server/deviations.md).
