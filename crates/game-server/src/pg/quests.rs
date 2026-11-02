//! The quest store of the PostgreSQL backend.

use super::*;

impl QuestStore for PgBackend {
    async fn start_quest(
        &self,
        character_id: u32,
        quest_id: u16,
        seconds_left: Option<u32>,
        changes: &[SlotChange],
    ) -> Result<Option<Vec<Changed>>> {
        let id = i32::try_from(character_id)?;
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        let mut rows = Vec::with_capacity(changes.len());
        for change in changes {
            let Some(changed) = apply_change(&mut tx, id, change).await? else { return Ok(None) };
            rows.push(changed);
        }
        // A finished row of the same quest goes, as the original's database agent does on a start (`docs/original/quest.md`
        // section 9).
        sqlx::query(
            "INSERT INTO character_quests (character_id, quest_id, seconds_left) VALUES ($1, $2, $3)
             ON CONFLICT (character_id, quest_id) DO UPDATE SET finished = false, success = false, counts = '{0,0,0}', seconds_left = $3",
        )
        .bind(id)
        .bind(i16::try_from(quest_id)?)
        .bind(seconds_left.map(i32::try_from).transpose()?)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(rows))
    }

    async fn quest_counts(&self, character_id: u32, quest_id: u16, counts: [u8; 3]) -> Result<()> {
        sqlx::query("UPDATE character_quests SET counts = $3 WHERE character_id = $1 AND quest_id = $2 AND NOT finished")
            .bind(i32::try_from(character_id)?)
            .bind(i16::try_from(quest_id)?)
            .bind(counts.map(i16::from).to_vec())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn end_quest(&self, character_id: u32, end: &QuestEnd) -> Result<Option<Vec<Changed>>> {
        let id = i32::try_from(character_id)?;
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        let paid = sqlx::query("UPDATE characters SET money = money + $2 WHERE id = $1 AND money + $2 BETWEEN 0 AND 4294967295")
            .bind(id)
            .bind(end.money)
            .execute(&mut *tx)
            .await?;
        if paid.rows_affected() != 1 {
            return Ok(None);
        }
        let mut rows = Vec::with_capacity(end.changes.len());
        for change in &end.changes {
            let Some(changed) = apply_change(&mut tx, id, change).await? else { return Ok(None) };
            rows.push(changed);
        }
        let quest_id = i16::try_from(end.quest_id)?;
        let ended = if end.keep {
            sqlx::query(
                "UPDATE character_quests SET finished = true, success = $3, counts = '{0,0,0}', seconds_left = NULL
                  WHERE character_id = $1 AND quest_id = $2 AND NOT finished",
            )
            .bind(id)
            .bind(quest_id)
            .bind(end.success)
            .execute(&mut *tx)
            .await?
        } else {
            sqlx::query("DELETE FROM character_quests WHERE character_id = $1 AND quest_id = $2 AND NOT finished")
                .bind(id)
                .bind(quest_id)
                .execute(&mut *tx)
                .await?
        };
        if ended.rows_affected() != 1 {
            return Ok(None);
        }
        tx.commit().await?;
        Ok(Some(rows))
    }

    async fn save_quest_times(&self, character_id: u32, times: &[(u16, u32)]) -> Result<()> {
        if times.is_empty() {
            return Ok(());
        }
        let ids: Vec<i16> = times.iter().map(|t| i16::try_from(t.0)).collect::<Result<_, _>>()?;
        let secs: Vec<i32> = times.iter().map(|t| i32::try_from(t.1)).collect::<Result<_, _>>()?;
        sqlx::query(
            "UPDATE character_quests q SET seconds_left = t.secs
               FROM unnest($2::smallint[], $3::integer[]) AS t(quest_id, secs)
              WHERE q.character_id = $1 AND q.quest_id = t.quest_id AND NOT q.finished",
        )
        .bind(i32::try_from(character_id)?)
        .bind(ids)
        .bind(secs)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
