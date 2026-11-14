# Every opcode against the original server

This page is one catalogue of every opcode the US EP4 client `game.exe` (built 2010-04-26, patch 108) handles (server → client) or sends
(client → server), with what the original EP4.5 `ps_game.exe` (built 2009-07-15, the PDB build) does with it. It is for people who write a
server, study the protocol or patch the client, and it is the place to look an opcode up by its number.

Sizes include the 2-byte opcode and exclude the length prefix. Messages are `sysmsg-uni.txt` IDs. Field details live on the
[packet catalogue](client-packets.generated.md), [World packets](client-world-packets.md) and [Social packets](client-social-packets.md);
this page links to them rather than repeating them.

## In short

- **One row per [opcode](glossary.md#opcode) and direction.** Each row gives the size, the fields the client reads or writes, what the client does with it, what the original server does, and a status.
- **Status says who uses it.** **both** means the original server and the client both handle the opcode; **client-only** means the client handles or sends it but the original never does; other values are written out in the row.
- **The client knows more than the original server uses.** 41 packets the client handles are never sent by the original server.
- **One mismatch in the original**: the original server answers lapis extraction with 34 bytes, but the US client reads 33, so the gold shown goes wrong until the next money update (see below).
- **Warehouse gold looks possible but is not**: the client has the request and the answer, but never creates the window that would send it.

### Using this page

- **If you write a server:** the opportunities this catalogue opens up are on [Opportunities the client already supports](afterglow/opportunities.md); [What a server must do for this client](afterglow/server-checklist.md) collects the advice.
- **If you patch or mod the client:** the features a patch could switch on are on [Region features](region-features.md).
- **To check a claim yourself:** read the "How the columns were filled" table below, then redo a row with the method in [Tools and method](tools-and-method.md).

Status words:

- **confirmed**: read in a Ghidra decompile of this `game.exe` or of `ps_game.exe` (names from the server's PDB);
- **confirmed (disasm)**: read in the disassembly only;
- **ref**: from the server pages of this site (themselves from `ps_game.exe` + PDB), not re-read here;
- *inferred*: indirect evidence (names, message IDs, layout);
- **live**: needs one logged try in game (none was done: no client patch and no live test were made for this page).

## How the columns were filled

| column | source | limits |
|---|---|---|
| fields | the client's reads (`if:` / `loop:` = conditional / repeated) | readers only; see the per-family pages for meanings |
| client | the per-opcode tables of [World packets](client-world-packets.md), [Social packets](client-social-packets.md) and the client's action notes (page named after the text), or a note from this pass (address given) | rows marked *EP6/PDB name* were not traced beyond the name |
| original ps_game, S→C | **every instruction in `ps_game.exe` that stores the opcode as a 16-bit immediate** (`MOV word ptr […], 0xNNNN`), grouped by function, minus functions that talk to other servers (`SendDB*`, `*DBAgent*`, `CClientToMgr`, `CMonitorNet`, game log, sockets) | **confirmed (disasm)**, with two blind spots handled by hand: opcodes computed at run time (`0x0508`/`0x051E` in `CUser::LevelUp`, `0x1C01`/`0x1C02` in `CMarketMgr::SearchMarketFirst`) and replies that echo the request (`0x0208`, `0x2305`, `0x2306`). Small values collide with other servers' opcodes in the same function; the function names say which situation sends it. "never" = no instruction in `ps_game.exe` uses the value as a 16-bit store |
| original ps_game, C→S | the `case` labels of the `CUser::Packet*` dispatchers (decompiled), plus the compare chains (`PacketGetInfo`, `PacketNpc`, `PacketPriest`, `PacketUQ`, `PacketBilling`, `PacketPVPGuild`, `PacketProcessing`) | **confirmed**. "ignored" = no case in any dispatcher; `PacketProcessing` has **no group for `0x1Dxx`, `0x27xx`** and drops unknown groups |
| status | **both**: the original sends or handles it and the client handles or sends it; **client-only**: the client handles or sends it, the original never does; other values as written | GM opcodes (`F1xx`–`FAxx`) are listed but were not checked one by one here (see [GM opcodes and commands](gm-and-commands.md)) |


## The extraction answer

Confirmed. The original server always answers lapis extraction with the **34-byte** `0x0802` (see
[Lapis and enchanting](../original/lapis.md#4-extraction-0x0802)), but this client reads **33 bytes** (no `gemPos`; `0x57cb40`, region
feature 0 off; see the [client facts](CLIENT-FACTS.md), "Compared with the server pages"). The extra byte shifts every field after it by
one, so the gold shown is wrong until the next money update. What a server does about it is on
[Opportunities the client already supports](afterglow/opportunities.md#the-extraction-answer).

## Region features

The client's per-region feature switches, with what each controls and what flipping it would do, are on [Region features](region-features.md).

## Opportunities for a server

The packet forms the client already supports that a server could use with no client change are on
[Opportunities the client already supports](afterglow/opportunities.md).

## Catalogue by family

One row per opcode and direction. `client` cells end with the page they come from; cells without one are from this pass.

### 00xx — connection

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0002` | S→C | var | — | disconnect: "connection with server has been disconnected" (10069), closes | closes connections (`SAgent`), not a client packet | both |
| `0x0003` | C→S | 2 | — | keepalive | handled: SAgent::PacketSystem | both |
| `0x0004` | S→C | 3 | u8 | kicked: `u8` 0 → message box "disconnect due to using logged in ID." (10070), else 10071 "system error"; then closes the game (`0x54c860`, confirmed) | not from ps_game (the scan cannot tell: the value 4 is everywhere); *inferred* login/session duplicate-login kick | client handles it; sender not settled |

### 01xx — character screen, world entry, logout

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0101` | S→C | var | u8 u32 if:u32 if:u16 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16… | *EP6/PDB name* GameCharListOutgoing (TP_CHAR_LIST) | sends: CUser::PacketUserDBChar | both |
| `0x0102` | S→C | 3 | u8 | create result (`u8`): 0 ok, others = name/slot errors (`docs/original/ps_game.md` §6) | sends: CUser::PacketCharacter, CUser::PacketUserDBChar | both |
| `0x0102` | C→S | var | — | create a character (name + looks) | handled: CUser::PacketCharacter | both |
| `0x0103` | S→C | 7 | u8 u32 | delete result `u8, u32 charId` | sends: CUser::PacketUserDBChar | both |
| `0x0103` | C→S | 6 | +2:u32 | *EP6/PDB name* GameCharDeleteIncoming | handled: CUser::PacketCharacter | both |
| `0x0104` | S→C | 7 | u8 u32 | *EP6/PDB name* GameUserEnterWorldOutgoing | sends: CUser::ItemBagToBank, CUser::ItemGemRemovePos, CUser::PacketUserDBChar | both |
| `0x0104` | C→S | 6 | +2:u32 | *EP6/PDB name* GameCharEnterWorldIncoming | handled: CUser::PacketCharacter | both |
| `0x0105` | S→C | var | u16 u16 u16 u16 u16 u16 u32 u32 u32 u32 u32 u32 u32 u32 u32 u32 u32 b… | own character data (stats, money, position, `char guildName[25]` tail; an empty tail is fine) | sends: CUser::SendCharacterData | both |
| `0x0106` | S→C | var | u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 l… | inventory list: count + item units (bag, slot, type, typeId, quality, gems, count, craft name) | sends: CUser::SendCharacterItem | both |
| `0x0107` | S→C | var | — | *EP6/PDB name* GameUserLeaveWorldOutgoing | sends: CWorld::LogoutUser | both |
| `0x0107` | C→S | 2 | — | *EP6/PDB name* GameCharLeaveWorldIncoming | handled: CUser::PacketCharacter | both |
| `0x0108` | S→C | var | u8 loop:u16 loop:u8 loop:u8 loop:u32 | skill list: `u8 n`, n × (`u16 skill, u8 level, u8 ?, u32 cooldown`) | sends: CUser::SendCharacterSkill | both |
| `0x0109` | S→C | 4 | u8 u8 | *EP6/PDB name* GameCharCountryOutgoing (TP_CHAR_COUNTRY) | sends: CUser::PacketUserDBChar | both |
| `0x0109` | C→S | 3 | +2:u8 | choose the faction (character screen) | handled: CUser::PacketCharacter | both |
| `0x010A` | S→C | var | u8 var | buffs on the character at login: `u8 n`, n × 11 bytes | sends: CUser::SendCharacterSetSkill | both |
| `0x010B` | S→C | var | u8 var | quick slots: `u8 n`, n × (`u8 bar, u8 slot, u8 bag, u16 number`) | sends: CUser::SendCharacterQuick | both |
| `0x010B` | C→S | var | — | save the quick slots (on logout) | handled: CUser::PacketProcessing | both |
| `0x010D` | S→C | var | — | quit confirmed; the client closes | sends: CWorld::LogoutUser | both |
| `0x010D` | C→S | 2 | — | quit the game | handled: CUser::PacketProcessing | both |
| `0x010E` | S→C | 3 | u8 | *EP6/PDB name* GameCharRenameOutgoing (TP_CHAR_RENAME) | sends: CUser::PacketCharacter, CUser::PacketUserDBChar | both |
| `0x010E` | C→S | 27 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u32 +26:u8 | *EP6/PDB name* GameCharRenameIncoming (TPC_CHAR_RENAME) | handled: CUser::PacketCharacter | both |
| `0x010F` | S→C | 7 | u8 u32 | restore result `u8, u32 charId` | sends: CUser::PacketUserDBChar | both |
| `0x010F` | C→S | 6 | +2:u32 | *EP6/PDB name* GameCharRestoreIncoming | handled: CUser::PacketCharacter | both |
| `0x0110` | S→C | 3 | u8 | `u8`: 0 → leaves the world into a waiting state (char object state 9, scene 5); 1 → "server saturated. Please connect later" (10096); 2 → "checking server. Please check notice board" (10059) (`0x54c9b0`, confirmed) | sends `0x0110 02` from `PacketMaininterface` case `0x227` (portal to map 200 while the other server is unavailable); the other stores of the value are other servers’ packets | both |
| `0x0111` | S→C | 38 | u32 bytes[32] | `u32, char[32]`: recreates the connection object and stores the u32: reconnect to another game server (with `0x0112`/`0x0113`) (`0x54c910`, confirmed; purpose *inferred* from the original's reconnect path, `docs/original/world.md`) | sends: `CWorld::LogoutUser` (reconnect to another server) | both |
| `0x0112` | S→C | 8 | u32 u16 | `u32 objectId, u16 map`: sets the local object id and map (reconnect path) (`0x54ca50`, confirmed) | sends: `CUser::PacketUserDBChar` (reconnect path only) | both |
| `0x0113` | S→C | var | — | reconnect variant of `0x010B`: does nothing visible (thunk to an empty function, confirmed) | sends: `CUser::PacketUserDBChar` (reconnect path, instead of `0x010B`) | both |
| `0x0114` | S→C | 6 | u32 | *EP6/PDB name* GameCharServerTimeOutgoing (TP_CHARACTER_SERVERTIME) | sends: CUser::PacketUserDBChar | both |

### 02xx — main interface: items, stats, skills, portals, mounts, resets

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0201` | S→C | 29 | u32 u8 u16 u32 u32 u32 u32 u32 | a player enters the view (id, country, map position…) | sends: CCell::SendCellInfoEnter, CZone::EnterUser, CZone::SendMoveUser | both |
| `0x0201` | C→S | 2 | — | ready in the zone (after loading) | handled: `CUser::PacketProcessing` (state 3: enter the zone) | both |
| `0x0202` | S→C | 6 | u32 | a player leaves the view | sends: CCell::SendCellInfoLeave, CZone::LeaveUser, CZone::SendMoveUser … | both |
| `0x0204` | S→C | var | u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 bytes[21]* u8 u8 u8 u8 u8 u16 u8… | item move result: both slots rewritten (type 0 = empty). Bag 100 = warehouse, 255 = guild warehouse, **200 = the web inventory** (`27xx`, this doc); 74-byte form with the money for warehouse withdrawals (`client-world-packets.md`) | sends: CGuild::ItemBagToGuildBank, CGuild::ItemGuildBankToBag, CGuild::ItemGuildBankToGuildBank … | both |
| `0x0204` | C→S | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | *EP6/PDB name* GameItemBagToBagIncoming (TPC_ITEM_BAGTOBAG) | handled: CUser::PacketMaininterface | both |
| `0x0205` | S→C | var | u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:u8 if:u8 if:u8 if:u8 if:u8… | pickup result: stores the item, `ch_pikup001.wav`, "Attained <i>" (52) or for gold "Gained <b>G" (53) and adds to money. Error codes: 0 "This doesn't belong to you" (202… (`client-world-packets.md`) | sends: CUser::ExchangeItemAdd, CUser::ItemCreate, CUser::ItemGet … | both |
| `0x0205` | C→S | 6 | +2:u32 | *EP6/PDB name* GameItemGetIncoming | handled: CUser::PacketMaininterface | both |
| `0x0206` | S→C | 7 | u8 u8 u8 u8 u8 | the slot after a drop: `byCount` = what is left (0 empties it); bag 0 also removes the equipment's look/effects (`client-world-packets.md`) | sends: CUser::ExchangeItemAdd, CUser::ExchangePvP, CUser::ItemDrop … | both |
| `0x0206` | C→S | 5 | +2:u8 +3:u8 +4:u8 | *EP6/PDB name* GameItemDropIncoming (TPC_ITEM_DROP) | handled: CUser::PacketMaininterface | both |
| `0x0207` | S→C | var | if:u32 if:u32 u32 | experience: two `u32` with region feature 17 (on): adds to EXP; level-up packets separate | sends: CMap::ObeliskDestroy, CMap::ObeliskReset, CMap::SendPortalLinkChange … | both |
| `0x0208` | S→C | 14 | u16 u16 u16 u16 u16 u16 | stat points spent (echo of the request): adds each, subtracts the sum from free points; no message (the client's action notes, §1) | echo: `PacketMaininterface` case `0x208` sends the request back (`SConnection::Send(…, 0xe)`) | both |
| `0x0208` | C→S | 14 | +2:u16 +4:u16 +6:u16 +8:u16 +10:u16 +12:u16 | *EP6/PDB name* GameUserStatusUpIncoming | handled: CUser::PacketMaininterface | both |
| `0x0209` | S→C | 7 | u8 u8 u16 u8 | *EP6/PDB name* GameUserSkillGetOutgoing (TP_USER_GETSKILL) | sends: CUser::EnableLearnSkill, CUser::LearnSkill, CUser::SendGetSkill | both |
| `0x0209` | C→S | 5 | +2:u16 +4:u8 | *EP6/PDB name* GameUserSkillGetIncoming (TPC_USER_GETSKILL) | handled: CUser::PacketMaininterface | both |
| `0x020A` | S→C | 4 | u8 u8 | *EP6/PDB name* GameUserPortalUseOutgoing (TPC_USER_USEPORTAL) | sends: CUser::PacketMaininterface | both |
| `0x020A` | C→S | 3 | +2:u8 | *EP6/PDB name* GameUserPortalUseIncoming (TPC_USER_USEPORTAL) | handled: CUser::PacketMaininterface | both |
| `0x020B` | S→C | 20 | u32 u16 u32 u32 u32 | *EP6/PDB name* GameUserSetMapPosOutgoing (TP_USER_SETMAPPOS) | sends: CUser::PacketPC, CUser::Rebirth, CUser::Soul … | both |
| `0x020C` | S→C | 7 | u8 u32 | *EP6/PDB name* GameUserGateKeeperUseOutgoing (TP_USER_USEGATENPC) | sends: CUser::PacketMaininterface | both |
| `0x020C` | C→S | 7 | +2:u32 +6:u8 | *EP6/PDB name* GameUserGateKeeperUseIncoming (TPC_USER_USEGATENPC) | handled: CUser::PacketMaininterface | both |
| `0x020D` | S→C | 7 | u8 u32 | **warehouse gold**: `u8 dir, u32 amount`. 0 → "Found <v>G in warehouse" (236), money + amount, warehouse money − amount; 1 → "Stored <v>G in warehouse" (237), the reverse (`0x54d6e0`, confirmed) | never (no opcode store) | **client-only** |
| `0x020D` | C→S | 7 | +2:u8 +3:u32 | **warehouse gold**: `u8 dir` (0 withdraw, 1 deposit), `u32 amount`, from a number dialog in either warehouse window (`0x49d210`, `0x4fd530`, confirmed) that **no code ever creates**: dead without a client change | ignored (no case) | **dead** |
| `0x020E` | S→C | 7 | u8 u32 | *EP6/PDB name* GameUserKillCountOutgoing (TP_USER_KILLCOUNT) | sends: CUser::KillPCFrom, CUser::KillPCTo | both |
| `0x020F` | S→C | 7 | u8 u32 | `u8 faction, u32 count`: per-faction kill count, sent to every player of that faction (**ref** [Player kills 2](../original/pvp-2.md#9-the-counts-and-grades), `CWorldThread::SendAllKillCount`) | sends: CWorldThread::SendAllKillCount | both |
| `0x0210` | S→C | 3 | u8 | *EP6/PDB name* GameUserSetRunOutgoing (TP_USER_SETRUN) | sends: CUser::RemApplySkillByOtherSkill, CUser::RemApplySkillHide, CUser::RemApplySkillToMOB … | both |
| `0x0210` | C→S | 2 | — | walk/run toggle | handled: CUser::PacketMaininterface | both |
| `0x0211` | S→C | 11 | u8 u32 u32 | per-faction value with a countdown run by the client (the faction's kill count and the time left of its top grade; the Goddess Blessing item raises the grade, **ref** [Player kills 2](../original/pvp-2.md#9-the-counts-and-grades); client facts) | sends: CUser::SendKCStatus | both |
| `0x0212` | S→C | var | — | auto-attack started/stopped | sends: CUser::SendAttackStart | both |
| `0x0212` | C→S | 2 | — | stop the auto-attack | handled: CUser::PacketMaininterface | both |
| `0x0213` | S→C | 6 | u32 | *EP6/PDB name* GameUserSetMoneyOutgoing (TP_USER_SETMONEY) | sends: CGuildMgr::CreateGuild, CMiniGame::GameExit, CUser::ExchangePvP … | both |
| `0x0214` | S→C | var | u8 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 | stat reset answer: `u8 ok`, then free points + 6 stats (order STR REC INT WIS DEX LUC) → 5239/5240 (the client's action notes, §2) | sends: CUser::PacketMaininterface, CUser::StatResetStatus | both |
| `0x0214` | C→S | 2 | — | stat reset (NPC type 13, or the item) | handled: CUser::PacketMaininterface | both |
| `0x0215` | S→C | var | u8 if:u16 if:u8 | skill reset answer: `u8 ok`, `u16 points`, **optional `u8`** (counts the free reset down when ≠ 0) → 5241/5242 | sends: CUser::PacketMaininterface, CUser::StatResetSkill | both |
| `0x0215` | C→S | 2 | — | skill reset (bags III+ empty) | handled: CUser::PacketMaininterface | both |
| `0x0216` | S→C | 4 | u8 u8 | mount: `u8 ok, u8 on` → 5245 / 5243 got on / 5244 got off | sends: CUser::ItemBagToBag, CUser::PacketMaininterface, CUser::UpdateRebirth | both |
| `0x0216` | C→S | 2 | — | mount / dismount (Summon Stone, equipment slot 13) | handled: CUser::PacketMaininterface | both |
| `0x0217` | S→C | 6 | u32 | mount summon started (cast bar), `u32` | sends: CUser::PacketMaininterface | both |
| `0x0218` | S→C | 4 | u8 u8 | *EP6/PDB name* GameStatusResultInfoOutgoing (TP_STATUS_RESULTINFO) | sends: CUser::StatusKillDeathResult | both |
| `0x0218` | C→S | 2 | — | *EP6/PDB name* GameStatusResultInfoIncoming | handled: CUser::PacketMaininterface | both |
| `0x0219` | S→C | 4 | u8 u8 | *EP6/PDB name* GameStatusResultKillOutgoing (TP_STATUS_RESULTKILL) | sends: CUser::StatusKillResult | both |
| `0x0219` | C→S | 2 | — | *EP6/PDB name* GameStatusResultKillIncoming | handled: CUser::PacketMaininterface | both |
| `0x021A` | S→C | 7 | u8 u32 | *EP6/PDB name* GameStatusResultDeathOutgoing (TP_STATUS_RESULTDEATH) | sends: CUser::StatusDeathResult | both |
| `0x021A` | C→S | 2 | — | *EP6/PDB name* GameStatusResultDeathIncoming | handled: CUser::PacketMaininterface | both |
| `0x021B` | S→C | 4 | u8 u8 | free reset "chances left" counts (`u8 stat, u8 skill`), shown by NPC type 13 | sends: CUser::PacketUserDBChar | both |
| `0x021C` | S→C | 10 | u32 u32 | riding together: `u32, u32` (passenger, driver) | sends: CUser::SendShapeVehicle2, CWorld::ZoneLeaveUserMove, CZone::MoveUser | both |
| `0x021C` | C→S | 2 | — | leave a shared mount (key binding) | handled: CUser::PacketMaininterface | both |
| `0x021D` | S→C | 6 | u32 | ride-together invitation from `u32` (dialog) | sends: CUser::PacketMaininterface | both |
| `0x021D` | C→S | 6 | +2:u32 | invite a player onto the mount (`u32`) | handled: CUser::PacketMaininterface | both |
| `0x021E` | S→C | 3 | u8 | ride-together answer `u8` | sends: CUser::CancleActionVehicle2, CUser::LeaveZone, CUser::PacketMaininterface | both |
| `0x021E` | C→S | 3 | +2:u8 | answer a ride invitation (`u8`) | handled: CUser::PacketMaininterface | both |
| `0x0220` | S→C | 18 | u8 u8 u16 u32 u32 u32 | Teleportation Stone save answer: 1 → 441, 2 → 227, 0 → slot stored | sends: CUser::PacketMaininterface | both |
| `0x0220` | C→S | 17 | +2:u8 +3:u16 +5:f32 +9:u32 +13:u32 | Teleportation Stone: save a point (`u8 index 1–4, u16 map, f32 x, y, z`) | handled: CUser::PacketMaininterface | both |
| `0x0221` | S→C | 6 | u32 | *EP6/PDB name* GameUserItemCastOutgoing | sends: CUser::ItemUse | both |
| `0x0222` | S→C | 7 | u32 u8 | move-by-item cast ended `u32, u8` | sends: CUser::CancleMove, CUser::UpdateResetPosition | both |
| `0x0223` | S→C | 6 | u32 | party summon cast `u32` | sends: CUser::UpdateResetPosition | both |
| `0x0224` | S→C | 7 | u32 u8 | party summon: "Do you want to accept <p>'s party member summon?" (818) | sends: CUser::ItemGet, CUser::PacketMaininterface, CUser::UpdateTimeover | both |
| `0x0224` | C→S | 3 | +2:u8 | answer a party summon (`u8`) | handled: CUser::PacketMaininterface | both |
| `0x0225` | S→C | var | u8 loop:bytes[15] | Teleportation Stone saved points: `u8 n`, n × 15 (never more than 4, never index 0) | sends: CUser::SendSavePoint | both |
| `0x0226` | S→C | 10 | u32 u8 u8 u8 u8 | *EP6/PDB name* GameUserBodyChangeOutgoing (TP_BODYCHG) | sends: CUser::PacketMaininterface | both |
| `0x0226` | C→S | 8 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 | *EP6/PDB name* GameUserBodyChangeIncoming (TPC_BODYCHG) | handled: CUser::PacketMaininterface | both |
| `0x0227` | C→S | 3 | +2:u8 | use a portal that asks first (dialog 0x17, yes): `u8 portal` (`0x48c120` → `0x5898e0`, confirmed) | handled: CUser::PacketMaininterface | both |
| `0x0228` | S→C | var | — | gender change refused: 862 | sends: CUser::ItemRemake, CUser::PacketMaininterface | both |
| `0x0230` | S→C | 4 | u16 | *EP6/PDB name* GameCharHonorOutgoing (TP_CHARACTER_HONOR) | sends: CUser::PacketMyShop, CUser::SendCharacterHonor | both |

### 03xx — information requests

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0301` | S→C | 12 | u32 u16 u16 u8 u8 | player info (`u32 id, u16, u16, u8, u8`) | sends: CUser::Death, CUser::DeathByNPC, CUser::PacketGetInfo | both |
| `0x0301` | C→S | 6 | +2:u32 | ask for a player's info | handled: CUser::PacketGetInfo | both |
| `0x0302` | S→C | 10 | u32 u16 u16 | *EP6/PDB name* GameGetInfoUserHealthOutgoing (TP_USER_GETHP) | sends: CUser::KillPCTo, CUser::PacketGetInfo, CWorld::LeaveUser | both |
| `0x0302` | C→S | 6 | +2:u32 | *EP6/PDB name* GameGetInfoUserHealthIncoming | handled: CUser::PacketGetInfo | both |
| `0x0303` | S→C | var | u32 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u32 loop:u8 loop:u8 loop:u8 byte… | *EP6/PDB name* GameGetInfoUserShapeOutgoing_EP5 (TP_USER_GETSHAPE) | sends: CUser::Death, CUser::SendUserShape, CZone::SendUserShape | both |
| `0x0303` | C→S | 6 | +2:u32 | ask for a player's looks | handled: CUser::PacketGetInfo | both |
| `0x0304` | S→C | 12 | u32 u32 u8 u8 | mob info answer (`u32 id, u32 HP, u8, u8`) | sends: CMob::SendState, CUser::PacketGetInfo, CUser::Rebirth … | both |
| `0x0304` | C→S | 6 | +2:u32 | ask for a mob's info | handled: CUser::PacketGetInfo | both |
| `0x0305` | S→C | 10 | u32 u32 | *EP6/PDB name* GameGetInfoMobHealthOutgoing (TP_MOB_GETHP) | sends: CMob::ResetAttack, CMob::SendHP, CUser::KillSoulForDisconnect … | both |
| `0x0305` | C→S | 6 | +2:u32 | *EP6/PDB name* GameGetInfoMobHealthIncoming | handled: CUser::PacketGetInfo | both |
| `0x0307` | S→C | var | u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:bytes[6] loop:bytes[21]* | *EP6/PDB name* GameGetInfoUserItemsOutgoing (TP_USER_ITEMS) | sends: CUser::LeaveWorld, CUser::PacketGetInfo | both |
| `0x0307` | C→S | 6 | +2:u32 | `0x0307` `TP_USER_ITEMS` (equipment list). confirmed (disasm) (the client's action notes) | handled: CUser::PacketGetInfo | both |

### 04xx — world state, ground items, stalls

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0401` | S→C | 22 | u32 u8 u8 u8 u8 u32 u32 u32 | item on the ground (model from type/typeID). `byKind` = drop height for the falling animation: 0 none, 1 → 0.1, 2 → 1.0, 3 → 2.0. Type 44/typeID 246 is a special ground… (`client-world-packets.md`) | sends: CCell::SendCellInfoEnter, CUser::ItemCreate, CUser::ItemGet … | both |
| `0x0402` | S→C | 6 | u32 | removes the ground item (`client-world-packets.md`) | sends: CCell::SendCellInfoLeave, CUser::ItemDrop, CUser::ItemGemAdd … | both |
| `0x0403` | S→C | 4 | u8 u8 | **ignored** (empty function) (`client-world-packets.md`) | sends: CCell::SendCellInfoEnter, CDoor::UpdateAutoClose, CDoor::Use … | ignored by the client |
| `0x0404` | S→C | 6 | u32 | world clock in seconds; advanced locally (client facts) | sends: CUser::PacketShop, CUser::PacketUserDBChar, CUser::ProcessCharacterCreate | both |
| `0x0405` | S→C | var | u32 u32 u32 u32 u16 u8 u8 u8 u8 u8 u8 u8 u64 u64 bytes[21]* | a personal-shop / corpse-like object enters the view: ids, position, model by race (`data/Character/Human`, `Elf`, `DeathEater`), `u64` ×2, `char[21]` (*inferred* the market stall of `0x23xx`) | sends: CCell::SendCellInfoEnter, CUser::ExchangeItemAdd, CUser::PacketMyShop | both |
| `0x0406` | S→C | 6 | u32 | that object leaves the view | sends: CCell::SendCellInfoLeave, CUser::ExchangeItemAdd, CUser::KillSoul … | both |
| `0x0451` | S→C | 5 | u8 u8 u8 | weather (`u8 kind, u8, u8`): rain plays `Rain.wav` | sends: CWorldThread::Update, CZone::SendWeather, CZone::UpdateWeather | both |

### 05xx — actions, combat, skills, item use

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0501` | S→C | 21 | u32 u8 u16 u32 u32 u32 | *EP6/PDB name* GameCharMoveOutgoing (TP_PC_MOVE) | sends: CUser::PacketGetInfo, CUser::PacketPC, CUser::QuestStart … | both |
| `0x0501` | C→S | 17 | +2:u8 +3:u16 +5:f32 +9:u32 +13:u32 | *EP6/PDB name* GameCharMoveIncoming (TPC_PC_MOVE) | handled: CUser::PacketPC | both |
| `0x0502` | S→C | 17 | u8 u32 u32 u16 u16 u16 | player hits player: attack animation, pending damage applied when the blow lands (unknown attacker: at once). Local attacker + result ≥ 5: error message (`client-world-packets.md`) | sends: CUser::AttackUser, CUser::PacketPC, CUser::QuestEnd … | both |
| `0x0502` | C→S | 6 | +2:u32 | *EP6/PDB name* GameCharAttackUserIncoming | handled: CUser::PacketPC | both |
| `0x0503` | S→C | 17 | u8 u32 u32 u16 u16 u16 | player hits mob, same pattern (`client-world-packets.md`) | sends: CUser::AttackMob, CUser::SendAttackError | both |
| `0x0503` | C→S | 6 | +2:u32 | *EP6/PDB name* GameCharAttackMobIncoming | handled: CUser::PacketPC | both |
| `0x0504` | S→C | 11 | u32 u8 u32 | player death animation if HP < 1; attackers stop; for the local player opens the death/revive window (`FUN_004ca4b0`) (`client-world-packets.md`) | sends: CUser::SendDeath | both |
| `0x0505` | S→C | 12 | u32 u16 u16 u16 | **adds** the three (signed) values, clamped at 0 (`client-world-packets.md`) | sends: CUser::SendRecoverAdd, CUser::SendRecoverAddSet, CUser::SendRecoverChange … | both |
| `0x0506` | S→C | 7 | u32 u8 | *EP6/PDB name* GameCharMotionOutgoing (TP_PC_MOTION) | sends: CUser::PacketPC | both |
| `0x0506` | C→S | 3 | +2:u8 | *EP6/PDB name* GameCharMotionIncoming | handled: CUser::PacketPC | both |
| `0x0507` | S→C | var | u32 u8 u8 u8 u8 if:bytes[6] | *EP6/PDB name* GameCharEquipmentOutgoing (TP_PC_EQUIPMENT) | sends: CUser::SendEquipment | both |
| `0x0508` | S→C | 20 | u32 u16 u16 u16 u32 u32 | the same update, effect and sound **deferred**: sets a pending flag on the player (`+0x1c9`); `FUN_00416c60` plays the level-up effect and `ch_levup001.wav` when the nex… (`client-world-packets.md`) | sends: `CUser::LevelUp` (opcode computed: `0x508`, or `0x51E` when someone else made the kill) | both |
| `0x0509` | S→C | var | — | **explicitly ignored** by the client | never (no opcode store) | ignored by the client |
| `0x050A` | S→C | 11 | u32 u8 u8 u8 u8 u8 | *EP6/PDB name* GameCharItemUseOutgoing (TP_PC_USEITEM) | sends: CUser::ItemUseError, CUser::ItemUseNSend | both |
| `0x050A` | C→S | 4 | +2:u8 +3:u8 | sender(s) `005861a0` | handled: CUser::PacketPC | both |
| `0x050B` | S→C | 9 | u32 u8 u16 | sets one maximum: type **0 HP, 1 SP, 2 MP** (`client-world-packets.md`) | sends: CUser::SendMaxHP, CUser::SendMaxMP, CUser::SendMaxSP | both |
| `0x050C` | S→C | 6 | u16 u16 | subtracts a skill's cost from the local player's SP and MP (`client-world-packets.md`) | sends: CUser::SendUseSPMP | both |
| `0x050D` | S→C | 9 | u32 u16 u8 | adds/updates a buff on the local player; the u32 is the key for `0x050E`; duration from `Skill.SData` (+0x46 seconds; hours if flag +0x7F = 1; 30 s for type 6) (`client-world-packets.md`) | sends: CUser::SendSkillApply | both |
| `0x050E` | S→C | 6 | u32 | removes that buff (message 837 for the resurrection rune) (`client-world-packets.md`) | sends: CUser::ChkEndTimeSkill, CUser::ItemEquipmentRem, CUser::RemApplySkillSleep … | both |
| `0x050F` | S→C | 15 | u32 u16 u8 u16 u16 u16 | periodic skill effect on a player (heal or damage by skill type) (`client-world-packets.md`) | sends: CUser::ChkKeepSkill, CUser::RemApplySkillBuff, CUser::RemApplySkillDebuff | both |
| `0x0510` | S→C | 13 | u32 u32 u16 u8 | starts the cast animation toward a player (`client-world-packets.md`) | sends: CUser::UseSkillUser | both |
| `0x0511` | S→C | 20 | u8 u32 u32 u16 u8 u16 u16 u16 | skill on a player/self: effects, damage or heal, messages 1106/1108/1207/1208/1305/1307 (`client-world-packets.md`) | sends: CUser::AttackUser, CUser::BuffUser, CUser::ChkTargetUser … | both |
| `0x0511` | C→S | 7 | +2:u8 +3:u32 | sender(s) `00586250` | handled: CUser::PacketPC | both |
| `0x0512` | S→C | 26 | u8 u32 u32 u16 u8 u16 u16 u16 u16 u16 u16 | drain on a player (`client-world-packets.md`) | sends: CUser::AttackUser | both |
| `0x0513` | S→C | 20 | u8 u32 u32 u16 u8 u16 u16 u16 | area hit on one player ("Received <v> damage by <s>", 1308/1309) (`client-world-packets.md`) | sends: CUser::AttackUser, CUser::BuffUser, CUser::ChkProtect … | both |
| `0x0514` | S→C | 19 | u32 u32 u16 u8 u16 u16 u16 | additional damage on a player (`client-world-packets.md`) | sends: CUser::AttackUser | both |
| `0x0515` | S→C | 16 | u32 u32 u16 u16 u16 | mirrored damage between players (`client-world-packets.md`) | sends: CUser::AttackUser, CUser::ChkKeepSkill, CUser::RemApplySkillDebuff | both |
| `0x0516` | S→C | 13 | u32 u32 u16 u8 | cast start toward a mob (`client-world-packets.md`) | sends: CUser::UseSkillMob | both |
| `0x0517` | S→C | 20 | u8 u32 u32 u16 u8 u16 u16 u16 | skill on a mob (`client-world-packets.md`) | sends: CUser::AttackMob, CUser::ChkTargetMob, CUser::EnableUseSkillMob … | both |
| `0x0517` | C→S | 7 | +2:u8 +3:u32 | sender(s) `00586290` | handled: CUser::PacketPC | both |
| `0x0518` | S→C | 26 | u8 u32 u32 u16 u8 u16 u16 u16 u16 u16 u16 | drain on a mob (`client-world-packets.md`) | sends: CUser::AttackMob | both |
| `0x0519` | S→C | 20 | u8 u32 u32 u16 u8 u16 u16 u16 | area hit on one mob ("Wounded <t> for <v>", 1101/1102) (`client-world-packets.md`) | sends: CUser::AttackMob, CUser::ChkProtect, CZone::UpdateTrab | both |
| `0x051A` | S→C | 19 | u32 u32 u16 u8 u16 u16 u16 | additional damage on a mob (`client-world-packets.md`) | sends: CUser::AttackMob | both |
| `0x051B` | S→C | 16 | u32 u32 u16 u16 u16 | mirrored damage on a mob (`client-world-packets.md`) | sends: CMob::Attack, CMob::AttackUser | both |
| `0x051C` | S→C | 8 | u32 u8 u8 | player speeds (`client-world-packets.md`) | sends: CCell::SendCellInfoEnter, CUser::ItemEquipmentAdd, CUser::ItemEquipmentRem … | both |
| `0x051D` | S→C | 7 | u32 u8 | *EP6/PDB name* GameCharShapeOutgoing_EP5 (TP_PC_SHAPE) | sends: CCell::SendCellInfoEnter, CUser::SendShape, CUser::SendShapeMe … | both |
| `0x051E` | S→C | 20 | u32 u16 u16 u16 u32 u32 | **level up** with effect and `ch_levup001.wav`; for the local player updates level, points and the exp bar; message 217 (self) / 218 (party) (`client-world-packets.md`) | sends: `CUser::LevelUp` (computed, with `0x0508`) | both |
| `0x051F` | S→C | 12 | u32 u16 u16 u16 | sets all three maxima (`client-world-packets.md`) | sends: CUser::SendEnergyAllMax | both |
| `0x0520` | S→C | 7 | u32 u8 | party type of a player (`u32 id, u8`) | sends: CParty::BreakParty, CParty::EnterNormalParty, CParty::EnterRaidParty … | both |
| `0x0521` | S→C | 8 | u16 u16 u16 | sets the local player's current values (absolute) (`client-world-packets.md`) | sends: CUser::SendRecoverAddSet, CUser::SendRecoverSet, CUser::StatResetSkill … | both |
| `0x0522` | S→C | 10 | u32 u32 | player-kill info `u32 kills, u32 deaths` | sends: CUser::KillPCTo, CUser::PacketAdminCmdB | both |
| `0x0523` | S→C | var | u32 u8 if:u8 if:var* | personal-shop sign over a player: `u32 id, u8 open`, then the title (`u8 len, text`) when open | sends: CUser::PacketMyShop, CZone::SendUserShape, stMyShop::Ended … | both |
| `0x0524` | S→C | 9 | u32 u8 u16 | a mob changes shape (`u32, u8, u16`) | sends: CUser::SendShape | both |
| `0x0525` | S→C | 4 | u16 | "<v> absorption damage." (1011) (`client-world-packets.md`) | sends: CUser::Damage | both |
| `0x0551` | S→C | 23 | u32 u8 u32 u32 u32 u32 | rebirth: `u32 id, u8 kind, u32 map/x/y/z` | sends: CUser::LifeSkill, CUser::Rebirth, CUser::Soul | both |
| `0x0553` | C→S | 3 | +2:u8 | *EP6/PDB name* GameCharRebirthIncoming | handled: CUser::PacketPC | both |
| `0x0554` | S→C | 8 | u8 u8 u32 | *EP6/PDB name* GameItemRepairOutgoing (TP_ITEM_REPAIR) | sends: CUser::ItemRepair, CUser::ItemUse | both |
| `0x0554` | C→S | 4 | +2:u8 +3:u8 | *EP6/PDB name* GameItemRepairIncoming (TPC_ITEM_REPAIR) | handled: CUser::PacketPC | both |
| `0x0555` | S→C | 4 | u16 | **the "Blessing of Goddess" modifier** used by the client to compute the lapis-link success rate (stored at `0x84d6d6`; rate = base − value/10). Sent by the client from… (the client's action notes) | sends: CUser::PacketPC | both |
| `0x0555` | C→S | 2 | — | `0x0555` `u16` Goddess-blessing rate modifier. confirmed (the client's action notes) | handled: CUser::PacketPC | both |
| `0x0556` | C→S | 2 | — | — confirmed (disasm) (the client's action notes) | handled: CUser::PacketPC | both |
| `0x0557` | C→S | 8 | +2:u8 +3:u8 +4:u32 | sender(s) `005894f0` | handled: CUser::PacketPC | both |
| `0x0558` | S→C | 9 | u32 u16 u8 | scouting result (`u32, u16, u8`) | sends: CUser::SkillOnce | both |

### 06xx — mobs

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0601` | S→C | 17 | u32 u8 u16 u32 u32 | creates the mob from `Monster.SData[wMOBID]` (model, **max HP** from the record), random facing; `byNew != 0` plays the spawn animation. **Current HP starts at 0**: send… (`client-world-packets.md`) | sends: CCell::SendCellInfoEnter, CUser::LevelUp, CZone::EnterMob … | both |
| `0x0602` | S→C | 6 | u32 | removes the mob (`client-world-packets.md`) | sends: CCell::SendCellInfoLeave, CZone::MobRemove, CZone::MobRemoveByID … | both |
| `0x0603` | S→C | 15 | u32 u8 u32 u32 | moves toward x/z; `byMotion` stored (walk 0 / run 1, **confirmed** in the client: `0x431aa0` picks the walk slot for 0 and the run slot for anything else; **ref** [Monsters: walking about](../original/mobs.md#6-walking-about): the server sends 0 for an idle step and 1 for chase and path moves ([Monsters fighting back](../original/mob-fight.md#4-chasing), [Monster paths](../original/pathing.md#3-chasing))) (`client-world-packets.md`) | sends: CGuildMgr::CreateGuild, CMiniGame::CardRun, CMiniGame::CardStop … | both |
| `0x0604` | S→C | 6 | u32 | removes the mob (no existence check) (`client-world-packets.md`) | never (no opcode store) | **client-only** |
| `0x0605` | S→C | 17 | u8 u32 u32 u16 u16 u16 | mob attack animation (one of its 3 attacks, `Monster.SData`), damage applied to the player when the blow lands; unknown mob: applied at once (`client-world-packets.md`) | sends: CMob::Attack, CUser::PacketAdminCmdB | both |
| `0x0606` | S→C | 11 | u32 u8 u32 | mob dies (death animation if its HP < 1); players attacking it stop (`client-world-packets.md`) | sends: CMob::Death | both |
| `0x0607` | S→C | 15 | u32 u16 u8 u16 u16 u16 | periodic skill effect on a mob: skill type 4 (heal) adds `value[0]` to HP, otherwise subtracts; numbers shown (`client-world-packets.md`) | sends: CMob::ChkKeepSkill, CMob::RemApplySkill, CUser::PacketUserDBSetInfo | both |
| `0x0608` | S→C | 14 | u32 u32 u32 | sets the mob's max and current HP (absolute). PDB `TP_MOB_GETHP` is 10 bytes: the EP4 client wants both values (`client-world-packets.md`) | never (no instruction in ps_game uses the value `0x608`) | **client-only** |
| `0x0609` | S→C | 8 | u32 u8 u8 | mob speeds (`client-world-packets.md`) | sends: CMob::SendSpeed | both |
| `0x060B` | S→C | 21 | u8 u32 u32 u8 u16 u8 u16 u16 u16 | mob skill on a player; `byAttackType` 0/1/2 picks which of the mob's attack animations to play; heal-type skills (type 2/4) heal the mob instead (`client-world-packets.md`) | sends: CMob::AttackUser, CMob::HealUser, CMob::SendUseSkill | both |
| `0x060C` | S→C | 27 | u8 u32 u32 u8 u16 u8 u16 u16 u16 u16 u16 u16 | drain: damage to the player, `wHeal` to the mob (`client-world-packets.md`) | sends: CMob::AttackUser, CUser::KillPCFrom, CUser::KillPCTo … | both |
| `0x060D` | S→C | 20 | u8 u32 u32 u16 u8 u16 u16 u16 | area (range) skill hit on one player (`client-world-packets.md`) | sends: CMob::AttackUser, CMob::HealUser | both |
| `0x060E` | S→C | 19 | u32 u32 u16 u8 u16 u16 u16 | additional/periodic damage from a mob skill: "Received another <v> damage by <p>'s <s>" (1304) (`client-world-packets.md`) | sends: CMob::AttackUser | both |
| `0x060F` | S→C | 16 | u32 u32 u16 u16 u16 | mirrored damage to a player; note the client subtracts **2 × wDamage[0]** from HP (confirmed in the disassembly), so follow with an HP update (`client-world-packets.md`) | never (no opcode store) | **client-only** |
| `0x0610` | S→C | 10 | u32 u32 | adds `nHP` (signed) to the mob's HP (`client-world-packets.md`) | sends: CMob::LuaRecover, CMob::PAI_AddEnergy, CMob::UpdateRecover … | both |

### 07xx — NPC shop, warehouse contents

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0701` | S→C | var | u8 loop:u8 loop:u8 | **replaces** the open shop's goods list (up to 256 entries) (`client-world-packets.md`) | sends: CUser::ExchangeItemAdd, CUser::ItemCreate, CUser::ItemGemRemoveAll … | both |
| `0x0702` | S→C | 12 | u8 u8 u8 u8 u8 u8 u32 | 0: item put in bag/slot (durability from `Item.SData`), **money set to `dwMoney`** (the new total), "Attained <i>" (52), `ui_pay001.wav`; 1: "Insufficient gold" (303); 2… (`client-world-packets.md`) | sends: CUser::ExchangeItemAdd, CUser::ExchangePvP, CUser::ItemDrop … | both |
| `0x0702` | C→S | 8 | +2:u32 +6:u8 +7:u8 | buy from an NPC shop (`u32 npcId?`, `u8 slot, u8 count`) with **no money check** | handled: `CUser::PacketShop` (compare, not a switch case) | both |
| `0x0703` | S→C | 12 | u8 u8 u8 u8 u8 u8 u32 | 0: the slot now holds `byCount` (0 empties it), money = `dwMoney`, "Sold <i>" (54); 1: "No item located on that spot" (239) (`client-world-packets.md`) | sends: CUser::ItemBagToBag, CUser::PacketShop | both |
| `0x0703` | C→S | 5 | +2:u8 +3:u8 +4:u8 | *EP6/PDB name* GameShopSellIncoming (TPC_SHOP_SELL) | handled: `CUser::PacketShop` | both |
| `0x0711` | S→C | var | u32 u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:… | warehouse contents and warehouse money; send several packets for more than 50 items (`client-world-packets.md`) | sends: CUser::ItemCloakAdd, CUser::ItemCloakRemove, CUser::SendCharacterBank | both |

### 08xx — lapis, enchanting, badges

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0801` | S→C | 18 | u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u32 | ≠0 "Succeeded in linking <i> with <r>" (55); 0 "Failed to link…" (56). Then: item (#5 bag, #6 slot) gem position #7 ← lapis #8; lapis stack (#2,#3) count ← #4 (0 = gone)… (the client's action notes) | sends: CUser::ItemGemAdd, CUser::ItemGet, CUser::ItemGetMoney … | both |
| `0x0801` | C→S | 8 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 | link a lapis (the client's action notes, §1) | handled: CUser::PacketGem | both |
| `0x0802` | S→C | var | if:u8 if:u8 if:u8 if:bytes[6] if:bytes[6] if:bytes[6] if:bytes[6] if:… | extract result: **33 bytes here** (`u8 success, bag, slot, u8[6] × 4, u32 money`, handler `0x558510`, clears all six gems); 34 with `gemPos` only with region feature 0 (`0x558700`, clears one gem). Messages 57/58 | sends: CUser::ItemGemRemoveAll, CUser::ItemGemRemovePos, CWorld::LeaveUser … | both, **wrong size** (34 sent, 33 read) |
| `0x0802` | C→S | 4, 8 | +2:u8 +3:u8 | extract lapis: always the **4-byte** form here; the 8-byte form (hammer + gem) needs region feature 0 | handled: CUser::PacketGem | both |
| `0x0803` | S→C | 15 | u8 u8 u8 bytes[6] u32 | *EP6/PDB name* GameCloakInfoAddOutgoing (TP_CLOAK_ADD) | sends: CUser::Exchange, CUser::ItemCloakAdd, CUser::PacketUserDBChar | both |
| `0x0803` | C→S | 10 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 +8:u8 +9:u8 | *EP6/PDB name* GameCloakInfoIncoming (TPC_CLOAK_ADD) | handled: CUser::PacketGem | both |
| `0x0804` | S→C | 4 | u8 u8 | *EP6/PDB name* GameCloakInfoRemoveOutgoing (TP_CLOAK_REM) | sends: CUser::Exchange, CUser::ItemCloakRemove, CUser::PacketMaininterface | both |
| `0x0804` | C→S | 4 | +2:u8 +3:u8 | remove a cloak badge | handled: CUser::PacketGem | both |
| `0x0805` | S→C | var | u8 u8 u8 u8 u8 u8 u8 u8 u8 u32 bytes[21]* | ≠0 "<i> [<v>]level Upgrade Completed." (67); 0 "…Upgrade failed." (68); level read from the craft name (> 50 = armour, minus 50) (the client's action notes) | sends: CUser::ExchangePvP, CUser::ItemLapisianAdd | both |
| `0x0805` | C→S | 9 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 +8:u8 | *EP6/PDB name* GameLapisianAddIncoming_EP5 (TPC_LAPISIAN_ADD) | handled: CUser::PacketGem | both |
| `0x0806` | S→C | var | u8 u8 u8 bytes[21]* | failure 5502 "This item is not for reconstruction." (the client's action notes) | sends: CUser::ItemCompose | both |
| `0x0806` | C→S | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | *EP6/PDB name* GameItemComposeIncoming_EP5 (TPC_ITEM_COMPOSE) | handled: CUser::PacketGem | both |
| `0x0807` | S→C | var | u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | 0 success; 2 "It includes the Reward Item from the Quest." (5511); 3 "It includes the Item shop's item." (5513); other: failure sound only (the client's action notes) | sends: CUser::ItemRemake | both |
| `0x0807` | C→S | 8 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 | *EP6/PDB name* GameItemRemakeIncoming_EP5 (TPC_ITEM_REMAKE) | handled: CUser::PacketGem | both |
| `0x0808` | S→C | var | u8 if:u8 if:u8 if:bytes[21]* | failure 5503 "You can't reward with this item." (the client's action notes) | sends: CUser::ItemConsider | both |
| `0x0808` | C→S | 4 | +2:u8 +3:u8 | *EP6/PDB name* GameItemRemake2Incoming | handled: CUser::PacketGem | both |

### 09xx — quests

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0901` | S→C | var | u8 var | adds active quests (login) (`client-world-packets.md`) | sends: CUser::SendCharacterQuest, CUser::SendSkillApply | both |
| `0x0902` | S→C | 8 | u32 u16 | quest accepted: `bgm_quest01.wav`, removes it from the finished list (repeatable quests), adds it to the active list; its `u16 value` starts as the quest's `NpcQuest.SDa`… (`client-world-packets.md`) | sends: CUser::AddApplySkill, CUser::ChkEndTimeSkill, CUser::ClearApplySkillByDeath … | both |
| `0x0902` | C→S | 8 | +2:u32 +6:u16 | *EP6/PDB name* GameQuestStartIncoming | handled: CUser::PacketQuest | both |
| `0x0903` | S→C | 22 | u32 u16 u8 u8 u32 u32 u8 u8 u8 u8 | `bySuccess` 0: "Failed the quest" (244). Otherwise "Completed the quest" (243), `bgm_quest02.wav`, adds `dwMoney` to money ("Gained <b>G"), puts the reward item (count 1… (`client-world-packets.md`) | sends: CUser::QuestEnd, CUser::QuestRemoveAdmin, CUser::QuestSuccessResult | both |
| `0x0903` | C→S | 8 | +2:u32 +6:u16 | *EP6/PDB name* GameQuestEndIncoming | handled: CUser::PacketQuest | both |
| `0x0905` | S→C | 6 | u16 u8 u8 | sets progress counter `byIndex` (0 or 1) of an active quest and prints it: faction kills "battles against the other faction : <v>/<v2>" (349/350) or monster kills "<p> <… (`client-world-packets.md`) | sends: CUser::QuestUpdateKillPC | both |
| `0x0906` | S→C | var | u8 if:var | adds finished quests (login) (`client-world-packets.md`) | sends: CUser::SendCharacterQuestFinish | both |
| `0x0907` | S→C | 4 | u16 | opens the reward choice for that quest; the player's pick comes back as client `0x0907` (`client-world-packets.md`) | sends: CUser::QuestEnd | both |
| `0x0907` | C→S | 5 | +2:u16 +4:u8 | *EP6/PDB name* GameQuestEndSelectIncoming (TPC_QUEST_END_SELECT) | handled: CUser::PacketQuest | both |
| `0x0908` | C→S | 4 | +2:u16 | *EP6/PDB name* GameQuestQuitIncoming | handled: CUser::PacketQuest | both |

### 0Axx–0Fxx — trade, party/union, member status, guild, NPCs, Priest

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x0A01` | S→C | 6 | u32 | trade request from `dwID`: opens the yes/no dialog "<p> requested a transaction" (210); busy → answers `0x0A02` = 1 at once (`client-social-packets.md`) | sends: CUser::PacketExchange, CUser::QuestAdd, CWorld::AddKillCount … | both |
| `0x0A01` | C→S | 6 | +2:u32 | player menu "request transaction" (5091) on a player; prints "Requested <p> a transaction" (209) (`client-social-packets.md`) | handled: CUser::PacketExchange | both |
| `0x0A02` | C→S | 3 | +2:u8 | trade request dialog; refuse prints "Rejected transaction request" (300); also sent (1) automatically when busy (`client-social-packets.md`) | handled: CUser::PacketExchange | both |
| `0x0A03` | S→C | 7 | u32 u8 | stores the partner id; 0: opens the trade window (mode 0x0C); 1: "The opponent refused transaction" (321); 2: "Transaction in process with another user" (322); both with… (`client-social-packets.md`) | sends: CMap::ObeliskDestroy, CUser::CancleActionExc, CUser::LeaveZone … | both |
| `0x0A04` | S→C | 3 | u8 | trade over, window closed, partner cleared. 0: "Transaction has been completed successfully" (211) and **money += partner's offer − my offer**; 1: silent; 2: "Transactio… (`client-social-packets.md`) | sends: CUser::CancleActionExc, CUser::Exchange, CUser::LeaveZone … | both |
| `0x0A05` | S→C | 4 | u8 u8 | ready state: `byWho` 1 = me, 2 = partner, 3 = both; `byOK` **0 = ready (locked), 1 = not ready** (`client-social-packets.md`) | sends: CUser::ExchangeCancleReady, CUser::PacketExchange, CUser::PacketUserDBChar … | both |
| `0x0A05` | C→S | 3 | +2:u8 | ready button (0 or 1 by the current state); 2 when the trade window closes while a partner is set (`client-social-packets.md`) | handled: CUser::PacketExchange | both |
| `0x0A06` | S→C | 6 | u8 u8 u8 u8 | my offer: copies the item at bag/slot into my trade slot `byExcSlot` (0–7) with count `byCount` (`client-social-packets.md`) | sends: CMob::ItemDrop, CUser::PacketExchange | both |
| `0x0A06` | C→S | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | item dropped on a free trade slot (count asked when the stack is > 1). Refused by the client itself: "Not a tradeable item" (443, `Item.SData` flag), quest item types 27… (`client-social-packets.md`) | handled: CUser::PacketExchange | both |
| `0x0A07` | S→C | 4 | u8 u8 | empties a trade slot: `byWho` 1 = mine, 2 = the partner's (`client-social-packets.md`) | sends: CUser::ExchangeItemRemove, CUser::PacketExchange | both |
| `0x0A07` | C→S | 3 | +2:u8 | right-click on one of my offered items (`client-social-packets.md`) | handled: CUser::PacketExchange | both |
| `0x0A08` | S→C | 7 | u8 u32 | money offer: 1 = mine, 2 = the partner's (`client-social-packets.md`) | sends: CMarketMgr::CheckMarketItemEndDate, CMarketMgr::DirectBuy, CUser::PacketExchange | both |
| `0x0A08` | C→S | 6 | +2:u32 | money entered in the trade window (`client-social-packets.md`) | handled: CUser::PacketExchange | both |
| `0x0A09` | S→C | var | u8 u8 u8 u8 u16 bytes[6] bytes[21]* | the partner put an item in their slot `byExcSlot` (`client-social-packets.md`) | sends: CMarketMgr::CheckMarketItemEndDate, CMarketMgr::RetrievalItem, CUser::PacketExchange | both |
| `0x0B01` | S→C | var | u8 u8 loop:u32 loop:bytes[21] loop:u16 loop:u8 loop:u16 loop:u16 loop… | builds the list: the `byCount` members, then the local player appended; leader = the entry at index `byBoth` in that order (index `byCount` = the local player). Sets "in… (`client-social-packets.md`) | sends: CGuildMgr::CreateGuild, CParty::EnterNormalParty | both |
| `0x0B02` | S→C | 6 | u32 | invitation dialog (mode 4) "<p> invited" (204), `ui_party001.wav`; busy → answers `0x0B03 (1, my id)` (`client-social-packets.md`) | sends: CGuild::Fire, CUser::PacketParty | both |
| `0x0B02` | C→S | 6 | +2:u32 | player menu "party invite"; prints "Invited <p>" (203) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B03` | S→C | 7 | u8 u32 | invitation failed, with `ui_error001.wav`: 0 "<p> rejected the party's invitation" (206), 1 "<p> cannot be invited to the party" (304) — **0 and 1 show nothing unless `d`… (`client-social-packets.md`) | sends: CGuild::JoinRequest, CUser::LeaveZone, CUser::SendPartyError | both |
| `0x0B03` | C→S | 7 | +2:u8 +3:u32 | invitation dialog; refuse prints "Rejected the party invitation" (208); also (1, own id) automatically when busy (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B04` | S→C | var | u32 bytes[21]* u16 u8 u16 u16 u16 u16 u16 u16 u16 u32 u32 u32 u8 loop… | adds a member (ignored if already listed): "<p> joined the party" (205), `ui_party002.wav` (`client-social-packets.md`) | sends: CGuild::CreateAddUser, CGuild::JoinOK, CGuild::JoinUser … | both |
| `0x0B05` | S→C | 6 | u32 | my id: party cleared, "Left the party for good" (305). Other: removed, "<p> left the party for good" (306) (`client-social-packets.md`) | sends: CGuild::JoinLeave, CGuildMgr::FireGuild, CGuildMgr::RemoveGuildZeroUser … | both |
| `0x0B05` | C→S | 2 | — | player menu "leave party", or `/withdraw` (5081); prints "Left the party." (358) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B06` | S→C | 6 | u32 | my id: party cleared, "A party leader kicked you from the party" (309), `ui_error001.wav`. Other: removed; "<p> has been kicked from the party" (310) if in view (`client-social-packets.md`) | sends: CGuild::JoinExpel, CParty::ExpelUser | both |
| `0x0B06` | C→S | 6 | +2:u32 | player menu "kick" (prints 301) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B07` | S→C | 6 | u32 | new leader: mine "You are granted as a party leader" (307), other "<p> is granted as a party leader" (308). `dwID = 0` clears the leader silently (the original server se… (`client-social-packets.md`) | sends: CGuild::CreateAddUser, CGuild::DeleteChar, CGuild::JoinRequest … | both |
| `0x0B07` | C→S | 6 | +2:u32 | player menu "grant leadership" (prints 302) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B08` | S→C | 8 | u32 u8 u8 | "<p> attained <i>" (61) for a party member's loot (item name coloured) (`client-social-packets.md`) | sends: CGuild::JoinExpel, CGuild::JoinLeave, CGuild::JoinNO … | both |
| `0x0B09` | S→C | var | bytes[21]* | invitation by name (from `/party <name>`): same dialog as `0x0B02`, with this name (`client-social-packets.md`) | sends: CGuild::DeleteCharChangeMaster, CGuild::JoinOK, CGuild::NpcUpgrade … | both |
| `0x0B09` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `/party <name>` (5079) when not in a union; prints "Invited <p>" (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B0B` | S→C | var | u8 u8 u8 u16 u8 u8 u8 loop:u16 loop:u32 loop:bytes[21]* loop:u16 loop… | adds the listed members by slot (does not clear first; `wStartEndFlag` and `bRaidPartyType` unused); leader/sub-leader by slot; my group from my slot (`client-social-packets.md`) | sends: CGuild::GuildPvPLoss, CGuild::GuildPvPWin, CParty::EnterRaidParty … | both |
| `0x0B0C` | S→C | var | u16 u32 bytes[21]* u16 u8 u16 u16 u16 u16 u16 u16 u16 u32 u32 u32 u8… | adds a member at `slot`; my own id: union window on, "Joined the party" (207); other: "<p> joined the party" (205); `ui_party002.wav` (`client-social-packets.md`) | sends: CParty::EnterRaidParty | both |
| `0x0B0D` | S→C | 6 | u32 | my id: "Left the party for good" (305) and the union is cleared (as `0x0B16`); other: "<p> left the party for good" (306), removed (`client-social-packets.md`) | sends: CParty::LeaveRaidParty | both |
| `0x0B0D` | C→S | 2 | — | union: leave | handled: CUser::PacketParty | both |
| `0x0B0E` | S→C | 16 | u8 u8 u32 u32 u32 | the current party becomes a union: members keep their party order as slots; leader/sub-leader = the members at those slots; "Union Party created." (6532) (`client-social-packets.md`) | sends: CUser::PacketParty | both |
| `0x0B0E` | C→S | 8 | +2:u8 +3:u8 +4:u32 | union: `u8, u8, u32` (*inferred* move a member between groups) | handled: CUser::PacketParty | both |
| `0x0B0F` | S→C | 6 | u32 | loot rule: 1 "Item Loot is changed order distribution." (6516), 2 random (6517), 4 leader (6518) (`client-social-packets.md`) | sends: CGuild::NpcUpgrade, CUser::PacketParty | both |
| `0x0B0F` | C→S | 6 | +2:u32 | union settings (loot rule) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B10` | S→C | 3 | u8 | auto-join on (6520) / off (6521) (`client-social-packets.md`) | sends: CUser::PacketGuild, CUser::PacketParty | both |
| `0x0B10` | C→S | 3 | +2:u8 | union settings (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B11` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `/join <name>` (6543), only when in no party ("You are in party already." 6537 otherwise): join that player's union if it has auto-join (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B12` | S→C | 18 | u32 u32 u32 u32 | swaps the members at the two slots (the last two fields are read but unused); updates my group (`client-social-packets.md`) | sends: CUser::PacketGuild, CUser::PacketParty | both |
| `0x0B12` | C→S | 10 | +2:u32 +6:u32 | "Move Group" (6506), then drop on a slot (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B13` | S→C | 6 | u32 | new sub-leader: "You become a sub-party leader ." (6514) / "<p><is> become a sub-party leader." (6515) (`client-social-packets.md`) | sends: CGuildMgr::RemoveGuildZeroUser, CParty::ExpelRaidPartyUser, CParty::LeaveRaidParty … | both |
| `0x0B13` | C→S | 6 | +2:u32 | "Delegate sub-leader" (6508) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B14` | S→C | 6 | u32 | union invitation dialog (mode 0x19, texts 10007/204); busy → `0x0B15 (1, my id)` (`client-social-packets.md`) | sends: CUser::PacketParty | both |
| `0x0B14` | C→S | 6 | +2:u32 | "Union invitation" (6513); prints "Invited <p>" (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B15` | S→C | 7 | u8 u32 | same as `0x0B03` (same reader and logic) (`client-social-packets.md`) | never (no opcode store) | **client-only** |
| `0x0B15` | C→S | 7 | +2:u8 +3:u32 | union invitation dialog (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B16` | S→C | var | — | union dismantled: "Union Party dismantled." (6531), all union state reset, party marks of members in view cleared (`client-social-packets.md`) | sends: CGuildMgr::UpdateGuildPointRank, CUser::PacketParty | both |
| `0x0B16` | C→S | 2 | — | "Dismantle Union" (6502) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B17` | S→C | 6 | u32 | new union leader: 307 / 308 (`client-social-packets.md`) | sends: CParty::ExpelRaidPartyUser, CParty::LeaveRaidParty, CParty::SetRaidPartyBoth | both |
| `0x0B17` | C→S | 6 | +2:u32 | "Delegate leader" (6507) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B18` | S→C | var | bytes[21]* | union invitation by name (dialog as `0x0B14`) (`client-social-packets.md`) | sends: CUser::PacketParty | both |
| `0x0B18` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `/party <name>` while in a union (leader or sub-leader, union not full: else 6539 "Can't invite member because party is full." / 6540 "You don't have invitation authorit… (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0B19` | S→C | 6 | u32 | 3 "Already in another party." (346), 4 "The character doesn't exist." (347), 5 "Already in a party." (348), 10 "Can't Config Union party." (6522), 11 "Can't change Group… (`client-social-packets.md`) | sends: CUser::SendRaidPartyError | both |
| `0x0B1A` | S→C | 6 | u32 | my id: "Party leader kicked you." (6533), union cleared; other: "<p><is> kicked from party." (6534) (`client-social-packets.md`) | sends: CParty::ExpelRaidPartyUser | both |
| `0x0B1A` | C→S | 6 | +2:u32 | union "Kick" (6509) (`client-social-packets.md`) | handled: CUser::PacketParty | both |
| `0x0C01` | S→C | 9 | u32 u8 u16 | sets current HP/SP/MP: **type 0 HP, 1 SP, 2 MP** (`client-social-packets.md`) | sends: CUser::SendEnergyHPToParty, CUser::SendUseSPMP | both (covered by another packet) |
| `0x0C02` | S→C | 9 | u32 u8 u16 | sets one maximum (same types) (`client-social-packets.md`) | sends: CUser::PacketComm, CUser::SendMaxHP, CUser::SendMaxMP … | both (covered by another packet) |
| `0x0C03` | S→C | 12 | u32 u16 u16 u16 | sets the three current values (`client-social-packets.md`) | sends: CUser::PacketComm, CUser::SendEnergyAllToParty | both |
| `0x0C04` | S→C | 9 | u32 u16 u8 | adds/updates a buff on the member (time = `Skill.SData` +0x36 × 1000; level 100 counts as 1) (`client-social-packets.md`) | sends: CUser::PacketComm, CUser::SendSkillApply | both |
| `0x0C05` | S→C | 9 | u32 u16 u8 | removes the buff with that skill and level (`client-social-packets.md`) | sends: CUser::SendSkillRemoveParty | both |
| `0x0C06` | S→C | 6 | u32 | clears the member's buff list (`client-social-packets.md`) | sends: CUser::Death, CUser::DeathByNPC, CUser::PacketComm | both |
| `0x0C07` | S→C | 20 | u32 u16 u32 u32 u32 | member position (party window/minimap) (`client-social-packets.md`) | sends: CParty::SendMove, CUser::PacketComm | both |
| `0x0C08` | S→C | 12 | u32 u16 u16 u16 | sets the three maxima (`client-social-packets.md`) | sends: CUser::PacketComm, CUser::SendEnergyAllMaxToParty | both |
| `0x0C09` | S→C | 8 | u32 u16 | member level (`client-social-packets.md`) | sends: CUser::LevelUp | both |
| `0x0C0A` | S→C | 9 | u32 u8 u16 | the same updates on union members (`FUN_004c88b0` lookup). **There is no `0x0C0F` handler.** (`client-social-packets.md`) | sends: CUser::SendEnergyHPToParty, CUser::SendUseSPMP | both |
| `0x0C0B` | S→C | 9 | u32 u8 u16 | *inferred*: union member: one value max (as `0x0C02`) | sends: CUser::SendMaxHP, CUser::SendMaxMP, CUser::SendMaxSP | both |
| `0x0C0C` | S→C | 12 | u32 u16 u16 u16 | *inferred*: union member: HP/SP/MP (as `0x0C03`) | sends: CUser::SendEnergyAllToParty | both |
| `0x0C0D` | S→C | 12 | u32 u16 u16 u16 | *inferred*: union member: all maxima (as `0x0C08`) | sends: CUser::SendEnergyAllMaxToParty | both |
| `0x0C0E` | S→C | 9 | u32 u16 u8 | *inferred*: union member: buff added (as `0x0C04`) | sends: CUser::SendSkillApply | both |
| `0x0C10` | S→C | 9 | u32 u16 u8 | *inferred*: union member: buff removed (as `0x0C05`) | sends: CUser::SendSkillRemoveParty | both |
| `0x0C11` | S→C | 6 | u32 | *inferred*: union member died (as `0x0C06`) | sends: CUser::Death | both |
| `0x0C12` | S→C | 20 | u32 u16 u32 u32 u32 | *inferred*: union member position (as `0x0C07`) | sends: CParty::SendMove | both |
| `0x0C13` | S→C | 8 | u32 u16 | the same updates on union members (`FUN_004c88b0` lookup). **There is no `0x0C0F` handler.** (`client-social-packets.md`) | sends: CUser::LevelUp | both |
| `0x0C14` | S→C | 8 | u32 u8 u8 | loot message for a union member (`client-social-packets.md`) | sends: CUser::ItemGet | both |
| `0x0D01` | S→C | var | u8 u32 u8 bytes[25]* | older creation result: as `0x0D21` without remark, code 9 and guild ids. **server**: the original server sends `0x0D01` for creation failures (`CGuildCreate::SendGuildCr`… (`client-social-packets.md`) | sends: CGuildCreate::SendGuildCreateFailed, CMiniGame::GameEnter | both |
| `0x0D01` | C→S | 27 | +2:bytes[25] | older creation path in the same window (confirm case 4); **the original server ignores `0x0D01`** (`client-social-packets.md`) | handled: CUser::PacketUserDBSavePoint (compare) | both |
| `0x0D02` | S→C | var | u32 bytes[25]* | dialog (mode 1): "<guild name> cannot join in another guild for 3 days once you join in a guild. Do you still want to join in?" (8034); answer `0x0D22` (`client-social-packets.md`) | sends: CMiniGame::GameExit, CUser::PacketMaininterface | both |
| `0x0D03` | S→C | 3 | u8 | disband result: 0 "Failed to disband guild." (380), 1 "Disbanded guild." (381; my guild cleared), 2 "insufficient party members" (382), 3 "party member's rejection" (383) (`client-social-packets.md`) | sends: CGuildCreate::SendGuildFireFailed, CParty::ExpelRaidPartyUser, CParty::ExpelUser … | both |
| `0x0D03` | C→S | 2 | — | guild window "disband" (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D04` | S→C | 6 | u32 | dialog (mode 2): "Guild master has requested guild dismiss. Do you agree?" (10106); answer `0x0D04` (`client-social-packets.md`) | sends: CGuildCreate::SendGuildFireAgree | both |
| `0x0D04` | C→S | 3 | +2:u8 | disband agreement dialog (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D05` | S→C | 6 | u32 | dialog (mode 3): 8034 text; answer `0x0D06` (`client-social-packets.md`) | never (no opcode store) | **client-only** |
| `0x0D05` | C→S | 6 | +2:u32 | `/sign up for guild` (5080) on the targeted player: invite (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D06` | S→C | 7 | u8 u32 | to the inviter: 0 "Guild rejected join-request" (385), else "Guild accepted join-request" (384) (`client-social-packets.md`) | sends: CUser::LeaveZone, CUser::PacketGuild | both |
| `0x0D06` | C→S | 3 | +2:u8 | guild invitation dialog (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D07` | S→C | 3 | u8 | join request (from the guild list): 0 "Rejected join-request." (387), else "Requested to join." (386). The original server sends 2 for "still under the join penalty", wh… (`client-social-packets.md`) | sends: CGuild::JoinRequest, CUser::PacketGuild | both |
| `0x0D07` | C→S | 6 | +2:u32 | guild list "join request" (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D08` | S→C | var | u8 u32 u8 bytes[25]* | joined: "<b> guild accpeted join-request" (sic, 388), my guild name and id set (the rank is not stored here); 0: "<b> guild rejected join-request" (389) (`client-social-packets.md`) | sends: CGuild::JoinNO, CGuild::JoinOK, CGuild::JoinUser | both |
| `0x0D08` | C→S | 7 | +2:u8 +3:u32 | join-request list: accept (1) / refuse (0) (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D09` | S→C | 3 | u8 | 1 "Quit guild." (390), my guild cleared; 0 "Impossible to quit guild." (391) (`client-social-packets.md`) | sends: CGuild::JoinLeave | both |
| `0x0D09` | C→S | 2 | — | guild window "leave" (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D0A` | S→C | 7 | u8 u32 | **needs `dwUserID` in my member list, else nothing.** 0 "Impossible to kick <b> from guild." (394); my id: "Kicked from guild." (392), guild cleared; other: "Kicked <b>… (`client-social-packets.md`) | sends: CGuild::JoinExpel | both |
| `0x0D0A` | C→S | 6 | +2:u32 | member list "kick" (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D0B` | S→C | var | u32 u8 u8 bytes[21]* | **ignored** (empty function) | never (no opcode store) | ignored by the client |
| `0x0D0C` | S→C | 7 | u8 u32 | member event, by `byState`: < 100 sets the member's level; 101 (0x65) "Guild has been disbanded." (395), guild cleared; 102 "<b> quit guild." (397) and 103 "<b> has been… (`client-social-packets.md`) | sends: CGuild::ChangeGuildUserLevel, CGuild::ChangeUserLevel, CGuild::ConnectUser … | both |
| `0x0D0C` | C→S | 7 | +2:u8 +3:u32 | member list rank buttons: 0 = promote, 1 = demote (**confirmed (disasm)** plus the button picture `guild_button1.tga`; sender `0x588bc0`; [Social packets](client-social-packets.md)) | handled: CUser::PacketGuild | both |
| `0x0D0D` | S→C | var | — | clears the guild list before a new one (`client-social-packets.md`) | sends: CGuildMgr::SendGuildList | both |
| `0x0D0E` | S→C | var | — | **ignored** (end of list) (`client-social-packets.md`) | sends: CGuildMgr::SendGuildList | both |
| `0x0D0F` | S→C | var | u8 var | old guild list / add (no rank, remark or points) (`client-social-packets.md`) | never (no opcode store) | **client-only** |
| `0x0D10` | S→C | 52 | bytes[50] | old guild list / add (no rank, remark or points) (`client-social-packets.md`) | never (no opcode store) | **client-only** |
| `0x0D11` | S→C | 6 | u32 | removes a guild from the list (`client-social-packets.md`) | sends: CGuildMgr::FireGuild | both |
| `0x0D12` | S→C | var | u8 var | adds members with status 1 / 2 / 3: the guild window prints 1 and 3 as "On" and 2 as "Off", so 1 and 3 are online and 2 is offline (**confirmed**); why 3 has its own colour (a war zone?) stays *inferred*; my entry sets my rank ([Social packets](client-social-packets.md)) | sends: CGuild::SendGuildUserList | both |
| `0x0D13` | S→C | var | u8 var | adds members with status 1 / 2 / 3: the guild window prints 1 and 3 as "On" and 2 as "Off", so 1 and 3 are online and 2 is offline (**confirmed**); why 3 has its own colour (a war zone?) stays *inferred*; my entry sets my rank ([Social packets](client-social-packets.md)) | sends: CGuild::SendGuildUserList | both |
| `0x0D14` | S→C | 32 | u8 bytes[29] | adds one member (status `byState`); shows my guild name over that player (`client-social-packets.md`) | sends: CGuild::JoinOK, CGuild::JoinUser | both |
| `0x0D15` | S→C | 6 | u32 | removes a member, clears the name over that player (`client-social-packets.md`) | never (no instruction uses `0xD15`) | **client-only** |
| `0x0D16` | S→C | var | u8 var | join requests (for the master/officers) (`client-social-packets.md`) | sends: CGuild::SendGuildJoinList | both |
| `0x0D17` | S→C | 30 | bytes[28] | adds one join request (`client-social-packets.md`) | sends: CGuild::JoinRequest | both |
| `0x0D18` | S→C | 6 | u32 | removes it (`client-social-packets.md`) | sends: CGuild::DeleteChar, CGuild::JoinNO, CGuild::JoinOK … | both |
| `0x0D19` | S→C | var | u8 var | adds members with status 1 / 2 / 3: the guild window prints 1 and 3 as "On" and 2 as "Off", so 1 and 3 are online and 2 is offline (**confirmed**); why 3 has its own colour (a war zone?) stays *inferred*; my entry sets my rank ([Social packets](client-social-packets.md)) | sends: CGuild::SendGuildUserList | both |
| `0x0D20` | S→C | var | u8 bytes[65]* | sets my guild's remark in the guild list (`byResult` unused) (`client-social-packets.md`) | sends: CGuild::UpdateRemark, CUser::PacketGuild | both |
| `0x0D20` | C→S | 67 | +2:bytes[65] | remark edit (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D21` | S→C | var | u8 u32 u8 bytes[25]* bytes[65]* | creation result (current version). 1: "Formed <b>guild." (372), my guild name set, and guild name + id set on every party member in view. Failure codes: 0 "Failed to for… (`client-social-packets.md`) | sends: CGuildMgr::CreateGuild, CUser::PacketGuild | both |
| `0x0D21` | C→S | 92 | +2:bytes[24] +27:bytes[65] | guild-creation window (name, then remark) (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D22` | S→C | var | u32 bytes[25]* | dialog (mode 1): "<guild name> cannot join in another guild for 3 days once you join in a guild. Do you still want to join in?" (8034); answer `0x0D22` (`client-social-packets.md`) | sends: CGuildCreate::SendGuildCreateAgree | both |
| `0x0D22` | C→S | 3 | +2:u8 | creation agreement dialog (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D23` | S→C | 6 | u32 | my guild's Etin (`client-social-packets.md`) | sends: CGuild::NpcUpgrade, CGuild::SendEtin, CGuildMgr::UpdateGuildPointRank … | both |
| `0x0D23` | C→S | 2 | — | guild house windows (Etin count) (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D24` | S→C | var | u8 var | guild house NPC levels (`client-social-packets.md`) | sends: CGuild::SendNpcLvList | both |
| `0x0D25` | S→C | 7 | u8 u16 u8 u8 | 0: NPC level updated; 1 "insufficient Etin" (8003), 2 "upgrade is done one by one level." (8004), 3 "failed to upgrade." (8005), 4 "low Rank" (8006) (`client-social-packets.md`) | sends: CGuild::NpcUpgrade | both |
| `0x0D25` | C→S | 5 | +2:u16 +4:u8 | NPC upgrade (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D26` | S→C | 5 | u16 u8 | ranking battle notices with `ui_news001.wav`: type < 24 = "From <b>hour, guild ranking battle will get started later…" (8000), 100 started (8001), 101 10 minutes left (8… (`client-social-packets.md`) | sends: CWorld::SendGuildRankNotics, CZone::SendAllGuildRankNotices | both |
| `0x0D27` | S→C | 14 | u32 u32 u32 | stored (**unknown**; the PDB's `TP_GUILD_RANKING_GETPOINT` is 8 bytes, but the original server's `CGuild::SendRankingZone` sends `0x0D27` with 14) (`client-social-packets.md`) | sends: CGuild::SendRankingZone | both |
| `0x0D28` | S→C | 4 | u8 u8 | 1 "Your Rank isn't high enough." (8026), 2 "level is not high enough…" (8027), 3 "guild warehouse is full." (8028), 4 "Not authorized" (2038) (`client-social-packets.md`) | sends: CGuild::ItemBagToGuildBank, CGuild::ItemGuildBankToBag, CGuild::ItemGuildBankToGuildBank … | both |
| `0x0D29` | S→C | var | u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 l… | guild bank contents (`client-social-packets.md`) | sends: CGuild::SendBankItemList | both |
| `0x0D2F` | S→C | var | u8 var | adds guilds to the list (`client-social-packets.md`) | sends: CGuildMgr::SendGuildList | both |
| `0x0D30` | S→C | 122 | bytes[120] | adds one guild (`client-social-packets.md`) | sends: CGuildMgr::CreateGuild | both |
| `0x0D31` | S→C | var | u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* u32 | sets a bank slot (`client-social-packets.md`) | sends: CGuild::ItemBagToGuildBank, CGuild::ItemGuildBankToBag | both |
| `0x0D32` | S→C | var | u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* u32 | empties the slot (`client-social-packets.md`) | sends: CGuild::ItemBagToGuildBank, CGuild::ItemGuildBankToBag | both |
| `0x0D34` | S→C | 7 | u8 u32 | 0: money = `dwMoney`, "purchased guild house." (8035); 1 already (8029), 2 "Insufficient gold" (303), 3 not authorized (2038), 4 ranking requirement (8030) (`client-social-packets.md`) | sends: CUser::PacketGuild | both |
| `0x0D34` | C→S | 6 | +2:u32 | guild house purchase at the NPC (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D35` | S→C | var | u8 var | empties those bag slots and sums their Etin (typeID 1–4 = 1, 10, 100, 1000 each): "You return total <v>Etin." (8052) (`client-social-packets.md`) | sends: CUser::PacketGuild | both |
| `0x0D35` | C→S | 6 | +2:u32 | return Etin at the NPC (`client-social-packets.md`) | handled: CUser::PacketGuild | both |
| `0x0D36` | S→C | 3 | u8 | "In the guild battle,<b>guild is the winner…" (8050) with my guild name (`client-social-packets.md`) | sends: CGuild::GuildPvPWin | both |
| `0x0D37` | S→C | var | u8 var | new ranking: every guild set to rank 31, then the listed ones; "Each guild's gained point has been settled…" (8054) (`client-social-packets.md`) | sends: CGuildMgr::UpdateGuildPointRank | both |
| `0x0D38` | S→C | 6 | u32 | guild house reset: "Guild House server has been reset." (8057), sound and effect (`client-social-packets.md`) | never (no opcode store) | **client-only** |
| `0x0E01` | S→C | 23 | u32 u8 u16 u32 u32 u32 u16 | creates the NPC (type/typeId index `NpcQuest.SData`), facing `wDir` (`client-world-packets.md`) | sends: CCell::SendCellInfoEnter, CUser::PvPChkLoseDeath, CZone::EnterNpc … | both |
| `0x0E02` | S→C | 6 | u32 | removes the NPC (`client-world-packets.md`) | sends: CCell::SendCellInfoLeave, CUser::PacketPoint, CUser::PvPChkLoseDeath … | both |
| `0x0E03` | S→C | 19 | u32 u8 u32 u32 u32 | walks the NPC to x/y/z (turns to face it); `byMotion != 0` is kept as a flag that multiplies the step by 4, so it means run (**confirmed**, `0x433d90`; [World packets](client-world-packets.md)) | sends: CUser::PacketPVP, CUser::PacketPoint, CUser::PvPChkLeaveByAttack … | both |
| `0x0E04` | S→C | 6 | u32 | **ignored** (empty function) (`client-world-packets.md`) | never (no opcode store) | ignored by the client |
| `0x0E04` | C→S | 6 | +2:u32 | NPC clicked (`u32 npcId`) | handled: CUser::PacketNpc | both |
| `0x0E05` | S→C | 13 | u8 u32 u32 u16 | guard attacks a player (corrected: the player list, not a mob): attack animation, the HP lowered when the blow lands, at once for an unknown NPC (`client-world-packets.md`) | sends: CNpc::Attack, CUser::PacketPoint | both |
| `0x0E06` | S→C | 13 | u8 u32 u32 u16 | guard attacks a monster (corrected: the monster list, not a player), same pattern (`client-world-packets.md`) | sends: CNpc::Attack, CUser::PacketPoint | both |
| `0x0F01` | S→C | var | u8 if:u16 if:u16 if:u8 if:u8 if:u8 | Priest NPC reading: `u8 ok`, then four ratings; ok → dialog 0x69, else 303 "Insufficient gold" (the client's action notes, §4) | sends: `CUser::PacketPriest` | both |
| `0x0F01` | C→S | 2 | — | Priest NPC option 2 (paid reading) | handled: CUser::PacketPriest | both |
| `0x0F02` | S→C | 3 | u8 | Priest reading result `u8`: 1 shows it, else 303 | sends: `CUser::PacketPriest` | both |
| `0x0F02` | C→S | 2 | — | Priest result confirm | handled: CUser::PacketPriest | both |

### 11xx — chat

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x1101` | S→C | var | u32 u8 var* | *EP6/PDB name* GameChatNormalOutgoing (TP_CHAT_NORMAL) | sends: CUser::PacketChatE | both |
| `0x1102` | S→C | var | u8 bytes[21]* u8 var* | *EP6/PDB name* GameChatSendToOutgoing (TP_CHAT_SENDTO) | sends: CUser::PacketChatE | both |
| `0x1103` | S→C | var | bytes[21]* u8 var* | *EP6/PDB name* GameChatTradeOutgoing (TP_CHAT_SENDALL) | sends: CUser::PacketChatE | both |
| `0x1104` | S→C | var | bytes[21]* u8 var* | *EP6/PDB name* GameChatGuildOutgoing | sends: CGuild::SendAllGuildChat | both |
| `0x1105` | S→C | var | u32 u8 var* | *EP6/PDB name* GameChatPartyOutgoing (TP_CHAT_SENDPARTY) | sends: CUser::PacketChatE | both |
| `0x1106` | S→C | 3 | u8 | *EP6/PDB name* GameChatErrorOutgoing (TP_CHATE_ERROR) | sends: CUser::PacketAdminChat, CUser::PacketChatE | both |
| `0x1107` | S→C | var | u32 u8 var* | shout chat | sends: CUser::PacketChatE | both |
| `0x1108` | S→C | var | bytes[21]* u8 var* | *EP6/PDB name* GameChatAllOutgoing (TP_CHAT_SENDALL) | sends: CUser::PacketAdminChat, CUser::PacketChatE | both |
| `0x1109` | S→C | var | u8 u32 u8 var* | NPC/mob speech bubble: `u8, u32 id, u8 len, text` | sends: CMob::LuaSay, CMob::PAI_Say, CMob::PAI_Say2 | both |
| `0x110A` | S→C | 8 | u32 u16 | mob speech by text index (`u32, u16`) | sends: CMob::LuaSayByIndex | both |
| `0x110B` | S→C | var | u32 bytes[32]* | mob voice line (`u32, char[32]` sound name) | sends: CMob::LuaSayByVoice | both |
| `0x1111` | S→C | var | bytes[21]* u8 var* | *EP6/PDB name* GameChatZoneOutgoing (TP_CHAT_ZONE) | sends: CUser::PacketChatE | both |
| `0x1111` | C→S | var | — | *EP6/PDB name* GameChatZoneIncoming | handled: CUser::PacketChatE | both |
| `0x1112` | S→C | var | — | *EP6/PDB name* GameChatUnionOutgoing | sends: CUser::PacketChatE | ignored by the client |
| `0x1112` | C→S | var | — | *EP6/PDB name* GameChatUnionIncoming | handled: CUser::PacketChatE | both |

### 12xx — GM petition

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x1201` | C→S | var | — | `/request GM` + text (GM petition) | handled: CUser::PacketUQ | both |

### 1Cxx — auction house

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x1C01` | S→C | var | if:u8 if:u8 if:u8 loop:bytes[48] | search results (opens the AH window, dialog 0x75, if closed); the two flags enable the page buttons (the client's action notes) | sends: `CMarketMgr::SearchMarketFirst` (computed `0x1C01`/`0x1C02`) | both |
| `0x1C01` | C→S | 39 | +2:bytes[4] +6:bytes[4] +10:u32 +14:u32 +18:u32 +22:u32 +26:u32 +30:u… | Search (`0x459f60` case 6). classFilter: 0 = both or neither faction checkbox, 1 = only the first, 2 = only the second. grade: the window's choice, 6 → 7 and 7 → 0xFF (a… (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C02` | S→C | var | if:u8 if:u8 if:u8 loop:bytes[48] | search results (opens the AH window, dialog 0x75, if closed); the two flags enable the page buttons (the client's action notes) | sends: `CMarketMgr::SearchMarketFirst` (computed) | both |
| `0x1C02` | C→S | 4 | +2:bytes[2] | "search this item" on a selected result row (row bytes 1–2 are the item's Type/TypeID) (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C03` | S→C | var | if:u8 if:u8 if:u8 loop:bytes[48] loop:bytes[48] | a page; inserted in reverse order after a "previous" request (the client's action notes) | sends: CMarketMgr::SearchMarketNext, CMarketMgr::SearchMarketPrev, CMarketMgr::SearchMarketRefresh | both |
| `0x1C03` | C→S | 3 | +2:u8 | page buttons (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C04` | S→C | var | u8 loop:bytes[52] | the four lists (registrations, bids, items to collect, gold to collect) (the client's action notes) | sends: CMarketMgr::GetSellList | both |
| `0x1C04` | C→S | 2 | — | my registrations / my bids list (tab "progress"); each **once per session** (flags `0x84cdbc`/`0x84cdbd`), later kept up to date by the results below (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C05` | S→C | var | u8 loop:bytes[48] | the four lists (registrations, bids, items to collect, gold to collect) (the client's action notes) | sends: CMarketMgr::GetTenderList | both |
| `0x1C05` | C→S | 2 | — | my registrations / my bids list (tab "progress"); each **once per session** (flags `0x84cdbc`/`0x84cdbd`), later kept up to date by the results below (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C06` | S→C | var | u8 loop:bytes[41] | the four lists (registrations, bids, items to collect, gold to collect) (the client's action notes) | sends: CMarketMgr::GetEndItemList, CMarketMgr::RetrievalItem | both |
| `0x1C06` | C→S | 2 | — | items to collect / gold to collect (tab "completion"); once per session (`0x84cdbe`/`0x84cdbf`) (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C07` | S→C | var | u8 loop:bytes[53] | the four lists (registrations, bids, items to collect, gold to collect) (the client's action notes) | sends: CMarketMgr::GetEndMoneyList | both |
| `0x1C07` | C→S | 2 | — | items to collect / gold to collect (tab "completion"); once per session (`0x84cdbe`/`0x84cdbf`) (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C08` | S→C | var | u8 if:u8 if:u8 if:u8 if:u32 if:bytes[52] | 0 "Registration for your auction is complete." (7116): **gold = the u32** (after commission), the stack loses `count` (all → slot cleared), row added to my registrations… (the client's action notes) | sends: CUser::PacketMarket | both |
| `0x1C08` | C→S | 14 | +2:bytes[4] +6:u32 +10:u32 | register an item. period index 0–3 = **7 h, 24 h, 72 h, 168 h** (table `0x6a98e8`; default 2). Client checks first: an item chosen (7100), minBid > 0 (7112) and ≤ 3,800,… (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C09` | S→C | var | u8 if:bytes[41] | 0 7119 "The Auction for this item is canceled." (row moves to items to collect); else 7120 (the client's action notes) | sends: CMarketMgr::UnRegisterItem, CUser::PacketMarket | both |
| `0x1C09` | C→S | 6 | — | cancel my auction, after 7118 (commission is lost) (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C0A` | S→C | var | u8 if:u32 if:bytes[41] | 0 7111 (gold set, row to items to collect); 1 7107 "…has been closed."; 2 7108 "…registered by you."; 3 7102 "You don't have enough golds." (the client's action notes) | sends: CMarketMgr::DirectBuy, CUser::PacketMarket | both |
| `0x1C0A` | C→S | 6 | — | buy now, after "Do you want to process buy now option for <i> with <b>golds?" (7110) (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C0B` | S→C | var | u32 u8 if:u32 if:bytes[48] if:u32 | 0 7106 "Bidding Successful"; 1 7107; 2 7108; 3 7109 "…bid already."; 4 7102; 5 7104 "You can't bid lower price than minimum bidding price." (row price ← u32) (the client's action notes) | sends: CMarketMgr::Tender, CUser::PacketMarket | both |
| `0x1C0B` | C→S | 10 | +2:u32 +6:u32 | bid (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C0C` | S→C | var | u8 if:u32 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:u8 if:u8 if:u8 if:u… | 0: item added by the `0x0205` logic (`0x55a670`; the fields after `id` are `TP_ITEM_GET`'s), 7121; else 7122 (the client's action notes) | sends: CUser::MarketItemGet, CUser::PacketMarket | both |
| `0x1C0C` | C→S | 6 | +2:u32 | collect an item; "collect all" sends the next one after each success (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C0D` | S→C | var | u8 if:u32 if:u32 | 0: gold set, chat 53 "Gained <b>G" with the difference, 7123; else 7124 (the client's action notes) | sends: CMarketMgr::CharGetMoney, CUser::PacketMarket | both |
| `0x1C0D` | C→S | 6 | +2:u32 | collect gold; "collect all" likewise (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C0E` | S→C | 55 | bytes[53] | notices "<i>item has been sold successfully." (7130) / "<i>item sale has been failed." (7134) (the client's action notes) | sends: CMarketMgr::CheckMarketItemEndDate, CMarketMgr::DirectBuy | both |
| `0x1C0F` | S→C | 43 | bytes[41] | "<i>item auction has been canceled due to expiration." (7131) / "<i>item has been sold." (7133) (the client's action notes) | sends: CMarketMgr::CheckMarketItemEndDate | both |
| `0x1C10` | S→C | 13 | bytes[11] | "<i>item has been bid with <b> golds." (7132); updates my registration's price (the client's action notes) | sends: CMarketMgr::Tender | both |
| `0x1C11` | S→C | 43 | bytes[41] | "<i>item auction has been canceled due to expiration." (7131) / "<i>item has been sold." (7133) (the client's action notes) | sends: CMarketMgr::CheckMarketItemEndDate | both |
| `0x1C12` | S→C | 55 | bytes[53] | notices "<i>item has been sold successfully." (7130) / "<i>item sale has been failed." (7134) (the client's action notes) | sends: CMarketMgr::DirectBuy, CMarketMgr::RetrievalItem, CMarketMgr::Tender … | both |
| `0x1C13` | S→C | 3 | u8 | 0 "Vault is preparing for auction open." (7135); 1 "Vault Auction has been opened." (7136) (the client's action notes) | sends: CUser::PacketMarket | both |
| `0x1C14` | S→C | var | if:u8 loop:bytes[48] | interest list (the client's action notes) | sends: CMarketMgr::GetConcernList | both |
| `0x1C14` | C→S | 2 | — | interest list (tab 3), every time the tab opens (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C15` | S→C | var | u8 if:bytes[48] | 0 7125 "registered on your interest list"; 1 7126 "This product has been sold."; 2 7127 "already registered"; 3 7128 "can't add … anymore" (the client's action notes) | sends: CMarketMgr::AddConcertMarket, CUser::PacketMarket | both |
| `0x1C15` | C→S | 6 | +2:u32 | add to the interest list (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C16` | S→C | 7 | u32 u8 | 0: row removed (the client's action notes) | sends: CUser::PacketMarket | both |
| `0x1C16` | C→S | 6 | +2:u32 | remove from the interest list (the client's action notes) | handled: CUser::PacketMarket | both |
| `0x1C17` | S→C | 3 | u8 | 0: interest list cleared (the client's action notes) | sends: CUser::PacketMarket | both |
| `0x1C17` | C→S | 2 | — | clear the interest list, after 7129 (the client's action notes) | handled: CUser::PacketMarket | both |

### 1Dxx — legacy mail-box account

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x1D02` | S→C | 5 | u8 u8 u8 | legacy mail-box account: create result `u8` → 7147 / 7148 (`0x55baf0`, confirmed) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D04` | S→C | 16 | u8 bytes[13] | legacy mail-box: account name (`u8, char[13]`) stored (`0x55b9f0`) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D06` | S→C | 11 | u8 u8 u8 u8 u8 u32 | legacy mail-box auction: bid result → 7151 / 7157 (`0x55bb80`) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D07` | S→C | var | u32 bytes[257]* bytes[21]* bytes[13]* u32 u32 u32 u8 u8 u8 u16 u8 u8… | legacy mail-box: one entry (257-char text, 21-char name, 13-char account, values, an item with gems and craft name) added to a list (`0x55be20`) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D0B` | S→C | var | u16 loop:u32 loop:bytes[257]* loop:bytes[21]* loop:bytes[13]* loop:u3… | legacy mail-box: list of those entries; 7161 when empty (`0x55bf00`) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D0E` | S→C | 7 | u32 u8 | legacy mail-box: entry removed (`u32 id, u8`) / 7153 (`0x55bc30`) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D10` | S→C | 3 | u8 | legacy mail-box: `u8` → 7154 (`0x55bc90`) | never (no opcode store) | client-only (legacy, no request side) |
| `0x1D11` | S→C | var | u32 u32 u8 u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | legacy mail-box: claim result: 0 → removes the entry, puts an item in `bag/slot` with "You get <b>." (7166) and adds gold; 1 → 7156 (`0x55bcd0`, confirmed) | never (no opcode store) | client-only (legacy, no request side) |

### 21xx — obelisks and roads

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x2101` | S→C | var | u8 var | obelisk list (`u8 n`, n × 13) | sends: CMap::SendObeliskList | both |
| `0x2102` | S→C | 7 | u32 u8 | obelisk / portal-link change (`u32, u8`) | sends: CMap::SendObeliskChange, CMap::SendPortalLinkChange | both |
| `0x2103` | S→C | 6 | u16 u8 u8 | obelisk destroyed (`u16, u8, u8`) | sends: CMap::ObeliskDestroy | both |
| `0x2104` | S→C | 6 | u16 u8 u8 | portal link (road) change; **the client never prints its messages** (byte compared with 1010–1021, `obelisks.md`) | sends: CMap::UpdatePortalLink | both |

### 22xx–27xx — friends, personal shop, duels, guild battles, item mall, web inventory

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x2201` | S→C | var | u8 loop:u32 loop:u8 loop:u8 loop:bytes[21]* loop:u8 loop:var* | friend list at login | sends: CUser::SendCharacterFriend | both |
| `0x2202` | S→C | var | bytes[21]* | buddy request from name: dialog mode 0x0B (the client's action notes) | sends: CUser::PacketComm | both |
| `0x2202` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | *EP6/PDB name* GameFriendRequestIncoming (TPC_COMM_FRIEND_REQ) | handled: CUser::PacketComm | both |
| `0x2203` | S→C | 3 | u8 | 0 "Buddy registration has been rejected." (399); 1 "…accepted." (400); 2 "User is not in the same area." (401); other: nothing (the client's action notes) | sends: CUser::LeaveZone, CUser::PacketComm | both |
| `0x2203` | C→S | 3 | +2:u8 | answer a buddy request (1 accept, 0 refuse) | handled: CUser::PacketComm | both |
| `0x2204` | S→C | var | u32 u8 bytes[21]* | adds the friend (marked online) (the client's action notes) | sends: CUser::PacketComm | both |
| `0x2205` | S→C | 6 | u32 | *EP6/PDB name* FriendRemoveOutgoing (TP_COMM_FRIEND_DEL) | sends: CUser::PacketComm | both |
| `0x2205` | C→S | 6 | +2:u32 | *EP6/PDB name* GameFriendRemoveIncoming | handled: CUser::PacketComm | both |
| `0x2206` | S→C | var | u32 u8 var* | *EP6/PDB name* GameFriendMemoOutgoing (TP_COMM_FRIEND_MEMO) | sends: CUser::PacketComm | both |
| `0x2206` | C→S | var | — | *EP6/PDB name* GameFriendMemoIncoming | handled: CUser::PacketComm | both |
| `0x2207` | S→C | 7 | u32 u8 | friend online flag (the client's action notes) | sends: CUser::InitFriend, CUser::PacketComm, CUser::ReleaseFriend | both |
| `0x2208` | S→C | var | u8 loop:u32 loop:bytes[21]* loop:u8 loop:var* | block list at login | sends: CUser::SendCharacterFriend | both |
| `0x2209` | S→C | var | u32 bytes[21]* | *EP6/PDB name* GameBlockAddOutgoing (TP_COMM_BLOCK_ADD) | sends: CUser::PacketComm | both |
| `0x2209` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | *EP6/PDB name* GameBlockAddIncoming (TPC_COMM_BLOCK_ADD) | handled: CUser::PacketComm | both |
| `0x220A` | S→C | 6 | u32 | *EP6/PDB name* GameBlockRemoveOutgoing (TP_COMM_BLOCK_DEL) | sends: CUser::PacketComm | both |
| `0x220A` | C→S | 6 | +2:u32 | *EP6/PDB name* GameBlockRemoveIncoming | handled: CUser::PacketComm | both |
| `0x220B` | S→C | var | u32 u8 var* | *EP6/PDB name* GameBlockMemoOutgoing (TP_COMM_BLOCK_MEMO) | sends: CUser::PacketComm | both |
| `0x220B` | C→S | var | — | *EP6/PDB name* GameBlockMemoIncoming | handled: CUser::PacketComm | both |
| `0x220C` | S→C | 3 | u8 | party search: non-zero 404 "Registered on Party Search", 0 405 "Failed to register…" (the client's action notes) | sends: CUser::PacketComm | both |
| `0x220C` | C→S | 2 | — | register on the party search | handled: CUser::PacketComm | both |
| `0x220D` | S→C | var | u8 loop:u8 loop:u8 loop:bytes[21]* | party-search results (`u8 n`, n × (`u8 level, u8 job, char name[21]`)) | sends: CUser::PacketComm | both |
| `0x220E` | S→C | 3 | u8 | non-zero "Succeeded in blocking." (402); 0 "Failed to block." (403) (the client's action notes) | sends: CUser::PacketComm | both |
| `0x2301` | S→C | var | — | personal shop: enter setup mode | sends: CUser::PacketMyShop | both |
| `0x2301` | C→S | 2 | — | open a personal shop (setup) | handled: CUser::PacketMyShop | both |
| `0x2302` | S→C | var | — | personal shop: setup cancelled | sends: CUser::PacketMyShop, stMyShop::Ended | both |
| `0x2302` | C→S | 2 | — | cancel the setup | handled: CUser::PacketMyShop | both |
| `0x2303` | S→C | var | — | personal shop opened (409) | sends: CUser::PacketMyShop | both |
| `0x2303` | C→S | var | — | open the shop with a title | handled: CUser::PacketMyShop | both |
| `0x2304` | S→C | var | — | personal shop closed (410) | sends: CUser::PacketMyShop, stMyShop::Stopped | both |
| `0x2304` | C→S | 2 | — | close the shop | handled: CUser::PacketMyShop | both |
| `0x2305` | S→C | 9 | u8 u8 u8 u32 | item placed in a shop slot (echo of the request) | echo: `PacketMyShop` case `0x2305` sends the request back | both |
| `0x2305` | C→S | 9 | +2:u8 +3:u8 +4:u8 +5:u32 | put an item in a shop slot (`u8 bag, slot, shopSlot, u32 price`) | handled: CUser::PacketMyShop | both |
| `0x2306` | S→C | 3 | u8 | shop slot cleared (echo) | echo: `PacketMyShop` case `0x2306` | both |
| `0x2306` | C→S | 3 | +2:u8 | remove a shop item | handled: CUser::PacketMyShop | both |
| `0x2307` | S→C | 7 | u8 u32 | shop slot price changed (`u8 slot, u32 price`) | never: `PacketMyShop` handles a `0x2307` request (price change) without answering; the client never sends one | **client-only** |
| `0x2308` | S→C | 8 | u8 u8 u32 | an item sold: "<p> is sold with <b>G." (412), 5357 commission on map 42 | sends: CUser::PacketMyShop | both |
| `0x2309` | S→C | 7 | u8 u32 | visit a shop: dialog 0x0E | sends: CUser::PacketMyShop | both |
| `0x2309` | C→S | 6 | +2:u32 | visit a player's shop | handled: CUser::PacketMyShop | both |
| `0x230A` | S→C | var | — | left a visited shop | sends: CUser::PacketMyShop | both |
| `0x230A` | C→S | 2 | — | leave a visited shop | handled: CUser::PacketMyShop | both |
| `0x230B` | S→C | var | u8 var | visited shop item list (`u8 n`, n × 37) | sends: stMyShop::SendItemList | both |
| `0x230C` | S→C | var | — | the visited shop closed | sends: stMyShop::SendStopped | both |
| `0x230D` | S→C | var | u8 if:u32 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:bytes[6… | buy result: 0 413, 1 415 + item, 2 303, 3 469 | sends: CUser::PacketMyShop | both |
| `0x230D` | C→S | 4 | +2:u8 +3:u8 | buy from a visited shop | handled: CUser::PacketMyShop | both |
| `0x230E` | S→C | 4 | u8 u8 | a visited shop slot's count | sends: CUser::PacketMyShop | both |
| `0x2401` | S→C | 10 | u32 u32 | duel request "<p> requested battle to <t>." (5325) | sends: CUser::PacketPVP | both |
| `0x2401` | C→S | 6 | +2:u32 | request a duel | handled: CUser::PacketPVP | both |
| `0x2402` | S→C | 7 | u8 u32 | duel answer/result: 0 5326, 2 5327, 3 5328, 4 5334 | sends: CUser::PacketPVP, CUser::PvPChkLeaveByAttack, CUser::PvPChkLeaveByDeath … | both |
| `0x2402` | C→S | 3 | +2:u8 | answer a duel (1 accept, 0 refuse) | handled: CUser::PacketPVP | both |
| `0x2403` | S→C | 10 | u32 u32 | duel area centre `f32 x, z` after both stakes are ready; 6 s countdown (`duels.md`) | sends: CUser::PacketPVP | both |
| `0x2404` | S→C | var | — | duel fight starts: 5311 (`duels.md`) | sends: CUser::PvPChkLeaveByTime | both |
| `0x2405` | S→C | 3 | u8 | duel result: 1 won, 2 lost, 3 draw (`duels.md`) | sends: CUser::PvPChkLoseDeath | both |
| `0x2406` | S→C | 7 | u8 u32 | duel cancelled/ended: **0** disconnect 5313, **1** out of area 5314/5336, **2** gave up 5315/5310, **3** attacked 5329 (`duels.md`) | sends: CUser::PacketPVP, CUser::PvPChkLeaveByAttack, CUser::PvPChkLeaveByDeath … | both |
| `0x2406` | C→S | 2 | — | `/defeat`: give up the duel | handled: CUser::PacketPVP | both |
| `0x2407` | S→C | 7 | u32 u8 | duel stake window opens (`u32 partner, u8` 0 ok / 2 busy) (`duels.md`) | sends: CUser::PacketPVP | both |
| `0x2408` | S→C | 3 | u8 | duel stake window closed (2 by me, 3 by the partner) (`duels.md`) | sends: CUser::CancleActionExc, CUser::ExchangePvP, CUser::PacketPVP … | both |
| `0x2409` | S→C | 4 | u8 u8 | duel stake ready flags (as `0x0A05`) | sends: CUser::ExchangeCancleReadyPvP, CUser::PacketPVP | both |
| `0x2409` | C→S | 3 | +2:u8 | duel stake ready | handled: CUser::PacketPVP | both |
| `0x240A` | S→C | 6 | u8 u8 u8 u8 | duel stake item added (as `0x0A06`) | sends: CUser::PacketPVP | both |
| `0x240A` | C→S | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | duel stake add item | handled: CUser::PacketPVP | both |
| `0x240B` | S→C | 4 | u8 u8 | duel stake item removed | sends: CUser::ExchangeItemRemove, CUser::PacketPVP | both |
| `0x240B` | C→S | 3 | +2:u8 | duel stake remove item | handled: CUser::PacketPVP | both |
| `0x240C` | S→C | 7 | u8 u32 | duel stake money | sends: CUser::PacketPVP | both |
| `0x240C` | C→S | 6 | +2:u32 | duel stake money | handled: CUser::PacketPVP | both |
| `0x240D` | S→C | var | u8 u8 u8 u8 u16 bytes[6] bytes[21]* | *EP6/PDB name* GamePvPExcAddDestOutgoing_EP5 (TP_EXC_ADD_DEST) | sends: CUser::PacketPVP | both |
| `0x2501` | S→C | 10 | u32 u32 | *EP6/PDB name* GamePvPGuildRequestOutgoing (TP_PVPG_REQ) | sends: CUser::PacketPVPGuild | both |
| `0x2501` | C→S | 6 | +2:u32 | *EP6/PDB name* GamePvPGuildRequestIncoming | handled: CUser::PacketPVPGuild | both |
| `0x2502` | S→C | 7 | u8 u32 | *EP6/PDB name* GamePvPGuildAnswerOutgoing (TP_PVPG_ANS) | sends: CUser::PacketPVPGuild, CUser::PvPChkLeaveByDeath, CUser::PvPChkLeaveByGuildLeave … | both |
| `0x2502` | C→S | 3 | +2:u8 | *EP6/PDB name* GamePvPGuildAnswerIncoming (TPC_PVPG_ANS) | handled: CUser::PacketPVPGuild | both |
| `0x2503` | S→C | 14 | u32 u32 u32 | guild war: area centre `f32 x, z`, enemy guild id; countdown (`guild-wars.md`) | sends: CUser::PacketPVPGuild | both |
| `0x2504` | S→C | var | — | guild war fight starts: 5316 (`guild-wars.md`) | sends: CUser::PvPChkLeaveByTime | both |
| `0x2505` | S→C | 3 | u8 | guild war result: 1 own guild won, 2 enemy won, 3 draw (`guild-wars.md`) | sends: CGuild::PvPUserLeave | both |
| `0x2506` | S→C | 7 | u8 u32 | guild war member event: 0 disconnect, 1 out of area, 2 gave up, 3 defeated (`guild-wars.md`) | sends: CGuild::PvPUserLeave | both |
| `0x2601` | S→C | var | u32 u8 loop:bytes[29] | item-mall points (`u32`) and the purchase log | sends: CUser::SendPointLog | both |
| `0x2602` | S→C | var | u8 if:u32 if:bytes[21]* if:u32 if:u32 if:u8 if:var | *EP6/PDB name* GamePointBuyOutgoing (TP_POINT_BUY) | sends: CUser::PacketPoint | both |
| `0x2602` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | *EP6/PDB name* GamePointBuyIncoming (TPC_POINT_BUY) | handled: CUser::PacketPoint | both |
| `0x2603` | S→C | var | u8 if:bytes[21]* if:u32 if:bytes[21]* if:u32 if:u32 | *EP6/PDB name* GamePointGiftSendOutgoing (TP_POINT_GIFT_SEND) | sends: CUser::PacketPoint, CUser::PacketUserDBPoint | both |
| `0x2603` | C→S | 44 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u32 +27:u32 +31:u32… | *EP6/PDB name* GamePointGiftSendIncoming (TPC_POINT_GIFT_SEND) | handled: CUser::PacketPoint | both |
| `0x2604` | S→C | var | bytes[21]* bytes[21]* u8 loop:u8 loop:u8 loop:u8 loop:u8 | item-mall gift received: sender, product, items | sends: CUser::PacketUserDBPoint | both |
| `0x2605` | S→C | 6 | u32 | *EP6/PDB name* GamePointOutgoing | sends: CUser::PacketPoint, CUser::PacketUserDBPoint | both |
| `0x2605` | C→S | 2 | — | *EP6/PDB name* GamePointRequest | handled: CUser::PacketPoint | both |
| `0x2606` | S→C | var | u8 loop:u8 loop:u8 loop:u8 loop:u8 | *EP6/PDB name* GamePointStoredItemListOutgoing (TP_POINT_STORED_ITEMLIST) | sends: CUser::SendCharacterStoredPointItem | both |
| `0x2607` | S→C | 5 | u8 u8 u8 | *EP6/PDB name* GamePointStoredItemGetOutgoing (TP_POINT_STORED_ITEMGET) | sends: CUser::PacketPoint | both |
| `0x2607` | C→S | 3 | +2:u8 | *EP6/PDB name* GamePointStoredItemGetIncoming (TPC_POINT_STORED_ITEMGET) | handled: CUser::PacketPoint | both |
| `0x2609` | S→C | var | u8 loop:u8 | clears item-mall storage slots: `u8 n`, n × `u8 slot` (< 240) (`0x559910`, confirmed) | never (no opcode store) | **client-only** |
| `0x2701` | S→C | var | u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 l… | **web inventory** list: `u8 n`, n × (`u8 slot, type, typeId, u16 quality, gems[6], count, char craftName[21]`) into a 36-slot store; items move to bags with `0x0204` bag 200 (`0x550d50`, window `0x4a36d0`, confirmed) | never (no opcode store) | **client-only** |
| `0x2702` | S→C | var | u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | web inventory: one slot set (same handler) | never (no opcode store) | **client-only** |
| `0x2703` | S→C | 3 | u8 | web inventory: one slot cleared (`0x550e50`) | never (no opcode store) | **client-only** |
| `0x2704` | S→C | 3 | u8 | Japan exit flow: 1 → dialog 0x12, else 10074 (the client's action notes, §4) | never (no opcode store) | Japan only |
| `0x2704` | C→S | 2 | — | `0x2704` `u8` (`0x550d20`): 1 opens dialog 0x12, else "cannot connect to server" (10074). confirmed (the client's action notes) | ignored (no `0x27xx` group in `PacketProcessing`) | Japan only (never sent here) |
| `0x2705` | C→S | 2 | — | — confirmed (the client's action notes) | ignored | Japan only (never sent here) |

### 31xx — mini-game

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0x3101` | S→C | 3 | u8 | *inferred*: mini-game (gambling house): enter result `u8` | sends: CUser::PacketMiniGame | both |
| `0x3101` | C→S | 3 | +2:u8 | *inferred*: mini-game: enter | handled: CUser::PacketMiniGame | both |
| `0x3102` | S→C | 11 | u8 u32 u32 | *inferred*: mini-game: slot bet `u8, u32, u32` | sends: CMiniGame::SlotBatting | both |
| `0x3102` | C→S | 2 | — | mini-game: slot bet | handled: CUser::PacketMiniGame | both |
| `0x3103` | S→C | 18 | u8 bytes[9] u8 u32 u8 | *inferred*: mini-game: slot result (9-byte reel, u32 winnings) | sends: CMiniGame::SlotRun | both |
| `0x3103` | C→S | 2 | — | mini-game: slot run | handled: CUser::PacketMiniGame | both |
| `0x3104` | S→C | 3 | u8 | *inferred*: mini-game: card start | sends: CMiniGame::CardStart | both |
| `0x3104` | C→S | 2 | — | mini-game: card start | handled: CUser::PacketMiniGame | both |
| `0x3105` | S→C | var | — | *inferred*: mini-game: slot restart | sends: CMiniGame::SlotRestart | both |
| `0x3105` | C→S | 2 | — | *inferred*: mini-game: slot restart | handled: CUser::PacketMiniGame | both |
| `0x3106` | S→C | 6 | u32 | *inferred*: mini-game: `u32` money | sends: CUser::PacketMiniGame | both |
| `0x3106` | C→S | 2 | — | mini-game: leave | handled: CUser::PacketMiniGame | both |
| `0x3107` | S→C | 8 | u8 u8 u32 | *inferred*: mini-game: card run `u8, u8, u32` | sends: CMiniGame::CardRun | both |
| `0x3107` | C→S | 3 | +2:u8 | mini-game: card pick | handled: CUser::PacketMiniGame | both |
| `0x3108` | S→C | 3 | u8 | *inferred*: mini-game: card next | sends: CMiniGame::CardNext | both |
| `0x3108` | C→S | 2 | — | mini-game: card next | handled: CUser::PacketMiniGame | both |
| `0x3109` | S→C | var | — | *inferred*: mini-game: card stop | sends: CMiniGame::CardStop | both |
| `0x3109` | C→S | 2 | — | *inferred*: mini-game: card stop | handled: CUser::PacketMiniGame | both |

### A1xx — login server

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0xA101` | S→C | 197 | u8 u8 u8 bytes[64] bytes[128] | login key exchange (ps_login) | ps_login | both |
| `0xA101` | C→S | 131 | +2:u8 +3:bytes[128] | login key exchange | ps_login | both |
| `0xA102` | S→C | 24 | u8 u32 u8 bytes[16] | login result (ps_login) | ps_login | both |
| `0xA104` | S→C | var | bytes[15] u8 loop:u16 | login: `char[15]`, list of `u16` (*inferred* login notice / server versions) | ps_login (not traced) | login server |
| `0xA105` | S→C | 3 | u8 | login: `u8` result of the `0xA105` request | ps_login (not traced) | login server |
| `0xA105` | C→S | 23 | +2:u8 +3:u32 +7:u32 +11:u32 +15:u32 +19:u32 | login: `u8` + 5 × u32 (*inferred* the OTP/card login path, unused) | ps_login (not traced) | login server |

### A2xx — login server

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0xA201` | S→C | var | u8 loop:bytes[38] | server list | ps_login | both |
| `0xA202` | S→C | 7 | u8 u32 | server select result + key | ps_login | both |
| `0xA202` | C→S | 7 | +2:u8 +3:u32 | select a server | ps_login | both |

### A3xx — game handshake

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0xA301` | S→C | 20 | u8 u8 bytes[16] | game handshake reply | sends: CWorld::LoginUser | both |
| `0xA301` | C→S | 22 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 | game handshake | handled: CUser::PacketProcessing | both |
| `0xA302` | S→C | 18 | bytes[16] | nProtect GameGuard CS authentication (`u8[16]`) | sends: CUser::NProtectCSASend | both |
| `0xA303` | S→C | var | u16 var | `u16 len, data`: passed to the anti-hack module (`0x405460`, the region-2 code path) (*inferred*) | never (no opcode store) | client-only (anti-hack, unused) |

### B1xx — billing, item-mall storage

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0xB101` | S→C | var | u8 loop:u8 loop:u8 loop:u8 loop:u8 | *EP6/PDB name* GameBillingItemListOutgoing (TP_BILLING_ITEMLIST) | sends: CUser::SendCharacterBillingItem | both |
| `0xB102` | S→C | 5 | u8 u8 u8 | *EP6/PDB name* GameBillingItemGetOutgoing (TP_BILLING_ITEMGET) | sends: CUser::PacketBilling | both |
| `0xB102` | C→S | 3 | +2:u8 | take an item from the item-mall storage | handled: CUser::PacketBilling | both |
| `0xB103` | S→C | 7 | u8 u32 | *EP6/PDB name* GameBillingServiceTypeOutgoing (TP_BILLING_SERVICETYPE) | sends: CUser::SendBillingInfo | both |
| `0xB104` | S→C | 6 | u32 | "3 hour-free service" time: shown **only with region feature 28** (off here): no effect on this client (`0x559cf0`, confirmed) | never (no opcode store) | client-only, **dead** (feature 28 off) |
| `0xB105` | S→C | 6 | u32 | *EP6/PDB name* GameBillingLogoutTimeOutgoing (TP_BILLING_LOGOFFTIME) | never (no opcode store) | **client-only** |
| `0xB106` | S→C | var | u8 var | **item-mall product names and prices**: `u8 n`, n × 76 (`char code[21], char name[51], u32 price`); for each code already in the mall list the client replaces the name and the price (`0x55a000`, confirmed) | never (no opcode store) | **client-only** |
| `0xB106` | C→S | 2 | — | ask for the item-mall product list (region feature 10, on): sent after a scene set-up (`0x426724`) | ignored: "there is no `0xB106` in this server" (`docs/original/ps_game.md` §6) | **client-only** |

### F1xx–FAxx (GM) — GM chat, notices and commands

| op | dir | size | fields | client | original ps_game | status |
|---|---|---|---|---|---|---|
| `0xF101` | S→C | var | u32 u8 var* | GM command result / notice (*EP6/PDB name* GameAdminChatNormalOutgoing (TP_CHAT_NORMAL)) | sends: CUser::PacketAdminChat | both |
| `0xF102` | S→C | var | u8 bytes[21]* u8 var* | GM command result / notice (*EP6/PDB name* GameAdminChatSendToOutgoing (TP_CHAT_SENDTO)) | sends: CUser::PacketAdminChat, UserSendChatTo | both |
| `0xF103` | S→C | var | bytes[21]* u8 var* | GM command result / notice (*EP6/PDB name* GameAdminChatTradeOutgoing) | sends: CUser::PacketAdminChat | both |
| `0xF104` | S→C | var | bytes[21]* u8 var* | GM command result / notice (*EP6/PDB name* GameAdminChatGuildOutgoing (TP_CHAT_SENDMAP)) | sends: CGuild::SendAllGuildAdminChat | both |
| `0xF105` | S→C | var | u32 u8 var* | GM command result / notice (*EP6/PDB name* GameAdminChatPartyOutgoing (TP_CHAT_SENDPARTY)) | sends: CUser::PacketAdminChat | both |
| `0xF106` | S→C | var | u16 var* | GM command result / notice (*EP6/PDB name* GameAdminNotifyOutgoing (TP_ADMIN_NOTIFY)) | sends: CWorld::SendNotice | both |
| `0xF107` | S→C | var | bytes[21]* | GM chat/whisper notices (mirrors `11xx`) | sends: CUser::PacketAdminChat | both |
| `0xF109` | S→C | var | bytes[21]* | GM chat/whisper notices (mirrors `11xx`) | sends: CUser::SendAdminSendToEnd | both |
| `0xF10A` | S→C | var | u32 u8 var* | GM chat/whisper notices (mirrors `11xx`) | sends: CUser::PacketAdminChat | both |
| `0xF10A` | C→S | var | — | GM command (sender `00589bb0`) | handled: CUser::PacketAdminChat | both |
| `0xF201` | S→C | var | u16 if:u32 if:u32 if:bytes[21]* if:u16 if:var* | GM command result / notice | sends: CUserQuestionMgr::GetQuestion | both |
| `0xF201` | C→S | 2 | — | GM command (sender `00587190`) | handled: CUser::PacketAdminUQ | both |
| `0xF202` | C→S | var | — | GM command (sender `005871c0`) | handled: CUser::PacketAdminUQ | both |
| `0xF203` | C→S | var | — | GM command (sender `00587230`) | handled: CUser::PacketAdminUQ | both |
| `0xF204` | S→C | 4 | u16 | GM command result / notice | sends: CUser::PacketAdminUQ | both |
| `0xF204` | C→S | 2 | — | GM command (sender `005872c0`) | handled: CUser::PacketAdminUQ | both |
| `0xF501` | S→C | 4 | u16 | GM command result / notice (*EP6/PDB name* GameAdminCmdErrorOutgoing (TP_ADMIN_CMDS_ERROR)) | sends: CUser::PacketAdminCmdA, CUser::SendAdminCmdError, CUser::SendAdminCmdSuccess | both |
| `0xF601` | S→C | 3 | u8 | GM command result / notice | sends: CUser::PacketAdminCmdA | both |
| `0xF601` | C→S | 24 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u8 | GM command (sender `005886f0`) | handled: CUser::PacketAdminCmdA | both |
| `0xF602` | S→C | 3 | u8 | GM command result / notice | sends: CUser::PacketAdminCmdA | both |
| `0xF602` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00588750`) | handled: CUser::PacketAdminCmdA | both |
| `0xF701` | S→C | 7 | u8 u32 | GM command result / notice (*EP6/PDB name* GameAdminCmdBSetStatusOutgoing) | sends: CUser::PacketAdminCmdB | both |
| `0xF701` | C→S | 28 | +2:u8 +3:u32 +7:u32 +11:u32 +15:u32 +19:u32 +23:u32 +27:u8 | GM command (sender `005882c0`) | handled: CUser::PacketAdminCmdB | both |
| `0xF702` | C→S | 5 | +2:u8 +3:u8 +4:u8 | GM command (sender `00588330`) | handled: CUser::PacketAdminCmdB | both |
| `0xF703` | C→S | 25 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u8 +24:u8 | GM command (sender `00588380`) | handled: CUser::PacketAdminCmdB | both |
| `0xF704` | C→S | 5 | +2:u16 +4:u8 | GM command (sender `005883f0`) | handled: CUser::PacketAdminCmdB | both |
| `0xF705` | C→S | 5 | +2:u16 +4:u8 | GM command (sender `00588430`) | handled: CUser::PacketAdminCmdB | both |
| `0xF706` | C→S | 2 | — | GM command (sender `00588540`) | handled: CUser::PacketAdminCmdB | both |
| `0xF707` | C→S | 2 | — | GM command (sender `00588570`) | handled: CUser::PacketAdminCmdB | both |
| `0xF708` | C→S | 2 | — | GM command (sender `005885a0`) | handled: CUser::PacketAdminCmdB | both |
| `0xF709` | S→C | var | — | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF709` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `005885d0`) | handled: CUser::PacketAdminCmdB | both |
| `0xF70A` | S→C | 4 | u16 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF70A` | C→S | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | GM command (sender `00588630`) | ignored (no case) | **client-only** |
| `0xF70B` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00588690`) | ignored (no case) | **client-only** |
| `0xF70C` | C→S | 6 | +2:u8 +3:u16 +5:u8 | GM command (sender `005884a0`) | handled: CUser::PacketAdminCmdB | both |
| `0xF70D` | C→S | 6 | +2:u8 +3:u16 +5:u8 | GM command (sender `005884f0`) | handled: CUser::PacketAdminCmdB | both |
| `0xF70E` | C→S | 6 | +2:u32 | GM command (sender `00588470`) | handled: CUser::PacketAdminCmdB | both |
| `0xF801` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587d20`) | handled: CUser::PacketAdminCmdC | both |
| `0xF802` | S→C | var | — | GM command result / notice | sends: CUser::PacketAdminCmdC | both |
| `0xF802` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587d80`) | handled: CUser::PacketAdminCmdC | both |
| `0xF803` | S→C | var | — | GM command result / notice | sends: CUser::PacketAdminCmdC | both |
| `0xF803` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587de0`) | handled: CUser::PacketAdminCmdC | both |
| `0xF804` | S→C | var | — | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF804` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587e40`) | handled: CUser::PacketAdminCmdC | both |
| `0xF805` | S→C | 4 | u16 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF805` | C→S | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | GM command (sender `00587ea0`) | ignored (no case) | **client-only** |
| `0xF806` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587f00`) | ignored (no case) | **client-only** |
| `0xF809` | C→S | 4 | +2:u16 | GM command (sender `00587f60`) | ignored (no case) | **client-only** |
| `0xF80A` | C→S | 4 | +2:u16 | GM command (sender `00587f90`) | ignored (no case) | **client-only** |
| `0xF80B` | S→C | 4 | u16 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF80B` | C→S | 25 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u16 | GM command (sender `00587fc0`) | handled: CUser::PacketAdminCmdC | both |
| `0xF80C` | S→C | 4 | u16 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF80C` | C→S | 25 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u16 | GM command (sender `00588080`) | handled: CUser::PacketAdminCmdC | both |
| `0xF80D` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `005880e0`) | handled: CUser::PacketAdminCmdC | both |
| `0xF80E` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00588140`) | handled: CUser::PacketAdminCmdC | both |
| `0xF80F` | S→C | 4 | u16 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF80F` | C→S | 44 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u32 +27:u32 +31:u32… | GM command (sender `005881a0`) | ignored (no case) | **client-only** |
| `0xF810` | S→C | 4 | u16 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF810` | C→S | 44 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u32 +27:u32 +31:u32… | GM command (sender `00588230`) | ignored (no case) | **client-only** |
| `0xF901` | S→C | var | u8 u16 u32 u32 u16 u16 u16 u16 u16 u16 u16 u16 u16 u16 u8 u8 u8 u8 lo… | GM command result / notice | sends: CUser::PacketAdminCmdD | both |
| `0xF901` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `005877d0`) | handled: CUser::PacketAdminCmdD | both |
| `0xF904` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF904` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587830`) | handled: CUser::PacketAdminCmdD | both |
| `0xF905` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF905` | C→S | 33 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:f32 +27:u32 +31:u16 | GM command (sender `00587890`) | handled: CUser::PacketAdminCmdD | both |
| `0xF906` | S→C | var | u8 var* | GM command result / notice | sends: CUser::PacketAdminCmdD | both |
| `0xF906` | C→S | var | — | GM command (sender `005879f0`) | handled: CUser::PacketAdminCmdD | both |
| `0xF907` | S→C | var | u8 var* | GM command result / notice | sends: CUser::PacketAdminCmdD, CUser::UpdateAdminNotice | both |
| `0xF907` | C→S | var | — | GM command (sender `00587a60`) | handled: CUser::PacketAdminCmdD | both |
| `0xF908` | S→C | var | u8 var* | GM command result / notice (*EP6/PDB name* GameAdminCmdDNoticeToOutgoing) | sends: CUser::PacketAdminCmdD, CUser::UpdateAdminNotice | both |
| `0xF908` | C→S | var | — | GM command (sender `00587ae0`) | handled: CUser::PacketAdminCmdD | both |
| `0xF909` | S→C | var | u8 var* | GM command result / notice | sends: CUser::PacketAdminCmdD, CUser::UpdateAdminNotice, CWorld::SendNotice | both |
| `0xF909` | C→S | var | — | GM command (sender `00587b90`) | handled: CUser::PacketAdminCmdD | both |
| `0xF90A` | S→C | var | u8 var* | GM command result / notice | sends: CUser::PacketAdminCmdD, CUser::UpdateAdminNotice, CWorld::SendNotice | both |
| `0xF90A` | C→S | var | — | GM command (sender `00587c10`) | handled: CUser::PacketAdminCmdD | both |
| `0xF90B` | S→C | var | u8 var* | GM command result / notice (*EP6/PDB name* GameAdminCmdDNoticeOutgoing (TP_ADMIN_CMDS_NOTICE)) | sends: CUser::PacketAdminCmdD, CUser::UpdateAdminNotice, CWorld::SendNotice | both |
| `0xF90B` | C→S | var | — | GM command (sender `00587ca0`) | handled: CUser::PacketAdminCmdD | both |
| `0xF90C` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xF90C` | C→S | 33 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:f32 +27:u32 +31:u16 | GM command (sender `00587940`) | handled: CUser::PacketAdminCmdD | both |
| `0xFA01` | C→S | 2 | — | GM command (sender `005872f0`) | ignored (no case) | **client-only** |
| `0xFA02` | C→S | 2 | — | GM command (sender `00587320`) | ignored (no case) | **client-only** |
| `0xFA05` | C→S | 2 | — | GM command (sender `00587350`) | ignored (no case) | **client-only** |
| `0xFA06` | C→S | 2 | — | GM command (sender `00587380`) | ignored (no case) | **client-only** |
| `0xFA07` | S→C | var | u8 var* | GM command result / notice | sends: CUser::PacketAdminCmdE | both |
| `0xFA07` | C→S | var | — | GM command (sender `005873b0`) | ignored (no case) | **client-only** |
| `0xFA08` | S→C | 16 | u16 u32 u32 u32 | GM command result / notice | sends: CUser::PacketAdminCmdE | both |
| `0xFA08` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587450`) | ignored (no case) | **client-only** |
| `0xFA09` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xFA09` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `005874b0`) | ignored (no case) | **client-only** |
| `0xFA0A` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xFA0A` | C→S | 12 | +2:u32 +6:u32 +10:u16 | GM command (sender `00587510`) | ignored (no case) | **client-only** |
| `0xFA0B` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `005875c0`) | ignored (no case) | **client-only** |
| `0xFA0C` | C→S | 2 | — | GM command (sender `00587620`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA0D` | S→C | var | — | GM command result / notice | sends: CUser::PacketAdminCmdE | both |
| `0xFA0D` | C→S | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | GM command (sender `00587650`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA0E` | S→C | var | — | GM command result / notice | sends: CUser::PacketAdminCmdE, CUser::UpdateAdminCmd, CZone::UpdateApplyEffect | both |
| `0xFA0E` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `005876b0`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA0F` | S→C | var | — | GM command result / notice | sends: CUser::PacketAdminCmdE | both |
| `0xFA0F` | C→S | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | GM command (sender `00587710`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA10` | S→C | var | — | GM command result / notice | sends: CUser::PacketAdminCmdE, CUser::UpdateAdminCmd, CZone::UpdateApplyEffect | both |
| `0xFA10` | C→S | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | GM command (sender `00587770`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA11` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xFA11` | C→S | 4 | +2:u16 | GM command (sender `00587560`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA12` | S→C | 20 | u32 u16 u32 u32 u32 | GM command result / notice | never (no opcode store) | **client-only** |
| `0xFA13` | C→S | 49 | +2:bytes[24] +27:u32 +31:u32 +35:u32 +39:u32 +43:u32 +47:u8 +48:u8 | GM command (sender `005887b0`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA14` | C→S | 28 | +2:bytes[24] +27:u8 | GM command (sender `00588830`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA15` | C→S | 31 | +2:bytes[24] +27:u32 | GM command (sender `00588880`) | handled: CUser::PacketAdminCmdE | both |
| `0xFA16` | C→S | 2 | — | GM command (sender `00588020`) | ignored (no case) | **client-only** |
| `0xFA17` | C→S | 2 | — | GM command (sender `00588050`) | ignored (no case) | **client-only** |
| `0xFA18` | C→S | 3 | +2:u8 | GM command (sender `00587590`) | handled: CUser::PacketAdminCmdE | both |


## In other episodes

- The EP3 server pack shares opcode groups, connection states and damage formulas with the EP4.5 server this page compares against
  (**ref** [Episode 3](episodes/ep3.md), citing the [EP3 server pack](../original/ep3.md)).
- For EP5.4 the server page reads the packets from an EP5.4 client decompilation and checks them in the EP6 client dump; it does not
  claim an original EP5.4 client was read (**ref** [Episode 5](episodes/ep5.md)). The EP6 buff, target-select, item-mall product list and
  chance-preview packets agree with that reading, with the changes listed on the server page (**ref** [Episode 6](episodes/ep6.md)).

## Open questions

- **Live** (nothing was run): `0xB106` changing a mall entry; `0x0004`'s message box.
- Who sends `0x0004` originally (session or login server); `0xA104`/`0xA105` (login) not traced. The server pages show that the kick of a second login closes the connection silently (**ref** [A second login](../original/client-extras.md#2-a-second-login-of-the-same-account)), but not that no original server ever sends `0x0004`.
- Web inventory (not pursued; kept here as reference only): the opener is found (confirmed (disasm)): a side tab on the inventory window
  (x left−16…left, y top+0xb2…top+0x10e, `0x4e6250`) toggles dialog mode 0x18 with a computed store (`SBB`/`AND 0x18` at `0x4e632d`),
  refused with 806 "Can use after closing other active windows." while another dialog is open. It exists **only for region 4** (Japan):
  four `CMP [0x6f1558],4` (`0x4e5952` textures, `0x4e5a8a` clean-up, `0x4e6282` click, `0x4e7594` draw). Needs a client patch on those
  four checks, the art (`item_inven_bg.tga`, `item_inven_button.tga`, `item_inven_button2.tga`, `web_inven.tga`: all missing from both
  archives) and a server that sends `0x2701`.
- The opcode-store scan cannot separate client packets from other servers' packets that share a value inside one function; rows whose
  "original" cell names unrelated functions (e.g. `0x0603`, `0x0A01`, `0x0B07`, `0x0D03`) should be read with the family pages.
- GM opcodes (`F1xx`–`FAxx`): checked one by one on [GM opcodes and commands](gm-and-commands.md), which supersedes the GM table above
  (e.g. `FA01`–`FA0B` are handled; `FA12`, `FA19`, `FA1A` are also sent).

## How the tables were made

The tables were produced in one pass from: a scan of every 16-bit opcode store over all 466 opcodes in `ps_game.exe`, the decompiled 35
`CUser::Packet*` functions, the client's generated catalogue and the per-opcode tables of the client pages, plus hand notes for the rows
those do not cover. To redo a row, look for the stores of `0xNNNN` in `ps_game.exe` and read the dispatcher cases.
