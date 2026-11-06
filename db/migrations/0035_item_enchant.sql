-- world.item_enchant (docs/plans/enchant-table.md) for a database whose live `world` was built before the table existed: the same DDL as
-- db/world/schema.sql, empty, so the server keeps the built-in enchant tables until tools/world/import.py --reset imports
-- Data/ItemEnchant.ini. Only the live schema: the frozen original_world gets it with that re-import, which records it (a table added
-- here would be one the import did not record). A database without `world` yet makes it from the DDL at its first import. Guarded as
-- 0031, since the tests of other binaries run all migrations again.
DO $$
BEGIN
    IF to_regnamespace('world') IS NOT NULL THEN
        CREATE TABLE IF NOT EXISTS world.item_enchant (
            step          smallint PRIMARY KEY CHECK (step BETWEEN 0 AND 20),
            weapon_rate   integer  NOT NULL CHECK (weapon_rate >= 0),
            armour_rate   integer  NOT NULL CHECK (armour_rate >= 0),
            weapon_add    smallint NOT NULL CHECK (weapon_add >= 0),
            armour_add    smallint NOT NULL CHECK (armour_add >= 0)
        );
    END IF;
END
$$;
