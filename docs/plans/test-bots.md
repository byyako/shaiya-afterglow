# Test bots and the PvP swarm (spec and build plan)

**Specified 2026-11-12 (first draft and this full version the same day); part 1 built** (`crates/bot`: link, login, enter, world,
metrics, `bot probe`, the `pvp-battle` loop for one bot; 17 tests). What part 1 settled, where the code corrected the spec:

- **Leaving needs sitting**: the server ignores `0x0107` from a standing player (`server.rs`, `OP_LOGOUT_TO_SELECT`), so a bot sends
  `0x0506 1`, then `0x0107`, and waits for the reply.
- **A refused step** is `0x020B` on the bot's own id and then a `0x0502` whose bytes equal result 5 on itself (`world::move_refused`);
  the bot counts the `0x020B` and swallows that one result-5 packet right after it.
- **A step does not stop the server's attack** (only a refused or over-long one does), and a second `0x0502` on the same target is
  ignored; the bot still re-asks after moving, harmlessly. The result codes are `game_server::combat::RESULT_*`.
- **`0x0511`'s number** is the `number` field of the skill's `0x0108` entry, not its place in the list. The game connection's keys are
  `game_session_keys` (the hashed login IV). `0x0301` is not sent; `0x0303` is asked once per player id per session.
- **Every bot needs a weapon in bag 0** (an unarmed bot gets result 7 and only walks), and characters fight sooner from level 10 up;
  a death above level 10 can drop an item. Rebirth kind 2 needs the map's binds, or the dead bot comes back where it fell. Unshelves "Test bots and a load test"
(`docs/plans/roadmap.md`, "Frozen or later"). The first use is a **battle of 200 to 300 players**, to find out whether the server and the
2010 client cope with high-scale PvP. The later uses (a partner bot for the two-client checklist items, bot-only scenarios, a soak test)
build on the same crate and are sketched in section 14.

## In short

- A new crate, `crates/bot`, binary `bot`: `seed` makes bot accounts with level-60 characters built from the definitions, `swarm` logs
  N of them in through the login server like the real client and has them fight, `unseed` removes them, `probe` is `game_probe` as it was.
- Each bot speaks only the client's packets (`0x0501` steps, `0x0502` attacks, `0x0511` skills, `0x0553` rebirth, and the questions the
  client asks about players it sees), so it goes through the same code a player does.
- The game server gains a **stats line** (traffic, outboxes, the monster pass, **lock waits**, saves, memory), useful to any operator
  and the main instrument here.
- The maintainer's real client stands in the battle; the bots measure the server, the maintainer judges the client by eye.
- Runs R0 (10 bots) to R5 (the largest that held, 30 minutes), results in `docs/testing/load-pvp.md`.

## 1. The maintainer's decisions (2026-11-12)

1. **The bots come back, and the PvP swarm is their first job**: 200 to 300 players fighting in one place.
2. **Bots measure the server; the client is judged by eye.** Bots do not run `game.exe`, so they cannot say whether the client keeps up.
   The maintainer stands in the battle with the real client while the bots fight around it; to that client the bots are ordinary
   players, so it receives the full traffic of the fight.
3. **One game server at a time.** The client always connects to the game server on port **30810** (`docs/client/re-network.md`,
   `0x785A`), so the server under test is the one the maintainer's client reaches (section 10).
4. **Where the bots run**: this box first (4 cores, 7.6 GB; builds and tests do not run during a run). If R1's figures show the bots
   themselves eating the CPU the server needs, the 300 run moves the bots to another Linux machine on the LAN (or WSL2 on the Windows
   machine that runs the client).
5. **Where the battle is**: any spot where the factions may fight (every map but 40 and 42, `docs/original/pvp.md` section 1). Proelium
   Frontier (map 18) is the default (section 5.1).
6. **Classes and gear** matter only so far as the bots can move, fight and use skills: all six classes of each faction in equal numbers,
   level 60, ordinary gear (no enchants, no lapis), so that nobody dies in one hit or never dies.
7. **The maintainer's own account** is `player1` with the character TestVail, a GM (status 16) on the game's database since
   2026-11-12, so they can join the battle or watch it freely.

8. **One checkout, one game server (Q1, decided the same day).** The dev box runs the game the maintainer plays from this checkout
   (`.env`'s `DATABASE_URL`, database `newshaiya`); the second database (`shaiya-afterglow-ext1`) is only for `cargo test`, through
   `TEST_DATABASE_URL`. The bots run against the server the maintainer plays on; section 10's run is just its settings. The old second
   checkout (`../shaiya-afterglow-live`) is retired.

## 2. What exists to build on

| what | where | what it gives the bot |
|---|---|---|
| Login, server list, select, game handshake, character screen | `crates/game-server/examples/game_probe.rs` (197 lines) | the whole road to the character screen, with the client version `0x10042600` |
| Entering the world, the cipher switch | `crates/game-server/tests/common/mod.rs` (`Client`, `enter_world`, `select_and_enter`, `read_zone_entry`) | select, the burst up to `0x010B`, `0x0201`, the zone entry |
| Encoders and parsers | `shaiya-proto` (`world`, `combat`, `skill`, `game`, `login`) | every packet in section 4 |
| Accounts | `login_server::accounts::PgAccountStore::create_account` | the server's own password hashing |
| Race and class pairs | `game_server::creation` (`RACE_CLASSES`: human 0, 1, 5; elf 2, 3, 4; vail 2, 4, 5; deatheater 0, 1, 3), `starting_stats` | valid characters |
| Who may wear what | `game_server::inventory::Inventory::can_equip` | gear the server accepts |
| Stats and maxima | `game_server::stats` | HP, MP and SP the server would compute |
| The definitions | `defs.items` (`reqlevel`, `country`, the six class flags `attackfighter` … `defensemage`, `grow`, `reqstr` … `reqluc`, `range`), `defs.skills` (`skilllevel`, `reqlevel`, the class flags, `country`, `grow`, `prevskillid`, `targettype`, `attackrange`, `sp`, `mp`, `resettime`, `readytime`) | seeding and skill choice |
| The map | `world.maps`, `world.map_binds` (map 18: country 1's rebirth box around (409, 20, 143), country 2's around (863, 3, 645)) | the battle point and the walk back |

Facts the design rests on:

- **The cipher.** Client to server is AES-CTR all the way. Server to client is AES-CTR until `0x010B` (the last packet of the
  enter-world burst) and then a fixed XOR table (`shaiya_crypto::XorTable`, seed `LENASOFTFOREVER!`) until `0x0107`. In the world, a bot
  therefore pays a table XOR per packet it receives, not an AES pass: reading the battle is cheap.
- **Who is an enemy.** `0x0201` carries no faction. The client asks `0x0303 u32 id` for a player whose looks it does not know (the
  shape: `country`, `job`, the name, the equipment; `docs/client/re-gameplay-state.md`, shape state `+0x17c`); the server also answers
  `0x0301 u32 id` (HP and speeds). The bot asks `0x0303` once per player that enters its view, and `0x0301` only if a real client's log
  shows the client asking it on entry (build part 1).
- **Attacks run on the server.** `0x0502 u32 id` starts an attack the server keeps ticking at the attacker's attack speed
  (`docs/original/pvp.md` sections 2 and 3) until the target dies, leaves the checks, or is out of reach, which ends it with `0x0502`
  result 5 (`combat::RESULT_OUT_OF_RANGE`; the others: 0 hit, 1 critical, 3 miss, 7 no weapon, 10 cannot attack). Reach: horizontal
  distance ≤ slack + the attacker's range + 2.3. A step cancels the attack, so a bot that moves must send `0x0502` again.
- **Moving.** On foot the speed is `g_ConstUserMoveSpeed[move speed]` = 3, 4, 5, 6, 7 units a second (normal: 5); a step is at most 35
  units; the check allows `t × speed + 50` units per 5-second window (`docs/original/move-check.md`). `0x020B` on one's own id after a
  step means a refusal.
- **Dying.** `0x0504 u32 id, u8 killer kind, u32 killer` to the view; `0x0553 u8 kind` asks to come back (2: at the bind position);
  `0x0551 u32 id, u8 kind, u32 exp, f32 x, y, z` answers. Above level 10 a player killed by a player can drop an item (`pvp.md` 6.2).
- **The locks.** `Zones` (`zone.rs`) keeps every map's players in **one** `Mutex<HashMap<u16, HashMap<u32, Placed>>>` (`maps`) and every
  map's monsters in one more (`mobs`); every lock in the server is taken through `Relock::guard` (`sync.rs`, 671 call sites). They are
  `std` mutexes taken inside tokio tasks: a long wait blocks a worker thread, and this box has four.
- **Logging.** `tools/game-server.sh` defaults to `RUST_LOG=info,game_server=debug`, which logs every guard's and monster's step; with
  300 players that is a load of its own. Runs use `RUST_LOG=info`.

## 3. The crate

A new workspace crate, `crates/bot` (binary `bot`), depending on `shaiya-proto`, `shaiya-crypto`, `login-server` (accounts),
`game-server` (creation, stats, inventory rules), `sqlx`, `tokio`, `serde_json`. Modules:

| module | holds |
|---|---|
| `link` | the framed connection, AES-CTR both ways, the XOR switch after `0x010B` and back after `0x0107` (from `tests/common`) |
| `login` | key exchange, login, server list, select (from `game_probe`) |
| `enter` | game handshake, character screen, select slot 0, the burst (`0x0105` own data, `0x0108` skills, `0x010B` quick slots), `0x0201`, the zone entry |
| `world` | the bot's view of the world: itself, the players in view (id, faction, job, position, HP, alive), its skills |
| `scenario/pvp_battle` | the fight loop of section 5 |
| `metrics` | counters and timings, the 10-second windows, the report (section 6) |
| `seed` | `seed`, `unseed`, the character generator (section 8) |
| `main` | the commands below |

`game_probe` becomes `bot probe` with the same arguments, and `examples/game_probe.rs` is removed. `tests/common` keeps its own `Client`
(it drives an in-process server through the memory backend).

### 3.1 Commands

```
bot seed    --count 300 [--prefix bot] [--map 18] [--level 60] [--template <character id>]...   accounts and characters (section 8)
bot unseed  [--prefix bot]                                                                    removes them (section 8.5)
bot swarm   --login <host:30800> --count 300 [--prefix bot] [--ramp 5] [--scenario pvp-battle]
            [--at <map,x,z>] [--radius 60] [--duration 10m] [--steps-per-sec 2] [--skill-every 6s]
            [--rebirth-after 5s] [--idle-share 0] [--report report.json] [--first 1]                 the battle (sections 5 to 7)
bot probe   <login host:port> <user> <password> [server id] [create <name> | delete <id> | restore <id>]   game_probe as it was
```

`--first` picks the first bot number, so two machines can split one swarm (`--first 1 --count 150` and `--first 151 --count 150`). The
password of every bot account is `SHAIYA_PASSWORD` (as `create_account`). `seed` and `unseed` read `DATABASE_URL` like the other tools.
Exit codes: 0 done, 1 an error, 2 refused (as `data-editor`). `swarm` stops on Ctrl+C or at `--duration`, logs every bot out
(`0x0107`, then closing), writes the report, and prints the summary.

## 4. The protocol a bot speaks

### 4.1 Sent

| when | packet | layout |
|---|---|---|
| login | key exchange, `0xA102` login, server select with `0x10042600` | as `game_probe` |
| game | `0xA301` handshake, `0x0104` select (slot 0's character), `0x0201` enter | as `tests/common` |
| a player enters the view | `0x0303 u32 id` (and `0x0301 u32 id` if the client does, part 1) | as the client |
| walking | `0x0501 u8 motion, u16 dir, f32 x, y, z` | motion 1 (run); `dir` from the heading; `y` as the last position the server sent |
| attacking | `0x0502 u32 id` | once per target, again after a move, an out-of-reach result or a new target |
| a skill | `0x0511 u8 number, u32 target` | `number` is the skill's place in the `0x0108` list |
| dead | `0x0553 u8 2` | after `--rebirth-after` |
| leaving | `0x0107` | at the end of the run |

The `dir` encoding and `y` handling are taken from a real client's log in build part 1 (`RUST_LOG=game_server=debug` shows each
`0x0501`), so a bot's steps look like the client's.

### 4.2 Read

| packet | the bot uses |
|---|---|
| `0x0105`, `0x0108`, `0x010B` (burst) | own id, position, level; the skills and their numbers; the switch to XOR |
| `0x051C` speeds | own move and attack speed (index into the speed table) |
| `0x0201` / `0x0202` | a player enters (id, position) / leaves the view |
| `0x0303`, `0x0301` | that player's country and job / HP |
| `0x0501` (from the server) | another player's step: `u32 id, u8 motion, u16 dir, f32 x, y, z` |
| `0x0502` | an attack's result: `u8 result, u32 attacker, u32 target, u16 × 3 damage`; result 5 to itself means out of reach |
| `0x0511` | a skill's outcome |
| `0x0302` | a player's HP (`u32 id, u16 max, u16 HP`) |
| `0x0521`, `0x0505` | own HP, MP, SP (absolute; gained) |
| `0x0504` | a death: who, by whom |
| `0x0551` | a rebirth: who, where |
| `0x020B` | a put-back: on its own id after a step, a refused move |

Everything else is counted by opcode and dropped. A packet that fails to parse is counted as a protocol error and logged once per opcode.

## 5. The `pvp-battle` scenario

### 5.1 The battle point

Default: the midpoint of the two factions' rebirth boxes on the map (`world.map_binds`); on map 18 about (636, 394), some 340 units from
each, a little over a minute's walk at 5 units a second. `--at map,x,z` overrides it. The bots are seeded standing at their own
faction's rebirth box (section 8), so the battle forms as they walk in, as a real one would. The map's monsters stay: they attack whoever
passes, as in a real fight.

### 5.2 One bot's loop

```
            ┌──────────── no enemy in view ────────────┐
            v                                           │
  WALK_TO_POINT ── enemy in view ──> CHASE ── in reach ──> FIGHT ── target dead / gone ──┐
       ^                               ^   └─── out of reach (result 5) ───┘             │
       │                               └─────────────────── next target ─────────────────┘
       └──── 0x0551 (back at the bind) ──── DEAD <── own 0x0504 (from any state)
```

- **Target**: the nearest enemy (other country, alive, in view) by the last known position; ties by id. A new target only when the
  current one dies, leaves the view or has been out of reach for 5 seconds.
- **Walking** (`WALK_TO_POINT`, `CHASE`, and wandering inside `--radius` when no enemy is in view): steps toward the goal at
  `--steps-per-sec` (default 2), each step `0.9 × speed / steps-per-sec` units (2.25 at speed 5), never more than 35, so no window fails.
  A refusal is counted and the bot takes the server's position.
- **Reach**: the equipped weapon's `range` from `defs.items` (read once by `swarm` for every bot's weapon) plus 2.3, less 0.5 of margin;
  melee and ranged bots therefore fight at their real distances.
- **Fighting**: `0x0502` once, then waiting for results. Every `--skill-every` (default 6 s, with ±30 % jitter so 300 bots do not cast
  in step), the first of its skills that is an attack on a player (`targettype` an enemy, `attackrange` ≥ the distance, cost within its
  MP and SP, its `resettime` passed since its last use): `0x0511`, then `0x0502` again (a skill ends the attack).
- **Dead**: wait `--rebirth-after` (default 5 s, a person reading the screen), send `0x0553 2`, and on `0x0551` walk back.
- **Idle share**: `--idle-share 0.3` makes 30 % of the bots stand at the point and do nothing but read, which lowers the load without
  lowering the crowd the client must draw.
- **Gear loss**: deaths drop items now and then, as for players. A bot without a weapon (`0x0502` result 7) keeps walking with the crowd
  and is counted; `bot seed` re-equips everyone before the next run.

### 5.3 Ramp and end

`--ramp` bots a second log in (default 5; the login server makes 8 RSA keys at start and never more, so faster logins queue on them).
The report counts from the moment the last bot is in the world; the ramp's own figures are kept apart. At the end every bot sends `0x0107`
and closes; the server saves them as it saves anyone who leaves.

## 6. What the bots measure

Per bot, summed per 10-second window for the swarm:

- packets and bytes received and sent, by opcode;
- **the action round trip**: from a `0x0502` sent to the first `0x0502` result for that target (the delay a player feels between the
  click and the first blow), and from a `0x0511` sent to its outcome; p50, p95, p99 and the maximum per window;
- **the step echo**: from a step sent to the same step seen by another bot (sampled: each bot stamps one step in ten), the delay with
  which the others see a player move;
- bots connecting, in the world, dead, disconnected (and why: the server closed, a timeout, an error); logins and their duration;
- move refusals, protocol errors, `0x0502` results by code, kills and deaths.

`report.json`: the run's arguments, the build and server versions, then one object per window with the figures above, and a summary
(the steady-state windows only). The printed summary is one screen: bots in world, the round trips, packets a second per bot, refusals,
errors, disconnects.

## 7. The server under test: the stats line

Added to the game server for every operator, behind a setting `stats_every` (seconds; default 0, off). Every `stats_every` seconds one
`info` line `stats` with these fields, and the same figures from `console <id> stats`:

| field | measured where |
|---|---|
| `players`, `connections` | the existing counters |
| `in_pps`, `in_bps`, `out_pps`, `out_bps` | atomics bumped where the connection reads a frame and where the outbox is drained to the socket (`server.rs` world loop, `outbox.rs`) |
| `outbox_max`, `outbox_sum`, `outbox_closed` | the outboxes' byte counts (`outbox.rs` `Shared`); connections ended by `outbox_limit` since the last line |
| `mob_pass_max_ms`, `mob_pass_avg_ms`, `mob_period_ms` | a timer around each pass of `mob_updater` (`server/tasks.rs`) |
| `lock_wait_ms`, `lock_waits`, `lock_worst` | `Relock::guard` (`sync.rs`) tries `try_lock` first and times only a lock it has to wait for, into a counter per lock type (`type_name::<T>()`); the line gives the total, the count and the three types waited on longest |
| `saves_queued`, `game_log_dropped` | the save writer's queue depth; the game log's dropped records |
| `rss_mb`, `workers` | `/proc/self/statm`; the tokio runtime's worker count |

The lock timing costs nothing when a lock is free (the `try_lock` succeeds) and two clock reads when it is not. It is on even with
`stats_every = 0` only as atomics (no log line); a test checks that `guard` still recovers a poisoned lock.

**Expected to show first**: `maps` (one mutex for every map's players, taken by every step, attack and view change) and the fan-out of
each step to every player in view (about 300 × 300 sends a second for 300 players stepping once a second). If the line shows long waits on
`maps`, the fix is its own change (per-map locks, or taking the view's recipients out of the lock before sending), with the run repeated.

`docs/server/settings.md` gets `stats_every`; `docs/server/deploy.md` mentions it under monitoring; the status board on the dev box
(`~/.local/bin/shaiya-status.sh`) shows the last `stats` line.

## 8. Seeding

### 8.1 What `bot seed` makes

`--count N` accounts `<prefix>0001` … `<prefix>NNNN` (status 0), each with one character `<Prefix>0001` … (the name capitalised, as the
client shows names), alternating factions (odd numbers Light, even Fury) and cycling through the six classes, so 300 bots are 25 of each
faction and class. Each account gets its `account_factions` row (that country, `max_grow` 3).

### 8.2 The character generator

Without `--template`, each character is built from the definitions with the server's own rules:

- **Race and class**: the first race of the faction whose `RACE_CLASSES` row has the class (Light: human, elf; Fury: vail, deatheater).
  Sex, hair, face and size are the first values creation accepts.
- **Mode**: `grow` 1 (normal); refused if the mode's level cap (the experience table) is below `--level`.
- **Level and stats**: `--level` (60), the experience of that level's start; `starting_stats(job, family)` plus every stat point of the
  levels gained, spent on the class's main stat (STR for fighters and defenders, DEX for rangers and archers, INT for mages, WIS for
  priests) up to what the best gear needs, the rest on REC; `stat_point` 0.
- **HP, MP, SP**: the maxima `game_server::stats` computes for that character with that gear, so it enters the world full.
- **Skills**: every skill of `defs.skills` usable by the class (its flag 1), the faction (`country`), the mode (`grow`) and the level
  (`reqlevel`), at the highest `skilllevel` whose `reqlevel` is reached, following `prevskillid`; numbered from 0 in skill id order as
  learning would; `skill_point` 0.
- **Gear**: for each equipment slot the server knows (0 to 13), the item of `defs.items` with the highest `reqlevel` at or below the
  level that `Inventory::can_equip` accepts for this character (class, faction, mode, level, stat requirements), with quality at the
  item's maximum and no gems; slots without a candidate stay empty. A class with a shield takes one.
- **Bags**: 20 of the first HP potion (25/1) and 20 of the first MP potion, as a player would carry; no gold.
- **Quick slots**: the attack skills in their numbers' order on bar 0.
- **Where**: the map and its faction's rebirth box (a random point inside it).

With `--template <character id>` (given once, or once per faction and class), each bot is a copy of that character instead (its row,
items, skills and quick slots), renamed and placed as above; the template must be logged out.

### 8.3 Writing

One transaction per account and character: the account through `PgAccountStore::create_account`, then `account_factions`, `characters`,
`character_skills`, `character_items` (new uids from `item_uid_seq`), `character_quick_slots`. A second `bot seed` with the same prefix
does not make new ones: it puts every existing bot back to the generated (or template) state (gear, bags, HP, position), which is how a
run starts clean after the last one's deaths and drops.

### 8.4 Guards

- Only names matching `^<prefix>\d{4}$` are touched; a prefix that matches any existing account not made by `bot seed` (no bot
  character, or a character name that does not match) is refused.
- Refused while any bot account has a session in the game (`sessions.game_connected_at`): a bot's rows are not changed under a running
  bot. Other players may be online: seeding touches only bot rows.
- `--count` above 1000 is refused (one machine's sensible limit; a larger swarm is several `--first` ranges).

### 8.5 `bot unseed`

Deletes the prefix's characters (with their items, skills, quick slots, quests, friends and the rest by the foreign keys' cascade) and
accounts, in one transaction, after the same guards. It does not touch `game_log`, `chat_log` or the faction kill counts: section 9 keeps
the kill counts out of a run, and the logs keep what happened.

### 8.6 Check

A test in the crate (against `DATABASE_URL`, skipped without it, as the other PostgreSQL tests) seeds two characters, one of each
faction, under a throwaway prefix; loads each through the PostgreSQL store (`CharacterStore::load`, and the items and skills as the game
server loads them) and checks level, stats, the gear in its slots and the skills; then puts the two loaded characters into the memory
backend and enters the world with both in the in-process server: they see each other as enemies, one attacks the other, the attacked
one dies and comes back. `bot unseed` removes them.

## 9. The run's settings

In `config/game.toml` (and for the login server, the environment `tools/login-server.sh` reads), only for a run, set back after:

| setting | default | for the run | why |
|---|---|---|---|
| `max_per_ip` | 8 | count + 10 | every bot comes from one address |
| `connects_per_minute` | 30 | 60 × ramp | the ramp's connections |
| `LOGIN_MAX_PER_IP` | 20 | count + 10 | as above, at the login server |
| `LOGIN_CONNECTS_PER_MINUTE` | 30 | 60 × ramp | as above |
| `LOGIN_KEY_POOL` | 8 | 32 | the key exchanges of the ramp |
| `kill_count` | true | false | the factions' kill counts do not move with bot kills |
| `stats_every` | 0 | 10 | the stats line |
| `RUST_LOG` | `info,game_server=debug` (dev scripts) | `info` | debug logs every step of 300 players |
| the console's `maxusers` | | above count + 1 | |

`chat_per_sec` and the move check stay as they are: bots must pass what a player passes. `docs/testing/load-pvp.md` lists these with a
one-line way to apply them and to put them back.

## 10. Where it runs, and a run step by step

### 10.1 The board

The dev box's wall display (tmux window `shaiya`, `~/.local/bin/shaiya-status.sh`) shows the servers, players online, load, memory and
the last `stats` line; it is the monitor during a run.

### 10.2 A run

1. Settings of section 9 in; restart the game and login servers.
2. `bot seed --count N` (players may be online; only bot rows change).
3. The maintainer logs in with `player1` (GM) and walks to the battle point (or `/move` there).
4. `bot swarm --login <LAN address>:30800 --count N --report runs/RN.json` (a second machine: `--first`).
5. At the end: the report's summary, the server's `stats` lines for the run (`grep stats logs/game-server.log`), the maintainer's notes
   (section 11), into `docs/testing/load-pvp.md`.
6. Settings back; restart.


## 11. The runs

| run | bots | the real client | question |
|---|---|---|---|
| R0 | 10 | none | the bot works: no refusals, no protocol errors, kills, deaths and rebirths happen, the report is filled |
| R1 | 50 | none | the stats line and the report agree; a first load figure; how much CPU the bots take (decision 4) |
| R2 | 100 | the maintainer, in the battle | does the client draw and keep up at 100 |
| R3 | 200 | the maintainer | the same at 200; the lock waits and the round trip |
| R4 | 300 | the maintainer | the same at 300 (bots on a second machine if R1 says so) |
| R5 | the largest count that held | the maintainer | 30 minutes: does anything grow (memory, outboxes, the ground items, the save queue) |

Ten minutes of battle each, from the last login. **What the maintainer notes** in each run with the client: does it draw everyone;
the frame rate (the client's own counter if it has one, otherwise by feel); stutter when the crowd comes into view; whether attacks and
skills land late; freezes or a crash; whether it was dropped (the server logs an `outbox_limit` close for its connection).

**Holding**, to be confirmed after R1 gives a baseline: the p95 action round trip under 250 ms, the step echo under 500 ms, no connection
closed by the server, the monster pass inside its period, and the real client playable.

## 12. Risks

| risk | effect | answer |
|---|---|---|
| one `maps` mutex for the world | waits grow with the square of the crowd; tokio workers block | measured by the stats line; fixed in its own change |
| the client cannot draw 300 | the maintainer's client stutters or crashes | that is the answer the run is for; R2 to R4 find the limit |
| the real client reads slowly | its outbox passes 4 MB and the server drops it | the stats line shows `outbox_max`; `outbox_limit` raised for the run if needed, and the finding noted |
| the bots share the server's CPU | the server looks slower than it is | R1 measures the bots' CPU; decision 4's second machine |
| debug logging | the log is the load | `RUST_LOG=info` (section 9) |
| ground items from PvP deaths | the ground fills over a long run | R5 watches it; items expire as for players |
| the game log | a row per death, drop and kill | on the SSD now; kept, not cleaned |
| bot code that does what no client does | load that is not real | section 4 mirrors the client's packets; part 1 compares with a real client's log |

## 13. Build

Each part is one session, with its tests; `cargo fmt`, clippy with `-D warnings` and the workspace tests pass at the end of each.

1. **The crate and one bot.** `crates/bot` with `link`, `login`, `enter`, `world`, `bot probe` (and `game_probe` removed). The
   `pvp-battle` loop and the metrics for one bot. Test: two bots of the two factions against the in-process server (the memory backend,
   as `tests/common`): they see each other as enemies, step without a refusal, one kills the other, the dead one comes back and walks
   back. Compare the `0x0501` a bot sends with a real client's (direction, `y`) from the dev server's debug log, and fix the encoding.
2. **Seeding.** `bot seed` (the generator of 8.2, `--template`, the reset of 8.3), `bot unseed`, the guards; the test of 8.6.
3. **The swarm and the report.** N bots, the ramp, `--first`, the 10-second windows, `report.json`, the summary; a test with 20 bots
   against the in-process server.
4. **The stats line.** The counters of section 7, the lock timing in `sync.rs` (with the poisoned-lock test still passing), the
   setting, `console stats`, the docs; the status board shows it.
5. **The run page and R0, R1.** `docs/testing/load-pvp.md` (settings, the steps of section 10, the runs' table); R0 and R1 on the dev
   server without the client.
6. **R2 to R5 with the maintainer**, and the fixes they call for.

**Done when**: R0 to R4 are run and written up in `docs/testing/load-pvp.md`, with the largest battle the server and the client held and
what limits each; the bottlenecks found are fixed or listed.

## 14. Later uses (not in this build)

- **Partner mode** (`bot partner <account>`): one bot driven from the terminal that accepts trade, party, guild and duel invites from a
  named player and follows them, for the two-client items of `docs/testing/live-test-solo.md`.
- **Bot-only scenarios**: checklist items that need no eyes (an answer packet and a database row), run as a test command.
- **The soak test**: hundreds of bots spread over the maps, hours long, for the 1.0 capacity figure.

## 15. The Sonnet build prompt (one part at a time)

Parts 1 to 4 are built by a Sonnet subagent each, started by Opus from this prompt with `<N>` filled in (the maintainer's choice,
2026-11-12); Opus reviews each part (the diff, the tests, the spec's section for it) before the next starts. Parts 5 and 6 run against the server the maintainer plays on and are done with
the maintainer, not delegated.

```
Build part <N> of the test bots in /media/drive0/projects/shaiya-afterglow (branch main), from docs/plans/test-bots.md section 13,
part <N> only. Read the spec whole first (sections 2 to 8 hold the facts and the layouts; section 12 the risks), then the code each
section names before you change or copy it: crates/game-server/examples/game_probe.rs, crates/game-server/tests/common/mod.rs,
shaiya-proto (world.rs, combat.rs, skill.rs, game.rs, login.rs), shaiya-crypto, and for part 2 game_server::creation, ::inventory
(can_equip), ::stats and pg/character.rs; for part 4 sync.rs, outbox.rs, server.rs (the world loop), server/tasks.rs (mob_updater),
config.rs. Where the spec and the code disagree, the code is right: follow it, and list each disagreement in your report with the file
and line. Where a choice is not the spec's to make, take the smaller one and list it.

The machine: 4 cores, 7.6 GB, short on memory. Run one cargo command at a time, never two in parallel. The tests: cargo test -p <crate>
while working; at the end of the part cargo test --workspace once (about 25 minutes). The PostgreSQL tests use TEST_DATABASE_URL from
.env (the test database, port 5434); never point anything at DATABASE_URL, which is the game the maintainer plays (newshaiya, 5433).
Parts 1 to 4 test against the in-process server (the memory backend, as tests/common does) and the test database only.

Before each commit: cargo fmt --all and cargo clippy --workspace --all-targets -- -D warnings clean (CI enforces both). Stage the paths
you changed by name. Commit messages: "Bots <N>: <what>", the project's style (a plain sentence, what and why; see git log).
Commits carry no AI trailer and no mention of AI (no Co-Authored-By, no session lines), in the message or the code.

Do not, without the maintainer: push; start, stop or restart the login, game or data editor servers (they serve the maintainer's game);
run bot seed, unseed or swarm against DATABASE_URL or port 30800/30810; change .env or config/game.toml; touch other repositories.

Finish with a report: the commits; the tests added and the workspace test count; every place you departed from the spec, with why;
what is not tested; anything part <N+1> must know.
```
