//! PostgreSQL implementation of the game server's backend. Tables: `db/migrations`.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Result, bail};
use shaiya_crypto::SessionKeys;
use shaiya_proto::world::GUILD_BANK;
use sqlx::PgPool;

use crate::backend::{
    AdminStore, Body, Bought, Changed, CharacterStore, CharacterSummary, Claim, Claimed, CreateOutcome, Faction, FriendStore, FriendWrite,
    GiftNotice, GiftOrder, GiftOutcome, GuildCreated, GuildStore, GuildWrite, Handed, Handover, Heartbeat, ItemStore, ItemUnit, Kept, Mall,
    MarketItem, MarketOp, MarketStore, MarketWrite, MarketWritten, Marks, NewCharacter, NewGuild, NewItem, PlayCharacter, PointLogRow,
    PointStore, Premium, Purchase, QuestEnd, QuestStore, QuickSlot, RenameOutcome, SavePoint, SavedQuest, SessionGate, Skill, SkillStore,
    Storage, Stored, StoredAuction, StoredFriend, StoredGuild, StoredItemRow, StoredMember, StoredMoneyRow, StoredPointItem,
    StoredQuestion,
};
use crate::buffs::SavedApplied;
use crate::combat::Progress;
use crate::creation::BaseItemDef;
use crate::drops::DropTables;
use crate::enter::ExpTable;
use crate::inventory::{BANK, ItemDef, ItemDefs, SlotChange};
use crate::mobs::{MobAttack, MobDef, MobDefs, SpawnArea, mob_country, respawn_time};
use crate::quests::{QuestDef, QuestDefs, QuestResult};
use crate::server::Live;
use crate::skills::{SkillDef, SkillDefs};
use crate::stats::StatusTable;
use crate::world::{BindArea, Gate, GateKeeper, MapNpcRow, Portal, World};

/// How long a session stays valid without a heartbeat once its player is in the game. The heartbeat renews it while a connection of
/// this server holds the account, so it runs out when this server stops or when its end was lost.
pub const SESSION_GAME_SECS: f64 = 120.0;
/// The login server's notification of every login (`docs/original/client-extras.md` section 3.2).
pub const ACCOUNT_LOGIN_CHANNEL: &str = "account_login";

/// The operator tool's notification of an account's points changed (`docs/original/item-mall.md` deviation 5): `<user id>`.
pub const ACCOUNT_POINTS_CHANNEL: &str = "account_points";

/// The `console` tool's command for a game server (`docs/original/operator-console.md` section 6 step 6):
/// `<server id> <request id> <command line>`.
pub const CONSOLE_CHANNEL: &str = "game_console";

/// The game server's answer to the tool: `<request id> <lines joined by \n>`.
pub const CONSOLE_REPLY_CHANNEL: &str = "game_console_reply";

/// What `NOTIFY` may carry is 8000 bytes; a reply stays under this.
pub const CONSOLE_REPLY_MAX: usize = 7900;

/// A `game_console` payload: the server it is for, the request id and the command line.
pub fn parse_console_request(payload: &str) -> Option<(u8, &str, &str)> {
    let (server, rest) = payload.trim().split_once(' ')?;
    let (request, line) = rest.trim_start().split_once(' ')?;
    Some((server.parse().ok()?, request, line.trim()))
}

/// A `game_console_reply` payload for a request: the lines joined by newlines, cut (at a line) to fit.
pub fn console_reply(request: &str, lines: &[String]) -> String {
    let mut reply = format!("{request} ");
    for (i, line) in lines.iter().enumerate() {
        if reply.len() + line.len() + 1 > CONSOLE_REPLY_MAX {
            reply.push_str("\n...");
            break;
        }
        if i > 0 {
            reply.push('\n');
        }
        reply.push_str(line);
    }
    reply
}

/// An `account_login` payload, `<user id> <created>`: the account and when its new session was made (microseconds since the
/// epoch, as `SessionGate::claim` returns it).
pub fn parse_account_login(payload: &str) -> Option<(u32, i64)> {
    let (user_id, created) = payload.split_once(' ')?;
    Some((user_id.parse().ok()?, created.parse().ok()?))
}

pub struct PgBackend {
    pool: PgPool,
    /// A deletion request takes effect after this long.
    delete_delay: Duration,
}

impl PgBackend {
    pub fn new(pool: PgPool, delete_delay: Duration) -> Self {
        Self { pool, delete_delay }
    }
}

/// The columns every item write returns (`docs/original/game-log.md` deviation 4): the row's unique id and making time in
/// microseconds since 1970.
macro_rules! stored_columns {
    () => {
        "uid, (extract(epoch FROM made_at) * 1000000)::int8"
    };
}

/// A row as `stored_columns!` returns it.
type StoredRow = (i64, i64);

fn stored((uid, made_at): StoredRow) -> Stored {
    Stored { uid, made_at }
}

/// The result of one change: exactly one row touched, or `None` (refused).
fn one(rows: Vec<StoredRow>) -> Option<Changed> {
    match rows[..] {
        [row] => Some(Changed { row: Some(stored(row)), swapped: None }),
        _ => None,
    }
}

/// Stores a new item; the row it got, or `None` if the slot is taken.
async fn insert_item(tx: &mut sqlx::PgConnection, character_id: i32, item: &NewItem) -> Result<Option<Stored>> {
    // An item back from the ground keeps its gems, craft name, make type, and when it left whole its uid and making time.
    let kept = item.kept.unwrap_or_default();
    let make_type = if kept.make_type == 0 { String::new() } else { char::from(kept.make_type).to_string() };
    let (uid, made_at) = kept.origin.unzip();
    let rows: Vec<StoredRow> = sqlx::query_as(
        // Not ON CONFLICT: the slot key is deferrable (migration 0007), which ON CONFLICT does not accept.
        concat!(
            "INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, count, gems, craft_name, make_type,
                                      made_at, uid)
         SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                COALESCE(to_timestamp($12::float8 / 1000000), now()), COALESCE($13, nextval('item_uid_seq'))
          WHERE NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3)
          RETURNING ",
            stored_columns!()
        ),
    )
    .bind(character_id)
    .bind(i16::from(item.bag))
    .bind(i16::from(item.slot))
    .bind(i32::try_from(item.item_id)?)
    .bind(i16::from(item.item_type))
    .bind(i16::from(item.type_id))
    .bind(i32::from(item.quality))
    .bind(i16::from(item.count))
    .bind(kept.gems.to_vec())
    .bind(String::from_utf8_lossy(kept.craft_name()).into_owned())
    .bind(make_type)
    .bind(made_at.map(|micros| micros as f64))
    .bind(uid)
    .fetch_all(tx)
    .await?;
    Ok(rows.first().copied().map(stored))
}

/// The account and server whose warehouse a character uses.
#[derive(Clone, Copy)]
struct BankOwner {
    user: i32,
    server: i16,
}

fn touches_bank(change: &SlotChange) -> bool {
    let bank = |(bag, _): (u8, u8)| bag == BANK;
    match change {
        SlotChange::Insert(item) => item.bag == BANK,
        SlotChange::SetCount { bag, .. }
        | SlotChange::SetQuality { bag, .. }
        | SlotChange::SetGems { bag, .. }
        | SlotChange::SetCraftName { bag, .. }
        | SlotChange::Remove { bag, .. } => *bag == BANK,
        SlotChange::Move { from, to } => bank(*from) || bank(*to),
        SlotChange::Swap { a, b } => bank(*a) || bank(*b),
    }
}

/// A change that involves the warehouse (bag 100, `bank_items`), inside a transaction; `None` if a place is not as the change
/// expects. An item crosses between a bag and the warehouse only by a move, which keeps the row's uid and dates.
async fn apply_bank_change(tx: &mut sqlx::PgConnection, id: i32, owner: BankOwner, change: &SlotChange) -> Result<Option<Changed>> {
    let BankOwner { user, server } = owner;
    let rows: Vec<StoredRow> = match *change {
        SlotChange::SetCount { slot, count, .. } => {
            sqlx::query_as(concat!("UPDATE bank_items SET count = $4 WHERE user_id = $1 AND server_id = $2 AND slot = $3 RETURNING ", stored_columns!()))
                .bind(user)
                .bind(server)
                .bind(i16::from(slot))
                .bind(i16::from(count))
                .fetch_all(&mut *tx)
                .await?
        }
        SlotChange::SetQuality { slot, quality, .. } => {
            sqlx::query_as(concat!("UPDATE bank_items SET quality = $4 WHERE user_id = $1 AND server_id = $2 AND slot = $3 RETURNING ", stored_columns!()))
                .bind(user)
                .bind(server)
                .bind(i16::from(slot))
                .bind(i32::from(quality))
                .fetch_all(&mut *tx)
                .await?
        }
        SlotChange::Remove { slot, .. } => {
            sqlx::query_as(concat!("DELETE FROM bank_items WHERE user_id = $1 AND server_id = $2 AND slot = $3 RETURNING ", stored_columns!()))
                .bind(user)
                .bind(server)
                .bind(i16::from(slot))
                .fetch_all(&mut *tx)
                .await?
        }
        SlotChange::Move { from: (BANK, from), to: (BANK, to) } => sqlx::query_as(concat!(
            "UPDATE bank_items SET slot = $4 WHERE user_id = $1 AND server_id = $2 AND slot = $3
                AND NOT EXISTS (SELECT 1 FROM bank_items WHERE user_id = $1 AND server_id = $2 AND slot = $4) RETURNING ",
            stored_columns!()
        ))
        .bind(user)
        .bind(server)
        .bind(i16::from(from))
        .bind(i16::from(to))
        .fetch_all(&mut *tx)
        .await?,
        SlotChange::Swap { a: (BANK, a), b: (BANK, b) } => {
            // One statement; the slot key is checked at its end (migration 0014).
            let swapped: Vec<(i64, i64, i16)> = sqlx::query_as(
                "UPDATE bank_items SET slot = CASE WHEN slot = $3 THEN $4 ELSE $3 END
                  WHERE user_id = $1 AND server_id = $2 AND slot IN ($3, $4)
              RETURNING uid, (extract(epoch FROM made_at) * 1000000)::int8, slot",
            )
            .bind(user)
            .bind(server)
            .bind(i16::from(a))
            .bind(i16::from(b))
            .fetch_all(&mut *tx)
            .await?;
            return Ok(swap_rows(&swapped, i16::from(a), i16::from(b)));
        }
        SlotChange::Move { from: (bag, slot), to: (BANK, to) } => sqlx::query_as(concat!(
            "WITH moved AS (
                DELETE FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3
                   AND NOT EXISTS (SELECT 1 FROM bank_items WHERE user_id = $4 AND server_id = $5 AND slot = $6)
                RETURNING item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
             INSERT INTO bank_items (user_id, server_id, slot, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
             SELECT $4, $5, $6, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid FROM moved RETURNING ",
            stored_columns!()
        ))
        .bind(id)
        .bind(i16::from(bag))
        .bind(i16::from(slot))
        .bind(user)
        .bind(server)
        .bind(i16::from(to))
        .fetch_all(&mut *tx)
        .await?,
        SlotChange::Move { from: (BANK, from), to: (bag, slot) } => sqlx::query_as(concat!(
            "WITH moved AS (
                DELETE FROM bank_items WHERE user_id = $1 AND server_id = $2 AND slot = $3
                   AND NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $4 AND bag = $5 AND slot = $6)
                RETURNING item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
             INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
             SELECT $4, $5, $6, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid FROM moved RETURNING ",
            stored_columns!()
        ))
        .bind(user)
        .bind(server)
        .bind(i16::from(from))
        .bind(id)
        .bind(i16::from(bag))
        .bind(i16::from(slot))
        .fetch_all(&mut *tx)
        .await?,
        // New items are never made in the warehouse, nothing swaps across it, and lapis and craft names change only in the bags.
        SlotChange::Insert(_)
        | SlotChange::Move { .. }
        | SlotChange::Swap { .. }
        | SlotChange::SetGems { .. }
        | SlotChange::SetCraftName { .. } => return Ok(None),
    };
    Ok(one(rows))
}

/// The two rows a swap statement returned (`uid`, making time, the slot they now hold): the one now at `a` and the one now at `b`;
/// `None` unless both are there.
fn swap_rows(rows: &[(i64, i64, i16)], a: i16, b: i16) -> Option<Changed> {
    let at = |slot: i16| rows.iter().find(|r| r.2 == slot).map(|r| Stored { uid: r.0, made_at: r.1 });
    (rows.len() == 2).then_some(Changed { row: at(a), swapped: at(b) })
}

fn touches_guild_bank(change: &SlotChange) -> bool {
    let bank = |(bag, _): (u8, u8)| bag == GUILD_BANK;
    match change {
        SlotChange::Insert(item) => item.bag == GUILD_BANK,
        SlotChange::SetCount { bag, .. }
        | SlotChange::SetQuality { bag, .. }
        | SlotChange::SetGems { bag, .. }
        | SlotChange::SetCraftName { bag, .. }
        | SlotChange::Remove { bag, .. } => *bag == GUILD_BANK,
        SlotChange::Move { from, to } => bank(*from) || bank(*to),
        SlotChange::Swap { a, b } => bank(*a) || bank(*b),
    }
}

/// A change that involves the guild's bank (bag 255, `guild_bank_items`), inside a transaction; `None` if a place is not as the
/// change expects. An item crosses between a bag and the bank by a move or a swap, which keep the rows' uids and dates.
async fn apply_guild_bank_change(tx: &mut sqlx::PgConnection, id: i32, guild: i32, change: &SlotChange) -> Result<Option<Changed>> {
    let rows: Vec<StoredRow> = match *change {
        SlotChange::SetCount { slot, count, .. } => {
            sqlx::query_as(concat!("UPDATE guild_bank_items SET count = $3 WHERE guild_id = $1 AND slot = $2 RETURNING ", stored_columns!()))
                .bind(guild)
                .bind(i16::from(slot))
                .bind(i16::from(count))
                .fetch_all(&mut *tx)
                .await?
        }
        SlotChange::Remove { slot, .. } => {
            sqlx::query_as(concat!("DELETE FROM guild_bank_items WHERE guild_id = $1 AND slot = $2 RETURNING ", stored_columns!()))
                .bind(guild)
                .bind(i16::from(slot))
                .fetch_all(&mut *tx)
                .await?
        }
        SlotChange::Move { from: (GUILD_BANK, from), to: (GUILD_BANK, to) } => sqlx::query_as(concat!(
            "UPDATE guild_bank_items SET slot = $3 WHERE guild_id = $1 AND slot = $2
                AND NOT EXISTS (SELECT 1 FROM guild_bank_items WHERE guild_id = $1 AND slot = $3) RETURNING ",
            stored_columns!()
        ))
        .bind(guild)
        .bind(i16::from(from))
        .bind(i16::from(to))
        .fetch_all(&mut *tx)
        .await?,
        SlotChange::Swap { a: (GUILD_BANK, a), b: (GUILD_BANK, b) } => {
            // One statement; the slot key is checked at its end (migration 0021).
            let swapped: Vec<(i64, i64, i16)> = sqlx::query_as(
                "UPDATE guild_bank_items SET slot = CASE WHEN slot = $2 THEN $3 ELSE $2 END WHERE guild_id = $1 AND slot IN ($2, $3)
              RETURNING uid, (extract(epoch FROM made_at) * 1000000)::int8, slot",
            )
            .bind(guild)
            .bind(i16::from(a))
            .bind(i16::from(b))
            .fetch_all(&mut *tx)
            .await?;
            return Ok(swap_rows(&swapped, i16::from(a), i16::from(b)));
        }
        SlotChange::Move { from: (bag, slot), to: (GUILD_BANK, to) } => move_bag_to_guild_bank(tx, id, guild, (bag, slot), to).await?,
        SlotChange::Move { from: (GUILD_BANK, from), to: (bag, slot) } => sqlx::query_as(concat!(
            "WITH moved AS (
                DELETE FROM guild_bank_items WHERE guild_id = $1 AND slot = $2
                   AND NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $3 AND bag = $4 AND slot = $5)
                RETURNING item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
             INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
             SELECT $3, $4, $5, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid FROM moved RETURNING ",
            stored_columns!()
        ))
        .bind(guild)
        .bind(i16::from(from))
        .bind(id)
        .bind(i16::from(bag))
        .bind(i16::from(slot))
        .fetch_all(&mut *tx)
        .await?,
        // A bag place and a bank place exchange their items: the bank's item out first, then the bag's into the freed bank slot, then
        // the first into the freed bag place.
        SlotChange::Swap { a: (bag, slot), b: (GUILD_BANK, bank) } | SlotChange::Swap { a: (GUILD_BANK, bank), b: (bag, slot) } => {
            // item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at (microseconds since the epoch), uid.
            type Row = (i32, i16, i16, i32, Vec<u8>, i16, String, String, i64, i64);
            let out: Option<Row> = sqlx::query_as(
                "DELETE FROM guild_bank_items WHERE guild_id = $1 AND slot = $2
                 RETURNING item_id, type, type_id, quality, gems, count, craft_name, make_type,
                           (extract(epoch FROM made_at) * 1000000)::int8, uid",
            )
            .bind(guild)
            .bind(i16::from(bank))
            .fetch_optional(&mut *tx)
            .await?;
            let Some((item_id, item_type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)) = out else {
                return Ok(None);
            };
            // The bag's item is now in the bank.
            let Some(Changed { row: Some(in_bank), .. }) = one(move_bag_to_guild_bank(tx, id, guild, (bag, slot), bank).await?) else {
                return Ok(None);
            };
            // The bag place was emptied by the move.
            let placed: Vec<StoredRow> = sqlx::query_as(concat!(
                "INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, gems, count, craft_name, make_type,
                                              made_at, uid)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, to_timestamp($12::float8 / 1000000), $13) RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(bag))
            .bind(i16::from(slot))
            .bind(item_id)
            .bind(item_type)
            .bind(type_id)
            .bind(quality)
            .bind(gems)
            .bind(count)
            .bind(craft_name)
            .bind(make_type)
            .bind(made_at as f64)
            .bind(uid)
            .fetch_all(&mut *tx)
            .await?;
            let [in_bag] = placed[..] else { return Ok(None) };
            let (in_bag, in_bank) = (Some(stored(in_bag)), Some(in_bank));
            // `row` is the one now at `a`.
            let bank_first = matches!(change, SlotChange::Swap { a: (GUILD_BANK, _), .. });
            return Ok(Some(if bank_first { Changed { row: in_bank, swapped: in_bag } } else { Changed { row: in_bag, swapped: in_bank } }));
        }
        // New items are never made in the bank, and lapis, qualities and craft names change only in the bags.
        SlotChange::Insert(_)
        | SlotChange::Move { .. }
        | SlotChange::Swap { .. }
        | SlotChange::SetQuality { .. }
        | SlotChange::SetGems { .. }
        | SlotChange::SetCraftName { .. } => return Ok(None),
    };
    Ok(one(rows))
}

/// A bag place's item into an empty bank slot, keeping its uid and making time; the row it became (none when it did not go).
async fn move_bag_to_guild_bank(
    tx: &mut sqlx::PgConnection,
    id: i32,
    guild: i32,
    (bag, slot): (u8, u8),
    bank: u8,
) -> Result<Vec<StoredRow>> {
    Ok(sqlx::query_as(concat!(
        "WITH moved AS (
            DELETE FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3
               AND NOT EXISTS (SELECT 1 FROM guild_bank_items WHERE guild_id = $4 AND slot = $5)
            RETURNING item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
         INSERT INTO guild_bank_items (guild_id, slot, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid)
         SELECT $4, $5, item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid FROM moved RETURNING ",
        stored_columns!()
    ))
    .bind(id)
    .bind(i16::from(bag))
    .bind(i16::from(slot))
    .bind(guild)
    .bind(i16::from(bank))
    .fetch_all(&mut *tx)
    .await?)
}

/// One bag change inside a transaction; `None` if a place is not as the change expects.
async fn apply_change(tx: &mut sqlx::PgConnection, id: i32, change: &SlotChange) -> Result<Option<Changed>> {
    let place = |(bag, slot): (u8, u8)| (i16::from(bag), i16::from(slot));
    let rows: Vec<StoredRow> = match change {
        SlotChange::Insert(item) => {
            return Ok(insert_item(tx, id, item).await?.map(|row| Changed { row: Some(row), swapped: None }));
        }
        SlotChange::SetCount { bag, slot, count } => {
            sqlx::query_as(concat!(
                "UPDATE character_items SET count = $4 WHERE character_id = $1 AND bag = $2 AND slot = $3 RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(*bag))
            .bind(i16::from(*slot))
            .bind(i16::from(*count))
            .fetch_all(&mut *tx)
            .await?
        }
        SlotChange::SetQuality { bag, slot, quality } => {
            sqlx::query_as(concat!(
                "UPDATE character_items SET quality = $4 WHERE character_id = $1 AND bag = $2 AND slot = $3 RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(*bag))
            .bind(i16::from(*slot))
            .bind(i32::from(*quality))
            .fetch_all(&mut *tx)
            .await?
        }
        SlotChange::SetGems { bag, slot, gems } => {
            sqlx::query_as(concat!(
                "UPDATE character_items SET gems = $4 WHERE character_id = $1 AND bag = $2 AND slot = $3 RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(*bag))
            .bind(i16::from(*slot))
            .bind(gems.to_vec())
            .fetch_all(&mut *tx)
            .await?
        }
        SlotChange::SetCraftName { bag, slot, craft_name } => {
            let name: Vec<u8> = craft_name.iter().copied().take_while(|&b| b != 0).collect();
            sqlx::query_as(concat!(
                "UPDATE character_items SET craft_name = $4 WHERE character_id = $1 AND bag = $2 AND slot = $3 RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(*bag))
            .bind(i16::from(*slot))
            .bind(String::from_utf8_lossy(&name).into_owned())
            .fetch_all(&mut *tx)
            .await?
        }
        SlotChange::Remove { bag, slot } => {
            sqlx::query_as(concat!(
                "DELETE FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3 RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(i16::from(*bag))
            .bind(i16::from(*slot))
            .fetch_all(&mut *tx)
            .await?
        }
        SlotChange::Move { from, to } => {
            let ((from_bag, from_slot), (to_bag, to_slot)) = (place(*from), place(*to));
            sqlx::query_as(concat!(
                "UPDATE character_items SET bag = $4, slot = $5 WHERE character_id = $1 AND bag = $2 AND slot = $3
                    AND NOT EXISTS (SELECT 1 FROM character_items WHERE character_id = $1 AND bag = $4 AND slot = $5) RETURNING ",
                stored_columns!()
            ))
            .bind(id)
            .bind(from_bag)
            .bind(from_slot)
            .bind(to_bag)
            .bind(to_slot)
            .fetch_all(&mut *tx)
            .await?
        }
        SlotChange::Swap { a, b } => {
            // One statement; the slot key is checked at its end (migration 0007).
            let ((a_bag, a_slot), (b_bag, b_slot)) = (place(*a), place(*b));
            let swapped: Vec<(i64, i64, i16, i16)> = sqlx::query_as(
                "UPDATE character_items
                    SET bag  = CASE WHEN bag = $2 AND slot = $3 THEN $4 ELSE $2 END,
                        slot = CASE WHEN bag = $2 AND slot = $3 THEN $5 ELSE $3 END
                  WHERE character_id = $1 AND ((bag = $2 AND slot = $3) OR (bag = $4 AND slot = $5))
              RETURNING uid, (extract(epoch FROM made_at) * 1000000)::int8, bag, slot",
            )
            .bind(id)
            .bind(a_bag)
            .bind(a_slot)
            .bind(b_bag)
            .bind(b_slot)
            .fetch_all(&mut *tx)
            .await?;
            let at = |bag: i16, slot: i16| swapped.iter().find(|r| r.2 == bag && r.3 == slot).map(|r| Stored { uid: r.0, made_at: r.1 });
            return Ok((swapped.len() == 2).then(|| Changed { row: at(a_bag, a_slot), swapped: at(b_bag, b_slot) }));
        }
    };
    Ok(one(rows))
}

/// Every column of a market item's row, for moving it between `character_items`, `market_auctions` and `market_result_items`.
macro_rules! market_item_columns {
    () => {
        "item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid"
    };
}

/// A new auction of a bag row: the whole stack with every column, or part of it as a new row (a new uid, made now).
macro_rules! new_auction_sql {
    ($item:expr) => {
        concat!(
            "INSERT INTO market_auctions (server_id, seller_id, seller_name, ends_at, min_money, direct_money, period, deposit, price, ",
            market_item_columns!(),
            ") SELECT $1, $2, $3, to_timestamp($4::float8 / 1000000), $5, $6, $7, $8, $5, ",
            $item,
            " FROM character_items WHERE character_id = $9 AND bag = $10 AND slot = $11 AND count = $12 RETURNING id, ",
            stored_columns!()
        )
    };
}

// Below the macros above, which these modules use (a `macro_rules!` is in scope only after its definition).
mod admin;
mod character;
mod effects;
mod friends;
mod guild;
mod heartbeat;
mod items;
mod market;
mod points;
mod quests;
mod session;
mod skills;

pub use effects::{load_item_effects, load_item_effects_in, load_skill_abilities, load_skill_abilities_in};
use heartbeat::QuestRowSaved;
pub use heartbeat::{
    enchant_table, load_base_items, load_boss_groups, load_box_lines, load_class_base, load_exp, load_item_defs, load_item_enchant,
    load_kill_status, load_live, load_mob_ai, load_mob_defs, load_mob_lua, load_obelisks, load_quest_defs, load_quest_links,
    load_sequence_groups, load_skill_defs, load_status_table, load_world,
};
use items::ItemRow;

/// The enabled extensions whose `server` range excludes this build: `(name, version, range)`
/// (`docs/plans/extensions-split.md` section 3.1). A range the data editor accepted always parses; one that does not is skipped.
pub async fn extensions_for_other_servers(pool: &PgPool) -> Result<Vec<(String, String, String)>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT name, version, manifest->>'server' FROM content.extensions WHERE enabled AND manifest ? 'server' ORDER BY install_seq",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|(name, version, range)| {
            crate::version::parse_range(range).ok().is_some_and(|r| crate::version::refusal(name, version, &r).is_some())
        })
        .collect())
}

/// The item names (`defs.items.itemname`) the auction house's name search reads, by type and type id.
pub async fn load_item_names(pool: &PgPool) -> Result<HashMap<(u8, u8), Vec<u8>>> {
    let rows: Vec<(i16, i16, String)> = sqlx::query_as("SELECT type, typeid, itemname FROM defs.items").fetch_all(pool).await?;
    rows.into_iter().map(|(t, id, name)| Ok(((u8::try_from(t)?, u8::try_from(id)?), name.into_bytes()))).collect()
}

/// The names the game log's texts carry: the items', monsters' and skills' (`docs/original/game-log.md` section 2).
pub async fn load_game_log_names(pool: &PgPool) -> Result<crate::game_log::Names> {
    let items: Vec<(i16, i16, String)> = sqlx::query_as("SELECT type, typeid, itemname FROM defs.items").fetch_all(pool).await?;
    let mobs: Vec<(i16, String)> = sqlx::query_as("SELECT mobid, mobname FROM defs.mobs").fetch_all(pool).await?;
    let skills: Vec<(i16, String)> = sqlx::query_as("SELECT skillid, skillname FROM defs.skills").fetch_all(pool).await?;
    let mut names = crate::game_log::Names::default();
    for (t, id, name) in items {
        if let (Ok(t), Ok(id)) = (u8::try_from(t), u8::try_from(id)) {
            names.items.insert((t, id), name);
        }
    }
    for (id, name) in mobs {
        if let Ok(id) = u16::try_from(id) {
            names.mobs.insert(id, name);
        }
    }
    for (id, name) in skills {
        if let Ok(id) = u16::try_from(id) {
            names.skills.entry(id).or_insert(name);
        }
    }
    Ok(names)
}

/// Writes game log entries (`docs/original/game-log.md` deviation 1): one multi-row `INSERT` per table and 500 rows, stamped with the
/// game server's id.
pub async fn write_game_log(pool: &PgPool, server_id: u8, entries: &[crate::game_log::Entry]) -> Result<()> {
    use crate::game_log::{ChatRow, Entry, Row};
    use std::time::UNIX_EPOCH;
    let seconds = |at: std::time::SystemTime| at.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    let actions: Vec<&Row> = entries.iter().filter_map(|e| if let Entry::Action(row) = e { Some(row) } else { None }).collect();
    let chats: Vec<&ChatRow> = entries.iter().filter_map(|e| if let Entry::Chat(row) = e { Some(row) } else { None }).collect();
    for chunk in actions.chunks(500) {
        let mut query = sqlx::QueryBuilder::new(
            "INSERT INTO game_log (server_id, user_name, user_id, character_id, character_name, level, exp, map, x, y, z, at, action,
                                   value1, value2, value3, value4, value5, value6, value7, value8, value9, value10,
                                   text1, text2, text3, text4) ",
        );
        query.push_values(chunk, |mut b, row| {
            let h = &row.header;
            b.push_bind(i16::from(server_id))
                .push_bind(&h.user_name)
                .push_bind(h.user_id as i32)
                .push_bind(h.character_id as i32)
                .push_bind(&h.character_name)
                .push_bind(h.level as i16)
                .push_bind(i64::from(h.exp))
                .push_bind(h.map as i16)
                .push_bind(h.position[0])
                .push_bind(h.position[1])
                .push_bind(h.position[2]);
            b.push("to_timestamp(").push_bind_unseparated(seconds(row.at)).push_unseparated(")");
            b.push_bind(i16::from(row.action));
            for value in row.values {
                b.push_bind(value);
            }
            for text in &row.texts {
                b.push_bind(text.as_deref());
            }
        });
        query.build().execute(pool).await?;
    }
    for chunk in chats.chunks(500) {
        let mut query =
            sqlx::QueryBuilder::new("INSERT INTO chat_log (server_id, user_id, character_id, chat_type, target_name, text, map, at) ");
        query.push_values(chunk, |mut b, row| {
            b.push_bind(i16::from(server_id))
                .push_bind(row.user_id as i32)
                .push_bind(row.character_id as i32)
                .push_bind(i16::from(row.chat_type))
                .push_bind(row.target.as_deref())
                .push_bind(&row.text)
                .push_bind(row.map as i16);
            b.push("to_timestamp(").push_bind_unseparated(seconds(row.at)).push_unseparated(")");
        });
        query.build().execute(pool).await?;
    }
    Ok(())
}

/// Deletes the game log's and the chat log's rows older than `days` days (deviation 5); the rows deleted.
pub async fn purge_game_log(pool: &PgPool, days: u32) -> Result<u64> {
    let days = i32::try_from(days)?;
    let mut rows =
        sqlx::query("DELETE FROM game_log WHERE at < now() - make_interval(days => $1)").bind(days).execute(pool).await?.rows_affected();
    rows +=
        sqlx::query("DELETE FROM chat_log WHERE at < now() - make_interval(days => $1)").bind(days).execute(pool).await?.rows_affected();
    Ok(rows)
}

/// The item mall's products (`defs.product_list`, `usp_Read_ProductList`): code, price and the 24 entries (a missing one as 0).
pub async fn load_products(pool: &PgPool) -> Result<crate::mall::Products> {
    use sqlx::Row;
    let entries: Vec<String> = (1..=24).map(|i| format!("COALESCE(itemid{i}, 0)::int4, COALESCE(itemcount{i}, 0)::int2")).collect();
    let sql = format!("SELECT productcode, buycost, {} FROM defs.product_list ORDER BY productcode", entries.join(", "));
    // Built from constants only.
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql)).fetch_all(pool).await?;
    let mut products = Vec::new();
    for row in rows {
        let code: String = row.try_get(0)?;
        let price: i32 = row.try_get(1)?;
        let entries = (0..24)
            .map(|i| {
                let item: i32 = row.try_get(2 + 2 * i)?;
                let count: i16 = row.try_get(3 + 2 * i)?;
                Ok((u32::try_from(item)?, u8::try_from(count)?))
            })
            .collect::<Result<Vec<_>>>()?;
        products.push((code, u32::try_from(price)?, entries));
    }
    Ok(crate::mall::Products::new(products))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_login_payloads() {
        assert_eq!(parse_account_login("5 1760784000123456"), Some((5, 1_760_784_000_123_456)));
        assert_eq!(parse_account_login("5"), None);
        assert_eq!(parse_account_login("x 1"), None);
        assert_eq!(parse_account_login("5 1 2"), None);
    }

    #[test]
    fn console_payloads() {
        assert_eq!(parse_console_request("1 0a1b2c3d exp 1.5"), Some((1, "0a1b2c3d", "exp 1.5")));
        assert_eq!(parse_console_request("2  ab  kick char X "), Some((2, "ab", "kick char X")));
        assert_eq!(parse_console_request("1 ab"), None);
        assert_eq!(parse_console_request("x ab info"), None);
        let lines = ["players 1".to_owned(), "exp x1.000".to_owned()];
        assert_eq!(console_reply("ab", &lines), "ab players 1\nexp x1.000");
        let long: Vec<String> = (0..2000).map(|_| "a long answer line".to_owned()).collect();
        let reply = console_reply("ab", &long);
        assert!(reply.len() <= CONSOLE_REPLY_MAX + 4 && reply.ends_with("\n..."));
    }
}
