# After Episode 8

This page covers everything after Episode 8 in any region: the gamigo builds of 2020 to 2022 and the Chinese folders labelled `ep4` to `ep13` in a public archive. Very little is documented publicly, and the only dates here are catalogue dates or file link dates. It is for readers who found these labels and want to know what they do and do not mean.

## In short

- **gamigo builds ga 5 to 37.** The OpenShaiya catalogue lists gamigo client builds from 2020-12-03 to 2022-06-15 (catalogue dates, **ref**). One of them was read at header level.
- **gamigo patch names.** "The Cryptic Menace" and "The Void" appear in gamigo press items; dates were not read (**ref**).
- **Chinese folders `ep4` to `ep13`.** Ten folders with patch numbers 151 to 533. Their numbers do not follow the Western episode numbers (*unverified* mapping).
- **Catalogue dates are not release dates.** The archive's dates for the Chinese, Korean, French, Russian and Phoenix folders are one import day in 2022; they say nothing about when a build was released.
- **One Chinese build is packed** and was not analysed.

### Using this page

- **If you write a server:** nothing here is a server source. Use [Episode 8](ep8.md) for the unification notice.
- **If you patch or mod the client:** the gamigo build below is unpacked and uses the same imports as the EP4.5 client, but nothing about its code was read.
- **To check a claim yourself:** [Tools and method](../tools-and-method.md) for reading a PE header.

## Releases

| Region | Date | Patch | Source | Status |
|---|---|---|---|---|
| gamigo (EU, merged versions) | first build 2020-12-03 (catalogue date) | ga 5 | OpenShaiya catalogue | **ref** |
| gamigo | 2021-12-08 (catalogue date) | ga 32 | OpenShaiya catalogue | **ref** |
| gamigo | latest 2022-06-15 (catalogue date) | ga 37 | OpenShaiya catalogue | **ref** |
| US | last `game.exe` change 2020-11-17 (catalogue date) | 396 | OpenShaiya catalogue | **ref** |
| China | folders `ep4` to `ep13`, patches 151 to 533 | — | OpenShaiya catalogue | import dates only; no release dates |

Rows with the same early gamigo paths as the US service share history with the US patches, so the first gamigo-only build is the one dated 2020-12-03.

## What changed for players

Only the gamigo patch names are known, not their content: "The Cryptic Menace" and "The Void" (**ref**). The unification and Episode 8 content are on the [Episode 8](ep8.md) page. A community forum blurb lists the labels "EP3, EP4, EP4.5, EP5, an EP5.1 client, Korean EP6, EP7 Chinese files billed as EP5 compatible, EP9 CN and a 7.0 server set" (*unverified*: one forum post; no dates).

## What changed in the client

- **gamigo patch 32** (unpacked): imports include `d3d9.dll`, `DINPUT8.dll`, `DSOUND.dll`, `DDRAW.dll` and the other core DLLs of the EP4.5 client; linker 9.00; sections `.text .rdata .data .rsrc .reloc`; no protector sections (**confirmed (file)**). Its version resource names Aeria Games GmbH, a German-locale resource (**confirmed (file)**).
- **gamigo patch 37** is 53,505,888 bytes in the catalogue; not on hand, not analysed (**ref** size only).
- **Chinese `ep13`:** packed, with two imports only, a protector's stub pattern; not analysed (**confirmed (file)** headers).
- **Chinese `ep4` to `ep10`:** 1.3 to 1.5 MB, small; full client, packed or partial: *unverified*/**open**; not downloaded. `ep11` and `ep12` are 2.7 MB; `ep11` to `ep13` look packed by size (*inferred*, only `ep13` read).

## Builds known

| Label | Link date (UTC) | Size | SHA-256 | Where from | Status |
|---|---|---|---|---|---|
| gamigo patch 32 | 2021-12-06 10:45 | 5,352,488 B | `509c4a8fbe4d5292…` | a public collection of client builds | **confirmed (file)**; size matches the catalogue |
| Chinese `ep13` | 2014-10-08 08:43 | 2,787,160 B | `36785109222329d0…` | OpenShaiya archive, folder `shaiya-cn/archived/ep13` | **confirmed (file)** headers only; packed, not analysed |
| gamigo patch 37 | — | 53,505,888 B | — | catalogue size only | not on hand |

The Chinese link date is the packer's output date and may not match the game build (*inferred*).

## Compared with this client

- gamigo patch 32 is 2.15 MB larger than the US EP4.5 `game.exe` and links 11 years later; both import `d3d9.dll`; the gamigo build has a `.reloc` section and linker 9.00 (**confirmed (file)** for both).
- No code comparison was made (**open**).

## Server side

None studied on this site for these builds.

## Sources

- gamigo, "The Cryptic Menace": https://press.gamigo.com/shaiya-the-cryptic-menace-patch-is-live
- gamigo, "The Void": https://press.gamigo.com/media-alert-shaiya-dares-players-to-step-into-its-latest-patch-the-void
- gamigo, "Shaiya Unifies its Source Code", 2020-12-10: https://corporate.gamigo.com/en/presse/shaiya-unifies-its-source-code/
- The OpenShaiya archive catalogue (patch numbers, dates, sizes).

## Open

- Dates and content of the two named gamigo patches.
- What a Chinese `ep` number means in Western terms.
- The contents of the packed Chinese and gamigo patch 37 builds: not planned on this site.
