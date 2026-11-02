# Architecture

How the server is put together: the programs, how a player's login reaches a game server, what runs inside the game server, and
which program writes which part of the database.

## In short

- Players connect to two programs: the login server (port 30800) and a game server (port 30810, one per world). The operator's tools
  are separate small programs.
- PostgreSQL is the only shared state. The programs never talk to each other directly, only through rows and `NOTIFY`, so several
  game servers can share one login server, on one machine or many.
- A login writes a session row; the game server the player picks claims it and takes the cipher keys from it.
- Inside the game server, each connection is a task that owns its player; the maps live in one shared structure with a fixed lock
  order; saves, the guilds, the friends and the game log are written by background tasks so that no player waits for the database.
- The imported game data (`defs`, `world`) is replaced by the importers; the player data (`public`) is made by the migrations.

## Programs

| program | what it does |
|---|---|
| `login-server` | the login connection (port 30800, `LOGIN_BIND`): RSA key exchange, the account check, the server list, server selection; writes the session. Runs the migrations. |
| `game-server` | one process per game server (port 30810, `GAME_BIND`): the handshake, the character screen, the world. Runs the migrations too. |
| `data-editor` | the data editor (`docs/plans/done/data-editing.md`): a web page (`serve`) and a command line that keep every change to `defs` and `world` in the `edits` schema and write them back after each import (`apply`). It reaches the game server through the console channel (`reload`, `restart`). |
| `console` | sends one console command to a running game server (`console <server id> <command…>`) and prints the answer (`docs/original/operator-console.md`). |
| `servers` | manages the `game_servers` rows: `list`, `add`, `enable`, `disable`, `remove`, `heartbeat`. |
| `create_account` | makes an account (`docs/server/settings.md`, the README). |
| `mall` | the item mall's operator side: points, premium, deliveries (`docs/original/item-mall.md`). |

`console`, `servers`, `create_account` and `mall` are binaries of the `login-server` crate (`crates/login-server/src/bin/`). PostgreSQL is the only
shared state: the programs never talk to each other except through it (rows and `NOTIFY`). The crates under them: `shaiya-crypto` (the
login and game ciphers), `shaiya-proto` (every packet's builder and parser).

The original had seven services (login, session, game, dbagent, gamelog, userlog, PSM agents) that talked to each other and to SQL Server. Most of
that existed to work around SQL Server and the old network design, so the two programs players connect to are the whole server, and PostgreSQL
does what the session, dbagent and log services did. Several game servers can run next to one login server, on one machine or many.

## How a login reaches the world

Tables (`db/migrations`, run by both servers at start):

- `accounts`, `login_log`: accounts with the original `Status` semantics, and a record of every attempt.
- `game_servers`: `id`, `name`, `address` (IPv4, one row per game server, unique), `capacity`, an optional required client `version`, `enabled`, and the
  heartbeat fields `players` and `last_seen`. An operator creates the rows (`servers add`).
- `sessions`: one row per account, written at login and replaced by a newer login: `identity` (16 random bytes the client holds), `aes_key` and `aes_iv`,
  `login_ip`, `server_id` (set when the player picks a server), `expires_at`, `game_connected_at` (set when a game server claims the session).

The steps:

1. The client logs in. The login server checks the account, writes the session and lists `game_servers`: a server is **open** while it is enabled and its
   `last_seen` is newer than `LOGIN_HEARTBEAT_SECS` (default 15), so a server that stops reporting shows as closed by itself. The new session also sends
   `NOTIFY account_login`, so a game server that holds the same account's older connection drops it.
2. The client picks a server. The login server checks, in the original's order, that the server exists and is open, that it is not full (staff pass) and, if
   checking is on, the client version; then it binds the session to that server and replies with the server's IP.
3. The client connects to that IP on port 30810 and sends `0xA301` with its user id and identity. The game server claims the session (the identity and its own
   `server_id` must match; a session is claimed once), takes the key and IV from it, and starts the game cipher from `SHA-256(aes_iv)[0..16]`
   (`docs/original/ps_game.md` section 4). The session row is deleted when the connection ends.
4. The game server's heartbeat (every `heartbeat_interval`, 5 seconds by default) writes `players` and `last_seen` to its `game_servers` row (no `last_seen`
   while it is not accepting connections) and renews `expires_at` of every session it has claimed. A session lives 5 minutes after login, 2 minutes after
   selection and 2 minutes after each renewal; the login server removes expired rows a minute after expiry.

**One IP per game server.** The client connects to the game server on port 30810, fixed in `game.exe` (`docs/original/client-login.md`), and the
server-select reply carries only an IPv4 address, so every game server needs its own address: extra addresses on the network card, or separate machines,
VMs or containers. The database enforces it (`game_servers.address` is a unique `/32` IPv4 address).

## Inside the game server

The source files named in this section are under `crates/game-server/src/`.

- **Connections.** `serve` (`server/tasks.rs`) accepts on the listening socket and starts one task per connection (`handle` in `server.rs`) after the
  limits: `/server off`, the console's `maxusers`, the connection permits, and the per-address share (`limits.rs`). The task runs the handshake under a
  timeout, then the character screen, then the world loop. A connection owns its player's state (`PlayCharacter`, `WorldState`) and reads three sources in one
  `select!`: the client's packets, the packets other players' actions queued for it (a byte-limited outbox), and the events the zone sends it (`ZoneEvent`:
  a kill's experience, a death, a loot share, a GM's order).
- **The zones.** There is no task per map. `Zones` (`zone.rs`, `zone/`) is one shared structure holding every map's players, monsters, NPCs, ground items,
  fields, and the parties, trades, guilds, friends, wars, instances, the ranking battle, the GM orders and bans, each behind its own mutex with a fixed
  lock order (written on the struct). A connection takes the locks it needs, changes what it owns, and puts packets for others into their outboxes. The
  data the world is made of (`defs`, `world`, the item and skill tables, the quests, the item mall's products) is loaded once at start into `GameData`;
  `reload` swaps the part that can change (`Live`) for the next pass.
- **Background tasks** (started in `main.rs` and `server/tasks.rs`, each restarted after a panic by `supervise.rs`): the heartbeat; the console's shutdown
  countdown; the monster updater (every 500 ms, or every 100 ms with smooth chasing: fights, walking, respawn, the guards, the fields, recovery, bodies and
  expiring items, in the original's order); the monster id saver; the ranking clock (the guild ranking battle's notices, close and update); the market pass
  (auctions ending); and the listener for `NOTIFY` on `account_login`, `account_points` (the item mall's points changed) and `game_console` (a command from
  the `console` tool or the data editor, answered on `game_console_reply`).
- **Writers.** Saves (`start_saves`), the guilds and the friends each have a queue and a writer task, so a connection never waits for a slow write; a failed
  write is retried, a panic costs that write. On shutdown (`quit`, a signal) every connection is closed as if the client had gone, saved, and the queues are
  flushed.
- **The game log.** A background task takes the records the game makes (a bounded queue of 10,000; nothing waits for it, a full queue drops records with a
  warning) and writes them in batches to `game_log` and `chat_log`, every row carrying the game server's id (`docs/original/game-log.md`, `game_log.rs`).
- **The stores.** The game server talks to storage through small traits (`backend.rs`: `CharacterStore`, `ItemStore`, `GuildStore`, `FriendStore`,
  `MarketStore`, `PointStore`, and others). `pg/` implements them on PostgreSQL (`character.rs`, `items.rs`, `skills.rs`, `quests.rs`, `guild.rs`,
  `friends.rs`, `market.rs`, `points.rs`, `session.rs`, `heartbeat.rs`, `admin.rs`); `memory.rs` implements them in memory for the tests.

## The schemas and who writes them

| schema | holds | made by | written by |
|---|---|---|---|
| `public` | accounts, sessions, game servers, characters and everything a player owns (items, skills, quests, guilds and their banks and houses, friends, the auction house, the item mall's points, the game log, the GM tools' records) | the migrations (`db/migrations`, 0001 onward) | the login server (accounts, sessions), the game server (the rest), the `servers`, `create_account` and `mall` tools |
| `defs` | the game's definitions: items, skills, monsters, drops, the level and status tables | `db/defs/schema.sql` | `tools/defs/load_into_postgres.py` (the import of the original database), `data-editor apply` |
| `world` | maps, NPCs, spawns, portals, obelisks, boss groups, monster AI and Lua scripts, quests | `db/world/schema.sql` | `tools/world/import.py` (the import of the original files), `data-editor apply` |
| `client` | the client's `Item.SData` and `Skill.SData` as shipped, and what a client data patch changes | `db/client/schema.sql` | `tools/sdata/load_client.py`, `tools/sdata/client_rows.sql`; read by `tools/sdata/client_patch.py` |
| `edits` | every change made through the data editor, with its history, the draft, and the map pictures | migration `0030_data_edits.sql` | `data-editor` only |

`defs`, `world` and `client` hold the original's data and are not part of the migrations: the importers replace them, and `data-editor apply` writes the
edits back afterwards. The game server reads `defs` and `world` at start and writes only to `public`.

## Configuration

Every setting comes from the environment (a `.env` in the project root is read at start): `docs/server/settings.md` lists each with its default. Running it:
`docs/server/deploy.md`.
