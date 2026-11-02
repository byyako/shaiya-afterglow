# Player kills, part 2: skills on players, the faction kill count

This page covers skills used against players in the EP4.5 game server `ps_game.exe` — attack and debuff skills on
players (one target, around a player, around oneself, and the players inside the areas of skills aimed at monsters), their hit chance and
damage against a player (magic included), drain, additional damage, reflection, the lasting effect of a player's debuff and its damage
over time — and the faction kill count (`KillStatus.SData`, the counts, grades and their bonuses, the 10-minute timers, `0x020F` and
`0x0211`, the Goddess Blessing). It was read in the Ghidra decompiler and the disassembly with the PDB names.

[Part 1](pvp.md) has the normal attacks on players, the damage record, death by a player and the counters. Skills on monsters
are on [Skills 2](skills-use.md) (asking, checks, casting, costs) and [Skills 3](skills-buffs.md) (applied skills);
monster skills on players, including debuffs on players, are on [Monster skills](mob-skills.md).

Not covered here: duels and guild wars (`m_nPvPStatus`, `PvPChkLeaveByAttack`, the duel flag of the desperate counterattack's death; see
[Duels](duel.md) and [Guild wars](guild-war.md)); item option skills (`AttackItemOption`, `BeAttackedItemOption`; see
[Item option skills](item-options.md)); obelisks (`CMap::ObeliskDestroy`, `ObeliskReset`), which move and reset the counts too
(see [Obelisks](obelisks.md)); the operator tool's kill count commands (`CClientToMgr::OnRecv`); game logs.

## In short

- **Skills hit players much as they hit monsters,** with two differences: a player's magic defence and the level gap cut magic damage by
  a percentage, and every area skill also hits enemy players in range, whatever it was aimed at.
- **No skills on others on map 42.** There, the server ignores every skill request aimed at a player: no heals, buffs or one-off skills
  on anyone.
- **The faction kill count.** Every player kill adds one to the killer's faction count. Reaching set counts raises the faction's grade
  (30 grades), and each grade gives every player of that faction bonuses: more HP, MP and SP, recovery, critical, evasion, better lapis
  linking, and less experience lost on death.
- **The top grade lasts 10 minutes.** When a faction reaches it, the other faction's count is reset to 0; after 10 minutes the top
  faction drops back to grade 0 too. A Goddess Blessing raises its faction 5 grades at once.
- **The bonuses are applied inconsistently** in the original: some depend on whether the player logged in before or after the grade
  changed.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's: equal to the PDB's up to `stUserInfo`'s
map, 4 above them from the kill counts on (`m_wMap` `+0x164`, the kill counts `+0x14c`, the PvP block `+0x54cc`); `CWorld` offsets are the
code's, 0x40 above the PDB's (`m_KCInfo` `+0x1fc`). "rand()" is the C library's (0 to 32767); "trunc" the x87 store with the rounding
mode set to chop.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). `CUser` offsets are the code's: equal to the PDB's up to `stUserInfo`'s map, 4 above them from the kill counts
on (`m_wMap` `+0x164`, the kill counts `+0x14c`, the PvP block `+0x54cc`); `CWorld` offsets are the code's, 0x40 above the PDB's
(`m_KCInfo` `+0x1fc`). Part 1 (`docs/original/pvp.md`) has the normal attacks on players, the damage record, death by a player and the
counters; the skills on monsters are `docs/original/skills-use.md` (asking, checks, casting, costs) and `docs/original/skills-buffs.md`
(applied skills), monster skills on players `docs/original/mob-skills.md` (debuffs on players, section 8).

Covered: attack and debuff skills on players (one target, around a player, around oneself, and the players inside the areas of skills
aimed at monsters), their hit chance and damage against a player (magic included), drain, additional damage, reflection, the lasting
effect of a player's debuff and its damage over time; the faction kill count (`KillStatus.SData`, the counts, grades and their bonuses, the
10-minute timers, `0x020F` and `0x0211`, the Goddess Blessing).

**Not covered:** duels and guild wars (`m_nPvPStatus`, `PvPChkLeaveByAttack`, the duel flag of the desperate counterattack's death); item
option skills (`AttackItemOption`, `BeAttackedItemOption`); obelisks (`CMap::ObeliskDestroy`, `ObeliskReset`), which move and reset the
counts too; the operator tool's kill count commands (`CClientToMgr::OnRecv`); game logs.

"rand()" is the C library's (0 to 32767); "trunc" the x87 store with the rounding mode set to chop.

## 1. Asking for a skill on a player

Confirmed, `CUser::ChkTargetUser` 0x44e140.

`0x0511` (`u8 number, u32 target`) with a skill of target type **3** (one target) or **7** (around the target) of **kind 1 (attack) or 3
(debuff)**: the target must be in the caster's zone (`CZone::FindUser`), **not of the caster's side** (`IsTeam` false: part 1 section 1)
and pass `EnableAttackUser(wType)` (0x456800): alive (or `wType` 121 or 122), fully in the world (or the caster itself), not in shape 13
(hidden). Then the target is that player (kind 1); the previous target is not kept (it is not oneself). Anything else: `0x0511` result 11
as before.

**Correction to `docs/original/skills-use.md` section 3**: the first check of `ChkTargetUser` is **the caster's map** (`+0x164`, `m_wMap`;
not `m_wHG`) **is 42 and the zone's map has a war type** (`CMap::m_nWarType`, part 1 section 1). Map 42 has `WarType = P`, so **`0x0511` is
ignored without an answer on map 42**, whatever the skill: no heals, buffs or one-off skills on anyone there (the `0x0517` path to monsters
is not affected).

The tick (`CUser::UpdateAttack`, `docs/original/skills-use.md` section 4) checks a player target with `EnableAttackUser(wType)` again: in
the zone, alive, entered, not hidden; with a skill `wType` is not 0, so **the sides are not looked at again** (they cannot change for two
players in one zone). Then `UseSkillUser` (the checks, range by `EnableAttackRange(player, byAttackRange)`: no size; casting `0x0510`) as
for skills on oneself. When the skill does not kill and the caster attacks normally (`m_nAttackMode` 1), normal attacks go on against that
player (part 1 section 3).

## 2. What the skill does

Confirmed, `CUser::AttackUser(now, CUser*, CSkill*)` 0x451c80, `CUser::SkillAttack` 0x451d90, `CUser::SkillDebuff` 0x452bf0.

The cooldown, then by kind: **1** `SkillAttack`, **3** `SkillDebuff` (2, 4, 5, 6 as before), then the costs and `wDisable` as before. By
target type:

| | attack (`SkillAttack`) | debuff (`SkillDebuff`) |
|---|---|---|
| 3, one player | `SkillAttackSelect(player)` (below) | one hit, `single` |
| 6, around oneself | `SkillAttackRange` (section 4) | `SkillDebuffRange` (section 4) |
| 7, around a player | `SkillAttackRangeSelect(player)` (section 4) | `SkillDebuffRangeSelect(player)` (section 4) |

**`SkillAttackSelect(player)`** (0x451dd0), by `wType`:

| `wType` | what |
|---|---|
| 21, 24, 25, 26, 27 | one hit, `single` (27 is the drain) |
| 22 | **`0x0511` result 4** (caster, target, skill, values 0; `CUser::SendUseSkill` with the player form) to the players within 60, then **`byMultiAttack` hits, not single** (`0x0513` each), stopping when the target or the caster is dead |
| 23 | one hit, `single`; then, when the caster lives, `CUser::Damage(own HP)` and on death **`CUser::Death(1, own id)`** (the duel flag: `docs/original/duel.md` section 6, `docs/original/guild-war.md` section 6), as for monsters |

Every hit is **`CUser::AttackUser(target, skill, single, option = 1)`** (section 3).

## 3. One skill hit on a player

Confirmed, `CUser::AttackUser(CUser*, stSkillInfo*, bool single, bool option)` 0x44f4c0.

The attack kind is the skill's **`byTypeAttack`** (0 melee, 1 ranged, 2 magic). Packets by `single` and `wType`:

| | hit or miss | drain (`wType` 27, single only) | additional damage | reflected |
|---|---|---|---|---|
| single | **`0x0511`** (20 bytes) | **`0x0512`** (26 bytes: as `0x0511` + `u16 heal[3]`) | **`0x0514`** | **`0x0515`** |
| area | **`0x0513`** (20 bytes) | (no drain in areas) | `0x0514` | `0x0515` |

`0x0511`/`0x0513`: `u8 result, u32 caster, u32 target, u16 skill id, u8 level, u16 value[3]` (HP, SP, MP), to the players within 60 of the
caster (`CZone::PSendView`). `0x0514` (`TP_PC_SKILL_ADD`, 19 bytes: `u32 caster, u32 target, u16 skill, u8 level, u16 add[3]`) and
`0x0515` (16 bytes: `u32 target, u32 caster, u16 damage[3]`) to the caster's view (`CObjectConnection::PSendViewCombat`).

1. **The target stands up** (`CUser::StatusStand`).
2. **Hit or miss** (`CUser::AttackSuccess(target, kind, skill)` 0x456b10, section 5). A miss: the packet with **result 3** and values 0
   (`0x0512` for a drain, heals 0); single hits of **skill 56** (Charge) lower the caster's move-check allowance by its `byAttackRange` (as
   on monsters, not built there either). **Nothing else**: no debuff, no item options.
3. A hit: **`CUser::StatusCastingStop(target)`** (cast broken, mount left, hiding and transformation but Evolution ended, woken up:
   `docs/original/mob-skills.md` section 9); Charge as above.
4. **Reflection** (`CUser::ChkMirror(kind)` on the target), decided before the damage.
5. **Damage** (`CUser::AttackDamage(target, kind, skill)` 0x457030, section 6); a critical wears the caster's weapon (`QualityByAttack`).
6. **Drain**: unless reflected, `CUser::Damage(target, kind 1, caster, damage)` (part 1 section 4 step 6: the record, armour wear, SP and
   MP, give-up SP/MP, absorption; a kill: the caster stops attacking and **`CUser::Death(target, 1, caster)`**), then the caster heals by
   the damage (`CUser::Heal`: each of HP, SP, MP up to its maximum, the heals shrink to what fitted; **also after a kill**). `0x0512` with the
   result, the damage and the heals (all 0 when reflected).
   **Others**: unless reflected, `CUser::Damage(target, 1, caster, damage)` (a kill as above); the packet with the result (0, or 1
   critical) and the damage (0 when reflected). Then the **additional damage** when `wAddDamage` is not 0 and the target was not killed
   (**also when reflected**): `CUser::Damage(target, 1, caster, wAddDamage)` as it is (a kill as above), `0x0514`.
7. **Reflected**, and nothing died yet in this hit: `CUser::Damage(caster, 1, target, damage)`; when it kills, **the target** stops its
   attack and **`CUser::Death(caster, 1, target)`**; `0x0515`.
8. **A debuff** (`byTypeEffect` 3), **unless the target was killed by the hit or the additional damage** (a caster killed by the
   reflection does not stop it): **`CUser::AddApplySkill(target, skill, 1, caster)`** (`docs/original/skills-buffs.md` section 3: map
   rule, refresh, `0x050D`; its effects are those of `docs/original/mob-skills.md` section 8).
9. With `option` (always here): `AttackItemOption`, `BeAttackedItemOption` (`docs/original/item-options.md`).

A death in any of these steps goes through the victim's damage record (part 1 sections 5 and 6): experience, counts and losses for a
player killer of record.

## 4. Areas

Confirmed, `CUser::SkillAttackRange` 0x451f80, `SkillAttackRangeSelect(player)` 0x452560, `SkillDebuffRange(player)` 0x452c50, `SkillDebuffRangeSelect(player)` 0x453050, and the monster versions `SkillAttackRangeSelect(mob)` 0x450f70, `SkillDebuffRangeSelect(mob)` 0x451570.

1. **The animation**: `0x0511` result 4 (caster, the target player or 0 around oneself, skill, values 0) for the player versions, `0x0517`
   result 4 for the monster versions, to the players within 60 of the caster.
2. The centre: the caster (type 6), the target player (type 7 on a player) or the target monster (type 7 on a monster, its position). The
   **5 × 5 cells** around the centre's cell are searched, **players first, then monsters**.
3. **Players**: not dead, **not of the caster's side** (`IsTeam(caster, player)` false: the caster itself never), fully in the world, and
   within **`byApplyRange`** of the centre (horizontal, ≤). Attack skills by `wType`: **21, 23, 24, 25, 26** one area hit
   (`AttackUser(player, skill, false, true)`, `0x0513`); **22** `byMultiAttack` area hits, stopping when the player or the caster is dead;
   **27** nothing. Debuffs: one area hit each.
4. **Monsters**: as before (`docs/original/skills-use.md` section 9).
5. `wType` 23 (around oneself or a player): after the whole area, the caster gives up its HP as in section 2.

So since part 2 **every area skill hits enemy players too**, whatever it was aimed at; until now only monsters.

## 5. Hit or miss on a player with a skill

Confirmed, `CUser::AttackSuccess` 0x456b10.

`La`, `Lt` the caster's and the target's levels; the numbers of `docs/original/combat.md` section 4.

1. **Block** (`CUser::ChkBlock(kind)` on the target): a miss.
2. **State protections** (as monster skills, `docs/original/mob-skills.md` section 7 step 2): `byStateType` 7 with the target's
   `m_bBlockDying`, or 8 with `m_bBlockDeath`: the protection is used up and it is a miss; then `byStateType` ≠ 0 with the target's
   **`m_pStateBlock[byStateType]`** (lapis) ≠ 0: a miss.
3. **`bySuccessType`**: **1** hit when `rand() % 100 < bySuccessValue`; **2** `bySuccessValue + 100 × (LUC / (target LUC + LUC))` in integer
   division; **3** the same with WIS (no row uses 2 or 3).
4. Otherwise the kind's formula. **0 melee, 1 ranged**: part 1's (section 4 step 2) with that kind's numbers, clamped 1..99, hit when
   `rand() % 100 < rate`. **2 magic** (the target resists):
   `a = clamp(((Lt − La − 2) × 100 + Lt) / (La + Lt) × 1.1 + target magic evasion, 1, 70)`,
   `b = clamp((target WIS × 11 − caster WIS × 10) / (caster WIS + target WIS) × 3.9 + caster magic hit, 1, 70)` (single precision, the
   divisions in floating point), `rate = clamp(trunc(a + b), 1, 90)`; **a miss when `rand() % 100 < rate`**. (Magic evasion and hit are
   `m_pAttack[2].nPerDefense` and `nPerAttack`, the `add` terms: 0 without buffs.) Any other kind: a hit.

## 6. Damage to a player with a skill

Confirmed, `CUser::AttackDamage` 0x457030.

`damage[3]` from 0, result 0. By `byDamageType`:

| type | damage |
|---|---|
| **0** | `wDamage` as it is; nothing else |
| **1** | `damage = wDamage`, then the formula with **A = the caster's attack + `wDamage[0]`**; SP and MP keep `wDamage[1]`, `wDamage[2]` |
| **2** and **6+** | the formula; then for type 2 each value × `wDamage[i]` when that is not 0 (SP and MP stay 0) |
| **3** | `wDamage[i]` × **the caster's current** HP, SP, MP (each with `wDamage[i]` ≠ 0, at least 1), at most 30000 |
| **4** | `trunc(wDamage[i] / 100 × the target's maximum` HP, SP, MP`)`, at least 1, at most 30000 |
| **5** | the same with **the target's current** HP, SP, MP |

Types 0, 3, 4, 5 never give a critical. **The element** for the formula: `byAttrType` ≠ 0 replaces the caster's attack element; **`wType`
24** uses row `element`, column 0; otherwise `m_AttackAttr[element][the target's defence element]` when both are below 9, else 1.

**The formula, melee and ranged**: part 1's normal blow (section 4 step 5) with `A` as above for type 1: `d = A − target defence` (+
`rand() % m_nItemAttackVar`), at least 0; ranged within 3 × 0.7 unless `m_nCrossRoad`; × the element; `+ R × 0.1`; the critical (chance
`trunc(critical − target LUC × 0.034)` clamped 1..99) `d × 0.5 + R2 × 1.4 + d` (ranged within 3: × 0.7 again).

**The formula, magic** (new; normal blows are never magic):

- `P1 = clamp((101 × Lt − 100 − 100 × La) / (La + Lt) × 2.4, 1, 50)`;
- `P2 = clamp(target magic defence / (caster m_nItemAttack + caster INT) × 100, 1, 70)` (magic defence `m_pAttack[2].nDefense`;
  `m_nItemAttack` the attack the equipment adds);
- `x = E × (caster magic attack + w)` (`w` = `wDamage[0]` for type 1, else 0; magic attack `m_pAttack[2].nAttack`);
- `d = max(0, x − x / 100 × (P1 + P2))`, `+ R × 0.1` (`R = rand() % caster LUC + 1`, 0 when LUC is 0);
- critical as above: `d = d × 0.5 + R2 × 1.4 + d`, result 1;
- `damage[0] = trunc(d)`. No attack spread, no defence subtracted: the target's magic defence and the level gap are a percentage cut of up to
  120 %.

Then for every formula type: each value at most 30000, and **`damage[0]` of 10 or less becomes `rand() % 10 + 1`**.

## 7. A player's debuff on a player

Confirmed, `CUser::ChkKeepSkill` 0x487190.

Applied as a monster's (`docs/original/mob-skills.md` section 8: the effects by `wType`, death and dispel clear it), but **its attacker is
a player** (kind 1). A damage-over-time payment (every 3 s and at the end; the death sentence at the end):

- **not reflected**: `CUser::DamageByKeepSkill(victim, 1, caster, damage)`: the record, give-up SP/MP; at or below the HP **`CUser::Death(
  victim, 1, caster)`** (the killer of record decides); `0x050F` within 60 of the victim;
- **reflected** (`ChkMirror(the skill's attack kind)` on the victim): **`0x050F` with zero damage** to the victim's view, then the caster,
  when it is in the victim's zone, takes the damage (`DamageByKeepSkill(caster, 1, victim, damage)`, a death `CUser::Death(caster, 1,
  victim)`) and **`0x0515`** (victim, caster, damage) goes to the victim's view. A caster no longer in the zone takes nothing.

## 8. The faction kill count: the data

Confirmed, `CGameData::LoadKCStatus` 0x4bf4a0.

`Data/KillStatus.SData` (the client has the same bytes): `u32 n`, then per record `u8 faction, i32 kills, u16 index, 6 × (u8 type, u16
value)`. Each faction's list starts with **grade 0** (all zero); each record is the next grade of its faction (`stKCStatus`: `nIndex` = its
place, `dwKillCount` = `kills`, `nKillValue` = `index`, not read anywhere else; `pStatus[type] = value` for types 1 to 27). Then, per
faction and type, **each grade takes the larger of its own and the grade below's value** (the bonuses build up). The shipped file has 30
grades per faction, kills **150, 300, … 3000, 3750, 4500, 5400, 6300, 7200, 8400, 10200, 11400, 12000, 12288**; grade 30 is the top
(`GetKCStatusMax` = the count − 1).

**The bonus types** (`CUser::InitKCStatus` 0x48be90, `CUser::UpdateKCStatus` 0x48b780 for 1 to 23; the world copies 24 to 27):

| type | adds (`trunc(× 0.01)` where a percentage) | in the file |
|---|---|---|
| 1, 2, 3 | max HP, MP, SP + `max × v %` | Fury 1–3 |
| 4, 5, 6, 7, 8, 9 | STR, REC, INT, WIS, DEX, LUC + v; REC also max HP + 5v, WIS max MP + 5v, DEX max SP + 5v (see deviation 1) | grade 29 |
| 10, 11, 12 | HP, SP, MP recovery (`m_nRecHP`, `m_nSitRecHP`, and the same for SP, MP) + `max HP, SP, MP × v %` | yes |
| 13, 14, 15 | HP, SP, MP recovery while fighting (`m_nAtkRec*`) + `max × v %` | 26 |
| 16 | critical (`m_nCritical`) + v | 30 |
| 17 | melee hit (`m_pAttackAdd[0].nPerAttack`) + v | no |
| 18, 19 | melee, ranged defence + `that defence × v %` | 15, 14 |
| 20 | magic defence + v (see deviation 1) | 14 |
| 21, 22, 23 | melee, ranged, magic evasion + v | 30 |
| 24 | **lapis linking and extraction rate + v** (the `KC[country]` term of `docs/original/lapis.md` section 1) | 1, 3, 5 |
| 25 | **experience lost at death − v %** (`CUser::ExpDrop`: `loss −= trunc(v / 100 × loss)`) | 50, 100 |
| 26, 27 | nothing: copied into the world, never read in this build | 10, 4 |

## 9. The counts and grades

Confirmed, `CWorld::AddKillCount` 0x413600, `AddKillCountGrade` 0x413410, `SetKillCount` 0x4138e0, `ResetKillCount` 0x413270, `UpdateKillCount` 0x413300, `InitKillCount` 0x413150, `ApplyKillCount` 0x4139c0.

The world keeps per faction (`m_KCInfo[2]`, `stKCInfo`): the **grade** (`nIndex`), the **count** (`dwCount`), the **next grade's count**
(`dwCountNext`), a **reset time** (`dwResetTime`, 0 = none) and a **send time** (`dwSendTime`), and the grade's types 24 to 27.

- **Loading**: ps_dbAgent answers the server's start with **`0x0205`** (`TP_DB_SYSTEM_WORLDKILLCOUNT`, `u32 count[2]`); `InitKillCount`:
  each faction's count; the grade = the highest whose `kills` ≤ count; the next count = the next grade's, or at the top **reset time = now +
  10 min**; types 24 to 27 copied.
- **A kill** (`KillPCTo`, part 1 section 7): **`AddKillCount(killer's country, 1)`**. Nothing while **`g_bDisableKillCount`** (a console
  command, off by default) or while **either** faction has a reset time. Otherwise count + 1; when it reaches the next count, the new grade
  is taken (`GetKCStatusByCount`); below the top the next count follows, **at the top: reset time = now + 10 min**. Then `ApplyKillCount`
  (types 24 to 27 copied; every zone re-applies the grade to that faction's players and sends `0x020F`, below), send time = now + 10 min,
  and **at the top: the other faction's count goes to 0** (`ResetKillCount(other)`: `SetKillCount(other, 0)`, its `0x020F` marked, its send
  time = now + 10 min, its reset time 0, `0x0205` with both counts to ps_dbAgent), game log `0x0A01`.
- **`SetKillCount(faction, value)`**: nothing when equal; else the count, grade, next count as in loading (at the top the reset time),
  `ApplyKillCount`, send time = now + 10 min.
- **The world tick** (`UpdateKillCount(now)`, from the world send thread), per faction: **without a reset time**, when the send time is
  passed: `0x020F` marked, send time = now + 10 min; **with one**, when it is passed: **`SetKillCount(faction, 0)`** (the top's 10 minutes are
  over: back to grade 0), `0x020F` marked, send time = now + 10 min, reset time 0. When anything happened: **`0x0205` (`u32 count[2]`) to
  ps_dbAgent** (the counts are saved every 10 minutes and at resets, not at each kill).
- **The Goddess Blessing** (item special 30, 100/41; `CUser::ItemUse` 0x465c7f): **`AddKillCountGrade(country, 5)`**: refused (the item's
  error answer, `docs/original/item-use.md` section 3) while counting is disabled or either faction has a reset time; otherwise the grade
  `min(grade + 5, top)`, **the count = that grade's `kills`**, and the rest as a kill reaching a grade (next count or the top's reset time and
  the other faction's reset, `ApplyKillCount`, send time); the item is used up.

**The zones** (`CWorldThread::UpdateKillCount` 0x407c30, every world-thread pass): a faction marked by `ApplyKillCount`: every zone
**re-applies the grade to its players of that faction** (`CZone::ApplyKillCountAllUser` → `CUser::UpdateKCStatus`, section 10) and then
`0x020F`; a faction marked for sending only: `0x020F`. **`0x020F`** (`TP_..._KILLCOUNT`, 7 bytes: **`u8 faction, u32 count`**, the count at
most the top grade's `kills`) goes to **every player of that faction** in every zone (`CZone::SendAllCountryWithEnterWaitUser`). A top grade
also resets the maps' obelisks (`CMap::ObeliskReset`, built, `docs/original/obelisks.md` section 5).

**`0x0211`** (on entering, `CUser::SendKCStatus` 0x481920, burst item 14): `u8 country, u32 count, u32 ms` = the player's faction's count
and the time left until its reset time (0 without one). The client's countdown is the top grade's 10 minutes.

## 10. The grade on a player

Confirmed, `CUser::InitKCStatus` 0x48be90, `CUser::UpdateKCStatus` 0x48b780.

The player keeps what its faction's grade added (`m_pKCStatus[28]`). **`InitKCStatus`** runs inside `CUser::InitEquipment` (entering the
world, resets), **after the equipment and before the attack adds and the passive skills**: types 1 to 3 on the maxima so far, then 10 to 15
on those maxima, 18 and 19 on the defence (which `InitEquipment` has cleared: **0**), 20, 4 to 9, 16, 17, 21 to 23. **`UpdateKCStatus`**
(a grade change in the player's faction, every player of it in every zone): each type's old addition is taken off and the new one added (the
percentages of the current values), then `0x050B` for each maximum that changed, the current values capped (`SendRecoverAdd(0, 0)`) and
the attack numbers recomputed (`SetAttack`).

The two disagree (original bugs): **type 8** (DEX) adds 5 × v to **max MP** in `InitKCStatus` but to max SP in `UpdateKCStatus`; **type 20**
goes to magic **defence** in `InitKCStatus` but to magic **evasion** (on top of type 23) in `UpdateKCStatus`; **types 18 and 19** are 0 after
any `InitEquipment` and only count after a grade change.

## 11. The new server

**Skills on players**: `ChkTargetUser`'s attack and debuff branch (enemy, alive, not hidden), the map-42 rule for `0x0511`; the effect of
sections 2 to 7: one hit, multi-hit, desperate counterattack and drain on a player; areas around oneself, around a player and around a
monster hitting enemy players first, then monsters; hit and damage with magic; reflection, additional damage, the debuff with a player as
its attacker; damage over time from a player (reflected back onto it); deaths through the damage record. The zone keeps an applied skill's
attacker kind (player or monster). When the skill goes off, a player it harms must still be an enemy (alive, not hidden, of the other side);
otherwise nothing happens and nothing is spent, as when a monster target is gone. The caster's own packets keep the original's order (a
victim's `0x0504` before the hit). Fire Thorns and the fields (traps, fire walls) hit players too (built: `docs/original/thorns-fields-pvp.md`).

**The faction kill count**: the grades from `KillStatus.SData` (imported into `world.kill_status` by `tools/world/import.py`), the counts per
game server in Postgres (`faction_kill_counts`: server id, country, count; read at start, written when the original sends `0x0205`), the
kill, the Goddess Blessing, the 10-minute tick, `0x020F` to the faction, `0x0211` with the real values, the bonuses in the character's numbers
(`stats::compute`), the lapis rate term (type 24) and the death's experience loss (type 25). A setting turns counting off
(`g_bDisableKillCount`).

**Deviation** (approved):

1. **The grade's bonuses are worked out the same way every time**, as the new server works out all numbers from scratch: types 1 to 3 and 10
   to 15 on the maxima after the equipment, **18 and 19 on the defence before the grade's own additions** (not 0 after entering), **type 8**
   (DEX) adds max **SP** (as DEX does everywhere), **type 20** is **magic defence** (as 18 and 19 are defences; the file has it beside 19).
   The original's `InitKCStatus` and `UpdateKCStatus` disagree on all three (section 10).

**Not built here**: the operator tool's kill count commands; the Charge allowance (as for monsters). Built: duels and guild
wars (`docs/original/duel.md`, `docs/original/guild-war.md`), item options (`docs/original/item-options.md`), obelisks
(`docs/original/obelisks.md`).
