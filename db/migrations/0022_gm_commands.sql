-- Operator (GM) commands part 1 (docs/original/gm-commands.md, deviations 16 and 17): the quests a GM closed, server-wide and kept
-- across restarts, and event tags a GM puts on characters. The account bans of deviation 14 use accounts.status and blocked_until.
CREATE TABLE closed_quests (
    server_id smallint NOT NULL,
    quest_id integer NOT NULL,
    closed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (server_id, quest_id)
);

CREATE TABLE character_events (
    character_id integer NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    name varchar(20) NOT NULL CHECK (length(name) BETWEEN 1 AND 20),
    added_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, name)
);
CREATE UNIQUE INDEX character_events_name ON character_events (character_id, lower(name));
