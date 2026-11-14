# Generating the client data files

This page collects what is known about the client's data files (`*.SData`) of the client `game.exe` (US build 2010-04-26, patch 108):
how they are encrypted, how their records are laid out, what is needed to rebuild them byte for byte, and what a server's database lacks
to generate them. The original EP4.5 server `ps_game.exe` reads items, skills and monsters from its database (`PS_GameDefs`), while the
client shows its own copies; generating the client files from the database would make the client show what the server does.

## In short

- **The client keeps its own copies of the game's data** (the [SData](glossary.md#sdata) files): items, skills, monsters, NPCs and quests, the item mall and more. The server
  never sends them, so the client shows whatever its files say.
- **The files can be read and rebuilt exactly.** The encryption is known, and every file is fully explained by its records: rebuilding
  one from its parsed records gives the same bytes.
- **Most of what a generated item file needs is already in the server's database**; the rest (descriptions, models, icons, the client's
  English names) would have to come from the client's own file.
- **A generated file would reach players in a client patch.** Each difference it fixes (prices, stack sizes, tooltips, level and trade
  marks) changes what players see, so each is a choice to make before shipping.

### Using this page

- **If you write a server:** what to send and check is collected in [What a server must do for this client](afterglow/server-checklist.md#generating-the-client-data-files).
- **If you patch or mod the client:** "Getting generated files to the client" says how a rebuilt file reaches players; rebuild only after a byte-for-byte round trip of the stock file passes.
- **To check a claim yourself:** the encryption and record layout are under "What is proven"; the archive wrapper is in [data-archive.md](data-archive.md).

## What is proven for `Item.SData`

- **Encryption**: SEED, with the EP4-7 header: 40-byte signature, `u32` checksum, `u32` size, 16 zero bytes, then 16-byte blocks padded
  with zeros. Encrypting the decrypted client `Item.SData` gives the **same bytes** as the original.
- **Records**: the decrypted file is fully explained by its records: rebuilt from (per group: the record count; per record: name text,
  description text, 81 fixed bytes) it comes out **byte-identical**. Strings are `u32` length + text + one NUL, nothing hidden after it.
- **Shape**: 100 groups (the item types); each group is a dense array by type id: record `i` is type id `i + 1`, the count is the highest
  type id, and missing ids are empty records (type 0; 134 of them). Text is plain ASCII in this file (no byte above `0x7F`).
- So a generator only needs, per item: the 81 fixed bytes (all but four are columns of the database's `Items` table) plus the client-only
  fields below.

| Original client file | SHA-256 |
|---|---|
| `Item.SData`, US patch 108 (`data/item/item.sdata`, encrypted as shipped) | `bb1ef0fccfc7d80be854b5376d18e41234920921ed62d791dc4a9ac78321ef48` |
| the same, decrypted | `efb6523292ee8e524e379f261abe8138b3816247f94d581e54826159ea03b0ba` |

## What the database lacks for `Item.SData`

| Field | In the `Items` table? | Note |
|---|---|---|
| name | different | the client's English names; the table's `ItemName` is the server's (7,729 differ, some Chinese or Korean) |
| description | no | tooltip text with markup (`\n`, `{c5}` colours) |
| model, icon, `type2`, `type3` | no | bytes +2, +3, +14, +15: looks and the client's own grouping |
| range | wider | one byte in the client, two in the database (only the low byte goes in the file) |
| 16-bit fields | signed | `smallint` holds them as signed (44061 reads −21475), so the file needs them as `u16` again |
| empty records | no | type 0 records where no item exists: generated from the gaps |

With the client's own values for those fields kept alongside the database, the original file can be **restored** byte for byte (client
values for everything) or a **matched** file generated (the fixed fields from the database, the rest from the client's records). The
differences on [Item.SData against the server](item-sdata-vs-defs.md) are what a matched file changes for players (prices, stack sizes,
tooltips, level and trade marks the client checks before sending); each one is a choice to confirm before shipping.

## The other client data files (US client up to patch 108)

The client's SData files, from the US patch history, and their server side. Each is fully explained by its records: a reader for the
client files (Skill, NpcSkill, Monster, Cash, GuildHouse, KillStatus) and the reader for the server's `NpcQuest.SData` read every byte, and
the client files are rebuilt byte for byte from the parsed records.

| Client file | Last patch ≤ 108 | Server side | Status |
|---|---|---|---|
| `data/item/item.sdata` | 107 | the database's `Items` | compared ([Item.SData against the server](item-sdata-vs-defs.md)); round trip proven |
| `data/npc/npcquest.sdata` | 99 | the **server's own** `NpcQuest.SData` | same layout as the server's, English text; compared ([NpcQuest.SData against the server](npcquest-sdata-vs-world.md)): **every shop identical**, one gate fare differs |
| `data/character/skill.sdata` | 65 | the database's `Skills`, levels 1 to 3 | 326 skills × 3 levels, 116 fixed bytes; compared ([Skills](skill-monster-sdata-vs-defs.md#skills-skillsdata-against-skills-levels-1-to-3)) |
| `data/monster/npcskill.sdata` | 93 | `Skills` level 100 | the skill layout, 348 × 3 (first record used); compared ([Monster skills](skill-monster-sdata-vs-defs.md#monster-skills-npcskillsdata-against-skills-level-100)) |
| `data/monster/monster.sdata` | 93 | the database's `Mobs` | 2,474 records, name + 29 bytes; compared ([Monsters](skill-monster-sdata-vs-defs.md#monsters-monstersdata-against-mobs)) |
| `data/cash.sdata` | 62 | the database's `ProductList` (the item mall) | 85 products; compared ([The item mall](skill-monster-sdata-vs-defs.md#the-item-mall-cashsdata-against-productlist)) |
| `data/character/killstatus.sdata` | 0 | the server's `Data/KillStatus.SData` | plain; **byte-identical** to the server's |
| `data/npc/guildhouse.sdata` | 84 | `GuildHouse.ini` (server) | plain, 648 bytes; prices and NPCs agree ([Guild houses](skill-monster-sdata-vs-defs.md#guild-houses-kill-ranks-priest-talk)) |
| `data/npc/priesttalk.sdata` | 0 | none (client text only) | plain, Korean text; keep as shipped |

The files as received (SHA-256; "decrypted" = the SEED-decrypted file, checksum checked):

| File | As shipped | Decrypted |
|---|---|---|
| `Item.SData` | `bb1ef0fccfc7d80be854b5376d18e41234920921ed62d791dc4a9ac78321ef48` | `efb6523292ee8e524e379f261abe8138b3816247f94d581e54826159ea03b0ba` |
| `NpcQuest.SData` | `cef2ef8ade873843f4b22d9d97798d74ff0d068e7c6fbd75a4c2d3ec887c399e` | `6dd06f2e12677050188eaeeb7ff07bbe024524a516eb314a4815f601b8dd2579` |
| `Skill.SData` | `0ff8450aa07ddf1a6eace85bf61435f44e3699ccdaa722fe10703bf8d09a8d92` | `a7bca7064b86438107fca7a72befad67e9d585b156cfe5befaacad12a0ebc0fa` |
| `NpcSkill.SData` | `89e2f92ef61e544b2edc351de4b27e8128e83b601c38ecee3f932e3097f3c7be` | `c298e3f8cf4e55d6f820ed08a7455731fc5597c371c5e52c03ca6253e48bd6b5` |
| `Monster.SData` | `10cd2b353276608b6c64b06d31bd5b21854df9d9bb81c488b978ff106b8a90e4` | `701fed38a1286c93e362574d0b3da654253ef3dd993266df2bce623f0f36e75f` |
| `Cash.SData` | `63905ac5d86b406665c39f50aa8a006bc92d5ab05844c7537f6cbb7d97d8af30` | `c7f4c86e3f98bd08cd0910c7c76822378c61e06cf6ed6098bb1226b18da63181` |
| `KillStatus.SData` | `ae82bd49a7e8293e8f40e66430ed0e48287b532384b631c8ba097355e19d04c7` | (plain) |
| `GuildHouse.SData` | `739fb38a5e2bf3020ecd0220b2bddf1b680ad9890893e48b29c975fedc68b73d` | (plain) |
| `PriestTalk.SData` | `fe42e85b507f11825e533d7cee28192afaf9ed4ed7a83b5a3c3b08df8d04570f` | (plain) |

The server's own `PS_Game/Data/NpcQuest.SData`: `fd4dd9ba2ca376a18bcc138cf848f148d9df8c467ce219eaf52237f52669bda7`.

Maps (client world files against the server's svmaps) are not SData and not covered here.

## Getting generated files to the client

A generated file would ship in a client patch: the client's archive is `data.sah`/`data.saf`, and patches arrive as `psNNNN.patch`
files (patch 108 is `ps0108.patch`). A generated `Item.SData` (SEED-encrypted as above) would go into such a patch at
`data/item/item.sdata`.

## In other episodes

- **EP8 data formats:** public tool repositories exist for the EP8 data files (BinarySData, `DB*Data` files); this site did not check them
  (*unverified*, [EP8](episodes/ep8.md)). Nothing on this page is claimed for those files.

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#generating-the-client-data-files).

## Open questions

- Which values a shipped file should take, difference by difference (see the comparison pages).
