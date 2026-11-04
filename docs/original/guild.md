# Guilds

This page covers guilds in the EP4.5 game server `ps_game.exe`: creating a guild with its party's agreement, inviting,
join requests and their answers, leaving, kicking, ranks, the remark, disbanding with the officers' agreement, what happens at login,
logout, level-up and character deletion, the guild list, guild chat and the guild name in the looks. It was decompiled with Ghidra and
checked in the disassembly; the original database's procedures were read in the restored `PS_GameData`. The client side (what `game.exe`
does with each `0x0Dxx` packet, its dialogs, the result messages and the unit layouts) is on [Social packets](../client/client-social-packets.md).

On other pages: the guild house (`0x0D34`), Etin (`0x0D23`, `0x0D35`, keep Etin) and the house NPCs' upgrades (`0x0D24`, `0x0D25`) on
[Guild houses](guild-house.md); its bank (`0x0D29`, `0x0D31`, `0x0D32`) on [Guild bank](guild-bank.md); the ranking
battle and guild points (`0x0D26`, `0x0D27`, `0x0D36`, `0x0D37`) on [Guild ranking battle](guild-ranking.md); guild war
(`CUser::PacketPVPGuild`, `PvPChkLeaveByGuildLeave`, member status 3 and `0x0D19`) on [Guild wars](guild-war.md); the GM
commands (`UpdateUserLevelAdminCmd`) on [GM commands 1](gm-commands.md#86-guilds-fa13-fa14-fa15); the game logs (`0x0B01` to
`0x0B13` to the game log server) on [The game log](game-log.md).

## In short

- **Founding a guild takes a full party.** The founder must be level 11 or more, not in easy mode, and lead a party of exactly seven
  who are all free to join; every one of them must agree, and the founder pays 1000 gold.
- **Joining.** Officers (the master and rank 2) can invite a player of the same faction nearby, or accept a join request sent from the
  guild list. After joining, a player cannot found a guild or send a join request for 72 hours.
- **Ranks run from 1 (the master) to 9 (a new member).** At most seven members are officers. Officers can promote, demote and kick
  members below them. The master can only leave as the last member.
- **Disbanding** a guild of more than two members needs the master and at least two officers in a party to agree.
- **Faults in the original:** the founding and disbanding agreements wait forever and can point at players who have logged out; a
  deleted master can leave a guild without one.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (the PDB's `CUser` members after
`stUserInfo` are 4 bytes lower; see [NPC shops](shop.md)).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), decompiled with REA (Ghidra) and checked in the disassembly (`tools/disasm.py`); the
original database's procedures from the restored `PS_GameData` (`docs/original/database-notes.md`). Client side:
`docs/client/client-social-packets.md` (what `game.exe` does with each `0x0Dxx` packet, its dialogs, the result messages and the unit
layouts). `CUser` offsets are the code's (the PDB's `CUser` members after `stUserInfo` are 4 bytes lower; `docs/original/shop.md`).

Covered: creating a guild with its party's agreement, inviting, join requests and their answers, leaving, kicking, ranks, the remark,
disbanding with the officers' agreement, what happens at login, logout, level-up and character deletion, the guild list, guild chat and
the guild name in the looks. **Not covered** (a later part): the guild house (`0x0D34`), its bank (`0x0D29`, `0x0D31`, `0x0D32`), Etin
(`0x0D23`, `0x0D35`, keep Etin), guild NPC upgrades (`0x0D24`, `0x0D25`), the ranking battle and guild points (`0x0D26`, `0x0D27`,
`0x0D36`, `0x0D37`), guild war (`CUser::PacketPVPGuild`, `PvPChkLeaveByGuildLeave`, member status 3 and `0x0D19`), GM commands
(`UpdateUserLevelAdminCmd`) and game logs (`0x0B01` to `0x0B13` to the game log server). Built since, but for `0x0D19` and the game logs:
`docs/original/guild-house.md`, `docs/original/guild-ranking.md`, `docs/original/guild-bank.md`, `docs/original/guild-war.md` and
`docs/original/gm-commands.md` section 8.6 (`FA13` is `UpdateUserLevelAdminCmd`).

## 1. The state

Confirmed.

**The player** (`CUser`, code offsets; PDB names):

| offset | PDB name | |
|---|---|---|
| +0x1748 | `m_dwGuildID` | its guild's id, 0 without one |
| +0x174C | `m_nGuildUserLevel` | its rank (section 2) |
| +0x1750 | `m_dwGuildReqID` | the guild it asked to join (a pending join request), 0 without |
| +0x1754 | `m_dwGuildMasterReqID` | the id of the player who invited it (a pending invitation), 0 without |
| +0x1758 | `m_pGuild` | its `CGuild` |
| +0x175C | `m_pGuildCreate` | the creation or disband agreement it takes part in (`CGuildCreate`) |
| +0x54C0 | `m_dwGuildJoinRemainHour` | the end of its join penalty as a tick count, 0 without (section 10) |
| +0x5838 | `m_dwTimeOverGuild` | when the pending invitation lapses (`GetTickCount() + 30000`) |

**A guild** (`CGuild`, `CGuild::Reset` 0x42bcd0, `CGuildMgr::CreateGuild` 0x4321e0): id +0x08, name +0x0C (25 bytes), master's name +0x25
(21), the count of members of rank below 3 +0x3C, country +0x40, points +0x44, ranking +0x48 (31 = unranked), Etin +0x4C, guild house
flags +0x54/+0x55, remark +0x56 (65), seven slots for the **online** members of rank below 3 +0x4D8 (who get the join-request packets,
`CGuild::SendAdmin`), and three maps of member entries (`stGuildUserInfo`: id, name, job, level, rank, the `CUser*` when online): the
**online** members (+0x4F4, count +0x514), the **offline** members (+0x520, count +0x540) and the **join requests** (+0x54C).

`CGuildMgr` keeps every guild by id and by name (**the name map compares without case**, `SSyncMap<stStringi,…>`), every member's guild by
character id (+0x88), the largest guild id handed out (`TP_DB_GUILD_MAXID` at start; each new guild takes the next one with
`InterlockedIncrement`), and the creation and disband agreements in progress (`CGuildCreate`: kind +0x08, 1 creation / 0 disband; name
+0x09; the yes count +0x24; the requester's id +0x28 and `CUser*` +0x2C; seven member ids +0x30, cleared as each says yes; seven `CUser*`
+0x4C; remark +0x80).

## 2. Ranks

Confirmed.

`m_nGuildUserLevel` / `byGuildUserLevel`: **1 the master**, 2 to 9 the members, **9 a new member**; **0 a join request** (stored as a
guild member row of rank 0, section 12). "Officers" below means rank below 3 (the master and rank 2): they invite, kick, see and answer the
join requests and get `0x0D17`/`0x0D18`. At most seven members have rank below 3 (the slots at +0x4D8): a promotion to 2 needs fewer than
7 (section 7).

## 3. The packets

Confirmed, `CUser::PacketGuild` 0x4764a0.

The handler takes `0x0D03` to `0x0D0A`, `0x0D0C`, `0x0D20` to `0x0D23`, `0x0D25`, `0x0D34` and `0x0D35`; every other opcode of the
family is ignored, including the client's old creation `0x0D01`. Each case takes the guild lock first (`CGuildLock`). The layouts are in
the client notes; the ones the server sends are listed in section 13.

## 4. Creating a guild

Confirmed, case `0x0D21`, case `0x0D22`, `CGuildMgr::CreateGuild` 0x4321e0.

`0x0D21` `char szGuildName[25], char szRemark[65]` (the server cuts both at their last byte). Checked in this order; a failure answers
the **requester alone** with `0x0D21` (98 bytes: `u8 code`, `u32 0`, `u8 0`, the name and the remark as sent):

1. level below 11 → **4**;
2. easy mode (`m_byGrow` 0) → **5** ("restriction on growing mode");
3. already in a guild → **8**; in an agreement already → **3**;
4. not in a party, or the party does not have **exactly 7** members → **3** ("shortage of members");
5. each member: in a guild → **8**; easy mode → **5**; in an agreement, or not in the requester's zone (`CUser::EqualZone`) → **3**;
   its join penalty running (section 10) → **9**;
6. a guild with that name exists (any case) → **6**.

Then a `CGuildCreate` of kind 1 holds the name, the remark, the requester and the seven members (their `m_pGuildCreate` set), and **all
seven, the requester included**, get the agreement dialog `0x0D22` (96 bytes: `u32` requester id, name, remark; `SendGuildCreateAgree`
excludes id 0, so nobody). Each answer `0x0D22 u8` (1 yes, 0 no) from a member of a creation agreement, not in a guild:

- **no** → `0x0D01` (33 bytes: `u8 7`, then the name at offset 8; `SendGuildCreateFailed`) to all seven: "rejection of party members";
  the agreement ends (every member's `m_pGuildCreate` cleared);
- **yes** → the member's id is cleared from the list and the count goes up; at **7** yes, `CreateGuild`.

`CreateGuild`: the name taken meanwhile → `0x0D01 6` to all; the requester's money below **1000** → `0x0D01 2` to all. Otherwise:

1. a guild with the next id, the requester's country, the name, the requester's name as master's name, points 0, Etin 0, ranking 31, the
   remark; the requester **loses 1000 gold**; the database agent gets the guild (`TP_DB_GUILD_CREATE` with the account name) and each
   member (`TP_DB_GUILD_USERCREATE`, `CGuild::CreateAddUser` 0x42d3e0);
2. the requester becomes rank **1**, the six others rank **9**, all as online members;
3. the requester gets `0x0D21` (98: `u8 1`, `u32` guild id, `u8 1`, name, remark), `0x0213` (`u32` its money) and the member list
   (section 9); each other member `0x0D21` with rank **9** and the member list;
4. every player of that country gets `0x0D30` (the guild's `GUILD_UNIT`, points 0, `CWorld::SendAllCountry`).

The agreement has no time limit, and it is not ended when a member logs out or leaves the party (section 14).

## 5. Inviting

Confirmed, cases `0x0D05` and `0x0D06`, `CGuild::JoinUser` 0x42e7a0.

`0x0D05 u32 id` (the client's `/sign up for guild` on the targeted player). The inviter must be rank below 3; the target is looked up **in
the inviter's zone** (`CZone::FindUser`); it must not be the inviter, must have the inviter's country, and must have no guild, no
agreement, no pending invitation and no pending join request. Then the target's invitation is set (inviter's id, 30 s) and the target gets
`0x0D05 (inviter id)` (the dialog with the 3-day warning). **Any failure** (also when the inviter has no guild or is not an officer)
answers the inviter `0x0D06 (0, id)` "Guild rejected join-request".

`0x0D06 u8` (1 yes, 0 no), with an invitation pending: the inviter is looked up in the zone (gone → nothing but the invitation cleared).

- **yes**: the inviter still has a guild and rank below 3 → `JoinUser`, and the inviter gets `0x0D06 (1, inviter id)` "Guild accepted
  join-request";
- **no**: the inviter gets `0x0D06 (0, inviter id)`.

The invitation is cleared either way; unanswered, `CUser::UpdateTimeover` clears it after 30 s without a word. **The join penalty is not
checked for an invitation.**

`JoinUser`: the player becomes an online member of rank **9** (database: `TP_DB_GUILD_USERCREATE`), gets `0x0D08` (33: `u8 1`, `u32` guild
id, `u8 9`, the guild name); the guild's online members (the player included) get `0x0D14` (32: `u8 1` online, the player's
`GUILDUSER_UNIT`); the player gets the member list, the guild bank and the NPC levels (`SendBankItemList`, `SendNpcLvList`); its join
penalty starts (72 hours).

## 6. Join requests (verified, cases `0x0D07` and `0x0D08`, `CGuild::JoinRequest` 0x42e120, `JoinOK` 0x42e330, `JoinNO` 0x42e630,
`JoinRequestCancle` 0x42e040)

`0x0D07 u32 guildId` (the guild list's "join request"):

1. the join penalty running → `0x0D07 2` (the US client shows "Requested to join." for it);
2. easy mode, a guild, an agreement or a pending invitation → `0x0D07 0`; no such guild, or a guild of the other country → `0x0D07 0`;
3. a pending request to another guild is withdrawn first (`JoinRequestCancle`: removed, `TP_DB_GUILD_USERDELETE`, the officers get
   `0x0D18 (id)`);
4. `JoinRequest`: already asked this guild → `0x0D07 0`; otherwise a join-request entry (rank 0; database `TP_DB_GUILD_USERCREATE` with
   rank 0), the player's `m_dwGuildReqID` set, the online officers get `0x0D17` (30: `GUILDJOINUSER_UNIT`), the player `0x0D07 1`.

`0x0D08 u8 ok, u32 id` from an officer (rank below 3, in a guild):

- **accept** (`JoinOK`): the entry leaves the requests and becomes a member of rank 9 (database `TP_DB_GUILD_USERUPDATE`); the officers get
  `0x0D18 (id)`; the guild's online members get `0x0D14` (state 1 when the player is online in the world, 0 when not); an online player also
  gets `0x0D08 (1, guild id, 9, name)`, the member list, the bank and NPC levels, and its join penalty starts. An offline one finds itself in
  the guild at its next login;
- **refuse** (`JoinNO`): the entry is removed (database `TP_DB_GUILD_USERDELETE`); an online player gets `0x0D08 (0, guild id, 9, name)`
  "… guild rejected join-request" and its request cleared; the officers get `0x0D18 (id)`.

An id that is not a pending request is ignored. Requests are kept across logouts and restarts (they are database rows).

## 7. Ranks

Confirmed, case `0x0D0C`, `CGuild::ChangeGuildUserLevel` 0x42e9f0.

`0x0D0C u8 direction, u32 id`: **0 promotes** (rank − 1), **1 demotes** (rank + 1); anything else is ignored. The requester must be rank 1
or 2 and not the target; the target (online or offline member) must have a **higher rank number** than the requester; the new rank must
be **2 to 9**; a new rank of 2 needs fewer than 7 members below 3; **rank 2 cannot make another rank 2**. Then the rank is stored (database
`TP_DB_GUILD_USERUPDATE`), the officer slots follow, and the guild's online members get `0x0D0C (200 + rank, id)` "<b>'s authorization has
been changed". Refusals are silent. The master's rank never changes this way (nobody has a lower number).

## 8. Leaving and kicking

Confirmed, cases `0x0D09` and `0x0D0A`, `CGuild::JoinLeave` 0x42da90, `JoinExpel` 0x42dcb0.

`0x0D09` (not in an agreement): **the master can leave only as the last member**; otherwise `0x0D09 0` "Impossible to quit guild". Leaving:
the member is removed (database `TP_DB_GUILD_USERDELETE`), the player gets `0x0D09 1`, the guild's online members `0x0D0C (102, id)` "<b>
quit guild". A guild left with no member is disbanded (section 11).

`0x0D0A u32 id` from an officer (not in an agreement): the target, online or offline, must have a **higher rank number** than the
requester; otherwise `0x0D0A (0, id)` to the requester "Impossible to kick". Kicking: removed (database `TP_DB_GUILD_USERDELETE`); an online
target gets `0x0D0A (1, id)` "Kicked from guild" and loses its guild; the requester gets `0x0D0A (1, id)`; the guild's online members get
`0x0D0C (103, id)`.

## 9. The lists

Confirmed, `CGuild::SendGuildUserList` 0x42c980, `SendGuildJoinList` 0x42ce10, `CGuildMgr::SendGuildList` 0x432000.

- **Members**: `0x0D12` the online members, then `0x0D13` the offline ones (`u8 count`, `GUILDUSER_UNIT` × count, **at most 40 per
  packet**, more packets as needed); `0x0D19` would list the members in the guild war zone (not built). The player's own entry tells its
  client its rank.
- **Join requests** (to officers): `0x0D16` (`u8 count`, `GUILDJOINUSER_UNIT` × count, 40 per packet).
- **Guilds**: `0x0D0D`, then `0x0D2F` (`u8 count`, `GUILD_UNIT` × count) **14 per packet** with the guilds of the player's country, then
  `0x0D0E`.

`GUILD_UNIT` (120): `u32 id, char name[25], char master[21], char remark[65], u8 ranking, u32 points`. `GUILDUSER_UNIT` (29): `u32 id, u8
rank, u16 level, u8 job, char name[21]`. `GUILDJOINUSER_UNIT` (28): `u32 id, u16 level, u8 job, char name[21]`.

## 10. Login, logout, level-up, deletion, the join penalty

Confirmed.

- **Entering the world** (`CUser::PacketUserDBChar` case `0x407`): `CWorld::WaitUser` → `CUser::EnterWorld` sends **the guild list**
  (section 9) right after `0x0404`; then `CUser::InitGuild` → `CGuildMgr::ConnectUser` → `CGuild::ConnectUser` 0x42d1c0: a player with a
  pending join request gets its `m_dwGuildReqID` back; a member moves from offline to online (its guild, rank, officer slot) and **the other
  online members** get `0x0D0C (104, id)`; then `SendCharacterGuild`: the member list, and to an officer the join requests. All of it goes
  **before `0x0105`** in the burst (`docs/original/enter-world.md` section 3), whose guild name field carries the guild's name. The bank and
  NPC levels follow later in the burst (section 3 item 19).
- **Leaving the world** (`CUser::LeaveWorld` → `ReleaseGuild` → `CGuild::DisconnectUser` 0x42d300): online → offline, the remaining online
  members get `0x0D0C (105, id)`.
- **Level-up** (`CUser::LevelUp` → `CGuild::ChangeUserLevel` 0x42ebd0): the entry's level, and `0x0D0C (level, id)` to the online members
  (state below 100 = a level).
- **Deleting a character** (`CUser::PacketUserDBChar` → `CGuildMgr::DeleteCharGuildUser` → `CGuild::DeleteChar` 0x42edf0): a pending join
  request is withdrawn (`0x0D18` to the officers); a member is removed and the online members get `0x0D0C (102, id)`. **A deleted master**
  hands the guild to the offline member with the lowest rank number (`DeleteCharChangeMaster` 0x42ec60: rank 1, master's name, database
  update), only while fewer than 7 members are below rank 3.
- **The join penalty**: in memory `m_dwGuildJoinRemainHour` = now + 72 h when joining (sections 5 and 6); at login the database agent sends
  the hours left (`usp_Read_Char_GuildJoinDate_R`: 72 minus the hours since the character's **latest `GuildChars.JoinDate`**, which is also
  written for a join request). It blocks creating (code 9) and join requests (`0x0D07 2`), not invitations.
- **At start** (`CClientToDBAgent::PacketSystemConnect` → `RemoveGuildZeroUser` 0x4326d0): guilds with no member are disbanded.

## 11. Disbanding

Confirmed, cases `0x0D03` and `0x0D04`, `CGuildMgr::FireGuild` 0x4325e0, `CGuild::Fire` 0x42d5a0.

`0x0D03` from the master, not in an agreement:

- **2 members or fewer** (online and offline): disbanded at once;
- otherwise the master must be in a party with **at least 2 officers of the guild** in its zone (not in an agreement); fewer → `0x0D03 2`
  "insufficient party members". A `CGuildCreate` of kind 0 holds the master and those officers, and **all of them, the master included**, get
  `0x0D04 (master id)` "Guild master has requested guild dismiss". Answers `0x0D04 u8` (1 yes, 0 no) from an officer of that agreement:
  **no** → `0x0D03 3` "party member's rejection" to all and the agreement ends; **yes** → cleared from the list, the count goes up, and **more
  than 2** yes disbands.

Disbanding (`Fire`): the online members get `0x0D0C (101, 0)` "Guild has been disbanded", every member and request is removed and the
online ones lose their guild; the guild is dropped from the manager (database `TP_DB_GUILD_DELETE`: `Guilds.Del` 1, every `GuildChars` row
`Del` 1); every player of the country gets `0x0D11 (guild id)`. `0x0D03 1` "Disbanded guild" is never sent.

## 12. Guild chat and the looks

Confirmed.

- **Guild chat** `0x1104 u8 len, text` (`CUser::PacketChatE`, length 2 to 128 as every chat): in a guild, `0x1104` (`char name[21], u8
  len, text`) to **every online member, the speaker included** (`CGuild::SendAllGuildChat` 0x42c130).
- **The guild name** goes in `0x0105` (section 10) and in the looks `0x0303` (`CZone::SendUserShape`, `CUser::SendUserShape`,
  `CUser::GetGuildName`); **the guild id** in `0x0201`. Nothing is resent when a player's guild changes: the client of a member shows the
  name over the new member (`0x0D14`) and clears it (`0x0D15`, `0x0D0C 102/103`), the others see it when they next get the looks.

## 13. The original database

Confirmed, `PS_GameData`.

`Guilds` (`GuildID`, `GuildName`, `MasterUserID`, `MasterCharID`, `MasterName`, `Country`, `TotalCount`, `GuildPoint`, `Del`,
`CreateDate`, `DeleteDate`), `GuildDetails` (house, rank 31, Etin, keep Etin, `Remark`), `GuildChars` (`GuildID`, `CharID`, `GuildLevel`,
`Del`, `JoinDate`, `LeaveDate`). `usp_Create_GuildChar_E` refuses a character with a live row (one guild or one request at a time) and
writes `JoinDate`; `usp_Delete_GuildChar_E` sets `Del` and `LeaveDate`; `usp_Save_GuildChar_E` with rank 1 also makes the character the
guild's master; `usp_Create_Guild_E` refuses a name in use (after trimming spaces); `usp_Delete_Guild_E` closes the guild and its rows.

Server → client packets used here (sizes with the opcode): `0x0D01` 33, `0x0D03` 3, `0x0D04` 6, `0x0D05` 6, `0x0D06` 7, `0x0D07` 3,
`0x0D08` 33, `0x0D09` 3, `0x0D0A` 7, `0x0D0C` 7, `0x0D0D` 2, `0x0D0E` 2, `0x0D11` 6, `0x0D12`/`0x0D13` 3 + 29n, `0x0D14` 32, `0x0D16`
3 + 28n, `0x0D17` 30, `0x0D18` 6, `0x0D20` 68, `0x0D21` 98, `0x0D22` 96, `0x0D2F` 3 + 120n, `0x0D30` 122, `0x0213` 6, `0x1104` 24 + len.

## 13a. The remark

Confirmed, case `0x0D20`, `CGuild::UpdateRemark` 0x42efb0.

`0x0D20 char szRemark[65]` (cut at its last byte) from the **master**, without `'` in it: stored (`TP_DB_GUILD_UPDATEREMARK`), and the online
members get `0x0D20 (1, remark)`. Otherwise (not the master, or a `'`) the requester gets `0x0D20 (0, the remark it sent)`, which its client
shows as if it had been taken (the client ignores the result byte).

## 14. Faults in the original

- **A creation or disband agreement waits forever** for an answer, and holds `CUser*` pointers: a member who logs out leaves a dangling
  pointer in it (the next send to it reads freed memory), and the requester cannot start another until every member has answered.
- **A deleted master leaves the guild without one** when 7 members are already below rank 3, and the new master is chosen among the
  offline members only.
- **A refused remark looks taken** to the master's client (section 13a).
- **The join penalty starts with a join request** too (the database's latest `JoinDate`), but only after a relog: in memory it starts
  only with a join. An invitation ignores it altogether.
- **A guild left empty by deletions** stays until the next server start (`RemoveGuildZeroUser`).

## 15. The new server

- **Storage**: tables `guilds` (id, server, name unique without case among live guilds, country, master, remark, created/disbanded) and
  `guild_members` (character → guild, rank 0 to 9, joined), one row per character (a member or a request, as `usp_Create_GuildChar_E`
  allows), and `characters.guild_joined_at` for the penalty. Loaded at start for this server; guilds with no member are disbanded then.
- **In memory**: `crate::guild` (`Guilds`: guilds, members with name, level, job, rank and online state, requests, invitations, agreements,
  every online player's penalty) in the zone object, taken after `maps` and `parties`; `crate::zone::guild` sends the packets. Every change
  to the database goes, in order, through one writer task; creating a guild waits for its transaction (the guild row, the members and the
  requester's 1000 gold together) before anything is sent.
- **Members online** from the enter-world burst (which carries the guild list, the member list and the join requests before `0x0105`, as
  the original) to leaving the world.
- **Deviations** (approved):
  1. **The party size needed to create a guild is a setting**, `guild_party_size`, default 7 as the original; 1 lets a player create
     one alone (no party needed). For testing with few accounts.
  2. **Agreements lapse after 30 s** (creation: `0x0D01 0` "Failed to form guild"; disbanding: `0x0D03 0` "Failed to disband guild"), and
     end at once when one of their members leaves the world (creation `0x0D01 3`, disbanding `0x0D03 2`).
  3. **Disbanding needs the yes of everyone asked** (the master and every officer in its party on its map, at least 2 besides the
     master), where the original disbands at the third yes however many were asked.
  4. **The join penalty counts from the latest join** (creating, an accepted invitation or request), at login and in memory alike; a join
     request does not start it. It still does not block invitations (as the original).
  5. **A deleted master's guild always gets a master**: the member with the lowest rank number, the longest in the guild among equals,
     online or not; a guild left empty is disbanded at once.
  6. **A refused remark is answered with the current one** (`0x0D20 0`), so the master's client does not show a remark that was not kept.
  7. **Names and remarks are checked**: a name of 2 to 24 printable ASCII bytes without `'` or `<`, not starting or ending with a space
     (else `0x0D21 6`); a remark of printable ASCII without `'` whose colour markers are safe (at creation else `0x0D21 0`; a new remark
     else as deviation 6).
  8. **The looks are resent to the player's view** (`0x0303`) when its guild changes, so everyone around sees the new name at once.
  9. **Nothing joins twice**: an invitation or a join request accepted while the player is in an agreement is ignored; a member of a
     creation who joined another guild during the agreement is left out of the new one; a pending join request of a new member (any way
     it joins) is withdrawn first (`0x0D18` to that guild's officers). Inviting needs a guild (the original lets a player without one send
     invitations that come to nothing).
  10. **A failed guild or friend write is tried again** (audit R16): 0.2 s, 1 s and 5 s after the failure, then logged at error with the
      write and given up; the writers keep their order meanwhile. At shutdown the guild and friend writers are flushed (up to 10 s) after
      the connections close, as the game log is.
  11. **Agreed agreements lapse too** (audit R27): a creation everyone said yes to lapses 30 s after the last yes if its founder's
      connection has not taken it up (`0x0D01 0`), and ends when its founder leaves the world (`0x0D01 3`); before, it blocked its
      members' guild actions until the server restarted. A creation on its way to the database does not lapse by the clock.
  12. **A disband agreed after a deposit is refused** (audit R23): the final yes checks the bank again and answers `0x0D03 0` when it
      holds items; the disband requests and answers run under the guild bank's turn, so no bank move is half done when the guild is
      closed.
  13. **The bank count fix-ups are written before the server listens** (audit R29): a stack whose stored count was wrong is corrected at
      the load by an awaited write, so a member's first bank move cannot race it through the writer's queue.
- The guild house, Etin and NPC upgrades: `docs/original/guild-house.md` (part 1); the ranking battle and the ranking:
  `docs/original/guild-ranking.md` (part 2, built); the bank: `docs/original/guild-bank.md` (part 3, built). Guild wars:
  `docs/original/guild-war.md`.
