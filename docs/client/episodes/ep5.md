# Episode 5

Episode 5 is the expansion that raised the level cap to 70 and added zone-control PvP. This page covers it with one caution up front: the one public article about it names the expansion ("The Relic of Mystra"), not an episode number, so tying that name to "Episode 5" is an inference. The page is for people who want to know what is documented about this episode, which client builds exist and how they relate to the US Episode 4.5 client this site studies.

## In short

- **Name and date.** A news site reported the expansion "The Relic of Mystra" on 2010-08-29 (**ref**, see Sources). It does not say "Episode 5", so that link is *inferred*.
- **Level cap 70.** The same article gives the cap as 70 (**ref**). The EP5.4 server pack on this site agrees: it ships content for levels 61 to 70 (**ref**, [server page](../../original/ep5.4.md)).
- **Headline content.** Over 100 new gear pieces, over 150 expanded skills and an invasion PvP system for taking whole zones (**ref**).
- **One client build on hand.** A French-labelled build in the OpenShaiya archive, folder `shaiya-fr/clients/ep5.4`. It is not packed. That it is EP5.4 rests on the folder name only (*unverified*).
- **Not studied in code.** Nothing on this page comes from reading that client's code. The facts about it are file headers (**confirmed (file)**).

### Using this page

- **If you write a server:** the EP5.4 server pack is the source of EP5-era behaviour, not this page. Start at [the EP5.4 server pack page](../../original/ep5.4.md) and [what a server must do for this client](../afterglow/server-checklist.md).
- **If you patch or mod the client:** treat the EP5.4 build as a different executable from the EP4.5 one. It is larger by 94,208 bytes and has its own link date, so offsets from this site's client pages do not carry over.
- **To check a claim yourself:** the header facts below can be re-read with any PE viewer; see [Tools and method](../tools-and-method.md).

## Releases

| Region | Date | Patch | Source | Status |
|---|---|---|---|---|
| English-language service (region not stated on the page) | 2010-08-29 (article date) | — | news article "The Relic of Mystra" | **ref**; that this is Episode 5: *inferred* |
| Korea | a three-stage "Mistra Relic" update starting 11 November, year not shown | — | a search-result snippet | *unverified* |
| France | link date 2011-12-08 for the archive build | 114 (catalogue) | OpenShaiya archive folder `shaiya-fr/clients/ep5.4` | **confirmed (file)** for the link date; release date not known |

A link date is when the build was linked, not when the episode launched. The archive's own dates for this folder are an import date and are not used here.

## What changed for players

- Level cap 70 (**ref** [MMOHuts](https://mmohuts.com/news/shaiya-expansion-pack-the-relic-of-mystra-released)).
- More than 100 new gear pieces for all classes in both factions (**ref**, same article).
- More than 150 expanded skills (**ref**, same article).
- An invasion PvP system for taking control of whole zones (**ref**, same article).
- Races and classes added: not stated (—).

## What changed in the client

- **Imports and layout (EP5.4-labelled build).** Same imported DLLs as the EP4.5 client, including `d3d9.dll`; sections `.text .rdata .data .rsrc`; linker 7.10, as in the EP4.5 client; no protector sections (**confirmed (file)**).
- **Skill levels.** Nine levels per skill for EP5.4, from a reconstruction (decompilation) of an EP5.4 client, not from the build above (**ref** [server page, section 6](../../original/ep5.4.md#6-the-client-side-checked-in-the-ep6-client)).
- **HP, MP and SP on the wire.** EP5 widened these fields to 32-bit numbers; this client reads 16-bit ones (**ref** [Client facts](../CLIENT-FACTS.md#watch-hp-mp-and-sp-above-65535-and-current-values-above-32767)). Not read in the EP5.4-labelled build itself.
- **Protocol.** The server page reads the EP5.4 packets from the same decompilation and checks them in the later EP6 client; it does not claim the original EP5.4 client was read (**ref**).

## Builds known

| Label | Link date (UTC) | Size | SHA-256 | Where from | Status |
|---|---|---|---|---|---|
| `ep5.4` (French folder) | 2011-12-08 03:38 | 3,297,280 B | `f8aba8ca504735e6…` | the OpenShaiya archive, folder `shaiya-fr/clients/ep5.4` | **confirmed (file)** for size, hash, headers; the EP5.4 label is *unverified* |

## Compared with this client

- Same imported DLL set and same linker generation (7.10) as the US EP4.5 `game.exe`; the EP5.4-labelled file is 94,208 bytes larger (**confirmed (file)** for both).
- Whether the code differs in the ways the server pack suggests (nine skill levels, a rewritten movement check) was not read in this build (**open**).

## Server side

- [The EP5.4 server pack, compared with EP4.5](../../original/ep5.4.md): its main programs (ps_game, ps_gamewar, ps_session, ps_dbAgent) were linked on 2011-09-14 and it carries level 61 to 70 content (**ref**). Its section 6 says no original EP5.4 client was available; the archive build above was found later and is the first original candidate.
- An Episode 6 project's server files are reported to be "the original from EP 5.4" (**ref** [Shaiya Essentials, Episode 6 project](https://sites.google.com/view/shaiyaessentials/episode-6-project)).

## Sources

- MMOHuts, "Shaiya Expansion Pack The Relic of Mystra Released", 2010-08-29: https://mmohuts.com/news/shaiya-expansion-pack-the-relic-of-mystra-released
- Shaiya Essentials, Episode 6 project: https://sites.google.com/view/shaiyaessentials/episode-6-project
- The OpenShaiya archive catalogue (folder names, sizes); the build itself was read from a copy of that archive.

## Open

- Does any source tie "Relic of Mystra" to the name Episode 5? (*unverified*)
- Region and date of the first Episode 5 release in the US and EU.
- Read the EP5.4-labelled build's code to confirm nine skill levels and the packet changes (**open**).
- Races and classes added, if any.
