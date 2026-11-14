# The client: game.exe

These pages cover the other end of the connection: how the Shaiya client talks to the servers, what it does with each packet it receives, how its engine and game state work, and what it can be made to do. They come from static analysis of the client's `game.exe` (disassembly and decompilation), plus checks against a real client where a page says so.

!!! info "The client these notes describe"
    | | |
    |---|---|
    | Build | US `game.exe`, built 2010-04-26 |
    | Patch level | 108 (`ps0108.patch`), Episode 4.5 |
    | SHA-256 | `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6` |

    Other builds, other regions and later patches may differ. Where a page compares the client with the server pages, the server pages are about the EP4.5 `ps_game.exe` and `ps_login.exe`.

## In short

- **Network:** the connection, the cipher, the framing and the dispatcher, then every packet the client handles (425) and sends (245), field by field.
- **Engine and game state:** start-up, the archive, text tables, windows, rendering and input, and the structures that hold the player, entities and inventory.
- **Game systems:** auction house, personal shop, duels, guild wars, obelisks, union parties, item mall and scripted monsters, each from the client's side.
- **Changing the client:** a patch kit for your own `game.exe`, plus a description of the companion DLL, the updater and the tools.

## Learn the client

A reading path, from the connection up to the tools:

1. **This page**, then [Words used on these pages](client/glossary.md). Which client, what the status words mean, how addresses and opcodes are written, and what the terms mean.
2. [How the client talks to the servers](client/re-network.md): connect, handshake, cipher, framing, dispatch, send; one packet each way end to end.
3. [Packet handling](client/client-packets.md): the dispatcher, the read routines and their underflow report, the built-in packet log.
4. [Engine internals](client/re-engine.md): start-up, the main loop, the archive, text tables, the UI windows, rendering and input.
5. [The client's game state](client/re-gameplay-state.md): the player and entity structures, inventory, how SData is used.
6. [Systems the client supports](client/client-systems.md): the map of every packet family to a game system.
7. [World packets](client/client-world-packets.md), then [Social packets](client/client-social-packets.md): what each in-world packet does on screen, field by field.
8. One system page, for example [Duels](client/duels.md): a whole feature from request to result.
9. [File formats](client/formats.md) and [the archive](client/data-archive.md): how models, maps and tables are stored and found.
10. [Episodes at a glance](client/episodes/index.md): the other Shaiya clients, and how this one compares.
11. [Tools and method](client/tools-and-method.md): how the findings were made, and how to check one yourself.
12. [Patch kit](client/client-patch-kit.md) and [the companion DLL](client/fixes-dll.md): how the client can be changed safely, and what each change touches.
13. [Every opcode against the original server](client/packet-opportunities.md) and the research notes: what the client handles, and what is still [open](client/re-open-items.md).

Advice for server authors is on the [Shaiya Afterglow](client/afterglow/index.md) tab.

## Sections

| Section | What it covers |
|---|---|
| Start here | [Client facts for server developers](client/CLIENT-FACTS.md) (corrections, request and reply facts, what the client filters itself) and [the systems map](client/client-systems.md) |
| Network | Connection and cipher, packet handling, the generated catalogue, world and social packets, every opcode against the original server |
| Engine | Engine internals; graphics and performance measurements |
| Gameplay systems | The client's game state, request and result messages, GM commands, and one page per system |
| Data formats and archive | Model, animation, map and effect formats; the `data.sah`/`data.saf` archive; unused files; custom animations; the English text pass; the SData comparisons and generation |
| Patching and the companion DLL | The patch kit, the companion DLL, operator chat copies, the updater and its patch files |
| Episodes (other clients) | [Episodes at a glance](client/episodes/index.md), one page per episode, and the US patch history |
| Tools and method | How the analysis was done and the tools it used |
| Research notes | Open items, dated research answers, and the quality-of-life feasibility study |

## Downloads

One download: the [patch kit for `game.exe`](client/client-patch-kit.md), a Python script and lists of bytes that patch your own copy ([game-exe-patch-kit.zip](game-exe-patch-kit.zip)). It contains no game file. The companion DLL, the updater and the tools are described on their pages and are not offered for download.

## Conventions

- Status words: **confirmed** (read in a decompile of this `game.exe`, with function and address), **confirmed (file)** (read in a binary or file of another episode, named on its page), *unverified* (reported by a source, not checked), **confirmed (disasm)** (read in disassembly or data bytes only), **ref** (taken from the server pages of this site, or from a 2012 Hex-Rays listing of a later client and matched here), *inferred* (follows from what was read), and **live** (needs a game session to check).
- Opcodes are written as `0x0702`; the high byte is the family. Field types are `u8`, `u16`, `u32` (little-endian) and `char[n]`. Packet sizes include the 2-byte opcode and exclude the length prefix.
- Function and data addresses are virtual addresses in this `game.exe`; for `.text`, `.rdata` and `.data` the file offset is the address minus `0x400000`.
- Every page starts with **In short**, a plain-language summary, then the detail.
