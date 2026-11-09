# The EP5.4 server pack, compared with EP4.5

This page compares the EP5.4 server pack's programs, databases and data with the EP4.5 original that the rest of these notes describe.
It gives the binaries, what changed overall, and links to the "Other episodes" section of each system note, where the detail is. It
was static analysis only: nothing from the pack was executed. Function names in EP5.4 have no PDB behind them; they were carried over
from EP4.5 by unique strings, call-graph shape, constants and switch tables, and every address below is in the named EP5.4 binary.

Status words as in `docs/README.md`: **confirmed** (read in the binary or file, with the address or the file), **inferred**.

## In short

- **A mixed set of builds.** `ps_game`, `ps_gamewar`, `ps_session` and `ps_dbAgent` are Visual Studio 2008 builds of 2011-09-14.
  `ps_gameLog` (2010-10-18) and `ps_userLog` (2009-09-22) are VS 2003 builds. **`ps_login` is the EP3 build of 2007-12-07 with 7 bytes
  changed** (confirmed, section 1).
- **Three programs were modified after linking** (confirmed): `ps_login` (the 7 bytes), `ps_game` (three jumps that switch off the new
  race-faction checks, the resource size, and more not located) and `ps_dbAgent` (an SQL format string rewritten in place, and one more
  small change not located). The file dates of 2015, 2018 and 2023 go with exactly these three.
- **The game server is EP4.5's program grown, not a rewrite.** The connection states, opcode groups and group handlers are the same, no
  opcode was removed, and most systems are unchanged case by case (confirmed). Packet lengths are still not checked.
- **What is new** (confirmed): the level cap 70, skills of nine levels, new skill damage types, buffs and abilities; a rewritten move
  check; the item-mall product list sent from a prebuilt cache and points through an external billing service; a target-buff
  subscription; link and extraction chance previews; multi-item boxes; item-triggered monster summons; timed server events run from the
  console; 20-slot personal shops; a map-70 relic field broadcast to the fields around it.
- **What was fixed** (confirmed): self-trade and trade during a duel or with a shop open; a remake naming the same item twice; the duel
  stake count; the party leader index on a kick; NaN positions on save and in the movement packets. Most other bugs in
  [`community-fixes.md`](community-fixes.md) are still there.
- **Databases:** the EP4.5 schema, in places an older copy of it. The database files are SQL Server 2000 except `PS_GameDefs`. **The
  `PS_ChatLog` database ships a trigger on its chat table that grants elevated rights on certain chat lines**: an operator who restores
  these files should drop it (section 4).
- **Data:** EP4.5's data plus the content for levels 61 to 70, repacked by a Chinese private server around 2012, with gaps. Maps 68 to 80
  are new, the party instances are switched off, map 60 is a town. The experience table for levels 1 to 60 is EP4.5's divided by 100.

## 1. The binaries

| program | linked (UTC) | linker | size | SHA-256 | PDB path (CodeView) | modified after linking |
|---|---|---|---|---|---|---|
| `ps_game.exe` | 2011-09-14 08:10:08 | 9.0 (VS 2008) | 1581568 | `1f9d3d07a9858d6756f95a283e36402ac1c30e5f4ea680673724f686f9397531` | `c:\Shaiya\Server\Bin\ps_game.pdb` | **yes**: stored PE checksum 0x18530a, computed 0x18e319; file date 2023-04-24 |
| `ps_gamewar.exe` | 2011-09-14 08:14:40 | 9.0 | 1481216 | `204fc8217bff5f495a42a7e07d14cea197ea9332bf7b1cae3ca8b6781df9009e` | `c:\Shaiya\Server\Bin\ps_gamewar.pdb` | no: checksum matches |
| `ps_login.exe` | **2007-12-07 02:46:58** | 7.10 (VS 2003) | 319488 | `4a04836152fa597293ac17d39145f09342ef9a04ad510736c98eb5a191cd1c5e` | `d:\SonoV\Bin.net\ps_login.pdb` | **yes**: the EP3 build with 7 bytes changed; file date 2018-06-12 |
| `ps_session.exe` | 2011-09-14 07:30:56 | 9.0 | 437760 | `20ceeaf9e9c4d224b95c98e8a1fce5fdb194d5354472ec404c2421a619573dd6` | `c:\Shaiya\Server\Bin\ps_session.pdb` | no: checksum matches |
| `ps_dbAgent.exe` | 2011-09-14 07:20:57 | 9.0 | 589312 | `70430743d50479e9af0e1cf0b17abff2da9adad50b4f46ca87016a39380f1f9d` | `c:\Shaiya\Server\Bin\ps_dbAgent.pdb` | **yes**: stored checksum 0x9881b, computed 0x92aa4; file date 2015-07-20 |
| `ps_gameLog.exe` | 2010-10-18 07:33:52 | 7.10 | 307200 | `7c1aa125dcc5d9766f2e9dbbc6732ab8dd1d7f1448b9744db662c236e19663dd` | `d:\sonov\source\server\Bin\ps_gameLog.pdb` | not known (VS 2003 stores no checksum); no sign of a patch |
| `ps_userLog.exe` | 2009-09-22 03:42:43 | 7.10 | 286720 | `7e23d5dc6972405acdfc9d95aa5b07c8284aca9f8c177996e779ee15519c9e92` | `d:\SonoV\Bin.net\ps_userLog.pdb` | not known; the file date is one hour before the link time, a time zone artefact |

All are in the pack's `PSM_Client/Bin/`; no PDBs ship. Each pairs with the EP4.5 program of the same name
([`server-inventory.md`](server-inventory.md) has the EP4.5 table). The full file list is in `docs/original/inventory/server-ep5.4/`.

How the modified programs were changed (confirmed unless marked):

- **`ps_login`**: its link stamp, entry point, image size and CodeView id are the EP3 `ps_login`'s; seven bytes differ from the EP3
  file. One makes the key reply's length byte be exactly 0x80 instead of at most 0x80; one removes the accept-time connection cap;
  one changes the quote character of the login query's user name and password. [`ps_login.md`](ps_login.md), Other episodes.
- **`ps_game`**: three conditional jumps made unconditional (EP5.4 ps_game 0x47b54e, 0x47b608, 0x47c134; `ps_gamewar` has the
  original jumps). They switch off the new check that a character's race belongs to the account's faction, at the character list and
  at loading. The `.rsrc` size fields were raised from 0x1b4 to 0x200. A fourth jump (EP5.4 ps_game 0x487fc7, `jne` → `jmp`) lets an item-mall purchase through when the billing link is down (inferred), the string table name `data/de_string.DB` became `data/cn_string.DB` (0x56f414), and the 19 level bounds of 80 are 70 in `ps_gamewar` (very likely edits too, inferred). Even with all these put back the checksum does not match (0x18d10e against 0x18530a), so at least one more edit was not located. [`enter-world.md`](enter-world.md), Other episodes.
- **`ps_dbAgent`**: the character save statement's format string at EP5.4 ps_dbAgent 0x487888 was rewritten in place, same length.
  One more small change (a word-sum difference of 0x100, one byte +1 at an odd offset) was not located; it is not in a string.

## 2. The game server

The detail is in each system note. In outline (confirmed unless marked):

- **Dispatch and protocol.** The same states, opcode groups and handlers as EP4.5; no opcode removed. New client opcodes: `0x0308`
  to `0x030A` (select and clear a target; the server then sends that target's buffs as `0x030B` list, `0x030C` added, `0x030D`
  removed), `0x0809`/`0x080A` (link and extraction chance), `0x0B1B` (party type, echoed by the client), `0xB106` answered before
  entering the world, the staff opcodes `0xF603`, `0xF70F` (`/mmake` on every server) and `0xFA1B` (guild rank by character id). The GameGuard path is gone. A connection whose receive queue
  passes 100 packets is closed and logged (EP4.5: 1000, no log). [`ps_game.md`](ps_game.md), [`protocol-notes.md`](protocol-notes.md).
- **Characters and combat.** Level cap 70, but 19 level checks use 80, where `ps_gamewar` of the same day has 70, so they were very
  likely raised after linking. Read literally, a level-70 character still gains experience and then rises one level per gain up to 80,
  with thresholds read from the wrong table row (the path is confirmed, the outcome inferred). The status table has no rows past 60, so
  levels 61 to 70 come without HP, SP and MP growth (inferred). Skills of nine levels. Skill damage types 6 to 12, ability types 47
  to 49, buff kinds 96 to 98, debuff kinds 74 to 76. Damage cap 60000 (EP4.5: 30000). [`stats.md`](stats.md), [`combat.md`](combat.md),
  [`skills-use.md`](skills-use.md), [`skills-buffs.md`](skills-buffs.md).
- **Movement.** A new move check: steps of at most 30 units, windows judged after 6 steps and 3 s, a failed player moved back,
  steps ignored for 1.1 s after a server move, NaN positions refused. [`move-check.md`](move-check.md).
- **Items.** Item-use effects 0 to 74 (EP4.5: 0 to 59), multi-item boxes, grades to 3072, counted free resets, item-triggered summons
  from `MobCreate.ini`, a sure-link hammer, chance previews, timed rate events for linking, extraction and enchanting.
  [`item-use.md`](item-use.md), [`lapis.md`](lapis.md).
- **Item mall and console.** The product list is sent from up to 50 prebuilt pages of 25 rows; points go through an external billing
  service (PayLetter) via `ps_session`. The console lost its eight live reloads and gained 19 words, including timed events kept in
  `ServerCmd.ini` (not shipped) and an in-game channel for staff. [`item-mall.md`](item-mall.md),
  [`operator-console.md`](operator-console.md).
- **Social.** Unchanged in code apart from kill-rate events, the `MapCurse` map key (unused by the data), the map-70 relic broadcast
  and the `0x0B1B` echo. [`guild.md`](guild.md), [`pvp.md`](pvp.md), [`obelisks.md`](obelisks.md).
- **Personal shops** have 20 slots, and a visitor who changes map is now taken off the seller's list (the EP4.5 `myshop-visit-cleanup` bug is fixed). **The auction house** is unchanged, case by case. `0xB103` is no longer sent. [`personal-shop.md`](personal-shop.md).
- **The war server** is covered in [`gamewar.md`](gamewar.md).

## 3. The other programs

- **`ps_session`** (unmodified): the core is unchanged. New are the PayLetter billing link, a broadcast of the billing link's state
  (`0x105`), 16 random bytes sent to each game server (`0x108`: the game server's XOR seed for the run, which it builds its XOR table from and sends
to the clients in `0xA301`; EP5.4 ps_game 0x406a14, 0x464ca0, 0x413d6d; confirmed), and two relays (`0x818`, the all-server `/mmake`; `0x81B`). Without `[PAYLETTER]` it never answers a purchase (`0x1B02`) (inferred). It takes EP3's
  0x63-byte `0x701` from `ps_login`. [`ps_login.md`](ps_login.md), Other episodes.
- **`ps_dbAgent`**: the same message set, with `0x0E09` (billing refused: drop the purchase or roll the gift back) and `0x0E0A` (billing confirmed) now handled; the item-mall procedures renamed (`*2`, `*_UsersMaster`), and four of them exist in no
  database of the pack, so the shipped item mall can't have worked with its own databases (inferred). Saved positions are checked for
  NaN. Saving is still a per-user 15-minute timer. [`database-notes.md`](database-notes.md), Other episodes.
- **`ps_gameLog`**: EP4.5's code reading the 87-byte EP5.4 record header (the user name grew from 19 to 32 bytes), so EP4.5 and EP5.4
  game and log servers can't be mixed. [`game-log.md`](game-log.md), Other episodes.
- **`ps_userLog`**: EP4.5's code without the monitor listener.

## 4. The databases

The pack's `PSM_Data/` holds ten databases as data files. `PS_GameDefs` is database version 661 (SQL Server 2008 R2); the other nine are
version 539 (SQL Server 2000), which SQL Server 2022 refuses. All ten are described in `docs/original/db/ep5.4/`, and
`docs/original/db/README.md` says how.

- **Schema** (confirmed): the same as EP4.5 in eight databases. `PS_ChatLog.ChatData` is `char(100)` (EP4.5: `char(10)`).
  `ProductList.RowID` became an identity column. `PS_UserData` lacks four tables and two procedures that EP4.5's backups have
  (`CreatedGuilds`, `UserLoginStatus`, `UserOnlineTimeLog`, `WebPointLog`). Their dates show that EP5.4 shipped an older copy, not
  that they were removed. Seven procedure texts differ from EP4.5's.
- **`PS_GameDefs` rows**: Items 16047 → 19572, Skills 1333 → 3738 (1,955 of them junk rows with id 0), Mobs 2747 → 3088, MobItems
  24723 → 27792, ExpDefs 240 → 280, ProductList 84 → 249.
- **A backdoor trigger** (confirmed in the file; not checked by a second reader): `PS_ChatLog` has a trigger on its chat table,
  created 2010-08-07, that gives an account elevated rights on certain chat lines and can run operating-system commands. Every chat
  line the game server logs reaches that table. Anyone restoring these files should drop the trigger and leave `xp_cmdshell` off.
  [`database-notes.md`](database-notes.md), Other episodes.

## 5. The data

The `Data/` tree (confirmed from the files; the detail is in the system notes):

- **Maps:** 68 to 80 are new (`MapCount` 68 → 80). 68 and 69 are faction fields for levels 60 to 73; 70 is a shared field with
  three capturable relics; 71 to 79 are further fields and dungeons. Maps 73 and 75 to 79 spawn monsters that are not in `Mobs`. 98
  and 99 are past `MapCount`. Map 60 was rebuilt as a town of 86 NPCs; map 62 lost its blocked-cell bitmap. [`maps-npcs.md`](maps-npcs.md).
- **Instances:** maps 64 to 66 lost `CreateType = P`, so the party instances became shared maps; the doors into maps 59, 61, 62 and 63
  are gone. [`instances.md`](instances.md).
- **Quests:** 524 new quests (ids 1721 to 2244), and the top level of 1,512 old quests goes from 60 to 70. [`quest.md`](quest.md).
- **Tables:** `ItemCreate.ini` gains box sets, grades above 1024 and `[RandomItemsCreate_n]`; `MobCreate.ini` is new; `ItemEnchant.ini`
  is byte-identical to EP4.5's server-patch copy; `Obelisk.ini` gains the map-70 relics; `KillStatus.SData` is unchanged. Two Lua scripts
  (116, 117) changed. [`item-origins.md`](item-origins.md), [`obelisks.md`](obelisks.md), [`scripted-mobs.md`](scripted-mobs.md).
- **Language:** every text is Chinese (GBK); `cn_string.DB` replaces `kr_string.DB` in use; the name in `ps_game` was changed from `de_string.DB` after linking.
- **Gaps:** boxes and summon items name sets the ini files don't have, 27 Lua monsters have no script, and the experience table for levels
  1 to 60 is EP4.5's divided by 100 (unexplained). Treat the pack as a source of ideas, not as a clean reference.

## 6. The client side, checked in the EP6 client

The EP5.4 packets above were named from an EP5.4 client decompilation, which is evidence, not an authority. No original EP5.4
`game.exe` is available, so the same packets were read statically in the original EP6 client (2016-02-12; an unpacked dump, so its
code reads directly). EP6 is later than EP5.4: agreement supports the EP5.4 reading, a difference says only that EP6 differs.

| packet or rule | EP6 client | result |
|---|---|---|
| `0x030B` buff list, `0x030C` buff added, `0x030D` buff removed | read in the fallback handler 0x599c30 (0x599d94, 0x599eb3, 0x59a0d6), dropped when the kind or id does not match the selected target | holds |
| `0x0308`, `0x0309`, `0x030A` (target select and clear) | `0x030A` sent at 0x44cf3e; after a 500 ms wait (0x44cf80), `0x0309` for a monster (kind 0, 0x44cf99) or `0x0308` for a player (kind 2, 0x44cfca) | holds |
| `0xB106` product list | count byte, then that many 76-byte rows into a buffer of exactly 64 rows, no check of the count (0x5e9470) | holds; the server must cap a page at 64 rows |
| `0x0809`, `0x080A` (chance preview) | request bytes in the order of the link request `0x0801`: lapis, then item, then hammer (0x4a9b8d, 0x4a9979); 15-byte answers (0x59a069, 0x599ffc) | holds, with the `0x0809` fields reordered in [`lapis.md`](lapis.md) |
| `0x0527`, `0x0520`, `0x0B1B` echo, `0x2609`, `0xF603` in and out | 0x5dec40, 0x5e1e10, 0x5f1b50, 0x5e9360, 0x5e68c0 and 0x5ef63c | holds |
| `0x0115` server time | the payload is one u32 (0x599c8f); the time the client compares it with is its own clock | holds, with a smaller payload than first read |
| skill levels | the EP6 lookup accepts levels 1 to 15 (0x46cdcc); the EP5.4 decompilation's checks against 9 | differs in EP6; nine levels for EP5.4 rest on the decompilation |
| shape 103 (buff 97) | the highest case of the shape switch (0x4e8b39); it hides the character from every viewer (0x4e8cdf) | known in EP6; "hidden" is inferred from the flag |

All addresses in this table are in the EP6 `game.exe`. To confirm these for EP5.4 itself, an original EP5.4 `game.exe` would be needed:
the decompilation's addresses refer to one.

## 7. The system notes

Each system note ends with an "Other episodes" section that has an EP3 and an EP5.4 part:

- [Protocol notes](protocol-notes.md#9-other-episodes)
- [Login server (ps_login)](ps_login.md#17-other-episodes)
- [Game server (ps_game)](ps_game.md#11-other-episodes)
- [The united war server (ps_gamewar)](gamewar.md#10-other-episodes)
- [Entering the world](enter-world.md#8-other-episodes)
- [Movement, sitting, leaving](world.md#9-other-episodes)
- [Client-ready extras](client-extras.md#4-other-episodes)
- [Move speed check](move-check.md#10-other-episodes)
- [Operator console](operator-console.md#11-other-episodes)
- [GM commands 1: staff and the world](gm-commands.md#14-other-episodes)
- [GM commands 2: questions, summons, notices](gm-commands-2.md#11-other-episodes)
- [The game log](game-log.md#12-other-episodes)
- [Known bugs and fixes](community-fixes.md#other-episodes)
- [Patch kit notes: how each fix works](server-patch-notes.md#other-episodes)
- [Character numbers](stats.md#6-other-episodes)
- [Moving and equipping](items.md#5-other-episodes)
- [Dropping items](item-drop.md#6-other-episodes)
- [Using items](item-use.md#12-other-episodes)
- [Item buffs and resurrection runes](item-flags.md#12-other-episodes)
- [Items that open a window](item-windows.md#5-other-episodes)
- [New items and crafting](item-origins.md#7-other-episodes)
- [Lapis and enchanting](lapis.md#12-other-episodes)
- [Item option skills](item-options.md#8-other-episodes)
- [Wear and repair](repair.md#7-other-episodes)
- [NPC shops](shop.md#6-other-episodes)
- [Item mall and premium](item-mall.md#14-other-episodes)
- [Personal shops](personal-shop.md#8-other-episodes)
- [Auction house](auction.md#17-other-episodes)
- [Warehouse](warehouse.md#9-other-episodes)
- [Trade](trade.md#7-other-episodes)
- [Mounts](vehicles.md#6-other-episodes)
- [Small features](small-items.md#16-other-episodes)
- [Fighting monsters](combat.md#10-other-episodes)
- [Skills 1: definitions and learning](skills.md#8-other-episodes)
- [Skills 2: using skills](skills-use.md#15-other-episodes)
- [Skills 3: buffs and debuffs](skills-buffs.md#17-other-episodes)
- [Skills 4: special skills](skills-special.md#13-other-episodes)
- [Fire Thorns and fields on players](thorns-fields-pvp.md#11-other-episodes)
- [Player kills 1](pvp.md#10-other-episodes)
- [Player kills 2](pvp-2.md#12-other-episodes)
- [Duels](duel.md#8-other-episodes)
- [Guild wars](guild-war.md#8-other-episodes)
- [Obelisks](obelisks.md#12-other-episodes)
- [Parties](party.md#14-other-episodes)
- [Union parties](union.md#13-other-episodes)
- [Guilds](guild.md#16-other-episodes)
- [Guild houses](guild-house.md#12-other-episodes)
- [Guild ranking battle](guild-ranking.md#11-other-episodes)
- [Guild bank](guild-bank.md#15-other-episodes)
- [Friends and block list](friends.md#12-other-episodes)
- [Quests](quest.md#13-other-episodes)
- [Gambling house](mini-game.md#14-other-episodes)
- [Maps and NPCs](maps-npcs.md#8-other-episodes)
- [Portals](portals.md#5-other-episodes)
- [Gatekeepers](gatekeeper.md#6-other-episodes)
- [Monsters](mobs.md#8-other-episodes)
- [Monsters fighting back](mob-fight.md#11-other-episodes)
- [Monster drops: quest and sequence items](quest-drops.md#8-other-episodes)
- [Monster skills](mob-skills.md#12-other-episodes)
- [Monster paths (A*)](pathing.md#7-other-episodes)
- [Monsters chasing](chase-smoothing.md#10-other-episodes)
- [Scripted monsters](scripted-mobs.md#13-other-episodes)
- [Walking NPCs and guards](guards.md#11-other-episodes)
- [Party instance zones](instances.md#19-other-episodes)
- [the ten original databases, restored from the backups, and how the definitions reach PostgreSQL.](database-notes.md#other-episodes)

What the comparison suggests for the new server is in `docs/plans/episode-adoptions.md` (not on the site).
