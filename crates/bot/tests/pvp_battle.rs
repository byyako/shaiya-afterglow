//! Two bots of the two factions against the in-process game server (the memory backend, as the game server's own socket tests): they
//! see each other as enemies, walk toward each other without a refusal, one kills the other, the dead one asks to come back after its
//! wait, comes back on `0x0551` and walks back to the battle point. No database.

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use bot::scenario::pvp_battle::{Battle, Config, State};
use bot::world::distance;
use game_server::memory::MemoryBackend;
use shaiya_proto::combat;
use shaiya_proto::game as g;
use support::common::*;
use support::{bot_in_world, drive};

/// An account with one character, made through the character screen as the client does, which then leaves the connection.
/// Returns the character's id.
async fn make_character(
    backend: &Arc<MemoryBackend>,
    addr: std::net::SocketAddr,
    user: u32,
    identity: [u8; 16],
    country: u8,
    name: &[u8],
) -> u32 {
    backend.add_session(user, session(identity, Some(SERVER_ID), LOCAL));
    let mut client = Client::connect(addr).await;
    client.enter(user, identity).await;
    client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[country])).await;
    // A human fighter for Light, a Deatheater fighter for Fury (races 0 and 3), as the game server's own tests do.
    client.write(create(0, if country == 0 { 0 } else { 3 }, 0, name)).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
    let list = client.read_list().await;
    let id = list.iter().map(|r| u32::from_le_bytes(r[1..5].try_into().unwrap())).find(|&id| id != 0).expect("the character");
    client.write(shaiya_proto::encode(g::OP_DISCONNECT, &[])).await;
    assert!(client.read().await.is_none());
    wait_gone(backend, user, Duration::from_secs(2)).await;
    backend.add_session(user, session(identity, Some(SERVER_ID), LOCAL));
    id
}

#[tokio::test]
async fn two_bots_of_the_two_factions_fight_one_dies_and_walks_back() {
    let backend = Arc::new(MemoryBackend::default());
    let (addr, _ctx) = start(backend.clone(), true).await;
    let light_id = make_character(&backend, addr, 7, [5; 16], 0, b"Lightbot").await;
    let fury_id = make_character(&backend, addr, 8, [6; 16], 1, b"Furybot").await;

    // Light at the human start; Fury 14 units east of it. The Fury bot has no weapon (`0x0502` result 7), so Light wins.
    backend.set_position(light_id, 1, [580.0, 78.56, 1762.12]);
    backend.set_position(fury_id, 1, [594.0, 78.56, 1762.12]);

    // The blows come every 4 s at attack speed 0: a level 10 Light (the highest level at which a death drops nothing) lands more of them
    // than a level 1 one against a level 1 Fury, so the kill comes within a blow or two.
    backend.set_level(light_id, 10);
    let mut light = bot_in_world(addr, 7, [5; 16]).await;
    let mut fury = bot_in_world(addr, 8, [6; 16]).await;
    assert_eq!((light.world.id, fury.world.id), (light_id, fury_id));

    // With a radius of 0 a bot is always walking to its point exactly. The Fury bot, which has nothing to fight with, goes back to where
    // it started and waits there to be killed; the battle point is moved far down the map after it dies, so that when it comes back to
    // life it has a long walk to make wherever it died.
    let (light_point, fury_point) = ([630.0, 1762.0], [594.0, 1762.0]);
    let far_point = [1500.0, 1762.0];
    let config = |point, idle| Config {
        point,
        radius: 0.0,
        steps_per_sec: 4.0,
        rebirth_after: Duration::from_millis(300),
        skill_every: Duration::from_secs(3600),
        idle,
        ..Config::default()
    };
    let (mut light_battle, mut fury_battle) = (Battle::new(config(light_point, false)), Battle::new(config(fury_point, false)));
    let started = Instant::now();

    let fight = async {
        tokio::join!(
            drive(&mut light, &mut light_battle, |bot, _| bot.metrics.kills >= 1),
            // The Fury bot dies, asks to come back after its wait and is back on 0x0551.
            drive(&mut fury, &mut fury_battle, |bot, _| bot.metrics.rebirths >= 1),
        );
    };
    tokio::time::timeout(Duration::from_secs(40), fight).await.expect("a kill and a rebirth within 40 s");
    println!("fight took {:?}", started.elapsed());

    fury_battle.set_point(far_point);

    // Seen as enemies: each bot had the other as a target, which needs its `0x0303` answer.
    assert!(light_battle.milestones.enemy_seen && fury_battle.milestones.enemy_seen);
    assert_eq!(light.world.players[&fury_id].country, Some(1));
    assert_eq!(fury.world.players[&light_id].country, Some(0));
    // Stepped toward each other and were never put back.
    assert!(light.metrics.steps > 0 && fury.metrics.steps > 0);
    assert_eq!((light.metrics.refusals, fury.metrics.refusals), (0, 0));
    assert_eq!((light.metrics.protocol_errors, fury.metrics.protocol_errors), (0, 0));
    // One killed the other; the first blow's round trip was measured.
    assert_eq!((light.metrics.kills, light.metrics.deaths), (1, 0));
    assert_eq!((fury.metrics.kills, fury.metrics.deaths), (0, 1));
    assert!(!light.metrics.attack_round_trip.is_empty());
    // The first 0x0502 found the Light bot not fighting, so the server answered at once with 0x0212: measured, and on loopback small
    // (well below the attack interval that the round trip to the first blow includes).
    assert!(!light.metrics.attack_ack.is_empty(), "the attack acknowledgement was measured");
    let (ack, rtt) = (light.metrics.attack_ack.max().unwrap(), light.metrics.attack_round_trip.max().unwrap());
    println!("attack ack {ack:?}, round trip to the first blow {rtt:?}");
    assert!(ack < Duration::from_millis(250) && ack < rtt, "ack {ack:?}, round trip {rtt:?}");
    assert!(fury_battle.milestones.unarmed && fury.metrics.results[&7] >= 1, "the Fury character has no weapon: result 7");
    // The dead one waited its 300 ms before 0x0553, sent it once, and is alive again.
    assert_eq!(fury_battle.milestones.rebirth_waits.len(), 1);
    assert!(fury_battle.milestones.rebirth_waits[0] >= Duration::from_millis(300), "{:?}", fury_battle.milestones.rebirth_waits);
    assert_eq!(fury.metrics.sent[&combat::OP_REBIRTH_REQUEST].packets, 1);
    assert!(fury.world.alive);
    assert!(matches!(fury_battle.state, State::Walk));

    // It walks back to the battle point: steps toward it, none refused.
    let at_bind = distance(fury.world.position, [far_point[0], 0.0, far_point[1]]);
    tokio::time::timeout(Duration::from_secs(20), drive(&mut fury, &mut fury_battle, |_, b| b.milestones.steps_since_rebirth >= 8))
        .await
        .expect("the walk back");
    let now_at = distance(fury.world.position, [far_point[0], 0.0, far_point[1]]);
    println!("bind {at_bind}, after the walk {now_at}");
    assert!(now_at < at_bind, "closer to the point after the walk back: {at_bind} -> {now_at}");
    assert_eq!(fury.metrics.refusals, 0);

    // They leave as players do.
    light.leave(Duration::from_secs(2)).await;
    fury.leave(Duration::from_secs(2)).await;
    wait_gone(&backend, 7, Duration::from_secs(3)).await;
    wait_gone(&backend, 8, Duration::from_secs(3)).await;
}
