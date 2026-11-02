//! The admin store of the PostgreSQL backend.

use super::*;

impl AdminStore for PgBackend {
    async fn load_live(&self, skills: &SkillDefs) -> Result<Live> {
        let items = load_item_defs(&self.pool).await?;
        Ok(load_live(&self.pool, &items, skills).await?.0)
    }

    async fn set_account_status(&self, user_id: u32, status: i16) -> Result<()> {
        sqlx::query("UPDATE accounts SET status = $2 WHERE id = $1").bind(i32::try_from(user_id)?).bind(status).execute(&self.pool).await?;
        Ok(())
    }

    async fn account_id(&self, username: &str) -> Result<Option<(u32, i16)>> {
        let row: Option<(i32, i16)> = sqlx::query_as("SELECT id, status FROM accounts WHERE lower(username) = lower($1) AND NOT deleted")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|(id, status)| Ok((u32::try_from(id)?, status))).transpose()
    }

    async fn save_marks(&self, character_id: u32, marks: Marks) -> Result<()> {
        sqlx::query("UPDATE characters SET grow = $2, vg = $3, cg = $4, og = $5, ig = $6 WHERE id = $1")
            .bind(i32::try_from(character_id)?)
            .bind(i16::from(marks.grow))
            .bind(marks.vg)
            .bind(i16::from(marks.cg))
            .bind(i16::from(marks.og))
            .bind(i16::from(marks.ig))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn character_account(&self, name: &str) -> Result<Option<(u32, i16)>> {
        let row: Option<(i32, i16)> = sqlx::query_as(
            "SELECT c.user_id, a.status FROM characters c JOIN accounts a ON a.id = c.user_id
              WHERE lower(c.name) = lower($1) AND c.deleted_at IS NULL",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|(id, status)| Ok((u32::try_from(id)?, status))).transpose()
    }

    async fn block_account(&self, user_id: u32, minutes: Option<u16>) -> Result<()> {
        sqlx::query("UPDATE accounts SET status = 2, blocked_until = CASE WHEN $2::int IS NULL THEN NULL ELSE now() + make_interval(mins => $2::int) END WHERE id = $1")
            .bind(i32::try_from(user_id)?)
            .bind(minutes.map(i32::from))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn lift_block(&self, user_id: u32) -> Result<bool> {
        let done = sqlx::query("UPDATE accounts SET status = 0, blocked_until = NULL WHERE id = $1 AND status BETWEEN 2 AND 6")
            .bind(i32::try_from(user_id)?)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn closed_quests(&self, server_id: u8) -> Result<Vec<u16>> {
        let rows: Vec<(i32,)> = sqlx::query_as("SELECT quest_id FROM closed_quests WHERE server_id = $1 ORDER BY quest_id")
            .bind(i16::from(server_id))
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(|(q,)| Ok(u16::try_from(q)?)).collect()
    }

    async fn set_quest_closed(&self, server_id: u8, quest: u16, closed: bool) -> Result<()> {
        if closed {
            sqlx::query("INSERT INTO closed_quests (server_id, quest_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        } else {
            sqlx::query("DELETE FROM closed_quests WHERE server_id = $1 AND quest_id = $2")
        }
        .bind(i16::from(server_id))
        .bind(i32::from(quest))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn character_id(&self, name: &str) -> Result<Option<u32>> {
        let row: Option<(i32,)> = sqlx::query_as("SELECT id FROM characters WHERE lower(name) = lower($1) AND deleted_at IS NULL")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|(id,)| Ok(u32::try_from(id)?)).transpose()
    }

    async fn add_event(&self, character_id: u32, name: &str) -> Result<bool> {
        // No conflict target: the primary key and the unique index on the lower-cased name both stop a repeat.
        let done = sqlx::query("INSERT INTO character_events (character_id, name) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(i32::try_from(character_id)?)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn remove_event(&self, character_id: u32, name: &str) -> Result<bool> {
        let done = sqlx::query("DELETE FROM character_events WHERE character_id = $1 AND lower(name) = lower($2)")
            .bind(i32::try_from(character_id)?)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn add_question(&self, server_id: u8, character_id: u32, name: &str, text: &[u8]) -> Result<u32> {
        let (id,): (i32,) = sqlx::query_as(
            "INSERT INTO user_questions (server_id, character_id, character_name, question) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(i16::from(server_id))
        .bind(i32::try_from(character_id)?)
        .bind(name)
        .bind(text)
        .fetch_one(&self.pool)
        .await?;
        Ok(u32::try_from(id)?)
    }

    async fn take_question(&self, id: u32, gm: u32) -> Result<()> {
        sqlx::query("UPDATE user_questions SET taken_by = $2 WHERE id = $1 AND answered_at IS NULL")
            .bind(i32::try_from(id)?)
            .bind(i32::try_from(gm)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn release_question(&self, id: u32) -> Result<()> {
        sqlx::query("UPDATE user_questions SET taken_by = NULL WHERE id = $1 AND answered_at IS NULL")
            .bind(i32::try_from(id)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn answer_question(&self, id: u32, gm: u32, kind: u8, text: &[u8], to: Option<&str>) -> Result<()> {
        sqlx::query(
            "UPDATE user_questions SET answered_at = now(), answered_by = $2, answer_kind = $3, answer = $4, answer_to = $5 \
             WHERE id = $1 AND answered_at IS NULL",
        )
        .bind(i32::try_from(id)?)
        .bind(i32::try_from(gm)?)
        .bind(i16::from(kind))
        .bind(text)
        .bind(to)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn open_questions(&self, server_id: u8) -> Result<Vec<StoredQuestion>> {
        sqlx::query("UPDATE user_questions SET taken_by = NULL WHERE server_id = $1 AND answered_at IS NULL AND taken_by IS NOT NULL")
            .bind(i16::from(server_id))
            .execute(&self.pool)
            .await?;
        let rows: Vec<(i32, i32, String, Vec<u8>, i64)> = sqlx::query_as(
            "SELECT id, character_id, character_name, question, extract(epoch FROM asked_at)::bigint FROM user_questions \
             WHERE server_id = $1 AND answered_at IS NULL ORDER BY id",
        )
        .bind(i16::from(server_id))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(id, character_id, name, text, asked_unix)| {
                Ok(StoredQuestion { id: u32::try_from(id)?, character_id: u32::try_from(character_id)?, name, text, asked_unix })
            })
            .collect()
    }
}
