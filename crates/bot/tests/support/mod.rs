//! What the bots' tests against the in-process game server share: the game server's own test helpers (`tests/common`) and the two
//! helpers that put a bot in the world and play it.
#![allow(dead_code)]

#[path = "../../../game-server/tests/common/mod.rs"]
pub mod common;

use std::time::Duration;

use bot::bot::Bot;
use bot::enter::connect_game;
use bot::scenario::pvp_battle::{Battle, run};
use common::LOGIN_KEYS;

/// A bot of the account `user`, entered with its first character.
pub async fn bot_in_world(addr: std::net::SocketAddr, user: u32, identity: [u8; 16]) -> Bot {
    let (link, screen) = connect_game(&addr.to_string(), user, &identity, &LOGIN_KEYS).await.unwrap();
    Bot::enter_from(link, &screen).await.unwrap()
}

/// Plays `bot` in slices until `done` says so.
pub async fn drive(bot: &mut Bot, battle: &mut Battle, done: impl Fn(&Bot, &Battle) -> bool) {
    while !done(bot, battle) {
        run(bot, battle, tokio::time::sleep(Duration::from_millis(40))).await.expect("the connection held");
    }
}
