//! `bot seed` and `bot unseed` (`docs/plans/test-bots.md` section 8): bot accounts, each with one level-60 character the generator
//! builds from the definitions by the server's own rules (or copies from a template character), written the way the game server
//! loads them; a second seed under the same prefix puts every bot back to that state; unseeding removes them. Both refuse (`Refused`,
//! exit code 2) when they would touch an account that is not a bot's or a bot that is in the game.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt;

use anyhow::{Context, Result};
use game_server::backend::{ItemUnit, NewItem};
use game_server::combat::{class_stat, max_level};
use game_server::creation::{RACE_CLASSES, starting_stats};
use game_server::enter::ExpTable;
use game_server::inventory::{Inventory, ItemDef, ItemDefs, SlotChange, Wearer};
use game_server::pg;
use game_server::skills::{self, ATTACK, Learner, QuickSlot, Skill, SkillDefs};
use game_server::stats::{self, StatusTable};
use rand::Rng;
use sqlx::PgPool;

/// The most bots one seed makes (one machine's sensible limit; a larger swarm is several `--first` ranges).
pub const MAX_COUNT: u32 = 1000;
/// The longest prefix: an account name is the prefix and four digits, and the client's names stop at 12.
pub const MAX_PREFIX: usize = 8;
/// The mode every generated character is in: normal.
const GROW: u8 = 1;
/// A bot's account may choose any mode (the original's maximum).
const MAX_GROW: i16 = 3;
/// Quick slots one bar of the client holds.
const BAR_SLOTS: usize = 10;
/// The `bag` a skill has in a quick slot (a real client's rows in the test database: bag 100, the skill's id as the number).
const QUICK_SKILL_BAG: u8 = 100;
/// The potions a bot carries of each kind, in bag 1.
const POTIONS: u32 = 20;
const POTION_TYPE: u8 = 25;
/// The equipment slots in the order they are filled: the shield before the weapon, so that a class with a shield gets a one-handed
/// weapon to go with it (`Inventory::can_equip`). Slot 13 is the mount, which a bot does not need.
const SLOT_ORDER: [u8; 13] = [6, 5, 0, 1, 2, 3, 4, 7, 8, 9, 10, 11, 12];
const WEAPON_SLOT: u8 = 5;

/// A guard said no (exit code 2): the message says which.
#[derive(Debug)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refused: {}", self.0)
    }
}

impl std::error::Error for Refused {}

fn refuse<T>(why: impl Into<String>) -> Result<T> {
    Err(Refused(why.into()).into())
}

/// What `seed` is asked for.
#[derive(Clone, Debug)]
pub struct Options {
    pub prefix: String,
    pub count: u32,
    pub map: u16,
    pub level: u16,
    /// Characters to copy instead of generating (a bot takes the template of its faction and class, else of its faction).
    pub templates: Vec<u32>,
    pub server_id: u8,
    /// Every bot account's password (`SHAIYA_PASSWORD`).
    pub password: String,
}

impl Options {
    pub fn new(prefix: &str, count: u32, password: &str) -> Self {
        Self { prefix: prefix.into(), count, map: 18, level: 60, templates: Vec::new(), server_id: 1, password: password.into() }
    }
}

/// The definitions the generator reads, loaded as the game server loads them.
pub struct Defs {
    pub items: ItemDefs,
    pub skills: SkillDefs,
    /// Every skill id of `defs.skills`, ascending.
    pub skill_ids: Vec<u16>,
    pub status: StatusTable,
    pub exp: ExpTable,
}

impl Defs {
    pub async fn load(pool: &PgPool) -> Result<Self> {
        let ids: Vec<i16> =
            sqlx::query_scalar("SELECT DISTINCT skillid FROM defs.skills WHERE skilllevel BETWEEN 1 AND 3 ORDER BY skillid")
                .fetch_all(pool)
                .await?;
        Ok(Self {
            items: pg::load_item_defs(pool).await.context("loading the item definitions")?,
            skills: pg::load_skill_defs(pool).await.context("loading the skill definitions")?,
            skill_ids: ids.into_iter().map(u16::try_from).collect::<Result<_, _>>()?,
            status: pg::load_status_table(pool).await.context("loading the status table")?,
            exp: pg::load_exp(pool).await.context("loading the experience table")?,
        })
    }
}

impl Defs {
    /// How far a character with these items and skills reaches, as the server counts it (`combat::gear`: the weapon's `range` in
    /// `defs.items`, the accessories' and lapis' and the passive skills' on top; an attack reaches that plus the target's size plus
    /// 2.3). What the swarm needs for a bot's `weapon_range`.
    pub fn attack_range(&self, items: &[ItemUnit], skills: &[Skill]) -> i32 {
        game_server::combat::gear(items, &self.items, &skills::passives(skills, &self.skills)).range
    }
}

/// A character as the generator or a template gives it, before a name and a place.
#[derive(Clone, Debug)]
pub struct Generated {
    pub family: u8,
    pub grow: u8,
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub job: u8,
    pub sex: u8,
    pub level: u16,
    pub exp: u32,
    pub stat_point: u16,
    pub skill_point: u16,
    /// STR, DEX, REC, INT, WIS, LUC.
    pub stats: [u16; 6],
    /// HP, MP, SP.
    pub health: [u16; 3],
    pub money: u32,
    /// Bag 0 (the equipment) and the bags.
    pub items: Vec<NewItem>,
    pub skills: Vec<Skill>,
    pub quick: Vec<QuickSlot>,
}

impl Generated {
    /// The faction (0 Light, 1 Fury) of its race.
    pub fn country(&self) -> u8 {
        self.family / 2
    }
}

/// A line of the report: one bot as seeded.
#[derive(Clone, Debug)]
pub struct BotLine {
    pub number: u32,
    pub username: String,
    pub character_id: u32,
    pub name: String,
    pub country: u8,
    pub family: u8,
    pub job: u8,
    pub level: u16,
    pub health: [u16; 3],
    /// The item id of the weapon in bag 0 slot 5.
    pub weapon: Option<u32>,
    /// Items in bag 0.
    pub gear: usize,
    pub skills: usize,
    /// How far it reaches (`Defs::attack_range`).
    pub range: i32,
    pub position: [f32; 3],
    /// An existing bot put back, not a new one.
    pub reset: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Seeded {
    pub bots: Vec<BotLine>,
}

impl Seeded {
    pub fn created(&self) -> usize {
        self.bots.iter().filter(|b| !b.reset).count()
    }

    pub fn reset(&self) -> usize {
        self.bots.iter().filter(|b| b.reset).count()
    }
}

/// The faction and class of bot number `number` (from 1): odd numbers Light, even Fury, the classes cycling in each faction.
pub fn bot_kind(number: u32) -> (u8, u8) {
    let country = if number % 2 == 1 { 0 } else { 1 };
    (country, (((number - 1) / 2) % 6) as u8)
}

/// The first race of the faction whose class table has the class (`creation::RACE_CLASSES`: Light is races 0 and 1, Fury 2 and 3).
pub fn race_for(country: u8, job: u8) -> Option<u8> {
    (country * 2..country * 2 + 2).find(|&family| RACE_CLASSES[usize::from(family)].contains(&job))
}

/// The stat a bot's stat points go to: what the class fights with.
fn main_stat(job: u8) -> usize {
    match job {
        0 | 1 => 0,
        2 | 3 => 1,
        4 => 3,
        _ => 4,
    }
}

fn unit(item: &NewItem) -> ItemUnit {
    ItemUnit {
        bag: item.bag,
        slot: item.slot,
        item_type: item.item_type,
        type_id: item.type_id,
        quality: item.quality,
        count: item.count,
        ..Default::default()
    }
}

fn new_item(def: &ItemDef, bag: u8, slot: u8, count: u8) -> NewItem {
    NewItem { bag, slot, item_id: def.item_id, item_type: def.item_type, type_id: def.type_id, quality: def.quality, count, kept: None }
}

/// The item for each equipment slot: of the definitions `Inventory::can_equip` accepts for this wearer, the one with the highest
/// `reqlevel` (ties: for the weapon the longest `range`, so a bow beats a sword of its level; then the lowest item id), at its definition's quality and with no gems.
pub fn pick_gear(items: &ItemDefs, wearer: &Wearer) -> Vec<NewItem> {
    let mut inventory = Inventory::new(&[]);
    let mut gear = Vec::new();
    for slot in SLOT_ORDER {
        let best = items
            .iter()
            .filter(|def| def.item_id != 0 && inventory.can_equip(items, def, slot, wearer))
            .max_by_key(|def| (def.level, if slot == WEAPON_SLOT { def.range } else { 0 }, Reverse(def.item_id)));
        if let Some(def) = best {
            let item = new_item(def, 0, slot, 1);
            inventory.apply(&SlotChange::Insert(item));
            gear.push(item);
        }
    }
    gear
}

/// 20 of the first potion that heals only one of HP (`health` 0), MP (1) or SP (2), in stacks from bag 1 slot `slot`.
fn potions(items: &ItemDefs, health: usize, slot: &mut u8) -> Vec<NewItem> {
    let only =
        |def: &&ItemDef| def.item_type == POTION_TYPE && (0..3).all(|i| (def.const_health[i] > 0) == (i == health)) && def.max_count > 0;
    let Some(def) = items.iter().filter(only).min_by_key(|def| def.type_id) else { return Vec::new() };
    let mut left = POTIONS;
    let mut stacks = Vec::new();
    while left > 0 {
        let count = left.min(u32::from(def.max_count)) as u8;
        stacks.push(new_item(def, 1, *slot, count));
        *slot += 1;
        left -= u32::from(count);
    }
    stacks
}

/// Every skill the class may learn, at the highest level its `reqlevel` allows, numbered as learning numbers them (the count at the
/// time, so 0, 1, 2, ... in skill id order), by `skills::learn` with all the points it could ask for.
fn learn_all(defs: &Defs, learner: &Learner) -> Vec<Skill> {
    let learner = Learner { skill_points: u16::MAX, ..*learner };
    let mut known: Vec<Skill> = Vec::new();
    for &id in &defs.skill_ids {
        for level in 1..=skills::MAX_LEVEL {
            if let Ok((skill, _)) = skills::learn(&learner, &known, id, level, &defs.skills) {
                match known.iter_mut().find(|s| s.id == id) {
                    Some(have) => *have = skill,
                    None => known.push(skill),
                }
            }
        }
    }
    known
}

/// The generator of section 8.2: the character of this faction and class at `level`, in the normal mode.
pub fn generate(defs: &Defs, country: u8, job: u8, level: u16) -> Result<Generated> {
    let Some(family) = race_for(country, job) else { return refuse(format!("no race of faction {country} plays class {job}")) };
    if level == 0 || level > max_level(GROW) {
        return refuse(format!("level {level} is outside the normal mode's 1 to {}", max_level(GROW)));
    }
    let (exp, _) = defs.exp.bounds(GROW, level);
    if level > 1 && exp == 0 {
        return refuse("the experience table has no row for that level in the normal mode");
    }
    // The levels gained give their stat points, and the class's own stat one more each, as `combat::add_exp` does.
    let gained = u32::from(level - 1);
    let mut stats = starting_stats(job, family);
    if let Some(own) = class_stat(job) {
        stats[own] = stats[own].saturating_add(gained as u16);
    }
    stats[main_stat(job)] = stats[main_stat(job)].saturating_add((gained * 5) as u16);

    let wearer = Wearer { level, family, job, grow: GROW, sex: 0 };
    let mut items = pick_gear(&defs.items, &wearer);
    let mut slot = 0;
    items.extend(potions(&defs.items, 0, &mut slot));
    items.extend(potions(&defs.items, 1, &mut slot));

    let learner = Learner { level, skill_points: 0, family, job, grow: GROW };
    let skills = learn_all(defs, &learner);
    let quick: Vec<QuickSlot> = {
        let mut attacks: Vec<&Skill> =
            skills.iter().filter(|s| defs.skills.get(s.id, s.level).is_some_and(|d| d.type_effect == ATTACK && !d.not_built())).collect();
        attacks.sort_by_key(|s| s.number);
        attacks
            .into_iter()
            .take(BAR_SLOTS)
            .enumerate()
            .map(|(slot, s)| QuickSlot { bar: 0, slot: slot as u8, bag: QUICK_SKILL_BAG, number: s.id })
            .collect()
    };

    // The maxima the server computes for it (`server::totals`): the status table, the stats, the gear and the passive skills.
    let base = stats::Base { job, level, grow: GROW, family, stats, grade: Default::default() };
    let units: Vec<ItemUnit> = items.iter().map(unit).collect();
    let totals = stats::compute(&defs.status, &base, &units, &defs.items, &skills::passives(&skills, &defs.skills));
    Ok(Generated {
        family,
        grow: GROW,
        hair: 0,
        face: 0,
        size: 0,
        job,
        sex: 0,
        level,
        exp,
        stat_point: 0,
        skill_point: 0,
        stats,
        health: totals.max_health(),
        money: 0,
        items,
        skills,
        quick,
    })
}

/// The boxes of a map's bind areas that a faction's players come back in (`World::bind_pos`: an area of its own country, 3 or 0).
pub async fn load_boxes(pool: &PgPool, map: u16, country: u8) -> Result<Vec<[f32; 6]>> {
    let own = if country == 0 { 1 } else { 2 };
    let rows: Vec<(i32, Vec<f32>)> = sqlx::query_as("SELECT country, boxes FROM world.map_binds WHERE map = $1 ORDER BY seq")
        .bind(i16::try_from(map)?)
        .fetch_all(pool)
        .await?;
    let area = rows.into_iter().find(|(c, _)| *c == own || *c == 3 || *c == 0);
    Ok(area.map(|(_, boxes)| boxes.as_chunks::<6>().0.to_vec()).unwrap_or_default())
}

/// A spot of one of the boxes, as the server picks a rebirth spot (whole-number offsets; the height of the box's floor plus 0.9).
pub fn spot_in(boxes: &[[f32; 6]], rng: &mut impl Rng) -> Option<[f32; 3]> {
    let b = boxes.get(rng.random_range(0..boxes.len().max(1)))?;
    let spread = |from: f32, to: f32| ((to - from) as i32).max(1) as u32;
    let x = b[0] + rng.random_range(0..spread(b[0], b[3])) as f32;
    let z = b[2] + rng.random_range(0..spread(b[2], b[5])) as f32;
    Some([x, b[1] + 0.9, z])
}

/// A character copied from the database: its row, its bags, skills and quick slots. It must exist, and its account must be out of the game.
#[derive(sqlx::FromRow)]
struct TemplateRow {
    user_id: i32,
    family: i16,
    grow: i16,
    hair: i16,
    face: i16,
    size: i16,
    job: i16,
    sex: i16,
    level: i16,
    stat_point: i16,
    skill_point: i16,
    str: i16,
    dex: i16,
    rec: i16,
    int: i16,
    wis: i16,
    luc: i16,
    hp: i16,
    mp: i16,
    sp: i16,
    exp: i64,
    money: i64,
}

pub async fn load_template(pool: &PgPool, id: u32, server_id: u8) -> Result<Generated> {
    let row: Option<TemplateRow> = sqlx::query_as(
        "SELECT user_id, family, grow, hair, face, size, job, sex, level, stat_point, skill_point, str, dex, rec, int, wis, luc,
                hp, mp, sp, exp, money
           FROM characters WHERE id = $1 AND server_id = $2 AND deleted_at IS NULL",
    )
    .bind(i32::try_from(id)?)
    .bind(i16::from(server_id))
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return refuse(format!("template character {id} does not exist on server {server_id}")) };
    let user = row.user_id;
    let in_game: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sessions WHERE user_id = $1 AND game_connected_at IS NOT NULL AND expires_at > now())",
    )
    .bind(user)
    .fetch_one(pool)
    .await?;
    if in_game {
        return refuse(format!("template character {id} is in the game: log it out first"));
    }
    let u8_of = |v: i16| u8::try_from(v);
    let u16_of = |v: i16| u16::try_from(v);
    let items: Vec<(i16, i16, i32, i16, i16, i32, i16)> = sqlx::query_as(
        "SELECT bag, slot, item_id, type, type_id, quality, count FROM character_items WHERE character_id = $1 ORDER BY bag, slot",
    )
    .bind(i32::try_from(id)?)
    .fetch_all(pool)
    .await?;
    let skills: Vec<(i16, i16, i16)> =
        sqlx::query_as("SELECT number, skill_id, level FROM character_skills WHERE character_id = $1 ORDER BY number")
            .bind(i32::try_from(id)?)
            .fetch_all(pool)
            .await?;
    let quick: Vec<(i16, i16, i16, i32)> =
        sqlx::query_as("SELECT bar, slot, bag, number FROM character_quick_slots WHERE character_id = $1 ORDER BY position")
            .bind(i32::try_from(id)?)
            .fetch_all(pool)
            .await?;
    Ok(Generated {
        family: u8_of(row.family)?,
        grow: u8_of(row.grow)?,
        hair: u8_of(row.hair)?,
        face: u8_of(row.face)?,
        size: u8_of(row.size)?,
        job: u8_of(row.job)?,
        sex: u8_of(row.sex)?,
        level: u16_of(row.level)?,
        stat_point: u16_of(row.stat_point)?,
        skill_point: u16_of(row.skill_point)?,
        stats: [row.str, row.dex, row.rec, row.int, row.wis, row.luc].map(|v| v.max(0) as u16),
        health: [row.hp, row.mp, row.sp].map(|v| v.max(0) as u16),
        exp: u32::try_from(row.exp)?,
        money: u32::try_from(row.money)?,
        items: items
            .into_iter()
            .map(|(bag, slot, item_id, item_type, type_id, quality, count)| {
                Ok(NewItem {
                    bag: u8::try_from(bag)?,
                    slot: u8::try_from(slot)?,
                    item_id: u32::try_from(item_id)?,
                    item_type: u8::try_from(item_type)?,
                    type_id: u8::try_from(type_id)?,
                    quality: u16::try_from(quality)?,
                    count: u8::try_from(count)?,
                    kept: None,
                })
            })
            .collect::<Result<_>>()?,
        skills: skills
            .into_iter()
            .map(|(number, id, level)| {
                Ok(Skill { id: u16::try_from(id)?, level: u8::try_from(level)?, number: u8::try_from(number)?, delay: 0 })
            })
            .collect::<Result<_>>()?,
        quick: quick
            .into_iter()
            .map(|(bar, slot, bag, number)| {
                Ok(QuickSlot {
                    bar: u8::try_from(bar)?,
                    slot: u8::try_from(slot)?,
                    bag: u8::try_from(bag)?,
                    number: u16::try_from(number)?,
                })
            })
            .collect::<Result<_>>()?,
    })
}

/// The account name of bot `number`, and its character's name (the first letter capitalised, as the client shows names).
pub fn bot_names(prefix: &str, number: u32) -> (String, String) {
    let username = format!("{prefix}{number:04}");
    let mut name = username.clone();
    name[..1].make_ascii_uppercase();
    (username, name)
}

pub fn check_prefix(prefix: &str) -> Result<()> {
    if prefix.is_empty() || prefix.len() > MAX_PREFIX || !prefix.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return refuse(format!("a prefix is 1 to {MAX_PREFIX} ASCII letters or digits (the account is the prefix and four digits)"));
    }
    Ok(())
}

/// One existing account the prefix matches.
struct Existing {
    user_id: i32,
    username: String,
    /// The character `seed` made for it, when that is what it has.
    character: Option<i32>,
}

/// The guards of section 8.4 that both commands share: the accounts named `<prefix>` and four digits are all bots' (an account of a
/// player, or of anyone else, that happens to match refuses the prefix), and none of them is in the game.
async fn bot_accounts(pool: &PgPool, prefix: &str) -> Result<Vec<Existing>> {
    check_prefix(prefix)?;
    let pattern = format!("^{prefix}[0-9]{{4}}$");
    let accounts: Vec<(i32, String, i16)> =
        sqlx::query_as("SELECT id, username, status FROM accounts WHERE username ~* $1 ORDER BY username")
            .bind(&pattern)
            .fetch_all(pool)
            .await?;
    let mut found = Vec::new();
    for (user_id, username, status) in accounts {
        let characters: Vec<(i32, String)> =
            sqlx::query_as("SELECT id, name FROM characters WHERE user_id = $1 ORDER BY id DESC").bind(user_id).fetch_all(pool).await?;
        let made_by_seed = status == 0 && !characters.is_empty() && characters.iter().all(|(_, name)| name.eq_ignore_ascii_case(&username));
        if !made_by_seed {
            return refuse(format!(
                "the account {username} matches the prefix but is not a bot's (status {status}, characters {characters:?})"
            ));
        }
        found.push(Existing { user_id, username, character: characters.first().map(|c| c.0) });
    }
    let in_game: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sessions s JOIN accounts a ON a.id = s.user_id
          WHERE a.username ~* $1 AND s.game_connected_at IS NOT NULL AND s.expires_at > now()",
    )
    .bind(&pattern)
    .fetch_one(pool)
    .await?;
    if in_game > 0 {
        return refuse(format!("{in_game} bot account(s) of this prefix are in the game: stop the swarm first"));
    }
    Ok(found)
}

/// Which template a bot takes: its faction and class, else the first of its faction.
fn template_for(templates: &[Generated], country: u8, job: u8) -> Option<&Generated> {
    let mine = || templates.iter().filter(|t| t.country() == country);
    mine().find(|t| t.job == job).or_else(|| mine().next())
}

/// Whose character it is and where it stands.
#[derive(Clone, Copy)]
struct Place<'a> {
    user_id: i32,
    server_id: u8,
    /// The character `seed` made before, to put back.
    existing: Option<i32>,
    name: &'a str,
    map: u16,
    at: [f32; 3],
}

/// Writes a character's row and what hangs on it into `tx`: a new one for `user_id`, or the existing `character` put back (its row
/// overwritten, its bags, skills, quick slots and applied skills replaced). Returns its id.
async fn write_character(tx: &mut sqlx::PgConnection, place: &Place<'_>, g: &Generated) -> Result<i32> {
    let Place { user_id, server_id, existing, name, map, at } = *place;
    let id = match existing {
        Some(id) => {
            sqlx::query(
                "UPDATE characters SET server_id = $2, slot = 0, name = $3, family = $4, grow = $5, hair = $6, face = $7, size = $8, job = $9,
                        sex = $10, level = $11, stat_point = $12, skill_point = $13, str = $14, dex = $15, rec = $16, int = $17, wis = $18,
                        luc = $19, hp = $20, mp = $21, sp = $22, map = $23, dir = 0, exp = $24, money = $25, pos_x = $26, pos_y = $27,
                        pos_z = $28, kills = 0, deaths = 0, victories = 0, defeats = 0, kill_level = 0, dead_level = 0,
                        delete_requested_at = NULL, deleted_at = NULL, fallen_at = NULL
                  WHERE id = $1",
            )
            .bind(id)
            .bind(i16::from(server_id))
            .bind(name)
            .bind(i16::from(g.family))
            .bind(i16::from(g.grow))
            .bind(i16::from(g.hair))
            .bind(i16::from(g.face))
            .bind(i16::from(g.size))
            .bind(i16::from(g.job))
            .bind(i16::from(g.sex))
            .bind(i16::try_from(g.level)?)
            .bind(i16::try_from(g.stat_point)?)
            .bind(i16::try_from(g.skill_point)?)
            .bind(i16::try_from(g.stats[0])?)
            .bind(i16::try_from(g.stats[1])?)
            .bind(i16::try_from(g.stats[2])?)
            .bind(i16::try_from(g.stats[3])?)
            .bind(i16::try_from(g.stats[4])?)
            .bind(i16::try_from(g.stats[5])?)
            .bind(i16::try_from(g.health[0])?)
            .bind(i16::try_from(g.health[1])?)
            .bind(i16::try_from(g.health[2])?)
            .bind(i16::try_from(map)?)
            .bind(i64::from(g.exp))
            .bind(i64::from(g.money))
            .bind(at[0])
            .bind(at[1])
            .bind(at[2])
            .execute(&mut *tx)
            .await?;
            sqlx::query("DELETE FROM character_items WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM character_skills WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM character_quick_slots WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM character_applied_skills WHERE character_id = $1").bind(id).execute(&mut *tx).await?;
            id
        }
        None => {
            sqlx::query_scalar(
                "INSERT INTO characters (user_id, server_id, slot, name, family, grow, hair, face, size, job, sex, level, stat_point,
                        skill_point, str, dex, rec, int, wis, luc, hp, mp, sp, map, dir, exp, money, pos_x, pos_y, pos_z)
                 VALUES ($1, $2, 0, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, 0, $24,
                         $25, $26, $27, $28)
                 RETURNING id",
            )
            .bind(user_id)
            .bind(i16::from(server_id))
            .bind(name)
            .bind(i16::from(g.family))
            .bind(i16::from(g.grow))
            .bind(i16::from(g.hair))
            .bind(i16::from(g.face))
            .bind(i16::from(g.size))
            .bind(i16::from(g.job))
            .bind(i16::from(g.sex))
            .bind(i16::try_from(g.level)?)
            .bind(i16::try_from(g.stat_point)?)
            .bind(i16::try_from(g.skill_point)?)
            .bind(i16::try_from(g.stats[0])?)
            .bind(i16::try_from(g.stats[1])?)
            .bind(i16::try_from(g.stats[2])?)
            .bind(i16::try_from(g.stats[3])?)
            .bind(i16::try_from(g.stats[4])?)
            .bind(i16::try_from(g.stats[5])?)
            .bind(i16::try_from(g.health[0])?)
            .bind(i16::try_from(g.health[1])?)
            .bind(i16::try_from(g.health[2])?)
            .bind(i16::try_from(map)?)
            .bind(i64::from(g.exp))
            .bind(i64::from(g.money))
            .bind(at[0])
            .bind(at[1])
            .bind(at[2])
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query(
        "INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, count)
         SELECT $1, * FROM unnest($2::int2[], $3::int2[], $4::int4[], $5::int2[], $6::int2[], $7::int4[], $8::int2[])",
    )
    .bind(id)
    .bind(g.items.iter().map(|i| i16::from(i.bag)).collect::<Vec<_>>())
    .bind(g.items.iter().map(|i| i16::from(i.slot)).collect::<Vec<_>>())
    .bind(g.items.iter().map(|i| i32::try_from(i.item_id)).collect::<Result<Vec<_>, _>>()?)
    .bind(g.items.iter().map(|i| i16::from(i.item_type)).collect::<Vec<_>>())
    .bind(g.items.iter().map(|i| i16::from(i.type_id)).collect::<Vec<_>>())
    .bind(g.items.iter().map(|i| i32::from(i.quality)).collect::<Vec<_>>())
    .bind(g.items.iter().map(|i| i16::from(i.count)).collect::<Vec<_>>())
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO character_skills (character_id, number, skill_id, level)
         SELECT $1, * FROM unnest($2::int2[], $3::int2[], $4::int2[])",
    )
    .bind(id)
    .bind(g.skills.iter().map(|s| i16::from(s.number)).collect::<Vec<_>>())
    .bind(g.skills.iter().map(|s| i16::try_from(s.id)).collect::<Result<Vec<_>, _>>()?)
    .bind(g.skills.iter().map(|s| i16::from(s.level)).collect::<Vec<_>>())
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO character_quick_slots (character_id, position, bar, slot, bag, number)
         SELECT $1, * FROM unnest($2::int2[], $3::int2[], $4::int2[], $5::int2[], $6::int4[])",
    )
    .bind(id)
    .bind((0..g.quick.len()).map(|p| p as i16).collect::<Vec<_>>())
    .bind(g.quick.iter().map(|q| i16::from(q.bar)).collect::<Vec<_>>())
    .bind(g.quick.iter().map(|q| i16::from(q.slot)).collect::<Vec<_>>())
    .bind(g.quick.iter().map(|q| i16::from(q.bag)).collect::<Vec<_>>())
    .bind(g.quick.iter().map(|q| i32::from(q.number)).collect::<Vec<_>>())
    .execute(&mut *tx)
    .await?;
    Ok(id)
}

/// `bot seed`: makes bots `<prefix>0001` to `<prefix>NNNN`, or puts the ones that exist back to their state (section 8.3). One
/// transaction per bot.
pub async fn seed(pool: &PgPool, defs: &Defs, options: &Options) -> Result<Seeded> {
    if options.count == 0 || options.count > MAX_COUNT {
        return refuse(format!("--count is 1 to {MAX_COUNT}: a larger swarm is several ranges"));
    }
    let existing: HashMap<String, Existing> =
        bot_accounts(pool, &options.prefix).await?.into_iter().map(|e| (e.username.to_ascii_lowercase(), e)).collect();
    let mut templates = Vec::new();
    for &id in &options.templates {
        templates.push(load_template(pool, id, options.server_id).await?);
    }
    let hash = {
        let password = options.password.clone();
        login_server::accounts::validate_credentials(&bot_names(&options.prefix, 1).0, &options.password).context("the password")?;
        tokio::task::spawn_blocking(move || login_server::accounts::hash_password(&password)).await??
    };
    let mut boxes: HashMap<u8, Vec<[f32; 6]>> = HashMap::new();
    for country in 0..2 {
        boxes.insert(country, load_boxes(pool, options.map, country).await?);
    }
    // Decided before anything is written: a faction with no box on the map, or no template, stops the whole seed.
    for country in 0..2 {
        if boxes[&country].is_empty() {
            return refuse(format!("map {} has no rebirth box for faction {country} (world.map_binds)", options.map));
        }
    }
    let mut rng = rand::rng();
    let mut seeded = Seeded::default();
    for number in 1..=options.count {
        let (country, job) = bot_kind(number);
        let (username, name) = bot_names(&options.prefix, number);
        let generated = if options.templates.is_empty() {
            generate(defs, country, job, options.level)?
        } else {
            match template_for(&templates, country, job) {
                Some(t) => t.clone(),
                None => return refuse(format!("no template for faction {country} (bot {number})")),
            }
        };
        let at = spot_in(&boxes[&country], &mut rng).expect("boxes checked non-empty");
        let found = existing.get(&username.to_ascii_lowercase());
        let mut tx = pool.begin().await?;
        let user_id = match found {
            Some(e) => e.user_id,
            None => {
                sqlx::query_scalar("INSERT INTO accounts (username, password_hash, status) VALUES ($1, $2, 0) RETURNING id")
                    .bind(&username)
                    .bind(&hash)
                    .fetch_one(&mut *tx)
                    .await?
            }
        };
        sqlx::query(
            "INSERT INTO account_factions (user_id, server_id, country, max_grow) VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id, server_id) DO UPDATE SET country = EXCLUDED.country, max_grow = EXCLUDED.max_grow",
        )
        .bind(user_id)
        .bind(i16::from(options.server_id))
        .bind(i16::from(generated.country()))
        .bind(MAX_GROW)
        .execute(&mut *tx)
        .await?;
        let place =
            Place { user_id, server_id: options.server_id, existing: found.and_then(|e| e.character), name: &name, map: options.map, at };
        let id = write_character(&mut tx, &place, &generated).await.with_context(|| format!("writing {username}"))?;
        tx.commit().await?;
        seeded.bots.push(BotLine {
            number,
            username,
            character_id: u32::try_from(id)?,
            name,
            country: generated.country(),
            family: generated.family,
            job: generated.job,
            level: generated.level,
            health: generated.health,
            weapon: generated.items.iter().find(|i| i.bag == 0 && i.slot == WEAPON_SLOT).map(|i| i.item_id),
            gear: generated.items.iter().filter(|i| i.bag == 0).count(),
            skills: generated.skills.len(),
            range: defs.attack_range(&generated.items.iter().map(unit).collect::<Vec<_>>(), &generated.skills),
            position: at,
            reset: found.is_some(),
        });
    }
    Ok(seeded)
}

/// `bot unseed` (section 8.5): removes the prefix's characters, with what hangs on them, and accounts, in one transaction, after the
/// guards. The logs stay (the login log keeps its rows, no longer pointing at an account). Returns how many accounts went.
pub async fn unseed(pool: &PgPool, prefix: &str) -> Result<usize> {
    let bots = bot_accounts(pool, prefix).await?;
    if bots.is_empty() {
        return Ok(0);
    }
    let users: Vec<i32> = bots.iter().map(|b| b.user_id).collect();
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM character_renames WHERE character_id IN (SELECT id FROM characters WHERE user_id = ANY($1))")
        .bind(&users)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM characters WHERE user_id = ANY($1)").bind(&users).execute(&mut *tx).await?;
    sqlx::query("UPDATE login_log SET account_id = NULL WHERE account_id = ANY($1)").bind(&users).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM accounts WHERE id = ANY($1)").bind(&users).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(bots.len())
}

/// The command lines of `bot seed` and `bot unseed`.
pub mod cli {
    use anyhow::{Result, bail};

    use super::{Options, Refused};

    pub const SEED_USAGE: &str =
        "usage: bot seed --count <n> [--prefix bot] [--map 18] [--level 60] [--server 1] [--template <character id>]...";
    pub const UNSEED_USAGE: &str = "usage: bot unseed [--prefix bot]";

    fn value<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str> {
        args.get(i + 1).map(String::as_str).ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
    }

    /// `args` are those after `seed`; the password is `SHAIYA_PASSWORD`'s.
    pub fn parse_seed(args: &[String], password: &str) -> Result<Options> {
        let mut options = Options::new("bot", 0, password);
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            match flag {
                "--count" => options.count = value(args, i, flag)?.parse()?,
                "--prefix" => options.prefix = value(args, i, flag)?.to_owned(),
                "--map" => options.map = value(args, i, flag)?.parse()?,
                "--level" => options.level = value(args, i, flag)?.parse()?,
                "--server" => options.server_id = value(args, i, flag)?.parse()?,
                "--template" => options.templates.push(value(args, i, flag)?.parse()?),
                _ => bail!("{SEED_USAGE}"),
            }
            i += 2;
        }
        if options.count == 0 {
            return Err(Refused("--count is required (1 to 1000)".into()).into());
        }
        Ok(options)
    }

    /// `args` are those after `unseed`; the prefix.
    pub fn parse_unseed(args: &[String]) -> Result<String> {
        match args {
            [] => Ok("bot".into()),
            [flag, prefix] if flag == "--prefix" => Ok(prefix.clone()),
            _ => bail!("{UNSEED_USAGE}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn bots_alternate_factions_and_cycle_the_classes() {
        let kinds: Vec<(u8, u8)> = (1..=12).map(bot_kind).collect();
        assert_eq!(kinds, [(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2), (0, 3), (1, 3), (0, 4), (1, 4), (0, 5), (1, 5)]);
        // 300 bots: 25 of each faction and class.
        let mut counts = HashMap::new();
        for n in 1..=300 {
            *counts.entry(bot_kind(n)).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), 12);
        assert!(counts.values().all(|&c| c == 25));
    }

    #[test]
    fn the_first_race_of_the_faction_that_has_the_class() {
        // Light: human 0 1 5, elf 2 3 4. Fury: vail 2 4 5 (race 2), deatheater 0 1 3 (race 3).
        let light: Vec<u8> = (0..6).map(|job| race_for(0, job).unwrap()).collect();
        let fury: Vec<u8> = (0..6).map(|job| race_for(1, job).unwrap()).collect();
        assert_eq!(light, [0, 0, 1, 1, 1, 0]);
        assert_eq!(fury, [3, 3, 2, 3, 2, 2]);
    }

    #[test]
    fn names_are_the_prefix_and_four_digits() {
        assert_eq!(bot_names("bot", 7), ("bot0007".into(), "Bot0007".into()));
        assert_eq!(bot_names("z1", 300), ("z10300".into(), "Z10300".into()));
    }

    #[test]
    fn a_spot_is_inside_its_box_at_the_floor_plus_nine_tenths() {
        let boxes = [[405.3f32, 19.5, 139.0, 412.4, 21.5, 146.7]];
        let mut rng = rand::rng();
        for _ in 0..200 {
            let [x, y, z] = spot_in(&boxes, &mut rng).unwrap();
            assert!((405.3..412.4).contains(&x) && (139.0..146.7).contains(&z), "{x} {z}");
            assert!((y - 20.4).abs() < 0.01);
        }
        assert_eq!(spot_in(&[], &mut rng), None);
    }

    #[test]
    fn the_command_lines() {
        let o = cli::parse_seed(&args("--count 300 --prefix zz --map 18 --level 50 --template 4 --template 9"), "password").unwrap();
        assert_eq!((o.count, o.prefix.as_str(), o.map, o.level, o.templates.as_slice()), (300, "zz", 18, 50, &[4, 9][..]));
        assert_eq!((o.server_id, o.password.as_str()), (1, "password"));
        let none = cli::parse_seed(&args("--prefix zz"), "password").unwrap_err();
        assert!(none.downcast_ref::<Refused>().is_some(), "no --count is a refusal");
        assert!(cli::parse_seed(&args("--bogus 1"), "p").is_err());
        assert!(cli::parse_seed(&args("--count"), "p").is_err());
        assert_eq!(cli::parse_unseed(&[]).unwrap(), "bot");
        assert_eq!(cli::parse_unseed(&args("--prefix zz")).unwrap(), "zz");
        assert!(cli::parse_unseed(&args("zz")).is_err());
    }

    #[test]
    fn prefixes_are_short_alphanumeric_names() {
        assert!(check_prefix("bot").is_ok() && check_prefix("z1a2b3c4").is_ok());
        for bad in ["", "toolongpre", "b_t", "b t", "b%"] {
            assert!(check_prefix(bad).is_err(), "{bad:?}");
        }
    }
}
