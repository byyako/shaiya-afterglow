# Guild ranking battle

This page covers the weekly guild ranking battle in the EP4.5 game server `ps_game.exe`: the open times and the
clock, the notices, going in once per battle, each guild's own copy of map 50, the close, battle points from monster kills, and the
ranking update that ranks the guilds and opens or closes their houses. It was read in the Ghidra decompiler and the disassembly with the
PDB names and types. Data: `Data/Map.ini` (map 50), the map files and monsters of map 50. Client texts: `sysmsg-uni.txt`. Nothing was
executed.

Read: `CWorldSendThread::Run` 0x406f90 (0x407022…0x4070a3), `CWorldThread::Update` 0x407390 (0x407896…0x407976),
`CWorld::CheckGuildRankNotices` 0x414370, `CWorld::CheckGuildRankStatus` 0x4143e0, `CMap::IsGuildRankMapOpenTime` 0x428d20,
`CMap::IsGuildRankMapCloseTime` 0x428df0, `CMap::GetGuildRankNotices` 0x428eb0, `CZone::MoveAllBindPos` 0x423050, `CZone::SendAll`
0x4207f0, `CWorld::SendAllCountry` 0x412f50, `CUser::PacketMaininterface` 0x468420 (`0x020A` / `0x0227`, case 5),
`CWorld::GetZoneByMapID` 0x413df0 (case 5), `CUser::ZoneEnterUserMove` 0x44b8a0, `CGuild::ConnectUser` 0x42d1c0,
`CGuild::DisconnectUser` 0x42d300, `CMob::ItemDrop` 0x4a5ea0 (0x4a614b…0x4a61f0), `CGuild::SendRankingZone` 0x42c6e0,
`CGuildMgr::UpdateGuildPointRank` 0x432b70, `CGuildMgr::SendGuildList` 0x432000. The client side is on
[Social packets](../client/client-social-packets.md) (`0x0D26`, `0x0D27`, `0x0D37`).

The house itself is on [Guild houses](guild-house.md), the guild bank on [Guild bank](guild-bank.md). Not covered:
`CreateType` G, the operator commands (`FA12`, `FA14`, `FA15`), the game log records, and `CGuild::GuildPvPWin` / `GuildPvPLoss` (never
called, see [Guild wars](guild-war.md)).

## In short

- **On Sunday 21:00–22:00** (server local time) every guild may send its members into **its own copy of map 50**. Each member may go
  in **once per battle**; dying or logging out there ends that member's battle.
- **There is no ranking, faction or house requirement** to take part, beyond the portal's own faction.
- **Monsters there drop no gold.** Each kill gives the killer's guild **battle points** instead: 1 to 3 for ordinary monsters, 3000 to
  10000 for the three bosses.
- **At 22:00 everyone still inside is sent back** to the guild house halls within a minute.
- **Thirty minutes later the guilds are ranked**, per faction, by battle points plus a bonus for Etin returns and entries (at most half
  the battle points). Places 1 to 30 get that ranking; a guild with a house pays its upkeep and its house opens, or closes when the
  guild is short of Etin.
- **Every other guild becomes unranked**, however high it was, and its house closes. A guild that sat out one battle loses its rank.
- **Nothing is saved before the ranking**: a server restart during the battle or in the 30 minutes after loses the points, or the
  whole week's ranking.

Status words: **confirmed** = read in the code (address given); **inferred** = follows from what was read.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names and types), read with the decompiler (REA) and the disassembly (objdump):
`CWorldSendThread::Run` 0x406f90 (0x407022…0x4070a3), `CWorldThread::Update` 0x407390 (0x407896…0x407976), `CWorld::CheckGuildRankNotices`
0x414370, `CWorld::CheckGuildRankStatus` 0x4143e0, `CMap::IsGuildRankMapOpenTime` 0x428d20, `CMap::IsGuildRankMapCloseTime` 0x428df0,
`CMap::GetGuildRankNotices` 0x428eb0, `CZone::MoveAllBindPos` 0x423050, `CZone::SendAll` 0x4207f0, `CWorld::SendAllCountry` 0x412f50,
`CUser::PacketMaininterface` 0x468420 (`0x020A` / `0x0227`, case 5), `CWorld::GetZoneByMapID` 0x413df0 (case 5, 0x4140cd…0x4141d7),
`CUser::ZoneEnterUserMove` 0x44b8a0, `CGuild::ConnectUser` 0x42d1c0, `CGuild::DisconnectUser` 0x42d300, `CMob::ItemDrop` 0x4a5ea0
(0x4a614b…0x4a61f0), `CGuild::SendRankingZone` 0x42c6e0, `CGuildMgr::UpdateGuildPointRank` 0x432b70, `CGuildMgr::SendGuildList` 0x432000.
Layouts from the PDB: `stMOBInfo`, `stGuildUserInfo`, `TP_GUILD_RANKING_NOTICS`, `TP_GUILD_UPDATE`. Data: `Data/Map.ini` (map 50), the
map files and monsters of map 50 (`world.maps`, `world.map_portals`, `world.mob_areas`, `defs.mobs`). Client texts: `sysmsg-uni.txt`.
Nothing was executed. The client side is `docs/client/client-social-packets.md` (`0x0D26`, `0x0D27`, `0x0D37`) and
`docs/client/CLIENT-FACTS.md` (the `0x020A` refusals and the question that sends `0x0227`).

Part 1 (`docs/original/guild-house.md`) covers the house itself; its section 8 (the ranking update) is restated and completed here, and its
deviation 1 (no ranking update) ends when this is built. **Not covered:** the guild bank (`0x0D29`, `0x0D31`, `0x0D32`; part 3,
`docs/original/guild-bank.md`), `CreateType` G, the operator commands (`FA12`, `FA14`, `FA15`, and the operator zone choice `CUser +0x54c8`
that `GetZoneByMapID` honours for admin levels up to 10), the game log records, `CGuild::GuildPvPWin` / `GuildPvPLoss` (never called,
`docs/original/guild-war.md`).

## 1. What it is

On Sunday 21:00–22:00 (server local time) every guild may send its members into **its own copy of map 50** (a zone per guild, as the
houses). Each member may go in **once per battle**. Monsters there drop no gold; instead each kill gives the killer's guild **battle
points**. Thirty minutes after the battle closes, every guild's score (battle points plus a bonus for Etin returns and entries) ranks the
guilds of each country; places 1 to 30 keep or get a ranking and, with a house, pay its upkeep and open it. Everyone else becomes
unranked, and their house closes.

## 2. The open times

Confirmed, `IsGuildRankMapOpenTime`, `IsGuildRankMapCloseTime`.

`CreateTime` of the map (`CMap +0x1a8`, entries `(u16 weekday, u16 hour, u16 hours)`, weekday 0 Sunday, `0xffff` ends the list). **Only
the first two entries are read** (the party maps read seven). Open: on the entry's weekday from `hour` while the hour is below
`hour + hours`; an entry that runs past 24 also on the next weekday (Saturday's next is Sunday) while the hour is below `hour + hours − 24`.
This is the party maps' rule (`docs/original/instances.md` section 2) with two entries. `IsGuildRankMapCloseTime` is its negation (a map
without entries is "closed"). Shipped: map 50 `(0, 21, 1)`.

**The battle's tag** (`IsGuildRankMapOpenTime`'s out value while open): `start hour << 24 | entry weekday << 16 | day of month << 8 |
month`. It names one battle; section 4 compares it.

Which map: `CheckGuildRankStatus` and `CheckGuildRankNotices` look at the maps of `CreateType` R (5); status stops at the first one
(map 50, the only one shipped).

## 3. The clock

Confirmed, `CWorldSendThread::Run`, `CWorldThread::Update`.

- **Every 60 s** (`CheckGuildRankStatus`): while the map is open, a flag is set. When it is found closed with the flag set, the flag is
  cleared and the **ranking update is due 30 minutes later** (1,800,000 ms); when that time passes, `UpdateGuildPointRank` runs once
  (section 7). Nothing is stored: a server that is down when the battle closes, or restarts in the 30 minutes after, skips that update.
- **Every 59 s** (`CheckGuildRankNotices`): the notice type is reset to none, then set from the clock (below) for each R map.
- **Every 60 s, per world thread** (`CWorldThread::Update`): (a) every zone of an R map that has players, while the map is closed: all of
  them are moved out (section 6); (b) when a notice type is set: **`0x0D26`** to every player in every zone of that thread (so to
  everyone in the world).

**The notice** (`GetGuildRankNotices`, per entry, the minute must match exactly):

| when (local time) | type | client text |
|---|---|---|
| on the entry's weekday, at `hour − 1`:30 and `hour − 1`:50 (not for an entry starting at hour 0) | `hour` (the start hour, below 24) | 8000 "From <hour>, guild ranking battle will get started later… please wait in guild house" |
| at `hour`:00 | 100 | 8001 "The guild ranking battle has just started. There is only 1 chance to join…" |
| at (end hour − 1):50 | 101 | 8002 "…will be finished in 10 minutes…" |
| at (end hour − 1):59 | 102 | 8053 "After 1 minute, guild ranking battle will be finished then all guild members will be transferred to guild house automatically…" |

(For an entry past midnight the last two come on the next weekday.) **`0x0D26`** (5 bytes): `u16 map id` (the R map, 50), `u8 type`.
Since the type lives 59 s and is sent on a separate 60 s beat, a notice is usually sent once, but now and then never (the beats drift by
1 s a minute).

## 4. Going in: `0x020A` / `0x0227` to an R map

Confirmed, `PacketMaininterface` case 5.

Client → server: `u8 portal` (`0x0227` is handled by the same code as `0x020A`). After the usual portal checks
(`docs/original/portals.md` section 2), for a destination of `CreateType` 5, in order:

| check | answer `0x020A u8 portal, u8 reason` | client |
|---|---|---|
| no guild | 0 | 8038 |
| the map is not open | 1 | 8039 "no time for Ranking battle." |
| the player is **on an R map already**, or its tag is not this battle's | the move (no answer) | |
| otherwise (it took part in this battle and is outside now) | 2 | 8040 "already participated in Ranking battle." |

There is **no ranking, country or house requirement** (beyond the portal's own country). The client's reason 5 (question 8043 "Ranking
Battle zone. You can only participate once in the zone. Do you want to participate?", whose yes sends `0x0227`) is never sent.

**Shipped portals to map 50**: map 53 portal 0 (country 0) and map 54 portal 1 (country 2), both to (507.78, 3.08, 98.41) (53 and 54 are
the two countries' guild house halls, where part 1's house portals also are).

**The player's tag** (`CUser +0x54c4`): 0 for a new character object; on arrival in an R zone while the map is open it becomes the
battle's tag. It survives logging out through the guild member entry (`stGuildUserInfo +0x38`, `dwRankingZoneEnterFlag`:
`DisconnectUser` copies it there, `ConnectUser` back), in memory only. A member who left the guild and joined another starts the next
login with the new entry's 0.

## 5. The zone

Confirmed, `GetZoneByMapID` case 5, `ZoneEnterUserMove`.

Found when the player arrives: **key = the guild's id**; refused (no zone, connection closed) without a guild, when the map is closed,
or when the player's tag is already this battle's. Otherwise the guild's zone of the map, made now if there is none (the map's monsters
and NPCs, as any zone; map 50 has one NPC, type 1 id 219). Released as party zones when empty (`ExpireTime` 2: 2.5 minutes).

**On arrival** (`ZoneEnterUserMove`, map type 5, while the map is open): the player's tag = the battle's, and **the guild's entry count
(`CGuild +0xa0`) + 1**. Each member can therefore count once per battle.

Map 50 inside: portals 0, 1 and 2 (country 0, levels 16–30, 1–15 and 31–60) lead to other spots of map 50 (a move within the zone;
the case 5 check lets a player already on an R map through). Portals 3, 4 and 5 are instance portals 1003, 1001, 1002: no zone pair
sets them on map 50 (`docs/original/instances.md` section 5), so **nobody passes** on the new server. (Correction: the
shipped `Obelisk.ini` does have boss groups for them, `[bothmob_021]` to `[bothmob_023]`; killing each boss opens its portal for everyone
in that zone, `docs/original/obelisks.md` section 11.3; built: each guild's zone has the three bosses, and their deaths open
that zone's portals 3 to 5.)
Logging in on map 50 and coming back to life there: the map's `RebirthMapPos1` / `RebirthMapPos2` by country (map 53 (50.43, 7.09, 86.57),
map 54 (97.33, 5.89, 42.03)), as for every map with a `CreateType`. So a member who dies or logs out is out of that battle.

## 6. The end

Confirmed, `CWorldThread::Update` 0x4078b0, `CZone::MoveAllBindPos`.

Every 60 s, each zone of an R map with players while the map is closed: every player in it whose logout is not pending gets a move to
its bind position for its country (`CMap::GetBindPos`: map 50's rebirth points above), due 200 ms later, the move type of a gatekeeper.
So up to a minute after 22:00 everyone is back in the halls of maps 53 and 54. Nothing else happens at the close; the points stay until
the ranking update.

## 7. Battle points

Confirmed, `CMob::ItemDrop` 0x4a614b, `CGuild::SendRankingZone`.

When a monster dies **in a zone of an R map**, after its item drops, if its definition has **`Money1` (`wMoney`) 0 and `Money2`
(`wMoneyAdd`) above 0**: no gold is dropped; instead, if the drop's owner (the killer of record, `docs/original/combat.md` section 7)
has a guild, `SendRankingZone(Money2)`:

1. the guild's **battle points** (`CGuild +0x98`) + `Money2`;
2. if they are now **above** its country's best (a pair per country, `0x5471d0` points / `0x5471d8` guild id, 0 at start), that pair
   becomes (these points, this guild);
3. **`0x0D27`** (14 bytes): `u32 the guild's battle points, u32 its country's best points, u32 that best guild's id`, to every online
   member of the guild whose zone is on an R map;
4. the database gets `0x0B0B` with the guild's **stored** points (`+0x44`, unchanged: nothing is saved) and the game log a record.

(`Money2` is the gold's random part elsewhere; with `Money1` 0 such a monster drops gold only outside R maps.)

**Map 50's monsters** (all with `Money1` 0):

| monsters | level | points each | on the map |
|---|---|---|---|
| 1856, 1857 | 16, 17 | 2 | 33 each, x 470–550, z 169–369 |
| 1858, 1859 | 32 | 3 | 48 each, x 175–420, z 90–275 |
| 1860, 1861 | 33 | 3 | 53 each, x 120–421, z 323–723 |
| 1862–1865 | 59–61 | 1 | 25–40 each, x 636–955, z 84–651 |
| 1854 (boss) | 33 | 3000 | 1, (232, 258) |
| 1852 (boss) | 63 | 4000 | 1, (671, 293) |
| 1850 (boss) | 65 | 10000 | 1, (500, 789) |

(1685, two in one area, has no definition and never spawns.) The bosses have no Lua script (AI 1). Points are kept in memory only.

## 8. The ranking update

Confirmed, `CGuildMgr::UpdateGuildPointRank`.

For every guild (in the guild manager's order, ascending id):

1. **score** = battle points + **min(Etin returns + 25 × entries, battle points / 2)** (integer halves), and 0 without battle points;
2. battle points, Etin returns and entries go back to 0; **the guild's points (`dwGuildPoint`) = the score**;
3. a score of 0: **ranking 31, `byUseHouse` 0**; otherwise the guild joins its country's list (country 0 Light, anything else Fury).

Then per country, **highest score first** (a multimap by score walked from the top: equal scores, the guild added later, so the higher
id, first — *inferred* from the container), place 1, 2, …:

- places **1 to 30**: ranking = the place. A guild that **bought** its house: Etin below the keep Etin → `byUseHouse` **2**; otherwise
  **Etin − keep Etin**, `byUseHouse` **1**, and its online members get **`0x0D23`** (`u32` Etin). Without a house nothing else changes.
- places 31 and on: **ranking 31, `byUseHouse` 0**.

The database gets every guild (`0x0B16`, 14 bytes each, 30 a packet: `u32 guild, u32 points, u32 Etin, u8 ranking, u8 byUseHouse`).
Each country's players get **`0x0D37`** (`CWorld::SendAllCountry`): `u8 count`, then per ranked guild of that country in place order
`u32 guild id, u32 score, u8 place` (3 + 9n bytes, n ≤ 30). The client sets every guild of its list to ranking 31, then the listed
ones, and prints 8054 "Each guild's gained point has been settled so guild ranking is renewed…". Last, both countries' best pairs go
back to 0.

The house keeps its NPC levels whatever happens; a closed house (`byUseHouse` 0 or 2) opens again at a later ranking that places the
guild with enough Etin.

**The guild list** (`0x0D2F`, `SendGuildList`) carries each guild's ranking (`+0x48`) and points (`+0x44`), so a ranking shows there
from the next login; `0x0D30` for a new guild has ranking 31 and points 0.

## 9. Original oddities

- **The client's ranking-battle question is never used**: `0x020A` enters at once (the client would ask "You can only participate once…"
  on reason 5 and send `0x0227`).
- **A notice can be skipped** (section 3's two beats), and one minute of the start hour can pass before players inside a closed map are
  moved out.
- **Nothing is saved before the update**: a restart during the battle or in the 30 minutes after loses the battle's points and
  entries, or the update itself (with it the week's ranking).
- **A guild that sat out a battle loses its ranking and its house's opening**, however high it was.
- **The per-player tag lives in the guild's member entry**, so a member who changes guilds and logs in again may enter a second time
  for the new guild.
- **The bonus counts entries, not people**: every arrival counts, but the tag allows one per member per battle, so it is the number of
  members who went in.
- `0x0D27`'s "best" pair only grows during a battle and is reset at the update, not at the start.

## 10. The new server

Built (`docs/plans/done/guild-ranking.md`); `cargo test` covers the clock, the scores, the guild side, the zone
and the database write. Not yet tried with the client: section 35 of `docs/testing/live-test-checklist.md`.

- **Data**: `world.maps` already has map 50's `create_time` (`{0, 21, 1}`), `expire_time` (2) and rebirth points. The R maps' rules are
  loaded like the party and house maps' (`InstanceRules`), with **two** time entries for R.
- **The clock** (`crate::ranking`, pure): fed the local time once a minute (at each new minute of the local clock), it says: the
  notice of that minute (section 3's table), whether the battle is open, its tag (`(date, start hour)` of the open entry), and when the
  update is due (30 minutes after a close seen while running). A server task calls it every minute: notices to everyone
  (`0x0D26`), the close (section 6), the update (section 8). Test settings: `ranking_times` (e.g. `"(1,14,1)"`, replaces map 50's
  `CreateTime`) and `ranking_update_delay_secs` (default 1800).
- **Guild state** (`crate::guild`): per guild `battle_points` and `battle_entries` (memory only, as the original), with the stored
  `etin_returns`; per country the best `(points, guild)`; per character the tag of the last battle entered (memory only, deviation 3).
- **Going in**: `0x020A` and **`0x0227`** to an R map: section 4's order; a player already in an R zone moves within its zone. The zone
  is the guild's instance zone of map 50 (`instance::Owner::Guild`, as the house zones; the zone key keeps houses and battles apart by
  map). The portal books the player into it; **arrival** counts the entry and sets the tag.
- **Points**: the kill pass, for a monster that dies in a zone of map type R with `money` 0 and `money_add` above 0: no gold; the
  owner's guild gets `money_add` points and its members in R zones `0x0D27`.
- **The update**: section 8 in one place (`crate::ranking` scores and places, `crate::guild` applies), one database transaction
  (`GuildWrite::Ranking`: every guild's points, Etin, ranking, `use_house`, and Etin returns 0), `0x0D23` to the guilds that paid,
  `0x0D37` per country. The guild list's units carry the real ranking and points.
- **Packets**: `0x0D26`, `0x0D27`, `0x0D37` as above; `0x0227` parsed as `0x020A`.
- **Where**: `crate::ranking` (clock, scores, places), `crate::guild` (`count_entry`, `add_battle_points`, `apply_ranking`), `zone/ranking.rs`
  (going in, arrival, the close, the points), `server::ranking_clock` (the minute task), `GuildWrite::Ranking` / `RankingDue` and migration
  `0020_guild_ranking.sql`. Extra: anyone still in the closed battle map is called out at every minute (as the original's beat), not only
  at the first.
- **Deviations** (built; all approved):
  1. **Notices are sent exactly once at their minute** (the original's two beats can skip one), and **the close moves players out at
     the first minute after the close** (as the original's next 60 s beat at the latest).
  2. **`0x020A` to the ranking map asks first**: the answer is reason 5 (the client's question 8043), and only `0x0227` (its yes) goes
     in, after the same checks. The client re-asks every 15 s while the player stands in the portal. (Original: `0x020A` goes straight
     in.)
  3. **One entry per character per battle**, whatever its guild: the tag is kept per character (in memory), so changing guilds does
     not give a second entry.
  4. **The update is not lost to a short restart**: the server notes in the database when a battle closed and has not been ranked
     (`ranking_due`), and runs the update at start-up if that time has passed. (Battle points and entries stay in memory as the
     original, so a restart during the battle still loses them.)

## 11. Other episodes

The EP3 `ps_game.exe` (linked 2008-01-17) and the EP5.4 one (linked 2011-09-14, modified after linking) were compared with the EP4.5
build, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

The battle exists: `CMap::IsGuildRankMapOpenTime` (EP3 ps_game 0x4247c0, matched by call graph) and the console line
`cmd guildhouse cur=%d, max=%d, guildrank cur=%d, max=%d` are there (confirmed). It was not compared further.

### EP5.4

- The clock is the same: `IsGuildRankMapOpenTime` and `IsGuildRankMapCloseTime` (EP5.4 ps_game 0x42f770, 0x42f850) read the same two
  `(weekday, hour, hours)` entries with the same past-midnight handling (confirmed for the open test).
- The notices come at the same minutes (EP5.4 ps_game 0x42f996–0x42fa0c; confirmed). The type now goes through a global (EP5.4 ps_game
  0x582780) read by `CWorld::CheckGuildRankNotices` (0x41a780); the two-beat timing that can skip a notice is unchanged (inferred).
- `UpdateGuildPointRank` and `SendRankingZone` keep their constants (inferred); `GetGuildRankJoinCount` has no match (inlined or removed,
  not settled).
