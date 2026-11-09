# Fire Thorns and fields hitting players

This page covers what the EP4.5 game server `ps_game.exe` does when Fire Thorns (skill 47, `wType` 84) and the
fields (Trap 113, `wType` 101; Fire Wall 48, `wType` 102) reach **players**: the player branches of `CUser::ChkProtect` and
`CZone::UpdateTrab`. The monster branches, the timers and the field damage types are on [Skills 4: special skills](skills-special.md).
It was read in the Ghidra decompiler and the disassembly with the PDB names. Nothing was executed.

Read: `CUser::ChkProtect` 0x486830, `CZone::UpdateTrab` 0x4226b0, `CZone::TrabDamage` 0x422480, `CUser::Damage` 0x44c1e0,
`CUser::RemApplySkillHide` 0x489290, `CZone::PSendView` 0x421220, and the calls around them. Not covered: what the client draws for
these packets, and the game log.

## In short

- **Fire Thorns** hurts every enemy player within its reach of the caster every three seconds, by a flat amount: no hit roll, no
  defence. "Enemy" is the usual rule: the other faction, or the caster's duel partner or guild-war enemy.
- **Fire Wall** hurts every player of another faction standing in it, every second. **A Trap** takes a share of the first victim's
  current HP and is gone, whether the victim is a player or a monster.
- **Fields go by faction only.** They never hit a duel partner or guild-war enemy of the owner's own faction, and never hit players on
  maps 40 and 42.
- **Hidden players are hit**: their Stealth ends first, then the damage lands.
- **A kill counts as a player kill** for the caster or the field's owner, as long as that player is still in the zone. A field whose
  owner left the map kills for no one.
- **A Fire Thorns kill stops whatever the caster was attacking or casting**, even a different target.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (the PDB's plus 4 from the map pointer on).

## 1. Fire Thorns on players

Confirmed, `CUser::ChkProtect` 0x486830.

Called from the three-second pass for each Fire Thorns on a living player (`docs/original/skills-special.md` section 7). After
`m_dwPrevTick = now`, it walks the **3 × 3 cells** around the caster's cell (clamped to the map), row by row, and in **each cell first the
players, then the monsters** (the monster branch is the one already built). A player in the cell is hit when all hold:

1. **alive**: state (`+0x142c`) is not 1;
2. **`CUser::IsTeam(target, caster)` is false** (`docs/original/pvp.md` section 1): the other country on a map without a war type, or the
   caster's duel partner while fighting, or an enemy in a guild war. So the caster itself, its country (outside duels and guild wars) and
   everyone on maps 40 and 42 (outside duels and guild wars) are never hit;
3. **attackable and visible**: `+0x5775` and `+0x5774` both set (a staff member who is untouchable or invisible is passed over);
4. **within `byApplyRange`** (`stSkillInfo + 0x57`) of the caster: the horizontal distance (x and z), `≤`.

**Hidden players are hit.** There is no shape check: a hidden enemy within reach is first **revealed** (`RemApplySkillHide`: its Stealth
ends, `0x0210 1`, the shape goes out), then hit.

The hit:

1. `RemApplySkillHide` on the target (does nothing unless it is hiding);
2. **`CUser::Damage(1, caster id, damage)`** with the skill's **`wDamage` HP, SP, MP as they are** (`+0x68`, `+0x6a`, `+0x6c`): no hit roll,
   no defence, no formula, `byDamageType` ignored (as for monsters). `CUser::Damage` is the same function monsters' hits use (section 3);
3. **when it kills**: the caster **stops attacking** (inline `StatusAttackStop` on the caster, when its state is neither 0 nor 1: state 0,
   no target, `m_nAttackMode` 0, `m_nAttackNext` 0, recovery state at least 1), then **`CUser::Death(1, caster id, false)`** on the target
   (section 4);
4. **`0x0513`** (20 bytes: `u8 result 0, u32 caster, u32 target, u16 skill, u8 level, u16 HP, SP, MP`) with the damage **as `Damage` left it**
   (section 3: give-up SP/MP and absorption change it) through `PSendView` around **the caster's position**, range **60**, naming the caster
   and the target (section 5). After the death's `0x0504` when it killed.

Nothing else: the target does not stand up, its cast does not break, sleep and transformations do not end (only hiding), no aggro, no party
entry, no game-log record. The caster's MP or SP are not spent again (the buff was paid when cast).

## 2. Fields on players

Confirmed, `CZone::UpdateTrab` 0x4226b0.

Every second, for each field of the zone, the 3 × 3 cells around the field's cell (clamped), row by row, in **each cell first the players,
then the monsters**. A player in the cell is hit when all hold:

1. **alive** (state not 1);
2. **its map has no war type**: `CMap::m_nWarType` (`+0x7c` of the zone's map) is not 1, so **fields never hit players on maps 40 and 42**;
3. **of another country than the field** (`CTrab + 0x34`, the owner's country when it was laid, against `+0x12d`). **Not `IsTeam`**: a duel
   partner or guild-war enemy of the owner's own country is **not** hit, and the owner (same country) never is;
4. attackable and visible (`+0x5775`, `+0x5774`);
5. within the skill's **`byApplyRange`** of the **field's position** (horizontal, `≤`).

A trap's state (`byStateType`) **does not matter for players**: the bounce test is in the monster branch only. Hidden players are hit and
revealed, as in section 1.

The hit:

1. `RemApplySkillHide` on the target;
2. **`TrabDamage`** (section 8 of part 4, unchanged) with the **target's current HP, SP, MP** (`+0x1238`, `+0x1240`, `+0x123c`) and its
   maxima: **type 0** `wDamage` as it is; **type 5** `trunc(wDamage[i] / 100 × current)`, at least 1 for a non-zero `wDamage[i]`; type 4 the
   same with the maxima (no field uses it); each at most 30000. Fire Wall: 32/50/80 HP; Trap: 15/25/35 % of the current HP;
3. **`CUser::Damage(1, owner id, damage)`**; **when it kills, `CUser::Death(1, owner id, false)`** on the target. **No attack stop** for the
   owner (unlike section 1). The owner need not be in the zone or even online: the field keeps its id;
4. **`0x0513`** (layout as section 1: result 0, **owner**, target, skill, level, the damage as `Damage` left it) through `PSendView` around
   **the field's position**, range 60, naming the owner and the target;
5. **a trap (101) is used up by its first victim, player or monster**: removed at once, nothing sent, and the pass moves to the next
   field. A fire wall (102) hits everyone in range every second.

Then the expired fields go, as before.

## 3. `CUser::Damage(kind, attacker, damage)`

Confirmed, 0x44c1e0.

The same function as for a monster's hit, which the new server already has (`zone::mob_skills::hurt_player_skip`): nothing for a dead
player or one with 0 HP; `QualityByDefense` (the armour wears); **`CDamage::Add(kind, attacker, HP damage)`** (the damage record: here kind 1,
the caster or the field's owner); SP and MP lose their parts (not below 0); give-up SP/MP (`+0x1338` 1, 2 or 3) pays the HP damage from SP
and/or MP first, moving what it paid into the SP/MP parts; absorption (`+0x12fc`, at least 1 HP left of the hit, `0x0525` to the player); HP at
or below the damage: HP, SP, MP 0, the party's HP bars, **returns true**; otherwise HP down, the party's bars, the low-HP passive
(`ChkHPAbilityApply`), returns false. The damage array is changed in place, and the callers send it.

## 4. The death

`CUser::Death(1, id, false)` is `docs/original/pvp.md` section 6 with the record: the killer of record (most HP damage, earliest on a tie)
decides. A player killer of record **in the victim's zone** gets the kill as any player kill (type 3 between countries: counts, the faction
kill count, items and gold by `ItemDropByUser`, the reward panels; duel and guild-war types 4 and 5 by their rules; `docs/original/duel.md`,
`docs/original/guild-war.md`). A killer of record **not in the zone** (a field's owner who left the map or logged out): nothing more, and the
victim loses experience at rebirth as after a monster. `0x0504` names the killer of record.

## 5. `CZone::PSendView`

Confirmed, 0x421220.

`PSendView(packet, size, position, range, id1, id2, kind)` visits the cells from `round((x − range) / 40)` to `round((x + range) / 40)` (and
the same in z), clamped, and in each sends to the players within `range` of the position **or whose id is `id1` or `id2`** (`CCell::PSendView`
0x424780; `kind` is not used for the choice). So the named caster or owner and the target get the packet even a little beyond 60, but only
while they stand in those cells (about 80 at most from the position). The new server sends within 60 only (deviation 1).

## 6. The new server now

- `Zones::keep_effects` → `Zones::thorns` (`zone.rs`): monsters only (living, not unbeatable, another country, within reach), `0x0519` within
  60 of the caster; a killed monster does **not** stop the caster's attack (part 4 section 12's note).
- `Zones::fields` (`zone.rs`): monsters only, with the trap's bounce test and first-victim rule; the `players` map is borrowed immutably and
  the pass is skipped for a map with no monsters.
- Neither takes the war-map list: `World::is_war_map` lives in `ctx.data.world`; the casts carry `war_map` (`Cast.war_map`).

## 7. Deviations (approved)

1. **`0x0513` goes to the players within 60 of the hit** (the caster's position for Fire Thorns, the field's position for a field), the
   victim included, through the existing `send_around`; nobody is named beyond that. The original also reaches the named caster or owner
   while they stand in the cells around (section 5, up to about 80): an owner 60 to 80 from its field misses the packet here. Fire Thorns
   is unchanged by this (the caster is at the centre). (A field's owner anywhere in the zone was proposed and not taken.)
2. **Order inside one pass**: the original walks cell by cell, players then monsters in each cell, in list order. The new server takes
   **all the players in reach first (ascending id), then all the monsters (ascending id)**, as its monster branches already sort ids. This
   only changes **which victim a trap takes** when a player and a monster are both in reach in different cells, and the order of the
   `0x0513`/`0x0519` packets in one tick. Approved.
3. **A Fire Thorns kill does not stop the caster's own attack** (players and monsters alike, as built for monsters now). The original's
   inline `StatusAttackStop` is the normal-attack kill rule copied into the aura: it stops **whatever** the caster was attacking or casting,
   so a mage fighting monster B loses its attack and its cast because the aura killed player A behind it. Kept the new server's way: the
   caster's attack tick finds its own target gone (or not) as usual. Approved.

Not deviations (the original's behaviour, built as it is): hidden enemies are hit and revealed; fields ignore duels and guild wars and never
hit on maps 40 and 42; Fire Thorns follows `IsTeam` (duel partner and guild-war enemies of one's own country are hit); a field's kill
while its owner is away counts for no one; staff need `visible` and `pvp` to be hit (`docs/original/gm-commands.md` deviation 15, the
split of `+0x5775`).

## 8. Build plan

All in `crates/game-server`. Keep the comment density and naming of `zone.rs`; cite this document's sections in doc comments.

1. **War maps into the passes.** `Zones::keep_effects(now, world: &World, defs, loot, rand)` and `Zones::fields(now, world, defs, loot, rand)`
   take `&World` (callers in `server.rs` around line 487 and 504 pass `&ctx.data.world`); each map's `war = world.is_war_map(map)`. Update
   every test caller.
2. **The target tests** in `zone/pvp.rs`. Factor the side part out of `attackable` as `pub(super) fn enemies(me: &Placed, target:
   &Placed, war_map: bool) -> bool` = `a != b && (duel::fighting(me, b) || guild_war::enemies(..) || !same_side(a, b, war_map))`, so that
   `attackable` = alive, visible, `pvp`, not hidden, and `enemies` (its behaviour unchanged). Add `pub(super) fn area_victim(target: &Placed)
   -> bool` = `!target.dead && target.occupant.visible && target.occupant.pvp` (**no shape test**: hidden players are hit). Fire Thorns:
   `area_victim(t) && enemies(caster, t, war)`. Fields: `area_victim(t) && !war && t.occupant.shape.country != field.country`.
3. **Fire Thorns** (`Zones::thorns`): `players` becomes `&mut HashMap<u32, Placed>` (the call site in `keep_effects` already collected
   `due` before calling). Before the monster part: the player victims (section 1 tests 1 to 4, `near_cells(p.cell, cell)`, horizontal
   `distance(...) <= reach`), ascending id (deviation 2). For each, still alive at its turn: `reveal(players, id)`; then
   `mob_skills::hurt_player_skip(&mut pass, mobs, players, (cp::KILLER_PLAYER, caster), id, s.damage, None)` with a `Pass { zones: self,
   map, defs, loot, now, rand }` — this already records the harm, wears the armour, pays give-up, absorbs (`0x0525`), kills through
   `pvp::player_died` (kill rewards, duel and guild-war rules, `0x0504` to the view) and returns the damage shown; then
   `0x0513` = `sk::skill_use(sk::OP_USE_PARTY, combat::RESULT_HIT, caster, id, s.id, s.level, shown)` through
   `send_around(Some(players), at, ATTACK_RANGE, ..)` around the caster's position (deviation 1). Do **not** call
   `mob_skills::struck`, `shape_and_sleep_end` or `casting_stop` (section 1: only hiding ends). No attack stop (deviation 3). If the
   caster died meanwhile (it cannot be hit by its own aura, but keep the guard), stop.
4. **Fields** (`Zones::fields`): `self.maps.lock()` mutably; run every map that has fields even without monsters (an empty `MobMap` as
   `keep_effects` does). For each field, before its monsters: the player victims (section 2 tests 1 to 5), ascending id. For each:
   `reveal`; `buffs::field_damage(s, current, max)` with `current = [hp, sp, mp]`, `max = [max hp, max sp, max mp]` (mind that
   `Vitals::health` and `max` are stored HP, MP, SP: reorder), then `hurt_player_skip` with `(cp::KILLER_PLAYER, f.owner)`; `0x0513` as above
   through `send_around` around `f.position` (deviation 1). **A trap is used up by its first victim**: after a player is hit, push `f.id` to `used`
   and skip the field's monsters. The existing monster part stays as it is.
5. **Docs**: in `docs/original/skills-special.md` replace "(not built: the new server hits monsters only, section 12)" in sections 7 and 8
   with a pointer here, and change section 12's first deviation to "built: `docs/original/thorns-fields-pvp.md`"; the same in
   `docs/original/pvp-2.md` section 11 (`docs/original/party.md` ~152 stays: still no party entry); README "Not built": drop
   "Fire Thorns and fields hitting players". Add a "Built" section here with the commit.
6. **Tests** (in `zone.rs`'s test module, beside `fields_hit_the_monsters_around` and the Fire Thorns test near line 4680; reuse their
   setup helpers):
   - Fire Thorns: an enemy-country player within reach loses `wDamage` HP and both get `0x0513` (`OP_USE_PARTY`, result 0, caster, target);
     one beyond reach and one of the caster's country are not hit; on a war map (`war = true`) nobody is; a hidden enemy is revealed
     (`0x050E` for its Stealth) and hit; a same-country duel partner while fighting is hit; staff with `visible` or `pvp` false is not.
   - Fire Thorns kill: the victim dies (`0x0504` naming the caster), a `ZoneEvent::PlayerKill` reaches the caster, `0x0513` comes after
     `0x0504`; the caster's `attacking` is unchanged (deviation 3).
   - Fire Wall: an enemy-country player in range loses 32 HP every pass; the owner's country, a same-country duel partner and anyone on a
     war map are not hit; a player kill with the owner gone from the zone kills with no `PlayerKill`.
   - Trap: a player victim takes `trunc(15 % of current HP)` (at least 1) whatever the trap's state type, and the trap is gone (a monster
     in reach afterwards is not hit); with no player, the old monster behaviour holds.
   - Absorption and give-up go through as for monster hits (one test with `vitals.absorption` set is enough).
7. **Run** `cargo test --workspace` and `cargo clippy --workspace --all-targets` (no new warnings), build release and restart the
   game server.
8. **Live test section 46** in `docs/testing/live-test-checklist.md` two clients:
   StaffChar (Fury, level 60) against MageChar (Light mage, knows Fire Wall 48) and TestChar (knows Trap
   113). Fire Thorns 47: give MageChar the skill at level 1 if it cannot learn it (an insert into `character_skills` while the server is
   stopped). Items: Fire Thorns hurts StaffChar standing next to MageChar every 3 s (StaffChar
   sees its HP drop and the hit), not a Light character; Fire Wall hurts StaffChar every second inside it; a Trap laid by TestChar takes
   15 % of StaffChar's HP once and disappears; a hidden StaffChar (Fury assassin Stealth, if it has it) is revealed by either; nothing on map
   42; a kill by Fire Wall counts as MageChar's kill (kill count up) while MageChar stands near.

## 9. Not covered

- What the client shows for `0x0513` from a buff or a field (the hit number, the animation): expected to work, as the original sends
  it; to be seen in the live test (`docs/testing/client-research-queue.md`).
- The game log records of these hits and deaths (with the rest of the game log).

## 10. As built

Built as section 8 describes, all in `crates/game-server`:

- `Zones::keep_effects` and `Zones::fields` take `&World` (`server.rs` passes `&ctx.data.world`); `thorns` takes `war` and `&mut` players.
- `zone/pvp.rs`: `enemies` (the side part of `attackable`, which keeps its behaviour) and `area_victim` (no shape test).
- Fire Thorns hit the enemy players first (ascending id), then the monsters; fields the same, with a trap used up by its first player
  victim. Both go through `mob_skills::hurt_player_skip` (harm record, armour wear, give-up, absorption, death by `pvp::player_died`), reveal
  hidden players first, and send `0x0513` through `send_around` within 60 of the caster or the field (deviation 1). No attack stop (3).
- Tests in `zone.rs`: `fire_thorns_hit_enemy_players`, `fire_thorns_kill_a_player_without_stopping_the_attack`,
  `fire_thorns_go_through_absorption`, `fields_hit_enemy_players`, `a_trap_takes_a_player_once_whatever_its_state`.
- Live test section 46 (`docs/testing/live-test-checklist.md`); MageChar was given Fire Thorns 47 (`character_skills` number 7). The open client
  question (what is drawn for `0x0513` from a buff or a field) is entry 3 of `docs/testing/client-research-queue.md`.

## 11. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) with this one,
read statically like the rest of the note; the overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those
builds are carried over from EP4.5 (matched by strings, constants or the call graph, as those pages list).

### EP3

- `CZone::UpdateTrab` (EP3 ps_game 0x41e630) has the same structure; EP4.5 has a few more constants (0x65, 0x50) (confirmed).
  `TrabDamage` (EP3 ps_game 0x41e400, matched by call position in `UpdateTrab`) has the same size and constants, cap 30000
  (confirmed).
- `CUser::Damage` (EP3 ps_game 0x4376d0, 0x437750) subtracts the damage directly: no absorption and no `0x0525` (confirmed).

### EP5.4

- `CZone::UpdateTrab` (EP5.4 ps_game 0x429090) and `TrabDamage` (EP5.4 ps_game 0x428ed0) make the same calls (hide removal,
  `CUser::Damage` and `Death`, `CMob::Damage`, `Death` and `SetTarget`), send the same packets (`0x0513` players, `0x0519` monsters) and
  keep the same constants; only the cap changed, 30000 → 60000 (EP5.4 ps_game 0x429066) (confirmed).
- The Fire Thorns aggro check is unchanged: the helper (EP5.4 ps_game 0x4580d0) returns no monster while the caster's status is 2
  (confirmed).
- `CUser::Damage` also subtracts a damage reduction that a skill (ability type 48) can now raise (read at EP5.4 ps_game 0x458148,
  confirmed; [`skills.md`](skills.md)).
