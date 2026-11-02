# The archive: data.sah and data.saf

`data.sah` is the index, `data.saf` is a flat blob of file contents. This page gives the layout of both, what the 108 archive contains,
how `game.exe` looks a file up, the SData wrapper, and what a repack changed. Reader: the client repository's `tools/sah.py` (list, stats, check, extract).
The layout matches the public Parsec library (`Sah.cs`, `SDirectory.cs`, `SFile.cs`); the free list below is not read by Parsec.
Status words: **confirmed** (read in `game.exe` or checked by the tool), *inferred*.

## In short

- **Two files:** `data.sah` lists names, offsets and lengths in a directory tree; `data.saf` is the files laid end to end.
- **The 108 archive holds 20,412 files (about 1.97 GiB referenced)** and about 10.6 % of `data.saf` is space released by old patches.
- **The client finds a file by a hash of its lower-cased name** in each directory; it never reads the free list. If two entries share a
  name, the later one wins.
- **Only the SData tables are encrypted** (SEED, ECB, one fixed key); everything else is stored plain.
- **A repack can drop the dead space** (−270 MB here) without changing what the client sees; the method and result are below.

## Index layout (little-endian)

```
header     char[3] "SAH"; i32 version (0 here); u32 file_count (20412); byte[40] zero
directory  str name; u32 n_files; file[n_files]; u32 n_dirs; directory[n_dirs]     (root name is empty)
file       str name; i64 saf_offset; u32 length; i32 version   (version is a Unix timestamp: when the file was packed)
str        i32 length including the NUL; bytes (cp949 / ASCII)
free list  u32 count (2989); free_entry[count]                                   (after the root directory)
free_entry str name; i64 saf_offset; u32 length; i32 version                     (same shape as a file entry)
trailer    4 bytes 60 49 6b 0b (unknown)
```

The free list ("rest space" table) records blocks in `data.saf` that patches released. 2072 entries are named `rest space`;
the rest keep the name of the file that used to live there (old `Item.SData`, `Monster.SData`, map `.wld`, effect textures...).
Every free entry seen so far has `length = 1`, so the field may not be a length here. How the updater sizes free blocks is *open*.

## Contents (version 108 client)

20,412 files, about 1.97 GiB referenced. By size: Sound (wav) 408 MiB, world (`.wld`, `.dg` dungeons) 402 MiB, Entity (`.smod` static
models) 392 MiB, Monster 208 MiB, Character 197 MiB, interface 178 MiB. By type: `.dds` 8,983 files / 813 MiB, `.wav` 408 MiB,
`.tga` 197 MiB, `.dg` 156 MiB. There are 9 `.SData` tables (6.3 MiB): the client-side item, monster, skill... definitions.

## Integrity (`sah.py check`)

- No entry points outside `data.saf`, and no entries overlap.
- **Unreferenced space: 252 MB (10.6 %) of `data.saf`.** Most file entries are separated by 1 byte. The large gaps (up to 2.9 MB) are
  space released by patches that was never reclaimed.
- **5 duplicate paths that differ only in case**, all in `world/dungeon/`: `f_dun_hell.dg`, `guildanteroom_d.dg`, `guildanteroom_l.dg`,
  `r1_dun3.dg`, `b3_dun1_1f.dg`. In the case checked (`R1_DUN3.dg` / `r1_dun3.dg`) both copies have the same length and version
  but sit at different offsets. Which one the client opens depends on how its path lookup compares names (the lookup keeps the later entry, see below).

## How the client looks files up (confirmed)

- `0x40d530` opens `data.sah`; accepts the "SAH" header (64-bit offsets) and an older headerless form (32-bit offsets).
- `0x40d340` reads the tree; each directory keeps a `std::map<u32 hash, node*>`. The key is **djb2 of the lower-cased name**
  (`h = 0x1505; h = h*33 + c`, ASCII A–Z folded, signed chars) computed in `0x40a490`. Insertion uses `operator[]`, so for
  two entries with the same name (in any case) **the later entry wins**; distinct names with equal hashes would also shadow each
  other (none in this archive). Lookup goes through `safopen` (`0x5964e0`, "SAFile.cpp"), which logs `File Not Open [path]`.
- The client never reads the free list (it stops after the tree); only the updater uses it.

## Repack

A repacking tool (the client repository's `tools/repack.py`) wrote a new `data.sah`/`data.saf` pair, checked it against the original byte for byte per file,
and a swap tool exchanged the pairs; the original pair is kept so the swap can be undone.

- 2,369,651,955 → 2,099,924,489 bytes (−270 MB): no gaps, empty free list.
- The 5 duplicate dungeon entries were removed, keeping the later one as the client does (`B3_DUN1_1F.dg` had two different versions;
  the second, which the client used, is kept).
- 8 files the client requests but patch 108 lacked were restored from later official US patches: `Interface/Icon/116.DDS`, `131.DDS`
  (ps0132), `29.dds` (ps0160), `icon_skill3.dds` (ps0288), `invasionUI_Background/Icon/Insignia.tga` and `talk1_close_button_3.tga`
  (ps0120). Not restored: `world/1.svmap` (ps0193, later-episode server map data).
  The other ~59 "missing" files (icons 25–28, 30, 41, 101–141, `icon_skill4–10`, a few effects, login lightmaps) were **never shipped
  in any US patch**; the client probes numbered files, so these gaps were the same in the official game.
- Repacking does not speed up loading: the lookup is already a per-folder hash map (see `graphics.md`).

## SData tables (client copies)

| File | Encrypted | Plain size |
|---|---|---|
| `Item/Item.SData` | SEED | 2,960,114 |
| `Npc/NpcQuest.SData` | SEED | 3,104,237 |
| `Character/Skill.SData` | SEED | 221,910 |
| `Monster/NpcSkill.SData` | SEED | 158,836 |
| `Monster/Monster.SData` | SEED | 132,986 |
| `Cash.SData` | SEED | 24,816 |
| `Character/KillStatus.SData`, `Npc/PriestTalk.SData`, `Npc/GuildHouse.SData` | no | — |

**Wrapper (confirmed):** a 40-byte ASCII signature (compared with the string at `0x6a9ae8`), a u32 CRC-32 of the plaintext (the usual
zlib CRC, table at `0x6f4d40`, function `0x5a99a0`), a u32 plaintext size, 16 zero bytes, then the zero-padded plaintext as 16-byte
**ECB** blocks through the SEED block function `0x5aff00` with a fixed round-key table at `0x6f5140` (decrypt loop `0x5a9a50`). The
key material is that table in `game.exe`; it is not reproduced here. All six encrypted files decrypt with a matching CRC, and
re-encrypting gives back the original bytes exactly (the client repository's `tools/sdata_crypt.py`, a port of the Parsec SEED code). The Item loader ignores
a bad CRC; the other five loaders reject it (`re-engine.md`, "The SData tables").

Plaintext `Item.SData` starts `u32 100` (max type), then per type `u32 count` and records with length-prefixed name and description
strings. Record layouts: Parsec's `Shaiya/Item/` sources and the SData pages of this section (`item-sdata-vs-defs.md`,
`skill-monster-sdata-vs-defs.md`, `npcquest-sdata-vs-world.md`).
