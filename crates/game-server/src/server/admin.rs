//! Operator (GM) commands on the connections (`docs/original/gm-commands.md`, `docs/plans/done/gm-commands.md` steps 6 to 8). The GM's
//! connection checks and carries out what touches only the GM; what changes another player's character is an order that the
//! target's connection carries out (`carry_out`) and answers the GM for. The rules are `crate::admin`'s.

use crate::sync::Relock;
use shaiya_proto::admin as ap;

use super::*;
use crate::admin::{self, Check, Heard, NoticeKind, Repeat, Stat};
use crate::backend::NewItem;
use crate::inventory::SlotChange as Change;
use crate::pvp;
use crate::user_questions::Taken;
use crate::zone::{AdminOrder, BanKind, CureKind};

/// A random number as the zone passes use them (15 bits).
fn random() -> impl FnMut() -> u32 {
    use rand::Rng;
    let mut rng = rand::rng();
    move || rng.random::<u32>() & 0x7fff
}

fn text(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

/// The line every command leaves in the log, and its record in the game log (`0x0C03`, `CUser::SendLogAdmin`): the staff level, the
/// opcode as the command's name (the original's text is unknown) and the detail, which names the target.
fn log<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, opcode: u16, detail: impl std::fmt::Display) {
    tracing::info!(gm = character.id, name = %character.name, opcode = format_args!("{opcode:#06X}"), %detail, "gm command");
    let detail = detail.to_string();
    ctx.game_log.action(crate::game_log::staff_command(
        record::header(character, state),
        state.staff_level,
        &format!("{opcode:04X}"),
        None,
        Some(detail.as_str()).filter(|d| !d.is_empty()),
    ));
}

/// `F501 0`, or `F501 opcode` when the command failed.
async fn answer(conn: &mut Connection, opcode: u16, done: bool) -> Result<()> {
    conn.send(ap::answer(if done { 0 } else { opcode })).await
}

/// A packet from a GM (`opcode >= 0xF000`): the level check of its family, then the command.
pub(super) async fn packet<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let level = player.privilege.load(Ordering::Relaxed);
    tracing::debug!(gm = character.id, level, opcode = format_args!("{opcode:#06x}"), payload = %hex(payload), "gm packet");
    match admin::check(level, opcode) {
        Check::Close => bail!("a player sent the staff opcode {opcode:#06x}"),
        Check::Refused => {
            tracing::debug!(
                gm = character.id,
                level,
                opcode = format_args!("{opcode:#06x}"),
                "gm command refused: not for this level, or not built"
            );
            return conn.send(ap::answer(opcode)).await;
        }
        Check::Allowed => {}
    }
    let (id, map) = (character.id, state.map);
    match opcode {
        ap::OP_CHAT_NORMAL | ap::OP_CHAT_WHISPER | ap::OP_CHAT_TRADE | ap::OP_CHAT_GUILD | ap::OP_CHAT_PARTY | ap::OP_CHAT_UNION => {
            gm_chat(conn, ctx, player, character, state, opcode, payload).await?
        }

        ap::OP_SET_LEVEL | ap::OP_CLEAR_LEVEL => {
            let (name, new) = if opcode == ap::OP_SET_LEVEL {
                let Some((name, new)) = ap::parse_set_level(payload) else { return Ok(()) };
                (name, new)
            } else {
                let Some(name) = ap::parse_name(payload) else { return Ok(()) };
                (name, 11)
            };
            log(ctx, character, state, opcode, format_args!("target {} level {new}", text(&name)));
            let ordered = admin::may_set_level(new)
                && ctx.zones.find_by_name(&name).is_some_and(|(zone, target, ..)| {
                    ctx.zones.admin_order(zone, target, AdminOrder::StaffLevel { gm: id, op: opcode, level: new })
                });
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }

        ap::OP_SET => {
            let Some(request) = ap::parse_set(payload) else { return Ok(()) };
            log(
                ctx,
                character,
                state,
                opcode,
                format_args!("target {} stat {} value {}", text(&request.name), request.stat, request.value),
            );
            let ordered = Stat::from_u8(request.stat).is_some()
                && ctx.zones.find_by_name(&request.name).is_some_and(|(zone, target, ..)| {
                    ctx.zones.admin_order(zone, target, AdminOrder::Set { gm: id, stat: request.stat, value: request.value })
                });
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }

        ap::OP_MAKE_ITEM => {
            let Some((item_type, type_id, count)) = ap::parse_make_item(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("item {item_type}/{type_id} x{count}"));
            // An unknown item gets no answer at all.
            let Some(def) = ctx.data.items.get(item_type, type_id) else { return Ok(()) };
            let done = make_items(conn, ctx, character, state, def, count).await?;
            answer(conn, opcode, done).await?;
        }

        ap::OP_RECALL_ITEM => {
            let Some((name, item_type, type_id)) = ap::parse_recall_item(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {} item {item_type}/{type_id}", text(&name)));
            let ordered = ctx.data.items.get(item_type, type_id).is_some()
                && ctx.zones.find_by_name(&name).is_some_and(|(zone, target, ..)| {
                    zone == map && ctx.zones.admin_order(zone, target, AdminOrder::RecallItem { gm: id, item_type, type_id })
                });
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }

        ap::OP_MOB_MAKE => {
            let Some((mob, count)) = ap::parse_mob(payload) else { return Ok(()) };
            let at = ctx.zones.position_of(map, id).unwrap_or(state.position);
            log(ctx, character, state, opcode, format_args!("monster {mob} x{count} at {at:?}"));
            let made = ctx.zones.mob_gen(map, mob, at, count, Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut random());
            answer(conn, opcode, made > 0).await?;
        }
        ap::OP_MOB_REMOVE => {
            let Some((mob, count)) = ap::parse_mob(payload) else { return Ok(()) };
            let at = ctx.zones.position_of(map, id).unwrap_or(state.position);
            log(ctx, character, state, opcode, format_args!("monster {mob} x{count} at {at:?}"));
            let known = ctx.data.live().mobs.get(mob).is_some();
            if known {
                ctx.zones.mob_remove_near(map, mob, at, count, Instant::now());
            }
            answer(conn, opcode, known).await?;
        }
        ap::OP_REMOVE_TARGETED => {
            let Some(object) = ap::parse_mob_id(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("monster object {object}"));
            let done = ctx.zones.mob_remove_id(map, object, Instant::now());
            answer(conn, opcode, done).await?;
        }
        ap::OP_NPC_MAKE => {
            let Some((kind, type_id, _)) = ap::parse_npc(payload) else { return Ok(()) };
            let at = ctx.zones.position_of(map, id).unwrap_or(state.position);
            log(ctx, character, state, opcode, format_args!("npc {kind}/{type_id} at {at:?}"));
            let done = ctx.zones.npc_create(map, kind, type_id, at, &ctx.data.world);
            answer(conn, opcode, done).await?;
        }
        ap::OP_NPC_REMOVE => {
            let Some((kind, type_id, count)) = ap::parse_npc(payload) else { return Ok(()) };
            let at = ctx.zones.position_of(map, id).unwrap_or(state.position);
            log(ctx, character, state, opcode, format_args!("npc {kind}/{type_id} x{count} at {at:?}"));
            let known = ctx.data.world.has_npc(kind, type_id);
            if known {
                ctx.zones.npc_remove_near(map, kind, type_id, at, count);
            }
            answer(conn, opcode, known).await?;
        }

        ap::OP_SERVER_ON | ap::OP_SERVER_OFF => {
            // `F706` opens, `F707` closes, each as often as sent, as the original (`docs/original/small-items.md` section 4: the
            // user's client sends `F707` for `/server off` through fixes.dll; the stock client sends `F706` for both words).
            let on = opcode == ap::OP_SERVER_ON;
            console::set_accepting(ctx, on).await;
            log(ctx, character, state, opcode, format_args!("accepting connections: {on}"));
            answer(conn, opcode, true).await?;
        }
        ap::OP_SHUTDOWN => {
            log(ctx, character, state, opcode, "refused: the original answers with the failure only");
            answer(conn, opcode, false).await?;
        }
        ap::OP_ACCOUNT_KICK => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("account {}", text(&name)));
            if let Some((user, status)) = ctx.store.account_id(&text(&name)).await? {
                // A character of the account in the world gets the order (it is told and protected like `/kick`'s target).
                let in_world = ctx.store.characters(user, ctx.config.server_id).await?.iter().find_map(|c| ctx.zones.locate(c.id));
                let order = AdminOrder::Kick { gm: id, gm_level: level, op: opcode };
                if in_world.is_some_and(|m| ctx.zones.admin_order(m.map, m.id, order)) {
                    return Ok(());
                }
                // On the character screen: closed with no packet, unless the account is staff of the GM's level or higher.
                let kicked = (user == player.user_id || admin::may_ban(level, status)) && ctx.online.kick(user, Kick::Operator { op: 0 });
                return answer(conn, opcode, kicked).await;
            }
            answer(conn, opcode, false).await?;
        }
        ap::OP_ACCOUNT_BAN | ap::OP_BAN => ban_account(conn, ctx, player, character, state, opcode, payload).await?,
        ap::OP_ACCOUNT_UNBAN | ap::OP_UNBAN => unban_account(conn, ctx, character, state, opcode, payload).await?,

        ap::OP_QUEST_ON | ap::OP_QUEST_OFF => {
            let Some(quest) = ap::parse_quest(payload) else { return Ok(()) };
            let close = opcode == ap::OP_QUEST_OFF;
            log(ctx, character, state, opcode, format_args!("quest {quest} {}", if close { "closed" } else { "opened" }));
            let closed = ctx.closed_quests.guard().contains(&quest);
            if ctx.data.quests.get(quest).is_none() || closed == close {
                return answer(conn, opcode, false).await;
            }
            ctx.store.set_quest_closed(ctx.config.server_id, quest, close).await?;
            {
                let mut set = ctx.closed_quests.guard();
                if close {
                    set.insert(quest);
                } else {
                    set.remove(&quest);
                }
            }
            answer(conn, opcode, true).await?;
        }
        ap::OP_EVENT_ADD | ap::OP_EVENT_REMOVE => {
            let Some((name, event)) = ap::parse_event(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("character {} event {}", text(&name), text(&event)));
            let done = match ctx.store.character_id(&text(&name)).await? {
                Some(target) if opcode == ap::OP_EVENT_ADD => ctx.store.add_event(target, &text(&event)).await?,
                Some(target) => ctx.store.remove_event(target, &text(&event)).await?,
                None => false,
            };
            answer(conn, opcode, done).await?;
        }

        ap::OP_INVISIBLE | ap::OP_VISIBLE => {
            let visible = opcode == ap::OP_VISIBLE;
            log(ctx, character, state, opcode, format_args!("visible {visible}"));
            state.gm.visible = visible;
            for packet in ctx.zones.set_gm_flags(map, id, Some(visible), None, None) {
                conn.send(packet).await?;
            }
            answer(conn, opcode, true).await?;
        }
        ap::OP_UNTOUCHABLE | ap::OP_TOUCHABLE => {
            // `/attack on` makes the staff member untouchable; the one byte of the original is both flags here (deviation 15).
            let attackable = opcode == ap::OP_TOUCHABLE;
            log(ctx, character, state, opcode, format_args!("attackable {attackable}"));
            (state.gm.attackable, state.gm.pvp) = (attackable, attackable);
            ctx.zones.set_gm_flags(map, id, None, Some(attackable), Some(attackable));
            answer(conn, opcode, true).await?;
        }
        ap::OP_PVP_ON | ap::OP_PVP_OFF => {
            let pvp = opcode == ap::OP_PVP_ON;
            log(ctx, character, state, opcode, format_args!("players may attack: {pvp}"));
            state.gm.pvp = pvp;
            ctx.zones.set_gm_flags(map, id, None, None, Some(pvp));
            answer(conn, opcode, true).await?;
        }

        ap::OP_WARNING => {
            let Some((name, message)) = ap::parse_named_text(payload) else { return answer(conn, opcode, false).await };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let found = ctx.zones.find_by_name(&name);
            if let Some((zone, target, ..)) = found {
                ctx.zones.send_to(zone, target, &ap::warning(message));
            }
            answer(conn, opcode, found.is_some()).await?;
        }
        ap::OP_WATCH => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let found =
                ctx.zones.find_by_name(&name).and_then(|(zone, target, ..)| ctx.zones.position_of(zone, target).map(|at| (zone, at)));
            // The answer comes first, then the position.
            answer(conn, opcode, found.is_some()).await?;
            if let Some((zone, at)) = found {
                conn.send(ap::watch(crate::world::map_of(zone), at)).await?;
            }
        }

        // `/cwatch <name>`: the GM listens to that player's lines from now on, its earlier link dropped (section 8.4).
        ap::OP_LISTEN => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let found = ctx.zones.find_by_name(&name);
            if let Some((_, target, ..)) = found {
                ctx.zones.listen(id, target);
                tracing::info!(gm = id, target, "gm listens to a player's chat");
            }
            answer(conn, opcode, found.is_some()).await?;
        }
        // `/cwatch off` (the client's DLL sends `FA0C`).
        ap::OP_LISTEN_OFF => {
            log(ctx, character, state, opcode, "listen off");
            answer(conn, opcode, ctx.zones.listen_off(id)).await?;
        }

        ap::OP_MOVE_TO_PLAYER | ap::OP_MOVE_TO_SPOT | ap::OP_MOVE_BIND | ap::OP_MOVE_GUILD => {
            gm_move(conn, ctx, player, character, state, opcode, payload).await?
        }

        ap::OP_SILENCE | ap::OP_STOP | ap::OP_SILENCE_OFF | ap::OP_STOP_OFF => {
            let (minutes, name) = if matches!(opcode, ap::OP_SILENCE | ap::OP_STOP) {
                let Some((minutes, name)) = ap::parse_ban(payload) else { return Ok(()) };
                (minutes, name)
            } else {
                let Some(name) = ap::parse_name(payload) else { return Ok(()) };
                (0, name)
            };
            log(ctx, character, state, opcode, format_args!("target {} minutes {minutes}", text(&name)));
            let until = matches!(opcode, ap::OP_SILENCE | ap::OP_STOP).then(|| admin::ban_until(minutes, Instant::now()));
            let ordered = ctx
                .zones
                .find_by_name(&name)
                .is_some_and(|(zone, target, ..)| ctx.zones.admin_order(zone, target, AdminOrder::Ban { gm: id, op: opcode, until }));
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }

        ap::OP_GUILD_GRADE => {
            let Some((guild, member, grade)) = ap::parse_guild_grade(payload) else { return answer(conn, opcode, false).await };
            log(ctx, character, state, opcode, format_args!("guild {} member {} grade {grade}", text(&guild), text(&member)));
            answer(conn, opcode, ctx.zones.admin_guild_grade(&guild, &member, grade)).await?;
        }
        ap::OP_GUILD_RANK => {
            let Some((guild, ranking)) = ap::parse_guild_rank(payload) else { return answer(conn, opcode, false).await };
            log(ctx, character, state, opcode, format_args!("guild {} ranking {ranking}", text(&guild)));
            answer(conn, opcode, ctx.zones.admin_guild_ranking(&guild, ranking)).await?;
        }
        ap::OP_GUILD_POINTS => {
            let Some((guild, points)) = ap::parse_guild_points(payload) else { return answer(conn, opcode, false).await };
            log(ctx, character, state, opcode, format_args!("guild {} points {points}", text(&guild)));
            answer(conn, opcode, ctx.zones.admin_guild_points(&guild, points)).await?;
        }

        ap::OP_CALL => {
            let Some(country) = ap::parse_call(payload) else { return Ok(()) };
            let at = ctx.zones.position_of(map, id).unwrap_or(state.position);
            log(ctx, character, state, opcode, format_args!("country {country}"));
            let players: Vec<_> = ctx
                .zones
                .players_of(map)
                .into_iter()
                .map(|(player, staff, country, position)| (player, if staff { 1 } else { 11 }, country, position))
                .collect();
            let calls = admin::call_targets((id, at), country, &players, random());
            let now = Instant::now();
            for (target, delay) in calls {
                ctx.zones.admin_order(map, target, AdminOrder::Called { position: at, zone: map, due: now + delay });
            }
            // No answer, as the original.
        }

        // The auction house (`docs/original/auction.md` section 12): on map 42 only.
        ap::OP_AUCTION_SEARCH => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("seller {}", text(&name)));
            if crate::world::map_of(map) != crate::market::AUCTION_MAP {
                return answer(conn, opcode, false).await;
            }
            let page = market::seller_search(ctx, state, name);
            conn.send(page).await?;
            answer(conn, opcode, true).await?;
        }
        ap::OP_AUCTION_RECALL => {
            let Some((name, market_id)) = ap::parse_auction_recall(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("seller {} auction {market_id}", text(&name)));
            if crate::world::map_of(map) != crate::market::AUCTION_MAP || name == character.name.as_bytes() {
                return answer(conn, opcode, false).await;
            }
            // An online seller is disconnected first (`SConnection::Close(9)`: no packet).
            if ctx.zones.find_by_name(&name).is_some()
                && let Some((user, _)) = ctx.store.character_account(&text(&name)).await?
                && ctx.online.kick(user, Kick::Operator { op: 0 })
            {
                tracing::info!(gm = id, seller = %text(&name), "auction recall: the seller is disconnected");
            }
            match market::recall(ctx, id, &name, market_id).await? {
                Some(packets) => {
                    for packet in packets {
                        conn.send(packet).await?;
                    }
                    answer(conn, opcode, true).await?;
                }
                None => answer(conn, opcode, false).await?,
            }
        }

        ap::OP_UQ_GET => uq_get(conn, ctx, character, state).await?,
        ap::OP_UQ_COUNT => {
            log(ctx, character, state, opcode, "");
            let waiting = ctx.questions.guard().waiting_count();
            conn.send(ap::uq_count(waiting.min(usize::from(u16::MAX)) as u16)).await?;
        }
        ap::OP_UQ_ANSWER | ap::OP_UQ_ANSWER_TO => uq_answer(ctx, character, state, opcode, payload).await?,

        ap::OP_CURE | ap::OP_HEAL | ap::OP_CURE_ALL => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let kind = match opcode {
                ap::OP_CURE => CureKind::Debuffs,
                ap::OP_HEAL => CureKind::Heal,
                _ => CureKind::All,
            };
            // Deviation 5: a name nobody has is refused, not turned into the GM itself.
            let ordered = ctx
                .zones
                .find_by_name(&name)
                .filter(|&(zone, ..)| zone == map)
                .is_some_and(|(zone, target, ..)| ctx.zones.admin_order(zone, target, AdminOrder::Cure { gm: id, op: opcode, kind }));
            if !ordered {
                answer(conn, opcode, false).await?;
                if kind != CureKind::Debuffs {
                    conn.send(ap::gm_cmd_whisper(&admin::cure_line(opcode, false, &name))).await?;
                }
            }
        }
        ap::OP_CLEAR_BAGS | ap::OP_CLEAR_EQUIPMENT => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let ordered = ctx.zones.find_by_name(&name).filter(|&(zone, ..)| zone == map).is_some_and(|(zone, target, ..)| {
                ctx.zones.admin_order(zone, target, AdminOrder::Clear { gm: id, gm_level: level, op: opcode })
            });
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }
        ap::OP_KICK => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let ordered = ctx.zones.find_by_name(&name).is_some_and(|(zone, target, ..)| {
                ctx.zones.admin_order(zone, target, AdminOrder::Kick { gm: id, gm_level: level, op: opcode })
            });
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }
        ap::OP_QUEST_ADD | ap::OP_QUEST_REMOVE => {
            let Some((name, quest)) = ap::parse_name_quest(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {} quest {quest}", text(&name)));
            let order = if opcode == ap::OP_QUEST_ADD {
                AdminOrder::QuestAdd { gm: id, gm_level: level, quest }
            } else {
                AdminOrder::QuestRemove { gm: id, gm_level: level, quest }
            };
            let ordered = ctx.data.quests.get(quest).is_some()
                && ctx
                    .zones
                    .find_by_name(&name)
                    .filter(|&(zone, ..)| zone == map)
                    .is_some_and(|(zone, target, ..)| ctx.zones.admin_order(zone, target, order));
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }

        ap::OP_QUIRY => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            let ordered = ctx
                .zones
                .find_by_name(&name)
                .is_some_and(|(zone, target, ..)| ctx.zones.admin_order(zone, target, AdminOrder::Quiry { gm: id }));
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }
        ap::OP_SUMMON => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            // Deviation 11: a map with a create type is refused with an answer.
            let at = ctx.zones.position_of(map, id).unwrap_or(state.position);
            let order = AdminOrder::Summon { gm: Some(id), op: opcode, zone: map, position: at };
            let ordered = state.entered
                && admin::amove_allowed(ctx.data.world.create_type(map))
                && ctx.zones.find_by_name(&name).is_some_and(|(zone, target, ..)| ctx.zones.admin_order(zone, target, order));
            if !ordered {
                answer(conn, opcode, false).await?;
            }
        }
        ap::OP_SUMMON_TO | ap::OP_SUMMON_PARTY => {
            let Some((name, x, z, to_map)) = ap::parse_summon_to(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {} map {to_map} at {x}, {z}", text(&name)));
            let world = &ctx.data.world;
            let usable = to_map != 200 && world.is_safe_pos(to_map, [x, 0.0, z]) && admin::amove_allowed(world.create_type(to_map));
            let target = ctx.zones.find_by_name(&name).filter(|_| usable);
            let Some((zone, target, ..)) = target else { return answer(conn, opcode, false).await };
            let order = |gm| AdminOrder::Summon { gm, op: opcode, zone: to_map, position: [x, 0.0, z] };
            if opcode == ap::OP_SUMMON_TO {
                if !ctx.zones.admin_order(zone, target, order(Some(id))) {
                    answer(conn, opcode, false).await?;
                }
            } else {
                // The target alone, or every member of its party in the world; the GM is answered once.
                let members = ctx
                    .zones
                    .party_members_placed(target)
                    .map_or_else(|| vec![(zone, target)], |m| m.iter().map(|m| (m.map, m.id)).collect());
                let ordered = members.into_iter().filter(|&(map, member)| ctx.zones.admin_order(map, member, order(None))).count();
                answer(conn, opcode, ordered > 0).await?;
            }
        }

        ap::OP_NOTICE_STAFF => {
            let Some(message) = ap::parse_notice(payload) else { return answer(conn, opcode, false).await };
            log(ctx, character, state, opcode, format_args!("{} bytes", message.len()));
            ctx.zones.send_staff(&ap::notice(opcode, message));
            answer(conn, opcode, true).await?;
        }
        ap::OP_NOTICE_VIEW | ap::OP_NOTICE_TO | ap::OP_NOTICE_ZONE | ap::OP_NOTICE_COUNTRY | ap::OP_NOTICE_ALL => {
            notice_command(conn, ctx, character, state, opcode, payload).await?
        }

        _ => unreachable!("check() lets only the built opcodes through: {opcode:#06x}"),
    }
    Ok(())
}

/// `F201` `/UQGET`: the oldest question, or the one already held (deviation 3 of part 2).
async fn uq_get<S: Backend>(conn: &mut Connection, ctx: &Context<S>, character: &PlayCharacter, state: &WorldState) -> Result<()> {
    log(ctx, character, state, ap::OP_UQ_GET, "");
    let taken = ctx.questions.guard().take(character.id);
    match taken {
        Taken::New(q, n) => {
            ctx.store.take_question(q.id, character.id).await?;
            conn.send(ap::uq_question(n, q.id, q.time, &q.name, &q.text)).await
        }
        Taken::Again(q, n) => conn.send(ap::uq_question(n, q.id, q.time, &q.name, &q.text)).await,
        Taken::Empty => conn.send(ap::uq_none()).await,
    }
}

/// `F202` (kind 1 to 3) and `F203` (the fourth button, with a name): the answer is saved, and `F203`'s text reaches the named player as
/// whisper lines (deviation 2). Nothing is answered to the GM.
async fn uq_answer<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, opcode: u16, payload: &[u8]) -> Result<()> {
    let (kind, question, answer, to) = if opcode == ap::OP_UQ_ANSWER {
        let Some((kind, question, answer)) = ap::parse_uq_answer(payload) else { return Ok(()) };
        // The kinds are 1 to 3 (the table keeps 4 for `F203`).
        (kind.clamp(1, 3), question, answer, None)
    } else {
        let Some((question, name, answer)) = ap::parse_uq_answer_to(payload) else { return Ok(()) };
        (4, question, answer, Some(name))
    };
    log(ctx, character, state, opcode, format_args!("question {question} kind {kind}"));
    let answered = ctx.questions.guard().answer(character.id, question);
    let Some(answered) = answered else {
        tracing::debug!(gm = character.id, question, "answer for a question nobody holds");
        return Ok(());
    };
    let found = to.as_deref().and_then(|name| ctx.zones.find_by_name(name));
    let to_name = to.as_deref().map(|name| found.as_ref().map_or_else(|| text(name), |f| text(&f.3)).chars().take(20).collect::<String>());
    ctx.store.answer_question(answered.id, character.id, kind, &answer, to_name.as_deref()).await?;
    if let Some((zone, target, ..)) = found {
        for line in admin::answer_lines(&answer) {
            ctx.zones.send_to(zone, target, &chat::whisper_op(ap::OP_CHAT_WHISPER, false, character.name.as_bytes(), &line));
        }
    }
    Ok(())
}

/// A staff member leaving the world: the question it holds goes back to the front of the queue (`CUser::LeaveWorld`).
pub(super) async fn leave_questions<S: Backend>(ctx: &Context<S>, gm: u32) {
    let given_back = ctx.questions.guard().give_back(gm);
    if let Some(question) = given_back
        && let Err(error) = ctx.store.release_question(question).await
    {
        tracing::warn!(%error, question, "could not release the question");
    }
}

/// `0x1201` `/request GM`: at most three open questions per character (deviation 4), written to the database first.
pub(super) async fn user_question<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, payload: &[u8]) -> Result<()> {
    let Some(message) = ap::parse_user_question(payload) else { return Ok(()) };
    if !ctx.questions.guard().may_ask(character.id) {
        tracing::debug!(id = character.id, "question ignored: three are open already");
        return Ok(());
    }
    let id = ctx.store.add_question(ctx.config.server_id, character.id, &character.name, &message).await?;
    let question = crate::user_questions::Question {
        id,
        character_id: character.id,
        name: character.name.clone().into_bytes(),
        text: message,
        time: packed_now(),
    };
    ctx.questions.guard().ask(question);
    tracing::info!(id = character.id, name = %character.name, question = id, "user question");
    Ok(())
}

/// The packed local time now.
fn packed_now() -> u32 {
    admin::packed_from_unix(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64))
}

/// The load of a restart, in the packet's packed form.
pub(super) fn packed_from_unix(unix: i64) -> u32 {
    admin::packed_from_unix(unix)
}

/// `F907` to `F90B`: the notice is sent now and, with a repeat, again every so many seconds while the GM is in the world.
async fn notice_command<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let id = character.id;
    let kind = NoticeKind::of_opcode(opcode).expect("only the notice opcodes get here");
    let (name, country, notice) = match opcode {
        ap::OP_NOTICE_TO => {
            let Some((name, notice)) = ap::parse_notice_to(payload) else { return Ok(()) };
            (Some(name), 0, notice)
        }
        ap::OP_NOTICE_COUNTRY => {
            let Some((country, notice)) = ap::parse_notice_country(payload) else { return Ok(()) };
            (None, country, notice)
        }
        _ => {
            let Some(notice) = ap::parse_repeat_notice(payload) else { return Ok(()) };
            (None, 0, notice)
        }
    };
    log(ctx, character, state, opcode, format_args!("repeat {} seconds, {} bytes", notice.repeat, notice.len));
    // The "off" form stops the repeat of its own kind (deviation 14 for `F908`).
    if notice.len < 2 {
        state.notices.stop(kind);
        return answer(conn, opcode, true).await;
    }
    if usize::from(notice.len) > chat::MAX_TEXT {
        return answer(conn, opcode, false).await;
    }
    let target = match (kind, name) {
        (NoticeKind::To, Some(name)) => match ctx.zones.find_by_name(&name) {
            Some((_, target, ..)) => target,
            None => return answer(conn, opcode, false).await,
        },
        // Deviation 15: a faction is 0 or 1.
        (NoticeKind::Country, _) if country > 1 => return answer(conn, opcode, false).await,
        (NoticeKind::Country, _) => u32::from(country),
        _ => 0,
    };
    let mut message = notice.text.to_vec();
    if let Some(last) = message.last_mut() {
        *last = 0;
    }
    if notice.repeat == 0 {
        state.notices.stop(kind);
    } else {
        let period = Duration::from_secs(u64::from(notice.repeat));
        state.notices.set(kind, Repeat { due: Instant::now() + period, period, text: message.clone(), target });
    }
    send_notice(conn, ctx, state, id, kind, target, &message).await?;
    answer(conn, opcode, true).await
}

/// A notice to the players its kind reaches.
async fn send_notice<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    state: &WorldState,
    id: u32,
    kind: NoticeKind,
    target: u32,
    message: &[u8],
) -> Result<()> {
    let packet = ap::notice(kind.opcode(), message);
    match kind {
        // Those around the GM, and the GM itself.
        NoticeKind::View => conn.send(ctx.zones.send_near(state.map, id, packet, NORMAL_CHAT_RANGE)).await?,
        NoticeKind::To => ctx.zones.send_to_char(target, &packet),
        NoticeKind::Zone => ctx.zones.send_zone(state.map, &packet),
        NoticeKind::Country => ctx.zones.send_country(target as u8, None, &packet),
        NoticeKind::All => ctx.zones.send_world(&packet),
    }
    Ok(())
}

/// The repeats that are due: sent again from where the GM is now; a whisper notice stops when its player has gone (section 5.4).
pub(super) async fn notices_due<S: Backend>(conn: &mut Connection, ctx: &Context<S>, state: &mut WorldState, id: u32) -> Result<()> {
    for (kind, repeat) in state.notices.due(Instant::now()) {
        if kind == NoticeKind::To && ctx.zones.locate(repeat.target).is_none() {
            state.notices.stop(kind);
            continue;
        }
        send_notice(conn, ctx, state, id, kind, repeat.target, &repeat.text).await?;
    }
    Ok(())
}

/// The name the console's answers whisper from.
const SERVER_NAME: &[u8] = b"Server";

/// The longest piece of an answer line in one whisper (the chat line holds 127 bytes and the zero).
const WHISPER_CUT: usize = 120;

/// `text` cut into pieces of at most `max` bytes, at character boundaries.
fn cut(text: &str, max: usize) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while rest.len() > max {
        let mut at = max;
        while !rest.is_char_boundary(at) {
            at -= 1;
        }
        pieces.push(&rest[..at]);
        rest = &rest[at..];
    }
    if !rest.is_empty() {
        pieces.push(rest);
    }
    pieces
}

/// `F101` to `F105`, `F10A`: the GM's ordinary chat (section 5).
async fn gm_chat<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let country = player.faction.map_or(0, |f| f.country);
    let name = character.name.as_bytes();
    let (target, message) = if opcode == ap::OP_CHAT_WHISPER {
        let Some((target, message)) = chat::parse_whisper(payload) else { bail!("malformed GM whisper") };
        (Some(target), message)
    } else {
        (None, chat::parse_message(payload))
    };
    let message = match message {
        Text::Ok(message) => message,
        Text::Empty => return Ok(()),
        Text::Invalid => bail!("GM chat text over 128 bytes or cut short"),
    };
    // A level-1 GM's line starting with a dot is a console command (`docs/original/operator-console.md` section 6 step 5): no chat.
    if opcode == ap::OP_CHAT_NORMAL
        && (1..=ctx.config.console_level).contains(&state.staff_level)
        && let Some(text) = crate::console::typed_command(message)
    {
        let answer = console::line(ctx, &text, &character.name, record::header(character, state), state.staff_level).await;
        for line in answer.iter().flat_map(|line| cut(line, WHISPER_CUT)) {
            let mut text = line.as_bytes().to_vec();
            text.push(0);
            conn.send(chat::whisper(false, SERVER_NAME, &text)).await?;
        }
        return Ok(());
    }
    // A chat ban drops the line, as for the players' opcodes; so does markup that would crash the clients.
    if ctx.zones.bans_of(id).chat.is_some_and(|until| !until.passed(Instant::now())) || !chat::markup_is_safe(message) {
        return Ok(());
    }
    match opcode {
        // With a Message To Server armed: to everyone as `0x1108` instead (`CUser::PacketAdminChat`).
        ap::OP_CHAT_NORMAL if state.chat_item_armed => {
            super::said(ctx, character, state, Heard::Trade, None, message);
            super::all_server_chat(ctx, state, id, name, message)
        }
        ap::OP_CHAT_NORMAL => {
            super::said(ctx, character, state, Heard::Normal, None, message);
            conn.send(ctx.zones.send_near(map, id, chat::by_id(opcode, id, message), NORMAL_CHAT_RANGE)).await?
        }
        ap::OP_CHAT_WHISPER => {
            // Same country only (`FindUserZone` and the country check).
            match ctx.zones.find_by_name(&target.expect("parsed above")) {
                Some((zone, target, target_country, target_name)) if target_country == country => {
                    ctx.zones.send_to(zone, target, &chat::whisper_op(opcode, false, name, message));
                    conn.send(chat::whisper_op(opcode, true, &target_name, message)).await?;
                    super::said(ctx, character, state, Heard::Whisper, Some(&target_name), message);
                }
                _ => conn.send(chat::whisper_failed()).await?,
            }
        }
        ap::OP_CHAT_TRADE => {
            ctx.zones.send_world(&chat::by_name(opcode, name, message));
            super::said(ctx, character, state, Heard::All, None, message);
        }
        // Logged and copied only when the GM is in a guild, a party, at the head of a union, as the original.
        ap::OP_CHAT_GUILD => {
            if ctx.zones.guild_chat(id, &chat::by_name(opcode, name, message)) {
                super::said(ctx, character, state, Heard::Guild, None, message);
            }
        }
        ap::OP_CHAT_PARTY => {
            if ctx.zones.party_chat(id, &chat::by_id(opcode, id, message)) {
                super::said(ctx, character, state, Heard::Party, None, message);
            }
        }
        _ => {
            if ctx.zones.union_chat(id, &chat::by_id(opcode, id, message)) {
                super::said(ctx, character, state, Heard::Party, None, message);
            }
        }
    }
    Ok(())
}

/// `F702`: `count` of the item into the GM's own bags, `min(left, stack)` to each empty slot in order, all saved in one write
/// (section 7.2). True when all of it fitted.
async fn make_items<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    def: &crate::inventory::ItemDef,
    count: u8,
) -> Result<bool> {
    let id = character.id;
    let stack = def.max_count.max(1);
    let mut left = count;
    let mut changes = Vec::new();
    {
        let mut rand = random();
        for (bag, slot) in state.inventory.free_slots() {
            if left == 0 {
                break;
            }
            let here = left.min(stack);
            let kept = Kept { make_type: b'S', craft_name: crate::lapis::new_craft_name(def, &mut rand), ..Kept::default() };
            changes.push(Change::Insert(NewItem {
                bag,
                slot,
                item_id: def.item_id,
                item_type: def.item_type,
                type_id: def.type_id,
                quality: def.quality,
                count: here,
                kept: Some(kept),
            }));
            left -= here;
        }
    }
    if count == 0 || changes.is_empty() {
        return Ok(false);
    }
    let Some(rows) = ctx.store.trade(id, 0, &changes).await? else {
        bail!("the database refused items the server could place: {changes:?}");
    };
    for change in &changes {
        state.inventory.apply(change);
    }
    record::created(ctx, character, state, &changes, &rows);
    send_item_changes(conn, state, &changes, true).await?;
    Ok(left == 0)
}

/// `FA09`, `FA0A`, `FA11`, `FA12`: the GM moves at once (deviation 6) through the ordinary move.
async fn gm_move<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let (id, world) = (character.id, &ctx.data.world);
    let country = player.faction.map_or(0, |f| f.country);
    // Not while a logout or a move by item is pending.
    if state.logout.is_some() || state.item_move.is_some() {
        log(ctx, character, state, opcode, "refused: a logout or a move is pending");
        return answer(conn, opcode, false).await;
    }
    // A map the world has, other than the other server's 200 (deviation 7).
    let usable = |map: u16, x: f32, z: f32| map != 200 && world.is_safe_pos(map, [x, 0.0, z]);
    let destination: Option<(u16, [f32; 3])> = match opcode {
        ap::OP_MOVE_TO_PLAYER => {
            let Some(name) = ap::parse_name(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
            ctx.zones
                .find_by_name(&name)
                .filter(|&(zone, ..)| admin::amove_allowed(world.create_type(zone)))
                .and_then(|(zone, target, ..)| ctx.zones.position_of(zone, target).map(|at| (zone, at)))
        }
        ap::OP_MOVE_TO_SPOT => {
            let Some((x, z, map)) = ap::parse_go(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("map {map} at {x}, {z}"));
            (usable(map, x, z) && world.create_type(map) == 0).then_some((map, [x, 0.0, z]))
        }
        ap::OP_MOVE_BIND => {
            let Some(map) = ap::parse_map(payload) else { return Ok(()) };
            let map = if map == 0xFFFF { state.map } else { map };
            log(ctx, character, state, opcode, format_args!("map {map}"));
            if usable(map, 0.0, 0.0) && world.create_type(map) == 0 {
                let position = ctx.zones.position_of(state.map, id).unwrap_or(state.position);
                Some(world.bind_pos(map, country, position, &mut random()))
            } else {
                None
            }
        }
        _ => {
            let Some(go) = ap::parse_guild_go(payload) else { return Ok(()) };
            log(ctx, character, state, opcode, format_args!("map {} at {}, {} for {}", go.map, go.x, go.z, text(&go.name)));
            let create_type = world.create_type(go.map);
            if usable(go.map, go.x, go.z) && admin::gm_zone_allowed(create_type) {
                let owner = match create_type {
                    3 => ctx
                        .zones
                        .find_by_name(&go.name)
                        .and_then(|(_, target, ..)| ctx.zones.party_id_of(target))
                        .map(crate::instance::Owner::Party),
                    5 => ctx
                        .zones
                        .guild_id_by_name(&go.name)
                        .filter(|_| ctx.zones.ranking_open().is_some())
                        .map(crate::instance::Owner::Guild),
                    _ => ctx
                        .zones
                        .guild_id_by_name(&go.name)
                        .filter(|&g| ctx.zones.guild_house_enabled(g))
                        .map(crate::instance::Owner::Guild),
                };
                owner.and_then(|owner| ctx.zones.gm_zone(go.map, owner, world, &ctx.data.live().mobs)).map(|zone| (zone, [go.x, 0.0, go.z]))
            } else {
                None
            }
        }
    };
    let Some((zone, position)) = destination else { return answer(conn, opcode, false).await };
    move_player(conn, ctx, state, id, zone, position, None).await?;
    answer(conn, opcode, true).await
}

/// `F70A` `/aban` (an account) and `F805` `/ban` (a character's account), deviation 14.
async fn ban_account<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &PlayCharacter,
    state: &WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let Some((minutes, name)) = ap::parse_ban(payload) else { return Ok(()) };
    log(ctx, character, state, opcode, format_args!("target {} minutes {minutes}", text(&name)));
    let found = if opcode == ap::OP_ACCOUNT_BAN {
        ctx.store.account_id(&text(&name)).await?
    } else {
        ctx.store.character_account(&text(&name)).await?
    };
    let level = player.privilege.load(Ordering::Relaxed);
    let Some((user, status)) = found.filter(|&(user, status)| user != player.user_id && admin::may_ban(level, status)) else {
        return answer(conn, opcode, false).await;
    };
    ctx.store.block_account(user, (minutes > 0).then_some(minutes)).await?;
    // The banned player's connection gets the notice, then ends.
    ctx.online.kick(user, Kick::Banned { op: opcode, minutes });
    tracing::info!(user, status, minutes, "account blocked by a GM");
    answer(conn, opcode, true).await
}

/// `F70B` `/abancan` and `F806` `/bancan`.
async fn unban_account<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let Some(name) = ap::parse_name(payload) else { return Ok(()) };
    log(ctx, character, state, opcode, format_args!("target {}", text(&name)));
    let found = if opcode == ap::OP_ACCOUNT_UNBAN {
        ctx.store.account_id(&text(&name)).await?
    } else {
        ctx.store.character_account(&text(&name)).await?
    };
    let done = match found {
        Some((user, _)) => ctx.store.lift_block(user).await?,
        None => false,
    };
    answer(conn, opcode, done).await
}

/// What `/set` reads of the target.
fn sheet_of(character: &PlayCharacter, state: &WorldState) -> admin::Sheet {
    admin::Sheet {
        job: character.job,
        grow: character.grow,
        level: character.level,
        exp: character.exp,
        money: state.money,
        stat_point: character.stat_point,
        skill_point: character.skill_point,
        stats: character.stats,
        honor: character.honor,
        vg: character.vg,
        cg: character.cg,
        og: character.og,
        ig: character.ig,
        kills: character.kills[0],
        deaths: character.kills[1],
    }
}

/// The speeds again to the view after a move ban started or ended.
async fn resend_speeds<S: Backend>(conn: &mut Connection, ctx: &Context<S>, state: &mut WorldState, id: u32) -> Result<()> {
    if state.speeds() != state.sent_speeds {
        state.sent_speeds = state.speeds();
        let (attack, movement) = state.sent_speeds;
        conn.send(ctx.zones.change_speeds(state.map, id, attack, movement)).await?;
    }
    Ok(())
}

/// One order from a GM, on the connection of the character it changes. The GM is answered through the zone.
pub(super) async fn carry_out<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    order: AdminOrder,
) -> Result<()> {
    let id = character.id;
    match order {
        AdminOrder::Set { gm, stat, value } => set(conn, ctx, character, state, gm, stat, value).await,
        AdminOrder::StaffLevel { gm, op, level } => {
            player.privilege.store(level, Ordering::Relaxed);
            // The connection's own copies follow, so the console and the staff packets go with the level held now (R26).
            let was_staff = state.staff;
            state.staff_level = level;
            state.staff = level <= crate::enter::STAFF_PRIVILEGE_MAX;
            ctx.zones.set_staff(state.map, id, state.staff);
            if was_staff && !state.staff {
                // No longer staff: a question held goes back to the queue now, since leaving the world only does it for staff.
                leave_questions(ctx, id).await;
            }
            // Too low a level to listen: the copies would stop at the next line (`PacketChatE`), the link goes now.
            if level > admin::LISTEN_LEVEL_MAX {
                ctx.zones.listen_off(id);
            }
            // A player again: the staff switches (`/char`, `/attack`) go back to a player's, so nothing is left on that it cannot turn off.
            if level > crate::enter::STAFF_PRIVILEGE_MAX {
                state.gm = admin::Flags::on_entering(false);
                for packet in ctx.zones.set_gm_flags(state.map, id, Some(true), Some(true), Some(true)) {
                    conn.send(packet).await?;
                }
            }
            ctx.store.set_account_status(player.user_id, admin::status_of(level)).await?;
            conn.send(ap::level_answer(op, level)).await?;
            ctx.zones.send_to_char(gm, &ap::answer(0));
            tracing::info!(id, level, "staff level changed by a GM");
            Ok(())
        }
        AdminOrder::RecallItem { gm, item_type, type_id } => {
            // Bags 1 and up (deviation 5), none while a logout is pending.
            if state.logout.is_some() {
                ctx.zones.send_to_char(gm, &ap::answer(ap::OP_RECALL_ITEM));
                return Ok(());
            }
            let changes: Vec<Change> = state
                .inventory
                .items()
                .filter(|i| i.item_type == item_type && i.type_id == type_id && i.bag != 0 && state.inventory.is_bag((i.bag, i.slot)))
                .map(|i| Change::Remove { bag: i.bag, slot: i.slot })
                .collect();
            if !changes.is_empty() {
                let Some(rows) = ctx.store.trade(id, 0, &changes).await? else {
                    bail!("the database refused removing {changes:?}");
                };
                record::removed(ctx, character, state, &changes, &rows);
                for change in &changes {
                    state.inventory.apply(change);
                }
                send_item_changes(conn, state, &changes, false).await?;
            }
            ctx.zones.send_to_char(gm, &ap::answer(0));
            tracing::info!(id, item_type, type_id, removed = changes.len(), "items recalled by a GM");
            Ok(())
        }
        AdminOrder::Ban { gm, op, until } => {
            match (op, until) {
                (ap::OP_SILENCE, Some(until)) => ctx.zones.set_ban(id, BanKind::Chat, until),
                (ap::OP_STOP, Some(until)) => {
                    ctx.zones.set_ban(id, BanKind::Move, until);
                    state.move_banned = true;
                }
                (ap::OP_SILENCE_OFF, _) => ctx.zones.end_ban(id, BanKind::Chat),
                _ => {
                    ctx.zones.end_ban(id, BanKind::Move);
                    state.move_banned = false;
                }
            }
            resend_speeds(conn, ctx, state, id).await?;
            conn.send(ap::bare(op)).await?;
            ctx.zones.send_to_char(gm, &ap::answer(0));
            Ok(())
        }
        AdminOrder::Called { position, zone, due } => {
            state.called = Some((due, zone, position));
            Ok(())
        }
        AdminOrder::Cure { gm, op, kind } => cure(conn, ctx, character, state, gm, op, kind).await,
        AdminOrder::Clear { gm, gm_level, op } => {
            if !admin::may_touch(gm_level, player.privilege.load(Ordering::Relaxed), gm == id) || state.logout.is_some() {
                ctx.zones.send_to_char(gm, &ap::answer(op));
                return Ok(());
            }
            clear(conn, ctx, character, state, gm, op).await
        }
        AdminOrder::Kick { gm, gm_level, op } => {
            if !admin::may_touch(gm_level, player.privilege.load(Ordering::Relaxed), gm == id) {
                ctx.zones.send_to_char(gm, &ap::answer(op));
                return Ok(());
            }
            ctx.zones.send_to_char(gm, &ap::answer(0));
            // The connection sends `op` and ends, as a ban does.
            ctx.online.kick(player.user_id, Kick::Operator { op });
            tracing::info!(id, op = format_args!("{op:#06X}"), "kicked by a GM");
            Ok(())
        }
        AdminOrder::QuestAdd { gm, gm_level, quest } => {
            if !admin::may_touch(gm_level, player.privilege.load(Ordering::Relaxed), gm == id)
                || !quest_added(conn, ctx, character, state, quest).await?
            {
                ctx.zones.send_to_char(gm, &ap::answer(ap::OP_QUEST_ADD));
                return Ok(());
            }
            ctx.zones.send_to_char(gm, &ap::answer(0));
            Ok(())
        }
        AdminOrder::QuestRemove { gm, gm_level, quest } => {
            if !admin::may_touch(gm_level, player.privilege.load(Ordering::Relaxed), gm == id)
                || !quest_removed(conn, ctx, character, state, quest).await?
            {
                ctx.zones.send_to_char(gm, &ap::answer(ap::OP_QUEST_REMOVE));
                return Ok(());
            }
            ctx.zones.send_to_char(gm, &ap::answer(0));
            Ok(())
        }
        AdminOrder::Quiry { gm } => {
            ctx.zones.send_to_char(gm, &ap::answer(0));
            ctx.zones.send_to_char(gm, &ap::sheet(&quiry_sheet(character, state)));
            Ok(())
        }
        AdminOrder::Summon { gm, op, zone, position } => {
            // A dead player, or one with a logout, a move or a map change under way, stays (deviation 13 and 12).
            if state.dead || state.logout.is_some() || state.item_move.is_some() || !state.entered {
                if let Some(gm) = gm {
                    ctx.zones.send_to_char(gm, &ap::answer(op));
                }
                return Ok(());
            }
            move_player(conn, ctx, state, id, zone, position, Some(op)).await?;
            if let Some(gm) = gm {
                ctx.zones.send_to_char(gm, &ap::answer(0));
            }
            tracing::info!(id, op = format_args!("{op:#06X}"), zone, ?position, "summoned by a GM");
            Ok(())
        }
    }
}

/// `F801`, `F80D`, `F80E` on the character they cure (section 4.1 and 4.5 of part 2).
async fn cure<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    gm: u32,
    op: u16,
    kind: CureKind,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let line = |ok| ap::gm_cmd_whisper(&admin::cure_line(op, ok, character.name.as_bytes()));
    if state.dead {
        ctx.zones.send_to_char(gm, &ap::answer(op));
        if kind != CureKind::Debuffs {
            ctx.zones.send_to_char(gm, &line(false));
        }
        return Ok(());
    }
    // The cooldowns end with nothing sent about it (the client keeps its own timers).
    if kind != CureKind::Heal {
        state.cooldowns.clear();
        for packet in ctx.zones.clear_applied(map, id, kind == CureKind::All) {
            conn.send(packet).await?;
        }
    }
    if let Some(packet) = ctx.zones.fill(map, id, state.totals.max_health()) {
        conn.send(packet).await?;
    }
    ctx.zones.send_to_char(gm, &ap::answer(0));
    if kind != CureKind::Debuffs {
        ctx.zones.send_to_char(gm, &line(true));
    }
    tracing::info!(id, op = format_args!("{op:#06X}"), "cured by a GM");
    Ok(())
}

/// `F802` (the bags 1 and up) and `F803` (the equipment) emptied: no packet per item, the client empties itself (section 4.2).
async fn clear<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    gm: u32,
    op: u16,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    ctx.zones.cancel_actions(id);
    if op == ap::OP_CLEAR_BAGS {
        let changes: Vec<Change> = state.inventory.usable_items().map(|i| Change::Remove { bag: i.bag, slot: i.slot }).collect();
        if !changes.is_empty() {
            let Some(rows) = ctx.store.trade(id, 0, &changes).await? else {
                bail!("the database refused emptying the bags: {changes:?}");
            };
            record::removed(ctx, character, state, &changes, &rows);
            for change in &changes {
                state.inventory.apply(change);
            }
        }
        conn.send(ap::bare(op)).await?;
    } else {
        let slots: Vec<u8> = state.inventory.items().filter(|i| i.bag == 0).map(|i| i.slot).collect();
        let removals: Vec<Change> = slots.iter().map(|&slot| Change::Remove { bag: 0, slot }).collect();
        if !removals.is_empty() {
            // The worn qualities are saved before the pieces go, as an item move does.
            let mut saved = take_unsaved_wear(state);
            saved.extend(removals.iter().cloned());
            let Some(rows) = ctx.store.trade(id, 0, &saved).await? else {
                bail!("the database refused emptying the equipment: {saved:?}");
            };
            record::removed(ctx, character, state, &removals, &rows[saved.len() - removals.len()..]);
            for change in &removals {
                state.inventory.apply(change);
            }
            renumber(conn, ctx, character, state).await?;
            if slots.iter().any(|&slot| vehicles::touches_mount([(0, slot); 2])) {
                vehicles::mount_slot_changed(conn, ctx, state, id).await?;
            }
            for &slot in slots.iter().filter(|&&slot| slot < VISIBLE_SLOTS) {
                conn.send(ctx.zones.change_equipment(map, id, slot, None)).await?;
            }
        }
        conn.send(ap::bare(op)).await?;
    }
    ctx.zones.send_to_char(gm, &ap::answer(0));
    tracing::info!(id, op = format_args!("{op:#06X}"), "emptied by a GM");
    Ok(())
}

/// `F80B`: the quest starts as an ordinary start would, with none of its conditions (deviation 9). False when it is in progress already
/// or its start items do not fit.
async fn quest_added<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    quest: u16,
) -> Result<bool> {
    let id = character.id;
    let Some(def) = ctx.data.quests.get(quest) else { return Ok(false) };
    if state.quests.find(quest).is_some() {
        return Ok(false);
    }
    let Ok(plan) = crate::quests::plan_admin_start(def, &state.inventory, &ctx.data.items) else { return Ok(false) };
    let seconds = (def.time_limit > 0).then(|| def.time_limit.saturating_mul(60));
    let Some(rows) = ctx.store.start_quest(id, quest, seconds, &plan.changes).await? else {
        tracing::warn!(id, quest, changes = ?plan.changes, "the database refused a quest start the server allowed");
        return Ok(false);
    };
    for change in &plan.changes {
        state.inventory.apply(change);
    }
    record::created(ctx, character, state, &plan.changes, &rows);
    state.quests.forget_finished(quest);
    state.quests.take(def, Instant::now());
    super::sync_quests(ctx, id, &state.quests);
    send_item_changes(conn, state, &plan.changes, false).await?;
    conn.send(qp::started(0, quest)).await?;
    send_item_changes(conn, state, &plan.changes, true).await?;
    tracing::info!(id, quest, name = %def.name, "quest given by a GM");
    Ok(true)
}

/// `F80C`: the quest in progress is dropped (its row deleted, so it can be taken again) and its start items leave the bags
/// (deviation 8). False when it is not in progress.
async fn quest_removed<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    quest: u16,
) -> Result<bool> {
    let id = character.id;
    let Some(def) = ctx.data.quests.get(quest) else { return Ok(false) };
    if state.quests.find(quest).is_none() {
        return Ok(false);
    }
    let changes: Vec<Change> = state
        .inventory
        .usable_items()
        .filter(|i| {
            def.start_items.iter().any(|&(item_type, type_id, _)| item_type != 0 && i.item_type == item_type && i.type_id == type_id)
        })
        .map(|i| Change::Remove { bag: i.bag, slot: i.slot })
        .collect();
    let end = QuestEnd { quest_id: quest, success: false, keep: false, money: 0, changes };
    if ctx.store.end_quest(id, &end).await?.is_none() {
        tracing::warn!(id, quest, "the database refused dropping a quest");
        return Ok(false);
    }
    for change in &end.changes {
        state.inventory.apply(change);
    }
    state.quests.drop(quest);
    super::sync_quests(ctx, id, &state.quests);
    // `0x0502` with success 0 (`CUser::QuestRemoveAdmin`).
    ctx.game_log.action(crate::game_log::quest_end(record::header(character, state), quest, false, 0, None, &def.name));
    conn.send(qp::ended(&qp::Ended { npc_id: 0, quest_id: quest, ..Default::default() })).await?;
    send_item_changes(conn, state, &end.changes, false).await?;
    tracing::info!(id, quest, name = %def.name, "quest taken away by a GM");
    Ok(true)
}

/// `F901`'s sheet (section 5.1): the totals in the order STR, DEX, REC, INT, LUC, WIS and bag 0's items.
fn quiry_sheet(character: &PlayCharacter, state: &WorldState) -> ap::QuirySheet {
    let [str, dex, rec, int, wis, luc] = state.totals.stats.map(|v| v.clamp(0, i32::from(u16::MAX)) as u16);
    let items = state
        .inventory
        .items()
        .filter(|i| i.bag == 0)
        .map(|i| {
            let mut craft_name = [0u8; 21];
            let len = i.craft_name.len().min(20);
            craft_name[..len].copy_from_slice(&i.craft_name[..len]);
            ap::QuiryItem { slot: i.slot, item_type: i.item_type, type_id: i.type_id, quality: i.quality, gems: i.gems, craft_name }
        })
        .collect();
    ap::QuirySheet {
        mode: character.grow,
        level: character.level,
        exp: character.exp,
        money: state.money,
        stat_point: character.stat_point,
        skill_point: character.skill_point,
        stats: [str, dex, rec, int, luc, wis],
        hg: character.honor,
        vg: character.vg.max(0) as u16,
        cg: character.cg,
        og: character.og,
        ig: character.ig,
        items,
    }
}

/// `F701` on the character it changes (section 7.1, deviations 3 and 4): the new values, saved, the packets that follow from them, then
/// `F701` to the target and `F501 0` to the GM.
async fn set<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    gm: u32,
    stat: u8,
    value: u32,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let refused = |ctx: &Context<S>| ctx.zones.send_to_char(gm, &ap::answer(ap::OP_SET));
    let Some(kind) = Stat::from_u8(stat) else {
        refused(ctx);
        return Ok(());
    };
    // A dead character is not set: the refill that follows a level or stat change would bring it back.
    if state.dead {
        refused(ctx);
        return Ok(());
    }
    let Some(done) = admin::apply_set(&sheet_of(character, state), kind, value, &ctx.data.exp, pvp::MAX_COUNT) else {
        refused(ctx);
        return Ok(());
    };
    let s = done.sheet;
    let money_change = i64::from(s.money) - i64::from(state.money);
    (character.grow, character.level, character.exp, character.stat_point, character.skill_point, character.stats) =
        (s.grow, s.level, s.exp, s.stat_point, s.skill_point, s.stats);
    (character.honor, character.vg, character.cg, character.og, character.ig) = (s.honor, s.vg, s.cg, s.og, s.ig);
    (character.kills[0], character.kills[1]) = (s.kills, s.deaths);
    // The database first.
    match kind {
        Stat::Money => {
            if ctx.store.trade(id, money_change, &[]).await?.is_none() {
                bail!("the database refused the money of a /set");
            }
            state.money = s.money;
            character.money = s.money;
        }
        Stat::Level
        | Stat::Exp
        | Stat::StatPoint
        | Stat::SkillPoint
        | Stat::Str
        | Stat::Dex
        | Stat::Rec
        | Stat::Int
        | Stat::Luc
        | Stat::Wis => {
            let progress = Progress { level: s.level, exp: s.exp, stat_point: s.stat_point, skill_point: s.skill_point, stats: s.stats };
            let health = ctx.zones.health(map, id).unwrap_or(state.health);
            ctx.store.save_progress(id, &progress, health).await?;
        }
        Stat::Honor => ctx.store.save_honor(id, s.honor).await?,
        Stat::Kills | Stat::Deaths => ctx.store.save_kills(id, character.kills).await?,
        Stat::Grow | Stat::Vg | Stat::Cg | Stat::Og | Stat::Ig => {
            ctx.store.save_marks(id, crate::backend::Marks { grow: s.grow, vg: s.vg, cg: s.cg, og: s.og, ig: s.ig }).await?
        }
    }
    // Then what the client and the others are told. A level's start exp comes first.
    let mut extras = done.extras.iter();
    if kind == Stat::Level
        && let Some(&(extra, extra_value)) = extras.next()
    {
        conn.send(ap::set_answer(extra, extra_value)).await?;
    }
    if done.announce {
        // `0x0308` for a level a GM set (`CUser::LevelUp`).
        ctx.game_log.action(crate::game_log::level_up(record::header(character, state)));
        announce_level(conn, ctx, character, state, true).await?;
    } else if kind.base_index().is_some() {
        // A stat changes the maxima (deviation 3): the new numbers to the view, then a refill.
        renumber(conn, ctx, character, state).await?;
        if let Some(packet) = ctx.zones.fill(map, id, state.totals.max_health()) {
            conn.send(packet).await?;
        }
    }
    if kind == Stat::Kills {
        conn.send(ctx.zones.set_kills(map, id, s.kills)).await?;
    }
    for &(extra, extra_value) in extras {
        conn.send(ap::set_answer(extra, extra_value)).await?;
    }
    conn.send(ap::set_answer(stat, done.value)).await?;
    ctx.zones.send_to_char(gm, &ap::answer(0));
    tracing::info!(id, stat, value = done.value, "set by a GM");
    Ok(())
}

/// The moment the next ban of the character ends.
pub(super) fn next_ban_end<S>(ctx: &Context<S>, id: u32) -> Option<Instant> {
    let bans = ctx.zones.bans_of(id);
    [bans.chat, bans.moves]
        .into_iter()
        .flatten()
        .filter_map(|until| match until {
            admin::Until::At(end) => Some(end),
            admin::Until::Forever => None,
        })
        .min()
}

/// Bans that have run out end by themselves (section 4): the player is told (`FA0E`, `FA10`), a move ban gives the speed back.
pub(super) async fn bans_ended<S: Backend>(conn: &mut Connection, ctx: &Context<S>, state: &mut WorldState, id: u32) -> Result<()> {
    let (bans, now) = (ctx.zones.bans_of(id), Instant::now());
    if bans.chat.is_some_and(|u| u.passed(now)) {
        ctx.zones.end_ban(id, BanKind::Chat);
        conn.send(ap::bare(ap::OP_SILENCE_OFF)).await?;
    }
    if bans.moves.is_some_and(|u| u.passed(now)) {
        ctx.zones.end_ban(id, BanKind::Move);
        state.move_banned = false;
        resend_speeds(conn, ctx, state, id).await?;
        conn.send(ap::bare(ap::OP_STOP_OFF)).await?;
    }
    Ok(())
}

/// A `/xcall` move is due: to the GM's spot, unless a logout is pending.
pub(super) async fn called_move<S: Backend>(conn: &mut Connection, ctx: &Context<S>, state: &mut WorldState, id: u32) -> Result<()> {
    let Some((_, zone, position)) = state.called.take() else { return Ok(()) };
    if state.logout.is_some() {
        return Ok(());
    }
    move_player(conn, ctx, state, id, zone, position, None).await
}
