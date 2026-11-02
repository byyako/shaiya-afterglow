# Scripted monsters: what the client does with `06xx` and `1109`–`110B`

This page covers what a server-side monster script (the original EP4.5 server's Lua AI in `ps_game.exe`, `CMob::Lua*` /
`CMob::PAI_*`) can make the client `game.exe` (US build 2010-04-26, patch 108) show: speech, sounds, animations, HP changes. It was
read from Ghidra decompiles of both binaries, with the server's function names from its debug symbols (`ps_game.pdb`). Status words:
**confirmed** (decompiled), **confirmed (disasm)**, **ref** (another page), *inferred*, **live** (needs a test; nothing here was run).
Sizes include the 2-byte opcode. Message numbers are `sysmsg-uni.txt` IDs.

## In short

- **The client has no boss logic.** A boss is an ordinary monster; everything a script does reaches the client as ordinary packets.
- **Monster speech is a chat line**, "name : text", not a bubble over the monster. Some lines also show in the middle of the screen in
  pink for 10 seconds.
- **A script can play a sound** at the monster, change its speed or HP, or swap it for another monster.
- **The client drops all of this for a monster it has not been shown yet**, so the monster's appear packet must come first.

## Summary

- **There is no client-side boss logic.** No handler checks a monster id, a boss flag or a script state; a "boss" is an ordinary
  `0x0601` mob whose model, size, attacks and max HP come from `Monster.SData[wMOBID]`. Everything scripted is driven by packets
  the server sends. The only special case found is `0x0601 byNew`, which plays motion 4 (spawn) for any mob. confirmed
  (`0x552380`; no mob-id compare in the `06xx` handlers listed below)
- **Speech is chat, not a bubble.** `0x1109` and `0x110A` print a chat line "`<mob name> : <text>`" (the name from
  `Monster.SData`); nothing is drawn over the mob. `0x110B` plays a sound file at the mob's position. confirmed
- **All three need the mob to be known**: the client looks the id up in its mob table (`0x447900`, `ECX = 0x7051a0`) and drops the
  packet when the mob is not in view (no `0x0601` yet) or its `wMOBID` has no `Monster.SData` record. confirmed

## Speech and voice (`11xx`)

| op | size | fields | client (`game.exe`) | original `ps_game.exe` | status |
|---|---|---|---|---|---|
| `0x1109` | 8 + len | `u8 kind, u32 mobId, u8 len, char text[len]` (the client copies `len` bytes into a 132-byte stack buffer whose first 128 bytes are zeroed first, so a closing NUL may be counted or not: **keep `len` ≤ 128**; 133–255 overrun the stack; the original sends ≤ 127 characters and, from `LuaSay`, always the full 136-byte buffer) | handler `0x57e650` → `0x554260` with chat kind 4 (`kind == 0`) or 5 (`kind != 0`): `sprintf("%s : %s", name, text)` and `0x41e2d0(type, line)` with **type 0x0F** for kind 0 and **type 0x1E** for kind ≠ 0. Type 0x0F goes to the chat window only. Type 0x1E goes to the chat window **and** the screen-centre list `0x21e4e60` (drawn by `0x4b7d00` at mid-screen − 150 px, 17 px a line, in pink `0xFFFF80FF`, at most 5 lines, each 10 s, `0x4b8220`; sysmsg 481 uses it too). No word filter, no block list (that code path only runs for kinds 0–3). A trailing `"\n"` is drawn by `DrawText` without `DT_SINGLELINE` (format `0x100`), i.e. an empty second line: invisible (*inferred*) | `CMob::PAI_Say` (kind **0**, to the mob's target only, or that player's party), `CMob::PAI_Say2` and `CMob::LuaSay` (kind **1**, to every player within a radius of the mob) | confirmed |
| `0x110A` | 8 | `u32 mobId, u16 msgId` | handler `0x57e6f0` → `0x5532c0`: `"%s : %s"` with the mob's name and **sysmsg `msgId`** (`0x41c180`, `<p>`/`<t>`/`<b>`… markers expanded from their current buffers; an ID missing from `sysmsg-uni.txt` prints "GetMsg Error <id>"), type 0x1E (chat and screen centre, as above). IDs 6401–6404 (the Cailum Sacra boss lines) exist | `CMob::LuaSayByIndex` (to every player within a radius) | confirmed |
| `0x110B` | 38 | `u32 mobId, char sound[32]` (client keeps 31 characters + NUL) | handler `0x57e740` → `0x553340`: plays `sound` as a 3-D sound at the mob's position (`0x527830(name, mob + 4, 100.0, 0)`), through the same loader as every other effect sound: the file is **`data/sound/<sound>`** (`0x5955a0` `sprintf("%s/%s")` with the directory at `0x21edc20 + 0x888`, set to `"data/sound"` by `0x526f80`), loaded once and cached by name | `CMob::LuaSayByVoice` (`TP_CHAT_MONSTER_BY_VOICE`, 38 bytes, to every player within a radius) | confirmed |

- Sound names: the client's sounds are under `Sound/` in the archive (`ui_*.wav`, `mob_*.wav`, `ch_*.wav`, … 899 files); send the
  **bare file name** (`mob_explosion001.wav`), as every other call site does (`ui_error001.wav`); a `Sound/` prefix would look for
  `data/sound/Sound/…` and play nothing. confirmed. There are no dedicated voice files in this client (`VOICE_NPC` / `VOICE_CHAR`
  exist only as config keys).
- Radius: the original passes the radius as a float from the script and checks `sqrt(dx² + dz²) <= radius` per player in the
  cells around the mob. confirmed (`CMob::LuaSay` `0x4b1d30`)

## The `06xx` packets a script can use

Field meanings are on [World packets](client-world-packets.md#mobs-0x06xx); this is what matters for scripted behaviour. All confirmed there unless noted.

| op | what a script gets from it |
|---|---|
| `0x0601` 17 `u32 id, u8 byNew, u16 wMOBID, f32 x, z` | spawn a mob (e.g. adds); `byNew != 0` plays motion 4 (spawn animation), random facing. **HP starts at 0** (`0x415670`): follow with `0x0608`. A `0x0601` for an id the client already has is **ignored entirely** (no model or HP change, `0x552380`): a boss phase that swaps `wMOBID` needs `0x0602` first. Model/size/attacks/max HP are `Monster.SData[wMOBID]` (`0x461210`, 44-byte records) |
| `0x0602` / `0x0604` 6 `u32 id` | despawn (no death animation). `0x0604` skips the existence check (client-only, the original never sends it) |
| `0x0603` 15 `u32 id, u8 byMotion, f32 x, z` | move toward x/z; `byMotion` is stored at mob `+0x84` (*inferred*: walk/run) and nothing else in the handler reads it |
| `0x0605` 17 | normal attack: one of the mob's 3 attack animations, damage applied when the blow lands |
| `0x060B` / `0x060C` / `0x060D` / `0x060E` | skills on a player (single, drain, area, extra damage); `byAttackType` 0/1/2 picks the attack animation; heal-type skills (type 2/4) heal the mob |
| `0x0607` 15 | periodic skill effect on the mob (heal adds, else subtracts; numbers shown) |
| `0x0608` 14 `u32 id, u32 maxHP, u32 HP` | **set** max and current HP (the only way to change max HP after spawn, e.g. an enrage phase). Client-only: the original never sends it |
| `0x0609` 8 `u32 id, u8 attackSpeed, u8 moveSpeed` | change speeds (enrage, slow) |
| `0x0610` 10 `u32 id, i32 nHP` | add/subtract HP (the original's `CMob::LuaRecover`, `PAI_AddEnergy`) |
| `0x0606` 11 `u32 id, u8 killerType, u32 killerId` | death: death animation only if the client's HP for it is < 1, attackers stop (`0x551c50`) |
| `0x0524` 9 `u32 id, u8, u16` | a mob changes shape |

There is no client → server `06xx` packet and no client request tied to a script: the client never asks for mob state.

## For a server

- A scripted line: `0x1109` with `kind = 1` to the players around the mob (radius as the script says), `kind = 0` to the mob's
  target only. Keep the text ≤ 127 bytes and run it through the same `<<NNNcCC` markup check as player chat ([Packet handling](client-packets.md#findings-for-a-server),
  "Chat markup crash"): `0x1109` text is printed with the chat renderer.
- A line from `sysmsg` (localised on the client): `0x110A` with the message ID; prefer it for fixed boss lines.
- A sound: `0x110B` with a file name the archive has under `Sound/`.
- Send them **after** the mob's `0x0601` reached the player, or the client drops them.

## Open

- Nothing left from the decompile (answered: type 0x1E's screen area, the `Sound/` prefix, the `len` and NUL handling).
  **Live** only: how a kind-1 line looks in the centre list next to other notices.
