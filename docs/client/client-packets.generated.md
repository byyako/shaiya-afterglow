# Packet catalogue (generated)

This page lists every packet the client `game.exe` (US build 2010-04-26, patch 108) handles from the server and every packet it sends,
with the fields it reads or writes. It was generated from Ghidra dumps of the client, so it shows what the code reads, not what each
field means; the notes and confirmed details are on [Packet handling](client-packets.md), [World packets](client-world-packets.md) and
[Social packets](client-social-packets.md).

## In short

- **425 opcodes come in, 245 go out.** Each table row is one opcode, with the client function that handles or builds it.
- **"Client reads"** is the order of the reads the handler makes. The fixed size is given where the reads add up to one.
- **The two right-hand columns** compare with other sources: the name a public EP6 project gives the packet, and the size of the matching
  struct in the EP4.5 server's PDB (✓ when it agrees).
- **Sizes** include the 2-byte opcode and exclude the 2-byte length prefix.

Field notation: `u8/u16/u32/f32/u64/bytes[n]` in read order; `*` = read-up-to (variable tail); `loop:`/`if:` = read inside a loop or
condition (count or presence depends on earlier fields); `var(x)` = size from an expression.

## Server → client: 425 opcodes handled

Main dispatcher `FUN_00582e10`; opcodes it does not list go to the network object's fallback (vtable `0x6A5C20` slot
`0x600`, `FUN_00551030`), which handles `0x0114` itself and passes families `0x1Cxx` / `0x1Dxx` to sub-dispatchers.

| opcode | handler | client reads | fixed size | EP6 name | EP4.5 PDB size |
|---|---|---|---|---|---|
| `0x0002` | `FUN_0054c7e0` | — (no reads) |  |  |  |
| `0x0004` | `FUN_00578ed0` | u8 | 3 |  |  |
| `0x0101` | `FUN_00578f00` | u8 u32 if:u32 if:u16 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u64 if:u64 if:bytes[21]* if:bytes[6] | var | GameCharListOutgoing (TP_CHAR_LIST) | 83 |
| `0x0102` | `FUN_005791c0` | u8 | 3 |  |  |
| `0x0103` | `FUN_005791f0` | u8 u32 | 7 |  |  |
| `0x0104` | `FUN_00579230` | u8 u32 | 7 | GameUserEnterWorldOutgoing |  |
| `0x0105` | `FUN_00579270` | u16 u16 u16 u16 u16 u16 u32 u32 u32 u32 u32 u32 u32 u32 u32 u32 u32 bytes[25]* | var |  |  |
| `0x0106` | `FUN_005793d0` | u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:bytes[21]* | var |  |  |
| `0x0107` | `FUN_004015a0` | — (no reads) |  | GameUserLeaveWorldOutgoing |  |
| `0x0108` | `FUN_005796c0` | u8 loop:u16 loop:u8 loop:u8 loop:u32 | var |  |  |
| `0x0109` | `FUN_00579780` | u8 u8 | 4 | GameCharCountryOutgoing (TP_CHAR_COUNTRY) | 4 ✓ |
| `0x010a` | `FUN_0057d010` | u8 var((local_b04 & 0xff) * 0xb) | var |  |  |
| `0x010b` | `FUN_0057d070` | u8 var((local_504 & 0xff) * 5) | var |  |  |
| `0x010d` | `FUN_0054c900` | — (no reads) |  |  |  |
| `0x010e` | `FUN_00579880` | u8 | 3 | GameCharRenameOutgoing (TP_CHAR_RENAME) | 3 ✓ |
| `0x010f` | `FUN_0057d0d0` | u8 u32 | 7 |  |  |
| `0x0110` | `FUN_00579800` | u8 | 3 |  |  |
| `0x0111` | `FUN_00579830` | u32 bytes[32] | 38 |  |  |
| `0x0112` | `FUN_005797c0` | u32 u16 | 8 |  |  |
| `0x0113` | `FUN_0054ca40` | — (no reads) |  |  |  |
| `0x0114` | `FUN_00551030` | u32 | 6 | GameCharServerTimeOutgoing (TP_CHARACTER_SERVERTIME) | 6 ✓ |
| `0x0201` | `FUN_005798e0` | u32 u8 u16 u32 u32 u32 u32 u32 | 29 |  |  |
| `0x0202` | `FUN_00579990` | u32 | 6 |  |  |
| `0x0204` | `FUN_00579b70` | u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 bytes[21]* u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 bytes[21]* if:u32 if:u32 if:u32 | var | GameItemBagToBagOutgoing |  |
| `0x0205` | `FUN_0057a190` | u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:bytes[21]* | var | GameItemGetOutgoing (TP_ITEM_GET) | 36 |
| `0x0206` | `FUN_0057a300` | u8 u8 u8 u8 u8 | 7 | GameItemDropOutgoing (TP_ITEM_DROP) | 7 ✓ |
| `0x0207` | `FUN_0057a380` | if:u32 if:u32 u32 | var |  |  |
| `0x0208` | `FUN_0057a400` | u16 u16 u16 u16 u16 u16 | 14 |  |  |
| `0x0209` | `FUN_0057a490` | u8 u8 u16 u8 | 7 | GameUserSkillGetOutgoing (TP_USER_GETSKILL) | 7 ✓ |
| `0x020a` | `FUN_00582ad0` | u8 u8 | 4 | GameUserPortalUseOutgoing (TPC_USER_USEPORTAL) | 4 ✓ |
| `0x020b` | `FUN_0057a5c0` | u32 u16 u32 u32 u32 | 20 | GameUserSetMapPosOutgoing (TP_USER_SETMAPPOS) | 20 ✓ |
| `0x020c` | `FUN_00580630` | u8 u32 | 7 | GameUserGateKeeperUseOutgoing (TP_USER_USEGATENPC) | 7 ✓ |
| `0x020d` | `FUN_0057a640` | u8 u32 | 7 |  |  |
| `0x020e` | `FUN_0057a690` | u8 u32 | 7 | GameUserKillCountOutgoing (TP_USER_KILLCOUNT) | 7 ✓ |
| `0x020f` | `FUN_0057a6e0` | u8 u32 | 7 |  |  |
| `0x0210` | `FUN_0057a500` | u8 | 3 | GameUserSetRunOutgoing (TP_USER_SETRUN) | 3 ✓ |
| `0x0211` | `FUN_0057a530` | u8 u32 u32 | 11 |  |  |
| `0x0212` | `FUN_0054fb60` | — (no reads) |  |  |  |
| `0x0213` | `FUN_0057a590` | u32 | 6 | GameUserSetMoneyOutgoing (TP_USER_SETMONEY) | 6 ✓ |
| `0x0214` | `FUN_00580680` | u8 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 if:u16 | var |  |  |
| `0x0215` | `FUN_00580740` | u8 if:u16 if:u8 | var |  |  |
| `0x0216` | `FUN_00580ba0` | u8 u8 | 4 |  |  |
| `0x0217` | `FUN_00580bf0` | u32 | 6 |  |  |
| `0x0218` | `FUN_00580cd0` | u8 u8 | 4 | GameStatusResultInfoOutgoing (TP_STATUS_RESULTINFO) | 4 ✓ |
| `0x0219` | `FUN_00580d20` | u8 u8 | 4 | GameStatusResultKillOutgoing (TP_STATUS_RESULTKILL) | 4 ✓ |
| `0x021a` | `FUN_00580d70` | u8 u32 | 7 | GameStatusResultDeathOutgoing (TP_STATUS_RESULTDEATH) | 7 ✓ |
| `0x021b` | `FUN_005810b0` | u8 u8 | 4 |  |  |
| `0x021c` | `FUN_00580c20` | u32 u32 | 10 |  |  |
| `0x021d` | `FUN_00580c70` | u32 | 6 |  |  |
| `0x021e` | `FUN_00580ca0` | u8 | 3 |  |  |
| `0x0220` | `FUN_00581100` | u8 u8 u16 u32 u32 u32 | 18 |  |  |
| `0x0221` | `FUN_00581190` | u32 | 6 | GameUserItemCastOutgoing |  |
| `0x0222` | `FUN_005811c0` | u32 u8 | 7 |  |  |
| `0x0223` | `FUN_00581210` | u32 | 6 |  |  |
| `0x0224` | `FUN_00581240` | u32 u8 | 7 |  |  |
| `0x0225` | `FUN_00581900` | u8 loop:bytes[15] | var |  |  |
| `0x0226` | `FUN_0057b1b0` | u32 u8 u8 u8 u8 | 10 | GameUserBodyChangeOutgoing (TP_BODYCHG) | 10 ✓ |
| `0x0228` | `FUN_00557c90` | — (no reads) |  |  |  |
| `0x0230` | `FUN_005798b0` | u16 | 4 | GameCharHonorOutgoing (TP_CHARACTER_HONOR) | 4 ✓ |
| `0x0301` | `FUN_0057a730` | u32 u16 u16 u8 u8 | 12 |  |  |
| `0x0302` | `FUN_0057a860` | u32 u16 u16 | 10 | GameGetInfoUserHealthOutgoing (TP_USER_GETHP) | 10 ✓ |
| `0x0303` | `FUN_005799c0` | u32 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u32 loop:u8 loop:u8 loop:u8 bytes[21]* if:bytes[6] bytes[25]* | var | GameGetInfoUserShapeOutgoing_EP5 (TP_USER_GETSHAPE) | 97 |
| `0x0304` | `FUN_0057a8c0` | u32 u32 u8 u8 | 12 |  |  |
| `0x0305` | `FUN_0057a930` | u32 u32 | 10 | GameGetInfoMobHealthOutgoing (TP_MOB_GETHP) | 10 ✓ |
| `0x0307` | `FUN_0057a7b0` | u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:bytes[6] loop:bytes[21]* | var | GameGetInfoUserItemsOutgoing (TP_USER_ITEMS) | 419 |
| `0x0401` | `FUN_0057a980` | u32 u8 u8 u8 u8 u32 u32 u32 | 22 | GameItemZoneEnterOutgoing (TP_ITEM_ENTER) | 22 ✓ |
| `0x0402` | `FUN_0057aa40` | u32 | 6 | GameItemZoneLeaveOutgoing |  |
| `0x0403` | `FUN_0057aa70` | u8 u8 | 4 |  |  |
| `0x0404` | `FUN_0057aac0` | u32 | 6 |  |  |
| `0x0405` | `FUN_0057d110` | u32 u32 u32 u32 u16 u8 u8 u8 u8 u8 u8 u8 u64 u64 bytes[21]* | var |  |  |
| `0x0406` | `FUN_0057d250` | u32 | 6 |  |  |
| `0x0451` | `FUN_0057aaf0` | u8 u8 u8 | 5 |  |  |
| `0x0501` | `FUN_0057ab50` | u32 u8 u16 u32 u32 u32 | 21 | GameCharMoveOutgoing (TP_PC_MOVE) | 21 ✓ |
| `0x0502` | `FUN_0057aca0` | u8 u32 u32 u16 u16 u16 | 17 | GameCharAttackUserOutgoing (TP_PC_ATTACK) | 17 ✓ |
| `0x0503` | `FUN_0057ad30` | u8 u32 u32 u16 u16 u16 | 17 | GameCharAttackMobOutgoing (TP_PC_ATTACK) | 17 ✓ |
| `0x0504` | `FUN_0057adc0` | u32 u8 u32 | 11 | GameUserDeathOutgoing (TP_DEATH) | 11 ✓ |
| `0x0505` | `FUN_0057ae20` | u32 u16 u16 u16 | 12 | GameUserRecoverOutgoing (TP_USER_RECOVER) | 12 ✓ |
| `0x0506` | `FUN_0057ae90` | u32 u8 | 7 | GameCharMotionOutgoing (TP_PC_MOTION) | 7 ✓ |
| `0x0507` | `FUN_0057aee0` | u32 u8 u8 u8 u8 if:bytes[6] | var | GameCharEquipmentOutgoing (TP_PC_EQUIPMENT) | 16 |
| `0x0508` | `FUN_0057af90` | u32 u16 u16 u16 u32 u32 | 20 |  |  |
| `0x0509` | `ignored` | ignored (explicit no-op case) |  |  |  |
| `0x050a` | `FUN_0057b120` | u32 u8 u8 u8 u8 u8 | 11 | GameCharItemUseOutgoing (TP_PC_USEITEM) | 11 ✓ |
| `0x050b` | `FUN_0057b230` | u32 u8 u16 | 9 | GameCharMaxEnergyOutgoing (TP_SET_MAX) | 9 ✓ |
| `0x050c` | `FUN_0057b290` | u16 u16 | 6 |  |  |
| `0x050d` | `FUN_0057b2e0` | u32 u16 u8 | 9 | GameCharSkillSetOutgoing (TP_PC_SKILL_SET) | 9 ✓ |
| `0x050e` | `FUN_0057b340` | u32 | 6 | GameCharSkillRemoveOutgoing (TP_PC_SKILL_REMOVE) | 6 ✓ |
| `0x050f` | `FUN_0057b370` | u32 u16 u8 u16 u16 u16 | 15 | GameUserSkillKeepOutgoing (TP_SKILL_KEEP) | 15 ✓ |
| `0x0510` | `FUN_0057b400` | u32 u32 u16 u8 | 13 |  |  |
| `0x0511` | `FUN_0057b470` | u8 u32 u32 u16 u8 u16 u16 u16 | 20 | GameCharSkillUseOutgoing_EP5 (TP_PC_SKILL_USE) | 20 ✓ |
| `0x0512` | `FUN_0057b590` | u8 u32 u32 u16 u8 u16 u16 u16 u16 u16 u16 | 26 |  |  |
| `0x0513` | `FUN_0057b680` | u8 u32 u32 u16 u8 u16 u16 u16 | 20 |  |  |
| `0x0514` | `FUN_0057b740` | u32 u32 u16 u8 u16 u16 u16 | 19 |  |  |
| `0x0515` | `FUN_0057b7e0` | u32 u32 u16 u16 u16 | 16 |  |  |
| `0x0516` | `FUN_0057b860` | u32 u32 u16 u8 | 13 |  |  |
| `0x0517` | `FUN_0057b8d0` | u8 u32 u32 u16 u8 u16 u16 u16 | 20 |  |  |
| `0x0518` | `FUN_0057b990` | u8 u32 u32 u16 u8 u16 u16 u16 u16 u16 u16 | 26 |  |  |
| `0x0519` | `FUN_0057ba80` | u8 u32 u32 u16 u8 u16 u16 u16 | 20 |  |  |
| `0x051a` | `FUN_0057bb40` | u32 u32 u16 u8 u16 u16 u16 | 19 |  |  |
| `0x051b` | `FUN_0057bbe0` | u32 u32 u16 u16 u16 | 16 |  |  |
| `0x051c` | `FUN_0057ccd0` | u32 u8 u8 | 8 |  |  |
| `0x051d` | `FUN_0057d280` | u32 u8 | 7 | GameCharShapeOutgoing_EP5 (TP_PC_SHAPE) | 7 ✓ |
| `0x051e` | `FUN_0057b020` | u32 u16 u16 u16 u32 u32 | 20 |  |  |
| `0x051f` | `FUN_0057b0b0` | u32 u16 u16 u16 | 12 |  |  |
| `0x0520` | `FUN_0057bc60` | u32 u8 | 7 |  |  |
| `0x0521` | `FUN_00580090` | u16 u16 u16 | 8 | GameUserRecoverSetOutgoing (TP_USER_RECOVERSET) | 8 ✓ |
| `0x0522` | `FUN_005800f0` | u32 u32 | 10 | GameUserKillInfoOutgoing (TP_USER_KILLINFO) | 10 ✓ |
| `0x0523` | `FUN_005807c0` | u32 u8 if:u8 if:var(local_85)* | var |  |  |
| `0x0524` | `FUN_0057d2d0` | u32 u8 u16 | 9 |  |  |
| `0x0525` | `FUN_00581290` | u16 | 4 |  |  |
| `0x0551` | `FUN_0057d330` | u32 u8 u32 u32 u32 u32 | 23 |  |  |
| `0x0554` | `FUN_0057ac10` | u8 u8 u32 | 8 | GameItemRepairOutgoing (TP_ITEM_REPAIR) | 8 ✓ |
| `0x0555` | `FUN_0057ac70` | u16 | 4 |  |  |
| `0x0558` | `FUN_0057b530` | u32 u16 u8 | 9 |  |  |
| `0x0601` | `FUN_0057c0b0` | u32 u8 u16 u32 u32 | 17 |  |  |
| `0x0602` | `FUN_0057c130` | u32 | 6 |  |  |
| `0x0603` | `FUN_0057c160` | u32 u8 u32 u32 | 15 |  |  |
| `0x0604` | `FUN_0057c1d0` | u32 | 6 |  |  |
| `0x0605` | `FUN_0057c200` | u8 u32 u32 u16 u16 u16 | 17 | GameMobAttackCharOutgoing (TP_MOB_ATTACKPC) | 17 ✓ |
| `0x0606` | `FUN_0057c290` | u32 u8 u32 | 11 |  |  |
| `0x0607` | `FUN_0057c350` | u32 u16 u8 u16 u16 u16 | 15 |  |  |
| `0x0608` | `FUN_0057c2f0` | u32 u32 u32 | 14 |  |  |
| `0x0609` | `FUN_0057cd30` | u32 u8 u8 | 8 |  |  |
| `0x060b` | `FUN_0057bd00` | u8 u32 u32 u8 u16 u8 u16 u16 u16 | 21 |  |  |
| `0x060c` | `FUN_0057bdd0` | u8 u32 u32 u8 u16 u8 u16 u16 u16 u16 u16 u16 | 27 |  |  |
| `0x060d` | `FUN_0057bed0` | u8 u32 u32 u16 u8 u16 u16 u16 | 20 |  |  |
| `0x060e` | `FUN_0057bf90` | u32 u32 u16 u8 u16 u16 u16 | 19 |  |  |
| `0x060f` | `FUN_0057c030` | u32 u32 u16 u16 u16 | 16 |  |  |
| `0x0610` | `FUN_0057bcb0` | u32 u32 | 10 | GameMobRecoverOutgoing (TP_MOB_RECOVER) | 10 ✓ |
| `0x0701` | `FUN_0057c3e0` | u8 loop:u8 loop:u8 | var |  |  |
| `0x0702` | `FUN_0057c450` | u8 u8 u8 u8 u8 u8 u32 | 12 |  |  |
| `0x0703` | `FUN_0057c4f0` | u8 u8 u8 u8 u8 u8 u32 | 12 | GameShopSellOutgoing (TP_SHOP_SELL) | 12 ✓ |
| `0x0711` | `FUN_0057c590` | u32 u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:bytes[21]* | var | GameCharBankOutgoing (TP_CHAR_BANK) | 1657 |
| `0x0801` | `FUN_0057c930` | u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u8 u32 | 18 |  |  |
| `0x0802` | `FUN_0057cb40` | if:u8 if:u8 if:u8 if:bytes[6] if:bytes[6] if:bytes[6] if:bytes[6] if:u32 u8 u8 u8 u8 bytes[6] bytes[6] bytes[6] bytes[6] u32 | var |  |  |
| `0x0803` | `FUN_00581e30` | u8 u8 u8 bytes[6] u32 | 15 | GameCloakInfoAddOutgoing (TP_CLOAK_ADD) | 15 ✓ |
| `0x0804` | `FUN_00581eb0` | u8 u8 | 4 | GameCloakInfoRemoveOutgoing (TP_CLOAK_REM) | 4 ✓ |
| `0x0805` | `FUN_0057ca50` | u8 u8 u8 u8 u8 u8 u8 u8 u8 u32 bytes[21]* | var | GameLapisianAddOutgoing (TP_LAPISIAN_ADD) | 36 |
| `0x0806` | `FUN_0057c6d0` | u8 u8 u8 bytes[21]* | var | GameItemComposeOutgoing (TP_ITEM_COMPOSE) | 26 |
| `0x0807` | `FUN_0057c740` | u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | var | GameItemRemakeOutgoing (TPC_ITEM_REMAKE) | 8 |
| `0x0808` | `FUN_0057c890` | u8 if:u8 if:u8 if:bytes[21]* | var | GameItemRemake2Outgoing |  |
| `0x0901` | `FUN_0057cdf0` | u8 var((local_704 & 0xff) * 7) | var |  |  |
| `0x0902` | `FUN_0057ce50` | u32 u16 | 8 | GameQuestStartOutgoing (TP_QUEST_START) |  |
| `0x0903` | `FUN_0057cea0` | u32 u16 u8 u8 u32 u32 u8 u8 u8 u8 | 22 | GameQuestEndOutgoing_EP5 (TP_QUEST_END) | 22 ✓ |
| `0x0905` | `FUN_0057cf80` | u16 u8 u8 | 6 |  |  |
| `0x0906` | `FUN_0057cd90` | u8 if:var((local_304 & 0xff) * 3) | var |  |  |
| `0x0907` | `FUN_0057cfe0` | u16 | 4 | GameQuestEndSelectOutgoing (TP_QUEST_END_SELECT) | 4 ✓ |
| `0x0a01` | `FUN_0057de80` | u32 | 6 | GameExchangeRequestOutgoing (TP_EXC_REQUEST) | 6 ✓ |
| `0x0a03` | `FUN_0057deb0` | u32 u8 | 7 |  |  |
| `0x0a04` | `FUN_0057df00` | u8 | 3 |  |  |
| `0x0a05` | `FUN_00581f80` | u8 u8 | 4 | GameExchangeReadyOutgoing (TP_EXC_READY) | 4 ✓ |
| `0x0a06` | `FUN_00581fd0` | u8 u8 u8 u8 | 6 | GameExchangeAddOutgoing (TPC_EXC_ADD) | 6 ✓ |
| `0x0a07` | `FUN_005820e0` | u8 u8 | 4 |  |  |
| `0x0a08` | `FUN_00582130` | u8 u32 | 7 |  |  |
| `0x0a09` | `FUN_00582040` | u8 u8 u8 u8 u16 bytes[6] bytes[21]* | var | GameExchangeAddDestOutgoing_EP5 (TP_EXC_ADD_DEST) | 35 |
| `0x0b01` | `FUN_0057d6d0` | u8 u8 loop:u32 loop:bytes[21] loop:u16 loop:u8 loop:u16 loop:u16 loop:u16 loop:u16 loop:u16 loop:u16 loop:u16 loop:u32 loop:u32 loop:u32 loop:u8 loop:u16 loop:u8 loop:u32 | var |  |  |
| `0x0b02` | `FUN_0057d870` | u32 | 6 |  |  |
| `0x0b03` | `FUN_0057d8a0` | u8 u32 | 7 |  |  |
| `0x0b04` | `FUN_0057d8f0` | u32 bytes[21]* u16 u8 u16 u16 u16 u16 u16 u16 u16 u32 u32 u32 u8 loop:u16 loop:u8 loop:u32 | var |  |  |
| `0x0b05` | `FUN_0057da30` | u32 | 6 |  |  |
| `0x0b06` | `FUN_0057da60` | u32 | 6 |  |  |
| `0x0b07` | `FUN_0057da90` | u32 | 6 |  |  |
| `0x0b08` | `FUN_0057de20` | u32 u8 u8 | 8 |  |  |
| `0x0b09` | `FUN_00578ce0` | bytes[21]* | var |  |  |
| `0x0b0b` | `FUN_005816f0` | u8 u8 u8 u16 u8 u8 u8 loop:u16 loop:u32 loop:bytes[21]* loop:u16 loop:u8 loop:u16 loop:u16 loop:u16 loop:u16 loop:u16 loop:u16 loop:u16 loop:u32 loop:u32 loop:u32 loop:u8 loop:u16 loop:u8 loop:u32 | var |  |  |
| `0x0b0c` | `FUN_005815a0` | u16 u32 bytes[21]* u16 u8 u16 u16 u16 u16 u16 u16 u16 u32 u32 u32 u8 loop:u16 loop:u8 loop:u32 | var |  |  |
| `0x0b0d` | `FUN_005813b0` | u32 | 6 |  |  |
| `0x0b0e` | `FUN_00581330` | u8 u8 u32 u32 u32 | 16 |  |  |
| `0x0b0f` | `FUN_005813e0` | u32 | 6 | GamePartySetDivTypeOutgoing (TP_PARTY_SETDIVTYPE) | 6 ✓ |
| `0x0b10` | `FUN_00581410` | u8 | 3 | GamePartySetAutoJoinOutgoing (TPC_PARTY_SETAUTOJOIN) | 3 ✓ |
| `0x0b12` | `FUN_00581440` | u32 u32 u32 u32 | 18 |  |  |
| `0x0b13` | `FUN_005814e0` | u32 | 6 |  |  |
| `0x0b14` | `FUN_00581570` | u32 | 6 |  |  |
| `0x0b15` | `FUN_0057d8a0` | u8 u32 | 7 |  |  |
| `0x0b16` | `FUN_00578de0` | scene vt 0x51c |  |  |  |
| `0x0b17` | `FUN_005814b0` | u32 | 6 |  |  |
| `0x0b18` | `FUN_00578e00` | bytes[21]* | var |  |  |
| `0x0b19` | `FUN_00581540` | u32 | 6 |  |  |
| `0x0b1a` | `FUN_00581510` | u32 | 6 |  |  |
| `0x0c01` | `FUN_0057dac0` | u32 u8 u16 | 9 |  |  |
| `0x0c02` | `FUN_0057db20` | u32 u8 u16 | 9 |  |  |
| `0x0c03` | `FUN_0057db80` | u32 u16 u16 u16 | 12 |  |  |
| `0x0c04` | `FUN_0057dbf0` | u32 u16 u8 | 9 |  |  |
| `0x0c05` | `FUN_0057dc50` | u32 u16 u8 | 9 |  |  |
| `0x0c06` | `FUN_0057dcb0` | u32 | 6 |  |  |
| `0x0c07` | `FUN_0057dce0` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0x0c08` | `FUN_0057dd60` | u32 u16 u16 u16 | 12 |  |  |
| `0x0c09` | `FUN_0057ddd0` | u32 u16 | 8 |  |  |
| `0x0c0a` | `FUN_0057dac0` | u32 u8 u16 | 9 |  |  |
| `0x0c0b` | `FUN_0057db20` | u32 u8 u16 | 9 |  |  |
| `0x0c0c` | `FUN_0057db80` | u32 u16 u16 u16 | 12 |  |  |
| `0x0c0d` | `FUN_0057dd60` | u32 u16 u16 u16 | 12 |  |  |
| `0x0c0e` | `FUN_0057dbf0` | u32 u16 u8 | 9 |  |  |
| `0x0c10` | `FUN_0057dc50` | u32 u16 u8 | 9 |  |  |
| `0x0c11` | `FUN_0057dcb0` | u32 | 6 |  |  |
| `0x0c12` | `FUN_0057dce0` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0x0c13` | `FUN_0057ddd0` | u32 u16 | 8 |  |  |
| `0x0c14` | `FUN_0057de20` | u32 u8 u8 | 8 |  |  |
| `0x0d01` | `FUN_0057f670` | u8 u32 u8 bytes[25]* | var |  |  |
| `0x0d02` | `FUN_0057f610` | u32 bytes[25]* | var |  |  |
| `0x0d03` | `FUN_0057f730` | u8 | 3 |  |  |
| `0x0d04` | `FUN_0057f700` | u32 | 6 |  |  |
| `0x0d05` | `FUN_0057f760` | u32 | 6 |  |  |
| `0x0d06` | `FUN_0057f790` | u8 u32 | 7 |  |  |
| `0x0d07` | `FUN_0057f7e0` | u8 | 3 |  |  |
| `0x0d08` | `FUN_0057f810` | u8 u32 u8 bytes[25]* | var |  |  |
| `0x0d09` | `FUN_0057f880` | u8 | 3 |  |  |
| `0x0d0a` | `FUN_0057f8b0` | u8 u32 | 7 |  |  |
| `0x0d0b` | `FUN_0057f900` | u32 u8 u8 bytes[21]* | var |  |  |
| `0x0d0c` | `FUN_0057f990` | u8 u32 | 7 | GameGuildUserStatusOutgoing (TP_GUILD_USERSTATE) | 7 ✓ |
| `0x0d0d` | `FUN_005564a0` | — (no reads) |  |  |  |
| `0x0d0e` | `FUN_005a8700` | — (no reads) |  |  |  |
| `0x0d0f` | `FUN_0057f9e0` | u8 var((local_7d4 & 0xff) * 0x32) | var |  |  |
| `0x0d10` | `FUN_0057fa40` | bytes[50] | 52 |  |  |
| `0x0d11` | `FUN_0057fa80` | u32 | 6 |  |  |
| `0x0d12` | `FUN_0057fb10` | u8 var((local_914 & 0xff) * 0x1d) | var |  |  |
| `0x0d13` | `FUN_0057fb70` | u8 var((local_914 & 0xff) * 0x1d) | var |  |  |
| `0x0d14` | `FUN_0057fbd0` | u8 bytes[29] | 32 |  |  |
| `0x0d15` | `FUN_0057fc20` | u32 | 6 |  |  |
| `0x0d16` | `FUN_0057fc50` | u8 var((local_8c4 & 0xff) * 0x1c) | var |  |  |
| `0x0d17` | `FUN_0057fcb0` | bytes[28] | 30 | GameGuildJoinListAddOutgoing (TP_GUILD_JOINLISTADD) | 30 ✓ |
| `0x0d18` | `FUN_0057fcf0` | u32 | 6 |  |  |
| `0x0d19` | `FUN_0057fab0` | u8 var((local_914 & 0xff) * 0x1d) | var |  |  |
| `0x0d20` | `FUN_00582910` | u8 bytes[65]* | var |  |  |
| `0x0d21` | `FUN_00582660` | u8 u32 u8 bytes[25]* bytes[65]* | var |  |  |
| `0x0d22` | `FUN_0057f610` | u32 bytes[25]* | var |  |  |
| `0x0d23` | `FUN_00582960` | u32 | 6 |  |  |
| `0x0d24` | `FUN_00582870` | u8 var((local_84 & 0xff) << 2) | var |  |  |
| `0x0d25` | `FUN_005829e0` | u8 u16 u8 u8 | 7 |  |  |
| `0x0d26` | `FUN_00582770` | u16 u8 | 5 |  |  |
| `0x0d27` | `FUN_005826e0` | u32 u32 u32 | 14 |  |  |
| `0x0d28` | `FUN_00582a50` | u8 u8 | 4 |  |  |
| `0x0d29` | `FUN_005822f0` | u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:bytes[21]* | var | GameGuildBankItemListOutgoing (TP_GUILD_BANK_ITEM_LIST) | 1653 |
| `0x0d2f` | `FUN_005827c0` | u8 var((local_12c4 & 0xff) * 0x78) | var |  |  |
| `0x0d30` | `FUN_005828d0` | bytes[120] | 122 |  |  |
| `0x0d31` | `FUN_00582420` | u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* u32 | var |  |  |
| `0x0d32` | `FUN_00582540` | u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* u32 | var |  |  |
| `0x0d34` | `FUN_00582990` | u8 u32 | 7 |  |  |
| `0x0d35` | `FUN_00582820` | u8 var((local_1e4 & 0xff) << 1) | var |  |  |
| `0x0d36` | `FUN_00582740` | u8 | 3 |  |  |
| `0x0d37` | `FUN_00582b20` | u8 var((local_16c & 0xff) * 9) | var |  |  |
| `0x0d38` | `FUN_00582aa0` | u32 | 6 |  |  |
| `0x0e01` | `FUN_0057e000` | u32 u8 u16 u32 u32 u32 u16 | 23 | GameNpcZoneEnterOutgoing (TP_NPC_ENTER) | 23 ✓ |
| `0x0e02` | `FUN_0057e0a0` | u32 | 6 | GameNpcZoneLeaveOutgoing |  |
| `0x0e03` | `FUN_0057e0d0` | u32 u8 u32 u32 u32 | 19 | GameNpcMoveOutgoing (TP_NPC_MOVE) | 19 ✓ |
| `0x0e04` | `FUN_0057e150` | u32 | 6 |  |  |
| `0x0e05` | `FUN_0057e180` | u8 u32 u32 u16 | 13 |  |  |
| `0x0e06` | `FUN_0057e1f0` | u8 u32 u32 u16 | 13 |  |  |
| `0x0f01` | `FUN_0057df30` | u8 if:u16 if:u16 if:u8 if:u8 if:u8 | var |  |  |
| `0x0f02` | `FUN_0057dfd0` | u8 | 3 |  |  |
| `0x1101` | `FUN_0057e260` | u32 u8 var(local_90 & 0xff)* | var | GameChatNormalOutgoing (TP_CHAT_NORMAL) | 135 |
| `0x1102` | `FUN_0057e2e0` | u8 bytes[21]* u8 var(local_a8 & 0xff)* | var | GameChatSendToOutgoing (TP_CHAT_SENDTO) | 153 |
| `0x1103` | `FUN_0057e380` | bytes[21]* u8 var(local_a4[0] & 0xff)* | var | GameChatTradeOutgoing (TP_CHAT_SENDALL) | 152 |
| `0x1104` | `FUN_0057e410` | bytes[21]* u8 var(local_a4[0] & 0xff)* | var | GameChatGuildOutgoing |  |
| `0x1105` | `FUN_0057e4a0` | u32 u8 var(local_90 & 0xff)* | var | GameChatPartyOutgoing (TP_CHAT_SENDPARTY) | 135 |
| `0x1106` | `FUN_0057e520` | u8 | 3 | GameChatErrorOutgoing (TP_CHATE_ERROR) | 3 ✓ |
| `0x1107` | `FUN_0057e550` | u32 u8 var(local_90 & 0xff)* | var |  |  |
| `0x1108` | `FUN_0057e5c0` | bytes[21]* u8 var(local_a4[0] & 0xff)* | var | GameChatAllOutgoing (TP_CHAT_SENDALL) | 152 |
| `0x1109` | `FUN_0057e650` | u8 u32 u8 var(local_90 & 0xff)* | var |  |  |
| `0x110a` | `FUN_0057e6f0` | u32 u16 | 8 |  |  |
| `0x110b` | `FUN_0057e740` | u32 bytes[32]* | var |  |  |
| `0x1111` | `FUN_0057e7b0` | bytes[21]* u8 var(local_a4[0] & 0xff)* | var | GameChatZoneOutgoing (TP_CHAT_ZONE) | 152 |
| `0x1112` | `ignored` | ignored (explicit no-op case) |  | GameChatUnionOutgoing |  |
| `0x1c01` | `FUN_0055d490` | if:u8 if:u8 if:u8 loop:bytes[48] | var |  |  |
| `0x1c02` | `FUN_0055d490` | if:u8 if:u8 if:u8 loop:bytes[48] | var |  |  |
| `0x1c03` | `FUN_0055d490` | if:u8 if:u8 if:u8 loop:bytes[48] loop:bytes[48] | var |  |  |
| `0x1c04` | `FUN_0055d490` | u8 loop:bytes[52] | var |  |  |
| `0x1c05` | `FUN_0055d490` | u8 loop:bytes[48] | var |  |  |
| `0x1c06` | `FUN_0055d490` | u8 loop:bytes[41] | var |  |  |
| `0x1c07` | `FUN_0055d490` | u8 loop:bytes[53] | var |  |  |
| `0x1c08` | `FUN_0055d490` | u8 if:u8 if:u8 if:u8 if:u32 if:bytes[52] | var |  |  |
| `0x1c09` | `FUN_0055d490` | u8 if:bytes[41] | var |  |  |
| `0x1c0a` | `FUN_0055d490` | u8 if:u32 if:bytes[41] | var |  |  |
| `0x1c0b` | `FUN_0055d490` | u32 u8 if:u32 if:bytes[48] if:u32 | var |  |  |
| `0x1c0c` | `FUN_0055d490` | u8 if:u32 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:bytes[21]* | var |  |  |
| `0x1c0d` | `FUN_0055d490` | u8 if:u32 if:u32 | var |  |  |
| `0x1c0e` | `FUN_0055d490` | bytes[53] | 55 |  |  |
| `0x1c0f` | `FUN_0055d490` | bytes[41] | 43 |  |  |
| `0x1c10` | `FUN_0055d490` | bytes[11] | 13 |  |  |
| `0x1c11` | `FUN_0055d490` | bytes[41] | 43 |  |  |
| `0x1c12` | `FUN_0055d490` | bytes[53] | 55 |  |  |
| `0x1c13` | `FUN_0055d490` | u8 | 3 |  |  |
| `0x1c14` | `FUN_0055d490` | if:u8 loop:bytes[48] | var |  |  |
| `0x1c15` | `FUN_0055d490` | u8 if:bytes[48] | var |  |  |
| `0x1c16` | `FUN_0055d490` | u32 u8 | 7 |  |  |
| `0x1c17` | `FUN_0055d490` | u8 | 3 |  |  |
| `0x1d02` | `FUN_0055bfe0` | u8 u8 u8 | 5 |  |  |
| `0x1d04` | `FUN_0055bfe0` | u8 bytes[13] | 16 |  |  |
| `0x1d06` | `FUN_0055bfe0` | u8 u8 u8 u8 u8 u32 | 11 |  |  |
| `0x1d07` | `FUN_0055bfe0` | u32 bytes[257]* bytes[21]* bytes[13]* u32 u32 u32 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | var |  |  |
| `0x1d0b` | `FUN_0055bfe0` | u16 loop:u32 loop:bytes[257]* loop:bytes[21]* loop:bytes[13]* loop:u32 loop:u32 loop:u32 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:bytes[21]* | var |  |  |
| `0x1d0e` | `FUN_0055bfe0` | u32 u8 | 7 |  |  |
| `0x1d10` | `FUN_0055bfe0` | u8 | 3 |  |  |
| `0x1d11` | `FUN_0055bfe0` | u32 u32 u8 u8 u8 u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | var |  |  |
| `0x2101` | `FUN_0057fd20` | u8 var((local_518 & 0xff) * 0xd) | var |  |  |
| `0x2102` | `FUN_0057fd80` | u32 u8 | 7 |  |  |
| `0x2103` | `FUN_0057fdd0` | u16 u8 u8 | 6 |  |  |
| `0x2104` | `FUN_0057fe30` | u16 u8 u8 | 6 |  |  |
| `0x2201` | `FUN_00580140` | u8 loop:u32 loop:u8 loop:u8 loop:bytes[21]* loop:u8 loop:var(local_3d)* | var |  |  |
| `0x2202` | `FUN_00578db0` | bytes[21]* | var |  |  |
| `0x2203` | `FUN_00580220` | u8 | 3 | GameFriendAnswerOutgoing (TPC_COMM_FRIEND_ANS) | 3 ✓ |
| `0x2204` | `FUN_00580250` | u32 u8 bytes[21]* | var | GameFriendAddOutgoing (TP_COMM_FRIEND_ADD) | 28 |
| `0x2205` | `FUN_005802b0` | u32 | 6 | FriendRemoveOutgoing (TP_COMM_FRIEND_DEL) | 6 ✓ |
| `0x2206` | `FUN_005802e0` | u32 u8 var(local_3c & 0xff)* | var | GameFriendMemoOutgoing (TP_COMM_FRIEND_MEMO) | 58 |
| `0x2207` | `FUN_00580350` | u32 u8 | 7 |  |  |
| `0x2208` | `FUN_005803a0` | u8 loop:u32 loop:bytes[21]* loop:u8 loop:var(local_37)* | var |  |  |
| `0x2209` | `FUN_00580440` | u32 bytes[21]* | var | GameBlockAddOutgoing (TP_COMM_BLOCK_ADD) | 27 |
| `0x220a` | `FUN_00580490` | u32 | 6 | GameBlockRemoveOutgoing (TP_COMM_BLOCK_DEL) | 6 ✓ |
| `0x220b` | `FUN_005804c0` | u32 u8 var(local_3c & 0xff)* | var | GameBlockMemoOutgoing (TP_COMM_BLOCK_MEMO) | 58 |
| `0x220c` | `FUN_00580560` | u8 | 3 |  |  |
| `0x220d` | `FUN_00580590` | u8 loop:u8 loop:u8 loop:bytes[21]* | var |  |  |
| `0x220e` | `FUN_00580530` | u8 | 3 | GameBlockAnswerOutgoing (TP_COMM_BLOCK_ANS) | 3 ✓ |
| `0x2301` | `FUN_005590d0` | — (no reads) |  |  |  |
| `0x2302` | `FUN_00559130` | — (no reads) |  |  |  |
| `0x2303` | `FUN_00559140` | — (no reads) |  |  |  |
| `0x2304` | `FUN_00559170` | — (no reads) |  |  |  |
| `0x2305` | `FUN_00580840` | u8 u8 u8 u32 | 9 |  |  |
| `0x2306` | `FUN_005808b0` | u8 | 3 |  |  |
| `0x2307` | `FUN_005808e0` | u8 u32 | 7 |  |  |
| `0x2308` | `FUN_00580930` | u8 u8 u32 | 8 |  |  |
| `0x2309` | `FUN_00580990` | u8 u32 | 7 |  |  |
| `0x230a` | `FUN_005593c0` | — (no reads) |  |  |  |
| `0x230b` | `FUN_005809e0` | u8 var((local_1c0 & 0xff) * 0x25) | var | GameMyShopItemListOutgoing (TP_MYSHOP_BUY_LIST) | 928 |
| `0x230c` | `FUN_00559490` | — (no reads) |  |  |  |
| `0x230d` | `FUN_00580a40` | u8 if:u32 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u8 if:u16 if:bytes[6] if:bytes[21]* | var |  |  |
| `0x230e` | `FUN_00580b50` | u8 u8 | 4 |  |  |
| `0x2401` | `FUN_00580dc0` | u32 u32 | 10 |  |  |
| `0x2402` | `FUN_00580e10` | u8 u32 | 7 |  |  |
| `0x2403` | `FUN_00580e60` | u32 u32 | 10 |  |  |
| `0x2404` | `FUN_00550400` | — (no reads) |  |  |  |
| `0x2405` | `FUN_00580eb0` | u8 | 3 |  |  |
| `0x2406` | `FUN_00580ee0` | u8 u32 | 7 |  |  |
| `0x2407` | `FUN_00581f00` | u32 u8 | 7 |  |  |
| `0x2408` | `FUN_00581f50` | u8 | 3 |  |  |
| `0x2409` | `FUN_00581f80` | u8 u8 | 4 |  |  |
| `0x240a` | `FUN_00581fd0` | u8 u8 u8 u8 | 6 |  |  |
| `0x240b` | `FUN_005820e0` | u8 u8 | 4 |  |  |
| `0x240c` | `FUN_00582130` | u8 u32 | 7 |  |  |
| `0x240d` | `FUN_00582040` | u8 u8 u8 u8 u16 bytes[6] bytes[21]* | var | GamePvPExcAddDestOutgoing_EP5 (TP_EXC_ADD_DEST) | 35 |
| `0x2501` | `FUN_00580f30` | u32 u32 | 10 | GamePvPGuildRequestOutgoing (TP_PVPG_REQ) | 10 ✓ |
| `0x2502` | `FUN_00580f80` | u8 u32 | 7 | GamePvPGuildAnswerOutgoing (TP_PVPG_ANS) | 7 ✓ |
| `0x2503` | `FUN_00580fd0` | u32 u32 u32 | 14 |  |  |
| `0x2504` | `FUN_005507f0` | — (no reads) |  |  |  |
| `0x2505` | `FUN_00581030` | u8 | 3 |  |  |
| `0x2506` | `FUN_00581060` | u8 u32 | 7 |  |  |
| `0x2601` | `FUN_00582180` | u32 u8 loop:bytes[29] | var |  |  |
| `0x2602` | `FUN_00582200` | u8 if:u32 if:bytes[21]* if:u32 if:u32 if:u8 if:var((local_a4 & 0xff) * 5) | var | GamePointBuyOutgoing (TP_POINT_BUY) | 157 |
| `0x2603` | `FUN_00581b40` | u8 if:bytes[21]* if:u32 if:bytes[21]* if:u32 if:u32 | var | GamePointGiftSendOutgoing (TP_POINT_GIFT_SEND) | 57 |
| `0x2604` | `FUN_00581be0` | bytes[21]* bytes[21]* u8 loop:u8 loop:u8 loop:u8 loop:u8 | var |  |  |
| `0x2605` | `FUN_005822c0` | u32 | 6 | GamePointOutgoing |  |
| `0x2606` | `FUN_00581a50` | u8 loop:u8 loop:u8 loop:u8 loop:u8 | var | GamePointStoredItemListOutgoing (TP_POINT_STORED_ITEMLIST) | 963 |
| `0x2607` | `FUN_00581ae0` | u8 u8 u8 | 5 | GamePointStoredItemGetOutgoing (TP_POINT_STORED_ITEMGET) | 5 ✓ |
| `0x2609` | `FUN_00581cc0` | u8 loop:u8 | var |  |  |
| `0x2701` | `FUN_00582bb0` | u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:u8 loop:bytes[21]* | var |  |  |
| `0x2702` | `FUN_00582ce0` | u8 u8 u8 u16 u8 u8 u8 u8 u8 u8 u8 bytes[21]* | var |  |  |
| `0x2703` | `FUN_00582de0` | u8 | 3 |  |  |
| `0x2704` | `FUN_00582b80` | u8 | 3 |  |  |
| `0x3101` | `FUN_0057fe90` | u8 | 3 |  |  |
| `0x3102` | `FUN_0057fec0` | u8 u32 u32 | 11 |  |  |
| `0x3103` | `FUN_0057ff20` | u8 bytes[9] u8 u32 u8 | 18 |  |  |
| `0x3104` | `FUN_0057ffa0` | u8 | 3 |  |  |
| `0x3105` | `FUN_005550b0` | — (no reads) |  |  |  |
| `0x3106` | `FUN_0057ffd0` | u32 | 6 |  |  |
| `0x3107` | `FUN_00580000` | u8 u8 u32 | 8 |  |  |
| `0x3108` | `FUN_00580060` | u8 | 3 |  |  |
| `0x3109` | `FUN_00555130` | — (no reads) |  |  |  |
| `0xa101` | `FUN_0057d3c0` | u8 u8 u8 bytes[64] bytes[128] | 197 |  |  |
| `0xa102` | `FUN_0057d450` | u8 u32 u8 bytes[16] | 24 |  |  |
| `0xa104` | `FUN_0057d570` | bytes[15] u8 loop:u16 | var |  |  |
| `0xa105` | `FUN_0057d540` | u8 | 3 |  |  |
| `0xa201` | `FUN_0057d4c0` | u8 loop:bytes[38] | var |  |  |
| `0xa202` | `FUN_0057d620` | u8 u32 | 7 |  |  |
| `0xa301` | `FUN_0057d670` | u8 u8 bytes[16] | 20 |  |  |
| `0xa302` | `FUN_00578e30` | bytes[16] | 18 |  |  |
| `0xa303` | `FUN_00578e80` | u16 var(local_1004 & 0xffff) | var |  |  |
| `0xb101` | `FUN_00581960` | u8 loop:u8 loop:u8 loop:u8 loop:u8 | var | GameBillingItemListOutgoing (TP_BILLING_ITEMLIST) | 963 |
| `0xb102` | `FUN_005819f0` | u8 u8 u8 | 5 | GameBillingItemGetOutgoing (TP_BILLING_ITEMGET) | 5 ✓ |
| `0xb103` | `FUN_00581d20` | u8 u32 | 7 | GameBillingServiceTypeOutgoing (TP_BILLING_SERVICETYPE) | 7 ✓ |
| `0xb104` | `FUN_00581d70` | u32 | 6 |  |  |
| `0xb105` | `FUN_00581da0` | u32 | 6 | GameBillingLogoutTimeOutgoing (TP_BILLING_LOGOFFTIME) | 6 ✓ |
| `0xb106` | `FUN_00581dd0` | u8 var((local_1304 & 0xff) * 0x4c) | var | GameBillingProductListOutgoing (TP_BILLING_PRODUCTLIST) | 9731 |
| `0xf101` | `FUN_0057e840` | u32 u8 var(local_90 & 0xff)* | var | GameAdminChatNormalOutgoing (TP_CHAT_NORMAL) | 135 |
| `0xf102` | `FUN_0057e8c0` | u8 bytes[21]* u8 var(local_a8 & 0xff)* | var | GameAdminChatSendToOutgoing (TP_CHAT_SENDTO) | 153 |
| `0xf103` | `FUN_0057e960` | bytes[21]* u8 var(local_a4[0] & 0xff)* | var | GameAdminChatTradeOutgoing |  |
| `0xf104` | `FUN_0057e9f0` | bytes[21]* u8 var(local_a4[0] & 0xff)* | var | GameAdminChatGuildOutgoing (TP_CHAT_SENDMAP) | 152 |
| `0xf105` | `FUN_0057ea80` | u32 u8 var(local_90 & 0xff)* | var | GameAdminChatPartyOutgoing (TP_CHAT_SENDPARTY) | 135 |
| `0xf106` | `FUN_0057eb00` | u16 var(local_7dc & 0xffff)* | var | GameAdminNotifyOutgoing (TP_ADMIN_NOTIFY) | 2004 |
| `0xf107` | `FUN_00578d10` | bytes[21]* | var |  |  |
| `0xf109` | `FUN_00578d60` | bytes[21]* | var |  |  |
| `0xf10a` | `FUN_005812c0` | u32 u8 var(local_40d)* | var |  |  |
| `0xf201` | `FUN_0057eb60` | u16 if:u32 if:u32 if:bytes[21]* if:u16 if:var(local_418 & 0xffff)* | var |  |  |
| `0xf204` | `FUN_0057ec30` | u16 | 4 |  |  |
| `0xf501` | `FUN_0057ec60` | u16 | 4 | GameAdminCmdErrorOutgoing (TP_ADMIN_CMDS_ERROR) | 4 ✓ |
| `0xf601` | `FUN_0057f5b0` | u8 | 3 |  |  |
| `0xf602` | `FUN_0057f5e0` | u8 | 3 |  |  |
| `0xf701` | `FUN_0057f530` | u8 u32 | 7 | GameAdminCmdBSetStatusOutgoing |  |
| `0xf709` | `FUN_00554010` | — (no reads) |  |  |  |
| `0xf70a` | `FUN_0057f580` | u16 | 4 |  |  |
| `0xf802` | `FUN_00553e20` | — (no reads) |  |  |  |
| `0xf803` | `FUN_00553e70` | — (no reads) |  |  |  |
| `0xf804` | `FUN_00554010` | — (no reads) |  |  |  |
| `0xf805` | `FUN_0057f440` | u16 | 4 |  |  |
| `0xf80b` | `FUN_0057f470` | u16 | 4 |  |  |
| `0xf80c` | `FUN_0057f4a0` | u16 | 4 |  |  |
| `0xf80f` | `FUN_0057f4d0` | u16 | 4 |  |  |
| `0xf810` | `FUN_0057f500` | u16 | 4 |  |  |
| `0xf901` | `FUN_0057eee0` | u8 u16 u32 u32 u16 u16 u16 u16 u16 u16 u16 u16 u16 u16 u8 u8 u8 u8 loop:u8 loop:u8 loop:u8 loop:u16 loop:bytes[6] loop:bytes[21]* | var |  |  |
| `0xf904` | `FUN_0057f080` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0xf905` | `FUN_0057f100` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0xf906` | `FUN_0057f200` | u8 var(local_409)* | var |  |  |
| `0xf907` | `FUN_0057f260` | u8 var(local_409)* | var |  |  |
| `0xf908` | `FUN_0057f2c0` | u8 var(local_409)* | var | GameAdminCmdDNoticeToOutgoing |  |
| `0xf909` | `FUN_0057f320` | u8 var(local_409)* | var |  |  |
| `0xf90a` | `FUN_0057f380` | u8 var(local_409)* | var |  |  |
| `0xf90b` | `FUN_0057f3e0` | u8 var(local_409)* | var | GameAdminCmdDNoticeOutgoing (TP_ADMIN_CMDS_NOTICE) | 131 |
| `0xf90c` | `FUN_0057f180` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0xfa07` | `FUN_0057ec90` | u8 var(local_409)* | var |  |  |
| `0xfa08` | `FUN_0057ecf0` | u16 u32 u32 u32 | 16 |  |  |
| `0xfa09` | `FUN_0057ed60` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0xfa0a` | `FUN_0057ede0` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0xfa0d` | `FUN_005537d0` | — (no reads) |  |  |  |
| `0xfa0e` | `FUN_005537e0` | — (no reads) |  |  |  |
| `0xfa0f` | `FUN_005537f0` | — (no reads) |  |  |  |
| `0xfa10` | `FUN_00553800` | — (no reads) |  |  |  |
| `0xfa11` | `FUN_0057ee60` | u32 u16 u32 u32 u32 | 20 |  |  |
| `0xfa12` | `FUN_0057ee60` | u32 u16 u32 u32 u32 | 20 |  |  |

## Client → server: 245 opcodes sent (callers of `FUN_00585640` and `FUN_00585520`)

| opcode | length | layout (offset:type, after the opcode) | sender(s) | EP6 name | EP4.5 PDB size |
|---|---|---|---|---|---|
| `0x0003` | 2 |  | `00585790` |  |  |
| `0x0102` | iVar1 + 0xbU & 0xffff |  | `005857c0` | GameCharCreateIncoming |  |
| `0x0103` | 6 | +2:u32 | `00585860` | GameCharDeleteIncoming |  |
| `0x0104` | 6 | +2:u32 | `00585890` | GameCharEnterWorldIncoming |  |
| `0x0107` | 2 |  | `005858c0` | GameCharLeaveWorldIncoming |  |
| `0x0109` | 3 | +2:u8 | `00585b40` |  |  |
| `0x010b` | (uint)param_1 * 5 + 3 |  | `005866e0` | GameCharQuickSlotIncoming |  |
| `0x010d` | 2 |  | `00585760` |  |  |
| `0x010e` | 27 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u32 +26:u8 | `00589460` | GameCharRenameIncoming (TPC_CHAR_RENAME) | 27 ✓ |
| `0x010f` | 6 | +2:u32 | `005894c0` | GameCharRestoreIncoming |  |
| `0x0201` | 2 |  | `005858f0` |  |  |
| `0x0204` | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | `00585950` | GameItemBagToBagIncoming (TPC_ITEM_BAGTOBAG) | 6 ✓ |
| `0x0205` | 6 | +2:u32 | `005859a0` | GameItemGetIncoming |  |
| `0x0206` | 5 | +2:u8 +3:u8 +4:u8 | `005859d0` | GameItemDropIncoming (TPC_ITEM_DROP) | 5 ✓ |
| `0x0208` | 14 | +2:u16 +4:u16 +6:u16 +8:u16 +10:u16 +12:u16 | `00585a20` | GameUserStatusUpIncoming |  |
| `0x0209` | 5 | +2:u16 +4:u8 | `00585aa0` | GameUserSkillGetIncoming (TPC_USER_GETSKILL) | 5 ✓ |
| `0x020a` | 3 | +2:u8 | `00585b70` | GameUserPortalUseIncoming (TPC_USER_USEPORTAL) | 4 ≠ |
| `0x020c` | 7 | +2:u32 +6:u8 | `00585ba0` | GameUserGateKeeperUseIncoming (TPC_USER_USEGATENPC) | 7 ✓ |
| `0x020d` | 7 | +2:u8 +3:u32 | `00585be0` |  |  |
| `0x0210` | 2 |  | `00585ae0` |  |  |
| `0x0212` | 2 |  | `00585b10` |  |  |
| `0x0214` | 2 |  | `00589030` |  |  |
| `0x0215` | 2 |  | `00589060` |  |  |
| `0x0216` | 2 |  | `005892b0` |  |  |
| `0x0218` | 2 |  | `00586db0` | GameStatusResultInfoIncoming |  |
| `0x0219` | 2 |  | `00586de0` | GameStatusResultKillIncoming |  |
| `0x021a` | 2 |  | `00586e10` | GameStatusResultDeathIncoming |  |
| `0x021c` | 2 |  | `005892e0` |  |  |
| `0x021d` | 6 | +2:u32 | `00589310` |  |  |
| `0x021e` | 3 | +2:u8 | `00589340` |  |  |
| `0x0220` | 17 | +2:u8 +3:u16 +5:f32 +9:u32 +13:u32 | `00589690` |  |  |
| `0x0224` | 3 | +2:u8 | `00589730` |  |  |
| `0x0226` | 8 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 | `005861e0` | GameUserBodyChangeIncoming (TPC_BODYCHG) | 8 ✓ |
| `0x0227` | 3 | +2:u8 | `005898e0` |  |  |
| `0x0301` | 6 | +2:u32 | `00588900` |  |  |
| `0x0302` | 6 | +2:u32 | `00588960` | GameGetInfoUserHealthIncoming |  |
| `0x0303` | 6 | +2:u32 | `00585920` |  |  |
| `0x0304` | 6 | +2:u32 | `00588990` |  |  |
| `0x0305` | 6 | +2:u32 | `005889c0` | GameGetInfoMobHealthIncoming |  |
| `0x0307` | 6 | +2:u32 | `00588930` | GameGetInfoUserItemsIncoming |  |
| `0x0501` | 17 | +2:u8 +3:u16 +5:f32 +9:u32 +13:u32 | `00586060` | GameCharMoveIncoming (TPC_PC_MOVE) | 17 ✓ |
| `0x0502` | 6 | +2:u32 | `00586110` | GameCharAttackUserIncoming |  |
| `0x0503` | 6 | +2:u32 | `00586140` | GameCharAttackMobIncoming |  |
| `0x0506` | 3 | +2:u8 | `00586170` | GameCharMotionIncoming |  |
| `0x050a` | 4 | +2:u8 +3:u8 | `005861a0` |  |  |
| `0x0511` | 7 | +2:u8 +3:u32 | `00586250` |  |  |
| `0x0517` | 7 | +2:u8 +3:u32 | `00586290` |  |  |
| `0x0553` | 3 | +2:u8 | `00586740` | GameCharRebirthIncoming |  |
| `0x0554` | 4 | +2:u8 +3:u8 | `00586960` | GameItemRepairIncoming (TPC_ITEM_REPAIR) | 4 ✓ |
| `0x0555` | 2 |  | `005869a0` |  |  |
| `0x0556` | 2 |  | `005888d0` |  |  |
| `0x0557` | 8 | +2:u8 +3:u8 +4:u32 | `005894f0` |  |  |
| `0x0702` | 8 | +2:u32 +6:u8 +7:u8 | `005862d0` |  |  |
| `0x0703` | 5 | +2:u8 +3:u8 +4:u8 | `00586320` | GameShopSellIncoming (TPC_SHOP_SELL) | 5 ✓ |
| `0x0801` | 8 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 | `00586370` |  |  |
| `0x0802` | 4, 8 | +2:u8 +3:u8 | `00586540`, `00586580` |  |  |
| `0x0803` | 10 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 +8:u8 +9:u8 | `005895d0` | GameCloakInfoIncoming (TPC_CLOAK_ADD) | 10 ✓ |
| `0x0804` | 4 | +2:u8 +3:u8 | `00589650` |  |  |
| `0x0805` | 9 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 +8:u8 | `005863e0` | GameLapisianAddIncoming_EP5 (TPC_LAPISIAN_ADD) | 9 ✓ |
| `0x0806` | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | `00586450` | GameItemComposeIncoming_EP5 (TPC_ITEM_COMPOSE) | 6 ✓ |
| `0x0807` | 8 | +2:u8 +3:u8 +4:u8 +5:u8 +6:u8 +7:u8 | `005864a0` | GameItemRemakeIncoming_EP5 (TPC_ITEM_REMAKE) | 8 ✓ |
| `0x0808` | 4 | +2:u8 +3:u8 | `00586500` | GameItemRemake2Incoming |  |
| `0x0902` | 8 | +2:u32 +6:u16 | `005865f0` | GameQuestStartIncoming |  |
| `0x0903` | 8 | +2:u32 +6:u16 | `00586630` | GameQuestEndIncoming |  |
| `0x0907` | 5 | +2:u16 +4:u8 | `00586670` | GameQuestEndSelectIncoming (TPC_QUEST_END_SELECT) | 5 ✓ |
| `0x0908` | 4 | +2:u16 | `005866b0` | GameQuestQuitIncoming |  |
| `0x0a01` | 6 | +2:u32 | `00586b30` | GameExchangeRequestIncoming (TP_EXC_REQUEST) | 6 ✓ |
| `0x0a02` | 3 | +2:u8 | `00586b60` |  |  |
| `0x0a05` | 3 | +2:u8 | `00586b90` | GameExchangeReadyIncoming (TPC_EXC_READY) | 3 ✓ |
| `0x0a06` | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | `00586bc0` |  |  |
| `0x0a07` | 3 | +2:u8 | `00586c10` |  |  |
| `0x0a08` | 6 | +2:u32 | `00586c40` |  |  |
| `0x0b02` | 6 | +2:u32 | `005869d0` |  |  |
| `0x0b03` | 7 | +2:u8 +3:u32 | `00586a00` |  |  |
| `0x0b05` | 2 |  | `00586a40` |  |  |
| `0x0b06` | 6 | +2:u32 | `00586a70` |  |  |
| `0x0b07` | 6 | +2:u32 | `00586aa0` |  |  |
| `0x0b09` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00586ad0` |  |  |
| `0x0b0d` | 2 |  | `00585d20` |  |  |
| `0x0b0e` | 8 | +2:u8 +3:u8 +4:u32 | `00585c20` |  |  |
| `0x0b0f` | 6 | +2:u32 | `00585cd0` | GamePartySetDivTypeIncoming (TPC_PARTY_SETDIVTYPE) | 6 ✓ |
| `0x0b10` | 3 | +2:u8 | `00585c80` | GamePartySetAutoJoinIncoming (TPC_PARTY_SETAUTOJOIN) | 3 ✓ |
| `0x0b11` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00585ff0` |  |  |
| `0x0b12` | 10 | +2:u32 +6:u32 | `00585da0` |  |  |
| `0x0b13` | 6 | +2:u32 | `00585e40` |  |  |
| `0x0b14` | 6 | +2:u32 | `00585ee0` |  |  |
| `0x0b15` | 7 | +2:u8 +3:u32 | `00585fa0` |  |  |
| `0x0b16` | 2 |  | `00585d60` |  |  |
| `0x0b17` | 6 | +2:u32 | `00585df0` |  |  |
| `0x0b18` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00585f30` |  |  |
| `0x0b1a` | 6 | +2:u32 | `00585e90` |  |  |
| `0x0d01` | 27 | +2:bytes[25] | `005889f0` |  |  |
| `0x0d03` | 2 |  | `00588a30` |  |  |
| `0x0d04` | 3 | +2:u8 | `00588a60` |  |  |
| `0x0d05` | 6 | +2:u32 | `00588a90` |  |  |
| `0x0d06` | 3 | +2:u8 | `00588ac0` |  |  |
| `0x0d07` | 6 | +2:u32 | `00588af0` |  |  |
| `0x0d08` | 7 | +2:u8 +3:u32 | `00588b20` |  |  |
| `0x0d09` | 2 |  | `00588b60` |  |  |
| `0x0d0a` | 6 | +2:u32 | `00588b90` |  |  |
| `0x0d0c` | 7 | +2:u8 +3:u32 | `00588bc0` |  |  |
| `0x0d20` | 67 | +2:bytes[65] | `00589b10` |  |  |
| `0x0d21` | 92 | +2:bytes[24] +27:bytes[65] | `00589a80` |  |  |
| `0x0d22` | 3 | +2:u8 | `00589ae0` |  |  |
| `0x0d23` | 2 |  | `00589970` |  |  |
| `0x0d25` | 5 | +2:u16 +4:u8 | `005899a0` |  |  |
| `0x0d34` | 6 | +2:u32 | `00589940` |  |  |
| `0x0d35` | 6 | +2:u32 | `00589910` |  |  |
| `0x0e04` | 6 | +2:u32 | `00586e40` |  |  |
| `0x0f01` | 2 |  | `00586d50` |  |  |
| `0x0f02` | 2 |  | `00586d80` |  |  |
| `0x1111` | param_3 + 3 |  | `005870d0` | GameChatZoneIncoming |  |
| `0x1112` | local_2006 + 3 |  | `00589c20` | GameChatUnionIncoming |  |
| `0x1201` | param_1 + 4 & 0xffff |  | `00587130` |  |  |
| `0x1c01` | 39 | +2:bytes[4] +6:bytes[4] +10:u32 +14:u32 +18:u32 +22:u32 +26:u32 +30:u32 +34:u32 +38:u8 | `00459f60` |  |  |
| `0x1c02` | 4 | +2:bytes[2] | `00459f60` |  |  |
| `0x1c03` | 3 | +2:u8 | `00459f60` |  |  |
| `0x1c04` | 2 |  | `00458430`, `00459b60` |  |  |
| `0x1c05` | 2 |  | `00458430`, `00459b60` |  |  |
| `0x1c06` | 2 |  | `00458430` |  |  |
| `0x1c07` | 2 |  | `00458430` |  |  |
| `0x1c08` | 14 | +2:bytes[4] +6:u32 +10:u32 | `0045a950` |  |  |
| `0x1c09` | 6 |  | `0045a950` |  |  |
| `0x1c0a` | 6 |  | `0045a950` |  |  |
| `0x1c0b` | 10 | +2:u32 +6:u32 | `0045a950` |  |  |
| `0x1c0c` | 6 | +2:u32 | `00459f60`, `0055d490` |  |  |
| `0x1c0d` | 6 | +2:u32 | `00459f60`, `0055d490` |  |  |
| `0x1c14` | 2 |  | `00458430`, `00459f60` |  |  |
| `0x1c15` | 6 | +2:u32 | `00459f60` |  |  |
| `0x1c16` | 6 | +2:u32 | `00459f60` |  |  |
| `0x1c17` | 2 |  | `0045a950` |  |  |
| `0x2202` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00588db0` | GameFriendRequestIncoming (TPC_COMM_FRIEND_REQ) | 23 ✓ |
| `0x2203` | 3 | +2:u8 | `00588e10` |  |  |
| `0x2205` | 6 | +2:u32 | `00588e40` | GameFriendRemoveIncoming |  |
| `0x2206` | local_2002 + 7 |  | `00588e70` | GameFriendMemoIncoming |  |
| `0x2209` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00588ef0` | GameBlockAddIncoming (TPC_COMM_BLOCK_ADD) | 23 ✓ |
| `0x220a` | 6 | +2:u32 | `00588f50` | GameBlockRemoveIncoming |  |
| `0x220b` | local_2002 + 7 |  | `00588f80` | GameBlockMemoIncoming |  |
| `0x220c` | 2 |  | `00589000` |  |  |
| `0x2301` | 2 |  | `00589090` |  |  |
| `0x2302` | 2 |  | `005890c0` |  |  |
| `0x2303` | local_2006 + 3 |  | `005890f0` |  |  |
| `0x2304` | 2 |  | `00589160` |  |  |
| `0x2305` | 9 | +2:u8 +3:u8 +4:u8 +5:u32 | `00589190` |  |  |
| `0x2306` | 3 | +2:u8 | `005891e0` |  |  |
| `0x2309` | 6 | +2:u32 | `00589210` |  |  |
| `0x230a` | 2 |  | `00589240` |  |  |
| `0x230d` | 4 | +2:u8 +3:u8 | `00589270` |  |  |
| `0x2401` | 6 | +2:u32 | `00589370` |  |  |
| `0x2402` | 3 | +2:u8 | `005893a0` |  |  |
| `0x2406` | 2 |  | `005893d0` |  |  |
| `0x2409` | 3 | +2:u8 | `00586c70` |  |  |
| `0x240a` | 6 | +2:u8 +3:u8 +4:u8 +5:u8 | `00586ca0` |  |  |
| `0x240b` | 3 | +2:u8 | `00586cf0` |  |  |
| `0x240c` | 6 | +2:u32 | `00586d20` |  |  |
| `0x2501` | 6 | +2:u32 | `00589400` | GamePvPGuildRequestIncoming |  |
| `0x2502` | 3 | +2:u8 | `00589430` | GamePvPGuildAnswerIncoming (TPC_PVPG_ANS) | 3 ✓ |
| `0x2602` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00589760` | GamePointBuyIncoming (TPC_POINT_BUY) | 23 ✓ |
| `0x2603` | 44 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u32 +27:u32 +31:u32 +35:u32 +39:u32 +43:u8 | `005897f0` | GamePointGiftSendIncoming (TPC_POINT_GIFT_SEND) | 44 ✓ |
| `0x2605` | 2 |  | `005897c0` | GamePointRequest |  |
| `0x2607` | 3 | +2:u8 | `005895a0` | GamePointStoredItemGetIncoming (TPC_POINT_STORED_ITEMGET) | 3 ✓ |
| `0x2704` | 2 |  | `00589b50` |  |  |
| `0x2705` | 2 |  | `00589b80` |  |  |
| `0x3101` | 3 | +2:u8 | `00588c00` |  |  |
| `0x3102` | 2 |  | `00588c30` |  |  |
| `0x3103` | 2 |  | `00588c60` |  |  |
| `0x3104` | 2 |  | `00588c90` |  |  |
| `0x3105` | 2 |  | `00588cc0` |  |  |
| `0x3106` | 2 |  | `00588cf0` |  |  |
| `0x3107` | 3 | +2:u8 | `00588d20` |  |  |
| `0x3108` | 2 |  | `00588d50` |  |  |
| `0x3109` | 2 |  | `00588d80` |  |  |
| `0xa101` | 131 | +2:u8 +3:bytes[128] | `00586770` |  |  |
| `0xa105` | 23 | +2:u8 +3:u32 +7:u32 +11:u32 +15:u32 +19:u32 | `00586870` |  |  |
| `0xa202` | 7 | +2:u8 +3:u32 | `005868d0` |  |  |
| `0xa301` | 22 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 | `00586910` |  |  |
| `0xb102` | 3 | +2:u8 | `00589540` | GameBillingItemBankToBagIncoming |  |
| `0xb106` | 2 |  | `00589570` |  |  |
| `0xf10a` | local_2006 + 3 |  | `00589bb0` |  |  |
| `0xf201` | 2 |  | `00587190` |  |  |
| `0xf202` | param_3 + 10 |  | `005871c0` |  |  |
| `0xf203` | param_3 + 0x1e |  | `00587230` |  |  |
| `0xf204` | 2 |  | `005872c0` |  |  |
| `0xf601` | 24 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u8 | `005886f0` |  |  |
| `0xf602` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00588750` |  |  |
| `0xf701` | 28 | +2:u8 +3:u32 +7:u32 +11:u32 +15:u32 +19:u32 +23:u32 +27:u8 | `005882c0` |  |  |
| `0xf702` | 5 | +2:u8 +3:u8 +4:u8 | `00588330` |  |  |
| `0xf703` | 25 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u8 +24:u8 | `00588380` |  |  |
| `0xf704` | 5 | +2:u16 +4:u8 | `005883f0` |  |  |
| `0xf705` | 5 | +2:u16 +4:u8 | `00588430` |  |  |
| `0xf706` | 2 |  | `00588540` |  |  |
| `0xf707` | 2 |  | `00588570` |  |  |
| `0xf708` | 2 |  | `005885a0` |  |  |
| `0xf709` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `005885d0` |  |  |
| `0xf70a` | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | `00588630` |  |  |
| `0xf70b` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00588690` |  |  |
| `0xf70c` | 6 | +2:u8 +3:u16 +5:u8 | `005884a0` |  |  |
| `0xf70d` | 6 | +2:u8 +3:u16 +5:u8 | `005884f0` |  |  |
| `0xf70e` | 6 | +2:u32 | `00588470` |  |  |
| `0xf801` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587d20` | GameAdminCmdCCharCureIncoming |  |
| `0xf802` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587d80` |  |  |
| `0xf803` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587de0` |  |  |
| `0xf804` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587e40` | GameAdminCmdCCharKickIncoming |  |
| `0xf805` | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | `00587ea0` |  |  |
| `0xf806` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587f00` |  |  |
| `0xf809` | 4 | +2:u16 | `00587f60` |  |  |
| `0xf80a` | 4 | +2:u16 | `00587f90` |  |  |
| `0xf80b` | 25 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u16 | `00587fc0` |  |  |
| `0xf80c` | 25 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u16 | `00588080` |  |  |
| `0xf80d` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `005880e0` |  |  |
| `0xf80e` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00588140` |  |  |
| `0xf80f` | 44 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u32 +27:u32 +31:u32 +35:u32 +39:u32 +43:u8 | `005881a0` |  |  |
| `0xf810` | 44 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:u32 +27:u32 +31:u32 +35:u32 +39:u32 +43:u8 | `00588230` |  |  |
| `0xf901` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `005877d0` |  |  |
| `0xf904` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587830` | GameAdminCmdDMoveCharIncoming |  |
| `0xf905` | 33 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:f32 +27:u32 +31:u16 | `00587890` |  |  |
| `0xf906` | local_2006 + 3 |  | `005879f0` |  |  |
| `0xf907` | iVar3 |  | `00587a60` |  |  |
| `0xf908` | iVar3 |  | `00587ae0` | GameAdminCmdDNoticeToIncoming |  |
| `0xf909` | iVar3 |  | `00587b90` |  |  |
| `0xf90a` | iVar3 |  | `00587c10` |  |  |
| `0xf90b` | iVar3 |  | `00587ca0` | GameAdminCmdDNoticeIncoming |  |
| `0xf90c` | 33 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 +23:f32 +27:u32 +31:u16 | `00587940` |  |  |
| `0xfa01` | 2 |  | `005872f0` |  |  |
| `0xfa02` | 2 |  | `00587320` |  |  |
| `0xfa05` | 2 |  | `00587350` |  |  |
| `0xfa06` | 2 |  | `00587380` |  |  |
| `0xfa07` | local_1ff1 + 0x18 |  | `005873b0` |  |  |
| `0xfa08` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587450` | GameAdminCmdEFindCharIncoming |  |
| `0xfa09` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `005874b0` |  |  |
| `0xfa0a` | 12 | +2:u32 +6:u32 +10:u16 | `00587510` |  |  |
| `0xfa0b` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `005875c0` |  |  |
| `0xfa0c` | 2 |  | `00587620` |  |  |
| `0xfa0d` | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | `00587650` | GameAdminCmdEChatBanEnableIncoming |  |
| `0xfa0e` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `005876b0` | GameAdminCmdEChatBanDisableIncoming |  |
| `0xfa0f` | 25 | +2:u16 +4:u32 +8:u32 +12:u32 +16:u32 +20:u32 +24:u8 | `00587710` | GameAdminCmdEMoveBanEnableIncoming |  |
| `0xfa10` | 23 | +2:u32 +6:u32 +10:u32 +14:u32 +18:u32 +22:u8 | `00587770` | GameAdminCmdEMoveBanDisableIncoming |  |
| `0xfa11` | 4 | +2:u16 | `00587560` |  |  |
| `0xfa13` | 49 | +2:bytes[24] +27:u32 +31:u32 +35:u32 +39:u32 +43:u32 +47:u8 +48:u8 | `005887b0` |  |  |
| `0xfa14` | 28 | +2:bytes[24] +27:u8 | `00588830` |  |  |
| `0xfa15` | 31 | +2:bytes[24] +27:u32 | `00588880` |  |  |
| `0xfa16` | 2 |  | `00588020` |  |  |
| `0xfa17` | 2 |  | `00588050` |  |  |
| `0xfa18` | 3 | +2:u8 | `00587590` |  |  |
| ? | 23, 27, 3, param_3 + 0x18, param_3 + 3, uVar2, unaff_EDI) |  | `004705d0`, `004cd7f0`, `005867c0`, `00586e70` … |  |  |
