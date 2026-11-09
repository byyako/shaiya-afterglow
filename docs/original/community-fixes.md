# Known bugs in the original server and their fixes

Bugs in the original EP4.5 game server, `ps_game.exe`, for people still running the original files: how each one shows in play, where
it is in the code, and how to fix it. Many now have a fix in the [patch kit](../server-patches.md), a checked, reversible byte patcher for
this build. Fixes made by the community are named with their authors.

Each entry that the new server also had to face ends with a **New server** line saying what it does. The new server is this project's
clean-room replacement for the original files; everything else on this page is about the original.

Nothing here was tried on a live server: the bugs, the reproduction steps and the fixes all come from reading the code.

Every bug was looked up in one build of `ps_game.exe`, the one the rest of this site describes:

| | |
|---|---|
| SHA-256 | `0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547` |
| size | 1,323,008 bytes |
| built | 2009-07-15 (PE timestamp, UTC) |

**The addresses are specific to that build** (SHA-256 `0fb7c9f1…`). Other builds and episodes have the same functions at other
addresses, and some bugs may not exist there.

## In short

- **Money and shops.** A large purchase from an NPC or a personal shop could wrap its price round to almost nothing; a personal
  shop's visitor list can fill up with players who have left, so nobody else can open it.
- **Items.** A modified client can trade, stake or sell bound items, pass them on through the guild bank, link or enchant a cloak
  with anything (an apple enchants a cloak to +20 absorption), remake one item into a new one, and use blacksmith services anywhere;
  enchant rolls repeat within a second; a trade or duel stake whose count shrank after the offer duplicates the item.
- **Skills.** A rune right after a skill cuts its delay; skill levels could be skipped when learning; hiding in Stealth lets
  equipment bonuses pile up; damage over time on players comes out too low; Illusion breaks after a relog.
- **Parties and quests.** Kicking a member can secretly move the leadership; the full-party bonus is lost on the kill that levels you
  up; quest items vanish when the bags are full; a modified client hands quests in without going to their NPC.
- **Text.** A malformed chat colour marker crashes every client that shows the line. An apostrophe in a character or guild name, a
  guild remark, a gift's target, a GM notice or a GM command's target reaches the database unescaped.
- **What to do:** the [patch kit](../server-patches.md) fixes 30 clear bugs and two design choices in the file on disk. It was checked in
  the disassembly (most fixes also in an emulator), not on a live server, so try it on a spare copy first. A few bugs have no fix yet;
  the one that would mostly help cheaters is [named without the details](#held-back-until-there-is-a-fix).

The login server's security problems (SQL injection through the login, account enumeration and others) are on the
[login server](ps_login.md#13-security-findings-in-the-original) page.

## How to read this page

Each bug gives:

- **Symptom:** what goes wrong.
- **To reproduce:** how it should show at a player level, worked out from the code. "A modified client" means the normal client never sends that request, but the
  server accepts it.
- **Where:** the function and address in the build above.
- **Fix:** the kit's fix id and how far it was checked, or a community fix, or what a fix must do when there is none yet.

The labels:

- **Verified in disassembly:** the fix was assembled, written into a copy of the build, disassembled again and checked against the
  code around it, its jump targets and the stack. **Not run on a live server.**
- **Verified in disassembly and emulation:** as above, and its instructions were also run in an x86 emulator. Not run on a live
  server.
- **Fix planned:** no bytes yet; what a fix must do is given.
- **Community fix:** made and published by the named author; read here as evidence of the bug, not checked or run, and not hosted
  here.

## Money and shops

### Gold overflow when buying from an NPC {#npc-shop-price-wrap}

- **Symptom:** buying from an NPC shop charges `count × price` computed in 32 bits, so a large enough total wraps round and only the
  remainder is charged: expensive items for almost nothing.
- **To reproduce:** buy a stack whose total price passes 4,294,967,295 gold.
- **Where:** `CUser::PacketShop` 0x46b55a, `imul eax, [esi+0x80]` (see [NPC shops](shop.md)).
- **Fix:** kit [`npc-shop-price-wrap`](../server-patches.md#npc-shop-price-wrap), verified in disassembly and emulation: an unsigned
  multiplication, and a total that needs more than 32 bits answers result 1, as too little money does. The community fix
  `GoldBugFix.CT` by **ScreaM** makes the same refusal in the running process; the kit's version is written from this build's
  disassembly and patches the file.

- **Further references:** `0x0702`, `count * nBuy`.

- **New server:** **fixed**: prices are computed in 64 bits; one that does not fit is "not enough money" (`inventory.rs`)

### Buying from any merchant on the map {#npc-shop-range}

- **Symptom:** the server only checks that the merchant is on the same map, not near the player.
- **To reproduce:** a modified client buys from a merchant far away.
- **Where:** `CZone::FindNpc`, used by the shop handler (see [NPC shops](shop.md)).
- **Fix:** planned: require the merchant to be in the player's view. The distance should be confirmed against a running client first.

- **New server:** **fixed**: the merchant must be in the player's view

### A personal shop's visitor list fills up with players who left {#myshop-visit-cleanup}

- **Symptom:** a visitor who leaves the zone while a personal shop is open in front of them is never taken off the seller's visitor
  list. After sixteen such visitors the list is full and nobody else can open that shop.
- **To reproduce:** open another player's personal shop, then change map (a portal, a gatekeeper, a teleport) with the shop window
  still open. Repeat with sixteen visitors.
- **Where:** `CUser::LeaveZone` 0x44aefa calls `stMyShop::Unuse` (0x4784a0) on the leaver's own shop instead of the seller's (see
  [Personal shops](personal-shop.md)).
- **Fix:** kit [`myshop-visit-cleanup`](../server-patches.md#myshop-visit-cleanup), verified in disassembly.

- **New server:** **fixed**: a visitor leaving the map or the world is taken out of the seller's list ([Personal shops](personal-shop.md), deviation 4)

### Personal-shop prices wrap round {#myshop-price-wrap}

- **Symptom:** buying from a personal shop charges `count × price` computed in 32 bits, as the NPC shop does, so a total past
  4,294,967,295 wraps round. Only the client checks the total when an item is placed; the server takes any price.
- **To reproduce:** a modified client places an item at 2,147,483,648 gold a piece (2³¹); buying two then costs 0 (2 × 2³¹ wraps to
  0), and 2,147,483,649 a piece makes two cost 2.
- **Where:** `CUser::PacketMyShop`, the buy (`0x230D`) at 0x479078, `imul edx, [ebx+eax*4+0x1644]` at 0x4790f0 (see
  [Personal shops](personal-shop.md)).
- **Fix:** kit [`myshop-price-wrap`](../server-patches.md#myshop-price-wrap), verified in disassembly and emulation: an unsigned
  multiplication, and a total that needs more than 32 bits answers result 2 ("Insufficient gold").

- **Further references:** `count × slot price`.

- **New server:** the NPC-shop product is computed in 64 bits; a personal shop's too, and a total above 2³² − 1 is refused ([Personal shops](personal-shop.md), deviation 3)

## Bound items

Items whose `ReqOg` mark (`stItemInfo.byOG`, not 0) says they may not change hands: 1,013 items in the standard data. The market and
the death drop check the mark; these three places check only for quest items (real type 27), or nothing, and leave the rest to the
client.

### Bound items can be traded {#bound-trade}

- **Symptom:** a modified client can put bound items in the trade window.
- **Where:** `CUser::PacketExchange` 0x46ec90 (see [Trade](trade.md)).
- **Fix:** kit [`bound-trade`](../server-patches.md#bound-trade), verified in disassembly.

- **New server:** **fixed**: the server refuses them

### Bound items can be staked in a duel {#bound-duel}

- **Symptom:** a modified client can stake bound items in a duel.
- **Where:** `CUser::PacketPVP`, the stakes check at about 0x47d764 (see [Duels](duel.md)).
- **Fix:** kit [`bound-duel`](../server-patches.md#bound-duel), verified in disassembly.

- **New server:** see [bound-trade](#bound-trade)

- **New server:** **fixed by design** (duel stakes and personal-shop placing refuse bound items and quest items: [Duels](duel.md), [Personal shops](personal-shop.md) deviation 2)

### Bound and quest items can be sold in a personal shop {#bound-shop}

- **Symptom:** placing an item in a personal shop checks nothing about the item, so a modified client can sell bound or quest items.
- **Where:** `CUser::PacketMyShop` 0x478e2b (see [Personal shops](personal-shop.md)).
- **Fix:** kit [`bound-shop`](../server-patches.md#bound-shop), verified in disassembly.

- **New server:** see [bound-trade](#bound-trade)

- **New server:** see [bound-duel](#bound-duel)

### Bound and quest items can enter the guild bank by a swap {#guild-bank-bound}

- **Symptom:** a deposit into the guild bank refuses bound and quest items, but taking an item out onto a bag place that is already
  taken swaps the two, and the bag's item goes into the bank without those checks. Another member can then take it out.
- **To reproduce:** with a bound item in the bags, take an item out of the guild bank onto the bound item's place (`0x0204` from bag
  255 to that place; whether the normal client allows that drop was not checked).
- **Where:** `CGuild::ItemGuildBankToBag` 0x42fea0, the swap from 0x42fff1 (see [Guild bank](guild-bank.md)).
- **Fix:** kit [`guild-bank-bound`](../server-patches.md#guild-bank-bound), verified in disassembly and emulation: the swap applies the
  deposit's checks; a refused swap changes nothing and answers nothing, as a refused deposit.

- **New server:** **fixed** (guild bank deviation 2)

## Linking, enchanting and repairing

### Anything can be linked into a cloak {#cloak-lapis}

- **Symptom:** for a cloak (real type 24) the "can this be linked" check answers yes before looking at the second item, so any item
  serves as a lapis: its type id lands in the cloak's badge bytes, for the price of its grade (usually nothing).
- **To reproduce:** a modified client links a non-lapis item into a cloak.
- **Where:** `CUser::EnableGemAdd` 0x45fa50 (see [Lapis and enchanting](lapis.md)).
- **Fix:** kit [`cloak-lapis`](../server-patches.md#cloak-lapis), verified in disassembly: cloaks take no lapis.

- **New server:** **fixed** (deviation 1, proposed): cloaks take neither

### Anything can enchant a cloak {#cloak-lapisia}

- **Symptom:** the same shortcut in the enchant check: any item serves as a Lapisia on a cloak. An apple enchants a cloak to +20
  absorption, for free.
- **To reproduce:** a modified client enchants a cloak with a non-Lapisia item.
- **Where:** `CUser::EnableLapisianAdd` 0x45f960 (see [Lapis and enchanting](lapis.md)).
- **Fix:** kit [`cloak-lapisia`](../server-patches.md#cloak-lapisia), verified in disassembly: cloaks take no Lapisia.

- **New server:** see [cloak-lapis](#cloak-lapis)

### The enchant material's place is not checked {#enchant-material-bounds}

- **Symptom:** the bag and slot of the enchant material index the item table without a bounds check, so a bad place reads outside it:
  a crash, or a stray object taken as the material.
- **To reproduce:** a modified client names a material place outside the bags.
- **Where:** `CUser::ItemLapisianAdd` 0x45fb20 (see [Lapis and enchanting](lapis.md)).
- **Fix:** kit [`enchant-material-bounds`](../server-patches.md#enchant-material-bounds), verified in disassembly: a place outside the
  bags is no material.

- **Further references:** `0x0805`.

- **New server:** **fixed**: a place outside the bags is no material

### Enchant rolls repeat within a second {#enchant-seed}

- **Symptom:** every enchant builds a fresh random generator and seeds it with the time in seconds xor the session id, so two enchants
  by one player within the same second roll the same number, and the roll can be worked out from the server's clock.
- **To reproduce:** enchant twice within one second with the same material count: both succeed or both fail.
- **Where:** `CUser::ItemLapisianAdd`, the seed at 0x45fc87 to 0x45fc9b (`SRandom::SRandom` 0x4a8840, `SRandom::SetRandomSeed`
  0x4a8980; see [Lapis and enchanting](lapis.md)). Nothing else seeds this generator, so simply dropping the reseed would
  leave it seeded with the time alone.
- **Fix:** kit [`enchant-seed`](../server-patches.md#enchant-seed), verified in disassembly and emulation: the CPU's cycle counter
  (`rdtsc`) is mixed into the seed, so no two enchants share one. The rates are unchanged.

- **Further references:** `time(0)`, 0x45fc7e.

- **New server:** **fixed by design**: one generator for the server

### Remaking with one item {#remake-distinct}

- **Symptom:** a remake (three of the same item into one new one) does not check that the three places differ. Naming one item three
  times turns it into a new item with fresh craft stats, and its object is freed three times, which can crash the server or worse.
- **To reproduce:** a modified client sends `0x0807` with the same bag and slot three times.
- **Where:** `CUser::ItemRemake` 0x4608b0 (see [Lapis and enchanting](lapis.md)).
- **Fix:** kit [`remake-distinct`](../server-patches.md#remake-distinct), verified in disassembly and emulation: the request is ignored
  unless the three items are different.

- **New server:** **fixed** (deviation 2, proposed): three different places

### Blacksmith work needs no blacksmith {#blacksmith-range}

- **Symptom:** repairing, linking, extracting and enchanting are handled anywhere; only the client offers them at a blacksmith.
- **To reproduce:** a modified client repairs or links away from any NPC.
- **Where:** repairs in `CUser::PacketPC` 0x46ac4d; linking, extracting and enchanting in `CUser::PacketGem` 0x46bd40 (see
  [Wear and repair](repair.md)).
- **Fix:** planned: require a blacksmith in the player's view (the same distance question as the merchants).

- **Further references:** `0x0554`, `0x0801`, `0x0802`.

- **New server:** **fixed** (deviation 1, proposed): a blacksmith must be in view

- **New server:** **fixed** (deviation 3, proposed): a blacksmith must be in view

### The Operator's Exclusive is no sure link {#op-exclusive}

- **Symptom:** the staff Linking Hammer (item 100/82, 2000 %) multiplies the rate like any hammer, and the 50 % limit applied after it
  holds it at 50 %, though it was evidently meant to be certain.
- **To reproduce:** link with the Operator's Exclusive: about half the attempts fail.
- **Where:** `CUser::ItemGemAdd` 0x4616fa (see [Lapis and enchanting](lapis.md)).
- **Fix:** kit [`op-exclusive`](../server-patches.md#op-exclusive), verified in disassembly. A design choice, applied only when named:
  hammers of 1000 % or more always link.

- **New server:** **fixed** (deviation 4, approved): hammers of 1000 % or more always link

### Repairing one item can spoil the shield's quality save {#repair-shield-slot}

- **Symptom:** after a one-item repair the new quality is written into the saved-quality copy of slot 6 (the shield), whatever was
  repaired; the index is left over from an earlier loop. The shield's next quality change may then be skipped, or saved needlessly,
  when the character leaves.
- **To reproduce:** not visible directly: repair one item other than the shield, let the shield lose quality, and compare its quality
  after a relog.
- **Where:** `CUser::ItemRepair` 0x464340 (see [Wear and repair](repair.md)).
- **Fix:** kit [`repair-shield-slot`](../server-patches.md#repair-shield-slot), verified in disassembly and emulation: the saved quality is
  written for the repaired slot, and only for the worn slots (bag 0), as the shield's before was a leftover index.

- **New server:** **fixed by design**: each slot's own quality is saved

### Helmets and cloaks never drop {#helmet-cloak-drop}

- **Symptom:** checks on real types 16 (helmets) and 24 (cloaks) leave them out when items drop.
- **Where:** not looked up in this build (item drops are not covered on these pages).
- **Fix:** community fix in `ps_gametablas.CT`, "Fix drop of helmets and capes", by **Eric-Dutra16** and **andr3y_you96**. Whether it
  is a bug or a choice is up to the server.

- **New server:** **open**: a design choice when drops are built

## Skills and buffs

### A rune right after a skill cuts the skill's delay {#rune-skill-cut}

- **Symptom:** using a rune (item 100133) straight after a skill cuts the skill's delay.
- **To reproduce:** use a skill, then the rune at once.
- **Where:** `CUser::ItemUse` 0x465630 (see [Using items](item-use.md)). On the server side, the rune's casting is free when
  it is cancelled by a step, so it can be repeated without end; the server still spaces skills by the attack speed, and the gain is
  the client's skill animation cut short (client side, not checked).
- **Fix:** community fix `Ep4 Skillcut Fix.CT` by **Cups** (2018), written for this build: it stores the time of the last skill or buff
  use and ignores the rune for 3 seconds after it. Kit: not built. The same rule needs the time of the last skill kept per player, and no field of the player object is
  proven unused; stopping the rune while the next attack time has not come would also stop it after every normal blow.

- **Further references:** `0x0221`, `CancleMove`.

- **New server:** **fixed** (kit: not built, it needs a per-player field none proven free; deviation 4, approved): every move by item (specials 31, 32, 47 to 49) is refused within 3 s of a `0x0511`/`0x0517`, with the error answer

### Skill levels can be skipped when learning {#learn-skill-level}

- **Symptom:** the learn request accepts any level above the one known, so level 3 can be bought straight from level 1 for level 3's
  points only. For a passive skill the server then removes level 2's effect, which was never added, so the character's numbers drift
  for good.
- **To reproduce:** a modified client asks for a level two above the one known.
- **Where:** `CUser::LearnSkill` 0x4842e0, `CUser::RemSkillPassive` (see [Skills 1](skills.md)).
- **Fix:** kit [`learn-skill-level`](../server-patches.md#learn-skill-level), verified in disassembly: only the next level can be learnt.

- **Further references:** `0x0209`.

- **New server:** **fixed**: only the next level can be learnt

### Equipment bonuses pile up in Stealth {#stealth-equip}

- **Symptom:** while a character has a shape (Stealth sets shape 13), equipping and unequipping skip the item's attack and defence.
  When Stealth ends only the shape is reset, nothing is recomputed (a transformation's end recomputes everything). So an item taken
  off in Stealth keeps its bonus, and putting it back on afterwards adds it again. Repeatable.
- **To reproduce:** hide with Stealth, take an item off, let Stealth end, put the item back on: its attack and defence count twice.
- **Where:** the shape checks at 0x454d1d, 0x454e21, 0x454fcb and 0x455200 in `CUser::ItemEquipmentAdd`, and the matching ones in
  `CUser::ItemEquipmentRem`; Stealth's end in `CUser::RemApplySkillBuff` (see [Skills 3](skills-buffs.md)).
- **Fix:** kit [`stealth-equip`](../server-patches.md#stealth-equip), verified in disassembly: equipment counts normally while hidden,
  so nothing can drift and item moves stay allowed. The community fixes refuse item moves instead: a code cave by **Bowie**
  (kreon800) that skips item moves in shape 13, and an earlier one at the start of `CUser::ItemBagToBag` (0x45B690) whose author is
  not named.

- **Further references:** `m_byShape`, `+0x1354`, `wType`.

- **New server:** **fixed by design**: the numbers are recomputed from scratch after every change and hiding leaves the equipment's numbers alone (`docs/original/skills-special.md` section 5.4), so nothing can drift; item moves stay allowed in Stealth (refusing them as the patch does would be a separate choice)

### Damage over time on players comes out too low {#time-damage-player}

- **Symptom:** for players the "time since the last payment" part of a damage-over-time debuff is added to the base instead of
  multiplying it, as the monster version does. A debuff paid in one go at its end deals about one payment, however long it ran.
- **To reproduce:** compare a damage-over-time debuff paid at its end on a player with the same kind on a monster.
- **Where:** `CUser::TimeDamage` 0x486290 computes `base + elapsed / 3000`; `CMob::TimeDamage` computes `base × elapsed / 3000` (see
  [Monster skills](mob-skills.md)).
- **Fix:** kit [`time-damage-player`](../server-patches.md#time-damage-player), verified in disassembly: players multiply as monsters do.

- **New server:** **fixed**: players take `base × elapsed / 3000` as monsters do

### Low-HP passives pile up rows in the database {#low-hp-rows}

- **Symptom:** each time a low-HP passive takes effect the database agent is told to add an applied skill, but that row is never
  deleted, so rows pile up.
- **To reproduce:** let a low-HP passive trigger several times and look at the character's applied skills in the database.
- **Where:** `CUser::ChkHPAbilityApply` 0x487020 (see [Skills 3](skills-buffs.md)).
- **Fix:** planned: do not write these rows (write applied skills of kinds 3 and 4 once, when the character is saved).

- **Further references:** `0x0901`.

- **New server:** **fixed by design**: only kinds 3 and 4 are written, once, when the character is saved

### An ending block buff can cancel a newer one {#block-buff-end}

- **Symptom:** when a buff that blocks one kind of attack ends (Magic Protection and Magic Evasion block magic, Eagle Eye ranged),
  the server clears that kind's block outright, even if another buff of the same kind set it since.
- **To reproduce:** have Magic Evasion and Magic Protection on at once; when the older one runs out, magic is no longer blocked
  though the other still shows.
- **Where:** `CUser::RemApplySkillBuff` 0x485130, cases 85 and 86 (see [Skills 3](skills-buffs.md)).
- **Fix:** planned: recompute buff effects from the whole list when one ends.

- **New server:** **fixed by design**: buff effects are recomputed from the whole list, the newest block buff of each kind counts

### Fire Thorns' extra aggro never happens {#fire-thorns-aggro}

- **Symptom:** a self buff with an aggro value (Fire Thorns, Mass Taunt) should anger the monster the player is fighting, but the
  server only does it when the player is not attacking, and a player always is when an instant skill goes off.
- **To reproduce:** use Fire Thorns while fighting a monster that is hitting a party member: it does not turn to you.
- **Where:** `CUser::BuffUser` 0x450ac0, the status check at 0x450c9f (see [Skills 3](skills-buffs.md)).
- **Fix:** kit [`fire-thorns-aggro`](../server-patches.md#fire-thorns-aggro), verified in disassembly. A design choice, applied only when
  named: the aggro goes to the monster the caster was attacking.

- **New server:** **fixed** (a design choice): the aggro goes to the monster the caster was attacking

### Illusion comes back without its monster after a relog {#illusion-save}

- **Symptom:** Illusion is saved with the character, but the monster it copies is not. After the next login the character has the
  Illusion shape with monster 0, and its item attack and defence are 0 until Illusion ends.
- **To reproduce:** as a ranger, use Illusion on a monster, log out and back in before it ends.
- **Where:** `CUser::SendDBAgentCharGetInfo` saves every lasting buff except Disguise; `CUser::InitEquipment(bool)` 0x454ba0 finds no
  monster (see [Skills 4](skills-special.md)).
- **Fix:** kit [`illusion-save`](../server-patches.md#illusion-save), verified in disassembly: Illusion is not saved, like Disguise.

- **Further references:** `wChangeType`, `m_wTransFormTargetMobID`, `InitEquipment(false)`.

- **New server:** **fixed**: Illusion is not saved, like Disguise

### Illusions look wrong to players arriving later {#illusion-newcomers}

- **Symptom:** a player coming into view of an illusioned character is told its shape but not which monster it copies; only the
  change itself sends the monster.
- **To reproduce:** while someone is illusioned, come into view of them from far away: they do not look like the monster.
- **Where:** `CZone::SendMoveUser` 0x41f350, `CCell::SendCellInfoEnter` 0x423aa0 (see [Skills 4](skills-special.md)).
- **Fix:** planned: send newcomers the shape-change packet (`0x0524`) with the monster.

- **Further references:** `0x051D`.

- **New server:** **fixed**: newcomers get `0x0524`

## Parties

### Kicking a member can move the leadership secretly {#party-kick-leader}

- **Symptom:** a kick moves the later members down but, unlike leaving, does not adjust the leader's index or the loot turn. When the
  kicked member came before the leader, the server takes the next member (or a stale copy of the last one) as leader while every
  client still shows the old one. A leader kicking itself does the same.
- **To reproduce:** pass the leadership to a member who joined later, then have the new leader kick someone who joined before them.
- **Where:** `CParty::ExpelUser` 0x447120, against `CParty::LeaveNormalParty` 0x446960 (see [Parties](party.md)).
- **Fix:** kit [`party-kick-leader`](../server-patches.md#party-kick-leader), verified in disassembly and emulation: a kick moves the
  leader's slot and the loot turn down as leaving does, and a leader kicking itself is refused with no answer.

- **New server:** **fixed**: kicking adjusts both as leaving does; kicking oneself is ignored

### The full-party bonus is lost on a level-up {#party-exp-levelup}

- **Symptom:** in a full party of seven each experience gain counts twice, except the gain that levels the player up, which counts
  once.
- **To reproduce:** in a party of seven, compare the experience of the kill that levels you up with the ones before.
- **Where:** `CUser::AddExpFromUser` 0x458240 (see [Parties](party.md)).
- **Fix:** kit [`party-exp-levelup`](../server-patches.md#party-exp-levelup), verified in disassembly: the bonus is paid on every gain.

- **New server:** **fixed**: the bonus is paid on every gain

## Quests

### Quest items are lost to full bags {#quest-full-bags}

- **Symptom:** the items a quest gives at its start, and its reward item, vanish when the bags are full; the quest starts or ends
  anyway.
- **To reproduce:** fill the bags, then start a quest that gives an item, or end one with an item reward.
- **Where:** `CUser::QuestAddItem` 0x47e6f0 (see [Quests](quest.md)).
- **Fix:** planned: do not start or end the quest, and answer "Insufficient inventory".

- **New server:** **fixed**: the start or end does not happen, with "Insufficient inventory"

### Quests handed in without their NPC {#quest-end-npc}

- **Symptom:** the end request names the NPC by its object id, and for item, monster and player-kill quests the server looks the NPC
  up only when that id is not 0. With id 0 the quest is handed in anywhere.
- **To reproduce:** a modified client sends the quest end request (`0x0903`) with NPC id 0 for a quest whose end NPC is far away.
- **Where:** `CUser::QuestEndChkItemPick`, `QuestEndChkKillMOB` and `QuestEndChkKillPC` (0x47f670 to 0x47f740), called from
  `CUser::QuestEndRequest` 0x47f2a0 (see [Quests](quest.md)).
- **Fix:** kit [`quest-end-npc`](../server-patches.md#quest-end-npc), verified in disassembly and emulation: a quest that names an end
  NPC is not handed in with NPC id 0; quests without one are unchanged.

- **New server:** **fixed**: a quest that names an end NPC is only handed in at it

### Quest NPCs need not be near {#quest-npc-range}

- **Symptom:** starting and ending a quest look the NPC up anywhere in the zone, without a distance check.
- **To reproduce:** a modified client starts or ends a quest far from its NPC.
- **Where:** `CZone::FindNpc`, called from the `CUser::QuestStartChk…` and `QuestEndChk…` functions (see [Quests](quest.md)).
- **Fix:** planned: require the NPC to be in the player's view (the same distance question as the merchants).

- **New server:** **fixed**: the NPC must be in the player's view

## Monsters, the map and the gatekeeper

### The gatekeeper takes the fare while a logout is pending {#gatekeeper-logout}

- **Symptom:** with a logout pending, the gatekeeper takes the money and the player does not move.
- **To reproduce:** start logging out, then use a gatekeeper before the countdown ends.
- **Where:** `CUser::PacketMaininterface`, request `0x020C` (see [Gatekeepers](gatekeeper.md)).
- **Fix:** kit [`gatekeeper-logout`](../server-patches.md#gatekeeper-logout), verified in disassembly: the request fails and nothing is
  taken.

- **New server:** **fixed**: the request fails and nothing is taken

### The monster speed packet has no monster id {#mob-speed-id}

- **Symptom:** when a monster gives up a chase and goes home, the speed change sent to the players around (`0x0609`) is built on the
  stack without the monster's id, so it carries whatever the stack held.
- **To reproduce:** pull a monster until it gives up and runs home; its speed is not updated on the screens around (what the client
  does with the leftover id was not checked).
- **Where:** `CMob::SendSpeed` 0x4906d0 writes only the opcode and the two speeds; called from `CMob::ResetAttack` (see
  [Monsters fighting back](mob-fight.md)).
- **Fix:** kit [`mob-speed-id`](../server-patches.md#mob-speed-id), verified in disassembly: the packet carries the monster's id.

- **New server:** **fixed**: the packet carries the monster's id

### Positions that are not numbers pass the map check {#nan-pos}

- **Symptom:** the check that keeps players inside the map (a position outside closes the connection) lets NaN coordinates through,
  because of how its x87 comparisons are written.
- **To reproduce:** a modified client sends a NaN position.
- **Where:** `CMap::IsSafePos` 0x415360, used on entering a zone and on every move (see [Maps and NPCs](maps-npcs.md)).
- **Fix:** kit [`nan-pos`](../server-patches.md#nan-pos), verified in disassembly: NaN is refused.

- **New server:** **fixed**: NaN is refused

## Chat

### A colour marker that crashes clients {#chat-markup-filter}

- **Symptom:** the client draws `<<` + three digits + `c` + two digits as a colour or item-link marker for the next NNN bytes (the
  client itself adds one for `/chatcolor` and for item names). A marker whose NNN runs past the end of the text crashes every client
  that displays the line, the sender's included, and the original server relays it.
- **To reproduce:** type `<<999c01` in normal chat (seen crashing clients on a test server).
- **Where:** a client bug (the marker is parsed by the client's text drawing); the server relays player text in `CUser::PacketChatE`
  0x470490 (`11xx`), `CUser::PacketAdminChat` 0x470e90 (`F1xx`), the GM notices in `CUser::PacketAdminCmdD` 0x473970 (`F906` to
  `F90B`), memos in `CUser::PacketComm` 0x475870 and shop titles in `CUser::PacketMyShop` (see
  [Packet handling](../client/client-packets.md)).
- **Fix:** kit [`chat-markup-filter`](../server-patches.md#chat-markup-filter), verified in disassembly and emulation: in each of those
  paths the text is checked in place before it is copied; every well-formed marker that fits stays, and the first `<` of any other
  `<<` becomes a space, so the length is unchanged. Which other windows (memos, the shop sign, notices) parse markers is not confirmed
  from the client; filtering them is a precaution.

- **Further references:** `<<NNNcCC`.

- **New server:** **fixed**: the server drops chat whose markers are not well formed (`chat::markup_is_safe`), without echoing it

## Trades and duels

### A trade or duel stake that shrank duplicates the item {#exchange-count-guard}

- **Symptom:** the trade reads each offered bag place again only when it completes. When the count offered is not the count there, it
  makes a new item of the offered count for the receiver and subtracts that count from the giver's, without checking that many are
  still there. A count lowered after the offer wraps round: offer 5 of a stack, let it drop to 3, and the receiver gets 5 while the
  giver keeps 254. The duel's stakes window does the same.
- **To reproduce:** lower the count of an offered stack without ending the trade, then complete it. Most item actions end the trade
  first (moves, drops, shops, item use, gems, quests, the market) or empty the offered slot (the warehouse, the guild bank), and no
  concrete way to lower the count mid-trade has been confirmed in this build; a delayed item use (one consumed after its own
  cancel, such as a recall's timer) is the likeliest candidate, not tried.
- **Where:** `CUser::ExchangeItemAdd` 0x46f910, the subtraction at 0x46fb3b; `CUser::ExchangePvP` 0x47caf0, at 0x47cd29 (see
  [Trade](trade.md) and [Duels](duel.md)).
- **Fix:** kit [`exchange-count-guard`](../server-patches.md#exchange-count-guard), verified in disassembly and emulation: a slot whose
  count is now below the count offered is skipped, as one whose item is gone.

- **Further references:** `SConnection::Send`, 0x4ED0E0, 0x4D4D20, shaiya-episode-6 (github.com/kurtekat/shaiya-episode-6).

- **New server:** **fixed by design**: one transaction for the whole exchange, every offered item checked again, the start taken once under a lock, oneself refused

### The rest of the trade's weak points {#trade-other}

- **Saving is not one step.** Money and each item go to the database agent as separate messages, so a server crash or a lost database
  connection part-way through leaves a half-done trade, which can leave both players with an item. This needs the database agent to
  write a trade in one transaction; it cannot be patched in `ps_game.exe`.
- **Both pressing ready at once** cannot run the exchange twice in this build, as far as the code shows: the network threads only
  queue packets, and each zone's packets are handled one after another on its world thread (`CWorldThread::Update` →
  `CZone::UpdatePackets` → `CUser::UpdatePackets`), and two traders are always in one zone.
- **Trading with oneself** is undone when the request is answered.
- The "trade exploit fix" in `ps_gametablas.CT` is for a later episode, whose trade has an extra confirm step (`0x0A0A`); EP4.5 already
  clears both ready flags on every change to an offer.

## Text that reaches the database

In each of these, player text goes into a database statement without its apostrophes escaped. The kit's five filters replace each
apostrophe (byte `0x27`) with a space before the text is used, so `O'Neil` becomes `O Neil`; lengths and the other name rules are
unchanged. They cover these requests in this `ps_game.exe` only. They do not clean names already stored, and they do not protect the
database agent (`ps_dbAgent`) or the game log (`ps_gameLog`) from direct connections: keep those services unreachable from outside.

### Character names {#character-name-filter}

- **Symptom:** an apostrophe in a new character's name, or in a rename, reaches the database unescaped.
- **Where:** the character create and rename requests in `CUser::PacketCharacter`, hooks at 0x46c006 and 0x46c173.
- **Fix:** kit [`character-name-filter`](../server-patches.md#character-name-filter), verified in disassembly and emulation.

- **New server:** **fixed by design**: values are bound parameters

### Guild names and remarks {#guild-text-filter}

- **Symptom:** an apostrophe in a new guild's name or remark reaches the database unescaped. (Changing the remark later refuses an
  apostrophe; with the filter it becomes a space and is accepted.)
- **Where:** `CUser::PacketGuild`, hooks at 0x476507 (create) and 0x477102 (remark).
- **Fix:** kit [`guild-text-filter`](../server-patches.md#guild-text-filter), verified in disassembly and emulation.

- **New server:** see [character-name-filter](#character-name-filter)

### Gift target names {#gift-target-filter}

- **Symptom:** an apostrophe in the name of the character a gift is sent to reaches the database unescaped.
- **Where:** `CUser::PacketPoint`, hook at 0x479f4a.
- **Fix:** kit [`gift-target-filter`](../server-patches.md#gift-target-filter), verified in disassembly and emulation.

- **New server:** see [character-name-filter](#character-name-filter)

### GM notices {#gm-notice-filter}

- **Symptom:** an apostrophe in the text of a GM notice or command detail reaches the game log's database unescaped.
- **Where:** `CUser::SendLogAdmin` 0x471bd0, hook at 0x471ca8. The filter changes only the copy sent to the game log; the notice
  players see keeps its text.
- **Fix:** kit [`gm-notice-filter`](../server-patches.md#gm-notice-filter), verified in disassembly and emulation. Earlier community
  fixes for the same problem: `fix sql injection gm commands.CT` (author not named), which replaces apostrophes with spaces, and
  `Prevent SQL Injection.CT` by **Cups**, which skips the database call of the notice commands.

- **Further references:** `0x0C03`, `0x0C02`, 0x404820.

- **New server:** **fixed by design**: the new server uses bound parameters

### GM command target names {#gm-target-filter}

- **Symptom:** an apostrophe in the target name a GM types into a command (a kick, a summon, a whisper notice and others) reaches the
  game log's database unescaped.
- **Where:** `CUser::SendLogAdmin` 0x471ae0 and 0x471bd0 (14 and 26 callers), hooks at 0x471b94 and 0x471c94. Only the copy sent to the
  game log changes.
- **Fix:** kit [`gm-target-filter`](../server-patches.md#gm-target-filter), verified in disassembly and emulation.

- **New server:** see [gm-notice-filter](#gm-notice-filter)

### What else reaches the game log {#game-log-texts}

The game log server (`ps_gameLog.exe`) puts every text it is sent into its statements in quotes, without escaping, except chat, whose
apostrophes it turns into `_`. With the five filters, every text `ps_game.exe` sends it that a player or GM can choose is filtered or
escaped: chat by the log server itself, the whisper target because the game sends the found player's own name, names at creation,
GM details and targets. Two things remain: names stored with an apostrophe before the filters (check once, for example
`SELECT CharName FROM PS_GameData.dbo.Chars WHERE CharName LIKE '%''%'`, and the same for guild names), and account names, which come
from the server's own registration. So a separate patch for `ps_gameLog.exe` is not needed, as long as its port is not reachable
from outside.

## Held back until there is a fix

These are known, but the exact steps would mostly help people abuse servers that cannot fix them yet, so only the symptom is given
here.

- **Item mall points.** In some setups the item mall gives items whose points are never taken from the account, and the server keeps
  playing on with them. It cannot be fixed by patching `ps_game.exe` alone: the database agent would have to report the failure and the
  game handle it. Running one world per account pool, and not changing points from outside the game while a player is online, avoids
  it.
  The new server takes the points and gives the items in one database transaction, so the failure cannot happen
  ([Item mall and premium](item-mall.md), section 9).

## Not a bug after all

An earlier version of this page said that party invitation failures name nobody (the player id in `0x0B03` left as stack garbage).
Checking every call of `CUser::SendPartyError` (0x467660) again showed that each one loads the right player id first, so the packet
does carry it in this build.

## Customisations seen in the tables (not bugs)

`ps_gametablas.CT` also changes rules on purpose: staff may attack both factions and see both factions' chat; cross-faction trade
through the auction house, inspect and whisper; resurrection, linking, enchanting, repairing and extracting cost nothing; a union leader
may summon the raid. These are server options, not fixes.

## Credits and other references

The community fixes named above, with their authors. None of their files are hosted here; get them where their authors published
them. Most are Cheat Engine tables that patch the running process; several are for other builds of `ps_game.exe`, which is why each
bug was looked up again in this build.

- **ScreaM:** `GoldBugFix.CT` (NPC shop price overflow; the kit's `npc-shop-price-wrap` makes the same refusal on disk).
- **Cups:** `Ep4 Skillcut Fix.CT` (rune skill cut), `Prevent SQL Injection.CT` (GM notices); also ShaiyaLibrary, a C++ library
  injected into `ps_game.exe` (other builds, and an Ep8 project) with struct layouts, used only to cross-check facts found in this
  build (no licence file, so nothing was copied), and the Cheat Engine structure file `Shaiya.CSX`, whose early `CUser` offsets agree
  with this build (character id `0x128`, level `0x136`, money `0x140`) and later ones do not.
- **Bowie** (kreon800): the Stealth item-move code cave.
- **Eric-Dutra16** and **andr3y_you96:** the helmet and cape drop fix in `ps_gametablas.CT`.
- **Shen1l:** `Shen1l_Bugfix.CT`, scripts for a later build (it patches `pet.dll`, which EP4.5 does not have); nothing in it maps onto
  this build.
- **Forum reports that could not be verified.** Searches of elitepvpers and RageZone turned up fixes and bug reports that the forums would
  not open in full, so they are listed unconfirmed: a "Multibuff / DEX / 4× WIS" patched `ps_game` (BlackoutDude, 2015) with no visible
  details; the "enchant overflow" crash and nubness's fix (2016), said there to be for the EP4.5 `ps_game.exe`, but the exact packet was
  never published; a packet-based dupe via a fixed `ps_dbAgent.exe` (LPH / lilprohacker), reported by others as not actually stopping the
  dupe; a friend-request crash and "same ItemUID" duplication named in a bug-request thread. None of these was reproduced or confirmed
  against this build; where the mechanism matched something found independently (the Stealth drift, the bound-item holes) it is written up
  above from the code.
- **Later-build tables.** `Shen1l_Bugfix.CT` hooks `ps_game.exe+86319`, `+40EBD`, `+8C741` and `pet.dll+7624` (a "Packet Market", a
  battle-trade visual-slot script, an Archer/Hunter LUC tweak); the offsets and `pet.dll` show it is for a later build, and its
  battle-trade `SConnection::Send` is `0x4ED0E0` (this build: `0x4D4D20`), as with `ps_gametablas.CT`. Kept as research only.
- prof_shaiyafiles: a staff command list (`GMCodes.pdf`), an item spreadsheet, SQL procedures and a PHP management panel.

## Other episodes

The EP3 and EP5.4 builds of `ps_game` (EP3 linked 2008-01-17; EP5.4 2011-09-14, modified after linking), `ps_dbAgent` (EP3
2008-01-04; EP5.4 2011-09-14, modified after linking) and `ps_gameLog` (EP3 2007-12-07; EP5.4 2010-10-18) were checked for the bugs on
this page, read statically; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). "Present" means the same defect is there;
"cannot occur" means the request does not exist in that build. Every address is in the named build's `ps_game` unless another binary is
named. The post-link changes to EP5.4 `ps_game` apply none of these fixes.

| fix | EP3 | EP5.4 |
|---|---|---|
| `npc-shop-price-wrap` | present (inferred, same 32-bit `imul` at 0x451e70) | present (confirmed, 0x479871) |
| `npc-shop-range` | not checked | present (confirmed: the NPC is found anywhere in the zone, case 0x4796ae) |
| `myshop-visit-cleanup` | not checked | not checked |
| `myshop-price-wrap` | present (confirmed, 0x45e4dc) | present (confirmed, 0x4878a8); a new check refuses a sale that would overflow the seller's gold (0x4878e8) |
| `bound-trade` | present (confirmed, `ExchangeItemAdd` 0x455bf0) | present (inferred) |
| `bound-duel` | not checked | present (inferred: the stake checks read only real type, count and slot, 0x48c565–0x48c5c1) |
| `bound-shop` | not checked | present (inferred from the compares in the place case of `PacketMyShop` 0x487370) |
| `guild-bank-bound` | present (inferred, 0x42b170) | present (inferred, 0x435bc0) |
| `cloak-lapis` | present (confirmed, 0x4490c5) | present (0x46cae3) |
| `cloak-lapisia` | cannot occur (no `0x0805`) | present (0x46c9f5) |
| `enchant-material-bounds` | cannot occur | fixed (0x46cbf5–0x46cc04) |
| `enchant-seed` | cannot occur | present (0x46ccf9) |
| `remake-distinct` | cannot occur | fixed (0x46da24) |
| `blacksmith-range` | present (inferred, `PacketGem` 0x452610) | present (inferred) |
| `op-exclusive` | present (confirmed, 0x449537) | partly: a new item special 69 hammer links for sure (0x46e776); the 2000 % hammers are still capped |
| `repair-shield-slot` | present (confirmed, 0x44c0c8) | present (confirmed, 0x47208e) |
| `helmet-cloak-drop` | the EP3 behaviour: drops and boxes refuse real types 16 and 24 (confirmed, 0x48763a, 0x44db7a) | not located; boxes refuse real type 24 only (0x473bbd) |
| `rune-skill-cut` | not checked | not checked |
| `learn-skill-level` | not checked | present (confirmed, 0x49354f) |
| `stealth-equip` | not checked | gone: the shape tests were removed (confirmed) |
| `time-damage-player` | present (inferred, same constants in `CUser::TimeDamage` 0x46abf0) | present (confirmed, 0x495e2a) |
| `low-hp-rows` | not checked | unchanged (inferred, 0x497050) |
| `block-buff-end` | not checked | present (confirmed, 0x4949b4, 0x4949dd) |
| `fire-thorns-aggro` | not checked | present (confirmed, 0x4580d0) |
| `illusion-save` | not checked | present (inferred, 0x47ad70) |
| `illusion-newcomers` | not checked | not checked |
| `party-kick-leader` | not checked | partly fixed: the leader's index moves down (confirmed, 0x450324); the loot turn does not |
| `party-exp-levelup` | not checked | present (confirmed, 0x46517b) |
| `quest-full-bags` | not checked | not checked |
| `quest-end-npc` | not checked | present (confirmed, 0x48e6c6, 0x48e754) |
| `quest-npc-range` | not checked | present (confirmed, `CZone::FindNpc` 0x41e460 has no distance) |
| `gatekeeper-logout` | not checked | present (confirmed, 0x476db0) |
| `mob-speed-id` | present (confirmed, `CMob::SendSpeed` 0x4745f0) | present (confirmed, 0x4a1050) |
| `nan-pos` | present (inferred: `CMap::IsSafePos` 0x412190 is identical to EP4.5's) | fixed: `0x0501` and `0x0220` refuse non-finite positions (confirmed, 0x478514, 0x477555); EP5.4 ps_dbAgent also zeroes a non-finite position before saving (confirmed, EP5.4 ps_dbAgent 0x427160) |
| `chat-markup-filter` | not checked | not checked |
| `exchange-count-guard` | present (confirmed, 0x455e37) | fixed for duel stakes (confirmed, 0x48bacd); present in the trade (confirmed, 0x47e81a) |
| `trade-other` | not checked | fixed: trading with oneself, or while in a duel or with a shop open, is refused (0x47de00) |
| `character-name-filter`, `guild-text-filter`, `gift-target-filter` | present (confirmed: EP3 ps_dbAgent writes the same names into `'%s'` unescaped) | present (confirmed: the EP5.4 ps_dbAgent statements are unchanged; the guild remark still refuses only `'`, 0x485c63, and creation checks nothing) |
| `gm-notice-filter`, `gm-target-filter`, `game-log-texts` | present (confirmed: EP3 ps_gameLog escapes only chat) | present (confirmed: EP5.4 ps_gameLog escapes only chat) |

Held back: the item-mall points. EP5.4 still hands the items over before the points are confirmed, but its billing answer can take them
back by a purchase number (EP5.4 ps_game 0x407386–0x407692; confirmed), and its DB agent gained a gift rollback procedure. That path
needs a billing service the pack does not configure.

Login server ([`ps_login.md`](ps_login.md) section 13): EP5.4 `ps_login` requires a key-reply length of exactly 0x80 (EP5.4 ps_login
0x404542), which removes the heap write that EP3 and EP4.5 have, and changes the login query's quotes, which narrows but does not close
the SQL injection (confirmed for the bytes).
