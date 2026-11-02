# Skills, part 4: hiding, transformations, fields, one-off skills and the three-second effects

This page covers the special skills in the EP4.5 game server `ps_game.exe`: the player's shape and the packets that
show it, hiding (`wType` 88) and finding the hidden (93), transformations (90), what ends them, fields (`byTypeEffect` 5), the one-off
skills (6), and the effects that applied skills have **every three seconds** (heal over time, Fire Thorns, finding the hidden, damage over
time on monsters). It also covers the client's run toggle `0x0210` and its stop request `0x0212`. It comes from static analysis; nothing
was executed.

[Part 1](skills.md) has the definitions, [part 2](skills-use.md) using a skill, [part 3](skills-buffs.md)
buffs, applied skills and debuffs on monsters.

Not covered here: player kills (skills that hurt players; see [Player kills 2](pvp-2.md), and for Fire Thorns and fields
[Fire Thorns and fields hitting players](thorns-fields-pvp.md)), parties, mounts (see
[Mounts](vehicles.md)), the item skills (rows with `typeshow` 0: goddess forms, item hiding), not seeing monsters (89: no
learnable row), and monster skills (see [Monster skills](mob-skills.md)).

## In short

- **Shapes.** A character can be hidden (Stealth), disguised as an enemy player, turned into the look of a monster (Illusion), or
  evolved into a monster's form (Evolution). Other players see the new shape.
- **Hiding** keeps aggressive monsters and guards from noticing the player, and stops other players from attacking or even buffing them.
  Attacking, being hit by a monster, or an enemy's Detect ends it.
- **Transformations change how you fight.** An evolved or illusioned player fights with the monster's attack and defence instead of the
  equipment's; a disguised player has no equipment attack or defence at all while it lasts. Evolution is the only one that survives
  fighting.
- **Some effects tick every three seconds**: heal over time, damage over time, Detect, and Fire Thorns, which damages every enemy around
  the mage.
- **Fields** are placed at the caster's feet: a fire wall burns everything nearby every second for 12 seconds; a trap hits its first
  victim once and is gone.
- **One-off skills**: Resurrection brings a dead ally back at the caster's position with full HP and no experience lost; Scout reveals an
  enemy's level, mode and element defence; Return sends the caster to the map's bind point after the cast.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s map field on); `CMob`, `CSkill`, `CTrab` and `stSkillInfo` offsets
match the PDB. "trunc" and "rand()" as on [Fighting monsters](combat.md). Ticks are milliseconds (`GetTickCount`).

## 1. The data

Learnable rows (`typeshow` ≠ 0, levels 1 to 3) of the kinds covered here. Jobs as the six `byJob` flags (fighter, defender, ranger, archer, mage,
priest); a row with no job flag cannot be learnt.

| skill | kind / `wType` | target | job | what |
|---|---|---|---|---|
| 63 Stealth | 4 / 88 | 2 oneself | ranger | hiding, 10/20/60 s |
| 179 | 4 / 88 | 5 the party around | none | not learnable |
| 43 Detect | **4** / 93 (the data says 6; the loader makes every 93 a buff, part 1) | 2 | priest | finds the hidden within 5/10/15, 30 s |
| 107 Disguise | 4 / 90, `wChangeType` 100 | 2 | ranger | the look of an enemy player, 180/300/600 s |
| 77 Illusion | 4 / 90, `wChangeType` 102 | 2 | ranger | the look of a monster, 60/180/300 s |
| 127 Evolution | 4 / 90, `wChangeType` 10/11/12, `wChangeLevel` 2049/413/679 | 3 a player | priest | a monster's shape and fighting numbers, 60 s |
| 97 Polymorph | 3 / 90, `wChangeType` 101 | 3 | priest | turns an enemy player into a monster: player kills (`docs/original/pvp-2.md`) |
| 47 Fire Thorns | 4 / 84 | 2 | mage | **damage around oneself every 3 s** (section 7), 300 s |
| 113 Trap | 5 / 101 | 1 | ranger, archer | a trap at one's feet, 30 s |
| 169 | 5 / 101 | 1 | none | not learnable |
| 48 Fire Wall | 5 / 102 | 1 | mage | damage around a spot every second, 12 s |
| 126 Resurrection | 6 / 121 | 3 | priest | a dead player of one's side comes back where the caster stands |
| 125 | 6 (loader) / 122 | 3 | none | not learnable (`KillSoul` on an enemy's body) |
| 100 Scout | 6 / 123 | 3 | ranger | an enemy player's element defence, level and mode |
| 192 Return | 6 / 125 | 2 | all | back to the bind position after the cast (10/9/8 s) |

No learnable row has `wType` 89 or 124.

## 2. The shape

Confirmed, `CUser::GetShape` 0x48a300, `CUser::SendShape` 0x482590, `CUser::SendShapeMe` 0x482510, `CUser::SendUserShape` 0x482ad0.

`m_byShape` (code `+0x1354`) is **0** normally, **13** while hidden, and **`wChangeType`** while transformed (100 disguised, 101 polymorphed, 102
an illusion, 10 to 12 evolved). `m_nShapeType` (`+0x1358`) is `wChangeLevel` (a monster id), or for an illusion the copied monster's id.

`GetShape`: 13 while the character is invisible to others (a staff flag, `stAdminInfo`; `docs/original/gm-commands.md` section
13), the mount's shape while riding (`docs/original/vehicles.md` section 3), else `m_byShape`.

**`SendShape`** goes to the player's view (the 5 × 5 cells, `CZone::SendView`): for shape **102** **`0x0524`** (`TP_PC_SHAPE2`, 9 bytes: `u32
player id, u8 102, u16 m_wTransFormTargetMobID`), otherwise **`0x051D`** (`TP_PC_SHAPE`, 7 bytes: `u32 player id, u8 shape`). `SendShapeMe` is
`0x051D` to the player only (not used by the code covered here).

**`SendUserShape`** sends **`0x0303`** (the looks, as the reply to the client's looks request, `docs/original/world.md`) to the view. With shape
100 and a copied look (section 5.1) it carries **the copied player's** death flag, motion, country, race, hair, face, size, job, sex, party
type, mode, kill count, name, equipment and guild name, under the disguised player's own id. **`CZone::SendUserShape`** (the reply to a looks
request, 0x41fd00) does the same.

When the shape shows (verified, `CZone::SendMoveUser` 0x41f350, `CCell::SendCellInfoEnter` 0x423aa0, `CZone::EnterUser`):

- whenever it changes (sections 3 to 6);
- **when the player comes into another player's view** (entering a zone, moving between cells): after `0x0201`, a player whose shape is not 0
  is followed by **`0x051D`** with the shape, **even for 102** (no monster id: the original sends `0x0524` only on the change itself);
- entering a zone: `SendShape` to the view.

What a shape forbids (verified, the checks listed):

| shape | | where |
|---|---|---|
| not 0 | aggressive monsters do not pick the player as a new target (a monster already fighting goes on) | `CMob::FindTarget`, `FindTargetEx`, `FSM_IsExistUser` |
| not 0 | NPC guards do not pick the player | `CNpc::FindTarget` |
| not 0 | mounting (`0x0216`) | `CUser::PacketMaininterface` |
| not 0 | item attack and defence of an item put on or taken off are not counted (section 5.4) | `CUser::ItemEquipmentAdd` / `Rem` |
| not 0 and not 13 | **using skills** (`0x0511`, `0x0517` ignored, part 2 section 2) | `CUser::PacketPC` |
| 13 | other players cannot attack the player (`0x0502` ignored, the attack tick stops) nor aim a skill at them (`EnableAttackUser`: result 11 when choosing, a stop at the tick) **buffs and heals of their own side included** | `CUser::PacketPC`, `UpdateAttack`, `EnableAttackUser` |
| 13 | running (`0x0210` answered 0, section 10) | `CUser::PacketMaininterface` |
| 13, ≥ 100 | being copied by a disguise | `CUser::IsCloneTargetingUser` |
| 100 | item moves (`0x0204` ignored) | `CUser::PacketMaininterface` |

A shape does not stop normal attacks (`0x0503`); the attack tick ends hiding and most transformations (section 6).

## 3. Hiding

Confirmed, `CUser::AddApplySkillBuff` / `RemApplySkillBuff` case 88.

Stealth is a buff (part 3 section 4: `0x0511` result 0 within 60, `AddApplySkill`; not while a personal shop is open). Applying it removes any
other hiding or transformation first (part 3 section 3, the exclusive skills; section 6 here). **The effect**:

1. `m_byShape` = 13; **`SendShape`** (`0x051D` with 13 to the view, the player included);
2. **`0x0210`** with **0** to the player (`CUser::SendRunMode`: the client walks);
3. **`StatusAttackStop`** (section 10: attacking and casting stop).

**The end** (time, cancel `0x0511` #255, or the removals of section 6): `m_byShape` = 0, `SendShape` (`0x051D` with 0), **`0x0210` with 1**. The
player's run flag (`m_bRun`) is not touched; the client is told to run again whatever it was.

## 4. Finding the hidden

Confirmed, `CUser::ChkFindHide` 0x486e00.

Detect is a buff (`wType` 93). **When applied** and **at every three-second pass while it lasts** (section 7), `ChkFindHide`: every player in the
3 × 3 cells around the detector's cell who is alive, **not of the detector's side** (`CUser::IsTeam` false: another country, or a duel or guild
war opponent; on maps with a war type everyone is of one side), visible, and within **`byApplyRange`** of the detector (horizontal distance, ≤)
is **revealed** (`RemApplySkillHide`, section 6). Players of one's own side are never revealed. Its end does nothing.

## 5. Transformations

Confirmed, `CUser::AddApplySkillBuff` / `RemApplySkillBuff` case 90, `CUser::InitEquipment(bool)` 0x454ba0.

Choosing the target (part 2 section 3, `wType` 90 with target type 2):

- **`wChangeType` 102, Illusion**, through **`0x0517` only** (aimed at a monster; `0x0511` gives result 11): the monster must be in the zone,
  not dead, not a two-country (scripted) monster or an obelisk, and **of a level not above the player's** (`stMOBInfo.wLevel`); then its id is
  kept in `m_wTransFormTargetMobID` and the skill aims at the player. Otherwise result 11.
- **`wChangeType` 100, Disguise**, through **`0x0511` only** (aimed at a player; `0x0517` gives result 11): **`IsCloneTargetingUser`**: another
  player on the same map, **of a level not above the caster's**, visible, alive, not hidden and not disguised or an illusion (shape below 100
  and not 13), not the caster's duel or guild-war opponent, and **of the other country**. Then the target's look is copied (`m_pCloneUser`,
  section 2's fields) and the skill aims at the caster. Otherwise result 11.
- Any other `wChangeType` with target type 2: oneself. Evolution (target type 3) goes the buff way: a living player of one's side
  (`IsTeamUsable`, `EnableAttackUser`), **not riding a mount** (skill 127).

**The effect** (applied, as a buff, part 3 section 3):

1. `m_byShape = wChangeType`, `m_nShapeType = wChangeLevel` (for 102: the copied monster's id);
2. **`InitEquipment(false)`** (section 5.4);
3. **`SendShape`** (`0x0524` for 102, else `0x051D`);
4. for 100: **`SendUserShape`** (`0x0303` with the copied look to the view).

**The end**: `InitEquipment(true)`, `m_byShape` = 0, `m_nShapeType` = 0, `SendShape` (`0x051D` with 0), and for 100 `SendUserShape` (the real look
again).

### 5.4 Fighting numbers while transformed

`InitEquipment(false)` sets the four item numbers of the attack formulas (`docs/original/combat.md` section 4: item attack, its variation, item
defence, item magic defence) **from the monster `m_nShapeType`** when that id is a monster with a level and HP: its first attack's damage and
added damage (`pAttack[0].wDamage`, `wDamageAdd`), its `wDefense` and `wMagic`; **otherwise all four are 0**. Then `SetAttack`. So an evolved
player fights with the monster's numbers (2049: 91 + 70, defence 28 / 23; 413: 123 + 54, 30 / 27; 679: 211 + 105, 47 / 38), an illusion with
the copied monster's, and **a disguised player has no item attack or defence at all** while it lasts. `InitEquipment(true)` adds every equipped
item's numbers back (`ItemEquipmentAttackAdd`) and recomputes.

While the shape is not 0, putting an item on or taking it off skips that item's four numbers (`ItemEquipmentAdd` / `Rem`; stats, maxima and
speeds still change). With a transformation this keeps the monster's numbers; with **hiding** (which never calls `InitEquipment`) it lets item
numbers drift: the original's Stealth equipment bug (`docs/original/community-fixes.md`).

## 6. What ends hiding and transformations

Confirmed, `CUser::RemApplySkillHide` 0x489290, `CUser::RemApplySkillToMOB` 0x489560, `CUser::RemApplySkillByOtherSkill` 0x489740, `CUser::ClearApplySkillByDeath` 0x488000.

- **Their time**, and **cancelling** (`0x0511` #255): as any buff (part 3 sections 6, 7), with the end of sections 3 and 5.
- **Being revealed** (`RemApplySkillHide`, only while the shape is not 0): the first applied `wType` 88 skill is taken off: **`0x050E`** (its
  key) to the player, the party told, `m_byShape` = 0, `SendShape`, **`0x0210` with 1**. Callers: finding the hidden (section 4), a field's hit
  (section 8), Fire Thorns' hit (section 7).
- **Attacking** (`RemApplySkillToMOB`, called **at every attack tick in status 2** before the blow or skill (`CUser::UpdateAttack`), by
  `StatusCastingStop` (not by the cancel-by-step `StatusCastingStopByMove`), and after a damage-over-time hit on the player): only while the
  shape is not 0, the first applied skill of `wType` 88 or 90 in the list:
  - 88: as being revealed;
  - 90: taken off (`0x050E`, party told), `InitEquipment(true)`, shape 0, `m_nShapeType` 0, `SendShape`; for 100 `SendUserShape`; for 101
    `m_nTransFormFixMob` = 0 and `SendSpeed`; **except skill 127 at levels 1 to 3 with a `wChangeType` other than 101**: **Evolution survives
    fighting**.
- **Another hiding or transformation** (`RemApplySkillByOtherSkill`, part 3 section 3): the same packets as the end.
- **Death** (part 3 section 8): 90 is undone (`InitEquipment(true)`, shape 0, for 100 `SendUserShape`), then the shape is cleared for all;
  **no `0x051D`** and no `0x050E` (the client clears them on death; *inferred*). **Being hit by a monster** ends hiding and transformations too (**corrected**: every
  monster hit calls `StatusCastingStop`, `docs/original/mob-skills.md` section 9).

## 7. The three-second effects

Confirmed, `CZone::UpdateApplyEffectTick` 0x4221d0, `CUser::ChkKeepSkill` 0x487190, `CUser::ChkProtect` 0x486830, `CMob::ChkKeepSkill` 0x4a10d0.

**Correction to part 3.** The world thread (`CWorldThread::Update`) runs, per zone, the applied-skill ends every 1 s (`UpdateApplySkill`, part 3
section 6), **`UpdateApplyEffectTick` every 3 s**, and fields every 1 s (section 8). The three-second pass goes through the living players, then
the monsters that are not dead. The pass has its own clock: the first tick after a skill is applied comes 0 to 3 s later.

**Players** (`ChkKeepSkill(now)`): each applied skill in list order, stopping if the player died:

| `wType` | every three seconds |
|---|---|
| **81**, heal over time | each of HP, SP, MP: `trunc((now − m_dwPrevTick) / 3000 × wTimeHeal[i])` (0 when `wTimeHeal[i]` is 0, chop mode); **`m_dwPrevTick = now`**; `CUser::Heal` (not above the maxima, the values shrink to what fitted); **`0x050F`** (part 3's layout, with what was healed) to the players within 60. **At the end** (time or cancel) part 3's end heal pays the time since the last tick. |
| **84**, Fire Thorns | **`ChkProtect`**: `m_dwPrevTick = now`; in the 3 × 3 cells around the player, **players** of the other side (as section 4) within `byApplyRange`: revealed (section 6), `CUser::Damage(wDamage)`, death by the caster, **`0x0513`** (result 0, caster, target, skill, level, damage) within 60 (built: `docs/original/thorns-fields-pvp.md`); **monsters** alive, not in state 2, of another country than the player, within **`byApplyRange`** (horizontal, ≤): **`CMob::Damage(kind 1, caster, REC, wAggro)` with `wDamage` as it is** (no hit roll, no formula, `byDamageType` ignored); not killed: the monster targets the caster (`CMob::SetTarget(caster, true)`); killed: the caster stops attacking (`StatusAttackStop`) and `CMob::Death(1, caster)` (experience and drops as a kill); **`0x0519`** (result 0, caster, monster, skill, level, damage) within 60 of the caster; the monster targets the caster again. |
| **93**, Detect | `ChkFindHide` (section 4) |
| **61**, damage over time on a player | `CUser::TimeDamage` (monster skills, `docs/original/mob-skills.md` section 8) |

Afterwards: an expired `m_dwDisableBySkill` is cleared with `SendSpeed`; after a damage-over-time hit on a living player,
`RemApplySkillToMOB` and the player wakes (`RemApplySkillSleep`).

**Monsters** (`CMob::ChkKeepSkill(now)`), each applied skill in list order, stopping when the monster dies: **`wType` 61**:
`CMob::TimeDamage(now − m_dwPrevTick)` (part 3 section 12's formula), **`m_dwPrevTick = now`**, `CMob::Damage` by the debuff's attacker, `Death`
when it kills, **`0x0607`** within 60 (sent whatever the damage). Afterwards, when any 61 hit and the monster lives, **it wakes up**
(`RemApplySkillSleep`). **At the end** part 3's end damage pays the time since the last tick.

So part 3's "all at once at the end" is wrong for 61 and 81: they pay **every three seconds and the remainder at the end**; and part 3's "84
does nothing" is wrong: Fire Thorns is an aura of damage.

## 8. Fields

Confirmed, `CUser::SkillField` 0x4534e0, `CZone::EnterTrab` 0x4178d0, `CZone::UpdateTrab` 0x4226b0, `TrabDamage` 0x422480.

Kind 5, target type 1 ("a place": the target is the caster, part 2 section 3); only `wType` **101** (trap) and **102** (fire wall) do anything.
The effect (after the costs and cooldown of part 2 section 7):

1. **`0x0511`** result 0, the caster, **target 0**, the skill, level, values 0, to the players within 60;
2. a **field** (`CTrab`) with a new id, **at the caster's position**, the caster's id and country, the skill, **ending `now + dwKeepTime × 1000`**,
   is added to the zone (when its position is inside the map). Nothing else is sent: the client draws it from the `0x0511`.

**Every second** (`UpdateTrab`, after the monsters' idle pass) each field, in order:

1. **Players** in the 3 × 3 cells around the field's cell: alive, on a map whose war type is not 1, **of another country than the field**, visible,
   within **`byApplyRange`** of the field (horizontal, ≤): revealed (section 6), damage (below), `CUser::Damage`, death by the field's owner,
   **`0x0513`** (result 0, owner, player, skill, level, damage) within 60 of the field. Built later (`docs/original/thorns-fields-pvp.md`; at first the new server hit
   monsters only, section 12).
2. **Monsters** in the same cells: alive, not in state 2; **a trap (101) only hits monsters that its `byStateType` does not bounce off**: with
   `byStateType` 0 none at all, else those whose `pState[byStateType − 1]` is 0 (both trap rows have state 7); of **neutral or another country**
   than the field; within `byApplyRange`: damage (below), **`CMob::Damage(kind 1, owner, REC 0, wAggro)`**; not killed: when the owner is in the
   zone, the monster targets them (`CMob::SetTarget(owner, true)`); killed: `CMob::Death(1, owner)` (experience and drops to the owner as a
   kill); **`0x0519`** (result 0, owner, monster, skill, level, damage) within 60 of the field.
3. **A trap (101) is used up by its first victim** (player or monster): removed at once, nothing sent. A fire wall (102) hits everyone in range
   every second.
4. When its end has passed, the field is removed (nothing sent).

**Damage** (`TrabDamage`, by `byDamageType`): **0** `wDamage` as it is; **5** `trunc(wDamage[i] / 100 × current HP, SP, MP)`, at least 1 each
when `wDamage[i]` ≠ 0; 4 the same with the maxima (no field uses it); all at most 30000. Fire Wall: 32/50/80 HP within 3/4/5 for 12 s; Trap:
15/25/35 % of the current HP within 2, once. A field takes no hit roll and no defence.

## 9. One-off skills

Confirmed, `CUser::SkillOnce(CUser*, info)` 0x453610, `CUser::LifeSkill` 0x459d20, `CUser::SendUseSkill` 0x44e900.

Kind 6, through `AttackUser` (part 2 section 7) after the costs and cooldown. `SendUseSkill(result, caster, target)` is **`0x0511`** (20 bytes,
part 2's layout, values 0) **to the players within 60 of the caster**. `bySuccessType` is rolled as for skills (`CUser::AttackSuccess`; every
learnable row has type 1 with 100: always). By `wType`:

- **121, Resurrection** (target type 3). Choosing (part 2 section 3): a player of one's side (`IsTeamUsable`), **dead allowed**; a target in
  ultimate mode (mode 3) needs skill level 2 or 3; otherwise result 11. At the effect: **the target must be dead** (else `0x0511` result 11 to
  the caster only); a failed roll: result 3. Then `0x0511` result 0 (caster, target), and **`LifeSkill`**: only when the target is dead and
  `m_nStatusLife` is not 2: the body is removed (`CZone::LeaveDead`), **`0x0551`** (`TP_DEAD_REBIRTH`: target id, **kind 4**, the target's
  **experience unchanged**, **the caster's position**) to the target's view, **HP, SP, MP at their maxima** (`SendRecoverChange`, `0x0505`),
  motion 0, recovery state at least 1, **state 0**. No experience is lost. The target's server position is **not** changed (the client puts the
  character at the caster's position).
- **122** (`KillSoul` on an enemy's body unless in ultimate mode): no job can learn it.
- **123, Scout** (target type 3): choosing needs a player **not of one's side**; at the effect, a failed roll: result 3; **the same country:
  result 11** (to the players within 60); otherwise result 0 and **`0x0558`** (`TP_PC_SCOUTING_INFO`, 9 bytes: **`i32 element defence, u16 level,
  u8 mode`** of the target) **to the caster**.
- **124**: result 0 and nothing else (no row).
- **125, Return** (target type 2): when the caster is alive: `0x0511` result 0, **target 0**; **`CMap::GetBindPos(country, position)`** of the
  current map (`docs/original/mob-fight.md` section 8) and, unless a logout is pending, the move is scheduled as a gatekeeper's
  (`m_nMovePositionType` 1, now, map, position: `docs/original/gatekeeper.md` section 3). The wait is the skill's cast time (8 to 10 s).
- Other `wType` values (93 is a buff after loading): nothing, no packet, and the player stays as they were (the costs are spent).

Every case above ends with `StatusStand` on the **target** (a sitting target stands up silently; a pending logout is cancelled). Not reached by
anything covered here (casting needs the caster standing; targets of 121 are dead).

## 10. Stopping and running

Confirmed, `CUser::PacketMaininterface` cases 0x0212, 0x0210; `CUser::StatusAttackStop` 0x489ed0.

- **`0x0212`** from the client (2 bytes, no payload; the client sends it often): **`StatusAttackStop`**: unless the player is in state 0 or
  dead: **state 0, no target, `m_nAttackMode` 0, `m_nAttackNext` 0**, recovery state at least 1. **A cast in progress is dropped** (nothing spent:
  the costs belong to the effect). **No reply.** (The server's own `0x0212` is "attack start", part 2.)
- **`0x0210`** from the client: while hidden (shape 13) the answer is **`0x0210` with 0** and nothing changes; otherwise the run flag
  (`m_bRun`, 1 at login) flips and the answer is **`0x0210` with the new flag** (3 bytes: `u16 0x0210, u8 run`). The server reads nothing else
  from it and uses the flag for nothing else.

## 11. The packets

| op | direction | size | layout | when |
|---|---|---|---|---|
| `0x051D` | S → C | 7 | `u32 player, u8 shape` | a shape changes (view); after `0x0201` for a player with a shape |
| `0x0524` | S → C | 9 | `u32 player, u8 102, u16 monster id` | an illusion starts (view) |
| `0x0303` | S → C | var | the looks (`docs/original/world.md`) | a disguise starts or ends (view), and the looks reply |
| `0x0210` | C → S / S → C | 3 | `u8 run` | run toggle; hiding (0) and its end (1) |
| `0x0212` | C → S | 2 | — | stop attacking |
| `0x0558` | S → C | 9 | `i32 element defence, u16 level, u8 mode` | scouting (caster) |
| `0x0551` | S → C | 23 | `u32 player, u8 4, u32 exp, f32 x, y, z` | resurrected (target's view) |
| `0x0511` | S → C | 20 | part 2 | fields and one-off skills (within 60) |
| `0x0513` | S → C | 20 | as `0x0511` | a field or Fire Thorns hits a player |
| `0x0519` | S → C | 20 | as `0x0517` | a field or Fire Thorns hits a monster |
| `0x050F` | S → C | 15 | part 3 | each heal-over-time tick (within 60) |
| `0x0607` | S → C | 15 | part 3 | each damage-over-time tick on a monster (within 60) |

## 12. The new server

- **Shape** joins the player's zone record next to `PlayerBuffs`, computed from the applied list (the hiding or transformation in it), with
  the copied monster id or copied look kept with the applied skill. `0x051D` / `0x0524` / `0x0303` as section 2, also after `0x0201` for
  players entering the view and on entering a zone (including after a relog with a saved hiding or transformation). The looks reply uses the
  copied look while disguised. Monster target searches skip players with a shape; skill targeting and buffs on others refuse hidden
  targets (and so do `0x0502` and the skills on players, built: `docs/original/pvp.md` section 9, `docs/original/pvp-2.md` section 11);
  skills are refused while transformed (not hidden); item moves while disguised.
  The copied look and monster are taken when the skill is chosen, as the original's target checks do.
- **Fighting numbers** stay computed from scratch: while transformed, the four item numbers come from the monster (or are 0) whatever is
  equipped; **while hidden, equipment counts normally** (the Stealth equipment bug cannot happen).
- **Ending**: the attack tick (normal and skill, oneself as the target included) ends hiding and transformations except Evolution, before the
  blow or skill; revealing by Detect (fields and Fire Thorns would, if they hit players). A cast cancelled by a step does not
  (`StatusCastingStopByMove` does not call `RemApplySkillToMOB`; `StatusCastingStop`, which does, serves the monster and player hits, built
  since: `docs/original/mob-skills.md` section 9, `zone::mob_skills::struck`).
- **The three-second pass** joins the zone's timers: heal over time, Fire Thorns, Detect for players, damage over time for monsters, with the
  end payments of part 3 counting from the last tick. A monster killed by Fire Thorns does not stop the caster's own attack at once: the
  next attack tick finds it gone and stops.
- **Fields** live in the zone (a list per map), checked every second with the monsters' idle pass.
- **One-off skills**: Resurrection, Scout, Return as section 9; Return reuses the gatekeeper's scheduled move.
- **`0x0212`** stops attacking and casting; **`0x0210`** answers as section 10.
- **Deviations**:
  - **fields and Fire Thorns hit monsters only** (built: `docs/original/thorns-fields-pvp.md`, players are hit and hidden ones
    revealed); Detect does reveal enemy players (no damage involved);
  - **Resurrection moves the target's server position to the caster's position** (where the client puts them), so that the next steps pass the
    move check; the original leaves the server position at the body;
  - **players coming into view with an illusion get `0x0524`** (with the monster id) rather than the original's bare `0x051D` 102;
  - **Illusion is not saved** (the copied monster is not kept: after a relog the original would restore shape 102 with no monster and zero item
    numbers); Disguise is not saved, as in the original (`wChangeType` 100);
  - the map war type (`Map.ini` `WarType`, maps 40 and 42) was not imported: no map counted as one side (imported since,
    `docs/original/pvp.md` section 9: `World::is_war_map`);
  - Polymorph (97) is refused as the original refuses it on monsters (result 11); on players it was refused as player kills, and since
    those are built it is a debuff on an enemy player like any other (`docs/original/pvp-2.md` section 11); 122 and 124 (no job, no
    row) and 89 (no row) are refused with result 10;
  - staff invisibility and party skills were not built here; both are built (`docs/original/gm-commands.md` section 13,
    `docs/original/party.md` section 13; 179 has no learnable row); mounts are in `docs/original/vehicles.md` (Evolution refuses a mounted
    target);
  - `StatusStand` on a one-off skill's target (section 9) is not built (nothing reaches it).
