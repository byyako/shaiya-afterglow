# Tools and method

This page explains how the findings on this site's client pages were made, which tools were used and what they are for, and how to check a
claim yourself. The target is the US `game.exe` (build 2010-04-26, patch 108; SHA-256
`5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`). The tools are described, not offered for download; the only
download on the client pages is the patch kit (`client-patch-kit.md`).

## In short

- **Everything is static.** The client was read in Ghidra and with disassemblers; it was not run to learn how it works. The few facts that
  needed a running game are marked **live** and listed as open.
- **A claim is "confirmed" only when a function and address say so.** Everything else is *inferred* or **ref** (from an older Hex-Rays
  listing of a later client, matched here by constants and message ids).
- **Formats are checked against every file.** A parser that consumes each file of a type to its last byte is the test, not a sample.
- **You can check any claim:** the pages give addresses; the strings, imports and constants that led there are in the "How to find it"
  parts of the research pages.
- **Public sources are credited:** the community format notes by Castor, the Parsec library, and the 2012 patcher for the first byte
  patches.

## Method

1. **Find the code from something visible:** a string (`File Not Open [%s]`, `SAFile.cpp`, a sysmsg id), an import (`Direct3DCreate9`,
   `GetAsyncKeyState`), a constant (a table of floats, an opcode immediate), or a packet id seen in the packet log.
2. **Follow references** to the function, then up to a caller that explains why it runs (a UI dispatcher case, a network-vtable slot, the
   main loop). Opcode handlers are network-vtable slots (`0x6A5C20` + slot) and readers are the `0x57xxxx` functions; the dispatcher
   is `0x582e10` with 393 cases.
3. **Read the decompile,** and check anything that matters against the instructions: Ghidra's decompiler drops stack stores of constant
   lists and mis-types some floats. A finding read only in instructions is labelled **confirmed (disasm)**.
4. **Cross-check** against other evidence: a real packet in the log, a data file parsed to its last byte, the original server's behaviour,
   or the same function in the 2012 listing of a later client (**ref**).
5. **Write the status word with the claim.** A claim that rests on a guess about intent stays *inferred*; one that needs a game session is
   **live**.
6. **Stop rule:** if a question takes about 30 queries without reaching a confirmed function, it is written up as open with what was tried.

Evidence words: **confirmed** (a decompile of this `game.exe`), **confirmed (disasm)**, **ref**, *inferred*, **live**. Addresses are virtual
addresses in `game.exe`; for `.text`, `.rdata` and `.data` the file offset is the VA minus `0x400000` (raw offset equals RVA). Opcodes are
little-endian `u16`; sizes include the 2-byte opcode and exclude the transport's length prefix.

## Disassemblers and decompilers

- **Ghidra** (12.1) with a headless server mode: a script keeps the project open and answers requests (`dec`, `tree`, `refs`, `disasm`,
  `scalar`, `u32`) from files, so one start-up serves a whole session. The project holds `game.exe` (10,778 functions after analysis) and
  the reference programs (the original updater, a community editor). Ghidra scripts by purpose:
  - decompile or list around addresses (`DecompileAt`, `DisasmAt`), a function and its callees (`DecompileTree`), bulk decompiles
    (`DumpFunctions`);
  - references to an address or a string (`RefsTo`, `DecompileStrRefs`), vtable discovery (`FindVtables`), and functions that touch many
    of a set of structure offsets (`FieldUsers`);
  - repairs for what the auto-analysis misses: unresolved switch tables (`FindJumpTables`, `FollowJumpTables`), pointer-only functions
    (`DefineMissingCode`), a wrong non-returning flag (`FixNoReturn`);
  - naming the original server's functions from its debug symbols and marking `__thiscall` on class methods (`ApplyNames`,
    `SetThiscall`).
- **Disassembly with `objdump`** for single instructions and data bytes (`objdump -d -Mintel --start-address=… --stop-address=…`).
- **ILSpy** (command line) to read .NET programs: the 2012 community patcher was decompiled this way to learn what each byte patch does.
- **Reference material that is not run:** a closed-source community editor (Castor's shStudio) read statically, below; the Parsec library
  (public, MIT) for file layouts; a 2012 Hex-Rays listing of a later US client with auto names only.

## Python tools by purpose

| Purpose | Tool | What it does |
|---|---|---|
| Archive | `tools/sah.py` (client repository) | reads `data.sah` (`list`, `stats`, `check`, `extract`); integrity and dead-space report (`data-archive.md`) |
| Archive | `tools/repack.py`, `tools/swap_archive.py` (client repository) | rebuild the archive (dedupe as the client does, add and remove files, verify) and swap two archive pairs by renaming |
| Archive audits | `tools/unreferenced.py`, `tools/archive_access_audit.py`, `tools/texture_audit.py` (client repository) | files whose names appear nowhere (`cleanup.md`); lookup-structure audit and texture container checks (`graphics.md`) |
| SData | `tools/sdata_crypt.py`, `tools/seed_constants.py`, `tools/sdata_diff.py` (client repository) | SEED decrypt and encrypt of `.SData`, and a record-by-record comparison of two files |
| Formats | `tools/formats.py` (client repository) | parsers for every model, effect and world format, with an archive-wide `check` that must consume each file exactly (`formats.md`) |
| Animation | `tools/anim.py` (client repository) | skeletal animation as the client does it: `check` over every model and animation, posed `png` and `obj` export |
| Packets | `tools/dispatch_table.py`, `tools/packet_catalog.py` (client repository) | build the catalogue of handled and sent packets from Ghidra dumps (`client-packets.generated.md`) |
| Logs | `tools/clientlog.py` (client repository) | decodes the client's own packet log (`Log.txt`) against the catalogue, masking the password |
| Text | `tools/sysmsg.py` (client repository) | applies a message table to `sysmsg-uni.txt`, checking tags and `%` arguments (`english-text.md`) |
| Patching | `tools/patch_client.py` (client repository) | writes a patched `game.exe` copy from named patches, verifying original bytes and the file hash (`client-patch-kit.md`) |
| Public text | `tools/site_leakcheck.py` (client repository) | scans pages for private paths, hosts, names and over-long decompiler blocks before they are published |

The client can write its own **packet log**: a small `gsConfig.cfg` with a log level makes it dump every sent and received packet to
`Log.txt` (`client-packets.md`), which is how packet layouts are checked against real traffic.

## The SData viewer

A small Windows viewer (.NET Framework 4.8, no dependencies) opens an encrypted or decrypted `.SData`, detects the standard SEED wrapper,
requires the CRC-32 to match before showing anything, and never writes its input. It offers record and text search, a field view with byte
offsets, paged hex, file information (hashes of source and plaintext) and a CSV export that neutralises text starting like a spreadsheet
formula. Structured layouts exist for the EP4 Item, Skill, NpcSkill, Monster, Cash, NpcQuest, KillStatus and GuildHouse tables; anything
else falls back to hex and searchable text runs. Item ids are **one-based** bank and row positions; monster ids are **zero-based** record
positions. A successful parse proves byte boundaries, not every field's meaning. It is described here, not offered.

## The animation lab

An offline set of Python tools (standard library only) for EP4 skeletal animation: validate and read/write `.ANI` files byte-for-byte,
export to editable JSON and import back with a skeleton check, check the whole archive corpus, and make a self-contained animated HTML
preview with timeline, skeleton overlay and before/after comparison. `custom-animations.md` describes the format contract and what was
proven in game.

## A community editor as a format reference

Castor's shStudio (0.9.0.6, MFC and OpenGL, 32-bit, unpacked, with full RTTI) was read statically; it was never run. Its class names show
what it covers: archive open and pack (with the SEED cipher), the SData tables with explicit per-episode layouts, every model and
animation format, `.WLD` worlds, dungeons, heightmaps, server maps and the zone file, and an import path from the game database. It also
notes that `PriestTalk.SData` is "very likely not used by the game client", which the client's code confirms (`research-priest-packets.md`).
Because each format class has a loader reachable from its vtable, the quickest route to a format specification is: find the class's vtable,
follow its load virtual, and check the result against the game's own loader for the same extension.

## How to check a claim yourself

- Take the address from the page, open your own `game.exe` (verify the SHA-256 first) in Ghidra or `objdump` at that VA, and compare the
  bytes, immediates or call targets named in the claim.
- For a file format, parse every file of that type in your archive with the layout on `formats.md`; a layout is right when each file
  ends exactly at the parser's last read.
- For a packet, enable the client's packet log and compare a real frame with the layout on the packet pages.
- For a patch, apply it to a copy and compare the bytes: `client-patch-kit.md` lists each patch's original and new bytes.

## Sources and credits

Format layouts began with Castor's (castor4878) posts on elitepvpers; the Parsec library by matigramirez provided matching C# readers
and the SEED code port; the first byte patches come from Shaiya Patcher 1.0.1 (2012); the OpenShaiya archive provided later official US patches and clients for comparison. Everything else on these pages was read
from the 108 client itself.
