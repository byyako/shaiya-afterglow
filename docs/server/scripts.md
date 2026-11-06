# Scripts

An extension may carry **Lua scripts**. A script reacts to events (a login, a kill, a timer, a chat command), acts through a small,
versioned API (items, gold, buffs, notices, teleports, monsters spawned for a while, values stored per character), and may refuse an
action or change a number at the places the [rules](rules.md) are asked. Scripts are an addition: no script is installed by default, and
with none the server behaves as the original. `docs/plans/ext-5-lua.md` is the design; this page is how to use them.

**Not released yet.** Scripts are on `main` and come with 0.3.0; the latest release, 0.2.0, does not have them. The extension system is
still being built, so this page may change before then.

The scripts are the extensions' own. The original's monster scripts (`Data/Lua/*.lua`, the Cailum Sacra bosses) are another thing and
are not touched (`docs/original/scripted-mobs.md`, `docs/original/instances.md`).

## For operators

### Installing a scripted extension

    target/release/data-editor ext check extensions/kill-streak      # compiles the scripts and shows what they register
    target/release/data-editor ext install extensions/kill-streak
    target/release/console 1 reload                                  # puts the scripts in force, no restart

`install` and `upgrade` compile the scripts, run their top level once in a state that does nothing (the *dry run*), and copy them into
the database (`content.ext_scripts`); the game server reads them from there and never opens the extension's folder. `ext show <name>` lists
the scripts (size and SHA-256), the options with their defaults and the data editor's values, and what the dry run registered.
`disable` and `remove` take an extension's scripts out of force at the next `reload`; `remove` also drops the values the scripts stored.

Four extensions are bundled in `extensions/`, none installed by default: `kill-streak` (streak announcements and bounties), `welcome`
(a login text, adverts, `!help`, `!online`, `!time`), `boss-rewards` (rewards for a boss group's boss and for player kills),
`monster-madness` (`!madness` for staff). Each has a `README.md` with its options.

### Options

An extension's options are named in its `extension.toml` with a type, a default, a limit and a line of documentation. Their values come
in three layers, lowest first:

1. the manifest's default;
2. the config file, a table named after the extension: `[ext.kill-streak]` with `bounty_gold = 50000`;
3. the data editor: `data-editor draft add set setting ext.kill-streak.bounty_gold value 50000`, then `apply-draft`. The value is JSON;
   the editor checks it against the option's type and limits, and the effect is *live*.

`reload` takes new option values, as it takes new scripts (settings, by contrast, wait for a restart). `game-server config` lists each
enabled extension's options with the layer of each value (`# extension default`, `# config file`, `# data editor (change set 14)`). A
wrong value is an error that names the place (`config/game.toml line 12: ext.kill-streak.bounty_gold: -5 is below 0`) and stops the start
or fails the `reload`. Lines for an extension that is not installed, is disabled or has no scripts only warn: you may have turned it off
and kept its lines.

### Seeing and changing the scripts

`console <id> scripts` (in the game, `.scripts`) lists one line per extension: `<name> <state> <n> handlers timers <n> spawns <n> faults
<n>`, where the state is `running`, `disabled (<reason>)` or `not loaded (<reason>)`.

`reload` reads the scripts again together with the rules and the monsters, all or nothing: a script that does not compile or fails at its
top level fails the whole `reload`, names the file and line, and changes nothing (`reload failed, nothing changed: extension welcome:
welcome/main.lua:12: …`). On success the answer gains `, <n> scripts of <m> extensions`. The old states finish the calls they are in;
their timers end; Lua variables start empty (what must survive lives in stored values); extensions disabled after errors run again.

For a script author editing a file on the server: `data-editor ext scripts <folder>` replaces only the scripts and option specs of the
installed extension of the same version, without a version bump (it refuses if the folder's rows, rules, settings or migrations differ
from what is installed: then `ext upgrade` with a higher version), then `console <id> reload`.

### When a script fails

A script never stops the server. An error in a call (a Lua error, a limit, a wrong answer at a point) leaves things as if the handler were
not there and is logged as a warning (one line per extension every 10 seconds, the others counted). **Five errors within 60 seconds, or
one memory error, disable the extension's scripts** until the next `reload` or start: its events, commands, timers and item takeovers stop,
its monsters live out their time, its stored values stay. Staff of console level get a whisper (`extension kill-streak: scripts disabled
after 5 errors in 60 s: …`) and `scripts` shows the reason.

`scripts = false` in `config/game.toml` loads no script at all (restart). `script_memory_mb` (1 to 256, default 16) is the memory limit of
one extension's Lua state.

### Where a failing script cannot hurt

No script ever runs inside a zone lock or any other lock: events are queued to one script thread, and a point (a refusal or a number)
runs on the asking player's own connection, outside every lock, within 5 ms. A script that loops forever is cut at its instruction or
time limit. See *The sandbox* below.

## For authors

### The manifest

```toml
format = 1
name = "kill-streak"
version = "1.0.0"
title = "Kill streaks and bounties"
description = "…"
api = 1                     # the Lua API version the scripts are written for; required with `scripts`
scripts = ["main.lua"]      # 1 to 64 files, loaded in this order into the extension's one Lua state

[options.bounty_gold]
type = "integer"            # string, integer, float, boolean, maps, strings, integers
default = 100000
min = 0
max = 100000000
doc = "Gold the server puts on a streaker's head."
```

Script paths are `^[a-z0-9_-]+(/[a-z0-9_-]+)*\.lua$` inside the folder, at most 256 KiB each and 1 MiB together, UTF-8. `api` without
`scripts`, `scripts` without `api`, `[options]` without `scripts` and an `api` above the server's (`needs a newer game server: API 2, this
one has 1`) are refused. Option keys are `^[a-z][a-z0-9_]{0,39}$`, at most 64; strings are printable ASCII up to 1,000 bytes; lists up to
256 entries; `maps` takes ids and `"a-b"` ranges. In Lua, `game.options` is a read-only table of the values in force: `maps` as a set
(`game.options.maps[35] == true`), lists as arrays.

One Lua state per extension: its files share globals; extensions never share a state or call each other.

### Events

Registered with `game.on(name, fn)` while the files load; one handler per name per extension. Events are queued and run on the script
thread, after what raised them; a handler that orders something (a gift, a teleport) sees the result in a callback, later.

| event | when | handler gets |
|---|---|---|
| `login` | the character enters the world | `p` |
| `logout` | it leaves (reads of `p` give `nil` from then on) | `p` |
| `map_enter`, `map_leave` | every zone entry (the first included) and exit | `p`, `map` |
| `kill` | a monster died with a player of most damage and a fair kill | `p`, `mob`, `near` |
| `boss_kill` | as `kill`, for a boss group's boss (after `kill`) | `p`, `mob`, `near` |
| `pvp_kill` | a player killed another for the faction (not a duel, a guild war or a flagged death) | `killer`, `victim`, `last_hit` or `nil`, `near` |
| `death` | a player died | `victim`, `by` (`"player"`, `"monster"`, `"npc"`), `killer` (a handle, or the monster's id), `pvp` |

`mob` is `{ id, uid, name, level, map, x, z, boss, spawned }` (`spawned` is the extension that spawned it with `game.spawn`, or `nil`);
`near` is the players the kill was shared with (the party members near, or just `p`). Chat commands and timers are registered apart:

```lua
game.command("bounty", function(p, args) … end, { staff = 0, help = "!bounty <name> <gold>" })
game.every(1800, function() game.notice_all("Visit the shop") end)   -- seconds, 1 to 86,400; at most 64 timers
local id = game.after(60, fn);  game.cancel(id)
```

A player types `!word args` (the client sends it as normal chat; a colour marker in front is ignored, any case is read as lower case).
Only words a script registered are taken; any other line stays chat, and so does `! name text` (the client's whisper prefix). A command is
not relayed; it counts in the chat flood limit, and a chat ban drops it. `staff = 0` is everyone, `staff = n` the staff of levels 1 to n
(a player typing it gets plain chat). A word another enabled extension registered is a load error of the later one.

### Points: refusing and changing numbers

A **point** is a place where the server asks the script *after* its own checks and the [rules](rules.md) (a rule's refusal is final: no
script is asked). It runs on the asking connection, in 5 ms. Register with `game.on("trade", fn)` and so on; a handler that returns
nothing changes nothing, and a wrong answer (a factor of 200, a string from `exp`) changes nothing and counts as an error.

| point | handler gets | may return |
|---|---|---|
| `trade` | `a` (asker), `b` | `false[, text]`: refused (the asker is told the request is busy) |
| `personal_shop` | `p` | `false[, text]` |
| `trade_chat` | `p`, `text` | `false[, text]` (a refused line uses no cooldown) |
| `chat` | `p`, `channel` (`"normal"`, `"whisper"`, `"zone"`, `"shout"`, `"party"`, `"guild"`, `"union"`, `"all"`), `text`, `to` | `false[, text]`: the line is dropped |
| `exp` | `p`, `gain` (after the rules), `source` (`"mob"`, `"party"`, `"quest"`, `"pvp"`) | a factor 0 to 100 |
| `pvp_kills` | `p`, `times` | a whole number 0 to 100 (multiplies the counted kill) |
| `pvp_death` | `p` | `false`: the death is not counted |
| `enchant` | `p`, `item` (`type`, `type_id`, `step`), `jump` | a jump 1 to 19 |

`game.on_item({ "25/1", "100/131" }, fn)` takes over the use of items (each `"type/type_id"` must exist; two extensions cannot take the
same one). `fn(p, item)` (`item` has `type`, `type_id`, `bag`, `slot`, `count` and `effect`, the type of the item's extension effect
in force such as `"level_up"`, or `nil`) is asked after the level, race and reuse checks and
returns `"use"` (one is used up), `"keep"` (done, nothing used up), `false[, text]` (refused) or nothing (the core's own use, or, for an
item that has an extension effect, that effect: `docs/server/effects.md` section 4). The order of an item's use is the core's checks, the
rules' `item_use` point, the reuse group, this script, the extension's effect, the core's effect.

With several extensions, in install order: the first refusal ends the asking; factors multiply (each extension is asked with the number
the earlier ones gave); `enchant`'s last answer stands. A refusal's text is whispered from `Server` (printable ASCII, up to 100 bytes),
at most once per 10 seconds per extension, point and player; without a text the player sees nothing.

### The API (version 1)

Names are stable within an API version: version 1 may gain functions and optional arguments, never lose or change one.

Reading: `game.api`, `game.extension`, `game.options`, `game.now()` (Unix seconds), `game.player(id)`, `game.find(name)`,
`game.online()`, `game.online_count()`, `game.item_name(type, type_id)`, `game.mob_name(id)`, `game.commands()`. A **player handle** `p` has
`p.id`, `p.name` and `p:online()`, `p:level()`, `p:faction()` (`"light"` or `"fury"`), `p:map()`, `p:position()` (x, y, z), `p:kills()`,
`p:job()`, `p:staff()` (0, or the staff level it entered the world with), `p:guild()`, `p:party()` (handles of its party or union online
here), `p:has_feature(name)` (always `false` in API 1). The reads go to the live world and give `nil` for a player who is not online;
handles compare equal by character.

Acting:

| call | does |
|---|---|
| `p:message(text)` | a whisper from `Server`, cut into lines of 120 bytes (up to 1,200) |
| `p:notice(text)` | a notice to that player (127 bytes) |
| `game.notice_all(text)`, `game.notice_faction("light"/"fury", text)`, `game.notice_map(map, text)` | the world, country and zone notices (1,999 / 127 / 127 bytes) |
| `game.banner(text[, name])` | the centre-screen line to everyone (127 bytes) |
| `p:give_item(type, type_id, count[, cb])` | `count` 1 to 255 into the bags as a GM's make; what does not fit is not given; `cb(given)` |
| `p:take_item(type, type_id, count[, cb])` | all or nothing from the bags (not the warehouse, not what is worn); `cb(ok)` |
| `p:give_gold(n[, cb])`, `p:take_gold(n[, cb])` | `n` 1 to 2³¹−1; give caps at 2³²−1 in hand; take is all or nothing; `cb(amount)`, `cb(ok)` |
| `p:add_buff(skill_id, level[, cb])` | a buff skill as the item path applies one; `cb(ok)` |
| `p:teleport(map, x, y, z[, cb])` | as a GM's summon (not while dead or entering); `cb(ok)` |
| `game.spawn(mob_id, count, map, x, z, opts)` | monsters that never respawn within `opts.radius` (0 to 50, default 10) of the spot, removed quietly after `opts.lifetime` seconds (required, 10 to 3,600); `opts.exp = false`, `opts.drops = false` give no experience or drops; returns their ids; 50 a call, 300 alive per extension |
| `game.despawn(ids)`, `game.alive(ids)` | the extension's own only |
| `game.log(text)`, `print` | a line in the server log |

Texts to players are printable ASCII (other bytes become `?`), and a text with a colour marker that would crash the clients raises a Lua
error. An order (give, take, buff, teleport) is carried out by that player's own connection, the way a GM's order is; the callback hears
the result later, on the script thread, never inside the call that ordered it. An order for a player who is not in a zone answers `false`
at once, and so do orders waiting when a player leaves. Rate limits per extension: 20 notices to many and banners a minute; per player 5
messages or notices a second and 50 orders a second. An overrun is a Lua error.

### Stored values

`p:get(key)`, `p:set(key, value)` (the character's, per extension) and `game.get(key)`, `game.set(key, value)` (the extension's own);
`nil` deletes. Keys are `^[a-z0-9_.:-]{1,64}$`; values are booleans, finite numbers and strings up to 1,000 bytes (tables are not
stored in API 1); at most 200 keys per character per extension and 10,000 for the extension's own. A character's values are loaded at
login (before `login` is queued) and stay until its `logout` handlers have run; `p:get` of a player whose values are not loaded gives
`nil` and `p:set` is a Lua error, so a bounty on someone offline lives in `game.set("bounty.<id>", …)`. A `set` changes the memory at
once; a writer sends the changes to PostgreSQL every 2 seconds and at shutdown. `ext disable` keeps the values; `ext remove` drops them.

An extension enabled by `reload` while players are online sees their values from their next login. Values are not shared between
game servers on one database: each loads its own copy, and a global written on two at once keeps the last write.

### The sandbox

Lua 5.1 with `math`, `string`, `table`, a reduced `os` (`time`, `date`, `difftime`, `clock`) and the base library without `dofile`,
`loadfile`, `load`, `loadstring`, `require`, `module`, `getfenv`, `setfenv`, `newproxy`, `coroutine` and `string.dump`;
`collectgarbage("count")` only; `io`, `package` and `debug` do not exist; only text chunks load. There is no file, process, network or SQL
access: the only way out of a state is `game`.

| | instructions | time |
|---|---|---|
| the load (all the files' top levels) | 5,000,000 | 500 ms |
| an event, a command, a timer, a callback | 1,000,000 | 20 ms |
| a point | 200,000 | 5 ms |

Past a limit the call ends with an error that names the extension and the handler; `pcall` cannot swallow it. The one thing the limit
cannot interrupt is a single call into a library function that takes long by itself, such as a pathological `string.find` pattern on a
long string; keep patterns simple. Memory is limited per state by `script_memory_mb`.

### Checking a script

`data-editor ext check <folder>` compiles every file and runs the load with an API that records the registrations and does nothing else; a
compile or load error names the file and line, and the summary lists the events, points, commands, item takeovers and timers the scripts
register. It cannot know what a handler does when it runs: test it on a drill server (`docs/plans/ext-1-foundation.md` section 9).
