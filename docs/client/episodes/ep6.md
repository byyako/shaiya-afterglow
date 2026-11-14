# Episode 6

Episode 6 raised the level cap to 80 and added the Mystra islands, with a civil-war storyline. It is the best documented later episode here: three news sources agree on the date and the cap, and several client builds of the era are on hand. The page is for people comparing it with the US EP4.5 client this site studies.

## In short

- **Launch.** US service, announced 2012-09-24 and reported 2012-09-25, as "Trinity Artifact" (**ref**, three news sources).
- **Level cap 80** (**ref**, same sources).
- **Content.** Two new islands with enemies and dungeons, a Threemas versus Risen civil-war theme, new PvP skills and equipment sets, new dungeons (**ref**).
- **Client builds.** Four unpacked builds from 2013 to 2016 are on hand, one of them a dump. The facts here are file headers (**confirmed (file)**) plus what a server page read in the dump (**ref**).
- **Skill levels.** The EP6 client's skill lookup accepts levels 1 to 15 (**ref** [server page, section 6](../../original/ep5.4.md#6-the-client-side-checked-in-the-ep6-client)).

### Using this page

- **If you write a server:** the server page above lists EP6 client behaviour checked against EP5.4 packets; read it before assuming EP5.4 packets are unchanged. See also [what a server must do for this client](../afterglow/server-checklist.md).
- **If you patch or mod the client:** the EP6 builds are a different generation of the executable (linker 9.00 against 7.10 here). Do not reuse offsets.
- **To check a claim yourself:** [Tools and method](../tools-and-method.md).

## Releases

| Region | Date | Patch | Source | Status |
|---|---|---|---|---|
| US (publisher Aeria Games) | 2012-09-24 announced, 2012-09-25 launched | catalogue: patch 200, catalogue date 2012-09-25 | three news articles, OpenShaiya catalogue | **ref**; the link between patch 200 and the launch is *inferred* |
| Phoenix distribution | patch 229 in the catalogue; "ep6.3" folder label | 229 | OpenShaiya archive, folder `shaiya-phoenix/clients/ep6.3` | label *unverified*; build link date 2013-05-21 is **confirmed (file)** |
| Spanish and Portuguese services | build link dates 2016-01-21 (ES patch 171), 2016-01-27 (PT patch 182) | 171 / 182 | a public collection of client builds | **confirmed (file)** for link dates; launch dates not known |

The US `game.exe` in the catalogue drops from 3,651,072 B to 2,390,528 B at patch 200, on the date of the launch. That size change is a catalogue fact (**ref**); what it means (a restructured executable, a different build) was not checked because patch 200's file was not read (**open**).

## What changed for players

- Level cap 80 (**ref** [F2P News](https://www.f2p.com/shaiya-launches-episode-six-trinity-artifact/), [MMOReviews](https://www.mmoreviews.com/?p=28623), [MMOHuts](https://mmohuts.com/news/shaiya-announces-trinity-artifact-expansion/)).
- Two new islands with enemies and dungeons (**ref**, same sources).
- Threemas versus Risen civil-war theme (**ref**).
- New PvP skills and equipment sets, new dungeons (**ref**).
- Races and classes added: not stated (—).

## What changed in the client

- **Graphics API.** All four builds import `d3d9.dll`; none imports `d3d8.dll` or `d3d11.dll` (**confirmed (file)**).
- **Packer.** None of the four has protector sections (**confirmed (file)**). The build labelled `game.ep6.v1` has only `.text`, `.rsrc` and `.idata` sections and one import entry per function: its import table was rebuilt, which points to a dump of a loaded image (*inferred*). The Spanish and Portuguese files carry an extra `.idata` section while catalogue ES 171 is 2,522,560 B, so the files on hand are likely unpacked forms (*inferred*).
- **Linker.** 9.00 in all four (**confirmed (file)**); the EP4.5 client here is 7.10.
- **Extra imports.** `COMDLG32.dll` appears in the EP6 and Phoenix builds and not in the EP4.5 client; the Spanish build imports one more DLL that the others do not (**confirmed (file)**).
- **Skill levels.** The EP6 lookup accepts 1 to 15; EP5.4 per its decompilation used 9 (**ref** server page, section 6).
- **Protocol.** The buff, target-select, item-mall product list and chance-preview packets were read in the dump and agree with the EP5.4 reading, with the changes listed on that page (**ref**).

## Builds known

| Label | Link date (UTC) | Size | SHA-256 | Where from | Status |
|---|---|---|---|---|---|
| Phoenix `ep6.3` | 2013-05-21 08:02 | 3,941,208 B | `feabe681b1b2cbd6…` | OpenShaiya archive, folder `shaiya-phoenix/clients/ep6.3` | **confirmed (file)** |
| Spanish patch 171 | 2016-01-21 07:22 | 4,000,256 B | `38b9000c82f15906…` | a public collection of client builds | **confirmed (file)** |
| Portuguese patch 182 | 2016-01-27 09:08 | 4,000,768 B | `359e34f99c575f20…` | a public collection of client builds | **confirmed (file)** |
| `game.ep6.v1` (dump) | 2016-02-12 02:13 | 4,052,448 B | `7b138c69c3c3dd6a…` | a public collection of client builds | **confirmed (file)**; code read by a server page (**ref**) |

A link date is a stamp a tool can set or keep; it is not a release date. Its date, 2016-02-12, matches the catalogue dates of US patch 343, German 290, Spanish 173 and Portuguese 183, and the Spanish 171 and Portuguese 182 link dates match their own catalogue dates: a 2016 Western build line (*inferred*). The `game.ep8.v1` file (linked 2016-07-12, the day of US patch 354) fits the same line, so its EP8 label is *unverified* (see [Episode 8](ep8.md)). No US EP6 build is on hand.

## Compared with this client

- The US EP4.5 client links 2010-04-26, is 3,203,072 B and uses linker 7.10; the EP6 builds here are 0.7 to 0.85 MB larger and use 9.00 (**confirmed (file)**).
- Both generations import `d3d9.dll` and the same core set (`ijl15.dll`, `WINMM.dll`, `WS2_32.dll`, `DINPUT8.dll`, `DSOUND.dll`, `DDRAW.dll`) (**confirmed (file)**).
- Beyond headers, no comparison of code was done on this page (**open**).

## Server side

- [The EP5.4 server pack](../../original/ep5.4.md), section 6 ([the client side, checked in the EP6 client](../../original/ep5.4.md#6-the-client-side-checked-in-the-ep6-client)): the packets of the EP5.4 pack read in the EP6 dump (**ref**).

## Sources

- F2P News, 2012-09-25: https://www.f2p.com/shaiya-launches-episode-six-trinity-artifact/
- MMOReviews, 2012-09-25: https://www.mmoreviews.com/?p=28623
- MMOHuts, 2012-09-24: https://mmohuts.com/news/shaiya-announces-trinity-artifact-expansion/
- The OpenShaiya archive catalogue (patch numbers, dates, sizes).

## Open

- What patch 200's `game.exe` is (the size dropped by over 1.2 MB).
- A US build of Episode 6 itself: none on hand.
- Races and classes added, if any.
- Read the Phoenix, Spanish and Portuguese builds' code against the dump.
