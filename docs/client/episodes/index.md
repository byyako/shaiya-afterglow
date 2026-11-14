# Shaiya episodes

This section collects what is known about each Shaiya episode, from Episode 1 to the builds after Episode 8, and which client files from them exist in public archives. The client this site studies is the US build of April 2010, a state the community calls EP4.5. The pages here say how other episodes relate to it, and are honest about how little is documented for most of them.

## In short

- This site studies one client in depth: the US `game.exe` of patch 106 (linked 2010-04-26). See [Episode 4.5](ep4-5.md).
- For the other episodes the evidence is thin: a few news articles, the OpenShaiya catalogue of patches, and the headers of a handful of client files read without running them.
- Dates from a regional build are link dates, not release dates. A regional build lags the original release.
- No page here says what Episode 2 or Episode 7 contained in code. For both, no client file is on hand.
- Every claim carries a status word. **ref** means a named public source or a page of this site; *unverified* means one source that cannot be checked here; **confirmed (file)** means read from a file's headers.

### Using this page

- **If you write a server:** the matrix shows which episodes have a server pack studied on this site: [EP3 vs EP4.5](../../original/ep3.md) and [EP5.4](../../original/ep5.4.md). Nothing is claimed for the others.
- **If you patch or mod the client:** every episode has its own executable. Do not carry addresses from this site's client pages to another episode's file.
- **To check a claim yourself:** hash a file and read its PE header and import table ([Tools and method](../tools-and-method.md)); the build tables on each episode page give the size and the first 16 hex of the SHA-256.

## Timeline

| Episode | First release found | Regions in the evidence | Builds evidenced | Page |
|---|---|---|---|---|
| EP1 | 2007-12-18 (US catalogue date, patch 0) | US | one public-collection `game.exe` (linked 2007-12-14) | [Episode 1](ep1.md) |
| EP2 | none found | none | none | [Episode 2](ep2.md) |
| EP3 | none found; EU closed beta in November 2008, episode unnamed | Europe (beta), Russia | Russian `ep3` (linked 2009-09-03) | [Episode 3](ep3.md) |
| EP4 "Eternity" | 2009-05-11 (news report) | not stated | none distinct; the community "EP4" file is the EP4.5 file edited | [Episode 4](ep4.md) |
| EP4.5 | this client's build: 2010-04-26 link date (**confirmed (file)**); first EP4.5 release — | US, Russia | US patch 106 (this client), Russian `ep4.5` | [Episode 4.5](ep4-5.md) |
| EP5 "Relic of Mystra" | 2010-08-29 (news report; the link to EP5 is inferred) | not stated, France | French `ep5.4` folder (linked 2011-12-08) | [Episode 5](ep5.md) |
| EP6 "Trinity Artifact" | 2012-09-24 (announced), 2012-09-25 (launched) | US, Phoenix, Spain, Portugal | four builds, 2013 to 2016 | [Episode 6](ep6.md) |
| EP7 "Iron Invasion" | 2013-08-26 (US report), 2013-08-01 (Korea) | US, Korea | none | [Episode 7](ep7.md) |
| EP8 | 2020-12-10 (unification notice) | gamigo, Korea | two packed files | [Episode 8](ep8.md) |
| After EP8 | 2020-12-03 (first gamigo build, catalogue date) | gamigo, China | gamigo patch 32, Chinese `ep13` | [After Episode 8](later.md) |

The US patch list behind these dates is on [US patch history](us-patches.md). The catalogue's dates for the Russian, French, Korean, Phoenix and Chinese folders are one import day and are not used here.

## Comparison matrix

Each cell repeats a line of the episode page named in its column heading. "—" means not known, or not stated by any source read. The status word is in the cell.

| | [EP1](ep1.md) | [EP2](ep2.md) | [EP3](ep3.md) | [EP4](ep4.md) | [EP4.5](ep4-5.md) | [EP5](ep5.md) | [EP6](ep6.md) | [EP7](ep7.md) | [EP8](ep8.md) | [Later](later.md) |
|---|---|---|---|---|---|---|---|---|---|---|
| First release | 2007-12-18 US catalogue date (**ref**) | — | none found; EU closed beta Nov 2008, episode unnamed (**ref**, link *unverified*) | 2009-05-11 report (**ref**) | this client's build: 2010-04-26 link date (**confirmed (file)**); first EP4.5 release — | 2010-08-29 report (**ref**; EP5 *inferred*) | 2012-09-24 announced, 2012-09-25 launched (**ref**) | 2013-08-26 US (**ref**) | 2020-12-10 announcement (**ref**); first release — | 2020-12-03 first gamigo build, catalogue date (**ref**) |
| Level cap | — | — | 60 in the 2008 EU beta (**ref**; episode *unverified*) | — (article gives none) | — | 70 (**ref**) | 80 (**ref**) | — (not stated; 80 *inferred* from weapons) | — (60 for a SEA beta, *unverified*) | — |
| Races and classes | — | — | — | — | — | — | — | — | — | — |
| Headline systems added | — | — | server pack: no Lua monsters, party instances or auction house (**ref**) | battlefields, Oblivion Insula, item evolution, raids to 30 (**ref**) | server side: auction house, union parties, Lua monsters, party instances (**ref**) | 100+ gear pieces, 150+ skills, invasion PvP (**ref**) | two islands, civil-war theme, PvP skills and sets (**ref**) | Stable Erde battlegrounds, world bosses, Infinite Sanctuary, Chaotic Square, dyes (**ref**) | armour sets, dyeing, skills, interface (**ref**) | patch names only: "The Cryptic Menace", "The Void" (**ref**) |
| Client builds evidenced | one public-collection file (**confirmed (file)**) | none | Russian `ep3` (**confirmed (file)**) | none distinct (the "EP4" file is the EP4.5 file edited) (*inferred*) | US patch 106; Russian `ep4.5` (**confirmed (file)**) | French `ep5.4` folder (**confirmed (file)**; label *unverified*) | Phoenix `ep6.3`, Spanish 171, Portuguese 182, `game.ep6.v1` (**confirmed (file)**) | none | `game.ep8.v1` (label *unverified*; linked the same day as US patch 354, *inferred* same build line), Korean `ep8` (headers only, **confirmed (file)**) | gamigo patch 32, Chinese `ep13` (**confirmed (file)**) |
| Link date of the build | 2007-12-14 (**confirmed (file)**) | — | 2009-09-03 (**confirmed (file)**) | — | 2010-04-26; Russian 2011-03-22 (**confirmed (file)**) | 2011-12-08 (**confirmed (file)**) | 2013-05-21; 2016-01-21; 2016-01-27; 2016-02-12 (**confirmed (file)**) | — | 2016-07-12; Korean 2014-06-13 (**confirmed (file)**) | 2021-12-06; Chinese 2014-10-08 (**confirmed (file)**) |
| Graphics API import | `d3d9.dll` (**confirmed (file)**) | — | `d3d9.dll` (**confirmed (file)**) | — | `d3d9.dll` (**confirmed (file)**) | `d3d9.dll` (**confirmed (file)**) | `d3d9.dll`, all four (**confirmed (file)**) | — | not readable in the packed Korean file; `d3d9.dll` in the dumped image (**confirmed (file)**) | `d3d9.dll` in patch 32 (**confirmed (file)**) |
| Packer or protector | none (**confirmed (file)**) | — | not packed (**confirmed (file)**) | — | not packed (**confirmed (file)**) | none (**confirmed (file)**) | none (**confirmed (file)**); `game.ep6.v1` likely a dump (*inferred*) | — | packed, not analysed (**confirmed (file)**) | patch 32 not packed; Chinese `ep13` packed, not analysed (**confirmed (file)**) |
| SData skill levels | — | — | — | — | 3 (**ref**) | 9 (**ref**, from a decompilation) | 15 (**ref**) | — | — | — |
| HP, MP and SP wire width | — | — | — | — | 16-bit (**ref**) | 32-bit (**ref**); not read in the French build | — | — | — | — |
| Server pack studied here | — | — | [EP3 pack](../../original/ep3.md) (**ref**) | — (the EP3 page spans it, *inferred*) | [what EP4.5 added](../../original/ep3.md) (**ref**) | [EP5.4 pack](../../original/ep5.4.md) (**ref**) | [section 6](../../original/ep5.4.md#6-the-client-side-checked-in-the-ep6-client) (**ref**; EP6 client read) | — | — | — |
| Pages here | [Episode 1](ep1.md) | [Episode 2](ep2.md) | [Episode 3](ep3.md) | [Episode 4](ep4.md) | [Episode 4.5](ep4-5.md) | [Episode 5](ep5.md) | [Episode 6](ep6.md) | [Episode 7](ep7.md) | [Episode 8](ep8.md) | [Later](later.md), [US patches](us-patches.md) |

Two cautions. Community labels can name hybrids, such as an EP8 client on EP5 services, so a label on a file says little about the server it ran with (see [Episode 8](ep8.md)). And a link date is the compiler's or packer's output, not a release date.

## How these pages were made

- The client files were read by their bytes only: size, SHA-256, link time, sections, entropy and imported library names. Nothing was run, unpacked or changed.
- Release and patch dates come from the OpenShaiya catalogue of patches (read-only) and from public news and publisher pages, each named on the page that uses it.
- Where a source could not be opened, the page says so, and the claim is marked *unverified* or left out.
- Packed clients are described by header facts alone and are not analysed.

## The pages

- [Episode 1](ep1.md), [Episode 2](ep2.md), [Episode 3](ep3.md), [Episode 4](ep4.md), [Episode 4.5](ep4-5.md)
- [Episode 5](ep5.md), [Episode 6](ep6.md), [Episode 7](ep7.md), [Episode 8](ep8.md), [After Episode 8](later.md)
- [US patch history of `game.exe`](us-patches.md)

## Sources

- The OpenShaiya archive catalogue, a public archive of Shaiya patches and clients.
- The news and publisher sources named on each episode page.

## Open

- Dated sources for Episodes 2 and 3.
- Reading the code of the EP5.4-labelled French build, which could settle the nine skill levels.
- Any client build that is clearly EP4, EP7 or from the US Episode 6 launch.
