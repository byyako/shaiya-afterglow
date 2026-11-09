# Login server (ps_login)

This page covers the EP4.5 login server, `ps_login.exe` (built 2009-06-10): how it accepts a connection, the key exchange and the
cipher, the login check against the database, the server list and server selection, and the security problems in the original code.

The binary is `ShaiyaServerPatch/Server/ServerMain/PS_Login/ps_login.exe`
(SHA-256 `8aa4b377a1d05d97a8ab1def841f0291643a47226bb80ffb8524ba8029c13f07`). It was studied by static analysis in Ghidra 12.1.4;
nothing was executed. Function names come from `ps_login.pdb`, the program's debug-symbol file (6 of the 7 server PDBs match their
binaries exactly, and this one does). Addresses are virtual addresses in the image (base `0x400000`).

## In short

- **The login server is the front door.** The client connects to it first, proves who the player is, picks a game server from the list,
  and is then sent on to that game server.
- **Encryption starts before the password is sent.** The server sends one of a pool of ready-made RSA public keys, the client answers with
  a random secret, and both sides turn it into an AES key. From then on every packet is encrypted, except its 2-byte length.
- **Names and passwords are cut at 12 characters.** Whatever the client sends, the server only looks at the first 12 characters of each.
- **The account's status number doubles as its staff level.** Ordinary players come out as privilege 11; game masters as 1 to 5.
- **The original has real security holes**: the login query is built by pasting the username and password into SQL text, so a crafted
  12-character name can run a statement; it also tells "no such account" apart from "wrong password", and counts failed logins per
  connection, not per address.

Status words on this page: **confirmed** = read in the code. **inferred** = follows from confirmed code but is not directly shown.

## 1. Connection state machine

Confirmed.

`CUser::OnRecv` (0x4042f0). Per-connection state at `this+0x134`: 0 = connected, 1 = login query in progress,
2 = authenticated, 3 = login reply and server list sent. Flags: `this+0x150` = encryption active,
`this+0x151` = public key sent, `this+0x138` = 0xFF means a server may be selected, 0xFE after a selection was accepted.

Before login the server accepts exactly three opcodes. Anything else disconnects with reason 9 (`FUN_00411160(this, 9, 0)`):

| opcode | allowed when | action |
|---|---|---|
| `0xA101` | key sent and encryption not yet active | if `payload[0] <= 0x80`: `CUserCrypto::KeyInit(payload + 1)`, else disconnect |
| `0xA102` | encryption active and state 0 | copy username and password, state := 1, queue the database check |
| `0xA202` | state 3 and `this+0x138 == 0xFF` | server selection (below) |

When encryption is active every incoming packet is decrypted first: `SAes::EncCTR(data + 2, length - 2)`.

## 2. Handshake `0xA101`, server to client

Confirmed.

`CUserCrypto::KeyPublicSend` (0x404de0). The server holds a **pool of pre-generated RSA keys** (`CKeyMgr`, entries of 0x84 bytes
at `0x456dbc`, count at `0x456db8`) and picks one at random per connection (`rand() % count`). Packet buffer (before the 2-byte
length prefix, 0xC5 bytes):

```
u16 0xA101
u8  0                      encryption mode, always 0
u8  exponentLen = dwords * 4   (64 for the pool keys)
u8  modulusLen  = dwords * 4   (128)
u8  exponent[64]
u8  modulus[128]
```
Wire size is 0xC5 + 2 = 0xC7 bytes, matching the forum capture. Sets `this+0x15 = 1` (public key sent).

## 3. Key exchange

Confirmed.

Client reply `0xA101`: `u8 length (<= 0x80)` then the RSA ciphertext. `CUserCrypto::KeyInit` (0x404ea0):

1. The ciphertext bytes (as many as the length byte says; the real client sends 128) are loaded raw into a big integer (dwords in
   memory order) = **little-endian integer** (`BinToMpz` 0x40bc10, called at 0x404ed2).
2. `SRsa::Dec` (0x40c2c0) decrypts with the CRT components `p, q, dP, dQ, qInv` (offsets `+0x18` to `+0x60` of the key).
   Plain RSA, no padding.
3. The result is copied back out as raw dwords into a 128-byte buffer (little-endian, zero-padded at the high end).
4. `HMAC-SHA256` (`FUN_00410c90`: standard, SHA-256 IV `6a09e667...`, ipad `0x36`, opad `0x5c`, keys longer than 64 bytes are hashed first):
   - **key** = the 128-byte decrypted buffer (so it is hashed to 32 bytes first),
   - **message** = the modulus, 128 bytes (as sent, little-endian).
5. 32-byte digest stored at `this+0x34`. `digest[0..16]` = **AES key**, `digest[16..32]` = **IV / initial counter**.
6. `FUN_0040c7b0(128, key)` = standard AES-128 key expansion (FIPS-197), run for both the encrypt context (`+0x54`) and the
   decrypt context (`+0x16c`).
7. Both counters (`+0x148`, `+0x260`) are set to the same IV and both stream positions to 0. `this+0x14 = 1` activates encryption.

Weakness worth fixing in the rewrite: the length byte is only checked to be `<= 0x80` (`CUser::OnRecv` 0x404540) and is never compared
with the packet's size, so a packet shorter than its length byte makes `KeyInit` read past the received data. A length byte of 0 to 4
also makes `BinToMpz` write zeros in front of its buffer: it sizes the number as `n = ceil(length / 4)` dwords and clears dwords `n - 1`
and `n - 2` (0x40bc50, 0x40bc57), so 0 clears the 8 bytes before the heap buffer and 1 to 4 the 4 bytes before it (confirmed). Whether
that crashes the process is inferred (it corrupts the heap block's header; only a run would show when it fails).

## 4. Stream cipher

Confirmed.

`SAes::EncCTR` (0x40db40), used for both directions (`CUserCrypto::Enc` 0x405010 and `::Dec` 0x405030 both call it, with the
encrypt object at `+0x54` and the decrypt object at `+0x16c`):

- **AES-128 in counter mode, continuous keystream.** The object keeps the 16-byte counter (`+0xF4`), a 16-byte keystream buffer
  (`+0x108`) and a position (`+0x104`). Leftover keystream bytes are consumed first; whole blocks follow; a trailing partial block
  generates one more block and leaves the position in the middle of it. **State persists across packets**: there is no per-packet reset
  and no overlapping tail (contradicts the forum description).
- **Counter increment is little-endian across the 16 bytes**: the first dword is the low word and carries upward
  (`SAes::CtrInc` 0x40db10: `add [eax+0xf4], 1; jne ...; add [eax+0xf8], 1; ...`). This is not NIST big-endian CTR.
- The block input is the counter bytes as stored in memory, so the first byte of the IV is the least significant byte of the counter.
- Applies to `data + 2`, length `u16(data) - 2`: the 2-byte length field stays plaintext, the opcode and payload are encrypted.
- Encryption and decryption are the same operation. Server send and server receive use separate objects, both seeded with the
  same key and IV, so each direction has its own counter starting at the same value.
- The block encrypt `AES_encrypt` (0x40cf00, the PDB name) is OpenSSL's table implementation in the standard byte order: it loads
  the input as big-endian words (byte swaps at 0x40cf08 to 0x40cf82), stores the output the same way (0x40d409), and its tables are the
  standard ones (`Te0` at 0x447f78 begins `0xc66363a5`, `Te4` at 0x448f78 `0x63636363`) (confirmed). The new server's copy uses a
  standard AES-128, passes the FIPS-197 known answer (`crates/shaiya-crypto/tests/known_answers.rs`), and the real client logs in
  through it (section 14).

## 5. Login `0xA102` client to server

Confirmed.

`CUser::OnRecv`, state 0, encryption active. Payload layout used by the server:
- bytes 0 to 31: username (NUL at index 31 forced). **Index 12 is also forced to 0, so the username is cut at 12 characters.**
- bytes 32 onward: password. **Byte 44 is forced to 0, so the password is cut at 12 characters.**
- The server never looks past byte 44, so the packet may be 48 bytes (password 16) or 51 bytes (password 19) of payload and
  both work. The forum/OpenShaiya "19" and Core/Imgeneus "16" disagreement does not matter to the server.
- Username goes to `this+0x3c0`, password to `this+0x3e0`, then the database check is queued.

## 6. Password check

Confirmed, details inferred.

`CUser::OnDispatchDB` (0x404a10) case 2 hashes the password with `SMD5::MD5` (0x40a3b0: the MD5 initial values, then `md5_update`
0x40a160 and `md5_final` 0x40a210) and writes the 16-byte digest as **32 upper-case hex digits** with `BinToHexL` (0x40b0b0, called with
16 bytes at 0x404a97; its digit table is `0123456789ABCDEF`) (confirmed), then calls `CUser::ChkPasswdMD5` (0x404700), its only caller being 0x404ab5. It runs the first of these stored procedures; `CUser::ChkPasswd`
(0x404580), the plain-password form, has no caller in this build (corrected 2026-11-09, found when comparing the EP3, EP4.5 and EP5.4 builds: [`ep3.md`](ep3.md), [`ep5.4.md`](ep5.4.md)) (strings at 0x447054, 0x4470b0):

```
EXEC usp_try_gamelogin_korea '%s','%s', %I64d,'%s'
EXEC usp_try_gamelogin3_r    '%s','%s', %I64d,'%s'
```
Arguments: user id (varchar 18), password, session id (bigint), client IP (varchar 15). Whether the password argument is plain text or an MD5 hex string depends on the variant: `usp_Try_GameLogin3_R` takes `varchar(18)` and hashes it itself, so it receives plain text; the `_Taiwan`/`_korea` style takes `varchar(32)`, so it receives the MD5 hex that `OnDispatchDB` computes. The binary has two paths, and only one runs: `ChkPasswdMD5` (0x404700) calls `GetUserMD5` (0x406e50), which formats **only
`usp_try_gamelogin_korea`** (the string at 0x447054, pushed at 0x406ed9) with the MD5 hex; `GetUser` (0x406b50), the only user of the
`usp_try_gamelogin3_r` string (0x406bd2), is called only from `ChkPasswd` (0x4045e6), which nothing calls (no call and no pointer to it
in the image) (confirmed). Only `usp_Try_GameLogin3_R` and `usp_Try_GameLogin_Taiwan` exist in the restored `PS_UserData` (their texts
were read from the backup), so this build needs a `usp_try_gamelogin_korea` that the shipped databases do not have. `GetUserMD5` binds
three result columns (a `smallint` status, the `UserUID` and a third `int`, 0x406e50); the two shipped procedures return two.

`usp_Try_GameLogin3_R` (read from the restored database, `sp_helptext`) does:
1. `SELECT UserUID, Enpassword, Status, Leave FROM Users_Master WHERE UserID = @UserID`.
2. Unknown user (`UserUID = 0`) or `Leave = 1` returns status **-3**.
3. Otherwise compares `master.dbo.fn_md5(@InPassword)` with `Enpassword` (`char(32)`, an MD5 hex digest). Mismatch returns status **-1**.
   `fn_md5` is an extended function in `master`, not part of the backups. The `Taiwan` variant calls `sp_LoginSuccessCheck` instead
   (and takes `varchar(32)`).
4. Status 2 to 6 means blocked: it looks up `Users_Block.BlockEndDate`; if the block expired, status is reset to 0 and written back.
5. Returns one row `(Status, UserUID)`.
6. For status 0, 16, 32, 48, 64 or 80 it writes a login log row (`usp_Insert_LoginLog_E`). The Taiwan variant also upserts
   `UserLoginStatus` (login status 1, login time, session id).

So `Users_Master.Status` is a mix of account state and GM level: 0 normal, 2 to 6 blocked states, and 16/32/48/64/80 are the staff levels (the
login server's byte at `this+0x104` is the "privilege" it forwards to the client and uses in the user-count check). Other `Users_Master`
columns: `Pw varchar(12)` (a plain-text password column, which explains the 12-character limit), `Enpassword char(32)`, `Admin bit`,
`AdminLevel tinyint`, `UseQueue bit`, `UserType char(1)`, `Point int`, `UserIp`, `Birth`. The rewrite should not keep a plain-text column and
should hash with a modern algorithm (the server sees the plain password after decryption, so any hash can be used).

On success: user id (the procedure's out value) at `this+0x100`, privilege at `this+0x104`, state := 2, the user is added to the online map
(`CServer::AddUser`), and a `0x701` message of 0x67 bytes goes to the **session server**: the client's IPv4 address at offset 2, the user id at 6,
the privilege at 10, the 64-bit session id at 11, the user name at 19, the 16-byte identity at 51, the 32 bytes of
`CUserCrypto::GetKeyGen` (the AES key and the first 16 bytes of SHA-256 of the login counter) at 67, and the login procedure's third
result column at 99 (confirmed, built at 0x40481d to 0x404869) (corrected 2026-11-09, found when comparing the EP3, EP4.5 and EP5.4 builds: [`ep3.md`](ep3.md), [`ep5.4.md`](ep5.4.md)). So the game server learns the
AES key through the session server, which is how the XOR layer and the game handshake are keyed later (consistent with [W]).

On failure: attempt counter `this+0x130` is incremented; below 10 the state returns to 0 and an `0xA102` with the error status is sent;
at 10 the connection is dropped (reason 9).

## 7. Login reply `0xA102` server to client

Confirmed.

`CUser::LoginOK` (0x404930), 24 bytes before the length prefix:
```
u16 0xA102
u8  0              status (success)
u32 userId         this+0x100
u8  privilege      this+0x104
u8  identity[16]   this+0x140
```
then the server list is sent immediately and state := 3. A failure sends the same opcode with the error status in byte 2 (the rest unused).

## 8. Server list `0xA201`

Confirmed.

`CServerList::SendList` (0x4072c0): `u16 0xA201, u8 count` then `count` entries of 0x26 (38) bytes: `u8 id, u8 status, u16 players, u16 capacity,
char name[32]` (NUL-terminated). Sent in chunks of at most 48 entries (`0x30`). A server that is not connected is reported with status 2
(closed), players 0, capacity 0x7FFF. In "combined list" mode (`DAT_004508fd != 0`) it iterates the combined table instead.

## 9. Server selection `0xA202`

Confirmed.

Request: `u8 serverId, u32 clientVersion` (version at `payload[1..5]`). Checks in order, each failure answered with
`u16 0xA202, u8 result, u8 ip[4] = 0` (7 bytes before the length prefix):

| result | meaning | check |
|---|---|---|
| `0xFE` (-2) | cannot connect | server id unknown, or `EnableConnect` false |
| `0xFC` (-4) | server saturated | `EnableUserCount(list, privilege < 0xB)` false: the server-full check (confirmed, 0x407500). With the flag set (staff, `privilege <= 10`, `setbe` at 0x40443d) it passes; otherwise it passes only while the entry's player count (`+0x10`, written by `SetUserCount` 0x4072a0) is below its capacity (`+0x12`, `SetUserCountMax` 0x4072b0) |
| `0xFD` (-3) | version mismatch | `EnableVersion(clientVersion, list)` false |

On success the login server forwards a `0x703` message (8 bytes) to the session server, sets `this+0x138 = 0xFE`, and the session server answers it.
The session's `0x703` reply carries no address (ps_session 0x4040e1, confirmed). The login server answers the client when that reply
comes (`CClientToMgr::OnRecv` 0x405562 to 0x4055d8): `u16 0xA202`, as the result byte the byte at offset 6 of the reply (the server id,
or 0xFF when the session refused; also stored at `this+0x138`), and the IP of that server from **the login server's own server list**
(entry `+0x0c`, 0 for an id outside the list; inlined at 0x4055b3 to 0x4055c7) (confirmed) (corrected 2026-11-09, found when comparing the EP3, EP4.5 and EP5.4 builds: [`ep3.md`](ep3.md), [`ep5.4.md`](ep5.4.md)). The `0xFE/0xFD/0xFC` values equal the -2/-3/-4 codes in the Imgeneus doc.

## 10. What this changes in the protocol notes

- The three open questions are answered: HMAC argument order (key = decrypted number, message = modulus), keystream behaviour (continuous
  AES-CTR, little-endian counter, persistent state, no overlapping tail), and password length (server cuts both fields at 12 characters).
- New facts: RSA key pool, 12-character limit, privilege-dependent user cap, error codes for server selection, the `0x701`/`0x703` session
  server messages, the login stored procedures and the meaning of `Users_Master.Status`.

## 11. Open items

- Settled 2026-11-09: `ps_session` does not complete the select-server reply (section 9), and it passes the `0x701` key bytes to the game
  server in its `0x801` reply (`docs/original/ps_game.md` section 3.2).
- Settled: `GetUserMD5` (0x406e50) runs `usp_try_gamelogin_korea` only, and maps `Status` as in section 12 (section 6).
- Settled: the cipher is standard AES-128 (section 4).
- `CKeyMgr::GenKey` / `SRsa::GenKeyPair`: the public exponent. Settled 2026-11-09: the pool starts with 16 keys of 1024 bits made at
  start-up (0x456dbc to 0x4575fc), and `CKeyMgr::OnUpdate` (0x406af0) adds one per run up to 1024 (confirmed; the same in EP3).

## 12. Database result mapping and privilege

Confirmed, `FUN_00406e50` at 0x406e50.

After the query the function reads the first row `(Status, UserUID)` and converts `Users_Master.Status` into the `0xA102` result byte. The
privilege byte sent to the client is also derived here.

| `Status` value | `0xA102` result | meaning (client messages, from [W]) |
|---|---|---|
| `UserUID == 0` | 1 | account does not exist |
| -3 | 1 | account does not exist / left |
| -2 | 2 | cannot connect |
| -1 | 3 | wrong ID or password |
| 1 | 9 | cannot connect, user request |
| 2 | 4 | account cannot log into the game |
| 6 | 5 | cannot log into the game or homepage |
| 0x11 | 10 | blocked |
| 0x21 | 11 | restricted |
| 0x31 | 12 | restricted |
| 0x41 | 13 | unused message |
| 0x51 | 14 | unused message |
| 0x80 | 6 | being terminated at the user's request |
| 0x100 | 7 | being terminated at the user's request |
| 0x200 | 8 | being terminated at the user's request |
| anything else | see below | |

For the remaining values (including 0, 16, 32, ...): bit 1 set gives 10; bit 0 set gives 11; if `(Status >> 7) & 7 == 0` the login succeeds with
**privilege = `(Status >> 4) & 7`, and 11 (0x0B) when that is 0**; otherwise result 6.

So an ordinary player is privilege **11**, and staff are 1 to 5 (`Status` 16, 32, 48, 64, 80). The select-server check
`EnableUserCount(list, privilege < 11)` therefore means "this account is staff". My first version sent 0 for players, which falls in the staff
range. Fixed: the new server defaults to 11.

## 13. Security findings in the original

Confirmed.

1. **SQL injection.** `FUN_00406e50` builds the query with `sprintf("EXEC usp_try_gamelogin_korea '%s','%s', %I64d,'%s'", username, password, ...)`
   and runs it directly. Nothing in the path escapes quotes: `CUser::OnRecv` only cuts each field at 12 characters. Twelve characters are enough
   for an injected statement (for example `';SHUTDOWN--` is exactly 12). The remaining arguments are numeric or the client IP, which the server
   supplies. The same pattern very likely exists in the other query variants (`usp_try_gamelogin3_r`, the Taiwan form) built by the same code.
2. **The key reply's length byte.** It is only checked to be at most 0x80 and never against the packet's size: a short packet makes
   `KeyInit` read past the received data, and a length of 0 to 4 makes `BinToMpz` zero 4 or 8 bytes in front of its heap buffer
   (section 3).
3. **Account enumeration.** Result 1 (no such account) differs from result 3 (wrong password).
4. **Unlimited reconnects.** Failed logins are counted per connection (10), not per address, so guessing continues by reconnecting.
5. **Plain-text password column** (`Users_Master.Pw`, `varchar(12)`) next to the MD5 hash, and an unsalted MD5 for the hash.

The new server: parameterised queries (no string building), exact-length key reply, the same refusal for unknown account and wrong password,
argon2id hashes, and no plain-text column. A limit per address is now in place (concurrent connections, failed-login lockout, parallel hashing); see README.

Part 1 of the 2026-10 audit (`docs/server/audit.md`) adds, again beyond the original: at most 30 new connections a minute from one address;
the key exchange within 30 s of the connect and the login within 10 minutes of the key exchange, pings extending neither; the key
exchange's RSA step off the async threads, four at a time.

Part 3 of the audit changes the login guard, again beyond the original: wrong passwords are counted per account and address (5 in 15
minutes make that address wait 15 minutes for that account, answered "wrong password" without hashing) and per account over all addresses
(30 an hour), by the lowercased name, unknown names included; a good login clears only its own account's counts and no longer the
address's (the address lockout of 20 in 10 minutes stands); a new password has 6 to 16 printable characters, read to 16 at login (the original cut it at 12, section 3, and the username still is; 12 is not
the client's: the US client's login boxes take 19 characters and send the first 16 of a password, see below; accounts with shorter passwords
still log in).

**What the client allows** (a read-only look at `game.exe`, for the 12-character question): the login window's text input
(the IME object whose `+0x24` is the cap, set by `FUN_0051f440`, checked in `FUN_00522370`: input is refused once the typed byte length
reaches it) gets **0x13 = 19** at the login window's construction (`FUN_004b1f70`) and after a failed login (`FUN_004b61b0`), **12 only when
the region variable `0x6f1558` is 4 (Japan), 31 when it is 0xb**; this build starts at region 6, so 19. Nothing on the Tab or click paths changes
it, so the ID and the password boxes share it (inferred: the cap setter `0x51f440` has many callers, and typing into both boxes
settles it; `docs/testing/live-test-checklist.md` section 64). The window's buffers are 35 bytes each. The sender
(`0x5867c0`) puts **32 bytes of ID and the first 16 bytes of the password** into `0xA102` (a 16-character password goes out without a
terminating zero; the 17th to 19th typed characters are dropped on send). So a client cannot use a password longer than 16 characters, and
nothing in it needs 12..

## 14. What the real client sent

- Key exchange, cipher, login and server list were accepted by the real US client: the section 3 and 4 behaviour is confirmed against it.
- Server select: `client_version = 268707328` (`0x10042600`). `Version.ini` says 108, so the field is not that number; it is probably
  computed by the client. The original compared it to a per-server value in the session/game configuration (`GameVersion` in `ps_game.ini`).
- On quitting, the client sends opcode `0x010B` (no payload) on the login connection.
- After a successful select it closes the login connection (connection reset) and later opens a new one.

## 15. Confirmation from the guide, and the staff levels

The original files' `Docs/GuideToShaiyaConfiguration.pdf` (the configuration guide) section 19 lists the `Status` values (taken from `usp_Insert_UserBlockLog_R`, Korean
translated by machine, so the wording is approximate): 0 normal; 1 stop; 2 block (game); 4 block (web); 6 block (game and web); 8 block
(character); 9 restricted; **16 / 32 / 48 / 64 / 80 = GM level 1 to 5, 16 being full operator permissions**; 128 / 256 / 512 withdrawal;
17 to 81 = GM plus stop; and negative values for the messages shown to the player.

This matches the code: the staff level is `(Status >> 4) & 7` and becomes the privilege byte, so 16 gives privilege 1 (most powerful). Status 4
and 8 let the login through (they are enforced elsewhere). The guide's negative values -4 to -16 agree with the bit rules for 11 of the 13
values (blocked: -5 -6 -9 -13 -14; restricted: -7 -11 -15; termination: -4 -12 -16). The two that differ are -8 and -10 (the guide's text
reads as "disconnected" and "restricted"; the rules give termination and blocked), which may be translation noise. These checks are unit tests in
`crates/login-server/src/status.rs`.

## 16. The client version

Confirmed, in `game.exe`.

The `0xA202` version field is the constant `0x10042600`, a literal in the code that builds the packet. For this US build it never changes.
The original server only compares it when a global check is switched on and the server entry has a version; `0xFFFFFFFF` means "any".
The new server mirrors that: `LOGIN_CHECK_VERSION` (off by default) and `LOGIN_GAME_VERSION` (unset means any).

## 17. Other episodes

The EP3 `ps_login.exe` (linked 2007-12-07) and the EP5.4 one were compared with the EP4.5 build, read statically like the rest of this
note. The EP5.4 file is the EP3 build of 2007-12-07 with 7 bytes changed, so an EP3 address is also the EP5.4 address of the same code.
Overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- The network library, the key exchange, the AES cipher, the server list, server selection, the console commands and the session link
  are the same code as EP4.5 (confirmed, side-by-side diffs; for example `CUser::OnRecv` EP3 ps_login 0x4042f0, `CServerList::SendList`
  EP3 ps_login 0x407180).
- The login packet keeps a user name of up to 31 characters and a password of up to 32 (EP3 ps_login 0x4044ea, 0x4044ee); EP4.5 cuts
  both at 12 (confirmed).
- The password is not hashed. The check that runs is `ChkPasswd` (EP3 ps_login 0x404580, matched by call-graph shape), which calls
  `usp_Try_GameLogin_Taiwan` with the plain password and binds two result columns (confirmed, call site EP3 ps_login 0x404a73).
- So `0x701` to the session server is 0x63 bytes, without EP4.5's last 4-byte field (confirmed, built at EP3 ps_login 0x4046bc).
- The combined server list is loaded only with `[GAMESERVERS] EnableServerComb=1` (EP3 ps_login 0x40757a); there is no `[MONITOR]`
  listener (confirmed).
- What operators should know: the `usp_Try_GameLogin_Taiwan` in the shipped EP3 `PS_UserData` has its password check commented out,
  so it accepts any password for an existing account (confirmed from the procedure text). Together with the call above, an EP3 server as
  shipped lets anyone into any account by name.

### EP5.4

The 7 changed bytes are three patches (confirmed, `cmp` of the two files):

| where (EP5.4 ps_login) | change | effect |
|---|---|---|
| 0x404542, `0xA101` case | `ja` → `jne` after `cmp al, 0x80` | the key reply's length byte must be exactly 0x80 |
| 0x411abd, `SAgent::OnAccept` (matched by its unique string) | `jge` → two `nop` | the `MaxConnection` cap is gone; connections are accepted up to the pool size |
| 0x445e70, the login query (used at 0x406ab8) | `'%s'` → `"%s"` around the user name and password | narrows the SQL injection of section 13 item 1 but does not close it; the text is still unescaped (inferred; not worth settling, the new server uses parameters) |

The shipped `ps_login.ini` sets `MaxConnection=3000`, which no longer limits accepting (confirmed: the value is stored only at
`SAgent+0x12c`, by `SAgent::Init` 0x41169f, and besides the patched compare at 0x411aab its only reader is `GetMaxConnection` 0x404030;
the other writes are the constructor's 0xFFFF at 0x411f3d and `SetMaxConnection` 0x403660), and has no `[GAMESERVERS]` block,
which this code does not need. The EP5.4 `usp_Try_GameLogin_Taiwan` does check the password (through `sp_LoginSuccessCheck`;
confirmed from the procedure text).

Comparing the builds showed this about the EP4.5 text above:

- The key-reply defect of sections 3 and 13 item 2 includes a heap write: with a length byte of 0 to 4, `BinToMpz` zeroes 4 or
  8 bytes in front of its buffer (EP4.5 ps_login 0x40bc50, 0x40bc57); whether that crashes the process is inferred. Sections 3 and 13
  now say so.
- Sections 6, 9 and 11 were corrected in place on 2026-11-09 (the procedure that runs, the `0x701` layout, the `0x703` reply, the key
  pool).
