-- Who holds each obelisk of each game server (docs/original/obelisks.md section 7): what the original's ps_dbAgent kept for its
-- server (0x0207), by map and obelisk number; country 0 light, 1 fury, 2 neutral, and the last faction that took it. Written at
-- every change of owner and at the resets.
CREATE TABLE obelisk_owners (
    server_id     smallint NOT NULL,
    map           smallint NOT NULL,
    number        smallint NOT NULL,
    country       smallint NOT NULL CHECK (country IN (0, 1, 2)),
    last_destroy  smallint NOT NULL CHECK (last_destroy IN (0, 1, 2)),
    PRIMARY KEY (server_id, map, number)
);
