# Balance analysis, part 3: the classes

The twelve classes at levels 15, 30 and 60 (and 55, for levelling through the 51-58 fields), built the way a player would build them,
against the yardstick monsters and against each other; the faction pairs; the modes; the levers for each imbalance; and the questions
only the maintainer can answer. **Analysis only**: nothing in the game, the database (read-only queries), the server code or the client was
changed, and nothing here is a decision. Part 1 (`docs/plans/balance/1-formulas.md`) explains the formulas and the model, part 2
(`docs/plans/balance/2-content.md`) inventories what exists.

**Status words** as in parts 1 and 2: **verified** = read in the server's code or the data as the server loads it (`file::function` or
the query is named); **modelled** = an output of `tools/balance/` (expected values: see its README and section 2.6 here for what the
fights leave out); **inferred** = a reasonable reading not checked in code or play. Every number in the tables is modelled unless it
says otherwise.

**The tables**: `tools/balance/builds.py` makes the builds and runs every fight (about eight minutes with four processes) and writes
CSV tables to `data/balance/` (git-ignored); `tools/balance/builds_report.py` prints this document's tables from them. Every build is a
`model.py` string (appendix A; `builds.csv` has all of them, including the PvE and variant strings), so any line here can be rerun with
`python3 model.py char|mob|pvp "<string>"`.

| CSV | what |
|---|---|
| `builds_plans.csv` | per class, mode and level: the weapon kinds tried and their scores, the skills learnt, the points left |
| `builds_search.csv` | every stat split and lapis order tried (35 × 2 per class and level, two rounds) with its PvE and PvP scores |
| `builds.csv` | the chosen builds: split, lapis order, the PvP and PvE strings |
| `builds_pve.csv` | each build against its monsters, with plain, expected and good gear and with a party's buffs |
| `builds_variants.csv` | the builds without the percentage skills, and without the mage's damage over time, the points spent again |
| `builds_pvp.csv` | every pair at 15, 30, 60 in every variant: seconds to kill |
| `builds_factions.csv`, `builds_modes.csv` | the faction steps; normal and hard against ultimate |
| `builds_lever_wall.csv`, `builds_lever_skills.csv`, `builds_lever_hp.csv` | the levers of section 8 |

**Revised 2026-11-06** (every table regenerated with `builds.py` and `builds_report.py`; two causes):

- **Enchant tables** (commit e9ae177): the server and the model now read `Data/ItemEnchant.ini`. The builds' gear is weapon +3 and armour
  +4, which give the same 21 attack and 20 absorption as the old built-in tables, so no fight changed from this; only the cost of
  those steps fell (about 90 dangerous Lapisia to about 30, section 2.1) and part 2 section 4.3 was rewritten.
- **Physical PvP averages** (commit 829aeaa): the model's expected physical blow in PvP had used a 17-point grid whose fractional
  spread values were truncated, so it came out about 0.5 a blow low. Kill times in PvP are now shorter by up to a few percent: 30 of
  the document's table rows changed, each by 1 to 3 seconds at most, in the PvP kill-time tables (section 5), the faction and
  mode comparisons, the stat-split scores and the PvE tables' PvP columns. The chosen builds are the same except one, the
  normal-mode level-30 assassin, which now picks weapon 10 over 15 and has 497 attack, not 486 (the modes table).
  No ranking or conclusion changed.

## 0. The maintainer's goals (asked 2026-10-26)

| question | answer |
|---|---|
| PvE or PvP? | **Both equally.** |
| Which levels matter? | **The level cap (60)** most; also **15 and 30**, the levels of the two smaller PvP maps. |
| Which modes? | **Mostly ultimate** on private servers, a mix on the original server; ultimate used to be gated behind a level. The maintainer wants to discuss **why keep easy, normal and hard at all if everyone plays ultimate** (section 7). |
| How far from the original? | **A rebalance**, and **lay out what could or should be reworked**. |
| Faction differences? | **Decide after the analysis**: show where the factions differ today first (section 6). |
| Lapis and enchanting? | **Fully in scope**: strength, success rates and costs. |

## 1. What stands out (details in the sections named)

1. **Gear decides more than class** (sections 3.3, 4.2, 8.1). The gear a player reaches in play (grade-35 lapis in every room, crafted
   numbers of a kept drop, weapon +3, armour +4) **doubles or triples HP** at every level: a level-60 fighter has 8,201 HP with plain
   gear and 17,540 with expected gear, a mage 3,839 and 12,945. Almost all of it is lapis: level-56-60 armour has five or six rooms a
   piece, and a Life Lapis Lv4 (grade 35, 12 % a link) is 240 HP a room. Crafted HP at 50 a point instead of 100 would take only about
   750 HP off a level-60 build.
2. **The 51-58 wall is a gear wall** (sections 4.2, 8.2). With plain gear, eight of the twelve level-55 builds (every ranged and
   magic class) die before killing the most placed level-55 field monster (1501, map 45), and the fighter, defender, warrior and
   guardian live only 1.0 to 1.9 times the kill. With expected gear every class solos it (it dies after 55 to 252 s, against kills of
   7.5 to 60 s), at 40 to 185 hours a level.
3. **At 60, PvP is fast and lopsided against the casters** (section 5). The median physical class kills in 4 to 7.5 s and the casters
   are killed in a median 4.8 s. Mages and pagans win 2 to 3 of their 11 duels, priests and oracles 0 to 1; rangers and assassins 10.
   A fighter kills a mage in 2.5 s with three skills (Regina Power, Enhance Wind Spin, Deadly Strike; 3.8 s without Regina Power)
   against its 95 defence.
4. **The level-60 percentage skills halve the kill times of their four classes** (sections 5.3, 8.3): without 180 to 184 the fighter's
   median goes from 7.5 to 18 s, the archer's from 4.8 to 12, the mage's from 11 to 24; the defenders then win 8 or 9 duels instead of
   4 to 6. Halving their value takes back most of it for the fighter, archer and mage, nothing for the ranger and assassin (their other
   skills kill in the same 5 s).
5. **The mage's damage over time is not decisive with real gear** (section 5.4): part 2 found Flame Shock taking more than a mirror's
   HP in one use against part 1's bare builds; with real HP, halving every mage damage-over-time skill changes the mage's median kill
   time by under a second at 30 and 60.
6. **The defenders are nearly unkillable at 15 and 30** (section 5.1): at 30 the defender and guardian are killed in a median 180 s and
   the fighter and warrior never kill them (Secure Guard: melee attack and defence +500 for 40 s; REC; the shield; physical damage is
   attack minus defence). At 60 they are killed in 17 to 18 s.
7. **The stat split matters less than the gear, but all-into-the-damage-stat is the worst choice** (section 3.2): the best splits are
   within a few per cent of each other, and putting every point into STR or INT scores 0.38 to 0.59 of the best in PvP at 60. DEX
   (hit and evasion) and **LUC** (critical, evasion, and a cut to the other side's critical) are in most chosen splits; LUC takes half
   or more of the free points in seven of the twelve level-60 builds.
8. **Casters cast a third faster with a short sword than with a staff** (section 3.4): every action, spells too, waits the weapon's
   attack interval (verified, part 1 section 4.1), and a short sword has speed 7 (1,500 ms) where staves have 5 (2,000 ms); the item
   attack counts for spells. Every mage, pagan, priest and oracle build but the level-15 oracle takes a short sword.
9. **The factions are even when the builds are equal** (section 6): the race's stats and HP move a pair's kill times by about 10 % or
   less. What
   moves them is the race skills (the human fighter's Speedy Attack, the vail's Bloody Karma), Fury's kill-count grades
   (at grade 13 the fighter needs 6.2 s for the warrior instead of 3.8), and the oracle being stronger than the priest at every level.
10. **Normal and hard characters are no match for ultimate ones** (section 7): at 60 a normal character is killed by the ultimate
    twelve in a median 4.8 to 11 s and needs 26 to 215 s to kill them; hard 6.1 to 14 s and 9.4 to 31 s. Normal levels about three
    times faster to 30.
11. **Correction to part 2 (section 9.1)**: the guild halls (maps 53, 54) are **not** closed off. Gatekeeper 44 stands in Erina (map
    1) and gatekeeper 45 in Reikeuseu (map 2), and each takes its faction to the halls for 9,900 with no guild check; the ranking battle
    needs only a guild. Part 2's "closed circle" was wrong.
12. **Three model fixes** (section 9.2): skills that take a share of the target's current HP are now taken of what is left, skills that
    cost the whole SP or MP pool are kept for last, and the gear picker values lapis rooms and crafted numbers. Part 1's level-60 sample
    tables come out the same.

## 2. How the builds were made

### 2.1 The steps (modelled; `builds.py`)

1. **Weapon**: for every weapon kind the class may wield and can get at the level (and one-handed with a shield where it may carry one),
   the picker's best obtainable item of that kind; kept: the kind whose reference build (3/4 of the free points into the damage stat,
   1/4 DEX or WIS, every skill) does best on PvE and PvP together (`choose_weapon`; the options and scores are in
   `builds_plans.csv`).
2. **Skills, within the points** (75 at 15, 150 at 30, 300 at 60 in ultimate; 3 and 4 a level in normal and hard;
   `item_use::reset_skill_points`): first the passives that count for the worn weapon (and shield) and the ability passives; then the
   class's lasting and opening buffs (section 2.3); for the priest Healing and Recovery; then the attack skills in the order of the
   damage they do in the reference fights (the field monster, the mirror class); at 60 the percentage skill; then defensive and debuff
   skills the model does not count (blocks, reflection, heals, stuns), until the points run out (`choose_skills`). Each at the highest
   level the points left allow. Interpretation and Backpack Upgrade are never learnt.
3. **Stats and lapis order**: every split of the free points in quarters over four stats (STR, DEX, REC, LUC for the physical classes;
   INT, WIS, REC, LUC for the casters; 35 splits) and two lapis orders (the build's main stat first, or HP first): 70 candidates per
   class and level. Each is scored on **PvE** (experience per hour against the level's field monster: 2125 at 15, 2183 at 30, 1501 at
   60) and **PvP** (the geometric mean, over the twelve classes, of their kill time over ours, each clipped to 0.1 to 10), each
   relative to the best candidate, and the build with the best geometric mean of the two is kept (`score_candidates`). **Two rounds**:
   the first against the twelve reference builds, the second against the first round's choices; five of the 36 choices changed in
   the second round (`builds.csv` `round1`). Level 55 takes the level-60 split; normal and hard take the ultimate split of their level.
4. **Gear** (part 2 sections 3.3 to 4.3), what a player reaches in play:
   - **expected** (every table unless it says otherwise): weapon **+3**, armour **+4** (the dangerous Lapisia of Cailum Sacra: 20
     Lapisia and 10 weapons for +3, 14 and 4 pieces for +4); **crafted numbers** of a kept drop: the item's count of numbers sitting on
     the stats the build wants (HP first), each of the mean size of the smallest of three draws (3 for a level-60 armour piece: 300 HP
     or 3 points; 6 on a level-60 weapon); **grade-35 lapis** (Lv4, the Dual lapis; 9 to 12 % a link) in every room but the cloak's,
     chosen per room by the build's wants (`model.pick_lapis`); a basic **element lapis** (grade 34, 17 to 20 %) in the weapon and the
     upper armour. The picker values the rooms and crafted numbers when it chooses items.
   - **good**: crafted numbers at the size a tenth of drops reach (7 on level-60 armour: 700 HP), grade-36 lapis (Lv5; 1 to 4 % a link).
   - **plain**: no enchant, no crafted numbers, no lapis: a character as it finds its gear.
   What expected gear costs at 60 (modelled arithmetic from part 2): 29 armour rooms of Life Lapis Lv4 at 12 % is about 240 links
   (33 million at 137,900 a try) and as many lapis (monster 738 drops one in 7); the enchant steps need about 30 dangerous Lapisia (5
   for the weapon, 5 a piece for five armour pieces; about 90 with the server's old built-in tables) from the Cailum Sacra floor
   bosses (one in 10 a kill).

### 2.2 Elements

In PvP every build carries the same element in the weapon and the upper armour (fire), so the element factor is 1.0 both ways (part 1
section 4.6): the tables show the classes, not an element matchup (one element ahead would be × 1.4 or × 1.3, one behind × 0.5).
Against a monster the PvE string swaps the weapon's element lapis for the one strong against that monster (`best_element`); the
armour's element cuts every monster blow by 20 % (part 1 section 4.4).

### 2.3 Buffs and race skills (verified data, `defs.skills`; the choice which are on is inferred)

| | lasting (on in every fight) | opening (cast before a PvP fight, on until the first runs out) |
|---|---|---|
| fighter / warrior | Bash (critical +15), Concentration (melee hit +21), Leadership (melee attack +41, party), Energy Control (SP +800) | Berserk Fighter (melee attack +150, defence +100, 40 s); Speedy Attack (attack speed +2, 45 s, human) / Power up Pause (melee attack +170, 30 s, deatheater) |
| defender / guardian | Bash, Concentration, Hard Skin (HP +244, defence +13), Tactical Formation (defence +42, party), Magic Skin (magic defence +45), Energy Control | Secure Guard (melee attack and defence +500, 40 s) |
| ranger / assassin | Agility (DEX +60), Incantation (LUC +90, party) | Perfect Position (evasion +20, critical +39, 30 s), Big Coup (hit +500, 60 s), Flash Attack (attack speed +1, 60 s); Bloody Karma (vail) |
| archer / hunter | Sharp Shooter (ranged hit +21), Marksman Song (ranged attack and defence +39, party) | Big Coup, Flash Attack |
| mage / pagan | Support Spell (magic attack +80, party), Body Protector (defence +47) | Bloody Karma (vail pagan) |
| priest / oracle | Prayer (STR, REC, WIS +40, party), Body Protector | Bloody Karma (vail oracle) |

Race skills: **Berserker** (human: hit +8 all kinds, 180 s every 180 s) is lasting; **Bloody Karma** (vail: HP +458, melee and magic
attack +108, 60 s every 180 s) opens a PvP fight; **Battle Cry** (deatheater: sitting HP recovery +5 %) only shortens rest between
kills; **Evasion** (elf: evasion +240 for 30 s) costs every SP and MP point and is not cast (section 6.3). The **party** variant
(section 4.3) adds the other classes' party buffs and the priest's Aura of Peace (HP +850). Etain Bless (185, the priest's level-60
buff) costs the whole pool and is not cast.

### 2.4 The fights

- **PvE**: the model's rotation against the monster from the SP/MP pool (and with potions, "unlimited"), its damage per second on the
  build, and **experience per hour** = the kill's experience (solo, × 1.2, no bonuses) over the cycle: the kill, the attack timer after
  the last blow, **5 s to reach the next monster** (inferred; spawn density is not modelled) and the rest sitting until HP, SP and MP
  are back (5 % of the maximum + 28 every 5 s, plus the sit-recovery passives; part 1 section 3.6). Zero when the build dies first.
  A kill time of 0.0 means the first blow kills.
- **PvP**: both sides from full health, at range, from the SP/MP pool; with each side's opening buffs until the first of them runs out
  (30 to 60 s), then on without them, the HP lost and the SP/MP left carried over (`builds.ttk`). "never" = not within 300 s.
- **Variants**: without the percentage skills 180 to 184, and also without the mage's and pagan's damage-over-time skills (Flame
  Shock, Sustain Doom, Hellfire, Meteor, the two Leakages): the skill points are spent again on other skills (`builds_variants.csv`);
  **opening debuffs**: each class's defence-lowering debuffs landed on the target for the whole fight (Wide Shock, Panic and Magic Range,
  Tetanus and Misfortune or Blunt, Shot Pointer, confusion); **plain** and **good** gear.

### 2.5 Two things a reader should know about the numbers

- **Many splits score nearly the same** (section 3.2): the chosen build is the best of a flat top, and a rerun after a small change can
  pick a neighbour. The conclusions here are the ones that hold across the top splits; a single cell of a table should not be read as
  exact.
- **Expected values, best case**: the model has no variance, no movement, no healing, no blocks or reflection (Untouchable, Melee
  Reflector, Magic Mirror, Dayfly Shield), no stuns' lasting effect, no lapis procs (part 1 section 7.4). Healers and defenders are
  therefore shown weaker in PvP than they are; the burst classes stronger.

## 3. The builds

### 3.1 The character sheets (modelled; ultimate, expected gear, lasting buffs; the PvP fights add the opening buffs of section 2.3)

"Free points" are the free stat points by stat (126 at 15, 261 at 30, 531 at 60), on top of the class's starting stats and its
point a level. "Lapis first": which the build's lapis order puts first (each room then takes the lapis worth most to the build, so a
"stat" build still fills armour rooms the stat lapis cannot enter with Life Lapis). Attack, hit and defence are of the build's own
kind (magic for the casters, melee or ranged otherwise); evasion and defence are melee.

#### Level 15 (ultimate)

| class | weapon | free points | lapis first | skill points | HP | SP / MP | attack | hit | evasion | defence | magic def. | crit % | absorb | interval ms |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fighter | Noble War-mace (1H mace + shield) | STR 95 DEX 31 | stat | 50/75 | 3,110 | 1,659 / 154 | 279 | 53 | 45 | 40 | 18 | 7 | 100 | 1750 |
| defender | Noble War-mace (1H mace + shield) | STR 95 DEX 31 | stat | 54/75 | 3,438 | 1,426 / 219 | 232 | 52 | 43 | 98 | 29 | 7 | 100 | 1750 |
| ranger | Noble Iron Claw (claw) | STR 126 | stat | 50/75 | 2,232 | 1,313 / 272 | 275 | 49 | 60 | 25 | 21 | 5 | 80 | 1500 |
| archer | Noble Lightning Bow (crossbow) | STR 126 | stat | 42/75 | 2,195 | 1,022 / 380 | 249 | 32 | 31 | 35 | 23 | 5 | 80 | 2250 |
| mage | Noble Light Short Sword (short sword) | INT 126 | stat | 43/75 | 1,816 | 255 / 1,502 | 289 | 0 | 29 | 32 | 33 | 1 | 80 | 1500 |
| priest | Noble Light Short Sword (short sword) | INT 63 WIS 63 | stat | 40/75 | 1,926 | 253 / 1,872 | 187 | 3 | 26 | 39 | 126 | 2 | 80 | 1500 |
| warrior | Noble Thorns Club (1H mace + shield) | STR 31 DEX 64 REC 31 | stat | 50/75 | 3,285 | 1,829 / 154 | 202 | 68 | 62 | 73 | 20 | 7 | 100 | 1750 |
| guardian | Noble Thorns Club (1H mace + shield) | STR 95 DEX 31 | stat | 54/75 | 3,458 | 1,416 / 219 | 235 | 49 | 43 | 100 | 31 | 6 | 100 | 1750 |
| assassin | Noble Iron Claw (claw) | STR 126 | stat | 50/75 | 2,232 | 1,298 / 277 | 274 | 47 | 58 | 25 | 23 | 5 | 80 | 1500 |
| hunter | Noble Piercing Javelin (javelin) | STR 126 | stat | 42/75 | 2,215 | 1,002 / 370 | 250 | 30 | 29 | 37 | 23 | 5 | 80 | 2250 |
| pagan | Noble Light Short Sword (short sword) | INT 95 WIS 31 | stat | 43/75 | 1,816 | 225 / 1,672 | 260 | 0 | 25 | 32 | 68 | 1 | 80 | 1500 |
| oracle | Noble Black Bird (staff) | INT 95 WIS 31 | stat | 43/75 | 1,926 | 248 / 1,707 | 230 | 0 | 25 | 39 | 92 | 1 | 80 | 2000 |

#### Level 30 (ultimate)

| class | weapon | free points | lapis first | skill points | HP | SP / MP | attack | hit | evasion | defence | magic def. | crit % | absorb | interval ms |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fighter | Heroic Heaven's Door (1H mace + shield) | STR 131 REC 130 | stat | 134/150 | 10,681 | 2,042 / 216 | 494 | 77 | 54 | 231 | 26 | 14 | 120 | 1750 |
| defender | Heroic Heaven's Door (1H mace + shield) | STR 65 DEX 131 REC 65 | stat | 139/150 | 10,897 | 2,169 / 326 | 316 | 167 | 144 | 327 | 37 | 14 | 120 | 1750 |
| ranger | Heroic Mephisto (claw) | STR 261 | stat | 99/150 | 7,991 | 2,729 / 464 | 587 | 108 | 132 | 44 | 32 | 13 | 100 | 1500 |
| archer | Heroic Siege Bow (bow) | LUC 261 | stat | 121/150 | 7,870 | 1,957 / 796 | 281 | 57 | 90 | 63 | 50 | 68 | 100 | 1750 |
| mage | Heroic Apsaras (short sword) | INT 131 WIS 130 | stat | 102/150 | 6,962 | 333 / 3,874 | 471 | 0 | 48 | 52 | 220 | 4 | 100 | 1500 |
| priest | Heroic Apsaras (short sword) | INT 196 WIS 65 | stat | 138/150 | 7,270 | 362 / 3,555 | 498 | 5 | 47 | 79 | 197 | 4 | 100 | 1500 |
| warrior | Heroic Chaos Mash (1H mace + shield) | STR 131 REC 130 | stat | 134/150 | 10,701 | 2,032 / 216 | 497 | 72 | 54 | 233 | 30 | 13 | 120 | 1750 |
| guardian | Heroic Chaos Mash (1H mace + shield) | STR 65 DEX 131 REC 65 | stat | 139/150 | 10,917 | 2,159 / 326 | 319 | 162 | 144 | 329 | 41 | 13 | 120 | 1750 |
| assassin | Heroic Mephisto (claw) | STR 261 | stat | 99/150 | 7,991 | 2,714 / 469 | 586 | 106 | 130 | 44 | 34 | 13 | 100 | 1500 |
| hunter | Heroic Siege Arch (bow) | LUC 261 | stat | 121/150 | 7,890 | 1,937 / 786 | 282 | 55 | 88 | 65 | 50 | 68 | 100 | 1750 |
| pagan | Heroic Apsaras (short sword) | INT 131 WIS 130 | stat | 102/150 | 6,962 | 318 / 3,854 | 479 | 0 | 46 | 52 | 217 | 4 | 100 | 1500 |
| oracle | Heroic Apsaras (short sword) | LUC 261 | stat | 131/150 | 7,275 | 337 / 3,680 | 160 | 0 | 83 | 80 | 141 | 63 | 100 | 1500 |

#### Level 60 (ultimate)

| class | weapon | free points | lapis first | skill points | HP | SP / MP | attack | hit | evasion | defence | magic def. | crit % | absorb | interval ms |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fighter | Mellan Skia (spear) | STR 399 DEX 132 | stat | 298/300 | 17,540 | 6,993 / 459 | 1188 | 186 | 161 | 152 | 46 | 22 | 100 | 1750 |
| defender | Ampu Fatum (1H mace + shield) | DEX 132 LUC 399 | stat | 300/300 | 21,083 | 6,184 / 720 | 401 | 181 | 214 | 430 | 111 | 110 | 120 | 1500 |
| ranger | Buse Nuikterion (claw) | STR 266 LUC 265 | stat | 224/300 | 15,835 | 7,117 / 952 | 779 | 157 | 234 | 82 | 64 | 83 | 100 | 1250 |
| archer | Death Foshubil (crossbow) | DEX 132 LUC 399 | stat | 285/300 | 15,494 | 6,889 / 1,676 | 629 | 187 | 234 | 120 | 66 | 103 | 100 | 1500 |
| mage | Cultist's Blade (short sword) | WIS 266 LUC 265 | stat | 273/300 | 12,945 | 450 / 12,154 | 466 | 0 | 118 | 95 | 470 | 67 | 100 | 1500 |
| priest | Optare Rogarr (short sword) | INT 531 | stat | 279/300 | 13,952 | 577 / 12,035 | 1078 | 8 | 80 | 168 | 256 | 3 | 100 | 1500 |
| warrior | Mellan Skia (spear) | STR 267 DEX 132 LUC 132 | stat | 300/300 | 18,090 | 5,963 / 679 | 1025 | 180 | 180 | 140 | 50 | 48 | 100 | 1750 |
| guardian | Draco Cranio (1H mace + shield) | DEX 132 LUC 399 | stat | 300/300 | 21,663 | 5,154 / 940 | 410 | 175 | 216 | 424 | 115 | 110 | 120 | 1500 |
| assassin | Heartseeker (short blade) | STR 399 LUC 132 | stat | 224/300 | 16,026 | 5,956 / 1,580 | 978 | 144 | 202 | 79 | 70 | 55 | 100 | 1250 |
| hunter | Artraiya Adein (javelin) | DEX 132 LUC 399 | stat | 285/300 | 16,049 | 5,824 / 1,886 | 642 | 184 | 232 | 109 | 70 | 105 | 100 | 1500 |
| pagan | Cultist's Blade (short sword) | WIS 266 LUC 265 | stat | 273/300 | 13,165 | 855 / 11,004 | 481 | 0 | 118 | 99 | 457 | 68 | 100 | 1500 |
| oracle | Optare Rogarr (short sword) | INT 531 | stat | 279/300 | 14,112 | 992 / 10,955 | 1095 | 0 | 82 | 160 | 255 | 3 | 100 | 1500 |

### 3.2 The stat split (modelled; `builds_search.csv`, second round)

Each cell: the split and lapis order, then its PvE and PvP score relative to the best of the 70 candidates (1.00 = the best).

#### Level 15: the chosen split against the best for PvE alone, PvP alone and all into the damage stat

| class | chosen (PvE / PvP, 1 = the best) | best PvE | best PvP | all damage stat | round 1 choice |
|---|---|---|---|---|---|
| fighter | str:3,dex:1 stat: 0.95 / 1.00 | str:1,dex:3 stat: 1.00 / 0.75 | str:3,dex:1 stat: 0.95 / 1.00 | str:4 stat: 0.93 / 0.86 | str:3,dex:1 stat |
| defender | str:3,dex:1 stat: 1.00 / 1.00 | str:3,luc:1 stat: 1.00 / 0.74 | str:3,dex:1 stat: 1.00 / 1.00 | str:4 stat: 1.00 / 0.92 | str:3,dex:1 stat |
| ranger | str:4 stat: 0.99 / 1.00 | str:2,dex:1,luc:1 stat: 1.00 / 0.62 | str:4 stat: 0.99 / 1.00 | str:4 stat: 0.99 / 1.00 | str:4 stat |
| archer | str:4 stat: 1.00 / 0.64 | str:4 stat: 1.00 / 0.64 | rec:4 stat: 0.49 / 1.00 | str:4 stat: 1.00 / 0.64 | str:4 stat |
| mage | int:4 stat: 1.00 / 0.87 | int:4 stat: 1.00 / 0.87 | int:3,wis:1 stat: 0.80 / 1.00 | int:4 stat: 1.00 / 0.87 | int:4 stat |
| priest | int:2,wis:2 stat: 1.00 / 1.00 | int:2,wis:2 stat: 1.00 / 1.00 | int:2,wis:2 stat: 1.00 / 1.00 | int:4 stat: 0.93 / 0.86 | int:2,wis:2 stat |
| warrior | str:1,dex:2,rec:1 stat: 0.98 / 0.99 | str:1,dex:3 stat: 1.00 / 0.88 | str:2,dex:1,rec:1 stat: 0.95 / 1.00 | str:4 stat: 0.93 / 0.75 | str:1,dex:2,rec:1 stat |
| guardian | str:3,dex:1 stat: 1.00 / 1.00 | str:3,luc:1 stat: 1.00 / 0.71 | str:3,dex:1 stat: 1.00 / 1.00 | str:4 stat: 1.00 / 0.91 | str:3,dex:1 stat |
| assassin | str:4 stat: 0.99 / 1.00 | str:2,dex:1,luc:1 stat: 1.00 / 0.63 | str:4 stat: 0.99 / 1.00 | str:4 stat: 0.99 / 1.00 | str:4 stat |
| hunter | str:4 stat: 1.00 / 0.67 | str:4 stat: 1.00 / 0.67 | rec:4 stat: 0.49 / 1.00 | str:4 stat: 1.00 / 0.67 | str:4 stat |
| pagan | int:3,wis:1 stat: 1.00 / 1.00 | int:3,wis:1 stat: 1.00 / 1.00 | int:3,wis:1 stat: 1.00 / 1.00 | int:4 stat: 0.98 / 0.84 | int:3,wis:1 stat |
| oracle | int:3,wis:1 stat: 0.97 / 1.00 | int:2,wis:2 stat: 1.00 / 0.93 | int:3,wis:1 stat: 0.97 / 1.00 | int:4 stat: 0.93 / 0.89 | int:3,wis:1 stat |

#### Level 30: the chosen split against the best for PvE alone, PvP alone and all into the damage stat

| class | chosen (PvE / PvP, 1 = the best) | best PvE | best PvP | all damage stat | round 1 choice |
|---|---|---|---|---|---|
| fighter | str:2,rec:2 stat: 0.93 / 0.90 | str:2,dex:2 stat: 1.00 / 0.74 | dex:2,rec:2 stat: 0.64 / 1.00 | str:4 stat: 0.97 / 0.65 | str:2,dex:2 stat |
| defender | str:1,dex:2,rec:1 stat: 0.88 / 0.94 | str:1,luc:3 stat: 1.00 / 0.47 | dex:3,rec:1 stat: 0.79 / 1.00 | str:4 stat: 0.99 / 0.55 | str:1,dex:2,rec:1 stat |
| ranger | str:4 stat: 0.99 / 1.00 | str:2,dex:2 stat: 1.00 / 0.77 | str:4 stat: 0.99 / 1.00 | str:4 stat: 0.99 / 1.00 | str:4 stat |
| archer | luc:4 stat: 0.99 / 1.00 | str:4 stat: 1.00 / 0.79 | luc:4 stat: 0.99 / 1.00 | str:4 stat: 1.00 / 0.79 | luc:4 stat |
| mage | int:2,wis:2 stat: 1.00 / 1.00 | wis:4 stat: 1.00 / 0.41 | int:2,wis:2 stat: 1.00 / 1.00 | int:4 stat: 0.95 / 0.67 | int:2,wis:2 stat |
| priest | int:3,wis:1 stat: 1.00 / 0.87 | int:4 stat: 1.00 / 0.56 | luc:4 stat: 0.78 / 1.00 | int:4 stat: 1.00 / 0.56 | int:3,wis:1 stat |
| warrior | str:2,rec:2 stat: 0.93 / 0.83 | str:2,dex:2 stat: 1.00 / 0.68 | dex:2,rec:2 stat: 0.64 / 1.00 | str:4 stat: 0.97 / 0.56 | str:2,dex:2 stat |
| guardian | str:1,dex:2,rec:1 stat: 0.88 / 0.93 | str:1,luc:3 stat: 1.00 / 0.47 | dex:3,rec:1 stat: 0.79 / 1.00 | str:4 stat: 0.99 / 0.54 | str:1,dex:2,rec:1 stat |
| assassin | str:4 stat: 0.99 / 0.90 | str:2,dex:2 stat: 1.00 / 0.71 | str:2,rec:2 stat: 0.85 / 1.00 | str:4 stat: 0.99 / 0.90 | str:4 stat |
| hunter | luc:4 stat: 0.99 / 1.00 | str:3,dex:1 stat: 1.00 / 0.74 | luc:4 stat: 0.99 / 1.00 | str:4 stat: 0.70 / 0.78 | luc:4 stat |
| pagan | int:2,wis:2 stat: 0.99 / 1.00 | wis:4 stat: 1.00 / 0.33 | int:2,wis:2 stat: 0.99 / 1.00 | int:4 stat: 0.95 / 0.55 | int:2,wis:2 stat |
| oracle | luc:4 stat: 0.78 / 1.00 | int:4 stat: 1.00 / 0.76 | luc:4 stat: 0.78 / 1.00 | int:4 stat: 1.00 / 0.76 | int:4 stat |

#### Level 60: the chosen split against the best for PvE alone, PvP alone and all into the damage stat

| class | chosen (PvE / PvP, 1 = the best) | best PvE | best PvP | all damage stat | round 1 choice |
|---|---|---|---|---|---|
| fighter | str:3,dex:1 stat: 1.00 / 0.79 | str:3,dex:1 stat: 1.00 / 0.79 | str:1,dex:3 stat: 0.71 / 1.00 | str:4 stat: 0.85 / 0.54 | str:3,dex:1 stat |
| defender | dex:1,luc:3 stat: 0.89 / 1.00 | str:3,dex:1 stat: 1.00 / 0.87 | dex:1,luc:3 stat: 0.89 / 1.00 | str:4 stat: 0.83 / 0.64 | str:1,dex:1,luc:2 stat |
| ranger | str:2,luc:2 stat: 1.00 / 0.93 | str:2,luc:2 stat: 1.00 / 0.93 | str:1,luc:3 stat: 0.93 / 1.00 | str:4 stat: 0.97 / 0.88 | str:2,luc:2 stat |
| archer | dex:1,luc:3 stat: 1.00 / 1.00 | dex:1,luc:3 stat: 1.00 / 1.00 | dex:1,luc:3 stat: 1.00 / 1.00 | str:4 stat: 0.94 / 0.67 | dex:1,luc:3 stat |
| mage | wis:2,luc:2 stat: 1.00 / 0.89 | wis:2,luc:2 stat: 1.00 / 0.89 | wis:2,rec:1,luc:1 stat: 0.74 / 1.00 | int:4 stat: 0.93 / 0.75 | wis:2,luc:2 stat |
| priest | int:4 stat: 1.00 / 1.00 | int:4 stat: 1.00 / 1.00 | int:4 stat: 1.00 / 1.00 | int:4 stat: 1.00 / 1.00 | int:4 stat |
| warrior | str:2,dex:1,luc:1 stat: 1.00 / 0.77 | str:2,dex:1,luc:1 stat: 1.00 / 0.77 | dex:4 stat: 0.64 / 1.00 | str:4 stat: 0.86 / 0.46 | str:2,dex:1,luc:1 stat |
| guardian | dex:1,luc:3 stat: 0.90 / 1.00 | str:3,dex:1 stat: 1.00 / 0.83 | dex:1,luc:3 stat: 0.90 / 1.00 | str:4 stat: 0.79 / 0.62 | dex:1,luc:3 stat |
| assassin | str:3,luc:1 stat: 0.98 / 0.95 | str:2,luc:2 stat: 1.00 / 0.93 | dex:1,luc:3 stat: 0.73 / 1.00 | str:4 stat: 0.97 / 0.95 | str:3,luc:1 stat |
| hunter | dex:1,luc:3 stat: 1.00 / 1.00 | dex:1,luc:3 stat: 1.00 / 1.00 | dex:1,luc:3 stat: 1.00 / 1.00 | str:4 stat: 0.93 / 0.65 | dex:1,luc:3 stat |
| pagan | wis:2,luc:2 stat: 1.00 / 0.89 | wis:2,luc:2 stat: 1.00 / 0.89 | wis:2,rec:2 stat: 0.74 / 1.00 | int:4 stat: 0.93 / 0.78 | wis:2,luc:2 stat |
| oracle | int:4 stat: 1.00 / 1.00 | int:4 stat: 1.00 / 1.00 | int:4 stat: 1.00 / 1.00 | int:4 stat: 1.00 / 1.00 | int:3,wis:1 stat |

- **The top is flat**: at 60 most classes have three or more splits within 0.05 of the best on both scores.
- **All into the damage stat is the worst kind of build for PvP at 60** (0.38 to 0.59 of the best for the melee classes, the archer and
  the mage): STR adds 1.3 attack a point, but without DEX the build misses (hit is a contest of both sides' hit and evasion, part 1
  section 4.2) and without LUC it neither criticals nor dodges.
- **LUC** pays three ways at 60: 0.19 % critical a point (a level-60 build with 400 LUC criticals on most blows; a critical is × 1.5 and
  a larger luck roll), 0.125 evasion a point, and 0.034 % off every opponent's critical a point (part 1 section 3.3). It is in nine of
  the twelve level-60 choices.
- **REC** is chosen at 30 by the fighters and defenders (the long fights of section 5.1), rarely at 60: lapis and crafted numbers give
  far more HP than 5 a point.
- **The casters split by level**: INT at 15, INT and WIS at 30 (the mage's MP pool and magic defence), WIS and LUC for the mage and
  pagan at 60 and all INT for the priest and oracle at 60.

### 3.3 Where the HP comes from (modelled; `builds_lever_hp.csv`)

#### HP from gear: the real build, with crafted HP/MP/SP at 50 a point, and with plain gear

| class | level | HP | crafted at 50 | plain gear |
|---|---|---|---|---|
| fighter | 15 | 3,110 | 2,910 | 1,720 |
| fighter | 30 | 10,681 | 10,281 | 4,451 |
| fighter | 60 | 17,540 | 16,790 | 8,201 |
| defender | 15 | 3,438 | 3,238 | 2,048 |
| defender | 30 | 10,897 | 10,497 | 4,722 |
| defender | 60 | 21,083 | 20,183 | 10,249 |
| ranger | 15 | 2,232 | 2,082 | 1,187 |
| ranger | 30 | 7,991 | 7,641 | 2,606 |
| ranger | 60 | 15,835 | 15,085 | 6,524 |
| archer | 15 | 2,195 | 2,045 | 1,150 |
| archer | 30 | 7,870 | 7,520 | 2,485 |
| archer | 60 | 15,494 | 14,744 | 6,180 |
| mage | 15 | 1,816 | 1,666 | 786 |
| mage | 30 | 6,962 | 6,612 | 1,592 |
| mage | 60 | 12,945 | 12,195 | 3,839 |
| priest | 15 | 1,926 | 1,776 | 896 |
| priest | 30 | 7,270 | 6,920 | 1,900 |
| priest | 60 | 13,952 | 13,202 | 4,636 |
| warrior | 15 | 3,285 | 3,085 | 1,895 |
| warrior | 30 | 10,701 | 10,301 | 4,456 |
| warrior | 60 | 18,090 | 17,340 | 8,841 |
| guardian | 15 | 3,458 | 3,258 | 2,068 |
| guardian | 30 | 10,917 | 10,517 | 4,742 |
| guardian | 60 | 21,663 | 20,763 | 10,889 |
| assassin | 15 | 2,292 | 2,142 | 1,247 |
| assassin | 30 | 8,195 | 7,845 | 2,810 |
| assassin | 60 | 16,484 | 15,734 | 7,233 |
| hunter | 15 | 2,215 | 2,065 | 1,170 |
| hunter | 30 | 7,890 | 7,540 | 2,505 |
| hunter | 60 | 16,049 | 15,299 | 6,795 |
| pagan | 15 | 1,876 | 1,726 | 846 |
| pagan | 30 | 7,166 | 6,816 | 1,796 |
| pagan | 60 | 13,623 | 12,873 | 4,582 |
| oracle | 15 | 1,986 | 1,836 | 956 |
| oracle | 30 | 7,479 | 7,129 | 2,094 |
| oracle | 60 | 14,570 | 13,820 | 5,314 |

The level-60 fighter as an example (`model.py` with parts of the gear left out): 8,201 HP plain; 9,731 with crafted numbers and enchants
but no lapis; 16,010 with lapis but no crafted numbers; 17,540 with both. The lapis are worth about 7,800 HP (29 armour rooms of Life
Lapis Lv4 at 240, less what the plain set's items had), the crafted numbers about 1,500 (three numbers of 3 on five pieces, HP 300 each
piece), and the class's ultimate status HP 6,019 of it.

### 3.4 Casters and daggers (verified data; modelled effect)

Every action waits the weapon's attack interval, a spell too (part 1 section 4.1, `docs/original/skills-use.md` sections 2 to 7), the
item attack is shared by every kind (part 1 section 3.3), and every attack spell allows every weapon (`defs.skills` `needweapon` 1 to
15 for Flame Shock, Kamain Breath, Sonic Bolt, Turbulence, Hailstorm, Magic Spot). The level-59 short sword Cultist's Blade (10/106 to
10/141; mage, priest, ranger) has attack 130 + 18 and speed 7, the staves Draconic Staff 146-174 + 32-34 and Tiamat 158 + 34 speed 5:
a third more casts a second for about a fifth less item attack. The weapon search picks the short sword for every caster build from
level 30 (and for the mage, pagan and priest at 15). Whether the client lets a caster cast at that pace with a short sword is client
question 19 (section 10).

## 4. PvE

### 4.1 Kill time, death time and experience (modelled; expected gear, solo, lasting buffs)

At 55 and 60 the second and third monsters are 1547 (level 58, maps 45 to 47, part 2's level-60 yardstick) and 982 (level 53, the most
placed monster of the Elemental Cave, maps 25 and 60: defence 72, attack 310, a dungeon monster). A level-60 character gets nothing for
982 (more than 6 levels below, part 1 section 5.2). "Hours a level" is the experience to pass that level (ultimate) at this rate.

#### Level 15: kill and death against monster 2125

| class | 2125: kill s (pool / potions) | 2125: dies in s | 2125: exp / h | hours a level |
|---|---|---|---|---|
| fighter | 1.8 / 1.8 | never | 69,989 | 0.7 |
| defender | 3.5 / 3.5 | never | 83,132 | 0.6 |
| ranger | 3.0 / 3.0 | never | 77,687 | 0.6 |
| archer | 2.2 / 2.2 | never | 66,360 | 0.7 |
| mage | 2.5 / 2.5 | never | 76,428 | 0.6 |
| priest | 1.5 / 1.5 | never | 62,830 | 0.8 |
| warrior | 1.8 / 1.8 | never | 71,792 | 0.7 |
| guardian | 3.5 / 3.5 | never | 83,146 | 0.6 |
| assassin | 3.0 / 3.0 | never | 77,564 | 0.6 |
| hunter | 2.2 / 2.2 | never | 66,110 | 0.7 |
| pagan | 2.5 / 2.5 | never | 77,750 | 0.6 |
| oracle | 2.0 / 2.0 | never | 56,704 | 0.8 |

#### Level 30: kill and death against monster 2183

| class | 2183: kill s (pool / potions) | 2183: dies in s | 2183: exp / h | hours a level |
|---|---|---|---|---|
| fighter | 0.0 / 0.0 | never | 244,776 | 3.6 |
| defender | 7.0 / 7.0 | never | 143,385 | 6.1 |
| ranger | 3.0 / 3.0 | never | 212,111 | 4.2 |
| archer | 0.0 / 0.0 | never | 273,049 | 3.2 |
| mage | 3.0 / 3.0 | never | 121,907 | 7.2 |
| priest | 1.5 / 1.5 | never | 179,520 | 4.9 |
| warrior | 0.0 / 0.0 | never | 244,546 | 3.6 |
| guardian | 7.0 / 7.0 | never | 143,385 | 6.1 |
| assassin | 3.0 / 3.0 | never | 211,981 | 4.2 |
| hunter | 0.0 / 0.0 | never | 272,489 | 3.2 |
| pagan | 3.0 / 3.0 | never | 121,590 | 7.2 |
| oracle | 3.0 / 3.0 | never | 139,648 | 6.3 |

#### Level 55: kill and death against monsters 1501, 982

| class | 1501: kill s (pool / potions) | 1501: dies in s | 1501: exp / h | 982: kill s (pool / potions) | 982: dies in s | 982: exp / h | hours a level |
|---|---|---|---|---|---|---|---|
| fighter | 7.5 / 7.5 | 118 | 3,079,609 | 4.5 / 4.5 | never | 345,816 | 40.2 |
| defender | 27 / 22 | 144 | 977,593 | 10 / 10 | never | 208,009 | 126.6 |
| ranger | 12 / 12 | 90 | 2,504,011 | 6.2 / 6.2 | 284 | 414,902 | 49.4 |
| archer | 60 / 60 | 94 | 667,492 | 20 / 20 | 298 | 304,194 | 185.4 |
| mage | 10 / 10 | 55 | 2,072,338 | 6.0 / 6.0 | never | 371,333 | 59.7 |
| priest | 9.0 / 9.0 | 60 | 2,606,468 | 7.5 / 7.5 | never | 403,861 | 47.5 |
| warrior | 9.0 / 9.0 | 132 | 2,649,967 | 4.5 / 4.5 | never | 400,636 | 46.7 |
| guardian | 18 / 18 | 252 | 1,152,782 | 10 / 10 | never | 242,793 | 107.3 |
| assassin | 11 / 11 | 85 | 2,659,546 | 6.2 / 6.2 | 278 | 384,671 | 46.5 |
| hunter | 54 / 54 | 102 | 806,667 | 20 / 20 | never | 314,120 | 153.4 |
| pagan | 10 / 10 | 56 | 1,981,364 | 6.0 / 6.0 | never | 353,473 | 62.5 |
| oracle | 7.5 / 7.5 | 61 | 2,841,570 | 7.5 / 7.5 | never | 388,991 | 43.5 |

#### Level 60: kill and death against monsters 1501, 1547, 982

| class | 1501: kill s (pool / potions) | 1501: dies in s | 1501: exp / h | 1547: kill s (pool / potions) | 1547: dies in s | 1547: exp / h | 982: kill s (pool / potions) | 982: dies in s | 982: exp / h |
|---|---|---|---|---|---|---|---|---|---|
| fighter | 1.8 / 1.8 | 112 | 732,907 | 3.5 / 3.5 | 81 | 2,540,078 | 1.8 / 1.8 | never | 0 |
| defender | 9.0 / 9.0 | never | 245,049 | 10 / 10 | 205 | 934,374 | 6.0 / 6.0 | never | 0 |
| ranger | 8.8 / 8.8 | 105 | 549,870 | 10 / 10 | 77 | 1,978,780 | 3.8 / 3.8 | never | 0 |
| archer | 6.0 / 6.0 | 114 | 225,729 | 7.5 / 7.5 | 83 | 921,567 | 6.0 / 6.0 | never | 0 |
| mage | 6.5 / 6.5 | 57 | 452,480 | 9.5 / 9.5 | 51 | 1,685,001 | 4.5 / 4.5 | never | 0 |
| priest | 7.5 / 7.5 | 64 | 497,119 | 9.0 / 9.0 | 53 | 1,861,717 | 6.0 / 6.0 | never | 0 |
| warrior | 1.8 / 1.8 | 120 | 687,957 | 3.5 / 3.5 | 86 | 2,382,230 | 1.8 / 1.8 | never | 0 |
| guardian | 9.0 / 9.0 | never | 278,975 | 10 / 10 | 206 | 1,064,470 | 6.0 / 6.0 | never | 0 |
| assassin | 8.8 / 8.8 | 94 | 538,245 | 10 / 10 | 71 | 1,939,818 | 3.8 / 3.8 | never | 0 |
| hunter | 6.0 / 6.0 | 114 | 243,846 | 6.0 / 6.0 | 84 | 1,028,457 | 6.0 / 6.0 | never | 0 |
| pagan | 6.5 / 6.5 | 58 | 433,500 | 9.5 / 9.5 | 52 | 1,622,285 | 4.5 / 4.5 | never | 0 |
| oracle | 7.5 / 7.5 | 64 | 482,229 | 7.5 / 7.5 | 53 | 2,033,874 | 6.0 / 6.0 | never | 0 |

- **15 and 30 are trivial in PvE** with expected gear: every build kills the yardstick in 0 to 9 s and is never in danger; a level takes
  under an hour at 15 and 3 to 7 hours at 30.
- **At 55 the classes spread fourfold**: 40 to 50 hours a level for the fighter, warrior, assassin, ranger, priest and oracle; 60 for
  the mage and pagan; 107 to 127 for the defender and guardian; 153 to 185 for the archer and hunter, whose level-60 split (LUC and DEX,
  no STR, chosen for PvP) kills 1501 in 54 to 60 s. The level-55 builds take the level-60 split (section 2.1); an archer levelling
  through 55 would put more into STR.
- **At 60** against 1547 every class lives 51 to 206 s and kills in 3.5 to 10 s (0.9 to 2.5 million experience an hour, for nothing:
  the cap).

### 4.2 Gear and the wall (modelled; level 55 against 1501)

#### Level 55 against 1501: gear and a party's buffs

| class | plain gear: HP: kill / die s | expected gear | good gear | expected + party buffs |
|---|---|---|---|---|
| fighter | 8,138: 18 / 34 | 19,301: 7.5 / 118 | 25,951: 6.0 / 174 | 20,351: 6.0 / 155 |
| defender | 8,234: 42 / 43 | 17,574: 27 / 144 | 23,084: 22 / 204 | 18,624: 14 / 176 |
| ranger | 5,784: 36 / 25 | 15,124: 12 / 90 | 20,634: 11 / 127 | 16,174: 10 / 111 |
| archer | 5,449: 54 / 24 | 14,789: 60 / 94 | 20,299: 48 / 136 | 15,839: 28 / 120 |
| mage | 3,413: 11 / 10 | 12,748: 10 / 55 | 18,228: 10 / 79 | 13,798: 9.0 / 70 |
| priest | 4,111: 18 / 12 | 13,451: 9.0 / 60 | 18,961: 9.0 / 86 | 14,301: 7.5 / 70 |
| warrior | 8,778: 24 / 36 | 19,941: 9.0 / 132 | 26,591: 7.5 / 188 | 20,991: 6.0 / 172 |
| guardian | 9,719: 39 / 61 | 20,827: 18 / 252 | 27,507: 16 / never | 21,877: 12 / never |
| assassin | 6,010: 35 / 24 | 15,350: 11 / 85 | 20,860: 10 / 119 | 16,400: 10 / 104 |
| hunter | 6,089: 58 / 28 | 15,429: 54 / 102 | 20,939: 44 / 148 | 16,479: 28 / 130 |
| pagan | 3,698: 11 / 11 | 12,968: 10 / 56 | 18,228: 10 / 79 | 14,018: 9.0 / 72 |
| oracle | 4,331: 16 / 13 | 13,671: 7.5 / 61 | 18,961: 7.5 / 85 | 14,521: 7.5 / 72 |

With plain gear the ranger, archer, mage, priest, assassin, hunter, pagan and oracle die before 1501 does, and the fighter, defender,
warrior and guardian live 1.0 to 1.9 times the kill time. Expected gear doubles or triples their HP: every class then outlives the
kill, the archer and hunter by 1.6 to 1.9 times, the melee classes by 8 to 16. Section 8.2 has the monster's side of the same
question.

### 4.3 A party's buffs (modelled)

The last column of the table above adds the other classes' party buffs (Leadership, Tactical Formation, Incantation, Marksman Song,
Support Spell, Prayer, Aura of Peace): kill times fall 10 to 55 % (the archer's and hunter's most, 60 → 28 s and 54 → 28 s, from
Prayer's STR and Incantation's LUC), HP rises by Aura of Peace's 850 and Prayer's REC. Party kills share experience
(part 1 section 5.2, `docs/original/party.md`), which the table does not show; the level-59 to 64 monsters of 200,000 HP and more are
party content whatever the gear (part 2 section 6).

### 4.4 Healing (not modelled)

The priest's and oracle's death times leave out their own heals (Healing, Recovery, Healing Fall), the defender's Self Aid and the
mage's Diversion, and every potion. Against 1501 a priest loses about 220 HP a second (13,952 HP in 64 s); a repeatable heal of
`heal value + WIS × 2` every 1.5 s would cover much of it at the cost of not attacking (inferred). A healer's PvE is a trade the model
does not make.

## 5. PvP

### 5.1 The twelve against the twelve (modelled; seconds for the row to kill the column; the column kills the row in the mirrored cell)

#### PvP, level 15: seconds for the row to kill the column

| kills → | Fig | Def | Ran | Arc | Mag | Pri | War | Gua | Asn | Hun | Pag | Ora |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Fig** | 30 | 55 | 16 | 13 | 11 | 12 | 55 | 58 | 16 | 14 | 11 | 12 |
| **Def** | 66 | 152 | 38 | 30 | 24 | 26 | 121 | 157 | 39 | 30 | 25 | 29 |
| **Ran** | 36 | 69 | 22 | 20 | 15 | 16 | 62 | 70 | 22 | 20 | 15 | 18 |
| **Arc** | 62 | 75 | 39 | 30 | 20 | 21 | 102 | 75 | 40 | 29 | 22 | 23 |
| **Mag** | 28 | 34 | 18 | 18 | 16 | never | 30 | 36 | 20 | 18 | 26 | 38 |
| **Pri** | never | never | 16 | 18 | 16 | never | never | never | 18 | 18 | never | never |
| **War** | 48 | 146 | 23 | 21 | 16 | 19 | 95 | 151 | 22 | 21 | 16 | 19 |
| **Gua** | 67 | 152 | 40 | 30 | 24 | 26 | 123 | 153 | 39 | 30 | 25 | 27 |
| **Asn** | 35 | 66 | 20 | 18 | 14 | 15 | 60 | 68 | 20 | 18 | 15 | 16 |
| **Hun** | 65 | 79 | 40 | 32 | 21 | 22 | 116 | 79 | 42 | 30 | 22 | 24 |
| **Pag** | 30 | 38 | 20 | 20 | 18 | never | 32 | 38 | 20 | 20 | 24 | 42 |
| **Ora** | 50 | never | 16 | 16 | 16 | never | 184 | never | 18 | 18 | 24 | never |

#### PvP, level 30: seconds for the row to kill the column

| kills → | Fig | Def | Ran | Arc | Mag | Pri | War | Gua | Asn | Hun | Pag | Ora |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Fig** | 86 | never | 62 | 46 | 15 | 18 | 86 | never | 62 | 46 | 15 | 22 |
| **Def** | 87 | never | 37 | 33 | 16 | 18 | 87 | never | 39 | 32 | 18 | 19 |
| **Ran** | 60 | 172 | 25 | 26 | 18 | 20 | 62 | 174 | 26 | 26 | 19 | 22 |
| **Arc** | 65 | 179 | 30 | 30 | 18 | 20 | 63 | 177 | 30 | 30 | 18 | 24 |
| **Mag** | 38 | 40 | 28 | 30 | never | never | 38 | 40 | 28 | 30 | never | never |
| **Pri** | never | never | 21 | 22 | never | never | never | never | 21 | 22 | never | never |
| **War** | 79 | never | 65 | 46 | 16 | 18 | 76 | never | 65 | 46 | 16 | 24 |
| **Gua** | 87 | never | 39 | 32 | 16 | 18 | 87 | never | 39 | 33 | 18 | 19 |
| **Asn** | 52 | 172 | 22 | 22 | 16 | 18 | 54 | 176 | 24 | 24 | 16 | 20 |
| **Hun** | 65 | 180 | 30 | 30 | 18 | 20 | 65 | 180 | 30 | 30 | 18 | 24 |
| **Pag** | 34 | 36 | 24 | 26 | never | never | 36 | 38 | 26 | 26 | never | never |
| **Ora** | 93 | 224 | 34 | 57 | 44 | 58 | 99 | 238 | 39 | 57 | 44 | 76 |

#### PvP, level 30 without the mage's damage over time: seconds for the row to kill the column

| kills → | Fig | Def | Ran | Arc | Mag | Pri | War | Gua | Asn | Hun | Pag | Ora |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Fig** | 86 | never | 62 | 46 | 15 | 18 | 86 | never | 62 | 46 | 15 | 22 |
| **Def** | 87 | never | 37 | 33 | 16 | 18 | 87 | never | 39 | 32 | 18 | 19 |
| **Ran** | 60 | 172 | 25 | 26 | 18 | 20 | 62 | 174 | 26 | 26 | 19 | 22 |
| **Arc** | 65 | 179 | 30 | 30 | 18 | 20 | 63 | 177 | 30 | 30 | 18 | 24 |
| **Mag** | 38 | 40 | 27 | 30 | never | never | 38 | 40 | 28 | 30 | never | 46 |
| **Pri** | never | never | 21 | 22 | never | never | never | never | 21 | 22 | never | never |
| **War** | 79 | never | 65 | 46 | 16 | 18 | 76 | never | 65 | 46 | 16 | 24 |
| **Gua** | 87 | never | 39 | 32 | 16 | 18 | 87 | never | 39 | 33 | 18 | 19 |
| **Asn** | 52 | 172 | 22 | 22 | 16 | 18 | 54 | 176 | 24 | 24 | 16 | 20 |
| **Hun** | 65 | 180 | 30 | 30 | 18 | 20 | 65 | 180 | 30 | 30 | 18 | 24 |
| **Pag** | 33 | 36 | 24 | 26 | never | 217 | 34 | 36 | 26 | 26 | never | 39 |
| **Ora** | 93 | 224 | 34 | 57 | 44 | 58 | 99 | 238 | 39 | 57 | 44 | 76 |

#### PvP, level 60, every skill: seconds for the row to kill the column

| kills → | Fig | Def | Ran | Arc | Mag | Pri | War | Gua | Asn | Hun | Pag | Ora |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Fig** | 3.8 | 17 | 8.8 | 8.8 | 2.5 | 2.5 | 3.8 | 18 | 7.5 | 8.8 | 2.5 | 2.5 |
| **Def** | 18 | 108 | 26 | 26 | 9.0 | 10 | 20 | 110 | 24 | 27 | 9.0 | 10 |
| **Ran** | 5.0 | 20 | 5.0 | 5.0 | 3.0 | 4.0 | 5.0 | 21 | 5.0 | 5.0 | 4.0 | 4.0 |
| **Arc** | 4.8 | 6.5 | 6.0 | 6.0 | 4.8 | 4.8 | 4.8 | 6.5 | 6.0 | 6.0 | 4.8 | 4.8 |
| **Mag** | 9.5 | 16 | 9.0 | 9.0 | 12 | 12 | 9.5 | 16 | 11 | 11 | 12 | 12 |
| **Pri** | 18 | 26 | 16 | 16 | 46 | 22 | 20 | 27 | 18 | 18 | 46 | 24 |
| **War** | 3.5 | 18 | 7.0 | 7.0 | 3.5 | 3.5 | 3.5 | 18 | 5.2 | 7.0 | 3.5 | 3.5 |
| **Gua** | 16 | 103 | 24 | 24 | 9.0 | 10 | 18 | 102 | 22 | 26 | 9.0 | 10 |
| **Asn** | 4.0 | 15 | 5.0 | 5.0 | 3.0 | 3.0 | 4.0 | 15 | 4.0 | 5.0 | 3.0 | 3.0 |
| **Hun** | 4.8 | 6.0 | 6.0 | 6.0 | 4.8 | 4.8 | 4.8 | 6.0 | 6.0 | 6.0 | 4.8 | 4.8 |
| **Pag** | 9.0 | 16 | 9.0 | 9.0 | 11 | 12 | 9.5 | 16 | 9.0 | 9.0 | 12 | 12 |
| **Ora** | 16 | 24 | 15 | 15 | 36 | 20 | 18 | 24 | 16 | 16 | 38 | 20 |

#### PvP, level 60 without the percentage skills 180 to 184: seconds for the row to kill the column

| kills → | Fig | Def | Ran | Arc | Mag | Pri | War | Gua | Asn | Hun | Pag | Ora |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Fig** | 11 | 49 | 18 | 18 | 3.8 | 3.8 | 12 | 49 | 18 | 18 | 3.8 | 3.8 |
| **Def** | 18 | 108 | 26 | 26 | 9.0 | 10 | 20 | 110 | 24 | 27 | 9.0 | 10 |
| **Ran** | 10 | 39 | 10 | 10 | 6.0 | 7.0 | 10 | 39 | 10 | 10 | 6.0 | 7.0 |
| **Arc** | 10 | 20 | 13 | 13 | 7.2 | 7.8 | 12 | 21 | 12 | 14 | 7.8 | 7.8 |
| **Mag** | 24 | 34 | 22 | 22 | 29 | 30 | 24 | 37 | 24 | 24 | 30 | 33 |
| **Pri** | 18 | 26 | 16 | 16 | 46 | 22 | 20 | 27 | 18 | 18 | 46 | 24 |
| **War** | 12 | 46 | 19 | 19 | 3.5 | 3.5 | 13 | 46 | 19 | 19 | 3.5 | 5.2 |
| **Gua** | 16 | 103 | 24 | 24 | 9.0 | 10 | 18 | 102 | 22 | 26 | 9.0 | 10 |
| **Asn** | 7.0 | 35 | 9.0 | 9.0 | 5.0 | 6.0 | 8.0 | 35 | 9.0 | 10 | 5.0 | 6.0 |
| **Hun** | 10 | 19 | 13 | 13 | 6.5 | 7.8 | 10 | 20 | 12 | 13 | 7.8 | 7.8 |
| **Pag** | 22 | 31 | 21 | 21 | 28 | 30 | 24 | 32 | 22 | 22 | 30 | 32 |
| **Ora** | 16 | 24 | 15 | 15 | 36 | 20 | 18 | 24 | 16 | 16 | 38 | 20 |

#### PvP, level 60 without the percentage skills and the mage's damage over time: seconds for the row to kill the column

| kills → | Fig | Def | Ran | Arc | Mag | Pri | War | Gua | Asn | Hun | Pag | Ora |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Fig** | 11 | 49 | 18 | 18 | 3.8 | 3.8 | 12 | 49 | 18 | 18 | 3.8 | 3.8 |
| **Def** | 18 | 108 | 26 | 26 | 9.0 | 10 | 20 | 110 | 24 | 27 | 9.0 | 10 |
| **Ran** | 10 | 39 | 10 | 10 | 6.0 | 7.0 | 10 | 39 | 10 | 10 | 6.0 | 7.0 |
| **Arc** | 10 | 20 | 13 | 13 | 7.2 | 7.8 | 12 | 21 | 12 | 14 | 7.8 | 7.8 |
| **Mag** | 30 | 54 | 30 | 33 | 60 | 58 | 32 | 58 | 32 | 34 | 60 | 60 |
| **Pri** | 18 | 26 | 16 | 16 | 46 | 22 | 20 | 27 | 18 | 18 | 46 | 24 |
| **War** | 12 | 46 | 19 | 19 | 3.5 | 3.5 | 13 | 46 | 19 | 19 | 3.5 | 5.2 |
| **Gua** | 16 | 103 | 24 | 24 | 9.0 | 10 | 18 | 102 | 22 | 26 | 9.0 | 10 |
| **Asn** | 7.0 | 35 | 9.0 | 9.0 | 5.0 | 6.0 | 8.0 | 35 | 9.0 | 10 | 5.0 | 6.0 |
| **Hun** | 10 | 19 | 13 | 13 | 6.5 | 7.8 | 10 | 20 | 12 | 13 | 7.8 | 7.8 |
| **Pag** | 27 | 46 | 27 | 27 | 54 | 51 | 28 | 49 | 28 | 30 | 56 | 54 |
| **Ora** | 16 | 24 | 15 | 15 | 36 | 20 | 18 | 24 | 16 | 16 | 38 | 20 |

### 5.2 Summary: duels won, median time to kill and to be killed

#### PvP summary, level 15: duels won of 11, median seconds to kill / to be killed

| class | every skill |
|---|---|
| fighter | 10: 14 / 50 |
| defender | 9: 30 / 75 |
| ranger | 4: 20 / 20 |
| archer | 1: 39 / 20 |
| mage | 2: 28 / 16 |
| priest | 3: never / 22 |
| warrior | 9: 21 / 102 |
| guardian | 10: 30 / 75 |
| assassin | 7: 18 / 22 |
| hunter | 0: 40 / 20 |
| pagan | 3: 30 / 22 |
| oracle | 5: 24 / 24 |

#### PvP summary, level 30: duels won of 11, median seconds to kill / to be killed

| class | every skill | without 180-184 and mage DoT |
|---|---|---|
| fighter | 6: 46 / 65 | 6: 46 / 65 |
| defender | 10: 33 / 180 | 10: 33 / 180 |
| ranger | 8: 26 / 30 | 8: 26 / 30 |
| archer | 4: 30 / 30 | 4: 30 / 30 |
| mage | 0: 38 / 18 | 0: 38 / 18 |
| priest | 0: never / 20 | 0: never / 20 |
| warrior | 7: 46 / 65 | 7: 46 / 65 |
| guardian | 10: 33 / 180 | 10: 33 / 180 |
| assassin | 9: 22 / 30 | 9: 22 / 30 |
| hunter | 4: 30 / 30 | 4: 30 / 30 |
| pagan | 0: 36 / 18 | 2: 34 / 18 |
| oracle | 3: 57 / 24 | 2: 57 / 24 |

#### PvP summary, level 60: duels won of 11, median seconds to kill / to be killed

| class | every skill | without 180-184 | without 180-184 and mage DoT | with opening debuffs | plain gear | good gear |
|---|---|---|---|---|---|---|
| fighter | 5: 7.5 / 9.0 | 4: 18 / 16 | 4: 18 / 16 | 5: 7.5 / 9.0 | 8: 2.5 / 4.5 | 6: 10 / 14 |
| defender | 4: 20 / 17 | 8: 20 / 34 | 8: 20 / 39 | 4: 16 / 14 | 7: 6.0 / 7.5 | 4: 24 / 20 |
| ranger | 10: 5.0 / 9.0 | 8: 10 / 18 | 8: 10 / 18 | 10: 5.0 / 8.0 | 6: 3.0 / 4.5 | 8: 6.0 / 12 |
| archer | 8: 4.8 / 9.0 | 8: 12 / 18 | 8: 12 / 18 | 8: 4.8 / 8.0 | 6: 3.5 / 4.5 | 8: 6.0 / 12 |
| mage | 2: 11 / 4.8 | 2: 24 / 7.2 | 0: 34 / 7.2 | 2: 8.0 / 4.8 | 2: 4.5 / 1.5 | 2: 15 / 4.0 |
| priest | 0: 20 / 4.8 | 0: 20 / 7.8 | 2: 20 / 7.8 | 0: 20 / 4.8 | 0: 7.5 / 2.5 | 0: 26 / 5.0 |
| warrior | 7: 5.2 / 9.5 | 5: 19 / 18 | 5: 19 / 18 | 7: 5.2 / 9.0 | 8: 3.5 / 4.5 | 7: 7.0 / 14 |
| guardian | 6: 18 / 18 | 9: 18 / 35 | 9: 18 / 39 | 5: 16 / 14 | 7: 6.0 / 7.5 | 5: 22 / 20 |
| assassin | 10: 4.0 / 9.0 | 9: 8.0 / 18 | 9: 8.0 / 18 | 11: 4.0 / 8.0 | 7: 3.0 / 4.5 | 8: 5.0 / 14 |
| hunter | 8: 4.8 / 9.0 | 9: 10 / 18 | 9: 10 / 18 | 8: 4.8 / 8.0 | 6: 3.5 / 4.5 | 8: 6.0 / 12 |
| pagan | 3: 9.5 / 4.8 | 3: 24 / 7.8 | 1: 30 / 7.8 | 3: 8.0 / 4.8 | 2: 4.5 / 2.5 | 2: 14 / 4.0 |
| oracle | 1: 18 / 4.8 | 1: 18 / 7.8 | 3: 18 / 7.8 | 1: 18 / 4.8 | 1: 7.5 / 2.5 | 1: 24 / 5.0 |

- **15**: the fighter, warrior, defender and guardian win 9 or 10 of 11; the archer and hunter 0 or 1 (they kill in a median 39 to 40 s
  and die in 20); the priest kills none of the four melee classes within 300 s.
- **30**: the defender and guardian win 10 and are killed in a median 180 s (the fighter and warrior never kill them); the mage, pagan
  and priest win none. Fights last a median 22 to 57 s.
- **60**: the ranger and assassin win 10, the archer and hunter 8, the warrior 7, the casters 0 to 3. The casters die in a median 4.8 s
  against every physical class; the defenders take 17 to 18 s; physical builds kill each other in 4 to 9 s.

### 5.3 The level-60 percentage skills (modelled; part 2 section 5.5)

| class | skill | median kill s (with / without) | duels won (with / without) |
|---|---|---|---|
| fighter | 180 Regina Power | 7.5 / 18 | 5 / 4 |
| warrior | 180 Regina Power | 5.2 / 19 | 7 / 5 |
| ranger | 182 Syeeric Touch | 5.0 / 10 | 10 / 8 |
| assassin | 182 Syeeric Touch | 4.0 / 8.0 | 10 / 9 |
| archer | 183 Pantera Vision | 4.8 / 12 | 8 / 8 |
| hunter | 183 Pantera Vision | 4.8 / 10 | 8 / 9 |
| mage | 184 Kamain Breath | 11 / 24 | 2 / 2 |
| pagan | 184 Kamain Breath | 9.5 / 24 | 3 / 3 |

Without them every other class's fight lengthens too (they are the bulk of the burst), and the defenders go from 4 or 6 duels won to 8
or 9. The skills are resisted by most monsters (part 2 section 5.5): their weight is PvP only.

### 5.4 The mage's damage over time (modelled)

With part 1's bare builds Flame Shock took more than a mirror's HP in one use (part 2 section 5.3). With real gear the target has two to
three times the HP and the damage over time is one source among several: at 60 the mage without it (and without Kamain Breath) kills
in a median 34 s against 24 with it, the pagan 30 against 24; halving every mage damage-over-time skill (lever table, section 8.3) moves
the median by under a second.

### 5.5 Opening debuffs and gear (modelled)

Debuffs landed for the whole fight (the "with opening debuffs" column) change little: defence cuts of 49 to 114 matter most against the
defenders' 930 (the guardian is killed in 14 s instead of 18). Gear changes every number: with **plain gear** everyone dies in a median
1.5 to 7.5 s (the casters in 1.5 to 2.5 s); with **good gear** the fights are 20 to 40 % longer than with expected gear and the order of
the classes stays.

## 6. The factions

### 6.1 From equal builds to the real ones (modelled; Light kills Fury / Fury kills Light, seconds)

Step 1 gives the Fury class the Light build whole (split, skills, buffs, items) and neither side its race skills: what is left is the
race's starting stats and HP bonus (part 1 section 6.1). Step 2 adds each side's race skills (Berserker, Speedy Attack for the human
fighter; Power up Pause for the deatheater warrior; Bloody Karma for the vail). Step 3 is each side's own build (its own items, split
and skills). Steps 4 and 5 put both sides at kill-count grade 13 and 30 (part 1 section 6.3).

#### Level 15: Light kills Fury / Fury kills Light, seconds

| pair | same build, no race skills | + race skills | + own builds (the real ones) | + grade 13 both | + grade 30 both |
|---|---|---|---|---|---|
| fighter / warrior | 38 / 36 | 30 / 30 | 55 / 48 | 62 / 48 | 38 / 34 |
| defender / guardian | 158 / 149 | 157 / 152 | 157 / 152 | 167 / 152 | 107 / 98 |
| ranger / assassin | 22 / 24 | 22 / 20 | 22 / 20 | 24 / 20 | 20 / 16 |
| archer / hunter | 29 / 32 | 29 / 32 | 29 / 32 | 32 / 32 | 27 / 25 |
| mage / pagan | 18 / 16 | 18 / 16 | 26 / 18 | 28 / 18 | 32 / 20 |
| priest / oracle | never / never | never / never | never / never | never / never | never / 262 |

#### Level 30: Light kills Fury / Fury kills Light, seconds

| pair | same build, no race skills | + race skills | + own builds (the real ones) | + grade 13 both | + grade 30 both |
|---|---|---|---|---|---|
| fighter / warrior | 96 / 94 | 86 / 79 | 86 / 79 | 95 / 79 | 68 / 63 |
| defender / guardian | never / never | never / never | never / never | never / never | 257 / 234 |
| ranger / assassin | 25 / 26 | 26 / 22 | 26 / 22 | 29 / 22 | 22 / 19 |
| archer / hunter | 30 / 30 | 30 / 30 | 30 / 30 | 32 / 30 | 26 / 22 |
| mage / pagan | never / never | never / never | never / never | never / never | 194 / 151 |
| priest / oracle | never / never | never / never | never / 58 | never / 58 | 80 / 39 |

#### Level 60: Light kills Fury / Fury kills Light, seconds

| pair | same build, no race skills | + race skills | + own builds (the real ones) | + grade 13 both | + grade 30 both |
|---|---|---|---|---|---|
| fighter / warrior | 5.2 / 5.2 | 3.8 / 5.2 | 3.8 / 3.5 | 6.2 / 3.5 | 3.8 / 3.5 |
| defender / guardian | 108 / 109 | 106 / 112 | 110 / 103 | 120 / 100 | 100 / 80 |
| ranger / assassin | 5.0 / 5.0 | 5.0 / 5.0 | 5.0 / 5.0 | 5.0 / 5.0 | 5.0 / 4.0 |
| archer / hunter | 6.0 / 6.0 | 6.0 / 6.0 | 6.0 / 6.0 | 6.0 / 6.0 | 6.0 / 6.0 |
| mage / pagan | 12 / 12 | 12 / 11 | 12 / 11 | 12 / 11 | 16 / 14 |
| priest / oracle | 21 / 21 | 24 / 20 | 24 / 20 | 27 / 20 | 33 / 24 |

#### The real builds side by side (HP, attack, defence; Light / Fury)

| pair | level | HP | attack | defence |
|---|---|---|---|---|
| fighter / warrior | 15 | 3,110 / 3,285 | 279 / 252 | 40 / 73 |
| defender / guardian | 15 | 3,438 / 3,458 | 232 / 235 | 98 / 100 |
| ranger / assassin | 15 | 2,232 / 2,292 | 275 / 289 | 25 / 25 |
| archer / hunter | 15 | 2,195 / 2,215 | 249 / 250 | 35 / 37 |
| mage / pagan | 15 | 1,816 / 1,876 | 289 / 275 | 32 / 32 |
| priest / oracle | 15 | 1,926 / 1,986 | 187 / 245 | 39 / 39 |
| fighter / warrior | 30 | 10,681 / 10,701 | 544 / 647 | 281 / 283 |
| defender / guardian | 30 | 10,897 / 10,917 | 816 / 819 | 527 / 529 |
| ranger / assassin | 30 | 7,991 / 8,195 | 587 / 638 | 44 / 44 |
| archer / hunter | 30 | 7,870 / 7,890 | 281 / 282 | 63 / 65 |
| mage / pagan | 30 | 6,962 / 7,166 | 471 / 531 | 52 / 52 |
| priest / oracle | 30 | 7,270 / 7,479 | 498 / 212 | 79 / 80 |
| fighter / warrior | 60 | 17,540 / 18,090 | 1338 / 1345 | 252 / 240 |
| defender / guardian | 60 | 21,083 / 21,663 | 901 / 910 | 930 / 924 |
| ranger / assassin | 60 | 15,835 / 16,484 | 779 / 1086 | 82 / 79 |
| archer / hunter | 60 | 15,494 / 16,049 | 629 / 642 | 120 / 109 |
| mage / pagan | 60 | 12,945 / 13,623 | 466 / 589 | 95 / 99 |
| priest / oracle | 60 | 13,952 / 14,570 | 1078 / 1203 | 168 / 160 |

### 6.2 What each difference is worth (modelled; verified data where named)

| source | where it shows | size |
|---|---|---|
| race stats and HP (human LUC + 4, elf DEX + 4, vail INT and WIS + 2, deatheater STR and REC + 2; +10 HP, MP or SP) | step 1 | under 10 % of a kill time at every level (38 / 36 s at 15, 5.2 / 5.2 at 60 for the fighters) |
| race skills | step 2 | the human fighter's Speedy Attack (attack speed +2): 5.2 → 3.8 s at 60, 38 → 30 at 15; Bloody Karma (+458 HP, +108 attack) makes the oracle, pagan and assassin harder to kill and faster; Battle Cry and Evasion nothing in a fight |
| items (part 2 section 3.8) | step 3 | mixed with the different splits the search chose; the Fury builds have 20 to 680 HP more in every pair at 30 and 60 (inferred: the race HP, items such as the Fury quest bracelets, and the splits) |
| kill-count grades | steps 4, 5 | at grade 13 Fury has +9 % HP, MP, SP and +7 % ranged and magic defence and Light nothing comparable: the fighter needs 6.2 s for the warrior instead of 3.8, the priest 27 for the oracle instead of 24; at grade 30 both have +10 stats, +30 critical and evasion and the gap closes or turns |
| the oracle and pagan | every level | the oracle kills the priest in 20 s at 60, 58 at 30 (the priest never kills it at 30); the pagan kills the mage faster at 15 (18 s against 26). Their splits differ (the search), and the vail's INT, WIS and Bloody Karma's +108 magic attack add to it |

### 6.3 The elf's Evasion (verified data, inferred use)

Evasion (189) gives +90 / +180 / +240 melee and ranged evasion for 10 / 20 / 30 s and costs every SP and MP point (cost 1, part 1
section 4.1). A ranger or archer who casts it has no skills for the rest of the fight; a mage has no spells. It is therefore not cast
in any table; its value is a 30-second dodge window after which the elf fights with normal blows only. Against Bloody Karma (+458 HP,
+108 attack, no cost) and Berserker (+8 hit, kept up) it is the weakest race skill and the only one with a cost.

## 7. The modes

### 7.1 What the modes give and cost (verified, part 1 section 5.1; part 2 sections 3.1 and 5.1)

| | easy | normal | hard | ultimate |
|---|---|---|---|---|
| top level | 30 | 60 | 60 | 60 |
| status HP/SP/MP | × 1 | × 1 | × 1.1 | × 1.5 |
| free stat points at 60 (a level) | (30: 145) | 295 (5) | 413 (7) | 531 (9) |
| skill points at 60 | (30: 92) | 182 | 241 | 300 |
| skill rows learnable (of 640) | 336 | 481 | 578 | 640 |
| equipment rows wearable (obtainable) | 3,400 (1,802) | 6,906 (4,033) | 8,824 (4,682) | 9,720 (5,201) |
| the best level-60 weapons | no | no (hard quests) | yes | yes |
| experience to 60 | (cap 30) | 284.7 million | 1,125 million | 1,405 million |
| death | 5 % of the level's span (part 1 section 5.2) | the same | the same | **the original deletes the character**; built here as the fall (kept, restorable; corrected, `docs/plans/balance/plan.md` section 2) |

### 7.2 Normal and hard against ultimate (modelled; the same split as the ultimate build of the level, the mode's own weapon, gear and skills)

#### Level 15: the modes (normal / hard / ultimate)

| class | HP | attack | skill points | field monster kill:die s | hours a level | normal, hard against the ultimate twelve (median kill:killed s) |
|---|---|---|---|---|---|---|
| fighter | 2,761 / 2,830 / 3,110 | 222 / 250 / 279 | 47 / 61 / 75 | 1.8:never / 1.8:never / 1.8:never | 0.4 / 0.8 / 0.7 | 32:31 / 18:36 |
| defender | 3,047 / 3,125 / 3,438 | 175 / 203 / 232 | 47 / 61 / 75 | 7.0:never / 5.2:never / 3.5:never | 0.4 / 0.7 / 0.6 | 64:58 / 46:63 |
| ranger | 1,931 / 1,991 / 2,232 | 202 / 239 / 275 | 47 / 61 / 75 | 4.5:never / 3.0:never / 3.0:never | 0.4 / 0.6 / 0.6 | 37:18 / 26:19 |
| archer | 1,908 / 1,965 / 2,195 | 193 / 221 / 249 | 47 / 61 / 75 | 6.8:never / 6.8:never / 2.2:never | 0.6 / 1.1 / 0.7 | 46:17 / 39:17 |
| mage | 1,620 / 1,659 / 1,816 | 216 / 253 / 289 | 47 / 61 / 75 | 4.0:never / 4.5:never / 2.5:never | 0.4 / 0.8 / 0.6 | 44:14 / 36:16 |
| priest | 1,705 / 1,749 / 1,926 | 145 / 166 / 187 | 47 / 61 / 75 | 4.0:never / 4.0:never / 1.5:never | 0.6 / 1.1 / 0.8 | never:22 / never:22 |
| warrior | 2,866 / 2,970 / 3,285 | 178 / 240 / 252 | 47 / 61 / 75 | 1.8:never / 1.8:never / 1.8:never | 0.4 / 0.8 / 0.7 | 42:47 / 24:59 |
| guardian | 3,067 / 3,145 / 3,458 | 177 / 206 / 235 | 47 / 61 / 75 | 7.0:never / 5.2:never / 3.5:never | 0.4 / 0.7 / 0.6 | 65:58 / 46:63 |
| assassin | 1,991 / 2,051 / 2,292 | 216 / 253 / 289 | 47 / 61 / 75 | 4.5:never / 3.0:never / 3.0:never | 0.4 / 0.6 / 0.6 | 34:19 / 24:20 |
| hunter | 1,928 / 1,985 / 2,215 | 194 / 222 / 250 | 47 / 61 / 75 | 6.8:never / 6.8:never / 2.2:never | 0.6 / 1.1 / 0.7 | 48:17 / 42:17 |
| pagan | 1,680 / 1,719 / 1,876 | 217 / 246 / 275 | 47 / 61 / 75 | 4.0:never / 4.0:never / 2.5:never | 0.4 / 0.9 / 0.6 | 48:20 / 38:21 |
| oracle | 1,765 / 1,809 / 1,986 | 188 / 216 / 245 | 47 / 61 / 75 | 4.0:never / 2.0:never / 2.0:never | 0.6 / 0.9 / 0.8 | never:24 / never:24 |

#### Level 30: the modes (normal / hard / ultimate)

| class | HP | attack | skill points | field monster kill:die s | hours a level | normal, hard against the ultimate twelve (median kill:killed s) |
|---|---|---|---|---|---|---|
| fighter | 9,376 / 9,912 / 10,681 | 469 / 506 / 544 | 92 / 121 / 150 | 3.5:never / 0.0:never / 0.0:never | 2.2 / 4.1 / 3.6 | 73:40 / 60:50 |
| defender | 9,663 / 10,138 / 10,897 | 767 / 791 / 816 | 92 / 121 / 150 | 8.8:never / 7.0:never / 7.0:never | 2.3 / 6.3 / 6.1 | 55:116 / 43:142 |
| ranger | 7,095 / 7,466 / 7,991 | 435 / 522 / 587 | 92 / 121 / 150 | 3.0:never / 3.0:never / 3.0:never | 1.4 / 4.2 / 4.2 | 46:25 / 36:26 |
| archer | 7,016 / 7,378 / 7,870 | 271 / 169 / 281 | 92 / 121 / 150 | 2.2:never / 8.0:never / 0.0:never | 1.7 / 5.8 / 3.2 | 62:26 / 148:28 |
| mage | 6,325 / 6,644 / 6,962 | 375 / 419 / 471 | 92 / 121 / 150 | 4.5:never / 4.5:never / 3.0:never | 3.0 / 8.7 / 7.2 | never:17 / never:17 |
| priest | 6,545 / 6,876 / 7,270 | 362 / 422 / 498 | 92 / 121 / 150 | 3.5:never / 3.5:never / 1.5:never | 2.4 / 7.0 / 4.9 | never:18 / never:18 |
| warrior | 9,396 / 9,932 / 10,701 | 471 / 609 / 647 | 92 / 121 / 150 | 3.5:never / 0.0:never / 0.0:never | 2.3 / 4.1 / 3.6 | 75:40 / 62:51 |
| guardian | 9,683 / 10,158 / 10,917 | 769 / 794 / 819 | 92 / 121 / 150 | 8.8:never / 7.0:never / 7.0:never | 2.3 / 6.3 / 6.1 | 57:120 / 45:148 |
| assassin | 7,299 / 7,670 / 8,195 | 497 / 573 / 638 | 92 / 121 / 150 | 3.0:never / 3.0:never / 3.0:never | 1.4 / 4.2 / 4.2 | 40:26 / 33:27 |
| hunter | 7,021 / 7,383 / 7,890 | 167 / 167 / 282 | 92 / 121 / 150 | 10:never / 8.0:never / 0.0:never | 2.2 / 5.8 / 3.2 | 206:26 / 139:28 |
| pagan | 6,529 / 6,848 / 7,166 | 436 / 479 / 531 | 92 / 121 / 150 | 4.5:never / 4.5:never / 3.0:never | 3.0 / 8.7 / 7.2 | never:17 / 176:18 |
| oracle | 6,784 / 7,115 / 7,479 | 204 / 201 / 212 | 92 / 121 / 150 | 3.0:never / 3.5:never / 3.0:never | 2.2 / 6.8 / 6.3 | 203:19 / 169:21 |

#### Level 60: the modes (normal / hard / ultimate)

| class | HP | attack | skill points | field monster kill:die s | hours a level | normal, hard against the ultimate twelve (median kill:killed s) |
|---|---|---|---|---|---|---|
| fighter | 15,352 / 14,843 / 17,540 | 1071 / 1233 / 1338 | 182 / 241 / 300 | 9.0:91 / 3.5:90 / 1.8:112 | - / - / - | 39:6.5 / 18:6.4 |
| defender | 16,149 / 18,228 / 21,083 | 847 / 907 / 901 | 182 / 241 / 300 | 30:194 / 16:285 / 9.0:never | - / - / - | 46:11 / 31:14 |
| ranger | 11,550 / 13,294 / 15,835 | 607 / 711 / 779 | 182 / 241 / 300 | 19:69 / 11:86 / 8.8:105 | - / - / - | 32:6.4 / 11:7.1 |
| archer | 11,291 / 13,059 / 15,494 | 437 / 603 / 629 | 182 / 241 / 300 | 20:64 / 9.0:88 / 6.0:114 | - / - / - | 26:6.4 / 15:7.1 |
| mage | 9,767 / 10,991 / 12,945 | 426 / 458 / 466 | 182 / 241 / 300 | 15:42 / 12:48 / 6.5:57 | - / - / - | 214:4.8 / 28:6.1 |
| priest | 10,238 / 11,874 / 13,952 | 760 / 939 / 1078 | 182 / 241 / 300 | 12:47 / 9.0:55 / 7.5:64 | - / - / - | 34:5.9 / 27:6.9 |
| warrior | 15,272 / 17,611 / 18,090 | 978 / 1254 / 1345 | 182 / 241 / 300 | 9.0:89 / 6.0:118 / 1.8:120 | - / - / - | 39:6.4 / 23:7.4 |
| guardian | 16,169 / 18,808 / 21,663 | 849 / 915 / 910 | 182 / 241 / 300 | 30:193 / 12:284 / 9.0:never | - / - / - | 48:11 / 30:14 |
| assassin | 12,008 / 13,943 / 16,484 | 821 / 979 / 1086 | 182 / 241 / 300 | 16:66 / 11:78 / 8.8:94 | - / - / - | 26:6.4 / 9.4:7.1 |
| hunter | 11,311 / 13,614 / 16,049 | 438 / 616 / 642 | 182 / 241 / 300 | 20:63 / 7.5:89 / 6.0:114 | - / - / - | 26:6.4 / 14:7.1 |
| pagan | 10,225 / 11,669 / 13,623 | 537 / 573 / 589 | 182 / 241 / 300 | 15:42 / 12:49 / 6.5:58 | - / - / - | 144:5.1 / 26:6.1 |
| oracle | 10,696 / 12,492 / 14,570 | 871 / 1056 / 1203 | 182 / 241 / 300 | 10:47 / 7.5:55 / 7.5:64 | - / - / - | 31:6.1 / 22:6.9 |

- **At 60 a normal character is prey**: the ultimate twelve kill it in a median 4.8 to 11 s and it needs 26 to 215 s for them. Hard
  is closer (6.1 to 14 s to be killed, 9.4 to 31 s to kill) but behind every time. The gap comes from the points (295 or 413 against
  531, 182 or 241 skill points against 300), the hard-quest weapons and the ultimate-only skills; the × 1.5 status HP is a small share
  of a geared character's HP (section 3.3; inferred from the parts, not measured one by one).
- **In PvE the modes are all viable at 60**: a normal build kills 1501 in 9 to 30 s and lives 42 to 194 s.
- **Normal levels fast**: to 30 it needs a third of the experience, which the "hours a level" column shows (1.4 to 3.0 hours at 30
  against 3.2 to 7.2 in ultimate); from 50 on, a fifth (part 1 section 5.3).

### 7.3 Why keep easy, normal and hard if everyone plays ultimate (the case, no decision)

**For keeping them**

- **A shorter road to 60**: normal needs 284.7 million experience to 60 against 1,405 million (a fifth from level 50 on). For players
  with little time, or an alt, that is the difference between reaching the cap and not.
- **The ultimate death rule**: the original deletes an ultimate character that dies without a rune. It is built here as the fall (the
  character kept and restorable by staff; corrected, `docs/plans/balance/plan.md` section 2), so the other modes are where
  players who do not want that risk go. The runes that prevent it are mall-only today.
- **Content and fidelity**: 896 equipment rows and 62 skill rows are ultimate only, the best weapons are hard quests; the modes are part
  of the original game, and easy (cap 30) is a gentle start.
- **The original's own gate**: ultimate used to be reached only after a level (the maintainer's answer); kept modes could bring that back as
  a progression rather than a choice at creation.

**Against keeping them as they are**

- **PvP**: the shared maps put every mode together, and the tables of 7.2 show normal and hard characters losing to ultimate ones at
  every level; on a server where most play ultimate the other modes are a trap for anyone who PvPs.
- **No reason to pick them**: the only gains (faster levelling, no deletion if built) do not offset fewer points, less HP and the
  missing items and skills; private servers' players overwhelmingly choose ultimate (the maintainer's answer).
- **Three times the balancing**: every table of this analysis has a normal and a hard column; items and skills carry `grow` gates;
  the experience table has four curves.

**Options** (part 1 section 2 says where each lives):

1. **Ultimate only**: creation offers ultimate alone; the other modes' data stays unused. A server change (`creation.rs`, the creation
   packet) and the client's creation screen (whether it offers the modes is a client question).
2. **Modes as a levelling choice only**: the same points, HP factor, items and skills for all; the modes differ in the experience curve
   (and the death rule). Server constants (`POINTS_PER_LEVEL`, `stats::scaled`) and the `grow` gates in data; the tooltips need a
   client data patch where they show the mode.
3. **Separate PvP**: keep the modes as they are and keep them apart in PvP (maps or matchmaking by mode). A server rule per map.
4. **As the original**, with the ultimate gate by level the maintainer remembers.

## 8. The levers

For each imbalance: what moves it, by how much (modelled where a number is given), where the change lives (part 1 section 2) and what
else it touches. No decision.

### 8.1 Gear HP (sections 1.1, 3.3)

| lever | effect (modelled) | where | also touches |
|---|---|---|---|
| Life Lapis values (240 / 360 / 500) or the armour's lapis rooms (5 to 6 at 56-60) | the bulk of a geared build's HP: halving the lapis HP takes about 3,500 off a level-60 fighter | `defs.items` (lapis rows, items' `slot`) | every PvP time; the lapis tooltips and room display (client data patch) |
| crafted HP/MP/SP per point (100) | 50 instead of 100: about −750 HP at 60, −400 at 30 | server constant (`stats::CRAFT_HEALTH`) | the value of a lucky drop (part 2 section 3.5: up to 1,300 a number) |
| the link rates (`lapis::base_rate`: grade 35 at 12 %) | how many players reach "expected" gear and how fast | server constant | the lapis economy (part 2 section 4) |

### 8.2 The 51-58 wall (sections 1.2, 4.2; `builds_lever_wall.csv`)

#### The 51-58 wall: level-55 builds with plain gear against monster 1501, its defence and attack scaled (kill / die s; ✗ dies first)

| class | defence × 1, attack × 1 | defence × 0.5, attack × 1 | defence × 1, attack × 0.5 | defence × 0.5, attack × 0.5 | defence × 0.25, attack × 0.5 | defence × 0.25, attack × 0.35 | defence × 0.15, attack × 0.35 |
|---|---|---|---|---|---|---|---|
| fighter | 18 / 34 | 15 / 34 | 18 / 79 | 15 / 79 | 12 / 79 | 12 / 132 | 12 / 132 |
| defender | 42 / 43 | 33 / 43 | 42 / 124 | 33 / 124 | 22 / 124 | 22 / 278 | 21 / 278 |
| ranger | 36 / 26 ✗ | 18 / 26 | 36 / 54 | 18 / 54 | 14 / 54 | 14 / 81 | 12 / 81 |
| archer | 54 / 24 ✗ | 52 / 24 ✗ | 54 / 53 ✗ | 52 / 53 | 36 / 53 | 36 / 82 | 32 / 82 |
| mage | 11 / 10 ✗ | 9.0 / 10 | 11 / 22 | 9.0 / 22 | 7.0 / 22 | 7.0 / 33 | 7.0 / 33 |
| priest | 18 / 12 ✗ | 15 / 12 ✗ | 18 / 28 | 15 / 28 | 14 / 28 | 14 / 47 | 14 / 47 |
| warrior | 24 / 36 | 15 / 36 | 24 / 85 | 15 / 85 | 14 / 85 | 14 / 145 | 12 / 145 |
| guardian | 39 / 62 | 24 / 62 | 39 / never | 24 / never | 18 / never | 18 / never | 16 / never |
| assassin | 35 / 24 ✗ | 19 / 24 | 35 / 51 | 19 / 51 | 15 / 51 | 15 / 76 | 14 / 76 |
| hunter | 58 / 29 ✗ | 56 / 29 ✗ | 58 / 64 | 56 / 64 | 38 / 64 | 38 / 99 | 34 / 99 |
| pagan | 11 / 11 ✗ | 9.0 / 11 | 11 / 24 | 9.0 / 24 | 7.0 / 24 | 7.0 / 36 | 7.0 / 36 |
| oracle | 16 / 13 ✗ | 14 / 13 ✗ | 16 / 30 | 14 / 30 | 14 / 30 | 14 / 50 | 12 / 50 |

Halving the monster's attack (third column) lets every class but the archer live through the kill with plain gear; halving its
defence (second column) shortens kills and saves the mage, pagan, ranger and assassin, not the others. The monsters' numbers live in `defs.mobs` and change live through the data
editor (`docs/plans/done/data-editing.md`); they also set the parties' pace, the experience per hour of everyone (expected gear already solos
them) and the ranged classes' kiting, which the model does not show.

### 8.3 Skills (sections 1.4, 1.5; `builds_lever_skills.csv`)

#### Skill levers: median and fastest seconds to kill the other eleven

| class | level | lever | median | fastest |
|---|---|---|---|---|
| mage | 30 | as is | 38 | 28 |
| mage | 30 | damage over time × 0.5 | 39 | 28 |
| pagan | 30 | as is | 36 | 24 |
| pagan | 30 | damage over time × 0.5 | 36 | 26 |
| mage | 60 | as is | 11 | 9.0 |
| mage | 60 | damage over time × 0.5 | 10 | 10 |
| pagan | 60 | as is | 9.5 | 9.0 |
| pagan | 60 | damage over time × 0.5 | 10 | 10 |
| fighter | 60 | as is | 7.5 | 2.5 |
| fighter | 60 | percentage skill × 0.5 | 15 | 3.8 |
| warrior | 60 | as is | 5.2 | 3.5 |
| warrior | 60 | percentage skill × 0.5 | 14 | 3.5 |
| ranger | 60 | as is | 5.0 | 3.0 |
| ranger | 60 | percentage skill × 0.5 | 5.0 | 4.0 |
| assassin | 60 | as is | 4.0 | 3.0 |
| assassin | 60 | percentage skill × 0.5 | 4.0 | 3.0 |
| archer | 60 | as is | 4.8 | 4.8 |
| archer | 60 | percentage skill × 0.5 | 8.5 | 7.2 |
| hunter | 60 | as is | 4.8 | 4.8 |
| hunter | 60 | percentage skill × 0.5 | 8.5 | 6.0 |
| mage | 60 | as is | 11 | 9.0 |
| mage | 60 | percentage skill × 0.5 | 17 | 16 |
| pagan | 60 | as is | 9.5 | 9.0 |
| pagan | 60 | percentage skill × 0.5 | 16 | 16 |

| lever | where | also touches |
|---|---|---|
| the percentage skills 180 to 184: value (87 to 92 % of the maximum), success (70 %), cooldown (8 to 20 min) | `defs.skills` levels 1 and 2 + the client's `Skill.SData` (tooltips) | their PvE use is already nil (part 2 section 5.5) |
| the mage's damage over time | as above | little with real gear (table above) |
| Secure Guard (+500 / +500 for 40 s), the defenders at 15 and 30 | `defs.skills` 75 | the defenders' tanking for parties |
| the priest's and oracle's attack skills, if they should win duels | `defs.skills` | their role; heals not modelled |
| Speedy Attack (human fighter only) and Bloody Karma against Evasion and Battle Cry | `defs.skills` 129, 191, 189, 190 | the faction balance (section 6) |

### 8.4 Formulas and items (sections 1.3, 1.7, 1.8)

| imbalance | lever | where | also touches |
|---|---|---|---|
| physical burst against casters (a mage dies in 2.5 s to a fighter) | the casters' armour defence (`defs.items`, the mage-priest armour family: 46 defence for the best level-60 set, part 2 section 3.4), Body Protector's +47, or the PvP physical formula (attack − defence, `combat::pvp_strike`) | data, or a server build | a formula change needs the client's window to agree (question 18) |
| LUC as the best second stat | the LUC terms (0.19 critical, 0.125 evasion, 0.034 critical cut a point; `combat::player`, `combat::pvp_strike`) | server build | the client's window (question 18) |
| casters faster with daggers | the staves' speed (`defs.items` `attacktime`, 5 → 7), or a fixed interval for magic skills (`combat::attack_interval`) | data or a server build | client question 19 |
| the archer and hunter at 15 and in PvE at 55 | crossbow and javelin attack, the ranged attack formula (STR × 1 + LUC × 0.3), their skill values | data or a server build | their PvP at 60, where they win 8 duels |

### 8.5 Factions and modes

| imbalance | lever | where |
|---|---|---|
| Fury's kill-count grades (HP, MP, SP, ranged and magic defence from grade 3 and 13) | the grade rows | `world.kill_status` (server data); the client has `KillStatus.SData` (question 18 (d)) |
| the Fury quest bracelets (value 132 against 18) and shields (double magic defence) | the item rows | `defs.items` + client data patch |
| race skills | section 8.3 | |
| normal and hard against ultimate | the options of section 7.3 | server constants, data, creation |

## 9. Corrections and notes

### 9.1 The guild halls are reachable (verified; corrects part 2 section 2.4)

`world.map_npcs` places **gatekeeper 44 (Light) on map 1, Erina, and gatekeeper 45 (Fury) on map 2, Reikeuseu** (query: `type = 2 AND
type_id IN (44, 45)`; imported from the original's `1.svmap` and `2.svmap` by `tools/world/import.py`). Their only destination is the
halls (53, 54) for 9,900 (`world.npc_gates`). `server.rs::usable_gate` applies guild rules only inside a guild-house map
(`guild_house::gate_rate`), so in the capitals any player of the gatekeeper's faction with 9,900 goes. The ranking battle (map 50,
from the halls' portals, Sunday 21:00 to 22:00) needs only a guild (`docs/original/guild-ranking.md` section 4). So the halls'
merchants (205, 216, potions) are open to everyone, the ranking battle to every guild, and a new guild can earn battle points, a ranking
and then a house. Part 2's "closed circle" came from a wrong reading of the placement query; part 2's question about the original
placing 44 and 45 is answered: the original's data does.

### 9.2 Model changes in this part (`tools/balance/`)

- `model.py`: build strings take `craft=`, `craftorder=`, `lapis=`, `element=` and per-item `@step` and `#crafted numbers` (README);
  the gear picker values lapis rooms and crafted numbers when those options are given; skills of damage type 5 (a share of the
  target's **current** HP: Terrible Attack, Protection Charge, Trap, Dragon Slayer) are taken of what is left rather than of full HP
  (before, two of them in a row could exceed the target's HP); skills that cost the whole SP or MP pool (Sow and Reap, Mana Decay) are
  used only when no other skill is ready and affordable; a rotation can start from a given SP/MP pool. `formulas.ItemDef` keeps
  `server`, `reqstr`, `reqvg`, `reqig`.
- Part 1's level-60 sample tables (section 7.3) rerun with these changes come out the same.

## 10. Client questions (added to `docs/testing/client-research-queue.md`)

- **18**: does the character window compute its own attack, defence, hit, evasion and critical (and hold the status table and the mode
  factors), and what does it show from `KillStatus.SData`? A formula lever (section 8.4) needs the answer.
- **19**: does the client hold its own wait between skills, and do spells with a short sword go out faster than with a staff (section
  3.4)?
- **20**: what the client shows for the race skills (Evasion's cost) and which level-60 skill rows its window offers.

## 11. What cannot be settled without play

- **What gear players actually reach**, and how fast: "expected" assumes a full set of grade-35 lapis (about 240 links and as many lapis
  at 60) and kept drops with numbers on the right stats.
- **The flat top of the stat split**: which of several nearly equal builds players settle on.
- **Real fights**: kiting (the ranged classes at range, the melee closing in), stuns and sleeps, blocks and reflection, heals and potions,
  the 70 % of the percentage skills as streaks; the model's PvP is two characters standing and trading from full health.
- **Party PvE**: how parties split experience and pace at 55 to 64.
- **The client** (section 10).

## 12. Questions for the maintainer

1. **The 51-58 fields**: should every class solo them with the gear it finds (plain gear: eight of twelve cannot), or is linking lapis
   the intended way in?
2. **Gear HP**: is a doubling or tripling of HP from lapis and crafted numbers the game you want, or should lapis HP and the crafted
   factor (100 a point) come down?
3. **The percentage skills 180 to 184**: keep, weaken (value, success, cooldown), or take out of PvP?
4. **Casters in PvP**: should a mage or priest last longer than a median 4.8 s against physical classes at 60 (their defence, or the
   physical burst)?
5. **Priests and oracles**: support only, or able to win duels?
6. **The defenders at 15 and 30** (killed in a median 75 and 180 s): intended?
7. **Casters with short swords** casting a third faster than with staves: keep, or tie spell speed to something else?
8. **LUC** as the best second stat at 60: keep?
9. **Factions**: mirror them (kill-count grades, race skills, the Fury bracelets and shields), or keep the differences as flavour?
10. **Enchant ceiling**: weapon +3 and armour +4 are what the builds use; with the original's `ItemEnchant.ini` rates players can reach about +6 / +7 (part 2 section 4.3). Keep that, or change the rates or put safe Lapisia in play?
11. **Origins**: should the safe Lapisia, the Recreation Rune and the Lucky Charm come into play (drops, quests, a currency), or stay
    out?
12. **The guild halls** (section 9.1): they are open to all as the original placed them; anything to change?
13. **The modes** (section 7.3): ultimate only; modes as a levelling choice only; separate PvP; or as the original?
14. **The ultimate death rule** (the original deletes the character): build it, or not? (Correction: it is built, as the fall; `docs/plans/balance/plan.md` section 2.)

## Appendix A: the build strings (ultimate, PvP strings with the opening buffs)

The PvE strings (lasting buffs only, the weapon's element against the monster), the normal and hard builds and the variants are in
`builds.csv`, `builds_pve.csv` and `builds_variants.csv`.

- fighter 15: `fighter ultimate 15 alloc=str:3,dex:1 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,rec element=1,1 skills=1/1,9/1,19/1,30/1,31/1,32/2,39/1,40/1,55/2,56/2,57/2,73/1,112/1,129/1,138/1,139/1 buffs=30/1,31/1,112/1,139/1,188/1,129/1 craftorder=hp,str,dex,sp`
- fighter 30: `fighter ultimate 30 alloc=str:2,rec:2 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,rec element=1,1 skills=1/2,9/3,19/1,24/2,30/2,31/3,32/3,39/2,40/2,55/3,56/3,57/3,58/1,73/2,112/2,118/2,129/2,138/3,139/2,302/1 buffs=30/2,31/3,112/2,139/2,188/2,58/1,129/2 craftorder=hp,str,rec,sp`
- fighter 60: `fighter ultimate 60 alloc=str:3,dex:1 weapon=6 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,rec element=1,1 skills=1/3,8/3,18/3,24/3,30/3,31/3,32/3,38/3,39/3,40/3,55/3,56/2,58/3,73/3,112/3,118/3,129/3,138/3,139/3,151/3,152/3,153/3,156/3,157/3,158/3,180/2,301/3,302/2 buffs=30/3,31/3,112/3,139/3,188/3,58/3,129/3 craftorder=hp,str,dex,sp`
- defender 15: `defender ultimate 15 alloc=str:3,dex:1 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:str,hp,rec,dex,sp element=1,1 skills=2/1,9/1,19/1,30/1,31/1,32/2,41/2,59/2,98/1,99/2,105/1,111/1,130/1,131/1,138/1,139/1,303/1,304/1 buffs=30/1,31/1,59/2,111/1,304/1,139/1,188/1 craftorder=hp,str,dex,sp`
- defender 30: `defender ultimate 30 alloc=str:1,dex:2,rec:1 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:dex,hp,rec,str,sp element=1,1 skills=2/2,9/3,19/1,25/2,30/2,31/3,32/3,41/3,59/3,62/2,75/1,88/1,90/1,98/2,99/2,105/1,111/2,119/2,130/1,131/1,138/3,139/2,303/3,304/1 buffs=30/2,31/3,59/3,111/2,304/1,139/2,188/2,75/1 craftorder=hp,dex,str,rec,sp`
- defender 60: `defender ultimate 60 alloc=dex:1,luc:3 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:luc,hp,rec,str,dex,sp element=1,1 skills=2/3,9/3,19/2,25/1,30/3,31/3,32/3,41/3,59/3,62/3,75/3,89/3,90/3,98/3,99/3,105/3,111/3,130/3,131/3,138/3,139/3,171/3,175/2,181/2,303/3,304/3,306/3,307/3 buffs=30/3,31/3,59/3,111/3,304/3,139/3,188/3,75/3 craftorder=hp,luc,dex,sp`
- ranger 15: `ranger ultimate 15 alloc=str:4 weapon=15 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=1/1,11/1,16/1,21/1,32/2,44/1,63/1,104/2,114/1,120/1,134/2,141/1,145/2,310/1 buffs=44/1,120/1 craftorder=hp,str,sp`
- ranger 30: `ranger ultimate 30 alloc=str:4 weapon=15 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=1/2,11/3,16/2,21/1,26/2,32/3,44/3,63/2,101/1,114/2,120/2,134/3,136/2,141/1,142/1,145/3,310/1 buffs=44/3,120/2,136/2,142/1 craftorder=hp,str,sp`
- ranger 60: `ranger ultimate 60 alloc=str:2,luc:2 weapon=15 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=1/3,11/3,16/3,21/2,26/3,32/3,44/3,63/3,91/3,101/3,114/3,120/3,134/3,136/3,141/3,142/3,145/3,161/3,176/2,182/2,310/2 buffs=44/3,120/3,91/3,136/3,142/3 craftorder=hp,str,luc,sp`
- archer 15: `archer ultimate 15 alloc=str:4 weapon=14 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=2/1,13/1,23/1,32/2,33/1,45/2,46/1,67/1,93/1,108/1,121/1,133/1,141/1,149/1 buffs=133/1,121/1 craftorder=hp,str,sp`
- archer 30: `archer ultimate 30 alloc=luc:4 weapon=13 shield=0 enchant=3 armour=4 craft=exp lapis=35:luc,hp,str,dex,sp element=1,1 skills=2/2,12/3,22/1,27/2,32/3,33/3,45/3,46/2,51/1,67/2,92/1,93/2,101/1,108/2,121/2,133/2,136/2,141/1,142/1,149/2,172/1 buffs=133/2,121/2,136/2,142/1 craftorder=hp,luc,sp`
- archer 60: `archer ultimate 60 alloc=dex:1,luc:3 weapon=14 shield=0 enchant=3 armour=4 craft=exp lapis=35:luc,hp,str,dex,sp element=1,1 skills=2/3,13/3,23/3,27/3,32/3,33/3,46/3,51/3,67/3,92/3,93/3,101/3,108/3,121/3,133/3,136/3,141/3,142/3,149/3,163/3,172/3,177/2,183/2,315/3 buffs=133/3,121/3,136/3,142/3 craftorder=hp,luc,dex,sp`
- mage 15: `mage ultimate 15 alloc=int:4 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,mp,wis,rec element=1,1 skills=6/1,14/1,29/2,32/2,35/2,36/2,47/1,52/1,109/3,117/1,132/1,143/1 buffs=117/1,132/1 craftorder=hp,int,mp`
- mage 30: `mage ultimate 30 alloc=int:2,wis:2 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,mp,wis,rec element=1,1 skills=6/3,14/3,28/2,29/3,32/3,35/3,36/3,47/2,49/1,52/2,70/1,74/2,84/1,109/3,117/1,132/2,146/1 buffs=117/1,132/2 craftorder=hp,int,wis,mp`
- mage 60: `mage ultimate 60 alloc=wis:2,luc:2 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:wis,hp,int,mp,rec element=1,1 skills=6/3,14/3,28/3,29/3,32/3,35/3,47/3,49/3,74/3,82/3,84/3,95/3,96/3,109/3,117/3,132/3,143/3,146/3,164/3,178/2,184/2,321/2,323/2 buffs=117/3,132/3 craftorder=hp,wis,luc,mp`
- priest 15: `priest ultimate 15 alloc=int:2,wis:2 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,wis,mp,rec element=1,1 skills=6/1,14/1,32/2,53/1,54/1,102/2,103/1,116/1,123/1,132/1,137/1,144/1,148/1 buffs=123/1,132/1,188/1 craftorder=hp,int,wis,mp`
- priest 30: `priest ultimate 30 alloc=int:3,wis:1 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,wis,mp,rec element=1,1 skills=6/3,14/3,28/2,29/3,32/3,42/1,53/2,54/3,74/2,85/1,86/1,102/3,103/2,116/2,123/2,124/2,132/2,137/2,144/2,148/2 buffs=123/2,132/2,188/2 craftorder=hp,int,wis,mp`
- priest 60: `priest ultimate 60 alloc=int:4 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,wis,mp,rec element=1,1 skills=6/3,14/3,28/3,29/3,32/3,42/3,53/3,54/3,74/3,85/3,86/3,102/3,103/3,116/3,123/3,124/3,132/3,137/3,144/3,148/3,167/3,168/3,186/3,324/2 buffs=123/3,132/3,188/3 craftorder=hp,int,mp`
- warrior 15: `warrior ultimate 15 alloc=str:1,dex:2,rec:1 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:dex,hp,str,sp,rec element=1,1 skills=1/1,9/1,19/1,30/1,31/1,32/2,39/1,40/1,55/2,56/2,57/2,73/1,112/1,128/1,138/1,139/1 buffs=30/1,31/1,112/1,139/1,190/1,128/1 craftorder=hp,dex,str,rec,sp`
- warrior 30: `warrior ultimate 30 alloc=str:2,rec:2 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,rec element=1,1 skills=1/2,9/3,19/1,24/2,30/2,31/3,32/3,39/2,40/2,55/3,56/3,57/3,58/1,73/2,112/2,118/2,128/2,138/3,139/2,302/1 buffs=30/2,31/3,112/2,139/2,190/2,58/1,128/2 craftorder=hp,str,rec,sp`
- warrior 60: `warrior ultimate 60 alloc=str:2,dex:1,luc:1 weapon=6 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,rec element=1,1 skills=1/3,8/3,18/3,24/1,30/3,31/3,32/3,38/3,39/3,40/3,55/3,57/3,58/3,73/3,112/3,118/3,128/3,138/3,139/3,151/3,152/3,153/3,156/3,157/3,174/2,180/2,301/3,302/1 buffs=30/3,31/3,112/3,139/3,190/3,58/3,128/3 craftorder=hp,str,dex,luc,sp`
- guardian 15: `guardian ultimate 15 alloc=str:3,dex:1 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:str,hp,rec,dex,sp element=1,1 skills=2/1,9/1,19/1,30/1,31/1,32/2,41/2,59/2,98/1,99/2,105/1,111/1,130/1,131/1,138/1,139/1,303/1,304/1 buffs=30/1,31/1,59/2,111/1,304/1,139/1,190/1 craftorder=hp,str,dex,sp`
- guardian 30: `guardian ultimate 30 alloc=str:1,dex:2,rec:1 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:dex,hp,rec,str,sp element=1,1 skills=2/2,9/3,19/1,25/2,30/2,31/3,32/3,41/3,59/3,62/2,75/1,88/1,90/1,98/2,99/2,105/1,111/2,119/2,130/1,131/1,138/3,139/2,303/3,304/1 buffs=30/2,31/3,59/3,111/2,304/1,139/2,190/2,75/1 craftorder=hp,dex,str,rec,sp`
- guardian 60: `guardian ultimate 60 alloc=dex:1,luc:3 weapon=7 shield=1 enchant=3 armour=4 craft=exp lapis=35:luc,hp,rec,str,dex,sp element=1,1 skills=2/3,9/3,19/2,25/1,30/3,31/3,32/3,41/3,59/3,62/3,75/3,89/3,90/3,98/3,99/3,105/3,111/3,130/3,131/3,138/3,139/3,171/3,175/2,181/2,303/3,304/3,306/3,307/3 buffs=30/3,31/3,59/3,111/3,304/3,139/3,190/3,75/3 craftorder=hp,luc,dex,sp`
- assassin 15: `assassin ultimate 15 alloc=str:4 weapon=15 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=1/1,11/1,16/1,21/1,32/2,44/1,63/1,104/2,114/1,120/1,134/2,141/1,145/2,310/1 buffs=44/1,120/1,191/1 craftorder=hp,str,sp`
- assassin 30: `assassin ultimate 30 alloc=str:4 weapon=15 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=1/2,11/3,16/2,21/1,26/2,32/3,44/3,63/2,110/1,114/2,120/2,134/3,136/2,141/1,142/1,145/3,310/1 buffs=44/3,120/2,136/2,142/1,191/2 craftorder=hp,str,sp`
- assassin 60: `assassin ultimate 60 alloc=str:3,luc:1 weapon=9 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=1/3,11/3,16/3,21/2,26/3,32/3,44/3,63/3,91/3,110/3,114/3,120/3,134/3,136/3,141/3,142/3,145/3,161/3,176/2,182/2,310/2 buffs=44/3,120/3,91/3,136/3,142/3,191/3 craftorder=hp,str,luc,sp`
- hunter 15: `hunter ultimate 15 alloc=str:4 weapon=11 shield=0 enchant=3 armour=4 craft=exp lapis=35:str,hp,dex,sp,luc element=1,1 skills=2/1,13/1,23/1,32/2,33/1,45/2,46/1,67/1,93/1,108/1,121/1,133/1,141/1,149/1 buffs=133/1,121/1,190/1 craftorder=hp,str,sp`
- hunter 30: `hunter ultimate 30 alloc=luc:4 weapon=13 shield=0 enchant=3 armour=4 craft=exp lapis=35:luc,hp,str,dex,sp element=1,1 skills=2/2,12/3,22/1,27/2,32/3,33/3,45/3,46/2,51/1,67/2,92/1,93/2,108/2,110/1,121/2,133/2,136/2,141/1,142/1,149/2,172/1 buffs=133/2,121/2,190/2,136/2,142/1 craftorder=hp,luc,sp`
- hunter 60: `hunter ultimate 60 alloc=dex:1,luc:3 weapon=11 shield=0 enchant=3 armour=4 craft=exp lapis=35:luc,hp,str,dex,sp element=1,1 skills=2/3,13/3,23/3,27/3,32/3,33/3,46/3,51/3,67/3,92/3,93/3,108/3,110/3,121/3,133/3,136/3,141/3,142/3,149/3,163/3,172/3,177/2,183/2,315/3 buffs=133/3,121/3,190/3,136/3,142/3 craftorder=hp,luc,dex,sp`
- pagan 15: `pagan ultimate 15 alloc=int:3,wis:1 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,mp,wis,rec element=1,1 skills=6/1,14/1,29/2,32/2,35/2,36/2,47/1,52/1,109/3,117/1,132/1,143/1 buffs=117/1,132/1,191/1 craftorder=hp,int,wis,mp`
- pagan 30: `pagan ultimate 30 alloc=int:2,wis:2 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,mp,wis,rec element=1,1 skills=6/3,14/3,28/2,29/3,32/3,35/3,36/3,47/2,49/1,52/2,70/1,74/2,84/1,109/3,117/1,132/2,146/1 buffs=117/1,132/2,191/2 craftorder=hp,int,wis,mp`
- pagan 60: `pagan ultimate 60 alloc=wis:2,luc:2 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:wis,hp,int,mp,rec element=1,1 skills=6/3,14/3,28/3,29/3,32/3,35/3,47/3,49/3,74/3,82/3,84/3,95/3,96/3,109/3,117/3,132/3,143/3,146/3,164/3,178/2,184/2,321/2,323/2 buffs=117/3,132/3,191/3 craftorder=hp,wis,luc,mp`
- oracle 15: `oracle ultimate 15 alloc=int:3,wis:1 weapon=12 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,wis,mp,rec element=1,1 skills=14/1,32/2,37/2,53/1,54/1,102/2,103/1,116/1,123/1,132/1,137/1,144/1,148/1 buffs=123/1,132/1,191/1 craftorder=hp,int,wis,mp`
- oracle 30: `oracle ultimate 30 alloc=luc:4 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:luc,hp,wis,int,mp,rec element=1,1 skills=6/3,14/3,28/2,32/3,42/1,53/2,54/3,74/2,85/1,86/1,102/3,103/2,116/2,123/2,124/2,132/2,137/2,144/2,148/2 buffs=123/2,132/2,191/2 craftorder=hp,luc,mp`
- oracle 60: `oracle ultimate 60 alloc=int:4 weapon=10 shield=0 enchant=3 armour=4 craft=exp lapis=35:int,hp,wis,mp,rec element=1,1 skills=6/3,14/3,28/3,29/3,32/3,42/3,53/3,54/3,74/3,85/3,86/3,102/3,103/3,116/3,123/3,124/3,132/3,137/3,144/3,148/3,167/3,168/3,186/3,324/2 buffs=123/3,132/3,191/3 craftorder=hp,int,mp`

## Next

The analysis is done (parts 1 to 3). The maintainer answered section 12 on 2026-10-27; the plan is `docs/plans/balance/plan.md`.
