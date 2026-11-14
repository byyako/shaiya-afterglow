# load-pvp: the PvP swarm runs

The bots of `docs/plans/test-bots.md` fighting in one place on the dev box's game server, to find how large a battle the server and
the 2010 client hold. This page has the run's settings, the steps, and the results of each run.

## Settings for a run

Only for a run, set back after (`docs/plans/test-bots.md` section 9). `config/game.toml`, at the top of the file (top-level keys must
come before the first `[table]`), between two marker comments so the block is easy to take out:

```toml
# LOAD TEST <date>, remove after the run
max_per_ip = 1020            # count + 20: every bot comes from one address
connects_per_minute = 1200   # 60 x ramp, with room
kill_count = false           # the factions' kill counts do not move with bot kills
stats_every = 10             # the stats line
# END LOAD TEST
```

Check with `GAME_CONFIG=config/game.toml target/release/game-server config | grep -E 'max_per_ip|connects|kill_count|stats_every'`.
The login server and the log level come from the environment (`.env` does not set them):

```sh
tools/game-server.sh stop; tools/login-server.sh stop
export RUST_LOG=info LOGIN_MAX_PER_IP=1020 LOGIN_CONNECTS_PER_MINUTE=1200 LOGIN_KEY_POOL=32
tools/login-server.sh start; tools/game-server.sh start
```

Back: `bot unseed`, remove the block (or copy the saved file back; `diff` it against the file saved before the run, which must be
empty), and in a fresh shell without those variables `tools/game-server.sh restart; tools/login-server.sh restart`. Then check that it
is back, by the processes' own environment and not by what the shell remembers:

```sh
for p in $(pgrep -f 'target/release/(game|login)-server'); do tr '\0' '\n' < /proc/$p/environ | grep -E '^(RUST_LOG|LOGIN_|GAME_CONFIG)'; done
```

It shows no `LOGIN_` variable and `RUST_LOG` as the script's default (`info,game_server=debug`); `tools/game-server.sh logs 40` shows
`stats line off` and `counting=true`; both scripts' `status` say running; the game server's log has no `heartbeat failed` in the
minute after.

The settings after the 2026-11-12 runs were **not** put back: at the start of the load-fixes work (2026-11-13) the block and the
variables were still live in the running servers, and they were put back on 2026-11-14 (at 09:19 UTC, after the runs). After the 2026-11-14 runs `config/game.toml`
equals `logs/game.toml-before-load-test` (no LOAD TEST block), and both servers were restarted with clean environments at 09:19 UTC.

## A run

1. `cargo build --release --workspace`, then the settings above and the restart.
2. `target/release/bot unseed` first when bots from an earlier run exist: `bot seed` on existing bots does not change their password,
   so bots left from a run whose password nobody has cannot be used by `bot swarm`. Then
   `SHAIYA_PASSWORD=<this session's password> target/release/bot seed --count N` (a seed after an unseed gives every bot that
   password; never write it to a file or a commit). A second seed before each run puts every bot back (gear, HP, position). Both
   commands are refused while a bot account has a session in the game (see "Open findings", F3): a lost session row now expires within
   120 s, so wait 3 minutes and try once more.
3. `bot swarm --login 192.168.1.240:30800 --count N --ramp 5 --duration 10m --report logs/load-pvp/R-N.json`.
   **`--login` must be the LAN address**, not `127.0.0.1`: the server list sends the bots to the game server at its listed address,
   and the game server only accepts a session from the address that logged in. With `127.0.0.1` every handshake is refused
   ("no matching session").
4. The server's stats lines: `sed 's/\x1b\[[0-9;]*m//g' logs/game-server.log | grep 'stats: secs'`.
5. `bot unseed` removes the bots.

`--duration` counts from the first login, so a 700-bot run at 5 a second has 140 s of ramp and 450 s of steady state.

## Results (2026-11-12, dev box: 4 cores, 7.6 GB; bots and server on the same box)

Map 18, battle point (636, 394), ramp 5 a second unless noted, `--duration 10m`, the real client (`player1`, GM, invisible) in the
battle for part of the 300 run. Reports and CPU samples in `logs/load-pvp/` (not in git).

| | 300 | 700 | 1000 (ramp 10) |
|---|---|---|---|
| in the world | 300 of 300 | 700 of 700 | 712 of 1000 (288 failed at entry) |
| disconnected by the server | 0 | 0 | 5 |
| attack acknowledgement p50 / p95 / p99 | 3 / 16 / 29 ms | 36 / 131 / 185 ms | 63 / 163 / 220 ms |
| step echo p50 / p95 / p99 | 9 / 31 / 63 ms | 81 / 962 / 3234 ms | 502 / 6194 / 9553 ms |
| attack round trip p50 / p95 | 126 / 921 ms | 262 / 1144 ms | 572 / 4017 ms |
| per bot received | 207 packets/s (4.4 KB/s) | 430 packets/s (9.1 KB/s) | 427 packets/s (9.1 KB/s) |
| server out (stats line, steady) | 40 to 95 k packets/s, 1 to 2 MB/s | 250 to 325 k packets/s, 5.5 to 7 MB/s | flat at 275 to 320 k packets/s |
| outboxes queued (sum) | 0 to 3 KB | 0 to 3 MB in waves, 24 MB at the logout | up to 29 MB |
| `maps` lock wait per 10 s, longest | 0.6 s, 12 ms | 1.6 to 2.6 s, 30 ms | 2.5 to 3.6 s, 43 ms |
| monster pass max | 9 to 16 ms | 12 to 30 ms | up to 43 ms |
| server CPU avg / max (of 400 %) | 61 / 100 % | 221 / 288 % | 190 / 284 % |
| bots CPU avg / max | 27 / 80 % | 71 / 92 % | 67 / 91 % |
| load average max | 4.5 | 7.6 | 6.9 |
| server RSS | 78 MB | 241 MB (after all runs) | 217 MB |
| kills / deaths / rebirths | 1474 / 1503 / 1503 | 2902 / 2967 / 2992 | 618 / 652 / 628 (110 s steady) |
| refusals, protocol errors | 0, 0 | 0, 0 | 0, 0 |
| logout clean | 300 of 300 | 697 of 700 | 705 of 707 |

The step echo maximum is about 30 s in every run, even at 300 where p99 is 63 ms: most likely a stamped step matched late (a bot that
died or left the view), not a real 30 s delay. Read p95 and p99.

**What held**: 300 holds by every measure (the spec's targets are a p95 attack acknowledgement under 250 ms and a p95 step echo under
500 ms). 700 all get in and nobody is dropped, and the acknowledgement holds (p95 131 ms), but the p95 step echo is 962 ms: the others
see a player move up to a second late. Sending levels off at about 300 k packets a second, and the outboxes fill in waves as the crowd
bunches. **The knee on this box is between 300 and 700 in one place**; a run at 500 would place it.

**What limits it**: the server's CPU, not the bots. Every step goes to everyone in view, so the traffic grows with the square of the
crowd: 300 to 700 is 2.3 times the players and about 4 times the packets. The bots used under a core at 700 and 1000; moving them to
another machine would free that core but not move the knee much. The `maps` mutex is the lock that waits (the spec's expectation),
up to a quarter of the time across the 4 workers at 700 to 1000.

**1000 at 10 a second**: entering is the first thing to break. The game server's database pool ran dry: the log has 1,225 slow
acquisitions (past 2 s; median 2.7 s, longest 6.1 s), 1,221 of them in the one minute 05:26, and 28 `pool timed out` (25 dropped
connections, two failed session ends and one lost game-log batch); no slow statement was logged. 288 bots gave up waiting for the
burst after selecting their character, and do not reconnect. The 700 run at 5 a second had no pool waits at all.

## Results (2026-11-14, after the load fixes)

The same box (4 cores, 7.6 GB; bots and server together), the same battle (map 18, point (636, 394)), bots only (no real client:
`docs/plans/load-fixes.md` C9), on the build of `6c3c5a9` (the load fixes of pieces 1 to 10). R2-300 ran 08:51:04 to 09:01:35 UTC,
R2-700 09:02:21 to 09:12:52 (both ramp 5 a second, `--duration 10m`) and E2-1000 09:13:34 to 09:18:06 (ramp 10, `--duration 4m`,
entering only, so 130 s of steady state and 15 stats lines). The settings were the block above; `db_pool` was the default 24. Reports,
CPU samples and the full collection are in `logs/load-pvp/` (`R2-300.json`, `R2-700.json`, `E2-1000.json`, `R2-summary.txt`; not in
git). "Steady" means the stats lines with at least 99 % of the bots in the world. Each column is followed by the 2026-11-12 figure
(the old run, from the table above).

| | R2-300 | 2026-11-12 | R2-700 | 2026-11-12 | E2-1000 (ramp 10) | 2026-11-12 |
|---|---|---|---|---|---|---|
| in the world | 300 of 300 | 300 of 300 | 700 of 700 | 700 of 700 | 1000 of 1000 | 712 of 1000 |
| disconnected by the server | 0 | 0 | 0 | 0 | 0 | 5 |
| attack acknowledgement p50 / p95 / p99 | 3.2 / 11.6 / 20.3 ms | 3 / 16 / 29 ms | 4.1 / 16.4 / 26.4 ms | 36 / 131 / 185 ms | 7.5 / 25.3 / 37.3 ms | 63 / 163 / 220 ms |
| step echo p50 / p95 / p99 | 6.3 / 20.3 / 30.0 ms | 9 / 31 / 63 ms | 7.8 / 25.3 / 37.3 ms | 81 / 962 / 3234 ms | 15.0 / 39.0 / 60.1 ms | 502 / 6194 / 9553 ms |
| attack round trip p50 / p95 | 41 / 921 ms | 126 / 921 ms | 93 / 882 ms | 262 / 1144 ms | 211 / 962 ms | 572 / 4017 ms |
| per bot received | 229 packets/s (4.9 KB/s) | 207 (4.4 KB/s) | 478 packets/s (10.2 KB/s) | 430 (9.1 KB/s) | 592 packets/s (12.6 KB/s) | 427 (9.1 KB/s) |
| server out (stats line, steady) | 43 to 130 k packets/s (median 64.6 k), 0.9 to 2.8 MB/s | 40 to 95 k, 1 to 2 MB/s | 84 to 706 k (median 306 k), 1.4 to 15.3 MB/s | 250 to 325 k, 5.5 to 7 MB/s | 403 to 656 k (median 563 k), 7.8 to 14.5 MB/s | flat at 275 to 320 k |
| outboxes queued (sum), most | 146 KB (median 1.2 KB) | 0 to 3 KB | 160 KB (median 19 KB) | 0 to 3 MB in waves, 24 MB at the logout | 213 KB (median 13 KB) | up to 29 MB |
| `maps` lock wait per 10 s (total of the waits), median (most); longest single wait | 1.19 s (1.52 s); 21.6 ms | 0.6 s; 12 ms | 2.70 s (4.38 s); 27.5 ms | 1.6 to 2.6 s; 30 ms | 5.80 s (7.15 s); 20.0 ms | 2.5 to 3.6 s; 43 ms |
| monster pass max | 12.3 to 21.4 ms | 9 to 16 ms | 10.5 to 31.6 ms | 12 to 30 ms | 12.8 to 31.2 ms | up to 43 ms |
| server CPU avg / max (of 400 %), steady (whole run) | 42 / 56 % (38 %) | 61 / 100 % | 152 / 212 % (124 %) | 221 / 288 % | 187 / 217 % (131 %) | 190 / 284 % |
| bots CPU avg / max, steady | 27 / 35 % | 27 / 80 % | 86 / 114 % | 71 / 92 % | 113 / 126 % | 67 / 91 % |
| load average max | 3.7 | 4.5 | 6.6 | 7.6 | 7.7 | 6.9 |
| server RSS | 83 MB | 78 MB | 134 MB | 241 MB (after all runs) | 151 MB | 217 MB |
| kills / deaths / rebirths | 1584 / 1625 / 1625 | 1474 / 1503 / 1503 | 3094 / 3196 / 3223 | 2902 / 2967 / 2992 | 1220 / 1234 / 1276 (130 s steady) | 618 / 652 / 628 (110 s steady) |
| refusals, protocol errors | 0, 0 | 0, 0 | 0, 0 | 0, 0 | 0, 0 | 0, 0 |
| logout clean | 300 of 300 | 300 of 300 | 699 of 700 | 697 of 700 | 958 of 1000 | 705 of 707 |
| **new rows** | | | | | | |
| packets per write (`out_pps` over `out_wps`), median (range) | 3.11 (2.09 to 6.42) | one packet per write | 2.92 (2.08 to 6.27) | one per write | 4.19 (3.71 to 5.77) | one per write |
| writes a second (`out_wps`), median | 21.6 k | not counted | 111 k | not counted | 134 k | not counted |
| `maps` held per 10 s (`maps_held_ms`), average (most) | 0.58 s = 5.8 % (0.69 s) | not measured | 1.67 s = 16.7 % (2.57 s) | not measured | 3.03 s = 30.3 % (3.55 s) | not measured |
| longest single hold of `maps`, median (most) | 7.7 ms (17.5 ms) | not measured | 10.3 ms (21.1 ms) | not measured | 12.2 ms (19.8 ms) | not measured |
| pool waits: `db_waits` (their total), steady; whole window | 0; 0 | not measured | 2 (5 ms); 5 (12 ms) | not measured | 27 (142 ms); 106 (357 ms), worst single 21 ms | 1,225 slow acquisitions, median 2.7 s |
| pool size, most in use | 22, 5 | not measured | 24, 3 | not measured | 24, 4 | not measured |
| `pool timed out`, `time to acquire`, slow statements in the log | 0, 0, 0 | none logged | 0, 0, 0 | none logged | 0, 0, 0 | 28, 1,225, 0 |

`db_waits` counts only the acquisition of `load`'s connection (piece 4), not every statement's. The step echo maximum is again about
30 s in every run (29.9 s, 30.0 s, 30.0 s): the stamped step matched late, as before; read p95 and p99. The old 300 run had the real
client in the battle for part of it; the new ones are bots only. Two things on the box, so the comparison is not exact: an older
sampler (a 500-bot baseline from an earlier session, `top` for one second every 5 s) was still running during all three runs, a small
extra load; and the 2026-11-12 runs used the old build, so the comparison is before and after the whole set of fixes (the write
batching, the heartbeat and session fixes, the pool at entry, the `bans` lock, the `maps` deferral and the step's follow-up checks),
not each piece alone. The kill-streak extension (`kill-streak/main.lua:35`, `game.notice_all: rate limit`) disabled itself in R2-300
at 08:55:28 (`scripts disabled: 5 errors in 60 s`), because of the battle's kill count, as it did in the old runs (05:14:23 in the old
300 run, 05:27:34 in the old 1000 run); R2-700 and E2-1000 ran without it, like the old 700. It is not a bug of this work.

**What held**: every target. R2-700 has all 700 in the world, nobody disconnected, no refusals or protocol errors, the p95 attack
acknowledgement at 16.4 ms (131 before; the target is 250) and the p95 step echo at 25.3 ms (962 before; the target is 500), the
p99 step echo at 37 ms where it was 3.2 s, the server's CPU at 152 % steady (221 before) with the traffic as large (a median 306 k
packets a second out, as the old 250 to 325 k), and the logout clean for 699 of 700. R2-300 is no worse in any row the spec judges
(the p95 acknowledgement 11.6 ms against 16, the step echo 20.3 against 31, the server CPU 42 % against 61, nobody dropped).
Each write now carries about three packets (2.9 to 3.1 at 300 and 700, 4.2 at 1000), where it carried one.

**What limits it now**: the server's CPU still grows with the crowd, and the `maps` lock is the busiest lock. The server uses 152 %
of the 400 % at 700 and 187 % at 1000 in the steady state; `maps` is held 17 % of the time at 700 and 30 % at 1000 (longest single
hold 21 ms and 20 ms); the traffic grows with the crowd (a median 306 k packets a second out at 700, 563 k at 1000, so 1.4 times
the players and 1.8 times the packets, and each bot receives 478 and then 592 packets a second). The knee is **not found**: 1000
entered and fought, with the latencies far inside the targets (the entry check does not judge them: ack p95 25.3 ms, step echo p95
39.0 ms), so the largest battle held on this box with bots is at least 1000. No larger run was made. At 1000 the bots take 113 % and the
load average reached 7.7 on 4 cores, so the box itself is the next limit, and the 1000 run lasted 4 minutes (130 s steady), not 10.
A run above 1000, and a longer one, would place the knee; the real client at 300 and above is still the question of the next
section.

**Entering**: E2-1000 enters everyone at 10 a second. All 1000 of 1000 are in the world (712 before), the last login at 100.0 s
(login p50 68 ms, p95 185 ms, longest 434 ms), the pool never ran dry: no `pool timed out` (28 before), no `time to acquire` (1,225
before), no slow statement, and `load`'s acquisitions waited 27 times for 142 ms in the worst steady 10 s and 106 times for 357 ms in
the whole window (the longest single wait 21 ms), with at most 4 of the 24 connections in use. What changed is a character's load
(on one pooled connection instead of 15 acquisitions, in 12 statements instead of 15) and the heartbeat's session rows (one write
in six); the runs do not say how much each did. The logout at 1000 is the bots' own gap, not the server's (F6).

## The real client

In the 300 run the client (`player1`, invisible GM) closed twice, at 05:13:53 and 05:18:20 UTC, each 2.5 to 4 minutes after entering
the battle. Both times the server saw a reset from the client's side (`Connection reset by peer`), with nothing queued for it
(`outbox_closed=0`, largest outbox under 3 KB). The maintainer: the client crashed, and **it did not seem to lag before it did**. So
the crash is not the server falling behind: it is the client itself, with 300 players in view. Not yet known: whether it crashes
without the bots' fight (300 standing still), whether it is the number in view or something one bot does, and whether an error is
left behind. That is the next question for R2 (100 bots with the client) and R3 (200). The 2026-11-14 runs were bots only, so the crashes are not rechecked after the load fixes: R2 and R3 with the client are still to do.

## Open findings

- **F1, fixed: bots chased the invisible GM.** The server tells everyone about an invisible staff member as the original does (the
  entry, the steps, and shape 13, `zone.rs` `INVISIBLE_SHAPE`); the client draws nothing, but the bots ignored shapes and took the GM
  for the nearest enemy (the server then refused their attacks). Bots now read `0x051D` and skip a player in shape 13 (hiding or
  invisible) until it shows again (`crates/bot/src/world.rs`, test `a_hidden_player_is_no_target_until_shown_again`). The 700 run
  used the fix.
- **F2, fixed by the figures (pieces 1, 7 to 10): the `maps` lock and the step fan-out** were the scaling limit, as
  `docs/plans/test-bots.md` section 7 expected. The write batching (piece 1, `f0896b4`), the world loop keeping its next ban end
  instead of locking `bans` at every wake (piece 7, `b7dd31e`), the `maps` guard with its hold timer (piece 8, `83bb08b`), the sends
  made under `maps` deferred until it is released (piece 9, `412f9c3`) and a step taking `maps` three times instead of five (piece 10,
  `06fb355`) together bring the 700 run from a p95 step echo of 962 ms to 25.3 ms with the CPU down from 221 % to 152 % (table
  above). Per-map locks were not done: the battle is all on map 18, so they would not have changed this run. The cost of piece 9's
  design: the order of sends is kept per thread, not across threads (below, "Open questions"). *Open question, no cause claimed*: the
  `maps` lock wait per 10 s is higher than on 2026-11-12 at 300 (1.19 s median against about 0.6 s) and at 700 (2.70 s against about
  1.8 s), although the latencies and the CPU are better. It is measured as before (`Relock::guard`, `monitor::lock_waited`, both
  unchanged), and `maps_held` does not include the wait (the hold timer starts after the lock is taken). The total wait is about
  twice the total hold time, and the longest single wait is above the longest hold in most 10 s windows, which fits a hand-off and
  scheduling delay on a loaded box (load average 3.7 to 7.7) more than longer holds, but it was not measured. It is not a criterion
  of the spec; F2's status rests on the latencies.
- **F3, fixed (piece 2, `04bfdb5`): a session whose end could not be written stayed "in the game".** In the old 1000 run two entries
  failed while the database pool was dry: the log has `could not end the session of a failed handshake` for user 981 and `could not
  end the session` (the normal end of a connection after the character screen) for user 843, both `pool timed out`. The heartbeat
  (`pg/heartbeat.rs`) refreshed `expires_at` of every session `WHERE server_id = $1 AND game_connected_at IS NOT NULL`, whether or not
  this process held that connection, so the row never expired. A player was **not** locked out: a new login replaces the account's
  row (`login-server/src/pg.rs`). What a stale row blocked was `bot seed` and `bot unseed`, which refuse while a bot has one
  (`crates/bot/src/seed.rs`); the two rows (bot0836, bot0974) were deleted by hand. A restart did not clear a stale row: the new
  process's heartbeat renewed it again, and it went only when the server was down for over 120 s. The fix is both ways: the
  heartbeat renews only the sessions of the accounts this server holds, and a failed end is tried again in the background (200 ms,
  1 s, 5 s). A restart now holds nobody, so a lost row expires within 120 s of the old process's last heartbeat, and the login server
  deletes it a minute later. The 2026-11-14 runs had no `could not end the session` at all, so the retry path is covered by the
  tests (`a_session_end_that_fails_is_tried_again`) but was not exercised by the runs. A side effect of piece 6 (a held row is
  rewritten only when under 90 s are left): a held session now tolerates about 85 s of failed heartbeats before its row expires (it
  was about 115 s) and is purged about 145 s after the last one; the old 1000 run's `pool timed out` window was about 90 s
  (05:26:12 to 05:27:42).
- **F4, fixed or better by the entry check (pieces 4 to 6): the database pool at entry.** 10 logins a second with 700+ in the world
  emptied the pool in the old 1000 run (1,225 slow acquisitions, 28 `pool timed out`, 288 bots given up). Entering took about 20
  statements, each taking its own connection from the pool, so a task waited in the queue for each of them. A `load` now takes one
  connection for all its statements (piece 4, `85d19af`, with the `db_` fields of the stats line) and runs 12 statements instead of 15
  (piece 5, `3876678`), and the heartbeat rewrites a session row only when it has under 90 s left (piece 6, `2f81384`, one write in six).
  E2-1000 enters all 1000 with no `pool timed out`, no slow acquisition and 106 waits of 357 ms in all (table above); `db_pool`
  stays 24, since raising it would not help on one box. The heartbeat's player-count update was named here before; it is not an
  entry cost: two statements every 5 s whatever the number of players. The leaving saves (five to six connections per logout) are
  not changed.
- **F5, answered: not a bug.** With `kill_count = false` a kill cannot move the counts (`kc.rs`, `frozen`). The counts read Light 0,
  Fury 1366 at every start since 2026-11-03, before the bots. The `INSERT INTO faction_kill_counts` is the ten-minute tick, which saves
  the unchanged counts whether counting is on or off, as the original's `CWorld::UpdateKillCount`. None of the slow statements was
  logged during the runs: the ones counted (36 `INSERT INTO faction_kill_counts` and 2,101 `UPDATE game_servers SET players`) are from 2026-11-04 to 11-11,
  before the databases moved to the SSD (one more `INSERT`, on 2026-11-14, is after the runs). Pinned by `kc.rs` `with_counting_off_kills_move_nothing_but_the_tick_still_sends_and_saves` and
  `tests/game_flow.rs` `kill_count_false_survives_loading_the_saved_counts`.
- **F6, new, open: the bots' logout leaves bots behind at 1000.** E2-1000 logged 958 of 1000 clean; R2-700 one of 700. From 09:17:36 the server logged `logout pending` (10 s) for all 1000 bots and `logout cancelled: attacked` 747 times for 57 of
  them (a few over and over: 158, 129, 116, 80 and 78 times); 15 of the 57 asked again and got out, and the other 42 had their logout
  cancelled by a hit and never asked again (each closed at 30 s: `connection ended in the world; the character stays linger=10s`),
  and the server freed all of them by 09:18:21. The log does not name the kind of hit; the code shows what the bots do not watch for:
  `crates/bot/src/bot.rs` `leave` (about lines 142 to 176) asks again only on `OP_ATTACK_USER` or a single-target skill
  (`OP_USE_USER`, `0x0511`) aimed at it, but the server also stands a sitting player up on area-skill hits (`zone.rs` `hit_player`
  with `single = false` sends `OP_USE_PARTY`, `0x0513`; monster hits do the same) and `crates/bot/src/world.rs` does not parse
  `0x0513`. That fits the 42 being struck by area-skill or monster hits. A later change to the bots should treat a `0x0513` hit, or
  any stand-up, as "struck". The one unclean bot in R2-700 shows the same pattern. Not fixed in this work; the server's side is as designed.

## Open questions

- **The real client** was not in the 2026-11-14 runs (bots only). Two things are unchecked with it: whether the swapped order of
  design B2 (piece 9; two operations on two threads can reach a third player's outbox in the other order than they held the lock)
  shows as artifacts on a real client, which drops a packet about an id it does not know yet (an appearance after a death, a hit on
  a just-spawned monster, a dropped `0x0610` HP delta leaving a bar off for the fight); and whether the client still crashes at 300
  in view (the 2026-11-12 crashes). R2 and R3 with the client are still to do. If the artifacts show, the fallback is to deliver the
  deferred sends under a second mutex taken before `maps` is released.
- **The `maps` wait** (F2): why the wait per 10 s is higher than on 2026-11-12 with better latency.
- **The knee**: no run above 1000 was made.
