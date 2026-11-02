# The game log

This page covers the original's game log: every record the EP4.5 game server `ps_game.exe` (built 2009-07-15) sends to the game log
server, and what `ps_gameLog.exe` makes of each one in `PS_GameLog.dbo.ActionLog` and `PS_ChatLog.dbo.ChatLog`. It was read in the Ghidra
decompiler and the disassembly with the PDB names and the PDB's types (the 89 `TP_GAMELOG_*` structs), together with the stored
procedures of the original `PS_GameLog` database. Nothing was executed.

Read in `ps_game.exe`: every reference to `g_pClientToGameLog` (0x546f4c; 170, in 92 functions; [the list](#4-the-senders)),
`CUser::SetGameLogMain` 0x45b0d0, `CUser::SendChatLogA` 0x470150 and `SendChatLogB` 0x470210 with their 18 callers in `CUser::PacketChatE`
0x470490 and `PacketAdminChat`, `CMob::SendLogBothMob` 0x4a1810 and its callers' test, the two `CUser::ItemGet` (0x45d2e0, 0x45df40),
`CWorld::AddKillCount` 0x413600, `CUser::PacketMaininterface` (`0x030A`), `CUser::PacketPC` (`0x0305`), `CUser::EnterWorld` /
`LeaveWorld`, `CGuild::NpcUpgrade`, `CGuild::SendRankingZone`; the `m_byDropType` stores (`CItem +0x78`). In `ps_gameLog.exe`:
`CConnection::OnRecv` 0x4011f0, `PacketSystem` 0x4012d0, `OnDispatchDB` 0x401600, `OnDBWrite` 0x401670, `GetGameLogMainQuery` 0x401830,
`WriteLogChar` 0x4018c0, `WriteLogItem` 0x401f20, `WriteLogQuest` 0x403740, `WriteLogMoney` 0x403870, `WriteLogChat` 0x4039e0,
`WriteLogKeep` 0x403c10, `WriteLogGuild` 0x404590, `WriteLogAdmin` 0x404820, `WriteLogGamble` 0x404a40, `WriteLogPvp` 0x404c50,
`WriteLogParty` 0x404d80, `WriteLogItemMake` 0x401800, `ConvTimeToString` 0x407370, the name tables at 0x442fd0 and 0x443090. Procedures:
`usp_Insert_Action_Log_E`, `usp_Insert_ItemMake_Log_E`, `uxp_CreateTable`.

## In short

- **The game server sends one record for almost everything that matters**: entering and leaving the world, deaths and kills, level-ups,
  skills and stats, every item gained, lost, bought, sold, traded, stored, linked or auctioned, gold, quests, guilds, duels, parties,
  staff commands, obelisks, boss fights, the gambling house, and every chat line.
- **The log server writes them to two databases**: one row per record in `ActionLog` (an action number, the character, the place and
  time, ten number columns and four text columns), and the chat in `ChatLog`.
- **Nothing ever deletes rows.**
- **Items are followed by their unique id**: every item record names it, so an item can be traced from where it was made.
- **Some records are lost**: the whole item-make group is dropped by the log server, and two guild records (a member expelled, a member
  deleted) fail on a mistake in the log server's own query text.
- **No answer comes back**, so the game never waits for the log.

Status words: **confirmed** = read in the code (address given). Action types are decimal.

## How it was read

What the original writes to its game log: every record `ps_game.exe` sends to the game log server, what `ps_gameLog.exe` makes of each one
(`PS_GameLog.dbo.ActionLog` and `PS_ChatLog.dbo.ChatLog`), and what the new server does instead. Static analysis of `ps_game.exe` (the same
build as the other analysis documents) and `ps_gameLog.exe` (sha256 `e9b78c75…`, the same file in `PSM_Client/Bin` and `GameMain/PS_GameLog`):
Ghidra pseudocode through REA, the disassembly with PDB names (`tools/disasm.py`, `docs/original/symbols/ps_game.tsv`, `docs/original/symbols/ps_gameLog.tsv`)
and the PDB's types (`tools/pdb_types.py`, the 89 `TP_GAMELOG_*` structs). The stored procedures' text from the restored `PS_GameLog` backup
(`docs/original/database-notes.md`). Nothing was executed.

Read: `ps_game.exe`: every reference to `g_pClientToGameLog` (0x546f4c; 170, in 92 functions; the list is section 4), `CUser::SetGameLogMain`
0x45b0d0, `CUser::SendChatLogA` 0x470150 and `SendChatLogB` 0x470210 with their 18 callers in `CUser::PacketChatE` 0x470490 and
`PacketAdminChat`, `CMob::SendLogBothMob` 0x4a1810 and its callers' test, the two `CUser::ItemGet` (0x45d2e0, 0x45df40), `CWorld::AddKillCount`
0x413600, `CUser::PacketMaininterface` (`0x030A`), `CUser::PacketPC` (`0x0305`), `CUser::EnterWorld` / `LeaveWorld` (their tests),
`CGuild::NpcUpgrade`, `CGuild::SendRankingZone`; the `m_byDropType` stores (`CItem +0x78`). `ps_gameLog.exe`: `CConnection::OnRecv` 0x4011f0,
`PacketSystem` 0x4012d0, `OnDispatchDB` 0x401600, `OnDBWrite` 0x401670, `GetGameLogMainQuery` 0x401830, `WriteLogChar` 0x4018c0, `WriteLogItem`
0x401f20, `WriteLogQuest` 0x403740, `WriteLogMoney` 0x403870, `WriteLogChat` 0x4039e0, `WriteLogKeep` 0x403c10, `WriteLogGuild` 0x404590,
`WriteLogAdmin` 0x404820, `WriteLogGamble` 0x404a40, `WriteLogPvp` 0x404c50, `WriteLogParty` 0x404d80, `WriteLogItemMake` 0x401800,
`ConvTimeToString` 0x407370, the name tables at 0x442fd0 and 0x443090. Procedures: `usp_Insert_Action_Log_E`, `usp_Insert_ItemMake_Log_E`,
`uxp_CreateTable`.

**Not covered** (section 10): the user questions' records (they go to another connection; the mini-games' records are built, `docs/original/mini-game.md`)
two money checks at world entry and exit, and how the original's reconnecting sender treats records while the log server is down.

## 1. The path

Confirmed.

`ps_game` holds one connection to the log server, `g_pClientToGameLog` (`CClientToGameLog`, an `SConnectionTBaseReconnect`). On connect it sends
**`0x0101`** (35 bytes, `TP_GAMELOG_CONNECT`: `u8 byDBID, char szName[32]`); every record is a packet whose first `u16` is its type, sent with
`SConnectionTBaseReconnect::Send` as it happens. No answer comes back.

`ps_gameLog` (`CConnection::OnRecv`): group `0x01xx` is handled at once (`PacketSystem`: `0x0101` picks the three database contexts of that
DB id, `+0x120` game log, `+0x124` chat log, `+0x128` a third, and closes the connection when one is missing; `0x0102` closes it); groups
`0x02xx`, `0x03xx`, `0x04xx`, `0x05xx`, `0x08xx` to `0x0Fxx` are queued and written by the database thread (`OnDispatchDB` → `OnDBWrite`);
any other group is logged ("Error Packet") and dropped. `OnDBWrite` dispatches by group, each only while its context exists:

| group | writer | database | |
|---|---|---|---|
| `0x02xx` | — | — | **dropped**: `OnDBWrite` returns for the group; `WriteLogItemMake` (`0x0201` only → `DBWrite::LogItemMake`, `usp_Insert_ItemMake_Log_E`) is never called |
| `0x03xx` | `WriteLogChar` | game log | |
| `0x04xx` | `WriteLogItem` | game log | |
| `0x05xx` | `WriteLogQuest` | game log | |
| `0x08xx` | `WriteLogMoney` | game log | |
| `0x09xx` | `WriteLogChat` | chat log | `usp_Insert_Chat_Log_E` |
| `0x0Axx` | `WriteLogKeep` | game log | |
| `0x0Bxx` | `WriteLogGuild` | game log | |
| `0x0Cxx` | `WriteLogAdmin` | game log | |
| `0x0Dxx` | `WriteLogGamble` | game log | |
| `0x0Exx` | `WriteLogPvp` | game log | |
| `0x0Fxx` | `WriteLogParty` | game log | |

Each writer builds one `EXEC usp_Insert_Action_Log_E …` text with `sprintf` (no quoting except in the chat writer, section 3.5) and runs it;
an error is logged (`DBWrite::LogGame: err=%d, query=%s`) and the record is lost. An unknown type inside a known group writes nothing.

`usp_Insert_Action_Log_E` inserts one row into **`PS_GameLog.dbo.ActionLog`**:

| column | type | |
|---|---|---|
| `row` | int identity | |
| `UserID` | varchar(18) | the account name |
| `UserUID` | int | the account id |
| `CharID` | int | |
| `CharName` | varchar(50) | |
| `CharLevel` | tinyint | |
| `CharExp` | int | |
| `MapID` | smallint | |
| `PosX`, `PosY`, `PosZ` | real | |
| `ActionTime` | datetime | |
| `ActionType` | tinyint | the record kind (section 3) |
| `Value1` | bigint | |
| `Value2`, `Value3` | int | |
| `Value4` | bigint | |
| `Value5` … `Value10` | int | |
| `Text1` … `Text4` | varchar(100) | |

(`ActionTypeDefs` in the same database names each type and its value columns; the backup has no rows.) `uxp_CreateTable` makes per-day
tables, but the insert always goes to `dbo.ActionLog`. Nothing ever deletes rows.

## 2. The common parts

Confirmed.

**The header** (`TP_GAMELOG_MAIN`, 74 bytes, filled by `CUser::SetGameLogMain`): `u16 wType; u32 dwUserID` (`CUser::m_dwBillingID` +0x578c, the
account id); `char szUserName[19]` (`m_szUserName` +0x5794); `u32 dwCharID` (+0x128); `char szCharName[21]` (+0x188); `u16 wLevel` (+0x136);
`u32 dwExp` (+0x13c); `u16 wMap` (+0x164); `float fPosX, fPosY, fPosZ` (+0xd0); `u32 dwTime` (`GetSystemTime`, the packed time). The
writer turns it into `UserID, UserUID, CharID, CharName, CharLevel, CharExp, MapID, PosX, PosY, PosZ, ActionTime`, the time through
`ConvTimeToString` (`YYYY-MM-DD hh:mm:ss` from the packed `u32`: seconds bits 0–5, minutes 6–11, hours 12–16, day 17–21, month 22–25, year
26–31 + 2000). "With the header" below means these columns are filled; the records without it say which columns they fill.

**The item block** (`TP_GAMELOG_ITEM`, after the header): `u64 qwUniqueID` (the item's `ItemUID`), `u32 dwItemID`, `char szItemName[32]`,
`u8 byGem[6]`, `u32 dwCreateTime` (packed), `char szCraftName[21]`. Every item record writes **`Value1` = the unique id, `Value2` = the item id,
`Text1` = the item name**, and uses two texts:

- **G** = `"%d,%d,%d,%d,%d,%d (Option:%s)"`: the six gem bytes and the craft name (the writer cuts the craft name at 20 characters);
- **C** = the item's making time through `ConvTimeToString`.

Below, "item" means V1, V2 and T1 as said; V*n* and T*n* are `Value`*n* and `Text`*n*; unlisted columns are NULL.

**The stats order.** `CUser` keeps the base stats as str, dex, int, wis, rec, luc (+0x170 … +0x17a); the texts label them `s,d,i,w,r,l` in
that order. (The new server's `stats` array is str, dex, rec, int, wis, luc: write them by meaning.)

## 3. The records

ActionTypes are decimal. "Sent by" is the `ps_game` function (section 4 lists every call site); a record the original never sends is marked.

### 3.1 Character (`0x03xx`, `WriteLogChar`; all with the header)

| type | action | sent by | values and texts |
|---|---|---|---|
| `0x0301` | 102 | `CUser::Death` (by a monster, two places), `DeathByNPC` | V1 monster id, V2 skill id (0 a normal blow); T1 the monster's name |
| `0x0302` | 103 | `CUser::KillPCTo` (the killer's record) | V1 the victim's char id, V2 skill id; T1 the victim's name; T2 `"kill=%d, death=%d, win=%d, lose=%d"` (`dwKillCount[4]`, the killer's counters) |
| `0x0303` | 104 | **never sent** | as `0x0302` |
| `0x0304` | 105 | `CUser::Rebirth` | nothing more |
| `0x0305` | 106 | the ways back to life | **V1 = 0 and V2 1 to 6**: no values, T1 = the way: 1 `sel_base` (`0x0553` kind 2), 2 `sel_party` (kind 3), 3 `sel_itemsmall` (kind 4), 4 `sel_itembig` (kind 5) — all four from `CUser::PacketPC` — 5 `timeover` (`UpdateRebirth`, the timer), 6 `logout` (`KillSoulForDisconnect`, leaving while dead). **Otherwise** (`CUser::LifeSkill`, revived by a player's skill): V1 the caster's char id, V2 the skill id, T1 the caster's name |
| `0x0306` | 107 | `CUser::EnterWorld` (always) | V1 billing type, V2 billing time; T1 the client's IP (`inet_ntoa`); T2 `"money=%u, bank=%u, cash=%d"`; T3 `"level=%d,skp=%d,stp=%d"`; T4 `"s=%d,d=%d,i=%d,w=%d,r=%d,l=%d"` (base stats) |
| `0x0307` | 108 | `CUser::LeaveWorld` (always) | T2 `"money=%u, bank=%u, cash=%d, firstmoney=%u"` (first money: the money at entry); T3 `"level=%d,skp=%d,stp=%d"`; T4 the stats as above (T1 NULL) |
| `0x0308` | 146 | `CUser::LevelUp` | nothing more (the header carries the new level) |
| `0x0309` | 141 | `CUser::LearnSkill` | V1 skill id, V2 skill level, V3 the skill points left; T1 the skill's name |
| `0x030A` | 151–156 | `CUser::PacketMaininterface` (`0x0208`), one record per stat raised | action 151 str, 152 dex, 153 int, 154 wis, 155 rec, 156 luc (157 for any other type); V1 the stat's new value, V2 the points put in |
| `0x030B` | 195 | `CUser::StatusKillResult` | V1 the step (`byStep`), V2 the result (`dwResult`) of the kill-count reward steps |
| `0x030C` | 196 | `CUser::StatusDeathResult` | the same for the death steps |
| `0x030D` | 230 | `CUser::EnterWorld`, with no auto character (`m_dwAutoCharID` 0) and stored offline money of 10000 or more (`CWorld::LoadOffCharMoney`) | V1 money, V2 old money (not built: section 10) |
| `0x030E` | 231 | `CUser::LeaveWorld` (a money check, not read) | V1 money, V2 first money (not built: section 10) |
| `0x030F` | 232 | `CUser::EnterWorld` when `m_nMovePositionType` ≤ 3 | T1 `'Enter'` |
| `0x0310` | 232 | `CUser::LeaveWorld` when `m_nMovePositionType` ≤ 3 | T1 `'Leave'` |

### 3.2 Items (`0x04xx`, `WriteLogItem`; all with the header and the item block)

| type | action | sent by | values and texts (besides V1, V2, T1) |
|---|---|---|---|
| `0x0401` | 111 | `CUser::ItemGet` (both overloads: from the ground, by drop type, section 3.12), `ItemRemake` (the new item) | V3 count; T2 by `byGetType`: 1 `'ItemCreate'`, 2 `'Remake'` (`ItemRemake`), else NULL (`ItemGet` sends 0; no sender of 1 was found); T3 G; T4 C |
| `0x0402` | 112 | `ItemDrop` (three places), `ItemRemove`, `ItemRemoveFree`, and the consumed items of `ItemGemAdd`, `ItemGemRemoveAll/Pos`, `ItemLapisianAdd`, `ItemRemake` | T2 G; T3 C; T4 `"bag=%d,slot=%d,Count=%d"` |
| `0x0403` | 113 | `CUser::PacketShop` (buy, three places) | V3 NPC id, V4 count, V5 price, V6 money after; T3 G; T4 C |
| `0x0404` | 114 | `CUser::PacketShop` (sell, two places) | V3 NPC id, V4 count, V5 price, V6 money after; T2 G; T3 C; T4 `"bag=%d,slot=%d"` |
| `0x0405` | 115 | `CUser::ExchangeItemAdd` (the receiving side), `PacketMyShop` (two places, a personal-shop sale; which side each is was not read) | V3 the other's char id, V4 count, V5 money; T2 the other's name; T3 G; T4 C |
| `0x0406` | 116 | `CUser::ExchangeItemAdd` — the giving side | V3 the other's char id, V4 count, V5 money; T2 the other's name; T3 `"G-C"`; T4 `"bag=%d,slot=%d"` |
| `0x0407` | 117 | `CUser::ItemDropByUserDeath`, `ItemDropByMobDeath` | V3 the killer's id; T2 the killer's name; T3 `"G-C"`; T4 `"bag=%d,slot=%d"` |
| `0x0408` | 118 | `CUser::ItemGet` (from the ground, an item dropped at a death: `m_byDropType` 3) | V3 the dead player's char id (`CItem +0x7c`); T2 its name (looked up online, empty if gone); T3 G; T4 C |
| `0x0409` | 119 | `CUser::ItemGemAdd` (two places) | V3 lapis item id, V4 lapis unique id, V5 NPC id, V6 money, V7 success; T2 the lapis's name; T3 G; T4 C |
| `0x040A` | 120 | `CUser::ItemGemRemoveAll`, `ItemGemRemovePos` (two each) | the same for the lapis taken out |
| `0x040B` | 121 | `CUser::ItemBagToBank` (two), `ItemBankToBag` | V3 count, V4 bag, V5 slot, V7 warehouse slot; T3 G; T4 C |
| `0x040C` | 122 | `CUser::ItemBagToBank`, `ItemBankToBag` | V3 count, V4 warehouse slot, V6 bag, V7 slot, V8 the fee; T3 G; T4 C |
| `0x040D` | 113 | `CUser::PacketBilling` (billing storage to bag) | V3 NPC id, V4 count; T2 `'Billing'`; T3 G; T4 C |
| `0x040E` | 112 | `CUser::ItemUseNSend` (an item used up) | T2 `'use_item'`; T3 `"G-C"`; T4 `"bag=%d,slot=%d"` |
| `0x040F` | 116 | `CUser::ExchangePvP` (duel stakes handed over) | as `0x0406`, T3 `"pvp-G-C"` |
| `0x0410` | 113 | `CUser::PacketPoint` (item mall purchase) | V4 count; T2 `'PointItem'`; T3 G; T4 C |
| `0x0411` | 113 | `CUser::PacketPoint` (item mall storage to bag) | V4 count; T2 `'StoredPointItem'`; T3 G; T4 C |
| `0x0412` | 113 | `CUser::ItemCloakAdd` (a cloak badge) | V4 count, V5 money, V6 money after; T2 `'Cloak'`; T3 G; T4 C |
| `0x0413` | 200 | `CGuild::ItemBagToGuildBank` (two), `ItemGuildBankToBag` (the swap) | V3 count, V4 bank slot, V5 bag, V6 slot, V7 guild id; T2 the guild's name; T3 G; T4 C |
| `0x0414` | 201 | `CGuild::ItemGuildBankToBag` | V3 count, V4 bank slot, V5 bag, V6 slot, V7 guild id; T2 the guild's name; T3 G; T4 C |
| `0x0415` | 112 | `CUser::PacketGuild` (an Etin item handed in) | T2 `'etin_return'`; T3 `"G-C"`; T4 `"bag=%d,slot=%d"` |
| `0x0416`, `0x0417` | 206, 207 | **never sent** | |
| `0x0418`, `0x0419` | 208, 209 | **never sent** (item box actions, `"WRAgent ACK[Result=%d]"`) | |
| `0x0426` | 212 | `CUser::ItemLapisianAdd` (two places) | as `0x0409` for the lapisian |
| `0x0427` | 213 | `CUser::ItemCompose` | V3 the compose item's id, V4 its unique id; T2 the old G (`"… (Option:<old craft name>)"`); T3 the new G; T4 C |
| `0x0428` | 214 | `CUser::ItemConsider` | T2 the gems with `(Option:)`; T3 G; T4 C |
| `0x0429` | 215 | `CUser::ItemRemake` | T2 `"%I64d,%I64d,%I64d"` (the three items used up, by unique id); T3 G; T4 C |
| `0x042A` | 216 | `CUser::PacketMarket` (register, two places) | V3 market id, V4 bag, V5 slot, V6 count, V7 minimum bid, V8 buy-now price, V9 deposit, V10 money after; T2 G; T3 C; T4 `"Term=%dH, EndDate=%s"` |
| `0x042B` | 217 | `CMarketMgr::UnRegisterItem` (cancel) | V3 market id, V4 the bidder's char id; T2 G; T3 C; T4 the bidder's name |
| `0x042C` | 218 | `CMarketMgr::DirectBuy` (buy now) | V3 market id, V4 price, V5 money after, V6 the seller's char id; T2 G; T3 C; T4 the seller's name |
| `0x042D` | 219 | `CMarketMgr::Tender` (a bid) | V3 market id, V4 the bid, V5 money after, V6 the seller's char id, V7 the outbid char id; T2 G; T3 C; T4 the seller's name |
| `0x042E` | 220 | `CUser::MarketItemGet` (collect an item) | V3 market id, V4 bag, V5 slot, V6 count; T2 G; T3 C |
| `0x042F` | 221 | `CMarketMgr::CharGetMoney` (collect money; header only, no item block) | V1 0, V2 0, V3 market id, V4 money id, V5 money, V6 deposit, V7 returned, V8 money after |

### 3.3 Quests (`0x05xx`, `WriteLogQuest`; with the header)

| type | action | sent by | |
|---|---|---|---|
| `0x0501` | 131 | `CUser::QuestStart` | V1 quest id, V2 NPC id; T1 the quest's name, T2 the NPC's name |
| `0x0502` | 133 | `CUser::QuestEnd`, `QuestSuccessResult`, `QuestRemoveAdmin` | V1 quest id, V2 success, V3 money, V4 reward item id, V5 count; T1 the quest's name, T2 the item's name |

### 3.4 Money (`0x08xx`, `WriteLogMoney`; with the header)

| type | action | sent by | |
|---|---|---|---|
| `0x0801` | 161 | `CUser::ItemGet` (gold from the ground, **always**, whatever its drop type; both overloads), `ItemGetMoney` | V1 the amount, V2 money after |
| `0x0802` | 162 | **never sent** | |
| `0x0803` | 163 | `CUser::Exchange` (the receiving side, two places) | V1 amount, V2 money after, V3 the other's char id; T1 the other's name, T2 the other's IP |
| `0x0804` | 164 | `CUser::Exchange` (the giving side), `PacketMaininterface` (one place, not read) | V1 amount, V2 money after, V3 the other's char id; T1 the other's name |
| `0x0805` | 164 | `CUser::ExchangePvP` (duel stakes) | as `0x0804`, T3 `'pvp'` |

### 3.5 Chat (`0x09xx`, `WriteLogChat`, `usp_Insert_Chat_Log_E` into the chat log)

`SendChatLogA(type, len)` sends `0x0901` (`TP_GAMELOG_CHAT_A`: `u32 dwUserID, u32 dwCharID, u16 wMapID, u8 byChatType, u8 byMsgLen, char
szMsg[128]`); `SendChatLogB` sends `0x0902` with `char szCharName[21]` (the whisper's target) before the length. No header. The writer cuts
the text at 127 bytes, **adds a space at the end**, replaces every `'` with `_` (0x403b46), and inserts `UserUID, CharID, ChatType, TargetName (NULL
for A), ChatData, MapID, ChatTime` (ChatTime: the log server's clock at the write). Chat types, from the call sites:

| chat type | `0x11xx` (`PacketChatE`) | staff `0xF1xx` (`PacketAdminChat`) |
|---|---|---|
| 1 normal | `0x1101` | yes |
| 2 whisper (B, with the target) | `0x1102` | yes (two places) |
| 3 guild | `0x1104` | yes |
| 4 party | `0x1105`, `0x1112` | yes (two places) |
| 5 to all | `0x1101` with a chat item, `0x1103` (trade), `0x1108` | yes |
| 6 shout | `0x1107` | — |
| 7 zone | `0x1111` | — |

### 3.6 Keep, monsters and the market (`0x0Axx`, `WriteLogKeep`)

| type | action | sent by | |
|---|---|---|---|
| `0x0A01` | 170 | `CWorld::AddKillCount`, `AddKillCountGrade`: a country's kill count **reached its top grade** (the event that resets the other side) | no header (all user columns empty, map and position 0); V1 the country |
| `0x0A02` | 171 | `CMap::ObeliskRespawn` | no user columns; MapID and position; V1 the country; T1 the obelisk monster's name |
| `0x0A03` | 172 | `CMap::ObeliskDestroy` | the same, V2 the next country |
| `0x0A04` | 173 | `CMob::SendLogBothMob`, from the monster's actions, **only for a monster with a Lua AI** (`m_bLuaAI`) **or of a boss group** (`m_pRespawn->bBothMob`, `Obelisk.ini`) | no user columns; MapID and position of the monster; V1 its HP, V2 the damage, V3 monster id; T1 its name, T2 the action (1 `attack`, 2 `damage`, 3 `heal`, 4 `death`, 5 `debuffadd`, 6 `debuffremove`, 7 `itemdrop`, 8 `moneydrop`; 0 or above 8 `none`), T3 the player's name, T4 the skill's name |
| `0x0A05` | 174 | `CUser::PacketUserDBChar` (a rename) | `TP_GAMELOG_CHARRENAME`: UserID, UserUID, CharID and CharName (the old name) only; V1 char id; T1 the old name, T2 the new name |
| `0x0A06` | 198 | `CMob::ItemDrop` (a sequence item dropped, `TP_GAMELOG_SEQUENCEITEM_DROP`) | CharID and CharName of the one it fell for, MapID and position; V1 unique id, V2 item id, V3 monster id, V4 `dwID` (the drop's id); T1 the item's name, T2 the monster's name, T3 the making time |
| `0x0A07` | 199 | **never sent** (`GuildPvPWin`/`Loss` are never called, `docs/original/guild-war.md`) | |
| `0x0A08` | 222 | `CMarketMgr::DirectBuy` (the seller's record of a sale) | CharID, CharName only; V1 unique id, V2 item id, V3 market id, V4 money id, V5 money, V6 deposit, V7 returned, V8 the buyer's char id; T1 item name, T2 G, T3 C, T4 the buyer's name |
| `0x0A09` | 223 | `CMarketMgr::CheckMarketItemEndDate`, `RetrievalItem` (two): unsold, or recalled by a GM | V1, V2 item, V3 market id, V4 the bidder's char id; T1, T2 G, T3 C, T4 `'TimeOut'` or `"GM(<gm name>)"` |
| `0x0A0A` | 224 | `CMarketMgr::Tender` (the seller's record of a bid) | V3 market id, V4 the bidder's char id; T1, T2 G, T3 C, T4 the bidder's name |
| `0x0A0B` | 225 | `CMarketMgr::CheckMarketItemEndDate` (won by the highest bid) | V3 market id, V4 the seller's char id; T1, T2 G, T3 C, T4 the seller's name |
| `0x0A0C` | 226 | `CMarketMgr::DirectBuy`, `Tender`, `UnRegisterItem`, `RetrievalItem` (a bidder outbid or refunded) | V3 market id, V4 money id, V5 the reason, V6 the seller's char id; T1, T2 G, T3 C, T4 the seller's name |

### 3.7 Guilds (`0x0Bxx`, `WriteLogGuild`; with the header; V1 guild id, T1 its name)

| type | action | sent by | |
|---|---|---|---|
| `0x0B01` | 175 | `CGuildMgr::CreateGuild` | |
| `0x0B02` | 176 | `CGuild::Fire` (disbanded) | |
| `0x0B03` | 177 | `CGuild::JoinRequest` | |
| `0x0B04` | 178 | `CGuild::CreateAddUser`, `JoinOK`, `JoinUser` | |
| `0x0B05` | 179 | `CGuild::JoinLeave` | V2 0 |
| `0x0B06` | 179 | `CGuild::JoinExpel` (two) | V2 1 — **never stored**: the text has 11 values before the texts, one too many for the procedure, so the `EXEC` fails |
| `0x0B07` | 179 | `CGuild::DeleteChar` (a member deleted) | V2 2 — **never stored**, the same fault |
| `0x0B08` | 202 | **never sent** | |
| `0x0B09` | 203 | `CGuild::NpcUpgrade` | V2 the NPC type word, V3 its new level, V4 the guild's Etin after, V5 the keep Etin; T2 `"<kind> <shop type>"`: the low byte names the kind (0 `NpcInit`, 1 `Shop`, 2 `GateKeeper`, 3 `Smith`, 4 `Priest`, 5 `Gamble`, 6 `Warehouse`, 7 `Story`, 8 `Guard`, 9 `Animal`), the high byte the shop type (0 empty, 1 `Weapon`, 2 `Defence`, 3 `General`, 4 `Potion`, 5 `Job Product`, 6 `Vehicle`, 7 `Cloak`, 8 `Emblem`, 9 `Bank`) |
| `0x0B0A` | 205 | `CUser::PacketGuild` (Etin handed in) | V2 the guild's Etin, V3 the Etin returned |
| `0x0B0B` | 204 | `CGuild::SendRankingZone` (battle points) | V2 the points got, V3 the battle total, V4 the total |

### 3.8 Staff (`0x0Cxx`, `WriteLogAdmin`, `CUser::SendLogAdmin`)

`TP_GAMELOG_ADMIN_CMDTA` (`u32 dwUserID, char szUserName[16], u32 dwCharID, char szCharName[21], u16 wMap, float pos[3], u8
byAdminLevel, char szCmdName[32]`), `CMDTB` adds `char szTargetName[21]`, `CMDTC` also `char szCmdInfo[100]`. Action **180** for all three:
UserID, UserUID, CharID, CharName, MapID, position (level and exp 0), ActionTime the log server's clock; V1 the staff level; T1 the command's
name; T2 the target (`0x0C02`, `0x0C03`); T3 the detail (`0x0C03`).

### 3.9 Mini-games (`0x0Dxx`, `WriteLogGamble`)

`CMiniGame::GameEnter` `0x0D01` (190), `GameExit` `0x0D02` (191), `SendGameLogChgMoney` `0x0D03`, `SendGameLogSlotRun` `0x0D04`,
`SendGameLogCardRun` `0x0D05` (194). Built with the gambling house (`docs/original/mini-game.md` section 7; `game_log::gamble_*`, rows with level and
experience 0, the map and position only on `0x0D01`).

### 3.10 Duels (`0x0Exx`, `WriteLogPvp`; with the header; V1 the opponent's char id, T1 its name)

| type | action | sent by | |
|---|---|---|---|
| `0x0E01` | 123 | `CUser::PvPChkLoseDeath` (the winner's record) | |
| `0x0E02` | 124 | `CUser::PvPChkLoseDeath` (the loser's) | |
| `0x0E03` | 125 | `PvPChkLeaveByAttack`, `ByDeath`, `ByMove`, `ByZoneLeave`, `PacketPVP` (two each: both players) | V2 the cancel reason (`byCancle`) |

### 3.11 Parties (`0x0Fxx`, `WriteLogParty`; with the header)

`0x0F01` (210): `CParty::EnterNormalParty`, `EnterRaidParty`; `0x0F02` (211): `BreakParty`, `LeaveNormalParty`, `LeaveRaidParty`,
`ExpelUser`, `ExpelRaidPartyUser`. V1 the party's index.

### 3.12 What items get a pick-up record

Confirmed, `CItem::m_byDropType` +0x78.

`CUser::ItemGet` logs a ground pick-up by the item's drop type: **1** (`CMob::ItemDrop`: every monster drop, items and gold) and **2**
(`CUser::ItemDrop`: a player's drop) → `0x0401`; **3** (`ItemDropByUser`, `ItemDropByUserDeath`, `ItemDropByMobDeath`, `ItemDropPvP`: dropped at a
death) → `0x0408`; 0 → none. Gold (type 26) always writes `0x0801`. `0x0205` (item made, group `0x02xx`) is sent at every pick-up and
creation and never stored (section 1).

## 4. The senders

Confirmed.

Every function of `ps_game.exe` that sends to `g_pClientToGameLog`, with its types: `CGuild::CreateAddUser` 0B04, `DeleteChar` 0B07, `Fire` 0B02,
`ItemBagToGuildBank` 0413 ×2, `ItemGuildBankToBag` 0413 0414, `JoinExpel` 0B06 ×2, `JoinLeave` 0B05, `JoinOK` 0B04, `JoinRequest` 0B03, `JoinUser`
0B04, `NpcUpgrade` 0B09, `SendRankingZone` 0B0B; `CGuildMgr::CreateGuild` 0B01; `CMap::ObeliskDestroy` 0A03, `ObeliskRespawn` 0A02;
`CMarketMgr::CharGetMoney` 042F, `CheckMarketItemEndDate` 0A09 0A0B 0A0C, `DirectBuy` 0A0C 0A08 042C, `RetrievalItem` 0A0C 0A09 ×2 …,
`Tender` 0A0C 0A0A 042D, `UnRegisterItem` 0A0C 042B; `CMiniGame::*` 0D01–0D05; `CMob::ItemDrop` 0A06, `SendLogBothMob` 0A04; `CParty::*` 0F01
0F02; `CUser::Death` 0301 ×2, `DeathByNPC` 0301, `EnterWorld` 0306 030D 030F, `Exchange` 0803 0804 ×2, `ExchangeItemAdd` 0405 0406,
`ExchangePvP` 0805 040F, `ItemBagToBank` 040B ×2 040C, `ItemBankToBag` 040B 040C, `ItemCloakAdd` 0412, `ItemCompose` 0427, `ItemConsider` 0428,
`ItemCreate` 0205, `ItemDrop` 0402 ×3, `ItemDropByMobDeath` 0407, `ItemDropByUserDeath` 0407, `ItemGemAdd` 0409 ×2 0402, `ItemGemRemoveAll` 040A ×2
0402, `ItemGemRemovePos` 040A ×2 0402, `ItemGet` 0801 0401 0205 0408 …, `ItemGetMoney` 0801, `ItemLapisianAdd` 0426 ×2 0402, `ItemRemake` 0402 0401
0429, `ItemRemove` 0402, `ItemRemoveFree` 0402, `ItemUseNSend` 040E, `KillPCTo` 0302, `KillSoulForDisconnect` 0305, `LearnSkill` 0309,
`LeaveWorld` 0307 030E 0310, `LevelUp` 0308, `LifeSkill` 0305, `MarketItemGet` 042E, `PacketBilling` 040D, `PacketGuild` 0415 0B0A,
`PacketMaininterface` 030A ×6 0804, `PacketMarket` 042A ×2, `PacketMyShop` 0405 ×2, `PacketPC` 0305 ×4, `PacketPVP` 0E03 ×2, `PacketPoint` 0410
0411, `PacketShop` 0404 ×2 0403 ×3, `PacketUserDBChar` 0A05, `PvPChkLeaveBy*` 0E03 ×2 each, `PvPChkLoseDeath` 0E01 0E02, `QuestEnd` 0502,
`QuestRemoveAdmin` 0502, `QuestStart` 0501, `QuestSuccessResult` 0502, `Rebirth` 0304 ×2, `SendChatLogA` 0901, `SendChatLogB` 0902,
`SendLogAdmin` 0C01 0C02 0C03, `StatusDeathResult` 030C, `StatusKillResult` 030B, `UpdateRebirth` 0305; `CWorld::AddKillCount` 0A01,
`AddKillCountGrade` 0A01; and the connection itself (`CClientToGameLog::SendConnect` 0101, `SendDisconnect`, `CReConnector::OnUpdate`,
`InitServer`, `CServer::CServer`).

**A correction**: `docs/original/gm-commands-2.md` says the user questions write game-log records `0x0401` to `0x0403`. They go to
`g_pClientToLog` (0x546f48, a `CLogConnection`, not the game log): `CUserQuestionMgr::AddQuestion` 0401, `AddAnswer` 0402, `AddAnswerSend` 0403.
In the game log those types are item records.

## 5. The new server now

There is no game log. What exists: `tracing` lines at `info` for staff commands (`server/admin.rs` `log`: "gm command") and market actions
(`server/market.rs` `log`), the item mall's `point_log` table (`docs/original/item-mall.md`), the user questions' own table
(`docs/original/gm-commands-2.md`), and the database rows themselves (`character_items` keeps `uid`, `made_at`, `make_type`). In memory an item
(`ItemUnit`, `crates/shaiya-proto`) has no unique id or making time: those live only in the database rows (and in `Kept.origin` while an item
is on the ground or leaves whole). The account name is not in memory (`Claimed` has none). A ground item knows only `from_mob`, not the
original's three drop types or who dropped it.

## 6. Deviations

1. **No log server; the game server writes PostgreSQL itself.** A table `game_log` with the `ActionLog` columns (section 1; every value
   column `bigint`, as unsigned 32-bit values do not fit `int`) and a table `chat_log` with the `ChatLog` columns. The same ActionType
   numbers and the same placement of values and texts as section 3, so whoever knows the original's log reads this one. Rows are written
   by a background task in batches; the game never waits for them. When the database is slow or down the queue (bounded) fills and later
   records are dropped with a warning (the original also loses a record whose `EXEC` fails). Text is passed as parameters: names and chat
   keep their `'` (the original's chat writer turns them into `_`; the other writers do not escape at all), and chat gets no added space.
   ActionTime is the moment the record is made (`timestamptz`), for staff and chat too (the original uses its log server's clock there).
   Both tables get a **`server_id`** column: the original keeps one set of log databases per game server (the `byDBID` of `0x0101`), here
   the servers share the tables. *Why not a separate log program as the original*: the original split every service into its own process
   (2004-era, the log database often on another machine) so that the heavy log writes never slowed the game; an in-process background
   writer with a queue gives the same isolation without a second protocol and service. Approved.
2. **The chat log**: the original stores every chat line of types 1 to 7 and staff chat (section 3.5), whispers included. Built, switched
   by `GAME_CHAT_LOG`, **on by default** (as the original). Approved.
3. **The two guild records the original never stores** (`0x0B06` expelled, `0x0B07` a member deleted; section 3.7): **stored** (V2 1 and
   2), since the fault is a typo in the log server's text, not a choice. Approved.
4. **Item unique ids and making times come from the database writes.** The item records' V1 (unique id) and C (making time) need what
   memory does not have (section 5). The item writes (`ItemStore::trade`, `guild_bank`, `exchange`, `drop_item`, `take_stakes`, and the
   market and mall stores) return the `uid` and `made_at` of every row they insert, change, move or delete (`RETURNING`), and each item
   record is written after its write with those, so **every item record names the row as stored**. The aim is to follow items for
   duplication: a stack split or a new item is a new `uid` with its own record, a whole item changing hands keeps its `uid`, and the
   database already refuses two rows with one `uid` (`character_items.uid UNIQUE`), so a duplicate shows as a `uid` (or a count) that
   appears without the record that made it. (The other two options: ids only where memory has them, most records without — rejected, it
   cannot follow items; ids carried in memory for every item — the most complete but the largest change, every place that makes an item.)
   Chosen: the option that tracks duplication.
5. **Retention**: the original never deletes rows. A setting `GAME_LOG_DAYS` (default 0 = keep forever, as the original); when set, a
   daily pass deletes `game_log` and `chat_log` rows older than that many days. Approved.

Not deviations (the original's behaviour, built as it is): the records the original never sends stay unsent (`0x0303`, `0x0416` to
`0x0419`, `0x0802`, `0x0A07`, `0x0B08`); `0x0205` and the item-make log are not written (section 1); `0x0A04` is written for Lua-AI monsters only until the boss groups of `Obelisk.ini` exist (they belong to the
obelisk boss work); gold pick-ups always log `0x0801`, item pick-ups only by drop type (section 3.12).

## 7. Build plan

The plan the code was written from; section 11 records where the code differs from it, and the code is authoritative. All in
`crates/game-server` unless said. The steps are in order.

1. **Migration `db/migrations/0029_game_log.sql`**: `game_log (id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, server_id smallint
   NOT NULL, user_name varchar(32) NOT NULL DEFAULT '', user_id integer NOT NULL DEFAULT 0, character_id integer NOT NULL DEFAULT 0,
   character_name varchar(50) NOT NULL DEFAULT '', level smallint NOT NULL DEFAULT 0, exp bigint NOT NULL DEFAULT 0, map smallint NOT NULL
   DEFAULT 0, x real NOT NULL DEFAULT 0, y real NOT NULL DEFAULT 0, z real NOT NULL DEFAULT 0, at timestamptz NOT NULL, action smallint NOT
   NULL, value1 … value10 bigint, text1 … text4 varchar(100))` with indexes `(character_id, at)`, `(action, at)`, `(at)` and **`(value1)
   WHERE value1 IS NOT NULL`** (to follow a `uid`); `chat_log (id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, server_id smallint NOT
   NULL, user_id integer NOT NULL, character_id integer NOT NULL, chat_type smallint NOT NULL, target_name varchar(50), text varchar(200)
   NOT NULL, map smallint NOT NULL, at timestamptz NOT NULL)` with `(character_id, at)` and `(at)`. **No foreign keys** (the log outlives
   deleted characters). Comment the tables as the other migrations do (after `ActionLog` / `ChatLog`, this document).
2. **`game_log.rs`** (new module, `pub mod game_log;` in `lib.rs`):
   - `pub struct Header { user_id: u32, user_name: String, character_id: u32, character_name: String, level: u16, exp: u32, map: u16,
     position: [f32; 3] }` with `Header::of(character: &PlayCharacter, map, position)` (the account name from step 4) and `Header::none()`
     (the records without user columns, `0x0A01` to `0x0A04`), `Header::map_only(map, position)`.
   - `pub struct ItemPart { uid: Option<i64>, item_id: u32, name: String, gems: [u8; 6], craft_name: Vec<u8>, made_at: Option<i64> }`; `fn
     gems_text(&self) -> String` = G (section 2; craft name cut at 20, `from_utf8_lossy`); `fn made_text(&self) -> Option<String>` = C
     (`YYYY-MM-DD hh:mm:ss`, local time as the server's other packed times, from microseconds).
   - `pub struct Row { header: Header, at: SystemTime, action: u8, values: [Option<i64>; 10], texts: [Option<String>; 4] }` and `pub struct
     ChatRow { user_id, character_id, chat_type: u8, target: Option<String>, text: String, map: u16, at: SystemTime }`; `pub enum Entry {
     Action(Row), Chat(ChatRow) }`.
   - **One constructor per record of section 3** (`pub fn death_by_mob(h, mob_id, skill, mob_name) -> Row`, `pub fn item_get(h, item,
     count) -> Row`, `pub fn shop_buy(...)`, … named after the record, with the action numbers and placements of section 3 as constants and
     doc comments citing the subsection). Texts longer than 100 bytes are cut (the column).
   - `#[derive(Clone)] pub struct GameLog { tx: Option<mpsc::Sender<Entry>>, server_id: u8, chat: bool, dropped: Arc<AtomicU64> }`
     (the writer stamps `server_id` from `GAME_SERVER_ID`/the config on every row): `GameLog::off()`;
     `GameLog::memory(chat) -> (GameLog, Arc<Mutex<Vec<Entry>>>)` for tests (a task-free sink: an enum inside, not a channel); `pub fn
     action(&self, row: Row)` and `pub fn chat(&self, row: ChatRow)` (nothing when off, or chat off): `try_send`, on a full queue count the
     drop and warn at most once a minute with the count. `GameLog::spawn(pool: PgPool, config) -> GameLog`: channel of 10000; a task that
     takes up to 500 entries at a time (`recv_many`) and writes them with `pg::write_game_log`; an error is a `warn` with the count lost.
     When `GAME_LOG_DAYS` > 0 the same task runs `pg::purge_game_log(days)` at start and every 24 h.
   - Unit tests: each constructor's placement against section 3 (one test per subsection is enough, checking every column of two or three
     records each, plus G, C, the stat order of `0x0306`/`0x0307`, the `0x0305` ways, the `0x030A` actions, the `0x0B09` names, the
     `0x0A04` actions); the queue drops when full and counts it; chat off sends no chat rows.
3. **`pg.rs`**: `pub async fn write_game_log(pool, entries: &[Entry]) -> Result<()>`: one multi-row `INSERT` per table with
   `sqlx::QueryBuilder::push_values` (chunks of 500); `pub async fn purge_game_log(pool, days) -> Result<u64>`. A PostgreSQL test in
   `tests/postgres.rs` (skips without `DATABASE_URL` as the others): write two action rows and one chat row, read them back. **`config.rs`**:
   `GAME_LOG` (bool, default true), `GAME_CHAT_LOG` (bool, default per deviation 2), `GAME_LOG_DAYS` (u32, default 0); add them to the README
   settings table. **`Context`** (`server.rs`): `pub game_log: GameLog`, `GameLog::off()` in `Context::new`; `main.rs` sets
   `GameLog::spawn(pool.clone(), &config)` when `GAME_LOG` is on, before the `Arc`.
4. **What memory lacks** (deviation 4 first):
   - **The rows' ids from the writes.** `backend.rs`: `pub struct Stored { pub uid: i64, pub made_at: i64 }` (microseconds, as
     `Kept.origin`). `ItemStore::trade` and `guild_bank` return `Result<Option<Vec<Option<Stored>>>>` (`None`: refused, nothing changed;
     otherwise one entry per `SlotChange`, in order: the row it inserted, changed, moved or deleted, `None` for a change that touches no
     row); `exchange` the same per `Handover` (the row the receiver ends up with: the moved row, or the new copy for part of a stack, plus
     the giver's row when it remains: use `Vec<(Option<Stored>, Option<Stored>)>`, receiver then giver); `drop_item` and `take_stakes`
     already return `Kept.origin` for whole items — make part of a stack return the giver's row too (`Kept.origin` of the copy stays
     `None` until it lands; its new row's id comes from the pick-up's `Insert`). In `pg.rs` add `RETURNING uid, (extract(epoch FROM made_at)
     * 1000000)::int8` to `insert_item`, each `UPDATE`/`DELETE` of `apply_change`, `apply_bank_change`, `apply_guild_bank_change` and the
     handover statements (a `Swap` returns the row now at `a`, the other at `b` is not needed). The memory backend gives new items ids from
     a counter and `made_at` now, and keeps them per slot so changes return the same ids. Every caller that only checked the boolean uses
     `.is_some()`; the market (`MarketStore`) and the mall (`PointStore::buy`, `take_stored`) return the ids of the rows they create or take
     the same way. A PostgreSQL test: an insert, a split, a move to the warehouse and a delete return the ids the rows have.
   - the account name: `PlayCharacter.account_name: String`, loaded in `pg::load` (join `accounts`), "" in the memory
   backend. Ground items: replace `GroundItem.from_mob: bool` by `drop_type: u8` (1 monster, 2 a player's drop, 3 dropped at a death; keep a
   `fn from_mob(&self) -> bool { self.drop_type == 1 }` for the gold rule) and add `dropped_by: u32` (the dead player's id for type 3), set
   where items go on the ground (`CMob::ItemDrop` → monsters' drops; `drop_item`; the death drops in `server/pvp.rs` `lose_item` and the
   mob-death drops). `ItemPart.uid` / `made_at` come from the write's `Stored` (or `Kept.origin`); NULL only where no row exists (gold, and the records of
   section 3 that carry no item).
5. **The records, by section** — call `ctx.game_log.action(game_log::…)` where the new server does what the original's sender does. The
   build reads each place before adding the call; where a value has no source in the new server, write NULL and note it in the "Built"
   section.
   - 3.1: death by a monster (where a monster's hit kills a player: `mob_skills::hurt_player*` → `pvp::player_died` with a monster killer,
     and the plain monster blows), player kills (`pvp::player_died` for the killer, with its counters after), `rebirth` / `come_back` /
     `rune_rebirth` / `revived_by_item` / the timer / `leave_dead` / revived by a skill (the `0x0305` ways), `in_world` (`0x0306`, then
     `0x030F` 'Enter'), the logout end (`finish_logout` / `linger_end`: `0x0307` with the money at entry kept in the world state, then `0x0310`
     'Leave'), `award_exp` level ups (one `0x0308` per level gained, after the level changes), `learn_skill`, `spend_stat_points` (one
     `0x030A` per stat with points put in), the kill/death reward steps (`pvp::next_steps` users: `0x030B`/`0x030C`).
   - 3.2: `pick_up` (by drop type, section 3.12; gold `0x0801`), `drop_item` (`0x0402`), `buy` / `sell` (`0x0403` / `0x0404`, the NPC id from
     the shop), `warehouse_move` (`0x040B` / `0x040C`), the trade's hand-over (`trade_exchange`: `0x0405`/`0x0406` per item for both sides,
     `0x0803`/`0x0804` for money; T2 of `0x0803` the other's IP if the connection has it), personal shops (`server/my_shop.rs`: `0x0405`;
     the original sends two, sides not read: write one for the buyer and one for the seller, and say so in "Built"), duel stakes (`server/duel.rs`: `0x040F`, `0x0805`), the death drops (`0x0407`), lapis (`server/lapis.rs`: `0x0409`, `0x040A`,
     `0x0426`, `0x0427`, `0x0428`, `0x0429`, and `0x0402` for each consumed lapis / hammer / item as the original), items used up
     (`server/items.rs`, `0x040E`), the mall (`server/mall.rs`: `0x0410` purchase, `0x0411` storage take, `0x040D` billing storage), cloak
     badges (`server/lapis.rs` `0x0803` handler: `0x0412`), the guild bank (`0x0413` / `0x0414`), Etin hand-in (`0x0415` for the item,
     `0x0B0A`), staff item removal (`0x0402`), the market (`server/market.rs`: `0x042A` to `0x042F` and `0x0A08` to `0x0A0C` beside its
     existing `log`).
   - 3.3: `quest_start` (`0x0501`), `finish_quest` / `fail_quest` / the staff quest removal (`0x0502`).
   - 3.5: the chat handler (`0x11xx`, and `0x1108` armed by a chat item) and staff chat (`server/admin.rs` `gm_chat`): `ctx.game_log.chat`
     with the types of section 3.5; whispers with the target's name.
   - 3.6: `kc.rs` top-grade event (`0x0A01`, the country), obelisk captures and resets (`zone/obelisk.rs`: `0x0A02` / `0x0A03`), Lua-AI
     monsters (`zone/lua.rs`, `zone/scripted.rs`, `fight.rs`: `0x0A04` for attack, damage taken, heal, death, debuff added/removed, item and gold
     drops — only for monsters with a Lua AI), renames (`0x0A05`), sequence drops (`drops.rs` users, `0x0A06`).
   - 3.7: `guild.rs` / `zone/guild.rs`: create, disband, join request, join (all three ways), leave, expel and member deleted (deviation 3),
     house NPC upgrades (`0x0B09`), battle points (`add_battle_points`, `0x0B0B`).
   - 3.8: every staff command (`server/admin.rs` `log` already sits at each): `0x0C01` without a target, `0x0C02` with one, `0x0C03` with a
     detail; T1 the command's name as the original's `szCmdName` (use the command's opcode name, e.g. `"FA0A"`, where the original's text is
     unknown, and say so in "Built").
   - 3.10: duels (`duel.rs`, `server/duel.rs`): win/lose (`0x0E01`/`0x0E02`), cancels (`0x0E03` for both, with the reason).
   - 3.11: `party.rs` users: join (`0x0F01`), leave, kick, break-up (`0x0F02`), with the party id.
6. **Tests** beside the code they touch (one or two per subsection, through `GameLog::memory`): a pick-up of a monster drop writes
   `0x0401` with action 111 and count; a pick-up of a death drop writes 118 with the dead player's id; gold writes 161 with the amount and
   money after; a shop buy writes 113 with NPC, count, price and money; a level-up writes 146 per level; raising two stats writes two
   `0x030A` rows (actions 151 and 155 for str and rec); a whisper writes a chat row of type 2 with the target and the text as typed (a `'`
   kept); chat off writes none; a guild expel writes action 179 with V2 1; a duel win writes 123 and 124; a staff command writes 180.
   Duplication tracking (deviation 4): a bought item's `0x0403` names the `uid` its row got; a whole item traded keeps one `uid` in the
   giver's `0x0406` and the receiver's `0x0405`; part of a stack traded or dropped gives the new row a new `uid` (and the giver's row keeps
   its own); a warehouse move keeps the `uid`.
7. **Run** `~/.cargo/bin/cargo test --workspace` and `cargo clippy --workspace --all-targets` (no new warnings), commit, build release,
   restart the game server (`tools/game-server.sh restart >/dev/null 2>&1`; the migration runs at start).
8. **Play test, section 48** of `docs/testing/live-test-checklist.md`: one or two clients
   doing ordinary things — pick up a monster drop and gold, buy and sell at a shop, put an item in the warehouse and take it out, trade an
   item and gold, learn a skill, raise a stat, level up, die to a monster and come back, chat normal, whisper, party and trade, a staff
   command, log out. Each item says what row to expect; **the rows are checked in the database afterwards** (`psql … "SELECT action, value1 … FROM game_log ORDER BY id DESC LIMIT 50"`),
   so the items are things to do in the client and the check is a query.
9. **Docs**: a "Built" section here (the commit, what has no source and stays NULL); README "Not built": drop "the game log records";
   the README settings table: `GAME_LOG`, `GAME_CHAT_LOG`, `GAME_LOG_DAYS`; a short "following an item" note in `docs/original/database-notes.md`
   (`SELECT … FROM game_log WHERE value1 = <uid> ORDER BY at`, and that `value3`/`value4` of the gem records hold the lapis's ids); `docs/original/gm-commands-2.md`: the
   correction of section 4 (the questions go to `g_pClientToLog`); a line in `docs/server/architecture.md` where the database writers are
   described; the "Not covered … game log" notes of the other analysis documents may stay (they point here by name).

## 8. Order inside one action

The original sends each record at its own point of the action (before or after the client's answer, varying by function). The new server
writes the record after the action has been applied (the state it records is the state after), except where section 3 names a value from
before (first money, the old craft name). The order of rows of one action follows the order of section 3's senders (e.g. `0x0306` before
`0x030F`, the giving side's `0x0406` before the receiving side's `0x0405`). No packet changes.

## 9. Client

Nothing of this reaches the client. No client questions.

## 10. Not covered

- **The user questions' records** (`0x0401` to `0x0403` on `g_pClientToLog`, section 4): another log connection, not read further; the new
  server keeps the questions in its own table already.
- **The mini-games' records** (`0x0Dxx`): built with the gambling house (`docs/original/mini-game.md` section 7).
- **`0x030D` and `0x030E`**: the money checks at entry (offline money, `LoadOffCharMoney`, with no auto character) and at exit (not read); the
  new server has neither offline money nor auto characters.
- **How `SConnectionTBaseReconnect::Send` treats records while the log server is down** (dropped or queued): not read; the new server
  drops when its queue is full (deviation 1).
- **`ActionTypeDefs`'s names and descriptions**: the backup has no rows; the names above come from the writers' formats and the structs.
- The boss groups' `0x0A04` records wait for the obelisk boss work (`docs/original/obelisks.md` section 11).

## 11. As built

Play test: **section 48** of `docs/testing/live-test-checklist.md`. The rows are checked in the database; the client has nothing to show.

**As specified.** Migration 0029; `game_log.rs` (a constructor per record of section 3, the queue of 10000, the writer task of 500 at a
time, the daily retention pass, `GameLog::flush` at shutdown so the queue is written before the process ends); `pg::write_game_log`
(`(pool, server_id, entries)`: the server id is a parameter, not a field of the rows), `purge_game_log`; `GAME_LOG`, `GAME_CHAT_LOG`,
`GAME_LOG_DAYS`; `PlayCharacter.account_id` and `account_name` (loaded with the character, empty in the memory backend); the item names
the texts carry come from `defs.items`, `defs.mobs` and `defs.skills` (`pg::load_game_log_names`), held by the `GameLog`.

**Deviation 4 (the item writes return the rows they touch).** Done with a few changes to the shapes of section 7 step 4:

- `ItemStore::trade` and `guild_bank` return `Option<Vec<Changed>>`, `Changed { row, swapped }`: the row each change inserted, changed, moved or
  deleted; a `Swap` names the row now at `a` in `row` and the one now at `b` in `swapped` (the guild bank's bag/bank swap needs both).
  `exchange` returns `Option<Vec<Handed>>` (`receiver`, `giver`); `drop_item` returns `(Kept, Stored)` and `take_stakes` a list of them (the
  giver's row: the deleted one for all of it, the one that keeps the rest for part of it); `PointStore::buy` gives `Bought::Done(points,
  Vec<Stored>)` and `take_stored` an `Option<Stored>`; `MarketWritten` has `ops` (the row each op touched: the auction a `NewAuction` made, the
  item an `EndAuction` moved, the auction a `SetBid` changed, the item row a `DeleteItemRow` deleted), `taken` and `placed`.
- **Added**: `QuestStore::start_quest` and `end_quest` return the rows too (a quest's start items and reward are new items).
- The memory backend keeps an id and a time per slot (`ItemIds`), so a row keeps its id through changes, moves, the warehouse and a drop and
  pick-up (`Kept::origin`); tests read them.
- `Stored` is also what a lapis work asks for: `server/lapis.rs` `save` returns the rows of its changes and, for the places a record must name
  that no change touches (a failed link leaves the item alone), adds a change that sets the count to what it is, so that the write returns the
  row.
- Ground items: `GroundItem.dropped: Dropped { kind, by }` (1 a monster's drop, 2 a player's, 3 a death's, with the player), carried by
  `ZoneEvent::Loot` too; `from_mob()` is the gold rule.

**Additions beyond section 3.** `0x0401` with T2 `'ItemCreate'` (action 111) for every item the server makes that no other record names: a box's
item, a quest's start items and reward, a staff member's `F702`, `F80B`'s start items (the original's `0x0205` is never stored, and this
way **a new unique id always has the record that made it**; section 3.2 says no sender of `byGetType` 1 was found). `ZoneEvent` lost its
`Copy` for `ZoneEvent::Record(Box<Row>)`: the guilds and the parties make their records without the character's numbers, and the member's
connection puts its header on (`Guilds` writes one directly, with the character's id, name and level, for a member out of the world).

**Choices at the points the build was left.**

- Personal shop (`0x0405`, sides not read): the buyer's record names the item it got (the new row) with the seller and the price paid; the
  seller's names the item it sold (its stack's row, or the one that moved) with the buyer and the gold it got. Both use `0x0405`.
- Staff commands: T1 is the opcode in hex (`"FA0A"`); every command is written as `0x0C03` with T2 NULL and T3 the detail text the server's
  own log line has (it names the target); V1 is the staff level at entering the world.
- The warehouse: `0x040B` into it (V3 count, V4 and V5 the bag place, V7 the warehouse slot; V6 NULL as in the table), `0x040C` out of it (V8 the
  fee); a move inside the warehouse is not logged (the original has no sender). The market's refund reasons (`0x0A0C` V5) are this server's
  numbers, the original's not having been read: 1 outbid, 2 cancelled, 3 bought directly, 4 recalled.
- The duel's cancel (`0x0E03`) comes from the zone as `ZoneEvent::DuelCancelled` with the other side's id and name, for both players; the
  win and the loss (`0x0E01`, `0x0E02`) as `DuelResult` now carries them.
- Whispers are logged when delivered (type 2, T target as the player is named); a refused whisper is not.

**Stays NULL (no source), or is not written.**

- `0x0306` V2 (billing time); `0x0306`/`0x0307` `bank=` is 0 (the warehouse holds no gold here) and `cash=` the account's points; `0x030F` and
  `0x0310` are written at every entering and leaving (the `m_nMovePositionType ≤ 3` test is not kept).
- `0x0301` V2 and `0x0302` V2 (the skill used: the zone's death event does not carry it); a death by a guard NPC (`KILLER_NPC`) is not
  logged.
- The NPC id of `0x0404` (the sell packet names none), `0x040D` (the billing take), `0x0409`, `0x040A`, `0x0426` (the blacksmith in view is not
  identified); `0x0803` T2 (the other's address); `0x0501` T2 (the NPC's name); `0x0A05` UserID (the account's name is not known on the
  character screen).
- `0x0A04`: T4 (the skill) for blows and attacks; a debuff's **removal** (`debuffremove`) is not logged; the monsters are those with a Lua AI and
  the boss groups' (Boss, follower, chest).
- `0x0A06`: V1 and T3 (the item's row is made when it is picked up: its `0x0401` names it) and V4 (the ground id) when the item goes straight
  into a party member's bags.
- Not built (section 10): `0x030D`, `0x030E`, the user questions' records. (The mini-games' `0x0Dxx` are built: `docs/original/mini-game.md`.)

**Also.** The flaky `a_boss_chain_in_an_instance_zone_opens_the_zones_pair` (it killed a hard-coded monster id that follows the visiting order of a
`HashMap`) looks the id up now. Tests: unit tests of every subsection of `game_log.rs`, the guild and kc ones, the integration tests of
`tests/game_flow.rs` (shop, pick-up, death drop, drop, warehouse, trade, chat, levels, stats, deaths and the ways back, quests, staff) and two
PostgreSQL tests (the writer and the retention pass; the rows the item writes return). A duel's win and loss records and the market's are
not exercised by an integration test (live test items instead).
