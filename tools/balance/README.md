# tools/balance: the balance model

A model of how characters fight in Shaiya Afterglow, built from the server's own formulas and the game's data, for the balance analysis
(`docs/plans/balance/1-formulas.md` explains every formula; parts 2 and 3 use these tools). Analysis only: nothing here writes to the
database, the game or the client.

Python 3.11 or later, standard library only. The data comes read-only from the dev database through `docker exec newshaiya-postgres
psql` (as `tools/defs/verify.py` does) and is cached as CSV in `data/balance/cache/` (git-ignored); `--refresh` reads it again.

## Files

| file | what |
|---|---|
| `formulas.py` | a line-by-line port of the numbers in `crates/game-server/src/`: `stats.rs` (maxima, stat totals, speeds, crafted stats, faction grade), `combat.rs` (attack and defence numbers, hit chances, every damage formula for players, monsters and player kills, experience and levels), `skills.rs` (passives, abilities), `buffs.rs` (what applied skills and debuffs add up to, give up SP/MP), `lapis.rs` (enchant steps, elements, lapis), `kc.rs`, `creation.rs`, `item_use.rs` (stat and skill resets), the recovery tick of `zone.rs`. Every function names its Rust original; the rolls are taken in the Rust order, so a test can feed the same `rand()` sequence |
| `expect.py` | expected values of those rolls: hit and critical chances exactly, damage means averaged over the attack spread and the luck roll (a grid of at most 17 values each), with the floors, caps, "10 or less becomes 1 to 10" and absorption applied at each point |
| `data.py` | the definitions as the server's loaders read them (`pg.rs`): items, skills, monsters, the status and experience tables, the faction kill-count grades, the client's English names, spawns and placements, and where items come from (`origins()` with sides and chances, `sources()` its kinds: shops, drops, sequence items, boxes, quests, the start, the mall) |
| `model.py` | builds (class, race, mode, level, stats, gear, skills, buffs, grade), the character sheet, fights against a monster and against a player, the experience curve; a command line |
| `content.py` | part 2's inventory: CSV tables in `data/balance/` (items, brackets, Light/Fury pairs, duplicates, item skills, lapis, tools, enchant costs, skills, per-class skills at 15/30/60, monsters, every origin, the client's differences); `--quick` leaves out the model's numbers, `--only items,skills,…` |
| `content_report.py` | the Markdown tables of `docs/plans/balance/2-content.md`, from those CSVs |
| `builds.py` | part 3's builds (`docs/plans/balance/3-classes.md`): the weapon, the skills within the points, the stat split and lapis order searched on PvE and PvP for the twelve classes at 15, 30, 55 and 60 in every mode; their fights against the yardstick monsters and each other (12 × 12 in variants), the faction steps, the modes and the levers. CSV tables `data/balance/builds*.csv`; every build is a model string. About eight minutes with `--jobs 4`; `--only plans,search,final,variants,pvp,factions,modes,levers` |
| `builds_report.py` | the Markdown tables of the part 3 document, from those CSVs (`--section builds,pve,pvp,…`) |
| `scenarios.py` | the sample tables of the part 1 document: every class of both factions at levels 15, 30 and 60 against a common monster, and the mirrored pairs |
| `check_saved.py` | end-to-end check: the maxima the model computes for every character in the database next to what the server saved |
| `test_formulas.py` | the Rust unit tests replayed against the port, hand-worked PvP blows, and the expected values checked against sampling the exact functions |

## Use

```
cd tools/balance
python3 -m unittest test_formulas          # no database needed
python3 check_saved.py                     # needs the dev database
python3 model.py char "fighter human ultimate 60"
python3 model.py char "mage elf ultimate 60 alloc=int:80,wis:20 enchant=10" --json
python3 model.py mob  "archer ultimate 60" --mob 1547 --mob 2461
python3 model.py pvp  "ranger ultimate 60" "assassin ultimate 60 without=183"
python3 model.py curve --mode hard
python3 scenarios.py --levels 60 --without 180,182,183,184
```

A build is a string: the class (Light names fighter, defender, ranger, archer, mage, priest; Fury warrior, guardian, assassin, hunter,
pagan, oracle; or `job=0..5`), optionally the race (human, elf, vail, deatheater; default the first race of that class on that side),
the mode (easy, normal, hard, ultimate; default ultimate), the level (default the mode's top), then `key=value` parts:

| key | meaning |
|---|---|
| `alloc=str:70,dex:30` | where the free stat points go (shares); default all into STR (melee and ranged classes) or INT (mage, priest) |
| `stats=a,b,c,d,e,f` | base STR, DEX, REC, INT, WIS, LUC as stored, instead of `alloc` |
| `gear=auto\|none` | `auto` (default): the rough picker, the best-scoring item per slot that the character may equip and can get (shop, drop, box, quest or start; `mall=1` adds the item mall); `none`: naked |
| `weapon=6` | the weapon real type the picker must take |
| `enchant=20`, `armour=10` | the weapon's enchant step, and every craftable armour piece's step |
| `shield=1` / `shield=0` | take a shield (only with a one-handed weapon; default only the defender) |
| `item=slot:type/typeid+30/40@3#0,10,0,0,0,0,3,0,0` | put an item in a slot, with lapis (type 30 ids) after `+`, its enchant step after `@` and its nine crafted numbers (STR, DEX, REC, INT, WIS, LUC, HP, MP, SP) after `#`; repeat for more slots |
| `craft=exp\|good\|top`, `craftorder=hp,str,dex` | crafted numbers on every craftable item: the item's count of numbers on the wanted stats in order, each the mean, the tenth-best or the highest of the smallest of three draws (`model.crafted`) |
| `lapis=35`, `lapis=35:str,hp` | every empty lapis room but the cloak's filled with the linkable lapis (obtainable, not dangerous, grade 31 to 35) worth most by the wants (`model.pick_lapis`); the picker then values items' rooms |
| `element=1` or `element=1,2` | a basic element lapis (grade 34) in the weapon (unless it has its own element) and the upper armour |
| `skills=auto\|none\|id/level,...` | `auto` (default): every skill the class may learn at this level and mode, at its highest level (skill points are reported, not enforced) |
| `without=180,174` | skill ids to leave out |
| `buffs=id/level,...` | applied skills: `wType` 82 and 91 abilities, the attack (94) and defence (95) element buffs |
| `grade=N` | the faction's kill-count grade, 0 to 30 |

## What the fight model does and does not do

- **Expected values**, not a dice simulation: each action deals its mean damage (hit chance × mean landed damage). The variance of a
  real fight (a streak of criticals, a run of misses) is not shown.
- **The rotation** is greedy: at each turn of the attack timer, the ready, affordable action with the most damage per millisecond of
  timer it uses (a cast adds its ready time). A skill's cooldown starts when it goes off. Normal attacks and skills share the one attack
  timer (`docs/original/skills-use.md` section 4). Damage over time pays every 3 s, a recast restarts it. Buff skills are not cast in
  the rotation: give them with `buffs=` if they should count.
- **SP and MP** are not regained while fighting (the original gives only the faction grade's fight recovery), so the "pool" kill time
  stops using skills when they run out; the "unlimited" time stands for potions.
- **Against a monster**, its attacks run in the order of `CMob::AttackAI` (entries 2 to 0 when ready; the HP-conditioned entries are
  skipped at full HP); its skills are sampled from the exact functions. Range, chasing, aggro, other monsters, a party, healing, item
  option skills (lapis procs), blocks and reflection are not modelled.
- **Against a player**, both sides act at once from full health and at range (`--close` for within 3). Blocks, reflection, debuffs'
  lasting effects (stun, slow), healing, the lapis option skills (including the near-death and death lapis), movement and line of sight
  are not modelled.
- **Area skills** count one target. **Multi-hit** skills roll each hit. **Drain**, **sacrifice** and the **desperate attack** (`wType` 23)
  are left out of the rotation (the last kills its user).
- **The gear picker** is rough (a score of attack, defence, stats and HP, plus the lapis its rooms will hold and its crafted numbers when
  `lapis=` or `craft=` is given); part 2 rates items properly. Prefer explicit `item=` for real comparisons.
- **Damage type 5** (a share of the target's current HP) is taken of what is left of the target when the skill goes off; a skill that
  costs the whole SP or MP pool is used only when no other skill is ready and affordable (both added in part 3; part 1's sample
  tables are unchanged by them).

## Validation

- `test_formulas.py` replays the unit tests of `stats.rs`, `combat.rs`, `lapis.rs`, `kc.rs` and `item_use.rs` with their inputs and
  roll sequences (23 test functions), plus hand-worked PvP blows, and checks `expect.py` against 40,000 samples of the exact functions.
- `check_saved.py`: of the 9 characters in the dev database, the 8 saved at full health show exactly the model's maxima;
  the ninth's HP was saved as 32,767, the `smallint` limit of `characters.hp`, below the model's 56,713 (a test character with GM
  items).
- Where the documents and the code disagree, the code wins: see `docs/plans/balance/1-formulas.md`, "Discrepancies".
