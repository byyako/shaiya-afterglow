# Item effects and skill abilities

An **item effect** is a named behaviour, written in Rust inside the server, that an extension's rows give to an item: a stone that raises a
level, a Lapisia that always succeeds, a rune that redraws one number. A **skill ability** is the same for a skill level while it is
applied to a player: a kill that counts five times. An extension names the type and its parameters in `rows.toml`; the server does the
rest. Effects are an addition: a server without an extension that has them behaves as the original. `docs/plans/ext-3-effects.md` is the
design; this page is how to use them, and (section 6) how to add a new type.

**Released in 0.3.0.** Effects are in 0.3.0; 0.2.0 does not have them.

## 1. The reference extensions

Four folders of `extensions/` use them. None of the three new ones is installed by `tools/setup.sh`; each has a README.

| extension | items | what it does |
|---|---|---|
| `level-stones` | 100/215, 216, 217 | Level Stone (one level more), Level Stone (15) and (30) (up to that level) |
| `perfect-lapisia` | 95/3 (weapon), 95/8 (armour) | an enchant always succeeds and may rise 1, 2 or 3 steps (60, 30, 10 in a hundred) and never destroys the item |
| `stat-runes` | 100/221 to 226 | a Recreation Rune for each of STR, DEX, REC, INT, WIS and LUC: it redraws that number only |
| `kill-boost` (1.1.0) | 100/212 to 214 and skill 200/1 to 3 | the Kill Boost Stones: the buff counts a player kill 2, 5 or 10 times; same as 1.0.0, the multipliers are now abilities |

Every reference item is **grade 999**, the drop grade no monster drops, so they come only from the item mall, the GM's `/make <type>
<type id>`, or a script's `give_item`. To use them:

    target/release/data-editor ext install extensions/level-stones      # then: tools/game-server.sh restart
    target/release/data-editor ext disable level-stones                  # the items stay; they do what their own columns say

Effects belong to **enabled** extensions and are read when the game server starts, so an install, upgrade, enable or disable needs a
restart. A disabled extension's items are not removed: each does what its own `defs` columns say, which for the reference items is the
core's refusal (a stone special 0 with no HP, SP or MP, `use_failed`, the item kept), and for the Lapisia and runes an ordinary Weapon or
Armor Lapisia and Recreation Rune.

A player sees the item's name and text from the client's own data (`Item.SData`), which comes with a client data patch
(`tools/sdata/client_patch.py`); until a client has the patch the new items show as `???`. The effects themselves are server-only: they
are not in `defs` and not in the client's rows, so `content hashes defs` and the earlier patches do not change.

**Kill Boost Stones from 1.0.0.** The 1.0.0 extension set the setting `kill_boost_skills`; 1.1.0 gives the skill rows `kill_boost`
abilities instead. Moving is optional (`data-editor ext upgrade extensions/kill-boost`, then a restart); the setting keeps working as a
shorthand that gives the same abilities at start. Nothing the players see changes.

## 2. Keeping an item off some maps: `deny item_use`

The rules' `item_use` point refuses an item in a scope, before the reuse group and the scripts (`docs/server/rules.md`):

```toml
[rules.no-level-stones-in-pvp]
type = "deny"
point = "item_use"
items = ["100/215-217"]
maps = [2, 3, 4]
message = "Level stones do not work here."
```

Then `target/release/game-server rules --check`, and `target/release/console <server id> reload`. The player is whispered the message and
the item stays, its reuse time untouched.

## 3. How an item's use is decided

For a use request (`0x050A`, `0x0557`, `0x0559`) the server goes down this list and stops at the first answer:

1. the core's checks: alive, the item exists, level, race and mode, the skill-cut guard for moves (an item with an effect is not refused
   for having no core effect);
2. the rules' `item_use` point;
3. the reuse group (a refusal after this puts the group's time back);
4. a script that took the item over with `game.on_item` (`"use"`, `"keep"`, `false` end here; nothing goes on);
5. the extension's effect, if the item has one and its extension is enabled;
6. otherwise the core's own effect.

A Lapisia's effect acts in the enchant (`0x0805`) and a rune's in the recreation (`0x0806`), so at `0x050A` such an item keeps the
core's answer (refused). In a use request of another kind than oneself (`0x0557`, `0x0559` on a player or a monster) an item with a use
effect fails, as the experience wheels do.

## 4. For extension authors: the keys

An `[[item]]` entry of `rows.toml` may have an `effect`, a `[[skill]]` entry `abilities`. Both are checked by the game server's own parser
at `ext check`, `install` and `upgrade` (errors name the entry and the key: `rows.toml [[item]] #1 (100/215): effect: levels: 0 is not 1
to 59`), and again when the server starts. `ext check` prints each, and warns when an item with an effect would do something else while
its extension is disabled (`when the extension is disabled, 100/230 is a potion`). An entry that gives an **original** row an effect or
abilities needs `client` or `server_only` even when no column changes: the client's text would be wrong without it.

```toml
[[item]]
id = "100/215"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "drop", "server", "count"] }
set = { itemname = "Level Stone", grade = 999 }
effect = { type = "level_up", levels = 1 }
client = { name = "Level Stone", look = "100/108", description = "Raises your level by one." }

[[skill]]
id = "200/1"
copy = { from = "250/1" }
set = { skillname = "Kill Boost", keeptime = 1, fixrange = 1, abilitytype1 = 0, abilityvalue1 = 0, abilitytype2 = 0, abilityvalue2 = 0, abilitytype3 = 0, abilityvalue3 = 0 }
abilities = [{ type = "kill_boost", times = 2 }]
client = { name = "Kill Boost", look = "250/1", description = "Player kills count double for 1 hour." }
```

An item has at most one effect; a skill level at most four abilities (levels 1 to 3 of an extension's own skills). An extension may give
an effect or abilities only to the ids it owns.

### Item effects

| `type` | parameters | the row must be | what it does |
|---|---|---|---|
| `level_up` | exactly one of `levels` (1 to 59) and `to` (2 to 60); optionally `max` (2 to 60) | an item the core uses (type 25, 44 or 100) | raises the user's level |
| `enchant` | `rate` (1 to 1,000,000, default 1,000,000), `jumps` (`[[steps, weight], …]`, steps 1 to 19, weights 1 to 1,000,000, at most 19), `safe` (`true`/`false`, default `false`); at least one | a Lapisia (item type 95) | the enchant's success rate, its jump and its danger |
| `recreate` | `stats` (1 to 9 of `"str"`, `"dex"`, `"rec"`, `"int"`, `"wis"`, `"luc"`, `"hp"`, `"mp"`, `"sp"`, no repeat), `value` (`"random"` or `"max"`, default `"random"`) | a Recreation Rune (`special` 62) | redraws the named numbers only |

**`level_up`.** The target is the user's level plus `levels`, or `to`, cut at `max` and at the mode's cap (30 in easy mode, 60
otherwise). A target that is not above the user's level refuses the use (`use_failed`, a whisper `This item cannot raise your level any
further.`, the item kept). Otherwise the item is used up and saved, the experience becomes the start of the target level (so the stat and
skill points of each level come as from levelling, the class stat too), a `0x0308` game-log record is written for each level, and the
client gets `0x0207` and the level-up packets. Progress inside the old level is not carried over, as with the GM's `/set level`. A level
is not experience: the rules' and the scripts' `exp` points and the item flags' factor are not asked.

**`enchant`.** `rate` replaces the step's success rate (the material's count is still added to the draw); a rate of 1,000,000 always
succeeds. On a success `jumps` draws the Lapisia's own jump in place of the rules' `enchant` rule (the scripts' `enchant` point is asked
after either); the step stays at most 20. `safe = true` means a failure never destroys the item, whatever the Lapisia's VG column says;
`false` leaves the core's rule. The price, the material, the checks and the answer are the core's.

**`recreate`.** Only the named numbers the item already has (not 0 in its craft name) change; every other number and the enchant step
stay. `"random"` draws each as the smaller of two draws from 0 to the item's top, as the core's rune does; `"max"` makes it the item's top.
An item with none of the named numbers is refused as the core's rune refuses one (the window's failure, result 1), and the rune stays.

### Skill abilities

| `type` | parameters | what it does |
|---|---|---|
| `kill_boost` | `times` (1 to 100) | a counted player kill counts `times` times while the skill level is applied to the killer, by an item, a cast or a script; with several, the highest |

The abilities live on the skill definitions the server loads (`SkillDef.ext`), so an applied skill carries them. The setting
`kill_boost_skills` is a shorthand: at start each entry gives the matching skills a `kill_boost` ability, and a level that has both keeps
the higher.

## 5. Scripts

`game.on_item` is unchanged and is step 4 above: returning nothing runs the item's effect. The `item` table an `item_use` handler gets
has the field `effect`, the type name of the item's effect in force (`"level_up"`), or `nil`. `docs/server/scripts.md`.

## 6. Adding a type

A new effect or ability is, in the server (`crates/game-server/src/effects.rs`): a variant of `ItemEffect` (or `Ability`) with its
parameters, an arm in `check_effect` (or `check_abilities`) and in `fits_row`, a handler at the request's place (`server/effects.rs` for a
use, `lapis.rs` for enchant and recreation, or the place that reads the number for an ability), unit tests of the parser and the handler, a
`game_flow` test, and a section here. The data editor needs no change: it validates through the server's parser. An extension then names the
type in its rows. A newer data editor that writes a type an older server does not know makes that server refuse to start, naming the
extension and the item.
