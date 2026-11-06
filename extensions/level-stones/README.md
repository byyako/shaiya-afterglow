# Level Stones

The reference for an item effect (`docs/plans/ext-3-effects.md` section 10.1, `docs/server/effects.md`). Three stones:

| item | name | effect |
|---|---|---|
| 100/215 | Level Stone | `level_up`, `levels = 1`: one level more |
| 100/216 | Level Stone (15) | `to = 15`: up to level 15 |
| 100/217 | Level Stone (30) | `to = 30`: up to level 30 |

A stone gives the stat and skill points of every level gained, as levelling does, and the experience becomes the start of the level it
reaches. It is refused (the stone stays) when the user is at or above its level, at the mode's cap (30 in easy mode, 60 otherwise), or at
the stone's `max`. Level Stone (30) from level 20 shows level 30 at once.

    target/release/data-editor ext install extensions/level-stones      # then restart the game server

The stones are grade 999, so no monster drops them. Sell them in the item mall, make them with the GM's `/make 100 215` (or 216, 217),
or give them from a script. Their client text and look come with the next client data patch (`tools/sdata/client_patch.py`); until a
client has it they show as `???`.

Disabled (`data-editor ext disable level-stones`, then restart), each stone is an item the core refuses, and it stays in the bag.

## Change a stone

A stone is a row and an effect: edit `rows.toml` and `data-editor ext upgrade` with a higher version. `max` keeps a stone from working
above a level: `effect = { type = "level_up", levels = 5, max = 40 }`.

## Keep the stones off some maps

The rules' `item_use` point refuses an item by its id. In `config/game.toml`:

    [rules.no-level-stones-in-pvp]
    type = "deny"
    point = "item_use"
    items = ["100/215-217"]
    maps = [2, 3, 4]
    message = "Level stones do not work here."

then `console <server id> reload`.
