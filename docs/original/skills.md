# Skills, part 1: definitions, learning, the skill list, quick slots and passive skills

This page covers how the EP4.5 game server `ps_game.exe` loads skill definitions, what a character keeps, learning a
skill, the skill list sent on entering the world, quick slots, and passive skills with their abilities. It comes from static analysis;
nothing was executed. Using skills (casting, cooldowns, damage) is [part 2](skills-use.md); buffs, debuffs and the low-HP
abilities are [part 3](skills-buffs.md); special skills are [part 4](skills-special.md); monster skills are on
[Monster skills](mob-skills.md).

## In short

- **Every skill has three levels**, each a separate row in the skill table with its own cost and requirements. A fourth kind of row, level
  100, holds the effects monsters and items use (sleep, stun and the like).
- **Learning** costs skill points and needs a high enough character level, the right race, class and mode. The server does not insist
  that levels are learnt in order, so a modified client can skip straight to level 3.
- **Cooldowns freeze while you are offline.** The server saves the seconds left, not the time they end.
- **Quick slots are the client's business.** The server stores whatever the client sends and gives it back at login.
- **Passive skills** raise numbers for as long as they are known: weapon and shield mastery, extra bags (up to five), sitting recovery,
  attack speed with a weapon type, and stat or attack bonuses.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s base
stats on; see [Character numbers](stats.md)); the names are the PDB's.

## 1. Definitions

Confirmed, `CGameData::LoadConstSkill` 0x4bffe0, `CGameData::GetSkillInfo` 0x402770.

`EXEC usp_Read_SkillMaxID_R` sizes the tables, then every row of the skill table is read column by column into a 160-byte `stSkillInfo`. The
columns are read in table order; the community column names mostly match the fields, with two exceptions:

| column (`defs.skills`) | field | notes |
|---|---|---|
| `skillid`, `skilllevel`, `skillname` | `wSkillID`, `bySkillLV`, `szName` | |
| `country` | `byCountry` | who may learn it, by race (section 3) |
| `attackfighter` … `defensemage` | `byJob[6]` | 1 = this job may learn it |
| `prevskillid`, `reqlevel`, `grow`, `skillpoint` | `wPrevSkill`, `wLevel`, `byGrow`, `byPoint` | |
| `typeshow`, `typeattack`, `typeeffect`, `typedetail` | `byTypeShow`, `byTypeAttack`, `byTypeEffect`, `wType` | |
| `needweapon1` … `needweapon15`, `shield` | `byNeedWeapon[16]` | |
| `sp`, `mp`, `readytime`, `resettime`, `attackrange` | `wNeedSP`, `wNeedMP`, `byReadyTime`, `wResetTime`, `byAttackRange` | |
| `statetype`, `attrtype`, `disable` | `byStateType`, `byAttrType`, `wDisable` | |
| `successtype`, `successvalue`, `targettype`, `applyrange`, `multiattack`, `keeptime` | `bySuccessType`, `bySuccessValue`, `byTargetType`, `byApplyRange`, `byMultiAttack`, `dwKeepTime` | |
| `weapon1`, `weapon2`, `weaponvalue`, `bag` | `byWeapon[2]`, `byWeaponValue`, `byBag` | |
| **`arrow`** | **`wAggro`** | not an arrow count |
| `damagetype`, `damagehp/sp/mp`, `timedamagetype`, `timedamagehp/sp/mp`, `adddamagehp/sp/mp` | `byDamageType`, `wDamage[3]`, `byTimeDamageType`, `wTimeDamage[3]`, `wAddDamage[3]` | |
| `abilitytype1..3`, `abilityvalue1..3` | `stAbility[3]` (`byType`, `wValue`) | |
| `healhp/sp/mp`, `timehealhp/sp/mp` | `wHeal[3]`, `wTimeHeal[3]` | |
| `defencetype`, `defencevalue`, `limithp` | `byDefenseType`, `byDefenseValue`, `byLimitHP` | |
| **`fixrange`** | **`byDontRemByDeath`** | 1 = kept on death, and `keeptime` is in hours |
| `changetype`, `changelevel` | `wChangeType`, `wChangeLevel` | |

After reading a row:

- `byWeapon[i]`: below 1 → 0, above 19 → 19 (item real types, `docs/original/shop.md`).
- `byDefenseType` and **`byTypeAttack`** are clamped to 1..3 and then lowered by one, so the stored attack kind is 0 melee, 1 ranged, 2 magic.
- `byAttrType` 9 or more → 0; `byStateType` above 15 → 0; `byAttackRange` at most 60.
- An ability with a type but value 0 gets value 1.
- `wType` 93 sets `byTypeEffect` 4; `wType` 122 sets `byTypeEffect` 6.
- `byDontRemByDeath` = 1: `dwKeepTime × 3600`.
- **Only levels 1 to 3 are stored** here, at `[level − 1][skill id]`. `GetSkillInfo(id, level)` answers for levels 1 to 3 from this table
  (an id with `wSkillID` 0 there is absent) and for level **100** from a second table (the level-100 rows are sleep, stun, silence and the
  like, used by monsters and items; later parts). Any other level has no definition.

The data (`defs.skills`, 1,333 rows): levels 1, 2 and 3 have 326 rows each, level 100 has 355. `typeshow` 0 (338 rows) marks skills that
cannot be learnt (section 3).

## 2. What a character keeps

Confirmed, `stUserInfo`, `CSkill`, `CUser::PacketUserDBSetInfo` 0x46d780.

- **Skills**: `m_nNumSkill` (`+0xac0`) and `m_pSkill[256]` (`+0xac4`) of `CSkill` (`m_nSkillID`, `m_nSkillLV`, **`m_nNumber`** = the index in
  `m_pSkill`, **`m_dwResetTime`** = the tick from which it can be used again, the definition). A new skill gets number `m_nNumSkill` (so
  numbers run 0, 1, 2, … in the order skills were learnt); a higher level keeps the number.
- Stored by the database agent as `DB_SKILL_UNIT` (`u16 skill id, u8 level, u8 number, u16 delay`); on loading (`0x0400`-family reply) a skill
  without a definition is skipped and the rest are put at `m_pSkill[number]` with **`m_dwResetTime = now + delay × 1000`**. Saving (`TP_DB_SKILL_SET_LIST`,
  `u8 number, u16 delay` each) stores the remaining seconds, so **cooldowns stand still while the player is offline**.
- **Quick slots**: `m_nNumQuick` (`+0xec4`) and `m_pQuick[128]` (`+0xec8`) of `stQuick` (`u8 bar, u8 slot, u8 bag, u16 number`). They are
  the client's: the server stores what the client sends (`docs/original/world.md` section 4: the client's `0x010B` in the world, `u8 count`
  below 128, then 5 bytes each), loads them back as they are and sends them in the burst (`0x010B`, `docs/original/enter-world.md`). Nothing
  checks them.
- Skill points: `m_wSkillPoint` (`+0x13a`), raised at level-up (`docs/original/combat.md` section 5) and saved with the character.

## 3. Learning: `0x0209`

Confirmed, `CUser::PacketMaininterface` case 0x468a4e, `CUser::LearnSkill` 0x4842e0, `CUser::EnableLearnSkill` 0x484080.

Client → server, 5 bytes (`TPC_USER_GETSKILL`): `u16 skill id, u8 level`. **Ignored while a skill reset is pending (`m_bResetSkillWait`) or
while dead (status 1).**

Reply **`0x0209`** (`TP_USER_GETSKILL`, 7 bytes): `u8 result, u8 number, u16 skill id, u8 level`. Results: **0** learnt, **1** not enough skill
points, **2** level too low, **3** cannot learn. Every failure reply has number 0 and echoes the requested id and level.

1. Level not 1 to 3 → result 3. No definition for (id, level) → result 3.
2. Look the id up among the character's skills.
   - **Not known:** a level other than 1 gets **no reply**. Level 1 → the checks below; on success a new `CSkill` (number = count, cooldown
     ready now) is added.
   - **Known:** if its level is at or above the requested one → result 3. Otherwise the checks below; on success the same `CSkill` takes the
     new level and definition and its cooldown is ready now. **The requested level is not required to be the next one** (section 7).
3. The checks (`EnableLearnSkill`), in this order:
   1. skill points below `byPoint` → result 1;
   2. character level below `wLevel` → result 2;
   3. `byCountry` against the race (0 human, 1 elf, 2 vail, 3 deatheater): 0 → human only, 1 → elf only, 2 → human or elf, 3 → deatheater
      only, 4 → vail only, 5 → vail or deatheater, 6 and above → anyone. **A mismatch gets no reply.**
   4. `byJob[job]` 0 → result 3;
   5. mode (`m_byGrow`) against `byGrow`, the lowest mode the skill needs: easy needs `byGrow` 0; normal 0 or 1; hard anything but 3; ultimate
      anything. Otherwise result 3;
   6. `byTypeShow` 0 → result 3.

   `wPrevSkill` is **not** checked here (it is checked when the skill is used, part 2).
4. On success: skill points −= `byPoint`; the passive is updated (`ChkGetSkillPassive`, section 5); the database agent gets `0x0801`
   (`TP_DB_SKILL_ADD`: `u32 char id, u16 skill points, u16 skill id, u8 level`); the client gets `0x0209` with result 0, the number, id and level;
   a game-log record.

## 4. The skill list on entering

Confirmed, `CUser::SendCharacterSkill` 0x483540.

**`0x0108`** (`TP_CHAR_SKILL`), in the burst (`docs/original/enter-world.md` section 3, always sent): `u8 count` (= `m_nNumSkill`), then per skill
8 bytes: `u16 skill id, u8 level, u8 number, u32 delay`, delay = `(m_dwResetTime − now) / 1000` in whole seconds, at least 0. The passives are
applied before the burst (`InitEquipment`, then `InitPassive`, then the buffs), so the maxima in `0x0105` include them.

## 5. Passive skills

Confirmed, `CUser::InitPassive` 0x4899a0, `ChkGetSkillPassive` 0x489a40, `AddSkillPassive` 0x489a80, `RemSkillPassive` 0x489c80.

A skill is passive when `byTypeEffect` is 0. On entering, every passive is added (`InitPassive`). On learning one, the previous level's
effect is removed first (`RemSkillPassive` with the definition at level − 1, when that exists) and the new level's added. `wType`
(`typedetail`) chooses the effect:

| `wType` | effect | data |
|---|---|---|
| 1 | weapon mastery: `m_byWeaponDamage[byWeapon[i]] += byWeaponValue` for each non-zero `byWeapon[i]`; if the weapon in slot 5 has one of those real types, `itemAttack += m_byWeaponDamage[its real type]` | 27 rows |
| 2 | bags: **`m_nMaxBag = byBag`** (removing sets 2) | 3 rows, 3/4/5 bags |
| 4 | abilities: each of the three `stAbility` through `SetSkillAbility` (section 6) | 15 rows, types 2, 10, 11, 12, 23, 26, 29 |
| 5 | language: `m_nLanguage = level` | 3 rows; nothing in the server reads it |
| 6 | low-HP abilities: remembers (skill, level) in `m_pHPAbility`; the abilities apply while HP is below `byLimitHP` % | 15 rows; part 3 |
| 8 | weapon speed: `m_byWeaponSpeed[byWeapon[i]] += byWeaponValue`; if the weapon matches, attack speed += it and `SendSpeed` | 25 rows |
| 9 | `m_nCrossRoad += 1` (ranged attacks within 3 lose their × 0.7, `docs/original/combat.md`) | none in the data |
| 10 | shield mastery: `m_byShieldValue += byWeaponValue`; if the item in slot 6 has real type `byWeapon[0]`, `itemDefence += it` | 3 rows |
| 3, 7, others | nothing | |

Equipping uses the same arrays (`CUser::ItemEquipmentAdd` 0x454c70, removal mirrors it): a weapon put in slot 5 adds `m_byWeaponDamage[its
real type]` to `itemAttack` and `m_byWeaponSpeed[its real type]` to the attack speed; anything put in slot 6 adds `m_byShieldValue` to
`itemDefence`. So the passive bonuses follow the weapon and shield, whenever they change.

## 6. Abilities

Confirmed, `CUser::SetSkillAbility` 0x485e00.

`SetSkillAbility(type, value, byTypeEffect)` adds `value` (removing adds `−value`). Shared by passives and buffs. Types 1 to 3 change nothing
when `byTypeEffect` is 3; after a change of stats or attack numbers it runs `CUser::SetAttack`, after the maxima `SendMaxHP/MP/SP`, after speeds
`SendSpeed`.

| type | adds to | type | adds to |
|---|---|---|---|
| 1 | max HP | 2 | max MP |
| 3 | max SP | 4 | STR |
| 5 | REC, and max HP + 5 × value | 6 | INT |
| 7 | WIS, and max MP + 5 × value | 8 | DEX, and max SP + 5 × value |
| 9 | LUC | 10, 11, 12 | sitting recovery % of HP, SP, MP (`m_nSitRecHPP/SPP/MPP`) |
| 13, 14 | `m_nDecSP`, `m_nDecMP` (skill cost reductions, part 2) | 15 | attack range |
| 16 | attack speed | 17 | move speed |
| 18 | critical (`m_nCritical`) | 19 | `m_nCoolTime` (part 2) |
| 20, 21, 22 | hit add, melee / ranged / magic | 23, 24, 25 | attack add, melee / ranged / magic |
| 26, 27, 28 | defence add, melee / ranged / magic | 29, 30, 31 | evasion add, melee / ranged / magic |
| 32, 33, 34 | `nDisable` of melee / ranged / magic attacks | 35 to 46 | item-skill flags (`m_bSkillGetExpUP2X`, `m_bSkillNDQuality`, …, `m_bSkillAmulet` levels 40 to 42; `docs/original/item-use.md` section 7) |

The adds of types 20 to 31 are the `add` terms of the player's numbers (`m_pAttackAdd[kind]`: `nPerAttack` = hit, `nAttack`, `nPerDefense`
= evasion, `nDefense`), in the formulas of `docs/original/combat.md` section 4; critical is `trunc(LUC × 0.19 + m_nCritical)`.

## 7. The new server

- `defs.skills` is loaded at start (`pg::load_skill_defs`, levels 1 to 3, and 100 since the monster skills) with the column meanings and
  corrections of section 1 (`crates/game-server/src/skills.rs`).
- Learnt skills in `character_skills` (character, number, skill id, level, remaining cooldown in seconds as the original saves it), quick slots
  in `character_quick_slots` (character, position, bar, slot, bag, number; replaced as a whole on each `0x010B`), migration 0008; a learnt
  skill and the skill points left are written in one transaction.
- `0x0209`, `0x0108` and the `0x010B` save follow sections 2 to 4 (packets in `crates/shaiya-proto/src/skill.rs`); passive types 1, 2, 4, 8,
  9 and 10 feed the numbers that `crate::stats` and `crate::combat` compute from scratch (so removal and level changes need no
  bookkeeping), and ability types 10 to 12 the zone's sitting recovery; type 5 is kept but has no effect; type 6 is built with part 3
  (`buffs.rs`, `PlayerBuffs::low_hp_check`). A changed maximum or speed is sent as after an equipment change, before the `0x0209` answer.
- Cooldowns are saved since part 2: what is left of each, in whole seconds, on leaving the world (`save_cooldowns`, `character_skills.delay`).
- **Deviations**:
  - **a level can only be learnt after the one below it** (the original lets a hacked client buy level 3 straight away for level 3's points
    only, and then removes level 2's passive effect, which was never added, so the totals go wrong);
  - a request the original leaves without a reply (unknown skill at level 2 or 3, a race that may not learn it) gets result 3, so the
    client is not left waiting.

## 8. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by strings, constants, the call graph or switch tables, as those pages list).

### EP3

- **Definitions** (`CGameData::LoadConstSkill`, EP3 ps_game 0x497a70): the same 63 columns, levels 1 to 3 and 100, the same clamps for
  attack kind, defence type, range and the `wType` 93 and 122 rules (confirmed). Different (confirmed): weapons 1 to **15** (EP3 ps_game
  0x4984e1; EP4.5 1 to 19), `AttrType` above 4 becomes 0 (EP3 ps_game 0x49851c; five elements), and the record is **156 bytes** (EP3
  ps_game 0x49861a) against 160. Which field EP4.5 added was not identified.
- **Learning**: `EnableLearnSkill` (EP3 ps_game 0x468bc0) has the same constants (confirmed); `LearnSkill` (EP3 ps_game 0x468e20) has the
  level skip of section 3 (EP3 ps_game 0x468fcf, confirmed).
- **Abilities**: `CUser::SetSkillAbility` (EP3 ps_game 0x46a760) has the same switch over types 1 to 46 (confirmed).
- The skill list on entering, quick slots and passive skills were not compared.

### EP5.4

- **Nine skill levels.** `LoadConstSkill` (EP5.4 ps_game 0x4d8620) reads the same 63 columns with the same clamps, stores levels 1 to 9
  (EP5.4 ps_game 0x4d90c3) and 100, and `GetSkillInfo` (EP5.4 ps_game 0x41bb30) answers them (confirmed). The record grew to 164 bytes:
  a `TypeDetail` of 1000 or more is stored as `TypeDetail % 1000` with `TypeDetail / 1000` in the new field (EP5.4 ps_game 0x4d914e to
  0x4d916b, confirmed). Who reads the new field was not checked.
- **Learning** accepts levels 1 to 9 (EP5.4 ps_game 0x493419); otherwise the same checks, order and `0x0209` results, and the level skip
  of section 3 is still there (EP5.4 ps_game 0x49354f) (confirmed).
- **Abilities**: the switch has 49 cases (EP5.4 ps_game 0x495585, confirmed). 47 makes a counted player kill add 2
  ([`pvp.md`](pvp.md)); 48 adds to the damage reduction that EP4.5 sets only from items and lapis (read in `CUser::Damage`, EP5.4 ps_game
  0x458148); 49 adds the value to all six stats and 5 × the value to max HP, SP and MP (EP5.4 ps_game 0x495952). The data's types 50 and 51
  fall outside the switch and do nothing here (inferred).
- **Data** (`PS_GameDefs`, confirmed): skill ids 355 → 380, levels 4 to 9 added to old skills (415 rows), 1,270 of 1,298 shared rows
  changed (points, required level, damage, times, costs), and 1,955 junk rows with skill id 0.
