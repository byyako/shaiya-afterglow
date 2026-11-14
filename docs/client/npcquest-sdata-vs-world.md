# The client's `NpcQuest.SData` against the server's

This page compares the NPC and quest file of the client `game.exe` (US build 2010-04-26, patch 108), `NpcQuest.SData`, with the
original EP4.5 server's own copy, `PS_Game/Data/NpcQuest.SData` (1,379 NPCs, 3,416 shop items, 207 gate destinations, 1,720 quests).
The client uses its copy for what it **shows**: the shop window, the gatekeeper's destinations and fares, the NPC names and talk, the
quest window. The server uses its copy for what **happens**.

## In short

- **The two [SData](glossary.md#sdata) files have the same layout**; only the text differs: English in the client, Korean on the server.
- **Every shop lists the same items in the same order** on both sides. That matters, because a purchase names the position in the list,
  not the item.
- **One gatekeeper fare differs**: the trip to the auction house costs 1,200 gold on the server but the client shows 1,000.
- **The NPCs and quests the client lacks, or has empty, are all unused**: none of them is placed on a map.

### Using this page

- **If you write a server:** what to send and check is collected in [What a server must do for this client](afterglow/server-checklist.md#npcquestsdata-against-the-server).
- **If you patch or mod the client:** "What differs for a player" is the short list; the gatekeeper fare is the one value a player can see at odds with the server.
- **To check a claim yourself:** [sdata-generation.md](sdata-generation.md) has the file layout; [tools-and-method.md](tools-and-method.md) lists the tools.

## How the file was read

The client file is SEED-encrypted like `Item.SData` (checksum checked). Decrypted, it has **exactly the server's layout**: the reader
written for the server's file (see [Maps and NPCs](../original/maps-npcs.md)) reads it to the last byte. Only the text differs (English
instead of Korean; 3,104,237 bytes against 2,637,480). Everything but the texts was compared, with the server's map files marking which
NPCs are placed on a map.

## Shops (what players see first)

**All 252 merchants the client has list the same items in the same order as the server.** This matters more than prices: a purchase
(`0x0702`) names the position in the merchant's list, not the item (see [NPC shops](../original/shop.md)), so a different list would sell
a different item than the one clicked. Nothing to fix. Prices in the shop window come from the client's `Item.SData`, not from this file;
those differences are on [Item.SData against the server](item-sdata-vs-defs.md) (capes, mount stones).

The server has 49 more merchants (type ids 253 to 301). **None is placed on a map** (the highest placed merchant is 248), so the client
never has to show them.

## NPCs

- Client 1,271, server 1,379. The 108 the client lacks are the ends of the lists (gatekeepers 62 to 69, type 3: 45 to 50, type 6: 35 to
  39, type 7: 633 to 662, type 10: 74 to 83, and the merchants above). **None is placed.**
- 48 more are **empty records in the client** (no name, country 0, zero values): gatekeepers 60 and 61, type 3: 41 to 44, type 6: 30 to
  34, type 7: 595 to 632. On the server they have data; **none is placed** either.
- Every placed NPC (1,072 kinds; the 44 map entries of type 0 find no NPC and are skipped, see [Maps and NPCs](../original/maps-npcs.md))
  has a full record in the client. Country and the three values after the id agree for all of them; so do the merchants' extra byte and
  every NPC's quest lists.

## Gatekeepers

One difference in the 61 the client has, and it is **placed**:

| Gatekeeper | Destination | Server (charges) | Client (shows) |
|---|---|---|---|
| Tellonia Druimos (2/21, map 35) | 0: map 42, "Vault Guild Auction House" | **1,200** | 1,000 |

Its second destination (map 40, "Underground Stadium") is 1,000 on both sides; maps and positions agree everywhere. A player with 1,000
to 1,199 gold sees a fare they can pay and gets the failure answer (`0x020C` result 0, see [Gatekeepers](../original/gatekeeper.md)).
Whether the client checks the money against its own fare before sending is not known (*live*).

## Quests

The 1,720 quests agree in every field (levels, faction, jobs, start and end NPCs, items, monster counts, the three results with
experience, money and item rewards), and so do the item links (505 items that start or end a quest). One difference:

| Quest | Field | Server | Client |
|---|---|---|---|
| 1228 "Batan's Present" (start NPC 7/38, placed) | previous quest | **9999** | 1221 |

Neither side can offer it: 9999 is no quest, and 1221 is an empty quest (no name, no start NPC) that nobody can finish. The 34 other
quests with previous quest 9999 (1526 to 1559) say 9999 on both sides.

## What differs for a player

The one visible difference is the auction house fare: 1,200 gold on the server against 1,000 shown by the client. The file has the
server's own layout, so only its texts need a source of their own (see [Generating the client data files](sdata-generation.md)).

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#npcquestsdata-against-the-server).
