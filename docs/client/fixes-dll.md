# The companion DLL

This page describes `fixes.dll`, a small 32-bit Windows library that changes the US `game.exe` (build 2010-04-26, patch 108) at run time
for things a handful of changed bytes cannot do: switching on features the client already contains, fixing broken text and GM commands,
keeping the game from freezing the mouse in the background, adding one new packet handler, and optional graphics controls. It describes the
load path, the safety checks, every change with its address, the original bytes or slot it replaces and the new behaviour, and the
settings. **The DLL and its source are not published and not offered for download;** the page exists so the changes can be understood,
checked against your own copy of the client, and reproduced by anyone who writes their own. For plain byte patches see
`client-patch-kit.md`. Status words: **confirmed** (read in the binary, address given), **confirmed (disasm)**, [*inferred*](glossary.md#inferred), [**live**](glossary.md#live)
(needs a game session).

## In short

- **It loads before the game starts** because `game.exe` carries one extra import for it, so Windows runs its `DllMain` first.
- **It changes nothing unless the executable is the right one:** every write first checks the build stamp and then the exact
  bytes it replaces, and logs what it did.
- **Its biggest tool is one hook:** the client decides most [region features](glossary.md#region-feature) through one function, `IsOn(n)`; the DLL answers "yes" for
  a chosen list (default: 0, 5, 8, 23, 26, 43, 45, 46, 62) and asks the original for everything else.
- **It also fixes:** the `0x2104` road lines, 34 Korean texts, five broken GM chat commands (and a quote parser), anti-freeze in the
  background, and adds a handler for a new operator-chat packet (`operator-listen.md`).
- **Graphics controls are off by default** and install no Direct3D hook until a setting asks for one (`graphics.md`).
- **Everything is a setting** in an INI file next to the DLL; defaults are decided, not detected.

### Using this page

- **To check what the DLL does:** each section names the address it changes and the original bytes; compare them with your own copy of the client as [Tools and method](tools-and-method.md) describes. The DLL itself is not published.
- **If you use a DLL like this:** it reads a settings file next to it (table under "Settings"). An empty `features` list installs no hook, a missing file means defaults, and `log` on writes a log of every change applied or refused. Start the game as usual; a refused change shows in the log, not on screen.
- **If you write a server:** "Region features" lists what each feature needs from a server (for example `0x0608` for exact boss HP).
- **For plain byte patches** that need no DLL, see [the patch kit](client-patch-kit.md).

## How it loads

A patch tool appends a fifth section, `.fixes`, to `game.exe`: a copy of the 16 import descriptors plus one for `fixes.dll!fixes_init`
and a null entry, with their lookup and address tables, the hint/name entry and the DLL name. The import data directory then points at
it (size `0x168`), `SizeOfImage` becomes `0x1e46000` and the section count 5. The new section is at RVA `0x1e45000`, raw data at the end of
the file, characteristics `0xC0000040`; its header sits at file offset `0x2d8`, right after the section table, over two leftover
`mciqtz32.dll` strings that nothing references (no VA or RVA points at them; bound imports are off). The original import tables are not
moved. Checked with `llvm-objdump -p` and by running the game.

Without the DLL next to it, Windows refuses to start a patched `game.exe`, which is why the byte-patch kit does not include the import.
`fixes_init` only has to exist (it returns the DLL's version); the work happens in `DllMain` on process attach, before the game's entry
point. The DLL reads its settings file from its own directory, applies each change, and, if logging or metrics are on, writes a log
listing every change as applied or refused.

## Safety rules

- **Build check:** nothing at all is changed unless the PE `TimeDateStamp` is `0x4bd5001e`; any other build is logged and left alone.
- **Byte-checked writes:** each change is written only if the bytes currently there equal the expected original; otherwise it is skipped
  and logged.
- **Redirected calls** are `CALL rel32` instructions that must currently call the expected function; **import hooks** replace one import
  address table entry and keep the real function for the replacement to call.
- **Settings are per change,** so a single one can be turned off without a new `game.exe`.

## Region features

The client keeps a feature object at `0x7013d4` (built by `0x405cd0`, vtable `0x6a5614`). Each of its 63 features has a list of region numbers
ended by `-100`; the constructor builds every list with stack stores. `IsOn(n)` (`0x405c60`, `__thiscall`, one argument) finds feature `n`
and returns true when the client's region (`0x6f1558`; 6 is English) is in its list, or when the list holds 0 and the login address is
the original developers' internal one. **Confirmed.** Every call goes through the vtable slot at `0x6a5618` (no direct calls exist, by byte
scan), so the DLL overwrites that one slot with its own function:

```
IsOn(features, n):                  // replacement, __thiscall
    if n in 0..62 and bit n of the chosen set is on:
        return true
    return original_IsOn(features, n)   // 0x405c60
```

This is one place for every feature and does not depend on the list layouts. The alternative, rewriting one region constant per
feature in the constructor, works but each list differs and some are filled by `REP STOSD`. The default set and what each feature does:

| Feature | What it does when on | Needs from a server |
|---|---|---|
| 0 | lapis extraction with a chosen gem (8-byte `0x0802`, 34-byte answer); extraction hammers usable | answers the 8-byte request with 34 bytes |
| 5 | Easy mode asks for confirmation at character creation | – |
| 8 | the revive window says when you have no Resurrection Rune | – |
| 23 | time left on the continuous-resurrection buff | – |
| 26 | target HP percentage under the target bar (25 shows it only when `0x431a40` allows: the target is monster id 2382 or 2383, the two Descended Goddess bosses; redundant with 26) | `0x0608` for exact maxima |
| 43 | a brand-new character gets two default quick slots | – |
| 45 | an options checkbox "lock the skill window" | – |
| 46 | auto-dismount: a skill or quick slot used while riding sends `0x0216` and queues the action (resume path `0x416ba0` → `0x4c49c0`) | the answer must return the entity to state 0 (**live**) |
| 62 | dragging a quick slot onto another swaps them | – |

The full per-feature table, with every caller and the reasons features 6, 7/55, 27, 28/29, 36, 41, 44, 48–51 are not recommended, is in
`packet-opportunities.md`; the research notes are in `research-movement-storages-auction.md`.

## The `0x2104` road lines

`0x2104` carries a road-state change. The handler reads the new-state byte where it needs the 16-bit link id, so the "a road to … will be
opened" lines never show. The DLL replaces the 5 bytes at `0x54fcd5` (file offset `0x14FCD5`): `0F B6 44 24 10` (`MOVZX EAX, byte
[ESP+0x10]`) → `0F B7 44 24 08` (`MOVZX EAX, word [ESP+8]`). **Confirmed (disasm);** the same bytes are the kit's `road-messages` patch
(`client-patch-kit.md`). It is useful once a server builds road portals with link ids 1010, 1011, 1020 and 1021 (`obelisks.md`).

## Anti-freeze in the background

The old byte patch stops the window procedure copying `WM_ACTIVATEAPP` into the "app active" flag `0x701B08` (a 6-byte write at `0x409710`)
and starts the flag at 1. The game then keeps running when it loses focus, **including its input code, which was never meant to run
unfocused**: mouse and keyboard come from DirectInput in foreground mode (cooperative level 6 at `0x594647`, mouse flags from `0x594680`,
set up by `0x51e6e1`); without focus `GetDeviceState` fails and the polls (`0x5943e0` mouse, `0x594280` keyboard) return 0 but have already
flipped their double buffer, so the input step `0x51e720` keeps comparing two old frames. *Inferred* from the symptom: a camera button
held at focus loss stays "down", the cursor stays hidden and `SetCursorPos` puts it back every frame, which holds the mouse inside the
game window. About 20 places also read keys with `GetAsyncKeyState` (`0x426840`, `0x44a0b9`, `0x464908`, … `0x4ff695`), which reports the
physical state whatever window has focus, so keys typed elsewhere reach the game.

The DLL keeps the same NOP and the flag at 1, but tracks real focus itself and gives the game **no input without focus**: the two
DirectInput polls (calls at `0x51e73d`, `0x51e74d`, `0x4b6c07` redirected) flip as the original does and leave an empty current state;
the `GetAsyncKeyState` (`0x6a5460`) and `SetCursorPos` (`0x6a53c4`) import entries are hooked to return 0 and do nothing. A button held at
focus loss then reads as released, so the game shows the cursor again through its own path. Windowed mode is the target; full screen
loses the Direct3D device on focus loss (**live**). A background frame cap is in the graphics settings.

## English texts and GM command fixes

**Texts.** 34 Korean literals in `.rdata` are overwritten in place with English, each checked against its original bytes and sized to leave
room for its zero: the GM question window's buttons and fields (`0x6ab288`–`0x6ab350`: Hold, Resolve, Delete, Forward, Name, Done, Cancel,
and the Name/Date/Server/Subject/Text/note labels), the `/mobinfo` block (`0x6a84b8`–`0x6a8acc`), the item-mall "%s points" (`0x6aca70`) and the
four-category rating window (`0x6aca9c`–`0x6acaf0`). The labels in the question window were CP949 Korean literals, not corrupt payloads.
The larger text pass on `sysmsg-uni.txt` is a data change, described in `english-text.md`.

**Commands** (`gm-and-commands.md` has the evidence and the privilege each needs). The English parser tests are wrong in five ways; the DLL
fixes them in place (**confirmed**, bytes checked):

| Bug | Cause | Fix |
|---|---|---|
| `/server off` sends "server on" | `PUSH 9` compares `/server on` on 9 bytes at `0x4728b7`, and that test comes first | `PUSH 10` |
| `/cwatch off` watches a player named "off" | `/cwatch ` (8 bytes, `0x470a7c`) is tested before `/cwatch off` (`0x470aac`) | swap the two 24-byte test sequences |
| `/eventadd`, `/eventrem`, `/partysummon` with arguments never sent | the English test compares one byte more than the literal, which ends in a space, so it matches only the bare word | corrected compare lengths, plus `/partysummon`'s argument offset |
| text arguments need double quotes (`/warning name "text"`) | the quote parser `0x46d5e0` copies only what is inside `"…"` | 19 call sites redirected to a parser that takes the rest of the line when there is no quote |
| `/mobinfo` and the faction words of `/bnotice` exist only in Korean | Korean literals only (`0x4706ba`; `REPE CMPSB` at `0x473f1f`, `0x473f39`) | English spellings (`/bnotice light|dark`) |

## The `0xF502` handler

The DLL replaces the network vtable fallback slot `0x6a6220` (original `0x551030`) with a wrapper that handles the new operator-chat packet
`0xF502` and forwards every other opcode to the original with the original `thiscall` arguments; it replaces the slot only when its four
bytes match. A new handler is therefore a new case in that wrapper. See `operator-listen.md` for the wire layout and display.

## Graphics

Optional and off by default: the `Direct3DCreate9` import (`0x6a5548`) is redirected only when a graphics setting asks for it, and device
vtable slots (`CreateDevice`, `Release`, `Reset`, `Present`, the state-block calls and the four draw calls) are patched in the native
table to add frame caps, borderless windowed mode, anisotropic filtering and metrics. Three timing wrappers sit on the map and world
loader calls (`0x42e87a`, `0x42f836`, `0x452643`). Details and measurements: `graphics.md`.

## Settings

| Section / key | Default | Effect |
|---|---|---|
| `[fixes] features` | `0,5,8,23,26,43,45,46,62` | comma list of region features (0–62) to switch on; an empty list installs no hook |
| `[fixes] anti_freeze` | on | the background-input behaviour above |
| `[fixes] road_messages` | on | the `0x2104` fix |
| `[fixes] english` | on | the 34 texts |
| `[fixes] command_fixes` | on | the GM command fixes |
| `[fixes] listen_chat` | on | the `0xF502` handler |
| `[fixes] log` | off | write a log of what was applied or refused |
| `[graphics] …` | off | frame caps, borderless, anisotropy, metrics (`graphics.md`) |

Unknown keys are logged and ignored. A missing file means defaults.

## What it does not do

It does not change data files (those go through the updater, `updater.md`), does not touch the server, and does not apply to any other
`game.exe`. Wall-climb and market-anywhere are not in it; they are byte patches that change gameplay and are applied only when named
(`client-patch-kit.md`).

## Checks that were run

A disposable client copy with the DLL and import loaded, started on a hidden desktop: the log listed all changes as applied and the game ran
and was closed. The visible effects (extraction hammer picking one gem, target HP percent, auto-dismount, Easy-mode question, revive text,
quick-slot swap, the options checkbox, default quick slots, anti-freeze with alt-tab while holding the camera button) are **live** checks.
Unit tests cover the settings parser, every byte-checked write and the graphics hooks (`graphics.md`).
