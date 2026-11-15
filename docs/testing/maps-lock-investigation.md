# The `maps` lock wait: an investigation (2026-11-15)

No production code was changed; the instrumentation lived in a throwaway worktree (`target/maps-probe/wt`, gitignored) and is
described in section 2. Eleven 300-bot runs and one 700-bot run (server side) on the dev box on 2026-11-15.

The question (`docs/testing/load-pvp.md`, F2 and "Open questions"): in the 2026-11-14 runs the `maps` wait per 10 s (`lock_worst`
`maps ...ms/N` on the stats line) was higher than on 2026-11-12 (R2-300 1.19 s median against 0.62 s; R2-700 2.70 s against 1.79 s),
although the latencies and the CPU were better.

## 1. Short answer

- **The holds did not get longer.** A `maps` hold is 18 to 25 us on average and on the CPU; the step's `move_to` is the hold
  behind which 83 % of the wait begins (37 us a hold at 300, 68 us at 700). The monster updater's long holds (1 to 5 ms) cause
  about 1 % of it.
- **The waiters are the per-connection handlers**, mostly tiny exclusive reads: the step's `riding` (47 %) and `left_areas`
  (36 %) and the attack's checks. The monster updater almost never waits; the heartbeat does not take `maps`.
- **The wait is sleep and scheduling, not work**: a contended acquisition puts a tokio worker to sleep in `futex`, and it costs
  about 0.45 ms to get it running again, twelve times the hold. With the bots at `nice 19` a wait is 22 % shorter; spinning 50 us
  first ends two thirds of the waits but the total only falls 11 %, because the waits over 1 ms (a woken worker without a CPU)
  carry it.
- **The deferred flush and `DeferScope` add no hold time** (after the unlock; the deferral off made holds 20 % longer and left the
  wait unchanged). `maps_held_ms` does include the unlock's `futex` wake (about 28 % of it), a measuring quirk.
- **Why it grew**: like for like the new build waits 27 % more than the old (890 against 699 ms at 300, the same evening), the
  rest of the 2026-11-14 figure was that run's conditions. The 27 % arrived with piece 1 (write batching): workers that no longer
  spend 6.9 s per 10 s in per-packet `write`s are idle more and handle bursts of steps in parallel, so they meet at `maps` 41 % more
  often; each meeting costs the same. The latency fell because batching saved far more worker time (about 2.7 s per 10 s) than the
  extra wait costs (about 0.2 s).
- **Fix first**: take `maps` once per step instead of three times (`riding` and `left_areas` into `move_to`'s hold), expected
  about -35 to -50 % of the wait for a small change; fix `HoldTimer`'s place in `MapsGuard`. Per-map locks give nothing for a
  one-map battle; shorter holds and spinning give little (section 5).

## 2. The instrumentation

A probe build of `5f74544` in the worktree `target/maps-probe/wt` (uncommitted, never on `main`): `crates/game-server/src/maps_probe.rs`
plus small edits to `zone.rs` (`Zones::maps` is `#[track_caller]` and does `Relock::guard`'s steps by hand; `MapsGuard` gets a `Drop`
that keeps the field order lock, timer, scope, before 023d594), `outbox.rs` (`deferred_len`) and `server/tasks.rs` (the probe's lines after each stats
line). Per call site of `Zones::maps` (the caller's file and line) and per 10 s it logs:

- `acq`, `hold_ms`, `hold_max_us`, a hold histogram: the hold from just after the lock is taken to just before the unlock;
- `contended`, `wait_ms`, `wait_max_us`, a wait histogram: the acquisitions whose `try_lock` failed, timed exactly as
  `Relock::guard` times them (`monitor::lock_waited` is still called, so the stats line's `lock_worst` stays comparable);
- `handoff_ms`: the part of a wait after the last release (the lock already free, the waiter not yet running);
- `caused_n`, `caused_ms`: waits attributed to the site that held the lock when the wait began;
- `flush_ms`, `flushed`: the deferred delivery after the unlock (`DeferScope`'s drop) and its packet count;
- for one hold in 16 per thread, the thread's CPU time (`CLOCK_THREAD_CPUTIME_ID`) against the wall time of the hold (`cpu_*`,
  `preempted` = more than 20 us off the CPU).
- `MAPS_PROBE_NODEFER=1` opens no deferral scope, so the sends under `maps` are made at once as before piece 9 (the A/B switch).

Two caveats found in the data, both small against the effect:

- **The CPU sample perturbs the holds it samples.** Reading the thread CPU clock is a real system call that updates the scheduler's
  account of the running task, and on this kernel (6.12, EEVDF) that is where an expired slice gets its reschedule, so a sampled hold
  is preempted far more often than an unsampled one: at 700 the sampled holds average 81 us of wall time and 20 us of CPU, while the
  15 in 16 unsampled holds average 21 us of wall time. So the unsampled holds are on the CPU almost all their time, the sampled
  `off-CPU` share overstates preemption, and the probe adds about 15 % to the total hold (the sampled ones' extra wall time),
  which shows as somewhat more wait in the probe runs than in a plain build.
- **`maps_held_ms` on the stats line includes the unlock.** `HoldTimer` is dropped after the `MutexGuard` (the field order that keeps
  the delivery after the unlock), so it also times the unlock itself; for a contended lock that is a `futex` wake system call. The
  probe's hold stops before the unlock: at 300 it sums 448 ms per 10 s where the stats line in the same run says 616 ms, at 700
  1292 ms against 1859 ms. About a quarter of `maps_held` is the release, not the critical section.

Helpers (scratch, not in git): `sched.sh` / `sched2.sh` sample every 10 s, per thread of the game server, `/proc/<pid>/task/<tid>/schedstat`
(CPU time, **run-queue wait**: runnable but not running, time slices) and the voluntary and involuntary context switches, plus
(`sched2.sh`) the bots' threads summed, every CPU's frequency and idle-state counters, and `/proc/stat`.

## 3. The runs

All on the dev box (i5-3470, 4 cores without SMT, 7.6 GB, `schedutil` governor 1.6 to 3.2 GHz, `intel_idle` C1/C1E/C3/C6, kernel
6.12 `PREEMPT_DYNAMIC` voluntary, HZ 250), bots and server together, map 18 at (636, 394), ramp 5 a second, `--duration 10m`, the
LOAD TEST block of `load-pvp.md`, `RUST_LOG=info`. The older 500-bot sampler (`top` every 5 s, pid 2833464) ran during all of them
as it did during the 2026-11-14 runs. Reports in `logs/load-pvp/` (not in git).

### 3.1 The probe runs (2026-11-15, by the stopped agent; data checked and reused)

| | M-300 (deferral on) | M-300-nodefer | M-700 (deferral on) |
|---|---|---|---|
| time (UTC) | 17:55 to 18:06 | 18:09 to 18:20 | 18:21 to 18:32 (stopped before the swarm's report; server side only) |
| steady stats lines | 55 | 55 | 46 |
| swarm: ack p95, step echo p95 | 11.1 ms, 20.3 ms | 11.6 ms, 17.1 ms | (no report) |
| acquisitions per 10 s | 21,221 | 21,079 | 51,800 |
| contended | 2,384 (11.2 %) | 2,420 (11.5 %) | 4,992 (9.6 %) |
| hold per 10 s (probe) / `maps_held_ms` (stats) | 448 / 616 ms | 539 / 730 ms | 1,292 / 1,859 ms |
| mean hold | 21 us | 26 us | 25 us |
| wait per 10 s (probe) / stats line | 1,277 / 1,269 ms | 1,180 / 1,201 ms | 3,073 / 3,002 ms |
| mean wait per contended acquisition | 536 us | 488 us | 616 us |
| hand-off part of the wait | 139 ms (11 %) | 156 ms (13 %) | 342 ms (11 %) |
| deferred flush (after the unlock) | 215 ms, 665 k packets | none | 883 ms, 3.07 M packets |
| waits over 1 ms per 10 s (count) | 333 | 341 | 915 |
| holds over 1 ms per 10 s (count) | 62 | 63 | 176 |
| workers' run-queue wait per 10 s (`schedstat`) | 2.71 s (312 us per slice) | 2.75 s (322 us) | 9.67 s (390 us) |
| workers' CPU per 10 s | 4.17 s | 4.02 s | 14.38 s |
| involuntary switches per 10 s | 3,267 | 3,091 | 14,325 |

M-300 ran on a probe build from before the `MAPS_PROBE_NODEFER` switch was added (the same fields); M-300-nodefer and M-700 on the
final one.

Per site at 700 (per 10 s; the biggest; `caused` = wait that began while this site held the lock):

| site | caller | acq | hold ms (mean) | contended | wait ms (mean) | caused ms | flush ms |
|---|---|---|---|---|---|---|---|
| `zone.rs` `move_to` | step | 10,862 | 742 (68 us) | 219 (2.0 %) | 92 (420 us) | 2,151 | 657 |
| `zone/vehicle.rs:172` `riding` | step (`speed_checked`), attack | 14,894 | 61 (4 us) | 2,349 (15.8 %) | 1,556 (662 us) | 124 | 3 |
| `zone/duel.rs:363` `left_areas` | step | 10,862 | 48 (4 us) | 1,581 (14.6 %) | 882 (558 us) | 79 | 2 |
| `zone/pvp.rs:260` `attack_player` | attack | 1,689 | 138 (82 us) | 46 | 24 | 297 | 129 |
| `zone.rs` `shape` | attack, skill | 5,079 | 25 (5 us) | 370 | 197 (533 us) | 58 | 1 |
| `zone.rs` `end_shape_by_attack` | attack | 1,853 | 14 (7 us) | 194 | 194 (997 us) | 23 | 1 |
| `zone.rs` `use_skill` | skill | 137 | 55 (401 us) | 3 | 1 | 99 | 59 |
| `zone.rs` `revive_here` | rebirth | 70 | 33 (469 us) | 11 | 8 | 66 | 17 |
| `zone.rs` `idle` | monster updater | 10 | 53 (5.3 ms) | 0 | 0 | 30 | 0 |
| `zone/npc.rs` `npc_pass` | monster updater | 23 | 29 (1.3 ms) | 0 | 0 | 29 | 0 |
| `zone/move_tick.rs` `move_steps` | monster updater | 80 | 26 (322 us) | 4 | 3 | 10 | 1 |
| `zone/pvp.rs:232` `attack_verdict` | attack | 3,973 | 13 (3 us) | 92 | 32 | 24 | 0 |

At 300 the order is the same (`move_to` 190 ms of hold and 786 ms caused; `riding` 607 ms and `left_areas` 382 ms of the 1,277 ms of
wait).

### 3.2 The old build on today's box (X-old-300)

The build of the 2026-11-12 runs (`85fa13e`, worktree `target/maps-probe/wt-old`), 300 bots, 18:45 to 18:56 UTC, the same bots
binary as the other runs (the bots changed only in their logout since). It answers whether the higher wait is the box or the build.

| | R-300 (2026-11-12, old build) | **X-old-300 (today, old build)** | R2-300 (2026-11-14, new build) | M-300 (today, new build + probe) |
|---|---|---|---|---|
| `maps` wait per 10 s, median | 618 ms | **699 ms** | 1,186 ms | 1,269 ms |
| waits per 10 s | 1,431 | **1,558** | 2,181 | 2,359 |
| mean wait | 444 us | **449 us** | 550 us | 538 us |
| longest wait (median of the lines) | 9.4 ms | 10.6 ms | 15.8 ms | |
| out packets a second | 59 k | 69 k | 65 k | 70 k |
| ack p95, step echo p95 | 16, 31 ms | 16.4, 34.2 ms | 11.6, 20.3 ms | 11.1, 20.3 ms |
| workers' CPU per 10 s | | 6.88 s | | 4.17 s |
| workers' run-queue wait per 10 s | | 3.43 s (307 us per slice) | | 2.71 s (312 us) |
| workers' voluntary / involuntary switches per 10 s | | 4,116 / 6,696 | | 5,320 / 3,267 |

**The box did not change: the old build on today's box waits as it did on 2026-11-12** (699 ms against 618 ms, the same 450 us a
wait). Part of the higher wait comes with the new build (below, 3.2a). The old build's workers burn 65 % more CPU, are
preempted twice as often and sleep less often; the new build's workers sleep (block or park) more and are preempted less.

The new build rerun today, plain (X-main-300, the `main` release binary, 19:33 to 19:44 UTC): **890 ms in 2,104 waits (423 us
each)**, longest 9.4 ms, `maps_held_ms` 543 ms; ack p95 10.6 ms, step echo p95 16.4 ms; workers 4.34 s of CPU, 2.75 s of run-queue
wait (316 us per slice), 5,158 voluntary and 3,322 involuntary switches per 10 s. So, like for like on one evening: **the new build
waits 27 % more than the old (890 against 699 ms), all of it from 35 % more contended acquisitions (2,104 against 1,558); a wait
costs the same (423 against 449 us).** The rest of the gap in `load-pvp.md` (1,186 ms on 2026-11-14 for the same server code) was
that run's conditions and the spread between runs (the later builds range 890 to 1,114 ms this evening, 3.2a), not the code. (The probe without its CPU sample, 872 ms, matches the plain
build, so P-nocpu-300 is a faithful picture.)

### 3.2a Which piece (the bisect)

Plain builds of the commits after each step of the load-fixes work (worktrees `target/maps-probe/wt-<commit>`), 300 bots, one
after another on the same evening.

| build | X-old-300 `85fa13e` | X-p1-300 `f0896b4` (piece 1, write batching) | X-p7-300 `a729cdf` (+ pieces 2 to 7) | X-p9-300 `14221f1` (+ pieces 8, 9) | X-main-300 `5f74544` (+ piece 10) |
|---|---|---|---|---|---|
| `maps` wait per 10 s | 699 ms | **1,008 ms** | 1,046 ms | 1,114 ms | 890 ms |
| waits per 10 s | 1,558 | **2,203** | 2,258 | 2,092 | 2,104 |
| mean wait | 449 us | 458 us | 463 us | 533 us | 423 us |
| workers' CPU per 10 s | 6.88 s | 4.18 s | 4.19 s | 4.35 s | 4.34 s |
| voluntary / involuntary switches | 4,116 / 6,696 | 5,383 / 3,185 | 5,282 / 3,173 | 5,486 / 3,449 | 5,158 / 3,322 |
| ack p95, step echo p95 | 16.4, 34.2 ms | 11.1, 17.1 ms | 10.2, 16.4 ms | 15.0, 18.7 ms | 10.6, 16.4 ms |

### 3.3 The new build with the bots at the lowest priority (X-main-300-nice)

`5f74544` (the `main` release binary, no probe), the bots under `nice -n 19`, 18:56 to 19:08 UTC. Everything else as X-main-300.

| | R2-300 (2026-11-14, nice 0) | X-main-300 (today, nice 0) | **X-main-300-nice** |
|---|---|---|---|
| `maps` wait per 10 s | 1,186 ms | 890 ms | **667 ms** |
| waits per 10 s | 2,181 | 2,104 | 2,030 |
| mean wait | 550 us | 423 us | **328 us** |
| longest wait | 15.8 ms | 9.4 ms | 7.7 ms |
| `maps_held_ms` (stats line) | 576 ms | 543 ms | 484 ms |
| ack p95, step echo p95 | 11.6, 20.3 ms | 10.6, 16.4 ms | 9.3, 13.2 ms |
| workers' run-queue wait per 10 s | | 2.75 s, 316 us per slice | 2.30 s, 211 us per slice |

With the bots out of the server's way, the number of waits hardly moves (2,104 to 2,030) but **each wait is 22 % shorter (40 %
against R2-300)**, the run-queue wait per slice a third shorter, and the total falls to what the old build had (667 against 699 ms). A contended acquisition puts a tokio worker to sleep in the kernel (`std::sync::Mutex`
spins only briefly), and what the wait then costs is mostly how long the woken worker sits runnable before it gets a CPU, which
is longer when the bots' threads hold the CPUs. That part is the box (bots and server on 4 cores), not the server.

Against today's plain runs: the new build 890 ms (423 us a wait) at nice 0 and 667 ms (328 us) at nice 19; the old build (X-old-300-nice,
20:19 to 20:31 UTC) 699 ms (449 us) and 678 ms in 1,716 waits (395 us). The old build's workers, busy 7.7 s per 10 s with their
per-packet writes, are less exposed to the bots; the new build's idler workers gain more from having the CPUs to themselves.

### 3.4 The probe without the CPU sample (P-nocpu-300)

The probe build with `MAPS_PROBE_NOCPU=1` (no thread-clock reads, so the holds are not perturbed), 300 bots, 19:10 to 19:21 UTC.
Stats line: wait 865 ms in 2,050 waits (422 us each), `maps_held_ms` 523 ms; ack p95 10.6 ms, step echo p95 15.7 ms. The probe:
20,441 acquisitions, 2,090 contended (10.2 %), **hold 375 ms** (18 us a hold), wait 872 ms, hand-off 136 ms, flush 217 ms (640 k
packets) per 10 s. Workers: 4.42 s of CPU and 2.75 s of run-queue wait (315 us per slice) per 10 s.

| site (per 10 s) | caller | acq | hold ms (mean) | contended | wait ms (mean) | waits begun behind it (their ms, mean) |
|---|---|---|---|---|---|---|
| `move_to` | step | 4,690 | 174 (37 us) | 142 | 34 (238 us) | **1,633 (722 ms, 442 us)** |
| `riding` | step, attack | 5,913 | 4.2 (0.7 us) | **970** | **408 (421 us)** | 72 (2 ms) |
| `left_areas` | step | 4,690 | 3.1 (0.7 us) | **701** | **312 (444 us)** | 34 (1 ms) |
| `attack_player` | attack | 746 | 32 (43 us) | 25 | 11 | 144 (75 ms) |
| `end_shape_by_attack` | attack | 830 | 0.9 | 72 | 42 (582 us) | 0 |
| `shape` | attack, skill | 993 | 2.5 | 63 | 24 | 4 |
| `use_skill` | skill | 68 | 11 (161 us) | 2 | 1 | 23 ms |
| `revive_here` | rebirth | 30 | 8 (270 us) | 5 | 2 | 20 ms |
| `idle` | monster updater | 10 | 53 (5.3 ms) | 0 | 0 | 0 (2 ms) |
| `move_steps` | monster updater | 80 | 26 (318 us) | 3 | 0.4 | 1 ms |
| `npc_pass` | monster updater | 23 | 24 (1.0 ms) | 0 | 0 | 1 (3 ms) |
| `recover` | monster updater | 10 | 15 (1.4 ms) | 0 | 0 | 0 |

So, unperturbed: **the step's `move_to` is the hold almost every wait begins behind (83 % of the wait), and its waiters are the
other connections' tiny acquisitions** (`riding`, `left_areas`, the attack's checks): a 37 us hold, and the wait it starts lasts
442 us, twelve times as long. The monster updater holds `maps` for 1 to 5 ms at a time but almost nobody is waiting then (its
tick seems to run in the workers' lulls; tokio fires timers when a worker turns to its driver); with the CPU sample on (M-300) the
updater's holds were stretched by the sampling preemption and showed 240 ms of caused wait, which was the artifact. The CPU
sample was a large part of the M-300 wait: 1,277 ms with it, 872 ms without.

### 3.5 Spinning before sleeping (P-spin-300)

The same probe build with `MAPS_PROBE_SPIN_US=50` as well: a waiter retries `try_lock` for up to 50 us before it calls
`Mutex::lock` (and sleeps). 19:21 to 19:33 UTC.

| per 10 s | P-nocpu-300 (sleep at once) | P-spin-300 (spin 50 us first) |
|---|---|---|
| contended acquisitions | 2,090 | 2,577 |
| of them ended within the spin | | about 1,700 (66 %) |
| waits under 50 us / 50 us to 1 ms / over 1 ms | 846 / 998 / 247 | 2,039 / 379 / 158 |
| total wait (probe; stats line) | 872 ms; 865 ms | 778 ms; 820 ms |
| hand-off part | 136 ms | 56 ms |
| hold (probe; `maps_held_ms`) | 375 ms; 523 ms | 409 ms; 469 ms |
| ack p95, step echo p95 | 10.6, 15.7 ms | 14.4, 19.5 ms |
| workers' CPU; voluntary switches | 4.42 s; 5,296 | 4.42 s; 3,946 |

Two thirds of the waits are behind a hold that ends within 50 us, and a spin catches them (the voluntary switches fall by a
quarter). But **the total wait falls only 11 %**: it is carried by the waits over 1 ms (247 and 158 of them, 1 to 15 ms each),
and those are not a short hold but a worker that slept and then did not get a CPU for milliseconds. The latencies were not better
(within the run-to-run spread of these 300 runs; the CPU spent spinning is on a box with no spare core). A spinning lock
(`parking_lot` spins for a few microseconds, then yields, then parks) would therefore help little here.

## 4. Findings

### Q1. Which call sites hold `maps` longest, and how much wait each causes

By total hold (P-nocpu-300, per 10 s; the 700 picture in 3.1 is the same order): the step's `move_to` (174 ms, 37 us a hold; at
700 742 ms, 68 us: a scan of every player on the map for the distance, the `0x0501` copy and deferred send to each one in range,
and `swap_view` when the cell changes), the monster updater's `idle` (53 ms, 5.3 ms a hold, once a second), `attack_player`
(32 ms, 43 us), `move_steps` (26 ms, 0.3 ms), `npc_pass` (24 ms, 1.0 ms), `recover` (15 ms, 1.4 ms), `use_skill` (11 ms, 0.16 ms)
and `revive_here` (8 ms, 0.27 ms). The longest single holds (1 to 10 ms) are the updater's passes.

By the wait they cause, it is not that order: `move_to` begins 83 % of the wait (1,633 waits, 722 ms of 872), `attack_player`
9 % (75 ms), `use_skill` and `revive_here` 2 to 3 % each; the updater's long holds cause about 1 % (a few ms), and the step's own
tiny acquisitions (`riding`, `left_areas`) almost none. A wait that begins behind a 37 us `move_to` lasts 442 us on average.

### Q2. Who the waiters are

The per-connection handlers, nearly all of them: `riding` (the step's speed check, and the attack and skill checks; 47 % of the
wait), `left_areas` (the step's last acquisition; 36 %), the attack's `end_shape_by_attack`, `shape`, `set_attacking` and
`attack_verdict` (about 10 % together), `move_to` itself (4 %). The monster updater (the "world tick": `mob_updater` in
`server/tasks.rs`, one task) waits almost never (under 1 ms per 10 s in all). The heartbeat does not take `maps` at all
(`server/tasks.rs` `heartbeat` reads `ctx.online`); no heartbeat site appears in the probe.

The waiters' own holds are under 1 us. They wait because each is an exclusive lock taken for a read of one field, at a moment when
another connection's step is in `move_to`: `riding` and `left_areas` are contended 15 to 16 % of the time where the lock is held
under 4 % of the time, so the steps of several connections are being handled at the same moment far more often than chance.

### Q3. Scheduling delay or longer holds; the flush and `DeferScope`

**Scheduling delay, by every measure:**

- The holds did not get longer: 18 to 25 us on average, almost all on the CPU (the unsampled holds' wall time equals the sampled
  holds' CPU time); `maps_held_ms` was 576 ms per 10 s at 300 on 2026-11-14 and 523 to 543 ms today, against a wait of 865 to 1,186.
- A wait costs ten to twelve times the hold it waits behind (442 us behind a 37 us `move_to`). `std::sync::Mutex` spins only
  briefly and then sleeps in `futex`, which blocks the whole tokio worker; the wait then lasts until the holder's unlock wakes it
  **and the kernel runs it**. The workers' run-queue wait (runnable, not running) is 300 to 320 us per time slice at 300 and
  390 us at 700, the same size as the mean `maps` wait.
- Putting the bots at `nice 19` cut each wait by 22 % (423 to 328 us, X-main-300 against X-main-300-nice; 40 % against R2-300)
  with the number of waits unchanged; spinning 50 us before sleeping ended two thirds of the waits within the spin but cut the total only 11 %, because the
  total is carried by the waits of 1 ms and more, a woken worker that does not get a CPU (bots, the other workers, `top`, PostgreSQL
  on 4 cores).
- The hand-off part (the lock already free, the waiter not running yet, after the last release) is 11 to 16 % of the wait, and it
  is an underestimate: the mutex is not fair, so a running thread takes the free lock ahead of the woken one, and the woken one goes
  back to sleep.

**The deferred flush adds no hold time.** It runs after the unlock (`MapsGuard`'s fields: lock, then timer, then scope, before 023d594; since then timer, lock, scope); the probe
times it separately: 217 ms per 10 s for 640 k packets at 300 (0.34 us a packet), 883 ms for 3.07 M at 700, all outside the lock.
With the deferral off (M-300-nodefer) the holds grew 20 % (448 to 539 ms), and the wait did not move (1,277 to 1,180 ms, within the
spread). `DeferScope`'s drop is the flush, so it adds nothing either. What does add to the **reported** hold is `HoldTimer` being
dropped after the `MutexGuard`: `maps_held_ms` includes the unlock's `futex` wake, about 28 % of it (523 against 375 ms).

### Q4. Why the wait grew while the latency fell

1. **That run's conditions, not the code, for most of the 2026-11-14 figure**: the same server code waits 890 ms today against
   1,186 ms then,
   and the old build waits 699 ms today against 618 ms on 2026-11-12. Like for like the new build waits 27 % more, not 92 %.
2. **That 27 % comes with piece 1, the write batching** (X-p1-300: 1,008 ms in 2,203 waits, against 699 ms in 1,558 for the old
   build): the number of contended acquisitions rose 41 %, and the cost of one did not change (449 to 458 us). Pieces 2 to 10 did not
   add to it: every later build has 2,092 to 2,258 waits per 10 s and 890 to 1,114 ms of wait (X-p7-300 1,046, X-p9-300 1,114,
   X-main-300 890), a spread of about +-12 % between single runs, where the old build is at 1,558 waits and 699 ms.
3. **The likely mechanism** (it fits the switch counts and the per-site contention; it was not traced task by task): one write per
   packet kept the workers busy in `write` system calls (6.88 s of CPU per 10 s at 300); a
   connection's packets queued behind that, and the steps of different connections were handled more one after another. With
   batching the workers need 4.2 s, so they are idle more (voluntary switches 4,116 to 5,383 per 10 s, involuntary 6,696 to 3,185),
   and when a burst of packets arrives, idle workers are woken to take the tasks in parallel. More workers handling steps at the
   same instant meet more often at `maps`. Every such meeting costs the waiter a sleep and a wake-up (about 0.45 ms on this box),
   whatever the hold.
4. **Why the latency still fell**: the wait is blocked worker time, not time on the critical path of every packet. At 300 the new
   build's extra wait is about 190 ms per 10 s across 4 workers (under 0.5 % of their time), while batching saved about 2.7 s of
   worker CPU per 10 s and took the per-packet write out of every connection's path, and piece 9 took the sends out of the lock (so a
   waiter is behind a 37 us hold rather than a longer one). At 1000 (E2-1000, 5.8 s of wait per 10 s, 15 % of the 4 workers) it is
   no longer small, which is why it is worth fixing before a larger run.

## 5. Fix options, ranked

What the runs say a fix must do: the wait is (the number of contended acquisitions) times (about 0.45 ms of sleep and wake-up on
this box). The second factor is the box's scheduling and is hardly moved by the server (spinning: -11 %); the first is moved by
**how many times a handler takes `maps`**, not by how long `move_to` holds it (holds +20 % with the deferral off: no change in the
wait). Gains below are for the 300 battle unless said; at 700 and 1000 the same shares hold (3.1).

| rank | option | what | expected gain | cost | risk |
|---|---|---|---|---|---|
| 1 | **One acquisition per step** | the step's `riding` (in `speed_checked`, `server.rs`) and `left_areas` (`zone/duel.rs:363`) done inside `move_to`'s hold (or the rider state read from the connection's own `WorldState`), as piece 10 did for the trade, duel and war checks; the same for `move_back` | the step's `riding` (4,690 of its 5,913 acquisitions) and `left_areas` carry about 73 % of the wait; one acquisition a step instead of three leaves it contended at about the rate `riding` is now (16 %) instead of 0.34 contended acquisitions a step: **about -35 to -50 % of the wait** (at 300 from about 870 to 450 to 550 ms per 10 s), more if fewer parallel meetings also spare the attack's sites | small: two call sites and two zone functions; the tests of the speed check, the duel and the war areas | low: every send under the lock is still deferred in order; the speed check must see the same mount state it sees now |
| 2 | **Fix the hold figure** | `MapsGuard` declares `_held` before `lock` (fields drop in declaration order: the timer, then the unlock, then the delivery, which stays after the unlock) | `maps_held_ms` stops including the unlock's `futex` wake (about 28 % of it today); no runtime gain | trivial | none (the delivery order invariant is unchanged) |
| 3 | **Lock-free reads on the attack and skill path** | the small exclusive acquisitions of an attack (`attack_verdict`, `set_attacking`, `shape`, `end_shape_by_attack`, `riding`) folded into `attack_player`'s hold or read from per-player state the owner writes | about 10 to 15 % more of the wait (those sites' share) | medium: an attack takes `maps` five to six times today | medium: a read outside the lock can be stale (a mount or shape changed by another task); each field needs its own argument |
| 4 | **Shorter `move_to`** | one shared packet buffer per fan-out instead of a `to_vec` per recipient (design C of `load-fixes.md`), the recipients found by cell instead of a distance scan over the map (design D) | little on the wait (see the nodefer A/B); `move_to` is 46 % of all hold time at 300 and 57 % at 700, so CPU and the chance of a long hold fall | medium to high: the outbox type changes everywhere (C); a spatial index (D) | medium |
| 5 | **Do not block a worker on `maps`** | the world's state behind one task (an actor fed by channels) or per-map tasks, so a busy lock queues work instead of putting a thread to sleep | the whole wait turns into queueing that the other tasks can run past; largest gain at 1000 | very high: 193 sites, every zone call becomes a message | high |
| 6 | **Per-map locks** (design A) | `HashMap<u16, Mutex<...>>` | **nothing for this battle** (one map); divides the contention in a spread-out world | high: 193 sites, cross-map walks take every lock, a map change takes two | medium |
| 7 | Spin before sleeping (`parking_lot` or by hand) | measured: `MAPS_PROBE_SPIN_US=50` | -11 % of the wait, more CPU, latency not better | small | low, but not worth it on a 4-core box shared with the bots |

**Fixes 1 and 2 are built, not yet measured** (2026-11-15). Fix 1: `Zones::step` takes `maps` once for a whole step (the vehicle status for the
speed check, the move or the move back, the trade check, the duel and war areas; `server.rs` runs the speed check as a closure under
the lock); a zone test counts the takes. Fix 2: `MapsGuard` drops the timer, then the lock, then the deferred scope, so `maps_held_ms`
no longer includes the unlock (a test reads the timer's end with the lock still taken). The predicted drop in the wait is still to be
checked by a 300 and a 700 run with the plain build; section 4's holds from before the change include the unlock.

Not a fix, but for reading the figure: **on this box a quarter of the wait is the bots' doing** (nice 19: -25 %, 3.3), so a knee run with
the bots on another machine would show a smaller `maps` wait for the same server. And the probe's CPU sample (`CLOCK_THREAD_CPUTIME_ID`)
must stay off for figures to compare (it raised the wait about 45 % in M-300).

Recommended order: 1 and 2 together (small, low risk, the measured bulk of the wait), then a 300 and a 700 run with the plain build
to check the predicted drop; 3 if the attack sites are then the larger share; 4 for CPU at 1000; 5 and 6 only for a bigger or
spread-out world.

## 6. Files

Not in git (`logs/` and `target/` are ignored):

- `logs/load-pvp/`: the stopped agent's `M-300*`, `M-300-nodefer*`, `M-700-sched.log`, `M-700-swarm.txt` (header only: the 700 swarm
  was stopped before its report) and `M-server.log` (all three probe runs' server log, with the `maps_probe` lines); this
  evening's `X-old-300*` (server log `X-server.log`), `X-main-300*` (the first attempt failed at entry, `X-main-300-failed-swarm.txt`:
  a cleanup of the run before removed the bots while they were being seeded; the rerun is the one used), `X-main-300-nice*`,
  `X-p1-300*`, `X-p7-300*`, `X-p9-300*`, `X-old-300-nice*`, `P-nocpu-300*`, `P-spin-300*`: for each the swarm report (`.json`,
  `-swarm.txt`), the server log (`-server.log`) and the 10 s samples (`-sched.log`).
- `target/maps-probe/wt`: the probe worktree (`5f74544` plus the uncommitted probe, with the `MAPS_PROBE_NOCPU` and
  `MAPS_PROBE_SPIN_US` switches added this evening); `target/maps-probe/wt-old`, `wt-f0896b4`, `wt-a729cdf`, `wt-14221f1`: the
  bisect worktrees; `target/maps-probe/bin/`: the binaries run.
- `target/maps-probe/tools/`: `probe.diff` and `maps_probe.rs` (the instrumentation), the samplers (`sched.sh`, `sched2.sh`), the
  run scripts (`run2.sh`, `srv.sh`, `seq.sh`, `seq2.sh`, the plans), and the summaries (`sum.py` for a run's stats lines and samples,
  `probe.py` and `detail.py` for the probe's per-site lines; `wtsites.txt` maps the first probe build's lines to functions,
  `wtsites2.txt` the final one's, where `zone.rs` lines after 1787 are 22 further on).

After the runs `config/game.toml` was put back to `logs/game.toml-before-load-test` (no LOAD TEST block), the bots were unseeded,
and the login and game servers were restarted from `target/release` with clean environments (`RUST_LOG` the scripts' defaults, no
`LOGIN_` variable, `stats line off`, `counting=true`), listening on 30800 and 30810.
