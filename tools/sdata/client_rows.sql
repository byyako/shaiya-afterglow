-- What a client data patch changes, beyond what the database already holds (docs/plans/done/client-patches.md section 2). Loaded by
-- tools/sdata/client_patch.py before every run (after tools/sdata/load_client.py has imported the originals into `client`).
--
-- A record of the generated Item.SData / Skill.SData is the client's original (client.items, client.skills) unless it has a row here;
-- then it is the server's fields (defs.items, defs.skills) with the client-only fields of its row. So every change of
-- tools/defs/additions.sql needs a row here (client.item_rows, client.skill_rows) or a reason why not (client.server_only), and
-- `client_patch.py check` fails when one is missing. One block per server change, each naming the additions.sql block it goes with.
-- Names and descriptions are printable ASCII (check 7); a new row repeats the name additions.sql gave.

BEGIN;

TRUNCATE client.item_rows, client.skill_rows, client.server_only;

-- The kill-boost stones (additions.sql "The kill-boost stones"; docs/plans/done/client-patches.md decisions 1 to 4): items 100/212 to 214
-- (the empty "???177") look like the Mini Sun EXP Stone 100/108 (model, icon, type2, type3); skill 200 levels 1 to 3 (the empty
-- "Blank") use the icon, animation, toggle byte and sound of its buff 250/1 (icon 225). Each item and its buff share the text.
CREATE TEMP TABLE kill_boost (level smallint, item_name text, buff_name text, description text) ON COMMIT DROP;
INSERT INTO kill_boost VALUES
  (1, 'Kill Boost Stone', 'Kill Boost',
   'Player kills count double for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out.'),
  (2, 'Kill Boost Stone Lv2', 'Kill Boost Lv2',
   'Player kills count 5 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out.'),
  (3, 'Kill Boost Stone Lv3', 'Kill Boost Lv3',
   'Player kills count 10 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out.');

INSERT INTO client.item_rows (type, typeid, name, description, model, icon, type2, type3, since_patch, note)
SELECT 100, 211 + k.level, k.item_name, k.description, i.model, i.icon, i.type2, i.type3, 109,
       'kill-boost stone level ' || k.level || ', looks like the Mini Sun EXP Stone 100/108'
FROM client.items i, kill_boost k
WHERE i.grp = 100 AND i.idx = 108;

INSERT INTO client.skill_rows (skill_id, level, name, description, icon, animation, toggle_type, sound, since_patch, note)
SELECT 200, k.level, k.buff_name, k.description, s.icon, s.animation, s.toggle_type, s.sound, 109,
       'kill-boost buff level ' || k.level || ', icon and look of the Mini Sun EXP Stone buff 250/1'
FROM client.skills s, kill_boost k
WHERE s.skill_id = 250 AND s.slot = 1;

-- The monsters' skills (level 100) live in the client's NpcSkill.SData, which is not generated yet: check 4 refuses a change to one
-- until it is, so nothing is listed here for them.

COMMIT;
