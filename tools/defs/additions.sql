-- The server's own definition rows, not in the original PS_GameDefs: applied by tools/defs/additions.sh after every
-- load_into_postgres.py (the load replaces the schema) and after verify.py (which compares the load with the original).
-- Each change names the client patch that carries the same values (both copies must match: docs/client/sdata-generation.md).
-- Updates only, so running it twice changes nothing.

BEGIN;

-- The kill-boost stones (docs/original/operator-console.md decision 3, research entry 7; docs/plans/done/client-patches.md decision 1): items
-- 100/212, 100/213 and 100/214 (the empty "???177" of both copies) give skill 200 levels 1, 2 and 3 (empty "??"; 200/100 stays the
-- monsters' Weakening Curse). Each skill is a copy of the Mini Sun EXP Stone's 250/1 (a kind-4 buff, detail 82) without its ability,
-- lasting one hour and kept on death (fixrange 1: keeptime in hours, counting down while offline). They do nothing by themselves: the
-- multipliers live in GAME_KILL_BOOST_SKILLS (default 200/1:2,200/2:5,200/3:10), so the kills counted are 2, 5 and 10 times while
-- the level is applied. Each item is a copy of the Mini Sun EXP Stone 100/108 and puts its level in `attacktime` (the skill's level)
-- and 200 in `range` (the skill id). Client side: Item.SData 100/212 to 214 and Skill.SData 200 levels 1 to 3 with the same values and
-- the names and texts of tools/sdata/client_rows.sql, in the first data patch (109).

UPDATE defs.skills AS s
SET (country, attackfighter, defensefighter, patrolrogue, shootrogue, attackmage, defensemage, prevskillid, reqlevel, grow, skillpoint,
     typeshow, typeattack, typeeffect, typedetail, needweapon1, needweapon2, needweapon3, needweapon4, needweapon5, needweapon6,
     needweapon7, needweapon8, needweapon9, needweapon10, needweapon11, needweapon12, needweapon13, needweapon14, needweapon15, shield,
     sp, mp, readytime, resettime, attackrange, statetype, attrtype, disable, successtype, successvalue, targettype, applyrange,
     multiattack, keeptime, weapon1, weapon2, weaponvalue, bag, arrow, damagetype, damagehp, damagesp, damagemp, timedamagetype,
     timedamagehp, timedamagesp, timedamagemp, adddamagehp, adddamagesp, adddamagemp, abilitytype1, abilityvalue1, abilitytype2,
     abilityvalue2, abilitytype3, abilityvalue3, healhp, healsp, healmp, timehealhp, timehealsp, timehealmp, defencetype, defencevalue,
     limithp, fixrange, changetype, changelevel)
  = (SELECT country, attackfighter, defensefighter, patrolrogue, shootrogue, attackmage, defensemage, prevskillid, reqlevel, grow,
            skillpoint, typeshow, typeattack, typeeffect, typedetail, needweapon1, needweapon2, needweapon3, needweapon4, needweapon5,
            needweapon6, needweapon7, needweapon8, needweapon9, needweapon10, needweapon11, needweapon12, needweapon13, needweapon14,
            needweapon15, shield, sp, mp, readytime, resettime, attackrange, statetype, attrtype, disable, successtype, successvalue,
            targettype, applyrange, multiattack, keeptime, weapon1, weapon2, weaponvalue, bag, arrow, damagetype, damagehp, damagesp,
            damagemp, timedamagetype, timedamagehp, timedamagesp, timedamagemp, adddamagehp, adddamagesp, adddamagemp, abilitytype1,
            abilityvalue1, abilitytype2, abilityvalue2, abilitytype3, abilityvalue3, healhp, healsp, healmp, timehealhp, timehealsp,
            timehealmp, defencetype, defencevalue, limithp, fixrange, changetype, changelevel
     FROM defs.skills WHERE skillid = 250 AND skilllevel = 1)
WHERE s.skillid = 200 AND s.skilllevel = 1;

UPDATE defs.skills
SET skillname = 'Kill Boost', keeptime = 1, fixrange = 1,
    abilitytype1 = 0, abilityvalue1 = 0, abilitytype2 = 0, abilityvalue2 = 0, abilitytype3 = 0, abilityvalue3 = 0
WHERE skillid = 200 AND skilllevel = 1;

-- Levels 2 and 3 are copies of level 1 as just changed (every column), with their own names.
UPDATE defs.skills AS s
SET (country, attackfighter, defensefighter, patrolrogue, shootrogue, attackmage, defensemage, prevskillid, reqlevel, grow, skillpoint,
     typeshow, typeattack, typeeffect, typedetail, needweapon1, needweapon2, needweapon3, needweapon4, needweapon5, needweapon6,
     needweapon7, needweapon8, needweapon9, needweapon10, needweapon11, needweapon12, needweapon13, needweapon14, needweapon15, shield,
     sp, mp, readytime, resettime, attackrange, statetype, attrtype, disable, successtype, successvalue, targettype, applyrange,
     multiattack, keeptime, weapon1, weapon2, weaponvalue, bag, arrow, damagetype, damagehp, damagesp, damagemp, timedamagetype,
     timedamagehp, timedamagesp, timedamagemp, adddamagehp, adddamagesp, adddamagemp, abilitytype1, abilityvalue1, abilitytype2,
     abilityvalue2, abilitytype3, abilityvalue3, healhp, healsp, healmp, timehealhp, timehealsp, timehealmp, defencetype, defencevalue,
     limithp, fixrange, changetype, changelevel)
  = (SELECT country, attackfighter, defensefighter, patrolrogue, shootrogue, attackmage, defensemage, prevskillid, reqlevel, grow, skillpoint,
            typeshow, typeattack, typeeffect, typedetail, needweapon1, needweapon2, needweapon3, needweapon4, needweapon5, needweapon6,
            needweapon7, needweapon8, needweapon9, needweapon10, needweapon11, needweapon12, needweapon13, needweapon14, needweapon15, shield,
            sp, mp, readytime, resettime, attackrange, statetype, attrtype, disable, successtype, successvalue, targettype, applyrange,
            multiattack, keeptime, weapon1, weapon2, weaponvalue, bag, arrow, damagetype, damagehp, damagesp, damagemp, timedamagetype,
            timedamagehp, timedamagesp, timedamagemp, adddamagehp, adddamagesp, adddamagemp, abilitytype1, abilityvalue1, abilitytype2,
            abilityvalue2, abilitytype3, abilityvalue3, healhp, healsp, healmp, timehealhp, timehealsp, timehealmp, defencetype, defencevalue,
            limithp, fixrange, changetype, changelevel
     FROM defs.skills WHERE skillid = 200 AND skilllevel = 1)
WHERE s.skillid = 200 AND s.skilllevel IN (2, 3);

UPDATE defs.skills SET skillname = 'Kill Boost Lv2' WHERE skillid = 200 AND skilllevel = 2;
UPDATE defs.skills SET skillname = 'Kill Boost Lv3' WHERE skillid = 200 AND skilllevel = 3;

UPDATE defs.items AS i
SET (reqlevel, country, grow, special, slot, quality, buy, sell, grade, drop, server, count)
  = (SELECT reqlevel, country, grow, special, slot, quality, buy, sell, grade, drop, server, count FROM defs.items WHERE itemid = 100108)
WHERE i.itemid IN (100212, 100213, 100214);

UPDATE defs.items SET itemname = 'Kill Boost Stone', range = 200, attacktime = 1 WHERE itemid = 100212;
UPDATE defs.items SET itemname = 'Kill Boost Stone Lv2', range = 200, attacktime = 2 WHERE itemid = 100213;
UPDATE defs.items SET itemname = 'Kill Boost Stone Lv3', range = 200, attacktime = 3 WHERE itemid = 100214;

-- Checks: each change above touched exactly its row.
DO $$
BEGIN
  IF (SELECT count(*) FROM defs.skills WHERE skillid = 200 AND skilllevel IN (1, 2, 3) AND typeeffect = 4 AND typedetail = 82
        AND fixrange = 1 AND keeptime = 1 AND abilitytype1 = 0 AND abilitytype2 = 0 AND abilitytype3 = 0) <> 3
     OR (SELECT count(*) FROM defs.items WHERE itemid IN (100212, 100213, 100214) AND range = 200 AND attacktime = itemid - 100211) <> 3 THEN
    RAISE EXCEPTION 'the kill-boost rows are not as expected (is defs loaded?)';
  END IF;
END $$;

COMMIT;
