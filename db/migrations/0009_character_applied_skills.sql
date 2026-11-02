-- Skills applied to a character when it was last saved: buffs and debuffs (docs/original/skills-buffs.md section 13).
-- seconds is the time left then. saved_at tells how long the character has been away: skills kept through death (byDontRemByDeath)
-- count down offline, the others stand still, as in the original.
CREATE TABLE character_applied_skills (
    character_id integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    position     smallint    NOT NULL CHECK (position BETWEEN 0 AND 255),
    skill_id     smallint    NOT NULL CHECK (skill_id > 0),
    level        smallint    NOT NULL CHECK (level BETWEEN 1 AND 100),
    seconds      integer     NOT NULL CHECK (seconds >= 0),
    saved_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, position)
);
