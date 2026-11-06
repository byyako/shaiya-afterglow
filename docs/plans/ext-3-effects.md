# Extensions chunk 3, item effects and skill abilities (spec and build plan)

**Specified 2026-11-05; not built.** Chunk 3 of `docs/plans/extensions.md` (section 6, work item 7.3). Chunks 1, 2 and 5 are built
(`docs/plans/ext-1-foundation.md`, `docs/plans/ext-2-rules.md`, `docs/plans/ext-5-lua.md`; the operator's pages `docs/server/rules.md` and
`docs/server/scripts.md`): extensions with their manifest and rows, settings in four layers, rules at eight points, scripts with the `game`
API. This chunk adds a **registry of item effects and skill abilities**: named, parameterised behaviours written in Rust that an
extension's rows name in `rows.toml`, so a new item or skill row can do something the original's `special` and ability numbers do not. It
ships the **level-up stone** (the plan's reference), moves the **Kill Boost Stone** onto the registry without changing what it does, adds
**Perfect Lapisia** and **stat Recreation Runes**, and gives the rules an **item-use point**. With no extension installed nothing changes.

Background read for this spec: `docs/plans/extensions.md` sections 2, 2a, 4, 5, 6, 7.3; `docs/plans/ext-1-foundation.md` sections 4, 5
and 6; `docs/plans/ext-2-rules.md`; `docs/server/rules.md`; `docs/plans/ext-5-lua.md` (its top line) and `docs/server/scripts.md`;
`crates/game-server/src/item_use.rs`, `server/items.rs` (`use_item`, `Effect`, `item_skill`, `Done`), `buffs.rs` (`BuffEffects`,
`item_skill_refused`), `skills.rs` (`SkillDef`, `Bonuses::add`, `KillBoost`, `SkillMatch`), `zone.rs` (`Zones::kill_boost`),
`server/pvp.rs` (`kill_counted`), `lapis.rs` and `server/lapis.rs` (`enchant`, `compose`, `reroll`), `combat.rs` (`add_exp`,
`max_level`), `server.rs` (`award_exp`, `announce_level`), `main.rs` (the load), `config.rs` (`kill_boost_skills`); `crates/data-editor/src/ext/`
(`rows.rs`, `ops.rs`, `ids.rs`, `tables.rs`); `docs/original/item-use.md`, `item-flags.md`, `item-windows.md`, `skills-buffs.md`,
`skills-special.md`, `lapis.md` sections 5 and 6; `docs/client/research-extension-features.md`, `research-balance.md` (the client's stat
window), `research-gambling-kill-boost.md` ("what adding dedicated content takes"), `client-patch-0109.md`. The drill database
(`shaiya-afterglow-ext1`) was queried for the free item slots of section 10.

## 1. The maintainer's decisions (2026-11-05)

1. **Custom GM status levels leave this chunk for wave R3** (`docs/plans/extensions.md` sections 4 B, 6 and 7.7): they touch the login
   server, the account status and the client's privilege byte, not items. Chunk 3 is item effects and skill abilities only.
2. **The effects built and shipped**: the level-up stone (the reference), the Kill Boost Stone ported, **Perfect Lapisia** and **stat
   Recreation Runes**. Not now: an experience ability for VIP and Union buffs, a kill-count rune (section 12).
3. **The rules get an item-use point now**: a `deny` rule with `point = "item_use"` and an optional `items` key, scoped like every rule
   (section 6). Without such a rule nothing changes.
4. **`kill_boost_skills` keeps working** as a shorthand: at start each entry becomes a `kill_boost` ability on those skills, as chunk 2
   folded three settings into rules. The Kill Boost extension moves to the row form (section 5.4). No migration step for operators.

Decided here (no behaviour of the original or of 0.2.0 changes, so not asked):
- **An extension's effects and abilities are server-only data**, kept in `content` tables beside its rows, never in the `defs` columns.
  The client's `Skill.SData` carries the three ability pairs and the client's stat window reads abilities 4 to 9 and 23 to 28 from it
  (`docs/client/research-balance.md`); an ability number the client does not know would be shipped to it and read by code nobody has
  checked. With the registry in `content`, the `defs` rows stay as they are, the client rows are the ones chunk 1 generates, and patch
  109 regenerates unchanged. The player reads what an item does from its client description, as for the Kill Boost Stone.
- **Handlers are Rust**, in the game server's new `effects` module. An extension adds an effect by naming a type of the registry with its
  parameters; a new type is a handler added to the server (section 3.5). Effect types written in Lua are left for later (section 12):
  `game.on_item` already lets a script take an item over.
- **Effects and abilities belong to enabled extensions only** (chunk 1 decision 4: disable means "behaviour off", the rows stay). A
  disabled extension's items do what their own `defs` columns say, which for the reference items is the core's error answer, the item
  kept (section 3.3).
- **They take effect at restart**, as the rows they sit on (`ext install`, `upgrade`, `enable`, `disable` already print "restart the game
  servers" for rows).
- **The order at item use** is: the core's checks, the rules, the reuse group, a script that took the item over (`game.on_item`), the
  extension's effect, the core's own effect (section 3.3). So `game.on_item` keeps its meaning (`nil` is "do what the item does without
  the script") and its API is unchanged: chunk 5's "move `on_item` onto the registry" is this chain.
- **A Lapisia's own jump table stands in for the rules' `enchant` rule** (section 5.2): the item is the player's specific choice; the
  scripts' `enchant` point is still asked after it.
- **A level from a stone is not experience**: the rules' and the scripts' `exp` points and the item flags' factor are not asked
  (section 5.1).

## 2. What changes

| Today (chunks 1, 2, 5 built) | After chunk 3 |
|---|---|
| `[[item]]` and `[[skill]]` entries of `rows.toml` hold `id`, `copy`, `set`, `client`, `server_only`, `note` | also `effect` (items) and `abilities` (skills), section 4 |
| an item's use is decided by its `special`, `range`/`attacktime` and HP numbers (`item_use::effect`), or a script's `on_item` | an extension's effect comes between the script and the core (section 3.3) |
| the Lapisia's success rate is the step's (`ENCHANT_RATES`); a Recreation Rune rerolls every crafted number | a Lapisia or rune with an effect has its own rate, jump, safety, or rerolls named numbers only (5.2, 5.3) |
| the kill boost: `kill_boost_skills` (the Kill Boost extension sets it) read by `Zones::kill_boost` | a `kill_boost` ability on the skill rows; the setting a shorthand that makes the same abilities (5.4) |
| rule points: attack, trade, personal_shop, trade_chat, exp, pvp_kills, pvp_deaths, enchant | also `item_use` (section 6) |
| `extensions/`: kill-boost 1.0.0 and the four script extensions | kill-boost 1.1.0, and `level-stones`, `perfect-lapisia`, `stat-runes` (section 10) |
| the Lua `item` table of `item_use`: `type`, `type_id`, `bag`, `slot`, `count` | also `effect` (the type of the item's effect in force, or `nil`), section 7 |
| `custom GM status levels` in chunk 3 | wave R3 (decision 1) |

Nothing else of the core changes: an item without an effect, a skill without abilities and a server without extensions behave byte for
byte as before.

## 3. The registry

### 3.1 Item effects

An **item effect** is a type name and its parameters. Each type acts in one **request**: the item's own use (`0x050A`, `0x0557`,
`0x0559`), the Lapisia of an enchant (`0x0805`) or the rune of a recreation (`0x0806`). The types of this chunk:

| `type` | request | parameters | what it does | the row must be |
|---|---|---|---|---|
| `level_up` | use | exactly one of `levels` (1 to 59) and `to` (2 to 60); optionally `max` (2 to 60) | raises the user's level (section 5.1) | an item the core uses (item type 25, 44 or 100: real type 26) |
| `enchant` | enchant | optionally `rate` (1 to 1,000,000, default 1,000,000), `jumps` (as the rules' `enchant`: `[[steps, weight], …]`, steps 1 to 19, weight 1 to 1,000,000, at most 19 entries), `safe` (`true`/`false`, default `false`); at least one given | the Lapisia's success rate, its jump on success and whether a failure may destroy the item (section 5.2) | a Lapisia (item type 95) |
| `recreate` | recreation | `stats` (required: a list of 1 to 9 of `"str"`, `"dex"`, `"rec"`, `"int"`, `"wis"`, `"luc"`, `"hp"`, `"mp"`, `"sp"`, no repeats), `value` (`"random"` or `"max"`, default `"random"`) | new values for the named crafted numbers only (section 5.3) | a Recreation Rune (`special` 62, the core's test of a rune) |

Every type has `type`; any other key is an error naming it, as are a key of another type and a value out of range. An item has at most one
effect.

### 3.2 Skill abilities

A **skill ability** is a type name and its parameters on a skill level. It counts while that skill level is applied to a player, whoever
applied it (an item, a cast, a script's `add_buff`), exactly as the original's three ability pairs do, but it is server-only. The type of
this chunk:

| `type` | parameters | what it does | read where |
|---|---|---|---|
| `kill_boost` | `times` (required, 1 to 100) | a counted player kill counts `times` times while applied; with several, the highest | `server/pvp.rs` `kill_counted` through `Zones::kill_boost` (section 5.4) |

A skill level has at most 4 abilities (an `abilities` list in its row). They live on the definition: `SkillDef` gains `ext: Abilities`, a
small `Copy` value (at most 4 entries of the `Ability` enum, each with its parameters inline), filled at load (section 3.4), so an
applied skill carries them in its `Applied.def` and nothing is looked up when a kill is counted. Only levels 1 to 3 of extension rows get
abilities from rows (chunk 1 section 5.6); the shorthand of section 5.4 may name any level, as the setting does today.

### 3.3 How an item's use is decided

`server/items.rs` `use_item`, for a request of the player (unchanged up to the quest items, which keep their path and ask nothing below):

1. the core's checks as now: alive, the place, the definition, the effect not refused, `item_use::may_use` (level, race, mode), the
   skill-cut guard for moves; **the core's own refusal now looks at the effect in force**: an item with an extension effect of request
   `use` is not refused for having no core effect (as a script-registered item is not);
2. **the rules' `item_use` point** (section 6): a refusal answers `use_failed` and whispers, the reuse time untouched;
3. the reuse group (`state.reuse.start`);
4. **a script that took the item over** (`game.on_item`), as now: `"use"`, `"keep"`, `false` end here; `nil` goes on;
5. **the extension's effect** of request `use` (`Effect::Ext`, looked up by the item's id in `GameData.effects`), if the item has one
   whose extension is enabled;
6. otherwise **the core's effect** (`item_use::effect`, unchanged).

`item_use::effect(def)` stays the pure function of the original's columns. A new `item_use::effect_in_force(def, &ItemEffects) -> Effect`
returns `Effect::Ext` for an item with a `use` effect, else `effect(def)`. An item whose effect is of another request (a Lapisia, a rune)
keeps the core's answer at `0x050A`: refused, as today, since both are used through their own requests.

`Effect::Ext` with `0x0557`/`0x0559` of a kind other than self (`it::KIND_SELF`) is `Done::Failed`, as the experience wheels are.

A handler answers with the existing `Done` (`UsedUp`, `Consumed`, `Failed`) and optionally a refusal text, whispered from `Server` as a
rule's (`refused::tell_refused`; at most once per 10 s per effect type and player, `Refusal.rule` an id per effect type).

**A disabled extension**: its effects are not loaded, so step 5 never matches and the item does what its `defs` columns say. The reference
rows are built so that this is the core's refusal (section 10): `use_failed`, the item kept. `ext check` warns for an item whose own
columns give a core effect other than a refusal (it prints that effect: "when the extension is disabled, 100/230 is a potion").

### 3.4 Validation and loading

- **One parser, in the game server**, as for rules and options: `game_server::effects::check_effect(&serde_json::Value) ->
  Result<ItemEffect>` and `check_abilities(&serde_json::Value) -> Result<Abilities>` (errors name the key and say what is allowed), and
  `effects::fits_row(&ItemEffect, item_type, special) -> Result<(), String>` (the last column of 3.1). The data editor calls them at
  `ext check`, `install` and `upgrade` (section 4); the game server calls them at start on what the database holds.
- **The load** (`main.rs`, after the items and skills): `pg::load_item_effects(&pool)` and `pg::load_skill_abilities(&pool)` read the rows
  of **enabled** extensions (`JOIN content.extensions … WHERE enabled`), parse each, and build `GameData.effects: ItemEffects` (item id →
  `Arc<ItemEffect>`) and the abilities, which `SkillDefs::attach_abilities` writes into the definitions; then the `kill_boost_skills`
  shorthand is attached the same way (section 5.4). An effect whose item is not in `defs.items`, a type this server does not know (a newer
  data editor) or a parse error **stops the start**, naming the extension, the item or skill and the key. One info line:
  `item effects loaded` with the counts by type, and the abilities' count.
- The memory backend and the test server get the same through `GameData` (a default that is empty, and a helper the tests use).

### 3.5 Adding an effect type later

A new item effect or skill ability is, in the server: a variant of `ItemEffect` (or `Ability`) with its parameters, its arm in
`check_effect` (or `check_abilities`) and `fits_row`, its handler at the request's place (the use chain of 3.3, `lapis.rs` for enchant and
recreation, or the place that reads the number for an ability), unit tests of the parser and the handler, a `game_flow` test, a line in
`docs/server/effects.md`; and, in an extension, a row that names it. Nothing in the data editor changes for a new type: it validates through
the server's parser. That is the plan's "done when": a new effect is added by a row and a handler.

## 4. `rows.toml` and the database

### 4.1 The keys

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
client = { name = "Kill Boost", look = "250/1", description = "…" }
```

- `effect` (`[[item]]` only): a table, validated by `check_effect`, then by `fits_row` against the **computed** row (after `copy` and
  `set`). `abilities` (`[[skill]]` only): a list of tables, validated by `check_abilities`. On a `[[drop]]` either is an unknown key; on
  the other kind the message says which kind takes it (`abilities` belongs to `[[skill]]`).
- Errors name the file and entry as chunk 1's (`rows.toml [[item]] #1 (100/215): effect: levels: 0 is not 1 to 59`,
  `rows.toml [[item]] #2 (100/216): effect level_up needs an item the core uses (item type 25, 44 or 100); 95/3 is type 95`).
- **The client rule** of chunk 1 (an entry whose row differs from the original needs `client` or `server_only`) also holds for an entry that
  gives an **original** row an effect or abilities (a change of what an original item does), even when no column changes: the client's
  own text would be wrong otherwise.
- An extension may give an effect only to items in its `[ids]`, abilities only to skills in it (already true of every entry).

### 4.2 Migration `db/migrations/0034_effects.sql`

```sql
-- Item effects and skill abilities of extensions (docs/plans/ext-3-effects.md section 4.2). Server-only: never in defs or the client.
CREATE TABLE IF NOT EXISTS content.ext_item_effects (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    item_id   integer NOT NULL PRIMARY KEY,        -- one effect per item, whoever owns it
    effect    jsonb NOT NULL                       -- as written in rows.toml
);
CREATE TABLE IF NOT EXISTS content.ext_skill_abilities (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    skill_id  smallint NOT NULL,
    level     smallint NOT NULL,
    abilities jsonb NOT NULL,                      -- the list as written
    PRIMARY KEY (skill_id, level)
);
```

The manifest's format stays 1 (the two keys are new in rows files, which have no format number; an older data editor refuses them as
unknown keys, which is the right answer for an older tool).

### 4.3 The commands

- `ext check <folder>`: the validations of 4.1 and the warning of 3.3; prints each effect and ability (`100/215 level_up levels = 1`,
  `200/1 kill_boost times = 2`).
- `ext install` / `upgrade`: writes the two tables in the same transaction as the rows (upgrade: replaced). `ext remove`: the cascade
  (the item rows go by chunk 1's remove, with its holder and reference checks). `ext list` gains an effects count (items with an effect plus
  skill levels with abilities); `ext show` lists them.
- The hints: "restart the game servers" already printed for rows; for an extension that has effects and no rows changed (an upgrade that
  only changes parameters), the same line.
- `ext scripts <folder>` (chunk 5): a changed `effect` or `abilities` is a changed row for its comparison ("rows, rules, settings or
  migrations changed: use `ext upgrade` with a higher version").
- `content check`, `hashes`, `client_patch.py`: untouched (the tables are not `defs`).

## 5. The effects of this chunk

### 5.1 `level_up` (the reference)

- The **target level**: `levels = n` gives the current level + n; `to = n` gives n. Then at most `max` when given, and at most the mode's
  cap (`combat::max_level`: 30 in easy mode, 60 otherwise).
- **Refused** (`Done::Failed`: `use_failed`, the reuse time put back, the item kept) when the target is not above the current level: at the
  cap, at or above `to`, at or above `max`. The whisper: `This item cannot raise your level any further.`
- **Otherwise, in this order** (audit R13: the item, then what lasts): the item used up and saved (`consume`); the experience becomes the
  **start of the target level** (`ExpTable::total(grow, target − 1)`), given through `combat::add_exp` with the difference, so the stat and
  skill points of every level gained come as from levelling, and the class stat as `add_exp` gives it; `save_progress`; one `0x0308` game-log
  record per level (as `award_exp`); `0x0207` with the gain; `announce_level` with `by_other = true` (not the player's own kill); `0x050A`.
  Progress inside the current level is not carried over (as the GM's `/set level`).
- **Not asked**: the item flags' factor, the rules' `exp` point and the scripts' `exp` point. A level is not experience (decided here).
- **Log**: `tracing::info!(id, item, from, to, "level by item")`.

### 5.2 `enchant` (Perfect Lapisia)

At `server/lapis.rs` `enchant`, for a Lapisia with an `enchant` effect (`lapis::enchant` gains the Lapisia's effect, if any):

- **Rate**: `rate` replaces `ENCHANT_RATES[step][armour]`. **A rate of 1,000,000 always succeeds** without a draw (the core's test
  `materialCount + draw < rate` could fail at the highest draws when a material is counted). The material, its count and the price are used
  as now.
- **Jump**: on success, with `jumps`, the jump is drawn from the Lapisia's table (its own draw below 1,000,000, the weights proportional, as
  the rule), and **the rules' `enchant` rule is not asked**; without `jumps` the rule is asked as now. The scripts' `enchant` point is asked
  after either, with the jump so far. The step stays at most 20 (`Enchant::new_name`).
- **Safety**: `safe = true` means a failure never destroys the item, whatever the Lapisia's `vg` says; `false` (the default) leaves the
  core's rule (`dangerous` from `vg`).
- Everything else (the checks of `lapis::enchant`, the fits, the cap, the answer `0x0805`, the game-log records) is the core's.

### 5.3 `recreate` (stat Recreation Runes)

At `server/lapis.rs` `compose` (`0x0806`), for a rune with a `recreate` effect, after the core's checks (`lapis::compose`: craftable, a
special-62 rune, not a quest reward):

- Only the **named crafted numbers that the item already has** (not 0 in its craft name) change; every other number and the enchant step
  stay. None of the named numbers present: refused with `0x0806` result 1, the rune kept (the core's refusal answer).
- `value = "random"`: each named number becomes the smallest of **two** draws `rand() % (W + 1)` (`W` the item's `reqwis`, the draw count
  of the core's rune); `"max"`: each becomes `W`.
- The rest is the core's `reroll`: the save with the rune used up, `0x0806` with the new craft name, the game-log `0x0427` record, the
  numbers sent for an equipped item.
- `lapis::rerolled_values(item, def, &Recreate, rand) -> Option<[u8; 9]>` is the pure part (beside `roll_craft`).

### 5.4 `kill_boost` and the port of the Kill Boost Stone

- `Zones::kill_boost(map, id) -> u32`: the highest `times` among the `kill_boost` abilities of the player's applied skills (`Applied.def.ext`),
  1 when none. The `boosts` parameter goes; `kill_counted` stops passing `ctx.config.kill_boost_skills`.
- **The shorthand** (decision 4): at start, after the abilities from rows, `SkillDefs::attach_kill_boosts(&config.kill_boost_skills)` adds
  `kill_boost { times }` to every loaded definition the entry matches (`SkillMatch::matches`: the id, and the level when given, any loaded
  level otherwise, level 100 included, as today). Several matches on one level keep the highest, so the result is the old one: the highest
  `times` of the entries matching an applied skill. A skill level that would get a fifth ability from the shorthand keeps its four and the
  start logs a warning (no shipped skill has any, so it cannot happen with the original data).
- `docs/server/settings.md`: `kill_boost_skills` "makes `kill_boost` abilities on those skills at start; an extension's rows can give the same
  ability (`docs/server/effects.md`)". `game-server config` shows it as before.
- **The extension**: `extensions/kill-boost` 1.1.0: each `[[skill]]` gains `abilities = [{ type = "kill_boost", times = 2 }]` (5, 10), and
  `extension.toml` loses `[settings]` (and its header comment says where the multipliers now are). The `defs` rows and client rows are the
  same, so `content hashes defs` and patch 109 do not change. On a server where 1.0.0 is installed, `data-editor ext upgrade
  extensions/kill-boost` replaces the setting's default with the abilities; before the upgrade it keeps working through the setting.
- Disabled, the extension's abilities are not loaded: the stones and buffs stay and count once, as before.

## 6. The rules' `item_use` point

| | |
|---|---|
| point | `item_use` (`rules::Point::ItemUse`), a refusal, one player |
| rule | `deny` with `point = "item_use"` and optionally `items`: a list of `"type/type_id"`, `"type/a-b"` (a range of type ids) and `"type"` (every item of a type), as the manifest's `[ids] items` plus the whole type; missing means every item |
| place | `server/items.rs` `use_item`, step 2 of section 3.3: after the core's checks, before the reuse group and the scripts |
| who | the user (level, kills, faction), the map it stands on |
| answer | `use_failed`, and the whisper of chunk 2 decision 1: the rule's `message` or `You cannot use that here.` |
| not covered | quest items (they have no use to refuse), a Lapisia in an enchant and a rune in a recreation (their own requests; the `enchant` point is chunk 2's), items used by the death window (`0x0553` kinds 4 and 5) |

- `items` is accepted only on `deny` with `point = "item_use"` (elsewhere it stays the `enchant` rule's own key with its meaning there);
  validation as the scope's: type 1 to 255, type ids 1 to 255, low end at most the high end.
- Compiled as the other points (`docs/plans/ext-2-rules.md` section 8): a sorted list of `(type, low, high)`; the point's first test is the
  empty list of rules. One `RuleSet::item_use(map, who, item_type, type_id) -> Option<Refusal>`.
- Example in `config --example` and `docs/server/rules.md`:

```toml
# Level stones do not work on the PvP maps.
[rules.no-level-stones-in-pvp]
type = "deny"
point = "item_use"
items = ["100/215-217"]
maps = [2, 3, 4]
message = "Level stones do not work here."
```

- The scripts' `item_use` point (registered items) stays after the rule, as every script point is (a rule's refusal is final).

## 7. Scripts

- `game.on_item` (API 1) is unchanged in name, arguments and answers; it is step 4 of section 3.3. A script may take over an item that has
  an extension effect (its own or another extension's); `nil` then runs that effect. This is how chunk 5's "chunk 3 may move `on_item` onto
  the registry, keeping the API" is settled: the chain is one function (`effects::use_chain`), the script one link of it.
- The `item` table passed to an `item_use` handler gains **`effect`**: the type name of the item's extension effect in force (`"level_up"`),
  or `nil`. API 1 may gain fields (`docs/plans/ext-5-lua.md` section 7); `API_VERSION` stays 1.
- `docs/server/scripts.md`: the order of 3.3 in the `item_use` row, and the new field.

## 8. The client

- **No `fixes.dll` feature.** Every packet the effects send is one the server already sends (`0x050A`, `0x0207`, the level-up packets,
  `0x0805`, `0x0806`).
- **Client rows** come from the rows as chunk 1 specified: the reference items take empty records of the US patch 108 `Item.SData`
  (section 10), with a look-alike, a name and a description; skills of this chunk are the Kill Boost's (unchanged). After the build the
  next generated patch (110) carries the new items; the client side packs and signs it (`docs/plans/done/client-patches.md`). Until a
  client has it, the items show as `???`.
- **New research entries** (`docs/testing/client-research-queue.md`; each is also a live-test item, so a live test may answer it first):
  - **24. A usable item with `special` 0, no skill link and no HP, SP or MP** (the level stone's record, a copy of 100/108 without its
    skill): does the client send `0x050A` for it (or refuse it locally with 227), and does a level jump of more than one (`0x051E` from a
    level of 20 to 30) show the level, the points and the bars right? What it changes: the level stone's row (a special the client sends
    for), or nothing.
  - **25. A Lapisia in an empty record of group 95** (95/3, 95/8, copies of 95/1 and 95/6): does the enchant window take an item by its type
    alone, or does it read another field of the record, so that a copy behaves as the original? What it changes: the columns the Perfect
    Lapisia rows copy.
  - **26. A Recreation Rune in an empty record of group 100** (100/221 to 226, copies of 100/171 with special 62): does the client open the
    recreation window from the special (so any special-62 item does), and does it show the new craft name when only one number changes?
    What it changes: the stat runes' rows.
- The ids the reference items use are inside the ceilings of `crates/data-editor/src/ext/ids.rs` (group 95 has 8 records, group 100 has
  255), so no client research beyond these is needed.

## 9. Settings and rules touched

- **Settings**: none new. `kill_boost_skills` keeps its key, parser and layers; its meaning is now "makes abilities" (5.4).
- **Rules**: a new point and the `items` key of `deny` (section 6); `game-server rules`, the console's `rules` and the editor's target `rule`
  take it with no change of their own (they handle every type through the parser).
- **Defaults**: a default install has no extension with effects; with none, `item_use` is asked with an empty rule list and the chain of
  3.3 never reaches step 5, so the server behaves as the original (release plan: original behaviour by default).

## 10. The reference extensions

All bundled in `extensions/`, each with a `README.md`, **not installed by `tools/setup.sh`** (as the script extensions). Every reference
item has **grade 999**, the grade of the 5,180 rows no drop table names (the drill: `defs.mob_items` names 394, the Lapisia's, 12 times),
so none drops from monsters; operators sell them in the item mall, make them with the GM's `/make`, or give them from a script. The free
records used (the drill, `defs.items` names starting `???`): group 100 has 75, of which 100/212 to 214 are the Kill Boost Stones'; this
chunk uses 9 more (100/215 to 217, 100/221 to 226); group 95 has 4 (95/3, 4, 5, 8), of which it uses 2. Pets (chunk 4) need egg records
too; its spec chooses the group.

### 10.1 `level-stones` (the reference)

`[ids] items = ["100/215-217"]`. Three items, copies of the Mini Sun EXP Stone 100/108's use and trade columns as the Kill Boost Stones
(eleven columns: those of `extensions/kill-boost` but `grade`), `special` 0, `range` 0, `attacktime` 0 (the placeholders' own), `reqig` 0,
`grade` 999, the look of 100/108:

| item | name | effect | description (client) |
|---|---|---|---|
| 100/215 | Level Stone | `levels = 1` | Raises your level by one. |
| 100/216 | Level Stone (15) | `to = 15` | Raises your level to 15. |
| 100/217 | Level Stone (30) | `to = 30` | Raises your level to 30. |

Disabled, each is special 0 with no HP, SP or MP: the core refuses it and the stone is kept. The README shows `max` and a `deny item_use`
rule (section 6).

### 10.2 `perfect-lapisia`

`[ids] items = ["95/3", "95/8"]`.

| item | copy of | name | effect |
|---|---|---|---|
| 95/3 | 95/1 Weapon Lapisia (every column) | Perfect Weapon Lapisia | `{ type = "enchant", rate = 1000000, jumps = [[1, 60], [2, 30], [3, 10]], safe = true }` |
| 95/8 | 95/6 Armor Lapisia (every column) | Perfect Armor Lapisia | the same |

`set = { itemname = …, grade = 999 }`. Descriptions: "An enchant with it always succeeds and may rise up to three steps." Disabled, each is
an ordinary Weapon or Armor Lapisia (the core's rate and danger): the README says so.

### 10.3 `stat-runes`

`[ids] items = ["100/221-226"]`. Six copies of the Recreation Rune 100/171 (every column: special 62, its stack of 255), `grade` 999, the
rune's look: STR, DEX, REC, INT, WIS and LUC Recreation Rune, each `{ type = "recreate", stats = ["<stat>"], value = "random" }`, described
"Draws a new value for the item's STR (only), as a Recreation Rune does." The README shows `value = "max"` (the auto-max rune of the
catalogue) and several stats in one rune. Disabled, each is an ordinary Recreation Rune.

### 10.4 `kill-boost` 1.1.0

Section 5.4. Its README (new) says what changed from 1.0.0 and how to upgrade.

## 11. Cost

- **Item use** (a request, not a hot path): one hash lookup of the item's id in `GameData.effects`, and the rules' `item_use` point, whose
  first test is its empty list.
- **The kill boost**: the same scan of the player's applied list as today, reading a field of each definition instead of matching a list of
  settings; cheaper.
- **Enchant and recreation**: one lookup each. **The load**: two small queries at start. Nothing is added to the zone's passes, to combat or
  to the stat formulas.

## 12. Open for later

- **More abilities** (each a variant, section 3.5): `exp` (a factor applied with the item flags' factor, for VIP and Union buffs), `gold`
  and `drop` factors, a map pass (needs the waves' map-entry point, `docs/plans/ext-2-rules.md` section 5.4). Offered and not chosen now
  (decision 2), with the kill-count rune (an item that adds N kills).
- **Effect types written in Lua** (a `game.effect(name, fn)` with parameters from the row), once a script author asks; `on_item` covers the
  need today.
- **The editor's target for effects** (change a stone's parameters without an `ext upgrade`): not needed while extensions own the rows.
- The catalogue's Tag and Level stone, Union and VIP buffs, Reroll Doubler and auto-max (the `"max"` rune is the latter), enchant and link
  costs: wave R3's design pass.
- **Custom GM status levels**: wave R3 (decision 1).

## Build

Steps 1 to 9, one after the other; each runs its tests (one cargo command at a time: the machine is short on memory, no parallel builds
or test runs), is committed, and the drill's game server is rebuilt and restarted after each commit that changes the server.

Conventions (as chunks 1, 2 and 5): no `cargo fmt` (wide lines by convention); cargo is `~/.cargo/bin/cargo`; match the surrounding code
(doc comments naming the docs section, `anyhow`, tests next to the code, PostgreSQL tests skipped without `DATABASE_URL`); stage named
paths only (other sessions share the tree); no mention of AI in commits or code. **The development checkout's `.env` points at the drill**
(`shaiya-afterglow-ext1`, port 5434), never at `newshaiya`; the live dev box (worktree `/media/drive0/projects/shaiya-afterglow-live`,
database `newshaiya`) changes only in step 9. Decisions 1 to 4 are the maintainer's; where this plan leaves something open or the code
disagrees with it, the question goes to the maintainer.

**Step 1. The effects module, pure** (`crates/game-server/src/effects.rs`, an `effects/` folder if it passes about 800 lines):
`ItemEffect` (`LevelUp { by: LevelBy, max: Option<u16> }`, `Enchant { rate, jumps, safe }`, `Recreate { stats, value }`), `Request` (`Use`,
`Enchant`, `Recreate`), `check_effect`, `fits_row`; `Ability` (`KillBoost { times }`), `Abilities` (`Copy`, at most 4), `check_abilities`;
`ItemEffects` (item id → `Arc<ItemEffect>`, `get`, counts by type); the pure parts: `level_target(level, grow, &LevelUp) -> Option<u16>`,
the enchant rate and jump (`enchant_rate(step_rate, &Enchant) -> Rate` with `Always`, the jump drawn with the rules' weighted draw, which
moves to a shared function), `lapis::rerolled_values` (5.3). Unit tests: every type and key parses, every error names the key (unknown key,
key of another type, out of range, `levels` with `to`, neither); `fits_row` for each type against the right and a wrong row;
`level_target` (+1 at 59 → 60, +1 at 60 → none, `to = 30` at 29 → 30 and at 30 → none, `max = 50` at 50 → none, easy mode capped at 30);
the rate of 1,000,000 succeeds with the largest material and draw, a rate below fails at its edge; the jump at the weight boundaries and the
cap of 20; recreate: only named numbers that are present change, `"max"` gives `W`, `"random"` the smaller of two fixed draws, none present
→ `None`, the step kept.

**Step 2. The data editor and the database**: migration 0034 (4.2); `rows.rs` reads `effect` and `abilities` (4.1); `ops.rs` validates
them with the server's parser and `fits_row` against the computed row, applies the client rule of 4.1, writes and replaces the two tables at
install and upgrade, prints them at check and show, counts them at list, warns as 3.3; `ext scripts`' comparison includes them. Tests
(data-editor, unit and PostgreSQL): the parse errors with their labels; a test extension with one effect and one ability installs and both
tables hold them; a wrong type of item refused naming the entry; an effect on an original row without `client` or `server_only` refused;
upgrade replaces them; remove deletes them; `ext show` prints them; the disabled-fallback warning appears for a potion-like row.

**Step 3. Loading, and the abilities in the definitions**: `SkillDef.ext`; `SkillDefs::attach_abilities` and `attach_kill_boosts`;
`pg::load_item_effects`, `load_skill_abilities` (enabled extensions only); `GameData.effects`; `main.rs` loads and logs them and stops on an
error (3.4); `Zones::kill_boost(map, id)` from the abilities; `kill_counted` without the setting; the test server's `GameData` helper. Tests:
the zone's `the_kill_boost_finds_an_applied_skill_by_id_and_level` with the same expectations, its entries now attached through the shorthand;
a new zone test with abilities from rows, and with a row's ability and the setting on one skill (the highest wins); a PostgreSQL test: a
disabled extension's effects and abilities are not loaded, an enabled one's are; every existing kill test unchanged.

**Step 4. The Kill Boost port**: `extensions/kill-boost` 1.1.0 (5.4) with its README. Tests: the test that checks every folder of
`extensions/` passes; a data-editor PostgreSQL test installs 1.0.0, upgrades to 1.1.0, and finds the `defs` rows and the client rows
unchanged (the same `content hashes defs`, the same `content.client_skills` / `client_items` rows) and `kill_boost_skills` without an
extension default. **On the drill**: migration 0034 at the drill server's start; `data-editor ext upgrade extensions/kill-boost`;
`game-server config` shows `kill_boost_skills` at its default; the start log counts 3 abilities; `client_patch.py rebuild --patch 109 --out
<scratch dir>` prints `MATCH` (the chunk's "done when").

**Step 5. The use chain and `level_up`**: `item_use::effect_in_force`, `Effect::Ext`, the order of 3.3 in `use_item` (the core's refusal
seeing the effect in force), `server/effects.rs` with the `level_up` handler (5.1) and the refusal whisper; the Lua `item.effect` field
(section 7). Tests (`game_flow`): a +1 stone at level 10 gives level 11 at its start with the points of one level, the level-up packets, the
item used up and the progress saved; a `to = 30` stone at 30 answers `use_failed` with the whisper and keeps the stone; an easy-mode
character at 30 is refused a +1; a stone with its extension's effect absent (disabled) answers `use_failed` and is kept; an `on_item`
script answering `nil` lets the stone's effect run, `"keep"` keeps it without a level; the script sees `item.effect == "level_up"`; every
existing item-use test unchanged.

**Step 6. The rules' `item_use` point** (section 6): `Point::ItemUse`, the `items` key on `deny`, `RuleSet::item_use`, the call site and the
default text, the `config --example` lines (regenerate `config/game.toml.example`, its test stays). Tests: `rules` unit tests (the key on
`deny item_use` only; ranges and whole types; a missing `items` matches every item; scope with maps and levels); `game_flow`: a `deny
item_use` rule on a map refuses a potion there with `use_failed` and the whisper, the reuse time untouched (a second potion of the group
works at once elsewhere), and the same potion works on another map; with no rule every item-use test unchanged.

**Step 7. `enchant` and `recreate`** (5.2, 5.3) in `lapis.rs` and `server/lapis.rs`. Tests: `lapis` unit tests (a rate of 1,000,000
succeeds with 100 of material and the highest draw; the Lapisia's jump table used and the rules' rule not asked; `safe` keeps an item a
dangerous Lapisia would destroy); `game_flow`: an enchant with a Perfect Lapisia whose table is `[[3, 1]]` raises the step by three; a
`recreate` rune for STR changes only STR and keeps the enchant step; on an item without STR it answers `0x0806` result 1 and keeps the rune;
every existing enchant and recreation test unchanged.

**Step 8. The reference extensions** (section 10): `extensions/level-stones`, `perfect-lapisia`, `stat-runes`, each with its README. Read the
columns of 100/108, 95/1, 95/6 and 100/171 on the drill before writing the rows (the eleven columns of 10.1; that none of the reference rows
names a skill; their `grade` 999, and that no other source of items names grade 999 either: the boxes' sets of `ItemCreate.ini`, the
quest drop groups, `docs/original/item-origins.md` and `quest-drops.md`; if one does, the question goes to the maintainer). Tests: the folder test passes for all; `game_flow` tests with the rows and effects of each folder (built
as `ext install` computes them, or set directly with the same rows): a Level Stone from level 1, a Perfect Lapisia on a weapon, a STR rune.
**On the drill**: the three installed with `data-editor ext install`; `ext show` lists their effects; the start log counts them; the
`deny item_use` example pasted, `rules --check`, `console <id> reload`, `.rules` lists it.

**Step 9. Docs, the drill, the live dev box**:
- `docs/server/effects.md` (new, published under Settings in `site/mkdocs.yml`, beside `rules.md` and `scripts.md`): for operators (the
  reference extensions, enabling, the restart, the `deny item_use` rule) and for authors (the `effect` and `abilities` keys, every type with
  its parameters and an example, the chain of 3.3, what a disabled extension's items do, how a new type is added). `docs/server/settings.md`
  (`kill_boost_skills`), `rules.md` (the point and `items`), `scripts.md` (the order and `item.effect`), `deviations.md` section 1 (effects
  are additions, none installed by default). `docs/original/item-use.md` section 11, `lapis.md` section 11, `item-flags.md` (one line each
  where an extension's effect or ability now comes in). `docs/plans/extensions.md`: sections 3 and 7.3 (built), section 4 B's GM row and
  sections 6 and 7.3 (GM status levels moved to wave R3, decision 1), section 5's reference set. `README.md` (features, test count),
  `CHANGELOG.md` "Unreleased" (chunk 3; for operators with the Kill Boost Stones: `data-editor ext upgrade extensions/kill-boost`, optional,
  nothing breaks without it), `docs/plans/roadmap.md`, `docs/plans/README.md`, this file's top line (built, with what the build settled where
  the text left a choice). `docs/testing/client-research-queue.md`: entries 24 to 26 (section 8). `tools/check_links.py` and
  `site/build.py` clean.
- `docs/testing/live-test-checklist.md`: section 61 with the items below.
- **The live dev box** (as chunk 5 step 10): `pg_dump -Fc` of `newshaiya` to `data/backups/pre-ext3-<date>.dump` first; the live worktree
  checked out at the built commit, rebuilt, servers restarted (migration 0034 runs); `data-editor ext upgrade extensions/kill-boost`; the
  three reference extensions installed **disabled** (`ext install`, then `ext disable`), for the maintainer's live test (`ext enable <name>`,
  restart); `client_patch.py check`. **Ask the maintainer before generating patch 110** (it stamps `since_patch` on the new client rows), and
  before pushing or deploying the site. Commit.

**Live-test items** (section 61, for the maintainer with one or two real clients and patch 110 installed; turn each extension on with
`data-editor ext enable <name>` and `tools/game-server.sh restart`; give the items with the GM's `/make`):
1. The Kill Boost Stones after the upgrade: a stone's buff makes a player kill count 2, 5 or 10 times, as before (section 58's items).
2. `level-stones`: the Level Stone raises a level-10 character to 11 with the level-up effect, the new points and full bars; Level Stone
   (30) from level 20 shows level 30 at once (research entry 24); at 30 it is refused with the whisper and stays in the bag; the stone's
   name and text show in the tooltip.
3. `perfect-lapisia`: the Perfect Weapon Lapisia goes into the enchant window (entry 25) and always succeeds, sometimes by two or three
   steps; the item's tooltip and other players' view show the new step.
4. `stat-runes`: the STR Recreation Rune opens the recreation window (entry 26) and changes only STR; on an item without STR it fails and
   the rune stays.
5. A `deny item_use` rule for the stones on one map (uncomment the example, `console 1 reload`): the stone is refused there with the rule's
   message and works elsewhere.
6. Disabled again (`ext disable level-stones`, restart): the stones are refused and kept.

**Done when**: the whole test suite passes (one run, after the last step); the Kill Boost Stone behaves as before (its tests unchanged in
their numbers, live item 1); patch 109's data still regenerates unchanged from the database (`client_patch.py rebuild --patch 109`:
`MATCH`, step 4); a new effect is added by a row and a handler (the three reference extensions); with no extension installed the server
behaves as before (every existing test unchanged).
