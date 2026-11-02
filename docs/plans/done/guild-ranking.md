# Build plan: the guild ranking battle (guild houses part 2)

Built; the behaviour is described in `docs/original/guild-ranking.md`.

Spec: `docs/original/guild-ranking.md`. Read it first, then part 1
(`docs/original/guild-house.md`, built) and the code it names below. Build all four proposed deviations of the spec's
section 10 as written; they were approved with the others.

## Steps (in this order; `cargo test -p game-server` green after each)

1. **Protocol** (`crates/shaiya-proto/src/guild.rs`, `world.rs`): builders for `0x0D26` (`u16 map, u8 type`, 5 bytes), `0x0D27`
   (`u32, u32, u32`, 14 bytes), `0x0D37` (`u8 count`, then `u32 guild, u32 score, u8 place` each, 3 + 9n); `OP_PORTAL_CONFIRM = 0x0227`
   with the same parse as `0x020A`. Length tests like the existing ones (`guild_added` is 122).

2. **The clock** (new `crates/game-server/src/ranking.rs`, pure, unit-tested; add to `lib.rs`): input the local time as
   `(weekday 0 = Sunday, hour, minute, date)` and the R map's open entries (first **two** only). Gives: `is_open`, the battle tag while
   open (`(date, start hour)` of the open entry), and the notice of that minute (spec section 3's table: `hour−1`:30 and :50 → the start
   hour, not for an entry starting at 0; `hour`:00 → 100; `end−1`:50 → 101; `end−1`:59 → 102; past-midnight entries' 101/102 on the next
   weekday). A `Battle` state machine stepped once per local minute: remembers "seen open"; on the first closed minute after that it
   reports **Closed** and sets the update due at now + delay; when due, reports **Update** once. Also the pure ranking (section 8):
   `score(points, returns, entries) = points + min(returns + 25 × entries, points / 2)`, 0 without points; and the per-country placing
   (highest score first; equal scores → higher guild id first). Tests: the open rule (reuse `InstanceRules::is_open`'s cases), every
   notice minute, the close and the due update, the score (e.g. 100 points, 3 returns, 4 entries → 150), places 1–30 and 31+.

3. **Config and data**: `config.rs` gets `GAME_RANKING_TIMES` (optional, e.g. `"(1,14,1)"` or several `(d,h,n)`; replaces map 50's
   `CreateTime`) and `GAME_RANKING_UPDATE_DELAY_SECS` (default 1800). `pg.rs` ~line 1891 loads `InstanceRules` only for create types 3
   and 6: add 5 (the R map's expire time and open times; R keeps two time entries).

4. **Guild state** (`guild_house.rs` `House` ~line 159, `guild.rs`): per guild `battle_points: u32`, `battle_entries: u32` (memory only;
   not loaded or saved); in `Guilds`: the best `(points, guild)` per country and a map character id → the tag of the last battle
   entered (deviation 3: per character, memory only). Methods: `count_entry(me, tag)`, `entered(me, tag) -> bool`,
   `add_battle_points(guild, n) -> (guild points, best points, best guild)` (spec section 7 steps 1–2; the best only when strictly
   above), and `apply_ranking(...)` (section 8: sets points, ranking, `use_house`, takes the keep Etin, zeroes the three counters; returns
   which guilds paid with their new Etin, and each country's `0x0D37` list). **The guild list** (`Guild::unit`, guild.rs ~line 109)
   must carry `house.ranking` and `house.points` instead of `UNRANKED` / 0.

5. **Database**: `db/migrations/0020_guild_ranking.sql` with a one-row state for deviation 4 (e.g. `guild_ranking (id smallint PRIMARY KEY
   CHECK (id = 1), due timestamptz)`). `backend.rs` `GuildWrite::Ranking { rows: Vec<(guild, points, etin, ranking, use_house)> }` (one
   transaction; also `etin_returns = 0` for every guild) and `GuildWrite::RankingDue(Option<time>)`; implement in `pg.rs` (~line 1354,
   the other `GuildWrite` arms) and `memory.rs` (~line 589). Load the due time at start-up; if it has passed, run the update right
   after the guilds load. Extend `crates/game-server/tests/postgres.rs` for the new write.

6. **Going in** (`server.rs` `portal_use` ~line 2485 and the `OP_PORTAL` arm ~line 1985; new `Zones::enter_ranking` in
   `zone/instance.rs` beside `enter_guild_house`): for create type `GUILD_RANKING`, in order: no guild → 0; closed → 1; the player is
   in an R zone already → a move **within its current zone** (portals 0–2 of map 50 lead to map 50: return `state.map`, not 50); has
   `entered(tag)` → 2; then, for `0x020A` → **reason 5** (deviation 2: the client asks), for `0x0227` → book into the guild's zone of
   map 50 (`instance::Owner::Guild`, found or made like the house zone) and move. A pending logout keeps the player in place, as the
   other branches. Keep the `Battle` state where both the portal code and the zone can read it (e.g. `Mutex<ranking::Battle>` in
   `Zones`, next to `guilds`).

7. **Arrival** (`Zones::enter`, zone.rs ~line 2613, where the player is unbooked): when the zone's map is of type R and the battle is
   open, `count_entry(player, tag)` (once per character per battle; it also records the tag).

8. **Points** (the kill pass, zone.rs ~line 2255, where `drops::roll` runs): in a zone whose map is type R, a monster with `money` 0
   and `money_add` above 0 drops **no gold** (items as usual); if the drop owner (`killer_owner`) has a guild,
   `add_battle_points(guild, money_add)` and `0x0D27` to that guild's online members who are in an R zone. Log it (debug: guild,
   mob, points, total).

9. **The minute task** (spawn in `main.rs` like `mob_updater`, or a per-minute step inside it): at each new local minute step the
   `Battle`. A notice → `0x0D26 (50, type)` to every player in the world. **Closed** → every player in a zone of map 50 is moved to
   `world.bind_pos(zone, country, ...)` (map 53 or 54) through a zone event like `ZoneEvent::Recalled` (skip players logging out),
   and write `RankingDue(Some(due))`. **Update** → `apply_ranking`, `GuildWrite::Ranking`, `RankingDue(None)`, `0x0D23` to each guild
   that paid, `0x0D37` per country (`Zones::send_country`). Info log lines: "ranking battle open", "ranking battle closed", "ranking
   notice", "ranking update" (with each placed guild), "ranking zone made".

10. **Docs**: the spec's section 10 says "built" where true; `docs/original/guild-house.md` deviation 1 marked done;
    `docs/original/guild.md` last bullet (built).

11. **Finish**: `cargo clippy` clean as the rest of the crate, `cargo test` (all, with the PostgreSQL tests), release build.

## Live test

Section 35 of `docs/testing/live-test-checklist.md`; it needs `GAME_RANKING_TIMES` and `GAME_RANKING_UPDATE_DELAY_SECS` set for a window
an hour or so ahead.
