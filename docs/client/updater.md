# The updater and the patch files

This page describes the original `Updater.exe` of the US client (2009-01-29): the files it uses, its flow, the format of the `psNNNN.patch`
files it downloads, and its second, TCP transport. It then describes, in prose, a replacement updater that adds signatures, removals and
compaction. The original was read statically (MFC, WinINet and WinSock) and never run. Status words: **confirmed** (read in the binary
or checked on real patches), [*inferred*](glossary.md#inferred).

## In short

- **The original updater is a simple HTTP client:** it reads a host name from a text file, fetches a small version file, then downloads
  one `psNNNN.patch` per missing version and merges it into the archive.
- **A patch is an ordinary ZIP** holding a small archive in the same `.sah`/`.saf` format as the game's, plus optional loose files such
  as a new `game.exe`.
- **The original does no integrity or authenticity check:** plain HTTP, no signature. Anyone who can answer for the patch host can
  replace `game.exe`.
- **A replacement updater** keeps the same protocol (so old patches still work) and adds a required Ed25519 signature on every patch and
  on itself, removal lists, crash-safe application and compaction of dead archive space.
- **A TCP message transport also exists** in the original; which server spoke it is not known.

### Using this page

- **To see what a patch holds:** a `psNNNN.patch` is an ordinary ZIP; open it with any ZIP tool, then list the small archive inside with the client repository's `tools/sah.py list --sah update.sah`.
- **To start the game without the updater:** use a shortcut to `game.exe start game`, started in the client folder (see "Launching and a running game"); the `no-updater` patch of [the patch kit](client-patch-kit.md#no-updater-run-without-the-updater-default) does the same without the shortcut.
- **If you write a server or a patch host:** "Files the original updater uses" lists what the host must serve; the replacement updater section says what a signed one adds.
- **To check a claim yourself:** the flow is *inferred* from strings; the patch format is confirmed on real patches.

## Files the original updater uses

| File | Role |
|---|---|
| `UpdaterURL.txt` | host name of the patch server (missing → "Can't Find UpdaterURL.txt") |
| `Version.ini` | `[Version] CheckVersion`, `CurrentVersion` (108 here), `StartUpdate` = `UPDATE_START` while patching, `UPDATE_END` when done (a crash marker) |
| `UpdateVersion.ini` | downloaded from `http://<host>/Shaiya/UpdateVersion.ini`: `[Version] CheckVersion`, `UpdaterVersion`, `PatchFileVersion` (confirmed, `0x406b60`) |
| `UpdateVersion-mjw-.ini` | local override: if present, its values replace the downloaded ones (a developer/test hook) |
| `new_updater.exe`, `test_updater.exe` | self-update: `http://<host>/Shaiya/new_updater.exe`; `game.exe` waits for the updater window to close (`patches.md`, the wait at file offset `0xB6E5`) |
| `ps%04u.patch` | one patch per version: `http://<host>/Shaiya/patch/ps0109.patch` etc. |

## Flow (*inferred* from strings and the functions above)

1. Read `UpdaterURL.txt`, fetch `UpdateVersion.ini`. A `CheckVersion` mismatch → "The version does not match. Please install the client
   again".
2. `UpdaterVersion` newer → download `new_updater.exe`, restart into it.
3. For each version `CurrentVersion+1 .. PatchFileVersion`: download `ps%04u.patch` (resuming from the existing file length), apply
   it, bump `CurrentVersion`. Free disk space is checked first.
4. Start `Game.exe start game` (`0x402990`).

## Patch file format (confirmed on real patches)

`psNNNN.patch` is a **standard ZIP** (deflate) containing:

- `update.sah` + `update.saf`: a small archive in exactly the `data.sah/saf` format (offsets relative to `update.saf`, no free list);
  The client repository's `tools/sah.py list --sah update.sah` reads it as is.
- optional loose files copied into the client folder as they are (for example `game.exe` in `ps0064`).

The updater (`0x409560`) opens `data.sah` and `update.sah`, merges every patch file into `data.saf` (reusing blocks from the "rest
space" free list, see `data-archive.md`), and rewrites `data.sah`.

Checked against the public OpenShaiya archive (`archive.openshaiya.org`, prefix `shaiya-us/`):

- `ps0064.patch` (2009-07-22, 6.9 MB): 56 archive files (SData tables, the "yeastbunny" costume models and textures, helmet MLTs, two
  icons) plus `game.exe`.
- `ps0108.patch` (2010-05-17, 1.9 MB): only `Item/Item.SData`; it is byte-identical to the one in the 108 `data.saf`, which confirms the
  client is at patch level 108.

The archive also holds `clients/` (most `game.exe` versions 2007–2012), `original/` (the first US install) and `decompiled/` (the
2012 Hex-Rays listing of a later client used as a reference in these pages).

## Second transport: TCP messages (confirmed, `0x4044e0`)

Besides HTTP, a message handler takes 2-byte opcodes:

| Opcode | Payload | Action |
|---|---|---|
| `0x0204` | u32 latest version | equal to local → nothing to do; else start fetching the difference |
| `0x0205` | — | up to date, finish |
| `0x0102` | u32 size at +6 | start receiving `new_updater.exe` |
| `0x0104` | data | `new_updater.exe` chunk; after every 20 chunks reply `{u16 len=4, u16 0x0104}` |
| `0x0103` | — | updater transfer done → restart |
| `0x0201` | u32 patch no., u32 size at +6 | start receiving `ps%04u.patch` (resumes: size − existing length) |
| `0x0203` | data | patch chunk; ack `{4, 0x0203}` every 20 chunks |
| `0x0202` | — | patch done → apply |

Which server and port spoke this is not known (no evidence it is the login server). An HTTP-only updater is simpler.

## A replacement updater (described, not published)

The replacement is a small Windows program written in Rust with a plain Win32 window (status line, progress bar, log, **Start game** /
**Retry**, **Close**; `--cli` and `--check` use the console). It is not offered for download (same policy as the companion DLL,
`fixes-dll.md`). Its design:

**Protocol: unchanged from the original, so old patches work.** `UpdaterURL.txt` holds `host`, `host:port` or a full `http://…` (https
works too); it fetches `<host>/Shaiya/UpdateVersion.ini`, compares with `Version.ini`, downloads `<host>/Shaiya/patch/psNNNN.patch`
for each missing version, applies it, bumps `CurrentVersion`, then starts `game.exe start game` as a raw command line (see "Launching").

**Signed patches (required).** A patch can replace `game.exe` and DLLs, so a spoofed patch host would mean code execution on players'
machines; the original has exactly this weakness. Every patch comes with `psNNNN.patch.sig`, an Ed25519 signature over
`"shaiya-patch-v2\0" + CheckVersion + "\0" + lower-case file name + "\0" + SHA-256(patch)`. Binding the name means a signed `ps0109`
cannot be served as `ps0110`; binding `CheckVersion` means patches of an older client generation cannot be replayed after it is bumped.
The rule for the publisher: never sign a different file under an already published version number; publish fixes as the next version.
The public key is compiled into the updater, and a build without a key refuses all patches. Tests: valid signatures install; a one-byte
change, a missing signature, a foreign key and a signature from another `CheckVersion` are all refused, and the client stays at the last
good version.

**Applying a patch.** New contents are appended to `data.saf` and synced before the index is replaced (temp file + rename, previous
index kept as `data.sah.bak`), so an interrupted update never leaves a broken index. Loose files are written through a temp file and
rename; the running updater is never overwritten (written as `<name>.new`). Application never unpacks more than its index references and
streams loose files to disk (no size is taken from the ZIP header).

**Removals and compaction.** A patch may carry `remove.txt`: archive paths to drop, one per line (`#` lines and a `path` header are
skipped). They are dropped **before** the patch's own files are added; a listed path the client does not have is only reported. Paths are
Unicode in lists and patches and **code page 949 in the index** (for example `Item/3DO/10051원.3do`). Removed and replaced files leave
dead space, so after installing (and on every start while it is still there) the updater **compacts** `data.saf` once at least 64 MB are
unused, the game is closed and the disk has room for a copy of the live data plus 256 MB. Compaction copies the live ranges in their old
order into a temporary `data.saf`, drops shadowed duplicate entries and the free list, writes a temporary index, syncs both, creates a
commit marker and renames both over the originals. At every start the updater completes a committed compaction or deletes an
uncommitted one, so index and archive always match. Problems (no space, game running) are logged and retried next time; the game works
either way. A test on a copy of the stock 108 archive installed a `ps0109` with removals and restored files and ended with the same
19,939 files and contents as a hand-cleaned archive.

**Self-update.** Each build has an `UpdaterVersion`. If `UpdateVersion.ini` names a higher one, the updater downloads
`new_updater.exe` and its signature, verifies it, renames itself to `<name>.old` (allowed for a running exe on Windows), puts the new
file in its place, starts it with the same arguments plus `--self-updated` and exits; the new one deletes `<name>.old`. The signature
covers the name `shaiya-updater-<N>.exe`, so an updater signed as version N cannot be offered as a higher version and a patch
signature is never valid for an updater. A restarted updater never self-updates again in that run, so a build with a wrong version
number cannot loop; `--no-self-update` skips it. Tests refused a v2 build offered as v3 and a correctly signed v3 with one byte
appended.

**Window behaviour.** The embedded manifest declares `asInvoker` (without it Windows treats any exe with "updater" in its name as an
installer and asks for administrator rights), Common Controls 6 and DPI awareness. Closing the window mid-download is safe (`.part`
files; `StartUpdate=UPDATE_START` makes the next run resume). Not yet built: download resume.

## Launching and a running game (confirmed, decompile of `0x40b1a0`)

- **What `game.exe` checks:** first the single-instance mutex and the `GAME`/`Shaiya` window (both skipped by the `multi-client`
  patch), then `strncmp(lpCmdLine, "start game", 10)`. Anything else starts `Updater.exe` from the current folder (with three extra
  arguments for region 5) and exits; `new updater` as the command line runs the original's own updater swap (`new_updater.exe` →
  `Updater.exe`, killing a "Shaiya Updater" dialog). So **a shortcut to `game.exe start game` (start in the client folder) skips the
  updater**, also for a second client with the multi-client patch.
- **Quoting bug:** a launcher that passes the argument through an API that quotes it because of the space delivers `"start game"`
  with the quotes to `lpCmdLine`; the compare fails and an unpatched `game.exe` tries to start `Updater.exe` instead. The replacement
  passes the raw command line. A client with the `no-updater` patch hides this.
- **A running game:** the replacement refuses to install a patch while a `game.exe` from its folder runs ("close every Shaiya window,
  then press Retry"), checked by trying to open `game.exe` for writing, which Windows refuses (sharing violation 32) while it runs. It
  checks before the first download and again before each patch is applied. When there is nothing to install, Start game works as usual.
