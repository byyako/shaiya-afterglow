-- The name change (docs/original/small-items.md section 9.4), after the original's Chars.RenameCnt (a tinyint counter of renames
-- allowed: the Name Change Stone adds one, a rename takes one), Chars.OldCharName and CharRenameLog. A rename allowed before becomes
-- one rename left.
ALTER TABLE characters ADD COLUMN rename_count smallint NOT NULL DEFAULT 0 CHECK (rename_count BETWEEN 0 AND 255);
UPDATE characters SET rename_count = 1 WHERE rename_allowed;
ALTER TABLE characters DROP COLUMN rename_allowed;
-- The name before the last rename (OldCharName).
ALTER TABLE characters ADD COLUMN old_name varchar(20) NULL;

-- Every rename (CharRenameLog), with both names.
CREATE TABLE character_renames (
    id           bigserial   PRIMARY KEY,
    server_id    smallint    NOT NULL,
    character_id integer     NOT NULL REFERENCES characters (id),
    old_name     varchar(20) NOT NULL,
    new_name     varchar(20) NOT NULL,
    renamed_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX character_renames_character_idx ON character_renames (character_id);
