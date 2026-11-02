# Skills, part 2: using skills on monsters and on oneself

This page covers using a skill in the EP4.5 game server `ps_game.exe`: asking for a skill, the checks, the target,
casting, cooldowns, SP and MP costs, attack and debuff skills hitting monsters (single, around the target, around oneself), the hit
chance and damage of skills, drain, additional damage, the self-sacrificing attack, and the heal and sacrifice skills on oneself and
other players. It comes from static analysis; nothing was executed. Definitions, learning and the skill list are [part 1](skills.md);
the formulas of normal attacks, which skills reuse, are on [Fighting monsters](combat.md); the client's side of every packet is on
[World packets](../client/client-world-packets.md).

Not covered here: buffs (`byTypeEffect` 4), what debuffs leave behind (the monster's `AddApplySkill`), dispel (`wType` 42), cancelling
one's own buff (`0x0511` with number 255), the low-HP passives (type 6) — all in [part 3](skills-buffs.md); fields (5), the
one-off skills (6: resurrection and the like) and transformations (`wType` 90) — in [part 4](skills-special.md); skills against
players ([Player kills 2](pvp-2.md)); party heals with a party ([Parties](party.md)); the weapon lapis skills
(`CUser::AttackItemOption`; see [Item option skills](item-options.md)); and monster skills ([Monster skills](mob-skills.md)).

## In short

- **A skill goes off on the next swing.** Asking for a skill does not use it at once: it replaces the character's next normal attack.
  Skills with a cast time show a casting animation first; moving cancels the cast for free.
- **Costs are paid when the skill goes off,** not when the cast starts, and the cooldown starts at the same moment. Some skills cost
  "everything": all the caster's SP or MP.
- **Combo skills** need the previous skill in the chain to be on cooldown, that is, to have just been used.
- **Area skills** hit every enemy monster (and enemy player) within the skill's range of the target or the caster, including the target
  itself.
- **Magic skills use a resist roll** based on levels, wisdom and magic evasion; melee and ranged skills use the normal hit formula, and
  some skills have a fixed chance.
- **Damage types.** Most skills add their damage to the character's attack and go through the normal formula, critical hits included.
  Others do a fixed amount, a share of the target's HP, or a multiple of the caster's own HP.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s base stats on; see [Character numbers](stats.md)); `CMob`
and `stSkillInfo` offsets match the PDB. "rand()" and "trunc" as on [Fighting monsters](combat.md). Ticks are milliseconds
(`GetTickCount`).

## 1. The kinds of skills in the data

`byTypeEffect` (`typeeffect`) is the kind, `byTargetType` (`targettype`) whom it reaches, `wType` (`typedetail`) the variant. Learnable rows
(`typeshow` ≠ 0, levels 1 to 3, 549 rows):

| kind | target types (rows) | `wType` | here |
|---|---|---|---|
| 0 passive | 0 (91) | | part 1 |
| 1 attack | 3 one monster (97), 7 around the target (47), 6 around oneself (3), 0 (6) | 21 to 27 | sections 7 to 9; target type 0 cannot be used (section 3) |
| 2 heal | 2 oneself (12), 3 one player (18), 5 the party around (6) | 41 heal, 42 dispel, 43 sacrifice | section 10 |
| 3 debuff | 3 (116), 7 (51), 6 (15), 0 (2) | 61 to 72, 90, 122, 21 | the damage, sections 7 to 9; the lasting effect is part 3 |
| 4 buff | 2 (108), 4 party (32), 3 (9), 0 (6), 5 (2) | 69, 81 to 94, 123 | part 3 |
| 5 field | 1 (9) | 101, 102 | later |
| 6 once | 2 (6), 3 (4) | 93, 121, 123, 125 | later |

Target types: **1** a place, **2** oneself, **3** one target, **4** the party, **5** the party around oneself, **6** around oneself, **7** around
the target.

## 2. Asking: `0x0511` and `0x0517`

Confirmed, `CUser::PacketPC` cases 0x46a89b and 0x46a9ce.

Client → server, 7 bytes (`TPC_PC_SKILL_USE`): **`u8 number, u32 target`**. `number` is the skill's number from `0x0108` (part 1 section 2).
`0x0511` aims at a player (the client also sends it for skills on oneself), `0x0517` at a monster. Both are ignored without a reply when:

1. the player is dead (status 1); for `0x0511` only, **number 255 with a target other than 0 removes the player's own buff with that key**
   (`CUser::RemApplySkill`, part 3) and nothing else happens;
2. the player is casting (status 3);
3. the player's shape (`m_byShape`) is neither 0 nor 13; the attack speed is 255 (`GetAttackSpeed`: stunned, asleep, frozen, disabled by a
   skill); the player sits (motion 1); the player rides (`m_nVehicleStatus` ≠ 0);
4. the number is not below the skill count, or there is no skill at that number;
5. the skill's `byTypeShow` is 0; the character's level is below `wLevel` (when `wLevel` ≠ 0); `byJob[job]` is 0; `byGrow` is above the
   character's mode (a plain comparison, unlike learning).

Then the target is chosen (section 3). If that succeeds: a player in status 0 gets **`0x0212`** (attack start, as for `0x0503`) and the
"fighting" recovery state; status becomes 2 (attacking) unless it was 1 or 2; **`m_nAttackNext` = 2 (a skill next) and `m_nAttackSkill` =
number**. Whether or not a target was found, the server-side move is then cancelled (`CancleMove`). `m_nAttackMode` is left as it is: **1 while the player
attacks normally (`0x0503` sets it), else 0**; it decides what happens after the skill (section 4).

Nothing is used yet: the skill goes off at the player's next attack time (section 4), like the next normal blow.

## 3. Choosing the target

Confirmed, `CUser::ChkTargetMob` 0x44e6a0, `CUser::ChkTargetUser` 0x44e140.

The target (`m_Target`: kind 1 player, 2 monster, 4 none; and an id) is chosen by `byTargetType`. Before a skill aims at oneself, the current
target is kept in `m_TargetPrev`, so normal attacks can go back to it afterwards (section 4).

`0x0517` (`ChkTargetMob`):

| target type | target |
|---|---|
| 1, 4, 5 | oneself |
| 2 | oneself (`wType` 90, transformations, has its own checks: later) |
| 3, 7 | **the monster**, when `wType` ≠ 90, the kind is 1 (attack) or 3 (debuff), the monster is in the player's zone and attackable (`EnableAttackMob`: alive, of another country, `docs/original/combat.md` section 1) |
| 6 | oneself (`wType` 73: no target, kind 4; not in the learnable data) |

`0x0511` (`ChkTargetUser`) does the same for 1, 2, 4, 5 and 6. For 3 and 7 it needs a player: an attack or debuff on a player of the other side
(player kills, `docs/original/pvp-2.md` section 1), or a heal, buff or one-off skill on a living player who may receive it (`IsTeamUsable`,
and not a mount rider for skill 127); a one-off skill with `wType` 121 has its own rules. `0x0511` is ignored without a reply when the
character's map (`+0x164`, `m_wMap`) is 42 and that map has a war type (`+0x7c`): **on map 42 always** (corrected in
`docs/original/pvp-2.md` section 1).

Anything else (target type 0 among them, so the six attack skills of target type 0 cannot be used this way) gets **`0x0517` / `0x0511` result
11** "Wrong target" (section 6) and nothing changes.

## 4. The attack tick

Confirmed, `CUser::UpdateAttack` 0x48ca50, `CUser::StatusNextAttack` 0x48c970.

Every 100 ms the zone runs `UpdateAttack(now)`; nothing happens before the player's next attack time (`m_dwNextAttackTime`). Normal attacks are
`docs/original/combat.md` section 2. In status 2 with `m_nAttackNext` 2 (attack speed 255 or no skill at the number: stop, `0x0502` result 10):

- target a monster: if it is no longer attackable, **stop** (`StatusAttackStop`, nothing sent); else **use on a monster** (section 5);
- target a player (oneself here): `EnableAttackUser(wType)`: the player must be in the zone, alive (or `wType` 121 or 122), fully entered, not in
  shape 13; else stop. Then **use on a player** (section 5);
- target kind 4: use on a monster without one.

Afterwards, unless the player is now casting (status 3), **`StatusNextAttack(killed)`**, `killed` = the target died:

- not killed and `m_nAttackMode` ≠ 0: status 2 (with `0x0212` from status 0), **normal attacks next**; when the target was oneself it goes back to
  `m_TargetPrev`, or the player **stops** when there was none;
- otherwise, from a status other than 0 and 1: status 0, no target, `m_nAttackMode` 0, `m_nAttackNext` 0, recovery state normal.

Then `CancleMove` and `m_bPrevSkill` = 1 (a normal attack sets it to 0; not read by anything covered here).

**Status 3 (casting)** when the time comes: the same target handling (a monster no longer attackable: stop), then **the effect at once**
(section 7) **without the checks of section 6 again** (costs, cooldown, range are not looked at a second time), `StatusNextAttack(killed)`,
**next attack time = now + `g_ConstUserAttackSpeed[attack speed]`**, `CancleMove`, `m_bPrevSkill` = 1.

## 5. Using

Confirmed, `CUser::UseSkillMob` 0x44f2d0, `CUser::UseSkillUser` 0x44f110.

1. The checks (section 6). If one fails (its error is already sent): with `m_nAttackMode` ≠ 0 the player goes back to normal attacks
   (`m_nAttackNext` 1, status 2); otherwise status 0 and recovery state normal. The next attack time is not changed.
2. **`byReadyTime` ≠ 0: casting.** Status 2 → 3; **`0x0516`** (target a monster) or **`0x0510`** (target a player) to the players within 60 of
   the caster, 13 bytes: `u32 caster id, u32 target id, u16 skill id, u8 level` (`TP_PC_SKILL_CASTING`; for `0x0516` the monster's id); **next
   attack time = now + `byReadyTime` × 250 ms**. The effect comes then (section 4).
3. Otherwise **the effect now** (section 7). Then: `m_nAttackMode` ≠ 0: status 2, normal attacks next; else status 0, recovery state normal.
   **Next attack time = now + `g_ConstUserAttackSpeed[m_nAttackSpeed]`** (clamped 0 to 9).

**Moving cancels a cast** (`CUser::StatusCastingStopByMove` 0x48a090, from the move packet `0x0501`): in status 3, a self target goes back to
`m_TargetPrev`; then with `m_nAttackMode` 0 the player stops, else status 2, normal attacks next, next attack time = now. **Nothing is sent and
nothing is spent** (the costs and the cooldown belong to the effect). `CUser::StatusCastingStop` (0x489f10, the same plus leaving a mount and
waking up) is used by events not covered here.

## 6. The checks

Confirmed, `CUser::EnableUseSkillMob` 0x44ee50, `CUser::EnableUseSkillUser` 0x44eb40.

In this order; each failure sends one packet **to the player only**, 20 bytes, laid out as `0x0511` / `0x0517` (section 8) with the result,
`u32 caster id`, `u32 0`, the skill id and level, `u16 0, 0, 0`. The opcode is `0x0517` for a monster target and `0x0511` for a player target,
**except result 8, always `0x0511`**, and **result 10 from the monster path, `0x0511`**.

| # | check | result |
|---|---|---|
| 1 | **SP** below `wNeedSP − trunc(wNeedSP / 100 × m_nDecSP)` | 6 "Insufficient energy" |
| 2 | **MP** below `wNeedMP − trunc(wNeedMP / 100 × m_nDecMP)` | 6 |
| 3 | **equipment**: `byNeedWeapon[15]` (column `shield`) ≠ 0 needs any item in equipment slot 6 (shield); otherwise a weapon in slot 5 whose real type `t` has `byNeedWeapon[t − 1]` ≠ 0 | 7 "Unavailable equipment" |
| 4 | **cooldown**: now before the skill's `m_dwResetTime`; with `m_nCoolTime` ≥ 1 (ability 19, part 3) before `m_dwResetTime − m_nCoolTime × 1000`, and never when `m_dwResetTime ≤ m_nCoolTime × 1000` | 9 "Recharge must be completed" |
| 5 | **previous skill**: `wPrevSkill` ≠ 0 needs the player to know that skill **and its cooldown to be running** (now before its `m_dwResetTime`): a combo that must follow it | 8 "A previous skill is required" |
| 6 | the attack kind (`byTypeAttack`, 0 melee, 1 ranged, 2 magic) disabled (`m_pAttack[kind].nDisable`, abilities 32 to 34) | 10 "Cannot attack" |
| 7 | **range**, only when there is a target: monster `EnableAttackRange(monster, byAttackRange)`, player `EnableAttackRange(player, byAttackRange)` | 5 "Insufficient range" |

`m_nDecSP` and `m_nDecMP` come from abilities 13 and 14 (buffs, part 3; no passive has them); the multiplication is single precision and
truncated (`fistp` with the chop mode).

Range (`CUser::EnableAttackRange` 0x44c030 for monsters, 0x44bf90 for players): horizontal distance ≤ **slack + the player's range
(`m_nAttackRange`) + `byAttackRange` + 2.3**, plus the monster's `bySize` for a monster (its current position, `GetCurrentPos`). Slack as for
normal attacks (`docs/original/combat.md` section 2). A normal attack is the same with `byAttackRange` 0. Oneself is always in range.

## 7. The effect

Confirmed, `CUser::AttackMob(now, CMob*, CSkill*)` 0x450cf0, `CUser::AttackUser(now, CUser*, CSkill*)` 0x451c80.

In this order:

1. **The cooldown starts**: `m_dwResetTime = now + wResetTime × 1000`. With `m_nCoolTime` ≥ 1: the same when `wResetTime` is above 500,
   otherwise `m_dwResetTime = now` (no cooldown at all; with the check of section 6 the long ones also come back `m_nCoolTime` seconds early).
   The only source in the data is skill 28's low-HP passive (ability 19, value 500: part 3, built: its abilities join the bonuses).
2. **What it does**, by kind and target type:

   | through | kind 1 attack | kind 2 heal | kind 3 debuff | others |
   |---|---|---|---|---|
   | a monster target (`AttackMob`) | 3: `SkillAttackSelect`; 7: `SkillAttackRangeSelect` | | 3: one hit (section 8, single); 6: `SkillDebuffRange`; 7: `SkillDebuffRangeSelect` | nothing |
   | a player target (`AttackUser`) | 3: on a player; 6: `SkillAttackRange`; 7: around a player | `SkillHeal` (section 10) | 3: on a player; 6: `SkillDebuffRange` (the player version); 7: around a player | 4 `SkillBuff`, 5 `SkillField`, 6 `SkillOnce` |

   Skills **on a monster** with these target types: section 8 (one hit) and section 9 (areas).
3. **The costs** (`CUser::SendUseSPMP` 0x481ca0), when `wNeedSP` or `wNeedMP` is not 0: each cost is the value less `trunc(value / 100 ×
   m_nDecSP)` (resp. `m_nDecMP`), and **a cost of exactly 1 means everything the player has** (sacrifice, `wNeedSP` 1: 14 rows, `wNeedMP` 1:
   12 rows); SP and MP are lowered, at least to 0. **`0x050C`** to the player only, 6 bytes: **`u16 SP spent, u16 MP spent`** (what actually
   went). Party members get the new values (`0x0C01`/`0x0C0A`, `docs/original/party.md` section 5).
4. **`wDisable`** ≠ 0: `m_dwDisableBySkill = now + wDisable × 1000` and `SendSpeed` (attack speed 255 meanwhile). No learnable skill has it.

So **the costs are taken when the skill goes off, not when the cast starts**, and nothing checks them again then.

## 8. One hit on a monster

Confirmed, `CUser::AttackMob(CMob*, stSkillInfo*, bool single, bool option)` 0x44fd90.

`single` is true for a skill on its target (target type 3), false for each monster in an area. The packet is **`0x0517`** for a single hit and
**`0x0519`** (`TP_PC_SKILL_RANGE`) for an area hit; both 20 bytes: `u8 result, u32 caster id, u32 monster id, u16 skill id, u8 level, u16
value[3]` (HP, SP, MP), to the players within 60 of the caster.

1. **Hit or miss** (`CUser::AttackSuccess(monster, kind, skill)` 0x44d010, section 11). A miss: result 3 with values 0 (a drain, `wType` 27, as
   **`0x0518`**, 26 bytes, values and heals 0); the monster targets the player (`CMob::SetTarget(player, true)`) and, unless it already
   fights in a scripted way (`m_nBattleType` ≠ 0), is marked attacked and in battle. Skill 56 (Charge) lowers the player's move-check allowance
   by its `byAttackRange` (hit or miss; the dash is the client's).
2. A sleeping monster wakes up (`CMob::RemApplySkillSleep`).
3. **Damage** (`CUser::AttackDamage(monster, kind, damage, skill)` 0x44d540, section 12); a critical result wears the weapon
   (`QualityByAttack`, `docs/original/repair.md` section 2).
4. `CMob::Damage(monster, kind 1, player id, REC, wAggro, party)`: HP, SP, MP lowered as for a normal attack (`docs/original/combat.md` section
   3); the skill's **`wAggro`** (column `arrow`) is passed along, *inferred* to be added to the aggro with the hit (`CDamageMob::Add` not
   read again for it). On a kill: the player stops (`StatusAttackStop`) and
   **`CMob::Death`** (experience, drops) as for a normal attack.
5. **The packet**: `0x0517` / `0x0519` with the result (0, or 1 critical) and the three damages. **Drain (`wType` 27, single only):** the caster
   gets the damage back (`CUser::Heal` 0x4505a0: each of HP, SP, MP raised by the damage, but not above its maximum; the heals shrink to what
   fitted) and the packet is **`0x0518`** (`TP_PC_SKILL_USE_DRAIN`, 26 bytes): as `0x0517` plus `u16 heal[3]`.
6. **Additional damage** (`wAddDamage`, column `adddamagehp/sp/mp`, 10 rows), when one of the three is not 0 and the monster still lives (not for a
   drain): `CMob::Damage` again with those values as they are (no formula), a kill as above, then **`0x051A`** (`TP_PC_SKILL_ADD`, 19 bytes:
   `u32 caster id, u32 monster id, u16 skill id, u8 level, u16 add[3]`) to the players in view.
7. The monster targets the player and is marked attacked and in battle. **A debuff (kind 3) that did not kill** then applies its lasting effect
   (`CMob::AddApplySkill`, part 3). With `option`, the weapon's lapis skills may follow (`AttackItemOption`, `docs/original/item-options.md`).

**Variants on one target** (`CUser::SkillAttackSelect` 0x450e20, kind 1, target type 3, by `wType`):

| `wType` | rows | what |
|---|---|---|
| 21, 24, 25, 26, 27 | 66, 0, 12, 0, 9 | one hit (27 is the drain) |
| 22 | 9 | **`0x0517`** with **result 4**, the monster's id and values 0 (`SendUseSkill`) to the players within 60, then **`byMultiAttack` hits as area hits** (`0x0519` each), stopping when the monster or the caster dies |
| 23 | 1 | one hit, then **the caster gives up all their HP** (below) |

A debuff on one monster (kind 3, target type 3) is one single hit whatever its `wType`.

**`wType` 23** ("desperate counterattack", damage type 3: twice the caster's current HP): after the hit(s), if the caster lives,
`CUser::Damage(own HP)` brings them to 0 and **`CUser::Death(1, own id)`**: the death of `docs/original/mob-fight.md` section 8; the
damage gets its own entry in the damage record, and the killer of record decides who killed them (`docs/original/pvp.md` sections 5 and 6):
the player themselves (kind 1) when their own HP was the largest entry, nothing more then; experience loss applies as for a death by a
monster.

## 9. Areas

Confirmed, `CUser::SkillAttackRangeSelect` 0x450f70, `CUser::SkillAttackRange` 0x451f80, `CUser::SkillDebuffRangeSelect` 0x451570, `CUser::SkillDebuffRange` 0x451980 / 0x452c50.

1. **`0x0517`** (around a monster) or **`0x0511`** (around oneself, the target being oneself) with **result 4**, the caster, the monster's id or
   0, the skill, values 0, to the players within 60: the client plays the area animation.
2. The centre is the target monster's position (target type 7) or the caster's (6). The cells within 2 of the centre's cell (5 × 5 cells of
   the zone's grid) are searched, players first, then monsters.
3. **Monsters**: alive and of another country than the caster, within **`byApplyRange`** (column `applyrange`; horizontal distance, ≤) of the
   centre. So the target itself is hit as one of the area. Attack skills by `wType`: 21, 23, 24, 25, 26 one area hit (section 8 with `single`
   false, `0x0519`); 22 `byMultiAttack` area hits, stopping when the monster or the caster dies; 27 (drain) nothing. Debuffs: one area hit
   each.
4. **Players** of the other side who are alive and attackable within the same distance are hit the same way (`CUser::AttackUser(player,
   skill, false, true)`, player kills: built, `docs/original/pvp-2.md` section 4).
5. `wType` 23: the caster gives up their HP as in section 8, after the whole area (a duel or guild war changes the killer's bookkeeping: built
   since, `docs/original/duel.md` section 7, `docs/original/guild-war.md` section 7).

## 10. Heals

Confirmed, `CUser::SkillHeal` 0x452b50, `CUser::HealUser` 0x450670, `CUser::SkillHealParty` 0x4538a0, `CUser::SkillHealNParty` 0x453aa0.

Target types 2 and 3 with `wType` 41, 42 or 43: `HealUser(target, skill, single = true)`; 4: the party; 5: the party around.

- **41, heal**: `heal = wHeal` (columns `healhp`, `healsp`, `healmp`); **the HP heal adds the caster's WIS × 2** when it is not 0. The target
  is healed (`CUser::Heal`: not above the maxima, the values shrink to what fitted). **`0x0511`** with result 0, caster id, target id, skill
  id, level, the three healed amounts (HP, SP, MP), to the players within 60 of the caster.
- **42, dispel**: removes debuffs (`RemStateSkill`, part 3); `0x0511` with values 0.
- **43, sacrifice** (skill 106): level 1 gives the target SP equal to **the caster's maximum SP**, level 2 SP and MP (the caster's maxima),
  level 3 HP (the caster's maximum HP), each not above the target's maximum; `0x0511` with (HP, SP, MP) given, to the players in view
  (`PSendViewCombat`). Its costs (1 = everything, section 7) are the price.
- Party heals (`SkillHealParty`, type 4): `0x0511` result 4 to the players within 60; **without a party the caster heals themselves** (as a
  member, `HealUser(self, skill, false)`, packet **`0x0513`**). Type 5 (`SkillHealNParty`): `0x0511` result 4, then the other party members
  within `byApplyRange`; **without a party nobody is healed**. With a party: `docs/original/party.md` section 10.

## 11. Hit or miss with a skill

Confirmed, `CUser::AttackSuccess` 0x44d010.

1. A miss when the monster is unbeatable (`m_bUnBeatable`) or going home (state 3).
2. With a skill: a miss when **the monster resists its state** (`byStateType` ≠ 0 and the monster's `pState[byStateType − 1]` ≠ 0, the
   definition's 15 state columns) or **the skill itself** (one of the monster's six `pSkill` bytes equals the skill id).
3. With a skill, by `bySuccessType` (column `successtype`):
   - **1**: hit when `rand() % 100 < bySuccessValue` (no other formula; 64 rows of kinds 1 and 3);
   - **2**: hit when `rand() % 100 < bySuccessValue + 100 × (LUC / (monster LUC + LUC))` in integer division, so `bySuccessValue`, or + 100 when
     the monster's LUC is 0; **3**: the same with WIS. No row uses 2 or 3;
   - otherwise the kind's formula:
4. Kind 0 (melee) and 1 (ranged): the normal attack's rate (`docs/original/combat.md` section 3) with that kind's numbers.
5. **Kind 2 (magic)**, a resist roll (`Lu`, `Lm` levels):
   `a = clamp(((Lm − Lu − 2) × 100 + Lm) / (Lu + Lm) × 1.1 + monster magic evasion, 1, 70)`;
   `b = clamp((monster WIS × 11 − WIS × 10) / (WIS + monster WIS) × 3.9 + player magic hit, 1, 70)`;
   `rate = clamp(trunc(a + b), 1, 90)`; **a miss when `rand() % 100 < rate`**. Monster magic evasion is `m_pAttack[2].nPerDefense` (`add`, 0
   without buffs), the player's magic hit `m_pAttack[2].nPerAttack` (`add`); the monster's WIS is its definition's `wWis`.

## 12. Damage with a skill

Confirmed, `CUser::AttackDamage` 0x44d540.

`damage[3]` starts at 0, result 0. By `byDamageType` (column `damagetype`):

| type | damage | rows (kinds 1 / 3) |
|---|---|---|
| **0** | `wDamage` (columns `damagehp`, `damagesp`, `damagemp`) as it is; **nothing else** (no critical, no limit, no minimum) | 22 / 85 |
| **1** | `damage = wDamage`, then the formula below with **A = attack + `wDamage[0]`**; SP and MP keep `wDamage[1]`, `wDamage[2]` | 99 / 87 |
| **2** | the formula below, then each of the three × `wDamage[i]` when that is not 0 | 9 / 5 |
| **3** | `wDamage[i]` × **the caster's current** HP, SP, MP (for each `wDamage[i]` ≠ 0, at least 1), then at most 30000 | 3 / 0 |
| **4** | `trunc(wDamage[i] / 100 × the monster's maximum)` (`dwHP`, `wSP`, `wMP` of its definition), at least 1, at most 30000 | 11 / 2 |
| **5** | the same with **the monster's current** HP, SP, MP | 9 / 5 |
| other | as 1 | 0 |

**The formula** is the normal attack's (`docs/original/combat.md` section 3, step 3) for the skill's kind (0, 1 or 2) with these changes:

- the kind's attack is the player's `m_pAttack[kind].nAttack` (magic: `trunc(INT × 1.3 + WIS × 0.2 + add + itemAttack)`, `docs/original/combat.md`
  section 4) and the defence the monster's for that kind (magic: `Magic + add`);
- **the element**: `byAttrType` ≠ 0 replaces the player's attack element (column `attrtype`); **`wType` 24** uses row `element`, column 0 (the
  monster's element is ignored);
- ranged (kind 1) only: × 0.7 within 3 unless `m_nCrossRoad`, again after a critical; magic is never reduced;
- the critical roll and its formula are the same for all three kinds; the result is 1 on a critical.

After the formula (types 1, 2 and others): each value at most 30000, and **`damage[0]` of 10 or less becomes `rand() % 10 + 1`**. Types 0, 3, 4
and 5 return before that and never give a critical.

## 13. The packets

| op | direction | size | layout | when |
|---|---|---|---|---|
| `0x0511` | C → S | 7 | `u8 number, u32 target player` | section 2 |
| `0x0517` | C → S | 7 | `u8 number, u32 target monster` | section 2 |
| `0x0510` | S → C | 13 | `u32 caster, u32 target player, u16 skill, u8 level` | cast starts (within 60) |
| `0x0516` | S → C | 13 | as `0x0510`, target monster | cast starts (within 60) |
| `0x0511` | S → C | 20 | `u8 result, u32 caster, u32 target player, u16 skill, u8 level, u16 value[3]` | errors (caster only), heals, result 4 around oneself |
| `0x0517` | S → C | 20 | as `0x0511`, target monster | errors (caster only), single hits, result 4 around a monster and at a multi-hit's start |
| `0x0518` | S → C | 26 | as `0x0517` + `u16 heal[3]` | drain |
| `0x0519` | S → C | 20 | as `0x0517` | area hits (and each multi-hit) |
| `0x051A` | S → C | 19 | `u32 caster, u32 monster, u16 skill, u8 level, u16 add[3]` | additional damage |
| `0x0513` | S → C | 20 | as `0x0511` | a heal of a party member (and of oneself without a party, type 4) |
| `0x050C` | S → C | 6 | `u16 SP spent, u16 MP spent` | costs (caster only) |

Values are always HP, SP, MP (`docs/client`). Results: 0 hit (or heal), 1 critical, 3 miss, 4 area animation, 5 range, 6 energy, 7
equipment, 8 previous skill, 9 cooldown, 10 cannot attack, 11 wrong target.

## 14. The new server

- The skill tick joins the existing attack timer of the connection (`server.rs`): `next` becomes normal or skill (number), with the attack mode
  (`0x0503` sets it), a casting state and the previous target as in sections 2, 4 and 5. Moving (`0x0501`) cancels a cast.
- `skills.rs` gets the checks of section 6 and the effect choice of section 7 as pure functions; `combat.rs` the skill hit and damage of sections
  11 and 12 next to the normal ones; `zone.rs` one entry for a skill on a monster that does the single hit, the multi-hit and the two areas and
  returns the packets and kills, as `attack_mob` does for normal attacks.
- Cooldowns live on the learnt skills and are saved as remaining seconds in `character_skills` (column already there, part 1); costs change SP
  and MP in the zone's `Vitals` and are sent as `0x050C`; `0x0108` sends the remaining cooldowns on entering.
- Heals change the zone's `Vitals` of the target; a heal on another player needs them in the caster's zone (the original's
  `EnableAttackUser`).
- **Deviations**:
  - **skills of kinds 4, 5 and 6 (buff, field, one-off) and dispel (`wType` 42) were refused with result 10** "Cannot attack", and cost nothing,
    until parts 3 and 4 built them (the original would take the costs and start the cooldown); what is still refused is in
    `docs/original/skills-special.md` section 12;
  - debuffs did their damage but left no lasting effect until part 3 (`docs/original/skills-buffs.md` section 12);
- Not built either, as nothing in the learnable data needs it yet: `wDisable` (section 7 step 4). Charge's (skill 56, the Fighter's and
  Warrior's) lower move-check allowance is built with the check (`docs/original/move-check.md`; an earlier note here wrongly had Charge
  as not learnable). `IsTeamUsable` (section 3) was not read here: a heal on another player needs
  them alive and of the caster's country, and since the duels and guild wars not in a duel and on the same side of a war
  (`docs/original/duel.md` section 5, `docs/original/guild-war.md` section 5). Any monster killed by a skill stops the caster (section 8 step
  4), the target or one in its area.
