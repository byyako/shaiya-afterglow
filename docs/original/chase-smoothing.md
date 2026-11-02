# Monsters chasing: why they stop and start

This page explains why monsters "move, pause, move, pause" while they chase a player on an EP4.5 server: how the client moves a
monster after each `0x0603`, how often the game server `ps_game.exe` sends the steps, and why the two do not quite
meet. It also covers the client's NPC mover, which works the same way. Server: the Ghidra decompiler and the disassembly with the PDB
names. Client: the US `game.exe` (2010-04-26, patch 108), read statically. Nothing was executed. Monster numbers are the shipped
`Monster.SData`.

The chase itself (the steps, the searched way, the leash) is on [Monster paths (A\*)](pathing.md) and
[Monsters fighting back](mob-fight.md); guards and walking NPCs on [Walking NPCs and guards](guards.md).

## In short

- **The client walks each monster step at the monster's own pace** and stands the monster still when it gets there, whenever the next
  step has not come yet.
- **The usual monster needs about 1.9 seconds** for a chase step on the client, but **the server sends one every 2 seconds**: the
  monster's chase time is 1.8 s, and the server only looks at it in a pass every half second.
- **So the client arrives a little early, stops, and starts again** at the next step: short stops, each with the run animation dropped
  and picked up. Every original server does this.
- **The server's position is a whole step ahead.** The server moves a monster the whole step at once when it sends it; reach, attack
  range, the leash and the view all use that position, so what the player sees is up to one step (about 9 units) behind.
- **Walking NPCs stand still about a second** at every step, for the same kind of reason.

Status words: **confirmed** = read in the code (address given); **inferred** = follows from what was read or from a model.

## How it was read

Why monsters "move, pause, move, pause" while they chase (seen in play first with the crocodiles of `AI` 11; most
monsters do it), what the original does, and a proposed change (deviation 1, section 6) that keeps them moving.

Sources. Server: `ps_game.exe` (same build as the other analysis documents), Ghidra decompile through REA and the disassembly with PDB
names (`docs/original/symbols/ps_game.tsv`); the chase itself is specified in `docs/original/pathing.md` sections 3 to 5 and
`docs/original/mob-fight.md` sections 1, 4 and 5, not repeated here. Client: the US `game.exe` of `docs/client/` (2010-04-26, patch 108,
SHA-256 `5f045bcb…7e3401a6`), read **read-only** (objdump and Ghidra; nothing was run or
changed) for this one question: how the client moves a monster after `0x0603`. Nothing was executed. Numbers come from the database
(`defs.mobs`, which agrees with the client's `Monster.SData` for every spawned monster: `docs/client/skill-monster-sdata-vs-defs.md`
section 4) and from a simulation of both sides (section 4).

## 1. The client's monster mover

Confirmed, static.

**`0x0603` handler** (0x54e810, `u32 id, u8 byMotion, f32 x, f32 z`): looks the monster up (0x447900 on the monster list 0x7051a0); stores
x at `+0x5c` and z at `+0x64` (the "predicted position", `몬스터 예측 위치` in the client's own debug dump, 0x430fd5; `+0x60` is not set
from the packet) and the motion at `+0x84`; when the spot differs from the current position (`+0x4`, `+0xc`), stores the unit direction
towards it (normalised by 0x5cda1b, height 0) at `+0x68` (the "predicted direction"). It does **not** move the monster.

**The per-frame update** (the monster loop at 0x44f714 calls 0x431aa0 for every monster, every frame):

1. Looks up the monster's `Monster.SData` record by its mob id `+0x2c` (0x461210 on 0x854ecc).
2. The speed adjustment from the monster's **move speed index `+0xb5`** (jump table 0x431f5c): 0 → −4, 1 → −2, 2 → 0, 3 → +2, 4 → +4,
   anything else → 0. This is the server's `g_ConstMobMoveSpeed`. The index is 2 when the monster is made (0x41579a) and is set by
   `0x0304` (0x54d830, `u32 id, u32 HP, u8 attack speed, u8 move speed`) and `0x0609` (0x54f2b0).
3. **The speed**: running (`+0x84` ≠ 0) uses `ChaseStep` (record `+0x20`, u8) and `ChaseTime` (`+0x1c`, u32 ms); walking uses `NormalStep`
   (`+0x18`) and `NormalTime` (`+0x14`). With `s` = step + adjustment and `T` = time:
   **`v = s / (T × 0.001) − 0.3` units per second** (constants 0x6a8594 = 0.001, 0x6a67f4 = 0.3), times the frame time 0x701b14 (seconds,
   from the performance counter; 0.05 when it reads 0 or less, 0.1 in scenes 2, 5 and 9 when it reads 1 or more).
4. Turns the facing (`+0x10`) towards the predicted direction, then moves the position towards the predicted position by `v × frame`:
   - **more than 20 left** (0x6a6688): **jumps there** at once;
   - the frame's move reaches it (what is left, less 0.005, is not more than the move): **puts the monster on it and switches it to
     standing** (action `+0x4c` 0, animation 6 unless it is attacking, `+0x4c` 2);
   - else a step of `v × frame` (the height from the terrain).

So the client walks every `0x0603` at the monster's own pace, independent of when the server sends the next one, and **stands still in
between** whenever it gets there first.

For the usual monster (`ChaseStep` 9, `ChaseTime` 1800, index 2): `v` = 9 / 1.8 − 0.3 = **4.70 units/s**, so a 9-unit chase step takes
**1.915 s**. Walking (`NormalStep` 8, `NormalTime` 4000): 1.70 units/s, 4.7 s for an 8-unit step.

Chase timings of the spawned monsters (`defs.mobs` joined to `world.mob_area_mobs`): `ChaseTime` 1800 for all but 2 kinds (2400) and 34
that do not move (0); `ChaseStep` 8 to 14, mostly 9 (711 kinds), 8 (360) and 10 (125).

## 2. The original server's step times

Confirmed.

- A full chase step is `s = g_ConstMobMoveSpeed[move speed] + byChaseStep`, the same `s` as the client's (section 1 step 2), and waits
  **`dwChaseTime`** (`docs/original/pathing.md` section 3); a shorter last step waits `trunc(g / s × dwChaseTime)`; a turn of a searched
  way is taken at once (the next chase time now).
- The waits are only looked at **when a pass runs**: ordinary monsters chase and go home from the attack pass every **500 ms**
  (`CZone::UpdateMobAttack` 0x421990 → `CMob::UpdateAttack` 0x4a7db0: skips state 100, then for state 1 `AttackAI` only once
  `+0xcb4` ≤ now, for state 3 `ReturnHome`); scripted monsters (`AI` 10 and above, state 100) fight from their machine's `Battle` in the
  idle pass every **1000 ms** (`docs/original/scripted-mobs.md`); Lua monsters chase with `ChaseExp` (`dwChaseTime` − 400) in their
  one-second update (`docs/original/instances.md` section 10).
- So a 1800 ms wait becomes **2000 ms** in practice (the fourth 500 ms pass, or the second 1000 ms one), and a Lua 1400 ms wait also 2000.

The new server does exactly this (`fight::chase`, `fight::homeward`, the 500 ms `mob_fight`, the 1 s `scripts` and `lua` passes).

## 3. Why the monsters stop

The client needs 1.915 s for a step the server sends every 2.0 s: it **arrives about 85 ms early and switches to standing**, then the
next `0x0603` starts the run again. A last, shorter step or a turn waits a whole extra pass. That is the "move, pause": short stops, each
with the run animation dropped and picked up again. The original server does the same; it is not a bug of ours. The stops cannot be
removed while keeping the original's 2-second rhythm, because the client walks faster than that (4.70 against 4.5 units/s).

## 4. Simulation (the server's code against a model of section 1)

A scratch test (not kept) ran `fight::chase` on open ground: monster 24 units from its target, `ChaseStep` 9, `ChaseTime` 1800, the target
standing or walking away, and fed every `0x0603` to a model of the client mover (60 frames a second, the rules of section 1 step 4).

| server | target | steps sent every | stops (number, longest) | client behind the server at most |
|---|---|---|---|---|
| as the original, 500 ms pass | standing | 2.0 s | 2, 0.10 s | 8.9 |
| as the original, 500 ms pass | walking 3 u/s | 2.0, 2.0, 1.0, 2.0 s | 4, 0.25 s | 8.9 |
| as the original, 1 s pass (scripted) | walking 3 u/s | same | 4, 0.25 s | 8.9 |
| as the original, move speed index 0 | walking 1 u/s | 2.0 s | 3, 0.30 s | 5.1 |
| exact `dwChaseTime` on a 100 ms tick | walking 3 u/s | 1.8 s | 0 | 9.8 |
| exact `ChaseExp` wait (1400) on a 100 ms tick | walking 3 u/s | 1.4 s | 0 | **13.6, and growing** (1.7 u/s faster than the client) |
| **paced to the client (section 6), 100 ms tick** | standing / 3 / 4.6 / 4.69 u/s | 1.9 s | **0** | 9.0 to 9.4 |
| paced, move speed index 0 / 4 | 1 / 3 u/s | 2.0 / 1.3–1.8 s | 0 | 5.1 / 12.9 |
| paced, Lua `ChaseExp` | walking 3 u/s | 1.9 s | 0 | 9.4 |

No case reached the client's 20-unit jump. Running the waits exactly on a fine tick removes the stops but lets the client fall behind
wherever the server's pace is quicker than the client's (the Lua chase: by 1.7 units every second until the 20-unit jump). Pacing each
step by the client's own walking time removes the stops in every case and keeps the client within one step of the server.

## 5. What a monster's server position means

Unchanged by any of this: the server moves a monster's position the whole step at once when it sends `0x0603`; the client gets there
later. Reach, the attack's range test, the leash and the view all use the server's position, so the client always shows the monster up to
one step (about 9 units) behind where the server has it. That is the original's behaviour and the proposal keeps it.

## 6. Deviations (all approved and built, section 9)

1. **Chase steps are paced to the client.** Every step a monster *runs* (`0x0603` motion 1: the chase of ordinary, scripted and Lua
   monsters, going home, the Lua `WhileReturnHome`) waits for the client's walking time instead of the original's wait:
   - each monster keeps **the time the client will reach its last spot** (`client_eta`): with `d` the length of the step just sent and
     `v` the client's speed of section 1 step 3 (from the monster's `ChaseStep`, `ChaseTime` and the move speed index the server last
     sent), **`client_eta = max(client_eta, now) + d / v`**, and the **next step may go at `client_eta − 100 ms`**;
   - a step that sends nothing (the monster already in reach, the next turn "now") keeps the original's timing;
   - where `v` is 0.05 units/s or less (a `ChaseTime` of 0, a tiny step), the original's wait.

   *Why*: section 3; the client never reaches a spot before the next one arrives, so it never stands still in a chase, and the time
   anchored on the client's arrival does not drift (section 4). *What changes for players*: monsters chase and run home at the pace the
   client shows them, **about 10 % quicker than the original's actual 2-second rhythm** (1.915 s instead of 2.0 s for the usual 9-unit
   step; 5 % slower than the bare `dwChaseTime` of 1.8 s the data asks for); a player running away is caught a little sooner; Lua
   monsters' `ChaseExp` is paced the same way (1.915 s instead of 2.0 s; their 400 ms quicker steps were lost to the one-second pass in
   the original too).
2. **A 100 ms move tick between the passes.** The monster updater runs every 100 ms; the existing passes keep their times (500 ms
   fights, 1 s idle, scripts and Lua) and decide as now. On the ticks in between, a monster that is **in the middle of a chase or of
   going home** (its last turn sent a running step) and whose next step time has come takes **one more step and nothing else**:
   - the chase: the same `fight::chase` with the attack entry the last fight turn chose (`entry_used`), against the target's current
     position; only a step is applied; any other outcome (in reach, give up, go home, unfair, no way) is **left to the next pass**, which
     decides it as now, and the monster's path is left as it was;
   - going home (`fight::homeward`): only a step is applied, arrival and the rest are left to the next pass;
   - only while the target is still on the map and may be targeted (`targetable`); never for a monster stunned, asleep, stopped (move
     speed 255) or dead.

   *Why*: with only the 500 ms and 1 s passes, a step due at 1.915 s would still wait for the 2.0 s pass and the stops would stay.
   Running the whole fight turn every 100 ms would change what monsters decide (retargeting by aggro, `byOK` 6, entry choice and the
   calls of `rand()` happen once per turn), so only the step is repeated. A monster that comes into reach on a tick in between strikes
   at the next pass, at most 400 ms later (900 ms for scripted and Lua monsters), as a monster reaching it during the original's wait.
3. **A setting**: `GAME_SMOOTH_CHASE` (default `true`); `false` keeps the original's waits and passes exactly.
4. **The Lua `WhileMove` is paced too** (approved): a Lua monster walking to a spot (motion 0) waits for the client's
   walking time, `NormalStep` and `NormalTime` in the speed of section 1 step 3; the move tick takes its steps while it is 10 or more from
   the spot (nearer, the pass's `5 + rand() % 5` may call it there). *Why*: in the original the server walks 2 units/s against the
   client's 1.7, so a long walk leaves the client behind until its 20-unit jump.
5. **Guards and walking NPCs are paced too** (approved; the client's NPC mover in section 7): a guard's running
   steps (chasing, going back) and an NPC's walking steps wait for the client's walking time (`nMoveDist / nMoveSpeed`, four times that
   running), anchored on the client's arrival as in deviation 1, and the move tick takes them between the passes (a step only: no look
   around, no blow, no arrival at a walk's position, whose wait is drawn by the pass). *What changes*: a guard's full chase step keeps its
   2 s (the client runs 16 units in exactly 2 s), so it now goes 100 ms early instead of just as the client stands; its last, shorter step
   no longer waits for the next 500 ms pass; a walking NPC (16 units every 8 s, which the original's 3 s pass makes 9 s) walks on
   without its 1 s stop. Ordinary monsters' idle walking about and the scripted `MoveRandom` stay as the original.

Not changed: monsters' idle walking about and the scripted `MoveRandom`/`MoveRandomRun` (random steps around home), monster attacks and
skills, what a pass decides, the step lengths and spots, and the server's own position of a monster or NPC (section 5).

## 7. The client's NPC mover

Confirmed, static, read-only as section 1.

**`0x0E03` handler** (0x54f7c0, `u32 id, u8 byMotion, f32 x, y, z`): looks the NPC up (0x4479a0), stores the spot at `+0x8c` to `+0x94`,
the unit direction towards it at `+0x98` (when it differs from the position `+0x38`/`+0x40`), and **`byMotion ≠ 0` as the run flag
`+0xac`**. It does not move the NPC.

**The per-frame update** (0x433e26 on): the speed is **`+0xf4 / (+0xf8 × 0.001)`** units a second, **times 4.0 when running**
(0x6a67a4), times the frame time; `+0xf4` and `+0xf8` are the NPC's `NpcQuest.SData` record `+0x0c` and `+0x10` (0x433770, by NPC
type), which are `nMoveDist` and `nMoveSpeed` (the client's values agree with `world.npcs`: `docs/client/npcquest-sdata-vs-world.md`).
The rest is the monster mover's: more than 20 left jumps there, a frame that reaches the spot puts the NPC on it and ends the move,
else a step. No 0.3 is taken off.

For the usual NPC (16, 8000): walking 2 units/s (16 units in 8 s), running 8 units/s (16 units in 2 s). The server's guard chase step is
16 every `nMoveSpeed / 4` = 2000 ms (`docs/original/guards.md` section 8), exactly the client's pace; its last, shorter step waits
`trunc(g / s × 2000)` and then the next 500 ms pass. Walking NPCs step 16 every 8 s, but only on the 3 s pass (`CZone::UpdateNpcIdle`,
`docs/original/guards.md` section 4), so every 9 s: the client stands 1 s each step.

## 8. Build notes

- `fight.rs`: `client_speed(step: i32, time: Duration) -> f32` (section 1 step 3, in f32 as the client) and `paced(now, eta, d, v) ->
  (Instant, Instant)` (new `client_eta`, next step time); `Chase::Step`/`Homeward::Step` gain the distance moved, or the callers measure
  it.
- `mobs.rs` `Mob`: `client_eta: Instant` (reset on spawn and when a monster is moved by anything else: a jump, a Lua move, rebirth) and
  `running: bool` (the last turn sent a running step; cleared on every other outcome).
- `server.rs` `mob_updater`: interval 100 ms; `mob_fight` and guards on every fifth tick, the one-second passes on every tenth (keep their
  order); `zones.chase_steps(now, …)` on the others when `GAME_SMOOTH_CHASE` is on.
- `zone.rs`: `chase_steps` (deviation 2); `fight_turn`, `go_home`, the scripted `battle` and the Lua `chase_ex`/`WhileReturnHome` pace their
  steps when the setting is on.
- Tests: `fight` unit tests for `client_speed` (9/1800 → 4.70, index 0 → 2.48, time 0 → none) and the pacing; zone tests that a chasing
  monster steps at 1.8–1.9 s on 100 ms ticks, that a tick in between never strikes, retargets or goes home, and that with the setting off the
  steps come every 2 s as now. Live test: a new checklist section (crocodiles of `AI` 11 and an ordinary monster, standing and walking away;
  watch that the monster never stands still until it is in reach, and that it still gives up at the leash).

## 9. As built

`crates/game-server`: `pacing.rs` (the client's speeds, `paced`), `mobs::Moving` and `Mob::client_eta`/`moving`, `zone.rs`
(`paced_next`, `stop_moving`, `Zones::set_smooth_chase`; the paced steps of `fight_turn` and `go_home`), `zone/mob_skills.rs` (the
skill chase), `zone/lua.rs` (`chase_ex`, `walk` for `WhileMove` and `WhileReturnHome`), `zone/npc.rs` (`npc::Moving`, `Npc::client_eta`,
the paced guard chase, going back and walk, `move_tick`), `zone/move_tick.rs` (`Zones::move_steps`), `server.rs` (`mob_updater` on
100 ms ticks with smoothing: the passes on every fifth), `config.rs` (`GAME_SMOOTH_CHASE`). Tests: `pacing` (speeds, arrival
anchoring), `zone::move_tick` (a chase stepping at 938 ms then 1977 ms on the ticks, no blow and no decision on a tick, the original's
waits with smoothing off), `zone::npc` (a guard's paced chase and its last step on a tick, a walking NPC's step at 7.9 s). The flow
tests run with smoothing off (their timings are the original's). Live test: `docs/testing/live-test-checklist.md` section 42.

Small choices made in the build:

- **A chase step on a tick also needs the monster's attack time** (`m_dwNextAttackTime`): the pass only fights a monster whose attack time
  has come, so a monster that has just struck does not run off on a tick before its next turn.
- **The entry is the last turn's.** A tick does not look at the other attack entries; a skill entry that becomes ready between the passes
  is taken at the next pass.
- **A Lua monster's tick step** needs its stack's top to be still the same status (`COMBAT` for a chase, `HOME`, `MOVING`) and, for a
  chase, no hold-position (`LuaSetHoldPosion`) and a target without a shape, as `AttackAIExp` asks.
- **Whatever moves a monster otherwise** (an idle step, a summon, a Lua jump) leaves its arrival time as it is; an arrival time in the past
  counts from now, so at most one step's time is affected.
