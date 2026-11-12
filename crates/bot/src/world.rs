//! What one bot knows of the world: itself, the players in its view (id, faction, job, position, alive) and its skills, kept up to date
//! from the packets the server sends. No sockets here: `World::handle` takes one decrypted packet and says what it meant.

use std::collections::HashMap;

use game_server::combat as server_combat;
use shaiya_proto::skill::SkillEntry;
use shaiya_proto::{combat, frame, skill, world};

/// Units a second on foot by move speed 0 to 4 (`g_ConstUserMoveSpeed`, `docs/original/move-check.md`).
const MOVE_UNITS: [f32; 5] = [3.0, 4.0, 5.0, 6.0, 7.0];

/// A player in the view, as far as the bot knows.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    /// From `0x0303`; `None` until the server has answered.
    pub country: Option<u8>,
    pub job: u8,
    pub position: [f32; 3],
    pub alive: bool,
    pub in_view: bool,
    asked: bool,
}

/// What a packet meant for the bot's decisions.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A player entered the view (`0x0201`); a `0x0303` question is queued for it when it is new.
    Entered(u32),
    Left(u32),
    /// `0x0303` told this player's faction and job.
    Shape(u32),
    /// Another player stepped.
    Moved(u32),
    /// `0x0502`: an attack's result.
    Attack {
        result: u8,
        attacker: u32,
        target: u32,
    },
    /// `0x0511`: a skill's outcome.
    Skill {
        result: u8,
        caster: u32,
        target: u32,
    },
    /// `0x0504`.
    Died {
        id: u32,
        killer_kind: u8,
        killer: u32,
    },
    /// `0x0551`.
    Reborn(u32),
    /// `0x020B` on the bot's own id: the server put it back (a refused move).
    Refused,
    /// A packet that is the wrong size for its opcode.
    Bad(u16),
}

#[derive(Debug)]
pub struct World {
    pub id: u32,
    pub country: u8,
    pub job: u8,
    pub level: u16,
    pub map: u16,
    pub position: [f32; 3],
    pub dir: u16,
    pub alive: bool,
    /// HP, MP, SP now (`0x0521`, `0x0505`).
    pub health: [u16; 3],
    pub max_health: [u16; 3],
    /// Index into the speed tables, `0xFF` when the server says it cannot act.
    pub attack_speed: u8,
    pub move_speed: u8,
    pub skills: Vec<SkillEntry>,
    pub players: HashMap<u32, Seen>,
    asks: Vec<u32>,
    /// The packet before was a `0x020B` on the bot's own id: a `0x0502` result 5 right after it is the original's refusal packet, not
    /// an out-of-reach result (both have the same bytes).
    after_refusal: bool,
}

fn u16_at(p: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([p[at], p[at + 1]])
}

fn u32_at(p: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([p[at], p[at + 1], p[at + 2], p[at + 3]])
}

fn f32_at(p: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(p, at))
}

fn vec3_at(p: &[u8], at: usize) -> [f32; 3] {
    [f32_at(p, at), f32_at(p, at + 4), f32_at(p, at + 8)]
}

/// Horizontal distance: the server's move and reach checks ignore the height.
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl World {
    /// A bot about to enter the world: its character's id, faction (`0x0109`) and the list record's level, job and map.
    pub fn new(id: u32, country: u8, job: u8, level: u16, map: u16) -> Self {
        Self {
            id,
            country,
            job,
            level,
            map,
            position: [0.0; 3],
            dir: 0,
            alive: true,
            health: [0; 3],
            max_health: [0; 3],
            attack_speed: 0,
            move_speed: 2,
            skills: Vec::new(),
            players: HashMap::new(),
            asks: Vec::new(),
            after_refusal: false,
        }
    }

    /// Units a second the bot walks at.
    pub fn speed(&self) -> f32 {
        MOVE_UNITS.get(usize::from(self.move_speed)).copied().unwrap_or(MOVE_UNITS[2])
    }

    /// The bot took a step (the server has it unless it answers with a refusal).
    pub fn stepped(&mut self, x: f32, z: f32, dir: u16) {
        self.position[0] = x;
        self.position[2] = z;
        self.dir = dir;
    }

    /// The players to ask `0x0303` of, once each.
    pub fn take_asks(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.asks)
    }

    /// The nearest living enemy in view (known faction other than the bot's own) by the last known positions, ties by id.
    pub fn nearest_enemy(&self) -> Option<(u32, f32)> {
        self.nearest_enemy_except(None)
    }

    /// `nearest_enemy` without the player `skip`.
    pub fn nearest_enemy_except(&self, skip: Option<u32>) -> Option<(u32, f32)> {
        self.players
            .iter()
            .filter(|(id, s)| Some(**id) != skip && s.in_view && s.alive && s.country.is_some_and(|c| c != self.country))
            .map(|(&id, s)| (id, distance(self.position, s.position)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }

    /// Whether `id` is an enemy in view now: alive, of a known faction other than the bot's own.
    pub fn is_enemy_in_view(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|s| s.in_view && s.alive && s.country.is_some_and(|c| c != self.country))
    }

    fn seen(&mut self, id: u32) -> &mut Seen {
        self.players.entry(id).or_insert(Seen { country: None, job: 0, position: [0.0; 3], alive: true, in_view: false, asked: false })
    }

    /// Applies one decrypted packet (length field included) and returns what it meant. An opcode the bot does not use returns nothing.
    pub fn handle(&mut self, packet: &[u8]) -> Vec<Event> {
        let opcode = frame::opcode(packet);
        let p = frame::payload(packet);
        let echo = std::mem::take(&mut self.after_refusal);
        let mut events = Vec::new();
        let bad = |events: &mut Vec<Event>| events.push(Event::Bad(opcode));
        match opcode {
            world::OP_CHAR_DATA => {
                if p.len() < 40 {
                    bad(&mut events);
                } else {
                    self.max_health = [u16_at(p, 4), u16_at(p, 6), u16_at(p, 8)];
                    self.dir = u16_at(p, 10);
                    self.position = vec3_at(p, 28);
                }
            }
            skill::OP_CHAR_SKILL => match p.split_first() {
                Some((&count, rest)) if rest.len() >= usize::from(count) * 8 => {
                    self.skills = rest
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .take(usize::from(count))
                        .map(|s| SkillEntry { skill_id: u16_at(s, 0), level: s[2], number: s[3], delay: u32_at(s, 4) })
                        .collect();
                }
                _ => bad(&mut events),
            },
            world::OP_SPEED => {
                if p.len() < 6 {
                    bad(&mut events);
                } else if u32_at(p, 0) == self.id {
                    self.attack_speed = p[4];
                    self.move_speed = p[5];
                }
            }
            world::OP_USER_ENTER => {
                if p.len() < 27 {
                    bad(&mut events);
                    return events;
                }
                let (id, dir, position) = (u32_at(p, 0), u16_at(p, 5), vec3_at(p, 7));
                if id == self.id {
                    self.dir = dir;
                    self.position = position;
                } else {
                    let seen = self.seen(id);
                    seen.position = position;
                    seen.in_view = true;
                    seen.alive = true;
                    if !seen.asked {
                        seen.asked = true;
                        self.asks.push(id);
                    }
                    events.push(Event::Entered(id));
                }
            }
            world::OP_USER_LEAVE => match p.len() {
                4 => {
                    let id = u32_at(p, 0);
                    if let Some(seen) = self.players.get_mut(&id) {
                        seen.in_view = false;
                    }
                    events.push(Event::Left(id));
                }
                _ => bad(&mut events),
            },
            world::OP_USER_SHAPE => {
                if p.len() < 15 {
                    bad(&mut events);
                } else {
                    let id = u32_at(p, 0);
                    let seen = self.seen(id);
                    seen.country = Some(p[6]);
                    seen.job = p[11];
                    events.push(Event::Shape(id));
                }
            }
            // Another player's step, and the position sync (motion 0xC4) that comes before a `0x0301` or `0x0303` answer.
            world::OP_MOVE => {
                if p.len() != 19 {
                    bad(&mut events);
                } else {
                    let id = u32_at(p, 0);
                    if id != self.id {
                        self.seen(id).position = vec3_at(p, 7);
                        events.push(Event::Moved(id));
                    }
                }
            }
            world::OP_POSITION => {
                if p.len() < 18 {
                    bad(&mut events);
                } else {
                    let (id, position) = (u32_at(p, 0), vec3_at(p, 6));
                    if id == self.id {
                        self.position = position;
                        self.after_refusal = true;
                        events.push(Event::Refused);
                    } else {
                        self.seen(id).position = position;
                    }
                }
            }
            combat::OP_ATTACK_USER => {
                if p.len() < 15 {
                    bad(&mut events);
                } else {
                    let (result, attacker, target) = (p[0], u32_at(p, 1), u32_at(p, 5));
                    let refusal_echo = echo && result == server_combat::RESULT_OUT_OF_RANGE && attacker == self.id && target == 0;
                    if !refusal_echo {
                        events.push(Event::Attack { result, attacker, target });
                    }
                }
            }
            skill::OP_USE_USER => {
                if p.len() < 12 {
                    bad(&mut events);
                } else {
                    events.push(Event::Skill { result: p[0], caster: u32_at(p, 1), target: u32_at(p, 5) });
                }
            }
            combat::OP_DEATH => {
                if p.len() < 9 {
                    bad(&mut events);
                } else {
                    let (id, killer_kind, killer) = (u32_at(p, 0), p[4], u32_at(p, 5));
                    if id == self.id {
                        self.alive = false;
                    } else {
                        self.seen(id).alive = false;
                    }
                    events.push(Event::Died { id, killer_kind, killer });
                }
            }
            combat::OP_REBIRTH => {
                if p.len() < 21 {
                    bad(&mut events);
                } else {
                    let (id, position) = (u32_at(p, 0), vec3_at(p, 9));
                    if id == self.id {
                        self.alive = true;
                        self.position = position;
                    } else {
                        let seen = self.seen(id);
                        seen.alive = true;
                        seen.position = position;
                    }
                    events.push(Event::Reborn(id));
                }
            }
            combat::OP_RECOVER_SET => {
                if p.len() < 6 {
                    bad(&mut events);
                } else {
                    self.health = [u16_at(p, 0), u16_at(p, 2), u16_at(p, 4)];
                }
            }
            combat::OP_RECOVER => {
                if p.len() < 10 {
                    bad(&mut events);
                } else if u32_at(p, 0) == self.id {
                    for (i, now) in self.health.iter_mut().enumerate() {
                        let gained = i16::from_le_bytes([p[4 + 2 * i], p[5 + 2 * i]]);
                        *now = now.saturating_add_signed(gained);
                    }
                }
            }
            _ => {}
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enter(id: u32, at: [f32; 3]) -> Vec<u8> {
        world::user_enter(&world::UserEnter { char_id: id, kind: 1, dir: 0, position: at, guild_id: 0, vehicle: 0 })
    }

    fn shape(id: u32, country: u8) -> Vec<u8> {
        world::user_shape(&world::UserShape { id, country, job: 2, name: b"X".to_vec(), ..Default::default() })
    }

    #[test]
    fn a_player_is_asked_for_once_and_is_an_enemy_when_the_other_faction() {
        let mut w = World::new(1, 0, 0, 1, 18);
        w.position = [0.0, 0.0, 0.0];
        assert_eq!(w.handle(&enter(2, [10.0, 0.0, 0.0])), [Event::Entered(2)]);
        assert_eq!(w.take_asks(), [2]);
        assert_eq!(w.nearest_enemy(), None, "faction unknown yet");
        assert_eq!(w.handle(&shape(2, 1)), [Event::Shape(2)]);
        assert_eq!(w.nearest_enemy(), Some((2, 10.0)));
        // Out of the view and back: not asked again.
        w.handle(&world::user_leave(2));
        assert_eq!(w.nearest_enemy(), None);
        w.handle(&enter(2, [10.0, 0.0, 0.0]));
        assert!(w.take_asks().is_empty());
        // Same faction: no enemy.
        w.handle(&enter(3, [1.0, 0.0, 0.0]));
        w.handle(&shape(3, 0));
        assert_eq!(w.nearest_enemy(), Some((2, 10.0)));
    }

    #[test]
    fn the_nearest_enemy_wins_and_ties_go_to_the_lower_id() {
        let mut w = World::new(1, 0, 0, 1, 18);
        for (id, x) in [(9, 5.0), (7, 5.0), (8, 4.0)] {
            w.handle(&enter(id, [x, 0.0, 0.0]));
            w.handle(&shape(id, 1));
        }
        assert_eq!(w.nearest_enemy(), Some((8, 4.0)));
        w.handle(&combat::death(8, combat::KILLER_PLAYER, 3));
        assert_eq!(w.nearest_enemy(), Some((7, 5.0)));
    }

    #[test]
    fn death_and_rebirth_of_the_bot_itself() {
        let mut w = World::new(1, 0, 0, 1, 18);
        assert_eq!(w.handle(&combat::death(1, combat::KILLER_PLAYER, 2)), [Event::Died { id: 1, killer_kind: 1, killer: 2 }]);
        assert!(!w.alive);
        assert_eq!(w.handle(&combat::rebirth(1, combat::REBIRTH_BIND, 0, [5.0, 6.0, 7.0])), [Event::Reborn(1)]);
        assert!(w.alive);
        assert_eq!(w.position, [5.0, 6.0, 7.0]);
    }

    #[test]
    fn a_refusal_is_the_put_back_and_its_0x0502_is_not_an_out_of_reach_result() {
        let mut w = World::new(1, 0, 0, 1, 18);
        assert_eq!(w.handle(&world::position(1, 18, [3.0, 4.0, 5.0])), [Event::Refused]);
        assert_eq!(w.position, [3.0, 4.0, 5.0]);
        assert!(w.handle(&world::move_refused(1)).is_empty());
        // The same bytes alone, not after a put-back, are the out-of-reach result.
        assert_eq!(
            w.handle(&combat::attack_refused(server_combat::RESULT_OUT_OF_RANGE, 1)),
            [Event::Attack { result: 5, attacker: 1, target: 0 }]
        );
    }

    #[test]
    fn short_packets_are_bad_not_a_panic() {
        let mut w = World::new(1, 0, 0, 1, 18);
        for opcode in [world::OP_CHAR_DATA, world::OP_USER_ENTER, world::OP_MOVE, world::OP_POSITION, combat::OP_DEATH, combat::OP_REBIRTH]
        {
            assert_eq!(w.handle(&shaiya_proto::encode(opcode, &[1, 2])), [Event::Bad(opcode)]);
        }
    }

    #[test]
    fn own_data_skills_speeds_and_health() {
        let mut w = World::new(1, 0, 0, 1, 18);
        let data = world::CharData { max_health: [200, 50, 100], dir: 90, position: [1.0, 2.0, 3.0], ..Default::default() };
        w.handle(&world::char_data(&data));
        assert_eq!((w.max_health, w.dir, w.position), ([200, 50, 100], 90, [1.0, 2.0, 3.0]));
        w.handle(&skill::char_skills(&[SkillEntry { skill_id: 5, level: 2, number: 3, delay: 0 }]));
        assert_eq!(w.skills, [SkillEntry { skill_id: 5, level: 2, number: 3, delay: 0 }]);
        w.handle(&world::speed(1, 4, 3));
        assert_eq!((w.attack_speed, w.move_speed, w.speed()), (4, 3, 6.0));
        w.handle(&combat::recover_set([100, 40, 30]));
        w.handle(&combat::recover(1, [-10, 5, 0]));
        assert_eq!(w.health, [90, 45, 30]);
    }
}
