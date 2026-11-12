//! One bot in the world: its connection, what it knows and what it has measured. `receive` is the only place packets are read, so the
//! counters and round trips are kept in one spot whatever scenario drives the bot.

use std::time::{Duration, Instant};

use shaiya_proto::{combat, frame, world};

use crate::enter::{CharacterScreen, connect_game, enter_world};
use crate::link::{Link, LinkError};
use crate::login::Selected;
use crate::metrics::Metrics;
use crate::world::{Event, World};

pub struct Bot {
    pub link: Link,
    pub world: World,
    pub metrics: Metrics,
}

impl Bot {
    pub fn new(link: Link, world: World) -> Self {
        Self { link, world, metrics: Metrics::default() }
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

    /// The next packet from the server: counted, applied to the world and returned as what it meant. Cancel-safe (see `Link::next`).
    pub async fn receive(&mut self) -> Result<Vec<Event>, LinkError> {
        let packet = match self.link.next().await {
            Ok(packet) => packet,
            Err(error) => {
                self.metrics.disconnect = Some(error.clone());
                return Err(error);
            }
        };
        let now = Instant::now();
        self.metrics.packet_received(frame::opcode(&packet), packet.len());
        let events = self.world.handle(&packet);
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
                Event::Attack { result, attacker, target } if attacker == me => self.metrics.attack_result(result, target, now),
                Event::Skill { caster, .. } if caster == me => self.metrics.skill_outcome(now),
                Event::Died { id, .. } if id == me => self.metrics.deaths += 1,
                Event::Died { killer_kind, killer, .. } if killer_kind == combat::KILLER_PLAYER && killer == me => self.metrics.kills += 1,
                Event::Reborn(id) if id == me => self.metrics.rebirths += 1,
                _ => {}
            }
        }
        Ok(events)
    }

    /// Sends the `0x0303` question for every player that entered the view and has not been asked about.
    pub async fn ask_about_new_players(&mut self) -> Result<(), LinkError> {
        for id in self.world.take_asks() {
            self.send(shaiya_proto::game::char_id_request(world::OP_USER_SHAPE, id)).await?;
        }
        Ok(())
    }

    /// Leaves as a player does: sits (the server ignores a logout request from a standing player), asks for the character screen with
    /// `0x0107`, waits up to `within` for its answer and closes.
    pub async fn leave(&mut self, within: Duration) {
        let sit = shaiya_proto::encode(world::OP_MOTION, &[world::MOTION_SIT]);
        if self.send(sit).await.is_ok() && self.send(shaiya_proto::encode(world::OP_LOGOUT_TO_SELECT, &[])).await.is_ok() {
            let wait = async {
                while let Ok(packet) = self.link.next().await {
                    self.metrics.packet_received(frame::opcode(&packet), packet.len());
                    if frame::opcode(&packet) == world::OP_LOGOUT_TO_SELECT {
                        return;
                    }
                }
            };
            let _ = tokio::time::timeout(within, wait).await;
        }
        self.link.close().await;
    }
}
