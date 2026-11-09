# Union parties: making one, joining, groups, the leader and sub-leader, and what a union changes elsewhere

This page covers the union ("raid") party of up to 30 players in the EP4.5 game server `ps_game.exe`: `0x0B0B` to
`0x0B1A`, `0x0C0A` to `0x0C14`, `0x1112`, and the union's experience, loot, gold, party skills and chat. It was read with the decompiler
and confirmed in the disassembly where noted. The client side is on the client's [Union parties](../client/union-parties.md) page and
[Social packets](../client/client-social-packets.md) ("Union ("raid") party").

This page completes [Parties](party.md): everything said there holds for a union unless this page says otherwise.

The `0x0B0B` that `CUser::PacketAdminCmdE` sends at 0x4755fd is not this page's union list: it is `FA15` `/guildpoint`'s message to
ps_dbAgent with a guild's points, which uses the same number (see [GM commands 1](gm-commands.md#86-guilds-fa13-fa14-fa15)).
Not covered here: game logs (see [The game log](game-log.md)), quests.

## In short

- **A union is a party of up to 30**, shown as five groups of six. A party leader turns the party into a union; the leader and a
  sub-leader can invite, and the leader can move members between groups, hand over the lead, kick, or dismantle the union.
- **Auto-join.** With auto-join on, anyone of the same faction can join a union by naming one of its members (`/join <name>`), from
  anywhere in the world.
- **Loot rules.** The leader chooses: in turn, a random member, or everything to the leader.
- **Experience is shared more thinly than in a party**: the share table is one step behind the party's (two members get 73 % each
  instead of 100 %), and from 18 members on each gets 1 %. There is no full-party bonus.
- **Group-only skills.** Party buffs reach only the caster's group of six; only the party heal that includes the caster reaches the whole
  union.
- **Client quirks.** The US client ignores the leader's union chat line, and a player who joins is not fully counted in the union by
  the client until it gets its own "new member" packet, which the original never sends to it.

Status words: **confirmed** = read in the code (address given). `CUser` offsets are the code's.

## 1. The union is a party

Confirmed, `CParty`, `CParty::Reset` 0x444ca0.

A union is a normal `CParty` with its flag set; the same object, id and member array. The fields used (party section 1):

| field | offset | |
|---|---|---|
| `m_nBothIndex` | +0x0C | the leader's index in the member array |
| `m_nNumUser` | +0x10 | member count, **at most 30** in a union (7 in a party) |
| `m_pParty[30]` | +0x14 | `{int Index, CUser* pUser}`: in a union, **`Index` is the member's slot** (0 to 29; group = slot / 6) |
| `m_nItemDivType` | +0x104 | the loot rule (section 7) |
| `m_nItemDivSeq` | +0x108 | whose turn it is (rules 1 and 3) |
| `m_nSubBothIndex` | +0x110 | the sub-leader's index in the member array |
| `m_bRaidPartyFlag` | +0x114 | **1** union, 0 party |
| `m_bAutoJoinFlag` | +0x115 | auto-join (section 4) |

The member array stays packed in joining order (a member leaving moves the later ones down, each **keeping its slot**); the slots
are what the client shows as the 5 groups of 6. `Reset` (a party being created) sets every slot to `0xFFFF`, the sub-leader index to
**1**, the flags to 0.

Every party function tests the flag with **`== 1`** for "union" and **`== 0`** for "party": `CParty::Send` (0x4455d0, to every
member) sends **nothing** for any other value, `LeaveUser` takes the party path, the kill gives no experience (sections 3 and 8).

**`CParty::GetGroup(user)`** (0x445980): the group (0 to 4) of the member's slot; −1 when the user is not a member or its slot is 30
or more.

**Party type** (`0x0520`, party section 1): unchanged, **2 for the leader, 1 for everyone else**, the sub-leader included.

## 2. Making a union, its settings

Confirmed, `CUser::PacketParty` 0x4676c0 cases 0x0B0E, 0x0B0F, 0x0B10; `CParty::SetRaidParty` 0x4458f0.

**`0x0B0E`** (8 bytes: `u8 bRaidType`, `u8 bAutoJoin`, `u32 nDivType`): the sender must lead a party (the member at the leader index)
and `bRaidType` must not be 0; otherwise nothing. Then, **whether the party is a union already or not** (the server's code; this client
sends it only for "Create Union", "Config Union" sends `0x0B0F` / `0x0B10`, see below):

1. `SetRaidParty`: every member's slot = its index in the array (0, 1, 2, … in joining order); then the sub-leader index = **the first
   index that is neither the leader's nor the current sub-leader index**, when there is one (else it stays).
2. The flag = `bRaidType` (the byte as sent), the loot rule = `nDivType` (**no check of the value**), auto-join = `bAutoJoin`.
3. **`0x0B0E`** (16 bytes: `u8 bRaidType, u8 bAutoJoin, u32 nDivType, u32 leader slot, u32 sub-leader slot`) to every member.

What step 1 gives: on creation the sub-leader index is `Reset`'s 1, so **a party led from index 0 gets index 2 as sub-leader when it
has 3 or more members** (index 1 with 2 members); led from any other index, index 0. On a "Config Union" of an existing union, **the
groups are reset to joining order and the sub-leader moves to another member** (the first index that is neither leader nor the
current sub-leader): an original quirk of reusing the creation code.

**`0x0B0F`** (6 bytes, `u32 nDivType`): the sender leads a union: the loot rule = the value; **`0x0B0F`** (the same 6 bytes) to every
member. **`0x0B10`** (3 bytes, `u8 bAutoJoin`): the sender leads a union: auto-join = the byte; **`0x0B10`** (3 bytes) to every member.
The client (`docs/client/CLIENT-FACTS.md`, its answers, verified there): the union settings window's OK sends `0x0B0E`
(`u8 1, u8 autoJoin, u32 rule`) when not in a union; **in a union and its leader, `0x0B0F` (rule 1, 2 or 4) only when the rule
changed and `0x0B10` only when auto-join changed**; a sub-leader or member sends nothing. So this client never sends `0x0B0E` for an
existing union. Received, `0x0B0F` sets the rule and prints 6516 / 6517 / 6518 for 1 / 2 / 4 (other values silent); `0x0B10` sets
auto-join and prints 6520 / 6521. A `0x0B0E` received in an existing union **rebuilds the client's union list from its normal
party list** (the `0x0B01` / `0x0B02` members, slot = index there): members known only from `0x0B0C` / `0x0B0B` vanish, and without
the leader and sub-leader slots among them it sets nothing else.

## 3. Entering a union

Confirmed, `CParty::EnterRaidParty` 0x445a30, `CParty::GetEmptySlot` 0x446480, `CParty::EnterUser` 0x445a10.

`EnterUser` takes the union path when the flag is 1 (else `EnterNormalParty`, party section 3). Refused when the union has 30, or when
`GetEmptySlot` finds no slot: **the lowest slot 0 to 29 no member holds**. Then:

1. The user goes at the end of the array with that slot.
2. When others are already there: **`0x0B0C`** (`u16 slot` + the member unit, party section 3, at most **40** applied skills) to
   **the members already there** (the count is not raised yet, so not the user).
3. The user gets **`0x0B0B`**, the list of every member **itself included**, 6 units to a packet:
   `u8 flag, u8 leader slot, u8 sub-leader slot, u16 loot rule, u8 union flag, u8 auto-join, u8 n`, then n × (`u16 slot` + unit, at
   most **35** applied skills here), in array order. `flag` is 1 for a packet of 6 when the total (the user included) is not a
   multiple of 6, else 0 (the client reads neither `flag` nor the union flag).
4. Count + 1; party search registration cleared; the highest level recomputed; **`0x0520`** for the user to its view; a game log.

The user gets **no `0x0B0C` of its own and no `0x0B0E`**: the list is all it is told. **On the client that is not enough**
(`docs/client/CLIENT-FACTS.md`, verified there): `0x0B0B` fills the list, the leader, sub-leader, rule, auto-join and the joiner's
group, but sets no in-union flag, role or window and prints nothing; only an own `0x0B0C` (or a `0x0B0E`) does (role 2, the union
window, "Joined the party" 207). Without it the joiner sees no party chat, its settings window would send `0x0B0E`, and `/party`
sends `0x0B09`. An own `0x0B0C` after the list is harmless (a slot already listed is kept, and 207 comes last): deviation 9.

## 4. Inviting, answering, auto-join

Confirmed, `PacketParty` cases 0x0B14, 0x0B15, 0x0B18, 0x0B11.

Union failures go to the requester as **`0x0B19`** (`CUser::SendRaidPartyError` 0x467690, 10 bytes: `u32 code, u32 id`; the client
reads the code only). The client prints codes 3, 4, 5, 10 to 14 and nothing for the others.

**`0x0B14`** (6 bytes, `u32 target id`), the invitation by id, in this order: the target is the sender: **4**; the sender busy (party
section 1): **1**; the sender is in a party and is neither **its leader nor its sub-leader** with fewer than 30 members (the leader test
does not look at the union flag; a sender without a party passes): **2**; the target not in the sender's zone (`CZone::FindUser`):
**4**; of the other country: **4**; the target in a party or with an invitation pending: **5** when it is in the sender's party, else
**3**; the target busy: **1**. Otherwise the invitation is pending exactly as party section 2 step 7 (the same `m_dwPartyReqID`, 30 s),
and the target gets **`0x0B14`** (`u32 inviter id`).

**`0x0B18`** (23 bytes, `char name[21]`, the last byte forced to 0): **`0x0B09`'s checks and codes** (party section 2) with the union
authority (leader or sub-leader, fewer than 30, or no party), the failures as **`0x0B03`** (`CUser::SendPartyError`), not `0x0B19`;
the target gets **`0x0B18`** (the inviter's name).

**`0x0B15`** (7 bytes: `u8 answer` 0 yes / 1 no, `u32` own id), from the invited player: as `0x0B03` (party section 3), ignored without
a pending invitation or when already in a party; the inviter looked up in the answerer's zone; the invitation cleared either way. Either
busy: `0x0B19` **1** to the inviter; **no**: `0x0B19` **0** to the inviter (silent on the client). **Yes**: an inviter without a party
creates one (**a normal party**, rule 1) and the answerer enters it; an inviter that leads its party or is the union's sub-leader:
the answerer enters it (`EnterUser`: a union or a party, as the flag says). Otherwise, or when entering fails: `0x0B19` **1**.

The two invitations share one pending slot: a `0x0B03` answer to a `0x0B14` invitation is taken as well (and the other way round),
each with its own checks.

**`0x0B11`** (23 bytes, `char name[21]`, the last byte forced to 0), `/join <name>`: ignored when the sender is in a party. The named
player is looked up **in the whole world** (`CWorld::FindUser`): not found, in no party, its party has no leader, or of the other
country: **`0x0B19` 12**; auto-join off: **13**; 30 members: **14**. Otherwise `CParty::AutoPartyJoin` (0x4473d0): only a union with
auto-join, then `EnterRaidParty` (section 3; a refusal there is silent). No zone, busy or distance check.

A normal party's `0x0B02` and `0x0B09` keep their own checks (party section 2: fewer than **7**), and an answer then enters the union
(`EnterUser`) when the inviter's party is one.

## 5. Leaving and kicking

Confirmed, `CParty::LeaveRaidParty` 0x446510, `CParty::ExpelRaidPartyUser` 0x446cd0.

**`0x0B0D`** (2 bytes): in a party, `CParty::LeaveUser` (0x4464f0): the union path when the flag is 1, else the party's (so `0x0B0D`
also leaves a normal party, and `0x0B05` a union). **`0x0B1A`** (6 bytes, `u32 id`): the sender in a union, **its leader**, and the
id a member (`CParty::FindUser`): `ExpelRaidPartyUser` (the leader may name itself).

Both do the same with their own packet:

1. **`0x0B0D`** (or **`0x0B1A`**) (`u32 id` of the one going) to **every member, the one going included**; its party cleared.
2. The later members move down, each keeping its slot; the leader and sub-leader indexes move with them; the loot turn goes down by one
   when it pointed past the slot after the one going (party section 4).
3. Count − 1. **0**: the union is gone. **1**: the last member leaves too (`LeaveRaidParty`, also after a kick: `0x0B0D` with its own
   id).
   Otherwise: **the leader went**: the sub-leader becomes leader and the new sub-leader is the first member that is not the leader;
   **the sub-leader went**: the first member that is not the leader becomes sub-leader.
4. A guild creation the one going had started is cancelled; the highest level recomputed.
5. **`0x0B17`** (`u32 new leader id`) to the members when the leader changed, then **`0x0B13`** (`u32 new sub-leader id`) when the
   sub-leader changed.
6. **`0x0520`** for the one going (type 0) and the new leader (type 2) to their views; a game log.

Leaving the world leaves the union first (party section 4). A union does not survive a relog.

**In a union, `0x0B05` leaves it** (as `0x0B0D`) but **`0x0B06` and `0x0B07` run the party's code** (`CParty::ExpelUser`,
`CParty::SetPartyBoth`): `0x0B06` / `0x0B07` to the members instead of `0x0B1A` / `0x0B17`, the sub-leader not looked after, and
the party's kick bug (party section 4: the leader index not adjusted). The client sends `0x0B1A` and `0x0B17` in a union.

## 6. Leader, sub-leader, groups, dismantling (verified, cases 0x0B17, 0x0B13, 0x0B12, 0x0B16; `CParty::SetRaidPartyBoth` 0x4452f0,
`SetRaidPartySubBoth` 0x445220, `ChangeUser` 0x445110, `BreakParty` 0x444fa0)

**`0x0B17`** (6 bytes, `u32 id`): the sender leads a union; `SetRaidPartyBoth(id)`: the member with that id at another index than the
leader's, else nothing. The leader index = that member's; **`0x0B17`** (`u32 id`) to every member; when the new leader was the
sub-leader, the sub-leader = the first member that is not the new leader (the old leader, at index 0, as a rule) and **`0x0B13`** to
every member; **`0x0520`** for the new and the old leader to their views.

**`0x0B13`** (6 bytes, `u32 id`): the sender leads a union; the member with that id at another index than the leader's becomes
sub-leader; **`0x0B13`** with **the sub-leader's id** to every member, **even when nobody matched** (then the unchanged sub-leader).

**`0x0B12`** (10 bytes, `u32 source slot, u32 target slot`): the sender is in a union and is its leader **or sub-leader**.
`ChangeUser`: both slots held: the two members swap slots; only the target held: that member takes the source slot; only the source
held: that member takes the target slot; neither: nothing. **Neither slot is checked against 0 to 29.** Then, **whatever happened**,
**`0x0B12`** (18 bytes: `u32 source, u32 target, u32 leader slot, u32 sub-leader slot`) to every member.

**`0x0B16`** (2 bytes): the sender leads a union: **`0x0B16`** (2 bytes) to every member, then `BreakParty`: every member's party
cleared, **`0x0520`** (type 0) for each to its view, game logs; the party is reset and removed. **Dismantling ends the party too**: nobody
is left in a normal party.

## 7. Loot

Confirmed, `CMob::ItemDrop` 0x4a5ea0 and 0x4a65a0, `CParty::GetRaidItemDivSeq` 0x4459d0.

Completes party section 9. The rule is the union's `m_nItemDivType`: the client offers **1 in turn, 2 random, 4 the leader**.

- **Owner of the item made** (0x4a65a0): rule 4 (in any party) with the leader in the monster's zone (`CMob::EnableApplyZoneItem`):
  **the leader**; otherwise the killer. A quest item (real type 27) or a definition with **`ReqDex` (`wDex`, +0x38) 0** (most
  potions, lapis, materials; most gear has 1 to 8): the killer. Party id: the killer's party, as before.
- Quest items stay out of the rules (party section 9).
- **Rule 4, union only**: the leader in range (`EnableApplyRangeItem(100)`: alive, in the world, the killer's zone and map, within 100
  of the killer): an item with `ReqDex` 0 **goes on the ground**; any other **straight into the leader's bags** (`ItemGet`). The leader
  out of range: on the ground.
- **Rule 2**: a random member (`rand() % count`, out of all members); in range: into its bags; out of range: **on the ground** (no second
  draw).
- **Rule 1** (in turn) and **3** (in turn, on the ground owned by that member alone): as party section 9, over the union's members.
- **Any other value** (0, 5, …: only from a modified client, section 2): everything on the ground.
- **Gold**: unchanged (party section 9): `amount / count` (all members counted), at least 1, to every member in range.

`ItemGet` and a manual pick-up tell the members within 100 with **`0x0C14`** (8 bytes, as `0x0B08`) instead of `0x0B08`.

## 8. Experience

Confirmed, `CMob::Death` 0x491286 to 0x491390; `CGameData::GetRaidPartyExp` 0x458160.

As party section 8, with **`m_RaidPartyExp`** (0x5423e8, 31 doubles) in place of `m_PartyExp`, **indexed by the member count the same
way** (`count > 30`: 1.0):

| members | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 to 30 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| share | 1.0 | 0.73 | 0.645 | 0.56 | 0.475 | 0.39 | 0.305 | 0.27 | 0.251 | 0.235 | 0.219 | 0.21 | 0.17 | 0.14 | 0.1 | 0.06 | 0.03 | 0.01 |

**The union table is one step behind the party table** (a party of 2 shares 1.0 each, a union of 2 0.73): an original quirk, kept.
The level gap (`GetPartyExpByLevel`) and the highest level (`ChkPartyExpByLevel`, the same for both kinds) are unchanged. **No
full-party bonus** in a union (`CUser::AddExpFromUser` 0x458311: a normal party of 7 only). A party flag other than 0 and 1 gives
nobody experience.

## 9. Member status, chat, skills (verified: the senders; `CUser::PacketChatE` 0x470a3d; `CUser::SkillHealParty` 0x4538a0,
`CUser::SkillBuffParty` 0x453c90)

**Status** (party section 5): every sender picks the union opcode when the flag is 1, to the same members:

| party | union | | party | union |
|---|---|---|---|---|
| `0x0C01` | `0x0C0A` | | `0x0C05` | `0x0C10` |
| `0x0C02` | `0x0C0B` | | `0x0C06` | `0x0C11` |
| `0x0C03` | `0x0C0C` | | `0x0C07` | `0x0C12` |
| `0x0C08` | `0x0C0D` | | `0x0C09` | `0x0C13` |
| `0x0C04` | `0x0C0E` | | `0x0B08` | `0x0C14` |

(There is no `0x0C0F`.) The layouts and sizes are the party's.

**Party chat `0x1105`** goes to every union member (party section 6). **`0x1112`** (client → server: `u8 length, text`): the sender in a
union and its leader **or sub-leader**; a length above 128 **closes the connection**, 1 or less does nothing; else **`0x1112`** (`u32`
the sender's character id (+0x128), `u8 length, text`) to every member. **This client ignores `0x1112` from the server**
(`client-packets.generated.md`: an explicit no-op), so the leader's union line reaches nobody's screen.

**Party skills** (party section 10): in a union, **the party heal with the caster (target type 4, `SkillHealParty`) reaches every
member** (in range, and the help checks); **the party buff (`SkillBuffParty`) and both kinds without the caster (type 5,
`SkillHealNParty`, `SkillBuffNParty`) only the members of the caster's group** (`GetGroup` equal, so never a member at a slot of 30 or
more). This corrects party section 10's "(In a union, only the caster's group of 6.)", which held for all four.

Rebirth next to the leader (party section 11) uses the union's leader.

## 10. The packets

| op | dir | bytes | section |
|---|---|---|---|
| `0x0B0B` | S → C | 10 + units | 3, the member list for a joiner |
| `0x0B0C` | S → C | 4 + unit | 3, a new member |
| `0x0B0D` | both | 2 / 6 | 5, leave / someone left |
| `0x0B0E` | both | 8 / 16 | 2, make or set up a union |
| `0x0B0F` | both | 6 | 2, the loot rule |
| `0x0B10` | both | 3 | 2, auto-join |
| `0x0B11` | C → S | 23 | 4, `/join` |
| `0x0B12` | both | 10 / 18 | 6, a group move |
| `0x0B13` | both | 6 | 5 and 6, the sub-leader |
| `0x0B14` | both | 6 | 4, invite by id / the invitation |
| `0x0B15` | C → S | 7 | 4, the answer |
| `0x0B16` | both | 2 | 6, dismantle |
| `0x0B17` | both | 6 | 5 and 6, the leader |
| `0x0B18` | both | 23 | 4, invite by name / the invitation with the inviter's name |
| `0x0B19` | S → C | 10 | 4, a failure code |
| `0x0B1A` | both | 6 | 5, kick / someone was kicked |
| `0x0C0A`–`0x0C14` | S → C | | 7 and 9 |
| `0x1112` | both | 3 + text / 7 + text | 9 |

## 11. The new server

- **A union is a party with union settings** (`crates/game-server/src/party.rs`, `Party::union`): each member's slot, the sub-leader,
  the loot rule and auto-join; the size limit 30. The zone code (`crates/game-server/src/zone/party.rs`) sends the union packets in the
  original's order: `Zones::union_set`, `union_rule`, `union_auto_join`, `union_join`, `union_move`, `union_sub_leader`,
  `union_leader`, `union_kick`, `union_break`, `union_chat`; invitations and answers share `party_invite` / `party_answer` with a
  union flag; entering and leaving (`enter_party`, `depart`) take the union path for a union.
- The party pass (party section 13) and every member-status sender pick the union opcodes. Kills: `ZoneEvent::PartyKill` and
  `PlayerKill` carry the union flag for the experience table (`party::member_exp`; `CUser::Death` picks the table the same way for a
  player kill) and no full-party bonus (`party::full_party`); the loot rules 1 to 4 at the kill (`Zones::attack_mob`), with the item
  definitions' `ReqDex` (`ItemDef::req_dex`); the heal without the caster reaches only the caster's group (`Zones::union_group`), every other
  party skill the whole union (deviation 6).
- `0x1112`: its text is checked first, as for every chat kind (too long closes the connection, whoever sends it).
- **Deviations** (1 to 5 approved; 6 and 7 requested; 8 and 9 approved):
  1. **Any non-zero `bRaidType` makes a union** (the original stores the byte, and a value other than 1 leaves a party that nothing is
     sent to and that gives no experience).
  2. **The loot rule is 1 to 4**: `0x0B0E` or `0x0B0F` with another value is ignored (the original takes any value; 0 and 5 and up put
     every item on the ground). The client offers 1, 2 and 4.
  3. **Group moves stay within slots 0 to 29**, and `0x0B12` goes out only when a member moved (the original takes any slot number, so
     a member could be moved out of every group and out of the client's 30-slot table, and it answers a move that did nothing).
  4. **"Config Union" keeps the sub-leader**: `0x0B0E` on an existing union renumbers the slots as the original does but leaves the
     sub-leader where it is (the original moves it to another member each time). *Replaced by deviation 8, now approved.*
  5. **In a union, `0x0B06` and `0x0B07` act as `0x0B1A` and `0x0B17`** (the original runs the party's kick and leader change: the
     sub-leader not looked after, the leader index wrong after some kicks).
  6. **Party buffs reach the whole union**, with or without the caster, like the party heal with the caster (the original limits
     buffs, and the heal without the caster, to the caster's group of 6). The heal without the caster keeps the group.
  7. **A leader's union line goes out as `0xF10A`** (`u32 id, u8 length, text`), the staff's union line, which this client shows to
     anyone as "[Union] <text>" (message 6606, `docs/client/gm-and-commands.md`, "F1xx — GM chat"), instead of `0x1112`, which it ignores.
     The checks are the original's (a union, its leader or sub-leader).
  8. **`0x0B0E` for an existing union is ignored** (approved). This client never sends it (section 2: "Config Union" is `0x0B0F` /
     `0x0B10`); the original's answer would reset the groups, move the sub-leader and make every client rebuild its union list
     from its party list, dropping the members that joined the union.
  9. **A joiner gets its own `0x0B0C` after the `0x0B0B` list** (approved; section 3): without it this client does not count itself
     in the union (no window, no "Joined the party", no party chat).
  - As for parties: "busy" is a personal shop; quest items (the union's members in the killer's zone count) and guild creation are built
    since, as for a party (`docs/original/quest-drops.md`, `docs/original/guild.md` section 4); the game log records are written ([The game log](game-log.md)); premium bonuses: [Item mall and premium](item-mall.md).

## 12. Client side (answered in `docs/client/CLIENT-FACTS.md`)

- `0x0B0F` / `0x0B10`: what sends them and what the client shows, section 2.
- A joiner without an own `0x0B0C` is not in the union on the client (section 3, deviation 9).
- `0x0B0E` to an existing union: the client rebuilds its list from its party list (section 2, deviation 8).
- `0x1105` shows for union members ("[Party]<p> : <b>", 353, looked up in the union list). `0xF10A`: the id is read and dropped; the
  line is 6606 "[Union] <b>" in the chat window and the screen-centre list (pink, 10 s); no name is added.

## 13. Other episodes

The EP3 `ps_game.exe` (linked 2008-01-17) and the EP5.4 one (linked 2011-09-14, modified after linking) were compared with the EP4.5
build, read statically like the rest of this note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md).

### EP3

There are no union parties: `0x0B0C`–`0x0B18`, `0x0B1A` and the union chat `0x1112` are not handled (confirmed, dispatch tables).

### EP5.4

The union cases are in the same `PacketParty` table with the same handlers (confirmed). `CParty::EnterRaidParty` (EP5.4 ps_game
0x44ed90, matched by constants) keeps the 30-member limit (0x44edb0) and the groups of 6 (0x44f0bc) (confirmed). The only constant change
in the union functions is the party type packet: `0x0520` to the view became `0x0B1B` to the player, which the client echoes back for the
server to relay as `0x0520` (confirmed for the opcode; see [`party.md`](party.md)). The union experience share is inlined and was not
compared.
