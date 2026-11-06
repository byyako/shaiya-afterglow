# The enchant tables from `ItemEnchant.ini`: spec and build plan

**Decided by the maintainer, 2026-11-05: the server follows the original.** The enchant success rates and the attack and absorption each
step adds come from the operator's `Data/ItemEnchant.ini`, as the original loads them, not from the tables built into `ps_game.exe`.
Found while writing the player guide (`docs/plans/player-guide.md`, gap G2).

## 1. What the original does

Confirmed (`docs/original/lapis.md` sections 2 and 5):

- At start, `CGameData::LoadData` (0x4bf320) calls `CGameData::InitItemEnchant` (0x4c7ab0). It opens `data/ItemEnchant.ini`; when that
  fails it logs "CGameData::InitItemEnchant fopen error" and **keeps the built-in tables**. Otherwise, for steps `00` to `20`, it sets
  four values with `GetPrivateProfileIntA(section, key, 0, ".\\Data\\ItemEnchant.ini")`, **a missing key giving 0**:

  | section | key | into |
  |---|---|---|
  | `LapisianEnchantPercentRate` | `WeaponStepNN` | `g_LapisianEnchantPercentRate[NN].weapon` (0x542058), the success rate in millionths of leaving step NN |
  | `LapisianEnchantPercentRate` | `DefenseStepNN` | `[NN].armour` (0x54205c) |
  | `LapisianEnchantAddValue` | `WeaponStepNN` | `g_LapisianEnchantAddValue[NN].weapon` (0x542100, `u16`), the **total** attack at step NN |
  | `LapisianEnchantAddValue` | `DefenseStepNN` | `[NN].armour` (0x542102, `u16`), the total absorption at step NN |

- The add values are **totals per step, not per-step amounts**: step `s` adds `table[s]` (the built-in table happens to be `7s` / `5s`).
- The roll is unchanged: success when material count + a draw below 1,000,000 < `rate[step]`; step 20 is refused before the roll.
- The file of the original server patch (`PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/Data/ItemEnchant.ini`, the folder
  `tools/setup.sh` asks for) gives the table of `lapis.md` sections 2 and 5: weapon attack 7, 14, 21, 31 ... 311 (the client window's
  formula), armour 5 a step, rates from 90 % (weapon +1) and 99 % (armour +1) down to 0.02 %, step 20 at 0. So +20 can be reached.
- `/reitemenh` (`CGameData::ReItemEnchant` 0x4c7bc0) reloads the file; *inferred* original bug: when a step's defence value differs from
  the weapon value it writes the weapon value into the defence slot (`docs/client/research-balance.md`, "Findings outside the questions").
  Not to be copied: the reloads are not built (`docs/original/operator-console.md` section 9, point 4).

## 2. What the server does today

`crates/game-server/src/lapis.rs`: `ENCHANT_RATES` (the built-in rates) and `ENCHANT_ADD` (`[7, 5]`, multiplied by the step in
`enchant_bonus`). Nothing reads `ItemEnchant.ini`. So from +4 a weapon has less attack than the original and the client's window show,
the rates are far lower, and weapons stop at +12, armour at +14. The guide says this on its first page ("Shaiya Afterglow").

## 3. The build

1. **Import** (`tools/world/import.py`, which already reads the PS_Game `Data` folder's `.ini` files with `read_profile`): read
   `ItemEnchant.ini` as `GetPrivateProfileIntA` does (the section and key names case-insensitive, `;` comments, the leading integer of the
   value, trailing tabs and spaces ignored, a missing key 0) into a new table **`world.item_enchant (step smallint primary key 0..20,
   weapon_rate integer, armour_rate integer, weapon_add smallint, armour_add smallint)`** in `db/world/schema.sql`, 21 rows. **No file:
   no rows**, and a line saying the built-in tables will be used (the original's behaviour). Re-running the import replaces the rows.
2. **Load** (`pg.rs`, with the other world data at start): 21 rows → an `EnchantTable`; no rows → the built-in tables (today's
   constants, kept as the fallback). Anything but 0 or 21 rows, or steps outside 0 to 20: a start error naming the table.
3. **Use** (`lapis.rs`): `Enchant::rate` from the table; `enchant_bonus(craft_name)` returns `(table[s].weapon_add, 0)` for a weapon
   step `s` and `(0, table[s].armour_add)` for an armour step (a lookup, no multiplication). `stats::compute`, `combat::gear` and
   `server::gear` take the table where they call `enchant_bonus`. The `enchant` rule's jumps (`docs/server/rules.md`) keep working: they
   only choose the new step.
4. **The balance model** (`tools/balance/`): `formulas.enchant_bonus` and the rates read the same table (`data.py` from
   `world.item_enchant`, the built-in tables when it is empty); `test_formulas.py` gets a case for each; `content.py`'s enchant-cost table
   and `docs/plans/balance/1-formulas.md` section 4.9 (and section 1, point 7) are redone or marked as describing the built-in tables.
5. **Tests**: `lapis.rs` units with a table (a step's add value is the table's, not 7 × step; a rate of 0 never succeeds; the fallback);
   the import's parser on the patch file's layout (tabs after values, Korean comments in CP949, a missing key); `pg` loading 0, 21 and a
   wrong count of rows.
6. **Docs**: `docs/original/lapis.md` section 11 (built, and how), `docs/server/deviations.md` (no row needed once built; until then a
   lapis row says the server uses the built-in tables), the guide's Shaiya Afterglow note on `docs/guide/character-numbers.md`, the
   CHANGELOG, `docs/server/run.md` or the README if the import's output mentions the file, and a live-test item in
   `docs/testing/live-test-checklist.md`: a weapon taken from +3 to +4 shows the same attack in the character window as the server uses
   (the window's formula is the file's, `docs/client/research-balance.md`), and an armour step shows "<n> absorption damage" with the
   file's value.

Not in scope: an operator setting to choose the built-in tables (the operator edits the file instead), the reload command.

**Done when**: a fresh setup imports the patch file's 21 rows, the server enchants with its rates and add values, a server without the
file behaves as today, the tests and the model agree, and the live test is listed.
