-- Replace the free-form privilege column with the original Users_Master.Status semantics:
-- 0 normal, 16/32/48/64/80 staff levels 1 to 5 (16 = full operator), other values block or restrict.
-- See crates/login-server/src/status.rs.
ALTER TABLE accounts ADD COLUMN status smallint NOT NULL DEFAULT 0;
UPDATE accounts SET status = privilege * 16 WHERE privilege BETWEEN 1 AND 5;
ALTER TABLE accounts DROP COLUMN privilege;
