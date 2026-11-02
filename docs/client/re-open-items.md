# Open items in the client notes: what was settled, what needs a run, what is left

## In short

The notes mark about 320 statements as *inferred*, *open*, *unverified* or *not checked*. This page sorts them. Most are not
questions about `game.exe` at all: they are guesses about what the **original server** does with a value (a field's unit, a rank, which
server function sends a packet), and a client binary cannot answer those. A smaller group needs the client to be **run** (what a
window looks like, whether a crash happens). The rest are real static questions; this pass settled the ones listed under "Settled" and
corrected the page each one came from. Everything here is static analysis of `game.exe` (US, 2010-04-26, patch 108, SHA-256
`5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`) in Ghidra; nothing was run, and no file was changed. Addresses are
virtual addresses in `game.exe`.

Status words: **settled** = read in the binary, page corrected; **partly** = part of it read, the rest stays *inferred*;
**live** = needs the client running; **server** = a question about the original server, not answerable from the client;
**open** = answerable from the binary, not done; **skip** = not worth the effort.

## Settled in this pass

| Item | Page | Answer | Evidence | Status |
|---|---|---|---|---|
| Feature 46 (auto-dismount): who replays the queued action | [packet-opportunities.md](packet-opportunities.md) | When the entity's state becomes 0, `0x4c17a0` (case 0) calls `0x416ba0`; that function ends the dismount (clears the flag at `+0x378`, plays the `ef_dmg_transf01.wav` effect) and, **only if region feature 46 is on**, calls `0x4c49c0`. That function re-runs the queued action: kind 0 → quick slot `0x4d4e10`, kind 1 → skill bar `0x4db300`, kind 2 → skill window `0x4f12c0`, then clears the pending flag `0x21e8988`. The queue itself is the globals `0x21e8988` (pending), `0x21e898c` (kind), `0x21e8990`/`0x21e8994` (arguments), written by the three functions above | decompiles of `0x416ba0`, `0x4c17a0`, `0x4c49c0`; write references to `0x21e8988`–`0x21e8994` | settled. That the server's reply produces the state change is still a live check |
| `[VIDEO] TEXTURE`: a reader of `0x70196c` | [re-engine.md](re-engine.md) | It is a texture size limit. `0x4444d0` passes it to `0x599dc0` (stores it in `0x6f4c8c`); `0x52abd0` turns 128 into 256 and 64 into 128 before creating a texture surface; the settings window `0x4ed990` writes it as well | disassembly at `0x444a1f`, `0x52abe7`; reference list of `0x70196c` | settled |
| Ground item count (`0x0401`) kept by the client? | [re-gameplay-state.md](re-gameplay-state.md) | No. The creator `0x55acb0` takes eight arguments and never reads the fifth (the count); the constructor `0x5571c0` takes none. A dropped stack shows no number | decompile of `0x55acb0` | settled |
| Opcode `0x0002`: what it does beyond its message | [re-network.md](re-network.md) | Shows sysmsg 10069 (message box before the world is entered, white chat line after), sets the main state `0x701b1c` to 0, flags the network object (`+4` = 1) and posts `WM_CLOSE` to the main window | decompile of `0x54c7e0` | settled |
| Opcode `0x0004`: same | [re-network.md](re-network.md) | Reader `0x578ed0` calls slot `+0x14` = `0x54c860`. `u8` 0 → sysmsg 10070, any other → 10071; same box/notice rule, same reset and `WM_CLOSE`. The text is the client's own English pass: 10070 "Disconnected: this ID logged in elsewhere", 10071 "Disconnected due to a system error" | decompiles of `0x578ed0`, `0x54c860` | settled |
| Opcode `0x0111`: what the handler does | [re-network.md](re-network.md) | `0x54c910` frees the global network object `0x7013d8`, builds a new one (`0x4c` bytes, constructor `0x41e190`), sets `+0x1ec` = 1, stores the new address at `0x2223818 + 0x1f0` (the object the pointer `0x2223818` names) and sets the main state to 6 (the reconnect branch of the frame function). The purpose (a hop between game servers) stays *inferred* | decompile of `0x54c910` | partly |
| Opcode `0x0D0C` (client → server): which button sends 0 and which 1 | [client-social-packets.md](client-social-packets.md) | The sender is `0x588bc0` (7 bytes: `u8 state, u32 id`). Its two callers are in the guild window's click handler `0x4e35e0`: the button object at window `+0x2d8c` sends 0 (`0x4e3c6d`), the one at `+0x3260` sends 1 (`0x4e3cb5`), both with the selected member's id. Which label each button carries was not traced, so demote/promote stays *inferred* | disassembly at both call sites | partly |
| Opcode `0x1C03`: direction byte | [client-actions.md](client-actions.md) | The sender `0x459f60` sends `u8` 1 for its action 7 and `u8` 2 for its action 8, and sets the window mode (`+0x1878`) to 7 and 0 respectively. That actions 7 and 8 are the previous and next buttons is still *inferred* | decompile of `0x459f60` | partly |
| Priest: does a `0x0F02 u8 1` reply crash the client? | the priest note (`client-research-20261026-priest`) | The lookup `0x479390` reads its four table slots at `+0x198008c`/`+0x1980090` of the NPC data object and every path dereferences the table pointer (`ptr + 4 + …`). Nothing in the file fills those slots, so the pointer is null: the read is certain **in the code**. That the window actually reaches it, and the crash itself, remain a live check | decompile of `0x479390`; the earlier note's reference scan | partly (code confirmed, behaviour live) |
| Opcode `0x2104` (road change): does the client ever send or print it | [obelisks.md](obelisks.md) | No instruction uses `0x2104` as an immediate, so the client never sends it; the receive side is the case in the jump table already documented, and it prints nothing | immediate scan for `0x2104` | settled (re-checked, no page change) |

## Needs the client running (**live**)

These cannot be settled from the file; each is one logged try. Where the note already holds the cheapest test, it is named.

| Item | Page | Why it is live |
|---|---|---|
| A `0x0F02 u8 1` reply crashes the client; what the four Korean priest buttons look like in the US client | priest note | needs a server that can send the reply (test steps are in the note) |
| Whether a type-14 NPC is clickable and sends `0x0F01` | priest note | needs an NPC the client cannot create from its own data |
| `+0x279` = 2 is death (death closes the priest window); Escape closes it | priest note | state meaning |
| Resolutions outside the list use layout 1 and aspect 1.25: how it looks | [re-engine.md](re-engine.md) | appearance |
| Region features whose effect is only read in code (8, 23, 24, 41, 43, 45, 62) | [packet-opportunities.md](packet-opportunities.md) | every "effect line" there is a code reading; none was run |
| Feature 46: the server's `0x0216` answer produces the state change that replays the queue | [packet-opportunities.md](packet-opportunities.md) | needs mount, skill and server together |
| `0x0004`'s message box; `0xB106` changing a mall entry; feature 0's request and answer | [packet-opportunities.md](packet-opportunities.md) | needs a sender |
| Gruesome Attack and Enhance Wind Spin weapon checks; Healing cast bars | [client-actions.md](client-actions.md) | whether the server's answer is the real gate |
| `0x0F02` exactly 10 gold: client sends, server refuses | priest note | needs both ends |
| Combined forward and strafe speed (`research-movement-storages-auction.md`), camera-button double buffer (`shipping.md`) | those pages | symptoms are seen only in play |
| Graphics claims: unproven performance benefit of the DLL controls; title/costume hiding paths | [quality-of-life feasibility](qol-feasibility.md) | feasibility, not measurements |

## About the original server (**server**)

The client holds only the *use* of these values, so the notes say "inferred from the original's name" and that is as far as the client
goes: the commission factor 5 % of the personal shop and its rounding, the auction commission rounding, the guild rank meaning
(rank < 5 or < 3, master = 1), the guild-house `+8` cost being Etin, the unit of the guild-war time field, `ItemDropPvP` paying the
stakes, `byMotion` values meaning walk/run on the server's side, the faction kill-count packets (`0x020F`, `0x0211`) and the
"blessing of the goddess" event, the union packets `0x0B0E` and `0x0C0B`–`0x0C12` (the table marks them *inferred* because they share
a handler with the party versions), `0xA104`/`0xA105` on the login server, and who sends `0x0004` first. A copy of the server's own
source or a packet capture would answer them; the client file cannot.

## Open, answerable from the binary (not done in this pass)

| Item | Page | What is known | What a next pass should read |
|---|---|---|---|
| Stat bonuses `0x84d6a4`–`0x84d6b8` | [re-gameplay-state.md](re-gameplay-state.md) | read by the status window `0x4f3f80`; no instruction writes these absolute addresses; the loop at `0x46b7e0` zeroes `0x84d6bc`–`0x84d6d4` only | writers that reach them through the object at `0x84d668` (offsets `+0x3c`–`+0x50`) |
| Mob `byMotion` (`0x0603`) and NPC motion flag (`0x0E03`) | [client-world-packets.md](client-world-packets.md) | the handler `0x54e810` stores it at mob `+0x84` and nothing in that handler reads it; a search for `+0x84` readers is too noisy to attribute without the mob update function | the mob per-frame update, to see whether `+0x84` is read |
| Which guild button is demote and which promote (`0x0D0C`) | [client-social-packets.md](client-social-packets.md) | the two button objects and their sends are known (above) | the draw/label code for window `+0x2d8c` and `+0x3260` |
| Auction page buttons: actions 7 and 8 of `0x459f60` | [client-actions.md](client-actions.md) | the byte each action sends is known | the caller that maps the clicked button to the action |
| Warehouse and other storages in memory | [re-gameplay-state.md](re-gameplay-state.md) | packet layouts known | the arrays and their handlers |
| Selected target, guild member list and ranks | [re-gameplay-state.md](re-gameplay-state.md) | target kinds 1–4 in `0x43c000`; member id and name on the entity | where the click stores the target; the list storage |
| Item and skill fields the client ignores | [re-gameplay-state.md](re-gameplay-state.md), [item-sdata-vs-defs.md](item-sdata-vs-defs.md) | the fields it reads are listed | a scan of every use of the three record pointers |
| Scene states 8 and 9; UI slot 5; FPS overlay flag (renderer `+0x410`); error reporter `0x40c4c0` | [re-engine.md](re-engine.md) | each is described; `0x40c4c0` formats its message (`0x5a7c70`) and hands it to the logger `0x40a2a0`/`0x5a41b0` | which window returns 4 for state 8; the caller of slot 5; the flag's writer; the sink of the reporter |
| `data/Config.cfg` keys | [re-engine.md](re-engine.md) | opened and parsed by the same parser as `gsConfig.cfg` (`0x5a8cd0`; callers `0x40a774`, `0x5a9343`, `0x4bc406`); the US build ships no such file | the key names inside the parser |
| Per-pass drawing inside `0x447d30` | [re-engine.md](re-engine.md) | outer passes only | draw order inside the scene function |
| Network: accumulation buffer capacity at `0x222aa40` and overflow; click and drawing steps of the worked examples | [re-network.md](re-network.md) | no overflow check found | the receive buffer size; the click handler of a ground item; the HP bar drawing |
| `FUN_00430280`: the optional `Data/World/<id>.svmap` read and its feature flag | [formats.md](formats.md) | optional; "not verified" | the flag's origin |

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
*unconfirmed*, *not traced*), then each was put in one of the groups above by asking: does the answer live in `game.exe`, in the
original server, or only in a running client? The settled rows were read with Ghidra through the scripts in `tools/ghidra_scripts`
(decompile, cross-reference list, disassembly around an address, immediate scan); the source page was corrected in the same pass.
