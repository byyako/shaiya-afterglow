# The effect asset collector (`eft.exe`)

The original US archive carried one Windows program, `Effect/eft.exe` (49,152 bytes, SHA-256
`e67b12c45c47dbafd7446196c514504fe15790d586293badedb9e7b9f2c2ec46`). The client never opens it, and the cleanup removed it
(`cleanup.md`). This page explains what it is, what it reads and writes, how its code relates to the effect loader in `game.exe`, and
whether it is any use for making effects today. Everything was read statically in Ghidra and with a byte-level PE reader. The program
was never run. Addresses are virtual addresses ([VA](glossary.md#va)) in `eft.exe` unless they are marked `game.exe` (the 2010-04-26 build, patch 108).
**confirmed** means read in a decompile with the function and address given; **confirmed (disasm)** means read in the disassembly
or the file bytes; *inferred* means a reasonable reading that the code does not state by itself.

## In short

- `eft.exe` is a small developer utility from August 2005, not an effect editor. It is one dialog box: you drop `.EFT` files onto it,
  and it copies every `.3DE` mesh and `.dds` texture those effects name into two new folders, `3DE_COPY` and `DDS_COPY`.
- It was meant to sit in the developers' effect folder, next to the `3DE` and `DDS` folders the game reads (`data/Effect/3DE`,
  `data/Effect/DDS`). It most likely ended up in the archive because the whole folder was packed.
- It reads only the start of an `.EFT` file: the signature and the two name lists. It never reads the emitters or sequences, so it
  settles none of the effect fields that `formats.md` marks unknown or inferred.
- Its header code follows the same steps as the header part of the game's loader, quirks included. The machine code differs because
  the two were built with different compilers.
- Its main use today is as a recipe: a few lines of script do the same job when an effect has to be packed with its meshes and
  textures. Running it is not advised. It trusts every length and name in the file, so a crafted `.EFT` can overflow its stack or
  overwrite files outside its folder.

### Using this page

- **If you make effects:** read section 5 ("Usefulness today") first; it says what to do instead of running `eft.exe`.
- **If you patch or mod the client:** sections 3 and 4 show what the `.EFT` header holds; the rest of the format is in [formats.md](formats.md).
- **To check a claim yourself:** open `eft.exe` in Ghidra ([tools-and-method.md](tools-and-method.md)) and start from the addresses in section 3.

Related pages: [formats.md](formats.md) (the `.EFT` and `.3DE` layouts and the loaders), [cleanup.md](cleanup.md) (why the file was
removed), [data-archive.md](data-archive.md) (the archive), [tools-and-method.md](tools-and-method.md) (how the analysis was done).

## 1. Identity

| Field | Value | Status |
|---|---|---|
| Format | PE32, i386, GUI subsystem (2), image base `0x400000`, entry `0x401613` | confirmed (disasm) |
| Link time | 2005-08-03 07:19:23 UTC (header time stamp `0x42F06FFB`) | confirmed (disasm) |
| Linker | 6.0 (optional header); OS/subsystem version 4.0; checksum 0; no relocations | confirmed (disasm) |
| Sections | `.text` (`0x401000`, 0x58a4 bytes), `.rdata` (`0x407000`, 0xaa6), `.data` (`0x408000`, 0x3f9c), `.rsrc` (`0x40c000`, 0x108); file alignment 0x1000 | confirmed (disasm) |
| Packer | none: standard section names and flags, byte entropy 6.65 in `.text` (normal for x86 code), plain import table, and the entry point is the usual C runtime start-up | confirmed (disasm) |
| Security flags | DllCharacteristics 0 (no DEP or ASLR opt-in), no certificate (no security directory), no manifest | confirmed (disasm) |
| Debug info | no debug directory, so no PDB path; no version resource; no build strings | confirmed (disasm) |
| Resources | one dialog, id 101, language 1042 (Korean); no icon, menu or string table | confirmed (disasm) |

**Imports** (60 functions, all through the import table at `0x407000`):

| DLL | Functions | Used by |
|---|---|---|
| `USER32.dll` (8) | `CreateDialogParamA`, `ShowWindow`, `GetMessageA`, `TranslateMessage`, `DispatchMessageA`, `EndDialog`, `SetDlgItemTextA`, `MessageBoxA` | the dialog (`0x401390`, `0x4011d0`) |
| `SHELL32.dll` (3) | `DragAcceptFiles`, `DragQueryFileA`, `DragFinish` | drag and drop (`0x4011d0`) |
| `SHLWAPI.dll` (1) | `PathIsDirectoryA` | output folders (`0x4011d0`) |
| `KERNEL32.dll` (48) | `CopyFileA`, `CreateDirectoryA`, `GetModuleFileNameA`; the rest (`CreateFileA`, `ReadFile`, `WriteFile`, heap, environment, code-page and process functions) | the tool; the static C runtime |

The C runtime also loads `user32.dll` by name (`LoadLibraryA` in its message-box helper at `0x405baf`) to show its own
"Runtime Error!" box. Nothing in the
program touches the registry, the network, other processes or the console. (confirmed)

**Compiler and runtime.** The Rich header lists Visual C++ 6.0 tools: C compiler build 9782 (57 objects), C++ compiler build 9782
(2 objects), linker build 8034, MASM 6.13 build 7299, the resource converter build 1735, and 60 import entries (confirmed (disasm)).
Build 9782 is the C/C++ compiler of Visual C++ 6.0 SP6 (*inferred* from public Rich-header tables). The runtime is the
**statically linked, single-threaded C library** (LIBC): its "R6xxx" messages are in `.rdata` (`0x4071b0`…), and there are no thread or
TLS imports (confirmed (disasm)). **No MFC:** no MFC DLL import, no MFC class names or message maps, and the program is a plain Win32
dialog (confirmed). The two C++ objects are most likely the tool's own source file(s), compiled as C++ but using only C (*inferred*).
`game.exe`, for comparison, was linked with linker 7.10 (Visual C++ .NET 2003) almost five years later.

## 2. What it is and how it was used

The tool is a **dependency copier for effect files**. A developer put `eft.exe` in the effect folder (the folder that holds `3DE\` and
`DDS\`), started it, and dropped one or more `.EFT` files onto the window. For each file the tool read the two name lists at the start
and copied every listed mesh from `3DE\` to `3DE_COPY\` and every listed texture from `DDS\` to `DDS_COPY\`. At the end it showed a
"copy complete" box. The result is a folder pair with exactly the meshes and textures that a chosen set of effects needs: what you
want when you hand a set of effects to another team or pack them into a patch (*inferred*: the code shows what it does, not why).

| Function | Address | What it does | Status |
|---|---|---|---|
| `WinMainCRTStartup` | `0x401613` | C runtime start-up; calls `WinMain(GetModuleHandleA(NULL), …)` | confirmed |
| `WinMain` | `0x401390` | stores the folder of the exe, creates the dialog, runs the message loop | confirmed |
| Dialog procedure | `0x4011d0` | drop handling and closing (below) | confirmed |
| `CopyEffectAssets` | `0x401000` | reads one `.EFT` and copies its files (section 3) | confirmed |
| `sprintf`, `fopen`, `_fsopen`, `fread`, `fclose` | `0x401450`, `0x401600`, `0x4015e0`, `0x4014f8`, `0x4014a2` | C runtime (identified by their bodies: string-stream flags `0x42`, share mode `0x40`, the `FILE` buffer logic) | confirmed |

`WinMain` (`0x401390`) calls `GetModuleFileNameA` into a global at `0x40a948` and cuts the string after the last backslash, which
leaves the exe's folder with a trailing `\`. Every path the tool builds starts with it. The working folder and the drop location do
not matter. (confirmed)

**The window.** One dialog, resource 101, 266 × 141 dialog units, font "System" 10, no title text. It is created **modeless**
(`CreateDialogParamA` at `0x4013df`, shown with `SW_SHOW`) and driven by `WinMain`'s `GetMessage` loop, which also stops when the
flag at `0x40aa4c` is set (confirmed (disasm)). Controls:

| Id | Kind | Text (resource, UTF-16) | Behaviour |
|---|---|---|---|
| 1 | default push button "OK" | `OK` | closes the tool (`WM_COMMAND` 1) |
| 2 | push button | `Cancel` | closes the tool (`WM_COMMAND` 2) |
| 1001 | static text, 204 × 55 units | `EFT 파일 드래그&&드롭` ("EFT file drag & drop") | replaced by the path of each file while it is processed |

The two message-box strings are ANSI (code page 949) in `.data`: text `복사완료` ("copy complete", `0x408070`) and caption `완료`
("done", `0x40807c`). On a system whose ANSI code page is not Korean they would show as garbage characters (*inferred*). There is no
menu and there are **no command-line arguments**: `WinMain` reads only its first parameter (the instance handle; the disassembly at
`0x401395` reads `[ESP+0x28]` and nothing else), so the command line is ignored (confirmed (disasm)). Dropping is the only input.

The dialog procedure, renamed (`0x4011d0`, confirmed; the folder-creation `sprintf`s pass the exe folder at `0x40a948`):

```c
INT_PTR CALLBACK EftDialog(HWND dlg, UINT msg, WPARAM wp, LPARAM lp)
{
    char buf[260];                                  /* also receives the names read by CopyEffectAssets */
    switch (msg) {
    case WM_INITDIALOG: DragAcceptFiles(dlg, TRUE); g_dlg = dlg; return TRUE;
    case WM_DROPFILES: {
        HDROP drop = (HDROP)wp;
        sprintf(buf, "%s3DE_COPY", g_exeDir); if (!PathIsDirectoryA(buf)) CreateDirectoryA(buf, NULL);
        sprintf(buf, "%sDDS_COPY", g_exeDir); if (!PathIsDirectoryA(buf)) CreateDirectoryA(buf, NULL);
        int count = DragQueryFileA(drop, 0xFFFFFFFF, buf, 260);
        for (int i = 0; i < count; i++) {
            DragQueryFileA(drop, i, buf, 260);
            SetDlgItemTextA(dlg, 1001, buf);       /* show the file being processed */
            CopyEffectAssets(buf);                  /* result ignored */
        }
        DragFinish(drop);
        MessageBoxA(NULL, TEXT_COPY_COMPLETE, TEXT_DONE, MB_OK);
        return TRUE;
    }
    case WM_COMMAND: if (LOWORD(wp) != IDOK && LOWORD(wp) != IDCANCEL) return TRUE;   /* else close */
    case WM_CLOSE: case WM_DESTROY: g_quit = 1; EndDialog(dlg, 0); return TRUE;
    }
    return FALSE;
}
```

The tool accepts any dropped file: anything whose first three bytes are not `EFT` is skipped without a message. It does not recurse
into dropped folders. (confirmed)

## 3. File I/O

Every file the tool touches:

| File | Access | Function (call site) | Status |
|---|---|---|---|
| its own path | `GetModuleFileNameA`, to get the folder | `0x401390` (`0x4013a5`) | confirmed |
| `<exe folder>3DE_COPY`, `<exe folder>DDS_COPY` | created with `CreateDirectoryA` when `PathIsDirectoryA` says they are missing, on every drop | `0x4011d0` (`0x401286`, `0x4012b0`) | confirmed |
| each dropped file | `fopen(path, "rb")` (mode `0x40806c`), then `fread` | `0x401000` (`0x401017`) | confirmed |
| `<exe folder>3DE/<name>` → `<exe folder>3DE_COPY/<name>` | `CopyFileA(src, dst, FALSE)`: overwrites, result ignored | `0x401000` (`0x40111a`) | confirmed |
| `<exe folder>DDS/<name>` → `<exe folder>DDS_COPY/<name>` | the same | `0x401000` (`0x4011a7`) | confirmed |

The format strings are `%s3DE/%s` (`0x40805c`), `%s3DE_COPY/%s` (`0x40804c`), `%sDDS/%s` (`0x408040`), `%sDDS_COPY/%s` (`0x408030`),
`%s3DE_COPY` (`0x408090`) and `%sDDS_COPY` (`0x408084`) (confirmed (disasm)). The tool writes no file of its own: no log, no settings,
no list, and it never writes an `.EFT`. The `.Seff` files in the same archive folder are not read by it.

The reader, renamed (`0x401000`, confirmed; checked against the disassembly for the `sprintf` arguments):

```c
int CopyEffectAssets(char *path)                      /* path: the dialog's 260-byte buffer */
{
    char sig[4] = {0}, src[260], dst[260];
    int len = 0, n3de = 0, ndds = 0;
    FILE *f = fopen(path, "rb");
    if (!f) return 1;                                 /* unreadable file: silently skipped */
    fread(sig, 1, 3, f);
    if (strcmp(sig, "EFT") != 0) { fclose(f); return 0; }        /* "EFT" at 0x408068 */
    fread(&n3de, 4, 1, f);
    for (int i = 0; i < n3de; i++) {
        fread(&len, 4, 1, f);
        fread(path, 1, len, f);                       /* name over the dropped path; no length check */
        sprintf(src, "%s3DE/%s", g_exeDir, path);
        sprintf(dst, "%s3DE_COPY/%s", g_exeDir, path);
        CopyFileA(src, dst, FALSE);
    }
    fread(&ndds, 4, 1, f);
    for (int i = 0; i < ndds; i++) {
        /* the same with "%sDDS/%s" and "%sDDS_COPY/%s" */
    }
    return 1;                                         /* f is never closed */
}
```

**The layout it uses** is the first four lines of the `.EFT` layout in `formats.md`, and nothing after them:

| Field | `eft.exe` (`0x401000`) | `game.exe` loader (`0x546710`) | Same? |
|---|---|---|---|
| `char[3]` signature | read into a zeroed 4-byte buffer, `strcmp` with `"EFT"` (`0x408068`) | read into a zeroed 4-byte buffer, 4-byte compare with `"EFT"` (`0x6b6120`, `REPE CMPSB` at `0x546783`) | yes |
| `i32` 3DE count | a local, loop count | object `+0x04`; pointer array `+0x18` | yes |
| 3DE names: `i32 len`, `len` bytes | read over the 260-byte drop buffer; no length check; the stored NUL ends the name | read into a 268-byte stack buffer; no length check; passed to the mesh cache `0x538af0` with `"data/Effect/3DE"` (`0x6b6110`) | yes |
| `i32` DDS count | a local | object `+0x08`; pointer array `+0x1c` | yes |
| DDS names | as for 3DE | texture cache `0x538a00` with `"data/Effect/DDS"` (`0x6b6100`) | yes |
| emitters, sequences | not read | read (`formats.md`) | — |

All rows confirmed (both decompiles, the game side also in the disassembly). **Version:** neither program has a version field or
accepts any other signature. The 2005 tool and the 2010 client read the same header, so it did not change over those five years. The
tool says nothing about the emitter record, so it cannot tell whether that record changed. (confirmed for the header; the rest is out of
the tool's reach)

**Fields whose meaning the tool reveals:** none in the emitter or sequence records. It has no field names, ranges, defaults or labels,
because it has no editing UI and never reads that far. What it does confirm about the header:

- Names are **stored with their trailing NUL inside `len`**. The tool depends on it: it never terminates the string itself, so a name
  without its NUL would pick up the tail of the dropped path that the buffer held before (confirmed). Replaying the tool's reads over
  every `.EFT` in the 108 archive (446 files: 1,835 mesh and 4,780 texture references; 478 files in the original archive) finds no
  name without its NUL and none longer than 30 bytes (checked with a script that reads the archive, not by running the tool).
- The `3DE` and `DDS` folder names, and their place next to the `.EFT` files, are the ones the game uses (`data/Effect/3DE`,
  `data/Effect/DDS`) (confirmed).
- Five mesh names in three files point at `.3DE` files that are in neither archive, and emitters in those files use them. For those
  names the tool's `CopyFileA` fails silently. What the game does with them is now in `formats.md` (`.EFT`, header details).

## 4. Code shared with `game.exe`

| `eft.exe` | `game.exe` | What matches | Status |
|---|---|---|---|
| `0x401000` (all of it) | `0x546710`, from the start to the end of the DDS list | same order of reads; the 3-byte signature in a zeroed 4-byte buffer; `i32 len` + `len` bytes per name; no length check; **"cannot open" returns 1 and "wrong signature" closes the file and returns 0 in both** | confirmed; shared source *inferred* |
| `"%s3DE/%s"` (`0x40805c`), `"%sDDS/%s"` (`0x408040`) | `"%s/%s"` (`0x6a7f34`) in the mesh cache `0x538af0` with `"data/Effect/3DE"` / `"data/Effect/DDS"` | folder names and the folder/name join | confirmed (disasm) |

The matching return values are the strongest sign that the tool's reader was copied from the engine's loader source and cut after the
two lists (*inferred*). The machine code is **not** shared: the tool is Visual C++ 6.0 with the CRT `fopen`/`fread`, while the game is
Visual C++ .NET 2003 reading through its archive (`0x40c550` opens, archive first, then disk; `0x58c860` reads). Three 16-byte pieces
of the tool's code (`0x40102b`, `0x4010b8`, `0x40124e`) occur nowhere in `game.exe` (confirmed (disasm)). The game's own copy of the
header code (excerpt, renamed; `0x546710`, confirmed):

```c
int EffectFile::Load()
{
    char sig[4] = {0}, name[268];
    int len = 0;
    ArchiveFile *f = ArchiveOpen(this->path, "rb");      /* 0x40c550; path at +0x28 */
    if (!f) return 1;                                    /* missing file counts as loaded */
    ArchiveRead(sig, 1, 3, f);                           /* 0x58c860 */
    if (memcmp(sig, "EFT", 4) != 0) { ArchiveClose(f); return 0; }
    ArchiveRead(&this->meshCount, 4, 1, f);              /* +0x04 */
    this->meshes = new Mesh3de *[this->meshCount];       /* +0x18 */
    for (int i = 0; i < this->meshCount; i++) {
        ArchiveRead(&len, 4, 1, f);
        ArchiveRead(name, 1, len, f);                    /* no length check */
        this->meshes[i] = g_meshCache.Get("data/Effect/3DE", name);   /* 0x538af0; null if it fails */
    }
    ArchiveRead(&this->textureCount, 4, 1, f);           /* +0x08; then textures via 0x538a00 into +0x1c */
    /* ... emitters and sequences (formats.md) ... */
}
```

## 5. Usefulness today

**The tool itself.** It does one small job: list and copy the meshes and textures that a set of effects needs. That job is still
useful when a new or changed effect has to go into a patch, because the client looks up every name in the archive and an effect whose
`.3DE` or `.dds` is missing loses that mesh or texture. A script does the same job in a few lines (`tools/formats.py` already reads the
two name lists) and can do more: check that every name exists, report names used by no effect, and write the archive paths directly.
The tool cannot make or edit effects. It has no emitter fields, no preview and no writer.

**Its format knowledge.** Only the header, which `formats.md` already had. Nothing in it narrows the open emitter fields (`+dc`, the
sequence `delay`, the exact facing matrices).

**What a replacement effect tool would need** (from `formats.md` and the game's loader and renderer):

- A reader and writer for the whole `.EFT` layout in `formats.md`, round-trip exact on all 446 files (the parser in
  `tools/formats.py` already consumes every file to the last byte).
- `.3DE` reading with the irregular files listed in `formats.md`, and DXT `.dds` loading.
- A preview that follows the game's update and draw (`0x54a0f0`/`0x54acd0` update, `0x5484d0` billboards, `0x548070` meshes, the
  blend list at `0x6b61f0`, colour/velocity/size keys, the texture flip-book, the 200-particle limit), and sequence playback.
- Validation the client does not do: indices into the mesh, texture and scene lists in range, names short (the client reads them into
  a 268-byte stack buffer with no length check), NUL-terminated and present in the archive, at most 200 particles per emitter.
- Packing: put the `.EFT`, its `.3DE` and `.dds` into the archive (`data-archive.md`), which is the step `eft.exe` helped with.

**Risks of running it** (read from the code, not tested):

- **Stack overflow from a crafted file.** The name length comes from the file and is used unchecked to read into the dialog
  procedure's 260-byte stack buffer (`0x401000` reading into the buffer of `0x4011d0`). A long length overwrites that procedure's saved
  registers and return address. The program opts out of nothing (no DEP or ASLR flags, no stack cookie in VC6 code), so this is
  exploitable code execution (confirmed (disasm) for the overflow; exploitability *inferred*).
- **Writes outside its folder.** Names are not checked for `..\` or drive letters, and `CopyFileA` overwrites. A crafted name can
  copy any readable file over any writable file the account can reach (confirmed).
- **Quieter defects.** Copy failures and wrong files are ignored; the `.EFT` is never closed (one leaked handle per file); a name
  without its NUL builds a wrong path (confirmed).
- **Provenance.** It is unsigned code from 2005 whose only copy came out of a game archive; there is nothing to check it against.

With the shipped `.EFT` files the overflow cannot trigger (names are 30 bytes at most), but nothing the tool does needs the tool. The
safe path is to use its logic, not the binary. If someone ran it anyway: a throw-away virtual machine with no shared folders, a copy
of the effect folder only, and no network.

## 6. Open questions

| Question | What would settle it |
|---|---|
| Where is the real effect editor, the program that wrote the emitter records (and the scene and sequence names the client skips)? | Searching other client builds (earlier US patches, other regions) and their archives for further developer executables or editor file names. |
| Did `Effect/eft.exe` ship in every client, or was it packed into one archive by mistake? | Comparing the earliest available US client archives and other regions' archives for `Effect/eft.exe` and its hash. |
| Is the Rich build 9782 really Visual C++ 6.0 SP6 (and not SP5)? | A Rich-header comparison with binaries known to be built by VC6 SP5 and SP6 compilers. |
| Where does the emitter instance get its mesh (`+0x1dad0`) from the `.EFT` mesh list, and does a missing `.3DE` really draw as billboards? | Tracing the writer of `+0x1dad0` in the instance set-up (`0x549f80` and its callers); or a **live** look at the three monster attack effects named in `formats.md`. |
| What are the `.Seff` files in the same folder (`game.exe` names four, e.g. `data/effect/Weather.Seff` at `0x6a88f4`)? | Following those names to their loader; the format is not in `formats.md` yet. |
