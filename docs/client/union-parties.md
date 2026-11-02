# Union parties (`0B0B`–`0B1A`, `0C0A`–`0C14`)

This page is the short version of the 30-member "raid" party, or union, of the client `game.exe` (US build 2010-04-26, patch 108), with
the points most likely to bite a server. The full packet tables are on [Social packets](client-social-packets.md#union-raid-party). Status:
**ref** (that page, decompiled) unless marked.

## In short

- **A union is made from a party** and holds up to 30 players in 5 groups of 6, with a leader, a sub-leader, a loot rule and an
  auto-join setting.
- **The client becomes a union member only when it receives its own member packet.** The original server does not send that packet to a
  player who joins, so on this client the joiner never really joins: no union window and no party chat.
- **Settings change through their own small packets**, not by re-creating the union.
- **Union chat** goes out as `0x1112`, but the client ignores `0x1112` from the server; lines sent to it as `F10A` show in the chat
  window and in the middle of the screen.

## Summary

- A union is made **from an existing party** (`0x0B0E`): the party members keep their party order as slots 0…; 5 groups of 6
  (`slot / 6`), a leader, a sub-leader, a loot rule (1 order, 2 random, 4 leader) and an auto-join flag.
- **The client sends `0x0B0E` only to create** ("Create Union", not in a union). The same settings window on an existing union
  ("Config Union", leader only) sends **`0x0B0F` (`u32` loot rule 1/2/4)** and/or **`0x0B10` (`u8` auto-join)**, each only when it
  differs from the current value; a sub-leader or member sends nothing. confirmed (`0x4916f5`…`0x491836`)
- The client's **received `0x0B0E` rebuilds the union from its normal party list** (`0x21e4838`, the `0x0B01`/`0x0B02` members), slot =
  index there, after clearing the union list; it needs the leader and sub-leader slots among them, or it sets nothing else (no
  6532, no union flags). Members known only from `0x0B0C`/`0x0B0B` drop out. confirmed (`0x560780`)
- **Only an own `0x0B0C` (or `0x0B0E`) makes the client a union member**: it sets the in-union flag `0x21e4928`, the role `0x21e485c`
  = 2, the window flags and prints 207. `0x0B0B` sets none of these (only the list, leader, sub-leader, rule, auto-join and my
  group); without the flag party chat `0x1105` is looked up in the empty party list (nothing shows), the settings window would
  send `0x0B0E` and `/party` sends `0x0B09`. Send the joiner its own `0x0B0C` too (either order: a slot already listed is kept,
  `0x48bd70` is a map insert). confirmed (`0x561340`, `0x5610c0`)
- Union member units are the party unit **preceded by `u16 slot`** (57 + 7 × buffs bytes); lists (`0x0B0B`) are packed units.
- `0x0B0B` **adds** members by slot; the client never clears the list on it (`wStartEndFlag` and `bRaidPartyType` are unused). Send
  `0x0B16` (or rely on the leave packets) before rebuilding.
- Invitations: `0x0B14` / `0x0B18` (by id / by name) → the invitee answers `0x0B15` with **0 = yes, 1 = no and its own id**; the
  inviter is the server's to remember. `/party <name>` sends `0x0B18` instead of `0x0B09` while in a union (leader or sub-leader,
  union not full).
- `/join <name>` (`0x0B11`) joins a union with auto-join; errors are `0x0B19` 12 / 13 / 14 (6535 / 6536 / 6538).
- Member bars: `0x0C0A`–`0x0C13` mirror the party's `0x0C01`–`0x0C09` (same layouts, **HP, SP, MP** order); there is **no `0x0C0F`**
  handler; `0x0C14` is the union loot line (as `0x0B08`).
- The union chat is `0x1112` (client → server; the client ignores `0x1112` from the server) and staff `F10A`; the leader's
  `/raid`, `/alert`, `/command` lines need region feature 30 (on for region 6) ([Player chat commands](gm-and-commands.md#player-chat-commands-and-prefixes)). Received `F10A`
  (`u32 id, u8 len, text`): the id is read and dropped; the text prints as 6606 "[Union] <b>" with type 0x20, i.e. in the chat
  window **and** the screen-centre list (pink, 10 s, at most 5 lines; see [Scripted monsters](scripted-monsters.md#speech-and-voice-11xx)). No name is added. confirmed
  (`0x5812c0` → `0x554230`)
- Party chat `0x1105` shows for union members: in a union the sender is looked up in the union list and printed as 353
  "[Party]<p> : <b>" (type 10); an unknown sender prints nothing. confirmed (`0x554bd0`)

## Packet order (original, ref; with the joiner's own `0x0B0C` added)

- Create: `0x0B0E` (16) to the members.
- Join: the joiner gets `0x0B0C` (its own unit) and `0x0B0B` with the others (several packets if needed); the others get `0x0B0C`.
  (The original's `ps_game.exe` sends the joiner only the `0x0B0B` list, see [Entering a union](../original/union.md#3-entering-a-union);
  the client then misses the flags above.)
- Settings: `0x0B0F` / `0x0B10` to all members; the client stores the value and prints 6516 / 6517 / 6518 (loot 1 / 2 / 4, other
  values silently) or 6520 / 6521 (auto-join on / off). confirmed (`0x55ea30`, `0x55ea80`)
- Leave / kick: `0x0B0D` / `0x0B1A` to all, then `0x0B17` / `0x0B13` if the leader / sub-leader changed.
- Leader, sub-leader, group moves (`0x0B17`, `0x0B13`, `0x0B12`): to all members.
- Dismantle: `0x0B16` to all, then the union is removed.

## Open

- Whether the original sends `0x0B16` before a new `0x0B0B` list (not traced; see [Social packets](client-social-packets.md#gaps)).
