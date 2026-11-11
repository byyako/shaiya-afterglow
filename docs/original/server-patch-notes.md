# Patch kit notes: how each fix works

This page is the engineering write-up of the [patch kit](../server-patches.md): what each fix changes, why the bytes are what they
are, and how they were checked. The kit page has the download and the byte lists.

A checked, reversible byte patcher for people still running the **original** EP4.5
Shaiya game server. It fixes bugs and exploits in `ps_game.exe` on disk; it does not
touch the database, the client, or any other server file, and it never runs the
executable. The patcher and the byte lists are in `tools/server-patches/`
(`patch_ps_game.py`, `fixes.json`). The executable itself is never distributed with
the kit.

This is the companion to `docs/original/community-fixes.md`, which lists the same bugs and
what the new (clean-room) server does about each. Every patch here was found by
static analysis of the one build below and **verified in disassembly** — assembled,
written into a copy, disassembled again and checked against the surrounding code,
the jump targets and the stack. **Nothing was run against a live server**, because
none is available here; no patch is claimed to be tested live.

## The build

These patches are for exactly one build of `ps_game.exe`:

- **SHA-256:** `0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547`
- **Size:** 1,323,008 bytes
- **Built:** 2009-07-14

The patcher refuses any other file. Its addresses are specific to this build; they
do not apply to other episodes or recompiles. Many community fixes circulate as
Cheat Engine tables (`.CT`) that edit the running process in memory; this kit instead
edits the file on disk, so the fix survives a restart and needs no injector.

## Using it

```
python3 patch_ps_game.py list [ps_game.exe]        # every fix and its state
python3 patch_ps_game.py status ps_game.exe        # which fixes are applied
python3 patch_ps_game.py apply  ps_game.exe        # all clear-bug fixes
python3 patch_ps_game.py apply  ps_game.exe <id>…  # only the named fixes
python3 patch_ps_game.py revert ps_game.exe all    # undo every applied fix
```

`apply` with no ids applies the **clear-bug** set (the default). **Design-choice**
fixes change how the game plays and are applied only when named. The first `apply`
writes a `ps_game.exe.bak` next to the file. Every change is printed as a
before/after byte list.

**Safety.** Before any change the tool reconstructs the pristine image by undoing
every patch it recognises; that image must hash to the SHA-256 above. So it only ever
edits this exact build, whether the file is pristine or already carries some of these
fixes, and it refuses a file with edits it does not recognise. `apply` and `revert`
are idempotent, and a full revert restores the file byte-for-byte (there is an
automated test for this).

## The code cave

30 of the fixes need a few extra instructions, which go in the
unused, already mapped padding after the last function in `.text` (virtual address
0x520250 onward; the fixes use 0x520250–0x520812).
Each such fix redirects a handful of bytes at its site to its own slot in that
region, which runs the displaced instructions plus the fix and jumps back. When any
cave fix is applied, the tool also raises `.text`'s VirtualSize (file offset
0x200) from 0x11f244 to 0x120000 so the region is mapped explicitly; it
restores the field when the last cave fix is reverted. (The community's own patched
builds use the same padding without the VirtualSize change, because the whole raw
section is mapped anyway; the kit changes it so the mapping is not left implicit.)

## Clear-bug fixes (applied by default)

### `mob-speed-id` — Monster speed packet carries the monster's id

- **Symptom:** CMob::SendSpeed builds 0x0609 without the monster id; nearby players get stack garbage as the id when a monster resets its speed.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/mob-fight.md section 5)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x490742 | 0x090742 | `83c050884c2407` | `e909fb08009090` |
  | 0x520250 (cave) | 0x120250 | `00000000000000000000000000000000000000` | `83c0508b500c89542402884c2407e9e604f7ff` |

### `gatekeeper-logout` — Gatekeeper takes no fare when a logout is pending

- **Symptom:** With a logout pending the gatekeeper subtracts the fare and the player does not move.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/gatekeeper.md section 2)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x4691a7 | 0x0691a7 | `83bddc570000007530` | `e9b7700b0090909090` |
  | 0x520263 (cave) | 0x120263 | `000000000000000000000000000000000000` | `83bddc570000000f85af90f4ffe93b8ff4ff` |

### `time-damage-player` — Damage over time on players multiplies, as on monsters

- **Symptom:** A damage-over-time debuff on a player adds elapsed/3000 to the base instead of multiplying, unlike the monster version.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/mob-skills.md section 8)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x4864f6 | 0x0864f6 | `442410` | `4c2410` |
  | 0x486527 | 0x086527 | `442410` | `4c2410` |
  | 0x486559 | 0x086559 | `442410` | `4c2410` |

### `illusion-save` — Illusion is not saved across a relog

- **Symptom:** Illusion (change type 102) is saved with the character but its copied monster is not; after login the player has shape 102 with monster 0 and item attack/defence drop to 0 until it ends.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/skills-special.md section 5.4)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x46cbc2 | 0x06cbc2 | `6683bb98000000640f84af000000` | `e9ae360b00909090909090909090` |
  | 0x520275 (cave) | 0x120275 | `000000000000000000000000000000000000000000000000… (33 bytes)` | `6683bb98000000640f84fcc9f4ff6683bb98000000660f84… (33 bytes)` |

### `myshop-visit-cleanup` — Leaving a personal-shop visit clears the seller's list

- **Symptom:** Leaving the zone while visiting a personal shop removes the visitor from its own shop, not the seller's, so the seller's list keeps dead visitors and fills up.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/personal-shop.md section 6)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x44af2e | 0x04af2e | `8bc5e86bd50200` | `e963530d009090` |
  | 0x520296 (cave) | 0x120296 | `00000000000000000000000000000000` | `8d80dc150000e8ff81f5ffe98facf2ff` |

### `learn-skill-level` — Only the next skill level can be learnt

- **Symptom:** 0x0209 accepts any level above the one known, so a modified client jumps skill levels; a passive then drifts for good.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/skills.md sections 3 and 7)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x4844b0 | 0x0844b0 | `395910897424107c29` | `e9f1bd090090909090` |
  | 0x5202a6 (cave) | 0x1202a6 | `000000000000000000000000000000000000000000` | `897424108b41104039d80f850342f6ffe92742f6ff` |

### `enchant-material-bounds` — The enchant material's place is bounds-checked

- **Symptom:** 0x0805's material bag and slot index the item table without bounds, so a bad place reads outside it (a crash, or a stray object as material).
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/lapis.md section 5)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x45fbb3 | 0x05fbb3 | `0fb645048d14400fb645058d0cd08b8c8fc4010000` | `e903070c0090909090909090909090909090909090` |
  | 0x5202bb (cave) | 0x1202bb | `000000000000000000000000000000000000000000000000… (43 bytes)` | `0fb645043b870413000077180fb64d0583f918730f8d1440… (43 bytes)` |

### `cloak-lapis` — Cloaks take no lapis (link)

- **Symptom:** EnableGemAdd says yes for a cloak before checking the lapis, so any item can be linked into a cloak for the price of its grade (usually free).
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/lapis.md sections 3 and 5)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x45fa69 | 0x05fa69 | `83fd187505` | `e978080c00` |
  | 0x5202e6 (cave) | 0x1202e6 | `0000000000000000000000000000` | `83fd180f8428f8f3ffe97ff7f3ff` |

### `cloak-lapisia` — Cloaks take no Lapisia (enchant)

- **Symptom:** EnableLapisianAdd says yes for a cloak before checking the Lapisia, so any item can enchant a cloak (an apple to +20 absorption, free).
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/lapis.md sections 3 and 5)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x45f979 | 0x05f979 | `83ff187505` | `e976090c00` |
  | 0x5202f4 (cave) | 0x1202f4 | `0000000000000000000000000000` | `83ff180f8448f7f3ffe981f6f3ff` |

### `nan-pos` — NaN positions are refused

- **Symptom:** CMap::IsSafePos lets NaN coordinates pass its x87 range checks.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/maps-npcs.md section 2)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x415360 | 0x015360 | `d9442408d81d28a45300` | `e99daf10009090909090` |
  | 0x520302 (cave) | 0x120302 | `000000000000000000000000000000000000000000000000… (49 bytes)` | `d9442408dde8dfe0f6c4447519d944240cdde8dfe0f6c444… (49 bytes)` |

### `bound-trade` — Bound items cannot be traded

- **Symptom:** PacketExchange checks only real type 27, not byOG, so a modified client can trade account/character-bound items.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/trade.md section 2.3)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x46f03a | 0x06f03a | `8b703083bea00000001b0f8490020000` | `e9f4120b009090909090909090909090` |
  | 0x520333 (cave) | 0x120333 | `000000000000000000000000000000000000000000000000… (31 bytes)` | `8b703083bea00000001b0f8497eff4ff807e31000f858def… (31 bytes)` |

### `bound-duel` — Bound items cannot be staked in a duel

- **Symptom:** The duel stakes window checks only real type 27, not byOG, as the trade window does.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/duel.md section 4)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x47d764 | 0x07d764 | `8b703083bea00000001b0f84a5020000` | `e9e92b0a009090909090909090909090` |
  | 0x520352 (cave) | 0x120352 | `000000000000000000000000000000000000000000000000… (31 bytes)` | `8b703083bea00000001b0f84b7d6f5ff807e31000f85add6… (31 bytes)` |

### `party-exp-levelup` — The full-party experience bonus is paid on a level-up

- **Symptom:** In a party of 7 each gain counts the bonus twice except a gain that levels the player up, which counts it once.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/party.md section 8)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x45835f | 0x05835f | `3bf0733d03f3` | `e92c800c0090` |
  | 0x520390 (cave) | 0x120390 | `0000000000000000000000000000000000` | `01de39c67305e9ca7ff3ff90e9ff7ff3ff` |

### `stealth-equip` — Equipment counts normally in Stealth

- **Symptom:** An item moved while hidden (Stealth, shape 13) keeps or doubles its attack and defence, because the equip code skips its numbers while any shape is set and Stealth never recomputes them.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/skills-buffs.md section 5); the community's own fix is a code cave by Bowie (kreon800/shaiya) that skips item moves in shape 13, and an earlier ItemBagToBag cave of unnamed authorship; this kit instead counts the items.
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x5203a1 (cave) | 0x1203a1 | `0000000000000000000000000000000000` | `80bf5413000000740780bf541300000dc3` |
  | 0x5203b2 (cave) | 0x1203b2 | `0000000000000000000000000000000000` | `80b85413000000740780b8541300000dc3` |
  | 0x454d1d | 0x054d1d | `80bf5413000000` | `e87fb60c009090` |
  | 0x454e21 | 0x054e21 | `80bf5413000000` | `e87bb50c009090` |
  | 0x454fcb | 0x054fcb | `80bf5413000000` | `e8d1b30c009090` |
  | 0x455200 | 0x055200 | `80bf5413000000` | `e89cb10c009090` |
  | 0x45544c | 0x05544c | `80bf5413000000` | `e850af0c009090` |
  | 0x455565 | 0x055565 | `80bf5413000000` | `e837ae0c009090` |
  | 0x4556ca | 0x0556ca | `80bf5413000000` | `e8d2ac0c009090` |
  | 0x455aeb | 0x055aeb | `80bf5413000000` | `e8b1a80c009090` |
  | 0x456136 | 0x056136 | `80b85413000000` | `e877a20c009090` |
  | 0x4562f6 | 0x0562f6 | `80b85413000000` | `e8b7a00c009090` |

### `bound-shop` — Bound items cannot be sold in a personal shop

- **Symptom:** The personal-shop place handler stores a slot without checking the item, so a modified client can sell quest or bound items.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/personal-shop.md sections 3.3 and 5)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x478e9d | 0x078e9d | `898cbde4150000` | `e921750a009090` |
  | 0x5203c3 (cave) | 0x1203c3 | `000000000000000000000000000000000000000000000000… (60 bytes)` | `5089c88d04400fb6d38d04c28b8485c401000085c0741f8b… (60 bytes)` |

## Input filtering (clear-bug fixes, applied by default)

These four fixes replace the **byte** `0x27` (ASCII apostrophe) with `0x20`
(space) before the indicated player text reaches the database statement builder.
For example, `O'Neil` becomes `O Neil`. Character, guild and gift fields are
normalized in the request buffer before validation/lookup, storage and copies;
GM details are normalized only in the outgoing log copy, leaving the displayed
notice and its source intact. Each replacement keeps the byte count and NUL.
Every new hook is five whole instruction bytes and has its own cave.

This is deliberately byte normalization, not a general SQL-escaping layer.
It covers these client-originating paths through this exact `ps_game.exe`.
It does not clean existing database names, account names, other log fields,
direct requests to `ps_dbAgent`/`ps_gameLog`, or other executables. Keep those
services private. Existing length checks and name rules still apply; malformed
packet lengths and the existing unbounded GM copy are separate problems.
The original chat log writer replaces quotes with `0x5f` (underscore,
`ps_gameLog` 0x403b46; see `docs/original/game-log.md` section 3.5); these
patches use the requested space byte.

The hooks and SQL sinks below were independently checked using `objdump`
and the matching PDB symbol TSVs. The original executables were read only.
The new x86 code is also exercised in Unicorn, with real `TrimString` and
`strncpy` code and a stub for the guild lock constructor. These checks establish
instruction behavior, not live database or server behavior.

### `character-name-filter` — Character create and rename names replace quotes with spaces

The create hook is at **0x46c006**, resume **0x46c00b**; rename is at
**0x46c173**, resume **0x46c178**. Both replay `call TrimString` (0x46c250),
then scan `EBP` up to **20 bytes** or the first NUL, preserving the call's
registers and flags. `TrimString(name,21)` already forces byte 20 to NUL
and removes spaces; filtering afterwards keeps the replacement spaces.
The normalized name is validated and copied to create DB packet `0x0404`
or rename `0x0409`, including the rename log. In `ps_dbAgent`,
`DBCharacter::CreateCharacter` formats `Usp_Create_Char_R` at 0x42040f;
`RenameCharacter` formats `usp_Save_Char_Name_E` at 0x41d72e. Both use
quoted `%s` names. The old name on a rename may still contain legacy data.

**Status:** verified against the pristine executable, disassembly and x86 emulation; live test pending.

| virtual address | file offset | before | after |
|---|---|---|---|
| 0x46c006 | 0x06c006 | `e845020000` | `e9f4430b00` |
| 0x5203ff (cave) | 0x1203ff | `0000000000000000000000000000000000000000000000000000000000000000000000000000` | `e84cbef4ff9c608d7500b914000000803e00740b803e277503c6062046e2f0619de9e6bbf4ff` |
| 0x46c173 | 0x06c173 | `e8d8000000` | `e9ad420b00` |
| 0x520425 (cave) | 0x120425 | `0000000000000000000000000000000000000000000000000000000000000000000000000000` | `e826bef4ff9c608d7500b914000000803e00740b803e277503c6062046e2f0619de92dbdf4ff` |

### `guild-text-filter` — Guild names and remarks replace quotes with spaces

At **0x476507** (resume **0x47650c**), scan the create request's name
(`EBX+2`, **24 bytes**) and remark (`EBX+0x1b`, **64 bytes**), then
replay `call CGuildLock::CGuildLock` (0x42b800). Both final terminators
were already forced by the original handler. Filtering precedes duplicate-name
lookup and every durable copy.

The remark-update hook at **0x477102** replays `add ebx,2; mov ecx,ebx`,
scans the resulting `EBX` for **64 bytes** or NUL, then resumes at
**0x477107**. This path already **rejects** quotes at 0x477110; the patch
normalizes them before that check so the sanitized remark is accepted.
Both scans preserve registers/flags. The DB agent's `DBGuild::CreateGuild`
uses `sprintf` at 0x4281b5 for `usp_Create_Guild2_E`; `UpdateRemark`
uses it at 0x42888f for `usp_Save_GuildRemark_E`.

**Status:** verified against the pristine executable, disassembly and x86 emulation; live test pending.

| virtual address | file offset | before | after |
|---|---|---|---|
| 0x476507 | 0x076507 | `e8f452fbff` | `e93f9f0a00` |
| 0x52044b (cave) | 0x12044b | `000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000` | `9c608d7302b918000000803e00740b803e277503c6062046e2f0619d9c608d731bb940000000803e00740b803e277503c6062046e2f0619de878b3f0ffe97f60f5ff` |
| 0x477102 | 0x077102 | `83c3028bcb` | `e986930a00` |
| 0x52048d (cave) | 0x12048d | `00000000000000000000000000000000000000000000000000000000000000000000000000` | `83c30289d99c608d33b940000000803e00740b803e277503c6062046e2f0619de9556cf5ff` |

### `gift-target-filter` — Gift target names replace quotes with spaces

Hook **0x479f4a**, resume **0x479f4f**. Scan `EBP=request+0x17`
for **20 bytes** or NUL, preserving registers and flags; the original handler
already forced `request[0x2b]=0`. Replay `strncpy` (0x504c60) with
its original three stack arguments. This filters the response copy and the
later `0x0e03` database request. In `ps_dbAgent`,
`DBCharacter::SaveGiftPointItem` formats this target in the gift statement
at 0x41f287 and notification statement at 0x41f5ec.

**Status:** verified against the pristine executable, disassembly and x86 emulation; live test pending.

| virtual address | file offset | before | after |
|---|---|---|---|
| 0x479f4a | 0x079f4a | `e811ad0800` | `e963650a00` |
| 0x5204b2 (cave) | 0x1204b2 | `0000000000000000000000000000000000000000000000000000000000000000000000000000` | `9c608d7500b914000000803e00740b803e277503c6062046e2f0619de88d47feffe9779af5ff` |

### `gm-notice-filter` — GM notice log text replaces quotes with spaces

Hook **0x471ca8**, resume **0x471cad**, in the three-argument
`CUser::SendLogAdmin` (0x471bd0). The cave replays the load of `CL`
from `[EAX]`, substitutes a space for a quote, and replays the store to
`[EDX+EAX]`, preserving EFLAGS. The original add/test/back-edge then
processes the next byte; the NUL and the source-based packet length computation
remain intact. No source text is rewritten. This covers every detail string
in the `0x0c03` log packet, including notices. The other two overloads and
the log's command/target/header fields are outside this fix.
`ps_gameLog::CConnection::WriteLogAdmin` (0x404820) passes the detail to
quoted `%s` in `EXEC usp_Insert_Action_Log_E`, then queries at 0x4049f5.

**Status:** verified against the pristine executable, disassembly and x86 emulation; live test pending.

| virtual address | file offset | before | after |
|---|---|---|---|
| 0x471ca8 | 0x071ca8 | `8a08880c02` | `e92be80a00` |
| 0x5204d8 (cave) | 0x1204d8 | `00000000000000000000000000000000000000` | `9c8a0880f9277502b120880c029de9c217f5ff` |

## Pass 2 (clear-bug fixes, applied by default)

Nine more fixes, appended after the earlier caves (0x5204eb–0x5207b5), so every
earlier fix keeps its distributed bytes and addresses. Each was assembled at its
final address, written into a copy, disassembled again, and checked against the
surrounding code, its jump targets and the stack. A test also checks that no
branch anywhere in `.text` lands inside a hooked range. Each cave was then
executed in Unicorn against crafted state and compared with an independent
Python model, and with the original bytes on every path the two should share
(`test_pass2.py`). Nothing was run against a live server. No new search of the
community forums was made for this pass: GoldBugFix (below) is the one community
fix already known for any of these bugs.

### `myshop-price-wrap` — Personal-shop prices that wrap past 2^32 are refused

- **Symptom:** Buying from a personal shop (`0x230D`) multiplies count by the
  slot's price with a 32-bit `imul` (`CUser::PacketMyShop` 0x4790f0). A price of,
  say, 0x80000001 bought twice costs 2 gold.
- **Fix:** an unsigned `mul`. When the product needs more than 32 bits, the
  answer is result 2 ("Insufficient gold"), through the handler's own refusal at
  0x479116, which builds its packet from scratch and reads no register. Otherwise
  the product goes where the original keeps it (`edx`) and the handler goes on
  unchanged.
- **Status:** verified in disassembly and emulation (12 count/price pairs at
  and around the 2^32 boundary, against the original's product).
  **Credit:** Shaiya Afterglow analysis (docs/original/personal-shop.md section 5); the
  same refusal as ScreaM's `GoldBugFix.CT` for NPC shops.
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x4790ed | 0x0790ed | `0fb6d10faf948344160000` | `e9f9730a00909090909090` |
  | 0x5204eb (cave) | 0x1204eb | `000000000000000000000000000000000000000000000000… (25 bytes)` | `0fb6c98b848344160000f7e10f82198cf5ff89c2e9f48bf5… (25 bytes)` |

### `npc-shop-price-wrap` — NPC-shop prices that wrap past 2^32 are refused

- **Symptom:** `CUser::PacketShop` (`0x0702` buy) has the same 32-bit product at
  0x46b55e (count × `nBuy`).
- **Fix:** as above, with the not-enough-money answer, result 1, at 0x46b57e. That
  path reads the item definition from `ebp` and the opcode word at `[esp+0x20]`,
  which the cave sets as the original sets them before the test. `edx` is
  preserved.
- **Status:** verified in disassembly and emulation. **Credit:** `GoldBugFix.CT`
  by ScreaM (the same refusal, made in the running process with Cheat Engine);
  this is an on-disk version written from our own disassembly, nothing copied.
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x46b55a | 0x06b55a | `0fb643070faf8680000000` | `e9a54f0b00909090909090` |
  | 0x520504 (cave) | 0x120504 | `000000000000000000000000000000000000000000000000… (35 bytes)` | `520fb64307f7a6800000005a7205e94eb0f4ff8b6c241066… (35 bytes)` |

### `remake-distinct` — Remaking needs three different items

- **Symptom:** `0x0807` (`CUser::ItemRemake` 0x4608b0) never checks that its three
  places differ. One item named three times becomes a new item with fresh craft
  stats, and its object is freed three times.
- **Fix:** before the first make-type test, compare the three item pointers
  (`esi`, `ecx`, `edx`). If two are the same, take the handler's silent exit
  (0x461013, the same exit as for an empty place). The displaced
  `mov bl,[esi+0x68]; cmp bl,0x51` runs after the check, so the original `je`
  sees its own flags.
- **Status:** verified in disassembly and emulation. **Credit:** Shaiya Afterglow
  analysis (docs/original/lapis.md section 7).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x460983 | 0x060983 | `8a5e6880fb51` | `e99ffb0b0090` |
  | 0x520527 (cave) | 0x120527 | `000000000000000000000000000000000000000000000000… (35 bytes)` | `39ce0f84e40af4ff39d60f84dc0af4ff39d10f84d40af4ff… (35 bytes)` |

### `enchant-seed` — Each enchant roll gets its own seed

- **Symptom:** `CUser::ItemLapisianAdd` builds a fresh `SRandom` (a Mersenne
  Twister) on the stack for every enchant (`SRandom::SRandom` 0x4a8840 at
  0x45fc82) and seeds it with `time(0)` xor the session id (0x45fc87–0x45fc9b).
  Two enchants by one player in the same second roll the same number, and the
  roll can be worked out from the clock.
- **Why not just drop the reseed:** nothing else seeds this generator. Monsters
  keep their own at `CMob+0x2bc`, seeded at creation. Without the call, the
  constructor seeds lazily with `time(0)` alone, so every player enchanting in
  the same second would roll the same.
- **Fix:** keep the per-enchant seed and xor in the CPU cycle counter (`rdtsc`).
  `eax` and `edx`, which `rdtsc` writes, are dead at that point (`SetRandomSeed`
  takes the seed in `edi`). The rates and the `% 1000000` roll are unchanged.
- **Status:** verified in disassembly and emulation (the seed equals
  `time ^ session ^ tsc`; two runs give different seeds; the other registers are
  kept). **Credit:** Shaiya Afterglow analysis (docs/original/lapis.md section 5).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x45fc8e | 0x05fc8e | `8bf833bb5c570000` | `e9b7080c00909090` |
  | 0x52054a (cave) | 0x12054a | `0000000000000000000000000000000000` | `89c733bb5c5700000f3131c7e93bf7f3ff` |

### `quest-end-npc` — Quests that name an end NPC are handed in at it

- **Symptom:** `CUser::QuestEndChkItemPick`, `QuestEndChkKillMOB` and
  `QuestEndChkKillPC` (0x47f670–0x47f740) look the end NPC up only when the
  packet's NPC id is not 0. A `0x0903` with id 0 finishes item, monster and
  player-kill quests anywhere.
- **Fix:** in `CUser::QuestEndRequest`, on the branch for quests with an end NPC
  (`byEndNPCType` ≠ 0, the jump table at 0x47f301), NPC id 0 (`[esp+0x18]`)
  takes the function's silent exit (0x47f358), as an unknown NPC does. Quests
  without an end NPC take the other branch and are unchanged.
- **Client:** the patch assumes an unmodified client always names the NPC when
  handing in. The new server enforces the same rule, and a kill quest with an end
  NPC (quest 155, NPC 8/47) was handed in through the real client on it
  (seen in play). Not every quest kind has been tried that way.
- **Status:** verified in disassembly and emulation. **Credit:** Shaiya Afterglow
  analysis (docs/original/quest.md sections 7 and 11).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x47f2fc | 0x07f2fc | `83f8037757` | `e95a120a00` |
  | 0x52055b (cave) | 0x12055b | `000000000000000000000000000000000000000000000000… (25 bytes)` | `837c2418000f84f2edf5ff83f8030f87e9edf5ffe98dedf5… (25 bytes)` |

### `chat-markup-filter` — Malformed chat colour markers are broken up

- **Symptom:** the client renders `<<` + 3 digits + `c` + 2 digits as a
  colour/link marker for the next NNN bytes. A marker whose NNN runs past the text
  (`<<999c01`) crashes every client that displays the line, and the original
  server relays it (docs/client/client-packets.md, "Chat markup crash").
- **Fix:** one shared routine (cave 0x520574) walks a text up to its first NUL or
  a limit. It keeps each well-formed marker with 1 ≤ NNN ≤ the bytes after it,
  and turns the first `<` of any other `<<` into a space. The byte count and the
  NUL stay as they are, so lengths computed later still agree. Valid markers (the
  client's own `/chatcolor`, item names) pass unchanged. The routine preserves
  every register and the flags. It is called from five hooks, each cleaning the
  text in place in the received packet before anything copies it:

  | hook | path | text |
  |---|---|---|
  | 0x47052f, `PacketChatE` dispatch | `1101`–`1105`, `1107`, `1108`, `1111`, `1112` | `u8 len` at +2, text at +3, len ≤ 0x80 |
  | | `1102` whisper | len at +0x17, text at +0x18 |
  | 0x470f5a, `PacketAdminChat` dispatch | `F101`, `F103`–`F105`, `F108`, `F10A`; `F102` | as `11xx`; as the whisper |
  | | `F106` (GM line to everyone) | `u16 len` at +2 (≤ 0x7D0), text at +4, the last byte excluded (the handler makes it the NUL) |
  | 0x4739d6, `PacketAdminCmdD` dispatch | notices `F906`–`F90B` | the length at +2, +4, +0x19 (`F908`) or +5 (`F90A`), the text after it, ≤ 0x80 |
  | 0x4758a6, `PacketComm` dispatch | memos `0x2206`, `0x220B` | at +7, 50 bytes (byte 57 is forced to NUL by the case) |
  | 0x478d0a, `PacketMyShop` `0x2303` open | the shop title (also sent to players arriving later) | at +3, 120 bytes |

  Other opcodes, and lengths the handler would refuse anyway, are left alone.
  The memo hook replaces the switch's own indirect jump: the flags differ at the
  switch targets, and every target writes them before reading them (checked).
- **Not covered:** guild remarks and guild names (the guild window; whether it
  uses the marker parser is a client question, and both already pass through
  `guild-text-filter`'s hooks); character names (the name rules); scripted
  monster lines (`0x1109`, `0x110A`: server data, not client text). Whether the
  memo and shop-title windows parse markers at all is not confirmed from the
  client; the filter there is a precaution and harmless either way. Only the
  chat crash itself was seen live.
- **Status:** verified in disassembly and emulation (19 text cases × every
  opcode and layout above, plus untouched opcodes and over-long lengths; text
  compared with an independent model, registers and flags with the original
  bytes). **Credit:** Shaiya Afterglow analysis (the client-side finding in
  docs/client/client-packets.md).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x520574 (cave) | 0x120574 | `000000000000000000000000000000000000000000000000… (126 bytes)` | `9c6089f785c97473803f00740347e2f88d460139f8736466… (126 bytes)` |
  | 0x47052f | 0x07052f | `0fb70605ffeeffff` | `e9be000b00909090` |
  | 0x5205f2 (cave) | 0x1205f2 | `000000000000000000000000000000000000000000000000… (76 bytes)` | `51560fb7063d0211000075090fb64e1783c618eb1b2d0111… (76 bytes)` |
  | 0x470f5a | 0x070f5a | `0fb70705ff0effff` | `e9dff60a00909090` |
  | 0x52063e (cave) | 0x12063e | `000000000000000000000000000000000000000000000000… (109 bytes)` | `515689fe0fb7072d01f1000083f809774d83f80175090fb6… (109 bytes)` |
  | 0x4739d6 | 0x0739d6 | `0fb70305ff06ffff` | `e9d0cc0a00909090` |
  | 0x5206ab (cave) | 0x1206ab | `000000000000000000000000000000000000000000000000… (73 bytes)` | `51560fb7032d06f9000083f805772b8d730285c074138d73… (73 bytes)` |
  | 0x4758a6 | 0x0758a6 | `ff248580624700` | `e949ae0a009090` |
  | 0x5206f4 (cave) | 0x1206f4 | `000000000000000000000000000000000000000000000000… (34 bytes)` | `83f804740583f809751151568d7307b932000000e867feff… (34 bytes)` |
  | 0x478d0a | 0x078d0a | `8d46038db578160000` | `e9077a0a0090909090` |
  | 0x520716 (cave) | 0x120716 | `000000000000000000000000000000000000000000000000… (30 bytes)` | `8d4603565189c6b978000000e84dfeffff595e8db5781600… (30 bytes)` |

### `guild-bank-bound` — Bound items cannot enter the guild bank by a swap

- **Symptom:** taking an item out of the guild bank onto a taken bag place
  (`CGuild::ItemGuildBankToBag`, the swap branch from 0x42fff1) puts the bag's
  item into the bank without the deposit's checks. A bound (`byOG` ≠ 0) or quest
  (real type 27) item then sits in the bank for another member to take.
- **Fix:** before the swap, apply the deposit's two checks to the bag item. If
  either fails, release the guild's lock (the critical section kept at
  `[esp+0x2c]`, as the function's own money refusal does) and return without an
  answer, as a refused deposit does.
- **Status:** verified in disassembly and emulation (empty place, normal, quest,
  bound; the lock released exactly once on refusal, the stack balanced).
  **Credit:** Shaiya Afterglow analysis (docs/original/guild-bank.md section 13).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x42ffeb | 0x02ffeb | `0f8444020000` | `e944070f0090` |
  | 0x520734 (cave) | 0x120734 | `000000000000000000000000000000000000000000000000… (46 bytes)` | `0f84fbfaf0ff8b0e8b493083b9a00000001b740b80793100… (46 bytes)` |

### `gm-target-filter` — GM log target names replace quotes with spaces

- **Symptom:** the two- and three-argument `CUser::SendLogAdmin` (0x471ae0,
  0x471bd0; 14 and 26 call sites) copy the target name a GM typed into
  `0x0C02`/`0x0C03`. `ps_gameLog` puts it into quoted SQL text unescaped
  (`WriteLogAdmin` 0x404820).
- **Fix:** the same in-copy replacement as `gm-notice-filter`, at the target copy
  loops (0x471b94, 0x471c94). The source stays as it is (it is the command's own
  argument), and EFLAGS and the loop's NUL test are kept.
- **Status:** verified in disassembly and emulation (single bytes under four
  flag states; whole names through the original loop). **Credit:** Shaiya Afterglow
  analysis (docs/original/game-log.md section 3.8).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x471b94 | 0x071b94 | `8a08880c02` | `e9c9eb0a00` |
  | 0x520762 (cave) | 0x120762 | `00000000000000000000000000000000000000` | `9c8a0880f9277502b120880c029de92414f5ff` |
  | 0x471c94 | 0x071c94 | `8a08880c02` | `e9dcea0a00` |
  | 0x520775 (cave) | 0x120775 | `00000000000000000000000000000000000000` | `9c8a0880f9277502b120880c029de91115f5ff` |

### `exchange-count-guard` — A trade or duel stake that shrank is skipped

- **Symptom:** at the end of a trade, `CUser::ExchangeItemAdd` reads each offered
  bag place again. When the count offered differs from the count there, it makes
  a new item of the offered count for the receiver and subtracts that count from
  the giver's byte count (0x46fb3b) without checking that many are still there.
  A count lowered after the offer wraps round (3 − 5 = 254): the receiver gets
  the offered count and the giver keeps 254. The duel stakes window
  (`CUser::ExchangePvP`, 0x47cd29) has the same code.
- **Fix:** when the count offered is above the count now in the place, skip that
  slot, as the original skips one whose item is gone (trade 0x4700e6, duel
  0x47d052). Equal still moves the whole item and smaller still splits, as
  before. This matches the new server's trade deviation 7.
- **What triggers it:** most item actions end a trade first (`CancleActionExc`)
  or empty an offered slot (`ExchangeItemRemove`). One path is in the code
  (confirmed, not tried): a recall or movement rune's move is carried out 5 s
  after the use, by `CUser::UpdateResetPosition`, which takes the rune with
  `CUser::ItemUseNSend` (0x48d73f, 0x48d980, 0x48db6f, 0x48dd55); that lowers the
  stack's count (0x465250) without looking at a trade, and `PacketExchange`
  neither reads nor cancels a pending move (only `PacketPC`, `UpdateAttack` and
  `StatusCastingStop` call `CancleMove`). So a stack of runes offered in a trade
  opened during the cast is one short when the exchange runs. Trying it against
  the original is not worth it: the guard closes the wrap whatever the path.
- **Status:** verified in disassembly and emulation (both sites; equal,
  smaller, larger and boundary counts; the original goes on to split where the
  patch skips). **Credit:** Shaiya Afterglow analysis (docs/original/trade.md sections
  3 and 6, docs/original/duel.md section 4).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x46f9db | 0x06f9db | `8a47023a45420f853a010000` | `e9a80d0b0090909090909090` |
  | 0x520788 (cave) | 0x120788 | `0000000000000000000000000000000000000000000000` | `8a47023a45420f8453f2f4ff0f874cf9f4ffe982f3f4ff` |
  | 0x47ccbb | 0x07ccbb | `8a4b023a4d42754c` | `e9df3a0a00909090` |
  | 0x52079f (cave) | 0x12079f | `0000000000000000000000000000000000000000000000` | `8a4b023a4d420f8418c5f5ff0f87a1c8f5ffe959c5f5ff` |

### What pass 2 does not fix

- **Item mall points** (`docs/original/item-mall.md` section 9, oddity 1): not
  patchable in `ps_game` alone. The game hands over the items at once and takes
  the price from its own held copy of the points. ps_dbAgent queues the database
  write at once too (job 0x12, posted at 0x406fe2 in its `0x0E02` case). When
  that write fails because the account's points are short in the database
  (another world, or points changed outside the game while the player is on),
  ps_dbAgent rolls back and drops the character on its side (job 9 →
  `CUser::SaveExitCharacter`) without telling `ps_game`. The player keeps playing
  with items that are no longer saved. A fix needs a failure answer from
  ps_dbAgent and a handler for it in `ps_game`: a new message flow, not a byte
  patch. Mitigation: one world per point pool, and no point changes from outside
  the game while the account is online.
- **Trade, half-written exchanges:** each item and the money go to ps_dbAgent as
  separate messages. Making them one transaction is a change to the database
  agent's protocol; not patchable here.
- **Trade, both pressing ready at once** (docs/original/trade.md section 6): not
  reachable (confirmed). The network threads only
  queue packets (`SSyncQueueBuffer` at `CUser+0x7c`). The zone's world thread
  handles them one after another (`CWorldThread::Update` → `CZone::UpdatePackets`
  → `CUser::UpdatePackets` → `OnRecv`, under the thread's lock), and two traders
  are always in one zone: the partner is found in the requester's zone
  (`CZone::FindUser`, 0x46ece0) and leaving the zone resets both sides'
  exchange (`CUser::LeaveZone` 0x44af6e to 0x44b01a). A zone belongs to one
  world thread (confirmed): each zone is put on exactly one thread's list, at
  start (`CServer::InitWorldThread` 0x406b4c to 0x406c5e) or when an instance
  zone is made (`CServer::AddWaitZone` 0x406880, round robin, taken in by
  `CWorldThread::Update` 0x4073b8), and leaves it only when freed (0x407b97).
- **The game log server:** no second patcher. Of what `ps_game` sends, every
  client-controlled text is now filtered or escaped: chat by the log writer
  (`'` → `_`), the whisper target by being the found player's own name, GM details
  and targets by `gm-notice-filter`/`gm-target-filter`, character and guild names
  at creation by the input filters. Names already stored with a quote, and
  account names (from the operator's registration), are not; check the database
  once, e.g.
  `SELECT CharName FROM PS_GameData.dbo.Chars WHERE CharName LIKE '%''%'`. Keep
  `ps_gameLog`'s port private, since a direct connection bypasses `ps_game`.

## Pass 3 (clear-bug fixes, applied by default)

Two more fixes, appended after pass 2's caves (0x5207b6–0x520812), so every earlier
fix keeps its distributed bytes and addresses. Same method as pass 2: assembled at
the final address, written into a copy, disassembled again, checked against the
surrounding code, executed in Unicorn against crafted state and compared with an
independent Python model and with the original bytes on every path the two should
share (`test_pass3.py`, which also checks that no branch lands inside a hook and
that each fix applies and reverts alone and on top of the earlier kit). Nothing was
run against a live server.

### `party-kick-leader` — Kicking a party member keeps the leader and the loot turn

- **Symptom:** `CParty::ExpelUser` (0x447120) moves the later members down and
  drops the count but, unlike `CParty::LeaveNormalParty` (0x446960), leaves the
  leader's slot (`+0xc`) and the loot turn counter (`+0x108`) alone. Kicking a
  member who came before the leader makes the next member (or a stale copy of the
  last pointer) the server's leader while every client still shows the old one. A
  leader kicking itself makes the next member leader the same way.
- **Fix:** two hooks. At 0x44718d, once the kicked member's slot is found, a slot
  equal to the leader's takes the function's "not found" exit (0x44716f: the lock
  is released, nothing is sent, the caller ignores the result). At 0x4471d4, before
  the count drops, the loot turn goes down by one when it is above the kicked slot
  plus one, and the leader's slot goes down by one when the kicked slot is before
  it, as `LeaveNormalParty` does. The displaced `add`/`mov` run last so the `jne`
  after the hook still sees the count's flags. Scratch registers eax and ecx are
  dead at both points; the flags the first cave sets are dead (a call follows).
- **Client:** none needed. The client only sees the same `0x0B06` it always got;
  a kick of the leader by itself now gets no answer, as any refused kick does.
- **Status:** verified in disassembly and emulation (every count 2–6, slot,
  leader and turn). **Credit:** Shaiya Afterglow
  analysis (docs/original/party.md section 4).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x44718d | 0x04718d | `8b87dc000000` | `e924960d0090` |
  | 0x5207b6 (cave) | 0x1207b6 | `0000000000000000000000000000000000000000… (20 bytes)` | `3b5d0c0f84b069f2ff8b87dc000000e9c969f2ff… (20 bytes)` |
  | 0x4471d4 | 0x0471d4 | `834510ff8b4510` | `e9f1950d009090` |
  | 0x5207ca (cave) | 0x1207ca | `000000000000000000000000000000000000000000000000… (43 bytes)` | `8b8d080100008d430139c17e0749898d080100008b450c39… (43 bytes)` |

### `repair-shield-slot` — Repairing one item updates that item's saved quality

- **Symptom:** `CUser::ItemRepair` (0x464340), one item: after the quality is set it
  also writes the new quality into the saved-quality copy (`+0x1aa`, one `u16` per
  slot) at the index the gem loop left in `edx`, always 6 (the shield), whatever
  was repaired. The shield's own wear may then be skipped when the character
  leaves, or saved when it did not change; the repaired slot's copy stays old.
  (The repair itself, its price and the `0x070A` save are right.)
- **Fix:** one hook at 0x4649cf. In the cave the write goes to the repaired slot
  (`[esp+0xdc]`) and only for the equipped slots (bag 0); a bag item has no copy
  and the old code wrote the shield's. `test ebx,ebx` runs again at the end so the
  `jne` after the hook sees the bag's flags; `edx` is overwritten afterwards on
  both paths.
- **Client:** none needed; nothing is sent differently.
- **Status:** verified in disassembly and emulation (bag 0 and others, several
  slots and qualities, against the original). **Credit:** Shaiya Afterglow analysis (docs/original/party.md section 4)
  this build (docs/original/repair.md section 5).
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x4649cf | 0x0649cf | `66897e446689bc55aa010000` | `e921be0b0090909090909090` |
  | 0x5207f5 (cave) | 0x1207f5 | `000000000000000000000000000000000000000000000000… (30 bytes)` | `66897e4485db750f8b9424dc0000006689bc55aa01000085… (30 bytes)` |

## Design-choice fixes (opt-in, applied only when named)

These are not bug fixes everyone will want; they change how the game plays, matching
choices the new server made. Apply them by id if you want them.

### `fire-thorns-aggro` — Fire Thorns / Mass Taunt aggro works

- **Symptom:** A self buff with an aggro value should anger the monster the caster is fighting, but the server only does it when the caster is not attacking, which an instant skill never is.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/skills-buffs.md section 4)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x450ca5 | 0x050ca5 | `7432` | `9090` |

### `op-exclusive` — Hammers of 1000% or more always link

- **Symptom:** The staff Linking Hammer multiplies the rate like any hammer and the 50% cap then holds it at 50%, though it was meant to be certain.
- **Status:** verified in disassembly. **Credit:** Shaiya Afterglow analysis (docs/original/lapis.md section 3)
- **Bytes:**

  | virtual address | file offset | before | after |
  |---|---|---|---|
  | 0x4616fa | 0x0616fa | `83fb327e05bb32000000` | `e972ec0b009090909090` |
  | 0x520371 (cave) | 0x120371 | `000000000000000000000000000000000000000000000000… (31 bytes)` | `81fbe80300007c07bb64000000eb0a83fb327e05bb320000… (31 bytes)` |

## Not in this version

The following original-server problems are known and analysed but not patched here
yet. They are listed in `docs/original/community-fixes.md`; a later pass adds them to the kit.

- **NPC-range checks** — merchant, blacksmith, warehouse and quest handlers act with
  the NPC only on the same map, not in view. A range check needs a distance threshold
  that should be confirmed against a running client before shipping.
- **The rune-after-skill delay** (`docs/original/community-fixes.md`, "Skill-cut with
  runes"). The rule needs the time of the player's last skill request kept per
  player, and no field of the player object is proven unused (the structure is
  0x5eb0 bytes; its last 0x18 are a critical section, and the dynamically indexed
  arrays hide the gaps). Cups' table keeps it in a field of its own build.
  Refusing the rune while the player's next attack time has not come would stop
  it, but also after every normal blow, which changes more than the bug.
  (The price wraps, remaking from one item and the per-enchant reseed were added
  in pass 2; the party-kick leader slip and the repair's saved-quality slot in
  pass 3.)
- The item mall's points and the trade's half-written exchanges: see "What pass 2
  does not fix" above.

## Rebuilding the byte lists

`fixes.json` is generated from `build_fixes.py`, which assembles each cave with the
GNU assembler at its final address (so the jumps are right) and checks every "before"
against the real build. It is a development tool; applying the patches does not need
it. With a copy of the build present:

```
python3 build_fixes.py ps_game.exe     # regenerate and verify fixes.json
PS_GAME_EXE=/path/to/ps_game.exe python3 -m unittest -v test_patch_ps_game test_input_filters test_pass2 test_pass3
```

The behavioral tests additionally need the Python `unicorn` package. They execute
only the selected x86 instructions in emulated memory, never launch a server or
connect to a database. Without the exact executable the binary tests skip; without
Unicorn the emulation tests skip. A skipped test is not validation.

Input-filter verification covers both character hooks, both guild hooks, gift
target copying and the full GM detail loop. Fixtures include plain text, quotes at
each boundary, repeated quotes, empty strings, embedded NULs, maximum widths and
high bytes. Canary checks bound writes, and register/EFLAGS/stack comparisons check
the displaced instructions. The GM tests also run the original source-length
calculation and check the pushed packet size. The round-trip suite applies and reverts every fix,
the default set, and mixed selections.

Validation: the combined command above passed **14 tests,
no skips**, using the exact SHA-256 build and Unicorn 2.1.4. Rebuilding
`fixes.json` against that executable is deterministic; all original 17 entries
retain their previously distributed bytes and addresses.

Validation (pass 2): the combined command passed **34 tests,
no skips** (exact SHA-256 build, Unicorn 2.1.4, GNU binutils `objdump`). The
rebuilt `fixes.json` keeps all 21 earlier entries byte for byte; the nine new
ones follow them in the cave. Pass 2 adds checks that no branch in `.text` lands
inside a new hook, that the caves stay in the region without overlapping, that
each new fix applies and reverts alone and on top of the earlier kit, and an
emulation test for every new cave.

### Live acceptance (not done)

From the kit directory, the selective apply command is:

```
python3 patch_ps_game.py apply /path/to/spare/ps_game.exe character-name-filter guild-text-filter gift-target-filter gm-notice-filter
```

On an isolated copy of this exact server build, apply the four filters (or the
default clear-bug set), then check character create and rename, guild create and
remark update, a gift, and each notice command with plain and quoted text.
Verify normalized values in the intended database rows, matching client replies,
a successful gift to the normalized recipient, and a complete GM log row whose
detail has spaces while the displayed notice retains its original text. Check
normal and maximum-length inputs and revert the copy to the recorded SHA-256.
Existing quotes in stored names need separate cleanup before this test. No original
server file or database was changed for the static/emulated validation.

## Other episodes

The byte lists here are for the EP4.5 `ps_game.exe` only; the EP3 (linked 2008-01-17) and EP5.4 (2011-09-14) builds are other images
and the kit does not apply to them. Which of the fixed bugs those builds still have is in
[`community-fixes.md`](community-fixes.md#other-episodes); overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).
