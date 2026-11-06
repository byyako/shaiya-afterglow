# Hitting and being hit

Every step from pressing attack to the HP a blow takes off, in the original game: the attack timer, whether a blow lands, how much it
does to a monster, what a monster does back, and how players hurt each other. The numbers it starts from (hit, evasion, attack, defence,
critical) are on [Your character's numbers](character-numbers.md), which also explains the words used here; elements, magic resistance,
absorption and blocks are on [Elements, resistances and defences](elements-defences.md).

## In short

- **One action at a time.** A normal blow or a skill goes off when your attack timer is ready; then the timer waits 1 to 4 seconds by your
  attack speed. A skill with a cast time casts first; moving cancels the cast and costs nothing.
- **Hitting is a contest of both sides' hit and evasion, and of the levels.** At equal level and equal numbers you land 80 % of blows on a
  monster. Your own evasion raises your chance too, so DEX counts twice.
- **Against monsters, levels matter a lot.** Each level a monster has above yours takes about 10 % off your attack and adds about 10 % to
  its defence; in its blows on you it is the reverse. A monster at or below your level gains you nothing more.
- **Against players there is no level factor for physical blows**: damage is attack minus defence. Magic is different: the target's magic
  defence cuts a percentage (at most 70 %) of the spell.
- **Criticals** do 1.5 times as much. Your chance is LUC × 0.19 minus a little for the target's LUC, never under 5 % against monsters.
- **Some skills ignore everything**: those that do a share of the target's HP or a multiple of yours skip defence, elements and
  criticals, and only their hit roll can stop them.
- **A blow worked out by the formula does at most 30,000,** and one that would do 10 or less does a random 1 to 10 instead.

## 1. Where the numbers are worked out

A blow uses the attacker's numbers for its **attack kind** (melee, ranged or magic) and the target's numbers for the same kind. A normal
blow is melee or ranged by the weapon; a skill names its own kind. Monsters' normal blows are always melee.

## 2. The timeline

1. Each character has **one attack timer**. When it is ready the next action goes off: a normal blow on your target, or the skill you
   asked for (asking does not use a skill at once: it takes the place of your next blow).
2. After every action the timer waits by your attack speed: **4 s at speed 0 down to 1 s at speed 9** (2.5 s at speed 3)
   ([Speeds](character-numbers.md#8-speeds)).
3. A skill with a **cast time** first casts for its ready time × 0.25 s; the effect comes at the end of the cast, then the wait. **Moving
   cancels a cast**, and nothing is spent.
4. A skill is checked when you ask (enough SP and MP after cost reductions, the right weapon or a shield, its cooldown over, the skill
   before it in a combo just used, in range). When it goes off, **its cooldown starts and its SP and MP are taken**. A cost of exactly 1
   means *all* your SP (or MP).
5. **Cost reductions** (abilities 13 and 14) cut costs by a percentage; Potential Motive's ability 19 removes cooldowns of 500 s or less
   while it lasts.

Monsters attack every 2.5 s or so with their normal blow (their data's own interval), use a skill entry when its cooldown allows (then wait
2.5 s more), and some entries only below a share of their HP.

Source: [Skills 2](../original/skills-use.md) sections 2 to 7; monsters in [Monsters fighting back](../original/mob-fight.md) section 3.

## 3. Hit chance

All three cases use the same shape: a level term, minus the defender's chance against you, plus yours against it.

```
you on a monster:       rate = 160 × Lyou / (Lyou + Lmob) − 100 × mob hit / (your evasion + mob hit) + 100 × your hit / (mob evasion + your hit)
                        held between 20 and 99 %
a monster on you:       the same seen from the monster (its level in the first term), held between 5 and 95 %
a player on a player:   the same with both players' numbers, held between 1 and 99 %
the blow lands when a roll of 0 to 99 is below the rate
```

- **Your evasion raises your own hit chance.** The middle term is the monster's hit against your evasion: the higher your evasion, the
  smaller that term, the higher your rate. DEX adds to both your hit and your evasion, so it counts twice.
- At equal levels and equal numbers the rate is 80 %. Five levels of gap move it only 4 points (the first term); the numbers matter more.
- Before the roll, a **block** of the defender turns the blow into a miss ([Blocks](elements-defences.md#7-blocks-reflection-and-giving-up-sp-or-mp)).

What DEX does at level 60 against a level-60 monster (monster LUC 150; the player's hit and evasion from DEX and level only):

| your DEX (total) | monster DEX 100 | monster DEX 270 | monster DEX 456 |
|---|---|---|---|
| 12 (none added) | 58 % | 32 % | 20 % (the floor) |
| 100 | 83 % | 53 % | 36 % |
| 200 | 99 % | 71 % | 52 % |
| 400 | 99 % | 95 % | 76 % |

The placed monsters of levels 58 to 62 have DEX 52 to 456 (270 in the middle), so a melee build with little DEX misses them often.
The other way round, the same monster with DEX 270 lands 95 % of its blows on you at DEX 12 or 100, 88 % at 200 and 64 % at 400.

Source: `CUser::AttackSuccess` ([Fighting monsters](../original/combat.md) section 3), `CMob::AttackSuccess`
([Monsters fighting back](../original/mob-fight.md) section 6), the player version ([Player kills 1](../original/pvp.md)
section 4).

## 4. Damage on monsters

```
P = ((Lmob − Lyou − 1) × 100 + Lmob) / 100 × 10        the level factor, held between 1 and 95
A = your attack of the kind + a roll of 0 to (spread − 1)     (ranged within 3 of the monster: × 0.7)
D = the monster's defence of the kind (magic: its magic defence)
E = the element factor, your attack element against the monster's (Elements)
R = a roll of 1 to your LUC

damage = (A − A × P / 100) × E − (D + D × P / 100) + R × 0.1        at least 0
critical: damage × 1.5 + R2 × 1.4 × (1 − P / 100),  R2 another roll of 1 to LUC      (ranged within 3: × 0.7 again)
then cut to a whole number, at most 30,000; 10 or less becomes a random 1 to 10
```

**The level factor P** (in %) by how many levels the monster is above you:

| monster above you | 0 or below | +1 | +2 | +3 | +4 | +5 | +7 | +10 |
|---|---|---|---|---|---|---|---|---|
| at level 15 | 1 | 1.6 | 11.7 | 21.8 | 31.9 | 42.0 | 62.2 | 92.5 |
| at level 30 | 1 | 3.1 | 13.2 | 23.3 | 33.4 | 43.5 | 63.7 | 94.0 |
| at level 60 | 1 | 6.1 | 16.2 | 26.3 | 36.4 | 46.5 | 66.7 | 95 |

P takes that share off your attack **and** adds it to the monster's defence. It never goes below 1 %, so a weaker monster is not easier
than one of your own level.

- **The critical chance** is your critical minus the monster's LUC × 0.034, **held between 5 and 99 %**: even a character with no LUC
  gets one blow in twenty.
- **The luck bonus** (R × 0.1) is small: 0.1 to 3 at LUC 30.
- **Ranged attacks within 3** of the monster lose 30 % (twice on a critical). The cross-road passive that would remove this exists in the
  code but not in the data.
- A critical **wears your weapon**.

Source: `CUser::AttackDamage` and `CUser::CriticalSuccess`, [Fighting monsters](../original/combat.md) section 3.

## 5. Skills

**Hit or miss**, in order:

1. A monster that resists the skill's state (its data lists 15 states) or the skill itself is never hit.
2. **Success type 1**: a flat chance, the skill's own (for example 70 %). No level or stat counts.
3. Otherwise **melee and ranged skills** use the hit chance of section 3, and **magic skills** a resist roll: the target resists when a
   roll of 0 to 99 is below a + b, with a from the level gap and the target's magic evasion, b from the two WIS and your magic hit
   (written out in [Resisting magic](elements-defences.md#4-resisting-magic)). At equal levels with your WIS above the monster's it is
   about 2 %.

**Damage by the skill's damage type**:

| type | damage | defence, element, critical, level factor |
|---|---|---|
| 0 | the skill's values as they are | none |
| 1 | the formula of section 4 with **A = attack + the skill's value** | all |
| 2 | the formula of section 4, then **× the skill's value** | all |
| 3 | the skill's value × **your own current** HP (or SP, MP) | none |
| 4 | the skill's value **% of the target's maximum** HP (SP, MP) | none |
| 5 | the skill's value **% of the target's current** HP | none |

Types 3, 4 and 5 are at least 1 and at most 30,000; they and type 0 never give a critical.

- **The element**: a skill with its own element uses it instead of your weapon's; a few skills ignore the target's element.
- **Multi-hit** skills roll each hit separately; **area** skills hit every enemy (monsters and players of the other side) within their
  area of the target or of you, the target included.
- **Drain** heals you by the damage it does (up to your maxima). **Additional damage** adds a flat amount after the hit.
- A **debuff** that hits and does not kill leaves its effect: damage over time (paid every 3 s for its time; a recast restarts it and the
  rest is lost), lowered numbers, stun, hold, sleep, no element, a taunt, or death at its end.
- **Heals** give their value, and the HP part **+ your WIS × 2**.

Source: [Skills 2](../original/skills-use.md) sections 8 to 12 (`CUser::AttackMob`, `CUser::AttackSuccess`,
`CUser::AttackDamage`), [Skills 3](../original/skills-buffs.md) section 12.

## 6. Monsters on you

A monster's normal blow uses its data's attack, the same level factor P **turned around**, and your melee defence:

```
A = the monster's attack + a roll of 0 to (its added attack − 1)
damage = (A + A × P / 100) × E − (D − D × P / 100) + R × 0.1        P from the monster's level above yours, as in section 4
D = your melee defence;  R = a roll of 1 to the monster's LUC
E = the table's first row against your upper armour's element: 1.0 with none, 0.8 for fire, water, earth or wind, 0.7 for their strong forms
critical (chance: monster LUC × 0.19 − your LUC × 0.034, 5 to 99 %): damage × 1.5 + R2 × 1.4 × (1 + P / 100)
```

So a monster two levels above you at level 60 hits **16 % harder and through 16 % of your defence**. The monster's own element never
counts in its normal blows; any element on your upper armour cuts them by 20 or 30 % ([Elements](elements-defences.md#2-elements)).
Monster criticals are common: their LUC is often in the hundreds, so a chance of 30 % or more is usual at the cap.

**Monster skills** use the same formula with the skill's attack kind: a melee skill against your melee defence, a ranged one against
your ranged defence, a magic one against your **magic defence, subtracted the same way** (monsters' magic is not a percentage cut). Their
attack is the normal blow's attack and spread, plus the monster's attack number of that kind (WIS + level for magic, 0 otherwise), plus
the skill's value for type 1; their critical has no R2 part; the skill's own element counts against your armour's; types 0, 3, 4 and 5
work on your values as in section 5.

**Then, on you**: SP and MP damage; giving up SP or MP (a buff) pays the HP damage first; **absorption** takes its flat amount off (at least
1 left); a blow that leaves you alive may start your low-HP passive. A hit stands you up, breaks a cast, wakes you and ends hiding.

Source: `CMob::Attack` and `CMob::AttackDamage` ([Monsters fighting back](../original/mob-fight.md) section 6), the
skill version ([Monster skills](../original/mob-skills.md) sections 5 and 7), `CUser::Damage`.

## 7. Players against players

**Physical blows** (normal blows, melee and ranged skills):

```
hit: section 3 with both players' numbers, 1 to 99 %; the target's block first
d = attack (+ the skill's value for type 1) − the target's defence + a roll of 0 to (spread − 1)     at least 0; NO level factor
ranged within 3: × 0.7
d = trunc(d × E)                E: your attack element against the target's upper armour element
d = trunc(d + R × 0.1)
critical (your critical − the target's LUC × 0.034, held between 1 and 99 %): d × 1.5 + R2 × 1.4
type 2 skills: × the skill's value; 10 or less: 1 to 10; then the target's absorption
```

Defence is **subtracted in full**: a target whose defence comes near your attack takes little, and one whose defence passes it takes 1
to 10 a blow. The level gap does nothing here.

**Magic skills** first pass the resist roll ([Resisting magic](elements-defences.md#4-resisting-magic)), then:

```
P1 = (101 × Ltarget − 100 − 100 × Lyou) / (Lyou + Ltarget) × 2.4      held between 1 and 50: the level gap
P2 = target's magic defence / (your INT + your item attack) × 100      held between 1 and 70: the target's magic defence
d = E × (magic attack + the skill's value) × (1 − (P1 + P2) / 100) + R × 0.1
critical: d × 1.5 + R2 × 1.4
```

| target above you (at level 60) | 0 or below | +1 | +2 | +3 | +5 | +10 |
|---|---|---|---|---|---|---|
| P1 | 1 % | 1.2 % | 3.2 % | 5.1 % | 8.9 % | 17.9 % |

So magic meets magic defence as a **percentage of your own INT and weapon**: it can never take more than 70 % off, and magic defence above
70 % of the caster's INT + item attack does nothing more against that caster. More INT makes every target's magic defence worth less.

**What cuts through everything**: skills of damage types 0, 3, 4 and 5 skip defence, elements, the level gap and criticals; only the hit
roll stops them (most have a flat 70 %). The level-60 ultimate skills of four classes take **87 to 92 % of the target's maximum HP**
this way (Regina Power, Syeeric Touch, Pantera Vision, Kamain Breath, with cooldowns of 8 to 20 minutes). Two weapon lapis do **90 % and
100 % of the target's maximum HP** on any hit, stopped only by the magic resist roll, a magic block or the matching protection
([Lapis option skills](elements-defences.md#8-lapis-option-skills)).

**Reflection** on the target sends the whole blow back; **give up SP or MP** and **absorption** apply as against monsters; damage over
time from a player's debuff is not absorbed. Killing blows are credited to whoever did the most damage since the victim was last at
full HP, not to the last blow.

Source: `CUser::AttackUser` and `CUser::AttackDamage` ([Player kills 1](../original/pvp.md) sections 4 and 5),
skills on players ([Player kills 2](../original/pvp-2.md) sections 3, 5 and 6).

## 8. Experience and the level gap

```
experience = trunc(monster experience × ((Lmob − Lyou + 1) × 0.2 + 1))      0 when the monster is more than 9 levels above you
the killer alone: × 1.2
```

| monster minus you | −6 or less | −5 | −3 | −1 | 0 | +2 | +4 | +9 | +10 or more |
|---|---|---|---|---|---|---|---|---|---|
| factor | 0 | 0.2 | 0.6 | 1.0 | 1.2 | 1.6 | 2.0 | 3.0 | 0 |

The experience goes to whoever did the most damage to the monster (the party shares it, [Parties](../original/party.md)). A player kill
gives 50 × the victim's level × the same factor (at least 1, so a victim far below still gives 1). Death costs 5 % of the level's span, never below the level's start, nothing up to
level 10 ([Monsters fighting back](../original/mob-fight.md) section 8).

Source: `CMob::GetExp` and `CMob::Death`, [Fighting monsters](../original/combat.md) section 5; [Player kills 1](../original/pvp.md)
section 6.

## 9. Worked examples

All three use the fighter of [example A](character-numbers.md#13-worked-example-a-a-level-60-ultimate-fighter): level 60, melee hit 147,
evasion 149, attack 932 with a spread of 54, defence 165, critical 2, LUC 15, no element on the weapon. Each was worked by hand and checked
with the balance model, which follows the server's code; the ranges agree exactly.

### B: the fighter against a monster two levels above

**Monster 1811** (a level-62 young dragon of the original's data, placed 50 times): HP 301,624, DEX 440, LUC 201, defence 210, element
wind. Its numbers: melee hit 440 × 0.5 + 62 = 282, melee evasion 440 × 0.5 + 31 + 201 × 0.125 = 276.

```
hit = 160 × 60 / 122 − 100 × 282 / (149 + 282) + 100 × 147 / (276 + 147)
    = 78.69 − 65.43 + 34.75 = 48.0  → 48 %
P   = ((62 − 60 − 1) × 100 + 62) / 100 × 10 = 16.2
E   = no element against wind = 0.8
the lowest blow  (A = 932, R = 1):   932 × 0.838 × 0.8 − 210 × 1.162 + 0.1 = 624.8 − 244.0 + 0.1 = 380.9 → 380
the highest blow (A = 985, R = 15):  985 × 0.838 × 0.8 − 244.0 + 1.5        = 660.3 − 244.0 + 1.5 = 417.8 → 417
critical chance = 2 − 201 × 0.034 = −4.8 → the floor, 5 %
a critical: 572 to 644
```

So about **399 a landed blow**, one landed blow in two, every 2.75 s: around 70 damage a second, and more than an hour for this monster
alone (it is party content). The element matters more than it looks, because the defence is taken off after it: with E = 1.0 the
average blow would be about 560, and with a fire weapon (1.4 against wind, [Elements](elements-defences.md#2-elements)) about 880.

### C: the fighter against a defender (physical PvP)

A human **defender**, level 60, ultimate: base STR 210, DEX 9, REC 402, INT 10, WIS 10, LUC 14, the defender's **Daynorth** set
(defence 95, HP 1,825, each piece STR 2, DEX 1, REC 4), **Kazee's Teeth** and a **Bowen Shield** (defence 46, HP 968, REC 35). Totals
STR 244, DEX 22, REC 473; **defence 473 + 95 + 46 = 614**, hit 71, evasion 72, HP 11,692.

```
hit = 160 × 60 / 120 − 100 × 71 / (149 + 71) + 100 × 147 / (72 + 147) = 80 − 32.3 + 67.1 = 114.8 → held at 99 %
d   = 932 − 614 + a roll of 0 to 53  = 318 to 371
E   = no element against no element = 1.0
+ trunc(R × 0.1), R 1 to 15          = + 0 or 1
a normal blow: 318 to 372, about 345
critical chance = 2 − 14 × 0.034 = 1.5 → 1 %; a critical 478 to 578
```

The same attack that does about 399 to monster 1811 does about 345 here, and the defender's 11,692 HP take some 34 blows (over
1.5 minutes). Every point of defence takes one point off each blow: with 932 defence the fighter would do 1 to 10.

### D: a mage against a priest (magic PvP)

An elf **mage**, level 60, ultimate: base INT 499, WIS 118, LUC 9, **Draconic Staff** (attack 174, spread 34, INT 25, WIS 25) and the
mage's **Ophiston** set (each piece INT 4, WIS 2, LUC 1). Totals INT 544, WIS 153, LUC 14; magic attack 544 × 1.3 + 153 × 0.2 + 174 =
**911**; critical 2. A human **priest**, level 60, ultimate: base WIS 392, REC 222, **Tiamat** (WIS 48) and the priest's **Ophiston** set
(each piece WIS 4, magic defence 7 + 35 + 28 + 13 + 17 = 100 in all). Totals WIS 460, LUC 17; **magic defence 460 + 100 = 560**; HP 5,331.
The spell: **Flame Choir Lv3** (damage type 1, + 2,108, fire, 3 s cast).

```
resist: a = ((60 − 60 − 2) × 100 + 60) / 120 × 1.1 + 0 = −1.3 → held at 1
        b = (460 × 11 − 153 × 10) / (153 + 460) × 3.9 + 0 = 5.76 × 3.9 = 22.5
        the priest resists 23 % of the time
P1 = (101 × 60 − 100 − 100 × 60) / 120 × 2.4 = −0.8 → held at 1
P2 = 560 / (544 + 174) × 100 = 78.0 → held at 70
E  = fire against no element = 1.2
d  = 1.2 × (911 + 2,108) × (1 − 71 / 100) = 3,622.8 × 0.29 = 1,050.6, + R × 0.1 (R 1 to 14)
a landed spell: 1,050 to 1,052; critical chance 2 − 17 × 0.034 = 1.4 → 1 %, a critical 1,577 to 1,597
```

The priest's magic defence is already past the 70 % cap against this mage, so more of it would change nothing; more INT on the mage
would. Six landed spells would kill the priest; with the spell's 60 s cooldown that is a fight of other skills in between.

## Shaiya Afterglow

The server uses these formulas as the original does. It computes in double precision and rounds to single precision where the original
stores a number, so results agree except, rarely, by one at an exact boundary. Combat differences of its own (fixes of the original's bugs
and the settings that change gameplay) are listed in [Deviations](../server/deviations.md).
