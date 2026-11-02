# Monsters fighting back: targets, chasing, going home, their attacks; player death, rebirth and recovery

This page covers monsters in combat in the EP4.5 game server `ps_game.exe` — how they pick a target, call for help,
chase, go home and attack — and the player's side of it: death, rebirth and recovery over time. It comes from static analysis; nothing was
executed. Players attacking monsters are on [Fighting monsters](combat.md), spawning and walking about on
[Monsters](mobs.md), the client's side on [World packets](../client/client-world-packets.md). Scripted monsters (`AI` 10 and above,
state 100 or a stack machine) are on [Scripted monsters](scripted-mobs.md): they reach `AttackAI` and what follows through their
`Battle` handler, and that page lists where they leave the paths below.

## In short

- **Who a monster attacks.** Aggressive monsters attack the first player who comes within their chase range. Others wait to be hit.
  Some call nearby monsters to help. Every 5 seconds a fighting monster may switch to whoever has built up the most aggro (REC counts
  for a lot).
- **Chasing has limits.** A monster gives up when its target is more than 45 units from it or from its home, and walks home. Back
  home, it is fully healed and forgets who hurt it.
- **Monster damage** uses the same kind of formula as player damage, with a level factor; a hit is 5 % to 95 % likely.
- **Dying** costs 5 % of the current level's experience (nothing up to level 10) when the player comes back. After 200 seconds, or when
  the player asks, they come back at the map's bind point with full HP, SP and MP. In ultimate mode, dying without a rebirth item or
  skill deletes the character.
- **Recovery.** Every 5 seconds a player regains a little HP, SP and MP; sitting gives much more (28 plus 5 % of the maximum). Many
  monsters heal while idle.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CUser` offsets are the code's (4 above the PDB's from `stUserInfo`'s base stats on); `CMob` offsets match the PDB. "rand()" is the C
library's (0 to 32767), "trunc" the x87 store with chop rounding, times are `GetTickCount` milliseconds.

## 1. The passes

Confirmed, zone update 0x4075a0.

| every | pass | does |
|---|---|---|
| 100 ms | `CZone::UpdateUserAttack` | players' normal attacks (`docs/original/combat.md`) |
| 500 ms | `CZone::UpdateMobAttack` 0x421990 | `CMob::UpdateAttack(now)` for every monster (section 3) |
| 1000 ms | `CZone::UpdateMobIdle` | walking about, looking for targets (section 2), bodies, respawns (`docs/original/mobs.md`) |
| 1000 ms | `CZone::UpdateApplySkill` 0x421f80 | per player `CUser::UpdateRecover` (section 9) and `CUser::UpdateRebirth` (section 8); per monster `CMob::UpdateRecover` (section 9) |

Monster states (`m_nStatus`): **0 idle, 1 fighting, 2 dead, 3 going home** (100 scripted). The target is `m_Target` (kind 1 = player, id).

## 2. Becoming a target

Confirmed.

**`CMob::SetTarget(player, help)`** (0x49fad0): only a monster in state 0 takes a target (state 4 too unless `m_nMoveMode` is 1; state 4 is
not used by the code covered here), and not for a dead player. Then: state 1, next move time 0, target = the player. With `help` set, by the
monster's `AI`: 2 → `FindHelpNormal`, 3 → `FindHelpAttack`, 5 → `FindHelpNormal2` (below). Then for each attack entry 0 to 2 with `byOK` set
and a skill (`pInfo`): its ready time `m_pAttackTime[i]` becomes 0 for entry 0 or a skill without reuse time, otherwise
**`now + rand() % (wResetTime × 1000)`**. Entries without a skill keep their time.

Who calls it:

- **A player's hit that does not kill** (`CUser::AttackMob` 0x44cfcd, the normal attack of `docs/original/combat.md` section 3, step 8):
  `SetTarget(player, 1)` unless the player is dead, and the monster is "in battle". A miss does not.
- **Aggressive monsters** (`CMob::Targeting` 0x49fcf0, from the idle pass, `docs/original/mobs.md` section 6 step 1): `AI` **1 or 3** run
  `CMob::FindTarget` (0x49fd20): the players in the 3 × 3 cells around the monster's cell (rows of z, then x), the first one that is alive,
  has no shape (`m_byShape` 0), sees monsters (`m_bNotSeeMOB` 0), is visible and attackable as staff (`stAdminInfo` `bVisible`, `bAttackable`),
  and is at most **`byChaseRange`** away (horizontal) gets `SetTarget(player, 1)`. Country is not checked here (the attack pass drops a target
  of the monster's own country, section 3). The idle pass only tries when the next new-target time (`m_dwNextNewTargetingTime`, `now + 5000`
  at creation) is unset or reached; it then clears it, so an idle aggressive monster looks every second.
- **Calls for help** (only from `SetTarget` with `help`; the player must be of another country than the caller, alive, see monsters and be
  visible and attackable): every idle (state 0) monster in the 3 × 3 cells around the caller within the **caller's** `byChaseRange` (horizontal)
  gets `SetTarget(player, 0)`, when it is
  - `AI` 2 (`FindHelpNormal` 0x49ff70): of the same monster id as the caller;
  - `AI` 3 (`FindHelpAttack` 0x4a0470): of `AI` 1 or 3;
  - `AI` 5 (`FindHelpNormal2` 0x4a0200): any.

**Changing target by aggro** (`CMob::ReTargetByAggro` 0x490ab0): at most every 5 s (`m_dwNextTargetingTime = now + 5000` after each try). The
damage record's entry with the most aggro (`CDamageMob::GetMaxAggro`: the first with the highest; aggro is the attacker's REC × 10 per hit,
`docs/original/combat.md`) becomes the target when it is a player, not the current target, on the map, of another country, targetable
(`CUser::EnableTargeting`: alive, sees monsters, visible and attackable) and **at most 45** from the monster: state 1 (unless dead), next move 0.
Runs from the attack pass (section 3) and after each hit that leaves the monster alive in state 1 (`CMob::Damage` 0x490ee0).

## 3. The attack pass

Confirmed, `CMob::UpdateAttack` 0x4a7db0, `CMob::AttackAI` 0x4a1ab0.

`CMob::UpdateAttack(now)`: nothing for scripted or dead monsters or before **`m_dwNextAttackTime`**. Then:

- **State 1**: nothing while the attack speed is 255 (stunned, asleep). The target must be on the map and alive; otherwise the monster drops it
  (state 0, no target). Then `AttackAI(now, player)`.
- **State 3**: `ReturnHome(now)` (section 5).

**`AttackAI(now, player)`**: the monster drops the target (state 0, no target) when the player is of the monster's country (`nCountry`, 2 = both
never matches), dead, does not see monsters, or is not visible or not attackable. Otherwise it is in battle, and the attack entries are tried
**from 2 down to 0**; an entry is skipped when its `byOK` is 0 or `now < m_pAttackTime[i]`, and by `byOK`:

| `byOK` | the entry is used |
|---|---|
| 3 | while HP is from 20 % to 40 % of the maximum: `trunc(max × 0.2) ≤ HP ≤ trunc(max × 0.4)` |
| 4 | while `HP ≤ trunc(max × 0.05)` |
| 5 | while `HP < trunc(max × 0.2)` |
| 6 | when it has a fixed vision target, or `FindAttackUserMinHP` finds an attacker, which becomes the target (`SetTarget(…, 1)`); `docs/original/mob-skills.md` section 2 |
| anything else (1, 2, 7 …) | always |

For a used entry (all but 6 first run `ReTargetByAggro`), `m_nCurrAttack = i`, then:

- **A monster that does not walk** (`AI` 4, `byChaseStep` 0 or an obelisk guard): a normal attack (`byType` 0) needs **`CMob::EnableAttackRange`**
  (0x4a19c0): horizontal distance **`< bySize + byRange + 0.3`**; out of range the monster **drops the target** (state 0). In range: `CMob::Attack`
  (section 6), and the entry's ready time becomes `now + g_ConstMobAttackSpeed[speed] + dwTime` (entry 0's time is not set).
- **A walking monster**: a normal attack first runs **`CMob::Chase(now, player position, byRange)`** (section 4). When it says "in range", `Attack`
  and the entry's ready time as above; otherwise nothing more (the chase took its step).
- **A skill** (`byType` ≠ 0): `CMob::UseSkill` (section 7).

Only the first usable entry is tried. **When none is usable**, `m_dwNextAttackTime = now + g_ConstMobAttackSpeed[speed] + 2500`.
`g_ConstMobAttackSpeed` (0x54251c) is **`{1500, 1000, 500, 0, -500, -1000, -1500}`** for attack speeds 0 to 6 (the speed clamped to 0..6; a new
monster has 3, so 0).

## 4. Chasing

Confirmed, `CMob::Chase` 0x4a6de0.

**Corrected** by `docs/original/pathing.md` section 3: step 5 sets a **path** (the target's cell on a free line) and later turns walk
it without testing the line again, so a chase runs to where the target was, then aims again.

Returns "in range" or not; `d` is the horizontal distance from the monster to the target's position (`m_vPos`: where its last step ended).

1. Nothing (not in range) before **`m_dwNextChaseTime`**.
2. **In range when `bySize + range + 1.5 > d`**: the chase path is reset and the attack goes ahead.
3. Nothing when the move speed is 255.
4. **Too far from home: the target more than 45 from home** (`m_vPosRespawn`, horizontal) → state 3 (going home), no target, path reset.
   **The target more than 45 from the monster** → state 0, no target (the idle pass then sends it home, `docs/original/mobs.md` section 6 step 3).
5. With the truncated coordinates of the monster (`mx, mz`) and the target (`tx, tz`), `CMap::EnableLine(mx, mz, tx, tz)` (the line walk of
   `docs/original/mobs.md` section 6, here also giving the **last free cell** before a blocked one):
   - **Free**: the vector `v = (tx − mx, 0, tz − mz)` (from the truncated monster position), `L = |v|`, `n = v / L`, the step
     **`s = g_ConstMobMoveSpeed[move speed] + byChaseStep`** (`{-4, -2, 0, 2, 4}`, so `byChaseStep` at speed 2), and the gap left
     `g = L − (bySize + 0.4) − range`. The monster's truncated position is added to its **chase path** (below).
     - `g ≥ s`: a full step, **`MoveMob(position + n × s, motion 1)`**, next chase time `now + dwChaseTime`.
     - else, when `bySize + 0.6 − range ≥ L`: no move, next chase time `now`.
     - else: **`MoveMob(position + v − n × (bySize + range + 0.4), motion 1)`** (to stand that far from the target's truncated position), next chase
       time `now + trunc(g / s × dwChaseTime)`.
   - **Blocked, the target's cell blocked or off the map** (`CMap::IsBlockSafe`): when the monster already stands within 1 of the last free cell
     (both axes), it gives up unfairly: **`m_bFairCombat` cleared** (a kill now gives nothing, `docs/original/combat.md` section 6), state 3, no
     target. Otherwise, when the target is within `bySize + entry 0's byRange + 0.3` of the last free cell, the monster heads for that cell (as in
     the free case, with the cell as the target position); if not, state 3 and no target.
   - **Blocked with the target's cell free**: an A* search (`AStar::FindPath`) and the steps along its path; when no path is found the monster
     stops (`CMob::StatusAttackStop`: state 0, no target). The search and its path: `docs/original/pathing.md`.
6. Every case sends `0x0603` with motion 1 through `CZone::MoveMob` (`docs/original/mobs.md` section 6: cell changes, then the view), and `MoveMob`
   refuses positions off the map.

**The chase path** (`ChasePathList`, a stack of whole-number cells): `AddPath(x, z)` replaces the newest cell when the line from the one before it to
`(x, z)` is free (`CMap::EnableLine`), adds nothing when the newest cell is `(x, z)` already, and pushes `(x, z)` otherwise.

## 5. Going home

Confirmed, `CMob::ReturnHome` 0x4a7670, from the attack pass in state 3.

**Corrected** by `docs/original/pathing.md` section 4: the chase path's newest cell is taken off when the monster sets out for it,
and the search is covered there.

1. Nothing before the next chase time.
2. **Home when less than 10 from home** (horizontal; with a chase path left the code also tests 0.5, with the same result): state 0, then
   **`CMob::ResetAttack`**, the paths reset and the **damage record cleared** (`CDamageMob::Reset`, `m_bFairCombat` set again).
3. Otherwise nothing at move speed 255; else with the truncated home `(hx, hz)` and position:
   - **The line home is free**: `v`, `L`, `n` and `s` as in section 4, the gap `g = L − (bySize + 1)`: `g ≥ s` a full step (`MoveMob(position + n × s,
     motion 1)`, next chase time `now + dwChaseTime`); else onto `(hx, y, hz)` (`y` the monster's), next chase time `now + trunc(g / s × dwChaseTime)`.
   - **Blocked, home blocked or off the map**: when the last free cell is the monster's own, its **home becomes where it stands**; state 0, paths reset.
   - **Blocked, home free**: the newest cell of the chase path is taken off and walked to as above; without a chase path an A* search
     (`docs/original/pathing.md`): without a path, state 0 and the home becomes where it stands.

**`CMob::ResetAttack`** (0x4905c0): buffs and attack modifiers cleared, move speed 2, attack speed 3, attack ready times 0, **HP, SP and MP back to
the definition's**, the stats reset; **`0x0305`** (`u32 id, u32 HP`) to the players within 60 of the monster; **`0x0609`** (`TP_SET_SPEED`, 8 bytes:
`u32 id, u8 attack speed, u8 move speed`) within 60 (**original bug:** `CMob::SendSpeed` 0x4906d0 never writes the id, so the packet carries
whatever was on the stack); the attack numbers again (`CMob::SetAttack`).

## 6. One normal attack

Confirmed, `CMob::Attack` 0x4a2000.

Nothing while stunned or asleep, or with its melee kind disabled (`m_pAttack[0].nDisable`). The entry is `pAttack[m_nCurrAttack]`; normal entries
have `byType` 0, so the attack kind is **0 (melee) for every normal attack**.

1. **`CUser::StatusStand(player)`** (0x48a100): a sitting player stands up (motion 0, recovery state at least 1, personal shop closed) and a pending
   logout is cancelled (unless it is a reconnect).
2. **Hit or miss** (`CMob::AttackSuccess` 0x4a23f0, no skill, kind 0): blocked by the player's block (`CUser::ChkBlock`: block-all, or a skill's block
   count or chance; none without skills) is a miss. With `Lm`, `Lu` the monster's and the player's levels:
   **`rate = trunc(160 × Lm / (Lu + Lm) − 100 × player.hit / (mob.evasion + player.hit) + 100 × mob.hit / (player.evasion + mob.hit))`**, clamped to
   **5..95** (NaN, from 0/0, gives the lowest integer: 5); **hit when `rand() % 100 < rate`**. (The numbers are the kind 0 ones of
   `docs/original/combat.md` section 4. Kinds 1 and 2 have their own formulas but are never used by normal attacks.)
3. **A miss: `0x0605`** (`TP_MOB_ATTACKPC`, 17 bytes: `u8 result` 3, `u32 monster id`, `u32 player id`, `u16 0, 0, 0`) to the players within 60 of the
   monster; then step 7.
4. **Damage** (`CMob::AttackDamage` 0x4a2920, kind 0):
   - level factor **`P = clamp((100 × (Lm − Lu − 1) + Lm) / 100 × 10, 1, 95)`** (as for players);
   - **`A = wDamage × m_fStr + mob attack`** (`m_fStr` 1.0, the mob's melee attack number 0 without buffs), **plus `rand() % wDamageAdd`** when that
     is above 0;
   - element **`E = m_AttackAttr[0][player defence element]`** when the player's element is below 9 (else 1): the table's first row, so 1.0 for no
     element, 0.8 for 1 to 4, 0.7 for 5 to 8 (the entry's `byAttrib` is not used). The defence element (`m_nAttrDef`, `CUser::SetAttrDef`) is the
     **upper armour's** (slot 1) `byAttrib`, changed by its lapis: a lapis element replaces none and cancels a different one;
   - `R = rand() % LUC + 1` with the monster's LUC (0 when LUC is 0); `D` = the player's melee defence;
   - **`damage = (A + A / 100 × P) × E − (D − D / 100 × P) + R × 0.1`**, at least 0;
   - **critical**: chance `trunc(mob.critical / 100 × 100 − player LUC × 0.034)` clamped to **5..99** (`mob.critical = trunc(LUC × 0.19)`); when
     `rand() % 100 <` chance: `R2 = (rand() % LUC + 1) × 1.4` (0 when LUC is 0), **`damage = damage × 1.5 + R2 / 100 × P + R2`**, result 1 (else 0);
   - `damage[0] = trunc(damage)`, damage at most 30000, **at most 10 becomes `rand() % 10 + 1`**. SP and MP damage 0.
5. **`CUser::Damage(player, 2, monster id, damage)`** (0x44c1e0): nothing for a dead player or one with HP 0. Armour wear (`QualityByDefense`,
   `docs/original/repair.md` section 2); the damage is recorded on the player (`CDamage`, for player kills); SP and MP lose `damage[1]`, `damage[2]` (floored at 0); skill
   effects that take the damage from SP or MP or reduce it (`0x0525`) are built (`docs/original/skills-buffs.md` section 10,
   `docs/original/lapis.md` section 11). **When `HP ≤ damage[0]`: HP, SP and MP become 0 and the player
   dies** (section 8, `CUser::Death(2, monster id)`), after the monster drops its target (state 0); else HP −= damage.
6. **A hit: `0x0605`** with the result (0 or 1), the monster id, the player id and the three damages, to the players within 60 of the monster
   (**after** the death packets of a kill). Then `CUser::BeAttackedItemOption` (the armour's lapis skills, `docs/original/item-options.md`).
7. **`m_dwNextAttackTime = now + trunc((g_ConstMobAttackSpeed[speed] + pAttack[0].dwTime) / m_fAttackSpeed)`** (`m_fAttackSpeed` 1.0; always
   entry 0's time).

## 7. Monster skills (`CMob::UseSkill` 0x4a48e0): see `docs/original/mob-skills.md`

Skill entries refer to the `MobSkills` table (`usp_Read_MobSkills_R`, `CGameData::LoadConstMobSkill`), not to the players' skills. At load
(`CGameData::LoadConstMob` 0x4c1d6a) an entry with a skill id keeps it only when the skill exists and is an attack or debuff (`byTypeEffect` 1 or
3) or a heal of type 41 or 42; otherwise the entry is cleared (`byType` and `byOK` 0, never used). Entries without a skill have `byRange` capped at
60, and entries 0 and 1 an element above 4 set to 0.

`UseSkill`: by `byTypeEffect` 1 attack (`SkillAttack`: by `byTargetType` 3 the target, 6 an area around the monster, 7 an area around the target;
`CMob::AttackUser` and the skill version of `CMob::AttackDamage`), 2 heal, 3 debuff; then `m_dwNextAttackTime = now + g_ConstMobAttackSpeed[speed] +
2500`. In `AttackAI` a heal is used without chasing, other skills after a chase into the skill's `byAttackRange`; the entry's ready time becomes
`now + wResetTime × 1000`. Analysing these, and loading `MobSkills`, is the next step. 1,249 definitions have a skill in entry 1, 134 only skills
in entry 0.

## 8. Player death and rebirth

Confirmed.

**`CUser::Death(killer kind, killer id)`** (0x4586a0): nothing when already dead. Experience to lose: `m_nDropExp` = **5** (percent; 0 or 1 with
death-protection items, not covered); state 1 (dead), HP, SP and MP 0; trade, personal shop and shared mount cancelled. Killed by a player:
player-kill rules (`docs/original/pvp.md` sections 6 and 9). Then the buffs that end with death are removed, the damage record cleared, a
body is recorded (`CUser::EnterDead`, used by resurrection skills), and the **rebirth time is `now + 200000`** (200 s), rebirth type 0.
**`0x0504`** (`TP_DEATH`, 11 bytes: `u32 player id, u8 killer kind, u32 killer id`; kind 2 = monster) to the player's 5 × 5 view. Party members
get `0x0C06` (`docs/original/party.md` section 5). The client opens its death window on `0x0504` for the local player (`docs/client`).

Every monster attacking a dead player drops it on its next pass (section 3) and goes home from the idle pass.

**Client `0x0553`** (`u8 kind`, `CUser::PacketPC` 0x46ab2a):

| kind | |
|---|---|
| 2 | **back to life at the bind position** (`CUser::KillSoul`) |
| 3 | the same, but in a party the player waits 30 s (rebirth type 1: next to the leader, `docs/original/party.md` section 11) |
| 4, 5 | with a resurrection item (`KillSoulByItem`, `KillSoulByItemNoMove`; not covered) |

`0x0552` (rise where the body lies, after a resurrection skill: `m_nStatusLife` 1) never does anything in the original: it needs a state
that only `CUser::Soul` sets, and nothing calls `Soul` (see [Item buffs](item-flags.md#7-coming-back-to-life)). The new server does not handle it.

**`CUser::UpdateRebirth`** (0x48d240, every second): a dead player whose rebirth time has passed goes back to life as with `0x0553` kind 2
(rebirth type 1 in a party: the party version).

**`CUser::KillSoul`** (0x4599e0), for a dead player whose `m_nStatusLife` is not 2:

- **Ultimate mode** (`grow` 3), unless player-killed (`m_nDropExp` −1): with a rebirth skill or a resurrection item the player comes back at the
  bind position with **half** HP, SP and MP; without, the experience drop, then **the character is deleted** (`0x0406` to the database agent) and
  logged out. Not covered (the new server treats ultimate mode as the others for now).
- Otherwise: **`CUser::ExpDrop(m_nDropExp)`**, the **bind position** (`CMap::GetBindPos`, below) of the player's current map, and
  **`CUser::Rebirth(position, 4, map, full)`**.

**`CUser::ExpDrop(percent)`** (0x45ad50): nothing up to level 10, for 0 or less, or with the no-drop item. With `T` the experience totals
(`docs/original/enter-world.md` section 3.1), `span = T[grow][level] − T[grow][level − 1]`, **`loss = trunc(percent / 100 × span)`**, lowered by the
faction's kill-count bonus (`m_KCInfo` type 25: `loss −= trunc(v / 100 × loss)`, `docs/original/pvp-2.md` section 8), **at most `exp − T[grow][level − 1]`** (never below the level's start; nothing when that is
0 or less); `exp −= loss`, saved.

**`CMap::GetBindPos(country, position)`** (0x4295a0), on the map the player is on:

1. The map's rebirth point for the country (`Map.ini` `RebirthMapPos` for both, else `RebirthMapPos1` light and `RebirthMapPos2` fury,
   `world.maps`): when its map exists and the point is on that map (`CMap::IsSafePos`): **that map and `(x, y + 0.9, z)`**.
2. Else the map's **bind areas** (`stMapBind`: count, country, attribute, boxes `x1, y1, z1, x2, y2, z2`): the area nearest to the player (by its first
   box's `x1, z1`, horizontal; the first of equals, starting from area 0 at distance 9999) among those with boxes whose country is the player's
   (light 1, fury 2) or 0 or 3; area 0 when none is. A random box (`rand() % count`), then **`z = z1 + rand() % trunc(z2 − z1)`**, **`y = y1 + 0.9`**,
   **`x = x1 + rand() % trunc(x2 − x1)`** (whole-number offsets, in that order), this map.
3. Without bind areas: where the player is, on this map.

**`CUser::Rebirth(position, kind, map, full)`** (0x459ec0), only for a dead player: with `full`, HP, SP and MP **at their maxima** (`KillSoul` passes
full). The body record goes. Then:

- **Same map**: the player moves there (`CZone::MoveUser` → `MoveUserLocal`: view swapped as for a step, **no `0x020B`**); **`0x0551`**
  (`TP_DEAD_REBIRTH`, 23 bytes: `u32 player id, u8 kind` (4), `u32 experience` (after the drop), `f32 x, y, z`) to the player's view, the player
  included; then **`0x0505`** with the new HP, MP and SP (as amounts gained: they were 0) to the players within 60.
- **Another map**: `0x0551` to the player only, `0x0505` within 60, the player leaves the zone (`CWorld::ZoneLeaveUserMove`) and gets **`0x020B`**
  (`u32 id, u16 map, f32 x, y, z`), and is placed when its client sends `0x0201` (as for a gatekeeper, `docs/original/gatekeeper.md` section 3).

Finally motion 0, recovery state at least 1, **state 0 (alive)**.

## 9. Recovery over time

Confirmed.

**Players** (`CUser::UpdateRecover` 0x48cfb0, every second, acts when `now ≥ m_dwNextRecoverTime`; nothing while dead): the amounts by the
recovery state `m_nRecStatus`:

| state | HP | SP | MP |
|---|---|---|---|
| 0 sitting | `trunc(SitRecHPP × maxHP × 0.01 + maxHP × 0.05 + SitRecHP)` | same with SP | same with MP |
| 1 normal | `RecHP` | `RecSP` | `RecMP` |
| 2 fighting | `AtkRecHP` | `AtkRecSP` | `AtkRecMP` |

At start (`CUser::InitEquipment` 0x454560): **`RecHP/SP/MP` 4, `SitRecHP/SP/MP` 28, `SitRecHPP/SPP/MPP` 0, `AtkRec*` 0**, recovery state 1. Lapis of
type 16 add their HP, SP and MP values to `Rec*` and `SitRec*` instead of to the maxima (`CUser::GemEquipmentAdd`); passive skills and kill-count
bonuses add more (built: `docs/original/skills.md` section 7, `docs/original/pvp-2.md` section 11).

`CUser::SendRecoverAddSet` (0x482060): the amounts are added, each value capped at its maximum. When anything changed: **`0x0505`** (`u32 id, i16
HP, MP, SP` gained) to the players within 60 (and the player's damage record cleared when HP just reached the maximum); then **`0x0521`**
(`TP_USER_RECOVERSET`, 8 bytes: `u16 HP, MP, SP`, absolute) to the player itself when alive and either some value is at its maximum or the player
sits.

Then the next time is **`now + 5000`**, and the next state: a player in state 0 (alive, not attacking) that sits → 0, that stands → 1 (other motions:
unchanged); a player attacking (state 2) → 2. (Starting an attack sets 2 at once and stopping sets 1, `docs/original/combat.md` section 1.)

**Monsters** (`CMob::UpdateRecover` 0x4a8210, every second): definitions with `nRecoverValue` (the `Attack3` column) above 0, when `now ≥
m_dwNextRecoverTime` (0 at creation), below their maximum HP and **idle (state 0)**: HP += `min(nRecoverValue, max − HP)`, the next time `now + 5000`,
and **`0x0610`** (`TP_MOB_RECOVER`, 10 bytes: `u32 id, i32 HP gained`) to the players within 60 of the monster. (1,489 of the 2,747 definitions recover.)

## 10. The new server

- Monsters: the states of section 1 (`MobState` in `crates/game-server/src/mobs.rs`); entry choice, chasing and going home in
  `crates/game-server/src/fight.rs`; targets, aggro, calls for help, the 500 ms attack pass, the recovery pass and revival in `zone.rs`; the
  monster's blow in `combat.rs` (`mob_strike`). The monster updater runs every 500 ms (attacks) and every second (walking, respawns, recovery).
- A player's HP, MP and SP, its numbers against monsters (melee hit, evasion, defence, LUC; the upper armour's element) and its recovery state
  are kept by the zone, so monster attacks and recovery see the same values as the connection; the connection is told of a death
  (`ZoneEvent::Died`) and of being stood up (`ZoneEvent::StoodUp`), handles `0x0553` and the 200 s timer, and ignores moves and attacks while dead.
- Bind areas come from the map files (`world.map_binds`, `tools/world/import.py`), rebirth points from `world.maps`.
- HP, MP and SP are loaded with the character, saved when leaving the world, on coming back to life and with each level-up.
- **Deviations**:
  - (**built**: the A* search and the path a monster keeps, `docs/original/pathing.md`; before it, a monster whose straight line
    to its target was blocked stopped, one with no way home took its spot as home, and the newest chase path cell was taken off when the
    monster reached it)
  - (**built**: monster skills and `byOK` 6 entries, `docs/original/mob-skills.md`; **correction**: a monster's hit also breaks the
    player's cast, wakes it and ends hiding, that document's section 9)
  - monster skills and `byOK` 6 entries were not used at first (such entries were skipped, so the 134 monsters with only a skill in entry 0 did not
    attack yet);
  - aggressive monsters pass over players of their own country (the original takes the first player in range and drops it on its next attack
    turn, so such a player could shield others);
  - `0x0609` carries the monster's id (original bug, `docs/original/community-fixes.md`);
  - after entering the world the player gets its current HP, MP and SP (`0x0521`), since the burst only carries the maxima;
  - ultimate-mode death does not delete the character; a lower maximum after an equipment change also lowers the current value (the
    original's handling of that case is not read).
- Not yet: lapis recovery values (type 16: dropped; no lapis has it). Built since (`docs/original/item-flags.md`): the resurrection
  items of `0x0553` kinds 4 and 5, the death-protection items (`m_nDropExp` 0 or 1), the ultimate-mode death (kept as fallen); `0x0552`
  never does anything in the original (section 7 there). Built since as well: the
  resurrection skill (`docs/original/skills-special.md` section 12) and the Target Resurrection Rune (`docs/original/item-use.md` section 11),
  parties, player kills, block and damage-reducing skills, item effects on being hit (`docs/original/item-options.md` section 7), lapis
  defence elements, the kill-count bonuses, staff invisibility (`docs/original/gm-commands.md` section 13).
