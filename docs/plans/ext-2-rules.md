# Extensions chunk 2, rules and hooks (spec and build plan)

**Specified 2026-11-04; built 2026-11-04** (steps 1 to 9, one commit each, rehearsed on the drill, then on the live dev box). Chunk 2 of `docs/plans/extensions.md` (section 6, work item 7.2). Chunk 1 is built
(`docs/plans/ext-1-foundation.md`): the settings registry and its four layers, extensions with their manifest, the data editor's
`setting` target. This chunk adds **rules**: named, scoped allow/deny/change statements that the core asks at a few fixed places in the
code (the **points**), set from the same layers as the settings (built-in, enabled extensions, `config/game.toml`, the data editor),
and put in force **live** by the console's `reload`. It builds the reference set the plan names (no experience or kills in a map,
level-gap protection, enchant jumps) and folds the three scoped one-off settings in as rules without changing what they do.

**What the build settled where the text left a choice** (none changes a decision of section 1):
- `Config.config_path` is `Option<String>`: `GAME_CONFIG` as named, `None` for the default file, so `reload` reads with the rule `start` uses
  (a missing default file is fine, a missing named one an error). `Config::load` and `Config::inspect` return a third value, the rules'
  `RuleSources`; `config::read_rule_layers_in(conn, content, edits)` is the query behind it and `Backend::rule_layers` (the data editor's
  tests pass their own schema names).
- `rules::resolve(&Config, &RuleSources)` takes the layer each folded setting came from in `RuleSources.setting_layers` (for the comment
  `# setting market_maps (config file)`); `from_settings` returns `(setting, rule)` pairs; `RuleSet::from_values` builds a set from named
  rules for tests and tools.
- `check_rule` accepts `{ "enabled": false }` alone (it turns off a lower rule); `check_whole_rule` is for an extension's own rule and
  refuses it. A manifest names its rules without the prefix, and a `/` there is refused.
- On `pvp_gain` the key `kills` is the multiplier (section 4.1), so that rule cannot be scoped by the kill count; on every other type
  `kills` is the scope key.
- A trade request asks the core's checks first (the faction test with the permission in its place, busy, staking), then `deny trade`:
  a `deny trade` rule whispers only when the core would have let the request through.
- A modifier counts a hit at a point only where it changes the number (`pvp_gain`: `exp` not 1, `kills` not 1, `deaths = false`), so
  one player kill is up to three hits; a refusal counts one hit, the rule that refuses.
- The rules read the faction of the receiver of experience as the character's family over two (creation checks it), and of the others
  from the connection's faction.
- The console's `rules` shows the layer in a word without spaces (`file`, `editor:12`, `extension:pets`, `setting:market_maps`); the
  longer text is `game-server rules`'s.
- `config --example` ends with the rules heading and examples, and its header gains the line `# the rules: game-server rules` (`config`
  itself prints no header).
- `reload` names a setting when a value in the *file* differs from the one in force, as the text says: a setting the editor overrides is
  named whenever the file differs from it, though the file's value would not count at a start either.
- The weights of `enchant.jumps` are proportional (the draw below 1,000,000 is scaled to their sum); the jump is drawn after a success,
  with a draw of its own, and `Enchant.new_name` caps the step at 20.
- The folded settings keep their debug log lines (the rule's name starts with `setting.`); other rules log `refused by a rule`.


Background read for this spec: `docs/plans/extensions.md` sections 2, 2a, 3, 5, 6, 7.2; `docs/plans/ext-1-foundation.md` sections 5, 7
and 8; `crates/game-server/src/config.rs`, `switches.rs`; `crates/data-editor/src/registry.rs`, `settings.rs`, `apply.rs`;
`docs/original/operator-console.md`; `docs/plans/done/data-editing.md`; the code of the points (section 5, each with its place).

## 1. The maintainer's decisions (2026-11-04)

1. **A refused player is told by a whisper from `Server`** (`0x1102`, as the console's in-game answers): the rule's own `message`, or
   the point's default text; at most one whisper per rule per player every 10 seconds; `message = ""` makes a rule silent. The rules
   made from the old settings are silent, as those settings are today.
2. **Level-gap protection works both ways**: when two players' levels differ by more than the gap, neither may attack the other
   (normal blows, skills, area effects, thorns, fields).
3. **No Rust extension trait in this chunk.** The points are built with the rules as their only consumer, shaped so that chunk 5 plugs
   Lua into the same points and chunk 3 adds Rust item-effect handlers. Extension options (the file's `[ext.<name>]`, the manifest's
   `options`, the editor's `ext.<name>.<key>`) stay reserved until chunk 5, their first consumer.
4. **Three settings fold in as rules**: `cross_faction_trade_maps`, `market_maps`, `trade_chat_min_level`. Their keys keep working as
   shorthands that make a rule. The others stay plain settings: `exp_rate` and `kill_rate` (global, changed live by the console),
   `kill_count` (global on/off), `kill_boost_skills` (chunk 3 moves it onto the effect registry), `trade_chat_cooldown_secs`,
   `guild_party_size`.

Decided here (not behaviour of the original or of 0.2.0, so not asked):
- **Duels and guild wars are not ruled.** The attack point asks the rules only when the two are enemies by faction (`CUser::IsTeam`'s
  country test); a duel partner while fighting and a guild war's other side stay enemies whatever the rules say (both are agreed to).
- **The rules live in `config/game.toml` under `[rules]`** (the table chunk 1 reserved), not in a file of their own.
- **One rule replaces another of the same name whole**, from the layer above; there is no merging of keys.
- **The rules are read through `serde_json::Value`** (the file's TOML tables converted to it, the database's `jsonb` read as text), so
  one parser serves every layer. `serde_json` becomes a dependency of `game-server` (already in the workspace's lock through
  `data-editor`).
- **The rules a running server holds live in `Zones`** (`Zones::rules() -> Arc<RuleSet>`, swapped by `reload`), because the zone's own
  passes (casts, thorns, fields) need them and `Zones` is reachable from every place that asks.
- **Time windows use the server's local time**, as the guild ranking battle's open times do (`server.rs`, `chrono::Local`).

## 2. What changes

| Today (chunk 1 built) | After chunk 2 |
|---|---|
| `[rules]` in `config/game.toml` refused as reserved | the rules of the file (section 4.2) |
| manifest key `rules` refused as reserved | an extension's rules, `[rules.<name>]` in `extension.toml`, stored in `content.ext_rules` at install (4.4) |
| `[ext.<name>]`, manifest `options`, editor key `ext.<name>.<key>` reserved for chunk 5 | unchanged (decision 3) |
| `cross_faction_trade_maps`, `market_maps`, `trade_chat_min_level` read at their call sites | the same keys, each making a rule at start (section 7); the call sites ask the rules |
| `reload` re-reads monsters and drop tables | also re-reads the rules from the file and the database, all or nothing (4.6) |
| the data editor's targets | a new target `rule` (4.5), effect live |
| `game-server config` | also `game-server rules` and `game-server rules --check [file]` (4.7) |
| console commands | also `rules`: the rules in force with how often each matched (4.7) |
| no veto or modifier points | eight points (section 5) |

## 3. The scope model

A rule's **scope** says where and for whom it holds. Every key is optional; a missing key matches everything.

| key | value | matches when |
|---|---|---|
| `maps` | a list of map ids and ranges: `[35, 36, "40-42"]` | the map of the action is in the list |
| `level` | a range `"1-15"`, `"60-"`, `"-30"`, a number `30`, or a list of these | the player's level is in one of them |
| `kills` | as `level`, on the player's kill count (`kills[0]`, the number the client shows) | the player's kills are in one of them |
| `faction` | `"light"` (country 0) or `"fury"` (1) | the player's faction |
| `days` | a list of weekdays, 0 = Sunday to 6 (as `ranking_times`) | today, the server's local time |
| `hours` | a range `"18-22"` of hours 0 to 23, inclusive (18:00 to 22:59), a number, or a list | the hour now, the server's local time |
| `who` | `"either"`, `"both"`, `"actor"`, `"target"` (two-player points only, section 5) | which of the two players `level`, `kills` and `faction` are tested on |

**Combining**: the keys are ANDed (every key given must match); the entries of one key are ORed (any listed map, any listed range).
`days` and `hours` together mean "on those days, at those hours".

**Everything except**: any of `maps`, `level`, `kills`, `days`, `hours` may be written `{ not = <the same value> }`: it matches when the
plain form would not. `maps = { not = [42] }` is every map but the Auction House; `level = { not = "1-15" }` is level 16 and up.

**Two-player points** (attack, trade): `who` says which player the player keys test. The default is `"either"` for a refusal (a rule
"no PvP at levels 1 to 15" refuses an attack in which either side is 1 to 15: decision 2's spirit, protection both ways) and `"both"`
for a permission (cross-faction trade at levels 60 and up needs both traders at 60 and up). On a one-player point `who` is refused as
an unknown key.

Validation (each error names the place, section 4.2): map ids 0 to 65535, levels 1 to 255 (a level of 0 is refused: the client has none),
kills 0 to 4294967295, a range's low end at most its high end, weekdays 0 to 6, hours 0 to 23, `faction` one of the two words, `not`
the only key of its table and not nested.

## 4. Rules

### 4.1 The rule types of this chunk

Every rule has `type`, the scope keys of section 3, and optionally `enabled` (default `true`) and `message` (refusals only; printable
ASCII, at most 100 bytes; `""` silent).

| `type` | points (section 5) | own keys | what it does |
|---|---|---|---|
| `deny` | `point = "attack"`, `"trade"`, `"personal_shop"` or `"trade_chat"` (required) | | refuses the action in scope |
| `level_gap` | attack | `gap` (required, 1 to 255) | refuses an attack between two players whose levels differ by more than `gap`, both ways (decision 2) |
| `cross_faction_trade` | trade | | players of both factions may trade in scope (the original allows it nowhere); `who` defaults to `"both"` |
| `pvp_gain` | exp (source pvp), pvp kills, pvp deaths | `exp` (a number 0 to 100, default 1), `kills` (a whole number 0 to 100, default 1), `deaths` (`true` or `false`, default `true`); at least one given | what a player kill gives in scope: the killer's experience × `exp`; the counted kill × `kills` (0: not counted at all); `deaths = false`: the victim's death is not counted |
| `exp` | exp | `factor` (required, 0 to 100), `sources` (a list of `"mob"`, `"party"`, `"quest"`; default `["mob", "party"]`) | experience from those sources × `factor` in scope; the PvP kill's experience is `pvp_gain`'s, not this one's |
| `enchant` | enchant | `jumps` (required: a list of `[steps, weight]`, steps 1 to 19, weight 1 to 1000000, at most 19 entries), `items` (`"weapons"`, `"armour"` or `"both"`, default both) | on a successful enchant the step rises by a weighted draw of `steps` instead of 1, at most to step 20 |

Default refusal texts (the whisper of decision 1 when a rule gives no `message`):

| point | text |
|---|---|
| attack (`deny`) | `You cannot attack that player here.` |
| attack (`level_gap`) | `The level difference is too great to fight.` |
| trade | `Trading is not allowed here.` |
| personal_shop | `A personal shop cannot be opened here.` |
| trade_chat | `Trade chat is not allowed here.` |

Any other key is an error naming it; so is a key of another type (`gap` on a `deny`).

### 4.2 The file

The table `[rules]` of `config/game.toml` holds one sub-table per rule, keyed by the rule's name:

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

**Names**: `^[a-z0-9][a-z0-9-]{0,39}$` for a rule of the file or the editor; a name `<extension>/<name>` (quoted in TOML:
`[rules."pets/no-capture"]`) names an extension's rule to replace or turn off (section 4.3); names starting with `setting.` belong to
the settings (section 7) and are refused ("change the setting `market_maps` instead").

**Errors** stop the start (and fail a `reload`, section 4.6) and name the file, the line and the key, as the settings do:
`config/game.toml line 14: rules.level-gap.gap: 0 is not 1 to 255`, `line 9: rules.safe-evenings: unknown key "hour"`,
`line 3: rules.x: a rule needs a type`. The line of a key is found from the `toml` crate's spans (`config::read_file` already reads
them for the settings; the rules' keys are one table deeper).

`read_file` keeps rejecting `[ext.<name>]` as reserved for chunk 5, and any other table as an unknown setting.

### 4.3 The layers and precedence

Lowest first, as the settings (`docs/plans/ext-1-foundation.md` 7.1):

1. **Settings**: the rules the three folded settings make (section 7), named `setting.<key>`, after the settings are resolved.
2. **Enabled extensions**: `content.ext_rules` of the enabled extensions, named `<extension>/<name>`, in `install_seq` order.
3. **The file**: `[rules.<name>]`.
4. **The data editor**: `content.rule_values`, written only through the `edits` overlay (target `rule`, 4.5).

A rule of a higher layer **with the same name replaces** the lower one whole (its scope, type and keys). `enabled = false` turns it off;
a table holding only `enabled = false` is enough to turn off a rule of a lower layer (`[rules."pets/no-capture"] enabled = false`). A name
that no lower layer has needs a `type`. Layers 3 and 4 may name extension rules; layer 2 names only its own (two extensions never
clash: names are prefixed). A `setting.` name is refused in layers 2 to 4.

**The order of the rules in force**: layer 4 first, then 3, 2, 1, by name within a layer. Where a point takes one answer (the message of
a refusal, the enchant jump) the first matching rule in this order gives it (section 5.2).

### 4.4 Extensions

The manifest key `rules` opens (`crates/data-editor/src/ext/manifest.rs` `RESERVED` loses `"rules"`): `[rules.<name>]` tables of the same
shape as the file's, the name without the prefix. `ext check` and `ext install` validate each with the game server's own parser
(`game_server::rules::check_rule(name, &serde_json::Value)`, as `check_setting` for settings) and store it in
`content.ext_rules (extension, name, rule jsonb)` (migration 0032). `ext upgrade` replaces them; `ext remove` drops them (`ON DELETE
CASCADE`); `ext list` gains a rules count, `ext show` lists them. A disabled extension's rules do not count (chunk 1 decision 4).
`ext enable` / `disable` / `install` / `upgrade` / `remove` print "`console <id> reload` puts its rules in force" when the extension has
rules, beside the existing "restart" lines for rows and settings.

### 4.5 The data editor's target `rule`

Beside `setting` (`crates/data-editor/src/settings.rs`), the same pattern:

- registry entry: name `rule`, side `Content`, table `rule_values`, key `name`, one field `rule` (`Kind::Rule`: a JSON object, the rule's
  keys as in the file), add and remove allowed, effect **live** (`reload`);
- key: a rule name of section 4.2 (`<extension>/<name>` allowed, `setting.` refused); validated by `game_server::rules::check_rule`;
  an object holding only `{"enabled": false}` is valid for a name some lower layer has (checked at resolve; the editor cannot know the
  file, so it accepts it and `game-server rules` reports an unmatched one as an error, as the server's start does);
- `crates/data-editor/src/rules.rs`: `key`, `apply_line`, `apply_edits`, `revert`, mirroring `settings.rs` (the key is a name, so it stays
  out of the numeric key code); `registry.rs` `key_text` / `parse_key` treat `rule` as they treat `setting`; `apply.rs` `apply_all`
  calls `crate::rules::apply_edits` and `revert_all` leaves out `target <> 'rule'` as it does `setting` and calls `crate::rules::revert`;
- History, Undo, export and import work as for every target (the export carries the `rule` lines like any other).

Command line (as the settings): `data-editor draft add add rule level-gap '{"rule": {"type": "level_gap", "gap": 15}}'`,
`data-editor draft add set rule level-gap rule '{"type": "level_gap", "gap": 25}'`, `data-editor draft add remove rule level-gap`,
then `apply-draft -m …`; the effect line says `live: console <id> reload`.

### 4.6 `reload`

The console's `reload` (`crates/game-server/src/server/console.rs` `reload`) reads, before it swaps anything: the monsters and drop
tables (as now), the file named at start (`Config.config_path`, new: the path `read_config_file` used), the rules of the enabled
extensions and the editor's rules (`Backend::rule_layers`, new). It resolves the rules with the settings resolved at start (the folded
settings' rules do not change at a reload: settings take effect at restart, chunk 1 decision 7). Only when everything succeeded it swaps
`GameData::set_live` and `Zones::set_rules`; on any error nothing changes and the answer is the error, as now. The answer gains the
count: `reloaded: <n> monsters, <n> drop grades, <n> box sets, <n> rules (<ms> ms)`.

A setting whose value in the file differs from the value in force is not taken, and is named: the server log gets one warning line, and
the answer gains `; settings changed in the file take effect at the next start: market_maps, guild_party_size`.

A whisper's rate limit (decision 1) is per rule id; a reload gives every rule a new id, so a changed rule may whisper again at once.

### 4.7 Seeing the rules

- `game-server rules`: no world, no listening; reads the layers as the start does (one connection, no migrations, a database
  without the tables has no database layers, as `game-server config`) and prints the rules in force as the file would hold them, each
  with a comment naming its layer (`# config file`, `# extension pets`, `# data editor (change set 12)`, `# setting market_maps (config
  file)`), then the rules replaced or turned off, commented out, each with what replaced it. Exit 1 and the error on any error.
- `game-server rules --check [file]`: the file alone (default `GAME_CONFIG` or `config/game.toml`), settings and rules, no database:
  `config/game.toml: 4 rules, no errors` or the first error; exit status 0 or 1. Meant for an operator before `reload`.
- Console `rules` (in game `.rules`, help entry): one line per rule in force, `<name> <type> <layer> <hits>`, `hits` being how often it
  matched since the start or the last reload; `no rules` when there are none. The whisper lines stay under 120 bytes as the console's
  other answers.
- `game-server config` stays the settings; its header comment gains one line, `# the rules: game-server rules`.

## 5. The points

### 5.1 The points of this chunk and their places

Read 2026-11-04 at `main` 7a9e1a8; the builder re-reads each place before changing it. The core's own checks come first at every
point: a rule never lets through what the core refuses, except the one permission (`cross_faction_trade`), which takes the place of the
faction test only.

| # | point | kind | place | what changes |
|---|---|---|---|---|
| 1 | **attack** (player on player) | refusal, two players | `zone/pvp.rs` `attackable` (callers: `Zones::may_attack_player`, `attack_player`, `zone.rs` `Cast::enemy`, which covers single targets and the area victims of `Cast::area` through `players_around`); `zone.rs` `thorns` (the `area_victim && enemies` filter); `zone.rs` `fields` (the `area_victim && country` filter) | asked only when the two are enemies by faction (decided here, section 1): `enemies` tells why (duel, war, faction); the rules' `deny attack` and `level_gap` refuse. Fields keep the caster's level, kills and country at creation (`zone.rs` `Field` gains `owner: Who`, filled in `Cast::field`) so the test holds after the caster left |
| 2 | **trade** | refusal and permission, two players | `zone/trade.rs` `Zones::trade_request` (called from `server.rs` `tp::OP_REQUEST`, which today passes `any_faction` from `cross_faction_trade_maps`) | the zone, which holds both players, asks `cross_faction_trade` in place of `any_faction` (the parameter goes) and the `deny trade` rules; a refusal answers `tp::START_BUSY` as the faction test does today, and returns the refusal for the whisper |
| 3 | **personal_shop** | refusal | `server/my_shop.rs` `mp::OP_BEGIN` (the `market_maps` test) | the `deny personal_shop` rules in place of the test |
| 4 | **trade_chat** | refusal | `server.rs` `chat::OP_TRADE` (the `trade_chat_min_level` test) | the `deny trade_chat` rules in place of the level test; the cooldown stays as it is |
| 5 | **exp** | modifier | `server.rs` `award_exp` (callers: `ZoneEvent::Kill` → source mob, `ZoneEvent::PartyKill` → party, `ZoneEvent::PlayerKill` → pvp, the quest reward (`plain`) → quest) | `award_exp` gains `source: ExpSource`; after the item factor (or at once for `plain`), the gain and the bonus × the product of the matching rules' factors (`exp` rules for mob, party, quest; `pvp_gain.exp` for pvp), truncated; a product of exactly 1 changes nothing |
| 6 | **pvp kills** | modifier | `server/pvp.rs` `kill_counted` | first thing: `times` = the product of the matching `pvp_gain.kills`; 0 returns at once (no player-kill quest count, no kill, no faction count); otherwise `by = kill_rate × kill_boost × times` (the faction count and the quest stay 1) |
| 7 | **pvp deaths** | refusal (silent) | `server.rs` `ZoneEvent::Died` (`if counted { death_counted(…) }`) | counted only when no matching `pvp_gain` says `deaths = false` (tested on the victim) |
| 8 | **enchant** | modifier | `server/lapis.rs` `enchant` after the roll; `lapis.rs` `Enchant::new_name` | on success the first matching `enchant` rule draws the jump (`draw` below 1,000,000 against the weights); `Enchant` gains `jump: u8` (1 without a rule), `new_name` writes `min(step + jump, 20)`; the `enchant` info log line gains `jump`. Failure is unchanged |

Points 1 to 4 and 7 are refusals; 5, 6 and 8 change a number. **Who** a rule tests: the attacker and the target (1), the requester and
the one asked (2), the player (3, 4, 8), the receiver of the experience (5: each party member for itself), the killer (6), the victim
(7). The map is the map where the action happens (both players share it at points 1, 2, 6 and 7).

### 5.2 How several rules combine, and the order

- **Refusals**: refused if any matching rule refuses. The whisper names the first in the order of 4.3.
- **The permission** (`cross_faction_trade`): granted if any matching rule grants it and no `deny trade` matches.
- **Multiplying modifiers** (`exp.factor`, `pvp_gain.exp`, `pvp_gain.kills`): the product of every matching rule; `deaths`: not counted if
  any matching rule says `false`.
- **`enchant`**: the first matching rule alone (a product of jump tables means nothing).
- **The order of a number's changes**: the original's formula → the console's switches (the experience event inside `combat::mob_exp`,
  `kill_rate`) → item effects (`state.effects.exp_factor()`, the kill boost) → the rules. So a rule's factor 0 always wins, and rules with
  factor 1 leave every number as before, byte for byte.

### 5.3 What the player sees, and the log

- A refusal at a request the player made (point 1 at `server.rs` `cp::OP_ATTACK_USER`, the skill target `Aim::Named if harms`, the item
  skill in `server/items.rs` `it::KIND_PLAYER`; points 2, 3, 4) sends the whisper of decision 1 from `Server`
  (`chat::whisper(false, SERVER_NAME, text)`; `SERVER_NAME` moves from `server/admin.rs` to a shared `pub(crate)` constant), unless the
  rule is silent or the player was told by that rule in the last 10 seconds (`WorldState.told`, a short list of `(rule id, Instant)`, at
  most 8 entries, the oldest dropped). The rest of the answer is the one the core gives today for a refusal (none for an attack and a
  shop, `START_BUSY` for a trade, nothing for trade chat).
- A refusal inside a running action (an attack's later blows, area victims, thorns, fields, a death not counted) is silent: the
  victim is skipped, as an ally is.
- **Log**: a refusal at a request: `tracing::debug!(id, map, point, rule = name, "refused by a rule")` (the existing debug lines of the
  three folded settings stay as they are, so their output does not change); a modifier that changed a number:
  `tracing::debug!(id, map, point, rules = names, before, after, "changed by rules")`. Debug, because points 1 and 5 are on hot paths.
- **Hits**: each rule in force counts its matches (`AtomicU64`, relaxed), shown by the console's `rules`.

The zone returns what the connection needs to tell the player: `Zones::attack_verdict(map, me, target, war_map) -> Verdict`
(`Verdict::Yes`, `No`, `Ruled(Refusal)`; `may_attack_player` stays as `attack_verdict(…).is_yes()` so its tests and callers that only ask
keep working), `trade_request` returns `Option<Refusal>`. `Refusal { rule: u32 (the id), name: Arc<str>, text: Option<Arc<str>> }`.

### 5.4 Left for the waves

Not built now; each is a later point at the place named here (re-read when its wave is specified, `docs/plans/extensions.md` 7.7):
whisper (`server.rs` `chat::OP_WHISPER`; cross-faction is chunk 6), party and union invites (`Zones::party_invite`), inspect
(`ZoneEvent::Inspect` and its request), map entry and exit (`move_player`, portals, gatekeepers, `town_secs`, `logout_secs`), equip
(the bag-0 moves in `inventory.rs`), skill use (`server.rs` skill start), chat in general (`said` and the other `chat::OP_*`), warehouse
and guild bank stores, item use (`item_use.rs`, chunk 3's registry), damage, crit and the stat formulas (`combat.rs`, `stats.rs`), item
loss on death (`server/pvp.rs` `lose_to`), PvP rank rewards (`pvp::claim`'s level 16).

## 6. The reference rules

1. **No experience or kills in a map** (the market bug): `pvp_gain` with `maps`, `exp = 0`, `kills = 0`, `deaths = false` (the first
   example of 4.2). Points 5, 6, 7. A kill there still drops items and gold as today (item loss is a later point).
2. **Level-gap protection**: `level_gap` with `gap` (the second example). Point 1, both ways (decision 2), with the whisper "The level
   difference is too great to fight." at the request.
3. **Enchant jumps**: `enchant` with `jumps` (the third example). Point 8. The client shows the new step from the craft name it gets in
   `0x0805` and `0x0507` (`docs/original/lapis.md` section 5); a jump of more than one step is unverified in the client (live test).

## 7. The settings folded in

At start, after the settings are resolved (`config::resolve`), `rules::from_settings(&Config)` makes layer 1:

| setting | rule made | when |
|---|---|---|
| `cross_faction_trade_maps = [42]` | `setting.cross_faction_trade_maps`: `cross_faction_trade`, `maps = [42]` | the list is not empty |
| `market_maps = [35, 36, 42]` | `setting.market_maps`: `deny`, `point = "personal_shop"`, `maps = { not = [35, 36, 42] }`, `message = ""` | the list is not empty (empty: every map, as the original) |
| `trade_chat_min_level = 10` | `setting.trade_chat_min_level`: `deny`, `point = "trade_chat"`, `level = "1-9"`, `message = ""` | the value is 2 or more (0 and 1 refuse nobody) |

So the default install has one rule in force (`setting.trade_chat_min_level`), which `game-server rules` lists with its layer `setting
trade_chat_min_level (default)`. The `Config` fields stay (the tests build `Config` directly, and `game-server config` shows them); the
three call sites stop reading them. The keys keep their documentation in `docs/server/settings.md`, each with "makes the rule …; a
`[rules]` entry can do the same with more scope". **Proof that behaviour is unchanged**: the existing tests that set them pass without
change (`game_flow` `trade_chat_has_a_level_and_a_cooldown`, `two_players_trade_items_and_money`, `a_personal_shop_sells_part_of_a_stack`
and the second `market_maps` test; the `zone/tests.rs` trade tests change only in passing the rules instead of `any_faction`), plus one test per setting comparing the old test with the rule at the
edges (level 9 and 10, map in and out of the list, both factions on 42 and elsewhere).

## 8. Cost

The attack point runs on every normal blow and for every player an area effect or a field could reach; the experience point on every
monster kill. They stay cheap:

1. **Compiled once.** `RuleSet` (built at start and at `reload`) holds one `Vec` of compiled rules **per point**; a compiled scope is
   plain data (a sorted map list and a `not` flag, a few `(low, high)` ranges, an `Option<u8>` faction, a 168-bit hour-of-week mask). No
   strings, maps or JSON are touched when a point is asked.
2. **Nothing to ask, nothing done.** Each point begins `if self.<point>.is_empty() { return <allow / 1> }`. Without rules for a point the
   cost is that branch and the `Arc` snapshot.
3. **One snapshot per operation.** The caller takes `Zones::rules()` (an `Arc` clone under a short `RwLock` read, as `GameData::live()`)
   once per request, blow, cast or field pass, never per victim.
4. **No clock on the hot path.** The hour of the week (0 to 167, local time) is a global `AtomicU16` that `main` refreshes every 30
   seconds (`rules::set_hour_of_week`); tests set it themselves. A time-scoped rule tests one bit.
5. **Map first.** A rule's map test comes first; a rule for other maps costs one binary search. With the handful of rules an operator
   writes, the scan of a point's `Vec` is shorter than the zone's own range test.
6. **No allocation, no database, no lock held** while a rule is asked; a hit is one relaxed atomic add; the debug log lines cost nothing
   unless debug logging is on.

## 9. The reserved places (chunk 1), and how they open

| place | chunk 1 | chunk 2 |
|---|---|---|
| `[rules]` in `config/game.toml` (`config::read_file`) | "reserved for chunk 2" | opens: section 4.2 |
| manifest key `rules` (`ext/manifest.rs` `RESERVED`) | "reserved for a later chunk" | opens: section 4.4 |
| `[ext.<name>]`, manifest `options`, editor key `ext.<name>.<key>` | reserved for chunk 5 | unchanged, the same messages (decision 3) |
| manifest keys `scripts`, `api` | reserved | unchanged (chunk 5) |

The tests of chunk 1 that expect the `[rules]` and `rules` refusals change into tests that those places now parse (and the remaining
reservations keep their tests).

## 10. Open for later

- **Lua at the same points** (chunk 5): the eight points get a second consumer; a Lua handler may refuse or change a number the same
  way, after the rules (the spec of chunk 5 settles the order). The events (kill, login, timer, …) are chunk 5's.
- **Rust item-effect handlers** (chunk 3), and whether a compiled-in Rust extension trait is ever needed (decision 3).
- **The waves' points** (5.4) and the catalogue's rule types (chunk 7).
- **Per-server rules** (one database, several game servers): the file is per server already; the editor's rules are shared.
- **The editor's page** for rules and settings (the data-editing page work).
- **The notice packet** (client research entry 12 g) as a second way to tell a player, if it proves better than the whisper.

## Build

Steps 1 to 9, one after the other; each runs its tests (one cargo command at a time: the machine is short on memory, no parallel builds
or test runs), is committed, and the drill's game server is rebuilt and restarted after each commit that changes the server.

Conventions (as chunk 1): no `cargo fmt` (wide lines by convention); cargo is `~/.cargo/bin/cargo`; match the surrounding code (doc
comments naming the docs section, `anyhow`, tests next to the code, PostgreSQL tests skipped without `DATABASE_URL`); stage named paths
only (other sessions share the tree); no mention of AI in commits or code. **The development checkout's `.env` points at the drill**
(`shaiya-afterglow-ext1`, port 5434), never at `newshaiya`; the live dev box (worktree `/media/drive0/projects/shaiya-afterglow-live`,
database `newshaiya`) changes only in step 9. Decisions 1 to 4 are the maintainer's; where this plan leaves something open or the code
disagrees with it, the question goes to the maintainer.

**Step 1. The rules module, pure** (`crates/game-server/src/rules.rs`, a `rules/` folder if it passes about 800 lines; `serde_json`
added to `game-server`'s dependencies): `Who { level: u16, kills: u32, country: u8 }`; the scope (section 3) and its parser; the rule
types (4.1) and `check_rule(name, &Value) -> Result<()>`; `RuleSources` (settings rules, extension rules, the file's rules with their
lines, the editor's rules with their change sets) and `resolve(&Config, &RuleSources) -> Result<(RuleSet, Vec<ResolvedRule>)>` (4.3,
errors naming the layer and place); `from_settings(&Config)` (section 7); `RuleSet` with one method per point (`attack(map, a, b)`,
`trade(map, a, b)`, `cross_faction_trade(map, a, b)`, `denied(Point, map, a)`, `exp_factor(map, a, source)`, `pvp_kills(map, a)`,
`pvp_deaths(map, a)`, `enchant_jump(map, a, armour, draw)`), `len`, `list` (name, type, layer, hits); `set_hour_of_week`. Unit tests:
every type parses and every key error names the key; every scope key alone, `not` on each, AND across keys and OR within one, `who`
on both kinds of point; ranges at their edges; days and hours with the clock set; the layers (an extension's rule replaced by the file,
the file's by the editor, `enabled = false` alone, a name no layer has without `type`, `setting.` refused, a disabled extension's rules
absent since `resolve` gets only the enabled ones); the combinations of 5.2 (two refusals: the first's message; the permission beaten by
a refusal; products; `enchant` first match); `from_settings` for each setting at its edges and at its default; `enchant_jump` against
fixed draws at the weight boundaries and the step-20 cap; an empty `RuleSet` answers allow and 1 at every point.

**Step 2. The file and the database layers**: `config::read_file` parses `[rules]` into the file layer with lines (the reserved-table
test for `[rules]` becomes a parse test; `[ext.…]` stays refused); `Config.config_path`; migration `db/migrations/0032_rules.sql`
(`content.ext_rules (extension text REFERENCES content.extensions (name) ON DELETE CASCADE, name text, rule jsonb NOT NULL, PRIMARY KEY
(extension, name))`, `content.rule_values (name text PRIMARY KEY, rule jsonb NOT NULL)`); `Config::read` also reads the rules of the
enabled extensions and the editor (read as `rule::text`, when the tables exist, as it does for the settings); `Backend::rule_layers()`
for `reload` (PostgreSQL: the same two queries; the memory backend: a value the tests set); `main.rs` resolves the rules after the
settings and stops the start on an error; `game-server rules` and `rules --check` (4.7); `config --example` ends with the commented
example of 4.2 under a `# [rules]` heading (regenerate `config/game.toml.example`; its test stays). Tests: a file with rules gives the
rules with their lines; errors at the right line; `rules --check` exit codes (a test of the function behind it); the example still reads
as nothing set.

**Step 3. The rules in `Zones`, the clock, the whisper**: `Zones` holds `RwLock<Arc<RuleSet>>` (`rules()`, `set_rules()`), set by
`main` (and the test server's start, with `from_settings` of its `Config`); the 30-second clock task in `main`; `SERVER_NAME` shared;
`WorldState.told` and one helper `tell_refused(conn, state, &Refusal)` that applies the 10-second limit. Unit test of the limit (two
refusals of one rule within 10 s: one whisper; another rule: a whisper; silent rule: none).

**Step 4. The attack point** (5.1 row 1): `enemies` tells why two players are enemies; `attackable` takes the map and the `RuleSet`
and asks `attack` for faction enemies only; `attack_verdict`, `may_attack_player` as a wrapper; `Cast` carries the snapshot taken when
the cast starts; `thorns` and `fields` take one snapshot per pass; `Field.owner: Who`. The three request sites whisper. Tests
(`zone/tests.rs`): `deny attack` on a map refuses there and not elsewhere; `level_gap` both ways (21 levels apart with gap 20: neither may
attack; 20 apart: both may); a duel and a guild war across the gap still fight; an area skill skips a protected victim and hits the
others; thorns and a field skip one too (the field after its caster left); with no rules, every existing attack test passes unchanged.
`game_flow`: `a_rule_refuses_an_attack_and_the_server_says_why` (two players of the two factions; `0x0502` gets no attack start and a
whisper from `Server` with the text; a second `0x0502` within 10 s no second whisper).

**Step 5. Trade, personal shop, trade chat** (rows 2 to 4) and **the folding** (section 7): the three call sites ask the rules; the
`any_faction` parameter of `trade_request` goes (its zone tests pass the rules instead). Tests: every test named in section 7 passes
unchanged; one test per folded setting at its edges; a `deny trade` rule answers `START_BUSY` and whispers; a `cross_faction_trade`
rule with `level = "60-"` and `who` both: one trader at 59 is refused.

**Step 6. Experience and player kills** (rows 5 to 7): `ExpSource`, `award_exp`'s factor, `kill_counted`'s `times`, the death count.
Tests: `combat`/server unit tests of the factor's truncation with the item factor (a factor of 1 gives exactly today's numbers); `game_flow`
`killing_a_monster_gives_experience_and_a_level` with an `exp` rule `factor = 2` on its map (twice the gain) and on another map (unchanged);
`pvp_gain` decisions as unit tests of `RuleSet` (a whole player kill has no flow test today, `docs/original/operator-console.md` section
10; it is a live-test item).

**Step 7. Enchant** (row 8): `Enchant.jump`, `new_name`, the rule asked after a success with its own draw. Tests: `lapis` unit tests
(`new_name` with jumps 1, 3 and a cap at 20 for weapons and armour); a `game_flow` enchant test, if one exists, unchanged without rules.

**Step 8. `reload`, the console's `rules`, the editor and extensions**: `reload` as 4.6 (all or nothing, the count, the settings
warning); the console command `rules` with its help entry; the data editor's target `rule` (4.5); the manifest's `rules` (4.4, migration
0032's `content.ext_rules`, `ext list` and `show`, the hint lines). Tests: `game_flow` `reload_puts_new_rules_in_force` (a rule written to
the test's config file and to the memory backend's layers; before `reload` the attack goes through, after it is refused; a broken rule
makes `reload` fail and changes nothing, monsters included); the console's `rules` answer; data-editor PostgreSQL tests: draft → apply
→ `content.rule_values`; `game-server`'s `resolve` (with that table's rows) puts it over the file's rule of the same name; undo removes it;
export and import carry it; `{"enabled": false}` on an extension's rule; `ext install` of a test extension with `[rules]` stores them,
`ext disable` leaves them out of the resolved set, `ext remove` deletes them; a bad rule in a manifest refused at `ext check` naming the
rule and key. A dry run on the drill: a rule added with `data-editor draft add …`, `apply-draft`, `game-server rules` shows it from the
editor, `undo`, gone.

**Step 9. Docs, the drill, the live dev box**:
- `docs/server/rules.md` (new, published on the site under Settings in `site/mkdocs.yml`): what a rule is, the scope, every type with an
  example, the layers, `reload`, `game-server rules` and `--check`, the console's `rules`, the editor's commands, the three settings as
  shorthands. `docs/server/settings.md`: the reserved sentence about `[rules]` replaced by a link; the three settings' lines (section 7).
  `docs/server/deviations.md` section 1: rules are additions, none in force by default but the one the trade-chat setting has always
  made. `docs/original/operator-console.md` (the `reload` answer, the `rules` command), `docs/original/pvp.md`, `lapis.md`, `trade.md`,
  `personal-shop.md`: one line each where a point now asks the rules. `README.md` (feature list, test count), `CHANGELOG.md` "Unreleased"
  (chunk 2; no migration step: nothing an operator has changes), `docs/plans/roadmap.md`, `docs/plans/extensions.md` section 3 and 7.2
  (built), `docs/plans/README.md` (this file), this file's top line (built, with what the build settled where the text left a choice).
  `tools/check_links.py` and `site/build.py` clean.
- `docs/testing/live-test-checklist.md`: a new section after 58 with the items only a real client shows (below).
- **The drill**: migration 0032 runs at the drill server's start; `game-server rules` on the drill shows `setting.trade_chat_min_level`
  and the drill's own `market_maps` and `cross_faction_trade_maps` rules from the file; the examples of 4.2 pasted, `rules --check`,
  `console <id> reload` on the drill's server, `.rules` answers.
- **The live dev box** (as chunk 1 section 9 step 5): `pg_dump -Fc` of `newshaiya` to `data/backups/pre-ext2-<date>.dump` first; the live
  worktree checked out at the built commit, rebuilt, servers restarted (migration 0032 runs); `game-server rules` shows the three
  folded rules from its `config/game.toml`; the examples of 4.2 appended to that file **commented out**, for the maintainer's live test
  (uncomment, `console 1 reload`). Commit; ask before pushing or deploying the site.

**Live-test items** (the new checklist section, for the maintainer with one or two real clients; how to turn each rule on: uncomment it in
`config/game.toml`, `target/release/console 1 reload`):
1. Without any `[rules]`: trade chat below level 10 says nothing (as before); a personal shop outside 35, 36, 42 does not open (as
   before); both factions trade on 42 (as before).
2. `level-gap` (set `gap` to fit two test characters): neither can attack the other; the attacker gets the whisper from `Server` once
   in 10 s; a duel between them still works.
3. `safe-evenings` style `deny attack` on a map: attacks refused with the rule's message; area skills there do not hit players.
4. `market-no-pvp-gain`: a player kill on 42 gives no experience, the kill counter and the faction count do not move, the victim's death
   counter does not move; on another map they do.
5. `enchant-jumps` (weights changed to `[[3, 1]]` for the test): a successful enchant goes up three steps; the item's tooltip, the
   blacksmith window and other players' view (`0x0507`) show the new step.
6. `.rules` in game answers the list with hit counts; `.reload` after editing a rule puts it in force without a restart.

**Done when**: the whole test suite passes (one run, after the last step); the three settings' keys work as before (their tests
unchanged); the two reference rules and the enchant jump work from the file and from the editor, with undo and `reload`.
