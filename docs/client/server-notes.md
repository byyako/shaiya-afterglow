Server-side working notes moved out of the published client pages; never in the site nav.

## From CLIENT-FACTS.md

## Server side: acted on (Shaiya Afterglow)

- **Item use** (`docs/original/item-use.md`, built): the `0x050A`/`0x0557` facts, the bag-0 failure, the Warehouse
  Recall Rune's window and buff, and the `0x0214`/`0x0215`/`0x021B` counters are folded into its section 10a; they agree with what was built.
- **Chat names**: `shaiya-proto` now names `0x1103` `OP_TRADE` and `0x1108` `OP_ALL_SERVER`; behaviour was already the client's
  meaning (`0x1103` to the faction, `0x1108` server-wide with a Message To Server item); `docs/original/world.md` section 6a renamed too.
- **Warehouse** (`docs/original/warehouse.md`, built): the NPC warehouse and the rune's window, `0x0711` contents only, and
  the 74-byte `0x0204` withdrawal answer with the money are what the server sends; the rune's buff (ability 39) lets moves happen away from a
  keeper. The Warehouse Recall Rune is item **100/54** in both the server's data and the client's `Item.SData` (100/53 is Large Prevent Of
  Item Drop); the corrections table said 100/53 and now says 100/54.
- **`Item.SData` against `defs.items`** (`docs/client/item-sdata-vs-defs.md`): the client file decrypted and compared record by record. No
  server change: the database wins. Items the client cannot show are not reachable in the normal world; the reachable differences are prices,
  stack sizes and tooltips (capes and mount stones cost less than the client shows).
- **The other eight SData files** (`docs/client/npcquest-sdata-vs-world.md`, `docs/client/skill-monster-sdata-vs-defs.md`): all decrypted,
  read to the last byte and compared. `NpcQuest.SData` has the server's own layout and **every shop is identical** to the server's; the
  skill, monster and mall differences are the original's own (no server change). `KillStatus.SData` is byte-identical to the server's.
- **`0x0802` and `0x0215`**: extraction is answered with **33 bytes** to the 4-byte request (34 only to the 8-byte form,
  `docs/original/lapis.md` section 4, deviation 5); the **free** skill reset is answered with **6 bytes** (`u8 1, u16 points, u8 1`), the Skill
  Reset Stone with 5 (`docs/original/item-use.md` section 8, deviation 5).
- **`0x020F` and `0x0211`** (`docs/original/pvp-2.md` section 9): the faction kill count. `0x020F` (`u8 faction, u32 count`) goes
  to that faction's players every 10 minutes and at grade changes; `0x0211` carries the count and the time left of the top grade's 10
  minutes (the client's countdown). The bonuses come from `KillStatus.SData` (the same bytes as the client's).
- **Duels `0x24xx`** (`docs/original/duel.md`): built from `CUser::PacketPVP` and every `PvPChkLeave*` path. The `0x2406`
  reasons are those of `duels.md` (0 the partner left the map, 1 out of the area, 2 gave up, 3 a hit from someone else or another
  death). The server keeps the original's 40 around the centre (the client warns at 35). Only a kill counts a victory and a defeat
  (`0x2405`); `/defeat` counts none on the server, so the client's own +1 on `0x2406 2` shows until the next login. The stakes go on
  the ground for the winner at the two players' feet (`ItemDropPvP`); every other end gives each side its own back there.
- **Personal shops `0x23xx`** (`docs/original/personal-shop.md`): built from `CUser::PacketMyShop`. The server enforces the market
  maps (35, 36, 42 by default, `GAME_MARKET_MAPS`), sends `0x2309` only for a visit that succeeds (the client opens the window on any), and
  takes the 5 % commission on map 42 as `personal-shop.md` says.
- **Guild wars `0x25xx`** (`docs/original/guild-war.md`): built from `CUser::PacketPVPGuild`, `CGuild::PvPUserSet`,
  `PvPUserLeave` and the `PvPChkLeave*` guild halves. Corrections to `guild-wars.md`: `0x2503` and `0x2504` go only to **the members
  within 40 of the centre when the war is accepted** (the war's players), not every member; members who come later never take part. The
  original sends `0x2506` events **1** (out of the area, and also leaving the map or the guild) and **3** (defeated, any death); never 0 or
  2 (this server sends 0 for leaving the world). `0x2505` is only 1 or 2: no tie, no time limit; a guild loses when its last player is out,
  or **its officer** (the one who asked or answered) leaves the area, the map or the guild. The result goes to everyone who took part,
  already out or not. Same-country guilds may fight (their players become enemies). The enemy's name in `0x2505 2` is missing for a guild
  of the other country (the client's guild list is per country), as with the original.
- **Audit F1 and F2** (`docs/server/audit.md`): both fixed. F1: the lasting effects (fame, the stat and skill resets, the free repair)
  are saved only after the item's decrement; `0x050A` still follows the effect's packets. F2: a pick-up whose save fails puts the item
  back on the ground before the connection ends; a missing definition does too (error 1). `0x020D` and `0x2104`: never built, left out.
- **Union parties** (`docs/original/union.md`): specified from `ps_game.exe`. For the client side (section 12 there):
  what sends `0x0B0F` (loot rule, `u32`) and `0x0B10` (auto-join, `u8`): the client has senders 0x585cd0 / 0x585c80 and handlers
  0x5813e0 / 0x581410, missing from `union-parties.md`; whether a joiner, which the original sends only the `0x0B0B` list (no own
  `0x0B0C`, no `0x0B0E`), gets the union window and line 207; after "Config Union" (`0x0B0E` to an existing union) whether the
  client's slots follow the server's joining order; whether `0x1105` shows for union members. Corrections: the original's joiner gets
  no own `0x0B0C`; union invitation failures are `0x0B19` 10 bytes (`u32 code, u32 id`), `0x0B18` failures are `0x0B03` codes.
  Built: a union leader's `/raid` / `/alert` line goes out as `F10A` (`u32 char id, u8 len, text`), since
  this client ignores `0x1112` from the server (`docs/original/union.md` deviation 7): where does "[Union] <b>" (6606) show?
  **Answers acted on**: a joiner now gets its own `0x0B0C` after the `0x0B0B` list (deviation 9), and `0x0B0E` for an
  existing union is ignored, "Config Union" being `0x0B0F` / `0x0B10` on this client (deviation 8, replacing 4); both proposed.
  `0x1105` already went to every union member; `0xF10A` as built.
- **Scripted monsters** (`docs/original/scripted-mobs.md`, from `scripted-monsters.md`): the AI state machines of
  `mob_ai.ini` (`AI` 10 to 99) are specified and built; their speech is `0x1109`, kind 0 to the target (or its party) for `Say`, kind
  1 within a radius for `Say2` (200 in the data, wider than the view, so far players drop it, as with the original). Two choices for the client side to confirm live
  (deviations 1 and 2 there): the lines are sent in **English** (the original's `kr_string.DB` is Korean), and **with a closing zero
  counted in `len`**, as this file's section on `0x1109` reads the client; the original sends `len = strlen` with the file's `"\n"` and
  no zero. Open: where the kind-1 lines show (type 0x1E). Not built: `0x110A`, `0x110B` and `0x0608` (the Lua bosses use them; their
  maps are party instances the server does not open). **Answers read**: the lines stay below 128 bytes with their zero
  (the server says nothing for a text of 128 or more), and the kind-1 screen list needs nothing; `0x110A` / `0x110B` (bare file
  names), the repeated-`0x0601` rule and the party-instance facts wait for the Lua bosses.
- **`0x0608` and `0x0004`** (`docs/original/client-extras.md`, built): `0x0608` (max HP from the database,
  current HP) now follows every `0x0601` to the same players (deviation 1). A second login of an account now ends the older game
  connection, as the original session server's `0x0802` did (the login server notifies the game servers through PostgreSQL); the
  kicked connection gets `0x0004 00` first, then FIN, and is drained before the socket closes (deviation 2); the new login's
  handshake waits for the old character to go instead of being refused (deviation 3). Proposed. The login connection sends no
  `0x0004` (the original closes an older login silently, and only on another login server). Open: whether `0x0004` on the
  **login** connection shows the same box (not needed now).
- **Party instance maps 64–66 and the Lua bosses** (`docs/original/instances.md`, from "Answers for the server" item 2):
  built. `0x020A` to a P map refuses with exactly the client's codes in the original's order (9 closed, 4 no party, 11 the party's
  zone full, 10 too few members); a party gets its own zone of the map; "Section 4" (portal country 1001) is the zone's own and is
  never announced: a closed one gets no answer and the client's 15-second re-ask finds it open once a floor boss died
  (`LuaUpdateInsZonePortal` sends nothing, as you found). The Lua scripts run as shipped (Lua 5.1): `0x110A` with sysmsg 6401–6404
  for the boss lines, `0x110B` and the 136-byte `0x1109` for scripts that use them. A phase that replaces a monster always does so
  with a new id (`LuaCreateMob`, the old one removed with `0x0602`), so the repeated-`0x0601` case does not arise. Not tested live yet.
- **Operator chat listening, `F502`** (from "F502 operator chat listening (client side)"): built for your
  `fixes.dll` contract. `FA0B` links the GM to the player (`F501 0`, unknown name `F501 FA0B`), `FA0C` ends it (`F501 0`, none
  `F501 FA0C`); each line the watched player gets relayed sends the linked GM only `u16 F502, char name[21], u8 len, text[len]`, 24+len
  bytes, the text ending in a counted zero. **a/b confirmed** from `SendListenChat`'s call sites in `ps_game.exe` (`docs/original/gm-commands.md`
  section 8.4): `a` is the line's kind, `b` a whisper's target: `[NORMAL]`, `[TO,<target>]`, `[ALL]` (`0x1103`), `[GUILD]`, `[PARTY]`
  (also the union leaders' line), `[SHOUT]`, `[TRADE]` (`0x1108` / Message To Server), `[ZONE]` (`0x1111`; the original counted the
  zero after `ZONE` into the prefix, here 4 letters, server deviation 22). The prefix is kept as the original's otherwise; no new protocol
  variant. Links end when either side leaves the world (deviation 21). Not tried live: needs the DLL installed (live test section 49).
  F706 = open and F707 = closed and the other command contracts stay as built.
- **Research answers, queue 1, 2, 4, 5**: applied (move check: client speeds recorded, no forward-speed ceiling
  before diagonal/lag calibration; the storages are opened by the store button and the Light's/Fury's Bank Tellers, 1/179 and 1/180, and
  their lists are pushes; `0xB103` keeps the packed date; the auction rows stay fully filled). The tellers' placed spots are now in live
  test section 44. Queue 3 stays open until live test 46 shows a wrong display. The `2104` road messages wait for roads.
- **Research answers, queue 6 (gambling house)** (`research-gambling-kill-boost.md`, "Entry 6"): compared
  claim by claim with `docs/original/mini-game.md` and the build; **every verified claim matches, no server change**. The type-5 NPC,
  the seven-row selector (`0x3101 u8 table`, 100 to 3000), all nine answers' fields, cells and cards 0 to 21 (22 never sent), `short` as
  the "no re-bet" flag, the money taken only at `0x3102` result 1 and `0x3106`, `0x0213` + `0x3106` then the map reload and `0x0201`,
  no movement or chat input while seated. A unit test now replays the window's own wallet arithmetic against the server's money over
  400 random sessions (`mini_game::tests::the_clients_wallet_follows_the_servers_money`). Section 13 there lists each match. New live
  checks in section 52 (items 6 to 9): the bet after the card game ends (not read: the server clears it), card round five, the gold
  shown against the server's, chat and movement while seated. To check on your side: the report names `0x84cef0` both as the table's
  price (set at `0x3101`) and as what state `0x18` puts in the wallet on `0x3106`; if `0x3106`'s callback stages the money there, say so.
  The server is right either way (the wallet should end as `0x3106`'s money, live item 8).
- **Research answers, queue 7 (kill-boost item)** ("Entry 7"): no reuse; the maintainer chose new content. Item **100/212**
  becomes the **Kill Boost Stone** (a copy of 100/108: Special 0, country 6) giving skill **200 level 1**, "Kill Boost" (a copy of 250/1:
  kind 4, detail 82, targets oneself) with the three abilities (0, 0), **KeepTime 1 with the duration flag 1** (one hour, kept on death
  and counting down offline). 200 rather than 327: its levels 1 to 3 are empty in both copies (`skill-monster-sdata-vs-defs.md` lists
  no difference for 200) and no item names it, and below 256 the item's skill byte is the real id. The monsters' 200/100 (Weakening
  Curse, your `NpcSkill.SData` group 199) is untouched; `GAME_KILL_BOOST_SKILLS` now takes `id/level`, default `200/1`, so it never
  counts. Server rows: `extensions/kill-boost` (before 0.3.0: `additions.sql`), applied on the dev database. **For your side, in the first client data patch**
  (`docs/plans/roadmap.md` step 1b, which also builds the generator): `Item.SData` 100/212 with skill byte 200, level byte 1, the stack and
  icon of 100/108, a name and description; `Skill.SData` 200 level 1 as above with the name "Kill Boost" (levels 2 and 3 left empty);
  check that 200's levels in your copy are empty before writing them.
- Noted, not acted on yet: blocking invitations and the unfiltered chat kinds (server-only, a design choice), `0xB103` type 0, the
  auction house rows.

## Open for the server (client side)

Everything the client side found that the server docs do not cover yet, checked read-only against `docs/` and the commit log up to
the obelisks work. Nothing here is required; each line says what it would get the players. Client docs named are in this folder.

**Corrections (do not build these as first written):**
- **Warehouse gold `0x020D`: not reachable on this client.** The request and the answer handler exist, but the number dialog that
  sends `0x020D` (warehouse window `+0x720`) is **never created**: every store to that field writes 0 (verified, byte scan of
  `.text`; `packet-opportunities.md` opportunity 1). Leave it out; the original has no `0x020D` case either.
- **`0x2104` (road opened)**: still dead on an unmodified client. The handler reads the wrong argument; it wants **link ids 1010,
  1011, 1020, 1021** (Jungle of Light, Desert of Light, Jungle of Fury, Desert of Fury: messages 5374, 5373, 5372, 5371). A 5-byte
  client patch is designed (`obelisks.md` summary), not applied. If road portals are ever built and the patch ships, number the four
  roads that way and send `0x2104` only when a road opens.
- **Guild wars**: `guild-wars.md` now carries the server side's corrections (`0x2503`/`0x2504` to the war's players only; `0x2505`
  1/2 only; `0x2506` 1 and 3 only). **Obelisks**: `obelisks.md` keeps the server side's "Answered by the server side" section.

**Not built, client-ready:**
1. ~~**`0x0608` mob max HP**~~: built (see "Server side: acted on").
2. ~~**`0x0004` duplicate login**~~: built with the kick of the older connection (see "Server side: acted on").
3. **`0xB106` product list** (`u8 n`, n × `char code[21], char name[51], u32 price`): renames/reprices `Cash.SData` products by code;
   the client asks once per scene set-up and nobody answers (`item-mall.md`, `packet-opportunities.md` opportunity 3). Optional.
4. **`0xB103` type 0** for the billing line (shows nothing); already in "Noted, not acted on".
5. **Item mall beyond `0x2601`**: buy `0x2602`/gift `0x2603` take a `char productCode[21]`; mall storage `0x2606`/`0x2607`
   (240 slots), billing storage `0xB101`/`0xB102` (`item-mall.md`). The client checks neither points nor bag space.
6. **Auction house `1Cxx`** and **mail `1Dxx`** (receive-only): `auction-house.md` has the row layouts.
7. **Region feature 0 (lapis extraction with a chosen gem)**: if the client side ships that patch, the server already answers the
   8-byte request with 34 bytes, so nothing changes here. Other client patches being considered (`packet-opportunities.md` "Region
   features"): 24 on-screen login keyboard, 25/26 target HP % (pairs with item 1), 46 auto-dismount (uses `0x0216` as built), 5, 8,
   23, 43, 45, 62 (client-only).

**From the client side's read-only inventory audit (against `crates/game-server` at that date):** two P1 failure paths,
not executed, and not visible in `docs/original/item-use.md` / `docs/original/item-drop.md` since:
- **F1, item use**: a persistent effect (e.g. Fame's `save_honor`, stat/skill resets) is committed and sent **before** `use_up`
  consumes the item in a separate transaction (`server/items.rs` around `save_honor` and `Done::UsedUp`). If the consume fails or the
  write fails between them, the effect stays and the item remains. Suggested: effect + decrement in one transaction, then packets.
- **F2, pickup**: `zones.pick_up` removes the ground item and broadcasts `0x0402` before the gold/inventory save; a failed or erroring
  save (or an unknown item definition) returns without putting the loot back (`zone.rs` `pick_up`, `server.rs` pickup caller).
  Overflow/capacity branches do restore it. Suggested: reserve under the ground lock, save, then remove and publish.

**Live (client side, if possible):** the 12-step log in the client repo's `client-actions.md`, plus `0x0802` with the
33-byte answer, the NPC-13 "chances left" counts, F501 failure codes, the word filter on region 6, Trap Lv1 for an Archer, Gruesome
Attack with a sword. Results will be added here.

## Not done yet (resume list)

1. ~~Finish comparing with the server docs/code~~: done (section "Compared with the server's docs"). Left for later features:
   the auction-house row contents (`1Cxx`, not built; the client reads only Type/TypeID and the id), mail (receive-only, not built).
   ~~For the server to decide: the `0x0802` 33-byte answer and the optional `0x0215` byte~~: both built (see "Server side: acted on").
2. ~~The other client SData files~~: compared (see "Server side: acted on"). Next on that path: the `client` schema and the
   generator (`docs/client/sdata-generation.md`, "Getting generated files to the client").
3. Live packet log (optional; not done, the maintainer chose "not now"). The checklist is in the client repo's `client-actions.md`
   (12 steps; new: the NPC-13 "chances left" counts); also open: Trap Lv1 for an Archer, Gruesome
   Attack with a sword.

## From sdata-generation.md

## 5. Order of work (when we get to it)

1. ~~Get the client's copies of the files in section 3 and record their SHA-256~~ (done, section 3).
2. ~~Compare each with the server's data as for items; write up the differences players can see~~ (done: `npcquest-sdata-vs-world.md`,
   `skill-monster-sdata-vs-defs.md`).
3. `client` schema and import (section 2), with a test that regenerates each original byte for byte.
4. The generator (a `cargo` binary reading PostgreSQL, or the Python tools first), with that same test as its guard.
5. Decide, difference by difference, which values the shipped files take; then the first generated patch, through the client repo.

**Steps 3 to 5 built for `Item.SData` and `Skill.SData`:** `db/client/schema.sql` and `tools/sdata/load_client.py` (the import),
the client rows in `content.client_items` / `client_skills` (what a patch changes; before 0.3.0: `client_rows.sql`), `tools/sdata/client_patch.py` (`restore` rebuilds both originals byte for byte,
`check` runs the eight checks, `generate` writes `data/client-patches/psNNNN/`). The design and the first patch (109) are in
`docs/plans/done/client-patches.md`; the other files follow the same pattern.

**Next:** the first patch has a purpose: the Kill Boost Stone (`Item.SData` 100/212, `Skill.SData` 200 levels 1 to 3, the server's
rows in `extensions/kill-boost`). Steps 3 to 5 become `docs/plans/roadmap.md` step 1b (client data patches), before the test bots.

## From item-sdata-vs-defs.md (original "For the server" section, before it was made public)

## 5. For the server

Nothing to change: the database wins, and the reachable differences are the original's (prices, tooltips). Worth a look in a live test:
whether the client refuses to buy a cape (24/1) or a mount stone (42/1) with less money than the price it shows (live). The full list of
named differences comes from `tools/sdata/compare_items.py`.

**Client side checked (`CLIENT-FACTS.md` "SData differences: what the client does")**: the shop sends a buy with no money
check (no refusal for capes or mount stones); equipping is refused below the client's level (the rings, amulets, Echidna, Zion Cross and
mount stones 42/1 to 4 cannot be equipped between the two levels); item use and stack moves are left to the server.

## From npcquest-sdata-vs-world.md (original "For the server" section, before it was made public)

## 5. For the server

Nothing to change: `world.*` stays as it is. The one visible difference is the auction house fare (1,200 against 1,000 shown). If generated
client files ship later (`sdata-generation.md`), this file is the easiest to generate: it is the server's own layout, and only the texts need
a `client` table of their own.

## From skill-monster-sdata-vs-defs.md (original "For the server" section, before it was made public)

## 7. For the server

Nothing to change. The visible differences, all the original's own:

- archers cannot learn Trap (the client does not offer it);
- Pollution Zone asks one skill point more in the client;
- Kimuraku's target bar (max HP ten times lower in the client);
- the Lv3 stat remedies show no bonus (mall, later);
- tooltips: Potential Nimble, Slow, Rapid Shot Lv3; cast bars of Healing, Healing Prayer and Recovery longer than the cast.

Worth a look in a live test: whether the client sends Pollution Zone with exactly 3 points, Gruesome Attack with a sword, and whether it
refuses to attack from the server's reach on the Aidion and Astaroth monsters (*live*).

**Client side checked (`CLIENT-FACTS.md` "SData differences: what the client does")**: the skill window's learn arrow uses
the client's job flags, levels and points (Pollution Zone needs 4 points in this client; an Archer cannot raise Trap past the level it
has); no weapon check was found before a skill is sent; Kimuraku's bar can be fixed from the server with `0x0608` (`u32 id, u32 maxHP,
u32 HP`), which overwrites the client's max HP.
