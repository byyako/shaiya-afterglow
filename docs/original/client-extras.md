# Client-ready extras: monster max HP (`0x0608`) and the duplicate-login kick (`0x0004`)

This page covers two packets that the US client `game.exe` (built 2010-04-26, patch 108) already understands but the original EP4.5
servers never send, and what the original does instead: when the client learns a monster's HP, and how a second login of the same
account ends the first one. The server side was read in `ps_session.exe`, `ps_login.exe` and `ps_game.exe` (EP4.5, with their PDB
symbol names). The client facts come from the [client pages](../client/CLIENT-FACTS.md), in particular the
[packet opportunities](../client/packet-opportunities.md).

Not covered: the billing packets of the same handlers (`0x0807`, `0x0808`). GM kicks are on [GM commands 2](gm-commands-2.md#43-f804-kick-name),
the game logs on [The game log](game-log.md).

## In short

- **Monster health bars can be wrong in the original.** The client takes a monster's maximum HP from its own data file
  (`Monster.SData`). Where that file and the server's database disagree, the bar is wrong for the whole fight. The boss Kimuraku is the
  clearest case: its bar stays full for the first nine tenths of the fight. The client has a packet (`0x0608`) that would set the right
  values, but the original server never sends it.
- **Logging in twice kicks the first session, silently.** When an account logs in again, the session server tells the server holding
  the old session to drop it. The old connection is simply closed, with no message. A character in the world then stays there for 10
  seconds before it is saved.
- **The client has a message for this that is never used.** Packet `0x0004` makes the client show "disconnect due to using logged in
  ID." and close itself. None of the original paths sends it.

## 1. Monster HP on the client

Confirmed, client side.

- **`0x0601`** (`TP_MOB_ENTER`, 17 bytes) creates the monster from `Monster.SData[wMOBID]`: model, and **max HP from the client's
  file**; **current HP starts at 0** (client `0x552380`, constructor `0x415670`). A `0x0601` for an id the client already has is
  ignored entirely.
- **`0x0608`** (14 bytes: `u32 id, u32 maxHP, u32 HP`, client `0x57c2f0` → `0x54eff0`) looks the monster up and **sets** its max
  HP (`+0x54`) and HP (`+0x58`), the two values the target bar draws. An unknown id is dropped, so it must come after the `0x0601`.
  The PDB's `TP_MOB_GETHP` is 10 bytes; this client reads 14.
- The original never sends `0x0608` (no instruction in `ps_game.exe` uses the value `0x608`). Current HP reaches the client only
  through `0x0304` (the client's question about a monster, `docs/original/mobs.md`) and the damage and recovery packets
  (`0x0610` adds a signed amount); the max HP stays the file's. Where the two differ the bar is wrong for the whole fight: Kimuraku
  (835, map 11) has 1,115,438 HP in the database and 115,438 in `Monster.SData`, so its bar stays full for the first nine tenths
  (`docs/client/skill-monster-sdata-vs-defs.md`).
- The client also uses the max for the target-HP percentage under the bar (`packet-opportunities.md` opportunities 25 and 26).

## 2. A second login of the same account

Confirmed, `ps_session.exe`, `ps_login.exe`, `ps_game.exe`.

**The session server** keeps one record per account (`CUserMgr`). `CUserMgr::AddUser` (0x409700), for a login of an account whose
record is still active (`+0x74`):

1. **At a login server's server list** (`+0x8` = 1): only when that is **another** login server than the new login's
   (`+0x9` ≠ the new one), `0x0702` (`u32 user id`) to that login server. A second login on the same login server sends nothing.
2. **In a game server** (otherwise): `0x0802` (`u32 user id`) to that game server (`CServer::FindGame` with the record's server, or
   `0xFE` when `+0x88` is set), the server's user count lowered (`CServerList::SendUserCount`), a log line (`0x0302`, kind `0x67`).
3. Either way the record then becomes the new login's (`+0x8` = 1, `+0x74` = 1, the new login server, keys and address).

**The login server**, on `0x0702` (`CClientToMgr::OnRecv` 0x4050a0, case at 0x405604): finds the connection by user id, removes it
and **closes it (`SConnection::Close`, reason `0x65`) with nothing sent**.

**The game server**, on `0x0802` from the session server (`CClientToMgr::OnRecv` 0x409850, case at 0x409ab3):
`CWorld::LeaveUserBill(user id, 0x67)` (0x40d3e0) finds the user (`FindUserBill`) and, the first time (`+0x57b8` counted up to 1),
**closes its connection (`SConnection::Close`, reason `0x67`) with nothing sent**. From there it is an ordinary lost connection
(`docs/original/world.md` section 4.3): in the world with no logout pending, **the character stays 10 s** and is then saved; a
logout pending is saved at once; on the character screen it simply goes.

**The new client** meanwhile is at the server list. If it picks the game server while the old character is still there, its
`0xA301` is refused with no answer (`CWorld::LoginUser`: the user id already in this world, `docs/original/ps_game.md` section 3.3),
and the client gives up after its own wait; a later try works.

**`0x0004`** (3 bytes, `u8`; client `0x578ed0` → `0x54c860`): 0 → message 10070 "disconnect due to using logged in ID.", anything
else → 10071 "disconnect due to system error", shown as a Windows box "Message" (or the in-game box), after which the client closes
itself (`WM_CLOSE` to its own window). Whether any original server sends it is not settled (the value 4 is everywhere in the code,
`packet-opportunities.md`); none of the paths above does.

## 3. The new servers

### 3.1 State before this step

- The login server replaces the account's `sessions` row on every login (`crates/login-server/src/pg.rs`, `create`), so an older
  login connection still at the server list can no longer bind a server (`bind_server` finds a different identity): it gets "cannot
  connect", as near to the original's silence as matters. Nothing changes there.
- The game server refuses a handshake for an account it still has (`Online` in `crates/game-server/src/server.rs`), as the
  original, but **nothing ever ends the older connection**: a second login leaves the first player in the game and the second one
  locked out until the first leaves on its own (a half-open connection: until its idle timeout).

### 3.2 The kick (the original's `0x0802`)

- The login server's session insert also sends **`pg_notify('account_login', '<user id> <created>')`** in the same statement, where
  `<created>` is the new session's `created_at` in microseconds since the epoch (not secret; the identity is never put in it). The
  notification reaches the listeners when the insert commits, before the client has the server list.
- Each game server **listens on `account_login`** (`sqlx::postgres::PgListener`, reconnecting by itself). For a user id it has a
  connection for, whose session was created **before** `<created>`, it ends that connection: the connection's next wait sees the
  kick, and from there it is an ordinary lost connection (section 2: the stay of `linger_secs` in the world, saved at once with
  a logout pending, gone at once on the character screen). A connection whose session is the new one (the same `created`, or later)
  is never touched, so a notification arriving late cannot end the new login.
- The claim of a session returns its `created` for this (`SessionGate::claim`).
- Several game servers: each one only ends what it has; one login server kicks on all of them, as the session server did.

### 3.3 Deviations

1. **`0x0608` after every `0x0601`**: `u32 id`, the monster's max HP from the database (`defs` / `MobDef::hp`), its current HP;
   in the same flush, to the same players (spawns, a player's view on entering a map, cell changes of a player or a monster,
   summons). The original never sends it. Every target bar then shows the right max and current HP from the first frame
   (Kimuraku's included). Nothing else changes: damage, `0x0304`, `0x0305` and `0x0610` stay as they are.
2. **A kicked game connection gets `0x0004 00` first** ("disconnect due to using logged in ID.", and the client quits by itself),
   sent with the connection's current encryption, then the sending side shut down (FIN, so the 3 bytes are not lost to a reset)
   and what the client still sends read and dropped for up to 10 s (the logout's grace) before the socket closes. The original
   closes silently; the old client then only shows its own "connection lost".
3. **A handshake for an account whose older connection is still leaving this server waits for it** (up to
   `handshake_wait_secs`, default 15 s: the 10 s stay and 5 s more, inside the client's 30 s) and then goes on, instead of
   being refused. The wait happens before the session is claimed, so a
   handshake that ends up refused still leaves the session unused; one that waits in vain is refused as before. With the kick of
   3.2 the older connection is always leaving, so the second login gets in as soon as the first character is saved, without the
   player trying again.

### 3.4 Open (client side)

- Whether `0x0004` on the **login** connection shows the same box (not used: the login server sends nothing, 3.1).
- *Live*: the box's text in this client's language file, and that the client indeed quits after it.
