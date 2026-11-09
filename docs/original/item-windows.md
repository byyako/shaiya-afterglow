# Items that open a window: saved positions, body change, experience wheels

This page covers four items whose use starts with a window in the client: the Teleportation Stone and its four saved positions
(special 39), the Appearance Change and Sex Change Scrolls (specials 44 and 45), and the experience wheels (special 50). A *special* is
the number in an item's definition that says what using it does. It is about the EP4.5 game server `ps_game.exe`,
read in the Ghidra decompiler and the disassembly with the PDB names. The client side ("specials 39, 44, 45 and 50 open a window first")
is in the [client facts](../client/CLIENT-FACTS.md) and the [packet catalogue](../client/client-packets.generated.md) (`0x0220`, `0x0225`,
`0x0226`). Item use in general is on [Using items](item-use.md); this page completes what it says about specials 39 and 50.

The items: Teleportation Stone **100/65** (special 39; 100/166 to 100/168 are unnamed copies), Appearance Change Scroll **100/70** (44),
Sex Change Scroll **100/71** (45), the small and large experience wheels **44/213** and **44/214** (50). Items are named type/type id.

## In short

- **Four saved positions.** A character can save up to four places and later return to one with a Teleportation Stone. A place can only
  be saved on an ordinary map (not a dungeon or an instance) and within 50 units of where the character stands. Using the stone takes 5
  seconds, and moving, attacking or casting cancels it.
- **Changing looks.** The Appearance Change Scroll sets a new hair, face and size; the Sex Change Scroll also changes the sex, unless the
  character wears an item made for one sex only. The server does not check the values the client sends, so a modified client could pick
  looks the game does not offer.
- **Experience wheels hold a character at its level.** On maps 18 and 30, a character at least halfway through a level can use one to drop
  back to the start of the level and get its buff, so it can stay in the low-level fighting maps.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's; `CMap` from the PDB (`m_nMapID` `+0x78`, `m_nMapType` `+0x80`, `m_nCreateType` `+0x1a4`).

## How it was read

From `ps_game.exe` (EP4.5, with its PDB names), read in the Ghidra decompile (REA) and the disassembly (`objdump`, names from
`docs/original/symbols/ps_game.tsv`). Client side: `docs/client/CLIENT-FACTS.md` ("specials 39, 44, 45 and 50 open a window first"),
`docs/client/client-packets.generated.md` (`0x0220`, `0x0225`, `0x0226`). `CUser` offsets are the code's; `CMap` from the PDB (`m_nMapID`
`+0x78`, `m_nMapType` `+0x80`, `m_nCreateType` `+0x1a4`). Item use in general: `docs/original/item-use.md` (its sections 7 and 9 already name
specials 50 and 39; this document completes them).

The items (`defs.items`): Teleportation Stone **100/65** (special 39; 100/166 to 100/168 are unnamed copies), Appearance Change Scroll
**100/70** (44), Sex Change Scroll **100/71** (45), the small and large experience wheels **44/213** and **44/214** (50).

## 1. Saved positions

Confirmed.

A character has **four saved positions** (`+0x5870`: four `u32` maps, `0xFFFF` = none; `+0x5880`: four `x, y, z`). ps_dbAgent loads them with
the character (`0x0D01`, `TP_DB_SAVEPOINT_LIST`, `CUser::PacketUserDBSavePoint` 0x46e810: up to 4 × `u8 slot 0 to 3, u16 map, f32 x, y, z`).

**The list `0x0225`** (`CUser::SendSavePoint` 0x483d60, `TP_SAVEPOS_LIST`), in the enter-world burst after `0x0211` and before `0x0230`
(`docs/original/enter-world.md` section 3, row 15), **only when one is set**: `u8 count`, then per set position in slot order `u8 index (1 to
4), u16 map, f32 x, y, z` (15 bytes each).

**Saving one: `0x0220`** (`CUser::PacketMaininterface` 0x469858). Client → server, 17 bytes (`TPC_SAVEPOS_WRITE`): `u8 index, u16 map, f32 x,
y, z`. Not refused while dead. The answer is **`0x0220`** (18 bytes, `TP_SAVEPOS_WRITE`): `u8 result, u8 index, u16 map, f32 x, y, z`:

| check, in order | result |
|---|---|
| the player's map is a dungeon (`m_nMapType` 1: `MapType = D` in `Map.ini`; anything else is 0) | 1 |
| `index` is 0 or above 4 | 2 |
| `map` is not the player's map, or the position is more than **50** from the player's (`SVector::Distance2`, x and z) | 1 |
| the map's create type is not 0 (an instance, guild or event map) | 2 |
| otherwise: slot `index − 1` holds the player's map and **the position sent**; ps_dbAgent `0x0D02` (`TP_DB_SAVEPOINT_SET`: char id, slot, map, x, y, z) | **0** |

On a refusal only the result and the index are filled; the map and position bytes are whatever the stack held.

**Using the Teleportation Stone** (`CUser::ItemUse` 0x466097, special 39; the request's `target` is the index 1 to 4, which only `0x0557`
carries, so the client's window is *inferred* to send `0x0557`): refused (the error answer) when the index is not 1 to 4, the slot is not set, the level cap of the map refuses (`ChkMoveMap`), on
another map `CMap::EnableMoveByPortal` refuses (the obelisk portals), the map is not loaded, `x` or `z` is not within `[0, map size)`, the
map's create type is not 0, or a logout is pending. Otherwise the move by item of `docs/original/item-use.md` section 9: type 2, due in 5 s,
`0x0221` to the players within 60, cancelled by a step, an attack or a skill; when due the item is used up and the player moves, `0x0222` with
the result byte **= the index**.

## 2. Body change: `0x0226`

Confirmed, `CUser::PacketMaininterface` 0x469a80.

Client → server, 8 bytes (`TPC_BODYCHG`): `u8 bag, u8 slot, u8 hair, u8 face, u8 size, u8 sex`, sent from the scroll's window. Ignored while
dead, for bag 0, outside the bags (`CUser::IsBag`) or for an empty slot. An item whose real type is not 26 (the usable items) gets the item
use error (`0x050A` failure). Then by the item's special:

- **44** (Appearance Change Scroll): hair, face and size become the values sent; the sex is kept.
- **45** (Sex Change Scroll): when any item equipped in slots 0 to 12 has special **64 or 65** (2 and 3 such items in the data), the answer
  is **`0x0228`** (2 bytes, `TP_BODYCHG_FAIL`) and nothing changes. Otherwise hair, face, size **and sex** become the values sent.
- any other special: the item use error.

**Nothing checks the values**: any byte is taken, sex too. On a change: ps_dbAgent **`0x0615`** (`TP_DB_SET_BODY`: char id, hair, face,
size, sex), the item used up as any item (`CUser::ItemUseNSend`: one fewer, the `0x050A` answer), then **`0x0226`** (10 bytes, `TP_BODYCHG`:
`u32 player id, u8 hair, u8 face, u8 size, u8 sex`) to the players in view, the player included (`PSendViewCombat`). Players who come into
view later get the new looks in `0x0303` as usual.

## 3. Experience wheels

Confirmed, `CUser::ItemUse` 0x4657cc, special 50.

The wheel names a skill (`byRange`, `byAttackTime`), a buff. Refused (the error answer) unless:

- the experience is **at least halfway through the level**: `exp ≥ (GetExp(grow, level − 1) + GetExp(grow, level)) / 2` (the two totals
  added as `u32`, halved);
- the map is **18 or 30** (`m_nMapID`);
- the skill can be applied (`CUser::UseItemSkill`, `docs/original/item-use.md` section 7).

Then **`0x0207`** (10 bytes: `u32 GetExp(grow, level − 1) − exp`, a negative amount as `u32`; `u32 0`), the experience becomes the level's
start total `GetExp(grow, level − 1)` (saved, `CUser::SendDBExp`), and the wheel is used up. So a player in the low-level fighting maps can
stay at its level.

## 4. The new server

- **Saved positions**: table `character_save_points` (migration 0015: character, slot 0 to 3, map, x, y, z), loaded with the character, sent
  as `0x0225` in the burst between `0x0211` and `0x0230` when any is set; `0x0220` checked as section 1 and saved at once
  (`CharacterStore::save_point`). The Teleportation Stone uses the move by item that the recall runes use (result byte = the index); its
  destination checks are those of the recall runes (level cap, a standable position on the map, create type 0, no logout pending; the
  obelisk portals are not built, as for the runes).
- **Body change**: `0x0226` as section 2, saved (`CharacterStore::save_body`), the scroll used up, `0x0226` to the view; the zone keeps the
  new looks for `0x0303`.
- **Experience wheels**: as section 3.
- **Deviation 1 (approved):** the body change refuses a sex other than 0 or 1, as character creation does here (the original takes any byte,
  which a modified client can use to give a character a body the game does not have). Answer: the item use error, nothing changes.
- **Deviation 2 (approved):** a saved position is **the server's position of the player**, not the one the client sends (the original keeps
  the client's, which may be up to 50 away and on a spot no one can stand on). The 50 check and the other refusals stay.
- On a refused `0x0220` the map and position bytes are zeros (the original leaves stack bytes).

## 5. Other episodes

The EP3 build of `ps_game.exe` (2008-01-17) and the EP5.4 build (2011-09-14, modified after linking) were compared with the EP4.5 build
of this note, read statically like the rest of the note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

- The body change (`0x0226`), the saved positions and the experience wheels exist (the `0x0226` case is in the dispatch table); their
  bodies were not compared.
- `0x0228` (body change failed) is built only by EP4.5 (confirmed: the opcode constant is not in the EP3 image).

### EP5.4

- **Unchanged** (confirmed: the same calls and constants): the body change (`0x0226`, EP5.4 ps_game 0x4777dc), the experience wheels
  (special 50) and the Teleportation Stone (special 39).
- The recall tables of specials 47 to 49 are the same (EP5.4 ps_game 0x56e608, 0x56e628, 0x56e648). A new special 74 recalls to map 39
  with the same checks and 5-second cast ([`item-use.md`](item-use.md)).
