Patch kit for the original Shaiya EP4.5 game server (ps_game.exe)
==================================================================

A checked, reversible byte patcher for people still running the original EP4.5
server files. It fixes bugs and exploits in ps_game.exe on disk. It does not touch
the database, the client or any other server file, and it never runs the
executable. The executable itself is not included: you patch your own copy.

The bugs, how each shows in play, and what each fix does are written up at
https://tylerd.me/shaiya/community-fixes/ and the byte lists at
https://tylerd.me/shaiya/server-patches/

Which file
----------
Exactly one build of ps_game.exe:

  SHA-256  0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547
  size     1,323,008 bytes
  built    2009-07-15 (PE timestamp, UTC)

The patcher refuses any other file. The addresses are specific to this build; they
do not apply to other episodes or recompiles.

Using it (Python 3, no other packages)
---------------------------------------
  python3 patch_ps_game.py list [ps_game.exe]        every fix and its state
  python3 patch_ps_game.py status ps_game.exe        which fixes are applied
  python3 patch_ps_game.py apply  ps_game.exe        all clear-bug fixes
  python3 patch_ps_game.py apply  ps_game.exe <id>   only the named fixes
  python3 patch_ps_game.py revert ps_game.exe all    undo every applied fix

"apply" with no ids applies the clear-bug fixes. The design-choice fixes change how
the game plays and are applied only when named. The first "apply" writes
ps_game.exe.bak next to the file. Every change is printed as before/after bytes.

Stop the server before patching, patch a copy first, and keep the backup.

Safety
------
Before any change the tool rebuilds the original image by undoing every patch it
recognises; that image must hash to the SHA-256 above. So it only ever edits this
exact build, whether the file is untouched or already carries some of these fixes,
and it refuses a file with edits it does not know. "apply" and "revert" can be run
again safely, and a full revert gives back the original file byte for byte.

What was checked
----------------
Every fix was assembled, written into a copy of the build, disassembled again and
checked against the surrounding code, its jump targets and the stack ("verified in
disassembly"). The input filters and the later fixes were also run
instruction by instruction in an x86 emulator ("verified in disassembly and
emulation"). None of the fixes has been run on a live server. Test on a spare copy
with a spare database before using them for real.

Later versions of the kit add fixes after the earlier ones: a fix keeps its bytes
and addresses, so a file patched with an earlier version is still recognised.

Code cave
---------
Most fixes need a few extra instructions. These go in the unused, already mapped
padding after the last function in .text (from VA 0x520250). While any such fix
is applied, the tool also raises .text's VirtualSize (file offset 0x200) from
0x11f244 to 0x120000 so that region is mapped explicitly, and puts it back when
the last one is reverted.

Files
-----
  patch_ps_game.py   the patcher
  fixes.json         the byte lists: for each fix, every changed range with its
                     address, file offset, original bytes and patched bytes
  README.txt         this file

No warranty. Shaiya and its trademarks belong to their owners.
