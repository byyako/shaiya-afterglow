# The united war server (ps_gamewar)

This page covers `ps_gamewar.exe`, the original's second game-type server. It is one shared battleground, "map 200" from the
home servers' side, which players from **several game servers** enter at the same time. The page covers what it is, how a player
gets there and back, how it keeps each player's data on their own home server, what it reports home, its maps and level bands,
and how the EP5.4 build differs. The new server does not build it (section 9).

Binaries:

| build | path | linked | SHA-256 |
|---|---|---|---|
| EP4.5 | `PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/ps_gamewar.exe` (byte-identical to the `PSM_Client/Bin/` copy) | 2009-04-01, VS 2003 | `feee89f46ea99d310010ca860a79e678edeea4873b1b0f0c42336a677317c85f` |
| EP5.4 | `PSM_Client/Bin/ps_gamewar.exe` (no PDB) | 2011-09-14, VS 2008, PDB path `c:\Shaiya\Server\Bin\` | `204fc8217bff5f495a42a7e07d14cea197ea9332bf7b1cae3ca8b6781df9009e` |
| EP3 | none: the EP3 pack has no war server | | |

This was static analysis only; nothing was executed. EP4.5 function names come from `ps_gamewar.pdb`. Its GUID matches the
executable but its age does not (exe 42, PDB 66; `docs/original/server-inventory.md`). Every address below was checked against the
disassembly: each named function begins at its address with a proper prologue, and its callers line up. Addresses are virtual
(base `0x400000`). Raw symbols were made with `tools/pdb_symbols.py`. The home server's side is read in the EP4.5 `ps_game.exe` and
the session server's side in `ps_session.exe`, both with their PDBs.

Status words: **confirmed** = read in the code. **inferred** = follows from confirmed code but is not shown directly.

## In short

- **It is a cross-server battleground.** One `ps_gamewar` process serves the players of every game server in a region. Players keep
  their own characters, and each character's saves go back to the database of the server it came from. It has no database of its
  own.
- **It is `ps_game` with about a dozen extra functions.** Of its 5,459 functions, 5,447 have the same name in `ps_game`. Only 12 are
  its own: the server table, one DB-agent and one game-log connection per home server, the war start positions, and reporting kills
  home. The EP4.5 copy is a build from 3.5 months before the shipped `ps_game`. It lacks the auction house, the Lua monster scripts
  and the console's live data reloads.
- **Its world is maps 100 to 108.** Light (Human, Elf) arrive in room 100 and Fury (Deatheater, Vail) in room 101. Each room has a
  merchant, a blacksmith, three gatekeepers and two portals home. The gatekeepers lead to maps 102, 103 and 104: copies of the PvP
  fields 18 (Proelium Frontier), 30 and 0 (D-Water Borderland), with the same monsters, obelisks and guards.
- **The fields have level bands.** The war server's gatekeeper refuses a destination unless the player's level is within that map's
  band: 10–15 for 102, 21–30 for 103, 31–60 for 104 (70 in EP5.4). The ordinary game server has no such check.
- **The session server decides who may go.** A move to map 200 (item 100169, a GM move, or a portal that leads there) asks the
  session. The answer is "go", "server saturated" (the war server is full) or "checking server" (it isn't there). Then the home server
  saves the character, lets go of it, and sends the client the war server's address (`0x0111`). Going home works the same way,
  with the home server's address.
- **Kills count at home.** The war server adds up faction kills and sends half of them (rounded down) to **every** home server,
  where they go into the faction kill count (the Goddess blessing grades, `docs/original/pvp-2.md` section 9).
- **Guild and chat still work across servers.** Guild members on the war server show as online at home, and whispers, guild chat
  and guild admin chat are relayed both ways, routed by server id. On the war server, the guild-name slot over a player's head
  shows the **home server's name** instead.
- **EP5.4 kept it and fixed the lag.** Its gamewar was linked the same day as its `ps_game`. Apart from the war code, the only
  differences are that the auction house is left out and the language table differs. The war data changes in small ways: the top
  band goes to 70, and map 104's exits now lead to the faction rooms rather than straight home.

## 1. What is in the binary

Confirmed.

The functions found only in `ps_gamewar` (names from the PDB):

| function | address | what it does |
|---|---|---|
| `StrParseServerMapping` | 0x404490 | parses one `[GAMESERVERS] GameNN` line (section 2) |
| `CServer::ConnectToDBAgentWar` | 0x406200 | opens one `CClientToDBAgent` per home server (port 30911) into `g_pClientToDBAgentWar[]` |
| `CServer::ConnectToLogGameWar` | 0x406370 | the same for the game logs (port 30912) into `g_pClientToGameLogWar[]` |
| `CUser::SendDBAgent`, `CUser::SendGameLog` | 0x4381a0, 0x4381d0 | send to **the player's** home DB agent or game log, by the index at `CUser+0x580c` |
| `CUser::GetRealServerID` | 0x462140 | the player's home server id (from `CUser+0x5790`, the server's DB id), or `0xFF` |
| `CMap::GetGameStartPosWar` | 0x424bc0 | a random start position per race from `MapWar.ini`'s `*GameStartMapPosWar` |
| `CWorld::SendKillCountWar` | 0x4114d0 | reports the faction kills home (section 6) |
| `CDamageMob::GetPartyDamage`, `GetPrivateDamage` | 0x4973e0, 0x497450 | no callers in this build |

Some shared functions behave differently on the war server:

- `CUser::GetGuildName` (0x4620e0) returns the home server's name from the server table, cut to 24 characters, instead of the
  guild's name. Its callers are `CUser::SendCharacterData`, `CUser::SendUserShape`, `CZone::SendUserShape` and
  `CUser::ChkTargetUser`.
- `CWorld::LogoutUser` sends a leaving player to their home server's address (section 4).
- The gatekeeper in `CUser::PacketMaininterface` checks the destination's level band (section 3).
- `CServerApp::OnConsoleCommand` has `/moveall` (section 4).

All 95 functions that write a character's data go through `CUser+0x580c`. Among them are the item, quest, skill, level, money and
death functions, party and guild membership, and the mini-game. A save always reaches the home server's database. The war server's
`[DB]` section is empty, so it has no database of its own.

**Present in EP4.5 `ps_game` but missing here** (1,555 names, mostly library code). Every function of the auction house
(`CMarketMgr`, `CUser::PacketMarket`), the offline sellers' money (`CWorld::*OffCharMoney`), the Lua monster scripts (`LuaMob`,
`CMob`'s state machine), the guild bank's game-log records, the console's live reloads (`CGameData::ReLoadConst*`,
`CItemDrop::ReItemCreate`), `CZone::MobGenEx` and `CWorld::CheckGuildRankStatus`. These are the features added to `ps_game` between
April and July 2009. Nothing shows they were left out on purpose. The EP5.4 build (section 8) has everything except the auction house.

## 2. Configuration

Confirmed, the loader at 0x4039f6 to 0x403b30 and `StrParseServerMapping`.

`Config/ps_gamewar.ini` has the same shape as `ps_game.ini` (`[SERVER] ServerName=Game254`, `[CONNECTION]`, `[GAMEINFO]`,
`[LOGFILE]`, `[MONITOR]`), plus:

```ini
[GAMESERVERS]
GameServerCount=2
Game01=Teos, 1, 1, 203.0.113.1, 10.0.0.1, 10.0.0.1
Game02=Ilios, 2, 2, 203.0.113.2, 10.0.0.2, 10.0.0.2
```

(This example is made up. The shipped files say `GameServerCount=1` and have no `Game01` line, so as shipped the server would
connect to no home server.)

The fields of one `GameNN` line go into a 0x38-byte entry of `g_GameWarServerInfo` (0x4f8558):

| field | entry offset | meaning | read by |
|---|---|---|---|
| name | +0x08 (24 bytes used) | the server's name, shown in place of guild names | `CUser::GetGuildName` |
| server id | +0x00 | the id in the login server list; must be below 256 | `GetRealServerID` (returns it minus 1) |
| DB id | +0x04 | matched against the player's `m_nServerDBID` (`CUser+0x5790`) | every lookup of a player's home |
| home game IP | +0x2c | the home game server's public address, which a player going home is sent to | `CWorld::LogoutUser` (0x40c53a) |
| DB agent IP | +0x30 | the home DB agent, port 30911 | `ConnectToDBAgentWar` |
| game log IP | +0x34 | the home game log, port 30912 | `ConnectToLogGameWar` |

Limits: more than 250 servers → "Diable 250+ game server" and the server does not start. A count of 0 → "Game server count is 0".
More than 256 is clamped to 256.

The home server's side (`ps_game.ini`, `InitLogFile` 0x403d73 to 0x403dbc) has two keys:

- `[UNITED_WAR_SERVER] PublicIP`: the war server's address. It is resolved with port 30810 and stored in `g_dwUnitedWarServer`
  (0x546f84), then sent to a leaving player in `0x0111`.
- `[NEW_SERVER] Check`: while it is non-zero, every move to map 200 is refused (section 4), along with the other effects listed in
  `docs/original/operator-console.md`. A newly opened server is kept out of the war.

`Data/MapWar.ini` takes the place of `Map.ini`. It has `MapCount = 9`, the four ordinary start positions,
`HU/EL/BA/DEGameStartMapPosWar`, and one `[SET_ZONE_100]` to `[SET_ZONE_108]` section per map. The keys are those of `Map.ini`
(`docs/original/maps-npcs.md` section 1), plus `GateKeeperLvMin` and `GateKeeperLvMax` (section 3). Maps 106 and 107 are dungeons
that respawn into rooms 100 and 101. `Data/ObeliskWar.ini` takes the place of `Obelisk.ini` and has the same format
(`docs/original/obelisks.md`). Its comments list "goddess's guidance" portal ids 1010 to 1050 (Light 1010 and 1011, Fury 1020 and
1021, to maps 47 and 45).

## 3. The maps

Confirmed, `tools/svmap.py` and `tools/npcquest.py` on the EP4.5 files.

| map | size | type | what it is |
|---|---|---|---|
| 100 | 136 | D | Light's room: the Human and Elf start (50.43, 7.09, 86.57) |
| 101 | 112 | D | Fury's room: the Deatheater and Vail start (97.33, 5.89, 42.03) |
| 102 | 1024 | F | map 18 (Proelium Frontier): same monster areas, NPCs and regions, 26 bytes differ. The obelisk at (560, 540) with its four guards (`ObeliskWar.ini`) |
| 103 | 1024 | F | map 30: same monsters and NPCs, 2 of its 4 portals. Its obelisk at (900, 180) |
| 104 | 2048 | F | map 0 (D-Water Borderland): same monster areas, 6 fewer NPCs, 2 of its 12 portals, 4 fewer regions. Its obelisk and eight guards |
| 105, 108 | 1024 | F | identical to each other: 26 monster areas, no NPCs, no portals |
| 106, 107 | 168, 192 | D | one monster area each: small dungeons that respawn into rooms 100 and 101 |

The home servers never load maps 100 to 108: `Map.ini` stops at 67 (`docs/original/maps-npcs.md` section 1). No gatekeeper or
portal reaches 105 to 108 in the shipped data.

**The rooms.** Each room holds five NPCs, with one gatekeeper per field. The names are the `NpcQuest.SData` Korean names.

| room | merchant | blacksmith | gatekeeper → 102 | → 103 | → 104 |
|---|---|---|---|---|---|
| 100 (Light) | 205 마리 비이라 (31 items) | 36 아니아 메이어 | 46 제나스 파커 | 47 미아 아이린 | 50 쿠룸 보더 |
| 101 (Fury) | 216 비네 윌슨 (31 items) | 37 트레버 멕컬리스터 | 48 케이휜 미니언 | 49 디아나 케르샤 | 51 노미드 샤우어 |

**The portals.** Each portal is limited to one faction (the svmap's country field: 1 Light, 2 Fury).

| map | portals |
|---|---|
| 100 | two, Light, all levels: to map 200 (home) |
| 101 | two, Fury, all levels: to map 200 (home) |
| 102 | Light → room 100, Fury → room 101 |
| 103 | Fury → room 101, Light → room 100; levels 1 to 99 only |
| 104 | Light and Fury: to map 200 (home) |

On the war server, a move to map 200 is the way home (section 4). The coordinates that map-200 portals carry go unused.

**The level bands.** Confirmed in the war server's gatekeeper (`CUser::PacketMaininterface` 0x4559cc to 0x4559e5). After the
usual checks (`docs/original/gatekeeper.md` section 2), it compares the player's level (`CUser+0x136`) with the destination map's
`GateKeeperLvMin` (`CMap+0x1e0`) and `GateKeeperLvMax` (`+0x1e4`). Below the minimum or above the maximum, the request fails like the
gatekeeper's other failures. The loader (`CMap::LoadGameMapConfig`, 0x42481b to 0x42485b) defaults the keys to 0 and 60, so a map
without them takes levels 0 to 60. Shipped bands: 102 levels 10 to 15, 103 levels 21 to 30, 104 levels 31 to 60. The gatekeeper
is the only reader of the bands that was found. Portals carry their own level range in the svmap (103's is 1 to 99). That GM moves
skip the bands is inferred. The ordinary `ps_game` reads no such keys and has no such check.

## 4. Getting there and back

Confirmed. Home side: EP4.5 `ps_game`, `CClientToMgr::OnRecv` 0x409850 (jump table at 0x409f78) and `CWorld::LogoutUser`
0x40c860. Session side: `ps_session` `CConnection::PacketGame` 0x403040 (jump tables at 0x40394c and 0x40396c).

1. **The request.** Any of three things asks for map 200:
   - item special 59 (the only such item is 100169, `docs/original/item-use.md`), which sends `0x0816 u32 user`;
   - a GM move to map 200 (`FA0A` or `FA11`, in `PacketAdminCmdE`), which sends `0x080C u32 user`;
   - a confirmed portal or gatekeeper destination on map 200 (in `PacketMaininterface`, 0x468ae9 and 0x469002), which also sends
     `0x080C u32 user`.

   Each is sent to the session manager (`g_pClientToMgr`). Before that, the request is dropped without an answer while a logout
   is already pending (`CUser+0x57dc`) or the player has no character in the world (`+0x68 == -1`). It is refused with `0x0110 2`
   to the client while `[NEW_SERVER] Check` is set.
2. **The session's answer** (case 0x403782, the same for `0x080C` and `0x0816`). The session sends the same opcode back,
   `u32 user, u8 result`:

   | asked by | result |
   |---|---|
   | the war server (connection id `0xFE`) | 0: going home is always allowed |
   | a home server, while no war server is registered (`CServerList+0x33a0`) or its entry is closed (`+0x339d`) | 2, "checking server" |
   | a home server, while the war server's user count (`+0x33a4`) is at or above its limit (`+0x33a6`) | 1, "server saturated" |
   | a home server, otherwise | 0 |
3. **The home server lets go.** On `0x080C` (case 0x409bc7) it sends `0x0110 result` to the client
   (`docs/client/packet-opportunities.md`). On 0 it also:
   - schedules a type-2 logout (`CUser+0x57d8` = 2, the type the quit request uses, `docs/original/world.md` section 4) for 5
     seconds later (`+0x57dc`);
   - sets the server-move flag (`+0x57e0` = 1);
   - saves the character (`SendDBAgentCharGetInfo`) and drops it from the DB agent (`SendDBAgentDisconnect`).

   `0x0816` (case 0x409c69) acts only on a player waiting for an item move (`+0x581c` set and `+0x5818 == 6`). It stores the
   result in `+0x57e4` and sets the server-move flag to 2 on result 0, or to 0 otherwise.
4. **The reconnect.** When the leaving player is logged out (`CWorld::LogoutUser`, logout type 2) with the server-move flag at 1,
   the home server:
   - sends `0x080A u32 user, u32 (CUser+0x128)` to the session, which records the time and the value on its user record
     (case 0x403722);
   - sends the client **`0x0111 u32 address, u8[32]`**: the war server's address (`g_dwUnitedWarServer`) and the 32 bytes that
     `CWorld::LoginUser` copied to `CUser+0x57ec` when the player arrived.

   Without the flag, the player gets the ordinary `0x010D` (quit). The client's side of `0x0111` is in
   `docs/client/packet-opportunities.md`: it recreates its connection and reconnects. On arrival, the war server places the
   character in room 100 or 101 with `GetGameStartPosWar` (`CUser::PacketUserDBSetInfo`). It keeps the player's DB id (`+0x5790`)
   and uses it to pick the home DB agent (`CWorld::LoginUser`, `EnterUserWorld`). *Inferred:* the 32 bytes are what lets the
   session accept the reconnect on the other server. Their origin and the war server's check were not traced.
5. **Going home.** On the war server, any move to map 200 (the room portals, map 104's exits) sends the same `0x080C`. The
   session always answers 0. The war server's `LogoutUser` sends `0x0111` with the address of **the player's home game server**
   (the server table's +0x2c, found by DB id). If the player's DB id is not in the table, it logs "LogoutUser Error : n ==
   g_nMaxDBID" and sends `0x010D`. The console command **`/moveall`** (id 0x20, `CServerApp::OnConsoleCommand` case at 0x40348c,
   war server only) sends `0x080C` for every player in the game (`+0x5754 == 4`) with no logout pending. It empties the war
   server.

## 5. Data on the war server

Confirmed, the callers of `CUser::SendDBAgent` and `SendGameLog` (22 and 19 functions).

| what | goes to |
|---|---|
| items, money, experience, levels, stats, skills, quests, quick slots, character info | the home DB agent, through `CUser+0x580c` |
| item, trade, PvP, skill and death records | the home game log, through `CUser+0x580c` |
| guild membership changes (`CGuild::Join*`, `CreateAddUser`) | the home DB agent |
| the faction kill counts | the session, to every home server (section 6) |

There is no store of the war server's own, so nothing is lost when it restarts: every player is saved at home. The obelisks and
the faction kill counters on the war server are kept in memory only. That follows from the empty `[DB]` section; whether anything
else holds the obelisk state was not checked.

## 6. Kills reported home

Confirmed, `CWorld::SendKillCountWar` 0x4114d0, called from `CWorldSendThread::Run`. Session: case 0x403806. Home side: cases
0x409cec and 0x409d17.

The war server keeps the same four faction counters as `ps_game` (`CWorld+0x2dc` to `+0x2e8`, filled by `AddKillCount`,
`AddKillCountGrade` and `SubKillCount`). On every send-thread pass it takes `counter >> 1` of each one that is at least 2 and
sends it to the manager: `0x080D u8 country, u32 n` for the added counts and `0x0817 u8 country, u32 n` for the subtracted ones.
Then it sets the counter to 0, so an odd kill is lost. The session sends both opcodes on to every game server except the war
server (`CServer::SendAllGameExcludeWar` 0x4022c0, which skips connection id `0xFE`). Each home server applies `0x080D` as
`CWorld::AddKillCount(country, n)` and `0x0817` as `SubKillCount`. A war-server kill therefore counts at half weight toward
**every** realm's faction grade, not only the killer's.

## 7. Guilds and chat across servers

Confirmed. Session: cases 0x403818 to 0x403866. Home side: cases 0x409d41 to 0x409efb.

The session routes each message to the game server named in it (`CServer::FindGame`):

| opcode | routed by | home server does |
|---|---|---|
| `0x0810 u32, u32 char, u32 guild, u8 server` | the server byte (+0x0e) | `CGuild::ConnectWarUser`: the member shows online, and members get `0x0D0C 106` |
| `0x0811 u32, u32 char, u32 guild, u8 server` | the server byte (+0x0e) | `CGuild::DisconnectWarUser`: offline |
| `0x0812 u32 user, …` | the byte at +0x1b | a whisper (`0x1104`) to that player |
| `0x0814 u32 user, …` | as `0x0812` | an admin whisper (`0xF104`) |
| `0x0813 …, u32 guild, …, u8 server (+0x20), text` | the server byte (+0x20) | `CGuild::SendAllGuildChat` |
| `0x0815 …, u32 guild, …, u8 server (+0x20), text` | as `0x0813` | `CGuild::SendAllGuildAdminChat` |
| `0x080F u32 user` | | `CWorld::LeaveUserBill` (kick, reason 0x6D) |

On the war server, `CUser::PacketChatE` and `PacketAdminChat` call `GetRealServerID`, which supplies the server byte. Guild join and
leave on the war server go through the home DB agent (section 5).

## 8. The EP5.4 build

Confirmed from strings, PE headers and the data files (no PDB).

- **Linked the same day as `ps_game`** (2011-09-14 08:14 and 08:10, VS 2008), so the war server is current with the game.
- **Only in gamewar:** the same war-only strings as EP4.5 (`GAMESERVERS`, `Game%02d`, `*GameStartMapPosWar`,
  `GateKeeperLvMin/Max`, `data/ObeliskWar.ini`, `.\data\mapwar.ini`, `/moveall`, the DB-agent and game-log war connections). The
  log lines now name the address and port ("Connect DBAgentWar %d %s, %s").
- **Only in `ps_game`:** every `CMarketMgr` string, the auction house. It was left out of the war server on purpose this time,
  since a cross-server auction would need a shared market. `UNITED_WAR_SERVER` and `PublicIP` (the home side of the move) are also
  only in `ps_game`. `NEW_SERVER` is gone from both.
- **Language table:** `de_string.DB` in gamewar, `cn_string.DB` in game, where EP4.5 had `kr_string.DB`.
- **The Lua monster scripts are in** (`Lua/` 111 to 118, as in EP4.5's game).
- **Data:**

  | file | change from EP4.5 |
  |---|---|
  | `ObeliskWar.ini` | none |
  | `MapWar.ini` | map 104's `GateKeeperLvMax` 60 → 70, the EP5 level cap |
  | maps 100, 101, 102, 107 | byte-identical |
  | map 103 | its two portals' level range 1–99 → 0–999 |
  | map 104 | its two exits lead to the faction rooms (Light → 100, Fury → 101) instead of map 200, as 102's do; one NPC placement fewer each for two NPCs |
  | maps 105, 108 | one monster area and both bind areas moved slightly |
  | map 106 | declared size 168 → 1024, contents unchanged |

## 9. In the new server

Not built, by decision (`docs/server/deviations.md`, the "Not built, by decision" line of section 8). Concretely:

- Moves to map 200 are refused: item 100169, GM moves (`FA0A`, `FA11`), and gatekeeper or portal destinations on map 200, none of
  which exist in the shipped home-server data (`docs/original/gatekeeper.md`, `docs/original/operator-console.md`).
- Maps 100 to 108, `MapWar.ini` and `ObeliskWar.ini` are not imported.
- The session-relay opcodes of sections 4 to 7 (`0x080A` to `0x0817`) have no counterpart; the new server's programs share one
  database instead (`docs/server/architecture.md`).

**Quirks of the original**, for anyone reading the code:

- `SendKillCountWar` loses an odd kill on every pass, and the halved count goes to every realm, not only the killer's.
- The shipped `ps_gamewar.ini` names no home server, so as shipped the war server connects to nothing.
- The EP4.5 war server is a separate, older build, without four months of `ps_game` changes.
- In EP4.5, map 104's exits skip the faction rooms. EP5.4 corrects that.
- `CDamageMob::GetPartyDamage` and `GetPrivateDamage` exist but nothing calls them.
