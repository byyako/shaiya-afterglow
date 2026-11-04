# Rules

A **rule** is a named, scoped statement the server asks at a few fixed places (the *points*): "no attacks on this map in the evening",
"players more than 20 levels apart cannot fight", "no experience from player kills in the capitals", "an enchant may jump two steps". Rules
are an addition: with none written the server behaves as the original, apart from the one rule the `trade_chat_min_level` setting has
always made (section 8). `docs/plans/ext-2-rules.md` is the design; this page is how to use them.

## 1. Where rules come from

The same four layers as the settings (`docs/server/settings.md`), lowest first. A rule of a higher layer **with the same name replaces**
the lower one whole (its scope, type and keys; keys are never merged).

1. **The settings**: the three settings that became rules make one each, named `setting.<key>` (section 8).
2. **The enabled extensions**: `[rules.<name>]` in `extension.toml`, named `<extension>/<name>` (section 7).
3. **The config file**: `[rules.<name>]` in `config/game.toml`.
4. **The data editor**: the target `rule` (section 6).

`enabled = false` turns a rule off; a table that holds only `enabled = false` turns off the rule of that name in a lower layer
(`[rules."pets/no-capture"]` with `enabled = false`). A name no lower layer has needs a `type`. A rule name is lowercase letters, digits and
`-` (up to 40), or `<extension>/<name>`; names starting with `setting.` belong to the settings and are refused in the upper layers.

When several rules answer one question, the order is: the editor's, the file's, the extensions', the settings'; by name within a layer.
`game-server rules` lists them in that order (section 5).

## 2. Writing a rule

```toml
# No experience, kills or deaths from player kills in the capitals and the Auction House.
[rules.market-no-pvp-gain]
type = "pvp_gain"
maps = [35, 36, 42]
exp = 0
kills = 0
deaths = false

# Players whose levels differ by more than 20 cannot fight, except in the Auction House.
[rules.level-gap]
type = "level_gap"
gap = 20
maps = { not = [42] }

# Enchanting may jump: +1 seven times in ten, +2 twice, +3 once.
[rules.enchant-jumps]
type = "enchant"
jumps = [[1, 70], [2, 20], [3, 10]]

# A safe zone for PvP, weekday evenings.
[rules.safe-evenings]
type = "deny"
point = "attack"
maps = [2]
days = [1, 2, 3, 4, 5]
hours = "18-22"
message = "No fighting here in the evening."
```

Every rule has a `type`, optionally the scope keys of section 3, `enabled` (default `true`) and, for a refusal, a `message`. Any other key
is an error that names it. `config/game.toml.example` ends with these examples, commented out.

### The scope

Every scope key is optional; a missing key matches everything.

| key | value | matches when |
|---|---|---|
| `maps` | a list of map ids and ranges: `[35, 36, "40-42"]` | the map of the action is in the list |
| `level` | a range `"1-15"`, `"60-"`, `"-30"`, a number `30`, or a list of these | the player's level is in one of them |
| `kills` | as `level`, on the player's kill count (the number the client shows) | the player's kills are in one of them |
| `faction` | `"light"` or `"fury"` | the player's faction |
| `days` | a list of weekdays, 0 = Sunday to 6 | today, the server's local time |
| `hours` | a range `"18-22"` of hours 0 to 23 (18:00 to 22:59), a number, or a list | the hour now, the server's local time |
| `who` | `"either"`, `"both"`, `"actor"`, `"target"` | on a two-player point, which of the two players `level`, `kills` and `faction` are tested on |

The keys are ANDed (every key given must match); the entries of one key are ORed. `days` and `hours` together mean "on those days, at
those hours". `maps`, `level`, `kills`, `days` and `hours` may be written `{ not = <the same value> }`: `maps = { not = [42] }` is every
map but the Auction House. Map ids are 0 to 65535, levels 1 to 255, weekdays 0 to 6, hours 0 to 23.

On a two-player point (an attack, a trade) the player keys are tested on both players: `who = "either"` (the default for a refusal) means a
rule "no PvP at levels 1 to 15" refuses an attack in which *either* side is 1 to 15, protection both ways; `"both"` (the default for a
permission) needs both; `"actor"` and `"target"` test the attacker / requester or the one attacked / asked. `who` is an error on a rule of
a one-player point. `kills` is also a key of `pvp_gain` (below), where it is the multiplier: that rule cannot be scoped by the kill count.

## 3. The rule types

| `type` | point | own keys | what it does |
|---|---|---|---|
| `deny` | `point = "attack"`, `"trade"`, `"personal_shop"` or `"trade_chat"` (required) | | refuses the action in scope |
| `level_gap` | attack | `gap` (1 to 255, required) | refuses an attack between two players whose levels differ by more than `gap`, both ways |
| `cross_faction_trade` | trade | | players of both factions may trade in scope (the original allows it nowhere); `who` defaults to `"both"` |
| `pvp_gain` | experience of a player kill, kills, deaths | `exp` (0 to 100, default 1), `kills` (a whole number 0 to 100, default 1), `deaths` (`true`/`false`, default `true`); at least one | what a player kill gives in scope: the killer's experience × `exp`; the counted kill × `kills` (0: not counted at all, the player-kill quest and the faction's count too); `deaths = false`: the victim's death is not counted |
| `exp` | experience | `factor` (0 to 100, required), `sources` (`"mob"`, `"party"`, `"quest"`; default `["mob", "party"]`) | experience from those sources × `factor` in scope; a player kill's experience is `pvp_gain`'s, not this one's |
| `enchant` | enchant | `jumps` (a list of `[steps, weight]`, steps 1 to 19, weight 1 to 1,000,000, at most 19), `items` (`"weapons"`, `"armour"`, `"both"`) | on a successful enchant the step rises by a weighted draw of `steps` instead of 1, at most to step 20 |

Refusals are checked **after** the core's own checks: a rule never lets through what the core refuses. The one permission,
`cross_faction_trade`, takes the place of the faction test only, and a `deny trade` rule beats it. Duels and guild wars are agreed to and
are not ruled: two duel partners and the two sides of a guild war stay enemies whatever the rules say; the attack point asks only when the
two are enemies by faction alone.

When several rules apply: a refusal holds if any matching rule refuses (the message is the first's in the order of section 1); the
multiplying kinds (`exp.factor`, `pvp_gain.exp`, `pvp_gain.kills`) multiply; a death is uncounted if any matching rule says so; `enchant`
takes the first matching rule alone. The order of a number's changes is the original's formula, the console's switches (`exp`,
`killrate`), the item effects, then the rules, so a factor of 0 always wins and rules with a factor of 1 leave every number as it was.

### What the player sees

A refused request (an attack, a skill on a player, a lapis or item skill on a player, a trade request, a personal shop, trade chat) gets
a **whisper from `Server`**: the rule's `message`, or the point's default text, at most once per rule per player every 10 seconds;
`message = ""` makes a rule silent. The rest of the answer is the one the core gives for a refusal (nothing for an attack and a shop, the
"busy" answer for a trade). Inside a running action (later blows, area victims, thorns, fields, a death not counted) a protected player is
skipped silently. A message is printable ASCII, at most 100 bytes.

| refusal | default text |
|---|---|
| `deny` attack | `You cannot attack that player here.` |
| `level_gap` | `The level difference is too great to fight.` |
| `deny` trade | `Trading is not allowed here.` |
| `deny` personal_shop | `A personal shop cannot be opened here.` |
| `deny` trade_chat | `Trade chat is not allowed here.` |

The log, with `RUST_LOG=debug`, has a line for each refusal (`refused by a rule`) and each number a rule changed (`changed by rules`).

## 4. `reload`

The console's `reload` (`target/release/console <server id> reload`, or `.reload` in the game for staff) reads the monsters and drop
tables as before, and also the config file named at start, the rules of the enabled extensions and the data editor's rules, and puts them
in force **together, or not at all**: on any error the answer is the error and nothing changes. The answer counts the rules:

```
reloaded: 2747 monsters, 294 drop grades, 68 box sets, 4 rules (103 ms)
```

Settings do not reload: a setting whose value in the file differs from the one in force is named in the answer
(`; settings changed in the file take effect at the next start: market_maps`) and takes effect at the next start. The rules the three
settings make are the ones from the start. A reload gives every rule a new hit count, and a changed rule may whisper again at once.

## 5. Looking at the rules

- `target/release/game-server rules` prints the rules in force as the file would hold them, each under a comment naming its layer
  (`# config file`, `# extension pets`, `# data editor (change set 12)`, `# setting market_maps (config file)`), then the rules that were
  replaced or turned off, commented out, with what did it. It reads the same file and database as a start, starts nothing and exits 1
  with the error if the rules do not resolve (including a `{ enabled = false }` that turns off nothing).
- `target/release/game-server rules --check [file]` checks a file alone (default `GAME_CONFIG`, then `config/game.toml`), settings and
  rules, with no database: `config/game.toml: 4 rules, no errors`, or the first error, exit status 0 or 1. Run it before `reload`.
- `rules` in the console (`.rules` in the game) lists one line per rule in force, `<name> <type> <where from> <hits>`; hits count how
  often a rule matched since the start or the last reload.
- Errors name the place: `config/game.toml line 14: rules.level-gap.gap: 0 is not 1 to 255`,
  `config/game.toml line 9: rules.safe-evenings: unknown key "hour"`, `extension pets: rules.no-capture.maps: …`,
  `data editor (change set 12): rules.x: …`. A bad rule in the file stops the start.

## 6. The data editor

The target `rule` (key = the rule's name, one field `rule`, an object with the rule's keys), effect **live**:

```
target/release/data-editor draft add add rule level-gap '{"rule": {"type": "level_gap", "gap": 15}}'
target/release/data-editor draft add set rule level-gap rule '{"type": "level_gap", "gap": 25}'
target/release/data-editor draft add remove rule level-gap
target/release/data-editor apply-draft -m "level gap"          # then: console <server id> reload
```

The game server's own parser checks every rule; History, Undo, export and import work as for any edit. An editor rule beats the file's
and the extensions' of the same name. `{"rule": {"enabled": false}}` turns off a lower rule (`pets/no-capture`); the editor cannot know
whether a lower rule of that name exists, so `game-server rules` (and the start) report one that turns off nothing.

## 7. Rules in an extension

`extension.toml` may hold `[rules.<name>]` tables, named without the extension's prefix; `ext check` and `ext install` check each with the
game server's parser and store it; `ext upgrade` replaces them, `ext remove` drops them, `ext list` shows a count and `ext show` the
rules. The server names them `<extension>/<name>`. A disabled extension's rules do not count. After installing, enabling, disabling or
removing an extension with rules, `console <server id> reload` puts the change in force. An operator replaces or turns off an extension's
rule by writing `[rules."<extension>/<name>"]` in the file or the editor.

## 8. The settings that became rules

These keys keep working as before; each makes a rule at start, and a `[rules]` entry can do the same with more scope. They do not change at
a `reload` (settings take effect at the next start).

| setting | the rule it makes | when |
|---|---|---|
| `cross_faction_trade_maps = [42]` | `setting.cross_faction_trade_maps`: `cross_faction_trade` on those maps | the list is not empty |
| `market_maps = [35, 36, 42]` | `setting.market_maps`: `deny` `personal_shop` on every other map, silent | the list is not empty (empty: every map, as the original) |
| `trade_chat_min_level = 10` | `setting.trade_chat_min_level`: `deny` `trade_chat` for levels 1 to 9, silent | the value is 2 or more |

So a default install has one rule in force, `setting.trade_chat_min_level`. `exp_rate` and `kill_rate` stay settings (the console changes
them live), as do `kill_count`, `kill_boost_skills`, `trade_chat_cooldown_secs` and `guild_party_size`.

## 9. Not yet

The points are the eight above. Others (whisper, party and union invites, inspect, map entry and exit, equipping, skill use, item use,
damage and the stat formulas, item loss on death) are for later waves; Lua scripts will ask the same points (extensions chunk 5).
