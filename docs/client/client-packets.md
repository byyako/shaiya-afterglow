# How the client receives and sends packets

This page describes how the client `game.exe` (US build 2010-04-26, patch 108) reads packets from the server and sends its own: the
dispatcher, the read routines and their error report, the send routines, a packet log built into the client, and a few findings that
matter to a server. The full per-opcode table is on [Client packet catalogue](client-packets.generated.md); the connection, the cipher and the framing are on [How the client talks to the servers](re-network.md).

Status words: **confirmed** = read in the code; **ref** = stated on a linked server page; *inferred*; **open** = not traced.

## In short

- **Every packet from the server goes through one big switch** on its 2-byte [opcode](glossary.md#opcode). A packet the client does not know is silently
  ignored.
- **The client reads fields one by one, in order**, so the order of its reads is the packet layout it expects. A packet that is too short
  is padded with zeros and reported once as "Packet Underflow", which makes a handy check for a server developer.
- **The client has a packet log built in.** A small config file next to `game.exe` turns on a hex dump of every packet sent and received.
- **One chat trick crashes clients.** A colour marker typed into chat with a length that runs past the text crashes every client that
  shows the line, so any server that relays chat is exposed to it (the patch kit has a filter for the original server).

### Using this page

- **If you write a server:** "Findings that matter to a server" lists the checks the client makes on what it receives, and the chat
  marker filter you need if the server relays chat. [What a server must do for this client](afterglow/server-checklist.md#packet-handling)
  collects the rules.
- **If you patch or mod the client:** to see the traffic, create `gsConfig.cfg` next to `game.exe` with the three lines shown under
  "Built-in packet logging"; the log file `Log.txt` then holds a hex dump of every packet sent and received (read from the code, not yet tried in play: **live**).
- **To check a claim yourself:** the receive and send tables give the addresses to open in a decompiler; "How the catalogue is made"
  and [Tools and method](tools-and-method.md) say how the dumps were produced.

## Receive path (confirmed)

| Address | What |
|---|---|
| `0x585390` | receive loop: per packet, decrypt (`0x4010e0`), then dispatch |
| `0x582e10` | **main dispatcher**: one `switch` on the u16 opcode, 393 cases. Most call a handler `FUN_xxx(stream)`; some call a slot of the network object's vtable instead (resolved statically, see below); `0x0509` and `0x1112` are explicit no-ops. |
| `0x6A5C20` | vtable of the network object (386 slots; constructed in `FUN_0040a4f0`, stored at `0x2223818` and `0x222ea40` by `FUN_0040a6f0`) |
| vt `+0x600` = `0x551030` | **fallback** for opcodes not in the switch: handles `0x0114` (server time → clock offset `0x21e8730`), passes family `0x1Cxx` to `0x55d490` and `0x1Dxx` to `0x55bfe0`; anything else is silently ignored |
| `0x589d10` | `read(dst, n)`: copy exactly n bytes. If the packet is too short it zero-fills and calls vt `+0x5fc` |
| `0x589da0` | `read_upto(dst, n)`: copy up to n bytes (variable tails such as strings) |
| vt `+0x5fc` = `0x552c40` | **underflow error** `CNetWork::PacketError` (`NetWork.cpp`): logs `Packet Underflow. Protocol : 0x%04X Size : %d Read %d`; in error-report mode (`0x22332f0 == 2`) shows the "Shaiya Error Report-Packet Error" window. Reported once per opcode. |

A **vtable** is the table of function pointers a C++ object carries; "vt `+0x600`" is the slot at that offset. Handlers read fields one at
a time in order, so the catalogue's "client reads" column is the wire layout as the client expects it. Reads inside `if`s and loops depend
on earlier fields (counts, result codes).

## Send path (confirmed)

`0x585640 send(buf, len)`: only while connected (`0x6f4604`); logs via the packet dumper, prepends the u16 total length, encrypts
(`0x4010a0`: AES-CTR; the client never XORs what it sends), `send()`. On `WSAEWOULDBLOCK` it queues up to 16 KiB, then "Send Error!!!".

`0x585520 send3(u16 opcode, payload, len)` does the same but writes the opcode itself; only the auction house uses it (`0x1C01`–`0x1C17`,
22 call sites; see [Auction house](auction-house.md)). Both are scanned: 245 distinct opcodes are sent from 241 functions. Most senders
build the packet on the stack, so the catalogue gives offsets (a stack slot Ghidra fills byte by byte shows as `bytes[n]`; `send3`
payloads holding a copied string still show the slot types, see [Auction house](auction-house.md) for those).

## Built-in packet logging (confirmed statically; not yet tried live)

`0x5851b0` dumps every packet as hex (`SEND>> [opcode/len] xx xx …`, and `RECV>>`) through the gsCommon logger (`PacketDump`,
`ProjectSNet.cpp`) when bit `0x40` of the log mask `0x2232e14` is set. To turn it on, put a file `gsConfig.cfg` next to `game.exe`:

```
[gsCommon]
LogDefaultListener=true
LogLevel=67
```

- `WinMain` (`0x40b1a0`) calls `gsCommonSDM::Init(1)` (`0x5a9490`) first thing; with argument 1 it loads `gsConfig.cfg` if it exists.
  Parser `0x5a8cd0`: INI-like, `[Section]`, `key<sep>value` with separator tab, `:` or `=`; lines starting with `#`, `;`, `@` are
  comments. The file may also be gsZip-packed (first u32 `0x02704F43`); plain text is fine. Other keys seen: `ChangeDirectory`,
  `RegDefaultKeyparentName` (registry root, HKLM), `MiniDumperReport_*` (crash-report mailer settings).
- Log init `0x5a5980`: `LogDefaultListener` is a boolean (`true`, `TRUE` or non-zero); when set, the logger at `0x2232dd8` opens its file
  (`0x5a3d90`, mode `wt`). File name `Log.txt` (static string `0x6f4cb0`), in the current directory, or
  `UserData/Logs/<YYYYmmddHHMMSS>Log.txt` when mask bit `0x1000` is set. `LogLevel` is the mask, parsed with `atol` (decimal only).
  Without the file the mask is 3.

## Findings that matter to a server

- **86 opcodes match the EP4.5 server's PDB struct sizes exactly** (✓ in the catalogue), so those layouts are confirmed from both ends.
  (The PDB is the debug-symbol file shipped with `ps_game.exe`; it names the server's packet structs and gives their sizes.)
- **`0x020A` portal use**: the client sends **3 bytes** (`u8 portal`); `TPC_USER_USEPORTAL` is 4 bytes because the same struct is used for
  the reply (`byPortal, byResult`). The client sends it whenever the player stands in a portal
  box of its own `world/<map>.wld` (index = position in that list, the same order as the server's `.svmap`), with no level or country
  check, and again every 15 s while still inside (`0x443541`, flag at network object `+0x358`). Refusals (`0x555b60`, chat type 7):
  0 → 8038, 1 → 8039, 2 → 8040, 3 → 8041, 4 → 8042 "only party members allowed", 5 → the ranking-battle question 8043 (yes sends
  **`0x0227 u8 portal`**), 6 → 8044, 7 → 8047, 8 → 8045, 9 → 6600 "Now party instance zone closed.", 10 → 6601 (lack of party members),
  11 → 6602 "You failed to enter.", 12 → 6603; others silent. Confirmed.
- **`0x0114` server time** is handled (through the fallback), reads a u32 and sets the client's clock offset.
- **`0x1112` (union/party-leader chat) received from the server is ignored** by this client, although the client *sends* `0x1112`.
- **`0x0509`** is explicitly ignored.
- The client never sends anything the server cannot attribute: every send has a constant opcode except some in the big UI function
  `0x4705d0` and helpers that take the opcode as an argument (one `?` row in the catalogue).
- **`0x0205` pick up (`u32 groundItemId`) and `0x0502` attack a player (`u32 playerId`)**, both 6 bytes, were missing from the catalogue
  until their send stubs (`0x5859a0`, `0x586110`) became Ghidra functions; they are in it now. **`0x0502` is also a server → client
  opcode** (`TP_PC_ATTACK`, 17 bytes, also used with `bySuccess = 5` when a move is refused), so the same number is a 6-byte request
  and a 17-byte answer. Details: [World packets](client-world-packets.md).
- **The underflow error is a layout check.** If the client logs `Packet Underflow. Protocol : 0x…`, the packet it was given is shorter
  than the catalogue says.

### Chat markup crash (`<<NNNcCC`)

- **Symptom and cause.** The client's text renderer treats `<<` + 3 digits + `c` + 2 digits as a colour/link marker for the
  next NNN bytes (the client builds it only for item names: `sprintf("<<%03dc%02d", strlen(name), colour)` + name, e.g. in the `0x0205`
  handler; recognised by `FUN_0041bec0`). A chat line with a marker whose NNN runs past the text crashes **every client that displays it**,
  the sender's included. The client lets a player type it.
  All `11xx`/`F1xx` chat with text is affected.
- **The check.** A message containing `<<` is safe only if each marker is `<<` + 3 digits + `c` + 2 digits with 1 ≤ NNN ≤ the bytes
  left after the 8-byte marker; otherwise it must be dropped or broken up. For the original server, the patch kit's
  [`chat-markup-filter`](../original/community-fixes.md#chat-markup-filter) breaks such markers up in place. Whether the renderer stops at the
  first NUL inside the text, or counts to the packet's length, was not checked (the cautious reading is to the first NUL).

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#packet-handling).

## How the catalogue is made

The catalogue is produced from headless Ghidra runs over `game.exe`, in four steps:

1. Once per Ghidra project, define the code the auto-analysis missed and follow every switch table (below).
2. Decompile the dispatcher `0x582e10` and turn its switch into an opcode table and lists of handlers, senders and sub-dispatchers.
3. Dump every handler, every sender, the fallback `0x551030` and the two sub-dispatchers (`0x55d490` for `1Cxx`, `0x55bfe0` for `1Dxx`).
4. Build the catalogue from those dumps, cross-referenced with an EP6 packet catalogue and the struct sizes from the server's PDB.

## Code Ghidra's analysis missed

A script listed every `JMP [reg*4 + table]` whose targets are not all disassembled and referenced, and `FF 24 xx` jumps in bytes never
disassembled. Before the fix: 512 table jumps, 8 unresolved, plus 7 in undisassembled code. The cause of the second kind: about **1,400
functions reached only through pointers were not Ghidra functions**, among them most of the network object's vtable slots (the packet
*logic*, e.g. `0x557500` for `0x0204`, `0x55a670` for `0x0205`) and SEH handlers (Windows structured exception handling). Earlier dumps
created them on the fly, so their switch tables were not followed.

A second script fixed it (it writes the Ghidra project):

- code pointers with no function: u32s in `.rdata`/`.data` next to another code pointer (vtables, callback and SEH scope tables), and
  `PUSH`/`MOV` immediates. 16-byte aligned targets (and CRT `mov edi, edi` prologues) became functions; unaligned ones (`__except`
  blocks, catch continuations) plain code. Bogus `pointer` data Ghidra had put on code (`0x441a19`, `0x4ffff0`) and three misaligned
  orphan instruction runs (`0x55a912`, `0x500021`, `0x6a006a`) were cleared where they blocked a real flow;
- then every table jump was followed until nothing new turned up (2 passes).

Result: 9,391 → **10,778 functions**; switch tables newly followed in `0x43c000` (case 6 of the action state `0x7050b0`: sends `0x0212`
and recomputes the move direction; +638 bytes), `0x4f3f80`, `0x4f8e50`, `0x503f90`, `0x55a670` (the `0x0205` error switch), `0x5ac1d0`
(49 cases). Left: 6 jumps in the CRT `memcpy` variants `0x5b5f60`/`0x5c3bc0` (entry 0 of their tables is unused; Ghidra already handles
them) and ~72 KB of `.text` that nothing points to (dead code, mostly after CRT functions). Rebuilding the catalogue afterwards found
**no new handler or request beyond `0x0205` and `0x0502`**, and no handler's fields changed: the readers (`FUN_0057xxxx`) were already
functions; only logic functions were missing.

Also fixed: Ghidra had marked the CRT `_free` (`0x5b8b87` and its two jump thunks) **non-returning**, so every decompiled function stopped
at its first `free()` (the decompiler printed "Subroutine does not return" and dropped the rest). Clearing the flag removed the
`CALL_RETURN` flow override analysis had put on the 2,545 calls to it, disassembled the code after each call and regrew the 1,680
functions holding them. The functions still marked non-returning are genuine (`__CxxThrowException`, `_exit`, `_abort`, `_longjmp` and two
wrappers that throw). The catalogue did not change (readers do not free), but decompiles of logic and UI code made before this fix may
miss code after a `free`.

## Open questions

- Handlers whose reads are hidden in callees (e.g. `0xFA0E` → `0x5537e0`, "no reads"): follow the callee.
- Name the handlers (what each does in the UI and world), starting with the in-world families `0x05xx` (movement/combat), `0x02xx`
  (items/view), `0x0Dxx`/`0x0Bxx`.
- Login-connection packets (`0xA1xx`/`0xA2xx`) also go through this dispatcher; compare with the [login server](../original/ps_login.md).
- Try the packet log live and document the exact line format of `RECV>>` (where in the receive path it is called).
