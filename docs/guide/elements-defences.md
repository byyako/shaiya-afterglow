# Elements, resistances and defences

What stands between a blow and your HP in the original game: elements, defence and magic defence, the magic resist rolls, resisting
debuffs, absorption, blocks and reflection, and the lapis that cast skills on hits. The words used here are explained on
[Your character's numbers](character-numbers.md#1-words-this-guide-uses); the damage formulas are on [Hitting and being hit](hitting.md).

## In short

- **Four elements in a circle**: fire beats wind, wind beats earth, earth beats water, water beats fire (× 1.4 one way, × 0.5 the other).
  Each has a strong form that hits harder. Your weapon's element attacks; your **upper armour's** element defends.
- **Having no element is a weakness**: an attack with no element does 20 % less to a target with one, and an element attacking a target
  with none does 20 % more.
- **Against monsters, any element on your upper armour cuts every normal monster blow by 20 %** (30 % for a strong form), whatever the
  monster's own element. It is the cheapest defence in the game.
- **Defence is subtracted; magic defence is a percentage** against players. Every point of defence takes a point off each physical blow;
  magic defence cuts a share of a spell, at most 70 %, measured against the caster's own INT and weapon.
- **Magic is resisted, not dodged.** A magic skill rolls against the target's level, WIS and magic evasion; between players of equal level
  the target's WIS decides most of it.
- **Nothing gives a general resistance to debuffs.** A debuff is stopped by its hit roll, a block, a one-off protection item, or a lapis
  that blocks that state.
- **Absorption** takes a flat amount off every blow you take, but always leaves at least 1.

## 1. What comes between a blow and your HP

In the order the game applies them to a blow on a player:

1. **Block** (a buff): the blow becomes a miss.
2. The **hit roll** (or for magic, the **resist roll**); for a debuff, the **state protections** first.
3. **Reflection** (a buff), decided before the damage: the attacker takes the blow instead.
4. The **damage formula**, with defence or magic defence and the **element**.
5. **Give up SP or MP** (a buff): the HP damage is paid from SP, MP or both first.
6. **Absorption**: a flat amount off, at least 1 left.

## 2. Elements

There are nine element values: 0 none, 1 fire, 2 water, 3 earth, 4 wind, and 5 to 8 the strong forms of the same four. The factor, rows
the attacker's element, columns the defender's:

| attacking ↓ / defending → | none | fire | water | earth | wind | strong fire | strong water | strong earth | strong wind |
|---|---|---|---|---|---|---|---|---|---|
| **none** | 1.0 | 0.8 | 0.8 | 0.8 | 0.8 | 0.7 | 0.7 | 0.7 | 0.7 |
| **fire** | 1.2 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.4 | 1.0 | 1.3 |
| **water** | 1.2 | 1.4 | 1.0 | 0.5 | 1.0 | 1.3 | 1.0 | 0.4 | 1.0 |
| **earth** | 1.2 | 1.0 | 1.4 | 1.0 | 0.5 | 1.0 | 1.3 | 1.0 | 0.4 |
| **wind** | 1.2 | 0.5 | 1.0 | 1.4 | 1.0 | 0.4 | 1.0 | 1.3 | 1.0 |
| **strong fire** | 1.3 | 1.0 | 0.5 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 |
| **strong water** | 1.3 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.5 | 1.0 |
| **strong earth** | 1.3 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.5 |
| **strong wind** | 1.3 | 0.5 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 |

In words: an element against the one it beats is × 1.4 (strong form × 1.6, against a strong form × 1.3 or 1.4); against the one that
beats it × 0.5 (× 0.4 against a strong form); against itself and its opposite (fire and earth, water and wind) × 1.0. Any element against a
target without one is × 1.2 (strong × 1.3); no element against any element × 0.8 (× 0.7 against a strong form).

**Your attacking element**:

1. the weapon's own element;
2. then its lapis in order: a weapon without an element takes the first element lapis' element; a lapis of the same element changes
   nothing; **a lapis of a different element cancels the element to none** (and the rest are not looked at);
3. a skill with its own element uses that instead; a few skills ignore the target's element (they read the "none" column);
4. an **element buff** (Flame, Storm, Earth and Surgy Weapon: fire, wind, earth, water for 10 minutes) replaces the weapon's; only one
   at a time, the newest.

**Your defending element** is your **upper armour's**, worked out the same way from the armour and the lapis linked in it (Blaze-light,
Water-light, Earth-panic and Wind-scroll Lapis give the four elements, their Lv2 forms the strong ones). A debuff of "no element" takes
it away while it lasts.

**Against monsters**:

- Your element counts **on attack**, against the monster's element.
- A monster's **normal blows** read only the table's first row against your armour: × 1.0 with no element, **× 0.8 with any of the four,
  × 0.7 with a strong form**. The monster's own element does not count. So any element on the upper armour cuts every normal monster blow
  by 20 or 30 % ([example E](#9-worked-example-e-a-monster-blow-on-the-fighter)).
- A monster's **skills** use their own element against your armour's (the full table).

**Between players**, your attacking element meets the target's upper armour element (the full table), for physical blows and spells alike.

Source: `CGameData::m_AttackAttr` ([Fighting monsters](../original/combat.md) section 3), `CUser::SetAttrAtk` and
`CUser::SetAttrDef` ([Lapis and enchanting](../original/lapis.md) section 2), the element buffs ([Skills 3](../original/skills-buffs.md)
section 5.1), monster blows ([Monsters fighting back](../original/mob-fight.md) section 6, [Monster skills](../original/mob-skills.md)
section 7).

## 3. Defence against magic defence

| | defence (melee, ranged) | magic defence |
|---|---|---|
| from | REC 1 a point, armour, shield, lapis, buffs, the faction bonus | WIS 1 a point, armour, lapis, buffs, the faction bonus |
| a monster's blow on you | **subtracted**, less the level factor: D − D × P / 100 | (monster magic skills) **subtracted** the same way |
| your blow on a monster | its defence **subtracted, plus** the level factor: D + D × P / 100 | its magic defence the same |
| a player's blow on you | **subtracted in full**, no level factor | **a percentage cut** of the spell: your magic defence / (the caster's INT + item attack), at most 70 % |

What one point is worth against players:

- **A point of defence** is one point off every physical blow, whatever the attacker's attack. Against an attack of 932 (the fighter of
  [example A](character-numbers.md#13-worked-example-a-a-level-60-ultimate-fighter)), 614 defence leaves about 345 a blow and 932 leaves
  1 to 10.
- **A point of magic defence** is 100 / (the caster's INT + item attack) % off that caster's spells, up to the 70 % cap. Against the mage of
  [example D](hitting.md#d-a-mage-against-a-priest-magic-pvp) (INT 544 + staff 174 = 718) each point is 0.14 %, and anything above 503
  (70 % of 718) does nothing more against that mage. Against a weaker caster the cap comes sooner.
- The level gap adds its own cut to spells (P1, 1 % at equal level, about 1.8 % a level above at level 60).

Source: [Hitting and being hit](hitting.md) sections 4, 6 and 7, with their notes.

## 4. Resisting magic

A magic skill (and a lapis option skill, which is magic) **misses when a roll of 0 to 99 is below the resist rate**:

```
your spell on a monster:
  a = ((Lmob − Lyou − 2) × 100 + Lmob) / (Lyou + Lmob) × 1.1 + the monster's magic evasion        held between 1 and 70
  b = (monster WIS × 11 − your WIS × 10) / (your WIS + monster WIS) × 3.9 + your magic hit         held between 1 and 70
  resist = trunc(a + b), held between 1 and 90 %

a monster's spell on you: the same with the sides swapped
  a from the level gap seen from you and your magic evasion; b from your WIS × 11 against the monster's WIS × 10, + the monster's magic hit

a player's spell on a player: the same as on a monster, with the target player's level, WIS and magic evasion
```

- **a** is the level gap: 1 % (the floor) for a target up to two levels above you or any level below; above that it grows by about 0.8 %
  a level (3.2 % at five levels above, 7.4 % at ten, at level 60).
- **b** is the WIS contest: the target's WIS counts 11 against the caster's 10, so at equal WIS the target has the edge (b = 1 / 2 × 3.9
  ≈ 2 %). A caster with a tenth more WIS than the target puts b at its floor, 1 %; a target with much more WIS climbs towards 70 %.
- **Magic hit** (the caster) and **magic evasion** (the target) are added straight to b and a; they come only from buffs, passives and the
  faction bonus (grade 30 gives +30 magic evasion).
- Examples: the mage of [example D](hitting.md#d-a-mage-against-a-priest-magic-pvp) (WIS 153) is resisted **23 %** of the time by the priest
  (WIS 460) and **15 %** by monster 1811 (level 62, WIS 288); against a monster of its level with less WIS, 2 %.

Source: `CUser::AttackSuccess` ([Skills 2](../original/skills-use.md) section 11), `CMob::AttackSuccess`
([Monster skills](../original/mob-skills.md) section 7), the player version ([Player kills 2](../original/pvp-2.md) section 5).

## 5. Resisting debuffs and states

Every debuff (and the two lapis attacks of near death and death) carries one of **15 states**:

| state | what it does | state | what it does |
|---|---|---|---|
| 1 sleep | no attacks or moves; a hit wakes | 9 poison | damage over time (HP, SP or MP) |
| 2 stun | no attacks or moves | 10 illness | damage over time (HP, SP or MP) |
| 3 silence | no magic | 11 delusion | damage over time (HP, SP or MP) |
| 4 darkness | no melee | 12 doom | lower melee attack |
| 5 pause | no moves | 13 fear | lower melee defence |
| 6 slow | slower attacks | 14 dull | lower DEX |
| 7 near death | a blow of 90 % of maximum HP | 15 bad luck | lower LUC |
| 8 death | a blow of 100 % of maximum HP | | |

(These are the states of the lapis skills; other debuffs use the same numbers, and some skills carry no state at all.)

**Monsters** resist by their data: each monster lists which of the 15 states it resists and up to six skills it ignores; a skill of a
resisted state or a listed skill always misses it. Beyond that, the skill's hit roll decides.

**Players** have no resistance stat for debuffs. A debuff from a monster or a player reaches you unless, in this order:

1. you **block** its attack kind (a block buff, below);
2. it is **near death** or **death** and you carry the matching one-off protection (Scroll Protected Faint, Scroll Protected Death, or the
   cure items that set both): the debuff misses and the protection is used up;
3. you wear a **lapis that blocks its state** in any slot but the weapon (the Anti- lapis: Anti-Sleep, Anti-Stun, ..., Anti-Death,
   linked in the upper armour): it always misses;
4. its own **hit roll** fails: a flat chance (success type 1), the hit chance of melee and ranged skills, or the magic resist roll of
   section 4 for magic ones.

Once it lands, it lasts its time; a cure item or a dispel takes it off (a dispelled damage over time pays nothing more). Death clears
every debuff.

Source: monsters `CUser::AttackSuccess` ([Skills 2](../original/skills-use.md) section 11); players `CMob::AttackSuccess` ([Monster
skills](../original/mob-skills.md) section 7) and its player version ([Player kills 2](../original/pvp-2.md) section 5); protections
`CUser::ItemUseRemState` ([Using items](../original/item-use.md) section 5); lapis state blocks ([Lapis and enchanting](../original/lapis.md)
section 2).

## 6. Absorption

**Absorption** comes from lapis (the Absorption Lapis, 20 to 100 a stone) and from **armour enchant steps** (5 a step on every piece, see
[Enchanting](character-numbers.md#enchanting)). It takes a **flat amount off the HP damage of every blow** you take, after give up SP or
MP:

```
damage ≤ absorption: 1 is left
otherwise:           damage − absorption
```

The game tells you with a line "<n> absorption damage". It works against monsters and players alike, on normal blows and skills; damage
over time is not absorbed. Absorption matters most against many small blows; against a 1,500 blow 50 absorption is 3 %.

The walking guards hit with a different kind of blow, which absorbs only 1 point, and only of a blow of half your
maximum HP or more.

Source: `CUser::Damage` ([Lapis and enchanting](../original/lapis.md) section 2), the guards ([Walking NPCs and
guards](../original/guards.md) section 6).

## 7. Blocks, reflection and giving up SP or MP

These come only from buffs; **a shield has no block of its own** (it is a defence item).

| buff | skills | what it does |
|---|---|---|
| **block a count** | Magic Protector (2 to 4 spells), Magic Defy (2 to 5 spells) | the next N attacks of that kind are misses, then the buff ends |
| **block a chance** | Eagle Eye (ranged, 100 % for 15 s) | each attack of that kind is a miss at that chance |
| **block everything** | Untouchable, Power of Guard, Blank | every attack of every kind is a miss while it lasts |
| **reflection** | Melee Reflector (melee), Magic Mirror (magic), Dayfly Shield (all kinds, 40 s) | a blow that hit is turned back: you take nothing, the attacker takes the damage |
| **give up SP or MP** | Health Assistant | the HP damage of each blow is paid from SP (level 1), MP (level 2) or SP then MP (level 3) first |

- Blocks are checked before the hit roll, so a blocked blow is a plain miss; a block counts only for its attack kind (monsters' normal
  blows are melee).
- Reflection is decided after the hit and before the damage; a monster that resists the reflecting skill is not reflected. Against a
  player's damage over time, the tick is sent back to the caster; against a monster's, it is just cancelled.
- Give up SP or MP comes before absorption.

Source: `CUser::ChkBlock`, `CUser::ChkMirror` and `CUser::Damage` ([Skills 3](../original/skills-buffs.md) sections 5
and 10), [Player kills 1](../original/pvp.md) section 4; the shield ([Character numbers](../original/stats.md) section 2).

## 8. Lapis option skills

Some lapis cast a skill on every hit, without a roll of their own: the skill's own hit roll decides.

| lapis | where | the skill, on each landed hit |
|---|---|---|
| Sleep, Stun, Silence, Darkness, Pause, Slow, Poison, Illness, Delusion, Doom, Fear, Dull, Bad Luck Lapis | weapon | that debuff on the target, magic: stopped by the magic resist roll |
| **Dying Lapis, Death Lapis** | weapon | **one blow of 90 % / 100 % of the target's maximum HP**, magic: only a magic block, the state protections of section 5 and the resist roll stop it |
| Pain, Freezing, Blindside Lapis and their kind (Lv6, Lv7) | weapon | a short debuff (stun, pause, darkness and others, 3 to 60 s) at a flat **7 to 13 %**; the Anti- lapis of its state still stop it |
| the Anti- lapis (Anti-Sleep to Anti-Bad Luck) | upper armour | block their state on you, **and** cast that state's skill on whoever hits you |

- Every landed hit tries every such lapis on the weapon; every hit you take tries those on your armour. An area skill tries them on each
  target.
- Between equal players the magic resist roll is often low (2 % at equal WIS), so a Death Lapis lands on most hits it tries; the
  Anti-Death Lapis (or a Scroll Protected Death, once) is the counter.
- **A quirk against players**: when a player hits you, the armour check looks at the shield first, then the helmet, and stops at the
  first empty slot. Without a shield (and a helmet) your armour lapis never strike back at players. Against monsters they always do.

Source: [Item option skills](../original/item-options.md) sections 1 to 5 (`CUser::AttackItemOption`,
`CUser::BeAttackedItemOption`).

## 9. Worked example E: a monster blow on the fighter

The fighter of [example A](character-numbers.md#13-worked-example-a-a-level-60-ultimate-fighter) (level 60, melee defence 165, melee hit
147, evasion 149, LUC 15) is hit by **monster 1811** of [example B](hitting.md#b-the-fighter-against-a-monster-two-levels-above) (level 62,
normal blow 1,289 + a roll of 0 to 399, LUC 201, melee hit 282, evasion 276; its own element, wind, does not count).

```
hit  = 160 × 62 / 122 − 100 × 147 / (276 + 147) + 100 × 282 / (149 + 282)
     = 81.31 − 34.75 + 65.43 = 112.0 → held at 95 %
P    = 16.2 (two levels, as in example B, now in the monster's favour)
A    = 1,289 to 1,688;  D − D × P / 100 = 165 × 0.838 = 138.3;  R × 0.1 = 0.1 to 20.1
critical chance = trunc(201 × 0.19) − 15 × 0.034 = 38 − 0.5 = 37.5 → 37 %
```

| the fighter's upper armour | E | a normal blow | on average | a critical |
|---|---|---|---|---|
| Daynorth Armor with Life Lapis, no element | 1.0 | 1,289 × 1.162 − 138.3 + 0.1 = **1,359** to 1,688 × 1.162 − 138.3 + 20.1 = **1,843** | 1,601 | 2,041 to 3,091 |
| + **Water-light Lapis** (water) | 0.8 | **1,060** to **1,450** | 1,255 | 1,591 to 2,503 |
| + Water-light Lapis + **Absorption Lapis Lv5** (50) | 0.8 | 1,010 to 1,400 | 1,205 | 1,541 to 2,453 |
| + **Water-Light Lapis Lv2** instead (strong water) | 0.7 | 910 to 1,254 | 1,082 | 1,367 to 2,209 |

Counting the criticals, a landed blow averages about 1,960 without an element on the armour and 1,550 with one, so the fighter's
8,434 HP last about five landed blows without it and six with it; the element saves 346 a normal blow, the absorption another 50. The monster lands 95 % of its blows and more than a third of them as criticals, which is why the
fighter's defence (138 after the level factor) barely shows.

Each row was worked by hand and checked with the balance model, which follows the server's code: the ranges agree exactly and the
averages to the unit.

## Shaiya Afterglow

The server follows the original for everything on this page, with two fixes of the original's bugs:

- **Armour lapis strike back at players whatever else you wear**: an empty shield or helmet slot no longer ends the check (section 8).
- **Damage over time on players** is paid as on monsters, for the time it ran. The original adds instead of multiplying there, so on
  players a damage-over-time debuff did about one tick's damage at each payment however long it had run.

The full list: [Deviations](../server/deviations.md).
