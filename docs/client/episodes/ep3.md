# Episode 3

Episode 3 is the oldest episode that has both a client build and a server pack on hand. The public record is thin: one German magazine interview from late 2008 and a Russian-region client linked in 2009.

## In short

- A European closed beta began in early November 2008 and the level cap in that interview was 60. The article does not name the episode.
- A Russian-region client labelled `ep3` in the OpenShaiya archive was linked on 2009-09-03. Regional builds lag the original release, so that date does not date Episode 3 itself.
- The EP3 server pack studied on this site is the same program set as EP4.5, smaller. Its notes are on a server page.
- No dated release for "Episode 3" itself was found.

### Using this page

- **If you write a server:** read [the EP3 server pack page](../../original/ep3.md). It lists what EP4.5 added over EP3.
- **If you patch or mod the client:** the Russian build is the only EP3-labelled client file; it is a different region from this kit's client.
- **To check a claim yourself:** read its PE header and imports ([Tools and method](../tools-and-method.md)).

## Releases

| Region | Date | Patch | Source | Status |
|---|---|---|---|---|
| Europe | closed beta early November 2008 (interview dated 2008-10-31) | — | a German games magazine, interview with a publisher's product manager | **ref**; episode unnamed, so the link to EP3 is *unverified* |
| Russia | build linked 2009-09-03 | 1 (catalogue) | the OpenShaiya archive, folder `shaiya-ru/clients/ep3` | **confirmed (file)** for the link time |
| any | Episode 3 release date | — | none found | — |

The catalogue date of the Russian rows is the day the archive imported them (2022-06-26), not a release date; only the file's link time dates that build.

## What changed for players

- Top level 60, reachable in about five months at about six hours a day, in the late-2008 European beta. **ref** ([PC Games interview](https://www.pcgames.de/Shaiya-Spiel-32570/Specials/Shaiya-Interview-zur-Closed-Beta-des-kostenlosen-Online-Rollenspiels-Teil-2-665462/)); which episode that beta ran is *unverified*.
- The server pack labelled EP3 keeps the same level cap of 60, has maps 0 to 44 only, and has no Lua monsters, party instances or auction house. **ref** ([EP3 server pack](../../original/ep3.md))

## What changed in the client

- The Russian `ep3` build imports `d3d9.dll`, `ijl15.dll`, `WINMM.dll`, `WS2_32.dll`, `DINPUT8.dll`, `DSOUND.dll` and `DDRAW.dll`: the same library set as the client in this kit. **confirmed (file)**
- It is linked with the 7.10 linker, has the sections `.text .rdata .data .rsrc`, and is not packed. **confirmed (file)**
- It is 3,166,208 bytes, 36,864 bytes smaller than this kit's client. **confirmed (file)**
- Nothing else about its code has been read.

## Builds known

| Label | Link date (UTC) | Size (bytes) | SHA-256 | Where it comes from | Status |
|---|---|---|---|---|---|
| Russian `ep3` `game.exe` | 2009-09-03 08:20:28 | 3,166,208 | `9bba1df1a2966e99…` | the OpenShaiya archive, folder `shaiya-ru/clients/ep3` | **confirmed (file)** |

The folder label "ep3" is a community label: *unverified*. US patch 71 carries the same calendar date (2009-09-03) but is a 3,223,552-byte file, so the Russian build is not the US one. **ref** ([US patch history](us-patches.md))

## Compared with this client

- Same graphics API import and linker generation as [the EP4.5 client](ep4-5.md). **confirmed (file)**
- The EP3 server pack shares opcode groups, connection states and damage formulas with the EP4.5 server the client pages describe. **ref** ([EP3 server pack](../../original/ep3.md))

## Server side

[The EP3 server pack](../../original/ep3.md): programs linked December 2007 to January 2008, smaller than EP4.5, with the differences listed there. **ref**

## Sources

- PC Games (Germany), Shaiya closed beta interview, part 2, 2008-10-31: https://www.pcgames.de/Shaiya-Spiel-32570/Specials/Shaiya-Interview-zur-Closed-Beta-des-kostenlosen-Online-Rollenspiels-Teil-2-665462/
- OpenShaiya archive catalogue.

## Open

- A dated source that names Episode 3.
- Whether the Russian build is the Episode 3 client or simply the build one Russian service ran at the time (needs a dated Russian source).
- Code comparison between this build and the EP4.5 client (not done).
