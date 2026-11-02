//! The Lua monsters (`AI` 100 and up, `docs/original/instances.md` sections 9 to 15): their scripts, their stack state machine
//! and what each one keeps. The zone runs them (`zone/lua.rs`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The states (`StackStateMachine<CMob>`): idle (`WhileIdle`), combat (`WhileCombat`), dead, going home (`WhileReturnHome`), moving
/// (`WhileMove`). The top of the stack is the monster's status (`CMob::GetStatus`).
pub const IDLE: u8 = 0;
pub const COMBAT: u8 = 1;
pub const DEAD: u8 = 2;
pub const HOME: u8 = 3;
pub const MOVING: u8 = 4;

/// The events (section 10).
pub const EV_ATTACKABLE: u8 = 0;
pub const EV_ATTACKED: u8 = 1;
pub const EV_NORMAL_RESET: u8 = 2;
pub const EV_DEATH: u8 = 3;
pub const EV_RETURN_HOME: u8 = 4;
pub const EV_MOVE: u8 = 5;
pub const EV_MOVE_END: u8 = 6;
pub const EV_MOVE_AGAIN: u8 = 7;

/// The move types of `LuaFixedMove` the code tells apart: 3 is a walk home after a chase given up (section 13).
pub const MOVE_HOME: u8 = 3;

/// What a transition does to the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Push(u8),
    Pop,
    Replace(u8),
}

/// The callback a transition runs before its change (`CMob::On…`); `MoveEnd` only clears the move type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callback {
    None,
    Attackable,
    Attacked,
    NormalReset,
    Death,
    ReturnHome,
    MoveEnd,
}

/// The transition table `CMob::InitLuaFSM` sets up (section 10): for the top state and an event, the callback and the change.
pub fn transition(state: u8, event: u8) -> Option<(Callback, Change)> {
    use Callback as C;
    use Change::*;
    Some(match (state, event) {
        (IDLE, EV_ATTACKABLE) => (C::Attackable, Push(COMBAT)),
        (IDLE, EV_ATTACKED) => (C::Attacked, Push(COMBAT)),
        (IDLE, EV_NORMAL_RESET) => (C::NormalReset, Replace(IDLE)),
        (IDLE, EV_RETURN_HOME) => (C::ReturnHome, Push(HOME)),
        (IDLE, EV_MOVE | EV_MOVE_AGAIN) => (C::None, Push(MOVING)),
        (COMBAT, EV_ATTACKED) => (C::Attacked, Replace(COMBAT)),
        (COMBAT, EV_NORMAL_RESET) => (C::NormalReset, Pop),
        (COMBAT, EV_RETURN_HOME) => (C::ReturnHome, Replace(HOME)),
        (COMBAT, EV_MOVE) => (C::None, Replace(MOVING)),
        (COMBAT, EV_MOVE_AGAIN) => (C::None, Pop),
        (HOME, EV_NORMAL_RESET) => (C::NormalReset, Pop),
        (HOME, EV_MOVE) => (C::None, Pop),
        (MOVING, EV_ATTACKABLE) => (C::Attackable, Push(COMBAT)),
        (MOVING, EV_ATTACKED) => (C::Attacked, Push(COMBAT)),
        (MOVING, EV_NORMAL_RESET) => (C::NormalReset, Pop),
        (MOVING, EV_MOVE_END) => (C::MoveEnd, Pop),
        (MOVING, EV_MOVE | EV_MOVE_AGAIN) => (C::None, Replace(MOVING)),
        (IDLE | COMBAT | HOME | MOVING, EV_DEATH) => (C::Death, Replace(DEAD)),
        _ => return None,
    })
}

/// The stack itself; empty, it is idle again (`SetInitialState(0)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack(Vec<u8>);

impl Default for Stack {
    fn default() -> Self {
        Self(vec![IDLE])
    }
}

impl Stack {
    pub fn top(&self) -> u8 {
        self.0.last().copied().unwrap_or(IDLE)
    }

    /// Applies a transition's change; a pop that empties the stack starts it over in state 0.
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Push(state) => self.0.push(state),
            Change::Pop => {
                self.0.pop();
            }
            Change::Replace(state) => {
                self.0.pop();
                self.0.push(state);
            }
        }
        if self.0.is_empty() {
            self.0.push(IDLE);
        }
    }
}

/// `Data/Lua/<ai>.lua` by `AI` number (`world.mob_lua`).
#[derive(Clone, Debug, Default)]
pub struct Scripts(pub HashMap<u8, Vec<u8>>);

/// What a Lua monster keeps besides its `Mob` (section 9's fields).
#[derive(Clone, Copy, Debug)]
pub struct LuaFields {
    /// In battle (`m_bIsBattle`, `+0x128`).
    pub battle: bool,
    /// Attacked count (`+0x28c`): damage taken while alive.
    pub attacked: u32,
    /// `LuaSetOnlyLuaAttack` (`+0x290`), `LuaSetHoldPosion` (`+0x294`).
    pub only_lua_attack: bool,
    pub hold: bool,
    /// The move of `LuaFixedMove` or a chase given up (`+0x2a0`, `+0x2a4`; y is 0).
    pub move_type: u8,
    pub move_to: [f32; 3],
    /// Which transition hook is running (`+0x2b4`): `LuaFixedMove` is ignored inside one.
    pub context: u8,
    /// No target since: give up at this time (`+0x280`, `SetReTarget`).
    pub give_up_at: Option<Instant>,
    /// The retargeting term (`+0x288`) and the next retargeting time (`+0xcc4`).
    pub retarget_term: Duration,
    pub next_retarget: Instant,
    /// The next step of a move or of going home (`+0xcb8`).
    pub next_step: Instant,
}

impl LuaFields {
    pub fn new(now: Instant) -> Self {
        Self {
            battle: false,
            attacked: 0,
            only_lua_attack: false,
            hold: false,
            move_type: 0,
            move_to: [0.0; 3],
            context: 0,
            give_up_at: None,
            retarget_term: Duration::from_millis(5000),
            next_retarget: now,
            next_step: now,
        }
    }
}

/// A Lua monster's script state and stack machine. `lua` is `None` when its script did not load: it is then an ordinary monster.
pub struct LuaMob {
    pub lua: Option<mlua::Lua>,
    pub stack: Stack,
    pub fields: LuaFields,
}

impl std::fmt::Debug for LuaMob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaMob").field("loaded", &self.lua.is_some()).field("stack", &self.stack).field("fields", &self.fields).finish()
    }
}

/// What the scripts' `LuaMob(CMob)` gives: an object whose every `Mob:LuaX(…)` goes to the server's `__lua_mob(name, …)`, which
/// exists only while the server runs one of the monster's hooks (the original's `LuaDispatcher<LuaMob>`).
pub const PRELUDE: &str = r#"
function LuaMob(cmob)
    return setmetatable({}, { __index = function(_, name)
        return function(_, ...) return __lua_mob(name, ...) end
    end })
end
"#;

/// The Lua instructions between two looks at the budget.
const HOOK_EVERY: u32 = 10_000;
/// How many instructions one call into a script (a hook, or the script's first run) may take: a script that loops for ever would
/// otherwise hang the monster tick (`docs/server/audit.md` R28). The shipped scripts use a few hundred.
pub const INSTRUCTION_LIMIT: u32 = 1_000_000;

/// How many times the instruction hook has fired in the call running now; [`reset_budget`] starts a call afresh.
struct Budget(std::sync::Arc<std::sync::atomic::AtomicU32>);

/// A new Lua state for a monster (section 9): the base (without `dofile` and `loadfile`) and math libraries, an `os` with `time` alone (the scripts seed `math.random`
/// with it), `CMob` (the monster's id) and `LuaMob`, then the script run once. A call into it may run [`INSTRUCTION_LIMIT`] Lua
/// instructions; past that it ends with an error naming the script. `Err` with the reason when it does not load.
pub fn load(source: &[u8], name: &str, mob_id: u32) -> mlua::Result<mlua::Lua> {
    use std::sync::atomic::{AtomicU32, Ordering};
    let lua = mlua::Lua::new_with(mlua::StdLib::MATH, mlua::LuaOptions::default())?;
    let os = lua.create_table()?;
    os.set(
        "time",
        lua.create_function(|_, _: mlua::MultiValue| {
            Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()))
        })?,
    )?;
    lua.globals().set("os", os)?;
    // The base library is always there; its two file readers are not for scripts that come out of a database.
    lua.globals().set("dofile", mlua::Value::Nil)?;
    lua.globals().set("loadfile", mlua::Value::Nil)?;
    let fired = std::sync::Arc::new(AtomicU32::new(0));
    lua.set_app_data(Budget(fired.clone()));
    let script = name.to_owned();
    lua.set_hook(mlua::HookTriggers::new().every_nth_instruction(HOOK_EVERY), move |_, _| {
        if fired.fetch_add(1, Ordering::Relaxed) + 1 > INSTRUCTION_LIMIT / HOOK_EVERY {
            tracing::error!(script, limit = INSTRUCTION_LIMIT, "Lua script ran over its instruction limit in one call");
            return Err(mlua::Error::runtime(format!("{script} ran over {INSTRUCTION_LIMIT} instructions in one call")));
        }
        Ok(mlua::VmState::Continue)
    })?;
    lua.globals().set("CMob", mob_id)?;
    lua.load(PRELUDE).set_name("LuaMob").exec()?;
    lua.load(source).set_name(name).exec()?;
    Ok(lua)
}

/// Starts a new call's instruction count: the server calls this before each hook it runs in the state.
pub fn reset_budget(lua: &mlua::Lua) {
    if let Some(budget) = lua.app_data_ref::<Budget>() {
        budget.0.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stack_follows_the_original_table() {
        let mut s = Stack::default();
        let step = |s: &mut Stack, event| {
            let (callback, change) = transition(s.top(), event).expect("a transition");
            s.apply(change);
            callback
        };
        assert_eq!(step(&mut s, EV_ATTACKABLE), Callback::Attackable);
        assert_eq!(s.top(), COMBAT);
        // A fixed move from combat replaces it; its end pops back to idle.
        step(&mut s, EV_MOVE);
        assert_eq!((s.top(), s.0.len()), (MOVING, 2));
        assert_eq!(step(&mut s, EV_MOVE_END), Callback::MoveEnd);
        assert_eq!(s, Stack(vec![IDLE]));
        // Attacked while moving pushes combat on the move; a reset pops back to the move.
        step(&mut s, EV_MOVE);
        step(&mut s, EV_ATTACKED);
        assert_eq!(s, Stack(vec![IDLE, MOVING, COMBAT]));
        assert_eq!(step(&mut s, EV_NORMAL_RESET), Callback::NormalReset);
        assert_eq!(s.top(), MOVING);
        assert_eq!(step(&mut s, EV_DEATH), Callback::Death);
        assert_eq!(s.top(), DEAD);
        assert_eq!(transition(DEAD, EV_ATTACKED), None, "the dead take no event");
        assert_eq!(transition(COMBAT, EV_ATTACKABLE), None, "an FSM error");
        assert_eq!(transition(HOME, EV_MOVE), Some((Callback::None, Change::Pop)));
        let mut one = Stack(vec![COMBAT]);
        one.apply(Change::Pop);
        assert_eq!(one.top(), IDLE, "an emptied stack starts over");
    }

    #[test]
    fn scripts_load_with_their_object() {
        let lua = load(b"Mob = LuaMob(CMob)\nb = 0\nfunction OnNormalReset(t) b = t end", "test.lua", 42).unwrap();
        assert_eq!(lua.globals().get::<u32>("CMob").unwrap(), 42);
        lua.globals().get::<mlua::Function>("OnNormalReset").unwrap().call::<()>(7).unwrap();
        assert_eq!(lua.globals().get::<i64>("b").unwrap(), 7);
        // Without a hook running, the object's functions do not exist.
        assert!(lua.load("Mob:LuaSetHoldPosion(1)").exec().is_err());
        assert!(load(b"this is not lua", "bad.lua", 1).is_err());
    }

    #[test]
    fn a_script_gets_math_and_os_time_and_nothing_else() {
        let lua = load(b"math.randomseed(os.time())\nlater = os.time()", "seed.lua", 1).unwrap();
        assert!(lua.globals().get::<i64>("later").unwrap() > 1_600_000_000);
        for gone in
            ["os.execute", "os.exit", "os.remove", "os.getenv", "os.clock", "io", "package", "require", "dofile", "loadfile", "debug"]
        {
            assert!(lua.load(format!("assert({gone} == nil)")).exec().is_ok(), "{gone} is reachable");
        }
        assert!(lua.load("return math.floor(2.5)").eval::<i64>().unwrap() == 2);
    }

    #[test]
    fn a_script_that_loops_for_ever_ends_with_an_error_and_the_next_call_starts_afresh() {
        let started = std::time::Instant::now();
        let error = load(b"while true do end", "loop.lua", 1).expect_err("the first run is cut short");
        assert!(error.to_string().contains("loop.lua"), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "cut short promptly");

        let lua = load(
            b"function Spin() while true do end end\nfunction Work() local n = 0 for i = 1, 200000 do n = n + i end return n end",
            "w.lua",
            1,
        )
        .unwrap();
        let spin = lua.globals().get::<mlua::Function>("Spin").unwrap();
        assert!(spin.call::<()>(()).is_err());
        // The count is per call: work that is within the limit passes again and again after a runaway.
        let work = lua.globals().get::<mlua::Function>("Work").unwrap();
        for _ in 0..5 {
            reset_budget(&lua);
            assert!(work.call::<i64>(()).is_ok());
        }
        // Without a reset the calls add up.
        let mut failed = false;
        for _ in 0..20 {
            failed |= work.call::<i64>(()).is_err();
        }
        assert!(failed, "the count runs on until the server resets it");
    }
}
