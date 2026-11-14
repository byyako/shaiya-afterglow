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

Back: remove the block (or copy the saved file back), and in a fresh shell without those variables
`tools/game-server.sh restart; tools/login-server.sh restart`.

## A run

1. `cargo build --release --workspace`, then the settings above and the restart.
2. `SHAIYA_PASSWORD=<the bots' password> target/release/bot seed --count N`. A second seed puts every bot back (gear, HP, position).
   Refused while a bot account has a session in the game (see "Open findings", F3).
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

## The real client

In the 300 run the client (`player1`, invisible GM) closed twice, at 05:13:53 and 05:18:20 UTC, each 2.5 to 4 minutes after entering
the battle. Both times the server saw a reset from the client's side (`Connection reset by peer`), with nothing queued for it
(`outbox_closed=0`, largest outbox under 3 KB). The maintainer: the client crashed, and **it did not seem to lag before it did**. So
the crash is not the server falling behind: it is the client itself, with 300 players in view. Not yet known: whether it crashes
without the bots' fight (300 standing still), whether it is the number in view or something one bot does, and whether an error is
left behind. That is the next question for R2 (100 bots with the client) and R3 (200).

## Open findings

- **F1, fixed: bots chased the invisible GM.** The server tells everyone about an invisible staff member as the original does (the
  entry, the steps, and shape 13, `zone.rs` `INVISIBLE_SHAPE`); the client draws nothing, but the bots ignored shapes and took the GM
  for the nearest enemy (the server then refused their attacks). Bots now read `0x051D` and skip a player in shape 13 (hiding or
  invisible) until it shows again (`crates/bot/src/world.rs`, test `a_hidden_player_is_no_target_until_shown_again`). The 700 run
  used the fix.
- **F2, stats: the `maps` lock and the step fan-out** are the scaling limit, as `docs/plans/test-bots.md` section 7 expected. The fix
  is its own change (per-map locks, or the view's recipients taken out of the lock before sending), with the 700 run repeated.
- **F3, a bug: a session whose end could not be written stays "in the game" until the server restarts.** In the 1000 run two entries
  failed while the database pool was dry, and the server logged `could not end the session of a failed handshake` (pool timed out).
  The heartbeat (`pg/heartbeat.rs`) then refreshes `expires_at` of every session `WHERE server_id = $1 AND game_connected_at IS NOT
  NULL`, whether or not the server holds that connection, so the row never expires. For a player that is an account locked out until a
  restart. Here the two rows (bot0836, bot0974) were deleted by hand. The fix: the heartbeat refreshes only the sessions of the users
  the server has online, or the failed end is retried.
- **F4, the database pool at entry**: 10 logins a second with 700+ in the world empties it. Look at what entering costs (the loads,
  the heartbeat's player-count update) and the pool size (`db_pool`) before a faster ramp.
- **F5, answered: not a bug.** With `kill_count = false` a kill cannot move the counts (`kc.rs`, `frozen`). The counts read Light 0,
  Fury 1366 at every start since 2026-11-03, before the bots. The `INSERT INTO faction_kill_counts` is the ten-minute tick, which saves
  the unchanged counts whether counting is on or off, as the original's `CWorld::UpdateKillCount`. None of the slow statements was
  logged during the runs: the ones counted are from 2026-11-04 to 11-11, before the databases moved to the SSD (one more `INSERT`, on
  2026-11-14, is after the runs). Pinned by `kc.rs` `with_counting_off_kills_move_nothing_but_the_tick_still_sends_and_saves` and
  `tests/game_flow.rs` `kill_count_false_survives_loading_the_saved_counts`.
