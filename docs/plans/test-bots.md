# Test bots and the PvP swarm (spec and build plan)

**Specified 2026-11-12; not built.** Unshelves "Test bots and a load test" (`docs/plans/roadmap.md`, "Frozen or later"). The first use is
a **battle of 200 to 300 players**, to find out whether the server and the 2010 client cope with high-scale PvP. The later uses (a partner
bot for the two-client checklist items, bot-only scenarios, a soak test) build on the same crate and are sketched in section 9.

## 1. The maintainer's decisions (2026-11-12)

1. **The bots come back, and the PvP swarm is their first job**: 200 to 300 players fighting in one place.
2. **Bots measure the server; the client is judged by eye.** Bots do not run `game.exe`, so they cannot say whether the client keeps up.
   The maintainer stands in the battle with the real client while the bots fight around it; to that client the bots are ordinary
   players, so it receives the full traffic of the fight.
3. **The drill runs the test, never `newshaiya`.** The drill database (`shaiya-afterglow-ext1`, port 5434) gets the bot accounts. The
   client always connects to the game server on port **30810** (`docs/client/re-network.md`, `0x785A`), so for a test window the live
   dev box's servers are stopped, the drill's login and game servers start on 30800 and 30810, and afterwards the drill's are stopped and
   the live ones started again. Only one game server runs at a time.

Settled with the maintainer the same day:

4. **Where the bots run.** This box has 4 cores and 7 GB of memory, and the server and PostgreSQL need it too. Every bot also
   decrypts the whole battle's traffic (section 4), which is about as much work as the server's sending. Up to about 100 bots run
   here; the 300 run with the bots on another Linux machine on the LAN (or WSL2 on the Windows machine that runs the client, if that is
   the only other machine).
5. **Where the battle is**: any spot where the factions may fight (every map but 40 and 42, `docs/original/pvp.md`). Proelium
   Frontier (map 18) is the default, at an open spot both factions reach on foot from their rebirth points.
6. **Classes and gear** matter only so far as the bots can move, fight and use skills: level 60, all six classes of each faction in
   equal numbers, one ordinary set per class (no enchants, no lapis), so that nobody dies in one hit or never dies.
7. **The maintainer's own account on the drill** is their `player1` account with the character TestVail, copied from the live dev
   box (`newshaiya`) and made a GM (status 16), so they can join the battle or watch it freely.

## 2. What exists to build on

- `crates/game-server/examples/game_probe.rs` (197 lines): the login handshake (RSA, then AES-CTR), the server list, server select with
  the client version `0x10042600`, the game handshake (`0xA301`) and the character screen. It stops there.
- `crates/game-server/tests/common/mod.rs`: `Client` (the cipher, including the switch to XOR after `0x010B`), `enter_world`,
  `select_and_enter`, `read_zone_entry`, `step`. These enter the world against an in-process server.
- `shaiya-proto` encoders and opcodes for everything a fighter does: `0x0501` (a step; another player's step), `0x0502` (attack a
  player; its result), `0x0511` (use a skill on a player), `0x0504` (a death), `0x0553` (back to life; kind 2 at the bind position),
  `0x0551` (came back), `0x0521` and `0x0505` (own HP, MP and SP), `0x0201` / `0x0202` (a player enters or leaves the view), `0x051C`
  (speeds).
- `login_server::accounts::PgAccountStore::create_account` (what `create_account` uses): accounts with the server's own password hashing.
- The server's limits (`docs/server/settings.md`), which a swarm from one address hits at once (section 5).

Nothing in the server measures what this test needs: there is no timing of the monster pass, no lock-wait figure, no packets-a-second
count and no memory line (section 4.2).

## 3. The crate

A new workspace crate, `crates/bot`, binary `bot`, depending on `shaiya-proto`, `shaiya-crypto` and `login-server` (for the accounts).
The connection code of `game_probe.rs` and `tests/common` moves into `bot`'s library (`bot::link`, `bot::login`, `bot::enter`), and
`game_probe` becomes `bot probe` with the same arguments. `tests/common` keeps its own copy (it talks to an in-process server through the
memory backend, not through a login server).

```
bot seed     --count 300 --prefix bot --map 18 --at x,z [--templates <ids>]   accounts and characters on the drill (section 6)
bot unseed   --prefix bot                                                      removes them
bot swarm    --login host:30800 --prefix bot --count 300 --ramp 5 \
             --scenario pvp-battle --at 18,x,z --radius 60 --duration 30m \
             --report report.json                                              the battle (sections 4, 7)
bot probe    <login host:port> <user> <password> [...]                         game_probe as it was
```

All commands refuse to run against a database whose name or `.env` is the live box's (`newshaiya`), unless `--i-mean-it` is given.

## 4. A bot

### 4.1 Its life

```
connect login → key exchange → login → server list → select (0x10042600) → connect game → handshake → character screen
→ select slot 0 → enter-world burst → 0x0201 → zone entry → fight loop → (death → wait → 0x0553 kind 2 → walk back) → …
```

A bot is one tokio task holding its own `Link`, its position and speeds, its HP, and the players in its view (from `0x0201`, `0x0202`
and the moves `0x0501` it receives: id, faction, position, alive). It reads every packet; the ones it does not use are counted by opcode
and dropped. A packet it cannot parse is counted as a protocol error and logged once per opcode.

### 4.2 The `pvp-battle` scenario

- **Target**: the nearest enemy in view that is alive; none, walk toward the battle point (`--at`), then wander inside `--radius`.
- **Walking**: steps toward the target with the bot's run speed from `0x051C`, at a steady step rate a little below the client's, so no
  step fails the move check (`docs/original/move-check.md`: five-second windows with slack). A refusal (the put-back `0x020B` on the
  bot's own id; the `0x0502` sent with it shares its opcode with an attack's result, so the bot goes by the `0x020B`) is counted: a bot
  that is refused is a bot bug and the report shows it.
- **Fighting**: in reach, `0x0502` on the target at the attack speed; every `--skill-every` seconds (default 6) one of the class's
  attack skills the character has learnt (`0x0511`), when its MP or SP allows.
- **Dying**: on its own `0x0504`, wait `--rebirth-after` seconds (default 5, a person looking at the screen), send `0x0553` with kind 2,
  and on `0x0551` walk back to the battle. Dying to a player above level 10 can drop an item (`docs/original/pvp.md`): the bots lose
  gear as the battle goes on and the ground fills with it, as it would in a real battle. `bot seed` re-equips them before the next run.
- **Pacing knobs**: `--steps-per-sec`, `--skill-every`, `--rebirth-after`, and `--idle-share` (the share of bots that only stand and
  watch, which lowers the load without lowering the crowd the client must draw).

### 4.3 What a bot measures

Per bot, summed per 10 seconds for the swarm: packets and bytes in and out by opcode; connection errors; the time from connect to the
zone entry; and **the action round trip**: from an `0x0502` sent to its result coming back, which is the latency a player feels. The
report gives p50, p95 and p99 of the round trip for each 10 seconds, the bots alive, in the world and disconnected, and move refusals.

## 5. The server under test

### 5.1 Settings for the test window (the drill only)

The protection limits (`docs/server/audit.md`) are per address and close a swarm from one machine. In the drill's `config/game.toml`
and the login server's environment, for the test window only:

| setting | default | for the swarm |
|---|---|---|
| `max_per_ip` (game) | 8 | count + 10 |
| `connects_per_minute` (game) | 30 | 60 × ramp per second |
| `LOGIN_MAX_PER_IP` | 20 | count + 10 |
| `LOGIN_CONNECTS_PER_MINUTE` | 30 | 60 × ramp per second |
| `LOGIN_KEY_POOL` | 8 | 32 (the pool does not grow; `docs/plans/episode-adoptions.md` perf 4) |
| the console's `maxusers` | | above count + 1 |

`chat_per_sec` and the move check stay as they are: bots must pass what a player passes. The ramp (default 5 a second) keeps the login
server's key exchanges from queueing. `docs/testing/` gets a page with these values and how to put them back.

### 5.2 New: a stats line

Added to the game server (useful to any operator, not only to this test), behind a setting `stats_every` (seconds; default 0, off).
Every `stats_every` seconds one `info` line, and the same figures in `game-server console stats` output:

- players in the world, connections;
- packets and bytes in and out a second (counted where the connection reads and where the outbox is drained);
- the outboxes: the largest in bytes, the sum, and connections ended by `outbox_limit` since the last line (a client that cannot keep up
  is dropped at 4 MB: `docs/server/settings.md`);
- the monster pass: its longest and average time against its period (500 ms, or 100 ms with smooth chasing);
- **lock waits**: the longest and the total time connections waited for the `Zones` locks (`zone.rs`; all 300 on one map share one
  map's locks, the expected bottleneck), measured with a timer around the acquisitions in the move, attack, skill and view paths;
- the save queue's depth and the game log queue's drops;
- the process's resident memory (`/proc/self/statm`) and its tokio worker count.

This is `docs/plans/episode-adoptions.md` perf item 7 (a queue statistics line), and the base for perf item 2 (a watchdog).

## 6. Seeding

`bot seed` makes the accounts and characters before a window, with the game server **stopped** (it writes character rows directly):

1. **Templates.** Twelve template characters, one per faction and class, level 60, skills learnt, an ordinary set equipped. Assumed
   (decision 6): made once by hand on the drill with the GM commands and listed with `--templates`; `bot seed` refuses a template above level
   60 or with enchanted or linked gear.
2. **Accounts.** `<prefix>0001` … with one password from `SHAIYA_PASSWORD`, created through `PgAccountStore::create_account`.
3. **Characters.** Each account gets a copy of a template (factions alternating, classes in turn): the `characters` row (new id, name
   `Bot0001` …, the map and a spot near `--at`), its items, skills and quick slots, in one transaction per character. The tables a
   character spans are the ones `pg/character.rs` loads; a test checks that a seeded character loads with `load_character` and enters
   the world in the in-process server.
4. **Again.** A second `bot seed` with the same prefix resets each character's position, HP and gear to its template's instead of
   making new ones.

`bot unseed --prefix bot` deletes the accounts and their characters (refused for any account that does not match `^<prefix>\d{4}$`).

## 7. The runs

Each run on the drill, in a test window (decision 3). Ten minutes of battle unless said otherwise.

| run | bots | the real client | question |
|---|---|---|---|
| R0 | 10 | none | the bot works: no move refusals, no protocol errors, deaths and rebirths happen |
| R1 | 50 | none | the stats line and the report agree; a first load figure |
| R2 | 100 | the maintainer, in the battle | does the client draw and keep up at 100 |
| R3 | 200 | the maintainer | the same at 200; the server's lock waits and round trip |
| R4 | 300 | the maintainer | the same at 300; the first run where the bots run on another machine (decision 4) |
| R5 | the largest count that held | the maintainer | 30 minutes: does anything grow (memory, outboxes, ground items) |

The maintainer notes for each run with the client: does it draw everyone; frame rate (by feel, or the client's own counter if it has
one); stutter when the crowd comes into view; do attacks and skills land with a delay; does the client freeze or crash; was it dropped
(the server's log names an `outbox_limit` close).

**What would count as holding**, to be confirmed after R1 gives a baseline: the p95 action round trip under 250 ms, no connection
dropped by the server, the monster pass inside its period, and the real client playable.

The results go to `docs/testing/load-pvp.md`: each run's report summary, the stats lines, the maintainer's notes, and what to fix.
A server bottleneck found here (most likely the one map's locks, or the fan-out of every step to every viewer) is fixed in its own
change, with the run repeated.

## 8. Build

Each part is one session.

1. **The crate and one bot**: `crates/bot`, the link, login and enter code out of `game_probe`, `bot probe`; the fight loop of 4.2 and
   the measures of 4.3 for one bot, tested against the in-process server (`tests/common`) with two bots of the two factions: they find
   each other, one dies and comes back, no step is refused.
2. **Seeding**: `bot seed` and `bot unseed` (section 6), with the load-and-enter test; the `newshaiya` guard.
3. **The swarm and the report**: N bots, the ramp, the 10-second aggregation, `report.json` and a printed summary.
4. **The stats line** (5.2) in the game server: the counters, the lock-wait timers (cheap when `stats_every` is 0), the setting in
   `config.rs` and `docs/server/settings.md`, the console's `stats`.
5. **The test-window page** (`docs/testing/load-pvp.md`): the settings of 5.1, the stop and start of the two boxes, the seed, R0 to R5's
   table to fill. Then R0 and R1 on the drill without the client.
6. **R2 to R5 with the maintainer**, and the fixes they call for.

**Done when**: R0 to R4 are run and written up in `docs/testing/load-pvp.md`, with the largest battle the server and the client held
and what limits each; the bottlenecks found are fixed or listed.

## 9. Later uses (not in this build)

- **Partner mode** (`bot partner <account>`): one bot driven from the terminal that accepts trade, party, guild and duel invites from a
  named player and follows them, for the two-client items of `docs/testing/live-test-solo.md`.
- **Bot-only scenarios**: checklist items that need no eyes (an answer packet and a database row), run as a test command.
- **The soak test**: hundreds of bots spread over the maps, hours long, for the 1.0 capacity figure.
