//! One bot in the world: its connection, what it knows and what it has measured. `receive` is the only place packets are read, so the
//! counters and round trips are kept in one spot whatever scenario drives the bot.

use std::time::{Duration, Instant};

use shaiya_proto::{combat, frame, world};

use crate::echo::{EVERY, Echo};
use crate::enter::{CharacterScreen, connect_game, enter_world};
use crate::link::{Link, LinkError};
use crate::login::Selected;
use crate::metrics::Metrics;
use crate::world::{Event, World};

/// `0x0553` kind 2: back to life at the bind position.
const REBIRTH_KIND: u8 = 2;

pub struct Bot {
    pub link: Link,
    pub world: World,
    pub metrics: Metrics,
    /// The swarm's step board, when the bot is part of one.
    pub echo: Option<Echo>,
    /// Steps taken in all (the counters in `metrics` start again every window).
    steps_taken: u64,
}

impl Bot {
    pub fn new(link: Link, world: World) -> Self {
        Self { link, world, metrics: Metrics::default(), echo: None, steps_taken: 0 }
    }

    /// From a login to a bot in the world: connects to the game server, takes the account's first character and enters.
    pub async fn enter(selected: &Selected) -> anyhow::Result<Self> {
        let (link, screen) = connect_game(&selected.game_address, selected.user_id, &selected.identity, &selected.keys).await?;
        Self::enter_from(link, &screen).await
    }

    /// From the character screen on an open connection: the account's first character enters the world.
    pub async fn enter_from(mut link: Link, screen: &CharacterScreen) -> anyhow::Result<Self> {
        let character = screen.first_character()?;
        let world = enter_world(&mut link, screen.country, &character).await?;
        Ok(Self::new(link, world))
    }

    /// Sends one packet and counts it.
    pub async fn send(&mut self, packet: Vec<u8>) -> Result<(), LinkError> {
        self.metrics.packet_sent(frame::opcode(&packet), packet.len());
        let sent = self.link.write(packet).await;
        if let Err(error) = &sent {
            self.metrics.disconnect = Some(error.clone());
        }
        sent
    }

    /// A step to `(x, z)` is about to be sent: one in ten goes on the swarm's step board.
    pub fn stamp_step(&mut self, x: f32, z: f32) {
        self.steps_taken += 1;
        if self.steps_taken.is_multiple_of(EVERY)
            && let Some(echo) = &self.echo
        {
            echo.board.stamp(echo.slot, x, z, Instant::now());
        }
    }

    /// The next packet from the server: counted, applied to the world and returned as what it meant. Cancel-safe (see `Link::next`).
    pub async fn receive(&mut self) -> Result<Vec<Event>, LinkError> {
        let packet = match self.link.next().await {
            Ok(packet) => packet,
            Err(error) => {
                self.metrics.disconnect = Some(error.clone());
                return Err(error);
            }
        };
        Ok(self.apply(&packet))
    }

    /// Counts a packet from the server and applies it to the world.
    fn apply(&mut self, packet: &[u8]) -> Vec<Event> {
        let now = Instant::now();
        self.metrics.packet_received(frame::opcode(packet), packet.len());
        let events = self.world.handle(packet);
        let me = self.world.id;
        for event in &events {
            match *event {
                Event::Bad(opcode) => {
                    if self.metrics.protocol_error(opcode) {
                        tracing::warn!(
                            id = me,
                            opcode = format_args!("{opcode:#06x}"),
                            "a packet of the wrong size (logged once per opcode)"
                        );
                    }
                }
                Event::Refused => self.metrics.refusals += 1,
                Event::Moved(id) => {
                    if let (Some(echo), Some(seen)) = (&self.echo, self.world.players.get(&id))
                        && let Some(delay) = echo.board.seen(id, seen.position[0], seen.position[2], now)
                    {
                        self.metrics.step_echo.record(delay);
                    }
                }
                Event::Attack { result, attacker, target } if attacker == me => self.metrics.attack_result(result, target, now),
                Event::AttackStart => self.metrics.attack_started(now),
                Event::Skill { caster, .. } if caster == me => self.metrics.skill_outcome(now),
                Event::Died { id, .. } if id == me => self.metrics.deaths += 1,
                Event::Died { killer_kind, killer, .. } if killer_kind == combat::KILLER_PLAYER && killer == me => self.metrics.kills += 1,
                Event::Reborn(id) if id == me => self.metrics.rebirths += 1,
                _ => {}
            }
        }
        events
    }

    /// Sends the `0x0303` question for every player that entered the view and has not been asked about.
    pub async fn ask_about_new_players(&mut self) -> Result<(), LinkError> {
        for id in self.world.take_asks() {
            self.send(shaiya_proto::game::char_id_request(world::OP_USER_SHAPE, id)).await?;
        }
        Ok(())
    }

    /// Reads and applies packets until `done` says so (it sees the bot, each packet's opcode and what the packet meant) or `deadline`
    /// passes. False on the deadline or when the connection ends.
    async fn pump(&mut self, deadline: Instant, done: impl Fn(&Self, u16, &[Event]) -> bool) -> bool {
        loop {
            let Ok(next) = tokio::time::timeout_at(deadline.into(), self.link.next()).await else { return false };
            let Ok(packet) = next else { return false };
            // Still counted as in the world: a blow that lands while the bot sits is part of the fight.
            let events = self.apply(&packet);
            if done(self, frame::opcode(&packet), &events) {
                return true;
            }
        }
    }

    /// Leaves as a player does: sits (the server ignores a logout request from a standing player), asks for the character screen with
    /// `0x0107`, waits for its answer and closes. The server logs a sitting player out after its delay (`logout_secs`, 10 by default),
    /// and a blow or a skill on the player in the meantime stands it up without a word and cancels the logout, so a bot that is struck sits and
    /// asks again; a bot that is dead (or dies on the way out) first asks to come back with `0x0553`, as the server sends no answer to
    /// a dead player. Everything within `within` in all; true when the answer came.
    pub async fn leave(&mut self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        let mut answered = false;
        loop {
            if !self.world.alive {
                let rebirth = shaiya_proto::encode(combat::OP_REBIRTH_REQUEST, &[REBIRTH_KIND]);
                if self.send(rebirth).await.is_err() || !self.pump(deadline, |bot, _, _| bot.world.alive).await {
                    break;
                }
            }
            let sit = shaiya_proto::encode(world::OP_MOTION, &[world::MOTION_SIT]);
            if self.send(sit).await.is_err() || self.send(shaiya_proto::encode(world::OP_LOGOUT_TO_SELECT, &[])).await.is_err() {
                break;
            }
            let me = self.world.id;
            let left = std::sync::atomic::AtomicBool::new(false);
            let ended = self
                .pump(deadline, |bot, opcode, events| {
                    left.store(opcode == world::OP_LOGOUT_TO_SELECT, std::sync::atomic::Ordering::Relaxed);
                    left.load(std::sync::atomic::Ordering::Relaxed)
                        || !bot.world.alive
                        || events.iter().any(|e| {
                            matches!(e, Event::Attack { attacker: by, target, .. } | Event::Skill { caster: by, target, .. } if *target == me && *by != me)
                        })
                })
                .await;
            // Only the answer is a logout; a blow or a death that ended the wait sends the bot round again.
            if !ended || left.load(std::sync::atomic::Ordering::Relaxed) {
                answered = ended;
                break;
            }
        }
        self.link.close().await;
        answered
    }
}
