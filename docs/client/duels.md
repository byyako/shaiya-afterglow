# Duels (`24xx`): request, stake window, countdown, fight, result

This page covers the one-on-one "battle" of the client `game.exe` (US build 2010-04-26, patch 108), with its stake (an exchange window
before the fight). It was read from `game.exe` (handlers on the network object's vtable `+0x4c0`…`+0x4d4` and `+0x28c`…`+0x2a4`, state
machine `0x420660` and `0x421210`) and from the original EP4.5 server `ps_game.exe` (`CUser::PacketPVP` `0x47d0f0`,
`CUser::PvPChkLeaveByTime` `0x47c210`); struct names come from the server's debug symbols (`ps_game.pdb`). Who may attack whom is on
[Player kills](../original/pvp.md#1-enemies) (`IsTeam`: a duel makes the two players enemies when `m_nPvPStatus == 4`). Status words:
**confirmed**, **confirmed (disasm)**, *inferred*, **live**. Sizes include the opcode. Messages are sysmsg IDs.

## In short

- **A duel starts with a request** the other player has 10 seconds to accept.
- **Then both players see a stake window**, like a trade window: each can put items and gold at stake and press Ready.
- **When both are ready**, the fight area is set at the midpoint between them, a countdown runs, and the fight starts.
- **The fight ends** with a win, a loss or a draw, or is cancelled when a player disconnects, leaves the area (35 units around the
  centre), gives up with `/defeat`, or is attacked by someone else. The winner takes the stakes.

## Summary

- Flow (original, confirmed): `0x2401` request → both get `0x2401 (requester, target)`, 10 s → target answers `0x2402` → requester
  gets `0x2402 (1, target)` and **both get `0x2407 (partner, 0)`: the stake window** → `0x240A`/`0x240B`/`0x240C` stakes, `0x2409`
  ready → when both are ready the stakes are locked and both get **`0x2403 (f32 x, f32 z)`**: the fight area's centre (midpoint of
  the two players), 5 s → **`0x2404`**: fight → end with `0x2405` (result) or `0x2406` (cancel / give up).
- Client countdown: 6 s of "the battle starts after <v> seconds." (5308) after `0x2403`; `0x2404` prints "start battle!!" (5311)
  and plays `ui_challenge01.wav`. The client only allows `/defeat` (`0x2406`) after `0x2404` (duel state `0x7045b4 == 2`).
- **Area**: radius **35.0** around the `0x2403` centre (`0x6a67b8`); leaving it prints 5309 once ("…cancelled if you get out of
  allowed battle area."). The client cancels nothing itself: the server must send `0x2406 1`.
- **Catalogue correction**: `0x2406`'s reason codes are **0 disconnect, 1 out of area, 2 gave up, 3 attacked**, not 1–4
  (the [packet opportunities](packet-opportunities.md) table first read the handler's `+1`).
- Win/loss counters: the client adds 1 to `0x21e4f70` on a win and to `0x21e4f74` on a loss itself (*inferred*: the victory and
  defeat counts of the character window; [Player kills](../original/pvp.md#7-the-kill-and-death-counters) calls them the third and fourth counters). Send the
  counters only if they must be corrected.

## Request and answer

| op | dir | size | fields | client | original |
|---|---|---|---|---|---|
| `0x2401` | C→S | 6 | `u32 targetId` (`TPC_PVP_REQ`) | sender `0x589370` (player menu, *inferred*) | needs: neither player in a duel or guild war or `+0x1460` (*inferred*: exchange), same faction unless map 39/40; else `0x2402 (2, target)`. A debuff on either → `0x2402 (4, target)`. OK: both get `0x2401` 10, states 1/2, 10 s |
| `0x2401` | S→C | 10 | `u32 requesterId, u32 targetId` | only with no dialog open (`0x850a5c == 0`), and ignored if either player has `+0x250` set (*inferred*: dead or mounted). I am the requester → dialog 7 (waiting). I am the target → if busy (`0x701a00` or player `+0x2f0`) answers `0x2402 0` itself; else dialog 6 "<p> requested battle to <t>." (5325), `ui_party001.wav` | — |
| `0x2402` | C→S | 3 | `u8 answer` (**0 refuse, else accept**) (`TPC_PVP_ANS`) | dialog 6 | refuse: requester gets `0x2402 (0, target)`; accept: requester gets `0x2402 (1, target)`, both `0x2407` |
| `0x2402` | S→C | 7 | `u8 result, u32 id` | only while dialog 0/6/7: 0 → 5326 "<p> rejected the battle."; 2 → 5327 "can't request the battle to <p>."; 3 → if `id` is me: closes silently, else 5328 "…the opponent didn't respond."; 4 → 5334 "impossible to battle as Debuff skill is on <p>." (`<p>` = my own name when the debuffed one is not found); **1 and others: dialog closed silently** (1 = accepted; the stake window follows). Error results play `ui_error001.wav`. `id` must be a player in view or nothing is shown | 0, 1, 2, 4 as above; 3 on the 10 s timeout (`PvPChkLeaveByTime`) |

## Stake window (`0x2407`–`0x240D`)

Works like trade ([Social packets](client-social-packets.md#trade-0x0axx), `0x0Axx`) with its own store (`0x854c50`: my offer slots, 34 bytes each, partner's at
`0x854d60`; my offer's bag/slot/count at `0x854e70`). confirmed (`0x558db0`…`0x559050`)

| op | dir | size | fields | client |
|---|---|---|---|---|
| `0x2407` | S→C | 7 | `u32 partnerId, u8 result` | 0 → opens the stake window (dialog 0x0F) with that partner; 2 → 322 "Transaction in process with another user" + error sound |
| `0x240A` | C→S | 6 | `u8 bag, u8 slot, u8 count, u8 stakeSlot` | add my item |
| `0x240A` | S→C | 6 | `u8 bag, u8 slot, u8 count, u8 stakeSlot` (echo) | my offer slot = that bag item (copied from my bags) with `count` |
| `0x240D` | S→C | 35 | `u8 stakeSlot, u8 type, u8 typeId, u8 count, u16 quality, u8 gem[6], char craftName[21]` (`TP_EXC_ADD_DEST`) | partner's offer slot |
| `0x240B` | C→S | 3 | `u8 stakeSlot` | remove my item |
| `0x240B` | S→C | 4 | `u8 who (1 me, 2 partner), u8 stakeSlot` | clears that slot |
| `0x240C` | C→S | 6 | `u32 money` | my money stake |
| `0x240C` | S→C | 7 | `u8 who (1 me, 2 partner), u32 money` | sets that side's money |
| `0x2409` | C→S | 3 | `u8 state` (**0 ready, 1 not ready, 2 cancel**) | buttons |
| `0x2409` | S→C | 4 | `u8 who (1 me, 2 partner, 3 both), u8 state (0 ready, else not)` | ready marks |
| `0x2408` | S→C | 3 | `u8 reason` | closes the window, clears the partner; 2 / 3 (cancelled by me / by the partner) play the error sound |

Original: every change clears both ready flags (`ExchangeCancleReadyPvP`); `0x2409 0` → `(1, 0)` to self, `(2, 0)` to the partner;
both ready → stakes taken (`ExchangePvP`), `0x2403` to both; `0x2409 2` → `0x2408 2` to self, `0x2408 3` to the partner, duel ended.
The winner gets the stakes at the end (`CUser::ItemDropPvP`, *inferred* from the name). confirmed (order), *inferred* (stake payout)

## Fight and end

| op | dir | size | fields | client |
|---|---|---|---|---|
| `0x2403` | S→C | 10 | `f32 x, f32 z` (area centre) | duel state on (`0x421550`): area centre, 6-second countdown (5307 "<t> accepted battle." for the requester first, then 5308 each second), `bgm_vs.wav` |
| `0x2404` | S→C | 2 | — | fight: 5311 "start battle!!", `ui_challenge01.wav`, `/defeat` enabled |
| `0x2406` | C→S | 2 | — | `/defeat` (5335), only during the fight |
| `0x2405` | S→C | 3 | `u8 result` | **1 I won**: my wins +1, motion state 6, 5310 "<me> won at the battle."; **2 I lost**: my losses +1 (not in region 2), 5310 with the opponent's name; **3 draw**: 5312 "a tie game". Then the duel ends (`ui_challenge02.wav`, the client drops its type-3 effects on the players, *inferred* debuffs; state reset) |
| `0x2406` | S→C | 7 | `u8 reason, u32 id` | **0** 5313 "…canceled as <p> disconnected."; **1** `id` me: 5314 "…as you left the allowed battle area.", else 5336 "…as <p> is out of allowed battle area."; **2** gave up: `id` me → my losses +1, state 13, 5310 with the opponent; else my wins +1, state 6, 5315 "<p> admitted the failure." and 5310 with my name; **3** 5329 "…canceled as you've been attacked before it finished."; `0xFF` ignored. Then the duel ends |

Original for `/defeat` (`0x2406`, state 4 only): both get `0x2406 (2, giver's id)`, stakes paid, duel ended, game log. The other
`0x2406` reasons and `0x2405` come from `CUser::PvPChkLeaveByDeath` / `ByMove` / `ByZoneLeave` / `ByAttack` / `PvPChkLoseDeath`
(see [Duels on the server side](../original/duel.md#6-how-a-duel-ends)).

## For a server

- Keep the order: `0x2401` → `0x2402 1` → `0x2407` → stake packets → `0x2403` → (5 s) → `0x2404` → `0x2405` / `0x2406`.
- `0x2403` carries floats (the centre), not ids.
- Check the area on the server (35 units around the centre, *inferred* from the client's warning radius) and send `0x2406 1`.
- Players must be enemies only while the duel is in state 4 (`IsTeam`, see [Player kills](../original/pvp.md#1-enemies)).

## Open

- The original's death / move / attack cancel paths (`PvPChkLeave*`) and the stake payout were not read for this page; they are on
  [Duels on the server side](../original/duel.md#6-how-a-duel-ends). **Live**: the counters.
