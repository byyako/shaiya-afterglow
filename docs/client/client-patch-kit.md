# Patch kit for game.exe

A checked, reversible byte patcher for the US Shaiya EP4.5 client. It writes a patched **copy** of your own `game.exe` that starts
without the updater, can run more than once, shows a message the original code could not, and, only when you ask, makes a few more
changes. This page has the download, how to use it, what each patch does in the code, and every byte it changes.

## In short

- **What it is:** a small Python script and a list of byte changes. You run it on your own copy of `game.exe`; no game file is part
  of the download.
- **Which file:** only the US `game.exe` with SHA-256 `5f045bcb…` (below), patch 108. The script refuses any other file, so it cannot
  damage a different build.
- **What it changes by default:** four patches (no updater needed, no wait for the updater window, several clients at once, the
  "a road to … will be opened" messages). Four more are applied only when you name them: keep running without focus, the version
  number sent at server select, and two that change how the game plays (personal shop outside the market, walking up steep terrain).
- **How it is careful:** it writes a new file and never the input, undoes every patch it knows in memory and checks the original hash
  before and after, checks each original byte before writing it, prints every byte it changes, and can undo any patch.
- **How far it was checked:** every patch was read in the disassembly of this build. The kit gives byte-identical files to an
  independent script that applies the same patches. `no-updater` and `multi-client` are in daily use; for the
  others the status under each patch says what was checked. Try a copy first.

### Using this page

- **To patch your own `game.exe`:** download the kit (below), run `python3 patch_game_exe.py status game.exe` to see what a file carries, `python3 patch_game_exe.py apply game.exe game_patched.exe` to write a patched copy with the default set, and `python3 patch_game_exe.py revert game_patched.exe game_clean.exe all` to undo it. The tool prints every changed range and the hash of the copy. Detail is under "Using it".
- **To add an opt-in patch:** name it after the file names, for example `apply game.exe game_patched.exe anti-freeze`; "Each patch" says what it does first.
- **If you write a server:** the patch that matters most is `market-anywhere`, which removes the client's market-map check; see [Personal shop](personal-shop.md).
- **To check a claim yourself:** every patch has its addresses and byte list on this page; open the build in a decompiler as [Tools and method](tools-and-method.md) describes.

## Download

[game-exe-patch-kit.zip](../game-exe-patch-kit.zip): `patch_game_exe.py` (the patcher), `patches.json` (the byte lists) and
`README.txt`.

SHA-256 of the zip: `1b2525566f5eb5756979e3a50a337d734b05897996a2add830c9a159e51d28f3`

It needs Python 3.8 or newer and nothing else. Nothing in it is a game file.

## The build

The patches are for exactly one build of `game.exe`:

| | |
|---|---|
| SHA-256 | `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6` |
| size | 3,203,072 bytes |
| built | 2010-04-26 (PE timestamp `0x4bd5001e`), US version, patch 108 |

In this file the virtual address (VA) of a byte in `.text`, `.rdata` and `.data` is its file offset plus `0x400000`. The addresses on
this page and on the other client pages are specific to this build; they do not apply to other versions.

## Using it

```
python3 patch_game_exe.py list   [game.exe]                        # every patch, and its state in a file
python3 patch_game_exe.py status game.exe                          # which patches are applied, the login address
python3 patch_game_exe.py apply  game.exe out.exe                  # the default set
python3 patch_game_exe.py apply  game.exe out.exe <id> ...         # the default set plus the named patches
python3 patch_game_exe.py apply  game.exe out.exe --login A.B.C.D  # also write a login server address
python3 patch_game_exe.py revert game.exe out.exe <id> ...         # undo the named patches
python3 patch_game_exe.py revert game.exe out.exe all              # undo everything, the login address too
```

Examples:

```
python3 patch_game_exe.py apply game.exe game_patched.exe
python3 patch_game_exe.py apply game.exe game_patched.exe anti-freeze --login 203.0.113.5
python3 patch_game_exe.py revert game_patched.exe game_clean.exe all
```

`apply` with no ids applies the default set (`no-updater`, `skip-updater-wait`, `multi-client`, `road-messages`). Patches that the input
already carries stay, so you can add one patch to a file you patched before. The two gameplay patches (`market-anywhere`,
`wall-climb`) are applied only when named; there is no "all" shortcut for them. `--login` writes a login server address (an IPv4
address, see [the login address](#the-login-address)). The output is a new file: the tool refuses to write over its input, and over
any other existing file unless you add `--force`.

After each run the tool prints every range it changed as before and after bytes, then the SHA-256 of the copy. For the default
set with the original login address that hash is `df8c65506bc05406ec747071f9f7a7bf503f8764b5c870991a2d522c69ac643b`, and the tool says so when it matches.

## Safety

Every command runs the same checks, in this order.

1. The input is read and every range of every patch is classified as *original*, *patched* or *other*, and so is the login address
   (original, a valid NUL-padded IPv4 address, or other). Any *other*, or a patch that is only half applied, is refused, naming the
   offset and the bytes found.
2. The tool rebuilds the original image: it writes back the original bytes of every patched range and the original login address. That
   image must have the size and SHA-256 above, or the tool refuses ("not the US 2010-04-26 `game.exe`, patch 108, or it has edits
   this kit does not know"). A `game.exe` that has been given an extra import for another DLL is caught here too.
3. The result is built from the original image: the patches already applied, plus (`apply`) or minus (`revert`) the named ones.
   Just before each range is written, the bytes there must be its original bytes.
4. The output path must not be the input and must not exist (unless `--force`). The file is written.
5. The output is read back and classified again: exactly the intended patches must be patched and nothing may be *other*; undoing
   them must again give the original hash. If this fails the output is deleted and the tool exits with an error.

Exit codes: 0 done, 1 refused (the reason is printed), 2 usage error. A full `revert … all` gives back the original file byte for byte.

## Each patch

Status words are the ones used on the other client pages: **confirmed** (read in this `game.exe`), **confirmed (disasm)** (disassembly
or data bytes only). "The old patcher" is the 2012 *Shaiya Patcher* (see
[Origin](#origin)).

### `no-updater`: run without the Updater (default)

The startup function at `0x40b1a0` requires the command line to begin with `start game`, which only `Updater.exe` passes; without it
the client starts the updater instead and quits. The patch turns the `jz` at `0x40b97d` into an unconditional `jmp`, so the argument
is treated as present and the client runs when started directly. See [the updater](updater.md) for what the argument is for.
Status: confirmed; used in play.

### `skip-updater-wait`: no wait for the updater window (default)

Also in `0x40b1a0`: after the self-update step (`new_updater.exe`) the client polls for a window titled "Shaiya Updater" for up to
10 seconds. The `jz` at `0x40b6e5` becomes a `jmp`, so the wait is skipped. It is only reached during self-update. Status: confirmed
(disasm). The old patcher applied it by accident; see [Origin](#origin).

### `multi-client`: several game windows (default)

Two checks in `0x40b1a0` stop a second client. After `CreateMutexA`, `GetLastError() == ERROR_ALREADY_EXISTS` shows an "ERROR!!!" box
and quits; the `jnz` at `0x40b61f` becomes a `jmp`. Then `FindWindowA("GAME", "Shaiya")` finding a window shows an "ERROR!" box and
quits; the `jz` at `0x40b643` becomes a `jmp`. Both are needed. Status: confirmed; used in play.

### `road-messages`: the "a road to … will be opened" lines (default)

Packet `0x2104` (`u16 linkId, u8 oldState, u8 newState`) is read by `0x57fe30` and handled at `0x54fcc0`. The handler compares a
value with 1010 to 1021 (`ADD EAX,-0x3f2; CMP EAX,0xb`) and prints the system messages 5374, 5373, 5372 and 5371 for the four
road links, but it loads the **new state byte** (`MOVZX EAX, byte [ESP+0x10]` at `0x54fcd5`), which can never be that large, so the
lines never show. The patch loads the 16-bit link id instead (`MOVZX EAX, word [ESP+8]`), five bytes of the same length. A server
that numbers its four roads 1010, 1011, 1020 and 1021 then gets the line on every `0x2104` for those ids, so it should send the
packet only when a road opens. See [obelisks and roads](obelisks.md). Status: confirmed (disasm); not seen in play, because no
server sends those link ids yet. The companion DLL writes the same five bytes at run time ([the companion DLL](fixes-dll.md)).

### `anti-freeze`: keep running without focus (opt-in)

The window procedure at `0x409640` stores `wParam` of `WM_ACTIVATEAPP` in an "app active" flag at `0x701b08`, and the original
client pauses while it is 0. The patch replaces that store (`0x409710`, six bytes) with `nop`s and starts the flag at 1 in `.data`
(`0x701b08`), so the client always believes it is the active application and keeps running when you switch away. **Caveat:** input
keeps being read without focus, so keys typed in another program can reach the game and a held mouse button stays held; the
companion DLL's version of this change also changes the two DirectInput polls (see [the companion DLL](fixes-dll.md)). Status:
confirmed.

### `no-version`: send client version `0xFFFFFFFF` (opt-in)

When a world is chosen, the client (function `0x4dca00`) sends `0xA202` with the version `0x10042600`, a constant in the code. The
patch changes that constant at `0x4dcce7` to `0xFFFFFFFF`. It is only useful for a login server that compares the version; for any
other it does nothing. Status: confirmed (disasm).

### `market-anywhere`: personal shop outside the market (opt-in, gameplay)

In the game-scene update (`0x43c000`), when the action state `0x21e546c` is `0x10` (opening a personal shop), the original sends
the sit/stand motion `0x0506` only in region 3, or on map `0x2a` (or `0x23` / `0x24`, depending on the faction flag at
`0x21e4942`); elsewhere it plays `ui_error001.wav`. The `jz` at `0x441f11` becomes a `jmp`, so `0x0506` is always sent. This is a
change of the rules of the game: **the server must enforce the map rule itself.** See [personal shop](personal-shop.md). Status:
confirmed.

### `wall-climb`: walk up steep terrain (opt-in, gameplay)

Also in `0x43c000`, a block samples the ground at ±0.1 around the player, normalises the result and compares it with a threshold (a
terrain-slope check). The original skips the block only when a flag in the player object is set. The `jnz` at
`0x4428e6` becomes a `jmp`, so the block is always skipped. This changes movement for the player who patches; a server that checks
movement may correct or reject it. Status: confirmed (disasm).

## The login address

`--login A.B.C.D` writes an address into the 16-byte slot at `0x6a5b34` (file offset `0x2a5b34`), which holds the login server address
for region 6 (English), `174.142.118.105` in the original, followed by a NUL. The address is written as text and NUL-padded to 16
bytes. Only a dotted IPv4 address is accepted; whether a host name would work in that slot has not been checked. `revert … all`, or
`revert` with `--login-original`, writes the original back. A later `apply` on a patched file keeps the address in it unless you give
a new one.

## The byte lists

Every change of every patch, as in `patches.json` (`before` is the original, `after` the patched bytes):

#### `no-updater`

```text
VA 0x40b97d  file offset 0xb97d  2 bytes
  before 0F 84
  after  90 E9
```

#### `skip-updater-wait`

```text
VA 0x40b6e5  file offset 0xb6e5  1 byte
  before 74
  after  EB
```

#### `multi-client`

```text
VA 0x40b61f  file offset 0xb61f  1 byte
  before 75
  after  EB
VA 0x40b643  file offset 0xb643  1 byte
  before 74
  after  EB
```

#### `road-messages`

```text
VA 0x54fcd5  file offset 0x14fcd5  5 bytes
  before 0F B6 44 24 10
  after  0F B7 44 24 08
```

#### `anti-freeze`

```text
VA 0x409710  file offset 0x9710  6 bytes
  before 89 35 08 1B 70 00
  after  90 90 90 90 90 90
VA 0x701b08  file offset 0x301b08  1 byte
  before 00
  after  01
```

#### `no-version`

```text
VA 0x4dcce7  file offset 0xdcce7  4 bytes
  before 00 26 04 10
  after  FF FF FF FF
```

#### `market-anywhere`

```text
VA 0x441f11  file offset 0x41f11  1 byte
  before 74
  after  EB
```

#### `wall-climb`

```text
VA 0x4428e6  file offset 0x428e6  2 bytes
  before 0F 85
  after  90 E9
```

#### The login address

```text
VA 0x6a5b34  file offset 0x2a5b34  16 bytes
  original 31 37 34 2E 31 34 32 2E 31 31 38 2E 31 30 35 00
```

## What the kit does not do

- It does not add the import section that makes `game.exe` load the companion DLL: without the DLL Windows refuses to start the game.
  What that import is and what the DLL does is written up in [the companion DLL](fixes-dll.md). The DLL itself is not published.
- It has none of the DLL's changes (region features, English texts, GM command fixes, the improved anti-freeze, the graphics
  controls, the `0xF502` handler). Those are hooks into running code, not byte patches.
- It does not include the old patcher's "Change Font", and not its header stamp (the old patcher wrote a text stamp over the
  `e_lfarlc` field of the DOS header on every save; this kit leaves the header alone).
- It does not make `game.exe` talk to any particular server beyond the login address above, change `data.saf`, `sysmsg-uni.txt` or
  any other data file, or touch a server. Nothing in the download is a game file.

## Undoing

`revert game.exe out.exe all` on a patched file gives back the original file, byte for byte (the tool prints the original hash and
says so). You can also just keep your original `game.exe`, since the tool never changes its input.

## In other episodes

- The file a public collection labels "EP4" is this kit's client (the US patch 106 file) with eight byte ranges edited, among them a flag that turns the updater off and a changed login address; it is not a separate episode. **confirmed (file)** ([Episode 4](episodes/ep4.md))

## Origin

The patches were first published in **Shaiya Patcher 1.0.1** (2012), a program that finds each patch
by a byte signature and writes a fixed change. Its options were Multi-Client, Remove Updater, Anti-Freeze, Market Anywhere, Wall
Climb, No Version, Change IP and Change Font. Each patch was read again in this build here, and this kit applies them at fixed offsets
with the checks above instead of searching for signatures. `skip-updater-wait` is a patch the old patcher applied by accident: its
Multi-Client option matches the first of nine similar byte sequences, so running the patcher a second time on an already patched file
hit the next one, which is this wait. `road-messages` is new and was found here.
