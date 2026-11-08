# Pets

A player weakens a monster that is fighting them and uses a **Capture Net** on it. Caught, the monster becomes a **Pet Egg** in the bag;
used, the egg calls a **small copy of that monster** that follows its owner, is never fought, and **fetches the owner's drops** into the
bag. The pets are an extension (`extensions/pets`) made of two item effects, two rule points and a set of client-only monster records.
With the extension not installed or disabled the server behaves exactly as before. `docs/plans/ext-4-pets.md` is the design; this page is
how to run it.

**New in 0.4.0.** Migration 0036 (`content.client_monsters`, the `monster` kind of the extensions' ids, `character_pets`), the effects
`capture` and `pet_egg`, the rule points `capture` and `pet`, the `[[monster_look]]` entry of an extension's `rows.toml`, and
`Monster.SData` in the client data patches.

## 1. Install

    target/release/data-editor ext check extensions/pets
    target/release/data-editor ext install extensions/pets      # then: tools/game-server.sh restart

Effects are read when the game server starts, so an install, upgrade, enable or disable needs a restart. The extension is not installed
by `tools/setup.sh`. `ext show pets` prints what it made, the pets' records as one line (`monster_look 1-2473 → 3001-5473, day mapped, 7
fields set`).

## 2. The items

| item | name | effect | what it does |
|---|---|---|---|
| 100/227 | Capture Net | `capture` | used on the **weakest monster that is fighting the user** (lowest HP in percent; the nearest on a tie), within `range`, at `below` percent HP or less. A hit makes the egg and uses the net up; a miss uses the net up too ("It broke free!") and the monster goes on fighting |
| 100/228 | Pet Egg | `pet_egg` | holds one monster. Used, it calls the pet out beside the owner; used again, it sends the pet back. The egg stays. One pet out per player; another egg's pet replaces it |

The keys (all in `extensions/pets/rows.toml`; `docs/server/effects.md` has the tables):

- `capture`: `egg` (the item the catch becomes), `below` (default 30), `chance` (`[10, 60]`: the chance in percent at `below` and near death),
  `range` (15), `levels_above` (the monster's level at most the user's plus this), `except` (monster ids never caught).
- `pet_egg`: `base` (the pet of monster *m* is the client's record `base + m`; 0 is the wild record), `monsters` (the ids that have a pet
  record), `loot`, `loot_range` (5 to 30, default 15) and `gold`.

**The chance** is `c0 + (c1 − c0) × (below − p) / below` percent, rounded down, at the monster's HP percent `p`: with the defaults 10% at
30% HP, 26% at 20%, 43% at 10% and 58% at 1%. The roll is a random number modulo 100 below the chance.

**What can be caught.** A monster that came from a spawn area and is fighting the user. Never: a boss, a boss chain's follower or chest,
an obelisk's monster or guard, a monster of a Lua or state-machine script, an unbeatable one, a monster a script spawned (monster
madness), one that drops a quest item, one the egg has no record for, one in `except`, or (with `levels_above`) one too far above the user.
The refusals whisper (once in ten seconds for each kind) and keep the net: `No monster is fighting you.`, `That monster cannot be caught.`,
`It is too strong to catch. Bring it below 30% HP.`, `Your bags are full.`

**The egg.** One item row for every monster: the monster is in the egg's craft name, `P00123.6731A2F0.00` (the monster, the time of the
catch and a counter), which travels with the item through trades, the warehouse, the guild bank, the market and the ground. The client
cannot show anything per egg, so every egg reads "Pet Egg" until a client feature adds a line (client research entry 28); the player sees
which monster it is when the pet comes out. An egg made by `/make` or a script has no monster in it: using it says `This egg is empty.`
An egg of a monster the effect no longer lists says `This pet cannot come out.` The egg is in reuse group 11 (2 seconds).

**How players get the items.** Both are grade 999, so no monster drops them. Put them in the item mall (a product of `defs.product_list`
that names the item), make them with the GM's `/make 100 227 10`, give them from a script (`game.give_item`), or add a drop entry to a
monster (a `[[drop]]` entry of an extension's `rows.toml`, `docs/server/effects.md` section 4 and `docs/plans/ext-1-foundation.md`).

## 3. What the player sees

- The pet is a **monster** to the client: the English name, model and animations of the monster, about 40% of its wild size (a goblin at
  1.0 becomes 0.4, an ogre at 2.0 becomes 1.0, a giant at 5.0 becomes 2.0), and **impossible to click or target** (Size 0). Everyone in
  view sees it; walking and running are the client's own animations.
- It follows its owner: it stays within 4, walks to a point 2.5 from the owner when farther (it runs beyond 8) and **snaps** beside the
  owner beyond 20 (a teleport, a portal). It keeps up with a player on foot and on a mount.
- It is **hidden** while the owner is dead or an invisible staff member, and while a `deny pet` rule matches; it comes back by itself.
- A map change, a portal, a recall or an instance takes it along: it comes out again beside the owner on the new map. A logout and a login
  keep it (the egg is stored in `character_pets`); the pet goes the moment its egg leaves the bags (a trade, a sale, the warehouse, the
  guild bank, the market, a drop, a loss at death).
- It **fetches the owner's drops**: a monster drop whose owner (the killer of record) is the pet's owner, within `loot_range` of the owner,
  nearest first, gold if the egg's `gold` is on, a quest item only if the owner's quests take it. The pet runs to it and the owner picks it
  up from there: the usual "Attained" line or gold line, the sound, the game log's records. A drop that does not fit stays on the ground,
  untouched, with one "no room" answer, and the pet leaves it. It fetches nothing while its owner is dead or in a trade or personal shop.
  What another player or the owner dropped, and what a party shares, is not fetched. **Pets give no buffs.**

## 4. Rules

Two points of the rules (`docs/server/rules.md`, section 3), both for one player and a map:

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

`pet` is asked at the summon, at the arrival on a map and every 5 seconds for a pet that is out (a match hides it and whispers the message
once); `capture` is asked when a net is used. The extension itself ships no rule in force. A `deny item_use` rule with the egg's or the
net's id also refuses the use, but it does not hide a pet already out: use the two points.

## 5. The client data patch

The players' clients need a data patch for the two items' names and the pets' monster records: `tools/sdata/client_patch.py check`, then
`generate --patch N` on a developer's box. The patch carries `Item.SData` and a new `Monster.SData` (the original's 2,474 records, 527
empty fillers, then the pets 3001 to 5473; about 285 KB). The pets' records are copies of 1 to 2473 with a smaller size class, Size 0, HP 1
and the pets' own walking and running speeds. A server without patches 109 and 110 rebuilt from the database still matches them byte for
byte (`client_patch.py rebuild --patch N`): with no monster rows `Monster.SData` is not generated at all.

**`base = 0`.** The egg's `base` set to 0 makes the pet the wild monster's own record: full size, clickable, its own speed. It is the way
to try pets with a client that has no patch (an unpatched client sent a record it does not have shows nothing at all, harmlessly); the
server ignores an attack on a pet.

## 6. Disabling and removing

`data-editor ext disable pets` and a restart: both items do what their own columns say (the core refuses them and they stay in the bag),
no pet comes out, and the stored pets are kept for when it is enabled again. `ext remove pets` puts the two item rows back and deletes
the pets' monster records (the next whole-file patch drops them); it is refused while players hold the items. With `--force` the held
items are deleted after a confirmation, logged, and the game servers must be stopped.

## 7. Not in this release

A pet window with buttons (the client has no server-opened window), levels, evolution and abilities of pets, more than one pet, a name or
a size per pet (the client's scale classes are fixed), personal loot, and the egg naming its monster in the tooltip (client research
entry 28).
