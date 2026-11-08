# Extensions chunk 4, pets (spec and build plan)

**Specified 2026-11-07; not built.** Chunk 4 of `docs/plans/extensions.md` (section 6, work item 7.4). Chunks 1, 2, 3 and 5 are built
(`docs/plans/ext-1-foundation.md`, `ext-2-rules.md`, `ext-3-effects.md`, `ext-5-lua.md`; the operator's pages `docs/server/rules.md`,
`scripts.md`, `effects.md`). This chunk adds **pets**: a player weakens a monster that is fighting them and uses a **capture item** on it;
caught, the monster becomes a **Pet Egg** in the bag; used, the egg calls a **small copy of that monster** that follows its owner, is never
fought, and **fetches its owner's drops** into the bag. Everything is in one extension folder, `extensions/pets`, made of two item effects
(chunk 3's registry), two rule points (chunk 2) and a set of client-only monster records. With the extension not installed or disabled the
server behaves exactly as before. Release **0.4.0** follows this chunk (`docs/plans/extensions.md` section 2a decision 4).

The design is decision 1 of `docs/plans/extensions.md` section 2. The client answers it relies on are in
`docs/client/research-extension-features.md`: "Small monsters as pets, scenery and campfires" (entry 9), "Ground items" and "Items, ids
and ceilings" (entry 10). The operator's page this chunk writes is `docs/server/pets.md`.

## 1. The maintainer's decisions

From 2026-10-22 (decision 1 of the plan), not re-asked:

1. A **weakened, non-boss, non-quest** monster that is **fighting the player** can be caught with an item; the chance rises as its HP falls.
2. The catch becomes a **Pet Egg** item carrying the monster id. Summoned, the pet is a **small follower of that monster**, its animations
   kept; **one** pet out per player; **static** (no levels, no evolution).
3. It is **cosmetic** and **picks up its owner's drops**: it walks to the item, then the item goes into the owner's bag with the normal
   pick-up line; a full bag leaves the item on the ground. **No buffs** from pets.
4. Pets live in **their own extension folder behind their own flag**; with it off the server behaves exactly as before. Chunk 3's item
   effects and chunk 2's rules are the tools. Personal (instanced) loot is chunk 9, a separate spec.

From 2026-11-07 (this spec):

5. **One egg item for every monster.** The monster id is stored inside each egg (section 4.1). Any monster that passes section 3.2 can be
   caught. The client's tooltip cannot show anything per item (entry 10 b), so every egg reads "Pet Egg" until the companion DLL adds a
   line (section 11, entry A); the owner sees which monster it is when it comes out.
6. **A failed catch uses the capture item up** ("It broke free!"); the monster goes on fighting.
7. **Pets are about 40 % of their wild size**, by the client's fixed size classes (section 6.2): a goblin at 1.0 becomes 0.4, an ogre at 2.0
   becomes 1.0, a giant at 5.0 becomes 2.0.
8. **The pet fetches items and gold** that belong to its owner.

## 2. The parts, in one picture

```
capture item (100/227, effect "capture") ──use──▶ the weakest monster fighting the user ──caught──▶ Pet Egg (100/228, craft name "P00123.…")
                                                                                                           │
                                   rules: deny capture (maps, levels, …)                                   │ use (effect "pet_egg")
                                                                                                           ▼
            client-only monster records 3001..5473 (copies of 1..2473, small, unclickable) ◀── 0x0601 ── the pet (zone/pets.rs)
                                                                                                           │ follows, fetches
                                   rules: deny pet (maps where pets do not come out)                       ▼
                                                                  the owner's ground items ──▶ the owner's connection ──▶ 0x0205 "Attained"
```

What is new, by place:

| place | what |
|---|---|
| `db/migrations/0035_pets.sql` | `content.client_monsters`, the `monster` kind of `content.ext_ranges`, `public.character_pets` |
| `tools/sdata/load_client.py`, `client_patch.py` | the original `Monster.SData` imported into `client.monsters`; `Monster.SData` generated (section 6) |
| `crates/data-editor/src/ext/` | the `[[monster_look]]` entry of `rows.toml`, the `monsters` key of `[ids]` and its ceilings |
| `crates/game-server/src/effects.rs`, `server/effects.rs` | the effects `capture` and `pet_egg` |
| `crates/game-server/src/rules/` | the `deny` points `capture` and `pet` |
| `crates/game-server/src/zone/pets.rs` | the pets in the zones: view, follow, fetch, capture |
| `crates/game-server/src/server/pets.rs` | the owner's side: summon, dismiss, persistence, the picks |
| `extensions/pets` | the manifest, `rows.toml`, a README |
| `docs/server/pets.md` | the operator's and author's page |

## 3. Capture

### 3.1 The request and the target

Item use (`0x050A u8 bag, slot`) carries **no target** (entry 10 a: `0x4c7d40`); only skills do. A capture skill would need a new skill row,
a learnt skill and its client rows; instead the target follows from the server's own state, which the decision already requires: the monster
must be **fighting the player**. The use of a capture item goes down chunk 3's chain (`docs/server/effects.md` section 3: core checks, the
rules' `item_use`, the reuse group, `game.on_item`, then the effect), and the `capture` handler:

1. asks the rules' **`capture`** point (section 9) with the user's scope and map: a refusal is whispered as every `deny` is, the item kept;
2. takes the **candidates**: the monsters on the user's map that are alive, whose state is `MobState::Fighting(user)` (the monster's target is
   this player: aggro'd on it, `docs/original/mob-fight.md` section 1), and that are within the effect's `range` (default 15) of the user;
3. of those, the **one with the lowest HP in percent** (`hp × 100 / max_hp`), the nearest on a tie: the monster the player has been
   weakening. None: `No monster is fighting you.`, the item kept;
4. tests it (3.2). Not catchable: `That monster cannot be caught.`, the item kept;
5. its HP in percent above the effect's `below` (default 30): `It is too strong to catch. Bring it below 30% HP.` (the number is the
   effect's), the item kept;
6. no free bag slot for the egg: `Your bags are full.`, the item kept;
7. **rolls** (3.3). A miss: the item is used up (`Done::UsedUp`), `It broke free!`, the monster goes on fighting. A hit: 3.4.

The texts are whispers from `Server` under the effect type's refusal id (chunk 3's `refuse`, at most once in ten seconds per type); "It broke
free!" and the catch line of 3.4 are sent every time (they answer a roll, they are not refusals).

A later refinement, if the client tells the server what the player has selected (section 11, entry D): the selected monster instead of the
weakest. Not needed for this chunk.

### 3.2 What can be caught

A monster can be caught when **all** of these hold (each read from what the zone already keeps; the build checks the names against the code):

- it came from a **spawn area** (`Origin::Area`): not an obelisk, an obelisk's guard, a boss chain's boss, follower or chest;
- it is **not** a Lua monster (`Mob::lua`), **not** running a state machine (`Mob::scripted`), **not** unbeatable, and **not** a monster an
  extension's script spawned (`Zones::ext_spawns`): "monster madness" monsters are event monsters;
- its id is **not** named by any boss group or boss follower (`world.boss_groups`, `world.boss_followers`), so a boss that also stands in an
  area is a boss;
- it drops **no quest item** (`MobDef::quest_item` is `None`: the 271 monsters with `QuestItemID`);
- it has a **pet record**: its id is in the egg's `monsters` (section 4.2);
- it is not in the effect's `except` list, and, when the effect has `levels_above`, its level is at most the user's level plus that.

### 3.3 The chance

With `p` the monster's HP in percent (1 to 100; a live monster is at least 1), `b` the effect's `below` and `[c0, c1]` its `chance` (percent;
default `[10, 60]`):

    chance = c0 + (c1 − c0) × (b − p) / b          (whole percent, rounded down)

At the threshold the chance is `c0`, near death it approaches `c1`; with the defaults: 10 % at 30 % HP, 27 % at 20 %, 43 % at 10 %, 58 % at 1 %.
The roll is `rand() % 100 < chance`, with the zone's random source (so the tests fix it).

### 3.4 A catch

Under the `mobs` lock, the zone (`Zones::capture`) checks the monster again (alive, still fighting the user, still at most `below`) and
then takes it out **as a death without rewards**:

- `0x0602` to its view at once;
- its damage record, debuffs, trail, path and Lua events cleared (`MobMap::remove`'s list, except the removal), `hp = 0`, state `Idle`,
  `dead_until = now`: the next `remove_bodies` pass removes it and **counts its spawn entry down** as after a death, so it **respawns as
  usual** (`docs/original/combat.md` section 6);
- **no** experience, kill, drop, gold, kill count, quest progress, `Kill` event, script `kill` event, game-log death record: nothing a
  death gives. A `0x0606` is not sent (the monster does not die on screen; it vanishes).

The connection then puts the egg in the bag and uses up the capture item **in one save**: `Done::UsedUpFor(egg)` (the box's path,
`server/items.rs`), whose `0x0205` gives the "Attained Pet Egg" line; then the whisper `Caught! The egg is in your bag.` The monster's name
is not in the whisper: the server's names are the original's Chinese text (`defs.mobs.mobname`), the client's English names are only in
`Monster.SData`. The game log gets what a box gives (the item used, the item made); the log line (`tracing::info!`) names the monster id.

If the catch fails between the roll and the save (the monster died meanwhile), the capture item is used up as on a miss and the player is
told `It broke free!`.

## 4. The Pet Egg

### 4.1 One item, the monster inside

One item row, **100/228** "Pet Egg", carries any monster. Each egg's **craft name** (`ItemUnit::craft_name`, at most 20 bytes,
`backend::CRAFT_NAME_MAX`) holds:

    P<monster id, 5 digits>.<unix seconds of the catch, 8 hex digits>.<a counter, 2 hex digits>        P00123.6731A2F0.00

- The monster id is what the egg calls (`P00123`: monster 123).
- The rest makes each egg unique (two catches in one second get different counters; the counter is the zone's, wrapping), so the server
  can tell which egg's pet is out (4.3) without a row id: the inventory keeps `ItemUnit`s, not rows.
- Why the craft name: it travels with the item in every item packet (`0x0106`, `0x0204`, `0x0205`, `0x0711`), in the database row, through
  trades, the warehouse, the guild bank, the market and a drop on the ground (`backend::Kept`), with no new column. For types 25, 30, 38,
  41, 44, 95 and 100 the client clears the craft-name pointer before drawing the tooltip (entry 10 b, `0x49fa00`), so the text is never
  shown and never parsed by the client's stat lines. On the server, a craft name is read only for equipment (stat bonuses, the recreation
  rune, the enchant step): the build lists every reader of `craft_name` (`grep`) and confirms none acts on a type-100 item in the bags; if
  one does, it stops and asks.
- An egg whose craft name does not parse (made by `/make`, a script's `give_item`, or edited) is **empty**: its use says `This egg is empty.`
  and the egg stays.

The egg row is **not stackable** (`count` 1, as 100/108's): eggs never merge. The loader refuses a `pet_egg` effect on a row whose `count` is
not 1 (section 7.2).

What the player sees: the item name "Pet Egg" and its description, the same for every egg (decision 5). The catch line, the pet that comes
out and, later, the DLL's tooltip line (section 11, entry A) tell them apart.

### 4.2 Trading and storage

The egg is an ordinary item of its row's columns (copied from 100/108, section 10.2): it can be **traded, sold to a merchant, put in the
warehouse, the guild bank or the market, dropped, and lost at a death** wherever the original allows that for such an item. Nothing is
added for the egg: the craft name goes along. When an egg whose pet is out leaves its owner's bags, the pet goes (4.3).

### 4.3 Summon and dismiss

Using an egg (`0x050A`) with a parseable craft name, through chunk 3's chain to the `pet_egg` handler:

1. **This egg's pet is out**: it is **dismissed** (`0x0602` to its view), the stored pet cleared (section 8). The egg stays.
2. Otherwise the rules' **`pet`** point (section 9) is asked with the owner's scope and map; a refusal is whispered, nothing changes.
3. The monster id not in the egg effect's `monsters`: `This pet cannot come out.` (an egg from an earlier configuration).
4. Another pet is out: it goes first. Then **this egg's pet comes out** beside the owner (section 5.2) and is stored (section 8).

The answer is the success `0x050A` with the egg still in its slot: a new `Done::Kept(ItemUnit)` that sends what `Done::Consumed` sends
(`send_used`) for the unchanged unit, nothing saved. Whether the client shows an unchanged count right is checked live (section 12 item 4;
section 11, entry C). The egg's reuse time is its row's (`reuse group` of 100/108's columns), so summon and dismiss cannot be spammed faster.

**While a pet is out**, it shows whenever its owner is in a zone, alive, visible and allowed by the `pet` rules; otherwise it is **hidden**
(kept, not shown) and comes back by itself. The cases:

| what happens | the pet |
|---|---|
| logout, a character change, the connection ends | goes with the owner (`Zones::leave`); comes back at the next login (section 8) |
| a map change, a portal, a recall, a move by item, a party or guild zone, an instance | leaves with the owner and comes out beside it on the new map (the connection summons it after `enter`) |
| a warp or move on the same map (`relocate`, a GM's `/move`) | snaps to the owner at the next pass (section 5.3) |
| the owner dies | hidden; shown again when the owner is alive (any rebirth) |
| the owner rides a mount, rides with another, is disguised or transformed | follows as usual (its run is faster than a mount, section 5.3) |
| a staff member goes invisible | hidden (a pet must not give the GM away); shown when visible again |
| a `deny pet` rule starts to match (the owner enters such a map, a time window opens, a `reload`) | hidden at the next check (5.4), the rule's message whispered once; shown when it no longer matches |
| the egg leaves the owner's bags (trade, sale, warehouse, guild bank, market, drop, loss at death, deletion) | dismissed and the stored pet cleared |
| the egg is used again | dismissed (case 1 above) |
| the extension is disabled (at the next start) | no pet comes out; the stored pets are kept for when it is enabled again |

How the connection notices the egg left: after handling **any** request or zone event, a connection with a pet out looks for the egg's craft
name in its bags (at most a few hundred slots, only while a pet is out); missing, it dismisses. One place in the dispatch loop covers every
path that moves items, including those another connection starts (a trade's confirmation arrives as a zone event).

## 5. The pet in the zone

### 5.1 Not a monster of `MobMap`

The pet is **not** in `MobMap`. Every monster pass (respawn, idle, fight, the monsters' search for targets, guards, the Lua and script
passes, fields, area skills, debuffs, `remove_bodies`, the GM's monster commands, `mob_count`) works on `MobMap` and therefore never sees a
pet: it is never targeted, never aggro'd, never hit by an area skill, never counted by a spawn area, and attacks or skills naming its id find
no monster and do nothing (as for any unknown id). Item 4 of the plan's list (out of combat, never targeted, never counted) holds by construction.

`Zones` gains `pets: Mutex<HashMap<u16, PetMap>>` (by map; **taken after `maps` and `mobs`, before `ground`**; the lock-order comment of
`Zones` says so) and `last_pet_stamp` (the counter of 4.1). In `zone/pets.rs`:

```rust
pub struct PetLook { pub record: u16, pub wild: u16, pub loot: Option<PetLoot> }    // record = base + wild (base 0: the wild record)
pub struct PetLoot { pub range: f32, pub gold: bool }
struct Pet { id: u32, owner: u32, look: PetLook, egg: [u8; CRAFT_NAME_MAX], position: [f32; 3], cell: Option<(i32, i32)>,
             state: PetState, next_step: Instant, skipped: Vec<u32>, next_check: Instant, hidden_by: Option<Hide> }
enum PetState { Follow, Fetch { item: u32, arrive: Instant }, Waiting { item: u32, since: Instant } }
pub struct PetMap { pets: HashMap<u32, Pet>, owners: HashMap<u32, u32>, cells: HashMap<(i32, i32), Vec<u32>> }
```

`cell: None` is a hidden pet (not in `cells`, no packets). The object id comes from `last_mob_id` (the monsters' counter), so the client's
monster table never sees two objects with one id.

### 5.2 What the players see

The pet is a monster to the client: `0x0601` (`u32 id, u8 new, u16 wMOBID = look.record, f32 x, z`) then `0x0608` (max HP 1, HP 1: entry 9,
a max HP of 0 divides by zero), as `mob_enter` sends for a monster. **Everyone** in view sees it, with the record's name (the monster's
English name from `Monster.SData`), its model and animations, at the record's size class; **Size 0** (section 6.2) makes it impossible to
click or target (entry 9, `0x43b920`), so nobody attacks it from the client.

The view paths that send monsters also send pets: `Zones::enter` (a player coming onto the map gets the pets in its view after the
monsters), `swap_view` (pets leaving and coming into view with the monsters, before the NPCs), and a pet's own move (5.3), which leaves and
enters the views around on a cell change as `walk_mob` does. A pet coming out is `0x0601` with `new = 1`; a pet already there `new = 0`;
dismissed, hidden or gone, `0x0602`.

`0x0304` (a monster's state) and `0x0305` (its HP) asked for a pet's id are answered as for a monster: `0x0603` with its spot and
`0x0304` with HP 1 and the speeds `(ATTACK_SPEED, MOVE_SPEED)`, or `0x0305` with HP 1.

### 5.3 Following

The pets pass, `Zones::pets(now, rand)`, runs in the monster loop of `server/tasks.rs` (beside `expire_ext_spawns`, every
`MOB_UPDATE_INTERVAL` or pacing tick) and returns at once when no map has a pet. For each pet whose `next_step` has come:

- **owner gone** from the map: the pet is removed (it cannot happen through the normal paths; a safety net);
- **owner dead, invisible, or ruled out** (5.4): hidden; else, if hidden, shown again at the follow spot;
- **distance to the owner above 20**: it **snaps**: `0x0602`, then `0x0601` (`new = 0`) at the follow spot (the client itself snaps a
  monster whose target is 20 or more away, entry 9, so a walk is not worth sending);
- **above 4**: it walks or runs to the **follow spot**, the point 2.5 from the owner on the line to the pet (so it trails behind and to the
  side it came from): `0x0603` with motion **1 (run)** when the distance is above 8, else **0 (walk)**; its position becomes the spot at once
  (the client animates the way), `next_step = now + distance / speed`;
- **4 or less**: it stays.

Speeds (`PET_WALK = 3.7`, `PET_RUN = 9.7` units per second) are the client's for the look records' numbers (section 6.2: normal 1000 ms and 4,
chase 1000 ms and 10; `(Step + 0) / (Time × 0.001) − 0.3`, entry 9). A running player makes 3.0 to 7.0, a mount 7.0 or 8.0
(`docs/original/move-check.md`), so a pet keeps up. Pets do not use the monsters' pathing (`docs/original/pathing.md`): they go straight;
the owner just walked there, and a pet that cuts a corner through a wall is a cosmetic slip, not a gameplay one.

### 5.4 Rules and visibility checks

Every 5 seconds per pet (`next_check`) the pass asks the rules' `pet` point with the owner's scope (`Who` from its `Placed`, the map) through
the zone's `RuleSet` (as the attack point does). A match hides the pet and sends the owner the rule's message once (the zone's way of
whispering a refusal to a player: the build uses what the attack point's refusals use); when no rule matches any more, the pet comes back. The
summon (4.3 step 2) and the arrival on a map ask the same point at once.

## 6. The look: client monster records

### 6.1 What entry 9 found, and what follows

The draw scale is the record's **"Day" byte**, in fixed classes: 0 → 0.6, 1 → 1.0, 2 → 1.3, 3 → 1.6, 4 → 2.0, 5 → 5.0, 6 → 0.4, other → 1.0
(`0x4322e0`). **Size** is the click quad's width: **Size 0 cannot be clicked**. The name is the record's. A record id at or above the file's
count has no model and no name (`0x461210`); records may share a model. So a small, unclickable copy of a monster is **a new record** of
`Monster.SData` with the same model, a smaller Day class and Size 0: data only, no DLL. A per-object scale or name would need the DLL
(entry 9: the scale call `0x40e910`, the name source `0x447d30`); not needed.

### 6.2 The pets' records

One record per catchable monster: record **3000 + m** for monster **m**, for m = 1 to 2,473 (the client's file holds 2,474 records, ids 0
to 2,473; the server's `defs.mobs` holds 2,747, of which 2,474 to 2,746 are the original's placeholders with no client record). Each is a
copy of the client's own record m, with:

| field | value | why |
|---|---|---|
| `day` | by the wild record's: 0 → 6, 1 → 6, 2 → 0, 3 → 0, 4 → 1, 5 → 4, 6 → 6, 7 → 6 (others kept) | about 40 % of the wild size (decision 7): 0.6 → 0.4, 1.0 → 0.4, 1.3 → 0.6, 1.6 → 0.6, 2.0 → 1.0, 5.0 → 2.0, 0.4 → 0.4 |
| `size` | 0 | cannot be clicked or targeted |
| `hp` | 1 | a target bar never divides by zero |
| `quest_item` | 0 | |
| `normal_time`, `normal_step` | 1000, 4 | walk 3.7 a second (5.3) |
| `chase_time`, `chase_step` | 1000, 10 | run 9.7 a second |

Everything else (name, model, level, ai, element, the attack animations) is the wild record's, so the pet looks, walks, runs and idles as
that monster. The file becomes 5,474 records: the 2,474 originals unchanged, **2,474 to 3,000 fillers**, then 3,001 to 5,473 the pets. A
filler is a record with an empty name, model −1 (`0xFFFF`) and zeros: the model index is bounds-checked (`0x432e80`), so *inferred* a filler
draws nothing, as an id past the end does today; section 11 entry B and the live test (section 12 item 1) confirm it before patch 111 goes
to players. About 125 KB more in the file.

The base 3000 leaves room for the server's own monsters to grow (2,747 today) and makes "pet of monster m" readable in logs. The
`pet_egg` effect's `base` names it (section 7.2); `base = 0` means the wild record itself (full size, clickable, its own speed): a mode for
trying pets before a client has the patch, and the behaviour a client without patch 111 cannot give otherwise (an unpatched client sent
record 3123 shows nothing at all, harmlessly).

### 6.3 The client tables and the generator

Chunk 1 left monsters out of the extendable tables because `Monster.SData` was not generated (`ext-1-foundation.md` section 5.5). Now:

- **`client.monsters`**: the original patch-108 `Monster.SData` imported by `tools/sdata/load_client.py` beside `client.items` and
  `client.skills` (the reader is `client_sdata.py`'s `read_monsters`, which rebuilds the file byte for byte; columns `mob_id`, `name` and
  `MONSTER_FIELDS`). It is a client table like the others: in `content hashes client` if those are hashed, imported once as they are.
- **`content.client_monsters`** (migration 0035): the records extensions add, **one row per record**: `id integer PRIMARY KEY`,
  `extension text NOT NULL REFERENCES content.extensions`, `copy_of integer NOT NULL` (the `client.monsters` record copied), `fields jsonb
  NOT NULL` (the fields set, by `MONSTER_FIELDS` name; `name` allowed), `since_patch integer`, `note text NOT NULL`.
- **`tools/sdata/client_patch.py`** generates `Monster.SData` when `content.client_monsters` has rows, as it does `Item.SData` and
  `Skill.SData`: the originals from `client.monsters`, the fillers, then each row's copy with its fields, every value checked against its
  width (`width_value`) and the name against the longest (check 7). With no rows it generates no `Monster.SData` and leaves the file out of the
  patch, so **patches 109 and 110 still rebuild byte for byte** (`client_patch.py rebuild --patch 109` and `--patch 110`: MATCH, on the drill).
  `generate` records `since_patch` as for items. `remove` of the extension deletes its rows; the next whole-file patch drops the records
  (`docs/plans/done/client-patches.md`, the ledger's rule for a removed row).

### 6.4 `[[monster_look]]` in `rows.toml`, and `monsters` in `[ids]`

A new entry kind of the extension manager (`crates/data-editor/src/ext/tables.rs` gains its line: a client-only entry, no `defs` row):

```toml
[[monster_look]]
of = "1-2473"                 # the client's own records copied: ids and ranges, as rule lists are written
base = 3000                   # the copy of record m is record base + m
day = { "0" = 6, "1" = 6, "2" = 0, "3" = 0, "4" = 1, "5" = 4, "6" = 6, "7" = 6 }   # the copy's Day by the original's; missing: kept
set = { size = 0, hp = 1, quest_item = 0, normal_time = 1000, normal_step = 4, chase_time = 1000, chase_step = 10 }
note = "pets: small, unclickable copies"
```

`ext install` expands an entry into its rows of `content.client_monsters`. Checks, each naming the file, the entry and the key as chunk 1's
do: every `of` id exists in `client.monsters` (without the `client` schema: is 0 to 2,473, the constant of `ids.rs` with its source); every
`base + m` lies in the extension's `[ids] monsters`, at most 65,535, and is **not a `defs.mobs` id** (a record there would change how that
server monster looks); the `day` keys are 0 to 255 and values 0 to 255; `set` names `MONSTER_FIELDS` (or `name`, printable ASCII) with values
in their widths; two entries of one extension may not make the same record.

`[ids]` gains `monsters = ["3001-5473"]` (stored as kind `monster`, `major` 0, `minor_low`/`minor_high` the ids; migration 0035 widens the
kind's `CHECK`). The ceiling: from the server's monster count (`defs.mobs`, 2,747) to 65,535 (the u16 of `0x0601` and the file's count,
entry 11 a); overlaps between extensions are refused as for items. `ext show` and `ext check` print the entry as one line (`monster_look
1-2473 → 3001-5473, day mapped, 6 fields set`), not 2,473.

## 7. The two item effects

Added to chunk 3's registry (`crates/game-server/src/effects.rs`: a variant of `ItemEffect`, its arm in `check_effect`, `fits_row`,
`describe`, `request` = `Use`; `docs/server/effects.md` gains their rows). An extension may give them only to its own items, as every effect.

### 7.1 `capture`

| key | value | default |
|---|---|---|
| `egg` | `"type/type_id"` of the item that becomes the egg; it must have a `pet_egg` effect of the same extension | required |
| `below` | 1 to 100: the HP percent at or below which a catch can be tried | 30 |
| `chance` | `[c0, c1]`, each 0 to 100: the chance at `below` and near death (section 3.3) | `[10, 60]` |
| `range` | 1 to 60: how far the monster may be from the user | 15 |
| `levels_above` | 0 to 255: the monster's level at most the user's plus this | none (any level) |
| `except` | monster ids and ranges never caught (`[1234, "600-620"]`) | none |

`fits_row`: an item the core uses (type 25, 44 or 100), as `level_up`. The `egg` link is checked when the server loads the effects
(`pg::load_item_effects`): an `egg` without a `pet_egg` effect of the same extension makes the capture effect **skipped with a warning**, as
chunk 3 skips an effect that no longer suits its row.

### 7.2 `pet_egg`

| key | value | default |
|---|---|---|
| `base` | 0 to 65,535: the pet of monster m is record `base + m`; 0 is the wild record (section 6.2) | required |
| `monsters` | monster ids and ranges that have a pet record | required |
| `loot` | `true`/`false`: the pet fetches its owner's drops | `true` |
| `loot_range` | 5 to 30: how far from the owner the pet goes for a drop | 15 |
| `gold` | `true`/`false`: gold too (decision 8) | `true` |

`fits_row`: an item the core uses. At load: the row's `count` must be 1, and with `base` above 0 every `base + m` must lie in the
extension's `monsters` range (`content.ext_ranges`); otherwise the effect is skipped with a warning naming the item and the reason.
`base + m` above 65,535 is a parse error.

### 7.3 Disabled

With the extension disabled the items do what their own columns say: the core's refusal (`use_failed`, special 0 with no HP, SP or MP, as
the level stones; `ext check` already warns of this). An egg in a bag stays an egg; nothing comes out.

## 8. Persistence

`public.character_pets` (migration 0035): `character_id integer PRIMARY KEY REFERENCES characters ON DELETE CASCADE`, `egg text NOT NULL`
(the craft name), `changed timestamptz NOT NULL DEFAULT now()`. Written only when a pet comes out by an egg's use (an upsert) and when it is
dismissed by a use or because its egg left (a delete); not at hiding, map changes or logout. `Backend` gains `pet(character) ->
Option<String>` and `set_pet(character, Option<&str>)` (the PostgreSQL backend and the memory backend of the tests).

At **login** (once the character is in the world, after the first `enter`), a character with a stored pet whose egg is in its bags gets it
out (the same path as 4.3 step 4, without the store); an egg no longer in the bags clears the row. The connection keeps the pet out as
`WorldState::pet: Option<ActivePet { egg, look }>`, which every later `enter` (map change) uses to summon it again.

The menu of decision 1 (entry 10 e): the client has **no** server-opened window with free buttons (the NPC talk box reads only
`NpcQuest.SData`; `0x0701` fills a shop list; the dialogs are fixed yes/no texts). The egg is the menu: use to call, use to send back. A
real pet window is DLL work (`research-extension-features.md`, "A DLL overlay"); not in this chunk.

## 9. Rules

Two new `deny` points (`crates/game-server/src/rules/`: `Point::Capture` named `"capture"`, `Point::Pet` named `"pet"`; one-player points, so
`who` is an error on them; `items` stays `item_use`'s own key):

| point | asked | scope tested | default text |
|---|---|---|---|
| `capture` | at a capture item's use, before the target is looked for (3.1 step 1) | the user and its map | `You cannot catch monsters here.` |
| `pet` | at a summon (4.3), at the arrival on a map with a pet out, and every 5 s for a pet out (5.4) | the owner and its map | `Pets cannot come out here.` |

A refusal of `capture` is whispered as every refusal (rate limit included) and the capture item stays, its reuse time untouched (the
handler runs after the reuse group, so it puts the time back as chunk 3's handlers do on `Done::Failed`). A `pet` match at a summon is
whispered and nothing comes out; a `pet` match on a pet already out hides it and whispers once (5.4).

`deny item_use` with the egg's or the capture item's id also works (it refuses the use itself), but it does not hide a pet already out;
`docs/server/pets.md` says to use the two points. Examples for `docs/server/rules.md` and the extension's README:

```toml
# No pets in the capitals and the Auction House.
[rules.no-pets-in-town]
type = "deny"
point = "pet"
maps = [35, 36, 42]

# Catching only from level 10.
[rules.capture-from-10]
type = "deny"
point = "capture"
level = "1-9"
message = "You can catch monsters from level 10."
```

The extension itself ships **no** rule in force: its `extension.toml` carries these two, commented out.

Scripts: no new script point or event in this chunk. A script can still take the two items over with `game.on_item` (chain step 4), and
the `item` table's `effect` field names `"capture"` or `"pet_egg"` (chunk 3). A capture event for scripts is a later API version.

## 10. Auto-loot

### 10.1 What the pet fetches

A ground item is the pet's to fetch when all hold:

- it is a **monster's drop** (`GroundItem::from_mob()`) whose **owner is the pet's owner** (`GroundItem::owner`, the killer of record, the
  loot rule's member on the ground or the union leader, `zone.rs` drops). Not another player's item even after its 30 owner-only seconds;
  not what a player dropped (`Dropped::player`) or lost at a death (`Dropped::death`);
- gold only when the egg's `gold` is true; a quest item (real type 27) only when the owner's quests take it (the same `quest_ok` test a
  pick-up asks);
- it lies within `loot_range` of the **owner** and is not in the pet's `skipped` list.

Party gold is shared straight into the members' money and a party's items mostly go straight into a member's bags (`zone.rs`, the loot
rules): only what lands on the ground with the owner's id is fetched, as the decision says ("only its owner's drops").

### 10.2 The fetch

In the pets pass, a following pet whose egg has `loot` and whose owner is alive and not in a trade, a personal shop or a gamble (the
connection reports these through the state the zone already holds; the build uses the flags `Placed` has, or adds one) picks the item
**nearest to the pet**, then:

1. **Fetch**: `0x0603` run to the item's spot; `arrive = now + distance / PET_RUN`. If the item goes meanwhile (picked by its owner,
   expired), or the owner gets more than `loot_range + 10` away, the pet goes back to following.
2. **At arrival**: the zone sends the owner's connection `ZoneEvent::PetLoot { item }` and the pet waits (`Waiting`, at most 5 s; then the
   item is skipped and the pet follows).
3. **The owner's connection** runs the pick-up of `server.rs` (`pick_up`, `into_bags`, `picked_up`, the gold path with the item flags'
   bonus, the game log's pick-up records) with two differences:
   - it calls `Zones::pet_pick_up(map, owner, item, now, quest_ok)` instead of `pick_up`: the item must still be there and be the owner's
     (strictly, 10.1), and the range of 10 (`PICK_RANGE`) is measured from the **pet**, not the owner; the view gets `0x0402` as for any
     pick-up;
   - **before** taking it off the ground, it asks whether the item fits (`Inventory::pick_up` without applying it; gold: the money cap). If
     not, nothing moves: the item **stays on the ground untouched** (no `put_back`, so its lifetime is not renewed), the owner gets the normal
     "no room" answer (`0x0205` error `PICK_NO_ROOM`) once for that item, and the zone adds it to the pet's `skipped` list
     (`Zones::pet_skip`).
   A pick-up does not cancel the owner's actions (`cancel_actions` is the player's own pick-up's, not the pet's).
4. The owner sees exactly a pick-up: `0x0205` with the item ("Attained <i>") or the gold line, the sound (entry 10 c: an unsolicited `0x0205`
   is shown as a real one). Other players see the item vanish (`0x0402`) as the pet stands on it.
5. The pet follows again and looks for the next item.

`skipped` holds item ids (cleared when the pet hides or changes map, and pruned of ids no longer on the ground at each check).

## 11. For the client side

Entries for `docs/testing/client-research-queue.md` (numbered after the last, 27; the build adds them, step 10). None blocks the server
build: each has a server-side fallback that is already the design.

- **A. The egg's tooltip line (a DLL feature).** For items of type 100 (and 25, 44), the tooltip `0x49fa00` clears the craft-name pointer. A
  `fixes.dll` feature `pet-egg-tooltip`: for the item 100/228 only (or any item whose craft name starts with `P` followed by five digits and
  a dot), add one line "Contains: <name>", the name being `Monster.SData` record `<digits>`'s (`0x461210`). What it changes: the egg names its
  monster in the bag. Fallback (built): every egg reads "Pet Egg". The research found this *inferred* small (entry 10 b).
- **B. Appended `Monster.SData` records.** With patch 111: does a filler record (empty name, model −1, zeros) behave as a missing record when
  a server sends its id (nothing drawn, no crash, as `0x461210` returning null does today), and are records 3,001 to 5,473 loaded (the count
  read from the file, `0x460e20`) with their Day class and Size 0? What it changes: the filler's values. Live test: section 12 items 1 and 3.
- **C. `0x050A` for an item that stays.** The success answer of a use with the slot's count unchanged (the egg): does the client keep the item
  shown, its count and its reuse time right? What it changes: whether the egg answers `0x050A` or something else. Live test: item 4.
- **D. The selected target (a refinement).** Does the client send `0x0304` or `0x0305` when the player selects a monster (the target bar
  asking for HP), or only when a monster enters view? If on selection, the server can capture the selected monster instead of the weakest one
  fighting the player (section 3.1). What it changes: one line of the capture handler. No live test needed.
- **E. Patch 111.** The generated patch (Item.SData with 100/227 and 100/228, Monster.SData with the pets) is the client side's to pack,
  sign and hand out, as patches 109 and 110.

## 12. Live-test items (what one real client must show)

The build adds these to `docs/testing/live-test-checklist.md` as **section 63, "Pets"** (62 is the enchant tables), each with what to look
for and the console or GM commands to set it up:

1. **Patch 111 changes nothing else**: before installing `pets`, walk the maps you know: monsters look as before; spawn a placeholder monster
   whose id is a filler (2,500) with the GM's monster command: nothing shows, nothing crashes (entry B).
2. **Catch**: fight a normal monster, bring it under 30 % HP, use a capture item: "It broke free!" (item gone) or the monster vanishes,
   "Attained Pet Egg", "Caught! …". The monster respawns at its usual time.
3. **Refusals**: a healthy monster ("too strong"), nothing fighting you, a boss, a quest monster, a monster fighting your party member but not
   you; the item stays each time. A full bag: "Your bags are full."
4. **Summon and dismiss**: use the egg: a small pet of that monster beside you (about 40 % of its wild size); the egg stays with its count
   (entry C); use it again: it goes. Try to click the pet: nothing happens.
5. **Following**: walk, run, ride a mount, ride with a partner, take a portal, recall, die and revive, go invisible as staff: the pet keeps up,
   snaps after a teleport, hides while dead or invisible, comes back.
6. **What others see**: a second character sees the pet with the monster's name, walking and running with its animations; it cannot target it.
7. **Auto-loot**: kill monsters: the pet runs to each drop and you get "Attained" and the gold line; a party member's item stays; your own
   dropped item stays; a full bag: one "no room", the item stays and the pet leaves it.
8. **Relog**: log out with the pet out and back in: it comes back. Trade the egg away: the pet goes at once; the other player summons the same
   monster.
9. **Rules**: `deny pet` on a map: the pet hides on arrival with the message, returns on leaving; `deny capture` by level.
10. **Off**: `data-editor ext disable pets`, restart: both items are refused, no pet comes out, nothing else differs. `base = 0` (the wild
    record) on a client without patch 111: a full-size, clickable pet that the server ignores when attacked.

## 13. Build

Steps in order, **one commit each**, `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before each commit (CI enforces
both), the game server's and the data editor's tests after each step that touches them (one cargo command at a time: the machine is short on
memory). Rehearsed on the drill (`.env`: `shaiya-afterglow-ext1`, port 5434; the drill's game server on `:31810`, restarted with
`(GAME_BIND=0.0.0.0:31810 nohup tools/game-server.sh restart >/dev/null 2>&1 </dev/null &)`), never on `newshaiya`. Stage named paths only.

1. **Schema.** `db/migrations/0035_pets.sql`: `content.client_monsters` (6.3), the `monster` kind in `content.ext_ranges` (and any other
   kind `CHECK` the extension tables carry), `public.character_pets` (8). `tools/sdata/load_client.py` imports `client.monsters`; whatever
   runs it in `tools/setup.sh` runs it for monsters too (a re-run skips it as for items). Tests: the migration on a fresh database; the
   import rebuilds the file byte for byte (`read_monsters`' own check). Commit: "Pets 1: client monster table, extension monster kind,
   character pets".
2. **Generator and `[[monster_look]]`.** `client_patch.py` generates `Monster.SData` (6.3); its tests (`test_client_patch.py`: no rows → no
   file; fillers; a copied record with a day map and fields; width and name checks; the record count). The data editor: the `monsters` key of
   `[ids]`, its ceiling (`ids.rs`), the `[[monster_look]]` entry (6.4) with its checks, `ext show`/`check` output, `ext remove` deleting the
   rows; tests for each check. On the drill: `client_patch.py rebuild --patch 109` and `--patch 110` still MATCH. Commit: "Pets 2:
   Monster.SData from the database, monster_look rows".
3. **Rule points.** `capture` and `pet` (9): parse, `who` refused, default texts, `game-server rules --check` examples, the rules' tests;
   `docs/server/rules.md` section 3 and 9 (the points are eleven). Commit: "Pets 3: deny capture and deny pet".
4. **Effects.** `capture` and `pet_egg` (7): parse with every default and error, `fits_row`, `describe`, the load-time links (egg, `count`,
   ranges) with their warnings, `Done::Kept`; tests of the parser and the links; `docs/server/effects.md` rows. Commit: "Pets 4: capture and
   pet_egg effects".
5. **The pet in the zone.** `zone/pets.rs`: `PetMap`, `PetLook`, `summon_pet`, `dismiss_pet`, `leave` removing the owner's pet, the view
   paths (`enter`, `swap_view`, the pet's move), the pets pass with following, snapping and hiding (dead, invisible, the `pet` rule every
   5 s), `0x0304`/`0x0305` for pets; `server/tasks.rs` runs the pass. Zone tests (`zone/tests.rs`): a summon shows `0x0601` (record) and
   `0x0608` (1, 1) to the view; a player coming into view gets it; the follow spot, walk under 8, run above, snap above 20; hidden while the
   owner is dead or invisible and shown after; a `deny pet` rule hides it and lifts; the pet is in no monster pass (`attack_mob` on its id
   does nothing, an area skill around it hurts nothing, `mob_count` unchanged); a second summon replaces the first. Commit: "Pets 5: pets in
   the zone".
6. **Summon, dismiss, persistence.** `server/pets.rs`: the `pet_egg` handler (4.3), the craft-name parser (4.1, with tests of empty and
   malformed names), `WorldState::pet`, the summon after every `enter`, the egg-left check after each request and zone event, `Backend::pet`
   and `set_pet` (PostgreSQL and memory), the login path (8). Tests (`tests/game_flow.rs` or the server's own): use an egg → pet out and
   stored; use again → gone and cleared; another egg → replaced; a map change → out on the new map; relog → back; the egg traded, dropped,
   stored in the warehouse → dismissed and cleared; an empty egg refused; the extension disabled → refused and nothing out. Commit: "Pets 6:
   summon and dismiss by egg, kept across logins".
7. **Capture.** The `capture` handler (3.1), `Zones::capture` (3.4), the chance (3.3), the egg's craft name with the zone's counter. Tests:
   the weakest fighting monster chosen; each refusal of 3.1 and 3.2 (healthy, not fighting, a party member's, a boss group's, a quest
   monster, a Lua, scripted and script-spawned monster, out of range, `except`, `levels_above`, full bags) with the item kept; the chance at
   30, 20, 10 and 1 % with a fixed random source; a miss uses the item; a hit: the monster vanishes (`0x0602`), no `Kill` event, no ground
   item, no experience, the egg in the bag with the right craft name in one save, and after `remove_bodies` the spawn entry counted down and
   respawning. Commit: "Pets 7: capture".
8. **Auto-loot.** The fetch (10.2), `ZoneEvent::PetLoot`, `Zones::pet_pick_up`, `pet_skip`, the fit test before the pick. Tests: only the
   owner's monster drops (not a party member's, not a player's drop, not a death drop), gold by the option, the quest-item test; nearest
   first; the arrival time; the pick from the pet's spot; a full bag leaves the item untouched with one `PICK_NO_ROOM` and skips it; the
   5-second wait; nothing while the owner is dead or trading; the game log's pick-up records. Commit: "Pets 8: auto-loot".
9. **The extension.** `extensions/pets` (section 13.1): `extension.toml`, `rows.toml`, `README.md`. `ext check` and
   `ext install` on the drill, a drill start (the effects load without warnings), the whole game-server suite once. Commit: "Pets 9: the pets
   extension".
10. **Docs and the rehearsal.** `docs/server/pets.md` (the operator's page: install, the items and their keys, the rules, what players see,
    the client patch, `base = 0`, how players get capture items); `docs/server/effects.md` and `rules.md` cross-links; the client-research
    entries of section 11 in `docs/testing/client-research-queue.md` (28 to 32); section 63 of `docs/testing/live-test-checklist.md`
    (section 12); `README.md` (features, test count), `CHANGELOG.md` (0.4.0 unreleased: pets, migration 0035, the new rule points and effects,
    `[[monster_look]]`), `docs/plans/roadmap.md` and `docs/plans/extensions.md` 7.4 (built), this file's top line (built, and what the build
    settled where the text left a choice). On the drill: install and enable `pets`, restart, check a catch, a summon and a pick with the
    debug log. Commit: "Pets 10: docs, checklist, client entries".

Not in the build, each **asked first**: the live dev box (`shaiya-afterglow-live`: a `pg_dump` first, then fetching these commits from this
clone, migration 0035, `ext install extensions/pets` left **disabled**, as chunks 3 and 5 were); generating client patch 111 there; any push;
the docs site.

### 13.1 The extension's files

`extensions/pets/extension.toml`:

```toml
# Pets: catch a weakened monster with a Capture Net, call it out of its Pet Egg as a small follower that fetches your drops
# (docs/plans/ext-4-pets.md, docs/server/pets.md). Not installed by tools/setup.sh.

format = 1
name = "pets"
version = "1.0.0"
title = "Pets"
description = "Catch monsters, call them as small followers that pick up your drops."
authors = ["Shaiya Afterglow"]
license = "MIT"

[ids]
items = ["100/227-228"]
monsters = ["3001-5473"]

# Examples, off: uncomment to use (docs/server/rules.md).
# [rules.no-pets-in-town]
# type = "deny"
# point = "pet"
# maps = [35, 36, 42]
```

`extensions/pets/rows.toml` (the placeholders 100/227 and 100/228 are "???177", grade 999, free in both copies; the build confirms they are
still free and owned by no extension):

```toml
[[item]]
id = "100/227"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "drop", "server", "count"] }
set = { itemname = "Capture Net", grade = 999, count = 10 }
effect = { type = "capture", egg = "100/228", below = 30, chance = [10, 60], range = 15 }
client = { name = "Capture Net", look = "100/108", description = "Use it on a weakened monster that is fighting you (below 30% HP) to catch it in a Pet Egg. Bosses and quest monsters cannot be caught." }
note = "pets: the capture item"

[[item]]
id = "100/228"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "drop", "server", "count"] }
set = { itemname = "Pet Egg", grade = 999, count = 1 }
effect = { type = "pet_egg", base = 3000, monsters = ["1-2473"], loot = true, loot_range = 15, gold = true }
client = { name = "Pet Egg", look = "27/147", description = "A caught monster. Use it to call your pet; use it again to send it back. Your pet picks up your drops." }
note = "pets: the egg"

[[monster_look]]
of = "1-2473"
base = 3000
day = { "0" = 6, "1" = 6, "2" = 0, "3" = 0, "4" = 1, "5" = 4, "6" = 6, "7" = 6 }
set = { size = 0, hp = 1, quest_item = 0, normal_time = 1000, normal_step = 4, chase_time = 1000, chase_step = 10 }
note = "pets: small, unclickable copies"
```

The look of 27/147 is "Eggs of Wyvern"'s icon; whether a non-quest item may take a quest item's look is the generator's check 7 and the
live test's (item 4); if not, the build picks another egg-like icon and says so. `count = 10` stacks the nets; the stack size the client
shows comes from the generated row. Both items are grade 999 (no monster drops them): players get them from the item mall, a GM's `/make`,
or a script's `give_item`; `docs/server/pets.md` says how to add a mall product or a drop entry.

## 14. The Sonnet build prompt

```
Build extensions chunk 4, pets, in /media/drive0/projects/shaiya-afterglow (main), from docs/plans/ext-4-pets.md: section 13, steps 1 to
10 in order, one commit each, with the commit messages given there. Read the spec whole first, then docs/server/effects.md, rules.md,
scripts.md, and the code each step names before changing it. Where the spec and the code disagree, or a choice is not the spec's to
make, stop and ask the maintainer with a concrete example.

Conventions: Rust via ~/.cargo/bin/cargo; one cargo command at a time (the machine is short on memory); before each commit cargo fmt
--all and cargo clippy --all-targets -- -D warnings must be clean (CI enforces both); stage named paths only, never git add -A (other
sessions share the tree); no mention of AI in commits or code (no Co-Authored-By or session trailers); the checkout's .env points at the
drill (shaiya-afterglow-ext1, port 5434): never touch newshaiya. After code changes rebuild and restart the drill's game server with
(GAME_BIND=0.0.0.0:31810 nohup tools/game-server.sh restart >/dev/null 2>&1 </dev/null &) — never a plain restart in this clone.
On the drill after step 2: tools/sdata/client_patch.py rebuild --patch 109 and --patch 110 must still MATCH.

Do not, without asking: push, touch the live dev box (shaiya-afterglow-live), generate client patch 111 there, deploy the site.

Step 10 adds the live-test items as section 63 of docs/testing/live-test-checklist.md (the spec's section 12, ten items) and the client
entries A to E of section 11 to docs/testing/client-research-queue.md as 28 to 32. At the end, update the spec's top line with what the
build settled, and finish with: the commits, the test count, what is untested (the checklist items), and a ready-to-paste prompt for the
next session (the live dev box rehearsal and patch 111, on the maintainer's OK) with a model and effort recommendation.
```

## 15. The client-side prompt

```
Client side, pets (server spec docs/plans/ext-4-pets.md in byyako/shaiya-afterglow, section 11; entries 28 to 32 of
docs/testing/client-research-queue.md once the server build adds them). Read-only reading of game.exe (US 2010-04-26, patch 108) unless a
DLL feature is agreed; answers go to docs/client/research-extension-features.md under a new "Pets" heading, and each answered entry moves
to "Answered" in the queue.

A. A fixes.dll feature "pet-egg-tooltip": in the item tooltip 0x49fa00, for item 100/228 (or any item whose craft name is "P" + five
   digits + "."), add one line "Contains: <name>" with the name of Monster.SData record <digits> (0x461210). Say where the line is added,
   what the craft name pointer holds at that point for type 100 (the function clears it), and how the feature is switched on.
B. Monster.SData with appended records (patch 111, generated by tools/sdata/client_patch.py): a filler record (empty name, model -1, all
   zeros) sent in 0x0601 — nothing drawn and no crash, as an id past the end today? Records 3001-5473 (copies of 1-2473 with Day mapped,
   Size 0, HP 1, normal 1000 ms/4, chase 1000 ms/10): loaded, scaled, unclickable, walking at 3.7 and running at 9.7 units a second?
C. 0x050A success with the slot's count unchanged (the egg stays): the item stays shown with its count and reuse time?
D. When does the client send 0x0304 / 0x0305: on selecting a monster, or only when it enters view? (If on selection, the server can
   capture the selected monster.)
E. Pack, sign and hand out patch 111 when the maintainer has generated it on the dev box, as patches 109 and 110.
```
