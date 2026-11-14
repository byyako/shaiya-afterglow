# Inside the game.exe engine

A walkthrough of the client engine in the order the code runs it: from `WinMain` to a frame on screen, and the pieces each frame
uses (configuration, logging, the Direct3D device, the archive, text tables, UI windows, input and sound). Everything here was read
statically in the Shaiya US client (`game.exe`, 2010-04-26, patch 108, SHA-256
`5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`). Nothing was run. Addresses are virtual addresses in that file.
"confirmed" means the function was read in a decompile and the behaviour follows from its code; "confirmed (disasm)" means it was
read in the disassembly or the data bytes; *inferred* means a reasonable reading that the code does not state by itself; **open**
means not settled.

## In short

- **One thread, one loop.** `game.exe` is a single-threaded Win32 program. `WinMain` reads the settings, opens the archive, loads the
  text tables, checks that it is the only copy running and that the updater started it, creates one window, connects to the login
  server, and only then creates the Direct3D 9 device and the sound system.
- **The loop never sleeps.** It empties the Windows message queue, measures the time since the last frame and calls one frame function.
  There is no frame cap and no VSync (no waiting for the monitor), which is why the game runs at hundreds of frames per second. The frame function is a small state
  machine: intro, login, faction choice, character select, character creation, world loading, in the world, map change, and two rarer
  scenes.
- **Settings come from three text files.** `CONFIG.INI` next to the exe (video, sound, user options, key bindings, window positions),
  an optional `gsConfig.cfg` (logging) and an optional `data/Config.cfg`. `Version.ini` is not read by `game.exe`.
- **Files come out of the archive first.** Every file the game opens under `data/` is looked up in `data.sah` and read out of
  `data.saf`; only if it is missing there does the game try the loose file on disk. The `.SData` tables are decrypted with a 16-byte
  block cipher; one of them (`Item.SData`) skips its checksum check. See [the archive](data-archive.md).
- **Text is looked up by number.** Messages come from `data/sysmsg-uni.txt` (UTF-16, `id "text"` per line), with markers such as `<p>` and
  `<v>` filled in from fixed buffers. The UI is about 60 window classes sharing one small base class, drawn in a fixed order every frame.
- **Input and sound use DirectX.** Mouse and keyboard come from DirectInput 8, typing from ordinary window messages (with IME support),
  and some keys from `GetAsyncKeyState`. Sound effects use DirectSound 8 with 3D positions; music is streamed through the Audiere
  library.

### Using this page

- **If you write a server:** you need little from this page. The network side is in [How the client talks to the servers](re-network.md);
  section 7 here explains how the client turns a message number into text, and what it prints for a number that does not exist.
- **If you patch or mod the client:** section 3 lists the settings files and keys, section 6 how files are found, section 7 how
  messages are looked up, and section 12 how to turn on the log. [Patch kit for game.exe](client-patch-kit.md) and
  [The companion DLL](fixes-dll.md) apply changes.
- **To check a claim yourself:** every section gives the address to open; [Tools and method](tools-and-method.md) has the setup.

Related pages: [re-network.md](re-network.md) (the network layer, which runs inside this loop),
[data-archive.md](data-archive.md) (the archive format and SData wrapper), [formats.md](formats.md) (model, map and effect files),
[graphics.md](graphics.md) (frame and loading measurements, renderer hooks), [client-packets.md](client-packets.md) (the packet log
and `gsConfig.cfg`), [client-actions.md](client-actions.md) (the region number and the region feature table).

## 1. The pieces and where they live

| What | Address | Notes |
|---|---|---|
| `WinMain` | `0x40b1a0` | start-up, main loop, shut-down (confirmed) |
| `GameInit` | `0x40a6f0` | network, input, display, device, sound (confirmed) |
| Window procedure | `0x409640` | window class `GAME`, title `Shaiya` (confirmed) |
| Frame function | `0x42cc10` | one call per loop pass; `switch` on the scene state byte `0x705040` (confirmed) |
| Timer | `0x405850` | `QueryPerformanceCounter`, falls back to `timeGetTime` (confirmed) |
| `CONFIG.INI` readers | `0x408750` (calls `0x4076a0`, `0x407ea0`, `0x4080f0`, `0x408630`) | confirmed |
| Logger write | `0x5a4070` | logger object `0x2232dd8`, mask at `0x2232e14` (confirmed) |
| Renderer object | `0x21efe48` | IDirect3D9 at `+0x404`, device at `+0x408` (copy in `0x21efe40`) (confirmed) |
| Device creation / reset | `0x5315c0` / `0x530f90` | confirmed |
| Begin frame / end frame | `0x52ebf0` / `0x52fe20` | Clear + BeginScene / EndScene + Present (confirmed) |
| Archive index load | `0x40d530` (tree `0x40d340`) | root node `0x701f34` (confirmed) |
| Archive file open | `0x40c550`, `0x5964e0` | archive first, then disk (confirmed) |
| Text table load / lookup | `0x41df80` / `0x41c180` | `data/sysmsg-uni.txt`; table root `0x704294` (confirmed) |
| UI window base vtable | `0x6a69b0` | 9 slots; 61 derived vtables (confirmed (disasm)) |
| UI manager: create / input / draw | `0x424100` / `0x48c120` / `0x48aa00` | confirmed |
| Input step | `0x51e720` | DirectInput polls, mouse buttons and cursor (confirmed) |
| Sound init / effect / music | `0x526f80` / `0x527830` / `0x526d90` | sound manager object `0x21edc20` (confirmed) |

Global flags used throughout this page (each confirmed from the functions named in its section):

| Address | Meaning |
|---|---|
| `0x6f1558` | region number (1 Korea … 6 English … 17 Italy), from `CONFIG.INI [LOGIN] VERSION` |
| `0x701afc` | the game window handle |
| `0x701b04` | full screen requested (`[VIDEO] FULLSCREEN`) |
| `0x701b08` | the game window is the active application (`WM_ACTIVATEAPP`) |
| `0x701b0c` | the Direct3D device is lost |
| `0x701b14` | seconds since the previous frame (frame time) |
| `0x701b20` | "reset the frame clock" request (after moving or sizing the window) |
| `0x6f155c` / `0x6f1560` | configured width and height (`SIZE_X`, `SIZE_Y`) |
| `0x6f1564` / `0x6f1568` | client area width and height (`WM_SIZE`) |
| `0x6f156c` / `0x6f1570` | the configured size again, copied once in `WinMain`; the cursor is clamped to it |
| `0x705040` | scene state (section 5) |

## 2. Start-up order (`WinMain`, `0x40b1a0`)

In order, as the function runs (confirmed unless marked):

1. `gsCommonSDM::Init(1)` (`0x5a9490`): loads `gsConfig.cfg` if present and sets up the logger
   ([client-packets.md](client-packets.md), "Built-in packet logging").
2. Region-specific steps: region 5 splits the command line into four words (fewer → exit); region 1 compares the desktop folder's
   name with a Korean word and may refuse with "Korea Version"; region 4 deletes a list of loose files (`Data\Item\Item.SData`,
   `Data\Npc\NpcQuest.SData`, `Server\…`, `Server.zip`) and their folders. `CONFIG.INI` is only read in step 9, so these steps test the
   built-in region value (6 in this exe) and never run here.
3. `0x40d530`: opens `data.sah` and builds the archive tree (section 6).
4. `0x40aee0`: reads `data/filter.txt` (the chat word filter, section 7).
5. The chat object (`0x41e190`, stored at `0x7013d8`): its constructor loads `data/sysmsg-uni.txt` (`0x41df80`) and prepares two pools
   of 100 chat lines.
6. Single instance: `CreateMutexA` (name `Global\Shaiya` on newer Windows, `Shaiya` otherwise; the switch is a Windows-version value
   at `0x22330ec`, *inferred*: the C runtime's Windows major version). Already exists → message 10054 in an "ERROR!!!" box. Then `FindWindowA("GAME", "Shaiya")`: found →
   the same message, "ERROR!".
7. Launch argument: `new updater` → wait for `new_updater.exe`, close the running updater, replace `Updater.exe`, start it, exit
   (failures show message 10056 with `e01`…`e04`). Anything other than `start game` → start `Updater.exe` and exit.
8. `ijl15.dll` must be exactly `0x5b000` bytes (`0x409600` returns the file size); otherwise message 10038 ("game hack has been
   found") in a box titled "nProtect GameGuard".
9. `0x408750`: reads `CONFIG.INI` (section 3).
10. `0x4094c0`: physical memory. Less than 150 MiB → message 10038 again and exit. Otherwise it stores a memory tier at `0x701990`
    (0 below 400 MiB, 1 below 800 MiB, 2 up to 1.5 GiB, 3 above, 4 above 1.5 GiB with at least 700 MiB free). The tier limits how
    many copies of a sound are kept (section 11).
11. `0x409550`: notes whether Windows is NT-based (`0x701994`). Region 2 also records the install folder in the registry.
12. The window (section 4), seeded random numbers (`0x5b73f4(timeGetTime())`, confirmed (disasm): it stores the seed in the per-thread C-runtime block, which is `srand`), then `GameInit` (`0x40a6f0`).
13. `ShowWindow`, `UpdateWindow`, `SetFocus`, the timer is started (`0x405850(1)`), the cursor is hidden, the main loop runs (section 5).
14. Shut-down: `0x409e40` (logs `GameExit`, frees the chat object and the renderer helpers, disconnects, deletes the region-feature
    and network objects), `UnregisterClassA("GAME")`, frees the archive tree (`0x40d770`), `0x5a9350` (confirmed: logs `gsCommonSDM::Shutdown` and shuts the logger down),
    then `WinExec("Ending.exe")`.

`GameInit` (`0x40a6f0`), in order (confirmed):

| Step | Call | On failure |
|---|---|---|
| Log `****************GameInit****************` at level 4 | `0x5a4070` | — |
| Read `data/Config.cfg` (archive first, then disk) and parse it with the same parser as `gsConfig.cfg` | `0x40c550`, `0x5a8cd0` | skipped if missing |
| Cipher state off, message window and `WSAStartup` | `0x401160`, `0x5854b0` | message 10058 |
| Region feature object (`0x7013d4`) | `0x405cd0` | — |
| Network object, connect to the login server on port 30800, wait up to 30 s for the key packet | `0x40a4f0`, `0x578b10` | 10059 (connect), 10061 (timeout) |
| DirectX version check (must be 9 or later) | `0x52e480` → `0x534c20` | 10062 |
| DirectInput mouse and keyboard | `0x51e6c0` | 10063 |
| Full screen only: DirectDraw display mode | `0x52e490` | 10064 |
| Direct3D device; if both attempts fail, again with 16-bit colour | `0x5315c0` | 10064 |
| Sound, then two preloaded UI sounds (`ui_button001.wav`, `ui_window.wav`) | `0x526f80`, `0x528970` | — |

So the client is already talking to the login server before it has a Direct3D device. Network details: [re-network.md](re-network.md).

**How to find it.** `WinMain` is the function that references the strings `start game`, `GAME`/`Shaiya` and `Ending.exe`; `GameInit`
is the one that logs `GameInit`. The message numbers are `sysmsg-uni.txt` ids passed to `0x41c180` (section 7).

## 3. Configuration

### `CONFIG.INI`

The path is the exe's own folder plus `CONFIG.INI` (`GetModuleFileNameA`, cut at the last `\`, kept at `0x7013e0`). Every value is read
with `GetPrivateProfileStringA`. Booleans are "anything but `FALSE`" (`TRUE` is not checked). Defaults below apply when the key is
missing (all confirmed in the readers named).

| Section / key | Global | Default | Meaning |
|---|---|---|---|
| `[LOGIN] VERSION` | `0x6f1558` | 6 | region by name: `KOREA` 1, `CHINA` 2, `VIETNAM` 3, `JAPAN` 4, `TAIWAN` 5, `ENGLISH` 6, `GERMANY` 7, `SINGA_MALA` 8, `HONGKONG` 9, `FRANCE` 10, `RUSSIA` 11, `TURKEY` 12, `SPAIN` 13, `BRAZIL` 14, `LATIN_AMERICA` 15, `POLAND` 16, `ITALY` 17 (`0x408750`) |
| `[LOGIN] TEST_IP`, `INTERNAL_TEST_IP` | `0x701970` | — | pick another built-in login address by region name (`0x408750`) |
| `[TUTORIAL] MOUSE MOVE SKILL QUEST LEVELUP ITEM` | `0x701910`–`0x701924` | on | tutorial hints (`0x4076a0`) |
| `[VIDEO] FULLSCREEN` | `0x701b04` | **on** | full screen (section 4) |
| `[VIDEO] SIZE_X`, `SIZE_Y` | `0x6f155c`, `0x6f1560` | — | resolution; also picks the layout index `0x70199c` (below) |
| `[VIDEO] COLOR` | `0x70193c` | 32-bit | `16` = 16-bit colour and a 16-bit depth buffer |
| `[VIDEO] GLOW_LEVEL`, `GAMMA` | `0x7019a0`, `0x7019a8` | 2, 2 | numbers |
| `[VIDEO] RANGE` | `0x7019a4` + distances `0x701934`–`0x701960` | `MIDDLE` | `HIGH`/`MIDDLE`/`LOW`; sets the far distance (400/300/200) and the fog limits (section 9) |
| `[VIDEO] TEXTURE` | `0x7019ac`, `0x70196c` | `HIGH` | `HIGH` 2/−1, `MIDDLE` 1/128, `LOW` 0/64; `0x70196c` is a texture size limit: `0x4444d0` (at `0x444a1f`) hands it to `0x599dc0`, which stores it in `0x6f4c8c`, and `0x52abd0` (at `0x52abe7`) maps 128 to 256 and 64 to 128 (−1 stays) before creating a texture surface; the settings window `0x4ed990` writes it too (confirmed) |
| `[VIDEO] SHADOW WATER CLOAK HELMET WEAPONEFF SKILLWNDLOCK` | `0x701930`, `0x70192c`, `0x7019b0`–`0x7019bc` | on, off, on, on, on, off | display options |
| `[SOUND] VOL_BGM VOL_EFFECT VOL_WORLD` | `0x7019c0`–`0x7019c8` | 6 | volumes (`0x407ea0`) |
| `[SOUND] VOICE_CHAR VOICE_NPC DANGER_DETECT` | `0x7019cc`–`0x7019d4` | on | voices, danger warning sound |
| `[USER] USE_MOUSE INV_MOUSE INV_MOUSE_UPDOWN MOUSE_SENSITIVITY` | `0x7019d8`–`0x7019e4` | on, off, off, 3 | `INV_MOUSE` swaps the buttons (section 10) (`0x4080f0`) |
| `[USER] MY_NAME USER_NAME MONSTER_NAME USE_FILTER` | `0x7019e8`–`0x7019f4` | on | name labels, chat filter (`USE_FILTER` is forced on after reading) |
| `[USER] SHOW_INTERFACE` | `0x701984` | on | draw the UI at all (section 8) |
| `[USER] REJECT_FIGHT REJECT_PARTY WARNING_MSG REJECT_TRADE UI_REOPEN SCREEN_SWING TALK_BALLOON HIDE_DAMAGE` | `0x701a00`–`0x701a1c` | off, off, on, off, on, on, on, off | user options |
| `[OPTION] KEY0`–`KEY19`, `ADD_KEY0`–`ADD_KEY9` | `0x701a50`, `0x701a28` | tables `0x6f2bc0`, `0x6f2cb0` | key bindings, decimal (`0x408630`) |
| `[INTERFACE_<W>X<H>] <WINDOW>_POS_X/Y` | per window | constructor values | window positions, read and written by the UI base class (section 8) |

Resolution → layout index `0x70199c` (`0x4076a0`): 800×600 → 0, 1024×768 → 1, 1280×1024 → 2, 1360×768 → 3, 1920×1200 → 4,
1680×1050 → 5, 1280×720 → 6, 1600×900 → 7, **any other size → 1**. The index chooses the `[INTERFACE_…]` section and the aspect value
`0x6f1578` (1.25 for indexes 0–2, width/height otherwise). So `SIZE_X=1920, SIZE_Y=1080` renders at 1920×1080 but takes the 1024×768
UI layout section and an aspect of 1.25 (confirmed for the values; what that looks like on screen is **live**).

A typical `CONFIG.INI` also has an `[INTERFACE]` section and `[LOGIN] ID`; their readers were not traced.

### `gsConfig.cfg` and `data/Config.cfg`

`gsConfig.cfg` (loose file) holds the logger settings; its format, keys and the `LogLevel` mask are described in
[client-packets.md](client-packets.md). `GameInit` also opens `data/Config.cfg` (archive first, then disk; the US client ships no such file) and feeds it to the same parser
(`0x5a8cd0`, confirmed). That parser is a plain INI reader (`[section]` lines, `key=value`, lines starting `#`, `@` or `;` skipped) with no key names of its own; `GameInit` first empties the shared settings store at `0x22331cc` (`0x5a8c10`) and then fills it, so a present `Config.cfg` replaces what `gsConfig.cfg` put there (*inferred*: which file `0x5a9310` loads, and into which object, was not shown). The keys the exe asks that store for (confirmed, strings in the readers): `[Config] FontName` (`0x5315c0`, the UI font name for two language branches; defaults `simsun` and `arial`), `[gsCommon] LogDefaultListener` and `LogLevel` (`0x5a5980`), and `[gsCommon] MiniDumperReport_MailServer`, `_ip`, `_dir`, `_id`, `_pw`, `_MailMessage`, `_MailSubject`, `_MailAddress` (`0x5aabb0`, crash-report mailing). `0x5a9490` also uses the store (five sites) and was not decompiled, so whether it reads more keys is **open**. The separate `data/WorldMap.cfg` (read by `0x4bc1a0` with the same parser) holds `[WorldMap] count` and per-entry `Country`, `MapNum`, `Click_left`, `Click_top`, `Click_right`, `Click_bottom`, `ImgPos_x`, `ImgPos_y`.

### `Version.ini`

No string `version.ini` (any case) exists in `game.exe` (confirmed by a byte search), so the patch level file is only the updater's
business.

**How to find it.** Search the imports for `GetPrivateProfileStringA`; its callers are the five readers above plus the UI base class.
The section names (`VIDEO`, `SOUND`, …) are plain strings next to the key names.

## 4. The window and the Direct3D 9 device

### The window

`0x409f50` → `0x409d80` registers class `GAME` (style `CS_OWNDC`, window procedure `0x409640`) and creates the window `Shaiya`
(region 3 uses a Unicode copy, `0x40bf70`). Windowed: style `0x10CF0000` (`WS_OVERLAPPEDWINDOW | WS_VISIBLE`), centred, outer size
= the configured size plus the frame and caption. Full screen: style `0x90000000` (`WS_POPUP | WS_VISIBLE`) at 0,0 with the configured
size (confirmed, `WinMain`).

The window procedure (`0x409640`, confirmed) first passes every message to the text-input handler `0x522bf0` (typing and IME,
section 10); a return of 0 means "handled". Then:

| Message | What happens |
|---|---|
| `WM_ACTIVATEAPP` | stores `wParam` in `0x701b08`. When the window becomes active in the world, and the 60 s countdown `0x6f1580` has run out, it sends `0x2605` (point refresh, `0x5897c0`) and restarts the countdown |
| `WM_CREATE` | hides the cursor |
| `WM_DESTROY` | sets the network object's quit flag (`+4`) and posts quit; the frame function then returns 0 |
| `WM_SIZE` | client width/height to `0x6f1564`/`0x6f1568` |
| `WM_CLOSE` | `DestroyWindow` |
| `WM_SYSCOMMAND` | `0xF093` (the menu opened by the window icon) is swallowed; drag-move (`0xF012`), minimise, maximise, restore and `0xF050` set `0x701b20` |
| `WM_EXITSIZEMOVE` | sets `0x701b20` |

`0x701b20` makes the main loop restart the frame clock, so dragging the window does not produce one huge frame time.

### Display mode (full screen)

With `FULLSCREEN` on, `GameInit` calls `0x52e490` before the device: outside region 1 it creates a DirectDraw object, sets the
normal cooperative level on the desktop window and **changes the desktop display mode** to the configured size and colour depth
(`SetDisplayMode`, confirmed). The device itself is then created windowed (below). In region 1 this step is skipped and the device
is a real exclusive full-screen device.

### Device creation (`0x5315c0`)

`Direct3DCreate9(32)` (import `0x6a5548`), present parameters at renderer `+0x364`, then
`CreateDevice(adapter 0, HAL, window, 0x44, …)`; if that fails, again with `0x24` (confirmed).

| Offset (renderer) | Present parameter | Value |
|---|---|---|
| `+0x364`, `+0x368` | BackBufferWidth, Height | 0 (window size); full screen in region 1: the configured size |
| `+0x36c` | BackBufferFormat | 0 (current); full screen in region 1: `X8R8G8B8`, or from `0x52e730` at 16-bit |
| `+0x370` | BackBufferCount | 0 at creation, 1 after a reset |
| `+0x37c` | SwapEffect | 1 (`DISCARD`) |
| `+0x380` | hDeviceWindow | the game window |
| `+0x384` | Windowed | 1 unless `FULLSCREEN` **and** region 1 (confirmed (disasm), `0x531656`–`0x531666`) |
| `+0x388`, `+0x38c` | EnableAutoDepthStencil, format | 1; `D24S8` at 32-bit colour, `D16` at 16-bit |
| `+0x390` | Flags | 1 (`LOCKABLE_BACKBUFFER`) |
| `+0x398` | PresentationInterval | `0x80000000` (`IMMEDIATE`): no VSync |

Behaviour flags: `0x44` = hardware vertex processing + multithreaded, `0x24` = software vertex processing + multithreaded. After the
device: fonts (region 6: a GDI Arial 15 and two D3DX fonts of 13 and 12 created through `0x538c50`, stored at `+0x3a8` and
`+0x39c`), render helpers, and the adapter description logged at level 8 (`Graphic Adapter Driver : …`, via
`IDirect3D9::GetAdapterIdentifier`).

### Device loss

`0x52fe20` (end of frame) calls `EndScene` and `Present`. If `Present` returns `D3DERR_DEVICELOST`, it releases the font resources
(slot `+0x40` of each font, *inferred* `OnLostDevice`) and sets `0x701b0c`. While the window is active and the device is lost, the main
loop does not run frames; it asks `TestCooperativeLevel` each pass and, on `D3DERR_DEVICENOTRESET`, calls `0x530f90`, which rebuilds the
present parameters, calls `Reset`, re-creates the render helpers and fonts and clears the flag. A failed reset ends the game
(confirmed, `0x40bee6`–`0x40bf08`).

`0x52fe20` can also draw an FPS counter and the player position (`Player x:… y:… z:…, Map:%d`) when renderer `+0x410` is set. The only write to that field in the whole file is the renderer's constructor (`0x52f610`, sets 0), so nothing in this build turns the overlay on (confirmed, reference list of the offset `0x410`).

**How to find it.** The only caller of the `Direct3DCreate9` thunk (`0x693df2`) is `0x5315c0`. Device methods are called through the
device's vtable; the slot number gives the method (`+0x44` Present, `+0xa4` BeginScene, `+0xa8` EndScene, `+0xac` Clear, `+0xe4`
SetRenderState, `+0x104` SetTexture, `+0x10c` SetTextureStageState, `+0x114` SetSamplerState).

## 5. The main loop and frame timing

The loop at the end of `WinMain` (`0x40be24`–`0x40bf31`, confirmed (disasm)), with names added:

```c
// WinMain 0x40b1a0, main loop (0x40be24-0x40bf31)
for (;;) {
    while (pump_one_message())                 // 0x4095b0: PeekMessage(PM_REMOVE), Translate, Dispatch
        ;
    total_time = timer(5);                     // seconds since start   -> 0x701b10
    if (reset_clock) { timer(1); reset_clock = 0; }   // 0x701b20, set by the window procedure
    frame_dt = timer(6);                       // seconds since last call -> 0x701b14
    if (frame_dt <= 0) frame_dt = 0.05f;
    if (region is 2, 5 or 9 && frame_dt > 1.0f) frame_dt = 0.1f;
    point_refresh_countdown -= frame_dt;       // 0x6f1580, see WM_ACTIVATEAPP
    if (app_active && device_lost) {           // 0x701b08, 0x701b0c
        if (device->TestCooperativeLevel() == D3DERR_DEVICENOTRESET) {
            if (!reset_device()) break;        // 0x530f90
            device_lost = 0;
        }
        continue;                              // no frame while the device is lost
    }
    if (!idle_hook()) break;                   // 0x407670: always 1
    if (skip_frame == 0 && !frame()) break;    // 0x701b28 (no writer found); frame = 0x42cc10
}
```

- **No sleep and no cap.** Nothing in the loop waits; the message pump uses `PeekMessage`, not `GetMessage`, and the device presents
  immediately (section 4). The frame rate is limited only by the GPU and CPU. A background cap is something only an add-on can give
  ([graphics.md](graphics.md)).
- **Inactive window.** The frame function still runs (game logic, network, sound), but every scene checks `0x701b08` before
  `BeginScene`, so nothing is drawn or presented while another application is active (confirmed in `0x42cc10`). The anti-freeze patch
  that forces `0x701b08` to 1 therefore makes the game draw in the background too.
- **Timer** (`0x405850`, confirmed): modes 0 reset, 1 resume, 2 pause, 3 step 0.1 s, 4 absolute time, 5 time since start, 6 time
  since the previous mode-6 call. A mode-5 or mode-6 result below about 1.2e-7 s is replaced by 0.01 s. In region 6 there is **no
  upper clamp**: a long stall gives the next frame its full length.

### The scene states (`0x42cc10`)

Before the `switch`, the frame function returns 0 (ending the loop) if the network object is gone or its quit flag is set, sends the
keep-alive every 120 s ([re-network.md](re-network.md)), and runs two per-frame updates (`0x429d80`, `0x539f70`). The state byte is
`0x705040`; the jump table is at `0x430154` (confirmed (disasm)).

| State | Case starts at | What it is | Update / draw | Next |
|---|---|---|---|---|
| 0 | `0x42cccc` | intro: `sonovlogo.bmp`, `game_grade.tga`; loads `Weather.EFT`; message 10030 on failure | — | 1 |
| 1 | `0x42d4d0` | login screen (`bgm_title.wav`) | `0x4b61b0` / `0x4b3470` | 3 |
| 2 | `0x42d793` | faction choice (`select_country*.tga`; object `0x47b0a0` at `0x705048`) | `0x47bb10` / `0x47ada0` | 3, 1 |
| 3 | `0x42dc3c` | character list; waits up to 30 s for the server (message 10031); goes to 2 when the account has no faction yet (`0x21e4942` = 2) | — | 2, 4, 5 |
| 4 | `0x42e44b` | character creation | `0x464ab0` / `0x4640e0` | 3 |
| 5 | `0x42e632` | first world load: world object (`0x42a210`, `0x6608` bytes, `0x705054`), `BGM_load0001.wav`, loading screen `0x4b13f0` (*inferred*), map loader `0x452270` (called at `0x42e87a`) | — | 6 |
| 6 | `0x42eb4c` | in the world | `0x4216f0` / `0x4354f0` | 7, 3 |
| 7 | `0x42f654` | map change: the same loading path, map loader called at `0x42f836`; entered from `0x46a4e0` and state 9 | — | 6 |
| 8 | `0x42fbde` | loads `strip.wld`, clears the local player; entered when the UI window at manager `+0x260` returns 4 (`0x48f5e9`); that window is the gambling table selector (confirmed, below) | — | 9 |
| 9 | `0x430022` | the gambling scene: a scene object `0x488330` (`0x2eec` bytes) with `BGM_dung02.wav` | `0x4899c0` / `0x487860` | 7 |

The names of states 0–7 are *inferred* from their resources and transitions; states 8 and 9 are the gambling house. The window at manager `+0x260` is built by `0x486bf0` (vtable `0x6aae8c`, set-up `0x485bc0`: `gamble.TGA`, `gamble_button.tga`, `cancel_button.tga`); its input slot `0x486140` sends the chosen table (`0x588c00`), waits for the answer flags set by the handlers `0x554fb0`–`0x555130` and returns 4 once the answer says yes, which makes `0x48f5e9` set state 8 (confirmed; the wire side is on [research-gambling-kill-boost.md](research-gambling-kill-boost.md)). This settles
the two map-loader call sites that [graphics.md](graphics.md) calls `map_a` and `map_b`: `0x42e87a` is the first world entry (state 5),
`0x42f836` the map change (state 7) (confirmed (disasm), jump table ranges).

Each scene ends the same way: if the window is active and `0x52ebf0` (`Clear` + `BeginScene`) succeeds, draw, then `0x52fe20`
(`EndScene` + `Present`). Scene changes run a nested loop that keeps drawing until a fade object (vtable `0x6a80fc`, driven by
`0x41f980`/`0x41fa00`) has finished (confirmed structure; "fade" *inferred*).

**How to find it.** `WinMain`'s only call into the game after start-up is `0x42cc10` (at `0x40bf2a`). The `switch` is the jump at
`0x42ccc5`. `timeGetTime` and `QueryPerformanceCounter` are imported once each; the timer is their common caller.

## 6. From a file name to bytes

### The archive tree

`0x40d530` opens `data.sah` with `_open`, accepts the `SAH` header (64-bit offsets) or the old headerless form (32-bit offsets, flag
`0x701f30`), and reads the tree recursively with `0x40d340` ([data-archive.md](data-archive.md) has the file layout). In memory
(confirmed):

| Node | Offset | Type | Meaning |
|---|---|---|---|
| file entry (`0x30` bytes) | `+0x04` | std::string | name |
| | `+0x1c` | u32 | name hash (djb2 of the lower-cased name, `0x40a490`) |
| | `+0x20` | i64 | offset in `data.saf` (32 bits in the old form) |
| | `+0x28` | u32 | length |
| directory (`0x38` bytes) | `+0x1c` | u32 | name hash |
| | `+0x20` | std::map | files by hash |
| | `+0x2c` | std::map | sub-directories by hash |

The per-file version stamp is read and dropped. The root (`0x701f34`) is handed to three file helpers (`0x589e40`, `0x596390`,
`0x58c7b0`), which keep their own copies (one is `0x2232aa8`).

### Opening a file

`0x40c550` and `0x5964e0` do the same thing (both report errors as `safopen` in `SAFile.cpp`; a third helper, `0x58c9d0`, uses the
same strings and was not read). The excerpt is `0x5964e0` with names added:

```c
// safopen 0x5964e0 (same logic in 0x40c550)
int safopen(const char *path, const char *mode)
{
    if (mode is "wt" or "a+t")                      // writing: always the real disk
        return _open(path, ...) or report("File Not Open [%s]");
    if (mode is not "rb" or "rt") return report(...);

    int fd = _open("data.saf", binary_or_text);     // a new handle for every file
    if (fd != -1 && archive_find(&fd, path))        // 0x5963a0: walks the tree, seeks fd
        return fd;                                  // the caller reads from data.saf
    _close(fd);
    return _open(path, binary_or_text) or report(...);  // fall back to the loose file
}
```

`0x5963a0` only accepts paths that start with `data/` or `data\` (case-insensitive), strips that prefix, hashes each folder name and
looks it up in the current directory's map, then hashes the file name and seeks the handle to the entry's offset
(`_lseeki64`, full 64 bits). It also stores the entry in a global (`0x2232aac`; the `0x40c550` copy uses `0x701f38`) so the caller
can read its length (confirmed). Consequences:

- **The archive wins.** A loose file with the same path as an archived one is never read. A loose file is used only when the archive
  has no entry for that path.
- Every open creates a new `data.saf` handle; the caller reads and closes it. Nothing is cached ([graphics.md](graphics.md),
  "Offline audits", reached the same conclusion for textures).
- The "selected entry" global makes the helpers unsafe to call from two threads at once (*inferred*; the client never does).

Readers built on top: textures (`0x5966d0` for the 3D scene, `0x40c7c0` for UI textures) read the whole member into a buffer and pass it
to an in-memory texture loader (`0x5cd4ab`, confirmed (disasm): `D3DXCreateTextureFromFileInMemoryEx` compiled into the exe; a wrapper of 15 stack arguments that passes resource type 3 (a plain 2D texture) to the shared D3DX loader `0x5ccc36`), then free the
buffer; text files are read line by line from the same handle (`0x437350` ANSI, `0x4373e0` UTF-16).

### The SData tables

Loaders (confirmed): `0x460500` `Item/item.SData`, `0x4612f0` `Character/Skill.SData` and `Monster/NpcSkill.SData` (called from
`0x429ee0`, the world-entry resource load); `0x460e20` `Monster/Monster.SData`, `0x4796c0` `NPC/NpcQuest.SData`, `0x461bf0`
`Cash.SData`, `0x478320` `Character/KillStatus.SData` (called from `0x4248e0`, the game-data load). The Item loader, read in full:

1. opens the file through `0x40c550` (archive first) and reads a 64-byte header: 40 signature bytes, compared with the ASCII signature
   at `0x6a9ae8`; a u32 checksum; a u32 plaintext size; padding;
2. reads the rest of the archive entry (its length minus 64) and decrypts it in place with `0x5a9a50`: 16-byte blocks, each through
   the block function `0x5aff00` with a fixed round-key table at `0x6f5140` (ECB, no chaining; the cipher is SEED,
   [data-archive.md](data-archive.md));
3. computes a CRC-32 over the plaintext size (`0x5a99a0`, the usual reflected table at `0x6f4d40`).

The CRC function has six callers, one in each loader above. Five of them compare the result and branch away on a mismatch (confirmed
(disasm): Skill `0x46149f`, Monster `0x460fe1`, Cash `0x461d9c`, NpcQuest `0x479879`, KillStatus `0x478539`); **the Item loader
(`0x4606b6`) computes the CRC and ignores it**. So an `Item.SData` with a stale checksum still loads; the others would not. What the client then reads from each
record is the subject of the gameplay-state page.

**How to find it.** The strings `data.sah`, `data.saf`, `SAFile.cpp` and `File Not Open [%s]` lead to every function here. The SData
loaders are the callers of `0x5a9a50`; each one pushes its folder and file name.

## 7. Text: system messages and other tables

### `sysmsg-uni.txt`

Loaded once at start-up (`0x41df80`, confirmed): `data/sysmsg-uni.txt` is opened through `0x40c550` (`rb`), the two-byte byte-order mark
is skipped, and the file is read as UTF-16 lines of up to 2048 characters. Each line is converted to the region's ANSI code page
(`0x409b10`; region 3 uses UTF-8) and then:

- a line starting `//` is a comment;
- `sscanf("%d")` takes the id; a line without a number is skipped;
- the text is the part in double quotes (`0x46d5e0`, the same quoted-argument reader the chat commands use);
- the text is copied to the heap and stored in a `std::map<int, char *>` (root `0x704294`, insert `0x41ded0`). A repeated id
  overwrites the earlier one.

A line therefore looks like `12<TAB><TAB>"text"`.

### The lookup (`0x41c180`)

`GetMsg(id)` finds the id in the map and expands markers into one of **five rotating 1 KiB result buffers** (`0x702e88`, index
`0x70428c`), so a returned pointer stays valid only until five more lookups (confirmed). A missing id gives `GetMsg Error <id>`; a result
that would overflow gives `GetMsg Error <id> buf1/buf2/s:<n>`.

| Marker | Filled from |
|---|---|
| `<p>` | player name buffer `0x702a80` (256 bytes) |
| `<t>` | target name buffer `0x702278` |
| `<s>` | `0x702880` |
| `<i>` | item name buffer `0x702c88` |
| `<r>` | `0x702078` |
| `<b>` | `0x702478` (512 bytes; guild name or number) |
| `<v>`, `<v2>` | integers `0x704288`, `0x702070` |
| `<hp>`, `<up>`, `<zz>` | names produced by `0x563e00`/`0x563e40` from the bytes `0x702c80`, `0x702878`, `0x702879` (*inferred*: stat and item-type names) |
| four Korean markers (은/는, 이/가, 을/를, 으로) | `0x41b900`, `0x41b9b0`, `0x41ba20`, `0x41ba90`: pick the right Korean particle for the word before (*inferred* from the strings) |

A handler that shows a message first copies names into these buffers, then calls `0x41c180`. Marker names are compared without case
(`_stricmp`). The text is then printed by a chat or notice function (for example `0x41e8f0(type, id)`, *inferred* from its arguments;
[scripted-monsters.md](scripted-monsters.md) shows another in use).

### Other text files

- `data/filter.txt` (`0x40aee0`, read as text): sections start with `<` lines (a Korean "forbidden word" tag or `<ban`, and a Korean
  "filtering" tag or `<filter`, plus a "character" tag or `<char`); words of the first kind are stored alone, the filtering section
  holds `word replacement` pairs (confirmed structure; tag spellings from the strings).
- Korean literals compiled into the exe (GM tools, the item mall points label, some window labels) are not in any table; they can only
  be changed in the exe ([gm-and-commands.md](gm-and-commands.md)).

**How to find it.** The string `data/sysmsg-uni.txt` has one reference (`0x41dfca`); `GetMsg Error %d` has two, both in `0x41c180`.
`0x41c180` has hundreds of callers, which is the quickest way to tie a message id to the code that shows it.

## 8. UI windows

### The window class

All game windows derive from one base class (constructor `0x421750`, destructor `0x421760`, vtable `0x6a69b0`; about 60 constructors
store the base vtable before their own). A byte search for the base's last four slots finds **61 derived vtables** (confirmed (disasm)).
Every vtable has 9 slots:

| Slot | Offset | Base implementation | Meaning (status window as the example: vtable `0x6a69ec`) |
|---|---|---|---|
| 0 | `+0x00` | `0x421760` | destructor (`0x421d30`) |
| 1 | `+0x04` | stub `0x5b804d` (confirmed (disasm) `_purecall`: calls the runtime's handler if set, then exits with runtime error 25) | input: mouse handling, returns whether the click was taken (`0x4f3990`) |
| 2 | `+0x08` | stub `0x5b804d` | draw (`0x4f3f80`) |
| 3 | `+0x0c` | empty | load position: calls slot 7 with its two key names (`0x4f3f50`: `SHEET_STATUS_POS_X/Y`) |
| 4 | `+0x10` | empty | save position: calls slot 8 (`0x4f3f70`) |
| 5, 6 | `+0x14`, `+0x18` | empty (`0x407660`, `0x5a8700`) | optional hooks; slot 5 is called with 0 or 1 by the UI manager, but only the chat window overrides it (`0x423f40`: stores the byte at `+0x3baf`; confirmed, see the Open table) |
| 7 | `+0x1c` | `0x513930` | read X and Y from `CONFIG.INI [INTERFACE_<W>X<H>]`, falling back to `[INTERFACE_1024X768]`; clamp to the screen (`0x5138e0`) when asked |
| 8 | `+0x20` | `0x513cf0` | write X and Y to the same section (`WritePrivateProfileStringA`) |

Base fields (confirmed in `0x513930`, the status window constructor `0x421bb0` and its slots): `+0x04` x, `+0x08` y, `+0x0c` width,
`+0x10` height, `+0x20` visible (slots 1 and 2 return at once when it is 0). Buttons and other controls are embedded objects of
`0x4d4` bytes (constructor `0x5140c0`, set-up `0x515860` with a texture name and UV rectangle, hit test `0x514500`).

```c
// Status window constructor 0x421bb0 (abridged)
StatusWindow *StatusWindow_ctor(StatusWindow *w)
{
    base_fields_zero(w);                 // +0x04 .. +0x2c
    w->vtable = 0x6a69ec;
    construct_controls(w + 0x60, 0x4d4, 4);   // 4 buttons
    construct_controls(w + 0x13b0, 0x4d4, 12); // 12 more
    w->visible = 0;                      // +0x20
    w->x = screen_width - 297;  w->y = 0;     // default place: top right
    w->width = 277;            w->height = 465;
    w->vtable->load_position(w);         // slot 3: CONFIG.INI SHEET_STATUS_POS_X/Y
    clamp_to_screen(w);                  // 0x5138e0
    control_setup(.., "talk1_close_button.tga", ...); // the close button
    return w;
}
```

### The manager

The UI manager (`this` of `0x424100`, `0x48c120` and `0x48aa00`) holds one pointer per window in fixed fields (`+0x15c`…`+0x2b8`,
`+0x1fec`…`+0x2014`). `0x424100` creates about twenty of them at start-up (`operator new` + constructor, e.g. the status window,
`0x5274` bytes, at `+0x15c`); the other fields start empty and are filled elsewhere (*inferred*: when the window is first needed).

- **Input** (`0x48c120`, called from the world update `0x4216f0`): checks a few global states, then calls slot 1 of the windows one
  after another. Its result goes to the world update `0x43c000`, *inferred*: so a click taken by a window does not also move the
  character.
- **Draw** (`0x48aa00`, called at the end of the world draw `0x4354f0` when `SHOW_INTERFACE` is on): calls slot 2 of each window in
  a fixed sequence, so later windows are drawn on top (confirmed). Some windows are drawn only in certain states (inside nested
  conditions).
- **Modal yes/no dialogs** are not windows of their own kind: a global "pending dialog" mode (`0x850a5c`) selects which question the
  shared dialog shows ([client-social-packets.md](client-social-packets.md) lists the modes).

### Where the UI art comes from

UI textures are loaded by name from `data/interface` (`0x536790`: `"%s/%s"` → `0x40c7c0`, the archive-first texture reader; width and
height from the arguments or from the texture's level 0). 2D drawing is a textured quad of pre-transformed vertices
(`0x536a60`: `SetTexture`, then `DrawPrimitiveUP` with a 28-byte `XYZRHW | DIFFUSE | TEX1` vertex and the usual half-pixel offset).
Text is drawn with the D3DX font at renderer `+0x3a8` (`0x52ee20`). There are no layout or script files: positions and sizes are in
the constructors, and only the saved positions live in `CONFIG.INI` (confirmed for the status window; *inferred* for the rest).

**How to find it.** The base vtable is the one whose slots 1 and 2 point to the same stub; every UI constructor stores it first and its
own vtable next. The `INTERFACE_%dX%d` format string leads to slots 7 and 8, and the `…_POS_X` key names to each window's slots 3 and 4.

## 9. Scene and entity rendering (high level)

The world draw `0x4354f0` (state 6, confirmed) sets up the frame and calls the drawing passes in this order:

1. `0x53cec0`, `0x596fd0`, `0x596ed0`: per-frame preparation (not traced).
2. **Fog**: the start and end distances (`0x70194c`, `0x701950`) move toward the `RANGE` limits by a rate times the frame time, then
   `SetRenderState` sets fog colour, linear vertex fog, no range fog, fog start and end.
3. `0x5301a0`, the material (`SetMaterial`), linear filtering on sampler 0.
4. `0x539c50` with fog off, then fog on again (*inferred*: the sky).
5. `0x52e7c0(aspect 0x6f1574, near 0x701938, far 0x701934)` (*inferred*: the projection; the far value is the `RANGE` distance).
6. `0x447d30`: the scene (terrain, objects, characters, monsters, effects, weather; a large function of about 3,500 lines, not broken
   down here). Order in which its direct calls first appear (confirmed, decompile; names not read): `0x597120`, `0x596ed0`, `0x52b260`, `0x590d90`, `0x591200`, `0x523190`, `0x523300`, `0x593130`, `0x52f1f0`, `0x59a300`/`0x59a370`/`0x59a410`, `0x5301a0`, `0x439670`, `0x428430`, `0x4322e0`, `0x431650`, `0x4344d0`, `0x433950`, `0x40ea50`, `0x5403b0`, `0x535e40`, `0x52e970`, `0x526bd0`, `0x51b020`–`0x51b150`, `0x52d400`, `0x52ab40`, `0x52a9b0`, `0x594190`, `0x5359a0`, `0x546710`, `0x5484d0`, `0x548070`, `0x549990`, `0x430770`, `0x427cb0`, `0x420600`, `0x4349d0`, `0x432c40`, `0x429940`, `0x413820`, `0x4131e0`, `0x413050`, `0x539b40`, `0x53c690`, `0x53bcb0`, `0x53cad0`, `0x5302f0`, `0x433c90`, `0x43b410`, `0x4440f0`, `0x41b1e0`, `0x41b240`, `0x410500`, `0x431a60`, `0x461210`, `0x4c21a0`, `0x524cf0`, `0x530ac0`, `0x444190`, `0x444280` (what each draws is not read).
7. `0x5302f0`, alpha blending (`SRCALPHA`, `INVSRCALPHA`) and texture stages for 2D, `0x41eef0`.
8. The UI (`0x48aa00`, section 8) if `SHOW_INTERFACE` is on.

Model, animation, map and effect file formats, and how animations are played, are on [formats.md](formats.md); the map loader
`0x452270` and the WLD loader `0x4516f0`, with timings, on [graphics.md](graphics.md).

**How to find it.** Follow state 6 of the frame function; the render-state numbers passed to device slot `+0xe4` name each step.

## 10. Input

Three sources (all confirmed):

- **DirectInput 8** for the mouse and the keyboard state: `DirectInput8Create` (`0x5945e0`, version `0x800`), mouse device
  `0x594680` and keyboard device `0x594600`, both created from `0x51e6c0`. The keyboard uses non-exclusive foreground mode
  (cooperative level 6 pushed at `0x594647`); the mouse's level is built from the arguments `(0, 1)` (*inferred*: the same mode). Polled once per frame by the input step `0x51e720` (mouse `0x5943e0`, keyboard `0x594280`; the login screen polls the
  keyboard itself at `0x4b6c07`). Each poll keeps two buffers, "this frame" and "last frame".
- **Window messages** for typing: `0x522bf0` handles `WM_CHAR`, `WM_KEYDOWN`, the IME composition messages and
  `WM_INPUTLANGCHANGE` for the active text box, before the window procedure sees them.
- **`GetAsyncKeyState`** (import `0x6a5460`) at 18 call sites, for example `0x426840` (Enter) and `0x44a0b9`.

The input step `0x51e720` (input object at `0x21ea0b8`, *inferred* from the `+0x18` button field at `0x21ea0d0`):

| Field | Meaning |
|---|---|
| `+0x18` | left button state (the right one when `INV_MOUSE` is on): 0 up, 1 pressed this frame, 3 held, 2 released this frame |
| `+0x20` | right button state, same values; pressing it hides the cursor (`ShowCursor(0)`), releasing shows it |
| `0x704354`, `0x704358` | cursor position in client coordinates (`GetCursorPos` + `ScreenToClient`), clamped to the client area |
| `0x705108`, `0x70510c` | while the right button is held: cursor movement since last frame; the cursor is put back with `SetCursorPos` every frame (camera turning) |

Because DirectInput is in foreground mode and `GetAsyncKeyState` reads the physical keys, the client behaves oddly when it keeps
running in the background with the anti-freeze patch: a held camera button can keep the cursor captured, and keys typed in other
programs reach the game (confirmed mechanism; the symptom is *inferred*).

**How to find it.** `DirectInput8Create` has one caller; the input step is the function that calls both polls and `SetCursorPos`.
`GetAsyncKeyState`, `GetCursorPos` and `SetCursorPos` are imported by name.

## 11. Sound

- **Set-up** (`0x526f80`, from `GameInit`): `DirectSoundCreate8` (`0x594fa0`) with priority cooperative level (2) on the game window,
  two more set-up calls (`0x595120`, `0x595160`; *inferred*: the primary buffer), the 3D listener's factors (`0x5951f0`: listener slots
  11, 12 and 15, distance, Doppler and rolloff factor, from the constants 1.0, 0.1, 2.0; which constant goes to which slot is
  *inferred*), and three folders: `data/sound/back`, `data/sound`, `data/sound` (confirmed).
- **Effects**: `0x527830(name, position, range, …)` on the sound manager `0x21edc20` finds or loads the buffer (`0x527700`, cached by
  name), sets the 3D position, two distances 5 and 30 (*inferred*: minimum and maximum), the effect volume, and plays it. A range of
  exactly 1000 is the marker for a non-positional sound (UI sounds such as `ui_error001.wav` use it). Preloaded sounds take an id and
  a number of copies (`0x528970`); on the lowest memory tier at most 3 copies are kept (confirmed).
- **Music**: `0x527110` switches the background music: `0x526d90` → `0x592e20` opens `data/sound/music/<name>` as a stream
  (`SMusic::CreateStream` in `SAudiereMusic.cpp`, a 256 KiB buffer). A track that is already playing is first moved to a second slot
  with the value 3000 (*inferred*: a 3-second fade-out); the second argument switches on a stream option (*inferred*: looping). The `SAudiereMusic` source name and the `audiere` string show that music goes through the
  open-source Audiere library rather than DirectSound directly.

**How to find it.** `DirectSoundCreate8` has one caller; the `.wav` names in the exe are arguments to `0x527830` and `0x527110`, and
`Open Music Failed! [%s]` marks the stream opener.

## 12. Logging

`0x5a4070(logger, level, format, …)` writes `[HH:MM:SS] [LEVEL ] text` to the log file and flushes it, but only when the file is open,
`(mask & level) == level`, and the level is not `0x1000` (confirmed). The mask is the logger's `+0x3c` (`0x2232e14`); `LogLevel` in
`gsConfig.cfg` sets it, and without the file it is 3 ([client-packets.md](client-packets.md)).

| Level | Name (`0x5a3480`) | Seen at |
|---|---|---|
| `0x1` | `ERROR ` | — |
| `0x2` | `WARN  ` | — |
| `0x4` | `TRACE ` | `GameInit`, `GameExit` banners |
| `0x8` | `INFO  ` | graphics adapter description |
| `0x10` | `DUMP  ` | — |
| `0x20` | `MEMORY` | — |
| `0x40` | `NET   ` | packet dumps (`SEND>>`/`RECV>>`, [re-network.md](re-network.md)) |
| `0x80` | `LOG   ` | — |
| `0x100` | `DEBUG ` | — |
| `0x200` | `SCRIPT` | — |
| other | `USER  ` | — |

`0x1000` is not a level: it puts the log in `UserData/Logs/<timestamp>Log.txt` instead of `Log.txt`. The common `LogLevel=67` is
`ERROR | WARN | NET`. Errors such as `File Not Open [%s]` go through a separate reporter (`0x40c4c0`). It formats its message (`0x5a7c70`), looks up the logger by name (`0x40a2a0`) and calls `0x5a41b0` with level 1 (ERROR), the function, file and line. That call checks the ERROR bit of the level mask, passes the entry to the registered listeners (`0x5a2cc0`) and, if the log file is open, writes `[time] [ERROR ] …` plus a tab-indented message line and flushes. Nothing is shown on screen (confirmed).

**How to find it.** The level names are adjacent strings; `0x5a3480` returns one per level and `0x5a4070` is its caller that formats
`[%s] [%s] %s`.

## In other episodes

- The EP1, EP3, EP4.5 and EP5.4-labelled builds all import `d3d9.dll` and the same audio, input and network libraries as this client
  (**confirmed (file)**, [Episode 1](episodes/ep1.md), [Episode 3](episodes/ep3.md), [Episode 4.5](episodes/ep4-5.md),
  [Episode 5](episodes/ep5.md)). All four EP6 builds import `d3d9.dll` too ([Episode 6](episodes/ep6.md)).

## Open

| Item | What is known | What is missing |
|---|---|---|
| Scene states 8 and 9 | the gambling house: 8 loads `strip.wld` after the gambling table window returns 4; 9 runs object `0x488330` with dungeon music, then goes to 7 | how the scene looks (**live**) |
| `[VIDEO] TEXTURE` | sets `0x70196c` to −1/128/64 | settled: readers `0x4444d0`, `0x52abd0` ([re-open-items.md](re-open-items.md)) |
| `data/Config.cfg` | opened and parsed like `gsConfig.cfg`; not shipped; keys listed in the section above | which keys a localised build ships (not checkable here: needs a localised build's files) |
| FPS overlay | renderer `+0x410` turns it on; only the constructor writes it (0) | nothing in this build sets it |
| Error reporter `0x40c4c0` | receives function, file and line plus a message; writes to the log through `0x5a41b0` at level ERROR | — |
| UI slot 5 | the chat window's override `0x423f40` stores the argument at `+0x3baf`; the chat input handler `0x475e00` skips its edge-resize and cursor handling while it is set; the manager passes 1 when the character-face window (`0x462cc0`, `ch_face.tga`) opens and 0 when it closes | the other windows ignore it |
| Resolutions outside the list | use layout 1 and aspect 1.25 | how it looks (**live**) |
| Per-pass rendering inside `0x447d30` | the order of the outer passes and of the direct calls (section 9) | what each call draws |
