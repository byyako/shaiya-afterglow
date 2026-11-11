# Scripted monsters: the AI state machines (`AI` 10 to 99) and the Lua bosses (`AI` 100 and up)

This page covers monsters that run a script in the EP4.5 game server `ps_game.exe`: the AI state machines of
`Data/mob_ai.ini` and `Data/AI/*`, how their conditions are evaluated, the functions and handlers they use, where they meet the ordinary
monster code, speech, summons, and a catalogue of the Lua bosses. It was read with the decompiler and the disassembler with the PDB names,
and from the AI files the server reads (`Data/mob_ai.ini`, `Data/AI/*`, `Data/kr_string.DB`, `Data/Lua/*.lua`); nothing was executed. The
client's side (what `0x1109` and the `06xx` packets show) is on the client's [Scripted monsters](../client/scripted-monsters.md) page.
Walking about, fighting and monster skills are on [Monsters](mobs.md), [Monsters fighting back](mob-fight.md) and
[Monster skills](mob-skills.md); this page says where scripted monsters leave or rejoin those paths.

Not covered here: the Lua side in depth (see [Party instance zones](instances.md)), and the `MoveRunAway` and `Debug` handlers (no
AI uses them).

## In short

- **Scripted monsters live on maps 45 to 47**: the scorpions, crocodiles, Helltooths and sphinx followers there (65 kinds), and the
  Cryptic One bosses of maps 48 and 49 (boss groups). Each runs a small state machine written in an editor and saved as text files the
  server loads at start.
- **What the scripts do**: wander or run, fight using the ordinary attack code, shout lines in chat, call other monsters to attack,
  summon helpers, change their own strength, defence, attack speed and recovery, and switch their element.
- **They fight once a second**, not twice like ordinary monsters, and they never come to help others.
- **Their speech is Korean** in the US client, so it shows as unreadable text.
- **Several script features do nothing in the original**: the "attack only with" settings are never read, a "hit and run" move never
  moves, and one end-of-battle test never fires.
- **Lua bosses** (`AI` 100 and up) are the bosses of the Cailum Sacra party instance, covered on [Party instance zones](instances.md).

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CMob` offsets match the PDB; `CUser` offsets are the code's.

## 1. Which monsters

Confirmed, `CMob::Init` 0x490040, `CAIFactory::SetFSM` 0x4a9130.

At creation a monster whose definition has **`byAI` (column `AI`) from 10 to 99** gets the state machine of that number from
`mob_ai.ini` (`CAIFactory::SetFSM(mob, byAI)`); when that works its state (`m_nStatus`) becomes **100** and `m_bUsePAIS` is set. A number
beyond the file's last line leaves an ordinary monster. **`byAI` 100 and up** gets a Lua script
instead (`CMob::InitLuaFSM`, section 11).

In the data (`defs.mobs`, spawns from `world.mob_areas`):

| `AI` | AI file | definitions | where |
|---|---|---|---|
| 10 | `sorp1` | 12 scorpion workers and guards, ruin mages (1505 …) | maps 45, 46 |
| 11 | `croco1` | 3 red and green crocodiles (1600, 1605, 1606) | map 47 |
| 12 | `croco2` | 3 (1601, 1602, 1607) | 47 |
| 13 | `Troll1` | 8 Helltooth fighters (1617 to 1624) | 47 |
| 14 | `Spink1` | 6 sphinx followers (1550, 1551, 1610 to 1613) | 46, 47 |
| 15 | `GoldenPig` | the golden pig (1968) and 1940 | not spawned (an event monster) |
| 16 | `Sorp3` | 5 scorpion guards (1506 …) | 45, 46 |
| 17 | `SorpNamed1` | 6 named scorpion priests and rulers (1518 …) | 45, 46 (1521 not spawned) |
| 18 | `SorpNamed2` | 5 (1519 …) | 45, 46 |
| 19 | `Sorp2` | 5 female scorpions and guards (1507 …) | 45, 46 |
| 20 to 22 | `CrocoNamed1` to `3` | 2 named crocodiles each (1629 to 1634) | 47 |
| 26 | `TrollNamed1` | 3 named Helltooths (1635 to 1637) | 47 |
| 27 | `HellTouthNamed1` | 4 (1638 to 1641) | 47 |
| 28 to 34 | `Deinos`, `Parridalis`, `Alcarian`, `ZinAlcaria`, `Belizabeth`, `Kirhiross`, `CrypticOne` | 2 or 4 bosses each (1700 to 1714, 1800 to 1814) | the boss groups 007 to 020 of `Obelisk.ini`, maps 48 and 49 (`docs/original/obelisks.md` section 11; **corrected**: earlier "not spawned by any file here", read from the spawn areas only) |
| 23 to 25, 35, 36 | `OrcNamed1` to `3`, `Haruhion`, `Freezing` | none (the US client's `Monster.SData` gives 35 and 36 to four bosses that are AI 1 on the server: `docs/original/mob-ai.md` section 5) | |
| 104, 111 to 118 | Lua `104.lua` (missing), `111.lua` to `118.lua` | one each (1890, 2464 to 2472) | 2471 on maps 65 and 66 (party instances), the rest summoned by scripts |

So the scripted monsters a player meets are those of maps 45 to 47 (65 definitions with `AI` 10 to 27) and the bosses of the boss
groups on maps 48 and 49 (`AI` 28 to 34). The numbers 0 to 9 are built into the server: [Monster AI 0 to 9](mob-ai.md).

## 2. The files (verified, `CAIFactory::LoadMobAI` 0x4a8df0, `InitFSM` 0x4a98f0, `LoadLocalVariableText` 0x4a99f0,
`LoadSystemFunctionText` 0x4a9b30, `LoadStateMachineText` 0x4a9d90, `LoadCPPFunctionsList` 0x4aa920)

**`data/mob_ai.ini`** (read at start by `InitDB`, text): one line per AI, `"%d %s"`: the number and a path without extension, e.g.
`10 data/ai/sorp1`, `13 data/ai/Troll1` (trailing blanks ignored). Every line becomes the next entry of a list and **`SetFSM` takes
entry `byAI` by its position**, not by the number written (the file lists 0 to 36 in order, so they agree). Lines with a number below 10
(`ai/empty`) are listed without loading anything; for the others the files are loaded, and **one that fails stops the server's start**
(`InitDB` fails). The files are on a Windows disk: the names in `mob_ai.ini` differ in case from the files (`Troll1` / `troll1.ai`); every
one of the 27 sets exists when compared without case.

For each line `InitFSM` reads four files, `<path>.alo`, `.afs`, `.ai`, `.ah`, each with `fscanf` patterns `"Key : %d\n"`, `"%s"`,
`"%u"`, `"%f"` in a fixed order (the editor's other output, `.ac`, `.PAI` and `_AI_FUNCTION.H`, is not read; `.PAI` holds the
editor's source with readable expressions, e.g. `ConditionExp : "GetEnergyPercent() <= 0.5"`):

- **`.alo`, the local variables**: `LocalVariableCount : n`, then `n` times `Type : t` (1 int, 3 dword; 2 float and 4 string exist as
  types, unused) and `Name : name`. A variable's key is the hash of its name (below).
- **`.afs`, the system functions** the AI calls: `SystemFunctionCount : n`, then per function `Name`, `FuncID` (the hash of the name),
  `ReturnType`, `ParamCount` and per parameter `ParamType` and `ParamPipe` (1 = written back into the variable passed, section 4;
  every `ParamPipe` in the data is 0).
- **`.ai`, the machine**: `BehaviorName`, `StateCount`, then per state: `StateName`, `StateInit`, `StateUpdate`, `StateExit` (handler
  names, the same as in `.ah`), `StartConditionCount` and the conditions, `UpdateConditionCount` …, `ExitConditionCount` …,
  `TransitionCount` and per transition `TransitionName` (a number), `TransitionStart` (this state), `TransitionEnd` (the target),
  `ConditionCount` and the conditions. At the end `StartState : name`.
  A **condition**: `ConditionType` (**0 = an action, 1 = a test**), `TokenCount`, then per token `Type`, `Name`, for functions
  `FuncID`, for operators `OpCode`, then `ParamCount` (unused, always `-842150451`), `NumberType` (1 int, 2 float, 3 dword) and `Value`
  (read as `%d`, `%f` or `%u` by `NumberType`; other number types read no value).
- **`.ah`, the state handlers**: one line per state, `name,enter,update,exit`, e.g. `battle,Battle,Battle,Empty`. A handler name is
  looked up among the seven C++ handlers `CMob::ConnectToFSM` (0x491db0) registers: `MoveRandom`, `MoveRandomRun`,
  `MoveAtSelectedPosition`, `MoveRunAway`, `Battle`, `Debug`, `ResetUser` (section 6); anything else (`Empty`) is no handler. In the
  data the `.ah` rows and the `.ai` names agree for every state.

**Names are hashed** (djb2: `h = 5381`, `h = h × 33 + byte` over the name's bytes, as signed `char`, 32-bit): states, variables and
functions are found by `h(name)`, case-sensitive. E.g. `h("Say") = 193470706` = 0x0B8820F2, the `FuncID` in the files.

**`data/kr_string.DB`** (`CAIStringDB::LoadStringDB` 0x402820, read at start): pairs of lines, `"%d\n"` then the text by `fgets`
(1024 bytes, **the line end kept**, the file opened in text mode so `"\r\n"` reads as `"\n"`), kept as `{id, text[128]}`. 31 lines,
ids 1 to 31, **Korean** (code page 949) apart from id 2 "Li laka Amon-Ra". Used by `Say` and `Say2` (section 5).

## 3. The machine at work (verified, `CKFSM<CMob>::EvaluateCurrentStateTransitions` 0x4a83e0, `KFSMState<CMob>::OnEnter` 0x495da0,
`OnUpdate` 0x4a8560, `OnExit` 0x495e00, `CMob::UpdateIdle` 0x4a7e60, `CMob::ResetFSM` 0x492310)

**Creation**: the local variables are created with value 0, the current state is the start state, and **its enter runs** (below).

**Every idle pass** (each second, `CZone::UpdateMobIdle`, `docs/original/mobs.md` section 6), for a monster with `m_bUsePAIS`:
`m_dwCurrentTime = now` (what `GetCurrentTime` returns), then by state:

- **100**: the **transitions** of the current state, then the current state's **update** (the update conditions in order, then the
  update handler). The ordinary idle turn (walking, looking for targets) does not run.
- **3** (going home): `CMob::ReturnHome(now)` (fight section 5). The attack pass (500 ms) also runs `ReturnHome` for state 3
  (`CMob::UpdateAttack` skips only state 100), so going home runs from both passes.
- **2** (dead): nothing.
- **0**, should the monster get there (nothing in the scripted paths sets it, section 7): the ordinary idle turn.

**The transitions**, in file order: for each, **every condition is evaluated** (actions run, tests count, section 4) and the
transition **fires when the number of true tests equals its number of tests** and its target is not the current state (a transition
to the state itself never fires; one without conditions fires at once). The first one that fires changes the state and the rest are
not looked at: **at most one change per pass**. A transition's actions run each time it is evaluated, also when its tests fail.

**A change of state**: the old state's **exit** (its exit conditions, then its exit handler), the current state becomes the target,
then the new state's **enter** (its start conditions in order, then its enter handler). The new state's update follows in the same
pass. Handlers run with the monster; conditions are evaluated as in section 4.

**`CMob::ResetFSM`** ("start over"): all attack kinds enabled again (`m_bAttackEnableType`, section 7), not in battle, not attacked
(`m_bIsBattle`, `m_bAttacked` 0), **no target**, state **100**, every local variable 0 (`CAIFactory::ClearVariable` 0x4a9840: int, float,
dword or string by its type), current state = the start state, its enter. The strength, defence, attack speed and recovery numbers set by
the script (section 5) are **not** reset. Called wherever an ordinary monster would become idle (state 0), section 7.

## 4. Conditions: tokens and their evaluation (verified, `CKFSM<CMob>::TravelCondition` 0x498cf0, `CKFSM<CMob>::CalculateParam` 0x49d020,
`KFSMTransition::AddCondition` 0x4a8cb0, the enums `KTOKEN_TYPE`, `AI_PARAM_TYPE`, `OPCODE` of the PDB)

A condition is a token list in **postfix order**, evaluated on a stack of values. A value (`AI_Param`) has a type (0 void, 1 int, 2 float,
3 dword = unsigned 32-bit, 4 string) and an int, a float, a dword and a string field (only the one of its type is meaningful; the others
are 0), and the variable it came from.

| token `Type` | what | evaluation |
|---|---|---|
| 2 number | `Value` with `NumberType` 1, 2 or 3 | pushes an int, float or dword |
| 3 variable | `Name` | pushes the local variable's value, remembering the variable |
| 4 function | `Name`, `FuncID` | **the arguments follow the function token**: for each of the `ParamCount` parameters of its `.afs` entry, the tokens up to the next `Param` token are evaluated as a sub-expression and its top value is the argument (**its own type; no conversion to `ParamType`**). Then the function (section 5); its result is pushed |
| 5 `Param` | ends one argument | |
| 7 string | | pushes a string (only in unused files) |
| 8 operator | `OpCode` | pops the right operand, then the left, pushes the result |
| 9 comparison | `OpCode` | pops right and left; **a true comparison adds 1 to the condition's count** |

Examples from the data (as `.PAI` writes them, and the tokens): `nCount == 0` → `nCount 0 ==`; `dwTime = GetCurrentTime()` →
`dwTime GetCurrentTime =`; `GetCurrentTime() - dwTime > 1000` → `GetCurrentTime dwTime - 1000 >`; `AttackUser(UserID, 10.0)` →
`AttackUser UserID Param 10.0 Param`; `AddEnergy(GetAttackCount() * 50000)` → `AddEnergy GetAttackCount 50000 * Param`.

**Operators** (the ones in the data: `=` 10, `-` 7, `*` 8; the others, `+` 6, `/` 9, `+=` 11, `-=` 12, `++` 13, `--` 14, `*=` 15,
`/=` 16, follow the same rules): **the result has the left operand's type**, the right one converted to it:

| left | right int | right float | right dword |
|---|---|---|---|
| int | int arithmetic (wrapping) | `trunc(float(left) op right)` | 32-bit arithmetic |
| float | `left op float(right)` | float | `left op float(unsigned right)` |
| dword | unsigned 32-bit arithmetic | `trunc(float(unsigned left) op right)` | unsigned 32-bit |

**`=`** gives the left operand's variable the right value converted to the variable's type (int from float: `trunc`; float from dword:
unsigned). `/` by an int 0 is a division by zero in the original (no script divides).

**Comparisons** (`>` 0, `<` 1, `>=` 2, `<=` 3, `==` 4, `<>` 5): int with int as signed numbers, dword with int as unsigned 32-bit numbers,
**any comparison with a float on either side in float**, the int or dword converted. **A right operand that is a dword never compares
true** (the original has no case for it; no test in the data has one), nor does a void or string left operand; a void or string right
operand reads as an int, its int field (0 for a void result). Operators read a void or string right operand the same way, and give
void for a void or string left one.

**A condition's result** is its count of true comparisons. An action (type 0) is evaluated the same way and normally has none.
`KFSMTransition::AddCondition` counts the tests (type 1) of a transition; the transition fires when the sum of its conditions'
results equals that count (section 3). Every test in the data has exactly one comparison.

**Function arguments read one field** (section 5 says which): an argument of another type reads 0 there. In the used files every
argument has the type its function reads.

## 5. The functions

Confirmed, `CMob::DoFunction` 0x4917a0, which dispatches on the `FuncID`, and the functions it calls.

"Target" is `m_Target` (a player, by id). Results not listed are void. Field read: `n` int, `f` float, `d` dword.

| name | arguments (field) | result, effect |
|---|---|---|
| `GetCurrentTime` | | dword: `m_dwCurrentTime`, this idle pass's tick count |
| `GetRandInt` | | int `rand() % 100` |
| `GetEnergyPercent` | | float HP / max HP (0 to 1) |
| `IsAttackMe` | | int `m_bAttacked` (section 7) |
| `IsBattle` | | int `m_bIsBattle` |
| `IsAttackMeAtMoment` | | int: 1 and clears `m_bAttackAtMoment` when set, else 0 |
| `IsFirstAttack` | | int: **`CMob::FindTarget`** (fight section 2: the first player in the 3 × 3 cells, alive, visible, seeing monsters, within `byChaseRange`, who **becomes the target**) found one: 1, else 0 |
| `IsEndBattle` | | prints a debug line; the result's type is never set (stack garbage): **compares false** (section 10) |
| `GetCurrentUser` | | dword: the target's id, 0 without one |
| `SelectUser` | id (`d`) | when not 0 and that player is on the map: `SetTarget(player, 1)` (section 7) |
| `IsExistUserAttackPoint` | | dword `CMob::GetMaxAttackUserID` (0x490ba0): before the next aggro time (`m_dwNextTargetingTime`) 0; with a fixed vision target its id (next time `now + 5000`); else the damage record's top-aggro entry when it is a player other than the target (the time not moved); else 0 and next time `now + 5000` |
| `GetCurrentMaxHpUser` | | dword: of the damage record's players who are on the map, of another country, alive, see monsters, visible and attackable, the one with the **highest current HP** (the first of equals; `CMob::FindAttackUserMaxHP` 0x4a06f0); 0 for none |
| `GetDistanceFromExit` | | float: horizontal distance from the position saved by `SetRunawayPosition` (0, 0, 0 until then) |
| `ArrivalChek` | | int: 1 when the monster is **less than 2** from the selected position (3-D, `SVector::Distance`), else 0 |
| `GetAttackCount` | | int: the hits counted since `BeginCheckAttackCount` (a negative count reads 0) |
| `Say` | id (`n`) | `PAI_Say`: line `id` of `kr_string.DB` to the target, or to the target's whole party (section 8) |
| `Say2` | id (`n`), radius (`n`) | `PAI_Say2`: line `id` to every player within the radius (section 8) |
| `UseMobSkill` | skill id (`n`) | `PAI_AttackSkill` (0x4afc80): with a target on the map, `CMob::UseSkill(now, the level-100 row of that skill id, target)` (mob skills section 3: no range check, no chase; the next attack time `now + speed + 2500`). An id outside 1 to the row count, or without a row, passes no skill: **a null-pointer crash in the original** (no id in the data; all 30 have rows) |
| `AttackUser` | id (`d`), radius (`f`) | `PAI_AttackCommand` (0x4afa20): every **other monster** within the radius (3-D, the cells within `radius / 40` rounded) gets `SetTarget(player, 1)` and its attack numbers recomputed (`SetAttack`); any AI, country or state (state-0 monsters fight, scripted ones take the target) |
| `SuchFrendlyandRun` | radius (`f`) | `PAI_FindNonBattleMob` (0x4b07f0): the selected position becomes the position of the first other monster within the radius (3-D) that is **not fighting** (an ordinary one in state 0, a scripted one not in battle); none: the monster's own position |
| `SetDrMove` | distance (`f`) | the selected position = `(0, 0, g_ConstMobMoveSpeed[move speed] + byNormalStep + distance)` turned about y by `rand() % 360` degrees: **an offset that is never added to the monster's position** (section 10) |
| `SetRunawayPosition` | | saves the current position (for `GetDistanceFromExit`) |
| `SetStr` | factor (`f`) | `m_fStr`: multiplies `wDamage` of its normal attacks (fight section 6 step 4) |
| `SetRec` | factor (`f`) | `m_fPhysicalDefense`, then `SetAttack`: the melee defence becomes `trunc(Defense × factor + add)` (combat section 4) |
| `SetAttackSpeed` | factor (`f`) | `m_fAttackSpeed`: the time between normal attacks is divided by it (fight section 6 step 7) |
| `SetHpChage` | factor (`f`) | `m_fRecoverRatio` (section 7, recovery) |
| `SetRecoveryTime` | ms (`n`) | `m_dwRecoverTime` (5000 at creation) |
| `SetEnableRecovery` | flag (`n`) | `m_bRecoverEnable = (flag == 1)` |
| `SerChangeRegist` | element (`n`) | `m_nAttrDef`: the monster's **defence element** (players' attacks use it for the element factor, combat section 3) |
| `AddEnergy` | amount (`n`) | `PAI_AddEnergy` (0x4b0b00): HP += amount, capped at max HP; when what was really added is above 0, **`0x0610`** (`u32 id, i32 amount`) to the players within 60 |
| `SetSummonMob` | id (`n`), radius (`f`), count (`n`), attack kinds (`n`) | `PAI_CreateMob` → `CZone::MobGen` (section 9) |
| `SetBattleType` | type (`n`) | `m_nBattleType` (section 7: misses by skills) |
| `SetEnableOnlyAttackType` | kind (`n`) | `m_bAttackEnableType`: 4 all, 5 none, 0 to 3 only that one; **nothing reads it** (section 7) |
| `SetEnableATAll` | | all four set |
| `BeginCheckAttackCount`, `EndCheckAttackCount`, `ResetAttackCount` | | counting on, off; the count 0 |

The `.afs` files also declare `GetBloodType`, `Addint`, `Func` and `TestArg`, which no `.ai` calls; `DoFunction` has no case for any name
outside this table (the result is void).

## 6. The state handlers (verified, `CMob::onBattle` 0x4b0520, `onMoveRandom` 0x4afcf0, `onMoveRandomRun` 0x4aff90,
`onMoveAtSelectedPosition` 0x4b01c0, `onResetUser` 0x4b05b0)

- **`Battle`** (enter and update of every AI's fighting state): nothing before `m_dwNextAttackTime` or while asleep or stunned. Then the
  target: when it is **not on the map or dead**, the vision fix is removed and `FindTarget` (section 5) looks for another; none found:
  **`ResetFSM`**. Otherwise **`PAI_Attack(now)`**: when the target is of the monster's country (`nCountry`), dead, does not see monsters,
  or is not visible or not attackable: `FindTarget`, none: `ResetFSM`; else **`CMob::AttackAI(now, player)`**, the ordinary fight
  (fight section 3: the entries 2 down to 0, chase, normal attack, entry skills). So **scripted monsters fight once a second** (the idle
  pass), not every 500 ms. (`onBattle`'s attack-speed test can never be true, as in `UseSkill`.)
- **`MoveRandom`**: one step as in the idle walk (mobs section 6 step 5, `L = g_ConstMobMoveSpeed[move speed] + byNormalStep + r`, angle
  `rand() % 10000` degrees), **every pass** (no move timer). When **not in battle** and away from home (a chase path and more than 0.5,
  or more than **10**): state 3 (going home) and no step. The step is taken when its target is **less than 10 from home** and the line
  is free: `MoveMob(target, motion 0)`.
- **`MoveRandomRun`**: as `MoveRandom` with `L = g_ConstMobMoveSpeed[move speed] + byNormalStep` (no `r`), the limits **20** instead of
  10, and **motion 1** (running).
- **`MoveAtSelectedPosition`**: when not in battle and away from home as for `MoveRandomRun` (20): state 3. The selected position's y
  becomes 0; when it is **20 or more from home** (horizontal), it becomes the monster's own position and nothing moves. Else, when the
  line to it is free (`CMap::EnableLine`, truncated coordinates), **`MoveMob(selected position, motion 1)` in one move** (so the monster's
  y is 0 afterwards, and `ArrivalChek`, a 3-D distance, is true); blocked: the selected position becomes the monster's own.
- **`ResetUser`**: unless a vision target is fixed: not attacked, not in battle, no target.
- `MoveRunAway`, `Debug`: registered, used by no AI.

## 7. Where scripted monsters meet the ordinary code

Confirmed.

**Becoming a target** (`CMob::SetTarget` 0x49fad0): a monster in **state 100** takes the player as target (unless a vision target is fixed)
and stays in state 100; **no call for help**; each attack entry with a skill gets its ready time `now + rand() % (wResetTime × 1000)`, or 0
without a reuse time (**entry 0 included**, unlike the ordinary path). Scripted monsters are never helpers: the helpers' searches
(`FindHelp*`) take only state-0 monsters.

**Being attacked** (the flags `IsAttackMe`, `IsBattle`, `IsAttackMeAtMoment` read; `CUser::AttackMob` 0x44ce10 and 0x44fd90, with
`CMob::SetHitSuccess` / `SetHitMiss` inlined): after `SetTarget(player, 1)`:

| player's attack | `m_bAttacked`, `m_bIsBattle`, `m_bAttackAtMoment` set |
|---|---|
| a normal blow that hits and does not kill | always |
| a normal blow that misses | no (no `SetTarget` either) |
| a skill blow that hits (one target or area) | always |
| a skill blow that misses (`0x0517` / `0x0518` / `0x0519` result 3) | only while **`m_nBattleType` is 0** (`SetBattleType(1)` turns it off) |

`m_bIsBattle` is also set at the start of every `AttackAI` (the monster is fighting). They are cleared only by `ResetFSM`, `ResetUser`,
and `IsAttackMeAtMoment` for its own flag.

**Hits counted** (`CMob::Damage` 0x490cf0 and 0x490ee0): while counting is on, every damage to a monster with HP above 0 adds 1, before
the damage is applied.

**Back to the script** (`ResetFSM`) wherever an ordinary monster becomes idle (state 0) in the fight code:

- the normal attack killed its target (`CMob::Attack` 0x4a2000): `FindTarget` first, `ResetFSM` only when it finds no one;
- `CMob::StatusAttackStop` (0x490980, e.g. no A* path): the same;
- the chase gives up, the target more than 45 from the monster (`CMob::Chase` 0x4a6de0);
- back home (`CMob::ReturnHome` 0x4a7670: less than 10 from home, the home moved to where it stands, and the blocked cases): `ResetFSM`
  where an ordinary monster gets state 0, after which `ResetAttack` and the record clearing follow as for any monster;
- `CMob::StatusChange(100)` (0x490930).

**Out of the script into the ordinary fight** (verified, `CMob::ReTargetByAggro` 0x490ab0 writes state 1 at 0x490b5c with no script
check): `AttackAI` runs `ReTargetByAggro` before each entry it uses (fight section 3), so when the damage record's top-aggro player is
another one, on the map, of another country, targetable and within 45, a scripted monster **takes it as target in state 1**: from then on
it is an ordinary fighting monster (the attack pass every 500 ms, no transitions, no handlers; its multipliers stay) until one of the
hand-backs below brings it to state 100. A taunt (`wType` 72, `docs/original/skills-buffs.md`) does the same. In state 1 the ordinary
paths that set state 0 without a script check also apply (the attack pass dropping a dead or vanished target, `AttackAI`'s own drop), and
in state 0 it is an ordinary idle monster (walking about, going home when 10 from home) until a hand-back.

The chase's "target more than 45 from home" and the blocked-chase cases set state 3 (going home) as for any monster. `AttackAI`'s own drop
(target of the monster's country, dead, …) sets state 0 without a script check, but `PAI_Attack` tests the same things first, so it is
not reached from `Battle`.

**Recovery** (`CMob::UpdateRecover` 0x4a8210, each second; fight section 9 for the ordinary rule, state 0 only): a scripted monster
recovers **only in state 100**: in battle (`m_bIsBattle`) only when `m_bRecoverEnable` is set, by **`trunc(recovery × m_fRecoverRatio)`**,
next time `now + m_dwRecoverTime`; not in battle by `recovery`, next time `now + 5000` (as an idle ordinary monster). Capped at max HP,
`0x0610` as usual.

**Not used**: `m_bAttackEnableType` (`SetEnableOnlyAttackType`, `SetEnableATAll`, the summons' attack kinds) is written but never read by
any code (checked over the whole disassembly), so these change nothing.

**Death and respawn**: as for any monster (`CMob::Death`); a respawned monster is a new one with a fresh machine and the multipliers at
1.0.

## 8. Speech

Confirmed, `CMob::PAI_Say` 0x4b06c0, `PAI_Say2` 0x4b0b80.

Both look the id up in `kr_string.DB`; an unknown id (32 and 33 are used by `Say2` but missing), an empty text or one of 128 bytes or
more says nothing. The packet is **`0x1109`** (`TP_CHAT_MONSTER`): `u16 op, u8 kind, u32 monster id, u8 len, text` with **`len =
strlen(text)`** and **8 + len bytes sent** (no NUL; the text ends with the file's `"\n"`).

- `Say`: **kind 0**, to the target only (on the map), or, when the target is in a party, to **every member of the party** (`CParty::Send`,
  wherever they are).
- `Say2`: **kind 1**, to every player on the map within the radius (horizontal, `≤`, the cells within `radius / 40` rounded). All uses
  pass 200.

The client prints kind 0 and 1 as "<monster name> : <text>" in chat (`docs/client/scripted-monsters.md`, "Speech and voice"). Which
monster says each line, and when, is on [Monster speech](mob-speech.md); the US client's own English of these lines (sysmsg 6001 to
6031) is never used by the original.

## 9. Summons

Confirmed, `CMob::PAI_CreateMob` 0x4b0660, `CZone::MobGen` 0x41dec0.

`SetSummonMob(id, radius, count, kinds)`: nothing unless the definition exists with an id, HP and a level. Then **`count` tries**, each at
`x + rand() % trunc(2 × radius) − radius`, `z` likewise (from the summoner's position, `y` the summoner's): a try whose truncated spot is off
the map or blocked is lost; otherwise a new monster of that id is created (a new id, `CMob::Init` with **no spawn entry**: it never
respawns), its attack kinds set from `kinds` (4 all, 5 none, else that one; unread, section 7), and it enters the zone (`CZone::EnterMob`:
`0x0601` to the players around). Used by `Alcarian` (one 1703 within 1) and `GoldenPig` (one golden box, 1967, within 10, at most every
7 s of being hit).

## 10. Original bugs and oddities

- **`SetDrMove` never moves the monster**: the selected position is a short offset from the map's corner (0, 0), so the following
  `MoveAtSelectedPosition` finds it 20 or more from home, makes it the monster's own position, and `ArrivalChek` is true at once.
  `SorpNamed1`'s "hit and run" (`AI` 17) therefore casts its skills without running.
- **`IsEndBattle`** returns an unset value; its one test (`CrocoNamed1`, `IsEndBattle() == 0`) never fires in practice.
- **`UseMobSkill` with an unknown skill id** dereferences a null skill (crash). No AI in the data does it.
- **The speech is Korean** in a US client, and every line carries a trailing `"\n"`.
- Two unused AIs (`scorlp1`, `testshaiya`, not in `mob_ai.ini`) pass a string to `Say` and an int variable to `SelectUser`, which read 0.

## 11. Lua monsters (`AI` 100 and up): catalogue only (`CMob::InitLuaFSM`, `CMob::LuaRegister` 0x4b1310, `CMob::Lua*`)

`byAI` ≥ 100 loads `Data/Lua/<byAI>.lua` into its own Lua state (`m_pLua`, `m_bLuaAI`) and uses the stack state machine (`StackStateMachine`,
`m_pFSM` at +0x44) instead of `m_nStatus`. The scripts define `Init`, `OnAttacked(time, char id)`, `OnAttackable(time, char id)`,
`OnReturnHome(time, attacked count)`, `OnNormalReset(time)`, `OnDeath(time, attacked count)`, `OnMoveEnd(time)` and
`WhileCombat(time, HP percent, attacked count)`, called by `CMob::OnAttacked` and the like (`CMob::LuaCall`), and call `Mob:Lua…`
methods: in the 8 scripts `LuaAttackAI(skill)`, `LuaSayByIndex(sysmsg id, radius)` (`0x110A`), `LuaCreateMob(id, count, x, z)`,
`LuaDeleteMob(id, count, x, z)`, `LuaFixedMove(…)`, `LuaSetHoldPosion(1)`, `LuaSetUnBeatable(1)`, `LuaUpdateInsZonePortal(1001, 0)`;
the binary has more (`LuaSay`, `LuaSayByVoice`, `LuaRecallUser`, `LuaRecover`, `LuaGetMobHP`, `LuaGetMobCountByType`, …). They are the
bosses of the Cailum Sacra instance ("카일룸 사크라"; 2471 spawns on maps 65 and 66, the others are summoned by the scripts), and those
maps are party instances (`Map.ini` create type 3). **Analysed in `docs/original/instances.md`** (sections 9 to 16).

## 12. The new server

- **Data**: `tools/world/import.py` reads `mob_ai.ini` and each listed AI's four files as they are (code page 949, file names compared
  without case) into `world.mob_ai` (`ai`, `path`, `alo`, `afs`, `ai_text`, `ah`), and `kr_string.DB` into `world.ai_strings` (`id`,
  `text` as bytes, `english`, deviation 1). The game server parses them at start (`pg::load_mob_ai`, `crates/game-server/src/mob_ai.rs`):
  an AI that does not parse is logged and its monsters stay ordinary monsters (the original does not start). All 27 parse.
- **The machine** (`mob_ai.rs`): `Machine::parse` (sections 2 and 4), `evaluate` with the original's typing (`operate`, `compare`), and
  per monster a `Script` (current state, variables, `Flags`: the attacked and battle flags, battle type, hit count, recovery, the
  multipliers, the defence element, the selected and exit positions) in `MobMap::scripts`. A monster whose `AI` has a machine is
  created in **`MobState::Scripted(target)`** (state 100) with `Mob::scripted` set.
- **The turn** (`crates/game-server/src/zone/scripted.rs`, `Zones::scripts`, run every second after the idle pass): a pending
  start-over, the transitions, the update, as in section 3 (the current state set after the new state's enter); state 3 takes a
  `ReturnHome` step. The 37 functions and the handlers are the original's; `Battle` runs the existing fight turn (`AttackAI`).
- **Hand-backs** (section 7): `scripted::become_idle` where an ordinary monster becomes idle in the chase (giving up at 45) and going
  home (arrived, home moved, no way); `scripted::lose_target` where it loses its target (deviation 6): `Battle` looks for another at
  once or starts over. `SetTarget` (`set_target`) has the state-100 path; `ReTargetByAggro` (`retarget`) and taunts keep it in its
  machine (deviation 4); the attack pass and the idle pass treat a scripted monster in states 0, 1 and 3
  as an ordinary one. Players' hits set the flags (`scripted::mark_attacked`), `CMob::Damage` counts hits, the recovery follows
  section 7, the attack kinds stop hits (deviation 5), and the multipliers reach the normal attack (`combat::mob_strike` strength, `fight::next_attack` speed) and the defence a
  player's attack meets (`scripted::scripted_fighter`: melee defence and element).
- **Speech**: `0x1109` (`shaiya_proto::chat::monster_say`), to the target's party members on other maps as well. **Summons** create
  monsters that never respawn (section 9).
- **Not built here**: the unused handlers (`MoveRunAway`, `Debug`: parsed, they do nothing). The Lua monsters are built,
  `docs/original/instances.md` section 18 (`crate::mob_lua`, `zone/lua.rs`: the eight shipped scripts as they are, every hook and function).
- **Deviations** (1 to 6 approved; 4 to 6 requested):
  1. **English lines**: `world.ai_strings.english` (translations of the 31 Korean lines, appendix) is what `Say` and `Say2` send; the
     original's Korean bytes show as garbage in this client. A row without English sends the original text.
  2. **`0x1109` without the line end, with its NUL**: `len` counts the NUL, as the client reads it (`docs/client/scripted-monsters.md`);
     the original sends the file's `"\n"` and no NUL.
  3. **`SetDrMove` is relative to the monster**: the selected position is the monster's position plus the offset, so `SorpNamed1` runs
     before casting (section 10). The other checks (20 from home, a free line) stay.
  4. **A monster running its machine keeps it when its target changes**: `ReTargetByAggro` and a taunt (`wType` 72) give it the new
     target in state 100 (the original puts it in state 1, the ordinary fight, so its script stops until it has been home; section 7).
  5. **The attack kinds are read**: a player's normal blow or skill hit of a kind the script turned off (`SetEnableOnlyAttackType`,
     `SetEnableATAll`, a summon's kinds; 0 melee, 1 ranged, 2 magic, 3 none of them) misses, as on a monster going home. Damage over
     time already applied still runs. The original writes the setting and never reads it (section 7), so `SorpNamed1`'s 5-second
     "OverCoat", `ZinAlcaria`'s shield, the golden pig's "melee only" and its box's did nothing.
  6. **A monster with a machine that loses its target looks for another player and goes on**, starting over only when it finds none:
     the target killed by its blow or skill, gone, or out of reach of a monster that does not walk (the original: after a kill the same
     in state 100; the other cases set state 0 and leave the machine).
  - Unlike the original, an unknown skill id in `UseMobSkill` does nothing (no crash), and `SetSummonMob` with a radius below 0.5 does
     nothing (`rand() % 0` in the original).

## Appendix: the lines (`kr_string.DB`) and the English of deviation 1

| id | original (Korean) | English |
|---|---|---|
| 1 | 이단자들에게 철퇴를! | Smite the heretics! |
| 2 | Li laka Amon-Ra | Li laka Amon-Ra |
| 3 | 뼈와 살을 분리해주마!! | I'll tear the flesh from your bones!! |
| 4 | 배신자들에게 피와 죽음의 향연을!! | A feast of blood and death for the traitors!! |
| 5 | 죽어라! 어리석은 것 | Die, fool! |
| 6 | 네 실력을 한 번 보자 | Let's see what you can do. |
| 7 | 죽음의 시간이다 | It is time to die. |
| 8 | 누구도 죽음을 피해가지는 못한다 | No one escapes death. |
| 9 | 저승길을 재촉하는구나!! | You hasten down the road to the underworld!! |
| 10 | 흐흐흐~ 오랫만에 고기맛을 보겠군. | Heh heh heh~ Fresh meat, at long last. |
| 11 | 과거의 잘못은 너희들의 죽음으로 보상 받으리라~ | Your deaths shall pay for the wrongs of the past~ |
| 12 | 우리들의 고통을 너희들도 느껴보아라~ | Now you shall feel our pain~ |
| 13 | 피와 광기의 폭풍을!!! | A storm of blood and madness!!! |
| 14 | 헤..헬투스님들의 여..영광을 위해!! | F-for the g-glory of the Helltooths!! |
| 15 | 이..이곳의 대..대장은 나..나다!! | I-I'm the b-boss around h-here!! |
| 16 | 너..너를 죽일거야 | I-I'm gonna kill you. |
| 17 | 꿈의 세계에 온 것을 환영한다 | Welcome to the world of dreams. |
| 18 | 신에게 대항하다니! 어리석은 것들~ | You dare defy a god! Fools~ |
| 19 | 이것이 끝이라고 생각하면 오산이다!! | If you think this is the end, you are mistaken!! |
| 20 | 게헤나의 불속에 부활할 것이다 | I shall rise again in the fires of Gehenna. |
| 21 | 여기까지 날 몰아붙인 것은 칭찬해주지. 그러나 유희는 여기까지다!! | I'll grant you credit for driving me this far. But the game ends here!! |
| 22 | 피의 축제에 온 것을 환영한다. 어리석은 피조물들아~ | Welcome to the festival of blood, foolish creatures~ |
| 23 | 벌레들과 상대하려니 피곤하군. 좀 쉬어볼까 | Swatting insects is tiresome. Perhaps I'll rest a while. |
| 24 | 불의 힘이 나를 감싸리라 | The power of fire shall wreathe me. |
| 25 | 나를 경배하라! 에테인의 피조물들아 | Worship me, creatures of Etain! |
| 26 | 앱소션 쉴드 전개 | Absorption Shield, deploy. |
| 27 | 물의 힘이 나를 보호하리라 | The power of water shall protect me. |
| 28 | 대지의 견고함이 나를 보호할 것이다 | The strength of the earth shall protect me. |
| 29 | 바람이여~ 나를 감싸라 | Wind~ wrap yourself around me. |
| 30 | 모든 것은 무로 돌아갈 것이다. | All shall return to nothing. |
| 31 | 너희들은 넘지말아야할 선을 넘어섰다. 보아라! 너희들의 하찮은 힘을... | You have crossed a line that must not be crossed. Behold! Your feeble power... |

## 13. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
and their `AI/`, `mob_ai.ini` and `Lua/` files, read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and
[`ep5.4.md`](ep5.4.md). Function names in those builds are carried over from EP4.5 (matched by strings, the call graph or switch
tables, as those pages list).

### EP3

- **No Lua monsters** (confirmed): no Lua library strings, no `Lua*` methods, no `OnAttacked`/`OnDeath`/`OnMoveEnd`, no `/aiset`.
- **The state machine is there**: the same handler names, and `CKFSM<CMob>::TravelCondition` (EP3 ps_game 0x47a5a0) has the same three
  switches of 8, 11 and 6 cases (confirmed). But `CMob::SetTarget` (EP3 ps_game 0x4810f0) has no state-100 branch and no `SetTargetEx`
  call (confirmed), so the hook of section 7 is not there in that form; how an EP3 scripted monster takes a target was not read.
- **Files** (confirmed): 152 AI files and a `mob_ai.ini` without lines 35 and 36. EP3 lacks the Haruhion, Freezing, `scorlp1` and
  `testshaiya` sets; its `CrypticOne.ai` has 10 states (EP4.5 11), its `Belizabeth.ai` 8 (EP4.5 9, one threshold 0.6 → 0.35), and EP4.5's
  `helltouthnamed1_AI_FUNCTION.H` adds functions EP3 lacks (`BEGINCHECKATTACKCOUNT`, `ADDENERGY`, `SETHPCHAGE`, `SAY2`, …).

### EP5.4

- `TravelCondition` (EP5.4 ps_game 0x4ac2f0) and `CalculateParam` (0x4b0f50) have the same switch sizes, and `CMob::ConnectToFSM`
  (0x4a29b0) registers the same seven handlers (confirmed), so the catalogue of sections 4 to 6 holds (inferred: the case bodies were not
  compared one by one).
- The 180 AI files are EP4.5's; `mob_ai.ini` drops lines 35 (Haruhion) and 36 (Freezing), which no monster uses in either episode, so
  nothing changes (confirmed).
- The `LuaMob` table (EP5.4 ps_game 0x581ea8) keeps the 23 methods and adds three (`LuaGetMobUID`, `LuaUseSkillAtMob`, `LuaMobCommand`
  with an `OnCommand` callback; [`instances.md`](instances.md)). 27 new monsters have Lua AI numbers 121 to 185 with no script shipped
  (confirmed, data).
- EP5.4 `LuaDeleteMob` (EP5.4 ps_game 0x4cb790) reads a fifth number argument (confirmed), most likely the radius that EP4.5 fixes at
  100 (inferred).
