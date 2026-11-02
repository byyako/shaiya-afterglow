-- The kill and death reward steps taken (docs/original/pvp.md section 8): the original's Chars.KillLevel and DeadLevel, 0 to 16.
ALTER TABLE characters
    ADD COLUMN kill_level smallint NOT NULL DEFAULT 0 CHECK (kill_level BETWEEN 0 AND 16),
    ADD COLUMN dead_level smallint NOT NULL DEFAULT 0 CHECK (dead_level BETWEEN 0 AND 16);
