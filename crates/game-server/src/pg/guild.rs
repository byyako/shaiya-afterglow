//! The guild store of the PostgreSQL backend.

use super::*;

/// A guild bank row: the guild and the item.
#[derive(sqlx::FromRow)]
struct GuildBankRow {
    guild_id: i32,
    #[sqlx(flatten)]
    item: ItemRow,
}

impl GuildStore for PgBackend {
    async fn load_ranking_due(&self) -> Result<Option<i64>> {
        let due: Option<Option<i64>> =
            sqlx::query_scalar("SELECT extract(epoch FROM due)::bigint FROM guild_ranking WHERE id = 1").fetch_optional(&self.pool).await?;
        Ok(due.flatten())
    }

    async fn load_guilds(&self, server_id: u8) -> Result<Vec<StoredGuild>> {
        // id, name, country, remark, ranking, points, etin, etin_returns, keep_etin, use_house, buy_house.
        type GuildRow = (i32, String, i16, String, i16, i64, i64, i32, i64, i16, bool);
        let guilds: Vec<GuildRow> = sqlx::query_as(
            "SELECT id, name, country, remark, ranking, points, etin, etin_returns, keep_etin, use_house, buy_house
               FROM guilds WHERE server_id = $1 AND disbanded_at IS NULL ORDER BY id",
        )
        .bind(i16::from(server_id))
        .fetch_all(&self.pool)
        .await?;
        let members: Vec<(i32, i32, String, i16, i16, i16)> = sqlx::query_as(
            "SELECT m.guild_id, c.id, c.name, c.level, c.job, m.rank
               FROM guild_members m JOIN characters c ON c.id = m.character_id JOIN guilds g ON g.id = m.guild_id
              WHERE g.server_id = $1 AND g.disbanded_at IS NULL AND c.deleted_at IS NULL
              ORDER BY m.joined_at, c.id",
        )
        .bind(i16::from(server_id))
        .fetch_all(&self.pool)
        .await?;
        let mut by_guild: HashMap<i32, Vec<StoredMember>> = HashMap::new();
        for (guild, id, name, level, job, rank) in members {
            let member = StoredMember {
                id: u32::try_from(id)?,
                name,
                level: u16::try_from(level)?,
                job: u8::try_from(job)?,
                rank: u8::try_from(rank)?,
            };
            by_guild.entry(guild).or_default().push(member);
        }
        let slots: Vec<(i32, i16, i32, i16)> = sqlx::query_as(
            "SELECT n.guild_id, n.slot, n.npc_key, n.level FROM guild_npc_levels n JOIN guilds g ON g.id = n.guild_id
              WHERE g.server_id = $1 AND g.disbanded_at IS NULL",
        )
        .bind(i16::from(server_id))
        .fetch_all(&self.pool)
        .await?;
        let ids: Vec<i32> = guilds.iter().map(|g| g.0).collect();
        let bank: Vec<GuildBankRow> = sqlx::query_as(
            "SELECT guild_id, 255::int2 AS bag, slot, type, type_id, quality, gems, count, craft_name FROM guild_bank_items
              WHERE guild_id = ANY($1) ORDER BY guild_id, slot",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await?;
        let mut bank_of: HashMap<i32, Vec<ItemUnit>> = HashMap::new();
        for row in bank {
            bank_of.entry(row.guild_id).or_default().push(row.item.into_unit()?);
        }
        let mut slots_of: HashMap<i32, Vec<(usize, u16, u16)>> = HashMap::new();
        for (guild, slot, key, level) in slots {
            slots_of.entry(guild).or_default().push((usize::try_from(slot)?, u16::try_from(key)?, u16::try_from(level)?));
        }
        guilds
            .into_iter()
            .map(|(id, name, country, remark, ranking, points, etin, etin_returns, keep_etin, use_house, bought)| {
                let members = by_guild.remove(&id).unwrap_or_default();
                let mut house = crate::guild_house::House {
                    ranking: u8::try_from(ranking)?,
                    points: u32::try_from(points)?,
                    etin: u32::try_from(etin)?,
                    etin_returns: u32::try_from(etin_returns)?,
                    keep_etin: u32::try_from(keep_etin)?,
                    use_house: u8::try_from(use_house)?,
                    bought,
                    ..Default::default()
                };
                for (slot, key, level) in slots_of.remove(&id).unwrap_or_default() {
                    if let Some(s) = house.slots.get_mut(slot) {
                        *s = (key, level);
                    }
                }
                Ok(StoredGuild {
                    id: u32::try_from(id)?,
                    name,
                    country: u8::try_from(country)?,
                    remark,
                    members,
                    house,
                    bank: bank_of.remove(&id).unwrap_or_default(),
                })
            })
            .collect()
    }

    async fn create_guild(&self, g: &NewGuild) -> Result<GuildCreated> {
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        let id: Option<i32> = sqlx::query_scalar(
            "INSERT INTO guilds (server_id, name, country, remark) VALUES ($1, $2, $3, $4)
             ON CONFLICT (server_id, lower(name)) WHERE disbanded_at IS NULL DO NOTHING RETURNING id",
        )
        .bind(i16::from(g.server_id))
        .bind(&g.name)
        .bind(i16::from(g.country))
        .bind(&g.remark)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(id) = id else { return Ok(GuildCreated::NameTaken) };
        let paid = sqlx::query("UPDATE characters SET money = money - $2 WHERE id = $1 AND money >= $2")
            .bind(i32::try_from(g.payer)?)
            .bind(i64::from(g.cost))
            .execute(&mut *tx)
            .await?;
        if paid.rows_affected() != 1 {
            return Ok(GuildCreated::NoMoney);
        }
        let ids: Vec<i32> = g.members.iter().map(|&(m, _)| i32::try_from(m)).collect::<Result<_, _>>()?;
        let ranks: Vec<i16> = g.members.iter().map(|&(_, r)| i16::from(r)).collect();
        // Their join requests elsewhere are withdrawn.
        sqlx::query("DELETE FROM guild_members WHERE character_id = ANY($1) AND rank = 0").bind(&ids).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO guild_members (character_id, guild_id, rank) SELECT unnest($1::integer[]), $2, unnest($3::smallint[])")
            .bind(&ids)
            .bind(id)
            .bind(&ranks)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE characters SET guild_joined_at = now() WHERE id = ANY($1)").bind(&ids).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(GuildCreated::Created(u32::try_from(id)?))
    }

    async fn buy_guild_house(&self, character_id: u32, guild_id: u32, price: u32, keep_etin: u32) -> Result<bool> {
        let (id, guild) = (i32::try_from(character_id)?, i32::try_from(guild_id)?);
        let mut tx = self.pool.begin().await?;
        let paid = sqlx::query("UPDATE characters SET money = money - $2 WHERE id = $1 AND money >= $2")
            .bind(id)
            .bind(i64::from(price))
            .execute(&mut *tx)
            .await?;
        if paid.rows_affected() != 1 {
            return Ok(false);
        }
        let bought = sqlx::query(
            "UPDATE guilds SET buy_house = true, keep_etin = $3
              WHERE id = $1 AND NOT buy_house AND disbanded_at IS NULL
                AND EXISTS (SELECT 1 FROM guild_members WHERE guild_id = $1 AND character_id = $2 AND rank = $4)",
        )
        .bind(guild)
        .bind(id)
        .bind(i64::from(keep_etin))
        .bind(i16::from(crate::guild::MASTER))
        .execute(&mut *tx)
        .await?;
        if bought.rows_affected() != 1 {
            return Ok(false);
        }
        tx.commit().await?;
        Ok(true)
    }

    async fn write_guild(&self, write: &GuildWrite) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        match write {
            &GuildWrite::Add { guild, character, rank } => {
                let character = i32::try_from(character)?;
                sqlx::query(
                    "INSERT INTO guild_members (character_id, guild_id, rank) VALUES ($1, $2, $3)
                     ON CONFLICT (character_id) DO UPDATE SET guild_id = EXCLUDED.guild_id, rank = EXCLUDED.rank, joined_at = now()",
                )
                .bind(character)
                .bind(i32::try_from(guild)?)
                .bind(i16::from(rank))
                .execute(&mut *tx)
                .await?;
                if rank != crate::guild::REQUEST {
                    sqlx::query("UPDATE characters SET guild_joined_at = now() WHERE id = $1").bind(character).execute(&mut *tx).await?;
                }
            }
            &GuildWrite::Accept { character } => {
                let character = i32::try_from(character)?;
                sqlx::query("UPDATE guild_members SET rank = $2, joined_at = now() WHERE character_id = $1")
                    .bind(character)
                    .bind(i16::from(crate::guild::NEW_MEMBER))
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE characters SET guild_joined_at = now() WHERE id = $1").bind(character).execute(&mut *tx).await?;
            }
            &GuildWrite::Rank { character, rank } => {
                sqlx::query("UPDATE guild_members SET rank = $2 WHERE character_id = $1")
                    .bind(i32::try_from(character)?)
                    .bind(i16::from(rank))
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::Remove { character } => {
                sqlx::query("DELETE FROM guild_members WHERE character_id = $1").bind(i32::try_from(character)?).execute(&mut *tx).await?;
            }
            GuildWrite::Remark { guild, remark } => {
                sqlx::query("UPDATE guilds SET remark = $2 WHERE id = $1")
                    .bind(i32::try_from(*guild)?)
                    .bind(remark)
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::Disband { guild } => {
                let guild = i32::try_from(guild)?;
                sqlx::query("UPDATE guilds SET disbanded_at = now() WHERE id = $1 AND disbanded_at IS NULL")
                    .bind(guild)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM guild_members WHERE guild_id = $1").bind(guild).execute(&mut *tx).await?;
            }
            &GuildWrite::Etin { guild, etin, returns } => {
                sqlx::query("UPDATE guilds SET etin = $2, etin_returns = $3 WHERE id = $1")
                    .bind(i32::try_from(guild)?)
                    .bind(i64::from(etin))
                    .bind(i32::try_from(returns)?)
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::Upgrade { guild, slot, key, level, etin, keep_etin } => {
                let guild = i32::try_from(guild)?;
                sqlx::query(
                    "INSERT INTO guild_npc_levels (guild_id, slot, npc_key, level) VALUES ($1, $2, $3, $4)
                     ON CONFLICT (guild_id, slot) DO UPDATE SET npc_key = EXCLUDED.npc_key, level = EXCLUDED.level",
                )
                .bind(guild)
                .bind(i16::from(slot))
                .bind(i32::from(key))
                .bind(i16::from(level))
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE guilds SET etin = $2, keep_etin = $3 WHERE id = $1")
                    .bind(guild)
                    .bind(i64::from(etin))
                    .bind(i64::from(keep_etin))
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::HouseBought { guild, keep_etin } => {
                sqlx::query("UPDATE guilds SET buy_house = true, keep_etin = $2 WHERE id = $1")
                    .bind(i32::try_from(guild)?)
                    .bind(i64::from(keep_etin))
                    .execute(&mut *tx)
                    .await?;
            }
            GuildWrite::Ranking { rows } => {
                for r in rows {
                    sqlx::query("UPDATE guilds SET points = $2, etin = $3, ranking = $4, use_house = $5, etin_returns = 0 WHERE id = $1")
                        .bind(i32::try_from(r.guild)?)
                        .bind(i64::from(r.points))
                        .bind(i64::from(r.etin))
                        .bind(i16::from(r.ranking))
                        .bind(i16::from(r.use_house))
                        .execute(&mut *tx)
                        .await?;
                }
            }
            &GuildWrite::Points { guild, points } => {
                sqlx::query("UPDATE guilds SET points = $2 WHERE id = $1")
                    .bind(i32::try_from(guild)?)
                    .bind(i64::from(points))
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::Standing { guild, ranking, use_house } => {
                sqlx::query("UPDATE guilds SET ranking = $2, use_house = $3 WHERE id = $1")
                    .bind(i32::try_from(guild)?)
                    .bind(i16::from(ranking))
                    .bind(i16::from(use_house))
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::BankCount { guild, slot, count } => {
                sqlx::query("UPDATE guild_bank_items SET count = $3 WHERE guild_id = $1 AND slot = $2")
                    .bind(i32::try_from(guild)?)
                    .bind(i16::from(slot))
                    .bind(i16::from(count))
                    .execute(&mut *tx)
                    .await?;
            }
            &GuildWrite::RankingDue(due) => {
                sqlx::query(
                    "UPDATE guild_ranking SET due = CASE WHEN $1::bigint IS NULL THEN NULL ELSE to_timestamp($1::bigint) END WHERE id = 1",
                )
                .bind(due)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
