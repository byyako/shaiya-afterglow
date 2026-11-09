# Skills, part 3: buffs, what debuffs leave behind, dispel and the low-HP passives

This page covers lasting skill effects in the EP4.5 game server `ps_game.exe`: the list of skills applied to a
player, buffs (`byTypeEffect` 4) on oneself and through the party target types, what each buff does, how applied skills end, cancelling
one's own buff, death, saving and loading, dispel (`wType` 42), the low-HP passives (type 6), and the lasting effects of debuffs on
monsters (`CMob::AddApplySkill`). It comes from static analysis; nothing was executed.

[Part 1](skills.md) has the definitions, learning, passives and `SetSkillAbility`; [part 2](skills-use.md) using a
skill (checks, casting, cooldowns, costs, hits, heals). Monster attacks are on [Monsters fighting back](mob-fight.md).

Not covered here: hiding (`wType` 88), transformations (90), fields (`byTypeEffect` 5) and the one-off skills (6) — see
[part 4](skills-special.md); debuffs on players (they come from monster skills and player kills; see
[Monster skills](mob-skills.md) and [Player kills 2](pvp-2.md)); parties (see [Parties](party.md)); and the
item skills (rows with `typeshow` 0, used by items).

## In short

- **Buffs and debuffs are a list on the character.** Each has a key the client uses to name it, a level, and an end time.
- **Recasting** a buff at the same level only resets its timer; a lower level does nothing; a higher level replaces it.
- **Most buffs change numbers**: stats, attack, defence, speeds. Others block a number of attacks, reflect damage back at a monster, take
  damage from SP or MP first, set the attack element (only one element buff at a time), or heal over time.
- **Death removes buffs**, except the few long ones from items that are "kept on death". Those keep counting down even while the player
  is offline; ordinary buffs pause while offline.
- **Dispel** removes debuffs; how many kinds depends on its level.
- **Low-HP passives** kick in for 30 seconds when HP drops below a threshold after a hit, at most once every 3 minutes.
- **Debuffs on monsters** lower their numbers, stun, stop or put them to sleep, remove their element defence, make them turn on the
  caster, or damage them over time. One skill kills the monster outright when it ends.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s map field on); `CMob`, `CSkill` and `stSkillInfo` offsets match the
PDB. "trunc" and "rand()" as on [Fighting monsters](combat.md). Ticks are milliseconds (`GetTickCount`).

## 1. The data

Learnable rows (`typeshow` ≠ 0, levels 1 to 3) of the kinds covered here, by `wType` (`typedetail`) and target type:

| kind | `wType` | rows (target types) | what (section 5 / 12) |
|---|---|---|---|
| 4 buff | 82 | 55 (2), 22 (4), 4 (3), 3 (0) | abilities |
| 4 | 91 | 9 (2) | abilities (same as 82) |
| 4 | 81 | 6 (4) | heal at once, and over time every 3 s (part 4 section 7) |
| 4 | 83 | 6 (2), 2 (4), 1 (0) | reflection (`nMirror`) |
| 4 | 84 | 3 (2) | Fire Thorns (47): damage around oneself every 3 s (**corrected**: part 4 section 7, `CUser::ChkProtect`) |
| 4 | 85, 86 | 6, 3 (2) | blocks of one attack kind |
| 4 | 87 | 3 (2) | damage taken from SP and/or MP first |
| 4 | 92 | 3 (2), 2 (4), 1 (0) | blocks everything |
| 4 | 94 | 8 (2) | attack element |
| 4 | 69 | 3 (2) | taunt around oneself |
| 4 | 123 | 2 (3) | nothing in the server |
| 4 | 88, 90 | 8, 9 | hiding, transformations: part 4 (`docs/original/skills-special.md`) |
| 2 heal | 42 | 3 (2), 3 (3), 3 (5) | dispel (section 9) |
| 3 debuff | 61 | 41 | damage over time, every 3 s and the rest at the end (part 4 section 7) |
| 3 | 62, 67 | 64, 12 | abilities lowered |
| 3 | 63 | 3 | the monster dies at the end |
| 3 | 64, 65, 66 | 19, 24, 8 | stun, stop, sleep |
| 3 | 68 | 3 | the monster's element defence removed |
| 3 | 72 | 6 | the monster turns on the caster |
| 0 passive | 6 | 15 | low-HP abilities (section 11) |

`dwKeepTime` (`keeptime`) is the duration in seconds; with `byDontRemByDeath` (`fixrange`) 1 it is in hours in the data and stored × 3600
(part 1 section 1). No learnable row has `byDontRemByDeath`; the item rows do.

## 2. The list

Confirmed, `stUserInfo::m_applySkill`, `CSkill`.

Each player has a list (`m_applySkill`, code `+0xa94`) of `CSkill` objects, one per applied skill:

| field | meaning |
|---|---|
| `m_dwID` | **the key**: a number from `CObjectMgr::CreateSkill`, unique among the live skill objects (*inferred* from its use: the client names a buff by it in `0x050E` and in a cancel) |
| `m_nSkillID`, `m_nSkillLV`, `m_pInfo` | the skill, its level, its definition |
| `m_dwPrevTick` | when it was applied (`now`) |
| `m_dwEndTime` | when it ends: `now + dwKeepTime × 1000` in ticks; **with `byDontRemByDeath`, `now / 1000 + dwKeepTime` in seconds** |
| `m_Attacker` | who applied it (kind 1 player / 2 monster, id) |

Monsters have the same kind of list (`CMob::m_applySkill`, section 12).

## 3. Applying a skill to a player

Confirmed, `CUser::AddApplySkill(info, kind, id)` 0x484850.

1. **Map rule** (`EnableApplySkillByMap` 0x4845f0): on map **18** these are not applied at all: skills 111, 112, 114, 116, 117, 120, 121, 123,
   127 at levels 2 and 3, and 124, 175, 181, 185 at any level.
2. **Exclusive skills**: when the new skill has `wType` 88 or 90, or is skill 114 with `wType` 82, the first applied skill that is skill 114
   with `wType` 82, or has `wType` 90 or 88, is removed first (`CUser::RemApplySkillByOtherSkill` 0x489740: `0x050E`, its effect undone).
3. The list is searched for the same skill id. Level 100 and levels 1 to 3 never meet (a level-100 entry and a level-1 entry of one id live side
   by side). With a match:
   - **same level: a refresh.** The attacker, `m_dwPrevTick = now` and the end time are set again (as in section 2); **`0x050D`** is sent again
     (step 5). The effect is not added a second time.
   - **lower or equal level** (new level 1 to 3): nothing at all.
   - **higher level**: the old entry is taken off (`0x050E`, its effect undone as in section 6), and the new one goes in as below.
4. Otherwise a new `CSkill` with a new key, the values of section 2.
5. **`0x050D`** (`TP_PC_SKILL_SET`, 9 bytes) to the player: **`u32 key, u16 skill id, u8 level`**. The client takes the duration from its own
   skill data (`docs/client/client-world-packets.md`). The database agent gets `0x0901` (`TP_DB_SKILL_APPLY_ADD`: char id, skill id, level,
   `dwKeepTime`); the party gets `0x0C04` / `0x0C0E` unless `byDontRemByDeath` (`docs/original/party.md` section 5).
6. **The effect**: kind 3 → `CUser::AddApplySkillDebuff` (debuffs on players: `docs/original/mob-skills.md` section 8), kind 4 →
   `CUser::AddApplySkillBuff` (section 5). The entry joins the list.

## 4. Buff skills

Confirmed, `CUser::SkillBuff` 0x453470, `CUser::BuffUser` 0x450ac0, `CUser::SkillBuffParty` 0x453c90, `CUser::SkillBuffNParty` 0x453ea0.

A buff goes off like any skill (part 2: checks, casting, cooldown and costs at the effect). The target is the caster for target types 0, 1, 2, 4,
5 and 6 (part 2 section 3); type 3 aims at a player (oneself, or another living player of one's country in the zone). By target type:

- **2, 3: `BuffUser(target, caster, single)`**:
  1. **`0x0511`** (20 bytes, part 2's layout: result **0**, caster id, target id, skill id, level, `u16 value[3]`) to the players within 60 of the
     caster. The values are 0, **except `wType` 81**: the target is healed at once by `wHeal` (HP + the caster's WIS × 2 when not 0; `CUser::Heal`:
     not above the maxima, the values shrink to what fitted) and the values are what was healed.
  2. **`AddApplySkill(info, 1, caster id)`** on the target (section 3), unless the target has a personal shop open and the skill has `wType` 88
     or 90.
  3. **Aggro**: when the skill is not `wType` 81 and has a `wAggro` (`arrow`), the target's status is not 2 and its target is a monster in the
     zone, that monster gets `wAggro` aggro toward the caster (`CMob::Aggro`). Only Fire Thorns (47) and Mass Taunt (105) have a `wAggro` among
     buffs; both aim at the caster and have no cast time, and an instant skill goes off in status 2 (part 2 section 2), so **in practice this
     never fires** (Mass Taunt's pull is its taunt, section 5.2).
- **4, the party** (`SkillBuffParty`): `0x0511` with **result 4**, the caster, target 0, the skill, values 0, to the players within 60. With a
  party, each member in range (`IsTeamUsable`, `EnableApplyRange` with `byApplyRange`; in a big party, only the caster's group) gets
  `BuffUser(member, caster, false)`. **Without a party the caster buffs themselves** (`BuffUser(caster, caster, false)`, when not dead).
- **5, the party around** (`SkillBuffNParty`): as type 4, but the caster is never among the members; **without a party nobody gets it.**

`BuffUser` with `single` false sends **`0x0513`** instead of `0x0511` (same layout).

## 5. What a buff does

Confirmed, `CUser::AddApplySkillBuff` 0x484cf0, `CUser::RemApplySkillBuff` 0x485130.

By `wType`, when applied / when it ends:

| `wType` | applied | ends |
|---|---|---|
| **82, 91** | the three abilities through `SetSkillAbility(type, value, 4)` (part 1 section 6: stats, maxima, adds, speeds, critical, cost reductions, cool time, …) | the same with `−value` |
| **81** | nothing (the heal at once is `BuffUser`'s); **every 3 s the same heal is paid for the time since the last payment** (part 4 section 7, **corrected**) | **the rest of the heal over time**: each of HP, SP, MP (`wTimeHeal`, columns `timehealhp/sp/mp`) × `trunc(elapsed / 3000 × wTimeHeal[i])` with `elapsed = now − m_dwPrevTick`; `CUser::Heal`; **`0x050F`** (`TP_SKILL_KEEP`, 15 bytes: `u32 player id, u16 skill id, u8 level, u16 healed[3]`) to the players within 60 |
| **83** | for each `i` with ability `i`'s value ≠ 0: `m_pAttack[i].nMirror = skill id`, `nMirrorLevel = level` | each that still holds this skill id → 0 |
| **85, 86** | `m_pAttack[byDefenseType].nBlock` = 1 (85: a count) or 2 (86: a chance), `nBlockValue = byDefenseValue` | `nBlock` = 0 |
| **87** | `m_nGiveupSPMP = level` | 0 |
| **92** | `m_bBlockAll += 1` | `−= 1`, not below 0 |
| **94** | attack element (section 5.1) | attack element back to the weapon's (`CUser::SetAttrAtk`) |
| **95** | `m_nAttrDef = byAttrType` | defence element back to the armour's (`CUser::SetAttrDef`) |
| **69** | **taunt** (`CUser::TauntMob(caster, byApplyRange, wAggro)`, section 5.2) | nothing |
| 88, 89, 90, 93 | hiding, not seeing monsters, transformation, finding the hidden | part 4 |
| **84** | nothing here; **every 3 s** damage around the player (part 4 section 7) | nothing |
| others (123, …) | nothing | nothing |

`byDefenseType` is stored lowered by one (part 1 section 1), so it is the attack kind 0 melee, 1 ranged, 2 magic: Magic Protection and Magic
Evasion (54, 141) block magic, Eagle Eye (73) ranged.

### 5.1 The attack element (`wType` 94)

`m_byAttrAtkBuff[9]` remembers which elements come from a buff. When the new element is below 9 and not yet from a buff: if the current attack
element comes from a buff, every applied `wType` 94 skill of another element is taken off (`0x050E`, its effect undone); then the attack element
(`m_nAttrAtk`) becomes `byAttrType` and that element is marked. So **one element buff at a time**, the newest. Ending: the mark is cleared, the
element becomes 0 and then the weapon's again.

### 5.2 Taunt (`wType` 69, `CUser::TauntMob` 0x4877f0)

The monsters in the 3 × 3 cells around the caster's cell that are not dead and within **`byApplyRange`** (horizontal distance, ≤) take the
caster as their target (`CMob::SetTarget(caster, true)`) and, while alive, get **`wAggro + REC × 10`** aggro toward the caster (when above 0);
a monster in state 1 then re-targets by aggro (`ReTargetByAggro`). Mass Taunt (105) has range 0 in the data.

## 6. How applied skills end

Confirmed, `CZone::UpdateApplySkill` 0x421f80, `CUser::ChkEndTimeSkill` 0x4865b0, `CUser::RemApplySkill(CSkill*)` 0x484c60.

Every second, for each player (after its recovery, `docs/original/mob-fight.md` section 9): every applied skill whose end has come
(`m_dwEndTime ≤ now`, or `≤ now / 1000` with `byDontRemByDeath`) is taken off the list; the database agent gets `0x0902`
(`TP_DB_SKILL_APPLY_REMOVE`); the player gets **`0x050E`** (`TP_PC_SKILL_REMOVE`, 6 bytes: **`u32 key`**); the party is told unless
`byDontRemByDeath`. Then the effect is undone (`RemApplySkill`): kind 4 as section 5 "ends", kind 3 the debuff's, kind 0 `wType` 6 the low-HP
abilities (section 11).

## 7. Cancelling one's own buff

Confirmed, `CUser::PacketPC` 0x0511, `CUser::RemApplySkill(key)` 0x484b10.

`0x0511` with **number 255** and a target ≠ 0 (part 2 section 2): the applied **buff** (kind 4) with that key is taken off; its effect ends as in
section 5 (so `wType` 81 heals for the time it ran); `0x0902` and **`0x050E`** as in section 6; the party is told. Nothing for an unknown key or
a skill that is not a buff. Allowed while doing anything except being dead.

**The client sends this only for some buffs** (verified (disasm), `game.exe` 0x4b8cef–0x4b8dc1, the right-click on a buff icon): never
when the skill record's byte `+0x1d` is 3; otherwise, unless `0x4c2b10` on the interface object answers yes (not read), only for
`TypeDetail` **88 (hiding), 90, 94 and 95**. Stat buffs such as 132 and 117 (`TypeDetail` 82) cannot be cancelled from the client
(found in play).

## 8. Death

Confirmed, `CUser::ClearApplySkillByDeath` 0x488000, from `CUser::Death`.

Every applied skill **without `byDontRemByDeath`** is taken off (`0x0902` to the database agent; **no `0x050E`**: the client clears them
itself), except, when the code's `+0x145c` (PDB `m_nDropExp`) is −1, skills with an id above 200 at a level other than 100. Undone on the way:
`wType` 6 (the low-HP mark cleared too), 62, 67, 82, 91 (their abilities, with `+value` for kinds 3: a debuff gives back what it took), 90
(shape). Then the state skills set is cleared wholesale: stop, stun, sleep, transformation, attack element marks, not-seeing-monsters, block-all,
the blocks and mirrors of all three kinds, give-up SP/MP, shape; and the elements are taken from the equipment again (`SetAttrDef`,
`SetAttrAtk`), then `SendSpeed`.

`byDontRemByDeath` skills stay, still counting down.

## 9. Dispel

Confirmed, `CUser::HealUser` `wType` 42, `CUser::RemStateSkill(info)` 0x488c90.

A heal-kind skill with `wType` 42 on a living player (target types 2, 3 and 5 as for heals: part 2 section 10): **`0x0511`** with values 0 to the
players within 60, and from the target's list, **by the dispel's level**:

- **1**: every applied skill whose `byStateType` is **9 to 15**;
- **2**: `byStateType` 1 to 6 or 9 to 15;
- **3**: every **debuff** (kind 3).

Each is taken off with `0x0902`, **`0x050E`**, the party told, and its debuff effect cancelled (`CUser::CancleApplySkillDebuff`). Players
carry debuffs since the monster skills and player kills were built (`docs/original/mob-skills.md` section 11, `docs/original/pvp-2.md`
section 11), so a dispel now removes them (`PlayerBuffs::dispel`).

## 10. Applied skills in monster attacks

Confirmed, `CUser::ChkBlock` 0x44c5a0, `CUser::Damage` 0x44c1e0, `CUser::ChkMirror` 0x44e080.

- **Block** (`ChkBlock(kind)`, `docs/original/mob-fight.md` section 6 step 2): **`m_bBlockAll` ≠ 0 blocks everything.** Else by
  `m_pAttack[kind].nBlock`: **1** blocks and counts `nBlockValue` down, and at 0 the buff ends (`CUser::RemBlockCount`); **2** blocks when
  `nBlockValue` is 100 or `rand() % 100 ≤ nBlockValue`. A blocked attack is a miss. Monster normal attacks are melee (kind 0); the 85 and 86
  rows block magic and ranged, which monster skills and player attacks bring (both built).
- **Give up SP/MP** (`m_nGiveupSPMP`, `wType` 87), in `CUser::Damage` after SP and MP damage: with HP damage `d` > 0, by level: **1** SP pays
  first (all of `d` if SP ≥ `d`, else SP goes to 0 and the rest stays HP damage); **2** the same with MP; **3** SP first, then MP for what is
  left. What was taken is added to the SP/MP damage values (shown in `0x0605`).
- `m_nReduceDamage` comes from equipment and lapis (`ItemEquipmentAdd`, `ItemLapisianAdd`, `GemEquipmentAdd`), not from skills: the HP damage
  is lowered by it (at least 1 left) and `0x0525` (`u16 absorbed`) tells the player. It belongs to items (`docs/original/lapis.md`
  section 11: `combat::absorb` in `zone::mob_skills::hurt_player_skip`, for every blow on a player).
- **Reflection** (`CUser::ChkMirror(kind)`, asked by `CMob::Attack`, by the player-kill code `CUser::AttackUser` and by monster skills): when
  `m_pAttack[kind].nMirror` holds a skill, that skill's `bySuccessType` decides: **1** `rand() % 100 ≤ bySuccessValue` (every 83 row has 1 and
  100: always); 2 and 3 compare with LUC × 10 or WIS × 10 / `bySuccessValue`. In **a monster's normal attack** (`CMob::Attack` 0x4a2000, kind 0),
  after a hit (a miss is not reflected): unless the monster resists that skill (one of its six skill bytes is the mirror skill's id), the damage
  is computed as usual but **the player takes none**: `0x0605` goes out with the result and values 0; then (not for scripted monsters) **the
  monster takes the damage itself** (`CMob::Damage` with the player as attacker, `CMob::Death` when it kills, the player stops attacking) and
  **`0x051B`** (16 bytes: `u32 player id, u32 monster id, u16 damage[3]`) goes to the players within 60 of the monster. So Counter Storm (89,
  melee) and 181 (all kinds) work against monsters now; Magic Reflection (86, magic) waits for monster skills and player kills.
- After a hit that leaves the player alive, **`CUser::ChkHPAbilityApply`** (section 11).

## 11. The low-HP passives

Confirmed, `CUser::AddSkillPassive` type 6, `CUser::ChkHPAbilityApply` 0x487020.

Learning a passive with `wType` 6 stores (skill, level) in `m_pHPAbility` (`wID`, `byLevel`; `bApply`, `dwPrevTime`), part 1 section 5. The 15
rows (skills 24 to 28, levels 1 to 3) have `byLimitHP` 8 to 25 and abilities such as attack speed + attack (24), evasion (25), magic defence +
critical (26), move speed + critical (27), and **`m_nCoolTime` 500 (28)**.

After each hit that leaves the player alive: when a low-HP passive is known, it is not in effect (`bApply` 0), **at least 180 s have passed since
it last took effect** (`dwPrevTime`, 0 at first), and **HP ≤ trunc(byLimitHP × max HP / 100)** (single-precision division): a `CSkill` with a
new key, the skill and level, **ending 30 s later** (whatever `dwKeepTime` says), joins the applied list; `0x050D` to the player (and `0x0901`,
party); `bApply` = 1, `dwPrevTime = now`; its three abilities are added. It ends like any applied skill (section 6: `0x050E`, abilities
removed, `bApply` 0), or with death (section 8). It is never saved (section 13 saves kinds 3 and 4 only).

`m_nCoolTime` (ability 19) is part 2 section 7: while skill 28's effect lasts, skills with `wResetTime` ≤ 500 get no cooldown at all.

## 12. Debuffs on monsters

Confirmed, `CMob::AddApplySkill` 0x4a0aa0, `CMob::AddApplySkillDebuff` 0x4a0ca0, `CMob::RemApplySkill` 0x4a0da0, `CMob::ChkEndTimeSkill` 0x4a12f0.

After a debuff's hit that did not kill (part 2 section 8 step 7), **`CMob::AddApplySkill(info, 1, player id)`**, kind 3 only:

1. Same skill id and level: refresh (attacker, `m_dwPrevTick = now`, `m_dwEndTime = now + dwKeepTime × 1000`; `wType` 72 taunts again). Same id,
   lower or equal level (1 to 3): nothing. Higher level: the old one ends first **with its end effect** (below), then the new one. Level 100 and 1
   to 3 live side by side.
2. Otherwise a new entry (`m_dwPrevTick = now`, end as above) and its effect (`AddApplySkillDebuff`). **Nothing is sent**: the hit packet was
   enough for the client.

| `wType` | applied | ends (and on a higher level's arrival) |
|---|---|---|
| **61** | nothing; **every 3 s the same damage for the time since the last payment** (part 4 section 7, **corrected**) | **the rest of the damage over time**: `CMob::TimeDamage(elapsed)` (below), `CMob::Damage` with the attacker (kind and id kept in the entry), **`CMob::Death`** when it kills; **`0x0607`** (`TP_SKILL_KEEP`, 15 bytes: `u32 monster id, u16 skill id, u8 level, u16 damage[3]`) to the players within 60 of the monster |
| **62, 67** | `CMob::SetSkillAbility(type, −value)` × 3 | the same with `+value` |
| **63** | nothing | **the monster loses all its HP** (`CMob::Damage` with its current HP; `Death`); `0x0607` with that HP |
| **64** | `m_nStun += 1`, `SendSpeed` | `−= 1`, `SendSpeed` |
| **65** | `m_nStop += 1`, `SendSpeed` | `−= 1`, `SendSpeed` |
| **66** | `m_nSleep += 1`, `SendSpeed` | `−= 1`, `SendSpeed` |
| **68** | `m_nAttrDef = 0` (no element defence) | back to the definition's element |
| **72** | `m_dwVisionFixUserID = caster`; the monster targets the caster (state 1 and next move 0 unless dead) | `m_dwVisionFixUserID = 0` |
| others | nothing | nothing |

**`CMob::TimeDamage(elapsed)`** (0x4a3be0): base values by `byTimeDamageType` (`timedamagetype`): **0** `wTimeDamage` (`timedamagehp/sp/mp`);
**3** `wTimeDamage[i] ×` the monster's current HP, SP, MP (at least 1 each when not 0); **4** and **5** a percentage of the monster's maximum and
current values (at least 1; *inferred* by analogy with part 2's damage types 4 and 5, the arithmetic is in `_ftol2` calls). Then **each × elapsed
/ 3000, truncated**, at most 30000. So a damage-over-time debuff deals `wTimeDamage` per 3 s it lasted (**corrected**: paid every 3 s by `CMob::ChkKeepSkill` and the rest at the end, part 4 section 7, not in one blow); a refresh
restarts the count (the time since the last payment is lost).

**`CMob::SetSkillAbility`** (0x4a1420), by type: 7 WIS, 8 DEX, 9 LUC; 16 attack speed, 17 move speed (then `SendSpeed`); 20 to 22 hit add, 23
to 25 attack add, 26 to 28 defence add, 29 to 31 evasion add (melee, ranged, magic; `m_pAttackAdd`); 32 to 34 `nDisable` of each kind; others
nothing. Then `CMob::SetAttack` (0x4902f0) recomputes the numbers with the adds, the current WIS (magic attack) and the current LUC (critical).

**Speeds**: `CMob::GetAttackSpeed` is 255 while stunned or asleep, else the attack speed clamped 0 to 6; `GetMoveSpeed` 255 while stopped,
stunned or asleep, else clamped 0 to 4. At attack speed 255 the attack pass does nothing (`CMob::UpdateAttack`); at move speed 255 the monster
neither walks idle, chases nor returns home. `SendSpeed` is `0x0609` within 60 (`docs/original/mob-fight.md` section 5).

**Waking**: a skill hit on a sleeping monster removes all its sleep entries without their end handling and sets `m_nSleep` 0, `SendSpeed`
(`CMob::RemApplySkillSleep` 0x4a16c0).

**Ending** every second (`CMob::ChkEndTimeSkill`, from `UpdateApplySkill` after `CMob::UpdateRecover`): entries whose end has come, through
`RemApplySkill` (the table). **On death and on reset** the list is dropped without any end handling (`CMob::ClearApplySkill` 0x4a0990), and
`ResetAttack` puts the numbers back.

## 13. Saving and loading

Confirmed, `CUser::SendDBAgentCharGetInfo` 0x46c980, `CUser::AddApplySkill(id, level, delay)` 0x487ce0, `CUser::InitApplySkill` 0x487e50, `CUser::SendCharacterSetSkill` 0x483890.

- **Saving** (with the character, `0x0904` `TP_DB_SKILL_APPLY_SET_LIST`: `u8 count`, then `u16 skill id, u8 level, u32 delay` each): every
  applied **kind 3 or 4** skill whose `wChangeType` is not 100, with `delay` = the seconds left (`(end − now) / 1000`, or `end − now / 1000`
  with `byDontRemByDeath`), 0 when past, **at most `dwKeepTime`**.
- **Loading** (one row at a time, before the burst): the definition must exist and be kind 3 or 4, and the map rule of section 3 allow it.
  - `byDontRemByDeath`: **the time offline counts**: when `delay` is below the seconds since the character was last saved
    (`m_dwTermSecCharDisconnected`), the row is deleted (`0x0902`); else it ends at `now / 1000 − offline + 10 + delay` seconds, at most
    `now / 1000 + dwKeepTime`.
  - Others: **the time offline does not count**: it ends at `now + (delay + 10) × 1000`, at most `now + dwKeepTime × 1000`.
  - New key, `m_dwPrevTick = now`, attacker kind 1 (2 for level 100) and id 0. No packet.
- **Before the burst** (`InitApplySkill`, after the equipment and the passives): each loaded skill's effect is added (kind 3 debuff, kind 4
  buff), so the maxima and speeds in the burst include them.
- **In the burst**: **`0x010A`** (`TP_CHAR_SETSKILL`, `docs/original/enter-world.md` section 3, only when the list is not empty): `u8 count`, then
  11 bytes each: **`u32 key, u16 skill id, u8 level, u32 seconds left`** (`(end − now) / 1000`, or `end − now / 1000`; below 0 → 1), up to 128 per
  packet.

## 14. The packets

| op | direction | size | layout | when |
|---|---|---|---|---|
| `0x050D` | S → C | 9 | `u32 key, u16 skill, u8 level` | a skill applied or refreshed (player only) |
| `0x050E` | S → C | 6 | `u32 key` | an applied skill ended, was cancelled, replaced or dispelled (not on death) |
| `0x050F` | S → C | 15 | `u32 player, u16 skill, u8 level, u16 healed[3]` | a heal over time ends (within 60) |
| `0x0511` | C → S | 7 | `u8 255, u32 key` | cancel one's own buff |
| `0x0511` / `0x0513` | S → C | 20 | part 2 | a buff (result 0) or the party types' animation (result 4) |
| `0x0607` | S → C | 15 | `u32 monster, u16 skill, u8 level, u16 damage[3]` | a monster's damage-over-time or death sentence ends (within 60) |
| `0x0609` | S → C | 8 | `u32 monster, u8 attack speed, u8 move speed` | a monster's speeds change (within 60) |
| `0x010A` | S → C | 3 + 11n | `u8 count, (u32 key, u16 skill, u8 level, u32 seconds) × n` | burst, when buffs are applied |
| `0x051B` | S → C | 16 | `u32 player, u32 monster, u16 damage[3]` | a monster's attack reflected onto itself (within 60) |
| `0x0525` | S → C | 4 | `u16 absorbed` | damage reduction from equipment (`docs/original/lapis.md` section 11) |

## 15. The new server

- **The list per player** lives in the zone next to `Vitals` (it ends by the zone's one-second pass and monster attacks read block-all and
  give-up). Keys come from one counter for the whole server. Whenever the list's effects change, the connection is told (a zone event) and
  recomputes the numbers from scratch as for equipment and passives: the abilities of applied kind-4 skills of `wType` 82 and 91, and of an
  active low-HP passive, join the passives' bonuses (part 1's `Bonuses`); `wType` 94 overrides the weapon's attack element. Changed maxima and
  speeds are sent as after an equipment change.
- Buffs follow sections 3 to 7; dispel section 9; death section 8; the low-HP passives section 11; monster attacks
  read block-all, the blocks (melee), reflection (melee) and give-up SP/MP (section 10).
- **Monsters** get a list of applied debuffs with the table of section 12 (abilities into the monster's numbers and speeds, stun/stop/sleep
  through speed 255, element defence, taunt), ended in the one-second pass; damage over time and the death sentence at the end through the
  existing kill path (experience to the top damage dealer, drops); `0x0607` and `0x0609`.
- **Saving**: a table `character_applied_skills` (character, skill, level, seconds left, saved at), written with the character on leaving and
  read on entering; the time offline is taken from "saved at" for the `byDontRemByDeath` rows. `0x010A` joins the burst.
- **Deviations**:
  - **hiding (88), transformations (90), fields (kind 5) and the one-off skills (kind 6) were still refused with result 10** (part 2), costing
    nothing; in part 4 (`docs/original/skills-special.md` section 12), which still refuses the fields other than 101 and 102 and
    the one-off skills other than 121, 123 and 125;
  - parties did not exist then: target type 4 buffs (and heals) the caster, type 5 nobody, as the original does without a party; parties are
    built (`docs/original/party.md` sections 10 and 13: the party types reach the members, who see applied skills by `0x0C04`,
    `0x0C05`);
  - the map-18 rule (section 3) is kept as data-independent code;
  - the database agent messages (`0x0901`, `0x0902`, `0x0904`) become the save on leaving; the low-HP effect is never written (the original
    writes it with `0x0901` and then never deletes it);
  - the aggro of `BuffUser` (section 4 step 3) is made to work, as its data intends: a buff with a `wAggro` on oneself (Fire Thorns, Mass
    Taunt) adds that aggro toward the caster on the monster the caster was attacking, without the original's status check (which keeps it
    from ever happening); buffs on other players add none (their fight is not known to the zone);
  - the effects are recomputed from the whole list, so for blocks and reflection the newest buff of each attack kind counts (the original
    lets an older block buff's end clear the block a newer one set);
  - death keeps every `byDontRemByDeath` skill; both exceptions of section 8 are built (`m_nDropExp` −1 keeps the skills with an id
    above 200 at a level other than 100: section 16);
  - a taunt debuff (72) turns the monster on the caster; what `m_dwVisionFixUserID` does in the monster's later choices (`AttackAI` entries
    with `byOK` 6) is built (`docs/original/mob-skills.md` section 2, `zone::mob_skills::ok_six`);
  - melee reflection (Counter Storm) is built for monster normal attacks; the magic and ranged blocks and magic reflection act on monster
    skills and player attacks since those were built (`docs/original/mob-skills.md` section 11, `docs/original/pvp-2.md` section 11);
    `m_nReduceDamage` is an item matter (built, `docs/original/lapis.md` section 11).
- **Original oddities kept**: a refresh of damage over time on a monster loses the time since its last three-second payment; (**corrected**
  in part 4: damage and heal over time are paid every 3 s, and the rest at the end or on cancelling, not in one blow); on death the client gets no `0x050E` (it clears the buffs itself, as the original expects).
- **Original oddities reachable since the monster skills**: `CUser::TimeDamage` (0x486290, damage over time on players, monster skills) **adds**
  `elapsed / 3000` to the base instead of multiplying, unlike the monster version; the new server multiplies (`docs/original/mob-skills.md`
  section 11, deviation).

## 16. The duel and guild-war death exception

Confirmed; approved; built (16.2).

**The original** (`CUser::ClearApplySkillByDeath` 0x488000, the test at 0x48805a): for each applied skill without `byDontRemByDeath`,
when `m_nDropExp` (`+0x145c`) is **−1** and the entry's skill id (`+0xc`) is **above 200** and its level (`+0x10`) is **not 100**, the
skill is **kept** (skipped, still counting down); every other one goes as section 8 says. `CUser::Death` sets `m_nDropExp` to −1 earlier
in the same call (0x4587e0, 0x45886c, 0x458cac: a duel's death, a guild war's, the flagged death) and calls `ClearApplySkillByDeath`
near its end (0x458edb), so the exception applies to exactly the deaths that cost no experience.

**What it keeps, in the shipped data** (`defs.skills`, id > 200, level ≠ 100, `keeptime` > 0, `fixrange` 0: 135 rows of 65 skills): the
item buffs 201 to 300 (the HP/SP/MP elixirs of 3 and 5 hours, the stat potions of 1 to 2 hours, speed, attack speed and critical potions,
the goddess blessings 274/275, the invincibility potions 276), and the newer player skills 302 to 316 (some of them debuffs a player put
on another). Level 100 rows are the monsters' skills (all 355 of them), so a monster's debuff still goes. Skills with ids up to 200 (the
classes' own buffs) still go.

**The new server today** clears before it knows the kind of death: `Buffs::clear_on_death` (`buffs.rs`) is called by the four places a
player's HP reaches 0 (`zone.rs` `give_up_life`, `zone/mob_skills.rs` the blow that kills, `zone/npc.rs` a guard's kill, and through
`mob_skills` every player blow), before `zone::pvp::player_died` computes `keep_exp` (`m_nDropExp` −1).

**Decided**: keep them as the original. A duel ends in `duel::won`/`end_duel` and a guild war's death takes the player out of the war
(`war_left` → `mode_end`), and both already take every debuff off (`pvp::mode_end`, with `0x050E` each), so a debuff kept by the death
goes a moment later anyway, as in the original (`PvPModeEnd`, `PvPGModeEnd`); what stays in practice is the buffs.

### 16.1 Build

1. **`buffs.rs`**: `clear_on_death(&mut self, no_exp_loss: bool) -> Vec<Applied>`: takes every applied skill that is not
   `keep_on_death`, except, with `no_exp_loss`, those with `def.id > 200 && def.level != 100` (doc comment: section 16, the test at
   0x48805a). `take_all_but_kept` (`F80E`) is unchanged.
2. **Move the clearing after `keep_exp`**: in `zone/pvp.rs` `player_died`, after `keep_exp` is known and before the `ZoneEvent::Died`
   send, call `p.occupant.buffs.clear_on_death(keep_exp)` and, when it took anything, send `ZoneEvent::Buffs` and `reform(players, victim,
   ...)` as the callers do now. Remove the four early calls (`zone.rs` `give_up_life`, `zone/mob_skills.rs`, `zone/npc.rs`; check with
   `grep -n clear_on_death`), keeping each caller's `before`/`effects` snapshot (taken before the death, used for `death_exp_percent`
   and `no_item_loss`) exactly where it is. Where a caller used the clear's result for something else (`mob_skills` sets `changed` for
   its own follow-up), keep that follow-up working from `player_died`'s side: the clear and its `ZoneEvent::Buffs` now happen there for
   every death. Read each caller before changing it; a death that does not reach `player_died` must still clear (none should exist:
   if one does, call `clear_on_death(false)` there).
3. **Tests** (`buffs.rs`): with `no_exp_loss` an elixir (id 207, level 2) and a stat potion stay, a class buff (id 31) and a monster
   debuff (id 9, level 100) go, a `keep_on_death` row stays; without it, everything not `keep_on_death` goes. A zone test (`zone.rs`
   tests, next to the duel death test around `keep_exp: true`): a duellist with an elixir buff dies to the partner and still has it; the
   same player dying to a monster loses it.
4. **Docs**: replace the deviation bullet of section 15 ("the second exception of section 8 … is not built") by "both exceptions of
   section 8 are built (section 16)"; add section 16.2. Live test: **section 50** of `docs/testing/live-test-checklist.md`.

**Live test (section 50)**: StaffChar and ProbeChar duel; ProbeChar drinks an elixir (e.g. a Life Elixir, skill 207) and a stat potion
first, loses the duel by death: after the rebirth both buffs still work (max HP and the stat still raised; after a relog they are still
listed, from the saved applied skills); a class buff ProbeChar had (a skill id up to 200) is gone. ProbeChar then dies to a monster: the
elixir goes. **Watch the client**: the original sends no `0x050E` at death because the client clears its own list; whether the client
keeps the icons of these kept buffs after a duel death is not known. If the icons vanish while the buff still works, note it: the fix
would be to send `0x050D` (applied) for each kept buff after the rebirth (a small follow-up, not built now).

### 16.2 As built

As planned, with these points:

- `PlayerBuffs::clear_on_death(no_exp_loss)` (`buffs.rs`) keeps `keep_on_death` rows, and with `no_exp_loss` the rows with `def.id > 200 &&
  def.level != 100`. The four early calls are gone (`zone.rs` `give_up_life`, `zone/mob_skills.rs`, `zone/npc.rs`; the player blows
  through `mob_skills` ended in the same function); `zone::pvp::player_died` clears right after `keep_exp` is computed, and sends
  `ZoneEvent::Buffs` when it took anything.
- **`reform(players, victim, true)` moved too**: it ran at the top of `player_died`, before the clearing; it now runs after it, so a
  transformation that goes with the death is still undone for the viewers. The clearing happens before `duel::won`/`end_duel`, so those
  see the list as the early calls used to leave it (without the kept skills).
- The `changed` flag of `mob_skills` (the death branch) is no longer set by the clear; `player_died` sends the event.
- Tests: `death_without_experience_loss_keeps_item_buffs` (`buffs.rs`) and `a_duel_death_keeps_the_item_buffs` (`zone.rs`: the partner's
  killing blow keeps an elixir and takes a class buff; a monster's kill takes the elixir).
- **Unverified:** the client's icons for the kept buffs after a duel death (live test 50); a guild war's death goes through the same
  `keep_exp` (`defeat`) but has no test of its own.

## 17. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by strings, constants, the call graph or switch tables, as those pages list).

### EP3

| function | EP3 | EP4.5 |
|---|---|---|
| `AddApplySkillBuff` (EP3 ps_game 0x469800) | `wType` 69 to 93 | 69 to 95 |
| `RemApplySkillBuff` (EP3 ps_game 0x469a60) | 81 to 92 (84 does nothing) | 81 to 95 |
| `AddApplySkillDebuff` (EP3 ps_game 0x469e80), `RemApplySkillDebuff` (0x469fd0) | 62 to 90, 61 to 90 | the same |

So the element buffs 94 and 95 (section 5) are new in EP4.5, with the four new elements (confirmed, switch bounds). Damage over time
(`CUser::TimeDamage`, EP3 ps_game 0x46abf0) has the same switch over types 0 to 5 and the same constants (confirmed). The block buffs' end (cases 85 and 86,
EP3 ps_game 0x469d68, 0x469d92), Stealth's end without a recompute (0x469dc7) and the low-HP passives' added rows (`ChkHPAbilityApply`
0x46b8f0) are as in EP4.5 (confirmed). The list, dispel and saving were not compared.

### EP5.4

| kind | applied (EP5.4 ps_game) | effect | status |
|---|---|---|---|
| buff 96 | byte set at 0x49462d, cleared at 0x494b4d | the skill checks skip the combo prerequisite | confirmed; meaning inferred |
| buff 97 | 0x494645 to 0x4946d0 | dismounts, then shape 103 and attack stop; removed with shape 0 (0x494a1a); hiding's end removes it too. In the EP6 client shape 103 hides the character from every viewer, like the stealth shape 13 without its exception for the party (EP6 game.exe 0x4e8cdf; the meaning of the flag is inferred) | confirmed |
| buff 98 | 0x494639, cleared at 0x494b68 | skills with a state effect fail on the character, from players (0x463a65) and monsters (0x4b6fd5): immunity to state debuffs | confirmed |
| debuffs 74 to 76 | 0x494d3a to 0x494e43 | one at a time (a new one removes the old type's entries); while one lasts, attacks, skills and item use are refused (0x4789a1 to 0x4790fc) | confirmed |
| debuffs 78, 79 | 0x494eec, 0x494f0d | new removal calls, not read | confirmed (calls) |

The buff and debuff switches grow to 69 to 98 and 81 to 98 (EP5.4 ps_game 0x494140, 0x494760); death and `SkillClearAll` clear the new
kinds (EP5.4 ps_game 0x498644, 0x4988af) (confirmed). `AddApplySkill` (EP5.4 ps_game 0x493970) no longer calls the map rule; the rule
(EP5.4 ps_game 0x4936a0) covers maps 18 **and 30** and runs only from `ChkApplySkillByMap` (EP5.4 ps_game 0x493730), so those buffs are
cast normally and stripped when that check runs (confirmed; when it runs inferred). Damage over time gains type 12, each tick doubling
the value (EP5.4 ps_game 0x495d48 to 0x495e12), and the cap is 60000 (confirmed). Still as in EP4.5: the player damage-over-time
addition (EP5.4 ps_game 0x495e2a) and the block-buff end (0x4949b4, 0x4949dd) (confirmed), the save filter (`SendDBAgentCharGetInfo`,
EP5.4 ps_game 0x47ad70) and the low-HP passives (`ChkHPAbilityApply`, EP5.4 ps_game 0x497050: 180 s, 30 s) (inferred, same calls).

**Target buffs (new packets).** A client selects a player (`0x0308`) or monster (`0x0309`) target with `u32 id` (6 bytes) or clears
it (`0x030A`, 2 bytes); the server then sends that target's buffs to everyone watching it: `0x030B` the list, `0x030C` one added,
`0x030D` one removed (EP5.4 ps_game 0x456bef, 0x456d31, 0x456e21 for players; 0x4b5f9f, 0x4b60a9, 0x4b6179 for monsters). Layouts
confirmed in the server immediates and the EP5.4 client; the meaning is confirmed client side, inferred for the server.

| `0x030C` / `0x030D` | size | field |
|---|---|---|
| 0 | 2 | opcode |
| 2 | 1 | kind (1 player, 2 monster) |
| 3 | 4 | object id |
| 7 | 2 | skill id |
| 9 | 1 | skill level |

`0x030B` has the same first 7 bytes, then `u8 n` and `n` rows of `u16 skill id, u8 level, u32 seconds left`.
