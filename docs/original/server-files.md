# The original server's files: configuration and data reference

This page lists every file the EP4.5 server programs read or write: the seven program configs in `Config/`, the data tables in `Data/`,
and the files the servers write themselves. For each it says which program reads it and when, every section and key the code reads,
what a missing key falls back to, the limits, what the shipped files hold, and how to change it. Where a system has its own page, this
page gives the summary and links there for the detail. It was read in the disassembly with the PDB names (`ps_game`, `ps_login` of
2009, `ps_session`, `ps_dbAgent`, `ps_gameLog`; `ps_userLog` and `ps_gamewar` without names), from the shipped files in
`PSM_Client/Bin/` and `PSM_Server/ShaiyaServerPatch/`; nothing was executed. The pack itself is described on [Server files
inventory](server-inventory.md).

## In short

- **Two kinds of file.** `Config/<program>.ini` sets up one program: its name, its port, the other programs it connects to, its
  database and its log. `Data/` holds the game's tables, which only the game servers (`ps_game`, `ps_gamewar`) read.
- **Every config is read with the Windows profile functions**, from `.\Config\` under the program's working folder. A missing key
  falls back to a built-in default, usually without any message.
- **Ports are left blank in every shipped config**, so each program uses its built-in port: login 30800, game 30810, session 30900, user
  log 30901, DB agent 30911, game log 30912.
- **Database passwords are RSA-encrypted hex**, made by `PSM_Server/ps_passwordEnc.exe`. The other fields of a `[DB]` line are plain.
- **Most data files are read once, at start.** Four can be read again while the server runs: `ItemCreate.ini` (`/reitemcre`),
  `ItemEnchant.ini` (`/reitemenh`, with a bug), `SequenceItemCreate.ini` (`/reseqitem`), and the database tables behind `/remob` and the
  like. Everything else needs a restart.
- **The text files are Korean (code page 949)**, and most are read with their own parsers: comments are lines that start with `//`, and
  text after a value is not a comment in those files.
- **Some shipped keys do nothing**: `ps_game.ini` `[DB] GameData`, `ps_session.ini` `[BILLING_GUM]`, `ps_login.ini`
  `EnableServerComb` in the 2009 build, and `Map.ini`'s `MapName = -1` (`-1` is not special to the code).
- **The servers write three files besides their logs**: `Data/MobID.ini` (the monster counter), a hidden `Shaiya.SData` in the Windows
  folder (each character's money at logout), and `.\Log\*.log`.

Status words: **confirmed** = read in the code (address or function given), **inferred** = follows from confirmed code but is not shown
directly.

## 1. All the files

| file | read by | when | changed while running | detail |
|---|---|---|---|---|
| `Config/ps_login.ini` | `ps_login` | start | no | section 2.5 |
| `Config/ps_session.ini` | `ps_session` | start | no | section 2.6 |
| `Config/ps_game.ini` | `ps_game` | start (`[DB] GameDef` again at each `/re…` command) | no | section 2.4 |
| `Config/ps_gamewar.ini` | `ps_gamewar` | start | no | section 2.4, [United war server](gamewar.md) |
| `Config/ps_dbAgent.ini` | `ps_dbAgent` | start | no | section 2.7 |
| `Config/ps_gameLog.ini` | `ps_gameLog` | start | no | section 2.7, [The game log](game-log.md) |
| `Config/ps_userlog.ini` | `ps_userLog` | start | no | section 2.7 |
| `Data/Map.ini` | `ps_game` | start | no | section 3.1 |
| `Data/MapWar.ini` | `ps_gamewar` (in place of `Map.ini`) | start | no | section 3.1, [United war server](gamewar.md) |
| `Data/Obelisk.ini` | `ps_game` | start | no | section 3.2, [Obelisks](obelisks.md) section 11 |
| `Data/ObeliskWar.ini` | `ps_gamewar` (in place of `Obelisk.ini`) | start | no | section 3.2 |
| `Data/ItemCreate.ini` | `ps_game`, `ps_gamewar` | start | `/reitemcre` | section 3.3 |
| `Data/ItemEnchant.ini` | `ps_game`, `ps_gamewar` | start | `/reitemenh` (bug: section 3.4) | section 3.4 |
| `Data/SequenceItemCreate.ini` | `ps_game`, `ps_gamewar` | start | `/reseqitem` | section 3.5 |
| `Data/MobID.ini` | `ps_game`, `ps_gamewar` | start; **written** every hour and at shutdown | written by the server | section 3.6 |
| `Data/GuildHouse.ini` | `ps_game`, `ps_gamewar` | start | no | section 3.7 |
| `Data/mob_ai.ini` and `Data/AI/*` | `ps_game`, `ps_gamewar` | start | no | section 3.8 |
| `Data/kr_string.DB` | `ps_game`, `ps_gamewar` | start | no | section 3.9 |
| `Data/Lua/<n>.lua` | `ps_game` | each time a monster with that AI is created | the next monster made uses the file as it is then (inferred) | section 3.10 |
| `Data/Map/<id>.svmap` | `ps_game`, `ps_gamewar` | start | no | section 3.11 |
| `Data/NpcQuest.SData` | `ps_game`, `ps_gamewar` | start | no | section 3.12 |
| `Data/KillStatus.SData` | `ps_game`, `ps_gamewar` | start | no | section 3.13 |
| `<Windows folder>\Shaiya.SData` | `ps_game` | read and deleted at start; written at shutdown | written by the server | section 4 |
| `.\Log\PS_<PROGRAM>__system.log` | every program | written | | section 2.2 |

**Not read by any EP4.5 program** (confirmed: no reference in the images): the PSM manager's own files, `Data/AI/*.ac`, `*.PAI` and
`*_AI_FUNCTION.H` (the AI editor's other output, section 3.8), and `ServerCmd.ini`, which only EP5.4 uses (section 7). The monster,
item and skill definitions are not files: they come from the `PS_GameDefs` database ([Database notes](database-notes.md)).

## 2. The program configs (`Config/*.ini`)

### 2.1 How they are read

Confirmed, the `InitLogFile`, `InitServer` and `InitDB` functions of each program.

- **Windows profile files.** Each value is read with `GetPrivateProfileStringA` (sometimes `…IntA`) from a fixed path relative to the
  working folder, `.\Config\<program>.ini`. Sections and keys are matched without case. A line starting with `;` is a comment to
  Windows. The shipped files' `//` lines are not comments: Windows reads them as keys nobody asks for, which is harmless.
- **A missing or empty key** gives the default in the tables below. Most programs convert numbers with `atoi`, so text reads 0.
- **Addresses** (`[CONNECTION]`, `[BILLING_GBK] IP`) are `host[:port]`, split at the colon (`StrParseIP`). Without a port the built-in
  port of that service is used (section 2.3).
- **`[DB]` lines** are `server, database, user, password` (`StrParseDB`): four fields separated by commas, spaces trimmed. The password is
  the hex form of an RSA-encrypted text, decrypted with a key built into the programs (`DBCryptoPasswdDecFromHex`;
  [Protocol notes](protocol-notes.md)). `PSM_Server/ps_passwordEnc.exe` makes these blobs. A plain password does not work. An empty
  server field (the shipped `ps_gamewar.ini`) leaves the server name empty for the ODBC connection.

**Example: how a value is written.** Put a note on a line of its own that starts with `;`. A note after the value on the same line
becomes part of the value:

```ini
[CONNECTION]
; the session server on this machine, at its built-in port 30900
Session=127.0.0.1
; the session server on another machine, at a port of our choosing
;Session=10.0.0.5:31900

[DB]
; server, database, user, password (the hex blob ps_passwordEnc.exe makes; never the plain password)
GameDef=127.0.0.1, PS_GameDefs, Shaiya, 52DF3B06881C9EB6...(256 hex digits in all)
```

### 2.2 The sections every program has

Confirmed, the same code in all seven (ps_game `InitLogFile` 0x403ca9 to 0x403d45 and `InitServer` 0x4041e9 onwards).

| section, key | default when missing | meaning |
|---|---|---|
| `[SERVER] ServerName` | the program's own name (ps_game: `PS_GAME`) | the name in the log's start line and in the handshakes with the other programs |
| `[SERVER] ServerPort` | the built-in port (section 2.3) | the TCP port this program listens on |
| `[SERVER] MaxConnection` | **32,767** | the most connections accepted at once (EP5.4's login server ignores it: [Login server](ps_login.md) section 17) |
| `[LOGFILE] Replace` | 1 | **non-zero**: the log is always `.\Log\PS_<PROGRAM>__system.log`. **0**: each start writes a new file, `.\Log\PS_<PROGRAM>__system.log__YYYYMMDD_HHMMSS.log` |
| `[LOGFILE] Append` | 1 | **non-zero**: add to the file (`a+t`); **0**: empty it at start (`w+t`). The folder is created if it is missing |
| `[MONITOR] Enable` | 0 | non-zero: open a UDP socket for the PSM management agent (`CMonitorNet`, `SUdp::Open`). If it cannot be opened ("Monitornet start failed") the program does not start |
| `[MONITOR] Port` | 0 | that UDP port (shipped 50000, 50001 for the log servers) |

Shipped: `Replace=1`, `Append=0` everywhere, so each program starts a fresh `PS_<PROGRAM>__system.log` and overwrites the last one; set
`Append=1` to keep the history, or `Replace=0` to keep one file per start. `ps_dbAgent` also writes `.\Log\PS_DBAGENT__charsave.log`.

**Example: the sections every program has.**

```ini
[SERVER]
; the name in the log's first line and in the handshakes
ServerName=Game01
; empty: the built-in port (30810 for a game server)
ServerPort=
; turn players away once 5000 are connected
MaxConnection=5000

[LOGFILE]
; always write .\Log\PS_GAME__system.log ...
Replace=1
; ... and add to it instead of emptying it at each start
Append=1

[MONITOR]
; no PSM management agent watching this program
Enable=0
Port=50000
```

### 2.3 The ports

Confirmed, the defaults in each `InitServer` (`ServerPort`) and at each `[CONNECTION]` key.

| program | listens on | others reach it as |
|---|---|---|
| `ps_login` | **30800** | (the clients) |
| `ps_game`, `ps_gamewar` | **30810** | (the clients; the war server's address in `[UNITED_WAR_SERVER]`) |
| `ps_session` | **30900** | `[CONNECTION] Session` |
| `ps_userLog` | **30901** | `[CONNECTION] UserLog` |
| `ps_dbAgent` | **30911** | `[CONNECTION] DBAgent` |
| `ps_gameLog` | **30912** | `[CONNECTION] GameLog` |

Change a port in both places: the program's own `ServerPort`, and `host:port` in every config that connects to it. The client is
configured with the login server's address only; the game server's address reaches players through the session and login servers
(`GamePublicIP`, section 2.4).

**Example: moving the game log to its own machine and port.** The game log listens on the new port, and every program that writes
to it is told the same port:

```ini
; ps_gameLog.ini
[SERVER]
ServerPort=31912

; ps_game.ini (and every other game server's)
[CONNECTION]
GameLog=10.0.0.20:31912
```

### 2.4 `ps_game.ini` (and `ps_gamewar.ini`)

Confirmed, `ps_game` `InitLogFile` 0x403c80, `InitServer` 0x4041b0, `InitDB` 0x403e79.

| section, key | default | meaning |
|---|---|---|
| `[CONNECTION] Session` | none: **the server does not start** | the session server, `host[:30900]`. "session connect failed" stops the start |
| `[CONNECTION] DBAgent` | none: does not start | the DB agent, `host[:30911]`; two connections are made to it |
| `[CONNECTION] UserLog` | none: does not start | the user log, `host[:30901]` |
| `[CONNECTION] GameLog` | none: does not start | the game log, `host[:30912]` |
| `[GAMEINFO] GameName` | (see note) | the name sent to the session server (`CClientToMgr::SendConnect`, kept in `g_szGameName`) |
| `[GAMEINFO] GamePublicIP` | none sent | the address players are given for this server: resolved by name (`gethostbyname`, so a host name works) and sent to the session server in `0x0805` with the version. The shipped `whatismyip.com` is a placeholder to replace with the server's real public address |
| `[GAMEINFO] GameVersion` | 0 | hexadecimal (`%X`, `0x` allowed). Sent with the address; the login server compares it with the client's version (`0x10042600` for the US client) while its version check is on, which it is at start. The shipped `0x00000000` makes the original login server refuse the US client until an operator types `/vchkoff` ([Login server](ps_login.md)) |
| `[GAMEINFO] GameLogDBID` | 0 | stored as the value **minus 1** (`g_nGameLogDBID`); read again by the reconnect loop. Which of the game log's `DBGameNN` databases this server's records go to (inferred; shipped 1 = `DBGame01`) |
| `[DB] GameDef` | none: does not start | the `PS_GameDefs` database (the definitions). Read again by each data reload command (section 5) |
| `[DB] GameData` | | **not read by `ps_game`** (confirmed: no reference); the game server reaches `PS_GameData` through the DB agent |
| `[UNITED_WAR_SERVER] PublicIP` | none | the war server's address (port 30810), sent to a player who goes to the war ([United war server](gamewar.md) section 2) |
| `[NEW_SERVER] Check` | 0 | a hidden switch for a newly opened server: counted player kills double, Lapisia enchanting at half price, and no move to the war map ([Operator console](operator-console.md)) |

Note on `GameName`: when the key is missing, the server copies an empty text over its name (0x404358, inferred from the code), so keep
it set.

**`ps_gamewar.ini`** has the same keys except `[CONNECTION] DBAgent` and `GameLog` (it connects to each home server's DB agent and game
log instead) and `[UNITED_WAR_SERVER]` (confirmed, the trace of its reads). It keeps `[NEW_SERVER] Check` and adds **`[GAMESERVERS]`**: `GameServerCount` (0 or above
250: the server does not start) and one `GameNN = name, server id, DB id, home game IP, home DB agent IP, home game log IP` line per
home server ([United war server](gamewar.md) section 2 has the table). The shipped file's server line is a placeholder.

**Example: a complete `ps_game.ini`** for one server with everything on one machine (`203.0.113.10` stands for the machine's public
address):

```ini
[SERVER]
ServerName=Game01
ServerPort=
MaxConnection=5000

[CONNECTION]
; every service on this machine, at its built-in port
Session=127.0.0.1
UserLog=127.0.0.1
DBAgent=127.0.0.1
GameLog=127.0.0.1

[DB]
GameDef=127.0.0.1, PS_GameDefs, Shaiya, 52DF3B06...(the encrypted password)

[GAMEINFO]
; keep the same name as the server's lines in ps_login.ini and ps_session.ini
GameName=Teos
; the address players are sent to: the public address or a host name, not 127.0.0.1
GamePublicIP=203.0.113.10
; the US client's version, so the login server's version check lets it in
GameVersion=0x10042600
; this server's records go to ps_gameLog's DBGame01
GameLogDBID=1

[LOGFILE]
Replace=1
Append=1

[MONITOR]
Enable=0
Port=50000

[UNITED_WAR_SERVER]
; the war server players go to through the war portal
PublicIP=203.0.113.50

[NEW_SERVER]
; 1 only for a newly opened server (double kill counts, cheaper enchanting, no war)
Check=0
```

**Example: the war server's list of home servers** (`ps_gamewar.ini`): name, server id, database id, then the home game server's
public address, its DB agent and its game log.

```ini
[GAMESERVERS]
GameServerCount=2
Game01=Teos, 1, 1, 203.0.113.10, 10.0.0.11, 10.0.0.12
Game02=Ilios, 2, 2, 203.0.113.20, 10.0.0.21, 10.0.0.22
```

### 2.5 `ps_login.ini`

Confirmed, `ps_login` of 2009 (`ShaiyaServerPatch/Server/ServerMain/PS_Login/`) `InitServer` 0x406407, `InitDB` 0x4067b2,
`CServerList::LoadCombServerList` 0x4076bd.

| section, key | default | meaning |
|---|---|---|
| `[CONNECTION] Session` | does not start | `host[:30900]` |
| `[CONNECTION] UserLog` | does not start | `host[:30901]` |
| `[DB] UserData` | does not start | the `PS_UserData` database (the accounts) |
| `[GAMESERVERS] GameServerCount` | 0 | how many `GameNN` lines follow; **above 250 the list is refused** (logged) |
| `[GAMESERVERS] Game01`, `Game02`, … | | `name, server id, DB id` (`StrParseServerMapping`): the name in the server list, the server's id (below 256, matching the game server's) and its database id ("use prev server id" in the shipped comment) |

**Two builds.** The `ps_login.exe` in `PSM_Client/Bin/` is the older one of 2007, and it reads the server list only with
`[GAMESERVERS] EnableServerComb = 1`. The 2009 build reads it always and has no such key. Use the 2009 build, which has a matching
PDB ([Server files inventory](server-inventory.md)).

**Example: two game servers in the login list.** Each line is the name players see, the server id (below 256) and the database id:

```ini
[CONNECTION]
Session=127.0.0.1
UserLog=127.0.0.1

[DB]
UserData=127.0.0.1, PS_UserData, Shaiya, 081EECD7...(the encrypted password)

[GAMESERVERS]
GameServerCount=2
Game01=Teos, 1, 1
Game02=Ilios, 2, 2
```

The shipped files give the server the same name in `ps_game.ini` `GameName`, in `ps_session.ini` and in `ps_login.ini` (all
`ServerName`); keep them the same (inferred from the shipped set; the matching code was not read).

### 2.6 `ps_session.ini`

Confirmed, `ps_session` `InitServer` 0x407c97 to 0x408037, `CServerList::Load` 0x4081e8.

| section, key | default | meaning |
|---|---|---|
| `[CONNECTION] UserLog` | does not start | `host[:30901]` |
| `[GAMESERVERS] GameServerCount`, `GameNN` | | the game servers it accepts, by name; up to 250 ("CServerList::Load diable 250+ game server" above that) |
| `[BILLING_GBK] Enable` | 0 | non-zero: connect to a GBK billing server (`SGBKClient::Open`) |
| `[BILLING_GBK] IP`, `Port` | | its address. A failed connection is logged ("GBK Server connect failed") and the start goes on |
| `[BILLING_GUM]` (all keys) | | **read by no build** (confirmed: the section name is in no image); leftovers of another billing system |

**Example:**

```ini
[CONNECTION]
UserLog=127.0.0.1

[GAMESERVERS]
GameServerCount=2
Game01=Teos
Game02=Ilios

[BILLING_GBK]
; no billing server: leave it off (the shipped [BILLING_GUM] section can be deleted, nothing reads it)
Enable=0
```

### 2.7 `ps_dbAgent.ini`, `ps_gameLog.ini`, `ps_userlog.ini`

Confirmed, the trace of each program's reads.

| program | its own keys | meaning |
|---|---|---|
| `ps_dbAgent` | `[DB] GameData` | the `PS_GameData` database (characters, items, guilds) it reads and saves for the game servers |
| `ps_gameLog` | `[DB] DBCount`, `DBGame01…`, `DBChat01…` | `DBCount` pairs of databases (`DBGame%02d`, `DBChat%02d`): the game log records and the chat log. Shipped `DBGame01 = PS_GameLog`, `DBChat01 = PS_ChatLog` ([The game log](game-log.md)) |
| `ps_userLog` | `[DB] UserLog` | the database of the user log (shipped `PS_UserData`) |

None of the three has a `[CONNECTION]` section: the other programs connect to them. `ps_userLog` reads `ps_userLog.ini` and
`ps_userlog.ini` (both spellings appear in the program; Windows does not care).

**Examples:**

```ini
; ps_dbAgent.ini
[DB]
GameData=127.0.0.1, PS_GameData, Shaiya, FAFB877F...(the encrypted password)

; ps_userlog.ini
[DB]
UserLog=127.0.0.1, PS_UserData, Shaiya, 133D58F3...(the encrypted password)

; ps_gameLog.ini: two pairs of databases; a game server with GameLogDBID=2 writes to the second (inferred)
[DB]
DBCount=2
DBGame01=127.0.0.1, PS_GameLog, Shaiya, 16AF507A...(the encrypted password)
DBChat01=127.0.0.1, PS_ChatLog, Shaiya, A732A019...(the encrypted password)
DBGame02=10.0.0.30, PS_GameLog2, Shaiya, ...(the encrypted password)
DBChat02=10.0.0.30, PS_ChatLog2, Shaiya, ...(the encrypted password)
```

## 3. The data files (`Data/`)

### 3.1 `Map.ini` (`MapWar.ini` for the war server)

Confirmed, `CMap::LoadGameMapConfig` 0x4270a0, `CMap::LoadConfig` 0x42696d, `ParseMapPos` 0x4264c0, `ParseMapPosArray` 0x426660,
`ParseMapOpenTimeArray` 0x426820, `CMap::Load` 0x4256f0. Read with the Windows profile functions from `.\data\map.ini`.

**Positions** are four numbers, `map, x, y, z`, in any of the forms the file uses: `-1, 0, 0, 0` or `(1, 577.98, 78.56, 1762.12)`. The
separators are spaces, commas, `=`, the brackets `( ) [ ] { }`, tabs and line ends. The map is a whole number and x, y, z decimals.

`[BASE]`:

| key | meaning |
|---|---|
| `MapCount` | how many maps are loaded, 0 to `MapCount − 1` (shipped 68). Every map needs a `[SET_ZONE_n]` or the defaults, and a map file |
| `HUGameStartMapPos`, `ELGameStartMapPos`, `BAGameStartMapPos`, `DEGameStartMapPos` | where a new character starts, per race (Human, Elf; Vail, Deatheater, in the code's order): a list of up to **64** positions, one picked at random for each new character. Missing: one built-in position each (`1, 813.0, 39.0, 1710.0`; `1, 1489.0, 30.0, 1720.0`; `2, 419.0, 230.0, 723.0`; `2, 1726.0, 262.0, 334.0`) |

`[SET_ZONE_n]`, one per map n. Keys marked "default" are taken from **`[DEFAULT_SET_ZONE]`** when the map's section lacks them:

| key | default from | meaning |
|---|---|---|
| `MapType` | yes (else `F`) | `D` = dungeon (`CMap +0x80` = 1), anything else a field. Dungeon maps change some rules, e.g. the item windows ([Item windows](item-windows.md)) |
| `MapName` | no | another map file under `data/map/`; missing = `<n>.svmap`. **`-1` is not special** to the code (the file's comment says it is), so `MapName = -1` in a map's section would look for a file named `-1` (inferred); leave the key out |
| `WarType` | no | `P` = a war map (`CMap +0x7c` = 1): there nobody counts as an enemy by country ([PvP](pvp.md)); maps 40 and 42 have it |
| `CreateType` | no | `S` 1, `F` 2, `P` 3 party instance, `G` 4 guild house, `R` 5 guild ranking, `H` 6; anything else 0, an ordinary shared map ([Party instance zones](instances.md) section 1, [Portals](portals.md)) |
| `RebirthMapPos` | yes | where players come back after death, both factions (`-1, 0, 0, 0` = none: the default point) |
| `RebirthMapPos1`, `RebirthMapPos2` | yes (`RebirthMapPos`) | the same per faction, Light and Fury ([Monsters fighting back](mob-fight.md) section 8) |
| `WeatherState` | yes | 0 none, 1 rain, 2 snow |
| `WeatherRate` | yes | the chance (%) that the weather comes, drawn after each wait |
| `WeatherPower` | yes | strength, 1 to 3 |
| `WeatherDelay` | yes | how long it lasts, minutes |
| `WeatherNoneDelay` | yes | the wait before the next draw (the comment gives no unit; minutes like `WeatherDelay`, inferred) |
| `CreateTime` | no | when an instance or guild map is open: up to **7** triples `(day, hour, hours)`; missing = always open ([Party instance zones](instances.md) section 2, [Guild ranking](guild-ranking.md)) |
| `CreateMinUser` | no | the fewest party members to enter (0) |
| `EnterMaxUser` | no | the most players in one zone (30) |
| `ExpireTime` | no | minutes an empty instance zone is kept (0: 30 s) |

The weather meanings come from the file's own comment block, which describes every key; the code stores them with `atoi`. The client is
told the weather in `0x0451` ([Movement](world.md) section 5).

**`MapWar.ini`** is read by `ps_gamewar` instead, with the same keys for maps 100 to 108, start positions `…GameStartMapPosWar`, and
`GateKeeperLvMin`/`Max` ([United war server](gamewar.md) section 2).

**Example: four kinds of map**, from the shipped file (the notes on `;` lines are added here):

```ini
[BASE]
MapCount = 68
; three start spots for Humans, one picked at random for each new character
HUGameStartMapPos = (1, 577.98, 78.56, 1762.12) (1, 580.10, 78.56, 1760.00) (1, 575.40, 78.56, 1764.30)

[DEFAULT_SET_ZONE]
; what a map gets when its own section leaves these keys out
MapType = F
RebirthMapPos = -1, 0, 0, 0
WeatherState = 0
WeatherRate = 30
WeatherPower = 2
WeatherDelay = 3
WeatherNoneDelay = 30

; a field with light rain: a 30 % chance every 60 minutes, lasting 10 minutes
[SET_ZONE_18]
MapType = F
WeatherState = 1
WeatherRate = 30
WeatherPower = 1
WeatherDelay = 10
WeatherNoneDelay = 60

; a field where each faction comes back from death at its own spot
[SET_ZONE_47]
MapType = F
RebirthMapPos1 = 47, 125.58, 8.4, 454.08
RebirthMapPos2 = 47, 571.37, 6.4, 194.08

; the auction house: a dungeon where nobody is an enemy
[SET_ZONE_42]
MapType = D
WarType = P

; a party instance, open Monday to Saturday from 21:00 for one hour (day 0 = Sunday),
; at least 18 party members to enter, at most 30 per zone, an empty zone kept 2 minutes
[SET_ZONE_64]
MapType = F
CreateType = P
CreateTime = (1, 21, 1)(2, 21, 1)(3, 21, 1)(4, 21, 1)(5, 21, 1)(6, 21, 1)
CreateMinUser = 18
EnterMaxUser = 30
ExpireTime = 2
RebirthMapPos1 = 47, 509.47, 7.3, 1525.32
RebirthMapPos2 = 47, 1404.15, 10.3, 308.92
```

(The three Human start spots are made up; the shipped file has one.)

### 3.2 `Obelisk.ini` (`ObeliskWar.ini`)

Confirmed, `CMap::LoadObelisk` 0x427300; the full key list is [Obelisks](obelisks.md) section 11.1.

Its own line parser, not the Windows functions: `//` starts a comment only at the start of a line, and **text after a value is not a
comment**. It holds, per `[zone_N]` map:

- `[Obelisk_K] = (x,y,z),(neutral,light,fury),c`: an obelisk, with the monster that stands for it per owner.
- `[Mob] = (x,y,z,r),(neutral,light,fury),n`: a guard group of the obelisk above.
- `[Portal_n, (map, portal)]`, then `[light]`/`[fury] = (map, Obelisk_K), …`: a portal opened by holding the named obelisks (the
  "roads"; the shipped file has none).
- `[bothmob_n, (map, stages, respawn, (pmap, portal))] = (x,y,z),(id1, …)`, then `[Mob]` followers and `[RandomRespawn]`: the boss
  groups (the dragons, the Cryptic One bosses, the guild-ranking bosses; 25 shipped).

The limits, the off-by-one and the bugs are in that section.

**Example:** the obelisk of map 18 with its four guard groups, and the first boss group (the comments are the file's own, translated).

```text
[zone_18]
    // the obelisk: where it stands, its monster while neutral / Light / Fury, then who holds it at start (1 Light, 2 Fury, 0 nobody)
    [Obelisk_001] = (560.34,2.20,540.20),(710, 712, 711),0
        // a guard group: centre and radius 3, the guard while neutral / Light / Fury, 2 of them
        [Mob] = (568.00,2.18,547.99,3),(713, 715, 714),2
        [Mob] = (551.08,2.39,531.39,3),(713, 715, 714),2

// a boss group on map 19: 2 stages, respawn 604800 s (7 days), it opens map 19's portal 1001
// the boss stands at (229.43, 1, 1031.10): 1974 first, 1977 when that one falls
[bothmob_001, (19, 2, 604800, (19,1001))]  =  (229.43, 1, 1031.10),(1974, 1977)
    // a follower: one monster per stage and one more (0 = none), radius 5, 1 of them
    [Mob]  = (237.85,1,1022.68,5),(0, 1980, 0), 1
```

### 3.3 `ItemCreate.ini`

Confirmed, `CItemDrop::InitItemCreate` 0x4be840 ([Where items come from](item-origins.md) section 5.1).

The box and pocket contents (items with `bySpecial` 42). Own parser: `[ItemCreate_n]` (n = 1 to 255) starts a set; each line in it is
`grade, weight`. Opening a box picks a line by weight (the weight over the set's total; **not** a percentage) and gives a random item of
that grade. Grades 0 to 1024; weight above 0; a grade with no items gives nothing; a grade given twice adds its weight to the first. The
box's `ReqVg` column (`shVG`) names its set; a box naming a set that is missing or empty gives an error and is kept ([Where items come
from](item-origins.md) section 5.2). Shipped: sets 1 to 68.

**Example:** set 19, the Fortune Bag, from the shipped file. The chances are the weights over their total (189): grade 1011 about
53 %, 1012 26 %, 1013 13 %, 1014 6 %, 1015 1 %. A box whose `ReqVg` is 19 opens with this set. (In the shipped data those five grades hold no items, so this bag gives nothing.)

```text
// lines starting with // or -- are comments; grade, then weight
[ItemCreate_19]
	1011	100
	1012	50
	1013	25
	1014	12
	1015	2
```

### 3.4 `ItemEnchant.ini`

Confirmed, `CGameData::InitItemEnchant` 0x4c7ab0 ([Lapis](lapis.md) section 2).

The Lapisia enchant tables, steps 00 to 20, read with the Windows functions:

| section | keys | meaning |
|---|---|---|
| `[LapisianEnchantPercentRate]` | `WeaponStep00` … `WeaponStep20`, `DefenseStep00` … `DefenseStep20` | the success chance of reaching that step |
| `[LapisianEnchantAddValue]` | the same keys | what the step adds: weapon attack, armour absorption |

A missing key reads **0**. When the file cannot be opened, the built-in tables stay. Values are unsigned, so a negative one wraps.
**`/reitemenh` has a bug**: it writes the weapon value into the defence slot wherever the two differ, so after a reload armour uses
the weapon row. Restart instead. The shipped `PSM_Client/Bin/Data` copy (99 % everywhere) is a private-server edit; the
`ShaiyaServerPatch` copy is the original's.

**Example:** the chance is in **millionths** (900000 = 90 %; compared with the roll `% 1000000`, [Lapis](lapis.md) section 5), the
add value in points. `WeaponStep00` is the chance of going from step 0 to step 1.

```ini
[LapisianEnchantPercentRate]
; 0 -> 1 succeeds 90 % of the time, 19 -> 20 0.02 %; nothing goes past 20
WeaponStep00=900000
WeaponStep01=800000
WeaponStep19=200
WeaponStep20=0
DefenseStep00=990000

[LapisianEnchantAddValue]
; a +1 weapon has 7 more attack, a +20 one 311
WeaponStep01=7
WeaponStep20=311
DefenseStep01=5
```

### 3.5 `SequenceItemCreate.ini`

Confirmed, `CItemDrop::InitSequenceItemCreate` 0x4be520 ([Monster drops](quest-drops.md) section 4).

Drops by count instead of by chance: every Nth monster the server makes drops an item, decided by the monster's id
(section 3.6).

| section, key | meaning |
|---|---|
| `[SEQUENCE_GROUP_COUNT] COUNT` | how many groups are read, at most 8 (0 or less: none) |
| `[SEQUENCE_GROUP_n] SEQUENCE` | N: the group fires for a monster whose id is a multiple of N. **0 in any group turns every group off** |
| `[SEQUENCE_GROUP_n] GRADE_0` … `GRADE_9` | the item grade for monster levels 1–9, 10–19, … 90–99 (0 = nothing) |

When a monster with money (`wMoney` > 0) dies, the groups are tried in order and **the first whose `SEQUENCE` divides the id decides**,
even when its grade is 0. Shipped: 7 groups, from every 30,000,001st monster to every 16,001st (a Weapon or Armor Lapisia); an eighth
section exists but `COUNT = 7` skips it. The drop is logged with make type `'V'` and game log record `0x0A06`.

**Example:** two of the shipped groups. The monster made as the 32,002nd (2 × 16,001), if it is of level 25, drops an item of grade
394; at level 75 it drops nothing (`GRADE_7=0`).

```ini
[SEQUENCE_GROUP_COUNT]
COUNT=2

; every 30,000,001st monster
[SEQUENCE_GROUP_1]
SEQUENCE=30000001
; levels 1-9, 10-19, 20-29 ...
GRADE_0=125
GRADE_1=125
GRADE_2=39
GRADE_3=39
GRADE_4=39
GRADE_5=39
GRADE_6=39
GRADE_7=0
GRADE_8=0
GRADE_9=0

; every 16,001st monster: a Weapon or Armor Lapisia for levels 1 to 69
[SEQUENCE_GROUP_2]
SEQUENCE=16001
GRADE_0=394
GRADE_1=394
GRADE_2=394
GRADE_3=394
GRADE_4=394
GRADE_5=394
GRADE_6=394
GRADE_7=0
GRADE_8=0
GRADE_9=0
```

The groups are tried in order, so put the rarest (the largest `SEQUENCE`) first: a monster whose id is a multiple of both counts takes
the first one only.

### 3.6 `MobID.ini`

Confirmed, `CObjectMgr::InitMobID` 0x40a340, `SaveMobID` 0x40a3c0.

`[INIT_MOBID_SET] MOB_ID = n`: the next monster id. Every monster made (spawns, respawns, summons, instances) takes the next number, and
the counter carries on across restarts: read at start (written as 1 when the file is missing), **saved every hour and at a normal
shutdown**. After a crash the ids since the last save are given out again, which repeats sequence drops. The shipped values are only where
each server's counter stood (EP4.5 1,613,972). Do not edit it while the server runs: the next save overwrites it.

**Example:** the whole file. Starting a new server from 1 is fine; copy the value across when moving a server to another machine.

```ini
[INIT_MOBID_SET]
MOB_ID=1613972
```

### 3.7 `GuildHouse.ini`

Confirmed, `CGuildMgr::InitGuildHouseIniFile` ([Guild houses](guild-house.md) section 2, the full table).

The guild house's prices and service levels, Windows functions. Sections `DefaultSet` (buy price, upkeep), `GateKeeper`, `ShopDefault`,
`ShopWeapon`, `ShopDefen`, `General`, `ShopPosion` (sic), `Bank`, `Smith`, `NpcLevel_Light`, `NpcLevel_Dark`. The keys are `KeepEtin`
and `UpgradeEtin` (the Etin per level), the `…DecreRate` price factors and `PermitSlot` (the bank's slots per level), as comma lists.
**A missing key keeps the built-in value**; a list is cut to its table's length. Gatekeeper level 3's rate is never read from the file.

**Example:** from the shipped file (the notes on `;` lines are translated from its Korean ones).

```ini
[DefaultSet]
; the price of a guild house
HouseBuyMoney=50000000
; its basic upkeep, in Etin
HouseKeepEtin=2000

[GateKeeper]
; upkeep per level 1, 2, 3
KeepEtin=200, 500, 800
; the Etin to reach level 1, 2, 3
UpgradeEtin=1000, 1500, 2000
; the price cut on teleports per level (0 = full price)
MoveDecreRate=0.0, 0.0, 0.0
```

### 3.8 `mob_ai.ini` and `Data/AI/*`

Confirmed, `CAIFactory::LoadMobAI` 0x4a8df0 ([Scripted monsters](scripted-mobs.md) section 2).

`mob_ai.ini` is one line per AI number, `n path`: `10 data/ai/sorp1`. **Lines are taken by their position, not by the number written**,
so keep them in order with no gaps; 0 to 9 are `ai/empty` placeholders (those numbers are built into the code: [Monster AI 0 to
9](mob-ai.md)). For each line from 10 the server reads four files, `<path>.alo` (the variables), `.afs` (the functions used), `.ai` (the
state machine) and `.ah` (the state handlers). **One file that fails to load stops the server's start.** Names are matched without case on
Windows. A monster definition's `AI` column (10 to 99) picks its line.

**Example:** the start of the file. Each line is the number, a space or tab, and the path without its extension; no comments.

```text
9 ai/empty
10 data/ai/sorp1
11 data/ai/croco1
```

A monster with `AI = 11` runs `data/ai/croco1.alo`, `.afs`, `.ai` and `.ah`. To add a script as number 37, add line 37 after line 36
and give the monster `AI = 37`.

### 3.9 `kr_string.DB`

Confirmed, `CAIStringDB::LoadStringDB` 0x402820.

The lines the AI scripts say (`Say`, `Say2`): pairs of lines, the id then the text (up to 127 bytes; the line end is kept). The server
sends the text as it is (`0x1109`), so it shows in the client's code page: the shipped Korean comes out unreadable in the US client.
Write it in the players' language, or have a Lua script use the client's own lines instead ([Monster speech](mob-speech.md)). EP5.4 reads
`cn_string.DB` instead.

**Example:** two lines. The number on one line, the text on the next; no comments, no blank lines between pairs.

```text
1
Smite the heretics!
2
Li laka Amon-Ra
```

### 3.10 `Lua/<n>.lua`

Confirmed, `CMob::LuaRegister` 0x4b1310 ([Party instance zones](instances.md) sections 9 to 16).

A monster whose `AI` is 100 or more runs `.\Data\Lua\<AI>.lua` in a Lua 5.1 state of its own (base, math and os libraries). The file
starts with `Mob = LuaMob(CMob)` and defines the hooks (`Init`, `OnAttacked`, `OnAttackable`, `OnNormalReset`, `OnDeath`,
`OnReturnHome`, `OnMoveEnd`, `WhileCombat`). A missing or failing file is logged and the monster has no script. The file is loaded when
the monster is created.

**Example:** a whole script, `Data/Lua/130.lua`, for a monster with `AI = 130`. It speaks a client line at half HP, once per fight,
and leaves a chest when it dies.

```lua
Mob = LuaMob(CMob)

bSaid = 0

function Init()
end
function OnAttacked( dwTime, dwCharID )
end
function OnAttackable( dwTime, dwCharID )
end
function OnNormalReset( dwTime )
  bSaid = 0
end
function OnReturnHome( dwTime, dwAttackedCount )
  bSaid = 0
end
function OnMoveEnd( dwTime )
end
function OnDeath( dwTime, dwAttackedCount )
  -- one monster 1250 (a treasure box) where it died
  Mob:LuaCreateMob( 1250, 1, 0.0, 0.0 )
end
function WhileCombat( dwTime, dwHPPercent, dwAttackedCount )
  if ( dwHPPercent <= 50 ) and ( bSaid == 0 ) then
    -- sysmsg 6403 "You can't win this fight." to the players within 200
    Mob:LuaSayByIndex( 6403, 200.0 )
    bSaid = 1
  end
end
```

Define every hook, even empty: a missing one is logged as an error each time it would run ([Party instance zones](instances.md)
section 14). The functions are listed in section 15 there. The AI number is one byte, so Lua scripts are numbers 100 to 255.

### 3.11 `Map/<id>.svmap`

Confirmed, `CMap::LoadMap` 0x425780 ([Maps and NPCs](maps-npcs.md) section 2; `tools/svmap.py` reads them).

The server's map, binary, little-endian: size, the blocked-cell bitmap, the monster spawn areas, the NPC placements, the portals and the
regions. It must agree with the client's map for positions to make sense. Edit it with a tool, not by hand.

### 3.12 `NpcQuest.SData`

Confirmed, `DBInitializeConstNpcQuest` 0x4caf80 ([Maps and NPCs](maps-npcs.md) section 3, [Quests](quest.md)).

Binary: the NPC definitions by type (merchants with their shops, gatekeepers with their destinations, the others), then the 1,720
quests. **The client has the same file**, so a change needs a client patch as well.

### 3.13 `KillStatus.SData`

Confirmed, `CGameData::LoadKCStatus` 0x4bf4a0 ([PvP 2](pvp-2.md) section 8).

Binary: `u32 n`, then per grade `u8 faction, i32 kills, u16 index, 6 × (u8 type, u16 value)`: the faction kill-count grades and their
bonuses. The client has the same bytes.

## 4. Files the servers write

| file | written by | what | notes |
|---|---|---|---|
| `.\Log\PS_<PROGRAM>__system.log` | every program | the log | name and mode by `[LOGFILE]` (section 2.2) |
| `.\Log\PS_DBAGENT__charsave.log` | `ps_dbAgent` | the character saves | |
| `Data/MobID.ini` | `ps_game`, `ps_gamewar` | the monster counter | section 3.6 |
| **`<Windows folder>\Shaiya.SData`** | `ps_game` | each character's money when it last left | confirmed, `CWorld::SaveOffCharMoney` 0x40db90 at `ServerShutdown`, `LoadOffCharMoney` 0x40d9a0 at `ServerStart` |

**`Shaiya.SData`** is easy to miss: it is not in the server's folder but in Windows' own (`GetWindowsDirectoryA`, so usually
`C:\Windows\Shaiya.SData`). It is `u32 count`, then that many pairs of `u32` (a character id and its money; the order inferred from the
reader). At logout the server records the character's money (`CUser::LeaveWorld`), and at the next entry it compares (`CUser::EnterWorld`;
game log record `0x030D` when the stored money is 10,000 or more: [The game log](game-log.md)). The file carries that table across a
restart; the server deletes it once read ("Delete Shaiya.SData = False" is logged when that fails). Unrelated to the client's
`.SData` files despite the name.

## 5. Changing the files

- **Restart after an edit**, except for the files with a reload command. The commands are typed in the game server's console window
  ([Operator console](operator-console.md) section 1):

  | command | reads again |
  |---|---|
  | `/reitemcre` | `ItemCreate.ini` |
  | `/reitemenh` | `ItemEnchant.ini` (with the armour bug of section 3.4) |
  | `/reseqitem` | `SequenceItemCreate.ini` |
  | `/remob <id>`, `/remobitem <id>`, `/reitem <id>`, `/reskill <id> <level>`, `/remobskill <id>` | one row of `PS_GameDefs` (monster, its drops, item, skill, monster skill), through `[DB] GameDef` |
  | `/aiset <mob> <ai>` | (no file) sets a monster definition's AI number live ([Monster AI 0 to 9](mob-ai.md) section 6) |

- **Encoding**: the data files are Korean code page 949. Keep that encoding, and keep numbers and keys in ASCII.
- **Comments**: in the files read with their own parsers (`Obelisk.ini`, `ItemCreate.ini`) only whole lines starting with `//` (or `--`
  in `ItemCreate.ini`) are comments. In the Windows profile files (`Config/*.ini`, `Map.ini`, `GuildHouse.ini`, `ItemEnchant.ini`,
  `SequenceItemCreate.ini`, `MobID.ini`) a value runs to the end of its line, so a note after a number is ignored by `atoi`, but **a note
  after a text value becomes part of it** (`MapType = F // field` is read as `F // field`, not `F`; it still counts as a field because
  only `D` matters).
- **Files the client also has** must change on both sides: `NpcQuest.SData`, `KillStatus.SData`, the map files, and anything the
  `PS_GameDefs` tables describe that the client has in its own SData (items, monsters, skills).
- **Keep a copy** of a working set: several shipped copies already differ (`ItemEnchant.ini`, two `ps_login.exe` builds), and the server
  gives few messages about a bad value.

## 6. The new server

This server reads none of these files while it runs. The data files are imported once into the `original_world` tables
(`tools/world/import.py`: `Map.ini`, the map files, `NpcQuest.SData`, `ItemCreate.ini`, `KillStatus.SData`, `Obelisk.ini`, `mob_ai.ini`
with the AI files, `kr_string.DB`, `Lua/*.lua`, `GuildHouse.ini`, `SequenceItemCreate.ini`, `ItemEnchant.ini`); the live `world` tables
are built from them, the installed extensions and the data editor's edits (README, "The game definitions"). The program configs are
replaced by one settings file and environment variables ([Settings](../server/settings.md)), and the monster counter of `MobID.ini` by
`game_servers.mob_id` in the database.

## 7. Other episodes

- **EP3** (2008): the same config sections except **`[MONITOR]`, `[NEW_SERVER]` and `[UNITED_WAR_SERVER]`**, which its `ps_game` does
  not have (confirmed, its strings); its `ps_login` is the 2007 build with `EnableServerComb` (section 2.5). Its `ps_game` reads `Map.ini`,
  `Obelisk.ini`, `ItemCreate.ini`, `SequenceItemCreate.ini`, `GuildHouse.ini`, `mob_ai.ini`, `MobID.ini`, the map files,
  `NpcQuest.SData`, `KillStatus.SData`, and its AI lines from **`tw_string.db`** in place of `kr_string.DB` (confirmed, its strings). No
  `Lua/` folder and no `ItemEnchant.ini`, `MapWar.ini` or `ObeliskWar.ini`; 152 AI files ([EP3](ep3.md)).
- **EP5.4** (2011): `Map.ini` loads 80 maps; `Obelisk.ini` adds the map-70 relics; `MobCreate.ini` is new; the AI lines come from
  `cn_string.DB`; and **`ServerCmd.ini`** keeps events started with the console's "keep" word ([EP5.4](ep5.4.md), [Operator
  console](operator-console.md)).
