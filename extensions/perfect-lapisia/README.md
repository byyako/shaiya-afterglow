# Perfect Lapisia

The reference for a Lapisia's `enchant` effect (`docs/plans/ext-3-effects.md` section 10.2, `docs/server/effects.md`).

| item | name | copy of | effect |
|---|---|---|---|
| 95/3 | Perfect Weapon Lapisia | 95/1 Weapon Lapisia | `enchant`, `rate = 1000000`, `jumps = [[1, 60], [2, 30], [3, 10]]`, `safe = true` |
| 95/8 | Perfect Armor Lapisia | 95/6 Armor Lapisia | the same |

An enchant with one always succeeds (a rate of 1,000,000 needs no draw), rises the step by 1, 2 or 3 in the proportions 60, 30 and 10
(`jumps`; the rules' `enchant` rule is not asked, the scripts' `enchant` point still is). A failure cannot happen at this rate, and `safe`
says that a failure of a Lapisia of this row never destroys an item either, whatever its VG column says (it matters when `rate` is
lowered). The price, the material and the checks are the core's. The step stops at 20.

    target/release/data-editor ext install extensions/perfect-lapisia      # then restart the game server

The Lapisia are grade 999, so no monster drops them: sell them in the item mall, make them with `/make 95 3` or `/make 95 8`, or give them
from a script. Their client text and look come with the next client data patch; until a client has it they show as `???`.

Disabled (`data-editor ext disable perfect-lapisia`, then restart), each is an ordinary Weapon or Armor Lapisia: the core's rate by
step, and the VG that may destroy a failed item.

## Variations

An effect needs at least one of `rate`, `jumps` and `safe`. `rate` defaults to 1,000,000, so a Lapisia with only `jumps` or only `safe`
always succeeds too. A Lapisia that gambles but never destroys: `effect = { type = "enchant", rate = 500000, safe = true }`. To keep the
core's rate by step and only change the jump for every Lapisia, use an `enchant` rule (`docs/server/rules.md`) instead of an effect.
