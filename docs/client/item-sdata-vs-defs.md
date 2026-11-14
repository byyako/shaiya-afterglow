# The client's `Item.SData` against the server's item table

This page compares the item file of the client `game.exe` (US build 2010-04-26, patch 108), `Item.SData`, with the item table the
original EP4.5 server `ps_game.exe` loads, `PS_GameDefs.Items` in its database. The database is what the original server uses ("the
database wins"), so the client file only decides what the client **shows** and what it **refuses before sending**. Every difference
below is the original game's own.

## In short

- **The client and the server each have their own item list** (the client's is an [SData](glossary.md#sdata) file), and they disagree on 532 named items. 123 of those can be had in
  normal play.
- **Most differences only change what players see**: a potion's tooltip shows flat points while the server heals a percentage, a cape
  costs 40 million in the shop window but the server charges 741,190, leggings promise less MP than they give.
- **A few change what players can do**, because the client checks some values itself: rings, amulets and mount stones cannot be
  equipped between the server's level and the client's higher level.
- **Names never matter**: the server never sends item names, so the client's own names are what players read.

### Using this page

- **If you write a server:** what to send and check is collected in [What a server must do for this client](afterglow/server-checklist.md#itemsdata-against-the-server).
- **If you patch or mod the client:** "Real differences in items a player can get" lists what a player notices; "What the client does with its own values" says which values the client enforces itself.
- **To check a claim yourself:** [sdata-generation.md](sdata-generation.md) has the file layout and how to decrypt it; [tools-and-method.md](tools-and-method.md) lists the tools.

## How the file was read

The file is SEED-encrypted (EP4-7 header, 64 bytes; checksum checked). It was decrypted with Parsec's constants (the decryption
reproduces Parsec's own encrypted/decrypted sample byte for byte) and read with the EP4/EP5 layout: `u32` 100 groups; per group `u32`
count; per record `u32`-length name, `u32`-length description, 81 fixed bytes (the offsets are in
[Client facts](CLIENT-FACTS.md#sdata-layouts)). The layout uses the whole file exactly. The records were then compared with the
database by type and type id. Spot check: Long Sword (1/1) agrees field for field, including the odd `ReqWis` 3 and `ReqLuc` 25043 the
database has too.

## Counts

- Client: **16,045 records**, 134 of them empty (type 0). Server: **16,047 rows**. No duplicate keys.
- Only on the server (136): 29/19, 29/20, 29/31, 29/32, and 69/125 to 69/190, 84/125 to 84/190, all placeholders (`???`, `...`).
- In both but **all zero in the client** (4,452): nearly all `???` placeholders on the server, plus the mooncake and lantern (44/169,
  44/170). The client would show nothing for these. **None is reachable in the normal world:** the three in a shop (44/162 to 164) are
  sold only by NPCs 298 and 299, which are placed on no map, and the 14 in a dropped grade (grades 613, 616, 756, 757, 759, 777, 865) drop
  only from monsters 1875 to 1967, which spawn nowhere.
- Names: 7,729 differ (wording, capitals, and the client's names often one tier off the server's, e.g. 1/32 "Noble Gladius" in the
  client, "Worship Gladius" in the database). The server never sends names, so they do not matter.

## Not real differences

- **`ReqLuc`, `Quality` and the other 16-bit fields**: the database keeps them as signed `smallint`, so values above 32,767 read negative
  (client 44061, database −21475: the same 16 bits). Compared as 16 bits, they agree.
- **`Range`** is one byte in the EP4/EP5 client file and two bytes in the database: 268 in the database is 12 in the client (256 + 12),
  for the remedies, charms, chocolates and similar items. The low bytes all agree.
- **`Grade`** (151 named items differ, mostly 999 against a number): the drop group, used only by the server.

## Real differences in items a player can get

Of the 532 named items that differ, **123** can be had in the normal world: sold by a placed merchant, in a grade that a spawned monster
drops, or a starting item. What the client does with its own value is *inferred* (not read in `game.exe`) unless marked; the client
checks are on [Client facts](CLIENT-FACTS.md#sdata-differences-what-the-client-does-with-its-own-values).

| Items | Server (`Items` table) | Client | Effect |
|---|---|---|---|
| Healing, Stamina, Mana and Super Potions, Mini to Great (25/19 to 27, 25/37 to 39), shop and drop | special **17**, recovery **35/50/75** (per cent) | special 0, flat amounts (1057, 1483, … 2400) | the tooltip shows flat points; the server heals the percentage (see [Using items](../original/item-use.md)) |
| Capes 24/1 to 6, 86 to 91, 171 to 176 and 39/1 to 6, 86 to 91, 171 to 176, shop | buy **741,190**, sell 74,119 | buy 40,000,000, sell 4,000,000 | the shop shows 40 million; the server charges 741,190. If the client checks the money against its own price before sending a buy, it refuses players with less than 40 million (live) |
| Mount summon stones 42/1 to 4, shop | level **25**, buy **7,500,000** | level 40, buy 11,000,000 | as for the capes; the client may also refuse levels 25 to 39 (live) |
| Mount summon stones 42/5 to 8, shop | buy **62,500,000** | 75,000,000 | as above |
| Gold bars [1] to [1000] Etin (43/1 to 4) | stack **100** | 50 (43/1, 43/2), 10 (43/3, 43/4) | the server stacks to 100; the client may refuse to merge past its own limit before sending (live) |
| 28/147 (attribute synthesis stone), shop | buy **40,000,000**, quality 100, stack 1 | buy 0, quality 50, stack 10 | the shop shows it free |
| Bootleggery Recall Rune (100/132), shop | buy **23,740** | 23,700 | 40 more than shown |
| Leggings 33/171 to 203, shop and drop | MP bonus **47 … 410** | 36 … 316 | the tooltip shows less MP than the server gives |
| Dragon Helmets 16/116 to 118 and Raffiniert Helmets 16/121 to 127, drop | HP/SP/MP 47/34/34 and 50/35/36 | the two sets' values swapped | tooltips swapped |
| Claws 65/237 to 248, drop | attack 94/16 and 115/19 | 90/18 and 110/20 | tooltip |
| Level 2 crystals 30/164 to 176, drop | slot 1 | slot 2 | the lapis slot rule (see [Lapis and enchanting](../original/lapis.md)) |
| 32/1 (iron breastplate), shop and drop | `ReqVg` 0 | 1 | unknown use |
| Pardalis wing, heart, claw (44/89 to 91), drop | level 51 | 0 | the client lets a lower level try to use them; the server decides |
| 44/99 (invincibility potion Lv1), shop | level 16, `Range` 199 | level 1, `Range` 20 | the client allows levels 1 to 15 to try; the server refuses |

## Other differences (items not reachable in normal play)

- **Stack sizes of the premium items (group 100)**: the database says **1** for most (the EXP stones, Eternal Endurance, Prevent Drop,
  the reset stones, Double Warehouse, Name Change and the pockets), the client **255**; the stat remedies (100/145 to 162) the reverse
  (255 in the database, 5 in the client). The server's stack limit decides combining (bag and warehouse moves, see
  [Moving and equipping items](../original/items.md)).
- **`ReqOg`** (1 = not tradeable or droppable): Zion Cross (9/92 to 95), Dark Rain (10/92 to 95) and the chocolates (44/150 to 154) are 1
  in the database and 0 in the client, so the client offers them in a trade and the server refuses (see [Trade](../original/trade.md)).
- **Level requirements**: Dread/Legendary/Goddess Rings and Amulets (22/154 to 156, 23/153 to 155) are level 22 in the database and
  31/44/51 in the client (the client refuses to equip below its value, confirmed on [Client facts](CLIENT-FACTS.md#sdata-differences-what-the-client-does-with-its-own-values));
  Zion Cross 55 against 53; Echidna (12/166 to 170) 51 against 46.
- **Stat bonuses**: Divine Spirit (11/106 to 115) and Primal Hawk (14/106 to 115) about a third higher in the database; the Halloween
  masks (16/70 to 74 and copies) +15 to every stat in the database, 0 in the client; Santa's Hat +30 against +10; Valentine Day Ring 0
  against +3.
- **Lapis placement flags** (`ReqLevel` to `ReqStr` for lapis): the Max lapis (30/7 to 42) allow one or two more item kinds in the client
  than in the database. Matters for linking.
- **Firecrackers** 100/115 to 117: buy 0 in the database (1,000 in the client), sell 1,000 against 100.
- **Special**: the Christmas Party Hats (16/81, 166, 251) special 0 in the database, 63 in the client.

## What the client does with its own values

Read in `game.exe` (see [Client facts](CLIENT-FACTS.md#sdata-differences-what-the-client-does-with-its-own-values)): the shop sends a buy
with no money check, so the client never refuses a cape or a mount stone for its own price. Equipping is refused below the client's
level, so the rings, amulets, Echidna, Zion Cross and mount stones 42/1 to 4 cannot be equipped between the two levels. Item use and
stack moves are not compared with the client's file at all. The differences on this page are the original game's own (prices, tooltips).

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#itemsdata-against-the-server).
