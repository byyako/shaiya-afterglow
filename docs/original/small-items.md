# Small features

This page collects small features of the EP4.5 game server `ps_game.exe` that have no page of their own: the
all-server chat and the Message To Server item, `/town`, looking at another player's equipment, the VG query, the name change, item
skills on another target (`0x0559` kinds 1 and 2), the Priest, and item special 25. It ends with three client facts about staff and the
reset counters. It was read in the Ghidra decompiler and the disassembly with the PDB names, in `ps_game.exe` and `ps_dbAgent.exe`, with
the stored procedures of the original `PS_GameData`. Nothing was executed. Not covered: the game log records these features write
(`0x0A05` for a rename and the like).

## In short

- **Message To Server** sends the next normal chat line to every player on the server, both factions. The item is used up when it is
  armed; leaving the world loses an armed one.
- **`/town`** takes a player back to the map's bind position after 10 seconds of standing still, like the Return skill without the
  skill. Moving or attacking cancels it silently.
- **Looking at another player's equipment** works only for a player of the same faction in the same zone, at any distance; staff cannot
  be looked at.
- **The Name Change Stone** allows one rename per use, done at the character screen. Online friend lists and guild lists keep the old
  name until they are loaded again.
- **The Priest** reads a character's five hidden numbers (honour, VG, CG, OG, IG) for 10 gold. It is the only place a US player sees
  its honour.
- **The snowball** (the only item that hits another target) checks neither faction, distance nor whether the target may be attacked,
  but its skill does no damage.

Status words: **confirmed** = read in the code (address given); **inferred** = follows from what was read.

`CUser` offsets are the code's. They equal the PDB's up to the money (`+0x140`) and are 4 above it from the map on: `+0x128` character
id, `+0x12d` country, `+0x140` gold, `+0x164` map, `+0x166` direction, `+0x168` HG (`m_wHG`, the honour), `+0x16a` VG (`m_shVG`), `+0x16c`
CG, `+0x16d` OG, `+0x16e` IG, `+0x188` name, `+0x1c4` the bags (`CItem*[6][24]`, bag 0 the equipment), `+0xd0` the position, `+0xdc` the
object id, `+0xe0` the zone, `+0x142c` the status (1 dead), `+0x5768` the privilege byte (11 a player, 1 to 10 staff), `+0x578c` the
account id. `CItem`: `+0x30` the definition, `+0x40` type, `+0x41` type id, `+0x44` quality, `+0x46` gems[6], `+0x4c` craft name[21].

## How it was read

From `ps_game.exe` and `ps_dbAgent.exe` (EP4.5, with their PDB names), read in the Ghidra decompile (REA) and the disassembly
(`tools/disasm.py`, names from `docs/original/symbols/ps_game.tsv` and `ps_dbAgent.tsv`); struct layouts from the PDB (`tools/pdb_types.py`,
`docs/original/symbols/ps_game.packets.txt`); stored procedures from the restored `PS_GameData` (the `newshaiya-mssql` container). Client side:
`docs/client/CLIENT-FACTS.md` ("Answers to the live test", "fixes.dll changes the server must follow"),
`docs/client/gm-and-commands.md` ("Answers for the server's live test") and `docs/client/packet-opportunities.md`, not repeated
here except where the server's side adds to them or disagrees.

Covered: part A, three fixes a live test requested (the `/kick` notice, the reset counters, the invisible staff member) and
one the client's `fixes.dll` asks for (`/server off`); part B, the small features an earlier survey left unbuilt: the all-server
chat with the Message To Server item, `/town`, looking at another player's equipment, the VG query, the name change, item skills on
another target (`0x0559` kinds 1 and 2) and the Priest. **Skipped** (no case worth building): item special 25 (section 10).
**Not covered:** the game log records these features write (`0x0A05` for a rename and the like).

`CUser` offsets are the code's. They equal the PDB's up to the money (`+0x140`) and are 4 above it from the map on: `+0x128` character id,
`+0x12d` country, `+0x140` gold, `+0x164` map, `+0x166` direction, `+0x168` HG (`m_wHG`, the honour), `+0x16a` VG (`m_shVG`), `+0x16c` CG,
`+0x16d` OG, `+0x16e` IG, `+0x188` name, `+0x1c4` the bags (`CItem*[6][24]`, bag 0 the equipment), `+0xd0` the position, `+0xdc` the object
id, `+0xe0` the zone, `+0x142c` the status (1 dead), `+0x5768` the privilege byte (11 a player, 1 to 10 staff), `+0x578c` the account id.
`CItem`: `+0x30` the definition, `+0x40` type, `+0x41` type id, `+0x44` quality, `+0x46` gems[6], `+0x4c` craft name[21].

# Part A: fixes from a live test

## 1. `/kick` and `/akick`: the notice before the close (client verified; server change)

**Now:** part 2's deviation 6 (`docs/original/gm-commands-2.md`) sends `F804` (`/kick`) or `F709` (`/akick`) to the target, then
`Connection::kick_out` shuts the sending side down at once (`server.rs`). The ban notices of part 1's deviation 18 (`F70A`, `F805`, `u16
minutes`) go the same way.

**The client** (`docs/client/gm-and-commands.md`, "`/kick`: `F804` before the close"): `F804`/`F709` → `0x554010` prints 339 "Log out by
operator" **in the chat window** only; the socket is in `WSAAsyncSelect` mode, so the `FD_CLOSE` that follows straight away opens the modal
10069 box ("connection with server has been disconnected") and the game quits after it: the chat line is never seen. The box can only say
10069, 10070 (`0x0004 0`), 10071 (`0x0004` other) or 10069 with an error code. **The fix the client side asks for:** send the notice, keep the
connection open 2 to 3 s, then close: the player reads 339 in the chat, then gets the box.

**The original** never sends `F804`/`F709` (it closes with `Close(9, 0)` and no packet), so the hold is part of our deviation 6 and needs
no new reading of `ps_game.exe`.

**Built** (deviation 1, approved): after an operator's notice (`F804`, `F709`) and a ban notice (`F70A`, `F805`), the connection

1. sends the notice under the current encryption (as now);
2. stops reading packets for the game: the character **leaves the world at once** as now (others see it go, it is saved), so the hold
   changes nothing in the world;
3. keeps the socket open for **`GAME_KICK_NOTICE_SECS`** (default **3**; 0 = close at once as now), reading and dropping whatever the client
   still sends; if the client closes first, the hold ends there;
4. then shuts down the sending side and drains as now (`QUIT_GRACE`).

The newer-login kick (`0x0004 0`, `Kick::NewLogin`) keeps closing at once: its text is the box itself (10070). The account is free for a new
login during the hold (the `Online` is dropped before it, as before the drain now), so a banned player's retry gets the login server's refusal
and a kicked one can log in again at once.

The ban notices are shown by the same handler family (`F805`/`F70A` → 340 "Ban connecting by operator" or 341 "… for <v> minutes"); that they
go to the chat window like 339 is *inferred* from the same handler table (`gm-and-commands.md`, the `F501`… table), not traced; the hold does no
harm if they don't.

**Live test:** StaffChar (staff) `/kick TestChar`: TestChar's chat shows "Log out by operator", about 3 s later the 10069 box. `/aban 2
testaccount2` with PriestChar online: "Ban connecting for 2 minutes by operator" in the chat, then the box.

## 2. The reset counters `0x021B` (client verified; no behaviour change)

**The client** (`CLIENT-FACTS.md`, "Reset counters (`0x021B`)"): the two bytes are display numbers only, "**<v> chances left**. Do you want to
initialize …?" (5172 stats, 5173 skills) at the NPC of type 13; nothing is gated on them; after a successful reset the client lowers each by 1
when it is not 0. The client side read `world::reset_flags` (`0xFF` / `1 − x`) as possibly showing "255 chances left".

**The original** (`docs/original/item-use.md` section 8, `CUser::PacketMaininterface`, the enter burst item 21): `0x021B` = `u8
−m_byResetStatus, u8 1 − m_byResetSkill`, with `m_byResetStatus` **forced to 0 at login** and `m_byResetSkill` 0 or 1 (values above 1 become
1). So the original always sends **`00 01`** while the free skill reset is unused and **`00 00`** after: "0 chances left" for the stats (its free
stat reset `0x0214` is always refused) and "1" or "0" for the skills. These are the real numbers of free resets left.

**The new server** sends exactly that: `enter.rs` calls `reset_flags(false, free_skill_reset_used)`, and `false` is the only value it ever
passes, so `0xFF` is never sent. After the free skill reset the 6-byte `0x0215` (item-use deviation 5) makes the client lower its count to 0;
after a relog `00 00` comes from the saved `free_skill_reset_used`.

**Built** (no deviation, clarity only): replace `reset_flags(stat_reset: bool, skill_reset: bool)` by **`reset_counts(stats_left: u8,
skills_left: u8)`**, which writes the two numbers as they are; `enter.rs` passes `(0, 1)` before the free skill reset and `(0, 0)` after. A test
pins `00 01` and `00 00`. If free stat resets are ever offered, the count goes there.

**Live test:** at the type-13 NPC (the combat instructor, e.g. 분노의 전투교관), the stat reset question says "0 chances left", the skill
reset "1 chances left" on a character that has not used its free skill reset; after using it, "0", also after a relog.

## 3. The invisible staff member (client verified; server matches; a packet log is needed)

**The client** (`gm-and-commands.md`, "Invisible GMs: what hides another player"): only **`0x051D` shape 13** hides another player, and
only on a client whose own privilege byte is **above 10** (a staff client never hides anyone); an unknown id is ignored; a **new `0x0201`
clears the flag**, so 13 must come **after** that player's `0x0201`; `0x051D` 0 and `0x0524` with shape 0 clear it too. A flagged player is
**not drawn** when it came in as `0x0201` **kind 2** (staff) **or** is of the other faction; otherwise it is drawn see-through (Stealth among
its own side).

**The server** (checked against the code):

- A player coming into view (`Zones::enter`, the cell change in `Zones::moved`): to each viewer its `0x0201` (`Placed::enter_packet`:
  **kind 2 when it entered as staff**, `Occupant::staff`), its `0x051C` speeds (within the speed range), then `Placed::form_packet`, which
  for an invisible staff member (`Occupant::visible` false) is `Placed::shape_packet` = **`0x051D <id> 13`** (`INVISIBLE_SHAPE`). The same
  three for every player already in view go to the newcomer, in that order.
- `/char on|off` (`Zones::set_gm_flags`): `0x051D` 13 or (visible) the real form, `0x051D <id> 0` for none, to the view.
- Every later shape change (`reform`, mounts in `vehicle.rs`) sends 13 again while the staff member is invisible, never 0.
- Nothing re-sends a `0x0201` for a player a viewer already has (a view change sends `0x0202` first).

That is what the client needs. TestChar's account (`testaccount`) has status 0 (privilege 11), and StaffChar is Fury
while TestChar is Light, so StaffChar must be hidden from TestChar even by the faction rule alone. The cause is not on either side's
paper, so the next step is a **packet log on the watching client** (`gsConfig.cfg`'s packet log, `Log.txt`, `RECV>>` lines; the client
repo's notes say how to turn it on). Opcodes are little-endian; StaffChar's id is 6:

| what | must show | must not show |
|---|---|---|
| StaffChar logs in near TestChar | `RECV>> 01 02 06 00 00 00 02 …` (the `0x0201`, 29 bytes, kind **02**), then (maybe `1C 05 06 00 00 00 …`, the speeds) **`1D 05 06 00 00 00 0D`** | a `1D 05 06 00 00 00 00` or a `24 05 06 00 00 00 …` afterwards; a second `01 02 06 00 00 00` with no `0D` after it |
| TestChar walks into view of an invisible StaffChar | the same pair, in that order | as above |
| `/char off` | `1D 05 06 00 00 00 00` (StaffChar drawn) | |
| `/char on` | `1D 05 06 00 00 00 0D` (hidden again) | |

If the log shows the pair in that order and StaffChar is still drawn, the lines go back to the client side. If the `0D` is missing or comes
first, it is a server bug.

**Built** (no deviation): a **debug log line** each time the server sends a staff member's shape 13 to a viewer
(`"staff hidden from viewer"`, staff id, viewer id, which path: enter, view change, `/char`), so the server log and the client log can be
laid side by side. Nothing else changes.

**Live test:** the table above, with TestChar's packet log on; and section 37's two items again.

## 4. `/server off` with `fixes.dll` (client verified; decided, section 4.1)

**The client** (`CLIENT-FACTS.md`, "fixes.dll changes the server must follow"): the patched client (`game.exe` +
`fixes.dll`, `command_fixes=1` by default) sends **`F707` for `/server off`**, as the original client was meant to; a client without the DLL
still sends `F706` for both words. It also sends `FA0C` for `/cwatch off` (no payload) and takes `light`/`dark` for `/bnotice` (the same
`F90A` packet), and the unquoted text forms (the same packets).

**The original** (`docs/original/gm-commands.md` 7.5): `F706` = accept new connections, `F707` = stop accepting, each idempotent.

**Now:** deviation 19 of `docs/original/gm-commands.md` makes `F706` **flip** the switch (the stock client could never close otherwise);
`F707` sets it off. With the DLL a `/server on` on an open server would therefore close it.

### 4.1 The choice (decided: revert)

**Deviation 19 of `docs/original/gm-commands.md` is withdrawn**: `F706` opens, `F707` closes, both idempotent, as the original. A GM on a
client without the DLL can open but not close the server again (as with the original server). (The other choice was to keep the flip.)
Deviation 20 (a closed server is listed as closed at once) stays.

`FA0C` and `/bnotice` need nothing: `FA0C` is answered `F501 FA0C` as now (listening was not built then, deviation 1 of part 1: this client has no
`F502` handler; for the companion DLL, `docs/original/gm-commands.md` section 8.4 and deviations 21 to 24), and `F90A` already reads the faction byte.

**Live test:** `/server off` (with the DLL) closes, a second `/server off` keeps it closed, `/server on` opens, a second keeps it
open; testaccount2's game login is refused only while closed.

# Part B: the small unbuilt features

## 5. The all-server chat and the Message To Server item

Confirmed.

### 5.1 Arming: item special 38 (`CUser::ItemUse` 0x4655e0, the case at 0x46605d)

The item is **Message To Server** (100/64, `reqig` 1, so reuse group 1: 15 s; the only special-38 item, in the database and the US client).
After the usual checks (`docs/original/item-use.md` section 2):

1. a chat item already armed (`m_bUseChatItem` `+0x5840` ≠ 0) → the error answer;
2. now before the all-server cooldown (`+0x5844`, a tick) → the error answer;
3. otherwise `+0x5840 = 1` and the item is **used up at once** (`CUser::ItemUseNSend`, `0x050A` to the view).

The flag is not saved: `CUser::ResetCharacter` (leaving the world) and `CUser::CUser` clear it, so an armed item is lost by leaving the
world (the item is already gone). Map changes keep it.

### 5.2 Using it (`CUser::PacketChatE` 0x470490, `CUser::PacketAdminChat`)

Before anything (all `0x11xx`): a chat ban (`+0x577c`) drops the line (built).

- **`0x1101` normal chat** (only while fully in the world, `+0x5758` 4; text 2 to 128 bytes, more closes, fewer is ignored): with the flag
  armed the line does **not** go to the view: it goes out as **`0x1108`** to **every player on the server**, is logged as kind 5 ("TRADE") and copied to a listening GM, and the **flag is
  cleared**. **No cooldown is checked or set on this path.**
- **`0x1108` from the client** (text 2 to 128, more closes): **ignored** unless the flag is armed and now is past `+0x5844`; then the same
  `0x1108` to everyone, the flag cleared, and **`+0x5844` = now + 30 s**. (No check of `+0x5758`.)
- **`F101`** (staff normal chat, `docs/original/gm-commands.md` section 5): with the flag armed it goes out as `0x1108` to everyone instead of
  `F101` to the view, and the flag is cleared (`PacketAdminChat` writes `+0x5840`).

**How it is sent back** (verified, `CWorld::PSendAll` 0x412ff0 → `CZone::WorldSendAllPush` 0x420250 type 3 → `CZone::PSendAll`
0x421030 for every zone and every instance zone): **every player in a zone, both factions, gets the same `0x1108`**; the two ids passed
(the sender's object id and 0) are the players who get it **at once** (`SConnection::Send`), everyone else gets it through their send queue
at priority 7 (skipped for a player whose queue already holds 2048 packets). So **the sender itself receives the `0x1108`, not the `0x1101`
echo** a normal line gives (`CObject::SendViewChat` includes the sender): its own line comes back as an all-server line, never as normal
chat, and nobody gets a `0x1101` for it. A player who is not in a zone (loading a map) gets nothing.

**`0x1108` to the clients** (`TP_CHAT_SENDALL`, `len + 24` bytes): `char name[21]` (the sender's), `u8 len`, `text[len]` (the text as it
came, zero included). The name field is the sender's name up to its zero (the rest of the 21 bytes is whatever the stack held; the new
server pads with zeros). The client shows it as "[chatting all members]<name> : <text>" (481) and drops it when it has the sender blocked
(`CLIENT-FACTS.md`, block list).

**The US client never sends `0x1108`** (`docs/original/world.md` section 6a, client verified), so with this client the item works through
`0x1101`: use the item, then the next normal chat line goes to the whole server. The 30 s cooldown and its check at arming never come into
play with this client (only the `0x1108` path sets it).

### 5.3 The new server (built)

- `Effect::MessageToServer` for special 38: refused while armed or before the cooldown; otherwise armed (in `WorldState`, not saved,
  dropped when the character leaves the world, kept over map changes) and the item used up.
- In the chat handler (after the ban and markup checks, which every chat line already passes): `OP_NORMAL` with the flag armed → `0x1108`
  (`chat::by_name(OP_ALL_SERVER, name, text)`) to every player in a zone **including the sender** (`Zones::send_all`), **no `0x1101` to anyone**
  (in particular not the usual echo to the sender), flag cleared, no cooldown; `OP_ALL_SERVER` → as 5.2; staff `F101`
  as 5.2. Logged at info ("all-server chat", the sender and the text).
- No deviation. One choice is asked (5.4).

### 5.4 A global chat without the item (decided: not now)

The server could send normal chat lines with a prefix as `0x1108` without the item (a global chat was asked for). Not built
now; only the original's item path is.

**Open for the client side** (not analysed here, `docs/client` is the client side's): how the client shows an `0x1108` whose name is its own
character (the same 481 line, or another colour or channel), and whether the client itself keeps a "message to server" state after a
successful special-38 `0x050A` that changes what it sends (the server's reading: it sends plain `0x1101`, `docs/original/world.md` section 6a).

**Live test:** TestChar uses a Message To Server: the item goes; a second one at once is refused (and the first is not lost); TestChar
types a normal line: PriestChar on another map and StaffChar (Fury) both see "[chatting all members]TestChar : …", and **TestChar's own
chat shows the same all-members line, not a normal one** (note how it looks); a player standing next to TestChar gets no normal-chat copy;
the next line is normal chat again; TestChar leaves to the character screen with an armed item and comes back: its next line is normal chat.

## 6. `/town`: `0x0556`

Confirmed, `CUser::PacketPC` 0x46a230, `CUser::UpdateResetPosition` 0x48d470, `CUser::CancleMove` 0x48a010.

The client sends `0x0556` (2 bytes) for `/return` (5076) or `/town`, not while busy or dead, and prints 251 itself
(`gm-and-commands.md`, player commands). The server never answers it.

**The request:** ignored while dead (`+0x142c` 1). Otherwise, when **no map change is pending** (`+0x57dc` 0, the deferred-move time of
`docs/original/gm-commands.md` section 8) **and no move is pending** (`m_dwMovePositionTime` `+0x581c` 0, the pending move shared with the
gatekeeper and the items, `docs/original/gatekeeper.md`, `docs/original/item-use.md` section 9): the pending move becomes **type 0**, due
**10 s later**, with the current map and position (both unused by type 0). Otherwise nothing.

**Cancelled** like the item moves (`CancleMove`: an accepted step, the start of an attack or skill `0x0502`/`0x0503`/`0x0511`/`0x0517`, a
hit that breaks a cast `StatusCastingStop`, the attack tick), but **silently**: type 0 is dropped with no `0x0222`.

**When due** (`UpdateResetPosition` type 0): the personal shop is closed (`stMyShop::Ended`, every type does it); **`CMap::GetBindPos(country,
position now)`** (0x4295a0, the bind position of the map the player is on now, `docs/original/mob-fight.md` section 8); none → nothing
happens. The same map → `CZone::MoveUser` and **`0x020B`** (`u32 id, u16 map, f32 x, y, z`, 20 bytes) to the view, the player included
(`CObject::SendView`); another map → `CWorld::ZoneLeaveUserMove` and `0x020B` to the player only (it then loads the map and sends `0x0201`).
**No logout check, no dead check** at that point (a hit cancels it first in practice), no item, no cooldown.

So `/town` is the Return skill (125/192) without the skill: back to the map's bind position after 10 s of standing still.

**The new server (built):** a pending move of a new kind "town" in the slot the item moves use (`state.item_move`), so that an item move refuses or
replaces it exactly as the item code already does for the shared slot, and every cancel point already there drops it, with no `0x0222`
and no reuse time to put back. Refused while dead, while a `/xcall`-type move is pending (`state.called`) or while any move is pending.
When due: the bind position of the current map (`World::bind_pos`, as the Return skill does) and the same move as the Return skill
(`move_player`: same map `0x020B` to the view, another map leave + `0x020B` to the player). No deviation.

**Live test:** TestChar types `/town` and stands still: 10 s later it is at the map's bind point (the same spot as the Return skill). Again,
and step within the 10 s: nothing happens (no message). Dead: nothing.

## 7. Looking at another player's equipment: `0x0307`

Confirmed, `CUser::PacketGetInfo` 0x469cf0, the case at 0x469f45.

**The request** (6 bytes): `u32 target id` (the object id). **Answered only when** the target is found **in the requester's zone**
(`CZone::FindUser`), the target's privilege byte is **above 10** (a player: staff cannot be looked at) and the target is of **the same
country** as the requester. No distance, view or state check; otherwise no answer.

**The answer** (`TP_USER_ITEMS`, `3 + 32 n` bytes, only the used part sent): `u8 n`, then for each **equipped item in slots 0 to 12** of bag 0, in
slot order (`TP_GETINFO_ITEMUNIT`, 32 bytes): `u8 slot, u8 type, u8 type id, u16 quality, u8 gems[6], char craft name[21]`. Exactly what the
client reads (`client-packets.generated.md`: `u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:bytes[6] loop:bytes[21]`).

**The new server (built):** the zone checks the target (same zone, not staff, same country); the equipment lives in the target's connection, so the
zone sends the target's connection an order (`ZoneEvent`), and the target's connection builds the packet from its bag 0 (with the quality as
worn now) and sends it to the requester (`Zones::send_to_char`). Staff = entered the world as staff (`Occupant::staff`; the original reads the
current privilege byte, which only differs after `/apower`/`/apowercan` in the same session). No deviation.

**Live test:** TestChar looks at PriestChar's equipment (the player menu): the window shows PriestChar's gear with its gems and craft
numbers; at StaffChar (Fury, or staff) nothing happens.

## 8. The VG query: `0x0555`

Confirmed, `CUser::PacketPC` 0x46a230.

**The request:** 2 bytes. **The answer** (`TP_ITEM_REPAIRVG`, 4 bytes): `i16 VG` (`m_shVG` `+0x16a`, the character's `vg`), always, with no
other check (not even death). The client keeps it for the lapis-link window, where the price is lowered by `VG / 10`
(`packet-opportunities.md`; the server's price rule is `docs/original/lapis.md` section 5 step 4).

**The new server (built):** answer the character's current `vg` (it can change by `/set VG`). No deviation.

**Live test:** with `/set` VG 1000 on TestChar (`F701` 12), the link window's price is 100 lower than with VG 0 and matches what the
server takes.

## 9. The name change: item special 51 and `0x010E`

Confirmed.

### 9.1 The stone (`CUser::ItemUse`, the case at 0x466b0f)

The item is **Name Change Stone** (100/136, `reqig` 10: reuse group 10, 600 s; the only special-51 item). After the usual checks:

1. used once already since entering the world (`+0x58dc` ≠ 0; cleared by `CUser::ResetCharacter`) → the error answer;
2. otherwise `+0x58dc = 1`, **`0x0616`** (6 bytes: `u32 account id`, the agent's routing field) to ps_dbAgent, and the item is **used up at
   once**, without waiting for the database.

ps_dbAgent (`CUser::OnDispatchDB` 0x403dd0, in the world state 2) → `DBCharacter::SaveEnableCharName` 0x41e8a0 →
**`Usp_Save_Char_EnableRename_E(m_dwCharID)`**: `UPDATE Chars SET RenameCnt = RenameCnt + 1 WHERE CharID = @CharID` for **the character in
the world**. `RenameCnt` is a `tinyint` (0 to 255) **counter of renames allowed**; at 255 the update overflows and fails (logged), the item is
already gone.

### 9.2 The character list

Each listed character's byte 76 is **1 when `RenameCnt` ≠ 0** (`docs/original/ps_game.md` section 6.1): the client then offers the rename for
that character.

### 9.3 The rename, `0x010E` (`CUser::PacketCharacter` 0x46bf20, `CUser::PacketUserDBChar` 0x46cf60; ps_dbAgent `CUser::PacketUserChar` 0x4056e0, `CUser::RenameCharacter` 0x4047f0, `DBCharacter::RenameCharacter` 0x41d6a0)

**The request** (`TPC_CHAR_RENAME`, 27 bytes): `u32 character id, char name[21]`.

1. Only when a rename may be sent (`m_bEnableDBCharacterRename` `+0x57c1`, set after the list and after each rename answer); it is cleared
   here. Otherwise nothing.
2. The name is trimmed (`TrimString`: every space removed, byte 20 forced to 0); its first two bytes must be non-zero, otherwise **`0x010E u8
   2`** (3 bytes).
3. **`0x0409`** to ps_dbAgent (31 bytes: `u32 account id, u32 character id, char name[21]`).
4. ps_dbAgent: the character must be **one of the account's five listed characters** with **`RenameCnt` > 0**; otherwise no rename (the
   answer on that path, `0x40591f`, was not traced). Then **`usp_Save_Char_Name_E(server, character, old name, new name)`** (both trimmed):
   - a **live character** (`Del = 0`) of that name exists (this server's database; the comparison is the database's, case-insensitive
     under its default collation) → −2;
   - `Chars`: `CharName` = the new name, **`RenameCnt − 1`**, **`OldCharName`** = the old name; `Guilds.MasterName`, `FriendChars.FriendName`
     and `BanChars.BanName` follow the new name; a `CharRenameLog (ServerID, CharID, CharName)` row; → 1.
   The agent's own copy of the slot gets the new name and `RenameCnt − 1`, and it answers `0x0409` with `u8 1` (53 bytes) or `u8 0`.
5. The game: `+0x57c1 = 1` again; **`0x010E u8 1`** on success (and a game log record `0x0A05`), **`0x010E u8 2`** otherwise. No new list:
   the client renames its own entry.

Nothing updates the game server's memory: an online friend's list and the guild's member list (`CGuildMgr`) keep the old name until they
are loaded again.

### 9.4 The new server (built)

- **Storage** (migration **0026**): `characters.rename_allowed boolean` becomes **`rename_count smallint`** (0 to 255; a `true` becomes 1),
  plus `characters.old_name varchar(20) NULL` and a log table `character_renames (id, server_id, character_id, old_name, new_name,
  renamed_at)`. The list's byte 76 = `rename_count > 0`.
- **The stone**: once per world entry (as `+0x58dc`), refused (error, item kept) when the count is already 255 (deviation 6); otherwise the
  count + 1 and the item used up in one write.
- **`0x010E`** at the character screen: one request at a time (a rename in flight ignores the next, as `+0x57c1`); the name parsed as now
  (`parse_char_rename`, cut at the first zero); then the **creation name rule** (2 to 18 ASCII letters or digits, deviation 3); the character
  live, of this account and this server, with a count above 0, and **not still in the world** (a lingering character of the previous
  connection, `Config::linger`) (deviation 4); the name free among live characters ignoring case (the `characters_name_key` index, every
  server, as creation); then one transaction: name, `old_name`, `rename_count − 1`, the log row; the market's seller and bidder names of
  that character follow (deviation 5). Answer `0x010E 1` or `0x010E 2`.
- **Memory** after a rename: the guild member list's name (`Guilds`) and the auction house's names (`Market`) are updated (deviation 5). Friends'
  lists of online players load names from the database at their login, as the original's: they show the new name after their next login.

**Live test:** TestChar uses a Name Change Stone (refused a second time in the same session); at the character screen TestChar offers
the rename; a taken name (PriestChar) fails; "Ranger2" succeeds and shows; the stone's count is used up (no rename offered after another
logout); PriestChar's friend list and the Builders guild list show the new name (the guild at once, the friend list after PriestChar's
relog); `character_renames` has the row.

## 10. Item special 25 (skipped)

`CUser::ItemUse` has a case (0x465b02): the weapon (bag 0 slot 5) gets `wQuality[0]` (the item's `+0x4c`) added, capped by the weapon's full
quality including its lapis (`docs/original/item-use.md` section 10); no weapon → error. **No item has special 25**, neither in `defs.items`
nor in the US client's `Item.SData` (checked). Nothing to build; `Effect::NotBuilt` for 25 becomes `Effect::Refused` (the error
answer, as now).

## 11. Item skills on another target: `0x0559` kinds 1 and 2

Confirmed; decided, 11.1.

`0x0559` (9 bytes: `u8 bag, u8 slot, u8 kind, u32 target`; refused while riding, `docs/original/item-use.md` section 1). For an item that
names a skill (`byRange`, `byAttackTime`) and is not an experience wheel (`CUser::ItemUse` 0x465904):

- **kind 1**: the player `target` found **in the user's zone** (`CZone::FindUser`) and **without a personal shop** (`+0x15e0`), then
  **`CUser::AttackUser(target, skill, single = false, option = false)`** (0x44f4c0, `docs/original/pvp-2.md` section 3: hit or miss, damage,
  debuff, as an area hit: `0x0513`). **No other check**: not the faction, not the distance, not whether the target is alive or attackable
  (`EnableAttackUser` is not called), not the map's PvP rules.
- **kind 2**: the monster `target` in the zone (`CZone::FindMob`), then **`CUser::AttackMob(mob, skill, false, false)`** (0x44fd90,
  `docs/original/skills-use.md` section 8: an area hit `0x0519`; the monster targets the player). No other check.
- Either way the item is **used up** once the target is found (whatever the hit did); target not found or another kind → the error answer.

**The data:** one item uses it, **44/147 "雪球"** (a snowball, reuse group 11: 2 s) with skill **193 level 1** (damage type 0 with `wDamage`
0: a hit that does nothing but show, range 14). In the US client's `Item.SData` the item is **"???147"**, a placeholder: no US player
has a real snowball.

### 11.1 The choice (decided: build it with checks, deviation 7; built)

The hit is the original's (an area-style hit, `single` false, no item options, the item used up once the hit is made), but only on a target
the user may attack:

- **kind 1**: the player is in the user's zone, is not the user, has no personal shop open, **may be attacked by the user as with a skill**
  (`Zones::may_attack_player`: alive, visible, not hidden, players-only switch on, and an enemy, the duel partner or a guild-war foe), and is
  **within the skill's range** (the range rule the skill code uses for a player target);
- **kind 2**: the monster is in the zone and **attackable** by the user (`Zones::attackable`: alive, of another country) and within the
  skill's range (`combat::in_range`, as for a skill on a monster);
- otherwise the error answer, the item kept and its reuse time put back (as every failed effect).

*Why*: without these, a modified client could hit (and debuff, and enter the damage record of) a player of its own faction anywhere on the
map with any skill item. The other choice was to skip kinds 1 and 2.

## 12. The Priest: `0x0F01` and `0x0F02`

Confirmed, `CUser::PacketPriest` 0x467560.

Only in the world (the world dispatcher). Both requests are 2 bytes; any other `0x0Fxx` closes the connection (`Close(9)`).

- **`0x0F01`** (the reading): when the gold is **above 10** (strictly: exactly 10 is refused): gold − 10, saved (`CUser::SendDBMoney`), and
  **`TP_PRIEST_START`** (10 bytes): `u8 1, u16 HG, i16 VG, u8 CG, u8 OG, u8 IG` (the character's five hidden numbers: HG is the honour). Not
  enough gold: `0x0F01 u8 0` (3 bytes).
- **`0x0F02`**: gold above 10 → gold − 10, saved, `0x0F02 u8 1`; otherwise `0x0F02 u8 0`.

No NPC, distance or state check (not even death). The client (`packet-opportunities.md`) opens dialog `0x69` with the numbers on `u8 1` and
shows 303 "Insufficient gold" on 0; `0x0F02` 1 "shows it". Which NPC's menu sends them is not known on the server side (open for the client).
Since the US character window has no honour line and the client ignores `0x0230` (`CLIENT-FACTS.md`), the reading is the only place a player
sees its honour.

**The new server (built):** as above, the gold through the money write the shops use (`ItemStore::trade` with no slot change). No deviation; the NPC
check stays off until the client side names the NPC (then it could be a deviation like the shops').

**Live test:** with a character holding 11 gold or more: the reading shows its HG (honour), VG, CG, OG, IG, gold − 10; with 10 gold: 303.
Mark "can't test" if no NPC offers it.

## 13. Original oddities

1. The Message To Server's cooldown is only set by `0x1108`, which the US client never sends: with this client the item has no cooldown
   besides its reuse group (15 s).
2. An armed Message To Server is lost by leaving the world; the item is already used up.
3. The Name Change Stone is used up even when the database cannot add the rename (255 already); the 0x0616 payload's account id is
   only routing, the agent renames the character it has in the world.
4. A rename leaves every online copy of the name stale (friend lists, the guild's member list) until loaded again.
5. `0x0307` and `0x0559` kind 1 find their target anywhere in the zone; `0x0559` kind 1 checks neither faction nor whether the target may be
   attacked.
6. The Priest refuses exactly 10 gold (`10 < gold`).
7. `/town`'s due move checks neither death nor a pending logout.

## 14. Deviations (all decided; all but 2 built)

1. **Approved, built. Kick and ban notices are held before the close** (section 1): after `F804`, `F709`, `F70A`, `F805` the socket stays open
   `GAME_KICK_NOTICE_SECS` (default 3) with the character already out of the world, so the client shows the chat line before its box.
   *Why*: the client needs it to show 339/340/341 at all (client verified). Amends deviation 6 of `docs/original/gm-commands-2.md` and
   deviation 18 of `docs/original/gm-commands.md` (when the notice is seen, not what is sent).
2. **Not built (decided): a global chat without the item** (section 5.4). The number is kept free.
3. **Approved, built. The new name follows the creation rule** (section 9.4): 2 to 18 ASCII letters or digits, unique ignoring case across the
   servers. *Why*: the original only needs two non-zero bytes (the database decides the rest); our creation already uses this rule.
4. **Approved, built. A rename is refused (`0x010E 2`)** for a character with no rename left, not of this account and server, deleted, or still in
   the world after the previous connection (section 9.4). *Why*: the original's agent gives no traced answer there; a lingering character would
   keep its old name in the world.
5. **Approved, built. A rename updates the server's own copies of the name**: the guild member list in memory and the auction house's seller and
   bidder names (database and memory), in the same step (section 9.4). *Why*: with names stored by id elsewhere, these are the only copies left;
   `/auctionsearch` and the auction rows would otherwise show a name nobody has.
6. **Approved, built. A Name Change Stone with 255 renames already allowed is refused** (error, item kept; section 9.4). *Why*: the original uses it
   up while the database update fails.
7. **Chosen, built. `0x0559` kinds 1 and 2 are built with the attack checks** (section 11.1): only on a target the user may
   attack with a skill, within the skill's range; otherwise the error answer and the item kept.
8. **Decided, built. Deviation 19 of `docs/original/gm-commands.md` is withdrawn** (section 4.1): `F706` opens, `F707`
   closes, as the original.

## 15. As built

Built as above (`docs/plans/done/small-items.md`): `crates/shaiya-proto` (`world::reset_counts`, `world::user_items`, `item::vg`, `item::OP_TOWN`,
`priest`), `crates/game-server` (`server.rs`, `server/items.rs`, `server/admin.rs`, `zone.rs`, `item_use.rs`, `creation.rs`, `guild.rs`,
`market.rs`, both stores) and migration `0026_rename.sql`; live test section 41. These small choices were made in the build:

- **The kick hold and the stay of a lost connection.** A connection that ends in the world normally leaves its character there for
  `GAME_LINGER_SECS` (10 s) with the socket shut at once; that would have closed the socket before the notice was read and kept the
  character in the world. A held kick or ban therefore skips the stay: the character leaves at once (as section 1 asks) and only the socket
  is held. The hold counts from the notice, so the box comes about 3 s after the chat line whatever the save takes. A newer login and an
  `/akick` of an account on the character screen (`op` 0) close at once as before.
- **`/town`** is taken only while the character is placed in its zone (as every world packet here); the pending move is the item moves'
  slot with no item, so a cancel drops it silently and the due move needs no item check.
- **The Priest** sends no `0x0213`: the original only saves the gold (`SendDBMoney`). If the client's gold does not drop by 10 after a
  reading, the live test says so (section 41).
- **`0x0559` kinds 1 and 2**: the range is the skill code's for its target (the weapon's range plus the skill's `attack_range`, with the
  move slack); a hidden player is refused (`may_attack_player`); the hit pays no SP or MP (as `AttackUser`/`AttackMob` outside `UseSkill`)
  and wears no weapon on a critical; a hit reflected back that kills the user ends in its death as for a skill.
- **The rename** also refuses a name still reserved after a recent deletion (above level 10, 7 days), as creation does, and a name that
  differs from the current one only in case is allowed. The answer to every refusal is `0x010E 2`, logged with the reason.
- **The migrations** are compiled into both servers: a `build.rs` in each now rebuilds them when `db/migrations` changes (before, a new
  migration file alone did not rebuild them, and a stale login server would refuse to start against a database that has 0026).

