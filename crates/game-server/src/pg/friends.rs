//! The friend store of the PostgreSQL backend.

use super::*;

impl FriendStore for PgBackend {
    async fn write_friends(&self, write: &FriendWrite) -> Result<()> {
        let id = |v: u32| i32::try_from(v);
        match write {
            FriendWrite::Friends(pairs) => {
                let (characters, friends): (Vec<i32>, Vec<i32>) =
                    pairs.iter().map(|&(c, f)| Ok((id(c)?, id(f)?))).collect::<Result<Vec<_>>>()?.into_iter().unzip();
                sqlx::query(
                    "INSERT INTO character_friends (character_id, friend_id) SELECT * FROM unnest($1::integer[], $2::integer[])
                     ON CONFLICT DO NOTHING",
                )
                .bind(characters)
                .bind(friends)
                .execute(&self.pool)
                .await?;
            }
            FriendWrite::RemoveFriend { character, friend } => {
                sqlx::query("DELETE FROM character_friends WHERE character_id = $1 AND friend_id = $2")
                    .bind(id(*character)?)
                    .bind(id(*friend)?)
                    .execute(&self.pool)
                    .await?;
            }
            FriendWrite::FriendMemo { character, friend, memo } => {
                sqlx::query("UPDATE character_friends SET memo = $3 WHERE character_id = $1 AND friend_id = $2")
                    .bind(id(*character)?)
                    .bind(id(*friend)?)
                    .bind(memo)
                    .execute(&self.pool)
                    .await?;
            }
            FriendWrite::Block { character, blocked } => {
                sqlx::query("INSERT INTO character_blocks (character_id, blocked_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                    .bind(id(*character)?)
                    .bind(id(*blocked)?)
                    .execute(&self.pool)
                    .await?;
            }
            FriendWrite::RemoveBlock { character, blocked } => {
                sqlx::query("DELETE FROM character_blocks WHERE character_id = $1 AND blocked_id = $2")
                    .bind(id(*character)?)
                    .bind(id(*blocked)?)
                    .execute(&self.pool)
                    .await?;
            }
            FriendWrite::BlockMemo { character, blocked, memo } => {
                sqlx::query("UPDATE character_blocks SET memo = $3 WHERE character_id = $1 AND blocked_id = $2")
                    .bind(id(*character)?)
                    .bind(id(*blocked)?)
                    .bind(memo)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }
}
