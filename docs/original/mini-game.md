# The gambling house

This page covers the mini-game of the EP4.5 game server `ps_game.exe`: a 3 × 3 slot machine with up to eight lines,
and a high-or-low card game played with a slot win as the stake (`0x31xx`, `CUser::PacketMiniGame`, `CMiniGame`). It was read in the
Ghidra decompiler and the disassembly with the PDB names, in `ps_game.exe` and `ps_gameLog.exe`. Nothing was executed. The client's side
(US `game.exe` 2010-04-26, patch 108) was read statically and is summed up [at the end](#13-the-clients-side-research-entry-6).

## In short

- **The whole game is in the server and the client, but nobody could ever reach it.** The shipped maps place six gambling NPCs (type 5),
  and the original server never creates type-5 NPCs ([Maps and NPCs](maps-npcs.md)). A server that creates them gets a working
  game.
- **Seven tables**, from 100 to 3000 gold a line. Sitting down takes the player out of the zone (others see the character leave) until
  they stand up.
- **The slot machine** pays by lines: up to eight lines a spin, each line of three equal symbols pays its symbol's multiplier (2 to 10)
  times the table's bet. After a loss the next spin is paid at once (it can be taken back).
- **It is a strong gold sink**: a spin gives back about **41 %** of the bet on average.
- **The card double-up** risks a slot win on high or low against the dealer's card, up to five rounds: each win multiplies the stake
  (up to 3840 times the first stake after five wins), each round is harder, a loss gives back a fifth of the stake.
- **Standing up, logging out or losing the connection** hands back whatever is on the table.
- **Sitting down checks nothing** but "in the world": not the NPC, death, a trade, a personal shop or a duel.

Status words: **confirmed** = read in the code (address given).

## How it was read

The original's mini-game: a 3 × 3 slot machine with up to eight lines, and a high-or-low card game played with a slot win as the stake.
`ps_game.exe` has all of it (`CUser::PacketMiniGame`, `CMiniGame`), the client has a handler and a sender for every `0x31xx`
(`docs/client/client-packets.generated.md`), and the shipped maps place six gambling NPCs (type 5), but **the original never creates type-5
NPCs** (`CMap::InitNPC`, `docs/original/maps-npcs.md` section 4), so nobody could reach the game. The new server builds it
with the NPCs shown and safety checks (section 9). Read from `ps_game.exe` (sha256 `0fb7c9f1…`, `PSM_Client/Bin`) and
`ps_gameLog.exe` with the PDB names (`docs/original/symbols/`), `tools/disasm.py` and REA/Ghidra; nothing executed.

**The client's side** was read statically by research entry 6 (`docs/client/research-gambling-kill-boost.md`):
section 13 compares it with this document and the build. Play test: section 11.

## 1. Where it sits

Confirmed.

- **The NPCs**: `world.npcs` type 5 has 8 rows (type ids 1 to 8, countries 0, 0, 0, 1, 1, 1, 2, 2); the map files place type ids 1 to 6
  once each: **map 1 (type id 1), map 2 (4), map 19 (2), map 20 (5), map 28 (3), map 29 (6)**, one position each.
- **The dispatch** (`CUser::PacketProcessing` 0x466da0): **in the world** (`m_nWhere` 4) only `0x3101` reaches `PacketMiniGame`
  (0x4673f0); **while waiting to enter a zone** (`m_nWhere` 3) every `0x31xx` does (0x466f1b), and chat (`0x11xx`, `F1xx`) only while a game
  is open (`+0x1760` ≠ 0); `0x0201` there enters the zone (`ZoneEnterUserMove`).
- **The state** lives in the user (`CUser+0x1760`, `CMiniGame`, 0x20 bytes): `+0 state` (0 none, 1 slot, 2 slot won, 3 card: waiting for
  the guess, 4 card won), `+4 table` (0 to 6), `+8 bet` (the slot's bet, then the card's stake), `+0xc lines` (lines bet, 0 to 8), `+0x10
  pending` (the slot's winnings, or a re-bet already paid), `+0x14 round` (cards won), `+0x18 dealer` (the dealer's card), `+0x1c` the user.
- **Leaving** (`CWorld::LeaveUser` 0x40cb07, `LeaveUserDBAgent` 0x40cde1) calls `GameExit` (section 4), so a logout or a lost connection
  hands back what is on the table.

## 2. The packets

Confirmed, `CUser::PacketMiniGame` 0x4781f0, jump table 0x47833c.

| opcode | client → server | server → client |
|---|---|---|
| `0x3101` | `u8 table` (0 to 6): sit down | `u8 ok` (3 bytes) |
| `0x3102` | — : bet one more line, or take back the re-bet | `u8 result, u32 bet, u32 money` (11): 0 refused, 1 line added, 2 bet handed back |
| `0x3103` | — : spin | `u8 ok, u8 cells[9], u8 won, u32 winnings, u8 short` (18) |
| `0x3104` | — : double up (start the card game with the slot win) | `u8 dealer` (3) |
| `0x3105` | — : take the slot win and play again | — (2) |
| `0x3106` | — : stand up | `u32 money` (6), after `0x0213 u32 money` |
| `0x3107` | `u8 guess` (0 low, else high) | `u8 card, u8 result, u32 amount` (8): 0 lost, 1 won (go on or stop), 2 won the fifth round |
| `0x3108` | — : next card | `u8 dealer` (3) |
| `0x3109` | — : stop and take the stake | `u32 money` (6) |

A packet in the wrong state gets **no answer**, except `0x3103` (answers `ok` 0, the rest of its 18 bytes unset in the original: zeros here)
and `0x3101` (answers 0). Money changes are saved at once (`0x0603` to the database agent, `CUser::SendDBMoney`); the client is told the
money only by `0x3102`, `0x3106` (with `0x0213` before it) and `0x3109`.

**The client reads** (section 13): every field above at these offsets and sizes, except that it reads **no payload** of `0x3105` and
`0x3109` (`0x3109`'s money is ignored), treats `0x3103`'s `won` and `short` as flags, and does not tell `0x3107` result 2 from 1 (it counts
the rounds itself). It takes the money from an answer only at `0x3102` result 1 and `0x3106`.

## 3. Sitting down and standing up

Confirmed, `CMiniGame::GameEnter` 0x4bb200, `CWorld::WaitUserFromZone` 0x40cf60, `GameExit` 0x4bb320.

**`0x3101 table`**: only in the world (`m_nWhere` 4). `GameEnter(table)`: refused when a game is open (state ≠ 0) or `table` ≥ 7; else
state 1, `table`, bet/lines/pending 0, and the game log record `0x0D01` (section 7). Then `WaitUserFromZone`: the player **leaves the
zone** (`CZone::LeaveUser`: the others see it go) and waits to enter it again (`m_nWhere` 2, then 3), exactly as for a move within the map;
**`0x3101 1`**. If the wait cannot start (not in a zone), the game is reset and **`0x3101 0`** (and the connection is closed, reason 9). No
other check: not the NPC, not death, trades, shops or duels.

**The table's bet per line** (`g_MiniGameNpc` 0x531ca4): **100, 300, 500, 1000, 1500, 2000, 3000** for tables 0 to 6.

**`0x3106`** (and every leave): `GameExit`: by state, **1**: the bet (`+8`, a re-bet included) back to the money; **2**: the slot's winnings
(`+0x10`) to the money, record `0x0D03`; **3, 4**: the card stake (`+8`) to the money, record `0x0D03`; each at most `0xFFFFFFFF`
(`CUser::ChkAddMoney`), saved; then **`0x0213 u32 money`**, state 0, record `0x0D02`. `PacketMiniGame` then sends **`0x3106 u32 money`**.
The player is still out of the zone: the client copies the money, reloads the map and sends `0x0201` (verified static, section 13).

## 4. The slot machine

Confirmed, `SlotBatting` 0x4bb480, `SlotRun` 0x4bb580, `SlotRestart` 0x4bb940, `InitMiniGame` 0x4bae60.

**`0x3102`** (state 1 only, else nothing): when no re-bet is pending (`pending` 0): with **lines < 8** and money ≥ the table's bet: bet +=
the table's bet, money −= it (saved), lines + 1, **result 1**; else **result 0**. When a re-bet is pending (`pending` ≠ 0): the bet goes back
to the money (at most `0xFFFFFFFF`, saved), bet, lines and pending 0, **result 2**. The answer carries the bet and the money after.

**`0x3103`** (state 1 with lines ≥ 1, else `ok` 0):

1. **The class** (how many lines win): `r = rand % 10000`; the class is the first `c` with `r < [6000, 8000, 9000, 9400, 9700, 9900, 9980,
   9999, 10000][c]` (60 %, 20 %, 10 %, 4 %, 3 %, 2 %, 0.8 %, 0.19 %, 0.01 % for classes 0 to 8); **class 7 becomes 6** (the pattern table
   has no class 7).
2. **The symbol** of the win (classes 1 to 8; 0 for class 0): `r = rand % 226`; the first `s` (0 to 21) with `r <` the running total of
   the weights below (20, 40, 60, 75, …, 226). The symbols, as `(multiplier, weight)` (`g_MiniGameCard` 0x531cc0):
   `(2,20) ×3, (3,15) ×3, (4,12) ×3, (5,10) ×3, (6,8) ×3, (7,6) ×3, (8,4) ×2, (9,3), (10,2)` for symbols 0 to 21 in that order.
   The client draws value `v` as tarot card `v` (0 The Fool … 21 The World, section 13); 22 is its card back and is never sent.
3. **The cells** (classes 0 to 7; indexes 0 to 8, rows of three: 0 1 2 / 3 4 5 / 6 7 8): a pattern `p = rand % 32` of `NO_BINGO`
   (section 4.1); each cell `k`, in order: with bit `k` of `NO_BINGO[p]` set, `v = rand % 11` (0 to 10), and if `v == s`, `v + 1`, 0 when that
   is 11; with the bit clear, `v = rand % 11 + 11` (11 to 21), and if `v == s`, `v + 1`, 11 when that is 22. No line can form: every pattern
   mixes the two halves on every line, and no cell equals `s` (with class 0, `s` is 0: symbol 0 never shows on a losing spin).
4. **The win** (classes 1 to 6): `k = rand % COUNT[c] + START[c]` and every cell whose bit is set in `BINGO[k]` becomes `s`; those
   patterns make exactly `c` lines (checked over the table). **Class 8**: all nine cells `s` (every line).
5. **The lines** (`g_MiniGameSlotLine` 0x531d98), **in this order, the first `lines` of them count**: `(3,4,5)` the middle row, `(0,1,2)`
   the top, `(6,7,8)` the bottom, `(2,4,6)`, `(0,4,8)` the diagonals, `(2,5,8)` the right column, `(1,4,7)` the middle, `(0,3,6)` the left.
   Each counted line whose three cells are equal wins its symbol.
6. **No winning line**: record `0x0D04` (lines, bet, 0, 0) and `0x0D03` (−bet); the bet is lost (it was paid at `0x3102`); then the
   **automatic re-bet**: with money ≥ bet, money −= bet (saved), `pending` = 1, state 1; else bet, lines and pending 0. **`0x3103 1, cells,
   0, 0, short`** with `short` = 1 when the re-bet could not be paid.
7. **Winning lines**: `pending` = Σ over them of the symbol's multiplier × the table's bet; state 2; records `0x0D04` (lines, bet, lines won,
   winnings) and `0x0D03` (−bet). **`0x3103 1, cells, 1, winnings, short`** with `short` = 1 when money + winnings < bet. The winnings
   are not in the money yet.

**`0x3105`** (state 2 only): the winnings to the money (capped, saved), record `0x0D03` (+winnings); then with money ≥ bet the same bet
again (money −= bet, saved; `pending` keeps a non-zero value: a re-bet is paid), else bet, lines and pending 0; state 1; **`0x3105`**.

**The return**: with the original numbers a spin gives back **40.7 % to 41.4 %** of the bet on average (by the number of lines; computed
from the tables: the classes' chances × the lines their patterns put among the counted ones × the mean multiplier 930/226).

### 4.1 The pattern tables

Confirmed, `g_NoBingoSet` 0x531e10, `g_BingoSet` 0x5322d0, `g_nNumBingo` 0x532290, `g_nStartBingo` 0x5322b0.

Each entry is 9 `int` flags in the original; here **bit `k` = cell `k`**.

`NO_BINGO` (32):
```
0x18c, 0x11c, 0x09c, 0x19c, 0x162, 0x152, 0x072, 0x172, 0x14a, 0x16a, 0x11a, 0x15a, 0x10e, 0x18e, 0x14e, 0x11e,
0x0e1, 0x0b1, 0x071, 0x0f1, 0x0a5, 0x0e5, 0x095, 0x0b5, 0x08d, 0x18d, 0x0ad, 0x09d, 0x063, 0x163, 0x0e3, 0x073,
```

`BINGO` (282; class `c` uses `START[c]..START[c] + COUNT[c]`: class 1 0 + 152, class 2 152 + 72, class 3 224 + 34, class 4 258 + 13,
class 5 271 + 6, class 6 277 + 4; entry 281, all cells, is class 8's and unused, as class 8 is written directly):
```
0x1c0, 0x1e0, 0x1d0, 0x1f0, 0x1c8, 0x1e8, 0x1d8, 0x038, 0x138, 0x0b8, 0x1b8, 0x078, 0x178, 0x0f8, 0x1c4, 0x124,
0x1a4, 0x164, 0x054, 0x154, 0x0d4, 0x134, 0x1b4, 0x074, 0x0f4, 0x1cc, 0x12c, 0x1ac, 0x16c, 0x05c, 0x15c, 0x0dc,
0x03c, 0x0bc, 0x1c2, 0x1e2, 0x092, 0x192, 0x0d2, 0x0b2, 0x1b2, 0x0f2, 0x1ca, 0x1ea, 0x09a, 0x19a, 0x0da, 0x03a,
0x13a, 0x07a, 0x17a, 0x1c6, 0x126, 0x1a6, 0x166, 0x096, 0x196, 0x056, 0x156, 0x136, 0x0b6, 0x076, 0x1ce, 0x12e,
0x1ae, 0x16e, 0x09e, 0x19e, 0x05e, 0x15e, 0x03e, 0x1c1, 0x1e1, 0x111, 0x191, 0x151, 0x131, 0x1b1, 0x171, 0x049,
0x149, 0x0c9, 0x069, 0x169, 0x0e9, 0x119, 0x199, 0x059, 0x0d9, 0x039, 0x0b9, 0x1c5, 0x125, 0x1a5, 0x165, 0x115,
0x195, 0x055, 0x0d5, 0x075, 0x0f5, 0x04d, 0x14d, 0x0cd, 0x12d, 0x1ad, 0x06d, 0x0ed, 0x11d, 0x19d, 0x03d, 0x0bd,
0x1c3, 0x1e3, 0x113, 0x093, 0x153, 0x0d3, 0x133, 0x0b3, 0x173, 0x0f3, 0x04b, 0x14b, 0x0cb, 0x06b, 0x16b, 0x0eb,
0x11b, 0x09b, 0x05b, 0x03b, 0x007, 0x107, 0x087, 0x187, 0x047, 0x147, 0x0c7, 0x027, 0x0a7, 0x067, 0x0e7, 0x017,
0x037, 0x00f, 0x10f, 0x08f, 0x18f, 0x02f, 0x0af, 0x01f, 0x1f8, 0x1e4, 0x1d4, 0x174, 0x1ec, 0x1dc, 0x13c, 0x1bc,
0x07c, 0x0fc, 0x1d2, 0x1f2, 0x1da, 0x0ba, 0x1ba, 0x0fa, 0x1e6, 0x0d6, 0x1b6, 0x176, 0x0f6, 0x1ee, 0x0de, 0x13e,
0x0be, 0x07e, 0x1d1, 0x1f1, 0x1c9, 0x1e9, 0x159, 0x139, 0x1b9, 0x079, 0x0f9, 0x1e5, 0x155, 0x135, 0x1b5, 0x1cd,
0x16d, 0x05d, 0x0dd, 0x193, 0x1b3, 0x1cb, 0x1eb, 0x19b, 0x15b, 0x0db, 0x13b, 0x0bb, 0x07b, 0x1c7, 0x127, 0x1a7,
0x167, 0x117, 0x097, 0x057, 0x0b7, 0x077, 0x04f, 0x14f, 0x0cf, 0x12f, 0x1af, 0x06f, 0x0ef, 0x11f, 0x09f, 0x03f,
0x1f4, 0x17c, 0x1fa, 0x1d6, 0x1de, 0x1be, 0x17e, 0x0fe, 0x1d9, 0x179, 0x1d5, 0x175, 0x1ed, 0x15d, 0x13d, 0x1bd,
0x07d, 0x0fd, 0x1d3, 0x1f3, 0x1bb, 0x17b, 0x0fb, 0x1e7, 0x197, 0x157, 0x0d7, 0x137, 0x0f7, 0x1cf, 0x16f, 0x19f,
0x05f, 0x0bf, 0x1fc, 0x1f6, 0x1f9, 0x1f5, 0x1dd, 0x1db, 0x1b7, 0x177, 0x1ef, 0x15f, 0x0df, 0x13f, 0x07f, 0x1fe,
0x17d, 0x1fb, 0x1d7, 0x1bf, 0x0ff, 0x1fd, 0x1f7, 0x1df, 0x17f, 0x1ff,
```

## 5. The card double-up

Confirmed, `CardStart` 0x4bba50, `CardRun` 0x4bbaa0, `GetRandCardUser` 0x4baf70, `CardNext` 0x4bbc50, `CardStop` 0x4bbc90.

**`0x3104`** (state 2 only): the stake = the slot's winnings; round 0; state 3; the dealer's card `d = rand % 14 + 4` (4 to 17);
**`0x3104 d`**. Cards run from 0 to 21.

**`0x3107 guess`** (state 3 only): the player's card (`GetRandCardUser`): `r = rand % 1000`; with **`r < CHANCE[round]`** (the original
`[400, 200, 100, 50, 10]` per mille for rounds 0 to 4, `g_MiniGameRandCard` 0x531df8) the card falls on the guessed side: high (`guess` ≠ 0)
`rand % (22 − d) + d` (d to 21), low `rand % (d + 1)` (0 to d); otherwise on the other side: high `rand % (d + 1)`, low `rand % (22 − d) + d`.
**The guess wins when the card is on its side and is not `d`** (a card equal to the dealer's loses).

- **Won**: round + 1; **stake = stake × round × 2** (×2, ×4, ×6, ×8, ×10: after five wins 3840 × the first stake); record `0x0D05` (round,
  1, stake). After the **fifth** win: the stake to the money (capped, saved), record `0x0D03` (+stake), state 1, bet/lines/pending 0,
  **`0x3107 card, 2, stake`**. Before it: state 4, **`0x3107 card, 1, stake`**.
- **Lost**: **a fifth of the stake comes back**: `back = (stake as f32 × 0.2f32)` truncated; record `0x0D05` (round, 0, back); the money +
  back (capped, saved), record `0x0D03` (+back); state 1, bet/lines/pending 0; **`0x3107 card, 0, back`**.

**`0x3108`** (state 4 only): a new dealer card `rand % 14 + 4`; state 3; **`0x3108 d`**.

**`0x3109`** (state 4 only): the stake to the money (capped, saved), record `0x0D03` (+stake); bet/lines/pending 0; state 1; **`0x3109
u32 money`**.

Standing up in states 3 and 4 takes the stake as `0x3109` does (section 3). **The return per round** with the original chances (before the
ties): round 1 92 %, round 2 96 %, round 3 78 %, round 4 59 %, round 5 30 % of the stake risked.

## 6. The new server before the build (see section 12 for what is built)

Type-5 NPCs are skipped (`world.rs` `World::build`, `UNPLACED_NPC_TYPE`, as `CMap::InitNPC`); no `0x31xx` is handled (unknown packets are
logged and ignored). A player out of the zone waits for `0x0201` (`WorldState.entered` false; `server.rs` `OP_USER_ENTER if
!state.entered`), as after a move.

## 7. The game log records

Confirmed, `CMiniGame::SendGameLog*`, `ps_gameLog.exe` `CConnection::WriteLogGamble` 0x404a40.

Action numbers and columns (`docs/original/game-log.md` section 3.9; user id, user name, character id, character name in every row; level
and experience 0):

| record | action | sent by | columns |
|---|---|---|---|
| `0x0D01` | 190 | `GameEnter` | the map and position; V1 money, V2 the table + 1, V3 the table's bet |
| `0x0D02` | 191 | `GameExit` | no map or position; V1 money |
| `0x0D03` | 192 | `SendGameLogChgMoney` | V1 money after, V2 the change (signed: −bet, +winnings, +stake, +back) |
| `0x0D04` | 193 | `SendGameLogSlotRun` | V1 lines, V2 bet, V3 lines won, V4 winnings |
| `0x0D05` | 194 | `SendGameLogCardRun` | V1 round, V2 1 won / 0 lost, V3 the stake (or what came back) |

`0x0D03`'s V1 is the money when the record is made: after the change it names for winnings, stakes and what comes back; for a spin's −bet
(the bet was paid at `0x3102`), before the automatic re-bet is paid.

## 8. Original oddities

- **The NPCs are never created**, so the game is unreachable in the original (section 6); the client's window is reached only by a
  modified server.
- **No check at all on sitting down** beyond "in the world" (section 3): the table index is the client's choice.
- **The automatic re-bet** after a loss pays the next spin at once; taking it back is `0x3102` again (result 2).
- **Standing up during the card game takes the stake** without the risk of the next guess (section 3).
- **`pending` stays non-zero after `0x3105`'s re-bet** (it marks the re-bet as paid; harmless).
- `rand()` modulo 10000, 226 and 1000 is slightly uneven in the original (`rand` gives 0 to 32767); the new server draws evenly.

## 9. Design decisions

1. **Built**: `mini_game` places the type-5 NPCs (all 6 placements) and handles `0x31xx`; false keeps them hidden as the original.
   Default **true**, with the original's odds (decision 2): an addition that is on unless the operator turns it off
   (`docs/server/deviations.md` section 1). Play test section 52 checks the client; if it misbehaves, set the setting to `false`.
2. **Fairer odds, still a sink**, as settings:
   - **`slot_payout_percent`** (default **100**, the original's; the fairer value is **220**): each winning line pays `multiplier × table bet × percent / 100` (truncated): about
     **90 %** of the bet back on average instead of 41 % (100 is the original).
   - **`card_win_chances`** (default the original's **`400,200,100,50,10`** since R1; the fairer value **`420,200,130,100,80`** per mille for rounds 1 to 5): each round
     gives back about 95 % of the stake risked (a little less, since a tie still loses). The chances must fall from round to round because a
     win multiplies the stake by 2 × the round.
3. **Safety checks**: `0x3101` answers 0 (and nothing changes) while the player is **dead, trading** (a trade window open), **has a personal
   shop open, is in a duel or a guild war**, or when **no type-5 NPC of the zone is within its view** (the 5 × 5 cells), and for a table
   above 6. A `0x0201` that arrives while a game is still open first stands the player up (section 3, the money handed back), then enters
   the zone as usual.

## 10. Build plan

The plan the code was written from; the code is authoritative where it differs. Everything is in `crates/game-server` unless said.

1. **Protocol** (`crates/shaiya-proto/src/mini_game.rs`, `pub mod mini_game;`): the opcodes `OP_SIT 0x3101`, `OP_BET 0x3102`,
   `OP_SPIN 0x3103`, `OP_DOUBLE 0x3104`, `OP_AGAIN 0x3105`, `OP_STAND 0x3106`, `OP_GUESS 0x3107`, `OP_NEXT 0x3108`, `OP_STOP 0x3109`
   and one builder per answer of section 2 (`sat(ok)`, `bet(result, bet, money)`, `spin(ok, cells, won, winnings, short)`, `dealer(op,
   d)`, `again()`, `stood(money)`, `guessed(card, result, amount)`, `stopped(money)`), with byte-exact tests of their sizes (3, 11, 18, 3,
   2, 6, 8, 6).
2. **Settings** (`config.rs`, README table): `mini_game` (bool, default true), `slot_payout_percent` (u32 1 to 1000, default
   220), `card_win_chances` (five numbers 0 to 1000, default `420,200,130,100,80`).
3. **`mini_game.rs`** (new, pure; `pub mod mini_game;`): the tables of sections 3 to 5 as constants (bets, symbols, class thresholds, lines,
   `NO_BINGO`, `BINGO`, `START`, `COUNT`); `pub struct Odds { payout_percent: u32, chances: [u32; 5] }` (from the config;
   `Odds::ORIGINAL` = 100 and the original chances, for tests); `pub struct MiniGame { state, table, bet, lines, pending, round, dealer }`
   (`pub fn sit(table) -> Option<MiniGame>`) and one method per packet taking `money: &mut u32` and a random source `rand: &mut impl
   FnMut(u32) -> u32` (a number in `0..n`), returning `Outcome { answer: Option<Vec<u8>>, money_changed: bool, records: Vec<Record> }`
   where `Record` is the 0x0D03/0x0D04/0x0D05 data of section 7; `pub fn stand(&mut self, money: &mut u32) -> Stood` (what `GameExit` hands
   back and its `0x0D03`). Money adds saturate at `u32::MAX` (`ChkAddMoney`). The slot's payout uses `odds.payout_percent`; the card's
   chance `odds.chances[round]`. Unit tests with a scripted `rand`: each transition of sections 3 to 5 and the wrong-state silences; bets up
   to 8 lines and the refusal without money; a losing spin forms no counted line (all 32 patterns × a sample of draws) and never shows `s`;
   each class's patterns form exactly `c` lines over all 8 (loop over `BINGO`); class 7 → 6; class 8 fills the nine cells; only the first
   `lines` lines pay; the payout with 100 and 220 percent; the re-bet paid and not paid (`short`); `0x3105` with and without money; the card
   round chances, the stake doubling to ×3840, the fifth-win end, the fifth back on a loss (`(stake as f32 * 0.2) as u32`); `0x3109`;
   `stand` in each state.
4. **The NPCs** (`world.rs` `World::build`): a parameter (or a `World` setting) so type 5 is placed when `mini_game` is on; adjust
   the test at ~568 (`row(1, 5, 1, …) // type 5 is never placed`) to both cases. Object ids are numbered in placement order, so the ids of
   the NPCs after them shift by one per gambling NPC: check nothing stores NPC object ids across restarts (nothing should).
5. **The connection** (`server/mini_game.rs`, new; `WorldState.mini_game: Option<MiniGame>`):
   - `0x3101` (only when `state.entered`, not dead): the checks of decision 3 (the trade, personal shop, duel and guild-war states the
     connection or the zone already keeps; the NPC: a new `Zones::npc_of_type_in_view(map, id, 5) -> bool` or the existing NPC lookup);
     `sit`; leave the zone the way a move within the map does (find the function `0x020B`-free moves use: the zone's leave plus
     `state.entered = false`, keeping `map` and `position`); record `0x0D01`; answer `0x3101 1`. Any refusal: `0x3101 0`, nothing changed.
   - `0x3102` to `0x3109`: only while `state.mini_game.is_some() && !state.entered` (the original's state 3; in the world they stay
     unhandled as now); call the method, apply `money` to the character, **save it at once** as other money changes do (e.g. the way `/set
     money` saves through `ctx.store.trade(id, delta, &[])`; read it first), write the records, send the answer. `0x3106`: `stand`, the
     money saved, `0x0213` then `0x3106`, record `0x0D02`, `mini_game = None` (the player stays out of the zone until `0x0201`).
   - `0x0201` with a game open: `stand` first (money, `0x0213`, records; no `0x3106`), then the existing entry.
   - **Leaving** (logout, lost connection, kick, shutdown): before the character is saved on leaving, `stand` and add the money back
     (find the leave path all of these share).
   - What else arrives while at the table (`!entered`) keeps today's handling for a player between zones.
6. **Game log** (`game_log.rs`): the five constructors of section 7 (`gamble_sit`, `gamble_stand`, `gamble_money`, `gamble_spin`,
   `gamble_card`) with their action numbers, and unit tests of their columns as section 3's other records have; remove "the mini-games'
   records" from `docs/original/game-log.md` sections 3.9, 10 and 11's not-built line (point to this document).
7. **Tests** (`tests/game_flow.rs`, memory backend): a player standing next to a type-5 NPC (place one in the test world) sits at table 0
   and gets `0x3101 1`, bets three lines (`0x3102 1` three times, money down by 300), spins (`0x3103` with `ok` 1, 18 bytes), stands up
   (`0x0213` and `0x3106` with the money, the winnings or the re-bet included), sends `0x0201` and is in the zone again (others see it);
   far from the NPC `0x3101 0`; a lost connection at the table saves the money with the bet handed back.
8. **Docs**: a "Built" section 12 here; `docs/original/maps-npcs.md` section 4 (type 5 placed with `mini_game`);
   `docs/original/ps_game.md`'s `m_nWhere` 3 row (the mini-game); README (settings, test count; the gambling house in the server's list);
   live test **section 52** of `docs/testing/live-test-checklist.md`.

## 11. Play test (section 52)

A character with some gold (e.g. 50 000) next to a gambling NPC: map 1's (type id 1, 아스란 에페보스, the client's Asran Epebos), map 2's, or
the others of section 1. `docs/testing/live-test-checklist.md` section 52 is the full list (it adds the checks of section 13's last part).

- [ ] The NPC is drawn (its model and name). Clicking it opens the table selector (seven rows, 100 to 3000 per line).
- [ ] Sitting at a table: the character leaves the view of a second client standing by.
- [ ] Bet one line, spin: the reels stop on nine symbols; a win shows its amount. Bet up to eight lines: the ninth bet is refused; with too
      little gold the bet is refused.
- [ ] After a loss the next spin is already paid (the gold went down by the bet); taking it back (`bet` again) returns it.
- [ ] After a win: "again" adds the win and pays the next spin; or double up: the dealer's card shows; guess high or low; a win doubles,
      then ×4 …; stop takes the stake; a loss gives back a fifth.
- [ ] Standing up: the gold shown is right, the character is back in the zone (the second client sees it again).
- [ ] Logging out at the table, back in: the bet or winnings on the table were handed back (the gold).
- [ ] Sitting down while dead, trading or far from the NPC (a modified client only: else skip) is refused.
- [ ] `game_log` has rows 190 to 194 for the session.
- [ ] If any of this fails on the client's side (no window, the character stuck out of the zone after standing up), set
      `mini_game=false` and report what happened for research entry 6.

## 12. As built

Steps 1 to 7 as planned, with these points:

- **Protocol**: `shaiya-proto/src/mini_game.rs` (the sizes of section 2 count the opcode but not the length field; the tests use that).
- **`mini_game.rs`** (pure): the tables as constants and one method per request (`bet_line`, `spin`, `again`, `double_up`, `guess`, `next_card`,
  `stop`, `stand`) taking the money and a random source `FnMut(u32) -> u32` (a number below `n`), returning an `Outcome` (answer, money
  changed, records). Differences from the plan's wording: `stand` takes `&self`; the records are returned as `Record::{Money, Slot, Card,
  Exit}` (`Exit` is `0x0D02`); the card game's stake lives in `bet`, as in the original. The spin draws in the order class, symbol (classes
  1 to 8), pattern, nine cells, winning pattern (class 8 skips the last two). **Record choices the original leaves unclear:** `0x0D05`'s round
  counts the win when it won (1 to 5) and the wins so far when it lost; the `0x0D03` of a spin names the money before the automatic re-bet.
  Tests: every winning pattern forms exactly its class's number of lines over the 8; no losing spin forms a line or shows symbol 0 (400
  sweeps); the cell edge rules (10 wraps to 0, 21 to 11); each class from scripted draws, class 7 as 6, class 8; only the first `lines`
  pay; the payout at 100 and 220 percent; the re-bet paid and not paid (`short`); `again`; the card rounds to ×3840, the tie, the fifth
  back; `stand` in each state; and the **return**: 40 to 42 % at the original numbers and 88 to 93 % at 220 for 1 to 8 lines, computed
  from the tables.
- **Settings**: `mini_game` (default true), `slot_payout_percent` (1 to 1000, default 220), `card_win_chances` (five numbers
  0 to 1000), refused at start when out of range.
- **The NPCs**: `World::build_with(…, gambling)` (`build` keeps the original's rule), `pg::load_world(pool, gambling)`; the type constant is
  `world::GAMBLING_NPC_TYPE`. The `world` test places a type-5 row both ways.
- **The connection** (`server/mini_game.rs`): `0x3101` in the world only, with the checks of decision 3 (`Zones::may_gamble`: alive, no
  trade, no personal shop, no duel, no guild war; `Zones::npc_kind_in_view(…, 5)`); the zone is left by the new `leave_zone`
  (extracted from `move_player`, which uses it too). The other requests only with a game open and `entered` false; a request in the world
  or without a game is ignored. Money: `trade(id, change, &[])` after each request (one write for the change, `state.money` follows).
  `0x0201` with a game open stands up first (`stand_for_entry`: `0x0213`, no `0x3106`); the shared cleanup after `world_loop` stands up
  before the character is saved (`stand_for_leaving`: logout, lost connection, kick, shutdown all pass there). A failed database write at
  the table closes the connection (the game is kept, so the leave still hands the table back).
- **Game log**: `game_log::gamble_sit`, `gamble_stand`, `gamble_money`, `gamble_slot`, `gamble_card` (190 to 194), headers with level and
  experience 0, the map and position only on 190. `docs/original/game-log.md` sections 3.9, 10 and 11 point here.
- **Tests**: `game_flow`: sitting (a table above 6 refused, a seat, the others see the character leave), three lines bet and saved at
  once, a spin (a 16-byte payload: 18 bytes with the opcode), wrong-state requests answered by nothing, standing up (`0x0213` then `0x3106`, the same money, saved), `0x0201` and
  the zone again, the game log rows; `0x0201` with a game open; a lost connection at the table hands the bet back. The shared test world
  has the gambling NPC only through `data_with(true)` (`start_gambling`), so the other tests' NPC views are unchanged. `zone`:
  `may_gamble` (shop, dead, duel).
- **Not tested**: a trade or a guild war as the reason for a refusal (the same one-line check as the duel's); the NPC being too far
  (`npc_kind_in_view` has its own users); the client.
- **Unverified in the client:** everything of section 11 (the window, the reels, what happens on standing up).

## 13. The client's side (research entry 6)

From `docs/client/research-gambling-kill-boost.md` (static: Ghidra and objdump on `game.exe` US patch 108, the client's
data files read; nothing run). Compared claim by claim with sections 2 to 5 and the build (`shaiya-proto/src/mini_game.rs`,
`mini_game.rs`, `server/mini_game.rs`). **Every verified claim matches; nothing in the server changes.** A test now replays the client's
own arithmetic against the server (`mini_game::tests::the_clients_wallet_follows_the_servers_money`, below).

**Verified static, matching the build:**

- **The NPC**: `0x0E01` (reader 0x57e000, handler 0x552730) passes type 5 through; NPC construction 0x433770 handles type 5 with its
  name and model from `NpcQuest.SData` (type 5 has 8 records: Asran Epebos, Bete Margarita, Piedia Epebos, Quibos Raksion, Aldea Lwena,
  Igra, Yutikia Epebos, Iba, models present in the archive). The build places them with `mini_game` (section 12).
- **The click**: type 5's primary action opens a **table selector** (UI state `0x6a`, `gamble.tga`) of **seven rows**; the row sends
  **`0x3101 u8 table`** (sender 0x588c00, 3 bytes). The NPC's id is not sent: the server's "a type-5 NPC in view" check (decision 3) is
  all it can check. The client's per-line table (0x6f2150) is **100, 300, 500, 1000, 1500, 2000, 3000**, the same as `TABLE_BETS`.
- **The answers' fields** (section 2): all nine at the offsets and sizes the builders write (3, 11, 18, 3, 2, 6, 8, 3, 6 bytes with the
  opcode). The requests: only `0x3101` (table) and `0x3107` (1 high, 0 low) carry a byte, as `parse_sit` / `parse_guess` read.
- **The cells and cards**: the nine cells row by row (0 1 2 / 3 4 5 / 6 7 8), revealed 0 to 8; value `v` is drawn as face `v` of
  `game_card%d.tga` (reels) or `tarocard%d.tga` (cards), 0 to 21 the 22 major arcana in order, **22 the back**. The server sends 0 to 21
  only (cells, the dealer's 4 to 17, the player's card 0 to 21). The client highlights winning lines in the same order as `LINES` and
  shows the server's amount (it computes no payout), so `slot_payout_percent` shows correctly.
- **`short`** is a "no re-bet" flag: after a losing spin, or after `0x3105`, the window keeps the bet and takes it from its own wallet when
  `short` is 0, and clears the lines and bet when it is not. The server's rule (section 4 steps 6 and 7, `0x3105`) is the same condition.
- **The gold**: the window copies the money on sitting down and takes it from the server only at **`0x3102` result 1** and **`0x3106`**;
  `0x3102` result 2, `0x3103`, `0x3105`, `0x3107` and `0x3109` change it by the window's own bet, the answer's amount, `short` and its own
  card round. The server's money follows the same steps, so they agree (the new test checks this over 400 random sessions, half of them
  with certain card wins so that the fifth round is reached). `0x3109`'s money and `0x3107`'s result 2 are kept as the original's: harmless.
- **The 30-second timeout**: the window waits for the answer it expects (0x485af0) and shows a timeout box after 30 s. The server answers
  every request the window can send in its state at once (the test also asserts that); only a request in the wrong state (a modified
  client) gets no answer, as in the original.
- **Standing up**: after `0x3106` the scene copies the window's money to the normal money, reloads the current map (scene 7) and sends
  **`0x0201`** (0x42fa77), as the build expects (`state.entered` false until then; `0x0213` before `0x3106` kept).
- **While seated**: the gambling scene runs no world movement (camera only) and has no chat input; packets are still received.
  Nothing to change: the server ignores moves while out of the zone, and the client sends no chat there.

**Not read (live test section 52):** whether the window clears its slot bet when the card game ends (the server does; the test's model
assumes the window does too), the NPC's look and click distance, the labels, the map reload and the reappearance to another player, and
what chat from others does on the gambling screen.

**Design notes of the research, not acted on:** none needed; it suggests keeping `0x0213` before `0x3106` and the 6-byte `0x3109`, as built.
