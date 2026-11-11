# GM commands 1: staff, characters and the world

This page covers the first half of the operator (GM) commands in the EP4.5 game server `ps_game.exe` (built 2009-07-15): the answer
`F501`, the GM's ordinary chat (`F101` to `F105`, `F10A`), staff levels (`F601`, `F602`), the character, item, monster, NPC and server
commands (`F701` to `F70E`) and the GM's own commands (`FA01` to `FA1A`: invisibility, moves, bans, listening, guild settings). It was
read in the Ghidra decompiler and the disassembly with the PDB names and types, and `ps_dbAgent.exe` for one call. Nothing was executed.

Read: `CUser::PacketAdminChat` 0x470e90, `CUser::PacketAdminCmdA` 0x471d00, `CUser::PacketAdminCmdB` 0x471f40, `CUser::PacketAdminCmdE`
0x474840, `CUser::SendAdminCmdError` 0x4719c0, `CUser::SendAdminCmdSuccess` 0x4719f0, `CUser::SendDBAgentAdminAuthStatus` 0x471950,
`CWorld::FindUser` 0x40d690 and 0x40d640 (by id), `CWorld::FindUserZone` 0x40d760, `CWorld::FindUserZoneMove` 0x40d840,
`CWorld::FindUserBill` 0x40d5c0, `CWorld::EnterUserWorldBill` 0x40d2e0, `CUser::EqualZone` 0x44bd20, `CUser::ItemDelete` 0x45f650,
`CZone::MobGen` 0x41d7b0, `CZone::MobRemove` 0x41e100, `CZone::MobRemoveByID` 0x41e910, `CZone::NpcCreate` 0x41ea50, `CZone::NpcRemove`
0x41ec60, `CUser::LevelChange` 0x48b0e0, `CUser::LevelUp` 0x48ad70, `CUser::UpdateResetPosition` 0x48d470, `CZone::UpdateAllCall`
0x423150, `CZone::UpdateApplyEffect` 0x4220a0, `CGuild::UpdateUserLevelAdminCmd` 0x430f40, `CGuild::SendAllGuildAdminChat` 0x42c3d0,
`CUserQuestionMgr::EnterAdmin` 0x448920 (and its caller `CUser::EnterWorld` 0x44a831), `CUser::GetShape` 0x48a300, `CUser::SendShape`
0x482590, `CUser::GetMoveSpeed` 0x48a4d0, `CUser::EnableTargeting` 0x454150, `CUser::EnableAttackUser` 0x456780,
`CCell::SendCellInfoEnter` 0x423b0d, `CUser::PacketChatE` 0x470490 (its chat-ban and listener check), and every reference to the GM
fields `+0x5774`, `+0x5775`, `+0x5778`, `+0x577c`, `+0x5780`, `+0x5784`, `+0x5818`, `+0x54c8` in the whole binary. From `ps_dbAgent.exe`:
`DBUser::UpdateAuthStatus` 0x42a270 and its query text.

The client side (every command the US client sends, its wire layout, its own level checks and what it does with each answer) is on the
client's [GM opcodes and commands](../client/gm-and-commands.md) page; points taken from it are marked "client". The second half (user
questions, `F8xx`, `F9xx`) is on [GM commands 2](gm-commands-2.md), the console on [The operator console](operator-console.md).
Not covered: the game log records ([The game log](game-log.md)), the second server (map 200).

## In short

- **Staff levels run from 1 (highest) to 10**; 11 is an ordinary player. Each family of commands has its own limit: level 1 for staff
  levels, 2 for characters and the world, 3 for player commands, 4 for summons and notices, 5 for the GM's own commands.
- **A command from a level too low, or one the server has no code for, disconnects the sender.** The US client sends four such commands
  (`/aban`, `/abancan`, `/pvpon`, `/pvpoff`), so typing them disconnects the GM.
- **Staff enter the world invisible and untouchable**, even levels 6 to 10 who cannot switch either off.
- **`/set`** changes a character's level, money, points, stats, grades, experience and kill counts, with a few slips: `/set level`
  gives no stat or skill points, and `/set DEX`, `REC` or `WIS` change the GM's own maximum HP, SP or MP rather than the target's.
- **Chat and move bans live in memory only**: a relog ends them.
- **GM moves are deferred** to the GM's next pass; an unknown map falls back to the GM's current one.
- **Listening to a player's chat** sends copies the stock US client cannot show.

Status words: **confirmed** = read in the code (address given); **inferred** = follows from what was read but is not shown directly;
**unknown** = not read. Names are zero-padded; the server writes a zero at the name's last byte before using it. Sizes include the
2-byte opcode.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names and types), read with the decompiler (REA) and the disassembly (objdump, names from
`docs/original/symbols/ps_game.tsv`): `CUser::PacketAdminChat` 0x470e90, `CUser::PacketAdminCmdA` 0x471d00, `CUser::PacketAdminCmdB` 0x471f40,
`CUser::PacketAdminCmdE` 0x474840, `CUser::SendAdminCmdError` 0x4719c0, `CUser::SendAdminCmdSuccess` 0x4719f0,
`CUser::SendDBAgentAdminAuthStatus` 0x471950, `CWorld::FindUser` 0x40d690 and 0x40d640 (by id), `CWorld::FindUserZone` 0x40d760,
`CWorld::FindUserZoneMove` 0x40d840, `CWorld::FindUserBill` 0x40d5c0, `CWorld::EnterUserWorldBill` 0x40d2e0, `CUser::EqualZone` 0x44bd20,
`CUser::ItemDelete` 0x45f650, `CZone::MobGen` 0x41d7b0, `CZone::MobRemove` 0x41e100, `CZone::MobRemoveByID` 0x41e910, `CZone::NpcCreate`
0x41ea50, `CZone::NpcRemove` 0x41ec60, `CUser::LevelChange` 0x48b0e0, `CUser::LevelUp` 0x48ad70, `CUser::UpdateResetPosition` 0x48d470,
`CZone::UpdateAllCall` 0x423150, `CZone::UpdateApplyEffect` 0x4220a0, `CGuild::UpdateUserLevelAdminCmd` 0x430f40,
`CGuild::SendAllGuildAdminChat` 0x42c3d0, `CUserQuestionMgr::EnterAdmin` 0x448920 (and its caller `CUser::EnterWorld` 0x44a831),
`CUser::GetShape` 0x48a300, `CUser::SendShape` 0x482590, `CUser::GetMoveSpeed` 0x48a4d0, `CUser::EnableTargeting` 0x454150,
`CUser::EnableAttackUser` 0x456780, `CCell::SendCellInfoEnter` 0x423b0d, `CUser::PacketChatE` 0x470490 (its chat-ban and listener check), and
every reference to the GM fields `+0x5774`, `+0x5775`, `+0x5778`, `+0x577c`, `+0x5780`, `+0x5784`, `+0x5818`, `+0x54c8` in the whole binary.
From `ps_dbAgent.exe` (same pack): `DBUser::UpdateAuthStatus` 0x42a270 and its query text. Nothing was executed. The client side is
`docs/client/gm-and-commands.md` (every command the US client sends, its wire layout, its own level checks and what it does with each answer;
its findings are used here and marked "client"); the client itself was not read for this. Cross-checked against
`docs/original/community-guide-notes.md` section 6 (the community's command table by `Status` 16 to 80, from a community file collection's `GMCodes.pdf`:
its tiers are the client's own level gates, and its "instant DC" for `/pvpon` is the original's missing case), `docs/original/client-login.md` /
`docs/original/ps_login.md` sections 12 and 15 (`Status` to privilege) and the EP6 rewrite's `AdminInfo` (shaiya-episode-6, github.com/kurtekat/shaiya-episode-6,
`sdev/include/shaiya/AdminInfo.h`: visible, attackable, move and chat ban times, listen ids and notice, in the same order as `+0x5774` to
`+0x5788` here, 0xA0 later in that build; its `FA0D`/`FA0F` call our minutes field "unknown"). No disagreement was found.

**Part 1** is what a staff member needs to work in the world: the answer `F501`, the GM's ordinary chat (`F101` to `F105`, `F10A`), staff
levels (`F601`, `F602`), the character, item, monster, NPC and server commands (`F701` to `F70E`) and the GM's own commands (`FA01` to `FA1A`:
invisibility, moves, bans, guild settings). **Part 2** (`docs/original/gm-commands-2.md`): the user questions (`F2xx`, `0x1201`),
the player commands of `PacketAdminCmdC` (`F8xx`: cure, clear bags, kick, quests) and `PacketAdminCmdD` (`F9xx`: character sheet, summons,
notices). **Not covered**: the game log records (`CUser::SendLogAdmin` and `SendChatLogA/B` write to the game log server, which the new
servers do not have), the auction commands' market side (`FA19`, `FA1A`: `docs/original/auction.md` section 12), the second
server (map 200). **Also built here** (section 13, deviations 14 to 18): the ten commands the US client sends but the original has no case
for, so it disconnects the GM (`F70A`, `F70B`, `F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`, `FA16`, `FA17`); their behaviour is the new
server's, designed from the client's texts.

Status words as in `docs/original/ps_game.md`: **verified** = read in the code (address given); **inferred** = follows from verified code
but is not shown directly; **unknown** = not read.

## 1. What it is

An account's staff level (1 highest to 10; 11 is an ordinary player) comes with the login. A staff member's client sends commands typed in
the chat box as `Fxxx` packets; the server checks the level for the whole family of the opcode, carries the command out and answers most of
them with `F501` (0 for done, else the opcode that failed). Staff members enter the world **invisible** and **untouchable** and can switch
both. Their ordinary chat travels as `F1xx` instead of `0x11xx`.

## 2. Who may send what

Confirmed.

- **The level** is `CUser +0x5768` (`u8`), set at login from the login server's answer (`CWorld::LoginUser` 0x40c699); 11 for players,
  1 to 10 for staff (`CUser::IsAdmin` 0x413d40: `<= 10`). The new login server already sends it (`accounts.status` bits 4 to 6, 11 for
  players; `docs/original/client-login.md`), and the game server keeps it as `Player::privilege`.
- **Family limits**: `F1xx` (`PacketAdminChat`) and `F2xx` need level ≤ 10, `F6xx` (`PacketAdminCmdA`) ≤ 1, `F7xx` (`B`) ≤ 2, `F8xx` (`C`)
  ≤ 3, `F9xx` (`D`) ≤ 4, `FAxx` (`E`) ≤ 5. A request from a higher level **closes the connection** (`SConnection::Close(9, 0)`); so does an
  opcode the family's handler has no case for: `F70A`, `F70B` (B's jump table), `FA03`, `FA04`, `FA16`, `FA17` (E's), any `F1xx` but
  `F101` to `F10A`, and any other `Fxxx` family. The US client sends `F70A` `/aban`, `F70B` `/abancan`, `FA16` `/pvpon` and `FA17`
  `/pvpoff` (client), so typing them disconnects the GM.
- The client checks the level itself before sending (`privilege <= N` per command, client section 2a); the server's checks are the family
  limits only. `FA13`, `FA14`, `FA15`, `FA19`, `FA1A` are level 4 in the client, 5 on the server.

## 3. The answer `F501`

Confirmed.

`F501 u16 value` (4 bytes): 0 = done (`SendAdminCmdSuccess`), otherwise the failing opcode (`SendAdminCmdError`, value in `AX`). The client
prints the confirmation it queued when the command was sent on 0, or a failure text chosen by the value (client section 1). Which commands
answer, and when, is given with each command below; some answer nothing (marked).

## 4. The GM's state

Confirmed.

| field | meaning | default | staff on entering the world |
|---|---|---|---|
| `+0x5774` | visible (0 = invisible) | 1 (`CUser::CUser` 0x449dde, `ResetCharacter` 0x44a295) | **0** |
| `+0x5775` | attackable (0 = untouchable) | 1 | **0** |
| `+0x577c` | chat ban until (tick; `0xFFFFFFFF` for ever; 0 none) | 0 | — |
| `+0x5778` | move ban until (same) | 0 | — |
| `+0x5780` | the GM listening to this player's chat (character id) | 0 | — |
| `+0x5784` | the player this GM listens to | 0 | — |
| `+0x54c8` | the zone a staff member's next map change goes into (party or guild id; −1 none) | 0 | — |

- **Entering**: `CUser::EnterWorld` (0x44a831) calls `CUserQuestionMgr::EnterAdmin` for level ≤ 10: `+0x5774 = 0`, `+0x5775 = 0`, a notice
  slot (part 2); a full list (256 staff online) closes the connection.
- **Invisible**: `CUser::GetShape` gives **shape 13** while `+0x5774` is 0; `CUser::SendShape` sends `0x051D u32 id, u8 shape` (7) to the zone's
  view (`0x0524` instead for shape 102 while visible). The player still enters everyone's view with `0x0201` (whose kind byte is **2 for any
  staff member**, `CCell::SendCellInfoEnter` 0x423b0d; the new server already does this) and then `0x051D` with 13 (0x423b80); the client
  is expected to hide shape 13 (client). `CZone::EnterUser` sends the shape on every map change when it is not 0.
- **Untouchable or invisible**: `CUser::EnableTargeting` (alive, `+0x1366` 0, `+0x5775` and `+0x5774` both set) and `CUser::EnableAttackUser`
  (both set, or oneself) decide whether a player may be targeted. The same two bytes are tested by every monster target search (`CMob::FindTarget`,
  `FindHelp*`, `FindAttackUserMaxHP`/`MinHP`, `AttackAI`, the skill range selections, the Lua and FSM searches), the guards (`CNpc::FindTarget`),
  the players' skill ranges (`CUser::SkillAttackRange*`, `SkillDebuffRange*`, `EnableAttackSkillRange`), `ChkProtect` (fire thorns) and
  `ChkFindHide` (detection) (xrefs of `+0x5774`/`+0x5775`). So an invisible or untouchable staff member is never a target of anything.
- **Chat ban**: `PacketChatE` (0x4704bc) and `PacketAdminChat` drop the whole chat packet, silently, while `+0x577c` is in the future, and
  clear it once past. **Move ban**: `CUser::GetMoveSpeed` gives 255 (cannot move) while `+0x5778` is set. `CZone::UpdateApplyEffect` (each
  player's pass) ends both when their time has passed: chat → `+0x577c = 0` and `FA0E` (2 bytes) to the player; move → `+0x5778 = 0`,
  `CUser::SendSpeed` (`0x051C`) and `FA10` (2 bytes). `0xFFFFFFFF` never passes. Both are **in memory only**: `CUser::ResetCharacter`
  clears them, so a relog ends a ban.
- **Listening** (`FA0B`): the player's `PacketChatE`/`PacketAdminChat` copy each line to the listener as `F502` when the listener's level is ≤ 5
  (else the link is dropped). The US client has no `F502` handler; the companion DLL `fixes.dll` adds one (section 8.4).

## 5. GM chat: `F101` to `F10A`

Confirmed, `PacketAdminChat` 0x470e90.

A staff client sends its ordinary chat with the `F1xx` opcode of the same kind (client: the send functions add `0xE000`). Level ≥ 11 closes
the connection (logged "Error AdminChat"). The chat ban applies first (section 4). A length byte above 128 **closes the connection**, below 2
the line is ignored. Each line goes to the listener (section 4) and the game log.

| request | wire after the opcode | what happens |
|---|---|---|
| `F101` normal | `u8 len, text` | only in the world (`+0x5758` 4); `F101 u32 char id, u8 len, text` (len + 7) to the view (`CObject::SendViewChat`, as `0x1101`). With a Message To Server item armed (`+0x5840`, special 38; `docs/original/small-items.md` section 5) it goes out as `0x1108` to everyone instead |
| `F102` whisper | `char name[21], u8 len, text` (name ends at +0x16) | the target must be in the world (`FindUserZone`) and of the **same country** (`+0x12d`): to it `F102 u8 0, sender name[21], u8 len, text` (len + 25), to the sender `F102 u8 1, target name[21], u8 len, text`; otherwise `0x1106 01` (3) to the sender |
| `F103` trade | `u8 len, text` | `F103 sender name[21], u8 len, text` to **everyone on the server, both countries** (`CWorld::SendAll`; the player's `0x1103` goes to its country only) |
| `F104` guild | `u8 len, text` | in a guild: `F104 sender name[21], u8 len, text` to every online member (`CGuild::SendAllGuildAdminChat`, layout by the disassembly 0x42c400…0x42c440) |
| `F105` party | `u8 len, text` | in a party: `F105 u32 char id, u8 len, text` to the party (`CParty::Send`) |
| `F10A` union notice | `u8 len, text` | only the leader or sub-leader of a union (`+0x114` set, slot `+0xc` or `+0x110`): `F10A u32 char id, u8 len, text` to the union, as the player's `0x1112` (already built as `chat::OP_UNION_NOTICE`) |
| `F106`, `F107`, `F108`, `F109` | — | a world notice and a one-to-one "GM talk"; the client never sends them (client) |

## 6. Staff levels: `F601`, `F602`

Confirmed, `PacketAdminCmdA` 0x471d00; level ≤ 1.

- **`F601`** `u8 level, char name[21]` (24 bytes; the name from +3, ended at +0x17). The client sends **`name[21], u8 level`** instead
  (client: a layout mismatch), so the original reads the level from the name's first byte and the name from its second. Target by
  `CWorld::FindUser` (any state). Not found → `F501 F601`. Found: its level `+0x5768` = the byte, **with no range check**; ps_dbAgent gets
  `0x0305 u16 status, char account[32]` (36; status = level × 16 for levels 1 to 5, else 0; `SendDBAgentAdminAuthStatus` 0x471950), and
  ps_dbAgent runs **`EXEC usp_Save_User_Status_E '<account>', <status>`** (`DBUser::UpdateAuthStatus`, ps_dbAgent 0x42a2cd); the target gets
  `F601 u8 level` (3), the GM `F501 0`. The client sets its own level from `F601` but its staff flag only at the next login (client).
- **`F602`** `char name[21]`. Same lookup (`F501 F602` when not found); level 11, `0x0305 (0, account)` to ps_dbAgent (the account becomes a
  player's, status 0), `F602 u8 11` to the target, `F501 0`.
- The account name is `CUser +0x5794` (verified: `CWorld::EnterUserWorldBill` keys the account map by it, and `0x0305` carries it).
- The new level counts at once on the server: a demoted GM loses the commands, a promoted player gains them (but its client does not send
  them before its next login, client). Since audit R26 that includes the in-game console (a typed dot line) and the staff packets (the
  questions of players): the connection's staff flag and level, and the occupant's flag in the zone, follow the new level, and a question a
  demoted GM held goes back to the queue.

## 7. Characters, items, monsters, NPCs, the server: `F701` to `F70E`

Confirmed, `PacketAdminCmdB` 0x471f40; level ≤ 2.

### 7.1 `F701` `/set`: `u8 stat, u32 value, char name[21]` (28; name ends at +0x1b)

The target is any player **in the world** (`CWorld::FindUserZone`: by name, state 4; the GM's zone does not matter). Not found or a stat
above 18 → `F501 F701`. A refused value → `F501 F701`. Otherwise the change, its packets, the database, then **`F701 u8 stat, u32 value`
(7) to the target** and `F501 0` to the GM (0x47296d). The value in the target's `F701` is the stored one.

| stat | limit (else refused) | what changes, besides the stored value | ps_dbAgent |
|---|---|---|---|
| 0 grow | ≤ 3; 0 (easy) only up to level 30 | `CUser::LevelUp(target)` (the level-up announcements, below) | `SendDBGrow` |
| 1 level | ≤ 60; ≤ 30 in easy mode; not the current level | exp = `GetExp(grow, level − 1)` (the level's start, confirmed: `CGameData::GetExp` 0x458180 is the table entry `grow × 61 + level`, called with `level − 1` at 0x47208a; the stat-16 case below takes `GetExp(grow, n)` as the end of level `n`, 0x4721fd), `F701 16 exp` to the target, `LevelChange` (the base maxima of the new level by job and mode; **no stat or skill points given or taken**, 0x48b0e0), `LevelUp`, then `F701` with the job's main stat and its value — **the GM's job** (`+0x134` of the GM: 0 STR(5), 1 REC(7), 2 DEX(6), 3 LUC(9), 4 INT(8), 5 WIS(10)) | `SendDBLevel`, `SendDBExp` |
| 2 money | none | — | `SendDBMoney` |
| 3 stat points, 4 skill points | ≤ 0x6FFF | — | `SendDBStatPoint` / `SkillPoint` |
| 5 STR, 6 DEX, 7 REC, 8 INT, 9 LUC, 10 WIS | ≤ 0x6FFF and not the current value | the base stat and the total change by the difference; `SetAttack`. DEX, REC, WIS also add 5 × the difference to max SP, HP, MP and send them with a fill (`SendMaxSP`/`HP`/`MP`, `SendRecoverAdd`) — **to the GM**, not the target (0x47244f: `esi` is reloaded with the GM) | `0x0610 u32 char, u8 which (1 STR, 2 DEX, 3 INT, 4 WIS, 5 REC, 6 LUC), u16 value` |
| 11 HG (honor) | ≤ 0xFFFF | — | `0x0605 u32 char, u8 1, u16` |
| 12 VG | ≤ 0x6FFF | — | `0x0605 u32 char, u8 2, u16` |
| 13 CG, 14 OG, 15 IG | ≤ 256, **stored in a byte** (256 becomes 0) | — | `0x0605 u32 char, u8 2, u16` (the same kind 2 for all four) |
| 16 exp | grow 0: ≤ the easy cap; else ≤ the mode's cap | the level becomes the first `n` (1 to 59) with `value < GetExp(grow, n)`; **when there is none the level becomes the grow number** (0x4721f2: the loop's counter starts as the grow); a changed level: `SendDBLevel`, `LevelChange`, `LevelUp`, `F701 1 level` | `SendDBExp` |
| 17 kills | clamped to the top kill-count table value | `0x0522 u32 id, u32 kills` to the view (as a kill) | `0x060C u32 char, u8 0, u32` |
| 18 deaths | clamped likewise | — | `0x060C u32 char, u8 1, u32` |

`CUser::LevelUp(target, 0, 0)` (0x48ad70) is the ordinary level-up's announcement: `SetAttack`, the character saved (`0x0601`), the level-up
packet to everyone within 60 in the form for "not by the player's own kill" (`0x051E`; `0x0508` is the own-kill form), the party's member
level (`0x0C09`/`0x0C13`), the maxima (`SendMaxAll`), a full refill (`SendRecoverAdd` with the maxima), `0x0308` and the guild's member level
(`0x0D0C (level, id)`). The new server's `award_exp` does the same when levels > 0.

### 7.2 Items: `F702`, `F703`

- **`F702`** `/imake`, `/getitem`: `u8 type, u8 type id, u8 count`. An unknown item → **no answer** at all. Otherwise
  `CUser::ItemCreate(item, count)` **into the GM's own bags** (`docs/original/item-origins.md` section 5.3: count 1 to 1000, one new item of
  `min(count, stack)` per empty slot of bags 1 and up, make type `'S'`, craft stats as for boxes, `0x0205` for each); success → `F501 0`, else
  `F501 F702` (no empty slot, or count 0; the items already placed stay).
- **`F703`** `/recallitem`: `char name[21], u8 type, u8 type id` (type at +0x17). The target: in the world with no map change pending
  (`FindUserZoneMove`), **in the GM's zone and map** (`EqualZone`); the item must exist. `CUser::ItemDelete` removes **every** item of that
  type and type id from **bags 0 (the equipment) to the last bag**, each through `CUser::ItemRemove` (the item gone with its packet). It
  always succeeds once the checks pass (also when nothing was found): `F501 0`; a failed check → `F501 F703`.

### 7.3 Monsters: `F704`, `F705`, `F70E`

All need the GM in a zone.
- **`F704`** `/mmake`: `u16 mob, u8 count` (count above 20 becomes 20). `CZone::MobGen` at the GM's position: up to `count × 10` tries; each
  places one monster at the GM's `x + (rand % 20 − 10)`, `z + (rand % 20 − 10)` and the GM's height, or at the GM's own spot when that place
  is off the map or blocked, or tries once more when the GM's spot is too. The monsters belong to no spawn entry and **never respawn**.
  Success when at least one was made (`F501 0`); an unknown monster or none made → `F501 F704`. (The new server's `LuaCreateMob` is the
  same function's twin, `CZone::MobGenEx`: `zone/lua.rs` `mob_gen_ex`.)
- **`F705`** `/mera`: `u16 mob, u8 count`. In the GM's cell and the 8 around it, up to `count` living monsters of that number **within 10**
  (x and z) of the GM are removed: `0x0602` to the view, and their spawn entry counts one less and starts its respawn time as for a death
  (`CZone::MobRemove`). No experience, no drops. `F501 0` even when none was near; an unknown monster → `F501 F705`.
- **`F70E`** `/mera t`: `u32 object id`. That monster of the GM's zone, removed the same way (`MobRemoveByID`); none → `F501 F70E`.
- **On an obelisk or a boss** (`docs/original/obelisks.md` sections 11.5 and 11.3): both commands make an obelisk fall quietly, `/mera`
  to its current country and `/mera t` to its base (its guards removed, `0x2102` to the map, the owner saved when it changed, no kill
  count, no `0x2103`); it rises again after its monster's respawn time (3 minutes) for that owner. A boss's chain moves on as for a
  death (`BothMobDestroy`: its followers go, the next stage; after the last the portal opens and the chests come), but the next boss is
  due after the removed one's own respawn time only. Built (an early version never raised the obelisk again).

### 7.4 NPCs: `F70C`, `F70D`

- **`F70C`** `/nmake`: `u8 type, u16 type id, u8 count` (count unused). An NPC of that kind (`CNpcData::GetNpc`) at the GM's x and z and its
  **height − 0.9**, entered into the zone (`0x0E01` to the view). It is also written into the map's NPC list (at most 64), so instance zones
  of that map made later have it too (`CZone::NpcCreate`). Never saved. Unknown NPC → `F501 F70C`.
- **`F70D`** `/nera`: `u8 type, u16 type id, u8 count`. Up to `count` NPCs of that kind in the 3 × 3 cells around the GM **within 10** are
  removed (`0x0E02` to the view), shipped ones too, until the server restarts. `F501 0` even when none was near; unknown kind → `F501 F70D`.

### 7.5 The server: `F706`, `F707`, `F708`, `F709`

- **`F706`** `/server on`, **`F707`** `/server off`: `SAcceptor::StartAcception` / `StopAcception` (new connections accepted or not), and
  `0x0204 u8 4, u8 server id, u8 closed` (5) to the session manager (`g_pClientToMgr`): the last byte is **1 after `/server off` and 0 after
  `/server on`** (it is set when the acceptor's flag is 0, `sete` at 0x472f19 and 0x472f81). The session server stores it for that server
  (`CServerList::GameStatus`, ps_session 0x408740, keyed by the link's own server id, not the packet's), puts its own id in byte 3 and sends
  the 5 bytes to every login server and admin link (ps_session 0x40325b to 0x403290); the login server writes the byte as the server's
  status in its list (`CServerList::SetStatus`, ps_login 0x407280), the status the `0xA201` list shows (`SendList` 0x407328) (confirmed).
  The `4` is read by nobody. `F501 0`.
- **`F708`** `/shutdown`: `F501 F708` only.
- **`F709`** `/akick`: `char name[21]` = an **account** name (`CWorld::FindUserBill`, keyed by `+0x5794`). Found → that connection is closed
  (`Close(9)`, no packet) and `F501 0`; else `F501 F709`.

## 8. The GM's own commands: `FA01` to `FA1A`

Confirmed, `PacketAdminCmdE` 0x474840; level ≤ 5.

### 8.1 Visibility and protection

- **`FA01`** `/char on` (투명설정, "set invisible"): `+0x5774 = 0`, `SendShape` (13 to the view), `F501 0`. **`FA02`** `/char off`: `+0x5774 = 1`,
  `SendShape` (the real shape), `F501 0`.
- **`FA05`** `/attack on` ("won't be attacked"): `+0x5775 = 0`, `F501 0`. **`FA06`** `/attack off`: `+0x5775 = 1`, `F501 0`. Nothing is sent
  to anyone else.

### 8.2 Messages and finding players

- **`FA07`** `/warning`: `char name[21], u8 len, text` (len at +0x17, 2 to 128, else `F501 FA07`). Target in the world (`FindUserZone`):
  `FA07 u8 len, text` (len + 3) to it, `F501 0`; not found → `F501 FA07`.
- **`FA08`** `/watch`: `char name[21]`. Target in the world: `F501 0` **first**, then `FA08 u16 map, f32 x, y, z` (16) to the GM; not found
  → `F501 FA08`.

### 8.3 Moves: `FA09`, `FA0A`, `FA11`, `FA12`

Each move is **deferred**: when the GM has no map change pending (`+0x57dc` 0) the command stores kind = its opcode, the time (now), the map
id and x, y, z in `+0x5818` to `+0x582c`, and `F501 0` goes out at once (also when nothing was stored). `CUser::UpdateResetPosition`
(0x48d470), on the GM's next pass: ends a personal shop (`stMyShop::Ended`), then for these kinds
- **another map**: `CWorld::ZoneLeaveUserMove(map, x, y, z)` and **`op u32 id, u16 map, f32 x, y, z`** (20, the command's own opcode) to the GM;
  the client loads the map as for `0x020B` (client: own id = map change);
- **the same map**: `CZone::MoveUser`; when it worked, `0x020B` (id, map, x, y, z) to the view, then the same 20-byte `op` packet to the GM.

`CWorld::GetMap(map)` gives the map; **an unknown map falls back to the GM's current map** (`FA0A`, `FA11`, `FA12`). x and z must be in
`[0, map size)` where checked. Map **200** (another server, `FA0A`, `FA11`) goes through ps_dbAgent (`0x080C u32 char`) or answers `0x0110 02`
when the new-server check is on; not covered.

| command | wire | target spot | checks (else `F501 op`) | zone choice `+0x54c8` |
|---|---|---|---|---|
| `FA09` `/amove` | `char name[21]` | the player's map and x, y, z | player in the world; **its map has no `CreateType`** (a player in an instance, house or ranking zone cannot be reached); its x, z in bounds | — |
| `FA0A` `/bmove` | `f32 x, f32 z, u16 map` | x, **y 0**, z | x, z in bounds | −1 when the map has a `CreateType` |
| `FA11` `/cmove` | `u16 map` (`0xFFFF` = current) | the map's bind point for the GM's country (`CMap::GetBindPos`) | none (no `F501` error path) | −1 when the map has a `CreateType` |
| `FA12` `/gmove` | `f32 x, f32 z, u16 map, char name[25]` (37) | x, y 0, z | x, z in bounds; then by `CreateType`: **3 (party maps)**: `name` is a *character*, online, in a party → its party; **5 (ranking map)**: `name` a guild (`CGuildMgr::FindGuild`) and the map's battle open (`CMap::IsGuildRankMapOpenTime`) → that guild; **6 (house maps)**: a guild with ranking < 31, a house bought (`+0x55`) and in use (`+0x54` 1) → that guild; **any other type (0, 1, 2, 4) → `F501 FA12`** | the party's or guild's id |

`+0x54c8` is read for staff (level ≤ 10) by `CWorld::GetZoneByMapID` (0x413df0) on maps of `CreateType` **3, 5 and 6**: when it is not 0
it is the zone's key in place of the party's or guild's id (0x413f0e, 0x4140ed, 0x4141fc), so the GM joins that instance zone, skipping the
ranking map's open-time check and the house check; a key with no zone yet gets a new, empty zone of that key. Type 4 has no staff path.
**−1 is not 0**, so a GM sent there by `FA0A`/`FA11` gets the zone keyed −1 (one zone shared by such GMs), not the usual pick.
`CUser::ZoneEnterUserMove` clears `+0x54c8` to 0 once the GM is in a zone of those types (0x44b91e, 0x44b937) (confirmed).

### 8.4 Listening: `FA0B`, `FA0C`

`FA0B` `/cwatch`: `char name[21]`. The GM's previous link is dropped; the player found by `FindUser` (any state) gets `+0x5780 = GM id`, the
GM `+0x5784 = player id`; `F501 0` (not found → `F501 FA0B`). `FA0C`: drops the link (`F501 0`), none → `F501 FA0C`. The stock US client
cannot show the copies; the companion DLL `fixes.dll` (`listen_chat=1`) shows them as `[Listen] Name: [a,b]text` in the GM chat category 0x10
(`docs/client/operator-listen.md`).

**The copy** (verified, `CUser::SendListenChat` 0x4702f0, mangled `SendListenChat(CUser *, char *, int, char *, int, char *, int)`:
the speaker, then three text/length pairs; sent to `this`, the listener). `F502 char name[21]` (the speaker's name, `+0x188`, copied with its
zero), `u8 len`, `text[len]`; 24 + `len` bytes with the opcode. With a first text of length > 0 the text is `[` first `]`, or `[` first `,`
second `]` when the second's length is > 0, then the line exactly as the speaker sent it (its length byte's count, so with the client's
zero). `len` is the prefix plus the line (at most 25 + 128 bytes, no overflow). The call sites, each right after the line was relayed and
beside its chat log call (`SendChatLogA`/`B`), so a refused line (no guild, no party, a whisper not delivered, the shout cooldown, a chat
ban) is never copied:

| line (`PacketChatE` 0x470490 / `PacketAdminChat` 0x470e90) | first text (length) | second |
|---|---|---|
| `0x1101` / `F101` | `NORMAL` (6) | — |
| `0x1101` / `F101` with a Message To Server armed, `0x1108` | `TRADE` (5) | — |
| `0x1102` / `F102` whisper; `F108` (GM talk, never sent by this client) | `TO` (2, string at 0x538338) | the target's name (its `strlen`) |
| `0x1103` / `F103` | `ALL` (3, 0x538334) | — |
| `0x1104` / `F104` (in a guild) | `GUILD` (5) | — |
| `0x1105` / `F105` (in a party), `0x1112` / `F10A` (a union's leaders) | `PARTY` (5) | — |
| `0x1107` | `SHOUT` (5) | — |
| `0x1111` | `ZONE` with **length 5** (0x538314, the zero counted) | — |
| `F106` (world notice, never sent by this client) | `NOTICE` (6) | — |

So `a` is the kind of line and `b` a whisper's target, both plain text. The listener is found by character id (`0x40d640`) at each line; when
it is not online nothing is sent and the link stays. Only the watched player's own lines are copied, never what it receives.

### 8.5 Bans: `FA0D` to `FA10`

| command | wire | effect | to the target |
|---|---|---|---|
| `FA0D` `/silence` | `u16 minutes, char name[21]` (name ends at +0x18) | chat ban: `+0x577c` = for ever when minutes is 0, else now + minutes × 60 000 ms | `FA0D` (2) |
| `FA0E` `/silence off` | `char name[21]` | `+0x577c = 0` | `FA0E` |
| `FA0F` `/stopon` | `u16 minutes, char name[21]` | move ban `+0x5778`, as above; `SendSpeed` (move speed 255) | `FA0F` |
| `FA10` `/stopoff` | `char name[21]` | `+0x5778 = 0`, `SendSpeed` | `FA10` |

The target is found by `FindUser` (any state); not found → `F501 op`; done → `F501 0`. The client shows 332 to 335 for `FA0D` to `FA10`
(client). The bans end by themselves as in section 4.

### 8.6 Guilds: `FA13`, `FA14`, `FA15`

- **`FA13`** `/guildgrade`: `char guild[25], char name[21], u8 grade` (49; name at +0x1b, grade at +0x30), grade 1 to 9.
  `CGuild::UpdateUserLevelAdminCmd`: the member by name (online through `FindUser`, else the guild's list); a member with rank 0, or already
  that rank → failure. An online member promoted into ranks 1 and 2 while more than 6 hold them → failure; the officer slots (7) follow. The
  rank is set (also on the online player, `+0x174c`), ps_dbAgent gets `0x0B09 (guild, char, rank)` (or `SendDBGuildUserUpdate` offline),
  **grade 1 copies the member's name as the guild's master name while the old master keeps rank 1** (two masters), and every online member
  gets `0x0D0C (200 + rank, char id)`. **Success sends no `F501`**; a failure (no guild, grade out of range, member refused) → `F501 FA13`.
- **`FA14`** `/guildrank`: `char guild[25], u8 ranking` (at +0x1b), 1 to 31: the guild's ranking `+0x48` = it and **`byUseHouse` `+0x54` = 1**,
  in memory only (no database call, nothing sent). Success: **no answer**; failure → `F501 FA14`.
- **`FA15`** `/guildpoint`: `char guild[25], u32 points` (at +0x1b): the guild's points `+0x44` = it, ps_dbAgent `0x0B0B u32 guild, u32 points`
  (10). **Never `F501 0`**; no such guild → `F501 FA15`.

### 8.7 `FA18` `/xcall`

Confirmed, `CZone::UpdateAllCall` 0x423150.

`u8 country` (0 Light, 1 Fury: the client sends its argument − 1). Every player of the GM's **zone** with level ≥ 11 (not staff), not the GM, of
that country and **farther than 10** (x and z) gets a deferred move (kind 1, no pending map change) to the GM's map and x, y, z at a random
time 1 to 11 s from now (`1000 + rand % 10000` ms). **No `F501`**.

### 8.8 `FA19`, `FA1A` (the market)

Only on a market map (`+0x1e0`); `FA19` `/auctionrecall`: `char name[21], u32 market id`; `FA1A` `/auctionsearch`: `char name[21]`
(`CMarketMgr::RetrievalItem`, `SearchCharName`). Both are `docs/original/auction.md` section 12 (built, with its deviation 3).

## 9. The packets

Confirmed.

| op | dir | size | layout | section |
|---|---|---|---|---|
| `F501` | S → C | 4 | `u16 value` (0 done, else the failing opcode) | 3 |
| `F101` | C → S / S → C | var | `u8 len, text` / `u32 char id, u8 len, text` | 5 |
| `F102` | both | var | `name[21], u8 len, text` / `u8 to_sender, name[21], u8 len, text` | 5 |
| `F103`, `F104` | both | var | `u8 len, text` / `name[21], u8 len, text` | 5 |
| `F105`, `F10A` | both | var | `u8 len, text` / `u32 char id, u8 len, text` | 5 |
| `F601` | both | 24 / 3 | `u8 level, name[21]` (what the original reads) / `u8 level` | 6 |
| `F602` | both | 23 / 3 | `name[21]` / `u8 11` | 6 |
| `F701` | both | 28 / 7 | `u8 stat, u32 value, name[21]` / `u8 stat, u32 value` | 7.1 |
| `F702` | C → S | 5 | `u8 type, u8 type id, u8 count` | 7.2 |
| `F703` | C → S | 25 | `name[21], u8 type, u8 type id` | 7.2 |
| `F704`, `F705` | C → S | 5 | `u16 mob, u8 count` | 7.3 |
| `F70E` | C → S | 6 | `u32 object id` | 7.3 |
| `F70C`, `F70D` | C → S | 6 | `u8 type, u16 type id, u8 count` | 7.4 |
| `F706`, `F707`, `F708` | C → S | 2 | — | 7.5 |
| `F709` | C → S | 23 | `name[21]` (an account) | 7.5 |
| `FA01`, `FA02`, `FA05`, `FA06`, `FA0C` | C → S | 2 | — | 8 |
| `FA07` | both | var | `name[21], u8 len, text` / `u8 len, text` | 8.2 |
| `FA08` | both | 23 / 16 | `name[21]` / `u16 map, f32 x, y, z` | 8.2 |
| `FA09` | both | 23 / 20 | `name[21]` / `u32 id, u16 map, f32 x, y, z` | 8.3 |
| `FA0A` | both | 12 / 20 | `f32 x, f32 z, u16 map` / as `FA09` | 8.3 |
| `FA11` | both | 4 / 20 | `u16 map` / as `FA09` | 8.3 |
| `FA12` | both | 37 / 20 | `f32 x, f32 z, u16 map, name[25]` / as `FA09` | 8.3 |
| `FA0B` | C → S | 23 | `name[21]` | 8.4 |
| `FA0D`, `FA0F` | C → S / S → C | 25 / 2 | `u16 minutes, name[21]` / — | 8.5 |
| `FA0E`, `FA10` | C → S / S → C | 23 / 2 | `name[21]` / — | 8.5 |
| `FA13` | C → S | 49 | `guild[25], name[21], u8 grade` | 8.6 |
| `FA14` | C → S | 28 | `guild[25], u8 ranking` | 8.6 |
| `FA15` | C → S | 31 | `guild[25], u32 points` | 8.6 |
| `FA18` | C → S | 3 | `u8 country` | 8.7 |
| `0x051D` | S → C | 7 | `u32 id, u8 shape` (13 = invisible) | 4 |
| `0x051C` | S → C | 7 | `u32 id, u8 attack speed, u8 move speed` (255 = cannot) | 4 |
| `0x020B` | S → C | 20 | `u32 id, u16 map, f32 x, y, z` | 8.3 |

Names are zero-padded; the server writes a zero at the name's last byte before using it. Sizes include the 2-byte opcode.

## 10. The database calls

Confirmed, in ps_game; ps_dbAgent's side for `0x0305` and the `0x06xx` messages (`CUser::PacketUserSetStatus` 0x405b00).

| op | from | to ps_dbAgent | ps_dbAgent |
|---|---|---|---|
| `0x0305` | `F601`, `F602` | `u16 status, char account[32]` | `EXEC usp_Save_User_Status_E '<account>', <status>` (0x42a2cd) |
| `0x0601` | `LevelUp` | the character's level, exp, points, stats (34) | saves them |
| `SendDBGrow` `0x060D`, `SendDBLevel` `0x060A`, `SendDBExp` `0x0602`, `SendDBMoney` `0x0603`, `SendDBStatPoint` `0x060E`, `SendDBSkillPoint` `0x060F` | `F701` | `u32 char, u32 value` (10) | the agent's copy of the value (`CUser::PacketUserSetStatus` 0x405b00), saved with the character by `usp_Save_Char_Info_E` (`DBCharacter::SaveCharacter` 0x42337c): `Level`, `Exp`, `Money`, `StatPoint`, `SkillPoint`. **The grow mode is not a column of that save**: it only feeds `SaveMaxGrow` (0x4200c0, the account's `MaxGrow`), so a `/set` grow is lost at the next login (confirmed) |
| `0x0610` | `F701` 5 to 10 | `u32 char, u8 which, u16 value` | the base stat (confirmed: ps_dbAgent table 0x4060e0, which 1 to 6 → `Str`, `Dex`, `Int`, `Wis`, `Rec`, `Luc` of the save) |
| `0x0605` | `F701` 11 to 15 | `u32 char, u8 kind (1 HG, else 2), u16 value` | the agent reads kind 1 to 5 as `Hg`, `Vg`, `Cg`, `Og`, `Ig` (ps_dbAgent 0x405c1a, table 0x4060cc), but `ps_game` sends 2 for all four of VG, CG, OG and IG (0x4727a8, 0x472804, 0x47285a), so **a `/set` of CG, OG or IG is saved as the character's VG** and the CG, OG or IG change itself is not saved (confirmed) |
| `0x060C` | `F701` 17, 18 | `u32 char, u8 0 kills / 1 deaths, u32` | `K1` (0) and `K2` (1) of the save (confirmed: ps_dbAgent 0x405d4c, `+0x80` + 4 × the kind) |
| `0x0B09` | `FA13` | `u32 guild, u32 char, u8 rank` | the member's rank (`docs/original/guild.md` section 7) |
| `0x0B0B` | `FA15` | `u32 guild, u32 points` | the guild's points |
| `0x0701`, item removal | `F702`, `F703` | as `ItemCreate` / `ItemRemove` | as for any new or removed item |

`FA14` saves nothing. NPCs and monsters made by a GM are never saved. The bans are never saved.

## 11. The client (from `docs/client/gm-and-commands.md`, not read here)

- The client is "staff" for privilege 1 to 10 and gates each command itself (its section 2a); it parses the arguments, sends the packet and
  prints a queued confirmation on `F501 0` or a failure text by opcode.
- `/apower` sends `F601` as `name[21], u8 level` while the original reads `u8 level, name[21]`.
- `F904`/`FA09`/`FA0A`/`FA11`/`FA12` with the client's own id are a map change; with another id they move that character. `F905`/`F90C` take the
  height from the terrain (part 2).
- **Open for the client side**: whether `0x051D` with 13 hides the character for every viewer (and what a staff viewer sees); whether
  a same-map move answered with `0x020B` only (no `FA0A`) leaves anything undone in the client (deviation 6 below); whether `0x020B` to
  another map with y 0 puts the character on the ground; what `F701` with stat 16 (exp) and stat 1 change on screen (level, bar); whether
  the level-up packet `0x051E` after `/set level` is shown as a level-up. These need a running client (live: `docs/testing/live-test-checklist.md`
  section 65).

## 12. Original oddities

- **Unhandled but sent commands disconnect the GM**: `/aban`, `/abancan` (`F70A`, `F70B`), `/pvpon`, `/pvpoff` (`FA16`, `FA17`), and every
  part-2 command the original lacks (`F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`, client).
- **`F601`'s layout differs** between the client and the server, and the level has no range check (0 or 255 are taken).
- **`/set level` sends the main stat of the GM's job**, and **`/set DEX`, `REC`, `WIS` change the GM's maxima**, not the target's.
- **`/set exp` beyond the last level's start sets the level to the grow number** (1 to 3).
- **`/set CG`, `OG`, `IG` accept 256 and store 0**; HG and VG/CG/OG/IG go to ps_dbAgent with kinds 1 and 2 only.
- **`/set level` gives or takes no stat or skill points**: a character set from 1 to 60 has the points of level 1.
- **`F702` with an unknown item answers nothing.**
- **`F703` removes the item from the equipment too**, and needs the target in the GM's zone, unlike `/set`.
- **The removal commands succeed when nothing was near** (`F705`, `F70D`).
- **An unknown map in `FA0A`, `FA11`, `FA12` falls back to the GM's current map**, with the given coordinates.
- **`FA09` cannot reach a player in an instance, house or ranking zone**, and `FA12` cannot target a plain map.
- **`FA13` grade 1 makes a second master**; `FA13`, `FA14` and `FA15` do not answer on success.
- **`FA14` sets `byUseHouse` to 1** with the ranking, in memory only: the next ranking update or a restart undoes it.
- **Bans end with a relog** (memory only, cleared on every character reset).
- **The listening copies (`F502`) go to a client that cannot show them**, and a zone line's (`0x1111`) prefix has a zero inside (`[ZONE\0]`), so
  a client that stops at the first zero shows only `[ZONE`.
- **A GM's link outlives its relog on the player's side**: the GM's reset clears `+0x5784`, not the player's `+0x5780`, so the copies start
  again when the GM comes back while its `/cwatch off` answers "none". A second GM's `/cwatch` takes the player from the first, whose `+0x5784`
  still names it.
- **Staff of any level enter invisible and untouchable**, also levels 6 to 10 who cannot send `FA02`/`FA06` to change it.

## 13. The new server (built)

Built per `docs/plans/done/gm-commands.md`, with all eighteen deviations below. Everything above as the original, with the deviations below. The checks run in the GM's own
connection; what changes another player is carried out by that player's connection (an order through its zone event channel), which
answers the GM. The game log has no server here: each command is logged at `info` ("gm command", with the GM, the opcode, the target and the
values) instead.

- **Data**: nothing new. **Database**: migration `0022_gm_commands.sql` for deviations 16 and 17 (two tables, below); otherwise no change. The staff level is `accounts.status` (bits 4 to 6 = level 1 to 5, as
  `usp_Save_User_Status_E`); `F601`/`F602` write it with `UPDATE accounts SET status = $2 WHERE id = $1`. `/set` saves through the existing
  calls (`save_progress`, money through `trade`, honor `save_honor`, kills `save_kills`) and one new call for grow, VG, CG, OG and IG
  (`UPDATE characters SET grow/vg/cg/og/ig = …`). Guild ranks and points through `GuildWrite`.
- **Memory**: per player in the zone (`Occupant`): `visible` and `attackable` (both false for staff on entering, true otherwise), read by the
  monster, guard and player target checks as the original's two bytes. Per character, in the zones: the chat and move bans with their end
  (deviation 9), the listener links (deviations 21 and 23). Per GM connection: nothing else (moves are carried out at once). Also per player in the zone: `pvp`
  (deviation 15; false for staff on entering, true otherwise). Server-wide: the closed quests (deviation 16), loaded at start.

**Deviations** (all approved and built):

1. **No disconnect for a staff member's unknown or unbuilt command**: from a staff member (level ≤ 10), an `Fxxx` the original has no case for
   and the new server does not define either (`FA03`, `FA04`; the ten of deviations 14 to 17 are built), a part-2 or market command not built
   yet (`F2xx`, `F8xx` other than `F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`, `F9xx`, `FA0B`, `FA0C`, `FA19`, `FA1A`; part 2 and
   `FA19`/`FA1A` are built, `docs/original/gm-commands-2.md` and `docs/original/auction.md` section 12, and `FA0B`/`FA0C` with
   deviations 21 to 24, which leaves the part-2 opcodes the original has no case for), or a family above the member's level (with deviation 15's exception), answers
   **`F501 <opcode>`** and is logged. A player (level 11) sending any `Fxxx` is still disconnected, as the original.
2. **`F601` reads the client's layout** (`name[21], u8 level`, 24 bytes) and takes only levels 1 to 10 or 11; anything else → `F501 F601`. The
   status saved is level × 16 for 1 to 5 and 0 otherwise, as the original. The original reads the level from the name's first byte, so with
   the US client it cannot work.
3. **`/set` on the target's own job and maxima**: `/set level` sends the main stat of the **target's** job, and every stat change (5 to 10)
   recomputes the target's totals and maxima (as a level-up does: `max_all` to the target and its view, then a fill) instead of adding to
   the GM's.
4. **`/set` values that fit**: CG, OG and IG take 0 to 255 (256 refused); `/set exp` beyond the start of the mode's highest level sets that
   highest level (60, or 30 in easy mode) instead of the grow number.
5. **`F703` empties the bags only** (bags 1 and up), not the equipment, and keeps the original's checks (the target in the GM's zone). Removing
   worn equipment would need the looks, stats and set changes of an unequip, which nothing else here does by force.
6. **GM moves happen at once, with `0x020B`**: `FA09`, `FA0A`, `FA11`, `FA12` move the GM when the command arrives (no deferred pass) through
   the existing move (`move_player`: the same map → `0x020B` to the view and the GM; another map → `0x020B` with the new map, as portals do),
   instead of the 20-byte `FAxx` packet. A move refused because a logout or another move is pending answers `F501 op` (the original answers
   0 and does nothing).
7. **Unknown maps and map 200 are refused**: `FA0A`, `FA11`, `FA12` with a map the world does not have, or 200, answer `F501 op` (the original
   falls back to the current map, or goes to another server).
8. **`FA12` into a zone that exists**: the GM enters the party's or guild's **existing** zone of that map; when it has none yet, the guild
   house and ranking maps make it as their portals do (`enter_guild_house`, `enter_ranking` without their refusals), a party map makes it as
   `enter_instance` would. A refusal by those (no room) → `F501 FA12`. The zone choice is not kept for later map changes (no `+0x54c8`).
9. **Bans survive a relog**: chat and move bans are kept by character id in the server's memory until they end or the server restarts (the
   original clears them on every relog); a player entering the world with a move ban gets move speed 255 at once.
10. **`FA13`, `FA14`, `FA15` answer `F501 0` on success**; **`FA13` grade 1 is refused** (`F501 FA13`: one master per guild); **`FA14` is saved**
    (the guild's ranking through `GuildWrite::Standing`, `use_house` 1 with it, as the original sets it in memory) so a restart keeps it until
    the next ranking update; **`FA15` writes the points** (`GuildWrite`) and also answers `F501 0`.
11. **NPCs made by `F70C` belong to the GM's zone only** (not to the map's list for later instance zones). They and `F70D`'s removals last until
    the zone goes or the server restarts, as the original.
12. **`/server off` refuses new game connections** (closed when accepted, logged) and **`/server on`** accepts them again; nothing tells the
    login server (it has no channel for it), so the server stays listed. Connections already in the world stay. The flag starts on at each
    start, as the original.
13. **Monster removal by `F705`/`F70E` is a quiet death at the next body pass** (`dead_until = now`, as `LuaDeleteMob`): gone within a second,
    with the spawn entry's respawn as the original's removal, no experience and no drops. `F70E` refuses a monster that is already dead.

Deviations 14 to 18 build the commands the original disconnects for (all approved). The client texts they
answer to are in `sysmsg-uni.txt` (the GM's confirmations on `F501 0`: 290 "Prohibit [<p>] from connecting", 291 "… for <v> minutes", 292
"Allow [<p>] to connect", 259 "Open [<b>]", 260 "Down [<b>]", 287 "Add [<b>]event of [<p>]", 288 "Remove [<b>]event of [<p>]", 266 "Will be
attacked", 265 "Won't be attacked anymore"; the failures 2035 "Can't use function of restricting account." for `F70A`/`F70B`, 2026 for
`F805`/`F806`, 2027 "Can't use quest commands.", 2028 "Can't use event commands."; the banned player's 340 "Ban connecting by operator" and 341
"Ban connecting for <v> minutes by operator", shown on `F70A`/`F805 u16 minutes`, client section 3).

14. **Account bans: `F70A` `/aban`, `F70B` `/abancan` (level ≤ 2), `F805` `/ban`, `F806` `/bancan` (level ≤ 3)**. Wire: `F70A`/`F805` `u16
    minutes, char name[21]` (25 bytes), `F70B`/`F806` `char name[21]` (23). `/aban` and `/abancan` name an **account** (as `/akick`); `/ban`
    and `/bancan` a **character**, whose account is meant (a deleted character's name is not found). Online or not, through the database.
    - **Ban**: the account is blocked as the login server already understands it: `status = 2` ("account cannot log into the game", 0xA102
      result 4) with `blocked_until = now + minutes`, or no end (`NULL`) for 0 minutes. The login server refuses it and lifts a timed block
      itself once it has passed (`crates/login-server/src/accounts.rs`, as the original's `Status` 2 to 6). If the account is in the world here,
      its connection gets **`F70A`/`F805 u16 minutes`** (the opcode the GM used; its client shows 340 or 341) and is then closed. Refused (`F501
      op`): no such account or character; the account is staff of the GM's level or higher (its `status` staff level ≤ the GM's level,
      `status` bits 4 to 6 not 0); the account is the GM's own.
    - **Lift**: an account whose `status` is 2 to 6 (a block) gets `status = 0, blocked_until = NULL`; otherwise `F501 op`. A staff account
      that was blocked comes back as a player (its level must be given again with `/apower`).
    - `F501 0` when done.
15. **`FA16` `/pvpon`, `FA17` `/pvpoff`: whether players may attack this GM** (any staff level 1 to 10, as the client sends them with no check
    of its own; the family's level 5 limit does not apply to these two). A second flag beside `attackable`: **`pvp`**. Players may attack a
    staff member, or reach it with a skill, only when it is visible and `pvp` is set; monsters and guards look at `attackable` as before.
    `/attack on|off` (`FA05`/`FA06`) sets both (as the original's one byte); `/pvpon` sets `pvp` only, `/pvpoff` clears it only. Both false on
    entering for staff, true for players (whom nothing changes). `F501 0`; nothing is sent to others.
16. **`F809` `/queston`, `F80A` `/questoff` (level ≤ 3): a saved, server-wide quest switch**. Wire: `u16 quest` (4 bytes; the client finds the
    id from the quest's name or takes the number). `/questoff` closes a quest: nobody can **start** it (`0x0902` is ignored for it, as for a
    quest the player may not take); quests already in progress carry on and can be finished. `/queston` opens it again. Saved in
    `closed_quests (server_id smallint, quest_id integer, closed_at timestamptz DEFAULT now(), PRIMARY KEY (server_id, quest_id))` and loaded at
    start. Unknown quest, closing a closed one or opening an open one → `F501 op`; else `F501 0`.
17. **`F80F` `/eventadd`, `F810` `/eventrem` (level ≤ 3): event tags on characters**. Wire: `char name[21], char event[21]` (44). A tag is a
    name (1 to 20 bytes, kept as typed, compared without case) on a character, online or not: `character_events (character_id integer
    REFERENCES characters ON DELETE CASCADE, name varchar(20), added_at timestamptz DEFAULT now(), PRIMARY KEY (character_id, name))` with a
    unique index on `(character_id, lower(name))`. `/eventadd` adds it (already there → `F501 F80F`), `/eventrem` removes it (none → `F501
    F810`); no such character → `F501 op`; else `F501 0`. **Nothing reads the tags yet.** Uses they are meant for, later (each a small
    change once wanted): an NPC or quest that gives an event reward only to tagged characters and removes the tag when it does (a "claim
    once" prize for event winners); a GM command acting on everyone with a tag (summon, notice or reward all participants: part 2's
    `/partysummon`-style moves and notices); a portal or map open only to tagged characters during an event; and the GM tool reading who
    took part. The original's own `/event on|off` (`F80D`/`F80E`, part 2) are unrelated: in this `ps_game.exe` they heal or cure a player.
18. **The ban notice before the kick**: the account's connection is closed with the ban packet first (deviation 14), so `OnlineUsers` carries
    a reason with its kick (a newer login: `0x0004 0` as now; an operator's `/akick`: nothing, as the original; a ban: `F70A`/`F805 u16
    minutes`).
    Amended by deviation 1 of `docs/original/small-items.md` (built): after the ban notice the socket stays open `kick_notice_secs`
    (3 s), the character already out of the world, so the client shows 340/341 before its disconnect box.

19. **`F706` flips the server switch** (found in play). The US client compares only 9 characters of "/server on" (`0x4728b7`), so
    typing `/server off` is taken as `/server on` and sends `F706`; `F707` is never sent (it needs the Korean word). `F706` therefore
    turns the switch to the other state (a `/server on` while on turns it off), `F707` still sets it off. *Why*: otherwise the server
    could never be closed to new logins with this client.
    **Withdrawn**: the companion DLL (`fixes.dll`, `docs/client/CLIENT-FACTS.md`) now sends `F707` for `/server
    off`, so `F706` opens and `F707` closes again, both idempotent, as the original (`docs/original/small-items.md` section 4; built per
    `docs/plans/done/small-items.md`).

20. **A closed server is listed as closed** (found in play). While `/server off` holds, the game server's heartbeat clears
    `game_servers.last_seen`, so the login server's server list shows it closed (the client's "closed" state) instead of normal; `/server on`
    (or the flip) writes the heartbeat at once.

Deviations 21 to 24 come with operator chat listening (`FA0B`, `FA0C`, `F502`; built for the companion DLL `fixes.dll`, which shows the
copies, `docs/client/operator-listen.md`; approved):

21. **A link ends when either side leaves the world** (`Zones::forget_listens` at leaving): the GM's and the player's link both go, so a relog
    never starts the copies again by itself (the original keeps the player's side, section 12). A GM whose level goes above 5 (`/apower`)
    loses its link at once (the original drops it at the player's next line; nothing is sent either way).
22. **`ZONE` without its zero**: the zone line's prefix is `[ZONE]`, 4 letters, so the line is seen (the original's 5 bytes put a zero inside
    the prefix). Every other kind is as the original's, letters and order: `NORMAL`, `TO,<target>`, `ALL`, `GUILD`, `PARTY`, `SHOUT`, `TRADE`.
23. **One link per player and per GM, both ways**: a second GM's `/cwatch` on the same player takes it from the first, whose `/cwatch off`
    then answers `F501 FA0C` (in the original the first GM keeps a half link).
24. **The copy ends in a counted zero**: the line as the speaker sent it, plus a zero when it has none, the whole text at most 255 bytes
    (never reached: 25 + 128 at most). The DLL accepts either; it prefers the zero.

Built: `shaiya_proto::admin::listen_chat` (`F502`), `admin::Heard` (each line's kind: the prefix and the chat log's type), `Zones::listen`,
`listen_off`, `forget_listens`, `send_listener` (`zone/admin.rs` `Listens`, a lock of its own), `FA0B`/`FA0C` in `server/admin.rs`, and
`server.rs` `said` beside every relayed line (the chat log's record and the copy, as the original's two calls). The GM's own guild, party and
union lines are logged and copied only when they reached a guild, party or union (the original's checks; the build logged them always).
Logged as "gm listens to a player's chat". Live test section 49 (needs the DLL installed).

- **Small choices made in the build** (not deviations from the spec's text, the spec leaves them open): `FA0A` and `FA11` into a map with a
  create type (party, house, ranking, guild maps) answer `F501 op` (the original's "−1 zone choice" has no equivalent; `FA12` is the way in);
  `FA13` also refuses changing the master's own grade (grade 1 is refused, so a demoted master could never be restored); `/set` on a
  dead character is refused (the refill that follows would bring it back); "within 10 (x and z)" is the horizontal distance
  (`SVector::Distance2`) for `/mera`, `/nera` and `/xcall`; `/set` level 0 is refused; the `F10A` GM union line is sent only by a union's leader
  or sub-leader as for players.
- **Not built** (answer `F501 op`, deviation 1): `FA03`, `FA04` (no case in the original, and the US client does not send them),
  map 200. Built: `FA0B`/`FA0C` with the copies (deviations 21 to 24, above), `FA19`/`FA1A` with the market (`docs/original/auction.md` section 12; `server/admin.rs`, `server/market.rs`
  `seller_search` and `recall`), the Message To Server variant of `F101` (`docs/original/small-items.md` section 5; `server/admin.rs`), and
  part 2: `F201` to `F204`, `0x1201`, the `F8xx` and `F9xx` cases (`docs/original/gm-commands-2.md`).
- **Not deviations, but different underneath**: `F501` for a command on another player comes after that player's connection has applied it
  (the original answers at once, from one thread); when the target leaves the world before its connection takes the order, nothing
  happens and the GM gets no answer. Saves go to the database before the answers, not through ps_dbAgent's queue.
  Every value from a command (names, texts) reaches SQL only as a bound parameter, so the original's SQL injection through GM commands
  (`docs/original/community-fixes.md`) cannot happen.

## 14. Other episodes

The EP3 `ps_game.exe` (linked 2008-01-17) and the EP5.4 one (linked 2011-09-14, modified after linking) were compared with the EP4.5
build, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

The same GM groups `0xF1xx` to `0xFAxx` (EP3 ps_game 0x457000 to 0x45a7e0, matched by switch position) with the same opcodes, except
`0xF10A` and `0xFA18` to `0xFA1A`, which EP4.5 added (confirmed, dispatch tables). The family levels were not compared.

### EP5.4

- The family limits are unchanged: `PacketAdminChat` 5, `PacketAdminUQ` 10, `PacketAdminCmdA` 1 (2 inside), `B` 2, `C` 3, `D` 4, `E` 5
  (confirmed). The staff level byte moved from `CUser+0x5768` to `+0x5808`.
- The opcodes the US client sends without a server case (`F70A`, `F70B`, `F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`, `FA16`, `FA17`)
  still fall to the group default and disconnect the GM (confirmed, dispatch tables).
- Three new opcodes (confirmed):

  | opcode | layout (with the opcode) | where (EP5.4 ps_game) | what |
  |---|---|---|---|
  | `0xF603` | `u8 len` (≤ 0x7F) @2, text @3 | 0x480b34 | console commands from the game, see [`operator-console.md`](operator-console.md) |
  | `0xF70F` | 5: `u16` @2, `u8` @4 | 0x481955 | `/mmake` on every server: as `F704` (count ≤ 20, `CZone::MobGen` at the GM), then `0x0818` (19 bytes: monster, count, map, x, y, z) to the session, which passes it to the other game servers; each summons the same at that spot on that map and reports `gm cmd mmakeall ok` as `0x080E` (EP5.4 ps_game 0x427f80); log `MobCreateAll` |
  | `0xFA1B` | 32: guild name @2 (cut at 0x1A), `u32` @0x1B, `u32 char id` @0x1B, `u8 rank` @0x1F (1 to 9) | 0x484099 | `FA13` by character id: sets the member's rank (leaders capped at 7), DB agent `0x0B09`, log `UserGuildLevel2` |
