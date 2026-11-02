//! The point store of the PostgreSQL backend (the item mall).

use super::*;

/// The account's points taken if it has them, in the transaction; the points left, `None` when short.
async fn take_points(tx: &mut sqlx::PgConnection, user_id: u32, price: u32) -> Result<Option<u32>> {
    let left: Option<i32> = sqlx::query_scalar("UPDATE accounts SET points = points - $2 WHERE id = $1 AND points >= $2 RETURNING points")
        .bind(i32::try_from(user_id)?)
        .bind(i32::try_from(price)?)
        .fetch_optional(tx)
        .await?;
    Ok(left.map(u32::try_from).transpose()?)
}

async fn add_log(
    tx: &mut sqlx::PgConnection,
    user_id: u32,
    character_id: u32,
    kind: i16,
    code: &str,
    points: u32,
    target: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO point_log (user_id, character_id, kind, product_code, points, target_name) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(i32::try_from(user_id)?)
        .bind(i32::try_from(character_id)?)
        .bind(kind)
        .bind(code)
        .bind(i32::try_from(points)?)
        .bind(target)
        .execute(tx)
        .await?;
    Ok(())
}

impl PointStore for PgBackend {
    async fn points(&self, user_id: u32) -> Result<u32> {
        let points: i32 =
            sqlx::query_scalar("SELECT points FROM accounts WHERE id = $1").bind(i32::try_from(user_id)?).fetch_one(&self.pool).await?;
        Ok(u32::try_from(points)?)
    }

    async fn buy(&self, p: &Purchase) -> Result<Bought> {
        let mut tx = self.pool.begin().await?;
        let Some(left) = take_points(&mut tx, p.user_id, p.price).await? else { return Ok(Bought::NoPoints) };
        let mut rows = Vec::with_capacity(p.items.len());
        for item in &p.items {
            let Some(row) = insert_item(&mut tx, i32::try_from(p.character_id)?, item).await? else { return Ok(Bought::Refused) };
            rows.push(row);
        }
        add_log(&mut tx, p.user_id, p.character_id, 1, &p.code, p.price, "").await?;
        tx.commit().await?;
        Ok(Bought::Done(left, rows))
    }

    async fn gift(&self, g: &GiftOrder) -> Result<GiftOutcome> {
        let mut tx = self.pool.begin().await?;
        let target: Option<i32> = sqlx::query_scalar(
            "SELECT user_id FROM characters WHERE lower(name) = lower($1) AND server_id = $2 AND deleted_at IS NULL AND delete_requested_at IS NULL",
        )
        .bind(&g.target)
        .bind(i16::from(g.server_id))
        .fetch_optional(&mut *tx)
        .await?;
        let Some(target) = target else { return Ok(GiftOutcome::NoCharacter) };
        let target_user = u32::try_from(target)?;
        if target_user == g.user_id {
            return Ok(GiftOutcome::OwnAccount);
        }
        // One gift at a time into an account's storage: its free slots are read under this lock. Both accounts are locked, in id order,
        // so two gifts between the same two accounts the other way round cannot lock them in opposite orders (audit R25).
        sqlx::query("SELECT id FROM accounts WHERE id IN ($1, $2) ORDER BY id FOR UPDATE")
            .bind(target)
            .bind(i32::try_from(g.user_id)?)
            .execute(&mut *tx)
            .await?;
        let taken: Vec<i16> = sqlx::query_scalar("SELECT slot FROM point_items WHERE user_id = $1 AND server_id = $2")
            .bind(target)
            .bind(i16::from(g.server_id))
            .fetch_all(&mut *tx)
            .await?;
        let free: Vec<u8> =
            (0..shaiya_proto::point::STORAGE_SLOTS).filter(|s| !taken.contains(&i16::from(*s))).take(g.rows.len()).collect();
        if free.len() < g.rows.len() {
            return Ok(GiftOutcome::StorageFull);
        }
        let Some(points_left) = take_points(&mut tx, g.user_id, g.price).await? else { return Ok(GiftOutcome::NoPoints) };
        let mut rows = Vec::new();
        for (&slot, &(item_id, count)) in free.iter().zip(&g.rows) {
            sqlx::query("INSERT INTO point_items (user_id, server_id, slot, item_id, count) VALUES ($1, $2, $3, $4, $5)")
                .bind(target)
                .bind(i16::from(g.server_id))
                .bind(i16::from(slot))
                .bind(i32::try_from(item_id)?)
                .bind(i16::from(count))
                .execute(&mut *tx)
                .await?;
            rows.push(StoredPointItem { slot, item_id, count });
        }
        add_log(&mut tx, g.user_id, g.character_id, 2, &g.code, g.price, &g.target).await?;
        tx.commit().await?;
        Ok(GiftOutcome::Sent { points_left, target_user, rows })
    }

    async fn add_gift_notice(&self, user_id: u32, server_id: u8, notice: &GiftNotice) -> Result<()> {
        sqlx::query("INSERT INTO point_gift_notices (user_id, server_id, product_code, sender_name) VALUES ($1, $2, $3, $4)")
            .bind(i32::try_from(user_id)?)
            .bind(i16::from(server_id))
            .bind(&notice.code)
            .bind(&notice.sender)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn take_gift_notices(&self, user_id: u32, server_id: u8) -> Result<Vec<GiftNotice>> {
        let mut rows: Vec<(i64, String, String)> =
            sqlx::query_as("DELETE FROM point_gift_notices WHERE user_id = $1 AND server_id = $2 RETURNING id, sender_name, product_code")
                .bind(i32::try_from(user_id)?)
                .bind(i16::from(server_id))
                .fetch_all(&self.pool)
                .await?;
        rows.sort_by_key(|r| r.0);
        Ok(rows.into_iter().map(|(_, sender, code)| GiftNotice { sender, code }).collect())
    }

    async fn take_stored(&self, character_id: u32, storage: Storage, had: StoredPointItem, item: &NewItem) -> Result<Option<Stored>> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        let sql = match storage {
            Storage::Mall => {
                "DELETE FROM point_items p USING characters c
                  WHERE c.id = $1 AND p.user_id = c.user_id AND p.server_id = c.server_id AND p.slot = $2 AND p.item_id = $3 AND p.count = $4"
            }
            Storage::Billing => {
                "DELETE FROM billing_items b USING characters c
                  WHERE c.id = $1 AND b.user_id = c.user_id AND b.slot = $2 AND b.item_id = $3 AND b.count = $4"
            }
        };
        let deleted = sqlx::query(sql)
            .bind(id)
            .bind(i16::from(had.slot))
            .bind(i32::try_from(had.item_id)?)
            .bind(i16::from(had.count))
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Ok(None);
        }
        let Some(row) = insert_item(&mut tx, id, item).await? else { return Ok(None) };
        tx.commit().await?;
        Ok(Some(row))
    }
}
