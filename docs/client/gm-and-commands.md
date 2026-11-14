# GM opcodes and chat commands

This page covers every GM opcode (`F1xx`–`FAxx`) the client `game.exe` (US build 2010-04-26, patch 108) sends or handles: the chat
command or window behind each, what the client does with the answers, and what the original EP4.5 server `ps_game.exe` does with each.
Then the player chat commands and prefixes, and how new commands could be added with and without a client change (design only).

Sources: Ghidra decompiles of `game.exe` and of `ps_game.exe` (with the function names from its debug symbols, `ps_game.pdb`), and the
message texts of `sysmsg-uni.txt`. Status words: **confirmed** (decompiled), **confirmed (disasm)** (disassembly only), **ref** (another
page), [*inferred*](glossary.md#inferred), [**live**](glossary.md#live) (needs a test; nothing here was run). In the tables, **handled** = the original server has a case for the
opcode; **client-only** = the client sends or reads it but the original never sends it or has no case for it. Sizes include the 2-byte
opcode; `name[21]` is a zero-padded character name; `len` bytes of text include the terminating zero unless noted.

## In short

- **The client decides who is a GM** from one number the login server sends, the privilege: below 11 is staff, and 1 is the highest.
  Each GM command needs a privilege at or below its own level (1 to 5).
- **GM commands are typed in chat** (`/kick`, `/notice`, `/amove` …), in English or Korean. The client checks the level, builds the
  packet and shows a confirmation when the server answers "done".
- **The original server is strict**: a GM command it does not know, or one above the GM's level, closes the GM's connection. Ten commands
  the client offers (`/ban`, `/aban`, `/pvpon` and others) have no case in the original, so using them disconnects the GM.
- **Players' unknown `/` commands are silently dropped** by the client; only text without a leading `/` reaches the server intact. A
  server that wants its own commands can use another prefix such as `.cmd`, or patch two jumps in the client.

### Using this page

- **If you write a server:** the tables of F1xx to FAxx give what each GM command sends and which privilege it needs. The original closes the GM's connection on a command it does not handle (third bullet above); decide what yours does. The checklist item for the two auction commands is in [What a server must do for this client](afterglow/server-checklist.md#auction-house).
- **If you play with a GM account:** type the command in the chat box with a leading `/` (for example `/kick Name`). The client checks the privilege first and prints "Unusable command" if it is too low. "Client-only GM commands" lists what is typed but never sent.
- **If you patch or mod the client:** "Adding commands" has the two ways (a prefix the client already passes through, or a client change); the fixes for five broken English commands are in [the companion DLL](fixes-dll.md#english-texts-and-gm-command-fixes).
- **To check a claim yourself:** start at the parser `0x475e00` and the privilege check `0x46d5a0`, using [Tools and method](tools-and-method.md).

## Summary

- **Who is a GM, client side**: the third byte of the login answer `0xA102` (`u8 status, u32 user, u8 privilege, …`) is stored at
  `0x84d668`; the client treats the player as staff when it is **below 11** (`0x84d66c = privilege < 11`, `0x55c6c0`). Each GM
  command then needs `privilege <= N` with N = 5, 4, 3, 2 or 1 (`0x46d5a0`; failure prints 254 "Unusable command for <p>'s GM
  rights" and sends nothing).
  confirmed
- **Who is a GM, original server**: `CUser+0x5768` (same scale). Every admin handler **closes the connection** (`SConnection::Close(9)`)
  when the level is too high: chat `F1xx` and `F2xx` need ≤ 10, `F6xx` ≤ 1, `F7xx` ≤ 2, `F8xx` ≤ 3, `F9xx` ≤ 4, `FAxx` ≤ 5. Any
  opcode a handler has no case for also closes the connection, as does any other `Fxxx` family (`CUser::PacketProcessing`). confirmed
- **Commands the original disconnects the GM for** (the client sends them, the original has no case): `F70A` /aban, `F70B` /abancan,
  `F805` /ban, `F806` /bancan, `F809` /queston, `F80A` /questoff, `F80F` /eventadd, `F810` /eventrem, `FA16` /pvpon, `FA17` /pvpoff.
  `F708` /shutdown answers an error only. confirmed
- **Different meaning**: the client's `/event on` (`F80D`) and `/event off` (`F80E`) mean "heal the named player" and "cure and clear
  buffs/debuffs of the named player" in this `ps_game.exe`. confirmed
- **Layout mismatch**: `/apower` sends `F601` as `name[21], u8 level`; the original reads `u8 level` at +2 and the name at +3. confirmed (disasm)
- **Catalogue corrections** (`packet-opportunities.md` GM table): `FA01`, `FA02`, `FA05`–`FA0B` are **handled** by `PacketAdminCmdE`
  (a jump table on `opcode − 0xFA01`, missed by the opcode scan); the client also sends **`FA12`** (`/gmove`, sender `0x5899e0`),
  **`FA19`** (`/auctionrecall`) and **`FA1A`** (`/auctionsearch`) (built inline in `0x4705d0`), which the catalogue lacks; the client never
  sends `F102`–`F109` as commands (`F102`–`F105` are a GM's ordinary chat, below). confirmed
- **Client bugs**: the English `/server off` sends "server on", `/cwatch off` watches a player named "off", `/eventadd`,
  `/eventrem` and `/partysummon` with arguments are never sent, and text arguments need double quotes (see
  [Bugs in the client's command parser](#bugs-in-the-clients-command-parser)). confirmed (disasm)
- **Hiding**: Stealth hides a player from the other faction only; an invisible GM is hidden from players and seen by staff (see
  [What hides another player](#what-hides-another-player)). confirmed
- **A player's unknown `/word` is dropped**: nothing is sent and nothing is shown (`0x4780c3`). A GM's unknown `/word` prints 253
  "Invalid command. Please check" and sends nothing. Text not starting with `/` goes to the server intact (see [Player chat commands](#player-chat-commands-and-prefixes)). confirmed (disasm)
- **Also**: `F501`'s failure value is the failing opcode for every command (all 46 call sites of
  `CUser::SendAdminCmdError` load it); the deferred moves are `op u32 charId, u16 map, f32 x, y, z` (20 bytes); `/cwatch` copies are
  `F502`, **which the stock client does not handle** (the [companion DLL](fixes-dll.md) adds a handler, [operator-listen.md](operator-listen.md)); `/autocure` only works against the developers' server `172.17.10.205`; `F202`/`F203`
  answers only go to the original's game log; `/xcall` summons a faction's players. [What the client sends at each privilege](#what-the-us-client-sends-at-each-privilege-110) lists what each privilege 1–10 can
  send. confirmed

## How a GM command travels

1. The chat box's Enter handler `0x475e00` runs the player commands first ([below](#player-chat-commands-and-prefixes)). A line starting with `/` that none of them
   took goes to `0x46eab0` when region feature 30 is on (it is for region 6: `/raid`, `/alert`, `/command`, `/chatcolor`), then, **only
   if `0x84d66c` is set**, to the GM parser `0x4705d0`, then to `/UQGET` / `/UQCOUNT`, else message 253. confirmed (disasm)
2. `0x4705d0` is an if-chain of `strncmp` against an English and a Korean spelling of each command (Korean literals `0x6aa0e4`–`0x6aa7f0`,
   CP949). Matches are **prefix** matches. It checks the level, parses the arguments (`sscanf`), queues a confirmation text and calls
   a sender `0x587130`–`0x5899e0`, which builds the packet and calls the send function `0x585640`. confirmed
3. The original answers almost every command with **`F501 u16`**: 0 = done (`CUser::SendAdminCmdSuccess` `0x4719f0`), otherwise the
   opcode that failed (`SendAdminCmdError` `0x4719c0`, value in `AX`; the call sites send `F701`–`F705`, `F708`, `F709`, `F70C`–`F70E`,
   `F801`–`F804`, `F80B`–`F80E`, `F901`, `F904`–`F90C`, `FA07`–`FA10`, `FA12`–`FA15`, `FA19`, `FA1A`; `F601`/`F602` in `PacketAdminCmdA`).
   confirmed. The client then prints the queued confirmation (0) or a failure text chosen by opcode (`0x554720`):

| `F501` value | message |
|---|---|
| 0 | the confirmation queued when the command was sent (the "echo" column below), as a type-0x14 line |
| `FA01` / `FA02` / `FA05` / `FA06` | 2010 "Transparent setting failed." / 2011 "Releasing transparent setting failed." / 2012 "Prohibiting attack failed." / 2013 "Release of attack prohibition failed." |
| `FA07` / `FA08` / `FA09`–`FA0A` | 2014 "Can't use warning function." / 2015 "Can't find the character." / 2016 "Can't move. Please check again." |
| `FA0B` / `FA0C` / `FA0D`–`FA0E` / `FA0F`–`FA10` | 2017 "Observation function failed." / 2018 "Quitting observation function failed." / 2019 "Can't use ban chatting function." / 2020 "Can't restrict the movement." |
| `F901` / `F904`–`F905` / `F906`–`F90B` | 2021 "Please check search commands again." / 2022 "Can't summon. Please check again." / 2023 "Can't post notice. Please check again." |
| `F801` / `F802`–`F803` / `F804`–`F806` | 2024 "Can't use healing function…" / 2025 "Please check emptying commands again." / 2026 "Can't restrict character-related issues." |
| `F809`–`F80C` / `F80D`–`F810` | 2027 "Can't use quest commands." / 2028 "Can't use event commands." |
| `F702` / `F703` / `F704` / `F705` | 2030 "Not authorized to create." / 2031 "Not authorized to collect." / 2032 "Not authorized to create." / 2033 "Not authorized to remove." |
| `F706`–`F708` / `F709`–`F70B` / `F70C` / `F70D`–`F70E` | 2034 "Please check the command again." / 2035 "Can't use function of restricting account." / 2036 "Not authorized to create." / 2037 "Not authorized to remove." |
| `F601`–`F602` | 2038 "Not authorized" |
| anything else (`F701`, `FA11`…) | 2029 "Can't change" |

## GM opcodes, client → server

"Lvl" is the client's check (`privilege <= Lvl`; "—" = any staff). "Orig" is `ps_game.exe`; its level limit is per family ([summary](#summary)).
Commands are given as English spelling / Korean spelling; arguments in `<>`, optional in `[]`. All confirmed (client decompile of
`0x4705d0` and the senders; server decompile of the `CUser::PacketAdmin*` handlers) unless marked.

### F1xx — GM chat (`CUser::PacketAdminChat`, level ≤ 10)

A GM's ordinary chat uses the `F1xx` opcode of the same chat kind (the send functions add `0xE000` when `0x84d66c` is set):

| op | from | wire | original | status |
|---|---|---|---|---|
| `F101` | plain text | `u8 len, text` (2–128) | needs the player in the world (`+0x5758 == 4`); `F101 u32 id, u8 len, text` to players in view; a GM being listened to (`/cwatch`) copies it to the listener; with the "message to server" flag (`+0x5840`) it goes out as `0x1108` to the map instead | handled |
| `F102` | `! name text` | `name[21], u8 len, text` | target must be online and of the **same faction**: `F102 u8 0, sender name[21], u8 len, text` to the target and `u8 1, target name` back; else `0x1106 01` | handled |
| `F103` | `$ text` | `u8 len, text` | `F103 name[21], u8 len, text` to **everyone** (`CWorld::SendAll`, both factions; the player `0x1103` is faction-only) | handled |
| `F104` | `# text` | `u8 len, text` | to the guild (`CGuild::SendAllGuildAdminChat`) | handled |
| `F105` | `@ text` | `u8 len, text` | `F105 u32 id, u8 len, text` to the party | handled |
| `F10A` | `/raid text`, `/alert text`, `/command text` (feature 30; `/command` skips only 6 characters, a client bug) | `u8 len, text` | only the leader or sub-leader of a raid party (`+0x114`): `F10A u32 id, u8 len, text` to the party | handled |
| `F106` | — (no sender) | `u16 len, text` (≤ 2000) | world notice | handled (client never sends) |
| `F107` / `F108` / `F109` | — (no sender) | `name[21]` / `u8 len, text` / — | a one-to-one "GM talk" session: `F107` opens it (both get `F107` with the other's name), `F108` talks (delivered as `F102`), `F109` ends (`CUser::SendAdminSendToEnd`) | handled (client never sends) |

`~ text` (zone, `0x1111`), `/yelling` (`0x1107`) and the player `/raid` (`0x1112`) have no GM variant; a GM sends the player opcode.

### F2xx — user questions (`CUser::PacketAdminUQ`, level ≤ 10)

| op | command | wire | original | answer in the client | status |
|---|---|---|---|---|---|
| `F201` | `/UQGET` (exact) | — | `CUserQuestionMgr::GetQuestion` → `F201` | `u16 n` (0 = none), `u32, u32, name[21], u16 len, text`: stored in the question list, window flag set (`0x554ed0`) | handled |
| `F202` | question window `0x496000`: the first three buttons (sender `0x5871c0`) | `u8 kind (1–3, the button), u32 questionId (the listed question's id), u16 n, text[n] + 0` (n ≤ 1000) | `CUserQuestionMgr::AddAnswer`: removes the question and writes a game-log record `0x0402` (GM name, kind, text); **nothing is sent to the player** | — | handled |
| `F203` | question window, fourth button ("answer to a player": a name field) (sender `0x587230`) | `u32 questionId, char name[21], u16 n, text[n] + 0` | `CUserQuestionMgr::AddAnswerSend`: removes the question, game-log record `0x0403` (GM name, target name, text); nothing sent in game | — | handled |
| `F204` | `/UQCOUNT` (exact) | — | `F204 u16 count` | 344 "Remaining number of QUEUE : <v>" | handled |

The questions come from players' `/request GM <text>` (`0x1201`, see [Player chat commands](#player-chat-commands-and-prefixes)).

### F6xx — staff levels (`PacketAdminCmdA`, level ≤ 1)

| op | command | Lvl | wire | original | status |
|---|---|---|---|---|---|
| `F601` | `/apower <level> <name>` / `/권한부여` | 1 | `name[21], u8 level` (24) | expects **`u8 level, name[21]`**: sets the target's level, tells the DB agent, `F601 u8 level` to the target, `F501` to the GM (`F501 F601` when not found). With this client the name is read one byte late and the level is the name's first byte. confirmed (disasm) | handled (layout differs) |
| `F602` | `/apowercan <name>` / `/권한취소` | 1 | `name[21]` | level 11, tells the login server (`0x0305`), `F602 u8 11` to the target | handled |

The target's client: `F601` sets `0x84d668` and prints 342 "Get authorization as an operator of <v> level."; `F602` prints 343. Neither
updates the staff flag `0x84d66c`, so GM commands start (or stop) working only after the next login. confirmed

### F7xx — characters, items, monsters, NPCs, server (`PacketAdminCmdB`, level ≤ 2)

| op | command | Lvl | wire | original | echo | status |
|---|---|---|---|---|---|---|
| `F701` | `/set <name> <stat> <value>` / `/셋팅` | 2 | `u8 stat, u32 value, name[21]` (28) | sets the stat of a player in the zone and sends `F701 u8 stat, u32 value` to them, plus the matching packets (`0x0610` stats, `0x0605` HG…IG, `0x0522` kills; level also `F701 1`, exp may add `F701 1` for the new level). Limits: grow ≤ 3 (Easy only up to level 30), level ≤ 60 (≤ 30 in Easy), stats ≤ 0x6FFF, HG ≤ 0xFFFF, CG/OG/IG ≤ 256, exp ≤ the table, kills/deaths clamped | 294 | handled |
| `F702` | `/imake <item name> [count]` / `/생성`, `/getitem <type> <id> [count]` | 2 | `u8 type, u8 typeId, u8 count` | creates the item in the **GM's** bags | 295 | handled |
| `F703` | `/recallitem <name> <item name>` / `/리콜아이템` | 2 | `name[21], u8 type, u8 typeId` | deletes that item from the player (same zone) | 297 | handled |
| `F704` | `/mmake <mobId> [count]`, `/몹생성 <mob name> [count]` | 2 | `u16 mob, u8 count` (count ≤ 20) | spawns at the GM (`CZone::MobGen`) | 295 | handled |
| `F705` | `/mera <mobId> [count]`, `/몹삭제 <mob name> [count]` | 2 | `u16 mob, u8 count` | removes mobs of that kind near the GM | 299 | handled |
| `F70E` | `/mera t` / `/몹삭제 t` (targeted mob) | 2 | `u32 mob object id` | removes that mob | 299 | handled |
| `F70C` | `/nmake <type> <id> [count]`, `/NPC생성 <npc name> [count]` | 2 | `u8 type, u16 id, u8 count` | creates the NPC at the GM | 295 | handled |
| `F70D` | `/nera <type> <id>`, `/NPC삭제 <npc name> [count]` | 2 | `u8 type, u16 id, u8 count` | removes it | 299 | handled |
| `F706` | `/server on` / `/서버제한취소` | 2 | — | `SAcceptor::StartAcception`, tells the login server (`0x0204`) | 326 | handled |
| `F707` | `/server off` / `/서버제한` | 2 | — | `StopAcception` (no new logins), tells the login server. **The English `/server off` sends `F706` instead** ([client bugs](#bugs-in-the-clients-command-parser)) | 327 | handled |
| `F708` | `/shutdown` / `/서버다운` | 2 | — | `F501` error only (disabled) | 328 | handled |
| `F709` | `/akick <name>` / `/유저킥` | 2 | `name[21]` | `CWorld::FindUserBill` (by account name, *inferred*) → closes that connection | 289 | handled |
| `F70A` | `/aban <minutes> <name>` / `/유저밴` | 2 | `u16 minutes, name[21]` | **no case: closes the GM's connection** | 290/291 | client-only |
| `F70B` | `/abancan <name>` / `/유저밴취소` | 2 | `name[21]` | **no case: closes the GM's connection** | 292 | client-only |

`/set` stat words (`0x46d7b0`, case-insensitive): GROW 0, LEVEL 1, MONEY or 돈 2, STATPOINT 3, SKILLPOINT 4, STR 5, DEX 6, REC 7,
INT 8, LUC 9, WIS 10, HG 11, VG 12, CG 13, OG 14, IG 15, EXP 16, KILL 17, DEATH 18; anything else prints 293 "Wrong stat name".
Name lookups that fail locally print 296 "<i> doesn't exist", 298 / 324 (monster), 325 (NPC), 323 "No monster currently targeted".

### F8xx — players (`PacketAdminCmdC`, level ≤ 3)

| op | command | Lvl | wire | original | echo | status |
|---|---|---|---|---|---|---|
| `F801` | `/cure [name]` / `/치료` (default: own name) | 3 | `name[21]` | clears skill cooldowns and type-3 effects (*inferred*: debuffs), full recover, on a player in the GM's zone (the GM when not found) | 282 | handled |
| — | `/autocure`, `/noautocure` / `/자동치료`, `/자동치료해제` | — | — | client-only toggle (no level check): while on, the chat loop sends `F801` with the GM's own name **every 100 ms** and clears its own cooldowns locally, **but only when the client's server address (`0x701970`) is `172.17.10.205`** (`0x4c2b10`, the developers' server); anywhere else nothing is sent. confirmed (disasm `0x475e17`) | — | client-only (uses `F801`; dead off that server) |
| `F802` | `/iclear <name>` / `/인벤청소` | 3 | `name[21]` | empties the bags; `F802` (2 bytes) to the player | 283 | handled |
| `F803` | `/eclear <name>` / `/장착청소` | 3 | `name[21]` | empties the equipment; `F803` to the player | 284 | handled |
| `F804` | `/kick <name>` / `/킥` | 3 | `name[21]` | closes the player's connection | 289 | handled |
| `F805` | `/ban <minutes> <name>` / `/밴` | 3 | `u16 minutes, name[21]` | **no case: closes the GM's connection** | 290/291 | client-only |
| `F806` | `/bancan <name>` / `/밴취소` | 3 | `name[21]` | **no case: closes the GM's connection** | 292 | client-only |
| `F809` | `/queston <quest name>` / `/퀘스트오픈`, `/queston <id>` | 3 | `u16 quest` | **no case: closes the GM's connection** | 259 | client-only |
| `F80A` | `/questoff …` / `/퀘스트다운` | 3 | `u16 quest` | **no case: closes the GM's connection** | 260 | client-only |
| `F80B` | `/questadd <name> <id>`, `/퀘스트추가 <name> <quest name>` | 3 | `name[21], u16 quest` | `CUser::QuestAddAdmin` | 261 | handled |
| `F80C` | `/questrem <name> <id>`, `/퀘스트삭제 …` | 3 | `name[21], u16 quest` | `CUser::QuestRemoveAdmin` | 262 | handled |
| `F80D` | `/event on <event>` / `/이벤트시작` | 3 | `char event[21]` | **heals** the player of that name (else the GM) and whispers a fixed text (`UserSendChatTo`) | 285 | handled (other meaning) |
| `F80E` | `/event off <event>` / `/이벤트종료` | 3 | `char event[21]` | cure + clear all effects of that player; whispers "cure, buff,debuff clear ok" / "… failed" | 285 | handled (other meaning) |
| `F80F` | `/eventadd <name> <event>` / `/이벤트추가` | 3 | `name[21], event[21]` (44) | **no case: closes the GM's connection**; the English `/eventadd` with arguments is never sent ([client bugs](#bugs-in-the-clients-command-parser)) | 287 | client-only |
| `F810` | `/eventrem <name> <event>` / `/이벤트삭제` | 3 | `name[21], event[21]` | **no case: closes the GM's connection**; the English `/eventrem` with arguments is never sent (as `/eventadd`) | 288 | client-only |

Quest names are resolved from the client's own quest data (`0x479630`); an unknown one prints 258 and sends nothing.

### F9xx — information, summons, notices (`PacketAdminCmdD`, level ≤ 4)

| op | command | Lvl | wire | original | echo | status |
|---|---|---|---|---|---|---|
| `F901` | `/quiry <name>` / `/조회` | 4 | `name[21]` | `F901` (mode, level, exp, money, points, six stats, HG/VG/CG/OG/IG, then up to 13 equipped items × 32 bytes) + `F501` | 278 | handled |
| `F904` | `/asummon <name>` / `/캐릭이동` (sends `FA08` first) | 4 | `name[21]` | moves the player to the GM (deferred move, see `FA09`) | 279, 280 | handled |
| `F905` | `/bsummon <name> <x> <z> [map]` / `/캐릭존이동` (sends `FA08` first) | 4 | `name[21], f32 x, f32 z, u16 map` (33) | moves the player there (map bounds checked) | 279, 281 | handled |
| `F90C` | `/partysummon <name> <x> <z> [map]` / `/파티이동` | 4 | as `F905` | moves the player's whole party. The English `/partysummon` with arguments is never sent ([client bugs](#bugs-in-the-clients-command-parser)) | 281 | handled |
| `F906` | `/gmnotice <text>` / `/운영자공지` | 4 | `u8 len, text` | `F906` to every staff member (`CUserQuestionMgr::SendAdminAll`) | 257 | handled |
| `F907` | `/cnotice [seconds] <text>` / `/챗공지`; off: `/챗공지해제` | 4 | `u16 repeat, u8 len, text` (off = `0, 0`) | `F907` to players in view; repeated every `repeat` s from the GM's notice slot (`CUser::UpdateAdminNotice`) | 257 / 2000 | handled |
| `F908` | `/wnotice <name> [seconds] <text>` / `/귓공지`; off: `/귓공지해제` | 4 | `name[21], u16 repeat, u8 len, text` | `F908` to that player, repeated | 257 / 2000 | handled |
| `F909` | `/znotice [seconds] <text>` / `/맵공지`; off: `/맵공지해제` | 4 | `u16 repeat, u8 len, text` | `F909` to the zone, repeated | 257 / 2000 | handled |
| `F90A` | `/bnotice <빛\|어둠> [seconds] <text>` / `/종족공지`; off: `/종족공지해제` | 4 | `u8 faction (0 light, 1 dark), u16 repeat, u8 len, text` | `F90A` to that faction (`CWorld::SendAllCountry`), repeated. The faction word must be the Korean 빛 or 어둠 (else 256) | 257 / 2000 | handled |
| `F90B` | `/notice [seconds] <text>` / `/공지`; off: `/공지해제` | 4 | `u16 repeat, u8 len, text` | `F90B` to everyone, repeated | 257 / 2000 | handled |

For `F907`–`F90B` a length below 2 stops that notice's repeat (success) and above 128 is an error; the repeat needs the GM's notice slot (`+0x5788`, allocated
for staff, *inferred*). `[seconds]` is an optional leading number (`0x46d720`).

### FAxx — GM self, observation, bans, guilds, auction (`PacketAdminCmdE`, level ≤ 5)

| op | command | Lvl | wire | original | echo | status |
|---|---|---|---|---|---|---|
| `FA01` | `/char on` / `/투명설정` | ≤ 5 | — | `+0x5774 = 0`, resends the GM's shape (transparency) | 263 | handled |
| `FA02` | `/char off` / `/투명해제` | 5 | — | `+0x5774 = 1`, resends the shape | 264 | handled |
| `FA05` | `/attack on` / `/공격설정` | 5 | — | `+0x5775 = 0` ("Won't be attacked anymore") | 265 | handled |
| `FA06` | `/attack off` / `/공격해제` | 5 | — | `+0x5775 = 1` | 266 | handled |
| `FA07` | `/warning <name> <text>` / `/경고` | 5 | `name[21], u8 len, text` | `FA07 u8 len, text` to the player | 270 | handled |
| `FA08` | `/watch <name>` / `/감시` | 5 | `name[21]` | `FA08 u16 map, f32 x, y, z` to the GM | 271 | handled |
| `FA09` | `/amove <name>` / `/이동` | 5 | `name[21]` | moves the GM to the player, deferred: `+0x5818` holds the opcode and `CUser::UpdateResetPosition` `0x48d470` later sends **`FA09 u32 id, u16 map, f32 x, y, z`** (20) to the moved player (another map: after `CWorld::ZoneLeaveUserMove`; same map: `CZone::MoveUser`, `0x020B` to the view first). confirmed | 267 | handled |
| `FA0A` | `/bmove <x> <z> [map]` / `/존이동` | 5 | `f32 x, f32 z, u16 map` (default: current map) | moves the GM there (deferred `FA0A`); map 200 goes through the other server (`0x080C`, or `0x0110 02` when it is down) | 268 | handled |
| `FA11` | `/cmove [map]` / `/맵이동` | 5 | `u16 map` (`0xFFFF` when omitted) | the GM's faction bind point on that map (or the current one); deferred `FA11`; map 200 as `FA0A` | 269 | handled |
| `FA12` | `/gmove <x> <z> [map] <guild>` / `/길존이동` | 5 | `f32 x, f32 z, u16 map, char guild[25]` (37) | moves the GM into a guild map (types 3, 5, 6 with their rules); deferred `FA12` | 268 | handled (not in the catalogue) |
| `FA0B` | `/cwatch <name>` / `/대화감시` | 5 | `name[21]` | the GM receives copies of that player's chat as **`F502 char name[21], u8 len, text`** with text = `[kind]` or `[kind,target]` + the line, e.g. `[NORMAL]`, `[TO,name]` (`CUser::SendListenChat` `0x4702f0`; the full table is on [GM commands 1](../original/gm-commands.md#84-listening-fa0b-fa0c)). **The client has no `F502` handler**: the copies are dropped (fallback) | 272 | handled (useless with this client) |
| `FA0C` | `/cwatch off` / `/대화감시해제` | 5 | — | stops it. **The English `/cwatch off` sends `FA0B` for a player named "off"** ([client bugs](#bugs-in-the-clients-command-parser)) | 273 | handled |
| `FA0D` | `/silence [minutes] <name>` / `/채금` | 5 | `u16 minutes (0 = until lifted), name[21]` | chat ban; `FA0D` to the player | 274 | handled |
| `FA0E` | `/silence off <name>` / `/채금해제` | 5 | `name[21]` | lifts it; `FA0E` to the player | 275 | handled |
| `FA0F` | `/stopon [minutes] <name>` / `/이동금지` | 5 | `u16 minutes, name[21]` | movement ban (speed resent); `FA0F` to the player | 276 | handled |
| `FA10` | `/stopoff <name>` / `/이동금지해제` | 5 | `name[21]` | lifts it; `FA10` to the player | 277 | handled |
| `FA13` | `/guildgrade <guild> <name> <grade>` / `/길원등급` (region 6 also takes `"guild name" name grade`) | 4 | `char guild[25], name[21], u8 grade` (49) | `CGuild::UpdateUserLevelAdminCmd`, grade 1–9 | 294 | handled |
| `FA14` | `/guildrank <guild> <rank>` / `/길드순위` | 4 | `char guild[25], u8 rank` | guild rank (< 32) | 294 | handled |
| `FA15` | `/guildpoint <guild> <points>` / `/길드포인트` | 4 | `char guild[25], u32 points` | sets the points, tells the DB agent (`0x0B0B`) | 294 | handled |
| `FA18` | `/xcall <1\|2>` / `/진영소환` | 5 | `u8 n − 1` (0 light, 1 dark) | `CZone::UpdateAllCall` `0x423150`: every **non-staff** player of that faction in the GM's zone, farther than a minimum distance, is moved to the GM's map and position after a random 1–11 s (deferred move kind 1); other values print 2022 locally | 336 | handled |
| `FA19` | `/auctionrecall <name> <marketId>` / `/경매회수` | 4 | `char name[21], u32 marketId` (27) | in a market map only: kicks the seller if online and withdraws the item (`CMarketMgr::RetrievalItem`) | 7140 | handled (not in the catalogue) |
| `FA1A` | `/auctionsearch <name>` / `/경매검색` | 4 | `char name[21]` (23) | `CMarketMgr::SearchCharName` | 7141 | handled (not in the catalogue) |
| `FA16` | `/pvpon` | — | — | **no case: closes the GM's connection** | 266 | client-only |
| `FA17` | `/pvpoff` | — | — | **no case: closes the GM's connection** | 265 | client-only |

`/char on` checks `privilege < 6`, the same as level 5.

### Client-only GM commands (nothing sent)

`/itemlv <n>` (item level shown, 218), `/tacticszone` (toggle `0x21e8714`), `/fogstart <n>` / `/fogend <n>`, `/camlimit on|off`,
`/item <item name>` / `/아이템보기` (prints the item's tooltip text), `/mob <name|id>` / `/몹보기` (prints HP and level), `/몹정보 <n>`
(Korean only), `/chatcolor <1-8>` / `/chatcolor off` (also open to players with feature 30). In the player parser but GM level 5:
`/time <n>`, `/wire`, `/point`, `/basic` (render modes). confirmed

### What the US client sends at each privilege (1–10)

The client is "staff" for privilege 1–10 (`< 11`); each command then needs `privilege <= Lvl` (the "Lvl" column of the tables above). The
original server's family limits are in brackets; a command the original has no case for, or whose family limit is exceeded,
**closes the connection**.

| privilege | commands the client sends | notes |
|---|---|---|
| 1 | everything in the client → server tables above, including `F601` `/apower` and `F602` `/apowercan` | |
| 2 | all but `F6xx`: `F7xx` (`/set`, `/imake`, `/getitem`, `/recallitem`, `/mmake`, `/mera`, `/nmake`, `/nera`, `/server on/off`, `/shutdown`, `/akick`, `/aban`, `/abancan`), `F8xx`, `F9xx`, `FAxx`, `F1xx`, `F2xx` | |
| 3 | `F8xx` (`/cure`, `/iclear`, `/eclear`, `/kick`, `/ban`, `/bancan`, `/queston`, `/questoff`, `/questadd`, `/questrem`, `/event on/off`, `/eventadd`, `/eventrem`), `F9xx`, `FAxx`, `F1xx`, `F2xx` | |
| 4 | `F9xx` (`/quiry`, `/asummon`, `/bsummon`, `/partysummon`, notices), the level-4 `FAxx` (`/guildgrade`, `/guildrank`, `/guildpoint`, `/auctionrecall`, `/auctionsearch`) and all level-5 ones, `F1xx`, `F2xx` | `/asummon`/`/bsummon` also send `FA08` (level 5 → fine at 4) |
| 5 | level-5 `FAxx` (`/char on/off`, `/attack on/off`, `/amove`, `/bmove`, `/cmove`, `/gmove`, `/warning`, `/watch`, `/cwatch`, `/silence`, `/stopon/off`, `/xcall`), `F1xx`, `F2xx`; client-only `/itemlv`, `/tacticszone`, `/fogstart`, `/fogend`, `/time`, `/wire`, `/point`, `/basic` | |
| 6–10 | `F1xx` GM chat (`F101`–`F105`, `F10A`), `F201` `/UQGET`, `F204` `/UQCOUNT`, the question window's `F202`/`F203`, `FA16` `/pvpon`, `FA17` `/pvpoff`; client-only `/camlimit on/off`, `/chatcolor` | `/pvpon`/`/pvpoff` have **no client level check** and the original has no case for them → disconnect at any level. `/autocure` sets its flag but sends nothing off the developers' server |

`/char on` tests `privilege < 6` (= level 5). Any other `/word` at privilege 1–10 prints 254 (level too low) or 253 (unknown).
confirmed (`0x4705d0`: every branch's `0x46d5a0(N)` call; no call in the `/autocure`, `/noautocure`, `/pvpon`, `/pvpoff`,
`/camlimit`, `/chatcolor` branches)

## GM opcodes, server → client (what the client does)

| op | client | original sends it |
|---|---|---|
| `F101` / `F105` | GM chat line (type 0x10, no receiver-side word filter) | yes (relays) |
| `F102` / `F103` / `F104` | named GM chat lines (whisper / trade / guild) | yes |
| `F106` | 356 "[Notice] <b>" (type 8) | yes (`CWorld::SendNotice`, ref) |
| `F107` / `F109` | nothing (empty methods) | yes (GM talk session) |
| `F10A` | 6606 "[Union] <b>" | yes |
| `F201` / `F204` | question list / 344 | yes |
| `F501` | the table in [How a GM command travels](#how-a-gm-command-travels) | yes |
| `F601` / `F602` | own level, 342 / 343 | yes |
| `F701` | sets the own value for that stat index (`0x554030`; mode, level, money, points, stats, HG…IG, exp, kills, deaths) | yes |
| `F802` / `F803` | 337 / 338 and empties the own bags / equipment locally | yes |
| `F709`, `F804` | 339 "Log out by operator" | never (the original just closes the connection) — client-only |
| `F805`, `F70A` | `u16`: 0 → 340 "Ban connecting by operator", else 341 "… for <v> minutes" | never — client-only |
| `F80B`, `F80C`, `F80F`, `F810` | nothing (`u16` read, empty method) | never — client-only |
| `F901` | prints the character sheet and equipment lines | yes |
| `F904` | (`u32 id, u16 map, f32 x, y, z`) for the own id: 336 "Summon by operator", moves; else moves that character | deferred move to the summoned player (confirmed, `FA09` row) |
| `F905`, `F90C` | same move, height taken from the terrain, then a movement packet | deferred move (confirmed) |
| `F906`–`F90B` | `ui_news001.wav` + 356 "[Notice] <b>" (type 0x18) | yes |
| `FA07` | 357 "[Warning] <b>" | yes |
| `FA08` | "[Map:%d, %d, %d]" | yes |
| `FA09`, `FA0A`, `FA11`, `FA12` | moves (own id: map change; other ids: position) | deferred move (confirmed) |
| `FA0D` / `FA0E` / `FA0F` / `FA10` | 332 / 333 / 334 / 335 | yes |

None of these checks that the receiving client is staff: a server may send notices (`F106`, `F906`–`F90B`), warnings (`FA07`) or
`F10A` lines to any player. confirmed (handlers read; `0x554bd0` only changes the filter for staff)

## Player chat commands and prefixes

Parse order in `0x475e00`, after the banned-word filter (`0x41db90`, when `0x7019f4` is set and the player is not staff: result 1
prints 250 and sends nothing, result 2 rewrites the text). Command words from sysmsg are compared as **prefixes** unless noted.

| input | sends | client checks / messages | original |
|---|---|---|---|
| `/return` (5076) or `/town` | `0x0556` (2) | not while busy (`0x4c4620`) or dead (player `+0xe4`); prints 251 | handled (`PacketPC`) |
| `/request GM <text>` (5077, case-insensitive) | `0x1201 u16 n, text[n]` (the text after `/request GM `, terminating zero included) | prints 252 "Successfully applied" | queued for GMs (`PacketUQ`) |
| `/victory` … `/insult` (5114–5123, exact) | `0x0506 u8 116 + i` | not while busy; the world shows 5228–5237 | handled |
| `/yelling <text>` (5078) | `0x1107 u8 len, text` (chat colour marker added) | — | handled |
| `/party <name>` (5079), `/초대`, `/party` | `0x0B09 name[21]` when not in a party; in a party `0x0B18 name[21]` (union invite) unless 6539 / 6540 apply; prints 203 | | handled |
| `/Join <name>` (6543), `/참여`, `/join` | `0x0B11 name[21]` | only outside a party/union, else 6537 | handled |
| `/sign up for guild` (5080) | `0x0D05 u32 target` | the current target must be a player (`0x84fa18 == 2`, *inferred*) | handled |
| `/withdraw` (5081) | `0x0B05` | — | handled |
| `/defeat` (5335, exact) | `0x2406` | only during a duel (`0x7045b4 == 2`) | handled |
| `/raid`, `/alert`, `/command <text>` (6605; feature 30) | `0x1112 u8 len, text` (staff: `F10A`) | `/raid` only when `0x21e485c` is 0 or 1 (*inferred*: union leader or sub-leader, as the original requires) | handled (`PacketChatE`) |
| `/chatcolor <1-8>`, `/Chat Color` (6607); `/chatcolor off`, `/Cancel Chat Color` (6608) | nothing; later chat gets a `<<NNNcCC` colour marker | 483 / 484 | — |
| `/time`, `/wire`, `/point`, `/basic` | nothing | GM level 5, else 254 | — |
| `@ <text>` | `0x1105` party (staff `F105`) | | handled |
| `# <text>` | `0x1104` guild (staff `F104`) | | handled |
| `~ <text>` | `0x1111` zone (no staff variant) | only with region feature 13 (on for region 6) | handled |
| `$ <text>` | `0x1103` trade (staff `F103`) | region 6: at most every 5 s and level ≥ 10 (other regions differ) | handled |
| `! <name> <text>` | `0x1102 name[21], u8 len, text` (staff `F102`); remembers the name for replies | | handled |
| any other text not starting with `/` | `0x1101 u8 len, text` (staff `F101`) | | handled |
| any other `/word` | **nothing**, no message (staff: 253) | | — |

Emote messages: `/victory` 116 "poses for victory" … `/insult` 125 "abuses hard" ([World packets](client-world-packets.md#motions-and-emotes-0x0506)). `/apoligize` is
spelled that way in sysmsg 5121.

## Adding commands

### Without a client change

- **What reaches the server with the text intact**: every line that does not start with `/` and is not one of the 2-character
  prefixes is sent as normal chat, `0x1101 u8 len, text` (staff: `F101`), whole line included. So a server can take commands typed
  with another leading character, e.g. **`.cmd args`** or **`!cmd args`** (`!` followed by a non-space is not a whisper). Other
  carriers: `! <name> <text>` (whisper, `0x1102`) to a reserved name such as `Server` (no real character may take it; the original
  would answer `0x1106` "not found"), and `/request GM <text>` (`0x1201`, `u16` length, prints "Successfully applied").
- **What does not**: a player's `/word` that the client does not know is dropped with no message (`0x4780c3`), so `/`-commands need
  the client patch below. Command prefixes are matched as prefixes: `/returns` acts as `/return`, `/party…` as `/party`.
- **Length**: the length byte includes the terminating zero; the original accepts 2–128 and closes the connection
  above 128. With a chat colour set, the client prepends an 8-byte `<<NNNcCC` marker and cuts the line to 127 bytes; a command parser
  should skip such a marker. The chat box's own input limit was not traced (bounded: `0x475e00` and `0x46d200` only).
- **Other filters**: the banned-word filter (when enabled) can block or rewrite the line before it is sent; `$ ` lines are throttled by
  the client; the server should keep its own privilege checks, since the client's gates are only convenience.
- **Replies**: a whisper from a fixed name (`0x1102 u8 0, name[21], u8 len, text`) is the simplest; `F106` / `F90B`-style notices
  ("[Notice]", with a sound for `F906`–`F90B`) and `FA07` ("[Warning]") also work for any player ([server → client](#gm-opcodes-server-client-what-the-client-does)).
- **GM opcodes a server could reuse** (all reach the server only from a staff client at the listed level):
  - free text: `F906` `/gmnotice <text>` (level 4, `u8 len, text`), `F907`/`F909`/`F90B` (`u16, u8 len, text`), `F908` and `FA07`
    (`name[21]` + text), `F10A` `/raid` (any staff);
  - two free 20-character words: `F80F` `/eventadd <a> <b>` and `F810` `/eventrem <a> <b>` (level 3), which the original rejects
    anyway; one word: `F80D` / `F80E` `/event on|off <word>`;
  - number + name: `F805` `/ban`, `F70A` `/aban` (`u16, name[21]`); name only: `F806`, `F70B`; no arguments: `FA16` `/pvpon`,
    `FA17` `/pvpoff` (no client level check);
  - the ones the original closes the connection for (`F70A`, `F70B`, `F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`, `FA16`, `FA17`)
    are the safest to redefine, since no original behaviour is lost. `F809`/`F80A` only carry a quest id the client could resolve.

### With a client change

- **Where the "table" is**: there is none. Commands are `strncmp` chains: player commands in `0x475e00` (words from sysmsg 5076–5081,
  5114–5123, 5335, 6543 plus inline literals `/town`, `/party`, `/join`, `/time`…), feature-30 commands in `0x46eab0`, GM commands
  in `0x4705d0` (inline English literals and the Korean literals at `0x6aa0e4`–`0x6aa7f0`).
- **Data only**: the sysmsg-backed words can be renamed in `sysmsg-uni.txt` (archive data, no exe change); this renames, it cannot add.
- **Byte patch (two jumps)**, to send unknown `/words` to the server as chat:
  - `0x4780c3`: `0F 84 88 01 00 00` (`JZ 0x478251`, non-staff → drop) → `0F 84 54 00 00 00` (`JZ 0x47811d`): a player's unknown
    `/word` goes out as `0x1101` with the slash.
  - `0x478111`: `68 FD 00 00 00` (`PUSH 0xFD`, start of message 253) → `EB 0A 90 90 90` (`JMP 0x47811d`): a staff member's unknown
    `/word` goes out as `F101`.
  - `0x47811d` is the existing "send as normal chat" block (`kind = 0`, `0x46d200(text)`), and `EBP` holds the line at both sites.
    The client's own commands still win (prefix matches), so new names must not start with an existing one. Not applied, not tested.
- **Companion DLL** (the client's action notes, §6): hook `0x46eab0` (`__thiscall bool(chatWindow, const char *line)`; called for every
  `/` line when feature 30 is on, before the staff gate, and it returns true when it handled the line) for player commands, and
  `0x4705d0` (same shape) for staff ones. A hook can parse its own command table, print help locally (`0x41e2d0(type, text)`) and
  send any packet through `0x585640(buffer, size)`, including new opcodes. Needed for autocompletion, local help or new UI; not for
  sending text.
- **Suggested order**: (1) `.`-prefixed chat commands parsed by the server, replies by whisper, privilege checked on the server; (2)
  the two-jump patch if `/`-commands are wanted; (3) the DLL only for client-side behaviour.

## Bugs in the client's command parser

Found by comparing every `strncmp` length in the chat parsers with its literal (186 compares), confirmed (disasm) and with a patched test
client. Each needs a client patch (one byte or a swapped test); a server cannot work around them except as noted.

| Problem | Cause | What a patch changes |
|---|---|---|
| `/server off` sends `F706` (server **on**) | `0x4728b7`: `PUSH 9` compares `/server on` on 9 bytes (`/server o`), and that test comes first | `PUSH 10` |
| `/cwatch off` sends `FA0B` for a player named "off" | `/cwatch ` (8 bytes, `0x470a7c`) is tested before `/cwatch off` (`0x470aac`) | swap the two tests (text, length and jump target) |
| `/eventadd`, `/eventrem`, `/partysummon` with arguments are never sent | the English test compares one byte more than the literal, which ends in a space, so it matches only the bare word: `PUSH 0xb` for `"/eventadd "` / `"/eventrem "` (10 bytes; `0x471b89`, `0x471bb9`), `PUSH 0xe` for `"/partysummon "` (13; `0x470c2c`); the line falls through to 253 "Invalid command" | `PUSH 0xa`, `PUSH 0xa`, `PUSH 0xd`; and `/partysummon`'s `ADD EBX, 0xe` (`0x474a1e`), which would skip the name's first letter, → `0xd`. No other literal ending in a space is compared on more than its length |
| Text arguments need double quotes (`/warning name "text"`, `/notice 0 "text"`) | `0x46d5e0` copies only what is inside `"…"`; with no quote it leaves the (zeroed) buffer empty | 19 call sites could take the rest of the line instead (the notices, `/warning`, `/event on/off`, the quest and event commands, `/item`, `/mob`, `/recallitem`); without a patch, type the quotes |
| The `/UQGET` question window shows garbage | 16 CP949 labels in `.rdata` (`0x6ab288`–`0x6ab350`) drawn with the English font | English labels in place |
| `/몹정보 <world id>` (monster info, `0x430fe0`) and the faction words of `/bnotice` (빛/어둠) exist only in Korean | Korean literals only (`0x4706ba`; `REPE CMPSB` at `0x473f1f`, `0x473f39`) | English spellings |

Left as they are: `/chatcolor off` is unreachable (region 6 only; `/chatcolor` matches first), and the Korean `/자동치료해제` is shadowed by
`/자동치료` (both work only against the developers' server anyway).

## What hides another player

Confirmed (decompiled). This is what a server must send for Stealth and for an invisible GM; nothing else hides a player.

- **`0x051D` `u32 id, u8 shape`** → `0x54f310` → `0x4c17a0`: finds the player (**an unknown id is ignored**), stores the shape at
  `+0x2a4`, and for **shape 13** sets the player's hidden flag **`+0x27c = 1` only if the player is me or my privilege byte `0x84d668` is
  above 10** (`0x4c1898`–`0x4c18a5`): **a staff client never hides anyone**. It also drops my target if it was that player. Shape 0
  clears `+0x27c` (`0x4c17fc`); `0x0524` shape 0 does too (`0x4c1b40`). Nothing else clears it for another player.
- **`0x0201` kind byte** (`u32 id, u8 kind, u16 dir, f32 x, y, z, u32, u32` → `0x551f60`): a **new** player object starts with every flag
  clear (`0x415970`), so the `0x051D` 13 must come **after** its `0x0201`; **kind 2 sets `+0x28c = 1`, the staff flag** (it also paints
  the name blue, `0xFF0000FF`); kind 1 plays the arrival effect.
- **The draw** (`0x447d30`, the player pass at `0x449646`, the shadow and name passes `0x44af2e`, `0x44b751`): a player with `+0x27c`
  is **skipped** when it is staff (`+0x28c == 1`) **or of the other faction** (`+0x276`, from `0x0303`); otherwise it is drawn
  translucent (`0x413c60`). Footsteps are silent for a hidden staff member (`0x40f010`).
- So **Stealth** (shape 13 on a player, kind 0 or 1) hides the player from the other faction and shows it translucent to its own; **an
  invisible GM** (kind 2 and shape 13) is hidden from every player's client and seen normally by staff clients.

## `/kick`: what the client shows

- **`F804`** (2 bytes) → `0x554010`: prints 339 "Log out by operator" **in the chat window** (type 0x13, not a screen-centre type) and
  sets `0x701b1c = 0`. No box, no disconnect. `F709` is the same handler. The original server never sends it (it just closes the
  connection; see [GM commands 2](../original/gm-commands-2.md#43-f804-kick-name)).
- **When the connection closes** (`WSAAsyncSelect`, message `0x2b10`): `FD_READ` dispatches every complete packet at once; `FD_CLOSE`
  calls slot `+0x0c` (`0x54c690`): a box with 10069 "connection with server has been disconnected", then `WM_CLOSE` (the game ends). A
  read error goes to slot `+0x08` (`0x54c710`: 10069 + "ERROR" and the code).
- So an `F804` sent right before the close is printed but practically not seen. Only four texts can be in the closing box: 10069,
  10070 "disconnect due to using logged in ID." (`0x0004 0`), 10071 "disconnect due to system error" (`0x0004` other), and 10069 with an
  error code. **To show it without a client change**, a server sends `F804` and keeps the connection open for 2 to 3 seconds before
  closing: the player sees the line in the chat, then the box.

## Open

- **Live**: most of the above was not run against a server (the parser bugs were confirmed in a test client). Worth checking first:
  the deferred-move timing and the `F904`/`F905` height handling in game; whether the banned-word filter is on for region 6 (`0x7019f4`).
- `F709` (`CWorld::FindUserBill`, the string overload: a name-keyed map, *inferred* the account name) and the minimum distance in
  `CZone::UpdateAllCall` (`0x53a45c`, not read).
- The chat box's maximum input length (not traced).
