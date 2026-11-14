# Finding unused files in the archive

This page explains how to tell which files in the US client's `data.saf` the game never opens (build 2010-04-26, patch 108), what the
evidence looks like, what looked unused but is used, and one mistake worth avoiding. The method works on any client of this family; the
counts are for the 108 archive. Status words: **confirmed** (read in `game.exe`, tested in play), *inferred*.

## In short

- **The client opens [archive](glossary.md#archive) files only by a full name,** either a literal in `game.exe` or one read from another data file (model
  lists, maps, effect lists, SData). So a model, effect or map file named in none of them cannot be opened.
- **About 500 files (about 109 MB) were never opened:** development leftovers, test models, tool binaries, Explorer thumbnail caches
  and a 2006 copy of a dungeon. Removing them left the game unchanged (tested in play).
- **A first attempt removed 58 files that are used.** Map files store names in fixed 256-byte buffers, and stale bytes after the
  terminating NUL ran into the next name when scanned naively.
- **Several files that look broken are used:** the client tolerates malformed models, so a file failing a strict parser is not
  evidence that it is unused.

### Using this page

- **If you patch or mod the client:** before removing any file, run the client repository's `tools/unreferenced.py`; it lists files no name points to. Treat the "looked broken but used" list as a do-not-remove list.
- **To check a claim yourself:** pick a file from "What was found", search for its name in `game.exe` strings and the decrypted SData ([tools-and-method.md](tools-and-method.md)); the archive layout is in [data-archive.md](data-archive.md).

## Method

`game.exe` opens archive files only by a full name that is either a literal in the exe or read from another data file. For models,
effects and maps the exe has **no format string and no bare extension** (`%s.EFT`, `.3DE`, …); the only run-time names are `%d.wld`,
`%d.txt`, minimap and icon `%d.tga|dds`, `%s_L.dds`, `%s_A%d.dds` and `%s_L%d.dds`. A model, effect or map file whose name appears in
neither the exe, nor any other archive file, nor the decrypted SData cannot be opened. Textures, sounds and animations are not judged
by this method, because some of their names are built at run time. A scanning tool (the client repository's `tools/unreferenced.py`) implements it.

## What was found

**Strong evidence (61 files, 75.6 MB):**

- Dungeon `.dg_pv` previews (12 files, 60.3 MB): no `_pv` string exists in `game.exe`; the dungeon loader opens only `<name>.dg` and
  its `_L<n>.dds` lightmaps.
- A 2006 `.zip` copy of a dungeon (3.6 MB); the client opens no `.zip` from the archive.
- One dungeon and its effect file that no map names; one test map.
- Windows `Thumbs.db` caches (24 files), an effect tool executable, an obsolete model list (`.TOL`), an old `sysmsg.txt` (the client
  opens `sysmsg-uni.txt` only), and `PriestTalk.SData` (no SData is opened except by its literal name, and this one has none).
- Test models and notes: test `.3DE`, `.mani`, `.dds` and text files named nowhere.
- One static model that is in no entity list and is the only `.SMOD` that does not parse.

**Probably unused (415 files, 33.0 MB):** names written nowhere, but the files look like real content left over from development or other
episodes: 166 `.3DE`, 181 `.SMOD` (22.1 MB), 15 `.VANI` (flags, horses, fish), 13 `.mani` (login-scene animations the login map does
not use), 37 `.3DC`, 28 `.3DO`, 32 `.EFT` and one `.Seff`. Removing them does not change the game; keeping them costs only disk space.

## Looked broken or unused, but the client uses them

| Kind of file | Why it seemed removable | Evidence it is used |
|---|---|---|
| `Item/3DO/arrow.3DO` | does not parse with the plain `.3DO` layout | its name is a literal in `game.exe`; it carries vertex-animation frames (`formats.md`) |
| `Mob_Rend_01_A/B.3DC` | the texture-name field is not empty, a strict parser rejects it | parts of ten `monster.MON` records that `Monster.SData` uses |
| `Npc_XXX_03.3DC` | a second mesh is appended | model of four `NPC.MON` records; the client reads only the first mesh |
| `fireballTest.3DE` | "test" in the name | listed by 14 `.EFT` files |
| three malformed `.3DE` files | do not parse | listed by `.EFT` files; the client tolerates the malformed layouts (see `formats.md`) |

Three more malformed `.3DE` files are named nowhere and fall in the probably-unused group.

## The 256-byte buffer lesson

A first attempt listed 534 files and removed 58 that maps and effect lists do name. `.wld` names are fixed 256-byte buffers, and the
bytes after the NUL are stale stack memory from the original map tool. A scan that searched all bytes read, for example,
`…abcout_crag7_L.SMOD` and treated the real name `out_crag7_L.SMOD` as unreferenced; the client then stopped with "map data load
error!!!" in that map. The fix: the scanner counts every suffix of a match as a reference, the list was pruned, and every asset named by
every map (4,268 references) was checked present. The pruned list (476 files, 109 MB) passed a test run. Any tool that reads `.wld`
names must stop at the first NUL (`formats.md`).

## Loose files next to the executable

Two files in the client folder are not needed: `zlib.dll` (1.1.3, 2000) is imported or named by nothing that ships (`game.exe` contains
zlib 1.2.3 and the original `Updater.exe` zlib 1.1.3 statically), and `ShaiyaUS.ini` (330 bytes, encrypted, 2007) is named by none of
`game.exe`, the configuration tool or the updater, in ASCII or UTF-16; it likely belonged to the original launcher or GameGuard. The game
launched normally without either. `ijl15.dll` is imported by `game.exe`, and the configuration tool, `CONFIG.INI`, `T_CONFIG.ini`,
`SORT_LIST.INI`, `notice.txt`, `tip.txt` and `Version.ini` are each named by `game.exe`, the configuration tool or the updater.
