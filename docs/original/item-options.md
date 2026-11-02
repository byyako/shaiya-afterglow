# Item option skills: lapis skills on hits

This page covers lapis that carry a skill in the EP4.5 game server `ps_game.exe`: which lapis carry one, when a
weapon's lapis strike the target of a hit and an armour's lapis strike back at the attacker, what each such skill does, and the guard
against one option starting another. It was read in the disassembly with the PDB names: `CUser::AttackItemOption(CUser*)` 0x44c630,
`CUser::AttackItemOption(CMob*)` 0x44c720, `CUser::BeAttackedItemOption(CUser*)` 0x44c970, `CUser::BeAttackedItemOption(CMob*)` 0x44cae0,
`CUser::AttackItemUser` 0x44c910, `CUser::AttackItemMob` 0x44c940, `CMob::IsBothmob` 0x44c610, and their callers.

Not covered here: item skills used from the bags on a target (`CUser::ItemUse` kinds 1 and 2; see [Using items](item-use.md);
they pass `option` 0 and start no option skills).

## In short

- **Some lapis cast a skill on every hit.** A weapon lapis with a skill tries it on whatever the wearer hits: sleep, stun, poison, and
  even near death and death (90 % or 100 % of the target's maximum HP).
- **Armour lapis strike back.** Upper-armour lapis with a skill both protect the wearer against that state and put it on whoever hits
  the wearer.
- **No extra roll.** Every hit tries every such lapis; only the skill's own hit-or-miss decides. The fast debuff lapis hit 7 to 13 % of
  the time.
- **A quirk against players.** When a player hits the wearer, the armour check stops at the first empty slot, starting with the shield
  and then the helmet. So a player without a shield and a helmet never strikes back at players with their armour lapis; against
  monsters it works regardless.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (see [NPC shops](shop.md));
`stItemInfo`, `stSkillInfo` and `CItem` as on [Lapis and enchanting](lapis.md), [Skills 1](skills.md) and
[Dropping items](item-drop.md). The functions use register conventions of their own (the compiler's); who is the owner and who
the target below was read from the code, not from the PDB signatures.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the disassembly (`objdump`, names from `docs/original/symbols/ps_game.tsv`):
`CUser::AttackItemOption(CUser*)` 0x44c630, `CUser::AttackItemOption(CMob*)` 0x44c720, `CUser::BeAttackedItemOption(CUser*)` 0x44c970,
`CUser::BeAttackedItemOption(CMob*)` 0x44cae0, `CUser::AttackItemUser` 0x44c910, `CUser::AttackItemMob` 0x44c940, `CMob::IsBothmob`
0x44c610, and their callers. `CUser` offsets are the code's (`docs/original/shop.md`); `stItemInfo`, `stSkillInfo` and `CItem` as in
`docs/original/lapis.md`, `docs/original/skills.md` and `docs/original/item-drop.md`. The functions use register conventions of their
own (the compiler's); who is the owner and who the target below was read from the code, not from the PDB signatures.

Covered: which lapis carry a skill, when a weapon's lapis strike the target of a hit and an armour's lapis strike back at the attacker, what
each such skill does, the guard against one option starting another. **Not covered:** scripted monsters (`CMob::IsBothmob`, section 6;
its name swap not built), item skills used from the bags on a target (`CUser::ItemUse` kinds 1 and 2, `docs/original/item-use.md`; built
since, `docs/original/small-items.md` section 11: they pass `option` 0 and start no option skills).

## 1. The data

Confirmed, `defs.items`, `defs.skills`.

A lapis' **`special`** (`bySpecial`, item `+0x46`) is a **skill id at level 100** (`CGameData::GetSkillInfo(special, 100)`: the second
table, `g_GameData + 0xa81820`, count at `+0xa8181c`, the addresses 0xfcd2c8 and 0xfcd2c4 in these functions). A gem counts when:
the gem byte is 1 to 255 and names a lapis (type 30) whose definition exists (`byType` and `byTypeID` not 0); its special is **not 0 and
not 16** (16 is the recovery lapis, `docs/original/lapis.md` section 2; no lapis has it); the special is below the level-100 table's
count and that row's `wSkillID` is not 0. There is **no chance roll of the option itself**: every hit tries every gem; the skill's own
hit-or-miss decides.

The lapis with a special (56 rows):

| lapis (type 30) | fits | special (skill at level 100) |
|---|---|---|
| 110, 112, 114, 116, 118, 120 | weapon | 1 to 6 (sleep, stun, silence, blindness, hold, slow) |
| 136, 138 (grade 99) | weapon | 7, 8 (near death, death) |
| 126, 128, 130, 122, 124, 134, 132 | weapon | 9 to 15 (poison, disease, confusion, curse, fear, dullness, misfortune) |
| 151 to 176 | weapon | 205, 206, 208, 209, 211, 212, 214, 215, 217, 218, 220, 221, 223, 224, 226, 227, 229, 230, 232, 233, 235, 236, 238, 239, 241, 242 |
| 111, 113, 115, …, 139 (odd, grade 38 to 98) | upper armour | 1 to 15 (the same skills) |

So **a weapon lapis strikes the target with its skill** on the hits of section 4, and **an upper-armour lapis strikes the attacker with the
same kind of skill** when its wearer is hit: the armour lapis 111 to 139 also block their state (`docs/original/lapis.md` section 2), and
on top of that put it on whoever hits the wearer.

The skills (level 100):

| skills | `typeeffect` | `wType` | attack kind | hit chance | what |
|---|---|---|---|---|---|
| 1 to 6, 9 to 15 | 3 debuff | 61 to 67 | magic (column 3) | success type 0: the magic resist roll | the debuff for `keeptime` (5 to 120 s) |
| 7, 8 | 1 attack | 21 | magic | success type 0 | one hit of **damage type 4**: 90 % or 100 % of the target's maximum HP |
| 205 to 242 (the 26 above) | 3 debuff | 61 to 66 | melee (column 0) | **success type 1: 7 to 13 %** | the debuff for 3 to 60 s |

Skills 207, 210, … (every third, `typeeffect` 0) are not named by any lapis. Any other `typeeffect` does nothing (section 3).

## 2. The weapon: `AttackItemOption`

Confirmed, 0x44c630 on a player, 0x44c720 on a monster.

Owner: the player who hit; target: the player or monster it hit. Nothing happens when:

- the owner's **option guard** is set (`+0x57b4`, section 5);
- the target is dead (a player's `+0x142c` = 1; a monster's status 2, `CMob::GetStatus`);
- the owner is dead (`+0x142c` = 1);
- the owner has **no weapon** (equipment slot 5, `+0x1d8`).

Otherwise the guard is set, the **weapon's six gems are taken in order** (`m_byGem[0..5]`) and each gem that counts (section 1) has its
skill go off on the target (section 3); then the guard is cleared. The target's death is not checked again between gems.

## 3. What a gem's skill does

Confirmed, `AttackItemUser` 0x44c910, `AttackItemMob` 0x44c940, and the copies inlined in the four functions.

By the skill's **`byTypeEffect`** (`+0x31`):

- **1 (attack)**: `CUser::SkillAttackSelect(target, skill)`, the one-target attack of a skill used by hand
  (`docs/original/skills-use.md` section 8 for a monster, `docs/original/pvp-2.md` section 2 for a player): by `wType`, one hit `single`
  (21, 24 to 27), the multi-hit (22) or the desperate counterattack (23); only 21 occurs.
- **3 (debuff)**: **one hit `single`** (`CUser::AttackMob(monster, skill, 1, 1)` or `CUser::AttackUser(player, skill, 1, 1)`):
  hit or miss, the packet, the debuff that stays (`docs/original/skills-use.md` section 8, `docs/original/pvp-2.md` section 3).
- **anything else**: nothing.

These are the hits of skills used by hand, with the owner as the caster, **without** the cooldown, the costs, the casting or the checks
of `UseSkillMob`/`UseSkillUser` (no range, no weapon kind, no country check: the target is the one already being fought). The packets are
the usual ones with the lapis skill's id and **level 100**: `0x0517`/`0x0518`/`0x051A` on a monster, `0x0511`/`0x0512`/`0x0514`/`0x0515`
on a player, and `0x0517`/`0x0511` result 4 before a multi-hit. A kill is the owner's (experience, drops, the killer of record) as with
any skill. Both hit functions are called with `option` 1, so a gem's skill can start option skills in turn (section 5).

## 4. The armour: `BeAttackedItemOption`

Confirmed, 0x44c970 by a player, 0x44cae0 by a monster.

Owner: the player who was hit; target: the player or monster that hit it. The equipment slots are taken in this order: **6 (shield), 0
(helmet), 1 (upper armour), 3 (gloves), 2 (lower armour), 4 (boots)**; in each, the six gems in order; each gem that counts has its skill
go off on the attacker as in section 3. Nothing happens when:

- **hit by a player**: the owner's guard is set, **the attacker is dead**, or **the owner is dead**;
- **hit by a monster**: the owner's guard is set, **the monster is dead** (status 2), or **the owner is dead**.

**The two versions differ on an empty slot**: hit by a monster, an empty slot is skipped and the next one is looked at; **hit by a player,
the first empty slot ends the scan** (the slot index register is overwritten before the test and the code leaves the loop). Since the
shield comes first, **a player without a shield never strikes back at players**, and one without a helmet neither (the lapis with a
special fit only the upper armour). Deviation 1 (section 7).

## 5. Where they run (verified, the callers) and the guard

| the hit | then |
|---|---|
| a player's **normal blow on a monster** (`CUser::AttackMob` 0x44ce10, `docs/original/combat.md` section 3) | when it hit, **did not kill** and the player lives: `SetTarget`, then **`AttackItemOption(monster)`** |
| a player's **normal blow on a player** (`CUser::AttackUser` 0x456870, `docs/original/pvp.md` section 4) | when it hit (also when reflected), after `0x0502` and the reflection: **`AttackItemOption(target)`**, then **`target.BeAttackedItemOption(attacker)`** |
| **a skill hit on a monster** (`CUser::AttackMob(monster, skill, single, option)` 0x44fd90) | when it hit, at the end (after the debuff), with `option`: **`AttackItemOption(monster)`** |
| **a skill hit on a player** (`CUser::AttackUser(player, skill, single, option)` 0x44f4c0) | when it hit, at the end, with `option`: **`AttackItemOption(target)`**, then **`target.BeAttackedItemOption(caster)`** |
| a **monster's normal blow** on a player (`CMob::Attack` 0x4a2000, `docs/original/mob-fight.md` section 6) | when it hit (also when reflected), after `0x0605`: **`player.BeAttackedItemOption(monster)`** |
| a **monster's skill hit** on a player (`CMob::AttackUser` 0x4a3fe0, `docs/original/mob-skills.md` section 5) | when it hit, at the end (step 9): **`player.BeAttackedItemOption(monster)`** |

A miss never starts them. **`option` is 1 in every skill hit** (single, area, multi-hit, debuff, by monsters or players; the callers of
0x44fd90 and 0x44f4c0 all push 1) **except `CUser::ItemUse`** (0). So every target of an area skill gets the caster's weapon options, and
every player hit by it its own armour options.

**The guard** (`+0x57b4`, one byte per player) is set while that player's weapon or armour options run, and makes any further
`AttackItemOption` or `BeAttackedItemOption` of **that player as the owner** do nothing. A gem's skill hit runs with `option` 1, so:
the owner's own options are blocked (no chain on itself); the other side's still run. Example: A hits B; B's armour lapis strikes A; that
skill hit runs `A.BeAttackedItemOption(B)`, A's guard is not set, so A's armour lapis strike B; that hit's `B.BeAttackedItemOption(A)`
finds B's guard set and stops. The depth is bounded by the two guards.

## 6. Scripted monsters

Confirmed, `CMob::IsBothmob` 0x44c610; not built.

A monster with `m_bLuaAI` (`+0x284`) or whose spawn entry (`m_pRespawn`, `+0xc84`) has its byte `+0x1c` set is "both": while a gem's skill
hits it, its `m_szPrevAttackSkill` (`+0xd14`) holds the lapis skill's name and is put back afterwards (`m_szPrevAttackUser` `+0xcf4` is
saved and restored too in the armour version). Only the monster scripts read these names. Scripted monsters are built
(`docs/original/scripted-mobs.md`, the Lua ones `docs/original/instances.md` section 18), but nothing there reads these names, so the swap is
not built (`docs/client/scripted-monsters.md`).

## 7. The new server

Built as above, with these deviations (both approved):

1. **Hit by a player, an empty armour slot is skipped** as when hit by a monster (approved): an equipped lapis always counts, whatever
   else is worn. In the original the first empty slot of shield, helmet, upper armour ends the scan, so the upper armour's lapis never
   strike back at a player unless a shield and a helmet are worn.
2. **A gem's skill aimed at a target that has died meanwhile** (killed by an earlier gem's skill of the same scan) **does nothing**
   (approved). In the original the hit is still worked out on the dead target: the roll, the packet with its damage, no damage taken.

How it is built: each player's counting gems are worked out with its numbers when its equipment changes (the weapon's six skill ids,
the armour's in slot order); the zone keeps the level-100 skills and runs a gem's skill through the same code as a skill used by hand,
with the owner as caster. The packets of a player's own lapis skills reach it in order through its connection's queue, except when its own
skill or blow on a monster set them off: then they follow that skill's or blow's packets, ahead of anything queued for it meanwhile (an
armour lapis striking back at it during an area skill shows after the whole skill, its cost included). A critical hit of a gem's skill would wear the weapon only when its owner
is the one attacking (no lapis skill can be critical: types 0 and 4 never are).
