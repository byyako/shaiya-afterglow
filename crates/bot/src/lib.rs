//! Test bots: protocol-level clients that speak only the packets the real client sends (`docs/plans/test-bots.md`). Built so far: the
//! connection (`link`), the login (`login`), the road into the world (`enter`), a bot's view of the world (`world`), its counters
//! (`metrics`), `bot probe` (`probe`), the `pvp-battle` loop for one bot (`scenario::pvp_battle`) the seeding of bot accounts (`seed`), and the swarm with its windows and report (`swarm`, `report`, `echo`).

pub mod bot;
pub mod echo;
pub mod enter;
pub mod link;
pub mod login;
pub mod metrics;
pub mod probe;
pub mod report;
pub mod scenario;
pub mod seed;
pub mod swarm;
pub mod world;
