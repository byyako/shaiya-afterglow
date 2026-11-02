-- The wire value for an ordinary player is 11; staff use 1 to 5 (see shaiya_proto::login::PRIVILEGE_PLAYER).
ALTER TABLE accounts ALTER COLUMN privilege SET DEFAULT 11;
UPDATE accounts SET privilege = 11 WHERE privilege = 0;
