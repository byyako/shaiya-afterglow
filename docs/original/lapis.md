# Lapis and enchanting: `0x0801` to `0x0808`

This page covers the blacksmith's work in the EP4.5 game server `ps_game.exe`: what linked lapis (gems) and enchant
levels add to the wearer, linking (`0x0801`), extraction (`0x0802`), enchanting with a Lapisia (`0x0805`), new random craft stats from a
Recreation Rune (`0x0806`) and for unidentified items (`0x0808`), making one item from three (`0x0807`), cloak badges (`0x0803`,
`0x0804`), and absorption (`0x0525`). It was read in the Ghidra decompiler and the disassembly with the PDB names: `CUser::PacketGem`
(0x46bd40) and what it calls.

The client side is in the [packet catalogue](../client/client-packets.generated.md) (the client sends `0x0801` 8 bytes, `0x0802` 4,
`0x0803` 10, `0x0804` 4, `0x0805` 9, `0x0806` 6, `0x0807` 8, `0x0808` 4) and the [client facts](../client/CLIENT-FACTS.md) (`0x0802` is
always the 4-byte form; `0x0801` and `0x0805` only while an NPC is selected, no NPC id on the wire).

On other pages: the guild house rates (maps of create type 6; [Guild houses](guild-house.md#9-what-a-houses-npcs-do-differently)),
premium accounts (`m_nUserBillingType` 2 or 3: rate × 1.5; [Item mall](item-mall.md#7-premium-accounts)) and the game log
(`0x04xx` to the game log server; [The game log](game-log.md)). Not covered: `NEW_SERVER` in `ps_game.ini` (halves the enchant price; not set in the
shipped configuration).

## In short

- **Linking** puts a lapis into a free socket of an item. Each lapis has a grade that sets its price and its chance: low-grade lapis link
  often, high-grade ones almost never (as little as 1 %). A Linking Hammer multiplies the chance, but it never goes above 50 %. The price
  is paid and the lapis used up whether it works or not.
- **Dangerous lapis** destroy the item when a link, extraction or enchant with them fails, unless the player carries a Lucky Charm (for
  linking only).
- **Extraction** takes the lapis back out. On a failure they are lost.
- **Enchanting** with a Lapisia and the right material raises a weapon's attack or an armour's absorption, up to +20. Each step is
  harder. The chances and what each step adds come from a data file the server reads at start (`Data/ItemEnchant.ini`): with the
  shipped file a weapon's first step works 90 % of the time and +20 can be reached, rarely; the built-in tables, used only without
  the file, stop weapons at +12 and armour at +14 (section 5).
- **Absorption** takes a fixed amount off every hit, but a hit always does at least 1 damage.
- **Recreation Runes and unidentified items** roll new random craft stats; three items of the same kind can be remade into one with
  fresh stats.
- **Flaws in the original**: a cloak can be "linked" or "enchanted" with any item at all, a remake can name the same item three times ([fixed by the kit](community-fixes.md#remake-distinct)),
  the server never checks that a blacksmith is near, and the US client misreads the extraction answer by one byte.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's: equal to the PDB's up to the money
(`+0x140`), 4 above it from the VG (`+0x16a`) on (see [NPC shops](shop.md)); `stItemInfo` and `CItem` as on
[Dropping items](item-drop.md).

**`PacketGem`** ignores everything while the player is dead (`+0x142c` = 1); it first cancels a trade (`CancleActionExc`) and closes the
personal shop (`stMyShop::Ended`); an unknown `08xx` closes the connection (reason 9).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`): `CUser::PacketGem` (0x46bd40) and what it calls. Client side: `docs/client/client-packets.generated.md`
(the client sends `0x0801` 8 bytes, `0x0802` 4, `0x0803` 10, `0x0804` 4, `0x0805` 9, `0x0806` 6, `0x0807` 8, `0x0808` 4) and
`docs/client/CLIENT-FACTS.md` (`0x0802` is always the 4-byte form; `0x0801` and `0x0805` only while an NPC is selected, no NPC id on the
wire). `CUser` offsets are the code's: equal to the PDB's up to the money (`+0x140`), 4 above it from the VG (`+0x16a`) on
(`docs/original/shop.md`); `stItemInfo` and `CItem` as in `docs/original/item-drop.md`.

Covered: what linked lapis and enchant levels add to the wearer, linking (`0x0801`), extraction (`0x0802`), enchanting with a Lapisia
(`0x0805`), new random craft stats by Recreation Rune (`0x0806`) and for unidentified items (`0x0808`), making one item from three
(`0x0807`), cloak badges (`0x0803`, `0x0804`), absorption (`0x0525`).
**Not covered:** the guild house rates (maps of create type 6; `docs/original/guild-house.md` section 9), premium accounts (`m_nUserBillingType` 2 or 3: rate × 1.5; `docs/original/item-mall.md` section 7), `NEW_SERVER` in
`ps_game.ini` (halves the enchant price; not set in the shipped configuration), the game log (`0x04xx` to the game log server).

**`PacketGem`** ignores everything while the player is dead (`+0x142c` = 1); it first cancels a trade (`CancleActionExc`) and closes the
personal shop (`stMyShop::Ended`); an unknown `08xx` closes the connection (reason 9).

## 1. The items

Confirmed, data from `defs.items`.

- **Lapis**: item type **30** (`m_byType` 30). Its definition is found by its type id (`g_pItemInfo` for type 30, `0x6882ac + id × 0xa8`;
  an id of 0, 256 or above, or an unused row is "no lapis"). What each column means for a lapis:
  - **where it fits** (`CUser::EnableGemAdd` 0x45fa50): `reqlevel` ≠ 0 any weapon (real types 1 to 15); `country` ≠ 0 helmet (real type
    16); `attackfighter` upper armour (17); `defensefighter` lower armour (18); `patrolrogue` shield (19); `shootrogue` gloves (20);
    `attackmage` boots (21); `defensemage` ring (22); `grow` amulet (23); `reqstr` ≠ 0 bracelet (25, item types 40 and 41);
  - **its grade** `reqig` (`byIG`): the price and the success rate;
  - **its size** `slot` (`bySlot`, 1 for every lapis): the lapis room it takes (the item's own `slot` column is its room);
  - **dangerous** when `reqvg` (`shVG`) ≠ 0: a failed link, a failed extraction or a failed enchant with it destroys the item (section 9);
  - **its effects** (section 2) and `quality`, which adds to the item's full quality (`docs/original/repair.md` section 5).
- **Lapisia**: item type **95**. `reqig` is its grade (96 weapon Lapisia, 40 armour Lapisia), `reqvg` ≠ 0 dangerous, and its placement
  columns as for lapis: 95/1 to 3 weapons, 95/6 to 8 the six armour pieces.
- **Linking Hammer**: real type 26 (item types 25, 44, 100) with **special 36**; `reqvg` is a percentage (110 to 450, 2000).
- **Extraction Hammer**: real type 26 with **special 58**; `reqvg` is the success rate in per cent (30, 30, 90).
- **Lucky Charm**: real type 26 with **special 57** (100/139): protects an item from a dangerous lapis (section 3).
- **Recreation Rune**: **special 62** (100/171), any type (section 6).
- **The material** of an enchant: the item whose `itemid` equals the equipment's `reqluc` (`wLuc`): 25043 for many weapons. The equipment
  must also be **craftable** (`server` ≠ 0); see section 5.

**The lapis price** (`GetGemMoneyByGrade(grade)` 0x45f6e0): grades 31 to 40 → 4095, 11250, 22965, 41280, 137900, 365000, 480000, 627000,
814000, 1040000 (the table at 0x531808, index grade − 30); grades 96 to 100 → 2500000, 3500000, 5500000, 7500000, 9800000 (0x531834, index
grade − 96); any other grade → 0.

**The link and extraction success rate** of a lapis of grade `g` (`R0`), in x87 arithmetic with single-precision operands, truncated:
`R0 = trunc(f32(289 − 8 × g) + f32(KC[country]) + f32(level) × 0.05f)`, at least 1; `KC` is the faction grade's bonus type 24 (`docs/original/pvp-2.md` section 8; 1 to 5 in the shipped file, 0 below grade 8).
Grade 31 gives 41 + level/20, grade 33 25, grade 35 9, grade 36 and above 1.

## 2. What lapis and enchants add

Confirmed, `CUser::GemEquipmentAdd` 0x4560c0, `GemEquipmentRem` 0x456280, `ItemEquipmentAdd` 0x454c70.

When an item is equipped (slots 0 to 12) each of its six gems that is a known lapis adds, **except in slot 7** (the cloak, whose gems are its
badge):

- attack range + `range`, attack speed + `attacktime`, move speed + `speed` (any slot);
- unless the player is in a shape (`m_byShape` ≠ 0): attack + `effect1`, attack spread + `effect2`, defence + `effect3`, magic defence +
  `effect4` (the full values, `wEffectN[0]`);
- STR, DEX, REC, INT, WIS, LUC + `conststr` to `constluc` (REC also 5 HP each, DEX 5 SP, WIS 5 MP), as an item's own stats do;
- HP, SP, MP + `consthp`, `constsp`, `constmp`, unless the lapis' `special` is **16**: then they add to the recovery numbers instead
  (`m_nRecHP/SP/MP` and `m_nSitRecHP/SP/MP`; no lapis has special 16);
- **absorption** (`m_nReduceDamage`, code `+0x12fc`) + `exp`;
- outside the weapon slot, a `special` of 1 to 15: one more **state block** of that state (`m_pStateBlock[special]`): a monster skill of
  that state type misses (`docs/original/mob-skills.md` section 7 step 2).

`GemEquipmentRem` takes the same off. **Elements** (`CUser::SetAttrAtk` 0x455f80 for the weapon, `SetAttrDef` 0x456020 for the upper
armour, slot 1): the item's `attrib`, then for each gem in order whose lapis has an `attrib`: the same element is kept; an item without
element takes the lapis'; a different element makes it **0 and stops**. An attack-element buff (`m_byAttrAtkBuff`) or a defence-element
skill (an applied skill with `+0x32` = 95) is not overridden (the existing buffs in `combat::gear`).

**Crafted values and the enchant level** (`CItem::CopyCraftExpassion` 0x4ba670, `ItemEquipmentAdd`): a craftable item's 20-character craft
name is ten two-digit numbers; numbers 1 to 9 are the crafted stats (built, `docs/original/stats.md`); the tenth, `v` (characters 18 and
19), is the **enchant level**: `v` 1 to 20 a weapon step, 51 to 70 an armour step `v − 50`; anything else nothing. A weapon step `s` adds
**attack** `E[s].weapon` (unless in a shape), an armour step **absorption** `E[s].armour`, from `g_LapisianEnchantAddValue` (0x542100):
step `s` → weapon `7s`, armour `5s` (step 20: 140 and 100) **as built in**; the server's start overwrites this table (below).

**The enchant tables come from a data file** (confirmed, `CGameData::InitItemEnchant` 0x4c7ab0, called from `CGameData::LoadData`
0x4bf320 at start): it opens `data/ItemEnchant.ini` (when that fails it logs "CGameData::InitItemEnchant fopen error" and the built-in
tables stay) and, for steps 00 to 20, sets **both** `g_LapisianEnchantPercentRate` (0x542058, section 5) and `g_LapisianEnchantAddValue`
(0x542100) from `GetPrivateProfileIntA` with a default of **0** for a missing key (sections `LapisianEnchantPercentRate` and
`LapisianEnchantAddValue`, keys `WeaponStepNN` and `DefenseStepNN`). The file of the original server patch
(`PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/Data/ItemEnchant.ini`) gives:

| step | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| weapon attack | 7 | 14 | 21 | 31 | 41 | 51 | 64 | 77 | 90 | 106 | 122 | 138 | 157 | 176 | 195 | 217 | 239 | 261 | 286 | 311 |
| armour absorption | 5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 | 60 | 65 | 70 | 75 | 80 | 85 | 90 | 95 | 100 |

The weapon row is `Σ (3 × max(1, ⌈k/3⌉) + 4)` for `k` = 1 to the step, the formula the US client's character window uses
(`docs/client/research-balance.md`); the armour row equals the built-in `5s`. A second copy in the original files
(`PSM_Client/Bin/Data/`) has 99 % for every rate and a different attack row from step 14 (175, 200, 245, 300, 350, 425, 500): a
private-server edit, not the original's.

**Absorption** (`CUser::Damage` 0x44c1e0, after give up SP/MP): HP damage `d` > 0 with absorption `A` > 0: `d ≤ A` → `d` = 1 and `d − 1`
absorbed; otherwise `d − A` and `A` absorbed. When something was absorbed: **`0x0525`** (4 bytes: `u16 absorbed`) to the player ("<n>
absorption damage"). (The other form of `CUser::Damage`, for damage of kind 3, absorbs 1 point of a blow of half the maximum HP or more; it
is used by the guards' blows on players, built: `docs/original/guards.md` section 6.)

## 3. Linking: `0x0801`

Confirmed, `CUser::ItemGemAdd` 0x461310.

Client → server, 8 bytes (`TPC_GEM_ADD`): `u8 lapisBag, u8 lapisSlot, u8 itemBag, u8 itemSlot, u8 hammerBag, u8 hammerSlot` (0, 0 without
a hammer). Ignored without an answer unless:

1. both bags at most the bag count (`m_nMaxBag`; bag 0 allowed), both slots below 24, both slots hold items, and the item's count is **1**;
2. `EnableGemAdd`: the item is a **cloak** (real type 24: always yes, see deviation 1), or the lapis is item type 30, fits (section 1) and
   **`CItem::EnableGem`** (0x4ba050) allows it: no gem of the item is already this lapis' type id, and the item's `slot` minus the sizes of
   its known gems is at least the lapis' size;
3. the item has an empty gem position (`CItem::GetGemSlot`: the first of 0 to 5 that is 0);
4. the price `P = trunc(f32(GetGemMoneyByGrade(g)) − f32(VG) / 10)` (`VG` the character's `vg`, code `+0x16a`) is at most the money.

Then the money − `P` (the price is taken whatever happens). The rate `R = R0` (section 1). **With a hammer** (hammer bag ≠ 0, the place in
the bags, an item there of real type 26 and special 36): `R = trunc(R × f32(hammer reqvg) / 100)` and **one hammer is used up**
(`CUser::ItemUseNSend`: `0x050A` with what is left, `docs/original/item-use.md` section 3); otherwise the hammer bytes count as 0, 0.
Then `R` limited to **1 to 50**; success when `rand() % 100 < R`. The limit holds every hammer back, the Operator's Exclusive (100/82,
2000 %) too: in the original it only brings any lapis to 50 % (deviation 4).

Either way **one lapis is used up** (the slot emptied at 0). The answer **`0x0801`** (20 bytes, `TP_GEM_ADD`): `u8 success, u8 lapisBag,
u8 lapisSlot, u8 lapis count left, u8 itemBag, u8 itemSlot, u8 gem position, u8 lapis type id, u8 0, u8 0, u8 0, u8 0, u32 money, u8
hammerBag, u8 hammerSlot`.

- **Success**: the gem position takes the lapis' type id; for an equipped item its effects come on (section 2), the weapon's or armour's
  element again, and the numbers are sent (speeds, maximum HP, SP, MP when changed). Then, for a **dangerous** lapis, if the player has a
  **Lucky Charm** (the first in bags 1 to the bag count, slots 0 to 23), one is used up (even though nothing failed).
- **Failure**: the item stays as it was. For a dangerous lapis: with a Lucky Charm one charm is used up and the item stays; without, the
  item is destroyed (section 9).

## 4. Extraction: `0x0802`

Confirmed, `CUser::ItemGemRemove` 0x4642d0, `ItemGemRemoveAll` 0x461fc0, `ItemGemRemovePos` 0x4633a0.

Client → server (`TPC_GEM_REMOVE`): `u8 bag, u8 slot`, and in the 8-byte form `u8 flag, u8 gemPos, u8 hammerBag, u8 hammerSlot`. The
English client always sends 4 bytes: **flag 0, all gems** (`CLIENT-FACTS.md`). Ignored unless the bag is at most the bag count and below 6,
the slot below 24 (below 13 in bag 0), an item is there with count 1, and it is not a cloak (real type 24). Flag 0: all gems; flag 1 with
`gemPos` below 6: that gem; anything else nothing.

The answer **`0x0802`** (34 bytes, `TP_GEM_REMOVE`): `u8 success, u8 bag, u8 slot, u8 gemPos, u8 gemBag[6], u8 gemSlot[6], u8 gemTypeId[6],
u8 gemCount[6], u32 money`: where each lapis taken out went (in order, the rest 0), `gemPos` 255 for all gems.

**The US client reads 33 bytes, without `gemPos`** (`docs/client/CLIENT-FACTS.md`, "Compared with the server's docs"; game.exe `0x57cb40`):
the 34-byte form only with region feature 0 on, the same switch that makes it send the 8-byte request. Read one byte off, `gemPos` (255)
is taken as the first lapis' bag and the money loses its top byte (the gold shown becomes money × 256 until the next money update). The new
server answers in the request's form: 33 bytes to the 4-byte request, 34 to the 8-byte one (deviation 5).

**All gems** (no hammer): nothing without a gem. The price is the sum over the known lapis among the gems of `max(0, price(g) − VG / 10)`
(integer division here); nothing when the money is short. The rate is `R0` of the **first** gem (lowest position; nothing when it is not a
known lapis), limited to 1 to 50; success when `rand() % 100 < R`.
- **Success**: the player needs one empty bag place per gem (bags 1 to the bag count, slots 0 to 23, in that order); **without enough,
  nothing happens** (no money taken). Otherwise the money − price, and each known lapis becomes a **new lapis item** (count 1, its quality,
  make type `'G'`, made now, a new unique id) in the next free place; the item's gems all become 0 and its quality is cut to its
  definition's (`quality`, the lapis' share of full quality is gone).
- **Failure**: the money − price, the gems are **lost** (all 0, quality cut the same way), `0x0802` with success 0, `gemPos` 255 and the
  places 0; if any of the gems was **dangerous**, the item is destroyed (section 9). (No Lucky Charm here.)

**One gem** (flag 1, only from a client with the other form): that gem must be a known lapis; price `max(0, price(g) − VG / 10)`; nothing
when the money is short or the hammer is not an **Extraction Hammer** (bag ≠ 0, in the bags, real type 26, special 58). **One hammer is used
up**, then success when `rand() % 100 < hammer reqvg`. Success: one free place needed (without one, nothing more happens: the hammer is
gone, the money not taken); the money − price, the lapis to that place as above, the gem 0, the quality cut. Failure: the money − price, the
gem lost, and the item destroyed if that lapis was dangerous.

In both, for an equipped item the effects of the gems taken come off, the elements are worked out again, the wear level again (the cut
quality), and the numbers are sent.

## 5. Enchanting: `0x0805`

Confirmed, `CUser::ItemLapisianAdd` 0x45fb20.

Client → server, 9 bytes (`TPC_LAPISIAN_ADD`): `u8 lapisiaBag, u8 lapisiaSlot, u8 materialBag, u8 materialSlot, u8 materialCount, u8
equipBag, u8 equipSlot`. Ignored without an answer unless:

1. the Lapisia's and the equipment's bags at most the bag count and slots below 24, both hold items (the material's place is **not**
   checked in the original: an out-of-range place is read out of bounds; here it is simply no material);
2. with a material: a count of 1 or more and at most what the slot holds (above 100 it counts as 100);
3. the equipment's count is 1 and **`CUser::EnableLapisianAdd`** (0x45f960) allows it: the equipment is a cloak (always yes, deviation 1),
   or the Lapisia is item type 95, the equipment is craftable (`server` ≠ 0) and has a `reqluc`, a material is given whose `itemid` is that
   `reqluc`, and the Lapisia fits (section 1);
4. its enchant step (the tenth craft number: weapons `v`, armour `v − 50` when `v` > 50; 0 for a name that is not 20 characters) is below
   **20**;
5. the price `GetGemMoneyByGrade(Lapisia's reqig)` (halved with `NEW_SERVER`; not built, by decision: the price stays full, `docs/original/operator-console.md` section 4) is at most the money.

Then the money − price. The roll (`SRandom`, a Mersenne Twister seeded with `time(0)` xor the session's id, `% 1000000`; the kit's [`enchant-seed`](community-fixes.md#enchant-seed) mixes the CPU cycle counter in): success when
**`materialCount + r < rate[step]`**, `rate` from `g_LapisianEnchantPercentRate` (0x542058, in millionths), weapon / armour:

| step | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 to 19 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| weapon | 700000 | 500000 | 300000 | 90000 | 30000 | 10000 | 3000 | 1000 | 300 | 100 | 30 | 10 | 1 | 1 | 1 |
| armour | 990000 | 980000 | 970000 | 300000 | 100000 | 30000 | 10000 | 3000 | 1000 | 300 | 100 | 30 | 10 | 3 | 1 |

(The equipment counts as armour when it is not a weapon, real types 1 to 15.) This is the **built-in** table; at start the server
replaces it with `Data/ItemEnchant.ini` (section 2). The original patch's file, in the same millionths (the rate of the step being left):

| step | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 to 19 | 20 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| weapon | 900000 | 800000 | 700000 | 600000 | 400000 | 200000 | 100000 | 80000 | 60000 | 10000 | 6000 | 2000 | 1000 | 600 | 200 | 0 |
| armour | 990000 | 980000 | 970000 | 800000 | 600000 | 400000 | 200000 | 100000 | 80000 | 60000 | 10000 | 6000 | 2000 | 1000 | 600 (14), 200 (15 to 19) | 0 |

With the built-in table the material count (at least 1) makes a rate of 1 impossible, so weapons stop at +12 and armour at +14; with the
file every step to +20 can succeed (step 20 is refused by check 4 anyway). Either way **one Lapisia** and **`materialCount`** of the
material are used up (slots emptied at 0). The answer **`0x0805`** (36 bytes, `TP_LAPISIAN_ADD`): `u8 success, u8 lapisiaBag, u8
lapisiaSlot, u8 Lapisia left, u8 materialBag, u8 materialSlot, u8 material left (0 without), u8 equipBag, u8 equipSlot, u32 money, char
craftName[21]`.

- **Success**: the step + 1 is written into the craft name (`CItem::SetEnchantStep`: a craftable item whose name is not 20 characters
  first gets "00000000000000000000"; characters 18 and 19 become the step, + 50 for armour). An equipped item's attack (weapon) or
  absorption (armour) changes from the old step's value to the new one's, and `CUser::SendEquipment` shows the new enchant level to the view
  (`0x0507`, `docs/original/items.md` section 2).
- **Failure**: nothing else, unless the Lapisia is **dangerous**: then the equipment is destroyed (section 9; no Lucky Charm).

## 6. New craft stats: `0x0806` and `0x0808`

Confirmed, `CUser::ItemCompose` 0x460550, `ItemConsider` 0x461040.

**The roll** (`CItem::ReGenerationCraftExpansion(resetEnchant, two)` 0x4baa50) chooses which of the nine crafted numbers (STR, DEX, REC,
INT, WIS, LUC, HP, MP, SP) get a value, by the item's `server` column `S`:

- weapons (real types 1 to 15), rings, amulets and bracelets (22, 23, 25): only the six stats: `S` = 1 one of six at random, 2 to 4 that
  many different ones, 5 all but one at random, 6 or anything else all six;
- other items (armour, real types 16 to 21): `S` = 1 one of nine, 2 to 7 that many different ones, 8 all but one, 9 all nine, anything else
  none.

Each chosen number becomes the **smallest of two** (`two` set) or **three** draws `rand() % (W + 1)`, `W` the `reqwis` column; the others 0.
The nine numbers are written into the craft name as two digits each; with `resetEnchant` the enchant step becomes "00" too (not used by
these requests: the enchant level stays). The roll writes characters 0 to 17 and ends the name after character 19, so for an item whose
name is not 20 characters (made by `CItem::Init`, which only clears the first byte) characters 18 and 19 are whatever the object held; the new
server takes them as "00", as `CItem::InitCraftExpansion` gives craftable items.

**`0x0806`** Recreation Rune (6 bytes, `TPC_ITEM_COMPOSE`: `u8 runeBag, u8 runeSlot, u8 itemBag, u8 itemSlot`): bags at most the bag
count, slots below 24, both places hold items, else ignored. Refused with **`0x0806`** (3 bytes, `u8 1`) unless the item is craftable
(`server` ≠ 0 and `reqwis` 1 to 99), the rune has special **62** and the item's make type is not `'Q'` (quest rewards). Otherwise the roll
with **two** draws, the numbers sent for an equipped item, **`0x0806`** (26 bytes: `u8 0, u8 itemBag, u8 itemSlot, char craftName[21]`),
then one rune used up (`ItemUseNSend`).

**`0x0808`** (4 bytes, `TPC_ITEM_CONSIDER`: `u8 bag, u8 slot`): ignored for a bad place. Refused with **`0x0808`** (3 bytes, `u8 1`) unless
the item is not real type 27, craftable, **all nine crafted numbers are 0** (`CItem::EnableConsider`) and its make type is neither `'Q'` nor
`'B'`. Otherwise the roll with **three** draws, free, and **`0x0808`** (26 bytes: `u8 0, u8 bag, u8 slot, char craftName[21]`).

## 7. One item from three: `0x0807`

Confirmed, `CUser::ItemRemake` 0x4608b0.

Client → server, 8 bytes (`TPC_ITEM_REMAKE`): `u8 bag[3], u8 slot[3]`. Ignored for a bad place or an empty one. The answer **`0x0807`**
with `u8 result`: **2** (3 bytes) when any of the three has make type `'Q'`, **3** when any has `'B'`, **1** when they are not the same item
(`itemid`) or it is not craftable. Otherwise **0** (37 bytes): `u8 0` and the new item as `TP_ITEM_UNIT` (34 bytes, `bag, slot, type, type
id, u16 quality, gems[6], count, craftName[21]`).

The three are destroyed one by one, each as in section 9 but without its own refusal checks (an equipped one comes off first, `0x0206`
`bag, slot, 0, 0, 0` for each). The new item (`CItem::Init`: count 1, its quality, make type `'C'`, a new unique id, **new random craft
stats**: `CItem::CreateCraftExpansion` 0x4ba2e0, the roll of section 6 with three draws and enchant "00") goes to the place of the **first of
the three that was not equipped** (bag 0 slot 0 if all were). Gems and enchant level of the three are lost. The original does not check that
the three places differ (deviation 2).

## 8. Cloak badges: `0x0803`, `0x0804`

Confirmed, `CUser::ItemCloakAdd` 0x464ad0, `ItemCloakRemove` 0x464d40.

The cloak's six gem bytes are its badge (`docs/original/enter-world.md`).
- **`0x0803`** (10 bytes, `TPC_CLOAK_ADD`: `u8 bag, u8 slot, u8 badge[6]`): bag at most the bag count, slot below 24, a cloak (real type 24)
  there, else ignored. With less than **450000** money: **`0x0803`** (15 bytes, `TP_CLOAK_ADD`: `u8 0, u8 bag, u8 slot, u8 badge[6], u32
  money`). Otherwise the gems become the badge, money − 450000, `0x0803` with 1 and the new money, and for the equipped cloak (bag 0 slot 7)
  `SendEquipment` (`0x0507` with the badge to the view). No guild check.
- **`0x0804`** (4 bytes: `u8 bag, u8 slot`): a cloak there: its gems become 0, **`0x0804`** (4 bytes, `u8 bag, u8 slot`), and for the
  equipped cloak `SendEquipment`. Free.

## 9. An item destroyed

The equipped item's quality is saved first if it changed (`0x070C`), it comes off (`ItemEquipmentRem`, `SendEquipment` to the view), the
place is emptied, and the player gets **`0x0206`** (7 bytes: `u8 bag, u8 slot, u8 0, u8 0, u8 0`: the place is empty).

## 10. Deviations (all approved or requested)

1. **Approved: cloaks are not linked or enchanted.** For a cloak (real type 24) `EnableGemAdd` and `EnableLapisianAdd` answer yes before looking at
   the lapis or Lapisia, so any item can be "linked" into a cloak (its type id written into the badge bytes, for the price of its grade:
   usually 0) or "enchant" it (an apple as Lapisia, free, to a +20 absorption cloak). The new server treats a cloak like other items:
   no lapis fits it (no placement column means a cloak) and enchanting needs a Lapisia, the material and a craftable cloak (none is).
2. **Approved: remake needs three different places.** The original accepts the same place three times: one item becomes a new one with fresh stats,
   and its object is freed three times (a crash or worse). The new server ignores such a request, as the kit's [`remake-distinct`](community-fixes.md#remake-distinct) does for the original.
3. **Approved: linking, extraction and enchanting need a blacksmith in view** (NPC type 3, as repair deviation 1): the client sends them only with
   an NPC selected, but the original checks nothing, so a modified client could link anywhere.
4. **Approved: the Operator's Exclusive always links.** A Linking Hammer of 1000 % or more (in the data only 100/82, 2000 %) makes the rate
   100 instead of being limited to 50: the hammer was evidently meant to be sure, and the original's limit (0x4616fa) holds it back like
   any other.
5. **Requested: the extraction answer follows the request's form.** The original always sends the 34-byte `0x0802`, which the US
   client (feature 0 off, 4-byte requests) reads one byte off (section 4). The new server sends 33 bytes, without `gemPos`, to a 4-byte
   request, and the original 34 to an 8-byte one.

## 11. The new server

- `ItemDef` keeps the extra columns (`slot` as the lapis room, `reqluc` as the material, `reqwis` as the craft limit, `reqstr` and `exp`);
  lapis effects, elements, state blocks and enchant steps go into `stats::compute` and `combat::gear`, which work the numbers out from
  the equipment each time, so adding or removing a gem or a step is "renumber".
- The decisions are pure functions (`lapis.rs`); the item changes are saved in one transaction with the money (`trade`), with two new
  changes: gems and craft name of a place.
- Random numbers come from the server's generator (`rand() % n` where the original uses `rand`, a uniform draw below 1000000 for the
  enchant). **The enchant rates and add values are the built-in tables of section 5 and 2** (`lapis::ENCHANT_RATES`, a flat 7 attack and 5
  absorption a step); the server does not read an `ItemEnchant.ini`, so it differs from the original with its shipped file (rates from
  step 0, attack from step 4). Not decided yet (2026-11-05).
- The absorption of section 2 is applied to monster blows (and since to player blows, `docs/original/pvp.md` section 9, and the guards' kind 3,
  `docs/original/guards.md` section 6); `0x0525` tells the player. Lapis state blocks make monster skills of that
  state miss.
- Make types are read from the database when a request needs them (`ItemStore::make_types`). Quest items are now stored with make type
  `'Q'` (`CUser::QuestAddItem`), extracted lapis with `'G'`, remade items with `'C'`.
- **Enchant jumps** (an addition, `docs/server/rules.md`): after a successful roll the first matching `enchant` rule draws how many steps the item rises (1 without a rule, at most to step 20); the craft name is written as for one step. Whether the client shows a jump of more than one step is for the live test.
- Not built: `NEW_SERVER` (by decision: the Lapisia price stays full). Built: the game log records, premium accounts (`docs/original/item-mall.md`), the guild house rates (`docs/original/guild-house.md` section 9) and
  random craft stats on dropped and bought items (`docs/original/item-origins.md` section 6).
