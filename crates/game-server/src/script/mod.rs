//! Lua scripts of extensions (`docs/plans/ext-5-lua.md`): the sandboxed state of each extension (`sandbox.rs`), the `game` table
//! (`api.rs`), the options (`options.rs`), and here the extension as the server holds it ([`ExtScript`]: its state, its
//! registrations, its faults), the set of them in force ([`ScriptSet`]) and the dry run `ext check` makes ([`check`]).
//!
//! `mob_lua.rs` and `zone/lua.rs` (the original's monster scripts) are apart from this module and share nothing with it
//! (section 8.4).

pub mod act;
pub mod api;
pub mod event;
pub mod options;
pub mod runner;
pub mod sandbox;
pub mod sources;
pub mod store;
pub mod world;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use mlua::{FromLuaMulti, IntoLuaMulti, Lua};

pub use api::{CommandMeta, LoadContext};
pub use options::{OptType, OptionSpec, ResolvedOption, check_option};
pub use sandbox::Limit;

use api::Shared;

/// The Lua API version this server offers (section 3.1).
pub const API_VERSION: u32 = 1;
/// The default memory limit of one extension's state, MB (the `script_memory_mb` setting, section 8.2).
pub const DEFAULT_MEMORY_MB: u32 = 16;
/// Faults within the window that disable an extension (section 8.3).
pub const FAULTS_TO_DISABLE: usize = 5;
pub const FAULT_WINDOW: Duration = Duration::from_secs(60);
/// At most one warning line per extension in this time; the others are counted.
const WARN_EVERY: Duration = Duration::from_secs(10);

/// The notification events of section 5 that `game.on` takes (`command` has `game.command`, a timer has `game.every`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Login,
    Logout,
    MapEnter,
    MapLeave,
    Kill,
    BossKill,
    PvpKill,
    Death,
}

impl Event {
    pub const ALL: [Event; 8] = [Event::Login, Event::Logout, Event::MapEnter, Event::MapLeave, Event::Kill, Event::BossKill, Event::PvpKill, Event::Death];
    pub const NAMES: [&'static str; 8] = ["login", "logout", "map_enter", "map_leave", "kill", "boss_kill", "pvp_kill", "death"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Event> {
        Self::ALL.into_iter().find(|e| e.name() == name)
    }
}

/// The points of section 6 that `game.on` takes (`item_use` has `game.on_item`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    Trade,
    PersonalShop,
    TradeChat,
    Chat,
    Exp,
    PvpKills,
    PvpDeath,
    Enchant,
}

impl Point {
    pub const ALL: [Point; 8] = [Point::Trade, Point::PersonalShop, Point::TradeChat, Point::Chat, Point::Exp, Point::PvpKills, Point::PvpDeath, Point::Enchant];
    pub const NAMES: [&'static str; 8] = ["trade", "personal_shop", "trade_chat", "chat", "exp", "pvp_kills", "pvp_death", "enchant"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Point> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }

    /// What a call at this point may use (section 8.2).
    pub fn limit(self) -> Limit {
        Limit::POINT
    }
}

/// What an extension registered, as `ext check` and `ext show` print it (section 3.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub events: Vec<String>,
    pub points: Vec<String>,
    /// `(word, staff)`
    pub commands: Vec<(String, u8)>,
    /// `"type/type_id"`
    pub items: Vec<String>,
    pub timers: usize,
}

impl Summary {
    /// Handlers: events, points, commands and item takeovers.
    pub fn handlers(&self) -> usize {
        self.events.len() + self.points.len() + self.commands.len() + usize::from(!self.items.is_empty())
    }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let list = |v: &[String]| if v.is_empty() { "none".to_owned() } else { v.join(", ") };
        writeln!(f, "events: {}", list(&self.events))?;
        writeln!(f, "points: {}", list(&self.points))?;
        let commands: Vec<String> = self.commands.iter().map(|(w, s)| if *s == 0 { format!("!{w}") } else { format!("!{w} (staff {s})") }).collect();
        writeln!(f, "commands: {}", list(&commands))?;
        writeln!(f, "item takeovers: {}", list(&self.items))?;
        write!(f, "timers: {}", self.timers)
    }
}

/// One extension's scripts to load: its files in order and its options with the values in force.
#[derive(Clone, Debug)]
pub struct ExtInput {
    pub name: String,
    /// `(path, source)` in the order of the manifest's `scripts`.
    pub scripts: Vec<(String, String)>,
    pub options: Vec<ResolvedOption>,
}

/// A failed call: the message names the extension and the handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError(pub String);

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScriptError {}

#[derive(Default)]
struct Faults {
    recent: VecDeque<Instant>,
    last_warn: Option<Instant>,
    suppressed: u32,
}

/// One extension with scripts, as the server holds it: its Lua state (a mutex, section 4.3), what the scripts registered, its
/// fault count (section 8.3). A disabled extension answers as if it had no handlers until the next `reload`.
pub struct ExtScript {
    pub name: String,
    shared: Arc<Shared>,
    lua: Mutex<Lua>,
    budget: Arc<sandbox::Budget>,
    faults: Mutex<Faults>,
    disabled: Mutex<Option<String>>,
    is_disabled: AtomicBool,
    fault_total: AtomicU64,
    /// Told when the extension is disabled: the extension's name and the reason (step 4 sets the staff whisper here).
    on_disable: Mutex<Option<Box<dyn Fn(&str, &str) + Send + Sync>>>,
}

impl ExtScript {
    /// Loads an extension's files, in order, into a new state within the load limit, then closes the registrations (section 9.1).
    /// `Err` names the file and line (a compile error, a top-level error, a bad registration, a clash, a limit).
    pub fn load(input: &ExtInput, ctx: &LoadContext, memory_mb: u32, dry: bool) -> Result<ExtScript> {
        let shared = Shared::new(&input.name, ctx.clone(), dry);
        let (lua, budget) = sandbox::new_state(memory_mb).map_err(|e| anyhow!("extension {}: {}", input.name, one_line(&e)))?;
        api::install(&lua, &shared, &input.options).map_err(|e| anyhow!("extension {}: {}", input.name, one_line(&e)))?;
        budget.begin(Limit::LOAD);
        for (path, source) in &input.scripts {
            let place = format!("{}/{}", input.name, path);
            let result = lua.load(source.as_bytes()).set_name(format!("@{place}")).set_mode(mlua::chunk::ChunkMode::Text).exec();
            if let Err(e) = result {
                // A compile or runtime error names its place; a limit or a binary chunk does not, so the file is added.
                let msg = one_line(&e);
                let msg = if msg.contains(&place) { msg } else { format!("{place}: {msg}") };
                bail!("extension {}: {msg}", input.name);
            }
        }
        shared.regs.lock().unwrap_or_else(|e| e.into_inner()).open = false;
        Ok(ExtScript {
            name: input.name.clone(),
            shared,
            lua: Mutex::new(lua),
            budget,
            faults: Mutex::new(Faults::default()),
            disabled: Mutex::new(None),
            is_disabled: AtomicBool::new(false),
            fault_total: AtomicU64::new(0),
            on_disable: Mutex::new(None),
        })
    }

    /// What the scripts registered.
    pub fn summary(&self) -> Summary {
        let regs = self.shared.regs.lock().unwrap_or_else(|e| e.into_inner());
        let mut events: Vec<String> = Vec::new();
        let mut points: Vec<String> = Vec::new();
        for h in &regs.handlers {
            if Event::from_name(h).is_some() {
                events.push((*h).to_owned());
            } else {
                points.push((*h).to_owned());
            }
        }
        Summary {
            events,
            points,
            commands: regs.commands.iter().map(|(w, m)| (w.clone(), m.staff)).collect(),
            items: regs.items.iter().map(|(t, i)| format!("{t}/{i}")).collect(),
            timers: regs.timers.len(),
        }
    }

    /// Whether the extension registered a handler for `hook` (an event or point name).
    pub fn handles(&self, hook: &str) -> bool {
        self.shared.regs.lock().unwrap_or_else(|e| e.into_inner()).handlers.contains(hook)
    }

    pub fn commands(&self) -> Vec<(String, CommandMeta)> {
        self.shared.regs.lock().unwrap_or_else(|e| e.into_inner()).commands.iter().map(|(w, m)| (w.clone(), m.clone())).collect()
    }

    pub fn items(&self) -> Vec<(u8, u8)> {
        self.shared.regs.lock().unwrap_or_else(|e| e.into_inner()).items.iter().copied().collect()
    }

    /// Whether the timer `id` is still set (a one-shot one is gone once it came due).
    pub fn has_timer(&self, id: u32) -> bool {
        self.shared.regs.lock().unwrap_or_else(|e| e.into_inner()).timers.iter().any(|t| t.id == id)
    }

    /// Monsters this extension spawned that are still alive (`game.spawn`, step 7).
    pub fn spawn_count(&self) -> usize {
        self.shared.world.spawn_count(&self.name)
    }

    pub fn timer_count(&self) -> usize {
        self.shared.regs.lock().unwrap_or_else(|e| e.into_inner()).timers.len()
    }

    /// The timers due at `now`: each repeating one is set to its next time (once, however late it is), a one-shot one is removed.
    pub fn due_timers(&self, now: Instant) -> Vec<u32> {
        let mut regs = self.shared.regs.lock().unwrap_or_else(|e| e.into_inner());
        let mut due = Vec::new();
        for t in regs.timers.iter_mut() {
            if t.due <= now {
                due.push(t.id);
                t.due = now + Duration::from_secs(t.secs);
            }
        }
        regs.timers.retain(|t| t.repeat || !due.contains(&t.id));
        due
    }

    /// The soonest a timer is due.
    pub fn next_timer(&self) -> Option<Instant> {
        self.shared.regs.lock().unwrap_or_else(|e| e.into_inner()).timers.iter().map(|t| t.due).min()
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn is_disabled(&self) -> bool {
        self.is_disabled.load(Ordering::Relaxed)
    }

    /// Why the extension is disabled, if it is.
    pub fn disabled_reason(&self) -> Option<String> {
        self.disabled.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Faults counted since the load (all of them, not only the recent ones).
    pub fn fault_count(&self) -> u64 {
        self.fault_total.load(Ordering::Relaxed)
    }

    /// Sets what is told when the extension is disabled.
    pub fn set_on_disable(&self, f: Box<dyn Fn(&str, &str) + Send + Sync>) {
        *self.on_disable.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }

    /// Runs `f` in the state under `limit`, labelled `label` for the error line: the mutex of the state is taken, the budget
    /// started afresh. An error is a fault (section 8.3): counted, warned, the extension disabled past the limit. A disabled
    /// extension is not run (`Err` with its reason).
    pub fn run<R>(&self, label: &str, limit: Limit, f: impl FnOnce(&Lua) -> mlua::Result<R>) -> Result<R, ScriptError> {
        if self.is_disabled() {
            return Err(ScriptError(format!("extension {} is disabled", self.name)));
        }
        let lua = self.lua.lock().unwrap_or_else(|e| e.into_inner());
        self.budget.begin(limit);
        let result = f(&lua);
        drop(lua);
        result.map_err(|e| self.fault(label, &e))
    }

    /// Calls the handler registered for `hook` with `args`. `Ok(None)`: no handler, or the extension is disabled. `Err`: the
    /// call failed or its answer was not an `R` (a fault).
    pub fn call<A: IntoLuaMulti, R: FromLuaMulti>(&self, hook: &str, limit: Limit, args: A) -> Result<Option<R>, ScriptError> {
        if self.is_disabled() || !self.handles(hook) {
            return Ok(None);
        }
        self.run(hook, limit, |lua| match api::handler_fn(lua, hook) {
            Some(f) => f.call::<R>(args).map(Some),
            None => Ok(None),
        })
    }

    /// As [`ExtScript::call`], with the arguments built inside the state (a table needs the state to be made in).
    pub fn call_built<R: FromLuaMulti>(&self, hook: &str, limit: Limit, build: impl FnOnce(&Lua) -> mlua::Result<mlua::MultiValue>) -> Result<Option<R>, ScriptError> {
        if self.is_disabled() || !self.handles(hook) {
            return Ok(None);
        }
        self.run(hook, limit, |lua| match api::handler_fn(lua, hook) {
            Some(f) => f.call::<R>(build(lua)?).map(Some),
            None => Ok(None),
        })
    }

    /// Counts a fault at `label`; the message is the extension, the handler and the error's first line.
    pub fn fault(&self, label: &str, err: &mlua::Error) -> ScriptError {
        self.fault_at(Instant::now(), label, err)
    }

    pub(crate) fn fault_at(&self, now: Instant, label: &str, err: &mlua::Error) -> ScriptError {
        let detail = format!("{label}: {}", one_line(err));
        let msg = format!("extension {}: {detail}", self.name);
        self.fault_total.fetch_add(1, Ordering::Relaxed);
        let memory = sandbox::is_memory_error(err);
        let mut faults = self.faults.lock().unwrap_or_else(|e| e.into_inner());
        faults.recent.push_back(now);
        while faults.recent.front().is_some_and(|t| now.saturating_duration_since(*t) > FAULT_WINDOW) {
            faults.recent.pop_front();
        }
        if faults.last_warn.is_none_or(|t| now.saturating_duration_since(t) >= WARN_EVERY) {
            let suppressed = std::mem::take(&mut faults.suppressed);
            faults.last_warn = Some(now);
            tracing::warn!(extension = %self.name, handler = label, suppressed, "script error: {msg}");
        } else {
            faults.suppressed += 1;
        }
        let too_many = faults.recent.len() >= FAULTS_TO_DISABLE;
        drop(faults);
        if memory || too_many {
            let reason = if memory { format!("a memory error: {detail}") } else { format!("{FAULTS_TO_DISABLE} errors in {} s: {detail}", FAULT_WINDOW.as_secs()) };
            self.disable(reason);
        }
        ScriptError(msg)
    }

    /// Disables the extension's scripts until the next reload (section 8.3).
    pub fn disable(&self, reason: String) {
        {
            let mut d = self.disabled.lock().unwrap_or_else(|e| e.into_inner());
            if d.is_some() {
                return;
            }
            *d = Some(reason.clone());
        }
        self.is_disabled.store(true, Ordering::Relaxed);
        tracing::error!(extension = %self.name, "scripts disabled: {reason}");
        if let Some(f) = self.on_disable.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(&self.name, &reason);
        }
    }
}

/// The first line of a Lua error: the message without the traceback.
pub(crate) fn one_line(err: &mlua::Error) -> String {
    let mut e = err;
    loop {
        match e {
            mlua::Error::CallbackError { cause, .. } => e = cause,
            mlua::Error::WithContext { cause, .. } => e = cause,
            _ => break,
        }
    }
    let text = match e {
        mlua::Error::RuntimeError(m) | mlua::Error::MemoryError(m) => m.clone(),
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        other => other.to_string(),
    };
    text.lines().next().unwrap_or("").trim().to_owned()
}

/// The extensions' scripts in force, in install order (section 4.1). Built by `start` and `reload`; the server swaps it whole.
#[derive(Default)]
pub struct ScriptSet {
    exts: Vec<Arc<ExtScript>>,
    /// Extensions whose scripts did not load: `(name, reason)`.
    failed: Vec<(String, String)>,
    events: [Vec<Arc<ExtScript>>; 8],
    points: [Vec<Arc<ExtScript>>; 8],
    commands: HashMap<String, (Arc<ExtScript>, CommandMeta)>,
    items: HashMap<(u8, u8), Arc<ExtScript>>,
}

impl ScriptSet {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads every extension in order (section 9.1). A clash with an earlier extension (a command word, an item) is the later one's
    /// load error. `base` carries what `on_item` ids are checked against and the world the API acts on.
    pub fn load(inputs: &[ExtInput], base: LoadContext, memory_mb: u32) -> ScriptSet {
        let mut set = ScriptSet::default();
        let mut ctx = base;
        for input in inputs {
            match ExtScript::load(input, &ctx, memory_mb, false) {
                Ok(ext) => {
                    // A disabled extension is told to the staff who may type console commands (section 8.3).
                    if let Some(world) = ctx.world.clone() {
                        ext.set_on_disable(Box::new(move |name, reason| world.tell_staff(&format!("extension {name}: scripts disabled after {reason}"))));
                    }
                    let ext = Arc::new(ext);
                    for (word, meta) in ext.commands() {
                        ctx.taken_commands.insert(word.clone());
                        set.commands.insert(word, (ext.clone(), meta));
                    }
                    for id in ext.items() {
                        ctx.taken_items.insert(id);
                        set.items.insert(id, ext.clone());
                    }
                    for (i, name) in Event::NAMES.iter().enumerate() {
                        if ext.handles(name) {
                            set.events[i].push(ext.clone());
                        }
                    }
                    for (i, name) in Point::NAMES.iter().enumerate() {
                        if ext.handles(name) {
                            set.points[i].push(ext.clone());
                        }
                    }
                    set.exts.push(ext);
                }
                Err(e) => set.failed.push((input.name.clone(), format!("{e:#}"))),
            }
        }
        set
    }

    pub fn is_empty(&self) -> bool {
        self.exts.is_empty() && self.failed.is_empty()
    }

    pub fn extensions(&self) -> &[Arc<ExtScript>] {
        &self.exts
    }

    pub fn failed(&self) -> &[(String, String)] {
        &self.failed
    }

    /// The extensions with a handler for the event, in install order (empty without scripts: one branch, section 10).
    pub fn for_event(&self, event: Event) -> &[Arc<ExtScript>] {
        &self.events[event as usize]
    }

    pub fn for_point(&self, point: Point) -> &[Arc<ExtScript>] {
        &self.points[point as usize]
    }

    pub fn command(&self, word: &str) -> Option<&(Arc<ExtScript>, CommandMeta)> {
        self.commands.get(word)
    }

    pub fn has_commands(&self) -> bool {
        !self.commands.is_empty()
    }

    pub fn item_owner(&self, item_type: u8, type_id: u8) -> Option<&Arc<ExtScript>> {
        if self.items.is_empty() { None } else { self.items.get(&(item_type, type_id)) }
    }
}

/// The dry run of `ext check` (section 3.3): compiles every file, runs the load in a state whose API does nothing, returns what the
/// scripts register. `item_exists` is `None` when the definitions are not at hand (an `on_item` id is then not verified).
pub fn check(input: &ExtInput, item_exists: Option<Arc<dyn Fn(u8, u8) -> bool + Send + Sync>>) -> Result<Summary> {
    let ctx = LoadContext { item_exists, ..LoadContext::default() };
    let ext = ExtScript::load(input, &ctx, DEFAULT_MEMORY_MB, true)?;
    Ok(ext.summary())
}
