-- Skills a character has learnt and its quick slots (docs/original/skills.md section 2).
-- number is the skill's index in the character's list: given in learning order, kept by later levels, referred to by the client.
-- delay is the cooldown left in seconds when the character was last saved; it stands still while the character is offline, as in the
-- original.
CREATE TABLE character_skills (
    character_id integer  NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    number       smallint NOT NULL CHECK (number BETWEEN 0 AND 255),
    skill_id     smallint NOT NULL CHECK (skill_id > 0),
    level        smallint NOT NULL CHECK (level BETWEEN 1 AND 3),
    delay        integer  NOT NULL DEFAULT 0 CHECK (delay BETWEEN 0 AND 65535),
    PRIMARY KEY (character_id, number),
    UNIQUE (character_id, skill_id)
);

-- The quick slots as the client last sent them (bar, slot, bag, number), in its order; the server does not interpret them.
CREATE TABLE character_quick_slots (
    character_id integer  NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    position     smallint NOT NULL CHECK (position BETWEEN 0 AND 127),
    bar          smallint NOT NULL CHECK (bar BETWEEN 0 AND 255),
    slot         smallint NOT NULL CHECK (slot BETWEEN 0 AND 255),
    bag          smallint NOT NULL CHECK (bag BETWEEN 0 AND 255),
    number       integer  NOT NULL CHECK (number BETWEEN 0 AND 65535),
    PRIMARY KEY (character_id, position)
);
