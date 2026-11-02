-- The factions' kill counts of each game server (docs/original/pvp-2.md section 9): the original's world kill counts, which
-- ps_dbAgent kept for its server (0x0205). Written every ten minutes and at the resets, as there.
CREATE TABLE faction_kill_counts (
    server_id  smallint NOT NULL,
    country    smallint NOT NULL CHECK (country IN (0, 1)),
    count      bigint   NOT NULL DEFAULT 0 CHECK (count BETWEEN 0 AND 4294967295),
    PRIMARY KEY (server_id, country)
);
