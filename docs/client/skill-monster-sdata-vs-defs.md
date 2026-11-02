# The client's other SData files against the server

This page compares the rest of the data files of the client `game.exe` (US build 2010-04-26, patch 108) with what the original EP4.5
server `ps_game.exe` uses: `Skill.SData` and `NpcSkill.SData` with the database's `Skills` table, `Monster.SData` with `Mobs`,
`Cash.SData` with `ProductList`, `GuildHouse.SData` with `GuildHouse.ini`, `KillStatus.SData` with the server's own copy. `Item.SData` is
on [Item.SData against the server](item-sdata-vs-defs.md), `NpcQuest.SData` on [NpcQuest.SData against the server](npcquest-sdata-vs-world.md).
As there, **the database wins**: the client file only decides what the client shows and what it refuses before sending. What the client
does with a value is *inferred* (not read in `game.exe`) unless marked.

## In short

- **Skills**: 80 of the 978 skill records differ. Most players see only a tooltip or cast bar that does not match, but two differences
  change what they can do: archers cannot learn Trap, and Pollution Zone asks one skill point more in the client.
- **Monsters**: one spawned boss, Kimuraku, has ten times less HP in the client's file, so its health bar stays full for most of the
  fight. A server can fix that by sending the real HP with `0x0608`.
- **Monster skills** differ in 34 records, but monster skills show no numbers, so players see at most a different state icon.
- **Item mall**: the client lists one product, "Phukey Long Cape", that the server's product table does not sell.
- **Kill ranks** are byte-identical on both sides, and guild-house prices agree.

## Reading the files

- `Skill`, `NpcSkill`, `Monster`, `Cash` are SEED-encrypted (checksums checked); `GuildHouse`, `KillStatus` and `PriestTalk` are plain.
- Each was read with Parsec's layout (the EP4 branch), and **every file is rebuilt byte for byte** from its records, so nothing is left
  unexplained. Values are compared at the client field's width (the database keeps 16-bit fields as signed `smallint`).
- **`Skill.SData`**: `u32` 326, then per skill 3 records (levels 1 to 3): name, description, 116 fixed bytes (Parsec's `SkillDefinition`
  without the EP6 `Effect` byte and abilities 4 to 10). Group *n* is skill id *n* + 1 (group 227 is "Warehouse Call", skill 228).
- **`NpcSkill.SData`**: the same record layout, `u32` 348 groups; only the first record of each is used. Group *n* is the monster skill
  with `Skills` id *n* + 1 at level 100.
- **`Monster.SData`**: `u32` 2,474, then per monster a name and 29 fixed bytes (Parsec's `MonsterRecord`); record *n* is mob id *n*.
- **`Cash.SData`**: `u32` 85 products; each `u32` index, bag (the mall tab), icon, cost, 24 × (`u32` item id = type × 1000 + type id,
  `u8` count), then name, product code and description.
- **`GuildHouse.SData`**: `u32` 1, house price, upkeep; 36 × 16-byte NPC upgrade rows; 24 × `u32` NPC ids. **`KillStatus.SData`**: `u32`
  60; each `u8` faction, `i32` kills, `u16` rank, 6 × (`u8` bonus type, `u16` value).

## Skills (`Skill.SData` against `Skills` levels 1 to 3)

Same 978 records on both sides; 80 differ. Icon, animation, sound and the toggle byte exist only in the client and are left out.

**Not real differences (27 records).** Remedies, nostrums, EXP stones and gold boosts (skills 237 to 262): the server has `keeptime` in
seconds with `fixrange` 2, the client the same duration in hours with `fixrange` 1 (10,800 against 3, and so on). The durations agree. On
the server, `fixrange` 1 means "kept on death, counted in hours" (see [Skills](../original/skills.md#1-definitions)), so **on the server
these buffs end on death**, where the client's own copy says they last. The server decides, and it sends the buff's end itself
(`0x050E`).

**Player skills that differ** (all learnable: `typeshow` ≠ 0, a job flag set; jobs in `byJob` order, the Light names):

| Skill | Server (`Skills`) | Client | What players see |
|---|---|---|---|
| 113 Trap Lv1 to 3 | jobs Ranger **and Archer** | Ranger only | the archer's skill window most likely does not offer it, so archers cannot learn it although the server allows it (*live*) |
| 158 Pollution Zone Lv1 to 3 (Fighter) | **3/3/4 points**, weapon types 1 to 8, lasts **15** s | 4/4/5 points, types 1, 3, 6, lasts 12 s | the window asks one point more; the client may refuse to send with 3 points, and the client allows fewer weapons (*live*) |
| 94 Gruesome Attack Lv1 to 3 (Ranger) | weapon types 1 to 15 | types 9, 10, 15 | the client may refuse the skill with other weapons (*live*); the server allows them |
| 156 Enhance Wind Spin Lv1 to 3 (Fighter) | weapon types 2, 4, 5, **6** | 2, 4, 5 | as above for type 6 |
| 102 Healing Lv1 to 3 (Priest) | cast time **6/5/4** (× 250 ms) | 7/7/7 | the client's cast bar is longer than the server's cast |
| 168 Healing Prayer Lv2, 3 | **7/6** | 8/8 | as above |
| 186 Recovery Lv1 to 3 | **10** | 12 | as above |
| 26 Potential Nimble Lv1 to 3 (Ranger) | two abilities | a third: ability 20 +8/+15/+25 | the tooltip promises a bonus the server does not give |
| 33 Slow Lv1 to 3 (Archer) | second ability value **2** | 0 | tooltip |
| 67 Rapid Shot Lv3 (Archer) | no added damage | +15 HP damage | the tooltip shows 15 more damage than the server deals |
| 312 Dark Panic Lv1 to 3 (Ranger) | state type **4** | 0 | the server's resist checks use state 4 (see [Hit or miss with a skill](../original/skills-use.md#11-hit-or-miss-with-a-skill)); the client's copy has none |

**Item skills that differ:**

| Skill | Items | Server | Client |
|---|---|---|---|
| 214 Blackart Nostrum | 100/12 (mall) | ability value **10** | 300 |
| 267 6-hour Continuous Resurrection | 100/130 (mall) | lasts **8 h** (28,800 s) | 6 h (21,600 s) |
| 280 to 285 stat remedies Lv3 | 100/147, 150, 153, 156, 159, 162 (mall) | **+80 for 5 h**, kept on death | no bonus, 1 h: the client shows the Lv3 remedies as doing nothing |
| 274, 275 Goddess Lv3 | Adam's and Eve's apples (44/221, 44/222, no source) | transformation 18/196, 21/197 | 20/679, 23/679 |
| 193 Snowball fight | the snowball (44/147) | cooldown 2, range 14 | 3, 0 |
| 276 Untouchable Lv1 | 44/168 (no source) | lasts **5** s | 4 s |
| 250 Lv2, 287, 295 to 300 | GM item 44/242, 100/209, 44/162 to 167 (no placed seller) | data | empty records |

Skill 174 Lv3 is an unused row (grow 2 against 1). None of the item skills above is reachable in the normal world except through the
item mall; the snowball is sold by one placed merchant.

## Monster skills (`NpcSkill.SData` against `Skills` level 100)

Client 348, server 355 (349 to 355 are unnamed placeholders the client lacks); 34 differ. The client uses these for the effects and
messages it shows; the server decides hits, damage and states.

- The status skills Spell Block, Crawl, Panic, Curse, Low-Speed, Toxin, Disease, Confusion, Kwip's Kurse, Slumber (214 to 242, two
  levels each): success **7 and 10** on the server against 8 and 13 (Slumber 12 against 15) in the client.
- The Greendieta skills (267 to 270): cooldown **20** against 2, range **30** against 38, and Fire's over-time damage **380** against
  760.
- Magic Spout (271) damage **344** against 430 and range 22 against 35; Elemental Shock (272) **675** against 975, range 25 against 30;
  Attribute Remove (273) range 20 against 30; All Damage (274) **1,300 to each of HP, SP, MP** against 0/666/666.
- Excrutiating Pain (253) cooldown 15 against 30 and state 2 against 1; Bind Down (168) state 4 against 2; Mana Glimmer (164) state 11
  against 0; Raid Block (211, 212) a second ability (33, +1) only on the server; Trapping Shot (121) attack type 3 against 2.

None of this is shown as a number to players (monster skills have no tooltip); the visible part is at most a different state icon.

## Monsters (`Monster.SData` against `Mobs`)

Client 2,474 (ids 0 to 2,473), server 2,747; the 273 the client lacks spawn nowhere. 98 differ, **33 of them spawn** (in the server's map
spawn areas):

| Monsters | Server | Client | Effect |
|---|---|---|---|
| Kimuraku (835, map 11) | HP **1,115,438** | 115,438 | the client takes **max HP from its file** (see [World packets](client-world-packets.md#mobs-0x06xx), `0x0601`), so the target bar stays full for the first nine tenths of the fight |
| 942 to 964, maps 23 and 24 (Aidion, Astaroth, the Kobolt and Vampiric monsters, 19 kinds) | `day` 1 (2 for 957, 958), `size` 1 or 2 | `day` 2 (3 for 957, 958, 960), `size` one more for six of them (956, 958, 960, 961, 963, 964) | `size` is part of the server's reach (see [Fighting monsters](../original/combat.md#2-the-attack-tick): a player's reach includes the monster's `bySize`); the client may walk closer than it needs to. What `day` does is not known |
| 13 quest monsters (134, 302, 304, 309, 310, 400, 534, 627, 633, 648, 2069, 2143, 2149) | quest item 1077 to 1142 | 0 | the server's own quest-drop field; the client does not need it (*inferred*) |

The other 65 are monsters that spawn nowhere (HP, AI, movement and attack types differ). Level, element and the movement times of every
spawned monster agree.

## The item mall (`Cash.SData` against `ProductList`)

85 products in the client, 84 in the database; the 84 agree in cost and contents (by product code). The client also lists **"Phukey Long
Cape"** (code `B2_HpoLM4801`, tab 4, 2,999 points, item 39/243), which the database does not sell: a purchase of that product finds no
row on the server.

## Guild houses, kill ranks, priest talk

- **`GuildHouse.SData`**: house price 50,000,000 and upkeep 2,000 agree with `GuildHouse.ini`; its 24 NPC ids are the ini's
  `NpcLevel_Light` and `NpcLevel_Dark` NPCs (gatekeepers 37 to 39 and 41 to 43, shops 196 to 225). The upgrade rows were not compared.
- **`KillStatus.SData`**: **byte-identical** to the server's `PS_Game/Data/KillStatus.SData` (SHA-256 `ae82bd49…04c7`). Nothing to
  compare.
- **`PriestTalk.SData`**: plain, 25 entries of **Korean** text (code page 949) even in the US client; no server side. Keep as shipped.

## For a server

Nothing to change. The visible differences, all the original's own:

- archers cannot learn Trap (the client does not offer it);
- Pollution Zone asks one skill point more in the client;
- Kimuraku's target bar (max HP ten times lower in the client);
- the Lv3 stat remedies show no bonus (mall items);
- tooltips: Potential Nimble, Slow, Rapid Shot Lv3; cast bars of Healing, Healing Prayer and Recovery longer than the cast.

Worth a look in a live test: whether the client sends Pollution Zone with exactly 3 points, Gruesome Attack with a sword, and whether it
refuses to attack from the server's reach on the Aidion and Astaroth monsters (*live*).

**What the client does, read in `game.exe`** (see [Client facts](CLIENT-FACTS.md#sdata-differences-what-the-client-does-with-its-own-values)):
the skill window's learn arrow uses the client's job flags, levels and points (Pollution Zone needs 4 points in this client; an Archer
cannot raise Trap past the level it has); no weapon check was found before a skill is sent; Kimuraku's bar can be fixed from the server
with `0x0608` (`u32 id, u32 maxHP, u32 HP`), which overwrites the client's max HP.
