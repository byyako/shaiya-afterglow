# Client crash with many players in view: the prompt for the client analysis

A prompt for a Claude session that has the client repository and its analysis tools open (Ghidra or REA on the US `game.exe`, 2010-04-26,
patch 108, SHA-256 `5f045bcb…01a6`). The facts are from the server runs of 2026-11-12 and 2026-11-15
(`docs/testing/load-pvp.md`). It asks for static analysis only; the answers go to `docs/client/crash-many-in-view.md`.

```
Task: find out why the stock EP4.5 client (US game.exe, build 2010-04-26, patch level 108) crashes when many other players are in view,
and say what the server can do about it. Static analysis only (Ghidra/REA on game.exe); do not run the game. Use the project's status
words (confirmed with function and address, inferred, live) and its stop rule (about 30 queries on one question without a confirmed
function: write it up as open with what was tried).

What is known (from the server side; the server was healthy each time):
- 100 bots in view: no crash (15 minutes). 300 in view (2026-11-12): crash; the maintainer said it did not lag first.
  About 490 and 501 in view (2026-11-15): crash at 00:28:09Z, at the moment a new player entered, and again 00:39:06Z in steady state,
  2 min 20 s after entering the world. Each time the connection ended with "reset by peer" (the client process died).
- The server sent about 350 packets a second to each client at 500 players; no backlog, nothing dropped.
- The bots stand in one spot (a battle at map 18, (636, 394)), fight, die and revive (0x0551 rebirth, 0x0505 recover) every few
  seconds. Other players in view are sent as 0x0201 (TP_USER_ENTER) / leave, 0x0202 moves, 0x0205 and 0x0506 and so on; the server's
  packet builders are in crates/shaiya-proto/src (world.rs, combat.rs).

Questions, in this order:
1. The table of other players in view (the structure the 0x0201 handler fills): its capacity, whether it is fixed or grows, what
   happens when it is full or when an id repeats, and where its entries are freed (0x0202 leave, a map change). Is there any fixed array
   or pool in the player, effect, name-plate, shadow, texture or sound path with a limit between 100 and 500?
2. What the client creates per other player (model, textures, bone or animation data, effects, name sprite, a shadow, a sound
   source): which allocations are per player, which of them can fail or overflow, and the check (if any) on the result.
3. The handlers for the packets bots send most: moves, attacks and damage results, death, rebirth (0x0551), recover (0x0505), appearance
   changes. For each: what it does to a player that died and came back, and whether a stale or unknown id, or a dead player's id, can
   reach a null pointer, a freed object or an out-of-range index. Anything that grows without a bound while a player stays in a crowd
   (a per-frame list, a log, a particle list) counts.
4. Memory: the client is a 32-bit process. Roughly how much address space and how many GDI or D3D objects 500 players would use if
   each takes what question 2 found, against the 2 GB limit (or 4 GB with the large-address-aware flag; say whether it has it).
5. The crash reporter (MiniDumper, the `MiniDumperReport_*` settings): where it writes a dump and how to read the faulting address from
   it, so the next crash can be matched to a function.

Deliverable: docs/client/crash-many-in-view.md in the server repository, written as the other client pages are (addresses, the status
words, no machine-specific paths), ending with: (a) the most likely cause(s), each with its status word; (b) what the server can do
that keeps the original's behaviour for other cases (send fewer players in view, throttle appearance or rebirth packets, do not send a
particular packet to a particular state), and which of those would test the cause; (c) the questions that only a live session or a
crash dump can settle, with the exact steps for the maintainer.
```
