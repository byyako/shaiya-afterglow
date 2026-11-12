# Stat Recreation Runes

The reference for a rune's `recreate` effect (`docs/plans/ext-3-effects.md` section 10.3, `docs/server/effects.md`). Six copies of the
Recreation Rune (100/171, special 62), one for each of STR, DEX, REC, INT, WIS and LUC (items 100/221 to 226):

    effect = { type = "recreate", stats = ["str"], value = "random" }

Used in the recreation window on a craftable item, a rune changes only the numbers it names that the item already has (not 0): each
becomes the smaller of two draws from 0 to the item's top, as the core's rune draws every number. The other numbers and the enchant
step stay. An item without the named number is refused as the core's rune refuses one (the window's failure), and the rune stays.

    target/release/data-editor ext install extensions/stat-runes      # then restart the game server

The runes are grade 999, so no monster drops them: sell them in the item mall, make them with `/make 100 221` (to 226), or give them from a
script. Their client text and look come with the next client data patch; until a client has it they show as `???`.

Disabled (`data-editor ext disable stat-runes`, then restart), each is an ordinary Recreation Rune.

## Variations

`value = "max"` sets each named number to the item's top (the auto-max rune of the catalogue); `stats` takes up to nine names
(`"str"`, `"dex"`, `"rec"`, `"int"`, `"wis"`, `"luc"`, `"hp"`, `"mp"`, `"sp"`), so one rune can redraw several:

    effect = { type = "recreate", stats = ["str", "dex"], value = "max" }
