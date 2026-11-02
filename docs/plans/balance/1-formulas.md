# Balance analysis, part 1: how the numbers work

How a character's numbers come about and how a blow becomes HP lost, for every class and race, against monsters and players, at every
level and mode; and a runnable model of it (`tools/balance/`, README there). **Analysis only**: nothing in the game, the database, the code
or the client was changed, and nothing here is a decision. Part 2 (`docs/plans/balance/2-content.md`) inventories the items, lapis,
skills and monsters; part 3 (`docs/plans/balance/3-classes.md`) compares the classes and lists the levers.

**Status words**: **verified** = read in the server's code (which follows the original binary, `docs/original/*.md`); **modelled** = an
output of `tools/balance/` (expected values, see its README for what it leaves out); **inferred** = a reasonable reading not checked in code
or play. Every formula names its code: `file::function` in `crates/game-server/src/`. Names: `defs.skills`, `defs.mobs` and many
`defs.items` names are Chinese or `???`; this document uses the client's English names (`client.items`, `client.skills`) where the client
has one, and monster ids with their Chinese `defs` name otherwise (no translation is guessed).

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

1. **Attack does not grow with level by itself** (sections 3.3, 4.2): it is the gear's attack plus about 1.3 a point of the class's
   damage stat (STR or INT; ranged 1.0 STR + 0.3 LUC). A level adds 1 hit and 1 evasion, the status table's HP/SP/MP, and changes the
   level-gap factor against monsters; nothing else.
2. **Against monsters, each level of gap is worth about 10 %** of attack lost and of monster defence gained (section 4.2), and the same in
   reverse for monster blows on the player. Fighting monsters below one's level gains nothing more (the factor stays at its 1 % floor).
3. **PvP physical damage is attack minus defence, flat** (no level factor); **PvP magic damage is a percentage cut** by the target's
   magic defence against the caster's INT + item attack (up to 70 %), so defence and magic defence work differently (section 4.5). A
   physical attacker whose attack does not exceed the target's defence does 1 to 10 a blow.
4. **Damage types 3, 4 and 5 ignore every defence, critical and element** (section 4.3). The ultimate-only level-60 skills of four classes
   take **87 % to 92 % of the target's maximum HP** at 70 % (180 Regina Power, 182 Syeeric Touch, 183 Pantera Vision, 184 Kamain Breath,
   cooldowns 8 to 20 minutes); the defender's counterpart is 181 Dayfly Shield (reflects every attack kind for 40 s) and the priest's
   185 Etain Bless (+70 critical, +1 attack speed, +2,437 MP for 10 minutes). Two weapon lapis do 90 % and 100 % of the target's
   maximum HP on any hit, resisted only by magic resistance (section 4.5).
5. **Your own evasion raises your hit chance** and the target's hit lowers it: the hit formula is a contest of both sides' hit and evasion
   (section 4.2). DEX counts twice.
6. **REC does not speed up HP recovery**; recovery is a flat 4 per 5 s standing, 5 % of the maximum + 28 sitting, and nothing while
   fighting (section 3.6). REC is HP (5 a point), defence (1 a point) and aggro.
7. **Enchanting can never pass +12 on weapons or +14 on armour**: the material is required and its count is added to the draw, against a
   rate of 1 in a million from those steps (section 4.9). The +20 values in the tables are unreachable.
8. **The two factions are not mirrored** in three places today: the race skills (one per race, very different in strength), the starting
   stats and HP/MP/SP bonuses by race, and the faction kill-count grades (Fury's give up to +9 % max HP and more defence; Light's
   none of that) (section 6). Everything else in the formulas is the same for both.
9. **Experience**: ultimate needs about **5×** the experience of normal from level 50 on, and the same as hard up to 30 (section 5.3); a
   level-59 ultimate character needs about 15,700 kills of the most common level-59 monster to level.
10. **The model agrees with the server** on all replayed unit tests and on the maxima of 8 of the 9 characters in the dev database (the
    ninth exceeds the 32,767 the `characters.hp` column can hold; section 7.2).

## 2. Where each number lives (the routes for any later change)

| what | where | how a change reaches players |
|---|---|---|
| Formulas: stats → numbers, hit, damage, critical, level factor, recovery, experience rules, points per level, enchant and link rates | server code (`stats.rs`, `combat.rs`, `zone.rs`, `lapis.rs`, …), constants copied from the original | a server build; **the client shows some derived numbers itself** (section 9) |
| Class base HP/SP/MP by level | `defs.status_defs` | server data; the client's own copy is not studied (section 9) |
| Experience per level by mode | `defs.exp_defs` | server data; the client draws the bar from the totals the server sends (`0x0105`, `0x051E`) |
| Items (stats, attack, defence, speeds, levels, classes, prices), lapis, Lapisia | `defs.items` (+ `tools/defs/additions.sql`) | server data **and a client data patch** (`Item.SData`: tooltips, equip levels the client enforces; `docs/plans/done/client-patches.md`) |
| Skills (damage, costs, cooldowns, abilities) | `defs.skills` levels 1 to 3 | server data **and a client patch** (`Skill.SData`: tooltips, cast bars, which skills the window offers) |
| Monster skills, lapis option skills | `defs.skills` level 100 | server data and a client patch (`NpcSkill.SData`, later) |
| Monsters (HP, attacks, defences, experience, drops) | `defs.mobs`, `defs.mob_items` | server data through the data editor (`docs/plans/done/data-editing.md`: live or next spawn); name and level only in the client's `Monster.SData` |
| Starting stats by class and race | server code (`creation.rs`, `CLASS_STATS`) | a server build; the client's creation screen shows its own numbers (not studied) |
| Faction kill-count grades | `world.kill_status` (from `KillStatus.SData`) | server data; the client has the same file (what it shows from it: not studied) |
| Element table, attack intervals, recovery amounts | server code constants (`combat::ELEMENTS`, `ATTACK_INTERVALS_MS`, `zone::REC_*`) | a server build |

## 3. A character's numbers

### 3.1 Maxima (verified, `stats::compute`, `stats::scaled`)

1. **HP, SP, MP** = the status row of the class and level (`defs.status_defs`; outside the table, class 0 level 60), **× 1.1 in hard mode,
   × 1.5 in ultimate** (single-precision factor, truncated); easy and normal × 1.
2. **Race**: human +10 SP, elf +10 MP, vail +5 SP and +5 MP, deatheater +10 HP.
3. **Stats**: **5 HP per REC, 5 SP per DEX, 5 MP per WIS** (base stats, and again for every point items, lapis, crafted stats, passives,
   buffs and the faction grade add).
4. Items add their HP/MP/SP; crafted HP/MP/SP count **100 per point**; lapis add theirs; passives and buffs (abilities 1 to 3) add flat
   amounts; the faction grade adds percentages of the maxima so far (section 6.3).

The status rows (normal mode; hard × 1.1, ultimate × 1.5):

| class (Light / Fury) | HP 1 / 15 / 30 / 60 | SP 1 / 15 / 30 / 60 | MP 1 / 15 / 30 / 60 |
|---|---|---|---|
| fighter / warrior | 205 / 699 / 1511 / 4013 | 125 / 252 / 411 / 804 | 50 / 73 / 121 / 283 |
| defender / guardian | 250 / 783 / 1659 / 4356 | 65 / 107 / 162 / 297 | 65 / 106 / 184 / 447 |
| ranger / assassin | 170 / 602 / 1313 / 3508 | 150 / 363 / 632 / 1290 | 60 / 83 / 133 / 302 |
| archer / hunter | 175 / 574 / 1229 / 3244 | 135 / 312 / 534 / 1078 | 70 / 148 / 294 / 778 |
| mage / pagan | 150 / 393 / 795 / 2039 | 60 / 97 / 142 / 257 | 170 / 471 / 1018 / 2804 |
| priest / oracle | 160 / 443 / 910 / 2350 | 70 / 112 / 168 / 305 | 150 / 414 / 889 / 2445 |

### 3.2 Base stats and points (verified, `creation::starting_stats`, `combat::add_exp`, `item_use::reset_stats`)

Starting STR, DEX, REC, INT, WIS, LUC by class, then by race: **human LUC + 4, elf DEX + 4, vail INT + 2 and WIS + 2, deatheater STR + 2
and REC + 2**.

| class | STR | DEX | REC | INT | WIS | LUC | **+1 a level to** |
|---|---|---|---|---|---|---|---|
| fighter / warrior | 14 | 12 | 9 | 8 | 7 | 11 | STR |
| defender / guardian | 10 | 9 | 12 | 10 | 10 | 10 | REC |
| ranger / assassin | 10 | 15 | 9 | 7 | 8 | 12 | DEX |
| archer / hunter | 11 | 10 | 10 | 7 | 10 | 13 | LUC |
| mage / pagan | 7 | 9 | 9 | 15 | 12 | 9 | INT |
| priest / oracle | 8 | 9 | 10 | 12 | 14 | 8 | WIS |

Each level adds **1 to the class stat** (59 at level 60) and **free stat points: 5 (easy, normal), 7 (hard), 9 (ultimate)**, and **skill
points 3, 3, 4, 5**. At level 60: 295 / 413 / 531 free stat points, 182 / 241 / 300 skill points (a skill reset gives level × points + 5).
Which races play which class: human fighter, defender, priest; elf ranger, archer, mage; deatheater warrior, guardian, hunter; vail
assassin, pagan, oracle (`creation::RACE_CLASSES`). So each Light class has one race and each Fury class one race: race and class go
together.

Note (verified, `combat::class_stat`): the class stat point goes to the stat the *original designers* gave the class, which is not
always the stat its damage reads: the archer gets LUC (ranged attack reads STR × 1, LUC × 0.3), the priest WIS (no damage formula
reads WIS beyond × 0.2 magic attack), the defender REC.

### 3.3 What each stat does (verified, `combat::player`, `stats::compute`, `combat::formula`, `combat::pvp_*`, `zone::recover`)

Per point, everything it touches anywhere in the server. "Melee" and "ranged" are the attack kinds of `combat::player` (the weapon
decides: real types 11, 13, 14 are ranged, every other weapon melee; skills name their own kind).

| stat | adds per point | also |
|---|---|---|
| **STR** | melee attack **+1.3**; ranged attack **+1.0** | nothing else (no equip requirements: `EnableEquipment` reads no stat) |
| **DEX** | melee and ranged **hit +0.5**; melee and ranged **evasion +0.5**; melee and ranged attack **+0.2**; **SP +5** | hit and evasion both raise one's own hit chance (section 4.2) |
| **REC** | **HP +5**; melee and ranged **defence +1** | monster aggro: every blow adds REC × 10 to the attacker's aggro (`CDamageMob::Add`); a taunt adds `wAggro` + REC × 10. **Not** recovery |
| **INT** | magic attack **+1.3** | in PvP magic, the target's magic defence is measured against the caster's INT + item attack (section 4.5): more INT, smaller cut |
| **WIS** | magic attack **+0.2**; magic **defence +1**; **MP +5** | heals add **WIS × 2** HP (`wType` 41 heals and the instant part of 81); the magic resist rolls compare both sides' WIS (sections 4.3, 4.5) |
| **LUC** | **critical +0.19** (percentage points); melee and ranged **evasion +0.125**; ranged attack **+0.3** | every landed blow adds `(rand() % LUC + 1) × 0.1` (mean (LUC+1)/20); a critical adds another roll × 1.4; **each point lowers the other side's critical chance by 0.034** |
| **level** | melee and ranged **hit +1**, **evasion +1** | the hit chance's level term, the monster level gap factor, the PvP magic level cut, experience |

Magic hit and magic evasion have **no stat term** at all: only passives, buffs and the faction grade (abilities 22, 31; grade 23) add
to them (verified, `combat::player`: `hit: g.0`, `evasion: g.1`).

The numbers each kind ends with (verified, `combat::player`; `add` = passives, buffs and the faction grade; item = equipment, lapis,
enchant and weapon mastery):

| kind | hit | evasion | attack | defence |
|---|---|---|---|---|
| melee | add + level + DEX × 0.5 | add + LUC × 0.125 + DEX × 0.5 + level | STR × 1.3 + add + item + DEX × 0.2 | add + REC + item defence (+ grade %) |
| ranged | as melee | as melee | LUC × 0.3 + add + item + STR + DEX × 0.2 | as melee |
| magic | add | add | INT × 1.3 + WIS × 0.2 + add + item | add + item magic defence + WIS |

Critical = LUC × 0.19 + passives, buffs and grade (ability 18, grade 16). The **item attack is shared by all three kinds**: a staff's
attack counts for spells, a bow's for its melee number too.

### 3.4 Equipment, lapis, crafted stats, enchants, passives, buffs (verified, `stats::compute`, `combat::gear`, `server::gear`)

In this order: base stats and the status row; each item in slots 0 to 12 (not the mount): its stats, HP/MP/SP, speeds, **attack and
spread for the weapon** (effects 1 and 2), **defence and magic defence for armour and the shield** (effects 3 and 4), all four effects
and move speed for accessories (slots 8 to 12); its **lapis** (any slot but the cloak: stats, HP/MP/SP, attack, spread, defence, magic
defence, range, speeds, **absorption**, a state block outside the weapon); its **crafted stats** (craftable items: nine numbers, the six
stats as points, HP/MP/SP × 100); its **enchant step** (weapon +7 attack a step, armour +5 absorption a step); then the faction grade;
then passives and buffs (abilities). Weapon and armour effects fall to **half below half quality and to nothing below a tenth**
(`combat::wear_level`; accessories and lapis never wear).

- **Weapon mastery** passives add attack for listed weapon real types, **weapon speed** passives attack speed; **shield mastery** adds
  defence while anything is in slot 6 (`skills::passives`).
- **Buffs** of `wType` 82 and 91 add their three abilities (the table of `docs/original/skills.md` section 6: stats, maxima, hit, attack,
  defence, evasion per kind, critical, speeds, range, cost reductions, cooldown cut, sit recovery); **debuffs** of `wType` 62 and 67 take
  theirs off (without touching the maxima).
- **Elements**: the weapon's element (its `attrib`, then its lapis: a first lapis element is taken, a second different one cancels to
  none) attacks; the **upper armour's** defends (`lapis::element`); buffs of `wType` 94 and 95 replace them.

### 3.5 Speeds (verified, `stats::compute`, `combat::attack_interval`)

**Attack speed** = weapon `attacktime` + accessories' + lapis' + weapon-speed passives + ability 16, clamped 0 to 9; the wait between
actions is **4000, 3000, 2750, 2500, 2250, 2000, 1750, 1500, 1250, 1000 ms** for 0 to 9. Weapons carry 2 to 7 (3000 to 1500 ms). **Move
speed** = 2 + accessories' + lapis' + ability 17, clamped 0 to 4. Every action, a normal blow or a skill, takes one wait; a skill with a
cast time takes its cast (`readytime` × 250 ms) and then the wait (section 4.1).

### 3.6 Recovery (verified, `zone::Zones::recover`, `stats::faction_grade`)

Every 5 s, for HP, MP and SP alike: **standing 4**; **sitting 5 % of the maximum + 28** (+ the sit-recovery percentage of abilities 10
to 12); **fighting nothing** (only the faction grade's fight recovery, types 13 to 15). The faction grade adds a percentage of the maximum
to the standing and sitting amounts. **No stat affects recovery.** So between fights a character recovers its maximum in about 20 ticks
(100 s) sitting whatever its class, and SP and MP spent in a fight come back only from potions or the grade.

### 3.7 What the stats do not do

No stat gates equipment (verified, `docs/original/items.md` section 3: level, race, class, mode and sex only). REC does not raise
recovery. INT does nothing for melee or ranged classes; STR nothing for magic; WIS only magic defence, MP, heals, the resist rolls and
0.2 magic attack. DEX is the only source of melee and ranged hit beyond level and buffs. LUC is the only source of critical chance beyond
items, passives, buffs and the grade.

## 4. From a swing to HP lost

### 4.1 The timeline (verified, `docs/original/skills-use.md` sections 2 to 7)

One attack timer per player. A normal blow or a skill goes off when the timer is due, then the timer waits the attack interval; a skill
with a cast time first casts (`readytime` × 250 ms; moving cancels it, nothing spent) and then the interval follows. A skill's
**cooldown starts when it goes off**, its **SP/MP are taken then** (a cost of exactly 1 is everything the player has), after the checks
(costs, weapon or shield, cooldown, the previous-skill combo, range). Cost reductions (abilities 13, 14) cut costs by a percentage; the
cooldown ability 19 (only skill 28's low-HP passive, 500) removes cooldowns of 500 s or less while it lasts.

**Monsters** attack every `attacktime1` ms with entry 0 (their speed starts at 3: no change), use a skill entry (1 or 2) whenever its
cooldown allows (then wait 2.5 s), and some entries only below an HP share (verified, `docs/original/mob-fight.md` section 3,
`fight.rs`, `zone.rs` attack pass).

### 4.2 A player's blow on a monster (verified, `combat::hit_rate`, `combat::formula`, `combat::strike`)

**Hit**: `rate = trunc(160 × Lp / (Lp + Lm) − 100 × mob.hit / (player.evasion + mob.hit) + 100 × player.hit / (mob.evasion + player.hit))`,
clamped **20 to 99**. At equal levels and equal numbers this is 80 %. The middle term means **the player's evasion raises its own hit
chance** (the monster's hit against it); a monster's DEX raises both its hit (DEX × 0.5 + level) and its evasion (DEX × 0.5 + level / 2 +
LUC × 0.125). The placed monsters of levels 58 to 62 have DEX 52 to 456 (median 270), so melee and ranged classes with little DEX hit
them 50 to 60 % of the time (modelled, section 7.3).

**Damage**, with the level factor **P = clamp(f32(((Lm − Lp − 1) × 100 + Lm) / 100 × 10), 1, 95)**:

```
A = attack + rand() % spread                      (ranged within 3: × 0.7)
D = monster defence (of the attack's kind)
damage = (A − A × P / 100) × E − (D + D × P / 100) + (rand() % LUC + 1) × 0.1      at least 0
critical (chance: critical − monster LUC × 0.034, clamped 5 to 99):
damage = damage × 1.5 + R2 − R2 × P / 100,  R2 = (rand() % LUC + 1) × 1.4          (ranged within 3: × 0.7 again)
at most 30,000; 10 or less becomes 1 to 10
```

P by how many levels the monster is above the player (P is 1 % for any monster at or below the player's level):

| monster above | 0 or less | +1 | +2 | +3 | +4 | +5 | +7 | +10 |
|---|---|---|---|---|---|---|---|---|
| player level 15 | 1 | 1.6 | 11.7 | 21.8 | 31.9 | 42.0 | 62.2 | 92.5 |
| player level 30 | 1 | 3.1 | 13.2 | 23.3 | 33.4 | 43.5 | 63.7 | 94.0 |
| player level 60 | 1 | 6.1 | 16.2 | 26.3 | 36.4 | 46.5 | 66.7 | 95.0 |

So each level of gap above +1 takes about 10 % off the player's attack **and** adds 10 % to the monster's defence. `E` is the element
factor (section 4.6). The weapon's quality matters (section 3.4); a critical wears the weapon.

### 4.3 Skills on monsters (verified, `combat::skill_hits`, `combat::skill_damage`, `docs/original/skills-use.md` sections 8 to 12)

**Hit**: a monster that resists the skill's state type or the skill itself is never hit. Then by success type: **1** a flat chance
(`successvalue` %), 2 and 3 unused; otherwise melee and ranged skills use the normal hit rate; **magic skills** are resisted with
`a + b` %, `a = clamp(((Lm − Lp − 2) × 100 + Lm) / (Lp + Lm) × 1.1 + monster magic evasion, 1, 70)`, `b = clamp((mob WIS × 11 − WIS ×
10) / (WIS + mob WIS) × 3.9 + player magic hit, 1, 70)`, total 1 to 90: at equal level and a player WIS above the monster's, **2 %**.

**Damage by `damagetype`**:

| type | damage | defence, element, critical, level factor |
|---|---|---|
| 0 | the skill's values as they are | none |
| 1 | the formula of 4.2 with **A = attack + the skill's value**; SP/MP values as they are | all |
| 2 | the formula of 4.2, **× the skill's value** | all |
| 3 | the skill's value × **the caster's current** HP (or SP, MP) | none |
| 4 | the skill's value **% of the monster's maximum** HP (SP, MP) | none |
| 5 | the skill's value **% of the monster's current** HP | none |

**Multi-hit** (`wType` 22) rolls each of `multiattack` hits separately; area skills hit every enemy within `applyrange`; **drain**
(27) heals the caster by the damage; **additional damage** (`wAddDamage`) adds flat damage after the hit; a **debuff** (kind 3) that hits
and does not kill applies its lasting effect: damage over time (`wType` 61, `wTimeDamage` per 3 s for the keep time, a recast restarts
it and loses the rest), lowered abilities (62, 67), stun, stop, sleep (64 to 66), no element (68), taunt (72), death at the end (63).
The skill's element replaces the weapon's when it has one; `wType` 24 ignores the monster's element (row element, column 0).

### 4.4 A monster's blow on a player (verified, `combat::mob_hit_rate`, `combat::mob_strike`, `combat::mob_skill_damage`, `zone::mob_skills::hurt_player_skip`)

**Hit**: `trunc(160 × Lm / (Lp + Lm) − 100 × player.hit / (mob.evasion + player.hit) + 100 × mob.hit / (player.evasion + mob.hit))`,
clamped **5 to 95**; the player's **blocks** (buffs of `wType` 85, 86, 92) turn a hit into a miss first.

**Damage**: the same P, **turned against the player**: `A = entry damage × strength + rand() % entry spread`,
**`(A + A × P / 100) × E − (D − D × P / 100) + R × 0.1`**, `E` = the first row of the element table against the **upper armour's** element
(the monster's own element never counts: 1.0 for none, 0.8 for elements 1 to 4, 0.7 for 5 to 8: **any element on the upper armour cuts
every monster blow by 20 or 30 %**). Critical: chance monster LUC × 0.19 − player LUC × 0.034 (5 to 99), damage × 1.5 + R2 × (1 + P/100).
Monster skills use the same with entry 0's damage (critical without the extra R2), and types 0, 3, 4, 5 as in 4.3 against the player's
values.

**Then, on the player** (`hurt_player_skip`): SP/MP damage; **give up SP/MP** (`wType` 87) pays HP damage from SP, MP or both first;
**absorption** (lapis and armour enchant steps) takes a flat amount off each blow, at least 1 left (`combat::absorb`); the damage record
(who killed whom); the low-HP passive may start (HP at or below its share: 30 s of abilities, at most every 180 s). **Reflection**
(`wType` 83: always, every row has success 1/100) sends a monster's normal blow back at it and the player takes nothing.

### 4.5 Players against players (verified, `combat::pvp_hit_rate`, `pvp_strike`, `pvp_skill_hits`, `pvp_resist_rate`, `pvp_skill_damage`)

**Physical** (normal blows, melee and ranged skills):

```
hit: as 4.2 with both players' numbers, clamped 1 to 99; the target's block first
d = attack (+ the skill's value for type 1) − target defence + rand() % spread       at least 0, NO level factor
ranged within 3: × 0.7 (unless the cross-road passive; none in the data)
d = trunc(d × E) (the target's upper armour element), + trunc((rand() % LUC + 1) × 0.1)
critical (attacker critical − target LUC × 0.034, clamped 1 to 99): d × 1.5 + R2 × 1.4    (ranged within 3: × 0.7 again)
type 2: × the skill's value; 10 or less: 1 to 10; then the target's absorption
```

**Magic** (skills of attack kind 2): resisted with `a + b` % (`pvp_resist_rate`: the level gap and the target's magic evasion, the two
WIS and the caster's magic hit; 2 % at equal level and WIS), then

```
P1 = clamp((101 × Lt − 100 − 100 × La) / (La + Lt) × 2.4, 1, 50)          1 % at equal level, about 1.2 % a level above
P2 = clamp(target magic defence / (caster INT + caster item attack) × 100, 1, 70)
d = E × (magic attack + the skill's value) × (1 − (P1 + P2) / 100),  + R × 0.1, critical d × 1.5 + R2 × 1.4
```

So physical damage meets defence as a **subtraction** (a defender whose defence nears the attacker's attack takes 1 to 10 a blow), and
magic damage meets magic defence as a **percentage** of the caster's own INT and weapon (never more than 70 %). Damage types 0, 3, 4, 5
skip all of it: **percentage skills cut through any defence** and are only stopped by the hit roll (most have success type 1, 70 %).

**Lapis option skills** (verified, `docs/original/item-options.md`): every landed hit tries **every gem with a skill** on the weapon (the
target) and, for the one hit, on the armour (the attacker): debuffs (sleep, stun, … 7 to 13 % or the magic resist roll) and **lapis 136
and 138 (grade 99): one hit of 90 % / 100 % of the target's maximum HP**, resisted only by the magic resist roll (about 98 % land between
equal players). Not in the model (section 7.4).

**Reflection** (`wType` 83 on the target) sends the whole blow back; **blocks** make a hit a miss; **give up SP/MP** and **absorption**
apply as against monsters; damage over time from a player's debuff is not absorbed.

### 4.6 Elements (verified, `combat::ELEMENTS`)

Rows the attacker's element, columns the defender's (0 none, 1 to 4 the basic elements, 5 to 8 their strong forms):

| | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1.0 | 0.8 | 0.8 | 0.8 | 0.8 | 0.7 | 0.7 | 0.7 | 0.7 |
| 1 | 1.2 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.4 | 1.0 | 1.3 |
| 2 | 1.2 | 1.4 | 1.0 | 0.5 | 1.0 | 1.3 | 1.0 | 0.4 | 1.0 |
| 3 | 1.2 | 1.0 | 1.4 | 1.0 | 0.5 | 1.0 | 1.3 | 1.0 | 0.4 |
| 4 | 1.2 | 0.5 | 1.0 | 1.4 | 1.0 | 0.4 | 1.0 | 1.3 | 1.0 |
| 5 | 1.3 | 1.0 | 0.5 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 |
| 6 | 1.3 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.5 | 1.0 |
| 7 | 1.3 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.5 |
| 8 | 1.3 | 0.5 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 |

Any element attacking a target without one is +20 % (+30 % for 5 to 8); no element against any element is −20 % (−30 %). Against
monsters the player's element counts only on attack (the monster's blows use row 0 against the armour, section 4.4).

### 4.7 Caps and floors (verified)

At most **30,000** a blow (`MAX_DAMAGE`); **10 or less becomes 1 to 10** at random for every formula type (not types 0, 3, 4, 5); every
blow on a player leaves **at least 1** after absorption. HP maxima are `u16` in the packets (65,535) and **`smallint` in the database
(32,767; `characters.hp`)**.

### 4.8 Buffs, debuffs and passives in the numbers (verified, `buffs::PlayerBuffs::effects`, `skills::passives`)

Everything a buff or passive does to the numbers goes through the abilities of section 3.4 (an ability adds to a stat, a maximum, a
kind's hit, attack, defence or evasion, critical, speeds, range, cost reduction or cooldown) or one of the special `wType`s: blocks (85
count, 86 chance, 92 all), reflection (83), give up SP/MP (87), elements (94, 95), heal over time (81), Fire Thorns (84), taunt (69),
hiding (88), transformations (90: the monster's attack and defence replace the equipment's). Low-HP passives (`wType` 6) add their
abilities for 30 s when HP falls below 8 to 25 %. Passives add weapon damage and speed per weapon type, shield defence, and abilities
(hit, attack, defence, evasion, maxima, sit recovery).

### 4.9 Lapis, enchanting and crafted stats: strength, chances, costs (verified, `lapis.rs`)

- **Enchant step**: weapon **+7 attack**, armour **+5 absorption** a step (`lapis::enchant_bonus`).
- **The enchant roll** (`lapis::enchant`, `Enchant::succeeds`): success when **material count + a draw below 1,000,000 < rate[step]**;
  the material (the item's `reqluc`) is **required** with a count of 1 to 100, so **more material lowers the chance**, and a rate of 1
  can never succeed. Each try uses one Lapisia and the material and costs the Lapisia's grade price: **2.5 million for a weapon
  Lapisia (grade 96), 1.04 million for an armour Lapisia (grade 40)**; the plain Weapon and Armor Lapisia (95/1, 95/6) are dangerous.
  Expected tries with one material:

  | to reach | +3 | +5 | +7 | +9 | +10 | +11 | +12 | +13 | +14 | +15 and above |
  |---|---|---|---|---|---|---|---|---|---|---|
  | weapon | 7 | 51 | 485 | 4,830 | 14,931 | 49,414 | 160,525 | never | never | never |
  | armour | 3 | 16 | 150 | 1,484 | 4,829 | 14,930 | 49,412 | 160,524 | 660,524 | never |

  A failure keeps the step; a **dangerous** Lapisia destroys the item on failure (no Lucky Charm for enchanting). In money alone
  (modelled arithmetic), a weapon to +7 costs about 1.2 billion and to +9 about 12 billion, past the 4.29 billion a character can hold.
- **Linking a lapis** (`lapis::base_rate`, `lapis::link`): `rate = trunc(289 − 8 × grade + KC + level × 0.05)`, clamped 1 to 50, × a
  Linking Hammer's 110 to 450 %, × 1.5 with premium, + a guild house bonus. At level 60: grade 31 **44 %**, 32 36 %, 33 28 %, 34 20 %, 35
  12 %, 36 4 %, **37 and above 1 %** (the faction grade adds 1 to 5). The price is the lapis' grade price (4,095 for grade 31 to
  1,040,000 for 40; 2.5 to 9.8 million for 96 to 100).
  Dangerous lapis destroy the item on a failed link unless a Lucky Charm is used. Extraction rolls the same rate (or a hammer's 30 to 90 %).
- **Crafted stats** (`lapis::roll_craft`): a craftable item has up to nine numbers, each the **smallest of three draws** of `0 to reqwis`
  when it drops, of two with a Recreation Rune; the item's `server` column decides how many numbers it gets. Points count as stats; HP,
  MP and SP × 100.

## 5. Levels and modes

### 5.1 What the mode changes (verified, `stats::scaled`, `combat::add_exp`, `combat::max_level`, `skills::check`, `docs/original/items.md`)

| | easy | normal | hard | ultimate |
|---|---|---|---|---|
| top level | 30 | 60 | 60 | 60 |
| HP/SP/MP of the status table | × 1 | × 1 | × 1.1 | × 1.5 |
| stat points a level | 5 | 5 | 7 | 9 |
| skill points a level | 3 | 3 | 4 | 5 |
| skills (`grow` column: the lowest mode) | grow 0 | 0, 1 | 0 to 2 | all |
| items (`grow` column) | 0 | 0, 1 | 0 to 2 | all |
| experience to level 60 | (cap 30) | 284.7 million | 1,125 million | 1,405 million |
| death | as normal | as normal | as normal | **the original deletes the character** (not built here: `docs/original/mob-fight.md` section 8) |

Learnable skill rows by their mode: 336 for everyone, 145 from normal, 97 from hard, 62 ultimate only (each class has 2 to 5 ultimate-only
skills, among them the level-60 percentage skills). Equipment rows: 8,012 / 3,506 / 2,000 / 917.

### 5.2 Experience from a kill (verified, `combat::mob_exp`, `combat::kill_exp`, `pvp::victim_exp`)

`exp = trunc(monster exp × ((Lm − Lp + 1) × 0.2 + 1))`, **0 when the monster is more than 9 levels above**, and 0 from 6 levels below;
the solo killer gets **× 1.2**. By level gap (monster − player): −5 0.2, −3 0.6, −1 1.0, **0 1.2**, +2 1.6, +4 2.0, +9 3.0 (before the 1.2).
EXP stones × 2, premium × 1.5, a charm + 0.2, the operator's event rate × its factor (`buffs::exp_factor`). Parties share by the tables of
`docs/original/party.md`. A player kill gives `50 × victim level × the same gap factor` (× 1.2 solo), at least 1. Death costs 5 % of
the level's span (1 % or 0 with charms, less with the faction grade's type 25).

### 5.3 The curve (verified data, `defs.exp_defs`; modelled kill counts)

Experience to pass a level (ultimate) and its ratio to the other modes:

| level | ultimate | ÷ normal | ÷ hard | running total (ultimate) |
|---|---|---|---|---|
| 1 | 200 | 2.0 | 1.0 | 200 |
| 10 | 15,400 | 2.0 | 1.0 | 53,200 |
| 20 | 172,800 | 3.0 | 1.0 | 849,200 |
| 30 | 880,800 | 3.0 | 1.0 | 5,809,400 |
| 40 | 4,242,000 | 4.0 | 1.25 | 32,160,200 |
| 50 | 25,000,000 | 5.0 | 1.25 | 130,449,100 |
| 55 | 123,750,000 | 5.0 | 1.25 | 510,499,100 |
| 59 | 295,000,000 | 5.0 | 1.25 | 1,405,399,100 |

Easy mode stops at 30 (195,730 a level from 30). **Kills to pass a level** against the most placed monster of that level, solo, no
bonuses (modelled):

| level | monster (kill exp) | normal | hard | ultimate |
|---|---|---|---|---|
| 15 | 2125 (237) | 100 | 200 | 200 |
| 30 | 2183 (633) | 464 | 1,391 | 1,391 |
| 58 | 1547 (17,992) | 2,708 | 10,831 | 13,539 |
| 59 | 2461 (18,734) | 3,149 | 12,597 | 15,747 |

The level-58 to 60 monsters have 9,500 to 340,000 HP; most kills at the cap are party kills (shared experience), which this table does
not show.

## 6. Factions and races: where they differ today

The formulas have **no faction branch** (verified: nothing in `combat.rs`, `stats.rs`, `buffs.rs`, `lapis.rs` reads the country). The
factions differ only through these data:

### 6.1 Race (verified, `creation::starting_stats`, `stats::compute`)

Starting stats (human LUC + 4, elf DEX + 4, vail INT and WIS + 2, deatheater STR and REC + 2) and the maxima bonus (human +10 SP, elf
+10 MP, vail +5 SP +5 MP, deatheater +10 HP). Since each class has one race per faction, every mirrored pair differs by these: e.g. the
fighter (human) has LUC + 4 where the warrior (deatheater) has STR + 2, REC + 2 and +10 HP. Small at the cap (a few points out of 600).

### 6.2 Race skills (verified data, `defs.skills` with `country` ≠ 6)

| race (its classes) | skill | levels 1 / 2 / 3 | keep / cooldown |
|---|---|---|---|
| human (fighter, defender, priest) | 188 Berserker | hit +3 / +5 / +8 to all three kinds | 90-180 s / 180 s |
| elf (ranger, archer, mage) | 189 Evasion | melee and ranged evasion +90 / +180 / +240; **costs all SP and MP** (cost 1) | 10-30 s / 180 s |
| deatheater (warrior, guardian, hunter) | 190 Battle Cry | sitting HP recovery +2 / +3 / +5 % of the maximum | 90-180 s / 180 s |
| vail (assassin, pagan, oracle) | 191 Bloody Karma | HP +60/+204/+458, melee and magic attack +15/+52/+108 | 20-60 s / 180 s |
| human fighter | 129 Speedy Attack | attack speed +2 | 20-45 s / 600 s, hard and up |
| deatheater warrior | 128 Power up Pause | melee attack +50 / +100 / +170 | 30 s / 600 s, hard and up |
| elf ranger and archer | 101 Misfortune (debuff) | target LUC −15 / −35 / −55 | 30 s, hard and up |
| vail assassin, deatheater hunter | 110 Blunt (debuff) | target DEX −15 / −35 / −55 | 30 s, hard and up |

These are far apart in value (an attack buff of +108 against +5 % sitting recovery; a hit buff against an evasion buff that empties SP
and MP). Part 3 measures them.

### 6.3 Faction kill-count grades (verified data, `world.kill_status`; `stats::faction_grade`, `server::gear`)

Each faction has 30 grades by its kill count (150 to 12,288 kills); the top lasts 10 minutes, then the faction drops to grade 0 and the
other faction's count resets. What the grades give, cumulated (verified data):

| bonus | Light | Fury |
|---|---|---|
| max HP % (type 1) | **none** | +6 % from grade 3, **+9 % from grade 13** |
| max MP %, SP % (2, 3) | +5 % from grade 27 | +6 % from 3, +9 % from 13, +11 % from 27 |
| ranged defence % (19) | +2 % from 14 | **+7 %** from 14 |
| magic defence (20) | +2 from 14 | **+7** from 14 |
| melee defence % (18) | +2 % from 15 | +2 % from 15 |
| recovery % of max (10 to 12) | 5 then 10 % | 5-6 then 10 % |
| fight recovery SP/MP (14, 15) | 3 % from 26 | 3 % from 26 |
| all six stats (4 to 9) | +10 from 29 | +10 from 29 |
| critical +30, all evasions +30 (16, 21 to 23) | grade 30 only | grade 30 only |
| link rate (24), less death loss (25) | +1 to +5, 50-100 % | the same |

So at equal kill counts **Fury's grades are stronger** (HP, MP, SP, ranged and magic defence). The client holds the same file
(`KillStatus.SData`); what it displays from it is not studied.

### 6.4 Items and skills

Mirrored classes have their own skill rows and item rows (the race and class columns). Whether they are mirrored is part 2's inventory.
The model's samples (section 7.3) show the pairs close on the same picker but not equal, from the race, the race skills (not cast in the
model) and different picked items.

## 7. The model (`tools/balance/`)

### 7.1 What it is

A port of the server's formulas (`formulas.py`), their expected values (`expect.py`), the data as the server loads it (`data.py`) and a
fight model (`model.py`): for a class, race, mode, level, stats, gear (with lapis and enchants), skills, buffs and faction grade it gives
the character sheet; against a monster, each action's expected damage, the time to kill with the SP/MP pool and with unlimited SP/MP
(potions), the monster's damage per second, the time to die, the mitigation and the effective HP; against a player, both sides' kill
times. Usage and limits: `tools/balance/README.md`.

### 7.2 Validation (verified)

- **Unit tests replayed** (`test_formulas.py`): the 23 relevant unit tests of `stats.rs`, `combat.rs`, `lapis.rs`, `kc.rs`, `item_use.rs`,
  with the same inputs and roll sequences, all pass; plus hand-worked PvP blows and the expected values against 40,000 samples of the
  exact functions (within 1.5 %).
- **Saved characters** (`check_saved.py`): the 8 characters of the dev database saved at full health show exactly the model's maxima
  (fighter, archer, ranger, mage, priest; normal and ultimate; levels 1 to 60); the ninth (an assassin with GM items) was saved with HP
  32,767, the column's limit, against the model's 56,713.
- Nothing differs between the port and the code. The documents differ from the code in a few places (section 8).

### 7.3 Sample runs (modelled; `scenarios.py`)

Rerun after part 2 corrected where items come from (quests of unplaced NPCs, mall boxes: `docs/plans/balance/2-content.md`
section 2), which changes what the gear picker takes; the first cut's tables are replaced.

Default builds: **ultimate, every free point into STR (melee and ranged classes) or INT (mage, priest)**, the rough gear picker
(obtainable items only, no enchant, no lapis), every learnable skill at its highest level (more than the skill points allow at 60: 447
to 469 against 300), no buffs, grade 0. These show the model works and where the questions are; **they are not the class comparison**
(part 3 builds realistic sets).

**Level 15** against monster 2125 (level 15, HP 848, defence 19, DEX 27, attack 63 + up to 27 every 2.5 s):

| class | HP | hit % | normal DPS | best skill | kill s (SP/MP pool) | kill s (unlimited) | monster DPS | dies in s | mitigation |
|---|---|---|---|---|---|---|---|---|---|
| fighter | 1485 | 77 | 67 | Deadly Strike Lv1 133/s | 10.0 | 10.0 | 18 | 82.1 | 0.41 |
| warrior | 1505 | 77 | 67 | Deadly Strike Lv1 134/s | 10.0 | 10.0 | 17 | 86.4 | 0.43 |
| defender | 1696 | 73 | 60 | normal attack 60/s | 13.5 | 13.5 | 10 | 171.4 | 0.68 |
| guardian | 1716 | 72 | 59 | normal attack 59/s | 13.5 | 13.5 | 9 | 184.4 | 0.70 |
| ranger | 1187 | 99 | 118 | Aggravation Lv1 146/s | 6.0 | 6.0 | 14 | 82.1 | 0.53 |
| assassin | 1187 | 97 | 115 | Aggravation Lv1 143/s | 6.0 | 6.0 | 15 | 79.5 | 0.51 |
| archer | 1150 | 74 | 54 | Rapid Shot Lv1 108/s | 11.2 | 11.2 | 16 | 73.6 | 0.49 |
| hunter | 1170 | 70 | 51 | Rapid Shot Lv1 103/s | 12.0 | 12.0 | 16 | 74.2 | 0.48 |
| mage | 776 | 75 | 6 | Sonic Bolt Lv2 146/s | 5.2 | 5.2 | 22 | 35.9 | 0.29 |
| pagan | 776 | 70 | 5 | Sonic Bolt Lv2 149/s | 5.2 | 5.2 | 23 | 33.9 | 0.25 |
| priest | 856 | 71 | 6 | Hailstorm Lv1 141/s | 6.0 | 6.0 | 22 | 38.4 | 0.27 |
| oracle | 856 | 70 | 5 | Hailstorm Lv1 142/s | 6.0 | 6.0 | 22 | 38.0 | 0.26 |

**Level 30** against monster 2183 (level 30, HP 1,802, defence 27, DEX 46, attack 113 + up to 49):

| class | HP | hit % | normal DPS | best skill | kill s (SP/MP pool) | kill s (unlimited) | monster DPS | dies in s | mitigation |
|---|---|---|---|---|---|---|---|---|---|
| fighter | 3324 | 81 | 191 | Deadly Strike Lv2 573/s | 4.0 | 4.0 | 27 | 121.2 | 0.54 |
| warrior | 3344 | 83 | 197 | Deadly Strike Lv2 590/s | 4.0 | 4.0 | 26 | 127.7 | 0.56 |
| defender | 3706 | 80 | 168 | Ground Boom Lv2 180/s | 10.0 | 10.0 | 10 | 385.4 | 0.84 |
| guardian | 3701 | 81 | 153 | Ground Boom Lv2 163/s | 11.2 | 11.2 | 10 | 353.0 | 0.82 |
| ranger | 2606 | 99 | 267 | Slasher Lv1 534/s | 6.0 | 6.0 | 28 | 91.9 | 0.53 |
| assassin | 2606 | 99 | 266 | Slasher Lv1 533/s | 6.0 | 6.0 | 29 | 90.7 | 0.52 |
| archer | 2485 | 79 | 128 | Rapid Shot Lv2 383/s | 8.2 | 8.2 | 28 | 89.9 | 0.54 |
| hunter | 2505 | 78 | 127 | Rapid Shot Lv2 380/s | 8.2 | 8.2 | 28 | 90.9 | 0.54 |
| mage | 1581 | 72 | 22 | Turbulence Lv1 548/s | 3.5 | 3.5 | 45 | 35.4 | 0.26 |
| pagan | 1592 | 70 | 21 | Turbulence Lv1 550/s | 3.5 | 3.5 | 46 | 35.0 | 0.24 |
| priest | 1759 | 70 | 21 | Hailstorm Lv2 383/s | 5.2 | 5.2 | 45 | 39.0 | 0.25 |
| oracle | 1785 | 70 | 21 | Hailstorm Lv2 384/s | 5.2 | 5.2 | 44 | 40.6 | 0.27 |

**Level 60** against monster 1547 (level 58, HP 9,520, defence 473, magic defence 289, DEX 170, attack 961 + up to 79):

| class | HP | hit % | normal DPS | best skill | kill s (SP/MP pool) | kill s (unlimited) | monster DPS | dies in s | mitigation |
|---|---|---|---|---|---|---|---|---|---|
| fighter | 8086 | 58 | 163 | Enhance Wind Spin Lv3 624/s | 42.0 | 24.0 | 481 | 16.8 | 0.10 |
| warrior | 8726 | 54 | 148 | Enhance Wind Spin Lv3 568/s | 44.0 | 28.0 | 478 | 18.3 | 0.10 |
| defender | 8936 | 56 | 129 | Behimoth Range Lv3 531/s | 58.0 | 44.0 | 444 | 20.1 | 0.17 |
| guardian | 9576 | 52 | 116 | Behimoth Range Lv3 523/s | 62.0 | 46.0 | 442 | 21.7 | 0.17 |
| ranger | 6530 | 80 | 265 | Slasher Lv3 920/s | 35.0 | 35.0 | 433 | 15.1 | 0.19 |
| assassin | 6775 | 74 | 239 | Slasher Lv3 833/s | 38.8 | 38.8 | 451 | 15.0 | 0.16 |
| archer | 6114 | 54 | 106 | Rapid Shot Lv3 378/s | 56.0 | 44.8 | 476 | 12.8 | 0.11 |
| hunter | 6804 | 52 | 120 | Rapid Shot Lv3 430/s | 49.5 | 39.0 | 468 | 14.5 | 0.12 |
| mage | 3904 | 47 | 2 | Flame Shock Lv2 1144/s | 12.0 | 12.0 | 498 | 7.8 | 0.07 |
| pagan | 4124 | 48 | 2 | Flame Shock Lv2 1108/s | 12.0 | 12.0 | 494 | 8.3 | 0.07 |
| priest | 4336 | 46 | 1 | Surgy Shock Lv2 774/s | 17.0 | 17.0 | 490 | 8.8 | 0.08 |
| oracle | 4556 | 47 | 2 | Surgy Shock Lv2 739/s | 18.8 | 18.8 | 490 | 9.3 | 0.08 |

**Mirrored pairs** (Light against Fury, at range, SP/MP pool; seconds to kill, Light / Fury; at 60 also without the four percentage
skills 180, 182, 183, 184):

| pair | 15 | 30 | 60 | 60 without 180-184 |
|---|---|---|---|---|
| fighter / warrior | 12.5 / 12.5 | 10.0 / 8.0 | 4.0 / 4.0 | 6.0 / 4.0 |
| defender / guardian | 24.8 / 24.8 | 22.0 / 22.5 | 20.0 / 20.0 | 20.0 / 20.0 |
| ranger / assassin | 9.0 / 9.0 | 9.0 / 9.0 | 3.8 / 5.0 | 6.2 / 7.5 |
| archer / hunter | 13.5 / 13.5 | 10.5 / 10.5 | 4.5 / 4.0 | 7.0 / 6.0 |
| mage / pagan | 5.0 / 5.0 | 3.5 / 3.5 | 3.5 / 3.5 | 3.5 / 3.5 |
| priest / oracle | 6.0 / 6.0 | 5.2 / 5.2 | 5.2 / 6.5 | 5.2 / 6.5 |

What these suggest (modelled with rough builds; part 3 tests each with real builds):

- **All-STR melee builds hit cap monsters only about 52 to 58 % of the time**; the ranger (DEX as its class stat) hits 80 %. DEX for
  hit is likely part of any real melee build (section 4.2).
- **Mages and priests die fastest to monsters** (8 to 9 s against 1547, 3,900 to 4,600 HP, mitigation 0.07 to 0.08) and kill fastest at
  30 and 60; their normal blows do nothing (1 or 2 a second), so they live on MP.
- **SP limits physical classes against tough monsters**: the archer's kill time drops from 56 s to 45 s with unlimited SP; mages are not
  MP-limited on one kill.
- **PvP at 60 is over in 3.5 to 6.5 s** for every pair but the defenders (20 s); without the percentage skills the warrior, the hunter
  and the ranger kill their mirror faster than it kills them (from different picked weapons and items, and the race: part 2 section 3.7,
  part 3).
- The defender's mitigation (0.68 at 15, 0.84 at 30) comes from shield, armour and REC against weak monster attacks: subtraction makes
  defence decisive when attacks are small, and nearly irrelevant at 60 against 961-attack monsters (0.17).

### 7.4 What the model leaves out

Real variance (it gives means); parties, healing, the support classes' buffs on others; blocks, reflection, stuns and slows in a fight;
**lapis option skills** (including the 90/100 % lapis); range, movement, kiting, line of sight; area skills on several targets; aggro and
other monsters; potions' timing (only "none" or "unlimited"); the low-HP passives; buffs unless given with `buffs=`; the item mall
unless `mall=1`; skill points (it learns everything). The gear picker is a rough score, not the best set.

## 8. Discrepancies between the documents and the code (the code wins)

| where | the document says | the code does |
|---|---|---|
| `docs/original/combat.md` (preamble) | "the new server computes in single precision" | double precision, rounded to single where the original stores (`combat.rs` header); results agree except at exact boundaries |
| `docs/original/combat.md` section 4, `docs/original/lapis.md` section 2 | lapis and enchant attack count "unless in a shape" | `combat::gear` adds them always, but `server::gear` replaces the item attack with the monster's while transformed, so the result is the same |
| `docs/original/stats.md` section 2 | item stats added as stored | `pg::load_item_defs` floors negative stats, HP and effects at 0; no row has a negative value (checked), so nothing changes today |
| `docs/original/skills-buffs.md` section 10 | Magic Reflection "waits for monster skills and player kills" | both are built since; reflection applies to them (`buffs::PlayerBuffs::reflects`) |

None of these changes a number.

## 9. What the client decides or shows itself

- **Tooltips** of items and skills come from the client's own copies (`Item.SData`, `Skill.SData`): any change of an item's or skill's
  numbers needs a client data patch to show (`docs/plans/done/client-patches.md`); 123 reachable items and about 30 skills already differ
  (`docs/client/item-sdata-vs-defs.md`, `docs/client/skill-monster-sdata-vs-defs.md`).
- **The client refuses** to equip below its own item level and may not offer skills its copy does not allow (the archer's Trap, the
  fighter's Pollution Zone points, weapon lists); a change of levels, classes or weapon lists needs the patch to take.
- **The character window's attack, defence and other derived numbers**: whether the client computes them with its own formulas or takes
  them from the server is **not studied** (no packet carries the attack numbers; inferred: the client computes them). If it computes
  them, **any change of a formula (section 3.3) would show wrong numbers in the window** until the client is patched, and a client binary
  patch is a different matter from a data patch. A client research question (to add to `docs/testing/client-research-queue.md` in part 3).
- **Casting bars and cooldown displays** use the client's skill copy; the server's cast times decide.
- **The status table** (base HP/SP/MP): whether the client has a copy (for the window before the server's `0x0105`) is not studied; the
  server sends the maxima itself.
- **Kill-count grades**: the client has `KillStatus.SData`; what it shows from it is not studied.

## 10. What cannot be settled without live play

- **Real builds and gear**: what players actually wear, enchant and link (the alpha will show); the picker's sets are a stand-in.
- **Fight conditions**: party sizes, healers, how often PvP is 1-on-1, ranges and kiting, potion use, reaction times; the model's kill
  times are best-case means.
- **Monster encounters**: which monsters players farm at each level and in what groups; the cap monsters look like party content.
- **The variance**: how often the percentage skills (70 %) and lapis procs decide a fight; means hide streaks.
- **The client's displayed numbers** (section 9) and whether players notice small changes.
- **Experience pace**: time per level depends on parties, maps and events, not only kill counts.
