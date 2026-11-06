-- Item effects and skill abilities of extensions (docs/plans/ext-3-effects.md section 4.2). Server-only: never in defs or the client, so
-- `content hashes defs` and the client patches are as before. Guarded (IF NOT EXISTS) as 0031: the tests of other binaries run all
-- migrations again inside throwaway schemas of the same database.
CREATE TABLE IF NOT EXISTS content.ext_item_effects (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    item_id   integer NOT NULL PRIMARY KEY,        -- one effect per item, whoever owns it
    effect    jsonb NOT NULL                       -- as written in rows.toml
);
CREATE TABLE IF NOT EXISTS content.ext_skill_abilities (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    skill_id  smallint NOT NULL,
    level     smallint NOT NULL,
    abilities jsonb NOT NULL,                      -- the list as written
    PRIMARY KEY (skill_id, level)
);
