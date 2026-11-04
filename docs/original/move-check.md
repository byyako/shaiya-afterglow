# The move speed check

This page covers what the EP4.5 game server `ps_game.exe` (built 2009-07-15) does with each `0x0501` step after the 35-unit step check
([Movement, sitting, leaving](world.md)): a five-second window over the distance walked, against the fastest speed the player
had in it. It was read in the disassembly with the PDB names. Nothing was executed. The client's own speeds come from the client's
`game.exe` (US build, patch 108).

## In short

- **The server adds up how far a player walked over five seconds** and compares it with how far the player's speed allows, plus a fixed
  50 units of slack. Too far: the step is refused and the player is put back where the window started.
- **It is generous on purpose.** At the normal speed a player may cover about three times the distance the client really walks in
  five seconds; steps that arrive late and bunched (lag) can never fail a window.
- **Riding** is checked at one fixed speed; **a passenger** behind someone is never checked.
- **Charge** (the Fighter's and Warrior's dash) takes its reach off the distance counted, since the dash is the client's own.
- **The check starts again** whenever the player enters a zone or the server moves the player.
- **A refusal does not take back the distance gained**: the server keeps the last accepted position while the client is put back, and
  the next step usually lands the client where the fast walking had brought it.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (the PDB's plus 4 from the map pointer on).

## 1. The check, `stUserMoveChk`

Confirmed.

`CUser` holds one at `+0x5494` (44 bytes):

| offset | `CUser` | meaning |
|---|---|---|
| `+0x00` | `+0x5494` | the window's start (`GetTickCount`) |
| `+0x04` | `+0x5498` | the window's end: start + **5000 ms** |
| `+0x08` | `+0x549c` | the position at the window's start (x, y, z) |
| `+0x14` | `+0x54a8` | the tick of the last reset |
| `+0x18` | `+0x54ac` | the last position counted (x, y, z) |
| `+0x24` | `+0x54b8` | the distance walked in the window (float) |
| `+0x28` | `+0x54bc` | the fastest speed in the window (float) |

**`Reset(pos)`** (0x46a0b0): start = now, end = now + 5000, both positions = `pos`, distance and fastest speed 0.

**`Update(pos, speed)`** (0x46a120), for each step that passed the 35-unit check:

1. The horizontal distance (x and z, a square root) from the last position counted to `pos` is added to the distance walked.
2. The fastest speed becomes `speed` if that is higher; the last position counted becomes `pos`.
3. Before the window's end: accepted (`true`), nothing else.
4. At or after it: with `t` = (now − start) / 1000.0 seconds, the step passes when **`t × fastest + 50.0 ≥ distance walked`** (the
   x87 compare at 0x46a1dd: `jp` taken when the allowance is not below). Either way the check starts again:
   - passed: `Reset(pos)`, accepted;
   - failed: `Reset(window start)`, so the new window starts from where the old one started, and **refused** (`false`).

**`Add(n)`** (0x46a100, inlined where used): the distance walked goes **down** by `n`. Its only users are `CUser::AttackMob` (0x44fef5,
0x44ff37) and `CUser::AttackUser` (0x44f5fc, 0x44f63e): after a skill with id **56** (Charge) reaches its target, hit or miss, by the
skill's `byAttackRange` (`stSkillInfo + 0x4c`; 19, 20, 22 for levels 1 to 3), the dash being the client's own. Charge is the
Fighter's and Warrior's (`attackfighter` 1, any race: `country` 6, from level 1).

## 2. The speed

Confirmed, `CUser::PacketPC` 0x46a36d.

By the vehicle status (`+0x1460`, `docs/original/vehicles.md`):

| status | speed passed |
|---|---|
| 2 riding | **8.0** |
| 3 behind someone | `g_ConstUserMoveSpeed[4]` + 10000.0 = **10007.0**, so never refused |
| 0 on foot, 1 summoning | **`g_ConstUserMoveSpeed[GetMoveSpeedNow()]`** |

`g_ConstUserMoveSpeed` (0x542508) = **3.0, 4.0, 5.0, 6.0, 7.0**. `CUser::GetMoveSpeedNow` (0x48a530) is the move speed `+0x12f0`
(base 2, items, lapis, buffs) clamped to 0..4. It is not `GetMoveSpeed` (0x48a4d0): a move ban or a held player (speed 255 to the
client) is checked at its raw speed, which does not matter since the client does not move then.

The same table, times three, is the reach slack of `CUser::GetAttackEnableRange` (built, `combat::range_slack`): both read it as units a
second. The client's own table is in section 2a.

### 2a. The client's speeds

Confirmed, `docs/client/research-movement-storages-auction.md` "Queue 1".

Player update 0x418c00 and the local mover 0x43c000 (move index at player `+0x275`, set by 0x051C). Base 5.0, the index adds
-2, -1, 0, +1, +2, and walking halves it. Forward speed, units a second:

| move index | walk | run |
|---|---:|---:|
| 0 | 1.5 | 3.0 |
| 1 | 2.0 | 4.0 |
| 2 | 2.5 | 5.0 |
| 3 | 3.0 | 6.0 |
| 4 | 3.5 | 7.0 |

Index 255 gives no movement. **Mounted**: run **7.0** when the client's mount flag (`+0x378`) is 1 (shapes 14, 16) and **8.0** when it
is 2 (shapes 15, 17, 24 to 27, 29, 30); walking is half (3.5, 4.0). The server's flat 8.0 riding speed covers both. A passenger follows
its driver (the exemption stays). **Other directions**: lateral coefficients 0.5, 1, 1.5, 2, 2.5 and backward 0.8, 1.3, 1.8, 2.3, 2.8 by
index (mounted: 2); the mover adds the directions, so a diagonal can exceed the forward-only table (inferred: about 5.22 at index 2).
The forward table is therefore not a hard maximum for every input. Time is scaled in seconds (frame delta).

**`0x0501` sending** (sender 0x586060, 17 bytes): no minimum interval or distance. Walking sends when more than **1000 ms** passed since
the last (helper 0x437050); turns and actions when more than **250 ms** (0x43c000); a landing or vertical change when more than **500 ms**;
a stop sends at once. Corrections and warps send directly (0x553550, 0x5536a0, 0x553cb0, 0x54f330). Equal-position packets and bursts are
normal and must be accepted.

**Conclusion of the client research: do not replace the generous check with an exact forward-speed ceiling without a live calibration**
(sustained diagonals, slopes, mount variants, lag and frame caps).

**Against the server** (`move_check.rs`): `SPEEDS` 3.0 to 7.0 are exactly the client's run speeds by index, and
`RIDING` 8.0 is the client's highest mounted speed, so the check is consistent with the table: an honest forward run is at most the speed
the window is given, and walking is slower. A sustained diagonal at index 2 (about 5.22) would walk 26 units in 5 s against the 75
allowed. The 50-unit slack is what absorbs the cases the table does not cover.

## 3. Where it starts again

Confirmed.

`Reset` with the player's position, inlined in:

- `CUser::EnterZone` (0x44ae30, from `CZone::EnterUser`): entering any zone, the first time and after every map change;
- `CUser::MoveByGame` (0x44adb0, from `CZone::MoveUserLocal`): every move the server makes on the same map.

And inside `Update` (section 1, step 4). Nothing else touches it.

## 4. A refused step

Confirmed, 0x46a3cf.

The step's position is replaced by the window start (`+0x549c`, after the failed `Update` reset to it), then as for a step that is too
long: `CUser::StatusAttackStop`, **`0x020B`** (20 bytes: `u32 id, u16 map, f32 x, y, z` = the window start) to the view with the
player (`PSendViewAction`), **`0x0502`** (17 bytes: `u8 5, u32 id`, 12 zero bytes) to the player. `CZone::MoveUserByPacket` is not called:
**the server keeps the position of the last accepted step**, while the client is put back at the window start.

## 5. What it lets through, and oddities

1. **Only about three times the speed is caught.** A window closes at the first step after 5 s, so with steps coming steadily `t` is
   about 5: at speed index 2 the allowance is 5 × 5 + 50 = **75 units in 5 s (15 a second)**, against 25 walked at 5 a second. A client
   walking twice as fast (50 in 5 s) passes every window; a window only fails above 15 a second (index 4: 17, riding: 18). The fixed 50
   is generous on purpose: steps that arrive late and bunched (lag) can never fail a window.
2. **The server and the client disagree after a refusal** (section 4): the client stands at the window start, the server at the last
   step, up to five seconds of walking further on. The next step from the window start is then usually more than 35 from the server's
   position and is refused too, putting the client at the server's position, which is where the fast walking had brought it. So a
   refusal does not take back the distance gained.
3. The Charge allowance (section 1) is subtracted at the hit, not where the dash was; with a reach of 19 to 22 it fits in one accepted
   step anyway.

## 6. What the new server does now, and what changes

Now: the 35-unit step check only (`server.rs`, `MAX_STEP`), without `StatusAttackStop` on a refusal. No speed check, so a client that
walks fast in steps of up to 35 units is never stopped.

To build (`move_check.rs`, the rules; the `0x0501` handler):

1. A `MoveCheck` per connection in the world, as section 1: `reset(now, pos)`, `update(now, pos, speed) -> bool`, `shorten(n)`.
2. The speed of section 2: riding 8.0, behind someone none (not checked), otherwise the table at the current move speed (`totals`,
   clamped 0..4, without the move ban).
3. Reset with the position wherever the server places the player: entering a zone (`state.entered`), `move_player` on the same map,
   coming back to life on the same map (both `revive_here` paths and the resurrection skill's `Resurrected`). The original resets on
   every server move (section 3); these are all the new server has.
4. After a step passes the 35-unit check and the map check: `update`; refused as section 4 (with deviation 1). A step that is too long
   also stops attacking now, as the original's (section 4, the same code).
5. Charge (skill 56) reaching its target: `shorten(attack range)`, as section 1.
6. The original's numbers (5 s, + 50, the table, 8.0) as constants.

## 7. Deviations (built, section 9)

1. **After a refused step the server also places the player at the window start** (oddity 2): the zone's position, the view and the
   connection's copy all become the window start, so client and server agree and the distance gained is taken back. The packets are the
   original's (`0x020B` to the view with the player, `0x0502` 5 to the player), and a mount pair is not split (unlike a teleport).
2. **`move_check`** (default `true`): `false` turns the speed check off (the 35-unit step check stays), for a client that turns out
   to walk faster than the table.
3. **Logging**: each refusal at `info` (distance walked, allowance, seconds, speed); each closed window at `debug` with its average
   speed, so that a play session can show the client's real speeds (section 2, not verified).
4. **Not taken now: a tighter check** (oddity 1), for example a running allowance that carries over between windows, which would
   catch a client walking 1.5 times as fast. The client's table is known now (section 2a) but not that it is a maximum for every input, so
   a live calibration (deviation 3) comes first, or it would put back honest players. Until then the original's thresholds are kept.

## 8. Not covered

- A live calibration (deviation 3's "move window" lines) of sustained diagonals, slopes, mount variants and lag, before any tighter check.
- The unidentified states of `docs/original/world.md` section 2 step 2 (`+0x142c`, `+0x1430`) that skip a step entirely.

## 9. Built

`move_check.rs` (the window, `Checked::{Open, Passed, Refused}`, the constants), in the `0x0501` handler after the step and map checks
(`server.rs`, `speed_checked`): riding 8.0, a passenger not checked, otherwise the table at the current move speed. Started again on
entering a zone, in `move_player`, and on the three same-map revivals. A refused step: `stop_attacking`, `Zones::move_back` (the
window start for the zone and the view, `0x020B` to them and the player, a mount pair kept), `0x0502` 5, then the trade, duel and war
distance checks; the connection's position is the window start. Charge takes its reach off when it goes off. A step that is too long
now stops attacking too. `move_check` (default `true`). Logs: "move refused: too fast" (`info`), "move window" (`debug`). Tests:
`move_check` (4) and `game_flow::walking_too_fast_goes_back_to_the_windows_start`. Live test: checklist section 45.
