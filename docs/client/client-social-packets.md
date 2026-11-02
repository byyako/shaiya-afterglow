# Social packets: trade, party and union, member status, guild

This page describes what the client `game.exe` (US build 2010-04-26, patch 108) does with every trade (`0x0Axx`), party and union
(`0x0Bxx`), party member status (`0x0Cxx`) and guild (`0x0Dxx`) packet, read from its handlers and send sites, and in which order the
original EP4.5 server `ps_game.exe` sends them.

The method is the one on [World packets](client-world-packets.md): the dispatcher (`0x582e10`) calls a reader (`FUN_005xxxxx`, field
order = wire order), which calls a slot of the network object's vtable (`0x6A5C20`) holding the logic (the "logic" column). Sizes include
the 2-byte opcode and exclude the length prefix. Field names are the EP4.5 server's own (from `ps_game.pdb`, the debug symbols shipped
with `ps_game.exe`); ✓ = the PDB struct has exactly the size the client reads or sends.

Status words:

- **confirmed**: read in the client code (the default; not repeated per row);
- **server**: read in the original EP4.5 `ps_game.exe` (decompiled with the PDB's function names). Used for what the client cannot tell:
  which packets the server sends, to whom, in which order;
- *inferred*: from indirect evidence (names, data use) only; **unknown**: not traced.

Messages are `sysmsg-uni.txt` IDs with their text. `<p>` = a player name the client fills in, `<b>` = a guild name or number.

## In short

- **Every invitation is a yes/no dialog** on the other player's screen. Party, union and trade answers use 0 for yes; guild answers use
  1 for yes.
- **A trade** is a request, a window on both sides, items and gold put in, and the exchange runs once both players press Ready. The original server allows trades only
  within the same faction and keeps a request open for 30 seconds.
- **A party** has up to 7 members. Their health bars move only when the server sends updates; the client never asks.
- **A union** ("raid") joins several parties; the client keeps its own list of the members and their groups.
- **A guild** has its own create, join, leave, kick and rank packets, a guild list, guild points and a guild house.

## Conventions shared by these families

- **Yes/no dialogs.** A request that needs the player's answer sets the pending-dialog mode `DAT_00850a5c` (4 party invite,
  5 trade request, 0x19 union invite, 1 guild-creation agreement, 2 guild-disband agreement, 3 guild invitation); the dialog
  code in `FUN_0048c120` sends the answer. **Party, union and trade answers send 0 = yes, 1 = no; guild answers send 1 = yes,
  0 = no.**
- **Names in dialogs and messages come from the player table**: the client looks the id up among the players in view
  (`FUN_00447880`). Unknown id: party/trade invitations show a generic text ("invited to a party" 10007, "receive a request
  for transaction." 10008) and some results show nothing (noted per row). So the server should only send ids of players the
  client has seen (`0x0201`), or accept the generic text.
- **Busy players auto-decline.** On a party/union invitation or a trade request the client answers "no" by itself when the
  player is busy (player object `+0x2f0` set) or has the refuse option on (`DAT_00701a04` party, `DAT_00701a0c` trade;
  *inferred* to be the "refuse party/trade" options).
- **Party type above heads**: `0x0520` (7) `u32 dwUserID, u8 byType` stores `byType` at player `+0x2c0` (*inferred*: the
  party/union mark drawn on that player). **server**: sent to the players in view of each member whose party state changes
  (join, leave, kick, leader change); the value comes from `CUser::GetPartyType` (values **unknown**).

## Trade (`0x0Axx`)

Client state: partner id `DAT_00854e98` (−1 = no trade), my 8 offered slots `DAT_00854c50` (34-byte item units) with their
source `bag, slot, count` in `DAT_00854e70`, the partner's 8 slots `DAT_00854d60`, my money offer `DAT_00854e90`, the partner's
`DAT_00854e94`, ready flags `DAT_00854e88` (me) / `DAT_00854e8c` (partner).

### Server → client

| op | size | fields (PDB) | logic | what the client does |
|---|---|---|---|---|
| `0x0A01` | 6 ✓ | `u32 dwID` (requester; `TP_EXC_REQUEST`) | `0x558b30` | trade request from `dwID`: opens the yes/no dialog "<p> requested a transaction" (210); busy → answers `0x0A02` = 1 at once |
| `0x0A03` | 7 ✓ | `u32 dwID` (partner), `u8 byOK` (`TP_EXC_START`) | `0x558ba0` | stores the partner id; 0: opens the trade window (mode 0x0C); 1: "The opponent refused transaction" (321); 2: "Transaction in process with another user" (322); both with `ui_error001.wav` |
| `0x0A04` | 3 ✓ | `u8 byOK` (`TP_EXC_END`) | `0x558c20` | trade over, window closed, partner cleared. 0: "Transaction has been completed successfully" (211) and **money += partner's offer − my offer**; 1: silent; 2: "Transaction has been canceled" (212); 3: "trade has been cancelled by the other player" (240); 4: "Insufficient inventory for a transaction" (241); 5: "The other player has insufficient inventory space." (242); 6: "Exceeded max. gold possessable." (468); 7: "The player you are trading with has exceeded the maximum amount of gold possible." (469). 2–7 play `ui_error001.wav` |
| `0x0A05` | 4 ✓ | `u8 byWho, u8 byOK` (`TP_EXC_READY`) | `0x558e70` | ready state: `byWho` 1 = me, 2 = partner, 3 = both; `byOK` **0 = ready (locked), 1 = not ready** |
| `0x0A06` | 6 ✓ | `u8 byBag, u8 bySlot, u8 byCount, u8 byExcSlot` (`TPC_EXC_ADD`, echo of the request) | `0x558ec0` | my offer: copies the item at bag/slot into my trade slot `byExcSlot` (0–7) with count `byCount` |
| `0x0A07` | 4 ✓ | `u8 byWho, u8 byExcSlot` (`TP_EXC_REM`) | `0x558fc0` | empties a trade slot: `byWho` 1 = mine, 2 = the partner's |
| `0x0A08` | 7 ✓ | `u8 byWho, u32 dwMoney` (`TP_EXC_MONEY`) | `0x559050` | money offer: 1 = mine, 2 = the partner's |
| `0x0A09` | 35 ✓ | `u8 byExcSlot, u8 byType, u8 byTypeID, u8 byCount, u16 wQuality, u8 byGem[6], char szCraftName[21]` (`TP_EXC_ADD_DEST`) | `0x558f20` | the partner put an item in their slot `byExcSlot` |

### Client → server

| op | size | fields (PDB) | sent when |
|---|---|---|---|
| `0x0A01` | 6 ✓ | `u32 dwID` (`TP_EXC_REQUEST`) | player menu "request transaction" (5091) on a player; prints "Requested <p> a transaction" (209) |
| `0x0A02` | 3 ✓ | `u8 byOK` (`TP_EXC_ANSWER`): **0 = accept, 1 = refuse** | trade request dialog; refuse prints "Rejected transaction request" (300); also sent (1) automatically when busy |
| `0x0A05` | 3 ✓ | `u8 byReady` (`TPC_EXC_READY`): **0 = ready, 1 = cancel ready, 2 = cancel the trade** | ready button (0 or 1 by the current state); 2 when the trade window closes while a partner is set |
| `0x0A06` | 6 ✓ | `u8 byBag, u8 bySlot, u8 byCount, u8 byExcSlot` (`TPC_EXC_ADD`) | item dropped on a free trade slot (count asked when the stack is > 1). Refused by the client itself: "Not a tradeable item" (443, `Item.SData` flag), quest item types 27, 28, 29, 99 "Cannot trade the quest items" (231), an item already offered |
| `0x0A07` | 3 ✓ | `u8 byExcSlot` (`TPC_EXC_REM`) | right-click on one of my offered items |
| `0x0A08` | 6 ✓ | `u32 dwMoney` (`TPC_EXC_MONEY`) | money entered in the trade window |

The same window is reused for duel stakes (`0x2409`–`0x240C`, same layouts) when its duel flag (`+0xab4`) is set.

### What the server must send (order from the original server, **server**)

1. `0x0A01` from A → target B gets `0x0A01 (A's id)`. B busy or other faction → A gets `0x0A03 (B, 2)`.
2. B answers `0x0A02`: 1 → A gets `0x0A03 (B, 1)`. 0 → **both** get `0x0A03 (other's id, 0)`: windows open.
3. Each add/remove/money change first cancels both ready flags (if set: `0x0A05 (3, 1)` to both), then echoes: add → the
   adder gets `0x0A06` (echo) and the other gets `0x0A09`; remove → `0x0A07 (1, slot)` to the remover, `(2, slot)` to the other;
   money → `0x0A08 (1, money)` / `(2, money)`.
4. Ready `0x0A05 0` → `0x0A05 (1, 0)` to the sender, `(2, 0)` to the other. When both are ready: the exchange runs.
5. Exchange: money moved (the client adds the difference itself on `0x0A04 0`), then **for each item**: the giver gets
   `0x0206` (`TP_ITEM_DROP`: bag, slot, and what is left there) and the receiver gets `0x0205` (`TP_ITEM_GET`, 36 bytes: the
   item in its new bag/slot, "Attained <i>"); then `0x0A04 0` to both. Failures: `0x0A04` 4/5 (inventory) or 6/7 (gold),
   each side getting its own view (4 to the one short of space, 5 to the other).
6. `0x0A05 2` (window closed) → `0x0A04 2` to the canceller, `0x0A04 3` to the other.

The original server limits trades to the same faction and keeps the request for 30 s.

## Party (`0x0Bxx`) and member status (`0x0Cxx`)

Client state: member list `DAT_021e4838` (vector of 0x50-byte member objects), leader id `DAT_021e4920`, "in a party"
`DAT_021e4924`. **The local player is a member of its own list**: the client appends itself (id `DAT_0084e6f4`, its name) after
the members the server lists. A party has **up to 7 members** (**server**).

### Member unit (`PARTY_USER_INFO_UNIT`, 1847 bytes max; sent packed)

| offset | type | field | client use (member object offset) |
|---|---|---|---|
| 0 | u32 | `dwUserID` | +0x00 |
| 4 | char[21] | name | +0x04 |
| 25 | u16 | level | +0x1A (updated by `0x0C09`) |
| 27 | u8 | *inferred* job | +0x1C (server takes it from the character's +0x134) |
| 28 | u16 | max HP | +0x1E |
| 30 | u16 | HP | +0x20 |
| 32 | u16 | max SP | +0x22 |
| 34 | u16 | SP | +0x24 |
| 36 | u16 | max MP | +0x26 |
| 38 | u16 | MP | +0x28 |
| 40 | u16 | map | +0x2A |
| 42 | f32 ×3 | x, y, z | +0x2C..+0x34 (minimap) |
| 54 | u8 | buff count n | |
| 55 | n × (`u16 wSkillID, u8 bySkillLV, u32 time`) | buffs | buff list (+0x40) |

The unit is **55 + 7n bytes** on the wire (not padded to 1847). The original server sends at most 40 buffs, `time` = remaining
seconds (**server**); the client stores the `u32` as it is, while `0x0C04` stores the skill's duration ×1000 (ms) in the same
field (*inferred*: the unit of this field is milliseconds and the original server's seconds are a mismatch; not visible
unless the party window shows buff timers).

### Server → client

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0B01` | 4 + Σunits (≤ 2052) | `u8 byBoth` (leader index), `u8 byCount`, units (`TP_PARTY_LIST`) | `0x55ffc0` | builds the list: the `byCount` members, then the local player appended; leader = the entry at index `byBoth` in that order (index `byCount` = the local player). Sets "in a party"; if `byCount > 0`: "Joined the party" (207), `ui_party002.wav` |
| `0x0B02` | 6 ✓ | `u32 dwID` (inviter; `TP_PARTY_REQUEST`) | `0x55e820` | invitation dialog (mode 4) "<p> invited" (204), `ui_party001.wav`; busy → answers `0x0B03 (1, my id)` |
| `0x0B03` | 7 ✓ | `u8 bySuccess, u32 dwID` (`TP_PARTY_ANSWER`) | `0x55e8b0` | invitation failed, with `ui_error001.wav`: 0 "<p> rejected the party's invitation" (206), 1 "<p> cannot be invited to the party" (304) — **0 and 1 show nothing unless `dwID` is a player in view**; 2 "This command is only available for a party leader" (345), 3 "Already in another party." (346), 4 "The character doesn't exist." (347), 5 "Already in a party." (348) |
| `0x0B04` | 6 + unit | unit (`TP_PARTY_ENTER`) | `0x560300` | adds a member (ignored if already listed): "<p> joined the party" (205), `ui_party002.wav` |
| `0x0B05` | 6 ✓ | `u32 dwID` (`TP_PARTY_LEAVE`) | `0x55fbf0` | my id: party cleared, "Left the party for good" (305). Other: removed, "<p> left the party for good" (306) |
| `0x0B06` | 6 ✓ | `u32 dwID` (`TP_PARTY_EXPEL`) | `0x55fcc0` | my id: party cleared, "A party leader kicked you from the party" (309), `ui_error001.wav`. Other: removed; "<p> has been kicked from the party" (310) if in view |
| `0x0B07` | 6 | `u32 dwID` | `0x55ed90` | new leader: mine "You are granted as a party leader" (307), other "<p> is granted as a party leader" (308). `dwID = 0` clears the leader silently (the original server sends 0 to the requester when a leader change fails) |
| `0x0B08` | 8 ✓ | `u32 dwID, u8 byType, u8 byTypeID` (`TP_PARTY_GETITEM`) | `0x55f2e0` (flag 0) | "<p> attained <i>" (61) for a party member's loot (item name coloured) |
| `0x0B09` | 23 ✓ | `char szName[21]` (`TP_PARTY_REQUESTBYNAME`) | `0x55e980` | invitation by name (from `/party <name>`): same dialog as `0x0B02`, with this name |
| `0x0B0F` | 6 ✓ | `u32 nDivType` (`TP_PARTY_SETDIVTYPE`) | `0x55ea30` | loot rule: 1 "Item Loot is changed order distribution." (6516), 2 random (6517), 4 leader (6518) |
| `0x0B10` | 3 ✓ | `u8 bAutoJoinFlag` | `0x55ea80` | auto-join on (6520) / off (6521) |

Member status (party; the union versions are below) — find the member by id, else ignore:

| op | size | fields (PDB) | logic | what the client does |
|---|---|---|---|---|
| `0x0C01` | 9 ✓ | `u32 dwUserID, u8 byType, u16 wPoint` (`TP_PARTY_SET_ENERGY`) | `0x55ee30` | sets current HP/SP/MP: **type 0 HP, 1 SP, 2 MP** |
| `0x0C02` | 9 | same layout | `0x55ef20` | sets one maximum (same types) |
| `0x0C03` | 12 ✓ | `u32 dwUserID, u16 wHP, u16 wSP, u16 wMP` (`TP_PARTY_SET_ENERGYALL`) | `0x55f010` | sets the three current values |
| `0x0C08` | 12 | `u32 dwUserID, u16 maxHP, u16 maxSP, u16 maxMP` | `0x55f0d0` | sets the three maxima |
| `0x0C04` | 9 ✓ | `u32 dwUserID, u16 wSkillID, u8 bySkillLV` (`TP_PARTY_SET_SKILL_ADD`) | `0x560560` | adds/updates a buff on the member (time = `Skill.SData` +0x36 × 1000; level 100 counts as 1) |
| `0x0C05` | 9 ✓ | same (`TP_PARTY_SET_SKILL_REMOVE`) | `0x55f710` | removes the buff with that skill and level |
| `0x0C06` | 6 ✓ | `u32 dwUserID` (`TP_PARTY_SET_DEATH`) | `0x55f8c0` | clears the member's buff list |
| `0x0C07` | 20 ✓ | `u32 dwUserID, u16 wMap, f32 fX, fY, fZ` (`TP_PARTY_SET_POS`) | `0x55f190` | member position (party window/minimap) |
| `0x0C09` | 8 ✓ | `u32 dwUserID, u16 wLevel` (`TP_PARTY_SET_LEVEL`) | `0x55f250` | member level |

The members' HP bars only move with these packets: the server must push them (the client never asks).

### Client → server

| op | size | fields | sent when |
|---|---|---|---|
| `0x0B02` | 6 | `u32 dwID` | player menu "party invite"; prints "Invited <p>" (203) |
| `0x0B03` | 7 | `u8 byResult` (**0 = accept, 1 = refuse**), `u32` **the local player's own id** | invitation dialog; refuse prints "Rejected the party invitation" (208); also (1, own id) automatically when busy |
| `0x0B05` | 2 | — | player menu "leave party", or `/withdraw` (5081); prints "Left the party." (358) |
| `0x0B06` | 6 | `u32 dwID` | player menu "kick" (prints 301) |
| `0x0B07` | 6 | `u32 dwID` | player menu "grant leadership" (prints 302) |
| `0x0B09` | 23 | `char szName[21]` | `/party <name>` (5079) when not in a union; prints "Invited <p>" |
| `0x0B0F` | 6 ✓ | `u32 nDivType` (1, 2, 4) | union settings (loot rule) |
| `0x0B10` | 3 ✓ | `u8 bAutoJoinFlag` | union settings |

### What the server must send (order from the original server, **server**)

- **Invite**: `0x0B02 (inviter id)` to the target (refused with `0x0B03` 4 if the target is the inviter itself, 1 if the inviter
  is busy; 3/5 if the target is in a party; leader-only check for an existing party; 30 s timeout).
- **Accept** (`0x0B03 0`): a new party is created when the inviter has none, then each joining member X (the inviter first):
  1. the members already in the party get `0x0B04 (X's unit)`;
  2. X gets `0x0B01 (leader index, members already there, their units)` — **X itself is not in the list**;
  3. players in view of X get `0x0520 (X, party type)`.

  So the inviter of a new party gets `0x0B01` with count 0 (no message), the invitee then gets `0x0B01` with the inviter and
  the inviter gets `0x0B04`. **Refuse** (`0x0B03 1`) → the inviter gets `0x0B03 (0, …)` "<p> rejected …".
- **Leave** (`0x0B05`): `0x0B05 (leaver id)` to every member **including the leaver**; if the leader left, the first remaining
  member becomes leader and the rest get `0x0B07 (new leader)`; if one member is left, that member leaves too (gets
  `0x0B05` with its own id) and the party is gone. `0x0520` for the changed players.
- **Kick** (`0x0B06`, leader only): `0x0B06 (id)` to every member including the kicked one.
- **Leader** (`0x0B07`): `0x0B07 (new id)` to all; failure → `0x0B07 (0)` to the requester.
- Then keep members current with `0x0C01`–`0x0C09` (HP/SP/MP changes, buffs, death, position, level).

## Union ("raid") party

A union is a party of up to **30** members in 5 groups of 6 (`slot / 6`), with a leader (`byBoth`), a sub-leader
(`bySubBoth`), a loot rule and an auto-join flag. Client state: map slot → member `DAT_021e4850`, leader id `DAT_021e4864`,
sub-leader `DAT_021e4868`, loot rule `DAT_021e486c`, auto-join `DAT_021e4870`, my role `DAT_021e485c` (0 leader, 1 sub-leader,
2 member), my group `DAT_021e4860`. Union member units are the party unit **preceded by `u16 slot`** (`RAIDPARTY_USER_INFO_UNIT`,
57 + 7n bytes on the wire). The window visibility is kept in `CONFIG.INI` `[UNIONPARTY] UNION_WINODW_VISIBLE` (sic).

### Server → client

| op | size | fields (PDB) | logic | what the client does |
|---|---|---|---|---|
| `0x0B0E` | 16 ✓ | `bool bRaidType, bool bAutoJoin, u32 nDivType, u32 nBothIndex, u32 nSubBothIndex` (`TP_PARTY_SETRAID`) | `0x560780` | the current party becomes a union: the union list is cleared and rebuilt from the **normal party list** (`0x21e4838`), slot = index there; leader/sub-leader = the members at those slots; only when both slots are found: my role, in-union flag `0x21e4928`, window flags, "Union Party created." (6532). Sent to an existing union, members known only from `0x0B0C`/`0x0B0B` drop out |
| `0x0B0F` | 6 | `u32 nDivType` | `0x55ea30` | loot rule = value; prints 6516 / 6517 / 6518 for 1 / 2 / 4 (type 7), other values stored silently |
| `0x0B10` | 3 | `u8 bAutoJoin` | `0x55ea80` | auto-join = byte; prints 6520 (on) / 6521 (off) |
| `0x0B0B` | 10 + Σunits (≤ 2058) | `bool wStartEndFlag, u8 byBoth, u8 bySubBoth, u16 nDivType, bool bRaidPartyType, bool bAutoJoinFlag, u8 byCount`, units (`TP_RAIDPARTY_LIST`) | `0x561340` | adds the listed members by slot (does not clear first, keeps a slot already listed; `wStartEndFlag` and `bRaidPartyType` unused); leader/sub-leader by slot, loot rule, auto-join; my group from my slot. Sets **no** in-union flag, role or window flag and prints nothing: a joiner needs its own `0x0B0C` |
| `0x0B0C` | 4 + unit (≤ 1851) | `u16 slot` + party unit (`TP_RAIDPARTY_ENTER`) | `0x5610c0` | adds a member at `slot`; my own id: union window on, "Joined the party" (207); other: "<p> joined the party" (205); `ui_party002.wav` |
| `0x0B0D` | 6 | `u32 dwID` | `0x55fdb0` | my id: "Left the party for good" (305) and the union is cleared (as `0x0B16`); other: "<p> left the party for good" (306), removed |
| `0x0B12` | 18 ✓ | `u32 nSourcePos, u32 nTargetPos, u32 nBothIndex, u32 nSubBothIndex` (`TP_PARTY_CHANGEUSER`) | `0x560b70` | swaps the members at the two slots (the last two fields are read but unused); updates my group |
| `0x0B13` | 6 ✓ | `u32 dwID` (`TP_PARTY_SET_SUBBOTH_RAIDPARTY`) | `0x55eb30` | new sub-leader: "You become a sub-party leader ." (6514) / "<p><is> become a sub-party leader." (6515) |
| `0x0B14` | 6 | `u32 dwID` (inviter) | `0x55ec90` | union invitation dialog (mode 0x19, texts 10007/204); busy → `0x0B15 (1, my id)` |
| `0x0B15` | 7 | as `0x0B03` | `0x55e8b0` | same as `0x0B03` (same reader and logic) |
| `0x0B16` | 2 ✓ | — (`TP_PARTY_BREAKPARTY`) | `0x55fe20` | union dismantled: "Union Party dismantled." (6531), all union state reset, party marks of members in view cleared |
| `0x0B17` | 6 ✓ | `u32 dwID` (`TP_PARTY_SET_BOTH_RAIDPARTY`) | `0x55eac0` | new union leader: 307 / 308 |
| `0x0B18` | 23 | `char szName[21]` | `0x55ed20` | union invitation by name (dialog as `0x0B14`) |
| `0x0B19` | 6 (PDB 10) | `u32 ErrorCode` (`TP_PARTY_ERROR_RAIDPARTY`; the PDB's trailing `u32 dwId` is not read) | `0x55ebb0` | 3 "Already in another party." (346), 4 "The character doesn't exist." (347), 5 "Already in a party." (348), 10 "Can't Config Union party." (6522), 11 "Can't change Group." (6523), 12 "Can't find the party." (6535), 13 "Leader don't select auto-join mode." (6536), 14 "Requested party is full." (6538); others silent |
| `0x0B1A` | 6 ✓ | `u32 dwId` (`TP_PARTY_EXPEL_RAIDPARTY`) | `0x55ff40` | my id: "Party leader kicked you." (6533), union cleared; other: "<p><is> kicked from party." (6534) |
| `0x0C0A`–`0x0C13` | | union mirrors of `0x0C01`–`0x0C09`: `0C0A`=`0C01`, `0C0B`=`0C02`, `0C0C`=`0C03`, `0C0D`=`0C08`, `0C0E`=`0C04`, `0C10`=`0C05`, `0C11`=`0C06`, `0C12`=`0C07`, `0C13`=`0C09` (same sizes) | same functions with the union flag | the same updates on union members (`FUN_004c88b0` lookup). **There is no `0x0C0F` handler.** |
| `0x0C14` | 8 | as `0x0B08` | `0x55f2e0` (flag 1) | loot message for a union member |

### Client → server

| op | size | fields (PDB) | sent when |
|---|---|---|---|
| `0x0B0D` | 2 | — | menu "Leave Union" (6501) |
| `0x0B0E` | 8 ✓ | `bool bRaidType, bool bAutoJoin, u32 nDivType` (`TPC_PARTY_SETRAID`) | the union settings window confirmed while **not** in a union ("Create Union", 6500); `bRaidType` is always 1 |
| `0x0B0F` | 6 | `u32 nDivType` (1 order, 2 random, 4 leader: the window's three buttons) | the same window in a union ("Config Union", 6503), **leader only**, when the rule differs from the current one |
| `0x0B10` | 3 | `u8 bAutoJoin` | the same, when the auto-join box differs from the current flag |
| `0x0B11` | 23 ✓ | `char m_szName[21]` (`TPC_PARTY_AUTOJOIN`) | `/join <name>` (6543), only when in no party ("You are in party already." 6537 otherwise): join that player's union if it has auto-join |
| `0x0B12` | 10 ✓ | `u32 nSourcePos, u32 nTargetPos` (`TPC_PARTY_CHANGEUSER`) | "Move Group" (6506), then drop on a slot |
| `0x0B13` | 6 ✓ | `u32 dwSubBothId` | "Delegate sub-leader" (6508) |
| `0x0B14` | 6 | `u32 dwID` | "Union invitation" (6513); prints "Invited <p>" |
| `0x0B15` | 7 | `u8 byResult` (0 yes, 1 no), `u32` own id | union invitation dialog |
| `0x0B16` | 2 | — | "Dismantle Union" (6502) |
| `0x0B17` | 6 ✓ | `u32 dwBothId` | "Delegate leader" (6507) |
| `0x0B18` | 23 | `char szName[21]` | `/party <name>` while in a union (leader or sub-leader, union not full: else 6539 "Can't invite member because party is full." / 6540 "You don't have invitation authority.") |
| `0x0B1A` | 6 ✓ | `u32 dwId` | union "Kick" (6509) |

**server**: a joining union member gets `0x0B0C` (its own unit; without it the client is not in the union, see `0x0B0B`), then `0x0B0B` with the existing members (several packets if
needed); the others get `0x0B0C`. Leaving/kicked: `0x0B0D`/`0x0B1A` to all, then `0x0B17`/`0x0B13` if the leader or
sub-leader changed. Leader/sub-leader changes and moves are sent to all members. `0x0B16` to all before the union is removed.
`/join` errors come back as `0x0B19` 12/13/14.

## Guild (`0x0Dxx`)

Client state: my guild name `DAT_021e54e4`, my rank `DAT_021e54e2` (0xFF = none; 1 = master *inferred*), my guild id at
player `+0x2c8`, guild name above each player at player `+0x2c4`, the guild list (`DAT_021e816c`, 120-byte entries), my guild's
member list (`FUN_004cb170`/`FUN_004c7260`), the join-request list (`FUN_004cb430`), guild house NPC levels `DAT_021e8198`,
guild bank `DAT_021e5be0` (40-byte slots).

### Units

| unit | size | fields |
|---|---|---|
| `GUILD_UNIT` (guild list) | 120 | `u32 dwGuildID, char szName[25], char szMasterName[21] (inferred), char szRemark[65], u8 byRank (31 = unranked), u32 dwPoint` |
| old guild list unit (`0x0D0F`, `0x0D10`) | 50 | `u32 dwGuildID, char szName[25], char[21]` (no remark, rank or points) |
| `GUILDUSER_UNIT` (member) | 29 | `u32 dwUserID, u8 byGuildUserLevel (rank), u16 wLevel, u8 (inferred job), char szName[21]` |
| `GUILDJOINUSER_UNIT` (join request) | 28 | `u32 dwUserID, u16 wLevel, u8 (inferred job), char szName[21]` |
| `TP_GUILD_BANK_UNIT` | 33 | `u8 bySlot, u8 byType, u8 byTypeID, u16 wQuality, u8 byGem[6], u8 byCount, char szCraftName[21]` |

### Server → client

| op | size | fields (PDB) | logic | what the client does |
|---|---|---|---|---|
| `0x0D21` | 98 ✓ | `u8 byOK, u32 dwGuildID, u8 byGuildUserLevel, char szGuildName[25], char szRemark[65]` (`TP_GUILD_CREATE`) | `0x555fe0` | creation result (current version). 1: "Formed <b>guild." (372), my guild name set, and guild name + id set on every party member in view. Failure codes: 0 "Failed to form <b> guild." (371), 2 gold (373), 3 "shortage of members" (374), 4 level (375), 5 "restriction on growing mode" (376), 6 "banned words for the name" (377; the original server also uses 6 for a name already taken), 7 "rejection of party members" (378), 8 "a party member's membership in another guild" (379), 9 "a member who got penalty of Guild signup" (8007) |
| `0x0D01` | 33 | `u8 byOK, u32, u8, char szGuildName[25]` | `0x555e20` | older creation result: as `0x0D21` without remark, code 9 and guild ids. **server**: the original server sends `0x0D01` for creation failures (`CGuildCreate::SendGuildCreateFailed`) |
| `0x0D22` / `0x0D02` | 96 (PDB; the client reads 31) | `u32 dwUserID, char szGuildName[25], char szRemark[65]` (`TP_GUILD_CREATEAGREE`; the client reads the id and the name) | `0x555140` | dialog (mode 1): "<guild name> cannot join in another guild for 3 days once you join in a guild. Do you still want to join in?" (8034); answer `0x0D22` |
| `0x0D03` | 3 ✓ | `u8 byOK` (`TP_GUILD_FIRE`) | `0x556630` | disband result: 0 "Failed to disband guild." (380), 1 "Disbanded guild." (381; my guild cleared), 2 "insufficient party members" (382), 3 "party member's rejection" (383) |
| `0x0D04` | 6 ✓ | `u32 dwUserID` (`TP_GUILD_FIREAGREE`) | `0x555170` | dialog (mode 2): "Guild master has requested guild dismiss. Do you agree?" (10106); answer `0x0D04` |
| `0x0D05` | 6 | `u32 dwUserID` (inviter) | `0x555180` | dialog (mode 3): 8034 text; answer `0x0D06` |
| `0x0D06` | 7 | `u8 byOK, u32 dwUserID` | `0x555190` | to the inviter: 0 "Guild rejected join-request" (385), else "Guild accepted join-request" (384) |
| `0x0D07` | 3 ✓ | `u8 byOK` (`TP_GUILD_JOINREQUSER`) | `0x5551d0` | join request (from the guild list): 0 "Rejected join-request." (387), else "Requested to join." (386). The original server sends 2 for "still under the join penalty", which this client shows as 386 |
| `0x0D08` | 33 ✓ | `u8 byOK, u32 dwGuildID, u8 byGuildUserLevel, char szGuildName[25]` (`TP_GUILD_JOINRESULTUSER`) | `0x555200` | joined: "<b> guild accpeted join-request" (sic, 388), my guild name and id set (the rank is not stored here); 0: "<b> guild rejected join-request" (389) |
| `0x0D09` | 3 ✓ | `u8 byOK` (`TP_GUILD_LEAVE`) | `0x556770` | 1 "Quit guild." (390), my guild cleared; 0 "Impossible to quit guild." (391) |
| `0x0D0A` | 7 ✓ | `u8 byOK, u32 dwUserID` (`TP_GUILD_EXPEL`) | `0x556820` | **needs `dwUserID` in my member list, else nothing.** 0 "Impossible to kick <b> from guild." (394); my id: "Kicked from guild." (392), guild cleared; other: "Kicked <b> from guild." (393) |
| `0x0D0B` | 28 | `u32, u8, u8, char[21]` | `0x555300` | **ignored** (empty function) |
| `0x0D0C` | 7 ✓ | `u8 byState, u32 dwUserID` (`TP_GUILD_USERSTATE`) | `0x556910` | member event, by `byState`: < 100 sets the member's level; 101 (0x65) "Guild has been disbanded." (395), guild cleared; 102 "<b> quit guild." (397) and 103 "<b> has been kicked from guild." (398), member removed; 104 online, 105 offline, 106 *inferred* in the guild war zone (member status 1/2/3); > 200: rank = `byState − 200`, "<b>'s authorization has been changed." (396), mine updated too |
| `0x0D0D` | 2 ✓ | — (`TP_GUILD_GUILDLISTSTART`) | `0x5564a0` | clears the guild list before a new one |
| `0x0D0E` | 2 | — | `0x5a8700` | **ignored** (end of list) |
| `0x0D2F` | 3 + 120n | `u8 byCount`, n × `GUILD_UNIT` (`TP_GUILD_GUILDLIST` is 3 + 40 × 120) | `0x556fe0` | adds guilds to the list |
| `0x0D30` | 122 ✓ | `GUILD_UNIT` (`TP_GUILD_GUILDLISTADD`) | `0x557100` | adds one guild |
| `0x0D0F` / `0x0D10` | 3 + 50n / 52 | old 50-byte units | `0x556ef0` / `0x556f70` | old guild list / add (no rank, remark or points) |
| `0x0D11` | 6 ✓ | `u32 dwGuildID` (`TP_GUILD_GUILDLISTREMOVE`) | `0x556df0` | removes a guild from the list |
| `0x0D12` / `0x0D13` / `0x0D19` | 3 + 29n | `u8 byCount`, n × `GUILDUSER_UNIT` (`TP_GUILD_USERLISTONLINE`, 3 + 40 × 29) | `0x555310` / `0x555450` / `0x5553b0` | adds members with status 1 / 2 / 3 (*inferred* online / offline / in war zone); my entry sets my rank |
| `0x0D14` | 32 ✓ | `u8 byState, GUILDUSER_UNIT` (`TP_GUILD_USERLISTADD`) | `0x5554f0` | adds one member (status `byState`); shows my guild name over that player |
| `0x0D15` | 6 | `u32 dwUserID` | `0x5555e0` | removes a member, clears the name over that player |
| `0x0D16` | 3 + 28n | `u8 byCount`, n × `GUILDJOINUSER_UNIT` (`TP_GUILD_JOINLIST`) | `0x555630` | join requests (for the master/officers) |
| `0x0D17` | 30 ✓ | `GUILDJOINUSER_UNIT` (`TP_GUILD_JOINLISTADD`) | `0x5556b0` | adds one join request |
| `0x0D18` | 6 ✓ | `u32 dwUserID` (`TP_GUILD_JOINLISTREMOVE`) | `0x555710` | removes it |
| `0x0D20` | 68 ✓ | `u8 byResult, char szRemark[65]` (`TP_GUILD_UPDATEREMARK`) | `0x5564b0` | sets my guild's remark in the guild list (`byResult` unused) |
| `0x0D23` | 6 ✓ | `u32 dwEtin` (`TP_GUILD_GUILD_GETETIN`) | `0x555d20` | my guild's Etin |
| `0x0D24` | 3 + 4n | `u8 byCount`, n × `u16 wGNpcType, u8 byNpcLV, u8 byNumber` (`TP_GUILD_NPC`) | `0x5562c0` | guild house NPC levels |
| `0x0D25` | 7 ✓ | `u8 byResult, u16 wGNpcType, u8 byNpcLV, u8 byNumber` (`TP_GUILD_NPC_UPGRADE`) | `0x5563b0` | 0: NPC level updated; 1 "insufficient Etin" (8003), 2 "upgrade is done one by one level." (8004), 3 "failed to upgrade." (8005), 4 "low Rank" (8006) |
| `0x0D26` | 5 ✓ | `u16 wMapID, u8 byNoticsType` (`TP_GUILD_RANKING_NOTICS`) | `0x555a80` | ranking battle notices with `ui_news001.wav`: type < 24 = "From <b>hour, guild ranking battle will get started later…" (8000), 100 started (8001), 101 10 minutes left (8002), 102 1 minute left (8053) |
| `0x0D27` | 14 | `u32, u32, u32` | `0x555b40` | stored (**unknown**; the PDB's `TP_GUILD_RANKING_GETPOINT` is 8 bytes, but the original server's `CGuild::SendRankingZone` sends `0x0D27` with 14) |
| `0x0D28` | 4 ✓ | `u8 byResultType, u8 byRequire` (`TP_GUILD_HOUSE_ACTION_ERR`) | `0x555d30` | 1 "Your Rank isn't high enough." (8026), 2 "level is not high enough…" (8027), 3 "guild warehouse is full." (8028), 4 "Not authorized" (2038) |
| `0x0D29` | 3 + 33n ✓ | `u8 byCount`, n × bank unit (`TP_GUILD_BANK_ITEM_LIST`) | `0x5508b0` | guild bank contents |
| `0x0D31` | 39 ✓ | bank unit + `u32 dwGuildCharID` (`TP_GUILD_BANK_ITEM_IN`) | `0x5509b0` | sets a bank slot |
| `0x0D32` | 39 ✓ | same (`TP_GUILD_BANK_ITEM_OUT`) | `0x550ac0` | empties the slot |
| `0x0D34` | 7 ✓ | `u8 byResult, u32 dwMoney` (`TP_GUILD_HOUSE_BUY`) | `0x555d90` | 0: money = `dwMoney`, "purchased guild house." (8035); 1 already (8029), 2 "Insufficient gold" (303), 3 not authorized (2038), 4 ranking requirement (8030) |
| `0x0D35` | 3 + 2n | `u8 byCount`, n × `u8 bag, u8 slot` (`TP_GUILD_ETIN_RETURN`) | `0x555c30` | empties those bag slots and sums their Etin (typeID 1–4 = 1, 10, 100, 1000 each): "You return total <v>Etin." (8052) |
| `0x0D36` | 3 ✓ | `u8 byGetPoint` (`TP_GUILD_PVP_WIN`, unused) | `0x555a50` | "In the guild battle,<b>guild is the winner…" (8050) with my guild name |
| `0x0D37` | 3 + 9n | `u8 byCount`, n × `u32 dwGuildID, u32 dwPoint, u8 byRank` (`TP_GUILD_UPDATE`, 3 + 30 × 9) | `0x556510` | new ranking: every guild set to rank 31, then the listed ones; "Each guild's gained point has been settled…" (8054) |
| `0x0D38` | 6 | `u32` | `0x5561c0` | guild house reset: "Guild House server has been reset." (8057), sound and effect |

Not handled: **`0x0D33`** (`TP_GUILD_HOUSE_KEEP_ETIN`, which the original server sends): the client ignores it.

### Client → server

| op | size | fields (PDB) | sent when |
|---|---|---|---|
| `0x0D21` | 92 ✓ | `char szGuildName[25], char szRemark[65]` (`TPC_GUILD_CREATE`) | guild-creation window (name, then remark) |
| `0x0D01` | 27 | `char szGuildName[25]` | older creation path in the same window (confirm case 4); **the original server ignores `0x0D01`** |
| `0x0D22` | 3 ✓ | `u8 byOK` (1 yes, 0 no; `TPC_GUILD_CREATEAGREE`) | creation agreement dialog |
| `0x0D03` | 2 | — | guild window "disband" |
| `0x0D04` | 3 ✓ | `u8 byOK` (1 yes, 0 no; `TPC_GUILD_FIREAGREE`) | disband agreement dialog |
| `0x0D05` | 6 | `u32 dwUserID` | `/sign up for guild` (5080) on the targeted player: invite |
| `0x0D06` | 3 | `u8 byOK` (1 yes, 0 no) | guild invitation dialog |
| `0x0D07` | 6 ✓ | `u32 dwGuildID` (`TPC_GUILD_JOINREQUSER`) | guild list "join request" |
| `0x0D08` | 7 ✓ | `u8 byOK, u32 dwUserID` (`TPC_GUILD_JOINRESULTUSER`) | join-request list: accept (1) / refuse (0) |
| `0x0D09` | 2 ✓ | — (`TPC_GUILD_LEAVE`) | guild window "leave" |
| `0x0D0A` | 6 ✓ | `u32 dwUserID` (`TPC_GUILD_EXPEL`) | member list "kick" |
| `0x0D0C` | 7 | `u8, u32 dwUserID` | member list rank buttons: sender `0x588bc0` (7 bytes: `u8 state, u32 id of the selected member`), called twice from the guild window's click handler `0x4e35e0`: the button object at window `+0x2d8c` sends 0 (`0x4e3c6d`), the one at `+0x3260` sends 1 (`0x4e3cb5`) (confirmed). *inferred* demote/promote; which is which **unknown** (the button labels were not traced) |
| `0x0D20` | 67 ✓ | `char szRemark[65]` (`TPC_GUILD_UPDATEREMARK`) | remark edit |
| `0x0D23` | 2 ✓ | — (`TPC_GUILD_GUILD_GETETIN`) | guild house windows (Etin count) |
| `0x0D25` | 5 ✓ | `u16 wGNpcType, u8 byNpcLV` (`TPC_GUILD_NPC_UPGRADE`) | NPC upgrade |
| `0x0D34` | 6 ✓ | `u32 dwNpcID` (`TPC_GUILD_HOUSE_BUY`) | guild house purchase at the NPC |
| `0x0D35` | 6 ✓ | `u32 dwNpcID` (`TPC_GUILD_ETIN_RETURN`) | return Etin at the NPC |

### What the server must send (original server, **server**)

- **Create** (`0x0D21` request): the requester must be level ≥ 11 and in a party of exactly 7 with no guild. Each other party
  member gets `0x0D22` (agreement dialog) and answers `0x0D22`. When all agree: every party member gets `0x0D21` (98 bytes,
  `byOK = 1`) and the member list (`0x0D12`/`0x0D13`/`0x0D19`), and the guild list of the faction gets `0x0D30`. Failures:
  `0x0D01`/`0x0D21` with the code.
- **Invite** (`0x0D05 id`, rank < 3): the target gets `0x0D05 (inviter)` (30 s); answer `0x0D06`: yes → target joins
  (`0x0D08` 33 bytes with `byOK = 1`, member list, guild bank `0x0D29`, NPC levels `0x0D24`; members get `0x0D14`) and the
  inviter gets `0x0D06 (1, …)`; no → inviter gets `0x0D06 (0, …)`. Invalid target → inviter gets `0x0D06 (0, id)`.
- **Join request** (`0x0D07 guildId`): requester gets `0x0D07 (1)` (or 0; 2 under penalty), the guild's officers get `0x0D17`.
  Officer answers `0x0D08`: accept → as invite-yes, plus `0x0D18` to officers; refuse → requester gets `0x0D08 (0, …)`,
  officers `0x0D18`.
- **Leave** (`0x0D09`): leaver gets `0x0D09 (1)`, the guild gets `0x0D0C (102, id)`. **Kick** (`0x0D0A id`, rank < 3):
  `0x0D0A (1, id)` to the kicked player and the requester, `0x0D0C (103, id)` to the guild.
- **Disband** (`0x0D03`, master): a guild of 2 members or fewer is disbanded at once. Otherwise the officers (rank < 3) in the
  master's party and zone get `0x0D04` (at least 2 needed, else `0x0D03 2`) and answer `0x0D04`; the original server
  disbands after its yes counter passes 2; a "no" cancels it (`CGuildCreate::SendGuildFireFailed`, *inferred* to be `0x0D03 3`
  "party member's rejection"). Disbanded: the guild gets `0x0D0C (101)` and
  the faction `0x0D11`.
- **Login / zone**: members get `0x0D0C (104/105)` when someone connects or disconnects; the guild list is sent as `0x0D0D`,
  `0x0D2F` (≤ 40 per packet), `0x0D0E`.
- Guild chat is `0x1104` (see the [packet catalogue](client-packets.generated.md)).

## Gaps

- `0x0520` party type values; `0x0D27` fields; the member unit's job byte and the guild units' second name (*inferred*).
- The union's `0x0B0B` start/end flag is ignored by the client; whether the original server relies on clearing (it does not
  send `0x0B16` before a new list) was not traced.
- Guild ranks: 1 = master is *inferred* (the server allows kicks and invites for rank < 3).
