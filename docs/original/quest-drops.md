# Monster drops: quest items and sequence items

This page covers the parts of a monster's drop in the EP4.5 game server `ps_game.exe` that the grade entries do not:
which quest item a monster carries, when it drops, who owns it and who may pick it up; quest-starting items; sequence items
(`Data/SequenceItemCreate.ini`) and the monster id counter they count on; and the server-wide drop limits (`wDrop`). It was read in the
Ghidra decompiler and the disassembly with the PDB names; struct offsets are the PDB's. Data counts are the shipped files.

The rest of `CMob::ItemDrop` 0x4a5ea0 (the grade entries and gold) is on [Fighting monsters](combat.md), the item it makes on
[New items and crafting](item-origins.md), and the loot rules on [Parties](party.md) and
[Union parties](union.md). Not covered: the game log records (`0x0A06` for a sequence item, the pick-up logs) and
`CItemDrop::ReSequenceItemCreate` (a reload of the file while running; nothing calls it from a packet).

Why it matters: of the 1,720 quests, **304** end with a quest item (types 27 to 29) that only a monster's quest drop makes, and **26**
quest items that start a quest come only from monsters too.

## In short

- **A monster can carry one quest item.** It only drops when the killer, or any member of the killer's party wherever that member is,
  has one of the item's quests in progress. Then there is one roll at the item's drop chance.
- **Quest-starting items** drop on the roll alone, for anyone, and anyone may pick them up.
- **A quest item always belongs to the killer** and always lands on the ground, whatever the party's loot rule.
- **Only a player with one of the item's quests in progress can pick it up**; anyone else gets "Unavailable quest item".
- **Sequence items** come from every Nth monster the server has ever made, counted across restarts: for example every 16,001st monster
  made drops a Lapisia. There is no roll; the monster's id alone decides.
- **The server-wide drop limits exist in the code** but no shipped monster can reach the five limited items.

Status words: **confirmed** = read in the code (address given).

## 1. The data

Confirmed.

### 1.1 A monster's quest item (`CGameData::LoadConstMob` 0x4c1720)

`usp_Read_Mobs_R` column 8, `QuestItemID` (`defs.mobs.questitemid`), becomes `stMOBInfo.byQuestItemType` `+0x2c` and `byQuestItemTypeID`
`+0x2d`:

| `QuestItemID` | type | type id |
|---|---|---|
| 0 | 0 (none) | 0 |
| 1 to 1000 | 27 | `QuestItemID` as a byte (`& 0xFF`) |
| 1001 to 2000 | 28 | `(QuestItemID + 0x18) & 0xFF` (= `QuestItemID − 1000` up to 1255) |
| above 2000 | 29 | `(QuestItemID + 0x30) & 0xFF` |

Shipped: **271** monsters carry one, **221** different items (types 27 and 28; values 1 to 255 and 1001 to 1145, so no byte wraps), all of
which exist in `defs.items`.

### 1.2 The item's numbers (`stItemInfo`, 168 bytes)

- **`wQuality[0]` `+0x4c`** (`defs.items.quality`) is the **drop chance in percent** for a quest drop. Type 27: 5 to 100, average 78.
- **`bQuestStart` `+0x9c`**: set at load by **`CGameData::CalcQuestItem`** 0x4bf400 for every item (type and type id not 0) whose link
  (1.3) has **`countStart` > 0**: the item starts a quest.
- `nRealType` `+0xa0`: **27** for item types 27, 28 and 29 (`inventory::real_type`).
- `wDrop` `+0x7e`, `nNumTimeDrop` `+0x8c`, `dwDropDelay` `+0x90`, `dwDropRand` `+0x94`, `dwNextDropTime` `+0x98`: section 5.

### 1.3 Item → quest links (`CItemQuestData`, `g_ItemQuestData` 0x3ed35d0)

`stQuestLink m_ItemQuestLink[256][256]` (16 bytes each: `int countStart, u16* pStartID, int countEnd, u16* pEndID`), by item type then type
id. **`CItemQuestData::Load`** 0x4c9d90 reads it **from `NpcQuest.SData`** (right after the NPCs, before the quests: for each type 0 to 255,
each type id 0 to 255, `u32 n`, `n × u16` quests started, `u32 m`, `m × u16` quests ended). `tools/npcquest.py` already parses it
(`parse_quests` returns it as `links`); `tools/world/import.py` stores it in `world.item_quest_links` (section 6).
`CItemQuestData::GetQuestLink(type, type id)` 0x4152f0 gives the link for type and type id **1 to 255** (else none).

Shipped: **481** items have "ended" lists, **46** "started" lists. The ended lists are each quest's end items (`byEndItemType1..3`) grouped by
item, **sorted by the quest's `wLevelMin`** (as `stQuestLink::AddEnd` 0x4c9310 sorts); they match the quests in the same file except one
(quest 1370's end item 44/96 has no link; real type 25, so it never matters). The server uses the file's lists.

### 1.4 Quests in progress (`CUser::QuestFind` 0x47e4b0)

`QuestFind(id)` looks only in the player's **in-progress** list (`+0x11f4`, `CQuest` with `wID` at `+8`), a quest waiting for a reward choice
included. Finished quests are a separate list (`QuestFindFinish`); they don't count.

## 2. `CMob::ItemDrop(killer, party)` 0x4a5ea0: the order

`killer` is the killer of record (the player passed in, may be none), `party` the killer's party or union (may be none). In this order:

1. **The quest item** (section 3), at most one.
2. The drop entries 1 to 9 (built, `docs/original/combat.md` section 7).
3. In a ranking battle zone (map `CreateType` 5) a monster with `wMoney` 0 and `wMoneyAdd` above 0 gives guild battle points and **the
   function returns** (built, `docs/original/guild-ranking.md`).
4. **The sequence item** (section 4), when `wMoney` (`Money1`) is not 0.
5. Gold (built).

Every item goes through `CMob::ItemDrop(item, count 1, party, sequence flag)` 0x4a65a0 (section 3.2 for quest items, 4.3 for sequence
items; the rest as built).

## 3. The quest item

Confirmed, 0x4a5f3b to 0x4a60be.

### 3.1 When it drops

Nothing when the monster's `byQuestItemType` is 0 or the item has no definition. Otherwise, with `chance` = the item's `wQuality[0]`:

- **A quest-starting item** (`bQuestStart`): **`rand() % 100 < chance`** → it drops. No killer, party or quest is looked at (with no
  killer the item has no owner).
- **Any other item**: only with a killer. The link's **ended** list (1.3) is walked in order; for each quest id, the killer — or, **in a
  party, each member of the killer's party** (every member slot, wherever the member is: no map or distance check) — is asked
  `QuestFind(id)`. At the **first** quest some member (or the killer alone) has in progress: **one** roll `rand() % 100 < chance` →
  drops or not, and the search ends either way. No such quest: nothing.

So there is at most one roll, and whether it happens only depends on "the killer or any party member has one of the item's quests in
progress". The roll does not look at how many of the item the player already holds.

### 3.2 The item (`CMob::ItemDrop` 0x4a65a0 with an item of real type 27)

Made as every monster drop: `CItem::Init(type, type id, 1, now, 'D')` (quality = the definition's, make type **`'D'`**), a new unique id,
`CreateCraftExpansion` (quest items are not craftable: empty name). Then:

- **owner = the killer** (whatever the loot rule; real type 27 is checked after the rule-4 owner choice and overrides it);
- **party id = the killer's party id** (0 without one);
- **no loot rule**: real type 27 skips the whole distribution, so the item always goes **on the ground** (`CZone::EnterItem` kind 2,
  `0x0401` to the view, 60 s, owner-only for the first 30 s with a party share, as built).

### 3.3 Picking it up (`CZone::GetPickItem` 0x4174a0)

With the pick check on (`g_bChkItemPick` 0x541af8, on by default; the new server always checks), after the owner check (error 0) and
the distance check (error 4): an item that is **not gold**, of **real type 27** and **not a quest-starting item** needs the picker to have
**one of the link's ended quests in progress** (`QuestFind` on the picker), else **error 2** (`0x0205` `00 02`: the client prints 407
"Unavailable quest item") and the item stays on the ground. A quest item with no ended list can never be picked up. Quest-starting items
can be picked up by anyone. Nothing else changes in `CUser::ItemGet` (no quest packet follows a pick-up: the end check reads the bags).

## 4. Sequence items

Confirmed.

### 4.1 The file (`CItemDrop::InitSequenceItemCreate` 0x4be520, `Data/SequenceItemCreate.ini`)

`GetPrivateProfileInt` reads: `[SEQUENCE_GROUP_COUNT] COUNT` (0 or less: no sequence items; above 8: 8 and a log line); for each group
`n` = 1 to the count, `[SEQUENCE_GROUP_n] SEQUENCE` and `GRADE_0` to `GRADE_9` (missing keys are 0). **A group with `SEQUENCE` 0 turns
every group off** (the count becomes 0). Into `CItemDrop::m_pSequenceItem[8]` (`stSequenceItemCreate`: `u32 sequence, u32 grade[10]`).

Shipped: `COUNT=7` (the file has an eighth section, `SEQUENCE=2357` with all grades 0, which is never read):

| group | `SEQUENCE` | grades for levels 1–9, 10–19, …, 60–69 (70–99: 0) |
|---|---|---|
| 1 | 30,000,001 | 125, 125, 39, 39, 39, 39, 39 |
| 2 | 22,500,001 | 125, 125, 389, 389, 389, 389, 389 |
| 3 | 15,500,001 | 37, 37, 388, 388, 388, 388, 388 |
| 4 | 3,500,001 | 390 × 7 |
| 5 | 1,500,001 | 36, 36, 156, 186, 41, 305, 305 |
| 6 | 1,000,001 | 36, 36, 34, 34, 34, 34, 34 |
| 7 | 16,001 | 394 × 7 (Weapon / Armor Lapisia, 95/1 and 95/6) |

### 4.2 Which monster (`CItemDrop::GetSequenceItem(level, id)` 0x4be490)

Only for a monster with **`wMoney` > 0** (1,436 of 2,747 definitions). With the monster's **`wLevel`** (1 to 99; 0 or 100 and above give
nothing) and its **object id** (`CMob` `+0x5c`, `m_dwID`): the groups in order; the **first** with `id % SEQUENCE == 0` (id not 0) decides:
its grade for `level / 10`; grade 0 → nothing (no later group is tried); else `CItemDrop::GetItem(grade)` (a random item of the grade,
section 5) → it drops. No roll: the id alone decides.

### 4.3 The monster id counter (`CObjectMgr`)

`CObjectMgr::CreateMob` 0x40a5e0 gives every new monster (spawns, respawns, the GM's and the Lua scripts' summons, instance zones alike)
**`InterlockedIncrement(m_dwMobID)`** as its id. The counter **survives restarts**: `CObjectMgr::InitMobID` 0x40a340 reads
`.\data\MobID.ini` `[INIT_MOBID_SET] MOB_ID` (writes `1` when the file is missing), and `CObjectMgr::SaveMobID` 0x40a3c0 writes it back
**every hour** (`CReConnector::OnUpdate` 0x405010, first an hour after the start, `GetTickCount() + 3,600,000`) and at
`ServerShutdown` 0x403a10. After a crash the ids since the last save are given out again.

So group 7 makes a Lapisia from every 16,001st monster made (grade 394 for levels 1 to 69), group 6 every 1,000,001st, and so on.

### 4.4 The item (`CMob::ItemDrop` 0x4a65a0 with the sequence flag)

As any monster drop (owner, loot rule and party distribution as built), except the **make type `'V'`** (0x56) instead of `'D'`, and a
game log record **`0x0A06`** (127 bytes: the item's unique id, the monster id and name, its object id, the map, its position, the item and
the killer) to the game log (built, [The game log](game-log.md)).

## 5. Server-wide drop limits

Confirmed; facts only.

`CGameData::LoadItemLimit` reads `usp_Read_ItemCount_ByServer`, then `CItemDrop::Init` 0x4bde50 lists each grade's unlimited items first, then
the limited ones; `CItemDrop::InitLimit` 0x4be3e0 gives each item with **`wDrop` 1 to 6** a period of 24 min, 4 h, 8 h, 12 h, 24 h or 72 h
(by `wDrop`): `dwDropDelay` = period − period / 10, `dwDropRand` = period / 10. `CItemDrop::GetItem` 0x4bdd10 picks a random item of the
grade; for a limited one it gives **nothing** while `now < dwNextDropTime` or once `nNumTimeDrop ≥ wDrop`, else counts the drop and sets
the next time to `now + dwDropDelay + rand() % dwDropRand`.

Shipped: only **5** items have `wDrop` (25/175 to 25/179, `wDrop` 1, grades 810 to 814), and **no monster entry, box set or sequence group
uses grades 810 to 814**. Nothing can reach them, so the built rule ("a limited item never drops") gives the same game. Left as it is.

## 6. The new server

- **Data**: `tools/world/import.py` stores the links of 1.3 in **`world.item_quest_links`** (`item_type`, `item_type_id`, `started
  smallint[]`, `ended smallint[]`, file order) and `SequenceItemCreate.ini` as read in **`world.sequence_groups`** (`group_no`, `sequence
  bigint`, `grades integer[10]`, for groups 1 to the file's `COUNT`, missing keys 0); the game server applies 4.1's rules (8 at most, a 0
  sequence turns all off). `defs.mobs.questitemid` is mapped as 1.1 at load (`MobDef::quest_item`).
- **Quests in progress at the zone**: the drop is decided in the zone pass, which does not hold the players' quest books. `Zones` keeps,
  per online character, the ids of its quests in progress; each connection sets them after loading its quests and after every change
  (start, end, give-up, failure, expiry, the GM's `F80B`/`F80C`) and clears them when it leaves the world.
- **The drop** (`drops::roll`): the quest item first (3.1), then the entries, then the sequence item (4.2) when `money` > 0, then gold. The
  zone's kill code treats any item of real type 27 as 3.2 (owner the killer of record, the killer's party id, on the ground, no loot rule).
  A sequence item is an ordinary drop with make type `'V'`.
- **Pick-up** (`Zones::pick_up`): 3.3's check with the picker's quests in progress, error 2 (`PICK_NO_QUEST`).
- **Monster ids**: the counter (`Zones::last_mob_id`) starts from the saved value and is saved every hour and at shutdown, in
  **`game_servers.mob_id`** (migration 0024, default 1, as a missing `MobID.ini`), the database's place for a game server's own numbers.
- Small choices when building: the quests in progress are first set when the connection builds its world state (before the character is
  in any zone) and then after each change of the list (`server.rs` `sync_quests`); a quest item in the loot is made even when the
  killer is gone from the map (no owner then, and `has_quest` is false, so only a quest-starting item can drop); a sequence item is
  logged at debug level (`sequence item`); the monster id counter is saved by `game_server::server::save_mob_ids` every
  `MOB_ID_SAVE_EVERY` (60 s) and once after the connections close.
- Not built: `ReSequenceItemCreate`, the limits of section 5 (unreachable).

## 7. Deviations (both approved; both built)

1. **Only party members in the killer's zone count for a quest drop** (the original asks every member, wherever it is). A member on another
   map makes an item drop that only the killer's party may take and that it can't use unless it has the quest too, so the drop is usually
   wasted. With this, a member elsewhere no longer makes the item drop for the others.
2. **The monster id counter is also saved every minute** (the original: every hour and at shutdown), so a crash gives out at most a
   minute's ids again instead of an hour's (repeated ids repeat sequence items). One small `UPDATE` a minute.
