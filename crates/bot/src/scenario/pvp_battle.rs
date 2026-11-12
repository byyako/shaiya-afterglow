//! The `pvp-battle` loop of section 5.2 of `docs/plans/test-bots.md` for one bot:
//!
//! ```text
//!   WALK ── enemy in view ──> CHASE ── in reach ──> FIGHT ── target dead / gone ──┐
//!    ^                          ^   └─── out of reach (result 5) ───┘              │
//!    │                          └────────────────── next target ───────────────────┘
//!    └──── 0x0551 (back at the bind) ──── DEAD <── own 0x0504 (from any state)
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

use game_server::combat as server_combat;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use shaiya_proto::skill::SkillEntry;
use shaiya_proto::{combat, skill, world as wp};

use crate::bot::Bot;
use crate::link::LinkError;
use crate::world::{Event, distance};

/// The server refuses a step longer than this.
const MAX_STEP: f32 = 35.0;
/// The server's reach is the weapon's range plus this (`combat::in_range`); the bot keeps `REACH_MARGIN` inside it.
const REACH_PLUS: f32 = 2.3;
const REACH_MARGIN: f32 = 0.5;
/// A step is this share of what the speed allows, so no 5-second window of the move check fails.
const STEP_SHARE: f32 = 0.9;
/// How often the bot thinks; steps come at `steps_per_sec`.
const TICK: Duration = Duration::from_millis(100);
/// After an out-of-reach result the bot closes in before it asks again.
const CLOSE_IN: Duration = Duration::from_secs(1);
/// How long after a skill the bot waits for its outcome before it attacks again.
const SKILL_WAIT: Duration = Duration::from_secs(2);
/// Motion 1: run.
const MOTION_RUN: u8 = 1;
/// `0x0553` kind 2: back to life at the bind position.
const REBIRTH_KIND: u8 = 2;

/// A skill the bot may use on an enemy player: the facts of its definition at the level the bot has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillRule {
    pub skill_id: u16,
    pub level: u8,
    pub attack_range: f32,
    pub sp: u16,
    pub mp: u16,
    pub reset: Duration,
}

#[derive(Clone, Debug)]
pub struct Config {
    /// Where the battle is (x, z).
    pub point: [f32; 2],
    /// The bot wanders within this of the point when no enemy is in view.
    pub radius: f32,
    pub steps_per_sec: f32,
    /// A skill every this long, with 30 % jitter either way.
    pub skill_every: Duration,
    /// How long a dead bot waits before `0x0553` (a person reading the screen).
    pub rebirth_after: Duration,
    /// Stands at the point and only reads; it still comes back to life and walks back to the point.
    pub idle: bool,
    /// The equipped weapon's range from the item definitions.
    pub weapon_range: i32,
    /// The attack skills the bot may use, in the order it tries them.
    pub skills: Vec<SkillRule>,
    /// A target that has not come within reach in this long is dropped for as long again.
    pub chase_limit: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            point: [0.0, 0.0],
            radius: 60.0,
            steps_per_sec: 2.0,
            skill_every: Duration::from_secs(6),
            rebirth_after: Duration::from_secs(5),
            idle: false,
            weapon_range: 0,
            skills: Vec::new(),
            chase_limit: Duration::from_secs(5),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Walk,
    Chase(u32),
    Fight(u32),
    Dead,
}

/// What happened, for the tests and the swarm's summary.
#[derive(Clone, Debug, Default)]
pub struct Milestones {
    /// An enemy (a player of the other faction, known by `0x0303`) was in view at some tick.
    pub enemy_seen: bool,
    /// How long each death waited before `0x0553` went out.
    pub rebirth_waits: Vec<Duration>,
    /// Steps taken since the last `0x0551`.
    pub steps_since_rebirth: u64,
    /// `0x0502` result 7: no weapon, so the bot only walks with the crowd from then on.
    pub unarmed: bool,
}

pub struct Battle {
    cfg: Config,
    pub state: State,
    pub milestones: Milestones,
    target: Option<u32>,
    /// The target the server was last told to attack and has not stopped attacking.
    attacking: Option<u32>,
    wander: Option<[f32; 2]>,
    next_step_at: Instant,
    next_skill_at: Instant,
    skill_used: HashMap<u8, Instant>,
    resume_attack_at: Option<Instant>,
    force_chase_until: Option<Instant>,
    chasing: Option<(u32, Instant)>,
    ignored: Option<(u32, Instant)>,
    died_at: Option<Instant>,
    rebirth_sent: bool,
    after_rebirth: bool,
    rng: StdRng,
}

/// How far one step goes: `STEP_SHARE` of what `speed` units a second allow in `period`, never more than the server's 35, and no further
/// than `remaining`.
fn step_length(speed: f32, period: Duration, remaining: f32) -> f32 {
    (STEP_SHARE * speed * period.as_secs_f32()).min(MAX_STEP).min(remaining)
}

/// The `dir` of a step by `(dx, dz)`, in the server's own convention (`game_server::world::degrees`: radians to truncated degrees, sign
/// flipped). Which way a whole number points on the map is to be checked by eye with the real client (run R2): the server's move check
/// does not read it.
fn heading(dx: f32, dz: f32) -> u16 {
    game_server::world::degrees(dx.atan2(dz))
}

/// The first of `rules` the bot has (by id and level), may afford and has rested, whose range reaches `distance`: the skill's number.
pub fn pick_skill(
    rules: &[SkillRule],
    owned: &[SkillEntry],
    last_used: &HashMap<u8, Instant>,
    distance: f32,
    health: [u16; 3],
    now: Instant,
) -> Option<(u8, SkillRule)> {
    rules.iter().find_map(|rule| {
        let number = owned.iter().find(|s| s.skill_id == rule.skill_id && s.level == rule.level)?.number;
        let rested = last_used.get(&number).is_none_or(|&at| now >= at + rule.reset);
        (rule.attack_range >= distance && rule.sp <= health[2] && rule.mp <= health[1] && rested).then_some((number, *rule))
    })
}

impl Battle {
    pub fn new(cfg: Config) -> Self {
        let now = Instant::now();
        let mut battle = Self {
            cfg,
            state: State::Walk,
            milestones: Milestones::default(),
            target: None,
            attacking: None,
            wander: None,
            next_step_at: now,
            next_skill_at: now,
            skill_used: HashMap::new(),
            resume_attack_at: None,
            force_chase_until: None,
            chasing: None,
            ignored: None,
            died_at: None,
            rebirth_sent: false,
            after_rebirth: false,
            rng: StdRng::from_rng(&mut rand::rng()),
        };
        battle.next_skill_at = now + battle.skill_gap();
        battle
    }

    /// Moves the battle point (where a bot without an enemy in view goes and wanders).
    pub fn set_point(&mut self, point: [f32; 2]) {
        self.cfg.point = point;
        self.wander = None;
    }

    /// `skill_every` with 30 % jitter either way, so that 300 bots do not cast in step.
    fn skill_gap(&mut self) -> Duration {
        self.cfg.skill_every.mul_f64(self.rng.random_range(0.7..1.3))
    }

    fn reach(&self) -> f32 {
        self.cfg.weapon_range as f32 + REACH_PLUS - REACH_MARGIN
    }

    fn step_period(&self) -> Duration {
        Duration::from_secs_f32(1.0 / self.cfg.steps_per_sec.max(0.1))
    }

    /// What a packet changes in the bot's plan (the world and the counters are already updated).
    pub fn on_event(&mut self, me: u32, event: &Event, now: Instant) {
        match *event {
            Event::Died { id, .. } if id == me => {
                self.state = State::Dead;
                self.died_at = Some(now);
                self.rebirth_sent = false;
                self.target = None;
                self.attacking = None;
                self.resume_attack_at = None;
                self.chasing = None;
            }
            Event::Reborn(id) if id == me => {
                self.state = State::Walk;
                self.died_at = None;
                self.rebirth_sent = false;
                self.after_rebirth = true;
                self.milestones.steps_since_rebirth = 0;
                self.wander = None;
            }
            Event::Attack { result, attacker, .. } if attacker == me => match result {
                server_combat::RESULT_OUT_OF_RANGE | server_combat::RESULT_CANNOT_ATTACK => {
                    self.attacking = None;
                    self.force_chase_until = Some(now + CLOSE_IN);
                }
                server_combat::RESULT_NO_WEAPON => {
                    self.attacking = None;
                    self.milestones.unarmed = true;
                }
                _ => {}
            },
            Event::Skill { caster, .. } if caster == me => self.resume_attack_at = None,
            _ => {}
        }
    }

    /// The target to go for: the current one while it is an enemy in view and not given up on, else the nearest enemy.
    fn choose_target(&mut self, bot: &Bot, now: Instant) -> Option<(u32, f32)> {
        if self.ignored.is_some_and(|(_, until)| now >= until) {
            self.ignored = None;
        }
        let ignored = self.ignored.map(|(id, _)| id);
        let current = self.target.filter(|&t| Some(t) != ignored && bot.world.is_enemy_in_view(t));
        let chosen = match current {
            Some(t) => Some((t, distance(bot.world.position, bot.world.players[&t].position))),
            None => bot.world.nearest_enemy_except(ignored),
        };
        let id = chosen.map(|(id, _)| id);
        if id != self.target {
            self.attacking = None;
            self.resume_attack_at = None;
        }
        self.target = id;
        chosen
    }

    /// One thought: back to life when the wait is over, else a target, a step, an attack or a skill.
    pub async fn tick(&mut self, bot: &mut Bot, now: Instant) -> Result<(), LinkError> {
        if !bot.world.alive {
            if self.state != State::Dead {
                self.on_event(bot.world.id, &Event::Died { id: bot.world.id, killer_kind: 0, killer: 0 }, now);
            }
            let died_at = self.died_at.unwrap_or(now);
            if !self.rebirth_sent && now >= died_at + self.cfg.rebirth_after {
                self.rebirth_sent = true;
                self.milestones.rebirth_waits.push(now - died_at);
                bot.send(shaiya_proto::encode(combat::OP_REBIRTH_REQUEST, &[REBIRTH_KIND])).await?;
            }
            return Ok(());
        }
        let target = if self.cfg.idle || self.milestones.unarmed { None } else { self.choose_target(bot, now) };
        let Some((id, gap)) = target else {
            self.target = None;
            self.attacking = None;
            self.chasing = None;
            self.state = State::Walk;
            return self.walk(bot, now).await;
        };
        self.milestones.enemy_seen = true;
        let closing = self.force_chase_until.is_some_and(|until| now < until);
        if gap <= self.reach() && !closing {
            self.state = State::Fight(id);
            self.chasing = None;
            return self.fight(bot, id, gap, now).await;
        }
        self.state = State::Chase(id);
        match self.chasing {
            Some((chased, since)) if chased == id => {
                if now - since > self.cfg.chase_limit {
                    self.ignored = Some((id, now + self.cfg.chase_limit));
                    self.chasing = None;
                }
            }
            _ => self.chasing = Some((id, now)),
        }
        let goal = bot.world.players[&id].position;
        let stop_at = self.reach() * 0.9;
        self.step_toward(bot, [goal[0], goal[2]], stop_at, now).await
    }

    async fn fight(&mut self, bot: &mut Bot, target: u32, gap: f32, now: Instant) -> Result<(), LinkError> {
        if self.resume_attack_at.is_some_and(|at| now < at) {
            return Ok(());
        }
        self.resume_attack_at = None;
        if now >= self.next_skill_at {
            self.next_skill_at = now + self.skill_gap();
            let pick = pick_skill(&self.cfg.skills, &bot.world.skills, &self.skill_used, gap, bot.world.health, now);
            if let Some((number, _)) = pick {
                self.skill_used.insert(number, now);
                // A skill ends the attack; the attack is asked for again when the outcome comes (or after a wait).
                self.attacking = None;
                self.resume_attack_at = Some(now + SKILL_WAIT);
                bot.metrics.skill_sent(now);
                return bot.send(skill::use_request(skill::OP_USE_USER, number, target)).await;
            }
        }
        if self.attacking != Some(target) {
            self.attacking = Some(target);
            bot.metrics.attack_sent(target, now);
            bot.send(shaiya_proto::encode(combat::OP_ATTACK_USER, &target.to_le_bytes())).await?;
        }
        Ok(())
    }

    /// No enemy to go for: back to the point, then wander inside the radius. An idle bot stands at the point.
    async fn walk(&mut self, bot: &mut Bot, now: Instant) -> Result<(), LinkError> {
        let [px, pz] = self.cfg.point;
        let from_point = distance(bot.world.position, [px, 0.0, pz]);
        let goal = if self.cfg.idle {
            if from_point <= 1.0 {
                return Ok(());
            }
            [px, pz]
        } else if from_point > self.cfg.radius {
            self.wander = None;
            [px, pz]
        } else {
            let arrived = self.wander.is_none_or(|w| distance(bot.world.position, [w[0], 0.0, w[1]]) < 1.0);
            if arrived {
                // A point inside the circle, evenly spread.
                let (angle, r) = (self.rng.random_range(0.0..std::f32::consts::TAU), self.cfg.radius * self.rng.random::<f32>().sqrt());
                self.wander = Some([px + r * angle.cos(), pz + r * angle.sin()]);
            }
            self.wander.expect("set above")
        };
        self.step_toward(bot, goal, 0.0, now).await
    }

    /// One step toward `goal` when one is due: `STEP_SHARE` of what the speed allows in a step's time, at most 35, and not closer than
    /// `stop_at` to the goal.
    async fn step_toward(&mut self, bot: &mut Bot, goal: [f32; 2], stop_at: f32, now: Instant) -> Result<(), LinkError> {
        if now < self.next_step_at {
            return Ok(());
        }
        let period = self.step_period();
        // A bot that fell behind does not catch up with a burst of steps.
        self.next_step_at = if now > self.next_step_at + period { now + period } else { self.next_step_at + period };
        let [x, y, z] = bot.world.position;
        let (dx, dz) = (goal[0] - x, goal[1] - z);
        let remaining = dx.hypot(dz) - stop_at;
        if remaining < 0.05 {
            return Ok(());
        }
        let length = step_length(bot.world.speed(), period, remaining);
        let span = dx.hypot(dz);
        let (nx, nz) = (x + dx / span * length, z + dz / span * length);
        let dir = heading(dx, dz);
        // `y` is the last height the server sent for the bot (`0x0105`, `0x0551`, `0x020B`): the server's move check is horizontal.
        bot.send(wp::move_request(&wp::Move { motion: MOTION_RUN, dir, position: [nx, y, nz] })).await?;
        bot.world.stepped(nx, nz, dir);
        bot.metrics.steps += 1;
        // A step cancels the attack, so the next fight asks for it again.
        self.attacking = None;
        if self.after_rebirth {
            self.milestones.steps_since_rebirth += 1;
        }
        Ok(())
    }
}

/// Plays until `stop` completes or the connection ends. The bot keeps its counters and `battle` its milestones afterwards.
pub async fn run(bot: &mut Bot, battle: &mut Battle, stop: impl Future<Output = ()>) -> Result<(), LinkError> {
    tokio::pin!(stop);
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    bot.ask_about_new_players().await?;
    loop {
        tokio::select! {
            () = &mut stop => return Ok(()),
            events = bot.receive() => {
                let now = Instant::now();
                for event in &events? {
                    battle.on_event(bot.world.id, event, now);
                }
                bot.ask_about_new_players().await?;
            }
            _ = ticks.tick() => battle.tick(bot, Instant::now()).await?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(skill_id: u16, range: f32, sp: u16, reset_s: u64) -> SkillRule {
        SkillRule { skill_id, level: 1, attack_range: range, sp, mp: 0, reset: Duration::from_secs(reset_s) }
    }

    fn owned(id: u16, number: u8) -> SkillEntry {
        SkillEntry { skill_id: id, level: 1, number, delay: 0 }
    }

    #[test]
    fn the_first_skill_that_reaches_and_is_affordable_and_rested_is_used() {
        let now = Instant::now();
        let rules = [rule(10, 3.0, 5, 10), rule(11, 30.0, 50, 10), rule(12, 30.0, 5, 10), rule(99, 30.0, 0, 0)];
        let skills = [owned(10, 0), owned(11, 1), owned(12, 2)];
        let mut used = HashMap::new();
        // At 10 units skill 10 does not reach, 11 costs too much SP, 12 is the one; 99 is not known to the bot.
        assert_eq!(pick_skill(&rules, &skills, &used, 10.0, [100, 0, 20], now).map(|p| p.0), Some(2));
        // Close in, skill 10 comes first.
        assert_eq!(pick_skill(&rules, &skills, &used, 2.0, [100, 0, 20], now).map(|p| p.0), Some(0));
        // Skill 10 used just now: it rests for 10 s; skill 12 after it, and nothing when out of SP.
        used.insert(0, now);
        assert_eq!(pick_skill(&rules, &skills, &used, 2.0, [100, 0, 20], now + Duration::from_secs(9)).map(|p| p.0), Some(2));
        assert_eq!(pick_skill(&rules, &skills, &used, 2.0, [100, 0, 20], now + Duration::from_secs(11)).map(|p| p.0), Some(0));
        assert_eq!(pick_skill(&rules, &skills, &used, 2.0, [100, 0, 4], now), None);
    }

    #[test]
    fn a_skill_of_another_level_than_the_bot_has_is_not_used() {
        let rules = [SkillRule { level: 2, ..rule(10, 30.0, 0, 0) }];
        assert_eq!(pick_skill(&rules, &[owned(10, 0)], &HashMap::new(), 1.0, [1, 1, 1], Instant::now()), None);
    }

    #[test]
    fn a_death_waits_then_a_rebirth_walks_back() {
        let mut battle = Battle::new(Config::default());
        let now = Instant::now();
        battle.on_event(7, &Event::Died { id: 7, killer_kind: 1, killer: 8 }, now);
        assert_eq!(battle.state, State::Dead);
        battle.on_event(7, &Event::Died { id: 9, killer_kind: 1, killer: 8 }, now);
        assert_eq!(battle.state, State::Dead, "another player's death is not the bot's");
        battle.on_event(7, &Event::Reborn(7), now);
        assert_eq!(battle.state, State::Walk);
    }

    #[test]
    fn out_of_reach_makes_the_bot_close_in_and_no_weapon_stops_the_fighting() {
        let mut battle = Battle::new(Config::default());
        let now = Instant::now();
        battle.attacking = Some(8);
        battle.on_event(7, &Event::Attack { result: 5, attacker: 7, target: 0 }, now);
        assert_eq!(battle.attacking, None);
        assert!(battle.force_chase_until.is_some());
        battle.on_event(7, &Event::Attack { result: 5, attacker: 8, target: 7 }, now);
        battle.on_event(7, &Event::Attack { result: 7, attacker: 7, target: 0 }, now);
        assert!(battle.milestones.unarmed);
    }

    #[test]
    fn steps_are_nine_tenths_of_the_speed_and_never_long_enough_to_be_refused() {
        // Speed 5, two steps a second: 2.25. Never more than 35 or than what is left.
        assert_eq!(step_length(5.0, Duration::from_millis(500), 100.0), 2.25);
        assert_eq!(step_length(5.0, Duration::from_millis(500), 1.0), 1.0);
        assert_eq!(step_length(7.0, Duration::from_secs(60), 1000.0), 35.0);
    }

    #[test]
    fn the_heading_is_the_servers_truncated_degrees() {
        assert_eq!(heading(0.0, 5.0), 0);
        assert_eq!(heading(-3.0, 0.0), 90);
        assert!((0..360).contains(&heading(1.0, -1.0)));
        for (dx, dz) in [(1.0f32, 1.0f32), (-2.0, 3.0), (4.0, -0.5), (0.0, -1.0)] {
            assert_eq!(heading(dx, dz), game_server::world::degrees(dx.atan2(dz)));
        }
    }

    #[test]
    fn the_skill_gap_stays_within_the_jitter() {
        let mut battle = Battle::new(Config::default());
        for _ in 0..200 {
            let gap = battle.skill_gap();
            assert!(gap >= Duration::from_millis(4200) && gap <= Duration::from_millis(7800), "{gap:?}");
        }
    }
}
