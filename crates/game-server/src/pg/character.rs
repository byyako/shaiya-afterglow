//! The character store of the PostgreSQL backend: the character list, creation and deletion, and entering the game.

use super::*;

/// A deleted character's name stays reserved this long when the character was above `RESERVED_NAME_LEVEL` (original rule).
const RESERVED_NAME_DAYS: f64 = 7.0;
const RESERVED_NAME_LEVEL: i16 = 10;

#[derive(sqlx::FromRow)]
struct ListRow {
    id: i32,
    slot: i16,
    created: i64,
    level: i16,
    family: i16,
    grow: i16,
    hair: i16,
    face: i16,
    size: i16,
    job: i16,
    sex: i16,
    map: i16,
    str: i16,
    dex: i16,
    rec: i16,
    int: i16,
    wis: i16,
    luc: i16,
    hp: i16,
    mp: i16,
    sp: i16,
    name: String,
    delete_pending: bool,
    rename_allowed: bool,
}

impl ListRow {
    fn into_summary(self) -> Result<CharacterSummary> {
        let u16 = |v: i16| u16::try_from(v);
        let u8 = |v: i16| u8::try_from(v);
        Ok(CharacterSummary {
            slot: u8(self.slot)?,
            id: u32::try_from(self.id)?,
            // The client does not use it; seconds since 1970, truncated.
            create_time: self.created as u32,
            level: u16(self.level)?,
            family: u8(self.family)?,
            grow: u8(self.grow)?,
            hair: u8(self.hair)?,
            face: u8(self.face)?,
            size: u8(self.size)?,
            job: u8(self.job)?,
            sex: u8(self.sex)?,
            map: u16(self.map)?,
            stats: [u16(self.str)?, u16(self.dex)?, u16(self.rec)?, u16(self.int)?, u16(self.wis)?, u16(self.luc)?],
            health: [u16(self.hp)?, u16(self.mp)?, u16(self.sp)?],
            // Filled from the equipment by `characters`.
            equipment_type: [0; 8],
            equipment_type_id: [0; 8],
            name: self.name.into_bytes(),
            delete_pending: self.delete_pending,
            rename_allowed: self.rename_allowed,
            cloak: [0; 6],
        })
    }
}

impl CharacterStore for PgBackend {
    async fn faction(&self, user_id: u32, server_id: u8) -> Result<Option<Faction>> {
        let row: Option<(i16, i16)> =
            sqlx::query_as("SELECT country, max_grow FROM account_factions WHERE user_id = $1 AND server_id = $2")
                .bind(i32::try_from(user_id)?)
                .bind(i16::from(server_id))
                .fetch_optional(&self.pool)
                .await?;
        row.map(|(country, max_grow)| Ok(Faction { country: u8::try_from(country)?, max_grow: u8::try_from(max_grow)? })).transpose()
    }

    async fn choose_faction(&self, user_id: u32, server_id: u8, country: u8) -> Result<bool> {
        if country > 1 {
            bail!("country must be 0 or 1");
        }
        let done = sqlx::query(
            "INSERT INTO account_factions (user_id, server_id, country)
             SELECT $1, $2, $3
              WHERE NOT EXISTS (SELECT 1 FROM characters WHERE user_id = $1 AND server_id = $2 AND deleted_at IS NULL)
             ON CONFLICT (user_id, server_id) DO UPDATE SET country = EXCLUDED.country",
        )
        .bind(i32::try_from(user_id)?)
        .bind(i16::from(server_id))
        .bind(i16::from(country))
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn characters(&self, user_id: u32, server_id: u8) -> Result<Vec<CharacterSummary>> {
        let user = i32::try_from(user_id)?;
        let server = i16::from(server_id);
        let mut tx = self.pool.begin().await?;
        let removed = sqlx::query(
            "UPDATE characters SET deleted_at = now()
              WHERE user_id = $1 AND server_id = $2 AND deleted_at IS NULL
                AND delete_requested_at <= now() - make_interval(secs => $3)",
        )
        .bind(user)
        .bind(server)
        .bind(self.delete_delay.as_secs_f64())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let rows: Vec<ListRow> = sqlx::query_as(
            "SELECT id, slot, extract(epoch FROM created_at)::bigint AS created, level, family, grow, hair, face, size, job, sex,
                    map, str, dex, rec, int, wis, luc, hp, mp, sp, name,
                    delete_requested_at IS NOT NULL AS delete_pending, rename_count > 0 AS rename_allowed
               FROM characters WHERE user_id = $1 AND server_id = $2 AND deleted_at IS NULL ORDER BY slot",
        )
        .bind(user)
        .bind(server)
        .fetch_all(&mut *tx)
        .await?;
        // What the selection screen shows of the equipment: bag 0 slots 0 to 7, and the cloak's gems as its look
        // (ps_dbAgent DBCharacter::LoadCharacterList).
        let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
        let equipment: Vec<(i32, i16, i16, i16, Vec<u8>)> = sqlx::query_as(
            "SELECT character_id, slot, type, type_id, gems FROM character_items
              WHERE character_id = ANY($1) AND bag = 0 AND slot < 8",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?;
        if removed > 0 && rows.is_empty() {
            sqlx::query("DELETE FROM account_factions WHERE user_id = $1 AND server_id = $2")
                .bind(user)
                .bind(server)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        let mut list = rows.into_iter().map(ListRow::into_summary).collect::<Result<Vec<_>>>()?;
        for (character_id, slot, item_type, type_id, gems) in equipment {
            let Some(c) = list.iter_mut().find(|c| c.id as i32 == character_id) else { continue };
            let slot = usize::try_from(slot)?;
            c.equipment_type[slot] = u8::try_from(item_type)?;
            c.equipment_type_id[slot] = u8::try_from(type_id)?;
            if slot == CLOAK_SLOT {
                c.cloak = gems.as_slice().try_into()?;
            }
        }
        Ok(list)
    }

    async fn create(&self, c: &NewCharacter) -> Result<CreateOutcome> {
        let mut tx = self.pool.begin().await?;
        let reserved: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM characters
                             WHERE lower(name) = lower($1)
                               AND ((deleted_at > now() - make_interval(days => $2) AND level > $3) OR fallen_at IS NOT NULL))",
        )
        .bind(&c.name)
        .bind(RESERVED_NAME_DAYS as i32)
        .bind(RESERVED_NAME_LEVEL)
        .fetch_one(&mut *tx)
        .await?;
        if reserved {
            return Ok(CreateOutcome::NameTaken);
        }
        let inserted: Result<i32, sqlx::Error> = sqlx::query_scalar(
            "INSERT INTO characters (user_id, server_id, slot, name, family, grow, hair, face, size, job, sex,
                                     level, stat_point, skill_point, str, dex, rec, int, wis, luc, hp, mp, sp,
                                     map, dir, money, pos_x, pos_y, pos_z)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23,
                     $24, $25, $26, $27, $28, $29)
             RETURNING id",
        )
        .bind(i32::try_from(c.user_id)?)
        .bind(i16::from(c.server_id))
        .bind(i16::from(c.slot))
        .bind(&c.name)
        .bind(i16::from(c.family))
        .bind(i16::from(c.grow))
        .bind(i16::from(c.hair))
        .bind(i16::from(c.face))
        .bind(i16::from(c.size))
        .bind(i16::from(c.job))
        .bind(i16::from(c.sex))
        .bind(i16::try_from(c.level)?)
        .bind(i16::try_from(c.stat_point)?)
        .bind(i16::try_from(c.skill_point)?)
        .bind(i16::try_from(c.stats[0])?)
        .bind(i16::try_from(c.stats[1])?)
        .bind(i16::try_from(c.stats[2])?)
        .bind(i16::try_from(c.stats[3])?)
        .bind(i16::try_from(c.stats[4])?)
        .bind(i16::try_from(c.stats[5])?)
        .bind(i16::try_from(c.health[0])?)
        .bind(i16::try_from(c.health[1])?)
        .bind(i16::try_from(c.health[2])?)
        .bind(i16::try_from(c.map)?)
        .bind(c.dir)
        .bind(i64::from(c.money))
        .bind(c.position[0])
        .bind(c.position[1])
        .bind(c.position[2])
        .fetch_one(&mut *tx)
        .await;
        match inserted {
            Ok(id) => {
                for item in &c.items {
                    if insert_item(&mut tx, id, item).await?.is_none() {
                        bail!("two starting items for bag {} slot {}", item.bag, item.slot);
                    }
                }
                tx.commit().await?;
                Ok(CreateOutcome::Created(u32::try_from(id)?))
            }
            Err(sqlx::Error::Database(e)) if e.constraint() == Some("characters_name_key") => Ok(CreateOutcome::NameTaken),
            Err(sqlx::Error::Database(e)) if e.constraint() == Some("characters_slot_key") => Ok(CreateOutcome::SlotTaken),
            Err(e) => Err(e.into()),
        }
    }

    async fn request_delete(&self, user_id: u32, server_id: u8, character_id: u32) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE characters SET delete_requested_at = now()
              WHERE id = $3 AND user_id = $1 AND server_id = $2 AND deleted_at IS NULL AND delete_requested_at IS NULL",
        )
        .bind(i32::try_from(user_id)?)
        .bind(i16::from(server_id))
        .bind(i32::try_from(character_id)?)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn restore(&self, user_id: u32, server_id: u8, character_id: u32) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE characters SET delete_requested_at = NULL
              WHERE id = $3 AND user_id = $1 AND server_id = $2 AND deleted_at IS NULL
                AND delete_requested_at > now() - make_interval(secs => $4)",
        )
        .bind(i32::try_from(user_id)?)
        .bind(i16::from(server_id))
        .bind(i32::try_from(character_id)?)
        .bind(self.delete_delay.as_secs_f64())
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn fall(&self, character_id: u32) -> Result<bool> {
        let done = sqlx::query("UPDATE characters SET deleted_at = now(), fallen_at = now() WHERE id = $1 AND deleted_at IS NULL")
            .bind(i32::try_from(character_id)?)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn load(&self, user_id: u32, server_id: u8, character_id: u32) -> Result<Option<PlayCharacter>> {
        let row: Option<PlayRow> = sqlx::query_as(
            "SELECT id, name, family, grow, hair, face, size, job, sex, level, stat_point, skill_point, str, dex, rec, int, wis, luc,
                    hp, mp, sp, map, dir,
                    pos_x, pos_y, pos_z,
                    exp, money, kills, deaths, victories, defeats, hg, vg, cg, og, ig, free_skill_reset_used, kill_level, dead_level,
                    (CASE WHEN guild_joined_at IS NOT NULL THEN greatest(extract(epoch FROM now() - guild_joined_at), 0) END)::float8 AS guild_joined_secs
               FROM characters
              WHERE id = $3 AND user_id = $1 AND server_id = $2 AND deleted_at IS NULL AND delete_requested_at IS NULL",
        )
        .bind(i32::try_from(user_id)?)
        .bind(i16::from(server_id))
        .bind(i32::try_from(character_id)?)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let items: Vec<ItemRow> = sqlx::query_as(
            "SELECT bag, slot, type, type_id, quality, gems, count, craft_name FROM character_items
              WHERE character_id = $1 ORDER BY bag, slot",
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await?;
        let bank: Vec<ItemRow> = sqlx::query_as(
            "SELECT 100::int2 AS bag, slot, type, type_id, quality, gems, count, craft_name FROM bank_items
              WHERE user_id = $1 AND server_id = $2 ORDER BY slot",
        )
        .bind(i32::try_from(user_id)?)
        .bind(i16::from(server_id))
        .fetch_all(&self.pool)
        .await?;
        let skills: Vec<(i16, i16, i16, i32)> =
            sqlx::query_as("SELECT number, skill_id, level, delay FROM character_skills WHERE character_id = $1 ORDER BY number")
                .bind(row.id)
                .fetch_all(&self.pool)
                .await?;
        let quick_slots: Vec<(i16, i16, i16, i32)> =
            sqlx::query_as("SELECT bar, slot, bag, number FROM character_quick_slots WHERE character_id = $1 ORDER BY position")
                .bind(row.id)
                .fetch_all(&self.pool)
                .await?;
        let applied: Vec<(i16, i16, i32, f64)> = sqlx::query_as(
            "SELECT skill_id, level, seconds, greatest(extract(epoch FROM now() - saved_at), 0)::float8 FROM character_applied_skills
              WHERE character_id = $1 ORDER BY position",
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await?;
        let quests: Vec<QuestRowSaved> = sqlx::query_as(
            "SELECT quest_id, finished, success, counts, seconds_left FROM character_quests WHERE character_id = $1 ORDER BY quest_id",
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await?;
        let save_points: Vec<(i16, i16, f32, f32, f32)> =
            sqlx::query_as("SELECT slot, map, x, y, z FROM character_save_points WHERE character_id = $1 ORDER BY slot")
                .bind(row.id)
                .fetch_all(&self.pool)
                .await?;
        let interest: Vec<i32> = sqlx::query_scalar("SELECT market_id FROM market_interest WHERE character_id = $1 ORDER BY position")
            .bind(row.id)
            .fetch_all(&self.pool)
            .await?;
        let mut character = row.into_play()?;
        character.account_id = user_id;
        character.account_name =
            sqlx::query_scalar("SELECT username FROM accounts WHERE id = $1").bind(i32::try_from(user_id)?).fetch_one(&self.pool).await?;
        character.mall = load_mall(&self.pool, user_id, server_id, character.id).await?;
        character.interest = interest.into_iter().map(u32::try_from).collect::<Result<_, _>>()?;
        character.save_points = save_points
            .into_iter()
            .map(|(slot, map, x, y, z)| Ok(SavePoint { slot: u8::try_from(slot)?, map: u16::try_from(map)?, position: [x, y, z] }))
            .collect::<Result<_>>()?;
        character.quests = quests
            .into_iter()
            .map(|(quest_id, finished, success, counts, seconds_left)| {
                let count = |i: usize| counts.get(i).map_or(Ok(0), |&c| u8::try_from(c));
                Ok(SavedQuest {
                    quest_id: u16::try_from(quest_id)?,
                    finished,
                    success,
                    counts: [count(0)?, count(1)?, count(2)?],
                    seconds_left: seconds_left.map(u32::try_from).transpose()?,
                })
            })
            .collect::<Result<_>>()?;
        character.offline_secs = applied.first().map_or(0, |a| a.3 as u64);
        character.applied = applied
            .into_iter()
            .map(|(skill_id, level, seconds, _)| {
                Ok(SavedApplied { skill_id: u16::try_from(skill_id)?, level: u8::try_from(level)?, seconds: u32::try_from(seconds)? })
            })
            .collect::<Result<_>>()?;
        character.items = items.into_iter().map(ItemRow::into_unit).collect::<Result<_>>()?;
        character.bank = bank.into_iter().map(ItemRow::into_unit).collect::<Result<_>>()?;
        character.skills = skills
            .into_iter()
            .map(|(number, id, level, delay)| {
                Ok(Skill {
                    id: u16::try_from(id)?,
                    level: u8::try_from(level)?,
                    number: u8::try_from(number)?,
                    delay: u16::try_from(delay)?,
                })
            })
            .collect::<Result<_>>()?;
        character.quick_slots = quick_slots
            .into_iter()
            .map(|(bar, slot, bag, number)| {
                Ok(QuickSlot {
                    bar: u8::try_from(bar)?,
                    slot: u8::try_from(slot)?,
                    bag: u8::try_from(bag)?,
                    number: u16::try_from(number)?,
                })
            })
            .collect::<Result<_>>()?;
        // The friends and blocked players who are not deleted, with their names and jobs as they are now (`docs/original/friends.md`
        // section 11, deviation 2).
        let lists: Vec<(bool, i32, String, i16, String)> = sqlx::query_as(
            "SELECT friend, id, name, job, memo FROM (
                 SELECT true AS friend, c.id, c.name, c.job, f.memo, f.created_at FROM character_friends f JOIN characters c ON c.id = f.friend_id
                  WHERE f.character_id = $1 AND c.deleted_at IS NULL AND c.delete_requested_at IS NULL
                 UNION ALL
                 SELECT false, c.id, c.name, c.job, b.memo, b.created_at FROM character_blocks b JOIN characters c ON c.id = b.blocked_id
                  WHERE b.character_id = $1 AND c.deleted_at IS NULL AND c.delete_requested_at IS NULL
             ) lists ORDER BY created_at, id",
        )
        .bind(i32::try_from(character.id)?)
        .fetch_all(&self.pool)
        .await?;
        for (friend, id, name, job, memo) in lists {
            let entry = StoredFriend { id: u32::try_from(id)?, name, job: u8::try_from(job)?, memo };
            if friend { character.friends.push(entry) } else { character.blocks.push(entry) }
        }
        Ok(Some(character))
    }

    async fn save_position(&self, character_id: u32, map: u16, position: [f32; 3], dir: i16, health: [u16; 3]) -> Result<()> {
        let [hp, mp, sp] = health.map(|v| v.min(i16::MAX as u16) as i16);
        // A position that is not finite is never written: the last good one stays (EP5.4's save check, `docs/original/ep5.4.md`,
        // "What was fixed"; `docs/original/move-check.md` section 10).
        if !position.iter().all(|v| v.is_finite()) {
            sqlx::query("UPDATE characters SET hp = $2, mp = $3, sp = $4 WHERE id = $1")
                .bind(i32::try_from(character_id)?)
                .bind(hp)
                .bind(mp)
                .bind(sp)
                .execute(&self.pool)
                .await?;
            return Ok(());
        }
        sqlx::query(
            "UPDATE characters SET map = $2, pos_x = $3, pos_y = $4, pos_z = $5, dir = $6, hp = $7, mp = $8, sp = $9 WHERE id = $1",
        )
        .bind(i32::try_from(character_id)?)
        .bind(i16::try_from(map)?)
        .bind(position[0])
        .bind(position[1])
        .bind(position[2])
        .bind(dir)
        .bind(hp)
        .bind(mp)
        .bind(sp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn save_honor(&self, character_id: u32, honor: u16) -> Result<()> {
        sqlx::query("UPDATE characters SET hg = $2 WHERE id = $1")
            .bind(i32::try_from(character_id)?)
            // Hg is signed, as in the original; the client reads it as u16.
            .bind(honor as i16)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_kills(&self, character_id: u32, kills: [u32; 4]) -> Result<()> {
        let [k, d, v, f] = kills.map(|n| i32::try_from(n).unwrap_or(i32::MAX));
        sqlx::query("UPDATE characters SET kills = $2, deaths = $3, victories = $4, defeats = $5 WHERE id = $1")
            .bind(i32::try_from(character_id)?)
            .bind(k)
            .bind(d)
            .bind(v)
            .bind(f)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_kill_steps(&self, character_id: u32, steps: [u8; 2]) -> Result<()> {
        sqlx::query("UPDATE characters SET kill_level = $2, dead_level = $3 WHERE id = $1")
            .bind(i32::try_from(character_id)?)
            .bind(i16::from(steps[0]))
            .bind(i16::from(steps[1]))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn claim_kill_reward(&self, character_id: u32, stat_point: u16, old_step: u8, new_step: u8) -> Result<bool> {
        let claimed = sqlx::query("UPDATE characters SET stat_point = $2, kill_level = $4 WHERE id = $1 AND kill_level = $3")
            .bind(i32::try_from(character_id)?)
            .bind(i16::try_from(stat_point)?)
            .bind(i16::from(old_step))
            .bind(i16::from(new_step))
            .execute(&self.pool)
            .await?;
        Ok(claimed.rows_affected() == 1)
    }

    async fn claim_death_reward(&self, character_id: u32, gold: u32, old_step: u8, new_step: u8) -> Result<bool> {
        let claimed = sqlx::query(
            "UPDATE characters SET money = money + $2, dead_level = $4
              WHERE id = $1 AND dead_level = $3 AND money + $2 <= 4294967295",
        )
        .bind(i32::try_from(character_id)?)
        .bind(i64::from(gold))
        .bind(i16::from(old_step))
        .bind(i16::from(new_step))
        .execute(&self.pool)
        .await?;
        Ok(claimed.rows_affected() == 1)
    }

    async fn save_stat_reset(&self, character_id: u32, progress: &Progress, health: [u16; 3], steps: [u8; 2]) -> Result<()> {
        let [str, dex, rec, int, wis, luc] = progress.stats.map(|v| v as i16);
        let [hp, mp, sp] = health.map(|v| v.min(i16::MAX as u16) as i16);
        sqlx::query(
            "UPDATE characters SET level = $2, exp = $3, stat_point = $4, skill_point = $5, str = $6, dex = $7, rec = $8, int = $9,
                    wis = $10, luc = $11, hp = $12, mp = $13, sp = $14, kill_level = $15, dead_level = $16
              WHERE id = $1",
        )
        .bind(i32::try_from(character_id)?)
        .bind(i16::try_from(progress.level)?)
        .bind(i64::from(progress.exp))
        .bind(i16::try_from(progress.stat_point)?)
        .bind(i16::try_from(progress.skill_point)?)
        .bind(str)
        .bind(dex)
        .bind(rec)
        .bind(int)
        .bind(wis)
        .bind(luc)
        .bind(hp)
        .bind(mp)
        .bind(sp)
        .bind(i16::from(steps[0]))
        .bind(i16::from(steps[1]))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn save_point(&self, character_id: u32, point: SavePoint) -> Result<()> {
        let [x, y, z] = point.position;
        sqlx::query(
            "INSERT INTO character_save_points (character_id, slot, map, x, y, z) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (character_id, slot) DO UPDATE SET map = EXCLUDED.map, x = EXCLUDED.x, y = EXCLUDED.y, z = EXCLUDED.z",
        )
        .bind(i32::try_from(character_id)?)
        .bind(i16::from(point.slot))
        .bind(i16::try_from(point.map)?)
        .bind(x)
        .bind(y)
        .bind(z)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn change_body(&self, character_id: u32, body: Body, (bag, slot): (u8, u8), had: u8) -> Result<bool> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        let scroll = if had > 1 { SlotChange::SetCount { bag, slot, count: had - 1 } } else { SlotChange::Remove { bag, slot } };
        let held: Option<i16> = sqlx::query_scalar("SELECT count FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3")
            .bind(id)
            .bind(i16::from(bag))
            .bind(i16::from(slot))
            .fetch_optional(&mut *tx)
            .await?;
        if held != Some(i16::from(had)) || apply_change(&mut tx, id, &scroll).await?.is_none() {
            return Ok(false);
        }
        sqlx::query("UPDATE characters SET hair = $2, face = $3, size = $4, sex = $5 WHERE id = $1")
            .bind(id)
            .bind(i16::from(body.hair))
            .bind(i16::from(body.face))
            .bind(i16::from(body.size))
            .bind(i16::from(body.sex))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn pet(&self, character_id: u32) -> Result<Option<String>> {
        Ok(sqlx::query_scalar("SELECT egg FROM character_pets WHERE character_id = $1")
            .bind(i32::try_from(character_id)?)
            .fetch_optional(&self.pool)
            .await?)
    }

    async fn set_pet(&self, character_id: u32, egg: Option<&str>) -> Result<()> {
        let id = i32::try_from(character_id)?;
        match egg {
            Some(egg) => {
                sqlx::query(
                    "INSERT INTO character_pets (character_id, egg) VALUES ($1, $2)
                     ON CONFLICT (character_id) DO UPDATE SET egg = EXCLUDED.egg, changed = now()",
                )
                .bind(id)
                .bind(egg)
                .execute(&self.pool)
                .await?
            }
            None => sqlx::query("DELETE FROM character_pets WHERE character_id = $1").bind(id).execute(&self.pool).await?,
        };
        Ok(())
    }

    async fn add_rename(&self, character_id: u32, stone: &SlotChange) -> Result<bool> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        let added = sqlx::query("UPDATE characters SET rename_count = rename_count + 1 WHERE id = $1 AND rename_count < 255")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if added.rows_affected() != 1 || apply_change(&mut tx, id, stone).await?.is_none() {
            return Ok(false);
        }
        tx.commit().await?;
        Ok(true)
    }

    async fn rename(&self, user_id: u32, server_id: u8, character_id: u32, new_name: &str) -> Result<RenameOutcome> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        let found: Option<(String, i16)> = sqlx::query_as(
            "SELECT name, rename_count FROM characters WHERE id = $1 AND user_id = $2 AND server_id = $3 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .bind(i32::try_from(user_id)?)
        .bind(i16::from(server_id))
        .fetch_optional(&mut *tx)
        .await?;
        let Some((old, count)) = found else { return Ok(RenameOutcome::NotFound) };
        if count <= 0 {
            return Ok(RenameOutcome::NoRename);
        }
        // The names creation refuses are refused here too: a live one (the unique index) or one still reserved.
        let reserved: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM characters
                             WHERE lower(name) = lower($1)
                               AND ((deleted_at > now() - make_interval(days => $2) AND level > $3) OR fallen_at IS NOT NULL))",
        )
        .bind(new_name)
        .bind(RESERVED_NAME_DAYS as i32)
        .bind(RESERVED_NAME_LEVEL)
        .fetch_one(&mut *tx)
        .await?;
        if reserved {
            return Ok(RenameOutcome::Taken);
        }
        let renamed = sqlx::query("UPDATE characters SET name = $2, old_name = name, rename_count = rename_count - 1 WHERE id = $1")
            .bind(id)
            .bind(new_name)
            .execute(&mut *tx)
            .await;
        match renamed {
            Ok(_) => {}
            Err(sqlx::Error::Database(e)) if e.constraint() == Some("characters_name_key") => return Ok(RenameOutcome::Taken),
            Err(e) => return Err(e.into()),
        }
        sqlx::query("INSERT INTO character_renames (server_id, character_id, old_name, new_name) VALUES ($1, $2, $3, $4)")
            .bind(i16::from(server_id))
            .bind(id)
            .bind(&old)
            .bind(new_name)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE market_auctions SET seller_name = $2 WHERE seller_id = $1").bind(id).bind(new_name).execute(&mut *tx).await?;
        sqlx::query("UPDATE market_auctions SET bidder_name = $2 WHERE bidder_id = $1").bind(id).bind(new_name).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(RenameOutcome::Renamed { old })
    }

    async fn save_progress(&self, character_id: u32, progress: &Progress, health: [u16; 3]) -> Result<()> {
        let [str, dex, rec, int, wis, luc] = progress.stats.map(|v| v as i16);
        let [hp, mp, sp] = health.map(|v| v.min(i16::MAX as u16) as i16);
        sqlx::query(
            "UPDATE characters SET level = $2, exp = $3, stat_point = $4, skill_point = $5, str = $6, dex = $7, rec = $8, int = $9,
                    wis = $10, luc = $11, hp = $12, mp = $13, sp = $14
              WHERE id = $1",
        )
        .bind(i32::try_from(character_id)?)
        .bind(i16::try_from(progress.level)?)
        .bind(i64::from(progress.exp))
        .bind(i16::try_from(progress.stat_point)?)
        .bind(i16::try_from(progress.skill_point)?)
        .bind(str)
        .bind(dex)
        .bind(rec)
        .bind(int)
        .bind(wis)
        .bind(luc)
        .bind(hp)
        .bind(mp)
        .bind(sp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Equipment slot of the cloak; its gems carry the cloak's look in the character list.
const CLOAK_SLOT: usize = 7;

#[derive(sqlx::FromRow)]
struct PlayRow {
    id: i32,
    name: String,
    family: i16,
    grow: i16,
    hair: i16,
    face: i16,
    size: i16,
    job: i16,
    sex: i16,
    level: i16,
    stat_point: i16,
    skill_point: i16,
    str: i16,
    dex: i16,
    rec: i16,
    int: i16,
    wis: i16,
    luc: i16,
    hp: i16,
    mp: i16,
    sp: i16,
    map: i16,
    dir: i16,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    exp: i64,
    money: i64,
    kills: i32,
    deaths: i32,
    victories: i32,
    defeats: i32,
    hg: i16,
    vg: i16,
    cg: i16,
    og: i16,
    ig: i16,
    free_skill_reset_used: bool,
    kill_level: i16,
    dead_level: i16,
    guild_joined_secs: Option<f64>,
}

impl PlayRow {
    fn into_play(self) -> Result<PlayCharacter> {
        let u16 = |v: i16| u16::try_from(v);
        let u8 = |v: i16| u8::try_from(v);
        let u32 = |v: i32| u32::try_from(v);
        Ok(PlayCharacter {
            id: u32(self.id)?,
            name: self.name,
            account_id: 0,
            account_name: String::new(),
            family: u8(self.family)?,
            grow: u8(self.grow)?,
            hair: u8(self.hair)?,
            face: u8(self.face)?,
            size: u8(self.size)?,
            job: u8(self.job)?,
            sex: u8(self.sex)?,
            level: u16(self.level)?,
            stat_point: u16(self.stat_point)?,
            skill_point: u16(self.skill_point)?,
            stats: [u16(self.str)?, u16(self.dex)?, u16(self.rec)?, u16(self.int)?, u16(self.wis)?, u16(self.luc)?],
            health: [u16(self.hp)?, u16(self.mp)?, u16(self.sp)?],
            map: u16(self.map)?,
            dir: self.dir,
            position: [self.pos_x, self.pos_y, self.pos_z],
            exp: u32::try_from(self.exp)?,
            money: u32::try_from(self.money)?,
            kills: [u32(self.kills)?, u32(self.deaths)?, u32(self.victories)?, u32(self.defeats)?],
            // Hg is a signed column in the original as well; the client reads it as u16.
            honor: self.hg as u16,
            vg: self.vg,
            cg: u8(self.cg)?,
            og: u8(self.og)?,
            ig: u8(self.ig)?,
            free_skill_reset_used: self.free_skill_reset_used,
            kill_steps: [u8(self.kill_level)?, u8(self.dead_level)?],
            items: Vec::new(),
            bank: Vec::new(),
            skills: Vec::new(),
            quick_slots: Vec::new(),
            applied: Vec::new(),
            offline_secs: 0,
            guild_joined_secs: self.guild_joined_secs.map(|secs| secs as u64),
            quests: Vec::new(),
            friends: Vec::new(),
            blocks: Vec::new(),
            save_points: Vec::new(),
            interest: Vec::new(),
            mall: Mall::default(),
        })
    }
}

/// The item mall's part of a character's load (`docs/original/item-mall.md` section 10): the account's points and premium, this
/// character's last 10 log rows (newest first), the account's item-mall storage on this server and its billing storage.
async fn load_mall(pool: &PgPool, user_id: u32, server_id: u8, character_id: u32) -> Result<Mall> {
    let user = i32::try_from(user_id)?;
    let (points, premium, until): (i32, i16, Option<i64>) =
        sqlx::query_as("SELECT points, premium, extract(epoch FROM premium_until)::bigint FROM accounts WHERE id = $1")
            .bind(user)
            .fetch_one(pool)
            .await?;
    let log: Vec<(String, i64, i32)> = sqlx::query_as(
        "SELECT product_code, extract(epoch FROM at)::bigint, points FROM point_log WHERE character_id = $1 ORDER BY at DESC, id DESC LIMIT 10",
    )
    .bind(i32::try_from(character_id)?)
    .fetch_all(pool)
    .await?;
    let stored: Vec<(i16, i32, i16)> =
        sqlx::query_as("SELECT slot, item_id, count FROM point_items WHERE user_id = $1 AND server_id = $2 ORDER BY slot")
            .bind(user)
            .bind(i16::from(server_id))
            .fetch_all(pool)
            .await?;
    let billing: Vec<(i16, i32, i16)> = sqlx::query_as("SELECT slot, item_id, count FROM billing_items WHERE user_id = $1 ORDER BY slot")
        .bind(user)
        .fetch_all(pool)
        .await?;
    let rows = |rows: Vec<(i16, i32, i16)>| -> Result<Vec<StoredPointItem>> {
        rows.into_iter()
            .map(|(slot, item_id, count)| {
                Ok(StoredPointItem { slot: u8::try_from(slot)?, item_id: u32::try_from(item_id)?, count: u8::try_from(count)? })
            })
            .collect()
    };
    Ok(Mall {
        points: u32::try_from(points)?,
        premium: Premium { kind: u8::try_from(premium)?, until },
        log: log
            .into_iter()
            .map(|(code, at, points)| Ok(PointLogRow { code, at, points: u32::try_from(points)? }))
            .collect::<Result<_>>()?,
        stored: rows(stored)?,
        billing: rows(billing)?,
    })
}
