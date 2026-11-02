-- The free skill reset (docs/original/item-use.md section 8): each character may reset its skills once for nothing (0x0215). The
-- original keeps this as a count (m_byResetSkill, capped at 1); 0x021B tells the client whether it is still there.
ALTER TABLE characters ADD COLUMN free_skill_reset_used boolean NOT NULL DEFAULT false;
