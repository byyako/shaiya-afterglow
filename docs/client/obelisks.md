# Obelisks and roads (`21xx`)

This page covers what the client `game.exe` (US build 2010-04-26, patch 108) does with the obelisk and portal-link ("road") packets.
It was read from `game.exe` (handlers on the network object's vtable `+0x3f0`…`+0x3fc`, minimap `0x4ba850`, status icon `0x4528d0`)
and from the original EP4.5 server `ps_game.exe` (`CMap::SendObeliskList` `0x42b040`, `SendObeliskChange` `0x42b200`,
`SendPortalLinkChange` `0x42b2c0`, `ObeliskDestroy` `0x42a410`, `UpdatePortalLink` `0x42a000`). Status words: **confirmed**,
**confirmed (disasm)**, *inferred*, **live**. Sizes include the opcode.

## In short

- **Obelisks** are monsters that a faction can capture by destroying them. The client shows them on the minimap and prints a line when
  one falls; the rules are the server's.
- **Roads** (portal links) can open or close a portal. The client only draws their state; it never blocks a portal itself.
- **The "a road will be opened" message never shows on this client**: its handler reads the wrong argument. A five-byte patch fixes it,
  but it has not been applied or tried.
- **The original server ships no road at all**, so in practice only obelisks appear.

## Summary

- The client keeps one list (`0x21e4914`, keyed by `u32` id) of **obelisks and portal links**, each with a state byte and a
  position; it draws them on the minimap and, on maps 68–70, shows a three-state status icon. Nothing else (no portal is blocked by
  the client; portal use is the server's decision). confirmed
- States: **0, 1, 2** for obelisks (drawn on the minimap; *inferred* neutral / light / dark), **100–103** for portal links (the
  original's link states 0–3 sent as 100 + state; 101–103 are drawn, and on maps 68–70 set the status icon to 1–3). confirmed
  (values), *inferred* (meanings)
- **`0x2104` never prints anything on this client**: the handler compares the last `u8` with 1010–1021 (`ADD EAX,-0x3f2; CMP
  EAX,0xb`), which a byte cannot reach, so the "a road to … will be opened" lines (5371–5374) are dead. Use `0x2102` for the link
  state and a notice (`F90B`-style or `0x1109`) for the text if wanted. confirmed (disasm `0x54fcc0`)
- **Why, and a one-instruction fix** (confirmed, decompile + bytes): the reader `0x57fe30` passes `(u16 linkId, u8 old,
  u8 new)`; the handler (`__thiscall`, `RET 0xc`) does `MOVZX EAX, byte [ESP+0x10]` (the **new state**) at `0x54fcd5`, then
  `ADD EAX,-0x3f2; CMP EAX,0xb` and an index table at `0x54fd3c` (`00 01 04 04 04 04 04 04 04 04 02 03`). The values it wants are
  **link ids**: 1010 → 5374 "a road to Jungle of Light will be opened.", 1011 → 5373 Desert of Light, 1020 → 5372 Jungle of Fury,
  1021 → 5371 Desert of Fury; 1012–1019 nothing. So it reads the wrong argument. Patch (5 bytes, same length): file `0x14fcd5`
  `0F B6 44 24 10` → `0F B7 44 24 08` (`MOVZX EAX, word [ESP+8]` = `linkId`). Then a server that numbers its four roads 1010, 1011,
  1020, 1021 gets the line (chat type 7, 8 on map 0) on every `0x2104` for those ids, whatever the state, so send it only when a
  road opens. Not applied, not tested.

## Packets (all server → client)

| op | size | fields | client | original |
|---|---|---|---|---|
| `0x2101` | 3 + 13n | `u8 n`, n × (`u32 id, u8 state, f32 x, f32 z`) | rebuilds the list (`0x553040`); on maps 68–70 a state 101/102/103 sets the status icon to 1/2/3 | `SendObeliskList` (the map's obelisks and links, *inferred*: on entering the map); the client's buffer holds 100 rows (1300 bytes) |
| `0x2102` | 7 | `u32 id, u8 state` | updates that entry's state (`0x551d20`), recomputes the maps-68–70 icon | `SendObeliskChange` (obelisk owner 0/1/2, to the zone) and `SendPortalLinkChange` (link state as 100–103, to the zone) |
| `0x2103` | 6 | `u16 mobId, u8 from, u8 to` | obelisk destroyed (`0x54fbb0`), name from `Monster.SData[mobId]`: from 0 → to 1 (light) or 2 (dark): the new owner's faction sees 463 "Destoyed <p>." (sic); from 1 → 2: light sees 462 "<p> is destroyed!", dark 463; from 2 → 1: dark 462, light 463; other pairs nothing. Chat type 8 on maps 0, 18, 30 and 70, else 7 | `ObeliskDestroy` (`CWorld::SendAll`) |
| `0x2104` | 6 | `u16 linkId, u8 oldState, u8 newState` (the original sends 100–103) | nothing (see summary) | `UpdatePortalLink` (`CWorld::SendAll`) |

Minimap (`0x4ba850`): every list entry with state < 3 is drawn (except on maps 105 and 108), entries with 101–103 when a further
check `0x4c4600` is 0; nothing is drawn on maps 68 and 69. Positions are the entry's `f32 x, z`. confirmed (draw), *inferred* (icon)

## For a server

- Send `0x2101` after the map's entry packets when the map has obelisks or road portals; `0x2102` on every change.
- `0x2103` for the destruction message (the faction logic is the client's).
- Portal use (`0x020A`) is checked by the server only; the client does not grey out a closed road.

## What the original server sends

Read in `ps_game.exe`; the details are on [Obelisks on the server side](../original/obelisks.md).

- The original lists the obelisks of `Data/Obelisk.ini`: map 0 (ids 1 to 7), map 18 (8) and map 30 (9). States 0, 1, 2 are owned by
  nobody (neutral), light, fury. The shipped file makes no portal link, so 100–103 rows and `0x2104` never occur.
