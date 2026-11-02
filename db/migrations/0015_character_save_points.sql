-- The four saved positions of a character (docs/original/item-windows.md section 1), after the original's save point table: slot 0 to 3
-- (index 1 to 4 in the packets), written by 0x0220 and used by the Teleportation Stone.
CREATE TABLE character_save_points (
    character_id integer  NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    slot         smallint NOT NULL CHECK (slot BETWEEN 0 AND 3),
    map          smallint NOT NULL CHECK (map >= 0),
    x            real     NOT NULL,
    y            real     NOT NULL,
    z            real     NOT NULL,
    PRIMARY KEY (character_id, slot)
);
