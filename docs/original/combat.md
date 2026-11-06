# Fighting monsters: normal attacks, damage, death, experience, level-up, drops and picking up

This page covers a player's normal attack on a monster in the EP4.5 game server `ps_game.exe`: starting the attack,
the attack timer, hit or miss, damage and critical hits, the monster's death, experience and level-up, drops, and picking items up. It
comes from static analysis; nothing was executed. The client's side is on [World packets](../client/client-world-packets.md).

Not covered here: skills (see [Skills 2: using skills](skills-use.md)), monsters fighting back (see
[Monsters fighting back](mob-fight.md)), parties (see [Parties](party.md)), and the scripted monsters (`AI` 10 and
above; see [Scripted monsters](scripted-mobs.md)).

## In short

- **Attack speed sets the pace.** A character swings every 1 to 4 seconds depending on its attack speed (0 to 9).
- **Hitting** depends on both levels and on hit against evasion; the chance is always between 20 % and 99 %.
- **Damage** is attack minus defence, scaled by the level gap and by the elements (a 9 × 9 table: fire against water and so on). A
  critical hit does 1.5 times as much and wears the weapon. A hit that would do 10 or less does a random 1 to 10.
- **Experience** goes to whoever did the most damage, with a 20 % bonus. Monsters 6 or more levels below the player give nothing. Each
  level gives 5, 7 or 9 stat points (by mode) and 3 to 5 skill points, and fills HP, SP and MP.
- **Drops.** Each monster has up to nine drop entries with their own chances, and a 30 % chance of gold. Drops lie on the ground for 60
  seconds; for the first 30 only the killer can pick them up.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s base
stats on; see [Character numbers](stats.md)); `CMob` offsets match the PDB.

"rand()" is the C library's (0 to 32767); "trunc" is the x87 store with the rounding mode set to chop. The original mixes single
precision stores with extended precision arithmetic, so a recomputation in single precision can differ by one at exact boundaries.

## 1. Starting to attack: `0x0503`

Confirmed, `CUser::PacketPC` case 0x46a7d4.

Client → server, 6 bytes: `u32 monster id`. Ignored when the player is dead (status 1) or in status 3, has attack speed 255 (stunned and the
like), sits (motion 1), rides a mount, or already attacks this monster. Then `CUser::EnableAttackMob`: the monster must be in the player's zone,
not dead, and of another country than the player (`stMOBInfo.nCountry` ≠ the character's country; 2 = neutral, attackable by all). If not, only
the move is cancelled. Otherwise: if the player was in status 0 it gets **`0x0212`** (2 bytes, no payload, `CUser::SendAttackStart`) and its
recovery state becomes "fighting"; status 2 (attacking), target = (monster, id), normal attacks next.

## 2. The attack tick

Confirmed, `CZone::UpdateUserAttack` every 100 ms → `CUser::UpdateAttack` 0x48ca50.

Nothing until the player's next attack time. Then, in status 2:

- Attack speed 255: stop (status 0), `0x0502` result 10.
- No weapon in equipment slot 5: stop, **`0x0502`** to the player only, 17 bytes: `u8 result` (7), `u32 player id`, `u32 0`, `u16 0, 0, 0`.
- The target is no longer attackable (section 1): stop, nothing sent.
- Out of range (below): next attack time as after an attack, stop, `0x0502` result 5.
- Otherwise the attack (section 3); if it killed the monster, stop.
- **Next attack time = now + `g_ConstUserAttackSpeed[attack speed]`** = 4000, 3000, 2750, 2500, 2250, 2000, 1750, 1500, 1250, 1000 ms for
  attack speeds 0 to 9 (0x5424e0).

"Stop" (`CUser::StatusAttackStop`): status 0, no target, recovery state back to normal.

**Range** (`CUser::EnableAttackRange` 0x44c030): horizontal distance from the player to the monster's current position (interpolated while it
walks, `CMob::GetCurrentPos`) ≤ **slack + monster `bySize` + the player's range (`m_nAttackRange`, weapon and accessories `byRange`) + 2.3**,
slack (`GetAttackEnableRange`) = `g_ConstUserMoveSpeed[move speed] × 3` (`{3, 4, 5, 6, 7}` for move speeds 0 to 4, so 15 at speed 2), scaled
by `elapsed / 1000` when the player's last move was at most 1000 ms ago, and at least 2.

## 3. One attack

Confirmed, `CUser::AttackMob` 0x44ce10.

The attack kind is **1 (ranged) for weapons of item types 11, 13 and 14** (`nRealType` 11, 13, 14 via `LITEM_TYPE`; the types with ranges up
to 25), otherwise 0 (melee). Magic (2) is only used by skills. If the player's kind is disabled (`m_pAttack[kind].nDisable`), nothing happens.

1. The monster wakes up if asleep.
2. **Hit or miss** (`CUser::AttackSuccess` 0x44d010): a miss when the monster is unbeatable or going home (state 3). Otherwise, with `Lu`, `Lm`
   the player's and the monster's levels and the numbers of section 4 for the kind:
   `rate = trunc(160 × Lu / (Lu + Lm) − 100 × mob.hit / (player.evasion + mob.hit) + 100 × player.hit / (mob.evasion + player.hit))`,
   clamped to 20..99 (0/0 counts as 20); **hit when `rand() % 100 < rate`**.
   A miss: **`0x0503`** (17 bytes: `u8 result` 3, `u32 player id`, `u32 monster id`, `u16 0, 0, 0`) to the players within 60 (`CZone::PSendView`),
   and the attack ends.
3. **Damage** (`CUser::AttackDamage` 0x44d540), into `damage[3]` (HP, SP, MP; only HP is used by normal attacks), result 0:
   - level factor `P = clamp((100 × (Lm − Lu − 1) + Lm) / 100 × 10, 1, 95)`;
   - `A = player.attack[kind]`; plus `rand() % m_nItemAttackVar` when that is above 0 (weapon effect 2, section 4);
   - ranged only: `A × 0.7` when the monster is within 3 (and `m_nCrossRoad` is 0);
   - element `E = m_AttackAttr[player attack element][monster element]` (9 × 9 table below) when both are below 9, else 1;
   - `R = rand() % LUC + 1` (0 when LUC is 0); `D = mob.defence[kind]`;
   - **`damage = (A − A × P / 100) × E − (D + D × P / 100) + R × 0.1`**, at least 0;
   - **critical** (`CUser::CriticalSuccess`): chance `trunc(player.critical / 100 × 100 − monster LUC × 0.034)` clamped to 5..99; when
     `rand() % 100 <` chance: `R2` like `R`, **`damage = damage × 1.5 + R2 × 1.4 − R2 × 1.4 × P / 100`** (ranged within 3: × 0.7 again), result 1;
   - `damage[0] = trunc(damage)`, at most 30000; **at most 10 becomes `rand() % 10 + 1`**.
4. A **critical** blow wears the weapon (`CUser::QualityByAttack`; `AttackDamage` returns 1 only for a critical; `docs/original/repair.md`).
5. **`CMob::Damage`** (0x490ee0): nothing for a dead monster or one with 0 HP. HP −= `damage[0]`; at or below 0 the monster dies: HP 0, state 2,
   no target, body removal at **now + 5 s (10 s when its definition has a respawn time)**. SP and MP likewise (floored at 0). The damage is
   recorded per attacker (`CDamageMob::Add`: kind, id, damage summed, aggro += REC × 10, party).
6. On a kill: stop attacking, then **`CMob::Death`** (section 6).
7. **`0x0503`**: `u8 result` (0 normal, 1 critical), `u32 player id`, `u32 monster id`, `u16 damage[0], damage[1], damage[2]`, to the players
   within 60. The client subtracts the damage itself.
8. Not killed: the monster targets the player (`CMob::SetTarget`) and is "in battle" (fighting back: not covered here).

Element table `CGameData::m_AttackAttr` (0x542260, static), rows the attacker's element, columns the defender's:

| | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 1.0 | 0.8 | 0.8 | 0.8 | 0.8 | 0.7 | 0.7 | 0.7 | 0.7 |
| 1 | 1.2 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.4 | 1.0 | 1.3 |
| 2 | 1.2 | 1.4 | 1.0 | 0.5 | 1.0 | 1.3 | 1.0 | 0.4 | 1.0 |
| 3 | 1.2 | 1.0 | 1.4 | 1.0 | 0.5 | 1.0 | 1.3 | 1.0 | 0.4 |
| 4 | 1.2 | 0.5 | 1.0 | 1.4 | 1.0 | 0.4 | 1.0 | 1.3 | 1.0 |
| 5 | 1.3 | 1.0 | 0.5 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 |
| 6 | 1.3 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.5 | 1.0 |
| 7 | 1.3 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 | 0.5 |
| 8 | 1.3 | 0.5 | 1.0 | 1.6 | 1.0 | 0.5 | 1.0 | 1.4 | 1.0 |

The player's attack element is the weapon's `byAttrib` (column `attrib`; lapis can change it, `CUser::SetAttrAtk`, `docs/original/lapis.md` section 2), the monster's its `Attrib`.

## 4. The numbers

**Player** (`CUser::SetAttack` 0x454280, after every change of stats or equipment); `L` = level, stats are the totals with equipment, `add` the
passive skill and buff additions (0 without skills):

| kind | hit | evasion | attack | defence |
|---|---|---|---|---|
| 0 melee | `trunc(add + L + DEX × 0.5)` | `trunc(add + LUC × 0.125 + DEX × 0.5 + L)` | `trunc(STR × 1.3 + add + itemAttack + DEX × 0.2)` | `add + REC + itemDefence` |
| 1 ranged | `trunc(add + L + DEX × 0.5)` | as melee | `trunc(LUC × 0.3 + add + itemAttack + STR + DEX × 0.2)` | `add + REC + itemDefence` |
| 2 magic | `add` | `add` | `trunc(INT × 1.3 + WIS × 0.2 + add + itemAttack)` | `add + itemMagicDefence + WIS` |

Critical (all kinds) `trunc(LUC × 0.19 + m_nCritical)`.

**Equipment** (`CUser::ItemEquipmentAdd` 0x454c70). The item definition's quality and effects come in three wear levels
(`CGameData::LoadConstItem`): `wQuality = (Q, trunc(Q × 0.5), trunc(Q × 0.1))`, `wEffectN = (E, trunc(E × 0.5), 0)` for the columns `quality`
and `effect1` to `effect4`. An equipped item's level is **0 while its quality is above `trunc(Q × 0.5)`, 1 while above `trunc(Q × 0.1)`, else 2**.

| slot | adds |
|---|---|
| 5 weapon | `m_nAttackRange += byRange`; attack speed (`docs/original/stats.md`); **`itemAttack += effect1[level]`, `itemAttackVar += effect2[level]`** |
| 8 to 12 accessories | range, attack and move speed; `itemAttack += effect1`, `itemAttackVar += effect2`, `itemDefence += effect3`, `itemMagicDefence += effect4` (full values) |
| others (armour) | **`itemDefence += effect3[level]`, `itemMagicDefence += effect4[level]`**; slot 6 also `itemDefence += m_byShieldValue` (shield mastery, `docs/original/stats.md` section 2; a shield has no block value) |

**Monster** (`CMob::SetAttack` 0x4902f0, at creation; from its definition, `add` 0 without buffs):

| kind | hit | evasion | attack | defence |
|---|---|---|---|---|
| 0 | `trunc(DEX × 0.5 + add + Lv)` | `trunc(DEX × 0.5 + Lv / 2 + LUC × 0.125 + add)` | `add` | `trunc(Defense × 1.0 + add)` |
| 1 | `trunc(DEX × 0.5 + add + Lv)` | `trunc(DEX × 0.5 + LUC × 0.125 + add + Lv)` | `add` | `dwDefenseAddSH + add + Defense` |
| 2 | `add` | `add` | `WIS + add + Lv` | `Magic + add` |

Critical `trunc(LUC × 0.19)`. (`dwDefenseAddSH` is not a column; 0 for the monsters here.) The monster's own attacks use its three attack
entries (fighting back, not covered).

## 5. Experience and level-up

**`CMob::GetExp(level)`** (0x491690): `d = Lm − level`; above 9 nothing; else **`trunc(Exp × ((d + 1) × 0.2 + 1.0))`**, 0 when negative (so
monsters 6 or more levels below give nothing). An administrator's double experience multiplies it (not covered).

The killer gets **`trunc(GetExp × 1.2)`** (`CMob::Death`) through **`CUser::AddExpFromUser`** (0x458240):
- nothing when 0, or at the maximum level: **30 in easy mode (grow 0), 60 otherwise**;
- premium bonuses (×1.5, ×2.0, +0.2) not covered;
- `exp += gain`; while `exp ≥ T[grow][level]` (the running totals of `docs/original/enter-world.md`) the level goes up (`LevelUpStatus`); at the
  maximum level `exp = T[grow][level − 1]`;
- no level-up: **`0x0207`** to the player, 10 bytes: `u32 gain, u32 0`.

**`CUser::LevelUpStatus`** (0x48a5e0), per level: level + 1; the class stat + 1 (base and total): fighter STR, defender REC (+5 max HP), ranger
DEX (+5 max SP), archer LUC, mage INT, priest WIS (+5 max MP) — jobs 0 to 5; **stat points + 5 (easy, normal), + 7 (hard), + 9 (ultimate);
skill points + 3, + 3, + 4, + 5**; max HP/SP/MP: the status table row of the old level out, the new one in (× 1.1 hard, × 1.5 ultimate).

**`CUser::LevelUp`** (0x48ad70), once after the levels: the attack numbers again; `0x0207` with the gain; then **`0x0508` when the player itself
made the kill, `0x051E` otherwise** (20 bytes: `u32 id, u16 level, u16 stat points, u16 skill points, u32 T[grow][level − 1], u32 T[grow][level]`)
to the players within 60; **`0x051F`** (12 bytes: `u32 id, u16 max HP, max MP, max SP`) within 60; HP, SP and MP **filled to the maximum** with
**`0x0505`** (12 bytes: `u32 id, i16 HP gained, MP gained, SP gained`) within 60 when something changed. The client
(`docs/client`) plays `0x051E`'s effect at once; `0x0508` sets the level and keeps the effect pending until the hit that caused it is shown
(the killing blow's `0x0503`, a skill, a quest end), which is why the killer gets `0x0508`: its level-up packets arrive before the blow.

## 6. Death

Confirmed, `CMob::Death` 0x4910f0, `CZone::UpdateMobDeath` 0x421a80.

The **killer of record** is the attacker with the most recorded damage (`CDamageMob::GetMaxDamage`). **`0x0606`** (11 bytes: `u32 monster id,
u8 killer kind` 1, `u32 killer id`) goes to the monster's view (5 × 5 cells). If that player is in the zone (and not in a party): the experience
of section 5, the drops of section 7 with that player as owner, quest kill counts. A monster that went home in between (`m_bFairCombat` cleared)
only sends `0x0606`. The damage list is cleared.

The body stays until its removal time (section 3); then (`UpdateMobDeath`, every second) **`0x0602`** to the view, out of the zone, and its
spawn entry: if the entry was full, its respawn time becomes **now** (respawn time 0), **now + respawn time ± up to 60 minutes** (`rand() % 120 − 60`
minutes, respawn codes 9 and above, 4 hours or more) or **now + respawn time**; then the count goes down by one (`docs/original/mobs.md`).

## 7. Drops

Confirmed, `CMob::ItemDrop` 0x4a5ea0 and 0x4a65a0.

The owner is the killer of record (0 without one).

1. Quest item (`QuestItemType`/`QuestItemTypeID`): built, `docs/original/quest-drops.md` sections 1 to 3.
2. **Items**: the monster's drop entries 1 to 9 (`defs.mob_items`, `ItemOrder` = the entry, `Grade`, `DropRate`; entry 0 is never used): for
   each with a grade, entries 1 to 6 drop when **`rand() % 100 < rate`**, entries 7 to 9 when **a random number below 1,000,000 < rate**
   (`SRandom::Random`). The item is a random one of the grade (`CItemDrop::GetItem`: `rand() % count`; the grade's items are every item with
   `Grade` 1 to 1024, type and type id not 0 and real type not 27; items with `Drop` above 0 can drop only that many times with delays, not
   covered: they do not drop here). One of it is created (`CItem::Init`, quality as bought, make type `'D'`) on the ground with random craft
   stats (`CItem::CreateCraftExpansion`, `docs/original/item-origins.md` sections 3 and 4).
3. Sequence items (`GetSequenceItem`, `SequenceItemCreate.ini`, monsters with `Money1` above 0): built, `docs/original/quest-drops.md` section 4.
4. **Gold**: with chance **30 %** (`rand() % 100 < 30`): `Money1 + rand() % Money2` (without the second part when `Money2` is 0); when above 0,
   a gold item (type 26, type id 0, count 1, the amount) on the ground.

**No number of the killer changes a drop chance or count** (confirmed, 0x4a5ea0 and 0x4a65a0, rechecked 2026-11-05): no stat (LUC
included), level, buff, kill-count grade, premium state or event rate. The chances and counts are the monster's definition alone; neither
function reads a rate global (the one global of 0x4a65a0, `g_ServerInfo` 0x542ca0, is the server byte of the new item's id), and
`CMob::Death` (0x4910f0) calls 0x4a5ea0 once per death (0x491263 a killer without a party, 0x49149c with its party, 0x491566 no killer).
What the killer's side is read for: its id (the owner), its zone and map (`CMob::EnableApplyZoneItem` 0x4a6560, who in a party gets an
item), **its quests and its party members' quests** (only whether the quest item may drop, `docs/original/quest-drops.md` section 3.1),
**its guild** (the ranking-zone points of step 3, 0x4a6182), and in a party each member's zone, map, distance (100), state and **gold
bonus** for its gold share (0x4a6311 to 0x4a642e: `m_bSkillMobGold` + 20 or 50 for a charm of level 2 or 3, `docs/original/item-flags.md`
section 4). Without a party the gold lies on the ground and the same bonus is added when it is picked up (`CUser::ItemGet`). So a gold
charm raises how much gold a kill gives, never whether anything drops.

**On the ground** (`CZone::EnterItem` 0x417320): at the monster's position plus `rand() % 1000 / 1000 − 0.5` on x and on z, **y = 0**; a new
object id (the items' own counter); removal at **now + 60 s**; owner as above. **`0x0401`** (22 bytes: `u32 id, u8 kind` 2 for a fresh drop,
`u8 type, u8 type id, u8 count, f32 x, y, z`) to the 5 × 5 view. In a player's view (`CCell::SendCellInfoEnter`): `0x0401` with kind 0, after the
NPCs; leaving the view `0x0402` (`u32 id`). Removal (`CZone::UpdateItem`, with the monster pass): `0x0402` to the view.

## 8. Picking up: `0x0205`

Confirmed, `CUser::ItemGet` 0x45d2e0, `CZone::GetPickItem` 0x4174a0.

Client → server, 6 bytes: `u32 item id`. `GetPickItem`: an unknown item is error 3 (silent). Someone else's item (owner set and not the player;
a party share is allowed) is error 0 ("doesn't belong to you") **until 30 s before its removal** (the first 30 s); with the pick check on
(`g_bChkItemPick`, on by default) an item more than **10** away is error 4; quest items need the quest (error 2, built: `docs/original/quest-drops.md` section 3.3). Otherwise it leaves the ground
(`0x0402` to the view).

Errors: **`0x0205`** 4 bytes `u8 0, u8 code`.

- **Gold**: amount (plus premium bonus, not covered); if the money would pass 2³² − 1 the item goes back on the ground (kind 1, 60 s again) and
  error 5. Else money += amount and **`0x0205`** 16 bytes: `u8 bag 1, u8 slot 0, u8 type 26, u8 type id, u8 count, u16 amount & 0xFFFF,
  u8 (amount >> 16) & 0xFF, u8 amount >> 24, u8 0 × 4, u8 0`.
- **Stackable item** (definition `byCount` above 1): first onto stacks of the same item in bags 1 to the bag count, slot order, up to `byCount`;
  each stack changed gets `0x0205` 16 bytes (`bag, slot, type, type id, count` the stack's new count, `u16 0`, 6 × 0, `u8 0`).
- **The rest** into the first empty slot (bags 1 to the bag count): `0x0205` with `bag, slot, type, type id, count, u16 quality, gems[6],`
  the craft name with its terminating zero (16 bytes with an empty name; stackables always the 16-byte form with quality and gems 0).
- No room left: error 1, the item goes back on the ground (kind 1, 60 s again).

## 9. The new server

- `crates/game-server/src/combat.rs` holds the numbers and rolls of sections 2 to 5; `zone.rs` resolves an attack on a monster
  (`Zones::attack_mob`), removes bodies (`Zones::remove_bodies`, in the per-second monster pass) and tells the player with the most damage
  about the kill (`ZoneEvent::Kill`); the connection (`server.rs`) keeps the target and the next attack time, and adds the experience.
- Experience, level, points, base stats and HP/MP/SP are saved on every gain (`save_progress`).
- Drops (`crates/game-server/src/drops.rs`) land on the ground in the zone with their owner and 60 s to live, show in the view, expire in
  the per-second pass, and are picked up with `0x0205` into the money or the bags (`Inventory::pick_up`). When the save of a pick-up fails,
  the item goes back on the ground (kind 1, 60 s again) before the connection ends, so it is never lost; an item whose definition is
  missing goes back with error 1.
- Deviations: the attack runs when it is due, not on a 100 ms grid; the monster's position is where its last step ended (the original
  interpolates while it walks); drops come before the killer's experience packets (the original sends the
  experience first); what is left of a partly stacked pickup goes back on the ground with error 1 (that path of the original is not read).
- Monsters fighting back, death and recovery: `docs/original/mob-fight.md`.
- Not yet: limited drops (`Drop` above 0: never dropped here; unreachable in the data, `docs/original/quest-drops.md` section 5), premium
  bonuses. Built: random craft stats on drops (`docs/original/item-origins.md` section 6), quest and sequence items
  (`docs/original/quest-drops.md` section 6), skills (`docs/original/skills-use.md`), parties (`docs/original/party.md` section 13).
