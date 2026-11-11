# Duels

This page covers duels in the EP4.5 game server `ps_game.exe`: the request and the answer, the stakes window, the
countdown and the fight, every way a duel ends (win, giving up, moving out of the area, being hit by someone else, dying otherwise,
leaving the map, timeouts), the stakes going to the winner, victories and defeats, and what a running duel changes elsewhere (who may hit
whom, party heals and buffs, Disguise, the desperate counterattack). It was read in the Ghidra decompiler and the disassembly with the PDB
names.

The client side is on the client's [Duels](../client/duels.md) page, the `0x24xx` rows of [Packet opportunities](../client/packet-opportunities.md),
[Social packets](../client/client-social-packets.md) (the trade window, which the client reuses for the stakes) and
[GM opcodes and commands](../client/gm-and-commands.md) (`/defeat`). Message texts are the client's `sysmsg-uni.txt` (ids 5305 to 5339,
quoted from a sample copy published with the Parsec project).

Not covered here: guild wars (`m_dwPvPGuildID` `+0x54e4`, `CUser::PacketPVPGuild` 0x47da70, `0x25xx`, `0x0D19`; every duel function also
has a guild-war half, left out here; see [Guild wars](guild-war.md)); the game log messages (`0x0E01` to `0x0E03` to the game
log server; see [The game log](game-log.md)).

## In short

- **Asking.** A player asks another on the same map; the other has 10 seconds to accept. Players of different factions can only duel on
  maps 39 and 40. Neither may have a debuff on them.
- **Stakes.** Both sides can put up to eight items and some money at stake, in a window like the trade window. When both are ready, the
  stakes leave their bags.
- **The fight.** A 5-second countdown, then the two fight inside a circle of 40 units around the point between them. Party members
  cannot heal or buff a duelling player.
- **Winning** means killing the other. The winner gets both stakes (dropped on the ground for the winner), a victory is counted, and the
  loser loses no experience.
- **Every other end cancels the duel and gives the stakes back**: giving up, stepping out of the circle, being hit by anyone else,
  dying some other way, or leaving the map. Giving up counts no defeat on the server, though the client counts one.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/packet-opportunities.md` (the `0x24xx` rows), `docs/client/client-social-packets.md`
(the trade window, which the client reuses for the stakes) and `docs/client/gm-and-commands.md` (`/defeat`). Message texts are the
client's `sysmsg-uni.txt` (ids 5305 to 5339, quoted from Parsec's sample copy). `CUser` offsets are the code's.

Covered: the request and the answer, the stakes window, the countdown and the fight, every way a duel ends (win, giving up, moving out
of the area, being hit by someone else, dying otherwise, leaving the map, timeouts), the stakes going to the winner, victories and
defeats, and what a running duel changes elsewhere (who may hit whom, party heals and buffs, Disguise, the desperate counterattack).
**Not covered:** guild wars (`m_dwPvPGuildID` `+0x54e4`, `CUser::PacketPVPGuild` 0x47da70, `0x25xx`, `0x0D19`; every duel function
also has a guild-war half, left out here); the game log messages (`0x0E01` to `0x0E03` to the game log server).

## 1. The state

Confirmed, `CUser::PvPReset` 0x47b1e0, `CUser::PvPModeEnd` 0x47b200.

| field | offset | |
|---|---|---|
| status `m_nPvPStatus` | +0x54CC | **0** none, **1** asked someone, **2** asked by someone, **5** stakes window open, **3** countdown, **4** fighting |
| deadline | +0x54D0 | `GetTickCount()` value; −1 none |
| partner | +0x54D4 | the other player's id (not a pointer: looked up each time with `CZone::FindUser` in **the player's own zone**) |
| centre | +0x54D8 | the duel area's centre (x, y, z), set when the countdown starts |
| stakes window | +0x1590 | a `CExchangePvP`: a trade window (`CExchange`, `docs/original/trade.md` section 1: state +0x1590, money +0x1598, 8 × `{bag, slot, count}` +0x159C, ready +0x15B4), plus what was taken out for the fight: money +0x15B8, 8 item objects +0x15BC (`CExchangePvP::ResetPvP` 0x47c960 clears those) |

Status, deadline and partner start at 0 / −1 when the character is loaded (`PvPReset` from `CUser::PacketUserDBChar`); nothing is saved.

**`PvPModeEnd`** (one player): its debuffs are removed (`CUser::PvPClearDebuff` 0x47c610: every applied skill whose `byTypeEffect` is 3,
whoever cast it, each with the database removal and `CUser::CancleApplySkillDebuff`, i.e. `0x050E`), its **damage record is cleared**
(`CDamage::Reset`), status 0, deadline −1, and **its attack target kind (+0x1528) is cleared**: it stops attacking. The partner field
and the stakes window are left as they are.

## 2. The packets

Confirmed, `CUser::PacketPVP` 0x47d0f0.

Client → server: `0x2401` (6) `u32 target id` request; `0x2402` (3) `u8` answer (**1 accept, 0 refuse**: the other way round from
trade); `0x2406` (2) give up (`/defeat`, 5335; the client sends it only while fighting); `0x2409` (3) `u8` stakes ready (0 ready, 1 not
ready, 2 close); `0x240A` (6) `u8 bag, slot, count, stake slot` add an item; `0x240B` (3) `u8 stake slot` take it back; `0x240C` (6)
`u32 money`. **`0x2403` to `0x2405`, `0x2407`, `0x2408` from the client close the connection** (`SConnection::Close(9)`), as does any
opcode of the family above `0x240C`.

Server → client:

| opcode | size | fields | |
|---|---|---|---|
| `0x2401` | 10 | `u32 requester, u32 target` (`TP_PVP_REQ`) | to both: "<p> requested battle to <t>." (5325) |
| `0x2402` | 7 | `u8 result, u32 id` (`TP_PVP_ANS`) | 0 refused (5326 "<p> rejected the battle."), 1 accepted, 2 not possible (5327 "can't request the battle to <p>."), 3 cancelled (5328 "…the opponent didn't respond."), 4 a debuff (5334 "impossible to battle as Debuff skill is on <p>.") |
| `0x2403` | 10 | `f32 x, f32 z` (`TP_PVP_READY`) | the area's centre; the countdown begins (5308, 5309) |
| `0x2404` | 2 | — (`TP_PVP_START`) | the fight begins (5311 "start battle!!") |
| `0x2405` | 3 | `u8` (`TP_PVP_RESULT`) | **1 won, 2 lost** |
| `0x2406` | 7 | `u8 reason, u32 id` (`TP_PVP_CANCLE`) | the duel ended without a winner: **0** the partner left the map, **1** out of the area, **2** gave up, **3** a death or a hit from someone else; `id` as in section 6 |
| `0x2407` | 7 | `u32 partner, u8 0` | the stakes window opens |
| `0x2408` | 3 | `u8` | the stakes window closes: **0** done (the countdown follows), **2** by this player, **3** by the partner |
| `0x2409` | 4 | `u8 who, u8 not_ready` | as `0x0A05`; who 1 the receiver, 2 its partner, 3 both |
| `0x240A` | 6 | `u8 bag, slot, count, stake slot` | the adder's request echoed |
| `0x240B` | 4 | `u8 who, u8 stake slot` | as `0x0A07` |
| `0x240C` | 7 | `u8 who, u32 money` | as `0x0A08` |
| `0x240D` | 35 | `u8 stake slot, type, type id, count, u16 quality, u8 gems[6], char craft_name[21]` (`TP_EXC_ADD_DEST`) | to the partner, as `0x0A09` |

The client side (`docs/client/duels.md`) reads the reasons the same way: 0 "…as <p> disconnected." (5313), 1 "…you left
the allowed battle area." (5314) for the mover / "…as <p> is out of allowed battle area." (5336) for the other, 2 "<p> admitted the
failure." (5315) with "<p> won at the battle." (5310), 3 "…as you've been attacked before it finished." (5329). `0x2402 3` naming the
receiver itself closes its dialog silently. **The client counts victories and defeats itself** on `0x2405` 1 / 2 and on `0x2406 2` (the
giver a defeat, the other a victory); the server counts only `0x2405`'s (section 6.1), so after a `/defeat` the client's numbers are one
off until the next login, as with the original. The client warns at 35 from the centre; the server ends the duel past 40.

## 3. Request and answer

### 3.1 `0x2401`

Confirmed, 0x47d129.

1. Ignored (nothing sent) unless the requester's status is 0, it is in no guild war and it is **on foot** (vehicle status +0x1460 0).
2. The target is looked up **in the requester's zone**. Not found, its status not 0, in a guild war or not on foot, **or of the other
   country unless the requester's map is 39 or 40** (`m_wMap` +0x164, `0x27`/`0x28`) → `0x2402 (2, target id from the packet)` to the
   requester.
3. Either of the two has a debuff (`CUser::PvPHaveDebuff` 0x47c5b0: an applied skill with `byTypeEffect` 3) → `0x2402 (4, target id)`
   to the requester.
4. Otherwise: requester status 1, target status 2, each the other's partner, **both deadlines now + 10 s**, and `0x2401 (requester,
   target)` to the requester, then to the target.

**No distance check, no check that either is alive, no check of trades, parties or shops.** Requesting oneself links the player to
itself (status 2 overwrites 1).

### 3.2 `0x2402`

Confirmed, 0x47d29a.

Only in status 2 with the partner found in the zone; otherwise nothing.
- **0 (refuse)**: both cleared (status, deadline, partner 0); `0x2402 (0, answerer id)` to the requester only.
- **anything else (accept)**: both **status 5**, deadlines −1; `0x2402 (1, answerer id)` to the requester; both stakes windows reset
  (`CExchange::Reset`, `ResetPvP`) and set to state 3 (open); **`0x2407 (requester id, 0)` to the answerer, `0x2407 (answerer id, 0)`
  to the requester**.

## 4. The stakes window

Confirmed, 0x47d4f6 to 0x47da0e, `CUser::ExchangeCancleReadyPvP` 0x47d0a0, `CUser::ExchangePvP` 0x47caf0.

The trade window's rules (`docs/original/trade.md` sections 2.3 to 2.6) with the duel's packets. Each request needs status 5, the
window in state 3 and the partner found in the zone; otherwise nothing.

- **`0x240A` add**: count ≠ 0; stake slot < 8 and empty; bag 1 to the bag count, slot < 24; an item there; its real type not 27 (quest
  items, types 27 to 29); its count ≥ the count staked; that bag and slot not already staked. Then ready cancelled (below), the slot
  stored, `0x240A` (the request) to the adder, `0x240D` to the partner with **the count staked**. **No bound-item (`reqog`) check**, as
  in trade.
- **`0x240B` take back**: stake slot < 8 and filled: ready cancelled, emptied, `0x240B (1, slot)` to the player, `(2, slot)` to the
  partner.
- **`0x240C` money**: at most the player's money: ready cancelled, stored (replacing the previous amount), `0x240C (1, money)` /
  `(2, money)`.
- **`0x2409` 1 (not ready)**: ready cancelled. **Ready cancelled** (`ExchangeCancleReadyPvP`): if either side is ready, both flags are
  cleared and both get `0x2409 (3, 1)`.
- **`0x2409` 2 (close)**: both windows reset; `0x2408 2` to the player, `0x2408 3` to the partner; `PvPModeEnd` for both: no duel.
- **`0x2409` 0 (ready)**: the player's flag set; `0x2409 (1, 0)` to it, `(2, 0)` to the partner. **When the partner is ready too**: both
  windows state 4; **`ExchangePvP` for the player, then for the partner** (below); both windows reset; **both status 3 (countdown)**;
  the **centre is the midpoint of the two players' positions** (`(a + b) / 2`, all three coordinates), stored on both; **both deadlines
  now + 5 s**; `0x2403 (centre x, centre z)` to the player, then to the partner.

**`ExchangePvP`** takes one side's stakes **out of its bags** and keeps them on the player (+0x15B8, +0x15BC) for the end of the fight;
every packet goes to that player:
1. Money: the amount staked, cut to the money the player has; money − amount (`SendDBMoney`), **`0x0213`** (`u32 money`).
2. Each stake slot in order with an item still there: **the whole item** when the count staked is all of it (or the item does not stack
   and its count is 1), its bag slot emptied; otherwise **a new item** with the count staked (a new unique id, made now, make type 'T',
   quality 0, no gems, craft name "0000…" when the definition allows crafting; the original's count goes down). **`0x0206`** (7 bytes,
   `TP_ITEM_DROP`: bag, slot, then type, type id and count left, all 0 when the whole item went).
3. Last, **`0x2408 0`**.

## 5. Countdown and fight (verified, `CUser::PvPChkLeaveByTime` 0x47c210, called every second for every player from
`CUser::UpdateTimeoverSec` and `CZone::UpdateApplySkill`)

Nothing happens unless the deadline has passed and the partner is in the zone.
- **Status 1 or 2** (no answer in 10 s): `PvPModeEnd` for both; `0x2402 (3, the asked player's id)` to the partner, then to the player.
- **Status 3** (5 s countdown over): **both status 4**, deadlines −1, **both damage records cleared**, **`0x2404`** to the partner, then
  to the player.
- Status 4 and 5 have no deadline.

**While fighting (status 4)**:
- **The two are enemies** (`CUser::IsTeam` 0x44bdf0: status 4 with the other as partner comes before every other rule): same country,
  war maps 40 and 42 included. Status 1, 2, 3 and 5 change nothing.
- **Party help**: `CUser::IsTeamUsable` 0x44bf30 is false when **either** side is fighting a duel (unless it is the caster itself): no
  party heals or buffs from or to a duelling player (`docs/original/party.md` section 10).
- **Disguise** (`wType` 90 with ability 100, `CUser::IsCloneTargetingUser` 0x44be70, from `ChkTargetUser` and `CUser::FindNearUser`):
  never copies the caster's duel partner.

## 6. How a duel ends

### 6.1 A win: one kills the other

Confirmed, `CUser::Death` 0x4586a0, `CUser::PvPGetDeathType` 0x47b270, `CUser::PvPChkLoseDeath` 0x47b2c0.

A death whose killer of record (`docs/original/pvp.md` section 5) is a player in the zone **who is the victim's partner while the victim
is fighting** (`PvPGetDeathType` 4; it checks the victim's status and partner, not the killer's) takes **type 4**: the victim's
`m_nDropExp` becomes **−1** (no experience lost at rebirth, and no ultimate-mode deletion) and none of part 1's player-kill rules apply
(no experience for the killer, no kill or death counts, no item or gold loss). Instead, `PvPChkLoseDeath(4, winner)` on the loser:
1. **The winner's own stake** goes on the ground for the winner, then **the loser's stake** goes on the ground for the winner
   (`CUser::ItemDropPvP` 0x47c990, below).
2. `PvPModeEnd` for the loser, then the winner.
3. **The loser's defeats** (+0x158) and **the winner's victories** (+0x154) go up by one, each only while below 65535, and are saved
   (`0x060C` index 3 and 2 to the database agent). No packet for them.
4. **`0x2405 1`** to the winner, **`0x2405 2`** to the loser.

Then the death goes on as any other (`0x0504` naming the winner, rebirth: section 8 of `docs/original/mob-fight.md`).

**The desperate counterattack** (`wType` 23, `docs/original/pvp-2.md` section 2) passes `Death`'s flag when the caster fights a duel
**with the player it aimed the skill at** (both partners of each other; the same test in `SkillAttackSelect`, `SkillAttackRange` and
`SkillAttackRangeSelect`) or is in a guild war. With the flag, when the caster dies of its own skill: `m_nDropExp` −1; then the killer of record decides: **the caster
itself** (only its own damage on record) and fighting → **it loses the duel to its partner** (`PvPChkLoseDeath(4, partner)`, the partner
found in the zone); **the partner** (both partners of each other) → the same; **another player** → nothing more (no player-kill rules,
no experience lost); **a monster** → a monster death as before. Without the flag a self-kill is part 1's "nothing more".

### 6.2 Who gets what: `ItemDropPvP`

Confirmed, 0x47c990.

`ItemDropPvP(holder, recipient)` puts what was taken from `holder` (section 4) **on the ground at the holder's position ± 0.5**
(`CUser::GetDropPosRand`, x and z) **in the holder's zone**, as `CZone::EnterItem` kind 1 (`0x0401` kind 1 to the view, removed after
60 s): owner **the recipient**, no party, drop type 3, drop id the holder. Money comes as a gold item (type 26, type id 0, count 1, the
amount). Items keep their object (unique id, gems, craft name). Then the holder's taken stakes are cleared.

Every other end returns each side's stake **to itself**: `ItemDropPvP(partner, partner)` and `ItemDropPvP(player, player)`, i.e. on the
ground at its own feet, its own.

### 6.3 Giving up: `0x2406`

Confirmed, 0x47d3c6.

Only while fighting. The partner (if in the zone) gets its stake back, `PvPModeEnd`, `0x2406 (2, giver's id)`; then the giver the same.
**No winner, no victory or defeat**: it is a cancel.

### 6.4 Moving out of the area

Confirmed, `CUser::PvPChkLeaveByMove` 0x47bef0, after every `0x0501` step, `PacketPC` 0x46a648.

- **Countdown or fight**: when the mover is **more than 40 from the centre** (x and z): `0x2406 (1, mover's id)`; the partner (if in the
  zone) gets its stake back, `PvPModeEnd` (written out in place), the packet; then the mover the same.
- **Stakes window**: when the mover is more than 40 **from the centre field**: `PvPModeEnd` and `0x2408 3` for the partner, `PvPModeEnd`
  and `0x2408 2` for the mover. **The centre is only set when a countdown starts** (section 4), so in the window it is the last duel's
  centre, or 0, 0 for a player who has not duelled since logging in: in practice **the first step in the stakes window closes it**
  unless the player stands within 40 of that old point. An original bug (deviation 2).
- Status 1 and 2: nothing.

### 6.5 Hit by someone else

Confirmed, `CUser::PvPChkLeaveByAttack` 0x47c750.

Right after a hit breaks the target's cast (`CUser::StatusCastingStop`, the same places): **any monster hit** (`CMob::Attack`,
`CMob::AttackUser`) or guard hit (`CNpc::Attack`) on a player in **any duel status**, and **a player's hit** (`CUser::AttackUser`,
normal blows and skills) on a player in any duel status **whose partner is not the attacker**. By the target's status:
- **1 or 2**: `PvPModeEnd` for both; `0x2402 (3, the asked player's id)` to the partner, then to the target.
- **3 or 4**: both stakes back; `PvPModeEnd` for both; `0x2406 (3, the target's id)` to the partner, then to the target.
- **5**: `PvPModeEnd` for both; `0x2408 2` to the target, `0x2408 3` to the partner.

### 6.6 Dying otherwise

Confirmed, `CUser::PvPChkLeaveByDeath` 0x47b530, at the end of `CUser::Death` and `CUser::DeathByNPC`.

After every death, on the dead player, with the partner in the zone (after a win the status is already 0: nothing):
- **1 or 2**: `PvPModeEnd` for both; `0x2402 (3, the asked player's id)` to the partner, then to the dead.
- **3 or 4**: both stakes back; `PvPModeEnd` for both; `0x2406 (3, the dead's id)` to the partner, then to the dead.
- **5**: `PvPModeEnd` for both; `0x2408 2` to the dead, `0x2408 3` to the partner. (Unreachable: `Death` closes the window first, below.)

### 6.7 Leaving the map

Confirmed, `CUser::PvPChkLeaveByZoneLeave` 0x47b920, from `CUser::LeaveZone`: map change, logout, lost connection.

On the leaver, the partner in the zone:
- **1 or 2**: `PvPModeEnd` for both; `0x2402 (3, the asked player's id)` to the partner, then to the leaver.
- **3 or 4**: both stakes back (**the leaver's on the ground of the map it is leaving**); `PvPModeEnd` for both; **`0x2406 (0, the
  leaver's id)` to the partner only**.
- **5**: `PvPModeEnd` for both; `0x2408 2` to the leaver, `0x2408 3` to the partner.

### 6.8 Anything that ends a trade

Confirmed, `CUser::CancleActionExc` 0x44b3d0, `CUser::ExchangeItemRemove` 0x46f310.

`CancleActionExc` (death, item moves, pick-ups, drops, shops, item use, gems, quests, leaving the zone: `docs/original/trade.md` section 4)
also closes an open stakes window (status 5): `PvPModeEnd` for both (partner first), `0x2408 2` to the player, `0x2408 3` to the partner.
`ExchangeItemRemove` (an item leaving a staked bag slot through the bank or market) empties that stake slot with ready cancelled and
`0x240B (1, slot)` / `(2, slot)`; with `CancleActionExc` on every item action this server has, it is not needed (as for trades).

## 7. The new server

Built as above, with these deviations (all approved):

1. **Requesting oneself is refused** with `0x2402 (2, id)` (the original links the player to itself; a self duel would take its money
   twice when the window completes, `ExchangePvP` running twice on one player).
2. **The stakes window's area**: the centre is set to the two players' midpoint when the request is accepted (and again when the
   countdown starts, as the original), so a step more than 40 from where the two stood closes the window, not the first step anywhere.
3. **Leaving the map during the countdown or the fight returns the leaver's own stake to its bags and money** instead of the ground of
   the map it is leaving (where nobody can pick it up and it would be gone in 60 s; a lost connection would cost the stake). What does not
   fit in the bags goes on the ground there, as the original. The partner's stake goes on the ground at its feet, its own, as the original.
4. **Bound items (`reqog` not 0) and item type 99 are refused as stakes**, besides real type 27 (trade deviation 6).
5. A staked item that is gone, another item or smaller than staked when the stakes come out, or gold the player no longer has,
   **cancels the duel for both** and nothing is taken (trade deviation 7; audit R3: it used to be skipped, and the gold cut to what the
   player had, so the other side's stakes were taken for free). The window is closed on both screens (`0x2408` 2 and 3); stakes the
   other side had already taken go on the ground at its feet, its own, as for any end of the countdown. Each side's stakes come
   out of its bags in **one database transaction** (its money and every item). **Part of a stack keeps the giver's quality, gems and
   craft name** with make type 'T' (trade deviation 3; the original's new item has none).
6. The step to the countdown is taken under one lock, once (trade's "both pressing ready at once").
7. **A trade, a personal shop and a duel do not mix** (audit R3, trade deviation 9): a request or an answer is refused (`0x2402 2`)
   while either side trades or has a shop, and a trade request is busy while a stakes window is open.
8. **Stakes come back in one save** (audit R15). When a stake is returned to its owner (leaving the map during the countdown or the
   fight, deviation 3), the gold and every item are planned on a copy of the bags and written in one transaction, then applied; what does
   not fit goes on the ground as before. If that save fails, all of it (the gold included) goes on the ground at the owner's feet, its
   own, before the connection ends; before, the items not yet saved were lost, since they live only in memory between the taking and the
   return (audit N2: a crash still loses them).

How it is built: each player's side lives with it in the zone (the partner looked up on the same map). When both are ready, each
connection takes its own stakes out (the player who pressed last at once, its partner when woken); the countdown starts once both
sides' are out. A duel that ends in that moment (a hit, a death, a step away, leaving) gives back what is already out on the ground at its
owner's feet, as the original's ends do; a side whose client still shows the window is told `0x2408` as for the window, the other
`0x2406` as for the countdown. Anything that ends a trade does not end it then (in the original the window is already done).

Game logs are not sent (none are built). Guild wars, with their halves of `IsTeam`, `IsTeamUsable` and the deaths, are in
`docs/original/guild-war.md`.

## 8. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by the opcode switch tables or constants, as those pages list).

### EP3

`PacketPVP` (EP3 ps_game 0x4623e0, matched by dispatch) handles the same `0x24xx` opcodes (confirmed, dispatch tables). The case bodies
and the stake checks were not compared.

### EP5.4

`PacketPVP` (EP5.4 ps_game 0x48bde0, matched by constants) handles the same seven opcodes with the same calls (confirmed). New checks
(confirmed):

| where | EP5.4 |
|---|---|
| request `0x2401` | refused ("not possible") when either side trades or has a personal shop (EP5.4 ps_game 0x48bea5 to 0x48bed3); **a request to oneself closes the connection** (EP5.4 ps_game 0x48bed9) |
| answer, stakes, start | the same trade, shop and self checks against the partner (for example EP5.4 ps_game 0x48c5e2 to 0x48c618 in `0x240A`) |
| a stake from a bag (`0x240A`) | the slot must still hold an item (EP5.4 ps_game 0x48c577) |

The trade side refuses too while either player duels (`0x0A05` to `0x0A08`). So EP5.4 closes the gaps of section 3 (no check of
trades or shops, the request to oneself) much as the new server does (section 7), except that it drops the connection on a request to
oneself instead of answering. **The stake count guard is fixed**: `ExchangePvP` (EP5.4 ps_game 0x48b950) takes
the partial-stack path only when the offered count is below the count held (`jb`, EP5.4 ps_game 0x48bacd; EP4.5 `jne` 0x47ccbe), so a
stack that shrank is handed over whole (confirmed); the trade keeps the old test (EP5.4 ps_game 0x47e81a). Bound items can still be
staked: no stake check reads the bound flag (confirmed: the checks from EP5.4 ps_game 0x48c548 to the store at 0x48c634 read the real
type, the count and the slot only).
