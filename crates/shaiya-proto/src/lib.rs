//! Wire format of the Shaiya login and game connections.
//!
//! Every packet is `u16 length | u16 opcode | payload`, little-endian, where `length`
//! counts the whole packet including the length field itself. After encryption starts,
//! everything following the length field is encrypted (see `shaiya-crypto`).

pub mod admin;
pub mod chat;
pub mod combat;
pub mod duel;
pub mod frame;
pub mod friend;
pub mod game;
pub mod gem;
pub mod ground;
pub mod guild;
pub mod guild_war;
pub mod item;
pub mod login;
pub mod market;
pub mod mini_game;
pub mod my_shop;
pub mod obelisk;
pub mod party;
pub mod point;
pub mod priest;
pub mod quest;
pub mod shop;
pub mod skill;
pub mod trade;
pub mod vehicle;
pub mod world;

pub use frame::{FrameDecoder, FrameError, HEADER_LEN, MAX_PACKET_LEN, encode};
