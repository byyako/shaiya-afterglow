# The EP3 server pack, compared with EP4.5

This page compares the EP3 server pack's programs, databases and data with the EP4.5 original that the rest of these notes describe.
EP3 is the older side: its programs were linked 18 to 20 months before EP4.5's. The page gives the binaries, what EP4.5 added or
changed, and links to the "Other episodes" section of each system note, where the detail is. It was static analysis only: nothing from
the pack was executed. EP3 has no PDBs; function names were carried over from EP4.5 by identical code after masking addresses,
unique strings, call-graph shape and switch tables, and every address below is in the named EP3 binary.

Status words as in `docs/README.md`: **confirmed** (read in the binary or file, with the address or the file), **inferred**.

## In short

- **The same programs, smaller.** 1937 EP4.5 `ps_game` functions have an EP3 counterpart, 819 of them identical after masking
  addresses. The connection states, opcode group table, character screen, enter-world burst, move check, level cap (60), hit and damage
  formulas, damage cap (30000), monster AI and A* are the same (confirmed).
- **What EP4.5 added** (confirmed): the auction house, union (raid) parties, enchanting with crafted stats, rune compose, remake and
  appraisal, Lua scripted monsters, party instances, nine elements and absorption (EP3 has five and none), the war-server move, the
  reconnect path, a second DB-agent link, a stalled-thread watchdog with crash dumps, and hot reloading from the console. 42 client
  opcodes in all.
- **What EP3 has that EP4.5 lost** (confirmed): an answer to the item-mall product list request `0xB106`; helmets and cloaks kept out of
  monster drops and boxes; the lapis `exp` column as a percent experience bonus (EP4.5 reuses the byte for absorption).
- **The community-fixed bugs are older than EP4.5.** Every EP4.5 flaw checked in these systems is already in EP3; none is fixed there.
- **`ps_login`**: the EP3 build is also the base of the EP5.4 `ps_login`. EP4.5 cuts the user name and password at 12 characters and
  hashes the password before the login procedure; EP3 sends them in plain text to a different procedure, which **accepts any password
  for an existing account** in the shipped database (section 4).
- **Databases:** a separately built, thinner set (SQL Server 2000 files) with few keys or defaults; `PS_GameDefs` is an empty stub.
- **Data:** maps 0 to 44 only, no Lua, no `ItemEnchant.ini`, fewer AI files, an older `NpcQuest.SData`.

## 1. The binaries

| program | linked (UTC) | linker | size | SHA-256 | PDB path (CodeView) |
|---|---|---|---|---|---|
| `ps_game.exe` | 2008-01-17 05:42:34 | 7.10 (VS 2003) | 1069056 | `a843dcbd1f767954bca0e1c6a9018948c8e1fc2289beae11b1dfcd69e8c816da` | `d:\SonoV\Bin.net\ps_game.pdb` |
| `ps_login.exe` | 2007-12-07 02:46:58 | 7.10 | 319488 | `2f9a0b2a47209861f479a89d38268918d2ee39bf8f1afceb4644789f537c4b0f` | `d:\SonoV\Bin.net\ps_login.pdb` |
| `ps_session.exe` | 2007-12-07 02:47:05 | 7.10 | 311296 | `31ec1b5cf818c403034da3b321bddbb02a97f47ad47840907212344185c76a3f` | `d:\SonoV\Bin.net\ps_session.pdb` |
| `ps_dbAgent.exe` | 2008-01-04 11:46:22 | 7.10 | 409600 | `ee9ee6c5494dd69fbb6e18e66a88b6f21ffb21d593603c35cdf020725aa751fc` | `d:\SonoV\Bin.net\ps_dbAgent.pdb` |
| `ps_gameLog.exe` | 2007-12-07 02:46:39 | 7.10 | 299008 | `d19bb3a2dbef263540e5888d4c36baaac534b80a9ed330b38e24cfe8e35c3ab7` | `d:\SonoV\Bin.net\ps_gameLog.pdb` |
| `ps_userLog.exe` | 2007-12-07 02:47:10 | 7.10 | 282624 | `de0ee6f1b6542eec95fcc70b819827d90ee261a869faaeaba18e582cbf5fdb3b` | `d:\SonoV\Bin.net\ps_userLog.pdb` |

All are in the pack's `PSM_Client/Bin/`, with no PDBs; there is no `ps_gamewar` ([`gamewar.md`](gamewar.md)). Every file carries the
date 2009-03-26 07:14, the date the pack was put together, not a build date. VS 2003 builds store no PE checksum, so whether a file was
changed after linking can't be read from its header; the EP3 `ps_login` is the unchanged base of the EP5.4 one ([`ep5.4.md`](ep5.4.md)).
Each pairs with the EP4.5 program of the same name ([`server-inventory.md`](server-inventory.md)). The full file list is in
`docs/original/inventory/server-ep3/`.

## 2. The game server

In outline (confirmed unless marked; the detail is in each system note):

- **Protocol.** The same dispatch structure and receive path, including the 1000-packet receive cap. EP4.5 added 42 client opcodes:
  `0x0307`, `0x0559`, `0x0805` to `0x0808` (enchant, rune, remake, appraisal), `0x0B0C` to `0x0B18` and `0x0B1A` (union parties),
  `0x1112`, `0xF10A`, `0x1C01` to `0x1C0D` and `0x1C14` to `0x1C17` (auction), and the staff opcodes `0xFA18` to `0xFA1A`. EP4.5 also
  added server packets: absorption `0x0525`, scouting `0x0558`, the server time and reconnect packets `0x0112` to `0x0114`, billing
  info `0xB103` and the server move `0x0110`/`0x0111`. EP3's only extra is the answer to `0xB106` in its pre-world state.
  [`protocol-notes.md`](protocol-notes.md), [`ps_game.md`](ps_game.md).
- **Characters.** EP3 trims the 21-byte name of a new character at offset 10, as EP4.5 does, so the 19-byte EP3 name in the
  protocol notes is not what the EP3 server enforces. [`enter-world.md`](enter-world.md).
- **Combat.** Five elements (a 5 × 5 table, the top-left of EP4.5's 9 × 9), no absorption; the lapis `exp` column adds a percentage
  to experience gained (EP3 ps_game 0x44018b, 0x442133). Buff kinds 94 and 95 (element buffs) came with EP4.5. Skill records are 156
  bytes (EP4.5: 160). [`combat.md`](combat.md), [`lapis.md`](lapis.md).
- **Monsters.** No Lua scripted monsters (no Lua library at all). Guards lack the 5-second one-guard-per-target lock (confirmed: their
  target search reads no clock). [`scripted-mobs.md`](scripted-mobs.md), [`guards.md`](guards.md).
- **Items.** No enchanting and no crafted stats: there is no `0x0805` handler, no enchant table, and the equipment code has no crafted
  values loop. The warehouse's second part starts at slot 40 (EP4.5: 120). Helmets and cloaks are refused in drops and boxes (EP3
  ps_game 0x48763a, 0x44db80), which is what `community-fixes.md` describes as "helmets and cloaks never drop". No item specials 59, 64
  or 65. [`items.md`](items.md), [`item-drop.md`](item-drop.md), [`warehouse.md`](warehouse.md).
- **Systems absent from EP3:** the auction house, union parties, party instances, the war-server move. [`auction.md`](auction.md),
  [`union.md`](union.md), [`instances.md`](instances.md).
- **Guild houses.** The lapis-mix discount needs guild ranking 6 or better (EP4.5: 14). [`guild-house.md`](guild-house.md).
- **Structure.** EP4.5 added a second DB-agent connection, a watchdog that reports a stalled world-database or send thread after 15 s,
  crash dumps, console hot reloading of items, monsters and skills, and an `IsSafePos` log on zone entry.

## 3. The other programs

- **`ps_login`**: the network library, RSA/AES handshake, server list and session messages are the same as EP4.5's. EP3 reads up to
  31 and 32 characters of user name and password (EP4.5: 12), sends the password in plain text, and calls
  `usp_Try_GameLogin_Taiwan` (EP4.5: an MD5 hash and `usp_try_gamelogin_korea`). Its `0x701` to the session server is 0x63 bytes
  (EP4.5: 0x67). [`ps_login.md`](ps_login.md), Other episodes.
- **`ps_session`**: the same core; it lacks what came with the war server and the staff message relay `0x80E`.
- **`ps_dbAgent`**: no market group (0x0F) and narrower switch tables in four groups; the item-mall procedures are named
  `*_PayLetterT`. [`database-notes.md`](database-notes.md), Other episodes.
- **`ps_gameLog`**: no party group, no market records, 12 character, 25 item and 7 keep records (EP4.5: 16, 47, 12).
  [`game-log.md`](game-log.md), Other episodes.
- **`ps_userLog`**: no Passpod (second password) case.

## 4. The databases

The pack's `PSM_Data/` holds ten databases as SQL Server 2000 data files (database version 539), described in `docs/original/db/ep3/`;
`docs/original/db/README.md` says how they were read. EP3 has `PS_Monitor` and `PS_Statistics` where EP4.5 has `PS_StatData` and
`PS_Statics`.

- **A thinner, separately built set** (confirmed): made 2009-01 to 2009-04, no primary keys or defaults on most tables, different name
  case (`CHARS`, `charitems`, `UserMaxgrow`), fewer tables (`PS_GameData` 27 against 54, `PS_Billing` 3 against 16).
- **`PS_GameDefs` is a stub**: its tables have one placeholder column each and no rows, so EP3's game definitions are not in the pack.
  `OMG_GameWeb` and `PS_Monitor` are empty.
- **The login procedure accepts any password** (confirmed in the file, read twice: the second time in the raw `PS_UserData_Data.MDF`,
  byte 839365): in `usp_Try_GameLogin_Taiwan` the password check is commented out and the result preset to success. Don't use this database for anything reachable.
- The item-mall delivery procedure `usp_Insert_Product_User_E` is in clear text here (EP4.5 and EP5.4 encrypt it): one transaction,
  a 240-slot cap, and an order number as the key.

## 5. The data

The `Data/` tree (confirmed from the files):

- **Maps 0 to 44 only**; of the shared maps, 30 are byte-identical to EP4.5's and 15 differ. Most differences are fewer NPCs and
  portals; map 42 is a different map. [`maps-npcs.md`](maps-npcs.md).
- **No `Lua/`, no `ItemEnchant.ini`, `MapWar.ini` or `ObeliskWar.ini`.** 140 of EP4.5's 180 AI files are identical, 12 differ, 28 are
  missing.
- **`Obelisk.ini`** has the four map-0 "roads" that EP4.5 left out, but they point at a portal id that no map has.
  [`obelisks.md`](obelisks.md).
- **Sequence items are switched off** (`COUNT=0`). [`quest-drops.md`](quest-drops.md).
- **The string table `tw_string.db` is Japanese** (Shift-JIS), despite its name.

## 6. The system notes

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
