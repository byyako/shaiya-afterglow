# What a server must do for this client

This page is part of the Shaiya Afterglow tab. It collects the advice for a server author that follows from the facts on the Client tab, which describe the US client `game.exe` (build 2010-04-26, patch 108). Every item is a piece of advice derived from a client fact, and each one links to that fact. "A server" means any server; where the notes of Shaiya Afterglow record what it did with an item, the status is given with a link to the page that holds the current truth.

## In short

- **The Client tab says what the client does; this page says what that asks of a server.** Each item has the advice first, then a link to the fact it comes from.
- **The sections follow the Client tab's page titles**, in the same order, so the section for a client page is easy to find.
- **A status after an item is Shaiya Afterglow's own**, as a **ref** to the server page that states it, using the server's words (**built**, **not built**, **tested in play**, **waiting for a test in play**). "Afterglow: not recorded" means no page of this site says.
- **Where the client crashes or opens a broken window, the item says so.** Those are the ones to read first (the chat marker, the Priest reply, the visit window).

### Using this page

- **If you write a server:** read the section of every system you build before you build it, and the "Client facts" section first.
- **If you patch or mod the client:** the items name the patches that change a rule (Market Anywhere for the personal shop, the road patch for `0x2104`); the patch kit is on [Patch kit for game.exe](../client-patch-kit.md).
- **To check a claim yourself:** follow the link after the item to the Client page; its detail sections give the addresses, and [Tools and method](../tools-and-method.md) says how to look at them.

## Client facts

### From "Compared with the server pages"

- **`0x0802` extract answer**: answer this client's 4-byte request with 33 bytes, without `gemPos`; the 34-byte form is read one byte off and shows the gold times 256. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Shaiya Afterglow: **ref** built, 33 bytes to the 4-byte request, 34 only to the 8-byte form ([Lapis and enchanting](../../original/lapis.md#4-extraction-0x0802)).
- **`0x0801` link answer**: either size works (18 bytes is what the client reads, 20 is accepted). (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Afterglow: not recorded.
- **`0x021B` reset counters**: send `00 01` while the free skill reset is unused (the client shows "0 chances left" for stats and "1" for skills); when a stat request still comes at 0 chances, answer `0x0214 u8 0`. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Shaiya Afterglow: **ref** built, the counters follow the client's reading ([Using items](../../original/item-use.md#8-the-resets)).
- **`0x0215` answer, 3rd byte**: answer a free skill reset with 6 bytes (`u8 1, u16 points, u8 1`) so the client counts it down; answer the Skill Reset Stone with 5. Optional. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Shaiya Afterglow: **ref** built ([Using items](../../original/item-use.md#8-the-resets)).
- **Teleportation Stone window**: never send index 0 in `0x0225` or `0x0220`, and never more than 4 entries in `0x0225`; the client stores at `index − 1` without a check and its buffer holds 4 entries. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Afterglow: not recorded.
- **Body change `0x0226`**: refuse a sex above 1 or a size above 4, which an unmodified client never sends. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Afterglow: not recorded.
- **`0x0207` exp**: send 10 bytes (two `u32`); a 6-byte answer would underflow. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#compared-with-the-server-pages)) Afterglow: not recorded.

### From "SData differences: what the client does with its own values"

- **Kimuraku 835 max HP**: send `0x0608` with the database's max HP after `0x0601` to correct the target bar; the original never does. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#sdata-differences-what-the-client-does-with-its-own-values)) Shaiya Afterglow: **ref** built, `0x0608` follows every `0x0601` to the same players ([Client-ready extras](../../original/client-extras.md#33-deviations)).

### From "System notes"

- **Personal shops (system note 3)**: enforce the market maps (Market Anywhere) on the server; the client opens the visit window on any `0x2309` and treats nearly every `0x230D` result as success. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#system-notes)) Shaiya Afterglow: **ref** built, the market maps are enforced and `0x2309` is sent only for a visit that succeeds ([Personal shops](../../original/personal-shop.md#7-the-new-server)).
- **Obelisks and roads (system note 5)**: send link states (100–103) with `0x2102`; `0x2104` prints nothing on this client. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#system-notes)) Shaiya Afterglow: **ref** built, `0x2102` goes with the link's state ([Obelisks and roads](../../original/obelisks.md#10-the-new-server)).

### From "Server questions answered from the client"

- **`0xB103` type 0**: send type 0 when the billing line should show nothing. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#server-questions-answered-from-the-client)) Shaiya Afterglow: **ref** built, `0xB103` sends the premium type, and type 0 when there is none or it has ended ([Item mall](../../original/item-mall.md#13-as-built)).
- **Chat names (`0x1103`, `0x1108`)**: relay each chat kind under the opcode it came in with, with the client's meaning for its scope. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#chat-opcode-names)) Shaiya Afterglow: **ref** built, `0x1103` is trade chat to the faction and `0x1108` is server-wide ([World](../../original/world.md#6a-chat)).

### From "Answers to server questions"

- **Scripted monsters, `0x1109` length**: keep `len` at 128 or below, with the closing zero counted or not; 133 to 255 overruns the client's stack. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, lines stay below 128 bytes with their zero ([Scripted monsters](../../original/scripted-mobs.md#12-the-new-server)).
- **Scripted monsters, trailing `"\n"`**: drop a trailing `"\n"` from a line; it draws an empty second line. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Afterglow: not recorded.
- **Scripted monsters, `0x110B`**: send a bare file name (no `Sound/` prefix); the client joins it to `data/sound/`. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built for the Lua bosses ([Party instance zones](../../original/instances.md#18-the-new-server)).
- **Boss phases**: a phase that changes the monster id needs `0x0602` for the old id before the new `0x0601`, because a `0x0601` for a known id is ignored. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, a replacing phase always uses a new id and removes the old one ([Party instance zones](../../original/instances.md#18-the-new-server)).
- **Party instances, entry**: answer `0x020A` to a party map with the client's refusal codes (4, 9, 10, 11); a closed "Section 4" portal needs no answer, and nothing is sent when it opens. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, refusals in the original's order, a closed portal unanswered ([Party instance zones](../../original/instances.md#3-entering-0x020a-to-a-p-map)).
- **Union parties, joiner**: send the joiner its own `0x0B0C` after the `0x0B0B` list, or the client does not count it as in the union. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, the joiner gets its own `0x0B0C` ([Union parties](../../original/union.md#3-entering-a-union)).
- **Union parties, `0x0B0E`**: do not send `0x0B0E` to an existing union; the client rebuilds its list from its party list. Use `0x0B0F` and `0x0B10` for settings. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, `0x0B0E` to an existing union is ignored ([Union parties](../../original/union.md#2-making-a-union-its-settings)).
- **Union parties, `0xF10A`**: put the speaker's name in the text of a union line, because the client adds none and drops the id. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, a union leader's line goes out as `F10A` ([Union parties](../../original/union.md#11-the-new-server)).
- **`0x0608`**: send it after the `0x0601`, to every player that gets that `0x0601` (also on re-entering view), and again whenever the absolute values should be reset. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built ([Client-ready extras](../../original/client-extras.md#33-deviations)).
- **`0x0004`**: send it to the older connection, then close that connection gracefully (flush, then shut down sending) so the 3 bytes are not lost to a reset. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#answers-to-server-questions)) Shaiya Afterglow: **ref** built, the older connection is kicked this way ([Client-ready extras](../../original/client-extras.md#32-the-kick-the-originals-0x0802)).

### From "Facts that correct earlier assumptions"

- **Warehouse gold `0x020D`**: leave it out; the client never creates the dialog that sends it. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#facts-that-correct-earlier-assumptions)) Afterglow: not recorded.
- **`0x2104` road opened**: if road portals are ever built and the client patch ships, number the four roads 1010, 1011, 1020, 1021 and send `0x2104` only when a road opens. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#facts-that-correct-earlier-assumptions)) Shaiya Afterglow: **ref** not built, waits for roads ([Obelisks and roads](../../original/obelisks.md#10-the-new-server)).
- **`0xB106` product list**: optional; answer the client's once-per-scene request to rename or reprice `Cash.SData` products by code. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#facts-that-correct-earlier-assumptions)) Shaiya Afterglow: **ref** not built ([Item mall](../../original/item-mall.md#11-deviations-approved-built-section-13)).
- **Item mall beyond `0x2601`**: build the buy and gift requests with a `char productCode[21]` and both storages; check points and bag space on the server, since the client checks neither. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#facts-that-correct-earlier-assumptions)) Shaiya Afterglow: **ref** built ([Item mall](../../original/item-mall.md#13-as-built)).
- **Auction house `1Cxx` and mail `1Dxx`**: build them from the row layouts when wanted; mail is receive-only. (client fact: [Client facts for server developers](../CLIENT-FACTS.md#facts-that-correct-earlier-assumptions)) Shaiya Afterglow: auction house **ref** built ([The auction house](../../original/auction.md#15-the-new-server)); mail: not recorded.
- **Region feature 0 (lapis extraction with a chosen gem)**: if a client patch turns it on, answer the 8-byte request with the original's 34 bytes. Other features worth pairing with a server are on [Opportunities the client already supports](opportunities.md). (client fact: [Client facts for server developers](../CLIENT-FACTS.md#facts-that-correct-earlier-assumptions)) Shaiya Afterglow: **ref** built, 34 bytes to the 8-byte form ([Lapis and enchanting](../../original/lapis.md#4-extraction-0x0802)).

## Packet handling

- Accept the **3-byte** portal request `0x020A` (`u8 portal`), although the reply struct is 4 bytes, and expect it again every 15 seconds while the player stands in the box (client fact: [Packet handling](../client-packets.md#findings-that-matter-to-a-server)). Afterglow: not recorded.
- Answer `0x020A` with the refusal codes the client words (0 to 12), and refuse a party-instance zone with the client's own codes (client fact: [Packet handling](../client-packets.md#findings-that-matter-to-a-server)). **ref** built for the party-instance maps, [Instances](../../original/instances.md).
- Accept the **6-byte** request `0x0502` (`u32 playerId`) and `0x0205` (`u32 groundItemId`) under the same numbers as the 17-byte and 36-byte answers (client fact: [Packet handling](../client-packets.md#findings-that-matter-to-a-server)). Afterglow: not recorded.
- Do not send `0x1112` to a client and expect it to show: the client ignores it from the server (client fact: [Packet handling](../client-packets.md#findings-that-matter-to-a-server)). **ref** built: a union leader's line goes out as `F10A`, [Union parties on the server](../../original/union.md#11-the-new-server).
- Check every chat line that carries text (`11xx`, `F1xx`) for the `<<NNNcCC` marker before relaying it: drop or break up a marker whose NNN runs past the text. An unfiltered marker crashes every client that shows the line, the sender's included (client fact: [Chat markup crash](../client-packets.md#chat-markup-crash-nnnccc)). **ref** built, [Robustness audit](../../server/audit.md).
- Treat a client log line `Packet Underflow. Protocol : 0x…` as a sign that one of your packets is shorter than the catalogue says (client fact: [Packet handling](../client-packets.md#findings-that-matter-to-a-server)). Afterglow: not recorded.

## Social packets

- Send ids only of players the client has already seen (`0x0201`) in invitation and result packets, or accept the generic text the client shows for an unknown id (client fact: [Social packets](../client-social-packets.md#conventions-shared-by-these-families)). Afterglow: not recorded.
- Trade: follow the original's order: request `0x0A01`, answer `0x0A02`, then `0x0A03` to both for the window; every add, remove or money change first cancels both ready flags, then echoes; at the end send `0x0206` to the giver and `0x0205` to the receiver for each item, then `0x0A04 0` to both; failures `0x0A04` 4 to 7 with each side's own view (client fact: [Trade](../client-social-packets.md#what-the-original-server-sends-in-order)). **ref** built, [Trade](../../original/trade.md#5-the-new-server).
- Trade: allow it only within a faction and keep a request open for 30 seconds, as the original does (client fact: [Social packets](../client-social-packets.md#in-short)). **ref** built with a setting for chosen maps, [Trade](../../original/trade.md#5-the-new-server).
- Party: send `0x0B04` to the members already there and `0x0B01` (not listing the joiner itself) to the joiner, `0x0520` to the players in view, and `0x0B05` to every member including the leaver on leaving (client fact: [Party](../client-social-packets.md#what-the-original-server-sends-in-order_1)). **ref** built, [Parties](../../original/party.md#13-the-new-server).
- Party: push the member status packets `0x0C01` to `0x0C09` yourself; the client never asks for them and the health bars move only with them (client fact: [Party](../client-social-packets.md#party-0x0bxx-and-member-status-0x0cxx)). **ref** built, [Parties](../../original/party.md#13-the-new-server).
- Guild: follow the original's create, invite, join-request, leave, kick and disband flows and their packets (`0x0D21`, `0x0D22`, `0x0D05` to `0x0D0C`, the list `0x0D0D` / `0x0D2F` / `0x0D0E`) (client fact: [Guild](../client-social-packets.md#what-the-original-server-sends-in-order_2)). **ref** built, [Guilds](../../original/guild.md#15-the-new-server).

## Union parties

- Send the joiner its own `0x0B0C` after the `0x0B0B` list: without it the client never becomes a union member (no union window, no party chat). The order of the two does not matter (client fact: [Union parties](../union-parties.md#summary)). **ref** built, [Union parties on the server](../../original/union.md#11-the-new-server).
- Send `0x0B16` or the leave packets before a `0x0B0B` list that should replace the old one: the client only adds members by slot (client fact: [Union parties](../union-parties.md#summary)). Afterglow: not recorded.
- Treat "Config Union" on an existing union as `0x0B0F` (loot rule) and `0x0B10` (auto-join), and ignore `0x0B0E` for a union that exists (client fact: [Union parties](../union-parties.md#summary)). **ref** built, [Union parties on the server](../../original/union.md#12-client-side-answered-in-docsclientclient-factsmd).

## Auction house

- Send `dwEndDate` in the packed local-time format; the client corrects it only by the `0x0114` offset (client fact: [Auction house](../auction-house.md#summary)). **ref** built, [The auction house](../../original/auction.md#15-the-new-server).
- Keep money ids distinct from market ids: the client sends back `dwMoneyID` in `0x1C0D` and does not keep the market id of a gold row (client fact: [Auction house](../auction-house.md#summary)). **ref** built, [The auction house](../../original/auction.md#15-the-new-server).
- Fill every row completely (all item fields, the money id for gold rows) and send results with the full row; the client loads each list once and has no refresh request afterwards (client fact: [Auction house](../auction-house.md#summary)). **ref** built, [The auction house](../../original/auction.md#15-the-new-server).
- The staff commands `FA19` `/auctionrecall` and `FA1A` `/auctionsearch` belong to the same system (client fact: [GM opcodes and commands](../gm-and-commands.md#faxx-gm-self-observation-bans-guilds-auction-packetadmincmde-level-5)). **ref** built, [The auction house](../../original/auction.md#16-deviations-all-approved-and-built).

## Personal shop

- Enforce the market maps yourself on `0x2301` and `0x2303`; the client's own map gate disappears with the Market Anywhere patch (client fact: [Personal shop](../personal-shop.md#summary)). **ref** built, [Personal shops](../../original/personal-shop.md#7-the-new-server).
- Send `0x2309` only for a visit that succeeds: the client opens an empty visit window for any `0x2309`, whatever its `ok` byte (client fact: [Personal shop](../personal-shop.md#summary)). **ref** built, [Personal shops](../../original/personal-shop.md#7-the-new-server).
- Send `0x230D` with 0, 2 or 3 for refusals only, and give a successful purchase a free `bag/slot`: the client writes the item there without checking (client fact: [Personal shop](../personal-shop.md#summary)). Afterglow: not recorded.
- Answer `0x2305` and `0x2306` with exact echoes; the client shows nothing until then (client fact: [Personal shop](../personal-shop.md#summary)). Afterglow: not recorded.
- Send the sign `0x0523` to the players in view on open and close, and `0x230C` to the visitors when the seller closes, cancels or leaves (client fact: [Personal shop](../personal-shop.md#summary)). Afterglow: not recorded.

## Duels

- Keep the order `0x2401`, `0x2402 1`, `0x2407`, the stake packets, `0x2403`, five seconds, `0x2404`, then `0x2405` or `0x2406` (client fact: [Duels](../duels.md#summary)). **ref** built, [Duels](../../original/duel.md).
- Put the fight area's centre as floats in `0x2403`, not ids (client fact: [Duels](../duels.md#summary)). **ref** built, [Duels](../../original/duel.md).
- Enforce the fight area on the server and send `0x2406 1` when a player leaves it: the client warns at 35 units and cancels nothing itself (client fact: [Duels](../duels.md#summary)). **ref** built; the server keeps the original's 40 units around the centre, [Duels](../../original/duel.md#6-how-a-duel-ends).
- Make the two players enemies only while the duel is in state 4 (client fact: [Duels](../duels.md#duels-24xx-request-stake-window-countdown-fight-result)). **ref** built, [Duels](../../original/duel.md#7-the-new-server).
- Send the win and loss counters only if they need correcting: the client adds its own +1 on a win and on a loss (client fact: [Duels](../duels.md#summary)). **ref** built: a surrender counts none on the server, so the client's +1 shows until the next login, [Duels](../../original/duel.md#6-how-a-duel-ends).

## Guild wars

- Send `0x2503` and `0x2504` only to the war's players (the members within 40 of the centre when the war is accepted; later arrivals never take part), each with the other guild's id (client fact: [Guild wars](../guild-wars.md#summary)). **ref** built, [Guild wars](../../original/guild-war.md).
- Send `0x2505` 1 or 2 only (no tie, no time limit) to everyone who took part, and `0x2506` events 1 (out of the area, the map or the guild) and 3 (any death) only (client fact: [Guild wars](../guild-wars.md#summary)). **ref** built, [Guild wars](../../original/guild-war.md#6-leaving-a-war-and-its-end).
- Make sure both guild names are in the client's guild list before the result, or the winner line is not printed (client fact: [Guild wars](../guild-wars.md#summary)). **ref** built: the enemy name is missing for a guild of the other country, as with the original, [Guild wars](../../original/guild-war.md).
- Make the players of the two guilds enemies once both guilds are ready (client fact: [Guild wars](../guild-wars.md#summary)). **ref** built, [Guild wars](../../original/guild-war.md#7-the-new-server).
- Enforce the area on the server and send `0x2506 1` (client fact: [Guild wars](../guild-wars.md#summary)). **ref** built, [Guild wars](../../original/guild-war.md#6-leaving-a-war-and-its-end).

## Obelisks and roads

- Send `0x2101` after the map's entry packets when the map has obelisks or road portals, `0x2102` on every change, and `0x2103` for the destruction message; the faction logic of the message is the client's (client fact: [Obelisks and roads](../obelisks.md#packets-all-server-client)). **ref** built, [Obelisks](../../original/obelisks.md#10-the-new-server).
- Check portal use (`0x020A`) on the server only: the client does not grey out a closed road (client fact: [Obelisks and roads](../obelisks.md#client-behaviour-the-packets-rely-on)). Afterglow: not recorded.
- If roads are ever built and the five-byte client patch ships, number the four roads 1010, 1011, 1020 and 1021 and send `0x2104` only when a road opens (client fact: [Obelisks and roads](../obelisks.md#summary)). **ref** not built: `0x2104` is left out, [Obelisks](../../original/obelisks.md#10-the-new-server).

## Item mall

- At world entry send `0x2601` with the account's points and, if the storage is used, `0x2606` (client fact: [Item mall](../item-mall.md#summary)). **ref** built, [Item mall](../../original/item-mall.md#13-as-built).
- On `0x2602` look the code up in the product table, check points and free bag slots, put the items in, then answer `0x2602 0` with the remaining points, one log row and one `TP_POINTITEM_UNIT` per item (client fact: [Item mall](../item-mall.md#summary)). **ref** built, [Item mall](../../original/item-mall.md#13-as-built).
- Optionally answer `0xB106` with the database's names and prices for the codes the client has, so a price change needs no client patch (client fact: [Item mall](../item-mall.md#summary)). **ref** not built, optional, [Client findings: acted on and open](client-findings-status.md#open).
- Put a gift (`0x2603`) in the recipient's mall storage and tell the recipient with `0x2604`; answer storage takes with `0x2607` (client fact: [Item mall](../item-mall.md#requests-client-server)). **ref** built, [Item mall](../../original/item-mall.md#13-as-built).

## Scripted monsters

- Send a scripted line as `0x1109`: `kind` 1 to the players around the monster (the radius is the script's), `kind` 0 to its target only; keep the text under 128 bytes with its zero (client fact: [Scripted monsters](../scripted-monsters.md#speech-and-voice-11xx)). **ref** built; the lines go out in English with the zero counted in `len`, and the server says nothing for a text of 128 bytes or more, [Scripted monsters](../../original/scripted-mobs.md#12-the-new-server).
- Pass the line through the same `<<NNNcCC` check as player chat, since the client prints `0x1109` text with the chat renderer (client fact: [Chat markup crash](../client-packets.md#chat-markup-crash-nnnccc)). **ref** built, [Robustness audit](../../server/audit.md).
- Use `0x110A` with a message id for fixed boss lines (client fact: [Scripted monsters](../scripted-monsters.md#speech-and-voice-11xx)). **ref** built for the Lua bosses with messages 6401 to 6404, [Instances](../../original/instances.md).
- Use `0x110B` with a bare file name that the archive has under `Sound/` (client fact: [Scripted monsters](../scripted-monsters.md#speech-and-voice-11xx)). **ref** built for the Lua bosses, [Instances](../../original/instances.md).
- Send these after the monster's `0x0601` has reached the player, or the client drops them (client fact: [Scripted monsters](../scripted-monsters.md#summary)). **ref** built; a phase that replaces a monster always uses a new id, [Instances](../../original/instances.md).

## Operator chat copies (F502)

- Keep the authorisation and the watch state on the server, and send copies only to the selected operator in the packet layout of the client page (client fact: [Operator chat copies](../operator-listen.md#wire-contract)). **ref** built, **waiting for a test in play**, [Listening, `FA0B` and `FA0C`](../../original/gm-commands.md#84-listening-fa0b-fa0c).
- Take the meaning of `a` and `b` in the prefix from the original server's call sites (client fact: [Operator chat copies](../operator-listen.md#wire-contract)). **ref** built: `a` is the line's kind and `b` a whisper's target, [Listening, `FA0B` and `FA0C`](../../original/gm-commands.md#84-listening-fa0b-fa0c).
- You may send `F502` without checking the client build: an unmodified client drops it silently (client fact: [Operator chat copies](../operator-listen.md#in-short)). Afterglow: not recorded.

## Item.SData against the server

- Change nothing in the server's database for the differences the page lists: the database wins and the reachable differences are the original's (prices, stack sizes, tooltips) (client fact: [Item.SData against the server](../item-sdata-vs-defs.md#in-short)). **ref** no server change was made, [Client findings: acted on and open](client-findings-status.md#acted-on).
- Remember what the client does with its own values: it sends a buy with no money check, refuses to equip below its own level (rings, amulets, Echidna, Zion Cross, mount stones 42/1 to 4), and leaves item use and stack moves to the server (client fact: [Item.SData against the server](../item-sdata-vs-defs.md#what-the-client-does-with-its-own-values)). Afterglow: not recorded.

## NpcQuest.SData against the server

- Leave the server's file as it is; the one visible difference is the auction house fare (1,200 against 1,000 shown) (client fact: [NpcQuest.SData against the server](../npcquest-sdata-vs-world.md#what-differs-for-a-player)). **ref** no server change was made, [Client findings: acted on and open](client-findings-status.md#acted-on).
- If generated client files are ever shipped, start with this file: it has the server's own layout and only its texts need a source (client fact: [NpcQuest.SData against the server](../npcquest-sdata-vs-world.md#what-differs-for-a-player)). Afterglow: not recorded.

## Skill and Monster SData

- Change nothing for the listed differences (archers cannot learn Trap, Pollution Zone asks one more point, Kimuraku's bar, the Lv3 stat remedies, some tooltips and cast bars): they are the original's own (client fact: [Skill and Monster SData](../skill-monster-sdata-vs-defs.md#what-players-see)). **ref** no server change was made, [Client findings: acted on and open](client-findings-status.md#acted-on).
- Send the real HP with `0x0608` (`u32 id, u32 maxHP, u32 HP`) to correct a monster whose client record has a wrong maximum (client fact: [Skill and Monster SData](../skill-monster-sdata-vs-defs.md#what-players-see)). **ref** built: `0x0608` follows every `0x0601`, [Client-ready extras](../../original/client-extras.md#1-monster-hp-on-the-client).
- Watch in a live test whether the client sends Pollution Zone with exactly 3 points and Gruesome Attack with a sword, and whether it refuses to attack from the server's reach on the Aidion and Astaroth monsters (client fact: [Skill and Monster SData](../skill-monster-sdata-vs-defs.md#open)). Afterglow: not recorded.

## Generating the client data files

- Ship a generated file in a client patch (`psNNNN.patch`); a generated `Item.SData` goes in at `data/item/item.sdata`, encrypted as the original (client fact: [Generating the client data files](../sdata-generation.md#getting-generated-files-to-the-client)). Afterglow: not recorded.
- Write the 16-bit fields back as `u16` when the database holds them as signed `smallint` (client fact: [Generating the client data files](../sdata-generation.md#what-the-database-lacks-for-itemsdata)). Afterglow: not recorded.
- Decide each difference a matched file would change before shipping it: prices, stack sizes, tooltips, level and trade marks (client fact: [Generating the client data files](../sdata-generation.md#what-the-database-lacks-for-itemsdata)). Afterglow: not recorded.

## The Priest packets 0x0F01 / 0x0F02

- If you build a "Priest NPC in view" check, it must require an NPC of type 14; none exists in the client data or the original world, so it equals "always refuse". Type 4 must not be the check (client fact: [The Priest packets](../research-priest-packets.md#replies-the-client-did-not-ask-for)). Afterglow: not recorded.
- Never answer `0x0F02` with `u8 1` to this client: it should crash (inferred). Answering `u8 0` shows a misleading "Insufficient gold"; ignoring the request is silent and safe (client fact: [The Priest packets](../research-priest-packets.md#replies-the-client-did-not-ask-for)). Afterglow: not recorded.
- If the original behaviour is kept, note the gold mismatch (the client wants 10 or more, the original server more than 10) and that each click is a separate charge (client fact: [The Priest packets](../research-priest-packets.md#replies-the-client-did-not-ask-for)). Afterglow: not recorded.

## Extension features

- Send only race 0 to 3, sex 0 to 1, hair 0 to 13 (creation 0 to 4), face 0 to 7 (creation 0 to 4) and size 0 to 4, and keep a strict server check: a race above 3 or a sex above 1 indexes the client's tables out of bounds (client fact: [Extension features](../research-extension-features.md#input-ranges)). Shaiya Afterglow: **ref** not built for hair, face and size (the clamp is a TODO, audit R21, [Robustness audit](../../server/audit.md)); race and sex: not recorded.
- Send a shop-sign length of 121 or less: an oversized length overwrites the return address and crashes every viewer (client fact: [Extension features](../research-extension-features.md#input-ranges)). **ref** the audit describes a refusal of a shop title that fails the marker check or holds a control byte, [Robustness audit](../../server/audit.md).
- Keep names and relayed text free of `<<NNNcCC` markers and inside the reader buffers (132 bytes for chat, 51 for memos, about 1500 for `0xF106`) (client fact: [Extension features](../research-extension-features.md#where-the-colour-marker-is-parsed)). **ref** built, [Robustness audit](../../server/audit.md).
- Drive the UI through the windows a server can fill: the shop goods list `0x0701`, the yes/no dialogs of party, union, guild, trade and duel, the reward picker `0x0907`, the crafting windows (remake is the simplest "put items in, server answers" window) (client fact: [What the client shows when a server sends it](../research-extension-features.md#what-the-client-shows-when-a-server-sends-it)). Afterglow: not recorded.
- Give small monsters a max HP of at least 1 and do not use size 0 unless the monster should be unclickable (client fact: [Extension features](../research-extension-features.md#small-monsters-as-pets-scenery-and-campfires)). Afterglow: not recorded.
- Keep system message 863 free of other uses, or give it a text in the client's message file: `0xB103` type 4 asks for it (client fact: [Extension features](../research-extension-features.md#what-the-client-shows-when-a-server-sends-it)). Afterglow: not recorded.

## Quality-of-life features

1. Start with the smallest useful additions: sell UI, inventory search and highlights, factual tooltip improvements, confirmed native visibility controls and acceptance of the existing region features; preserve custom display modes and correct validated aspect consumers first (client fact: [Quality-of-life features](../qol-feasibility.md#feasibility-matrix)). Afterglow: not recorded.
2. Then the broader client work: sequential sorting and stacking, readable cooldown and buff timers, a third visible bar on existing slot pages, consistent HUD, font and hit-test scaling (client fact: [Quality-of-life features](../qol-feasibility.md#implementation-boundaries)). Afterglow: not recorded.
3. Then the joint server features: protected items, explicit rules for remote blacksmith and warehouse use, warehouse gold, authoritative party and event timer information, well-defined batch operations (client fact: [Quality-of-life features](../qol-feasibility.md#implementation-boundaries)). Afterglow: not recorded.
4. Last the larger systems: buyback, atomic equipment presets and new replicated cosmetic systems; plan identity, persistence and assets before the UI (client fact: [Quality-of-life features](../qol-feasibility.md#implementation-boundaries)). Afterglow: not recorded.
