# Character numbers: max HP, SP and MP, stats and speeds, with equipment

This page covers how the EP4.5 game server `ps_game.exe` works out a character's maximum HP, SP and MP, its six stats
and its speeds, what equipment adds, what the client is told, and how stat points are spent. It comes from static analysis; nothing was
executed.

## In short

- **Maximum HP, SP and MP** come from a table by class and level, raised by 10 % in hard mode and 50 % in ultimate mode, plus a small
  race bonus, plus **5 for every point** of the matching stat: REC gives HP, DEX gives SP, WIS gives MP.
- **Equipment adds the same way**: an item's stat bonuses count like the character's own points (5 HP per REC, and so on), and its flat
  HP, SP and MP bonuses are added on top. Crafted items carry extra stats hidden in their "craft name": each crafted point of HP, MP or SP
  is worth 100.
- **Speed.** Every character starts at move speed 2; accessories can raise it. The client is told about speed and maximum changes only
  when they change.
- **Spending stat points** raises the stat and the matching maximum at once, but not the current HP, SP or MP. The server allows any
  mix of stats, at any time in the world.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.

`CUser` offsets are the code's; from `stUserInfo`'s base stats on they are 4 above the PDB's (see [NPC shops](shop.md)): base
stats `+0x170` to `+0x17a` (STR, DEX, INT, WIS, REC, LUC), max HP `+0x17c`, max MP `+0x180`, max SP `+0x184`, the totals of STR to LUC
`+0x1220` to `+0x1234` (same order), attack speed `+0x12ec`, move speed `+0x12f0`.

## 1. On entering the world

Confirmed, `CUser::InitEquipment` 0x454560.

1. Every computed value is cleared; **move speed 2**, attack speed 0, two bags (`docs/original/shop.md`); STR to LUC = the base stats.
2. **Max HP, SP, MP** = the class and level row of the status table (`CGameData::LoadStatus`, `defs.status_defs`: job 0 to 5, level 1 to 60,
   `hp, sp, mp`; outside that, the row of job 0 level 60), times **1.1 for hard mode, 1.5 for ultimate** (the float `1.1f`/`1.5f`, then truncated:
   rounding mode set to chop), plus the race: human **+10 SP**, elf **+10 MP**, vail **+5 SP, +5 MP**, deatheater **+10 HP**.
3. Plus **5 HP per base REC, 5 SP per base DEX, 5 MP per base WIS**.
4. Each equipped item in slots 0 to 12 is added (section 2; the mount in slot 13 is not), then passive skills and the attack are computed.

`CUser::SendCharacterData` (`0x0105`) sends these maxima. (Creating a character uses the same rules for its first values, `docs/original/ps_game.md`.)

## 2. Adding an item

Confirmed, `CUser::ItemEquipmentAdd` 0x454c70.

For every slot: STR to LUC += the item's `wConstStr` to `wConstLuc`; max HP += `wConstHP`, max SP += `wConstSP`, max MP += `wConstMP`; and
**5 HP per `wConstRec`, 5 SP per `wConstDex`, 5 MP per `wConstWis`**. By slot also:

| slot | adds |
|---|---|
| 5 weapon | range `byRange`, attack speed `byAttackTime` (and a per-weapon-type value from passive skills), attack (effects 1 and 2 by quality level, not covered here) |
| 8 to 12 (accessories) | range, attack speed, **move speed `bySpeed`**, effects 1 to 4 |
| others (armour) | defence and magic defence (effects 3 and 4 by quality level, not covered here); slot 6 also the shield-mastery value (below) |

**Slot 6 and shield mastery** (confirmed, 0x454c70 and `CUser::AddSkillPassive` 0x489a80, `wType` 10): whatever item is put in slot 6, its
defence gets **`m_byShieldValue`** (`+0x1334`), the sum of the shield-mastery passives learned (`docs/original/skills.md` section 5). A
shield has **no block value** of its own: nothing in `ItemEquipmentAdd` or `CUser::ChkBlock` (0x44c5a0) reads one, and blocks come only
from buffs (`wType` 85, 86, 92; `docs/original/skills-buffs.md` section 10). (The earlier "block value" here was this mastery value.)
Checked across the binary (2026-11-05): the `CUser` code that reads slot 6 directly (`+0x1dc`) is the shield check of skills
(`EnableUseSkillUser`, `EnableUseSkillMob`), `EnableEquipment` (a shield against a two-handed weapon), `EnterDead` (the body's look) and
the shield-mastery passive (`AddSkillPassive`, `RemSkillPassive`); the block state (`m_bBlockAll` `+0x1360`, `nBlock` and
`nBlockValue` at `+0x13a8` and `+0x13ac` per attack kind, stride `0x2c`) is written only by `AddApplySkillBuff`, `RemApplySkillBuff`,
`ChkBlock` (the count), `ClearApplySkillByDeath`, `SkillClearAll` and `InitEquipment` (which clears block-all). The 998 shield rows of
the data (real type 19) have only defence, magic defence, stats, HP/SP/MP and lapis rooms. Rechecked 2026-11-05 (every access to
`+0x1334`, `+0x1360`, the block slots and `+0x1dc` listed by function; `EnterWorld`'s `+0x1dc` is a stack slot): as above. One precision:
the mastery reaches slot 6 on two paths that never add up, `ItemEquipmentAdd` (0x454f7d, slot 6, the whole sum) and
`ItemEquipmentAttackAdd` (0x456508, the slot-6 case of its jump table 0x45658c), which only `InitEquipment(bool)` 0x454ba0 calls after
zeroing attack, spread, defence and magic defence (the recount, for example on leaving a shape); a passive learned while a shield is worn adds its own value at once only when the shield's
real type is the passive's (`AddSkillPassive` 0x489b2a), otherwise from the next equipment change.

**Crafted stats** (`m_wCraftExpansionAddValue`), for items whose definition allows them (`stItemInfo` `+0x4a`, the `server` column, not 0, and
`+0x3e`, the `reqwis` column, 1 to 99; `CItem::CopyCraftExpassion` 0x4ba670): the 20-character craft name is read as ten two-digit numbers;
numbers 1 to 9 are **STR, DEX, REC, INT, WIS, LUC, HP, MP, SP**, the tenth is the enchant level (attack for weapons, defence when above 50, from a
table; not covered here). They add: the stats as numbers (DEX also 5 SP each, REC 5 HP, WIS 5 MP), **HP, MP and SP 100 per point**. A craft
name that is not 20 characters becomes `"00000000000000000000"`.

Lapis (the gems) add their own effects (`CUser::GemEquipmentAdd` 0x4560c0, `docs/original/lapis.md` section 2). `CUser::ItemEquipmentRem` subtracts the same.

## 3. What the client is told

Confirmed.

Only when something changed, and not while entering the world:

- **Speed** `0x051C` to the players within 60 (`CZone::PSendView`): `u32 id, u8 attack speed` (clamped 0 to 9), `u8 move speed` (0 to 4); 255 for
  either in special states (stunned, sleeping, transformed and the like).
- **A maximum** `0x050B` (`TP_SET_MAX`, 9 bytes) to the players within 60: `u32 id, u8 type, u16 max`, type **0 HP** (`SendMaxHP`), **1 SP**
  (`SendMaxSP`), **2 MP** (`SendMaxMP`); also to the party (`0x0C02`/`0x0C0B`). Before it the player's current values are clamped and re-sent
  (`CUser::SendRecoverAdd`).

## 4. The new server (`crates/game-server/src/stats.rs`)

- Computes the maxima, stat totals and speeds as in sections 1 and 2 from the stored base stats and the equipment (with crafted stats), sends the
  maxima in `0x0105` and the speeds on entering, and `0x050B`/`0x051C` when an equipment change alters them.
- Lapis and the enchant table: `docs/original/lapis.md` section 2. The party copies: `0x0C08` from the party pass (`docs/original/party.md`
  section 13). (Attack and defence: `docs/original/combat.md`; current HP/SP/MP and recovery: `docs/original/mob-fight.md`; passive skills:
  `docs/original/skills.md`.)

**Character creation bug fixed with this:** the new server stored new characters' SP in the `mp` column and MP in the `sp` column (and so sent
them swapped in `0x0105`); creation now stores `hp, mp, sp` in that order.

## 5. Spending stat points: `0x0208`

Confirmed, `CUser::PacketMaininterface` 0x468420, the block at 0x46875d.

The client sends **`0x0208`** (`TP_USER_STATUSUP`, 14 bytes): `u16` **STR, DEX, REC, INT, WIS, LUC**, the points to add to each (note the
order: REC third, as in `characters`, not the in-memory order of section 1).

1. **Ignored, no answer**, while a stat reset is waiting (`+0x584e`, `m_bResetStatusWait` in the PDB) or while dead (`+0x142c == 1`), and when
   the six numbers add up to 0 (the sum is a signed 32-bit int of the six `u16`, so it cannot overflow).
2. **More than the character's stat points** (`+0x138`, `m_wStatPoint`; the check is `points >= sum`): the packet is sent back **with all six
   numbers 0** and nothing changes.
3. Otherwise each number is added to its **base stat** (`add word`, `+0x170` to `+0x17a`) and to its **total** (`+0x1220` to `+0x1234`), and
   the sum is taken from the stat points. Then, each only when its number is not 0: **max HP += 5 × REC** and `SendMaxHP`, **max SP += 5 × DEX**
   and `SendMaxSP`, **max MP += 5 × WIS** and `SendMaxMP` (`0x050B`, section 3); then the attack is recomputed (`CUser::SetAttack`), the
   database is told (`CUser::SendDBStatusUP` 0x46c780: `TP_DB_SET_STATUSUP` with the stat points left and the six base stats), and **the
   request is sent back unchanged** as the answer. A game log line (`0x030a`, `TP_GAMELOG_CHAR_STATUS_UP`) per non-zero stat follows (built, [The game log](game-log.md)).

Nothing else is checked: any mix of stats, any time in the world (in a shop, a trade, fighting, transformed). The current HP, SP and MP are
not raised, only the maxima. A stat reset item (`CUser::StatResetStatus` 0x480660): `docs/original/item-use.md` section 8.

**The new server** handles it the same way, recomputing everything from the base stats (section 4) and sending the maxima that changed
(HP, SP, MP in that order, as above) before the answer; the stat points and base stats are saved with the level and experience (one `UPDATE`).
A stat reset is saved before anything else is handled, so nothing waits for one (`docs/original/item-use.md`). **Deviation** (one): a request that would take a base stat above **32767** (the
`smallint` columns) is refused like one asking for too many points; the original's `u16` would wrap at 65536, out of reach in normal play.

## 6. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- **The same level-up numbers** (confirmed): `CUser::LevelUpStatus` (EP3 ps_game 0x46e630, matched by call graph) has the same per-mode
  stat and skill point jump tables, and `CGameData::LoadStatus` (EP3 ps_game 0x497650, matched by call graph) the same constants. The
  level cap is 60, 30 in easy mode (EP3 ps_game 0x44209a, 0x4420a0).
- **No crafted stats** (confirmed): `CUser::ItemEquipmentAdd` (EP3 ps_game 0x43f590, matched by call graph and checked by its
  `GemEquipmentAdd`, `SetAttrDef` and `SendMax*` calls) has no loop over crafted values, where EP4.5 adds the eleven crafted numbers at
  0x455102 to 0x455223, and EP3 has no craft-name parser like `CItem::CopyCraftExpassion`. The crafted stats of section 2 do not exist in
  EP3.
- **The lapis column `exp` is an experience bonus** (confirmed): `GemEquipmentAdd` adds the lapis' byte `+0x49` to `CUser+0x11e0` (EP3
  ps_game 0x44018b) and `AddExpFromUser` multiplies experience gained by `1 + value / 100` (EP3 ps_game 0x442133 to 0x44218d). EP4.5 adds
  the same byte to absorption instead ([`lapis.md`](lapis.md)).
- Spending stat points (`0x0208`) and `InitEquipment` were not compared.

### EP5.4

- **The same level-up numbers** (confirmed): `LevelUpStatus` (EP5.4 ps_game 0x49b3b0, matched by call graph) has the same jump tables and
  the × 1.1 / × 1.5 mode factors; `LoadStatus` (EP5.4 ps_game 0x4d8210) reads the same five columns; `StatusDefs` is identical (360 rows,
  levels 1 to 60).
- **The status table is 6 × 71** for the level cap of 70 (EP5.4 ps_game 0x4d83b2), but the shipped `StatusDefs` stops at level 60, so
  levels 61 to 70 get 0 from it and max HP, SP and MP drop at the step to 61 (inferred). `InitEquipment` (EP5.4 ps_game 0x461010) uses
  the level bounds 70 and 80 (confirmed).
- **Shapes no longer hide equipment** (confirmed): `ItemEquipmentAdd`/`ItemEquipmentRem` (EP5.4 ps_game 0x461640, 0x461d10, matched by
  call graph) never read the shape byte; EP4.5 skips the item's attack and defence while a shape is set (0x454d1d to 0x455200). This
  removes the cause of [`stealth-equip`](community-fixes.md#stealth-equip).
- **New skill abilities that change the numbers** (confirmed): 49 adds its value to all six stat totals and 5 × value to max HP, SP and
  MP (EP5.4 ps_game 0x495952); 48 is a damage reduction.
- `CUser` offsets: base stats `+0x16c`, stat totals `+0x121c`, speeds from `+0x12f4` (confirmed by matched accesses). `0x0208` was not
  compared.
