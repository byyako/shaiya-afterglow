# Parties: inviting, joining, leaving, the leader, member status, shared experience and loot

This page covers the normal party (up to 7 players) in the EP4.5 game server `ps_game.exe`, and everything it changes
elsewhere: experience, drops and gold, party heals and buffs, rebirth next to the leader, party chat. It was read with the decompiler and
confirmed in the disassembly where noted. The client side (what `game.exe` does with each `0x0Bxx` and `0x0Cxx` packet, the member unit,
the yes/no dialog conventions) is on [Social packets](../client/client-social-packets.md).

The union ("raid") party (`0x0B0B` to `0x0B1A`, `0x0C0A` to `0x0C14`) is on [Union parties](union.md). On other pages: the party
search (`0x22xx` `FINDPARTY`; [Friends](friends.md)), the party summon (`m_dwPartyCallReqID`, `TP_PARTYCALL_*`;
[Using items](item-use.md#9-moves-by-item)), quest kill counts of party members ([Quests](quest.md#6-progress)), and game
logs ([The game log](game-log.md)).

## In short

- **Up to seven players**, all of the same faction and on the same map when they join. The leader invites; anyone can leave; the leader
  can kick or hand over the lead.
- **Members see each other's health and buffs** wherever they are, and their position when on the same map.
- **Experience is shared** among the members within 100 units of the killer. Each member's share depends on the party's size and on how
  far below the highest-level member they are. A full party of seven gets double experience.
- **Loot is handed out in turn.** Each item drop goes straight into the bags of the next member in range. Gold is split evenly among the
  members in range.
- **Bugs in the original:** kicking a member can silently move the leadership; the full-party bonus is
  lost on a gain that levels a player up.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's (see [NPC shops](shop.md) on the
PDB's 4-byte shift).

## 1. The party

Confirmed, `CParty` 0x130 bytes, `CPartyMgr`.

| field | offset | |
|---|---|---|
| `m_dwID` | +0x08 | party id (`CPartyMgr` counter; a recycled party object keeps its old id) |
| `m_nBothIndex` | +0x0C | **the leader's index** in the member array |
| `m_nNumUser` | +0x10 | member count |
| `m_pParty[30]` | +0x14 | `{int Index, CUser* pUser}`; a normal party uses slots 0 to 6, in joining order |
| `m_nItemDivType` | +0x104 | loot rule: 1 in turn, 2 random, 3 in turn on the ground, 4 the leader (section 10) |
| `m_nItemDivSeq` | +0x108 | whose turn it is (rule 1 and 3) |
| `m_nMaxUserLevel` | +0x10C | the highest member level (section 9) |
| `m_nSubBothIndex`, `m_bRaidPartyFlag`, `m_bAutoJoinFlag` | +0x110.. | union only |

`CPartyMgr::CreateParty(user, rule)` (0x4474b0): a party with `m_nItemDivType = rule`, the creator entering first (section 3).
**Every normal party is created with rule 1** (`PacketParty` passes 1), and only a union leader can change it (`0x0B0F` requires the union
flag): **in a normal party, loot is always handed out in turn.**

On the player: `m_pParty` (code +0x173C), `m_dwPartyReqID` (+0x1740, the inviter of a pending invitation), `m_dwTimeOverParty` (+0x5834, its
deadline), `m_bPartyFindReg` (+0x1744). "Busy" in the checks below is **a personal shop open** (`m_MyShop.nStatus`, +0x15E0), which
the new server does not have.

**Party type** (`CUser::GetPartyType` 0x48a330): **0 no party, 1 member, 2 leader.** It goes out as **`0x0520`** (7 bytes:
`u32 player id, u8 type`, `CUser::SendPartyType` 0x482810) to the player's view (`CZone::SendView`, 5 × 5 cells), and in the
`0x0303` shape reply (`party_type`).

## 2. Inviting

Confirmed, `CUser::PacketParty` 0x4676c0, cases 0x0B02 and 0x0B09.

Failures go to the inviter as **`0x0B03`** (`CUser::SendPartyError` 0x467660, 7 bytes: `u8 code`, `u32 id`). **The original fills only the
code: the id is whatever was on the stack**, so the client's "<p> rejected …" and "<p> cannot be invited …" (codes 0 and 1) show only by
chance (`docs/client`: they need an id of a player in view).

**`0x0B02`** (6 bytes, `u32 target id`), in this order:

1. the target is the inviter itself: **4**;
2. the inviter is busy: **1**;
3. the inviter is in a party and is not its leader, or the party has 7: **2**;
4. the target is looked up **in the inviter's zone** (`CZone::FindUser`, the same map): not there, or of the other country: **4**;
5. the target is in a party or has an invitation pending: **5** when the inviter is in that same party, else **3**;
6. the target is busy: **1**;
7. otherwise the invitation is pending: target's `m_dwPartyReqID` = the inviter's id, deadline **now + 30 s**, and the target gets
   **`0x0B02`** (`u32 inviter id`).

**`0x0B09`** (23 bytes, `char name[21]`; the server forces the last byte to 0): the target by name (`CWorld::FindUserZoneMove`); not
found: **4**; then **2** (step 3's party and leader test) and **1** (step 2, the inviter busy); the target is the inviter, **not in the same
zone** (`CUser::EqualZone`),
or of the other country: **4**; then 5 and 6 as above; the pending invitation as step 7, and the target gets **`0x0B09`** (23 bytes:
**the inviter's name**).

No check is made of the target being dead, hidden or transformed.

**Expiry** (`CUser::UpdateTimeover` 0x48e310, every second): a pending invitation past its deadline is dropped **silently** (the target's
dialog stays open; a later answer finds nothing pending and is ignored).

## 3. Answering and joining

Confirmed, case 0x0B03; `CParty::EnterNormalParty` 0x445fe0.

**`0x0B03`** from the invited player (7 bytes: `u8 answer` 0 yes / 1 no, `u32` its own id): ignored without a pending invitation or when
already in a party. The inviter is looked up **in the answerer's zone**; the invitation is cleared either way; inviter gone: nothing.

- Either of them busy: `0x0B03` **1** to the inviter.
- **No** (`answer` ≠ 0): `0x0B03` **0** to the inviter ("<p> rejected the party's invitation").
- **Yes**: an inviter without a party creates one (rule 1, the inviter entering first); an inviter that leads a party: the answerer enters
  it. Otherwise (the inviter is in a party but no longer its leader, or entering fails because the party is full): `0x0B03` **1** to the
  inviter.

**Entering** (`EnterNormalParty(user)`), refused when the party has 7:

1. The user takes the next slot (`m_pParty[count]`).
2. When others are already there: **`0x0B04`** (the user's member unit) to **the members already there**.
3. **`0x0B01`** to the user: `u8 leader index`, `u8 count` (the members already there, **not the user**), their units in slot order.
4. Count + 1; `m_bPartyFindReg` cleared; the highest member level recomputed (section 9).
5. **`0x0520`** (the user, its party type) to the user's view; a game log.

So a new party's creator gets `0x0B01` with count 0 (the client shows nothing), then the answerer gets `0x0B01` with the creator, and the
creator gets `0x0B04` with the answerer.

**The member unit** (`PARTY_USER_INFO_UNIT`, sent packed: **55 + 7 × n bytes**): `u32 id`, `char name[21]`, `u16 level`, `u8 job`,
`u16 max HP, HP, max SP, SP, max MP, MP`, `u16 map`, `f32 x, y, z`, `u8 n`, then n × (`u16 skill id`, `u8 skill level`, `u32 seconds
left`): the player's applied skills **without those with `byDontRemByDeath`**, at most 40, seconds left = `(end − now) / 1000`, at
least 0.

## 4. Leaving, kicking, the leader

Confirmed.

**`0x0B05`** (2 bytes): in a party, `CParty::LeaveUser` → **`LeaveNormalParty(user)`** (0x446960):

1. **`0x0B05`** (`u32 id` of the leaver) to **every member, the leaver included**; the leaver's party cleared.
2. The slots after it move down; the turn counter (`m_nItemDivSeq`) goes down by one when it pointed past the leaver's slot + 1.
3. Count − 1. **0**: the party is gone. **1**: the last member leaves too, by the same function (it gets `0x0B05` with its own id).
   Otherwise: **the leader left: slot 0 becomes leader**, and every member gets **`0x0B07`** (`u32 new leader id`); a member before the
   leader left: the leader index − 1.
4. A guild creation the leaver had started is cancelled (guilds: built, `docs/original/guild.md`; the new server ends a creation
   agreement only after 30 s or when a member leaves the world, its deviation 2); the highest level recomputed.
5. **`0x0520`** for the leaver (type 0) to its view, and for a new leader (type 2) to its view; a game log.

**Leaving the world** (`CUser::LeaveWorld` 0x44aad0: character screen, logout, the end of a lost connection's 10 s) **leaves the party
first**, as above. A party does not survive a relog.

**`0x0B06`** (6 bytes, `u32 id`): the sender must lead a party that has that member; `CParty::ExpelUser` (0x447120): **`0x0B06`** (the
kicked id) to every member, the kicked one included; its party cleared; slots move down; count − 1; 0: gone; 1: the last member leaves
(as `LeaveNormalParty`, `0x0B05` with its own id); the highest level; `0x0520` for the kicked player. **Original bug: the leader index and
the turn counter are not adjusted.** When the kicked member's slot is before the leader's, the index then points one member further on
(or, at the end, at a stale copy of the last pointer): another player silently becomes "leader" in the server's eyes while every client
still shows the old one. The leader kicking itself makes the next member leader silently the same way.

**`0x0B07`** (6 bytes, `u32 id`): the sender must lead the party, and `CParty::SetPartyBoth(id)` (0x445470) must find that member at
another slot: leader index = that slot; **`0x0B07`** (the new leader's id) to every member; `0x0520` for the new and the old leader to
their views. Any failure: **`0x0B07` with id 0** to the sender only (the client clears its leader silently).

Any other `0x0Bxx` opcode the switch does not know (`0x0B01`, `0x0B04`, `0x0B08`, `0x0B0A`, `0x0B0B`, `0x0B19` and up from `0x0B1B`)
**closes the connection** (`SConnection::Close(9)`). The union opcodes (`0x0B0D` to `0x0B18`, `0x0B1A`) need a union, except `0x0B0D`
(leave a union), which leaves any party.

## 5. Member status

Confirmed, : the senders and their call sites.

Each goes to **the other members** (`CParty::Send(except, packet)` 0x445680), not the player itself, whatever map they are on:

| packet | bytes | fields | sent when (original call sites) |
|---|---|---|---|
| `0x0C01` | 9 | `u32 id, u8 type, u16 value`: type **0 HP**, 1 SP, 2 MP | a monster's hit (`CUser::Damage`: HP); skill costs (`SendUseSPMP`: SP, then MP) |
| `0x0C02` | 9 | `u32 id, u8 type, u16 max` (the `0x050B` body) | one maximum changed (`SendMaxHP`, `SendMaxSP`, `SendMaxMP`) |
| `0x0C03` | 12 | `u32 id, u16 HP, SP, MP` | heals (`HealUser`), recovery (`SendRecover*`), damage over time, some monster hits, stat resets |
| `0x0C08` | 12 | `u32 id, u16 max HP, max SP, max MP` | all maxima at once (`SendEnergyAllMax`, stat resets) |
| `0x0C04` | 9 | `u32 id, u16 skill, u8 level` | a skill applied to the player, unless `byDontRemByDeath` (`SendSkillApply`) |
| `0x0C05` | 9 | the same | an applied skill taken off (`SendSkillRemoveParty`: expiry, cancel, dispel, death and the rest) |
| `0x0C06` | 6 | `u32 id` | the player died (`CUser::Death`, `DeathByNPC`); the client clears that member's buffs |
| `0x0C07` | 20 | `u32 id, u16 map, f32 x, y, z` | **only to members on the same map** (`CParty::SendMove` 0x4457f0): **every fourth accepted move** (the moves for which `PSendViewMove` also reached the ring to 120; disassembly at 0x46a553), and a move within a map or to another map (`CZone::MoveUserLocal`, `CWorld::ZoneLeaveUserMoveLocal`) |
| `0x0C09` | 8 | `u32 id, u16 level` | a level-up (`CUser::LevelUp`), to **every member, the player included** |

The order inside the unit and in `0x0C03`/`0x0C08` is HP, SP, MP (the server's own `0x0505` and `0x051F` are HP, MP, SP).

## 6. Party chat

Confirmed, `CUser::PacketChatE`.

**`0x1105`** from a player in a party with a length of 2 to 128: **`0x1105`** (`u32 id, u8 length, text`) to **every member, the sender
included** (`CParty::Send`). Without a party, or with another length: nothing. (`0x1112`, leader chat, is a union matter.)

## 7. Who killed the monster (verified, `CUser::AttackMob` at 0x44cf04, `CMob::DamageParty` 0x490e40, `CDamageMob::Add` 0x4b8dd0,
`CDamageMob::GetMaxDamage` 0x4b8ef0)

The damage record (`docs/original/combat.md` section 3 step 5) holds entries `{kind, id, damage, aggro, party id}`. In `CUser::AttackMob`
(normal blows and skill hits), **before** `CMob::Damage`, a player in a party also adds **a party entry**: kind 5, the party id, the
damage, no aggro (`DamageParty`: only while the monster is alive with HP above 0). The player's own entry (kind 1) carries the party id
too. Damage over time, Fire Thorns and fields do not go through `AttackMob` and add no party entry.

**`GetMaxDamage`**: the entry with the most damage (first on a tie). **When that is a party**, the killer of record is **the member with
the most damage of its own among the kind-1 entries carrying that party id**. So a party's damage counts together against a solo player,
and the party's top damage dealer stands for it.

## 8. Experience

Confirmed, `CMob::Death` 0x4910f0, disassembly 0x4912a8 to 0x491499; `CParty::GetPartyExpByLevel` 0x444e40.

The killer of record must be in the monster's zone (as without a party, `docs/original/combat.md` section 6). Without a party: `trunc(GetExp
× 1.2)` to the killer, as before. **With a party**, each member (party locked, slot order):

1. **in range** (`CUser::EnableApplyRange(member, killer, 100)` 0x45de60): the member alive, in the world, **in the killer's zone and map,
   within 100 of the killer** (horizontal, `SVector::Distance2`); the killer itself counts like any member;
2. `base = trunc(GetExp(member level) × share[count])`, **`GetExp` taken at the member's own level** (so monsters 6+ levels below a member
   give that member nothing), `count` = all members, in range or not;
3. `exp = trunc(base × gap(m_nMaxUserLevel − member level))`;
4. `CUser::AddExpFromUser(member, exp)`; quest kill counts (`docs/original/quest.md` section 6).

**No ×1.2 in a party.**

| members | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| `share` (0x5423a8, doubles) | 1.0 | 1.0 | 0.73 | 0.645 | 0.56 | 0.475 | 0.39 |

| `m_nMaxUserLevel − level` | below 8 | 8 to 15 | 16 to 31 | 32 to 47 | 48 and more |
|---|---|---|---|---|---|
| `gap` | 1.0 | 0.8 | 0.6 | 0.4 | 0.2 |

`m_nMaxUserLevel` (`CParty::ChkPartyExpByLevel` 0x444d80) is the highest member level, recomputed on joining, leaving, kicking and every
member level-up, **only while the party has 2 or more** (a party back to 1 member keeps its old value, but a 1-member party never lasts).

**The full-party bonus** (`CUser::AddExpFromUser` 0x458240): in a normal party of **7**, the gain is added **twice**: experience +
`2 × gain`, and `0x0207` carries `u32 gain, u32 bonus` (the second field, 0 otherwise). **Original quirk: when the gain levels the player
up, the bonus is not added** (the level-up path adds the gain once and sends no `0x0207`).

## 9. Loot (verified, `CMob::ItemDrop` 0x4a5ea0 and 0x4a65a0, `CMob::EnableApplyRangeItem` 0x4a64f0, `CUser::ItemGet(CItem*)` 0x45df40,
`CUser::ItemGetMoney` 0x45ec70)

`ItemDrop(monster, killer, killer's party)`. "In range" below (`EnableApplyRangeItem(100)`) is: the killer in the world and in the monster's
zone; the member alive, in the world, in the killer's zone and map, within 100 of the killer.

**Items** (the drop entries of `docs/original/combat.md` section 7, each rolled as before). Each item is created with owner = the killer
and **party id = the killer's party** (`CItem::m_dwPartyID`), at the drop spot. Then, with a party and an item that is not a quest item
(real type 27), by the loot rule — **rule 1 for every normal party**:

- **1, in turn**: starting at `m_nItemDivSeq` (wrapping to 0 past the last member), the first member in range **gets the item straight
  into its bags** (`ItemGet(item)`), and the turn moves past that member. Nobody in range: the item goes on the ground.
- 2 random member (in range, else the ground); 3 in turn, but on the ground owned by that member alone (party id 0); 4 the leader (union
  only, with a `wDex` test on the item info): not reachable in a normal party.

**`ItemGet(item)`** places the item as a pick-up does (`docs/original/combat.md` section 8: stacks first, then the first empty slot;
`0x0205` for each change). When something went in, the members other than the receiver **within 100 of it** get **`0x0B08`** (8 bytes:
`u32 receiver id, u8 type, u8 type id`: "<p> attained <i>"). What does not fit goes on the ground at the drop spot (`0x0401` kind 1,
60 s), with its owner and party id. A manual pick-up (`0x0205`) by a party member also sends `0x0B08` to the members within 100.

**Picking up** (`CZone::GetPickItem`): someone else's item is free to the members of the item's party (with the pick check on, the
default; `docs/original/combat.md` section 8).

**Gold** (the 30 % roll as before): without a party, a gold item on the ground. **With a party, no gold item**: `share = trunc(amount /
count)` (all members counted), **at least 1**, and **every member in range** gets `share` straight into its money (`ItemGetMoney`: capped at
2³² − 1; **`0x0205`** 16 bytes as a gold pick-up: `bag 1, slot 0, type 26, type id 0, count 1, amount`). The shares of members out of range
are lost. (Premium gold bonuses: not covered.)

## 10. Party heals and buffs

Confirmed, `CUser::SkillHealParty` 0x4538a0, `CUser::SkillBuffParty` 0x453c90; types 5: part 3.

Completes `docs/original/skills-use.md` section 10 and `docs/original/skills-buffs.md` section 4. **Target type 4 with a party**:
`0x0511` result 4 to the players within 60 as before; then **each member, the caster included**, that the caster may help
(`IsTeamUsable`: itself, or not dueling, the same duel state, and the same country unless the map allows both) **and is in range**
(`EnableApplyRange(member, caster, byApplyRange)`: alive, in the world, same zone and map, within `byApplyRange`) gets the heal
(`HealUser(member, skill, false)`, `0x0513`) or the buff (`BuffUser(member, caster, false)`). **Type 5** the same without the caster.
(In a union, the type-4 heal reaches every member, the others only the caster's group of 6: `docs/original/union.md` section 9.)

## 11. Rebirth next to the leader

Confirmed, `CUser::PacketPC` 0x0553 at 0x46ab77, `CUser::UpdateRebirth`, `CUser::KillSoulParty` 0x4596a0.

Completes `docs/original/mob-fight.md` section 8. **`0x0553` kind 3**: without a party, back to life at the bind position at once (as kind
2). **In a party: the rebirth time becomes now + 30 s with rebirth type 1**; nothing else happens yet. When it passes (`UpdateRebirth`,
every second), `KillSoulParty`: for a dead player (not `m_nStatusLife` 2) in a zone, not in ultimate mode, still in a party, **whose
leader is alive and in the same zone**: `ExpDrop(m_nDropExp)` and **`Rebirth(the leader's position, kind 5, the player's map, full)`**
(`0x0551` with kind 5). Otherwise (no party any more, leader dead or elsewhere, the player being the dead leader): `KillSoul` (bind
position, kind 4). The 200 s automatic rebirth keeps type 0 (bind position).

## 12. The packets

| op | dir | bytes | section |
|---|---|---|---|
| `0x0B01` | S → C | 4 + units | 3, the list for a member joining |
| `0x0B02` | both | 6 | 2, invite by id / the invitation |
| `0x0B03` | both | 7 | 2 and 3, the answer / a failure code |
| `0x0B04` | S → C | 2 + unit | 3, a new member |
| `0x0B05` | both | 2 / 6 | 4, leave / someone left |
| `0x0B06` | both | 6 | 4, kick / someone was kicked |
| `0x0B07` | both | 6 | 4, the leader |
| `0x0B08` | S → C | 8 | 9, a member's loot |
| `0x0B09` | both | 23 | 2, invite by name / the invitation with the inviter's name |
| `0x0C01`–`0x0C09` | S → C | | 5 |
| `0x0520` | S → C | 7 | 1, party type of a player |
| `0x1105` | both | 3 + text / 7 + text | 6 |

## 13. The new server

- **The parties live in the zone object** (`Zones::parties`, `crates/game-server/src/party.rs` for the bookkeeping and
  `crates/game-server/src/zone/party.rs` for the packets), because every use crosses maps or needs zone state (positions, vitals,
  applied skills, the damage record, kills and drops): each party's members in slot order, leader slot, loot turn, highest level and
  what the others were last shown of each member; each player's party; the pending invitations with their deadlines (dropped in the
  one-second pass). A party outlives map changes (a player entering a map shows its party type in its looks and its position to the
  members there) and ends for a player when it leaves the world (`Zones::party_leave_world`, before `Zones::leave`, at the logout and
  at the end of a lost connection's 10 s).
- **Requests** `0x0B02`, `0x0B03`, `0x0B05`, `0x0B06`, `0x0B07`, `0x0B09` and `0x1105` go from the connection to the zone, which
  sends every packet through the players' outboxes.
- **Member status**: `0x0C07` on every fourth step, a teleport and entering a map; `0x0C09` with a level-up; everything else from the
  party pass (with the 500 ms monster pass): death (`0x0C06`), maxima (`0x0C08`), HP, SP and MP (`0x0C03`), applied skills taken
  off and added (`0x0C05`, `0x0C04`), each when it differs from what the others were last shown.
- **Kills**: the damage record holds party entries and the party of each player entry (`crates/game-server/src/mobs.rs`,
  `Dealer`); a party kill sends each member near the killer `ZoneEvent::PartyKill` (its connection works out its share at its own
  level), items go in turn as `ZoneEvent::Loot` to the receiving member's connection (into its bags, the rest back on the ground
  with the kill's owner and party), gold as `ZoneEvent::Gold` shares; ground items carry their party for picking up.
- **Party skills** (section 10) and **rebirth kind 3** (section 11, `cp::REBIRTH_PARTY`) use the party.
- **Deviations** (approved):
  - unknown `0x0Bxx` opcodes are ignored like any unhandled packet rather than closing the connection (unions are built:
    `docs/original/union.md`);
  - **`0x0B03` failure codes carry a real id**: the invited player's for codes 0, 1, 3 and 5 (so "<p> rejected …" shows), the inviter's
    own for 2 and 4;
  - **kicking keeps the leader right**: the leader slot moves down with the slots, and a leader kicking itself is ignored (section 4's
    original bug); the loot turn follows the leaving rule;
  - **the full-party bonus is also paid on a gain that levels up** (section 8's quirk);
  - **member status from the 500 ms pass**: `0x0C03` and `0x0C08` replace `0x0C01` and `0x0C02`, and death and applied skills follow at
    most half a second late, but nothing is missed;
  - "busy" is a personal shop set up or open (`docs/original/personal-shop.md`); quest items and guild creation are built
    (`docs/original/quest-drops.md`, `docs/original/guild.md` section 4); the game log records are written ([The game log](game-log.md)); premium bonuses: [Item mall and premium](item-mall.md).
  - **A party drop is never lost** (audit R14, not in the original): an item or gold share sent to a member whose connection is gone
    goes on the ground (the item where it dropped, the gold at the member's feet); a member whose bag write fails, or whose gold save
    is refused or fails, leaves it on the ground too, before the error ends the connection; what does not fit is put on the ground
    before the pick-up packets are sent; and the events still queued for a member who is leaving the world are put on the ground by its
    connection's cleanup (an item where it dropped, gold at the member's feet, its own to pick up).

## 14. Other episodes

The EP3 `ps_game.exe` (linked 2008-01-17) and the EP5.4 one (linked 2011-09-14, modified after linking) were compared with the EP4.5
build, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

`PacketParty` (EP3 ps_game 0x44ed10, matched by switch position) handles the normal party opcodes; the union opcodes `0x0B0C`–`0x0B18`
and `0x0B1A` do not exist (confirmed, dispatch tables). The party update packets `0x0C0A`–`0x0C14` are built only by EP4.5 (confirmed:
the opcode constants are absent from the EP3 image); how EP3 shows a member's state was not read. Kicking a member placed before the leader
moves the leadership as in EP4.5 (`CParty::ExpelUser` EP3 ps_game 0x432dc0, matched by call position; confirmed). There is no full-party
bonus in `AddExpFromUser` (EP3 ps_game 0x442070), so `party-exp-levelup` cannot occur (confirmed).

### EP5.4

- The same 20 opcodes with the same handlers, and the same experience by level gap (1.0, 0.8, 0.6, 0.4, 0.2) (confirmed,
  `CParty::GetPartyExpByLevel` EP5.4 ps_game 0x44e230, matched by call graph).
- **The party type goes through the client.** The server sends `0x0B1B` to the player only (`SendPartyType` EP5.4 ps_game 0x491670, and
  14 sites in `CParty`); the EP5.4 client echoes it, and the server relays the client's type byte, unchecked, as `0x0520` to the view
  (EP5.4 ps_game 0x475fa4) (confirmed). A modified client can therefore show any party mark to the players around; the server's own
  party state does not read it (inferred).

  | offset | size | `0x0B1B` (both directions) |
  |---|---|---|
  | 0 | 2 | opcode |
  | 2 | 4 | player id (ignored on the way back; the server uses the sender's id) |
  | 6 | 1 | party type: 0 none, 1 member, 2 leader |

- Kicking a member placed before the leader now moves the leader's index down (EP5.4 ps_game 0x450324; confirmed); the loot turn still
  does not move.
- The full-party bonus is still lost on a level-up (EP5.4 ps_game 0x46517b; confirmed).
