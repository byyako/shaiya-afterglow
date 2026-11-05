//! The `game` table (`docs/plans/ext-5-lua.md` section 7), built once per Lua state at load. This part holds the registration
//! functions (7.2), the values every script reads (`game.api`, `game.extension`, `game.options`, `game.now`) and `game.log`; the
//! functions that read or act on the world are added where the runtime has a world to give them (steps 4 to 7).
//!
//! The Lua functions themselves live in tables of the state's registry (`handlers`, `commands`, `items`, `timers`); the Rust side
//! keeps only their names and the data around them, so the state and [`Shared`] never hold each other alive.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mlua::{Lua, MultiValue, Table, Value};

use super::act;
use super::options::{self, ResolvedOption};
use super::world::ScriptWorld;
use super::{Event, Point};

/// The longest log line, bytes, and the most lines a second (section 7.4 `game.log`).
pub const LOG_MAX: usize = 1000;
pub const LOG_PER_SECOND: u32 = 20;
/// The timers an extension may hold at once and the range of their period (section 7.2).
pub const MAX_TIMERS: usize = 64;
pub const TIMER_SECS: std::ops::RangeInclusive<u64> = 1..=86_400;

/// Registry tables holding the Lua functions.
const HANDLERS: &str = "script.handlers";
const COMMANDS: &str = "script.commands";
const ITEMS: &str = "script.items";
const TIMERS: &str = "script.timers";

/// A command word `^[a-z][a-z0-9_]{0,15}$`.
pub fn valid_command_word(word: &str) -> bool {
    let mut chars = word.chars();
    word.len() <= 16 && chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Clone, Debug)]
pub struct CommandMeta {
    /// 0 for everyone, `n` for staff of levels 1 to `n`.
    pub staff: u8,
    pub help: String,
}

#[derive(Clone, Debug)]
pub struct Timer {
    pub id: u32,
    pub secs: u64,
    pub repeat: bool,
    pub due: Instant,
}

/// What the scripts registered (data only).
#[derive(Default)]
pub struct Registrations {
    /// True while the scripts' top levels run: only then may `game.on`, `command` and `on_item` be called.
    pub open: bool,
    pub handlers: BTreeSet<&'static str>,
    pub commands: BTreeMap<String, CommandMeta>,
    pub items: BTreeSet<(u8, u8)>,
    pub timers: Vec<Timer>,
    next_timer: u32,
}

/// What an extension's load needs to know about the others (clashes) and about the game's data.
#[derive(Clone, Default)]
pub struct LoadContext {
    /// Whether the item exists in the definitions; `None` skips the test (the data editor's `check` has no definitions).
    pub item_exists: Option<Arc<dyn Fn(u8, u8) -> bool + Send + Sync>>,
    pub taken_commands: HashSet<String>,
    pub taken_items: HashSet<(u8, u8)>,
    /// What the API acts on; `None` is a world with nobody in it (a dry run, a unit test).
    pub world: Option<Arc<dyn ScriptWorld>>,
}

#[derive(Default)]
struct LogState {
    second: Option<Instant>,
    count: u32,
    dropped: u32,
    captured: Vec<String>,
}

pub struct Shared {
    pub name: String,
    pub regs: Mutex<Registrations>,
    ctx: LoadContext,
    pub world: Arc<dyn ScriptWorld>,
    pub rates: Mutex<act::Rates>,
    log: Mutex<LogState>,
    /// A dry run (the data editor's `check`) logs nothing.
    dry: bool,
    capture_log: AtomicBool,
}

impl Shared {
    pub fn new(name: &str, ctx: LoadContext, dry: bool) -> Arc<Self> {
        let regs = Registrations { open: true, ..Registrations::default() };
        let world = ctx.world.clone().unwrap_or_else(act::null_world);
        Arc::new(Self { name: name.to_owned(), regs: Mutex::new(regs), world, rates: Mutex::default(), ctx, log: Mutex::new(LogState::default()), dry, capture_log: AtomicBool::new(false) })
    }

    fn regs(&self) -> std::sync::MutexGuard<'_, Registrations> {
        self.regs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Keeps the lines `game.log` writes (the tests read them with [`Shared::captured`]).
    pub fn capture_log(&self) {
        self.capture_log.store(true, Ordering::Relaxed);
    }

    pub fn captured(&self) -> Vec<String> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).captured.clone()
    }

    /// Writes a script's log line, at most [`LOG_PER_SECOND`] a second; the lines past it are counted in the next one.
    pub fn log_line(&self, text: &str) {
        let mut st = self.log.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if st.second.is_none_or(|s| now.duration_since(s) >= Duration::from_secs(1)) {
            st.second = Some(now);
            st.count = 0;
        }
        if st.count >= LOG_PER_SECOND {
            st.dropped += 1;
            return;
        }
        st.count += 1;
        let mut lines = Vec::with_capacity(2);
        if st.dropped > 0 {
            lines.push(format!("{} log lines dropped (over {LOG_PER_SECOND} a second)", st.dropped));
            st.dropped = 0;
        }
        let mut text = text;
        if text.len() > LOG_MAX {
            let mut end = LOG_MAX;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text = &text[..end];
        }
        lines.push(text.to_owned());
        for line in lines {
            if !self.dry {
                tracing::info!(extension = %self.name, "{line}");
            }
            if self.capture_log.load(Ordering::Relaxed) {
                st.captured.push(line);
            }
        }
    }
}

/// An error that names the place in the script that made the call: `kill-streak/main.lua:12: <msg>`.
pub(crate) fn script_error(lua: &Lua, msg: impl std::fmt::Display) -> mlua::Error {
    let place = lua.inspect_stack(1, |d| {
        let src = d.source().short_src.map(|s| s.into_owned()).unwrap_or_default();
        let line = d.current_line().unwrap_or(0);
        (src, line)
    });
    match place {
        Some((src, line)) if line > 0 && !src.is_empty() => mlua::Error::runtime(format!("{src}:{line}: {msg}")),
        _ => mlua::Error::runtime(msg.to_string()),
    }
}

fn registry_table(lua: &Lua, name: &str) -> mlua::Result<Table> {
    lua.named_registry_value::<Table>(name)
}

/// Builds the `game` table (and `print`) in `lua` for the extension `shared.name`.
pub fn install(lua: &Lua, shared: &Arc<Shared>, options: &[ResolvedOption]) -> mlua::Result<()> {
    for name in [HANDLERS, COMMANDS, ITEMS, TIMERS] {
        lua.set_named_registry_value(name, lua.create_table()?)?;
    }
    let game = lua.create_table()?;
    game.set("api", super::API_VERSION)?;
    game.set("extension", shared.name.as_str())?;
    game.set("options", options::lua_table(lua, options)?)?;
    game.set("now", lua.create_function(|_, ()| Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())))?)?;

    let s = shared.clone();
    let log = lua.create_function(move |lua, args: MultiValue| {
        let mut parts = Vec::with_capacity(args.len());
        for v in args.iter() {
            parts.push(match v {
                Value::String(s) => s.to_string_lossy().to_string(),
                Value::Nil => "nil".to_owned(),
                Value::Boolean(b) => b.to_string(),
                Value::Integer(i) => i.to_string(),
                Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
                Value::Number(n) => n.to_string(),
                other => format!("<{}>", other.type_name()),
            });
        }
        let _ = lua;
        s.log_line(&parts.join("\t"));
        Ok(())
    })?;
    game.set("log", log.clone())?;
    lua.globals().set("print", log)?;

    let s = shared.clone();
    game.set(
        "on",
        lua.create_function(move |lua, (name, func): (Value, Value)| {
            if !s.regs().open {
                return Err(script_error(lua, "game.on can only be called while the scripts load"));
            }
            let Value::String(name) = name else { return Err(script_error(lua, "game.on: the first argument is the event or point name")) };
            let name = name.to_string_lossy();
            let Value::Function(func) = func else { return Err(script_error(lua, format!("game.on({name:?}): the second argument is a function"))) };
            let Some(hook) = hook_name(&name) else {
                let hint = match name.as_ref() {
                    "item_use" => " (register the items with game.on_item)".to_owned(),
                    "command" => " (register a command with game.command)".to_owned(),
                    _ => format!(" (events: {}; points: {})", Event::NAMES.join(", "), Point::NAMES.join(", ")),
                };
                return Err(script_error(lua, format!("game.on: {name:?} is not an event or point{hint}")));
            };
            let mut regs = s.regs();
            if !regs.handlers.insert(hook) {
                return Err(script_error(lua, format!("game.on({name:?}): this extension already has a handler for it")));
            }
            drop(regs);
            registry_table(lua, HANDLERS)?.raw_set(hook, func)?;
            Ok(())
        })?,
    )?;

    let s = shared.clone();
    game.set(
        "command",
        lua.create_function(move |lua, (word, func, opts): (Value, Value, Option<Table>)| {
            if !s.regs().open {
                return Err(script_error(lua, "game.command can only be called while the scripts load"));
            }
            let Value::String(word) = word else { return Err(script_error(lua, "game.command: the first argument is the command word")) };
            let word = word.to_string_lossy().to_string();
            if !valid_command_word(&word) {
                return Err(script_error(lua, format!("game.command: {word:?} is not a command word (^[a-z][a-z0-9_]{{0,15}}$)")));
            }
            let Value::Function(func) = func else { return Err(script_error(lua, format!("game.command({word:?}): the second argument is a function"))) };
            let (mut staff, mut help) = (0u8, String::new());
            if let Some(opts) = opts {
                for pair in opts.pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    match (&key, &value) {
                        (Value::String(k), Value::Integer(_) | Value::Number(_)) if k.as_bytes() == b"staff" => {
                            let n = match &value {
                                Value::Integer(i) => *i as f64,
                                Value::Number(n) => *n,
                                _ => unreachable!(),
                            };
                            if n.fract() != 0.0 || !(0.0..=10.0).contains(&n) {
                                return Err(script_error(lua, format!("game.command({word:?}): staff is 0 (everyone) or a staff level 1 to 10")));
                            }
                            staff = n as u8;
                        }
                        (Value::String(k), Value::String(v)) if k.as_bytes() == b"help" => {
                            let v = v.to_string_lossy().to_string();
                            if v.len() > 100 || !v.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
                                return Err(script_error(lua, format!("game.command({word:?}): help is printable ASCII, at most 100 bytes")));
                            }
                            help = v;
                        }
                        _ => return Err(script_error(lua, format!("game.command({word:?}): the options are staff (a number) and help (a string)"))),
                    }
                }
            }
            let mut regs = s.regs();
            if s.ctx.taken_commands.contains(&word) {
                return Err(script_error(lua, format!("game.command: another extension already registered !{word}")));
            }
            if regs.commands.contains_key(&word) {
                return Err(script_error(lua, format!("game.command: !{word} is registered twice")));
            }
            regs.commands.insert(word.clone(), CommandMeta { staff, help });
            drop(regs);
            registry_table(lua, COMMANDS)?.raw_set(word, func)?;
            Ok(())
        })?,
    )?;

    let s = shared.clone();
    game.set(
        "on_item",
        lua.create_function(move |lua, (ids, func): (Value, Value)| {
            if !s.regs().open {
                return Err(script_error(lua, "game.on_item can only be called while the scripts load"));
            }
            let Value::Table(ids) = ids else { return Err(script_error(lua, "game.on_item: the first argument is a list of \"type/type_id\"")) };
            let Value::Function(func) = func else { return Err(script_error(lua, "game.on_item: the second argument is a function")) };
            let mut parsed = Vec::new();
            for v in ids.sequence_values::<Value>() {
                let Value::String(text) = v? else { return Err(script_error(lua, "game.on_item: the ids are strings \"type/type_id\"")) };
                let text = text.to_string_lossy().to_string();
                let Some(id) = parse_item(&text) else { return Err(script_error(lua, format!("game.on_item: {text:?} is not \"type/type_id\" (two numbers 1 to 255)"))) };
                if s.ctx.item_exists.as_ref().is_some_and(|exists| !exists(id.0, id.1)) {
                    return Err(script_error(lua, format!("game.on_item: item {text} does not exist")));
                }
                parsed.push((text, id));
            }
            if parsed.is_empty() || parsed.len() > options::MAX_LIST {
                return Err(script_error(lua, format!("game.on_item: 1 to {} items", options::MAX_LIST)));
            }
            let mut regs = s.regs();
            let mut seen = HashSet::new();
            for (text, id) in &parsed {
                if !seen.insert(*id) {
                    return Err(script_error(lua, format!("game.on_item: item {text} is listed twice")));
                }
                if s.ctx.taken_items.contains(id) {
                    return Err(script_error(lua, format!("game.on_item: another extension already takes over item {text}")));
                }
                if regs.items.contains(id) {
                    return Err(script_error(lua, format!("game.on_item: item {text} is taken over twice")));
                }
            }
            for (_, id) in &parsed {
                regs.items.insert(*id);
            }
            drop(regs);
            let items = registry_table(lua, ITEMS)?;
            for (_, (t, i)) in parsed {
                items.raw_set(item_key(t, i), func.clone())?;
            }
            Ok(())
        })?,
    )?;

    for (name, repeat) in [("every", true), ("after", false)] {
        let s = shared.clone();
        game.set(
            name,
            lua.create_function(move |lua, (secs, func): (Value, Value)| {
                let secs = match secs {
                    Value::Integer(i) if i >= 1 => i as u64,
                    Value::Number(n) if n.fract() == 0.0 && n >= 1.0 => n as u64,
                    _ => return Err(script_error(lua, format!("game.{name}: the seconds are a whole number, {} to {}", TIMER_SECS.start(), TIMER_SECS.end()))),
                };
                if !TIMER_SECS.contains(&secs) {
                    return Err(script_error(lua, format!("game.{name}: the seconds are a whole number, {} to {}", TIMER_SECS.start(), TIMER_SECS.end())));
                }
                let Value::Function(func) = func else { return Err(script_error(lua, format!("game.{name}: the second argument is a function"))) };
                let mut regs = s.regs();
                if regs.timers.len() >= MAX_TIMERS {
                    return Err(script_error(lua, format!("game.{name}: at most {MAX_TIMERS} timers")));
                }
                regs.next_timer += 1;
                let id = regs.next_timer;
                regs.timers.push(Timer { id, secs, repeat, due: Instant::now() + Duration::from_secs(secs) });
                drop(regs);
                registry_table(lua, TIMERS)?.raw_set(id, func)?;
                Ok(id)
            })?,
        )?;
    }
    let s = shared.clone();
    game.set(
        "cancel",
        lua.create_function(move |lua, id: u32| {
            let mut regs = s.regs();
            let before = regs.timers.len();
            regs.timers.retain(|t| t.id != id);
            let removed = regs.timers.len() != before;
            drop(regs);
            if removed {
                registry_table(lua, TIMERS)?.raw_set(id, Value::Nil)?;
            }
            Ok(removed)
        })?,
    )?;
    act::install(lua, shared, &game)?;
    lua.globals().set("game", game)?;
    Ok(())
}

/// The static name of the event or point `name`, if it is one `game.on` takes.
fn hook_name(name: &str) -> Option<&'static str> {
    Event::from_name(name).map(Event::name).or_else(|| Point::from_name(name).map(Point::name))
}

/// `"type/type_id"`.
pub fn parse_item(text: &str) -> Option<(u8, u8)> {
    let (t, i) = text.split_once('/')?;
    let num = |s: &str| (!s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u8>().ok()).flatten().filter(|n| *n >= 1);
    Some((num(t)?, num(i)?))
}

fn item_key(item_type: u8, type_id: u8) -> String {
    format!("{item_type}/{type_id}")
}

/// Fetches the Lua function a hook, command, item or timer registered.
pub fn handler_fn(lua: &Lua, hook: &str) -> Option<mlua::Function> {
    registry_table(lua, HANDLERS).ok()?.raw_get::<Option<mlua::Function>>(hook).ok().flatten()
}

pub fn command_fn(lua: &Lua, word: &str) -> Option<mlua::Function> {
    registry_table(lua, COMMANDS).ok()?.raw_get::<Option<mlua::Function>>(word).ok().flatten()
}

pub fn item_fn(lua: &Lua, item_type: u8, type_id: u8) -> Option<mlua::Function> {
    registry_table(lua, ITEMS).ok()?.raw_get::<Option<mlua::Function>>(item_key(item_type, type_id)).ok().flatten()
}

pub fn timer_fn(lua: &Lua, id: u32) -> Option<mlua::Function> {
    registry_table(lua, TIMERS).ok()?.raw_get::<Option<mlua::Function>>(id).ok().flatten()
}

pub fn drop_timer_fn(lua: &Lua, id: u32) {
    if let Ok(t) = registry_table(lua, TIMERS) {
        let _ = t.raw_set(id, Value::Nil);
    }
}
