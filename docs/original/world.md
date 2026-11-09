# In the world: movement, sitting, leaving and chat

This page covers what a player does once in the world: moving, sitting and standing, logging out or losing the connection, how the zone
decides who sees whom, asking about another player, and chat. It is about the EP4.5 game server `ps_game.exe` (built 2009-07-15) and
database agent `ps_dbAgent.exe`, and the US client `game.exe` (built 2010-04-26, patch 108). The analysis started from the packets the
real client sent; everything else is static analysis, nothing was executed.

## In short

- **Moving.** The client tells the server where the character goes. A step longer than 35 units, or one faster than the character's
  speed allows, is refused, and the server puts the character back where it was.
- **Logging out takes 10 seconds, sitting.** The exit dialog first makes the character sit. Only then does the client ask to leave, and
  the server waits 10 seconds; standing up or moving cancels it.
- **A lost connection does not save you.** If the connection drops while the character is fighting, the character stays in the world
  for 10 more seconds: monsters keep attacking it and it can die. A quick relog fails until it has gone.
- **Who sees whom.** A map is divided into squares of 40 × 40 units (cells). A player sees the 5 × 5 block of cells around them. Most
  actions are sent to the players within 80 units.
- **Chat.** Normal chat reaches players within 30 units, shout within 100 (once every 30 seconds), trade chat the whole faction on the
  server. Whispers only reach players of the same faction. The US client limits trade chat to level 10 and one message every 5 seconds,
  but the original server does not check this.

## 1. Dispatch in the world

Confirmed, `CUser::PacketProcessing` state 4, 0x466fbb.

Only while the player is in a zone (`m_pZone != 0`). By opcode group (`opcode & 0xFF00`):

| group | handler | notes |
|---|---|---|
| `0x01xx` | inline | `0x010B` save quick slots, `0x0107` and `0x010D` logout requests (section 4); any other `0x01xx` closes the connection (reason 9) |
| `0x02xx` | `CUser::PacketMaininterface` (0x468420) | interface actions (the client sent `0x0209` twice) |
| `0x03xx` | `CUser::PacketGetInfo` (0x469cf0) | |
| `0x05xx` | `CUser::PacketPC` (0x46a230) | `0x0501` move, `0x0506` motion (sections 2, 3); also `0x0502`, `0x0503`, `0x050A`, `0x0511`, `0x0517`, `0x0552` to `0x0559` |
| `0x07xx` | `CUser::PacketShop` | |
| `0x08xx` | `CUser::PacketGem` | |
| `0x09xx` | `CUser::PacketQuest` | |
| `0x0Axx` | `CUser::PacketExchange` | |
| `0x22xx` and others | further handlers (friends `0x22xx`, points `0x26xx`, ...) | not read yet |

An unknown group closes the connection (reason 9).

## 2. Moving, `0x0501`

Confirmed, `CUser::PacketPC` 0x46a286.

Client to server, 17 bytes: `u8 motion` (bit 7 is a flag kept with it), `u16 dir`, `f32 x, y, z`.

1. If the player sits, it stands up (`CUser::StatusStand`), which **cancels a pending logout** (section 4).
2. Certain states (`+0x142c == 1` and `+0x1430 != 1`, not identified) ignore the packet.
3. **Distance check:** the horizontal distance (x and z only, `SVector::Distance2`, a square root) from the stored position to the new one must be
   at most **35.0**. Otherwise the move is refused (below).
4. **Speed check:** `stUserMoveChk::Update` with the speed from `g_ConstUserMoveSpeed[move speed]` (or a vehicle speed). On failure the move is
   refused, using the last position the check accepted (`docs/original/move-check.md`).
5. Accepted: `CZone::MoveUserByPacket` moves the player (cells, view), stores `dir` and the motion byte, stops attacking or casting, and sends
   `0x0501` (21 bytes: `u32 objectId, u8 motion, u16 dir, f32 x, y, z`) **to the other players in view** (`PSendViewMove`), and to the party.
6. Refused: the server sends `0x020B` (20 bytes: `u32 objectId, u16 map, f32 x, y, z` of the position it keeps) to the player and the players in view,
   and `0x0502` (17 bytes: `u8 5, u32 objectId`, then 10 zero bytes) to the player. The client is put back.

## 3. Sitting and standing, `0x0506`

Confirmed, 0x46a659, `CUser::ChangeMotion` 0x48a1a0.

Client to server, 3 bytes: `u8 motion`. `ChangeMotion` cancels a pending logout, then accepts **0 (stand) and 1 (sit)** and stores it
(`m_byMotion`, `+0x1434` in the code). The answer is `0x0506` (7 bytes: `u32 objectId, u8 motion`, the requested value), sent to everyone in view when
accepted (`CObject::SendView`); any other value, such as the client's emotes (`/victory` to `/insult`, motions 116 to 125, `docs/client`), is not
stored but sent to the players within the action range of 80, the player included (`CObjectConnection::PSendViewAction`, 0x46a6aa).

## 4. Leaving the world

Confirmed.

### 4.1 The client (game.exe)

The exit dialog (`0x4ef0b6`) first **makes the character sit**: if the player is not sitting it sends `0x0506` with motion 1. It sends the logout
request only once the player sits, i.e. after the server's `0x0506` reply: **`0x0107`** to go back to the character screen, **`0x010D`** to quit
(`0x4ef157`). It counts down on its own. Closing the window sends `0x0107` and closes the connection (`0x409ed1`).

Without the answer exit does not work: if the server does not answer `0x0506`, the character never sits and no request is sent.

### 4.2 The server

- Request (0x4670f2 for `0x0107`, 0x46714f for `0x010D`): accepted only while sitting (`m_byMotion == 1`) and with no logout pending; otherwise ignored.
  It records the type (1 for `0x0107`, 2 for `0x010D`) and a deadline **10 seconds** ahead. Nothing is sent back.
- Standing up or moving cancels it (section 2, 3).
- When the deadline passes (`CZone::UpdateLogout`), `CWorld::LogoutUser` (0x40c860):
  - **Type 1 (character screen):** saves the character, removes it from the zone and the world, sets state 1, tells the database agent `0x304`, and sends
    `0x0107` (2 bytes). It is queued while XOR is still on, so it is XORed; right after, the server switches back to AES (`m_bInitGame = 0`), with the
    send counter where it stopped. The client's `0x0107` handler switches its side back to AES too. The database agent saves the character and sends the
    list again **with the faction** (`ps_dbAgent` `CUser::OnDispatchDB` → `LoadCharacterListWithCountry` with the flag set), so the client gets `0x0109`
    and the `0x0101` records, as after the handshake.
  - **Type 2 (quit):** sends `0x010D` (2 bytes); the client then closes. (With the reconnect feature it sends `0x0111` with a server address and key
    instead; not used.)
- The client's `0x010B` in the world saves its quick slots (`u8 count < 128`, then `u8 bar, u8 slot, u8 bag, u16 number` each).

The flag for the first list is also settled by this: both callers of `LoadCharacterListWithCountry` in ps_dbAgent pass it set, so `0x0109` comes before
every list sent after a load.

### 4.3 Losing the connection

Confirmed, `CWorld::LeaveUser` 0x40ca20, `CUser::KillSoulForDisconnect` 0x459380.

A closed client connection goes through `CWorld::LeaveUser` (called from `CServer::OnDisconnect` 0x406430; a close by the server itself, such as
reason 9, inferred to end there too), by the connection's state (`m_nWhere`, `+0x5758`: 0 none, 1 character screen, 4 in the world; jump table
0x40cccc). In the world (state 4; states 2 and 3 are much the same, 3 also ends a mini game):

1. A **dead** player (`m_nStatus` 1) comes back to life on paper: `KillSoulForDisconnect`.
2. Unless the database agent closed it (`m_bCloseByDBAgent`):
   - **no logout pending** (`m_nLogoutType` 0): logout type 2 with a deadline **10 s** ahead, and nothing else. **The character stays in the zone**:
     monsters go on attacking it, its own attack goes on, it can die. When the deadline passes, `CWorld::LogoutUser` type 2 finds the socket closed
     (`+0x68` = −1) and calls `LeaveUser` again, which now saves the character and tells the database agent it left (below);
   - **a logout already pending** (type 1 or 2, from section 4.2): saved at once (`SendDBAgentCharGetInfo`) and the database agent told
     (`SendDBAgentDisconnect`); the deadline is not waited for.
3. The character leaves the zone and the world when the database agent answers (`CWorld::LeaveUserDBAgent`, which also calls
   `KillSoulForDisconnect` for a player that died meanwhile).

Until then **the user is still in this world**, so a new `0xA301` for the same account is refused (`docs/original/ps_game.md` section 2:
`CWorld::FindUserBill`): a quick relog fails until the old character is gone.

**`CUser::KillSoulForDisconnect`**, for a dead player whose `m_nStatusLife` is not 2 (game log `0x305`):

- Ultimate mode (`grow` 3) unless player-killed (`m_nDropExp` −1): with a rebirth skill or a resurrection item, half HP, SP and MP at the bind position;
  without, the experience drop and **the character is deleted** (`0x0406`), as in `CUser::KillSoul` (`docs/original/mob-fight.md` section 8).
- Otherwise: **`CUser::ExpDrop(m_nDropExp)`** (5 %), **HP, SP and MP at their maxima**, and the position (and map) becomes the **bind position**
  (`CMap::GetBindPos` of the current map; outside a zone, of the saved map). The body is removed (`CWorld::LeaveDead`) and `m_nStatusLife` = 2. Nothing
  is sent and the player is not moved in the zone: the values are what gets saved, so the character logs in again alive at the bind position.

The character-screen logout (`LogoutUser` type 1) calls it too, except in ultimate mode.

## 5. The zone: cells, view and what players see of each other

Confirmed.

### 5.1 Cells

`CZone::Create` (0x415d50) divides a map into **cells of 40 × 40** (x and z). Cell `(i, j)` holds x in `[40i, 40i + 40)`, z in `[40j, 40j + 40)`,
but each cell also stores wider **inner bounds**, `[40i - 10, 40i + 50]` and `[40j - 10, 40j + 50]`. On a move, the player keeps its cell while the new
position is inside those bounds (`CCell::IsInnerCellView`); otherwise the cell becomes `(trunc(x / 40), trunc(z / 40))`. The 10-unit margin keeps players
on a border from switching back and forth. A position outside the map's grid closes the connection (reason 9).

**View:** the cells within 2 in both directions (a 5 × 5 block, clamped to the grid). `CZone::SendView` sends to every player in those cells, the player
itself included. **Range sends** (`CZone::PSendView`, `CCell::PSendRange`) go to the players within a horizontal distance of the position, always
including the acting player: 80 for moves and actions, 60 for speed. Every fourth move is also sent to the ring from 80 to 120 (`PSendViewMove`).

### 5.2 Entering the zone (`CZone::EnterUser` 0x415f80)

In this order:
1. `0x0201` `TP_USER_ENTER` of the player to everyone in view, itself included, with `byNew` = 1 (players; 2 for staff).
2. `0x051D` shape, only for transformed or shaped characters (`CUser::SendShape`; `0x0524` for one special shape).
3. `0x051C` speed to the players within 60 (`CUser::SendSpeed`).
4. To the player, for every cell in view (`CZone::SendCellInfoView` → `CCell::SendCellInfoEnter` 0x423aa0): each player there gets `0x0201`
   (`byNew` = 0 for players, 2 for staff), `0x051D` if shaped, `0x051C`; then the cell's other lists in this order: `0x0405`, mobs (`0x0601`), NPCs
   (`0x0E01`), items on the ground (`0x0401`), `0x0403` (`docs/original/maps-npcs.md` section 5). The loop does not skip the player itself, so it receives its own `0x0201` and `0x051C` again.
5. `0x0451` weather to the player (`CZone::SendWeather`): `u8 0, u8 type, u8 power`; `00 00 00` when the zone has no weather.
6. `0x2101` the map's obelisks and portal links (`CMap::SendObeliskList`): `u8 count`, 13 bytes each (`u32 id, u8 state, f32 x, f32 z`), up to 100 per
   packet; nothing when the map has none.
7. `CUser::EnterZone`: server bookkeeping (movement check start, GameGuard), no packet.

### 5.3 Moving between cells (`CZone::MoveUserByPacket` 0x416550, `CZone::SendMoveUser` 0x41f350)

When the cell changes, for the cells that leave the view: those players get `0x0202` (`u32 id`, 6 bytes) of the mover, and the mover gets `0x0202` of each
of them (`CCell::SendCellInfoLeave`; then `0x0406`, mobs `0x0602`, NPCs `0x0E02`, items `0x0402`). For the cells that enter the view: those players get the mover's `0x0201`
(`byNew` 0) and `0x051C`, and the mover gets theirs (`SendCellInfoEnter`). Then the move itself goes out as `0x0501` (section 2).

### 5.4 Leaving the zone (`CZone::LeaveUser` 0x4161b0)

`0x0202` (`u32 id`) to everyone in view.

### 5.5 Asking about a player: `0x0301` and `0x0303`

Confirmed, `CUser::PacketGetInfo` 0x469cf0, `CZone::SendUserShape` 0x41fd00.

The client draws another player only after asking about it: when a `0x0201` arrives it sends `0x0303` and `0x0301` with the player's id
(`u32`; seen live). The server looks the id up in the requester's zone.

- `0x0301`: if found, `0x0501` with motion `0xC4` (a position sync: `u32 id, u8 0xC4, u16 dir, f32 x, y, z`), then `0x0301` (12 bytes):
  `u32 id, u16 max HP, u16 HP, u8 attack speed, u8 move speed`. If not found, nothing.
- `0x0303`: if found, the same position sync, then the looks (ordinary players; a transformed or cloned player sends other data):

  | offset | field |
  |---|---|
  | 2 | `u32 id` |
  | 6 | `u8` 1 when `m_nStatus == 1` (not identified), else 0 |
  | 7 | `u8 motion` (0 stand, 1 sit) |
  | 8 | country, 9 race, 10 hair, 11 face, 12 size, 13 class, 14 sex, 15 party type (`GetPartyType`), 16 mode (one byte each) |
  | 17 | `u32` first kill counter |
  | 21 | equipment, bag 0 slots 0 to 7, 3 bytes each: type, type id, enchant level (the last two characters of a 20-character craft name when they are digits, else 0) |
  | 45 | `char name[21]` (the last byte forced to 0) |
  | 66 | if slot 7 holds an item: its 6 gem bytes (the cloak's look), and the guild name follows at 72 |
  | 66 or 72 | guild name and its zero; the packet ends there |

  If not found: `0x0202` with the id. The original also sends a personal-shop sign (`0x0523`) after it when the player runs a shop.
- `0x0302` (`u32 id, u16 max HP, u16 HP`), `0x0304`/`0x0305` (mobs) and `0x0307` are in the same handler.

## 6. The new server: the view

- One registry per game server of the players in each map's zone: position, direction, cell, kind (player or staff), speeds, and a way to send them
  packets.
- Enter: as 5.2 steps 1, 3, 4 (players, monsters, NPCs, ground items and obelisks) and 5 (`00 00 00`). The player's
  own `0x0201`/`0x051C` is not sent a second time.
- Move: update the position; cell change with the 10-unit margin; view changes as 5.3; `0x0501` (21 bytes) to the players within 80 (within 120 every
  fourth move), the mover included.
- Sit/stand `0x0506` to everyone in view; a refused move's `0x020B` to the players within 80; leave: `0x0202` to the view.
- Map bounds come from the map data (`world.maps`): a step off the map closes the connection (`World::is_safe_pos`).
- `0x0301` and `0x0303` as 5.5 (the party type and the guild name are filled; the status flag is 0).

## 6a. Chat

Confirmed, `CUser::PacketChatE` 0x470490.

`0x11xx` in states 3 and 4 (`0xF1xx` is staff chat, `CUser::PacketAdminChat`, not read). A muted player (`+0x577c` tick in the future) is ignored. Every
kind carries `u8 len` and `len` bytes of text that include the terminating zero (live: `04 "hey\0"`); `len > 128` **closes the connection**
(reason 9), `len < 2` is ignored. The server copies the text as it is.

| opcode | log name | request after the opcode | sent to | packet sent |
|---|---|---|---|---|
| `0x1101` | NORMAL | `u8 len, text` | players within **30**, the sender included (`PSendViewChat`) | `u32 charId, u8 len, text` |
| `0x1102` | TO (whisper) | `char target[21], u8 len, text` (target's last byte forced to 0) | the target, found by name on the whole server, **only if it has the sender's faction**; then the sender | to the target `u8 0, char sender[21], u8 len, text`; to the sender `u8 1, char target[21], u8 len, text`. Not found or other faction: `0x1106 u8 1` to the sender |
| `0x1103` | TRADE (client: "[Trade]") | `u8 len, text` | everyone of the sender's faction on the server (`CWorld::PSendAllCountry`) | `char sender[21], u8 len, text` |
| `0x1104` | GUILD | `u8 len, text` | the guild | `CGuild::SendAllGuildChat` (not read) |
| `0x1105` | PARTY | `u8 len, text` | the party | `u32 charId, u8 len, text` |
| `0x1107` | SHOUT | `u8 len, text` | players within **100**; only in a zone; once per **30 s** | `u32 charId, u8 len, text` |
| `0x1108` | ALL (client: "[chatting all members]") | `u8 len, text` | everyone on the server; only with a chat item active (`+0x5840`), once per 30 s; the item is used up | `char sender[21], u8 len, text` |
| `0x1111` | ZONE | `u8 len, text` | the sender's faction in the same map's zone (`CZone::SendAllCountry`); only in a zone | `char sender[21], u8 len, text` |
| `0x1112` | PARTY (leader) | `u8 len, text` | the party, only from its leader or sub-leader | `u32 charId, u8 len, text` |

Normal chat with a chat item active also goes out as `0x1108` to everyone. Other `0x11xx` close the connection. Every message is also written to the
chat log and copied to staff who listen to the sender (`SendChatLogA/B`, `SendListenChat`).

**The client side (game.exe, verified).** One send function (`0x586e70`) picks the opcode by kind: 0 normal `0x1101`, 1 `0x1103`, 2 guild `0x1104`,
3 party `0x1105`, 4 shout `0x1107` (staff get `0xF1xx`); whispers (`0x586f50`), guild (`0x586ff0`), `0x1103` (`0x587060`) and area `0x1111` (`0x5870d0`)
have their own. The chat box's channels are 0 normal, 1 whisper, 2 party, 3 guild, **4 trade (`$` prefix) → `0x1103`**, 5 area → `0x1111`. **This client
never sends `0x1108`.** So the US client's trade chat is the server's `0x1103`, sent to the whole faction.

The client limits trade chat by its region setting (`0x477e83`, table at `0x478274`): for the US build (region 6, and most others) **level 10 or more and at
most one message per 5 seconds**; regions 5 and 9 need level 15; region 2 only waits 15 seconds; region 4 has no limit. A blocked message is dropped
without a message (seen live with level 1 characters). The original server does not check either.

The new server: `0x1101`, `0x1102`, `0x1103`, `0x1107` (with its cooldown) and `0x1111` as above; `0x1103` (trade) also
enforces the client's limits on the server, level 10 and 5 seconds by default (`trade_chat_min_level`, `trade_chat_cooldown_secs`), so a modified
client cannot bypass them; `0x1104`, `0x1105`, `0x1108` and `0x1112` are handled with guilds, parties and chat items; staff listening and the chat log are in `docs/original/gm-commands.md` section 8.4 and `docs/original/game-log.md`. Names in
whispers are matched ignoring case (character names are unique ignoring case).

## 7. The new server: movement and leaving

1. Track position, direction and motion in the world; save map, position and direction when the player leaves the world (logout or connection loss).
2. `0x0501`: refuse moves longer than 35.0 horizontally with `0x020B` and `0x0502` as above; otherwise store the position. The broadcast to
   others comes with the zone; the speed check is in `docs/original/move-check.md`.
3. `0x0506`: accept 0 and 1, cancel a pending logout on any change, always answer `0x0506` with the requested motion.
4. `0x0107` / `0x010D` while sitting: logout after 10 s unless the player stands or moves. Then type 1: save, `0x0107` (XOR), back to AES, `0x0109` and
   the list, character screen again; type 2: save, `0x010D`, wait for the client to close.
5. `0x010B`: accepted and logged; quick slots are stored (`docs/original/skills.md` section 7).
6. Log the payload of every packet the world does not handle yet (hex, debug level), so that live sessions can be read afterwards.
7. A connection that ends in the world (closed, lost, idle, or closed by the server for a fault), with the player alive and no logout pending: the
   character **stays in the zone for 10 s** (section 4.3), as if the client were still there with nothing sent to it, then leaves and is saved. Not when
   the server shuts down. **Deviations:** a player who is dead when the connection ends leaves at once (the original keeps the body 10 s, with nothing
   left to happen to it); ultimate mode is treated as the other modes (no deletion), as for rebirth.
8. Leaving the world dead, however it happens (after the 10 s too): the experience drop, HP, SP and MP at their maxima, the bind position (and its map),
   saved; nothing is sent.
9. The handshake refuses an account that still has a connection on this server, the 10 s included (section 4.3; `docs/original/ps_game.md` section 7).

## 8. Open items

- `0x0209` and the rest of `PacketMaininterface`; the friends (`0x22xx`) and points (`0x26xx`) requests the client sends; `0xB106`.

## 9. Other episodes

The EP3 `ps_game.exe` (linked 2008-01-17) and the EP5.4 one (linked 2011-09-14, modified after linking) were compared with the EP4.5
build, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- Moving, the move check, zone entry and the map code are the same: `stUserMoveChk::Update` (EP3 ps_game 0x450b10), `CZone::EnterUser`
  (0x412db0) with the same calls, `CMap::IsSafePos` (0x412190) identical (confirmed). The map object is 0x1c4 bytes against 0x1f0, and the
  data has maps 0 to 44 (confirmed).
- The dispatch groups of this note are the same, with the same opcodes (confirmed, dispatch tables).

### EP5.4

- `0x0501` keeps its 17-byte request and the same accepted tail (view, party, trade distance, PvP leave checks) (EP5.4 ps_game
  0x4784f4; confirmed). Sitting and the logout requests keep their cases.
- **Non-finite positions are refused:** x, y and z must pass `_finite` and not be NaN, else the step is dropped without an answer
  (EP5.4 ps_game 0x478514–0x4785a4); the same test guards `0x0220` (confirmed).
- **The step check is new**: one function replaces the 35-unit step check and `stUserMoveChk`; see [`move-check.md`](move-check.md).
- **Target subscription:** `0x0308`/`0x0309` select a player or monster target and `0x030A` clears it; the server keeps a watcher list
  per player and monster and pushes the target's buffs (`0x030B`–`0x030D`) to those watching. Leaving a zone takes the player off every
  list (EP5.4 ps_game 0x455f20, 0x41c980) (confirmed).
- New `Map.ini` key `MapCurse`: a skill put on one faction entering the zone, no save point, no login inside (EP5.4 ps_game 0x41c899,
  0x477524; confirmed). No shipped file sets it.
- The chat opcodes (`0x11xx`) are the same set (confirmed, dispatch tables).
