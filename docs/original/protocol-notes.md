# Protocol notes

This page covers how the Shaiya client and the servers talk: the server processes, how a packet is framed, the login handshake and its
encryption, the character screen, and the data files the server loads. It is about Episode 4.5 (EP4.5): the server programs
`ps_login.exe` and `ps_game.exe` and the files shipped with them. Because it was first put together from public projects that target other
episodes, every claim says where it comes from.

## In short

- **Seven programs make up the original server.** The login server handles accounts and the server list, the game server runs the world,
  and the others keep track of sessions, save to the database and write logs. A player only ever connects to two of them: the login
  server (port 30800) and then the game server they pick (port 30810).
- **Every packet starts the same way.** Two bytes give the packet's length and two more give its *opcode*, a number that says what kind of
  packet it is (for example `0xA102` is the login request). The first byte of the opcode names a family: `0x07xx` is shops, `0x0Bxx` is
  parties, and so on.
- **The connection is encrypted from the first exchange.** The login server sends a public key, the client answers with a random secret,
  and both sides turn that secret into an AES key. The game server picks up the same key, and switches to a lighter XOR scheme for what it
  sends once the player is in the world.
- **The client checks its own version.** The server-select request carries a version number built into `game.exe`. A server can refuse a
  client whose version does not match.
- **Why it matters:** anyone writing a tool, a proxy or a server for this episode needs these layouts and the exact cipher, down to the
  byte order of the counter. Small differences, such as a name field of 19 bytes instead of 21, break the connection.

## How to read this document

Every claim carries a source tag and an episode, because the references target different episodes
and layouts changed between them.

| tag | meaning |
|---|---|
| **[I]** | Imgeneus docs/code, Episode 3 (GPL, facts only) |
| **[O]** | Origin and Genesis, early C#/C++ emulators, episode unstated, incomplete (MIT) |
| **[C]** | Shaiya-Core / shaiya-episode-6 headers, EP5, EP6, EP6.4 (BSD), structs mirror the real server executables |
| **[E]** | Eden, EP8 (no licence file; facts only) |
| **[P]** | Parsec docs, EP8 SData (MIT) |
| **[S]** | Observed directly in the original files' `Server/` (EP4.5 server zip from the OpenShaiya archive) |
| **[W]** | OpenShaiya docs, openshaiya.org/docs/packets (client builds `ps0198` US and `ps0032` Gamigo; the authors also cite an "Ep4 PDB"). The best-documented source so far |
| **[B]** | Read directly in the server binaries (`ps_login.exe`, `ps_game.exe`, parts of `ps_session.exe` and `ps_dbAgent.exe`), with symbols and struct layouts from the PDBs. Highest confidence; details in `docs/original/ps_login.md` and `docs/original/ps_game.md` |
| **[F]** | Forum post copied into the project's notes, describing the key exchange measured on an EP4 `game.exe` and `ps_login.exe`. A theory, not independently verified |

Confidence: **observed** (seen in a file in the project's reference set), **reported** (stated by a reference, one or more
sources), **unknown** (needs reverse engineering). Anything not marked observed is a lead, not a fact.

Reimplementation rule: this file records facts and layouts. The Rust server is written from these
notes, not from the reference code.

## 1. Server topology

Observed in `PSM_Client/Bin/Config/*.ini` [S]. The original server is seven cooperating processes
plus a management layer.

| process | role | talks to |
|---|---|---|
| `ps_login` | accounts, server list, session keys | ps_session, ps_userLog, DB `PS_UserData`; max 5000 connections |
| `ps_session` | tracks logged-in accounts across services; has unused billing sections | ps_userLog |
| `ps_game` | the game world (one per game server, e.g. `Game01`) | ps_session, ps_userLog, ps_dbAgent, ps_gameLog; DBs `PS_GameDefs`, `PS_GameData`; max 5000 connections |
| `ps_dbAgent` | persistence gateway for the game server | DB `PS_GameData` |
| `ps_gameLog` | game and chat logging | DBs `PS_GameLog`, `PS_ChatLog` |
| `ps_userLog` | account activity logging | DB `PS_UserData` |
| `ps_gamewar` | second game-type process (`Game254`) with the same config shape, apparently for the united-war zone (`[UNITED_WAR_SERVER]`) | session, userLog, game servers |

Other details:
- `ps_login.ini` maps a game server as `Game01 = name, ServerID, DB ID`.
- `ps_game.ini` has `GameVersion=0x00000000`. The login server compares this with the client
  version sent in `0xA202` (section 3), which is how the version gate works.
- Ports are blank in every shipped ini (`ServerPort=`), so they are compiled in or set by the management tool. Given by the project owner
  (from running the original software) **[U]**; the two marked [B] are also confirmed in `game.exe`:

  | service | port | note |
  |---|---|---|
  | login | 30800 | [B] `game.exe` connects here; the only port besides the game port meant to be public |
  | game | 30810 | [B] `game.exe` connects here after server select (`0x785A`) |
  | session | 30900 | |
  | userlog | 30901 | |
  | dbagent | 30911 | |
  | gamelog | 30912 | |
  | PSM agent | 40900 | management layer |
  | connectionManager / dbagent2 / gamelog | 1060 / 1056 / 1072 | [U] lists these; `ps_session.ini` [S] shows **1072 as the billing (GUM) port** and 10003 as a second billing port, but no build of `ps_session` reads `[BILLING_GUM]` (no such string in any image, confirmed), so the 1072 billing label belongs to an ini block nothing uses (corrected 2026-11-09, found when comparing the EP3, EP4.5 and EP5.4 builds: [`ep3.md`](ep3.md), [`ep5.4.md`](ep5.4.md)) |
- Database connection strings are stored as long hex blobs, so the configs are encrypted. **Unknown**
  algorithm. `PSM_Server/ps_passwordEnc.exe` is likely the tool that produces them.
- `[MONITOR]` ports 50000/50001 belong to the PSM management agent.

Target design: keep the logical split (login / game / persistence / logging) but not the process
count. See the architecture section of the project plan once it exists.

## 2. Transport framing

Reported by three independent sources, consistent: [I] [O] [E].

```
u16 length      total packet size INCLUDING these 2 bytes and the opcode
u16 opcode
u8  payload[length - 4]
```

- Little-endian throughout.
- Max packet is 2048 bytes: [C] defines `TP_MAIN { u16 opcode; u8 command[2044]; }`.
- Several packets may arrive in one TCP read. [E] loops over the buffer by `length`.
- After encryption starts (section 3) the opcode is inside the encrypted region [E], so it must be
  decrypted before dispatch. The length field itself stays in the clear [W].
- Opcode grouping: the high byte is the subsystem (0x01 character/session, 0x02 items, 0x03 other
  characters, 0x05 movement and combat, 0x06 mobs, 0x07 shops, 0x08 skills and cloaks, 0x09 quests,
  0x0A trade, 0x0B parties, 0x0D guilds, 0x0E NPCs, 0x11 chat, 0x22 friends, 0xF1 to 0xFA GM
  commands, 0xA1 to 0xA3 login and handshakes). Counts per family are in
  `docs/original/ep6-core-packet-catalogue.md`.
- The same numeric opcode can mean different things on different connections. Example: `0x010B` is
  "login terminate" on the login connection [O] and "quick slot / action bar" on the game
  connection [E][C]. Dispatch tables must be per service.
- `0x0003` is an empty ping/keepalive from the client [I][O].
- `0x010B` on the login connection is the client's "I am quitting" notice (observed from the real client, also in [O]).

## 3. Login connection

### 3.1 Handshake (`0xA101`)

Server sends first on connect.

Layout, now agreed by four sources [F] (EP4 capture) [W] [I] [O]:

```
u16 length = 0x00C7 (199)   u16 opcode = 0xA101
u8  encryptionMode = 0      (unused by the protocol, always 0)
u8  exponentLen    = 0x40   (64)
u8  modulusLen     = 0x80   (128)
u8  exponent[64]            little-endian, zero-padded
u8  modulus[128]            little-endian
```
The server generates a 1024-bit RSA key. [O]'s `00 40 80` header matches this, and its 192 random
filler bytes are just placeholders for the 64 + 128 key bytes.

Client reply (0xA101 again): `u8 length = 0x80` then 128 bytes of RSA ciphertext, packet length 0x0085
(133) [F]. The 129-byte payload seen in [I][O][E] is the same thing: 1 + 128.

### 3.2 Key derivation and cipher

Confirmed, in `ps_login.exe`, see `docs/original/ps_login.md`.

Tag **[B]** = read directly in the EP4.5-era login server binary (built 2009-06-10).

1. The client sends a random 128-byte value RSA-encrypted with the server's public key, as a **little-endian** integer, plain RSA with
   no padding. The server decrypts with CRT and keeps the result as 128 bytes little-endian, zero-padded. [B]
2. `digest = HMAC-SHA256(key = that 128-byte value, message = the 128-byte modulus)`. A key longer than 64 bytes is hashed first,
   as standard HMAC does, so the effective key is `SHA256(128 bytes)`. **Key = decrypted value, message = modulus**: [E], [W] and [B]
   agree; the forum post's `hmac(n, random)` order is wrong. [B]
3. `AES key = digest[0..16]`, `IV = digest[16..32]`. Standard AES-128 key schedule. [B]
4. **AES-128 counter mode with a continuous keystream.** One cipher object per direction, both seeded with the same key and IV, so each
   direction keeps its own counter. Each object holds the counter, a 16-byte keystream block and a position; leftover keystream bytes
   from one packet are used for the start of the next. There is no per-packet reset and no overlapping final block (the forum's
   description of an overlapping tail is wrong). [B]
5. **The counter is incremented little-endian**: byte 0 of the counter block is the least significant, carrying upward through the 16 bytes. This
   differs from NIST CTR, which is big-endian. [B]
6. The cipher covers everything after the 2-byte length field (`opcode + payload`). The length stays in clear. [B][W]
7. Before the exchange the connection accepts only `0xA101` (key reply); after it, `0xA102` (login) and `0xA202` (server select). Anything else
   disconnects. A key reply whose length byte is above `0x80` disconnects. [B]

Not yet tested: the AES block function's byte order against real data. A known-answer test against a real capture is needed (section 8).

The login server then passes the AES key and the first 16 bytes of SHA-256 of the login counter (offset 67), with the 16-byte identity
(offset 51), to the session server (message `0x701`) after a successful login, so the game server can rebuild the cipher state
(corrected 2026-11-09, found when comparing the EP3, EP4.5 and EP5.4 builds: [`ep3.md`](ep3.md), [`ep5.4.md`](ep5.4.md)). [B]

### 3.3 Login and server selection

| opcode | direction | layout | source |
|---|---|---|---|
| `0xA102` | client to server | `char username[32]`, `char password[16 or 19]`. The original server reads at most 12 characters of each (it forces a NUL at index 12 of the username and at index 12 of the password field) and never looks past byte 44, so both lengths work [B]. The new server reads the username to 12 and the password to 16 (all the client sends of it), so it wants at least 48 payload bytes | [C][I][W] + [B] |
| `0xA102` | server to client | `u8 status, u32 userId, u8 privilege, u8 identity[16]` (24 bytes with the opcode). On non-zero status only the first byte is needed [W]. The server list follows immediately on success | [B][C][I][W] agree |
| `0xA201` | server to client | `u8 count`, then per entry (38 bytes): `u8 id, u8 status (0 normal, 1 locked, 2 closed), u16 players, u16 capacity, char name[32]` (null-terminated). Sent in chunks of at most 48 entries; a server not connected is listed as status 2, players 0, capacity 0x7FFF | [B][W][I] |
| `0xA202` | client to server | `u8 serverId, u32 clientVersion` | [C][I] agree |
| `0xA202` | server to client | `u8 result, u8[4] worldIp` (7 bytes before the length prefix; the client formats the four bytes as `a.b.c.d` and connects to port 30810). Result `0xFE` (-2) cannot connect, `0xFD` (-3) version mismatch, `0xFC` (-4) server full; the IP is zero on errors and is filled in by the session server on success. [C] names the first byte `serverId`, [I]/[B] show it is the result | [B] |

Login status codes, `ps0198` client [W] (client message id in brackets):

| status | meaning |
|---|---|
| 0 | success: client reads userId, privilege, identity |
| 1 | account does not exist [10022] |
| 2 | account not enabled as a free challenger [10023] |
| 3 | wrong ID or password [10024] |
| 4 | account cannot log into the game [10025] |
| 5 | account cannot log into the game or homepage [10026] |
| 6, 7, 8 | account being terminated at the user's request [10027] |
| 9 | cannot connect, user request [10028] |
| 10 | account blocked [10122] |
| 11, 12 | account still restricted [10029] |
| 13, 14 | unused [10105, 10104] |
| 15 | subscription expired [808] |

Select-server result [I]: 0 success, -1 try later, -2 cannot connect, -3 version mismatch,
-4 server saturated.

The 16-byte `sessionKey` ("identity keys") is the credential the game server uses to recognise the
transfer. [E] confirms the login server shares it with the game server out of band (an API between
the two) and the game server checks the connecting IP against the login IP.

### 3.4 Server selection as the client does it

Confirmed, in `game.exe`, `FUN_004dca00`.

- Entries are 38 bytes. The client **will not select an entry whose status byte is non-zero**: it shows message `0x275F` locally and sends nothing.
- Request `0xA202`: `u8 serverId`, `u32 version`. The version is the literal **`0x10042600`** compiled into this `game.exe` (not read from
  `Version.ini`, not computed), built by `FUN_005868d0`.
- It then waits up to 30 s. A result byte `0xFC` shows message `0x2770` (server full), `0xFD` -> `0x2759` (version), `0xFE` -> `0x275A` (cannot
  connect), `0xFF` -> `0x275B`; a timeout shows `0x2758`. Any other value is a success and the next four bytes are the game server IP.
- On success it connects to that IP on port **30810**, sends the game handshake, and waits up to 30 s (`0x275D`/`0x275E` on failure).
- Server side (original, `ps_login.exe`): the version is compared only when a global check flag is on, and `0xFFFFFFFF` on either side means
  "any"; staff bypass the capacity check (`EnableUserCount`); each server has a connect-disabled flag.

## 4. Game connection

### 4.1 Entry

Confirmed, in `ps_game.exe`, see `docs/original/ps_game.md`.

| opcode | direction | layout | source |
|---|---|---|---|
| `0xA301` | client to server | `u32 userId, u8 identity[16]` (`TPC_GAME_CONNECT`, 22 bytes), **plain text** | [B] (agrees with [I]) |
| `0xA301` | server to client | `u8 success = 0, u8 encMode = 2, u8 encKey[16]` (`TP_GAME_CONNECT`, 20 bytes). `encKey` is the global XOR seed, the ASCII bytes `LENASOFTFOREVER!`. AES-encrypted. A failed handshake gets no reply, only a closed connection | [B] |
| `0xB103` | server to client | `u8 billingType, u32 remainTime`, right after the handshake reply | [B] |
| `0x0109` | server to client | `u8 country (0 and 1 factions, 2 none chosen), u8 maxGrow` (`TP_CHAR_COUNTRY`), before the character list when the database asks for it | [B] (agrees with [I][C]) |

Encryption on the game connection [B]: AES-128-CTR as on the login connection, key = the login AES key, both counters start at
`SHA-256(login IV)[0..16]` (ps_login hashes the IV before handing it over). Client to server stays AES for the whole connection. Server to
client switches from AES to an XOR table at the end of entering the world (decided by the server; there is **no `0xB106`** in this server:
in a zone it disconnects). XOR table: 128 SHA-256 blocks chained from the seed; byte `j` of a packet's `n = length - 2` encrypted bytes is
XORed with `table[n + j]`. The earlier [W] description (seed mixed with a hash of the AES key, `0xB106` trigger) does not match EP4.5.

### 4.2 Character screen

All little-endian. Note the **name length differs by episode**: 19 bytes in EP3 [I], 21 bytes in
EP5+ [C]. The EP3 server itself trims a 21-byte name at offset 10 of the create packet, like EP4.5 (EP3 ps_game 0x452896,
confirmed), so the 19 bytes are the EP3 client's figure, not what the EP3 server enforces (corrected 2026-11-09, found when comparing the EP3, EP4.5 and EP5.4 builds: [`ep3.md`](ep3.md), [`ep5.4.md`](ep5.4.md)). **EP4.5 [B]: requests carry 21-byte names; the list record carries 19 name bytes followed by the delete and rename flags.**
Exact EP4.5 layouts and checks are in `docs/original/ps_game.md` section 6, which supersedes the table below where they differ. There is
no name-check packet (`0x0119` disconnects).

| opcode | direction | layout | source |
|---|---|---|---|
| `0x0101` | server to client | one packet per slot; layout in 4.2.1 below | [W][I] |
| `0x0102` | client to server | EP3: `u8 slot, race, mode, hair, face, height, class, gender, char name[19]`. EP5+: `u8 slot, family, grow, hair, face, size, job, sex, char name[21]` (same fields, renamed; the EP3 "mode" is `grow`) | [I][C] |
| `0x0102` | server to client | `bool error` then the character list | [I] |
| `0x0103` | client to server | `u32 charId` (delete). EP3 reply `bool error, u32 charId` | request [I][C], reply [I] |
| `0x0104` | client to server | `u32 charId` (enter world). EP3 reply `bool error, u32 charId`; EP5+ server-side struct is just `u32 objectId` | request [I][C], reply differs: [I] vs [C] |
| `0x0107` | client to server | empty (leave world / close character connection). Server notifies others with `u32 objectId` | [I][C] |
| `0x010E` | both | rename; `u32 charId, char name[21]`; reply `u8 result (1 success, 2 fail)` | [C] (EP5+) |
| `0x010F` | client to server | restore deleted character: `u32 charId` | [I][C] |
| `0x0119` | both | name availability: request `char name[19]`, reply `bool available` | [C] |
| `0x0114` | server to client | server time `u32` | [C] |

#### 4.2.1 Character list record (`ps0198` US client, 81 bytes when the slot is used) [W]

`u8 slot, u32 charId` (0 = empty slot, and the rest is omitted), then: `u32 creationTime` (unused by the
client), `u16 level, u8 race, u8 mode, u8 hair, u8 face, u8 height, u8 class, u8 gender, u16 map,`
`u16 str, dex, rec, int, wis, luc, u16 hp, mp, sp, u8 itemType[8], u8 itemTypeId[8], char name[19],`
`u8 pendingDelete, u8 pendingRename, u8 cloakEmblems[6]`.
- Race: 0 human, 1 elf, 2 vail, 3 nordein [W]. [I] says 2 deatheater, 3 vail: conflict, check against the client.
- Class 0 to 5: fighter, defender, ranger, archer, mage, priest. Mode: 0 easy, 1 normal, 2 hard, 3 ultimate (the wording of mode 2 differs between sources).
- Name field is 19 bytes here, so the 19 vs 21 change in the create packet [C] happened after this layout.

#### 4.2.2 Character details (`0x0105`, sent after a successful select) [W]

Two variants documented: Gamigo `ps0032` (99 bytes) and US `ps0198` (87 bytes). The Gamigo one starts with six `u16`
base stats (str, dex, rec, int, wis, luc); the US one omits them and is otherwise identical:
`u16 statPoints, u16 skillPoints, u32 maxHp, u32 maxMp, u32 maxSp, u16 direction` (radians; degrees = `-direction * 180/pi`),
`u32 prevExp, u32 nextExp, u32 exp, u32 gold, f32 x, f32 y, f32 z, u32 kills, u32 deaths, u32 victories, u32 defeats, char guildName[25]`.
This is the "start loading the world" packet. The EP4.5 variant is unknown; the Gamigo `ps0032` build is the older of the two.

Race / faction numbering [I]: 0 human, 1 elf, 2 deatheater, 3 vail; modes 0 easy, 1 normal, 2 hard,
3 ultimate. Classes [C]: fighter, defender, ranger, archer, mage, priest (0 to 5).

### 4.3 World packets

Not written up by hand. `docs/original/ep6-core-packet-catalogue.md` lists 302 structs with opcode, direction,
original server name (`TP_*` / `TPC_*`), packed size and fields, extracted by
`tools/extract_core_packets.py` from the Shaiya-Core headers. Treat these as **EP5 to EP6.4** layouts:
- The headers carry `_EP5`, `_EP6` and `_EP6_4` variants of the same struct (item remake and compose,
  PvP exchange, quest end, shape, my-shop and lapisian packets). EP5 is the closest to EP4.5.
- Fourteen structs have no computable size (variable-length lists, templates), listed with `?`.
- The names are the real server symbol names, which makes them useful as cross-references when
  reading `ps_game.exe` in Ghidra.

## 5. Data files

### 5.1 SData

[P] documents the **EP8** format: a 128-byte header, `u32 fieldCount`, length-prefixed field names,
`u32 recordCount`, then records where every value is an `int64`; strings live in separate text
tables, and some files are wrapped in a KISA SEED header.

**The EP4.5 files we hold do not follow that layout** [S]. `KillStatus.SData` (1504 bytes) begins
`3c 00 00 00 00 96 00 00 00 01 00 0b 05 00 0c 05 ...` and has no 128-byte header; reading it with
the EP8 parser yields garbage. `NpcQuest.SData` (2,637,480 bytes) contains readable Korean text in
CP949 stored as `u32 length` followed by the bytes, mixed with small fixed-width fields. The files
appear to be unencrypted, little-endian binary. Record structure is **unknown** until the loader is
read in `ps_game.exe`. The server also loads a text-based set: `Map.ini`, `MapWar.ini`,
`MobID.ini`, `mob_ai.ini`, `ItemCreate.ini`, `ItemEnchant.ini`, `SequenceItemCreate.ini`,
`Obelisk.ini`, `ObeliskWar.ini`, `GuildHouse.ini`, `kr_string.DB`, `Map/` (77 map files, "SVMap"),
`Lua/` and `AI/`.

### 5.2 Mob AI

`Data/AI/` holds one set per boss: `.ai` (script), `.ac`, `.afs`, `.ah`, `.alo` and an
`*_AI_FUNCTION.H`, plus a shared `SysFunc.FDB`. Some also have `.PAI`. `*_AI_FUNCTION.H` files are
readable and likely document the script function interface. Not analysed yet.

### 5.3 Client data

The client keeps tables in its own archives. The OpenShaiya archive index lists client SData files
(`Item.SData`, `Skill.SData`, `Monster.SData`, `NpcQuest.SData`, ...) with size and checksum per
patch. The server zip contains only `NpcQuest.SData` and `KillStatus.SData`; the other tables come from
the database (`PS_GameDefs`) on the server side.

## 6. Database

Databases referenced by the configs [S]: `PS_UserData`, `PS_GameDefs`, `PS_GameData`, `PS_GameLog`,
`PS_ChatLog`. Update: `PSM_Data/` holds SQL Server backups of all ten databases (see
`docs/original/server-inventory.md`), so the schema is recoverable by restoring them; it is not yet read.
Imgeneus uses MySQL,
Origin/Genesis use their own tables, Eden uses PostgreSQL; none of these are the original schema.
The SQL layer for the rewrite will be designed from what the game needs, with a migration path
from the original MSSQL tables kept as a later goal.

## 7. Client version

The client in the original files' `Client/` reports `CurrentVersion=108` in `Version.ini` (US distribution;
`game.exe` dated 2010-04-26). Whether it matches this server is still **unresolved**: the server's
`NpcQuest.SData` is far smaller than the ~3.1 MB client tables near patch 108. Index lookup (OpenShaiya archive
sqlite) found no client `NpcQuest.SData` of exactly 2,637,480 bytes in any distribution. The nearest
US sizes are patch 13 (2008-04-30, 2,620,886) and patch 15 (2008-05-22, 2,648,476). The server's
own `GameVersion` and the game.exe version resource should settle this.

## 8. Next steps (in priority order)

Done: inventory, database restore, `ps_login.exe` (crypto, handshake, login, server list, server select).

1. Known-answer test for the cipher: implement the handshake and AES-CTR in Rust per section 3.2, and check it against a real
   client or the original server running in a sandbox (a Windows VM, not this host).
2. Read `ps_session.exe`: messages `0x701` and `0x703`, how the server-select reply gets the game server IP, the key hand-off.
3. Read `ps_game.exe` and `ps_dbAgent.exe` with their symbols. Done: game handshake (`0xA301`), the XOR layer, the character screen
   (`docs/original/ps_game.md`). Remaining: enter world, SData loaders, packet table, DB schema usage. Every `TP_*` struct is in
   `docs/original/symbols/ps_game.packets.txt`.
4. Identify the matching EP4.5 client (`game.exe`, `data.saf` format) and cross-check layouts marked EP3/EP5+.
5. Replace the `?` rows and EP5/EP6 assumptions in the packet catalogue with verified EP4.5 layouts.
6. Decide the Rust crate layout once framing and crypto are confirmed.

## Sources

Third-party projects consulted: `momo314/Imgeneus`, `7Power/Origin`,
`Drakkus/ShaiyaGenesis`, `Spelunkern/Shaiya-Core`, `kurtekat/shaiya-episode-6`,
`tristonplummer/Eden`, `matigramirez/Parsec`, `matigramirez/Etaine`. Archive index from
`archive.openshaiya.org`.

## 9. Other episodes

The EP3 and EP5.4 builds of the servers were compared with EP4.5, read statically like the rest of this note: `ps_login` (EP3
2007-12-07; EP5.4 is that EP3 build with 7 bytes changed), `ps_session` (EP3 2007-12-07, EP5.4 2011-09-14), `ps_game` (EP3 2008-01-17,
EP5.4 2011-09-14, modified after linking), `ps_dbAgent` (EP3 2008-01-04, EP5.4 2011-09-14, modified after linking) and `ps_gameLog`
(EP3 2007-12-07, EP5.4 2010-10-18). Overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **Login connection:** the same handshake, key exchange, AES-128-CTR and server list as EP4.5 (confirmed, side-by-side diffs). The
  `0xA102` login keeps up to 31 characters of user name and 32 of password (EP3 ps_login 0x4044ea, 0x4044ee), sent to the database in
  plain text (confirmed; details in [`ps_login.md`](ps_login.md)).
- **Login → session `0x701`** is 0x63 bytes, without EP4.5's last u32 (EP3 ps_login 0x4046bc), and the session's `0x801` reply to the
  game server is 90 bytes (EP3 ps_session 0x403158), without the auto-reconnect character id and the server id (confirmed).
- **Game connection:** the same connection states, the same opcode groups at the same switch positions, minus the market group `0x1C`
  (EP3 ps_game `PacketProcessing` 0x44e330, matched by its `0xA301` compare and state tree; confirmed). EP4.5 added 42 client opcodes:
  `0x0307`, `0x0559`, `0x0805`–`0x0808`, `0x0B0C`–`0x0B18`, `0x0B1A`, `0x1112`, `0x1C01`–`0x1C0D`, `0x1C14`–`0x1C17`, `0xF10A` and
  `0xFA18`–`0xFA1A` (confirmed, dispatch walk). EP3 has nothing EP4.5 lacks except `0xB106` in state 3, which it answers with the item-mall
  product list (EP3 ps_game 0x441950; confirmed).
- **Server-built packets only in EP4.5** (confirmed: the opcode constants are absent from the EP3 image): `0x0110`–`0x0114` (server move,
  reconnect, server time), `0x0228`, `0x0524`, `0x0525` (absorption), `0x0558` (scouting), the party updates `0x0C0A`–`0x0C14`, `0x110A`/
  `0x110B` (scripted monsters) and `0xB103` (billing info).
- No packet length checks, as in EP4.5 (confirmed).
- The create packet's name is a 21-byte field at offset 10, as in EP4.5 (EP3 ps_game 0x452896, `TrimString(…, 21)`, confirmed).

### EP5.4

- **Login:** the EP3 code with three patches: the `0xA101` key reply's length byte must be exactly 0x80 (EP5.4 ps_login 0x404542), the
  connection cap is removed (0x411abd) and the login query quotes the name and password with `"` (0x445e70) (confirmed).
- **Session:** reads `0x701` only up to offset 0x62 (EP5.4 ps_session 0x404168), so it takes the 0x63-byte form; its `0x801` reply is
  95 bytes, exactly the PDB struct (EP5.4 ps_session 0x403657) (confirmed). New session ↔ game messages (confirmed, dispatch): `0x105`
  (cash billing up or down), `0x108` (16 random bytes per session run, meaning unknown), `0x818` and `0x81B` (relays), and `0x1B01`–`0x1B03`
  (item-mall points through a PayLetter billing link).
- **Game:** the same dispatch; every EP4.5 opcode is still handled (confirmed, all 65536 values walked in both builds, EP5.4 ps_game
  `PacketProcessing` 0x474940, matched by its unique string). New:

  | opcode | dir | layout (lengths with the opcode) | where (EP5.4 ps_game) | status |
  |---|---|---|---|---|
  | `0x0308`, `0x0309` | C→S | 6: `u32 targetId` @2 | `PacketGetInfo` 0x477e19, 0x477eff | confirmed |
  | `0x030A` | C→S | 2 | 0x477fab | confirmed |
  | `0x030B`, `0x030C`, `0x030D` | S→C | target's buff list, one buff added (10: `u8 kind` @2, `u32 objectId` @3, `u16` @7, `u8` @9), one removed | 0x456bef, 0x456d31, 0x456e21 | confirmed |
  | `0x0809`, `0x080A` | C→S | 8: six `u8` @2..@7 | `PacketGem` | confirmed |
  | `0x0B1B` | both | 7: `u32 id` @2, `u8 partyType` @6 | `PacketParty` 0x475fa4 | confirmed |
  | `0xF603` | both | `u8 len` (≤ 0x7F) @2, text @3 | `PacketAdminCmdA` 0x480b34 | confirmed |
  | `0xF70F` | C→S | 5: `u16` @2, `u8` @4 | 0x481955 | confirmed |
  | `0xFA1B` | C→S | 32: guild name @2, `u32` @0x1B, `u8` @0x1F | 0x484099 | confirmed |
  | `0xB106` | S→C | `u8 n` @2, then n rows of 76 bytes (`code[21]`, `name[51]`, `u32 price`), at most 25 per page | 0x4dc090, in state 3 | confirmed |

  Any `0xB1xx` other than `0xB106` in state 3 closes the connection (EP5.4 ps_game 0x474b75). `0xA302` (GameGuard) is no longer routed
  (EP5.4 ps_game 0x4748e0). A connection whose receive queue holds more than 100 packets is closed and logged (EP5.4 ps_game 0x474864;
  EP4.5: 1000, no log). There are still no length checks (confirmed).
- **Game log:** the record header is 87 bytes, the user name 32 bytes instead of 19 (EP5.4 ps_game 0x467f60, EP5.4 ps_gameLog 0x401830;
  confirmed).
