# Quests

This page covers quests in the EP4.5 game server `ps_game.exe`: the quest definitions, who may take a quest, the five
ways to start one, kill counts (alone and in a party), the four ways to end one, choosing a reward, giving up, time limits, the lists at
login and what is stored. It was read in the disassembly with the PDB names, the quest data in `Data/NpcQuest.SData`, the database agent
`ps_dbAgent.exe` and the stored procedures of `PS_GameData`. The client side (what `game.exe` does with each `0x09xx` packet) is on
[World packets](../client/client-world-packets.md).

On other pages: the staff commands that add and remove quests (`CUser::QuestAddAdmin`, `QuestRemoveAdmin`, `F80B` and `F80C`;
[GM commands 2](gm-commands-2.md#44-quests-f80b-questadd-name-id-f80c-questrem-name-id)), and game logs (`TP_GAMELOG_QUEST_*` to
the game log server; [The game log](game-log.md)).

## In short

- **1,720 quest records** come from `NpcQuest.SData`, the same file the client uses. Each has a level range, race, class and mode
  limits, and may need an earlier quest to be done first.
- **Starting.** Most quests start by talking to an NPC; some by handing over an item, carrying one, or using one. Some give items at the
  start.
- **Ending.** A quest ends by talking to an NPC, bringing items, killing monsters, or killing players of the other faction. Monster
  quests without an end NPC finish the moment the last kill is made. Party members within 100 units all count a monster kill.
- **Rewards** are experience (doubled for quests open to modes below ultimate), money and sometimes an item; some quests let the player
  choose one of two or three rewards, and some pay more for bringing more items or killing more monsters.
- **One chance.** A quest that is finished, failed or given up can never be taken again, except the 69 repeatable ones. Timed quests
  fail when their time runs out; the clock stops while the player is offline.
- **Faults:** quests can be handed in without visiting the NPC, items given or rewarded vanish when the bags are full, and the NPC need
  not be anywhere near.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (see [NPC shops](shop.md) on the
PDB's 4-byte shift).

## 1. The definitions

Confirmed, : `tools/npcquest.py` reads the whole file to its last byte.

`DBInitializeConstNpcQuest` (0x4caf80) reads `data/NpcQuest.SData` in this order: the NPCs (`CNpcData::Load`, `docs/original/maps-npcs.md`
section 3), the item links (`CItemQuestData::Load` 0x4c9d90), the quests (`CQuestData::Load` 0x4ca5d0); then it builds the monster links
(`CMobQuestData::Init`) and marks the quest items (`CGameData::CalcQuestItem`).

**Item links**: 256 × 256 records, item type then type id, each `u32 n, n × u16` (quests started) and `u32 m, m × u16` (quests ended);
505 items have any. Only `CUser::QuestUpdatePickItem` reads the "ended" lists, and nothing calls it in this build (section 6).

**Quests**: `u32 n` (1,720), then per quest: `u16 id`, `u32 len, name[len]` (Korean, cut to 30 characters), `u32 len, summary[len]` (not
kept), the fixed fields below in this order (each read on its own; `stQuestInfo`, 464 bytes), three results, and seven texts
(`u32 len, text[len]`, not kept). **A quest is found by its position: id `n` is record `n − 1`** (`CQuestData::GetQuest`), and every
record's id is its position.

| field | type | meaning | in the file |
|---|---|---|---|
| `wLevelMin`, `wLevelMax` | u16, u16 | the level range | |
| `byCountry` | u8 | who by race (section 4) | 0: 85, 1: 74, 2: 639, 3: 59, 4: 66, 5: 616, 6: 181 |
| `byGrow` | u8 | the lowest mode (0 easy to 3 ultimate) | 0: 1044, 1: 232, 2: 340, 3: 104 |
| `bySex[2]` | u8 × 2 | not checked by the server | 1713 both, 7 one |
| `byJob[6]` | u8 × 6 | 1 for each class allowed | |
| `wHG`, `shVG` | u16, i16 | **two more previous quests** (section 4) | 35 and 17 set |
| `byCG`, `byOG`, `byIG` | u8 | not used | all 0 |
| `wPrevQuest` | u16 | the previous quest | 1092 set |
| `byParty`, `byPartyJob[6]` | u8 | not used | 0 (one quest has a party job) |
| `dwTimeMin` | u32 | minutes before it may end | all 0 |
| `dwTime` | u32 | **time limit** in minutes | 162 quests: 60, 120, 180, 360 |
| `dwTickStartTerm`, `dwTickKeepTime` | u32 | not used | all 0 |
| `dwTickRecvCount` | u32 | **not 0: repeatable** (section 9) | 69 quests |
| `byStartType` | u8 | how it starts (section 5) | 0: 390, 1: 1255, 2: 25, 3: 18, 4: 32 |
| `byStartNPCType`, `wStartNPCID` | u8, u16 | the NPC that gives it (type 0: any) | |
| `byStartItemType`, `byStartItemID` | u8, u8 | the item of start types 2 to 4 | |
| `byGetItem…1` to `3` | 3 × (type, type id, count) | **items given at the start** | 262 quests |
| `byEndType` | u8 | how it ends (section 7) | 0: 619, 1: 726, 2: 348, 3: 27 |
| `byEndNPCType`, `wEndNPCID` | u8, u16 | the NPC that takes it back (type 0: none) | |
| `byEndItem…1` to `3` | 3 × (type, type id, count) | **items to bring** (end type 1) | |
| `byPCKillCount` | u8 | players of the other faction to kill (end type 3) | |
| `wMOBKillID[2]`, `byMOBKillCount[2]` | 2 × (u16, u8) | monsters to kill (end type 2), read id, count, id, count | |
| `byResultType` | u8 | 1: always result 0; 0: chosen (below) | 1: 1356, 0: 364 |
| `byResultUserSelect` | u8 | when not 0, the player picks one of this many results | 2: 157, 3: 140 |
| `pResult[3]` | 3 × 32 bytes | `u16 wNeedMobID, u8 byNeedMobCount, u8 byNeedItemID, u8 byNeedItemCount, u32 dwNeedTime, u16 wNeedHG, i16 shNeedVG, u8 byNeedOG, u32 dwExp, u32 dwMoney, u8 byItemType, u8 byItemTypeID, u16 wNextQuest` | the `Need…ID` are 1-based indexes into the kills or the end items; `dwNeedTime` and the other needs are all 0; `wNextQuest` is the client's (the dialog goes on with it) |

Real quests vs. placeholders: 149 records end with type 0 and no end NPC; none of them has a name, a summary or a reward, and section 7
never ends them. 20 records start by "clicking" an NPC of type 0, which no NPC is (never startable).

## 2. The state

Confirmed, `CQuest`, `CUser` +0x11C8 and +0x11F4.

Per player, two lists of `CQuest` (36 bytes): **in progress** (`m_Quest`, `+0x11F4`) and **finished** (`m_QuestFinish`, `+0x11C8`). A
`CQuest` holds the quest id, `bySuccess` (finished: 0 failed, 1 done; in progress: 1 while a reward choice is pending), the start time,
the end time (start + `dwTime` minutes, 0 without a limit), the earliest end (start + `dwTimeMin` minutes, 0 without), **three counters**
(`byCount[3]`) and the definition. Two numbers count the quests in progress that end by killing players (`+0x57C4`) and monsters
(`+0x57C8`), so kills skip players without any.

## 3. The packets

Confirmed, `CUser::PacketQuest` 0x46be10.

Every quest packet first cancels what the player is doing (`CUser::CancleActionExc`) and closes its personal shop. Client → server:

| op | layout | handler |
|---|---|---|
| `0x0902` | `u32 npc id, u16 quest id` | start (section 5) |
| `0x0903` | `u32 npc id, u16 quest id` | end (section 7) |
| `0x0904` | as `0x0902` | start as well (the client never sends it) |
| `0x0907` | `u16 quest id, u8 result index` | the chosen reward (section 8) |
| `0x0908` | `u16 quest id` | give up (section 9) |

`0x0905`, `0x0906` and anything outside `0x0902`–`0x0908` close the connection (`SConnection::Close(9)`).

Server → client: `0x0901` list (section 10), **`0x0902` (8) `u32 npc id, u16 quest id`** started (the NPC's object id, 0 when started
without one), **`0x0903` (22) `u32 npc id, u16 quest id, u8 success, u8 result index, u32 exp, u32 money, u8 bag, u8 slot, u8 item type,
u8 type id`** ended, **`0x0905` (6) `u16 quest id, u8 counter, u8 count`** progress, `0x0906` finished list, **`0x0907` (4) `u16 quest
id`** "choose your reward". The client takes the reward item (count 1) into bag/slot from `0x0903` and ignores its `exp`: experience comes
with its own packets (section 7.4).

## 4. Who may take a quest

Confirmed, `CUser::QuestEnableStart` 0x47f080.

All of:
1. level ≥ `wLevelMin` and ≤ `wLevelMax`;
2. race (`+0x12E`) by `byCountry`: **0** human (0), **1** elf (1), **2** human or elf, **3** deatheater (3), **4** vail (2), **5** vail or
   deatheater, **6 or more** anyone;
3. `byJob[job]` not 0;
4. mode: the player's mode is at least the quest's (`byGrow` ≤ the player's `+0x12F`);
5. when any of `wPrevQuest`, `wHG`, `shVG` is set: **one of them** (the set ones) is among the player's finished quests **with success**
   (`QuestFindFinishSuccess`).

Sex is not checked (the client's business).

## 5. Starting

Confirmed, `CUser::QuestStartRequest` 0x47f220.

The quest id must be 1 to the number of quests; then by `byStartType`. **No answer when anything fails.** Every type also needs the quest
**neither in progress nor finished** (`QuestFind`, `QuestFindFinish`: a finished quest, failed or done, is never taken again; repeatable
quests are not kept as finished, section 9) and section 4.

| type | `QuestStartChk…` | needs |
|---|---|---|
| 0 | `Prev` 0x47f570 | an NPC with the packet's id **in the player's zone** (`CZone::FindNpc`); if `byStartNPCType` is set, of that type and type id |
| 1 | `ClickNPC` 0x47f390 | that NPC, of the quest's start type and type id |
| 2 | `ItemGive` 0x47f400 | that NPC as for 1, and **one** of the start item in the bags, **taken** (count − 1: `0x0206 bag, slot, type, type id, count`, or the slot emptied, 7 bytes) |
| 3 | `ItemPick` 0x47f490 | the packet's npc id field **equal to the start item's type id**, and the start item in the bags (kept) |
| 4 | `ItemUse` 0x47f500 | the packet's npc id field equal to the start item's type id, that type id the **last quest item used** (`+0x57D4`) and that use less than 5 minutes ago (`+0x57D0`) |

The bags searched are 1 to the usable bag count, slot by slot. **The NPC is looked up by object id in the whole zone: there is no distance
check.** For type 4, `CUser::ItemUse` (0x4655e0) arms it: using an item of type 27 or 28 records its type id and now + 300 s and does
nothing else; type 29 does the same and is then used up (`ItemUseNSend`).

Then (`CUser::QuestAdd` 0x47e010, `QuestStart` 0x47ea70): the quest goes into the in-progress list with counters 0, start now, end now +
`dwTime` minutes when set; the kill numbers follow its end type; the database gets the quest (`0x0A01`, `TP_DB_QUEST_START`: char id,
quest id, `dwTime`). The player gets **`0x0902 (NPC object id or 0, quest id)`**, then each start item with a type (`QuestGetStartItem`):
`CUser::QuestAddItem` (0x47e620) puts it, with the quest's count, **in the first empty slot** (no stacking) and sends `0x0205` (16 bytes:
bag, slot, type, type id, count, quality, gems 0, the empty craft name's zero, as a picked-up item without a craft name). **When the bags are full the item is lost** and the
quest stays started.

## 6. Progress

Confirmed.

**Monster kills**, `CUser::QuestUpdateKillMOB` (0x480140), from `CMob::Death` (0x4910f0): for the player who did the most damage alone,
or, in a party, **for every member within 100 of that player** (the same members who share the experience, `docs/original/party.md`
section 8). Skipped without a monster-kill quest in progress. For each quest of that player in progress whose end type is 2 and that
names the monster (`CMobQuestData`: the quests by monster id): when `wMOBKillID[i]` is the monster's id (each i, so one monster can count
for both) and counter i is below 255: counter i + 1, the database (`0x0A05`, `TP_DB_QUEST_UPDATE`), and **`0x0905 (quest, i, count)`**. Then,
**when the quest has no end NPC** and both counters reach their counts (`byMOBKillCount`), it ends at once (section 7, NPC id 0).
Counters go on past the count (up to 255).

**Player kills**, `CUser::QuestUpdateKillPC` (0x480020), from `CUser::KillPCTo`: only **the first** quest in progress with end type 3:
counter 0 + 1 (below 255), `0x0A05`, `0x0905 (quest, 0, count)`; without an end NPC it ends when counter 0 reaches `byPCKillCount`.

**Picked-up items**: `CUser::QuestUpdatePickItem` (0x480390) would count pick-ups of the end items the same way, but **nothing calls it**:
item quests are only checked against the bags when they end (section 7.2), and their counters stay 0 until then.

## 7. Ending

Confirmed, `CUser::QuestEndRequest` 0x47f2a0, `QuestEnd` 0x47ec40.

The quest must be in progress. With an end NPC (`byEndNPCType` ≠ 0), by `byEndType`: 0 `ClickNPC`, 1 `ItemPick`, 2 `KillMOB`, 3
`KillPC`; without one: 1, 2, 3 the same and **0 does nothing**. Each check (`QuestEndChk…`, 0x47f630 to 0x47f740):

| end type | needs | ends with NPC id |
|---|---|---|
| 0 | the NPC with the packet's id in the zone, of the quest's end type and type id | the packet's |
| 1 | that NPC **when the packet's NPC id is not 0**; the end items in the bags (7.2) | the packet's |
| 2 | that NPC when the packet's id is not 0; counter 0 ≥ `byMOBKillCount[0]` and counter 1 ≥ `byMOBKillCount[1]` | 0 |
| 3 | that NPC when the packet's id is not 0; counter 0 ≥ `byPCKillCount` | 0 |

**The original skips the NPC whenever the packet's NPC id is 0**, so item, monster and player quests can be handed in anywhere (section 11).

### 7.1 Which result (`QuestEnd`)

- Before `dwTimeMin` minutes have passed: nothing (no quest uses it).
- `byResultType` 1: result 0.
- Else with `byResultUserSelect` set: the quest is marked pending (`bySuccess` 1), **`0x0907 (quest)`** asks the player, and the items are
  not taken yet (section 8).
- Else: the **highest** result i (2, 1, 0) whose need is met — end type 1: `byNeedItemCount ≥ 1`, `byNeedItemID` 1 to 3 and that
  end item's count ≥ it; end type 2: `byNeedMobCount ≥ 1`, `wNeedMobID` 1 or 2 and that counter ≥ it — else **result 0**. In the data
  these are "bring 5 / 10 / 15" and "kill 2 / 5 / 10" quests with better rewards for more (64 quests).

### 7.2 The end items (`CUser::QuestFindItem` 0x47f8e0)

The bags (1 to the usable count, slot by slot) are counted per end item (an item counts for the first end item it matches). **Each count
must reach that end item's count**, or nothing happens. When the items are taken (all cases but a pending choice): **every stack of real
type 27 (quest items, item types 27 to 29) that matches an end item is taken whole**; other matching items only up to the count needed —
the end item's count, or, for the end item the chosen result names, that result's `byNeedItemCount` — stack by stack, each change to the
database (`0x0702`) and the player (`0x0206`, 7 bytes: `u8 bag, slot, type, type id, count`, all zero but bag and slot when emptied).

The counts it leaves in the quest's counters (used by 7.1) are muddled in the original (a counter is set from the wrong end item when a
result needs another one); with the data as shipped the outcome is the rule of 7.1 with the real counts.

### 7.3 Failure (`QuestEnd` with success 0)

`0x0903 (npc id, quest, 0, 0, 0, 0, 0, 0, 0, 0)` ("Failed the quest"), then the quest leaves the list as failed (section 9).

### 7.4 The reward (`CUser::QuestSuccessResult` 0x47ee40)

For result i:
1. **Experience**: `dwExp`, **doubled when the quest's `byGrow` is 0 to 2** (`cmp [quest+0x107], 2; ja; add esi, esi`); quests for
   ultimate mode only (grow 3) give it once. Added with `CUser::AddExpFromUser(killer 0, exp, no premium)`: `0x0207`, or the level-up
   packets with **`0x051E`** (the killer id 0 is not the player's, `docs/original/combat.md` section 5).
2. **Money**: `dwMoney`, the total capped at 2^32 − 1; database `0x0603`.
3. **Item**: the result's item, count 1, through `QuestAddItem` (the first empty slot; 0x0701 to the database). **When the bags are full
   it is lost** and the packet carries zeros.
4. **`0x0903 (npc id, quest, 1, i, exp, money, bag, slot, item type, type id)`**: the exp as doubled, the money as in the result.
5. The quest leaves the list as done (section 9).

## 8. Choosing a reward

Confirmed, `CUser::QuestEndSelect` 0x47ebe0.

`0x0907 (quest, index)`: the quest in progress and pending (`bySuccess` 1), the index below `byResultUserSelect`; with an end NPC the end
items must still be there and are taken now (7.2, the result's need for its item); then section 7.4 with NPC id 0. No NPC check here.
The new server cuts `byResultUserSelect` to 3 at load, with a warning (audit R24: it indexes the quest's three results, and the data
never holds more than 3 today).
Without an end NPC the items were already taken by the end request.

## 9. Leaving the list, giving up, time limits

Confirmed, `CUser::QuestRemove` 0x47e150, `UpdateQuest` 0x48d1a0.

**Giving up**, `0x0908 (quest)`: a quest in progress ends as a failure (7.3), NPC id 0. No other check.

**Time limits**: every 60 s (`CWorldThread::Update` → `CZone::UpdateQuest`) each player's quests with a limit whose end time has passed
fail (7.3, NPC id 0).

`QuestRemove`: out of the in-progress list, the kill numbers down; then
- **a repeatable quest** (`dwTickRecvCount` ≠ 0) is dropped: the database gets `0x0A02` (`TP_DB_QUEST_END`) with success **2**, which
  makes the database agent delete its row (`CUser::PacketUserQuest`, `AddDeleteQuest`); **it can be taken again at once, also after a
  failure**;
- any other goes to the finished list with success 0 or 1 (`0x0A02`, `usp_Save_Char_FinishedQuest_E`), **failed or not: it can never be
  taken again**, and quests that need it as the previous quest only open with success.

## 10. Login and leaving

Confirmed.

The database agent sends the quests in progress (`usp_Read_Char_Quest_R`: quest, `Delay`, three counters) and the finished ones
(`usp_Read_Char_FinishedQuest_R`: quest, success). `CUser::PacketUserDBSetInfo` rebuilds them (unknown quest ids are logged and skipped):
for a quest with a time limit **`Delay` is the minutes left**, end time = now + `Delay` minutes; without one it is the minutes since the
start. The burst then carries (`docs/original/enter-world.md` section 3, steps 10 and 11):
- **`0x0901`** (`CUser::SendCharacterQuest` 0x4835f0): `u8 n`, n × `u16 quest, u16 minutes left (0 without a limit or when past), u8
  counter[3]`; 255 per packet, only if any;
- **`0x0906`** (`SendCharacterQuestFinish` 0x483730): `u8 n`, n × `u16 quest, u8 success`; 255 per packet, only if any.

On saving (`CUser::SendDBAgentCharGetInfo`, `0x0A04`, `TP_DB_QUEST_SET_LIST`) each quest in progress is written with its counters and
`Delay`: with a limit the whole minutes left (at least 1, at most `dwTime`), so **the clock stops while the player is away**; without one
the minutes since the start (capped at `dwTimeMin`). A pending reward choice is not kept.

## 11. Faults in the original

1. **Handing in without the NPC**: an end request with NPC id 0 skips the NPC for item, monster and player quests that name one.
2. **Lost items**: start items and the reward item vanish when the bags are full; the quest still starts or ends.
3. **No distance check**: any NPC of the zone, by object id, starts and ends quests.
4. **Muddled counters** for "bring more for more" quests (7.2).

## 12. The new server

- **Data**: the quests go into the PostgreSQL schema `world` with the rest of `NpcQuest.SData` (`tools/world/import.py`): `world.quests`
  (every fixed field of section 1 under its meaning) and `world.quest_results` (three per quest). The item links are imported since, for the
  monsters' quest drops (`world.item_quest_links`, `docs/original/quest-drops.md` section 6). The texts stay in the client.
- **Storage**: migration `0011_character_quests.sql`: one row per character and quest, in progress (three counters, the seconds left for a
  quest with a limit) or finished (success). Starting a quest, with its start items given or taken, is one transaction; ending one (the
  items taken, the money, the reward item, the quest finished or dropped) is one transaction; counters are written as they change; the
  time left is written when the character leaves the world.
- **In memory**: the quests live with the player's connection (like its bags and money); kills reach it as the zone's kill events, which
  go to the same players as the original's quest kill counts. Each time limit is a timer of its own.
- Starting by using an item (type 4) works since item use was built (`docs/original/item-use.md` section 2: the quest item and the
  5 minutes are kept by the connection; `0x0902` names NPC 0, as for type 3).
- Player-kill quests (end type 3) are built with player kills (`docs/original/pvp.md` section 7: `Quests::player_killed`, called by
  `quest_player_kill` in `server/pvp.rs`), and the staff commands with the GM commands (`F80B`/`F80C`, `docs/original/gm-commands-2.md`
  section 4.4; the quest switch `F809`/`F80A`, `docs/original/gm-commands.md` deviation 16). **Not built**: `dwTimeMin`, unused by the data,
  is not checked.
- **Deviations** (approved):
  1. **The NPC must be in the player's view** to start or end a quest, as for the shops (the original takes any NPC in the zone).
  2. **A quest that names an end NPC is only handed in at that NPC**: NPC id 0 no longer skips it (fault 1).
  3. **Nothing is lost for want of room**: a start whose items do not fit, or an end whose reward item does not fit once the end items are
     taken, does not happen; the player gets `0x0205` error 1 ("Insufficient inventory"). Start items still go to empty slots, unstacked.
  4. **The end items of a reward choice are always taken at the choice** (the original takes them at the end request when the quest has no
     end NPC), so giving up while choosing keeps them.
  5. **"Bring more for more" uses the real counts** (fault 4), which is what the original's data intends.
  6. **Time limits end to the second** rather than at the next 60-second pass; the time left is kept to the second.
  7. **The money in `0x0903` is what was added** when the cap of 2^32 − 1 cut it.
- The quest items that monsters drop for these quests, and who may pick them up: `docs/original/quest-drops.md` (built).
