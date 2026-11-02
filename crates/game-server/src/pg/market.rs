//! The market store of the PostgreSQL backend.

use super::*;

/// The display columns of a market item (`market_auctions`, `market_result_items`, `market_result_money`).
#[derive(sqlx::FromRow)]
struct MarketItemCols {
    #[sqlx(rename = "type")]
    item_type: i16,
    type_id: i16,
    quality: i32,
    gems: Vec<u8>,
    count: i16,
    craft_name: String,
}

impl MarketItemCols {
    fn into_item(self) -> Result<MarketItem> {
        Ok(MarketItem {
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
struct AuctionRow {
    id: i32,
    seller_id: i32,
    seller_name: String,
    ends_at: i64,
    min_money: i64,
    direct_money: i64,
    period: i16,
    deposit: i64,
    bidder_id: Option<i32>,
    bidder_name: String,
    price: i64,
    #[sqlx(flatten)]
    item: MarketItemCols,
}

#[derive(sqlx::FromRow)]
struct ResultItemRow {
    market_id: i32,
    character_id: i32,
    result: i16,
    deadline: i64,
    #[sqlx(flatten)]
    item: MarketItemCols,
}

#[derive(sqlx::FromRow)]
struct ResultMoneyRow {
    id: i32,
    market_id: i32,
    character_id: i32,
    result: i16,
    money: i64,
    deposit: i64,
    return_money: i64,
    deadline: i64,
    #[sqlx(flatten)]
    item: MarketItemCols,
}

impl MarketStore for PgBackend {
    async fn load_market(&self, server_id: u8) -> Result<(Vec<StoredAuction>, Vec<StoredItemRow>, Vec<StoredMoneyRow>)> {
        let server = i16::from(server_id);
        let auctions: Vec<AuctionRow> = sqlx::query_as(
            "SELECT id, seller_id, seller_name, (extract(epoch FROM ends_at) * 1000000)::int8 AS ends_at, min_money, direct_money, period,
                    deposit, bidder_id, bidder_name, price, type, type_id, quality, gems, count, craft_name
               FROM market_auctions WHERE server_id = $1 ORDER BY id",
        )
        .bind(server)
        .fetch_all(&self.pool)
        .await?;
        let items: Vec<ResultItemRow> = sqlx::query_as(
            "SELECT r.market_id, r.character_id, r.result, (extract(epoch FROM r.deadline) * 1000000)::int8 AS deadline,
                    r.type, r.type_id, r.quality, r.gems, r.count, r.craft_name
               FROM market_result_items r JOIN characters c ON c.id = r.character_id
              WHERE c.server_id = $1 ORDER BY r.market_id",
        )
        .bind(server)
        .fetch_all(&self.pool)
        .await?;
        let money: Vec<ResultMoneyRow> = sqlx::query_as(
            "SELECT r.id, r.market_id, r.character_id, r.result, r.money, r.deposit, r.return_money,
                    (extract(epoch FROM r.deadline) * 1000000)::int8 AS deadline, r.type, r.type_id, r.quality, r.gems, r.count, r.craft_name
               FROM market_result_money r JOIN characters c ON c.id = r.character_id
              WHERE c.server_id = $1 ORDER BY r.id",
        )
        .bind(server)
        .fetch_all(&self.pool)
        .await?;
        let auctions = auctions
            .into_iter()
            .map(|r| {
                Ok(StoredAuction {
                    id: u32::try_from(r.id)?,
                    seller: u32::try_from(r.seller_id)?,
                    seller_name: r.seller_name,
                    ends_at: r.ends_at,
                    min: u32::try_from(r.min_money)?,
                    direct: u32::try_from(r.direct_money)?,
                    period: u8::try_from(r.period)?,
                    deposit: u32::try_from(r.deposit)?,
                    bidder: r.bidder_id.map(|id| Ok::<_, anyhow::Error>((u32::try_from(id)?, r.bidder_name))).transpose()?,
                    price: u32::try_from(r.price)?,
                    item: r.item.into_item()?,
                })
            })
            .collect::<Result<_>>()?;
        let items = items
            .into_iter()
            .map(|r| {
                Ok(StoredItemRow {
                    market_id: u32::try_from(r.market_id)?,
                    character: u32::try_from(r.character_id)?,
                    result: u8::try_from(r.result)?,
                    deadline: r.deadline,
                    item: r.item.into_item()?,
                })
            })
            .collect::<Result<_>>()?;
        let money = money
            .into_iter()
            .map(|r| {
                Ok(StoredMoneyRow {
                    id: u32::try_from(r.id)?,
                    market_id: u32::try_from(r.market_id)?,
                    character: u32::try_from(r.character_id)?,
                    result: u8::try_from(r.result)?,
                    money: u32::try_from(r.money)?,
                    deposit: u32::try_from(r.deposit)?,
                    return_money: u32::try_from(r.return_money)?,
                    deadline: r.deadline,
                    item: r.item.into_item()?,
                })
            })
            .collect::<Result<_>>()?;
        Ok((auctions, items, money))
    }

    async fn market(&self, write: &MarketWrite) -> Result<Option<MarketWritten>> {
        let character = i32::try_from(write.character)?;
        // Dropping the transaction without committing rolls everything back.
        let mut tx = self.pool.begin().await?;
        if write.money != 0 {
            let paid = sqlx::query("UPDATE characters SET money = money + $2 WHERE id = $1 AND money + $2 BETWEEN 0 AND 4294967295")
                .bind(character)
                .bind(write.money)
                .execute(&mut *tx)
                .await?;
            if paid.rows_affected() != 1 {
                return Ok(None);
            }
        }
        let micros = |t: i64| t as f64;
        let mut written = MarketWritten::default();
        for op in &write.ops {
            match op {
                MarketOp::NewAuction(a) => {
                    let Some((bag, slot, had, count)) = write.take else { bail!("a new auction without an item to take") };
                    let whole = count == had;
                    let (bag, slot, had) = (i16::from(bag), i16::from(slot), i16::from(had));
                    let sql = if whole {
                        new_auction_sql!(market_item_columns!())
                    } else {
                        new_auction_sql!("item_id, type, type_id, $13, $14, $15, $16, make_type, now(), nextval('item_uid_seq')")
                    };
                    let mut query = sqlx::query_as::<_, (i32, i64, i64)>(sql)
                        .bind(i16::from(a.server_id))
                        .bind(i32::try_from(a.seller)?)
                        .bind(&a.seller_name)
                        .bind(micros(a.ends_at))
                        .bind(i64::from(a.min))
                        .bind(i64::from(a.direct))
                        .bind(i16::from(a.period))
                        .bind(i64::from(a.deposit))
                        .bind(character)
                        .bind(bag)
                        .bind(slot)
                        .bind(had);
                    if !whole {
                        query = query
                            .bind(i32::from(a.item.quality))
                            .bind(a.item.gems.to_vec())
                            .bind(i16::from(a.item.count))
                            .bind(String::from_utf8_lossy(&a.item.craft_name).into_owned());
                    }
                    let made: Option<(i32, i64, i64)> = query.fetch_optional(&mut *tx).await?;
                    let Some((id, uid, made_at)) = made else { return Ok(None) };
                    let taken: Vec<StoredRow> = if whole {
                        sqlx::query_as(concat!(
                            "DELETE FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4 RETURNING ",
                            stored_columns!()
                        ))
                        .bind(character)
                        .bind(bag)
                        .bind(slot)
                        .bind(had)
                        .fetch_all(&mut *tx)
                        .await?
                    } else {
                        sqlx::query_as(concat!(
                            "UPDATE character_items SET count = count - $5 WHERE character_id = $1 AND bag = $2 AND slot = $3 AND count = $4 RETURNING ",
                            stored_columns!()
                        ))
                        .bind(character)
                        .bind(bag)
                        .bind(slot)
                        .bind(had)
                        .bind(i16::from(count))
                        .fetch_all(&mut *tx)
                        .await?
                    };
                    let [taken] = taken[..] else { return Ok(None) };
                    written.taken = Some(stored(taken));
                    written.ops.push(Some(Stored { uid, made_at }));
                    written.auction_ids.push(u32::try_from(id)?);
                }
                MarketOp::EndAuction { id, character: to, result, deadline } => {
                    let sql = concat!(
                        "INSERT INTO market_result_items (market_id, character_id, result, deadline, ",
                        market_item_columns!(),
                        ") SELECT id, $2, $3, to_timestamp($4::float8 / 1000000), ",
                        market_item_columns!(),
                        " FROM market_auctions WHERE id = $1 RETURNING ",
                        stored_columns!()
                    );
                    let moved: Vec<StoredRow> = sqlx::query_as(sql)
                        .bind(i32::try_from(*id)?)
                        .bind(i32::try_from(*to)?)
                        .bind(i16::from(*result))
                        .bind(micros(*deadline))
                        .fetch_all(&mut *tx)
                        .await?;
                    let [moved] = moved[..] else { return Ok(None) };
                    written.ops.push(Some(stored(moved)));
                    sqlx::query("DELETE FROM market_auctions WHERE id = $1").bind(i32::try_from(*id)?).execute(&mut *tx).await?;
                }
                MarketOp::SetBid { id, bidder, name, price } => {
                    let set: Vec<StoredRow> = sqlx::query_as(concat!(
                        "UPDATE market_auctions SET bidder_id = $2, bidder_name = $3, price = $4 WHERE id = $1 RETURNING ",
                        stored_columns!()
                    ))
                    .bind(i32::try_from(*id)?)
                    .bind(i32::try_from(*bidder)?)
                    .bind(name)
                    .bind(i64::from(*price))
                    .fetch_all(&mut *tx)
                    .await?;
                    let [set] = set[..] else { return Ok(None) };
                    written.ops.push(Some(stored(set)));
                }
                MarketOp::NewMoneyRow(m) => {
                    let id: i32 = sqlx::query_scalar(
                        "INSERT INTO market_result_money (market_id, character_id, result, money, deposit, return_money, deadline, type, type_id,
                                                          quality, gems, count, craft_name)
                         VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7::float8 / 1000000), $8, $9, $10, $11, $12, $13)
                         RETURNING id",
                    )
                    .bind(i32::try_from(m.market_id)?)
                    .bind(i32::try_from(m.character)?)
                    .bind(i16::from(m.result))
                    .bind(i64::from(m.money))
                    .bind(i64::from(m.deposit))
                    .bind(i64::from(m.return_money))
                    .bind(micros(m.deadline))
                    .bind(i16::from(m.item.item_type))
                    .bind(i16::from(m.item.type_id))
                    .bind(i32::from(m.item.quality))
                    .bind(m.item.gems.to_vec())
                    .bind(i16::from(m.item.count))
                    .bind(String::from_utf8_lossy(&m.item.craft_name).into_owned())
                    .fetch_one(&mut *tx)
                    .await?;
                    written.money_ids.push(u32::try_from(id)?);
                    written.ops.push(None);
                }
                MarketOp::DeleteItemRow(id) => {
                    let gone: Vec<StoredRow> =
                        sqlx::query_as(concat!("DELETE FROM market_result_items WHERE market_id = $1 RETURNING ", stored_columns!()))
                            .bind(i32::try_from(*id)?)
                            .fetch_all(&mut *tx)
                            .await?;
                    let [gone] = gone[..] else { return Ok(None) };
                    written.ops.push(Some(stored(gone)));
                }
                MarketOp::DeleteMoneyRow(id) => {
                    let gone =
                        sqlx::query("DELETE FROM market_result_money WHERE id = $1").bind(i32::try_from(*id)?).execute(&mut *tx).await?;
                    if gone.rows_affected() != 1 {
                        return Ok(None);
                    }
                    written.ops.push(None);
                }
            }
        }
        if let Some((market_id, bag, slot)) = write.place {
            let market_id = i32::try_from(market_id)?;
            // Not ON CONFLICT: the slot key is deferrable (migration 0007).
            let sql = concat!(
                "INSERT INTO character_items (character_id, bag, slot, ",
                market_item_columns!(),
                ") SELECT $1, $2, $3, ",
                market_item_columns!(),
                " FROM market_result_items WHERE market_id = $4 AND character_id = $1
                    AND NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3) RETURNING ",
                stored_columns!()
            );
            let placed: Vec<StoredRow> =
                sqlx::query_as(sql).bind(character).bind(i16::from(bag)).bind(i16::from(slot)).bind(market_id).fetch_all(&mut *tx).await?;
            let [placed] = placed[..] else { return Ok(None) };
            written.placed = Some(stored(placed));
            sqlx::query("DELETE FROM market_result_items WHERE market_id = $1").bind(market_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(Some(written))
    }

    async fn save_interest(&self, character_id: u32, ids: &[u32]) -> Result<()> {
        let id = i32::try_from(character_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM market_interest WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
        for (position, &market_id) in ids.iter().enumerate() {
            sqlx::query("INSERT INTO market_interest (character_id, position, market_id) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(i16::try_from(position)?)
                .bind(i32::try_from(market_id)?)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
