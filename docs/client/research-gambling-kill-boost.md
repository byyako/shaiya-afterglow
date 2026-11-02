# Gambling house and a kill-boost item

This page covers two client questions: how the client's gambling house works (which NPC opens it, what each of the
`0x3101`–`0x3109` answers does, which gold values the window trusts, what happens when the player stands up), and whether a new
"kill boost" consumable item and buff can be added without changing `game.exe`. Everything was read statically in the US `game.exe`
(build 2010-04-26, patch 108); nothing was launched. Addresses are VAs; for file-backed `.text`, `.rdata` and `.data` the file offset is the
VA minus `0x400000`. Status words: **confirmed** (decompiled data flow or parsed bytes), *inferred*, **live** (needs a game session).

## In short

- **The client has a complete gambling path for NPC type 5.** Clicking the NPC's primary action opens a table selector; choosing one of
  seven tables sends `0x3101` with a one-byte table index (not the NPC id) and, once accepted, runs a slot scene and a card scene.
- **Nine answers (`0x3101`–`0x3109`) drive the windows.** The payouts, faces and wallet come from the server's fields, not from a client
  calculation; a trailing "short" byte in `0x3103` is a **no-re-bet flag**, and some answers (`0x3105`, `0x3109`) carry nothing the
  client reads.
- **The gambling window shows its own wallet,** copied from the normal gold when it opens; only `0x3102` result 1 and `0x3106` assign it
  from the server. A mismatched wallet is not repaired by the other answers.
- **Standing up reloads the map and sends `0x0201`,** so a server treats it as a normal map entry. The ordinary chat editor and movement
  are absent from the gambling scene.
- **A dedicated kill-boost item is possible by adding data only:** a new consumable in `Item.SData`, a new neutral buff in `Skill.SData`
  (the multiplier is a server-side check on the buff id), and no new opcode. The one catch is a one-byte item field for the skill id.

## The gambling house

### Reaching the window

`0x0e01` (reader `0x57e000`, handler `0x552730`) passes the NPC type and id through without filtering out type 5. NPC construction
(`0x433770`) handles type 5 among cases 3..13, looks up the ordinary NPC definition (`0x479320(type, id)`), copies its name, motion and
model and uses the NPC model pool at `0x6f3fa4`. The interaction branch around `0x441617` saves the type at `0x854eac`, the id at
`0x854ea8` and opens UI state 100 (`0x850a5c`). In the UI dispatcher `0x48c120`, case 100, the primary NPC action (dialog return 2) with
type 5 selects UI state `0x6a`, which builds the table selector (`0x486bf0`; setup `0x485bc0`/`0x4860f0`) from `gamble.TGA`,
`gamble_button.tga` and `cancel_button.tga`. **Confirmed;** whether the model renders is a live question.

The selector update `0x486140` iterates **seven rows**, stores the selected index at selector `+0x2de0`, checks the player's money
against the table at `0x6f2150[index]` and at `0x486266` calls the sender `0x588c00`: **`u16 0x3101, u8 table`**, 3 bytes. It sends neither
the NPC's type id nor an object id. The table holds seven u32 per-line bets, 100 up to 3000 for indexes 0..6. A successful `0x3101` stores
the table at `0x84ceec` and the per-line price at `0x84cef0`, then enters scene 8 (transition) and scene 9 (the gambling scene).

### The nine answers

The network vtable starts at `0x6a5c20`; slots `+0x400..+0x420` point to nine callbacks that stage the answer in globals and set
`0x84cedc = 1..9` in opcode order. The slot window (`0x486e10`) and card window (`0x489260`) consume them; `0x485af0(expected)` waits for a
particular answer and can show a timeout dialog after 30 seconds. Lengths include the opcode and exclude transport framing; offsets start at
the opcode. **Confirmed.**

| Answer | Fields actually read | Window use |
|---|---|---|
| `0x3101` | +2 u8 ok (3 bytes) | Non-zero accepts the selected table; zero restores the controls and shows failure text. |
| `0x3102` | +2 u8 result, +3 u32 bet, +7 u32 money (11) | Result 1 replaces bet and local wallet with the server's fields and derives the active lines as bet / table price. Result 2 clears lines and bet and adds the locally held bet back to the wallet (it does not assign the answer's fields in that branch). Result 0 restores the controls. |
| `0x3103` | +2 u8 ok, +3 u8[9] cells, +12 u8 won, +13 u32 amount, +17 u8 short (18) | With ok non-zero: loads the nine faces, takes `won` and `short` as booleans, `amount` as winnings, and animates the reveal. With ok zero: back to the controls, cells ignored. |
| `0x3104` | +2 u8 dealer (3) | Stores the dealer face and switches from slot to card view; first round = 1, stake = slot winnings. |
| `0x3105` | **no payload read** | Slot collect / play-again acknowledgment: animates crediting the winnings to the local wallet, then uses the previous `short` flag to keep the same re-bet or clear it. |
| `0x3106` | +2 u32 money (6) | Replaces the gambling wallet, clears the bet, requests scene close; the outer scene copies the wallet to the normal global gold and reloads the map. |
| `0x3107` | +2 u8 card, +3 u8 result, +4 u32 amount (8) | Shows the player's face. Result 0 loses and animates the amount as returned gold; non-zero wins and uses the amount as the new stake. The local round counter decides whether it is the fifth and final round; result 2 is not a separate branch. |
| `0x3108` | +2 u8 dealer (3) | Updates the dealer face for the next high/low choice. |
| `0x3109` | **no payload read** | Collect already sent the request and animated wallet += the held stake; the callback only sets answer marker 9. A server's trailing u32 money is ignored. |

The senders for `0x3101..0x3109` are `0x588c00, 0x588c30, 0x588c60, 0x588c90, 0x588cc0, 0x588cf0, 0x588d20, 0x588d50, 0x588d80`; all send
the opcode only (2 bytes) except `0x3101` (table byte) and `0x3107` (guess byte). The card buttons in `0x489260` send **1 for high, 0 for
low**; the collect button calls `0x588d80` (`0x48937e`).

### Cells, faces and the "short" byte

The nine cell bytes are copied in row-major order into the window (`+0xc4` … `+0x144`, 16 bytes apart in three groups) and revealed in
index order 0..8. The slot renderer `0x484d30` indexes the face texture with the received value; it does not recompute a face from
payout odds, and the payout is the server's amount, not a client multiplier. When `won` is true it highlights the matching lines. The line
order matches the original server: middle row, top row, bottom row, the descending then ascending diagonals, then right, middle and left
columns.

`0x488550` loads `data/interface/gamecard/game_card<N>.tga` for N in 0..22 (22 is the back, shown while hidden); the card view loads
`tarocard<N>.tga` through `0x484cd0` with 22 as the back. Values 0..21 are the 22 major-arcana faces in the usual order (0 The Fool … 21 The
World, read from the shipped art) in both sets; a server must keep result faces in 0..21.

The trailing `short` byte is staged at `0x84cee1` and copied to window `+0xa4`. It is a **no-re-bet flag**, not another face or the win
count. After a losing spin, or after collecting a win with `0x3105`, the reset path in `0x486e10` either keeps the previous bet and
deducts it from the local wallet (`short = 0`) or clears all selected lines and the bet (`short != 0`). A server's reservation, refund and
re-bet behaviour must match this flag; changing only the gold balance will not produce a matching UI.

### Which gold values the client trusts

The normal wallet is `0x84d670`. `0x488550` copies it into window `+0x50`. The displayed bet or stake is `+0`, the previous bet `+4`, the
per-line price `+8`, the result winnings `+0x54` and the wallet animation target `+0x58`; helpers `0x486a50`/`0x486b20` animate the
arithmetic, and the renderer draws the window wallet, not the global one.

Only **`0x3102` result 1** (money → window wallet) and **`0x3106`** (money → wallet) assign gold from the server. The other updates derive from
the previous bet, the server's winnings or refund amount, the `short` flag and the local card round. In particular `0x3102` result 2,
`0x3105` and `0x3109` do not repair a mismatched wallet. A `0x3107` non-zero result on local round 5 credits the final amount and returns
to the slots; earlier non-zero results stay in card mode, so a server that ends earlier or extends past five rounds needs matching client
behaviour, not just a different result byte. Send a correct `0x0213` before `0x3106` and put the right money in `0x3106`.

### Standing up, map entry, movement and chat

The slot window `0x486e10` in state `0x18` waits for answer marker 6, assigns `0x84cef0` to wallet `+0x50`, sets the close byte `+0x74`
and the fade state `+0x2df4`. When the fade is complete the outer update `0x4899c0` returns 2. The main update `0x42cc10`, in scene 9,
handles that return at `0x4300c7..0x4300f5`: it copies the wallet to `0x84d670` (`0x4300da`), destroys the gambling scene and sets
`0x705040 = 7` (`0x4300f5`). Scene 7 reloads the current map with `0x452270(0x84e6e0, loading_window)`, rebuilds the local character and
calls `0x5858f0` (at `0x42fa77`), which sends **`u16 0x0201` only**, then returns to scene 6. So **a successful stand-up causes a map
reload and `0x0201`**; a server leaves the character waiting for entry and answers the normal enter flow.

In scene 9, `0x4899c0` handles the gambling controls and camera; the world mover does not run (camera rotation and zoom are not character
movement). The ordinary chat controller is absent from this scene's update and draw path. Incoming packets are still processed
(`0x585280`, message `0x2b10`, no scene-9 exclusion), so a server can send chat, but the client provides no chat editor or overlay there.

**Live:** NPC appearance and click distance; table and button labels; one loss with re-bet and one win with collect; card round five; gold
agreement; the stand-up reload and reappearance to others; the actual chat-key behaviour.

## A kill-boost item

### Is there an existing item?

**No existing item was proven both isolated and unobtainable.** Client data cannot prove absence from drops, shops, rewards or grants, and
placeholder names are not evidence. Adding content is feasible on the traced paths without a binary change, with the caveat below.

### How the client indexes items and skills

`Item.SData` is 100 banks, 16,045 records (about 2.96 MB plaintext); `Skill.SData` is 326 ids × 3 levels. Item type and type id and the
skill id are **one-based**: the item lookup `0x460cd0` subtracts 1 from each, and the skill lookup `0x461ba0` requires a non-zero id not
above the loaded count and a level of 1..3 (memory stride `0x88` per level). The skill loader (`0x4612f0`) allocates three records per
counted id, so **326 is not a hard maximum** on this path: appending id 327 fits the traced loops and the u16 buff id. **Confirmed.**

### Existing examples and reuse traps

- **Item 100/108** (a mini EXP stone) is an ordinary consumable granting skill **250/1**. Skill 250/1 has kind 4, detail 82, a one-hour
  duration (the description says seven days; the duration flag is hours with KeepTime 1) and a native EXP ability. Adding id 250 to a
  server's kill-boost list would boost every other source of that id.
- **Item 100/172** grants **223/1**, which another item (100/43) also grants, so renaming 172 would not isolate the boost. Similar shared
  ids affect 100/173–175.
- **Item 100/209** (a GM item) names skill 250/2, which is unavailable (kind 0, detail 0, duration 0).
- IDs 287..300 are kind-0, zero-duration placeholders. Because a server selects boosted buffs by **id**, reserving a spare level of an
  existing id does not isolate the boost from the other levels.

### Item use, buff duration, icon and text (confirmed)

- **Use:** `0x4c7d40` resolves the definition, checks requirements and cooldown, and for a Special-0 consumable sends
  `u16 0x050a, u8 bag, u8 slot` through `0x5861a0`. It does not send the item's skill id. Clone an ordinary consumable; Specials
  32/35/39/41/44/45/50 have their own flows.
- **Skill fields (file → memory):** `+1:u16` → `+8` icon number; `+0x15:u8` → `+0x1d` kind; `+0x16:u16` → `+0x1e` detail; `+0x3b:u16` →
  `+0x46` KeepTime; `+0x57:u8/+0x58:u16` → `+0x64/+0x66` first ability type/value (second at `+0x5a/+0x5b` → `+0x68/+0x6a`, third at
  `+0x5d/+0x5e` → `+0x6c/+0x6e`); `+0x6f:u8` → `+0x7f` duration flag. Name pointer at memory `+4`, description at `+0x84`.
- **Buff on and off:** the own-buff handler `0x56c020` for `0x050d` reads `u32 key, u16 skillId, u8 level` and looks up the definition (a
  missing definition yields no buff); `0x050e` (`0x56be00`) removes it by key. KeepTime is seconds (×1000 ms); flag 1 means hours (×3600 s,
  in `0x4b8710`). Detail 6 has special 30-second handling and should not be used. The login snapshot `0x010a` carries the remaining
  duration (*inferred* from earlier protocol notes).
- **Icon:** `0x4b8a60` calls `0x49dfd0`; at `0x49e01b` the icon is atlas `floor(icon/1000)+1`, cell `icon % 1000` (one-based), from
  `icon_skill.tga`, `icon_skill2.dds` and so on (`0x49d760`). A template's existing icon needs no new art.
- **Text:** the hover path (`0x4b8a60 → 0x49eb50`, or `0x49eaa0`) lays out the skill description with `0x49e2b0` and draws it with
  `0x49e9a0`. Rename **both the item and the skill strings**; changing only the item does not rename the buff tooltip. Custom names need
  no new system message.

### What adding dedicated content takes (proposal; not built)

1. **Reserve an item identity** on the server first: check every definition, source and existing inventory. A row such as 100/212 (a
   placeholder with zero skill and stack) is an example, not a reservation; type 100 already has 255 rows, so this replaces a row rather
   than appending a 256th. Clone an ordinary consumable's 81-byte fixed block and set requirements, stack, icon, Special 0, name and
   description.
2. **Reserve a new skill id.** 327 is only the next client id; a server may already use it. If free, raise the skill count from 326 to 327 and
   append all three level records (name, description, 116-byte fixed block). Clone 250/1, rename it, keep kind 4 and detail 82, and pick
   KeepTime 3600 with flag 0 for an ordinary hour (or KeepTime 1 with flag 1). Set all three ability pairs to (0, 0), clear damage, heal and
   transform fields, and keep the skill item-only. The client routes detail 82 in `0x4c0760`; a zero-ability buff is statically supported
   but not live-tested. The multiplier itself comes from the server's check on the skill id.
3. **Mind the item-link width.** Item fixed `+32` is a **u8**, loaded at `0x4609d1` into the low byte of memory `+0x26` (the high byte stays
   zero); `+33` is the skill level at `+0x28`. A local link cannot hold 327. Generic use sends only bag and slot, and incoming buffs carry
   u16 ids, and an existing item (100/130) already shows a low byte of 11 against a server id of 267, so a server definition with range
   327 and level 1 can use the ordinary path; any comparison tool must treat a low-byte 71 as a representational exception. This wide-link
   approach passes only after live checks of inventory, quick slot, tooltip and the active-action gate. A strict numeric match instead
   needs a free id of 255 or less, or a format or client change.
4. **Update the server's own definitions** (the new item row naming the full skill id and level, the skill rows, the boost list). The
   client file alone does not add a server row.
5. **Rebuild the client data deliberately:** re-encrypt the changed `Item.SData` and `Skill.SData` members with their 64-byte SEED header, a
   correct plaintext CRC-32 and real length, and 16-byte padding (`data-archive.md`), and update the archive index. A new mall product also
   needs `Cash.SData` and a matching server product list (`0xb106` only renames and reprices known product codes).

Before release, check that only the intended item applies the intended id and level, consumption, the icon, name, description and one-hour
duration agree, stats stay neutral, death, relog, expiry and refresh follow the chosen policy, and kills count the configured multiple only
while the buff is applied. These are outstanding **live** checks, not results.

## Open

- All **live** checks above (gambling scene behaviour, chat in scene 9, the wide item-link option).
- Whether an existing item can be proven unobtainable (needs the server's data, not the client's).
