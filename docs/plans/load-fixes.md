# Load fixes: what the PvP swarm found (spec and build plan)

**Specified 2026-11-13; built 2026-11-14** (pieces 1 to 11b: the fixes are committed with their tests, the runs of 2026-11-14 held every target, and the write-up is in `docs/testing/load-pvp.md`). The fixes for the open findings of `docs/testing/load-pvp.md` (F2 to F5) and the uncommitted
batching change in `server.rs`, then the 700 run again. Each piece below is built by one agent and reviewed before the next starts
(section 6 and `load-fixes-prompt.md`).

## Goal

The 700-player battle on the dev box holds by the spec's targets (`docs/plans/test-bots.md` section 11: attack acknowledgement p95 under
250 ms, step echo p95 under 500 ms, nobody dropped), the 300 run is no worse than on 2026-11-12, entering at 10 a second with 700 in
the world no longer empties the database pool, and a session whose end failed is not kept alive by the heartbeat. Every fix is committed
with its tests, the runs are written up in `docs/testing/load-pvp.md`, and `main` is pushed.

## 1. What is settled (the maintainer, 2026-11-13)

1. **All six items are done**, in the order of section 5: the batching change first, then F3, F5, F4, F2 in steps, then the runs.
2. **The finish line**: every fix committed with tests, the 700 run repeated and written up, then `git push` on `main`. Only
   repository-scoped deploy keys exist and there is no `gh`; a push that fails is reported, never worked around.
3. **Full autonomy on the dev box's servers** for the run pieces: an agent may edit `config/game.toml` for a run, restart the login
   and game servers, and run `bot seed`, `bot swarm` and `bot unseed` against the real game (database `newshaiya`, ports 30800 and
   30810), and must put everything back after (section 5, piece 11a). Tests use `TEST_DATABASE_URL` (port 5434) only.
4. **The machine**: 4 cores, 7.6 GB. One cargo command at a time, never two in parallel. The live binaries are built together:
   `cargo build --release --workspace`.
5. **Commits** carry no AI trailer and no mention of AI (no `Co-Authored-By`, no session lines), in the message or the code. Paths are
   staged by name. Messages: `Load fixes <N>: <what and why>`, a plain sentence, as `git log` shows.
6. **Review**: each piece is built by a Sonnet agent at high effort and reviewed by an Opus agent against the piece's checklist
   (section 6) before the next starts.
7. **Overnight, unattended** (answers of 2026-11-13): a piece rejected twice goes to an Opus fixer, then on; the push happens when every
   piece is accepted and the tests pass, even if a run target was missed (the miss is written up as an open finding); the maintainer is
   told by a push notification and a status file when the run stops or ends; the runs of piece 11a may be done at any hour.
8. **Answered** (section 10): the runs may restart the servers at any time; design B2 stands (piece 9), the second mutex is its fallback;
   piece 7 and the entry check E2-1000 stay; 192.168.1.240 is the LAN address; F3 is fixed both ways and `db_pool` stays 24.

## 2. Choices made in this spec (the maintainer may change them)

| # | choice | why |
|---|---|---|
| C1 | F3 is fixed **both** ways: the heartbeat renews only the sessions of the accounts this server holds, and a failed end is tried again in the background | the first makes a lost end expire in at most 120 s whatever happens (a restart included); the second ends it in seconds in the usual case |
| C2 | F5 is **not a bug**: no code change, a test that pins the behaviour, and the note in `load-pvp.md` | section 3.5 |
| C3 | F4: `load` takes **one** pooled connection for the whole load and makes fewer round trips; `db_pool` stays 24 | the pool ran dry from 15 separate acquisitions per entry each waiting in the queue, not from too few connections (section 3.4) |
| C4 | F4: the player count of the heartbeat is **not** batched | it is one statement every 5 s; its slow statements are not from the run (section 3.4) |
| C5 | F2: **outbox sends made while `maps` is held are deferred** until the lock is released (section 4, design B2); per-map locks are not done | the battle is on one map, so per-map locks would not change the measured run |
| C6 | F2 gains a piece the findings did not name: the world loop stops taking the `bans` mutex at every wake | the second most waited-on lock in the 1000 run (`HashMap<u32, Bans>`, 10,000 to 15,000 waits per 10 s) |
| C7 | The stats line gains `out_wps` (piece 1), `db_busy`, `db_size`, `db_waits`, `db_wait_ms`, `db_wait_max_ms` (piece 4), `maps_held_ms`, `maps_held_max_ms` (piece 8), **appended at the end** | the instruments for each fix; appended so the status board and anyone reading the line by position keep working |
| C8 | The runs: R2-300 and R2-700 as on 2026-11-12, plus **E2-1000**, an entry check (1000 bots at 10 a second, 4 minutes) | F4 is about entering at 10 a second with 700 in the world; only the 1000 run showed it |
| C9 | The runs are bots only, without the real client | the client is the maintainer's to judge (`test-bots.md` decision 2) |
| C10 | Batching: the cap is soft (a batch stops once it holds `BATCH_BYTES` or more, so it is at most 16 KB plus one packet), and the batch is built in a buffer kept on the connection | an outbox cannot give a packet back; a kept buffer does not allocate per write |

## 3. Facts found in the code (2026-11-13, commit 85fa13e plus the uncommitted diff)

### 3.1 The batching change (uncommitted, `crates/game-server/src/server.rs`)

- `Connection::send` (server.rs:346-352) now calls `seal` (354-362), which encrypts one packet in place after its two length bytes
  (XOR table in the world, AES-CTR before) and counts it (`monitor::counters().frame_out`), then `write` (385-415).
- `Connection::send_queued` (364-380) seals `first`, then takes more packets with `OutboxInbox::try_recv` while the buffer holds fewer
  than `BATCH_BYTES` (16 KB, 1091-1092), sealing each, and writes them in one `write`. `world_loop` calls it for the outbox branch
  (1573-1581) instead of `send`.
- **The XOR is per packet**: `XorTable::apply` (`crates/shaiya-crypto/src/game.rs`:47-50) uses the table from offset `n` for a packet of
  `n` bytes, so it must be applied to each packet alone, never to a batch. It is stateless between packets. AES-CTR (`CtrStream`) is
  stateful: its packets must be sealed in the order they are written.
- Outbox packets are only drained in `world_loop`, where the XOR is always on: `conn.xor` is set before the world (server.rs:1023) and
  cleared only in `finish_logout` (5006), which returns from the loop at once. `seal` still handles both, and the tests cover both.
- The outbox accounting: `try_recv` and `recv` subtract the packet from the queue's byte count and from the server-wide `outbox_sum`
  (`outbox.rs`:74-91). The overflow flag is sticky (set in `Outbox::send`, 48-66, never cleared): a packet drained into a batch cannot
  hide an overflow, the loop's checks (server.rs:1566-1569 and 1575-1578) still see it at the next turn.
- `write` (385-415): one deadline (`write_timeout`) for the whole `write_all`, `biased` so a write that goes through at once is not cut by
  a kick already waiting; a kick or the shutdown abandons the write, marks the connection closed and shuts the socket. With a batch the
  deadline now covers up to 16 KB plus one packet instead of one packet.
- `seal` counts a packet as sent before the write; a failed write has counted packets that did not go out. That was so before.
- `send_queued` returns at once when the connection is closed; the packet already taken by `recv` is dropped and the rest stay queued
  until the connection ends (as `send` did).
- The diff's comment says the write per packet "was the cost that grew with the crowd's square (`docs/testing/load-pvp.md`)". That page
  does not say so (it names the CPU, the `maps` lock and the fan-out, lines 79-82); the comment must not claim it.
- Existing tests that exercise this path: `tests/game_flow.rs` `a_client_that_stops_reading_is_disconnected_after_the_write_deadline`
  (4218), `a_kick_reaches_a_client_that_stopped_reading_at_once` (4238), `a_connection_over_its_outbox_limit_ends_once_the_write_is_through`
  (4256); the in-process swarm of `crates/bot` reads the stream through `FrameDecoder`.

### 3.2 Sessions and the heartbeat (F3)

- The heartbeat task (`server/tasks.rs`:214-224, every `heartbeat_secs`, default 5, `config.rs`:164) calls
  `Heartbeat::heartbeat(server_id, players, open)` (`backend.rs`:785-788; forwarded for `Arc` at 1293-1295). The console's
  `/server on|off` calls it too (`server/console.rs`:138).
- The PostgreSQL heartbeat (`pg/heartbeat.rs`:88-108) runs `UPDATE game_servers SET players …` and then
  `UPDATE sessions SET expires_at = now() + 120 s WHERE server_id = $1 AND game_connected_at IS NOT NULL` (`SESSION_GAME_SECS`,
  `pg.rs`:34): every in-game session of the server, whether or not this process holds it.
- A session is ended by `SessionGate::release` (`pg/session.rs`:51-58, `DELETE … WHERE user_id = $1 AND identity = $2`, so it never
  touches a newer login's row). It is called once at two places in `handle` (server.rs:769-775 after a failed handshake, 790-792 at the
  end) and an error is only logged.
- `OnlineUsers` (server.rs:574-689) holds every account with a connection here, from `Online::claim` (701-723, before the session is
  claimed) until the `Online` is dropped (733-737), which `handle` does after `release` (795). A character staying after its connection
  ended is still in it (the stay runs inside `in_world`).
- **What a stale row blocks**: the login server's `SessionStore::create` replaces the account's row on every login
  (`login-server/src/pg.rs`:83-92, `ON CONFLICT (user_id) DO UPDATE … game_connected_at = NULL`), and the game server claims the new
  identity, so **a player is not locked out**. The only readers of `game_connected_at` besides the game server are `bot seed` and
  `bot unseed`'s guards (`crates/bot/src/seed.rs`:397 and 519-520), which refuse while a bot has one.
- **A restart does not clear a stale row**: the new process's heartbeat renews every in-game row of the server again. The row goes only
  when the game server is down longer than 120 s (it then fails `expires_at > now()`), and the login server deletes it a minute after
  that (`purge_expired_sessions`, `login-server/src/pg.rs`:29-31, every 60 s, `login-server/src/main.rs`:37-40).
- `supervise::retried` (`supervise.rs`:39) with `WRITE_RETRIES` (35: 200 ms, 1 s, 5 s) is the server's retry helper.
- The memory backend records the last heartbeat (`memory.rs`:58-59, 1268-1269) and has failure switches in the form of
  `fail_next_trade` (80, 799).
- The 1000 run's log (`logs/game-server.log`, 2026-11-13): `could not end the session of a failed handshake` for user 981 at 05:26:43 and
  `could not end the session` (the normal end, server.rs:791) for user 843 at 05:26:54, both `pool timed out`.

### 3.3 The kill counts (F5)

- `kill_count = false` sets `KillCounts::disabled` (server.rs:247-248); `load_kill_counts` keeps the flag when it puts the saved counts in
  (`server/tasks.rs`:263-271), and the start logs `counting=false`.
- `add` (`kc.rs`:214-223), `sub` (228-244) and `add_grade` (259-269) do nothing while `frozen()` (176-178), which is true when `disabled`:
  **bot kills cannot move the counts** with `kill_count = false`. The player kill path is `server/pvp.rs`:39-40.
- `tick` (`kc.rs`:273-291) does not look at `disabled`: every 10 minutes it marks both factions sent and asks for a save, and
  `kill_count_changes` (`server/tasks.rs`:405-434) queues `Save::KillCounts`, which `save_kill_counts` writes as two
  `INSERT INTO faction_kill_counts … ON CONFLICT DO UPDATE` (`pg/heartbeat.rs`:40-53). That is the INSERT the slow-statement lines
  name: a rewrite of unchanged counts, two statements every 10 minutes, as the original's `CWorld::UpdateKillCount` does.
- The logs: every start since 2026-11-03 (`logs/live-worktree/game-server.log` and `logs/game-server.log`) loaded `light=0 fury=1366`,
  before any bot existed. The ten-minute saves on 2026-11-12/13 logged `applied: [false, false]` every time.
- **No slow statement at all is logged on 2026-11-13**: the 36 `INSERT INTO faction_kill_counts` and 2,101
  `UPDATE game_servers SET players` slow statements in `logs/game-server.log` are dated 2026-11-04 to 2026-11-11 (before the databases
  moved to the SSD on 2026-11-12). `load-pvp.md`'s "12" and "670" are not the 1000 run's.

### 3.4 Entering and the pool (F4)

- The pool: `PgPoolOptions::new().max_connections(db_pool).acquire_timeout(db_acquire)` (`main.rs`:77-78); `db_pool` 24 and
  `db_acquire_secs` 10 (`config.rs`:223-224). The game log writer shares it (`game_log.rs`:433-448); the notification listener has its
  own connection (`main.rs`:276). sqlx warns `acquired connection, but time to acquire exceeded slow threshold` past 2 s.
- **What one entry costs**, each statement taking its own connection from the pool unless it is in a transaction:

  | step | where | statements | acquisitions |
  |---|---|---|---|
  | handshake | `claimable`, `claim` (`pg/session.rs`:9-49), `faction` (`pg/character.rs`:69-77) | 3 | 3 |
  | the list | `characters` (`pg/character.rs`:97-149): one transaction, `UPDATE`, two `SELECT`s, sometimes a `DELETE` | 5 round trips | 1 |
  | select | `load` (`pg/character.rs`:259-395): the row, items, bank, skills, quick slots, applied skills, quests, save points, interest, the account's name, `load_mall` (779-800: points and premium, the log, point items, billing items), friends and blocks | 15 | 15 |
  | the world | `take_gift_notices` (`server/mall.rs`:29), the script values and the pet when those are on | 1 to 3 | 1 to 3 |

  About 25 round trips and 20 to 22 acquisitions per login. A task that needs 15 acquisitions one after the other waits in the pool's
  queue 15 times: with a median wait of 2.7 s (the 1000 run) a `load` takes over 30 s, longer than a bot waits for the burst.
- The 1000 run (`logs/game-server.log`, 2026-11-13): 1,225 slow acquisitions (median 2.7 s, longest 6.1 s), 1,221 of them in the minute
  05:26; 25 connections dropped with `pool timed out`, two failed session ends and one lost game-log batch (28 lines in all, 05:26:12
  to 05:27:42). From 05:26:05 no bot got past `load` for 50 s while the character screens went on (`entered the character screen` 74 in
  05:26:1x, `entering the world` 0). The 700 run at 5 a second had none.
- The heartbeat is two statements every 5 s whatever the number of players; the second rewrites every in-game session row each time
  (about 1000 row updates every 5 s at 1000 players).
- No code path holds a pooled connection while it waits for another: every `begin()` in `pg` uses only its transaction, and the
  handshake waits for an older connection (`Online::claim`, `handshake_wait`) holding none.

### 3.5 The `maps` lock and the fan-out (F2)

- `Zones` keeps every map's players in one `Mutex<HashMap<u16, HashMap<u32, Placed>>>` (`zone.rs`:649) and every map's monsters in one
  more (650); the lock order is written on the fields (653-708): `maps`, then `mobs`, then the rest. The lock table names them `maps` and
  `mobs` by their type names (`monitor.rs`:372-378).
- `maps` is taken at 193 places in `zone.rs` and `zone/*.rs` (`self.maps.guard()`), 20 more in the tests.
- **A send under the lock**: `Placed::send` (`zone.rs`:541-544) copies the packet (`to_vec`) and calls `Outbox::send`: an atomic add, an
  unbounded channel push, and the wake of the receiving connection's task (which can wake a parked worker thread). Every fan-out in the
  zone does this while holding `maps` (119 `.send(&` sites).
- **A step** (`server.rs`:2217-2269) takes `maps` five times: `riding` in `speed_checked` (3183), `move_to` (`zone.rs`:2804-2831),
  `trade_moved` (`zone/trade.rs`:189-194, `maps` and `trades` even without a trade), `duel_moved` (`zone/duel.rs`:359-369) and
  `war_moved` (`zone/guild_war.rs`:234-238). `move_to` scans every player on the map for the distance and sends `0x0501` to those in
  range, all under the lock; `swap_view` (2869-2917) sends the view changes under it too.
- `enter` (2703-2772) sends three packets to everyone in view and builds three for each of them, under the lock; `attack_player`
  (`zone/pvp.rs`:247-317) holds `maps` and `mobs` and sends the blow to the players within 60 (and the death packets before it, in that
  order).
- **The battle is on one map** (map 18): all 700 players are in one inner `HashMap`, so per-map locks would leave this run as it is.
- A player's own packets (the `Vec` a zone call returns) are written directly by its task (`conn.send`), while what others' actions send
  it goes through its outbox; and `move_to` sends `0x0501` by distance, not by view. So the order in which a client sees its own view
  changes and other players' packets is already not the order of the zone's state changes; only each outbox is in order.
- **Every world-loop wake takes the `bans` mutex**: `select!` evaluates `admin::next_ban_end(ctx, id)` (`server.rs`:1782,
  `server/admin.rs`:1462-1472, `zone/admin.rs`:349-351) on every turn of the loop. Every change of an in-world player's bans happens in
  its own task (`AdminOrder::Ban`, `server/admin.rs`:1092-1101; `bans_ended`, 1475-1484), so the next end can be kept in `WorldState`.
- `Relock::guard` (`sync.rs`:25-38) times only a wait, into the lock table by type name; a guard is `!Send`, so a spawned task cannot
  hold one across an `.await`.

### 3.6 The dev box now

- `config/game.toml` still holds the `# LOAD TEST 2026-11-12` block, and the login and game servers still run with `RUST_LOG=info`
  `LOGIN_MAX_PER_IP=1020 LOGIN_CONNECTS_PER_MINUTE=1200 LOGIN_KEY_POOL=32` (started 2026-11-13 05:24 UTC): the last run was not put
  back. `logs/game.toml-before-load-test` is the file without the block (they differ only by it).
- `tools/game-server.sh start` defaults `RUST_LOG=info,game_server=debug` when the environment has none and sources `.env`.
- `bot seed` on existing bots does not change their password (`crates/bot/src/seed.rs`:720-729); `bot swarm` needs it.

### 3.7 Where `load-pvp.md` and the code (or the log) disagree

| `docs/testing/load-pvp.md` | the code or the log |
|---|---|
| 84-87: the 1000 run had "17 pool timed out" | 28 `pool timed out` lines on 2026-11-13: 25 dropped connections, 2 failed session ends, 1 lost game-log batch (section 3.4) |
| 85-86: the heartbeat's `UPDATE game_servers SET players` "showed up 670 times as a slow statement" | no slow statement on 2026-11-13; those lines are from 2026-11-04 to 11-11 (section 3.3) |
| 107-109: both failed entries logged `could not end the session of a failed handshake` | one did (user 981); the other was the normal end of a connection after the character screen (user 843, server.rs:791) |
| 110-111: "for a player that is an account locked out until a restart" | a new login replaces the row (`login-server/src/pg.rs`:83-92); only `bot seed`/`unseed` are blocked (`seed.rs`:397, 519-520); a restart does not clear it (section 3.2) |
| 113-114: "the heartbeat's player-count update" as a cost of entering | the heartbeat is two statements every 5 s (`server/tasks.rs`:214-224), not per entry |
| 115-117: 12 `INSERT INTO faction_kill_counts` in the 1000 run; whether bot kills moved the counts "not yet checked" | none on 2026-11-13; with `kill_count = false` kills cannot move them (`kc.rs`:176-178, 214-219); the INSERTs are the ten-minute tick (section 3.3) |
| 29-30: the settings are put back after a run | they were not (section 3.6) |
| 105-106: the fix is "per-map locks, or the view's recipients taken out of the lock" | per-map locks would not change a one-map battle (section 3.5) |

Also: `docs/plans/README.md`:16 says the test bots are "not built" (parts 1 to 4 are); `CONTRIBUTING.md`:37 and `tests/postgres.rs`:28 let
the PostgreSQL tests fall back to `DATABASE_URL` when `TEST_DATABASE_URL` is not set (it is set in `.env`; a builder checks before
running them); `backend.rs`:786 says the heartbeat "keeps its players' sessions from expiring" and `docs/server/architecture.md`:57-58
that it "renews `expires_at` of every session it has claimed": it renews every in-game session with its server id, held by this process
or not, which piece 2 makes true.

## 4. F2: the designs

| design | what | for this run | cost and risk |
|---|---|---|---|
| A. Per-map locks | `HashMap<u16, Mutex<HashMap<u32, Placed>>>` | **nothing**: one map | 193 sites; cross-map walks (`find_by_name`, `send_country`, `notify_country`, `send_all`) take every map's lock; a map change holds two (an order by map id); `mobs` and the rest keyed by map would follow |
| B1. Recipients out of the lock, by hand | each broadcast collects `(outbox, packet)` under the lock and sends after it | the step and attack fan-outs leave the lock | every one of the 119 send sites is changed by hand; a mix of sent-now and sent-later packets in one operation reorders them |
| **B2. Deferred sends while `maps` is held** | `Zones::maps()` returns a guard that opens a deferral scope: every `Outbox::send` on that thread while it lives is queued in a thread-local list, in order, and delivered right after the lock is released | every send under `maps` leaves the lock: steps, views, attacks, entering | one mechanical rename (`self.maps.guard()` to `self.maps()`); the 119 send sites stay as they are; a send's outcome (closed, over the limit) is applied at delivery |
| C. Shared packet buffers | the outbox holds `Arc<[u8]>`; the batch copies and seals per connection | the per-recipient copy under the lock goes | the outbox type changes everywhere; composes with B2 and with batching; later |
| D. A spatial index | players by cell, a step scans only nearby cells | nothing: everyone is in view | for a spread-out world, later |

**Recommended: B2.** It takes the costly part of a send (the channel push and the wake) out of the lock at every site at once,
including the operations nobody would think to convert by hand (`enter`'s 2,100 sends at 700 players), keeps each operation's order
exactly (all its sends are queued in order and delivered in order, so `attack_player`'s death packets still come before the blow), and
is small to review: the guard, the scope, and a rename the compiler checks. What it gives up: two operations that finish within
microseconds of each other on two threads may reach a third player's outbox in the other order than they held the lock. The client
already sees such orders (section 3.5, last points), but it drops a packet about an id it does not know yet, so a swap (an appearance
after a death, a hit on a just-spawned monster) lasts until that unit's next update, and a dropped `0x0610` HP delta leaves a bar off
for the fight; the runs were bots only, so this is unchecked on a real client. If the run or the client shows artifacts from it, the fallback is to deliver under a
second mutex taken before `maps` is released (deliveries then keep the lock's order, at the price of being one at a time).

Its invariants:

- **Lock order** unchanged: `maps` before `mobs` and the rest. Delivery takes no zone lock (`Outbox::send` takes none).
- **No `.await` while the guard lives**; the guard and the scope are `!Send`, so a spawned task that tried would not compile, and the scope
  cannot move to another thread.
- **Delivery happens after the unlock**: the guard's fields are declared lock first, then the hold timer, then the scope, and Rust drops
  fields in declaration order. That order is load-bearing and commented.
- **Delivery never panics** (it runs in `Drop`, perhaps while unwinding): `Outbox::send` does not panic.
- A scope is counted (nested scopes deliver at the outer end) and is cleared by its own drop, so a panic under the lock leaves no scope
  open on the thread.
- The `Mutex` type does not change, so the lock table still names it `maps`.

## 5. The pieces

Piece 0 is the orchestrator's; pieces 1 to 11b are one agent each, in this order. Every piece ends with `cargo fmt --all` and
`cargo clippy --workspace --all-targets -- -D warnings` clean and the tests named in it passing. `cargo test -p <crate>` while working
(narrow with a test name); the whole `cargo test --workspace` (about 25 minutes) only where a piece says so. PostgreSQL tests run on
`TEST_DATABASE_URL` from `.env` (check `grep -c '^TEST_DATABASE_URL=' .env` is 1 first); nothing sets `DATABASE_URL` in a test command.

### Progress

| piece | what | state | commit | reviewed |
|---|---|---|---|---|
| 0 | the specs committed | done | 1774edb | by the maintainer's request, 2026-11-13 |
| 1 | batching | done | f0896b4 | accepted 2026-11-13, 1 round |
| 2 | F3, sessions | done | 04bfdb5 | accepted 2026-11-14, 1 round |
| 3 | F5, the kill counts | done | 23ccf8b | accepted 2026-11-14, 1 round |
| 4 | F4a, the pool measured, `load` on one connection | done | 85d19af | accepted 2026-11-14, 1 round |
| 5 | F4b, fewer round trips in `load` | done | 3876678 | accepted 2026-11-14, 1 round |
| 6 | F4c, the heartbeat's session writes | done | 2f81384 | accepted 2026-11-14, 1 round |
| 7 | F2a, the ban end kept in the world loop | done | b7dd31e | accepted 2026-11-14, 1 round |
| 8 | F2b, the `maps` guard and its hold time | done | 83bb08b | accepted 2026-11-14, 1 round |
| 9 | F2c, sends deferred while `maps` is held | done | 412f9c3 | accepted 2026-11-14, 1 round |
| 10 | F2d, a step's follow-up checks | done | 06fb355 | accepted 2026-11-14, 1 round |
| 11a | the runs and the cleanup | done | (no commit; logs/load-pvp/R2-summary.txt) | accepted 2026-11-14, 1 round; all targets held, no extra round |
| 11b | the write-up and the push | done | dc7654d, fixed in 5d21371 and 95b9f86 | accepted 2026-11-14 after 1 fix round (the F6 figure); the pushed head is a046a43, the two fix commits are local until the maintainer approves a push |

**Carry-overs for piece 11b** (non-blocking notes from the reviews; 11b took them with its write-up, small doc edits only; all taken except `defer()`'s `try_with`, which is a code change, left):

- Piece 6: a held session now tolerates about 85 s of failed heartbeats before its row expires (it was about 115 s), and is purged after about
  145 s; the 1000 run's `pool timed out` window was about 90 s. Say so in the write-up. `docs/server/settings.md` (the `db_pool` row): reword
  the entering sentence to "about 20 statements on 6 to 8 connections taken one after another (the handshake 3, the list 1, `load` 1 for its
  twelve statements, the world 1 to 3)", and drop "so the default stays" (the reason is that raising it does not help). `pg.rs`
  `SESSION_RENEW_BELOW` comment: a row is rewritten every 30 to 35 s, and at a 60 s interval it keeps 60 s, not 30 (the floor is `SESSION_GAME_SECS`
  minus one interval when that is smaller).
- Piece 4: `db_waits` counts only `load`'s acquisition; the write-up must say so when it quotes the figure.
- Piece 8: `monitor.rs` `maps_held` doc: the total can exceed the interval by up to one hold that straddles its start (reword); `Zones::maps` doc: "the one way to take `maps` outside the tests, so every hold is timed"; `docs/server/settings.md` `stats_every` row: separate the `maps_held` sentence from the `db_waits` parenthetical.
- Piece 9: the cost of B2 is wider than "for a moment": the client drops a packet about an id it does not know yet, so a swapped order across two threads (an appearance after a death, a hit on a just-spawned mob) lasts until that unit's next update, and a `0x0610` HP delta that was dropped leaves a bar off for the fight. Put one clause in section 8's first risk row and section 4's "What it gives up", and one sentence in the `DeferScope` doc (`outbox.rs`): the order is kept per thread, not across threads. The write-up in `load-pvp.md` says the real client was not in the runs (C9), so this artifact is unchecked; list it as an open question for a run with the client. `defer()` uses `DEFERRED.with`; `try_with` would match the rest of the file (optional).
- Piece 3: `docs/testing/load-pvp.md` F5 text: "the ones counted are from 2026-11-04 to 11-11" could name the 36 INSERTs and 2,101 UPDATEs.

### Piece 0. The specs committed (the orchestrator)

- **Files**: `docs/plans/load-fixes.md`, `docs/plans/load-fixes-prompt.md`, `docs/plans/README.md` (one line for `load-fixes.md`, after
  `test-bots.md`; and that line's "not built" becomes "parts 1 to 4 built").
- **Verify**: `python3 tools/check_links.py`.
- **Commit**: `Load fixes spec: batching, the session heartbeat, the kill counts, the pool at entry and the maps lock in steps, then the 700 run again`.
- **Done when**: committed; `git status` shows only the pre-existing `server.rs` change.
- **Done**: `1774edb` (2026-11-13), by the maintainer's session, not the orchestrator. The orchestrator starts at piece 1.

### Piece 1. Batching: finish `send_queued`

- **Files**: `crates/game-server/src/server.rs`, `crates/game-server/src/monitor.rs`, `crates/game-server/src/server/tests.rs`,
  `crates/game-server/tests/game_flow.rs` (the field list of `the_console_answers_stats_with_the_figures_of_the_stats_line`),
  `docs/server/settings.md` (the `stats_every` row names `out_wps`).
- **Behaviour**:
  - Keep the diff's shape: `seal` per packet, `send` = seal + write, `send_queued(first, outbox)` for the outbox branch.
  - The batch is built in a buffer kept on `Connection` (`batch: Vec<u8>`, set in `handle`'s constructor, server.rs:742): cleared, not
    freed, between writes. When `try_recv` gives nothing after `first`, `first` is written as it is, with no copy.
  - Each packet is sealed on its own bytes (`packet[2..]` of that packet), in the order taken from the queue, and appended; no seal runs
    on the batch buffer. A batch stops taking once it holds `BATCH_BYTES` or more (so it is at most 16 KB plus one packet).
  - One `write` per batch with the existing deadline, kick and shutdown handling; its doc comment says the deadline is for the batch.
  - Closed connection: as the diff (return before sealing; nothing more is taken).
  - A new counter `Counters::write_done()`, bumped once per successful `write` (direct sends and batches alike); the line gets
    `out_wps=` (writes a second) after `workers=`. `out_pps` stays per packet.
  - The doc comment of `send_queued` says what it does and why (fewer writes and segments when the outbox holds several packets), without
    the claim of section 3.1's last point but one.
- **Invariants**: the bytes on the wire are the packets in queue order, each sealed as `send` would have; AES-CTR's stream position
  advances in that same order; the queue's byte count and `outbox_sum` drop by exactly what was taken; an overflow is still seen at the
  next turn of the loop; a kick or shutdown during a batch write closes the connection as before.
- **Tests to add** (`server/tests.rs`, over a loopback `TcpListener` pair; build a `Connection` directly):
  - `queued_packets_go_out_in_one_write_in_queue_order`: five packets of different lengths in an outbox, `send_queued` with the XOR
    table on; the far side decodes five frames (`FrameDecoder`), un-XORs each, and gets the originals in order.
  - `a_batch_under_aes_keeps_the_stream_in_order`: the same with `xor` off and `send` set to a `CtrStream`; the far side decrypts with one
    `CtrStream` of the same key and IV, frame by frame.
  - `a_batch_stops_at_batch_bytes_and_leaves_the_rest_queued`: three 10 KB packets; after one call the far side has two and the outbox's
    `queued()` is 10 KB plus nothing else.
  - `a_closed_connection_takes_nothing_more_from_the_outbox`: `closed = true`; after the call the outbox still holds what was after `first`.
  - `monitor.rs`: `writes_are_counted_apart_from_packets` on a local `Counters::new()` and `Sampler` (not the process-wide one); update
    `the_line_reads_from_fixed_figures` for the new field.
- **Existing tests that must pass unchanged**: the three of section 3.1, `cargo test -p bot` (the in-process swarm).
- **Verify**: `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p game-server`;
  `cargo test -p bot`; then `cargo test --workspace` (the world loop is shared behaviour).
- **Commit**: `Load fixes 1: the world loop writes what the outbox holds in one write of up to 16 KB, each packet sealed in queue order, and the stats line counts writes`.
- **Done when**: committed; the four new tests and the existing ones pass; the report gives one stats line from the in-process swarm test
  if it prints one (otherwise says so).

### Piece 2. F3: the heartbeat renews only the sessions this server holds; a failed end is tried again

- **Files**: `crates/game-server/src/backend.rs`, `pg/heartbeat.rs`, `memory.rs`, `server.rs`, `server/tasks.rs`, `server/console.rs`,
  `crates/game-server/tests/postgres.rs`, `crates/game-server/tests/game_flow.rs`, `docs/server/architecture.md` (step 4 of the
  session's life, lines 57-59: "renews `expires_at` of every session it has claimed" becomes the sessions of the accounts it holds, with
  the restart sentence below).
- **Behaviour**:
  - `Heartbeat::heartbeat(server_id, players, open, online: &[u32])`, forwarded for `Arc` (backend.rs:1293). The caller passes
    `ctx.online.user_ids()` (a new `OnlineUsers` method: the accounts in the map, any order). Both callers (the task, the console's
    `/server on|off`) pass it.
  - PostgreSQL: the session update becomes `… WHERE server_id = $1 AND game_connected_at IS NOT NULL AND user_id = ANY($3)` with `$3`
    an `i32` array; an empty list updates nothing. The `game_servers` update is unchanged.
  - Memory: `last_heartbeat` also keeps the users (a second field, `last_heartbeat_users`, so existing asserts stay).
  - Ending a session: one helper `end_session(ctx, user_id, identity, why)` used at both places of `handle`. It tries `release` once
    inline (as now, before `drop(online)`); on an error it logs one `warn` and spawns a task that retries with
    `supervise::retried(&WRITE_RETRIES, …)` and logs the final outcome (`info` when a retry worked, `warn` "given up; it expires within
    120 s" when not). `handle` does not wait for the retries.
  - `MemoryBackend` gains `fail_releases: AtomicU32` (the next n `release` calls fail), in the form of `fail_next_trade`.
- **What happens on a restart** (write it in the doc sentence): the new process holds nobody, so no row of the old one is renewed; a row
  whose end was lost expires 120 s after the old process's last heartbeat and the login server deletes it a minute later. A player who
  logs in meanwhile gets a new row as always. Nothing is cleaned at start.
- **Invariants**: a session this server holds is renewed every heartbeat as before (claimed sessions on the character screen and in the
  world, and a character staying after its connection ended); a retry deletes only the identity it was given, so it can never end a newer
  login's session; the account leaves `OnlineUsers` at the same moment as before.
- **Tests to add**:
  - `postgres.rs` `the_heartbeat_renews_only_the_sessions_this_server_holds`: four accounts: A claimed on server 1 and named, B claimed on
    server 1 and not named (its end failed), C claimed on server 2 and named, D not yet claimed on server 1 and named. Set every
    `expires_at` to now + 5 s, heartbeat(1, …, &[A, C, D]): A has more than 100 s left; B, C and D still under 10 s.
  - `postgres.rs`: update `sessions_are_claimed_once_and_released` to the new signature (it names its user).
  - `game_flow.rs` `the_heartbeat_names_only_the_accounts_connected_here`: one player in the world: the users are `[7]`; after it quits
    and the session is released, `[]`.
  - `game_flow.rs` `a_session_end_that_fails_is_tried_again`: `fail_releases = 1`; a player quits; the session is gone within 2 s.
- **Verify**: fmt; clippy; `cargo test -p game-server --test postgres`; `cargo test -p game-server`; then `cargo test --workspace` (a
  trait every backend implements).
- **Commit**: `Load fixes 2: the heartbeat renews only the sessions of the accounts this server holds, and a session end that failed is tried again, so a lost end no longer keeps a row alive`.
- **Done when**: committed; the tests pass; the report states what a restart does, as above.

### Piece 3. F5: not a bug, pinned and written down

- **Files**: `crates/game-server/src/kc.rs` (a test), `crates/game-server/src/server/tests.rs` or `tests/game_flow.rs` (a test),
  `docs/testing/load-pvp.md` (F5's entry and the 1000 run's paragraph).
- **Behaviour**: no code change.
- **Tests to add**:
  - `kc.rs` `with_counting_off_kills_move_nothing_but_the_tick_still_sends_and_saves`: `disabled = true`; `add`, `sub` and `add_grade` give
    no change and the counts stay; `tick` at `PERIOD` gives `sent: [true, true], save: true` with the counts unchanged.
  - `kill_count_false_survives_loading_the_saved_counts`: a `Context` on the memory backend with `kill_count: false` and saved counts
    `[3, 4]`; after `load_kill_counts` the counts are `[3, 4]` and counting is still off (a player kill through the zone, or `add` on the
    context's `kill_counts`, moves nothing).
- **The note** (replace F5's text in `load-pvp.md`; keep its number): "**F5, answered: not a bug.** With `kill_count = false` a kill
  cannot move the counts (`kc.rs`, `frozen`). The counts read Light 0, Fury 1366 at every start since 2026-11-03, before the bots. The
  `INSERT INTO faction_kill_counts` is the ten-minute tick, which saves the unchanged counts whether counting is on or off, as the
  original's `CWorld::UpdateKillCount`. None of the slow statements was logged during the runs: the ones counted are from 2026-11-04 to
  11-11, before the databases moved to the SSD." And in the 1000 run's paragraph (84-87) replace the slow-statement sentence and the
  "17" with the figures of section 3.4 (1,225 slow acquisitions in one minute, 28 `pool timed out`, no slow statement).
- **Verify**: fmt; clippy; `cargo test -p game-server --lib kc`; the new server test by name; `python3 tools/check_links.py`.
- **Commit**: `Load fixes 3: with kill_count off the factions' counts cannot move; tests pin it, and the run page says the INSERTs were the ten-minute tick from before the runs`.
- **Done when**: committed; the two tests pass; `load-pvp.md` F5 and the 1000 paragraph match the log.

### Piece 4. F4a: the pool measured, and `load` on one connection

- **Files**: `crates/game-server/src/pg.rs` (a timed acquire), `pg/character.rs` (`load`, `load_mall`), `backend.rs` (a provided method
  and its `Arc` forwarding), `monitor.rs`, `server/tasks.rs` (`stats_figures`), `tests/game_flow.rs` (field list), `docs/server/settings.md`
  (`stats_every` row).
- **Behaviour**:
  - `PgBackend::acquire()` takes a connection from the pool and records the wait into new process-wide counters: `db_waits` (acquisitions
    that took 1 ms or more), `db_wait_ms` (their total), `db_wait_max_ms` (the longest since the last line).
  - `load` takes one connection with it and runs all fifteen statements on it (`&mut *conn`); `load_mall` takes `&mut PgConnection`
    instead of the pool. Nothing in `load` uses `self.pool` any more. No transaction (the reads are as consistent as before).
  - `Heartbeat::pool_gauges(&self) -> Option<(u32, u32)>` (the pool's size and its idle connections), provided as `None`, forwarded for
    `Arc`, `Some` for PostgreSQL. The line gets `db_size=`, `db_busy=` (size minus idle at the line), `db_waits=`, `db_wait_ms=`,
    `db_wait_max_ms=` after `out_wps=` (`-` for the gauges without a pool).
- **Invariants**: `load` returns the same character as before for the same rows; a connection is held for the whole load and given back
  on every path, an error included (the `PoolConnection` drop); no other connection is taken while it is held.
- **Tests to add**: `monitor.rs` `database_waits_are_summed_and_the_longest_starts_again` (local `Counters`); update
  `the_line_reads_from_fixed_figures` and the field list in `game_flow.rs`. The existing PostgreSQL tests that load characters
  (`characters_follow_the_original_rules`, `the_item_mall_takes_points_and_gives_items_together`, `friends_and_blocks_are_stored_once_and_load_without_the_deleted`,
  and the rest of `tests/postgres.rs`) must pass unchanged. Add `postgres.rs` `a_character_loads_on_a_pool_of_one_connection`: a
  `PgBackend` on a pool with `max_connections(1)` loads a character with items, bank, skills, quests, friends and a mall row.
- **Verify**: fmt; clippy; `cargo test -p game-server --test postgres`; `cargo test -p game-server`.
- **Commit**: `Load fixes 4: a character loads on one pooled connection instead of fifteen, and the stats line shows the pool and the waits for it`.
- **Done when**: committed; `grep -n 'self.pool' crates/game-server/src/pg/character.rs` shows none inside `load`.

### Piece 5. F4b: fewer round trips in `load`

- **Files**: `crates/game-server/src/pg/character.rs`, `tests/postgres.rs`.
- **Behaviour**: the account's name, points, premium and its end come with the character row (`JOIN accounts a ON a.id = characters.user_id`),
  so `load_mall` no longer reads `accounts` and the separate `SELECT username` goes; the items and the bank come in one statement
  (`SELECT … FROM character_items WHERE character_id = $1 UNION ALL SELECT 100::int2, … FROM bank_items WHERE user_id = $2 AND server_id = $3
  ORDER BY bag, slot`), split by bag 100 as now. Fifteen statements become twelve. The rest stays.
- **Invariants**: the same `PlayCharacter` for the same rows (bank order by slot, items by bag then slot). A plain `JOIN` is safe:
  `characters.user_id` references `accounts` (`db/migrations/0005_characters.sql`:9), so a character always has its account.
- **Tests to add**: `postgres.rs` `a_loaded_character_has_its_bank_its_bags_and_its_account_fields`: a character with bag items, two bank
  items, points 7 and a premium with an end: every field as written, the bank apart from the bags.
- **Verify**: fmt; clippy; `cargo test -p game-server --test postgres`; `cargo test -p game-server`.
- **Commit**: `Load fixes 5: a character loads in twelve statements instead of fifteen, the account's fields with its row and the bank with its bags`.
- **Done when**: committed; tests pass.

### Piece 6. F4c: the heartbeat's session writes; the pool size stays

- **Files**: `crates/game-server/src/pg/heartbeat.rs`, `pg.rs` (a constant), `tests/postgres.rs`, `docs/server/settings.md` (the
  `db_pool` row: one sentence on what entering costs and that more connections than about twice the cores do not help on one box),
  `docs/server/architecture.md` (step 4: renewed when under 90 s are left).
- **Behaviour**: the session update renews only rows with less than `SESSION_RENEW_BELOW` (90 s) left:
  `… AND expires_at < now() + make_interval(secs => $4)`. With a heartbeat every 5 s a held session always has 85 to 120 s left, and is
  written about every 30 s instead of every 5 s. `db_pool` stays 24.
- **Invariants**: a held session never has less than 85 s left while the heartbeat runs; a `heartbeat_secs` up to 60 still leaves 30 s
  (say so in the constant's comment).
- **Tests to add**: `postgres.rs` `a_held_session_is_renewed_only_when_it_runs_low`: a held session with 110 s left keeps its
  `expires_at` (compare before and after); one with 80 s left gets about 120 s.
- **Verify**: fmt; clippy; `cargo test -p game-server --test postgres`.
- **Commit**: `Load fixes 6: the heartbeat rewrites a held session only when it has under 90 s left, a sixth of the row writes`.
- **Done when**: committed; tests pass.

### Piece 7. F2a: the world loop keeps the next ban end

- **Files**: `crates/game-server/src/server.rs`, `server/admin.rs`, `tests/game_flow.rs`.
- **Behaviour**: `WorldState` gets `ban_end: Option<Instant>`, set at entry from `admin::next_ban_end` (it reads `bans` once), and set
  again after every `AdminOrder::Ban` and every `bans_ended`. The `select!` arm sleeps on `state.ban_end`; nothing in the loop's turn
  takes `bans` any more.
- **Invariants**: a ban that runs out ends at the same moment as before; a ban set or lifted by a GM while the player is in the world is
  seen at once (both go through the player's own task).
- **Tests to add**: `game_flow.rs` `a_timed_move_ban_ends_by_itself_in_the_world`: a move ban with an end 300 ms away reaches a player in
  the world (through the zone's admin order queue as a GM's `/stop` would put it; if the order carries only whole minutes, build the
  `Until::At` directly in the test); within 2 s the player gets `OP_STOP_OFF` without sending anything. The existing ban tests
  (`gm_chat_reaches_the_view_and_chat_bans_silence_a_player`, `account_bans_notify_and_disconnect_and_can_be_lifted`) pass.
- **Verify**: fmt; clippy; `cargo test -p game-server`.
- **Commit**: `Load fixes 7: the world loop keeps its player's next ban end instead of locking the bans at every wake`.
- **Done when**: committed; `grep -n next_ban_end crates/game-server/src/server.rs` shows no call inside `select!`.

### Piece 8. F2b: the `maps` guard, and how long it is held

- **Files**: `crates/game-server/src/zone.rs` and every `zone/*.rs` that takes `maps`, `monitor.rs`, `server/tasks.rs`,
  `tests/game_flow.rs` (field list), `docs/server/settings.md`.
- **Behaviour**:
  - `Zones::maps(&self) -> MapsGuard<'_>`: a struct holding the `MutexGuard` from `self.maps.guard()` and a hold timer; `Deref` and
    `DerefMut` to the map. Every `self.maps.guard()` in non-test zone code becomes `self.maps()`. The tests may keep `zones.maps.guard()`.
  - The timer records the hold (from taking to dropping) into `maps_held_ms` (total) and `maps_held_max_ms` (longest since the last line),
    appended to the line after the database fields. Field order: the guard first, the timer second (so the hold is measured to the
    unlock).
  - No behaviour change otherwise.
- **Invariants**: the `Mutex`'s type is unchanged (the lock table still says `maps`); lock order unchanged; `maps_held_ms` over the
  interval is the lock's busy share (it can never exceed the interval).
- **Tests to add**: `zone/tests.rs` `holding_maps_is_timed`: take `zones.maps()`, sleep 20 ms, drop; the process-wide total rose by at
  least 20 ms (only lower bounds: other tests add to it); `monitor.rs` the fixed-figures line updated.
- **Verify**: fmt; clippy; `cargo test -p game-server`.
- **Commit**: `Load fixes 8: the maps lock is taken through one guard that times how long it is held, shown on the stats line`.
- **Done when**: committed; `grep -rn 'self.maps.guard()' crates/game-server/src --include=*.rs | grep -v tests.rs` is empty.

### Piece 9. F2c: sends deferred while `maps` is held

- **Files**: `crates/game-server/src/outbox.rs`, `zone.rs` (`MapsGuard`), `zone/tests.rs`, `docs/server/audit.md` (R1's paragraph on the
  outbox: one sentence on the deferral).
- **Behaviour** (design B2, section 4):
  - `outbox.rs`: a thread-local `Deferred` list (`depth: u32`, `queue: Vec<(Outbox, Vec<u8>)>`); `outbox::defer() -> DeferScope`
    raises the depth; dropping the scope lowers it and, at 0, takes the queue and sends each packet in order (the same code a direct send
    runs). `Outbox::send` with a depth above 0 pushes `(self.clone(), packet)` and returns `Ok(())`. `DeferScope` is `!Send`
    (`PhantomData<*const ()>`). The queue's capacity is kept, cut back to 4096 entries when it grew past that.
  - `MapsGuard` gains the scope as its **last** field: lock, timer, scope; dropped in that order, the packets go out after the unlock.
  - A deferred send's outcome is applied at delivery: a closed outbox drops it, a full one drops it and sets `overflowed` (as a direct send).
- **Invariants**: those of section 4. Also: `ZoneEvent` channels are not deferred (their order against packets was never kept: two
  channels in an unbiased `select!`); nothing reads an outbox's `queued()` or a send's result under `maps` (all callers ignore it; check
  with `grep -rn 'outbox.send(' crates/game-server/src`).
- **Tests to add**:
  - `outbox.rs` `sends_inside_a_scope_wait_for_its_end_and_keep_their_order` (two outboxes, interleaved sends, nothing received inside,
    everything in order after).
  - `outbox.rs` `a_deferred_send_past_the_limit_overflows_at_delivery`.
  - `outbox.rs` `nested_scopes_deliver_once_at_the_outer_end`.
  - `outbox.rs` `a_panic_inside_a_scope_delivers_and_leaves_no_scope_open` (`catch_unwind`; afterwards a send outside is received at once).
  - `zone/tests.rs` `packets_sent_under_the_maps_lock_arrive_when_it_is_released`: a placed player's outbox is empty while a `zones.maps()`
    guard that sends to it lives, and holds the packet after it drops.
  - The whole zone suite (123 tests) and the flow tests pass unchanged; `attack_player`'s death-before-blow order is covered by the
    existing PvP tests (name the one that checks it in the report, or add `a_killing_blow_sends_the_death_before_the_blow` if none does).
- **Verify**: fmt; clippy; `cargo test -p game-server`; `cargo test -p bot`; then `cargo test --workspace`.
- **Commit**: `Load fixes 9: what is sent while the maps lock is held is queued in order and delivered once it is released, so the fan-out's channel pushes and wake-ups leave the lock`.
- **Done when**: committed; all tests pass.

### Piece 10. F2d: a step's follow-up checks

- **Files**: `crates/game-server/src/zone/trade.rs`, `zone/duel.rs`, `zone/guild_war.rs`, `server.rs`, `zone/tests.rs`.
- **Behaviour**: `trade_moved` first asks `trades` alone for a partner and returns without taking `maps` when there is none (it takes
  `trades` and releases it before `maps`; with a partner it then takes `maps`, then `trades`, in the lock order, and checks again).
  `duel_moved` and `war_moved` become one `Zones::left_areas(map, me, now)` that takes `maps` once and runs the duel check, then the war
  check, with the same effects in the same order. Both step paths of server.rs (2249-2251 and 2262-2264) call it. A step then takes
  `maps` three times instead of five.
- **Invariants**: the same packets for a trade partner too far, a duel left, a war area left, in the same order; the lock order kept.
- **Tests**: the existing zone tests that call `duel_moved` and `war_moved` (`zone/tests.rs` 2993-3059, 3426-3479) are moved to
  `left_areas` with the same assertions; add `a_trade_ends_when_a_step_takes_the_partners_apart` if no flow test covers it (name the
  one that does in the report otherwise).
- **Verify**: fmt; clippy; `cargo test -p game-server`; `cargo test -p bot`; then `cargo test --workspace` (the last code piece).
- **Commit**: `Load fixes 10: a step takes the maps lock three times instead of five, the trade checked first and the duel and war areas in one pass`.
- **Done when**: committed; the workspace passes.

### Piece 11a. The runs, and everything put back

Bots only (C9). Every server command runs in its own Bash call (a fresh shell); the run's variables are given on the command line, never
exported; database queries run in their own calls too (the `tools/dbconn.sh` hazard: it exports `.env`'s variables only when they are
not set, so a shell that set one keeps it). Nothing here runs a test or a second cargo command while a run is going.

1. **Save and check**: `cp config/game.toml logs/game.toml-before-load-fixes`. If it holds a `# LOAD TEST` block (section 3.6), the
   restore target is that file without the block; check `diff` of that against `logs/game.toml-before-load-test` is empty and note it.
   Record the servers' environment: `for p in $(pgrep -f 'target/release/(game|login)-server'); do tr '\0' '\n' < /proc/$p/environ | grep -E '^(RUST_LOG|LOGIN_|GAME_CONFIG)'; done`.
   Check memory and disk: `free -m` (at least 2 GB available) and `df -h /media/drive0` (at least 10 GB free); otherwise stop.
2. **Build**: `cargo build --release --workspace` (one cargo).
3. **Settings**: the block of `load-pvp.md` "Settings for a run", dated today, at the top of `config/game.toml` (one block only). Check:
   `GAME_CONFIG=config/game.toml target/release/game-server config | grep -E 'max_per_ip|connects|kill_count|stats_every'`.
4. **Restart with the run's environment**: `tools/game-server.sh stop` then `tools/login-server.sh stop`;
   `env RUST_LOG=info LOGIN_MAX_PER_IP=1020 LOGIN_CONNECTS_PER_MINUTE=1200 LOGIN_KEY_POOL=32 tools/login-server.sh start`;
   `env RUST_LOG=info tools/game-server.sh start`. Both must say started; `tools/game-server.sh logs 40` shows `stats line on` and
   `counting=false`. A server that does not start is a stop (section 6).
5. **Bots**: pick a password for this session (`SHAIYA_PASSWORD`, never written to a file or a commit). `target/release/bot unseed`
   first (bots left from an earlier run have a password nobody knows; refused while a bot has a game session: wait 3 minutes, which the
   fixed heartbeat lets expire, and try once more), then `SHAIYA_PASSWORD=… target/release/bot seed --count 1000` (covers all three runs;
   a second seed before each run puts the bots back).
6. **The runs**, each with `bot seed --count N` first, its start and end times (UTC) noted, and its `--report` in `logs/load-pvp/`:
   - **R2-300**: `bot swarm --login 192.168.1.240:30800 --count 300 --ramp 5 --duration 10m --report logs/load-pvp/R2-300.json`.
   - **R2-700**: the same with `--count 700`, `R2-700.json`.
   - **E2-1000**: `--count 1000 --ramp 10 --duration 4m`, `E2-1000.json` (entering only).
   A run lasts over 10 minutes: run `bot swarm` in the background (`run_in_background`) with its output to
   `logs/load-pvp/<name>-swarm.txt` and wait for it to end; sample every 5 s into `logs/load-pvp/<name>-cpu.log` in the form of
   `logs/load-pvp/R-700-cpu.log` (time, load average, the game server's and the bots' CPU and RSS, available memory, swap), with the
   CPU of the last 5 s (`top -b -d 5 -p <game pid>,<bot pid>` or `pidstat 5`, not `ps`'s average since start), and stop the sampler
   after.
7. **Collect** per run, by the run's time window (not the whole log): the swarm's summary; the stats lines
   (`sed 's/\x1b\[[0-9;]*m//g' logs/game-server.log | awk '$1 >= "<start>" && $1 <= "<end>"' | grep 'stats: secs'`); the counts of
   `time to acquire`, `pool timed out`, `slow statement`, `could not end the session` in the window; the figures of `load-pvp.md`'s table
   plus `out_wps` (and packets per write = `out_pps / out_wps`), `maps_held_ms`, the `db_` fields. Write them to
   `logs/load-pvp/R2-summary.txt` (not in git).
8. **Judge** (section 7). If a target is missed, follow section 7's one more round, then go on.
9. **Put everything back**, whatever happened above: `bot unseed`; remove the LOAD TEST block (the file must equal the restore target of
   step 1: `diff` empty); `env -u RUST_LOG -u LOGIN_MAX_PER_IP -u LOGIN_CONNECTS_PER_MINUTE -u LOGIN_KEY_POOL tools/game-server.sh restart`
   and the same for `tools/login-server.sh restart`; then check the environment as in step 1 (no `LOGIN_` variables; `RUST_LOG` the
   script's default `info,game_server=debug`), `tools/game-server.sh logs 40` shows `stats line off` and `counting=true`, both
   scripts' `status` say running, and the game server's log has no `heartbeat failed` in the minute after.
- **Files committed**: none (the results are written by 11b). `logs/` is not in git.
- **Done when**: the three runs have their reports and summary in `logs/load-pvp/`; the settings, environment and bots are back as
  step 9 checks; the report holds the figures of step 7 and the judgement.

### Piece 11b. The write-up and the push

- **Files**: `docs/testing/load-pvp.md`, `docs/plans/load-fixes.md` (the head line and the progress table), `docs/plans/README.md`
  (the `load-fixes.md` line: built), `docs/plans/test-bots.md` only if a fact there changed.
- **Behaviour**:
  - `load-pvp.md`: a new section `## Results (<date>, after the load fixes)` after the 2026-11-12 one, with the same table for R2-300 and
    R2-700 side by side with the 2026-11-12 300 and 700 columns, the new rows (packets per write, `maps` held per 10 s, pool waits), the
    entry check's lines (in the world of 1000, pool waits, `pool timed out`), and three short paragraphs as before: what held, what limits
    it now, entering. "Settings for a run" gets the restore check of piece 11a step 9 (the `/proc` environment check) and the `unseed`
    first.
  - The findings: each gets its status and the commit (`git log --oneline` for the hashes): F2 "fixed in part" or "fixed" by the
    figures, with pieces 7 to 10; F3 fixed (piece 2), and its text corrected by section 3.7 (a player is not locked out; a restart did
    not clear it); F4 fixed or "better" by the entry check, with pieces 4 to 6 and the pool waits; F5 as piece 3 left it. A target
    missed is a new open finding with the figures, not a reworded old one.
  - `load-fixes.md`: the head line says built with the date; every progress row done with its commit.
- **Verify**: `python3 tools/check_links.py`; `git status` clean apart from these files; `git log origin/main..main --oneline` lists the
  pieces' commits.
- **Commit**: `Load fixes 11: the 300 and 700 runs and the entry check after the fixes, the findings' status, and the plan marked built`.
- **Push**: `git push` (on `main`), when every piece is accepted and the workspace tests of the pieces passed, also when a run target was
  missed (the miss is in the write-up as an open finding). If it fails, stop and report the error; do not change remotes, keys or settings.
- **Done when**: pushed, or the push failure reported.

## 6. What a reviewer checks, piece by piece

Every piece: the commit's files are exactly the piece's (plus what the report justifies); the message is in the project's style with no
AI trailer or mention; `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean on the reviewed commit;
the tests named in the piece exist with those names and pass (run them); nothing points a test at `DATABASE_URL`; no `.unwrap()` on a
path a client can reach; comments say what and why in the project's voice; docs that name a changed behaviour are updated.

**Piece 1**: `seal` runs once per packet on that packet's bytes, never on the batch; seal order = `try_recv` order = byte order on the
wire; the AES test decrypts with one continuous stream; the batch buffer is reused and `first` is not copied when alone; the soft cap
is as specified and tested; a closed connection takes nothing more; `write_done` is bumped after a successful write only, for direct sends
too; `out_wps` is appended after `workers=`; the three flow tests of section 3.1 pass; the comment no longer claims what `load-pvp.md`
does not say; the workspace passed.

**Piece 2**: the SQL has `user_id = ANY($3)` with a typed `i32` array and an empty list updates nothing; `game_servers` is still updated
and its "not registered" error kept; both callers pass `user_ids()`; the `Arc` forwarding passes the list; the retry deletes by user and
identity only; `handle` does not await the retries; `drop(online)` happens where it did; the four-account test covers held, orphaned,
other server, unclaimed; the doc sentence states the restart behaviour of the piece; the workspace passed.

**Piece 3**: no non-test code changed; the kc test covers `add`, `sub`, `add_grade` and `tick` with counting off; the second test goes
through `load_kill_counts`; the `load-pvp.md` wording matches section 3.3's facts and dates, and the 1000 paragraph's numbers match
section 3.4.

**Piece 4**: no `self.pool` inside `load`; `load_mall` takes the connection; the acquire timer records only waits of 1 ms or more and the
max resets per line; `pool_gauges` is forwarded for `Arc` (otherwise the line would always say `-`); the line's new fields are after
`out_wps=` in the specified order; the pool-of-one test exists; all PostgreSQL tests pass.

**Piece 5**: twelve statements; the bank still sorted by slot and kept apart by bag 100; a character without an account row gives the
same outcome as before (an error), shown in the report; the new test checks the account fields and the bank.

**Piece 6**: the renewal threshold is a named constant with the reasoning in its comment; the `$4` bind is seconds as `f64` like `$2`;
the test compares `expires_at` before and after for the 110 s row; the settings sentence on `db_pool` is accurate and the default is
unchanged.

**Piece 7**: `ban_end` is refreshed after every path that changes the player's bans (`AdminOrder::Ban`'s every arm, `bans_ended`) and set
at entry; no `bans` lock in the loop's turn; the new test waits for `OP_STOP_OFF` without the client sending anything.

**Piece 8**: every non-test `self.maps.guard()` is gone; `MapsGuard` declares the lock before the timer (and the comment says why); the
`Mutex` type is unchanged; the new fields are appended; no behaviour changed (the zone suite untouched apart from the new test).

**Piece 9**: `MapsGuard`'s field order is lock, timer, scope, with a comment that the order is the point; the scope is `!Send`; depth
counting and nesting are tested; delivery uses the same path as a direct send (byte count, overflow flag, monitor counters); delivery
cannot panic; the panic test passes; the thread-local queue's capacity is bounded as specified; no `.await` between taking `maps()` and
its drop anywhere (grep the zone and server for `maps()` followed by `.await` in the same scope); the zone suite, the flow tests, `bot`'s
swarm and the workspace pass.

**Piece 10**: `trade_moved` never holds `trades` while it takes `maps`; the merged function keeps the duel check before the war check and
both step paths call it; the moved zone tests keep their assertions; the workspace passed.

**Piece 11a**: the three reports and the summary exist with the right names; the windows are per run; the settings file equals the
restore target (`diff` shown); the `/proc` environment check shows no `LOGIN_` variables and the default `RUST_LOG`; the bots are
unseeded (the report shows the command's output); the servers are up and the game server is listed; the judgement follows section 7;
no source file changed; at most one extra round, as section 7 allows.

**Piece 11b**: the new results section is dated, its numbers are the summary's (spot-check three against `logs/load-pvp/R2-summary.txt`
and the reports); the old section is unchanged apart from section 3.7's corrections; each finding's status names real commits; a missed
target is recorded as an open finding; links check; the push went through or its failure is reported verbatim.

## 7. The runs: success, regression, and what if not

- **R2-700 holds** when: 700 of 700 in the world; 0 disconnected by the server; 0 refusals and 0 protocol errors; attack acknowledgement
  p95 under 250 ms; step echo p95 under 500 ms; logout clean at least 697 of 700.
- **R2-300 is a regression** when any of: fewer than 300 in the world; any disconnect by the server; refusals or protocol errors above 0;
  attack acknowledgement p95 above 36 ms or step echo p95 above 51 ms (2026-11-12: 16 and 31 ms; the limit is the larger of twice that and
  that plus 20 ms); server CPU average above 81 % (61 % plus 20 points).
- **E2-1000 enters** when: no `pool timed out` in its window and at least 990 of 1000 in the world. Its latencies are not judged.
- **What is compared** with 2026-11-12: every row of `load-pvp.md`'s table, plus packets per write, `maps` held and waited per 10 s, and the
  pool's waits.
- **If a target is missed or 300 regresses**: record the figures. **One more round at most**: either the run is repeated once when it was
  disturbed (another load on the box, a bot or server crash, a mistake in the settings), or one setting of the run is changed once and that
  run repeated (for E2-1000, `db_pool = 48` in the LOAD TEST block). No new code is written in that round. Then piece 11b writes what
  stands, the miss as an open finding with its figures and what the stats line points at, and the orchestrator reports it.

## 8. Risks

| risk | effect | answer |
|---|---|---|
| deferred sends reorder two near-simultaneous operations at a third player | a client sees, say, a death before the dead player's entry; the client drops a packet about an id it does not know yet, so it lasts until that unit's next update, and a dropped `0x0610` HP delta leaves a bar off for the fight | the client already sees such orders (section 3.5); the fallback of section 4 keeps the lock's order |
| a long operation under `maps` queues thousands of packets on one thread | memory per worker; one delivery burst | bounded by the operation (`enter` at 1000: about 3000 entries); the capacity is cut back |
| an `.await` while `maps()` is held, in a future that is not spawned | the deferral scope would span other tasks' work on that thread | the guard is `!Send`; reviewers grep; spawned tasks cannot compile it |
| batching delays a packet behind 16 KB of others | a few microseconds more per write | the write is one system call either way; the run measures the step echo |
| the heartbeat stops renewing a session it should hold | a player in the world loses its session after 120 s | `OnlineUsers` holds every claimed account until after its release; the flow test checks the named users |
| a release retry ends a newer login | the player is thrown out | the delete is by identity, which every login changes |
| one connection for the whole `load` | a slow load holds a connection longer | it held one at a time before, with 15 waits; now one wait |
| the runs disturb the maintainer's game | the servers restart and the maintainer is kicked | the maintainer gave the autonomy; the run piece puts everything back and checks it |
| the box runs short of memory during a build or a run | the OOM killer, a failed run | one cargo at a time; the run piece checks memory and disk first and stops |
| process-wide counters in tests run in parallel | flaky asserts | the tests use local `Counters` or lower bounds only |

## 9. Not in this work

- Per-map locks (design A), shared packet buffers (C), a spatial index (D).
- The leaving saves on one connection (position, wear, cooldowns, applied skills, quest times: five to six acquisitions per logout); next
  if E2-1000 still waits on the pool.
- Batching the heartbeat's player count, or changing `heartbeat_secs`, `db_pool` or `db_acquire_secs` defaults.
- Skipping the kill counts' ten-minute save when they did not change (it would depart from the original).
- The real client in the runs, the client's crashes (R2 and R3 with the maintainer, `load-pvp.md` "The real client").
- Moving the bots to another machine; runs above 1000; the 30-minute R5.
- The status board (`~/.local/bin/shaiya-status.sh`), outside the repository.

## 10. What the maintainer should answer (answered 2026-11-13, see section 1 items 7 and 8)

1. May the runs restart the servers at any time, or is there a window (the maintainer's own play)?
2. Is the relaxed cross-operation order of design B2 acceptable, or should piece 9 deliver under the second mutex from the start?
3. Keep piece 7 (the `bans` lock, not in the findings) and the entry check E2-1000?
4. Is 192.168.1.240 still the dev box's LAN address for `--login`?
