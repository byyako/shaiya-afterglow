//! The item store of the PostgreSQL backend.

use super::*;

#[derive(sqlx::FromRow)]
pub(super) struct ItemRow {
    bag: i16,
    slot: i16,
    #[sqlx(rename = "type")]
    item_type: i16,
    type_id: i16,
    quality: i32,
    gems: Vec<u8>,
    count: i16,
    craft_name: String,
}

impl ItemRow {
    pub(super) fn into_unit(self) -> Result<ItemUnit> {
        Ok(ItemUnit {
            bag: u8::try_from(self.bag)?,
            slot: u8::try_from(self.slot)?,
            item_type: u8::try_from(self.item_type)?,
            type_id: u8::try_from(self.type_id)?,
            quality: u16::try_from(self.quality)?,
            gems: self.gems.as_slice().try_into()?,
            count: u8::try_from(self.count)?,
            craft_name: self.craft_name.into_bytes(),
        })
    }
}

#[derive(sqlx::FromRow)]
struct DroppedRow {
    gems: Vec<u8>,
    craft_name: String,
    make_type: String,
    made_at: i64,
    uid: i64,
}

impl ItemStore for PgBackend {
    async fn trade(&self, character_id: u32, money: i64, changes: &[SlotChange]) -> Result<Option<Vec<Changed>>> {
        let id = i32::try_from(character_id)?;
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        let paid = sqlx::query("UPDATE characters SET money = money + $2 WHERE id = $1 AND money + $2 BETWEEN 0 AND 4294967295")
            .bind(id)
            .bind(money)
            .execute(&mut *tx)
            .await?;
        if paid.rows_affected() != 1 {
            return Ok(None);
        }
        let bank = if changes.iter().any(touches_bank) {
            let (user, server): (i32, i16) =
                sqlx::query_as("SELECT user_id, server_id FROM characters WHERE id = $1").bind(id).fetch_one(&mut *tx).await?;
            Some(BankOwner { user, server })
        } else {
            None
        };
        let mut rows = Vec::with_capacity(changes.len());
        for change in changes {
            let applied = match bank {
                Some(owner) if touches_bank(change) => apply_bank_change(&mut tx, id, owner, change).await?,
                _ => apply_change(&mut tx, id, change).await?,
            };
            let Some(changed) = applied else { return Ok(None) };
            rows.push(changed);
        }
        tx.commit().await?;
        Ok(Some(rows))
    }

    async fn guild_bank(&self, character_id: u32, guild_id: u32, money: i64, changes: &[SlotChange]) -> Result<Option<Vec<Changed>>> {
        let (id, guild) = (i32::try_from(character_id)?, i32::try_from(guild_id)?);
        let mut tx = self.pool.begin().await?;
        let paid = sqlx::query("UPDATE characters SET money = money + $2 WHERE id = $1 AND money + $2 BETWEEN 0 AND 4294967295")
            .bind(id)
            .bind(money)
            .execute(&mut *tx)
            .await?;
        if paid.rows_affected() != 1 {
            return Ok(None);
        }
        let mut rows = Vec::with_capacity(changes.len());
        for change in changes {
            let applied = if touches_guild_bank(change) {
                apply_guild_bank_change(&mut tx, id, guild, change).await?
            } else {
                apply_change(&mut tx, id, change).await?
            };
            let Some(changed) = applied else { return Ok(None) };
            rows.push(changed);
        }
        tx.commit().await?;
        Ok(Some(rows))
    }

    async fn guild_return_etin(&self, character_id: u32, guild_id: u32, sum: u32, changes: &[SlotChange]) -> Result<Option<Vec<Changed>>> {
        let (id, guild) = (i32::try_from(character_id)?, i32::try_from(guild_id)?);
        let mut tx = self.pool.begin().await?;
        // Still a member (not a request) of a live guild; the lock keeps a removal from committing between this and the credit.
        let member: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM guild_members m JOIN guilds g ON g.id = m.guild_id
              WHERE m.character_id = $1 AND m.guild_id = $2 AND m.rank <> $3 AND g.disbanded_at IS NULL FOR SHARE OF m",
        )
        .bind(id)
        .bind(guild)
        .bind(i16::from(crate::guild::REQUEST))
        .fetch_optional(&mut *tx)
        .await?;
        if member.is_none() {
            return Ok(None);
        }
        let mut rows = Vec::with_capacity(changes.len());
        for change in changes {
            let Some(changed) = apply_change(&mut tx, id, change).await? else { return Ok(None) };
            rows.push(changed);
        }
        sqlx::query("UPDATE guilds SET etin = LEAST(etin + $2, 4294967295), etin_returns = etin_returns + 1 WHERE id = $1")
            .bind(guild)
            .bind(i64::from(sum))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(rows))
    }

    async fn make_types(&self, character_id: u32, places: &[(u8, u8)]) -> Result<Vec<u8>> {
        let id = i32::try_from(character_id)?;
        let mut types = Vec::with_capacity(places.len());
        for &(bag, slot) in places {
            let make_type: Option<String> =
                sqlx::query_scalar("SELECT make_type FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3")
                    .bind(id)
                    .bind(i16::from(bag))
                    .bind(i16::from(slot))
                    .fetch_optional(&self.pool)
                    .await?;
            types.push(make_type.and_then(|t| t.bytes().next()).unwrap_or(0));
        }
        Ok(types)
    }

    async fn drop_item(&self, character_id: u32, bag: u8, slot: u8, had: u8, count: u8) -> Result<Option<(Kept, Stored)>> {
        let id = i32::try_from(character_id)?;
        if count < had {
            let rows: Vec<StoredRow> = sqlx::query_as(concat!(
                "UPDATE character_items SET count = count - $5 WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4 RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(bag))
            .bind(i16::from(slot))
            .bind(i16::from(had))
            .bind(i16::from(count))
            .fetch_all(&self.pool)
            .await?;
            return Ok(rows.first().copied().map(|row| (Kept::SPLIT, stored(row))));
        }
        let row: Option<DroppedRow> = sqlx::query_as(
            "DELETE FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4
             RETURNING gems, craft_name, make_type, (extract(epoch FROM made_at) * 1000000)::int8 AS made_at, uid",
        )
        .bind(id)
        .bind(i16::from(bag))
        .bind(i16::from(slot))
        .bind(i16::from(had))
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| {
            let kept = Kept {
                gems: r.gems.as_slice().try_into()?,
                craft_name: Kept::pad_craft_name(r.craft_name.as_bytes()),
                make_type: r.make_type.bytes().next().unwrap_or(0),
                origin: Some((r.uid, r.made_at)),
            };
            Ok((kept, Stored { uid: r.uid, made_at: r.made_at }))
        })
        .transpose()
    }

    async fn take_stakes(&self, character_id: u32, money: u32, items: &[(u8, u8, u8, u8)]) -> Result<Option<Vec<(Kept, Stored)>>> {
        let id = i32::try_from(character_id)?;
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        let paid = sqlx::query("UPDATE characters SET money = money - $2 WHERE id = $1 AND money >= $2")
            .bind(id)
            .bind(i64::from(money))
            .execute(&mut *tx)
            .await?;
        if paid.rows_affected() != 1 {
            return Ok(None);
        }
        let mut kept = Vec::with_capacity(items.len());
        for &(bag, slot, had, count) in items {
            if count < had {
                let row: Option<(Vec<u8>, String, i64, i64)> = sqlx::query_as(
                    "UPDATE character_items SET count = count - $5 WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4
                     RETURNING gems, craft_name, uid, (extract(epoch FROM made_at) * 1000000)::int8",
                )
                .bind(id)
                .bind(i16::from(bag))
                .bind(i16::from(slot))
                .bind(i16::from(had))
                .bind(i16::from(count))
                .fetch_optional(&mut *tx)
                .await?;
                let Some((gems, craft_name, uid, made_at)) = row else { return Ok(None) };
                let craft_name = Kept::pad_craft_name(craft_name.as_bytes());
                kept.push((Kept { gems: gems.as_slice().try_into()?, craft_name, make_type: b'T', origin: None }, Stored { uid, made_at }));
                continue;
            }
            let row: Option<DroppedRow> = sqlx::query_as(
                "DELETE FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4
                 RETURNING gems, craft_name, make_type, (extract(epoch FROM made_at) * 1000000)::int8 AS made_at, uid",
            )
            .bind(id)
            .bind(i16::from(bag))
            .bind(i16::from(slot))
            .bind(i16::from(had))
            .fetch_optional(&mut *tx)
            .await?;
            let Some(r) = row else { return Ok(None) };
            let item = Kept {
                gems: r.gems.as_slice().try_into()?,
                craft_name: Kept::pad_craft_name(r.craft_name.as_bytes()),
                make_type: r.make_type.bytes().next().unwrap_or(0),
                origin: Some((r.uid, r.made_at)),
            };
            kept.push((item, Stored { uid: r.uid, made_at: r.made_at }));
        }
        tx.commit().await?;
        Ok(Some(kept))
    }

    async fn exchange(&self, money: [(u32, i64); 2], handovers: &[Handover]) -> Result<Option<Vec<Handed>>> {
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        for (id, amount) in money {
            let paid = sqlx::query("UPDATE characters SET money = money + $2 WHERE id = $1 AND money + $2 BETWEEN 0 AND 4294967295")
                .bind(i32::try_from(id)?)
                .bind(amount)
                .execute(&mut *tx)
                .await?;
            if paid.rows_affected() != 1 {
                return Ok(None);
            }
        }
        let mut rows = Vec::with_capacity(handovers.len());
        for h in handovers {
            let Some(handed) = hand_over(&mut tx, h).await? else { return Ok(None) };
            rows.push(handed);
        }
        tx.commit().await?;
        Ok(Some(rows))
    }
}

/// One item of a trade inside its transaction: the row moves to the receiver when all of it goes (its uid kept), otherwise a copy
/// with the count given is made for the receiver (a new uid, made now, `make_type` 'T') and the giver keeps the rest. The rows come
/// back; `None` when a place is not as the handover expects.
async fn hand_over(tx: &mut sqlx::PgConnection, h: &Handover) -> Result<Option<Handed>> {
    let (giver, receiver) = (i32::try_from(h.giver)?, i32::try_from(h.receiver)?);
    let (from_bag, from_slot) = (i16::from(h.from.0), i16::from(h.from.1));
    let (to_bag, to_slot) = (i16::from(h.to.0), i16::from(h.to.1));
    if h.count == h.had {
        let rows: Vec<StoredRow> = sqlx::query_as(concat!(
            "UPDATE character_items SET character_id = $5, bag = $6, slot = $7
              WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4
                AND NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $5 AND bag = $6 AND slot = $7)
              RETURNING ",
            stored_columns!()
        ))
        .bind(giver)
        .bind(from_bag)
        .bind(from_slot)
        .bind(i16::from(h.had))
        .bind(receiver)
        .bind(to_bag)
        .bind(to_slot)
        .fetch_all(&mut *tx)
        .await?;
        return Ok(match rows[..] {
            [row] => Some(Handed { receiver: Some(stored(row)), giver: None }),
            _ => None,
        });
    }
    let copied: Vec<StoredRow> = sqlx::query_as(concat!(
        "INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, gems, count, craft_name, make_type)
         SELECT $5, $6, $7, item_id, type, type_id, quality, gems, $8, craft_name, 'T' FROM character_items
          WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4
            AND NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $5 AND bag = $6 AND slot = $7)
          RETURNING ",
        stored_columns!()
    ))
    .bind(giver)
    .bind(from_bag)
    .bind(from_slot)
    .bind(i16::from(h.had))
    .bind(receiver)
    .bind(to_bag)
    .bind(to_slot)
    .bind(i16::from(h.count))
    .fetch_all(&mut *tx)
    .await?;
    let [copy] = copied[..] else { return Ok(None) };
    let kept: Vec<StoredRow> = sqlx::query_as(concat!(
        "UPDATE character_items SET count = count - $4 WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count > $4 RETURNING ",
        stored_columns!()
    ))
    .bind(giver)
    .bind(from_bag)
    .bind(from_slot)
    .bind(i16::from(h.count))
    .fetch_all(&mut *tx)
    .await?;
    Ok(match kept[..] {
        [row] => Some(Handed { receiver: Some(stored(copy)), giver: Some(stored(row)) }),
        _ => None,
    })
}
