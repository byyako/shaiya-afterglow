# Graphics and performance

This page covers the optional renderer controls in the companion DLL (see [The companion DLL](fixes-dll.md)), the loading timers, what the logs from
real play show, and the offline texture and archive audits. It describes the US `game.exe` (build 2010-04-26, patch 108). Status
words: **confirmed** (read in the binary, with address), *inferred*, **measured** (from logs of real play).

## In short

- **The game renders as fast as it can.** In seven play sessions (16x filtering, metrics on) it ran uncapped at about
  700–900 frames per second, focused or not. In-world stutter is not the problem; power use and heat are, especially with several
  clients open.
- **Loading is the real cost.** The first map load took 7.5–14.4 s, of which the nested WLD setup took 3.4–7.5 s.
- **Anisotropic filtering was nearly invisible** because only about a quarter of the draws were eligible; the rest were alpha-blended,
  depth-test-off or an unsupported vertex format. A later build also filters the second sampler (terrain detail and lightmaps).
- **With every option at its default the DLL installs no Direct3D hook at all.** No hook failure appeared in any log.
- **Not established:** any real FPS or loading gain, visual quality, long sessions, other graphics drivers.

### Using this page

- **If you patch or mod the client:** if you use a DLL like this, you try a renderer option by setting its key in the DLL's INI file and
  starting the game; "Classic" turns every option off again. Set `metrics` to true to get the graphics and loading logs,
  and read the numbers against "Measurements" below. The DLL itself is described in [The companion DLL](fixes-dll.md).
- **If you write a server:** nothing here depends on the server.
- **To check a claim yourself:** the "How it works" addresses are places in `game.exe` to open in a decompiler; the audits can be redone
  from the [archive format](data-archive.md) and [Tools and method](tools-and-method.md).

## Settings

The settings live in the DLL's INI file and are edited from a settings window in our launcher, which writes only the changed keys,
keeps comments, backs up the file first, and refuses to save while the client runs or if the file changed meanwhile. "Classic" clears
every option below plus features 21 and 35.

| Key | Default | Meaning |
|---|---|---|
| `foreground_fps` | 0 | 0 = uncapped, else 15–240. Best-effort sleep before `Present`; no busy wait, no timer-resolution or VSync change. The render thread also runs game logic, so a cap slows that loop too. |
| `background_fps` | 30 | The same while the game window is not focused. A missing key means 0. |
| `anisotropy` | 0 | 2/4/8/16 (clamped to the device). Applies to opaque, depth-tested, fixed-function 3D draws with a 2D texture and linear or weaker minification, on samplers 0 and 1. UI (XYZRHW) draws, alpha blending, shaders and state-block recording are excluded. State is read before and restored after each draw. |
| `borderless` | false | Windowed mode without the frame, same client size (no stretching). |
| `metrics` | false | Writes a graphics log about every 2 s and a loading log after map loads. |
| `[fixes] features` 21 / 35 | off | 21 = map resource preloading (synchronous, during the loading screen); 35 = extra glow controls. |

## Measurements

All **measured**, 7 play sessions, 16x filtering, metrics on, preload (feature 21) on in every run:

- Mean `Present` interval 1.0–1.5 ms, p99 ≤ 8 ms. Every session has exactly two intervals over 50 ms, both 2.0–3.0 s: the loading
  screens.
- First map load (`map_a`, map 1): 7.5–14.4 s; WLD part 3.4–7.5 s. No run with preload off exists yet, so whether preload pays for itself
  is open.
- At 16x, about 24 % of draws were filtered (sampler 0 only in the first builds). Zero failed draws, `Present` calls, state queries,
  changes or restores.

`Present` intervals include game work, caps and VSync; they are not GPU times. Percentiles are histogram-bin upper bounds.

## How it works

- **Interception.** After the build, PE header and two code anchors match, the `Direct3DCreate9` import (`0x6a5548`) is redirected.
  The factory's `CreateDevice` (slot 16) and the device's `Release`, `Reset`, `Present` (slots 2, 16, 17) and, with filtering,
  `BeginStateBlock`/`EndStateBlock`/the four `Draw*` calls (60, 61, 81–84) are patched **in the native vtable**, keeping its identity
  and the runtime's private entries past the public slots (an earlier per-object vtable copy crashed in `Clear`). Factory (17 slots)
  and device (119) tables are told apart; devices that share an already-patched table but were not created through the hook forward
  untouched. **confirmed**
- **State.** Per-object state lives in a registry keyed by object address with a thread-local, epoch-checked cache, so no lock is
  taken per draw. With caps and metrics off, `Present` and the draws forward without locks, clocks or focus checks.
- **Failure handling.** A failed sampler write is rolled back; a failed restore turns filtering off for that device. `Reset` clears
  frame history and state-block recording; the final `Release` drops the record.
- **Loading timers** (metrics only). The calls at `0x42e87a` and `0x42f836` to the map loader `0x452270` (`__thiscall`, `RET 8`) and
  at `0x452643` to the WLD loader `0x4516f0` (`RET 12`) are pointed at timing wrappers. `map_a`/`map_b` are call-site names, not
  proven "first entry"/"transition"; WLD time is nested inside map time. The timers exclude unloading, later scene/UI setup and
  server waits.

## Checks that were run

- 17 unit tests with a fake COM device: every draw ABI and return value, state restore on failed draws, the exclusions, capability
  clamping, failure rollback, state blocks, `Reset`/`Release` cleanup, the lock-free disabled path and the sampler 1 override.
- A native smoke test on a hidden window with the installed D3D9 HAL: device creation, `Clear`, `Present`, the 30 FPS cap, `Reset`,
  borderless size, log publication and zero leaked references. It never starts the game.
- Not established: any FPS or loading gain, visual quality, long sessions, other drivers.

## Offline audits

- **Textures** (a Python audit of the archive): 9,819 textures (8,986 DDS, 833 TGA, 1.06 GB) all pass their container checks; 10 are
  BMP files named `.dds` (Fortress lightmaps). Only six world DDS lack a full mip chain (`cannon.dds`, `desert_tree02/03.dds`,
  `R1_Trade_L0–L2.DDS`); adding them is a content change needing a visual check. 341 exact-duplicate groups (58 MB stored) are not
  proven to be duplicate GPU loads. A lossless RLE re-encode of `interface/slotmain.tga` would save 3.45 MB (82 %) with identical
  pixels, but saves disk, not time, and was not tried in game.
- **Archive**: the active archive has 19,939 entries in 167 folders, no gaps, overlaps or per-folder hash collisions. The engine
  already looks files up through per-folder hash maps (`0x40d530` reads the SAH, `0x40a490` is the hash, `0x5963a0` the lookup), so
  repacking or a new index will not speed loading (see `data-archive.md`). The texture reader `0x5966d0` opens `data.saf`, reads the
  whole member, creates the texture and closes the file on every call; a read cache or reusable buffer is only worth building if
  instrumentation shows repeated reads. The lookup keeps global "selected entry" state (`0x2232aac` and two siblings), so background
  loading needs a design first. **confirmed (disasm)**

## In other episodes

- The EP1, EP3, EP4.5, EP5.4-labelled and all four EP6 builds import `d3d9.dll`, the same Direct3D 9 library this client uses; no build
  read here imports an older or newer one (**confirmed (file)**, [Episode 1](episodes/ep1.md), [Episode 3](episodes/ep3.md),
  [Episode 4.5](episodes/ep4-5.md), [Episode 5](episodes/ep5.md), [Episode 6](episodes/ep6.md)). Whether the measurements here carry
  over to them is not known.

## Open

- Whether map preloading shortens later stalls enough to pay for its loading time (needs a run with it off).
- How 16x with sampler 1 looks in game: terrain at a low camera angle, foliage, water, character previews.
- Whether the texture reader's per-call open and read dominates loading (time it before any cache work).
