# Client research queue

Questions the server specs left open that only the client (`game.exe`) or a live session can answer. The server side does not read the
client without the maintainer's OK for each question; the client side (which writes `docs/client/CLIENT-FACTS.md`) does it. Each entry
says where the question comes from, what the answer would change on the server, and whether a live test answers it instead.

When an entry is answered: move it to "Answered" with where the answer is (`CLIENT-FACTS.md` section or a live-test log), and act on
it in the server spec it names.

## Open

3. **What the client shows for `0x0513` when the skill is Fire Thorns (a buff, kind 4) or a field (Trap, Fire Wall: kind 5) and the
   target is a player.** From `docs/original/thorns-fields-pvp.md` section 9. The original sends exactly this, so it should work; the
   live test (section 46) shows it. Client side, only if the live test looks wrong: the `0x0513` handler's use of the skill kind, whether
   it subtracts the HP shown from the target's bar or waits for another packet, and the effect it plays.
22. **How server texts from scripts read in the client** (asked 2026-11-04 by `docs/plans/ext-5-lua.md` sections 7.4 and 12, for the welcome
    notice, adverts, kill-streak announcements). For each of `F106` (world notice), the GM's to-one notice (`NoticeKind::To`), the country
    and zone notices, and `0x1108` (the centre-screen "[chatting all members]" line): the longest text shown on one line, whether and how it
    wraps or is cut, the sound, how long it stays; for `0x1108`, whether the sender name may be any text (and what an empty one shows),
    and whether a player who blocked that name misses it; whether several notices sent in the burst right after entering the world all
    show. What it changes: the API's length limits and which packet each reference script uses. The live test (section 60) shows the
    look in the meantime.
23. **The feature handshake that entry 11 (c) found missing** (from `docs/plans/ext-5-lua.md` section 12): the opcode and format a
    `fixes.dll` would send after `0x0201` naming the features it has (the research's cheapest way: one new client-to-server opcode), so
    the server can tell a modified client from an unmodified one and `p:has_feature(name)` can answer. Client side: a design to agree with
    the DLL work; the server then reads it (the original closes the connection on an unknown opcode; the server side checks what this
    server does with one before relying on it). Until then
    `has_feature` answers `false`.
24. **For the client side to fold in: the enchant lines and item blocks, read for the player guide** (2026-11-05, read-only,
    authorised by the maintainer; in `docs/guide/character-numbers.md` section 12 and `docs/guide/elements-defences.md` section 7).
    The enchant formula `0x4c4920` (factor 1/3 at `0x6ac568`) gives 0, 7, 14, 21, 31 ... 311 for steps 0 to 20, the original patch's
    `ItemEnchant.ini` attack row; it is called by the character window (`0x4f48f4`) and by the weapon tooltip (`0x4a0cb0`), for
    enchant values 1 to 50. The armour tooltip computes `5 × (value − 50)` at `0x4a2830` and prints it with message 485 "Absorption".
    No item line shows a block; messages 402 and 403 ("Succeeded in blocking", "Failed to block") are the friends block list's: the
    only code that prints them is `0x5559b0`, the handler after the block memo's (`0x555980`) in the `22xx` table at `0x6a6074`, the
    `0x220E` answer of `docs/client/client-actions.md` (rechecked 2026-11-05, with the formula reproduced at every step and message 485
    read from `sysmsg-uni.txt`; the other pushes of 0x192 are window coordinates). What it changes: nothing on the server; `docs/plans/enchant-table.md` relies on the
    client using the file's values. Suggested home: `docs/client/research-balance.md`, "The character window".
25. **A usable item with `special` 0, no skill link and no HP, SP or MP** (asked 2026-11-05 by `docs/plans/ext-3-effects.md` section 8, entry
    24 there; numbered 25 here because 24 was taken meanwhile). The level stone's record (100/215 to 217, copies of 100/108's use and
    trade columns without its skill): does the client send `0x050A` for it, or refuse it locally with the message 227? And does a level
    jump of more than one (`0x051E` from level 20 to 30) show the level, the points and the bars right? What it changes: the level
    stone's row (a `special` the client sends for), or nothing. A live test answers it first (`docs/testing/live-test-checklist.md`
    section 61, item 2).
26. **A Lapisia in an empty record of group 95** (asked by `docs/plans/ext-3-effects.md` section 8, entry 25 there). 95/3 and 95/8, copies of
    95/1 and 95/6: does the enchant window take an item by its type alone, or does it read another field of the record, so that a copy
    behaves as the original? What it changes: the columns the Perfect Lapisia rows copy. Live test: section 61, item 3.
27. **A Recreation Rune in an empty record of group 100** (asked by `docs/plans/ext-3-effects.md` section 8, entry 26 there). 100/221 to 226,
    copies of 100/171 with special 62: does the client open the recreation window from the special (so any special-62 item does), and does
    it show the new craft name when only one number changes? What it changes: the stat runes' rows. Live test: section 61, item 4.
28. **The Pet Egg's tooltip line (a `fixes.dll` feature)** (asked 2026-11-07 by `docs/plans/ext-4-pets.md` section 11 A). For items of type 100
    (and 25, 44), the tooltip `0x49fa00` clears the craft-name pointer. A `fixes.dll` feature `pet-egg-tooltip`: for the item 100/228 only
    (or any item whose craft name starts with `P` followed by five digits and a dot), add one line "Contains: <name>", the name being
    `Monster.SData` record `<digits>`'s (`0x461210`). Say where the line is added, what the craft-name pointer holds at that point for type
    100 (the function clears it), and how the feature is switched on. What it changes: the egg names its monster in the bag. Fallback
    (built): every egg reads "Pet Egg"; the player sees which monster it is when the pet comes out. The research found this *inferred* small
    (entry 10 b).
29. **Appended `Monster.SData` records** (asked by `docs/plans/ext-4-pets.md` section 11 B). With patch 111: does a filler record (empty
    name, model -1, all zeros) sent in `0x0601` behave as a missing record (nothing drawn, no crash, as `0x461210` returning null does for an
    id past the end today), and are records 3001 to 5473 (copies of 1 to 2473 with Day mapped, Size 0, HP 1, normal 1000 ms and 4, chase
    1000 ms and 10) loaded (the count read from the file, `0x460e20`), scaled, unclickable, walking at 3.7 and running at 9.7 units a second?
    What it changes: the filler's values. Live test: section 63, items 1, 4 and 6.
30. **`0x050A` for an item that stays** (asked by `docs/plans/ext-4-pets.md` section 11 C). The success answer of a use with the slot's count
    unchanged (the Pet Egg, used to call and to send back the pet): does the client keep the item shown, its count and its reuse time
    right? What it changes: whether the egg answers `0x050A` or something else. Live test: section 63, item 4.
31. **The selected target (a refinement)** (asked by `docs/plans/ext-4-pets.md` section 11 D). Does the client send `0x0304` or `0x0305` when
    the player selects a monster (the target bar asking for HP), or only when a monster enters view? If on selection, the server can
    capture the selected monster instead of the weakest one that is fighting the player. What it changes: one line of the capture handler
    (`server/pets.rs`, `use_capture`). No live test needed.
32. **Patch 111** (asked by `docs/plans/ext-4-pets.md` section 11 E). The generated patch (`Item.SData` with 100/227 and 100/228,
    `Monster.SData` with the 2,473 pets and 527 fillers) is the client side's to pack, sign and hand out, as patches 109 and 110. One
    point to settle then: the archive member of `Monster.SData` (`data/monster/monster.sdata`) is written as `Monster/Monster.SData` in the
    generator's manifest by the pattern of the other two (`Item/Item.SData`, `Character/Skill.SData`); the client side confirms it.
33. **Which received packet starts the client's world loading** (from `docs/original/enter-world.md` section 1.3, cluster A1 of
    `notes/confirm-ledger.md`). The client sends `0x0201` at the end of its loading, from the scene update (`0x42e9fa`, `0x42fa77`); the
    scene state byte `0x705040` has some twenty writers in that code and none is a packet handler. Which handler (of `0x0104`, `0x0105`,
    `0x010B` or another packet of the enter-world burst) moves the scene to loading, and does the client wait for the whole burst or only
    for that packet? What it changes: whether the new server may send anything of the burst after that packet, or must keep the
    original's order to the end. A live log (`Log.txt`) of a select shows when the client stops reading the burst, but not why.
34. **What the client shows with the `0x0404` world clock** (from `docs/original/enter-world.md` section 3, cluster A1). The handler
    (`0x54f020` → `0x4c1bb0`) splits the value into an 84-day period, the week, the 4-hour block (block / 6 = the day) and the seconds;
    which code reads those parts (a day and night cycle, a calendar on screen, timed events), and does a clock that starts at 0 at every
    server start (the new server's) look wrong anywhere? What it changes: whether the new server should keep the clock across restarts as
    the original does. A live test cannot answer it without knowing where to look.
35. **How long a world notice (`0xF106`) the client survives** (from `docs/original/operator-console.md` section 10, cluster A2 of
    `notes/confirm-ledger.md`). The reader (`0x57eb00`) takes the `u16` length and copies that many bytes into a 2000-byte stack buffer with
    no check, then `0x5533a0` copies the text to its end into the global at `0x702478` and prints 356 "[Notice] <b>". The client's code also
    uses `0x702878` (1024 bytes on), `0x702880` and `0x702a80` (1544 bytes on). Which of these belong to the same buffer, and so how long a
    notice can be before it overwrites something else? What it changes: the new server lets `F106` carry up to 1999 bytes; it should cap
    the text below whatever the answer is. A live test with a 1500-byte notice would show a crash, not which variable it hit.

## Answered

Answers to 1 to 5 are in `docs/client/research-movement-storages-auction.md`, to 6 and 7 in `docs/client/research-gambling-kill-boost.md`;
the server docs named were updated at the time.

1. **Running speed per move-speed index and the `0x0501` interval** ("Queue 1"). Walk 1.5-3.5 and run 3.0-7.0 by index 0..4, mounted
   run 7.0 or 8.0; `0x0501` has no minimum interval or distance (1000 ms while moving, 250 ms on turns, 500 ms on a vertical change).
   Recorded in `docs/original/move-check.md`. The research advises against an exact forward-speed ceiling without a live calibration.
2. **How the two storages are opened** ("Queue 2"). Item-mall storage: the store button in the point shop. Billing storage: a
   type-1 merchant with extra byte 9 (Light's/Fury's Bank Teller). Opening sends nothing; taking an item is a drag (`0x2607`/`0xB102`).
   Recorded in `docs/original/item-mall.md` and live-test section 44.
4. **`0xB103`** ("Queue 4"). `u32` is a packed local date (only YYYY-MM-DD shown, once, chat category 7); types 1 and 3 show the
   date, 2 none, 4 has a missing message. Recorded in `docs/original/item-mall.md`.
5. **Auction rows** ("Queue 5"). All fields except seller/bidder are drawn or used by the tooltip; nothing may be zeroed except
   what the original zeroed. Recorded in `docs/original/auction.md`.
6. **The gambling house `0x31xx`** (`docs/client/research-gambling-kill-boost.md`, "Entry 6" sections:
   "reachable NPC and table selection", "answers and window handlers", "standing up, map entry, movement and chat"). Type-5 NPCs are drawn;
   the click opens a seven-row table selector whose row is `0x3101`'s byte (100 to 3000 per line); every answer's fields match the build;
   cells and cards 0 to 21 are tarot faces (22 the back); `short` is the "no re-bet" flag; the window trusts the money only at `0x3102`
   result 1 and `0x3106`; after `0x3106` it reloads the map and sends `0x0201`; no movement or chat input while seated. Nothing changed
   on the server: `docs/original/mini-game.md` section 13 lists each match; the rest is live test section 52.
7. **An item for the kill-boost buff** (same file, "Entry 7" sections: "source identity and indexing", "existing examples and
   reuse traps", "item use, buff duration, icon and text", "what adding dedicated content takes"). No existing item is both unobtainable
   and alone with its buff, so the maintainer chose new content: item 100/212 becomes the Kill Boost Stone giving skill 200 level 1 (empty in
   both copies, an id below 256 so the client's one-byte item link holds it), one hour, kept on death, no ability;
   `kill_boost_skills` takes `id/level` (the monsters' 200/100 must not count); since the release it is empty by
   default, the Stones' entries being `200/1:2,200/2:5,200/3:10` (`docs/server/settings.md`). Server rows in
   `../shaiya-afterglow-extensions/kill-boost` (before 0.3.0 `additions.sql`); recorded in `docs/original/operator-console.md` decision 3; the client's copy is the first client data
   patch (`docs/plans/roadmap.md` step 1b).
8. **A picture of each map, lined up with world positions** (for the data editor). Answered in `docs/client/research-extension-features.md`,
   "Map pictures". The converted pictures for `data/map-images/` are still to be made (roadmap, "Open work items").
9. **Small mobs as pets: how the client sizes, names and targets a mob.** Answered in `docs/client/research-extension-features.md`, "Small
   monsters as pets, scenery and campfires": scale classes are fixed, size 0 makes a monster unclickable, max HP must be at least 1.
10. **Pets and personal loot: catching, summoning, picking up, and ground items shown to some players only.** Answered in
    `docs/client/research-extension-features.md`, "Ground items" (a ground item sent to only some players is fine) and "Small monsters as
    pets, scenery and campfires".
11. **Content limits and extension points of the client, for a plugin system.** Answered in `docs/client/research-extension-features.md`,
    "Items, ids and ceilings", "What a server can drive in the UI", "Effects, motions, banners and markers" and "A DLL overlay: design facts".
12. **What the client itself refuses or decides about factions, levels and targets.** Answered in
    `docs/client/research-extension-features.md`, "Faction rules" and "Levels and what the client blocks".
13. **Two input ranges for the robustness fixes** (`docs/server/audit.md` R21). Answered in `docs/client/research-extension-features.md`,
    "Input ranges". The server acted on (b): a shop title with an unsafe colour marker or a byte below 0x20 is refused
    (`shaiya_proto::my_shop`, R21). On (a), hair, face and size are still stored as sent, as in the original; the clamp (the creation
    screen sends 0 to 4 each; a changed appearance allows hair 0 to 13 and face 0 to 7) is the `TODO` in `creation.rs`, not built.
14. **Where else the colour marker is parsed** (from the patch kit's `chat-markup-filter`, `docs/original/server-patch-notes.md`).
    Answered in `docs/client/research-extension-features.md`, "Where the colour marker is parsed".
15. **Placeable world objects and rest spots (campfire, tent) and sitting.** Answered in `docs/client/research-extension-features.md`,
    "Small monsters as pets, scenery and campfires".
16. **Channelled actions and progress feedback (fishing, crafting, camping).** Answered in `docs/client/research-extension-features.md`,
    "Channelled actions, buff icons, chat colours".
17. **Many bosses at once (monster madness) and event feedback.** Answered in `docs/client/research-extension-features.md`, "Ground items" and
    "Effects, motions, banners and markers".
18. **Does the character window compute its own attack, defence and other derived numbers?** Answered in `docs/client/research-balance.md`,
    "The character window" (it does; see its "Short answers").
19. **The weapon's attack interval and casting.** Answered in `docs/client/research-balance.md`, "Attack interval and casting".
20. **The race skills and percentage skills as the client shows them.** Answered in `docs/client/research-balance.md`, "Race and percentage
    skills".
21. **The creation screen and the mode.** Answered in `docs/client/research-balance.md`, "The creation screen and the mode".
