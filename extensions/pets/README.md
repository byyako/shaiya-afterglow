# Pets

Catch a weakened monster, carry it as a Pet Egg, call it out as a small follower that fetches your drops
(`docs/plans/ext-4-pets.md`, the operator's page `docs/server/pets.md`). Two items, two effects and a set of client monster records:

| item | name | effect |
|---|---|---|
| 100/227 | Capture Net | `capture`: used on the weakest monster that is fighting you, at 30% HP or less, 15 units away at most. The chance is 10% at 30% HP and rises to 60% near death; a failed catch uses the net up ("It broke free!") |
| 100/228 | Pet Egg | `pet_egg`: holds one monster. Used, it calls the pet out beside you; used again, it sends it back. The egg stays. One pet out at a time |

The pet is a small, unclickable copy of the monster (the client's records 3001 to 5473 are copies of the monsters 1 to 2473, about 40%
of their size, Size 0 so nobody can target them). It follows its owner, never fights, and **fetches the owner's drops**: it runs to a
monster drop of the owner's within 15 units, and the owner picks it up from there with the usual "Attained" line; gold too. A full bag
leaves the drop on the ground. Pets give no buffs.

    target/release/data-editor ext install extensions/pets      # then restart the game server

Not installed by `tools/setup.sh`. Both items are grade 999, so no monster drops them: put them in the item mall, make them with the
GM's `/make 100 227` (and `/make 100 228` for an empty egg, which the game answers "This egg is empty."), or give them from a script
(`docs/server/pets.md` says how to add a mall product or a drop entry).

## The client needs a data patch

The players' clients show the two items as `???` and cannot draw the pets until they have a data patch that carries the items and the 3,000
new records of `Monster.SData` (the 2,473 pets and 527 fillers before them): `tools/sdata/client_patch.py check`, then `generate --patch N`. Until then, `base = 0` in the egg's effect
(the pet is the wild monster's own record: full size, clickable, its own speed; the server ignores an attack on it) lets a server try
pets with a client that has no patch.

## What it does not do

A boss, a quest monster (one that drops a quest item), a monster of an obelisk, a boss chain or a script is never caught. The egg is an
ordinary item: it can be traded, sold, stored and dropped, and the pet goes the moment its egg leaves the bags.

## Change it

Everything is in `rows.toml`: the net's `below`, `chance`, `range`, `levels_above` and `except`, the egg's `loot`, `loot_range` and
`gold`. Edit it and `data-editor ext upgrade extensions/pets` with a higher version, then restart. Two rule points keep pets and nets off
some maps or levels; the examples are in `extension.toml`, commented out, and in `docs/server/rules.md`:

    [rules.no-pets-in-town]
    type = "deny"
    point = "pet"
    maps = [35, 36, 42]

    [rules.capture-from-10]
    type = "deny"
    point = "capture"
    level = "1-9"
    message = "You can catch monsters from level 10."

then `console <server id> reload`.

Disabled (`data-editor ext disable pets`, then restart), both items are refused and stay in the bag, no pet comes out, and the stored
pets are kept for when it is enabled again.
