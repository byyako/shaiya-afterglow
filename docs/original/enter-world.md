# Entering the world

This page covers what happens between picking a character and standing in the world: the server's character select, the burst of packets
that describes the character to the client, the switch from AES to XOR encryption, and entering the zone. It covers both sides: the EP4.5
game server `ps_game.exe` (built 2009-07-15) with the database agent `ps_dbAgent.exe` (from `PSM_Client/Bin`), and the US client
`game.exe` (built 2010-04-26, patch 108, SHA-256 `5f045bcb…01a6`, no PDB).

Everything here comes from static analysis; nothing was executed. Status words: **confirmed** = read in the code, **PDB** = from the
type records of the server's debug-symbol file, **inferred** = follows from confirmed code but is not shown directly, **unknown** = not
read. Server addresses are in `ps_game.exe`, client addresses in `game.exe`. Lengths include the 2-byte opcode but not the 2-byte length
prefix.

## In short

- **Picking a character starts a burst.** When the player clicks "start", the server sends a fixed sequence of packets: the character's
  stats and experience, the inventory, the bank, skills, active buffs, quests, friends, save points, honor, cash points, the server time
  and finally the quick-slot bar. Packets for things the character does not have are simply left out.
- **The quick-slot packet is the switch.** The last packet of the burst (`0x010B`, quick slots) is the last one encrypted with AES. Both
  the server and the client switch what the server sends to the XOR table right after it.
- **Then the client loads the map.** The player waits on the loading screen. When the client has finished, it sends `0x0201`; the server
  checks the position, places the character in the world, and sends what is around it: players, monsters, NPCs, items, weather.
- **Light and Fury races.** The client only offers human and elf for Light (country 0) and vail and deatheater for Fury (country 1). The
  original server does not check this itself.

A note on server offsets: in `ps_game.exe`, `CUser` fields from `stUserInfo+0x1c` onwards sit **4 bytes later** than the PDB type record
says (for example the code reads max HP at `+0x17c`, the PDB has `m_wMaxHP` at `+0x178`), and parts of the `CUser` tail are shifted the
same way. Field names below are matched to the code by this +4 rule and by how the code uses them; the offsets are the code's.

## 1. The client side (game.exe, verified)

### 1.1 Cipher state

| address | what it does |
|---|---|
| `0x401000` | AES key setup for the game connection: key schedule for both directions, both counters from `key[16..32]`, positions 0, then **enc and dec on** |
| `0x4010a0` | encrypt (client to server): only if enc is on; AES-CTR in mode 0. **The client never XORs what it sends** |
| `0x4010e0` | decrypt (server to client): if the XOR flag is on and mode is 2, `byte[j] ^= table[n + j]` (n = length after the length field, the same as the server, `docs/original/ps_game.md` 5.2); otherwise AES if dec is on |
| `0x401160` | all three flags off |
| `0x4013c0` | enc and dec on (AES) |
| `0x4013d0` | the counter step: both AES counters = **SHA-256(stored login IV)[0..16]**, positions 0, key kept; then the stored IV is wiped |
| `0x401520` | builds the 4096-byte XOR table from the 16-byte seed (`block[0] = SHA-256(seed)`, `block[i] = SHA-256(last 16 bytes of block[i-1])`), stores the mode; mode 0 turns dec off |
| `0x401570` | XOR on (for mode 2, also for modes < 0 and > 3); mode 0 or 1 turns dec off instead |
| `0x4015a0` | XOR off; dec on unless mode is 0 |

### 1.2 Order on the game connection

1. After server select (`0x4dcdd5`): close the login connection, run `0x4013d0` (counters = SHA-256(login IV)[0..16]), connect to port 30810.
2. On connect (`0x4dce5d`): all flags off (`0x401160`), **send `0xA301` in plain text** (`0x586910`, 22 bytes), then enc and dec on (`0x4013c0`).
   This settles the open item in `docs/original/ps_game.md` section 2: the handshake request is plain text, and from the next packet on both
   directions are AES with fresh counters.
3. The `0xA301` reply handler (`0x55ce50`) builds the XOR table from `byEncKey` and `byEncMode` when `bySuccess == 0`.
4. Packets are decrypted **one at a time, each just before it is dispatched** (receive loop `0x585390`: decrypt `0x4010e0`, dispatch `0x582e10`).
5. **The handler of `0x010B` (`0x57d070`, quick slots) turns XOR on first thing.** `0x010B` itself was decrypted with AES; every packet after
   it is XOR. This matches the server, which sets `m_bInitGame` right after queuing `0x010B` (section 3). So the switch point is exactly
   "after `0x010B`", on both sides.
6. **The handler of `0x0107` (`0x582f43`) turns XOR off** and AES decrypt back on (return to the character screen; the server's
   `CWorld::LogoutUser` does the same on its side).

### 1.3 Other client facts

- **Create request length.** `0x5857c0` builds `0x0102` as 10 header bytes, then `lstrcpyA` of the name, and sends `lstrlenA(name) + 11` bytes:
  the name field is **not** 21 bytes on the wire but the name plus its terminating zero. (The server's parser already stops at the zero.)
- **Factions and races.** The faction screen shows `choose_submit_light.tga` for country 0 and `choose_submit_fury.tga` for country 1
  (`0x47be67`). The create screen's two race buttons (`0x4656c0`, `0x465c60`) set race 0 (human) and race 1 (elf) when the country is 0,
  race 3 (deatheater) and race 2 (vail) otherwise. So **Light = 0 = races 0 and 1, Fury = 1 = races 2 and 3.**
- **`0x0201`.** The client sends `0x0201` (2 bytes, no payload, `0x5858f0`) at the end of its world-loading step (two call sites in the scene
  update code, `0x42e9fa` and `0x42fa77`). What starts the loading (which received packet) was not traced.
- **`0xB106`.** The client sends `0xB106` (2 bytes, `0x589570`) at the end of a scene set-up function (`0x426735`), when a feature check
  (`[0x7013d4]->vtable[1](10)`) passes. The original server has no handler for it: in states 2 and 3 it is ignored, in state 4 any `0xB1xx` other
  than `0xB102` would close the connection. It arrives while the player is still waiting (state 2), so it is harmless there.

### 1.4 Checked with the US client

Login, server select, the plain-text `0xA301`, the character screen and select all worked. After the burst the client sent `0xB106` (about 5 s
later, scene start), then `0x0201` (about 13 s after the burst, map loaded); the server's XORed `0x0201` reply was accepted and the client stayed
connected and sent `0x2605` (cash point family). This confirms the burst, the `0x010B` XOR switch and the `TP_USER_ENTER` layout
against the real client.

## 2. Selecting a character, server side

1. Client `0x0104` (`u32 charId`) in state 1. Checks in `docs/original/ps_game.md` section 6.2 (list received, faction chosen). The server asks
   the database agent to select the character; its answer `0x408` makes the server call `CUser::SendDBAgentCharGetInfo` (0x46c980), which
   loads the character data (items, skills, quests, ...). The final `0x407` from the database agent runs the enter-world burst (section 3).
   The database-agent messages in between are not needed for the new server (one database, read directly) and were not written up.
2. Case `0x407` (verified, 0x46d4c1) first tells the session server `0x803` (char id), then `InitEquipment`, `InitPassive`, `InitApplySkill`,
   `BillingApply`, clamps current HP, MP and SP to their maxima, and sends the burst.

## 3. The enter-world burst

Confirmed, `CUser::PacketUserDBChar` case `0x407`.

All of these are queued in this order, **AES-encrypted**. Lengths include the opcode, not the length prefix. "Always" means sent even when empty.

| # | opcode | sent | length | content |
|---|---|---|---|---|
| 1 | `0x0104` | always | 7 | select result: `u8 0`, `u32 charId` |
| 2 | `0x0404` | always | 6 | `u32` world clock: a start value plus seconds since a start tick (`CWorld::GetWorldDay`). The start value comes from the database agent when the game server connects (`CClientToDBAgent::PacketSystemConnect`), so it is a clock kept across restarts. Meaning to the client unknown |
| 2a | `0x0D0D`, `0x0D2F`, `0x0D0E` | always | | the guild list of the character's country (`CUser::EnterWorld` → `CGuildMgr::SendGuildList`, `docs/original/guild.md` section 9) |
| 2b | `0x0D12`, `0x0D13`, `0x0D16` | in a guild | | its members, and to an officer the join requests (`CUser::InitGuild`, section 10 there) |
| 3 | `0x0112` | reconnect only (`m_dwAutoCharID != 0`) | 8 | `u32 objectId, u16 map`. Not used by us |
| 4 | `0x0105` | always | 59 + guild name length | `TP_CHAR_DATA`, section 3.1 |
| 5 | `0xB101` | if any | 3 + 4n | billing (premium) items: `u8 count`, then `u8 index, u8, u8, u8` per item (`SendCharacterBillingItem`). Not used by us yet |
| 6 | `0x0106` | **only if the character has items** | 3 + 34n | `TP_CHAR_ITEM`, section 3.2; at most 50 items per packet, more packets follow |
| 7 | `0x0711` | **only if the bank has items** | 7 + 33n | `TP_CHAR_BANK`: `u32 bankMoney, u8 count`, `TP_BANK_UNIT` × n (33 bytes: slot, type, typeId, `u16 quality`, gems[6], count, craftName[21]); 50 per packet. Note: an empty bank sends nothing, so the bank money is not sent either |
| 8 | `0x0108` | always | 3 + 8n | `TP_CHAR_SKILL`: `u8 count`, `TP_SKILL_UNIT` × n (`u16 skillId, u8 level, u8 number, u32 delay`, delay = remaining cooldown in seconds, at least 0) |
| 9 | `0x010A` | if any | 3 + 11n | `TP_CHAR_SETSKILL` (active buffs): `u32 id, u16 skillId, u8 level, u32 delay`; 128 per packet |
| 10 | `0x0901` | if any | 3 + 7n | quests in progress (`TP_QUEST_LIST`); 255 per packet |
| 11 | `0x0906` | if any | 3 + 3n | finished quests (`TP_QUEST_FINISH_LIST`): `u16 questId, u8`; 255 per packet |
| 12 | `0x2201`, `0x2208` | if any | variable | friends and block list, 25 per packet (`SendCharacterFriend`) |
| 13 | `0x051C` | always | 8 | speed: `u32 objectId, u8 attackSpeed, u8 moveSpeed` (section 3.3) |
| 14 | `0x0211` | always | 11 | `u8 country, u32 value, u32 remainingMs`: the faction's kill count and the time left of its top grade (`SendKCStatus`, `docs/original/pvp-2.md` section 9) |
| 15 | `0x0225` | if any save point | 3 + 15n | save points: `u8 count`, then `u8 index (1 to 4), u16 map, f32 x, y, z` |
| 16 | `0x0230` | always | 4 | `u16` honor (`m_wHG`) |
| 17 | `0x2601` | always | 7 + 29n | `TP_POINT_AND_LOG`: `u32 cashPoint, u8 count`, `TP_POINTLOG_UNIT` × n (`char productCode[21], u32 buyTime, u32 usePoint`), at most 10 |
| 18 | `0x2606` | if any | 3 + 4n | stored point (cash shop) items |
| 19 | guild | if in a guild | | `CGuild::SendBankItemList`, `CGuild::SendNpcLvList` (guild house: `docs/original/guild-house.md` section 1, `docs/original/guild-bank.md` section 4) |
| 20 | `0x0114` | always | 6 | `TP_CHARACTER_SERVERTIME`: the local time packed by the server's `GetSystemTime` (0x4cb8e0): `(year - 2000) << 26 \| month << 22 \| day << 17 \| hour << 12 \| minute << 6 \| second` (computed as `year - 16` with overflow, the same low bits) |
| 21 | `0x021B` | always | 4 | two bytes: `-byte(+0x584c)` and `1 - byte(+0x584d)` (inferred to be `m_byResetStatus`, `m_byResetSkill`: the stat and skill reset flags). For a normal character: `00 01` |
| 22 | `0x0210` | only in shape type 13 | 3 | run mode. Sent here on entering the zone while hidden (`Zones::enter`), not in the burst |
| 23 | `0x010B` | always (normal path) | 3 + 5n | `TP_CHAR_QUICK`: `u8 count`, then `u8 bar, u8 slot, u8 bag, u16 number` per quick slot. On the reconnect path `0x0113` (2 bytes) is sent instead |

Then `m_bInitGame = 1` (XOR on for everything sent after this point) and `SendNow`.

### 3.1 `0x0105` `TP_CHAR_DATA`

Confirmed, `CUser::SendCharacterData` 0x483140, PDB.

| offset | field | source |
|---|---|---|
| 2 | `u16 wStatPoint` | |
| 4 | `u16 wSkillPoint` | |
| 6 | `u16 wMaxHP`, 8 `wMaxMP`, 10 `wMaxSP` | the computed maxima (ints in the server, truncated to u16) |
| 12 | `u16 wDir` | |
| 14 | `u32 dwPrevExp` | experience total needed for the current level (below) |
| 18 | `u32 dwNextExp` | experience total needed for the next level |
| 22 | `u32 dwCurrExp` | the character's experience total |
| 26 | `u32 dwMoney` | |
| 30 | `f32 fPosX, fPosY, fPosZ` | position |
| 42 | `u32 dwKillCount[4]` | the four kill counters (`Chars.K1` to `K4`) |
| 58 | `char szGuildName[]` | guild name and its zero; the packet ends there (an empty name gives 59 bytes) |

Experience (verified, `CGameData::LoadExp` 0x4bfe00): `EXEC usp_Read_ExpDefs_R` gives (level, grow 1 to 4, exp), the new server's `defs.exp_defs`
(grow 1 to 4 = modes 0 to 3, levels 1 to 60). At load the server turns each mode's column into running totals: `T[mode][L] = exp[1] + ... + exp[L]`,
`T[mode][0] = 0`. Then `dwPrevExp = T[mode][level - 1]` and `dwNextExp = T[mode][level]`. Out of range (mode > 3 or level > 60 for next, > 61 for prev)
gives a fixed fallback value, `T[easy][60]`.
Example: a level 1 character in normal mode (grow 1 = defs grow 2) gets prev 0, next 100.

### 3.2 `0x0106` `TP_CHAR_ITEM`

Confirmed, 0x4832b0, PDB.

`u8 count`, then per item 34 bytes (`TP_ITEM_UNIT`): `u8 bag, u8 slot, u8 type, u8 typeId, u16 quality, u8 gems[6], u8 count, char craftName[21]`.
Bags 0 to `m_nMaxBag` (default 2, set by `InitEquipment`), slots 0 to 23, in that order; empty slots are skipped. Bag 0 is the equipment.
The craft name is copied with `strcpy` (unbounded) from the item record.

### 3.3 `0x051C` speed

Confirmed, `CUser::SendSpeedMe` 0x482730, `GetMoveSpeed` 0x48a4d0.

- `attackSpeed`: `m_nAttackSpeed` clamped to 0 to 9; `0xFF` while stunned, asleep, transformed or disabled by a skill.
- `moveSpeed`: `m_nMoveSpeed` clamped to 0 to 4; `0xFF` in the same states (0 when transformed with a fixed mob shape).
- `InitEquipment` (0x454560) starts both from fixed values (move speed **2**, attack speed 0) and adds the equipped items' values
  (`Items.Speed`-like fields of the weapon and a per-weapon-type table). The exact attack speed formula was not written up (unknown).

## 4. After the burst: waiting, then the zone

Confirmed.

1. Still in case `0x407`, `CWorld::WaitUser` (0x40ce70) moves the player from state 1 to **state 2** (waiting) and calls
   `CWorld::EnterUserWorld` → `CUser::EnterWorld` (0x44a820), which only talks to the database agent and log servers (nothing to the client).
2. The world's update loop (`CWorld::UpdatePackets`, 0x40c070) takes each waiting player, finds the zone for its map (`GetZoneByMapID`), adds it to
   that zone's entry queue and sets **state 3**. If the map has no zone it logs, sets state 1 and closes the connection (reason 9).
3. In state 3 the client's **`0x0201`** (sent after its loading, section 1.3) calls `CUser::ZoneEnterUserMove` (0x44b8a0) → `CZone::EnterUser`
   (0x415f80). That checks the position (`CMap::IsSafePos`, inside the map's cell grid; otherwise log and close, reason 9), places the player in its
   cell, sets the map id from the zone, and sets **state 4** (in the world). It then sends, to the player:
   - `0x0201` `TP_USER_ENTER` (29 bytes) through `CZone::SendView`, i.e. to everyone in view including the player itself:
     `u32 charId, u8 byNew, u16 dir, f32 x, y, z, u32 guildId, u32 vehicle`. `byNew` is 1 for players (permission above 10) and 2 for staff;
     `vehicle` is, for a player riding behind someone, its driver's id, otherwise 0 (`docs/original/vehicles.md` section 4);
   - `SendShape` (if the player has a shape), `SendSpeed`, `CZone::SendCellInfoView` (the players, mobs, NPCs and items around),
     `CZone::SendWeather`, `CMap::SendObeliskList`, then `CUser::EnterZone`. These are written up in `docs/original/world.md` section 5.
4. In state 2 everything from the client is ignored; in state 3 only `0x0201`, chat (`0x11xx`, `0xF1xx`) and `0x31xx` are handled.

So, from the client's point of view: `0x0104` … `0x010B` (AES) → XOR from here on → client loads the map → client `0x0201` → server `0x0201` (XOR) and
the world around it.

## 5. Items

Confirmed, unless marked.

- **Storage in the original:** `PS_GameData.CharItems` (`CharID, ItemID, ItemUID (bigint), Type, TypeID, Bag, Slot, Quality, Gem1-6, Craftname varchar(20),
  Count, Maketime, Maketype`). `ItemID` = `Type * 1000 + TypeID` (true for every base item, `defs.items` is keyed the same way).
- **Equipment shown on the character screen** (ps_dbAgent `DBCharacter::LoadCharacterList`, verified 0x41e200): `EXEC usp_Read_Char_Items_Simple_R charId, ?, 0`
  returns (slot, type, typeId, gems…) of the equipped items; for slot < 8 the list's `byEquipmentType[slot]` and `byEquipmentTypeID[slot]` are set, and for
  **slot 7 the six gem bytes become `byCloakInfo[6]`** (the cloak's look). That the procedure returns bag 0 only is inferred (its text was not read).
  Slot 5 is the weapon (base items, below) and 7 the cloak (above); the other slot meanings (0 helmet, 1 upper, 2 lower, 3 gloves, 4 boots, 6 shield)
  are community knowledge, not verified here.
- **Base items for a new character** (`CGameData::LoadBaseItem` 0x4bf800, `ProcessCharacterCreate`): rows of `usp_Read_BaseItemDefs_R` read as
  (family, job, itemId, type, typeId, count, slot, quality), skipped unless the item exists in the item definitions. **The `Slot` and `Quality` columns
  are not used.** Placement is done after loading, per (race, class), in row order: the **first item with type 1 to 19 (a weapon) goes to bag 0 slot 5**
  (equipped); every other item goes to **bag 1, slots 0, 1, 2, …** in order. Quality is taken from the item definition (`defs.items.quality`). Each gets a
  new unique id and the creation time. In `defs.base_items_defs` every combination has two rows: the weapon, and potions (count 10), a shield (type 19,
  human defender) or other gear (type 34, deatheater defender), which therefore go to bag 1 slot 0.
- **Unique ids** (`ProcessCharacterCreate` 0x4817c0): built from a time value, a running counter and the server id (`g_ServerInfo`). The new server uses a database
  sequence instead; the client only ever sees bag and slot.

## 6. The new server

### 6.1 Storage

`character_items`:
`character_id`, `bag` (0 equipment, 1 to 5 inventory), `slot` (0 to 23), `item_id` (`defs.items.itemid`), `type`, `type_id`, `quality`, `gems` (6 bytes),
`count`, `craft_name` (up to 20 bytes), `made_at`, `make_type`, `uid` (bigint from a sequence); unique `(character_id, bag, slot)`.
The bank (account-wide in the original, `UserStoredItems`) is `bank_items` (`docs/original/warehouse.md` section 8); an empty bank
sends nothing, as in the original.

### 6.2 Character screen changes

- Create: insert the base items in the same transaction as the character, placed by the rule in section 5.
- List: fill `byEquipmentType/TypeID[0..8]` from bag 0 slots 0 to 7 and `byCloakInfo` from slot 7's gems.

### 6.3 Select and the burst

On `0x0104` with a live character of this account on this server, load the character and its items and send, AES-encrypted, in this order (refusing
a character whose deletion is pending is an addition of the new server; whether the original refuses it was not checked):
`0x0104` (0, id), `0x0404` (seconds counter), `0x0105`, `0x0106` (only if items), `0x0108` (count 0), `0x051C` (attack speed from the weapon when known,
otherwise 0; move speed 2), `0x0211` (country, 0, 0), `0x0230` (honor), `0x2601` (0 points, no log), `0x0114` (time), `0x021B` (`00 01`), `0x010B` (count 0).
Then switch the send side to XOR. Bank, friends and save points are sent when not empty (`docs/original/warehouse.md` section 8,
`docs/original/friends.md` section 6, `docs/original/item-windows.md` section 4); the billing storage (`0xB101`), the points with the log (`0x2601`) and the
item-mall storage (`0x2606`) too (`docs/original/item-mall.md` section 8). Buffs (`0x010A`), quests (`0x0901`, `0x0906`, `docs/original/quest.md` section 10) and the guild
packets are sent where the original sends them.

### 6.4 Zone entry

State "waiting" after the burst. On the client's `0x0201`: send `0x0201` `TP_USER_ENTER` for the player itself (XOR) and mark the player as in the
world. The position is checked against the map (`World::is_safe_pos`: off the map closes the connection; since audit R21 a height outside -10,000 to 10,000 or one that is not a number is off the map too). The world clock (`0x0404`)
starts at 0 when the server starts. Players around, mobs, NPCs, items, weather and the obelisk list are sent as `docs/original/world.md`
section 5.2 (`Zones::enter`).

### 6.5 Faction and race

Create refuses a race outside the account's faction (Light 0: races 0 and 1; Fury 1: races 2 and 3). The original server did not check this; the client
offers only these races (section 1.3).

## 7. Open items

- The zone-entry senders (`SendShape`, `SendSpeed`, `CZone::SendCellInfoView`, `SendWeather`, `CMap::SendObeliskList`, `CUser::EnterZone`) and the in-world
  packets (movement, chat).
- The meaning of `0x0404`, `0x0211` and `0x021B` to the client; the attack speed formula; what makes the client start loading.
- Whether the client needs any of the "if any" packets for a character without them (the original never sends them empty, so it should not).
- `usp_Read_Char_Items_Simple_R` and `usp_Read_BaseItemDefs_R` procedure texts (the MSSQL container has them).

## 8. Other episodes

The EP3 `ps_game.exe` (linked 2008-01-17) and the EP5.4 one (linked 2011-09-14, modified after linking) were compared with the EP4.5
build, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- Character create `0x0102` is read the same way: faction ≠ 2, slot < 5, grow ≤ max grow, family < 4, job < 6, then a 21-byte name at
  offset 10 trimmed and checked (EP3 ps_game 0x452837, 0x452896; confirmed). The list record is 83 bytes (EP3 ps_game 0x4539cb).
- The enter-world burst (EP3 ps_game `PacketUserDBChar` 0x453660, case `0x407`) calls the same steps in the same order (confirmed, call
  sequences compared). EP4.5 added `BillingApply` before the burst, the reconnect path (`0x0112`, `0x0113`, the `0x306` request) and the
  server time `0x0114` (confirmed: absent from the EP3 image), and the `0xB103` billing info after the handshake.
- Zone entry (`CZone::EnterUser`, EP3 ps_game 0x412db0) makes the same calls; EP4.5 only adds a log line before closing on an unsafe
  position (confirmed).

### EP5.4

- `GameVersion` from `ps_game.ini` only goes to the session server in `0x0805` (EP5.4 ps_game 0x4077c5); the game server never compares
  it with anything a client sends (confirmed).
- The character screen handles the same seven requests and closes on any other; names stay at most 20 bytes, family 0 to 3, job 0 to 5
  (EP5.4 ps_game 0x47a29c; confirmed).
- **New create check:** races 0 and 1 need the account's country 0, races 2 and 3 country 1; otherwise the connection is closed with
  reason 9 (EP5.4 ps_game 0x49028b, 0x490391, 0x490298; confirmed).
- Two more race and faction checks were compiled (at the list: delete the character; at loading: disconnect) and switched off after
  linking by three jump edits (EP5.4 ps_game 0x47b54e, 0x47b608, 0x47c134; confirmed against the untouched `ps_gamewar`).
- The list shows a character on an instance map or a map with a `MapCurse` for its faction at that map's bind position
  (`CMap::GetBindPos` EP5.4 ps_game 0x42ff60; confirmed at the call level). `CWorld::LoginUser` (EP5.4 ps_game 0x413b30) no longer
  sends billing info.
- Before `0x0201`, in state 3, the server answers the client's `0xB106` with the item-mall product pages (EP5.4 ps_game 0x474b75;
  confirmed).
