//! Test bots: protocol-level clients that speak only the packets the real client sends (`docs/plans/test-bots.md`). Built so far: the
//! connection (`link`), the login (`login`), the road into the world (`enter`), a bot's view of the world (`world`), its counters
//! (`metrics`), `bot probe` (`probe`) and the `pvp-battle` loop for one bot (`scenario::pvp_battle`).

pub mod bot;
pub mod enter;
pub mod link;
pub mod login;
pub mod metrics;
pub mod probe;
pub mod scenario;
pub mod world;
