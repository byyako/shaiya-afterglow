//! Drives the game server's handshake, character screen and world entry with a client written from
//! docs/original/ps_game.md and docs/original/enter-world.md, against the in-memory backend.

use game_server::sync::Relock;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use game_server::combat;
use game_server::config::Config;
use game_server::inventory::{ItemDef, ItemDefs};
use game_server::memory::{MemoryBackend, StoredSession};
use game_server::mobs::{MobDef, MobDefs};
use game_server::server::{Context, GameData, Live, heartbeat, serve, shut_down};
use game_server::skills::SkillDef;
use shaiya_proto::admin as ap;
use shaiya_proto::chat;
use shaiya_proto::frame;
use shaiya_proto::game::{self as g, CreateRequest, rename_result};
use shaiya_proto::ground as gp;
use shaiya_proto::item as it;
use shaiya_proto::shop::{self, BuyRequest, ShopResult};
use shaiya_proto::skill as sk;
use shaiya_proto::trade as tp;
use shaiya_proto::world as w;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

mod common;
use common::*;

/// The merchant beside the human start, as the zone entry shows it.
fn start_merchant() -> Vec<u8> {
    w::npc_enter(&w::NpcEnter { object_id: 1, kind: 1, type_id: 1, position: [590.0, 78.0, 1770.0], dir: 0 })
}

/// The rows of one action in the context's memory log (`docs/original/game-log.md`), in the order they were made.
fn logged<S>(ctx: &Context<S>, action: u8) -> Vec<game_server::game_log::Row> {
    use game_server::game_log::Entry;
    ctx.game_log
        .recorded()
        .into_iter()
        .filter_map(|e| if let Entry::Action(row) = e { Some(row).filter(|r| r.action == action) } else { None })
        .collect()
}

/// A row's values V1 to V10 that are set, as `(n, value)`.
fn set_values(row: &game_server::game_log::Row) -> Vec<(usize, i64)> {
    row.values.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i + 1, v))).collect()
}

#[tokio::test]
async fn handshake_opens_an_empty_character_screen() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let (country, max_grow, list) = client.enter(7, [5; 16]).await;
    assert_eq!((country, max_grow), (2, 3), "no faction yet, ultimate allowed");
    for (slot, record) in list.iter().enumerate() {
        assert_eq!(record, &[slot as u8, 0, 0, 0, 0], "empty slot {slot}");
    }
    assert!(backend.session(7).unwrap().in_game);

    // The heartbeat reports the player while connected, and the session ends when the client leaves.
    tokio::spawn(heartbeat(ctx.clone()));
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert_eq!(*backend.last_heartbeat.guard(), Some((SERVER_ID, 1)));
    client.write(shaiya_proto::encode(g::OP_CHAR_LEAVE, &[])).await;
    assert!(client.read().await.is_none(), "0x0107 closes the connection");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(backend.session(7).is_none(), "session ended");
    assert_eq!(ctx.players.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn refused_handshakes_get_no_reply() {
    let backend = Arc::new(MemoryBackend::default());
    backend.add_session(1, session([1; 16], Some(SERVER_ID), LOCAL));
    backend.add_session(2, session([2; 16], Some(SERVER_ID + 1), LOCAL));
    backend.add_session(3, session([3; 16], None, LOCAL));
    backend.add_session(4, session([4; 16], Some(SERVER_ID), IpAddr::V4(Ipv4Addr::new(10, 9, 8, 7))));
    let (addr, _) = start(backend.clone(), true).await;

    for (user, identity, why) in [
        (1, [9; 16], "wrong identity"),
        (2, [2; 16], "bound to another server"),
        (3, [3; 16], "no server chosen"),
        (4, [4; 16], "logged in from another address"),
        (99, [1; 16], "no session"),
    ] {
        let mut client = Client::connect(addr).await;
        client.write(g::game_connect_request(user, &identity)).await;
        assert!(client.read().await.is_none(), "{why}: closed without a reply");
    }

    // A session is used once.
    let mut first = Client::connect(addr).await;
    first.enter(1, [1; 16]).await;
    let mut second = Client::connect(addr).await;
    second.write(g::game_connect_request(1, &[1; 16])).await;
    assert!(second.read().await.is_none(), "second connection with the same session");

    // Malformed handshake: closed.
    let mut short = Client::connect(addr).await;
    short.write(shaiya_proto::encode(g::OP_GAME_CONNECT, &[0; 19])).await;
    assert!(short.read().await.is_none());
}

#[tokio::test]
async fn address_check_can_be_switched_off() {
    let backend = Arc::new(MemoryBackend::default());
    backend.add_session(4, session([4; 16], Some(SERVER_ID), IpAddr::V4(Ipv4Addr::new(10, 9, 8, 7))));
    let (addr, _) = start(backend, false).await;
    let mut client = Client::connect(addr).await;
    client.enter(4, [4; 16]).await;
}

#[tokio::test]
async fn packets_before_the_handshake_are_tolerated_up_to_ten() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend, true).await;
    let mut client = Client::connect(addr).await;
    client.write(shaiya_proto::encode(g::OP_PING, &[])).await;
    for _ in 0..10 {
        client.write(shaiya_proto::encode(0x0501, &[1, 2, 3])).await;
    }
    client.enter(7, [5; 16]).await;

    let mut noisy = Client::connect(addr).await;
    for _ in 0..11 {
        noisy.write(shaiya_proto::encode(0x0501, &[])).await;
    }
    assert!(noisy.read().await.is_none(), "the 11th undefined packet closes the connection");
}

#[tokio::test]
async fn faction_create_delete_and_restore() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;

    // Creating before choosing a faction is refused.
    client.write(create(0, 0, 0, b"Hero")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(1));

    // Choose the light faction (no reply), then create: success, then the whole list again without the faction packet.
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(2, 0, 0, b"Hero")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    let list = client.read_list().await;
    assert_eq!(list[..4].iter().map(|r| r[0]).collect::<Vec<_>>(), [0, 1, 3, 4], "empty slots first");
    let hero = &list[4];
    assert_eq!(hero.len(), 81);
    assert_eq!(hero[0], 2, "slot");
    let hero_id = u32::from_le_bytes(hero[1..5].try_into().unwrap());
    assert_eq!(hero[11..18], [0, 1, 3, 4, 5, 0, 1], "family grow hair face size job sex");
    assert_eq!(&hero[54..58], b"Hero");
    assert_eq!(hero[73..75], [0, 0], "not pending deletion, no rename");

    // Same name again (any case) is refused; so is a taken slot, a race/class mismatch and a bad name.
    for packet in [create(3, 0, 0, b"hero"), create(2, 0, 1, b"Other"), create(3, 0, 2, b"Ranger"), create(3, 1, 2, b"x")] {
        client.write(packet).await;
        assert_eq!(client.read().await.unwrap(), g::char_create_response(1));
    }

    // The faction cannot change while the account has a character.
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[1])).await;

    // Delete request: confirmed with u8 0 + id; restore likewise. Unknown ids get no answer.
    client.write(g::char_id_request(g::OP_CHAR_DELETE, hero_id)).await;
    assert_eq!(client.read().await.unwrap(), g::char_success(g::OP_CHAR_DELETE, hero_id));
    client.write(g::char_id_request(g::OP_CHAR_DELETE, 999)).await;
    client.write(g::char_id_request(g::OP_CHAR_RESTORE, hero_id)).await;
    assert_eq!(client.read().await.unwrap(), g::char_success(g::OP_CHAR_RESTORE, hero_id));

    // Unknown 0x01xx opcodes close the connection, as in the original (there is no name check packet).
    client.write(shaiya_proto::encode(0x0119, b"Name\0")).await;
    assert!(client.read().await.is_none());

    // Reconnect with a new session: the light faction and the character are listed.
    backend.add_session(7, session([6; 16], Some(SERVER_ID), LOCAL));
    let mut again = Client::connect(addr).await;
    let (country, _, list) = again.enter(7, [6; 16]).await;
    assert_eq!(country, 0, "faction unchanged");
    assert_eq!(&list[4][54..58], b"Hero");

    // A due deletion removes the last character and clears the faction, as the original did.
    again.write(g::char_id_request(g::OP_CHAR_DELETE, hero_id)).await;
    assert_eq!(again.read().await.unwrap(), g::char_success(g::OP_CHAR_DELETE, hero_id));
    // The account may not connect twice: this connection goes first.
    again.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert!(again.read().await.is_none());
    backend.delete_due.store(true, Ordering::Relaxed);
    backend.add_session(7, session([8; 16], Some(SERVER_ID), LOCAL));
    let mut third = Client::connect(addr).await;
    let (country, _, list) = third.enter(7, [8; 16]).await;
    assert_eq!(country, 2, "faction cleared with the last character");
    assert!(list.iter().all(|r| r.len() == 5), "all slots empty");
}

#[tokio::test]
async fn select_enters_the_world() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Hero")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    let list = client.read_list().await;
    let hero = &list[4];
    let hero_id = u32::from_le_bytes(hero[1..5].try_into().unwrap());
    // The equipped sword shows in the list: equipment type and type id of slot 5.
    assert_eq!((hero[38 + 5], hero[46 + 5]), (5, 1), "weapon in the list");

    client.write(g::char_id_request(g::OP_CHAR_SELECT, hero_id)).await;
    let mut opcodes = Vec::new();
    let mut packets = Vec::new();
    while opcodes.last() != Some(&w::OP_CHAR_QUICK) {
        let packet = client.read().await.expect("burst");
        opcodes.push(frame::opcode(&packet));
        packets.push(packet);
    }
    // The guild list (empty here) always comes after 0x0404 (`CUser::EnterWorld`).
    assert_eq!(opcodes, [0x0104, 0x0404, 0x0D0D, 0x0D0E, 0x0105, 0x0106, 0x0108, 0x051C, 0x0211, 0x0230, 0x2601, 0x0114, 0x021B, 0x010B]);
    assert_eq!(packets[0], g::char_success(g::OP_CHAR_SELECT, hero_id));
    let data = frame::payload(&packets[4]);
    assert_eq!(data[16..20], 100u32.to_le_bytes(), "next exp for level 1 in normal mode");
    assert_eq!(data[28..32], 577.98f32.to_le_bytes(), "human start x");
    let items = frame::payload(&packets[5]);
    assert_eq!(items[0], 2, "sword and potions");
    assert_eq!(items[1..5], [0, 5, 5, 1], "bag 0 slot 5: type 5 id 1");
    assert_eq!(items[35..39], [1, 0, 25, 1], "bag 1 slot 0: potions");
    assert_eq!(items[35 + 12], 10, "ten potions");
    assert_eq!(items[5..7], 1200u16.to_le_bytes(), "quality from the item definition");

    // After 0x010B the server's packets are XORed (the client already switched). Client packets stay AES.
    client.write(shaiya_proto::encode(g::OP_PING, &[])).await;
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    let (enters, npcs) = read_zone_entry(&mut client).await;
    assert_eq!(enters.len(), 1, "alone in the zone");
    let gatekeeper = w::npc_enter(&w::NpcEnter { object_id: 3, kind: 2, type_id: 1, position: [600.0, 78.0, 1775.0], dir: 0 });
    let keeper = w::npc_enter(&w::NpcEnter { object_id: 6, kind: 6, type_id: 1, position: [595.0, 78.0, 1768.0], dir: 0 });
    let smith = w::npc_enter(&w::NpcEnter { object_id: 7, kind: 3, type_id: 1, position: [592.0, 78.0, 1772.0], dir: 0 });
    assert_eq!(
        npcs,
        [start_merchant(), keeper, smith, gatekeeper],
        "the merchant, warehouse keeper, blacksmith and gatekeeper in view (cell by cell), not the far ones"
    );
    let body = frame::payload(&enters[0]);
    assert_eq!(body.len(), 27);
    assert_eq!(body[..5], [hero_id as u8, (hero_id >> 8) as u8, 0, 0, 1], "id and player kind");

    // A second 0x0201 is not answered; leaving ends the session.
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    client.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert!(client.read().await.is_none());
}

#[tokio::test]
async fn moving_sitting_and_leaving_to_the_character_screen() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;

    // Accepted steps come back as 0x0501 (the original sends a move to everyone in range, the mover included);
    // a step over 35 is refused and the client is put back.
    client.write(step(580.0, 1765.0)).await;
    let echo = client.read().await.unwrap();
    assert_eq!(echo, w::user_move(id, &w::Move { motion: 0, dir: 90, position: [580.0, 78.56, 1765.0] }));
    client.write(step(640.0, 1765.0)).await;
    let back = client.read().await.unwrap();
    assert_eq!(back, w::position(id, 1, [580.0, 78.56, 1765.0]), "0x020B with the last accepted position");
    assert_eq!(client.read().await.unwrap(), w::move_refused(id));

    // A logout request needs the character to sit; the server answers every motion request.
    client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 1));
    // Standing up (here by moving) cancels a pending logout.
    client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    client.write(step(581.0, 1766.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE);
    tokio::time::sleep(Duration::from_millis(250)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 1), "no logout happened in between");

    // Sitting and asking: after the delay, 0x0107 (XOR), then back on AES with the faction and the list.
    client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    assert_eq!(client.read().await.unwrap(), w::logout_done(w::OP_LOGOUT_TO_SELECT));
    let country = client.read().await.unwrap();
    assert_eq!(country, g::char_country(0, 3));
    let list = client.read_list().await;
    assert_eq!(&list[4][54..60], b"Walker", "empty slots first, then the character");

    // The position was saved, and the same connection can enter again.
    use game_server::backend::CharacterStore;
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!((saved.position, saved.dir), ([581.0, 78.56, 1766.0], 90));
    select_and_enter(&mut client).await;

    // Quit: 0x010D after the delay; the client then closes.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    client.read().await.unwrap();
    client.write(w::logout_done(w::OP_LOGOUT_QUIT)).await;
    assert_eq!(client.read().await.unwrap(), w::logout_done(w::OP_LOGOUT_QUIT));
}

#[tokio::test]
async fn walking_too_fast_goes_back_to_the_windows_start() {
    let backend = backend_with_session([5; 16]);
    // Quiet for longer than the window without the connection timing out.
    let (addr, _) = start_with(backend.clone(), Config { idle_timeout: Duration::from_secs(30), ..config(true) }).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;

    // 270 units in steps of 30 at once: each step passes the step check, and the window stays open for 5 s.
    let (mut x, z) = (580.0, 1765.0);
    client.write(step(x, z)).await;
    read_op(&mut client, w::OP_MOVE).await;
    for _ in 0..9 {
        x += 30.0;
        client.write(step(x, z)).await;
        read_op(&mut client, w::OP_MOVE).await;
    }
    // The first step after the window's end closes it: far more than 5 × 5 + 50 walked, refused, back to where it started.
    tokio::time::sleep(Duration::from_millis(5100)).await;
    client.write(step(x + 30.0, z)).await;
    let back = read_op(&mut client, w::OP_POSITION).await;
    let p = frame::payload(&back);
    let at = |i: usize| f32::from_le_bytes(p[i..i + 4].try_into().unwrap());
    let start = [at(6), at(10), at(14)];
    assert!((start[0] - 580.0).abs() <= 35.0 && (start[2] - 1765.0).abs() <= 35.0, "the entry position, {start:?}");
    assert_eq!(read_op(&mut client, w::OP_MOVE_REFUSED).await, w::move_refused(id));
    // The server put the player there too (deviation 1): a short step from it passes the 35-unit check.
    client.write(step(start[0] + 10.0, start[2])).await;
    assert_eq!(
        read_op(&mut client, w::OP_MOVE).await,
        w::user_move(id, &w::Move { motion: 0, dir: 90, position: [start[0] + 10.0, 78.56, start[2]] })
    );
}

#[tokio::test]
async fn learning_skills_and_saving_quick_slots() {
    use game_server::backend::CharacterStore;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let walker = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!(walker.skill_point, 5);

    // The passive at level 1: REC +10 is 50 HP more, sent before the answer; 3 points left.
    let base = game_server::stats::Base { job: 0, level: 1, grow: 1, family: 0, stats: walker.stats, ..Default::default() };
    let max_hp = |skills: &[game_server::skills::Skill]| {
        let passives = game_server::skills::passives(skills, &ctx.data.skills);
        game_server::stats::compute(&ctx.data.status, &base, &walker.items, &ctx.data.items, &passives).max_health()[0]
    };
    client.write(sk::learn_request(1, 1)).await;
    let learnt = game_server::skills::Skill { id: 1, level: 1, number: 0, delay: 0 };
    assert_eq!(max_hp(&[learnt]), max_hp(&[]) + 50);
    assert_eq!(client.read().await.unwrap(), w::set_max(id, w::MAX_HP, max_hp(&[learnt])));
    assert_eq!(client.read().await.unwrap(), sk::learn_result(0, 0, 1, 1));
    // Level 2 needs level 5; a new skill only at level 1; a level skipped; no definition.
    for (skill, level, result) in [(1, 2, 2), (2, 2, 3), (1, 3, 3), (9, 1, 3)] {
        client.write(sk::learn_request(skill, level)).await;
        assert_eq!(client.read().await.unwrap(), sk::learn_result(result, 0, skill, level), "skill {skill} level {level}");
    }
    // An active skill takes number 1 and sends nothing else; the bag passive changes no number the client is told.
    client.write(sk::learn_request(2, 1)).await;
    assert_eq!(client.read().await.unwrap(), sk::learn_result(0, 1, 2, 1));
    client.write(sk::learn_request(3, 1)).await;
    assert_eq!(client.read().await.unwrap(), sk::learn_result(0, 2, 3, 1));
    client.write(sk::learn_request(4, 1)).await;
    assert_eq!(client.read().await.unwrap(), sk::learn_result(1, 0, 4, 1), "no points left");

    let slots = [sk::QuickSlot { bar: 0, slot: 0, bag: 100, number: 1 }, sk::QuickSlot { bar: 1, slot: 3, bag: 1, number: 2 }];
    client.write(sk::char_quick(&slots)).await;
    // Back to the character screen and in again: the burst carries both.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    client.read().await.unwrap();
    client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    assert_eq!(client.read().await.unwrap(), w::logout_done(w::OP_LOGOUT_TO_SELECT));
    client.read().await.unwrap();
    client.read_list().await;
    client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
    let mut burst = Vec::new();
    loop {
        let packet = client.read().await.expect("burst");
        let last = frame::opcode(&packet) == w::OP_CHAR_QUICK;
        burst.push(packet);
        if last {
            break;
        }
    }
    let entry = |skill_id, number| sk::SkillEntry { skill_id, level: 1, number, delay: 0 };
    assert!(burst.contains(&sk::char_skills(&[entry(1, 0), entry(2, 1), entry(3, 2)])));
    assert_eq!(burst.last().unwrap(), &sk::char_quick(&slots));
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!((saved.skill_point, saved.skills.len(), saved.quick_slots.len()), (0, 3, 2));
}

#[tokio::test]
async fn spending_stat_points() {
    use game_server::backend::CharacterStore;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    let walker = backend.load(7, SERVER_ID, 1).await.unwrap().unwrap();
    let progress =
        game_server::combat::Progress { level: 1, exp: walker.exp, stat_point: 6, skill_point: walker.skill_point, stats: walker.stats };
    backend.save_progress(walker.id, &progress, walker.health).await.unwrap();
    let id = select_and_enter(&mut client).await;

    let maxima = |stats| {
        let base = game_server::stats::Base { job: 0, level: 1, grow: 1, family: 0, stats, ..Default::default() };
        let passives = game_server::skills::Passives::default();
        game_server::stats::compute(&ctx.data.status, &base, &walker.items, &ctx.data.items, &passives).max_health()
    };
    // STR 1, DEX 2, REC 1, WIS 1: HP +5, SP +10, MP +5 go out in that order, then the request comes back.
    let points = [1, 2, 1, 0, 1, 0];
    let raised: [u16; 6] = std::array::from_fn(|i| walker.stats[i] + points[i]);
    let ([hp, mp, sp], [old_hp, old_mp, old_sp]) = (maxima(raised), maxima(walker.stats));
    assert_eq!((hp, mp, sp), (old_hp + 5, old_mp + 5, old_sp + 10));
    client.write(w::status_up(points)).await;
    assert_eq!(client.read().await.unwrap(), w::set_max(id, w::MAX_HP, hp));
    assert_eq!(client.read().await.unwrap(), w::set_max(id, w::MAX_SP, sp));
    assert_eq!(client.read().await.unwrap(), w::set_max(id, w::MAX_MP, mp));
    assert_eq!(client.read().await.unwrap(), w::status_up(points));
    // Nothing asked for: no answer; one point left, two asked: zeros.
    client.write(w::status_up([0; 6])).await;
    client.write(w::status_up([0, 0, 0, 2, 0, 0])).await;
    assert_eq!(client.read().await.unwrap(), w::status_up([0; 6]));
    // INT and LUC change no maximum the client is told.
    client.write(w::status_up([0, 0, 0, 0, 0, 1])).await;
    assert_eq!(client.read().await.unwrap(), w::status_up([0, 0, 0, 0, 0, 1]));
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    let mut expected = raised;
    expected[5] += 1;
    assert_eq!((saved.stat_point, saved.stats), (0, expected));
    // Raising the stats: one 151 to 156 per stat with the new value and the points put in, in the original's order (str, dex, int,
    // wis, rec, luc): here str, dex, wis, rec and luc.
    let raised_rows: Vec<(u8, Vec<(usize, i64)>)> =
        (151..=156).flat_map(|action| logged(&ctx, action)).map(|r| (r.action, set_values(&r))).collect();
    let (str_now, dex_now, rec_now, wis_now, luc_now) =
        (i64::from(raised[0]), i64::from(raised[1]), i64::from(raised[2]), i64::from(raised[4]), i64::from(raised[5]));
    assert_eq!(
        raised_rows,
        [
            (151, vec![(1, str_now), (2, 1)]),
            (152, vec![(1, dex_now), (2, 2)]),
            (154, vec![(1, wis_now), (2, 1)]),
            (155, vec![(1, rec_now), (2, 1)]),
            (156, vec![(1, luc_now + 1), (2, 1)]),
        ]
    );
}

#[tokio::test]
async fn using_items_and_resets() {
    use game_server::backend::{CharacterStore, ItemStore, NewItem};
    use game_server::inventory::SlotChange;
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"User")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    // Level 5 with its 20 points spent on DEX; a stone in bag 1 slot 2 and a firecracker in slot 3.
    let user = backend.load(7, SERVER_ID, 1).await.unwrap().unwrap();
    let mut stats = user.stats;
    stats[1] += 20;
    let progress = combat::Progress { level: 5, exp: user.exp, stat_point: 0, skill_point: 9, stats };
    backend.save_progress(user.id, &progress, user.health).await.unwrap();
    let new = |slot, item_type: u8, type_id: u8| {
        SlotChange::Insert(NewItem {
            bag: 1,
            slot,
            item_id: u32::from(item_type) * 1000 + u32::from(type_id),
            item_type,
            type_id,
            quality: 0,
            count: 1,
            kept: None,
        })
    };
    assert!(backend.trade(user.id, 0, &[new(2, 100, 48), new(3, 100, 115), new(4, 100, 131)]).await.unwrap().is_some());
    let id = select_and_enter(&mut client).await;

    // A potion at full health: used up all the same, no 0x0505. Its group's 15 s are not over for the next one.
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 0])).await;
    assert_eq!(client.read().await.unwrap(), it::used(id, 1, 0, 25, 1, 9));
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 0])).await;
    assert_eq!(client.read().await.unwrap(), it::use_failed(id));
    // The firecracker needs level 10; the equipment (bag 0) and an empty slot are ignored.
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 3])).await;
    assert_eq!(client.read().await.unwrap(), it::use_failed(id));
    client.write(shaiya_proto::encode(it::OP_USE, &[0, 5])).await;
    client.write(shaiya_proto::encode(it::OP_USE_ON, &[1, 9, 1, 0, 0, 0])).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0), "nothing answered the ignored uses");

    // The stone: the starting stats with 4 levels of STR, 20 points; 0x0214 first, the current values last, then the stone goes.
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 2])).await;
    let reset = [14 + 4, 12, 9, 8, 7, 11 + 4];
    assert_eq!(client.read().await.unwrap(), it::stats_reset(20, reset));
    let mut packets = Vec::new();
    while let Some(packet) = client.read().await {
        let done = frame::opcode(&packet) == it::OP_USE;
        packets.push(packet);
        if done {
            break;
        }
    }
    assert_eq!(packets.last().unwrap(), &it::used(id, 1, 2, 100, 48, 0));
    assert_eq!(frame::opcode(&packets[packets.len() - 2]), shaiya_proto::combat::OP_RECOVER_SET);
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!((saved.stat_point, saved.stats), (20, reset));
    assert!(saved.items.iter().all(|i| i.item_type != 100 || i.type_id != 48), "the stone is gone");

    // The free stat reset is always refused; the free skill reset works once (3 points a level, 5 more).
    client.write(shaiya_proto::encode(it::OP_RESET_STATS, &[])).await;
    assert_eq!(client.read().await.unwrap(), it::reset_refused(it::OP_RESET_STATS));
    client.write(shaiya_proto::encode(it::OP_RESET_SKILLS, &[])).await;
    assert_eq!(client.read().await.unwrap(), it::skills_reset(17, true));
    assert_eq!(frame::opcode(&client.read().await.unwrap()), shaiya_proto::combat::OP_RECOVER_SET);
    client.write(shaiya_proto::encode(it::OP_RESET_SKILLS, &[])).await;
    assert_eq!(client.read().await.unwrap(), it::reset_refused(it::OP_RESET_SKILLS));
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!((saved.skill_point, saved.free_skill_reset_used), (17, true));

    // Right after a skill request (here a skill the character no longer has), a rune is refused for 3 s: the skill-cut fix.
    client.write(sk::use_request(sk::OP_USE_USER, 0, id)).await;
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 4])).await;
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == it::OP_USE {
            assert_eq!(packet, it::use_failed(id));
            break;
        }
    }
    tokio::time::sleep(Duration::from_millis(1600)).await;
    client.write(shaiya_proto::encode(g::OP_PING, &[])).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // The recall rune casts for 5 s; a step cancels it.
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 4])).await;
    assert_eq!(client.read().await.unwrap(), it::move_cast(id));
    client.write(step(578.0, 1762.0)).await;
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == it::OP_MOVE_END {
            assert_eq!(packet, it::move_end(id, 0));
            break;
        }
    }
    // Again, waiting: 0x0222 with 255, to map 35 (0x020B to the player alone), and the rune used up.
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 4])).await;
    assert_eq!(client.read().await.unwrap(), it::move_cast(id));
    let cast = std::time::Instant::now();
    // The test client gives up reading after 5 s: a ping on the way keeps the connection alive meanwhile.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    client.write(shaiya_proto::encode(g::OP_PING, &[])).await;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(client.read().await.unwrap(), it::move_end(id, 255));
    assert!(cast.elapsed() >= Duration::from_millis(4900));
    assert_eq!(client.read().await.unwrap(), w::position(id, 35, [585.87, 27.0, 224.49]));
    assert_eq!(client.read().await.unwrap(), it::used(id, 1, 4, 100, 131, 0));
}

#[tokio::test]
async fn shutting_down_saves_players_and_closes_every_connection() {
    let backend = backend_with_session([5; 16]);
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut walker = Client::connect(addr).await;
    let id = enter_world(&mut walker, 7, [5; 16], true).await;
    walker.write(step(580.0, 1765.0)).await;
    assert_eq!(frame::opcode(&walker.read().await.unwrap()), w::OP_MOVE);
    let mut waiting = Client::connect(addr).await;
    waiting.enter(8, [6; 16]).await;

    assert!(shut_down(&ctx, Duration::from_secs(5)).await, "every connection finished");
    assert_eq!(walker.read().await, None);
    assert_eq!(waiting.read().await, None);
    use game_server::backend::CharacterStore;
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!(saved.position, [580.0, 78.56, 1765.0], "the last step was saved");
    assert!(backend.session(7).is_none() && backend.session(8).is_none(), "both sessions ended");
    assert_eq!(ctx.players.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn unexpected_character_packets_in_the_world_close_the_connection() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend, true).await;
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], true).await;
    // Unknown packets of other families are only logged (`0x2701`: the web inventory, region 4 only).
    client.write(shaiya_proto::encode(0x2701, &[1, 0, 0, 0])).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOTION);
    client.write(shaiya_proto::encode(g::OP_CHAR_SELECT, &[1, 0, 0, 0])).await;
    assert!(client.read().await.is_none());
}

#[tokio::test]
async fn entering_off_the_map_closes_the_connection() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Lost")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    // As if the character had been saved past the edge of the 2048 map.
    use game_server::backend::CharacterStore;
    backend.save_position(1, 1, [2048.0, 0.0, 100.0], 0, [1, 1, 1]).await.unwrap();
    client.write(g::char_id_request(g::OP_CHAR_SELECT, 1)).await;
    while frame::opcode(&client.read().await.expect("burst")) != w::OP_CHAR_QUICK {}
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    assert!(client.read().await.is_none(), "closed instead of placed");
}

#[tokio::test]
async fn buying_and_selling_at_a_merchant() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Shopper")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    backend.set_money(1, 1000);
    let id = select_and_enter(&mut client).await;
    let reply = |opcode, r: ShopResult| shop::shop_result(opcode, &r);
    let buy = |npc_id, index, count| shop::buy_request(&BuyRequest { npc_id, index, count });

    // Ten potions from the merchant in view: they join the ten the character started with (bag 1 slot 0).
    client.write(buy(1, 0, 10)).await;
    let bought = ShopResult { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 20, money: 900, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), reply(shop::OP_BUY, bought));
    // A sword does not stack: the next empty slot.
    client.write(buy(1, 1, 1)).await;
    let sword = ShopResult { bag: 1, slot: 1, item_type: 5, type_id: 1, count: 1, money: 400, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), reply(shop::OP_BUY, sword));
    // Not enough money: result 1 with the requested count.
    client.write(buy(1, 1, 1)).await;
    let poor = ShopResult { result: shop::result::NO_MONEY, item_type: 5, type_id: 1, count: 1, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), reply(shop::OP_BUY, poor));

    // Ignored without an answer: the merchant out of view, an index past its goods, more than a stack, two swords.
    for request in [buy(2, 0, 1), buy(1, 2, 1), buy(1, 0, 101), buy(1, 1, 2), buy(1, 0, 0)] {
        client.write(request).await;
    }
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0), "nothing answered the ignored requests");

    // Selling five potions, then the sword: 2 and 100 each.
    client.write(shop::sell_request(1, 0, 5)).await;
    let sold = ShopResult { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 15, money: 410, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), reply(shop::OP_SELL, sold));
    client.write(shop::sell_request(1, 1, 1)).await;
    let sold_out = ShopResult { bag: 1, slot: 1, item_type: 5, type_id: 1, count: 0, money: 510, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), reply(shop::OP_SELL, sold_out));
    // The equipped sword cannot be sold.
    client.write(shop::sell_request(0, 5, 1)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));

    // Every change was saved as it happened.
    let (money, items) = backend.stored(1).unwrap();
    assert_eq!(money, 510);
    let mut slots: Vec<_> = items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    slots.sort();
    assert_eq!(slots, [(0, 5, 5, 1), (1, 0, 25, 15)]);

    // The game log: both buys with the NPC, the count, the price and the money after; both sells (no NPC); and the rows' ids, which
    // follow the potions' stack and the sword from the purchase to the sale.
    let buys = logged(&ctx, 113);
    assert_eq!(buys.len(), 2);
    let (potions, sword) = (set_values(&buys[0]), set_values(&buys[1]));
    assert_eq!(potions[1..], [(2, 25001), (3, 1), (4, 10), (5, 100), (6, 900)]);
    assert_eq!(sword[1..], [(2, 5001), (3, 1), (4, 1), (5, 500), (6, 400)]);
    assert_eq!((buys[0].texts[0].as_deref(), buys[0].header.character_name.as_str(), buys[0].header.map), (Some(""), "Shopper", 1));
    let sells = logged(&ctx, 114);
    assert_eq!(sells.len(), 2);
    let (sold_potions, sold_sword) = (set_values(&sells[0]), set_values(&sells[1]));
    assert_eq!(sold_potions[1..], [(2, 25001), (4, 5), (5, 10), (6, 410)], "no NPC: V3 is NULL");
    assert_eq!(sold_sword[1..], [(2, 5001), (4, 1), (5, 100), (6, 510)]);
    assert_eq!(sword[0], sold_sword[0], "the sword keeps its unique id from the purchase to the sale");
    assert_eq!(potions[0], sold_potions[0], "the bought potions went onto the stack the sale took from");
    assert_eq!(sells[1].texts[3].as_deref(), Some("bag=1,slot=1"));
    assert_eq!(logged(&ctx, 107).len(), 1, "entering the world");
}

#[tokio::test]
async fn gatekeepers_move_players_on_the_map_and_to_other_maps() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Traveler")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    backend.set_money(1, 400);
    let id = select_and_enter(&mut client).await;

    // An empty destination fails, with the money unchanged.
    client.write(w::gatekeeper_request(3, 2)).await;
    assert_eq!(client.read().await.unwrap(), w::gatekeeper_result(false, 400));

    // Same map: paid, then the view changes (the NPCs at the start leave it, gatekeepers 4 and 5 come into it) and 0x020B.
    client.write(w::gatekeeper_request(3, 0)).await;
    assert_eq!(client.read().await.unwrap(), w::gatekeeper_result(true, 300));
    let mut seen = Vec::new();
    let arrived = loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == w::OP_POSITION {
            break packet;
        }
        seen.push((frame::opcode(&packet), u32::from_le_bytes(frame::payload(&packet)[..4].try_into().unwrap())));
    };
    seen.sort();
    assert_eq!(seen, [(0x0E01, 4), (0x0E01, 5), (0x0E02, 1), (0x0E02, 3), (0x0E02, 6), (0x0E02, 7)], "no 0x0501 for a teleport");
    assert_eq!(arrived, w::position(id, 1, [1000.0, 80.0, 1000.0]));

    // Gatekeeper 3 is now far away; gatekeeper 5 serves only Fury.
    client.write(w::gatekeeper_request(3, 1)).await;
    assert_eq!(client.read().await.unwrap(), w::gatekeeper_result(false, 300));
    client.write(w::gatekeeper_request(5, 1)).await;
    assert_eq!(client.read().await.unwrap(), w::gatekeeper_result(false, 300));

    // Another map: paid, 0x020B with the new map, then the client's 0x0201 places the player there.
    client.write(w::gatekeeper_request(4, 1)).await;
    assert_eq!(client.read().await.unwrap(), w::gatekeeper_result(true, 100));
    assert_eq!(client.read().await.unwrap(), w::position(id, 2, [165.25, 119.4, 397.89]));
    client.write(step(170.0, 400.0)).await;
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    let (enters, npcs) = read_zone_entry(&mut client).await;
    assert_eq!((enters.len(), npcs.len()), (1, 0), "placed alone on map 2; the step before 0x0201 was ignored");
    client.write(step(170.0, 400.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE, "moving on map 2");

    // 0x0002 closes the connection like a lost one: saved after the stay.
    client.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert!(client.read().await.is_none());
    use game_server::backend::CharacterStore;
    wait_gone(&backend, 7, Duration::from_secs(1)).await;
    let saved = backend.load(7, SERVER_ID, id).await.unwrap().unwrap();
    assert_eq!((saved.map, saved.position, saved.money), (2, [170.0, 78.56, 400.0], 100));
}

#[tokio::test]
async fn portals_check_faction_and_map_and_move_players() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;

    // Ignored without an answer: no such portal, a Fury portal for a Light character.
    client.write(w::portal_request(9)).await;
    client.write(w::portal_request(3)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0), "nothing answered the ignored portals");
    // A party instance zone without a party: reason 4.
    client.write(w::portal_request(2)).await;
    assert_eq!(client.read().await.unwrap(), w::portal_refused(2, 4));

    // Portal 0 leads to map 2: only the 0x020B of the move, then the client's 0x0201 places the player there.
    client.write(w::portal_request(0)).await;
    assert_eq!(client.read().await.unwrap(), w::position(id, 2, [165.25, 118.5, 397.89]));
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    let (enters, _) = read_zone_entry(&mut client).await;
    assert_eq!(enters.len(), 1);
    // Map 2 has no portals.
    client.write(w::portal_request(0)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));
}

#[tokio::test]
async fn moving_and_equipping_items() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    // Walker starts with the sword equipped (bag 0 slot 5) and ten potions in bag 1 slot 0.
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let place =
        |bag, slot, item_type: u8, count| w::ItemUnit { bag, slot, item_type, type_id: 1, quality: 1200, count, ..Default::default() };
    let sword = |bag, slot| place(bag, slot, 5, 1);
    let potions = |bag, slot, count| place(bag, slot, 25, count);

    // Unequip the sword into bag 1 slot 1: the view gets 0x0507 (slot 5 empty), then the answer with both places.
    client.write(w::item_move_request((0, 5), (1, 1))).await;
    assert_eq!(client.read().await.unwrap(), w::equipment(id, 5, None));
    assert_eq!(client.read().await.unwrap(), w::item_move_result((0, 5), None, (1, 1), Some(&sword(1, 1))));
    // Potions are no weapon: answered with both places unchanged, and no 0x0507.
    client.write(w::item_move_request((1, 0), (0, 5))).await;
    assert_eq!(client.read().await.unwrap(), w::item_move_result((1, 0), Some(&potions(1, 0, 10)), (0, 5), None));
    // Equip the sword again.
    client.write(w::item_move_request((1, 1), (0, 5))).await;
    assert_eq!(client.read().await.unwrap(), w::equipment(id, 5, Some(&sword(0, 5))));
    assert_eq!(client.read().await.unwrap(), w::item_move_result((1, 1), None, (0, 5), Some(&sword(0, 5))));

    // 95 more potions make a second stack in bag 1 slot 1; moving the 10 onto it fills it to 100 and leaves 5.
    backend.set_money(1, 0);
    {
        use game_server::backend::ItemStore;
        let item = game_server::backend::NewItem {
            bag: 1,
            slot: 1,
            item_id: 25001,
            item_type: 25,
            type_id: 1,
            quality: 1200,
            count: 95,
            kept: None,
        };
        assert!(backend.trade(1, 0, &[game_server::inventory::SlotChange::Insert(item)]).await.unwrap().is_some());
    }
    // The server knows the bags as loaded; leave and come back to see the new stack.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    client.read().await.unwrap();
    client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    assert_eq!(client.read().await.unwrap(), w::logout_done(w::OP_LOGOUT_TO_SELECT));
    client.read().await.unwrap();
    client.read_list().await;
    select_and_enter(&mut client).await;
    client.write(w::item_move_request((1, 0), (1, 1))).await;
    assert_eq!(client.read().await.unwrap(), w::item_move_result((1, 0), Some(&potions(1, 0, 5)), (1, 1), Some(&potions(1, 1, 100))));
    // Moving the rest to an empty slot; bag 3 is not usable (two bags until the bag skill): ignored.
    client.write(w::item_move_request((1, 0), (2, 23))).await;
    assert_eq!(client.read().await.unwrap(), w::item_move_result((1, 0), None, (2, 23), Some(&potions(2, 23, 5))));
    client.write(w::item_move_request((2, 23), (3, 0))).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));

    let (_, items) = backend.stored(1).unwrap();
    let mut slots: Vec<_> = items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    slots.sort();
    assert_eq!(slots, [(0, 5, 5, 1), (1, 1, 25, 100), (2, 23, 25, 5)]);
}

#[tokio::test]
async fn using_the_warehouse() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Keeper")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    backend.set_money(1, 100);
    // The account's warehouse holds 95 potions in slot 0 and a sword in slot 130 (the second half).
    let place =
        |bag, slot, item_type: u8, count| w::ItemUnit { bag, slot, item_type, type_id: 1, quality: 1200, count, ..Default::default() };
    backend.set_bank(7, SERVER_ID, vec![place(100, 0, 25, 95), place(100, 130, 5, 1)]);

    // The burst carries the warehouse right after the items.
    client.write(g::char_id_request(g::OP_CHAR_SELECT, 1)).await;
    let mut burst = Vec::new();
    while burst.last().is_none_or(|p: &Vec<u8>| frame::opcode(p) != w::OP_CHAR_QUICK) {
        burst.push(client.read().await.expect("burst"));
    }
    let at = burst.iter().position(|p| frame::opcode(p) == w::OP_CHAR_BANK).expect("0x0711");
    assert_eq!(frame::opcode(&burst[at - 1]), w::OP_CHAR_ITEM);
    assert_eq!(burst[at], w::char_bank(0, &[place(100, 0, 25, 95), place(100, 130, 5, 1)])[0]);
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(&mut client).await;
    let id = 1;

    // The ten potions go into slot 1 (no fee), then onto the 95: 100 there, 5 left.
    client.write(w::item_move_request((1, 0), (100, 1))).await;
    assert_eq!(client.read().await.unwrap(), w::item_move_result((1, 0), None, (100, 1), Some(&place(100, 1, 25, 10))));
    client.write(w::item_move_request((100, 1), (100, 0))).await;
    let combined = w::item_move_result((100, 1), Some(&place(100, 1, 25, 5)), (100, 0), Some(&place(100, 0, 25, 100)));
    assert_eq!(client.read().await.unwrap(), combined);
    // Ignored without an answer: the second half without Double Warehouse, the equipment straight in, into a taken slot, out into
    // a taken bag slot or the equipment.
    client.write(w::item_move_request((100, 1), (100, 200))).await;
    client.write(w::item_move_request((0, 5), (100, 2))).await;
    client.write(w::item_move_request((100, 130), (0, 5))).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0), "nothing answered the ignored moves");

    // Taking out: 5 % of the sell price, the money in the answer. 100 potions at 2: 10; the sword at 100: 5.
    client.write(w::item_move_request((100, 0), (1, 0))).await;
    assert_eq!(client.read().await.unwrap(), w::bank_to_bag_result((100, 0), None, (1, 0), Some(&place(1, 0, 25, 100)), 90));
    client.write(w::item_move_request((100, 130), (1, 0))).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0), "the bag slot is taken");
    client.write(w::item_move_request((100, 130), (1, 1))).await;
    assert_eq!(client.read().await.unwrap(), w::bank_to_bag_result((100, 130), None, (1, 1), Some(&place(1, 1, 5, 1)), 85));

    // Saved as it happened: the account's warehouse and the character's bags and money.
    let bank: Vec<_> = backend.bank(7, SERVER_ID).iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    assert_eq!(bank, [(100, 1, 25, 5)]);
    let (money, items) = backend.stored(1).unwrap();
    let mut slots: Vec<_> = items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    slots.sort();
    assert_eq!((money, slots), (85, vec![(0, 5, 5, 1), (1, 0, 25, 100), (1, 1, 5, 1)]));

    // The game log: two moves in (121; the ten potions into slot 1, then the combine is inside the warehouse and not logged) and two
    // out (122, with the fee in V8); the sword's row is the same going out as it was.
    let into = logged(&ctx, 121);
    assert_eq!(into.len(), 1);
    assert_eq!(set_values(&into[0])[1..], [(2, 25001), (3, 10), (4, 1), (5, 0), (7, 1)]);
    let out = logged(&ctx, 122);
    assert_eq!(out.len(), 2);
    assert_eq!(set_values(&out[0])[1..], [(2, 25001), (3, 100), (4, 0), (6, 1), (7, 0), (8, 10)]);
    assert_eq!(set_values(&out[1])[1..], [(2, 5001), (3, 1), (4, 130), (6, 1), (7, 1), (8, 5)]);
}

#[tokio::test]
async fn monsters_spawn_in_their_areas_and_answer_queries() {
    let backend = backend_with_session([5; 16]);
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let respawn = || ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);

    // The first pass spawns one monster per entry (the near area's and the far one's); the next fills the near area.
    assert_eq!(respawn(), 2);
    assert_eq!(client.read().await.unwrap(), w::mob_enter(1, true, 1, 600.0, 1780.0), "only the monster in view");
    assert_eq!(client.read().await.unwrap(), w::mob_max_hp(1, 120, 120));
    assert_eq!(respawn(), 1);
    assert_eq!(client.read().await.unwrap(), w::mob_enter(3, true, 1, 600.0, 1780.0));
    assert_eq!(client.read().await.unwrap(), w::mob_max_hp(3, 120, 120));
    assert_eq!(respawn(), 0, "every area is full");
    assert_eq!(ctx.zones.mob_count(1), 3);

    // The queries: a monster's state (after its position), its HP, a player's HP; nothing for an unknown monster.
    client.write(g::char_id_request(w::OP_MOB_INFO, 1)).await;
    assert_eq!(client.read().await.unwrap(), w::mob_move(1, 1, 600.0, 1780.0));
    assert_eq!(client.read().await.unwrap(), w::mob_info(1, 120, 3, 2));
    client.write(g::char_id_request(w::OP_MOB_HP, 3)).await;
    assert_eq!(client.read().await.unwrap(), w::mob_hp(3, 120));
    client.write(g::char_id_request(w::OP_MOB_HP, 99)).await;
    client.write(g::char_id_request(w::OP_USER_HP, id)).await;
    // A human fighter: 205 + 9 REC * 5.
    assert_eq!(client.read().await.unwrap(), w::user_hp(id, 250, 250));

    // A player entering later sees the monsters in view as already there (byNew 0), after the players and before the NPCs.
    let mut other = Client::connect(addr).await;
    other.enter(8, [6; 16]).await;
    other.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    other.write(create(0, 0, 0, b"Other")).await;
    assert_eq!(other.read().await.unwrap(), g::char_create_response(0));
    other.read_list().await;
    other.write(g::char_id_request(g::OP_CHAR_SELECT, 2)).await;
    while frame::opcode(&other.read().await.expect("burst")) != w::OP_CHAR_QUICK {}
    other.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    let mut seen = Vec::new();
    loop {
        let packet = other.read().await.unwrap();
        let op = frame::opcode(&packet);
        if op == w::OP_WEATHER {
            break;
        }
        if op == w::OP_MOB_ENTER {
            seen.push(packet);
        } else {
            seen.push(shaiya_proto::encode(op, &[]));
        }
    }
    let ops: Vec<_> = seen.iter().map(|p| frame::opcode(p)).collect();
    let first_mob = ops.iter().position(|&op| op == w::OP_MOB_ENTER).unwrap();
    assert!(ops[..first_mob].iter().all(|&op| op == w::OP_USER_ENTER || op == w::OP_SPEED), "players first: {ops:x?}");
    assert_eq!(ops[first_mob..first_mob + 4], [w::OP_MOB_ENTER, w::OP_MOB_MAX_HP, w::OP_MOB_ENTER, w::OP_MOB_MAX_HP]);
    assert!(ops[first_mob + 4..].iter().all(|&op| op == w::OP_NPC_ENTER), "NPCs last: {ops:x?}");
    let mut mobs: Vec<_> = seen.into_iter().filter(|p| frame::opcode(p) == w::OP_MOB_ENTER).collect();
    mobs.sort();
    let mut expected = vec![w::mob_enter(1, false, 1, 600.0, 1780.0), w::mob_enter(3, false, 1, 600.0, 1780.0)];
    expected.sort();
    assert_eq!(mobs, expected);
}

#[tokio::test]
async fn select_needs_an_own_live_character() {
    let backend = backend_with_session([5; 16]);
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, _) = start(backend.clone(), true).await;
    let mut owner = Client::connect(addr).await;
    owner.enter(7, [5; 16]).await;
    owner.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    owner.write(create(0, 0, 0, b"Hero")).await;
    owner.read().await.unwrap();
    let hero_id = u32::from_le_bytes(owner.read_list().await[4][1..5].try_into().unwrap());

    // Another account cannot select it: no answer, and the one select per list is used up.
    let mut other = Client::connect(addr).await;
    other.enter(8, [6; 16]).await;
    other.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    other.write(g::char_id_request(g::OP_CHAR_SELECT, hero_id)).await;
    other.write(g::char_id_request(g::OP_CHAR_DELETE, 999)).await;
    other.write(g::char_id_request(g::OP_CHAR_RESTORE, 999)).await;
    // Nothing comes back for any of these; the connection stays open until the client leaves.
    other.write(shaiya_proto::encode(g::OP_CHAR_LEAVE, &[])).await;
    assert!(other.read().await.is_none());

    // A character pending deletion cannot be selected either.
    owner.write(g::char_id_request(g::OP_CHAR_DELETE, hero_id)).await;
    assert_eq!(owner.read().await.unwrap(), g::char_success(g::OP_CHAR_DELETE, hero_id));
    owner.write(g::char_id_request(g::OP_CHAR_SELECT, hero_id)).await;
    owner.write(shaiya_proto::encode(g::OP_CHAR_LEAVE, &[])).await;
    assert!(owner.read().await.is_none());
}

#[tokio::test]
async fn two_players_see_each_other() {
    let backend = Arc::new(MemoryBackend::default());
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, ctx) = start(backend, true).await;

    // Two accounts, one character each, both human so both start at the same spot on map 1.
    let mut alice = Client::connect(addr).await;
    alice.enter(7, [5; 16]).await;
    alice.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    alice.write(create(0, 0, 0, b"Alice")).await;
    alice.read().await.unwrap();
    let alice_id = u32::from_le_bytes(alice.read_list().await[4][1..5].try_into().unwrap());
    let mut bob = Client::connect(addr).await;
    bob.enter(8, [6; 16]).await;
    bob.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    bob.write(create(0, 0, 0, b"Bob")).await;
    bob.read().await.unwrap();
    let bob_id = u32::from_le_bytes(bob.read_list().await[4][1..5].try_into().unwrap());

    for (client, id) in [(&mut alice, alice_id), (&mut bob, bob_id)] {
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
    }
    alice.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    assert_eq!(read_zone_entry(&mut alice).await.0.len(), 1);
    bob.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    let (seen, _) = read_zone_entry(&mut bob).await;
    assert_eq!(seen.len(), 2, "Bob sees himself and Alice");
    assert_eq!(frame::payload(&seen[1])[..5], [alice_id as u8, 0, 0, 0, 0], "Alice, already there");

    // Alice is told Bob arrived, then sees him move and sit.
    let arrived = alice.read().await.unwrap();
    assert_eq!(frame::payload(&arrived)[..5], [bob_id as u8, 0, 0, 0, 1], "Bob, new");
    assert_eq!(frame::opcode(&alice.read().await.unwrap()), w::OP_SPEED);
    bob.write(step(580.0, 1765.0)).await;
    assert_eq!(frame::opcode(&bob.read().await.unwrap()), w::OP_MOVE, "Bob's own echo");
    let moved = alice.read().await.unwrap();
    assert_eq!(moved, w::user_move(bob_id, &w::Move { motion: 0, dir: 90, position: [580.0, 78.56, 1765.0] }));
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    assert_eq!(bob.read().await.unwrap(), w::motion(bob_id, 1));
    assert_eq!(alice.read().await.unwrap(), w::motion(bob_id, 1));

    // Alice asks what Bob looks like: a position sync, then his looks with his name and the sword in slot 5.
    alice.write(shaiya_proto::encode(w::OP_USER_SHAPE, &bob_id.to_le_bytes())).await;
    let sync = alice.read().await.unwrap();
    assert_eq!((frame::opcode(&sync), frame::payload(&sync)[4]), (w::OP_MOVE, w::MOTION_SYNC));
    let shape = alice.read().await.unwrap();
    let body = frame::payload(&shape);
    assert_eq!(frame::opcode(&shape), w::OP_USER_SHAPE);
    assert_eq!(body[5], 1, "sitting");
    assert_eq!(body[19 + 15..19 + 17], [5, 1], "sword in slot 5");
    assert_eq!(&body[43..47], b"Bob\0");
    alice.write(shaiya_proto::encode(w::OP_USER_INFO, &bob_id.to_le_bytes())).await;
    alice.read().await.unwrap();
    let info = alice.read().await.unwrap();
    assert_eq!(frame::payload(&info)[..4], bob_id.to_le_bytes());

    // A colour marker running past the text would crash both clients: dropped, not even echoed (docs/client/client-packets.md).
    alice.write(shaiya_proto::encode(chat::OP_NORMAL, &[9, b'<', b'<', b'9', b'9', b'9', b'c', b'0', b'1', 0])).await;
    // Normal chat: both get it (Bob is about 3 units from Alice), and it is the next thing either receives.
    alice.write(shaiya_proto::encode(chat::OP_NORMAL, &[4, b'h', b'e', b'y', 0])).await;
    let said = chat::by_id(chat::OP_NORMAL, alice_id, b"hey\0");
    assert_eq!(alice.read().await.unwrap(), said);
    assert_eq!(bob.read().await.unwrap(), said);
    // A whisper to "alice" (any case): Alice gets it from Bob, Bob gets the copy addressed to Alice.
    let mut whisper = b"alice".to_vec();
    whisper.resize(21, 0);
    whisper.extend_from_slice(&[3, b'y', b'o', 0]);
    bob.write(shaiya_proto::encode(chat::OP_WHISPER, &whisper)).await;
    assert_eq!(bob.read().await.unwrap(), chat::whisper(true, b"Alice", b"yo\0"));
    assert_eq!(alice.read().await.unwrap(), chat::whisper(false, b"Bob", b"yo\0"));
    // Trade chat (0x1103, the whole faction) needs level 10: refused at level 1, so the next packet Bob gets is the 0x1106 below.
    bob.write(shaiya_proto::encode(chat::OP_TRADE, &[3, b'w', b't', 0])).await;

    // Nobody of that name: 0x1106.
    let mut nobody = b"Nobody".to_vec();
    nobody.resize(21, 0);
    nobody.extend_from_slice(&[3, b'y', b'o', 0]);
    bob.write(shaiya_proto::encode(chat::OP_WHISPER, &nobody)).await;
    assert_eq!(bob.read().await.unwrap(), chat::whisper_failed());
    // The chat log: Alice's "hey" (type 1), Bob's whisper (type 2, with the target) and nothing for the one nobody got.
    use game_server::game_log::Entry;
    let lines: Vec<_> =
        ctx.game_log.recorded().into_iter().filter_map(|e| if let Entry::Chat(row) = e { Some(row) } else { None }).collect();
    assert_eq!(
        lines.iter().map(|l| (l.chat_type, l.target.as_deref(), l.text.as_str())).collect::<Vec<_>>(),
        [(1, None, "hey"), (2, Some("Alice"), "yo")]
    );
    assert_eq!((lines[0].character_id, lines[0].map), (alice_id, 1));

    // Bob disconnects: Alice gets 0x0202.
    bob.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert_eq!(alice.read().await.unwrap(), w::user_leave(bob_id));
}

#[tokio::test]
async fn trade_chat_has_a_level_and_a_cooldown() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Trader")).await;
    client.read().await.unwrap();
    client.read_list().await;
    backend.set_level(1, 10);
    let id = select_and_enter(&mut client).await;

    let trade = |text: &[u8]| shaiya_proto::encode(chat::OP_TRADE, &[&[text.len() as u8][..], text].concat());
    client.write(trade(b"wts\0")).await;
    assert_eq!(client.read().await.unwrap(), chat::by_name(chat::OP_TRADE, b"Trader", b"wts\0"), "sent to the faction, sender included");
    // Within the cooldown: dropped. The motion reply shows nothing came in between.
    client.write(trade(b"again\0")).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));
    tokio::time::sleep(Duration::from_millis(350)).await;
    client.write(trade(b"later\0")).await;
    assert_eq!(client.read().await.unwrap(), chat::by_name(chat::OP_TRADE, b"Trader", b"later\0"));
}

#[tokio::test]
async fn using_skills_on_a_monster() {
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    for skill in [5, 6] {
        client.write(sk::learn_request(skill, 1)).await;
        client.read().await.unwrap();
    }
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    assert_eq!(client.read().await.unwrap(), w::mob_enter(1, true, 1, 600.0, 1780.0));
    assert_eq!(client.read().await.unwrap(), w::mob_max_hp(1, 120, 120));
    client.write(step(598.0, 1779.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE);

    // Skill 6 (number 1) starts a cast at once; a step stops it, and nothing more is said.
    client.write(sk::use_request(sk::OP_USE_MOB, 1, 1)).await;
    assert_eq!(client.read().await.unwrap(), cp::attack_start());
    assert_eq!(client.read().await.unwrap(), sk::casting(sk::OP_CAST_MOB, id, 1, 6, 1));
    client.write(step(598.5, 1779.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE);
    // Skill 5 (number 0) goes off when the cast would have ended: the hit, then the SP it took.
    client.write(sk::use_request(sk::OP_USE_MOB, 0, 1)).await;
    assert_eq!(client.read().await.unwrap(), cp::attack_start());
    assert_eq!(client.read().await.unwrap(), sk::skill_use(sk::OP_USE_MOB, 0, id, 1, 5, 1, [50, 0, 0]));
    assert_eq!(client.read().await.unwrap(), sk::costs(10, 0));
    assert_eq!(ctx.zones.mob(1, 1).map(|m| m.hp), Some(70));
    // Aimed at a player: a wrong target, at once.
    client.write(sk::use_request(sk::OP_USE_USER, 0, 1)).await;
    assert_eq!(client.read().await.unwrap(), sk::skill_use(sk::OP_USE_USER, 11, id, 0, 5, 1, [0; 3]));

    // Back to the character screen and in again: the cooldown left comes in the skill list, and is saved.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    client.read().await.unwrap();
    client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    assert_eq!(client.read().await.unwrap(), w::logout_done(w::OP_LOGOUT_TO_SELECT));
    client.read().await.unwrap();
    client.read_list().await;
    let stored = backend.play(id).unwrap();
    let delay = stored.skills.iter().find(|s| s.id == 5).unwrap().delay;
    assert!((27..30).contains(&delay), "about 29 s left: {delay}");
    assert_eq!(stored.skills.iter().find(|s| s.id == 6).unwrap().delay, 0, "the cancelled cast cost nothing");
    client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
    let mut skills = None;
    loop {
        let packet = client.read().await.expect("burst");
        if frame::opcode(&packet) == sk::OP_CHAR_SKILL {
            skills = Some(packet);
        } else if frame::opcode(&packet) == w::OP_CHAR_QUICK {
            break;
        }
    }
    let entry = |skill_id, number, delay| sk::SkillEntry { skill_id, level: 1, number, delay };
    assert_eq!(skills.unwrap(), sk::char_skills(&[entry(5, 0, u32::from(delay)), entry(6, 1, 0)]));
}

#[tokio::test]
async fn killing_a_monster_gives_experience_and_a_level() {
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    assert_eq!(client.read().await.unwrap(), w::mob_enter(1, true, 1, 600.0, 1780.0));
    assert_eq!(client.read().await.unwrap(), w::mob_max_hp(1, 120, 120));

    // Up to the monster (a fresh step leaves a slack of 2: 2 + 2.3 of reach), then attack: 0x0212 first.
    client.write(step(598.0, 1779.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE);
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), cp::attack_start());
    // A miss (1 in 100) waits for the next attack; a hit kills: 0x0606, then the blow.
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == cp::OP_MOB_DEATH {
            assert_eq!(packet, cp::mob_death(1, cp::KILLER_PLAYER, id));
            break;
        }
        assert_eq!(packet, cp::attack_mob(combat::RESULT_MISS, id, 1, 0), "only a miss comes before the death");
    }
    let blow = client.read().await.unwrap();
    assert_eq!(frame::opcode(&blow), cp::OP_ATTACK_MOB);
    let p = frame::payload(&blow);
    assert!(p[0] <= combat::RESULT_CRITICAL && u16::from_le_bytes([p[9], p[10]]) >= 120, "a killing blow: {p:?}");

    // 100 * 1.2 * 1.2 = 144: level 2 (the totals are 100 and 300). The player made the kill: 0x0508.
    assert_eq!(client.read().await.unwrap(), cp::exp_gain(144, 0));
    // Stat points 0 + 5, skill points 5 (from creation) + 3.
    assert_eq!(client.read().await.unwrap(), cp::level_up(false, id, 2, 5, 8, (100, 300)));
    // Fighter level 2 (215, 130, 55) plus STR 15 DEX 12 REC 9 WIS 7: HP 215 + 45, MP 55 + 35, SP 130 + 10 + 60.
    assert_eq!(client.read().await.unwrap(), cp::max_all(id, [260, 90, 200]));
    assert_eq!(client.read().await.unwrap(), cp::recover(id, [10, 5, 5]));
    let stored = backend.play(id).unwrap();
    assert_eq!((stored.level, stored.exp, stored.stat_point, stored.skill_point), (2, 144, 5, 8));
    assert_eq!(stored.stats[0], 15, "a fighter gains STR");
    assert_eq!(stored.health, [260, 90, 200]);
    // One 146 per level gained, at the new level.
    let levels = logged(&ctx, 146);
    assert_eq!((levels.len(), levels[0].header.level, levels[0].header.character_id), (1, 2, id));
    // The monster is gone: attacking it again does nothing.
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));
}

#[tokio::test]
async fn an_item_buff_is_not_applied_when_the_item_cannot_be_used_up() {
    use game_server::backend::{ItemStore, NewItem};
    use game_server::inventory::SlotChange;
    // Audit R13: the item is saved as used before the buff takes effect. A Large Increase EXP (skill 223, ability 35).
    let mut data = data();
    let buff = SkillDef {
        id: 223,
        level: 1,
        country: 6,
        jobs: [true; 6],
        type_effect: game_server::skills::BUFF,
        kind: game_server::buffs::ABILITIES,
        keep_time: 720,
        target_type: 2,
        success_type: 1,
        success_value: 100,
        keep_on_death: true,
        abilities: [(35, 1), (0, 0), (0, 0)],
        ..Default::default()
    };
    data.skills = std::mem::take(&mut data.skills).with([buff]);
    let stone = *data.items.get(100, 48).unwrap();
    let exp = ItemDef { item_id: 100043, type_id: 43, special: 0, range: 223, attack_time: 1, ..stone };
    data.items = ItemDefs::new(data.items.iter().cloned().chain([exp]));
    let backend = backend_with_session([5; 16]);
    let ctx = Arc::new(Context::new(config(true), backend.clone(), data));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx.clone()));
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    let item =
        SlotChange::Insert(NewItem { bag: 1, slot: 2, item_id: 100_043, item_type: 100, type_id: 43, quality: 0, count: 1, kept: None });
    assert!(backend.trade(1, 0, &[item]).await.unwrap().is_some());
    let id = select_and_enter(&mut client).await;

    backend.fail_next_trade.store(true, std::sync::atomic::Ordering::SeqCst);
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 2])).await;
    // The write fails and the connection ends; neither the buff's packets nor `0x050A` came before.
    while let Some(packet) = client.read().await {
        assert_ne!(frame::opcode(&packet), it::OP_USE, "no used-item packet: {packet:?}");
        assert_ne!(frame::opcode(&packet), sk::OP_USE_USER, "no buff applied: {packet:?}");
    }
    let (_, items) = backend.stored(id).unwrap();
    assert!(items.iter().any(|i| (i.bag, i.slot, i.type_id) == (1, 2, 43)), "the item is still in the bags");
}

#[tokio::test]
async fn an_exp_stone_and_a_gold_item_raise_kills_and_gold() {
    use game_server::backend::{ItemStore, NewItem};
    use game_server::drops::Loot;
    use game_server::inventory::SlotChange;
    use shaiya_proto::combat as cp;
    // A Large Increase EXP (skill 223, ability 35) and a Large Increase Gold Drop (235, ability 44 at 100), both `wType` 82 buffs.
    let mut data = data();
    let buff = |id, ability, value| SkillDef {
        id,
        level: 1,
        country: 6,
        jobs: [true; 6],
        type_effect: game_server::skills::BUFF,
        kind: game_server::buffs::ABILITIES,
        keep_time: 720,
        // As the rows of `defs.skills`: on oneself, always succeeding, kept in hours over death.
        target_type: 2,
        success_type: 1,
        success_value: 100,
        keep_on_death: true,
        abilities: [(ability, value), (0, 0), (0, 0)],
        ..Default::default()
    };
    data.skills = std::mem::take(&mut data.skills).with([buff(223, 35, 1), buff(235, 44, 100)]);
    let stone = *data.items.get(100, 48).unwrap();
    let exp = ItemDef { item_id: 100043, type_id: 43, special: 0, range: 223, attack_time: 1, ..stone };
    let gold = ItemDef { item_id: 100076, type_id: 76, special: 0, range: 235, attack_time: 1, ..stone };
    data.items = ItemDefs::new(data.items.iter().cloned().chain([exp, gold]));
    let backend = backend_with_session([5; 16]);
    let ctx = Arc::new(Context::new(config(true), backend.clone(), data));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx.clone()));
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    let item = |slot, type_id: u8| {
        SlotChange::Insert(NewItem {
            bag: 1,
            slot,
            item_id: 100_000 + u32::from(type_id),
            item_type: 100,
            type_id,
            quality: 0,
            count: 1,
            kept: None,
        })
    };
    assert!(backend.trade(1, 0, &[item(2, 43), item(3, 76)]).await.unwrap().is_some());
    let id = select_and_enter(&mut client).await;
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 2])).await;
    assert_eq!(read_op(&mut client, it::OP_USE).await, it::used(id, 1, 2, 100, 43, 0));
    client.write(shaiya_proto::encode(it::OP_USE, &[1, 3])).await;
    assert_eq!(read_op(&mut client, it::OP_USE).await, it::used(id, 1, 3, 100, 76, 0));

    // A monster's gold picked up: 70 000 + 100 %.
    let here = [578.0, 0.0, 1763.0];
    ctx.zones.drop_on_ground(
        1,
        Loot::Gold(70_000),
        here,
        id,
        0,
        std::time::Instant::now(),
        gp::KIND_DROPPED,
        game_server::zone::Dropped::MOB,
    );
    let entered = read_op(&mut client, gp::OP_ITEM_ENTER).await;
    let ground_id = u32::from_le_bytes(frame::payload(&entered)[0..4].try_into().unwrap());
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &ground_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut client, gp::OP_PICK_UP).await, gp::picked_up_gold(0, 1, 140_000));
    assert_eq!(backend.stored(id).unwrap().0, 140_000);
    // A player's own drop is not raised.
    ctx.zones.drop_on_ground(
        1,
        Loot::Gold(500),
        here,
        id,
        0,
        std::time::Instant::now(),
        gp::KIND_PLAYER_DROP,
        game_server::zone::Dropped::player(id),
    );
    let entered = read_op(&mut client, gp::OP_ITEM_ENTER).await;
    let ground_id = u32::from_le_bytes(frame::payload(&entered)[0..4].try_into().unwrap());
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &ground_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut client, gp::OP_PICK_UP).await, gp::picked_up_gold(0, 1, 500));

    // The kill of `killing_a_monster_gives_experience_and_a_level`: 144 doubled by the stone.
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    client.write(step(598.0, 1779.0)).await;
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    assert_eq!(read_op(&mut client, cp::OP_EXP).await, cp::exp_gain(288, 0));
    assert_eq!(backend.play(id).unwrap().exp, 288);
}

#[tokio::test]
async fn picking_up_gold_and_items() {
    use game_server::drops::Loot;
    use shaiya_proto::ground as gp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    // Walker: the sword in bag 0 slot 5, ten potions in bag 1 slot 0, at (577.98, 1762.12).
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let now = std::time::Instant::now();
    let here = [578.0, 0.0, 1763.0];
    let lay = |loot| ctx.zones.drop_on_ground(1, loot, here, id, 0, now, gp::KIND_DROPPED, game_server::zone::Dropped::MOB);

    lay(Loot::Gold(70_000));
    assert_eq!(client.read().await.unwrap(), gp::item_enter(1, gp::KIND_DROPPED, 26, 0, 1, here));
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &1u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), gp::item_leave(1));
    assert_eq!(client.read().await.unwrap(), gp::picked_up_gold(0, 1, 70_000));
    assert_eq!(backend.stored(id).unwrap().0, 70_000);

    // A potion stacks onto the ten (short form); a sword goes to the first empty slot with its quality.
    lay(Loot::Item { item_type: 25, type_id: 1, count: 1, quality: 1200, kept: None });
    lay(Loot::Item { item_type: 5, type_id: 1, count: 1, quality: 1100, kept: None });
    client.read().await.unwrap();
    client.read().await.unwrap();
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &2u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), gp::item_leave(2));
    let potions = w::ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 11, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), gp::picked_up(&potions));
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &3u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), gp::item_leave(3));
    let sword = w::ItemUnit { bag: 1, slot: 1, item_type: 5, type_id: 1, count: 1, quality: 1100, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), gp::picked_up(&sword));
    // Gone now: no answer.
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &3u32.to_le_bytes())).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));
    let (_, items) = backend.stored(id).unwrap();
    let mut slots: Vec<_> = items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count, i.quality)).collect();
    slots.sort();
    assert_eq!(slots, [(0, 5, 5, 1, 1200), (1, 0, 25, 11, 1200), (1, 1, 5, 1, 1100)]);

    // The game log: the gold with the money after; a monster's drops as 111 with the count, the potions on the ten's row.
    let gold = logged(&ctx, 161);
    assert_eq!(set_values(&gold[0]), [(1, 70_000), (2, 70_000)]);
    let gets = logged(&ctx, 111);
    assert_eq!(gets.len(), 2);
    assert_eq!(set_values(&gets[0])[1..], [(2, 25001), (3, 1)]);
    assert_eq!(set_values(&gets[1])[1..], [(2, 5001), (3, 1)]);
    assert_ne!(gets[0].values[0], gets[1].values[0], "two items, two rows");
    // An item dropped at a death is 118 with the dead player's id; a player's own drop is 111.
    ctx.zones.drop_on_ground(
        1,
        Loot::Item { item_type: 25, type_id: 1, count: 1, quality: 1200, kept: None },
        here,
        id,
        0,
        now,
        gp::KIND_PLAYER_DROP,
        game_server::zone::Dropped::death(9),
    );
    client.read().await.unwrap();
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &4u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), gp::item_leave(4));
    client.read().await.unwrap();
    let death = logged(&ctx, 118);
    assert_eq!((death.len(), set_values(&death[0])[2..].to_vec()), (1, vec![(3, 9)]));
    assert_eq!(logged(&ctx, 111).len(), 2, "the death drop is not a 111");
}

#[tokio::test]
async fn repairing_at_the_blacksmith() {
    use shaiya_proto::item as it;
    let backend = backend_with_session([5; 16]);
    let (addr, _ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    // The equipped sword (quality 1200, bought for 500) worn down to 1000, and 1000 gold; the blacksmith stands beside the start.
    backend.set_money(1, 1000);
    backend.set_quality(1, 0, 5, 1000);
    let id = select_and_enter(&mut client).await;
    let repair = |bag: u8, slot: u8| shaiya_proto::encode(it::OP_REPAIR, &[bag, slot]);

    // 200 short: 200 / 400 × 500 / 30 = 8.33, so 8.
    client.write(repair(0, 5)).await;
    assert_eq!(client.read().await.unwrap(), it::repaired(0, 5, 992));
    let (money, items) = backend.stored(id).unwrap();
    assert_eq!((money, items.iter().find(|i| (i.bag, i.slot) == (0, 5)).unwrap().quality), (992, 1200));
    // Everything with nothing worn still costs 1; an empty slot or a full item is ignored.
    client.write(repair(255, 255)).await;
    assert_eq!(client.read().await.unwrap(), it::repaired_all(991));
    client.write(repair(0, 6)).await;
    client.write(repair(0, 5)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));
    assert_eq!(backend.stored(id).unwrap().0, 991);
}

#[tokio::test]
async fn linking_at_the_blacksmith_and_cloak_badges() {
    use game_server::backend::{ItemStore, NewItem};
    use game_server::inventory::SlotChange;
    use shaiya_proto::gem;
    let backend = backend_with_session([5; 16]);
    let (addr, _ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    // Two lapis and a cloak in the bag, a million gold; the sword is equipped (bag 0 slot 5), the blacksmith beside the start.
    backend.set_money(1, 1_000_000);
    let lapis = NewItem { bag: 1, slot: 5, item_id: 30001, item_type: 30, type_id: 1, quality: 1200, count: 2, kept: None };
    let cloak = NewItem { bag: 1, slot: 6, item_id: 24001, item_type: 24, type_id: 1, quality: 1200, count: 1, kept: None };
    assert!(backend.trade(1, 0, &[SlotChange::Insert(lapis), SlotChange::Insert(cloak)]).await.unwrap().is_some());
    let id = select_and_enter(&mut client).await;

    // Linking into the sword: the lapis is used and the price paid whatever the roll; the gem is there only on success.
    client.write(shaiya_proto::encode(gem::OP_LINK, &[1, 5, 0, 5, 0, 0])).await;
    let answer = client.read().await.unwrap();
    let success = answer[4] == 1;
    let money = 1_000_000 - 4_095;
    let gem = if success { 1 } else { 0 };
    assert_eq!(answer, gem::linked(success, ((1, 5), 1), (0, 5), (0, 1), money, (0, 0)));
    let (stored, items) = backend.stored(id).unwrap();
    let sword = items.iter().find(|i| (i.bag, i.slot) == (0, 5)).unwrap();
    assert_eq!((stored, sword.gems[0]), (money, gem));
    // Nothing goes into a cloak (deviation 1): no answer.
    client.write(shaiya_proto::encode(gem::OP_LINK, &[1, 5, 1, 6, 0, 0])).await;
    // A badge for 450000, then off for nothing.
    client.write(shaiya_proto::encode(gem::OP_BADGE, &[1, 6, 1, 2, 3, 4, 5, 6])).await;
    assert_eq!(client.read().await.unwrap(), gem::badge_result(true, (1, 6), [1, 2, 3, 4, 5, 6], money - 450_000));
    let gems = |backend: &game_server::memory::MemoryBackend| {
        backend.stored(id).unwrap().1.iter().find(|i| (i.bag, i.slot) == (1, 6)).unwrap().gems
    };
    assert_eq!(gems(&backend), [1, 2, 3, 4, 5, 6]);
    client.write(shaiya_proto::encode(gem::OP_BADGE_REMOVE, &[1, 6])).await;
    assert_eq!(client.read().await.unwrap(), gem::badge_removed((1, 6)));
    assert_eq!(gems(&backend), [0; 6]);
    // Once more, then short of money: refused with the money.
    client.write(shaiya_proto::encode(gem::OP_BADGE, &[1, 6, 9, 9, 9, 9, 9, 9])).await;
    assert_eq!(client.read().await.unwrap(), gem::badge_result(true, (1, 6), [9; 6], money - 900_000));
    client.write(shaiya_proto::encode(gem::OP_BADGE, &[1, 6, 8, 8, 8, 8, 8, 8])).await;
    assert_eq!(client.read().await.unwrap(), gem::badge_result(false, (1, 6), [8; 6], money - 900_000));
    assert_eq!(gems(&backend), [9; 6]);
}

#[tokio::test]
async fn saved_positions_and_body_changes() {
    use game_server::backend::{Body, ItemStore, NewItem};
    use game_server::inventory::SlotChange;
    use shaiya_proto::item as it;
    let backend = backend_with_session([5; 16]);
    let (addr, _ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    // Two appearance scrolls in bag 1 slot 2, a sex change scroll in slot 3.
    let new = |slot, type_id: u8, count| {
        SlotChange::Insert(NewItem {
            bag: 1,
            slot,
            item_id: 100_000 + u32::from(type_id),
            item_type: 100,
            type_id,
            quality: 0,
            count,
            kept: None,
        })
    };
    assert!(backend.trade(1, 0, &[new(2, 70, 2), new(3, 71, 1)]).await.unwrap().is_some());
    let (here, sex) = backend.play(1).map(|p| (p.position, p.sex)).unwrap();
    let id = select_and_enter(&mut client).await;

    // Saving: index 1 near where the player stands keeps the server's position; a bad index (2), another map or too far (1) are
    // refused with zeros.
    let save = |index: u8, map: u16, at: [f32; 3]| {
        let mut p = vec![index];
        p.extend_from_slice(&map.to_le_bytes());
        at.iter().for_each(|v| p.extend_from_slice(&v.to_le_bytes()));
        shaiya_proto::encode(it::OP_SAVE_POINT, &p)
    };
    let near = [here[0] + 3.0, here[1], here[2] - 4.0];
    client.write(save(1, 1, near)).await;
    assert_eq!(client.read().await.unwrap(), it::save_point_result(0, it::SavePos { index: 1, map: 1, position: here }));
    for (index, map, at, result) in [(5, 1, near, 2), (2, 2, near, 1), (2, 1, [here[0] + 51.0, here[1], here[2]], 1)] {
        client.write(save(index, map, at)).await;
        assert_eq!(client.read().await.unwrap(), it::save_point_result(result, it::SavePos { index, map: 0, position: [0.0; 3] }));
    }
    let saved = backend.play(id).unwrap().save_points;
    assert_eq!(saved.len(), 1);
    assert_eq!((saved[0].slot, saved[0].map, saved[0].position), (0, 1, here));

    // The appearance scroll: hair, face and size, the sex kept; one scroll used.
    let change = |slot: u8, looks: [u8; 4]| shaiya_proto::encode(it::OP_BODY_CHANGE, &[1, slot, looks[0], looks[1], looks[2], looks[3]]);
    client.write(change(2, [3, 4, 5, 1 - sex])).await;
    assert_eq!(client.read().await.unwrap(), it::used(id, 1, 2, 100, 70, 1));
    assert_eq!(client.read().await.unwrap(), it::body_changed(id, [3, 4, 5, sex]));
    // The sex change scroll: sex 7 is refused (deviation 1), the other sex taken.
    client.write(change(3, [6, 7, 8, 7])).await;
    assert_eq!(client.read().await.unwrap(), it::use_failed(id));
    client.write(change(3, [6, 7, 8, 1 - sex])).await;
    assert_eq!(client.read().await.unwrap(), it::used(id, 1, 3, 100, 71, 0));
    assert_eq!(client.read().await.unwrap(), it::body_changed(id, [6, 7, 8, 1 - sex]));
    let play = backend.play(id).unwrap();
    assert_eq!(Body { hair: play.hair, face: play.face, size: play.size, sex: play.sex }, Body { hair: 6, face: 7, size: 8, sex: 1 - sex });
    assert!(play.items.iter().all(|i| (i.bag, i.slot) != (1, 3)), "the sex change scroll is gone");
    // A non-scroll (the potions) gets the item use error; the equipment is ignored.
    client.write(change(0, [1, 1, 1, 0])).await;
    assert_eq!(client.read().await.unwrap(), it::use_failed(id));
}

#[tokio::test]
async fn summoning_a_mount_and_getting_off() {
    use game_server::backend::{ItemStore, NewItem};
    use game_server::inventory::SlotChange;
    use shaiya_proto::{skill as sk, vehicle as vp};
    let backend = backend_with_session([5; 16]);
    let (addr, _ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    let mount = NewItem { bag: 0, slot: 13, item_id: 42001, item_type: 42, type_id: 1, quality: 0, count: 1, kept: None };
    assert!(backend.trade(1, 0, &[SlotChange::Insert(mount)]).await.unwrap().is_some());
    let id = select_and_enter(&mut client).await;
    let use_vehicle = || shaiya_proto::encode(vp::OP_USE, &[]);

    // Summoning starts (0x0217); asking again cancels it: 0x0216 1, 0 and the shape back to 0.
    client.write(use_vehicle()).await;
    assert_eq!(client.read().await.unwrap(), vp::ready(id));
    client.write(use_vehicle()).await;
    assert_eq!(client.read().await.unwrap(), vp::used(vp::GOT_OFF));
    assert_eq!(client.read().await.unwrap(), sk::shape(id, 0));
    // Sitting, or with the mount taken off, it is refused.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_SIT])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, w::MOTION_SIT));
    client.write(use_vehicle()).await;
    assert_eq!(client.read().await.unwrap(), vp::used(vp::SUMMON_REFUSED));
    client.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_STAND])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, w::MOTION_STAND));
    client.write(shaiya_proto::encode(w::OP_ITEM_MOVE, &[0, 13, 1, 5])).await;
    while frame::opcode(&client.read().await.unwrap()) != w::OP_ITEM_MOVE {}
    client.write(use_vehicle()).await;
    assert_eq!(client.read().await.unwrap(), vp::used(vp::SUMMON_REFUSED));
}

#[tokio::test]
async fn dropping_items_on_the_ground() {
    use shaiya_proto::ground as gp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    // Walker: the sword in bag 0 slot 5, ten potions in bag 1 slot 0.
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let drop = |bag: u8, slot: u8, count: u8| shaiya_proto::encode(gp::OP_ITEM_DROP, &[bag, slot, count]);
    // `0x0401` of kind 1 at the player's feet: id, kind, type, type id, count.
    let landed = |packet: Vec<u8>| -> [u8; 8] {
        assert_eq!(frame::opcode(&packet), gp::OP_ITEM_ENTER);
        packet[4..12].try_into().unwrap()
    };

    // The equipment, more than the slot holds and count 0 are ignored without an answer.
    client.write(drop(0, 5, 1)).await;
    client.write(drop(1, 0, 11)).await;
    client.write(drop(1, 0, 0)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));

    // Four of the ten: the slot keeps six.
    client.write(drop(1, 0, 4)).await;
    assert_eq!(landed(client.read().await.unwrap()), [1, 0, 0, 0, gp::KIND_PLAYER_DROP, 25, 1, 4]);
    let six = w::ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 6, ..Default::default() };
    assert_eq!(client.read().await.unwrap(), gp::item_left(1, 0, Some(&six)));
    assert_eq!(backend.stored(id).unwrap().1.iter().find(|i| (i.bag, i.slot) == (1, 0)).unwrap().count, 6);

    // The dropper takes them back at once: onto the six.
    client.write(shaiya_proto::encode(gp::OP_PICK_UP, &1u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), gp::item_leave(1));
    assert_eq!(frame::opcode(&client.read().await.unwrap()), gp::OP_PICK_UP);

    // All ten: the slot empties.
    client.write(drop(1, 0, 10)).await;
    assert_eq!(landed(client.read().await.unwrap()), [2, 0, 0, 0, gp::KIND_PLAYER_DROP, 25, 1, 10]);
    assert_eq!(client.read().await.unwrap(), gp::item_left(1, 0, None));
    let (_, items) = backend.stored(id).unwrap();
    assert!(items.iter().all(|i| (i.bag, i.slot) != (1, 0)), "{items:?}");

    // The game log: two drops (112) with the count in the last text; the potions' row is the same in both and in the pick-up
    // between them (111, a player's own drop), which topped the six up by four.
    let drops = logged(&ctx, 112);
    assert_eq!(drops.len(), 2);
    assert_eq!((drops[0].texts[3].as_deref(), drops[1].texts[3].as_deref()), (Some("bag=1,slot=0,Count=4"), Some("bag=1,slot=0,Count=10")));
    let picked = logged(&ctx, 111);
    assert_eq!((picked.len(), set_values(&picked[0])[2..].to_vec()), (1, vec![(3, 4)]));
    assert_eq!(drops[0].values[0], picked[0].values[0]);
    assert_eq!(drops[0].values[0], drops[1].values[0]);
}

#[tokio::test]
async fn a_lost_connection_leaves_the_character_in_the_world_for_a_while() {
    let backend = backend_with_session([5; 16]);
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut walker = Client::connect(addr).await;
    let walker_id = enter_world(&mut walker, 7, [5; 16], true).await;
    let mut watcher = Client::connect(addr).await;
    watcher.enter(8, [6; 16]).await;
    watcher.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    watcher.write(create(0, 0, 0, b"Watcher")).await;
    watcher.read().await.unwrap();
    let watcher_id = u32::from_le_bytes(watcher.read_list().await[4][1..5].try_into().unwrap());
    watcher.write(g::char_id_request(g::OP_CHAR_SELECT, watcher_id)).await;
    while frame::opcode(&watcher.read().await.unwrap()) != w::OP_CHAR_QUICK {}
    watcher.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    assert_eq!(read_zone_entry(&mut watcher).await.0.len(), 2, "the watcher sees itself and the walker");

    // The walker's client goes away without logging out: the character stays (the test setting: 300 ms), and a new connection
    // of the account waits for it to go, its session left unused meanwhile (docs/original/client-extras.md deviation 3).
    drop(walker);
    let gone = std::time::Instant::now();
    backend.add_session(7, session([9; 16], Some(SERVER_ID), LOCAL));
    let mut again = Client::connect(addr).await;
    again.write(g::game_connect_request(7, &[9; 16])).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!backend.session(7).unwrap().in_game, "the waiting handshake has not used the session");
    assert_eq!(ctx.players.load(Ordering::Relaxed), 2, "still counted");
    loop {
        let packet = watcher.read().await.expect("the walker leaves");
        if packet == w::user_leave(walker_id) {
            break;
        }
    }
    assert!(gone.elapsed() >= Duration::from_millis(250), "left after the stay, not at once: {:?}", gone.elapsed());

    // Once it is gone, the waiting connection goes on to the character screen.
    again.finish_enter().await;

    // A connection that keeps its account is refused once the wait is over (the test setting: 1 s).
    backend.add_session(7, session([10; 16], Some(SERVER_ID), LOCAL));
    let mut third = Client::connect(addr).await;
    let asked = std::time::Instant::now();
    third.write(g::game_connect_request(7, &[10; 16])).await;
    assert_eq!(third.read().await, None);
    assert!(asked.elapsed() >= Duration::from_millis(900), "after the wait: {:?}", asked.elapsed());
}

#[tokio::test]
async fn a_newer_login_ends_the_older_connection() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut old = Client::connect(addr).await;
    enter_world(&mut old, 7, [5; 16], true).await;

    // A notification for the session the connection has (or an older one) ends nothing.
    assert!(!ctx.online.kick_older(7, 1), "the same session");
    assert!(!ctx.online.kick_older(7, 0), "an older one");
    assert!(!ctx.online.kick_older(8, 2), "another account");

    // The account logs in again: 0x0004 0, and the server closes (docs/original/client-extras.md section 3.2, deviation 2).
    backend.add_session(7, StoredSession { created: 2, ..session([6; 16], Some(SERVER_ID), LOCAL) });
    let asked = std::time::Instant::now();
    assert!(ctx.online.kick_older(7, 2));
    loop {
        let packet = old.read().await.expect("the kick");
        if frame::opcode(&packet) == g::OP_KICKED {
            assert_eq!(frame::payload(&packet), [g::KICKED_LOGGED_IN]);
            break;
        }
    }
    assert_eq!(old.read().await, None, "closed after it");
    assert!(asked.elapsed() < config(true).kick_notice, "at once, with no notice hold: {:?}", asked.elapsed());
    assert!(ctx.online.contains(7), "the character stays a while");

    // The new login's connection waits for it and gets in; the old client's socket is still open and is drained meanwhile.
    let mut new = Client::connect(addr).await;
    new.write(g::game_connect_request(7, &[6; 16])).await;
    new.finish_enter().await;
    assert!(!ctx.online.kick_older(7, 2), "the new connection is not the old one");
    drop(old);
}

#[tokio::test]
async fn leaving_dead_comes_back_to_life_in_what_is_saved() {
    use game_server::mobs::MobAttack;
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    // A monster beside the start that survives the sword and kills with one blow.
    let mut data = data();
    let mut attacks = [MobAttack::default(); 3];
    attacks[0] = MobAttack { ok: 1, damage: 30000, range: 10, ..Default::default() };
    set_mobs(
        &mut data,
        MobDefs::new([MobDef {
            id: 1,
            level: 1,
            hp: 100_000,
            respawn: Duration::ZERO,
            country: 2,
            size: 1,
            chase_step: 8,
            normal_step: 4,
            chase_time: Duration::from_secs(60),
            chase_range: 20,
            attacks,
            ..Default::default()
        }]),
    );
    let ctx = Arc::new(Context::new(config(true), backend.clone(), data));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx.clone()));

    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let full = backend.play(id).unwrap().health;
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    client.write(step(598.0, 1779.0)).await;
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    // Hit (roll 0), no critical (99); the monster answers every pass until the player dies.
    let live = ctx.data.live();
    let loot = (&live.drops, &ctx.data.items);
    let mut rolls = [0, 99].into_iter().cycle();
    let mut later = std::time::Instant::now();
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        later += Duration::from_secs(5);
        ctx.zones.mob_fight(later, &ctx.data.world, &ctx.data.live().mobs, loot, &mut || rolls.next().unwrap());
        if ctx.zones.health(1, id) == Some([0; 3]) {
            break;
        }
    }
    assert_eq!(ctx.zones.health(1, id), Some([0; 3]), "killed by the monster: {:?}", ctx.zones.mob(1, 1).map(|m| m.state));
    while frame::opcode(&client.read().await.expect("the death")) != cp::OP_DEATH {}

    // Gone while dead: no stay, saved alive with HP, SP and MP full, and the account may connect again at once.
    drop(client);
    wait_gone(&backend, 7, Duration::from_millis(200)).await;
    let saved = backend.play(id).unwrap();
    assert_eq!((saved.health, saved.map), (full, 1));
    backend.add_session(7, session([9; 16], Some(SERVER_ID), LOCAL));
    let mut back = Client::connect(addr).await;
    back.enter(7, [9; 16]).await;
}

/// Test data with a monster beside the start that survives the sword and kills with one blow, and the two resurrection runes
/// (`docs/original/item-flags.md` section 7): the small one (100/81, VG 1) and the full one (100/44, VG 0).
fn deadly_data() -> GameData {
    use game_server::mobs::MobAttack;
    let mut data = data();
    let mut attacks = [MobAttack::default(); 3];
    attacks[0] = MobAttack { ok: 1, damage: 30000, range: 10, ..Default::default() };
    set_mobs(
        &mut data,
        MobDefs::new([MobDef {
            id: 1,
            level: 1,
            hp: 100_000,
            respawn: Duration::ZERO,
            country: 2,
            size: 1,
            chase_step: 8,
            normal_step: 4,
            chase_time: Duration::from_secs(60),
            chase_range: 20,
            attacks,
            ..Default::default()
        }]),
    );
    let stone = *data.items.get(100, 48).unwrap();
    let small = ItemDef { item_id: 100081, type_id: 81, special: 37, vg: 1, max_count: 10, ..stone };
    let full = ItemDef { item_id: 100044, type_id: 44, special: 37, vg: 0, max_count: 10, ..stone };
    data.items = ItemDefs::new(data.items.iter().cloned().chain([small, full]));
    data
}

/// The player attacks the deadly monster until it kills the player; returns once `0x0504` arrived.
async fn die(client: &mut Client, ctx: &Context<Arc<MemoryBackend>>, id: u32) {
    use shaiya_proto::combat as cp;
    if ctx.zones.mob(1, 1).is_none() {
        ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    }
    client.write(step(598.0, 1779.0)).await;
    let attack = shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes());
    client.write(attack.clone()).await;
    let live = ctx.data.live();
    let loot = (&live.drops, &ctx.data.items);
    let mut rolls = [0, 99].into_iter().cycle();
    // A monster going home after the last kill cannot be hit, and the swing that found it so may not lead to a fight; while the
    // monster is not fighting this player the attack is sent again, as a player clicks again. The wait is real time, not a count of
    // passes, so a slow machine (a busy CI runner) gets as long as it needs.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last_attack = std::time::Instant::now();
    let mut later = std::time::Instant::now();
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let fighting = ctx.zones.mob(1, 1).is_some_and(|m| m.state == game_server::mobs::MobState::Fighting(id));
        if !fighting && last_attack.elapsed() >= Duration::from_millis(500) {
            client.write(attack.clone()).await;
            last_attack = std::time::Instant::now();
        }
        later += Duration::from_secs(5);
        ctx.zones.mob_fight(later, &ctx.data.world, &ctx.data.live().mobs, loot, &mut || rolls.next().unwrap());
        if ctx.zones.health(1, id) == Some([0; 3]) {
            break;
        }
    }
    assert_eq!(ctx.zones.health(1, id), Some([0; 3]), "killed by the monster: {:?}", ctx.zones.mob(1, 1).map(|m| m.state));
    read_op(client, cp::OP_DEATH).await;
}

#[tokio::test]
async fn the_death_windows_resurrection_runes() {
    use game_server::backend::{ItemStore, NewItem};
    use game_server::inventory::SlotChange;
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    let mut ctx = Context::new(config(true), backend.clone(), deadly_data());
    ctx.set_game_log(game_server::game_log::GameLog::memory(true).0);
    let ctx = Arc::new(ctx);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx.clone()));
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Walker")).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    // The small rune first in the bags, the full one after it.
    let rune = |slot, type_id: u8| {
        SlotChange::Insert(NewItem {
            bag: 1,
            slot,
            item_id: 100_000 + u32::from(type_id),
            item_type: 100,
            type_id,
            quality: 0,
            count: 1,
            kept: None,
        })
    };
    assert!(backend.trade(1, 0, &[rune(2, 81), rune(3, 44)]).await.unwrap().is_some());
    let id = select_and_enter(&mut client).await;
    let full = backend.play(id).unwrap().health;
    let half = full.map(|v| v / 2);
    let at = |p: &[u8], i: usize| f32::from_le_bytes(p[i..i + 4].try_into().unwrap());

    // Kind 5: the full rune (VG 0) although the small one comes first; where the player lies (kind 1), at half.
    die(&mut client, &ctx, id).await;
    client.write(shaiya_proto::encode(cp::OP_REBIRTH_REQUEST, &[5])).await;
    assert_eq!(read_op(&mut client, it::OP_USE).await, it::used(id, 1, 3, 100, 44, 0));
    let back = read_op(&mut client, cp::OP_REBIRTH).await;
    let p = frame::payload(&back);
    assert_eq!(p[4], cp::REBIRTH_HERE);
    assert_eq!((at(p, 9), at(p, 17)), (598.0, 1779.0), "where it lies, at the step it made");
    assert_eq!(backend.play(id).unwrap().health, half, "back at half");

    // Kind 5 again: no rune of its own kind left, so the small one; still in place.
    die(&mut client, &ctx, id).await;
    client.write(shaiya_proto::encode(cp::OP_REBIRTH_REQUEST, &[5])).await;
    assert_eq!(read_op(&mut client, it::OP_USE).await, it::used(id, 1, 2, 100, 81, 0));
    let back = read_op(&mut client, cp::OP_REBIRTH).await;
    assert_eq!(frame::payload(&back)[4], cp::REBIRTH_HERE);
    assert_eq!(backend.play(id).unwrap().health, half);

    // Kind 4 with no rune left: as kind 2, at the bind position, full.
    die(&mut client, &ctx, id).await;
    client.write(shaiya_proto::encode(cp::OP_REBIRTH_REQUEST, &[4])).await;
    let back = read_op(&mut client, cp::OP_REBIRTH).await;
    assert_eq!(frame::payload(&back)[4], cp::REBIRTH_BIND);
    assert_eq!(backend.play(id).unwrap().health, full);

    // The game log: three deaths by the monster (301, V1 its id), each followed by the revival (105 and 106), the ways `sel_itembig`,
    // `sel_itembig` and, with no rune left, `sel_base`.
    let deaths = logged(&ctx, 102);
    assert_eq!(deaths.len(), 3);
    assert_eq!(set_values(&deaths[0])[0], (1, i64::from(ctx.zones.mob(1, 1).map_or(0, |m| m.mob_id))));
    assert_eq!(logged(&ctx, 105).len(), 3);
    let ways: Vec<_> = logged(&ctx, 106).iter().map(|r| (set_values(r), r.texts[0].clone().unwrap())).collect();
    assert_eq!(
        ways,
        [
            (vec![(1, 0), (2, 4)], "sel_itembig".to_owned()),
            (vec![(1, 0), (2, 4)], "sel_itembig".to_owned()),
            (vec![(1, 0), (2, 1)], "sel_base".to_owned()),
        ]
    );
    // The two runes used up are 112 with their own rows ('use_item', 040E): one of each.
    let used = logged(&ctx, 112);
    assert_eq!(used.iter().map(|r| r.texts[1].as_deref()).collect::<Vec<_>>(), [Some("use_item"), Some("use_item")]);
    assert_ne!(used[0].values[0], used[1].values[0]);
}

#[tokio::test]
async fn an_ultimate_death_without_a_rune_falls() {
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    let ctx = Arc::new(Context::new(config(true), backend.clone(), deadly_data()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx.clone()));
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    let ultimate =
        CreateRequest { slot: 0, family: 0, grow: 3, hair: 3, face: 4, size: 5, job: 0, sex: 1, name: b"Doomed".to_vec(), sent_len: 0 };
    client.write(g::char_create_request(&ultimate)).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    client.read_list().await;
    let id = select_and_enter(&mut client).await;
    assert_eq!(backend.play(id).unwrap().grow, 3);

    // Back to life asked: no rune, no Continuous Res Rune: fallen, back to the character screen without it.
    die(&mut client, &ctx, id).await;
    client.write(shaiya_proto::encode(cp::OP_REBIRTH_REQUEST, &[2])).await;
    // A motion on the way does not hold it in the world.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    read_op(&mut client, w::OP_LOGOUT_TO_SELECT).await;
    assert_eq!(client.read().await.unwrap(), g::char_country(0, 3));
    let list = client.read_list().await;
    assert!(list.iter().all(|slot| !slot.windows(6).any(|w| w == b"Doomed")), "the fallen character is not listed");
    assert!(backend.is_deleted(id) && backend.play(id).is_some(), "out of the list, kept in the store");
}

/// The next packet with `opcode`, skipping others (status passes and the like).
async fn read_op(client: &mut Client, opcode: u16) -> Vec<u8> {
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == opcode {
            return packet;
        }
    }
}

#[tokio::test]
async fn two_players_make_a_party_talk_and_part() {
    use shaiya_proto::party as pp;
    let backend = Arc::new(MemoryBackend::default());
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, _) = start(backend, true).await;
    let mut alice = Client::connect(addr).await;
    alice.enter(7, [5; 16]).await;
    alice.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    alice.write(create(0, 0, 0, b"Alice")).await;
    alice.read().await.unwrap();
    let alice_id = u32::from_le_bytes(alice.read_list().await[4][1..5].try_into().unwrap());
    let mut bob = Client::connect(addr).await;
    bob.enter(8, [6; 16]).await;
    bob.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    bob.write(create(0, 0, 0, b"Bob")).await;
    bob.read().await.unwrap();
    let bob_id = u32::from_le_bytes(bob.read_list().await[4][1..5].try_into().unwrap());
    for (client, id) in [(&mut alice, alice_id), (&mut bob, bob_id)] {
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
        client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
        read_zone_entry(client).await;
    }

    // Alice invites Bob; Bob says yes (0, his own id).
    alice.write(shaiya_proto::encode(pp::OP_INVITE, &bob_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut bob, pp::OP_INVITE).await, pp::invitation(alice_id));
    let mut yes = vec![0];
    yes.extend_from_slice(&bob_id.to_le_bytes());
    bob.write(shaiya_proto::encode(pp::OP_ANSWER, &yes)).await;
    assert_eq!(read_op(&mut alice, pp::OP_LIST).await, pp::list(0, &[]), "a new party: nobody else yet");
    let entered = read_op(&mut alice, pp::OP_ENTER).await;
    assert_eq!(frame::payload(&entered)[..4], bob_id.to_le_bytes());
    assert_eq!(&frame::payload(&entered)[4..8], b"Bob\0");
    let list = read_op(&mut bob, pp::OP_LIST).await;
    assert_eq!(frame::payload(&list)[..6], [0, 1, alice_id as u8, (alice_id >> 8) as u8, (alice_id >> 16) as u8, (alice_id >> 24) as u8]);
    assert_eq!(read_op(&mut bob, pp::OP_PARTY_TYPE).await, pp::party_type(bob_id, pp::TYPE_MEMBER));

    // Party chat reaches both, sender included.
    bob.write(shaiya_proto::encode(chat::OP_PARTY, &[3, b'y', b'o', 0])).await;
    let said = chat::by_id(chat::OP_PARTY, bob_id, b"yo\0");
    assert_eq!(read_op(&mut alice, chat::OP_PARTY).await, said);
    assert_eq!(read_op(&mut bob, chat::OP_PARTY).await, said);

    // Bob leaves: both hear it, and Alice, alone, leaves too.
    bob.write(shaiya_proto::encode(pp::OP_LEAVE, &[])).await;
    assert_eq!(read_op(&mut bob, pp::OP_LEAVE).await, pp::left(bob_id));
    assert_eq!(read_op(&mut alice, pp::OP_LEAVE).await, pp::left(bob_id));
    assert_eq!(read_op(&mut alice, pp::OP_LEAVE).await, pp::left(alice_id));

    // A union request is ignored, not a reason to close the connection.
    alice.write(shaiya_proto::encode(0x0B0E, &[1, 0, 1, 0, 0, 0])).await;
    alice.write(shaiya_proto::encode(chat::OP_NORMAL, &[3, b'o', b'k', 0])).await;
    assert_eq!(read_op(&mut alice, chat::OP_NORMAL).await, chat::by_id(chat::OP_NORMAL, alice_id, b"ok\0"));
}

/// Two human fighters on two accounts, side by side on map 1, in the world: (Alice, her id, Bob, his id).
async fn two_in_the_world(backend: &Arc<MemoryBackend>, addr: SocketAddr) -> (Client, u32, Client, u32) {
    two_in_the_world_with(backend, addr, false).await
}

/// `two_in_the_world`, Bob's receive buffer small when `bob_small` (see `Client::connect_small`).
async fn two_in_the_world_with(backend: &Arc<MemoryBackend>, addr: SocketAddr, bob_small: bool) -> (Client, u32, Client, u32) {
    two_in_the_world_of(backend, addr, bob_small, [0, 0]).await
}

/// `two_in_the_world_with`, Alice and Bob of the given factions (0 Light, 1 Fury).
async fn two_in_the_world_of(backend: &Arc<MemoryBackend>, addr: SocketAddr, bob_small: bool, countries: [u8; 2]) -> (Client, u32, Client, u32) {
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let mut clients = Vec::new();
    for (user, identity, name) in [(7, [5; 16], &b"Alice"[..]), (8, [6; 16], &b"Bob"[..])] {
        let mut client = if bob_small && user == 8 { Client::connect_small(addr).await } else { Client::connect(addr).await };
        client.enter(user, identity).await;
        let country = countries[usize::from(user == 8)];
        client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[country])).await;
        // A human fighter for Light, a Deatheater fighter for Fury (races 0 and 3).
        client.write(create(0, if country == 0 { 0 } else { 3 }, 0, name)).await;
        client.read().await.unwrap();
        let id = u32::from_le_bytes(client.read_list().await[4][1..5].try_into().unwrap());
        clients.push((client, id));
    }
    let (mut bob, bob_id) = clients.pop().unwrap();
    let (mut alice, alice_id) = clients.pop().unwrap();
    backend.set_money(alice_id, 1000);
    if countries[0] != countries[1] {
        // The factions start apart: Bob is put beside Alice's start.
        backend.set_position(bob_id, 1, [580.0, 78.56, 1762.12]);
    }
    for (client, id) in [(&mut alice, alice_id), (&mut bob, bob_id)] {
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
    }
    alice.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(&mut alice).await;
    bob.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(&mut bob).await;
    // Alice is told of Bob (0x0201 and his speed).
    alice.read().await.unwrap();
    alice.read().await.unwrap();
    (alice, alice_id, bob, bob_id)
}

/// The next packet that is not someone's step or looks (the zone's own traffic).
async fn next_trade_packet(client: &mut Client) -> Vec<u8> {
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) >> 8 == 0x0A || [gp::OP_PICK_UP, gp::OP_ITEM_DROP].contains(&frame::opcode(&packet)) {
            return packet;
        }
    }
}

#[tokio::test]
async fn two_players_trade_items_and_money() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(&backend, addr).await;

    // Alice takes her sword off into bag 1 slot 2 (before trading: moving an item ends a trade).
    alice.write(shaiya_proto::encode(w::OP_ITEM_MOVE, &[0, 5, 1, 2])).await;
    while frame::opcode(&alice.read().await.unwrap()) != w::OP_ITEM_MOVE {}

    // Alice asks, Bob says yes: both windows open, each with the other's id.
    alice.write(shaiya_proto::encode(tp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::request(alice_id));
    bob.write(shaiya_proto::encode(tp::OP_ANSWER, &[tp::ANSWER_YES])).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::start(alice_id, tp::START_OK));
    assert_eq!(next_trade_packet(&mut alice).await, tp::start(bob_id, tp::START_OK));
    // A third request while trading: busy.
    alice.write(shaiya_proto::encode(tp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::start(bob_id, tp::START_BUSY));

    // Bob is ready first; Alice then offers 4 of her 10 potions: Bob's ready is cancelled before he sees the offer.
    bob.write(shaiya_proto::encode(tp::OP_READY, &[tp::READY])).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::ready(tp::WHO_ME, false));
    assert_eq!(next_trade_packet(&mut alice).await, tp::ready(tp::WHO_PARTNER, false));
    // Neither an equipped item nor her sword (bound) can be offered: nothing comes of either.
    alice.write(shaiya_proto::encode(tp::OP_ADD, &[0, 7, 1, 1])).await;
    alice.write(shaiya_proto::encode(tp::OP_ADD, &[1, 2, 1, 1])).await;
    alice.write(shaiya_proto::encode(tp::OP_ADD, &[1, 0, 4, 0])).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::ready(tp::WHO_BOTH, true));
    let offer = tp::Offer { bag: 1, slot: 0, count: 4, trade_slot: 0 };
    assert_eq!(next_trade_packet(&mut alice).await, tp::added(&offer));
    assert_eq!(next_trade_packet(&mut bob).await, tp::ready(tp::WHO_BOTH, true));
    let potion = w::ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 10, quality: 1200, ..Default::default() };
    assert_eq!(next_trade_packet(&mut bob).await, tp::partner_added(0, &potion, 4));
    // More money than she has is ignored; 300 is shown to both.
    alice.write(shaiya_proto::encode(tp::OP_MONEY, &1001u32.to_le_bytes())).await;
    alice.write(shaiya_proto::encode(tp::OP_MONEY, &300u32.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::money(tp::WHO_ME, 300));
    assert_eq!(next_trade_packet(&mut bob).await, tp::money(tp::WHO_PARTNER, 300));

    // Both ready, Alice last: the potions go to Bob's first empty slot, part of her stack; the money with them.
    bob.write(shaiya_proto::encode(tp::OP_READY, &[tp::READY])).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::ready(tp::WHO_ME, false));
    assert_eq!(next_trade_packet(&mut alice).await, tp::ready(tp::WHO_PARTNER, false));
    alice.write(shaiya_proto::encode(tp::OP_READY, &[tp::READY])).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::ready(tp::WHO_ME, false));
    assert_eq!(next_trade_packet(&mut alice).await, gp::item_left(1, 0, Some(&w::ItemUnit { count: 6, ..potion.clone() })));
    assert_eq!(next_trade_packet(&mut alice).await, tp::end(tp::END_DONE));
    assert_eq!(next_trade_packet(&mut bob).await, tp::ready(tp::WHO_PARTNER, false));
    assert_eq!(next_trade_packet(&mut bob).await, gp::picked_up(&w::ItemUnit { slot: 1, count: 4, ..potion.clone() }));
    assert_eq!(next_trade_packet(&mut bob).await, tp::end(tp::END_DONE));

    let slots = |id| {
        let (money, items) = backend.stored(id).unwrap();
        let mut slots: Vec<_> = items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
        slots.sort();
        (money, slots)
    };
    assert_eq!(slots(alice_id), (700, vec![(1, 0, 25, 6), (1, 2, 5, 1)]));
    assert_eq!(slots(bob_id), (300, vec![(0, 5, 5, 1), (1, 0, 25, 10), (1, 1, 25, 4)]));

    // The game log: Alice gave four potions (116, her bag place) and Bob got them (115) as a new row (part of a stack gets a new unique
    // id, and Alice's row keeps its own); the gold: Alice's 304 (164), Bob's 303 (163).
    let (gave, got) = (logged(&ctx, 116), logged(&ctx, 115));
    assert_eq!((gave.len(), got.len()), (1, 1));
    let (gave_values, got_values) = (set_values(&gave[0]), set_values(&got[0]));
    assert_eq!(gave_values[1..], [(2, 25001), (3, i64::from(bob_id)), (4, 4), (5, 0)]);
    assert_eq!(got_values[1..], [(2, 25001), (3, i64::from(alice_id)), (4, 4), (5, 0)]);
    assert_ne!(gave_values[0], got_values[0], "the copy has a new unique id");
    assert_eq!((gave[0].header.character_id, got[0].header.character_id), (alice_id, bob_id));
    assert_eq!((gave[0].texts[1].as_deref(), got[0].texts[1].as_deref()), (Some("Bob"), Some("Alice")));
    assert_eq!(gave[0].texts[3].as_deref(), Some("bag=1,slot=0"));
    let (given_gold, received_gold) = (logged(&ctx, 164), logged(&ctx, 163));
    assert_eq!(set_values(&given_gold[0]), [(1, 300), (2, 700), (3, i64::from(bob_id))]);
    assert_eq!(set_values(&received_gold[0]), [(1, 300), (2, 300), (3, i64::from(alice_id))]);

    // A second trade, cancelled by Bob closing the window: 2 for him, 3 for her.
    bob.write(shaiya_proto::encode(tp::OP_REQUEST, &alice_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::request(bob_id));
    alice.write(shaiya_proto::encode(tp::OP_ANSWER, &[tp::ANSWER_YES])).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::start(bob_id, tp::START_OK));
    assert_eq!(next_trade_packet(&mut bob).await, tp::start(alice_id, tp::START_OK));
    bob.write(shaiya_proto::encode(tp::OP_READY, &[tp::CANCEL])).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::end(tp::END_CANCELLED));
    assert_eq!(next_trade_packet(&mut alice).await, tp::end(tp::END_CANCELLED_BY_PARTNER));

    // A third, refused: Bob is told with his own id, as the original does.
    bob.write(shaiya_proto::encode(tp::OP_REQUEST, &alice_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::request(bob_id));
    alice.write(shaiya_proto::encode(tp::OP_ANSWER, &[1])).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::start(bob_id, tp::START_REFUSED));

    // A fourth, ended by Alice stepping more than 10 away.
    bob.write(shaiya_proto::encode(tp::OP_REQUEST, &alice_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::request(bob_id));
    alice.write(shaiya_proto::encode(tp::OP_ANSWER, &[tp::ANSWER_YES])).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::start(bob_id, tp::START_OK));
    assert_eq!(next_trade_packet(&mut bob).await, tp::start(alice_id, tp::START_OK));
    alice.write(step(600.0, 1762.0)).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::end(tp::END_CANCELLED));
    assert_eq!(next_trade_packet(&mut bob).await, tp::end(tp::END_CANCELLED_BY_PARTNER));
}

/// Alice and Bob side by side with an open duel stakes window: (Alice, her id, Bob, his id).
async fn open_stakes_window(backend: &Arc<MemoryBackend>, addr: SocketAddr) -> (Client, u32, Client, u32) {
    use shaiya_proto::duel as dp;
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(backend, addr).await;
    alice.write(shaiya_proto::encode(dp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut bob, dp::OP_REQUEST).await, dp::request(alice_id, bob_id));
    bob.write(shaiya_proto::encode(dp::OP_ANSWER, &[dp::ACCEPTED])).await;
    assert_eq!(read_op(&mut alice, dp::OP_WINDOW).await, dp::window(bob_id));
    assert_eq!(read_op(&mut bob, dp::OP_WINDOW).await, dp::window(alice_id));
    (alice, alice_id, bob, bob_id)
}

#[tokio::test]
async fn staking_an_item_and_moving_it_closes_the_stakes_window_and_nothing_is_taken() {
    use shaiya_proto::duel as dp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, bob_id) = open_stakes_window(&backend, addr).await;
    let before = (backend.stored(alice_id).unwrap(), backend.stored(bob_id).unwrap());

    // Alice stakes 4 of her 10 potions and 300 gold (audit R3: the sequence of a free stake).
    let offer = tp::Offer { bag: 1, slot: 0, count: 4, trade_slot: 0 };
    alice.write(shaiya_proto::encode(dp::OP_STAKE_ADD, &[1, 0, 4, 0])).await;
    assert_eq!(read_op(&mut alice, dp::OP_STAKE_ADD).await, dp::staked(&offer));
    alice.write(shaiya_proto::encode(dp::OP_STAKE_MONEY, &300u32.to_le_bytes())).await;
    assert_eq!(read_op(&mut alice, dp::OP_STAKE_MONEY).await, dp::stake_money(dp::WHO_ME, 300));

    // She moves the potions to another slot: the window closes for both, as every item action ends a trade.
    alice.write(shaiya_proto::encode(w::OP_ITEM_MOVE, &[1, 0, 1, 5])).await;
    assert_eq!(read_op(&mut alice, dp::OP_WINDOW_CLOSED).await, dp::window_closed(dp::WINDOW_CLOSED_BY_ME));
    assert_eq!(read_op(&mut bob, dp::OP_WINDOW_CLOSED).await, dp::window_closed(dp::WINDOW_CLOSED_BY_PARTNER));

    // Both press ready on the closed window: nothing comes out of either one's bags.
    alice.write(shaiya_proto::encode(dp::OP_STAKE_READY, &[dp::READY])).await;
    bob.write(shaiya_proto::encode(dp::OP_STAKE_READY, &[dp::READY])).await;
    alice.write(shaiya_proto::encode(chat::OP_NORMAL, &[3, b'o', b'k', 0])).await;
    assert_eq!(read_op(&mut alice, chat::OP_NORMAL).await, chat::by_id(chat::OP_NORMAL, alice_id, b"ok\0"));
    let after = (backend.stored(alice_id).unwrap(), backend.stored(bob_id).unwrap());
    assert_eq!(after.0.0, before.0.0, "Alice's gold stays with her");
    assert_eq!(after.1.0, before.1.0);
    assert!(after.0.1.iter().any(|i| (i.bag, i.slot, i.item_type, i.count) == (1, 5, 25, 10)), "her potions are where she put them");
}

/// Alice stakes 4 potions and 300 gold against Bob, both press ready and Alice's stakes are out of her bags; then she leaves the
/// world (her stakes are returned as she goes), the store failing that return when `fail`. Returns Bob and what is stored for Alice.
async fn alice_leaves_with_her_stakes_out(fail: bool) -> (Client, u32, Vec<(u8, u8, u8, u8)>) {
    use shaiya_proto::duel as dp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, _) = open_stakes_window(&backend, addr).await;
    alice.write(shaiya_proto::encode(dp::OP_STAKE_ADD, &[1, 0, 4, 0])).await;
    alice.write(shaiya_proto::encode(dp::OP_STAKE_MONEY, &300u32.to_le_bytes())).await;
    assert_eq!(read_op(&mut alice, dp::OP_STAKE_MONEY).await, dp::stake_money(dp::WHO_ME, 300));
    alice.write(shaiya_proto::encode(dp::OP_STAKE_READY, &[dp::READY])).await;
    bob.write(shaiya_proto::encode(dp::OP_STAKE_READY, &[dp::READY])).await;
    // Her stakes are out of the bags (700 gold, 6 potions) once the countdown is announced.
    read_op(&mut alice, dp::OP_READY).await;
    assert_eq!(stored_money(&backend, alice_id, 700).await, 700);
    backend.fail_next_trade.store(fail, std::sync::atomic::Ordering::SeqCst);
    drop(alice);
    // Wait for the connection to be cleaned up: in the failing case Bob sees the stakes land at Alice's feet.
    let expected = if fail { 700 } else { 1000 };
    stored_money(&backend, alice_id, expected).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let (money, items) = backend.stored(alice_id).unwrap();
    let mut potions: Vec<_> = items.iter().filter(|i| i.bag == 1).map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    potions.sort();
    (bob, money, potions)
}

#[tokio::test]
async fn stakes_leave_with_their_owner_in_one_save_or_all_on_the_ground() {
    use shaiya_proto::duel as dp;
    // The save works: the gold and the potions are back in her bags, a stack of 4 added to her 6.
    let (mut bob, money, potions) = alice_leaves_with_her_stakes_out(false).await;
    assert_eq!((money, potions), (1000, vec![(1, 0, 25, 10)]));
    // The duel ended for Bob (she left the map).
    let _ = read_op(&mut bob, dp::OP_CANCEL).await;

    // The save fails: nothing is in her bags again, and both the gold and the potions lie at her feet, hers alone.
    let (mut bob, money, potions) = alice_leaves_with_her_stakes_out(true).await;
    assert_eq!((money, potions), (700, vec![(1, 0, 25, 6)]));
    let first = read_op(&mut bob, gp::OP_ITEM_ENTER).await;
    let second = read_op(&mut bob, gp::OP_ITEM_ENTER).await;
    let kinds: Vec<_> = [first, second].iter().map(|p| frame::payload(p).to_vec()).collect();
    assert_eq!(kinds.len(), 2, "the gold and the potions were put on the ground: {kinds:?}");
}

#[tokio::test]
async fn a_trade_cannot_open_inside_a_stakes_window_nor_a_duel_inside_a_trade() {
    use shaiya_proto::duel as dp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, bob_id) = open_stakes_window(&backend, addr).await;
    // The window is open: a trade request either way is busy.
    alice.write(shaiya_proto::encode(tp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::start(bob_id, tp::START_BUSY));
    bob.write(shaiya_proto::encode(tp::OP_REQUEST, &alice_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::start(alice_id, tp::START_BUSY));

    // Closing it frees both for a trade; a duel request during the trade is not possible.
    alice.write(shaiya_proto::encode(dp::OP_STAKE_READY, &[dp::CLOSE])).await;
    assert_eq!(read_op(&mut alice, dp::OP_WINDOW_CLOSED).await, dp::window_closed(dp::WINDOW_CLOSED_BY_ME));
    alice.write(shaiya_proto::encode(tp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::request(alice_id));
    bob.write(shaiya_proto::encode(tp::OP_ANSWER, &[tp::ANSWER_YES])).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::start(bob_id, tp::START_OK));
    alice.write(shaiya_proto::encode(dp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut alice, dp::OP_ANSWER).await, dp::answer(dp::NOT_POSSIBLE, bob_id));
    bob.write(shaiya_proto::encode(dp::OP_REQUEST, &alice_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut bob, dp::OP_ANSWER).await, dp::answer(dp::NOT_POSSIBLE, alice_id));
}

#[tokio::test]
async fn a_dead_player_neither_asks_for_a_trade_nor_answers_one() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start_with_data(backend.clone(), config(true), deadly_data()).await;
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(&backend, addr).await;
    die(&mut alice, &ctx, alice_id).await;
    // Dead, she asks Bob: he hears nothing, and she stays free to be asked.
    alice.write(shaiya_proto::encode(tp::OP_REQUEST, &bob_id.to_le_bytes())).await;
    bob.write(shaiya_proto::encode(tp::OP_REQUEST, &alice_id.to_le_bytes())).await;
    assert_eq!(next_trade_packet(&mut alice).await, tp::request(bob_id), "no request of hers came first");
    // Her yes does not open it: Bob hears no `0x0A03` before his own chat line comes back.
    alice.write(shaiya_proto::encode(tp::OP_ANSWER, &[tp::ANSWER_YES])).await;
    bob.write(shaiya_proto::encode(chat::OP_NORMAL, &[3, b'o', b'k', 0])).await;
    loop {
        let packet = bob.read().await.unwrap();
        assert_ne!(frame::opcode(&packet), tp::OP_START, "the trade did not open");
        if frame::opcode(&packet) == chat::OP_NORMAL {
            break;
        }
    }
}

/// The next personal shop packet (`0x23xx` or the sign `0x0523`).
async fn next_shop_packet(client: &mut Client) -> Vec<u8> {
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) >> 8 == 0x23 || frame::opcode(&packet) == shaiya_proto::my_shop::OP_SIGN {
            return packet;
        }
    }
}

#[tokio::test]
async fn a_personal_shop_sells_part_of_a_stack() {
    use shaiya_proto::my_shop as mp;
    let backend = Arc::new(MemoryBackend::default());
    // Map 1 counts as a market map here.
    let (addr, _) = start_with(backend.clone(), Config { market_maps: vec![1], ..config(true) }).await;
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(&backend, addr).await;

    // Bob sits, sets up his 10 potions at 7 each and opens as "Bob's"; Alice sees the sign.
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_SIT])).await;
    bob.write(shaiya_proto::encode(mp::OP_BEGIN, &[])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::begin());
    // The equipment cannot be placed; nothing comes of it.
    bob.write(shaiya_proto::encode(mp::OP_ADD, &[0, 5, 1, 7, 0, 0, 0])).await;
    let add = mp::Add { bag: 1, slot: 0, shop_slot: 0, price: 7 };
    bob.write(shaiya_proto::encode(mp::OP_ADD, &[1, 0, 0, 7, 0, 0, 0])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::added(&add));
    bob.write(shaiya_proto::encode(mp::OP_START, &[6, b'B', b'o', b'b', b'\'', b's', 0])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::started());
    let sign = mp::sign(bob_id, 6, b"Bob's");
    assert_eq!(next_shop_packet(&mut bob).await, sign);
    assert_eq!(next_shop_packet(&mut alice).await, sign);

    // Alice visits: the window and the potions.
    alice.write(shaiya_proto::encode(mp::OP_VISIT, &bob_id.to_le_bytes())).await;
    assert_eq!(next_shop_packet(&mut alice).await, mp::visiting(bob_id));
    let potion = w::ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 10, quality: 1200, ..Default::default() };
    assert_eq!(next_shop_packet(&mut alice).await, mp::list(&[(0, 7, &potion)]));

    // She buys 4 for 28: into her first empty slot, Bob keeps 6, the money moves (no commission off map 42).
    alice.write(shaiya_proto::encode(mp::OP_BUY, &[0, 4])).await;
    assert_eq!(next_shop_packet(&mut alice).await, mp::bought(972, 0, 6, (1, 1), &potion, 4));
    assert_eq!(next_shop_packet(&mut alice).await, mp::slot_count(0, 6));
    assert_eq!(next_shop_packet(&mut bob).await, mp::sold(0, 6, 28));
    let slots = |id| {
        let (money, items) = backend.stored(id).unwrap();
        let mut slots: Vec<_> = items.iter().filter(|i| i.bag > 0).map(|i| (i.bag, i.slot, i.item_type, i.count, i.quality)).collect();
        slots.sort();
        (money, slots)
    };
    assert_eq!(slots(alice_id), (972, vec![(1, 0, 25, 10, 1200), (1, 1, 25, 4, 1200)]), "part of a stack keeps its quality");
    assert_eq!(slots(bob_id), (28, vec![(1, 0, 25, 6, 1200)]));

    // More than is left, or more than she can pay: refused.
    alice.write(shaiya_proto::encode(mp::OP_BUY, &[0, 7])).await;
    assert_eq!(next_shop_packet(&mut alice).await, mp::buy_failed(mp::BUY_FAILED));

    // Bob stands up: the shop ends (0x2302 and the sign down, his own included), Alice's window closes.
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_STAND])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::end());
    assert_eq!(next_shop_packet(&mut bob).await, mp::sign_down(bob_id));
    assert_eq!(next_shop_packet(&mut alice).await, mp::closed_to_visitor());
    assert_eq!(next_shop_packet(&mut alice).await, mp::sign_down(bob_id));
    // Standing, he cannot set up again, and Alice has no shop to visit: only the request made sitting is answered.
    bob.write(shaiya_proto::encode(mp::OP_BEGIN, &[])).await;
    bob.write(shaiya_proto::encode(mp::OP_VISIT, &alice_id.to_le_bytes())).await;
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_SIT])).await;
    bob.write(shaiya_proto::encode(mp::OP_BEGIN, &[])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::begin());
}

#[tokio::test]
async fn a_guild_is_created_joined_talked_in_and_left() {
    use shaiya_proto::guild as gd;
    let backend = Arc::new(MemoryBackend::default());
    // Alone: no party of seven needed (`GAME_GUILD_PARTY_SIZE`).
    let (addr, _) = start_with(backend.clone(), Config { guild_party_size: 1, ..config(true) }).await;
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let mut clients = Vec::new();
    for (user, identity, name) in [(7, [5; 16], &b"Alice"[..]), (8, [6; 16], &b"Bob"[..])] {
        let mut client = Client::connect(addr).await;
        client.enter(user, identity).await;
        client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
        client.write(create(0, 0, 0, name)).await;
        client.read().await.unwrap();
        let id = u32::from_le_bytes(client.read_list().await[4][1..5].try_into().unwrap());
        clients.push((client, id));
    }
    let (mut bob, bob_id) = clients.pop().unwrap();
    let (mut alice, alice_id) = clients.pop().unwrap();
    backend.set_money(alice_id, 1500);
    backend.set_level(alice_id, 11);
    for (client, id) in [(&mut alice, alice_id), (&mut bob, bob_id)] {
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
        client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
        read_zone_entry(client).await;
    }

    // Alice asks, agrees herself, pays 1000 and is the master; Bob, of her country, hears of the new guild.
    let mut request = [0u8; gd::GUILD_NAME_LEN + gd::REMARK_LEN];
    request[..7].copy_from_slice(b"Knights");
    request[gd::GUILD_NAME_LEN..gd::GUILD_NAME_LEN + 2].copy_from_slice(b"Hi");
    alice.write(shaiya_proto::encode(gd::OP_CREATE, &request)).await;
    assert_eq!(read_op(&mut alice, gd::OP_CREATE_AGREE).await, gd::create_agreement(alice_id, b"Knights", b"Hi"));
    alice.write(shaiya_proto::encode(gd::OP_CREATE_AGREE, &[1])).await;
    let created = read_op(&mut alice, gd::OP_CREATE).await;
    let guild_id = u32::from_le_bytes(frame::payload(&created)[1..5].try_into().unwrap());
    assert_eq!(created, gd::create_result(gd::CREATED, guild_id, 1, b"Knights", b"Hi"));
    assert_eq!(read_op(&mut alice, w::OP_SET_MONEY).await, w::set_money(500));
    let unit = |id, rank, name: &[u8]| gd::MemberUnit { id, rank, level: if id == alice_id { 11 } else { 1 }, job: 0, name: name.to_vec() };
    assert_eq!(read_op(&mut alice, gd::OP_MEMBERS_ONLINE).await, gd::member_list(&[unit(alice_id, 1, b"Alice")], &[])[0]);
    let added = read_op(&mut bob, gd::OP_GUILD_ADD).await;
    assert_eq!(&frame::payload(&added)[29..35], b"Alice\0", "the master's name");
    assert_eq!(backend.stored(alice_id).unwrap().0, 500);
    assert_eq!(backend.guild_member(alice_id), Some((guild_id, 1)));

    // Alice invites Bob; he says yes and joins with rank 9.
    alice.write(shaiya_proto::encode(gd::OP_INVITE, &bob_id.to_le_bytes())).await;
    assert_eq!(read_op(&mut bob, gd::OP_INVITE).await, gd::invitation(alice_id));
    bob.write(shaiya_proto::encode(gd::OP_INVITE_ANSWER, &[1])).await;
    assert_eq!(read_op(&mut bob, gd::OP_JOINED).await, gd::joined(true, guild_id, 9, b"Knights"));
    assert_eq!(read_op(&mut alice, gd::OP_MEMBER_ADD).await, gd::member_added(gd::ADDED_ONLINE, &unit(bob_id, 9, b"Bob")));
    assert_eq!(read_op(&mut alice, gd::OP_INVITE_ANSWER).await, gd::invite_answer(true, alice_id));
    // Alice sees his new looks, with the guild's name.
    let looks = read_op(&mut alice, w::OP_USER_SHAPE).await;
    assert!(frame::payload(&looks).ends_with(b"Knights\0"));

    // Guild chat reaches both.
    bob.write(shaiya_proto::encode(chat::OP_GUILD, &[3, b'y', b'o', 0])).await;
    let said = chat::by_name(chat::OP_GUILD, b"Bob", b"yo\0");
    assert_eq!(read_op(&mut alice, chat::OP_GUILD).await, said);
    assert_eq!(read_op(&mut bob, chat::OP_GUILD).await, said);

    // The master cannot leave while Bob is in; Bob can.
    alice.write(shaiya_proto::encode(gd::OP_LEAVE, &[])).await;
    assert_eq!(read_op(&mut alice, gd::OP_LEAVE).await, gd::leave_result(false));
    bob.write(shaiya_proto::encode(gd::OP_LEAVE, &[])).await;
    assert_eq!(read_op(&mut bob, gd::OP_LEAVE).await, gd::leave_result(true));
    assert_eq!(read_op(&mut alice, gd::OP_MEMBER_STATE).await, gd::member_state(gd::STATE_LEFT, bob_id));
    timeout(Duration::from_secs(2), async {
        while backend.guild_member(bob_id).is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Bob's leaving is written");
    assert_eq!(backend.guild_member(alice_id), Some((guild_id, 1)));
}

#[tokio::test]
async fn quests_are_taken_counted_handed_in_and_given_up() {
    use shaiya_proto::combat as cp;
    use shaiya_proto::quest as qp;
    let request = |opcode, npc: u32, quest: u16| shaiya_proto::encode(opcode, &[&npc.to_le_bytes()[..], &quest.to_le_bytes()].concat());
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;

    // Quest 3 needs quest 1 done; quest 1 is taken at the merchant beside the start (object 1), not at the far one (object 2).
    client.write(request(qp::OP_START, 1, 3)).await;
    client.write(request(qp::OP_START, 2, 1)).await;
    client.write(request(qp::OP_START, 1, 1)).await;
    assert_eq!(client.read().await.unwrap(), qp::started(1, 1));
    // Not yet done: handing it in does nothing.
    client.write(request(qp::OP_END, 1, 1)).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(client.read().await.unwrap(), w::motion(id, 0));

    // The kill counts (0x0905) after its experience.
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    assert_eq!(client.read().await.unwrap(), w::mob_enter(1, true, 1, 600.0, 1780.0));
    assert_eq!(client.read().await.unwrap(), w::mob_max_hp(1, 120, 120));
    client.write(step(598.0, 1779.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE);
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    assert_eq!(read_op(&mut client, qp::OP_UPDATE).await, qp::update(1, 0, 1));
    assert_eq!(backend.play(id).unwrap().quests[0].counts, [1, 0, 0], "the counter is saved");

    // Handed in at the merchant: 100 experience (50 doubled), then 0x0903 with the money and the potion in the first free slot (the
    // starting potions are in bag 1 slot 0). A kill quest's 0x0903 names no NPC.
    client.write(request(qp::OP_END, 1, 1)).await;
    assert_eq!(client.read().await.unwrap(), cp::exp_gain(100, 0));
    let ended =
        qp::Ended { npc_id: 0, quest_id: 1, success: true, result: 0, exp: 100, money: 100, bag: 1, slot: 1, item_type: 25, type_id: 1 };
    assert_eq!(client.read().await.unwrap(), qp::ended(&ended));
    let stored = backend.play(id).unwrap();
    assert_eq!(stored.money, 100);
    assert!(stored.items.iter().any(|i| (i.bag, i.slot, i.item_type, i.count) == (1, 1, 25, 1)), "the potion is saved");
    assert_eq!(stored.exp, 244);
    assert_eq!(stored.quests.iter().map(|q| (q.quest_id, q.finished, q.success)).collect::<Vec<_>>(), [(1, true, true)]);
    // Never twice; quest 3 opens now.
    client.write(request(qp::OP_START, 1, 1)).await;
    client.write(request(qp::OP_START, 1, 3)).await;
    assert_eq!(client.read().await.unwrap(), qp::started(1, 3));

    // Quest 2 from any NPC (0x0902 names none), given up: 0x0903 failure. It is repeatable: taken again at once.
    client.write(request(qp::OP_START, 3, 2)).await;
    assert_eq!(client.read().await.unwrap(), qp::started(0, 2));
    client.write(shaiya_proto::encode(qp::OP_GIVE_UP, &2u16.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), qp::ended(&qp::Ended { quest_id: 2, ..Default::default() }));
    client.write(request(qp::OP_START, 3, 2)).await;
    assert_eq!(client.read().await.unwrap(), qp::started(0, 2));
    // Quest 3 given up is finished as failed, for good.
    client.write(shaiya_proto::encode(qp::OP_GIVE_UP, &3u16.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), qp::ended(&qp::Ended { quest_id: 3, ..Default::default() }));
    client.write(request(qp::OP_START, 1, 3)).await;

    // Back in: the lists come in the burst after the skills (0x0901 quest 2 in progress, 0x0906 quests 1 and 3).
    client.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert!(read_until_closed(&mut client).await);
    wait_gone(&backend, 7, Duration::from_secs(2)).await;
    backend.add_session(7, session([6; 16], Some(SERVER_ID), LOCAL));
    let mut back = Client::connect(addr).await;
    back.enter(7, [6; 16]).await;
    back.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
    let mut lists = Vec::new();
    loop {
        let packet = back.read().await.unwrap();
        match frame::opcode(&packet) {
            qp::OP_LIST | qp::OP_FINISHED => lists.push(packet),
            w::OP_CHAR_QUICK => break,
            _ => {}
        }
    }
    let active = qp::ActiveEntry { quest_id: 2, minutes_left: 0, counts: [0; 3] };
    assert_eq!(lists, [qp::active_list(&[active]), qp::finished_list(&[(1, true), (3, false)])].concat());

    // The game log: every start (131: the quest and the NPC asked), and the endings (133: V2 1 for a quest done, 0 for one given up).
    let started = logged(&ctx, 131);
    assert!(started.len() >= 4, "{} starts", started.len());
    assert_eq!(set_values(&started[0]).first(), Some(&(1, 1)));
    let ended = logged(&ctx, 133);
    assert!(ended.iter().any(|r| r.values[1] == Some(1)) && ended.iter().any(|r| r.values[1] == Some(0)));
}

/// Reads until the server closes the connection; true when it did.
async fn read_until_closed(client: &mut Client) -> bool {
    while client.read().await.is_some() {}
    true
}

#[tokio::test]
async fn friends_are_asked_accepted_blocked_and_kept() {
    use game_server::backend::CharacterStore;
    use shaiya_proto::friend as fr;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(&backend, addr).await;
    let name = |n: &[u8]| {
        let mut field = n.to_vec();
        field.resize(21, 0);
        field
    };

    // Alice asks for Bob by name (any case); Bob is asked and says yes; both get the other.
    alice.write(shaiya_proto::encode(fr::OP_REQUEST, &name(b"bob"))).await;
    assert_eq!(read_op(&mut bob, fr::OP_REQUEST).await, fr::request(b"Alice"));
    bob.write(shaiya_proto::encode(fr::OP_ANSWER, &[1])).await;
    assert_eq!(read_op(&mut alice, fr::OP_ANSWER).await, fr::answer(fr::DONE));
    assert_eq!(read_op(&mut alice, fr::OP_FRIEND_ADD).await, fr::friend_added(bob_id, 0, b"Bob"));
    assert_eq!(read_op(&mut bob, fr::OP_FRIEND_ADD).await, fr::friend_added(alice_id, 0, b"Alice"));
    // Asking again fails: Bob is listed already.
    alice.write(shaiya_proto::encode(fr::OP_REQUEST, &name(b"Bob"))).await;
    assert_eq!(read_op(&mut alice, fr::OP_ANSWER).await, fr::answer(fr::CANNOT_ASK));
    // A memo, exactly as sent.
    alice.write(shaiya_proto::encode(fr::OP_FRIEND_MEMO, &[&bob_id.to_le_bytes()[..], &[4], b"tank"].concat())).await;
    assert_eq!(read_op(&mut alice, fr::OP_FRIEND_MEMO).await, fr::friend_memo(bob_id, b"tank"));

    // Bob blocks Alice: her whispers find nobody.
    bob.write(shaiya_proto::encode(fr::OP_BLOCK_ADD, &name(b"Alice"))).await;
    assert_eq!(read_op(&mut bob, fr::OP_BLOCK_RESULT).await, fr::block_result(fr::BLOCKED));
    assert_eq!(read_op(&mut bob, fr::OP_BLOCK_ADD).await, fr::block_added(alice_id, b"Alice"));
    let mut whisper = name(b"Bob");
    whisper.extend_from_slice(&[3, b'y', b'o', 0]);
    alice.write(shaiya_proto::encode(chat::OP_WHISPER, &whisper)).await;
    assert_eq!(read_op(&mut alice, chat::OP_WHISPER_FAILED).await, chat::whisper_failed());

    // The party search: Alice registers and finds nobody; Bob registers and finds her.
    alice.write(shaiya_proto::encode(fr::OP_PARTY_SEARCH, &[])).await;
    assert_eq!(read_op(&mut alice, fr::OP_PARTY_SEARCH).await, fr::party_search_registered());
    bob.write(shaiya_proto::encode(fr::OP_PARTY_SEARCH, &[])).await;
    let found = read_op(&mut bob, fr::OP_PARTY_FOUND).await;
    assert_eq!(found, fr::party_found(&[fr::FoundUnit { level: 1, job: 0, name: b"Alice".to_vec() }]).unwrap());

    // What is stored: Alice's friend with the memo, Bob's friend and block.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let alice_saved = backend.load(7, SERVER_ID, alice_id).await.unwrap().unwrap();
    assert_eq!(alice_saved.friends.iter().map(|f| (f.id, f.name.as_str(), f.memo.as_str())).collect::<Vec<_>>(), [(bob_id, "Bob", "tank")]);
    let bob_saved = backend.load(8, SERVER_ID, bob_id).await.unwrap().unwrap();
    assert_eq!(bob_saved.friends.iter().map(|f| f.id).collect::<Vec<_>>(), [alice_id]);
    assert_eq!(bob_saved.blocks.iter().map(|f| f.id).collect::<Vec<_>>(), [alice_id]);

    // A 0x2207 from the client closes the connection, as in the original.
    alice.write(shaiya_proto::encode(fr::OP_ONLINE, &[0; 5])).await;
    assert!(read_until_closed(&mut alice).await);
    // Bob sees Alice go offline once her character has left the world.
    assert_eq!(read_op(&mut bob, fr::OP_ONLINE).await, fr::online(alice_id, false));
}

// Operator (GM) commands (`docs/original/gm-commands.md`).

/// Every packet up to and including the first one with that opcode.
async fn read_until(client: &mut Client, opcode: u16) -> Vec<Vec<u8>> {
    let mut seen = Vec::new();
    loop {
        let packet = client.read().await.expect("the connection stays open");
        let done = frame::opcode(&packet) == opcode;
        seen.push(packet);
        if done {
            return seen;
        }
    }
}

/// A staff member "Boss" (user 7, account `gmboss`, the given level) and a player "Bob" (user 8, account `bobby`) in the world on map 1,
/// both human beside the start. Returns the clients with their character ids and what Bob was sent on entering.
async fn gm_and_player(backend: &Arc<MemoryBackend>, addr: SocketAddr, level: u8) -> (Client, u32, Client, u32, Vec<Vec<u8>>) {
    gm_and_other(backend, addr, level, 11).await
}

/// The same with Bob of another staff level (11 is a player).
async fn gm_and_other(backend: &Arc<MemoryBackend>, addr: SocketAddr, level: u8, other: u8) -> (Client, u32, Client, u32, Vec<Vec<u8>>) {
    backend.add_session(7, StoredSession { privilege: level, ..session([5; 16], Some(SERVER_ID), LOCAL) });
    backend.add_session(8, StoredSession { privilege: other, ..session([6; 16], Some(SERVER_ID), LOCAL) });
    backend.add_account(7, "gmboss", i16::from(level) * 16);
    backend.add_account(8, "bobby", if other <= 5 { i16::from(other) * 16 } else { 0 });
    let mut clients = Vec::new();
    for (user, identity, name) in [(7, [5; 16], &b"Boss"[..]), (8, [6; 16], &b"Bob"[..])] {
        let mut client = Client::connect(addr).await;
        client.enter(user, identity).await;
        client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
        client.write(create(0, 0, 0, name)).await;
        client.read().await.unwrap();
        let id = u32::from_le_bytes(client.read_list().await[4][1..5].try_into().unwrap());
        clients.push((client, id));
    }
    let (mut bob, bob_id) = clients.pop().unwrap();
    let (mut gm, gm_id) = clients.pop().unwrap();
    for (client, id) in [(&mut gm, gm_id), (&mut bob, bob_id)] {
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
    }
    gm.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_until(&mut gm, shaiya_proto::combat::OP_RECOVER_SET).await;
    bob.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    let entry = read_until(&mut bob, shaiya_proto::combat::OP_RECOVER_SET).await;
    // The staff member is told of Bob (0x0201 and his speed).
    read_until(&mut gm, w::OP_SPEED).await;
    (gm, gm_id, bob, bob_id, entry)
}

/// A GM command: the opcode and the payload.
fn command(opcode: u16, payload: &[u8]) -> Vec<u8> {
    shaiya_proto::encode(opcode, payload)
}

/// A zero-padded name field.
fn field(name: &[u8], len: usize) -> Vec<u8> {
    let mut field = name.to_vec();
    field.resize(len, 0);
    field
}

/// The money a character has stored, once it has settled at `expected` (the server saves after the client's packets).
async fn stored_money(backend: &Arc<MemoryBackend>, id: u32, expected: u32) -> u32 {
    for _ in 0..50 {
        let money = backend.stored(id).unwrap().0;
        if money == expected {
            return money;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    backend.stored(id).unwrap().0
}

#[tokio::test]
async fn a_player_gambles_at_a_table_stands_up_and_enters_the_zone_again() {
    use shaiya_proto::mini_game as mp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start_gambling(backend.clone()).await;
    let (mut alice, alice_id, mut bob, _) = two_in_the_world(&backend, addr).await;
    // Alice stands by the gambling NPC with 1000 gold. The table's bet per line: 100 (table 0).
    alice.write(shaiya_proto::encode(mp::OP_SIT, &[7])).await;
    assert_eq!(read_op(&mut alice, mp::OP_SIT).await, mp::sat(false), "a table above 6");
    alice.write(shaiya_proto::encode(mp::OP_SIT, &[0])).await;
    assert_eq!(read_op(&mut alice, mp::OP_SIT).await, mp::sat(true));
    // The others see her leave the zone.
    assert_eq!(read_op(&mut bob, w::OP_USER_LEAVE).await, w::user_leave(alice_id));
    // Three lines: 100, 200, 300 bet.
    for lines in 1..=3u32 {
        alice.write(shaiya_proto::encode(mp::OP_BET, &[])).await;
        assert_eq!(read_op(&mut alice, mp::OP_BET).await, mp::bet(mp::BET_LINE, 100 * lines, 1000 - 100 * lines));
    }
    assert_eq!(stored_money(&backend, alice_id, 700).await, 700, "the bet is saved at once");
    // A spin: `ok` 1 and the nine cells; the winnings are not paid yet, and a lost spin pays the next bet.
    alice.write(shaiya_proto::encode(mp::OP_SPIN, &[])).await;
    let spin = read_op(&mut alice, mp::OP_SPIN).await;
    assert_eq!((frame::payload(&spin).len(), frame::payload(&spin)[0]), (16, 1));
    // A packet in the wrong state gets no answer: the next answer is the stand-up's.
    alice.write(shaiya_proto::encode(mp::OP_DOUBLE, &[])).await;
    alice.write(shaiya_proto::encode(mp::OP_GUESS, &[1])).await;
    // Standing up: `0x0213` with the money, then `0x3106` with the same; whatever lay on the table is in it.
    alice.write(shaiya_proto::encode(mp::OP_STAND, &[])).await;
    let set = read_op(&mut alice, w::OP_SET_MONEY).await;
    let stood = read_op(&mut alice, mp::OP_STAND).await;
    let money = u32::from_le_bytes(frame::payload(&stood).try_into().unwrap());
    assert_eq!(set, w::set_money(money));
    assert!((700..).contains(&money), "{money}: at least the 700 left over, the table's bet or the re-bet and the win back");
    assert_eq!(stored_money(&backend, alice_id, money).await, money);
    // Out of the zone until `0x0201`: then she is in it again and Bob sees her.
    alice.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(&mut alice).await;
    assert_eq!(frame::opcode(&read_op(&mut bob, w::OP_USER_ENTER).await), w::OP_USER_ENTER);
    // The game log (`docs/original/mini-game.md` section 7): 190 on sitting down with the map, 193 and 192 for the spin, 191 standing up.
    let sat = logged(&ctx, 190);
    assert_eq!((sat.len(), sat[0].values[..3].to_vec()), (1, vec![Some(1000), Some(1), Some(100)]));
    assert_eq!(sat[0].header.map, 1);
    let spun = logged(&ctx, 193);
    assert_eq!((spun.len(), spun[0].values[..2].to_vec(), spun[0].header.level), (1, vec![Some(3), Some(300)], 0));
    assert_eq!(logged(&ctx, 192)[0].values[1], Some(-300), "the spin's bet");
    let stood = logged(&ctx, 191);
    assert_eq!((stood.len(), stood[0].values[0], stood[0].header.map), (1, Some(i64::from(money)), 0));
}

#[tokio::test]
async fn a_game_still_open_is_closed_by_entering_the_zone_and_by_a_lost_connection() {
    use shaiya_proto::mini_game as mp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_gambling(backend.clone()).await;
    let (mut alice, alice_id, bob, _) = two_in_the_world(&backend, addr).await;
    // `0x0201` with a game open: the bet comes back with `0x0213`, then the zone is entered as usual.
    alice.write(shaiya_proto::encode(mp::OP_SIT, &[1])).await;
    assert_eq!(read_op(&mut alice, mp::OP_SIT).await, mp::sat(true));
    alice.write(shaiya_proto::encode(mp::OP_BET, &[])).await;
    assert_eq!(read_op(&mut alice, mp::OP_BET).await, mp::bet(mp::BET_LINE, 300, 700));
    alice.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    assert_eq!(read_op(&mut alice, w::OP_SET_MONEY).await, w::set_money(1000));
    read_zone_entry(&mut alice).await;
    assert_eq!(stored_money(&backend, alice_id, 1000).await, 1000);
    // A lost connection at the table: the bet is handed back before the character is saved.
    alice.write(shaiya_proto::encode(mp::OP_SIT, &[0])).await;
    assert_eq!(read_op(&mut alice, mp::OP_SIT).await, mp::sat(true));
    alice.write(shaiya_proto::encode(mp::OP_BET, &[])).await;
    alice.write(shaiya_proto::encode(mp::OP_BET, &[])).await;
    assert_eq!(read_op(&mut alice, mp::OP_BET).await, mp::bet(mp::BET_LINE, 100, 900));
    assert_eq!(read_op(&mut alice, mp::OP_BET).await, mp::bet(mp::BET_LINE, 200, 800));
    assert_eq!(stored_money(&backend, alice_id, 800).await, 800);
    drop(alice);
    drop(bob);
    assert_eq!(stored_money(&backend, alice_id, 1000).await, 1000, "the 200 bet came back");
}

#[tokio::test]
async fn staff_commands_are_checked_by_level_and_players_are_disconnected() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, _, _) = gm_and_player(&backend, addr, 5).await;
    // Level 5: /getitem is for level 2 and above, /pvpon for every level, the unbuilt ones are answered, not closed.
    gm.write(command(ap::OP_MAKE_ITEM, &[25, 1, 1])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_MAKE_ITEM));
    gm.write(command(0xFA03, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0xFA03));
    gm.write(command(ap::OP_PVP_ON, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    // A player sending a staff opcode is disconnected.
    bob.write(command(ap::OP_INVISIBLE, &[])).await;
    assert!(read_until_closed(&mut bob).await);
}

#[tokio::test]
async fn staff_make_items_into_their_own_bags() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut gm, gm_id, _bob, _, _) = gm_and_player(&backend, addr, 1).await;
    // An unknown item gets no answer: the next answer is the one of the next command.
    gm.write(command(ap::OP_MAKE_ITEM, &[99, 99, 1])).await;
    gm.write(command(ap::OP_SERVER_ON, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    gm.write(command(ap::OP_MAKE_ITEM, &[25, 1, 3])).await;
    let seen = read_until(&mut gm, ap::OP_ANSWER).await;
    assert_eq!(seen.iter().map(|p| frame::opcode(p)).collect::<Vec<_>>(), [0x0205, ap::OP_ANSWER]);
    assert_eq!(seen[1], ap::answer(0));
    let stored = backend.play(gm_id).unwrap();
    assert_eq!(
        stored.items.iter().filter(|i| i.item_type == 25 && i.count == 3 && i.bag >= 1).count(),
        1,
        "three potions in one new stack"
    );
    // The staff's game log: the command (180, level 1 and the opcode's name) and the new item's record (111, T2 'ItemCreate').
    let commands = logged(&ctx, 180);
    assert_eq!((commands[0].values[0], commands[0].texts[0].as_deref()), (Some(1), Some(&format!("{:04X}", ap::OP_MAKE_ITEM)[..])));
    let made = logged(&ctx, 111);
    assert_eq!(
        (made.len(), made[0].texts[1].as_deref(), set_values(&made[0])[1..].to_vec()),
        (1, Some("ItemCreate"), vec![(2, 25001), (3, 3)])
    );
    // No item without a count.
    gm.write(command(ap::OP_MAKE_ITEM, &[25, 1, 0])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_MAKE_ITEM));
}

#[tokio::test]
async fn set_changes_another_players_level_and_tells_both() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 1).await;
    let mut request = vec![1];
    request.extend_from_slice(&2u32.to_le_bytes());
    request.extend_from_slice(&field(b"bOb", 21));
    gm.write(command(ap::OP_SET, &request)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    // The start exp of level 2 first, the level-up for the view, the main stat of the fighter (STR, number 5), then the level.
    let seen = read_until(&mut bob, ap::OP_SET).await;
    assert_eq!(seen.last().unwrap(), &ap::set_answer(16, 100));
    let seen = read_until(&mut bob, ap::OP_SET).await;
    assert!(seen.iter().any(|p| frame::opcode(p) == shaiya_proto::combat::OP_LEVEL_UP));
    assert_eq!(frame::payload(seen.last().unwrap())[0], 5);
    assert_eq!(bob.read().await.unwrap(), ap::set_answer(1, 2));
    let stored = backend.play(bob_id).unwrap();
    assert_eq!((stored.level, stored.exp), (2, 100));
    // A level past the table's mode, or the level it has: refused.
    request[1..5].copy_from_slice(&61u32.to_le_bytes());
    gm.write(command(ap::OP_SET, &request)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_SET));
    // Money.
    request[0] = 2;
    request[1..5].copy_from_slice(&1_000_000u32.to_le_bytes());
    gm.write(command(ap::OP_SET, &request)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut bob, ap::OP_SET).await, ap::set_answer(2, 1_000_000));
    assert_eq!(backend.play(bob_id).unwrap().money, 1_000_000);
}

#[tokio::test]
async fn gm_chat_reaches_the_view_and_chat_bans_silence_a_player() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, gm_id, mut bob, bob_id, _) = gm_and_player(&backend, addr, 1).await;
    let say = |opcode, text: &[u8]| shaiya_proto::encode(opcode, &[&[text.len() as u8 + 1][..], text, &[0]].concat());
    gm.write(say(ap::OP_CHAT_NORMAL, b"hi")).await;
    assert_eq!(read_op(&mut bob, ap::OP_CHAT_NORMAL).await, chat::by_id(ap::OP_CHAT_NORMAL, gm_id, b"hi\0"));
    // Silenced for ever: Bob is told, nobody gets his line.
    let mut ban = 0u16.to_le_bytes().to_vec();
    ban.extend_from_slice(&field(b"Bob", 21));
    gm.write(command(ap::OP_SILENCE, &ban)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut bob, ap::OP_SILENCE).await, ap::bare(ap::OP_SILENCE));
    bob.write(say(chat::OP_NORMAL, b"yo")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    gm.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(gm.read().await.unwrap(), w::motion(gm_id, 0), "nothing from Bob before the echo");
    // Lifted: Bob is told and his lines reach the others again.
    gm.write(command(ap::OP_SILENCE_OFF, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut bob, ap::OP_SILENCE_OFF).await, ap::bare(ap::OP_SILENCE_OFF));
    bob.write(say(chat::OP_NORMAL, b"yo")).await;
    assert_eq!(read_op(&mut gm, chat::OP_NORMAL).await, chat::by_id(chat::OP_NORMAL, bob_id, b"yo\0"));
}

/// What the next whisper says (`0x1102`, the receiving form): its sender and its text.
async fn next_whisper(client: &mut Client) -> (String, String) {
    let packet = read_op(client, chat::OP_WHISPER).await;
    let body = &packet[4..];
    assert_eq!(body[0], 0, "from the sender, not to it");
    let name = String::from_utf8_lossy(&body[1..22]).trim_end_matches('\0').to_owned();
    let text = String::from_utf8_lossy(&body[23..23 + usize::from(body[22])]).trim_end_matches('\0').to_owned();
    (name, text)
}

#[tokio::test]
async fn a_level_one_gm_types_console_commands_with_a_dot() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut gm, gm_id, mut bob, _, _) = gm_and_player(&backend, addr, 1).await;
    let say = |opcode, text: &[u8]| shaiya_proto::encode(opcode, &[&[text.len() as u8 + 1][..], text, &[0]].concat());
    gm.write(say(ap::OP_CHAT_NORMAL, b".info")).await;
    let (from, text) = next_whisper(&mut gm).await;
    assert_eq!(from, "Server");
    assert!(
        text.starts_with("players 2 (") && text.contains("accepting yes") && text.contains("exp x1.000, killrate 1, shop on, killcount on"),
        "{text}"
    );
    // Wrong words and arguments change nothing and say how to use the command.
    gm.write(say(ap::OP_CHAT_NORMAL, b".foo")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "unknown command foo; try help");
    gm.write(say(ap::OP_CHAT_NORMAL, b".exp -1")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "usage: exp <factor> | exp off");
    assert_eq!(next_whisper(&mut gm).await.1.split(' ').next(), Some("exp"));
    assert_eq!(next_whisper(&mut gm).await.1, "e.g. .exp 1.5");
    assert_eq!(ctx.switches.exp_rate(), 1.0);
    // The switches.
    gm.write(say(ap::OP_CHAT_NORMAL, b".exp 2")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "exp x2.000");
    gm.write(say(ap::OP_CHAT_NORMAL, b".killrate 3")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "killrate 3");
    gm.write(say(ap::OP_CHAT_NORMAL, b".shop silent")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "shop silent");
    gm.write(say(ap::OP_CHAT_NORMAL, b".killcount off")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "killcount off");
    gm.write(say(ap::OP_CHAT_NORMAL, b".maxusers 40")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "maxusers 40");
    assert_eq!(
        (ctx.switches.exp_rate(), ctx.switches.kill_rate(), ctx.switches.shop(), ctx.switches.max_users()),
        (2.0, 3, game_server::switches::SHOP_SILENT, 40)
    );
    assert!(ctx.kill_counts.guard().disabled);
    // A notice reaches everyone, the typed lines themselves nobody.
    gm.write(say(ap::OP_CHAT_NORMAL, b".notice Hello all")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "notice sent");
    assert_eq!(read_op(&mut bob, ap::OP_NOTICE_WORLD).await, ap::world_notice("Hello all").unwrap());
    // A countdown stops logins; cancelling opens them again.
    gm.write(say(ap::OP_CHAT_NORMAL, b".quit 1")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "shutting down in 60 seconds");
    assert!(!ctx.accepting.load(std::sync::atomic::Ordering::Relaxed));
    gm.write(say(ap::OP_CHAT_NORMAL, b".quit")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "a shutdown is already counting down");
    gm.write(say(ap::OP_CHAT_NORMAL, b".quit cancel")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "the shutdown was cancelled");
    assert!(ctx.accepting.load(std::sync::atomic::Ordering::Relaxed));
    gm.write(say(ap::OP_CHAT_NORMAL, b".quit cancel")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "no shutdown is counting down");
    // `help` lists the commands, `help kick` one.
    gm.write(say(ap::OP_CHAT_NORMAL, b".help")).await;
    assert_eq!(next_whisper(&mut gm).await.1, "help [command]");
    // A line that is not a command is chat, and Bob never saw the typed ones.
    gm.write(say(ap::OP_CHAT_NORMAL, b"hi")).await;
    assert_eq!(read_op(&mut bob, ap::OP_CHAT_NORMAL).await, chat::by_id(ap::OP_CHAT_NORMAL, gm_id, b"hi\0"));
}

#[tokio::test]
async fn a_higher_level_gms_dot_line_is_ordinary_chat() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut gm, gm_id, mut bob, _, _) = gm_and_player(&backend, addr, 5).await;
    let say = |opcode, text: &[u8]| shaiya_proto::encode(opcode, &[&[text.len() as u8 + 1][..], text, &[0]].concat());
    gm.write(say(ap::OP_CHAT_NORMAL, b".killrate 9")).await;
    assert_eq!(read_op(&mut bob, ap::OP_CHAT_NORMAL).await, chat::by_id(ap::OP_CHAT_NORMAL, gm_id, b".killrate 9\0"));
    assert_eq!(ctx.switches.kill_rate(), 1);
}

#[tokio::test]
async fn a_listening_gm_gets_copies_of_the_players_lines_until_it_stops() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, gm_id, mut bob, bob_id, _) = gm_and_player(&backend, addr, 5).await;
    let say = |opcode, text: &[u8]| shaiya_proto::encode(opcode, &[&[text.len() as u8 + 1][..], text, &[0]].concat());
    gm.write(command(ap::OP_LISTEN, &field(b"Nobody", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_LISTEN));
    gm.write(command(ap::OP_LISTEN, &field(b"bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    bob.write(say(chat::OP_NORMAL, b"hi")).await;
    assert_eq!(read_op(&mut gm, ap::OP_LISTEN_CHAT).await, ap::listen_chat(b"Bob", b"NORMAL", None, b"hi\0"));
    // A whisper names its target; the copy comes beside the whisper itself.
    let mut whisper = field(b"Boss", 21);
    whisper.extend_from_slice(&[5]);
    whisper.extend_from_slice(b"psst\0");
    bob.write(shaiya_proto::encode(chat::OP_WHISPER, &whisper)).await;
    let seen = read_until(&mut gm, ap::OP_LISTEN_CHAT).await;
    assert!(seen.iter().any(|p| frame::opcode(p) == chat::OP_WHISPER));
    assert_eq!(seen.last().unwrap(), &ap::listen_chat(b"Bob", b"TO", Some(b"Boss"), b"psst\0"));
    // `/cwatch off`: no more copies, and a second one has nothing to stop.
    gm.write(command(ap::OP_LISTEN_OFF, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    bob.write(say(chat::OP_NORMAL, b"bye")).await;
    assert_eq!(read_op(&mut gm, chat::OP_NORMAL).await, chat::by_id(chat::OP_NORMAL, bob_id, b"bye\0"));
    gm.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    let seen = read_until(&mut gm, w::OP_MOTION).await;
    assert!(seen.iter().all(|p| frame::opcode(p) != ap::OP_LISTEN_CHAT), "no copy after the stop");
    assert_eq!(seen.last().unwrap(), &w::motion(gm_id, 0));
    gm.write(command(ap::OP_LISTEN_OFF, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_LISTEN_OFF));
}

#[tokio::test]
async fn staff_come_in_hidden_and_show_themselves_on_request() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, gm_id, mut bob, _, entry) = gm_and_player(&backend, addr, 1).await;
    // Bob saw the staff member arrive as kind 2, then with the hidden shape 13.
    let arrival = entry.iter().find(|p| frame::opcode(p) == w::OP_USER_ENTER && frame::payload(p)[..4] == gm_id.to_le_bytes()).unwrap();
    assert_eq!(frame::payload(arrival)[4], 2);
    let shape = entry.iter().find(|p| frame::opcode(p) == sk::OP_SHAPE).unwrap();
    assert_eq!(frame::payload(shape), [&gm_id.to_le_bytes()[..], &[13]].concat());
    // /char off: the real shape.
    gm.write(command(ap::OP_VISIBLE, &[])).await;
    assert_eq!(read_op(&mut bob, sk::OP_SHAPE).await, sk::shape(gm_id, 0));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    // /warning reaches the player.
    let mut warning = field(b"Bob", 21);
    warning.extend_from_slice(&[7]);
    warning.extend_from_slice(b"behave\0");
    gm.write(command(ap::OP_WARNING, &warning)).await;
    assert_eq!(read_op(&mut bob, ap::OP_WARNING).await, ap::warning(b"behave\0"));
}

#[tokio::test]
async fn gm_moves_happen_at_once_and_refuse_unknown_maps() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, _bob, _, _) = gm_and_player(&backend, addr, 1).await;
    let go = |x: f32, z: f32, map: u16| [&x.to_le_bytes()[..], &z.to_le_bytes(), &map.to_le_bytes()].concat();
    gm.write(command(ap::OP_MOVE_TO_SPOT, &go(700.0, 1700.0, 1))).await;
    let seen = read_until(&mut gm, ap::OP_ANSWER).await;
    let opcodes: Vec<_> = seen.iter().map(|p| frame::opcode(p)).collect();
    assert_eq!(opcodes[opcodes.len() - 2..], [w::OP_POSITION, ap::OP_ANSWER], "the view swap, 0x020B, then the answer");
    assert_eq!(seen.last().unwrap(), &ap::answer(0));
    gm.write(command(ap::OP_MOVE_TO_SPOT, &go(100.0, 100.0, 200))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_MOVE_TO_SPOT));
    gm.write(command(ap::OP_MOVE_TO_SPOT, &go(-5.0, 100.0, 1))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_MOVE_TO_SPOT));
}

#[tokio::test]
async fn server_off_refuses_new_connections_until_on() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut gm, _, _bob, _, _) = gm_and_player(&backend, addr, 1).await;
    backend.add_session(9, session([7; 16], Some(SERVER_ID), LOCAL));
    // Each word sets the switch, however often it is sent (`docs/original/small-items.md` section 4).
    for _ in 0..2 {
        gm.write(command(ap::OP_SERVER_OFF, &[])).await;
        assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
        assert!(!ctx.accepting.load(Ordering::Relaxed));
    }
    let mut refused = Client::connect(addr).await;
    refused.write(g::game_connect_request(9, &[7; 16])).await;
    assert!(refused.read().await.is_none(), "closed before the handshake answer");
    for _ in 0..2 {
        gm.write(command(ap::OP_SERVER_ON, &[])).await;
        assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
        assert!(ctx.accepting.load(Ordering::Relaxed));
    }
    let mut accepted = Client::connect(addr).await;
    accepted.enter(9, [7; 16]).await;
}

#[tokio::test]
async fn account_bans_notify_and_disconnect_and_can_be_lifted() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, _, _) = gm_and_player(&backend, addr, 2).await;
    let mut ban = 5u16.to_le_bytes().to_vec();
    ban.extend_from_slice(&field(b"Bobby", 21));
    gm.write(command(ap::OP_ACCOUNT_BAN, &ban)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    let notice = read_op(&mut bob, ap::OP_ACCOUNT_BAN).await;
    assert_eq!(notice, ap::ban_notice(ap::OP_ACCOUNT_BAN, 5));
    assert!(read_until_closed(&mut bob).await);
    let account = backend.account(8).unwrap();
    assert_eq!((account.status, account.blocked_minutes), (2, Some(5)));
    // Lifted; lifting again fails.
    gm.write(command(ap::OP_ACCOUNT_UNBAN, &field(b"bobby", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(backend.account(8).unwrap().status, 0);
    gm.write(command(ap::OP_ACCOUNT_UNBAN, &field(b"bobby", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_ACCOUNT_UNBAN));
    // Not one's own account, not a staff account of one's own level, not an unknown one.
    for name in [&b"gmboss"[..], b"nobody"] {
        let mut ban = 0u16.to_le_bytes().to_vec();
        ban.extend_from_slice(&field(name, 21));
        gm.write(command(ap::OP_ACCOUNT_BAN, &ban)).await;
        assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_ACCOUNT_BAN));
    }
    // /ban names a character: the GM's own is refused, as its own account.
    let mut ban = 0u16.to_le_bytes().to_vec();
    ban.extend_from_slice(&field(b"Boss", 21));
    gm.write(command(ap::OP_BAN, &ban)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_BAN));
}

#[tokio::test]
async fn a_closed_quest_cannot_be_started_until_opened_and_event_tags_are_kept() {
    use shaiya_proto::quest as qp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 1).await;
    let start_quest = || shaiya_proto::encode(qp::OP_START, &[&1u32.to_le_bytes()[..], &1u16.to_le_bytes()].concat());
    gm.write(command(ap::OP_QUEST_OFF, &1u16.to_le_bytes())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert!(ctx.closed_quests.guard().contains(&1));
    assert!(backend.closed_quests.guard().contains(&(SERVER_ID, 1)), "saved");
    bob.write(start_quest()).await;
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    let seen = read_until(&mut bob, w::OP_MOTION).await;
    assert!(seen.iter().all(|p| frame::opcode(p) != qp::OP_START), "nothing started");
    // Closing twice, an unknown quest: failures.
    gm.write(command(ap::OP_QUEST_OFF, &1u16.to_le_bytes())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUEST_OFF));
    gm.write(command(ap::OP_QUEST_ON, &99u16.to_le_bytes())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUEST_ON));
    gm.write(command(ap::OP_QUEST_ON, &1u16.to_le_bytes())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    bob.write(start_quest()).await;
    assert_eq!(read_op(&mut bob, qp::OP_START).await, qp::started(1, 1));

    // Event tags: once, then the failure; removed once.
    let tag = [field(b"Bob", 21), field(b"winners", 21)].concat();
    for (opcode, expected) in
        [(ap::OP_EVENT_ADD, 0), (ap::OP_EVENT_ADD, ap::OP_EVENT_ADD), (ap::OP_EVENT_REMOVE, 0), (ap::OP_EVENT_REMOVE, ap::OP_EVENT_REMOVE)]
    {
        gm.write(command(opcode, &tag)).await;
        assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(expected), "{opcode:04X}");
    }
    assert!(backend.events.guard().is_empty());
    let _ = bob_id;
}

/// `0x1201 /request GM`: `u16 n, text` with the zero counted.
fn request_gm(text: &[u8]) -> Vec<u8> {
    let mut body = text.to_vec();
    body.push(0);
    shaiya_proto::encode(ap::OP_USER_QUESTION, &[&(body.len() as u16).to_le_bytes()[..], &body].concat())
}

/// A packet Bob can be waited for: the motion answer says everything he sent before it has been handled.
async fn synced(client: &mut Client, id: u32) {
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(read_op(client, w::OP_MOTION).await, w::motion(id, 0));
}

#[tokio::test]
async fn players_ask_the_staff_and_staff_take_and_answer_the_questions() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, gm_id, mut bob, bob_id, _) = gm_and_player(&backend, addr, 1).await;
    // Nothing waits yet.
    gm.write(command(ap::OP_UQ_COUNT, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_COUNT).await, ap::uq_count(0));
    gm.write(command(ap::OP_UQ_GET, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_GET).await, ap::uq_none());

    bob.write(request_gm(b"where is the blacksmith?")).await;
    synced(&mut bob, bob_id).await;
    gm.write(command(ap::OP_UQ_COUNT, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_COUNT).await, ap::uq_count(1));
    gm.write(command(ap::OP_UQ_GET, &[])).await;
    let question = read_op(&mut gm, ap::OP_UQ_GET).await;
    let payload = frame::payload(&question);
    assert_eq!(question.len() - 2, 35 + 25, "35 + the text with its zero");
    assert_eq!(u16::from_le_bytes([payload[0], payload[1]]), 1, "the waiting count before it was taken");
    let id = u32::from_le_bytes(payload[2..6].try_into().unwrap());
    assert_eq!(payload[10..14], *b"Bob\0");
    assert_eq!(payload[31..33], 25u16.to_le_bytes());
    assert_eq!(&payload[33..], b"where is the blacksmith?\0");
    // The count counts the ones waiting; the question is held, and `/UQGET` again gives it again.
    gm.write(command(ap::OP_UQ_COUNT, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_COUNT).await, ap::uq_count(0));
    gm.write(command(ap::OP_UQ_GET, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_GET).await[2..], question[2..]);
    assert_eq!(backend.questions.guard()[0].taken_by, Some(gm_id));

    // The fourth button: the answer reaches the named player as a whisper from the GM, and the row is answered.
    let mut answer = id.to_le_bytes().to_vec();
    answer.extend_from_slice(&field(b"bob", 21));
    answer.extend_from_slice(&13u16.to_le_bytes());
    answer.extend_from_slice(b"at the forge\0");
    gm.write(command(ap::OP_UQ_ANSWER_TO, &answer)).await;
    assert_eq!(read_op(&mut bob, ap::OP_CHAT_WHISPER).await, chat::whisper_op(ap::OP_CHAT_WHISPER, false, b"Boss", b"at the forge\0"));
    {
        let rows = backend.questions.guard();
        assert_eq!(rows[0].answered, Some((gm_id, 4, b"at the forge\0".to_vec(), Some("Bob".to_owned()))));
    }
    gm.write(command(ap::OP_UQ_GET, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_GET).await, ap::uq_none(), "answered: nothing held, nothing waiting");

    // Three questions are kept at once, the fourth is not counted.
    for text in [&b"one"[..], b"two", b"three", b"four"] {
        bob.write(request_gm(text)).await;
    }
    synced(&mut bob, bob_id).await;
    gm.write(command(ap::OP_UQ_COUNT, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_COUNT).await, ap::uq_count(3));
    assert_eq!(backend.questions.guard().len(), 4, "the refused one is not even stored");
    // A first-three-buttons answer delivers nothing and is saved with its kind.
    gm.write(command(ap::OP_UQ_GET, &[])).await;
    let second = read_op(&mut gm, ap::OP_UQ_GET).await;
    assert_eq!(&frame::payload(&second)[33..], b"one\0");
    let second_id = u32::from_le_bytes(frame::payload(&second)[2..6].try_into().unwrap());
    let mut kind = vec![2];
    kind.extend_from_slice(&second_id.to_le_bytes());
    kind.extend_from_slice(&5u16.to_le_bytes());
    kind.extend_from_slice(b"note\0");
    gm.write(command(ap::OP_UQ_ANSWER, &kind)).await;
    gm.write(command(ap::OP_UQ_COUNT, &[])).await;
    assert_eq!(read_op(&mut gm, ap::OP_UQ_COUNT).await, ap::uq_count(2));
    assert_eq!(backend.questions.guard()[1].answered, Some((gm_id, 2, b"note\0".to_vec(), None)));
    synced(&mut bob, bob_id).await;
}

#[tokio::test]
async fn a_staff_member_leaving_gives_its_question_back() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 1).await;
    bob.write(request_gm(b"help")).await;
    synced(&mut bob, bob_id).await;
    gm.write(command(ap::OP_UQ_GET, &[])).await;
    read_op(&mut gm, ap::OP_UQ_GET).await;
    assert!(backend.questions.guard()[0].taken_by.is_some());
    gm.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert!(read_until_closed(&mut gm).await);
    wait_gone(&backend, 7, Duration::from_secs(2)).await;
    // The question is waiting again, for the next staff member (a restart would find it open too).
    let rows = backend.questions.guard();
    assert_eq!((rows[0].taken_by, rows[0].answered.is_none()), (None, true));
}

#[tokio::test]
async fn staff_cure_players_on_their_map_and_not_the_ones_nobody_has() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 3).await;
    // A name nobody has: the failure (not the GM cured), and for `/event` also the failure whisper.
    gm.write(command(ap::OP_CURE, &field(b"Nobody", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_CURE));
    gm.write(command(ap::OP_HEAL, &field(b"Nobody", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_HEAL));
    assert_eq!(read_op(&mut gm, ap::OP_CHAT_WHISPER).await, ap::gm_cmd_whisper(b"cure failed ->Nobody\0"));
    // Bob: done, with the whisper naming him.
    gm.write(command(ap::OP_CURE, &field(b"bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    gm.write(command(ap::OP_CURE_ALL, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut gm, ap::OP_CHAT_WHISPER).await, ap::gm_cmd_whisper(b"cure, buff,debuff clear ok ->Bob\0"));
    gm.write(command(ap::OP_HEAL, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut gm, ap::OP_CHAT_WHISPER).await, ap::gm_cmd_whisper(b"cure ok ->Bob\0"));
    synced(&mut bob, bob_id).await;

    // Another zone: the GM on map 2 cannot cure Bob on map 1.
    let go = |x: f32, z: f32, map: u16| [&x.to_le_bytes()[..], &z.to_le_bytes(), &map.to_le_bytes()].concat();
    gm.write(command(ap::OP_MOVE_TO_SPOT, &go(100.0, 100.0, 2))).await;
    read_until(&mut gm, ap::OP_ANSWER).await;
    gm.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_until(&mut gm, shaiya_proto::combat::OP_RECOVER_SET).await;
    gm.write(command(ap::OP_CURE, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_CURE));
}

#[tokio::test]
async fn staff_empty_the_bags_and_the_equipment_and_kick() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 3).await;
    assert!(backend.play(bob_id).unwrap().items.iter().any(|i| i.bag >= 1), "the starting potions");
    gm.write(command(ap::OP_CLEAR_BAGS, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut bob, ap::OP_CLEAR_BAGS).await, ap::bare(ap::OP_CLEAR_BAGS));
    let stored = backend.play(bob_id).unwrap();
    assert!(stored.items.iter().all(|i| i.bag == 0), "the bags are empty, the equipment stays: {:?}", stored.items);
    assert!(!stored.items.is_empty());
    gm.write(command(ap::OP_CLEAR_EQUIPMENT, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    let seen = read_until(&mut bob, ap::OP_CLEAR_EQUIPMENT).await;
    assert!(seen.iter().any(|p| frame::opcode(p) == w::OP_EQUIPMENT), "the looks change for the view first: {seen:?}");
    assert!(backend.play(bob_id).unwrap().items.is_empty());
    // A name nobody has, and one's own map only.
    gm.write(command(ap::OP_CLEAR_BAGS, &field(b"Nobody", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_CLEAR_BAGS));

    // `/kick`: Bob is told and the connection ends.
    gm.write(command(ap::OP_KICK, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut bob, ap::OP_KICK).await, ap::bare(ap::OP_KICK));
    assert!(read_until_closed(&mut bob).await);
    gm.write(command(ap::OP_KICK, &field(b"Nobody", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_KICK));
}

#[tokio::test]
async fn kick_and_ban_notices_are_held_before_the_close() {
    // docs/original/small-items.md deviation 1.
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let hold = config(true).kick_notice;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 1).await;
    gm.write(command(ap::OP_KICK, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut bob, ap::OP_KICK).await, ap::bare(ap::OP_KICK));
    let kicked = std::time::Instant::now();
    // Out of the world at once, not after the stay of a lost connection: the GM sees Bob go before the hold ends.
    assert_eq!(read_op(&mut gm, w::OP_USER_LEAVE).await, w::user_leave(bob_id));
    // The account is free during the hold: a newer login gets in.
    backend.add_session(8, session([9; 16], Some(SERVER_ID), LOCAL));
    let mut again = Client::connect(addr).await;
    again.enter(8, [9; 16]).await;
    assert!(kicked.elapsed() < hold / 2, "checked within the hold: {:?}", kicked.elapsed());
    assert!(timeout(hold / 4, bob.read()).await.is_err(), "the kicked socket is still open, with nothing more sent");
    assert_eq!(bob.read().await, None, "closed after the hold");
    assert!(kicked.elapsed() >= hold * 3 / 4, "not before the hold: {:?}", kicked.elapsed());

    // A ban notice is held the same way, also at the character screen.
    let mut ban = 2u16.to_le_bytes().to_vec();
    ban.extend_from_slice(&field(b"bobby", 21));
    gm.write(command(ap::OP_ACCOUNT_BAN, &ban)).await;
    assert_eq!(read_op(&mut again, ap::OP_ACCOUNT_BAN).await, ap::ban_notice(ap::OP_ACCOUNT_BAN, 2));
    let banned = std::time::Instant::now();
    assert!(timeout(hold / 4, again.read()).await.is_err(), "still open");
    assert_eq!(again.read().await, None);
    assert!(banned.elapsed() >= hold * 3 / 4, "not before the hold: {:?}", banned.elapsed());
}

#[tokio::test]
async fn staff_are_protected_from_lower_ranked_staff() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_other(&backend, addr, 2, 1).await;
    for opcode in [ap::OP_KICK, ap::OP_CLEAR_BAGS, ap::OP_CLEAR_EQUIPMENT] {
        gm.write(command(opcode, &field(b"Bob", 21))).await;
        assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(opcode), "{opcode:04X}");
    }
    gm.write(command(ap::OP_QUEST_ADD, &[field(b"Bob", 21), 4u16.to_le_bytes().to_vec()].concat())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUEST_ADD));
    // The same through the account: `/akick`.
    gm.write(command(ap::OP_ACCOUNT_KICK, &field(b"bobby", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_ACCOUNT_KICK));
    synced(&mut bob, bob_id).await;
    assert!(!backend.play(bob_id).unwrap().items.is_empty());
    // The higher rank is no protection for the GM: Bob (level 1) kicks Boss (level 2).
    bob.write(command(ap::OP_KICK, &field(b"Boss", 21))).await;
    assert_eq!(read_op(&mut bob, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut gm, ap::OP_KICK).await, ap::bare(ap::OP_KICK));
    assert!(read_until_closed(&mut gm).await);
}

#[tokio::test]
async fn staff_give_and_take_quests_and_see_the_character_sheet() {
    use shaiya_proto::quest as qp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 3).await;
    let add = |quest: u16| command(ap::OP_QUEST_ADD, &[field(b"Bob", 21), quest.to_le_bytes().to_vec()].concat());
    let remove = |quest: u16| command(ap::OP_QUEST_REMOVE, &[field(b"Bob", 21), quest.to_le_bytes().to_vec()].concat());
    // An unknown quest, and one not in progress to take away.
    gm.write(add(99)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUEST_ADD));
    gm.write(remove(4)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUEST_REMOVE));
    // Quest 4: Bob gets 0x0902 with no NPC and the start item.
    gm.write(add(4)).await;
    assert_eq!(read_op(&mut bob, qp::OP_START).await, qp::started(0, 4));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    let stored = backend.play(bob_id).unwrap();
    assert!(stored.items.iter().any(|i| (i.item_type, i.type_id) == (100, 48)));
    assert_eq!(stored.quests.iter().map(|q| (q.quest_id, q.finished)).collect::<Vec<_>>(), [(4, false)]);
    gm.write(add(4)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUEST_ADD), "in progress");

    // `/quiry`: the answer first, then the sheet with the sword.
    gm.write(command(ap::OP_QUIRY, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    let sheet = gm.read().await.unwrap();
    assert_eq!(frame::opcode(&sheet), ap::OP_QUIRY);
    assert_eq!(sheet.len() - 2, 37 + 32);
    assert_eq!(frame::payload(&sheet)[1..3], 1u16.to_le_bytes(), "level 1");
    gm.write(command(ap::OP_QUIRY, &field(b"Nobody", 21))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_QUIRY));

    // Taken away: Bob is told it failed, the item goes, the row is deleted and it can be given again.
    gm.write(remove(4)).await;
    assert_eq!(read_op(&mut bob, qp::OP_END).await, qp::ended(&qp::Ended { quest_id: 4, ..Default::default() }));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    let stored = backend.play(bob_id).unwrap();
    assert!(stored.items.iter().all(|i| (i.item_type, i.type_id) != (100, 48)));
    assert!(stored.quests.is_empty());
    gm.write(add(4)).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
}

#[tokio::test]
async fn staff_summon_players_to_a_spot() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, bob_id, _) = gm_and_player(&backend, addr, 4).await;
    let go = |x: f32, z: f32, map: u16| [&x.to_le_bytes()[..], &z.to_le_bytes(), &map.to_le_bytes()].concat();
    let summon_to = |name: &[u8], x: f32, z: f32, map: u16| [field(name, 21), go(x, z, map)].concat();
    // `/asummon`: Bob arrives where the GM stands, told with the command's packet.
    gm.write(command(ap::OP_MOVE_TO_SPOT, &go(700.0, 1700.0, 1))).await;
    read_until(&mut gm, ap::OP_ANSWER).await;
    gm.write(command(ap::OP_SUMMON, &field(b"Bob", 21))).await;
    assert_eq!(read_op(&mut bob, ap::OP_SUMMON).await, ap::summoned(ap::OP_SUMMON, bob_id, 1, [700.0, 0.0, 1700.0]));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    // `/bsummon`: the spot named, y 0; a map that does not exist, or a spot off the map: the failure.
    gm.write(command(ap::OP_SUMMON_TO, &summon_to(b"Bob", 800.0, 1650.0, 1))).await;
    assert_eq!(read_op(&mut bob, ap::OP_SUMMON_TO).await, ap::summoned(ap::OP_SUMMON_TO, bob_id, 1, [800.0, 0.0, 1650.0]));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    for (map, x) in [(999, 100.0), (1, -5.0), (200, 100.0)] {
        gm.write(command(ap::OP_SUMMON_TO, &summon_to(b"Bob", x, 100.0, map))).await;
        assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_SUMMON_TO), "map {map} x {x}");
    }
    gm.write(command(ap::OP_SUMMON_TO, &summon_to(b"Nobody", 100.0, 100.0, 1))).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_SUMMON_TO));
    // `/partysummon` for a player without a party: Bob alone, to another map this time.
    gm.write(command(ap::OP_SUMMON_PARTY, &summon_to(b"Bob", 165.0, 397.0, 2))).await;
    assert_eq!(read_op(&mut bob, ap::OP_SUMMON_PARTY).await, ap::summoned(ap::OP_SUMMON_PARTY, bob_id, 2, [165.0, 0.0, 397.0]));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
}

#[tokio::test]
async fn staff_notices_reach_who_they_name_and_repeat_until_stopped() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut gm, _, mut bob, _, _) = gm_and_player(&backend, addr, 4).await;
    let text = b"hello all\0";
    let notice = |n: &[u8], op: u16| ap::notice(op, n);
    let repeat = |seconds: u16, message: &[u8]| [&seconds.to_le_bytes()[..], &[message.len() as u8], message].concat();
    // `/gmnotice` is for the staff: the GM gets it, Bob does not.
    gm.write(command(ap::OP_NOTICE_STAFF, &[&[text.len() as u8][..], text].concat())).await;
    // The answer is sent at once, the notice follows through the zone.
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    assert_eq!(read_op(&mut gm, ap::OP_NOTICE_STAFF).await, notice(text, ap::OP_NOTICE_STAFF));
    // A length of 1 is no notice for it.
    gm.write(command(ap::OP_NOTICE_STAFF, &[1, 0])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_NOTICE_STAFF));
    // `/notice`: everyone.
    gm.write(command(ap::OP_NOTICE_ALL, &repeat(0, text))).await;
    let seen = read_until(&mut bob, ap::OP_NOTICE_ALL).await;
    assert!(seen.iter().all(|p| frame::opcode(p) != ap::OP_NOTICE_STAFF), "Bob never got the staff notice");
    assert_eq!(seen.last().unwrap(), &notice(text, ap::OP_NOTICE_ALL));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    // `/cnotice 1`: Bob, in view, gets it now and again a second later; the "off" form stops it.
    gm.write(command(ap::OP_NOTICE_VIEW, &repeat(1, text))).await;
    assert_eq!(read_op(&mut bob, ap::OP_NOTICE_VIEW).await, notice(text, ap::OP_NOTICE_VIEW));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    let again = timeout(Duration::from_millis(2500), read_op(&mut bob, ap::OP_NOTICE_VIEW)).await;
    assert_eq!(again.expect("a second notice within 2.5 s"), notice(text, ap::OP_NOTICE_VIEW));
    gm.write(command(ap::OP_NOTICE_VIEW, &[0, 0, 0])).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    // A repeat already due when it stopped may still be on its way; the next one must not come.
    let _ = timeout(Duration::from_millis(1500), read_op(&mut bob, ap::OP_NOTICE_VIEW)).await;
    assert!(timeout(Duration::from_millis(2500), read_op(&mut bob, ap::OP_NOTICE_VIEW)).await.is_err(), "stopped");
    // `/wnotice` to Bob by name, `/znotice` to the map, `/bnotice` to a faction (0 or 1 only).
    gm.write(command(ap::OP_NOTICE_TO, &[field(b"Bob", 21), repeat(0, text)].concat())).await;
    assert_eq!(read_op(&mut bob, ap::OP_NOTICE_TO).await, notice(text, ap::OP_NOTICE_TO));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    gm.write(command(ap::OP_NOTICE_ZONE, &repeat(0, text))).await;
    assert_eq!(read_op(&mut bob, ap::OP_NOTICE_ZONE).await, notice(text, ap::OP_NOTICE_ZONE));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    gm.write(command(ap::OP_NOTICE_COUNTRY, &[&[0u8][..], &repeat(0, text)].concat())).await;
    assert_eq!(read_op(&mut bob, ap::OP_NOTICE_COUNTRY).await, notice(text, ap::OP_NOTICE_COUNTRY));
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(0));
    gm.write(command(ap::OP_NOTICE_COUNTRY, &[&[2u8][..], &repeat(0, text)].concat())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_NOTICE_COUNTRY));
    gm.write(command(ap::OP_NOTICE_TO, &[field(b"Nobody", 21), repeat(0, text)].concat())).await;
    assert_eq!(read_op(&mut gm, ap::OP_ANSWER).await, ap::answer(ap::OP_NOTICE_TO));
}

#[tokio::test]
async fn the_auction_house_registers_sells_and_pays_out() {
    use game_server::backend::CharacterStore;
    use shaiya_proto::market as mp;
    let backend = Arc::new(MemoryBackend::default());
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    backend.add_session(8, session([6; 16], Some(SERVER_ID), LOCAL));
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut alice = Client::connect(addr).await;
    alice.enter(7, [5; 16]).await;
    alice.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    alice.write(create(0, 0, 0, b"Alice")).await;
    alice.read().await.unwrap();
    alice.read_list().await;
    let mut bob = Client::connect(addr).await;
    bob.enter(8, [6; 16]).await;
    bob.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    bob.write(create(0, 0, 0, b"Bob")).await;
    bob.read().await.unwrap();
    bob.read_list().await;
    let (alice_id, bob_id) = (1, 2);
    backend.set_money(alice_id, 1000);
    backend.set_money(bob_id, 9000);
    // Carol, on map 1: nothing answers there.
    backend.add_session(9, session([7; 16], Some(SERVER_ID), LOCAL));
    let mut carol = Client::connect(addr).await;
    carol.enter(9, [7; 16]).await;
    carol.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    carol.write(create(0, 0, 0, b"Carol")).await;
    carol.read().await.unwrap();
    carol.read_list().await;
    carol.write(g::char_id_request(g::OP_CHAR_SELECT, 3)).await;
    while frame::opcode(&carol.read().await.unwrap()) != w::OP_CHAR_QUICK {}
    carol.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(&mut carol).await;
    carol.write(shaiya_proto::encode(mp::OP_SELL_LIST, &[])).await;
    carol.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert_eq!(carol.read().await.unwrap(), w::motion(3, 0), "no answer off map 42");
    for (client, id) in [(&mut alice, alice_id), (&mut bob, bob_id)] {
        ctx.store.save_position(id, 42, [50.0, 1.0, 50.0], 0, [250, 85, 195]).await.unwrap();
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
        client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    }

    // Alice registers 3 of her 10 potions for 24 h: the fee is 120.
    let register = [1, 0, 3, 1, 0xE8, 3, 0, 0, 0x88, 0x13, 0, 0];
    alice.write(shaiya_proto::encode(mp::OP_REGISTER, &register)).await;
    let registered = read_op(&mut alice, mp::OP_REGISTER).await;
    let b = frame::payload(&registered);
    assert_eq!((b.len(), &b[..4], u32::from_le_bytes(b[4..8].try_into().unwrap())), (60, &[0, 1, 0, 3][..], 880));
    let auction = u32::from_le_bytes(b[8..12].try_into().unwrap());

    // Bob finds it, bids too low then at the next minimum; Alice is told of the bid.
    bob.write(shaiya_proto::encode(mp::OP_SAME_ITEM, &[25, 1])).await;
    let found = read_op(&mut bob, mp::OP_SAME_ITEM).await;
    assert_eq!(
        (&frame::payload(&found)[..3], u32::from_le_bytes(frame::payload(&found)[3..7].try_into().unwrap())),
        (&[0, 0, 1][..], auction)
    );
    let bid = |price: u32| shaiya_proto::encode(mp::OP_BID, &[auction.to_le_bytes(), price.to_le_bytes()].concat());
    bob.write(bid(1000)).await;
    assert_eq!(read_op(&mut bob, mp::OP_BID).await, mp::bid_too_low(auction, 1050));
    bob.write(bid(1050)).await;
    let bid_ok = read_op(&mut bob, mp::OP_BID).await;
    assert_eq!((frame::payload(&bid_ok)[4], u32::from_le_bytes(frame::payload(&bid_ok)[5..9].try_into().unwrap())), (0, 7950));
    assert_eq!(read_op(&mut alice, mp::OP_BID_ON).await, mp::bid_on(auction, 1050, 25, 1, 3));

    // Bob buys it now: he pays 5000 and gets his bid back to collect; Alice's sale pays 120 - 100 + 5000.
    bob.write(shaiya_proto::encode(mp::OP_BUY, &auction.to_le_bytes())).await;
    let bought = read_op(&mut bob, mp::OP_BUY).await;
    assert_eq!((frame::payload(&bought)[0], u32::from_le_bytes(frame::payload(&bought)[1..5].try_into().unwrap())), (0, 2950));
    let refund = read_op(&mut bob, mp::OP_OUTBID).await;
    let sold = read_op(&mut alice, mp::OP_SOLD).await;
    assert_eq!(u32::from_le_bytes(frame::payload(&sold)[9..13].try_into().unwrap()), 5020);

    // Collecting: the potions into Bob's first free place, the gold to both.
    bob.write(shaiya_proto::encode(mp::OP_END_ITEMS, &[])).await;
    assert_eq!(frame::payload(&read_op(&mut bob, mp::OP_END_ITEMS).await)[0], 1);
    bob.write(shaiya_proto::encode(mp::OP_GET_ITEM, &auction.to_le_bytes())).await;
    let got = read_op(&mut bob, mp::OP_GET_ITEM).await;
    assert_eq!(&frame::payload(&got)[..10], &[[0].as_slice(), &auction.to_le_bytes(), &[1, 1, 25, 1, 3]].concat()[..]);
    let money_id = |row: &[u8]| u32::from_le_bytes(frame::payload(row)[..4].try_into().unwrap());
    bob.write(shaiya_proto::encode(mp::OP_GET_MONEY, &money_id(&refund).to_le_bytes())).await;
    assert_eq!(read_op(&mut bob, mp::OP_GET_MONEY).await, mp::got_money(Some((money_id(&refund), 4000))));
    alice.write(shaiya_proto::encode(mp::OP_END_MONEYS, &[])).await;
    assert_eq!(frame::payload(&read_op(&mut alice, mp::OP_END_MONEYS).await)[0], 1);
    alice.write(shaiya_proto::encode(mp::OP_GET_MONEY, &money_id(&sold).to_le_bytes())).await;
    assert_eq!(read_op(&mut alice, mp::OP_GET_MONEY).await, mp::got_money(Some((money_id(&sold), 5900))));
    alice.write(shaiya_proto::encode(mp::OP_GET_MONEY, &money_id(&sold).to_le_bytes())).await;
    assert_eq!(read_op(&mut alice, mp::OP_GET_MONEY).await, mp::got_money(None), "collected once");

    // Saved as it happened.
    let (money, items) = backend.stored(alice_id).unwrap();
    assert_eq!((money, items.iter().find(|i| (i.bag, i.slot) == (1, 0)).map(|i| i.count)), (5900, Some(7)));
    let (money, items) = backend.stored(bob_id).unwrap();
    assert_eq!((money, items.iter().find(|i| (i.bag, i.slot) == (1, 1)).map(|i| (i.item_type, i.count))), (4000, Some((25, 3))));
    assert!(backend.market.guard().auctions.is_empty());
}

// Small items (`docs/original/small-items.md`).

/// Alice and Bob (Light, human fighters side by side on map 1) and Carol (Fury, a vail ranger on map 2) in the world, with
/// `alice_items` (type, type id) put in Alice's bag 1 from slot 2 on before she enters. Returns the clients and their ids.
async fn three_in_the_world(backend: &Arc<MemoryBackend>, addr: SocketAddr, alice_items: &[(u8, u8)]) -> ([Client; 3], [u32; 3]) {
    use game_server::backend::ItemStore;
    use game_server::inventory::{NewItem, SlotChange};
    let mut clients = Vec::new();
    let mut ids = Vec::new();
    for (user, identity, name, country, family, job) in
        [(7, [5; 16], &b"Alice"[..], 0, 0, 0), (8, [6; 16], b"Bob", 0, 0, 0), (9, [7; 16], b"Carol", 1, 2, 2)]
    {
        backend.add_session(user, session(identity, Some(SERVER_ID), LOCAL));
        let mut client = Client::connect(addr).await;
        client.enter(user, identity).await;
        client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[country])).await;
        client.write(create(0, family, job, name)).await;
        assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
        ids.push(u32::from_le_bytes(client.read_list().await[4][1..5].try_into().unwrap()));
        clients.push(client);
    }
    let changes: Vec<_> = (2..)
        .zip(alice_items)
        .map(|(slot, &(item_type, type_id))| {
            let item_id = u32::from(item_type) * 1000 + u32::from(type_id);
            SlotChange::Insert(NewItem { bag: 1, slot, item_id, item_type, type_id, quality: 0, count: 1, kept: None })
        })
        .collect();
    assert!(backend.trade(ids[0], 0, &changes).await.unwrap().is_some());
    for (client, &id) in clients.iter_mut().zip(&ids) {
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
        client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
        read_zone_entry(client).await;
    }
    // Each one's own motion answer says the entries are through (a neighbour's comes to the view as well).
    for (client, &id) in clients.iter_mut().zip(&ids) {
        client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
        while client.read().await.unwrap() != w::motion(id, 0) {}
    }
    (clients.try_into().ok().unwrap(), ids.try_into().unwrap())
}

/// A chat line as the client sends it: `u8 len`, the text with its zero.
fn say(opcode: u16, text: &[u8]) -> Vec<u8> {
    let mut payload = vec![text.len() as u8 + 1];
    payload.extend_from_slice(text);
    payload.push(0);
    shaiya_proto::encode(opcode, &payload)
}

/// The next chat packet (`0x11xx`), skipping the rest.
async fn next_chat(client: &mut Client) -> Vec<u8> {
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) & 0xFF00 == 0x1100 {
            return packet;
        }
    }
}

#[tokio::test]
async fn message_to_server_sends_the_next_normal_line_to_everyone() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let ([mut alice, mut bob, mut carol], [alice_id, _, _]) = three_in_the_world(&backend, addr, &[(100, 64), (100, 64)]).await;
    // Armed and used up; a second one while armed is refused and kept.
    alice.write(shaiya_proto::encode(it::OP_USE, &[1, 2])).await;
    assert_eq!(read_op(&mut alice, it::OP_USE).await, it::used(alice_id, 1, 2, 100, 64, 0));
    alice.write(shaiya_proto::encode(it::OP_USE, &[1, 3])).await;
    assert_eq!(read_op(&mut alice, it::OP_USE).await, it::use_failed(alice_id));
    assert!(backend.play(alice_id).unwrap().items.iter().any(|i| (i.bag, i.slot, i.type_id) == (1, 3, 64)), "the second one is kept");

    // The next normal line goes to every player, the other faction on another map included, as 0x1108; the sender gets the same
    // 0x1108 and nobody a 0x1101.
    alice.write(say(chat::OP_NORMAL, b"hello all")).await;
    let everyone = chat::by_name(chat::OP_ALL_SERVER, b"Alice", b"hello all\0");
    for client in [&mut alice, &mut bob, &mut carol] {
        assert_eq!(next_chat(client).await, everyone);
    }
    // Disarmed: the next line is normal chat again, which Bob gets first (no 0x1101 of the line before).
    alice.write(say(chat::OP_NORMAL, b"ok")).await;
    assert_eq!(next_chat(&mut bob).await, chat::by_id(chat::OP_NORMAL, alice_id, b"ok\0"));
    assert_eq!(next_chat(&mut alice).await, chat::by_id(chat::OP_NORMAL, alice_id, b"ok\0"));

    // A client-sent 0x1108 without the item is ignored.
    alice.write(say(chat::OP_ALL_SERVER, b"nope")).await;
    alice.write(say(chat::OP_NORMAL, b"still")).await;
    assert_eq!(next_chat(&mut bob).await, chat::by_id(chat::OP_NORMAL, alice_id, b"still\0"));
    // With the item armed it goes out (and starts the 30 s cooldown).
    alice.write(shaiya_proto::encode(it::OP_USE, &[1, 3])).await;
    assert_eq!(read_op(&mut alice, it::OP_USE).await, it::used(alice_id, 1, 3, 100, 64, 0));
    alice.write(say(chat::OP_ALL_SERVER, b"once")).await;
    assert_eq!(next_chat(&mut carol).await, chat::by_name(chat::OP_ALL_SERVER, b"Alice", b"once\0"));
}

#[tokio::test]
async fn town_moves_to_the_bind_position_after_standing_still() {
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let delay = config(true).town_delay;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    let town = || shaiya_proto::encode(it::OP_TOWN, &[]);
    // Never answered; the move comes after the delay, as 0x020B for the player.
    client.write(town()).await;
    let asked = std::time::Instant::now();
    let moved = read_op(&mut client, w::OP_POSITION).await;
    assert!(asked.elapsed() >= delay * 3 / 4, "not before the delay: {:?}", asked.elapsed());
    assert_eq!(frame::payload(&moved)[..4], id.to_le_bytes());

    // A second request while one is pending changes nothing: one move.
    client.write(town()).await;
    client.write(town()).await;
    read_op(&mut client, w::OP_POSITION).await;
    tokio::time::sleep(delay + delay / 2).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    let seen = read_until(&mut client, w::OP_MOTION).await;
    assert!(seen.iter().all(|p| frame::opcode(p) != w::OP_POSITION), "one move: {seen:?}");

    // A step cancels it without a word: no move, no 0x0222.
    let bind = shaiya_proto::frame::payload(&moved)[6..18].to_vec();
    let at = |at: usize| f32::from_le_bytes(bind[at..at + 4].try_into().unwrap());
    client.write(town()).await;
    client.write(step(at(0) + 5.0, at(8))).await;
    tokio::time::sleep(delay + delay / 2).await;
    client.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    let seen = read_until(&mut client, w::OP_MOTION).await;
    assert!(seen.iter().any(|p| frame::opcode(p) == w::OP_MOVE), "the step was taken: {seen:?}");
    assert!(seen.iter().all(|p| ![w::OP_POSITION, it::OP_MOVE_END].contains(&frame::opcode(p))), "cancelled silently: {seen:?}");
}

#[tokio::test]
async fn equipment_vg_and_the_priest() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let ([mut alice, mut bob, mut carol], [alice_id, bob_id, _]) = three_in_the_world(&backend, addr, &[]).await;
    // Bob looks at Alice's equipment: the list comes from her connection, in slot order.
    bob.write(g::char_id_request(w::OP_USER_ITEMS, alice_id)).await;
    let mut worn: Vec<_> = backend.play(alice_id).unwrap().items.into_iter().filter(|i| i.bag == 0).collect();
    worn.sort_by_key(|i| i.slot);
    assert!(!worn.is_empty(), "the starting sword");
    assert_eq!(read_op(&mut bob, w::OP_USER_ITEMS).await, w::user_items(&worn));
    // Carol, of the other country on another map, gets nothing: her next answer is the one to her motion.
    carol.write(g::char_id_request(w::OP_USER_ITEMS, alice_id)).await;
    carol.write(shaiya_proto::encode(w::OP_MOTION, &[0])).await;
    assert!(read_until(&mut carol, w::OP_MOTION).await.iter().all(|p| frame::opcode(p) != w::OP_USER_ITEMS));

    // The VG, always answered.
    alice.write(shaiya_proto::encode(it::OP_VG, &[])).await;
    assert_eq!(read_op(&mut alice, it::OP_VG).await, it::vg(0));

    // The Priest: more than 10 gold needed; 10 are taken. Alice has 11, Bob 10.
    use shaiya_proto::priest;
    drop((alice, bob));
    for (user, id) in [(7, alice_id), (8, bob_id)] {
        wait_gone(&backend, user, Duration::from_secs(3)).await;
        backend.set_money(id, if id == alice_id { 11 } else { 10 });
    }
    let mut again = Vec::new();
    for (user, identity, id) in [(7, [8; 16], alice_id), (8, [9; 16], bob_id)] {
        backend.add_session(user, session(identity, Some(SERVER_ID), LOCAL));
        let mut client = Client::connect(addr).await;
        client.enter(user, identity).await;
        client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
        while frame::opcode(&client.read().await.unwrap()) != w::OP_CHAR_QUICK {}
        client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
        read_zone_entry(&mut client).await;
        again.push(client);
    }
    let [mut alice, mut bob]: [Client; 2] = again.try_into().ok().unwrap();
    alice.write(shaiya_proto::encode(priest::OP_PRIEST_START, &[])).await;
    assert_eq!(read_op(&mut alice, priest::OP_PRIEST_START).await, priest::priest_start(Some(priest::Reading::default())));
    alice.write(shaiya_proto::encode(priest::OP_PRIEST_NEXT, &[])).await;
    assert_eq!(read_op(&mut alice, priest::OP_PRIEST_NEXT).await, priest::priest_next(false), "1 gold left");
    bob.write(shaiya_proto::encode(priest::OP_PRIEST_START, &[])).await;
    assert_eq!(read_op(&mut bob, priest::OP_PRIEST_START).await, priest::priest_start(None));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!((backend.play(alice_id).unwrap().money, backend.play(bob_id).unwrap().money), (1, 10));
    // Another 0x0Fxx closes the connection.
    bob.write(shaiya_proto::encode(0x0F03, &[])).await;
    assert!(read_until_closed(&mut bob).await);
}

#[tokio::test]
async fn item_skills_hit_only_what_may_be_attacked_within_range() {
    // docs/original/small-items.md section 11, deviation 7.
    use shaiya_proto::skill as sk;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let ([mut alice, _bob, _carol], [alice_id, bob_id, _]) = three_in_the_world(&backend, addr, &[(44, 147)]).await;
    // One per area: 1 beside the start, 2 far away.
    assert_eq!(ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0), 2);
    let throw = |kind: u8, target: u32| shaiya_proto::encode(it::OP_USE_SKILL, &[&[1, 2, kind][..], &target.to_le_bytes()].concat());
    // Refused, the item kept: Bob (her own side), the monster far away, an unknown kind.
    for (kind, target) in [(it::KIND_PLAYER, bob_id), (it::KIND_MOB, 2), (3, 1)] {
        alice.write(throw(kind, target)).await;
        assert_eq!(read_op(&mut alice, it::OP_USE).await, it::use_failed(alice_id), "kind {kind} on {target}");
    }
    // The monster beside her: one area hit (0x0519), then the item is used up.
    alice.write(throw(it::KIND_MOB, 1)).await;
    let seen = read_until(&mut alice, it::OP_USE).await;
    assert!(seen.iter().any(|p| frame::opcode(p) == sk::OP_AREA_MOB), "{seen:?}");
    assert_eq!(seen.last().unwrap(), &it::used(alice_id, 1, 2, 44, 147, 0));
}

#[tokio::test]
async fn the_name_change_stone_allows_one_rename_at_the_character_screen() {
    // docs/original/small-items.md section 9.4, deviations 3 to 6.
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let ([mut alice, _bob, _carol], [alice_id, _, _]) = three_in_the_world(&backend, addr, &[(100, 136), (100, 136)]).await;
    // Once per world entry: the second stone is refused and kept.
    alice.write(shaiya_proto::encode(it::OP_USE, &[1, 2])).await;
    assert_eq!(read_op(&mut alice, it::OP_USE).await, it::used(alice_id, 1, 2, 100, 136, 0));
    alice.write(shaiya_proto::encode(it::OP_USE, &[1, 3])).await;
    assert_eq!(read_op(&mut alice, it::OP_USE).await, it::use_failed(alice_id));
    assert_eq!(backend.renames(alice_id), Some((1, Vec::new())));

    // At the character screen the rename is offered (the list record's byte after the delete flag).
    /// Sits, logs out to the character screen and returns the character's list record.
    async fn to_select(client: &mut Client, id: u32) -> Vec<u8> {
        client.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
        while client.read().await.unwrap() != w::motion(id, 1) {}
        client.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
        read_op(client, w::OP_LOGOUT_TO_SELECT).await;
        assert_eq!(frame::opcode(&client.read().await.unwrap()), g::OP_CHAR_COUNTRY);
        client.read_list().await.into_iter().find(|r| r[1..5] == id.to_le_bytes()).unwrap()
    }
    let record = to_select(&mut alice, alice_id).await;
    assert_eq!((&record[54..59], record[74]), (&b"Alice"[..], 1));
    let rename = |name: &[u8]| shaiya_proto::encode(g::OP_CHAR_RENAME, &[&alice_id.to_le_bytes()[..], &field(name, 21)].concat());
    // The creation rule (too short, not letters and digits), and a taken name in another case: refused, the rename kept.
    for name in [&b"x"[..], b"Bad_Name", b"bob"] {
        alice.write(rename(name)).await;
        assert_eq!(alice.read().await.unwrap(), g::char_rename_response(rename_result::FAILED), "{}", String::from_utf8_lossy(name));
    }
    alice.write(rename(b"Ranger2")).await;
    assert_eq!(alice.read().await.unwrap(), g::char_rename_response(rename_result::SUCCESS));
    assert_eq!(backend.renames(alice_id), Some((0, vec![("Alice".into(), "Ranger2".into())])));
    alice.write(rename(b"Ranger3")).await;
    assert_eq!(alice.read().await.unwrap(), g::char_rename_response(rename_result::FAILED), "no rename left");

    // The next list shows the new name and no rename.
    alice.write(g::char_id_request(g::OP_CHAR_SELECT, alice_id)).await;
    while frame::opcode(&alice.read().await.unwrap()) != w::OP_CHAR_QUICK {}
    alice.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(&mut alice).await;
    let record = to_select(&mut alice, alice_id).await;
    assert_eq!((&record[54..62], record[74]), (&b"Ranger2\0"[..], 0));
}

/// The account's points as stored in the memory backend.
fn set_points(backend: &MemoryBackend, user_id: u32, points: u32) {
    backend.mall.guard().points.insert(user_id, points);
}

#[tokio::test]
async fn buying_in_the_item_mall() {
    use shaiya_proto::point as pp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    set_points(&backend, 7, 400);
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    assert_eq!(client.billing, [0, 0, 0, 0, 0], "no premium");

    // 150 potions (stacks of 100) and the stone into the empty places after the starting potions (bag 1 slot 0).
    let mut code = b"Potions".to_vec();
    code.resize(21, 0);
    client.write(shaiya_proto::encode(pp::OP_BUY, &code)).await;
    let answer = client.read().await.unwrap();
    let p = frame::payload(&answer);
    assert_eq!((frame::opcode(&answer), p.len(), p[0], &p[1..5]), (pp::OP_BUY, 35 + 15, 0, &100u32.to_le_bytes()[..]));
    assert_eq!(&p[5..12], b"Potions");
    assert_eq!(&p[30..34], &300u32.to_le_bytes(), "the log row's points");
    assert_eq!(&p[34..], &[3, 1, 1, 25, 1, 100, 1, 2, 25, 1, 50, 1, 3, 100, 48, 1]);
    let (_, items) = backend.stored(id).unwrap();
    let mut slots: Vec<_> = items.iter().filter(|i| i.bag == 1).map(|i| (i.slot, i.item_type, i.count)).collect();
    slots.sort();
    assert_eq!(slots, [(0, 25, 10), (1, 25, 100), (2, 25, 50), (3, 100, 1)]);
    assert_eq!(backend.mall.guard().points[&7], 100);
    let log: Vec<_> = backend.mall.guard().log.iter().map(|l| (l.0, l.1, l.2, l.3.code.clone(), l.3.points)).collect();
    assert_eq!(log, [(7, id, 1, "Potions".to_owned(), 300)]);

    // Short of points: 2; an unknown code: 1; the points refreshed: 100.
    client.write(shaiya_proto::encode(pp::OP_BUY, &code)).await;
    assert_eq!(client.read().await.unwrap(), pp::buy_refused(pp::BUY_NO_POINTS));
    client.write(shaiya_proto::encode(pp::OP_BUY, &[b'x'; 21])).await;
    assert_eq!(client.read().await.unwrap(), pp::buy_refused(pp::BUY_NO_PRODUCT));
    client.write(shaiya_proto::encode(pp::OP_RELOAD, &[])).await;
    assert_eq!(client.read().await.unwrap(), pp::reload(100));
    // Within 40 s a refresh gives what the connection holds; points the operator changes show at once (deviation 5).
    set_points(&backend, 7, 5000);
    client.write(shaiya_proto::encode(pp::OP_RELOAD, &[])).await;
    assert_eq!(client.read().await.unwrap(), pp::reload(100), "held for 40 s");
    assert!(ctx.online.points_changed(7));
    assert_eq!(client.read().await.unwrap(), pp::reload(5000));
    assert!(!ctx.online.points_changed(8), "not in the world");

    // The console's `shop off` answers the failure, `shop silent` nothing at all (`docs/original/operator-console.md` section 2).
    ctx.switches.set_shop(game_server::switches::SHOP_OFF);
    client.write(shaiya_proto::encode(pp::OP_BUY, &[b'x'; 21])).await;
    assert_eq!(client.read().await.unwrap(), pp::buy_refused(pp::BUY_NO_PRODUCT));
    ctx.switches.set_shop(game_server::switches::SHOP_SILENT);
    client.write(shaiya_proto::encode(pp::OP_BUY, &code)).await;
    client.write(shaiya_proto::encode(pp::OP_RELOAD, &[])).await;
    assert_eq!(client.read().await.unwrap(), pp::reload(5000), "the purchase got no answer: the next packet is the reload's");
    assert_eq!(backend.mall.guard().points[&7], 5000, "and bought nothing");
}

#[tokio::test]
async fn gifts_and_the_storages() {
    use shaiya_proto::point as pp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(&backend, addr).await;
    set_points(&backend, 7, 170);
    alice.write(shaiya_proto::encode(pp::OP_RELOAD, &[])).await;
    assert_eq!(read_op(&mut alice, pp::OP_RELOAD).await, pp::reload(170));
    let gift = |code: &[u8], to: &[u8]| {
        let mut p = code.to_vec();
        p.resize(21, 0);
        p.extend_from_slice(to);
        p.resize(42, 0);
        shaiya_proto::encode(pp::OP_GIFT, &p)
    };

    // To Bob, who is in the world: Alice pays, Bob's storage gets the stone in slot 0 and he is told at once.
    alice.write(gift(b"Stone", b"bob")).await;
    let answer = read_op(&mut alice, pp::OP_GIFT).await;
    let p = frame::payload(&answer);
    assert_eq!((p.len(), p[0], &p[1..4], &p[22..26]), (55, pp::GIFT_OK, &b"bob"[..], &120u32.to_le_bytes()[..]));
    assert_eq!(&p[26..31], b"Stone");
    let stored = pp::Stored { slot: 0, item_type: 100, type_id: 48, count: 1 };
    assert_eq!(read_op(&mut bob, pp::OP_GIFT_RECEIVED).await, pp::gift_received(b"Alice", b"Stone", &[stored]));
    assert_eq!(backend.mall.guard().log.last().map(|l| (l.0, l.1, l.2, l.4.clone())), Some((7, alice_id, 2, "bob".to_owned())));

    // Refused: herself (her own account), nobody of that name, an undefined item (6, and nothing stays locked), too few points.
    for (code, to, result) in [
        (&b"Stone"[..], &b"Alice"[..], pp::GIFT_OWN_ACCOUNT),
        (b"Stone", b"Nobody", pp::GIFT_NO_CHARACTER),
        (b"Bad", b"Bob", pp::GIFT_NO_PRODUCT),
        (b"Potions", b"Bob", pp::GIFT_NO_POINTS),
    ] {
        alice.write(gift(code, to)).await;
        let answer = read_op(&mut alice, pp::OP_GIFT).await;
        assert_eq!((frame::payload(&answer)[0], frame::payload(&answer).len()), (result, 55), "{}", String::from_utf8_lossy(to));
    }
    alice.write(gift(b"Stone", b"Bob")).await;
    assert_eq!(frame::payload(&read_op(&mut alice, pp::OP_GIFT).await)[0], pp::GIFT_OK, "the lock was not left set");
    assert_eq!(read_op(&mut bob, pp::OP_GIFT_RECEIVED).await, pp::gift_received(b"Alice", b"Stone", &[pp::Stored { slot: 1, ..stored }]));

    // Bob takes slot 0 into his first empty place (bag 1 slot 1, after the starting potions); an empty slot gives nothing.
    bob.write(shaiya_proto::encode(pp::OP_STORED_GET, &[0])).await;
    assert_eq!(read_op(&mut bob, pp::OP_STORED_GET).await, pp::taken(pp::OP_STORED_GET, 0, Some((1, 1))));
    bob.write(shaiya_proto::encode(pp::OP_STORED_GET, &[0])).await;
    assert_eq!(read_op(&mut bob, pp::OP_STORED_GET).await, pp::taken(pp::OP_STORED_GET, 0, None));
    let bob_items = backend.stored(bob_id).unwrap().1;
    assert!(bob_items.iter().any(|i| (i.bag, i.slot, i.item_type, i.type_id) == (1, 1, 100, 48)));
    assert_eq!(
        backend.mall.guard().stored[&(8, SERVER_ID)],
        [game_server::backend::StoredPointItem { slot: 1, item_id: 100048, count: 1 }]
    );

    // Bob leaves; the next gift waits as a notice, shown (one 0x2604 without rows) when he enters the world again.
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    bob.write(w::logout_done(w::OP_LOGOUT_TO_SELECT)).await;
    read_op(&mut bob, w::OP_LOGOUT_TO_SELECT).await;
    alice.write(gift(b"Stone", b"Bob")).await;
    assert_eq!(frame::payload(&read_op(&mut alice, pp::OP_GIFT).await)[0], pp::GIFT_OK);
    assert_eq!(bob.read().await.unwrap(), g::char_country(0, 3));
    bob.read_list().await;
    assert_eq!(backend.mall.guard().notices.len(), 1);
    bob.write(g::char_id_request(g::OP_CHAR_SELECT, bob_id)).await;
    let mut burst = Vec::new();
    while burst.last().is_none_or(|p: &Vec<u8>| frame::opcode(p) != w::OP_CHAR_QUICK) {
        burst.push(bob.read().await.unwrap());
    }
    let list = burst.iter().find(|p| frame::opcode(p) == pp::OP_STORED_LIST).expect("0x2606");
    assert_eq!(frame::payload(list), [2, 0, 100, 48, 1, 1, 100, 48, 1], "the freed slot 0 is the lowest free one");
    assert_eq!(bob.read().await.unwrap(), pp::gift_received(b"Alice", b"Stone", &[]));
    assert!(backend.mall.guard().notices.is_empty());
}

#[tokio::test]
async fn premium_and_the_billing_storage() {
    use shaiya_proto::point as pp;
    let backend = backend_with_session([5; 16]);
    let (addr, _) = start(backend.clone(), true).await;
    let until = chrono::Utc::now().timestamp() + 86_400;
    backend.mall.guard().premium.insert(7, game_server::backend::Premium { kind: 1, until: Some(until) });
    backend.mall.guard().billing.insert(7, vec![game_server::backend::StoredPointItem { slot: 4, item_id: 100048, count: 1 }]);
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    // 0xB103: the type and the end as a packed date.
    assert_eq!(client.billing[0], 1);
    assert_eq!(u32::from_le_bytes(client.billing[1..5].try_into().unwrap()), game_server::admin::packed_from_unix(until));
    // Type 1 opens the warehouse's second half.
    client.write(w::item_move_request((1, 0), (100, 130))).await;
    let moved = client.read().await.unwrap();
    assert_eq!(frame::opcode(&moved), w::OP_ITEM_MOVE);
    assert!(backend.bank(7, SERVER_ID).iter().any(|i| i.slot == 130));
    // The billing storage's item comes out with make type 'B'.
    client.write(shaiya_proto::encode(pp::OP_BILLING_GET, &[4])).await;
    assert_eq!(client.read().await.unwrap(), pp::taken(pp::OP_BILLING_GET, 4, Some((1, 0))));
    assert!(backend.stored(id).unwrap().1.iter().any(|i| (i.bag, i.slot, i.item_type) == (1, 0, 100)));
    assert!(backend.mall.guard().billing[&7].is_empty());
}

/// What `reload` loads in these tests: monster 1 as the test world has it (HP 120, killed by one blow of the start sword) with the
/// given drop entry and attack, and the drop tables of one potion (25/1) in grade 7.
fn reloaded(drops: [(u16, u32); 10], attack: game_server::mobs::MobAttack, hp: u32) -> Live {
    use game_server::mobs::MobAttack;
    let potion = ItemDef { grade: 7, ..*data().items.get(25, 1).unwrap() };
    let mut attacks = [MobAttack::default(); 3];
    attacks[0] = attack;
    let mobs = MobDefs::new([MobDef {
        id: 1,
        level: 1,
        hp,
        respawn: Duration::ZERO,
        exp: 100,
        country: 2,
        size: 1,
        chase_step: 8,
        normal_step: 4,
        chase_time: Duration::from_secs(60),
        chase_range: 20,
        attacks,
        drops,
        ..Default::default()
    }]);
    Live { mobs, drops: game_server::drops::DropTables::new(&ItemDefs::new([potion])) }
}

#[tokio::test]
async fn reload_puts_new_drops_in_force_and_a_failed_one_changes_nothing() {
    use game_server::game_log::Header;
    use game_server::mobs::MobAttack;
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], true).await;
    let run = |line: &'static str| {
        let ctx = ctx.clone();
        async move { game_server::server::console::line(&ctx, line, "test", Header::none(), 0).await }
    };
    // The monster drops nothing at first; a load that fails (the backend has nothing or an error) leaves it so.
    assert_eq!(ctx.data.live().mobs.get(1).unwrap().drops[1], (0, 0));
    assert_eq!(run("reload").await, ["reload failed, nothing changed: nothing to load"]);
    *backend.next_live.guard() = Some(Err("the table is gone".into()));
    assert_eq!(run("reload").await, ["reload failed, nothing changed: the table is gone"]);
    assert_eq!(ctx.data.live().mobs.get(1).unwrap().drops[1], (0, 0));
    let before = ctx.data.live();

    // Entry 1 now always gives grade 7. A snapshot taken before the reload keeps what it had.
    let mut drops = [(0, 0); 10];
    drops[1] = (7, 100);
    *backend.next_live.guard() = Some(Ok(reloaded(drops, MobAttack::default(), 120)));
    let answer = run("reload").await;
    assert_eq!(answer.len(), 1);
    assert!(answer[0].starts_with("reloaded: 1 monsters, 1 drop grades, 0 box sets, 3 rules ("), "{answer:?}");
    assert_eq!(ctx.data.live().mobs.get(1).unwrap().drops[1], (7, 100));
    assert_eq!(before.mobs.get(1).unwrap().drops[1], (0, 0));

    // A monster killed now drops the potion by the new table.
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    client.write(step(598.0, 1779.0)).await;
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    let dropped = timeout(Duration::from_secs(5), async {
        loop {
            let packet = client.read().await.unwrap();
            if frame::opcode(&packet) == gp::OP_ITEM_ENTER && frame::payload(&packet)[5..7] == [25, 1] {
                return packet;
            }
        }
    })
    .await
    .expect("the potion of grade 7 dropped");
    assert_eq!(frame::payload(&dropped)[4], gp::KIND_DROPPED);
}

#[tokio::test]
async fn reload_puts_a_new_attack_in_force() {
    use game_server::game_log::Header;
    use game_server::mobs::MobAttack;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    // The monster that survives the sword and kills with one blow, as `deadly_data` makes it, arrives by `reload`.
    let attack = MobAttack { ok: 1, damage: 30000, range: 10, ..Default::default() };
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], attack, 100_000)));
    let answer = game_server::server::console::line(&ctx, "reload", "test", Header::none(), 0).await;
    assert!(answer[0].starts_with("reloaded: 1 monsters"), "{answer:?}");
    die(&mut client, &ctx, id).await;
}

#[tokio::test]
async fn restart_counts_down_like_quit_and_cancel_ends_either() {
    use game_server::game_log::Header;
    let backend = Arc::new(MemoryBackend::default());
    let (_, ctx) = start(backend, true).await;
    let run = |line: &'static str| {
        let ctx = ctx.clone();
        async move { game_server::server::console::line(&ctx, line, "test", Header::none(), 0).await }
    };
    // Logins stop; another countdown, of either kind, is refused while this one runs; the process is not to exit with 75 yet.
    assert_eq!(run("restart 1").await, ["restarting in 60 seconds"]);
    assert!(!ctx.accepting.load(Ordering::Relaxed));
    assert_eq!(run("quit").await, ["a shutdown is already counting down"]);
    assert_eq!(run("restart").await, ["a shutdown is already counting down"]);
    assert!(!ctx.restart.load(Ordering::Relaxed));
    assert_eq!(run("quit cancel").await, ["the shutdown was cancelled"]);
    assert!(ctx.accepting.load(Ordering::Relaxed));
    // Without minutes it is the shortest countdown, as quit.
    assert_eq!(run("restart").await, ["restarting in 30 seconds"]);
    assert_eq!(run("quit cancel").await, ["the shutdown was cancelled"]);
    assert_eq!(run("quit 1").await, ["shutting down in 60 seconds"]);
}

// ---- Audit 2026-10, part 1: surviving hostile connections (docs/server/audit.md) ----

/// Sends `lines` full-length zone chat lines as `client`, reading and dropping whatever comes back, so only the others' reading
/// matters. Keep the handles: the connection closes when they go.
fn flood_zone_chat(client: Client, lines: usize) -> [tokio::task::JoinHandle<()>; 2] {
    let Client { stream, send, .. } = client;
    let mut send = send.expect("encrypted");
    let (mut read_half, mut write_half) = stream.into_split();
    let reader = tokio::spawn(async move {
        let mut buffer = [0u8; 65536];
        while matches!(read_half.read(&mut buffer).await, Ok(1..)) {}
    });
    let writer = tokio::spawn(async move {
        let text = [b'x'; 120];
        for _ in 0..lines / 200 {
            let mut bytes = Vec::new();
            for _ in 0..200 {
                let mut packet = say(chat::OP_ZONE, &text);
                send.apply(&mut packet[2..]);
                bytes.extend_from_slice(&packet);
            }
            if write_half.write_all(&bytes).await.is_err() {
                return;
            }
        }
        // Keep the socket open.
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    [reader, writer]
}

#[tokio::test]
async fn a_client_that_stops_reading_is_disconnected_after_the_write_deadline() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) =
        start_with(backend.clone(), Config { write_timeout: Duration::from_secs(1), outbox_limit: 1 << 30, ..config(true) }).await;
    let (alice, _, bob, _) = two_in_the_world_with(&backend, addr, true).await;
    // Bob never reads from here on; Alice floods the zone.
    let _flood = flood_zone_chat(alice, 40_000);
    let started = std::time::Instant::now();
    wait_gone(&backend, 8, Duration::from_secs(15)).await;
    let gone = started.elapsed();
    assert!(gone < Duration::from_secs(10), "the reader is gone within the deadline and the stay: {gone:?}");

    // The account can log in again.
    drop(bob);
    backend.add_session(8, session([7; 16], Some(SERVER_ID), LOCAL));
    let mut again = Client::connect(addr).await;
    again.enter(8, [7; 16]).await;
}

#[tokio::test]
async fn a_kick_reaches_a_client_that_stopped_reading_at_once() {
    use game_server::server::Kick;
    let backend = Arc::new(MemoryBackend::default());
    // The deadline is far away: only the kick can end the connection in time.
    let (addr, ctx) =
        start_with(backend.clone(), Config { write_timeout: Duration::from_secs(60), outbox_limit: 1 << 30, ..config(true) }).await;
    let (alice, _, _bob, _) = two_in_the_world_with(&backend, addr, true).await;
    let _flood = flood_zone_chat(alice, 40_000);
    // Long enough for the flood to fill Bob's buffers and block the server's write.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(backend.session(8).is_some(), "still connected, its write waiting");
    assert!(ctx.online.kick(8, Kick::Operator { op: 0 }));
    let kicked = std::time::Instant::now();
    wait_gone(&backend, 8, Duration::from_secs(10)).await;
    assert!(kicked.elapsed() < Duration::from_secs(5), "{:?}", kicked.elapsed());
}

#[tokio::test]
async fn a_connection_over_its_outbox_limit_ends_once_the_write_is_through() {
    let backend = Arc::new(MemoryBackend::default());
    // Neither the write deadline nor the idle timeout can end it in the time the test waits.
    let slow =
        Config { write_timeout: Duration::from_secs(60), idle_timeout: Duration::from_secs(60), outbox_limit: 64 * 1024, ..config(true) };
    let (addr, _) = start_with(backend.clone(), slow).await;
    let (alice, _, mut bob, _) = two_in_the_world_with(&backend, addr, true).await;
    let _flood = flood_zone_chat(alice, 40_000);
    tokio::time::sleep(Duration::from_secs(3)).await;
    // Bob reads again: a blocked write goes through, and the loop finds the queue overflowed.
    let reader = tokio::spawn(async move { while bob.read().await.is_some() {} });
    wait_gone(&backend, 8, Duration::from_secs(10)).await;
    reader.abort();
}

#[tokio::test]
async fn a_silent_client_in_a_busy_area_is_still_idled_out() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_with(backend.clone(), Config { idle_timeout: Duration::from_secs(1), ..config(true) }).await;
    let (mut alice, _, mut bob, _) = two_in_the_world(&backend, addr).await;
    // Alice talks every 100 ms, and every line reaches Bob; Bob says nothing. The idle time counts from Bob's last bytes, not from
    // the last packet the server sent it.
    let started = std::time::Instant::now();
    let talk = tokio::spawn(async move {
        for _ in 0..40 {
            alice.write(say(chat::OP_ZONE, b"anyone there?")).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        alice
    });
    while bob.read().await.is_some() {}
    let idled = started.elapsed();
    assert!(idled >= Duration::from_millis(900) && idled < Duration::from_secs(3), "{idled:?}");
    let _ = talk.await;
}

/// Whether the server has closed `client`'s connection (a reset counts), waiting up to `within`.
async fn closed_within(client: &mut Client, within: Duration) -> bool {
    let mut buffer = [0u8; 64];
    timeout(within, async { matches!(client.stream.read(&mut buffer).await, Ok(0) | Err(_)) }).await.unwrap_or(false)
}

#[tokio::test]
async fn the_ninth_connection_from_one_address_is_closed() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start_with(backend.clone(), Config { max_per_ip: 8, ..config(true) }).await;
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(Client::connect(addr).await);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut ninth = Client::connect(addr).await;
    assert!(closed_within(&mut ninth, Duration::from_secs(2)).await, "the ninth is closed at once");
    assert!(!closed_within(&mut held[0], Duration::from_millis(200)).await, "the others stay");
    // A slot freed is a slot given again.
    held.pop();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut again = Client::connect(addr).await;
    assert!(!closed_within(&mut again, Duration::from_millis(300)).await);
    assert_eq!(ctx.limits.tracked(), 1);
}

#[tokio::test]
async fn new_connections_are_limited_per_address_and_minute() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_with(backend.clone(), Config { connects_per_minute: 3, ..config(true) }).await;
    for _ in 0..3 {
        let mut client = Client::connect(addr).await;
        assert!(!closed_within(&mut client, Duration::from_millis(100)).await);
    }
    let mut fourth = Client::connect(addr).await;
    assert!(closed_within(&mut fourth, Duration::from_secs(2)).await, "the fourth in a minute is closed");
}

#[tokio::test]
async fn a_socket_that_only_pings_is_closed_at_the_handshake_deadline() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_with(backend.clone(), Config { handshake_timeout: Duration::from_secs(1), ..config(true) }).await;
    let mut client = Client::connect(addr).await;
    let started = std::time::Instant::now();
    let mut buffer = [0u8; 64];
    loop {
        // A plain-text ping, every 200 ms: each restarts a per-read timeout, none may extend the deadline.
        if client.stream.write_all(&shaiya_proto::encode(g::OP_PING, &[])).await.is_err() {
            break;
        }
        if let Ok(read) = timeout(Duration::from_millis(200), client.stream.read(&mut buffer)).await
            && matches!(read, Ok(0) | Err(_))
        {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(4), "still open after 4 s");
    }
    let closed = started.elapsed();
    assert!(closed >= Duration::from_millis(900) && closed < Duration::from_millis(2000), "{closed:?}");
}

#[tokio::test]
async fn a_handshake_without_the_session_neither_takes_the_slot_nor_waits_for_it() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut victim = Client::connect(addr).await;
    victim.enter(7, [5; 16]).await;
    assert!(ctx.online.contains(7));

    // A bogus `0xA301` with the victim's id used to wait for the victim's slot (the test setting: 1 s) before it was checked;
    // now the session is looked at first and the answer is a close at once.
    for identity in [[9; 16], [0; 16]] {
        let mut attacker = Client::connect(addr).await;
        let asked = std::time::Instant::now();
        attacker.write(g::game_connect_request(7, &identity)).await;
        assert!(attacker.read().await.is_none());
        assert!(asked.elapsed() < Duration::from_millis(500), "refused at once: {:?}", asked.elapsed());
    }
    assert!(ctx.online.contains(7), "the victim's entry is untouched");
    assert!(backend.session(7).unwrap().in_game, "and so is its session");
}

#[tokio::test]
async fn a_burst_of_chat_lines_is_relayed_five_at_a_time_and_the_rest_dropped() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_with(backend.clone(), Config { chat_per_sec: 3, chat_burst: 5, ..config(true) }).await;
    let (mut alice, _, mut bob, _) = two_in_the_world(&backend, addr).await;
    // Twenty lines in one go on the zone channel; Bob hears five.
    let mut burst = Vec::new();
    for n in 0..20u8 {
        burst.extend(say(chat::OP_ZONE, &[b'a' + n]));
    }
    // One write per line would do too: the packets are encrypted in order.
    for line in burst.chunks(burst.len() / 20) {
        alice.write(line.to_vec()).await;
    }
    let mut heard = Vec::new();
    while let Ok(Some(packet)) = timeout(Duration::from_millis(500), bob.read()).await {
        if frame::opcode(&packet) == chat::OP_ZONE {
            heard.push(packet);
        }
    }
    assert_eq!(heard.len(), 5, "the burst, no more");
    // The sender is not disconnected for it, and after a rest she can talk again.
    tokio::time::sleep(Duration::from_millis(700)).await;
    alice.write(say(chat::OP_ZONE, b"back")).await;
    assert_eq!(frame::opcode(&next_chat(&mut bob).await), chat::OP_ZONE);
}

#[tokio::test]
async fn three_hundred_packets_in_a_second_keep_the_connection_and_a_flood_ends_it() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_with(backend.clone(), Config { packets_per_sec: 100, ..config(true) }).await;
    let (mut alice, _, mut bob, _) = two_in_the_world(&backend, addr).await;
    // 300 pings at once: the burst of 200 and what the rest of the second refills; nothing ends.
    for _ in 0..300 {
        alice.write(shaiya_proto::encode(g::OP_PING, &[])).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    alice.write(say(chat::OP_ZONE, b"still here")).await;
    assert_eq!(frame::opcode(&next_chat(&mut bob).await), chat::OP_ZONE);

    // A thousand a second, on and on: over the rate for five seconds in a row ends the connection.
    let started = std::time::Instant::now();
    let flood = tokio::spawn(async move {
        while started.elapsed() < Duration::from_secs(15) {
            for _ in 0..50 {
                let mut ping = shaiya_proto::encode(g::OP_PING, &[]);
                alice.send.as_mut().unwrap().apply(&mut ping[2..]);
                if alice.stream.write_all(&ping).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    wait_gone(&backend, 7, Duration::from_secs(14)).await;
    let ended = started.elapsed();
    assert!(ended >= Duration::from_secs(4) && ended < Duration::from_secs(12), "{ended:?}");
    flood.abort();
}

#[tokio::test]
async fn the_monster_ticks_saves_go_to_a_queue_and_are_flushed_in_order() {
    use game_server::kc::Changes;
    use game_server::server::{flush_saves, kill_count_changes, start_saves};
    let backend = Arc::new(MemoryBackend::default());
    let (_, ctx) = start(backend.clone(), true).await;
    // Without the queue a save is written where it is asked for.
    kill_count_changes(&ctx, Changes { save: true, ..Changes::default() }).await;
    assert_eq!(*backend.kill_counts.guard(), [0, 0]);
    start_saves(&ctx);
    let changes = ctx.kill_counts.guard().add(1, 7, std::time::Instant::now());
    kill_count_changes(&ctx, Changes { save: true, ..changes }).await;
    assert!(flush_saves(&ctx, Duration::from_secs(5)).await);
    assert_eq!(*backend.kill_counts.guard(), ctx.kill_counts.guard().counts(), "written once the queue is flushed");
}

#[tokio::test]
async fn a_connection_that_panics_inside_a_zone_lock_leaves_the_zone_and_the_server_goes_on() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    let (mut alice, alice_id, mut bob, _) = two_in_the_world(&backend, addr).await;
    ctx.test_panic.store(alice_id, Ordering::Relaxed);
    alice.write(shaiya_proto::encode(g::OP_PING, &[])).await;
    // Alice's connection ends, her session is released, and Bob is told she left, through the lock the panic poisoned.
    assert!(read_until_closed(&mut alice).await);
    wait_gone(&backend, 7, Duration::from_secs(5)).await;
    loop {
        let packet = bob.read().await.expect("the zone still works");
        if packet == w::user_leave(alice_id) {
            break;
        }
    }
    // The next lock works: Bob moves and is answered, and a new login of Alice's account gets in.
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[1])).await;
    loop {
        if bob.read().await.expect("Bob's connection lives") == w::motion(2, 1) {
            break;
        }
    }
    backend.add_session(7, session([9; 16], Some(SERVER_ID), LOCAL));
    let mut again = Client::connect(addr).await;
    again.enter(7, [9; 16]).await;
}

#[tokio::test]
async fn a_shop_title_with_a_bad_colour_marker_or_a_control_byte_does_not_open_the_shop() {
    use shaiya_proto::my_shop as mp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start_with(backend.clone(), Config { market_maps: vec![1], ..config(true) }).await;
    let (mut alice, _, mut bob, bob_id) = two_in_the_world(&backend, addr).await;
    bob.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_SIT])).await;
    bob.write(shaiya_proto::encode(mp::OP_BEGIN, &[])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::begin());
    // A marker running past the text (the one that crashes clients), and a control byte: nothing opens, nothing is sent.
    bob.write(shaiya_proto::encode(mp::OP_START, b"\x08<<999c01\0")).await;
    bob.write(shaiya_proto::encode(mp::OP_START, &[4, b'a', 7, b'b', 0])).await;
    // The next good title opens it: the first thing Bob and Alice are told about the shop is this sign.
    bob.write(shaiya_proto::encode(mp::OP_START, &[6, b'B', b'o', b'b', b'\'', b's', 0])).await;
    assert_eq!(next_shop_packet(&mut bob).await, mp::started());
    let sign = mp::sign(bob_id, 6, b"Bob's");
    assert_eq!(next_shop_packet(&mut bob).await, sign);
    assert_eq!(next_shop_packet(&mut alice).await, sign, "the viewers saw no other sign first");
}

#[tokio::test]
async fn a_staff_member_moved_to_another_level_loses_the_console_at_once() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _) = start(backend.clone(), true).await;
    // Two level-1 staff members; Boss moves Bob to level 3.
    let (mut boss, _, mut bob, _, _) = gm_and_other(&backend, addr, 1, 1).await;
    let say = |text: &[u8]| shaiya_proto::encode(ap::OP_CHAT_NORMAL, &[&[text.len() as u8 + 1][..], text, &[0]].concat());
    bob.write(say(b".info")).await;
    assert_eq!(next_whisper(&mut bob).await.0, "Server", "level 1 has the console");

    let mut set = b"Bob".to_vec();
    set.resize(21, 0);
    set.push(3);
    boss.write(shaiya_proto::encode(ap::OP_SET_LEVEL, &set)).await;
    assert_eq!(read_op(&mut bob, ap::OP_SET_LEVEL).await, ap::level_answer(ap::OP_SET_LEVEL, 3));

    bob.write(say(b".info")).await;
    let heard = timeout(Duration::from_millis(500), async {
        loop {
            let packet = bob.read().await.unwrap();
            if frame::opcode(&packet) == chat::OP_WHISPER {
                return packet;
            }
        }
    })
    .await;
    assert!(heard.is_err(), "the typed line is chat now, not a console command");
}

/// The rules in force of `values`, put in the running server's zones.
fn put_rules<S>(ctx: &Context<S>, values: &[(&str, serde_json::Value)]) {
    ctx.zones.set_rules(game_server::rules::RuleSet::from_values(values).unwrap());
}

/// The texts of the whispers among `packets` (`0x1102`, the receiving form).
fn whisper_texts(packets: &[Vec<u8>]) -> Vec<String> {
    packets
        .iter()
        .filter(|p| frame::opcode(p) == chat::OP_WHISPER)
        .map(|p| String::from_utf8_lossy(&p[4 + 23..4 + 23 + usize::from(p[4 + 22])]).trim_end_matches('\0').to_owned())
        .collect()
}

/// Sends `packet`, then a request that is answered with `0x0302`: what the server sent in between, in order.
async fn answered_with(client: &mut Client, packet: Vec<u8>, marker: &[u8]) -> Vec<Vec<u8>> {
    client.write(packet).await;
    client.write(marker.to_vec()).await;
    let mut seen = Vec::new();
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == w::OP_USER_HP {
            return seen;
        }
        seen.push(packet);
    }
}

#[tokio::test]
async fn a_rule_refuses_an_attack_and_the_server_says_why() {
    use shaiya_proto::combat as cp;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    put_rules(&ctx, &[("safe", serde_json::json!({ "type": "deny", "point": "attack", "message": "No fighting here." }))]);
    // Alice is Light, Bob Fury: enemies, so the rules are asked.
    let (mut alice, _, _bob, bob_id) = two_in_the_world_of(&backend, addr, false, [0, 1]).await;
    let aim = shaiya_proto::encode(cp::OP_ATTACK_USER, &bob_id.to_le_bytes());
    let marker = shaiya_proto::encode(w::OP_USER_HP, &bob_id.to_le_bytes());
    let seen = answered_with(&mut alice, aim.clone(), &marker).await;
    assert!(!seen.contains(&cp::attack_start()), "no attack starts: {seen:x?}");
    assert_eq!(whisper_texts(&seen), ["No fighting here."]);
    // A second request within 10 seconds: refused again, but the rule does not whisper again.
    let seen = answered_with(&mut alice, aim.clone(), &marker).await;
    assert!(!seen.contains(&cp::attack_start()) && whisper_texts(&seen).is_empty(), "{seen:x?}");
    // Without the rule the attack starts.
    put_rules(&ctx, &[]);
    let seen = answered_with(&mut alice, aim, &marker).await;
    assert!(seen.contains(&cp::attack_start()) && whisper_texts(&seen).is_empty(), "{seen:x?}");
}

#[tokio::test]
async fn a_deny_trade_rule_answers_busy_and_whispers_once() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    put_rules(&ctx, &[("no-trade", serde_json::json!({ "type": "deny", "point": "trade" }))]);
    let (mut alice, alice_id, mut bob, bob_id) = two_in_the_world(&backend, addr).await;
    let request = shaiya_proto::encode(tp::OP_REQUEST, &bob_id.to_le_bytes());
    let marker = shaiya_proto::encode(w::OP_USER_HP, &bob_id.to_le_bytes());
    let seen = answered_with(&mut alice, request.clone(), &marker).await;
    assert!(seen.contains(&tp::start(bob_id, tp::START_BUSY)), "{seen:x?}");
    assert_eq!(whisper_texts(&seen), ["Trading is not allowed here."]);
    // Again within 10 seconds: busy again, no second whisper.
    let seen = answered_with(&mut alice, request.clone(), &marker).await;
    assert!(seen.contains(&tp::start(bob_id, tp::START_BUSY)) && whisper_texts(&seen).is_empty(), "{seen:x?}");
    // Bob never heard of it; without the rule the request goes through.
    put_rules(&ctx, &[]);
    alice.write(request).await;
    assert_eq!(next_trade_packet(&mut bob).await, tp::request(alice_id));
}

#[tokio::test]
async fn deny_rules_for_trade_chat_and_a_personal_shop_whisper_their_message() {
    use shaiya_proto::my_shop as mp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    let mut client = Client::connect(addr).await;
    client.enter(7, [5; 16]).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, b"Trader")).await;
    client.read().await.unwrap();
    client.read_list().await;
    backend.set_level(1, 10);
    let id = select_and_enter(&mut client).await;
    let marker = shaiya_proto::encode(w::OP_USER_HP, &id.to_le_bytes());
    let trade = shaiya_proto::encode(chat::OP_TRADE, &[4, b'w', b't', b's', 0]);
    // The settings' own rules are silent: the test server's market_maps closes map 1, and nobody hears of it.
    client.write(shaiya_proto::encode(w::OP_MOTION, &[w::MOTION_SIT])).await;
    let seen = answered_with(&mut client, shaiya_proto::encode(mp::OP_BEGIN, &[]), &marker).await;
    assert!(whisper_texts(&seen).is_empty(), "{seen:x?}");
    put_rules(
        &ctx,
        &[
            ("no-shops", serde_json::json!({ "type": "deny", "point": "personal_shop", "message": "No shops here." })),
            ("no-trade-chat", serde_json::json!({ "type": "deny", "point": "trade_chat", "level": "10-", "message": "Be quiet." })),
        ],
    );
    let seen = answered_with(&mut client, shaiya_proto::encode(mp::OP_BEGIN, &[]), &marker).await;
    assert_eq!(whisper_texts(&seen), ["No shops here."]);
    assert!(!seen.iter().any(|p| frame::opcode(p) == mp::OP_BEGIN), "the shop did not open");
    let seen = answered_with(&mut client, trade.clone(), &marker).await;
    assert_eq!(whisper_texts(&seen), ["Be quiet."]);
    assert!(!seen.iter().any(|p| frame::opcode(p) == chat::OP_TRADE), "nothing was said to the faction");
    // Each rule whispers once in ten seconds.
    let seen = answered_with(&mut client, trade, &marker).await;
    assert!(whisper_texts(&seen).is_empty());
}

/// Kills the first monster of the test world with `rules` in force and returns the experience packet (`0x0508`) it gave.
async fn exp_for_the_first_monster(rules: &[(&str, serde_json::Value)]) -> Vec<u8> {
    use shaiya_proto::combat as cp;
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    put_rules(&ctx, rules);
    let mut client = Client::connect(addr).await;
    let id = enter_world(&mut client, 7, [5; 16], true).await;
    ctx.zones.respawn(std::time::Instant::now(), &ctx.data.world, &ctx.data.live().mobs, &mut || 0);
    assert_eq!(client.read().await.unwrap(), w::mob_enter(1, true, 1, 600.0, 1780.0));
    assert_eq!(client.read().await.unwrap(), w::mob_max_hp(1, 120, 120));
    client.write(step(598.0, 1779.0)).await;
    assert_eq!(frame::opcode(&client.read().await.unwrap()), w::OP_MOVE);
    client.write(shaiya_proto::encode(cp::OP_ATTACK_MOB, &1u32.to_le_bytes())).await;
    assert_eq!(client.read().await.unwrap(), cp::attack_start());
    loop {
        let packet = client.read().await.unwrap();
        if frame::opcode(&packet) == cp::OP_MOB_DEATH {
            assert_eq!(packet, cp::mob_death(1, cp::KILLER_PLAYER, id));
            break;
        }
    }
    assert_eq!(frame::opcode(&client.read().await.unwrap()), cp::OP_ATTACK_MOB);
    client.read().await.unwrap()
}

#[tokio::test]
async fn an_exp_rule_changes_the_experience_on_its_map_only() {
    use shaiya_proto::combat as cp;
    let double = |maps: serde_json::Value| [("double", serde_json::json!({ "type": "exp", "factor": 2, "maps": maps }))];
    assert_eq!(exp_for_the_first_monster(&[]).await, cp::exp_gain(144, 0), "the number of killing_a_monster_gives_experience_and_a_level");
    assert_eq!(exp_for_the_first_monster(&double(serde_json::json!([1]))).await, cp::exp_gain(288, 0), "twice, on the map of the kill");
    assert_eq!(exp_for_the_first_monster(&double(serde_json::json!([2]))).await, cp::exp_gain(144, 0), "another map: unchanged");
    let half = [("half", serde_json::json!({ "type": "exp", "factor": 0.5, "sources": ["mob"] }))];
    assert_eq!(exp_for_the_first_monster(&half).await, cp::exp_gain(72, 0));
    let quests_only = [("q", serde_json::json!({ "type": "exp", "factor": 0, "sources": ["quest"] }))];
    assert_eq!(exp_for_the_first_monster(&quests_only).await, cp::exp_gain(144, 0), "a monster is not a quest");
}

#[tokio::test]
async fn reload_puts_new_rules_in_force_and_a_broken_one_changes_nothing() {
    use game_server::game_log::Header;
    use game_server::mobs::MobAttack;
    use game_server::rules::RuleSources;
    use shaiya_proto::combat as cp;
    let path = std::env::temp_dir().join(format!("reload-rules-{:08x}.toml", rand::random::<u32>()));
    std::fs::write(&path, "# no rules yet\n").unwrap();
    let backend = Arc::new(MemoryBackend::default());
    let config = Config { config_path: Some(path.to_str().unwrap().to_owned()), ..config(true) };
    let (addr, ctx) = start_with(backend.clone(), config).await;
    let (mut alice, alice_id, _bob, bob_id) = two_in_the_world_of(&backend, addr, false, [0, 1]).await;
    let run = |line: &'static str| {
        let ctx = ctx.clone();
        async move { game_server::server::console::line(&ctx, line, "test", Header::none(), 0).await }
    };
    let aim = shaiya_proto::encode(cp::OP_ATTACK_USER, &bob_id.to_le_bytes());
    let marker = shaiya_proto::encode(w::OP_USER_HP, &bob_id.to_le_bytes());

    // Before any rule the two are enemies (a request would start the attack).
    assert!(ctx.zones.may_attack_player(1, alice_id, bob_id, false));
    assert_eq!(run("rules").await.len(), 3, "the three settings' rules");

    // A rule written to the file: before `reload` nothing changes, after it the attack is refused with the rule's message.
    std::fs::write(&path, "guild_party_size = 3\n[rules.safe]\ntype = \"deny\"\npoint = \"attack\"\nmessage = \"Not in the file.\"\n").unwrap();
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 120)));
    // Not yet read: the same monsters, the same rules.
    assert_eq!(ctx.zones.rules().len(), 3);
    let answer = run("reload").await;
    assert!(
        answer[0].starts_with("reloaded: 1 monsters, 1 drop grades, 0 box sets, 4 rules (")
            && answer[0].ends_with("); settings changed in the file take effect at the next start: guild_party_size"),
        "{answer:?}"
    );
    assert_eq!(ctx.config.guild_party_size, 7, "the setting was not taken");
    let seen = answered_with(&mut alice, aim.clone(), &marker).await;
    assert_eq!(whisper_texts(&seen), ["Not in the file."]);
    let listed = run("rules").await;
    assert!(listed[0].starts_with("safe deny file ") && listed.len() == 4, "{listed:?}");
    assert!(listed[0].ends_with(" 1"), "one hit so far: {listed:?}");
    assert!(listed.iter().any(|l| l.starts_with("setting.market_maps deny setting:market_maps ")), "{listed:?}");

    // A broken rule fails the reload and changes nothing: not the rules, not the monsters.
    let hp_before = ctx.data.live().mobs.get(1).unwrap().hp;
    std::fs::write(&path, "\n[rules.safe]\ntype = \"level_gap\"\ngap = 0\n").unwrap();
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 5000)));
    let answer = run("reload").await;
    assert_eq!(answer, [format!("reload failed, nothing changed: {} line 4: rules.safe.gap: 0 is not 1 to 255", path.display())]);
    assert_eq!(ctx.data.live().mobs.get(1).unwrap().hp, hp_before, "the monsters are as they were");
    assert_eq!(ctx.zones.rules().len(), 4);
    let seen = answered_with(&mut alice, aim.clone(), &marker).await;
    assert!(!seen.contains(&cp::attack_start()), "the old rule still holds");

    // The database's layer: the data editor's rule beats the file's of the same name; its change set is shown; a new rule may whisper
    // at once.
    std::fs::write(&path, "[rules.safe]\ntype = \"deny\"\npoint = \"attack\"\nmessage = \"Not in the file.\"\n").unwrap();
    *backend.rule_layers.guard() = RuleSources {
        editor: vec![("safe".into(), serde_json::json!({ "type": "deny", "point": "attack", "message": "From the editor." }), Some(3))],
        ..RuleSources::default()
    };
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 120)));
    let answer = run("reload").await;
    assert!(answer[0].starts_with("reloaded: 1 monsters, 1 drop grades, 0 box sets, 4 rules ("), "{answer:?}");
    let seen = answered_with(&mut alice, aim, &marker).await;
    assert_eq!(whisper_texts(&seen), ["From the editor."]);
    assert!(run("rules").await[0].starts_with("safe deny editor:3 "));
    std::fs::remove_file(&path).unwrap();
}

// ------------------------------------------------------------------------------------------------ the extensions' scripts
// (docs/plans/ext-5-lua.md section 12, steps 4 to 9)

use game_server::script::sources::{ScriptExtension, ScriptSources};

type TestContext = Arc<Context<Arc<MemoryBackend>>>;

/// The sources of the extensions `(name, main.lua)` as the database would hold them, installed in this order and all enabled.
fn script_sources(scripts: &[(&str, &str)]) -> ScriptSources {
    ScriptSources {
        extensions: scripts
            .iter()
            .map(|(name, source)| ScriptExtension { name: (*name).to_owned(), scripts: vec![("main.lua".to_owned(), (*source).to_owned())], ..Default::default() })
            .collect(),
        installed: scripts.iter().map(|(name, _)| ((*name).to_owned(), true)).collect(),
        ..Default::default()
    }
}

/// Loads the extensions' scripts as `main` does and starts the script thread; a script that does not load fails the test.
fn put_scripts(ctx: &TestContext, scripts: &[(&str, &str)]) {
    let set = game_server::server::script::load_at_start(ctx, &script_sources(scripts)).unwrap();
    assert!(set.failed().is_empty(), "{:?}", set.failed());
    ctx.start_scripts(set);
}

/// The text of the next whisper from the server, within five seconds.
async fn next_server_whisper(client: &mut Client) -> String {
    timeout(Duration::from_secs(5), async {
        loop {
            let packet = client.read().await.unwrap();
            if frame::opcode(&packet) == chat::OP_WHISPER {
                return whisper_texts(&[packet]).remove(0);
            }
        }
    })
    .await
    .expect("a whisper within five seconds")
}

async fn console(ctx: &TestContext, line: &'static str) -> Vec<String> {
    game_server::server::console::line(ctx, line, "test", game_server::game_log::Header::none(), 0).await
}

#[tokio::test]
async fn a_script_welcomes_a_player_at_login() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    put_scripts(&ctx, &[("welcome", "game.on('login', function(p) p:message('Welcome, ' .. p.name .. '!') p:notice('Hi ' .. p.name) end)")]);
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], true).await;
    assert_eq!(next_server_whisper(&mut client).await, "Welcome, Walker!");
    let notice = read_op(&mut client, ap::OP_NOTICE_TO).await;
    assert_eq!(notice, ap::notice(ap::OP_NOTICE_TO, b"Hi Walker\0"));
    assert_eq!(console(&ctx, "scripts").await, ["welcome running 1 handlers timers 0 spawns 0 faults 0"]);
}

#[tokio::test]
async fn a_script_command_answers_and_is_not_relayed() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    put_scripts(
        &ctx,
        &[(
            "talk",
            "game.command('hi', function(p, args) p:message('hello ' .. args) end, { help = 'says hello' })\n\
             game.command('gm', function(p, args) p:message('staff only') end, { staff = 3 })",
        )],
    );
    let (mut alice, alice_id, mut bob, _bob_id) = two_in_the_world_of(&backend, addr, false, [0, 0]).await;
    alice.write(say(chat::OP_NORMAL, b"!hi there")).await;
    assert_eq!(next_server_whisper(&mut alice).await, "hello there");
    // A colour marker in front (the client adds one with a chat colour), any case, no arguments.
    alice.write(say(chat::OP_NORMAL, b"<<003c01!HI")).await;
    assert_eq!(next_server_whisper(&mut alice).await, "hello ");
    // Not chat: Bob (a few units away) hears none of it. Words nobody registered, a staff command typed by a player and the whisper
    // prefix with its space stay chat, in order, and "plain" is the first line he gets after them.
    let lines: [&[u8]; 4] = [b"!nothing here", b"!gm now", b"! alice hi", b"plain"];
    for text in lines {
        alice.write(say(chat::OP_NORMAL, text)).await;
    }
    for text in lines {
        let mut zeroed = text.to_vec();
        zeroed.push(0);
        assert_eq!(next_chat(&mut bob).await, chat::by_id(chat::OP_NORMAL, alice_id, &zeroed));
    }
}

#[tokio::test]
async fn a_script_timer_sends_a_notice() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    put_scripts(&ctx, &[("ads", "game.after(1, function() game.notice_all('Visit the shop') end)")]);
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], true).await;
    let notice = timeout(Duration::from_secs(5), read_op(&mut client, ap::OP_NOTICE_WORLD)).await.expect("the timer's notice");
    assert_eq!(notice, ap::world_notice("Visit the shop").unwrap());
    assert_eq!(ctx.zones.ext_scripts().extensions()[0].timer_count(), 0, "a one-shot timer is gone once it ran");
}

#[tokio::test]
async fn reload_swaps_scripts_and_a_broken_one_changes_nothing() {
    use game_server::mobs::MobAttack;
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    ctx.start_scripts(game_server::script::ScriptSet::empty());
    let (mut alice, _, _bob, _) = two_in_the_world_of(&backend, addr, false, [0, 0]).await;
    // Nothing is registered: the line is chat, and `scripts` says so.
    assert_eq!(console(&ctx, "scripts").await, ["no scripts"]);

    backend.set_script_sources(script_sources(&[("a", "game.command('ver', function(p) p:message('one') end)")]));
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 120)));
    let answer = console(&ctx, "reload").await;
    assert!(answer[0].starts_with("reloaded: 1 monsters, 1 drop grades, 0 box sets, 3 rules, 1 scripts of 1 extensions ("), "{answer:?}");
    alice.write(say(chat::OP_NORMAL, b"!ver")).await;
    assert_eq!(next_server_whisper(&mut alice).await, "one");

    // A script that does not compile fails the whole reload: the monsters, the rules and the old script stay.
    let hp_before = ctx.data.live().mobs.get(1).unwrap().hp;
    backend.set_script_sources(script_sources(&[("a", "game.command('ver', function(p) p:message('two') end\n")]));
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 5000)));
    let answer = console(&ctx, "reload").await;
    assert!(answer[0].starts_with("reload failed, nothing changed: extension a: a/main.lua:") , "{answer:?}");
    assert_eq!(ctx.data.live().mobs.get(1).unwrap().hp, hp_before, "the monsters are as they were");
    assert_eq!(ctx.zones.rules().len(), 3);
    alice.write(say(chat::OP_NORMAL, b"!ver")).await;
    assert_eq!(next_server_whisper(&mut alice).await, "one");

    // The fixed script replaces the old one.
    backend.set_script_sources(script_sources(&[("a", "game.command('ver', function(p) p:message('two') end)")]));
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 120)));
    assert!(console(&ctx, "reload").await[0].contains("1 scripts of 1 extensions"));
    alice.write(say(chat::OP_NORMAL, b"!ver")).await;
    assert_eq!(next_server_whisper(&mut alice).await, "two");
}

#[tokio::test]
async fn a_failing_script_is_disabled_and_the_server_carries_on() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, ctx) = start(backend.clone(), true).await;
    put_scripts(&ctx, &[("bad", "game.on('login', function(p) p:message('hi') error('boom') end)")]);
    // Five logins in a minute, each of which errors after its whisper.
    for round in 0..6 {
        backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
        let mut client = Client::connect(addr).await;
        enter_world(&mut client, 7, [5; 16], round == 0).await;
        if round < 5 {
            assert_eq!(next_server_whisper(&mut client).await, "hi", "login {round}");
            // The fault is counted after the handler ran: wait for it before the next login.
            timeout(Duration::from_secs(5), async { while ctx.zones.ext_scripts().extensions()[0].fault_count() < round + 1 { tokio::time::sleep(Duration::from_millis(10)).await } })
                .await
                .expect("the fault");
        }
        // Out of the world before the next connection.
        drop(client);
        timeout(Duration::from_secs(5), async { while ctx.zones.locate(1).is_some() { tokio::time::sleep(Duration::from_millis(20)).await } }).await.expect("left the world");
    }
    let lines = console(&ctx, "scripts").await;
    assert!(lines[0].starts_with("bad disabled (5 errors in 60 s: login: ") && lines[0].contains("boom") && lines[0].contains("faults 5"), "{lines:?}");
    // The sixth login (the last round above) got no whisper because the extension was disabled, and the world goes on: a seventh
    // connection enters and hears nothing.
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], false).await;
    let heard = timeout(Duration::from_millis(600), async {
        loop {
            if frame::opcode(&client.read().await.unwrap()) == chat::OP_WHISPER {
                return;
            }
        }
    })
    .await;
    assert!(heard.is_err(), "no whisper once disabled");
}

#[tokio::test]
async fn scripts_off_loads_nothing() {
    use game_server::mobs::MobAttack;
    let backend = Arc::new(MemoryBackend::default());
    let config = Config { scripts: false, ..config(true) };
    let (addr, ctx) = start_with(backend.clone(), config).await;
    ctx.start_scripts(game_server::script::ScriptSet::empty());
    let (mut alice, _, _bob, _) = two_in_the_world_of(&backend, addr, false, [0, 0]).await;
    backend.set_script_sources(script_sources(&[("a", "game.command('ver', function(p) p:message('one') end)")]));
    *backend.next_live.guard() = Some(Ok(reloaded([(0, 0); 10], MobAttack::default(), 120)));
    let answer = console(&ctx, "reload").await;
    assert!(answer[0].starts_with("reloaded: 1 monsters, 1 drop grades, 0 box sets, 3 rules ("), "no scripts clause: {answer:?}");
    assert_eq!(console(&ctx, "scripts").await, ["no scripts"]);
    alice.write(say(chat::OP_NORMAL, b"!ver")).await;
    assert_eq!(next_chat(&mut alice).await, chat::by_id(chat::OP_NORMAL, 1, b"!ver\0"), "the line stays chat");
}

#[tokio::test]
async fn a_value_a_command_set_is_read_after_a_relog() {
    let backend = backend_with_session([5; 16]);
    let (addr, ctx) = start(backend.clone(), true).await;
    put_scripts(
        &ctx,
        &[("count", "game.command('count', function(p) local n = (p:get('n') or 0) + 1 p:set('n', n) game.set('total', (game.get('total') or 0) + 1) p:message('n=' .. n) end)")],
    );
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], true).await;
    client.write(say(chat::OP_NORMAL, b"!count")).await;
    assert_eq!(next_server_whisper(&mut client).await, "n=1");
    // The write goes to the database in a batch: a refused one is put back and tried again.
    backend.fail_script_writes.store(true, Ordering::SeqCst);
    assert!(!game_server::server::script::flush_script_values(&ctx).await);
    assert!(backend.script_values.guard().is_empty());
    assert!(game_server::server::script::flush_script_values(&ctx).await);
    assert_eq!(backend.script_values.guard().get(&("count".to_owned(), 1, "n".to_owned())), Some(&serde_json::json!(1)));
    assert_eq!(backend.script_values.guard().get(&("count".to_owned(), 0, "total".to_owned())), Some(&serde_json::json!(1)));
    drop(client);
    timeout(Duration::from_secs(5), async { while ctx.zones.locate(1).is_some() || ctx.zones.script_store().is_loaded(1) { tokio::time::sleep(Duration::from_millis(20)).await } })
        .await
        .expect("left the world, the values out of the cache");

    // Logged in again, before anything was written the second time: the value is the stored one.
    backend.add_session(7, session([5; 16], Some(SERVER_ID), LOCAL));
    let mut client = Client::connect(addr).await;
    enter_world(&mut client, 7, [5; 16], false).await;
    client.write(say(chat::OP_NORMAL, b"!count")).await;
    assert_eq!(next_server_whisper(&mut client).await, "n=2");
    assert!(game_server::server::script::flush_script_values(&ctx).await);
    assert_eq!(backend.script_values.guard().get(&("count".to_owned(), 1, "n".to_owned())), Some(&serde_json::json!(2)));
    assert_eq!(ctx.zones.script_store().get_global("count", "total"), Some(serde_json::json!(2)));
}
