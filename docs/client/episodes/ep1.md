# Episode 1

Episode 1 is the earliest client in the catalogue (US patch 0), and the oldest build this site has read. This page collects what is known: one client file read from its headers, one catalogue date, and very little else. Little is documented publicly about this episode, so the page is short.

## In short

- The earliest US `game.exe` in the OpenShaiya catalogue is dated 2007-12-18 (catalogue date, patch 0).
- A public collection of client builds holds an EP1 `game.exe` of the same size (2,289,664 bytes). It is linked four days earlier. It is not the same file: its checksum differs from the catalogue's patch 0 and patch 2 files.
- That build already imports `d3d9.dll`, the same graphics library as the EP4.5 client this site studies.
- No content list for EP1 was found in a public source. Nothing here says what EP1 changed for players.

### Using this page

- **If you write a server:** nothing on this page changes your work. The server side of the early client is on [the EP3 server pack page](../../original/ep3.md), which compares an older pack with EP4.5.
- **If you patch or mod the client:** treat EP1 as a different client from the one in this kit. Do not apply addresses from the other client pages to it.
- **To check a claim yourself:** read the PE header and import table of the file (see [Tools and method](../tools-and-method.md)); the facts below need nothing more.

## Releases

| Region | Date | Patch | Source | Status |
|---|---|---|---|---|
| US | 2007-12-18 (catalogue date) | 0 | the OpenShaiya archive, folder `shaiya-us/original` | **ref** |
| US | "open beta since 18 December 2007" | — | one wiki mirror, seen only as a search snippet | *unverified* |
| Korea | earlier than the US; no date found | — | — | *unverified* |

The wiki snippet agrees with the catalogue date, which is the only reason to mention it. A claim that the game was launched in 2006 by its Korean developer and later published in the West by a Western publisher also appears in one snippet: *unverified*.

## What changed for players

Nothing is documented for this episode in any source read. Level cap, classes and zones for EP1 are not known here. A late-2008 European beta interview gives a 60-level cap (see [Episode 3](ep3.md)), but that article does not name the episode, so it says nothing about EP1.

## What changed in the client

- The EP1 build imports `d3d9.dll`, `ijl15.dll` (a JPEG library), `VERSION.dll`, `WINMM.dll`, `WS2_32.dll`, `DINPUT8.dll`, `DSOUND.dll` and `DDRAW.dll`. **confirmed (file)**
- It adds `OLEPRO32.DLL` to that list, which the later builds do not import. **confirmed (file)**
- It was linked with Visual C++ linker 6.00 and has a `.sxdata` section; the EP4.5 build uses linker 7.10 and has none. **confirmed (file)**
- It is not packed or protected. **confirmed (file)**

## Builds known

| Label | Link date (UTC) | Size (bytes) | SHA-256 | Where it comes from | Status |
|---|---|---|---|---|---|
| EP1 `game.exe` | 2007-12-14 09:35:52 | 2,289,664 | `9f22df294e4adf24…` | a public collection of client builds | **confirmed (file)** |
| US patch 0 `game.exe` | not read | 2,289,664 | not read | the OpenShaiya archive, folder `shaiya-us/original` | **ref** (catalogue size and date only) |

Same size, different checksum, linked four days before the catalogue's first patch: the two are not the same file. That they are neighbouring builds is *inferred*. The link time is the compiler's output and says when the file was linked, not when it was released. The label "EP1" is the collection's name for the file: *unverified*.

## Compared with this client

- Both import `d3d9.dll` and the same audio, input and network libraries. **confirmed (file)**
- This client is [US patch 106](ep4-5.md), about two years and four months later and 913 kB larger. **ref** ([US patch history](us-patches.md))

## Server side

No EP1 server pack was read. The oldest pack studied on this site is described in [the EP3 server pack page](../../original/ep3.md). **ref**

## Sources

- OpenShaiya archive catalogue (the US `game.exe` history), public archive of Shaiya patches.
- A public collection of client builds (the EP1 file).
- A Shaiya wiki mirror (search snippet only, page not readable).

## Open

- Which content the first release carried, and its level cap.
- Whether the Korean build that preceded the US one is on hand anywhere.
- The size match to patch 0 suggests the collection's file is a near-neighbour build. Reading patch 0 itself (not downloaded) would settle the difference.
