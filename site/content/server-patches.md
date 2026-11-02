# Patch kit for the original ps_game.exe

A checked, reversible byte patcher for people still running the original EP4.5 game server. It fixes KIT_COUNT of the bugs on
[Known bugs and fixes](original/community-fixes.md) in `ps_game.exe` on disk. This page has the download, how to use it, and every byte it
changes.

## In short

- **What it is:** a small Python script and a list of byte changes. You run it on your own copy of `ps_game.exe`; the executable is
  not part of the download.
- **Which file:** only the EP4.5 `ps_game.exe` with SHA-256 `0fb7c9f1…` (below). The script refuses any other file, so it cannot
  damage a different build.
- **What it changes:** the 30 clear bugs are fixed by default; two fixes that change how the game plays are applied only when you ask
  for them. It makes a backup first, prints every byte it changes, and can undo any fix.
- **How far it was checked:** every fix was checked in the disassembly, and most were also run in an x86 emulator; none was run on a
  live server. Try it on a spare copy and a
  spare database first.

## Download

[ps_game-patch-kit.zip](KIT_ZIP): `patch_ps_game.py` (the patcher), `fixes.json` (the byte lists) and `README.txt`.

SHA-256 of the zip: `KIT_SHA256`

It needs Python 3 and nothing else. Nothing in it is a game file.

## The build

These patches are for exactly one build of `ps_game.exe`:

| | |
|---|---|
| SHA-256 | `0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547` |
| size | 1,323,008 bytes |
| built | 2009-07-15 (PE timestamp, UTC) |

The addresses on this page and on the bug page are specific to this build; they do not apply to other episodes or recompiles. In this
file the virtual address of a byte in `.text` is its file offset plus 0x400000.

## Using it

```
python3 patch_ps_game.py list [ps_game.exe]        # every fix and its state
python3 patch_ps_game.py status ps_game.exe        # which fixes are applied
python3 patch_ps_game.py apply  ps_game.exe        # all clear-bug fixes
python3 patch_ps_game.py apply  ps_game.exe <id>…  # only the named fixes
python3 patch_ps_game.py revert ps_game.exe all    # undo every applied fix
```

`apply` with no ids applies the clear-bug set. The design-choice fixes (`fire-thorns-aggro`, `op-exclusive`) are applied only when
named. The first `apply` writes `ps_game.exe.bak` next to the file. Stop the server before patching.

**Safety.** Before any change the tool rebuilds the original image by undoing every patch it recognises; that image must hash to the
SHA-256 above. So it only edits this exact build, whether the file is untouched or already carries some of these fixes, and it refuses
a file with edits it does not know. `apply` and `revert` can be run again safely, and a full revert gives back the original file byte
for byte (an automated test applies and reverts every fix, the default set and mixed selections on a copy).

Many community fixes are Cheat Engine tables that edit the running process. This kit edits the file instead, so a fix survives a
restart and needs no injector.

## What "verified in disassembly" means

Each fix was assembled at its final address, written into a copy of the build, disassembled again and checked against the code around
it, its jump targets and the stack. "Verified in disassembly and emulation" adds that its instructions were run in an x86 emulator
(Unicorn) against prepared registers and memory and compared with an independent model, and with the original code on every path the
two should share. That covers the four input filters (with the original code they call) and the eleven fixes added in the second and third passes
(`myshop-price-wrap`, `npc-shop-price-wrap`, `remake-distinct`, `enchant-seed`, `quest-end-npc`, `chat-markup-filter`,
`guild-bank-bound`, `gm-target-filter`, `exchange-count-guard`, `party-kick-leader`, `repair-shield-slot`), which also passed a check that no branch anywhere in `.text` lands
inside one of their hooks. **None of the fixes has been run on a live server.** That is the test still owed, and the reason to try a
spare copy first.

## The code cave

30 of the fixes need a few extra instructions. These go in the unused, already mapped padding after the last function in `.text`
(virtual address 0x520250 onward; the fixes use 0x520250 to 0x520812). Fixes added later go after the earlier ones, so a fix keeps its
bytes and addresses from one version of the kit to the next. Such a fix replaces a few bytes at its site with a jump to its
own slot there, which runs the replaced instructions plus the fix and jumps back.

While any cave fix is applied, the tool also raises `.text`'s VirtualSize (file offset 0x200) from 0x11f244 to 0x120000, so the region
is mapped explicitly, and restores it when the last cave fix is reverted. (The whole raw section is mapped anyway, which is why
community patches that use the same padding work without this change; the kit makes the mapping explicit.)

## Not in this kit yet

Planned for a later version (each is on the bug page):

- the checks that an NPC is near (merchants, blacksmiths, quest NPCs), which need a distance confirmed against a running client;
- the rune used right after a skill, which needs a per-player field for the time of the last skill, and none is proven free.

Not patchable in `ps_game.exe` alone: the item mall's points (the database agent would have to report a failed purchase) and the
trade's item-by-item saving (the database agent would have to write a trade in one step). See
[the trade's weak points](original/community-fixes.md#trade-other) and the
[held-back list](original/community-fixes.md#held-back-until-there-is-a-fix).

## The byte lists

Every change of every fix, as in `fixes.json`. "In place" bytes replace code at the site; "code cave" bytes go in the padding
described above (originally zero).

<!-- kit:fixes -->
