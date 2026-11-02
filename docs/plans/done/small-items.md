# Build plan: small items (live-test fixes and the small unbuilt features)

Built; the behaviour is described in `docs/original/small-items.md`.

Spec: `docs/original/small-items.md`. Read it first (sections 1 to 12 are
the rules, 14 the deviations), then the client notes it names (`docs/client/CLIENT-FACTS.md`, `docs/client/gm-and-commands.md`; read only)
and the code named below. The maintainer decided on 2026-10-20: **deviation 19 of `docs/original/gm-commands.md` is withdrawn** (`F706` opens,
`F707` closes), **no global chat without the item** (deviation 2 not built), **`0x0559` kinds 1 and 2 are built with the attack checks**
(deviation 7). Deviations 1, 3, 4, 5 and 6 were **approved by the maintainer on 2026-10-20**.

**Migration: `0026_rename.sql`** (the only one; the latest now is `0025_market.sql`).

## Steps (in this order; `cargo test -p shaiya-proto` and `cargo test -p game-server` green after each)

1. **Protocol** (`crates/shaiya-proto`).
   - `world.rs`: replace `reset_flags(stat_reset: bool, skill_reset: bool)` by **`reset_counts(stats_left: u8, skills_left: u8)`** (the two
     bytes as they are; spec section 2); its test pins `00 01` and `00 00`.
   - New opcodes and builders, each with a one-line doc comment naming the spec section (put them where their family lives: `0x03xx` in
     `world.rs`, `0x05xx` in `item.rs` or `world.rs` as the existing `0x05xx` consts are, a new `priest.rs` for `0x0Fxx`):
     `OP_USER_ITEMS 0x0307` with `parse` = `parse_char_id` and `user_items(&[ItemUnit])` (`3 + 32 n`: `u8 n`, then per item `slot, type, type
     id, u16 quality, gems[6], craft name[21]` zero-padded; at most 13); `OP_VG 0x0555` with `vg(i16)` (4 bytes); `OP_TOWN 0x0556` (no
     payload); `OP_PRIEST_START 0x0F01` with `priest_start(Option<[u16 hg, i16 vg, u8 cg, u8 og, u8 ig]>)` (10 bytes with `u8 1`, 3 bytes `u8 0`)
     and `OP_PRIEST_NEXT 0x0F02` with `priest_next(bool)` (3 bytes).
   - `chat::by_name(OP_ALL_SERVER, …)` already builds `TP_CHAT_SENDALL`: add a test that pins `len + 24` and the zero padding of the name.
   - Tests: every size above; 13 equipment units → 419 bytes; an empty list → 3.

2. **Kick and ban notices held** (deviation 1; `config.rs`, `server.rs`).
   - `Config::kick_notice: Duration` from **`GAME_KICK_NOTICE_SECS`** (default 3; 0 = close at once). Tests set it small (e.g. 300 ms).
   - `Connection::kick_out`: send the packet as now; for `Kick::Operator { op != 0 }` and `Kick::Banned` **don't shut the socket down here**:
     remember that a hold is due (`closed`, `kicked` set as now, so nothing more is sent or read for the game). `Kick::NewLogin` and
     `Operator { op: 0 }` shut down at once as now.
   - After the world is left and the `Online` dropped (where `conn.drain()` is called now, `server.rs` ~line 779): when a hold is due, keep
     reading and dropping for `kick_notice` (stop early when the client closes), then `shutdown()`, then the usual `drain()`.
   - `game_flow` tests: `/kick` → the target gets `F804`, its connection is still open after half the hold, closed after the hold plus a
     margin; the character is out of the world (another client got `0x0202`) before the hold ends; a newer login of the kicked account during
     the hold works. Same for a ban notice. `Kick::NewLogin` closes at once.

3. **`/server`, F706/F707** (deviation 19 withdrawn; `server/admin.rs` ~line 154).
   - `F706` sets accepting on, `F707` off, both idempotent; the comment names `docs/original/small-items.md` section 4. Keep deviation 20 (the
     heartbeat at once). Fix the existing test that expects the flip. In `docs/original/gm-commands.md`'s small-choices list, nothing else
     changes (`FA0C` stays `F501 FA0C`).

4. **Reset counters and the invisible-staff log** (spec sections 2 and 3; `enter.rs`, `zone.rs`).
   - `enter.rs`: `w::reset_counts(0, if c.free_skill_reset_used { 0 } else { 1 })`.
   - `zone.rs`: a `tracing::debug!` "staff hidden from viewer" (staff id, viewer id, path `enter` / `view` / `char`) wherever an invisible
     staff member's `shape_packet` (13) goes to a viewer: `Zones::enter` (both directions), the cell change in `moved` (both directions),
     `set_gm_flags`. No behaviour change; the existing zone tests stay green.

5. **Message To Server and `0x1108`** (spec section 5; `item_use.rs`, `server/items.rs`, `server.rs` chat handler, `server/admin.rs` `F101`).
   - `item_use::Effect::MessageToServer` for special 38 (out of the `NotBuilt` arm); special **25 becomes `Effect::Refused`** (spec section 10).
   - `WorldState`: `chat_item_armed: bool` and `next_all_chat: Option<Instant>` (cleared/fresh with every world entry; kept over map changes).
   - The item: armed or before `next_all_chat` → `Done::Failed` (error answer, reuse time put back, item kept); else armed and used up.
   - Chat: `OP_NORMAL` while armed → `ctx.zones.send_all(&chat::by_name(OP_ALL_SERVER, name, text))`, **nothing else** (no `send_near`,
     no echo to the sender: the sender gets the `0x1108` from `send_all`), disarm, no cooldown. `OP_ALL_SERVER` (replaces the "not available"
     log): ignored unless armed and past `next_all_chat`; then the same, disarm, `next_all_chat = now + 30 s`. Info log "all-server chat"
     (id, text as the chat log does). `F101` (`server/admin.rs` ~line 652) while armed: the same `0x1108`, disarm, no `F101` to the view.
   - Tests: `item_use` unit test for the effect; `game_flow`: two clients on different maps of different factions; A uses a Message To
     Server (give it in the test's store), types a normal line: B gets `0x1108` with A's name, **A gets the `0x1108` too and no `0x1101`**, a
     third client next to A gets no `0x1101`; A's second item use while armed → `0x050A` error; the next line is a normal `0x1101`; a
     client-sent `0x1108` without the item → nothing.

6. **`/town`** (spec section 6; `server.rs`, `server/items.rs`).
   - `MoveKind::Town` in `ItemMove` (make the item part, `bag`/`slot`/`special`/`armed`, optional or use a marker the item checks skip), so the
     shared slot (`state.item_move`) is the original's single pending move: `0x0556` is ignored while dead, while `state.called` is set or while
     `state.item_move` is set; otherwise a town move due `Config::town_delay` later (**`GAME_TOWN_SECS`**, default 10; tests short).
   - `cancel_item_move` drops a town move **silently** (no `0x0222`, no reuse time); every existing cancel point stays as it is.
   - When due: `ctx.data.world.bind_pos(state.map, country, state.position, …)` and `move_player` exactly as the Return skill (~line 3326),
     without its logout check (spec: none). The personal shop ends first (`ctx.zones.shop_end`).
   - Tests: `game_flow`: `/town` then wait → `0x020B` to the bind point; `/town`, a step → no move and no `0x0222`; dead → nothing; `/town`
     twice → one move.

7. **Inspect `0x0307`, VG `0x0555`, Priest `0x0F01`/`0x0F02`** (spec sections 7, 8, 12; `server.rs` world dispatch, `zone.rs`).
   - `0x0307`: `Zones::inspect(map, asker, target) -> bool`: the target in the asker's zone, `!target.occupant.staff`, same country → send the
     target's connection **`ZoneEvent::Inspect { asker }`**; its handler builds `user_items` from bag 0 slots 0 to 12 of `state.inventory`
     (quality as now, gems, craft name) and sends it with `ctx.zones.send_to_char(asker, …)`. Otherwise nothing. Test: same faction gets the
     list (slot order, 32-byte units), an enemy or staff target gets nothing.
   - `0x0555`: `vp::vg(character.vg)` straight back (the current value: if `/set VG` changes it, use where that change is kept).
   - `0x0F01`/`0x0F02` (only in the world): gold `> 10` → gold − 10 through the money write the shops use (`ItemStore::trade(id, money, &[])`
     or its money-only form), then `priest_start(Some([hg, vg, cg, og, ig]))` / `priest_next(true)`; otherwise the `u8 0` answers. Any other
     `0x0Fxx` closes the connection (spec 12). Tests: 11 gold → answer and 1 left; 10 gold → `u8 0` and still 10; `0x0F03` closes.

8. **`0x0559` kinds 1 and 2** (deviation 7; `server/items.rs` ~line 102, the zone's skill-hit code).
   - Replace the "item skill on a target: not built" arm: kind 1 → the player target passes **`Zones::may_attack_player(map, id, target,
     war_map)`**, has no personal shop, and is within the skill's range as the skill code checks a player target; kind 2 → the monster passes
     **`Zones::attackable(map, mob, country, defs)`** and is within range (`combat::in_range`, as for a skill on a monster). Then one hit as an
     **area hit** (`single` false: `0x0513` on a player, `0x0519` on a monster) with **no item options**, using the zone's existing skill-hit
     functions (`docs/original/pvp-2.md` section 3, `docs/original/skills-use.md` section 8); the item used up. Anything else → `Done::Failed`.
   - Tests: a zone or `game_flow` test with a test item naming a damage skill: an enemy in range is hit and the item used; an ally, a target
     out of range, a hidden target and a monster of the user's own country are refused with the item kept. (The only real item, 44/147 with
     skill 193 level 1, does no damage.)

9. **The name change** (spec section 9; migration, `backend.rs`, `pg.rs`, `memory.rs`, `server.rs` character screen, `server/items.rs`,
   `guild.rs`, `market.rs`).
   - **`db/migrations/0026_rename.sql`** (comment it after the original's, as other migrations do): `ALTER TABLE characters ADD rename_count
     smallint NOT NULL DEFAULT 0 CHECK (rename_count BETWEEN 0 AND 255)`, `UPDATE … SET rename_count = 1 WHERE rename_allowed`, `DROP COLUMN
     rename_allowed`; `ADD old_name varchar(20) NULL`; `CREATE TABLE character_renames (id bigserial PRIMARY KEY, server_id smallint NOT NULL,
     character_id integer NOT NULL REFERENCES characters (id), old_name varchar(20) NOT NULL, new_name varchar(20) NOT NULL, renamed_at
     timestamptz NOT NULL DEFAULT now())` with an index on `character_id`.
   - `pg.rs` list query (~line 199): `rename_count > 0 AS rename_allowed` (the list's byte 76 stays as it is built).
   - Backend: `fn add_rename(&self, character_id: u32, item: &SlotChange) -> Result<bool>` (one transaction: `rename_count + 1` only when
     below 255, and the stone's slot change; `false` = refused, nothing changed) and `fn rename(&self, user_id, server_id, character_id,
     new_name: &str) -> Result<RenameOutcome>` (one transaction: the character live, of that account and server, `rename_count > 0`; name,
     `old_name`, `rename_count − 1`, the `character_renames` row, `market_auctions.seller_name` and `bidder_name` for that character;
     `RenameOutcome::{Renamed { old }, NoRename, Taken, NotFound}`; a unique violation on `characters_name_key` is `Taken`). `memory.rs` the same.
   - The stone (`Effect::NameChange` for special 51): once per world entry (`WorldState::rename_stone_used`), then `add_rename` with the
     item's count change; `false` → `Done::Failed` (deviation 6); success → used up (the store did the slot change: follow how the other
     store-written effects avoid writing the slot twice, e.g. the resets).
   - `0x010E` (`server.rs` ~line 990): one at a time (`Player.rename_pending` or just sequential: the handler awaits the store, so a second
     request is read only after the answer), the name through the **creation name rule** (`creation.rs`, 2 to 18 ASCII letters or digits;
     deviation 3), refused with `0x010E 2` when the character is still in the world (`ctx.zones` finds it: a lingering one) or the store says
     anything but `Renamed` (deviation 4); `Renamed` → `0x010E 1`, then `ctx.zones.renamed(character_id, &new_name)` updating the guild member
     list (`Guilds`, the member's `name`) and the market (`Market` auctions' `seller_name` / `bidder_name`) (deviation 5). Info log "renamed"
     (id, old, new).
   - Tests: the creation-rule refusals; `postgres.rs`: add a rename, rename (old name kept, count back to 0, log row), a taken name (any case,
     also on another server) → `Taken`, no credit → `NoRename`, 255 → `add_rename` false; `game_flow`: stone → list byte 76 is 1 → `0x010E` →
     `1` and the next list shows the new name; a second stone in the same world entry → error.

10. **Docs and finish.**
    - Spec section 14: mark each deviation "built" (1, 3 to 7) and section 1 to 12 "built" where they say "to build"; record any small choice
      you made (as `docs/original/auction.md` section 15 does). `docs/original/item-use.md` section 11 ("Not built"): specials 25, 38, 51 and
      `0x0559` kinds 1 and 2 point to the new spec. `docs/original/gm-commands-2.md` deviation 6 and `docs/original/gm-commands.md` deviation 18:
      one line each pointing to deviation 1 of the new spec.
    - `cargo test` (whole workspace) and `cargo clippy -p game-server` clean; the release binaries of **both** `game-server` and
      `login-server` (the login server runs the migrations too and must know 0026; the restart applies it).
    - Section 37 of the checklist: the `/server off` item reworded for the withdrawn flip.

## Live test

Section 41 of `docs/testing/live-test-checklist.md`; its setup gives the first player 3 Message To Server (100/64, item 100064) and
2 Name Change Stones (100/136, item 100136) in free bag 1 slots.
