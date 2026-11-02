# Item buffs: experience stones, protections, gold, charms, resurrection runes

This page covers what the item buffs of abilities 35 to 46 do in the EP4.5 game server `ps_game.exe`: experience
stones, the experience-loss protections, the gold-drop items, the three charms, Prevent Drop Equipped Item and the Continuous
Resurrection Runes, and the resurrection runes of the death window (`0x0553` kinds 4 and 5, `0x0552`). It was read in the Ghidra
decompiler and the disassembly with the PDB names. Nothing was executed. Item and skill numbers are the shipped `Item.SData` and
`Skill.SData`.

How the buffs are applied, shown and refused in pairs is on [Using items](item-use.md); the warehouse flags (abilities 39 and
43) on [Warehouse](warehouse.md); the protections of item quality (36) on [Wear and repair](repair.md); the item
protection against player kills (37) on [Player kills 1](pvp.md).

## In short

- **An experience stone alone gives ×1.5, not ×2.** The flag behind it is named "2X", but the ×2 needs a premium account's "1.5X" flag
  as well. Any charm adds 0.2 more. Quest rewards are never multiplied.
- **Experience lost at death** is normally 5 %. A Prevent Of EXP Drop item makes it 0; a Blue Dragon or White Tiger Charm makes it 1 %;
  a Red Phoenix Charm 0. The percent is fixed at the moment of death.
- **Gold-drop items** add their percent (100 on the shipped ones) to gold a monster dropped, and to a party's gold share. A White Tiger
  Charm adds 20 %, a Red Phoenix Charm 50 %. Gold from player kills is never raised.
- **The Red Phoenix Charm also opens the warehouse's second half.**
- **Prevent Drop Equipped Item does nothing**: nothing the original takes from a player ever touches equipped items, and the flag is read
  nowhere.
- **The death window's resurrection runes** bring the player back at half HP, SP and MP with no experience lost: the Small
  Ressurrection Rune at the bind position, the Ressurrection Rune where the player lies.
- **Continuous Res Runes, and the automatic use of a carried resurrection item, only work in ultimate mode.** There they save the
  character; without either, an ultimate-mode character that dies is deleted.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (the PDB's plus 4 from the map pointer on).
"trunc" is the x87 store with chop rounding (each of these functions sets the control word to chop around its `fistp`).

## How it was read

What the item buffs of abilities 35 to 46 do in the original, which the new server applies (`docs/original/item-use.md` section 7) but
mostly ignores, and the resurrection runes of the death window (`0x0553` kinds 4 and 5, `0x0552`). Static analysis of `ps_game.exe` (the
same build as the other analysis documents): Ghidra decompile through REA and the disassembly with PDB names (`docs/original/symbols/ps_game.tsv`,
`tools/pdb_types.py` for `CUser`, `CItem`, `stItemInfo`). Nothing was executed. Numbers from `defs.items` and `defs.skills`. "trunc" is
the x87 store with chop rounding (each of these functions sets the control word to chop around its `fistp`).

`CUser` offsets are the code's (the PDB's + 4 from the map on, `docs/original/small-items.md`).

## 1. The flags

Confirmed.

`CUser::SetSkillAbility` 0x485e00 (jump table 0x486144, index = ability − 1) sets them when a buff with the ability is applied (value
`v`) and undoes them when it ends (`−v`); `CUser::CUser` and `ResetCharacter` clear them. `CUser::UseItemSkill` refuses a second item of a
flag already set (`docs/original/item-use.md` section 7, built).

| ability | flag (PDB) | code offset | set | read by (every reader in the binary) | new server today |
|---|---|---|---|---|---|
| 35 | `m_bSkillGetExpUP2X` | `+0x58d0` | `+= v` | `CUser::AddExpFromUser` (section 2) | nothing |
| 36 | `m_bSkillNDQuality` | `+0x58b8` | `+= v` | `QualityByAttack`/`QualityByDefense` | built (`docs/original/repair.md`) |
| 37 | `m_bSkillNDItem` | `+0x58c0` | `+= v` | `CUser::ItemDropByUser` | built (`docs/original/pvp.md` 6.2) |
| 38 | `m_bSkillNDExp` | `+0x58bc` | `+= v` | `CUser::Death`, `DeathByNPC`, `ExpDrop` (section 3) | nothing |
| 39 | `m_bSkillEnableBank` | `+0x58c8` | `+= v` | the warehouse | built |
| 40 to 42 | `m_bSkillAmulet` | `+0x58b0` | level 1, 2, 3 (section 5) | `AddExpFromUser`, `Death`, `DeathByNPC`, `ItemGet`, `CMob::ItemDrop` | only the use refusal |
| 42 | `m_bSkillBank2X` too | `+0x58cc` | `+= v` | the warehouse's second half | **not**: only ability 43 counts |
| 43 | `m_bSkillBank2X` | `+0x58cc` | `+= v` | the warehouse's second half | built |
| 44 | `m_bSkillMobGold` | `+0x58b4` | **`= v` (0 when `v ≤ 0`)** | `CUser::ItemGet` (gold), `CMob::ItemDrop` (party gold) (section 4) | nothing |
| 45 | `m_bSkillNDItemEQ` | `+0x58c4` | `+= v` | **nothing** (section 6) | nothing |
| 46 | `m_bSkillRebirth` | `+0x58d8` | `+= v` | `CUser::KillSoul`, `KillSoulForDisconnect`, ultimate mode only (section 7) | nothing |

Two more flags have no item: `m_bSkillGetExpUP15X` (`+0x58d4`) and, again, `m_bSkillNDQuality` and `m_bSkillBank2X`, which **premium
billing** sets (`CUser::BillingApply` 0x457a70 by `m_nUserBillingType` `+0x5854`: 1 → `Bank2X`; 2 or 3 → `NDQuality`, `ExpUP15X`,
`Bank2X`). Premium is built (`docs/original/item-mall.md` section 7).

The shipped items (item type 100 naming the skill by `range`/`attacktime`; every one `wType` 82, kept over death unless noted):

| ability | skills | items |
|---|---|---|
| 35 | 222, 223, 250, 251, 272 | Small/Large Increase EXP (100042/100043), Mini/Small Sun EXP Stone (100108/100109), Sun EXP Stone (100135); **also 100087 "Small Prevent Of EXP Drop"** (data: it names skill 222, so it is an EXP stone) |
| 38 | 229 (not kept over death), 230, 240, 255 | Mini/Large/Small Prevent Of EXP Drop (100055/100056/100057), Prevent Of EXP Loss (100110) |
| 40, 41, 42 | 231, 232, 233 | Blue Dragon (100072), White Tiger (100073), Red Phoenix Charm (100074) |
| 44 | 235, 245, 262, 271 (value 100) | Large/Small Increase Gold Drop (100076, 100083, 100088), Mini Gold Increase (100114), Increase Drop Gold (100134) |
| 45 | 236 | Prevent Drop Equipped Item (100077) |
| 46 | 266, 267, 286 levels 1 to 3 | 3 Hr / 6 Hr / 7 Day / 15 Day / 30 Day Continuous Res Rune (100129, 100130, 100163 to 100165) |

## 2. Experience

Confirmed, `CUser::AddExpFromUser` 0x458240.

`AddExpFromUser(killer, gain, plain, …)`: unless `plain`, the gain is multiplied before anything else:

- **`m` = 1.0; 1.5 when either `m_bSkillGetExpUP2X` or `m_bSkillGetExpUP15X` is set; 2.0 when both are** (constants 0x53a3e0, 0x53a4b8,
  0x53a4c0; the branches at 0x458281). So **an EXP stone alone gives ×1.5**, not ×2: the ×2 needs premium as well.
- **+ 0.2 with any charm** (amulet level 1, 2 or 3; 0x53a3e8).
- **`gain = trunc(gain × m)`**, then (as now) 0 when negative, the full-party bonus equal to this gain (`docs/original/party.md` section 8),
  the level-ups, `0x0207`.

Who passes `plain` (verified at each call): monster kills, solo and every party member's share (`CMob::Death` 0x49125b, 0x491366,
0x49146f): **not plain**; player kills (`CUser::Death`): **not plain**; **quest rewards** (`CUser::QuestSuccessResult` 0x47eefc):
**plain**. Experience from `/set` or the experience wheels does not go through it.

## 3. Experience lost at death

Confirmed, `CUser::Death` 0x4586e2, `CUser::DeathByNPC` 0x459028, `CUser::ExpDrop` 0x45ad50.

At the death (both functions, before anything else): **`m_nDropExp` = 5**; then **0 with `m_bSkillNDExp`**; else **1 with a charm of
level 1 or 2**, **0 with a charm of level 3**. A player kill, a duel or a guild war then set it to −1 as before
(`docs/original/duel.md`, `docs/original/guild-war.md`). At rebirth `ExpDrop(m_nDropExp)` (`docs/original/mob-fight.md` section 8)
also does nothing while `m_bSkillNDExp` is set.

So the percent is fixed **when the player dies**: a protection whose buff ends at death (skill 229, Mini Prevent Of EXP Drop) still
protects that death; one used up while dead cannot help (items cannot be used dead).

## 4. Gold

Confirmed.

`p` = **`m_bSkillMobGold`** (the item's 100) **+ 20 with a charm of level 2, + 50 with level 3** (level 1 adds nothing).

- **Gold a monster dropped, picked up** (`CUser::ItemGet` 0x45d36b: a gold item, `CItem::m_byType` 26, whose **`m_byDropType` is 1**,
  set only by `CMob::ItemDrop` 0x4a64aa/0x4a66a6; players' drops are 2, deaths 3): with `p > 0`, **amount + trunc(p / 100 × amount)**
  (the amount as unsigned), then the money cap as now.
- **A party's gold share** (`CMob::ItemDrop` 0x4a63b0, `docs/original/party.md` section 9): each member in range gets **its share +
  trunc(p / 100 × share)** with its own `p`.

Gold from player kills (`docs/original/pvp.md` 6.2: drop type 3, or a party's share of it through `ItemGetMoney` directly) gets nothing.

## 5. The charms

Confirmed, `SetSkillAbility` cases 40 to 42.

Applying ability 40, 41 or 42 with `v > 0` sets the amulet level to 1, 2 or 3; ending it (`v ≤ 0`) sets it to 0 only if it is still
that level. Ability 42 **also adds to `m_bSkillBank2X`** (case 0x4860e4: the Red Phoenix Charm opens the warehouse's second half too).
`UseItemSkill` refuses a charm no stronger than the one in effect (built). What the levels do:

| | Blue Dragon (1) | White Tiger (2) | Red Phoenix (3) |
|---|---|---|---|
| experience (section 2) | + 0.2 | + 0.2 | + 0.2 |
| experience lost at death (section 3) | 1 % | 1 % | 0 % |
| gold (section 4) | — | + 20 % | + 50 % |
| warehouse second half | — | — | yes |

## 6. Prevent Drop Equipped Item (ability 45: no effect, verified)

`m_bSkillNDItemEQ` is written by `SetSkillAbility` and tested by `UseItemSkill` (one at a time), and **read nowhere else** (every
`+0x58c4` in the binary). Nothing the original takes from a player touches the equipment: a player kill takes items from **bags 1 and
up** only (`docs/original/pvp.md` 6.2), monsters take nothing. The item is used up and its buff runs, and nothing changes.

## 7. Coming back to life

Confirmed.

**`0x0553`** (`CUser::PacketPC` 0x46ab2a; kinds 2 and 3 built): each kind first sends a game-log record (`0x0305`; built, [The game log](game-log.md)); then:

- **kind 4 → `CUser::KillSoulByItem`** 0x459740, **kind 5 → `CUser::KillSoulByItemNoMove`** 0x4598a0. For a dead player in a zone, not
  already gone (`+0x1430` ≠ 2):
  1. **`m_nDropExp` −1** (a player kill, a duel, a guild war): **`KillSoul`** (the plain bind-position rebirth of kind 2), nothing used.
  2. **`FindItemRebirth(kind 5)`** (0x45f740): in bags 1 to the bag count, slots 0 to 23, in order, the first item with **real type 26 and
     `bySpecial` 37** and **`shVG` ≠ 0 (kind 4) or `shVG` = 0 (kind 5)**; none such: the first with real type 26 and special 37 at all.
     The shipped ones: **Small Ressurrection Rune** (100/81, VG 1: kind 4's) and **Ressurrection Rune** (100/44, VG 0: kind 5's). None:
     **`KillSoul`**.
  3. Found: **one is used up** (`CUser::ItemUseNSend`: `0x050A` with what is left, `docs/original/item-use.md` section 3), **HP, SP and MP
     = their maxima / 2** (integer division), **no experience lost**, and `CUser::Rebirth(…, full 0)`: **kind 4 at the bind position**
     (`CMap::GetBindPos`, as kind 2; `0x0551` kind 4), **kind 5 where the player lies** (`0x0551` **kind 1**, same map, no move).
- **`KillSoul`** (kind 2, the 200 s timer, `docs/original/mob-fight.md` section 8) **in ultimate mode** (grow 3, `m_nDropExp` not −1):
  - **`m_bSkillRebirth` set** (a Continuous Res Rune): HP, SP, MP at half, **no experience lost**, the bind position (`0x0551` kind 4);
    nothing used up;
  - else **a resurrection item** (`FindItemRebirth(0)`: kind 4's preference): used up, half, no loss, the bind position;
  - else the experience drop, then **the character is deleted** (`0x0406` to ps_dbAgent) and logged out.
  In the other modes `m_bSkillRebirth` and the runes do nothing here (the drop, full HP, the bind position).
- **`KillSoulForDisconnect`** 0x459380 (leaving the game dead): the same three ultimate-mode cases without packets; otherwise the drop and
  full HP at the bind position, as built.
- **`0x0552`** (`CUser::LifeSoul` 0x459ce0): only for a dead player whose `+0x1430` is **1**, which only **`CUser::Soul`** 0x4591c0 sets,
  and **`Soul` has no caller** in the binary. So **`0x0552` never does anything** in the original. (The resurrection skill is
  `CUser::LifeSkill` 0x459d20, built in `docs/original/skills-special.md` section 12.)

## 8. Original oddities

1. **An EXP stone gives ×1.5 alone** (section 2): the flag named "2X" only reaches ×2 with premium's "1.5X" flag.
2. **Prevent Drop Equipped Item does nothing** (section 6).
3. **Continuous Res Runes and the automatic use of a resurrection item only work in ultimate mode** (section 7); in the other modes the
   runes still work from the death window (kinds 4 and 5).
4. **After a player kill, a duel or a guild-war death the death window's runes are not used**: the player goes back to the bind
   position with full HP as with kind 2 (`m_nDropExp` −1 comes first).
5. Item 100087 "Small Prevent Of EXP Drop" is an EXP stone (skill 222).

## 9. What the new server does now, and what changes

Today: the buffs apply, show and are refused in pairs (`docs/original/item-use.md` section 7); abilities 36, 37, 39 and 43 work; 35,
38, 40 to 42 (besides the refusal), 44, 45, 46 do nothing; `0x0553` kinds 4 and 5 are ignored (debug log), `0x0552` is unhandled; the
experience lost is a fixed 5 % computed at rebirth; ultimate mode dies as the other modes (no deletion).

To build (as the original unless a deviation says otherwise): sections 2 to 5 and 7's kinds 4 and 5; `0x0552` and ability 45 stay
without effect; ultimate mode per the decision (section 10).

## 10. Deviations and decisions (approved; built, section 11)

1. **Decided: EXP stones give ×2** (section 2, oddity 1), ×2.2 with a charm; the original's ×1.5 without premium is not kept (premium
   is not built; when it is, its 1.5X flag gives ×1.5 alone and ×2 with a stone, as the original).
2. **Decided: ultimate mode dies as the original, without removing the character** (section 7): a Continuous Res Rune, else a carried
   resurrection item (used up), brings the player back at half with nothing lost; without either the experience drop, then the
   character **falls**: `deleted_at` set as for a deletion and **`fallen_at`** (migration 0027) so that it can be restored later (clear
   both; a paid restore was mentioned), out of its guild as a deletion (`usp_Delete_Char_E` leaves the guild), its name kept reserved
   while fallen, and the player back at the character screen at once (`0x0107`). Leaving the game dead does the same without packets.
3. **Decided: the original kept for safe deaths** (oddity 4): after a player kill of record, a duel or a guild-war death
   (`m_nDropExp` −1) the runes are not used, the player goes back to the bind position with full HP and nothing lost, and **an
   ultimate-mode character never falls from such a death** (the original's rule: `KillSoul` treats `m_nDropExp` −1 as the other modes).
4. **Decided: Prevent Drop Equipped Item stays without effect** (section 6).
5. **Decided: the Red Phoenix Charm opens the warehouse's second half** (section 5), as the original.

## 11. As built

`crates/game-server`: `buffs.rs` (`BuffEffects::exp_up`, `no_exp_loss`, `amulet`, `mob_gold`, `rebirth`, `bank_second_half` from 42;
`exp_factor`, `death_exp_percent`, `gold_percent`, `with_gold_bonus`), `zone/pvp.rs` (`player_died` takes the victim's effects from
before the death; `ZoneEvent::Died::drop_exp`), `zone.rs` (`GroundItem::from_mob` for drops of kind 2, `ZoneEvent::Gold::from_mob`,
revival at half), `server.rs` (`award_exp` with the factor unless a quest's; the percent fixed at death and used at rebirth; the gold
bonus on monster gold picked up and on a party's share; `0x0553` kinds 4 and 5; ultimate mode, `fall`, `WorldState::fallen`;
`leave_dead`), the stores' `fall` (PostgreSQL: `deleted_at`, `fallen_at`), migration `0027_fallen.sql`, `shaiya-proto` `REBIRTH_HERE`
(1). Tests: `buffs` (every flag and charm), `server` (1 % and 0 % losses), flow tests for the EXP stone and Gold Drop (288 for a
kill of 144, 140 000 for 70 000 of a monster's gold, a player's own gold not raised), the runes (kind 5 prefers VG 0, falls back to the
other rune, kind 4 without one is kind 2) and an ultimate death without a rune (back at the character screen, the character not
listed and kept). Live test: `docs/testing/live-test-checklist.md` section 43.

Small choices made in the build:

- **Fallen is final for the connection**: once fallen the character is never brought back to life by a later `0x0553`, the timer or
  leaving (the original's `+0x1430` 2), and neither a motion nor a monster's blow cancels its way out.
- **The percent at rebirth** is the death's (fixed in `player_died` from the buffs before death cleared them), zero when the no-loss
  flag is still on at rebirth (`ExpDrop`'s own check).
- **The full party's bonus** is the raised gain (the original adds the gain after the multiplier).
- **The player's own effects** raise its gold: each member's share is raised by that member's flags.
