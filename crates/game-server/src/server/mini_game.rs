//! The gambling house on the connection's side (`docs/original/mini-game.md` sections 2, 3 and 9): sitting down leaves the zone as a
//! move to another map does; at the table the requests of `0x3102` to `0x3109` go to the player's `MiniGame`, every money change is
//! saved at once, and whatever lies on the table goes back to the money when the player stands up, enters the zone again or leaves.
//! The rules are `crate::mini_game`'s.

use rand::Rng;
use shaiya_proto::mini_game as mp;

use super::*;
use crate::game_log::{self, Header};
use crate::mini_game::{MiniGame, Odds, Outcome, Record};
use crate::world::GAMBLING_NPC_TYPE;

fn odds<S>(ctx: &Context<S>) -> Odds {
    Odds { payout_percent: ctx.config.slot_payout_percent, chances: ctx.config.card_win_chances }
}

/// The header of a gambling record: level and experience 0 (section 7), the map and position only where the original sends them
/// (`0x0D01`).
fn header(character: &PlayCharacter, state: &WorldState, with_place: bool) -> Header {
    let h = record::header(character, state);
    Header { level: 0, exp: 0, map: if with_place { h.map } else { 0 }, position: if with_place { h.position } else { [0.0; 3] }, ..h }
}

fn write_records<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, records: Vec<Record>) {
    let h = header(character, state, false);
    for record in records {
        ctx.game_log.action(match record {
            Record::Money { after, change } => game_log::gamble_money(h.clone(), after, change),
            Record::Slot { lines, bet, won, winnings } => game_log::gamble_slot(h.clone(), lines, bet, won, winnings),
            Record::Card { round, won, amount } => game_log::gamble_card(h.clone(), round, won, amount),
            Record::Exit { money } => game_log::gamble_stand(h.clone(), money),
        });
    }
}

/// The money after a request, saved at once as `CUser::SendDBMoney` does (the change is written in one step).
async fn save_money<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState, money: u32) -> Result<()> {
    let change = i64::from(money) - i64::from(state.money);
    if change != 0 && ctx.store.trade(character.id, change, &[]).await?.is_none() {
        bail!("the database refused a gambling money change the server allowed ({change})");
    }
    state.money = money;
    Ok(())
}

/// A request at the table, or `0x3101` in the world (section 2).
pub(super) async fn packet<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    if !ctx.config.mini_game {
        tracing::debug!(id = character.id, opcode = format_args!("{opcode:#06x}"), "gambling house request ignored: GAME_MINI_GAME is off");
        return Ok(());
    }
    if opcode == mp::OP_SIT {
        // Only in the world (`m_nWhere` 4); a refusal answers 0.
        if state.entered {
            sit(conn, ctx, character, state, payload).await?;
        }
        return Ok(());
    }
    // The rest only while out of the zone with a game open (`m_nWhere` 3).
    if state.entered {
        return Ok(());
    }
    let Some(mut game) = state.mini_game.take() else { return Ok(()) };
    let result = at_table(conn, ctx, character, state, &mut game, opcode, payload).await;
    // The game stays unless the player stood up (`at_table` leaves `None` in that case by returning `true`).
    match result {
        Ok(true) => {}
        Ok(false) => state.mini_game = Some(game),
        Err(error) => {
            // Keep the game, so the leave still hands the table back.
            state.mini_game = Some(game);
            return Err(error);
        }
    }
    Ok(())
}

/// `0x3101` (section 3 and decision 3): the table, the checks, then out of the zone.
async fn sit<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    payload: &[u8],
) -> Result<()> {
    let id = character.id;
    let game = mp::parse_sit(payload).and_then(MiniGame::sit);
    let allowed =
        state.mini_game.is_none() && ctx.zones.may_gamble(state.map, id) && ctx.zones.npc_kind_in_view(state.map, id, GAMBLING_NPC_TYPE);
    let Some(game) = game.filter(|_| allowed) else { return conn.send(mp::sat(false)).await };
    ctx.game_log.action(game_log::gamble_sit(header(character, state, true), state.money, game.table(), game.table_bet()));
    stop_attacking(ctx, state, id);
    leave_zone(conn, ctx, state, id).await?;
    tracing::info!(id, table = game.table(), money = state.money, "sat at a gambling table");
    state.mini_game = Some(game);
    conn.send(mp::sat(true)).await
}

/// One request at the table. True when the player stood up (the game is over).
async fn at_table<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    game: &mut MiniGame,
    opcode: u16,
    payload: &[u8],
) -> Result<bool> {
    let mut money = state.money;
    let mut rand = |n: u32| rand::rng().random_range(0..n);
    let outcome = match opcode {
        mp::OP_BET => game.bet_line(&mut money),
        mp::OP_SPIN => game.spin(&mut money, &odds(ctx), &mut rand),
        mp::OP_DOUBLE => game.double_up(&mut rand),
        mp::OP_AGAIN => game.again(&mut money),
        mp::OP_GUESS => match mp::parse_guess(payload) {
            Some(high) => game.guess(high, &mut money, &odds(ctx), &mut rand),
            None => return Ok(false),
        },
        mp::OP_NEXT => game.next_card(&mut rand),
        mp::OP_STOP => game.stop(&mut money),
        mp::OP_STAND => {
            // `0x0213` with the money, then `0x3106`; the player stays out of the zone until `0x0201`.
            let money = stand_up(ctx, character, state, game).await?;
            conn.send(shaiya_proto::world::set_money(money)).await?;
            conn.send(mp::stood(money)).await?;
            return Ok(true);
        }
        _ => return Ok(false),
    };
    let Outcome { answer, records, .. } = outcome;
    save_money(ctx, character, state, money).await?;
    write_records(ctx, character, state, records);
    if let Some(answer) = answer {
        conn.send(answer).await?;
    }
    Ok(false)
}

/// Standing up, or any leave (`CMiniGame::GameExit`, section 3): what lies on the table goes back to the money, saved, with the
/// records. Returns the money.
async fn stand_up<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState, game: &MiniGame) -> Result<u32> {
    let mut money = state.money;
    let stood = game.stand(&mut money);
    save_money(ctx, character, state, money).await?;
    write_records(ctx, character, state, stood.records);
    tracing::info!(id = character.id, money, "stood up from a gambling table");
    Ok(money)
}

/// `0x0201` with a game still open: the player stands up first (`0x0213` with the money; no `0x3106`), then enters as usual.
pub(super) async fn stand_for_entry<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
) -> Result<()> {
    let Some(game) = state.mini_game.take() else { return Ok(()) };
    let money = stand_up(ctx, character, state, &game).await?;
    conn.send(shaiya_proto::world::set_money(money)).await
}

/// Leaving the world (logout, a lost connection, a kick, the shutdown): the table's money goes back before the character is saved.
pub(super) async fn stand_for_leaving<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) {
    let Some(game) = state.mini_game.take() else { return };
    if let Err(error) = stand_up(ctx, character, state, &game).await {
        tracing::warn!(%error, id = character.id, "could not hand a gambling table's money back on leaving");
    }
}
