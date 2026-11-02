# Balance (spec and build plan)

What changes in the game's numbers after the three-part balance analysis (`docs/plans/balance/1-formulas.md`,
`-2-content.md`, `-3-classes.md`), from the maintainer's answers to part 3's section 12 (2026-10-27). For each answer: the concrete
changes (data rows, server constants, formulas), where each lives (part 1 section 2), the client data or binary patches it needs
(`docs/testing/client-research-queue.md` entries 18 to 21), the order of work in build-sized steps, and how each step is checked with
`tools/balance/` (which table should move, and how far).

**Status words** for each change: **decided** (the maintainer's answer settles it; the build may go ahead), **proposed** (the maintainer's answer
was "discuss" or a question back; this document recommends one way and the step waits for the maintainer's OK, written into section 1 when
given), **open** (needs the maintainer's answer first; no step yet). Numbers in this document are part 3's (modelled, expected gear,
ultimate, every skill) unless a section says otherwise; "verified" as in parts 1 to 3.

Nothing here is built. The game, the database and the client are unchanged by this document.

**Parked (the maintainer, 2026-10-31):** the open-source release keeps the original's numbers (`docs/plans/release.md`). This plan
was to run possibly after the player alpha, and then as extension work rather than changes to the core. There is no alpha (the
maintainer, 2026-11-02): it stays parked, as extension work; read "the alpha" below as "a test server".

## 1. The answers and what follows

| # | question (part 3 section 12) | the maintainer's answer (2026-10-27) | what follows | status | step |
|---|---|---|---|---|---|
| 1 | the 51-58 fields: every class solo with found gear, or lapis the way in? | "Linking lapis would be their best bet." | no monster change; a guard that expected gear (linked grade-35 lapis) keeps soloing the level-55 field monster | decided | guard in every step (T1) |
| 2 | gear HP: keep the doubling/tripling, or bring lapis HP and the crafted factor down? | "Probably come down, or equalize survivability." | crafted HP/MP/SP 100 → 50 a point; Life Lapis left as it is (answer 1 makes lapis the way into 51-58); the classes' survivability evened by the steps for answers 4 and 6 | proposed | S9 |
| 3 | the percentage skills 180 to 184 | "Discuss options." | halve the value of the level-2 rows of 180, 182 and 183 (87/92/88 % of max HP → 44/46/44 %); keep 184 (the casters') at 87 % | proposed | S3 |
| 4 | casters in PvP (killed in a median 4.8 s at 60) | "Last slightly longer, but their damage should already be more / not as easily mitigated." | target: killed in a median 6.5 to 9 s, and the mage's and pagan's kill times no slower than the physical classes' median; mostly delivered by S3, the rest measured in S6 | decided (target), proposed (levers) | S3, S6 |
| 5 | priests and oracles: support only, or duel winners? | "Mostly support, but possible wins based on build." | target: a damage-built priest or oracle wins 2 to 4 of 11 duels at 60 and at least 1 at 30; Etain Bless (185) castable at a fixed cost | decided (target), proposed (levers) | S5 |
| 6 | the defenders at 15 and 30 (killed in a median 75 and 180 s) | "Can they kill equally? Probably needs adjusted." | they do not kill faster than others (section 3.5), they just do not die: target their time to be killed at 30 to at most twice the all-class median; a damage floor in the PvP physical formula, then Secure Guard's defence if needed | decided (adjust), proposed (lever) | S4 |
| 7 | casters cast a third faster with short swords | "Not sure, discuss? May need client info." | staves to attack speed 7 (as short swords), after client entry 19 | proposed, waits on entry 19 | S8 |
| 8 | LUC the best second stat at 60: keep? | "Possibly? Is there a way to make the skills a bit more viable?" | keep LUC for now; what "the skills" means is asked back (section 3.10) | open | none yet |
| 9 | factions: mirror, or keep the differences as flavour? | "Not sure, discuss? What do you mean kill-count grades for Fury?" | mirror the numbers (Light gets Fury's kill-count grade rows, the Light quest bracelets and shields Fury's values), keep the shapes (sword against axe spread), make the elf's Evasion castable | proposed | S7 |
| 10 | the enchant ceiling | "Probably will add safe Lapisia into the drops somewhere at rarer rates." | the safe Lapisia drop where the dangerous ones do, at half their rate; with today's enchant rates that moves the reachable ceiling about one step (section 3.2) | decided (drops), open (whether the rates change too) | S2 |
| 11 | the safe Lapisia, Recreation Rune, Lucky Charm in play? | "Drops most likely, maybe quests." | all three drop (quests later: quest rewards are client data, `NpcQuest.SData`) | decided | S2 |
| 12 | the guild halls | "Open to everyone? What do you mean? Is it not just open to the guild members when they have it purchased and they're ranked?" | the house is as the maintainer says; the halls in front of it are open to all (section 3.11); no change proposed | proposed (no change) | none |
| 13 | the modes | "Not sure, what do you think would be best here? One option is we could make them each level cap, but that takes leveling out entirely." | ultimate only at creation for the alpha; the other modes' data left in place | proposed, waits on entry 21 | S10 |
| 14 | the ultimate death rule | "Deletes character without a rune/continuous rune." | **already built** (section 2): the character falls (kept, restorable) without a rune; the runes have no in-play origin, so they get drops | decided | S2 |

## 2. Corrections found while writing this plan

1. **The ultimate death rule is built** (verified, `server.rs::fall` and `leave_dead`, `backend.rs::fall`, migration 0027;
   `docs/original/item-flags.md` section 10 deviation 2, decided by the maintainer 2026-10-20). An ultimate-mode death without a Continuous
   Res Rune running and without a resurrection item in the bags makes the character **fall**: the experience drop, `deleted_at` and
   `fallen_at` set (out of the list as a deletion, kept so staff can restore it), out of its guild, back to the character screen.
   Player kills, duels and guild-war deaths never make it fall (the original's rule). Part 3's section 7.1 ("not built here") and
   question 14 were wrong; corrected in part 3. The answer to question 14 is what is built.
2. **The runes that save an ultimate character are mall-only** (verified, `data/balance/origins.csv`): Resurrection Rune 100/44 (grade
   541), Small Resurrection Rune 100/81 (881), 3 Hr Continuous Res Rune 100/129: mall products; 100/130 and 100/163 to 100/165: no
   origin. Without mall points a player cannot protect an ultimate character today. S2 gives 100/44 and 100/81 drops.
3. **Secure Guard** (75, verified `defs.skills`): melee attack +500 at every level, defence **+200 / +350 / +500** (levels 1 to 3, from
   levels 23, 32, 43), 30 / 35 / 40 s, cooldown 300 s. Part 3's "+500 and +500 for 40 s" is level 3; a level-30 defender has level 1
   (+200 defence for 30 s), which is what part 3's tables used.

## 3. The changes, answer by answer

### 3.1 The 51-58 wall (answer 1; decided: no change)

Nothing changes in `defs.mobs`. Answer 1 makes linked lapis the intended way in, and part 3 section 4.2 shows expected gear
(grade-35 lapis in every room, crafted numbers, weapon +3, armour +4) soloing monster 1501 for every class, the closest the archer
(killed in 94 s, kills in 60). What follows is a **guard (T1)**: no later step may take that away. S9 (crafted factor) is the step
most likely to touch it.

### 3.2 Supply: Lapisia, runes, charm (answers 10, 11, 14; decided)

Every item below already exists in `defs.items` and in the client's `Item.SData` (they are mall or GM items), so **no client patch**:
only drop data, which the client never sees. All verified by query unless marked.

| item | grade now | goes to | drops from (starting rates, tunable live in the data editor) |
|---|---|---|---|
| 95/2 Power Lapisia of Syeeric, 95/7 Defense Lapisia of Syeeric (safe) | 999 (shared by 5,180 rows nothing drops) | a new grade holding these two (one of the two at random, as grade 394 holds the two dangerous ones) | **every monster entry that names grade 394** (the dangerous Lapisia), at **half** its rate: floor bosses 2464 to 2467 1 in 20, 2462 and 2463 1 in 80, 2460 and 2461 1 in 400 |
| 100/171 Recreation Rune | 619 (its own) | stays | the same floor bosses 1 in 50; every placed field monster of levels 51 to 60 1 in 5,000 |
| 100/139 Lucky Charm | 999 | a new grade of its own | the floor bosses 1 in 50; field monsters of levels 51 to 60 1 in 10,000 |
| 100/44 Resurrection Rune, 100/81 Small Resurrection Rune | 541, 881 (their own) | stay | every placed field monster of level 30 and up 1 in 1,000 each |

The **free grades** are found by query (1 to 1024, used by no item, no `defs.mob_items` entry, no `world.item_create` box set and no
sequence group). The rates go into a free entry of each monster: entries 1 to 6 hold whole percents (`droprate` 0 to 100), 7 to 9 per
million, so 1 in 20 is entry ≤ 6 at 5 %, the rest entries 7 to 9 (1 in 80 = 12,500; 1 in 400 = 2,500; 1 in 1,000 = 1,000; 1 in 5,000 =
200; 1 in 10,000 = 100). A monster with no free entry is reported, not forced. Each item's `drop` limit column is checked first (an item
with a limit never drops, `DropTables::new`); if one has a limit, the build stops and reports it.

**What it does to the ceiling** (modelled arithmetic from part 2 section 4.3, safe tries = 1 / chance of each step; a safe failure
keeps the step): a player reaches weapon +3 and armour +4 with dangerous Lapisia as today, then safe Lapisia for the next steps. Weapon
+3 → +4 needs 11 safe tries, → +5 another 33; armour +4 → +5 needs 10 a piece, → +6 another 33 a piece. At 1 in 20 from a floor boss,
and half of those of the weapon kind, **weapon +4 is about 440 floor-boss kills, +5 about 1,800; armour +5 on five pieces about
2,000**. So the drops move the reachable ceiling by about **one step (weapon +4, armour +5)** for a regular endgame party over the
alpha. A higher ceiling needs the enchant rates themselves (`lapis::ENCHANT_RATES`, steps 4 to 6: 9 %, 3 %, 1 % on weapons; 30 %, 10 %,
3 % on armour) or richer drops. The maintainer's question 10 answer settles the drops, not the rates (left **open**).

Where: `tools/defs/additions.sql`, a new block "Balance (docs/plans/balance/plan.md)": `UPDATE defs.items SET grade` for 95002, 95007 and
100139, `UPDATE defs.mob_items SET grade, droprate` for the entries chosen, each statement commented with its rule. The data editor
(`docs/plans/done/data-editing.md`, targets `mob_drop`, `item_grade`) changes the same rows live during the alpha; the build reads section 3
of that plan for the order in which `additions.sh` and the editor's stored edits are applied after a reload, and writes it into the
block's comment, so a live tweak is not silently undone (or is, knowingly).

**The guild-house smith's link bonus and the hammers** stay as they are (mall).

### 3.3 The percentage skills (answer 3; proposed)

The options (part 3 sections 5.3, 8.3; part 2 section 5.5):

| option | what | effect (modelled, part 3's lever table) | client |
|---|---|---|---|
| keep | as is | a 70 % roll for 87 to 92 % of the target's maximum HP every 8 to 20 minutes decides most duels of the four classes | none |
| **halve the value** | level-2 rows: 180 Regina Power 87 → 44, 182 Syeeric Touch 92 → 46, 183 Pantera Vision 88 → 44 (% of max HP, `damagehp`, damage type 4) | the fighter's median kill 7.5 → 15 s, the warrior's 5.2 → 14, the archer's and hunter's 4.8 → 8.5; the ranger's and assassin's 4 to 5 s unchanged (their other skills) | `Skill.SData` tooltip (patch 110) |
| lower the success | 70 → 35 % | the same mean, more variance: a duel is a coin flip | tooltip |
| PvP-only cut | server code: damage types 3 and 4 against players × 0.5 | the same as halving, the tooltip then misleads | none (but the tooltip lies) |
| out of PvP | refuse on players | the classes lose their level-60 skill | none |

**Recommended: halve 180, 182 and 183; keep 184 Kamain Breath (the mage's and pagan's) at 87 %.** It answers answer 4 at the same
time: the physical burst that kills a caster in 2.5 s is mostly these skills (part 3: without 180 to 184 the casters are killed in a
median 7.2 to 7.8 s instead of 4.8), while 184 stays the casters' burst that defence does not cut ("not as easily mitigated"). The
level-1 rows (43 to 46 % of the *current* HP, type 5) stay. The cooldowns and the 70 % stay. Their PvE use is nil already (resisted by
491 to 1,424 of 1,436 placed monsters, part 2 section 5.5) and stays so.

Where: `defs.skills` `damagehp` of (180, 2), (182, 2), (183, 2) in `additions.sql`; the client rows in `tools/sdata/client_rows.sql`
as "listed choices" (`docs/plans/done/client-patches.md` section 2: the client record keeps its own name, text and icon and takes the
server's numbers). The tooltip text of these skills may state the percentage in words: the build reads the client text
(`client.skills`) and, if the number is in the text, gives the row a corrected description.

### 3.4 Casters (answer 4; target decided, levers proposed)

Target (T2, T3): at 60 the mage, pagan, priest and oracle are killed in a median **6.5 to 9 s** (now 4.8), and the mage's and pagan's
median kill time is **no slower than the median of the eight physical classes' median kill times** (now 11 and 9.5 s against 5.1; after
S3, about 11 against about 11).

Levers, in order, measured after S3 and S5 (step S6):
1. S3 alone (above) is expected to bring the casters to about 6 to 7 s.
2. If T2 is still short: Body Protector (the casters' lasting buff, defence +47 today) also gives maximum HP (a second ability,
   ability 1, as Bloody Karma's), swept in steps of 500 HP. Defence is a weak lever against 1,300-attack builds (attack minus defence:
   +50 defence is under 5 % of a blow), HP is not.
3. If T3 fails (casters too slow): the mage's and pagan's attack skills' values (`damagehp`), the same factor on every one, in steps of
   10 %.

Where: `defs.skills` (Body Protector's rows, the attack skills) in `additions.sql`, client rows (patch 110).

### 3.5 The defenders at 15 and 30 (answer 6; adjust decided, lever proposed)

**"Can they kill equally?"** (modelled, part 3 section 5.2): no. At 15 the defender and guardian kill in a median 30 s against 14 for
the fighter, 18 for the assassin, 20 for the ranger; at 30 in 33 s, about the median of all classes (22 to 57). They win 9 or 10 of
11 because they are killed in a median 75 s at 15 and **180 s at 30** (the fighter and warrior never kill them within 300 s). At 60
they are killed in 17 to 18 s, about twice the others, which is a tank's place. The fault is at 30: PvP physical damage is attack
minus defence (part 1 section 4.5), and a level-30 defender's defence (327, 527 with Secure Guard) is close to the attackers' attack
(394 to 647), so blows land for 1 to 10.

Target (T4): at 30, the defender's and guardian's median time to be killed **at most twice the all-class median** (now 180 against
30, so at most 60 s), and no "never" in the 30 table; at 15, at most 2.5 times (now 75 against 22: at most 55 s). At 60 (T4b) they
stay the hardest to kill: at least 1.5 times the all-class median (now 17 to 18 against 9).

Levers, in order:
1. **A damage floor in the PvP physical formula** (server code): in `combat::pvp_strike` and the physical branch of
   `combat::pvp_skill_damage` (where the skill's bonus is added to the attack first), `d = max(attack − defence, attack × F)`
   before the spread, element, luck and critical. PvP only; the
   monster formulas are untouched. The client shows damage as the server sends it, so **no client patch** (the window's attack and
   defence numbers do not change). The build sweeps F in 0.2, 0.3, 0.4, 0.5 and takes the smallest that meets T4 without breaking T4b.
   It also helps against any high-defence target (the Fury shields, Absorption Lapis, the debuff-free guardian at 60).
2. If no F in that range meets T4 at 30: **Secure Guard level 1 and 2's defence** (+200, +350) down, in steps of 50 (`defs.skills`
   (75, 1), (75, 2) `abilityvalue2`; patch 110). Level 3 (+500, the level-60 one) unchanged.
3. At 15 (no Secure Guard before 23): if T4 at 15 fails after 1, the defenders' Hard Skin defence (+13) or the shield-mastery passive,
   reported with numbers for the maintainer to choose; not changed by the build alone.

### 3.6 Priests and oracles (answer 5; target decided, levers proposed)

Target (T5): with the build the search picks (at 60 all INT), the priest and oracle win **2 to 4 of 11** duels at 60 (now 0 and 1)
and **at least 1** at 30 (now 0 and 3). Their heals are not modelled (part 3 section 4.4), so the model shows them weaker than they
are in play; the target is kept modest for that reason.

Levers, in order:
1. **Etain Bless** (185, the priest's level-60 skill: MP +1,218 / +2,437, attack speed +1, critical +70 at level 2, 10 minutes) costs
   the whole SP and MP pool (`sp` 1, `mp` 1, verified) and is never worth casting before a fight. Give it a fixed cost (starting
   value: 600 MP, `sp` 0) and add it to the priest's opening buffs in `builds.py` (`OPENING[5]`). Swept: the cost, and the critical
   (70, 50, 35) if it overshoots T5's 4.
2. If T5 at 30 fails: the priest's and oracle's attack skills of levels 1 to 30 (`damagehp`), one factor, steps of 10 %.

Where: `defs.skills` (185, 1), (185, 2) and the attack skills, `additions.sql`; client rows (patch 110). Whether the client shows or
checks the whole-pool cost itself is entry 20 (a) (asked for Evasion; it covers 185 too, the build adds the id to the entry).

### 3.7 Factions (answer 9; proposed)

**What the kill-count grades are** (verified, part 1 section 6.3; `world.kill_status`, `stats::faction_grade`): each faction has one
**shared counter of its players' kills of the other faction**. At 150 to 12,288 kills the faction reaches grades 1 to 30, and every
player of that faction gets the grade's bonuses; the top grade lasts 10 minutes, then the faction drops to 0 and the other faction's
count resets. The bonuses are not the same for both: from grade 3 **Fury's players get +6 % maximum HP, MP and SP (+9 % from grade
13)** and +7 % ranged and magic defence from 14, **Light's get no HP at all** and +2 % defence. So the faction that kills more gets
stronger, and Fury gets stronger faster. The model shows it: at grade 13 on both sides, a fighter needs 6.2 s to kill a warrior
instead of 3.8.

Recommended: **mirror the numbers, keep the shapes.**

| difference | change | where | client |
|---|---|---|---|
| kill-count grades | Light's grade rows take Fury's values (nobody loses) | `world.kill_status` (server data, from `KillStatus.SData`) | the client has `KillStatus.SData`; whether it shows or adds the bonuses itself is entry 18 (d); the patch tool does not generate that file yet |
| the Fury quest bracelets Dignity and Fineness Loop (value 132) against Light's Arciel, Sinseal and Heliotrope Loop (18) | the Light bracelets take the Fury ones' numbers (all stats +2, HP 300, SP 200), matched by level | `defs.items` | `Item.SData`, patch 110 |
| shields: Fury's have double the magic defence (149 pairs) | each Light shield takes its Fury pair's magic defence (`item_pairs.csv`) | `defs.items` | patch 110 |
| the elf's Evasion (189) costs every SP and MP point and is never cast | a fixed cost (starting value: Battle Cry's 11 / 37 / 98 SP and MP) | `defs.skills` | patch 110; entry 20 (a) |
| swords against axes (same mean, Light steadier), job flags, gloves' WIS against INT | kept as flavour | | |
| Bloody Karma, Berserker, Battle Cry, Speedy Attack, Power up Pause | kept (part 3 section 6.1: the race skills move a pair by up to 30 % at 15 but even out at 60) | | |

### 3.8 Casters with short swords (answer 7; proposed, waits on entry 19)

Today every action waits the weapon's attack interval, spells too (verified, part 1 section 4.1), and a short sword has speed 7 (1,500
ms) against a staff's 5 (2,000 ms), so every caster build takes a short sword for a fifth less item attack.

| option | where | client |
|---|---|---|
| **staves to speed 7** (the casters then take staves: more item attack at the same pace) | `defs.items` `attacktime` of every staff row (real type of staves), data | `Item.SData` (the speed shown in the tooltip), patch 110 |
| spells on a fixed interval, whatever the weapon | `combat::attack_interval` for skills of attack kind 2 (server code) | entry 19 (a), (b): if the client holds its own wait or animation per weapon, a server-only change is not seen as intended |
| keep | | |

**Recommended: staves to 7, once entry 19 answers (a) and (b).** It is a data change, it makes the caster's weapon the caster's again,
and it adds about a fifth of item attack to casters, which answer 4 asks for ("their damage should be more"). The check (T7) is that
the casters' weapon choice in `builds_plans.csv` becomes a staff and T2/T3 still hold.

### 3.9 Gear HP (answer 2; proposed)

"Come down, or equalize survivability." Part 3 sections 3.3 and 8.1: of a level-60 fighter's 17,540 HP, lapis give about 7,800 and
crafted numbers about 1,500; halving the Life Lapis would take about 3,500 off and **would put the 51-58 wall back** for the classes
answer 1 lets in through lapis, and shorten every PvP fight further (they are 4 to 9 s already). So:

- **Crafted HP, MP and SP: 100 → 50 a point** (`stats::CRAFT_HEALTH`, server constant). About −750 HP at 60, −400 at 30 (part 3
  section 3.3's "crafted at 50" column). It matters more once the Recreation Rune drops (S2): rerolled numbers are the larger of the
  smallest-of-two, and a single number is up to 1,300 HP today.
- **Life Lapis unchanged.**
- **"Equalize survivability"** read as: the classes' times to be killed closer together. That is T2 (casters up), T4 (defenders at 30
  down) and T9 (the spread of duels won); no separate gear change.

Client: the item tooltip shows crafted HP; whether the client multiplies the crafted number by 100 itself is asked in **entry 18 (e)**
(added). If it does, the tooltip shows double the server's value until a client binary patch; the step waits for the answer.

Where: `crates/game-server/src/stats.rs` `CRAFT_HEALTH`, and its port `tools/balance/formulas.py` `CRAFT_HEALTH`.

### 3.10 LUC (answer 8; open)

LUC pays three ways (critical +0.19 % a point, evasion +0.125, and −0.034 % off the opponent's critical; part 1 section 3.3) and takes
half or more of the free points in seven of the twelve level-60 builds (part 3 section 3.2). The maintainer asks "is there a way to make the
skills a bit more viable?". Two readings, asked back:

- **the other stats** (STR, DEX, REC, INT, WIS) next to LUC: a formula change (more per point, or LUC's critical cut smaller). The
  character window probably computes these numbers itself (entry 18 (a)); until that is answered, a formula change could show wrong
  numbers in the window. No step until the answer.
- **the attack skills that are never worth casting** (part 2 section 5.4: 2 to 11 per class at 60): a data step raising their values
  to the normal blow or above, one class at a time, checked with `content.py`'s `class_skills_60.csv` (`below_normal_*`,
  `dominated_by` columns emptied). Not scheduled until the maintainer says that is what they meant.

Recommended meanwhile: keep LUC as it is.

### 3.11 The guild halls (answer 12; proposed: no change)

The maintainer's picture is right for the **guild house** itself: maps 51 (Light) and 52 (Fury) let in only a guild ranked 1 to 30 that
bought the house (50,000,000) and whose house opened at a ranking (`guild_house`, part 2 section 2.4). The **halls** are something
else: maps 53 and 54, the lobby in front of the house, with two potion merchants (205, 216) and the portals to the house and to the
ranking battle (map 50). Gatekeeper 44 in Erina and 45 in Reikeuseu take anyone of their faction there for 9,900, no guild needed, as
the original's own map files place them (part 3 section 9.1). From the halls, a guild member enters the ranking battle on Sunday 21:00
to 22:00 (any guild, no rank needed), and only a ranked guild with a house goes on into the house. That is how a new guild can start
ranking at all. Nothing to change.

### 3.12 The modes (answer 13; proposed, waits on entry 21)

The maintainer's idea "make them each level cap" (characters start at the cap) takes levelling out of the game; not recommended.

**Recommended: ultimate only at creation, for the alpha.** The reasons: private-server players pick ultimate anyway (the maintainer's answer
to part 3's goals); the other modes are prey in PvP (part 3 section 7.2: a normal level-60 character is killed by the ultimate twelve in
a median 4.8 to 11 s); one mode is one balance column instead of four; the alpha is wipeable, so the choice can be undone. What it
costs: no mode without the death rule. With the runes dropping (S2), the fall being restorable by staff (section 2), and player kills
never causing a fall, that is a fair price on an alpha. The other modes' data (`grow` gates, `defs.exp_defs` curves) stays in place.

If levelling turns out too slow in ultimate (part 3: 40 to 60 hours a level at 55 for most classes), the lever is the ultimate curve
in `defs.exp_defs` (server data; the client draws the bar from the totals the server sends), not a second mode.

Where: `creation.rs` (a minimum mode, `GAME_MIN_GROW`, default 0 = as the original; 3 on the alpha). Whether to refuse a lower mode
or create the character as ultimate anyway depends on what the creation screen does: **entry 21** (added) asks what the screen offers
for each `max_grow` the server sends (`TP_CHAR_COUNTRY`, `g::char_country`) and what it shows when the created character comes
back in another mode.

## 4. The targets (what each step's check measures)

At 60 unless named; expected gear; ultimate; "median" = the median over the other eleven classes of part 3's PvP tables
(`builds_pvp.csv`, variant "every skill"); "all-class median" = the median of the twelve classes' medians.

| id | what | now (part 3) | target | table |
|---|---|---|---|---|
| T1 | level-55 expected gear against 1501: every class's time to die over its time to kill | 1.57 at the least (archer 94 / 60) | at least 1.3 for every class | `builds_pve.csv`, report section `pve` (level 55) |
| T2 | the casters' (mage, pagan, priest, oracle) median time to be killed | 4.8 s each | 6.5 to 9 s | `pvp_summary` |
| T3 | the mage's and pagan's median kill time against the median of the eight physical classes' median kill times | 11 and 9.5 against 5.1 | at most that median | `pvp_summary` |
| T4 | the defender's and guardian's median time to be killed against the all-class median | 15: 75 / 22; 30: 180 / 30, and "never" cells | 15: at most 2.5 times; 30: at most 2 times, no "never" | `pvp_summary`, `pvp` |
| T4b | the same at 60 | 17 to 18 / 9 | at least 1.5 times | `pvp_summary` |
| T5 | duels won by the priest and oracle | 60: 0 and 1; 30: 0 and 3 | 60: 2 to 4 each; 30: at least 1 each | `pvp_summary` |
| T6 | factions at grade 13 on both sides: each pair's two kill times against the "own builds" column | fighter/warrior 6.2 / 3.5 against 3.8 / 3.5 | within 5 % of the "own builds" column at 15, 30 and 60 | `factions` |
| T7 | the casters' weapon choice (after S8) | short sword at 30 and 60 | a staff at 30 and 60 | `builds_plans.csv` |
| T8 | HP from crafted numbers (after S9) | `builds_lever_hp.csv` "crafted at 50" column | the HP column equals today's "crafted at 50" column | report section `levers` |
| T9 | the spread at 60 | wins 0 (priest) to 10 (ranger, assassin) | every class 2 to 9 | `pvp_summary` |

T9 is the overall aim of "equalize survivability" and a rebalance. The ranger and assassin are the likeliest to stay at 10 (halving 182
does not move them, part 3 section 8.3); if they do after S3 to S6, their attack skills are the next proposal (not in this plan's
steps; the build reports the numbers).

## 5. How a step is checked

1. **Baseline** (S1, once): `python3 builds.py --jobs 4` on today's data, the CSVs copied to `data/balance/baseline/` (git-ignored, as
   `data/balance/`).
2. **After a change**: refresh the model's cache (`python3 model.py char "fighter" --refresh` or `data.refresh()`), then two runs:
   - **fixed builds**: the baseline's build strings rerun under the new data and formulas, no new search (`builds.py --fixed
     data/balance/baseline/builds.csv`, built in S1). This isolates what the change does.
   - **full**: `builds.py --jobs 4` with the search, as players would adapt (about eight minutes). The search's top is flat (part 3
     section 2.5), so a single cell can move from a neighbour build; the targets are read from the full run, the fixed run explains them.
3. `python3 compare.py data/balance/baseline data/balance` (built in S1): per level and class, duels won, median kill, median killed,
   before → after; the level-55 PvE kill and die times; the HP table; and each target of section 4 with pass or fail.
4. `python3 -m unittest test_formulas` when the Rust formulas changed (S4, S9): the Rust unit tests the step adds are replayed in the
   port, as for every formula today.
5. The step's write-up goes into this document's "As built" part (below the Build section): the values chosen, the sweep's table, the
   targets' before and after.

## 6. Client side

**Patch 110** (Item.SData, Skill.SData; `docs/plans/done/client-patches.md`): every `defs.items` and `defs.skills` row this plan changes,
generated once the server values have settled (S11), after patch 109 is accepted in game by the client-side session (still pending,
roadmap). Until then the dev and alpha servers run with the new values and the tooltips show the old ones; the live-test checklist
says so for each changed row. Contents (as far as the proposals are OK'd): skills 180/2, 182/2, 183/2 (S3); 185/1, 185/2 (S5); 189/1
to 3 (S7); Body Protector, the casters' and priests' attack skills, 75/1 and 75/2 if their levers are used (S4 to S6); items: the Light
quest bracelets and shields (S7), every staff (S8).

**Not in a data patch**: `KillStatus.SData` (S7; the patch tool does not generate it; entry 18 (d) says whether the client needs it at
all). The PvP damage floor (S4) and drops (S2) need nothing in the client.

**Entries** (`docs/testing/client-research-queue.md`): 18 (a) before any stat formula change (answer 8); 18 (d) before S7's grade rows reach
players; **18 (e)** (added: the crafted HP in the item tooltip) before S9; 19 (a), (b) before S8; 20 (a) (Evasion's and Etain Bless's
whole-pool cost) before S5 and S7 reach players; **21** (added: the creation screen and the mode) before S10.

## 7. Where each change lives

| change | step | where | route to players |
|---|---|---|---|
| safe Lapisia, Lucky Charm grades; drop entries for them, the Recreation Rune and the resurrection runes | S2 | `defs.items.grade`, `defs.mob_items` in `tools/defs/additions.sql` | server data, live by `reload` or the data editor |
| percentage skills 180, 182, 183 level 2 | S3 | `defs.skills` `damagehp` in `additions.sql` | server data + patch 110 |
| PvP physical damage floor | S4 | `combat::pvp_strike`, `combat::pvp_skill_damage` (a constant `PVP_DAMAGE_FLOOR`); `formulas.py` port | server build |
| Secure Guard levels 1, 2 defence (if needed) | S4 | `defs.skills` (75, 1), (75, 2) | server data + patch 110 |
| Etain Bless cost (and critical) | S5 | `defs.skills` (185, 1), (185, 2) | server data + patch 110 |
| Body Protector HP; caster and priest attack skill values (if needed) | S5, S6 | `defs.skills` | server data + patch 110 |
| Light kill-count grades | S7 | `world.kill_status` (via `tools/world` or `additions.sql`, as the build finds the table loaded) | server data (+ `KillStatus.SData` per entry 18 (d)) |
| Light bracelets, Light shields; Evasion's cost | S7 | `defs.items`, `defs.skills` | server data + patch 110 |
| staves' attack speed | S8 | `defs.items.attacktime` | server data + patch 110 |
| crafted HP/MP/SP factor | S9 | `stats::CRAFT_HEALTH`; `formulas.py` | server build (+ a client binary patch if entry 18 (e) says so) |
| ultimate only | S10 | `creation.rs`, config `GAME_MIN_GROW` | server build and config |

## Build

Rebuild and restart the dev game server after each server change (`additions.sh` for data, then the console's `reload` or a restart);
add a live-test checklist item for each change players would notice (`docs/testing/live-test-checklist.md`), marked untested. Steps
marked **proposed** in section 1 wait for the maintainer's OK written into section 1's status column; build the **decided** ones first.

### S1. Tooling (no game change)

1. `builds.py --fixed <builds.csv>`: skip the plans and search stages, take the build strings from the given CSV, run final, pve,
   variants, pvp, factions, levers. `RACE_OPENING` and `OPENING` stay code (S5 and S7 edit them).
2. `compare.py <before dir> <after dir>`: the tables of section 5 step 3 and the targets of section 4 (T1 to T9) with pass/fail;
   `--targets` prints only those.
3. `content.py`'s enchant table: a column "safe tries from +3 (weapon) / +4 (armour)" and, given drop rates, the expected kills of a
   named monster (`--source 2464`), for section 3.2's arithmetic.
4. Run the baseline (section 5 step 1); check that `compare.py baseline baseline` prints today's numbers of section 4's "now" column
   (the full run may differ by a neighbour build in a cell: note any).
5. README of `tools/balance/`: the new options and files.

### S2. Supply (decided: answers 10, 11, 14)

1. Queries first, results in the write-up: free grades; the `drop` limit of 95002, 95007, 100139, 100171, 100044, 100081; the entries
   naming grade 394; the placed field monsters of levels 30+ and 51 to 60 (non-boss, placed: as part 2 section 6) and their free
   entries.
2. `additions.sql`, block "Balance (docs/plans/balance/plan.md)": the grade moves and drop entries of section 3.2, each statement commented.
3. `additions.sh`, reload; `content.py --refresh` (origins, tools, enchant): the six items now have drop origins (`origins.csv`), the
   enchant table shows section 3.2's numbers.
4. Check: `origins.csv` lists the new origins at the stated rates; nothing else in `origins.csv` changed (diff against the baseline's).
   No `builds.py` run needed (expected gear is unchanged by supply).
5. Live-test items: a floor boss kill dropping a safe Lapisia (GM spawn, many kills, or a test with the rate raised in the editor); a
   safe enchant failure keeping the item; a Resurrection Rune saving an ultimate character from the fall.

### S3. Percentage skills (proposed: answer 3)

`additions.sql`: `damagehp` of (180, 2) 44, (182, 2) 46, (183, 2) 44; `client_rows.sql`: the three as listed choices (and a corrected
description if the client text states the number). Check: fixed and full runs; `compare.py`: the fighter's, warrior's, archer's and
hunter's median kill times move as part 3's lever table (to about 15, 14, 8.5, 8.5 s); T2 moves toward 6.5 to 9 s; T3. Report T9.

### S4. Defenders (answer 6)

1. `combat.rs`: `PVP_DAMAGE_FLOOR` (a fraction; 0.0 = as the original) and `d = max(attack − defence, attack × floor)` in
   `pvp_strike` and the physical branch of `pvp_skill_damage` (attack there includes the skill's bonus), before the spread; unit tests (a blow against defence equal to attack lands for the
   floor; a blow against low defence unchanged; monsters untouched). The same in `formulas.py`, the tests replayed in `test_formulas.py`.
2. Sweep F in 0.2, 0.3, 0.4, 0.5 with the model (fixed builds; a `--pvp-floor` option or the constant edited per run), table in the
   write-up: T4 at 15 and 30, T4b, T9. Take the smallest F meeting T4 and T4b; set it in the server; full run.
3. If no F meets T4 at 30: Secure Guard (75, 1) and (75, 2) `abilityvalue2` down in steps of 50 at F = 0.3, until it does; client rows.
   If T4 at 15 still fails, report the numbers for the maintainer (section 3.5 lever 3); do not change more.
4. Server build, tests, restart. Live-test item: a fighter against a defender with Secure Guard at 30 (blows land for the floor).

### S5. Priests (answer 5)

`additions.sql`: (185, 1), (185, 2) `sp` 0, `mp` 600; `builds.py` `OPENING[5]` adds 185. Sweep the cost (300, 600, 1,200) and, if
T5's 4 is passed, the critical (70, 50, 35). If T5 at 30 fails: the priest's and oracle's attack skills of levels up to 30, one factor,
steps of 10 %. Client rows. Check T5, T2, T9. Add 185 to entry 20 (a) of the client queue.

### S6. Casters (answer 4)

Measure T2 and T3 after S3 to S5. Only if T2 fails: Body Protector's rows get maximum HP (ability 1), steps of 500. Only if T3 fails:
the mage's and pagan's attack skills, one factor, steps of 10 %. Client rows. If both hold, write "no change" in the write-up.

### S7. Factions (proposed: answer 9)

1. `world.kill_status`: Light's rows take Fury's values (find how the table is loaded, `tools/world/import.py` or a load script; the
   change survives a reload the same way as `additions.sql`). `stats::faction_grade` unchanged.
2. `defs.items`: the Light quest bracelets take the Fury bracelets' numbers, level for level; each Light shield its Fury pair's magic
   defence (`item_pairs.csv`, kind shield). `defs.skills` (189, 1 to 3): `sp` and `mp` 11, 37, 98. `builds.py` `RACE_OPENING` adds
   189 for the elves.
3. Check T6 (the grade-13 column against "own builds"), the "real builds side by side" HP column (the pairs closer), T9. Client rows
   for the items and 189; `KillStatus.SData` waits on entry 18 (d).

### S8. Staves (proposed: answer 7; after entry 19)

`defs.items.attacktime` 7 for every staff row the casters may wield (the staves' real type; list the rows). Check T7, T2, T3, T9.
Client rows.

### S9. Crafted factor (proposed: answer 2; after entry 18 (e))

`stats.rs` `CRAFT_HEALTH` 50 (and its unit tests), `formulas.py` the same. Check T8 and T1 (the guard). Server build, restart.

### S10. Ultimate only (proposed: answer 13; after entry 21)

`creation.rs` and config: `GAME_MIN_GROW` (default 0), refusing or raising a lower mode as entry 21's answer says; tests; the alpha's
config sets 3 (`docs/server/deploy.md` or B2's config list). No balance run.

### S11. Patch 110 (after patch 109 is accepted)

Generate with `client_patch.py` from every client row S3 to S8 added; its checks pass; hand over per `docs/plans/done/client-patches.md`
section 4; the ledger row.

## Work item: the build

**Goal**: build the plan. **Inputs**: this plan in full (sections 1, 4, 5 and the Build section first), part 3 of the analysis
(`docs/plans/balance/3-classes.md`) and `tools/balance/README.md` for the model.

**Order**: S1 (tooling and the baseline), S2 (supply), S4 and S5 (their targets are decided; their levers are the plan's), then
whichever of S3, S6 to S10 section 1 marks as OK'd by the maintainer; a step still "proposed" is skipped. S8, S9 and S10 also wait on
client research entries 19, 18 (e) and 21 (`docs/testing/client-research-queue.md`); while those are open, they are skipped.

**Each step**: the change; the checks of section 5 (fixed and full `builds.py` runs, `compare.py` against the baseline, the targets T1 to
T9); the write-up in the plan's "As built" part with the sweep tables and the chosen values. Data changes go in
`tools/defs/additions.sql` (a "Balance" block) and `tools/sdata/client_rows.sql`; server changes come with unit tests and their port in
`tools/balance/formulas.py`.

**Done when**: the built steps' targets pass, and the write-up gives the targets before and after, what is skipped and why.
