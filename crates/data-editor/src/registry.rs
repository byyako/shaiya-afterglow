//! The editor's targets (`docs/plans/done/data-editing.md` section 2): which tables and fields may change, under which rules, and when a
//! change takes effect. The rules are the ones the game server applies when it loads a row, so the editor never writes a row the server
//! would skip. Pure: the facts it needs from the database (map sizes, monsters that can spawn) come in as [`Facts`].

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Number, Value};

/// A refusal, worded for the page.
pub type Check<T> = Result<T, String>;

/// When a change takes effect (section 2): at once through `reload`, for monsters spawned after the reload, or after a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    Live,
    NextSpawn,
    Restart,
}

impl Effect {
    pub fn name(self) -> &'static str {
        match self {
            Effect::Live => "live",
            Effect::NextSpawn => "next spawn",
            Effect::Restart => "restart",
        }
    }
}

/// Which loaded schema a table lives in (`defs` is restored from the original database, `world` imported from its files).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Defs,
    World,
}

/// The schema names; parameters so that the tests can use scratch schemas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schemas {
    pub defs: String,
    pub world: String,
    pub edits: String,
}

impl Default for Schemas {
    fn default() -> Self {
        Self { defs: "defs".into(), world: "world".into(), edits: "edits".into() }
    }
}

impl Schemas {
    pub fn of(&self, side: Side) -> &str {
        match side {
            Side::Defs => &self.defs,
            Side::World => &self.world,
        }
    }

    /// `schema.table` of a target.
    pub fn table(&self, target: &Target) -> String {
        format!("{}.{}", self.of(target.side), target.table)
    }
}

/// A column's SQL type, which says how a JSON value is cast into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sql {
    Int2,
    Int4,
    Real,
    Ints,
    Reals,
}

impl Sql {
    /// The expression that turns the jsonb parameter `param` into a value of the column's type.
    pub fn cast(self, param: &str) -> String {
        let list = |ty: &str| {
            format!("ARRAY(SELECT e FROM jsonb_array_elements_text({param}::jsonb) WITH ORDINALITY AS t(e, n) ORDER BY n)::{ty}[]")
        };
        match self {
            Sql::Int2 => format!("(({param}::jsonb) #>> '{{}}')::smallint"),
            Sql::Int4 => format!("(({param}::jsonb) #>> '{{}}')::integer"),
            Sql::Real => format!("(({param}::jsonb) #>> '{{}}')::real"),
            Sql::Ints => list("integer"),
            Sql::Reals => list("real"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// What a field's value must be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A whole number within the range.
    Int(i64, i64),
    /// A coordinate: x and z inside the row's map (`0 <= v < size`, `CMap::IsSafePos`), y any real.
    Coord(Axis),
    /// A real greater than 0.
    Radius,
    /// A monster that can spawn (`MobDefs::get`: id, HP and level not 0).
    Mob,
    /// A map of `world.maps`.
    MapId,
    /// A drop entry's chance: 0 to 100 (a percent) for entries 1 to 6, 0 to 1,000,000 (per million) for 7 to 9.
    DropRate,
    /// A portal's country: 0, 1, 2; a value above 1000 is an instance portal's id and stays.
    PortalCountry,
    /// A list of monsters, as long as the present list: the first must spawn when `first_required`, the others spawn or are 0.
    Mobs { first_required: bool },
    /// Nine reals: three spots of x, y, z, inside the row's map.
    Spots,
}

#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub name: &'static str,
    pub sql: Sql,
    pub kind: Kind,
    pub effect: Effect,
    /// The column may be NULL (a boss group that is not random); such a field cannot be set while it is NULL.
    pub nullable: bool,
}

const fn f(name: &'static str, sql: Sql, kind: Kind, effect: Effect) -> Field {
    Field { name, sql, kind, effect, nullable: false }
}

const fn nullable(name: &'static str, sql: Sql, kind: Kind, effect: Effect) -> Field {
    Field { name, sql, kind, effect, nullable: true }
}

use Effect::{Live, NextSpawn, Restart};
use Kind::{Int, Mob};
use Sql::{Int2, Int4, Real};

const U8: Kind = Int(0, 255);
const U15: Kind = Int(0, 32_767);
const U31: Kind = Int(0, 2_147_483_647);
const X: Kind = Kind::Coord(Axis::X);
const Y: Kind = Kind::Coord(Axis::Y);
const Z: Kind = Kind::Coord(Axis::Z);

/// The editable columns of `defs.mobs`. Everything but the refused ones; HP, SP, MP and the AI are copied into a monster when it spawns,
/// so a living monster keeps them until it respawns; the rest is read from the definitions at each use (`reload`). The table is
/// `docs/plans/done/data-editing.md` section 2 ("When a defs.mobs field takes effect"); `day`, `attackspecial1`, `attackspecial2`,
/// `attackattrib1` and `attackattrib2` are read by nothing, so editing them changes nothing.
static MOB_FIELDS: &[Field] = &[
    f("exp", Int2, U15, Live),
    f("ai", Int2, U8, NextSpawn),
    f("money1", Int2, U15, Live),
    f("money2", Int2, U15, Live),
    f("questitemid", Int4, U31, Live),
    f("hp", Int4, Int(1, 2_147_483_647), NextSpawn),
    f("sp", Int2, U15, NextSpawn),
    f("mp", Int2, U15, NextSpawn),
    f("dex", Int2, U15, Live),
    f("wis", Int2, U15, Live),
    f("luc", Int2, U15, Live),
    f("day", Int2, U15, Live),
    f("size", Int2, U8, Live),
    f("attrib", Int2, U8, Live),
    f("defense", Int2, U15, Live),
    f("magic", Int2, U15, Live),
    f("resiststate1", Int2, U15, Live),
    f("resiststate2", Int2, U15, Live),
    f("resiststate3", Int2, U15, Live),
    f("resiststate4", Int2, U15, Live),
    f("resiststate5", Int2, U15, Live),
    f("resiststate6", Int2, U15, Live),
    f("resiststate7", Int2, U15, Live),
    f("resiststate8", Int2, U15, Live),
    f("resiststate9", Int2, U15, Live),
    f("resiststate10", Int2, U15, Live),
    f("resiststate11", Int2, U15, Live),
    f("resiststate12", Int2, U15, Live),
    f("resiststate13", Int2, U15, Live),
    f("resiststate14", Int2, U15, Live),
    f("resiststate15", Int2, U15, Live),
    f("resistskill1", Int2, U15, Live),
    f("resistskill2", Int2, U15, Live),
    f("resistskill3", Int2, U15, Live),
    f("resistskill4", Int2, U15, Live),
    f("resistskill5", Int2, U15, Live),
    f("resistskill6", Int2, U15, Live),
    f("normaltime", Int4, U31, Live),
    f("normalstep", Int2, U8, Live),
    f("chasetime", Int4, U31, Live),
    f("chasestep", Int2, U8, Live),
    f("chaserange", Int2, U8, Live),
    f("attacktype1", Int2, U15, Live),
    f("attacktime1", Int4, U31, Live),
    f("attackrange1", Int2, U8, Live),
    f("attack1", Int2, U15, Live),
    f("attackplus1", Int2, U15, Live),
    f("attackattrib1", Int2, U8, Live),
    f("attackspecial1", Int2, U8, Live),
    f("attackok1", Int2, U8, Live),
    f("attacktype2", Int2, U15, Live),
    f("attacktime2", Int4, U31, Live),
    f("attackrange2", Int2, U8, Live),
    f("attack2", Int2, U15, Live),
    f("attackplus2", Int2, U15, Live),
    f("attackattrib2", Int2, U8, Live),
    f("attackspecial2", Int2, U8, Live),
    f("attackok2", Int2, U8, Live),
    f("attacktype3", Int2, U15, Live),
    f("attacktime3", Int4, U31, Live),
    f("attackrange3", Int2, U8, Live),
    f("attack3", Int2, U15, Live),
    // Entry 3's `attack` is also the recovery per 5 s and its `attackplus` the obelisk-guard flag (not 0: it never walks).
    f("attackplus3", Int2, U15, Live),
    f("attackattrib3", Int2, U8, Live),
    // The respawn code (`mobs::respawn_time`); read when the monster dies.
    f("attackspecial3", Int2, U8, Live),
    f("attackok3", Int2, U8, Live),
];

static MOB_DROP_FIELDS: &[Field] = &[f("grade", Int2, Int(0, 1024), Live), f("droprate", Int4, Kind::DropRate, Live)];
static ITEM_GRADE_FIELDS: &[Field] = &[f("grade", Int2, Int(0, 1024), Live)];

static SPAWN_AREA_FIELDS: &[Field] = &[
    f("x1", Real, X, Restart),
    f("y1", Real, Y, Restart),
    f("z1", Real, Z, Restart),
    f("x2", Real, X, Restart),
    f("y2", Real, Y, Restart),
    f("z2", Real, Z, Restart),
];
static SPAWN_MOB_FIELDS: &[Field] = &[f("mob_id", Int4, Mob, Restart), f("count", Int4, Int(1, 100), Restart)];

static PORTAL_FIELDS: &[Field] = &[
    f("country", Int4, Kind::PortalCountry, Restart),
    f("level_min", Int4, U31, Restart),
    f("level_max", Int4, U31, Restart),
    f("to_map", Int4, Kind::MapId, Restart),
    f("to_x", Real, X, Restart),
    f("to_y", Real, Y, Restart),
    f("to_z", Real, Z, Restart),
];

static OBELISK_FIELDS: &[Field] = &[
    f("pos_x", Real, X, Restart),
    f("pos_y", Real, Y, Restart),
    f("pos_z", Real, Z, Restart),
    f("mob_neutral", Int4, Mob, Restart),
    f("mob_light", Int4, Mob, Restart),
    f("mob_fury", Int4, Mob, Restart),
    f("country", Int4, Int(0, 2), Restart),
];

static OBELISK_GUARD_FIELDS: &[Field] = &[
    f("pos_x", Real, X, Restart),
    f("pos_y", Real, Y, Restart),
    f("pos_z", Real, Z, Restart),
    f("radius", Real, Kind::Radius, Restart),
    f("mob_neutral", Int4, Mob, Restart),
    f("mob_light", Int4, Mob, Restart),
    f("mob_fury", Int4, Mob, Restart),
    f("count", Int4, Int(1, 100), Restart),
];

static BOSS_GROUP_FIELDS: &[Field] = &[
    f("respawn_secs", Int4, U31, Restart),
    f("pos_x", Real, X, Restart),
    f("pos_y", Real, Y, Restart),
    f("pos_z", Real, Z, Restart),
    f("mobs", Sql::Ints, Kind::Mobs { first_required: true }, Restart),
    nullable("random_min", Int4, U31, Restart),
    nullable("random_max", Int4, U31, Restart),
    nullable("random_spots", Sql::Reals, Kind::Spots, Restart),
];

static BOSS_FOLLOWER_FIELDS: &[Field] = &[
    f("pos_x", Real, X, Restart),
    f("pos_y", Real, Y, Restart),
    f("pos_z", Real, Z, Restart),
    f("radius", Real, Kind::Radius, Restart),
    f("mobs", Sql::Ints, Kind::Mobs { first_required: false }, Restart),
    f("count", Int4, Int(1, 16), Restart),
];

/// Where the map of a row's positions comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapFrom {
    /// The first part of the key (a spawn area).
    Key,
    /// The row's own `map` column (obelisks and boss groups).
    Column(&'static str),
    /// The row's `to_map` column (a portal's destination).
    ToMap,
    /// The parent row's `map` column: the target and the key part naming the parent (guards and followers).
    Parent(&'static str, usize),
    /// A parent area's map; positions are not edited (spawn monsters).
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub name: &'static str,
    pub side: Side,
    pub table: &'static str,
    /// The key's parts as the text key spells them (`map/seq`).
    pub key_names: &'static [&'static str],
    /// Each part's column, compared to the key part (a spawn monster's first two parts name its area instead).
    pub key_cols: &'static [&'static str],
    pub fields: &'static [Field],
    /// Fields that may not change, and why.
    pub refused: &'static [(&'static str, &'static str)],
    pub can_add: bool,
    pub can_remove: bool,
    pub map_from: MapFrom,
    /// The smallest sequence number a row added by the editor gets (the key's last part), so it never meets an imported one.
    pub new_seq_from: i64,
}

const MOB_REFUSED: &[(&str, &str)] = &[
    ("rowid", "the row's own number"),
    ("mobid", "the key"),
    ("mobname", "the client shows monster names from Monster.SData (a client patch, not built for monsters)"),
    ("level", "the client shows monster levels from Monster.SData (a client patch, not built for monsters)"),
];

static TARGETS: &[Target] = &[
    Target {
        name: "mob",
        side: Side::Defs,
        table: "mobs",
        key_names: &["mobid"],
        key_cols: &["mobid"],
        fields: MOB_FIELDS,
        refused: MOB_REFUSED,
        can_add: false,
        can_remove: false,
        map_from: MapFrom::None,
        new_seq_from: 0,
    },
    Target {
        name: "mob_drop",
        side: Side::Defs,
        table: "mob_items",
        key_names: &["mobid", "itemorder"],
        key_cols: &["mobid", "itemorder"],
        fields: MOB_DROP_FIELDS,
        refused: &[("rowid", "the row's own number"), ("mobid", "the key"), ("itemorder", "the key (entry 0 is never rolled)")],
        can_add: false,
        can_remove: false,
        map_from: MapFrom::None,
        new_seq_from: 0,
    },
    Target {
        name: "item_grade",
        side: Side::Defs,
        table: "items",
        key_names: &["itemid"],
        key_cols: &["itemid"],
        fields: ITEM_GRADE_FIELDS,
        refused: &[("*", "the client has its own copy of every other column of an item (docs/plans/done/client-patches.md)")],
        can_add: false,
        can_remove: false,
        map_from: MapFrom::None,
        new_seq_from: 0,
    },
    Target {
        name: "spawn_area",
        side: Side::World,
        table: "mob_areas",
        key_names: &["map", "seq"],
        key_cols: &["map", "seq"],
        fields: SPAWN_AREA_FIELDS,
        refused: &[("id", "renumbered by every import"), ("map", "the key"), ("seq", "the key")],
        can_add: true,
        can_remove: true,
        map_from: MapFrom::Key,
        new_seq_from: 100_000,
    },
    Target {
        name: "spawn_mob",
        side: Side::World,
        table: "mob_area_mobs",
        key_names: &["map", "area seq", "seq"],
        key_cols: &["seq"],
        fields: SPAWN_MOB_FIELDS,
        refused: &[("mob_area_id", "renumbered by every import"), ("seq", "the key")],
        can_add: true,
        can_remove: true,
        map_from: MapFrom::None,
        new_seq_from: 1,
    },
    Target {
        name: "portal",
        side: Side::World,
        table: "map_portals",
        key_names: &["map", "seq"],
        key_cols: &["map", "seq"],
        fields: PORTAL_FIELDS,
        refused: &[
            ("pos_x", "the client draws and detects portals from its own .wld files (docs/client/CLIENT-FACTS.md)"),
            ("pos_y", "the client draws and detects portals from its own .wld files (docs/client/CLIENT-FACTS.md)"),
            ("pos_z", "the client draws and detects portals from its own .wld files (docs/client/CLIENT-FACTS.md)"),
            ("map", "the key"),
            ("seq", "the client names a portal by its index"),
        ],
        can_add: false,
        can_remove: false,
        map_from: MapFrom::ToMap,
        new_seq_from: 0,
    },
    Target {
        name: "obelisk",
        side: Side::World,
        table: "obelisks",
        key_names: &["seq"],
        key_cols: &["seq"],
        fields: OBELISK_FIELDS,
        refused: &[
            ("seq", "the key"),
            ("map", "the owners table and the client's list key on it"),
            ("number", "the owners table and the client's list key on it"),
        ],
        can_add: false,
        can_remove: false,
        map_from: MapFrom::Column("map"),
        new_seq_from: 0,
    },
    Target {
        name: "obelisk_guard",
        side: Side::World,
        table: "obelisk_guards",
        key_names: &["obelisk seq", "seq"],
        key_cols: &["obelisk_seq", "seq"],
        fields: OBELISK_GUARD_FIELDS,
        refused: &[("obelisk_seq", "the key"), ("seq", "the key")],
        can_add: true,
        can_remove: true,
        map_from: MapFrom::Parent("obelisk", 0),
        new_seq_from: 1_000,
    },
    Target {
        name: "boss_group",
        side: Side::World,
        table: "boss_groups",
        key_names: &["seq"],
        key_cols: &["seq"],
        fields: BOSS_GROUP_FIELDS,
        refused: &[
            ("seq", "the key"),
            ("number", "the group's number in the file"),
            ("map", "the group's map"),
            ("stages", "follows the length of the monster list"),
            ("portal_map", "the portal the group opens"),
            ("portal_id", "the portal the group opens"),
        ],
        can_add: false,
        can_remove: false,
        map_from: MapFrom::Column("map"),
        new_seq_from: 0,
    },
    Target {
        name: "boss_follower",
        side: Side::World,
        table: "boss_followers",
        key_names: &["group seq", "seq"],
        key_cols: &["group_seq", "seq"],
        fields: BOSS_FOLLOWER_FIELDS,
        refused: &[("group_seq", "the key"), ("seq", "the key")],
        can_add: true,
        can_remove: true,
        map_from: MapFrom::Parent("boss_group", 0),
        new_seq_from: 1_000,
    },
];

/// Every target, parents before their children (the order rows are added in).
pub fn all() -> &'static [Target] {
    TARGETS
}

pub fn target(name: &str) -> Check<&'static Target> {
    TARGETS
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("unknown target {name:?} (targets: {})", TARGETS.iter().map(|t| t.name).collect::<Vec<_>>().join(", ")))
}

/// The child targets whose rows go with a row of `parent` when it is removed, and the target they hang from.
pub fn children_of(parent: &str) -> &'static [&'static str] {
    match parent {
        "spawn_area" => &["spawn_mob"],
        _ => &[],
    }
}

impl Target {
    pub fn field(&self, name: &str) -> Check<&'static Field> {
        let found = self.fields.iter().find(|f| f.name == name);
        found.ok_or_else(|| {
            let why = self.refused.iter().find(|(n, _)| *n == name || *n == "*").map(|(_, why)| *why);
            match why {
                Some(why) => format!("{}.{name} cannot be edited: {why}", self.name),
                None => format!(
                    "{} has no editable field {name:?} (fields: {})",
                    self.name,
                    self.fields.iter().map(|f| f.name).collect::<Vec<_>>().join(", ")
                ),
            }
        })
    }

    /// How many numbers the text key has.
    pub fn key_len(&self) -> usize {
        self.key_names.len()
    }

    /// The `WHERE` of a row; the key's parts are `$first`, `$first+1`, ... (`world` is the world schema's name).
    pub fn where_sql(&self, world: &str, first: usize) -> String {
        let at = |i: usize| format!("${}", first + i);
        match self.name {
            "spawn_mob" => {
                format!("mob_area_id = (SELECT id FROM {world}.mob_areas WHERE map = {} AND seq = {}) AND seq = {}", at(0), at(1), at(2))
            }
            _ => self.key_cols.iter().enumerate().map(|(i, c)| format!("{c} = {}", at(i))).collect::<Vec<_>>().join(" AND "),
        }
    }

    /// Parses a text key (`2/57`).
    pub fn parse_key(&self, text: &str) -> Check<Vec<i64>> {
        let parts: Result<Vec<i64>, _> = text.split('/').map(|p| p.trim().parse::<i64>()).collect();
        let parts = parts.map_err(|_| format!("{} key {text:?} must be {} as numbers", self.name, self.key_names.join("/")))?;
        if parts.len() != self.key_len() {
            return Err(format!("{} key {text:?} must be {} ({} numbers)", self.name, self.key_names.join("/"), self.key_len()));
        }
        if self.name == "mob_drop" && !(1..=9).contains(&parts[1]) {
            return Err(format!("mob_drop entry {} is not editable: entries 1 to 9 are (entry 0 is never rolled)", parts[1]));
        }
        Ok(parts)
    }

    /// The key a row added by the editor gets must lie in the range of new sequence numbers.
    pub fn check_new_key(&self, key: &[i64], parent_added: bool) -> Check<()> {
        let seq = *key.last().unwrap();
        let from = match (self.name, parent_added) {
            ("spawn_mob", false) => 1_000,
            _ => self.new_seq_from,
        };
        if seq < from {
            return Err(format!("a new {} gets a sequence number from {from} (the imported rows use the numbers below)", self.name));
        }
        Ok(())
    }

    /// The fields of a row about to be added: every editable field, exactly once.
    pub fn check_new_row(&self, value: &Value, ctx: &Ctx) -> Check<Map<String, Value>> {
        let Some(given) = value.as_object() else { return Err(format!("a new {} needs an object of its fields", self.name)) };
        for name in given.keys() {
            self.field(name)?;
        }
        let mut row = Map::new();
        for field in self.fields {
            let Some(v) = given.get(field.name) else { return Err(format!("a new {} needs the field {}", self.name, field.name)) };
            let one = Ctx { current: &Map::new(), ..*ctx };
            row.insert(field.name.to_owned(), field.check(v, &one)?);
        }
        self.check_row(&row, None)?;
        Ok(row)
    }

    /// Rules across fields; `changed` is the field being set (only the rules that involve it are checked), `None` checks them all.
    pub fn check_row(&self, row: &Map<String, Value>, changed: Option<&str>) -> Check<()> {
        let involves = |names: &[&str]| changed.is_none_or(|c| names.contains(&c));
        let num = |name: &str| row.get(name).and_then(Value::as_f64);
        match self.name {
            "spawn_area" => {
                if involves(&["x1", "x2"]) && num("x1") >= num("x2") {
                    return Err("x1 must be less than x2".into());
                }
                if involves(&["z1", "z2"]) && num("z1") >= num("z2") {
                    return Err("z1 must be less than z2".into());
                }
            }
            "portal" => {
                if involves(&["level_min", "level_max"]) {
                    let (min, max) = (num("level_min").unwrap_or(0.0), num("level_max").unwrap_or(0.0));
                    if (min, max) != (0.0, 0.0) && min > max {
                        return Err("level_min must not be above level_max (both 0 means no limit)".into());
                    }
                }
            }
            "boss_group" => {
                if involves(&["random_min", "random_max"])
                    && let (Some(min), Some(max)) = (num("random_min"), num("random_max"))
                    && min > max
                {
                    return Err("random_min must not be above random_max".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// What the rules need to know of the database.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// `world.maps`: id → side.
    pub map_sizes: BTreeMap<i64, i64>,
    /// The monsters that can spawn: id, HP and level not 0.
    pub spawnable: BTreeSet<i64>,
}

/// Where a value is being checked: the facts, the map the row's positions are inside, the row's present values, its key.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub facts: &'a Facts,
    pub map: Option<i64>,
    pub current: &'a Map<String, Value>,
    pub key: &'a [i64],
}

/// A JSON number in a canonical form: whole numbers as integers, reals as the shortest text of their 32-bit value (the columns are
/// `real`), so that equal values compare equal and the log is readable.
pub fn canon(value: &Value) -> Value {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(v) if v.fract() == 0.0 && v.abs() < 1e15 => Value::from(v as i64),
            Some(v) => real(v),
            None => value.clone(),
        },
        Value::Array(items) => Value::Array(items.iter().map(canon).collect()),
        _ => value.clone(),
    }
}

/// A real as the column holds it (`f32`), written with the fewest digits.
fn real(v: f64) -> Value {
    let narrow = v as f32;
    let text = format!("{narrow}");
    match text.parse::<f64>() {
        Ok(v) if v.fract() == 0.0 && v.abs() < 1e15 => Value::from(v as i64),
        Ok(v) => Number::from_f64(v).map_or(Value::Null, Value::Number),
        Err(_) => Value::Null,
    }
}

fn whole(v: &Value) -> Check<i64> {
    match v {
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Ok(i),
            (None, Some(f)) if f.fract() == 0.0 && f.abs() < 1e15 => Ok(f as i64),
            _ => Err(format!("{v} is not a whole number")),
        },
        _ => Err(format!("{v} is not a number")),
    }
}

fn finite(v: &Value) -> Check<f64> {
    match v.as_f64() {
        Some(f) if f.is_finite() => Ok(f),
        _ => Err(format!("{v} is not a number")),
    }
}

impl Field {
    /// Checks a new value for this field and returns it in canonical form.
    pub fn check(&self, value: &Value, ctx: &Ctx) -> Check<Value> {
        let name = self.name;
        let spawns = |id: i64| ctx.facts.spawnable.contains(&id);
        let out = match self.kind {
            Kind::Int(min, max) => {
                let v = whole(value)?;
                if !(min..=max).contains(&v) {
                    return Err(format!("{name} must be from {min} to {max} (got {v})"));
                }
                Value::from(v)
            }
            Kind::Coord(axis) => {
                let v = finite(value)?;
                if axis != Axis::Y {
                    check_inside(name, v, ctx)?;
                }
                real(v)
            }
            Kind::Radius => {
                let v = finite(value)?;
                if v <= 0.0 {
                    return Err(format!("{name} must be above 0 (got {v})"));
                }
                real(v)
            }
            Kind::Mob => {
                let v = whole(value)?;
                if !spawns(v) {
                    return Err(format!("{name}: monster {v} cannot spawn (it needs a definition with HP and a level)"));
                }
                Value::from(v)
            }
            Kind::MapId => {
                let v = whole(value)?;
                if !ctx.facts.map_sizes.contains_key(&v) {
                    return Err(format!("{name}: map {v} does not exist"));
                }
                Value::from(v)
            }
            Kind::DropRate => {
                let v = whole(value)?;
                let (entry, max) = match ctx.key.get(1) {
                    Some(&e) if (1..=6).contains(&e) => (e, 100),
                    Some(&e) => (e, 1_000_000),
                    None => (0, 1_000_000),
                };
                if !(0..=max).contains(&v) {
                    let unit = if max == 100 { "a percent" } else { "per million" };
                    return Err(format!("droprate of entry {entry} must be from 0 to {max} ({unit}; got {v})"));
                }
                Value::from(v)
            }
            Kind::PortalCountry => {
                let v = whole(value)?;
                let present = ctx.current.get("country").and_then(Value::as_i64);
                match present {
                    Some(p) if p > 1000 => {
                        if v != p {
                            return Err(format!("country {p} is an instance portal's id and stays (got {v})"));
                        }
                    }
                    _ => {
                        if !(0..=2).contains(&v) {
                            return Err(format!("country must be 0 (everyone), 1 (Light) or 2 (Fury) (got {v})"));
                        }
                    }
                }
                Value::from(v)
            }
            Kind::Mobs { first_required } => {
                let Some(items) = value.as_array() else { return Err(format!("{name} must be a list of monster ids")) };
                if let Some(present) = ctx.current.get(name).and_then(Value::as_array)
                    && present.len() != items.len()
                {
                    return Err(format!("{name} must keep its length of {} (one per stage; got {})", present.len(), items.len()));
                }
                if items.is_empty() {
                    return Err(format!("{name} must not be empty"));
                }
                let mut out = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let v = whole(item)?;
                    let none_ok = !(first_required && i == 0);
                    if !(spawns(v) || (none_ok && v == 0)) {
                        let zero = if none_ok { " (or 0 for none)" } else { "" };
                        return Err(format!("{name}[{i}]: monster {v} cannot spawn{zero}"));
                    }
                    out.push(Value::from(v));
                }
                Value::Array(out)
            }
            Kind::Spots => {
                let Some(items) = value.as_array() else { return Err(format!("{name} must be nine numbers")) };
                if items.len() != 9 {
                    return Err(format!("{name} must be nine numbers (three spots of x, y, z; got {})", items.len()));
                }
                let mut out = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let v = finite(item)?;
                    if i % 3 != 1 {
                        check_inside(&format!("{name}[{i}]"), v, ctx)?;
                    }
                    out.push(real(v));
                }
                Value::Array(out)
            }
        };
        Ok(out)
    }
}

/// `0 <= v < size` of the row's map.
fn check_inside(name: &str, v: f64, ctx: &Ctx) -> Check<()> {
    let Some(map) = ctx.map else { return Err(format!("{name}: the row's map is not known")) };
    let Some(&size) = ctx.facts.map_sizes.get(&map) else { return Err(format!("{name}: map {map} does not exist")) };
    if v < 0.0 || v >= size as f64 {
        return Err(format!("{name} must lie inside map {map} (0 up to {size}; got {v})"));
    }
    Ok(())
}

/// A key as text.
pub fn format_key(key: &[i64]) -> String {
    key.iter().map(i64::to_string).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn facts() -> Facts {
        Facts { map_sizes: [(0, 2048), (2, 1024)].into(), spawnable: [10, 11, 2133].into() }
    }

    fn check(t: &str, field: &str, value: Value, key: &[i64], map: Option<i64>, current: Value) -> Check<Value> {
        let facts = facts();
        let current = current.as_object().cloned().unwrap_or_default();
        let ctx = Ctx { facts: &facts, map, current: &current, key };
        target(t)?.field(field)?.check(&value, &ctx)
    }

    fn ok(t: &str, field: &str, value: Value, key: &[i64], map: Option<i64>) -> Value {
        check(t, field, value, key, map, json!({})).unwrap()
    }

    fn bad(t: &str, field: &str, value: Value, key: &[i64], map: Option<i64>) -> String {
        check(t, field, value, key, map, json!({})).unwrap_err()
    }

    #[test]
    fn keys_parse_and_format() {
        let area = target("spawn_area").unwrap();
        assert_eq!(area.parse_key("2/57").unwrap(), [2, 57]);
        assert_eq!(format_key(&[2, 57, 1]), "2/57/1");
        assert!(area.parse_key("2").is_err());
        assert!(area.parse_key("2/x").is_err());
        assert!(area.parse_key("2/57/1").is_err());
        assert_eq!(target("spawn_mob").unwrap().parse_key("2/57/1").unwrap(), [2, 57, 1]);
        let drop = target("mob_drop").unwrap();
        assert_eq!(drop.parse_key("2133/4").unwrap(), [2133, 4]);
        assert!(drop.parse_key("2133/0").is_err(), "entry 0 is never rolled");
        assert!(drop.parse_key("2133/10").is_err());
        assert!(target("nothing").is_err());
    }

    #[test]
    fn where_clauses_use_the_key_parts() {
        assert_eq!(target("mob_drop").unwrap().where_sql("world", 1), "mobid = $1 AND itemorder = $2");
        assert_eq!(target("obelisk_guard").unwrap().where_sql("world", 2), "obelisk_seq = $2 AND seq = $3");
        assert!(
            target("spawn_mob")
                .unwrap()
                .where_sql("w", 1)
                .starts_with("mob_area_id = (SELECT id FROM w.mob_areas WHERE map = $1 AND seq = $2) AND seq = $3")
        );
    }

    #[test]
    fn refused_fields_say_why() {
        for (t, field) in [
            ("mob", "mobname"),
            ("mob", "level"),
            ("mob", "mobid"),
            ("portal", "pos_x"),
            ("portal", "seq"),
            ("obelisk", "map"),
            ("obelisk", "number"),
            ("boss_group", "stages"),
            ("boss_group", "map"),
            ("item_grade", "buy"),
            ("spawn_area", "id"),
        ] {
            let err = target(t).unwrap().field(field).unwrap_err();
            assert!(err.contains("cannot be edited"), "{t}.{field}: {err}");
        }
        assert!(target("mob").unwrap().field("nothing").unwrap_err().contains("no editable field"));
        assert!(target("mob").unwrap().field("hp").is_ok());
    }

    #[test]
    fn mob_fields_follow_their_ranges() {
        assert_eq!(ok("mob", "hp", json!(5000), &[2133], None), 5000);
        assert_eq!(ok("mob", "hp", json!(1), &[2133], None), 1);
        assert!(bad("mob", "hp", json!(0), &[2133], None).contains("from 1 to"));
        assert!(bad("mob", "ai", json!(256), &[2133], None).contains("from 0 to 255"));
        assert_eq!(ok("mob", "ai", json!(255), &[2133], None), 255);
        assert!(bad("mob", "money1", json!(-1), &[2133], None).contains("from 0 to"));
        assert!(bad("mob", "exp", json!(1.5), &[2133], None).contains("whole number"));
        assert!(bad("mob", "exp", json!("a"), &[2133], None).contains("not a number"));
        assert_eq!(ok("mob", "exp", json!(7.0), &[2133], None), 7, "a whole real is accepted");
    }

    #[test]
    fn mob_effects_split_live_and_next_spawn() {
        let mob = target("mob").unwrap();
        assert_eq!(mob.field("hp").unwrap().effect, Effect::NextSpawn);
        assert_eq!(mob.field("ai").unwrap().effect, Effect::NextSpawn);
        assert_eq!(mob.field("mp").unwrap().effect, Effect::NextSpawn);
        assert_eq!(mob.field("attackplus3").unwrap().effect, Effect::Live);
        assert_eq!(mob.field("money1").unwrap().effect, Effect::Live);
        assert_eq!(mob.field("attackspecial3").unwrap().effect, Effect::Live);
        assert_eq!(target("spawn_area").unwrap().field("x1").unwrap().effect, Effect::Restart);
        assert_eq!(target("mob_drop").unwrap().field("grade").unwrap().effect, Effect::Live);
    }

    #[test]
    fn drop_rates_depend_on_the_entry() {
        assert_eq!(ok("mob_drop", "droprate", json!(100), &[2133, 1], None), 100);
        assert_eq!(ok("mob_drop", "droprate", json!(100), &[2133, 6], None), 100);
        assert!(bad("mob_drop", "droprate", json!(101), &[2133, 6], None).contains("0 to 100"));
        assert_eq!(ok("mob_drop", "droprate", json!(1_000_000), &[2133, 7], None), 1_000_000);
        assert!(bad("mob_drop", "droprate", json!(1_000_001), &[2133, 9], None).contains("per million"));
        assert!(bad("mob_drop", "droprate", json!(-1), &[2133, 1], None).contains("0 to 100"));
    }

    #[test]
    fn grades_run_from_0_to_1024() {
        assert_eq!(ok("mob_drop", "grade", json!(0), &[1, 1], None), 0);
        assert_eq!(ok("mob_drop", "grade", json!(1024), &[1, 1], None), 1024);
        assert!(bad("mob_drop", "grade", json!(1025), &[1, 1], None).contains("0 to 1024"));
        assert_eq!(ok("item_grade", "grade", json!(1024), &[30071], None), 1024);
        assert!(bad("item_grade", "grade", json!(-1), &[30071], None).contains("0 to 1024"));
    }

    #[test]
    fn positions_lie_inside_the_map() {
        assert_eq!(ok("spawn_area", "x1", json!(0), &[2, 57], Some(2)), 0);
        assert_eq!(ok("spawn_area", "x1", json!(1023.5), &[2, 57], Some(2)), 1023.5);
        assert!(bad("spawn_area", "x1", json!(1024), &[2, 57], Some(2)).contains("inside map 2"));
        assert!(bad("spawn_area", "z2", json!(-0.5), &[2, 57], Some(2)).contains("inside map 2"));
        assert!(bad("spawn_area", "x1", json!(5), &[9, 57], Some(9)).contains("does not exist"));
        assert!(bad("spawn_area", "x1", json!(5), &[2, 57], None).contains("not known"));
        assert_eq!(ok("spawn_area", "y1", json!(-300), &[2, 57], Some(2)), -300, "y is any real");
        assert_eq!(ok("spawn_area", "y1", json!(0.1), &[2, 57], Some(2)), 0.1);
    }

    #[test]
    fn reals_keep_to_the_column_precision() {
        assert_eq!(ok("spawn_area", "x1", json!(0.30000000000000004), &[2, 57], Some(2)), json!(0.3));
        assert_eq!(canon(&json!(2.0)), json!(2));
        assert_eq!(canon(&json!([1.0, 0.5])), json!([1, 0.5]));
    }

    #[test]
    fn monsters_must_be_able_to_spawn() {
        assert_eq!(ok("spawn_mob", "mob_id", json!(2133), &[2, 57, 1], None), 2133);
        assert!(bad("spawn_mob", "mob_id", json!(2134), &[2, 57, 1], None).contains("cannot spawn"));
        assert!(bad("spawn_mob", "mob_id", json!(0), &[2, 57, 1], None).contains("cannot spawn"));
        assert_eq!(ok("spawn_mob", "count", json!(1), &[2, 57, 1], None), 1);
        assert_eq!(ok("spawn_mob", "count", json!(100), &[2, 57, 1], None), 100);
        assert!(bad("spawn_mob", "count", json!(0), &[2, 57, 1], None).contains("1 to 100"));
        assert!(bad("spawn_mob", "count", json!(101), &[2, 57, 1], None).contains("1 to 100"));
        assert!(bad("obelisk", "mob_light", json!(5), &[1], Some(2)).contains("cannot spawn"));
    }

    #[test]
    fn portals_keep_instance_ids_and_check_destinations() {
        let p = |field: &str, v: Value, current: Value| check("portal", field, v, &[47, 6], Some(2), current);
        assert_eq!(p("country", json!(2), json!({"country": 0})).unwrap(), 2);
        assert!(p("country", json!(3), json!({"country": 0})).unwrap_err().contains("0 (everyone)"));
        assert_eq!(p("country", json!(1005), json!({"country": 1005})).unwrap(), 1005, "an instance id stays");
        assert!(p("country", json!(1), json!({"country": 1005})).unwrap_err().contains("instance portal"));
        assert!(p("country", json!(1006), json!({"country": 1005})).is_err());
        assert_eq!(p("to_map", json!(2), json!({})).unwrap(), 2);
        assert!(p("to_map", json!(99), json!({})).unwrap_err().contains("does not exist"));
        assert!(p("to_x", json!(1024), json!({})).is_err(), "inside the destination map (here 2)");
        assert_eq!(p("to_x", json!(1000), json!({})).unwrap(), 1000);
    }

    #[test]
    fn rows_check_their_corners_and_levels() {
        let area = target("spawn_area").unwrap();
        let row = |v: Value| v.as_object().unwrap().clone();
        assert!(area.check_row(&row(json!({"x1": 1, "x2": 2, "z1": 1, "z2": 2})), None).is_ok());
        assert!(area.check_row(&row(json!({"x1": 2, "x2": 2, "z1": 1, "z2": 2})), None).unwrap_err().contains("x1"));
        assert!(area.check_row(&row(json!({"x1": 1, "x2": 2, "z1": 3, "z2": 2})), None).unwrap_err().contains("z1"));
        assert!(
            area.check_row(&row(json!({"x1": 5, "x2": 2, "z1": 1, "z2": 2})), Some("z1")).is_ok(),
            "only the rules of the changed field"
        );
        let portal = target("portal").unwrap();
        assert!(portal.check_row(&row(json!({"level_min": 0, "level_max": 0})), None).is_ok());
        assert!(portal.check_row(&row(json!({"level_min": 10, "level_max": 50})), None).is_ok());
        assert!(portal.check_row(&row(json!({"level_min": 0, "level_max": 50})), None).is_ok());
        assert!(portal.check_row(&row(json!({"level_min": 60, "level_max": 50})), None).is_err());
        let boss = target("boss_group").unwrap();
        assert!(boss.check_row(&row(json!({"random_min": 5, "random_max": 4})), None).is_err());
        assert!(boss.check_row(&row(json!({"random_min": null, "random_max": null})), None).is_ok());
    }

    #[test]
    fn lists_keep_their_length() {
        let cur = json!({"mobs": [10, 11]});
        let f = |v: Value| check("boss_group", "mobs", v, &[3], Some(2), cur.clone());
        assert_eq!(f(json!([11, 10])).unwrap(), json!([11, 10]));
        assert!(f(json!([10])).unwrap_err().contains("length"));
        assert!(f(json!([0, 10])).unwrap_err().contains("cannot spawn"), "the first stage must spawn");
        assert_eq!(f(json!([10, 0])).unwrap(), json!([10, 0]), "later stages may be none");
        assert!(f(json!([10, 12])).unwrap_err().contains("[1]"));
        let g = |v: Value| check("boss_follower", "mobs", v, &[3, 1], Some(2), cur.clone());
        assert_eq!(g(json!([0, 10])).unwrap(), json!([0, 10]), "a follower may be none for a stage");
        assert!(f(json!("x")).is_err());
    }

    #[test]
    fn spots_are_nine_numbers_inside_the_map() {
        let spots = |v: Value| check("boss_group", "random_spots", v, &[3], Some(2), json!({}));
        assert!(spots(json!([1, 2, 3, 4, 5, 6, 7, 8, 9])).is_ok());
        assert!(spots(json!([1, 2, 3])).unwrap_err().contains("nine"));
        assert!(spots(json!([1, 2, 3, 4, 5, 6, 7, 8, 5000])).unwrap_err().contains("inside map 2"));
        assert!(spots(json!([1, 5000, 3, 4, 5, 6, 7, 8, 9])).is_ok(), "y is any real");
    }

    #[test]
    fn radii_and_counts() {
        assert!(bad("obelisk_guard", "radius", json!(0), &[1, 1], Some(2)).contains("above 0"));
        assert_eq!(ok("obelisk_guard", "radius", json!(0.5), &[1, 1], Some(2)), 0.5);
        assert_eq!(ok("boss_follower", "count", json!(16), &[1, 1], Some(2)), 16);
        assert!(bad("boss_follower", "count", json!(17), &[1, 1], Some(2)).contains("1 to 16"));
        assert!(bad("obelisk", "country", json!(3), &[1], Some(2)).contains("0 to 2"));
    }

    #[test]
    fn new_rows_need_a_high_sequence_number_and_every_field() {
        let area = target("spawn_area").unwrap();
        assert!(area.check_new_key(&[2, 100_000], false).is_ok());
        assert!(area.check_new_key(&[2, 99_999], false).unwrap_err().contains("100000"));
        let mob = target("spawn_mob").unwrap();
        assert!(mob.check_new_key(&[2, 57, 999], false).is_err());
        assert!(mob.check_new_key(&[2, 57, 1_000], false).is_ok());
        assert!(mob.check_new_key(&[2, 100_000, 1], true).is_ok(), "an added area counts its monsters from 1");
        let facts = facts();
        let none = Map::new();
        let ctx = Ctx { facts: &facts, map: Some(2), current: &none, key: &[2, 100_000] };
        let full = json!({"x1": 10, "y1": 0, "z1": 10, "x2": 20, "y2": 0, "z2": 20});
        assert!(area.check_new_row(&full, &ctx).is_ok());
        assert!(area.check_new_row(&json!({"x1": 10}), &ctx).unwrap_err().contains("needs the field"));
        assert!(area.check_new_row(&json!({"x1": 10, "y1": 0, "z1": 10, "x2": 5, "y2": 0, "z2": 20}), &ctx).unwrap_err().contains("x1"));
        assert!(area.check_new_row(&json!({"x1": 10, "y1": 0, "z1": 10, "x2": 20, "y2": 0, "z2": 20, "id": 1}), &ctx).is_err());
        assert!(!target("mob").unwrap().can_add && !target("portal").unwrap().can_remove);
        assert!(area.can_add && area.can_remove && mob.can_add);
    }

    #[test]
    fn every_target_is_listed_parents_first() {
        let names: Vec<_> = all().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "mob",
                "mob_drop",
                "item_grade",
                "spawn_area",
                "spawn_mob",
                "portal",
                "obelisk",
                "obelisk_guard",
                "boss_group",
                "boss_follower"
            ]
        );
        assert_eq!(children_of("spawn_area"), ["spawn_mob"]);
    }

    #[test]
    fn casts_cover_scalars_and_lists() {
        assert_eq!(Sql::Int2.cast("$1"), "(($1::jsonb) #>> '{}')::smallint");
        assert!(Sql::Ints.cast("$2").contains("jsonb_array_elements_text($2::jsonb) WITH ORDINALITY"));
        assert!(Sql::Reals.cast("$2").ends_with("::real[]"));
    }
}
