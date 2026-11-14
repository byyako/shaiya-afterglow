# Open items in the client notes: what was settled, what needs a run, what is left

This page lists the statements in the client notes that are not yet **confirmed**, sorts them by who or what can settle them, and records the ones that were settled by reading `game.exe`. It is for anyone who wants to know how far to trust the client pages, or who wants a next question to answer.

## In short

- **The notes mark about 320 statements as [*inferred*](glossary.md#inferred), *open*, *unverified* or *not checked*;** this page sorts them into groups.
- **Most are not questions about `game.exe`.** They are guesses about what the **original server** does with a value (a field's unit, a
  rank, which server function sends a packet), and a client binary cannot answer those.
- **A smaller group needs the client to be run** (what a window looks like, whether a crash happens); these are marked **live**.
- **The rest are real questions that the binary can answer.** The ones read so far are under "Settled", and the page each came from was
  corrected; the ones still to read are in the last table.
- **Everything here is static analysis** of `game.exe` (US, build 2010-04-26, patch 108, SHA-256
  `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`) in Ghidra: nothing was run, no file was changed. Addresses are
  virtual addresses ([VA](glossary.md#va)) in `game.exe`.

### Using this page

- **If you write a server:** the "original server" section lists what the client cannot answer; the original server's own files can.
- **If you patch or mod the client:** the "needs the client running" table lists what to try in play and what to watch for.
- **To check a claim yourself:** the last table is the queue of questions the binary can answer; start from the function or address named
  in the page each item came from ([tools-and-method.md](tools-and-method.md)).

Status words: **settled** = read in the binary, page corrected; **partly** = part of it read, the rest stays *inferred*;
**live** = needs the client running; **server** = a question about the original server, not answerable from the client;
**open** = answerable from the binary, not done; **skip** = not worth the effort.

## Settled

| Item | Page | Answer | Evidence | Status |
|---|---|---|---|---|
| Obelisk states 0/1/2: which minimap picture | [obelisks.md](obelisks.md) | State 0 → `obelisk_0.tga`, 1 → `obelisk_1.tga`, 2 → `obelisk_2.tga`; states 101–103 → `portal.tga`. The faction each picture stands for is not in the code (partly: the files' colours are purple, green, red-brown on average, no more) | images loaded `0x4b90c0` (`+0xc0`…`+0xf0`), drawn `0x4ba850` | partly |
| Item-mall fifth tab = purchase history | [item-mall.md](item-mall.md) | Tab index 4; the click handler `0x4ce550` sets `+0x6ea8 = 4` and loads `ins_market_past.tga` (`0x4cee81`). What its rows show stays *inferred*; the rows' fill by `0x2601` was not re-read | `0x4ce550`@`0x4cee81` | settled (tab), live (contents) |
| `0x2605` resend when the window becomes active (`0x40977b`): the condition | [item-mall.md](item-mall.md) | `WM_ACTIVATEAPP` (message `0x1c`, `wParam` non-zero) in `0x409640`, player object `0x84506c` exists, and cool-down `0x6f1580` is 0; then `0x5897c0` sends `0x2605` and sets the cool-down to 60.0 s (counted down by the frame time in `0x40b1a0`) | `0x409640`@`0x40977b`, `0x5897c0`, `0x40b1a0`@`0x40bead` | settled |
| `0x0556` button callers `0x4d52a7`, `0x4db66c`: which windows | [client-actions.md](client-actions.md) | Not windows: the quick-slot use handlers `0x4d4e10` and `0x4db300` (queued by `0x4c49c0`, case 0 and 1). They send `0x0556` when the used slot entry has the id `0xc0` and the player's `+0x378` is set; nothing is sent when `+0xe4` is non-zero. What `0xc0` names (item or skill) was not found: no `Skill.SData` entry has id 192 | `0x4d4e10`@`0x4d52a7`, `0x4db300`@`0x4db66c`, `0x4c49c0` | corrected (the callers); partly (the entry) |
| Duel request sender `0x589370`: caller; player `+0x1460` | [duels.md](duels.md) | Caller: the player context menu, entry "Request duel" (sysmsg 5008, code `0x1390`) in `0x48c120`@`0x490aaf`, after `0x447880` finds the target player. `+0x1460` is a server-side field (the original's player); the client's own `+0x1460` belongs to the NPC talk window (NPC id, -1 = none, `0x4ef5f0`), so "exchange" stays *inferred* | `0x48c120`@`0x490aaf`, `0x47c340`, `0x47d0d0` | settled (caller); still inferred (`+0x1460`) |
| Personal shop on map 42: the value the visit window adds | [personal-shop.md](personal-shop.md) | The seller-side handler `0x559220` (`0x2308`) adds `trunc(unitPrice × −0.05f)` to the amount in the "is sold" line on map 42 (`0x14ed` printed first). It uses the unit price once, not count × price: for a stack of 1 it equals the server's 5 % truncated; for more it differs (display only, gold comes from `money`) | `0x559220`@`0x5592b9`, float `0x6a9b9c`, `0x5b6d48` | settled (client side); the server page's rule (price × 0.05 truncated) agrees for a count of 1 |
| `FUN_00430280`: origin of the `.svmap` read flag | [formats.md](formats.md) | Region feature 16 (map loading bitmap): `0x452270` asks the feature object `0x7013d4` for feature `0x10`, then calls `0x430280`@`0x452336`; a failed read is ignored | `0x452270`@`0x452336` | settled |
| Region feature 25's condition `0x431a40` | [region-features.md](region-features.md) | True when the target mob's monster id (`+0x2c`) is 2382 or 2383 (`Monster.SData` index: "Descended Light Goddess", "Descended Fury Goddess"); the target window `0x4f8e50`@`0x4f9572` shows the percentage for feature 25 only then. Also called from `0x416cd0`@`0x417b1c` | `0x431a40`, `0x4f8e50` | settled |
| Feature 46 (auto-dismount): who replays the queued action | [packet-opportunities.md](packet-opportunities.md) | When the entity's state becomes 0, `0x4c17a0` (case 0) calls `0x416ba0`; that function ends the dismount (clears the flag at `+0x378`, plays the `ef_dmg_transf01.wav` effect) and, **only if region feature 46 is on**, calls `0x4c49c0`. That function re-runs the queued action: kind 0 → quick slot `0x4d4e10`, kind 1 → skill bar `0x4db300`, kind 2 → skill window `0x4f12c0`, then clears the pending flag `0x21e8988`. The queue itself is the globals `0x21e8988` (pending), `0x21e898c` (kind), `0x21e8990`/`0x21e8994` (arguments), written by the three functions above | decompiles of `0x416ba0`, `0x4c17a0`, `0x4c49c0`; write references to `0x21e8988`–`0x21e8994` | settled. That the server's reply produces the state change is still a live check |
| `[VIDEO] TEXTURE`: a reader of `0x70196c` | [re-engine.md](re-engine.md) | It is a texture size limit. `0x4444d0` passes it to `0x599dc0` (stores it in `0x6f4c8c`); `0x52abd0` turns 128 into 256 and 64 into 128 before creating a texture surface; the settings window `0x4ed990` writes it as well | disassembly at `0x444a1f`, `0x52abe7`; reference list of `0x70196c` | settled |
| Ground item count (`0x0401`) kept by the client? | [re-gameplay-state.md](re-gameplay-state.md) | No. The creator `0x55acb0` takes eight arguments and never reads the fifth (the count); the constructor `0x5571c0` takes none. A dropped stack shows no number | decompile of `0x55acb0` | settled |
| Opcode `0x0002`: what it does beyond its message | [re-network.md](re-network.md) | Shows sysmsg 10069 (message box before the world is entered, white chat line after), sets the main state `0x701b1c` to 0, flags the network object (`+4` = 1) and posts `WM_CLOSE` to the main window | decompile of `0x54c7e0` | settled |
| Opcode `0x0004`: same | [re-network.md](re-network.md) | Reader `0x578ed0` calls slot `+0x14` = `0x54c860`. `u8` 0 → sysmsg 10070, any other → 10071; same box/notice rule, same reset and `WM_CLOSE`. The text is the client's own English pass: 10070 "Disconnected: this ID logged in elsewhere", 10071 "Disconnected due to a system error" | decompiles of `0x578ed0`, `0x54c860` | settled |
| Opcode `0x0111`: what the handler does | [re-network.md](re-network.md) | `0x54c910` frees the global network object `0x7013d8`, builds a new one (`0x4c` bytes, constructor `0x41e190`), sets `+0x1ec` = 1, stores the new address at `0x2223818 + 0x1f0` (the object the pointer `0x2223818` names) and sets the main state to 6 (the reconnect branch of the frame function). The purpose (a hop between game servers) stays *inferred* | decompile of `0x54c910` | partly |
| Opcode `0x0D0C` (client → server): which button sends 0 and which 1 | [client-social-packets.md](client-social-packets.md) | The sender is `0x588bc0` (7 bytes: `u8 state, u32 id`). Its two callers are in the guild window's click handler `0x4e35e0`: the button object at window `+0x2d8c` sends 0 (`0x4e3c6d`), the one at `+0x3260` sends 1 (`0x4e3cb5`), both with the selected member's id. The labels are read in the row below (`+0x2d8c` = Upgrade, `+0x3260` = Degrade) | disassembly at both call sites | settled (see the later row) |
| Opcode `0x1C03`: direction byte | [client-actions.md](client-actions.md) | The sender `0x459f60` sends `u8` 1 for its action 7 and `u8` 2 for its action 8, and sets the window mode (`+0x1878`) to 7 and 0 respectively. That actions 7 and 8 are the previous and next buttons is read in the later row | decompile of `0x459f60` | settled (see the later row) |
| Priest: does a `0x0F02 u8 1` reply crash the client? | the priest note (`research-priest-packets`) | The lookup `0x479390` reads its four table slots at `+0x198008c`/`+0x1980090` of the NPC data object and every path dereferences the table pointer (`ptr + 4 + …`). Nothing in the file fills those slots, so the pointer is null: the read is certain **in the code**. That the window actually reaches it, and the crash itself, remain a live check | decompile of `0x479390`; the earlier note's reference scan | partly (code confirmed, behaviour live) |
| Opcode `0x2104` (road change): does the client ever send or print it | [obelisks.md](obelisks.md) | No instruction uses `0x2104` as an immediate, so the client never sends it; the receive side is the case in the jump table already documented, and it prints nothing | immediate scan for `0x2104` | settled (re-checked, no page change) |
| Guild member status 1/2/3 (`0x0D12`/`0x0D13`/`0x0D19`) | [client-social-packets.md](client-social-packets.md) | The three handlers store the status at member `+0x1e`. The guild window's member list `0x4e15b0` prints 1 and 3 as "On" (1 yellow, 3 green) and anything else as "Off" in red | decompiles of `0x555310`, `0x555450`, `0x5553b0`, `0x4e15b0`; strings at `0x6ad108`/`0x6ad10c` | settled for On/Off; why 3 differs stays *inferred* (war zone) |
| Guild rank meaning (my rank `0x21e54e2`) | [client-social-packets.md](client-social-packets.md) | 0xFF = none. Ranks 1 and 2 enable the member buttons (`0x4e36bf`, `0x4e3705`); only rank 1 may open the window's text-edit overlay (`0x4e3a93`, `0x4e5430`). The member list prints the rank as a number, with no titles | disassembly of `0x4e35e0`, `0x4e5310`; decompile of `0x4e15b0` | partly (that 1 is "master" is *inferred*; the client has no titles) |
| `0x0D0C` promote or demote | [client-social-packets.md](client-social-packets.md) | `+0x2d8c` (sends 0) is the first row of `guild_button1.tga`, "Upgrade"; `+0x3260` (sends 1) is the second row, "Degrade". 0 = promote, 1 = demote | disassembly of the button constructor `0x4e4450` (texture rows at `0x4e4850`, `0x4e4905`); the picture | settled |
| `0x1C03` actions 7 and 8 | [client-actions.md](client-actions.md) | `0x455100` returns 7 for the button at window `+0x1824` and 8 for `+0x1cf8`; `0x452e50` builds them from `market_button.tga`, left arrow at x 383 and right arrow at x 475. 1 = previous, 2 = next | disassembly at `0x455274`, `0x455298`, `0x4531da`, `0x4532c8`; the picture | settled |
| `0x0520` party type at player `+0x2c0` | [client-social-packets.md](client-social-packets.md) | The scene draw `0x447d30` reads it at `0x44bcdd` (and for the target at `0x44bdeb`): 0 nothing, 1 `party_member.tga`, any other value `party_jang.tga`. The server side lists 0/1/2 as none/member/leader | disassembly at both sites; texture names at `0x444b12`, `0x444b2b` | partly (leader meaning from the picture name only) |
| Receive accumulation buffer `0x222aa40`: capacity and overflow | [re-network.md](re-network.md) | No capacity check in the receive handler `0x585280`; the next used global is `0x222ea40`, 16,384 bytes on. The stream copy caps at `0x2000` bytes but the decrypt call is given the uncapped length | decompiles of `0x585280`, `0x589c90`, `0x4010e0`; reference lists | partly (overflow behaviour live) |
| Entity flag `+0x250` ("dead or mounted") | [duels.md](duels.md) | It is the life state of a player entity, not a mount flag: `0x551b50` (`0x0504`, death) sets 1, the death-animation code `0x418c00` waits for the timer at `+0x374` while it is 1 and then sets 2 (`0x410080` also sets 2); `0x54f330` (revive or warp) and the reset `0x415970` set 0. The click code `0x43c000` ignores players whose value is not 0. Nothing in the mount path writes it | disassembly at `0x418c74`, `0x4100a9`, `0x415c81`; decompiles of `0x54f330`, `0x551b50`; scalar search for `+0x250` (the `0x0216` mount answer is not among the writers) | settled |
| Selected target: kind and id | [re-gameplay-state.md](re-gameplay-state.md) | Kind is the global `0x84fa18` (0 none, 1–3 stored by the click code in `0x43c000` at `0x43ca16`, `0x43cb31`, `0x43d5a4`, `0x43d698`, `0x43d880`, `0x44215c`, `0x44220f`; 0 at `0x442280`); the id is the local entity's `+0x160` with the kind copied to `+0x164`. Four other UI functions also store kind 2. What kinds 1 and 3 stand for on screen is not read | reference list of `0x84fa18`; disassembly around each store | partly |
| Mob `byMotion` (`0x0603`) and NPC flag (`0x0E03`) readers | [client-world-packets.md](client-world-packets.md) | Mob `+0x84` is read by the mob move update `0x431aa0` (`0x431b49`, `0x431daf`): 0 picks the walk animation slot, anything else the run slot and the chase speed values. The NPC flag `+0xac` is read by the NPC move update `0x433d90` (`0x433f96`, `0x4340a9`): when set, the step is multiplied by 4.0 (`0x6a67a4`) | decompile of `0x431aa0`; disassembly of `0x433d90`; `u32` at `0x6a67a4` | settled |
| Warehouse contents in memory | [re-gameplay-state.md](re-gameplay-state.md) | `0x54d130` stores one item per call at `0x852c6e + slot × 34` (`+1` type, `+2` type id, `+3` count, `+4` quality, `+6`..`+11` gems, `+12` name) and the money at `0x84d674`; `0x54d240`, `0x5510d0` and `0x557500` (`0x0204`) write the same array; the UI readers are `0x4fb490`, `0x4fb8c0`, `0x4fc1c0`, `0x4fcb40`. The slot count of the array and the other storages are not read | decompile of `0x54d130`; reference list of `0x852c6e`..`0x852c7a` | settled for the warehouse |
| Player actions 2, 3, 10, 11 (`0x0506`) | [client-world-packets.md](client-world-packets.md) | The sender `0x586170` is called from `0x43c000` with 2 (`0x43f11b`, `0x44280f`), 3 (`0x43f315`), 10 (`0x43e7f3`) and 11 (`0x43ec72`); each is tied to a key binding: KEY9, KEY6, KEY2, KEY7 (globals `0x701a74`, `0x701a68`, `0x701a58`, `0x701a6c`), defaults Space, S, Q, E from the table `0x6f2bc0` copied by `0x408630`. That 2 is a jump and 3/10/11 are back-step and the two sidesteps follows from the keys and animations 13/14/15 (*inferred*) | disassembly of the five call sites; `u32` read of `0x6f2bc0`; decompile of `0x408630` | settled for the senders |
| Stat bonuses `0x84d6a4`–`0x84d6b8`: writer | [re-gameplay-state.md](re-gameplay-state.md) | tried: reference lists of all six addresses (only reads in `0x4f3f80`), of `0x84d690`/`0x84d6a0`/`0x84d6dc` and of the object `0x84d668` (only calls with ECX = `0x84d668` into `0x4c2b10`, `0x4c53f0`, `0x4c29a0`, `0x4c5520`, which do not write `+0x3c`..`+0x50`); the stat packet handlers `0x54cdc0`, `0x554030`, `0x54fe30`, `0x54eb40`, `0x54ef10` write the base stats only. No pointer or immediate to the block exists in the file | reference lists, decompiles | partly: no writer reachable statically |
| Scene states 8 and 9 | [re-engine.md](re-engine.md) | The gambling house. The window at manager `+0x260` is the gambling table selector (constructor `0x486bf0`, set-up `0x485bc0`); its input slot `0x486140` returns 4 after the answer flag `0x84cedc` is 1 and `0x84cee0` is non-zero, and `0x48f5e9` then sets state 8; state 9 runs the gambling scene object `0x488330` | decompile of `0x486140`, `0x48f5e9`; answer handlers `0x554fb0`–`0x555130` write `0x84cedc` | settled |
| `data/Config.cfg` keys | [re-engine.md](re-engine.md) | The parser `0x5a8cd0` has no key names: it is a generic INI reader filling the shared store `0x22331cc`. The keys the exe reads from that store: `[Config] FontName` (`0x5315c0`), `[gsCommon] LogDefaultListener`, `LogLevel` (`0x5a5980`) and eight `MiniDumperReport_*` keys (`0x5aabb0`). Caller `0x4bc406` reads a different file, `data/WorldMap.cfg` (`[WorldMap] count`, per-entry `Country`, `MapNum`, `Click_*`, `ImgPos_*`) | tried: the four readers and their strings; open: decompile of `0x5a9490` (five more uses of the store `0x22331cc`, so more keys are possible) | partly: `0x5a9490` not read |
| Error reporter `0x40c4c0` | [re-engine.md](re-engine.md) | Formats the message, looks up the logger (`0x40a2a0`) and calls `0x5a41b0` at level ERROR: listeners plus, if the log file is open, `[time] [ERROR ] …` and an indented message line. Nothing on screen | decompile of `0x40c4c0`, `0x40a2a0`, `0x5a41b0` | settled |
| UI vtable slot 5 (`+0x14`) | [re-engine.md](re-engine.md) | Only the chat window overrides it (`0x423f40`: byte at `+0x3baf`); the chat input handler `0x475e00` reads it at `0x475fb8` and skips its edge-resize and cursor code while it is set. The manager `0x48c120` passes 1/0 when the `ch_face.tga` window (`0x462cc0`) opens/closes (cases `0x14`, `0x15`) | decompile of `0x48c120`; disassembly at `0x475fb8` | settled |
| FPS overlay flag (renderer `+0x410`) | [re-engine.md](re-engine.md) | The only write is the renderer constructor `0x52f610` (sets 0); the `%3.2f fps` drawing in `0x52fe20` is never reached in this build | reference list of offset `0x410` | settled (dead code) |
| Library identities `0x5b73f4`, `0x5b804d`, `0x5a9350`, `0x5cd4ab` | [re-engine.md](re-engine.md) | `srand` (stores the seed in the per-thread block), `_purecall` (calls the handler, then runtime error 25), `gsCommonSDM::Shutdown` (logs the string, shuts the logger down), `D3DXCreateTextureFromFileInMemoryEx` (15-argument wrapper, resource type 3, into the statically linked D3DX loader `0x5ccc36`). The page said 14 arguments: corrected | decompile and disassembly of each | settled |
| Per-pass drawing inside `0x447d30` | [re-engine.md](re-engine.md) | The order of its direct calls is listed on the page (section 9); what each call draws is not read | decompile of `0x447d30` | partly |

## Needs the client running (**live**)

These cannot be settled from the file; each is one logged try. Where the note already holds the cheapest test, it is named.

| Item | Page | Why it is live |
|---|---|---|
| A `0x0F02 u8 1` reply crashes the client; what the four Korean priest buttons look like in the US client | [the Priest packets page](research-priest-packets.md) | needs a server that can send the reply (test steps are in the note) |
| Whether a type-14 NPC is clickable and sends `0x0F01` | [the Priest packets page](research-priest-packets.md) | needs an NPC the client cannot create from its own data |
| `+0x279` = 2 is death (death closes the priest window); Escape closes it | [the Priest packets page](research-priest-packets.md) | state meaning |
| Resolutions outside the list use layout 1 and aspect 1.25: how it looks | [re-engine.md](re-engine.md) | appearance |
| Region features whose effect is only read in code (8, 23, 24, 41, 43, 45, 62) | [packet-opportunities.md](packet-opportunities.md) | every "effect line" there is a code reading; none was run |
| Feature 46: the server's `0x0216` answer produces the state change that replays the queue | [packet-opportunities.md](packet-opportunities.md) | needs mount, skill and server together |
| `0x0004`'s message box; `0xB106` changing a mall entry; feature 0's request and answer | [packet-opportunities.md](packet-opportunities.md) | needs a sender |
| Gruesome Attack and Enhance Wind Spin weapon checks; Healing cast bars | [client-actions.md](client-actions.md) | whether the server's answer is the real gate |
| `0x0F02` exactly 10 gold: client sends, server refuses | [the Priest packets page](research-priest-packets.md) | needs both ends |
| Combined forward and strafe speed (`research-movement-storages-auction.md`), camera-button double buffer (`shipping.md`) | those pages | symptoms are seen only in play |
| Graphics claims: unproven performance benefit of the DLL controls; title/costume hiding paths | [quality-of-life feasibility](qol-feasibility.md) | feasibility, not measurements |

## About the original server (**server**)

The client holds only the *use* of these values, so the notes said "inferred from the original's name". The server pages read the
original server's code, and most of these are now **ref** (the client pages carry the link): the personal shop's 5 % commission on map 42,
truncated ([Personal shops](../original/personal-shop.md#5-buying-0x230d)); the auction fee, `hours × ceil(minBid ÷ 500,000) × 5`
([The auction house](../original/auction.md#52-the-fee-the-deposit)); the guild rank meaning, 1 = master and officers below 3
([Guilds](../original/guild.md#2-ranks)), and rank 4 or lower for a guild war ([Guild wars](../original/guild-war.md#3-request-0x2501));
the guild-house `+8` and `+0xa` being the upgrade and keep Etin (*inferred* from matching numbers, [Guild houses](../original/guild-house.md#2-guildhouseini)); `ItemDropPvP` laying
the stakes on the ground for the winner ([Duels](../original/duel.md#62-who-gets-what-itemdroppvp)); `byMotion` 0 = walk, sent for an idle step, and 1 = run, sent for chase and path moves
([Monsters: walking about](../original/mobs.md#6-walking-about), [fighting back](../original/mob-fight.md#4-chasing), [paths](../original/pathing.md#3-chasing)); `0x020F` and `0x0211` as the faction's kill count and the time
left of its top grade, with the Goddess Blessing raising the grade ([Player kills 2](../original/pvp-2.md#9-the-counts-and-grades)).
The guild-war time field has no client sentence (no `0x25xx` packet carries a time; the server's timers are 10 s and 5 s, the same
page). Still open: the client's own rounding of the auction fee it shows; what the original sends in the NPC `byMotion` of `0x0E03`
(*inferred* from the server's `MoveNpc` calls: motion 0 idle, 1 chase, [Walking NPCs and guards](../original/guards.md#3-walking), [chasing](../original/guards.md#8-chasing); no page shows a packet capture); the union packets `0x0B0E` and `0x0C0B`–`0x0C12` (the table marks them *inferred* because they share
a handler with the party versions); `0xA104`/`0xA105` on the login server (no server page covers them); and who sends `0x0004`
first (the server pages show that the other servers' kick of a second login closes the connection silently, but not that no original
server ever sends it). A copy of the server's own source or a packet capture would answer the rest; the client file cannot.

## Open, answerable from the binary (not done)

| Item | Page | What is known | What to read next |
|---|---|---|---|
| Guild member list storage | [re-gameplay-state.md](re-gameplay-state.md) | member id and name on the entity; the selected target is settled above | where the guild member list is stored |
| Item and skill fields the client ignores | [re-gameplay-state.md](re-gameplay-state.md), [item-sdata-vs-defs.md](item-sdata-vs-defs.md) | the fields it reads are listed | a scan of every use of the three record pointers |
| Network: click and drawing steps of the worked examples | [re-network.md](re-network.md) | the accumulation buffer question is partly settled above (tried: no capacity is declared; the nearest used global is 16,384 bytes on) | the click handler of a ground item; the HP bar drawing |

## Not worth settling (**skip**)

- The 31xx mini-game packets (`0x3101`–`0x3109`) and the 27xx item-list family: optional extras whose packets are described but whose
  purpose is only inferred from names and layouts.
- The Japan-only exit window, the web inventory opener (region 4), the anti-hack packet `0xA303` and the OTP login packet `0xA105`:
  nothing in the US client reaches them.
- The alternative stream cipher (`mode != 0`, [re-network.md](re-network.md)): present in code, never selected by a known server.
- Names guessed from a string alone (stat and item-type names in sysmsg tags, Korean particle markers, `ef_*` effect names) where
  the guess changes nothing about the behaviour.
- Emote rig notes (`EMOTE_RIG_REFERENCE.md`): *inferred* there means an animator's reading of a pose, not a binary question.

## How this was done

The statements were collected from every page in the client notes (search for *inferred*, *open*, *unverified*, *not checked*,
*not traced*), then each was put in one of the groups above by asking: does the answer live in `game.exe`, in the
original server, or only in a running client? The settled rows were read with Ghidra through the scripts in the client repository's `tools/ghidra_scripts`
(decompile, cross-reference list, disassembly around an address, immediate scan); the source page was corrected at the same time.
