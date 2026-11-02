# Data editing (spec and build plan)

Built (parts A and B); the console commands are described in `docs/original/operator-console.md`, the rest in this plan's "as built" sections.

The operator of a Shaiya Afterglow server wants to change the game's data without writing SQL: pick a map, see the monsters spawned on it and
what each drops, and change it; move or refill spawn areas; change obelisks, boss groups and where portals lead; tune monsters. Edits
must survive a fresh import of the original files, be reviewable and undoable, and work for **other people hosting the server**, who
have the binaries and a database but no git and no Python. This plan adds a third program, `data-editor`, with a web page and a drawn
map, an `edits` schema that holds every change with its history, and two console commands in the game server (`reload`, `restart`).

What the data looks like at the time of writing: 68 maps, 11,496 spawn areas holding 16,720 spawn entries, 1,306 different monsters
spawned (95 of them on more than one map), 2,747 monster definitions with 9 drop entries each (`defs.mob_items`), items in 294 drop
grades, 9 obelisks with 160 guard groups, 25 boss groups, 255 portals, 1,774 NPC placements. Background: `docs/plans/done/client-patches.md`
section 8 (which data has a client copy), `docs/original/combat.md` section 7 (the drop roll), `docs/original/mobs.md` (spawning,
respawn codes), `docs/original/obelisks.md`, `docs/original/portals.md`, `docs/original/operator-console.md` (the console and its
`NOTIFY` channel).

## 1. The maintainer's decisions (2026-10-22)

1. **Drops stay per monster**, as in the original: the page starts from a map, but a monster's drops are the monster's everywhere.
   The page says so whenever the monster is on other maps ("also spawns on maps 2, 5").
2. **Grades only** (no new kind of drop entry, no server change to the drop roll): a drop entry names a grade and a chance; one
   random item of the grade drops. The page shows each grade's items and who uses the grade, and offers a **"drop this one item"
   helper**: the item moves into a new grade of its own and the monster gets an entry with that grade. The page warns first that the
   item leaves its old grade, listing the monsters, box sets and sequence groups that stop giving it.
3. **A web page with a drawn map**: tables plus a top-down picture of each map with spawn boxes, NPCs, portals, obelisks, guard and boss
   spots drawn on a grid, clickable, spawn boxes and spots movable by dragging. The server has no terrain picture (its blocked-cell layer
   is 0.3 % filled on field map 2, though 81 % on dungeon map 65, where it outlines the corridors), so a **background picture per map is optional**: uploaded by the operator, and gathered from
   the extracted client by the client side (section 8).
4. **Scope of the first build**: drops and gold (monster drop entries, item grades, gold amounts), spawns (areas, their monsters and
   counts, adding and removing areas), monster stats (everything the client does not show), obelisks and their guards, boss groups and
   their followers, portals (where they lead, who may pass, level limits; not their position).
5. **Picking up changes**: a **restart button** (players online are shown first; the console's countdown with notices) **and a live
   reload** for what can safely change while running: drops, gold and monster stats. Spawns, obelisks, bosses and portals need the
   restart.
6. **Its own program**, `data-editor`, a Rust binary of this workspace shipped like `login-server` and `game-server`. It needs only the
   database, so it runs on the server machine or anywhere that reaches the database. It talks to game servers through the database
   (the console's `NOTIFY` channel, as the `console` tool does), so one editor serves every game server sharing the database.
7. **No git needed**: each Apply is a named **change set** stored in the database (who, when, the message, every value before and
   after), with a history and an Undo for any change set; plus **Export and Import** of all edits as one file (backup, moving edits to
   another server, sharing). On this dev box the export file may be kept in the repo (`EDITOR_EXPORT_FILE`, section 5).
8. **Access, for now, like the live-test page**: no login; only loopback, private and Tailscale addresses may connect. Later: sign in
   with a GM account of the accounts table (section 10).
9. **Later, on the roadmap**: converting an existing original server (its databases and files) to Shaiya Afterglow (`docs/plans/roadmap.md`).

## 2. What can be edited

The editor knows a fixed list of **targets** (the registry, `crates/data-editor/src/registry.rs`). Each names the table, the key, the
fields that may change and their rules, the fields that may not and why, whether rows can be added or removed, and when a change takes
effect: **live** (the console's `reload`), **next spawn** (a monster spawned after the reload gets it; living ones keep theirs) or
**restart**.

| target | table | key (as text) | editable | refused | add / remove | takes effect |
|---|---|---|---|---|---|---|
| `mob` | `defs.mobs` | `mobid` (`2133`) | every column but the refused ones: gold (`money1`, `money2`), `questitemid`, HP/SP/MP, stats, defence, resistances, movement, the three attacks, `ai`, respawn (`attackspecial3`, shown as its time) | `rowid`, `mobid`; `mobname` and `level` (the client shows them from `Monster.SData`: a client patch, not built for monsters) | no (a new monster needs a client record) | live, next spawn or restart per field (the table below) |
| `mob_drop` | `defs.mob_items` | `mobid/itemorder` (`2133/4`) | `grade` (0 = entry off, 1 to 1024), `droprate` (entries 1 to 6: 0 to 100, a percent; 7 to 9: 0 to 1,000,000, per million) | `itemorder` 0 (never rolled) | no (every monster has entries 1 to 9) | live |
| `item_grade` | `defs.items` | `itemid` (`30071`) | `grade` (0 to 1024) | every other column (the client copy: `docs/plans/done/client-patches.md`) | no | live |
| `spawn_area` | `world.mob_areas` | `map/seq` (`2/57`) | `x1, y1, z1, x2, y2, z2` (x and z inside the map, `x1 < x2`, `z1 < z2`) | | add (seq from 100,000), remove (with its monsters) | restart |
| `spawn_mob` | `world.mob_area_mobs` | `map/area seq/seq` (`2/57/1`) | `mob_id` (a monster that can spawn: id, HP and level not 0, as `MobDefs::get`), `count` (1 to 100; the original files use at most 8) | | add (seq from 1,000 in an original area, from 1 in an added one), remove | restart |
| `portal` | `world.map_portals` | `map/seq` (`47/6`) | `country` (0, 1, 2; a value above 1000 is an instance portal's id and stays), `level_min`, `level_max` (both 0 = no limit, else min ≤ max), `to_map` (a map of `world.maps`), `to_x, to_y, to_z` (inside that map) | `pos_x, pos_y, pos_z` (the client draws and detects portals from its own `.wld` files, `docs/client/CLIENT-FACTS.md`, Cailum Sacra); `seq` (the client names a portal by its index) | no (same reason) | restart |
| `obelisk` | `world.obelisks` | `seq` | position (inside the map), `mob_neutral, mob_light, mob_fury` (monsters that can spawn), `country` (0 to 2 as the file numbers it) | `seq`, `map`, `number` (the owners table and the client's list key on them) | no | restart |
| `obelisk_guard` | `world.obelisk_guards` | `obelisk seq/seq` | position, `radius` (> 0), the three monsters, `count` (1 to 100) | | add, remove | restart |
| `boss_group` | `world.boss_groups` | `seq` | `respawn_secs` (≥ 0), boss spot, `mobs` (one per stage, each can spawn), `random_min ≤ random_max`, `random_spots` (nine values) | `seq`, `number`, `map`, `stages`, `portal_map`, `portal_id` | no | restart |
| `boss_follower` | `world.boss_followers` | `group seq/seq` | position, `radius`, `mobs` (one per stage plus the chest monster, 0 = none), `count` (1 to 16) | | add, remove | restart |

**When a `defs.mobs` field takes effect** (read from `MobDef`, `Mob`, `Zones::new_mob`, `respawn` and the death code; the
registry's `Effect` follows this table). "Live" = the server reads the definition from the current snapshot (`GameData::live`) on every
use, so after `reload` living monsters follow at once; "next spawn" = copied into the monster when it spawns, living ones keep theirs.

| column(s) | effect | where it is read |
|---|---|---|
| `exp`, `dex`, `luc`, `wis`, `defense`, `magic`, `attrib`, `size`, `attacktime3` (the defence bonus), `attackattrib3` (country) | live | every hit, kill and attack check (`combat::monster`, `attackable`) |
| `resiststate1..15`, `resistskill1..6` | live | every skill landing |
| `attacktype`, `attackok`, `attacktime`, `attackrange`, `attack`, `attackplus` of entries 1 to 3 (entry 3's `attack` is also the recovery per 5 s, its `attackplus` the obelisk-guard flag) | live | the attack pass, every turn |
| `normaltime`, `normalstep`, `chasestep`, `chasetime`, `chaserange` | live | the idle and chase passes |
| `money1`, `money2`, `questitemid` | live | at the monster's death (`drops::roll`) |
| `attackspecial3` (the respawn code, shown as its time) | live for the next death or spawn | `respawn_due` and the spawn pass set the due time; a wait already running keeps its end |
| `hp`, `sp`, `mp` | next spawn | copied into the monster when it spawns (`Mob::new`, `new_mob`); the definition's HP is also its healing target and its percentage base, so a living monster with a changed definition heals to the new HP. A monster whose HP (or level) was 0 and is raised stays unspawnable until the restart: spawn entries, obelisks and boss groups drop unspawnable monsters when the server starts |
| `ai` | next spawn | the machine and Lua flags and the first state are set when it spawns |
| `level`, `mobname`, `mobid`, `rowid` | refused (above); `day`, `attackspecial1`, `attackspecial2`, `attackattrib1`, `attackattrib2` are read by nothing, so editing them changes nothing | |

Drop entries (`mob_drop`) are read at the death, so they are live; an item's grade (`item_grade`) is live through the drop tables, which
`reload` builds again from the freshly read `defs.items`.

Shown but not editable in this build: NPC placements, shops and gatekeepers (the shop window fills itself from the client's
`NpcQuest.SData`: a client patch), bind areas, map settings. Every rule above is also the rule the game server applies when it loads
the row (read the loader in `crates/game-server/src/pg.rs` for each table; where the loader is stricter, the editor takes the loader's
rule), so the editor never writes a row the server would skip.

**Grades, in plain words, on the page.** A drop entry's chance is shown as a percent (entries 1 to 6: `droprate` %; 7 to 9:
`droprate / 10,000` %), with the chance of each item of the grade (`chance / items that can drop`). "Items that can drop" uses the game
server's own rule (`DropTables::new`: grade 1 to 1024, type and type id not 0, real type not 27, no `drop` limit; limited items never
drop here). Gold: 30 % chance of `money1` to `money1 + money2 − 1`. A grade's page lists its items, the monsters and entries using it,
the box sets (`world.item_create`) and the sequence groups (`world.sequence_groups`) using it: changing an item's grade changes all of
them, and the page says so before the change goes into the draft.

## 3. Where edits live, and how they survive a reload

Schema `edits`, created by migration `db/migrations/0030_data_edits.sql` (so every binary that runs the migrations creates it; the
`defs` and `world` loads never touch it):

```sql
CREATE SCHEMA edits;

-- One per Apply (or Undo, or Import): who, when, why.
CREATE TABLE edits.change_sets (
    id          integer GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    made_at     timestamptz NOT NULL DEFAULT now(),
    made_by     text NOT NULL,                       -- the name typed on the page, or 'cli'
    message     text NOT NULL CHECK (message <> ''),
    undoes      integer REFERENCES edits.change_sets (id),
    undone_by   integer REFERENCES edits.change_sets (id)
);

-- The edits now in force on existing rows: per target, row key and field, the value and the value before any edit.
CREATE TABLE edits.fields (
    target      text NOT NULL,
    key         text NOT NULL,
    field       text NOT NULL,
    value       jsonb NOT NULL,
    original    jsonb,                               -- NULL for a field of a row the edits added
    change_set  integer NOT NULL REFERENCES edits.change_sets (id),   -- the last one that set it
    PRIMARY KEY (target, key, field)
);

-- Rows the edits added (their fields are in edits.fields) or removed (the row as it was, its child rows included).
CREATE TABLE edits.rows (
    target      text NOT NULL,
    key         text NOT NULL,
    action      text NOT NULL CHECK (action IN ('add', 'remove')),
    original    jsonb,
    change_set  integer NOT NULL REFERENCES edits.change_sets (id),
    PRIMARY KEY (target, key)
);

-- Every change of every change set, in order: the history, and what Undo reverses.
CREATE TABLE edits.log (
    change_set  integer NOT NULL REFERENCES edits.change_sets (id),
    seq         integer NOT NULL,
    target      text NOT NULL,
    key         text NOT NULL,
    field       text,                                -- NULL for a row added, removed or restored
    action      text NOT NULL CHECK (action IN ('set', 'unset', 'add', 'remove', 'restore')),
    before      jsonb,
    after       jsonb,
    PRIMARY KEY (change_set, seq)
);

-- Changes made on the page and not applied yet; one draft, shared by everyone using the editor.
CREATE TABLE edits.draft (
    seq         integer GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    target      text NOT NULL,
    key         text NOT NULL,
    field       text,
    action      text NOT NULL CHECK (action IN ('set', 'unset', 'add', 'remove')),
    value       jsonb,
    made_by     text NOT NULL,
    made_at     timestamptz NOT NULL DEFAULT now()
);

-- The optional background picture of a map and the world rectangle it shows: the picture's left edge is x1, right x2, top z1,
-- bottom z2 (a flipped axis is x1 > x2 or z1 > z2). Without a row the page draws the grid alone, z upward.
CREATE TABLE edits.map_images (
    map          smallint PRIMARY KEY,
    content_type text NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg')),
    image        bytea NOT NULL,
    x1 real NOT NULL, z1 real NOT NULL, x2 real NOT NULL, z2 real NOT NULL,
    source       text NOT NULL,                      -- where it came from ('upload', or the client file's name)
    added_at     timestamptz NOT NULL DEFAULT now()
);

-- A name per map for the page (the server has none: map names live in the client).
CREATE TABLE edits.map_names (
    map          smallint PRIMARY KEY,
    name         text NOT NULL
);
```

Values are JSON numbers (arrays for `mobs` and `random_spots`). Keys are the registry's text keys (section 2); the registry turns a
key into the row's `WHERE` (for `spawn_mob`: `mob_area_id = (SELECT id FROM world.mob_areas WHERE map = $1 AND seq = $2) AND seq = $3`;
`world.mob_areas.id` is renumbered by every import, so it is never part of a key).

**Surviving a reload.** `tools/defs/load_into_postgres.py` replaces `defs`, `tools/world/import.py` replaces `world`; `edits` stays.
`data-editor apply` writes every edit back. To know whether a schema is fresh, apply keeps a one-row marker table in it,
`defs.edits_applied` and `world.edits_applied` (`applied_at timestamptz`), which a load drops with its schema:

- **Marker missing** (the schema was just loaded): first refresh `original` of every `edits.fields` row of that schema's targets from
  the table, and `original` of every `remove` row; then write all edits; then create the marker. A field whose row no longer exists, an
  added row whose key now exists, or a removed row that is gone already is **reported and left alone** (the files changed under the
  edit); the report names the change set that made it. An `original` that differs from the one stored before is reported too
  ("the import changed mob 2133 hp 5000 → 5200 under your edit to 8000").
- **Marker present**: write all edits again (idempotent; it repairs a hand edit made with psql to an edited field).

Order after a load: `load_into_postgres.py` → `verify.py` → `tools/defs/additions.sh` (which now ends with `data-editor apply`);
`import.py` ends with `data-editor apply`. Both call it only when `target/release/data-editor` exists and print the command
otherwise. `additions.sql` therefore runs before the edits, and an edit wins over an addition on the same field.

## 4. Apply, Undo, Export and Import

**The draft.** Every change on the page goes into `edits.draft` at once, validated (the registry's rules, and: no edits to the rows of
a removed area, no removal of a row with draft edits pending). The page lists the draft as "what will change" (target, row, field,
now → new, the effect: live / next spawn / restart) with a remove button per line and "Discard all".

**Apply** (`POST /api/apply {message, by}`, or `data-editor apply-draft --message …`): one transaction, `LOCK TABLE edits.change_sets
IN EXCLUSIVE MODE` first so two Applies never interleave.
1. Validate the whole draft again against the current tables (another Apply may have run).
2. Insert the change set; for each draft line in order: write the log line (`before` = the value in force, `after` = the new one),
   update `edits.fields` / `edits.rows` (a `set` back to the original value deletes the field row, logged as `unset`), and write the
   table row in `defs` / `world`.
3. Empty the draft, commit, and answer with the change set id and the effects (which servers to reload or restart).
4. When `EDITOR_EXPORT_FILE` is set, write the export there (after the commit; a failure is reported, the Apply stands).

**Undo** (`POST /api/history/{id}/undo`): a new change set "Undo #id: <message>" whose lines reverse the log of `id` (set → set back to
`before`, or unset when `before` is the original; add → remove; remove → restore from `original`), applied as above. Refused, listing
the rows, when a later change set that is not undone touched the same target, key and field. Both change sets record each other
(`undoes`, `undone_by`).

**Export** (`GET /api/export`, `data-editor export <file> [--images]`): one JSON file, written in a stable order so two exports of
the same edits are identical:

```json
{"format": "newshaiya-edits", "version": 1, "exported_at": "…",
 "change_sets": […], "log": […], "fields": […], "rows": […], "map_names": […],
 "map_images": […]}
```

`map_images` (base64) only with `--images` (the pictures come from the client and are large). **Import** (`data-editor import <file>
[--replace]`, command line only): into empty edits tables, or with `--replace` after reverting every edit in force (each field back to
its original, added rows deleted, removed rows restored) and emptying the tables; then the schemas count as fresh (markers dropped) and
`apply` runs, refreshing the originals from this server's tables. Change set ids are kept as in the file.

## 5. The program

`crates/data-editor` (bin `data-editor`, lib for the tests), depending on `game-server` (lib) for the rules it must share: `MobDefs`
spawnability, `mobs::respawn_time`, the real type and `MAX_GRADE` of `drops`, so the editor and the server can never disagree.

```
data-editor serve                 the page (EDITOR_BIND, default 127.0.0.1:30890)
data-editor apply                 write all edits into defs and world (after a load; section 3)
data-editor apply-draft -m MSG    apply the draft from the command line
data-editor status                markers, counts of edits, the draft, problems found by a dry run of apply
data-editor history [N]           the last N change sets
data-editor undo <id>             undo a change set
data-editor export <file> [--images]
data-editor import <file> [--replace]
data-editor images import <folder>   <map id>.png or .jpg with <map id>.json {x1, z1, x2, z2}; names.json {id: name} into map_names
```

Settings (environment, `.env` read as the other binaries do): `DATABASE_URL` (required), `EDITOR_BIND` (default `127.0.0.1:30890`),
`EDITOR_HOSTS` (extra host names a request may be addressed to; none), `EDITOR_EXPORT_FILE` (unset: no automatic export). The editor runs the migrations at start, like the other binaries.

**Who may connect** (until the GM login of section 10): every request from an address that is not loopback, private (10/8, 172.16/12,
192.168/16, fc00::/7) or Tailscale (100.64.0.0/10) gets 403, whatever `EDITOR_BIND` says, and so does every request whose `Host` is not
`localhost`, `127.0.0.1` or `[::1]` (any port) or a name in `EDITOR_HOSTS` (comma separated; none by default), so a web page that rebinds its
own name to 127.0.0.1 cannot use the editor (`docs/server/audit.md` R17). `tools/checklist_server.py` has the same check, except that it also
takes any IP address as the host (the checklist page is opened by the box's address on the home network) and its extra names are
`CHECKLIST_HOSTS`. Requests that
change anything must be `POST`/`DELETE` with `Content-Type: application/json` and the header `X-Editor: 1` (a page on another site
cannot send those to the editor from the operator's browser). On a VPS the default bind is loopback: reach it with
`ssh -L 30890:127.0.0.1:30890`.

`tools/data-editor.sh start|stop|restart|status|logs` like `tools/checklist-server.sh`, binding `127.0.0.1:30890` (from another machine:
`ssh -L 30890:127.0.0.1:30890 <this box>`, then `http://localhost:30890/`). The dev
`.env` can set `EDITOR_EXPORT_FILE=db/edits/edits.json`: `db/edits/` is git-ignored.

## 6. The page

One HTML file with inline CSS and JavaScript (`crates/data-editor/src/page.html`, `include_str!`), no external scripts; a JSON API under
`/api/`. Layout: a map list on the left (id, name, kind, monsters, a picture mark), the drawn map in the middle, a side panel on the
right for what is selected, and a draft bar at the bottom ("3 changes: review · apply"). A "your name" box (kept in `localStorage`)
fills `made_by`.

**Views of a map** (tabs over the side panel, all reading `GET /api/maps/{id}`):
- **Monsters**: each monster spawned on the map: id, name, level, HP, EXP, gold, respawn time, how many on this map and in how many
  areas, "also on maps …"; opening one shows its nine drop entries (grade with its item count, chance, chance per item, the grade's
  items on hover) and gold, editable, and its stats grouped (life, stats, defence, resistances, movement, attacks, respawn).
- **Spawns**: the areas with their monsters and counts; selecting one highlights it on the map.
- **Portals**: index, position (read only), who may pass, levels, destination (map name and spot; "go to" opens that map with the
  spot marked).
- **Obelisks and bosses**: on the maps that have them.
- **NPCs**: read only, for orientation.

**The drawn map** (a `<canvas>`): world x to the right, z upward unless the map's picture says otherwise; wheel zooms, drag pans;
layers that can be switched off: background picture, blocked cells (`world.maps.blocked`, one bit per unit square, drawn
shaded; on dungeons it outlines the corridors), grid (every 64 units), spawn areas (filled by the average level of their
monsters, labelled with the count), NPCs, portals (a line to the destination when it is on the same map), bind areas, obelisks with
their guard circles, boss spots with follower circles. Hover shows a tooltip; a click selects. A selected spawn area has handles to
move and resize it; obelisk, guard, boss and follower spots can be dragged; a "new area" tool draws a rectangle. Every move goes into the
draft (coordinates rounded to 0.1). The canvas draws from the draft too, so pending changes show (outlined dashed).

**Grades and items**: `GET /api/grades/{g}` and an item search (`GET /api/items?q=`, by id or name) open in the side panel; the "drop
this one item" helper is a button on an item: it shows the warning of decision 2, then adds two draft lines (the item's new grade: the
lowest grade from 1 to 1024 that no item and no drop entry, box set or sequence group uses; and the monster's entry, chosen from its
entries that are off, or replacing one the operator picks).

**History**: change sets with message, who, when, the number of changes and their effects; one opens to its lines (before → after) with
"Undo". **Servers**: each game server of the `game_servers` table with its `info` answer (players online), and buttons "Reload drops
and monsters" (`reload`) and "Restart in 1 / 5 / 15 minutes" (`restart N`; minimum 30 s as `quit`), with a confirmation naming the
players online. After an Apply the page offers whichever the effects need.

**Map pictures**: per map, "Set picture" (upload PNG or JPEG up to 8 MB, and the four edge coordinates, pre-filled with 0 / size) and
"Remove picture"; a map name can be edited in place (`edits.map_names`, not a change set: it changes nothing in the game).

## 7. The game server: `reload` and `restart`

Two console commands (`crates/game-server/src/console.rs` words, help entries and parse tests; `server/console.rs` runs them), reachable
like the others from the `console` tool, from the editor through `NOTIFY`, and in game as `.reload` / `.restart` by level-1 staff:

- **`reload`**: reads `defs.mobs` (with `attach_skills` against the loaded skills), `defs.items` (for the grades only) and the drop tables
  (`DropTables::new`, `load_boxes`, `load_quest_links`, `load_sequences`), and swaps them in. Answer: `reloaded: <n> monsters, <n> drop
  grades, <n> box sets (<ms> ms)`, or the error, with nothing swapped. To make the swap possible, `GameData.mobs` and `GameData.drops`
  move into `pub live: std::sync::RwLock<Arc<Live>>` with `pub struct Live { pub mobs: MobDefs, pub drops: DropTables }` and
  `GameData::live(&self) -> Arc<Live>`; every user takes one snapshot (`let live = ctx.data.live();`) and keeps it for the whole
  operation; the `mob_updater` pass takes one per pass. Main's start-up code that builds the drop tables becomes one function used by
  both (`pg::load_live`). `ItemDefs` itself is not swapped (only the grades are read from the fresh load).
- **`restart [minutes]`**: the `quit` countdown with its notices, then the process exits with status **75** instead of 0. `quit` keeps
  exit 0. `docs/server/deploy.md`'s units already restart on a non-zero exit (`Restart=on-failure`); `tools/game-server.sh start` runs the
  binary in a loop that starts it again only after status 75 (a `stop`, Ctrl+C or crash ends the loop, as now).

## 8. Map pictures from the client (client research entry 8)

The server side never reads the client. The client side answers queue entry 8 (`docs/testing/client-research-queue.md`): where the
extracted client keeps a picture of each map (minimap or the big map window), which picture is which map id, how `game.exe` places a
world position on it (the rectangle and the axis directions, with addresses), and each map's name as the client shows it. It converts
the pictures to PNG into `data/map-images/` (git-ignored): `<map id>.png`, `<map id>.json` (`{"x1", "z1", "x2", "z2"}` as in
`edits.map_images`), `names.json`, and a `README.txt` naming each source file. `data-editor images import data/map-images` then loads
them. Until then the page draws the grid alone. Other operators upload their own pictures or import such a folder.

## 9. Client data

Nothing the editor writes has a client copy, by construction (section 2's refused fields); every target is server-only per
`docs/plans/done/client-patches.md` section 8. One overlap: an item's `grade` is also a field of `Item.SData`, so `client_patch.py check` 4
("every `defs.items` change is listed") would refuse a grade edit. Rule: **a difference in `grade` alone, on an item that has an
`edits.fields` row of target `item_grade`, passes check 4** (the client does not use the drop grade; a record that is generated anyway
carries the server's grade). The same holds later for `Monster.SData`: the editor's monster fields are the server-only ones.
`docs/plans/done/client-patches.md` section 3 check 4 says this as of this plan.

## 10. Open for later

- **Sign in with a GM account** (decision 8): name and password checked as the login server checks them (`login-server`'s accounts
  code, argon2), allowed for staff level 1 (`accounts.privilege`; a setting for the highest level allowed); a session cookie
  (`HttpOnly`, `SameSite=Strict`, 12 hours, kept in memory); `made_by` becomes the account name; the address limit becomes a setting
  (`EDITOR_ALLOW`, CIDR list) so the editor can be reached over the internet behind HTTPS (a reverse proxy, documented in
  `docs/server/deploy.md`). Until then: loopback/LAN/Tailscale, SSH tunnel on a VPS.
- **Live reload of spawns, portals, obelisks** (spawn tables are per zone and hold living monsters: more design).
- **NPCs, shops and gatekeepers**: need the `NpcQuest.SData` client patch (`docs/plans/done/client-patches.md` section 7).
- **New monsters, moving or adding portals, adding obelisks**: need client data (`Monster.SData`, the `.wld` files) first.
- **Converting an existing original server** (decision 9): `docs/plans/roadmap.md`, "Later".

## Build

Two parts, one after the other: **part A** (the game server's commands, the `edits` schema, the `data-editor` core and its command
line; no page) and **part B** (the page and the drawn map). Each reads sections 1 to 9 first.

### Part A

1. **Game server `reload`** (`cargo test -p game-server` green).
   - `GameData.live` as section 7; replace every use of `data.mobs` / `data.drops` (server, `server/items.rs`, `server/admin.rs`,
     the `mob_updater` pass, tests' `GameData` builders) with a snapshot. `Zones::new` keeps taking `&MobDefs` at start.
   - `pg::load_live(pool, &skills) -> Result<Live>`, used by `main` and by `reload`. The console's database work goes through the same
     path its other database commands use (read `server/console.rs` and `backend.rs`), so `tests/game_flow.rs` can run `reload` with a
     test backend whose monster drops and attack changed, and prove a monster killed after the reload drops by the new table and hits
     by the new attack, and that a failed load swaps nothing.
   - `console.rs`: the word, help ("reloads the monsters and the drops from the database: drops, gold and monster stats change at once"),
     parse tests.
2. **Game server `restart [minutes]`**: as section 7 (`Command::Restart(Option<u32>)`, the quit countdown with a flag in `Context` that
   `main` reads to exit with 75 after `shut_down`; `quit cancel` cancels either). Tests for the parse and the flag. `tools/game-server.sh`
   loop. `docs/server/deploy.md`: the exit status and that `Restart=on-failure` restarts it.
3. **Which monster fields are live**: read `MobDef`, the `Mob` struct and where a living monster copies from its definition (HP and max
   HP at spawn, anything else), `Zones::new` / `SpawnArea` (the respawn time), `set_obelisks` / `set_boss_groups`. Record per `defs.mobs`
   column: live, next spawn or restart, in the registry and as a short table in this plan's section 2 (replace "step A3 finds which").
4. **Migration** `db/migrations/0030_data_edits.sql`: section 3's tables, with a comment on each as in the other migrations.
5. **`crates/data-editor`** (workspace member; `axum` 0.8, `tokio`, `sqlx` as the game server's, `serde`, `serde_json`, `base64`; the
   `game-server` lib):
   - `registry.rs`: section 2's targets: table, key parse/format, fields with their type (int range, real inside the map, monster that
     can spawn, map id, int array of a length) and effect, refused fields with their reason, add/remove. The schema names (`defs`,
     `world`) are parameters so the tests can use scratch schemas. Unit tests for every rule (each refused field, each range edge, keys).
   - `store.rs`: draft, apply (section 4), undo, history, status. `apply.rs`: section 3's apply with the markers and the report.
   - `export.rs`: export and import.
   - `console.rs`: one request to a game server through `NOTIFY` (the protocol of `crates/login-server/src/bin/console.rs`).
   - `main.rs`: section 5's commands (no `serve` yet: part B).
   - PostgreSQL tests (`DATABASE_URL`, skipped without, as `crates/game-server/tests/postgres.rs`): scratch schemas made from
     `db/defs/schema.sql` and `db/world/schema.sql` with a few rows; draft → apply → the table changed and the log right; undo; undo
     refused after a later change; a fresh load (scratch schema dropped and made again with one original changed) → apply reports it
     and writes the edits; added area collision reported; export → import `--replace` → identical export.
6. **The dev box**: `tools/defs/additions.sh` and `tools/world/import.py` end with `data-editor apply` (section 3). Build it, run
   `data-editor status` and `apply` (no edits yet: nothing changes, markers made), then a trial from the command line: a draft line
   raising monster 2133's (or any spawned one's) entry 1 chance, `apply-draft`, `console 1 reload`, check the answer, `undo`, reload.
7. **`client_patch.py`**: if `tools/sdata/client_patch.py` exists, make check 4 follow section 9 (with a test case); if not, nothing
   (its plan already says so).
8. **Docs**: README (the program, its commands, `EDITOR_BIND`, `EDITOR_EXPORT_FILE`, the console's `reload` and `restart`, the test
   count), `docs/server/deploy.md` (the fifth binary, the SSH tunnel, export/import to move edits, `data-editor apply` after copying `defs` and
   `world`), `docs/original/operator-console.md` (the two new commands, as decisions of the new server), `docs/original/database-notes.md` (the
   `edits` schema and the order after a load), `docs/plans/roadmap.md` step 1f (part A built).

### Part A as built

Where the build departs from or adds to the spec above:

- **`apply` before every Apply and Undo.** If a loaded schema has no marker (a fresh load), Apply and Undo first write the edits in force
  into it (`apply::ensure_applied`) and return what that found (`Applied.problems`), so a new change never meets tables that lack the
  older ones. Without this, an Apply made right after a load would have looked like "the import changed it" at the next `apply`.
- **The marker table has a column `skipped`** (`edits_applied (applied_at, skipped text[])`): the added rows that met an imported row with
  the same key at the load. They stay unwritten at every later apply (the marker alone could not tell the imported row from the added
  one), and their spawn monsters with them.
- **Draft checks are a dry run**: a draft line is validated by applying the draft plus the line inside a transaction that is rolled back
  (with the change set numbering put back, so the history has no gaps). The rules are exactly Apply's.
- **Sequence numbers of added rows**: spawn areas from 100,000 as specified; a spawn monster from 1,000 in an original area, from 1 in an
  added one; obelisk guards and boss followers from 1,000 (the spec left these open; imported rows use the numbers below).
- **Boss stage lists**: `mobs` keeps its length; the first stage must spawn, later stages may be 0 (the loader drops them); followers'
  lists may hold 0 (none) anywhere. Positions are checked only for the fields being changed (an imported row that is off its map does not
  block edits to its other fields).
- **`exported_at`** is the time of the newest change set, not the time of the export, so two exports of the same edits are the same bytes.
- **Undo** refuses when a later change set that is not undone, and is not the undo of a change set later than this one, touched the same
  target and key (and field, or any field of a row added, removed or restored). Undoing an undo is allowed; it clears the "undone by" of
  the change set the undo had reversed.
- **`data-editor draft ...`** (list, add set/unset/add/remove, remove, clear) and `show <id>` exist for the command line; the page uses the
  same functions.
- **Not in part A**: `serve` and `images import` (part B).

### Part B as built

Where the build departs from or adds to the spec above:

- **Files**: `serve.rs` (routes, the address limit, the `X-Editor` rule, errors as `{"error"}`), `api.rs` (the page's reads, JSON built by
  PostgreSQL where it can be; the item rule uses `game_server::inventory::real_type`), `images.rs` (pictures, names, the folder import),
  `page.html` (one file, no external script). `tests/common` is the scratch-schema harness both PostgreSQL test files share.
- **Extra routes**: `GET /api/targets` (the registry as the page needs it: fields, kinds, effects, refused reasons), `/api/status`,
  `/api/edits` (every edit in force, to mark them), `/api/items/{id}/helper` (the facts behind "drop this one item": the old grade's
  users, the lowest free grade counting the draft), `/api/maps/{id}/blocked` (the blocked bits), `POST /api/servers/{id}/cancel` (`quit cancel`).
  Pictures are uploaded as JSON (`{data: base64, x1, z1, x2, z2, source}`) because every change is JSON with the header.
- **The draft collapses**: a `set` or `unset` of a field the draft already changes replaces that line (a drag that goes on is one change);
  a request with several lines is all or nothing (`Editor::draft_put`).
- **A grade change warns first**: changing an item's grade on the page lists the users of both grades (entries, box sets, sequence groups)
  and asks before the line goes into the draft; the helper's warning says the same for the one item.
- **Selecting and moving**: click selects, then a drag moves or (spawn areas) resizes the selection; dragging unselected ground pans, so
  dense maps stay usable. The view's orientation follows the picture's rectangle when there is one (`x1 > x2` or `z1 < z2` flips an axis);
  without one, x runs right and z up. The blocked-cell layer reads bit `z * size + x` as the server does.
- **`info` of each server** is asked with a two-second wait (`console::request_within`), so a stopped server shows "no answer" at once.
- **Not built**: GM sign-in (section 10), editing NPCs, a picture per layer other than the background, undo of a single draft line from the
  review window other than removing it.

### Part B checked

In a browser (headless Chromium driven by a script, against the dev database and a running game server): map 2's monster list and a drop
table (monster 2057: entries, chances, per item, items of each grade), editing a drop chance and an HP into the draft, the review window,
Apply with a message, the History window and its Undo, a grade's users, the helper's warning and its three draft lines, a server's reload
(`reloaded: 2747 monsters, 294 drop grades, 68 box sets`), selecting a spawn area on the map, moving and resizing it (four coordinate lines
in the draft, reduced to the changed ones), moving an obelisk, adding a guard group, drawing a new area with a monster, the portal and
bosses tabs, the blocked cells of dungeon 65, a picture imported from a folder and one uploaded on the page (and removed), the refusal of a
change without the header. Everything was undone or discarded afterwards. The PostgreSQL tests (`tests/api.rs`) cover one round trip per kind.
Not checked: a map picture from the real client (`data/map-images/` did not exist), a restart from the page to its end (the button started the countdown, "restarting in 60 seconds", and it was cancelled with `quit cancel` because a player was
online; `restart` itself is tested in `game-server`), a second browser at the same time.

### Part B

1. `data-editor serve`: axum routes for section 6 (`GET /api/maps`, `/api/maps/{id}`, `/api/maps/{id}/image`, `/api/mobs/{id}`,
   `/api/grades/{g}`, `/api/items?q=`, `/api/draft`, `/api/history`, `/api/history/{id}`, `/api/servers`, `/api/export`; `POST
   /api/draft`, `/api/apply`, `/api/history/{id}/undo`, `/api/servers/{id}/reload`, `/api/servers/{id}/restart`, `/api/maps/{id}/image`,
   `/api/maps/{id}/name`; `DELETE /api/draft/{seq}`, `/api/draft`, `/api/maps/{id}/image`), the address limit and the `X-Editor` rule of
   section 5, errors as `{"error": "…"}` with the registry's reason. Tests for the address limit and the header rule, and one API round
   trip per kind (PostgreSQL tests as in part A).
2. `page.html` as section 6. Check it in a browser against the dev database (the built-in browser or Chrome): map 2's monsters and a
   drop table, a grade's users, the helper's warning, moving and resizing a spawn area into the draft, the draft list, an Apply with a
   message, the history and its Undo, a server's reload. Screenshots of the map view of maps 0 and 2 into the report.
3. `data-editor images import`, and `tools/data-editor.sh`. If `data/map-images/` exists by then (research entry 8), import it and check
   a map's picture lines up with its spawn areas and NPCs; if not, say so in the report.
4. Docs: README and `docs/server/deploy.md` (the page, the tunnel), `docs/testing/live-test-checklist.md` a new section for the editor (an edited drop
   seen in game after `reload`, a moved spawn area after `restart`, a portal's new destination, an obelisk guard count), its line in
   `docs/testing/live-test-solo.md`, `docs/plans/roadmap.md` step 1f (built).
