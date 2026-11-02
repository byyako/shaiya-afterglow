# Robustness audit

A read-only review of `crates/`, made in October 2026 before 20 to 100 untrusted players were to be let in. It looked for what
F1 and F2 were (`docs/client/CLIENT-FACTS.md`, "Open for the server"): half-done changes, item and gold duplication, client values
trusted without a check, decoders that panic or allocate without limit, lock order and blocking under locks, leaked tasks, and what a
hostile client or a flood of connections does at the login server and in the first game packets.

How it was done: five reviews by area (decoders; connections and login; player-to-player exchanges; auction, mall and item changes;
locks, database transactions, GM and guild checks), then every finding below was checked against the code by the reviewer. A finding that did not hold is not here. What was checked and found sound is in section 4, so the next audit need not redo
it. Not covered: `memory.rs` (the test backend) beyond its parity with `pg.rs`, the client, and anything built after the audited commit.

Line numbers are those of the audited commit, before the fixes. `server.rs` is `crates/game-server/src/server.rs`; other game-server paths are relative to
`crates/game-server/src/`.

Section 2 has the findings in order of harm, section 3 each one in full, section 5 the fuzz harness and section 6 how the fixes were built.

## 1. Decisions

1. **Fuzz harness: proptest**, on the stable toolchain inside `cargo test` (cargo-fuzz would need nightly and runs by hand).
2. **Exchanges: cancel the whole trade or duel when an offer changed**, and close the gaps: a trade and a duel stake window cannot be
   open at once, and selling, dropping, moving or using a staked item closes the duel window, as it already ends a trade (R3).
3. **Game-server connections: at most 8 per IP address**, 30 s from connecting to finish the handshake (pings do not extend it), and a
   client that stops reading for 20 s is disconnected (R1, R2).
4. **Rate limits: extra chat is dropped quietly, floods are disconnected.** About 3 lines a second with bursts of 5 for the chat
   channels without a cooldown; more than about 100 packets a second for several seconds ends the connection (R4).
5. **Login guard: per account and address, passwords 6 to 12.** 5 wrong passwords for one account from one address make that address
   wait 15 minutes for that account; at most about 30 wrong passwords per account per hour from all addresses together; a good login
   clears only its own account's count; new passwords need 6 to 12 characters (R7).
6. **On a panic: survive it.** Locks keep working after a panic, the player's connection ends and is cleaned up and saved as on a lost
   connection, the server keeps running and logs it (R5).
7. **Small behaviour changes, all accepted:** auctions close on time (R20); stricter input checks (R21); a disband re-checks the guild
   bank (R23); more item actions end a trade, and no trading while dead (R3).
8. **Data editor: loopback and a host check.** The start script binds 127.0.0.1 (an SSH tunnel from elsewhere), and every request must
   be addressed to localhost (R17).

Defaults chosen without asking, because a real client never meets them: a per-address limit on new connections a minute at
both servers (with decision 3); at the login server, 30 s from connecting to finish the key exchange and 10 minutes from the key
exchange to log in, pings extending neither (R10).

## 2. Findings by harm

**Critical**: an outsider or one player can take the whole server down, or take another player's goods, on purpose.
**High**: memory or data loss, players locked out, or a feature frozen for everyone, with some effort or a common failure.
**Medium**: needs a database failure or bad timing, or makes an attack cheaper. **Low**: latent, data-dependent or defence in depth.

| id | harm | finding | where |
|---|---|---|---|
| R1 | critical | A client that stops reading wedges its connection forever: no kick, no newer login, no idle timeout; its packet queue grows without limit | `server.rs:643`, `server.rs:1653`, `zone.rs:74`, `zone.rs:517` |
| R2 | critical | One address can hold every game-server connection slot without logging in (no per-IP limit; a ping every 29 s keeps a socket) | `server.rs:215-240`, `server.rs:1003` |
| R3 | critical | Free stakes and phantom offers: a staked or offered item can leave the bags, the exchange skips it, and the other side still pays | `trade.rs:356-365`, `server/duel.rs:63-67`, `zone/my_shop.rs:89` |
| R4 | critical | No limit on normal, zone, whisper, party and guild chat: one player floods a map, fills every queue (with R1) and hogs the `maps` lock; several packets write to the database each time | `server.rs:2474-2532`, `server.rs:2668`, `friends.rs:290` |
| R5 | critical | One panic while a shared lock is held poisons it and every later player panics, while the heartbeat keeps the server listed; any panic in a connection skips its cleanup | 560 × `lock().unwrap()`; `server.rs:1555-1621`; `main.rs:110-116` |
| R6 | critical | An accept error (too many open files) ends `serve` and the process, without the shutdown save | `server.rs:217`, `main.rs:119`; login `server.rs:45` |
| R7 | high | Password guessing never stops: a good login with one's own account clears the address's failures; no per-account limit; 1-character passwords | login `limits.rs:91-95`, `server.rs:184`, `accounts.rs:49` |
| R8 | high | The auction-house turn and the guild-bank turn are held across writes to the client: one slow reader freezes every auction (and the end pass) or every guild bank | `server/market.rs:84-104,115-142`, `server/guild_bank.rs:23-70` |
| R9 | high | The handshake takes the account's slot before checking the identity, and one shared wake-up serves every waiting handshake | `server.rs:1020-1028`, `server.rs:795`, `server.rs:950` |
| R10 | high | Login server: pings keep a pre-login socket for ever; the RSA step runs on the async threads with no rate limit | login `server.rs:108`, `server.rs:136`, `server.rs:144` |
| R11 | high | Eight database connections for everything (the listener keeps one); the monster tick waits on database writes | `main.rs:25`, `main.rs:147`, `server.rs:552`, `server.rs:575` |
| R12 | medium | PvP rewards: the gold (or points) and the step are two writes; a failure between them lets the reward be claimed again | `server/pvp.rs:174-178`, `server/pvp.rs:193-199` |
| R13 | medium | F1 is only partly fixed: Blessing, item buffs, the experience wheel, moves by item and others take effect before the item is used up | `server/items.rs:114-128,193-229,248-250,465-478` |
| R14 | medium | Party loot and party gold vanish on a failed save, on a write error, or when the member leaves with the hand-off still queued | `server.rs:4448`, `server.rs:1687`, `server.rs:1583`, `zone.rs:2519,2544` |
| R15 | medium | Duel stakes come back piece by piece: a failure part-way loses the rest | `server/duel.rs:121-142` |
| R16 | medium | Guild writes are fire-and-forget, never retried, never flushed at shutdown; the Etin return and the house purchase are split from their item and gold change | `guild.rs:208-221`, `server/guild_house.rs:73-90,103-108`, `main.rs:126-132` |
| R17 | medium | The data editor answers anyone on the LAN or Tailscale without a login, and a web page through DNS rebinding | `tools/data-editor.sh:22`, `data-editor/src/serve.rs:41-66` |
| R18 | medium | Player-triggered log lines (per refused move, per unhandled packet with a hex dump, per chat line at debug); the start scripts default to debug | `server.rs:2244`, `server.rs:2463`, `server.rs:2858-2864`, `tools/*.sh` |
| R19 | medium | The in-world idle timer restarts with every packet sent to the player; a kick can be cut off mid-write | `server.rs:1651`, `server.rs:668-680` |
| R20 | low | Auctions take bids and buy-now up to a minute after their end | `market.rs:559`, `market.rs:582` |
| R21 | low | A move with a NaN or huge height is stored, relayed and saved; shop titles skip the chat-markup check; hair, face and size are not range-checked | `shaiya-proto/src/world.rs:330`, `world.rs:239-243`, `zone/my_shop.rs:128-138`, `creation.rs:208-210` |
| R22 | low | `frame::encode` panics on an oversize packet, and some list encoders rely on their callers to stay small (latent; under zone locks) | `shaiya-proto/src/frame.rs:15`, `friends.rs:27` |
| R23 | low | A disband agreed after a deposit strands the item in a dead guild | `guild.rs:1276-1293` |
| R24 | low | A quest reward choice indexes three results by a number loaded without a cap | `server.rs:2179`, `quests.rs:562`, `quests.rs:582` |
| R25 | low | Two gifts between the same two accounts at once lock the accounts in opposite orders; one is aborted (nothing lost) | `pg.rs:3858`, `pg.rs:3868` |
| R26 | low | The in-game console checks the staff level from when the character entered the world | `server/admin.rs:724` |
| R27 | low | A guild agreement everyone agreed to never lapses, blocking its members' guild actions until a restart | `guild.rs:461`, `guild.rs:1350` |
| R28 | low | Lua monster scripts get the whole `os` library and no instruction limit | `mob_lua.rs:180` |
| R29 | low | A handshake error after the claim leaves the session unreleased; the bank count fix-ups at start race the first moves | `server.rs:969`, `server.rs:258-260` |
| R30 | low | The database NOTIFY channel carries console commands and kicks with no secret (matters for the web API) | `main.rs:141-197` |

Kept as they are, with the reason, in section 3.6: N1 pick-up puts the loot back on any error; N2 value that lives only in memory;
N3 the warehouse fee wraps as in the original; N4 a lock order to write down; N5 the monster passes under `maps` and `mobs`; N6
`settle`'s `expect` before the trade partner's reply.

## 3. The findings

### 3.1 Critical

**R1. A client that stops reading wedges its connection.**
- Where: `Connection::send` writes with `self.stream.write_all(&packet).await?` and no timeout (`server.rs:643`). `world_loop` drains
  the player's outbox in a `select!` arm, `Some(packet) = inboxes.packets.recv() => { conn.send(packet).await?; continue; }`
  (`server.rs:1653`). The outbox is `UnboundedSender<Vec<u8>>` (`zone.rs:74`), fed by every broadcast through `Placed::send`
  (`zone.rs:517`). The kick, shutdown and idle checks live only inside `next_packet` (`server.rs:668-680`), which is not polled while
  the arm body waits. `kick_out` sends its packet through the same write (`server.rs:688`).
- How: a modified client enters the world in a busy place and stops reading, keeping its TCP window at zero; the kernel answers the
  zero-window probes, so TCP never gives up. Honest variant: a player's Wi-Fi drops without a reset in a busy area; the send buffer
  fills and the write waits until TCP's retransmission limit, about 15 minutes.
- Effect: the task never returns. The character stays in the zone, visible, and is never saved; `/kick`, bans, `allout`, the newer
  login's kick and shutdown are not seen; the account's `Online` entry is held, so the player's own new login is refused after
  `handshake_wait` (15 s); everything nearby piles into the unbounded queue (tens of KB a second, much more with R4) until the process
  is killed for memory, which runs no shutdown at all. A trade partner or shop buyer waiting on the task stalls 30 s; R8 makes it
  server-wide. At shutdown the 10 s grace runs out and the stuck player's leave-save is lost.
- Fix (decision 3):
  - Write with a deadline: `GAME_WRITE_TIMEOUT_SECS` (default 20). Expiry is an error, handled as a lost connection (linger and
    cleanup).
  - While a write waits, also watch the kick and shutdown: a kick or shutdown abandons the write, closes the socket and goes on as
    `kick_out` does now. A kicked connection has nothing more to say.
  - Bound the outbox by bytes: replace `pub type Outbox = UnboundedSender<Vec<u8>>` with a small type that keeps the sender and a
    shared queued-bytes counter, with the same `send(&self, Vec<u8>)` shape so the call sites do not change. Past `GAME_OUTBOX_LIMIT`
    (default 4 MiB, about 80 s of a busy town) it drops the packet and marks the connection as overflowed. `world_loop` ends an
    overflowed connection as a lost one. The drain subtracts what it writes. The events channel stays unbounded: its events carry items
    and gold, and with the write deadline the task always drains it.
  - Optional: drain up to about 16 KB of queued packets into one write.
- Player-visible: only a client that stops reading for 20 s is disconnected.
- Tests: a game_flow test whose second client stops reading while the first floods zone chat. The reader is gone within the deadline
  plus the linger, and can log in again. A kick reaches a stuck reader at once.

**R2. One address can hold every game-server connection slot.**
- Where: `serve` (`server.rs:215-240`) has only the global semaphore (`GAME_MAX_CONNECTIONS`, 5000) and the console's `maxusers`, both
  counting sockets that never logged in. `handshake` reads with `next_packet(ctx.config.handshake_timeout)`, a 30 s timeout per read,
  and `g::OP_PING => continue` (`server.rs:1003`) does not count against the ten undefined packets.
- How: from one address, open 5000 connections to 30810 and send `04 00 03 00` (a plain-text ping) on each every 29 s. One byte works
  too, since a partial frame restarts the read timeout. Each `0xA301` also costs a database round trip before anything is checked.
- Effect: "connection limit reached" for every real player; a lower `maxusers` makes it cheaper. No account is needed.
- Fix (decision 3):
  - A per-address limit at the game server, `GAME_MAX_PER_IP` (default 8), checked right after `accept` (the stream is dropped when
    over). Copy the login server's `IpLimits` pattern; a `ConnectionGuard` held by the task releases it.
  - A per-address limit on new connections, `GAME_CONNECTS_PER_MINUTE` (default 30), same mechanism at both servers.
  - An absolute deadline on the whole handshake: `timeout(handshake_timeout, handshake(...))` from `accept`. It includes the
    `Online::claim` wait (15 s), which fits inside 30 s.
- Player-visible: none for the real client, which sends `0xA301` at once.
- Tests: the ninth connection from 127.0.0.1 is closed; a pinging socket is closed at 30 s; the existing handshake tests keep passing.

**R3. Free stakes and phantom offers.**
- Where:
  - `trade::prepare` (`trade.rs:356-365`) skips an offer that is no longer in its slot as offered ("deviation 7",
    `docs/original/trade.md` section 5), and `plan` still moves the other side's gold and items.
  - `du::prepare` does the same for duel stakes, and `take_stakes` cuts a gold stake to what the player has,
    `let money = money.min(state.money)` (`server/duel.rs:66`).
  - Nothing stops the items from leaving:
    - `cancel_actions` ends only the trade and the personal shop (`zone/my_shop.rs:89`), so selling, dropping, moving or using a staked
      item leaves the duel window open.
    - `trade_request` checks only the trade book (`zone/trade.rs:47`), `duel_request` only duels, wars and mounts (`zone/duel.rs:154`),
      and the duel arm (`server.rs:2145`) never ends a trade. Taking the stakes (`server/duel.rs:63-97`) removes the items without
      ending a trade.
    - `change_body` (`server/items.rs:885`), `return_etin` (`server/guild_house.rs:61`), a rune revive (`server.rs:4651-4690`) and the
      consuming end of a move by item have no `cancel_actions`.
    - The trade arms have no `state.dead` check.
- How, without an accomplice: A stakes item X and 100M gold against D, then sells X to an NPC and spends the gold. D still sees both
  staked. Both press ready, and A's stakes come out empty while D's are taken. If A wins, A takes D's stakes; if A loses, D gets
  nothing.
- How, with an accomplice C and a victim B:
  1. A opens a duel stake window with C and a trade with B.
  2. A offers X to B and stakes the same X with C.
  3. A and C press ready: X leaves A's bags into the duel, and B is told nothing.
  4. B offers 100M and both press ready: the trade skips X and moves B's 100M to A, "completed".
  5. C walks away, the duel ends, and X comes back to A.
- Effect: deliberate, repeatable theft. The database checks cannot see it: every write is valid, the offer just is not there any more.
- Fix (decisions 2 and 7):
  - (a) `trade::prepare` and `du::prepare` return an error when any offer is gone or no longer matches (type, type id, at least the
    count offered; the uid as well if `ItemUnit` carries one), and a duel stake fails when the gold is short instead of being cut. The
    exchange is then cancelled for both: `0x0A04` with `END_CANCELLED` / `END_CANCELLED_BY_PARTNER` for a trade, the stake window
    closed for both for a duel. Nothing moves. Deviation 7 of `docs/original/trade.md` and deviation 5 of `docs/original/duel.md`
    are rewritten to say so.
  - (b) Trade and duel windows exclude each other: `trade_request` and `trade_answer` answer busy when either side has a duel window
    open, and `duel_request` and `duel_answer` refuse (`NOT_POSSIBLE`) when either side trades or has a personal shop. Opening a shop
    (`0x2301`) closes a duel window in its stake stage.
  - (c) `cancel_actions` also closes a duel window in its stake stage (before the stakes are taken), with the "closed by me" /
    "closed by partner" codes, so every item action that ends a trade ends the stake window too.
  - (d) Add `ctx.zones.cancel_actions(id)` to `change_body`, `return_etin`, the rune revive and the consuming branches of the move by
    item. Refuse `0x0A01`/`0x0A02` while dead.
  - Correct the claim in `docs/original/trade.md` section 6 ("every item action this server has ends the trade first").
- Tests:
  - Unit tests: `prepare` refuses a gone, swapped or smaller offer.
  - game_flow tests for both sequences above: nothing moves, both get the cancel.
  - A stake plus a sale closes the stake window.
  - A duel request during a trade is refused, and the reverse.

**R4. Chat and packet floods.**
- Where: only trade chat (`server.rs:2493`), shout (`server.rs:2509`) and all-server chat have a cooldown. Normal (`2474`), whisper
  (`2478`), zone (`2505`, `send_country` under the global `maps` lock), party (`2516`), guild (`2523`) and union-leader (`2532`) chat
  have none, and there is no per-connection packet rate at all.
- What else costs per packet:
  - Quick slots are saved in a transaction on every `0x010B` (`server.rs:2668`).
  - A friend memo is queued for the writer even when unchanged (`friends.rs:290`).
  - Guild remarks and rank toggles go to the guild writer.
  - Interest-list changes are saved inline.
  - A refused step logs at info and broadcasts a position (`server.rs:2244`).
- How: one account sends zone chat at line rate; each 128-byte line becomes one copy per same-faction player on the map.
- Effect:
  - With R1, memory runs out within minutes.
  - Without it, everyone lags behind the `maps` lock.
  - The chat log fills (on by default, kept forever by default), and the bounded game-log queue drops real records while full.
  - About seven accounts spamming database writes saturate the pool (R11).
- Fix (decision 4):
  - Chat: a token bucket per connection for the chat channels without a cooldown, 3 lines a second with a burst of 5
    (`GAME_CHAT_PER_SEC`, `GAME_CHAT_BURST`). Extra lines are dropped silently, with at most one debug line per connection per minute.
  - All packets: a token bucket per connection, 100 a second with a burst of 200 (`GAME_PACKETS_PER_SEC`). A connection over it for
    5 s in a row is disconnected, with one warn line.
  - No-op writes: skip the quick-slot save when the slots are unchanged, and skip a memo, remark or rank write that changes nothing.
- Player-visible: spam beyond the limit disappears; normal play never reaches either limit. Watch for the client's own bursts (entering
  the world, collect-all in the auction house), which are paced by the server's answers.
- Tests: a burst of 20 lines gives 5 relayed and the rest dropped; 300 packets in a second keep the connection; sustained flooding ends
  it.

**R5. One panic can kill the whole server.**
- Where: 560 `lock().unwrap()` in the game server and no poison recovery anywhere. Every pass and most player actions take `Zones.maps`.
  A panic in `world_loop` unwinds past `in_world`'s cleanup (`server.rs:1555-1621`), so the player stays in the zone, party, guild
  and friends lists, nothing is saved and the session is not released. The background tasks are spawned and forgotten
  (`main.rs:110-116`): a panic ends that task for good (monsters stop, auctions stop ending, the heartbeat stops). The heartbeat
  (`server.rs:371-380`) keeps listing a server whose `maps` is poisoned. `tools/game-server.sh:25` restarts only on exit status 75.
- No player-reachable panic inside a lock was found (section 4); this is about the next bug.
- Fix (decision 6):
  - Poison-tolerant locks: a small extension trait in the game server (e.g. `crate::sync::Relock` with `fn guard(&self)` doing
    `self.lock().unwrap_or_else(PoisonError::into_inner)`, and the same for the `RwLock`), then a mechanical replacement of every
    `.lock().unwrap()` / `.read().unwrap()` / `.write().unwrap()`. No new dependency. A recovered poisoning logs once at error.
  - Connections: run `world_loop` under `AssertUnwindSafe(...).catch_unwind()` (`futures-util`, or an equivalent) in `in_world`, so a
    panic becomes an `Err` and the cleanup and saves run as on a lost connection (no linger); log the panic with the character id.
    `handle`'s release then runs as for any error.
  - Background tasks: a `supervise(name, || task())` helper that restarts a periodic task after a panic (log, wait 1 s, start again):
    the mob updater, heartbeat, market pass, ranking clock, mob-id saver, console countdown and the account notifications. The guild
    and friends writers catch a panic per item instead, so their queue survives.
  - `tools/game-server.sh`: restart after any non-zero exit, not only 75, with a stop after 5 restarts within a minute. The systemd
    unit already restarts on failure.
- Tests: a test-only hook that panics inside a zone lock (or a unit test of the trait) shows the next lock works; a connection that
  panics leaves the zone and releases its session.

**R6. An accept error ends the process without saving anyone.**
- Where: `let (stream, peer) = listener.accept().await?;` (`server.rs:217`, login `server.rs:45`). In the game server's `main`,
  `result = serve(listener, ctx.clone()) => return result` (`main.rs:119`) returns at once. The shutdown path (`main.rs:123-137`:
  closing connections, saving, flushing the game log, saving the mob id) is skipped, and dropping the runtime cancels every connection
  task before its cleanup.
- How: `EMFILE` once the open files exceed the soft limit. Systemd's default for a service is 1024 against `GAME_MAX_CONNECTIONS` 5000,
  so about a thousand connections (R2) do it. `ENFILE`, `ENOMEM` and `ENOBUFS` can do it under memory pressure (R1, R4).
- Effect: every player is dropped and loses what was not saved yet (positions, HP, cooldowns, buffs, quest timers, wear), plus the
  game-log queue. `tools/game-server.sh` does not restart it (exit status not 75).
- Fix:
  - On an accept error, log at warn, sleep (100 ms doubling to 1 s while errors repeat) and go on. `serve` returns only if the
    listener is gone.
  - In `main`, any end of `serve` goes through the shutdown path.
  - `docs/server/deploy.md`: `LimitNOFILE=65536` in both units.
  - The scripts: `ulimit -n 65536` where allowed.
- Player-visible: none.

### 3.2 High

**R7. Password guessing never stops.**
- Where:
  - `record_success` clears the address's whole failure history (login `limits.rs:91-95`); it is called on any accepted login
    (`server.rs:184`).
  - Failures are counted per address only; there is no per-account count.
  - A password may be 1 character (`accounts.rs:49`).
- How: from one address, 9 wrong guesses per connection (the per-connection cap is 10), and before the twentieth failure one good login
  with an account the attacker registered (registration will be open). The count is cleared and guessing goes on. Argon2 runs four at
  a time (`accounts.rs:19`), about 100 to 250 guesses a second server-wide. Many addresses against one account are not limited at all.
  The FIFO hash semaphore also queues honest logins behind the attacker.
- Fix (decision 5), in `limits.rs` and `server.rs`:
  - A per-(account, address) failure count: 5 in 15 minutes → that address is answered "wrong password" for that account for 15
    minutes without hashing.
  - A per-account count over all addresses: 30 an hour → the account is answered the same way from every address until the hour
    clears. Accounts are keyed by the lowercased name, unknown names included, so the answer does not reveal which names exist. The
    map is bounded: entries expire, and there is a cap of about 100,000 with the oldest dropped.
  - `record_success` clears only its own account's entries, not the address's.
  - The address lockout (20 failures in 10 minutes) stays as it is.
  - `validate_credentials` requires 6 to 12 characters for a new password (`create_account`, and later the web API's registration and
    password change). Existing accounts with shorter passwords still log in.
- Player-visible: someone who mistypes five times waits 15 minutes on that address; new passwords need 6 characters.
- Tests: unit tests in `limits.rs`; a login_flow test where one account's failures from 127.0.0.1 do not lock another account out and
  a good login of another account does not clear them.

**R8. The auction-house and guild-bank turns are held across client writes.**
- Where:
  - The auction requests take `ctx.market.turn` (`server/market.rs:84`, `90`, `104`). `finish` (`server/market.rs:115-142`) then
    writes to the database and sends the answers with `conn.send(...).await` while holding it. The end pass needs the same turn
    (`server/market.rs:180`).
  - The guild bank takes one global `guild_bank_turn` (`server/guild_bank.rs:23`) and sends while holding it (`35`, `70`).
- How: a player in the auction house (or in a guild house beside the warehouse keeper) stops reading, as in R1, and sends one request;
  its write blocks while the turn is held.
- Effect: every auction request on the server blocks, and so does every connection that makes one (its whole task waits on the turn).
  Auctions stop ending. The guild bank freezes the same way for all guilds. Nothing is lost.
- Fix: keep the plan, the database write and the in-memory apply under the turn. Collect the packets to send into a `Vec`, drop the
  turn, then send them in order. (The R1 write deadline bounds it as well, but a turn should never wait on a client.) Optional: one turn
  per guild for the bank.
- Player-visible: none.

**R9. The handshake takes the account's slot before checking the identity.**
- Where: `Online::claim(&ctx.online, user_id, ...)` (`server.rs:1020`) runs before `ctx.store.claim` (`server.rs:1025`), which is where
  the identity, server, address and expiry are checked. Every `Online` drop wakes every waiting handshake through one shared `Notify`
  (`left`, `server.rs:795`; `notify_waiters`, `server.rs:950`).
- How:
  - User ids are sequential. Many bogus `0xA301` with the victim's id each hold the victim's slot for one database round trip, and
    the victim's real handshake must win each wake-up race against them. It has 15 s of rounds; this is probabilistic.
  - Waiters are free: present the id of any online account and wait 15 s. Every drop wakes all of them, and each one locks the global
    `OnlineUsers` mutex, so a few thousand waiters times many drops a second load every worker.
- Effect: probabilistic denial of one account's login, and a CPU herd for everyone. A session takeover is not possible: the claim
  requires the identity and, by default, the login address.
- Fix:
  - Check the session first with a read-only query (the claim's conditions without the update), then `Online::claim`, then the
    claiming `UPDATE` as now.
  - Wake only the account's own waiters: one `Notify` per waiting account, kept beside the entry and removed with it.
  - R2's limits bound the rest.
- Player-visible: none.

**R10. Login server: pre-login sockets held for ever; RSA on the async threads.**
- Where: reads time out per read (`server.rs:108`), and `(login::OP_PING, _) => {}` (`server.rs:136`) is accepted in every phase,
  the key exchange included. `key.decrypt_le` (`server.rs:144`, `shaiya-crypto/src/rsa.rs:50-56`) is a full 1024-bit `modpow`, without
  CRT, on the async worker.
- How:
  - Holding: 20 connections per address (`LOGIN_MAX_PER_IP`), each pinging every 29 s before the key exchange or every 599 s after it.
    About 250 addresses fill the 5000 slots, and nobody can log in.
  - CPU: churning connections, each costing one decrypt (around a millisecond), with no limit on new connections per address.
- Fix (the defaults in section 1):
  - The key exchange within `LOGIN_KEY_TIMEOUT_SECS` (30) of `accept`.
  - The login within `LOGIN_IDLE_SECS` (600) of the key exchange.
  - Pings extend neither; after the login the per-read idle stays.
  - `LOGIN_CONNECTS_PER_MINUTE` (default 30), as at the game server.
  - The decrypt in `spawn_blocking` behind a small semaphore, like the password hash. CRT is optional (about four times faster; the
    key pool would keep p and q).
- Player-visible: a player who sits at the login screen for more than 10 minutes without logging in is disconnected (as today with no
  pings).

**R11. Eight database connections for everything; the monster tick waits on the database.**
- Where: `PgPoolOptions::new().max_connections(8)` (`main.rs:25`). `PgListener::connect_with(&pool)` (`main.rs:147`) keeps one of them
  for good, leaving seven for about 100 players, the game-log writer, the guild and friends writers, the market pass, the heartbeat and
  every handshake. `mob_updater` awaits `obelisk_events` (`server.rs:552`, which saves owners) and `kill_count_changes` (`server.rs:575`,
  which saves counts) inside the tick. sqlx's default acquire timeout is 30 s.
- How: R4's write packets, a flood of handshakes (R2), or plain load at the alpha's peak.
- Effect: every save queues. Past the acquire timeout the `?`s end players' connections, some mid-change (R14, R16). The monster
  passes, invitation and trade expiry freeze while the tick waits. A late heartbeat makes the login server list the server as closed.
- Fix:
  - `GAME_DB_POOL` (default 24; PostgreSQL allows 100 by default).
  - The listener on its own connection (`PgListener::connect(&url)`).
  - An explicit acquire timeout, `GAME_DB_ACQUIRE_SECS` (default 10).
  - The tick hands the obelisk and kill-count saves to a spawned task (in order, one queue) instead of awaiting them.
  - The login server's pool to 16.
- Player-visible: none.

### 3.3 Medium

**R12. PvP rewards can be claimed twice.**
- Where:
  - The death reward (`0x021A`, `server/pvp.rs:193-199`) saves the gold with `ctx.store.trade` and then the step with
    `save_kill_steps`: two transactions.
  - The kill reward (`0x0219`, `server/pvp.rs:174-178`) saves the stat points with `save_progress` and then the step.
  - The stat reset (`server/items.rs:730-736`) has the same shape, but there a failure only loses the re-claim.
- Effect: if the second write fails (a database error or pool timeout; `?` ends the connection), the gold or points are saved and the
  step is not, so the reward is offered again at the next login. A player cannot force the failure.
- Fix: one store call per reward that writes both in one transaction, conditional on the old step:
  `UPDATE characters SET money = money + $2, dead_level = $3 WHERE id = $1 AND dead_level = $4 AND money + $2 <= 4294967295`, and the
  same with `stat_point` and `kill_level`. Check the row count. Do the same for the stat reset's progress and step. `memory.rs` gets
  the same methods.
- Player-visible: none.

**R13. F1 remainder: effects before the item is used up.**
- Fixed since F1: Fame, the stat and skill resets and Repair All call `consume` first; the name change is atomic.
- Still effect-then-consume: these arms return `Done::UsedUp`, and only then does `use_up` (`server/items.rs:248-250`) consume the item
  in its own transaction, ending the connection if refused.
  - Blessing (`212-222`, server-wide and saved).
  - Item buff skills (`114-118` and `item_skill`, saved at leave; mall buffs are long).
  - The experience wheel (`465-478`, `save_progress` committed first).
  - Moves by item (the move happens, then the use-up).
  - Target resurrection (`193-199`).
  - Recover and Cure (`121-128`).
  - The Message To Server (`226-229`).
- Effect: a database failure at the consume leaves the effect and the item, for example a free Blessing, mall buff or teleport.
- Fix:
  - For Blessing, item buffs, the experience wheel and moves by item: run the refusal checks, then `consume`, then the effect, as Fame
    does. Blessing needs `add_grade` split into a check and an apply under one lock, the check repeated at the apply.
  - The transient ones (Recover, Cure, resurrection of another player, the Message To Server) may stay; the risk is a free potion on a
    failure that ends the connection anyway. Say so in a comment.
- Player-visible: none (on a failure the item is now lost instead of kept).

**R14. Party loot and party gold can vanish.**
- Where:
  - The zone takes a party drop off the kill and sends it to a member as `ZoneEvent::Loot` or `ZoneEvent::Gold` (`zone.rs:2519`,
    `zone.rs:2544`), ignoring a send error.
  - `party_loot` does `into_bags(...).await?` (`server.rs:4448`) with nothing put back on an error.
  - Its packets are sent before the part that did not fit goes on the ground, so a write error loses that part too.
  - The `Gold` arm bails on a refusal (`server.rs:1687`).
  - When `world_loop` ends, `in_world` drains the events but keeps only `Record` (`server.rs:1583`), and the member is still in the
    zone until `ctx.zones.leave` a few lines later, so a drop handed to a member who is logging out is dropped.
- Effect: a monster's item or gold disappears for the party. Rare, but silent.
- Fix:
  - On an `into_bags` error, put the loot on the ground where it dropped (`drop_on_ground` with its owner and party, kind put-back),
    then return the error.
  - Put the rest on the ground before sending packets.
  - On a refused or failed gold save, put the gold on the ground for that member.
  - In the exit drain, put `Loot` on the ground at its position and the `Gold` on the ground at the member's position.
- Player-visible: none.

**R15. Duel stakes come back piece by piece.**
- Where: `put_back` (`server/duel.rs:121-142`) saves the gold in one transaction, then each item with its own `into_bags(...).await?`.
- Effect: a failure part-way (the database, the pool) loses the items not yet saved. They were taken out of the database when staked
  and live only in memory until then (N2).
- Fix: plan every item on a scratch copy of the inventory (`pick_up` per item in order) and save the gold and every change in one
  `ctx.store.trade`; apply to the real inventory after the commit; what does not fit goes on the ground as today. On an error, put
  everything on the ground for its owner before returning it.
- Player-visible: none.

**R16. Guild writes are fire-and-forget.**
- Where: `guild::run_writer` (`guild.rs:208-221`) logs a failed write and drops it, and so does `friends::run_writer`
  (`friends.rs:100`). Neither is flushed at shutdown: `main.rs:126-132` flushes only the game log.
- Split from their own change:
  - The Etin return removes the items in a transaction (`server/guild_house.rs:73-76`), then `guild_return_etin` (`:90`) raises the
    Etin in memory and queues the write. It credits nothing when the player left or was kicked from the guild during the await. An
    Etin item with special 0 counts 0 and is destroyed for nothing.
  - The house purchase takes the gold in a transaction (`:103-106`), then `guild_house_bought` (`:108`) queues `HouseBought`.
    Nothing is bought when the guild was disbanded meanwhile.
- Effect: after a failed or unflushed write, the guild's memory and the database differ until the restart, which then loses
  membership, ranks, Etin, NPC levels or a paid house.
- Fix:
  - The Etin return and the house purchase each become one store transaction: the items or gold, plus the guild's Etin and return
    count or the house flag, conditional in SQL on the player still being a member (or the master) of that guild. The in-memory guild
    is updated after the commit. Refuse the Etin return before removing anything when the player has no guild, and leave Etin items
    worth 0 in the bags.
  - Both writers retry a failed write three times (0.2 s, 1 s, 5 s), then log it at error with the write.
  - Both take a `Flush(oneshot)` item that `main` sends after `shut_down` and awaits with the grace.
- Player-visible: none.

**R17. The data editor answers anyone on the LAN.**
- Where: `tools/data-editor.sh:22` binds `0.0.0.0:30890`. The guard (`crates/data-editor/src/serve.rs:41-66`) allows loopback,
  private, ULA and Tailscale addresses, and for writes wants `X-Editor: 1` plus a JSON content type. There is no login and no `Host`
  check.
- How:
  - Anyone who reaches the box over the LAN, Tailscale or a VPN gets the editor, with apply, undo, reload and restart.
  - A web page the operator opens while the editor (or its SSH tunnel) is up can rebind its own host name to 127.0.0.1. Its requests
    are then same-origin, so it can set the header.
- Fix (decision 8):
  - The script binds `127.0.0.1:30890` by default (the program's own default already is).
  - The guard refuses any request whose `Host` is not `localhost`, `127.0.0.1` or `[::1]` (any port), or a name listed in
    `EDITOR_HOSTS`.
  - `docs/server/deploy.md` and the plan say to use an SSH tunnel.
  - `tools/checklist_server.py` gets the same `Host` check (dev tool).
- Player-visible: none.

**R18. Player-triggered log lines.**
- Where:
  - Info per refused step (`server.rs:2244`) and per unsafe-markup chat line with its hex (`server.rs:2463`).
  - Debug per unhandled packet with the whole payload in hex (`server.rs:2858-2864`: about 4 KB of log for a 2 KB packet) and per
    chat line (`server.rs:2468`).
  - `tools/game-server.sh:23` and `tools/login-server.sh:20` default to `game_server=debug` / `login_server=debug`, appending to a file
    without rotation. The systemd units in `docs/server/deploy.md` use `RUST_LOG=info` and the journal, which rotates.
- Effect: a cheap packet stream fills the disk on a server started with the scripts, and each line is a synchronous write on a worker.
- Fix:
  - The refused step and the unsafe-markup line go to debug.
  - The hex dumps are cut to the first 64 bytes.
  - A per-connection limit of one such line per kind per 10 s.
  - The scripts keep debug (they are the dev box's), but `docs/server/deploy.md` says the alpha runs under systemd at `info`.
  - `GAME_LOG_DAYS` and a `login_log` purge go to the operations step (B2).
- Player-visible: none.

**R19. The idle timer restarts with every packet sent to the player; a kick can be cut mid-write.**
- Where: `packet = conn.next_packet(idle)` (`server.rs:1651`) is a new future, with a new timer, every time another `select!` arm fires,
  so a silent client in a busy area is never idled out. `kick_out` runs inside `next_packet` (`server.rs:675`), which the outer
  `select!` can drop part-way through its write. The client then gets half a packet followed by the next one (it is being kicked
  anyway), and the kick is redone.
- Fix:
  - `Connection` keeps `last_read`; `next_packet` waits with `timeout_at(last_read + idle)`.
  - `next_packet` returns the kick to its caller instead of performing it, and `world_loop` (and the handshake and character screen)
    call `kick_out` in the arm body, where nothing cancels it.
- Player-visible: none.

### 3.4 Low

**R20. Auctions take bids and buy-now after their end.** `buy` and `bid` (`market.rs:559`, `582`) never look at `ends_at`; auctions end
in the pass every 60 s. Fix (decision 7): refuse with result 1 (closed) when `ends_at <= now`. A refused end write ("tried again next
pass") no longer keeps an auction open.

**R21. Inputs.**
- Heights:
  - `world::parse_move` (`shaiya-proto/src/world.rs:330`) accepts any float. `World::is_safe_pos` (`world.rs:239-243`) checks x and z
    (NaN fails the range) but not y, so a NaN or 1e38 height is stored, relayed to the view, saved and copied into save points.
  - The same holds for `item::parse_save_point` and the admin `parse_go`, `parse_summon_to` and `parse_guild_go`.
  - No server panic follows.
- Shop titles: `my_shop::parse_start` keeps up to 120 bytes, and `shop_open` (`zone/my_shop.rs:128-138`) shows them to the view
  without `chat::markup_is_safe`, which chat, memos, remarks and Lua lines all pass. If the sign is drawn by the same code as chat, a
  `<<999c01` title crashes every client in view (client queue entry 13 asks).
- Looks: hair, face and size from `0x0102` (`creation.rs:208-210`) and `0x0226` are stored as sent, as in the original
  (`docs/original/ps_game.md` line 335).
- Fix (decision 7):
  - The float decoders refuse non-finite values; `is_safe_pos` also wants `-10_000.0 <= y <= 10_000.0`.
  - A title failing `markup_is_safe`, or holding a byte below 0x20, is refused: the shop does not open and nothing is sent, as when
    `0x2303` is refused today.
  - Hair, face and size are clamped once client queue entry 13 gives the ranges. Until then they stay as they are, with a `TODO`
    naming the entry.

**R22. `frame::encode` panics on an oversize packet.**
- Where: `assert!(length <= MAX_PACKET_LEN, ...)` (`shaiya-proto/src/frame.rs:15`). Many encoders are built inside zone locks, where a
  panic poisons (R5).
- Bounded today only by their callers:
  - `party::list`: at most 2,012 bytes of 2,044. The margin goes if `MAX_MEMBERS` or `MAX_UNIT_BUFFS` grows.
  - The friend and block lists: 25 a packet, memos at most 50 bytes, but only because `valid_memo` (`friends.rs:27`) checks player
    input. A memo of more than 50 bytes written by a tool would overflow at 9 such friends, inside the friends lock, at every login.
  - `market::search`'s refresh path: no `PAGE` cap; it relies on auction ids rising.
  - The chat, notice and guild-ranking encoders.
- Fix:
  - `encode` stops panicking in release: `debug_assert!`, and in release an oversize payload is logged at error and cut to 2,044
    bytes.
  - The friend and block encoders cut each memo at 50 bytes.
  - The market refresh truncates to `PAGE + 1`.
  - The harness (section 5) checks every list encoder at its largest input.

**R23. A disband agreed after a deposit strands the item.** `disband_request` refuses a non-empty bank (`guild.rs:1252`), but
`disband_answer` (`guild.rs:1276-1293`) fires without checking again. A deposit made during the vote stays in the rows of a dead guild,
out of the players' reach. A bank move whose write commits while the guild goes falls into `bank_apply`'s fallback, which is right only
for a deposit. Fix (decision 7): the disband paths take `guild_bank_turn` first, and `fire` on the agreement path refuses
(`DISBAND_FAILED`) when the bank holds items.

**R24. A quest reward choice indexes by an uncapped number.** `0x0907` checks `index < def.user_select` (`server.rs:2179`), then
indexes `def.results[result]`, a `[QuestResult; 3]` (`quests.rs:562`, `582`). `user_select` is loaded without a cap. It is at most 3
in today's data, but a larger value would panic the connection (R5's ghost). Fix: clamp it to 3 at load with a warning; use `get`.

**R25. Two gifts between the same two accounts at once.** `gift` locks the target account (`pg.rs:3858`), then takes the buyer's
points (`pg.rs:3868`). A gift each way at the same moment deadlocks; PostgreSQL aborts one, which rolls back and ends that connection.
Fix: lock both account rows in id order first (`SELECT ... WHERE id IN ($1, $2) ORDER BY id FOR UPDATE`).

**R26. The in-game console uses the staff level from entering the world.** `(1..=ctx.config.console_level).contains(&state.staff_level)`
(`server/admin.rs:724`). `state.staff` and `state.staff_level` are fixed at entry (`server.rs:1431-1434`). A staff member demoted by
`F601` to another staff level keeps the console until they relog. Demotion to a player is safe: `admin::check` reads the live
privilege. Fix: check `player.privilege`, and update `state.staff`, `state.staff_level` and the occupant's staff flag when `F601`
applies.

**R27. Agreed guild agreements never lapse.** Leaving the world (`guild.rs:461`) and the one-second pass (`guild.rs:1350`) end only
agreements with `!a.agreed`. If the founder's connection never handles `ZoneEvent::GuildAgreed` (it left at that moment, or R1), the
agreement stays, and `in_agreement` refuses its members' guild actions until a restart. Fix: an agreed agreement lapses too (a due time
of 30 s after agreeing), and ends when its founder leaves the world.

**R28. Lua scripts get the whole `os` library and no instruction limit.** `mlua::Lua::new_with(StdLib::MATH | StdLib::OS, ...)`
(`mob_lua.rs:180`). The scripts come from `world.mob_lua` (imported data; the data editor cannot edit them). Of the eight scripts, one
uses `os.time` and nothing else. A script with a bug that loops for ever hangs the monster tick under `maps` and `mobs`. Fix: load
`MATH` only and set a global `os` table with `time` alone; set an instruction-count hook (every 10,000 instructions, an error past
about 1,000,000 a call) and log the script that hit it.

**R29. Two start and end gaps.**
- An error in `handshake` after the session was claimed (the reply send, `faction`, `send_list`) returns through
  `handshake(...).await?` (`server.rs:969`) without `ctx.store.release`. The heartbeat then renews the session row. It does not lock
  the account out, since the next login replaces the row. Fix: release in that path, for example with a guard that releases unless
  disarmed.
- The bank count fix-ups at start (`server.rs:258-260`) are queued to the writer and may race a member's first bank move. Fix: await
  them before `serve`.

**R30. NOTIFY is an unauthenticated control channel.** The game server carries out any `game_console` NOTIFY for its server id (quit,
restart, allout, reload, exp, kick) and any `account_login` / `account_points` notification (`main.rs:141-197`). PostgreSQL needs no
privilege to NOTIFY, so any role that can connect to the database can do it. Not a fix in this audit. The planned web API gives its role no
access to this database (or no NOTIFY-capable connection) and decides whether the console payloads carry a secret.

### 3.5 For the planned web API

R7's 6-to-12 password rule applies to registration and password changes. R30 decides the API's database access. R17's editor access
may fold into the API's staff login. Rate limits for registration belong with R7's limits.

### 3.6 Kept as they are

- **N1. Pick-up puts the loot back on any error.** `server.rs:4319-4323` and `4336-4340` put the item back on the ground on `Err` as
  well as on a refusal. If the connection to PostgreSQL drops after the server committed, the item is both saved and back on the ground.
  Putting back only on a refusal would lose the item in the far more common case where the error happened before the commit. Kept.
- **N2. Value that lives only in memory.** Duel stakes (taken out of the database at `take_stakes`), gambling-table money and ground
  items are lost by a crash or a `kill -9`; a graceful shutdown returns stakes and table money. Accepted for the alpha. A "held" table
  is possible later.
- **N3. The warehouse withdrawal fee wraps** (`inventory.rs:260`, `wrapping_mul`) as the original's 32-bit product does. Kept.
- **N4. Lock order.** `fields` (`zone.rs` ~3768) calls `damage_mob`, which takes `ground`, while `expire_instances`
  (`zone/instance.rs:113-127`) takes `ground` then `fields`. Both hold `maps` first, so no deadlock is possible. Write the order down in
  `Zones`' doc comment: maps, instances, mobs, npcs, ground, fields, then parties, then guilds or friends; the others are leaves.
- **N5. The monster passes hold `maps` and `mobs`** every 100 or 500 ms over every map. Measure the pass time at the alpha (operations
  step) before splitting anything.
- **N6. `settle`'s `expect`** (`server.rs:3966`) cannot fire (the plan checks the money), but it runs before the parked partner's reply.
  Part 2 sends the reply first.

## 4. Checked and found sound

So the next audit need not redo these (all at the audited commit):

- **Frames and decoders.**
  - `FrameDecoder` rejects lengths below 4 and above 2048 before handing out a frame, so `opcode` and `payload` cannot panic. The
    buffer stays at about 4 KB, since both servers read only when no whole frame is buffered.
  - All of shaiya-proto's client decoders check lengths before indexing (`get`, slice patterns, exact lengths; the three
    `try_into().unwrap()` follow exact-length checks). None allocates from a client count except `parse_quick`, which is bounded.
  - Neither server indexes a payload directly; the one inline decode (`0x0B10`, `server.rs:2008`) uses `first()`.
  - shaiya-crypto: `decrypt_le` refuses a ciphertext not below the modulus, `CtrStream` takes any length, and `XorTable::apply`'s
    bound holds for every sendable packet. There is no `unsafe` in proto or crypto.
- **Encoders.** Every list a player can influence stays within the packet today. Worst cases:
  - `char_skills` 2,041 and the party list 2,012.
  - `server_list` 1,825, the union lists 1,820, the active quests 1,786, `char_items` 1,701, the guild list 1,681.
  - The bank lists 1,655 and the market lists up to 1,921.
  - The friend lists 1,951 and the world notice 2,002.
- **Login.**
  - Only the handshake, ping and terminate are accepted before the key; the key reply must be exactly 128 bytes, and the phases advance
    only in order.
  - Credentials are cut at 12 bytes or the first zero; non-UTF-8 is treated as an unknown account.
  - The account lookup uses the lowercase index.
  - Argon2 runs in `spawn_blocking` behind a 4-slot semaphore.
  - No account enumeration by timing or answer: an unknown account is checked against a dummy hash and gets the same answer and log row.
- **Sessions.**
  - The claim is one `UPDATE` requiring the identity, the server, the expiry, an unused session and (by default) the login address.
    Two connections with the right identity are serialized by `Online` on one server and by the row lock across servers.
  - `release` deletes only its own identity's row.
  - A newer login's notification is sent in the same statement as the session write and kicks only an older claim. The linger (10 s)
    is shorter than `handshake_wait` (15 s).
- **Character screen.**
  - Create checks the slot, grow, race and class tables, the race against the faction, the sex, and a 2-to-18 ASCII name, unique
    without case among live characters (partial unique index).
  - Five live characters per account at most, by index.
  - Select, delete, restore and rename all filter on the account and server; select also on not deleted and no pending deletion.
  - A rename refuses a character still in the world and is one transaction.
  - One select per list; the faction only with no live characters.
- **Cleanup.** `in_world` has no `?` before its cleanup, and `world_loop`'s errors are captured, so the cleanup and saves run on every
  error path (not on a panic: R5). `Online` and `PlayerCount` are released by `Drop` on every path. One task per connection, nothing
  else spawned per connection.
- **Saves.**
  - Every multi-row write is one transaction: trade, the guild bank, stakes, the exchange, the market, the mall, guild creation,
    quests, body change, rename, learning, resets, quick slots, applied skills.
  - Money is always `money + $2 BETWEEN 0 AND 4294967295` (or `money >= $2`), with the row count checked.
  - Drift between memory and the database ends in a refusal and a disconnect, not wrapped gold.
  - `trade`'s `SetCount` and `Remove` are unconditioned writes. They are safe because each character's rows have one writer (its own
    connection; the account bank is exclusive through `Online`, linger included). The cross-character writes happen only while the
    other side's task is blocked (the trade's parked side, the shop buyer). The guild bank is serialized by its turn.
- **Exchanges.**
  - Trade: the step to exchanging happens once under the lock. No slot can be offered twice, and counts are checked at offer and at the
    exchange. Gold is checked at offer, at the plan and in the database. Trading with oneself is refused. Trade and shop packets end
    each other. One transaction covers both sides. The partner leaving or lingering is handled.
  - Personal shop: purchases queue to the seller's connection and are handled in order; the listing and the bag are checked again; the
    total uses `checked_mul`; the queue drops when the shop ends. Two shops buying from each other only wait 30 s.
  - Duel stakes are taken in one transaction and held exactly once.
  - Gambling saves every change at once as a range-checked delta, and the table is handed back on every way out.
  - Pick-up: F2 is fixed (put back on refusal), and the ground removal is atomic.
  - The auction house: the turn serializes plan to apply; every step checks its rows; collect-all and double clicks are refused the
    second time; commissions do not overflow; refunds are in the same transaction.
  - The item mall: points taken conditionally in the purchase's transaction; storage to bags matches slot, item and count.
- **NPC shops, the warehouse and item changes.**
  - Counts, prices (in u64 or checked), bag and slot bounds, the equipment bag, and stacking against `max_count`.
  - Lapis, repair, boxes, body change and name change are each one transaction with their price.
  - Learning and stat points check their points.
  - Quest ends update `AND NOT finished`, so no double reward.
  - Teleport scrolls re-check the item at the move.
- **Locks.**
  - The orders observed form no cycle (N4). No mutex is locked again on one path.
  - No `block_on`, `spawn_blocking` misuse, `thread::sleep` or file access in async code.
  - Every cross-task wait (trade, shop, mutual buy) is time-bounded with a withdraw.
  - Every index or `expect` on a client id inside a lock follows an existence check; every division is guarded.
- **Staff.**
  - Every opcode from `0xF000` goes through `admin::check` with the live privilege; a player is disconnected.
  - No staff action hides behind a player opcode.
  - The in-game console needs a staff level within `GAME_CONSOLE_LEVEL` (R26 aside).
- **Guild and party.** The rank checks hold for upgrade, purchase, remark, disband, kick, rank changes, invites, join answers and the
  bank. Party and union invites, kicks and slots are checked. Every list is chunked within a packet.
- **Queues.** The game-log queue is bounded (10,000, `try_send`, drops counted). Invitations, trades and guild invitations expire; user
  questions are 3 per character; interest lists 10; friend and block lists 100.
- **Mail (`1Dxx`):** no code; the opcodes reach the "not handled" arm.

## 5. The fuzz harness (proptest)

Goal: random and hostile input on every run of `cargo test`, catching panics, hangs and duplication, on the stable toolchain.
`proptest` is added as a dev-dependency of shaiya-proto, shaiya-crypto, login-server and game-server only (take the current 1.x that
`cargo add --dev proptest` picks); shaiya-proto keeps no runtime dependency. Commit the `proptest-regressions/` files proptest writes:
they are the failing cases found.

### 5.1 Decoders: `crates/shaiya-proto/tests/decoders.rs`

- **No panic, any input:** every client-to-server decoder in the table of 5.4 gets
  `prop_oneof![vec(any::<u8>(), 0..64), vec(any::<u8>(), n-1..=n+1), vec(any::<u8>(), 0..=2044)]`, where n is the length the decoder
  accepts (the middle arm reaches the accepting path). A small macro keeps it to one line per decoder. `parse_use` takes the opcode as
  well: try `0x050A`, `0x0557`, `0x0559`.
- **Round trips** where a builder exists: `login_request`, `handshake_response`, `select_server_request`, `game_connect_request`,
  `char_create_request`, `char_id_request`, `move_request`, `item_move_request`, `portal_request`, `gatekeeper_request`, `learn_request`,
  `use_request`, `shop::buy_request`, `shop::sell_request`, and `char_quick` against `parse_quick`. Check
  `parse(&build(&x)) == Some(x)` for generated x (with finite floats once R21 is in).
- **The frame layer:**
  - Any byte stream fed to `FrameDecoder` in arbitrary chunks never panics.
  - Every frame returned is 4 to 2048 bytes, and its length field equals its length.
  - After an error the stream stops.
  - The frames concatenated are a prefix of the input.
  - `encode(op, p)` for `p.len() <= 2044` comes back whole.
- **Encoders at their largest input** (after R22):
  - The party list with 6 others and 40 buffs each, the union lists, the friend and block lists (25 entries, and memos of 255 bytes
    to show the cut), the guild member, request and ranking lists, the market lists at `PAGE + 1` rows, `char_skills` with 255 skills,
    the quest list and `world_notice`: each at most 2,048 bytes.
  - `encode` with a 3,000-byte payload does not panic in release.
- **Text helpers:** `markup_is_safe`, `chat::text`, `parse_whisper`, `clean_name` and `name_field` never panic on any bytes.

### 5.2 Crypto: `crates/shaiya-crypto/tests/fuzz.rs`

`CtrStream::apply` on any length round-trips. `decrypt_le` on any 128 bytes never panics, with the fixed test key of
`tests/known_answers.rs` (key generation is slow). `XorTable::apply` on any packet up to 2,048 bytes.

### 5.3 Packet dispatch: `crates/game-server/tests/fuzz_dispatch.rs`

The game server is driven over loopback TCP with `MemoryBackend`, as `tests/game_flow.rs` does (there is no in-process entry:
`world_loop` takes a `Connection` around a real `TcpStream`).

- **Shared helpers:** move `config`, `data`, `data_with`, `set_mobs`, `session`, `start_with_data` and its wrappers, `Client` (with
  `connect`, `start_encryption`, `write`, `read`, `enter`, `finish_enter`, `read_list`), `wait_gone`, `backend_with_session`, `create`,
  `read_zone_entry`, `enter_world`, `select_and_enter` and `step` from `tests/game_flow.rs` into `tests/common/mod.rs`. Both test files
  `mod common;`. No change in what they do.
- **Panic detection:** a `std::panic::set_hook`, installed once (`OnceLock`), counts panics in a static `AtomicUsize` and prints the
  payload and location. Tokio catches task panics, so without it nothing would fail. Each case asserts the count did not change.
- **The server for fuzzing:**
  - `config(true)` with `console_level: 0`, so random GM chat cannot quit the server.
  - `max_connections` 500, short timeouts.
  - `data_with(true)` plus two monsters near the start: merchants, gatekeepers, the warehouse keeper, the blacksmith, the gambling NPC,
    portals.
  - One server per test function. Each case uses fresh user ids (`backend.add_session`) and fresh characters.
- **The packet strategy:**
  - Opcode: 70% from the client opcodes of 5.4, 10% from the unused numbers of those families, 20% any `u16`.
  - Payload:
    - 40% random bytes of the decoder's exact length.
    - 15% a field-aware payload for the hot paths:
      - bag from {0, 1, 2, 5, 6, 100, 255}, slot from {0, 1, 23, 24, 239, 240, 255};
      - counts {0, 1, 2, 254, 255};
      - ids {own, the other character's, 0, 1, `u32::MAX`};
      - money and prices {0, 1, `u32::MAX`};
      - floats {0, -1, NaN, inf, 1e38, a valid spot};
      - the hot paths: item move, use, drop, pick-up, buy, sell, trade, duel, shop, party, guild, quest, market, mall.
    - 30% random bytes of length 0 to 64.
    - 15% random bytes of length 0 to 2,044.
  - A case is a vector of 1 to 40 such packets. The client reads whatever comes back without asserting on it, and stops when the
    server closes; a close is a normal outcome (unknown `0x01xx` opcodes, malformed chat, off-map moves and player `Fxxx` all close).
- **Properties:**
  1. `one_player_survives` (64 cases by default): enter the world, send the case, then check:
     - no panic was counted;
     - a probe client on another fresh account can still enter the world and gets an answer to a step (so no zone lock is poisoned or
       held);
     - the fuzzed account is gone within linger plus a margin (`wait_gone`: the cleanup ran and the session was released).
  2. `two_players_conserve_value` (32 cases, 60 packets interleaved between the two):
     - Setup: two characters on the same spot, away from the merchants and the monsters, each given gold and some stackable and
       unstackable items in the backend before entering.
     - Weighting: toward trade (`0x0Axx`), duel (`0x24xx`), shop (`0x23xx`), party (`0x0Bxx`), drop and pick-up, and item and
       warehouse moves, with each other's ids and their own occupied slots.
     - Check, once both are gone: the gold of both plus the gold on the ground is at most the starting total, and for each
       (type, type id) the count in both characters' bags and warehouses plus the ground is at most the starting count.
     - This needs a test accessor for the ground (for example `#[doc(hidden)] pub fn ground_items(&self, map: u16) -> Vec<Loot>` on
       `Zones`) if none exists.
  3. `before_the_world_survives`: random packets before the handshake and on the character screen only. No panic; the connection ends
     by the handshake deadline or the idle; the probe still enters.
  4. `staff_dispatch_survives`: as 1, with a privilege-1 account, so the admin decoders and handlers run (the console stays off).
- **Run time:** the default `cargo test` run of this file stays under about 30 s; trim the case counts if not. Each property also has
  an `#[ignore]` long twin for deep runs:

```bash
PROPTEST_CASES=5000 ~/.cargo/bin/cargo test --release -p game-server --test fuzz_dispatch -- --include-ignored
```

- **What it does not catch:** R3 is theft, not duplication (the totals are kept), so it has its own regression tests in part 2; R1 and
  R2 are timing, with their own tests in part 1.

### 5.4 Decoder inventory (client to server)

All `fn(&[u8]) -> Option<T>` in shaiya-proto unless noted. The opcodes are those each decoder serves.

| module | decoders (opcodes) |
|---|---|
| login | `parse_handshake_response` (A101), `parse_login_request` (A102), `parse_select_server` (A202); ping 0003 and terminate have no payload |
| game | `parse_game_connect` (A301, plain text), `parse_char_create` (0102), `parse_char_id` (0103, 0104, 010F; in the world 0301 to 0305, 0307), `parse_char_country` (0109), `parse_char_rename` (010E), helper `clean_name` |
| world | `parse_move` (0501), `parse_npc_use` (0E04), `parse_motion` (0506), `parse_item_move` (0204), `parse_portal` (020A, 0227), `parse_status_up` (0208), `parse_gatekeeper` (020C), helper `enchant_level` |
| skill | `parse_learn` (0209), `parse_use` (0511, 0517), `parse_quick` (010B) |
| combat | `parse_attack` (0502 player, 0503 monster), `parse_rebirth_request` (0553) |
| chat | `parse_message` → `Text` (1101, 1103 to 1105, 1107, 1108, 1111, 1112; GM F101, F103 to F105, F10A), `parse_whisper` (1102, F102), `markup_is_safe` |
| party | `parse_union_set` (0B0E), `parse_union_move` (0B12), `parse_id` (0B02, 0B06, 0B07, 0B0F, 0B13, 0B14, 0B17, 0B1A), `parse_answer` (0B03, 0B15), `parse_name` (0B09, 0B11, 0B18); 0B10 is read inline (`server.rs:2008`) |
| trade (also duel, guild war, my_shop) | `parse_id` (0A01, 2401, 2501, 2309), `parse_byte` (0A02, 0A05, 0A07, 2402, 2409, 240B, 2502, 2306), `parse_offer` (0A06, 240A), `parse_money` (0A08, 240C) |
| guild | `parse_npc_upgrade` (0D25), `parse_create` (0D21), `parse_remark` (0D20), `parse_yes` (0D04, 0D06, 0D22), `parse_id` (0D05, 0D07, 0D0A, 0D34, 0D35), `parse_flag_and_id` (0D08, 0D0C) |
| friend | `parse_name` (2202, 2209), `parse_yes` (2203), `parse_id` (2205, 220A), `parse_memo` (2206, 220B) |
| quest | `parse_request` (0902 to 0904), `parse_choice` (0907), `parse_quest_id` (0908) |
| shop | `parse_buy` (0702), `parse_sell` (0703) |
| ground | `parse_pick_up` (0205), `parse_drop` (0206) |
| item | `parse_use(opcode, payload)` (050A, 0557, 0559), `parse_party_call_answer` (0224), `parse_repair` (0554), `parse_save_point` (0220), `parse_body_change` (0226) |
| gem | `parse_link` (0801), `parse_extract` (0802), `parse_badge` (0803), `parse_place` (0804, 0808), `parse_enchant` (0805), `parse_compose` (0806), `parse_remake` (0807) |
| market (map 42 only) | `parse_search` (1C01), `parse_same_item` (1C02), `parse_page` (1C03), `parse_register` (1C08), `parse_id` (1C09, 1C0A, 1C0C, 1C0D, 1C15, 1C16), `parse_bid` (1C0B) |
| my_shop | `parse_start` (2303), `parse_add` (2305), `parse_set_price` (2307), `parse_buy` (230D) |
| point (mall) | `parse_buy` (2602), `parse_gift` (2603), `parse_slot` (2607, B102) |
| vehicle | `parse_ask` (021D), `parse_answer` (021E) |
| mini_game | `parse_sit` (3101), `parse_guess` (3107) |
| admin (staff, except 1201) | `parse_user_question` (1201, a player opcode), `parse_set_level` (F601), `parse_name` (F602, F709, F70B, F801 to F804, F806, F80D, F80E, F901, F904, FA08, FA09, FA0B, FA0E, FA10, FA1A), `parse_set` (F701), `parse_make_item` (F702), `parse_recall_item` (F703), `parse_mob` (F704, F705), `parse_npc` (F70C, F70D), `parse_mob_id` (F70E), `parse_ban` (F70A, F805, FA0D, FA0F), `parse_quest` (F809, F80A), `parse_name_quest` (F80B, F80C), `parse_event` (F80F, F810), `parse_summon_to` (F905, F90C), `parse_notice` (F906), `parse_repeat_notice` (F907, F909, F90B), `parse_notice_to` (F908), `parse_notice_country` (F90A), `parse_named_text` (FA07), `parse_go` (FA0A), `parse_map` (FA11), `parse_guild_go` (FA12), `parse_guild_grade` (FA13), `parse_guild_rank` (FA14), `parse_guild_points` (FA15), `parse_call` (FA18), `parse_auction_recall` (FA19), `parse_uq_answer` (F202), `parse_uq_answer_to` (F203); helpers `name_field`, `family` |
| frame | `FrameDecoder::push`, `FrameDecoder::next_frame`, `frame::opcode`, `frame::payload` |

### 5.5 Login: `crates/login-server/tests/fuzz_login.rs`

Random packets in each phase: before the key, after the key, and after a login. The helpers of `tests/login_flow.rs` (`serve_with`,
`Client::connect`, `handshake`, `send_plain`, `send_encrypted`, `login`) move to `tests/common/mod.rs` in the same way. Use one
`KeyPool` for the file (generation is slow), and raise `LimitConfig` so 127.0.0.1 does not lock itself out, except in the property
that tests the lockout. Properties: no panic; every connection is answered or closed; a real client still logs in afterwards.

## 6. How it was built

The fixes were built in four parts, in this order: part 1 makes the server survive hostile connections; part 2 makes exchanges and saves whole; part 3 covers the login guard, the editor, inputs and the latent items; part 4 is the harness, written last so that it tests the fixed code. Each part kept `cargo test --workspace` and clippy green, kept `memory.rs` in parity with `pg.rs`, wrote its deviations from the original into the analysis note they belong to, and listed its play-test items in `docs/testing/live-test-checklist.md` section 54.

### Part 1: survive hostile connections (R1, R2, R4, R5, R6, R8, R9, R10, R11, R18, R19)

New settings: `GAME_WRITE_TIMEOUT_SECS` 20, `GAME_OUTBOX_LIMIT`
4194304, `GAME_MAX_PER_IP` 8, `GAME_CONNECTS_PER_MINUTE` 30, `GAME_CHAT_PER_SEC` 3, `GAME_CHAT_BURST` 5, `GAME_PACKETS_PER_SEC` 100,
`GAME_DB_POOL` 24, `GAME_DB_ACQUIRE_SECS` 10, `LOGIN_CONNECTS_PER_MINUTE` 30, and the login pool to 16.

### Part 2: whole exchanges and saves (R3, R12, R13, R14, R15, R16, R20, R23, R24, R25, R27, N6)

**Where the build differed from the plan:**
- R3 (c) was already in the code: `trade_end`, which `cancel_actions` calls, closes an open stakes window (`close_duel_window`), and so
  does `0x2301`; the build only added the tests the part asks for. The real gaps were (a), (b) and (d), plus the dead check.
- R3 (a) cancels the duel or trade through the same ends as before: the stakes window closes on both screens, and stakes the partner had
  already taken go on the ground at its feet (the existing end of a countdown); `docs/original/duel.md` deviation 5 says so.
- R13 left the party summon effect-then-use-up as well, with the transient effects: it only knows whether anyone was asked after asking.
- R16: the Etin items worth 0 stay in the bags; a refused return (left the guild meanwhile) answers nothing, a refused purchase answers
  "not the master".
- R27: a creation on its way to the database no longer lapses by the clock (the outcome ends it).
- Not changed: R14's `pvp.rs` kill-share sends still ignore a closed events channel (the same `Gold` arm and exit drain cover what
  reaches the connection; the zone-side fallback is only for the monster kills' party shares).

### Part 3: login guard, editor, inputs and latent items (R7, R17, R21, R22, R26, R28, R29)

**Where the build differed from the plan:**
- R7 (changed afterwards, approved): new passwords have **6 to 16** characters, not 6 to 12, and the login reads a password to 16.
  The 12 was the original server's cut; the client sends 16 bytes of the password (`docs/original/ps_login.md` section 3). The username stays at 12.
- R7: the address count of 20 failures in 10 minutes also counts the attempts made while an account is blocked (not the account's own
  counts, so a block cannot be kept going by trying). The settings are `LOGIN_PAIR_FAILURES`, `LOGIN_PAIR_WINDOW_SECS`,
  `LOGIN_PAIR_LOCKOUT_SECS`, `LOGIN_ACCOUNT_FAILURES`, `LOGIN_ACCOUNT_WINDOW_SECS`, `LOGIN_TRACKED_ACCOUNTS` (README). The older
  `LOGIN_FAILURE_*` address count no longer clears on a good login.
- R17: `tools/checklist_server.py` takes any IP address as the `Host` as well as `localhost` (the page is opened by the box's LAN address;
  a rebinding page has a name, never an address). Extra names: `CHECKLIST_HOSTS`. The editor follows the text exactly (`EDITOR_HOSTS`).
- R21: the height limit is `world::MAX_HEIGHT` (10,000). Hair, face and size are not clamped: client queue entry 13 (a) is still open, so
  `creation.rs` and `server/items.rs` carry the `TODO` the entry asks for. The shop title check is in `my_shop::parse_start`.
- R22: `frame::encode` logs through `tracing`, which `shaiya-proto` now depends on. The market refresh cut is `PAGE + 1` rows.
- R26: the order also updates `state.staff` and the occupant's flag (`Zones::set_staff`), and a demoted GM's held question is given back
  at once, since leaving the world only does that for staff.
- R28: `dofile` and `loadfile` are removed as well (the base library is always loaded; no shipped script uses them).
- R29: both gaps closed. The released-session path covers a handshake that times out as well as one that fails: `handle` keeps the
  claimed session in a slot the handshake fills. Not tested by a socket test (the failure must come from the store or the clock at the
  right instant); the claim slot is exercised by every handshake test.

### Part 4: the fuzz harness (section 5)

**Where the build differed from section 5, and what it found:**
- Files: `shaiya-proto/tests/decoders.rs` (142 tests: 109 decoders, 17 round trips and wrong-count checks, the frame layer, the encoders at
  their largest input, the text helpers), `shaiya-crypto/tests/fuzz.rs` (6), `login-server/tests/fuzz_login.rs` (4 properties) and
  `game-server/tests/fuzz_dispatch.rs` (4 properties), each property with an `#[ignore]` twin in the last two. The shared helpers moved to
  `tests/common/mod.rs` of game-server and login-server (the items `pub`, nothing changed in what they do; the login `serve_with` now calls
  a new `serve_with_keys`). `proptest` 1.12 is a dev-dependency of the four crates.
- Production code touched: `Zones::ground_items` (`#[doc(hidden)]`, for the value count), `MemoryBackend::give_items` and `set_position`
  (setters, as `set_money` is), `KeyPool` is `Clone` (one pool for `fuzz_login.rs`).
- **Regression files** go to `proptest-regressions/<test file>.txt` of each crate (`FileFailurePersistence::Direct`): the default place is
  worked out from a sibling `lib.rs`, which a file in `tests/` has none of, so proptest saved nothing and said so only on a failure. None
  is committed because no property kept a failing case: every failure seen while building was the test's own fault (below) or a planted bug.
- **Counting panics:** the hook counts only panics on tokio's worker threads (`tokio-rt-worker`, which is the server's tasks); a test's
  own assertions fail on the test thread and already fail that test. Counting every thread made one failing property fail the other three.
- **Opcodes and payloads** follow section 5.3, with a layout table per opcode (`PLAYER`, `STAFF` in `fuzz_dispatch.rs`) for the exact-length
  and field-aware kinds. The two-player property raises the field-aware share to half, since at 15% over 38 opcodes it sent a drop with
  a real bag and slot about once in 200 cases. The ids include the ground items of the moment, so pick-ups reach real ones.
- **Value conservation:** items must not grow, but gold may grow by what the lost items sell for. `0x0703` sells **anywhere** (as the
  original, `docs/original/shop.md` section 3), so a potion sold at the start spot is +2 gold and −1 potion, not a duplication; the first
  version of the check called it one. The ground is emptied between cases (`Zones::expire_items` with a time past the lifetime), so an
  earlier case's drops are not picked up by the next.
- **Staff accounts** get more than a player in the zone entry, so the helper `read_zone_entry` (a player's) trips; `fuzz_dispatch.rs`
  reads on to the player's HP, MP and SP packet instead.
- **Login:** one server and one `KeyPool` for the three connection-phase properties, with every limit raised; the lockout property starts
  a server per case (limit 6) and checks that a login never succeeds with wrong credentials, that a lockout never comes before the sixth
  failure, and that a locked-out address is not let in again. The first packets before the key are plain; a 128-byte ciphertext that
  decrypts leaves a session the fuzz client has no key for, so what follows is noise to the server, which is the point.
- **Debugging aids:** `FUZZ_TRACE=1` prints the opcodes of each case before it runs (a failure that is the server's state after an earlier
  case shrinks to any packet; the trace names the case that changed it) and `FUZZ_LOG=game_server=debug` turns the server's log on.
- **Checked that the harness can fail:** a planted `panic!` in the drop handler was found by `one_player` and `two_players` (the second
  shrank to one `0x0206` packet), a planted second ground drop was found by the value count (an item grew from 4 to 5), and a planted panic
  in the login decoder failed all four login properties. Each was reverted. The first try found the hook's thread name wrong, so it had
  counted nothing: the reason this check is written here.
- Default run times (debug, on the dev machine): `decoders` 3.3 s, `fuzz` 0.6 s, `fuzz_login` 12 s, `fuzz_dispatch` 10 s.
- Long runs (`PROPTEST_CASES=5000`, release): all passed. Release, 5,000 cases each: `decoders` 143 tests in 4 s, `fuzz` (crypto) 5 in 8 s, `fuzz_login` four twins in 2,714 s (the lockout
  property is most of it: each case hashes passwords, about 0.45 s), `fuzz_dispatch` four twins in 1,901 s (`--test-threads=2`; the
  one-player, staff and before-the-world twins together took about 30 minutes, the two-player one the rest). Run the twins only (`--ignored`) with
  two test threads: eight at once on this 8 GB box, with swap in use, made timeouts of the test's own clients fail the lockout property
  once (it passed alone, 200 cases in 90 s) and made the short ones crawl. The twins of a file are the same properties as the short ones,
  so `--include-ignored` only adds the short ones again.

**What the harness found in the server: nothing so far** (no panic, no hang, no duplication, no locked zone, no unreleased session in
the default runs or the long ones). The decoders held on 5,000 cases of each of 109 inputs.

What it did find was three faults in the tests themselves (one a race) and one failure not explained:
- the first panic hook counted nothing (the thread name was wrong) and the shared counter made one failure fail every property: found by
  planting a panic, as above;
- the first value check called `0x0703`'s sell-anywhere a gold duplication (it is the original's behaviour, `docs/original/shop.md`);
- the lockout property opened two connections per attempt and was refused the second one while the server recorded the sixth failure
  (`server.rs` sends the reply of a failed login and then counts it, so the next connection can still be let in or be refused a
  moment later: harmless, one attempt of slack in a 10-minute window, and nothing to change). It now uses one connection per attempt.
- Not explained: `staff_dispatch_survives` failed twice with `handshake reply` (a fresh account's handshake got no answer) in the
  first runs, both times with a planted panic compiled in and several properties running at once. It did not recur in 10 default runs,
  300 and 5,000 clean cases, nor when the planted panic was sent straight to a staff account and a probe followed. If it ever shows up
  again, run with `FUZZ_TRACE=1 FUZZ_LOG=game_server=debug` and keep the output.

Not covered, as section 5.3 says: R3 is theft and the timing items R1 and R2 have their own tests in parts 1 and 2. Also not covered: the
packets that need a second state set up first (an open personal shop with a buyer, a guild with members, a running instance, the
auction on map 42, a held user question): the opcodes are sent, but a handler behind such a state is reached only by chance. The
harness would reach them with fixtures of those states; that is a further piece of work, not part of this audit.
