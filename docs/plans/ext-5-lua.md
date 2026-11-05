# Extensions chunk 5, Lua (spec and build plan)

**Specified 2026-11-04; not built.** Chunk 5 of `docs/plans/extensions.md` (section 6, work item 7.5). Chunks 1 and 2 are built
(`docs/plans/ext-1-foundation.md`, `docs/plans/ext-2-rules.md`, the operator's page `docs/server/rules.md`): extensions with their
manifest and their rows, settings in four layers, rules at eight points of the core, `reload`. This chunk adds **scripts**: Lua files an
extension carries, stored in the database at install, run by the game server in a sandbox of their own. A script **reacts to events**
(login, a kill, a timer, a chat command, …), **acts through a small, versioned API** (items, gold, buffs, notices, teleport, monsters spawned
for a while, values stored per character), and may **refuse an action or change a number** at the points of chunk 2, after the rules. It
also opens the extension's **options** (the manifest's `[options]`, the file's `[ext.<name>]`, the editor's `ext.<name>.<key>`), reserved
since chunk 1 for their first consumer. Release **0.3.0** follows this chunk (`docs/plans/extensions.md` section 2a decision 4).

Background read for this spec: `docs/plans/extensions.md` sections 2, 2a, 5, 6, 7.5; `docs/plans/ext-1-foundation.md` sections 5 and 7;
`docs/plans/ext-2-rules.md`; `docs/server/rules.md`; `crates/game-server/src/rules/`, `mob_lua.rs`, `zone/lua.rs`, `config.rs`,
`server/console.rs`, `server.rs` (the world loop, chat, `award_exp`, `in_world`), `server/items.rs` (`use_item`), `server/admin.rs`
(notices, `make_items`, the admin orders), `zone/pvp.rs` (`player_died`), `zone.rs` (the monster's death); `crates/data-editor/src/rules.rs`,
`settings.rs`, `ext/`; `docs/original/instances.md` sections 9 to 15; `docs/client/research-extension-features.md` (entries 11 and 12 of
the client research queue) and `docs/client/gm-and-commands.md` ("Player chat commands and prefixes", "Adding commands"); mlua 0.12.2 with
`lua51`, `vendored`, `send` (`crates/game-server/Cargo.toml`).

## 1. The maintainer's decisions

From 2026-11-02 (`docs/plans/extensions.md` section 2a): **Lua scripts get the game API only** (no files, processes, network or raw SQL;
stored values per character and per script through the API); **the first scripts the API must serve** are the welcome notice, the advert
banner and custom chat commands; boss and kill rewards; event spawns ("monster madness"); kill streak and bounty.

From 2026-11-04 (this spec):

1. **Scripts live in the database**, as rows do. `ext install` and `ext upgrade` copy an extension's scripts into `content.ext_scripts`
   after compiling them; the game server reads them from there at start and at `reload` and never opens an extension folder. For authors
   writing a script, **`data-editor ext scripts <folder>`** replaces only the scripts (and option specs) of the installed extension, without a
   version bump; then `console <id> reload`. (The folder read by the server, and a development-only setting reading from it, were offered
   and not chosen.)
2. **Players type script commands with `!`**: `!bounty Name 1000`. The client sends a line starting with `!` and no space as normal chat
   (`! name text`, with the space, is its whisper prefix; `docs/client/gm-and-commands.md`). Only words a script registered are taken; any
   other line stays chat, as today.
3. **The reference kill-streak script has both bounties**: a streak of N kills puts a server-paid bounty on the killer that grows with the
   streak, paid to whoever ends it; players may add gold to a player's bounty with `!bounty <name> <gold>`, taken from them at once and paid
   to the next player who kills that target (never refunded).
4. **No Lua at the attack point in API 1.** It is asked inside the zone's locks for every blow, area victim, thorns and field pass; rules
   own attacks (`deny attack`, `level_gap`). Lua gets the seven other points of chunk 2, plus chat and item use (section 6).

Decided here (no behaviour of the original or of 0.2.0 changes, so not asked):
- **One Lua state per extension.** Its files (`scripts`, in order) run in one state and share its globals; extensions never share a state
  or call each other. The extension is the unit an operator enables, disables and sees disabled after faults ("disable that script" of the
  plan is "disable that extension's scripts").
- **Notification events run on one script thread, queued; points run on the asking connection's task, synchronously.** No script ever runs
  inside a zone lock (section 4).
- **Everything a script does to a player is an order** carried out by that player's own connection, as the GM orders are
  (`AdminOrder`, `ZoneEvent::Admin`); notices, spawns and reads act on the world directly. Results come back to an optional callback.
- **A script is asked after the rules**: a rule's refusal is final (no script is asked); a script may refuse what the rules let through, and
  changes the number the rules gave (section 6.2).
- **A failing script never stops the server**: at start an extension whose scripts do not load runs without them; at `reload` a script that
  does not load fails the whole reload and nothing changes (the operator is there to read the error), as rules do.
- **Options take effect at `reload`** (they belong to scripts, which reload), unlike settings (restart).
- **The four reference scripts are bundled extensions, not installed by setup**: `extensions/kill-streak`, `welcome`, `boss-rewards`,
  `monster-madness`. A default install still behaves as the original (release plan: original behaviour by default).

## 2. What changes

| Today (chunk 2 built) | After chunk 5 |
|---|---|
| manifest keys `scripts`, `api`, `options` refused as reserved | an extension's scripts, its API version and its options (section 3) |
| `[ext.<name>]` in `config/game.toml` refused as reserved | the extension's option values (3.4) |
| editor key `ext.<name>.<key>` refused as reserved | the editor's option values, target `setting`, effect live (3.4) |
| `data-editor ext …` | stores scripts and option specs at `install` and `upgrade`; new `ext scripts <folder>` (3.3) |
| Lua runs only the original's monster scripts (`mob_lua.rs`, `zone/lua.rs`) | unchanged, plus the extensions' scripts in a module of their own, `script/` (section 8) |
| `reload`: monsters, drops, rules | also scripts, options and the extensions' stored globals, all or nothing (9.2) |
| console commands | also `scripts` (9.3) |
| settings | also `scripts` (on/off) and `script_memory_mb` (8.2) |
| the rules' points | the same points (attack excepted) ask the scripts after the rules, plus two new points: chat, item use (section 6) |
| no events | nine events and two kinds of registered handler (commands, timers) (section 5) |

## 3. An extension with scripts

### 3.1 The folder and the manifest

```
extensions/kill-streak/
  extension.toml
  main.lua             the files `scripts` names, relative to the folder
  README.md
```

```toml
format = 1
name = "kill-streak"
version = "1.0.0"
title = "Kill streaks and bounties"
description = "Announces player-kill streaks, puts a growing bounty on a streaker and lets players add gold to a bounty."
api = 1                          # the Lua API version the scripts are written for (section 7); required with `scripts`
scripts = ["main.lua"]           # loaded in this order into the extension's one Lua state

[options.announce_at]
type = "integers"
default = [5, 10, 15, 20, 25]
doc = "Streak lengths announced to everyone."

[options.bounty_gold]
type = "integer"
default = 100000
min = 0
max = 100000000
doc = "Gold the server puts on a streaker's head for each kill from bounty_from on."
```

- `scripts`: a list of 1 to 64 paths, each `^[a-z0-9_-]+(/[a-z0-9_-]+)*\.lua$`, a file of the folder (no `..`, no absolute path), at most
  256 KiB each and 1 MiB together, UTF-8. A path listed twice is an error.
- `api`: a whole number; this server offers **1** (`game_server::script::API_VERSION`). `api` without `scripts`, `scripts` without `api`, and
  an `api` above the server's (`needs a newer game server: API 2, this one has 1`) are refused at `ext check` and `install`.
- `[options.<key>]`, key `^[a-z][a-z0-9_]{0,39}$`, as chunk 1 section 7.5 specified: `type` (`string`, `integer`, `float`, `boolean`,
  `maps`, `strings`, `integers`), `default` (required, of that type), `doc` (required, one line), `min` and `max` (integer and float only).
  At most 64 options. Strings printable ASCII, at most 1000 bytes; lists at most 256 entries; `maps` as the rules' `maps` (ids and
  `"a-b"` ranges).
- `crates/data-editor/src/ext/manifest.rs` `RESERVED` becomes empty (and goes); its three tests become parse tests.

### 3.2 What the database keeps

Migration `db/migrations/0033_scripts.sql` (guarded with `IF NOT EXISTS` as 0031 and 0032):

```sql
CREATE TABLE IF NOT EXISTS content.ext_scripts (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    seq       smallint NOT NULL,              -- the order of `scripts`
    path      text NOT NULL,
    source    text NOT NULL,
    sha256    text NOT NULL,
    PRIMARY KEY (extension, path),
    UNIQUE (extension, seq)
);
CREATE TABLE IF NOT EXISTS content.ext_options (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    name      text NOT NULL,
    spec      jsonb NOT NULL,                 -- type, default, doc, min, max as in the manifest
    PRIMARY KEY (extension, name)
);
-- Values a script stores (section 7.6): per character, and the extension's own.
CREATE TABLE IF NOT EXISTS content.script_values (
    extension    text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    character_id integer NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    key          text NOT NULL,
    value        jsonb NOT NULL,
    PRIMARY KEY (extension, character_id, key)
);
CREATE TABLE IF NOT EXISTS content.script_globals (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    key       text NOT NULL,
    value     jsonb NOT NULL,
    PRIMARY KEY (extension, key)
);
```

The manifest JSON (`content.extensions.manifest`) carries `api`, `scripts` and `options` as written. `ext remove` drops everything with
the extension (the cascade), stored values included; `ext disable` keeps it all (decision 4 of chunk 1: disable means "behaviour off").
The editor's option values live in `content.setting_values` under the name `ext.<extension>.<key>` (chunk 1 section 7.5).

### 3.3 The commands

- `ext check <folder>`: also reads every script, compiles it, and **dry-runs the load** (section 9.1) in a sandbox whose API records the
  registrations and does nothing else (`game_server::script::check(&Manifest-like input) -> Result<Summary>`): a file that does not
  compile names the file and line; a load error names it too; the summary lists the events, points, commands, item takeovers and
  timers the scripts register, printed by `ext check` and `ext show`.
- `ext install` / `upgrade`: the same checks, then `content.ext_scripts` and `content.ext_options` written (upgrade: replaced). The
  stored values are kept across an upgrade (a key the new version no longer uses stays until the script deletes it).
- **`ext scripts <folder>`** (decision 1): the folder's manifest must name an installed extension with the same `version`; only
  `api`, `scripts` and `options` are taken. Everything else must be as installed: the manifest JSON without those three keys equal to
  `content.extensions.manifest` without them, the rows the folder computes equal to `content.ext_rows` (the comparison `ext upgrade`
  makes, by key and row), and no migration file new or changed; otherwise it is refused with "rows, rules, settings or migrations
  changed: use `ext upgrade` with a higher version", naming the first difference. Same checks as install for the scripts; logged in
  `content.ext_log`; prints "`console <id> reload` puts them in force".
- `ext list` gains a scripts count; `ext show` lists the scripts (path, size, SHA-256), the options (with their defaults and the values in
  force from the database layer) and the dry run's summary.
- The hints (`install`, `upgrade`, `enable`, `disable`, `remove`, `scripts`): "`console <id> reload` puts its scripts in force" when the
  extension has scripts, beside the existing lines.

### 3.4 Options and their layers

As chunk 1 section 7.5, now built. Lowest first:

1. the manifest's `default` (`content.ext_options`);
2. the file: `[ext.<name>]` in `config/game.toml`, `<key> = <value>`;
3. the data editor: target `setting`, key `ext.<name>.<key>`, value as JSON text.

Only enabled extensions' options count. Validation by the spec's `type`, `min`, `max`; an unknown key, a wrong type or a value out of
range is an error naming the place (`config/game.toml line 12: ext.kill-streak.bounty_gold: -5 is below 0`, `data editor (change set
14): ext.welcome.text: …`). Errors stop the start and fail a `reload`, as settings and rules do. `[ext.<name>]` for an extension that
is not installed or not enabled is a **warning** (the operator may have disabled it and kept its lines), not an error.

- `config::read_file`: `[ext.<name>]` parses into the file layer (`FileLayer.options: Vec<FileOption { extension, key, value, line }>`),
  nested tables refused; the reserved message and its tests go.
- `data-editor`: `settings.rs` accepts `ext.<name>.<key>` when the extension is installed and the key is one of its options
  (`content.ext_options`), validated with `game_server::script::check_option(&spec, &value)`; the effect line says `live: console <id>
  reload` for these keys (restart for the others).
- `game-server config` lists the options of each enabled extension after the settings, each with its layer (`# extension default`,
  `# config file`, `# data editor (change set 14)`); `config --example` gains a commented `# [ext.<name>]` example.
- In Lua, `game.options` (section 7.1): a read-only table, `maps` as a set (`game.options.maps[35] == true`), lists as arrays.

## 4. How scripts run

### 4.1 The parts

`crates/game-server/src/script/` (a module of its own; `mob_lua.rs` and `zone/lua.rs` untouched):

| file | holds |
|---|---|
| `mod.rs` | `ScriptSet` (the extensions' states in force, swapped by `reload`), `ExtScript` (one extension: its `Mutex<mlua::Lua>`, its handlers, its fault count and state), the per-point and per-event lists (section 10), `API_VERSION`, `check`, `check_option` |
| `sandbox.rs` | the Lua state: libraries, removals, the instruction and time hook, the memory limit (section 8) |
| `api.rs` | the `game` table and the player handle (section 7), built once per state at load |
| `runner.rs` | the script thread: the job queue, timers, the callbacks of orders |
| `store.rs` | the stored values: the cache, the write-behind queue (section 7.6) |
| `tests.rs` | unit tests with a fake world |

`ScriptSet` lives in `Zones` as the rules do (`Zones::scripts() -> Arc<ScriptSet>`, `set_scripts`), with the job queue's sender
(`Zones::script_jobs`), so the zone's own code can queue an event and every connection can ask a point.

**The world the API acts on** is a trait object, `Arc<dyn ScriptWorld>` (implemented for `Context<S>`, set once the `Arc<Context>`
exists, held weakly), so the script module needs no backend generics: find a player and read its numbers (through `Zones`), send a
packet to one, a map, a faction or everyone, put an order for a player, spawn and remove monsters (`zone/admin.rs` `generate`, the GM's
and `LuaCreateMob`'s path), item, skill and monster definitions (`GameData`), the staff to tell of a disabled extension.

### 4.2 Events: queued, on the script thread

An event (section 5) is a **job** on a bounded queue (`std::sync::mpsc::sync_channel`, 10,000 jobs), sent with `try_send`: whoever
raises it (the zone under its locks, a connection) never waits; a full queue drops the job with a warning (at most one line every 10 s).
One OS thread (`script runner`, supervised as the other tasks: restarted after a panic) takes the jobs in order, runs each enabled
extension's handler for it in install order, then the timers that are due, the callbacks of finished orders, and sleeps until the next job
or timer. Each payload carries a snapshot of the players it names (id, name, level, faction, map, kills), taken where the event was raised.

### 4.3 Points: asked synchronously, outside every lock

A point (section 6) runs the handler on the asking connection's task: it locks the extension's state (`std::sync::Mutex`), resets the
budget, calls the handler, reads the answer. **Invariant: no point is asked while a zone lock, `ctx` lock or other mutex is held**; the
builder checks each call site, and the attack point is left out for this reason (decision 4). The wait for the state is bounded by the
longest call another holder may make (an event's time limit, 20 ms); the call itself by a point's (5 ms).

**Lock order**: a script state's mutex is taken first, a zone's locks (inside API calls) after; zone code never takes a script state's
mutex (it queues). So no cycle exists. API calls never call into another extension's state.

### 4.4 Orders: what a script does to a player

`p:give_item`, `take_item`, `give_gold`, `take_gold`, `add_buff`, `teleport` become a `ScriptOrder` queued for that player
(`Zones::script_order(id, order)`, as `admin_order`), which wakes its connection with `ZoneEvent::Script`; the connection carries the
orders out in its world loop as for `ZoneEvent::Admin` (`server/script.rs`, new), with the same code the GM and item paths use, and
sends each order's result back to the runner as a job (`OrderDone { extension, callback, result }`). An order for a player who is not
online on this server is answered `false` at once; a player who logs out with orders waiting gets them dropped (answered `false`).

So no script call is ever **re-entrant**: what an order causes (a map change, an item in the bag) raises its own event later, on the
thread, after the handler that ordered it returned.

## 5. The events

| event | when (place) | handler gets | raised from |
|---|---|---|---|
| `login` | the character enters the world: the first zone entry after the character screen (`server.rs`, `state.entered = true` the first time) | `p` | the connection |
| `logout` | the character leaves the world (`in_world`, after the world loop, before the saves) | `p` (offline from then on: reads give `nil`, orders `false`) | the connection |
| `map_enter` | every zone entry, the first included (after `login`) | `p`, `map` | the connection |
| `map_leave` | a zone left (`leave_zone`) and the logout | `p`, `map` | the connection |
| `kill` | a monster died with a player of most damage and a fair kill (`zone.rs` at `ZoneEvent::Kill` / `PartyKill`, once per death) | `p` (most damage), `mob` (section 7.3), `near` (the players the kill was shared with: the party members near, or `{p}`) | the zone |
| `boss_kill` | as `kill`, when the monster is a boss of a boss group (`Origin::Boss`, `world.boss_groups`); raised after `kill` | `p`, `mob`, `near` | the zone |
| `pvp_kill` | a player killed another for the faction (`zone/pvp.rs` `player_died`, where `ZoneEvent::PlayerKill` goes: not a duel, a guild war or a flagged death) | `killer` (of record), `victim`, `last_hit` (or `nil`), `near` | the zone |
| `death` | a player died (`player_died`, every death) | `victim`, `by` (`"player"`, `"monster"`, `"npc"`), `killer` (a handle, or the monster's id), `pvp` (true when `pvp_kill` was raised for it) | the zone |
| `command` | a player's `!word …` of a word the extension registered (section 7.2) | `p`, `args` (the rest of the line) | the connection |
| (timer) | a timer the extension set is due | — | the runner |

Not in API 1 (later, each a place to read when wanted): level up, quest end, party and guild changes, item picked up, trade done,
map enter refused (the waves' map points, chunk 2 section 5.4).

## 6. The points

### 6.1 The points of API 1

The script is asked **after** the core's checks and the rules, and only when they let the action through (or after the rules changed the
number). Every point but `chat` and `item_use` is one of chunk 2's; its place is chunk 2's section 5.1, re-read when building.

| point | place | handler gets | may return |
|---|---|---|---|
| `trade` | `tp::OP_REQUEST` in `server.rs`: `Zones::trade_request` splits into `trade_check(map, a, b) -> Result<(), Refused>` (the core's checks and the rules, nothing changed) and the request itself, which checks again; the script is asked between them | `a` (asker), `b` (asked) | `false[, text]`: refused (`START_BUSY`, as a rule's refusal) |
| `personal_shop` | `server/my_shop.rs` `mp::OP_BEGIN`, after the rule test | `p` | `false[, text]` |
| `trade_chat` | `chat::OP_TRADE`, after the rules and the cooldown test (a line refused by a script uses no cooldown) | `p`, `text` | `false[, text]` |
| `chat` (new) | the other player chat opcodes (`OP_NORMAL`, `OP_WHISPER`, `OP_ZONE`, `OP_SHOUT`, `OP_PARTY`, `OP_GUILD`, `OP_PARTY_LEADER`, `OP_ALL_SERVER`), after the ban, flood and markup checks and the `!` commands, before the line goes out; not the staff's `F1xx` lines | `p`, `channel` (`"normal"`, `"whisper"`, `"zone"`, `"shout"`, `"party"`, `"guild"`, `"union"`, `"all"`), `text`, `to` (a whisper's name, else `nil`) | `false[, text]`: the line is dropped (the sender sees nothing unless `text` is given) |
| `exp` | `award_exp`, after the rules' factor | `p`, `gain` (after the rules), `source` (`"mob"`, `"party"`, `"quest"`, `"pvp"`) | a factor 0 to 100 |
| `pvp_kills` | `server/pvp.rs` `kill_counted`, after the rules' `times` (a rule's 0 is final) | `p`, `times` | a whole number 0 to 100 |
| `pvp_death` | `ZoneEvent::Died`, after the rules' `deaths` test | `p` | `false`: the death is not counted |
| `enchant` | `server/lapis.rs` `enchant`, on success, after the rules' jump | `p`, `item` (`type`, `type_id`, `step` before), `jump` | a jump 1 to 19 (the step stays at most 20) |
| `item_use` (new) | `server/items.rs` `use_item`, **only for the items a script registered** (`game.on_item`, section 7.2) | `p`, `item` (`type`, `type_id`, `bag`, `slot`, `count`) | `"use"` (one is used up and the use answered done), `"keep"` (done, nothing used up), `false[, text]` (refused, `use_failed`), `nil` (the core's own use, as without the script) |

`item_use` for a registered item: the alive, bag, slot and definition checks as now, then `item_use::may_use` (level, race, mode), then the
reuse group's time (`state.reuse.start`), then the script; `nil` goes on with the core's path from the `Effect::Refused` test. **An item
no script registered takes exactly today's path**: the reordering applies to registered items only. Two enabled extensions registering the
same item is a load error of the later one.

A text a script returns with a refusal is whispered from `Server` as a rule's (`refused::tell_refused`; printable ASCII, at most 100 bytes,
markup-safe or dropped), at most once per 10 s per extension and point and player (`Refusal.rule` = an id per extension and point, new at
each load).

### 6.2 Several extensions, and the order

- The full order of a number's changes: the original's formula → the console's switches → item effects → **the rules → the scripts**, in
  install order, each script asked with the number the ones before it gave.
- **Refusals**: the first extension (install order) that refuses ends the asking; later ones are not asked. A rule's refusal comes first
  and no script is asked.
- **Factors** (`exp`, `pvp_kills`): multiplied. `award_exp` applies the product of the rules' and the scripts' factors once
  (`ruled_exp(gain, bonus, factor)`), so a script that returns 1 (or nothing) leaves the numbers byte for byte as they are.
- **`enchant`**: each asked with the jump so far; the last answer stands.
- **`item_use`**: one extension per item (above).
- A handler that returns nothing (`nil`) changes nothing. A wrong answer (a factor of 200, a string from `exp`, `false` from `enchant`)
  changes nothing and counts as a fault (8.3).

## 7. The API, version 1

Names are stable within an API version: version 1 may gain functions and optional arguments, never lose or change one; a change that
breaks scripts is version 2, and chunk 8 sets how long a server keeps offering an older version. Lua 5.1. Every script function sits in
one global table, `game`, plus the player handles it returns.

### 7.1 Reading

| function | gives |
|---|---|
| `game.api`, `game.extension` | `1`; the extension's name |
| `game.options` | the options in force (3.4), read-only |
| `game.now()` | Unix seconds (`os.time()` too) |
| `game.player(id)`, `game.find(name)` | a player handle (online on this server), or `nil`; `find` ignores case |
| `game.online()` | the online players' handles (at most 2,000), and `game.online_count()` |
| `game.item_name(type, type_id)`, `game.mob_name(id)` | the definition's name or `nil` |
| `game.commands()` | `{ word, help, staff }` for every command of every enabled extension (for a `!help`) |

**The player handle** `p`: `p.id`, `p.name` (fixed), and methods that read the live world (`nil` once offline):
`p:online()`, `p:level()`, `p:faction()` (`"light"` or `"fury"`), `p:map()`, `p:position()` (x, y, z), `p:kills()` (the number the client
shows), `p:job()` (0 to 5), `p:staff()` (0 for a player, else the staff level 1 to 10), `p:guild()` (id or `nil`), `p:party()` (the handles
of its party or union online on this server, itself included; `{p}` alone), `p:has_feature(name)` (`false` in API 1: there is no client
handshake yet, client research entry 23).

### 7.2 Registering

Only while the scripts load (their top level), except timers:

| function | does |
|---|---|
| `game.on(name, fn)` | an event (section 5) or a point (section 6); one handler per name per extension (a second is a load error) |
| `game.command(word, fn, { staff = 0, help = "…" })` | `!word` (decision 2): `word` `^[a-z][a-z0-9_]{0,15}$`; `fn(p, args)`; `staff = 0` everyone, `n` staff of levels 1 to n only (others: the line stays chat); a word another enabled extension registered is a load error of the later one |
| `game.on_item(ids, fn)` | the `item_use` point for these items, `ids` a list of `"type/type_id"` that exist in `defs.items` |
| `game.every(seconds, fn)`, `game.after(seconds, fn)` | a timer (1 to 86,400 whole seconds; at most 64 per extension), its id; `game.cancel(id)`. Timers run on the script thread; a timer due while the previous run of the same timer is late runs once |

**Commands**: a player's `0x1101` line (or a staff's `F101` line) that, after an optional colour marker (`<<NNNcCC`, the 8 bytes the
client adds with a chat colour), starts with `!` and a registered word, followed by a space or the end, is the command: it is not
relayed, it counts in the chat bucket like any line (R4), a chat ban drops it as it drops chat, and `command` is queued for the extension.
For staff of console level, a console command (`.word`) is unaffected: it uses `.`, scripts `!`. Unregistered `!lines` stay normal chat.

### 7.3 What events and points pass

`mob` (events `kill`, `boss_kill`): `{ id = <monster definition id>, uid = <the monster's object id>, name, level, map, x, z, boss
(boolean), spawned (the extension that spawned it, or nil) }`. `item` as section 6.1. Handles as 7.1 (snapshots for the numbers at the
event, live through the methods).

### 7.4 Acting

| function | does | limits |
|---|---|---|
| `p:message(text)` | a whisper from `Server` (`0x1102`, the rules' path) | 120 bytes, cut into lines as the console's answers |
| `p:notice(text)` | a notice to that player (the GM's `NoticeKind::To` packet, "[Notice]") | 127 bytes |
| `game.notice_all(text)`, `game.notice_faction(f, text)`, `game.notice_map(map, text)` | the world notice (`F106`, the console's `notice`), the country and zone notices (`NoticeKind::Country`, `Zone`) | 1,999 bytes all; 127 the others |
| `game.banner(text[, name])` | the centre-screen line to everyone (`0x1108`, "[chatting all members]"; sender `name`, default `Notice`) | 127 bytes; how it reads in the client: entry 22 |
| `p:give_item(type, type_id, count[, cb])` | an order: `count` (1 to 255) into the bags as `make_items` does (stacks, empty slots in order, one save, a `S` make type, the game log's `created` record); what does not fit is not given; `cb(given)` | |
| `p:take_item(type, type_id, count[, cb])` | an order: all or nothing, from the bags (not the warehouse, not equipped); `cb(ok)` | |
| `p:give_gold(n[, cb])`, `p:take_gold(n[, cb])` | orders: `n` 1 to 2^31 − 1; give caps at 2^32 − 1 in hand (the rest not given), take is all or nothing; both through `store.trade`; `cb(amount)`, `cb(ok)` | |
| `p:add_buff(skill_id, level[, cb])` | an order: the skill applied as the item path applies a buff skill (`item_skill`: a `BUFF` skill only, its own time and rules, `item_skill_refused` respected); `cb(ok)` | |
| `p:teleport(map, x, y, z[, cb])` | an order: `move_player` as a GM summon (refused while dead or a logout is pending); `cb(ok)` | a map with a world file |
| `game.spawn(mob_id, count, map, x, z, opts)` | monsters made as `LuaCreateMob` makes them (never respawn), placed within `opts.radius` (0 to 50, default 10) of the spot, removed quietly when `opts.lifetime` seconds (required, 10 to 3,600) pass (`0x0602`, nothing gained), with drops and experience unless `opts.drops = false` / `opts.exp = false`; returns their `uid`s | 50 per call, 300 alive per extension |
| `game.despawn(uids)`, `game.alive(uids)` | removes them quietly; how many still live | the extension's own only |
| `game.log(text)` (and `print`) | an info line in the server log, `extension = …`; at most 20 per second per extension, the rest counted | 1000 bytes |

**Texts to players**: bytes 0x20 to 0x7E (others become `?`), and a text that `chat::markup_is_safe` refuses raises a Lua error (the colour
marker crash, `docs/client/client-packets.md`). **Rate limits** per extension, each overrun a Lua error (a fault): notices to many and
banners 20 per minute; messages and notices 5 per second per player; orders 50 per second per player.

### 7.5 Stored values

| function | |
|---|---|
| `p:get(key)`, `p:set(key, value)` | the character's values of this extension; `nil` deletes |
| `game.get(key)`, `game.set(key, value)` | the extension's own values |

Keys `^[a-z0-9_.:-]{1,64}$`; values `boolean`, a finite number, or a string of at most 1,000 bytes (tables are not stored in API 1); at
most 200 keys per character per extension and 10,000 global keys per extension (a set past them is a Lua error). `p:get`/`p:set` on a
player who is not online here: `nil` / a Lua error (so a bounty on someone offline lives in `game.set("bounty.<id>", …)`).

### 7.6 How the values are kept

- **Per character**: loaded at the login, before `login` is queued (one query, `Backend::script_values(character_id)`; the connection
  does it, the values of enabled extensions only), into the runner's cache (`store.rs`, a `Mutex<HashMap>`); after the `logout` handlers
  ran, the character's pending writes are flushed and its values leave the cache.
- **Globals**: loaded per extension at start and at `reload` (`Backend::script_globals()`).
- **Writes**: a `set` changes the cache at once and queues an upsert or delete; a writer task (tokio, one connection from the pool)
  coalesces by key and writes every 2 s and at shutdown (`Backend::write_script_values(batch)`, one transaction). A failed write is
  retried at the next round and logged; a value is never read back from the database while cached.
- The memory backend implements the three for the tests.

## 8. The sandbox

### 8.1 The Lua state

One state per extension: `mlua::Lua::new_with(StdLib::MATH | StdLib::STRING | StdLib::TABLE | StdLib::OS, LuaOptions::default())` (the base
library is always there), then:
- **removed**: `dofile`, `loadfile`, `load`, `loadstring` (Lua 5.1's `loadstring` takes bytecode), `require`, `module`, `getfenv`,
  `setfenv`, `newproxy`, `string.dump`; `os` replaced by a table with `time`, `date`, `difftime`, `clock` only; `collectgarbage` replaced
  by one that answers `"count"` and refuses the rest; `print` is `game.log`;
- **never loaded**: `io`, `package`, `debug`, `coroutine` (not needed by API 1; leaving it out keeps the hook on one thread);
- **sources** load as text only (`ChunkMode::Text`; a binary chunk is refused), named `<extension>/<path>` for the error lines.

There is no file, process, network or SQL access: the only way out of the state is `game` (decision of section 2a).

### 8.2 Limits

| | instructions per call | time per call | |
|---|---|---|---|
| the load (all the files' top level together) | 5,000,000 | 500 ms | |
| an event, a command, a timer, a callback | 1,000,000 | 20 ms | |
| a point | 200,000 | 5 ms | |

The hook (`set_hook` every 1,000 instructions, as `mob_lua.rs` does every 10,000) counts the instructions and reads the clock; time spent
inside API calls counts. Past either limit the call ends with an error naming the extension and the handler. **Memory**:
`Lua::set_memory_limit` at `script_memory_mb` (a setting, 1 to 256, default 16) per state; an allocation past it is an error. A setting
`scripts` (default `true`): `false` loads no script at all (an operator's switch; restart).

### 8.3 What failing does

- An **error** in a call (a Lua error, a limit, a wrong answer at a point): the call is as if the handler were not there (a point allows
  and keeps the number; an event's handler is cut short; what it already ordered stands); a warning line (`extension`, `handler`,
  the error; at most one per extension every 10 s, the others counted in the next line); one **fault** counted.
- **5 faults within 60 s, or one memory error**: the extension's scripts are **disabled** until the next `reload` or start: its points answer
  as if absent, its events, commands, item takeovers and timers stop, its spawns live out their lifetime, its stored values stay. An
  error line in the log, and a whisper from `Server` to each online staff member of console level (`extension kill-streak: scripts
  disabled after 5 errors in 60 s: <last error>`). The console's `scripts` shows it (9.3). Nothing else changes.
- **A script that does not load** (a compile error, a top-level error, a bad registration, an unknown item in `on_item`, a clash):
  at start that extension runs without scripts (an error line; `scripts` shows the reason); at `reload` the reload fails and nothing
  changes (9.2).
- **Never blocking a zone**: no zone pass, tick or lock runs a script (events are queued with `try_send`); a point runs on one
  connection's task, outside the locks, bounded by its limit; the longest wait for a state is an event's limit.

### 8.4 The original's monster Lua stays apart

`mob_lua.rs` and `zone/lua.rs` (one state per Lua monster, the original's functions, `INSTRUCTION_LIMIT`) are not touched and share
nothing with `script/`: other states, another API, no `game`, their own budget. The test of the chunk's "done when" is their tests passing
unchanged.

## 9. Loading, reload, seeing

### 9.1 Loading

At start, after the rules (`main.rs`): with `scripts = true`, the enabled extensions' scripts (`content.ext_scripts`, by `install_seq` then
`seq`), option specs and globals are read (`Backend::script_sources()`); options resolved over the file and the editor (3.4; an error
stops the start); for each extension a state is made (8.1), `game` built with its options, every file run in order within the load
limit, and the registrations closed. An extension that fails is left out with its reason. The `ScriptSet` goes into `Zones`; the runner
thread starts; the writer task starts.

### 9.2 `reload`

`read_everything` (`server/console.rs`) also reads the script sources, options and globals and builds a new `ScriptSet` (every extension
loaded afresh, the dry state of none reused). Any error (an option, a script that does not load) fails the reload: nothing is swapped,
rules and monsters included, and the answer is the error (`reload failed, nothing changed: extension welcome: main.lua line 12: …`). On
success the rules and the scripts are swapped together; the old states finish the calls they are in and are dropped; their timers end
(the new states' top levels set their own); callbacks of orders still out are dropped (the orders themselves are carried out); Lua
globals start empty (what must survive lives in stored values); extensions disabled after faults run again. The answer gains
`, <n> scripts of <m> extensions`.

### 9.3 Seeing the scripts

- Console `scripts` (in game `.scripts`, help entry): one line per enabled extension with scripts, `<name> <state> <handlers> timers
  <n> spawns <n> faults <n>`, where `state` is `running`, `disabled (<reason>)` or `not loaded (<reason>)`, cut to the console's whisper
  width; `no scripts` when there are none.
- `game-server config` shows the options (3.4); `data-editor ext show` the scripts and the dry run.
- Log lines: `info` at load per extension (files, handlers, timers); `debug` per point answer that changed something (`extension`,
  `point`, `before`, `after`), as the rules' lines; `warn`/`error` of 8.3.

## 10. Cost

1. **No script, nothing done.** `ScriptSet` holds, per point, the list of extensions with a handler, and a bit per event; each point and
   each event site begins with that list or bit: one branch and the `Arc` snapshot (taken once per operation, as the rules').
2. **Events** build their payload and `try_send` only when an extension handles them; the zone keeps its pace whatever the scripts do.
3. **Points** cost a mutex, a budget reset, a few small tables and the call: a few microseconds for a handler of a few lines. The hot one
   is `exp` (each monster kill, per party member); `chat` each line. Commands are found in a `HashMap` of the snapshot, only for lines
   starting with `!`.
4. **Reads** through a handle take `Zones`' player lookup per call (as the console's `map` does), never the database.
5. **Stored values** are memory; the database sees one batched write every 2 s at most.

## 11. The reference scripts

Bundled in `extensions/`, each with a `README.md`, installed on the drill and the dev box in the build, not by `tools/setup.sh`.

### 11.1 `kill-streak` (the reference, decision 3)

Options:

| option | type | default | meaning |
|---|---|---|---|
| `maps` | maps | `[]` (everywhere) | where kills count for a streak and bounties are paid |
| `announce_at` | integers | `[5, 10, 15, 20, 25]` | streak lengths announced to everyone |
| `bounty_from` | integer | `5` | the streak from which the server puts a bounty on the killer |
| `bounty_gold` | integer | `100000` | the server's bounty per kill from `bounty_from` on |
| `bounty_cap` | integer | `10000000` | the server's bounty at most |
| `player_bounty_min` | integer | `10000` | the least gold a player may add with `!bounty` |
| `repeat_secs` | integer | `600` | a kill of the same victim within this time does not count (no farming a friend) |

Stored: per character `streak`, `best`, `last_victim`, `last_at`; globals `bounty.<character id>` (gold players added). Commands:
`!streak` (own streak, best, bounty), `!bounty <name> <gold>`, `!bounties` (the five largest of the players online).

```lua
-- Kill streaks and bounties (docs/plans/ext-5-lua.md section 11.1).
local o = game.options

local function counts(map)
  return next(o.maps) == nil or o.maps[map]
end

local function server_bounty(streak)
  if streak < o.bounty_from then return 0 end
  return math.min((streak - o.bounty_from + 1) * o.bounty_gold, o.bounty_cap)
end

local function placed(id) return game.get("bounty." .. id) or 0 end

local function announced(n)
  for _, at in ipairs(o.announce_at) do if at == n then return true end end
  return false
end

game.on("pvp_kill", function(killer, victim)
  if not counts(killer:map()) then return end
  if killer:get("last_victim") == victim.id and game.now() - (killer:get("last_at") or 0) < o.repeat_secs then return end
  killer:set("last_victim", victim.id)
  killer:set("last_at", game.now())
  local streak = (killer:get("streak") or 0) + 1
  killer:set("streak", streak)
  if streak > (killer:get("best") or 0) then killer:set("best", streak) end
  if announced(streak) then
    game.notice_all(killer.name .. " has killed " .. streak .. " players in a row! Bounty: " .. (server_bounty(streak) + placed(killer.id)) .. " gold")
  end
end)

game.on("death", function(victim, by, killer, pvp)
  local streak = victim:get("streak") or 0
  local bounty = server_bounty(streak) + placed(victim.id)
  victim:set("streak", nil)
  if not pvp or bounty == 0 or not counts(victim:map()) then return end
  game.set("bounty." .. victim.id, nil)
  killer:give_gold(bounty, function(given)
    game.log(killer.name .. " took " .. given .. " gold for " .. victim.name)
  end)
  game.notice_all(killer.name .. " ended " .. victim.name .. "'s streak of " .. streak .. " and took the bounty of " .. bounty .. " gold")
end)

game.command("streak", function(p)
  p:message("Streak " .. (p:get("streak") or 0) .. ", best " .. (p:get("best") or 0) .. ", bounty on you " .. (server_bounty(p:get("streak") or 0) + placed(p.id)) .. " gold")
end, { help = "your kill streak and the bounty on you" })

game.command("bounty", function(p, args)
  local name, gold = args:match("^(%S+)%s+(%d+)$")
  gold = tonumber(gold)
  local target = name and game.find(name)
  if not target or not gold then return p:message("Usage: !bounty <name online> <gold>") end
  if target.id == p.id then return p:message("Not on yourself.") end
  if gold < o.player_bounty_min then return p:message("At least " .. o.player_bounty_min .. " gold.") end
  p:take_gold(gold, function(ok)
    if not ok then return p:message("You do not have " .. gold .. " gold.") end
    game.set("bounty." .. target.id, placed(target.id) + gold)
    game.notice_all(p.name .. " put " .. gold .. " gold on " .. target.name .. "'s head (" .. placed(target.id) .. " in all)")
  end)
end, { help = "!bounty <name> <gold>: add gold to a player's bounty" })

game.command("bounties", function(p)
  local list = {}
  for _, q in ipairs(game.online()) do
    local b = server_bounty(q:get("streak") or 0) + placed(q.id)
    if b > 0 then list[#list + 1] = { q.name, b } end
  end
  table.sort(list, function(a, b) return a[2] > b[2] end)
  if #list == 0 then return p:message("No bounties.") end
  for i = 1, math.min(5, #list) do p:message(list[i][1] .. ": " .. list[i][2] .. " gold") end
end, { help = "the largest bounties online" })
```

The builder may tidy it, keeping its behaviour and its use of the API (it is the API's acceptance test: section 12 step 9).
Placed gold on a player who never dies again stays in `bounty.<id>` (decision 3: never refunded); `ext remove` drops it with the extension.

### 11.2 `welcome` (welcome notice, advert banner, commands)

Options: `text` (string, `"Welcome to the server, {name}!"`; `{name}` is the character's name), `adverts` (strings, `[]`), `advert_minutes`
(integer, 30, 1 to 1440), `advert_kind` (string, `"notice"`: `notice_all`, or `"banner"`). `login` → `p:message` of the text (and `p:notice`
of it); a timer every `advert_minutes` sends the next advert in turn; commands `!help` (the lines of `game.commands()` the player may use),
`!online` (the count), `!time` (the server's local time).

### 11.3 `boss-rewards` (boss and kill rewards)

Options: `item` (string `"type/type_id"`, `""` none), `item_count` (integer 1), `buff` (string `"skill/level"`, `""`), `gold` (integer 0),
`announce` (boolean true), `pvp_item`, `pvp_gold` (the same for a player kill), `pvp_maps` (maps, `[]` everywhere). `boss_kill` → each of
`near` gets the item, buff and gold; the announcement names the killer's party and the boss (`game.mob_name`). `pvp_kill` → each of `near`
gets `pvp_item` and `pvp_gold` on `pvp_maps`.

### 11.4 `monster-madness` (event spawns)

Options: `mobs` (integers, `[]`: any monster may be named), `max_alive` (integer 100, at most 300), `max_minutes` (integer 30), `item`
(string `"type/type_id"`, `""`: no item starts one), `item_mob`, `item_count`, `item_minutes`. Command `!madness <monster id> <count>
<minutes>` with `staff = 3`: spawns them around the staff member (`p:position()`), up to `max_alive` alive, announces it to the map, counts the
kills of its spawns (`kill` with `mob.spawned == game.extension`), announces the end when the last falls or the time ends. `!madness stop`
despawns them. With `item` set, `game.on_item` takes that item over: `"use"`, and the same around the user.

## 12. The client

- **Entry 22 (new, `docs/testing/client-research-queue.md`)**: how script texts read in the client: the longest line each of `F106`, the
  to-one notice, the country and zone notices and `0x1108` shows, wrapping, sound, whether `0x1108`'s sender may be any name (and what
  an empty one does), whether several notices at the login burst all show. Until answered, the API's lengths are the packets' own
  limits and the live test (section 13) shows the look.
- **Entry 23 (new)**: the feature handshake entry 11 (c) found missing: the opcode and format a `fixes.dll` would send after `0x0201`
  to say which features it has, so `p:has_feature` can answer. Until then it answers `false`.
- Nothing else in this chunk needs the client: every packet the API sends is one the server sends today.

## 13. Open for later

- **Lua at the attack point** (decision 4), and the waves' points (chunk 2 section 5.4) with a script consumer.
- Events not in API 1 (section 5).
- **Tables in stored values**, values of offline characters, reading another extension's values.
- `p:has_feature` once entry 23 is answered; an `on_item` for item-use effects once chunk 3's registry exists (chunk 3 may move
  `on_item` onto it, keeping the API).
- The author kit (chunk 8): the guide, `ext check`'s promises, how long an API version is offered.
- Per-server scripts (several game servers on one database): today every server loads every enabled extension's scripts, each with its own
  globals cache; a global written on two servers at once keeps the last write.

## Build

Steps 1 to 10, one after the other; each runs its tests (one cargo command at a time: the machine is short on memory, no parallel builds
or test runs), is committed, and the drill's game server is rebuilt and restarted after each commit that changes the server.

Conventions (as chunks 1 and 2): no `cargo fmt` (wide lines by convention); cargo is `~/.cargo/bin/cargo`; match the surrounding code (doc
comments naming the docs section, `anyhow`, tests next to the code, PostgreSQL tests skipped without `DATABASE_URL`); stage named paths
only (other sessions share the tree); no mention of AI in commits or code. **The development checkout's `.env` points at the drill**
(`shaiya-afterglow-ext1`, port 5434), never at `newshaiya`; the live dev box (worktree `/media/drive0/projects/shaiya-afterglow-live`,
database `newshaiya`) changes only in step 10. Decisions 1 to 4 are the maintainer's; where this plan leaves something open or the code
disagrees with it, the question goes to the maintainer.

**Step 1. The sandbox and the loader, pure** (`script/mod.rs`, `sandbox.rs`, a first `api.rs` with registration only): the state of 8.1,
the limits of 8.2 (instructions, time, memory), loading the files in order with the load limit, `game.on`, `command`, `on_item`, `every`,
`after`, `cancel`, `options`, `log`; `ExtScript` with its handler lists and fault count (8.3); `check` (the dry run, the summary) and
`check_option`; `API_VERSION`. Unit tests: every removed name is `nil` (`dofile`, `loadfile`, `load`, `loadstring`, `require`, `module`,
`getfenv`, `setfenv`, `newproxy`, `string.dump`, `io`, `package`, `debug`, `coroutine`, `os.execute`, `os.getenv`, `os.remove`, `os.exit`)
and `os.time`, `os.date`, `string`, `table`, `math` work; a binary chunk is refused; `while true do end` at load, in an event and in a point
ends within its time and names the handler; `string.rep("x", 2^30)` is a memory error; the budget is per call (a handler within its limit
runs again and again after a runaway, as `mob_lua.rs`'s test); 5 faults in 60 s disable, 4 do not, one memory error does; registrations
after the load are refused (but timers); a second `game.on("login")` is a load error; command words and item ids validated; `check` on a
syntax error names file and line; options by type, `min`, `max`, `maps` as a set.

**Step 2. The manifest, the database, the data editor**: migration 0033 (3.2); `manifest.rs` opens `api`, `scripts`, `options` (3.1; the
reserved tests become parse tests); `ext check`, `install`, `upgrade`, `remove`, `show`, `list` with scripts and option specs (3.3);
`ext scripts <folder>`; the editor's `ext.<name>.<key>` keys (3.4). Tests (data-editor, PostgreSQL): a test extension with two scripts and
options installs, `content.ext_scripts` holds both in order with their hashes, `ext show` prints the dry run; a script that does not compile
refuses `check` and `install` naming file and line; `api = 2` refused; `ext scripts` replaces the scripts, refuses a different version
and a changed `rows.toml`; `remove` drops scripts, options and stored values; an editor draft `set setting ext.test.text` validated against
the spec (a wrong type refused), applied, undone.

**Step 3. Options in the config file and the layers**: `config::read_file` reads `[ext.<name>]` (the reserved tests change); the
resolution of 3.4 (`script::resolve_options(specs, file, editor) -> Result<…>`), errors naming the place; the warning for an extension not
enabled; `game-server config` and `config --example` (regenerate `config/game.toml.example`, its test stays); the settings `scripts` and
`script_memory_mb` in `SETTINGS`. Tests: each layer over the one below; errors at the right line; a disabled extension's `[ext.…]` warns.

**Step 4. The runtime in the server: the set, the runner, reads, notices, the world's events**: `ScriptSet` in `Zones`; `ScriptWorld` for
`Context`; the runner thread and its queue (4.2), supervised; `main.rs` loads as 9.1; the API's reads (7.1) and notices and messages
(7.4 first rows) with the text rules and rate limits; the events `login`, `logout`, `map_enter`, `map_leave`; commands (7.2: the `!` parse
in `server.rs` and in `server/admin.rs` for `F101`, the chat bucket and ban kept); timers; the console `scripts` (9.3); `reload` as 9.2.
`MemoryBackend` gets `set_script_sources`. Tests: unit tests of the command parse (with and without a colour marker, `! name text` not a
command, an unregistered word stays chat, `staff` levels); `game_flow`: `a_script_welcomes_a_player_at_login` (a whisper from `Server` with
the text and the name), `a_script_command_answers_and_is_not_relayed` (a second player near hears nothing), `a_script_timer_sends_a_notice`,
`reload_swaps_scripts_and_a_broken_one_changes_nothing` (rules and monsters unchanged too), `a_failing_script_is_disabled_and_the_server_carries_on`
(a `login` handler that errors: after 5 logins in 60 s, `scripts` says disabled; the sixth login gets no whisper and the world goes on),
`scripts_off_loads_nothing` (setting `scripts = false`). Every existing test passes unchanged; the monster Lua tests in particular.

**Step 5. Stored values** (7.5, 7.6): `Backend::script_values`, `script_globals`, `write_script_values` (PostgreSQL and memory); the cache
and the writer task; the login load before `login`, the flush after `logout`, the shutdown flush. Tests: unit tests of the limits (keys,
values, counts); `game_flow`: a value set by a command is read after a relog (memory backend); PostgreSQL test: write, coalesce (100 sets
of one key: one row, the last value), delete, the cascade on the extension's removal and on a character's deletion.

**Step 6. Orders** (4.4, the order rows of 7.4): `ScriptOrder`, `Zones::script_order`, `ZoneEvent::Script`, `server/script.rs` carrying out
items (as `make_items`), gold (`store.trade`), buffs (`item_skill`'s path), teleport (`move_player`), the results back to the callbacks; an
offline player and a logout with orders waiting answer `false`. Tests (`game_flow`): a command that gives an item puts it in the bag
(`0x0204`, the save), a full bag gives what fits and the callback gets the count; `take_gold` with too little gold leaves the gold and calls
back `false`; `add_buff` applies a buff skill and refuses a non-buff; `teleport` moves the player (`0x020A`-less move as a GM summon); an
order for a player on no map calls back `false`.

**Step 7. The zone's events and spawns**: `kill` and `boss_kill` from `zone.rs` (with `near`), `pvp_kill` and `death` from `zone/pvp.rs`
`player_died`; `game.spawn`, `despawn`, `alive` with the lifetime pass (in the monster updater: spawns past their time removed as
`mob_remove2` removes) and the caps; `mob.spawned`. Tests: `zone/tests.rs`: a kill queues one `kill` with `near` the party near; a boss
group's boss queues `kill` then `boss_kill`; a faction kill queues `pvp_kill` and `death` with `pvp` true, a duel's death only `death`
with `pvp` false; spawns appear, are capped at 50 a call and 300 alive, vanish at their lifetime with nothing gained, `drops = false`
drops nothing. `game_flow`: `killing_a_monster_runs_the_kill_script` (a counter in stored values rises).

**Step 8. The points** (section 6): `trade` (with the `trade_check` split), `personal_shop`, `trade_chat`, `chat`, `exp`, `pvp_kills`,
`pvp_death`, `enchant`, `item_use` (registered items only), the order of 6.2, the refusal whisper with its 10 s limit, the debug lines.
At each site the builder confirms no lock is held (4.3) and writes it in a comment. Tests: unit tests of the combination (two extensions:
first refusal wins and the second is not asked; factors multiply; a rule's refusal asks no script; a wrong answer is a fault and changes
nothing); `game_flow`: an `exp` handler returning 2 doubles `killing_a_monster_gives_experience_and_a_level`'s gain and one returning 1
leaves it byte for byte; a `chat` handler dropping a word: the line is not heard; `trade_chat` refusal whispers and uses no cooldown;
`personal_shop` refusal; `trade` refusal answers `START_BUSY`; `item_use` on a registered item `"use"` uses one up, `"keep"` keeps it,
`false` answers `use_failed`, and an unregistered item's tests pass unchanged; `enchant` jump from a script (`lapis` unit level); a
point handler in an endless loop: the player's action goes through within the point's limit and the fault is counted. With no scripts,
every existing test passes unchanged.

**Step 9. The reference extensions** (section 11): `extensions/kill-streak`, `welcome`, `boss-rewards`, `monster-madness`, each with its
`README.md`; the release tarball carries `extensions/` already (chunk 1 5.1). Tests: `ext check` of each passes (a test that runs `check`
on every folder of `extensions/`); `game_flow` with the kill-streak script loaded from its folder: a streak of `bounty_from` faction kills
(through the test server's kill path, or the events queued directly if a whole player kill has no flow test, as chunk 2 step 6 found)
announces, and the death pays the bounty to the killer; `!bounty` takes the gold and the payout includes it; the same victim within
`repeat_secs` does not count; `welcome`'s login text and `!help`; `monster-madness`'s `!madness` spawns and `!madness stop` removes.

**Step 10. Docs, the drill, the live dev box**:
- `docs/server/scripts.md` (new, published under Settings in `site/mkdocs.yml`, beside `rules.md`): for operators (installing a scripted
  extension, options in the file and the editor, `reload`, `.scripts`, what a failing script does, `scripts = false`) and for authors (the
  manifest keys, the events, the points and the API of section 7 with an example each, the sandbox and limits, stored values, `ext scripts`
  for the edit loop). `docs/server/settings.md`: the two settings, `[ext.<name>]` replacing the reserved sentence. `docs/server/rules.md`:
  "after the rules, scripts" with a link. `docs/server/deviations.md` section 1: scripts are additions, none installed by default.
  `docs/original/operator-console.md` (`reload`'s answer, `scripts`), `docs/original/instances.md` section 18 (the monster Lua is apart
  from the extensions' scripts). `README.md` (features, test count), `CHANGELOG.md` "Unreleased" (chunk 5; no migration step),
  `docs/plans/roadmap.md`, `docs/plans/extensions.md` sections 3 and 7.5 (built), `docs/plans/README.md`, this file's top line (built, with
  what the build settled where the text left a choice). `tools/check_links.py` and `site/build.py` clean.
- `docs/testing/live-test-checklist.md`: section 60 with the items below.
- **The drill**: migration 0033 at the drill server's start; the four extensions installed with `data-editor ext install`; `.scripts` lists
  them running; a script edited and put in force with `ext scripts` and `console <id> reload`; a broken edit makes `reload` fail and changes
  nothing; `game-server config` shows the options.
- **The live dev box** (as chunk 2 step 9): `pg_dump -Fc` of `newshaiya` to `data/backups/pre-ext5-<date>.dump` first; the live worktree
  checked out at the built commit, rebuilt, servers restarted (migration 0033 runs); the four extensions installed **disabled**
  (`ext install`, then `ext disable`), for the maintainer's live test (`ext enable <name>`, `console 1 reload`). Commit; **ask before
  pushing, tagging 0.3.0 or deploying the site.**

**Live-test items** (section 60, for the maintainer with one or two real clients; turn each on with `data-editor ext enable <name>` and
`target/release/console 1 reload`):
1. `welcome`: the login whisper and notice show with the character's name; an advert every `advert_minutes` (set it to 1 for the test) as a
   notice, then as a banner (`advert_kind = "banner"` under `[ext.welcome]`, reload); `!help`, `!online`, `!time` answer and nobody near
   sees the lines.
2. `kill-streak` (`bounty_from = 2`, `announce_at = [2, 3]` for the test): two faction kills announce the streak; the killer's death pays the
   bounty to its killer (gold in the bag, the notice); `!bounty <name> <gold>` takes the gold and the payout includes it; `!streak` and
   `!bounties` answer.
3. `boss-rewards` (an item and a buff set): a boss of a boss group killed by a party gives each member near the item and the buff icon.
4. `monster-madness`: `!madness <id> 20 5` by a staff member spawns them around; they give drops; they vanish after 5 minutes; the end is
   announced.
5. A typo in a script, `ext scripts`, `.reload`: the answer names the file and line, and everything goes on as before.
6. `.scripts` lists the four with their state.

**Done when**: the whole test suite passes (one run, after the last step); the original's monster Lua behaves as before (its tests
unchanged); a script dropped into an extension runs and reloads without a restart; a failing script cannot hurt the server (the fault
tests of steps 1, 4 and 8).
