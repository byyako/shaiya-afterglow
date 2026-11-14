# Opportunities the client already supports

This page is part of the Shaiya Afterglow tab. It lists what a server can add for the US EP4.5 `game.exe` (patch 108) with **no client change**, because the client already reads or sends the packet, and what a **client patch** would unlock that a server could then use. It is for server authors. The client facts behind every row are on the Client tab; the status words are explained on the [overview of this section](index.md).

## In short

- **The client handles packets the original server never sends.** Sending them adds correct monster health bars, a "logged in elsewhere" message, item-mall names and prices from a database, and a free skill reset that counts down on screen.
- **One mismatch needs fixing in any server built on the original's behaviour.** The original answers lapis extraction with 34 bytes; this client reads 33.
- **Some switched-off features pair with a server change.** A client patch that flips a region feature lets a server use selectable extraction, a target HP percentage and more; the pairing table says what each needs.
- **Warehouse gold looks possible but is not** without a client change: the client never creates the dialog that asks.

### Using this page

- **If you write a server:** take the numbered rows in order of value; each says which packet, its size and fields, the client function that reads it, and the risk.
- **If you patch the client:** the features themselves are described on [Region features](../region-features.md), with their callers and what they need.
- **To check a claim yourself:** the addresses are in the evidence column; the method is on [Tools and method](../tools-and-method.md).

## Opportunities

Ranked by value. Each needs only server work; "risk" is what could go wrong for players.

| # | what players get | packets | evidence | risk |
|---|---|---|---|---|
| 1 | **Warehouse gold**: deposit and withdraw gold in the warehouse window. **Not reachable without a client change.** The request and the answer with messages 236/237 exist, but the number dialog that sends `0x020D` (window `+0x720`, direction `+0x724`) is **never created**: every store to `+0x720` in `.text` writes 0 and nothing writes `+0x724` (confirmed, byte scan of all `mov [reg+0x720]` / `+0x724`; the item-count dialog `+0x600` is created at `0x49ca73`/`0x4fc3f5`/`0x4fcd75`, so the method finds real ones). The S→C answer still works if a server sent it, but nothing in the client asks | C→S `0x020D` 7 (`u8 dir`: 0 withdraw, 1 deposit; `u32 amount`), from both warehouse windows (`0x49d210`, `0x4fd530`). S→C `0x020D` 7 (`u8 dir, u32 amount`): 0 → money + amount, warehouse money − amount, "Found <v>G in warehouse" (236); 1 → the reverse, "Stored <v>G in warehouse" (237) (`0x54d6e0`). The warehouse money already travels in `0x0711` (`TP_CHAR_BANK.dwMoney`) | **confirmed** (client); the original has **no `0x020D` case** (`PacketMaininterface`), and deposit/withdraw does not exist on the server ([Warehouse](../../original/warehouse.md)) | the client changes both totals itself from the answer: answer only after the server has moved the money, with the amount actually moved; refuse overflow (u32) on either side. Whether the button shows for region 6: **live** |
| 2 | **Correct monster HP bars** from the first frame, including Kimuraku (835, client file max 115,438 vs database 1,115,438) | S→C `0x0608` 14 (`u32 id, u32 maxHP, u32 HP`) after `0x0601`, and whenever HP changes if wanted (`0x54eff0`: mob `+0x54` max, `+0x58` HP) | **confirmed**; the original never sends it (no instruction uses `0x608`): status **client-only** (see [Client-ready extras](../../original/client-extras.md)) | none known; `0x0601` alone leaves current HP 0 (bar empty) until an update |
| 3 | **Item-mall names and prices from the database** (no `Cash.SData` edit): the client asks for the product list at the end of a scene set-up (`0xB106` 2, region feature 10 on, `0x426724`; seen after entering the world, see [Entering the world](../../original/enter-world.md)) and nobody answers | S→C `0xB106`: `u8 n`, n × 76 (`char code[21], char name[51], u32 price`). For each record whose code matches a product already in the client's list (`0x84cea0`, 148-byte entries), the client replaces the name (≤ 50 characters) and a `u32` at entry `+0xc` (*inferred* the price) (`0x55a000`) | **confirmed** (client); the original has no `0xB106` at all (see the [game server](../../original/ps_game.md#54-corrections-to-earlier-notes)); PDB struct `TP_BILLING_PRODUCTLIST` 9731 = 3 + 128 × 76 | only products the client already lists can change; codes must match exactly (case-sensitive compare); ≤ 128 records per packet (PDB size). New products still need `Cash.SData` |
| 4 | **Creation modes per account**: the second byte of `0x0109` (`byMaxGrow`) enables the mode buttons on the creation screen: **< 2 disables Hard, < 3 disables Ultimate** (`0x466be0`, `create_mode_button.tga`) | S→C `0x0109` 4 (`u8 country, u8 maxGrow`), before the creation screen opens | **confirmed** (client); the original sends it from `CUser::PacketUserDBChar` with the database's value (**ref** [game server](../../original/ps_game.md#61-what-the-client-receives-after-the-handshake): "`0x0109`, if `byWithCountry`"), and in the shipped database that value is always 3 | server-side rule needed (the server must also refuse a mode above the account's maximum in `0x0102`) |
| 5 | **Free skill reset counted down** on the NPC-13 menu | S→C `0x0215` with the optional 6th byte ≠ 0 (`0x580740`, `0x5528a0`) | **confirmed** ([client facts](../CLIENT-FACTS.md)) | none; 5 bytes keeps "1 chances left" on screen (original behaviour) |
| 6 | **"Duplicate login" message** instead of a silent disconnect | S→C `0x0004` 3 (`u8`): 0 → message box 10070 "disconnect due to using logged in ID.", else 10071 "system error" (a Windows box "Message", or the in-game box when network object `+0x370` is set); then the client posts `WM_CLOSE` to itself and quits (`0x54c860`) | **confirmed** (client). Who sends it originally is not settled (the opcode-store scan cannot tell for the value 4; *inferred* the session server's duplicate-login kick) | send it just before closing the older connection |
| 7 | **"Server busy / maintenance" answers** for a portal to another server | S→C `0x0110` 3 (`u8`): 1 → 10096 "server saturated. Please connect later", 2 → 10059 "checking server. Please check notice board" (`0x54c9b0`); the original sends `0x0110 02` when a confirmed portal (`0x0227`, dialog 0x17) leads to map 200 and the other server is down (`PacketMaininterface` case `0x227`) | **confirmed** (both) | value 0 starts a world change on the client: use 1/2 only as messages |
| 8 | **Old lists the client still understands** (no player gain, only freedom): `0x0D0F`/`0x0D10` 50-byte guild list, `0x0D15` remove member, `0x1D11` "You get <i>." with gold (legacy mail), `0x2609` clear several item-mall storage slots at once | see the tables | **confirmed** (client), never sent by the original | none; no new feature |
| 9 | **Extra trailing bytes are ignored** by readers that stop early: `0x0801` (reads 18, the original's 20 are fine), `0x0105` (guild-name tail read up to 25). Not a feature, but it lets one builder serve both clients | — | **confirmed** | do not rely on it for readers that loop on a count |

Not an opportunity here (checked): the **web inventory** (`0x2701`–`0x2703`, 36 slots moved with `0x0204` bag 200, window `0x4a36d0`)
needs `data/interface/web_inven.tga`, which is in neither archive (the original 108 archive or the repacked one), and its window opens as dialog mode 0x18
from a side tab on the inventory window that exists **only for region 4** (Japan; four region checks, its three tab textures also missing;
see [Open questions](../packet-opportunities.md#open-questions)). So it needs a client patch on those checks, four textures and a server that sends `0x2701`.
**`0xB104`** ("3 hour-free service") is shown only with feature 28 (off): dead here. **Mail (`1Dxx`)** is a leftover "mail-box account"
result set (7143–7166) with no request side in this client.

Systems the original runs with the most visible player value (all **ref**/confirmed in the tables): personal shop (`23xx`, client UI
complete; a "Market Anywhere" client patch already exists), duels (`24xx`), guild battles (`25xx`), union parties, the guild house, NPC guards
(`0E05`/`0E06` from `CNpc::Attack`), mob speech (`1109`–`110B` from mob Lua/AI), obelisks and roads (`21xx`), the item mall (`26xx`).

## The extraction answer

The client reads the lapis-extraction answer at 33 bytes, one fewer than the original server sends (34). The facts (`0x57cb40`, the effect of the extra byte) are on [Every opcode against the original server](../packet-opportunities.md#the-extraction-answer). The advice that follows from them:

- **Send 33 bytes to this client**: drop the `gemPos` byte from the `0x0802` answer. With the 34-byte form the uniform shift leaves lapis 1 to 5 in the right places, but the gold shown becomes `money × 256` (wrapped) plus the sixth count byte until the next money update, and a sixth lapis would be written at bag 255 (outside the client's bag table, no bound check in `0x558510`).
- **Or flip region feature 0** (see below): then the client sends the 8-byte request and reads the 34-byte answer, and the original server's behaviour matches as it is.

## Region-feature patches a server can pair with

A client patch that flips a region feature ([Region features](../region-features.md)) changes what the client sends or reads. These are the features a server would pair with. Each needs one **live** try before a server relies on it.

| Feature | Patch | What a server does with it |
|---|---|---|
| **0** | selectable lapis extraction | Nothing new: the original already handles the 8-byte request (`ItemGemRemovePos`) and always answers 34 bytes, so client and server agree as they are with the feature on; with it off, send 33 bytes (see above). A server must implement the 8-byte request, since with the feature on every extraction uses that form. |
| **25, 26** | target HP percentage | Send `0x0608` (opportunity 2): the percentage is computed from the bar's maximum. Without it the bar's maximum is the data file's value, which is wrong for Kimuraku. |
| **27** | click any player to attack | Decide who may be hit: the client stops checking faction, duel and guild war. Keep refusing illegal `0x0502` attacks. |
| **28** | "3 hour-free service" | Send `0xB104` if a free-service timer is wanted; the client shows the time left. |
| **29** | second login step (security code) | The login server sends `0xA104` (the code positions) and answers `0xA105`; harmless without `0xA104` except for the result-4 text. |
| **9** | 33-byte password field in `0xA102` | The login server must accept the 67-byte `0xA102`. |
| **46** | auto-dismount | Nothing new: the original already answers `0x0216`. |
| **55** | doubled weapon durability at load | Do not flip it unless the server uses the same doubled values: the two would disagree. |
| **58** | doubled guild-house upgrade costs (display and client pre-check) | The server charges the real cost (`0x0D25` carries none). Keep the two tables in step if flipped. |
| **6** | warehouse limited to 40 slots | Do not flip: it reduces the warehouse unless "Double Warehouse" is held. |
| 5, 8, 23, 45, 62 | small quality-of-life features | No server work; display and input only. |
| 43 | quick slots | Open: whether its two default quick slots reach the server, or only the client's copy of the bar (see the open questions below). |
| 24, 52 | on-screen login keyboard, extra login checkboxes | No server work. |

## Open questions for a server

- **Live** (nothing was run): `0xB106` changing a mall entry; `0x0004`'s message box; the "server busy" and "duplicate login" messages (opportunities 6 and 7) for region 6; each region feature flipped one at a time (the checks are listed under [Open on Region features](../region-features.md#open)).
- Whether region feature 43's two default quick slots reach the server, or only the client's copy of the bar.
- The client-side open points (who sends `0x0004` originally, the opcode-store scan's limits, the GM table) stay on [Every opcode against the original server](../packet-opportunities.md#open-questions).
