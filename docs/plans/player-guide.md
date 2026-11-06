# The player guide: spec and build plan

The site's home page promises "a plain-language guide to the game's mechanics next to this reference". This plan starts it with what
the maintainer asked for first: **a comprehensive breakdown of the player stat system**: what each stat does, how attacks, defences and
resistances use them, and every step from a character's base numbers to the HP a blow takes off.

## 1. Decisions (the maintainer, 2026-11-05)

| question | answer |
|---|---|
| Where | **A new site tab, "Player guide"**, pages in a new folder `docs/guide/` (built from `docs/` like every other page) |
| Shape | **Three linked pages** (section 3), not one long page |
| Depth | **Everything**: every formula and constant, worked examples, and the quirks a player would not guess |
| Missing facts | **Reverse `ps_game.exe` or `game.exe`** to settle what no note has yet (read-only: nothing run, nothing changed) |
| What it describes | **The original** (EP4.5 `ps_game.exe` and the US client), as the rest of the site does; each page ends with a short note on what Shaiya Afterglow does differently, if anything |

## 2. Sources

Most of the material is already written and checked; the work is gathering it into one readable place, confirming it against the
original and filling the gaps of section 4.

| source | what it gives | status |
|---|---|---|
| `docs/plans/balance/1-formulas.md` sections 3 to 6 | every formula, per-point table, the level factor table, elements, caps, enchant odds, modes, faction grades | **verified against the server's code**, which follows the original; not published (it is a plan). Its section 8 lists where the notes and the code disagree (none changes a number) |
| `docs/original/stats.md` | maxima, items, crafted stats, spending points (`0x0208`), with `ps_game.exe` addresses | confirmed in the binary |
| `docs/original/combat.md` sections 3 and 4 | the hit and damage formulas, the element table, the player's and the monster's numbers (`CUser::SetAttack` 0x454280, `CMob::SetAttack` 0x4902f0) | confirmed |
| `docs/original/skills.md` section 6, `skills-use.md`, `skills-buffs.md`, `skills-special.md` | the abilities table, skill hit and damage types, buffs, debuffs, blocks, reflection | confirmed |
| `docs/original/lapis.md` sections 2 and 5, `item-options.md` | lapis, enchant steps, absorption (`CUser::Damage` 0x44c1e0), the lapis option skills | confirmed |
| `docs/original/pvp.md`, `pvp-2.md`, `mob-fight.md` | player kills, monsters' blows, recovery, death | confirmed |
| `docs/client/research-balance.md`, "The character window" | what the character window shows and leaves out, and its own formulas | confirmed in `game.exe` |
| `crates/game-server/src/combat.rs`, `stats.rs`, `buffs.rs`, `lapis.rs`, `zone.rs` (recover) | the formulas as code, with unit tests that give known inputs and outputs | the server's code |
| `tools/balance/` (`formulas.py`, `model.py`) | a runnable copy of the formulas, for the worked examples | matches the server on the replayed tests (`1-formulas.md` section 7.2) |
| the dev database (`defs.status_defs`, `defs.items`, `defs.skills`, `defs.mobs`, `world.kill_status`) | the real numbers for the tables and the examples | data from the original files |

**The rule for each formula**: the page states it as the original's, so each one needs a `ps_game.exe` address (or a client address for
what the client does) in the reference note it links to. Where the only source today is the server's code (`1-formulas.md` names the
function), find the original's code with REA (`ps_game.exe` and its PDB are in the original server files, `PSM_Client/Bin/`;
the client is `Client/game.exe` of the original files) and add the address to the matching `docs/original/` note. If it cannot be found,
the page says "from the server's code, which follows the original" for that point.

## 3. The pages

Each page follows the site's pattern: **In short** (plain language, for players) on top, then the detail, with formulas in code blocks and
tables, and the names players know (the client's English item and skill names, `client.items` / `client.skills`, never the `defs`
Chinese names). Terms are defined the first time: the three attack kinds (melee, ranged, magic), "attack" against "damage", a percentage
point, the level gap. Links go to the reference pages for addresses and packets; the guide itself carries no addresses except where a
reader needs one to check a claim.

### 3.1 `docs/guide/character-numbers.md`: Your character's numbers

1. In short.
2. **The six stats**, one row each: everything a point adds, anywhere (the per-point table of `1-formulas.md` 3.3), and a line on what
   it does *not* do (3.7: no stat gates equipment, REC does not speed recovery, WIS's small share of magic attack, ...).
3. **Starting stats** by class and race, the class stat that rises by 1 a level, stat and skill points a level by mode, the totals at 60,
   spending points (any time, any mix; current HP/SP/MP not raised), the stat reset item.
4. **HP, SP and MP**: the status table by class at levels 1, 15, 30, 45, 60 (the full table from `defs.status_defs` as a collapsed
   block or a linked CSV), the mode factor, the race bonus, 5 a point.
5. **The derived numbers** per kind: hit, evasion, attack, defence, critical; the shared item attack; magic hit and evasion with no stat
   term.
6. **What equipment adds**: item stats and effects by slot, wear (half below half quality, nothing below a tenth; accessories and lapis
   never wear), accessories' full effects, lapis, crafted stats (HP/MP/SP × 100), enchant steps (gap G2), weapon and shield mastery.
7. **Buffs, debuffs and passives**: the abilities table (number, what it changes), the low-HP passives, transformations.
8. **Speeds**: attack speed 0 to 9 and its waits, move speed 0 to 4, what raises them.
9. **Recovery**: standing, sitting, fighting; why no stat changes it.
10. **Modes** (easy, normal, hard, ultimate): maxima, points, levels, which skills and items, death in ultimate.
11. **Faction kill-count grades**: what each grade adds, for Light and Fury, and that they are not mirrored.
12. **What the character window shows** and what it leaves out (faction grade, wear, mastery, armour enchant; no hit, evasion, critical,
    absorption), and where its numbers can differ from the server's (gap G2).
13. **Worked example A**: a level-60 ultimate fighter with a stated build (base stats, a named weapon, armour and lapis): every number
    of sections 4 to 8 worked out step by step, and what the window would show beside it.
14. Shaiya Afterglow: only a pointer (it follows the original; the deviations page has the few exceptions, for example the 32,767 stat
    limit of `docs/original/stats.md` section 5).

### 3.2 `docs/guide/hitting.md`: Hitting and being hit

1. In short.
2. **The timeline**: one attack timer, the wait by attack speed, casting (`readytime` × 250 ms, moving cancels), when the cooldown starts
   and the costs are taken, cost reductions.
3. **Hit chance**, player on monster, monster on player, player on player: the formula, its clamps (20-99, 5-95, 1-99), a small table
   of results, and the quirk that **your own evasion raises your hit chance** (DEX counts twice).
4. **Damage on monsters**: the attack roll with spread, the level factor P (formula and the table at 15, 30, 60), the element factor, the
   LUC roll, criticals (chance and size), ranged within 3, the floor (10 or less → 1 to 10) and the cap (30,000).
5. **Skills**: success types, damage types 0 to 5 (and that 3, 4 and 5 ignore defence, critical and element), multi-hit, area, drain,
   added damage, damage over time.
6. **Monsters on players**: the same P turned around, the upper armour's element, monster criticals, monster skills.
7. **Players on players**: physical (attack minus defence, no level factor), magic (the percentage cut, P1 and P2), criticals, the
   percentage skills and the two lapis that take 90 % and 100 % of maximum HP.
8. **Experience and the level gap** in brief (the gap factor, the 9-level ceiling, solo × 1.2), linking to the party and quest pages.
9. **Worked examples**: B a level-60 fighter on a monster two levels above (hit chance, a normal blow, a critical); C the same fighter
   against a defender (physical PvP, showing the subtraction); D a mage against a priest (magic PvP, showing the percentage cut). Use
   real monster and item rows and say which.
10. Shaiya Afterglow: computes in double precision rounded where the original stores (results agree except at exact boundaries); any
    combat deviation from `docs/server/deviations.md`.

### 3.3 `docs/guide/elements-defences.md`: Elements, resistances and defences

1. In short.
2. **Elements**: the nine-by-nine table with what it means in words (+20 % on a target without one, the four-element circle, the strong
   forms), where your attack element comes from (weapon `attrib`, then lapis, a second different lapis cancelling; a skill's own
   element; buffs 94 and 95; type 24 ignoring the target's), where your defending element comes from (the upper armour), and why
   **any element on the upper armour cuts every monster blow by 20 or 30 %**.
3. **Defence against magic defence**: a subtraction against a percentage, side by side, with a small table of what each point is worth.
4. **Resisting magic**: the resist rolls against monsters and between players, written out in full (gap G6).
5. **Resisting debuffs and states**: what monsters resist (state types, named skills), and whether and how a player resists a debuff
   from a monster or a player (gap G3).
6. **Absorption**: lapis and armour enchant, the flat cut, at least 1 left, the `0x0525` message; the kind-3 variant of the guards.
7. **Blocks, reflection, giving up SP/MP**: the buff kinds 85, 86, 92, 83, 87, the order they apply in, and what a shield's block value
   does (gap G1).
8. **Lapis option skills**: the debuffs on hit, the two maximum-HP lapis, what resists them.
9. **Worked example E**: one monster blow on a level-60 character with and without an elemental upper armour and with absorption.
10. Shaiya Afterglow: pointer as above.

### 3.4 The tab

- `site/mkdocs.yml`: a **Player guide** tab right after Home: "About the guide" (`guide.md`, a short landing page written for the site in
  `site/content/`, saying what the guide is, that the reference tabs hold the addresses, and listing the three pages), then the three
  pages.
- `docs/guide/README.md`: the folder's index for the repository (held back from the site like the other READMEs; add it to
  `site/README.md`'s held-back list). `docs/README.md`'s folder table gains `guide/`.
- `site/content/index.md`: the "A player guide is planned" note becomes a "Where to start" line pointing to the guide.

## 4. Gaps to settle (reverse the original if no note has it)

| | question | where to look first |
|---|---|---|
| G1 | **What a shield's block value does** (`docs/original/stats.md` section 2: "the shield also its block value"): a block chance, a count, and where it is rolled | `CUser::ItemEquipmentAdd` 0x454c70, the damage paths (`CUser::Damage` 0x44c1e0), `docs/original/skills-buffs.md` (blocks 85, 86, 92) |
| G2 | **Enchant attack**: the server adds `7 × step` (`g_LapisianEnchantAddValue` 0x542100, `docs/original/lapis.md` section 2) while the client's window adds `Σ (3 × max(1, ⌈k/3⌉) + 4)` (`docs/client/research-balance.md`, `0x4c4920`), and the plain summary of `lapis.md` says "up to +20" while the odds stop weapons at +12 and armour at +14 (`1-formulas.md` 4.9). Confirm the server's table from the binary, state both on the page, and say what the window shows | `ps_game.exe` 0x542100; `game.exe` 0x4c4920 |
| G3 | **Players resisting debuffs**: does a debuff from a monster or a player go through a hit roll, a resist state, an item or a buff? | `docs/original/skills-use.md` sections 8 to 12, `mob-fight.md`, `combat::pvp_skill_hits`, then `ps_game.exe` |
| G4 | **LUC and drops**: does LUC (or anything on the character) change drop chances? (expected: no; say so with the source) | `docs/original/combat.md` section 7, `docs/original/quest-drops.md` |
| G5 | **Ranged defence** for players: ability 27 and the faction grade's type 19 exist; confirm which blows read it | `CUser::SetAttack` 0x454280, `combat::player` |
| G6 | **The magic resist formulas in full** (`combat::pvp_resist_rate`; `1-formulas.md` gives the PvE one and summarises the PvP one) | the code, then the original's function |
| G7 | **Recovery**: the 5 s tick, standing 4, sitting 5 % + 28, the grade's shares, with the original's address | `docs/original/mob-fight.md`, `zone::Zones::recover` |
| G8 | **Hit and evasion of the monster's kind 1 and 2**, the monster critical on players, and the monster skill's critical without the extra roll: confirm in `ps_game.exe` | `docs/original/combat.md` section 4, `docs/original/mob-fight.md` |

What a gap turns up about `ps_game.exe` also goes into the matching `docs/original/` note (with its address), so the reference stays
complete; what it turns up about `game.exe` goes into the guide only, and is offered to the client side as a line in
`docs/testing/client-research-queue.md` (the client notes in `docs/client/` are the client side's to change).

## Build

One session. Steps:

1. Read the sources of section 2 and settle G1 to G8 (REA on `ps_game.exe`, `game.exe` only where a gap is about the client); write
   what was found into the `docs/original/` notes first, one commit.
2. Get the data for the tables and the examples from the dev database (`tools/dbconn.sh`, or the queries `tools/balance/data.py` makes),
   and work each example by hand, then check it with `tools/balance/formulas.py` (or a unit test's numbers in `combat.rs`). An example
   whose two results disagree is a finding: say so, do not publish it until settled.
3. Write the three pages, the landing page, `docs/guide/README.md`, the nav, the home page line and the index lines of section 3.4.
4. Check: `python3 tools/check_links.py` exits 0; `site/` builds with INFO lines only; read each page in the built `site/dist/` once.
5. Commit by named paths (one commit for the notes, one for the guide), push `main`, deploy the site
   (`SHAIYA_KEY=$HOME/.ssh/tylerd_deploy_key ./deploy.sh`, then `--go`) and check the live pages load.
6. Mark this plan built (a **Built** paragraph at its end: what was settled for each gap) and the roadmap line.

**Done when**: the three pages are live under the Player guide tab, every formula on them has its source, G1 to G8 are settled or
marked open with what is missing, and the worked examples agree with the model.

## Built (2026-11-05)

The three pages, the landing page (`site/content/guide.md`), `docs/guide/README.md`, the nav tab, the home page line and the index rows are
built and live at `tylerd.me/shaiya/guide/` (commits a1b5f23 for the notes, 3b8893b for the guide). What each gap settled:

| | settled | where |
|---|---|---|
| G1 | **A shield has no block value.** Slot 6 adds the shield-mastery value `m_byShieldValue` (`+0x1334`, `CUser::AddSkillPassive` 0x489a80, `wType` 10) to defence, for any item in the slot (`CUser::ItemEquipmentAdd` 0x454c70); nothing reads a block value; blocks are buffs only (85, 86, 92) | `docs/original/stats.md` section 2, `combat.md` section 4 corrected |
| G2 | **The original's enchant tables come from `Data/ItemEnchant.ini`**: `CGameData::InitItemEnchant` (0x4c7ab0, from `CGameData::LoadData` 0x4bf320) overwrites both `g_LapisianEnchantPercentRate` and `g_LapisianEnchantAddValue` at start (missing keys 0; no file: the built-in tables and a log line). The patch file's weapon attack is the client window's formula (7, 14, 21, 31 ... 311), armour 5 a step, rates 90 % for +1 down to 0.02 %, so +20 can be reached; the built-in tables (flat 7 / 5, weapons stop at +12, armour at +14) apply only without the file. A second copy in `PSM_Client/Bin/Data/` (99 % everywhere) is a private-server edit. The window shows the file's values. **Finding: Shaiya Afterglow uses the built-in tables** (`lapis::ENCHANT_RATES`, `ENCHANT_ADD`), so it differs from the original with its shipped file; the pages say so. **Decided 2026-11-05: follow the original**: the spec is `docs/plans/enchant-table.md`; until it is built, `docs/server/deviations.md` has it as lapis row 6. `1-formulas.md` sections 1 (point 7) and 4.9 describe the built-in tables | `docs/original/lapis.md` sections 2, 5, 11 |
| G3 | **No resistance stat for debuffs.** A debuff on a player is stopped by a block, the one-off protections of states 7 and 8, a lapis state block (the Anti- lapis), or its own roll (flat, the hit chance, the magic resist roll); monsters by their 15 state columns and six skill columns. From the notes, no new reverse needed | `mob-skills.md` section 7, `pvp-2.md` section 5, `item-use.md` section 5 |
| G4 | **Nothing on the character changes drop chances**: `CMob::ItemDrop` (0x4a5ea0, 0x4a65a0) reads no stat, level, buff, grade, premium or event rate (no rate global; `CMob::Death` calls it once). Rechecked 2026-11-05: the killer's side is read for the owner, a party's item share, the quest item's quest (killer or party), the guild's ranking points and the party's gold shares with each member's gold bonus; so gold charms raise the gold, never a chance | `docs/original/combat.md` section 7 |
| G5 | Ranged defence is add + REC + item defence (`CUser::SetAttack`); it is read by players' ranged blows and ranged skills on players and by monsters' **ranged skills** (`CMob::AttackDamage` 0x4a3070 takes the player's defence of the skill's kind, confirmed); monsters' normal blows are melee and never read it. So ability 27 and the grade's type 19 count only against those | the notes as they were |
| G6 | The three resist rolls were already in full in the notes (`skills-use.md` 11, `mob-skills.md` 7, `pvp-2.md` 5); written out on the third page, section 4 | |
| G7 | Recovery is `CUser::UpdateRecover` 0x48cfb0 with the amounts set in `CUser::InitEquipment` 0x454560 (`mob-fight.md` section 9); the grade's shares in `pvp-2.md` sections 8 and 10 | |
| G8 | The monster's kinds 1 and 2 are in `CMob::SetAttack` 0x4902f0 (`combat.md` section 4); the monster critical on players (`mob-fight.md` section 6) and the skill critical without the `+ R2` term confirmed again in 0x4a3070 | |

Other results:

- **`game.exe`**: the window facts first came from `docs/client/research-balance.md`; the enchant lines and the item blocks were then
  read in `game.exe` (495ea21) and are client research queue entry 24.
- **The worked examples** (A a level-60 ultimate fighter with Hallowed Spear +3 and the Daynorth set; B against monster 1811, level 62;
  C against a defender; D Flame Choir Lv3 from a mage on a priest; E monster 1811's blow with and without an elemental upper armour and
  absorption) were worked by hand and checked with `tools/balance/` (`formulas.py` with fixed rolls for the ranges, `expect.py` for the
  averages): every range agrees exactly. `expect.py`'s averages of physical PvP blows run about 0.5 low (its 17-point grid truncates
  fractional spread values in `_pvp_physical`), so the pages give "about" figures there; no formula disagrees.
- **The full status table** (every level) is not published: the home page says no data tables are distributed, so the page gives
  levels 1, 15, 30, 45 and 60 and names the source. To publish it, the maintainer decides.
- `1-formulas.md` section 3.5 gives weapon speeds 2 to 7 as 3000 to 1500 ms; speed 2 is 2,750 ms (the guide has the right table).
