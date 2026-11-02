# Guild wars ("GvG", `25xx`): request, countdown, members, result

This page covers the guild-against-guild battle of the client `game.exe` (US build 2010-04-26, patch 108). It was read from `game.exe`
(handlers on the network object's vtable `+0x4d8`…`+0x4ec`, state machine `0x420c20`, results `0x420af0`, member events `0x420930` /
`0x421210`) and from the original EP4.5 server `ps_game.exe` (`CUser::PacketPVPGuild` `0x47da70`, `CUser::PvPChkLeaveByTime`
`0x47c210`, `CGuild::PvPUserLeave`). Not to be confused with the guild-war **zone** state `0x0D19` / member state 106 (see
[Social packets](client-social-packets.md#guild-0x0dxx)). Status words: **confirmed**, **confirmed (disasm)**, *inferred*, **live**.
Sizes include the opcode.

## In short

- **Two guild officers agree to a war**: one asks, the other has 10 seconds to accept.
- **Only the members near the war's centre take part**: those within 40 units of the centre when the war is accepted. Members who arrive
  later never join.
- **After a countdown the fight starts.** Members drop out when they die or leave the area, the map or the guild.
- **A guild loses** when its last player is out or its officer leaves; the original server never declares a draw and sets no time
  limit.

## Summary

- Flow (original, confirmed): an officer sends `0x2501 targetId` → both officers get `0x2501 (requester, target)`, 10 s → the target
  answers `0x2502` → the requester gets `0x2502 (1, target)` and **the members of both guilds within 40 of the centre when the war is
  accepted** (the war's players) get **`0x2503 (f32 x, f32 z, u32 enemyGuildId)`** (each guild receives the other's id), 5 s →
  `0x2504` to the same players: fight → `0x2505` result / `0x2506` member events. (From a reading of
  `ps_game.exe`; see [Guild wars on the server side](../original/guild-war.md).)
- Who may ask (original): both players in a guild with guild rank **< 5** (`CUser +0x174c`, *inferred*: master and officers), the
  guilds different and not already in a war, neither player in a duel. Failure → `0x2502 (2, target)`; timeout → `0x2502 (3, …)`;
  refusal → `0x2502 (0, target)`. confirmed
- The client marks enemy-guild players in view as in the war when they are within **40.0** of the centre (`0x6a6690`, player
  `+0x37b`), and warns once with 5309 when the local player is farther than **35.0** (`0x6a67b8`). confirmed
- The enemy guild's **name** in the result comes from the client's guild list (`0x420aa0(guildId)`): if that guild is not in the
  list, the "<b> guild won." line is skipped. confirmed

## Packets

| op | dir | size | fields | client |
|---|---|---|---|---|
| `0x2501` | C→S | 6 | `u32 targetId` | sender `0x589400` |
| `0x2501` | S→C | 10 | `u32 requesterId, u32 targetId` (`TP_PVPG_REQ`) | only with no dialog open: I am the requester → dialog 9; the target → dialog 8 with 5340 "<p> requested the battel to <t>." (sic); ignored if either player has `+0x250` set; `ui_party001.wav`. No automatic refusal when busy (unlike duels) |
| `0x2502` | C→S | 3 | `u8 answer` (0 refuse, else accept) (`TPC_PVPG_ANS`) | dialog 8 |
| `0x2502` | S→C | 7 | `u8 result, u32 id` (`TP_PVPG_ANS`) | only while dialog 0/8/9: 0 → 5341 "<p> rejected GvG." (5342 if `id` is not in view); 2 → 5343 "can't request GvG to <p>." (5344 if not in view); 3 → `id` me: closes silently, else 5345 "GvG is canceled as the opponents didn't respond."; 1 and others: closes silently. Errors play `ui_error001.wav` |
| `0x2503` | S→C | 14 | `f32 x, f32 z, u32 enemyGuildId` | GvG on (`0x4215a0`): area centre, enemy guild id (`0x704494`), result queue cleared, enemy players in view marked (40.0); 6-second countdown with 5308, `bgm_vs.wav` |
| `0x2504` | S→C | 2 | — | fight: 5316 "Start GvG!!" |
| `0x2505` | S→C | 3 | `u8 result` | queued, shown after a short delay (`0x6a62d8`), then GvG ends: **1** "<own guild> guild won." (5317), **2** "<enemy guild> guild won." (5317), **3** "a tie game" (5312); 0 or others end it silently |
| `0x2506` | S→C | 7 | `u8 event, u32 id` | **0** 5331 "<p> disconnected."; **1** `id` me: 5332 "out of allowed battle area." and GvG ends for me, else 5333 "<p> is out of allowed battle area." and the mark is cleared; **2** `id` me: 5318 "give up GvG.", state 13, ends for me; else 5319 "<p> gave up GvG."; **3** `id` me: 5337 "have been defeated at the battle", GvG ends for me; else 5338 "<p> has been defeated at the battle." (mark cleared). `0xFF` ignored |

There is no client "give up" packet for GvG (no `/defeat` equivalent; `0x2406` is duel-only). Defeat (event 3) is per member:
*inferred* a dead member is out of the war.

## For a server

- Send `0x2503` and `0x2504` to the war's players only (the members within 40 of the centre at acceptance; later arrivals never
  take part), each with the **other** guild's id.
- What the original sends (read in `ps_game.exe`; see [Guild wars on the server side](../original/guild-war.md#6-leaving-a-war-and-its-end)): `0x2505` only **1** or **2** (no tie, no time limit; a guild loses
  when its last player is out or its officer leaves the area, map or guild), to everyone who took part; `0x2506` only events **1**
  (out of the area, also leaving the map or the guild) and **3** (any death). The client's 0, 2 and the 3 "tie" are never used by
  the original.
- Make sure both guild names are in the client's guild list before the result, or the winner line is not printed.
- Enemies: players of the two guilds are enemies once both guilds are "ready" (`IsTeam`, see [Player kills](../original/pvp.md#1-enemies)).
- Area: enforce it on the server and send `0x2506 1`.

## Open

- **Live**: the result delay.
