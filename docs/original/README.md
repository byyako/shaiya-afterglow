# original: what the original EP4.5 server does

One note per system of the original server (`ps_login.exe`, `ps_game.exe` and their neighbours), written from static analysis of the
executables and their debug symbols: the packets in, the checks the original makes, the packets out, and the numbers. Nothing was
executed. Every page starts with **In short**, then the detail; its status words say how sure the note is (see `docs/README.md`).
These notes are the reference the new server is built from, and the code's comments cite them by name. The site's pages for these notes
are picked by `site/mkdocs.yml`; the groups below are the site's groups.

## Server programs and protocol

- [`protocol-notes.md`](protocol-notes.md): Protocol notes
- [`ps_login.md`](ps_login.md): Login server (ps_login)
- [`ps_game.md`](ps_game.md): Game server (ps_game)
- [`gamewar.md`](gamewar.md): The united war server (ps_gamewar)
- [`enter-world.md`](enter-world.md): Entering the world
- [`world.md`](world.md): Movement, sitting, leaving
- [`client-extras.md`](client-extras.md): Client-ready extras
- [`move-check.md`](move-check.md): Move speed check
- [`operator-console.md`](operator-console.md): Operator console
- [`gm-commands.md`](gm-commands.md): GM commands 1: staff and the world
- [`gm-commands-2.md`](gm-commands-2.md): GM commands 2: questions, summons, notices
- [`game-log.md`](game-log.md): The game log
- [`community-fixes.md`](community-fixes.md): Known bugs and fixes
- [`server-patch-notes.md`](server-patch-notes.md): Patch kit notes: how each fix works

## Characters and items

- [`stats.md`](stats.md): Character numbers
- [`items.md`](items.md): Moving and equipping
- [`item-drop.md`](item-drop.md): Dropping items
- [`item-use.md`](item-use.md): Using items
- [`item-flags.md`](item-flags.md): Item buffs and resurrection runes
- [`item-windows.md`](item-windows.md): Items that open a window
- [`item-origins.md`](item-origins.md): New items and crafting
- [`lapis.md`](lapis.md): Lapis and enchanting
- [`item-options.md`](item-options.md): Item option skills
- [`repair.md`](repair.md): Wear and repair
- [`shop.md`](shop.md): NPC shops
- [`item-mall.md`](item-mall.md): Item mall and premium
- [`personal-shop.md`](personal-shop.md): Personal shops
- [`auction.md`](auction.md): Auction house
- [`warehouse.md`](warehouse.md): Warehouse
- [`trade.md`](trade.md): Trade
- [`vehicles.md`](vehicles.md): Mounts
- [`small-items.md`](small-items.md): Small features

## Combat and skills

- [`combat.md`](combat.md): Fighting monsters
- [`skills.md`](skills.md): Skills 1: definitions and learning
- [`skills-use.md`](skills-use.md): Skills 2: using skills
- [`skills-buffs.md`](skills-buffs.md): Skills 3: buffs and debuffs
- [`skills-special.md`](skills-special.md): Skills 4: special skills
- [`thorns-fields-pvp.md`](thorns-fields-pvp.md): Fire Thorns and fields on players
- [`pvp.md`](pvp.md): Player kills 1
- [`pvp-2.md`](pvp-2.md): Player kills 2
- [`duel.md`](duel.md): Duels
- [`guild-war.md`](guild-war.md): Guild wars
- [`obelisks.md`](obelisks.md): Obelisks

## Social systems and quests

- [`party.md`](party.md): Parties
- [`union.md`](union.md): Union parties
- [`guild.md`](guild.md): Guilds
- [`guild-house.md`](guild-house.md): Guild houses
- [`guild-ranking.md`](guild-ranking.md): Guild ranking battle
- [`guild-bank.md`](guild-bank.md): Guild bank
- [`friends.md`](friends.md): Friends and block list
- [`quest.md`](quest.md): Quests
- [`mini-game.md`](mini-game.md): Gambling house

## The world and its monsters

- [`maps-npcs.md`](maps-npcs.md): Maps and NPCs
- [`portals.md`](portals.md): Portals
- [`gatekeeper.md`](gatekeeper.md): Gatekeepers
- [`mobs.md`](mobs.md): Monsters
- [`mob-fight.md`](mob-fight.md): Monsters fighting back
- [`quest-drops.md`](quest-drops.md): Monster drops: quest and sequence items
- [`mob-skills.md`](mob-skills.md): Monster skills
- [`pathing.md`](pathing.md): Monster paths (A*)
- [`chase-smoothing.md`](chase-smoothing.md): Monsters chasing
- [`scripted-mobs.md`](scripted-mobs.md): Scripted monsters
- [`guards.md`](guards.md): Walking NPCs and guards
- [`instances.md`](instances.md): Party instance zones

## Reference notes (not on the site)

- [`client-login.md`](client-login.md): how the US `game.exe` reaches the login server, and how to point it elsewhere.
- [`community-guide-notes.md`](community-guide-notes.md): the community configuration guide (game data, logs, GM tools), unverified findings marked as such.
- [`database-notes.md`](database-notes.md): the ten original databases, restored from the backups, and how the definitions reach PostgreSQL.
- [`ep6-core-packet-catalogue.md`](ep6-core-packet-catalogue.md): the packet structs of a later episode's open headers, made by `tools/extract_core_packets.py`.
- [`server-inventory.md`](server-inventory.md): the original server files, listed and described.

## Data folders

- [`symbols/`](symbols/README.md): the symbol tables and packet structs read from the PDBs.
- [`db/`](db/README.md): the structure of the ten original databases.
- [`inventory/`](inventory/README.md): the lists of the two packs of original files.
