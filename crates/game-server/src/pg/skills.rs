//! The skill store of the PostgreSQL backend.

use super::*;

impl SkillStore for PgBackend {
    async fn learn(&self, character_id: u32, skill_points: u16, skill: Skill) -> Result<()> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO character_skills (character_id, number, skill_id, level, delay) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (character_id, number) DO UPDATE SET skill_id = EXCLUDED.skill_id, level = EXCLUDED.level, delay = EXCLUDED.delay",
        )
        .bind(id)
        .bind(i16::from(skill.number))
        .bind(i16::try_from(skill.id)?)
        .bind(i16::from(skill.level))
        .bind(i32::from(skill.delay))
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE characters SET skill_point = $2 WHERE id = $1")
            .bind(id)
            .bind(i16::try_from(skill_points)?)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn reset_skills(&self, character_id: u32, skill_points: u16, free: bool) -> Result<()> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM character_skills WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE characters SET skill_point = $2, free_skill_reset_used = free_skill_reset_used OR $3 WHERE id = $1")
            .bind(id)
            .bind(i16::try_from(skill_points)?)
            .bind(free)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn save_quick_slots(&self, character_id: u32, slots: &[QuickSlot]) -> Result<()> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM character_quick_slots WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
        for (position, q) in slots.iter().enumerate() {
            sqlx::query(
                "INSERT INTO character_quick_slots (character_id, position, bar, slot, bag, number) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(i16::try_from(position)?)
            .bind(i16::from(q.bar))
            .bind(i16::from(q.slot))
            .bind(i16::from(q.bag))
            .bind(i32::from(q.number))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn save_applied(&self, character_id: u32, applied: &[SavedApplied]) -> Result<()> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM character_applied_skills WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
        for (position, a) in applied.iter().take(256).enumerate() {
            sqlx::query(
                "INSERT INTO character_applied_skills (character_id, position, skill_id, level, seconds) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(i16::try_from(position)?)
            .bind(i16::try_from(a.skill_id)?)
            .bind(i16::from(a.level))
            .bind(i32::try_from(a.seconds)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn save_cooldowns(&self, character_id: u32, skills: &[Skill]) -> Result<()> {
        let numbers: Vec<i16> = skills.iter().map(|s| i16::from(s.number)).collect();
        let delays: Vec<i32> = skills.iter().map(|s| i32::from(s.delay)).collect();
        sqlx::query(
            "UPDATE character_skills c SET delay = d.delay FROM unnest($2::smallint[], $3::integer[]) AS d(number, delay)
              WHERE c.character_id = $1 AND c.number = d.number AND c.delay <> d.delay",
        )
        .bind(i32::try_from(character_id)?)
        .bind(numbers)
        .bind(delays)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
