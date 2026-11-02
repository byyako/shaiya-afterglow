# Build plan: the boss spawns of `Obelisk.ini` and the `/mera` fix (roads left as shipped)

Built; the behaviour is described in `docs/original/obelisks.md` sections 11.3 and 11.5 (decisions in section 11.7). The live test is
section **47** of `docs/testing/live-test-checklist.md`.

Read section 11 in full first, then sections 1 to 10 for the obelisks as built, and the code named below.

**Decided (2026-10-20, section 11.7)**: map 0's four exits **stay as shipped** (each open to its own faction, no road links): the
road code of 11.2 is **not built**, the importer keeps reading the shipped `<data>/Obelisk.ini` (no repository copy), deviations 3 to
6 are withdrawn. **Deviation 7** (boss followers count the ones really there) is approved; **8 is not taken** (every chain starts
afresh at each start, as the original). An earlier version of this plan built design B's roads; it is superseded.

Names below are functions and types, not line numbers (the files move).

## What gets built

| part | spec | where |
|---|---|---|
| The importer reads the 25 `[bothmob_…]` groups | 11.1 | `tools/world/import.py`, `db/world/schema.sql` |
| Loading and setting them up | 11.1 | `pg.rs`, `world.rs`, new `boss.rs` (pure) |
| Boss chains at run time, with their portals | 11.3, deviation 7 | `boss.rs`, new `zone/boss.rs`, `mobs.rs`, `zone.rs`, `zone/instance.rs`, `server.rs` |
| The `/mera` fix on obelisks (and `/mera` on bosses) | 11.5, 11.3 | `zone/admin.rs`, `zone/obelisk.rs` |

Not built, by decision: portal links (11.2). With no link `EnableMoveByPortal` allows every move, which is what the server does now.

## Steps (in this order; `cargo test -p game-server` green after each)

### 1. The importer and the `world` schema

`tools/world/import.py` `read_obelisks` (still reading `<data>/Obelisk.ini`) also returns the boss groups. State handling as now
(states 0, 1, 3, 4; heads without regard to case; the `strtok` delimiters; `atol`/`atof`; `[Portal_…]` with `[light]`/`[fury]` still
read and dropped), plus, as `CMap::LoadObelisk` reads them (11.1):

- **`[bothmob_n, (map, stages, respawn, (pmap, portal))] = (x,y,z),(id1, …)`** → a boss group: number n, map, stages cut to 15, the
  respawn time in seconds, the portal's map and id, the spot, and exactly `stages` monster ids (fewer: the group is dropped, with a
  printed warning).
- **`[Mob] = (x,y,z,r),(id1, …, id stages+1), n`** in state 4 after a group → a follower group: centre, radius, one monster per stage
  and one more (the chest monster; 0 = none), n cut to 16; at most 32 per group; a short line dropped (warning). `[Mob(32개이하)]`
  and the like do not match (`[mob]` must be followed by `]`).
- **`[RandomRespawn] = (min, max),(x,y,z),(x,y,z),(x,y,z)`** in state 4 → the last group is random, with min and max minutes and three
  spots.

`db/world/schema.sql` (comments in the style of the obelisk tables; imported as written, the server applies the rules):

- `world.boss_groups (seq integer PRIMARY KEY, number integer, map integer, stages integer, respawn_secs integer, portal_map integer,
  portal_id integer, pos_x real, pos_y real, pos_z real, mobs integer[], random_min integer, random_max integer, random_spots real[])`:
  file order in `seq`; `random_*` NULL for a fixed group, `random_spots` nine values.
- `world.boss_followers (group_seq integer REFERENCES world.boss_groups, seq integer, pos_x real, pos_y real, pos_z real, radius real,
  mobs integer[], count integer, PRIMARY KEY (group_seq, seq))`.

The summary line prints the groups and follower lines. Expected: 9 obelisks (unchanged), **25 boss groups** as in 11.4's table (group
001: 16 follower lines and 16 chest lines; 1984's lines imported, it never counts: level and HP 0 in `defs.mobs`). Run
`import.py <data> --dry-run` first and compare; then the real import (it replaces the `world` schema; the server reads it at start).

### 2. Loading (pure set-up, unit-tested)

- `pg.rs`: `load_boss_groups` (like `load_obelisks`) returns the rows; `World::set_boss_groups` (in `main.rs` next to `set_obelisks`)
  sets them up as the original does (`boss.rs`): every id looked up, a monster counting as for obelisks (`wID`, `wLevel`, `dwHP` not
  0); the portal resolves when its id is **at least 1001** and `pmap` has a portal whose map-file country is that id (a portal's id is
  its country when above 1000, `docs/original/portals.md` section 1: add `Portal::id()` returning it, `None` otherwise); else no portal,
  and the group still runs. A group runs only when its first stage's monster counts. Kept per map.
- Info log at start: `"boss groups loaded"` (groups per map, how many have a portal).

### 3. Boss chains (`boss.rs`, pure; `zone/boss.rs`)

`crates/game-server/src/boss.rs` (pure, unit-tested; add to `lib.rs`):

- `BossGroup`: number, map, the stages' monsters (`Option<u16>` each), the header respawn, the portal (`Option<(place, id)>`), the spot,
  `random: Option<(min, max, [spots; 3])>`, followers (`centre, radius, spread = 2 × trunc(radius), count, a monster per stage,
  chest`).
- `Chain` per group and zone: `stage`; `phase` as obelisks (`Due(Option<Instant>)`, `Standing(id)`, `Fallen(id)`) plus a `removed`
  flag; per follower group `alive`, `current`, `due`, `generation` (**deviation 7**: a group is full when its list is; a blocked spot
  is tried again on the next pass; an older body no longer counts, as `obelisk::Guards`); the chest ids.
- `destroy(&mut self) -> Destroy { followers: Vec<u32>, ended: bool }` (`BothMobDestroy`): every follower goes; stage + 1; past the
  last stage → stage 0 and `ended` (the portal opens, the chests come).
- Due times: a boss's body → now + its monster's respawn time (the code table, `docs/original/mobs.md` section 1); then, when the chain
  just ended and the boss was not removed, a fixed group → now + **header seconds + (rand % 120 − 60) minutes**, a random group → now
  + **min minutes + (rand % (max × 60)) seconds**. At set-up a fixed group is due at once, a random group by the random rule.
- A follower's spot: the formula of `GuardGroup::spot`; a random group's boss spot: `spots[rand % 3]`.

Zone (`zone/boss.rs`, mirrored on `zone/obelisk.rs`):

- `mobs.rs`: `MobMap` gets **`bosses: Vec<Chain>`**; `Origin` gets **`Boss { group, generation }`**, **`Follower { group, set,
  generation }`**, **`Chest { group, generation }`** (entry `usize::MAX`, as obelisks).
- **Every zone of the map** gets the chains (11.1: static and instance zones alike): one helper `Zones::new_mob_map(map, world, defs)`
  used by `Zones::new` (static maps; also make a static `MobMap` for a static map with boss groups and no spawn areas) and by
  `enter_instance`, `enter_guild_house` and `enter_ranking` (`zone/instance.rs`) instead of their `MobMap::new(world.spawn_areas_of(map),
  defs)`.
- **Respawn pass** (`Zones::respawn`, after `obelisk_respawn`): `boss_respawn` (`UpdateBothMobRespawn`). No boss standing and due →
  `BothMobRespawn`: at stage 0 first **the portal shuts** (country 3) and every follower and chest still there goes (`0x0602` to their
  view); then the stage's boss at its spot (no check of the spot, as obelisks) and each follower group with a monster for this stage
  filled (one try per follower at a random spot; blocked: tried again next pass). Boss standing → each follower group below its count
  and due gets one, due again after **that monster's** respawn time.
- **Death** (where `obelisk_death` is called: the kill path in `zone.rs`, the guards' kills in `zone/npc.rs`): a boss → `destroy`
  (killer or not); `ended` → **the portal opens** (country 0) and every follower group with a chest monster gets `count` chests at
  random spots (never respawned); a follower leaves its list. Experience and drops as for any monster.
- **Bodies** (`remove_bodies`, next to `obelisk_body`): a boss → the due times above; a follower counts its group down (due after its
  respawn time when the group was full, with `respawn_due`'s hour spread from code 9 on), only for its generation; a chest → nothing.
- A boss removed by a Lua script's `LuaDeleteMob` (`MobRemove2`): treated as a removal (not verified; no shipped script deletes a
  group's boss: 118 is 2469's own `OnDeath`, which makes 2470).
- Info logs: `"boss rose"` (zone, group, stage, monster), `"boss fell"` (zone, group, stage, ended), `"boss due"` (zone, group, in how
  long).

**The portals** (11.3: `stMapPortal::nCountry` and the zone's pair). `World` is read only, so:

- **Static maps**: add to `Zones` a `Mutex<HashMap<(map, place), u32>>` of portal countries that override the map file's (missing =
  the file's). Lock order `maps` → `mobs` → this. A chain's open/shut on a static map writes it: **the whole server** sees the door
  open until the chain restarts (11.3). `portal_use` (`server.rs`) reads `ctx.zones.portal_country(map, index)` instead of
  `portal.country` on a map without a `CreateType`.
- **Maps with a `CreateType`**: the zone's pair, `set_instance_portal(zone, id, 0 or 3)` (`UpdateInsZonePortalCountry`), which
  `portal_use` already reads.
- The shipped groups: maps 14, 17, 19, 20, 48, 49 static (portals 1001–1007; maps 48 and 49 become reachable through map 19 portal 8
  and map 20 portal 7), map 50 (guild ranking, create type 5) the zones' pairs 1001–1003, map 67 static (2469, no portal), map 64
  (create type 3) group 024 without a portal, groups 004 and 006 naming a portal that does not exist.

### 4. The `/mera` fix (11.5; the original, no deviation)

`zone/admin.rs` `mob_remove_near` (`/mera`, `CZone::MobRemove`) and `mob_remove_id` (`/mera t`, `MobRemoveByID`) take the `maps` lock
first (order `maps` → `mobs`). For each monster removed, by its `Origin`:

- **An obelisk** standing (`Phase::Standing(id)`): `Phase::Fallen(id)`, then the fall with **its current country** (`/mera`) or **its
  base country** (`/mera t`) as a **removal**: guards removed at once, `0x2102` to the map, no kill count, no `0x2103`; the owner saved
  when it changed (only `/mera t` on a captured one). Then `kill_quietly` as now: the body pass's `obelisk_body` makes it **due after
  its monster's respawn time** (3 min), so it rises again for that owner with its guards.
- **A guard**: leaves its group's `alive` list; `kill_quietly` (its body counts the group down, as now).
- **A boss**: `destroy` at once (followers gone, stage + 1; at the last stage the portal opens and the chests come), and `removed` set
  so its body applies only the monster's own respawn time (11.3: a removed boss has no body, so no chain-end time). **A follower**
  leaves its list. Then `kill_quietly`.

`obelisk_fall`'s `quiet: bool` becomes a three-way kind: **kill** (all of section 4), **removal** (`0x2102`, the owner's save when
changed; no count, no `0x2103`), **reset** (nothing told or saved here: `reset_obelisks` sends and saves itself, as now).

### 5. Docs and the test list

- `docs/original/obelisks.md`: the header's "Not built" list and section 10 say what is built (boss groups, `/mera`) and what is not
  by decision (portal links, 11.7); 11.3 and 11.5 marked built.
- `README.md`: the boss spawns and the `/mera` fix move from "Not built" to "Built, not yet tested"; obelisk portal links stay under
  "Not built" with "(left as shipped by decision, `docs/original/obelisks.md` 11.7)".
- `docs/original/instances.md` sections 7 and 16, `docs/original/guild-ranking.md` section 5: the boss groups are built (map 67's 2469
  now stands; the ranking zones' bosses open their portals).
- `docs/original/gm-commands.md` section 7.3: `/mera` and `/mera t` on an obelisk and a boss as 11.5 and 11.3.

### 6. Finish

`cargo clippy` clean as the rest of the crate, `cargo test` (all, with the PostgreSQL tests), the import (step 1), a release build; the
start lines ("obelisks loaded", "boss groups loaded") show the data.

## Tests

Unit (pure):

- **boss.rs**: set-up from rows (a group whose first stage does not count is not run; a portal id below 1001 or missing → none);
  a two-stage chain (group 001's shape): due at once; the first boss dies → stage 1, followers removed; the second dies → `ended`,
  stage 0; due times (fixed: header ± 60 min from `rand`; random: min + below max × 60 s; removed: its own respawn only); random spots
  by `rand % 3`; follower counting by generation (deviation 7).
- **Importer**: the shipped file gives 9 obelisks and 25 boss groups (`--dry-run` counts checked in the session); a group with too few
  ids dropped; `[Mob(…)]` lines ignored; `[RandomRespawn]` on group 024.

Zone (`zone.rs` tests module, as the obelisk tests there):

- A boss group on a static map: the boss and its followers spawn at the first pass; killing the last stage → the static portal's
  country 0 (`portal_use` lets a player through), the chests stand; the next rising shuts it (3) and removes the chests.
- The same group in an instance zone (create type 5 or 3): the zone's pair changes, the static override does not.
- Followers: a dead one comes back after its respawn time while the boss stands; none while it is down.
- `/mera` on an obelisk: guards gone, `0x2102` to the map, it rises again 3 min later (the body pass, then the respawn pass with the
  time advanced) for the same owner; `/mera t` on a captured one: back to its base, the owner's save queued.
- `/mera` on a boss: followers gone, stage + 1, due after its own respawn time even when it was the last stage (and the portal open).

## Live test

Section 47 of `docs/testing/live-test-checklist.md`.
