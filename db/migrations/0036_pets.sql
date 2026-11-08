-- Pets (docs/plans/ext-4-pets.md sections 6.3 and 8). The monster records an extension adds to the client's Monster.SData, the
-- `monster` kind of the ids an extension owns, and the pet each character has out. Guarded as 0031: the tests of other binaries run all
-- migrations again inside throwaway schemas of the same database.
--
-- One row per added record. `copy_of` is the record of client.monsters (the original Monster.SData, imported by tools/sdata/load_client.py)
-- that the new record starts as; `fields` holds the fields set on top of it, by client_sdata.MONSTER_FIELDS name (and `name`). Client-only:
-- there is no defs row for a client record, and client_patch.py generates Monster.SData from these rows when there are any.
CREATE TABLE IF NOT EXISTS content.client_monsters (
    id          integer PRIMARY KEY CHECK (id BETWEEN 1 AND 65535),
    extension   text NOT NULL REFERENCES content.extensions (name),
    copy_of     integer NOT NULL CHECK (copy_of >= 0),
    fields      jsonb NOT NULL,
    since_patch integer,
    note        text NOT NULL
);

-- The ids an extension owns gain the monster records: major 0, minor = the record ids.
ALTER TABLE content.ext_ranges DROP CONSTRAINT IF EXISTS ext_ranges_kind_check;
ALTER TABLE content.ext_ranges ADD CONSTRAINT ext_ranges_kind_check CHECK (kind IN ('item', 'skill', 'monster'));

-- The pet a character has out (section 8): the egg's craft name. Written when a pet comes out by an egg's use and deleted when it goes
-- by a use or because its egg left the bags; not at hiding, map changes or logout.
CREATE TABLE IF NOT EXISTS character_pets (
    character_id integer PRIMARY KEY REFERENCES characters (id) ON DELETE CASCADE,
    egg          text NOT NULL,
    changed      timestamptz NOT NULL DEFAULT now()
);
