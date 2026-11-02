# Monster skills: attacks, heals and debuffs used by monsters, and what debuffs do to players

This page covers the skills monsters use in the EP4.5 game server `ps_game.exe`: where they come from, how a monster
picks one, attacks, multi-hits, self-destruct, heals, the hit and damage formulas, the debuffs they leave on players, and how a monster's
hit breaks a player's cast, sleep or hiding. It comes from static analysis; nothing was executed. Monsters fighting with normal attacks are
on [Monsters fighting back](mob-fight.md), which this page completes; players' skills are on [Skills 1](skills.md) to
[Skills 4](skills-special.md). Scripted monsters (`AI` 10 and above) are on [Scripted monsters](scripted-mobs.md) (their
`UseMobSkill` calls `UseSkill` directly).

## In short

- **Monsters borrow the players' skill table.** Their skills are the rows at level 100 of the same table: sleep, stun, poison, area
  blasts and the like.
- **1,316 monsters** have at least one skill. Each skill has its own cooldown; some are used only when the monster's HP is low, and
  a few aim at the attacker with the lowest HP.
- **Area skills** hit every enemy player within range of the monster or of its target. Some monsters **self-destruct**: they hit, then
  die by their own hand (and give nothing when nobody had hurt them enough).
- **Healers** heal themselves or every monster of their side nearby.
- **Debuffs on players** lower stats, stun, stop, put to sleep, remove the armour's element, turn the player into a monster, or damage
  over time. One kills the player outright when it ends.
- **Any monster hit** breaks the player's cast, wakes them up, throws them off a mount, and ends hiding and most transformations.
- **Bug:** damage over time on players adds the elapsed time instead of multiplying by it, so a payment never grows with the time it
  covers.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s base
stats on); `CMob`, `stMOBInfo`, `stSkillInfo` and `CSkill` offsets match the PDB. "trunc" and "rand()" as on
[Fighting monsters](combat.md); ticks are milliseconds (`GetTickCount`).

## 1. The data

Confirmed.

- **There is no monster skill table.** `usp_Read_MobSkills_R` (`CGameData::LoadConstMobSkill` 0x4c0be0) reads the **`Skills` rows with
  `SkillLevel` 100**: 355 rows, already in `defs.skills`. `CGameData::GetSkillInfo(id, 100)` answers from them (part 1 section 1).
- A monster's attack entry with a skill (`AttackType` = `byType` = the skill id, part 1 of the fight document) gets `pInfo` = that level-100
  row at load (`CGameData::LoadConstMob` 0x4c1d6a). The entry keeps it only when the row exists and is **an attack (`byTypeEffect` 1), a
  debuff (3), or a heal (2) of `wType` 41 or 42**; otherwise the entry is cleared (`byType` and `byOK` 0, never used).
- In the data: **1,873 entries of 1,316 monsters** name a skill, all with a level-100 row; 20 of them have `byOK` 0 (never used) and 4 have
  `byOK` 6. Two are cleared at load (a hiding, kind 4 `wType` 88, and a kind-7 row). What is left, by kind and `wType`:

| kind | `wType` | what (sections) | target types used |
|---|---|---|---|
| 1 attack | 21 | one blow (5) | 3, 6, 7 |
| 1 | 22 | several blows, `byMultiAttack` (3) | 3, 6, 7 |
| 1 | 23 | **self-destruct**: a blow, then the monster dies (3, 4) | 6, 7 |
| 1 | 63 | nothing but the cast animation (skill 344) | 7 |
| 2 heal | 41 | heal itself or the monsters around (6) | 2, 6 |
| 3 debuff | 21, 25, 74 to 79 | a blow and an applied entry with no effect | 3, 6, 7 |
| 3 | 61 to 68, 90 | a blow and a debuff on the player (8) | 3, 6, 7 |

  Damage types of the attack rows used: 0 (4 entries), 1 (394), 2 (43), 3 (2), 4 (64), 5 (2), 6 (10). Six level-100 rows have add damage
  (`wAddDamage`), ten have `wType` 22 with `byMultiAttack` above 1, fourteen are heals.
- `byTypeAttack` is stored lowered by one (0 melee, 1 ranged, 2 magic, part 1 section 1); `byAttackRange` is at most 60.

## 2. Choosing a skill entry

Confirmed, `CMob::AttackAI` 0x4a1ab0.

As `docs/original/mob-fight.md` section 3 (entries from 2 down to 0, ready times, `byOK` conditions), with what skills change:

- **Taking a target** (`CMob::SetTarget`, fight document section 2) gives each entry with a skill a first ready time: 0 for entry 0 or a skill
  without reuse time, otherwise **`now + rand() % (wResetTime × 1000)`**.
- **`byOK` 6**: usable when the monster is taunted (`m_dwVisionFixUserID` set; then `ReTargetByAggro` runs as for the others) or when
  **`CMob::FindAttackUserMinHP`** (0x4a0810) finds someone: among the damage record's players in the zone, of another country, alive, seeing
  monsters, visible and attackable, the one with the lowest current HP. It then calls `SetTarget(that player, 1)`, which does nothing for a
  monster already fighting (state 1), so the entry goes against the current target. Otherwise the entry is skipped.
- For a used skill entry, `m_nCurrAttack = i` while it runs (it is the `byAttackType` of the packets) and 0 after.
- **A heal** (`byTypeEffect` 2), walking or not: `UseSkill` at once, no chase; then **entry ready time = `now + wResetTime × 1000`**.
- **Other skills, walking monster**: `CMob::Chase(now, player position, skill's byAttackRange)`. In range: `UseSkill` and the ready time
  as above. Not in range: the chase took its step; **if the chase dropped the target** (no target left), the ready time is set anyway.
- **Other skills, monster that does not walk**: `CMob::EnableAttackRange(player, skill's byAttackRange)` (horizontal distance below `bySize +
  range + 0.3`). In range: `UseSkill` and the ready time. Out of range: the ready time is set, and **the monster drops the target** (state 0
  unless dead, no target).

The skill's reuse time replaces the normal attack's `g_ConstMobAttackSpeed[speed] + dwTime` for its entry.

## 3. Using a skill

Confirmed, `CMob::UseSkill` 0x4a48e0, `CMob::SkillAttack` 0x4a4990, `CMob::SkillDebuff` 0x4a5090, `CMob::SkillHeal` 0x4a54e0.

`UseSkill(now, player)`: **nothing while asleep or stunned** (`m_nSleep`, `m_nStun`), or when the skill's attack kind is disabled for the monster
(`m_pAttack[byTypeAttack].nDisable`, part 3 section 12's abilities 32 to 34). (Its attack-speed 255 test can never be true: dead code.)
Otherwise by `byTypeEffect`: 1 `SkillAttack`, 2 `SkillHeal`, 3 `SkillDebuff`; then **`m_dwNextAttackTime = now + g_ConstMobAttackSpeed[attack
speed clamped 0..6] + 2500`** (the attack speed itself, not 255).

**`SkillAttack`** by `byTargetType`:

| target type | function | who |
|---|---|---|
| 3 | `SkillAttackSelect` 0x4a49d0 | the target |
| 6 | `SkillAttackRange` 0x4a4aa0 | the players around **the monster** |
| 7 | `SkillAttackRangeSelect` 0x4a4d90 | the players around **the target** |
| other | nothing | |

- **Target type 3**, by `wType`: **21, 24, 25, 26, 27**: one `AttackUser(skill, single)` on the target (section 5). **22**: the cast
  announcement (`SendUseSkill`, below, naming the target), then up to **`byMultiAttack`** times `AttackUser(skill, area)` on the target,
  stopping when it is dead. **23**: `AttackUser(skill, single)`, then, unless the monster died meanwhile (a reflection), **the monster dies by
  its own hand**: `CMob::Damage(monster, kind 2, its own id, damage = its current HP)` and `CMob::Death(2, its own id)` (section 4).
- **Target types 6 and 7**: the cast announcement first (naming the target for 7, nobody (0) for 6); then the players in the 5 × 5 cells around
  the centre's cell (the monster for 6, the target for 7) that are **alive, of another country than the monster** (`nCountry`; 2 matches no
  one), visible and attackable as staff, and at most **`byApplyRange`** from the centre (horizontal): by `wType` **21, 23, 24, 25, 26**: one
  `AttackUser(skill, area)`; **22**: up to `byMultiAttack` times, stopping when that player is dead; others: nothing. After the round, for
  **`wType` 23** the monster dies as above, but with kind 1 (`CMob::Damage(monster, kind 1, its own id, its HP)`, `CMob::Death(1, its id)`).

**`SkillDebuff`** by `byTargetType`: 3 one `AttackUser(skill, single)` on the target; 6 (`SkillDebuffRange` 0x4a50e0) the cast announcement
(nobody) and `AttackUser(skill, area)` for every player as above around the monster; 7 (`SkillDebuffRangeSelect` 0x4a52d0) the announcement
naming the target, then **the cells around the monster** but the distance measured **from the target** (an original mix-up; it only matters
for players more than two cells from the monster). No `wType` switch: every player passing the checks is hit once.

**`SkillHeal`**: target type 2 heals the monster itself (`HealUser(self, single)`); any other (6 in the data) heals the monsters around it
(section 6).

**The cast announcement** (`CMob::SendUseSkill` 0x4a3f50): **`0x060B`** with result **4**, the monster, the named player (or 0), `byAttackType`
= `m_nCurrAttack`, the skill id and level 100, damage 0.

All the monster skill packets go to **the players within 60 of the monster** (`CZone::PSendView` with range 60), **and always to the player
named in them** (`PSendView`'s extra id: the player hit, also beyond 60).

## 4. Self-destruct

Confirmed, `wType` 23, `CMob::Damage` 0x490cf0, `CMob::Death` 0x4910f0.

`CMob::Damage` with the monster's own id records **its remaining HP as damage it did to itself** in its damage record and kills it (state 2,
body time as any death: `now + 5 s` for respawn code 0, else `now + 10 s`). `CMob::Death` then gives the kill to the record's top damage dealer
(`docs/original/combat.md` section 6): when that is the monster itself, nobody gets experience or drops (only `0x0606`, naming the monster
itself as killer, kind 2 for target type 3 and kind 1 for 6 and 7); when the players had already done more damage than the HP it had left, the
top player gets the kill as usual.

## 5. One skill hit on a player

Confirmed, `CMob::AttackUser(player, skill, single)` 0x4a3fe0.

`single` is 1 for target-type-3 hits (and drains), 0 for multi-hits and area hits; it picks the packet: **`0x060B`** (single) or **`0x060D`**
(area).

1. **`CUser::StatusStand`**: a sitting player stands up (as for normal attacks).
2. **Hit or miss** (`CMob::AttackSuccess(kind = byTypeAttack, skill)`, section 7). **A miss**: `0x060B` (result 3, monster, player,
   `m_nCurrAttack`, skill, level, damage 0) or `0x060D` (result 3, monster, player, skill, level, 0); **nothing else** (no debuff).
3. **A hit**: **`CUser::StatusCastingStop`** (section 9): the player's cast is broken, it wakes up, its hiding or transformation (but
   Evolution) ends, a mount is left.
4. **Reflection**: `CUser::ChkMirror(kind)` (part 3 section 10) for the skill's attack kind, unless the monster resists the reflecting skill
   (one of its `pSkill` bytes equals the player's `m_pAttack[kind].nMirror`, the reflecting skill's id).
5. **Damage**: `CMob::AttackDamage(player, kind, skill)` (section 7) gives `damage[3]` and the result (0, or 1 for a critical).
6. **`wType` 27, drain** (single only; no monster uses it): unless reflected, `CUser::Damage(2, monster, damage)` (death below); the monster
   heals by the damage (`CMob::Heal`, section 6); **`0x060C`** (27 bytes: result, monster, player, `m_nCurrAttack`, skill, level, damage[3],
   heal[3]; both zero when reflected).
   **Others**: unless reflected, `CUser::Damage(2, monster, damage)`; then `0x060B` (result, monster, player, `m_nCurrAttack`, skill, level,
   damage[3]) or `0x060D` (result, monster, player, skill, level, damage[3]); damage zero when reflected. Then the **add damage**: when the
   skill has `wAddDamage` and the player was not just killed, **`CUser::Damage(2, monster, wAddDamage)`** (HP, SP, MP as they are; **also when
   the hit was reflected**) and **`0x060E`** (19 bytes: monster, player, skill, level, add damage[3]).
   `CUser::Damage` is the normal attack's (fight document section 6, step 5): nothing for a dead player or one with no HP; give-up SP/MP pays
   first; SP and MP lose their parts; **HP at or below the damage: the player dies** (`CMob::StatusAttackStop`: the monster drops its target;
   `CUser::Death(2, monster)`); else the low-HP passive check.
7. **Reflected** (and not scripted): if the player died meanwhile (from the add damage), skip to step 9. Otherwise the monster takes the
   damage from the player (`CMob::Damage(kind 1, player, damage)`; when it dies: the player stops attacking and `CMob::Death(1, player)`, a kill
   as usual) and **`0x051B`** (16 bytes: player, monster, damage[3]) goes to the players within 60 of the monster.
8. **A debuff** (`byTypeEffect` 3), unless the player died in this hit: **`CUser::AddApplySkill(skill, 2, monster id)`** (part 3 section 3;
   section 8 below). A refresh or a new entry sends `0x050D` (key, skill id, level 100) to the player.
9. `CUser::BeAttackedItemOption` (the armour's lapis skills, `docs/original/item-options.md`).

## 6. Monster heals

Confirmed, `CMob::HealUser` 0x4a5780, `CMob::SkillHealRange` 0x4a5520, `CMob::Heal` 0x4909e0.

- **Around** (`SkillHealRange`, after the cast announcement naming nobody): the monsters in the 5 × 5 cells around the caster that are **of the
  caster's country, or any when the caster's `nCountry` is 2**, not dead, and at most `byApplyRange` from it (horizontal): `HealUser(that
  monster, area)`. **The caster is among them.**
- **`HealUser`**, `wType` **41**: `heal = [wHeal[0] + the caster's definition WIS (only when wHeal[0] is not 0), wHeal[1], wHeal[2]]`; the
  monster healed: **`CMob::Heal`**: each value lowered so that HP, SP, MP do not pass the definition's maxima, then added; **reaching full HP
  clears its damage record** (`CDamageMob::Reset`). Then: single (itself) **`0x060B`** (result 0, the caster, player 0, `m_nCurrAttack`, skill,
  level, heal[3]); area **`0x060D`** (result 0, **the healed monster**, player 0, skill, level, heal[3]) around the healed monster. The client
  heals a monster by `0x060B` with a heal skill (`docs/client/client-world-packets.md`).
  `wType` **42** (dispel): `CMob::RemStateSkill` on the healed monster and the same packets with 0; no monster has one.

## 7. Hit and damage formulas

Confirmed, `CMob::AttackSuccess` 0x4a23f0, `CMob::AttackDamage` 0x4a3070, `CMob::CriticalSuccess` 0x4a28c0.

**Hit or miss** (`Lm`, `Lu` the monster's and the player's levels):

1. **`CUser::ChkBlock(kind)`** (part 3 section 10: block-all, or the newest block buff **of that attack kind**): blocked is a miss.
2. With a skill: `byStateType` 7 and the player's one-off protection `m_bBlockDying` set: the protection is used up (cleared) and it is a miss;
   `byStateType` 8 and `m_bBlockDeath`: the same. Then `byStateType` ≠ 0 and the player's **`m_pStateBlock[byStateType]`** ≠ 0: a miss.
   (`m_pStateBlock` comes from lapis, `CUser::GemEquipmentAdd`: `docs/original/lapis.md` section 2; the two protections from items, `CUser::ItemUseRemState`:
   `docs/original/item-use.md` section 5, built.)
3. By `bySuccessType`: **1** hit when `rand() % 100 < bySuccessValue`; **2** `bySuccessValue + 100 × trunc(m_nLuc / (wLuc + m_nLuc))` (the
   monster's current and definition LUC: in effect `bySuccessValue`), **3** the same with WIS; no monster skill uses 2 or 3.
4. Otherwise the attack kind's formula. **0 and 1**: the normal attack's rate (fight document section 6 step 2) with that kind's hit and
   evasion numbers. **2 (magic), the player resists**: `a = clamp(((Lu − Lm − 2) × 100 + Lu) / (Lu + Lm) × 1.1 + the player's magic evasion,
   1, 70)`, `b = clamp((player WIS × 11 − monster wWis × 10) / (player WIS + monster wWis) × 3.9 + the monster's magic hit, 1, 70)`, `rate =
   clamp(trunc(a + b), 1, 90)`; **a miss when `rand() % 100 < rate`**. (The player's magic evasion is `m_pAttack[2].nPerDefense`, the monster's
   magic hit `m_pAttack[2].nPerAttack`, 0 without debuffs.)

**Damage** (`damage[3]` from 0), by `byDamageType`:

| type | damage | critical, minimum |
|---|---|---|
| **0** | `wDamage` as it is | no, no |
| **1** | `damage = wDamage`, then the formula with **A = entry 0's `wDamage` + the monster's attack of that kind + `wDamage[0]`**; SP and MP keep `wDamage[1]`, `wDamage[2]` | yes, yes |
| **2** | the formula (A without the skill), then HP × `wDamage[0]` when that is not 0; SP and MP stay 0 | yes, yes |
| **3** | `wDamage[i]` × **the monster's own current** HP, SP, MP (each with `wDamage[i]` ≠ 0, at least 1), at most 30000 | no, no |
| **4** | `trunc(wDamage[i] / 100 × the player's maximum` HP, SP, MP`)`, at least 1, at most 30000 | no, no |
| **5** | the same with **the player's current** HP, SP, MP | no, no |
| 6 and above | the formula, A without the skill | yes, yes |

**The formula** (as the normal attack's, fight document section 6 step 4, with these differences): `P = clamp(((Lm − Lu − 1) × 100 + Lm) / 100 ×
10, 1, 95)`; **A = entry 0's `wDamage` + the monster's `m_pAttack[kind].nAttack` (+ `wDamage[0]` for type 1), plus `rand() % entry 0's
wDamageAdd`** when that is above 0 (always entry 0, whichever entry fired the skill); element **`E`**: `wType` 24 → `m_AttackAttr[byAttrType][0]`,
else `m_AttackAttr[byAttrType][player defence element]` when both are below 9, else 1; `R = rand() % LUC + 1` (monster LUC, 0 when 0); `D =`
the player's defence of that kind (`m_pAttack[kind].nDefense`: melee, ranged or magic defence); **`damage = (A + A / 100 × P) × E − (D − D / 100 ×
P) + R × 0.1`**, at least 0. **Critical**: chance `trunc(monster m_pAttack[kind].nCriticalAttack / 100 × 100 − player LUC × 0.034)` clamped
5..99; when `rand() % 100 <` chance: **`damage = damage × 1.5 + (rand() % LUC + 1) × 1.4 / 100 × P`** (0 for the random part when LUC is 0),
result 1. **Unlike the normal attack, there is no `+ R2` term.** Then `damage[0] = trunc(damage)`; each value at most 30000; **`damage[0]` at
most 10 becomes `rand() % 10 + 1`**.

## 8. Debuffs on players

Confirmed, `CUser::AddApplySkillDebuff` 0x4855b0, `CUser::RemApplySkillDebuff` 0x485720, `CUser::ChkKeepSkill` 0x487190.

Applied through `CUser::AddApplySkill` (part 3 section 3: map rule, same skill and level refreshes, `0x050D`), kept, saved and loaded like
buffs (part 3 sections 6, 8, 13; `CUser::InitApplySkill` re-applies the effects of loaded debuffs too). Their effects by `wType`:

| `wType` | applied | ends (time, death, dispel) |
|---|---|---|
| **61** | nothing; **every 3 s** (the three-second pass, part 4 section 7) the damage for the time since the last payment | the rest of it |
| **62, 67** | `CUser::SetSkillAbility(type, −value)` × 3 (abilities lowered, part 1's table) | the same with `+value` |
| **63** | nothing | **damage = the player's current HP** (it dies), as a damage-over-time payment below |
| **64** | `m_nStun += 1`, `SendSpeed` | `−= 1`, `SendSpeed` |
| **65** | `m_nStop += 1`, `SendSpeed` | `−= 1`, `SendSpeed` |
| **66** | `m_nSleep += 1`, `SendSpeed` | `−= 1`, `SendSpeed` |
| **68** | `m_nAttrDef = 0`: no defence element | `CUser::SetAttrDef` again when an upper armour is worn |
| **70** | `m_dwChaseID` = the attacker (no monster uses it) | 0 |
| **90** | transformation: `m_byShape = wChangeType`, `m_nShapeType = wChangeLevel`; for **101** `m_nTransFormFixMob = 1` and `SendSpeed`, otherwise `InitEquipment(false)` (part 4 section 5.4); `SendShape` | 101: `m_nTransFormFixMob = 0`, `SendSpeed`; otherwise `InitEquipment(true)`; shape 0; `SendShape` |
| 21, 25, 74 to 79 | nothing | nothing |

**A dispel** (`CUser::RemStateSkill`, part 3 section 9) takes debuffs off through **`CUser::CancleApplySkillDebuff`** (0x487ed0): the
table's undoing for 62 to 68, 70 and 90, **without the end payment of 61 and 63**. Death clears them likewise (part 3 section 8).

**Speeds** (`CUser::GetAttackSpeed` 0x48a460, `GetMoveSpeed` 0x48a4d0): attack speed **255** while asleep, stunned, disabled by a skill or
`m_nTransFormFixMob`; move speed **255** while stopped, asleep, stunned or disabled by a skill, **0** with `m_nTransFormFixMob`; otherwise the
values clamped (0..9, 0..4). `SendSpeed` is `0x051C` within 60 (`docs/original/stats.md`). At attack speed 255 the server refuses attacks and
skills (`docs/original/combat.md` section 1, part 2 section 6) and the attack tick stops attacking with `0x0502` result 10 (combat section 2);
moves are not checked by the server (the client holds still at move speed 255).

**A damage-over-time payment** (every 3 s for 61; at the end for 61 and 63): `CUser::TimeDamage(now − m_dwPrevTick)` for 61 (below),
`m_dwPrevTick = now`; the player's current HP for 63. Then, when the entry has an attacker and **`ChkMirror(the skill's attack kind)`** says
reflected: for a monster attacker that does not resist the reflecting skill, **nothing happens** but `0x050F` with zero damage (the damage is
not sent back to the monster); a monster that resists it, or no attacker: **`CUser::DamageByKeepSkill`** (0x44c3f0: nothing for a dead player
or one with no HP; give-up SP/MP; HP at or below: death by the entry's attacker, `CUser::Death(kind, id)`; else the low-HP passive check), then
**`0x050F`** (15 bytes: player, skill, level, damage[3]) within 60 (the tick) or to the player's view (the end). After a tick that hit a
living player: its hiding or transformation ends (`RemApplySkillToMOB`) and **it wakes up** (`RemApplySkillSleep`, section 9).

**`CUser::TimeDamage(elapsed)`** (0x486290): base values by `byTimeDamageType`: 0 `wTimeDamage`; 3 `wTimeDamage[i] ×` the player's current HP,
SP, MP; 4, 5 percentages of the player's maxima and current values (at least 1 each); then **`base + elapsed / 3000`** for each, at most 30000.
**An original bug** (`docs/original/community-fixes.md`): the monster version multiplies (`base × elapsed / 3000`), so on players a damage-over-time
debuff deals about `wTimeDamage` per payment however long it ran.

## 9. Being hit breaks casting, sleep and hiding

Confirmed, `CUser::StatusCastingStop` 0x489f10.

**Correction to `docs/original/mob-fight.md` section 6 and part 4 section 6**: **every monster hit** (a normal attack that is not a miss,
`CMob::Attack` 0x4a20de, and a skill hit, section 5 step 3) calls `CUser::StatusCastingStop(player)`:

1. a mount is left (mounts: built, `docs/original/vehicles.md` section 2, `zone::vehicle::casting_stop`);
2. `CUser::CancleMove`;
3. **`RemApplySkillToMOB`** (part 4 section 6): the first hiding or transformation ends, Evolution excepted;
4. **`CUser::RemApplySkillSleep`** (0x4893a0): when asleep, every applied `wType` 66 entry is taken off **without its end handling**
   (`0x050E` each, the database told), `m_nSleep = 0`, `SendSpeed`;
5. **a cast in progress (status 3) is broken**: the skill's target goes back to the previous one when it was oneself; with `m_nAttackMode` 0 the
   player stops (`StatusAttackStop`), else status 2 with normal attacks next and the next attack time now (as a step, part 2 section 5). Nothing
   is spent and nothing is sent.

So part 4's "being hit by a monster does not end hiding" is wrong: a monster already fighting a player who then hides ends the hiding with its
next hit. (Monsters do not take a hidden player as a new target.)

## 10. The packets

| op | size | layout | when |
|---|---|---|---|
| `0x060B` | 21 | `u8 result, u32 monster, u32 player, u8 attack entry, u16 skill, u8 level, u16 damage[3]` | single hit (0/1 hit, 3 miss); cast announcement (4); a monster healing itself (0, player 0, heal values) |
| `0x060C` | 27 | `0x060B` + `u16 heal[3]` | drain (no monster uses it) |
| `0x060D` | 20 | `u8 result, u32 monster, u32 player, u16 skill, u8 level, u16 damage[3]` | area and multi-hits; a monster healed by another (0, the healed monster, player 0) |
| `0x060E` | 19 | `u32 monster, u32 player, u16 skill, u8 level, u16 add damage[3]` | add damage |
| `0x051B` | 16 | `u32 player, u32 monster, u16 damage[3]` | a reflected skill hit (part 3) |
| `0x050D`, `0x050E` | 9, 6 | part 3 | a debuff applied / taken off (to the player) |
| `0x050F` | 15 | `u32 player, u16 skill, u8 level, u16 damage[3]` | damage over time, death sentence |
| `0x051C` | 8 | `u32 player, u8 attack speed, u8 move speed` | stun, stop, sleep, polymorph start and end |
| `0x0504`, `0x0606` | 11 | `docs/original/mob-fight.md`, `docs/original/combat.md` | a player or a monster killed |

All of them within 60 of the monster (and always to the player named), except `0x050D`/`0x050E` (the player) and `0x050F` (as section 8).

## 11. The new server

- **Definitions**: the skill definitions also load the level-100 rows; the monster attack entries keep a skill only as section 1 says.
- **The attack pass** (`fight_turn`) uses skill entries as section 2 (`byOK` 6 included, with the taunt and the damage record), with the
  first ready times of `SetTarget`, and carries out section 3: target-type-3, 6 and 7 attacks and debuffs, multi-hits, self-destruct, heals.
  A skill hit (section 5) uses the hit roll and damage of section 7, the players' blocks and reflection of every kind, give-up SP/MP, death,
  the low-HP passive, add damage, drains and the debuff.
- **Debuffs on players** go into the player's applied list (part 3) with the effects of section 8: abilities lowered, stun, stop, sleep,
  no element, polymorph, damage over time paid every 3 s and at the end, the death sentence. The connection's speeds follow section 8's rules.
- **Every monster hit** breaks a cast, wakes the player and ends hiding and transformations (section 9), normal attacks included.
- **Deviations**:
  - **damage over time on players multiplies** (`base × elapsed / 3000`, truncated, as on monsters) instead of the original's addition;
  - after a monster heals, the healed monster's HP also goes out as `0x0305` (absolute HP) within 60, so health bars stay right whatever the
    client does with `0x060D`;
  - lapis state resistance (section 7 step 2) was not built at first (only the item protections resisted a state); built
    (`docs/original/lapis.md` section 11: `state_blocks` in `zone::mob_skills::hit_player`);
  - a drain (`wType` 27; no monster has one) hits as a plain blow without healing the monster;
  - an area or multi-hit stops when a reflection kills the monster in the middle;
  - self-destruct's `0x0606` names the monster with killer kind 2 whatever the target type;
  - `0x050F` at the end of a damage over time goes within 60, as the tick (the original sends the end to the player's view);
  - mounts and parties were not covered here (item effects on being hit: `docs/original/item-options.md`); both are built (a hit gets
    the rider off, `docs/original/vehicles.md` section 5; the party sees the debuffs through the member status, `docs/original/party.md`
    section 13); a debuff reflected back to a player (player kills) is built too (`docs/original/pvp-2.md` section 11,
    `zone::mob_skills::keep_damage`).
- **Built in**: `crates/game-server/src/zone/mob_skills.rs` (sections 2 to 6, 8's payments and 9), `combat::mob_skill_hits` and
  `combat::mob_skill_damage` (section 7), `buffs::BuffEffects` (stun, stop, sleep, polymorph, no element) with the connection's speeds and
  refusals, `MobDefs::attach_skills` (section 1).
