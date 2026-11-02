# Community guide notes: game data, logs and GM tools

Source **[G]**: the original files' `Docs/GuideToShaiyaConfiguration.pdf`, "Guide to Shaiya Configuration" by ProfNerwosol for elitepvpers.de and
shaiyadevteam.info (17 pages, written mostly for EP4 and later server packs). Everything here is community knowledge from people editing
a running server, **not verified against the binaries**; treat it as strong leads. Where it overlaps something we verified, that is
noted. Credits in the guide go to ProfNerwosol, lilProHacker, [GM]Zar, Shinru2004, ShaiyaUnited, Ariez, ZDH, Angel, USD, il.mane, Zen TeRRoR,
AriezOMG, Danco1990 and others.

## 1. Where the truth lives (guide section 11)

- **The database wins.** Item, skill and monster values in `PS_GameDefs` override `Item.SData`, `Cash.SData`, `Monster.SData` and `Skill.SData` in the
  client's `data.saf`. A lot of monster configuration exists only in the database. The client files drive display: names, descriptions, models, maps,
  spoken lines.
- **`NpcQuest.SData` is server-side** (our server pack ships it). Moving the client from EP4 to EP5 requires a matching `NpcQuest.SData` or monsters and NPCs go
  missing from maps. This matches what we saw: the server's file did not match any client build's size.
- Some behaviour such as whether special weapons can be traded looks client-side only (`ReqOg` in `Item.SData`).
- "Character Load Error": either a server problem, or a corrupted SData file in `data.saf`.
- Known harmless glitch: when another player comes into view the client briefly shows wrong combat messages while it loads that character.

## 2. `PS_GameDefs.Items` field meanings (sections 1 to 3, 5 to 9, 15, 16)

Columns are `Items.<name>`; many "Req" columns are reused for unrelated purposes by the original developers.

| column | meaning |
|---|---|
| `Reqluc` (smallint) | ItemID of the item used to enhance this item. IDs above 32767 are stored minus 65536 (44068 is -21468). Values above 32767 can stall the game service with a numeric overflow |
| `ReqDex` | item name colour and quality group: 0 normal, 1 worship/heroic, 2 dread, 3 legendary, 4 goddess, 5 special, 7 consumable. Also used for auction sorting |
| `Effect1`, `Effect2` | weapon minimum damage, and the amount added to get maximum damage |
| `Effect3`, `Effect4` | armour defence, and resistance |
| `ReqVg` | linking hammer success chance; for random-item bags it holds the number of the `[ItemCreate_n]` set to roll |
| `ReqWis` | cap for orange HP/MP/SP bonuses (13 means 1,300) |
| `ReqInt` | minimum item level a lapis can be linked to (10 means level 10 and above) |
| `ReqIg` | cooldown group: using one item of the group blocks the others until the timer ends |
| `ReqRec` | only 4 items use it; purpose unknown |
| `ReqOg` | 1 means the item cannot be traded or dropped (set in `Item.SData`) |
| `Attrib` | element of the damage: 1 fire, 2 water, 3 earth, 4 wind |
| `AttackTime` | swing/throw speed, 1 extremely slow to 9 extremely fast (5 normal) |
| `Special` | behaviour tag. 42 is random-item boxes, 36 is linking hammers; many more, not mapped |
| `Slot` | number of lapis slots a lapis uses when linked (level 7 status lapis); 0 for ordinary items, 1 for quest items |
| `Speed` | movement speed bonus, used by only four items (two are Sonic lapis) |
| `Drop` | no item uses it |
| `Server` | groups items, values 3 to 6, pattern unknown |
| `ReqLevel`, `Country`, `Attackfighter`, `Defensefighter`, `Patrolrogue`, `Shootrogue`, `Attackmage`, `Defensemage`, `Grow`, `ReqStr` | for **lapis** only: ten flags saying where it can be linked, in this order: weapon, helmet, upper armour, lower armour, shield, gauntlet, shoes, ring, necklace, bracelet. Set to 1 to allow. Must match `Item.SData` |

Other item facts:
- **Appearance** comes from `<Type>.ITM` files in the client (for example `01.ITM` for one-handed swords): a list of 3DO model and DDS texture pairs, plus a
  final block of entries separated by a marker whose layout varies per file. The third byte of an item in `Item.SData` is the pair index. The last field of an
  entry selects a glow (0 blue, 1 red, 2 green, 3 white; `FF FF FF FF` for none).
- **Orange stats** are stored in `PS_GameData.CharItems.CraftName` as ten two-digit values: Str, Dex, Rec, Int, Wis, Luc, HP, MP, SP, lapisia. The first six run
  1 to 99. HP, MP and SP are multiplied by 100 in game (in-game cap about 1,300). Lapisia level: weapons 1 to 20, armour 50 to 70, and higher values display
  as [30] for weapons and [80] for armour. In EP3 this column held the name of the player who linked the lapis.
- **Random-item bags** (Snob Value Box, fortune bags): `ItemCreate.ini` has `[ItemCreate_n]` sections of two tab-separated columns, an item `Grade` and a drop chance.
  A chance of 0 to 100 is a percentage; above 100 acts as a weight so that one grade is chosen more often than another (checked: in
  `ps_game.exe` every chance is a weight over the set's total, `docs/original/item-origins.md` section 5.1). The bag's `Items.ReqVg` names the set and
  `Items.Special` must be 42.
- **Durations of timed items**: in `Skill.SData` and `PS_GameDefs.Skills`, column `KeepTime`, in hours for day-long items (720 is 30 days) and seconds for shorter ones.
  The remaining time of an active item is in `PS_GameData.CharApplySkills.LastResetTime` in seconds.
- **Status points per level** are a constant inside `ps_game.exe` (an `add word ptr [reg+0x138], 9` for the UM and `7` for the HM side in the pack the guide used);
  status reset stones use another value. In a rewrite this should simply be configuration.

## 3. Faction glitch (section 10)

An account's faction lives in `PS_GameData.UserMaxGrow.Country` (0 Alliance of Light, 1 Union of Fury, 2 not yet chosen). Characters are tied to the account's `UserUID`,
so changing that value keeps characters but changes which side the account is treated as; the wrong faction's gear can then not be equipped. A rewrite should enforce
that a character's faction matches the account's.

## 4. `PS_GameLog.ActionLog` action types (section 17)

One table records nearly every player and GM action. Columns: an action type, ten numeric values (`Value1`..`Value10`) and four texts (`Text1`..`Text4`). The guide
lists the codes it has decoded (the list is not complete):

| code | action | notes on the values |
|---|---|---|
| 103 / 104 | PvP kill gained / PvP death | Text1 the other player; Text2 current kills, deaths, wins, losses |
| 107 | login | Text1 IP; Text2 money, bank, cash, first money; Text3 level, status points, skill points; Text4 stats str, dex, int, wis, rec, luc |
| 108 | logout | same texts as login, no values |
| 111 | item acquired | ItemUID, ItemID, count; Text1 name; Text2 `ItemCreate` or `Remake` when created |
| 112 | item used or lost | ItemUID, ItemID; Text2 `use_item` or the item properties; covers drops, breaking, and the removal part of crafting |
| 113 / 114 | bought from / sold to an NPC | ItemUID, ItemID, NPC number, count, price, gold afterwards |
| 115 / 116 | bought from / sold to a player | the other player's name in Text2 |
| 117 | trade between players (receiving side) | Text2 who it came from |
| 120 | lapis extraction | item and lapis ids and UIDs, cost, success flag (value 7) |
| 121 / 122 | stored in / taken from the warehouse | ItemUID, ItemID, count, bag and slot |
| 131 | quest start | |
| 141 | skill learned or raised | SkillID, level, skill points used, Text1 skill name |
| 146 | level changed (up or down) | no data |
| 151 to 156 | status point added | Str, Dex, Int, Wis, Rec, Luc |
| 161 | gold picked up | amount, gold now held |
| 164 | gatekeeper used | cost, gold left, NPC id (gatekeepers have ids 2xxx) |
| 180 | GM command used | Text1 command, Text2 target player, Text3 effect |
| 195 | PvP compensation (status points) | |
| 212 | enchant | item, lapisia, cost, success flag (value 7), Text1/2 names |
| 213 | recreation | before and after orange stats in Text2/Text3 |
| 215 | item creation | new item, the UIDs of the items used |
| 232 | log enter / leave | Text1 `Enter` or `Leave` |

Item properties are written like `7,0,0,0,0,0(Option:00000000000000000051)`: the six linked lapis type IDs, then the orange stats (`CraftName`). The log is also how the
community found item duplication: duplicated items show up as repeated trade records.

## 5. Maps (section 13)

Map IDs 0 to 64. The guide notes that it leaves out Aedes Sacra and EP5 maps. AoL is the Alliance of Light, UoF the Union of Fury.

| id | name |
|---|---|
| 0 | D-Water Borderland 1-60 pvp map |
| 1 | Erina (Keolloseu) |
| 2 | Reikeuseu (Gliter) |
| 3 | D-Water Dungeon-1 (D1) |
| 4 | D-Water Dungeon-1 Lich's Room (D1-2) |
| 5 | Cornwell's Ruin |
| 6 | Cornwell's Ruin (Asmodeus's Room) |
| 7 | Argilla Ruin |
| 8 | Argilla Ruin (Knight's Room) |
| 9 | D-Water Dungeon 2 level 1 |
| 10 | D-Water Dungeon 2 level 2 |
| 11 | D-Water Dungeon 2 level 3 - Kimuraku level |
| 12 | Cloron's Lair level 1 |
| 13 | Cloron's Lair level 2 |
| 14 | Cloron's Lair level 3 - Cloron Dragon level |
| 15 | Fantasma's Lair level 1 |
| 16 | Fantasma's Lair level 2 |
| 17 | Fantasma's Lair level 3 - Fantasma Dragon level |
| 18 | Proelium frontier - 1-15 pvp map |
| 19 | Willieoseu (Arktuis Village) - 2nd AoL map |
| 20 | Keuraijen (Starfumos) - 2nd UoF map |
| 21 | Maytreyan - AoL dungeon |
| 22 | Maytreyan boss room |
| 23 | Aidion Nekria level 1 - UoF map |
| 24 | Aidion Nekria level 2 - Astaroth's level |
| 25 | Elemental Cave - AoL dungeon |
| 26 | Ruber Chaos - UoF dungeon |
| 27 | Ruber Chaos level 2 - Queen Mago level |
| 28 | Adellia (Huigronn Stronghold) - 3rd AoL map |
| 29 | Adeurian (Arena Stronghold) - 3rd UoF map |
| 30 | Cantabilian Borderland - pvp 20-30 |
| 31 | Paros Temple - AoL pvp 20-30 dungeon |
| 32 | Rapioru Maze - UoF pvp 20-30 dungeon |
| 33 | Fedion Temple - AoL dungeon (map 2) |
| 34 | Khalamus House - UoF dungeon (map 2) |
| 35 | Apulune - AoL capital |
| 36 | Iris - UoF capital |
| 37 | Cave of Stigma - AoL dungeon (map 3) |
| 38 | Aurizen Ruin - UoF dungeon (map 3) |
| 39 | Secret Battle Arena |
| 40 | Underground Stadium |
| 41 | Prison |
| 42 | Auction House |
| 43 | Skulleron (Pandorashys) - 4th AoL map |
| 44 | Astenes (Lanhaar) - 4th UoF map |
| 45 | Deep Desert 1 – pvp map |
| 46 | Deep Desert 2 – pvp map |
| 47 | Stable Erde (Jungle) – pvp map |
| 48 | Cryptic Throne |
| 49 | Cryptic Throne |
| 50 | Guild Ranking Battle map (GRB) |
| 51 | AoL Guild House |
| 52 | UoF Guild House |
| 53 | AoL Guild Management Office |
| 54 | UoF Guild Management Office |
| 55 | Sky City level 1 |
| 56 | Sky City level 2 |
| 57 | Sky City level 3 |
| 58 | Sky City - Seraphim level |
| 59 | Fedion Temple 2 - AoL dungeon (map 2) |
| 60 | Elemental Cave 2 - corrupted!! |
| 61 | Cave of Stigma 2 - AoL dungeon (map 3) |
| 62 | Khalamus House 2 - UoF dungeon (map 2) |
| 63 | Aurizen Ruin 2 - UoF dungeon (map 3) |
| 64 | Oblivion Insula - pvp map |

## 6. GM commands and who may use them (section 18)

Permission columns are the `Users_Master.Status` staff levels: **16** full operator, then 32, 48, 64, 80. `Y` allowed, `N` refused ("Insufficient rights"), `?` untested.
Commands the author could not test (the client disconnected on use, which they took to mean allowed) are not marked in this extract; see the guide. PN is a player
name, IN an item name, GN a guild name. 69 commands listed.

| command | description | 16 | 32 | 48 | 64 | 80 |
|---|---|---|---|---|---|---|
| `/char On/off` | turns GM visibility on/off | Y | Y | Y | Y | Y |
| `/attack on/off` | allows GM to be attacked on/off | Y | Y | Y | Y | Y |
| `/amove PN` | move to a player | Y | Y | Y | Y | Y |
| `/bmove X Z [Map]` | move to X Z coordinates. Map is optional and will | Y | Y | Y | Y | Y |
| `/cmove Map` | move between maps and factions | Y | Y | Y | Y | Y |
| `/itemlv #` | makes apparent to you and only you that you have | Y | Y | Y | Y | Y |
| `/xcall #` | summon faction (# - 1,2) from current map to | Y | Y | Y | Y | Y |
| `/tacticszone` |  | ? | ? | ? | ? | ? |
| `/fogstart distance` | sets where the fog starts | Y | Y | Y | Y | Y |
| `/fogend distance` | sets where the fog ends | Y | Y | Y | Y | Y |
| `/gmove` |  | Y | Y | Y | Y | Y |
| `/warning PN “message”` | give player a warning | Y | Y | Y | Y | Y |
| `/watch` |  | Y | Y | Y | Y | Y |
| `/cwatch` |  | Y | Y | Y | Y | Y |
| `/silence PN on/off` | mute player on/off | Y | Y | Y | Y | Y |
| `/stop PN on/off` | prevent player from moving on/off | Y | Y | Y | Y | Y |
| `/quiry PN` | lists all items held by a player | Y | Y | Y | Y | N |
| `/asummon PN` | summon player to you | Y | Y | Y | Y | N |
| `/bsummon PN X Z Map` | summon player to these coordinates | Y | Y | Y | Y | N |
| `/partysummon` |  | Y | Y | Y | Y | Y |
| `/item “IN”` | show detailed information about “item_name” | Y | Y | Y | Y | N |
| `/mob` |  | Y | Y | Y | Y | N |
| `/guildgrade` |  | Y | Y | Y | Y | N |
| `/guildrank GN R#` | set rank (R#) to guild name (GN) | Y | Y | Y | Y | N |
| `/guildpoint GN P#` | set guild points (P#) to guild name (GN) | Y | Y | Y | Y | N |
| `/gmnotice` | send notice to GM's | Y | Y | Y | Y | N |
| `/cnotice “message”` | send notice to your faction only | Y | Y | Y | Y | N |
| `/wnotice PN “message”` | send notice to tha player (PN) only | Y | Y | Y | Y | N |
| `/notice [T] “message”` | send a server notice (“M”), time (T) in seconds is | Y | Y | Y | Y | N |
| `/znotice “text”` | send notice to the map you're on only | Y | Y | Y | Y | N |
| `/bnotice` |  | Y | Y | Y | Y | N |
| `/cure [PN]` | cures you or player when name is specified | Y | Y | Y | N | N |
| `/autocure` |  | ? | ? | ? | ? | ? |
| `/npautocure` |  | ? | ? | ? | ? | ? |
| `/iclear PN` | deletes player's inventory | Y | Y | Y | N | N |
| `/eclear PN` | deletes player's equipment | Y | Y | Y | N | N |
| `/queston DC` |  | Y | Y | Y | N | N |
| `/questoff DC` |  | Y | Y | Y | N | N |
| `/questadd DC` |  | Y | Y | Y | N | N |
| `/pvpon DC` |  | Y | Y | Y | N | N |
| `/pvpoff DC` |  | Y | Y | Y | N | N |
| `/questrem DC` |  | Y | Y | Y | N | N |
| `/event` |  | ? | ? | ? | ? | ? |
| `/eventadd` |  | ? | ? | ? | ? | ? |
| `/eventrem` |  | ? | ? | ? | ? | ? |
| `/kick PN` | kicks player from server | Y | Y | Y | N | N |
| `/kickcan DC` |  | Y | Y | Y | N | N |
| `/ban PN DC` | bans player from server | Y | Y | Y | N | N |
| `/bancan DC` |  | Y | Y | Y | N | N |
| `/set PN Status #` | modify [player, status, amount] (Grow, Level, Exp, | Y | Y | N | N | N |
| `/imake “IN”` | create item with item_name (IN) | Y | Y | N | N | N |
| `/auctionsearch PN` | find all auctions of a player (PN) | Y | Y | N | N | N |
| `/recallitem` |  | Y | Y | N | N | N |
| `/mmake MobID` | create a monster | Y | Y | N | N | N |
| `/mera MobID` | remove monster with MobID | Y | Y | N | N | N |
| `/mera t` | remove selected monster | Y | Y | N | N | N |
| `/nmake NpcID` | create NPC with NpcID | Y | Y | N | N | N |
| `/nera NpcID` | remove NPC with NpcID | Y | Y | N | N | N |
| `/akick` |  | Y | Y | N | N | N |
| `/abancan DC` |  | Y | Y | N | N | N |
| `/aban DC` |  | Y | Y | N | N | N |
| `/server on` |  | Y | Y | N | N | N |
| `/server off` |  | Y | Y | N | N | N |
| `/shutdown DC` |  | ? | ? | ? | ? | ? |
| `/chatcolor 1-9` | change chat colour | Y | Y | Y | Y | Y |
| `/apower` |  | Y | N | N | N | N |
| `/apowercan` |  | Y | N | N | N | N |
| `/auctionrecall PN MID` | remove an item from a player's auction by market ID | Y | Y | N | N | N |
| `/getitem TY ID #` | get an item by type, type ID and optional quantity | Y | Y | N | N | N |

Observations that matter for a rewrite. The permissions form clean nested tiers, so a rewrite can express them as a minimum staff level per command:
- **All five levels (17 commands):** `/char`, `/attack`, `/amove`, `/bmove`, `/cmove`, `/itemlv`, `/xcall`, `/fogstart`, `/fogend`, `/gmove`, `/warning`, `/watch`,
  `/cwatch`, `/silence`, `/stop`, `/partysummon`, `/chatcolor`.
- **Levels 16 to 64, not 80 (14):** `/quiry` (list a player's items), `/asummon`, `/bsummon`, `/item`, `/mob`, `/guildgrade`, `/guildrank`, `/guildpoint` and the notice
  commands (`/gmnotice`, `/cnotice`, `/wnotice`, `/notice`, `/znotice`, `/bnotice`).
- **Levels 16 to 48 (13):** `/cure`, `/iclear`, `/eclear`, the quest commands, `/pvpon`, `/pvpoff`, `/kick`, `/kickcan`, `/ban`, `/bancan`.
- **Levels 16 and 32 only (16):** `/set`, `/imake`, `/getitem`, `/recallitem`, the auction commands, `/mmake`, `/mera`, `/nmake`, `/nera`, `/akick`, `/aban`, `/abancan`, `/server on|off`.
- **Level 16 only (2):** `/apower` and `/apowercan`.
- **Untested (7):** `/tacticszone`, `/autocure`, `/npautocure`, `/event`, `/eventadd`, `/eventrem`, `/shutdown`.
- `/set` edits grow, level, exp, money, stat and skill points, the six stats, five more attributes (HG, VG, CG, OG, IG) and kills and deaths.
- Every GM action is logged as action type 180.

## 7. Open questions this raises

- Which of these behaviours exist in the EP4.5 server binary, and which came with later episodes? The guide is a mix.
- The meaning of `Special` values other than 36 and 42, and of the `Server` column.
- Whether `ps_game.exe` really reads the `ItemCreate.ini` format above (the file is in our server pack, so it can be checked).
