-- An ultimate-mode character that died without a rune (docs/original/item-flags.md deviation 2): the original deletes it
-- (CUser::KillSoul's 0x0406, usp_Delete_Char_E: Del = 1); here deleted_at is set as for a deletion, and fallen_at marks why, so that
-- it can be restored later (clear both). Its name stays reserved while it is fallen.
ALTER TABLE characters ADD COLUMN fallen_at timestamptz NULL;
CREATE INDEX characters_fallen_name_idx ON characters (lower(name)) WHERE fallen_at IS NOT NULL;
