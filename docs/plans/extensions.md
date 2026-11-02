# Extensions, rules and pets (design and work plan)

**Frozen (decided 2026-10-22) until after the alpha** (`docs/plans/roadmap.md`, "Frozen"). Nothing below is scheduled; the work items of section 7 are kept for when it restarts.

Not a spec. This gathers what the maintainer decided and asked for (2026-10-22) so that each chunk of work can be specified and then built
later, one at a time. Nothing here is built. Each chunk (section 6) has a work item in section 7 (a spec first, then the build); the
roadmap (`docs/plans/roadmap.md`, phase 4) lists them in order.

## 1. Why

The server is to be released (login and game server files) so that other people can run their own servers or convert their original
files. They will want to change the game: most operators of the original edited the server's config files and data to do it. The
user also has custom ideas (pets, per-player loot) and a list of changes other servers sell (section 4). Doing each of these as a
one-off flag in `config.rs` does not scale and mixes the original's behaviour with custom behaviour. So: a way to package custom work
as **extensions**, a **rules** layer for the many "allow, deny or change this for a map, level range or faction" requests, and
**scripts** for event-driven features; pets is the first full extension.

## 2. The maintainer's decisions (2026-10-22)

1. **Pets**: a weakened, non-boss, non-quest mob that is aggro'd on the player can be caught with an item (the chance rises as its HP
   falls). The catch becomes a **Pet Egg** item carrying the monster id. Summoned, the pet is a small follower of that monster (its
   animations kept), one at a time, **static** (no levels or evolution). It is cosmetic and **picks up its owner's drops** (it walks to
   the item, then the item goes into the owner's bag so the owner sees the normal pick-up line; a full bag leaves the item on the ground).
   A buff is "eh", not wanted now. A menu if the client allows it (client research entry 10(e)); the egg model is fine without.
   Whether it can be small is client research entry 9; the fallback is a new `Monster.SData` record sharing a model.
2. **Per-player loot** (each player, or party member, rolls their own drops, as in WoW): wanted, a **later, separate spec** (chunk 9).
3. **The database is the source of truth.** Additions are not re-applied from `tools/defs/additions.sql` after each load: the original
   data is imported once, the rows an extension adds live in the main database, tagged with their origin, and the client-only fields
   (icon, model, description) live in database tables too, so a client patch is generated from the database alone.
4. **Plugins are for outside authors as well as for the maintainer.** The server is released; others build servers or convert their
   original files. So an extension is a folder with a manifest, there is a stable, documented way to write one, and Lua scripting is
   part of it (specified with the hook layer, built right after pets).
5. **Rules are stored in the database and edited in the data editor, and the config files still work.** Operators of the original
   edited config files; every rule must also be settable from a plain file. The layers: built-in defaults, then extension defaults,
   then the config files, then the editor's edits (the same overlay the `edits` schema already gives the game data).
6. **Cross-faction features are wanted** (trade, whisper, inspect, party, chat). The client should support most of it; whatever
   needs research is in client research entry 12, and what the client refuses by itself is the client-side session's to find.
7. **Work in chunks**, never one pile: each chunk is specified, then built, and ends in something that runs.
8. **Which of the catalogue (section 4) the maintainer's own server uses** is decided later, in a design pass of its own, after the
   foundations exist. The first spec builds a small reference set only (section 5).

## 3. What exists today

- **Lua** (`mlua`, Lua 5.1, vendored): runs only the original's monster scripts (`zone/lua.rs`, `mob_lua.rs`). No hooks for custom content.
- **One-off flags**, the precedents this generalises: `GAME_CROSS_FACTION_TRADE_MAPS`, `GAME_MARKET_MAPS` (the maps where a personal shop may be set up),
  `GAME_TRADE_CHAT_MIN_LEVEL` / `GAME_TRADE_CHAT_COOLDOWN_SECS`, `GAME_KILL_BOOST_SKILLS`, `GAME_KILL_COUNT`, `GAME_KILL_RATE`,
  `GAME_EXP_RATE`; all env variables read in `config.rs::from_env`, the operator console's switches (`switches.rs`) on top.
- **Content rows outside the original's data**: `tools/defs/additions.sql` (the Kill Boost Stone) re-applied after every defs load, and
  `tools/sdata/client_rows.sql` (client-only fields). Both are what chunk 1 moves into the database.
- **Edits overlay**: `docs/plans/done/data-editing.md` (migration 0030, `crates/data-editor`): an `edits` schema re-applied after every
  import, with history, undo, export and import; the page and drawn map (part B) are not built.
- **Client**: `fixes.dll` (client repo, `rust/fixes/src/`) adds packets and fixes; the signed `psNNNN.patch` updater carries data
  files; patch 109 (`docs/plans/done/client-patches.md`) is generated and waits for the client-side session to pack it.
- **Hooks**: none. The server is one crate of about 44,000 lines; behaviour sits where the original's code did.

## 4. The catalogue (the maintainer's list of changes other servers sell)

Every item is "applies to certain maps" unless it says otherwise: a rule has a **scope** (maps, level range, kill-count range,
faction, optionally a time window). "Client?" marks what needs client research (entries 9 to 12 of `docs/testing/client-research-queue.md`)
or a DLL change. The wave names are section 6's chunk 7.

**A. Scoped rules (allow, deny or change a behaviour):**

| Item | Wave | Client? |
|---|---|---|
| Cross-faction trade, whisper, inspect, party; chat combining; GMs see both sides' chat; GMs attack both sides | chunk 6 | yes (entry 12) |
| FFA map; a custom FFA system | R1 | yes (entry 12 c) |
| Higher levels cannot attack lower levels; higher levels get no kills from lower levels; protect low-level PvP zones | R1 | maybe |
| No exp from PvP zones; remove kills for a map (the market bug); special kill count by map, by amount (3x until 30,000 kills, 2x until 50,000) and by level range | R1 | no |
| Custom safe zone (every attack misses, no guards needed); no level cap on PvP rank rewards (normally level 16) | R1 | no |
| A map nobody can leave; custom exit and relog time; enter a map only if the party has an item or the leader has an item; a map for two raids (one per faction) | R2 | maybe |
| Block specific skills, equipment, or chat (by map, kill count, level, position, sitting); warehouse blocking by item id or type; items that cannot be stored, put in the guild bank or traded | R2 | maybe |
| Remove the AP item requirement (globally or for certain maps) | R2 | no |
| Items usable only by one gender or only in a guild; use an item by level or kills | R2 | maybe |
| Block trade-chat spam and the level+10 trade-chat hack (partly built: `GAME_TRADE_CHAT_*`) | R2 | no |

**B. Number changes (modifier points):**

| Item | Wave | Client? |
|---|---|---|
| Enchant jumps (not only +0 to +1); remove enchant and link costs; auto-max orange stat; Reroll Doubler (doubles the orange stats); Perfect Lapisia (100%, optionally +1 to +3) | R3 | item rows |
| 100% crit for a scope; stat formula changes (evasion so the dex glitch ends, HP per Rec, ...); changed Bless effects; a custom stat and skill point system | R3 | maybe |
| Custom GM status levels (no more 16/32/48/64); custom ability types for skills | R3 / chunk 3 | tooltip text |

**C. Items and skills (rows plus effect handlers):** level-up stone (+1, or to 15, 30, 70); stat-specific Rec rune (random or max); a
rune that raises the kill counter; map-pass consumables; Tag and Level stone; Union buffs; VIP buffs. Chunk 3; client rows (`Item.SData`,
`Skill.SData`) come from the database.

**D. Event features (scripts):** kill streak; bounty (target reward); auto PvP rewards; item, level, tag and AP rewards for kills;
special kill rewards (buff, item, skill); buff the raid when a boss dies; vet-manager buffs by PvP rank; guild buff merchant in the guild
house; leader resurrection in any mode; welcome notice; auto advertising banner; mention (tag) system; block unacceptable nicknames
(bad words, look-alike letters); custom GM commands; a custom naked/topless system. Chunks 5 and 7 (R4); the notice packet is in
entry 12 (g).

**Not extensions:** the Fury HP bug (core fix, goes to the live-test fixes); "also selling any modification" (the source list's sales line).

## 5. The architecture, in four layers

1. **Content (data).** Rows in `defs.*` and `world.*` carry an `origin` (`original` or `ext:<name>`); a client table set holds what the
   client needs for each row (`client.item_rows`, `client.skill_rows`, a monster table when pets need it). An extension installs and
   removes its rows through versioned, idempotent migrations and declares the **id ranges** it owns (items by type and type id,
   skills, monsters, NPCs, quests), checked against the client's ceilings (entry 11 a). The client patch generator reads the
   database only.
2. **Rules.** One table of rules: a type, a scope, parameters, an origin (file, editor or extension). The core calls **veto points**
   (can attack, trade, whisper, party, inspect, enter, leave, equip, use a skill, chat, store an item, use an item; each answers allow
   or deny with the text the player sees) and **modifier points** (exp gained, kill credit, damage, crit chance, enchant result, stat
   formula, kill-count multiplier), in a stated order. Rules come from the layers of decision 5. `reload` re-reads them live.
3. **Hooks in Rust, then Lua.** The same points plus events (kill, boss kill, login, logout, map enter, item use, chat, timer,
   command) offered to extensions written in Rust (a trait, compiled in behind a cargo feature and a run-time flag) and, the same
   shape, to Lua scripts with a small, versioned API (give an item, add a buff, send a notice, teleport, per-character stored values).
4. **Client.** Rows go to the client as data patches; features that need the client's code go into `fixes.dll` as named features; the
   manifest says which an extension needs, and the client can tell the server which features it has so an extension falls back for an
   unmodified client (entry 11 c).

Examples to base the plugin shape on: TrinityCore's script hooks, Eluna (Lua on a WoW core), The Forgotten Server's data folder with
XML and Lua, Bukkit/Spigot (events plus a manifest). Eluna and The Forgotten Server are the closest.

The reference set the first specs must prove (decision 8): rules: *no exp or kills in a map* and *level-gap protection*; modifier:
*enchant jumps*; item effect: *level-up stone* (the Kill Boost Stone moves onto the same registry); events/Lua: *kill streak*;
extension: *pets*.

## 6. The chunks

Each chunk is specified, then built, one after the other, and each ends in something that runs and is tested. Order and
dependencies:

```
client research batch (entries 8 to 12; the client side, any time)
        |
chunk 1 foundation ---- chunk 2 rules and hooks ---- chunk 3 item effects and skill abilities
                                  |                                   |
                                  +---- chunk 4 pets  (needs entries 9, 10)
                                  +---- chunk 5 Lua   (needs chunk 2)
chunk 6 cross-faction and client-bound rules (needs entry 12 and a DLL feature)
chunk 7 catalogue waves R1 to R4 (after the maintainer's design pass; needs chunks 2, 3, 5)
chunk 8 author kit and release packaging (after chunk 5)
chunk 9 per-player loot (its own spec; any time after chunk 2)
```

| # | Chunk | Done when | Needs from the client side |
|---|---|---|---|
| 1 | **Foundation**: the database as the source of truth (import once, `origin`, client tables, patch generation from the database alone), extension folders and manifest, the id-range registry, install/remove/enable, the Kill Boost Stone moved in as the first content pack, config layering | `additions.sql` and `client_rows.sql` are gone; patch 109 regenerates byte for byte from the database; an extension folder installs and removes cleanly | entry 11 (a), (d) |
| 2 | **Rules and hooks**: the scope model, the rules table, files and editor layering, `reload`, the veto and modifier points in the core, the reference rules and the enchant modifier, the `GAME_*` one-off flags folded in as rules | the two reference rules work from a file and from the editor, undo and reload included; the old flags still work as before | entry 12 (informs the veto messages) |
| 3 | **Item effects and skill abilities**: a registry for item-use effects and skill ability types, the level-up stone, the Kill Boost Stone ported, custom GM status levels | a new effect is added by a row and a handler, with its client rows generated | entry 11 (a) |
| 4 | **Pets**: the extension: capture, egg, summon, follow, auto-loot, the (possible) menu, the small size by whatever entry 9 finds | catch, summon, follow, loot, dismiss, relog and trade of the egg work with one real client | entries 9, 10 |
| 5 | **Lua**: the event hooks to Lua, the API, the sandbox, hot reload, per-character storage, the kill streak script | a script dropped in a folder runs and reloads without a restart; a failing script cannot hurt the server | entry 11 (c), 12 (g) |
| 6 | **Cross-faction and client-bound rules**: trade, whisper, inspect, party, chat combining, GMs seeing both sides, FFA, with the `fixes.dll` feature(s) the research says are needed | the listed actions work between factions in the maps the rules allow | entry 12 |
| 7 | **Catalogue waves** R1 (PvP and kills), R2 (access and restrictions), R3 (numbers), R4 (event features): each a handful of rule types or scripts from section 4, chosen by the maintainer's design pass | each wave's rules work and are documented | as marked |
| 8 | **Author kit**: the guide to writing an extension, an example extension, `ext check` (manifest, id ranges, client requirements), packaging for release, the conversion tool's tie-in | an outsider can write and install an extension from the guide alone | |
| 9 | **Per-player loot**: eligible-players-only ground items, personal rolls for the party, gold, bosses and chests, replacing the 30-second owner rule | a party gets individual drops; non-eligible players never see the item | entry 10 (c), (d) |

Each spec also (a) settles with the maintainer every choice that changes behaviour, (b) is written as
`docs/plans/ext-<n>-<name>.md` with a Build section concrete enough to build from, with the tests and the live-test items it adds, and
(c) names the client research it needs.

Conventions for every chunk: after code changes the dev game server is rebuilt and restarted; what only a real client can show goes into
`docs/testing/live-test-checklist.md` as new items; `docs/client/` changes are committed too, but the client itself is never worked on
here; `README.md` (feature list, test count) and `docs/plans/roadmap.md` are updated.

## 7. The work, chunk by chunk

Each chunk is a spec, then a build of that spec. A spec reads the inputs named, settles the points listed with the maintainer, and writes
the file named, with a Build section (files, schema, commands, tests, live-test items). The build follows that section in its order,
running the tests after each step; existing behaviour must not change unless the chunk says so. If the spec and the code disagree, the
build stops and the question goes to the maintainer.

### 7.1 Chunk 1, foundation

Spec: `docs/plans/ext-1-foundation.md` (written). Build: its Build section. Done when: patch 109 regenerates byte for byte from the
database alone and the Kill Boost Stone works as before.

### 7.2 Chunk 2, rules and hooks

**Needs**: chunk 1 built. **Inputs**: this plan, `docs/plans/ext-1-foundation.md`, `crates/game-server/src/config.rs` and `switches.rs`,
`docs/original/operator-console.md`, `docs/plans/done/data-editing.md` (the edits layer and the registry the rules plug into), and the
code and documents of the places the reference rules touch: kills and experience (`docs/original/combat.md`, `pvp.rs`, `server/pvp.rs`,
`kc.rs`), enchanting (`lapis.rs`, `docs/original/lapis.md`), trade (`trade.rs`, `GAME_CROSS_FACTION_TRADE_MAPS`), `GAME_MARKET_MAPS`.

**The spec settles**:
1. the scope model (maps, level range, kill-count range, faction, an optional time window; how scopes combine; a way to say "everything
   except");
2. the rules table, its origin layers and precedence, a plain-text file format for operators who edit files (comments, validation
   errors that name the line, a `check` command), the editor registry entry, `reload`;
3. the veto points and modifier points to add to the core now, with the exact call sites found by reading the code (and which are left
   for the waves), their order, the message the player sees and the log line;
4. the reference rules (no experience or kills in a map; level-gap protection) and the enchant-jump modifier;
5. folding the existing `GAME_*` one-off flags in as rules without changing behaviour (the old variables keep working);
6. cost: the veto points are on hot paths (attack, move, chat); how they stay cheap.

**Output**: `docs/plans/ext-2-rules.md`, with tests (a rule per kind, scope combinations, layering, reload). **Done when** (the build):
the whole test suite passes and the old `GAME_*` variables work as before.

### 7.3 Chunk 3, item effects and skill abilities

**Needs**: chunks 1 and 2 built. **Inputs**: this plan, `docs/plans/ext-1-foundation.md`, `docs/plans/ext-2-rules.md`,
`docs/original/item-use.md`, `item_use.rs`, `docs/original/skills.md`, `docs/original/skills-buffs.md`, `buffs.rs`,
`docs/original/operator-console.md` decision 3 (the Kill Boost Stone), `docs/plans/done/client-patches.md`.

**The spec settles**: a registry of item-use effects and of skill ability types that an extension adds a row and a handler to (what the
row holds, how an item in `defs.items` names its effect and parameters, how a skill's ability type is looked up, validation at load);
the level-up stone (+1, or to a level) as the reference; the Kill Boost Stone ported onto it without changing behaviour; custom GM
status levels (replacing the fixed 16/32/48/64 thresholds; what the client checks is in `docs/client/gm-and-commands.md`); the client
rows each new item and skill needs, generated from the database.

**Output**: `docs/plans/ext-3-effects.md`. **Done when** (the build): the Kill Boost Stone behaves as before and patch 109's data still
regenerates unchanged.

### 7.4 Chunk 4, pets

**Needs**: chunks 1 to 3 built and client research entries 9 and 10 answered. **Inputs**: this plan (decision 1 is the pet design),
`docs/plans/ext-1-foundation.md` to `docs/plans/ext-3-effects.md`, the client answers to entries 9 and 10 (`docs/client/`),
`docs/original/mobs.md`, `mob-fight.md`, `pathing.md`, `item-drop.md`, `docs/original/combat.md` sections 7 and 8 (drops, pick-up),
`mobs.rs`, `mob_ai.rs`, `zone.rs` and `zone/`, `drops.rs`, `inventory.rs`, `item_use.rs`.

**The spec settles**, pets as an extension:
1. capture: the item, how the server learns the target (entry 10 a), the conditions (not a boss, not a quest monster, aggro'd on the
   player, HP below a threshold, the chance curve), what the monster does on capture (removed without experience or drops, respawn as
   usual);
2. the Pet Egg item: how it carries the monster id (entry 10 b), what the tooltip shows, trading, the warehouse, the guild bank;
3. summon and dismiss (item use, logout, death, map change, zone change, instances, the vehicle/mount state), one pet per player;
4. the pet as a mob: its object, the follow behaviour and speed, staying out of combat, never targeted or aggro'd, never counted by
   spawn areas, what other players see;
5. the size and look by what entry 9 found (a `Monster.SData` record per pet monster, generated from the database, or the DLL), its name;
6. auto-loot: the radius, only its owner's drops, the walk to the item, the pick-up into the bag with the normal message and a full bag;
7. persistence (the active pet across relogs), the menu if entry 10 e allows one;
8. rules from chunk 2 that apply (maps where pets or capture are off) and their reference in the rules file.
Where the client answer is "cannot", the spec proposes the fallback.

**Output**: `docs/plans/ext-4-pets.md`, with the tests and the live-test items (what one real client must show). **Done when** (the
build): the pets live in their own extension folder and behind their own flag, and with the flag off the server behaves exactly as
before; the client rows and the DLL feature request go to `docs/testing/client-research-queue.md` or the hand-over files the spec names.

### 7.5 Chunk 5, Lua

**Needs**: chunk 2 built. **Inputs**: this plan, `docs/plans/ext-1-foundation.md`, `docs/plans/ext-2-rules.md`, the way the server runs the
original's monster scripts (`mob_lua.rs`, `zone/lua.rs`, `docs/original/instances.md` sections 9 to 15), the `mlua` version and features in
`crates/game-server/Cargo.toml`, and entries 11 c and 12 g of `docs/testing/client-research-queue.md` with their answers.

**The spec settles**, scripting for extensions:
1. the events (kill, boss kill, login, logout, map enter and leave, item use, chat, command, timer) and which can veto or change a
   value, in the same shape as the Rust hook points of chunk 2;
2. the API (give or take an item, gold, buff, notice to one player or everyone, teleport, read a player's level/faction/map/kills, run a
   custom chat command, per-character stored values in the database) with names that stay stable, and an API version in each
   extension's manifest;
3. the sandbox: no file or process access, instruction and time limits, a memory limit, what a failing or slow script does to the server
   (disable that script, log, never block a zone), the original's monster Lua kept separate;
4. loading, hot reload (`reload`), per-extension folders;
5. the kill streak script as the reference with its rules-file settings.

**Output**: `docs/plans/ext-5-lua.md`, with tests (a script per event, the limits, a crashing script). **Done when** (the build): the
original's monster Lua behaves as before.

### 7.6 Chunk 6, cross-faction and client-bound rules

**Needs**: chunk 2 built and the client answer to research entry 12 (if it is missing, the client research comes first).
**Inputs**: this plan, `docs/plans/ext-2-rules.md`, the client answer to entry 12 (`docs/client/`), `docs/original/party.md`, `friends.md`,
`trade.md`, `union.md`, `pvp.md`, the chat code, `GAME_CROSS_FACTION_TRADE_MAPS`.

**The spec settles**: for each of trade, whisper, inspect, party, chat combining, GMs seeing both sides' chat and attacking both sides,
and free-for-all maps: what the server must change, what the client refuses by itself, and which `fixes.dll` feature (named, with its
settings and the server-side handshake that tells the server the client has it) removes the client's refusal; the rule types and their
scopes (maps, factions, levels); what an unmodified client sees and the fallback; the hand-over file for the client side (what each DLL
feature must do, with the addresses entry 12 found).

**Output**: `docs/plans/ext-6-cross-faction.md` with a Build section for the server side, a client-side section, and the live-test items.
**Done when** (the server build): with no rule set the server behaves as before; the DLL features belong to the client side, so what needs
them is tried live only once they exist.

### 7.7 Chunk 7, the catalogue waves (after the maintainer's design pass)

The design pass first: go through section 4 with the maintainer, mark what their server uses, and put the answers in
`docs/plans/ext-catalogue.md` (what, which scope, which defaults). Then one wave at a time: R1 PvP and kills, R2 access and
restrictions, R3 numbers, R4 event features.

**Needs**: chunks 1 to 3 built and, for scripts, chunk 5. **Inputs**: this plan, `docs/plans/ext-catalogue.md`,
`docs/plans/ext-1-foundation.md` to `docs/plans/ext-3-effects.md`, `docs/plans/ext-5-lua.md`.

**The spec settles**, for every item the maintainer chose in the wave: the rule type or script, its scope and parameters, the call site
in the code (read it), the message the player sees, the defaults, the rules-file lines that turn it on, tests, and what needs the client
(research entries 9 to 12, a DLL feature) so it can be left for the client side. Items the existing hooks cannot express are listed with
the hook they need.

**Output**: `docs/plans/ext-7-<wave>.md`. **Done when** (the build): every rule is off until the rules file turns it on.

### 7.8 Chunk 8, the author kit

**Needs**: chunk 5 built. **Inputs**: this plan, `docs/plans/ext-1-foundation.md` to `ext-5-lua.md`, `docs/server/deploy.md`,
`docs/plans/roadmap.md` ("Later, for other operators"), `tools/`.

**The spec settles**, so that other people write extensions and convert their original servers: the guide an outsider needs to write an
extension from nothing (the folder, the manifest, data rows with client rows, a rules file, a Lua script, a Rust extension, testing it
against a dev database, packaging it); a worked example extension kept in the repository and tested by the suite; `ext check` (manifest,
id ranges and clashes, client requirements, API version, the files the extension adds); how an operator installs, updates and removes
extensions in a released build (no git, no Python, only the binaries and a database); versioning promises for the Lua API and the
manifest; and how the conversion tool for an existing original server (roadmap) installs the original's edits as an extension.

**Output**: `docs/plans/ext-8-author-kit.md`. **Done when** (the build): an outsider can write and install an extension from the guide
alone.

### 7.9 Chunk 9, per-player loot

**Needs**: chunk 2 built and the answers to entry 10 c and d. **Inputs**: this plan (decision 2 and chunk 9),
`docs/plans/ext-2-rules.md`, the client answers to entry 10 c and d, `docs/original/item-drop.md`, `docs/original/combat.md` sections 7
and 8, `docs/original/quest-drops.md`, `drops.rs`, `party.rs`, `docs/plans/done/data-editing.md` (the drop tables the editor changes).

**The spec settles**, per-player loot in the manner of WoW: each eligible player (the killer; each party member in range; who else) gets
their own rolls of the monster's drops and their own gold, shown to them only (the ground item sent to its eligible players, no one
else), picked up by them only, expiring on its own timer; what it replaces of the 30-second owner rule and the party/guild share rules;
bosses, chests, obelisk guards and quest-item drops; fairness and abuse (leaving the party to double-dip, multiboxing, the damage
threshold for eligibility); how it appears as a rule with scopes (per map, per monster type, per level range); the pet auto-loot
interaction; the cost on a crowded boss; the data editor's drop tables unchanged.

**Output**: `docs/plans/ext-9-personal-loot.md`. **Done when** (the build): with the rule off the drops behave exactly as before.
