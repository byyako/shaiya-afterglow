# Episode 8

Episode 8 is the version the publisher gamigo chose in December 2020 when it put every regional version of the game on one code base. The EP8 client builds on hand are protected with a commercial-style protector and were **packed, not analysed**: this page gives file-header facts only. It is for readers who want to know what is documented about EP8 and why this site says little about its client.

## In short

- **Unification notice.** On 2020-12-10 gamigo announced one code base with the EU version as the standard and older versions upgraded to Episode 8 (**ref**, two sources).
- **Content named in the notice.** New armour sets, a dyeing system, new skills, a refined interface, new quests, monsters and maps (**ref**).
- **Clients in six languages**: German, French, Italian, English, Spanish, Portuguese (**ref**).
- **Level cap not stated** for the live game. A 2019 Southeast Asia closed beta is reported with Episode 8 graphics and gameplay and a cap of 60: *unverified* (a search summary of a page that could not be opened).
- **Client builds on hand are packed.** Two files are labelled EP8. Their headers show protector-style sections; nothing was unpacked or read as code.

### Using this page

- **If you write a server:** no EP8 server pack was studied on this site. Public EP8 format projects are listed in the Sources; this site did not check them.
- **If you patch or mod the client:** nothing here helps with an EP8 client. Its executable is protected and was not examined.
- **To check a claim yourself:** the header facts can be re-read with any PE viewer ([Tools and method](../tools-and-method.md)).

## Releases

| Region | Date | Patch | Source | Status |
|---|---|---|---|---|
| gamigo, all versions | 2020-12-10 (announcement) | gamigo patches from 2020-12 are listed on [Later builds](later.md) | gamigo press release, MMOBomb | **ref** |
| Korea | a build labelled `ep8` links 2014-06-13 | — | OpenShaiya archive, folder `shaiya-kr/clients/ep8` | **confirmed (file)** for the link date; launch date not known |
| Southeast Asia (PlayPark) | closed beta 2019; server closure 2020-11-20 | — | a search summary | *unverified* |
| First release of Episode 8 anywhere | no source found | — | — | — |

Community labels can name hybrids: a project that runs an "EP8 client on EP5 services" exists, so an "EP8" label on a client file says nothing about the server it ran against (**ref** public tool repositories).

## What changed for players

- New armour sets (**ref** gamigo notice).
- A dyeing system (**ref**; the same system is described for the Korean and US Iron Invasion update on the [Episode 7](ep7.md) page, so the first episode with dyes is *unverified*).
- New skills, a refined interface, new quests, monsters and maps (**ref**).
- Level cap: not stated (—).

## What changed in the client

Not analysed. For the two EP8-labelled files:

- Both are packed or protected: very high-entropy sections, section names that look like a protector's, and a very small import table or a rebuilt one (**confirmed (file)**).
- The file named `game.ep8.v1` is a 35 MB image whose import table was rebuilt; it lists `d3d9.dll` among its imports, but that list belongs to the dumped image, not to code anyone read (**confirmed (file)**).
- The archive's `ep8` build imports only `kernel32.dll` and `comctl32.dll`, which is what a protector stub imports (**confirmed (file)**). Its real imports are not readable.
- Graphics API: not known for the code; the dumped image's rebuilt import table lists `d3d9.dll`. Packet formats, skill levels and HP/MP/SP width: not known (—).

## Builds known

| Label | Link date (UTC) | Size | SHA-256 | Where from | Status |
|---|---|---|---|---|---|
| `game.ep8.v1` | 2016-07-12 05:32 | 35,201,024 B | `67cc1429ece6bef9…` | a public collection of client builds | **confirmed (file)** headers only; packed, not analysed; label *unverified*: linked the same day as US patch 354 (*inferred* same build line), four years before the 2020 EP8 notice |
| Korean `ep8` | 2014-06-13 04:48 | 2,753,192 B | `fa02b1a459981628…` | OpenShaiya archive, folder `shaiya-kr/clients/ep8` | **confirmed (file)** headers only; packed, not analysed |

The link date of a packed file is the packer's output date and may not match the game build inside (*inferred*).

## Compared with this client

The US EP4.5 client here is not protected and imports `d3d9.dll` directly. For the EP8 files, no comparison beyond "packed against not packed" is possible (**confirmed (file)** for both headers).

## Server side

None studied on this site for EP8.

## Sources

- gamigo, "Shaiya Unifies its Source Code", 2020-12-10: https://corporate.gamigo.com/en/presse/shaiya-unifies-its-source-code/
- MMOBomb, 2020-12-10: https://www.mmobomb.com/news/gamigo-unifies-shaiya-source-code-makes-servers-available-everyone
- Public tool repositories for EP8 data formats (BinarySData, `DB*Data` files), for example the Parsec and Eden projects on GitHub.
- The OpenShaiya archive catalogue (folder names, sizes).

## Open

- Release date of the first Episode 8 anywhere.
- The live level cap, and whether 60 for the Southeast Asia beta is right.
- The contents of the two packed clients: not planned on this site.
