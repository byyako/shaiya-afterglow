# Player kills, part 1: normal attacks on players, death by a player, kill and death counts, the reward panels

This page covers fighting other players with normal attacks in the EP4.5 game server `ps_game.exe`: who may attack
whom, `0x0502` and the attack tick on a player, one blow (hit, damage, block, reflection), the damage record and who gets the kill, what
a death by a player does (experience, counts, quests, the items and gold the victim loses), the kill and death counters, and the kill and
death reward panels (`0x0218` to `0x021A`). It was read in the Ghidra decompiler and the disassembly with the PDB names.

The client side is on [World packets](../client/client-world-packets.md) (`0x0502`, `0x0515`, `0x0522`) and in the
[packet catalogue](../client/client-packets.generated.md) (`0x020E`, `0x0213`, `0x0218` to `0x021A`). The monster side of the same code is
on [Fighting monsters](combat.md) (attacks), [Monsters fighting back](mob-fight.md) (death and rebirth) and
[Parties](party.md) (shared experience and loot); this page gives what differs when the other side is a player.

Not covered here: skills on players, area skills and debuffs on players, and the faction kill count — see [part 2](pvp-2.md);
duels (`m_nPvPStatus` 1 to 4, `0x24xx`; see [Duels](duel.md)) and guild wars (5, `0x0D19`; see
[Guild wars](guild-war.md)); victories and defeats (the third and fourth counters, which only duels and guild wars move); item
option skills on hits (`CUser::AttackItemOption`, `BeAttackedItemOption`, as for monsters; see [Item option skills](item-options.md));
`NEW_SERVER` double kills.

## In short

- **Enemies are the other faction.** Players of the two countries can attack each other on every map except 40 and 42; players of the
  same country never can, outside duels and guild wars.
- **Player blows have no level factor**, unlike blows on monsters, and the hit chance runs from 1 % to 99 %.
- **The kill goes to whoever did the most damage**, not to whoever landed the last blow. A monster that finishes off a player who was
  mostly hurt by an enemy still leaves the kill to that enemy.
- **Dying to a player costs the usual 5 % experience** at rebirth, and above level 10 usually an item (a weapon, armour or accessory, not
  bound) and sometimes gold. A high Vile Gauge (`VG`) value makes losing an item less likely.
- **Kills and deaths are counted** up to 200,000 each. Reward panels pay stat points for kill milestones and gold for death milestones,
  in 16 steps.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's from the kill counts to
the PvP block and from the item-skill flags on; equal around `m_byUserPermission` `+0x5768` and `m_dwBillingID` `+0x578c`). "rand()" is
the C library's (0 to 32767); "trunc" is the x87 store with the rounding mode set to chop.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/client-world-packets.md` (`0x0502`, `0x0515`, `0x0522`) and
`docs/client/client-packets.generated.md` (`0x020E`, `0x0213`, `0x0218` to `0x021A`). `CUser` offsets are the code's (4 above the PDB's
from the kill counts to the PvP block and from the item-skill flags on; equal around `m_byUserPermission` `+0x5768` and `m_dwBillingID`
`+0x578c`). The monster side of the same code is in `docs/original/combat.md` (attacks), `docs/original/mob-fight.md` (death and rebirth)
and `docs/original/party.md` (shared experience and loot); this document gives what differs when the other side is a player.

Covered: who may attack whom, `0x0502` and the attack tick on a player, one blow (hit, damage, block, reflection), the damage record and
who gets the kill, what a death by a player does (experience, counts, quests, the items and gold the victim loses), the kill and death
counters, and the kill and death reward panels (`0x0218` to `0x021A`).

**Not covered** (part 2, `docs/original/pvp-2.md`, and later): skills on players (`CUser::AttackUser(now, CUser*, CSkill*)` 0x451c80 and
the skill branch of 0x44f4c0, area skills, debuffs on players); the faction kill count (`CWorld::AddKillCount`, `KillStatus.SData`, `stKCStatus`, `m_KCInfo`);
duels (`m_nPvPStatus` 1 to 4, `0x24xx`) and guild wars (5, `0x0D19`); victories and defeats (the third and fourth counters, which only
duels and guild wars move); item option skills on hits (`CUser::AttackItemOption`, `BeAttackedItemOption`, as for monsters);
`NEW_SERVER` double kills (below; not built by decision: the kill rate and kill boost of `docs/original/operator-console.md` instead).

"rand()" is the C library's (0 to 32767); "trunc" is the x87 store with the rounding mode set to chop.

## 1. Enemies

Confirmed, `CUser::IsTeam` 0x44bdf0, `CMap::LoadConfig` 0x426a4e.

`IsTeam(a, b)` is **true** (the same side: no attacks) when:

- `a` is `b`; or
- neither of these holds: a duel between them (`a.m_nPvPStatus` (`+0x54cc`) 4 with `a.m_dwPvPUser` (`+0x54d4`) = `b`), or a guild war
  between their guilds (both in guilds (`+0x1748`), each the other's `m_dwPvPGuildID` (`+0x54e4`), both `m_bPvPGuildReady` (`+0x54fc`));
  **and** either `a`'s map has a war type (`CMap::m_nWarType` `+0x7c`) or both are of the same country (`+0x12d`).

Otherwise false. **`Map.ini` `WarType = P`** sets the war type to 1 (any other value 0); maps **40 and 42** have it: there nobody is anyone's
enemy outside duels and guild wars. Outside duels (`docs/original/duel.md`) and guild wars (`docs/original/guild-war.md`), **players of
the two countries are enemies on every map but 40 and 42**, and players of one country never are.

## 2. Starting to attack a player: `0x0502`

Confirmed, `CUser::PacketPC` case 0x46a6b4.

Client → server, 6 bytes: `u32 player id`. Ignored (nothing at all) when the attacker is dead (status 1) or in status 3, has attack speed
255, sits (`+0x1434` = 1), is not on foot (vehicle status `+0x1460` ≠ 0), or already attacks this player (status 2 with target kind 1 and
this id). Then the target must be found **in the attacker's zone** (`CZone::FindUser`), **not dead**, **fully in the world** (`+0x5774` and
`+0x5775` both set, unless it is the attacker itself), **not in shape 13** (`+0x1354`), **not the attacker**, and **not of its side**
(`IsTeam` false). If not, only the move is cancelled. Otherwise exactly as `0x0503` (`docs/original/combat.md` section 1): status 0 →
**`0x0212`** and the recovery state "fighting" (at least 2); status 2 (attacking); target **kind 1** (player) and the id; normal attacks
next (`+0x143c` = 1, `+0x1440` = 1). **The move is cancelled in every case** (`CUser::CancleMove`, after the checks).

## 3. The attack tick on a player

Confirmed, `CUser::UpdateAttack` 0x48ce cases, target kind 1.

As for monsters (`docs/original/combat.md` section 2): attack speed 255, no weapon (`0x0502` result 7) and the timing are the same. A target
of kind 1 that is the attacker itself stops the attack. Then the target must pass the checks of section 2 again (in the zone, alive, in the
world, not shape 13, not the attacker, not of its side); **if not, the attack stops, nothing sent**.

**Range** (`CUser::EnableAttackRange(player)` 0x44bf90): horizontal distance between the two players' **current positions** (the target's
last known position, no interpolation) **≤ slack + the attacker's range (`m_nAttackRange`) + 2.3**; slack as for monsters
(`GetAttackEnableRange`). Unlike monsters, no size is added. Out of range: the next attack time as after an attack, stop, **`0x0502`
result 5** to the attacker (17 bytes, target 0, no damage).

In range: the blow (section 4); **when the target is dead after it, the attack stops**; the next attack time is now +
`g_ConstUserAttackSpeed[attack speed]` and the move is cancelled, as for monsters.

## 4. One blow

Confirmed, `CUser::AttackUser(CUser*)` 0x456870.

The kind is **1 (ranged) for weapons of real type 11, 13 and 14**, otherwise 0 (melee), as for monsters. Nothing happens when that kind is
disabled for the attacker (`m_pAttack[kind].nDisable`, `+0x13a4 + kind × 0x2c`).

1. **The target stands up** if it sits (`CUser::StatusStand`).
2. **Hit or miss** (`CUser::AttackSuccess(kind)` 0x456b10, target = the defender):
   - **block** first (`CUser::ChkBlock(kind)` on the target, `docs/original/skills-buffs.md` section 10): a blocked blow is a miss;
   - with `La`, `Lt` the attacker's and the target's levels and the numbers of `docs/original/combat.md` section 4 for the kind (both
     players):
     `rate = trunc(160 × La / (La + Lt) − 100 × target.hit / (attacker.evasion + target.hit) + 100 × attacker.hit / (target.evasion +
     attacker.hit))`, **clamped to 1..99** (monsters: 20..99); hit when **`rand() % 100 < rate`**.
   - A miss: **`0x0502`** (17 bytes: `u8 result` 3, `u32 attacker`, `u32 target`, `u16 0, 0, 0`) to the players within 60 of the
     attacker (`CZone::PSendView`), and the blow ends.
3. A hit **breaks the target's cast** (`CUser::StatusCastingStop`: casting, summoning a mount, riding, as a monster's hit,
   `docs/original/mob-skills.md` section 9). (A duel's leave check, `PvPChkLeaveByAttack`, follows here: `docs/original/duel.md` section 6.5.)
4. **Reflection** (`CUser::ChkMirror(kind)` on the target, `docs/original/skills-buffs.md` section 10): decided now, before the damage.
5. **Damage** (`CUser::AttackDamage(kind, no skill)` 0x457030), into `damage[3]` (HP, SP, MP; normal blows only HP), result 0 (hit):
   - `d = attacker.attack[kind] − target.defence[kind]` (whole numbers); plus `rand() % m_nItemAttackVar` when that is above 0; at least 0;
     **no level factor** (monsters have one);
   - ranged only: `d = trunc(d × 0.7)` when the attacker has no `m_nCrossRoad` and the two are **within 3** (≤ 3, horizontal);
   - element: `d = trunc(d × E)`, `E = m_AttackAttr[attacker's attack element][target's defence element]` when both are below 9, else 1
     (the target's defence element is the one monster blows use, `CUser` `+0x1284`);
   - `R = rand() % LUC + 1` (the attacker's LUC; 0 when LUC is 0); `d = trunc(R × 0.1 + d)`;
   - **critical** (`CUser::CriticalSuccess` 0x456fd0): chance `trunc(attacker.critical / 100 × 100 − target.LUC × 0.034)` **clamped to
     1..99**; when `rand() % 100 <` chance: `R2` like `R`, **`d = trunc(d × 0.5 + R2 × 1.4 + d)`** (ranged within 3: `trunc(d × 0.7)`
     again, without the `m_nCrossRoad` test), result 1;
   - at most 30000; **at most 10 becomes `rand() % 10 + 1`**.
   A critical wears the attacker's weapon (`CUser::QualityByAttack`, `docs/original/repair.md` section 2).
6. **Not reflected**: **`CUser::Damage(target, kind 1, attacker, damage)`** (0x44c1e0, as for a monster's blow,
   `docs/original/mob-fight.md` section 6 step 5: armour wear, the damage record (section 5), SP and MP, give-up SP/MP, absorption with
   `0x0525`, the party's HP view, the low-HP passive). When it kills: the attacker stops attacking, then **`CUser::Death(target, 1,
   attacker)`** (section 6).
7. **`0x0502`** (17 bytes: `u8 result` 0 or 1, `u32 attacker`, `u32 target`, `u16 damage[0], damage[1], damage[2]`; **all 0 when
   reflected**) to the players within 60 of the attacker. On a kill this comes **after** the death packets.
8. **Reflected**: `CUser::Damage(attacker, kind 1, target, damage)`; when it kills, **the target** stops its own attack and
   **`CUser::Death(attacker, 1, target)`**; then **`0x0515`** (16 bytes, `TP_PC_SKILL_MIRROR`: `u32 target, u32 attacker, u16 damage[3]`)
   to the players within 60 of the attacker (`CObjectConnection::PSendViewCombat`).
9. Item option skills of the two (`AttackItemOption`, `BeAttackedItemOption`): `docs/original/item-options.md`.

## 5. The damage record and the killer of record

Confirmed, `CDamage::Add` 0x4b8b60, `CDamage::GetMax` 0x4b8c40.

Every player keeps a record (`+0x1538`) of who hurt it: entries `(kind, id, total)`, kind 1 player, 2 monster, in the order of their first
blow. **`CUser::Damage`** (both forms) and **`CUser::DamageByKeepSkill`** add the blow's **HP damage as it came in** (before give-up SP/MP
and absorption) to the attacker's entry, a new entry at the end for a new attacker. This covers monsters too: monster blows, monster skills,
damage over time.

The record is **cleared** (`CDamage::Reset`) when the player **dies** (after the death's rules, section 6), and **whenever its HP comes
back to the maximum from below** (`SendRecoverAdd`, `SendRecoverAddSet`, `Heal`: natural recovery, potions, heals); also by duels and guild
wars (`docs/original/duel.md`, `docs/original/guild-war.md`).

The **killer of record** (`GetMax`) is the entry with the **highest total, the earliest on a tie**. At death (section 6) it decides who
killed the player, **whatever the last blow was**: a player who took most of the HP gets the kill even when a monster finished the victim,
and the other way round.

## 6. Death by a player

Confirmed, `CUser::Death(kind, id, flag)` 0x4586a0.

The start and the end are those of `docs/original/mob-fight.md` section 8: nothing for the dead; `m_nDropExp` 5 (0 with "Prevent EXP
Drop" or amulet 3, 1 with amulet 1 or 2); state dead, HP, SP, MP 0; trade, personal shop and shared mount cancelled. Then:

**With a record** (the usual case), its killer of record (`kind`, `id`) replaces the killer the call named:

- **kind 2** (a monster): a monster death as before (game log `0x301`).
- **kind 1**, the victim itself: nothing more (the flag below aside).
- **kind 1**, another player **not in the victim's zone** (`CZone::FindUser` on the victim's zone): nothing more.
- **kind 1**, another player in the zone, the **killer**. With `flag` set (`0` from every call in part 1): `m_nDropExp` −1 and the duel or
  guild war rules (`docs/original/duel.md` section 6.1, `docs/original/guild-war.md` section 6.2). Otherwise by
  `CUser::PvPGetDeathType(victim, killer)` 0x47b270: **4** when the killer duels the victim, **5** when their guilds are at war (each the
  other's enemy guild), otherwise **3**. Types 4 and 5: `m_nDropExp` −1 and `PvPChkLoseDeath` (the same documents).
  **Type 3** (the two countries, every kill in part 1), below. **`m_nDropExp` stays as it is: the victim loses experience at rebirth as
  after a monster** (and in ultimate mode the character is deleted, as there).

**Without a record** and killer kind 1 (another player): the victim's items and gold go as below (`ItemDropByUser(victim, killer)`), no
experience and no counts. (Rare: the record always holds the killing blow's attacker.)

The death packet **`0x0504`** (`u32 victim, u8 kind, u32 id`) names the **killer of record** (kind 1 = a player) when there is one, else the
killer the call named; to the victim's 5 × 5 view as before. The party gets `0x0C06` as before.

### 6.1 Type 3: experience and counts

The victim's experience value for a player of level `L` (`CUser::GetExp(victim, L)` 0x4581b0), with `V` the victim's level:
`d = V − L`; **above 9: 0**; else **`trunc(50 × V × ((d + 1) × 0.2 + 1.0))`, at least 1** (so a victim far below still gives 1, unlike a
monster's 0).

- **The killer in no party**: `CUser::AddExpFromUser(killer, trunc(GetExp(victim, killer level) × 1.2))` (the premium bonuses apply as
  for monsters; `docs/original/combat.md` section 5), then **`KillPCFrom(killer, victim)`** (the killer's kill and the victim's death,
  section 7).
- **The killer in a party** (party locked, slot order): each member **in range of the killer** (`EnableApplyRange(member, killer, 100)`,
  `docs/original/party.md` section 8: alive, in the killer's zone and map, within 100) gets
  `AddExpFromUser(member, trunc(trunc(GetExp(victim, member level) × share[count]) × gap(highest level − member level)))` (the tables of
  the party document; `count` = all members, at most 30, else 1.0; **no ×1.2**; a union uses `m_RaidPartyExp`, built,
  `docs/original/union.md` section 8). The **first** such member gets `KillPCFrom(member, victim)`, **each other** such member
  `KillPCTo(member, victim)`: **every member in range counts a kill**, the victim **one death** (none when nobody is in range).

Then, killer in a party or not, **`ItemDropByUser(victim, killer)`** (section 6.2).

### 6.2 What the victim loses

Confirmed, `CUser::ItemDropByUser` 0x45a180, `CUser::ItemDropByUserDeath` 0x45a5f0.

Nothing **at level 10 or below** (`m_wLevel` ≤ 10). Items, unless the victim has **"Prevent Item Drop"** (`m_bSkillNDItem`, code `+0x58c0`):

- **Players** (`m_byUserPermission` above 10): the chance by the victim's **VG** (`+0x16a`, signed, column `vg`): |VG| above 190: **65 %**,
  above 170: 70 %, above 140: 80 %, above 100: 85 %, above 50: 90 %, above 10: 95 %, else **100 %** (`rand() % 100 <` chance). Then one item
  at random (`rand() % n`) among the first **10** found in bags 1 to the bag count, slots 0 to 23, in order, whose definition has **real type
  1 to 25** (weapons, armour, accessories), **`byOG` 0** (column `reqog`; 1 013 items have 1) and **item type other than 94** (the guild
  gold bars).
- **Staff** (permission 10 or below): `rand() % 3 + 1` rounds, each one item at random among the first **25** in bags 1 to the bag count
  with item type other than 94 (any real type, `byOG` not looked at).

The item **leaves the slot whole** (`ItemDropByUserDeath`): ps_dbAgent `0x0702` (`u32 billing id, u8 bag, u8 slot, u8 count`: the
item's whole count, as for a drop), **`0x0206`** to the victim (`u8 bag, u8 slot, 0, 0, 0`: the slot is empty), the game
log `0x407`, and the same `CItem` (unique id, gems, craft name kept, as in `docs/original/item-drop.md`) goes on the ground at **the victim's
position ± 0.5 on x and z** (`CUser::GetDropPosRand` 0x48a270: `x + rand() % 1000 / 1000 − 0.5`, the same on z, y unchanged), **drop type
3**, drop id the victim, `CZone::EnterItem(kind 1)` (`0x0401` kind 1 to the view, removal in 60 s). **Owner**: the killer (no party); in the
killer's party, party id the killer's party and **the loot rule** of `docs/original/party.md` section 9 with the killer in place of the
monster's killer: **rule 1 (in turn): the first member in range from the turn gets it straight into its bags** (`CUser::ItemGet`); nobody in
range: the ground, owner the killer. (Rules 2 and 3 as there.)

**Gold**, unless "Prevent Item Drop", when the victim has any: **30 %** (`rand() % 100 < 30`): `loss = rand() % (5 × V) + 1`, at most the
money; money − loss, saved (`SendDBMoney`), **`0x0213`** (`u32 money`) to the victim. Then:
**the killer in a party**: `share = trunc(f32(loss) / f32(count))`, at least 1, to **each member in range of the killer** (100, as above) as
a pick-up of gold (`CUser::ItemGetMoney`, `docs/original/party.md` section 9; the remainder is lost); **otherwise** a gold item (type 26,
type id 0, count 1, the amount) on the ground at the victim's position ± 0.5, drop type 3, owner the killer, no party, kind 1.

## 7. The kill and death counters

Confirmed, `CUser::KillPCTo` 0x45aed0, `CUser::KillPCFrom` 0x45b050.

The character keeps `m_dwKillCount[4]`: **kills** (`+0x14c`, column `kills`, the original's `K1`), **deaths** (`+0x150`, `K2`), victories
(`+0x154`) and defeats (`+0x158`), all loaded and saved with the character; the first is in `0x0303` (`docs/original/world.md` section 5.5).

**`KillPCTo(killer, victim)`**: the killer's player-kill quest (`CUser::QuestUpdateKillPC`, `docs/original/quest.md` section 6); game log
`0x302`. When the kills are **below 200 000** (`g_nMaxKillValue`): kills + 1 (+ 2 while `NEW_SERVER` doubles them, at most 200 000; not
built by decision: the new server adds its kill rate, doubled by a kill-boost skill, `docs/original/operator-console.md` section 4), the faction kill count of the killer's country + 1 (`CWorld::AddKillCount`, part 2), ps_dbAgent **`0x060C`**
(`TP_DB_SET_KILLCOUNT`, 11 bytes: `u32 billing id, u8 index` 0, `u32 kills`), **`0x020E`** (7 bytes, `TP_USER_KILLCOUNT`: `u8 index` 0,
`u32 kills`) to the killer, and **`0x0522`** (10 bytes, `TP_USER_KILLINFO`: `u32 killer, u32 kills`) to the killer's 5 × 5 view (the
client keeps it on that player). At 200 000 nothing changes.

**`KillPCFrom(killer, victim)`**: `KillPCTo(killer, victim)`, then when the victim's deaths are **below 200 000** (`g_nMaxDeathValue`):
deaths + 1, `0x060C` index **1**, `0x020E` index **1** with the deaths to the victim.

(`NEW_SERVER`: `ps_game.ini` `[NEW_SERVER] Check`, 0 when absent as in the shipped files; when set, a timer switches kills to count twice
outside some hours. Not built, by decision: the kill rate (`kill_rate`, `.killrate`) and the kill-boost skills
(`kill_boost_skills`) of `docs/original/operator-console.md` section 4 instead; built.)

## 8. The reward panels: `0x0218`, `0x0219`, `0x021A`

Confirmed, `CUser::PacketMaininterface` 0x469a0c, 0x469a33, 0x469a5a.

Client → server, 2 bytes each (no payload). **Ignored in easy mode (grow 0) and below level 16.** The character keeps the kill and death
counts **at which it last took a reward** (`m_dwKillUsed[2]`, `+0x15c` and `+0x160`); the database keeps the **step** reached
(`Chars.KillLevel`, `DeadLevel`, tinyint).

The steps are fixed in the binary (16 each; `g_nMaxKillGrow` = 16 for each mode):

| step | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| kills (0x541d74, 20-byte rows) | 1 | 50 | 300 | 1000 | 5000 | 10000 | 20000 | 30000 | 40000 | 50000 | 70000 | 90000 | 110000 | 130000 | 150000 | 200000 |
| stat points, normal (grow 1) | 1 | 2 | 3 | 5 | 10 | 15 | 16 | 16 | 16 | 17 | 17 | 17 | 18 | 18 | 18 | 19 |
| hard (2) | 1 | 3 | 5 | 7 | 12 | 15 | 18 | 21 | 21 | 22 | 22 | 22 | 23 | 23 | 23 | 24 |
| ultimate (3) | 1 | 3 | 5 | 7 | 12 | 18 | 20 | 22 | 24 | 26 | 26 | 26 | 27 | 27 | 27 | 28 |
| deaths (0x541eb4, 12-byte rows) | 1 | 50 | 300 | 1000 | 5000 | 10000 | 20000 | 30000 | 40000 | 50000 | 70000 | 90000 | 110000 | 130000 | 150000 | 200000 |
| gold | 100 | 200 | 500 | 1000 | 5000 | 10000 | 20000 | 50000 | 100000 | 200000 | 300000 | 400000 | 500000 | 650000 | 800000 | 1000000 |

"The next step" for a used count `u`: the first step whose count is above `u` (step 1 for `u` = 0).

- **`0x0218`** (`CUser::StatusKillDeathResult` 0x48c8a0): **`0x0218`** (4 bytes, `TP_STATUS_RESULTINFO`: `u8 kill step, u8 death step`) with
  the next step of each (`GetKillStepEnable`, `GetDeathStepEnable`): 1 to 16, and when every step is taken **17 for kills but 11 for
  deaths** (an original quirk, kept: it only changes what the panel shows).
- **`0x0219`** (`CUser::StatusKillResult` 0x48c640): **nothing at all when the kills equal the used count.** Otherwise
  (`ChkKillResult`): when the next step exists and the kills reach its count, the reward is that step's stat points for the mode: stat
  points + reward (`u16`, saved, `SendDBStatPoint`), the used count = the step's count, ps_dbAgent `0x060C` index **4** with the **step
  number**, game log `0x30b`. **`0x0219`** (4 bytes, `TP_STATUS_RESULTKILL`: `u8 ok` 1 when a step was taken, `u8 stat points` given) — `0, 0`
  when the kills fall short. **One step per request.**
- **`0x021A`** (`CUser::StatusDeathResult` 0x48c750): the same with the deaths (`ChkDeathResult`): money + the step's gold (at most
  2³² − 1), saved (ps_dbAgent `0x0603`), `0x060C` index **5** with the step, game log `0x30c`. **`0x021A`** (7 bytes,
  `TP_STATUS_RESULTDEATH`: `u8 ok, u32 gold given`).

**Loading** (`CUser::PacketUserDBSetInfo` 0x46d8a5): the saved kill step, at most 16 (0 in easy mode, `GetMaxStatByGrow`), becomes the used
count = that step's kill count (0 for step 0); the death step likewise (`GetDeathStepCount`: 0 outside 1 to 16).

**The stat reset** (`CUser::StatResetStatus` 0x480660, `docs/original/item-use.md` section 8) sets the used kill count to **0** and saves
step 0 (`0x060C` index 4): the kill rewards can be taken again from step 1 (the reset gave the points back by level only). The death steps
stay.

## 9. The new server

**Built in part 1**: sections 1 to 8 for normal attacks, with these parts as the original:

- `0x0502` and the attack tick on players (enemies by country; maps 40 and 42 have no enemies: `WarType` imported);
- one blow with block, reflection, critical wear, armour wear, give-up SP/MP, absorption, the low-HP passive, the cast break and the ride
  rules of a monster's hit; the packets in the original order;
- the damage record on every player (monster and player blows, damage over time), cleared at full HP and at death, deciding the killer;
- death by a player: no experience change rules other than the original's (the victim still loses its 5 % at rebirth), the killer's or the
  party's experience, the counts with `0x020E`, `0x0522` and the database, the player-kill quests, the item and gold loss with the loot
  rules;
- the three panels, the steps saved as the original's `KillLevel` and `DeadLevel`, the stat reset's kill step.
- the desperate counterattack's death (`docs/original/skills-use.md` section 8) adds the caster's own HP to its record and is decided by
  the killer of record as any other death (the earlier deviation naming the caster always is gone).

Data: `characters` gets **`kill_level`** and **`dead_level`** (`smallint`, 0 to 16, default 0); the four counters already exist. The kill
and death counters are saved when they change (the original's `0x060C` at once).

**The rewards are claimed in one write** (audit R12): the death reward's gold and its step, and the kill reward's stat points and its
step, are each one transaction that holds only while the step in the database is still the one before (a second claim of the same step
changes nothing), and the stat reset saves its points, base stats and the first step together. The original saves the gold and the
step as two messages (`0x0603` and `0x060C`), so a failure between them offered the reward again.

**Rules** (`docs/server/rules.md`, an addition): between enemies by faction only, `0x0502`, the attack tick, skills, area effects, thorns and fields ask the `deny attack` and `level_gap` rules (a duel and a guild war are agreed to and never ruled); `pvp_gain` rules change the killer's experience (section 6.1), the counted kill (section 7, 0 counts nothing) and whether the victim's death is counted. A refused request gets a whisper from `Server`.

**Not built**: `NEW_SERVER` (by decision, the kill rate instead). The rest listed at the top is built: skills on players and the faction kill count (part 2),
duels with their victories and defeats (`docs/original/duel.md`), guild wars (`docs/original/guild-war.md`), item option skills on hits
(`docs/original/item-options.md`).

## 10. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by strings, constants, the call graph or switch tables, as those pages list).

### EP3

- The kill count exists: `data/KillStatus.SData` (string EP3 ps_game 0x4eb854), `CWorld::AddKillCount` (EP3 ps_game 0x410780) and the
  console's `enable/disable killcount` (confirmed).
- The PvP `AttackDamage` (EP3 ps_game 0x440ed0) has the same switch and the cap 30000 (EP3 ps_game 0x441128), with five elements
  (confirmed).
- The death drop (`ItemDropByUser` EP3 ps_game 0x443df0, `ItemDropByUserDeath` 0x444290) never reads the VG, so the honour rule of
  section 6 is not in EP3 (confirmed): a player loses an item with a flat 40 % (EP3 ps_game 0x443efa) and gold as in EP4.5 (30 %,
  0x444048). Details in [`item-flags.md`](item-flags.md), Other episodes.
- The kill counter, the damage record and the reward panels were not compared.

### EP5.4

What a counted kill adds in `CUser::KillPCTo` (EP5.4 ps_game 0x467ce0; confirmed at EP5.4 ps_game 0x467d93 to 0x467e0f):

| step | EP4.5 | EP5.4 |
|---|---|---|
| base | + 1 | + 1, or + 2 while a skill (ability type 47) is on |
| `[NEW_SERVER]` doubling | + 1 more while set | gone (the string is not in the image) |
| event bonus | none | + the `/killcnt` value inside its hour window |
| map bonus | none | + the map's `/killupmap` value |
| map curse | none | + a byte set on entering a map with `MapCurse` ([`maps-npcs.md`](maps-npcs.md)) |
| cap | at most `g_nMaxKillValue` | at most the maximum, once at the end |

The rest is unchanged: the faction kill count + 1 (not the bonus), `0x060C`, `0x020E` and `0x0522` (confirmed). The player-kill quest
update is skipped when the character has no such quest (EP5.4 ps_game 0x467d00), and the game log record grows from 0x75 to 0x82 bytes
(confirmed). An item skill of one kind is refused while the kill doubler is on or the kills are at a limit (EP5.4 ps_game 0x4726c1 to
0x4726ea, confirmed for the compares). The PvP damage cap is 60000 (EP5.4 ps_game 0x463dc0), the death drop is unchanged
(EP5.4 ps_game 0x4670a0), and the reward panels make the same calls (inferred, offsets moved).
