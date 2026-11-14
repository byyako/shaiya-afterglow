# Client findings: acted on and open

This page is part of the Shaiya Afterglow tab. It lists what the client side found in `game.exe` that matters to a server, and what Shaiya
Afterglow did with each finding, as its own notes record it. The server's pages hold the current truth: every line below links the page
that states the status, and a status word here is only a pointer to it. The client facts themselves are on the
[Client facts for server developers](../CLIENT-FACTS.md) page.

## In short

- Most findings were **built**: item use, the warehouse, extraction, duels, personal shops, guild wars, union parties, scripted
  monsters, party instances, the monster HP packet and the duplicate-login kick.
- A few client facts changed what the server sends: lapis extraction answers with 33 bytes, a joiner gets its own `0x0B0C`, union lines
  go out as `F10A`.
- The client gave corrections too: warehouse gold (`0x020D`) cannot be reached on this client, and `0x2104` prints nothing on an
  unmodified one. Their status is under "Open".
- Some work is **waiting for a test in play**: the party instances and the Lua bosses, and the client-side checks of the operator chat
  copies.
- A longer item is the HP limit: the 2010 client reads 16-bit health, so Shaiya Afterglow clamps at 65,535 and saves 32,767 (last section).

### Using this page

- **If you write a server:** read "Open" for what is still not built, and "Acted on" to see how a finding turned into behaviour.
- **If you patch or mod the client:** the "Open" list shows which client patches a server would pair with (see
  [Opportunities the client already supports](opportunities.md)).
- **To check a claim yourself:** the status words are defined on the [overview](index.md#status-words); the client addresses are on the
  pages linked from [Client facts for server developers](../CLIENT-FACTS.md).

## Acted on

Each item: the finding, then Shaiya Afterglow's status (**ref**, with the server page).

- **Item use** (`0x050A`, `0x0557`): the facts about bag 0 as failure, the Warehouse Recall Rune's window and buff, and the
  `0x0214`/`0x0215`/`0x021B` counters were folded into the server's page and agree with what was built. **ref** built
  ([Using items](../../original/item-use.md#8-the-resets)).
- **Chat names**: `0x1103` is named trade chat (to the faction) and `0x1108` all-server chat (server-wide, with a Message To Server
  item). Behaviour already matched the client's meaning. **ref** built ([World](../../original/world.md#6a-chat)).
- **Warehouse**: the NPC warehouse and the rune's window, `0x0711` contents only, and the 74-byte `0x0204` withdrawal answer with the
  money are what the server sends; the rune's buff (ability 39) lets moves happen away from a keeper. The Warehouse Recall Rune is item
  100/54 in both the server's data and the client's `Item.SData` (100/53 is Large Prevent Of Item Drop). **ref** built
  ([Warehouse](../../original/warehouse.md#7-the-client-from-docsclient)).
- **`Item.SData` against the server's items**: the client file was decrypted and compared record by record. No server change: the
  database wins. Items the client cannot show are not reachable in the normal world; the reachable differences are prices, stack sizes
  and tooltips (capes and mount stones cost less than the client shows). No server change
  (the comparison: [Item.SData against the server](../item-sdata-vs-defs.md)).
- **The other eight SData files**: all decrypted, read to the last byte and compared. `NpcQuest.SData` has the server's own layout and
  every shop is identical to the server's; the skill, monster and mall differences are the original's own (no server change).
  `KillStatus.SData` is byte-identical to the server's. No server change (the comparisons:
  [NpcQuest.SData against the server](../npcquest-sdata-vs-world.md), [Skill and Monster SData](../skill-monster-sdata-vs-defs.md)).
- **`0x0802` and `0x0215`**: extraction is answered with 33 bytes to the 4-byte request (34 only to the 8-byte form); the free skill
  reset is answered with 6 bytes (`u8 1, u16 points, u8 1`), the Skill Reset Stone with 5. **ref** built
  ([Lapis and enchanting](../../original/lapis.md#4-extraction-0x0802), [Using items](../../original/item-use.md#8-the-resets)).
- **`0x020F` and `0x0211`**: the faction kill count. `0x020F` goes to that faction's players every 10 minutes and at grade changes;
  `0x0211` carries the count and the time left of the top grade's 10 minutes (the client's countdown). The bonuses come from
  `KillStatus.SData`. **ref** built ([Player kills, part 2](../../original/pvp-2.md#9-the-counts-and-grades)).
- **Duels `0x24xx`**: built from the original's duel code. The `0x2406` reasons are those of [Duels](../duels.md) (0 the partner left
  the map, 1 out of the area, 2 gave up, 3 a hit from someone else or another death). The server keeps the original's 40 around the
  centre (the client warns at 35). Only a kill counts a victory and a defeat (`0x2405`); `/defeat` counts none on the server, so the
  client's own +1 on `0x2406 2` shows until the next login. The stakes go on the ground for the winner; every other end gives each side
  its own back. **ref** built ([Duels](../../original/duel.md#7-the-new-server)).
- **Personal shops `0x23xx`**: the server enforces the market maps (35, 36, 42 by default), sends `0x2309` only for a visit that
  succeeds (the client opens the window on any), and takes the 5 % commission on map 42. **ref** built
  ([Personal shops](../../original/personal-shop.md#7-the-new-server)).
- **Guild wars `0x25xx`**: `0x2503` and `0x2504` go only to the members within 40 of the centre when the war is accepted (the war's
  players), not every member; members who come later never take part. The original sends `0x2506` events 1 (out of the area, also
  leaving the map or the guild) and 3 (defeated), never 0 or 2. `0x2505` is only 1 or 2: no tie, no time limit; a guild loses when its
  last player is out, or its officer leaves the area, the map or the guild. The result goes to everyone who took part. Same-country
  guilds may fight. The enemy's name in `0x2505 2` is missing for a guild of the other country, as with the original. **ref** built
  ([Guild wars](../../original/guild-war.md#7-the-new-server); the client page [Guild wars](../guild-wars.md) carries the corrections).
- **Union parties**: a joiner now gets its own `0x0B0C` after the `0x0B0B` list, and `0x0B0E` for an existing union is ignored, because
  "Config Union" is `0x0B0F` / `0x0B10` on this client. A union leader's `/raid` and `/alert` lines go out as `F10A`, since this client
  ignores `0x1112` from a server. `0x1105` already went to every union member. Union invitation failures are `0x0B19` (10 bytes) and
  `0x0B18` failures are `0x0B03` codes. **ref** built ([Union parties](../../original/union.md#11-the-new-server)).
- **Scripted monsters**: the AI state machines (`AI` 10 to 99) are built; their speech is `0x1109`, kind 0 to the target (or its party)
  for `Say`, kind 1 within a radius for `Say2` (200 in the data, wider than the view, so far players drop it, as with the original). Two
  choices wait for a check in play: the lines are sent in English, and with a closing zero counted in `len`. The lines stay below 128
  bytes with their zero. `0x110A`, `0x110B` and `0x0608` for the Lua bosses are used only where those maps are opened. **ref** built, and
  the two choices are **waiting for a test in play** ([Scripted monsters](../../original/scripted-mobs.md#12-the-new-server)).
- **`0x0608` and `0x0004`**: `0x0608` (max HP from the database, current HP) follows every `0x0601` to the same players. A second login
  of an account ends the older game connection: it gets `0x0004 00` first, then a graceful close; the new login waits for the old
  character to go. The login connection sends no `0x0004`. **ref** built
  ([Client-ready extras](../../original/client-extras.md#33-deviations)).
- **Party instance maps 64 to 66 and the Lua bosses**: `0x020A` to a party map refuses with the client's codes in the original's order
  (9 closed, 4 no party, 11 the party's zone full, 10 too few members); a party gets its own zone; "Section 4" (portal country 1001)
  is the zone's own and never announced: a closed one gets no answer and the client's 15-second re-ask finds it open once a floor boss
  died. The Lua scripts run as shipped; a phase that replaces a monster always uses a new id with `0x0602` for the old one. **ref**
  built, **waiting for a test in play** ([Party instance zones](../../original/instances.md#18-the-new-server)).
- **Operator chat listening, `F502`**: built for the companion DLL's contract. `FA0B` links the GM to the player, `FA0C` ends it; each
  line the watched player receives is relayed to the linked GM as `u16 F502, char name[21], u8 len, text[len]`. The line's kind prefix
  (`[NORMAL]`, `[TO,<target>]`, `[ALL]`, `[GUILD]`, `[PARTY]`, `[SHOUT]`, `[TRADE]`, `[ZONE]`) follows the original's. Links end when
  either side leaves the world. **ref** built, **waiting for a test in play** (needs the DLL installed;
  [GM commands 1](../../original/gm-commands.md#13-the-new-server-built), [Operator chat copies (F502)](../operator-listen.md)).
- **Research answers on movement and storages**: the client's speeds are recorded (no forward-speed ceiling before the server's own
  calibration); the storages are opened by the store button and the Light's and Fury's Bank Tellers (1/179 and 1/180) and their lists
  are pushes; `0xB103` keeps the packed date; the auction rows stay fully filled. **ref** built
  ([The move speed check](../../original/move-check.md#2a-the-clients-speeds), [Item mall](../../original/item-mall.md#13-as-built)).
  A check of the tellers' spots waits in the server's play-test list; the `2104` road messages wait for roads.
- **Research answers on the gambling house**: compared claim by claim with the server's page and the build; every confirmed claim
  matches, no server change. A unit test now replays the window's own wallet arithmetic against the server's money. New checks in play
  are listed: the bet after the card game ends, card round five, the gold shown against the server's, chat and movement while seated.
  **ref** built, **waiting for a test in play** ([The gambling house](../../original/mini-game.md#13-the-clients-side-research-entry-6)).
- **Research answers on a kill-boost item**: no reuse; item 100/212 becomes a new Kill Boost Stone giving skill 200 level 1 (kept on
  death, one hour, counting down offline). The server rows exist; the client's data patch for them is a later step. **ref** built on the
  server, the client data patch **not built** ([Operator console](../../original/operator-console.md#4-design-decisions)).
- **`0x020D` and `0x2104`**: `0x2104` is **ref** not built ([Obelisks and roads](../../original/obelisks.md#10-the-new-server)); `0x020D`: Afterglow: not recorded. "Open" below says why the client side leaves both out.

## Open

Everything the client side found that the server pages do not cover yet. Nothing here is required; each line says what it would get
players.

**Corrections (do not build these as first written):**

- **Warehouse gold `0x020D`: not reachable on this client.** The request and answer handler exist, but the number dialog that sends
  `0x020D` is never created (confirmed, byte scan of `.text`). Leave it out; the original has no `0x020D` case either. Afterglow: not
  recorded (the client side: [Every opcode against the original server](../packet-opportunities.md)).
- **`0x2104` (road opened)**: still dead on an unmodified client. The handler reads the wrong argument; it wants link ids 1010, 1011,
  1020, 1021 (messages 5374, 5373, 5372, 5371). The 5-byte client fix is the default `road-messages` kit patch and is also in the companion DLL ([Obelisks and roads](../obelisks.md)).
  If road portals are built and the patch ships, number the four roads that way and send `0x2104` only when a road opens. **ref** not
  built ([Obelisks and roads](../../original/obelisks.md#10-the-new-server)).
- **Guild wars and obelisks**: the client pages [Guild wars](../guild-wars.md) and [Obelisks and roads](../obelisks.md) carry the
  server side's corrections.

**Client-ready and built:**

1. **`0xB103` type 0** for the billing line (shows nothing): the server sends the premium type, and type 0 when there is none or it
   has ended. **ref** built ([Item mall](../../original/item-mall.md#13-as-built)).
2. **Item mall beyond `0x2601`**: buy `0x2602` and gift `0x2603` take a `char productCode[21]`; mall storage `0x2606`/`0x2607` (240
   slots), billing storage `0xB101`/`0xB102`. The client checks neither points nor bag space. **ref** built
   ([Item mall](../../original/item-mall.md#13-as-built)).
3. **Auction house `1Cxx`** (the client page [Auction house](../auction-house.md) has the row layouts). **ref** built
   ([The auction house](../../original/auction.md#15-the-new-server)).
4. **Region feature 0 (lapis extraction with a chosen gem)**: if a client patch ships it, the server already answers the 8-byte
   request with 34 bytes, so nothing changes. Other client patches being considered (see [Region features](../region-features.md)): 24
   on-screen login keyboard, 25/26 target HP % (pairs with `0x0608`), 46 auto-dismount (uses `0x0216` as built), 5, 8, 23, 43, 45, 62
   (client-only). **ref** built for feature 0 ([Lapis and enchanting](../../original/lapis.md#4-extraction-0x0802)).

**Client-ready, not built or not recorded:**

1. **`0xB106` product list** (`u8 n`, n × `char code[21], char name[51], u32 price`): renames or reprices `Cash.SData` products by code;
   the client asks once per scene set-up and nobody answers. Optional. **ref** not built ([Item mall](../../original/item-mall.md#11-deviations-approved-built-section-13)).
2. **Mail `1Dxx`** (receive-only): the client page [Auction house](../auction-house.md) has the row layouts. Afterglow: not recorded.

**Live (needs a game session):** the 12-step packet log of [Actions and result messages](../client-actions.md), plus `0x0802` with the
33-byte answer, the NPC-13 "chances left" counts, `F501` failure codes, the word filter on region 6, Trap Lv1 for an Archer and
Gruesome Attack with a sword. None of it is done; the results will be added here.

## HP, MP and SP above 65,535

The client reads health as 16 bits, so a value above 65,535 cannot show correctly. The first bullet of the client page's
[Watch section](../CLIENT-FACTS.md#watch-hp-mp-and-sp-above-65535-and-current-values-above-32767) describes the wire; the rest is here.

- **The server clamps instead of rolling over.** `crates/game-server/src/stats.rs` `Totals::max_health` clamps to 65,535 before sending,
  and current health is kept as `u16`, so a character whose gear adds up past 65,535 silently plays at 65,535. No wrong display, but
  the extra HP is lost.
- **The save is lower still: 32,767.** `crates/game-server/src/pg/character.rs` (`save_position`, `save_stat_reset`, `save_progress`)
  cut current HP/MP/SP to `i16::MAX` because `characters.hp`/`mp`/`sp` are `smallint` (`db/migrations/0005_characters.sql`). A
  character above 32,767 current HP is saved at 32,767 and logs back in below full. This can bite before the 65,535 limit does.
- **The way out (if stats ever need it):** 32-bit current and max health in the server and the database, and a per-session wire width.
  The 2010 client can only read 16 bits (unless the companion DLL patches those readers), so it keeps `u16`; the EP5.4 decomp client
  is being taught to read either width, chosen by the client version it sends at server select (`0x10042600` = 16-bit). The game
  server would need that version from the login server (the `0xA301` hop does not carry it).
