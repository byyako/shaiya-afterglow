-- The guild ranking battle (docs/original/guild-ranking.md section 10, deviation 4): when a battle closed and its ranking update has not
-- run yet, so that a restart in the minutes between does not lose the update. One row; due is null when nothing is waiting.
CREATE TABLE guild_ranking (
    id  smallint PRIMARY KEY CHECK (id = 1),
    due timestamptz
);
INSERT INTO guild_ranking (id, due) VALUES (1, NULL);
