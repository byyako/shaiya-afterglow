# Trade between two players

This page covers player-to-player trade in the EP4.5 game server `ps_game.exe` (built 2009-07-15): the request, the window (eight item
slots and money on each side), ready, the exchange, everything that ends a trade, and the known duplication holes. It was read in the
disassembly with the PDB names (`tools/disasm.py`). The client's side (what `game.exe` does with each `0x0Axx` packet, its yes/no answer convention, the
result messages) is on [Social packets](../client/client-social-packets.md).

Not covered here: the duel stakes window (`CExchangePvP`, `0x2409` to `0x240C`; see [Duels](duel.md)), personal shops
(`stMyShop`, which every trade packet closes first; see [Personal shops](personal-shop.md)), and game logs
(`0x0803`/`0x0804`/`0x0405`/`0x0406` to the game log server; [The game log](game-log.md)).

## In short

- **Asking.** A player asks another of the same faction on the same map (the new server can allow other factions on chosen maps, section 5). The other has 30 seconds to say yes or no.
- **The window.** Each side offers up to eight items and some money. Any change to an offer cancels both "ready" marks, so nobody can
  agree to something that changed after they looked.
- **The exchange** happens when both have pressed ready, if both have the money they offered and enough empty slots for what they get.
- **Walking away ends it.** Moving more than 10 units from the partner, dying, moving or using an item, or leaving the map cancels the
  trade.
- **Quest items cannot be traded; bound items only by the client's say-so.** The original server checks quest items itself but leaves
  bound items to the client, so a modified client can trade them. The new server refuses them.
- **The original does not write the exchange to the database in one piece.** Money and each item are saved separately, so a crash
  halfway through can duplicate items. The new server writes the whole exchange in one transaction (section 5).

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's (see [NPC shops](shop.md) on the PDB's 4-byte shift).

## 1. The state

Confirmed, `CExchange` at `CUser` +0x1568, `CExchange::Reset` 0x449660.

| field | offset | |
|---|---|---|
| state | +0x1568 | **0** none, **1** asked someone (requester), **2** asked by someone, **3** window open, **4** exchanging |
| partner | +0x156C | `CUser*` of the other side |
| money | +0x1570 | the money offered |
| slots | +0x1574 | 8 × `{u8 bag, u8 slot, u8 count}`; bag 0 = empty |
| ready | +0x158C | `u8`, 1 once ready was pressed |
| request deadline | +0x5830 | `GetTickCount() + 30000`, set on the requester only |

`CExchange::Reset` clears all of it (not the deadline).

## 2. The packets

Confirmed, `CUser::PacketExchange` 0x46ec90.

Every trade packet first closes the player's personal shop (`stMyShop::Ended`). The handler takes `0x0A01` to `0x0A08`; anything else
in the family closes the connection (`SConnection::Close(9)`); `0x0A03` and `0x0A04` from the client are ignored.

Server → client: `0x0A01` (6) `u32 id` request; `0x0A03` (7) `u32 id, u8 result` start; `0x0A04` (3) `u8 result` end; `0x0A05` (4)
`u8 who, u8 not_ready`; `0x0A06` (6) `u8 bag, slot, count, trade slot` (echo); `0x0A07` (4) `u8 who, trade slot`; `0x0A08` (7) `u8 who,
u32 money`; `0x0A09` (35) `u8 trade slot, type, type id, count, u16 quality, u8 gems[6], char craft_name[21]`. **who: 1 the receiver
itself, 2 its partner, 3 both.** Client → server: `0x0A01` `u32 id`; `0x0A02` `u8` (0 yes, 1 no); `0x0A05` `u8` (0 ready, 1 not
ready, 2 cancel); `0x0A06` `u8 bag, slot, count, trade slot`; `0x0A07` `u8 trade slot`; `0x0A08` `u32 money`.

### 2.1 Request, `0x0A01`

1. The requester's state is not 0 → `0x0A03 (target id from the packet, 2)` to it.
2. The target is looked up **in the requester's zone** (`CZone::FindUser`, the same map); not there → nothing.
3. The target's state is not 0, **or its country differs** (`+0x12D`) → `0x0A03 (target id, 2)` ("Transaction in process with another
   user").
4. Otherwise both are linked, requester state 1, target state 2, the target gets `0x0A01 (requester id)`, the requester's deadline is now
   + 30 s.

**No distance check, no check that either is alive** (the client checks what it does before sending). Requesting oneself links the
player to itself and is undone by its own answer (step 2.2 fails).

### 2.2 Answer, `0x0A02`

Only in state 2 with a partner in state 1; otherwise the answerer's state is cleared, silently.
- **0 (yes)**: both state 3; the answerer gets `0x0A03 (requester id, 0)`, the requester `0x0A03 (answerer id, 0)`: both windows open.
- **anything else (no)**: both cleared; the requester gets `0x0A03 (its own id, 1)` ("The opponent refused transaction"). The id is
  the requester's own (`[esi+0xDC]`, esi = the requester), not the answerer's.

### 2.3 Offering an item, `0x0A06` (state 3)

Silently ignored unless: count ≠ 0; trade slot < 8 and empty; **bag 1 to the bag count** (`+0x1304`, so never the equipment) and slot
< 24; an item there; **its real type is not 27** (item types 27, 28, 29: quest items); its count ≥ the count offered; that bag and slot
not already offered in another trade slot; a partner. Then **ready is cancelled** (below), the slot is stored, the offerer gets
`0x0A06` (its request echoed) and the partner `0x0A09` with the item's type, type id, **the count offered**, quality, gems and craft
name.

The client also refuses, before sending: items its `Item.SData` marks not tradeable ("Not a tradeable item", 443) and item types 27,
28, 29 and 99 ("Cannot trade the quest items", 231). **The tradeable mark is `stItemInfo.byOG` (+0x31), the `ReqOg` column
(`defs.items.reqog`)**: 0 tradeable; the EP6 rewrite names 1 account bound and 2 character bound. 1,013 of the 16,047 item definitions
have 1, none 2. The EP4.5 server reads it elsewhere (`CUser::PacketMarket` 0x47792b refuses to list such items, `CUser::ItemDropByUser`
0x45a351 never drops them, `PacketMaininterface` 0x468612 refuses them together with real type 27) **but not in `PacketExchange`**: only
the client keeps bound items out of a trade, so a modified client could trade them.

### 2.4 Taking an item back, `0x0A07` (state 3)

Trade slot < 8 and filled, a partner: ready cancelled, the slot emptied, `0x0A07 (1, slot)` to the player, `(2, slot)` to the partner.

### 2.5 Money, `0x0A08` (state 3)

The amount ≤ the player's money (`+0x140`), a partner: ready cancelled, stored (it replaces the previous amount; 0 takes it back),
`0x0A08 (1, money)` to the player, `(2, money)` to the partner.

### 2.6 Ready, `0x0A05` (state 3, a partner)

- **2 (cancel; the client sends it when the window closes)**: both cleared; `0x0A04 2` ("Transaction has been canceled") to the player,
  `0x0A04 3` ("trade has been cancelled by the other player") to the partner.
- **1 (not ready)**: ready cancelled.
- **0 (ready)**: the player's ready flag set; `0x0A05 (1, 0)` to it, `(2, 0)` to the partner. **If the partner is ready too**: both state
  4, `CUser::Exchange(partner)` with the player who pressed last as `this` (section 3), then both cleared.

**Ready cancelled** (`CUser::ExchangeCancleReady` 0x46f450): if either side is ready, both flags are cleared and both get `0x0A05 (3, 1)`.
Any change to the offer (item, item back, money) does this first, so a change is always seen before the other side can agree to it.

## 3. The exchange

Confirmed, `CUser::Exchange` 0x46f4a0, `ExchangeItemAddEnable` 0x46f860, `ExchangeItemAdd` 0x46f910.

`this` = L, the player who pressed ready last; P its partner. Checks in order, each failure sending `0x0A04` to both and ending:

| check | L gets | P gets |
|---|---|---|
| P offers money and has less than it, or L's money + P's offer passes 2³² − 1 | 6 | 7 |
| L offers money and has less than it, or P's money + L's offer passes 2³² − 1 | 7 | 6 |
| L has fewer empty slots than P offers items | 4 | 5 |
| P has fewer empty slots than L offers items | 5 | 4 |

(6 "Exceeded max. gold possessable", 7 "The player you are trading with has exceeded the maximum amount of gold possible", 4 "Insufficient
inventory for a transaction", 5 "The other player has insufficient inventory space".) Empty slots are counted in bags 1 to the bag
count, before anything moves; items do not stack onto what the receiver has.

Then: L's money goes to P, P's to L (`SendDBMoney` for both; **no money packet: the client adds "partner's offer − mine" itself on
`0x0A04 0`**). Then **P's items to L, then L's items to P** (`ExchangeItemAdd`), each trade slot in order 0 to 7, skipping slots whose
item is gone:
- the receiver's **first empty slot**, bag by bag from 1, slot by slot; none → the item stays (cannot happen after the check);
- **the whole item moves** (the same item object, its unique id kept) when the count offered is all of it; otherwise **a new item** with
  the count offered is made for the receiver (a new unique id, made now, `Maketype` 'T' = 0x54, quality 0, no gems, the craft name
  "0000…" when the definition allows crafting) and the giver's count goes down;
- **the giver gets `0x0206`** (7 bytes, `TP_ITEM_DROP`): bag, slot, then type, type id and count left when part stayed, all three 0
  when the whole item went;
- **the receiver gets `0x0205`** (36 bytes, `TP_ITEM_GET`): its bag and slot, type, type id, the count, and **the giver's item's**
  quality, gems and craft name ("Attained <i>"). For a split item the client is shown the giver's values, not the new item's zeros.

Last, `0x0A04 0` to L, then to P ("Transaction has been completed successfully"). The database is told item by item (`0x0701` insert,
`0x0702` change) and money separately: a crash in the middle leaves a half-done trade.

## 4. What ends a trade

Confirmed.

**`CUser::CancleActionExc`** (0x44b3d0), by state:
- 1 (asking): the requester is cleared, silently. Its partner is cleared only if *its* state is 1, which it never is (it is 2): **the
  asked player stays "busy" until it answers** (its answer then fails the check in section 2.2 and clears it).
- 2 (asked): both cleared; the requester gets `0x0A03 (asked player's id, 1)` (refused).
- 3 or 4: both cleared; `0x0A04 2` to the player, `0x0A04 3` to the partner.

Called on: **death** (`CUser::Death`); **moving an item** (`0x0204` with two different places), **picking up** (`0x0205`) and the third
item request of `PacketMaininterface` (`0x0206`, dropping); **buying and selling at an NPC shop** (`PacketShop`, after the count checks);
item use (three `PacketPC` item requests), gems, quests, the market and personal shops. **Leaving the zone**
(`CUser::LeaveZone`: map change, logout, lost connection) does the same.

**A step** (`0x0501`, `PacketPC` 0x46a5d1): when the player is in any trade state with a partner and **the two are more than 10 apart**
(`SVector::Distance2`, x and z), both are cleared, `0x0A04 2` to the mover, `0x0A04 3` to the partner (in states 1 and 2 too).

**The request deadline** (`CUser::UpdateTimeover` 0x48e310), on the requester: past it in state 1 the requester is cleared silently,
the partner not (same state-1 check as above); past it in state 2 (only reachable on a player who requested itself) both.

An item leaving an offered slot by other means (`CUser::ExchangeItemRemove` 0x46f310: bank, guild bank, market) empties that trade slot
with `0x0A07`; the new server does not need this, because every way it has of moving an item ends the trade first.

## 5. The new server

Follows sections 1 to 4, with these deviations (numbered as the other notes refer to them):

1. **Trade between factions on chosen maps.** The original refuses any. `GAME_CROSS_FACTION_TRADE_MAPS` (comma-separated map ids,
   for example `42`, the Auction House) lists the maps where the country check is skipped. It is off by default, because the original
   allows no such trade (an addition, `docs/server/deviations.md` section 1). Whether the client offers "request transaction" on a player of
   the other faction is not known.
2. **One database transaction for the whole exchange**: both characters' money and every item, all or nothing. The plan is made from
   both players' bags as they are at that moment: the player who pressed ready last checks its own side and waits; its partner checks
   its side, writes everything, and hands the result back. Anything that ends the trade in the meantime (the partner cancelling,
   moving away, leaving) leaves both untouched.
3. **A split item keeps the giver's quality, gems and craft name** (what the original shows the receiver anyway), made now with
   `make_type` 'T'. A whole item keeps its row and unique id, as the original keeps the object.
4. **The request deadline clears both sides**, silently (the original leaves the asked player busy until it answers).
5. **Requesting oneself is refused** with `0x0A03 (id, 2)`.
6. **Bound items (`reqog` not 0) and item type 99 are refused** as the client refuses them, besides types 27 to 29.
7. **An offered item that is gone, another item or smaller than offered when the exchange runs cancels the trade for both**, nothing
   moving (`0x0A04 2` for the side whose bags changed, `3` for its partner, whichever side finds it). The original subtracts past zero.
   The exchange checks type, type id and a count of at least the count offered (`docs/server/audit.md` R3).
8. A buy or sell request ends the trade as soon as it parses (the original: after its count checks).
9. **A trade and a duel stakes window exclude each other** (audit R3, not in the original): a trade request or answer is answered
   busy (`0x0A03 2`) while either side's duel stakes window is open (or its stakes are coming out), and a duel request or answer is
   refused (`0x2402 2`, not possible) while either side trades or has a personal shop. Opening a shop (`0x2301`) closes a stakes window,
   as every trade end does.
10. **No trading while dead** (audit R3): `0x0A01` and `0x0A02` from a dead player are ignored. Death already ends a trade.
11. Every item action that takes an item out of the bags ends the trade first, which now also covers the body change scroll, the Etin
    return, a resurrection rune and the use-up of a move by item (audit R3); a trade that is cancelled by its partner receiving its own
    answer first (audit N6: the parked partner is answered before the exchange's `settle` runs).

## 6. Duplication exploits and how each build answers them

The community knows of trade dupes in the original files without a precise write-up ([Known bugs and community fixes](community-fixes.md)).
What the references and the code above show. **Patch kit** is the original `ps_game.exe` with the [patch kit](../server-patches.md);
**new server** is this project's Rust server.

| hole | where | patch kit | new server |
|---|---|---|---|
| **Half-written exchanges.** Money and each item go to the database agent as separate messages (`0x0701`/`0x0702`, `SendDBMoney`); a crash or a lost database connection part-way leaves one side with both copies. | `CUser::ExchangeItemAdd`, `CUser::Exchange` | not patchable: it needs one transaction in the database agent | one transaction for everything (deviation 2) |
| **The offered count shrinking.** The exchange reads bag and slot at the end, not the item offered. When the count offered is not the count there, the split path makes the offered count for the receiver and subtracts it from the giver's byte count without checking it is still there (0x46fb3b): offer 5, let the stack drop to 3, and the receiver gets 5 while the giver keeps 254. The duel's `CUser::ExchangePvP` (0x47cd29) does the same. Most item actions end the trade first (`CancleActionExc`) or empty the offered slot (`ExchangeItemRemove`); no path that lowers an offered count mid-trade was confirmed (a delayed item use, consumed after its own cancel (`UpdateResetPosition`), is the likeliest candidate, not tried). | `ExchangeItemAdd` reads `bag`/`slot` | [`exchange-count-guard`](community-fixes.md#exchange-count-guard) skips such a slot | every item action this server has ends the trade first (section 4), and the exchange checks each item against the copy taken when it was offered (type, type id, enough count) and **cancels the whole trade** when one does not match (deviation 7) |
| **Both pressing ready at once.** The ready flags are read and set without a lock. Not reachable as far as the code shows: the network threads only queue packets (`SSyncQueueBuffer` at `CUser+0x7c`) and the zone's packets are handled one after another on its world thread (`CWorldThread::Update` → `CZone::UpdatePackets` → `CUser::UpdatePackets` → `OnRecv`); two traders are always in one zone (that a zone belongs to one world thread is inferred). | `PacketExchange` case `0x0A05` | not needed | the step to "exchanging" is taken under one lock, once |
| **Bound items.** `PacketExchange` does not check `byOG`; only the client does. | section 2.3 | fixed by the kit ([Bound items can be traded](community-fixes.md#bound-trade)) | refused on the server (deviation 6) |
| **Trading with oneself.** The shaiya-episode-6 project (shaiya-episode-6, github.com/kurtekat/shaiya-episode-6, `handler_0xA0A`) closes the connection of a player whose partner is itself. In EP4.5 the self link is undone at the answer (section 2.2). | `PacketExchange` case `0x0A01` | unchanged | refused (deviation 5) |
| **Personal shop and trade together.** The same EP6 code refuses its confirm while either side has a personal shop open; EP4.5 closes the shop on every trade packet. | `stMyShop::Ended` | unchanged | every trade packet ends the shop and every shop packet ends the trade ([Personal shops](personal-shop.md)), so an item is never in both |
| **The EP5+ confirm step** (`0x0A0A`): `ps_gametablas.CT` "trade exploit fix" sends `0x0A0A` 1/2 to both after a change so that a confirmation does not survive it. It is for a later build (its `SConnection::Send` is at 0x4ED0E0, EP4.5's 0x4D4D20). | not in EP4.5 | not needed | not needed: the ready flag is cleared by every change (section 2.6) |
