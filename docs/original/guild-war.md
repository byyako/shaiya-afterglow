# Guild wars

This page covers guild wars (guild against guild) in the EP4.5 game server `ps_game.exe`: the request and the answer,
who takes part, the countdown and the fight, every way a player leaves a war and how a war ends, and what a war changes elsewhere (who
may hit whom, deaths, party help, Disguise, the desperate counterattack, duels). It was read in the disassembly with the PDB names. The
client side (the `0x25xx` handlers, the messages and the 40 area as the client draws it) is on the client's
[Guild wars](../client/guild-wars.md) page; message ids are the client's `sysmsg-uni.txt`.

The guild ranking battle and its zone (`0x0D19`, member state 106, `0x0D26`/`0x0D27`/`0x0D36`/`0x0D37`) are on
[Guild ranking battle](guild-ranking.md), item special 59 (a move to map 200) on [Using items](item-use.md), the game
log messages on [The game log](game-log.md).

## In short

- **Officers declare war.** A guild master or an officer of rank 2 to 4 asks an officer of another guild on the same map. Guilds of the
  same faction can fight each other too. The other side has 10 seconds to accept.
- **Who fights** is decided at that moment: every member of either guild who is online, on that map and within 40 units of the point
  between the two officers. Nobody can join later.
- **A 5-second countdown, then the fight.** Members of the two guilds in the war are enemies, even on maps 40 and 42; they cannot heal or
  buff anyone outside their own side of the war.
- **Dying in the war costs nothing**: no experience, items or gold lost, and no kill counted. The dead player is out of the war.
- **A guild loses** when its last fighter is out, or when its officer leaves the area or the map. There is no time limit, no tie, and no
  reward beyond the result message.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's here); `CGuild` offsets
are the code's (8 above the PDB's).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the disassembly (`objdump`, names from `docs/original/symbols/ps_game.tsv`). Client side:
`docs/client/guild-wars.md` (the `0x25xx` handlers, the messages and the 40 area as the client draws it). Message ids are the client's
`sysmsg-uni.txt`. `CUser` offsets are the code's (4 above the PDB's here); `CGuild` offsets are the code's (8 above the PDB's).

Covered: the request and the answer, who takes part, the countdown and the fight, every way a player leaves a war and how a war ends,
what a war changes elsewhere (who may hit whom, deaths, party help, Disguise, the desperate counterattack, duels).
**Not covered:** the guild ranking battle and its zone (`0x0D19`, member state 106, item special 59, `0x0D26`/`0x0D27`/`0x0D36`/`0x0D37`),
the game log messages.

## 1. The state

Confirmed.

Each guild (`CGuild`, `CGuild::PvPReset` 0x437eb0 clears the first four):

| field | offset | |
|---|---|---|
| status `m_nPvPStatus` | +0x4B4 | **0** none, **1** asked another guild, **2** asked by another guild, **3** countdown, **4** fighting |
| the other officer `m_dwPvPMaster` | +0x4B8 | the other guild's player who asked or was asked (an id, looked up in **the player's own zone** each time) |
| enemy guild `m_dwPvPGuild` | +0x4BC | set when the war is accepted |
| own officer `m_dwPvPGuildBoss` | +0x4C0 | this guild's player who asked or was asked |
| centre `m_vPvPPos` | +0x4C4 | x, y, z |
| map `m_nPvPMap` | +0x4D0 | the answerer's map |
| players in the war `m_nPvPUserCount` | +0x4D4 | counted when the war is accepted, one off for each that leaves |

Each player (all 0 / −1 when the character is loaded, `CUser::PvPReset` 0x47b1e0; nothing is saved):

| field | offset | |
|---|---|---|
| enemy guild `m_dwPvPGuildID` | +0x54E4 | non-zero: in the war (or the officer of a request) |
| centre `m_vPvPGPos` | +0x54E8 | the war's centre |
| deadline `m_dwPvPGTime` | +0x54F4 | the officers' timer; −1 none |
| joined `m_bPvPJoinUser` | +0x54F8 | took part (still set after leaving: gets the result) |
| ready `m_bPvPGuildReady` | +0x54FC | in the war: an enemy of the other guild's players in it |

**`PvPGModeEnd`** (0x47b230, one player): its debuffs are removed (`PvPClearDebuff`, as for duels: every applied skill with `byTypeEffect` 3,
`0x050E` each), its damage record cleared, enemy guild 0, deadline −1, **its attack target kind (+0x1528) cleared** (it stops attacking),
ready 0. "Joined" stays.

## 2. The packets

Confirmed, `CUser::PacketPVPGuild` 0x47da70.

A player **without a guild** (no `m_pGuild` or guild id 0) is ignored entirely. Client → server: `0x2501` (6) `u32 target id`;
`0x2502` (3) `u8` answer (0 refuse, anything else accept). Any other opcode of the family is ignored (the connection stays).

Server → client:

| opcode | size | fields | |
|---|---|---|---|
| `0x2501` | 10 | `u32 requester, u32 target` (`TP_PVPG_REQ`) | to both: the target's dialog, 5340 "<p> requested the battel to <t>." |
| `0x2502` | 7 | `u8 result, u32 id` (`TP_PVPG_ANS`) | **0** refused (5341), **1** accepted (silent), **2** not possible (5343 "can't request GvG to <p>."), **3** cancelled (5345 "…the opponents didn't respond."; silent when `id` is the receiver) |
| `0x2503` | 14 | `f32 x, f32 z, u32 enemy guild` | the war is on: the area's centre and the **other** guild's id; the client's 6-second countdown (5308) |
| `0x2504` | 2 | — | the fight begins: 5316 "Start GvG!!" |
| `0x2505` | 3 | `u8` | the result: **1** the receiver's guild won, **2** the enemy guild won (5317 "<g> guild won."), 3 a tie (5312, never sent) |
| `0x2506` | 7 | `u8 event, u32 id` | a player left the war: **0** "<p> disconnected." (never sent), **1** out of the area (5332 for itself, 5333 for the others), **2** gave up (never sent), **3** defeated (5337 / 5338) |

There is no give-up packet (`0x2406` is the duel's), no tie and no time limit: a war ends only when one side runs out of players or loses
its officer (section 6).

## 3. Request: `0x2501`

Confirmed, 0x47de54.

1. Ignored (nothing sent) unless the requester's **rank is 4 or lower** (`m_nGuildUserLevel` +0x174C: the master, rank 2 and ranks 3 and
   4), its guild's status is 0 and it is in no duel (`m_nPvPStatus` +0x54CC 0).
2. The target is looked up **in the requester's zone**. Not found, without a guild, of rank above 4, **in the requester's guild**, in a
   duel, or its guild's status not 0 → `0x2502 (2, target id from the packet)` to the requester.
3. Otherwise: the requester's guild status 1, the other officer = the target; the target's guild status 2, the other officer = the
   requester; **each guild's own officer is its player**; **both players' deadlines now + 10 s** and each one's enemy guild set (the
   other's guild); `0x2501 (requester, target)` to the requester, then to the target.

**No check of country, distance, life, mount, shop or the map**: guilds of the same country may fight (section 5 makes their players
enemies), and the request works anywhere both stand on one map.

## 4. Answer: `0x2502`

Confirmed, 0x47db0b.

Only from a player of rank 4 or lower whose guild's status is 2, with the other officer (the requester) found **in the answerer's zone**,
in a guild, rank 4 or lower; otherwise nothing. **It need not be the asked player**: any officer of that rank in the asked guild may answer
(its client has no dialog; the original does not check).

- **0 (refuse)**: both guilds' status, other officer and own officer 0; both players' enemy guild 0 (deadlines left as they are; nothing
  reads them without an enemy guild); `0x2502 (0, answerer id)` to the requester.
- **anything else (accept)**:
  1. both guilds' status 3; **the answerer's and the requester's deadlines now + 5 s**;
  2. the centre is the **midpoint of the two players' positions** (all three coordinates), stored in both guilds, and the map is the
     answerer's; each guild's enemy guild is the other;
  3. **who takes part** (`CGuild::PvPUserSet` 0x437ed0, the answerer's guild first): every **online member of the guild on the war map
     closer than 40 to the centre** (x and z) gets the enemy guild, the centre, joined 1, ready 1 and its damage record cleared, and is
     counted; **every other online member** gets enemy guild 0, joined 0 and ready 0 (an officer outside the area is out of the war too);
  4. `0x2502 (1, answerer id)` to the requester;
  5. **`0x2503 (centre x, centre z, enemy guild)`** to the answerer's guild's players in the war, then to the requester's
     (`CGuild::SendPvP` 0x4383c0: only to members whose enemy guild is set, and only while the guild's status is not 0).

Members who come into the area later, log in or join the guild after this never take part.

## 5. Countdown and fight

Confirmed, `CUser::PvPChkLeaveByTime` 0x47c210, the guild half; every second for every player.

Nothing happens unless the player has an enemy guild, its deadline has passed, **it is its guild's own officer**, and the other officer is
found in its zone. By its guild's status:
- **1 or 2** (no answer in 10 s): the other officer's guild locked; `PvPGModeEnd` for both officers; both guilds reset (`PvPReset`);
  `0x2502 (3, the asked officer's id)` to the other officer, then to this one.
- **3** (5 s over): both guilds' status 4, both officers' deadlines −1; **`0x2504`** to this guild's players in the war, then to the
  other's.
- 4 has no deadline.

**From the countdown on** (ready is set when the war is accepted, section 4 step 3):
- **Enemies** (`CUser::IsTeam` 0x44bdf0, `docs/original/pvp.md` section 1): two players are enemies when both are in guilds, each has
  the other's guild as its enemy guild and both are ready; this comes right after the duel rule and before the country rule, so
  players of one country fight each other, on maps 40 and 42 too. **Already during the countdown.**
- **Party help** (`CUser::IsTeamUsable` 0x44bf30, `CUser::SkillHealParty`): besides the duel rule, no party heal or buff passes between
  two players whose enemy guilds differ unless both are 0: players in a war help only those of their own side in the same war.
- **Disguise** (`CUser::IsCloneTargetingUser` 0x44be70): never copies a player whose guild is the caster's enemy guild while the caster's
  guild is that player's (both in guilds; ready not needed).
- **The desperate counterattack** (`wType` 23, `CUser::SkillAttackSelect`, `SkillAttackRange`, `SkillAttackRangeSelect`): `CUser::Death`'s
  flag is also set **whenever the caster has an enemy guild** (any stage, the officer of a request too; section 6.1).
- **Duels**: `0x2401` is ignored when the requester has an enemy guild, and refused (`0x2402 2`) when the target has one
  (`docs/original/duel.md` section 3.1); a guild war request needs both players out of duels (section 3).

## 6. Leaving a war, and its end

### 6.1 `CGuild::PvPUserLeave`

Confirmed, 0x438160.

`PvPUserLeave(guild, player, event)`: the guild's count goes down by one. Then:
- **The count is still above 0, and not (the player is the guild's own officer and the event is 1)**: `0x2506 (event, player id)` to the
  enemy guild's players in the war, then to its own (the player included: its enemy guild is still set); then on the player
  `PvPClearDebuff`, damage record cleared, enemy guild 0, attack target cleared, ready 0, deadline −1. The war goes on.
- **Otherwise the guild loses**: `0x2506 (event, player id)` as above; then **the enemy guild**: `0x2505 1` to every member who joined
  (`CGuild::SendPvPJoinUser` 0x438320: joined set, whether still in the war or not), `CGuild::PvPUserClear` 0x438070 (**every online
  member of the guild**, wherever it is: `PvPClearDebuff`, damage record cleared, enemy guild 0, deadline −1, attack target cleared,
  ready 0, joined 0), status and the rest 0; then **the player's guild** the same with `0x2505 2`.

So a guild loses when **its last player in the war leaves it**, or when **its own officer leaves with event 1** (out of the area, off the
map, out of the guild). An officer who dies is only one player less. Nothing else changes: no victories or defeats, no kill counts, no
guild points (`CGuild::GuildPvPWin` 0x42f020, +2 points and `0x0D36 2`, and `GuildPvPLoss` 0x42f080, +1 point, are never called in this
build).

### 6.2 Deaths

Confirmed, `CUser::Death` 0x4586a0, `CUser::PvPGetDeathType` 0x47b270, `PvPChkLoseDeath` 0x47b2c0, `PvPChkLeaveByDeath` 0x47b530.

- **Killed by an enemy in the war**: `PvPGetDeathType(victim, killer)` is **5** when it is not a duel's type 4, both are in guilds and
  each has the other's guild as its enemy guild (ready not checked). Then the victim's `m_nDropExp` is **−1** (nothing lost at rebirth)
  and **none of the player-kill rules apply** (no experience for the killer, no kill or death counts, no item or gold loss);
  `PvPChkLoseDeath(5)` calls `PvPUserLeave(victim's guild, victim, 3)`: **defeated**.
- **The desperate counterattack** with `Death`'s flag (section 5): `m_nDropExp` −1; a caster who killed itself, or was killed by any other
  player than its duel partner, with an enemy guild → `PvPChkLoseDeath(5)`: defeated (whatever its guild's status: on a request's officer
  it runs `PvPUserLeave` on a guild still asking, see deviation 3).
- **Any other death** (a monster, a guard, a player outside the war), after the death's own rules, `PvPChkLeaveByDeath`'s guild half on a
  player with an enemy guild, the other officer found in its zone: guild status **1 or 2** and the player is its guild's own officer →
  `PvPGModeEnd` for both officers, both guilds reset, `0x2502 (3, the asked officer's id)` to the other officer, then to the dead; status
  **3 or 4** → `PvPUserLeave(guild, player, 3)`: defeated. **Without the other officer in the zone nothing happens**: the dead player
  stays in the war.

### 6.3 Moving out of the area

Confirmed, `CUser::PvPChkLeaveByMove` 0x47bef0, after every `0x0501` step.

A player with an enemy guild **and no deadline** (so not an officer during a request or the countdown) more than **40** from the
centre (x and z) → when its guild's status is 3 or 4, `PvPUserLeave(guild, player, 1)`: out of the area. Members are checked from the
countdown on, the two officers only from the fight on (their deadline is cleared when it starts).

### 6.4 Leaving the map

Confirmed, `CUser::PvPChkLeaveByZoneLeave` 0x47b920, from `CUser::LeaveZone`: map change, logout, lost connection.

On a player with an enemy guild, the other officer found in its zone: status **1 or 2** and the player is its guild's own officer → as
the death above (`0x2502 (3, the asked officer's id)` to both); status **3 or 4** → `PvPUserLeave(guild, player, 1)`.

### 6.5 Leaving the guild

Confirmed, `CUser::PvPChkLeaveByGuildLeave` 0x47bd00, from `CGuild::JoinLeave`, `JoinExpel`, `Fire`, `CUser::LeaveGuild`.

As leaving the map, **after the player is already out of the guild's member list** (`JoinLeave` removes it first): a player leaving or
kicked in the war is not among the guild's players any more, so it gets neither its own `0x2506` nor a result, and when its leaving ends
the war, `PvPUserClear` does not reach it (its enemy guild and ready stay set; without a guild id it is nobody's enemy).

## 7. The new server

Built as above, with these deviations (all approved):

1. **The answer is refused when the two officers stand 80 or more apart** (x and z): the requester gets `0x2502 (2, answerer id)` and the
   answerer `0x2502 (3, its own id)` (its dialog closes silently). In the original an officer 40 or more from the midpoint is out of its
   own war, and its timer does nothing without an enemy guild; with both out the countdown never ends and both guilds stay at war until
   their players leave.
2. **Only the asked player answers** (the original takes the answer from any officer of rank 4 or lower of the asked guild).
3. **Before the countdown a death only cancels the request** (`0x2502 3` to both officers, as other deaths do) and the death's usual rules
   apply. In the original two officers who are each other's enemy guild already make a kill between them type 5, and the desperate
   counterattack runs `PvPUserLeave` on a guild still asking: stray `0x2506` packets and both guilds reset with the other officer still
   holding its enemy guild.
4. **The end of a war clears only the players who took part** (debuffs, damage record, attack); the original clears every online member of
   both guilds, wherever they are.
5. **Leaving the world** (logout, lost connection) sends event **0** "<p> disconnected." instead of 1; it still loses the war for the guild
   when the player is its own officer, as leaving the map does.
6. **A player leaving the guild or kicked is handled while still a member**: it gets its `0x2506 1` (and the result when its leaving ends
   the war) and is cleared like any other player leaving the war.
7. **A death, leaving the map or leaving the guild counts whether or not the other guild's officer is on the map.** The original looks
   that officer up first and does nothing without it: once it has been defeated and has left the map, the other side's players can no
   longer leave the war by dying to a monster or changing maps (only by stepping out of the area).

How it is built: the wars are kept by guild (status, the two officers, enemy guild, centre, map, deadline, the players in the war and
those who joined), taken after the maps; each player in a war carries its enemy guild and the centre. The timers are the war's, not the
officers': one per pair of guilds, run every second. Game logs are not sent. The enemy guild's name in `0x2505 2` comes from the client's
guild list, which holds only guilds of its own country (`docs/original/guild.md` section 9): after losing to a guild of the other
country the client prints no "<g> guild won." line, as with the original.

## 8. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by the opcode switch tables or constants, as those pages list).

### EP3

`PacketPVPGuild` (EP3 ps_game 0x462d50, matched by dispatch) handles the same `0x2501` and `0x2502` (confirmed, dispatch tables). The
case bodies were not compared.

### EP5.4

No differences found. `PacketPVPGuild` (EP5.4 ps_game 0x48c950, matched by constants) handles `0x2501` and `0x2502` with the same calls
(`CGuild::PvPUserSet`, `CGuild::SendPvP`, the 5000 ms countdown) (confirmed). `0x2501` reads the same three fields of both sides at moved
offsets, and the extra calls are lock releases on more exits (inferred: same checks). `PvPReset`, `PvPUserClear`, `PvPUserLeave`,
`SendPvPJoinUser` and the `PvPChkLeaveBy*` checks have matches with the same constants (inferred). The united war server is
[`gamewar.md`](gamewar.md).
