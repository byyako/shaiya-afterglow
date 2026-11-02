-- The monsters' object id counter (docs/original/quest-drops.md section 4.3): the original keeps it in MobID.ini and writes it back
-- every hour and at shutdown. Sequence items depend on a monster's id, so the counter must go on across restarts. 1 is what a missing
-- MobID.ini is written as. The game server saves it every minute and at shutdown.
ALTER TABLE game_servers ADD COLUMN mob_id bigint NOT NULL DEFAULT 1 CHECK (mob_id BETWEEN 0 AND 4294967295);
