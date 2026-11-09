# Guild bank

This page covers the guild bank in the EP4.5 game server `ps_game.exe`: bag 255 and the `0x0204` moves into, out of
and inside it, with their checks (the guild's ranking, the member's rank, the slot count by the house's warehouse level), the fee, the
packets `0x0D29`, `0x0D31`, `0x0D32` and `0x0D28`, the database calls, and what happens when a member leaves or the guild is disbanded.
It was read in the Ghidra decompiler and the disassembly with the PDB names and types, in `ps_game.exe` and partly in `ps_dbAgent.exe`
(same pack). Nothing was executed.

Read in `ps_game.exe`: `CUser::PacketMaininterface` 0x468420 (case `0x0204`, 0x4685bb…0x4686c4), `CUser::IsGuildBank` 0x45b410,
`CUser::IsBag` 0x45b3d0, `CGuild::ItemBagToGuildBank` 0x42f390, `CGuild::ItemGuildBankToBag` 0x42fea0,
`CGuild::ItemGuildBankToGuildBank` 0x42fb40, `CGuild::GetGuildHouseBankAllowSlotCount` 0x430ee0 (and its table 0x541ca0),
`CGuild::SendBankItemList` 0x42f250 (and its callers `CUser::PacketUserDBChar` 0x46d659, `CGuild::JoinOK` 0x42e513, `CGuild::JoinUser`
0x42e9a7), `CGuild::SendExcludeMe` 0x42c600, `CGuild::Send` 0x42c050, `CItem::EnableCombine` 0x42b540, `CClientToDBAgent::PacketGuild`
0x408e00 (case `0x0B0D`), `CGuild::Reset` 0x42bcd0, `CGuild::Fire` 0x42d5a0, `CGuild::JoinLeave` 0x42da90, `CGuild::JoinExpel`
0x42dcb0, `CGuild::DeleteChar` 0x42edf0, `CGuildMgr::FireGuild` 0x4325e0, `CGuildMgr::RemoveGuildZeroUser` 0x4326d0. In
`ps_dbAgent.exe`: `CUser::PacketUserItem` 0x408480 (cases `0x0712`, `0x0713`), `CConnection::PacketGuild` 0x40ba00 (cases `0x0B05`,
`0x0B11`, `0x0B14`, `0x0B15`), `GuildInfo::Release` 0x40f420, `GuildMgrSave::DBSaveItem` 0x4105b0. The client side is on
[Social packets](../client/client-social-packets.md) and [World packets](../client/client-world-packets.md) (`0x0204` with bag 255); the
client itself was not read for this page.

The house and its NPC levels (the bank's level is key 6) are on [Guild houses](guild-house.md), the ranking the bank depends on
on [Guild ranking battle](guild-ranking.md). The personal warehouse (bag 100), whose shapes the bank follows, is on
[Warehouse](warehouse.md). Not covered: the game log records beyond their opcodes, the operator commands, `CreateType` G.

## In short

- **Every guild has one bank of 240 slots**, shared by all its members, used through the ordinary item move with bag 255. The other
  members see every change at once.
- **Only a ranked guild (1 to 30) may use it at all.** A ranked guild that never bought a house still has 40 slots; the house's
  warehouse NPC level raises that up to 240.
- **What a member may do depends on the guild rank**: new members (rank 9) nothing; ranks 3 to 8 may put items into empty slots; the
  master and officers (ranks 1 and 2) everything, including taking items out.
- **Taking an item out costs 5 %** of the stack's sell price, as the warehouse does; guild bars are free.
- **Quest items and bound items cannot be put in** directly.
- **Disbanding the guild loses the bank**, with no warning to anyone.
- **The bank is not saved in one step** with the character's items and money, so a server crash between the two can lose or double an
  item.

Status words: **confirmed** = read in the code (address given); **inferred** = follows from what was read but is not shown directly;
**unknown** = not read.

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names and types), read with the decompiler (REA) and the disassembly (objdump, names from
`docs/original/symbols/ps_game.tsv`): `CUser::PacketMaininterface` 0x468420 (case `0x0204`, 0x4685bb…0x4686c4), `CUser::IsGuildBank` 0x45b410,
`CUser::IsBag` 0x45b3d0, `CGuild::ItemBagToGuildBank` 0x42f390, `CGuild::ItemGuildBankToBag` 0x42fea0, `CGuild::ItemGuildBankToGuildBank`
0x42fb40, `CGuild::GetGuildHouseBankAllowSlotCount` 0x430ee0 (and its table 0x541ca0), `CGuild::SendBankItemList` 0x42f250 (and its callers
`CUser::PacketUserDBChar` 0x46d659, `CGuild::JoinOK` 0x42e513, `CGuild::JoinUser` 0x42e9a7), `CGuild::SendExcludeMe` 0x42c600,
`CGuild::Send` 0x42c050, `CItem::EnableCombine` 0x42b540, `CClientToDBAgent::PacketGuild` 0x408e00 (case `0x0B0D`), `CGuild::Reset`
0x42bcd0, `CGuild::Fire` 0x42d5a0, `CGuild::JoinLeave` 0x42da90, `CGuild::JoinExpel` 0x42dcb0, `CGuild::DeleteChar` 0x42edf0,
`CGuildMgr::FireGuild` 0x4325e0, `CGuildMgr::RemoveGuildZeroUser` 0x4326d0. From `ps_dbAgent.exe` (same pack): `CUser::PacketUserItem`
0x408480 (cases `0x0712`, `0x0713`), `CConnection::PacketGuild` 0x40ba00 (cases `0x0B05`, `0x0B11`, `0x0B14`, `0x0B15`),
`GuildInfo::Release` 0x40f420, `GuildMgrSave::DBSaveItem` 0x4105b0 and its strings. Layouts from the PDB: `TP_GUILD_BANK_UNIT`,
`TP_GUILD_BANK_ITEM_LIST`, `TP_GUILD_BANK_ITEM_IN` / `OUT`, `TP_DB_GUILD_BANK_LIST`, `TP_DB_ITEM_BAGTOGUILDBANK`,
`TP_DB_ITEM_GUILDBANKTOBAG`, `TPC_DB_GUILD_BANKITEM_FIXCOUNT` / `DROP`, `TP_GAMELOG_ITEM_GUILDBANKADD` / `REMOVE`, `stItemInfo`. Data:
`Data/GuildHouse.ini` (`[Bank] PermitSlot`, already in `world.guild_house.bank_slots`), the house maps' NPCs (`world.map_npcs`),
`PS_GameData.GuildStoredItems` (`docs/original/db/PS_GameData.columns.txt`). Nothing was executed. The client side is
`docs/client/client-social-packets.md` (`0x0D28`, `0x0D29`, `0x0D31`, `0x0D32`), `docs/client/client-world-packets.md` (`0x0204` with bag
255) and `docs/client/CLIENT-FACTS.md` (the NPC warehouse windows `0x67` and `0x72`); the client itself was not read for this.

Part 1 (`docs/original/guild-house.md`) covers the house and its NPC levels (the bank's level is key 6), part 2
(`docs/original/guild-ranking.md`) the ranking that the bank depends on. The personal warehouse (bag 100) is `docs/original/warehouse.md`;
the guild bank follows the same shapes. **Not covered:** the game log records beyond their opcodes, the operator commands, `CreateType` G.

Status words as in `docs/original/ps_game.md`: **verified** = read in the code (address given); **inferred** = follows from verified code
but is not shown directly; **unknown** = not read.

## 1. What it is

Every guild has **one bank of 240 slots**, bag number **255** in the packets, shared by all its members. Items go in and out through the
ordinary item move `0x0204` with bag 255 on one side; the other members see each change at once (`0x0D31`, `0x0D32`), and the whole bank
comes with the login and on joining (`0x0D29`). How many slots may be used depends on the house's warehouse NPC level (key 6), from 40 at
level 0 to 240 at level 5. Only a **ranked** guild (ranking 1 to 30) may use it at all; what a member may do depends on its guild rank:
rank 9 (new members) nothing, ranks 3 to 8 put items into empty slots, ranks 1 and 2 (master and officers) everything.

## 2. The bank's state and its loading

Confirmed.

- **`CGuild +0xa4`**: `CItem*[240]`, the bank, slot by slot; **`CGuild +0x464`**: its lock (every bank function holds it while it reads or
  changes the slots). `CUser::IsGuildBank(bag, slot)`: **bag 255 and slot below 240**.
- **Loading** (`CClientToDBAgent::PacketGuild` case `0x0B0D`, `TP_DB_GUILD_BANK_LIST`: `u32 guild, u8 count`, then `count` ×
  `DB_BANK_UNIT` (46 bytes: `u64 uid, u8 slot, u8 type, u8 type id, u16 quality, u8 gems[6], u8 count, u32 made at, u8 make type, char
  craft name[21]`), up to 50 a packet, at server start after the guild list): for each unit an item is made (uid, type, type id, quality,
  gems, count, made at, make type, craft name) and put at `CGuild +0xa4 + slot × 4`; **the slot is not checked** against 240.
  - an item whose type and type id have no definition: an error line ("Error Guild Load : BankItem type=%d, typeid=%d") to the log and to
    the operator tool (`0x080E`), then **a deliberate crash** (a write to address 0);
  - an **equipment** item (`CItem::IsEquipment`: real type 1 to 25) whose count is above its definition's stack limit: the count becomes the
    limit, an error line ("Error GuildBankItem Count : …") and ps_dbAgent is told (`0x0B14`, `TPC_DB_GUILD_BANKITEM_FIXCOUNT`: `u32 guild,
    u8 slot, u8 count`). Other items keep a count above the limit.
- `CGuild::Reset` (when a guild object is made or reused) frees the 240 items; a disbanded guild's items stay in its object until then
  (section 11).
- **The database** (`PS_GameData.GuildStoredItems`): `RowID, GuildID, ItemID, Type, TypeID, ItemUID, Slot, Quality, Gem1…Gem6, Craftname,
  Count, Maketime, Maketype, Del`, written by ps_dbAgent (section 10).

## 3. How many slots

Confirmed, `GetGuildHouseBankAllowSlotCount`.

The level of the guild's slot of **key 6** (`CGuild +0x47c`, part 1 section 4), **0 when the guild has no such slot, at most 5**, picks
the entry of the `u16` table at 0x541ca0, which `InitGuildHouseIniFile` fills from `[Bank] PermitSlot` (built in and shipped: **40, 80,
120, 160, 200, 240**). A slot number at or above that count is refused by the checks below (`0x0D28 (2, slot / 40)`): with the shipped
table, slot `s` needs warehouse level `s / 40`, which is the number sent. The bank level comes from the NPC upgrade `0x0D25` with key 6
(part 1 section 4: rank 1 or 2, `Bank UpgradeEtin` 200, 500, 1000, 2000, 5000; levels 1 to 5).

**Nothing here asks whether the house is bought or open** (`byBuyHouse`, `byUseHouse`): a ranked guild without a house has 40 slots.

## 4. The contents: `0x0D29`

Confirmed, `CGuild::SendBankItemList`.

Server → client, `TP_GUILD_BANK_ITEM_LIST`: `u16 0x0D29, u8 count`, then `count` × `TP_GUILD_BANK_UNIT` (33 bytes): **`u8 slot, u8 type,
u8 type id, u16 quality, u8 gems[6], u8 count, char craft name[21]`** (the layout of the warehouse's `TP_BANK_UNIT`, count after the
gems). Slots 0 to 239 in order, **50 a packet** (a full one is 1653 bytes, the last `3 + 33n`), **nothing at all for an empty bank**.
Every slot that holds an item is listed, whatever the slot count allows.

Sent to one player, under the guild lock (`CGuildLock`):

- **entering the world** (`PacketUserDBChar` 0x46d659): at the end of the enter-world packets (after `SendSavePoint`, `SendCharacterHonor`,
  `SendPointLog`, `SendCharacterStoredPointItem`), when the character is in a guild: `0x0D29`, then `0x0D24` (the NPC levels);
- **joining**: `JoinOK` (an accepted join request; to the new member, when online, after its member list) and `JoinUser` (an accepted
  invitation): `0x0D29`, then `0x0D24`.

It is not sent again when the window opens.

## 5. The request: `0x0204` with bag 255

Confirmed, `PacketMaininterface` 0x468475…0x4686c4.

`u8 from bag, u8 from slot, u8 to bag, u8 to slot` (`docs/original/items.md` section 1). After the checks every move has (ignored while the
status is 1 or the shape 100, or when both places are the same; then the trade is cancelled and the personal shop closed) and the
warehouse's branches (`docs/original/warehouse.md` section 3), the move takes the user's guild lock (`CGuildLock`) and goes, in this order:

| from | to | further checks (otherwise nothing, no answer) | handler |
|---|---|---|---|
| a bag (`IsBag`: bag 0 to the bag count, slot below 24) | the guild bank (`IsGuildBank`) | the player has a guild; the bag place holds an item whose **real type is not 27** (item types 27 to 29) and whose **`byOG` (`stItemInfo +0x31`) is 0** | `ItemBagToGuildBank` |
| the guild bank | the guild bank | the player has a guild | `ItemGuildBankToGuildBank` |
| the guild bank | a bag (`IsBag`) | the player has a guild (**the bag place may hold an item**) | `ItemGuildBankToBag` |
| the guild bank | anything else (the warehouse, bag 255 slot 240 or above, …) | | nothing |

So nothing moves directly between the warehouse (100) and the guild bank (255). **Nothing checks where the player is**: no NPC, no map.
The client offers bag 255 only in its guild warehouse window (section 12), but a modified client can use the bank anywhere. "The player's
guild rank" below is `CUser +0x174c` (`m_nGuildUserLevel`: 1 master, 2 officers, 3 to 9 members, 9 new; `docs/original/guild.md` section 2);
"the ranking" is the guild's `CGuild +0x48` (31 = unranked).

## 6. Bag to bank

Confirmed, `CGuild::ItemBagToGuildBank`.

In order:

| check | answer |
|---|---|
| the bag is **0** (the equipment) | nothing |
| the guild's **ranking above 30** | `0x0D28 (1, 30)` "Your Rank isn't high enough." |
| the player's guild rank is **9** | `0x0D28 (4, 8)` "Not authorized" |
| the bank slot is at or above the slot count (section 3) | `0x0D28 (2, slot / 40)` "level is not high enough…" |
| the bank slot **holds an item** and the player's guild rank is **above 2** | `0x0D28 (2, 2)` "level is not high enough…" |
| otherwise | the move |

**The move**: the item leaves the trade window if it was offered (`ExchangeItemRemove`). An empty bank slot takes the item, the bag place is
emptied; a bank slot that holds an item **swaps** with the bag place (the bank's item goes into the bag, no fee, no check on it). Then:
ps_dbAgent `0x0712` (`TP_DB_ITEM_BAGTOGUILDBANK`, 15 bytes: `u32 dwUserID` (`CUser +0x578c`), `u32 guild, u8 bag, u8 slot, u8 count now in
the bank slot, u8 bank slot, u8 count now in the bag place (0 for none)`), the game log `0x0413` (`TP_GAMELOG_ITEM_GUILDBANKADD`, the item
that went in) and, for a swap, `0x0414` (`…GUILDBANKREMOVE`, the item that came out). **The other online members** (`CGuild::SendExcludeMe`)
get, for a swap, **`0x0D32`** with the item that came out, then **`0x0D31`** with the item now in the bank slot (section 9). Last, the
player gets **`0x0204`, 70 bytes** (`TP_ITEM_BAGTOBAG`): the bag place as it is now (empty, or the swapped item), then the bank place (bag
255) with its item.

Stacks never combine on the way in: a stack dropped onto the same item in the bank swaps with it (rank 1 or 2) or is refused.

## 7. Bank to bag

Confirmed, `CGuild::ItemGuildBankToBag`.

In order:

| check | answer |
|---|---|
| the bank slot is empty, or the bag is 0 | nothing |
| the guild's ranking above 30 | `0x0D28 (1, 30)` |
| the player's guild rank is **3 or more** | `0x0D28 (2, 2)` |
| the bank slot is at or above the slot count | `0x0D28 (2, slot / 40)` |
| the fee (below) is above the player's money | nothing |
| otherwise | the move |

**The fee** is the warehouse's (`docs/original/warehouse.md` section 5): nothing for item type **94** (the guild bars, `byType '^'`),
otherwise `count × nSell` (32 bits, made a `float`, then times `0.05f` in the x87 with the rounding set to chop): **5 % of the stack's sell
price, rounded down**.

**The move**: an empty bag place takes the item; a bag place that holds an item **swaps** with the bank slot (that item leaves the trade
window first, and goes into the bank **without the checks of section 5 or 6**: real type 27, `byOG`, rank 9). Then ps_dbAgent `0x0713`
(`TP_DB_ITEM_GUILDBANKTOBAG`, 15 bytes: `u32 dwUserID, u32 guild, u8 bank slot, u8 count now in the bag place, u8 bag, u8 slot, u8 count now
in the bank slot (0 for none)`), the game log `0x0414` (the item out) and, for a swap, `0x0413` first (the item in). The other online
members get **`0x0D32`** with the item that came out (its slot byte the bank slot) and, for a swap, **`0x0D31`** with the item now in the
bank slot. Then the money goes down by the fee, the player gets **`0x0204`, 74 bytes** (`TP_ITEM_BANKTOBAG`: the bank place (255) as it is
now, the bag place with the item, then `u32` the money now), and the money is saved (`CUser::SendDBMoney`). The fee is taken for a swap too.

## 8. Bank to bank

Confirmed, `CGuild::ItemGuildBankToGuildBank`.

In order:

| check | answer |
|---|---|
| the source slot is empty | nothing |
| the guild's ranking above 30 | `0x0D28 (1, 30)` |
| the player's guild rank is **3 or more** | `0x0D28 (2, 2)` |
| the **target** slot is at or above the slot count | `0x0D28 (2, target / 40)` |
| otherwise | the move |

The source slot is not checked against the count. **The move**, as the warehouse's (`docs/original/warehouse.md` section 6): to an empty
slot the item moves; to a taken slot, when the two items combine (`CItem::EnableCombine`: both stack limits above 1, same type and type id)
everything goes into the target when the sum fits the stack limit (the source emptied, its item freed), otherwise the target holds the limit
and the source the rest; items that do not combine swap. ps_dbAgent `0x0B11` (10 bytes: `u32 guild, u8 source slot, u8 count now in the
target, u8 target slot, u8 count now in the source (0 for none)`). No game log, no `0x0D31` / `0x0D32`. **Every online member, the player
included** (`CGuild::Send`), gets the **`0x0204`, 70 bytes**: the source place (255), then the target place (255), as they are now.

## 9. The packets

Confirmed.

| op | dir | size | layout | sent |
|---|---|---|---|---|
| `0x0204` | S→C | 70 / 74 | as the warehouse's: two places of 34 bytes (`u8 bag, u8 slot, u8 type, u8 type id, u8 count, u16 quality, u8 gems[6], char craft name[21]`; type 0 = empty), `+ u32 money` when the item left the bank | sections 6 to 8 |
| `0x0D29` | S→C | 3 + 33n | `u8 count`, n × bank unit | section 4 |
| `0x0D31` | S→C | 39 | `TP_GUILD_BANK_ITEM_IN`: bank unit (its slot the bank slot), `u32 dwGuildCharID` = **the character id of the member who moved it** | to the other online members: a bank slot now holds this item |
| `0x0D32` | S→C | 39 | `TP_GUILD_BANK_ITEM_OUT`: the same; the unit is **the item that left**, its slot byte the bank slot it left | to the other online members: that slot was emptied |
| `0x0D28` | S→C | 4 | `u8 type, u8 require` (`TP_GUILD_HOUSE_ACTION_ERR`) | the refusals above |

`0x0D28`'s types as the client prints them (`docs/client/client-social-packets.md`): 1 "Your Rank isn't high enough." (8026), 2 "level is
not high enough to have service upgraded." (8027), 3 "guild warehouse is full." (8028; **never sent**), 4 "Not authorized" (2038). The
client notes give only the texts; that the second byte is not shown is *inferred*. The bank's numbers: `(1, 30)` the ranking needed, `(2,
n)` the warehouse level needed, `(4, 8)` and `(2, 2)` the guild rank needed (8 or better, 2 or better).

`0x0D31` and `0x0D32` go to every other online member, wherever it is (not only those in the house).

## 10. The database calls

Confirmed, in ps_game; ps_dbAgent's side partly.

| op | from | to ps_dbAgent | ps_dbAgent |
|---|---|---|---|
| `0x0712` | bag to bank (section 6) | `TP_DB_ITEM_BAGTOGUILDBANK` | moves the character's item into the guild's entry (a swap when the entry holds one) and sets both counts from the packet (`CUser::PacketUserItem` 0x408480) |
| `0x0713` | bank to bag (section 7) | `TP_DB_ITEM_GUILDBANKTOBAG` | the reverse (same function) |
| `0x0B11` | bank to bank (section 8) | 10 bytes above | moves or swaps the two entries, sets their counts; an entry whose count is now 0 goes to the delete list |
| `0x0B14` | loading (section 2) | `TPC_DB_GUILD_BANKITEM_FIXCOUNT` | sets the entry's count |
| `0x0B15` | **never sent** by ps_game | `TPC_DB_GUILD_BANKITEM_DROP` (`u32 guild, u8 slot`) | deletes the entry |
| `0x0B0D` | — | ps_dbAgent → ps_game at start | the bank list (section 2) |

ps_dbAgent keeps every guild's bank in memory (`GuildInfo +0x6c`, 240 entries, each with a state: 2 = changed) and writes it to
`GuildStoredItems` later with `Usp_Save_Guild_StoredItem_Add_E`, `_Mod_E` and `_Del_E` (`GuildMgrSave::DBSaveItem`) — *inferred* to run on
its save timer, as the other `GuildMgrSave` writes. The money of a withdrawal is saved on its own (`0x060B`). So a move is not saved in one
step with the character's items or money.

## 11. When the guild changes

Confirmed.

- **A member leaves or is kicked** (`JoinLeave`, `JoinExpel`), or **its character is deleted** (`DeleteChar`): nothing touches the bank. The
  items stay with the guild; the player's client clears its guild on `0x0D09` / `0x0D0A` / `0x0D0C` (whether that clears its copy of the
  bank is a client question, section 12). On joining another guild, `0x0D29` brings that guild's bank.
- **The guild is disbanded** (`CGuildMgr::FireGuild`: `CGuild::Fire`, ps_dbAgent `0x0B05`, `0x0D11` to the country): the bank is not looked
  at; nobody gets the items back. ps_game keeps them in the freed guild object until it is reused (`CGuild::Reset`). ps_dbAgent runs
  `usp_Delete_Guild_E` (`DBGuild::DeleteGuild`); its body is not in the pack, so **whether the `GuildStoredItems` rows are deleted or kept
  is unknown** (the table has a `Del` column). Either way the guild is never loaded again: the items are gone for the players.
- **A guild with no members at start** (`RemoveGuildZeroUser`) is disbanded the same way.
- **The house closes, or the guild loses its ranking** (`docs/original/guild-ranking.md` section 8): only the ranking counts. A guild still
  ranked with its house closed (`byUseHouse` 2) or not bought keeps its bank; an **unranked** guild (31) gets `(1, 30)` for every move, and
  its items stay until it is ranked again. The warehouse level never goes down, so no item is ever above the slot count (but one could be
  moved out of a slot above it by a bank-to-bank move, whose source is not checked).

## 12. The client (from `docs/client`, not read here)

- The warehouse keeper's menu sends nothing and opens window `0x67`; **on maps 51 and 52 (the guild houses) the warehouse keeper opens
  window `0x72`** (`docs/client/CLIENT-FACTS.md`, row "NPC warehouse"). The houses' keepers are NPC **6/27** on map 51 at (436.5, 363.2)
  and **6/28** on map 52 at (482.6, 430.5) (`world.map_npcs`); type 6 is the warehouse keeper everywhere. That window `0x72` is the guild
  bank's (bag 255) is *inferred*: the client notes do not say what it shows.
- The client keeps the bank in `DAT_021e5be0` (40-byte slots), filled by `0x0D29`, `0x0D31`, `0x0D32` and by `0x0204` answers with bag 255;
  for `0x0204` with source 255 and a target outside the bank it reads the 74-byte form (`docs/client/client-world-packets.md`).
- **Open for the client side**: what window `0x72` shows and which moves it sends; whether `0x0D29` clears slots it does not list (the
  server sends only full slots, so a member who changes guilds may keep the old bank's items on screen); whether leaving a guild clears the
  bank; whether the second byte of `0x0D28` is shown; what the client does with `0x0D31`'s character id.

## 13. Original oddities

- **No place check**: the bank can be used from anywhere by a client that sends `0x0204` with bag 255 (the warehouse has the same hole).
- **A swap skips the deposit checks**: taking an item out onto a taken bag place puts that bag item into the bank without the real-type-27
  and `byOG` checks, so a bound or quest item can enter the bank that way (and another member can then take it out). It also skips the
  rank-9 refusal, but the withdrawal needs rank 1 or 2 anyway.
- **The rank refusals use type 2** ("level is not high enough to have service upgraded.") with 2, while rank 9's deposit uses type 4 ("Not
  authorized") with 8: the same kind of refusal shows two different texts, and the first one talks about an NPC level.
- **Type 3 "guild warehouse is full." is never sent.**
- **The bank needs a ranking, not a house**: a ranked guild that never bought a house has 40 slots; a ranked guild whose house is closed
  for want of Etin keeps the bank.
- **The source slot of a bank-to-bank move is not checked** against the slot count.
- **A bank-to-bank move tells every online member with `0x0204`**, while the moves in and out use `0x0D31` / `0x0D32` for the others.
- **Saving is split**: the item move goes to ps_dbAgent's memory and is written later; the withdrawal's money is saved apart. A crash
  between them loses or doubles items.
- **Loading crashes the server** on a bank item without a definition, and does not check the slot number.
- **Disbanding loses the bank**, with no warning to anyone.

## 14. The new server

Built (`docs/plans/done/guild-bank.md`); `cargo test` covers the packets (`shaiya-proto`), the rules
(`crate::guild_bank`: slot counts, every refusal, fees, swaps, combining, loading), the guild side (`crate::guild`: the burst, joining,
what the others are told, the disband refusal) and the SQL against a real PostgreSQL (`tests/postgres.rs`). Not yet tried with the client:
section 36 of `docs/testing/live-test-checklist.md`.

- **Data**: nothing new. `world.guild_house.bank_slots` holds `PermitSlot` (`HouseRules::bank_slots`); the bank's level is the house's
  key 6 (`House::level(KEY_BANK)`), capped at 5.
- **Database** (`db/migrations/0021_guild_bank.sql`): **`guild_bank_items`** (`guild_id` → `guilds`, `slot` 0 to 239, and the item columns
  of `bank_items`: `item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid`), primary key `(guild_id, slot)`
  deferrable as `bank_items`'; `uid` unique and kept when an item moves between a bag and the bank (as the warehouse). A disbanded guild's
  rows are kept with its row (`guilds.disbanded_at`), never loaded.
- **In memory**: each guild (`crate::guild::Guild`) holds its bank (240 places, bag 255), loaded with the guilds at start. The rules
  (sections 3, 5 to 8) are pure in `crate::guild_bank`: the slot count, the checks in the original's order, the fee
  (`inventory::withdrawal_fee`), the combining (as the warehouse's), and the changes to save. `server/guild_bank.rs` handles the request:
  the plan under the guild lock, then **one database transaction** for the money, the bag place and the bank place
  (`ItemStore::guild_bank`), then both sides applied and the packets sent. Guild bank moves run one at a time (one async lock), so two
  members never plan against the same state.
- **Packets**: `0x0D29` in the enter-world guild packets (before `0x0D24`) and on joining (both ways, before `0x0D24`); `0x0204` (70 or 74),
  `0x0D31`, `0x0D32`, `0x0D28` as sections 6 to 9; `0x0D31` / `0x0D32` to the other online members, a bank-to-bank `0x0204` to every
  online member.
- **Loading**: a bank row without an item definition is left out with an error line (the original crashes); an equipment item's count
  above its stack limit is lowered to it and saved, as the original's `0x0B14`.
- **Deviations** (built; all four approved):
  1. **The bank needs the house's warehouse keeper**: a move with bag 255 on either side is ignored (no answer, as the original's other
     refusals) unless the player stands on a guild house map (`CreateType` H, maps 51 and 52, so in its own guild's house zone) with a
     warehouse keeper (NPC type 6) in view. The original checks neither, so a modified client can use the bank anywhere. The Warehouse
     Recall buff does not count (the rune opens the personal warehouse's window). A consequence: the bank is usable only while the house can
     be entered (bought and open, part 1 section 7), or by someone still inside.
  2. **An item that enters the bank by a swap passes the deposit checks**: in a withdrawal onto a taken bag place, the bag's item must not be
     of real type 27 nor bound (`byOG`, the `reqog` column; `ItemDef::bound`); otherwise nothing happens. The original lets it in, which
     lets a bound item pass to another member.
  3. **The rank refusals say "Not authorized"**: `0x0D28 (4, 2)` instead of `(2, 2)` (a deposit onto a taken slot below rank 2, a
     withdrawal or bank-to-bank move from rank 3 on). The original's type 2 prints the NPC-level text; type 4 is what it uses for rank 9.
  4. **A guild whose bank holds items cannot be disbanded**: the master's `0x0D03` gets `0x0D03 0` ("Failed to disband guild.", 380) and
     nothing else happens, for the at-once disband (two members or fewer) and before any agreement is asked. Disbands that nobody asks for
     (no member left at start, the last member's character deleted) still happen; their bank rows stay with the disbanded guild, and the
     log names each item. The original disbands and the items are lost.
- **Built as**: the rules in `crate::guild_bank`; the guild's bank is `Guild::bank`; `Guilds::bank_plan` / `bank_apply` and the
  zone's `guild_bank_plan` / `guild_bank_apply` / `guild_bank_turn`; the handler `server/guild_bank.rs`; the database call
  `ItemStore::guild_bank` (one transaction: the money, the bag places and the bank places; a bag place and a bank place swap in
  three statements); loading in `start_guilds` (`guild_bank::check_loaded`, lowered counts saved with `GuildWrite::BankCount`).
  A bank row that has no definition is left out of memory but stays in the database; a deposit into that slot is then refused by
  the table's key and ends the connection, as any move the database refuses.
- **Not deviations, but different underneath**: the move, its fee and the money are saved together before the answer (the original saves
  them apart and later); a bank row's slot is checked (0 to 239) by the table.

## 15. Other episodes

The EP3 and EP5.4 builds of `ps_game` (EP3 2008-01-17; EP5.4 2011-09-14, modified after linking) and `ps_dbAgent` (EP3 2008-01-04;
EP5.4 2011-09-14, modified after linking) were compared with EP4.5, read statically like the rest of this note; overview in
[`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

The guild bank exists: the load error string (EP3 ps_game 0x4e3974), `CGuild::ItemBagToGuildBank` (0x42a6c0, matched by call graph,
the same 15 calls) with the same refusals of real type 27 and bound items (0x44f44f, 0x44f458), and `GuildMgrSave::DBSaveItem` in EP3
ps_dbAgent (0x40f770, matched by strings) (confirmed). `guild-bank-bound` is most likely there too (inferred, EP3 ps_game 0x42b170).

### EP5.4

- **A stack of count 0 is dropped at load** (`CClientToDBAgent::PacketGuild` EP5.4 ps_game 0x406025–0x4060ec, matched by the string
  `Error GuildBankItem Count`; confirmed): the game sends `0x0B15` (`u32 guild, u8 slot`, 7 bytes) to the database agent, logs
  `GuildItemLoad ItemCount=0 …` to the operator link as `0x080E`, and creates no item. EP4.5 never sends `0x0B15`.
- The rest of the loader is the same: an oversized equipment row is lowered and fixed with `0x0B14`, and an undefined item still crashes
  the server on purpose (EP5.4 ps_game 0x406006) (confirmed).
- The three moves keep their constants (inferred), and the bound-item swap is most likely still there (inferred, EP5.4 ps_game 0x435bc0).
