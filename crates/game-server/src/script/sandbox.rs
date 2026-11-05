//! The Lua state of one extension (`docs/plans/ext-5-lua.md` section 8): the libraries scripts get, the names removed, the
//! instruction and time hook, the memory limit. The only way out of the state is the `game` table (`api.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Value, VmState};

/// The Lua instructions between two looks at the budget (section 8.2).
pub const HOOK_EVERY: u32 = 1_000;

/// What one call into a script may use (section 8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub instructions: u32,
    pub millis: u32,
}

impl Limit {
    /// The load: every file's top level together.
    pub const LOAD: Limit = Limit { instructions: 5_000_000, millis: 500 };
    /// An event, a command, a timer, a callback.
    pub const EVENT: Limit = Limit { instructions: 1_000_000, millis: 20 };
    /// A point: asked while a player waits.
    pub const POINT: Limit = Limit { instructions: 200_000, millis: 5 };
}

/// The count of the call running now. [`Budget::begin`] starts a call afresh; the hook ends the call with an error when either
/// limit is passed, and every later look fails too (`tripped`), so a script cannot carry on by catching the error with `pcall`
/// (the wrappers installed by [`new_state`] rethrow while it is set).
pub struct Budget {
    base: Instant,
    fires: AtomicU32,
    max_fires: AtomicU32,
    instructions: AtomicU32,
    millis: AtomicU32,
    deadline_ns: AtomicU64,
    tripped: AtomicBool,
}

impl Budget {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            fires: AtomicU32::new(0),
            max_fires: AtomicU32::new(0),
            instructions: AtomicU32::new(0),
            millis: AtomicU32::new(0),
            deadline_ns: AtomicU64::new(0),
            tripped: AtomicBool::new(false),
        }
    }

    /// Starts a call under `limit`.
    pub fn begin(&self, limit: Limit) {
        self.fires.store(0, Ordering::Relaxed);
        self.max_fires.store(limit.instructions / HOOK_EVERY, Ordering::Relaxed);
        self.instructions.store(limit.instructions, Ordering::Relaxed);
        self.millis.store(limit.millis, Ordering::Relaxed);
        let deadline = self.base.elapsed() + Duration::from_millis(limit.millis as u64);
        self.deadline_ns.store(deadline.as_nanos() as u64, Ordering::Relaxed);
        self.tripped.store(false, Ordering::Relaxed);
    }

    /// Whether the running call went over a limit.
    pub fn tripped(&self) -> bool {
        self.tripped.load(Ordering::Relaxed)
    }

    fn check(&self) -> Result<(), String> {
        let over = if self.fires.fetch_add(1, Ordering::Relaxed) + 1 > self.max_fires.load(Ordering::Relaxed) {
            Some(format!("ran over {} instructions in one call", self.instructions.load(Ordering::Relaxed)))
        } else if self.base.elapsed().as_nanos() as u64 > self.deadline_ns.load(Ordering::Relaxed) {
            Some(format!("took longer than {} ms in one call", self.millis.load(Ordering::Relaxed)))
        } else {
            None
        };
        match over {
            Some(msg) => {
                self.tripped.store(true, Ordering::Relaxed);
                Err(msg)
            }
            None if self.tripped() => Err("is over its limit".to_owned()),
            None => Ok(()),
        }
    }
}

/// The names removed from the base library (section 8.1; Lua 5.1 loads `coroutine` with the base, and a coroutine would run apart
/// from the hook).
pub const REMOVED: [&str; 10] = ["dofile", "loadfile", "load", "loadstring", "require", "module", "getfenv", "setfenv", "newproxy", "coroutine"];

/// A Lua state for the extension `name`: `math`, `string`, `table`, a reduced `os` and the base library without what could reach a
/// file or build code; the memory limit; the budget hook. Nothing else is loaded (`io`, `package`, `debug`, `coroutine` never are).
pub fn new_state(memory_mb: u32) -> mlua::Result<(Lua, Arc<Budget>)> {
    let lua = Lua::new_with(StdLib::MATH | StdLib::STRING | StdLib::TABLE | StdLib::OS, LuaOptions::default())?;
    let globals = lua.globals();
    for removed in REMOVED {
        globals.set(removed, Value::Nil)?;
    }
    lua.globals().get::<mlua::Table>("string")?.set("dump", Value::Nil)?;
    // `os` keeps four functions; no clock-free way out of the state remains (`os.execute`, `getenv`, `remove`, `exit` are gone).
    let os: mlua::Table = globals.get("os")?;
    let reduced = lua.create_table()?;
    for keep in ["time", "date", "difftime", "clock"] {
        reduced.set(keep, os.get::<Value>(keep)?)?;
    }
    globals.set("os", reduced)?;
    // `collectgarbage("count")` only.
    globals.set(
        "collectgarbage",
        lua.create_function(|lua, what: Option<String>| match what.as_deref() {
            Some("count") | None => Ok(lua.used_memory() as f64 / 1024.0),
            Some(other) => Err(mlua::Error::runtime(format!("collectgarbage({other:?}) is not available to scripts"))),
        })?,
    )?;
    let budget = Arc::new(Budget::new());
    let hook = budget.clone();
    lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_EVERY), move |_, _| match hook.check() {
        Ok(()) => Ok(VmState::Continue),
        Err(msg) => Err(mlua::Error::runtime(msg)),
    })?;
    // `pcall` and `xpcall` rethrow once the budget is gone: the error the hook raises cannot be swallowed.
    let tripped = budget.clone();
    globals.set("__over_budget", lua.create_function(move |_, ()| Ok(tripped.tripped()))?)?;
    lua.load(
        r#"
        local over, opcall, oxpcall, error = __over_budget, pcall, xpcall, error
        __over_budget = nil
        local function keep(ok, ...)
            if not ok and over() then error("script is over its limit", 0) end
            return ok, ...
        end
        pcall = function(...) return keep(opcall(...)) end
        xpcall = function(f, handler) return keep(oxpcall(f, handler)) end
        "#,
    )
    .set_name("=sandbox")
    .exec()?;
    lua.set_memory_limit(memory_mb as usize * 1024 * 1024)?;
    Ok((lua, budget))
}

/// Whether `err` is Lua running out of the memory limit (section 8.3: one such error disables the extension).
pub fn is_memory_error(err: &mlua::Error) -> bool {
    match err {
        mlua::Error::MemoryError(_) => true,
        mlua::Error::CallbackError { cause, .. } => is_memory_error(cause),
        mlua::Error::WithContext { cause, .. } => is_memory_error(cause),
        _ => false,
    }
}
