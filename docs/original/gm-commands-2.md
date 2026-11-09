# GM commands 2: questions, players, summons and notices

This page covers the second half of the operator (GM) commands in the EP4.5 game server `ps_game.exe` (built 2009-07-15): the user
questions (players' `/request GM`, `0x1201`, and the GM's question window, `F201` to `F204`), the player commands of `PacketAdminCmdC`
(`F801` to `F804`, `F80B` to `F80E`: cure, clear bags, kick, quests) and `PacketAdminCmdD` (`F901`, `F904` to `F90C`: the character
sheet, summons, notices). It was read in the Ghidra decompiler and the disassembly with the PDB names and types. Nothing was executed.

Read: `CUser::PacketUQ` 0x470e50, `CUser::PacketAdminUQ` 0x471850, `CUserQuestionMgr::AddQuestion` 0x448b60, `GetQuestion` 0x448d60,
`AddAnswer` 0x448ea0, `AddAnswerSend` 0x448fb0, `FindRemoveQuestion` 0x4490c0, `NotAnswered` 0x448d10, `EnterAdmin` 0x448920,
`LeaveAdmin` 0x448990, `SendAdminAll` 0x448ac0, `UpdateAdminNotice` 0x448770, `SSyncList<UserQuestion>::RemoveFront` 0x449270,
`GetSystemTime` 0x4cb8e0, `CUser::LeaveWorld` 0x44aad0 (its staff part, 0x44ab27 to 0x44ab54), `CUser::PacketAdminCmdC` 0x473240,
`UserSendChatTo` 0x473120, `CUser::ClearBag` 0x45eda0, `CUser::ClearEquipment` 0x45ed80, `CUser::ItemRemoveFree` 0x45f470,
`CUser::SendRecoverAdd` 0x481f20, `CUser::QuestAddAdmin` 0x47e280, `CUser::QuestRemoveAdmin` 0x47e2e0 (with `QuestAdd` 0x47e010 and
`QuestStart` 0x47ea70), `CUser::PacketAdminCmdD` 0x473970, `CUser::UpdateAdminNotice` 0x48e090 (and its caller
`CZone::UpdateApplyEffect` 0x422191), `CUser::UpdateResetPosition` 0x48d470 (the branch for kinds other than 1 to 6, 0x48dd8c to
0x48ded6), and the PDB layouts of `UserQuestion`, `CUserQuestionMgr`, `stAdminInfo`, `stAdminNotice`, `CSkill`, `stSkillInfo`,
`stQuestInfo` and `CItem` (`ps_gamewar.pdb` of the same pack: the `CUser` fields after `stUserInfo`'s kill counts sit 4 bytes later in
`ps_game.exe`, and those from `stAdminInfo` on 8 bytes later; every offset below is `ps_game.exe`'s, matched to the PDB names by their
use).

Everything [GM commands 1](gm-commands.md) established (the family limits, `F501`, the GM's state `+0x5774` to `+0x5788`, the
deferred moves `+0x5818`, `FindUser`, `FindUserZone`, `FindUserZoneMove`, `EqualZone`) is used as written there. The client side is on
the client's [GM opcodes and commands](../client/gm-and-commands.md) page; points from it are marked "client". Not covered: the game log
records, and the six `F8xx` commands the original has no case for (`F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`; they disconnect the
GM).

## In short

- **Players ask the staff questions** with `/request GM <text>`; the questions wait in one queue for the whole server. A staff member
  takes the oldest with `/UQGET`, answers it in the question window and sees how many wait with `/UQCOUNT`.
- **An answer never reaches the player.** It only goes to a log, and the questions live in memory only: a restart loses them all.
- **Level 3 staff** can heal and cure, empty a player's bags or equipment, kick, and give or take quests. **Level 4** can read a
  character sheet, summon one player or a whole party, and post notices to the staff, those around, one player, the zone, a faction or
  everyone, each optionally repeated every so many seconds.
- **`/cure` and `/event` act on the GM itself** when the named player is not found.
- **`/questadd` skips every start condition** and gives a finished quest again; **`/questrem`** deletes the quest so it can be taken
  again, and strips its start items even from the equipment.
- **Summons** pull players out of instance zones, guild houses and the ranking battle, dead or alive, staff included.
- **Repeating notices follow the GM** and end when the GM leaves the world.

Status words: **confirmed** = read in the code (address given); **inferred** = follows from what was read but is not shown directly;
**unknown** = not read. Sizes include the 2-byte opcode.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names and types), read with the decompiler (REA) and the disassembly (`tools/disasm.py`, names from
`docs/original/symbols/ps_game.tsv`): `CUser::PacketUQ` 0x470e50, `CUser::PacketAdminUQ` 0x471850, `CUserQuestionMgr::AddQuestion` 0x448b60,
`GetQuestion` 0x448d60, `AddAnswer` 0x448ea0, `AddAnswerSend` 0x448fb0, `FindRemoveQuestion` 0x4490c0, `NotAnswered` 0x448d10, `EnterAdmin`
0x448920, `LeaveAdmin` 0x448990, `SendAdminAll` 0x448ac0, `UpdateAdminNotice` 0x448770, `SSyncList<UserQuestion>::RemoveFront` 0x449270,
`GetSystemTime` 0x4cb8e0, `CUser::LeaveWorld` 0x44aad0 (its staff part, 0x44ab27 to 0x44ab54), `CUser::PacketAdminCmdC` 0x473240,
`UserSendChatTo` 0x473120, `CUser::ClearBag` 0x45eda0, `CUser::ClearEquipment` 0x45ed80, `CUser::ItemRemoveFree` 0x45f470,
`CUser::SendRecoverAdd` 0x481f20, `CUser::QuestAddAdmin` 0x47e280, `CUser::QuestRemoveAdmin` 0x47e2e0 (with `QuestAdd` 0x47e010 and
`QuestStart` 0x47ea70), `CUser::PacketAdminCmdD` 0x473970, `CUser::UpdateAdminNotice` 0x48e090 (and its caller `CZone::UpdateApplyEffect`
0x422191), `CUser::UpdateResetPosition` 0x48d470 (the branch for kinds other than 1 to 6, 0x48dd8c to 0x48ded6), and the PDB layouts of
`UserQuestion`, `CUserQuestionMgr`, `stAdminInfo`, `stAdminNotice`, `CSkill`, `stSkillInfo`, `stQuestInfo` and `CItem`
(`ps_gamewar.pdb` of the same pack: the `CUser` fields after `stUserInfo`'s kill counts sit 4 bytes later in `ps_game.exe`, and those
from `stAdminInfo` on 8 bytes later; every offset below is `ps_game.exe`'s, matched to the PDB names by their use). Nothing was executed.
The client side is `docs/client/gm-and-commands.md` (read only; its findings are used here and marked "client"); the client itself was
not read for this. Everything part 1 established (`docs/original/gm-commands.md`: the family limits, `F501`, the GM's state `+0x5774` to
`+0x5788`, the deferred moves `+0x5818`, `FindUser`, `FindUserZone`, `FindUserZoneMove`, `EqualZone`) is used as written there.

**Part 2** is what part 1 left out: the user questions (players' `/request GM`, `0x1201`, and the GM's question window, `F201` to `F204`),
`PacketAdminCmdC` (`F801` to `F804`, `F80B` to `F80E`) and `PacketAdminCmdD` (`F901`, `F904` to `F90C`). The six `F8xx` commands the
original has no case for (`F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`) were built in part 1 (its deviations 14, 16, 17). **Not
covered**: the game log records every command and question writes (`SendLogAdmin`, the question records), as in part 1 (**a
correction**, `docs/original/game-log.md` section 4: the questions' `0x0401` to `0x0403` go to `g_pClientToLog`, a different log connection, not to
the game log, where those types are item records; the commands' `SendLogAdmin` records are in the game log); `FA0B`/`FA0C` (part 1, its deviations 21 to 24); the market commands `FA19`/`FA1A`; map 200.

Status words as in `docs/original/ps_game.md`: **verified** = read in the code (address given); **inferred** = follows from verified code
but is not shown directly; **unknown** = not read.

## 1. What it is

Players ask the staff questions with `/request GM <text>`; the questions wait in one queue for the whole server. A staff member takes the
oldest with `/UQGET`, answers it in the question window (which closes it) and sees how many wait with `/UQCOUNT`. Staff of level 3 or
higher (numbers 1 to 3) can heal, cure, empty the bags or equipment of, kick, and give or take quests from players; level 4 or higher can
read a player's character sheet, summon players (one, or a whole party), and post notices to the staff, to those around, to one player, to
the map, to a faction or to everyone, each notice optionally repeated every so many seconds.

## 2. Who may send what

Confirmed.

- **Families** (part 1 section 2): `F2xx` (`PacketAdminUQ`) needs level ≤ 10 (any staff), `F8xx` (`PacketAdminCmdC`) ≤ 3, `F9xx`
  (`PacketAdminCmdD`) ≤ 4. A higher level **closes the connection** (`SConnection::Close(9, 0)`), so does an opcode without a case.
- **Cases**: `F201` to `F204` (anything else in `F2xx` closes); `F801`, `F802`, `F803`, `F804`, `F80B`, `F80C`, `F80D`, `F80E` (jump table
  `opcode − 0xF801`, 14 entries: `F805` to `F80A` close); `F901`, `F904` to `F90C` (jump table `opcode − 0xF901`, 12 entries: `F902`, `F903`
  close; `F90D` and above close).
- `0x1201` is a player opcode (family `0x12`, `CUser::PacketUQ`): any other `0x12xx` closes the connection.
- The client's own gates (client section 2a): `/request GM` for everyone; `/UQGET`, `/UQCOUNT` and the window for any staff; the `F8xx`
  commands at level 3; the `F9xx` commands at level 4.

## 3. User questions

Confirmed.

### 3.1 Asking: `0x1201` (`CUser::PacketUQ` 0x470e50)

`0x1201 u16 n, text[n]` (4 + n bytes). The client sends the text after `/request GM ` with its terminating zero, n counting it, and prints
252 "Successfully applied" by itself (client section 4). The server takes **2 ≤ n ≤ 1000** (else ignored, no answer), writes a zero at the
text's last byte, and `CUserQuestionMgr::AddQuestion` (0x448b60) queues a `UserQuestion` (size 0x858):

| field | from |
|---|---|
| `dwQuestionID` | the manager's counter `m_dwQuestionID` (`+0`), then counted up (a number unique while the server runs; its start is `SetUniqueIDStart`) |
| `qwQuestionDBID` | `SUniqueIDGen::GenID` (the game log's key) |
| `dwQuestionUserID`, `szQuestionUserName[32]` | the account id `+0x578c` and account name `+0x5794` |
| `dwQuestionCharID`, `szQuestionCharName[21]` | the character id `+0x128` and name `+0x188` |
| `wQuestionLen`, `szQuestion[1000]` | n and the text |
| `dwQuestionTime` | `GetSystemTime()`: the local time packed as `((((((year − 16) × 16 + month) × 32 + day) × 32 + hour) × 64 + minute) × 64 + second`, in 32 bits (the year keeps 6 bits: `(year − 16) mod 64`) |

It goes to the **back** of the waiting list `m_listQuestion` (`+4`, count at `+0x24`), and the log connection `g_pClientToLog` gets `0x0401` (the question; not the game log, see the correction in the header). No
check of the player's state, of how many it has asked or of the queue's size; when the question heap is exhausted (`Alloc` returns null)
the question is dropped silently. Nobody is told that a question arrived. *Inferred*: only in the world, as the other world families (the
dispatch in `PacketProcessing` 0x4672de).

### 3.2 The staff list (`CUserQuestionMgr`)

`m_pAdminList[256]` with `m_nNumAdmin` (`+0xac`): `EnterAdmin` (from `CUser::EnterWorld`, level ≤ 10) adds the staff member, sets it
invisible and untouchable (part 1) and gives it a **notice slot** (`stAdminNotice`, 0x53c bytes, zeroed, `+0x5788`; section 5.4). A full list
(256) closes the connection. `LeaveAdmin` (from `CUser::LeaveWorld`) takes it out, frees the slot (so its repeating notices stop) and drops
its listening link. The list is "who entered the world as staff": `/apower` later changes nothing in it.

### 3.3 `F201` `/UQGET`: take the oldest question (`GetQuestion` 0x448d60)

`F201` (2 bytes). When the GM **already holds** a question (`stAdminInfo::dwProcessUQID`, `+0x576c`, not 0): **no answer at all**. Else the
front of the waiting list is taken (`RemoveFront`: the oldest), its id becomes the GM's held question, it moves to the "in process" list
`m_listQuestionProcess` (`+0x30`, count `+0x50`), and the GM gets

`F201 u16 n, u32 question id, u32 dwQuestionTime, char name[21], u16 len, text[len]` (35 + len bytes)

with **n = the waiting count + 1** (the count before this one was taken) and the asking character's name. An empty queue: `F201 u16 0`
(4 bytes). The client keeps the question in its list and opens the window (client).

### 3.4 `F202`, `F203`: answering (`AddAnswer` 0x448ea0, `AddAnswerSend` 0x448fb0)

- **`F202`** (the window's first three buttons, client): `u8 kind (1 to 3), u32 question id, u16 n, text[n]` (9 + n). **1 ≤ n ≤ 1000**, else
  ignored. A zero is written at the text's last byte.
- **`F203`** (the fourth button, "answer to a player", with a name field, client): `u32 question id, char name[21], u16 n, text[n]` (29 + n),
  the same n rule; the name ends at +0x1a.
- Both: the GM's held question is cleared (`+0x576c = 0`) **whatever the id**; then the question with that id is taken out of the in-process
  list (`FindRemoveQuestion`: **any** GM's question, not only the one this GM holds); none → nothing more. Found: the game log gets `0x0402`
  (`F202`: the question's log key and time, now, the kind, the GM's account name, the text) or `0x0403` (`F203`: the same with the typed
  name instead of the kind), and the question is freed. **Nothing is sent to anyone**: not to the GM, not to the asker, not to the named
  player. The kind's meaning (the three buttons' labels) is **unknown**.

### 3.5 `F204` `/UQCOUNT`

`F204` (2) → `F204 u16 waiting count` (4); the client prints 344 "Remaining number of QUEUE : <v>".

### 3.6 A GM leaving (`CUser::LeaveWorld` 0x44ab27)

For level ≤ 10: `SendAdminSendToEnd` (the "GM talk" of `F107`, part 1), then, when it holds a question, `NotAnswered`: the question leaves
the in-process list and goes to the **front** of the waiting list (the next `/UQGET` gets it again), then `LeaveAdmin`. A question taken
by a GM who never answers stays held until that GM leaves; a restart loses every question.

## 4. Player commands: `F8xx`

Confirmed, `PacketAdminCmdC` 0x473240; level ≤ 3.

Every name is `char name[21]` right after the opcode, ended by a zero written at +0x16. Each success answers `F501 0` (part 1 section 3) and
writes a game log record; each failure answers `F501 <op>`.

### 4.1 `F801` `/cure [name]` (the client sends its own name when none is given)

`name[21]` (23 bytes). The target: `FindUserZone(name)` (in the world), **or the GM itself when nobody of that name is**. The target must be
in the GM's zone (`EqualZone`) and not dead (`+0x142c` ≠ 1), else `F501 F801`. Then:
1. every learnt skill's cooldown ends (`CSkill::m_dwResetTime`, `+0x1c` of each of `m_pSkill[m_nNumSkill]`, `+0xac4`/`+0xac0`, set to 0);
   **nothing is sent**: the client's own cooldown display does not change (*inferred*: it keeps refusing the skills until its own timers
   end);
2. every applied skill whose definition has `byTypeEffect` 3 (**the debuffs**; buffs are 4, `docs/original/skills-buffs.md`) gets
   `m_dwEndTime = 0`: they end at the next pass over the applied skills (*inferred*: with their usual `0x050E`);
3. `SendRecoverAdd(max HP, max SP, max MP)`: HP, MP and SP full; `0x0505 u32 id, i16 hp, i16 mp, i16 sp` (what was gained) to the players
   within 60 (`CZone::PSendView`) when anything changed;
4. `F501 0`.

The client's `/autocure` repeats `F801` with the GM's name every 100 ms, but only towards the developers' server (client).

### 4.2 `F802` `/iclear <name>`, `F803` `/eclear <name>`

`name[21]`. The target: `FindUserZoneMove` (in the world, no map change pending) and in the GM's zone; else `F501 op`.
- **`F802`**: `CUser::ClearBag`: every slot (0 to 23) of bags **1 to the usable bag count** (`+0x1304`, `m_nMaxBag`) through
  `ItemRemoveFree`: the item leaves any trade window (`ExchangeItemRemove`), the database agent gets `0x0702` (item gone), the game log a
  record. **No packet per item**: the target gets **`F802`** (2 bytes) and its client empties its own bags and prints 337 (client). Then
  `F501 0`.
- **`F803`**: `CUser::ClearEquipment`: the 13 equipment slots (bag 0) the same way, each also with its wear saved when it changed
  (`0x070C`), `ItemEquipmentRem` (its stats off), `SendEquipment` (the looks to the view) and `SetAttack`. The target gets **`F803`** and its
  client empties its equipment and prints 338. Then `F501 0`.

The warehouse, the guild bank and the money are not touched. Staff targets are not protected.

### 4.3 `F804` `/kick <name>`

`name[21]`. `FindUser(name)` (any state); none → `F501 F804`. Found: **the target's** connection is closed (`Close(9, 0)`, no packet), then
`F501 0`. The client has a handler for `F804` (339 "Log out by operator"), which the original never sends (client).

### 4.4 Quests: `F80B` `/questadd <name> <id>`, `F80C` `/questrem <name> <id>`

`name[21], u16 quest` (25 bytes; the quest at +0x17). The client takes the id, or the name from its own quest data (client). The target:
`FindUserZoneMove` and in the GM's zone; the quest: `CQuestData::GetQuest` (1 to the number of quests); any failure → `F501 op`.

- **`F80B`** (`QuestAddAdmin` 0x47e280): refused when the quest is **in progress** (`QuestFind`). Otherwise `QuestAdd` and `QuestStart` as
  for an ordinary start (`docs/original/quest.md` section 5: counters 0, the time limit, `0x0A01` to the database agent, **`0x0902 u32 0,
  u16 quest`** to the target, then the start items `byGetItem1` to `3` into the first empty slots, **lost when the bags are full**). **None of
  the start checks apply**: not the level, job, country, mode, previous quests, start NPC or start item, and **not `QuestFindFinish`**: a
  finished quest is given again. `F501 0`.
- **`F80C`** (`QuestRemoveAdmin` 0x47e2e0): the quest must be **in progress** (`QuestFind`), else `F501 F80C`. The target gets **`0x0903 (npc 0,
  quest, success 0, result 0, exp 0, money 0, bag 0, slot 0, type 0, type id 0)`** (22 bytes, "failed"); the quest leaves the list and its
  kill numbers go down (end types 2 and 3); the database agent gets **`0x0A02 (char, quest, 2)`**, which **deletes** the quest's row (as for
  a repeatable quest, quest spec section 9): the quest is **not** kept as finished and **can be taken again**. Then every item of the quest's
  start items (`byGetItem1` to `3`, by type and type id) is removed with `CUser::ItemDelete` (part 1 section 7.2: **every** such item, from
  **bag 0, the equipment, to the last bag**). `F501 0`.

### 4.5 `F80D` `/event on`, `F80E` `/event off` (heal and cure, in this `ps_game.exe`)

`char event[21]` (23): the client sends the word after `/event on|off` (client). The server reads it as a **character name** and acts as
`F801`: `FindUserZone`, **else the GM itself**; the GM's zone; not dead.
- **`F80D`**: only `SendRecoverAdd` with the maxima (HP, MP, SP full). `F501 0`.
- **`F80E`**: every cooldown ends, **every applied skill** (buffs, debuffs, and the item buffs that survive death alike) gets `m_dwEndTime =
  0`, and the refill. `F501 0`.

Each then whispers the GM (`UserSendChatTo` 0x473120): **`F102 u8 0, char name[21] = "<GM Cmd>", u8 len, text`** (25 + len) with text = the
result + `" ->"` + the target's name + zero: `"cure ok ->Name"` (`F80D`) or `"cure, buff,debuff clear ok ->Name"` (`F80E`). On a failure (other
zone, dead) the GM gets `F501 op` **and** the whisper `"cure failed ->Name"` / `"cure, buff, debuff clear failed ->Name"`.

## 5. Information, summons, notices: `F9xx`

Confirmed, `PacketAdminCmdD` 0x473970; level ≤ 4.

### 5.1 `F901` `/quiry <name>`: the character sheet

`name[21]` (23). `FindUserZone(name)` (in the world, any zone); none → `F501 F901`. Found: **`F501 0` first**, then to the GM

| bytes | field | from |
|---|---|---|
| 1 | mode | `m_byGrow` (`+0x12f`) |
| 2 | level | `+0x136` |
| 4 | exp | `+0x13c` |
| 4 | money | `+0x140` |
| 2, 2 | stat points, skill points | `+0x138`, `+0x13a` |
| 6 × 2 | STR, DEX, REC, INT, LUC, WIS | the **totals** (`CUser::m_wStr` … `+0x1220`, `+0x1224`, `+0x1230`, `+0x1228`, `+0x1234`, `+0x122c`; *inferred* totals: the `CUser` copies after `stUserInfo`, which hold the base stats separately) |
| 2, 2 | HG, VG | `+0x168`, `+0x16a` |
| 1, 1, 1 | CG, OG, IG | `+0x16c`, `+0x16d`, `+0x16e` |
| 1 | k | the number of items below |
| k × 32 | the equipment | for each of bag 0's 13 slots that holds an item: `u8 slot, u8 type, u8 type id, u16 quality, u8 gems[6], char craft name[21]` |

as **`F901`** (37 + 32 k bytes). The client prints the sheet and the equipment lines (client).

### 5.2 Summons: `F904` `/asummon`, `F905` `/bsummon`, `F90C` `/partysummon`

All three are **deferred moves of the target** as part 1's `FA09` family (part 1 section 8.3): the move is stored in the **target's**
`+0x5818` to `+0x582c` (kind = the opcode, now, map, x, y, z) when the target has no map change pending (`+0x57dc` 0; otherwise nothing is
stored but the GM still gets `F501 0`), and `CUser::UpdateResetPosition` carries it out on the target's next pass (the branch for every kind
other than 1 to 6, 0x48dd8c): **same map** → `CZone::MoveUser`, `0x020B (id, map, x, y, z)` to the view, then **`op u32 id, u16 map, f32 x,
y, z`** (20 bytes, the command's opcode) to the target; **another map** → `CWorld::ZoneLeaveUserMove` and the same 20-byte packet. The
target's client: `F904` with its own id prints 336 "Summon by operator" and moves (a map change on another map); `F905` and `F90C` move it
with **the height taken from the terrain** (the server sends y 0) and then send a movement packet (client).

| command | wire | checks (else `F501 op`) | destination |
|---|---|---|---|
| `F904` `/asummon <name>` (the client sends `FA08` `/watch` first) | `name[21]` (23) | the GM in the world (`+0x5758` 4) and in a zone (`+0xe0`); **the GM's map has no `CreateType` (`CWorld::GetMap(+0x164)`, `+0x1a4` 0), else NO answer at all**; the target by `FindUserZoneMove` (any map, any zone, also an instance) | the GM's map and x, y, z |
| `F905` `/bsummon <name> <x> <z> [map]` (`FA08` first) | `name[21], f32 x, f32 z, u16 map` (33) | the target by `FindUserZoneMove`; the map known (`GetMap`), **without a `CreateType`**, `0 ≤ x < size`, `0 ≤ z < size` (`+4`) | that map (its id `+0x78`), x, **y 0**, z |
| `F90C` `/partysummon <name> <x> <z> [map]` | as `F905` | the map known, x and z in bounds (`F501 F90C`); **a `CreateType` → `F501 F905`** (the wrong opcode: the jump goes to `F905`'s error); the target by `FindUserZoneMove` (else `F501 F90C`) | as `F905`, for the target alone when it has no party, else for **every member of its party in the world** (`+0x5758` 4), on any map, each with its own pending check (`CPartyLock` held) |

No command checks the target's level (staff can be summoned), whether it is dead, or its zone (a player in an instance zone, a guild house
or the ranking battle is pulled out). The GM gets `F501 0` once the moves are stored.

### 5.3 Notices: `F906` to `F90B`

| command | wire | goes to | repeat slot |
|---|---|---|---|
| `F906` `/gmnotice <text>` | `u8 len, text` | every staff member in the staff list (section 3.2; `SendAdminAll`), the GM included | none |
| `F907` `/cnotice [s] <text>` | `u16 repeat, u8 len, text` | the GM's view (`CObject::SendViewChat`, as `F101`; *inferred* within 30, the new server's `NORMAL_CHAT_RANGE`) | `dwNormal` |
| `F908` `/wnotice <name> [s] <text>` | `name[21], u16 repeat, u8 len, text` (len at +0x19) | that player (`FindUserZone`, in the world; none → `F501 F908`) | `dwTo` (with the player's id) |
| `F909` `/znotice [s] <text>` | `u16 repeat, u8 len, text` | every player of the GM's zone (`CZone::SendAll`; nothing when the GM is in none, still `F501 0`) | `dwMap` |
| `F90A` `/bnotice <빛\|어둠> [s] <text>` | `u8 country, u16 repeat, u8 len, text` (len at +5) | every player of that country on the server (`CWorld::SendAllCountry`); **the country is not checked** (2 and above reach nobody) | `dwCountry` (with the country) |
| `F90B` `/notice [s] <text>` | `u16 repeat, u8 len, text` | everyone on the server (`CWorld::SendAll`) | `dwAll` |

The packet sent is **`op u8 len, text`** (3 + len, the command's own opcode); the client plays `ui_news001.wav` and prints 356 "[Notice] <b>"
(client). For each:
- `F906`: **2 ≤ len ≤ 128**, else `F501 F906`. `F501 0`.
- `F907` to `F90B`: the GM's notice slot must exist (staff always have one, section 3.2), else `F501 op`. **len < 2**: the repeat of that
  kind stops and `F501 0` (nothing sent; the client's "off" commands send `0, 0`); **len > 128**: `F501 op`. Otherwise: **repeat 0** stops
  that kind's repeat; **repeat s > 0** sets it (`due = now + s × 1000 ms`, `period = s × 1000`, the length and text kept, for `F908` the
  target's id, for `F90A` the country); the notice is sent once now, then `F501 0`.
- **`F908` with len < 2 stops the `F907` repeat** (`dwNormal`, 0x474321), not its own (`dwTo`); there is no way to stop a repeating whisper
  notice but sending another with repeat 0, which needs the target in the world.

### 5.4 The repeats (`CUser::UpdateAdminNotice` 0x48e090)

On each of the GM's passes of `CZone::UpdateApplyEffect` (the pass that also ends bans, part 1 section 4), for each kind whose due time is
set and past:
- `dwTo`: the player by id (`CWorld::FindUser` 0x40d640, any state); found → `F908` to it and `due += period`; **gone → that repeat stops**;
- `dwNormal`: `F907` to the players within **30** of the GM's position now (`CZone::SendView`); `due += period`;
- `dwMap`: `F909` to the GM's zone now; `dwCountry`: `F90A` to that country; `dwAll`: `F90B` to everyone; `due += period` each.

The repeats follow the GM (its position and zone at each repeat) and **end when the GM leaves the world** (the slot is freed). `due +=
period` catches up after a delay (one per pass until due is ahead again).

## 6. The packets

Confirmed.

| op | dir | size | layout | section |
|---|---|---|---|---|
| `0x1201` | C → S | 4 + n | `u16 n, text[n]` (2 ≤ n ≤ 1000, the zero counted) | 3.1 |
| `F201` | C → S / S → C | 2 / 35 + len or 4 | — / `u16 n, u32 id, u32 time, name[21], u16 len, text` or `u16 0` | 3.3 |
| `F202` | C → S | 9 + n | `u8 kind, u32 id, u16 n, text[n]` (1 ≤ n ≤ 1000) | 3.4 |
| `F203` | C → S | 29 + n | `u32 id, name[21], u16 n, text[n]` | 3.4 |
| `F204` | C → S / S → C | 2 / 4 | — / `u16 waiting` | 3.5 |
| `F801`, `F802`, `F803`, `F804` | C → S | 23 | `name[21]` | 4 |
| `F802`, `F803` | S → C | 2 | — (to the target) | 4.2 |
| `F80B`, `F80C` | C → S | 25 | `name[21], u16 quest` | 4.4 |
| `F80D`, `F80E` | C → S | 23 | `char event[21]` (read as a name) | 4.5 |
| `F102` | S → C | 25 + len | `u8 0, "<GM Cmd>"[21], u8 len, text` (the result whisper) | 4.5 |
| `F901` | C → S / S → C | 23 / 37 + 32 k | `name[21]` / section 5.1 | 5.1 |
| `F904` | C → S / S → C | 23 / 20 | `name[21]` / `u32 id, u16 map, f32 x, y, z` | 5.2 |
| `F905`, `F90C` | C → S / S → C | 33 / 20 | `name[21], f32 x, f32 z, u16 map` / as `F904` (y 0) | 5.2 |
| `F906` | both | 3 + len | `u8 len, text` | 5.3 |
| `F907`, `F909`, `F90B` | C → S / S → C | 5 + len / 3 + len | `u16 repeat, u8 len, text` / `u8 len, text` | 5.3 |
| `F908` | C → S / S → C | 26 + len / 3 + len | `name[21], u16 repeat, u8 len, text` / `u8 len, text` | 5.3 |
| `F90A` | C → S / S → C | 6 + len / 3 + len | `u8 country, u16 repeat, u8 len, text` / `u8 len, text` | 5.3 |
| `0x0902`, `0x0903` | S → C | 8, 22 | quest started / ended (`docs/original/quest.md`) | 4.4 |
| `0x0505` | S → C | 12 | `u32 id, i16 hp, i16 mp, i16 sp` gained | 4.1 |

Sizes include the 2-byte opcode. The original does not compare a text's length byte with the packet's size (it copies `len` bytes from its
receive buffer).

## 7. The database calls

Confirmed, in ps_game.

| op | from | to ps_dbAgent |
|---|---|---|
| `0x0702` | `F802`, `F803`, `F80C` (each item removed) | the item gone (bag, slot, count) |
| `0x070C` | `F803` | an equipped item's wear, when it changed |
| `0x0A01` | `F80B` | the quest started (char, quest, time limit) |
| `0x0A02` | `F80C` | `(char, quest, 2)`: the quest's row deleted |
| `0x0205`-style item creation | `F80B` | the start items (as `QuestAddItem`) |

The questions and answers go only to the log connection `g_pClientToLog` (`0x0401`, `0x0402`, `0x0403`; not the game log), never to the database: a restart loses them.

## 8. The client (from `docs/client/gm-and-commands.md`, not read here)

- `/request GM <text>` sends `0x1201` and prints 252 at once; `/UQGET` and `/UQCOUNT` are exact words for any staff; the question window
  (`0x496000`) shows a question from `F201` and has four buttons: three send `F202` with kind 1, 2 or 3, the fourth sends `F203` with a name.
- The `F501` failure texts: `F801` 2024, `F802`/`F803` 2025, `F804` 2026, `F80B`/`F80C` 2027, `F80D`/`F80E` 2028, `F901` 2021, `F904`/`F905` 2022,
  `F906` to `F90B` 2023; anything else (`F90C`) 2029 "Can't change". The confirmations on `F501 0`: 282 (`/cure`), 283, 284, 289 (`/kick`), 261, 262,
  285, 278, 279/280/281, 257 (notices; 2000 for the "off" forms).
- Handlers: `F802`/`F803` (337/338, the bags or equipment emptied locally), `F804` and `F709` (339 "Log out by operator", never sent by the
  original), `F80B`/`F80C` (read a `u16`, do nothing; never sent), `F901` (the sheet), `F904`/`F905`/`F90C` (moves), `F906` to `F90B` (notice), `F201`,
  `F204` (344).
- **Open for the client side**: the labels of the question window's four buttons (what kinds 1 to 3 mean); what the window shows of `F201`'s
  second `u32` (the packed time) and of `n`; whether the window clears itself after `F202`/`F203`; whether `F801` changes anything the client
  shows about cooldowns (it is sent nothing); whether `F802`/`F803` empty **all** bags and all 13 equipment slots locally, matching the
  server; whether a `F904`/`F905`/`F90C` for the client's own id on the same map, after the `0x020B` the view gets, moves it twice or once;
  what `F905` with y 0 does on a map change (it takes the height from the terrain, client).

## 9. Original oddities

- **Questions are memory only** and leave no trace in the database: a restart loses the queue and every answer went to the game log only.
- **An answer never reaches the player**, not even `F203`'s, which names one.
- **`/UQGET` while holding a question answers nothing**; a GM can answer another GM's held question, which then stays "held" by the first
  GM (its `/UQGET` answers nothing until it sends any `F202`/`F203`, which clears the hold whatever the id, or leaves the world).
- **No limit on questions**: a player can queue any number of 1000-byte questions.
- **`F801`, `F80D`, `F80E` act on the GM itself when the name is not found**, and answer `F501 0`.
- **`F801`'s cooldown reset is not sent**: the client keeps its own cooldowns (only the developers' `/autocure` clears them locally).
- **`F80E` ends every applied skill**, the item buffs that survive death included (bought ones too).
- **`F803` and `F80C` strip the equipment too** (`F80C` through `ItemDelete`, which also searches bag 0 for the quest's start items).
- **`F804` closes the target with no notice** though the client has one (339).
- **`F80B` skips every start condition**, gives a finished quest again, and loses the start items when the bags are full.
- **Staff are not protected** from `F802`, `F803`, `F804`, `F80B`, `F80C` or the summons by a lower-ranked staff member.
- **`F904` from a map with a `CreateType` answers nothing**; **`F90C` on such a map answers `F501 F905`**.
- **Summons move dead players** and pull players out of instance zones, houses and the ranking battle.
- **`F908` "off" stops the `F907` repeat**, not its own; **`F90A`'s country is not checked**.
- **Repeats catch up** after a delay (one per pass).

## 10. The new server

Everything above as the original, with the deviations below. As in part 1, the checks run in the GM's connection; what changes another
player (cure, clear, kick, quests, summons, the sheet) is an **order** carried out by that player's connection (part 1's `AdminOrder` through
`Zones::admin_orders` and `ZoneEvent::Admin`), which answers the GM through the zone. Each command is logged at `info` ("gm command"), as
in part 1, instead of the game log.

- **Data**: nothing new. **Database**: migration `0023_user_questions.sql` (deviation 1). Quests, items and equipment through the existing
  calls (`start_quest`, `end_quest`, `trade`).
- **Memory**: the waiting and in-process questions (deviation 1) in a server-wide `Questions` store (`Context`), loaded at start; per GM
  connection, the held question's id and the five repeating notices (kind, due, period, text, and the target or country).

**Deviations** (all fifteen approved and built):

1. **Questions are saved**: a table `user_questions (id serial PRIMARY KEY, server_id smallint, character_id integer REFERENCES characters ON
   DELETE CASCADE, character_name varchar(20), question bytea, asked_at timestamptz DEFAULT now(), taken_by integer NULL, answered_at
   timestamptz NULL, answered_by integer NULL, answer_kind smallint NULL, answer bytea NULL, answer_to varchar(20) NULL)` (migration 0023).
   A question is written when asked, marked taken (`taken_by`) by `/UQGET`, and answered (kind 1 to 3 for `F202`, 4 for `F203` with
   `answer_to`, the text, who and when) by `F202`/`F203`; nothing is deleted. At start the unanswered ones load as waiting, oldest first
   (`taken_by` cleared). A GM leaving puts its held question back at the front, as the original. The texts are kept as sent (`bytea`), so the
   GM sees them byte for byte. *Why*: the original's only record is the game log, which the new servers do not have; without this every
   restart loses the queue and nobody can read past answers.
2. **`F203` answers reach the player**: the text goes to the character named in `F203` (not necessarily the asker) when it is in the world,
   as whisper lines from the GM (`F102 u8 0, GM name[21], u8 len, text`, the text cut into pieces of at most 120 bytes, each with its zero);
   not in the world → only saved. **`F202` delivers nothing** (as the original): its kinds are unknown (section 8), so its text may be a
   staff note. *Why*: the client's fourth button is plainly "answer to a player", and an answer nobody receives is useless; once the client
   side names the three buttons, a kind that means "answer" can be delivered the same way.
3. **The held question**: `/UQGET` while holding one sends it again (instead of nothing), and a held question answered by another GM no longer
   blocks its holder (the hold is cleared on every connection holding that id). *Why*: a GM who closed the window could otherwise not get
   the question back, and a question answered elsewhere silently blocked the first GM.
4. **At most 3 unanswered questions per character**: a fourth `0x1201` is ignored (logged at `debug`). *Why*: the original takes any
   number of 1000-byte questions; with deviation 1 they are also stored. (The client still prints "Successfully applied".)
5. **A name not found is refused**: `F801`, `F80D`, `F80E` answer `F501 op` (and `F80D`/`F80E` whisper their "failed" line) when nobody of
   that name is in the world, instead of acting on the GM. The client's `/cure` without a name sends the GM's own name, which still works.
   *Why*: a typo should not cure the GM and report success.
6. **Kicks are announced**: `F804` `/kick` sends **`F804`** (2 bytes) to the target before its connection closes, and part 1's `F709` `/akick`
   sends **`F709`** the same way (part 1's `Kick::Operator` carries the opcode), so the client shows 339 "Log out by operator". *Why*: the
   client has the handler; as part 1's deviation 18 does for bans.
   Amended by deviation 1 of `docs/original/small-items.md` (built): the socket stays open `kick_notice_secs` (3 s) after the notice,
   the character already out of the world, so the client shows 339 before its disconnect box.
7. **Staff are protected from lower-ranked staff**: `F802`, `F803`, `F804`, `F80B`, `F80C` (and part 1's `F709`) on a character whose account is
   staff of the GM's level or higher (its level number ≤ the GM's) answer `F501 op`; the GM acting on itself is allowed. *Why*: as part 1's
   deviation 14 for bans: a level-3 member should not empty or kick a level-1 one.
8. **`F80C` takes the quest's start items from the bags only** (bags 1 and up), not the equipment, as part 1's deviation 5 for `F703`.
   `F803` does strip the equipment, with the stats, looks and mount changes of an ordinary unequip, since that is its whole purpose.
9. **`F80B` start items must fit**: when the quest's start items do not fit in the empty slots, `F501 F80B` and nothing changes (as the
   quest spec's deviation 3 for ordinary starts). As the original, every start condition is skipped, a closed quest (part 1 deviation 16)
   can be given, and a finished quest can be given again: its finished mark is replaced by the new start (one row per character and
   quest, `character_quests`).
10. **`F80E` keeps the item buffs that survive death** (`byDontRemByDeath`); every other applied skill ends, with `0x050E` each. *Why*:
    "cure, buff, debuff clear" should not destroy bought item effects (experience stones and the like).
11. **The silent and wrong answers of the summons**: `F904` from a map with a `CreateType` answers `F501 F904` (the original answers
    nothing); `F90C` on a map with a `CreateType` answers `F501 F90C` (the original answers `F501 F905`).
12. **Summons happen at once, with the original's packet**: the summoned player's connection moves it as soon as it takes the order
    (part 1's moves, deviation 6, use the same `move_player`), but the player itself gets the command's own 20-byte packet (`F904`, `F905`,
    `F90C`: `u32 id, u16 map, f32 x, y, z`) instead of `0x020B` for a move to another map, and after the view's `0x020B` for a move on the same
    map (as the original's `UpdateResetPosition`). *Why*: its client then prints "Summon by operator" (336) and, for `F905`/`F90C`, takes the
    height from the terrain, which `0x020B` with y 0 does not promise. A target with a logout or another move pending is skipped: `F501 op`
    for `F904`/`F905`, the member left out for `F90C`.
13. **Dead players are not summoned**: `F904`/`F905` on a dead target answer `F501 op`; `F90C` leaves dead members where they are. *Why*: the
    new server's moves expect a living player (its body and rebirth state stay with the zone it died in).
14. **`F908` "off" stops the whisper notice** (`dwTo`), not the `F907` one.
15. **`F90A` takes country 0 or 1 only**; anything else → `F501 F90A`.

- **Not deviations, but different underneath**: `F801`'s and `F80E`'s skills end at once with `0x050E` (the original sets their end time to 0
  for the next pass); a repeat's next time is the time it was sent plus the period (no catch-up burst); `F501` for an order comes from the
  target's connection after it acted, and nothing answers when the target left before taking it (as in part 1); `F901`'s `F501 0` and
  sheet both come from the target's connection, in that order; the queue order of deviation 1 is by question id, which is the original's
  order (a returned question is older than every one behind it).
- **Not built**: map 200 (as part 1). `FA19`/`FA1A` are built, with the market (`docs/original/auction.md`
  section 12), and `FA0B`/`FA0C` (part 1 section 8.4, deviations 21 to 24).

**Built** (`docs/plans/done/gm-commands-2.md`; code in `shaiya-proto` `admin.rs`, `crate::admin`, `crate::user_questions`, `zone/admin.rs`,
`server/admin.rs`, migration `0023_user_questions.sql`). Small choices the spec left open:

- `F201`'s `n` is the number of questions waiting **before** the take (the original's count at +0x24 after `RemoveFront`, plus one), so one
  waiting question gives `n = 1`; an `Again` gives the waiting count plus one.
- `F202` kinds outside 1 to 3 are saved as the nearest of them (the table keeps 4 for `F203`); `F203`'s `answer_to` is the stored name of the
  character when it is in the world, else the typed name cut to 20 characters.
- `F709` `/akick` on an account whose character is in the world is an order like `/kick` (the target is told `F709`, staff are protected,
  the answer comes from the target); an account that is online but not in the world is closed with no packet, spared when it is staff of
  the GM's level or higher.
- `F906` with a malformed payload answers `F501 F906`; the other notices ignore a malformed payload, as the part 1 commands do.
  The "off" form (length under 2) stops the repeat of its own kind and answers `F501 0` before the target or the country is looked at.
- A summoned player with a map change under way (`!entered`) is skipped like one with a logout or an item move pending.
- The answer `F501` of a command the GM's own connection finishes is sent at once, while notices and whispers it causes arrive through the
  zone a moment later: the GM may see the `F501` before its own notice.
- `F703` (part 1) took the equipped items too (`Inventory::is_bag` includes bag 0) though its deviation 5 says bags only; fixed with this
  change (`bag != 0`).

## 11. Other episodes

The EP3 and EP5.4 builds of `ps_game` (EP3 2008-01-17; EP5.4 2011-09-14, modified after linking) and `ps_userLog` (EP3 2007-12-07;
EP5.4 2009-09-22) were compared with EP4.5, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and
[`ep5.4.md`](ep5.4.md).

### EP3

`PacketUQ` (EP3 ps_game 0x456fc0) and `PacketAdminUQ` (0x4578a0) are at the same switch positions with the same opcodes, as are the
`PacketAdminCmdC`/`D` groups (confirmed, dispatch tables). Their bodies were not compared.

### EP5.4

The same opcode sets (`0x1201`, `F201` to `F204`, and the `C` and `D` groups) and the same staff levels (confirmed). `SendAdminAll`
(EP5.4 ps_game 0x453c20, matched by call graph) also carries the results of console commands typed in the game to every staff member
([`operator-console.md`](operator-console.md)).

Comparing the builds showed that the question records `0x0401` to `0x0403` sent to `g_pClientToLog` end in `ps_userLog`, which accepts
group `0x04` and writes nothing: its writer `WriteLogGameUQ` is empty (EP4.5 ps_userLog 0x4012d0, 24 bytes), in all three builds
(confirmed). So questions and answers are never stored anywhere, not only lost on a restart.
