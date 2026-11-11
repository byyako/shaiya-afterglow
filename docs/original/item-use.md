# Using items: `0x050A`, `0x0557`, `0x0559`, and the stat and skill resets

This page covers how the EP4.5 game server `ps_game.exe` handles using an item: the three requests, who may use an
item, the reuse groups (shared cooldowns), using one up, every item effect (`bySpecial`) the code knows, the stat and skill resets, and
the free reset requests (`0x0214`, `0x0215`). It was read in the Ghidra decompiler and the disassembly with the PDB names, and in
`ps_dbAgent.exe` for the reset saves. The client side is in the [packet catalogue](../client/client-packets.generated.md).

Not covered here: appearance, sex and name changes (`0x0226`, specials 44, 45, 51; see [Items that open a window](item-windows.md)),
the boxes (special 42; see [New items and crafting](item-origins.md)), the saved positions of the Teleportation Stone
(special 39; see [Items that open a window](item-windows.md)), and game logs (see [The game log](game-log.md)).

## In short

- **Potions and food** add HP, SP or MP: fruit and herbs a fixed amount, the healing, stamina and mana potions a percentage of the
  maximum. They are used up even at full health.
- **Shared cooldowns.** Items belong to *reuse groups*. Using one item of a group starts a timer for the whole group, from 2 seconds to 10
  minutes. If the item's effect then fails, the timer is put back.
- **Cure scrolls** remove matching debuffs; two kinds also give a one-off protection against a monster's dying or death attack. They are
  used up even when there was nothing to cure.
- **Buff items** (EXP stones, Prevent Item Drop and the like) are refused while a buff of the same kind is already running.
- **Recall runes and stones** move the player after a 5-second cast. Stepping, attacking or using a skill cancels the cast, and the item
  is only used up when the move happens.
- **Stat and Skill Reset Stones** give back all points for the character's level: stat points 5, 7 or 9 per level by mode, skill
  points 3, 4 or 5 per level plus 5. Every character also gets one free skill reset.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's; the PDB's names are 4 bytes lower from `m_pUserItem` on (see [NPC shops](shop.md)), but not
everywhere (`m_dwQuestLastUseItemTime` `+0x57d0` and `m_byQuestLastUseItemID` `+0x57d4` are not shifted), so the names below are matched
by use. `stItemInfo` as in the PDB: `byIG` `+0x32`, `shVG` `+0x34`, `byRange` `+0x42` (u16), `byAttackTime` `+0x44`, `bySpecial` `+0x46`,
`wConstHP/SP/MP` `+0x6a/+0x6c/+0x6e`, `nRealType` `+0xa0`. In the item definitions table these are the columns `reqig`, `reqvg`, `range`,
`attacktime`, `special`, `consthp`, `constsp`, `constmp`.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`tools/disasm.py`), and from
`ps_dbAgent.exe` for the reset saves. Client side: `docs/client/client-packets.generated.md`. `CUser` offsets are the code's; the PDB's names
are 4 bytes lower from `m_pUserItem` on (`docs/original/shop.md`), but not everywhere (`m_dwQuestLastUseItemTime` `+0x57d0` and
`m_byQuestLastUseItemID` `+0x57d4` are not shifted), so the names below are matched by use. `stItemInfo` as in the PDB: `byIG` `+0x32`,
`shVG` `+0x34`, `byRange` `+0x42` (u16), `byAttackTime` `+0x44`, `bySpecial` `+0x46`, `wConstHP/SP/MP` `+0x6a/+0x6c/+0x6e`, `nRealType`
`+0xa0`. In `defs.items` these are the columns `reqig`, `reqvg`, `range`, `attacktime`, `special`, `consthp`, `constsp`, `constmp`.

Covered: the three requests, who may use an item, the reuse groups, using one up, every `bySpecial` the code knows (sections 4 to 10; some
are described but not built, section 11), the stat and skill resets and the free reset requests (`0x0214`, `0x0215`). **Not covered:**
appearance, sex and name changes (`0x0226`, special 44, 45, 51), the boxes (special 42: `docs/original/item-origins.md` section 5), saved positions for
the teleportation stone (special 39), game logs.

## 1. The requests

Confirmed, `CUser::PacketPC` 0x46a230.

| C → S | bytes | payload | then |
|---|---|---|---|
| **`0x050A`** | 4 | `u8 bag, u8 slot` | `ItemUse(bag, slot, the player's own id, 0)` |
| **`0x0557`** | 8 | `u8 bag, u8 slot, u32 target` | `ItemUse(bag, slot, target, 0)` |
| **`0x0559`** | 9 | `u8 bag, u8 slot, u8 kind, u32 target` | `ItemUse(bag, slot, target, kind)`; while riding (`m_nVehicleStatus` `+0x1460` ≠ 0) the error answer (section 3) instead |

All three are **ignored while dead** (`+0x142c` = 1), **when the bag is 0** (the equipment) or the place is not a bag (`CUser::IsBag`: bag up to
`m_nMaxBag`, slot below 24, `docs/original/items.md`). Before `ItemUse` the trade is cancelled (`CUser::CancleActionExc`) and the personal shop
closed (`stMyShop::Ended`). `target` is a player for specials 32 and 41, a teleport slot (1 to 4) for 39, and the target of an item skill for
`0x0559` (`kind` 0 oneself, 1 a player, 2 a monster).

## 2. Who may use an item

Confirmed, `CUser::ItemUse` 0x4655e0.

1. Nothing (no answer) while dead, for an empty slot or a slot whose count is 0.
2. **Quest items** (item types 27, 28, 29, real type 27): the item's type id and **now + 300 s** are remembered (`m_byQuestLastUseItemID`,
   `m_dwQuestLastUseItemTime`) for quests that start by using an item (`docs/original/quest.md` section 5, start type 4); **type 29 is also used
   up** (section 3), types 27 and 28 stay. No other answer.
3. Any other item that is not real type **26** (item types 25, 44, 100): the error answer.
4. Level: `wLevel` (`reqlevel`) above the character's level → error. Country (`byCountry`) against the race as for equipping
   (`docs/original/items.md` section 3: 0 human, 1 elf, 2 human or elf, 3 deatheater, 4 vail, 5 vail or deatheater, other values everyone) →
   error. Mode: `byGrow` above the character's mode → error. (Classes and sex are not checked.)
5. **Reuse group** `byIG` (`reqig`), clamped to 11. For a group above 0: if now is before the group's time (`m_dwNextPotionReuseTime[group]`,
   `+0x1488`) → error; otherwise the group's time becomes **now + `g_pPotionReuseDelay[group]`**:

   | group | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 to 9 | 10 | 11 |
   |---|---|---|---|---|---|---|---|---|---|---|
   | delay (ms) | none | 15000 | 20000 | 25000 | 30000 | 60000 | 120000 | 0 | 600000 | 2000 |

   **When the effect then fails** (any error after this point), the group's time is put back to what it was.
6. If the item names a skill (`byRange` ≠ 0 and `byAttackTime` ≠ 0: skill id and level), section 7; otherwise first the cures of section 5,
   then the effect by `bySpecial` (section 4). A special the code does not list gives the error answer.

## 3. Using up and the error answer

Confirmed, `CUser::ItemUseNSend` 0x465120, `CUser::ItemUseError` 0x4653a0.

**Used up:** the count drops by 1; at 0 the slot is emptied. ps_dbAgent gets `0x0702` (the slot's new count). **`0x050A`** (`TP_PC_USEITEM`, 11
bytes: `u32 player id, u8 bag, u8 slot, u8 type, u8 type id, u8 count left`) goes to the players within 60 of the user, itself included
(`CZone::PSendView`), and, when the caller asks for it, straight to the player too (used after moving to another map, when the player is no
longer in a zone).

**The error answer:** `0x050A` with the player's id and **bag, slot, type, type id and count all 0**, to the player only. Nothing else changes.

New server (no deviation, the packets keep their order): for the effects that save something lasting (fame, the stat and skill resets,
the free repair of everything), the item's new count is saved **before** the effect's own write and `0x050A` sent after the effect's
packets, so a failed write never leaves the effect saved and the item kept. The original saves the effect first.
The same order now holds for the rest of what lasts (audit R13): the **Goddess Blessing** (the check that counting is not frozen, the
item saved as used, then the grade raised; a freeze in between uses the item for nothing, with a warning), the **item buffs** and the
**experience wheel** (the checks, the item, then the buff and the lost experience), and the **moves by item** (recall and guild
house runes, teleportation stones and movement runes: the checks and the safe destination, a trade, duel window or shop that held the
item ended, the item saved as used, then the move, with `0x050A` after it as before). Left effect-then-use-up on purpose, as transient
or asking: Recover (HP, SP, MP), Cure, the resurrection of another player, the Message To Server and the party summon (it asks the
members and is only used up when someone was asked); the worst case is a free potion on a database failure, which ends the
connection anyway.

## 4. The effects by `bySpecial`

Confirmed, the `switch` in `CUser::ItemUse`.

Each effect that succeeds uses the item up (section 3), unless said otherwise; "error" is the error answer with the reuse time put back.

| special | items in the data | effect |
|---|---|---|
| 0 | fruit, herbs | `wConstHP/SP/MP` all 0 → error; otherwise those amounts are **added** (section 6) |
| 1 to 11, 18 to 23, 26 | cure scrolls and potions, protections | section 5 |
| 17 | healing, stamina, mana, super potions | **percentages**: `trunc(float(wConstHP × max HP) × 0.01f)`, the same for SP and MP (section 6) |
| 25 | none | weapon repair: the weapon's (slot 5) quality + `wQuality[0]`, at most its full quality (section 10); no weapon → error |
| 30 | Goddess Blessing | `CWorld::AddKillCountGrade(country, 5)`; refused → error (`docs/original/pvp-2.md` section 9) |
| 31 | Party Member Summon Stone | section 9 |
| 32 | Movement Rune | section 9 |
| 33 | Skill Reset Stone | while no skill reset waits (`m_bResetSkillWait`): set it, **skill reset** (section 8); otherwise error |
| 34 | Stat Reset Stone | while no stat reset waits (`m_bResetStatusWait`): set it, **stat reset** (section 8); otherwise error |
| 35 | Warehouse Recall Rune | (it names a skill, so section 7 applies; the special's own branch — used up when not trading — is not reached) |
| 38 | Message To Server | when no chat item is armed (`m_bUseChatItem`) and its time has passed: arm it (`docs/original/small-items.md` section 5) |
| 39 | Teleportation Stone | to the saved position `target` (1 to 4): section 9 with move type 2 (saved positions: `docs/original/item-windows.md`) |
| 40 | Repair Hammer | **every equipped item** (slots 0 to 12) back to full quality (section 10) |
| 41 | Target Resurrection Rune | section 10 |
| 42 | boxes and pockets | an item by grade from set `shVG` of `ItemCreate.ini` (`docs/original/item-origins.md` section 5): no such set → error; the roll gives grade 0 → used up with nothing; no item of the grade or no empty slot → error |
| 43 | Increase Fame | honor + `shVG` (`reqvg`) when that is above 0 and fits in a u16; `0x0230` (`u16 honor`); otherwise error |
| 46 | firecrackers | just used up (the client shows the firework from `0x050A`) |
| 47, 48, 49 | Capital, Bootleggery, Arena Recall Rune | section 9 |
| 50 | experience wheels | they name a skill (section 7; `docs/original/item-windows.md` section 3) |
| 51 | Name Change Stone | allows a rename (`docs/original/small-items.md` section 9) |
| 56 | Guild House Teleport Stone | section 9, with the guild house's rules in `docs/original/guild-house.md` section 7 |
| 59 | none | a move to **map 200**, the second server (not covered): `CUser::ItemUse` (0x4665a5) refuses it while `+0x57dc` is set or `+0x68` is -1, then sets up a move as the recall runes do from `g_ItemSpecialMoveWar` (0x5318a8, 16 bytes per country: map 200, position 0, 0, 0 for both), sends `0x0221` to the players in view (`PSendViewCombat`) and `0x0816` to the manager (`g_pClientToMgr`); the only item is 100169 (no name). Belongs with map 200 (README) |
| 36, 37, 44, 45, 57, 58, 62, 65, 67, 68 | linking and extraction hammers, resurrection runes, appearance and sex change, charms, … | **error**: these are used through other requests (`0x0805` lapis, `0x0553` rebirth, `0x0226`, …) |

## 5. Cures and protections

Confirmed, `CUser::ItemUseRemState` 0x4653e0.

Before the `switch`, for items without a skill. Each line ends with the item used up:

| special | removes the applied skills (buffs and debuffs) whose `byStateType` is | and |
|---|---|---|
| 1 to 6, 9 to 11 (and 12 to 15) | the special | |
| 7 | 7 | sets **`m_bBlockDying`** (`+0x1365`) |
| 8 | 8 | sets **`m_bBlockDeath`** (`+0x1364`) |
| 18 | 9, 10, 11 | |
| 19 | 1, 2, 5, 6 | |
| 20 | 3, 4 | |
| 21 | 14, 15 | |
| 22 | 12, 13 | |
| 23 | 7, 8 | sets both protections |
| 26 | **every debuff** (`RemStateSkillDebuffAll`: `byTypeEffect` 3) | sets both protections |

Removal (`CUser::RemStateSkill(int …)` 0x488590 and its overloads) is the dispel's: `0x0902` to ps_dbAgent, **`0x050E`** (`u32 key`) to the
player, the party told, and the effect undone (`CUser::CancleApplySkillDebuff`) — `docs/original/skills-buffs.md` section 9. The cure is used
up **even when nothing was removed**.

A protection is a one-off: a monster skill of `byStateType` 7 (with `m_bBlockDying`) or 8 (`m_bBlockDeath`) misses and clears it
(`docs/original/mob-skills.md` section 7 step 2). Both are cleared by a stat or skill reset (`CUser::SkillClearAll` 0x488350) and are not saved.

## 6. Adding HP, SP and MP

Confirmed, `CUser::SendRecoverAdd` 0x481f20.

The amounts are added, each capped at its maximum. When something changed: **`0x0505`** (`u32 id, i16 HP, MP, SP` gained) to the players within 60,
itself included, the party told (`SendEnergyAllToParty`), and the damage record cleared when HP just reached the maximum. **No `0x0521`.** The
potion is used up even at full health. The percentages of special 17 are truncated (the code sets the FPU to chop), and `0.01f` is a little below 0.01, so a round product
loses one: 30 % of 1000 HP gives 299 (the new server is exact: section 11, deviation 3).

## 7. Items that name a skill

Confirmed, `CUser::ItemUse`, `CUser::UseItemSkill` 0x464f00.

The skill is `CGameData::GetSkillInfo(byRange, byAttackTime)`; none → error.

- **Special 50** (experience wheels): only when the experience is at least halfway through the level (two `CGameData::GetExp` totals averaged)
  and the map is **18 or 30**, and `UseItemSkill` applies: then `0x0207` (gain = the level's total − exp) and exp = the level's total (built
  since, `docs/original/item-windows.md` section 3).
- **`kind` 0** (`0x050A`, `0x0557`, `0x0559` with 0): `UseItemSkill`; applied → used up, otherwise error.
- `kind` 1: the skill as an attack on the player `target` in the zone (`CUser::AttackUser`) when that player has no personal shop (`+0x15E0`, `docs/original/personal-shop.md` section 6; this said trading before); `kind` 2: on the
  monster `target` (`CUser::AttackMob`). Used up. (The snowball, item 44/147, skill 193; `docs/original/small-items.md` section 11.)

**`UseItemSkill`** applies only **buffs** (`byTypeEffect` 4), on the player itself (`CUser::BuffUser(self, 1)`, `docs/original/skills-buffs.md`),
and refuses (error) when:

- the skill has an ability of a kind already in effect on the player (`HasBuff` 0x464e40: any of its three abilities of that type with a value):

  | ability | 35 | 36 | 37 | 38 | 39 | 43 | 44 | 45 | 46 |
  |---|---|---|---|---|---|---|---|---|---|
  | flag | `m_bSkillGetExpUP2X` | `m_bSkillNDQuality` | `m_bSkillNDItem` | `m_bSkillNDExp` | `m_bSkillEnableBank` | `m_bSkillBank2X` | `m_bSkillMobGold` | `m_bSkillNDItemEQ` | `m_bSkillRebirth` |
  | items | EXP stones | Eternal Endurance | Prevent Item Drop | Prevent EXP Drop | Warehouse Recall | Double Warehouse | Gold Drop | Prevent Drop Equipped | Continuous Res Rune |

  (The flags are set by applying such a buff, `0x486000` onwards in `SetSkillAbility`, or by premium billing items.)
- the skill has ability **40, 41 or 42** (the charms: amulet level 1, 2, 3) and the player's amulet level (`m_bSkillAmulet`) is **at least** that;
- the player already has the **same skill at a higher level** applied (`CUser::HasBuffUpLevel` 0x464e80).

Otherwise the buff is applied as a buff skill on oneself: the same skill and level refresh, a lower level is replaced, the keep time from the
definition (in hours when `fixrange` is 1). Remedies, nostrums, candies and EXP stones are all `wType` 82 (abilities); others are 81 (heal over
time), 90 (the goddess transformations), 91, 92.

## 8. The resets

Confirmed, `CUser::StatResetStatus` 0x480660, `CUser::StatResetSkill` 0x480b20.

Both first stop an attack or cast (status 2 or 3 → 0), clear the applied skills' effects (`SkillClearAll`: abilities, stun, sleep,
protections, …; the applied skills themselves stay and are re-applied by `InitApplySkill`) and the passive skills, then recompute everything
(`InitEquipment`, `InitPassive`, `InitApplySkill`). With `n` = level − 1:

**Stat reset:** the base stats become the class's starting stats (the creation table, `docs/original/ps_game.md`), **+ `n` to the class stat**
(fighter STR, defender REC, ranger DEX, archer LUC, mage INT, priest WIS: what each level-up added) and the race bonus (human LUC + 4, elf DEX + 4,
vail INT + 2 and WIS + 2, deatheater STR + 2 and REC + 2); **stat points = 5 × `n`** (easy, normal), **7 × `n`** (hard), **9 × `n`** (ultimate).
The kill-count stat bonus used (`+0x15c`) is set to 0 (`0x060C` index 4 to ps_dbAgent; `docs/original/pvp.md` section 8). ps_dbAgent `0x0611`
(`TP_DB_SET_RESETSTAT`: points and the six stats). The player gets **`0x0214`** (`TP_USER_RESETSTAT`, 17 bytes: `u8 1, u16 stat points, STR, REC,
INT, WIS, DEX, LUC`), then its shape (`0x051D`), speeds (`0x051C` to the view), the three maxima (`0x050B` to the view), the current HP, SP, MP
capped at the new maxima and **`0x0521`** (absolute), and the party the energy and maxima.

**Skill reset:** every learnt skill is removed; **skill points = 3 × `n` + 5** (easy, normal), **4 × `n` + 5** (hard), **5 × `n` + 5**
(ultimate). ps_dbAgent `0x0612` (`TP_DB_SET_RESETSKILL`; it deletes all the character's skills, `Usp_Save_Char_Skill_Del_E`). The player gets
**`0x0215`** (`TP_USER_RESETSKILL`, 5 bytes: `u8 1, u16 skill points`), the shape, `0x051C` speeds to the view, the maxima, `0x0521` and the party updates. Applied buffs stay until they end; quick slots are left to the client.

**Waiting:** the item sets `m_bResetStatusWait` / `m_bResetSkillWait` until ps_dbAgent confirms the save (`0x0509`, `0x050B` / `0x050A`, `0x050C`
in `CUser::PacketUserDBSetInfo`). While a stat reset waits, `0x0208` (stat points) is ignored; while a skill reset waits, `0x0209` (learning) is
ignored and another reset item gives the error.

**The free resets** (`CUser::PacketMaininterface`), 2-byte requests:

- **`0x0214`** (free stat reset): while no stat reset waits, answered **`0x0214` with `u8 0`** (3 bytes): refused, always.
- **`0x0215`** (free skill reset): while no skill reset waits, when the character's free skill reset is unused (`m_byResetSkill` `+0x584d` = 0):
  a skill reset as above (saved with `0x0614`, `ResetSkillEvt`), and `m_byResetSkill` becomes 1. When used: **`0x0215` with `u8 0`** (3 bytes).
- At login `m_byResetSkill` is loaded (values above 1 become 1; `m_byResetStatus` is forced to 0) and `0x021B` sends `u8 −m_byResetStatus,
  u8 1 − m_byResetSkill` (`docs/original/enter-world.md` burst item 21): **`00 01` while the free skill reset is unused, `00 00` after**.
- The client (`docs/client/CLIENT-FACTS.md`, `0x580740`, `0x5528a0`) reads an **optional `u8` after the points** of `0x0215` and lowers its
  skill "chances left" count (the NPC type 13 menu, from `0x021B`) only when that byte is not 0; `0x0214` success always lowers the stat
  count. With the original's 5 bytes the menu keeps saying "1 chances left" after the free reset until the next login.

## 9. Moves by item

Confirmed, `CUser::ItemUse`, `CUser::UpdateResetPosition` 0x48d470, `CUser::CancleMove` 0x48a010.

These use the pending move of `docs/original/gatekeeper.md` (`m_nMovePositionType` `+0x5818`, its time `+0x581c`, map and position) and
remember the item's place, a result byte and the reuse group (`m_nSavePosUseBag/Slot/Index`, `m_dwItemCoolTimeIndex`, `+0x5860` to `+0x586c`).
**The item is used up only when the move happens.**

**The checks of the destination** (`ChkMoveMap` 0x4650c0, `CMap::EnableMoveByPortal` 0x4299b0):

- level caps: map **6 or 8 at most level 35**, map **18 at most 15**, maps **30, 31, 32 at most 30**;
- a map that portals lead to must have one open to the player's country (portal country 0 everyone, 1 Light, 2 Fury); a map no portal leads to
  is open (checked only when the map changes, except for the recall runes, which always check). **The list holds only the portals linked to
  obelisks** (`CMap::LoadObelisk`, `Data/Obelisk.ini`, whose captures open and close them), not every portal of the map files.

**Using the item:**

| special | destination | also needs | type, result byte |
|---|---|---|---|
| 47, 48, 49 | `g_ItemSpecialMoveCity/Secret/Arena[country]`: Light **map 35 (585.87, 27, 224.49)**, Fury **map 36 (549.1, 42, 325.43)**; Bootleggery **map 42**, Light (78.61, 15.36, 27.95), Fury (23.39, 15.36, 106.4); Arena **map 40**, Light (128, 3.5, 82), Fury (58, 3.5, 82) | the destination map loaded and of map type 0 | 3, 255 |
| 32 | the party member `target` (not the player), its map and position **at the time of use** | a party; the member's map not of type 5 or 3, type 6 only within the same guild, not map 67; no move already pending | 5, 101 |
| 31 | — (the party is asked, below) | a party; the player's map not of type 5 or 3 and not map 67; no move already pending | 4, 100 |
| 39 | saved position `target` | saved position set, inside the map, create type 0 | 2, `target` |

When accepted and **no logout is pending**: the move is due **5 s later**, and **`0x0221`** (`u32 id`, 6 bytes) goes to the players within 60,
itself included: the client shows the casting. With a logout pending the recall runes and the stone give the error; **the movement rune still
sends `0x0221` and answers nothing more** (no move is set; the reuse time stays used); the summon stone does not look at the logout.

**Cancelling** (`CancleMove`, on an accepted step `0x0501`, an attack or skill start `0x0502`/`0x0503`/`0x0511`/`0x0517`, a hit that breaks a
cast `StatusCastingStop`, the attack tick): a pending move of type 2 or more is dropped, the reuse group's time set to 0, and **`0x0222`** (`u32 id,
u8 0`) goes to the players within 60. (Type 0 is dropped silently; type 1, the gatekeeper's, is never cancelled.)

**When the move is due** (types 2, 3, 5): the item must still be in its place with the same special, otherwise **`0x0222` with 0** and the
reuse time put back to 0. Otherwise `0x0222` with the result byte to the players within 60, then: **the same map**: the move within the zone
(`CZone::MoveUser`, a position off the map fails) and `0x020B` to the view; **another map**: the player leaves the zone, `0x020B` to the player
only, and the item's `0x050A` is sent to the player directly (it is no longer in a zone). The item is used up.

**The summon** (type 4, when due): with the item still there and a party, each member that is **not the player, has no summon pending
(`m_dwPartyCallReqID`), is alive**, passes `ChkMoveMap` for the summoner's map and, on another map, `EnableMoveByPortal`, and — when **the summoner's** map is
of type 4 or 6 — is in the summoner's guild (two players without a guild count as the same), and (in a union) in the same group, gets **`0x0223`** (`u32 summoner id`, 6 bytes) and 35 s to answer
(`m_dwTimeOverPartyCall`). At least one asked → the item is used up; none → `0x0222` with 0 and the reuse time put back.

**The answer `0x0224`** (C → S, 3 bytes: `u8 answer`), with a summon pending: when the summoner is still in the world and its map is not of type
5 or 3, and type 6 only within the same guild, **the summoner** gets **`0x0224`** (`u32 member id, u8 answer`, 7 bytes); with **answer 0**
(accepted), the level cap and portal checks for the summoner's map, and no logout pending, the member gets a gatekeeper-type move (type 1, due at
once) to **the summoner's map and position now**. The request is cleared either way. **After 35 s without an answer** the summoner gets
`0x0224` with 1 (`CUser::UpdateTimeover` 0x48e310) and the request is cleared.

## 10. Other effects

Confirmed.

- **Repair Hammer (40)**: for every equipped item in slots 0 to 12, its full quality is its definition's `wQuality[0]` plus each lapis' `wQuality[0]`;
  a changed quality recomputes that slot's quality level; ps_dbAgent `0x070B` (`TP_DB_ITEM_REPAIR_ALL`, the changed slots); the player gets
  **`0x0554`** (`TP_ITEM_REPAIR`, 8 bytes: `u8 255, u8 255, u32 money`). Used up even when nothing needed repairing.
- **Target Resurrection Rune (41)**: `target` must be another player **in the same zone** (any distance), **dead** (`+0x142c` = 1, and not in the
  `+0x1430` = 2 state), of **the same country**. It is brought back to life where the user stands (`CUser::LifeSkill` 0x459d20, as the
  Resurrection skill: `0x0551` kind 4 with the user's position to the target's view, HP/SP/MP full).
- **Increase Fame (43)**: honor (`m_wHG`, `hg`) + `shVG`; the item list's 69 adds `reqvg`.

## 10a. The client side (from the client work, `docs/client/CLIENT-FACTS.md`, game.exe verified)

- Items are used with `0x050A`; specials **41 and 32 send `0x0557`** (41 only with a player selected, 32 with the chosen party member);
  specials 39, 44, 45 and 50 open a window first. A `0x050A` answer with **bag 0 is a failure** (message 227).
- The **Warehouse Recall Rune** (special 35, skill 228): the successful `0x050A` opens the warehouse window; the buff's `0x050D` sets the
  client's "warehouse anywhere" flag, so using another rune while it lasts reopens the window without a packet; its `0x050E` closes a
  rune-opened window. The warehouse itself is in `docs/original/warehouse.md` (`0x0711`, the moves).
- `0x0214` / `0x0215` answers as built; after a successful reset the client lowers its own `0x021B` counter when it is not 0, which fits
  `00 01` (free skill reset available) becoming used.

## 11. The new server

- `0x050A`, `0x0557`, `0x0559` as in sections 1 to 3, with the reuse groups kept per character in memory (not saved, as the original).
- Built: quest items (start type 4 quests now start), special 0 and 17 (potions), the cures and protections of section 5 (protections kept in
  the zone with the player's other state, honoured by monster skills), buff items with the refusals of section 7 (`kind` 0), specials 33 and 34
  with the resets of section 8, the free resets `0x0214`/`0x0215` and a correct `0x021B`, the recall runes (47 to 49), the movement rune (32), the
  summon stone (31) with `0x0223`/`0x0224` and its 35 s timeout, the Repair Hammer (40; a free repair of everything equipped,
  `docs/original/repair.md`), the Target Resurrection Rune (41), Increase Fame (43), firecrackers (46).
- Built later: specials 39 and 50, and the body change (44, 45) through `0x0226` (`docs/original/item-windows.md`); the Repair Hammer's
  repair and ability 36 (no wear) with wear (`docs/original/repair.md`).
- Built later (`docs/original/small-items.md`): special 38 (Message To Server, section 5), special 51 (Name Change Stone, section 9) and
  `0x0559` kinds 1 and 2 with the attack checks (section 11); special 25 is refused (no item has it, section 10). Specials 30 and 56: built
  with `docs/original/pvp-2.md` section 9 and `docs/original/guild-house.md` section 7.
- Not built (error answer, item kept): special 59, and the specials of other requests (section 4, last row). Of the item-skill flags of section 7, the warehouse ones (39 anywhere, 43 the second half) are honoured by the
  warehouse (`docs/original/warehouse.md` section 8); ability 36 (no wear) by the equipment wear; ability 37 (no item loss) by the item loss at
  death (`docs/original/pvp.md` section 6.2); the others (35 the EXP stones, 38 no experience loss, 40 to 42 the charms, 44 gold drop, 46
  Continuous Res Runes) are built with `docs/original/item-flags.md`; 45 has no effect in the original either.
- A reset is saved in one transaction (base stats and points, or the skills deleted and the points) **before** its answer; the connection
  handles nothing else meanwhile, so nothing needs to wait.
- The free skill reset is stored per character (`characters.free_skill_reset_used`, migration 0013). A skill reset leaves the quick slots
  to the client, as the original.
- Obelisks are built (`docs/original/obelisks.md`), their portal links are not (section 8 there; the shipped `Obelisk.ini` defines none), so no
  portal is linked to one and `EnableMoveByPortal` lets every move through, as the original does for a map no linked portal leads to.
- The moves by item are kept by the connection; a step, an attack, a skill or a monster's hit that breaks a cast cancels them. The summons
  waiting for an answer are kept by the zone (one per member), and a member that leaves the world drops its own.
- **Deviations** (two, approved):
  1. the movement rune with a logout pending gives the error answer and puts the reuse time back, as the recall runes do, instead of showing
     a cast that never ends in a move;
  2. after a reset, only the speeds and maxima that changed are sent (`renumber`, as for equipment and stat points), not all of them with the
     shape; the current HP, SP and MP (`0x0521`) always go.
- **Deviation 3** (requested): percentages (special 17) are exact, `percent × max / 100` truncated; the original's `0.01f` loses
  one on round products (30 % of 1000 HP gives 299 there).
- **Deviation 5** (proposed, built, approved): the free skill reset (`0x0215` from the client) is answered with 6 bytes, `u8 1, u16 points,
  u8 1`, so the client's "chances left" count goes to 0 at once; the Skill Reset Stone (special 33) keeps the original's 5 bytes and leaves
  the count alone. Display only: the server decides with `free_skill_reset_used` either way.
- **Deviation 4** (the skill-cut fix, approved): a move by item (specials 31, 32, 47 to 49) is refused with the error answer
  within **3 s of a skill request** (`0x0511`, `0x0517`, whatever became of it). See `docs/original/community-fixes.md`, "Skill-cut with runes":
  a cast that is cancelled costs nothing (the item stays, `CancleMove` frees the reuse group), so a rune can be used and stepped out of
  without end, and its casting cuts the client's skill animation short. The server still spaces skills by the attack speed
  (`m_dwNextAttackTime`, `docs/original/skills-use.md` section 4) in both servers; what the cut gains is the time the client's animation
  would otherwise hold the player back. The community patch (Cups, 2018, at `ItemUse` 0x465630 of this same build) does the same for item
  100133 alone and answers nothing.
- As in the original, a dead member's yes to a summon moves the body; it stays dead on the new map (the zone takes a player entering with
  no HP as dead).
- **An extension's effect** (an addition, `docs/server/effects.md`): an item the extensions' rows gave a `use` effect is carried out between the script (`game.on_item`) and the core's own effect, after the rules' `item_use` point and the reuse group; the core's refusal of a bare special-0 item no longer applies to it. A level stone is the reference. With no such extension nothing here changes.

## 12. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **Specials 0 to 56** (confirmed for the bound): the `bySpecial` switch of `CUser::ItemUse` (EP3 ps_game 0x44ccf0, matched by dispatch,
  `0x050A`) ends at 56 (`cmp ecx, 0x38`, EP3 ps_game 0x44d01c); EP4.5 ends at 59. The case list shows special 59 (the move to the war
  server) as the one EP4.5 case EP3 lacks; which cases are filled was not re-checked.
- **Item skills are self buffs only** (confirmed): there is no `0x0559` and no target kind 1 or 2 (EP4.5 0x465904 to 0x465982,
  `AttackUser`/`AttackMob`), so no item hits another target.
- **Boxes (special 42) refuse helmets and cloaks** (confirmed): a rolled item of real type 16 or 24 gives the error answer and the box is
  kept (EP3 ps_game 0x44db7a to 0x44db8c). EP4.5 has no such check.
- `UseItemSkill`, the resurrection runes and the free resets exist; they were not compared.

### EP5.4

- **Specials 0 to 74** (confirmed): `ItemUse` (EP5.4 ps_game 0x472da0, matched by the `0x050A` case) switches over 0 to 74 (`cmp ecx,
  0x4a`, EP5.4 ps_game 0x473167); every EP4.5 case keeps its body (same call sequence). New cases:

| special | EP5.4 ps_game | what it does (confirmed) |
|---|---|---|
| 68 | 0x4743f9 | resets every guild house NPC level of the guild ([`guild-house.md`](guild-house.md)), rank 1 only; used up only when a level changed |
| 70 | 0x47335d | one counted stat reset (below) |
| 73 | 0x47448c | summons monsters at the player from a new `Data/MobCreate.ini` set `shVG` (map below 200, allowed by the set); logged to the game log as record `0x0430` |
| 74 | 0x474072 | a recall to map 39 with the checks and 5-second cast of specials 47 to 49 (no item in the data uses it) |

- **Free resets are counted** (confirmed): `0x0214` and `0x0215` are both allowed while the character's used count is below a limit,
  3 each by default (EP5.4 ps_game 0x476fac, 0x476fff), set with the console words `/resetstatcnt` and `/resetskillcnt`; the counts are
  read from the character record at load, and `0x021B` carries `limit − used` for each. Comparing with EP5.4 showed that section 8's
  fixed refusal of the free stat reset was changed in the later build.
- **Boxes** (confirmed, EP5.4 ps_game 0x473aaa): `shVG` 1000 to 1255 is a new kind that rolls every line of a `RandomItemsCreate` set
  ([`item-origins.md`](item-origins.md)); a rolled cloak (real type 24) is refused in both kinds.
- **Item buffs** (confirmed): `UseItemSkill` (EP5.4 ps_game 0x4725b0, matched by call order) has the nine "same kind running" checks of section 7 and one new
  ability, 47, a doubled kill count (EP5.4 ps_game 0x4726c1).
- **Two new gates on `0x050A`** (confirmed): `CUser+0x5880` (a server transfer is pending, EP5.4 ps_game 0x478f9a) and `CUser+0x1358`
  (EP5.4 ps_game 0x478fa7), the lock of the new debuff kinds 74 to 76 that also blocks attacks, skills, `0x0557`, `0x0559` and `/town`.
- The tables of specials 47 to 49 are unchanged (EP5.4 ps_game 0x56e608, 0x56e628, 0x56e648). Specials 61, 76 to 78 and 85 to 98 are on
  EP5.4 items but unused by this build (confirmed): the item-use switch (EP5.4 ps_game 0x473163) sends every special above 74 and
  special 61 (byte table 0x474754) to its default, the error answer `0x050A` with zeros, item kept (0x47468a, 0x472b80), and no compare
  of the special byte elsewhere names them.
