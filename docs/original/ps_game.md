# Game server (ps_game): handshake, encryption and character screen

This page covers the first part of a game connection in the EP4.5 game server, `ps_game.exe`: the handshake that hands a player over from
the login server, the AES and XOR encryption, the character list, and creating, deleting, selecting and renaming characters. Where the
handshake crosses into `ps_session.exe`, `ps_dbAgent.exe` and `ps_login.exe`, those were read too.

The binary is `ShaiyaServerPatch/Server/GameMain/PS_Game/ps_game.exe` (built 2009-07-15, SHA-256
`0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547`), with its exactly matching `ps_game.pdb`. It was studied by static
analysis; nothing was executed.

## In short

- **The handover.** After picking a server, the client connects to the game server and sends its user id and a 16-byte key it got from
  the login server. The game server asks the session server whether that pair is valid. If it is, the player gets the character screen;
  if not, the connection is simply closed with no message.
- **Encryption.** What the client sends stays AES-encrypted for the whole connection. What the server sends is AES until the player is in
  the world, then switches to a simple XOR table. The XOR key is the same text for every player (`LENASOFTFOREVER!`) and is sent to each
  client, so it hides nothing from anyone who can log in.
- **The character screen.** Up to 5 characters per account and server. The faction (Light or Fury) is stored per account and
  server, and a new character must be of a race that faction offers. A new character's stats come from its class and race, and its starting place is picked from a list in `Data/Map.ini`.
- **Deleting is a countdown.** A deleted character is only marked; it is removed for good when the list is next loaded more than 2 hours
  later, and it can be restored until then.
- **Weak spots in the original:** packet lengths are not checked, the client's address is never compared with the login address, and the
  server trusts the hair, face, size and sex values the client sends.

## How it was read

- Function names: `tools/pdb_symbols.py` writes them to `docs/original/symbols/ps_game.tsv` (7,493 procedures), and the same for `ps_session.tsv` and
  `ps_dbAgent.tsv`. The names used below were applied in a Ghidra session (145 functions and globals; that database is not kept, the TSV
  files are the record).
- Packet and record layouts: `tools/pdb_types.py` reads the PDB type stream. **The structs carry the original names and exact offsets**
  (`TP_*` = packet, `TPC_*` = the other direction or a server-to-server request). All of them are in `docs/original/symbols/ps_game.packets.txt`.
  Every size that the code also states (`push 0x14`, `push 0x53`, a 0x56-byte loop stride, ...) agrees with the PDB.
- Instructions: the Ghidra decompiler, and `tools/disasm.py` (objdump plus PDB names) for functions Ghidra did not create.

Status words: **confirmed** = read in the code (address given). **PDB** = layout from the PDB type records. **inferred** = follows from
confirmed code but is not shown directly. **unknown** = not read yet. Addresses are virtual addresses (image base `0x400000`). Lengths
include the 2-byte opcode but not the 2-byte length prefix, as in the original code.

## 1. Receive path and connection states

Confirmed.

`CUser::OnRecv` (0x466d20), for every packet:
1. If `CUserCrypto::m_bInitDec` is set, AES-CTR decrypts everything after the length field (section 4).
2. `SAgent::PacketSystem` (0x4d89f0) consumes system opcodes in every state: `0x0002` closes the connection (reason 1); `0x0003` is the
   keepalive, checked by `SReply::CheckRecv` (0x4dd6c0, see section 9).
3. `0xA302` goes to `CUser::NProtectCSARecv` (GameGuard CS authentication reply).
4. Everything else goes to `CUser::PacketProcessing` (0x466da0), which dispatches on `m_nWhere` (`CUser+0x5758`, the connection state):

| `m_nWhere` | meaning | accepted |
|---|---|---|
| 0 | connected, not authenticated | only `0xA301` (section 2). Anything else is logged (`WHERE_CONNECT Error Packet=%x`) and counted in `m_dwUndefinePacket`; the 11th such packet closes the connection (reason 0x65). Nothing is sent back |
| 1 | character screen | `0x01xx` only, to `CUser::PacketCharacter`. Other opcodes are ignored silently |
| 2 | waiting / moving between zones | nothing (ignored) |
| 3 | set in `CWorld::UpdatePackets` (not read further) | `0x0201`, `0x11xx` and `0xF1xx` (chat, when allowed), any `0x31xx`; the rest ignored. This is the state of a player at a gambling table (`CMiniGame`, `docs/original/mini-game.md`); the new server keeps it as `WorldState.mini_game` with `entered` false |
| 4 | in a zone | the full game dispatch; an unknown group closes the connection (reason 9) |

## 2. Handshake request `0xA301`, client to server

Confirmed, PDB.

`TPC_GAME_CONNECT`, 22 bytes:

| offset | field | type |
|---|---|---|
| 0 | `wType` = 0xA301 | u16 |
| 2 | `dwUserID` | u32, the user id from the login reply `0xA102` |
| 6 | `bySessionKey` | u8[16], the "identity" from the login reply |

- **It arrives in plain text.** `m_bInitDec` is cleared by `CUser::Reset` and only set by `CUserCrypto::InitAesKey`, which runs after the
  request has been accepted (section 3), so `OnRecv` does not decrypt it. Confirmed in `game.exe`: the client clears its cipher flags, sends
  `0xA301` unencrypted and only then turns AES on (`docs/original/enter-world.md` section 1.2).
- `PacketProcessing` accepts it when: the opcode is `0xA301`, `dwUserID != 0`, `CWorld::FindUserBill(dwUserID)` finds no user with that id
  already in this world, and this connection has not sent one before (`m_dwBillingID == 0`). It stores the user id in `m_dwBillingID` and
  sends `TPC_GUSER_CONNECT` (`0x801`, 30 bytes) to the session server: `dwUID` (this connection's index), `dwIP` (the client address),
  `dwUserID`, `bySessionKey`. Otherwise it is counted as an undefined packet (section 1).
- The packet length is not compared with 22 anywhere; a short packet reads stale buffer bytes.

## 3. The session server's check and the game server's completion

### 3.1 ps_session: `CConnection::PacketGame`, case `0x801`

Confirmed, 0x4030ac.

1. Look the user up by `dwUserID` (`CUserMgr::FindUser`). Not found: reply failure.
2. `stUserInfo::EnableConnectGame` (0x402fa0) must return true:
   - the 16-byte session key equals the identity issued at login (`repz cmpsd` over 16 bytes);
   - this game server's id equals the server chosen at server select, unless a reconnect flag is set;
   - this game server is not the one the user is already marked as playing on.
3. **`dwIP` is never read.** The original does not compare the game connection's address with the login address.
4. Success: the user is marked as in game, the server's player count is incremented, and the reply `0x801` is sent with `bySuccess = 0`.
   Failure: the same reply with `bySuccess = 1`.

### 3.2 The reply `TP_GUSER_CONNECT` (`0x801`, PDB 95 bytes; ps_session sends 99)

| offset | field | game server use (`CWorld::LoginUser` argument) |
|---|---|---|
| 2 | `bySuccess` | 0 = success |
| 3 | `dwUID` | which connection this answers |
| 7 | `dwUserUID` | must equal the id from `0xA301` |
| 11 | `byServerDBID` | `m_nServerDBID`, sent on to the database agent |
| 12 | `byUserPermission` | `m_byUserPermission` (the login "privilege": 11 player, 1 to 5 staff) |
| 13 | `qwSessionID` | `m_qwSessionID` (log correlation) |
| 21 | `szUserName[32]` | `m_szUserName`, sent on to the database agent |
| 53 | `byAesKey[32]` | `m_byAesKey`; `[0..16]` = AES key, `[16..32]` = initial counter (section 4) |
| 85 | `byBillingType` | stored, sent to the client in `0xB103` |
| 86 | `dwBillingRemainTime` | ps_session always sends 0 |
| 90 | `dwAutoCharID` | non-zero only for the reconnect feature: skip the character screen and enter with this character |
| 94 | `byServerID` | `m_byRealServerID` |
| 95 | u32 (not in the PDB struct) | stored in `m_nMovePositionType`; ps_session fills it from its user record. Purpose unknown |

### 3.3 Completion: `CWorld::LoginUser`

Confirmed, 0x40c510.

Finds the connection by `dwUID`. If there is none it logs and, on success, tells the session server `0x802` (user gone). Then:
- `dwUserUID` differs from the connection's `m_dwBillingID`: return silently (the client hangs until its 30 s timeout);
- `m_nWhere != 0`, `bySuccess != 0`, or the user id is already in this world: **close the connection (reason 9) without any reply.**

On success, in this order:
1. Copy the 32-byte key to `m_byAesKey` and call `CUserCrypto::InitAesKey` (section 4). From here the connection is encrypted both ways.
2. Store permission, session id, server DB id, user name, billing values, `dwAutoCharID`, `byServerID`.
3. `m_nWhere = 1` (character screen), add the user to the world (`CWorld::EnterUserWorldBill`).
4. `CUser::SendDBAgentConnect`: `0x301` `TP_DB_USER_CONNECT` (user id, server DB id, user name) to the database agent, or `0x306` with the
   character id when `dwAutoCharID != 0`. The character list comes back from there (section 6).
5. Send the handshake reply `0xA301` (section 3.4).
6. Send `0xB103` billing info (section 3.5).

### 3.4 Handshake reply `0xA301`, server to client

Confirmed, 0x40c73c, PDB.

`TP_GAME_CONNECT`, 20 bytes:

| offset | field | value |
|---|---|---|
| 0 | `wType` | 0xA301 |
| 2 | `bySuccess` | always 0. A failed handshake never gets a reply, only a closed connection |
| 3 | `byEncMode` | always 2 (a constant in the code; `CryptoGetMode` also just returns 2) |
| 4 | `byEncKey[16]` | the 16-byte XOR seed `g_xorkey` (section 5), the same for every player |

It is the first packet on the new AES stream: `InitAesKey` has run and the XOR flag is still off, so it is AES-encrypted (section 4.3).

### 3.5 Billing info `0xB103`, server to client

Confirmed, `CUser::SendBillingInfo` 0x457ad0.

7 bytes: `u16 0xB103, u8 billingType, u32 remainTime`. Sent right after the handshake reply. What the client does with the values is unknown;
the session server sends remain time 0, and the type comes from its user record.

## 4. AES on the game connection

Confirmed.

### 4.1 Where the key comes from

`ps_login`'s `CUserCrypto::GetKeyGen` (0x404d50 in ps_login.exe), run at login, produces the 32 bytes passed via the session server:
- `[0..16]` = the AES key negotiated on the login connection (`digest[0..16]`);
- `[16..32]` = **the first 16 bytes of `SHA-256(login IV)`**, not the login IV itself (it hashes `digest[16..32]`, 16 bytes, with the
  same SHA-256 code; then copies 16 of the 32 digest bytes);
- the identity is a separate fresh random 16-byte value (`SRandBlock16`).

ps_login puts these 32 bytes at offset 0x43 of its `0x701` message to ps_session (verified, 0x40484c), and ps_session copies its stored
32 bytes into `byAesKey` of the `0x801` reply (verified, 0x403160). That ps_session stores the `0x701` bytes unchanged in between is inferred
(the copy in `CUserMgr::AddUser` was not traced).

### 4.2 Setting it up

`CUserCrypto::InitAesKey` (0x457fc0) runs `AES_set_encrypt_key(key, 128 bits)` for both `m_AesDec` and `m_AesEnc`, sets both counters to
`byAesKey[16..32]` and both stream positions to 0, then sets `m_bInitDec = m_bInitEnc = 1`. So the game connection starts **fresh** streams
(it does not continue the login connection's counters), one per direction, from the same counter value.

The cipher code is the same library as in ps_login (`SAes::EncCTR` is 441 bytes in both; `SAes::CtrInc` is the same little-endian 128-bit
increment): continuous AES-128-CTR as described in `docs/original/ps_login.md` section 4.

### 4.3 Which packets use it

- Client to server: `CUserCrypto::Dec` always uses AES once `m_bInitDec` is set. **There is no XOR in this direction.**
- Server to client: `CUserCrypto::Enc` (0x458080) uses the XOR layer when `m_bInitGame` is set, otherwise AES when `m_bInitEnc` is set.
  The two are never combined. While XOR is in use the server's AES send counter does not move.
- Encryption runs when a packet is queued (`SConnection::Send` → `SWorker::SendPost` 0x4dc1b0 → `CUser::SendPacketCustomProc`), so the
  switch between AES and XOR follows the order of `Send` calls exactly. Packets over 2048 bytes close the connection instead of being sent.

## 5. The XOR layer

Confirmed.

### 5.1 Key and table

- `ServerStart` calls `CryptoInitKey2` (0x457f00) once at start-up. It sets the 16-byte seed `g_xorkey` to the ASCII bytes
  **`LENASOFTFOREVER!`** (`4c454e41534f4654464f524556455221`), a constant in the binary. `CryptoInitKey` (0x457eb0), which would use a random
  seed, has no callers.
- From the seed it builds `g_xorbuf`, 4096 bytes = 128 blocks of 32:
  `block[0] = SHA-256(seed)`, `block[i] = SHA-256(last 16 bytes of block[i-1])` for i = 1..127.
  The SHA-256 code is byte-for-byte the same as in ps_login (compared with addresses masked), where standard SHA-256 is proven by the
  real client logging in.
- Known-answer values (computed with Python's `hashlib` from the construction above):
  - `g_xorbuf[0..32]` = `6b8ad46b8607f6876c5678e55c89f157494108e63d349209f96ebbfbbba1f179`
  - `g_xorbuf[4064..4096]` = `6b5b0575b36ff8f4bd94d34334e10a0ef8d6a6888c9546fe62b8443a4be79e2e`
  - `SHA-256(g_xorbuf)` = `b3fb3fc7f845c90e85087b7dc9b7aede477a0f7fffec0104518f7f0bbec7dcb7`
- The seed is sent to every client in the `0xA301` reply (`byEncKey`), so the client can build the same table. It is one global key: it hides
  nothing from anyone who logs in, and in-world server-to-client traffic is effectively readable by any client.

### 5.2 Applying it (0x458080)

For a packet of total length `len` (the u16 length field), `n = len - 2` bytes after the length field (opcode and payload) are XORed:
`byte[2 + j] ^= g_xorbuf[n + j]` for `j` in `0..n`. The start position in the table depends on the packet length; there is no running state.
`n` is at most 2046 here, so `n + j` stays below 4092. A packet buffer shared by several recipients is XORed only once (reference count at
buffer `+0xC`), which is why one global table is used.

### 5.3 When it is on

- `m_bInitGame = 1` is set at the very end of entering the world (`CUser::PacketUserDBChar`, case `0x407`, at 0x46d6f5), after the whole
  enter-world burst has been queued, immediately before `SendNow`. Every packet queued after that is XORed. The last packet of the burst is
  `0x010B` (quick slots), and the client's `0x010B` handler is what turns XOR on in `game.exe` (verified on both sides,
  `docs/original/enter-world.md` section 1.2).
- `m_bInitGame = 0` (and `m_bInitEnc = 1`) when the player goes back to the character screen (`CWorld::LogoutUser`, 0x40c942). AES then
  resumes with the counter where it stopped.
- `CUser::LeaveCharSelect` / `EnterCharSelect` (0x466c80 / 0x466c70) do the same two writes but have no callers; the logic is inlined.

### 5.4 Corrections to earlier notes

- **There is no `0xB106` in this server.** No code compares with `0xB106`. In a zone, `0xB1xx` goes to `CUser::PacketBilling`, which
  accepts only `0xB102` and closes the connection (reason 9) for anything else; in state 3 `0xB1xx` is ignored. The XOR switch is decided by
  the server alone (section 5.3), not signalled by the client.
- The key is **not** combined with a hash of the AES key, and it is not per player. The "127 rounds, each hashing the last 16 bytes" part of the
  OpenShaiya description is right.
- `encryptionMode` is the constant 2.

## 6. Character screen

### 6.1 What the client receives after the handshake

Confirmed, `CUser::PacketUserDBChar` 0x46cf60.

In ps_dbAgent, `CUser::LoadCharacterListWithCountry` (0x404220) loads the characters and, when its flag is set, also sends `0x401`
(`TP_DB_CHAR_COUNTRY_LOAD`: the account's faction and max grow on this server, which the game server stores in `m_byCountry` / `m_byMaxGrow`),
then always `0x403` (`TP_DB_CHAR_LIST`: up to 5 characters, with `byWithCountry` = the same flag). Both callers of `LoadCharacterListWithCountry`
in ps_dbAgent set the flag (verified, `CUser::OnDispatchDB` 0x403e4d and 0x403ffb; `docs/original/world.md` section 4.2). The game server turns `0x403` into, in order:

1. If `byWithCountry != 0`: **`0x0109`** `TP_CHAR_COUNTRY`, 4 bytes: `u8 byCountry, u8 byMaxGrow`.
2. One **`0x0101`** per **empty** slot, slots 0 to 4 in order: 7 bytes, `u8 bySlot, u32 dwID = 0`.
3. One **`0x0101`** per character, in the database agent's order: 83 bytes (`TP_CHAR_LIST`):

| offset | field (PDB name) | notes |
|---|---|---|
| 2 | `bySlot` | 0 to 4 |
| 3 | `dwID` | character id |
| 7 | `dwCreateTime` | |
| 11 | `wLevel` | |
| 13 | `family` | race: 0 to 3 |
| 14 | `grow` | mode: 0 to 3 |
| 15 | `byHair`, `byFace`, `bySize`, `byJob`, `bySex` | one byte each (15 to 19); job = class 0 to 5 |
| 20 | `wMap` | the map shown; see below |
| 22 | `wStr, wDex, wRec, wInt, wWis, wLuc` | u16 each (22 to 33) |
| 34 | `wHP, wMP, wSP` | u16 each (34 to 39) |
| 40 | `byEquipmentType[8]` | |
| 48 | `byEquipmentTypeID[8]` | |
| 56 | `szName` | **19 bytes** copied from the 21-byte database name |
| 75 | delete pending | 1 if `dwDelReq != 0` (written into `szName[19]` of the PDB struct) |
| 76 | rename allowed | 1 if `byEnableRename != 0` (written into `szName[20]`) |
| 77 | `byCloakInfo[6]` | |

This is exactly the US `ps0198` layout documented by OpenShaiya (name 19, then delete and rename flags), now confirmed in the server.
The 0x0101 records for empty slots are sent before the filled ones. Map: `ChkMapByCharLevel` (0x46c320) and a map-table lookup replace
some stored maps (by map id, level and faction; maps 48, 49, 58 and 67 and maps flagged in the map table) with a return map before listing;
the exact table was not written up (unknown, needed only for the map shown on the selection screen).

After the list `m_bEnableDBCharacterSelect` and `m_bEnableDBCharacterRename` are set.

Country values: 0 and 1 are the two factions; **2 means none chosen yet** (inferred: the client may only send 0 or 1, and create, delete,
select and restore are refused while it is 2).

### 6.2 Requests in the character screen

Confirmed, `CUser::PacketCharacter` 0x46bf20.

Every request below is accepted only in state 1. Any other `0x01xx` opcode, **including `0x0119` (name check), closes the connection
(reason 9)**: this server has no name-availability packet. Packet lengths are never checked.

| opcode | request (offsets) | checks in the game server | result to the client |
|---|---|---|---|
| `0x0102` create | `TP_CHAR_CREATE`, 31 bytes: 2 `bySlot`, 3 `byFamily`, 4 `byGrow`, 5 `byHair`, 6 `byFace`, 7 `bySize`, 8 `byJob`, 9 `bySex`, 10 `szName[21]` | country != 2; slot < 5; grow <= max grow; family < 4; job < 6; name trimmed (below) and at least 2 characters. Hair, face, size and sex are not range-checked here. Then `CUser::ProcessCharacterCreate` (0x480e90) builds the database request | refused here: `0x0102` with `u8 1`. Otherwise the database agent's result is forwarded: `0x0102 u8 result`; **0 = success, followed by the full list again** (section 6.1 without the faction packet). Non-zero values are database failures (meanings unknown) |
| `0x0103` delete | `u32 charId` at 2 | country != 2 | when the database agent confirms: `0x0103`, 7 bytes, `u8 0, u32 charId`. The database call is `Usp_Delete_Char_Request_E`, a deletion request (the list's delete-pending flag) |
| `0x0104` select | `u32 charId` at 2 | the list has been received (`m_bEnableDBCharacterSelect`, cleared here so it works once) and country != 2 | when the database agent has loaded the character: `0x0104`, 7 bytes, `u8 0, u32 charId`, then the enter-world burst (not in scope here) |
| `0x0107` | none | | closes the connection (reason 1) |
| `0x0109` faction | `TPC_CHAR_COUNTRY`: `u8 byCountry` at 2 | must be 0 or 1. Sets country to 2 until the database agent answers `0x402`, which stores the new value. The game server does not check that the account has no characters | nothing is sent to the client |
| `0x010E` rename | `TPC_CHAR_RENAME`, 27 bytes: 2 `dwCharID`, 6 `byCharName[21]` | rename allowed (`m_bEnableDBCharacterRename`, cleared until the answer); name trimmed and at least 2 characters | `0x010E u8 2` if refused here; otherwise the database result: `u8 1` success, `u8 2` failure |
| `0x010F` restore | `u32 charId` at 2 | country != 2 | when confirmed: `0x010F`, 7 bytes, `u8 0, u32 charId` |

Name handling in the game server: `TrimString(name, 21)` (0x46c250) forces `name[20] = 0` and removes every space; `ChkCharName` (0x46bf00)
only requires the first two bytes to be non-zero. Character set, length limits beyond 20, uniqueness and banned words are left to the database.
(`ps_dbAgent` builds the create call as text, `Usp_Create_Char_R(%u,'%s',...)`; whether it escapes the name was not checked.)

### 6.3 Database rules behind the character screen

Confirmed, in the restored `PS_GameData` procedures and in ps_dbAgent.

- **Faction and max mode** live in `UserMaxGrow (ServerID, UserUID, Country, MaxGrow)`, one row per account and server. `usp_Read_User_MaxGrow_R`
  returns `Country` and the **constant 3** as max grow, so in this setup every account may create ultimate characters regardless of the stored
  value. `usp_Save_Char_Country_E` inserts the row on the first choice (with `MaxGrow = 1`) and otherwise updates `Country` (no rule against
  changing it). No row means no faction chosen (country 2 in the game server, inferred from section 6.1).
- **Names** (`usp_Create_Char_R`): spaces trimmed at both ends, then refused when any live character **on any server** has the name, or when
  a character above level 10 with that name was deleted less than 7 days ago. Comparison follows the database collation (case-insensitive in
  the usual SQL Server setup; the collation of the restored database was not checked). The skill points of a new character are forced to 5.
- **Create result** (`DBCharacter::CreateCharacter`, ps_dbAgent 0x420270): the client gets `0x0102 u8 0` on success and `u8 1` for any failure
  (name taken, database error), `3` if the user record is gone. The base items are inserted in the same transaction
  (`Usp_Save_Char_Item_Add_E`).
- **Slots**: `usp_Read_CharSlots_R` finds a live character in `(server, account, slot)`; the slot must be free.
- **Delete** is a countdown: `Usp_Delete_Char_Request_E` sets `Chars.RemainTime`; the list's delete-pending flag is `RemainTime != 0`, and
  restore clears it. When the countdown ends ps_dbAgent (`CUser::ProcessDelReq`) calls `usp_Delete_Char_E`, which sets `Del = 1`, records
  `DeleteDate`, leaves the guild and logs to `_DeletedChars`. `RemainTime` holds the request time (packed local date and time,
  `CUser::DeleteCharacterReq` 0x404650). `CUser::ProcessDelReq` (0x4042a0) runs when the list is loaded: every pending character whose request is
  **more than 7200 seconds (2 hours)** old is deleted (and the game server is told with `0x406`). If that leaves the account with no characters on
  the server, its faction is reset to 2 (none), so the player can choose again.
- The list (`usp_Read_Chars_R`) returns live characters (`Del = 0`) of that account on that server, with `RenameCnt` and the creation time.

### 6.4 How a new character is built

Confirmed, `CUser::ProcessCharacterCreate` 0x480e90.

Runs after the checks of section 6.2, then sends `TPC_DB_CHAR_CREATE` (`0x404`) to the database agent.

1. Level 1, stat points 0, skill points 0 (the create procedure then forces 5), exp, money and bank money 0, direction 0, `HG` 0, `VG` 200,
   `CG`/`OG`/`IG` 0.
2. Base HP, SP and MP: `g_GameData.m_Status[job][level 1]`, loaded from `PS_GameDefs` by `EXEC usp_Read_StatusDefs_R` (the new server's `defs.status_defs`).
   Hard mode (grow 2) multiplies the three by 1.1 and ultimate (3) by 1.5, truncated; easy and normal keep them.
3. Starting attributes per class (str, dex, rec, int, wis, luc): fighter 14 12 9 8 7 11, defender 10 9 12 10 10 10, ranger 10 15 9 7 8 12,
   archer 11 10 10 7 10 13, mage 7 9 9 15 12 9, priest 8 9 10 12 14 8.
4. Race bonus and allowed classes (any other combination **closes the connection**, reason 9):

   | family | bonus | classes |
   |---|---|---|
   | 0 human | SP +10, luc +4 | fighter, defender, priest |
   | 1 elf | MP +10, dex +4 | ranger, archer, mage |
   | 2 vail | SP +5, MP +5, int +2, wis +2 | ranger, mage, priest |
   | 3 deatheater | HP +10, rec +2, str +2 | fighter, defender, archer |

   This also settles the race numbering (2 vail, 3 deatheater, as OpenShaiya has it): the start-position keys are `HU`, `EL`, `BA`, `DE` in that
   order, and the classes match `defs.base_items_defs`.
5. Then HP += rec × 5, SP += dex × 5, MP += wis × 5 (with the bonuses included).
6. Position: a random entry from the race's start list, `CMap::LoadGameMapConfig` reading `Data/Map.ini` `[BASE]` `HUGameStartMapPos`,
   `ELGameStartMapPos`, `BAGameStartMapPos`, `DEGameStartMapPos` (format `(map, x, y, z)`, several allowed). The shipped file has one each:
   human (1, 577.98, 78.56, 1762.12), elf (1, 1678.92, 38.92, 1776.91), vail (2, 165.25, 118.50, 397.89), deatheater (2, 1838.93, 129.33, 443.43).
7. Base items: `g_GameData.m_pBaseItemInfo[family][job]` (up to 10, from `usp_Read_BaseItemDefs_R`, the new server's `defs.base_items_defs`), sent as
   `DB_BASEITEM_UNIT` records with a generated unique id and the creation time.

The game server does **not** check that the race belongs to the account's faction (light races 0 and 1 start on map 1, dark races 2 and 3 on
map 2), nor hair, face, size or sex. The client offers races 0 and 1 for country 0 (Light) and races 2 and 3 for country 1 (Fury), verified in
`game.exe` (`docs/original/enter-world.md` section 1.3); the new server enforces this.

The client sends the create request with a **variable length**: 10 header bytes, the name and its terminating zero (`lstrlenA(name) + 11` bytes),
not the 31 bytes of `TP_CHAR_CREATE`.

## 7. The new server's handshake

The new game server does this:

1. Accept only `0xA301` before authentication, in plain text, exactly 22 bytes (stricter than the original: reject other lengths).
2. Look up the session for `dwUserID`. Require: the identity matches (constant-time compare), the session is bound to **this** server id, it has not
   expired, and it has not already been used for a game connection. Mark it used in the same statement. It also requires the
   connecting address to equal the login address (`GAME_CHECK_IP`, on by default; the original did not check).
3. On any failure: close the connection without a reply (the original's behaviour; the client shows its own error). This includes an account that
   still has a connection on this server, in the world or lingering after a lost connection (the original's `CWorld::FindUserBill`;
   `docs/original/world.md` section 4.3); checked before the session is claimed, so a refused handshake does not use the session up.
4. On success, before sending anything: AES key = `aes_key`; both counters = `SHA-256(aes_iv)[0..16]`; positions 0. Decrypt everything the client sends from
   now on.
5. Send `0xA301` (`00 02` + `LENASOFTFOREVER!`) AES-encrypted, then `0xB103` (billing type 0, time 0 until its meaning is known).
6. Send the character screen (section 6.1) from the new server's own tables, mirroring the original as read: `0x0109` (faction, max grow 3)
   before the first list after the handshake, and not before the list that follows a successful create. Then the `0x0101` records.
7. Keep AES for everything sent until the end of the enter-world burst, then switch server-to-client to XOR (section 5).

## 8. Security findings in the original

Confirmed, unless marked.

1. **The handshake ignores the client address** (`dwIP` is sent to the session server and never compared).
2. **The XOR "key" is a constant** and is sent to every client; in-world server-to-client traffic has no confidentiality.
3. **Packet lengths are not checked** in the handshake or the character screen; short packets read leftover buffer contents.
4. A wrong `0xA301` gets no answer and the connection stays open until 11 junk packets (state 0 counter), so idle unauthenticated connections cost nothing to hold.
5. Faction can be re-chosen at any time from the character screen as far as the game server is concerned (`0x0109` is not restricted to
   accounts without characters). Whether the database procedure refuses it is unknown.
6. Character create trusts hair, face, size and sex values from the client.
7. The database agent formats the character name into the SQL text (inferred risk of SQL injection; the name only has spaces removed).

**The new server** (`docs/server/audit.md`, part 1): finding 4 is closed by limits the original does not have. At most 8 connections and 30
new connections a minute from one address, and the whole handshake within 30 s of the connect (pings do not extend it). A write the client
does not take within 20 s ends the connection as a lost one, and so does a queue of more than 4 MiB of packets for it. The idle time counts
from the client's last bytes. More than 100 packets a second for 5 seconds in a row ends the connection; the chat channels without a
cooldown pass 3 lines a second with a burst of 5 and drop the rest quietly. None of this is in the original, whose clients never meet it.

Part 3 of the audit (R21, R22, R29): a position with a coordinate that is not a finite number (a move `0x0501`, a saved position
`0x0220`, the GM's go, summon and guild-go commands) is refused by the decoder, and `World::is_safe_pos` also wants a height within
-10,000 to 10,000; the original stores any float. `frame::encode` no longer panics on an oversize payload: debug builds assert, release
builds log it at error and cut it to 2,044 bytes. A session the handshake claimed is released when the rest of the handshake fails or runs
out of time (the heartbeat would renew the row otherwise).

## 9. Keepalive `0x0003`

Partly confirmed.

Handled in every state. `SReply::CheckRecv` records the arrival time; when timing checks are enabled for the connection's agent, a keepalive that
arrives sooner than `interval - tolerance` after the previous one closes the connection (reason 10, likely speed-hack detection) and one later than
`interval + tolerance` closes it (reason 8). `SReply::CheckTime` also closes a connection whose keepalive is overdue. The interval and tolerances
are fields of the agent object set during start-up; their values were **not** read (no direct callers of `SAgent::InitReply`).

## 10. Open items

- Client side: what `game.exe` does with `0xB103` and with `0x0109` (`0xA301`, `byEncMode` and the XOR switch are settled in
  `docs/original/enter-world.md`).
- `ChkMapByCharLevel`'s map table. (The enter-world burst and states 2 to 4 are in `docs/original/enter-world.md`.)
- Database agent: when `byWithCountry` is set, and the creation stats, position and base items
  (`CUser::ProcessCharacterCreate` in ps_game). Max grow, names, create results and delete storage are in section 6.3.
- The extra u32 at offset 95 of ps_session's `0x801` reply.
- Keepalive timing values (section 9).
- Note on the PDB: the `CUser` type record places `m_Crypto` at `+0x5520`, the code uses `+0x5524`; the fields from `m_qwSessionID` on agree
  (8-byte alignment absorbs the difference). Names in this document come from the PDB, offsets from the code.
