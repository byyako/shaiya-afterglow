-- A character's quests (docs/original/quest.md section 12): one row per quest, in progress or finished, as the original's CharQuests.
-- In progress: the three counters and, for a quest with a time limit, the seconds left when the character last left the world (the
-- clock stands still while it is away, as in the original). Finished: success 1 done, 0 failed; a finished quest is never taken
-- again. Repeatable quests have no finished row.
CREATE TABLE character_quests (
    character_id integer  NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    quest_id     smallint NOT NULL CHECK (quest_id >= 1),
    finished     boolean  NOT NULL DEFAULT false,
    success      boolean  NOT NULL DEFAULT false,
    counts       smallint[] NOT NULL DEFAULT '{0,0,0}' CHECK (cardinality(counts) = 3),
    seconds_left integer  CHECK (seconds_left >= 0),
    PRIMARY KEY (character_id, quest_id)
);
