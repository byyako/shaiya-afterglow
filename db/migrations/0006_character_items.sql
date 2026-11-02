-- Items a character carries, after the original CharItems table (docs/original/enter-world.md section 5).
-- Bag 0 is the equipment (slot 5 the weapon, slot 7 the cloak), bags 1 to 5 the inventory.
-- item_id is the defs.items id (type * 1000 + type_id); it is not a foreign key so that the definitions can be reloaded.
CREATE SEQUENCE item_uid_seq;

CREATE TABLE character_items (
    character_id integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    bag          smallint    NOT NULL CHECK (bag BETWEEN 0 AND 5),
    slot         smallint    NOT NULL CHECK (slot BETWEEN 0 AND 23),
    item_id      integer     NOT NULL,
    type         smallint    NOT NULL CHECK (type BETWEEN 0 AND 255),
    type_id      smallint    NOT NULL CHECK (type_id BETWEEN 0 AND 255),
    quality      integer     NOT NULL DEFAULT 0 CHECK (quality BETWEEN 0 AND 65535),
    gems         bytea       NOT NULL DEFAULT '\x000000000000' CHECK (length(gems) = 6),
    count        smallint    NOT NULL DEFAULT 1 CHECK (count BETWEEN 1 AND 255),
    craft_name   varchar(20) NOT NULL DEFAULT '',
    -- the original's Maketype (meaning not yet known; empty when unknown)
    make_type    varchar(1)  NOT NULL DEFAULT '',
    made_at      timestamptz NOT NULL DEFAULT now(),
    -- the original's ItemUID: unique per item, never reused
    uid          bigint      NOT NULL DEFAULT nextval('item_uid_seq') UNIQUE,
    PRIMARY KEY (character_id, bag, slot)
);
