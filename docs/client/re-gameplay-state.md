# The client's model of the game

What `game.exe` keeps in memory about the game world: the local character, the other players, monsters, NPCs and items on the
ground, the inventory, how it uses the `.SData` tables, skills, buffs and the party. Everything here was read statically in the Shaiya
US client (`game.exe`, 2010-04-26, patch 108, SHA-256 `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`). Nothing was
run. Addresses are [virtual addresses](glossary.md#va) in that file. "confirmed" means the function named was read and the field is written or read
there as described; "confirmed (disasm)" means it was read in the disassembly; [*inferred*](glossary.md#inferred) marks a meaning the code does not state by
itself; **open** marks what was not settled. Field names in `code` style that look like `byJob` are the original server's packet
field names, matched by position.

## In short

- There is no single "game" object. The local character's numbers (level, money, experience, stats, points, position) live in plain
  global variables; everything in view (players, monsters, NPCs, items on the ground) lives in one entity manager with one lookup
  table per kind, keyed by the server's id.
- The local player is also an ordinary player entity in that manager. Its HP, MP and SP, name, looks and equipment are kept on the
  entity; the HUD reads them from there.
- The inventory is a fixed array: 6 bags (bag 0 = equipment) of 24 slots, 34 bytes per item, the same item unit the server sends.
- `Item.SData`, `Skill.SData` and `Monster.SData` ([SData](glossary.md#sdata)) are loaded into arrays indexed directly by type/id, so a lookup is two array reads.
  The in-memory item record is laid out differently from the file (a few padding bytes move the fields).
- Damage is not applied when the result packet arrives: it is stored on the attacker and applied when the attack animation lands.
- Buffs, the skill list and the party are standard containers in globals; each buff remembers its start time and duration in
  milliseconds, taken from `Skill.SData`.

### Using this page

- **If you write a server:** the bag array, the entity records and the stat globals say which values a packet must fill and how wide they are; the checklist is [What a server must do for this client](afterglow/server-checklist.md#client-facts).
- **If you patch or mod the client:** sections 2, 3 and 5 give the addresses of the globals, the bag array and the entity managers, the places a patch or a hook (see [the companion DLL](fixes-dll.md)) usually starts from.
- **To check a claim yourself:** open `game.exe` in a decompiler as [Tools and method](tools-and-method.md) describes and go to the handler named in the row (for example the `0x0105` handler in section 2).

Related pages: [client-world-packets.md](client-world-packets.md) (what each world packet does on screen),
[client-social-packets.md](client-social-packets.md) (party, guild, trade), [client-actions.md](client-actions.md) (what the client
sends for each action), [re-engine.md](re-engine.md) (the engine around this state), [re-network.md](re-network.md) (how packets arrive).

## 1. Where the state lives

| What | Address | Proven by | Confidence |
|---|---|---|---|
| Entity manager (players, mobs, NPCs, ground items) | `0x7051a0` (static object) | `ECX = 0x7051a0` before the player lookup at `0x54ced5` | confirmed (disasm) |
| Local player's id | `0x84e6f4` | set by `0x54ca50`, compared in every player handler | confirmed |
| Local player's entity | `0x84506c` (pointer) | created by `0x4c8bf0`, cleared on map change (`0x46b8bb`) and in the frame states | confirmed |
| Current map id | `0x84e6e0` (u16) | set with the id by `0x54ca50` | confirmed |
| Local character numbers | `0x84d670`–`0x84d6de`, `0x84e6e4`–`0x84e6f0` | `0x54cdc0` (`0x0105`) and section 2 | confirmed |
| Inventory | `0x84e6f8` | `0x54cf90` (`0x0106`), section 3 | confirmed |
| Buffs, skills, party | `0x21e4820`, `0x21e47dc`, `0x21e4838` | sections 8 and 9 | confirmed |

Every server packet about the world goes dispatcher → reader → a slot of the network object's vtable (`0x6A5C20`); the handlers named
below are those slots ([re-network.md](re-network.md), "Dispatch").

**How to find it.** The four lookups below are the most called small functions in the client (`0x447880` alone has 168 callers); the
global they are called on is the manager. The local character's globals are the targets of the `0x0105` handler `0x54cdc0`.

## 2. The local character

### Character numbers (globals)

| Address | Type | Name | Meaning | Proven by | Confidence |
|---|---|---|---|---|---|
| `0x84d670` | u32 | money | gold carried; written by about 30 handlers (shop, trade, quest, pickup) | `0x54cdc0`, `0x557d80`, `0x55a670` | confirmed |
| `0x84d678` | u32 | level | shown as `Lev.%d` in the status window | `0x54eb40` (level up), `0x4f3f80` | confirmed |
| `0x84d67c`, `0x84d67e`, `0x84d680` | u16 | max HP, MP, SP | from `0x0105`; also copied to the entity | `0x54cdc0` | confirmed |
| `0x84d684` | u32 | experience | current total | `0x54cdc0`, status window | confirmed |
| `0x84d688`, `0x84d68c` | u32 | previous / next level experience | the exp bar's two ends | `0x54cdc0` | confirmed |
| `0x84d690` | u16 | STR | | `0x54ef10`, `0x54fe30` | confirmed |
| `0x84d692` | u16 | INT | | same | confirmed |
| `0x84d694` | u16 | DEX | | same | confirmed |
| `0x84d696` | u16 | WIS | | same | confirmed |
| `0x84d698` | u16 | REC | | same | confirmed |
| `0x84d69a` | u16 | LUC | | same | confirmed |
| `0x84d6a4`–`0x84d6b8` | u32 × 6 | stat bonuses | added to the six stats in the status window (`"%d + %d"`) | `0x4f3f80` | confirmed (read); writer not found |
| `0x84d6dc`, `0x84d6de` | u16 | stat points, skill points | `0x54ef10` subtracts the stats it adds | `0x54cdc0`, `0x54ef10` | confirmed |
| `0x84e6e4`, `0x84e6e8`, `0x84e6ec` | f32 | position x, y, z | from `0x0105`; the local entity is placed here | `0x54cdc0`, `0x4c8bf0` | confirmed |
| `0x84e6f0` | u16 | direction | degrees, turned into the entity's facing vector | same | confirmed |
| `0x21e4940`, `0x21e4941` | u8 | attack speed, move speed (local copy) | from `0x051C` | `0x54f260` | confirmed |
| `0x21e4942` | u8 | faction | 0 Light, 1 Fury, 2 not chosen yet (faction screen) | `0x42cc10` state 3, `0x0109` | confirmed |
| `0x21e4f68`–`0x21e4f74` | u32 × 4 | kill counters | the four `dwKillCount` values of `0x0105` | `0x54cdc0` | confirmed |
| `0x21e54e4` | char * | guild name | heap copy, replaced when it changes | `0x54cdc0` | confirmed |

The stat order in memory is STR, INT, DEX, WIS, REC, LUC. It follows from two handlers that receive the six values in different
orders: the stat-point result `0x54ef10` (vtable `+0x80`, wire order STR, DEX, REC, INT, WIS, LUC as in the server's
`TP_USER_STATUSUP`) and the stat-reset result `0x54fe30` (vtable `+0x468`, wire order STR, REC, INT, WIS, DEX, LUC as in the
server's reset struct; *inferred*: opcode `0x0214`, reader `0x580680`, same shape). Both put each stat at the same address.

### The `0x0105` handler

```c
// 0x54cdc0, network vtable +0x28: TP_CHAR_DATA, field order = wire order
void on_char_data(Net *net, u16 stat_pts, u16 skill_pts, u16 max_hp, u16 max_mp, u16 max_sp,
                  u16 dir, u32 prev_exp, u32 next_exp, u32 exp, u32 money,
                  f32 x, f32 y, f32 z, u32 kills[4], char *guild)
{
    g_max_hp = max_hp;  g_max_mp = max_mp;  g_max_sp = max_sp;        // 0x84d67c..
    g_prev_exp = prev_exp;  g_next_exp = next_exp;  g_exp = exp;      // 0x84d688, 0x84d68c, 0x84d684
    g_stat_points = stat_pts;  g_skill_points = skill_pts;            // 0x84d6dc, 0x84d6de
    g_dir = dir;  g_pos = {x, y, z};  g_kills = kills;  g_money = money;
    net->char_data_received = 1;                                      // +0x1ec
    if (*guild) g_guild_name = strdup(guild);                         // 0x21e54e4
    Player *me = entities.player(g_my_id);                            // 0x447880 on 0x7051a0
    if (me) {
        me->max_hp = max_hp;  me->max_mp = max_mp;  me->max_sp = max_sp;
        clamp me->hp, me->mp, me->sp to the new maxima;
        if (*guild) me->guild_name = strdup(guild);
    }
}
```

The level, the stats and the character's looks are not in `0x0105`. The looks are copied from the character-select data when the
local entity is built (`0x4c8bf0`, confirmed); level and stats are written by `0x46a4e0` (confirmed), *inferred*: from the chosen
character's entry in the character list (`0x0101`).

**How to find it.** Follow `0x0105` in the dispatcher to its reader `0x579270`, which calls network vtable `+0x28`; for each global,
the reference list shows every handler that changes it (money has about 30 writers).

## 3. Inventory and equipment

### The bag array

`0x84e6f8`: 6 bags × 24 slots × 34 bytes (`0x1320` bytes in all; cleared as one block by `0x46a4e0`). Bag 0 is the equipment, bags 1–5
the inventory; slot address = `0x84e6f8 + bag × 0x330 + slot × 0x22` (confirmed, `0x54cf90`).

| Offset | Type | Name | Meaning | Proven by | Confidence |
|---|---|---|---|---|---|
| `+0x00` | u8 | — | derived from the item record's byte `+0x2b` (file byte 36) minus a correction for each linked lapis whose own `+0x2b` is above 1 | `0x54cf90` | confirmed (computation); *inferred*: free lapis slots |
| `+0x01` | u8 | type | `byType` | `0x54cf90` | confirmed |
| `+0x02` | u8 | typeId | `byTypeID` | same | confirmed |
| `+0x03` | u8 | count | `byCount`; an empty slot has count 0 (the handler skips those) | same | confirmed |
| `+0x04` | u16 | quality | `wQuality` (durability) | same | confirmed |
| `+0x06`–`+0x0b` | u8 × 6 | gems | `byGem[6]` (lapis typeIds) | same | confirmed |
| `+0x0c` | char[21] | craft name | `szCraftName` | same | confirmed |

This is the server's 34-byte item unit with a computed first byte instead of bag and slot. When a slot of bag 0 changes, `0x4c06a0(slot)`
updates the character's look (confirmed in `0x54cf90`).

### Equipment on a player entity

Each player entity also keeps what others can see of its equipment, so other players can be drawn without their inventory:

| Offset | Type | Meaning | Proven by | Confidence |
|---|---|---|---|---|
| `+0x180` | u8[13] | item type per equipment slot | `0x54d370` (`0x0303`), `0x4c8bf0` | confirmed |
| `+0x18d` | u8[13] | item typeId per slot | same | confirmed |
| `+0x19a` | u8[13] | enchant level per slot (*inferred* from the server's `ITEM_TYPE`) | `0x54d370` | confirmed (store) |
| `+0x384`, `+0x388` | u32, u16 | 6 extra bytes sent only when slot 7 is filled | `0x54d370` | confirmed (store); *inferred*: the cape's emblem |

`0x0303` sends 8 slots; the arrays hold 13, filled from the character list for the local player. Slot 5 is the weapon: the client
takes the attack range from the slot-5 item record (`+0x185`/`+0x192` → item `+0x26`) and its weapon class from `0x4c0290(+0x185)`
(confirmed, `0x551650`, `0x54de20`). The other slot names follow the server's order (helmet, upper armour, lower armour, gloves,
shoes, weapon, shield, cape, …; *inferred*).

The warehouse (`0x0711`) is stored by `0x54d130`, once per item, into an array at `0x852c6e` with 34 bytes per slot (`+1` type, `+2` type id, `+3` count, `+4` quality u16, `+6`..`+11` gems, `+12` craft name of 21 bytes; `+0` is a byte the handler computes from the item record `+0x2b` and the gems) and the money into `0x84d674` (**confirmed**, `0x54d130`; the `0x0204` handler `0x557500` and `0x54d240` write the same array). The other storages were not traced here (**open**); their packet layouts are in
[client-world-packets.md](client-world-packets.md).

**How to find it.** The `0x0106` reader `0x5793d0` calls vtable `+0x2c` (`0x54cf90`); the `REP STOSD` that clears `0x4c8` dwords from
`0x84e6f8` gives the array size.

## 4. SData in memory

`0x460500` and its siblings load the tables ([re-engine.md](re-engine.md), section 6, for the file and decryption path). In memory
(confirmed):

| Table | Lookup | Index | Record size | Notes |
|---|---|---|---|---|
| `Item.SData` | `0x460cd0(type, typeId)` | `records[type − 1][typeId − 1]`; types up to the byte at `+0x0c`, counts per type at `+0x08` | `0x5c` | type 0 or typeId 0 → none |
| `Skill.SData` | `0x461ba0(skillId, level)` | `records[skillId − 1][level − 1]`, levels 1–3; ids up to the u16 at `+0x1c` | `0x88` | level 100 is passed as 1 by the callers (`0x56c020`, `0x56be00`) |
| `Monster.SData` | `0x461210(mobId)` | `records[mobId]` from 0; count at `+0x14` | `0x2c` | max HP at `+0x0c` (used by `0x552380`) |

### The item record

The file record is two length-prefixed strings and 81 fixed bytes; the client keeps pointers to the two strings at `+0x00` and `+0x04`
and copies the fixed bytes with some padding (confirmed, `0x460500`):

| File bytes | Memory offset | Shift |
|---|---|---|
| 0–13 | `+0x08`–`+0x15` | +8 |
| 14, 15 | both written to `+0x08` | — |
| 16–32 | `+0x16`–`+0x26` | +6 |
| 33–66 | `+0x28`–`+0x49` | +7 |
| 67–74 (two u32) | `+0x4c`, `+0x50` | +9 |
| 75–80 | `+0x54`–`+0x59` | +9 |

So the rule "memory = file + 8" holds only for the first 14 fixed bytes (type, typeId, level u16 at `+0x0c`, faction, class flags).
File bytes 14 and 15 both overwrite `+0x08` (confirmed (disasm), `0x46091e` and `0x46092d`); in this `Item.SData` both repeat the
item's type, so the stored type is unchanged (checked over all 16,045 records).

Fields seen in use (each confirmed in the function named): `+0x0c` required level (equip check `0x4c1590`, listed in
[client-actions.md](client-actions.md)); `+0x26` attack range of a weapon (`0x551650`); `+0x2a` (file byte 35, the "special" value)
compared with 37 when the local character is built (`0x4c8bf0`); `+0x2b` (file byte 36) for the inventory's first byte (section 3).

### The skill record

Fields seen in use: `+0x10` required level ([client-actions.md](client-actions.md)), `+0x1e` (u16, a type: 6 gives a 30-second buff,
`0x4a`, `0x58` and `0x5a` are tested by the movement and shape code), `+0x36` (u16, recharge time in seconds), `+0x46` (u16, buff
duration in seconds), `+0x64` (compared with 46 for the resurrection rune's messages 837/838), `+0x7f` (1 = the duration is in hours)
(confirmed in `0x56c020`, `0x56c1d0`, `0x56be00`, `0x551650`, `0x4c8bf0`).

Which fields the client never reads was not settled: that needs a scan of every use of the three record pointers (**open**).

**How to find it.** The lookups are the small functions that multiply by `0x5c`, `0x88` and `0x2c`; the item loader is the one that
pushes `item.SData`.

## 5. Entities

Each kind lives in a `std::map<u32 id, Entity *>` inside the manager `0x7051a0`; a lookup returns 0 for an unknown id (confirmed).

| Kind | Map (manager offset) | Lookup | Create (handler, size, constructor) | Add to the scene | Remove |
|---|---|---|---|---|---|
| players | `+0x13d874` | `0x447880` | `0x0201` → `0x551f60`, `0x3dc` bytes, `0x415970`; the local one `0x4c8bf0` | `0x44e830` | `0x0202` → `0x54d330` → `0x44e650` |
| ground items | `+0x13d888` | `0x4478c0` | `0x0401` → `0x55acb0`, `0xac` bytes, `0x5571c0` (or `0x429970` for type 44 / typeId 246) | `0x44e8f0` | `0x0402` → `0x557690` |
| monsters | `+0x13d89c` | `0x447900` | `0x0601` → `0x552380`, `0x124` bytes, `0x415670` | `0x44e990` | `0x0602` → `0x54e7e0` → `0x44e710` |
| NPCs | `+0x13d8b0` | `0x4479a0` | `0x0E01` → `0x552730`, `0x12c` bytes, `0x487fe0` | `0x44ea50` | `0x0E02` → `0x54f790` |

An "enter" for an id that is already known updates the entity (players) or is ignored (monsters, NPCs, items), as
[client-world-packets.md](client-world-packets.md) says from the packet side.

### Player entity (`0x3dc` bytes)

| Offset | Type | Name | Meaning | Proven by | Confidence |
|---|---|---|---|---|---|
| `+0x10`–`+0x18` | f32 × 3 | position | x, y, z | `0x551f60`, `0x52fe20` (`Player x:%3.3f …`) | confirmed |
| `+0x1c`–`+0x24` | f32 × 3 | facing | unit vector from the direction (0 faces +z) | `0x551f60` | confirmed |
| `+0x34` | u32 | id | the server's character id | `0x551f60`, `0x54ca50` | confirmed |
| `+0xdc` | u32 | action | 2 while attacking; 8 is reset to 1 by a move | `0x54de20`, `0x551650` | confirmed (values); *inferred*: action state |
| `+0xf0` | char[21] | name | | `0x54d370`, `0x4f3f80` | confirmed |
| `+0x130`, `+0x134` | i32 | HP, max HP | | `0x54cdc0`, `0x54e2c0`, `0x54fd50` | confirmed |
| `+0x138`, `+0x13c` | i32 | MP, max MP | | same | confirmed |
| `+0x140`, `+0x144` | i32 | SP, max SP | | same, `0x54efc0` | confirmed |
| `+0x148`–`+0x150` | f32 × 3 | destination | where `0x0501` says the player is going | `0x551650` | confirmed |
| `+0x154`–`+0x15c` | f32 × 3 | destination facing | | `0x551650` | confirmed |
| `+0x168` | u32 | move motion | `0x0501`'s motion when below 3 | `0x551650` | confirmed (store); *inferred*: walk/run |
| `+0x174`, `+0x175`, `+0x176` | u8 | hair, face, size | `byHair`, `byFace`, `bySize` | `0x54d370` | confirmed |
| `+0x177`, `+0x178` | u8 | sex, class | `bySex`, `byJob`; the status window prints the class name from `+0x178` | `0x54d370`, `0x4f3f80` | confirmed |
| `+0x17c` | u32 | shape state | 2 once the looks are known; 0 makes the client ask for them with `0x0303` | `0x551f60` → `0x585920`, `0x54d370` | confirmed |
| `+0x180`–`+0x1a6` | | equipment | section 3 | `0x54d370` | confirmed |
| `+0x1a8`–`+0x1c9` | | pending hit | section 7 | `0x54de20`, `0x416cd0` | confirmed |
| `+0x274`, `+0x275` | u8 | attack speed, move speed | from `0x051C`; the move speed picks the walk/run speed table | `0x54f260` | confirmed |
| `+0x276`, `+0x277`, `+0x278` | u8 | faction, race, mode | `byCountry`, `byFamily`, `byGrow` | `0x54d370`, `0x4c8bf0` | confirmed |
| `+0x279` | u8 | death state | 2 when `0x0303` says dead (HP also set to 0) | `0x54d370` | confirmed |
| `+0x28c` | u8 | staff | 1 when `0x0201`'s kind byte is 2 | `0x551f60` | confirmed |
| `+0x298` | u8 | moving | set by `0x0501`, cleared on arrival and by motion 4 | `0x551650` | *inferred* |
| `+0x2c0` | u8 | party type | `byParty` of `0x0303` | `0x54d370` | confirmed (store) |
| `+0x2c4` | char * | guild name | | `0x54cdc0`, `0x54d370` | confirmed |
| `+0x2c8` | u32 | guild id | from `0x0201` | `0x551f60` | confirmed |
| `+0x2e4`, `+0x2e8` | u32 | chase target kind, id | 1 player, 2 monster; a moving player is pulled to within weapon range of it | `0x551650`, `0x54de20` | confirmed |
| `+0x2ec` | u32 | kill count | `dwKillCount` of `0x0303` | `0x54d370` | confirmed |
| `+0x378` | u8 | mount | 2 = riding behind someone | `0x551f60` | confirmed |
| `+0x3cc` | u32 | driver | the id of the player whose mount this one rides | `0x551f60` | confirmed (server name) |

The speeds the move-speed index selects (walk and run per index, mounted overrides) are in
[research-movement-storages-auction.md](research-movement-storages-auction.md); they come from the local mover `0x43c000` and the player updater `0x418c00`.

### Monster entity (`0x124` bytes)

| Offset | Type | Meaning | Proven by | Confidence |
|---|---|---|---|---|
| `+0x04`–`+0x0c` | f32 × 3 | position (y from the ground height) | `0x552380`, `0x430fe0` (`/mobinfo`) | confirmed |
| `+0x10`–`+0x18` | f32 × 3 | facing (random at creation) | same | confirmed |
| `+0x28` | u32 | world id (the server's id) | `0x552380`, `0x430fe0` | confirmed |
| `+0x2c` | u32 | monster id (`Monster.SData` index) | `0x430fe0` | confirmed |
| `+0x4c` | u32 | action | `0x430fe0` | confirmed |
| `+0x54`, `+0x58` | u32 | max HP, HP | `0x552380` (max from `Monster.SData +0x0c`), `0x54eff0` (`0x0608`), `0x430fe0` | confirmed |
| `+0x5c`–`+0x64` | f32 × 3 | predicted position (where it is walking) | `0x430fe0` | confirmed |
| `+0x68`–`+0x70` | f32 × 3 | predicted facing | same | confirmed |
| `+0x88` | f32 | size used in range checks | `0x551650` | *inferred* |
| `+0x98`, `+0x9c`, `+0xa0`–`+0xa4` | u8, u32, u16 × 3 | pending blow: flag, target player, damage HP/SP/MP | `0x431200` | confirmed |
| `+0xb4`, `+0xb5` | u8 | attack speed, move speed | `0x54f2b0` (`0x0609`) | confirmed |

`/mobinfo` (`0x430fe0`) prints exactly these fields: id, world id, HP, max HP, position, direction, predicted position and direction,
the player it targets and its action ([gm-and-commands.md](gm-and-commands.md)).

### NPC (`0x12c` bytes) and ground item (`0xac` bytes)

| Entity | Offset | Meaning | Proven by |
|---|---|---|---|
| NPC | `+0x38`–`+0x40` | position | `0x552730` |
| | `+0x44`–`+0x4c` | facing | same |
| | `+0x5c` | id | same |
| | `+0x60`, `+0x62` | type (u8), typeId (u16): the `NpcQuest.SData` record | same |
| | `+0x8c`–`+0xa0` | destination and its facing (`0x0E03`) | same |
| ground item | `+0x04` | id | `0x55acb0` |
| | `+0x20`–`+0x28` | position | same |
| | `+0x4c`, `+0x4d` | type, typeId | same |
| | `+0x50` | drop height for the falling animation (0, 0.1, 1.0, 2.0 by `byKind`) | same |

All confirmed in the functions named. The ground item's count is not stored: `0x55acb0` takes eight arguments (id, kind, type, typeId, the fifth is the count, position x, y, z) and never reads the fifth; the constructor `0x5571c0` takes none (confirmed, re-checked). The ground-item entity therefore has no count field to show.

**How to find it.** `operator new` with these sizes in the four enter handlers; the `/mobinfo` format strings label the monster fields.

## 6. Movement and prediction

- **Other players** (`0x0501` → `0x551650`): the packet's position becomes the **destination** (`+0x148`), its direction the destination
  facing; the entity then walks there over the next frames (the player updater `0x418c00`, documented with the speed tables in
  [research-movement-storages-auction.md](research-movement-storages-auction.md)). Motion 3 starts an animation (`+0xcc` = 18), motion 4 stops the walk. The local player's own
  `0x0501` echo is ignored unless a buff of skill type `0x4a` is active, which lets the server move the local player (confirmed).
- **Chasing**: when a moving player has a chase target (`+0x2e4`), the destination is pulled in so that it stops at weapon range
  (the weapon's record `+0x26`, or 1.0 without a weapon) from the target, and its height is taken from the ground (`0x40ea00`) (confirmed).
- **Monsters** (`0x0603`) and **NPCs** (`0x0E03`) work the same way with their own destination fields.
- **The local player** moves itself (`0x43c000`) and sends `0x0501`; the server's corrections arrive as warp packets
  ([client-world-packets.md](client-world-packets.md)).

## 7. Targeting and combat results

Attack and skill results are **deferred**. For a player hitting a monster (`0x0503` → `0x54de20`, confirmed):

1. If the attacker is the local player, the client re-checks line of sight to the monster (`0x437fe0`) and may correct its own
   position (`0x43a460`); otherwise it sets the attacker's chase target to the monster.
2. It starts the attack animation (`+0xdc` = 2, a swing chosen from the weapon class).
3. It stores the **pending hit** on the attacker:

| Offset (player) | Type | Meaning |
|---|---|---|
| `+0x1a8` | u32 | time of the attack (`timeGetTime`) |
| `+0x1b5` | u8 | pending kind: 1 damage, 2 heal |
| `+0x1b6` | u8 | target kind (0 for a monster in this handler) |
| `+0x1b8` | u32 | target id |
| `+0x1bc`, `+0x1be`, `+0x1c0` | u16 | damage to HP, SP, MP |
| `+0x1c2`, `+0x1c4`, `+0x1c6` | u16 | healing to HP, SP, MP |
| `+0x1c8` | u8 | result (`bySuccess`) |
| `+0x1c9` | u8 | level-up waiting for this hit (`0x0508`) |

4. `0x416cd0` applies the pending hit when the blow lands (and first applies any older pending hit, so two quick results never get
   lost): it changes the target's HP/SP/MP, prints the messages (for example 1103 for a heal on the local player), and spawns the
   floating numbers through `0x450870(position, …, value, …, kind, …)` (confirmed for the heal branch shown; the damage branch has the
   same shape).

Monsters keep their own pending blow on the monster (`+0x98`–`+0xa4`), applied to the target player by `0x431200`. The target bar
draws the monster's `+0x54`/`+0x58`, which is why `0x0608` must follow `0x0601`. The local "attacking" flag is `0x21e493c` (cleared
in `0x54de20`).

Which entity the local player has selected is held by the action code in `0x43c000` (target kind 1 ground item, 2 player, 3 monster,
4 NPC, per [client-world-packets.md](client-world-packets.md)); its storage was not located in this pass (**open**).

## 8. Skills and buffs

### The skill list (`0x0108` → `0x56c1d0`)

A `std::vector<Skill *>` at `0x21e47dc` (begin) / `0x21e47e0` (end), one 20-byte entry per learnt skill (confirmed):

| Offset | Type | Meaning |
|---|---|---|
| `+0x00` | u8 | the skill's number (`byNumber`, used when the client sends a skill) |
| `+0x02` | u16 | skill id |
| `+0x04` | u8 | level |
| `+0x08` | u32 | recharge time in ms (`Skill.SData +0x36` × 1000) |
| `+0x0c` | u32 | `GetTickCount` when the entry was set |
| `+0x10` | u32 | the packet's per-skill value × 1000 (*inferred*: remaining recharge) |

Skills 3, 4 and 5 also set the bytes `0x21e4e1d`–`0x21e4e1f` (*inferred*: passive flags).

### Buffs (`0x050D` → `0x56c020`, `0x050E` → `0x56be00`)

A `std::map<u32 key, Buff *>` at `0x21e4820` (head `0x21e4824`), keyed by the server's buff key (confirmed):

| Offset | Type | Meaning |
|---|---|---|
| `+0x00` | u32 | key |
| `+0x04` | u16 | skill id |
| `+0x06` | u8 | level (100 means level 1 for the lookup) |
| `+0x08` | u32 | duration in ms: `Skill.SData +0x46` × 1000; × 3600 when `+0x7f` = 1; 30,000 for skill type 6 |
| `+0x0c` | u32 | `GetTickCount` at the start |
| `+0x10` | u32 | the duration again (*inferred*: the original, for the icon's countdown) |

Adding a buff also adds its icon (`0x4c0760`) and refreshes the buff bar (`0x4c5470`); removing it removes the icon (`0x4c0e50`). A
second `0x050D` with the same key restarts the timer. Other players' and party members' buffs are separate (the party member's list,
section 9).

## 9. Party and guild

### Party (`0x0B01` → `0x55ffc0`)

A `std::vector<Member *>` at `0x21e4838` (begin) / `0x21e483c` (end) / `0x21e4840` (capacity); "in a party" is `0x21e4924` = 1. The
handler adds the listed members, then the local player as the last entry (confirmed). Member record (`0x50` bytes):

| Offset | Type | Meaning | Proven by |
|---|---|---|---|
| `+0x00` | u32 | character id | `0x55ffc0` |
| `+0x04` | char[21] | name | same |
| `+0x1a` | u16 | (unit `+0x15`) *inferred*: level | same |
| `+0x1c` | u8 | (unit `+0x17`) *inferred*: class | same |
| `+0x1e`, `+0x20` | u16 | max HP, HP | `0x55ffc0`, `0x55ee30` (`0x0C01` type 0) |
| `+0x22`, `+0x24` | u16 | max SP, SP | same (type 1) |
| `+0x26`, `+0x28` | u16 | max MP, MP | same (type 2) |
| `+0x2a` | u16 | (unit `+0x24`) | `0x55ffc0` |
| `+0x2c`, `+0x30`, `+0x34` | u32 | (unit `+0x26`, `+0x2a`, `+0x2e`) *inferred*: position | same |
| `+0x40` | vector | the member's buffs: `u16 skill, u8 level, u32` each | same |

So the party window shows HP, SP and MP in that order, matching `0x0C01`'s type numbers (HP 0, SP 1, MP 2;
[client-social-packets.md](client-social-packets.md)). Union (raid) parties use the same functions with a second lookup
(`0x4c88b0`).

### Guild

On every player entity: guild name `+0x2c4` and guild id `+0x2c8`; for the local character also `0x21e54e4` (confirmed). The guild
member list and ranks were not traced in this pass (**open**; the packets are in [client-social-packets.md](client-social-packets.md)).

## Open

| Item | What is known | What is missing |
|---|---|---|
| Item and skill fields the client ignores | the fields listed in section 4 are read | a full scan of record uses |
| Local selected target | partly: the kind is the global `0x84fa18` (0 none, 1–3 set by the click code in `0x43c000`, 2 also by the UI functions `0x502750`, `0x4fa7d0`, `0x5034e0`, `0x503f90`), the id is the local entity's `+0x160` (kind copy at `+0x164`); chase target `+0x2e4`/`+0x2e8` on the entity | what kinds 1 and 3 stand for on screen |
| Warehouse and other storages | warehouse settled: array `0x852c6e`, 34 bytes per slot, written by `0x54d130` (see above); packet layouts known for the others | the other storages' arrays; the slot count of the warehouse array |
| Guild member list | name and id on the entity | the list, ranks, guild points |
| Stat bonuses `0x84d6a4`–`0x84d6b8` | read by the status window; no instruction writes these six absolute addresses (reference search); the loop at `0x46b7e0` zeroes `0x84d6bc`–`0x84d6d4`, not these | the writer, which must reach them through the object at `0x84d668` (offsets `+0x3c`–`+0x50`) |
| Equipment slot names beyond 5 (weapon) and 7 | server order | a client-side proof for each slot |
| Ground item count | sent in `0x0401` | settled: dropped by `0x55acb0` (see [re-open-items.md](re-open-items.md)) |
| Party member fields `+0x1a`, `+0x1c`, `+0x2a`–`+0x34` | copied from the unit | their meaning on screen |
