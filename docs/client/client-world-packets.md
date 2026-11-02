# In-world packets: NPCs, monsters, combat, HP/MP/SP, items, shops, quests

This page describes what the client `game.exe` (US build 2010-04-26, patch 108) does with each in-world packet the server sends: NPCs,
monsters, attacks and skills, emotes, health and mana, items on the ground and in bags, NPC shops, the warehouse and quests. It also
lists what the client sends for each.

Every server packet goes dispatcher (`0x582e10`) → a small reader (`FUN_005xxxxx`, field order = wire order) → a slot of the network
object's vtable (`0x6A5C20`), which holds the logic; the "logic" column names that function. Field names are the EP4.5 server's own
(from `ps_game.pdb`, the debug symbols shipped with `ps_game.exe`) wherever its struct size matches what the client reads; meanings come
from the client code. Sizes include the 2-byte opcode and exclude the 2-byte length prefix. Status: **confirmed** unless marked
*inferred*.

## In short

- **The server tells the client everything it shows.** Monsters, NPCs, other players and items on the ground appear and disappear only
  when the server says so. A packet about something the client does not know is ignored.
- **Monster health has to be pushed.** The client never asks for a monster's HP. A new monster's bar starts empty until the server sends
  its HP with `0x0608`.
- **Damage numbers come in a fixed order**: HP, SP, MP in attack packets, but HP, MP, SP in recovery packets.
- **Shops fill themselves.** The client reads a merchant's goods from its own data file; the server only answers the buy and sell
  requests.
- **Emotes need no client work**: the chat commands `/victory`, `/clap` and the rest are motion numbers the server passes on to the
  players nearby.

## Conventions

- **Object ids.** The client keeps players, mobs, NPCs and ground items in separate tables, each searched by its own id
  (players `FUN_00447880`, mobs `FUN_00447900`, NPCs `FUN_004479a0`, ground items `FUN_004478c0`). An id must be unique within
  its kind; a packet about an unknown id is ignored (an "enter" for a known id is ignored too, so re-sending is harmless).
- **Positions** are `f32 x, y, z` with y up. Mobs are sent without y (the client takes the ground height).
- **Direction** `u16` = degrees; the facing vector is `(-sin d, 0, cos d)` (0 faces +z).
- **Damage triples are `HP, SP, MP`** (`wDamage[0]` lowers HP, `[1]` SP, `[2]` MP; every attack/skill handler). An EP6 packet
  catalogue labels `0x0605` as health, mana, stamina: wrong order for this client. Recovery packets use **`HP, MP, SP`**
  (`0x0505`, `0x0521`, `0x051F`). Skill levels: `bySkillLV = 100` is treated as level 1 everywhere.
- **Attack result `bySuccess`** (`0x0502`/`0x0503`, skills, mob attacks):

| value | meaning (client behaviour) |
|---|---|
| 0 | hit: damage number, system messages "Wounded <t> for <v>…" (1101) / "Received <v> damage…" (1308) |
| 1 | **critical**: larger damage number, always plays the hit reaction, messages 1102 / 1309 |
| 2, 3 | failed (miss/resist): no damage, "Failed <s> against <t>" (1108), "<p> tried <s> but failed" (1307) |
| 4 | *inferred* special result: plays an extra caster animation; with skill ability `0x17` on a mob it sets the mob's HP to 0 |
| 5 | attack refused: "Insufficient range" (312); the original server also uses `0x0502` with 5 to undo a refused move |
| 6 | "Insuffient energy" (313) |
| 7 | "Unavailable equipment" (314) |
| 8 | "A previous skill is required" (315) |
| 9 | "Recharge must be completed before next use" (215) |
| 10 | "Cannot attack" (316) |
| 11 | "Wrong target" (317) |

Values 5–11 only matter when the attacker is the local player: the client shows the message, plays `ui_error001.wav` and stops.

## NPCs (`0x0Exx`)

Mobs are **not** in this family: they are `0x06xx` (below).

| op | size | fields (PDB name) | logic | what the client does |
|---|---|---|---|---|
| `0x0E01` | 23 | `u32 dwID, u8 byType, u16 wTypeID, f32 fPosX, fPosY, fPosZ, u16 wDir` (`TP_NPC_ENTER`) | `0x552730` | creates the NPC (type/typeId index `NpcQuest.SData`), facing `wDir` |
| `0x0E02` | 6 | `u32 dwID` | `0x54f790` | removes the NPC |
| `0x0E03` | 19 | `u32 dwID, u8 byMotion, f32 x, y, z` (`TP_NPC_MOVE`) | `0x54f7c0` | walks the NPC to x/y/z (turns to face it); `byMotion != 0` is kept as a flag (*inferred* run) |
| `0x0E04` | 6 | `u32` | `0x407660` | **ignored** (empty function) |
| `0x0E05` | 13 | `u8 bySuccess, u32 dwID (NPC), u32 dwUserID, u16 wDamage` (PDB name `TP_NPC_ATTACKMOB`, but the target is a **player**) | `0x54f890` | guard attacks a **player** (looked up with 0x447880, the player list): attack animation, turns to the player (`+0xb8` = 1), the HP (`+0x130`) lowered by `wDamage` when the blow lands. Unknown NPC but known player: the HP is lowered at once (corrected: an earlier version read it as a monster) |
| `0x0E06` | 13 | `u8 bySuccess, u32 dwID (NPC), u32 dwMOBID, u16 wDamage` (PDB name `TP_NPC_ATTACKPC`, but the target is a **monster**) | `0x54f9e0` | guard attacks a **monster** (looked up with 0x447900, the monster list; `+0xb8` = 0): its HP (`+0x58`) lowered when the blow lands, at once for an unknown NPC (corrected: an earlier version read it as a player) |

Client → server: **`0x0E04` (6) `u32 npcId`** when the player clicks an NPC (`0x4416c4`, id from the NPC object). The client does
nothing with a `0x0E04` reply, so the server answers with the NPC's own packets (shop, quest, …).

The `.wld` maps also carry NPC positions, used only for the minimap and quest marks; NPCs on screen come
from `0x0E01`.

## Mobs (`0x06xx`)

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0601` | 17 | `u32 dwID, u8 byNew, u16 wMOBID, f32 fPosX, fPosZ` (`TP_MOB_ENTER`) | `0x552380` | creates the mob from `Monster.SData[wMOBID]` (model, **max HP** from the record), random facing; `byNew != 0` plays the spawn animation. **Current HP starts at 0**: send `0x0608` after it (or the target bar shows empty). A `0x0601` for an id already known is ignored entirely (send `0x0602` first to change `wMOBID`) |
| `0x0602` | 6 | `u32 dwID` | `0x54e7e0` | removes the mob |
| `0x0603` | 15 | `u32 dwID, u8 byMotion, f32 fPosX, fPosZ` (`TP_MOB_MOVE`) | `0x54e810` | moves toward x/z; `byMotion` stored (*inferred* walk 0 / run 1) |
| `0x0604` | 6 | `u32 dwID` | `0x54e8c0` | removes the mob (no existence check) |
| `0x0605` | 17 | `u8 bySuccess, u32 dwID (mob), u32 dwUserID, u16 wDamage[3]` (`TP_MOB_ATTACKPC`) | `0x54e8d0` | mob attack animation (one of its 3 attacks, `Monster.SData`), damage applied to the player when the blow lands; unknown mob: applied at once |
| `0x0606` | 11 | `u32 dwID, u8 byKillerType, u32 dwKillerID` (like `TP_DEATH`) | `0x551c50` | mob dies (death animation if its HP < 1); players attacking it stop |
| `0x0607` | 15 | `u32 dwID, u16 wSkillID, u8 bySkillLV, u16 value[3]` | `0x563b00` | periodic skill effect on a mob: skill type 4 (heal) adds `value[0]` to HP, otherwise subtracts; numbers shown |
| `0x0608` | 14 | `u32 dwID, u32 maxHP, u32 HP` | `0x54eff0` | sets the mob's max and current HP (absolute: mob `+0x54`, `+0x58`, what the target bar draws); dropped when the id is unknown, so it must follow the `0x0601`. PDB `TP_MOB_GETHP` is 10 bytes: the EP4 client wants both values |
| `0x0609` | 8 | `u32 dwID, u8 byAttackSpeed, u8 byMoveSpeed` (`TP_SET_SPEED`) | `0x54f2b0` | mob speeds |
| `0x060B` | 21 | `u8 bySuccess, u32 dwMOBID, u32 dwTarget (player), u8 byAttackType, u16 wSkillID, u8 bySkillLV, u16 wDamage[3]` (`TP_MOB_SKILL_USE`) | `0x565620` | mob skill on a player; `byAttackType` 0/1/2 picks which of the mob's attack animations to play; heal-type skills (type 2/4) heal the mob instead |
| `0x060C` | 27 | same + `u16 wHeal[3]` (`TP_MOB_SKILL_USE_DRAIN`) | `0x566110` | drain: damage to the player, `wHeal` to the mob |
| `0x060D` | 20 | `u8 bySuccess, u32 dwMOBID, u32 dwTarget, u16 wSkillID, u8 bySkillLV, u16 wDamage[3]` | `0x566850` | area (range) skill hit on one player |
| `0x060E` | 19 | `u32 dwMOBID, u32 dwTarget, u16 wSkillID, u8 bySkillLV, u16 wDamage[3]` | `0x566e20` | additional/periodic damage from a mob skill: "Received another <v> damage by <p>'s <s>" (1304) |
| `0x060F` | 16 | `u32 dwMOBID, u32 dwTarget, u16 wDamage[3]` | `0x5670f0` | mirrored damage to a player; note the client subtracts **2 × wDamage[0]** from HP (confirmed in the disassembly), so follow with an HP update |
| `0x0610` | 10 | `u32 dwID, i32 nHP` (`TP_MOB_RECOVER`) | `0x54fb80` | adds `nHP` (signed) to the mob's HP |

There is no client → server `0x06xx` packet: the client never asks for a mob's HP; the server must push it.

## Players: attacks, death, skills (`0x05xx`)

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0502` | 17 | `u8 bySuccess, u32 dwID (attacker), u32 dwTarget (player), u16 wDamage[3]` (`TP_PC_ATTACK`) | `0x54d960` | player hits player: attack animation, pending damage applied when the blow lands (unknown attacker: at once). Local attacker + result ≥ 5: error message |
| `0x0503` | 17 | same, `dwTarget` = mob | `0x54de20` | player hits mob, same pattern |
| `0x0504` | 11 | `u32 dwID, u8 byKillerType, u32 dwKillerID` (`TP_DEATH`) | `0x551b50` | player death animation if HP < 1; attackers stop; for the local player opens the death/revive window (`FUN_004ca4b0`) |
| `0x050D` | 9 | `u32 dwID (buff key), u16 wSkillID, u8 bySkillLV` (`TP_PC_SKILL_SET`) | `0x56c020` | adds/updates a buff on the local player; the u32 is the key for `0x050E`; duration from `Skill.SData` (+0x46 seconds; hours if flag +0x7F = 1; 30 s for type 6) |
| `0x050E` | 6 | `u32 dwID (buff key)` (`TP_PC_SKILL_REMOVE`) | `0x56be00` | removes that buff (message 837 for the resurrection rune) |
| `0x050F` | 15 | `u32 dwUserID, u16 wSkillID, u8 bySkillLV, u16 value[3]` (`TP_SKILL_KEEP`) | `0x563780` | periodic skill effect on a player (heal or damage by skill type) |
| `0x0510` | 13 | `u32 dwUserID, u32 dwTarget (player), u16 wSkillID, u8 bySkillLV` (`TP_PC_SKILL_CASTING`) | `0x563c70` | starts the cast animation toward a player |
| `0x0511` | 20 | `u8 bySuccess, u32 dwUserID, u32 dwTarget (player), u16 wSkillID, u8 bySkillLV, u16 wDamage[3]` (`TP_PC_SKILL_USE`) | `0x567720` | skill on a player/self: effects, damage or heal, messages 1106/1108/1207/1208/1305/1307 |
| `0x0512` | 26 | same + `u16 wHeal[3]` (`TP_PC_SKILL_USE_DRAIN`) | `0x569310` | drain on a player |
| `0x0513` | 20 | as `0x0511` (`TP_PC_SKILL_RANGE`) | `0x563fa0` | area hit on one player ("Received <v> damage by <s>", 1308/1309) |
| `0x0514` | 19 | `u32 dwUserID, u32 dwTarget, u16 wSkillID, u8 bySkillLV, u16 wDamage[3]` (`TP_PC_SKILL_ADD`) | `0x564820` | additional damage on a player |
| `0x0515` | 16 | `u32 dwUserID, u32 dwTarget, u16 wDamage[3]` (`TP_PC_SKILL_MIRROR`) | `0x564ba0` | mirrored damage between players |
| `0x0516` | 13 | as `0x0510`, target = mob | `0x564c90` | cast start toward a mob |
| `0x0517` | 20 | as `0x0511`, target = mob | `0x569b10` | skill on a mob |
| `0x0518` | 26 | as `0x0512`, target = mob | `0x56b430` | drain on a mob |
| `0x0519` | 20 | as `0x0513`, target = mob | `0x564e30` | area hit on one mob ("Wounded <t> for <v>", 1101/1102) |
| `0x051A` | 19 | as `0x0514`, target = mob | `0x565340` | additional damage on a mob |
| `0x051B` | 16 | as `0x0515`, target = mob | `0x565540` | mirrored damage on a mob |
| `0x0525` | 4 | `u16 value` | `0x55e9f0` | "<v> absorption damage." (1011) |

Client → server: **`0x0503` (6) `u32 mobId`** attack a mob (`0x44113b`); **`0x0511` / `0x0517` (7) `u8 byNumber, u32 target`**
use a skill on a player (or self) / on a mob (`byNumber` = the skill's number from `0x0108`); **`0x0502` (6) `u32 playerId`**
attack a player (sender `0x586110`, from the player-target branch of `FUN_0043c000` after the range check, when the target's
faction byte differs from the local player's or in the PvP cases).

## Motions and emotes (`0x0506`)

Client → server `0x0506` (3) `u8 motion`; server → client `0x0506` (7) `u32 dwID, u8 byMotion` (`TP_PC_MOTION`), handled by
`0x54e3f0` for any player in view.

| motion | sent by the client when | client shows (`0x54e3f0`) |
|---|---|---|
| 0 / 1 | stand / sit (sit key, exit dialog) | stand-up / sit-down animation, `ch_mstup001.wav` / `ch_mdown001.wav` |
| 2, 3, 10, 11 | player actions in `FUN_0043c000` (not identified) | other players only: 2 sets a vertical speed (*inferred* jump), 3/10/11 play animations 13/14/15 |
| 116–125 | chat commands `/victory`, `/begging`, `/love`, `/laugh`, `/clap`, `/greeting`, `/start`, `/apoligize` (sic), `/provocation`, `/insult` (sysmsg 5114–5123, `FUN_00475e00`), when `FUN_004c4620()` allows | the emote and a chat line "<p> poses for victory" … "<p> abuses hard." (sysmsg 5228–5237) |

So emotes need no client work: the server only has to accept these motion values and send `0x0506` to the players in view.

## Players: HP/MP/SP, levels, speed

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0505` | 12 | `u32 dwID, i16 wHP, i16 wMP, i16 wSP` (`TP_USER_RECOVER`) | `0x54e2c0` | **adds** the three (signed) values, clamped at 0 |
| `0x050B` | 9 | `u32 dwID, u8 byType, u16 wMax` (`TP_SET_MAX`) | `0x54ef70` | sets one maximum: type **0 HP, 1 SP, 2 MP** |
| `0x050C` | 6 | `u16 wSP, u16 wMP` (`TP_PC_USESPMP`) | `0x54efc0` | subtracts a skill's cost from the local player's SP and MP |
| `0x051F` | 12 | `u32 dwID, u16 maxHP, u16 maxMP, u16 maxSP` | `0x54e350` | sets all three maxima |
| `0x0521` | 8 | `u16 wHP, u16 wMP, u16 wSP` (`TP_USER_RECOVERSET`) | `0x54fd50` | sets the local player's current values (absolute) |
| `0x0522` | 10 | `u32 dwID, u32 dwValue` (`TP_USER_KILLINFO`) | `0x54fd90` | stores the value on that player (+0x2EC; use not traced) |
| `0x051E` | 20 | `u32 dwID, u16 wLevel, u16 wStatPoint, u16 wSkillPoint, u32 dwPrevExp, u32 dwNextExp` (`TP_USER_LEVELUP`) | `0x54eb40` (arg 1) | **level up** with effect and `ch_levup001.wav`; for the local player updates level, points and the exp bar; message 217 (self) / 218 (party) |
| `0x0508` | 20 | same | `0x54eb40` (arg 0) | the same update, effect and sound **deferred**: sets a pending flag on the player (`+0x1c9`); `FUN_00416c60` plays the level-up effect and `ch_levup001.wav` when the next hit is applied to that player's target (`0x416cd0`, called by the `0x0502`/`0x0503`/`0x0504` handlers and the skill-result handlers) or at quest end (`0x0903`). The original server sends `0x0508` for a level-up from the player's own kill, **before** the attack/skill result that kills, so the effect appears with the killing blow; `0x051E` for any other level-up |
| `0x051C` | 8 | `u32 dwID, u8 byAttackSpeed, u8 byMoveSpeed` (`TP_SET_SPEED`) | `0x54f260` | player speeds |

For the HUD of the local player the server needs: maxima (`0x051F` or `0x050B`), current values (`0x0521`), then deltas
(`0x0505`) or absolute updates (`0x0521`) as they change. Damage in attack packets is subtracted by the client itself.

## Items and drops

### Inventory model (client)
Bag 0 = equipment, bags 1–5 = inventory, 24 slots each (`index = bag × 24 + slot`). Item unit as sent (34 bytes,
`TP_ITEM_UNIT` without the leading bag/slot): `u8 byType, u8 byTypeID, u8 byCount, u16 wQuality, u8 byGem[6], char szCraftName[21]`
(order in `0x0204`/`0x0205`: type, typeID, count, quality, gems, craft name). **Gold is item type 26**: in `0x0205` the amount is the
`u32` formed by `wQuality` (low 16 bits) and `byGem[0..1]` (high 16 bits).

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0204` | 70 or 74 | source then destination, each `u8 bag, u8 slot, u8 type, u8 typeID, u8 count, u16 quality, u8 gem[6], char craftName[21]` (34 bytes, `TP_ITEM_BAGTOBAG`); + `u32 dwMoney` when moving **out of** a storage (`TP_ITEM_BANKTOBAG`, 74) | `0x557500`, … | result of an item move: both slots are rewritten (type 0 = empty). Bag **100 = warehouse**, **255 = guild warehouse**, 200 = another storage (UI `FUN_004a36d0`, not identified). The reader picks the handler by the bag pair; source 100, 255 or 200 with a destination outside that storage carries the extra `u32` (fee). Guild warehouse → warehouse (255 → 100) has no handler |
| `0x0205` | 36, or 4 on error | ok: `u8 bag (1–5), u8 slot, u8 type, u8 typeID, u8 count, u16 quality, u8 gem[6], char craftName[21]` (`TP_ITEM_GET`); error: `u8 0, u8 code` | `0x55a670` | pickup result: stores the item, `ch_pikup001.wav`, "Attained <i>" (52) or for gold "Gained <b>G" (53) and adds to money. Error codes: 0 "This doesn't belong to you" (202), 1 "Insufficient inventory" (406), 2 "Unavailable quest item" (407), 3 silent, 4 "Unavailable realm item" (408), 5 "Cannot possess any more gold" (467) |
| `0x0206` | 7 | `u8 byBag, u8 bySlot, u8 byType, u8 byTypeID, u8 byCount` (`TP_ITEM_DROP`) | `0x55aa90` | the slot after a drop: `byCount` = what is left (0 empties it); bag 0 also removes the equipment's look/effects |
| `0x0401` | 22 | `u32 dwID, u8 byKind, u8 byType, u8 byTypeID, u8 byCount, f32 fPosX, fPosY, fPosZ` (`TP_ITEM_ENTER`) | `0x55acb0` | item on the ground (model from type/typeID). `byKind` = drop height for the falling animation: 0 none, 1 → 0.1, 2 → 1.0, 3 → 2.0. Type 44/typeID 246 is a special ground object, at most 100 kept (oldest removed, when feature `0x30` is on) |
| `0x0402` | 6 | `u32 dwID` | `0x557690` | removes the ground item |
| `0x0403` | 4 | `u8, u8` | `0x50b250` | **ignored** (empty function) |

Client → server:
- **`0x0205` (6) `u32 groundItemId`**: pick up (sender `0x5859a0`, called at `0x43fcd3` with the ground item's id).
- `0x0204` (6) `u8 srcBag, srcSlot, dstBag, dstSlot` (bags as above); `0x0206` (5) `u8 bag, slot, count` drop.

## NPC shops and warehouse (`0x07xx`)

The shop window (`FUN_004f7720`, grid of 6 × 5 = 30 items per page) fills itself from the merchant's `NpcQuest.SData` record
(item count at +0x20, then `u8 type, u8 typeID` pairs) when the player opens it after clicking the NPC (`0x0E04`); the server
does not have to send the goods.

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0701` | 2 + 2n | `u8 n`, then n × `u8 type, u8 typeID` | `0x557d40` | **replaces** the open shop's goods list (up to 256 entries) |
| `0x0702` | 12 | `u8 bySuccess, u8 byBag, u8 bySlot, u8 byType, u8 byTypeID, u8 byCount, u32 dwMoney` (`TP_SHOP_BUY`) | `0x557d80` | 0: item put in bag/slot (durability from `Item.SData`), **money set to `dwMoney`** (the new total), "Attained <i>" (52), `ui_pay001.wav`; 1: "Insufficient gold" (303); 2: "Inventory is full" (238) |
| `0x0703` | 12 | same fields (`TP_SHOP_SELL`) | `0x557e90` | 0: the slot now holds `byCount` (0 empties it), money = `dwMoney`, "Sold <i>" (54); 1: "No item located on that spot" (239) |
| `0x0711` | 7 + 33n | `u32 dwMoney, u8 n`, then n × `u8 bySlot, u8 byType, u8 byTypeID, u16 wQuality, u8 byGem[6], u8 byCount, char szCraftName[21]` (`TP_CHAR_BANK`, at most 50 per packet) | `0x54d130` | warehouse contents and warehouse money; send several packets for more than 50 items |

Client → server: **`0x0702` (8) `u32 npcId, u8 listIndex, u8 count`** buy (`listIndex` = position in the goods list);
**`0x0703` (5) `u8 bag, u8 slot, u8 count`** sell (`TPC_SHOP_BUY`/`TPC_SHOP_SELL`, sent by the shop window `FUN_004f8220`).
Moving items into or out of the warehouse is `0x0204` with bag 100 (above).

## Quests (`0x09xx`)

The client keeps two lists: **active quests** (10-byte records: `u16 questId, u16 value, u8 count[…]`) and **finished quests**
(`u16 questId, u8 success`). Quest texts, targets and rewards come from `NpcQuest.SData` (`FUN_00479360(questId)`).

| op | size | fields | logic | what the client does |
|---|---|---|---|---|
| `0x0901` | 3 + 7n | `u8 n`, then n × `u16 questId, u16 value, u8 count0, u8 count1, u8 count2` | `0x561870` | adds active quests (login) |
| `0x0906` | 3 + 3n | `u8 n`, then n × `u16 questId, u8 success` (nothing more if n = 0) | `0x561960` | adds finished quests (login) |
| `0x0902` | 8 | `u32 dwNPCID, u16 wQuestID` | `0x5619f0` | quest accepted: `bgm_quest01.wav`, removes it from the finished list (repeatable quests), adds it to the active list; its `u16 value` starts as the quest's `NpcQuest.SData` field +0x30 (*inferred* time limit) |
| `0x0905` | 6 | `u16 wQuestID, u8 byIndex, u8 byCount` (`TP_QUEST_UPDATE`) | `0x5615f0` | sets progress counter `byIndex` (0 or 1) of an active quest and prints it: faction kills "battles against the other faction : <v>/<v2>" (349/350) or monster kills "<p> <t> battle : <v>/<v2>" (351/352) |
| `0x0907` | 4 | `u16 wQuestID` (`TP_QUEST_END_SELECT`) | `0x5615d0` | opens the reward choice for that quest; the player's pick comes back as client `0x0907` |
| `0x0903` | 22 | `u32 dwNPCID, u16 wQuestID, u8 bySuccess, u8 byResultType, u32 dwExp, u32 dwMoney, u8 byBag, u8 bySlot, u8 byItemType, u8 byItemTypeID` (`TP_QUEST_END`) | `0x561c20` | `bySuccess` 0: "Failed the quest" (244). Otherwise "Completed the quest" (243), `bgm_quest02.wav`, adds `dwMoney` to money ("Gained <b>G"), puts the reward item (count 1) in bag/slot if `byBag` ≠ 0, plays the quest-done effect; if result `byResultType` names a follow-up quest, the quest dialog continues with it. Then the quest leaves the active list and goes to the finished list (or, for a repeatable quest, out of it). **`dwExp` is not used here**: send the experience change separately |

Client → server: **`0x0902` (8) `u32 npcId, u16 questId`** accept; **`0x0903` (8) `u32 npcId, u16 questId`** hand in
(quest window); **`0x0907` (5) `u16 questId, u8 resultIndex`** chosen reward (`TPC_QUEST_END_SELECT`); **`0x0908` (4)
`u16 questId`** give up (no reply packet is handled for it).

## Gaps

- Ghidra's analysis had not followed five switch tables in `FUN_0043c000` (the player-action code: target kind 1 ground item,
  2 player, 3 mob, 4 NPC), leaving `0x43F000–0x441FFF` undefined; following them fixed it (function 21 KB → 30 KB). The two send stubs
  only called from there, `0x5859a0` (`0x0205`) and `0x586110` (`0x0502`), were not functions; they are now, and the
  [packet catalogue](client-packets.generated.md) lists both. A project-wide pass ([Packet handling](client-packets.md#code-ghidras-analysis-missed))
  found no other missing request.
- Result value 4, `0x0522`'s value and `0x0520`/`0x0524` are not traced.
