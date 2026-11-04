# Settings

The login server, the data editor and the tools are configured by environment variables; the servers and tools read `.env` in the working
directory first (git-ignored; start from `.env.example`), and the environment overrides it (the run scripts are the exception: they load
`.env` over the environment, below). Only `DATABASE_URL` is required.

**The game server's settings are not environment variables.** They are keys of a TOML file, `config/game.toml`, laid over the built-in
defaults and the defaults of the installed extensions, and under the data editor's overrides (section "Game server" below). With nothing
set the server behaves as the original EP4.5 server apart from its fixes, the operator's tools and the gambling house (`mini_game`); where a
default still differs from the original, the meaning says so with "(the original: …)". `docs/server/deviations.md` lists every deviation,
and section 1 there the settings that turn on something the original does not have.

How the values are read: an on/off setting is on for `1`, `true`, `yes` or `on` (any case) and off for anything else, so a misspelt
`ture` turns a setting off (in the file write `true` or `false`); a `_secs` setting is whole seconds; a number out of its range, or a value
that does not parse, stops the server at start with the setting's name and where its value came from.

## Login server (`login-server`)

| variable | default | meaning |
|---|---|---|
| `LOGIN_BIND` | `0.0.0.0:30800` | listen address |
| `LOGIN_HEARTBEAT_SECS` | `15` | a game server without a heartbeat for this long is listed as closed |
| `LOGIN_CHECK_VERSION` | `false` | enforce each game server's required client version (set with `servers add ... <version>`; this US client sends `0x10042600`) |
| `LOGIN_MAX_PER_IP` | `20` | simultaneous connections per address (the original: no limit) |
| `LOGIN_CONNECTS_PER_MINUTE` | `30` | new connections one address may open in a minute; the rest are closed unread (`docs/server/audit.md` R10; the original: no limit) |
| `LOGIN_KEY_TIMEOUT_SECS`, `LOGIN_IDLE_SECS` | `30`, `600` | the key exchange must finish within the first of these of the connection, and the login within the second of the key exchange (pings extend neither); at the server list, silence allowed (the original: no deadline) |
| `LOGIN_FAILURE_LIMIT`, `LOGIN_FAILURE_WINDOW_SECS`, `LOGIN_LOCKOUT_SECS` | `20`, `600`, `600` | wrong passwords per address (any account) before the address is locked out; a good login does not clear them (the original never locks out, here or in the two rows below) |
| `LOGIN_PAIR_FAILURES`, `LOGIN_PAIR_WINDOW_SECS`, `LOGIN_PAIR_LOCKOUT_SECS` | `5`, `900`, `900` | wrong passwords for one account from one address before that address is answered "wrong password" for that account, without hashing, for the lockout (`docs/server/audit.md` R7) |
| `LOGIN_ACCOUNT_FAILURES`, `LOGIN_ACCOUNT_WINDOW_SECS`, `LOGIN_TRACKED_ACCOUNTS` | `30`, `3600`, `100000` | wrong passwords for one account from all addresses together per window before every address is answered "wrong password" for it; how many accounts and account-address pairs are remembered (the oldest are dropped) |
| `LOGIN_KEY_POOL`, `LOGIN_MAX_CONNECTIONS`, `RUST_LOG` | `8`, `5000`, `info` | RSA keys generated at start, total connections, log level |

## Game server (`game-server`)

### Start-up variables

Four variables are read by the game server itself; `RUST_LOG` is the logger's. Everything else is a setting of the file.

| variable | default | meaning |
|---|---|---|
| `DATABASE_URL` | none | required: the settings' database layers are read from it, and the migrations run on that first connection |
| `GAME_BIND` | `0.0.0.0:30810` | listen address |
| `GAME_SERVER_ID` | `1` | this server's id in `game_servers` (`servers add`), at most 255 |
| `GAME_CONFIG` | unset | the settings file's path; unset, `config/game.toml` in the working folder (`tools/game-server.sh` sets its checkout's). A missing file is fine for the default path only: a path named here must exist |
| `RUST_LOG` | `info` | log level |

A game setting left in the environment (a `GAME_…` variable other than those four, in `.env` too: the run scripts load it) **stops the
server at start**, so nothing is silently ignored. The message holds the lines to paste into the file, and names a variable that is not a
setting at all (a typo the old server ignored):

```
game settings are read from config/game.toml now, not from the environment (docs/server/settings.md). Put these lines in
config/game.toml (GAME_CONFIG names another file), remove the variables (from .env too: the run scripts load it), and start again:
guild_party_size = "2"                   # was GAME_GUILD_PARTY_SIZE
market_maps = "35,36,42"                 # was GAME_MARKET_MAPS
```

### The settings file and its layers

`config/game.toml` has top-level keys only; the key is the old variable's name without `GAME_`, in lowercase
(`GAME_TRADE_CHAT_MIN_LEVEL` is `trade_chat_min_level`). A value may be a string (exactly the text the variable took), a number, a boolean,
or a list of integers or strings (joined with `,`): `market_maps = [35, 36, 42]` and `market_maps = "35,36,42"` are the same. Every value
then goes through the server's own parser, so each range below holds. `config/game.toml.example` lists every setting commented out with its
default and one line of meaning (`game-server config --example` prints it); `tools/setup.sh` writes a `config/game.toml` of comments when
there is none, and never overwrites one. The file is git-ignored: it is the operator's.

The layers, lowest first; a later one beats an earlier one:

1. **The built-in defaults**, the tables below.
2. **The extensions' defaults**: the `[settings]` of every *enabled* extension (`data-editor ext list`); disabling one stops its
   defaults counting. The Kill Boost Stone's `kill_boost_skills` comes this way. Two enabled extensions never set the same setting.
3. **The config file.**
4. **The data editor's overrides**: the target `setting` (`data-editor draft add set setting <name> value <value>`, then `apply-draft`; a
   removed override falls back to the layers below; History and Undo work as for any edit).

Settings take effect at the next start (`tools/game-server.sh restart`); the console's `exp`, `killrate`, `shop` and `maxusers` still
change their four values live until then. The tables `[ext.<name>]` and `[rules]` of the file, and the editor's `ext.<name>.<key>` keys,
are reserved for later chunks and refused (`docs/plans/ext-1-foundation.md` section 7.5). Errors name the place: `config/game.toml line 4:
unknown setting "trade_chat_minlevel"`, `config/game.toml line 6: market_maps: map id "x"`, `extension kill-boost: kill_boost_skills: …`,
`data editor (change set 12): slot_payout_percent: …`.

`game-server config` prints what is in force and where each value comes from (it reads the same file and database, runs no migrations and
starts nothing):

```
guild_party_size = 2                         # config file (default 7)
kill_boost_skills = "200/1:2,200/2:5,200/3:10" # extension kill-boost (default "")
trade_chat_min_level = 10                    # default
```

At start the server logs one line per setting that is not at its default, with its layer.

### The settings

| setting | default | meaning |
|---|---|---|
| `check_ip` | `true` | the game connection must come from the address the account logged in from (the original did not check) |
| `heartbeat_secs` | `5` | how often the player count and liveness are written |
| `handshake_secs`, `idle_secs` | `30`, `600` | time allowed for the whole handshake from the connect (pings do not extend it), and silence allowed afterwards (counted from the client's last bytes, whatever is sent to it; the original: no handshake deadline) |
| `max_per_ip`, `connects_per_minute` | `8`, `30` | simultaneous connections and new connections a minute from one address; the rest are closed unread (`docs/server/audit.md` R2; the original: no limit) |
| `write_timeout_secs` | `20` | a client that does not take a packet in this long is disconnected as a lost connection (R1; the original: no deadline) |
| `outbox_limit` | `4194304` | bytes of packets queued for one connection before it counts as not keeping up and ends (R1; the original: no limit) |
| `chat_per_sec`, `chat_burst` | `3`, `5` | lines a second and the burst for the chat channels without a cooldown (normal, whisper, zone, party, guild, union); extra lines are dropped quietly; 0 turns the limit off (R4; the original: no limit) |
| `packets_per_sec` | `100` | packets a second a connection may send, with a burst of twice that; over it for 5 seconds in a row ends the connection; 0 turns the limit off (R4; the original: no limit) |
| `db_pool`, `db_acquire_secs` | `24`, `10` | the database pool's size (PostgreSQL allows 100 connections by default; the notification listener has its own) and how long a query waits for one (R11) |
| `delete_delay_secs` | `7200` | a deletion request takes effect after this long (as in the original) |
| `logout_secs` | `10` | a logout request (exit to character selection or quit) takes effect after this long (as in the original) |
| `linger_secs` | `10` | a character whose connection ended in the world (no logout pending) stays there this long, as in the original; a new connection of the account waits for it to go (below) |
| `kick_notice_secs` | `3` | after an operator's kick or ban notice the connection stays open this long (the character already out of the world), so the client shows the notice before its disconnect box; 0 closes at once (`docs/original/small-items.md` deviation 1; the original: closes at once) |
| `move_check` | `true` | the move speed check: steps over five seconds against the move speed, as the original (`docs/original/move-check.md`); `false` leaves only the 35-unit step check |
| `smooth_chase` | `true` | running steps (monsters chasing and going home, guards, walking NPCs, the Lua moves) wait for the client's own walking time, with a step every 100 ms between the passes, so they move without stops (`docs/original/chase-smoothing.md`); `false` keeps the original's waits (the original: `false`; kept on as a display fix, `docs/server/deviations.md` section 9) |
| `log` | `true` | the game log (`docs/original/game-log.md`): what the original sent to its log server is written to PostgreSQL (`game_log`; the chat to `chat_log`) |
| `chat_log` | `true` | the chat log: every chat line of types 1 to 7, whispers and staff chat included, as the original |
| `log_days` | `0` | rows of the game log and the chat log older than this many days are deleted daily; `0` keeps them, as the original |
| `item_mall` | `true` | the item mall sells and sends gifts (`docs/original/item-mall.md`); `false` refuses both as the original's `g_bDisableShop` |
| `town_secs` | `10` | `/town` moves the player to the map's bind position after this long standing still (as in the original) |
| `handshake_wait_secs` | `15` | a new connection of an account whose older one is still leaving (ended by a newer login, or its character still staying) waits this long for it before being refused (`docs/original/client-extras.md`; the original: refused at once) |
| `trade_chat_min_level`, `trade_chat_cooldown_secs` | `10`, `5` | trade chat (`$`, faction-wide): lowest level and time between messages, as the US client limits it; 0 turns a limit off (the original server: no check) |
| `cross_faction_trade_maps` | empty | maps (comma-separated ids) where players of both factions may trade, for example `42` (the Auction House); empty for none, as the original (an addition: `docs/server/deviations.md` section 1) |
| `market_maps` | empty | maps (comma-separated ids) where a personal shop may be set up, for example `35,36,42` (the capitals and the Auction House, where the client offers it); empty for every map, as the original (a limit on any map list is an addition: `docs/server/deviations.md` section 1) |
| `guild_party_size` | `7` | the party size needed to create a guild (the original: exactly 7); 1 (or 0) lets a player create one alone |
| `kill_count` | `true` | the factions' kill counts move with player kills (`docs/original/pvp-2.md` section 9), as the original's console default |
| `ranking_times` | empty | the guild ranking battle's open times as `(weekday,hour,hours)` entries, e.g. `(1,14,1)` or `(1,14,1),(3,10,2)` (`docs/original/guild-ranking.md` section 10); empty keeps map 50's `CreateTime`, as the original; a test override |
| `ranking_update_delay_secs` | `1800` | how long after the ranking battle closes the ranking is updated (as in the original) |
| `exp_rate` | `1` | the monster experience event's factor at start, 0 or more (`docs/original/operator-console.md`); the console's `exp` changes it until the next start |
| `kill_rate` | `1` | what each counted player kill adds to the killer's kills, 1 to 100 (in place of the original's `[NEW_SERVER]`); the console's `killrate` changes it |
| `kill_boost_skills` | empty | skills that multiply a counted kill while applied, comma-separated, each `id[/level][:times]` (`id` alone is any level, `times` 1 to 100, 2 when left out; the highest matching entry counts); empty for none, as the original (an addition: `docs/server/deviations.md` section 1). The three Kill Boost Stones' buffs are `200/1:2,200/2:5,200/3:10` (items 100/212 to 214, 2×, 5× and 10×; their rows come from the extension `extensions/kill-boost`; the client's copy comes with client data patch 109, `docs/plans/done/client-patches.md`) |
| `mini_game` | `true` | the gambling house (`docs/original/mini-game.md`): `true` places the six type-5 NPCs (maps 1, 2, 19, 20, 28, 29) and handles the `0x31xx` slot machine and card double-up with the original's odds (the two rows below); `false` hides the NPCs as the original does (an addition, on by default: `docs/server/deviations.md` section 1) |
| `slot_payout_percent` | `100` | each winning slot line pays multiplier × the table's bet × this / 100 (1 to 1000); 100, the original's table, gives back about 41 % of the bets, 220 about 90 % (used only with `mini_game`) |
| `card_win_chances` | `400,200,100,50,10` | the double-up's chance (per mille, rounds 1 to 5) that the card falls on the guessed side; the default is the original's (fairer odds, for example `420,200,130,100,80`, are an operator's choice; used only with `mini_game`) |
| `console_level` | `1` | 1 to 5: staff levels 1 to this may type `.command` lines (`.help`, `.info`, `.exp 2`, …) in the game; the answers are whispers from `Server` |
| `max_connections` | `5000` | total connections |

## Data editor (`data-editor`)

| variable | default | meaning |
|---|---|---|
| `DATABASE_URL` | none | required |
| `EDITOR_EXPORT_FILE` | unset | when set, every Apply, Undo and Import writes the export there |
| `EDITOR_BIND` | `127.0.0.1:30890` | the page's listen address; only loopback, private and Tailscale clients are served whatever it is, and a request must name `localhost`, `127.0.0.1`, `[::1]` or a host in `EDITOR_HOSTS` |
| `EDITOR_HOSTS` | unset | extra host names (comma separated) the page answers to; anything else gets 403 |

## Tools (`tools/`, `compose.yaml`)

The import scripts reach the databases through `tools/dbconn.py` and `tools/dbconn.sh`, which read `.env` like the servers;
`compose.yaml` reads the same file.

| variable | default | meaning |
|---|---|---|
| `DATABASE_URL` | none | the user, password, host, port and database the tools use (as the servers) |
| `SHAIYA_PASSWORD` | unset | the new account's password for `create_account` (old name `NEWSHAIYA_PASSWORD`, still read); unset, it reads one line from standard input (so the password is not in the process list) |
| `PG_PASSWORD` | none | the password `compose.yaml` creates the PostgreSQL container with; must equal the password in `DATABASE_URL` |
| `PG_CONTAINER` | `shaiya-afterglow-postgres` | the PostgreSQL container: its `container_name` in `compose.yaml`, and where the tools run `psql` |
| `MSSQL_CONTAINER` | `shaiya-afterglow-mssql` | the SQL Server container of the import (`compose.yaml` profile `import`) |
| `SHAIYA_PSQL` | `auto` | (old name `NEWSHAIYA_PSQL`, still read) how the tools run `psql`: `docker` inside `PG_CONTAINER`, `host` with the host's `psql "$DATABASE_URL"`, `auto` the first when that container is running, else the second when `psql` is on the PATH |
| `PSM_DATA_DIR` | `./PSM_Data` | the folder of the original SQL Server backups, mounted read-only at `/backups` in the SQL Server container; `tools/setup.sh` sets it from its first argument |
| `PG_PORT`, `MSSQL_PORT` | `5433`, `14333` | the host ports (on 127.0.0.1) of the two containers; `PG_PORT` must match `DATABASE_URL` |
| `PG_VOLUME`, `MSSQL_VOLUME` | `shaiya-afterglow-pgdata`, `shaiya-afterglow-mssql-data` | the containers' data volumes |
| `COMPOSE_PROJECT_NAME` | the folder's name | docker compose's project; a second copy on one machine needs its own, with its own container and volume names and ports |

## Run scripts

`tools/login-server.sh`, `tools/game-server.sh` and `tools/data-editor.sh` source `.env` (with `set -a`, so a variable in `.env` wins over
the same one in the shell; give a one-off value in the shell only for a variable `.env` does not set; `tools/game-server.sh` also sets
`GAME_CONFIG` to the checkout's `config/game.toml` when that file exists and the variable is unset), then set `RUST_LOG` to
`info,login_server=debug` / `info,game_server=debug` (the data editor: `info`) unless it is set by then; `tools/data-editor.sh`
keeps `EDITOR_BIND` at `127.0.0.1:30890` unless it is set. The logs go to `logs/<name>.log`.

## Tests

| variable | default | meaning |
|---|---|---|
| `DATABASE_URL` | unset | the PostgreSQL tests use it (from `.env`) and skip themselves without it; the tests that need `defs` or `world` also skip on an empty database |
| `PROPTEST_CASES` | `5000` | cases per property of the `#[ignore]` deep twins in `fuzz_dispatch` and `fuzz_login` (the ordinary run has fixed counts); the README's "Development" has the command (`docs/server/audit.md` section 5) |
| `FUZZ_LOG` | unset | a `tracing` filter for `fuzz_dispatch`, e.g. `game_server=debug`, to see what the server logs while tracing a failure |
| `FUZZ_TRACE` | unset | when set, `fuzz_dispatch` prints each case's opcodes before it runs |
