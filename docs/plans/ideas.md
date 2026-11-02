# Ideas and discussions parked

Things the maintainer raised (2026-10-26) that are not scheduled. Nothing here is decided unless it says so. Each entry says what is known, what is open,
and where the details are. The live plan is `docs/plans/roadmap.md`. An idea becomes a spec only after it has been discussed.

## 1. Decisions taken in this discussion

- **Live tests are deferred to the alpha** (roadmap A3). Only structural checks stay before launch: the backup restore drill, deploy and
  rollback, the wipe script, one login-to-world run on the VPS.
- **An own client is deferred, possibly dropped.** The DLL (`fixes.dll`, Rust) and the signed data patches may do everything wanted for
  now. Why not now: the original client is the oracle that tells whether the server is right, an own client would delay the alpha by
  months and confound bugs, and it is a larger project than the server was. If it is ever reopened: a staged track (an asset converter
  into open formats and our own archive, a protocol client library = the shelved bot, a viewer, then play), Rust on wgpu so it can reach
  Linux, Mac, 64-bit and a browser (a browser needs a WebSocket gateway on the server), art from the original `data.saf` or a converted
  archive. Needs a feasibility spec first. The wishes behind it: new features, cross-platform or web or 64-bit, freer UI and maps.

## 2. First extensions to keep in mind (frozen with phase 4 until after the alpha)

The maintainer's candidates, "spitballing" small features that keep players interested. They are not designs; they are here so the plugin
system's spec (`docs/plans/extensions.md`, `docs/plans/ext-1-foundation.md`) is checked against them.

- **Fishing**, **crafting**, **forging** (the maintainer is not sure what it means: upgrading gear with a risk, making gear from materials, or
  rerolling stats; to be asked again), **camping** (a placeable campfire that other players can sit at or pitch a tent by, healing and
  maybe a rested-exp bonus; not sure).
- **Monster madness**: many bosses spawned around the player for everyone, started by a GM command and by a distributable event item
  (both), on the normal drop tables ("not all drops are 100%", but see section 3: boss tables are mostly guaranteed). Open: where they
  spawn ("generally around the player", open to discussion), cooldown and one active event per map for the item, load.
- **What these ask of the plugin system**: per-character state and storage; timed sessions; a temporary-spawn API that always cleans
  up; a loot-source hook; item use that runs in one database transaction; per-player notices; a route for the web API (B1) to run an
  extension action; safety caps (spawn count, tick time); anti-bot and anti-alt-farming; a client handshake so the server knows whether
  the DLL overlay is present (research entry 11 c).
- **Order of ease**: personal loot and monster madness need no client change (existing packets); fishing, crafting and forging want
  windows (a DLL overlay, entry 16 f) and campfires want a placeable object (entry 15).
- **Client research** for all of it is queued as entries 15, 16 and 17 (and 9 to 12) in `docs/testing/client-research-queue.md`, answered in
  `docs/client/research-extension-features.md`.

## 3. Per-player (personal) loot: the numbers and the lean

Read from `defs.mob_items`, `defs.items`, `world.boss_groups` and `crates/game-server/src/drops.rs`.

- Today: one roll per kill (entries 1 to 6 in percent, 7 to 9 in millionths, each hit drops one random item of the entry's grade; gold
  30%); each item on the ground with an owner (top damage dealer or the party's turn) for 30 s, gone at 60 s. Quest items are already
  rolled per eligible player.
- Normal spawned monsters: 1,306 types, 0.56 expected items per kill on average (median 0.45); 118 types have a guaranteed entry.
- Bosses: 23 mobs in 25 respawn groups, mostly three **guaranteed** (100%) entries; fractional entries are rare (Crabby's rings 0.5%,
  lapis, the Lua boss's gems). Pools are large (72 to 84 items covering every class): maps 48 and 49 drop the top tier (levels 56 and
  59), Kloren and Fantesima on maps 14 and 17 level 50 to 54 armour. Most groups respawn weekly: natural maximum about **154**
  boss-table items a week (about 44 top tier, about 98 Kloren and Fantesima).
- Monster madness of 50 bosses with three guaranteed items each, 30 participants: 150 items with the original shared roll (a week of
  world supply), **4,500** if every participant rolls the full table, about 150 with each rate divided by the number of participants.
  Boss HP limits how many actually die.
- The maintainer's lean (2026-10-26): "option 2", personal rolls with the rate divided by the participants; open to discussing further.
  The caveat: for a guaranteed 100% entry that makes the drop a 3.3% roll each at 30 players, so 36% of kills drop nothing and 26% drop
  two or more. Better: guaranteed entries are **assigned** (one item per entry per kill, to one eligible player by damage-weighted
  chance, visible only to that player), fractional entries are rolled personally with the rate divided by the eligible players, gold is
  split not multiplied, and events get a loot scale (default 1) and a per-player cap, optionally their own table.
- Open: eligibility (any hit, a damage threshold, party and union members in range); class-aware filtering (the pools are not, so it
  would multiply the effective supply per class); whether it is a per-scope rule (my suggestion: map, mob type, level range, default
  off in the original maps) or global. The design depends on the client answer (entry 17 e = 10 c and d): does it accept a ground item
  that only some players received? The spec comes after the answers are in (chunk 9 of the extensions,
  `docs/plans/ext-9-personal-loot.md`).

## 4. Balance project (analysis done; plan `docs/plans/balance/plan.md`)

The maintainer wants to look into changing and balancing items, armour, weapons, lapis, skills and stats across all classes and both
factions. A three-part analysis (how stats, skills and damage work and a validated model; the inventory of every item and
skill with outliers; the class-by-class comparison with the levers) came first, analysis only (`docs/plans/balance/`). The changes follow
as a separate plan and a build. Constraint to remember: the client holds its own copy of items and skills (tooltips), so a number reaches players
only through a signed client data patch (`docs/plans/done/client-patches.md`); server-only formulas can change with the server alone. Not
scheduled; the alpha's numbers will help.

## 5. Using this server with existing files: to be discussed in full later

The maintainer wants this on the list so it is not forgotten, and wants it fully discussed **before** anything is specified: how people
who run the original files could use this open-source server with what they already have, for example importing their database and
the other required information (definitions, world files, accounts and characters, their own edits, their client). The first notes are
in `docs/plans/roadmap.md` ("Later, for other operators": converting an existing original server; `tools/defs/` and `tools/world/import.py`
exist as dev tools, `docs/original/database-notes.md` maps the tables, the data editor carries later edits). Open for the discussion: what to
import and what to refuse, how their custom client or patcher fits, a dry-run report of what cannot be carried over, support burden,
and the order against the alpha and the plugin system. The spec comes after the discussion.
