# Friends, the block list and the party search

This page covers the friend list (asking, answering, removing, the memo, who shows as online), the block list (adding, removing, the
memo), the party search (`0x220C`/`0x220D`), the lists at login and the database, in the EP4.5 game server `ps_game.exe` (built
2009-07-15). It was read in the disassembly and the Ghidra decompiler with the PDB names, and in `ps_dbAgent.exe` and the `PS_GameData`
stored procedures (a restored MSSQL copy). The client side is in the [packet catalogue](../client/client-packets.generated.md) (`0x22xx`).

Renaming characters (`usp_Update_Char_Rename` also renames friend and block entries) is on
[Small features](small-items.md#9-the-name-change-item-special-51-and-0x010e), the game logs on [The game log](game-log.md).

## In short

- **Friends are mutual.** A player asks another of the same faction on the same map; once the other says yes, each is in the other's
  list. A player shows as online only to friends who also have them in their list.
- **Up to 100 friends and 100 blocked players**, each with a short memo.
- **Blocking does nothing on the server.** The server keeps the block list but never consults it: whispers, friend requests, party and
  guild invitations and trade requests from a blocked player all go through. Only the client acts on it.
- **Party search** lists up to 30 players on the same map, of the same faction and within 5 levels, who have registered as looking for
  a party.
- **Faults:** full lists are not refused (entries past 100 are saved but lost from memory), deleted characters stay in friend lists, and
  a memo with a malformed colour marker can crash the client that displays it.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (see [NPC shops](shop.md) on the
PDB's 4-byte shift).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the disassembly (`tools/disasm.py`) and the Ghidra decompile (REA), and from
`ps_dbAgent.exe` and the `PS_GameData` stored procedures (restored MSSQL copy). Client side: the packet catalogue in
`docs/client/client-packets.generated.md` (`0x22xx`). `CUser` offsets are the code's (`docs/original/shop.md` on the PDB's 4-byte shift).

Covered: the friend list (asking, answering, removing, the memo, who shows as online), the block list (adding, removing, the memo), the
party search (`0x220C`/`0x220D`), the lists at login and the database. **Not covered:** renaming characters (`usp_Update_Char_Rename` also
renames friend and block entries; renaming is built, `docs/original/small-items.md` section 9: the new server's friend and block
rows hold no names, the lists take the current name from `characters` when loaded, so a renamed character shows under its new name from
the owner's next login), game logs.

## 1. The state

Confirmed, `CFriend` at `CUser` +0x1780, `CFriend::Reset` 0x4b9690.

| field | offset | |
|---|---|---|
| friend count | +0x1780 | at most **100** (`CFriend::FriendAdd` 0x4b97f0 stores nothing past 100 and says nothing) |
| friends | +0x1784 | 100 × 80 bytes `stFriendInfo`: `u32 char id, u8 family, u8 grow, u8 job, char name[21], char memo[51]` |
| block count | +0x36C4 | at most **100** (`CFriend::BlockAdd` 0x4b9bb0, the same silent limit) |
| blocks | +0x36C8 | 100 × 76 bytes `stBlockInfo`: `u32 char id, char name[21], char memo[51]` |
| asked by | +0x5478 | `m_dwFriendReqID`: the id of the player whose friend request waits for this player's answer, 0 if none |
| answer deadline | +0x583C | `m_dwTimeOverFriend`: `GetTickCount() + 30000`, set with the request |
| party search | +0x1744 | `m_bPartyFindReg`, `u8`: 1 once registered for the party search |

Names are compared with `strcmp` (case matters); the lists are searched by id (`FriendFind`/`BlockFind` with a `u32`) or by name.
Removing (`FriendRemove`/`BlockRemove` by id) moves the later entries down; the name variants are never called.

## 2. The packets

Confirmed, `CUser::PacketComm` 0x475870.

The handler takes `0x2202` to `0x220C` through a jump table; **`0x2204`, `0x2207`, `0x2208`** and anything outside the range close the
connection (`SConnection::Close(9)`).

Client → server:

| opcode | size | layout | what |
|---|---|---|---|
| `0x2202` | 23 | `char name[21]` | ask a player to be friends |
| `0x2203` | 3 | `u8 yes` | answer a request (1 yes, else no) |
| `0x2205` | 6 | `u32 id` | remove a friend |
| `0x2206` | 7 + n | `u32 id, u8 n, char memo[n]` | a friend's memo |
| `0x2209` | 23 | `char name[21]` | block a player |
| `0x220A` | 6 | `u32 id` | unblock |
| `0x220B` | 7 + n | `u32 id, u8 n, char memo[n]` | a blocked player's memo |
| `0x220C` | 2 | — | the party search |

Server → client:

| opcode | size | layout | what |
|---|---|---|---|
| `0x2201` | var | `u8 count`, count × `u32 id, u8 job, u8 online, char name[21], u8 n, char memo[n]` | the friend list, 25 entries a packet |
| `0x2202` | 23 | `char name[21]` | someone asks to be friends (to the one asked) |
| `0x2203` | 3 | `u8 result` | to the asker: **0** refused, **1** done, **2** cannot ask |
| `0x2204` | 28 | `u32 id, u8 job, char name[21]` | a friend added |
| `0x2205` | 6 | `u32 id` | a friend removed |
| `0x2206` | 8 + n | `u32 id, u8 n, char memo[n], 0` | a friend's memo saved |
| `0x2207` | 7 | `u32 id, u8 online` | a friend's online state |
| `0x2208` | var | `u8 count`, count × `u32 id, char name[21], u8 n, char memo[n]` | the block list, 25 entries a packet |
| `0x2209` | 27 | `u32 id, char name[21]` | a block added |
| `0x220A` | 6 | `u32 id` | a block removed |
| `0x220B` | 8 + n | `u32 id, u8 n, char memo[n], 0` | a block's memo saved |
| `0x220C` | 3 | `u8 1` | registered for the party search |
| `0x220D` | var | `u8 count`, count × `u8 level, u8 job, char name[21]` | the party search results, at most 30 |
| `0x220E` | 3 | `u8 result` | block: **0** failed, **1** done |

The client sends the memo packets with only `n` bytes of text (`7 + n` bytes); the server reads them as the fixed 58-byte structs
`TP_COMM_FRIEND_MEMO`/`TP_COMM_BLOCK_MEMO` and puts a NUL at byte 57 (section 9, fault 3).

## 3. Asking

Confirmed, case `0x2202`.

A asks for B by name. The name's byte 20 is set to 0, then, in this order, the request fails with **`0x2203` 2 to A** unless:

1. B's name is not in A's friend list (by name);
2. B is in the world (`CWorld::FindUserZoneMove` by name) **and on A's map** (`CZone::FindUser` in A's zone with B's id);
3. B's id is not in A's friend list;
4. B is not A;
5. B is of A's faction (`+0x12D`);
6. B is not already asked by someone (`m_dwFriendReqID` 0).

Then:

- **B already has A in its list (by name)**: A adds B at once, no question asked: `FriendAdd` to A's list, the database row
  (`0x0C02` to the database agent: A's account, B's id, family, grow, job, name), **`0x2203` 1** to A, **`0x2204`** (B's id, job, name)
  to A, and **`0x2207`** (A's id, 1) to B, since A now shows as online in B's list.
- **Otherwise**: B's `m_dwFriendReqID` = A's id, the deadline now + 30 s, and **`0x2202`** (A's name) to B. A hears nothing until B answers.

The limit of 100 is not checked: `FriendAdd` drops the entry when the list is full, but the row is still written and `0x2204` still sent.

## 4. Answering

Confirmed, case `0x2203`.

Only with a request waiting (`m_dwFriendReqID` ≠ 0, else nothing happens). B's request is cleared first (the id and the deadline), then:

- A must be in the world (`FindUserZoneMove` by id) and on B's map; otherwise nothing more happens (A is not told).
- **A's id already in B's list, or the answer is not 1**: **`0x2203` 0** to A (refused).
- **Yes**: B adds A and A adds B (`FriendAdd` each, no check that A does not have B already), a database row for each (`0x0C02`, B's
  first), then **`0x2203` 1** to A, **`0x2204`** (B) to A and **`0x2204`** (A) to B.

**Timing out** (`CUser::UpdateTimeover` 0x48e3e0): once the deadline passes, B's request is cleared, silently. **Leaving the zone**
(`CUser::LeaveZone` 0x44b15d, so changing maps or leaving the world): a waiting request is refused, **`0x2203` 0** to A if A is on B's map,
and cleared.

## 5. Removing and the memo

Confirmed, cases `0x2205`, `0x2206`.

**Removing** (`u32 id`): if the id is in the list (`FriendRemove`, else nothing), the row goes (`0x0C03`), **`0x2205`** (id) to the
player, and if the removed friend is in the world, **`0x2207`** (the player's id, 0) to it: the player shows as offline in its list from
now on. The removed friend keeps the player in its own list. In the database, `usp_Delete_FriendChar_E` deletes the player's row and sets
`Refuse = 1` on the other side's row, a flag nothing reads (the database agent reads it and drops it).

**The memo** (`u32 id, u8 n, memo`): `n` at most **51** (else nothing), byte 57 set to 0, the friend found by id (else nothing); the memo
is copied (from byte 7 up to the first NUL), written (`0x0C04`), and echoed: **`0x2206`** (id, its length, the memo, NUL). The database
agent replaces `'` with a space before `usp_Save_FriendMemo_E`, which trims spaces.

## 6. Online

Confirmed, `CUser::SendCharacterFriend` 0x483ac0, `InitFriend` 0x4756d0, `ReleaseFriend` 0x4757a0.

A friend shows as online (`0x2201` field `online`, `0x2207`) when it is in the world **and has the player in its own list**: online is
only ever shown both ways.

- **Entering the world** (`CUser::PacketUserDBChar`, before the burst): `InitFriend` sends **`0x2207`** (the player's id, 1) to every
  friend in the player's list who is in the world, whether or not that friend lists the player (its client ignores ids it does not list).
  The burst then carries **`0x2201`** with `online` = the friend is in the world and lists the player, and **`0x2208`**
  (`docs/original/enter-world.md` section 3, step 12), each only if the list is not empty, 25 entries a packet.
- **Leaving the world** (`CUser::LeaveWorld`): `ReleaseFriend` sends **`0x2207`** (the player's id, 0) to every friend in its list who
  is in the world.
- Changing maps sends nothing. Level-ups send nothing (the list carries no level).

## 7. The block list

Confirmed, cases `0x2209`, `0x220A`, `0x220B`.

**Adding** (by name, byte 20 set to 0) fails with **`0x220E` 0** unless the name is not in the block list, the player is in the world
(`CWorld::FindUser` by name, any map), is not the player, is of its faction, and its id is not in the block list. Then `BlockAdd`, the row
(`0x0C06`: account, id, name), **`0x220E` 1** and **`0x2209`** (id, name). The 100 limit is not checked (as for friends).

**Removing** (`u32 id`): if found (else nothing), the row goes (`0x0C07`) and **`0x220A`** (id) is sent. **The memo**: as for friends but
`n` at most **50**, written with `0x0C08` (`usp_Save_BandMemo_E`), echoed with **`0x220B`**.

**The block list does nothing else in the server**: `BlockFind` is called only from `PacketComm`. Whispers, friend requests, party and
guild invitations and trade requests from a blocked player all go through. Whatever blocking does, the client does it.

## 8. The party search

Confirmed, case `0x220C`; `CParty::EnterNormalParty` 0x4463d2, `EnterRaidParty` 0x445f3d, `CUser::EnterParty`.

`0x220C`: a player **not in a party** is registered (`m_bPartyFindReg` = 1); a player in a party is not, but gets the rest anyway.
**`0x220C` 1** to the player, then **`0x220D`** with up to **30** players **on the same map** who are registered, of its faction, not the
player, and of a level within **±5** of the player's: each `u8 level` (the low byte), `u8 job`, the name. `0x220D` is sent only if
someone was found. The registration ends when the player joins a party (normal or union) and when the character leaves the world; there
is no packet to end it otherwise.

## 9. Faults in the original

1. **Full lists are not refused**: past 100 entries, a friend or block is written to the database and announced to the client but not
   kept in memory; at the next login the database agent loads only the first 100 rows (`DBFriend::LoadFriend` 0x425f80 stops at 100) and
   the rest stay in the table for ever.
2. **A deleted character stays in its friends' lists**: `usp_Read_FriendChar_R` joins `Chars` without looking at `Del`, so the deleted
   character keeps showing (offline) until each friend removes it.
3. **The memo is read past what the client sent**: the memo packet is `7 + n` bytes but the server copies up to byte 57, so the bytes past
   the text are whatever was in the receive buffer before.
4. **Memos are shown to the client unchecked**: a memo is client text, sent back in `0x2201`/`0x2208` at every login; a memo carrying
   the `<<NNNcCC` colour marker crashes the client that shows it (`docs/client/README.md`, the chat crash).
5. **Adding twice**: answering yes does not check whether the asker already has the one asked; two crossed requests (A asks B, B asks A,
   both say yes) give each a second copy of the other in memory and a duplicate row in `FriendChars` (no unique key).

## 10. The original database

Confirmed, `PS_GameData`.

- `FriendChars` (`RowID`, `CharID`, `FriendID`, `FriendName varchar(30)`, `Family`, `Grow`, `Job`, `Memo varchar(50)`, `Refuse`,
  `CreateDate`, `RefuseDate`). `usp_Read_FriendChar_R` returns `FriendID`, the friend's **current** name from `Chars`, family, grow, job,
  memo, refuse; the agent keeps the id, the job, the name and the memo.
- `BanChars` (`RowID`, `CharID`, `BanID`, `BanName varchar(30)`, `Memo varchar(50)`, `BanDate`); `usp_Read_BanChar_R` returns the stored
  `BanName`, not the current one.
- Database agent packets: `0x0C01` friend list and `0x0C05` block list (to the game server at character load), `0x0C02` add friend,
  `0x0C03` remove friend, `0x0C04` friend memo, `0x0C06` add block, `0x0C07` remove block, `0x0C08` block memo (`CUser::PacketUserFriend`
  0x406930 in the agent, writes kept in memory and saved by `DBFriend::SaveFriend`/`SaveBlock`).

## 11. The new server

- **Storage**: migration `0012_character_friends.sql`: `character_friends` (character, friend, memo, created; one row per pair, primary
  key on both) and `character_blocks` (character, blocked, memo, created; the same). The friend's and blocked player's name, job and
  deletion come from `characters` when the lists are loaded, so they are always current; the lists keep the order the entries were added.
- **In memory**: `crate::friends` (`Friends`: every player in the world as the friends need it (outbox, name, country, job, family,
  grow, level, map), each one's friend and block lists, the waiting requests, the party search registrations) in the zone object, taken
  after `maps` and `parties`; it sends the friend packets itself. Every database change goes, in order, through one writer task (as for
  guilds); a yes writes both rows in one transaction.
- **Loaded** with the character, sent in the burst (`0x2201` then `0x2208`, after the finished quests), the player known to the friends
  right after the burst (`0x2207` 1 to its friends then), forgotten on leaving the world (`0x2207` 0).
- **Deviations** (approved):
  1. **A full list refuses**: asking with 100 friends, or asking someone with 100, fails with `0x2203` 2; a yes when either side is full
     is answered with `0x2203` 0; blocking with 100 blocks fails with `0x220E` 0 (fault 1).
  2. **Deleted characters are left out** of the lists loaded while they are deleted; their rows stay, so a restored character is a friend
     again (fault 2). A character is deleted from the character screen, out of the world, so nothing is sent at deletion; its friends
     see the change at their next login.
  3. **The memo is exactly the `n` bytes sent**, up to the first NUL, at most 50 (the column's size; the original lets a friend memo have
     51 and the database truncates it), and **checked like chat**: printable text with safe colour markers only, `'` kept (the database
     has no SQL text); a memo that fails the check is not saved and not echoed (faults 3 and 4).
  4. **No duplicates**: a yes when the asker already has the one asked adds only what is missing on each side; the table's key makes a
     second row impossible (fault 5).
  5. **Blocking works**: whispers from a blocked player get the "not found" whisper failure (`0x1106`), as if the player were not online,
     and its friend requests fail with `0x2203` 2. Party and guild invitations and trade requests still go through, as in the original.
  6. **A request runs out when it is next looked at** (a new request for that player, or its answer) rather than in a timer pass; the
     effect is the same: after 30 s the answer is dropped silently and the player may be asked again.
  7. **A memo in a list is cut at 50 bytes** (audit R22): the list and memo packets never carry more than the column holds, so a memo a
     tool wrote can not overflow a list packet; the checks of deviation 3 are unchanged.
- Kept from the original: same map to ask and to answer, same faction, one waiting request per player, 30 s to answer, online shown only both ways,
  removing is one-sided, blocking needs the player in the world and of the same faction, the party search as in section 8.
