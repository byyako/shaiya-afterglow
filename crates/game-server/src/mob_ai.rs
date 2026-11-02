//! The scripted monsters' state machines (`docs/original/scripted-mobs.md`): the AI files of `world.mob_ai` parsed as `CAIFactory`
//! reads them (section 2), the condition evaluator with the original's typing (section 4), and each monster's script state. The
//! functions and handlers act on the zone (`zone/scripted.rs`).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

/// From this `byAI` on, a monster has a Lua script (`CMob::InitLuaFSM`), run by `crate::mob_lua` (`docs/original/instances.md` section 18).
pub const AI_LUA: u8 = 100;

/// The name hash the files and the machine use (djb2 over the bytes as signed `char`, 32-bit; `FuncID` in `.afs`).
pub fn hash(name: &[u8]) -> u32 {
    name.iter().fold(5381u32, |h, &b| h.wrapping_mul(33).wrapping_add(b as i8 as i32 as u32))
}

/// `GetTickCount` for the scripts: milliseconds on a clock that starts well above 0, as the original's tick count does, so that a
/// time variable still at 0 reads as long ago.
pub fn tick(now: Instant) -> u32 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = *ORIGIN.get_or_init(Instant::now);
    let since = now.checked_duration_since(origin).unwrap_or_default().as_millis() as u64;
    (since + 0x0100_0000) as u32
}

/// `AI_PARAM_TYPE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Void,
    Int,
    Float,
    Dword,
    Str,
}

impl Kind {
    fn from_code(code: i64) -> Self {
        match code {
            1 => Self::Int,
            2 => Self::Float,
            3 => Self::Dword,
            4 => Self::Str,
            _ => Self::Void,
        }
    }
}

/// A value on the stack (`AI_Param`): its type and the field of each type (only the type's own one is meaningful), and the local
/// variable it was read from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Value {
    pub kind: Kind,
    pub n: i32,
    pub f: f32,
    pub d: u32,
    pub var: Option<usize>,
}

impl Value {
    pub fn void() -> Self {
        Self::default()
    }

    pub fn int(n: i32) -> Self {
        Self { kind: Kind::Int, n, ..Self::default() }
    }

    pub fn float(f: f32) -> Self {
        Self { kind: Kind::Float, f, ..Self::default() }
    }

    pub fn dword(d: u32) -> Self {
        Self { kind: Kind::Dword, d, ..Self::default() }
    }

    fn zero(kind: Kind) -> Self {
        Self { kind, ..Self::default() }
    }
}

/// The functions `CMob::DoFunction` knows (section 5), by their hashed names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    GetCurrentTime,
    GetRandInt,
    GetEnergyPercent,
    IsAttackMe,
    IsBattle,
    IsAttackMeAtMoment,
    IsFirstAttack,
    IsEndBattle,
    GetCurrentUser,
    SelectUser,
    IsExistUserAttackPoint,
    GetCurrentMaxHpUser,
    GetDistanceFromExit,
    ArrivalChek,
    GetAttackCount,
    Say,
    Say2,
    UseMobSkill,
    AttackUser,
    SuchFrendlyandRun,
    SetDrMove,
    SetRunawayPosition,
    SetStr,
    SetRec,
    SetAttackSpeed,
    SetHpChage,
    SetRecoveryTime,
    SetEnableRecovery,
    SerChangeRegist,
    AddEnergy,
    SetSummonMob,
    SetBattleType,
    SetEnableOnlyAttackType,
    SetEnableATAll,
    BeginCheckAttackCount,
    EndCheckAttackCount,
    ResetAttackCount,
}

const FUNCS: [(&str, Func); 37] = [
    ("GetCurrentTime", Func::GetCurrentTime),
    ("GetRandInt", Func::GetRandInt),
    ("GetEnergyPercent", Func::GetEnergyPercent),
    ("IsAttackMe", Func::IsAttackMe),
    ("IsBattle", Func::IsBattle),
    ("IsAttackMeAtMoment", Func::IsAttackMeAtMoment),
    ("IsFirstAttack", Func::IsFirstAttack),
    ("IsEndBattle", Func::IsEndBattle),
    ("GetCurrentUser", Func::GetCurrentUser),
    ("SelectUser", Func::SelectUser),
    ("IsExistUserAttackPoint", Func::IsExistUserAttackPoint),
    ("GetCurrentMaxHpUser", Func::GetCurrentMaxHpUser),
    ("GetDistanceFromExit", Func::GetDistanceFromExit),
    ("ArrivalChek", Func::ArrivalChek),
    ("GetAttackCount", Func::GetAttackCount),
    ("Say", Func::Say),
    ("Say2", Func::Say2),
    ("UseMobSkill", Func::UseMobSkill),
    ("AttackUser", Func::AttackUser),
    ("SuchFrendlyandRun", Func::SuchFrendlyandRun),
    ("SetDrMove", Func::SetDrMove),
    ("SetRunawayPosition", Func::SetRunawayPosition),
    ("SetStr", Func::SetStr),
    ("SetRec", Func::SetRec),
    ("SetAttackSpeed", Func::SetAttackSpeed),
    ("SetHpChage", Func::SetHpChage),
    ("SetRecoveryTime", Func::SetRecoveryTime),
    ("SetEnableRecovery", Func::SetEnableRecovery),
    ("SerChangeRegist", Func::SerChangeRegist),
    ("AddEnergy", Func::AddEnergy),
    ("SetSummonMob", Func::SetSummonMob),
    ("SetBattleType", Func::SetBattleType),
    ("SetEnableOnlyAttackType", Func::SetEnableOnlyAttackType),
    ("SetEnableATAll", Func::SetEnableATAll),
    ("BeginCheckAttackCount", Func::BeginCheckAttackCount),
    ("EndCheckAttackCount", Func::EndCheckAttackCount),
    ("ResetAttackCount", Func::ResetAttackCount),
];

impl Func {
    /// The function of a `FuncID`; `None` for the names `DoFunction` has no case for.
    pub fn from_id(id: u32) -> Option<Self> {
        FUNCS.iter().find(|(name, _)| hash(name.as_bytes()) == id).map(|&(_, f)| f)
    }
}

/// The C++ state handlers `CMob::ConnectToFSM` registers (section 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    MoveRandom,
    MoveRandomRun,
    MoveAtSelectedPosition,
    MoveRunAway,
    Battle,
    Debug,
    ResetUser,
}

impl Handler {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "MoveRandom" => Self::MoveRandom,
            "MoveRandomRun" => Self::MoveRandomRun,
            "MoveAtSelectedPosition" => Self::MoveAtSelectedPosition,
            "MoveRunAway" => Self::MoveRunAway,
            "Battle" => Self::Battle,
            "Debug" => Self::Debug,
            "ResetUser" => Self::ResetUser,
            _ => return None,
        })
    }
}

/// `OPCODE`.
pub mod op {
    pub const GREATER: u8 = 0;
    pub const LESS: u8 = 1;
    pub const GREATER_EQUALS: u8 = 2;
    pub const LESS_EQUALS: u8 = 3;
    pub const EQUALS: u8 = 4;
    pub const NOT_EQUALS: u8 = 5;
    pub const PLUS: u8 = 6;
    pub const MINUS: u8 = 7;
    pub const MUL: u8 = 8;
    pub const DIV: u8 = 9;
    pub const SUBSTITUTION: u8 = 10;
    pub const PLUS_SUBSTITUTION: u8 = 11;
    pub const MINUS_SUBSTITUTION: u8 = 12;
    pub const ADD: u8 = 13;
    pub const SUB: u8 = 14;
    pub const MUL_SUBSTITUTION: u8 = 15;
    pub const DIV_SUBSTITUTION: u8 = 16;
}

/// One token of a condition (`CKToken` by `KTOKEN_TYPE`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Token {
    Number(Value),
    /// A local variable by its index; `None` for a name the `.alo` does not declare.
    Variable(Option<usize>),
    /// A function and its parameter count in the `.afs`.
    Function {
        func: Option<Func>,
        params: usize,
    },
    Param,
    Text,
    Operator(u8),
    Compare(u8),
}

/// A condition: an action (type 0) or a test (type 1), and its tokens.
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    pub test: bool,
    pub tokens: Vec<Token>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub target: usize,
    pub conditions: Vec<Condition>,
    /// How many of the conditions are tests (`KFSMTransition::AddCondition`).
    pub tests: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub name: String,
    /// Enter, update and exit handlers, from the `.ah`.
    pub handlers: [Option<Handler>; 3],
    pub on_enter: Vec<Condition>,
    pub on_update: Vec<Condition>,
    pub on_exit: Vec<Condition>,
    pub transitions: Vec<Transition>,
}

/// One AI of `mob_ai.ini`.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub name: String,
    pub states: Vec<State>,
    pub start: usize,
    /// The local variables' types, in `.alo` order.
    pub vars: Vec<Kind>,
}

impl Machine {
    /// Parses the four files (section 2); an error names what does not fit.
    pub fn parse(alo: &str, afs: &str, ai: &str, ah: &str) -> Result<Self, String> {
        let vars = parse_alo(alo)?;
        let funcs = parse_afs(afs)?;
        let handlers = parse_ah(ah);
        parse_ai(ai, &vars, &funcs, &handlers)
    }

    /// The variables at 0, each of its type (`CAIFactory::ClearVariable`).
    pub fn fresh_vars(&self) -> Vec<Value> {
        self.vars.iter().map(|&k| Value::zero(k)).collect()
    }
}

/// `Key : value` lines, read in order.
struct Lines<'a> {
    lines: Vec<(&'a str, &'a str)>,
    at: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let lines = text
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .map(|l| match l.split_once(" : ") {
                Some((k, v)) => (k.trim(), v.trim()),
                None => match l.split_once(':') {
                    Some((k, v)) => (k.trim(), v.trim()),
                    None => (l.trim(), ""),
                },
            })
            .collect();
        Self { lines, at: 0 }
    }

    fn get(&mut self, key: &str) -> Result<&'a str, String> {
        let Some(&(k, v)) = self.lines.get(self.at) else { return Err(format!("{key} expected at the end")) };
        if k != key {
            return Err(format!("{key} expected, {k} found (line {})", self.at + 1));
        }
        self.at += 1;
        Ok(v)
    }

    fn number(&mut self, key: &str) -> Result<i64, String> {
        let v = self.get(key)?;
        v.parse::<i64>().map_err(|_| format!("{key}: {v} is not a number"))
    }

    fn count(&mut self, key: &str) -> Result<usize, String> {
        let n = self.number(key)?;
        usize::try_from(n).map_err(|_| format!("{key}: {n}"))
    }
}

/// `.alo`: the local variables' hashes and types.
fn parse_alo(text: &str) -> Result<Vec<(u32, Kind)>, String> {
    let mut r = Lines::new(text);
    let n = r.count("LocalVariableCount")?;
    (0..n)
        .map(|_| {
            let kind = Kind::from_code(r.number("Type")?);
            Ok((hash(r.get("Name")?.as_bytes()), kind))
        })
        .collect()
}

/// `.afs`: each function's parameter count by its `FuncID`.
fn parse_afs(text: &str) -> Result<HashMap<u32, usize>, String> {
    let mut r = Lines::new(text);
    let n = r.count("SystemFunctionCount")?;
    let mut out = HashMap::new();
    for _ in 0..n {
        r.get("Name")?;
        let id = r.get("FuncID")?.parse::<u32>().map_err(|_| "FuncID".to_string())?;
        r.number("ReturnType")?;
        let params = r.count("ParamCount")?;
        for _ in 0..params {
            r.number("ParamType")?;
            r.number("ParamPipe")?;
        }
        out.insert(id, params);
    }
    Ok(out)
}

/// `.ah`: `name,enter,update,exit` per state; names the handlers do not know are none.
fn parse_ah(text: &str) -> HashMap<String, [Option<Handler>; 3]> {
    text.lines()
        .filter_map(|l| {
            let cols: Vec<&str> = l.trim().split(',').map(str::trim).collect();
            let [name, enter, update, exit] = cols[..] else { return None };
            Some((name.to_string(), [enter, update, exit].map(Handler::named)))
        })
        .collect()
}

fn parse_conditions(r: &mut Lines<'_>, n: usize, vars: &[(u32, Kind)], funcs: &HashMap<u32, usize>) -> Result<Vec<Condition>, String> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let test = r.number("ConditionType")? == 1;
        let count = r.count("TokenCount")?;
        let mut tokens = Vec::with_capacity(count);
        for _ in 0..count {
            let kind = r.number("Type")?;
            let name = r.get("Name")?;
            let id = if kind == 4 { r.get("FuncID")?.parse::<u32>().map_err(|_| "FuncID".to_string())? } else { 0 };
            let opcode = if kind == 8 || kind == 9 { r.number("OpCode")? } else { 0 };
            r.number("ParamCount")?;
            let number_type = r.number("NumberType")?;
            let value = if (1..=3).contains(&number_type) { r.get("Value")? } else { "" };
            let token = match kind {
                2 => Token::Number(match number_type {
                    1 => Value::int(value.parse().map_err(|_| format!("Value {value}"))?),
                    2 => Value::float(value.parse().map_err(|_| format!("Value {value}"))?),
                    3 => Value::dword(value.parse().map_err(|_| format!("Value {value}"))?),
                    _ => Value::void(),
                }),
                3 => Token::Variable(vars.iter().position(|&(h, _)| h == hash(name.as_bytes()))),
                4 => Token::Function { func: Func::from_id(id), params: funcs.get(&id).copied().unwrap_or(0) },
                5 => Token::Param,
                7 => Token::Text,
                8 => Token::Operator(u8::try_from(opcode).map_err(|_| format!("OpCode {opcode}"))?),
                9 => Token::Compare(u8::try_from(opcode).map_err(|_| format!("OpCode {opcode}"))?),
                // Parentheses, commas and the like do nothing in the evaluator.
                _ => continue,
            };
            tokens.push(token);
        }
        out.push(Condition { test, tokens });
    }
    Ok(out)
}

/// `.ai`, the machine.
fn parse_ai(
    text: &str,
    vars: &[(u32, Kind)],
    funcs: &HashMap<u32, usize>,
    handlers: &HashMap<String, [Option<Handler>; 3]>,
) -> Result<Machine, String> {
    let mut r = Lines::new(text);
    let name = r.get("BehaviorName")?.to_string();
    let count = r.count("StateCount")?;
    // Targets are names until every state is known.
    let mut states = Vec::with_capacity(count);
    let mut targets: Vec<Vec<String>> = Vec::with_capacity(count);
    for _ in 0..count {
        let state = r.get("StateName")?.to_string();
        r.get("StateInit")?;
        r.get("StateUpdate")?;
        r.get("StateExit")?;
        let n = r.count("StartConditionCount")?;
        let on_enter = parse_conditions(&mut r, n, vars, funcs)?;
        let n = r.count("UpdateConditionCount")?;
        let on_update = parse_conditions(&mut r, n, vars, funcs)?;
        let n = r.count("ExitConditionCount")?;
        let on_exit = parse_conditions(&mut r, n, vars, funcs)?;
        let n = r.count("TransitionCount")?;
        let mut transitions = Vec::with_capacity(n);
        let mut names = Vec::with_capacity(n);
        for _ in 0..n {
            r.get("TransitionName")?;
            r.get("TransitionStart")?;
            names.push(r.get("TransitionEnd")?.to_string());
            let k = r.count("ConditionCount")?;
            let conditions = parse_conditions(&mut r, k, vars, funcs)?;
            let tests = conditions.iter().filter(|c| c.test).count();
            transitions.push(Transition { target: 0, conditions, tests });
        }
        let handlers = handlers.get(&state).copied().unwrap_or([None; 3]);
        states.push(State { name: state, handlers, on_enter, on_update, on_exit, transitions });
        targets.push(names);
    }
    let index = |name: &str| states.iter().position(|s| s.name == name).ok_or_else(|| format!("no state {name}"));
    let start = index(r.get("StartState")?)?;
    let resolved: Vec<Vec<usize>> =
        targets.iter().map(|names| names.iter().map(|n| index(n)).collect::<Result<_, _>>()).collect::<Result<_, _>>()?;
    for (state, ids) in states.iter_mut().zip(resolved) {
        for (t, id) in state.transitions.iter_mut().zip(ids) {
            t.target = id;
        }
    }
    Ok(Machine { name, states, start, vars: vars.iter().map(|&(_, k)| k).collect() })
}

/// What the evaluator needs from the monster: its variables and the functions.
pub trait Host {
    fn vars(&mut self) -> &mut Vec<Value>;
    fn call(&mut self, func: Func, args: &[Value]) -> Value;
}

/// Evaluates a condition (`CKFSM::TravelCondition`): its count of true comparisons. Actions run as they are met.
pub fn evaluate(condition: &Condition, host: &mut impl Host) -> usize {
    let mut at = 0;
    let mut stack = Vec::new();
    let mut count = 0;
    run(&condition.tokens, &mut at, false, host, &mut stack, &mut count);
    count
}

/// The tokens from `at` on; an argument (`CKFSM::CalculateParam`) ends at its `Param` token.
fn run(tokens: &[Token], at: &mut usize, argument: bool, host: &mut impl Host, stack: &mut Vec<Value>, count: &mut usize) {
    while let Some(&token) = tokens.get(*at) {
        *at += 1;
        match token {
            Token::Param if argument => return,
            Token::Param => {}
            Token::Number(v) => stack.push(v),
            Token::Variable(Some(i)) => {
                let v = host.vars().get(i).copied().unwrap_or_default();
                stack.push(Value { var: Some(i), ..v });
            }
            Token::Variable(None) | Token::Text => {
                stack.push(Value { kind: if token == Token::Text { Kind::Str } else { Kind::Void }, ..Value::void() })
            }
            Token::Function { func, params } => {
                let mut args = Vec::with_capacity(params);
                for _ in 0..params {
                    let mut own = Vec::new();
                    let mut ignored = 0;
                    run(tokens, at, true, host, &mut own, &mut ignored);
                    // Each argument keeps its own type (no conversion to the declared one).
                    args.push(Value { var: None, ..own.last().copied().unwrap_or_default() });
                }
                stack.push(func.map_or_else(Value::void, |f| host.call(f, &args)));
            }
            Token::Operator(o) => {
                let right = stack.pop().unwrap_or_default();
                let left = stack.pop().unwrap_or_default();
                let result = operate(o, left, right);
                if o >= op::SUBSTITUTION
                    && let Some(i) = left.var
                    && let Some(slot) = host.vars().get_mut(i)
                {
                    *slot = Value { var: None, ..result };
                }
                stack.push(result);
            }
            Token::Compare(o) => {
                let right = stack.pop().unwrap_or_default();
                let left = stack.pop().unwrap_or_default();
                if compare(o, left, right) {
                    *count += 1;
                }
            }
        }
    }
}

/// The right operand as the evaluator reads it: void and strings as the int of their int field.
fn right_kind(v: Value) -> Kind {
    match v.kind {
        Kind::Float => Kind::Float,
        Kind::Dword => Kind::Dword,
        _ => Kind::Int,
    }
}

fn int_op(o: u8, a: i32, b: i32) -> i32 {
    match o {
        op::PLUS | op::PLUS_SUBSTITUTION => a.wrapping_add(b),
        op::MINUS | op::MINUS_SUBSTITUTION => a.wrapping_sub(b),
        op::MUL | op::MUL_SUBSTITUTION => a.wrapping_mul(b),
        op::DIV | op::DIV_SUBSTITUTION => a.checked_div(b).unwrap_or(0),
        op::ADD => a.wrapping_add(1),
        op::SUB => a.wrapping_sub(1),
        _ => b,
    }
}

fn dword_op(o: u8, a: u32, b: u32) -> u32 {
    match o {
        op::PLUS | op::PLUS_SUBSTITUTION => a.wrapping_add(b),
        op::MINUS | op::MINUS_SUBSTITUTION => a.wrapping_sub(b),
        op::MUL | op::MUL_SUBSTITUTION => a.wrapping_mul(b),
        op::DIV | op::DIV_SUBSTITUTION => a.checked_div(b).unwrap_or(0),
        op::ADD => a.wrapping_add(1),
        op::SUB => a.wrapping_sub(1),
        _ => b,
    }
}

fn float_op(o: u8, a: f32, b: f32) -> f32 {
    match o {
        op::PLUS | op::PLUS_SUBSTITUTION => a + b,
        op::MINUS | op::MINUS_SUBSTITUTION => a - b,
        op::MUL | op::MUL_SUBSTITUTION => a * b,
        op::DIV | op::DIV_SUBSTITUTION => a / b,
        op::ADD => a + 1.0,
        op::SUB => a - 1.0,
        _ => b,
    }
}

/// An operator (section 4): the result has the left operand's type, the right one converted to it; `=` is the right value so
/// converted. A left operand that is void or a string gives void.
pub fn operate(o: u8, left: Value, right: Value) -> Value {
    let right_f = |r: Value| match right_kind(r) {
        Kind::Float => r.f,
        Kind::Dword => r.d as f32,
        _ => r.n as f32,
    };
    match left.kind {
        Kind::Int => Value::int(match right_kind(right) {
            Kind::Float => float_op(o, left.n as f32, right.f) as i32,
            Kind::Dword => int_op(o, left.n, right.d as i32),
            _ => int_op(o, left.n, right.n),
        }),
        Kind::Float => Value::float(float_op(o, left.f, right_f(right))),
        Kind::Dword => Value::dword(match right_kind(right) {
            Kind::Float => float_op(o, left.d as f32, right.f) as i64 as u32,
            Kind::Dword => dword_op(o, left.d, right.d),
            _ => dword_op(o, left.d, right.n as u32),
        }),
        Kind::Void | Kind::Str => Value::void(),
    }
}

fn ordered<T: PartialOrd>(o: u8, a: T, b: T) -> bool {
    match o {
        op::GREATER => a > b,
        op::LESS => a < b,
        op::GREATER_EQUALS => a >= b,
        op::LESS_EQUALS => a <= b,
        op::EQUALS => a == b,
        op::NOT_EQUALS => a != b,
        _ => false,
    }
}

/// A comparison (section 4): int with int signed, dword with int unsigned, anything with a float in float; a dword on the right,
/// and a void or string on the left, never compare true.
pub fn compare(o: u8, left: Value, right: Value) -> bool {
    match (left.kind, right_kind(right)) {
        (_, Kind::Dword) => false,
        (Kind::Int, Kind::Int) => ordered(o, left.n, right.n),
        (Kind::Int, _) => ordered(o, left.n as f32, right.f),
        (Kind::Float, Kind::Int) => ordered(o, left.f, right.n as f32),
        (Kind::Float, _) => ordered(o, left.f, right.f),
        (Kind::Dword, Kind::Int) => ordered(o, left.d, right.n as u32),
        (Kind::Dword, _) => ordered(o, left.d as f32, right.f),
        (Kind::Void | Kind::Str, _) => false,
    }
}

/// `SetEnableOnlyAttackType(kind)` (`CMob::PAI_SetAttackType`), also a summon's attack kinds: 4 all, 5 none, 0 to 3 only that one
/// (another number turns all off: the original writes beyond the four).
pub fn attack_kinds(kind: i32) -> [bool; 4] {
    match kind {
        4 => [true; 4],
        0..=3 => std::array::from_fn(|i| i as i32 == kind),
        _ => [false; 4],
    }
}
/// What the script functions change on the monster (section 5) and the code outside the script reads (section 7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flags {
    /// `m_bAttacked`, `m_bIsBattle`, `m_bAttackAtMoment`.
    pub attacked: bool,
    pub battle: bool,
    pub at_moment: bool,
    /// `m_nBattleType`: not 0, a skill's miss does not mark the monster attacked.
    pub battle_type: i32,
    /// Hits counted (`m_bEnableCheckAttackCount`, `m_nAttackedCount`).
    pub counting: bool,
    pub count: i32,
    /// Recovery in battle (`m_bRecoverEnable`, `m_dwRecoverTime` ms, `m_fRecoverRatio`).
    pub recover: bool,
    pub recover_time: u32,
    pub recover_ratio: f32,
    /// `m_fStr`, `m_fAttackSpeed`, `m_fPhysicalDefense`.
    pub strength: f32,
    pub attack_speed: f32,
    pub defence: f32,
    /// `m_nAttrDef` set by `SerChangeRegist`; `None` keeps the definition's element.
    pub element: Option<u8>,
    /// `m_bAttackEnableType`: the players' attack kinds (0 melee, 1 ranged, 2 magic, 3 none of them) that can reach it; the original
    /// never reads it, the new server does (`docs/original/scripted-mobs.md` deviation 5).
    pub attack_kinds: [bool; 4],
    /// `m_vPosSelected`, `m_vPosExitPos`.
    pub selected: [f32; 3],
    pub exit: [f32; 3],
}

impl Default for Flags {
    /// As `CMob::Init` leaves them.
    fn default() -> Self {
        Self {
            attacked: false,
            battle: false,
            at_moment: false,
            battle_type: 0,
            counting: false,
            count: 0,
            recover: false,
            recover_time: 5000,
            recover_ratio: 1.0,
            strength: 1.0,
            attack_speed: 1.0,
            defence: 1.0,
            element: None,
            attack_kinds: [true; 4],
            selected: [0.0; 3],
            exit: [0.0; 3],
        }
    }
}

/// A scripted monster's machine state: its current state and variables, the flags, and whether it is due to start over
/// (`CMob::ResetFSM`; a new monster starts that way).
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub state: usize,
    pub vars: Vec<Value>,
    pub flags: Flags,
    pub restart: bool,
}

impl Script {
    pub fn new() -> Self {
        Self { state: 0, vars: Vec::new(), flags: Flags::default(), restart: true }
    }
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

/// The parsed AIs by number and the lines they say (`kr_string.DB`, the English of deviation 1 where there is one).
#[derive(Debug, Default)]
pub struct Library {
    pub machines: HashMap<u8, Machine>,
    pub lines: HashMap<i32, Vec<u8>>,
}

#[cfg(test)]
pub mod samples {
    //! AI files built as the editor writes them, for the tests.
    use super::*;

    const CROCO1_ALO: &str = "LocalVariableCount : 5\r\nType : 1\r\nName : nCount\r\nType : 3\r\nName : dwTime\r\nType : 3\r\nName : UserID\r\nType : 1\r\nName : TempNumber\r\nType : 3\r\nName : dwTimeSecond\r\n";
    const CROCO1_AH: &str = "init,Empty,Empty,Empty\nidle,Empty,Empty,Empty\nbattle,Battle,Battle,Empty\nBeStrong,Empty,Empty,Empty\n";

    fn afs(funcs: &[(&str, usize)]) -> String {
        let mut s = format!("SystemFunctionCount : {}\n", funcs.len());
        for (name, params) in funcs {
            s += &format!("Name : {name}\nFuncID : {}\nReturnType : 1\nParamCount : {params}\n", hash(name.as_bytes()));
            for _ in 0..*params {
                s += "ParamType : 1\nParamPipe : 0\n";
            }
        }
        s
    }

    /// A token as the editor writes it.
    fn tok(kind: u8, name: &str, number_type: u8, value: &str) -> String {
        let mut s = format!("Type : {kind}\nName : {name}\n");
        if kind == 4 {
            s += &format!("FuncID : {}\n", hash(name.as_bytes()));
        }
        if kind == 8 || kind == 9 {
            let code = ["> ", "< ", ">=", "<=", "==", "<>", "+ ", "- ", "* ", "/ ", "= "].iter().position(|o| o.trim() == name).unwrap();
            s += &format!("OpCode : {code}\n");
        }
        s + &format!("ParamCount : -842150451\nNumberType : {number_type}\nValue : {value}\n")
    }

    fn cond(kind: u8, tokens: &[String]) -> String {
        format!("ConditionType : {kind}\nTokenCount : {}\n{}", tokens.len(), tokens.concat())
    }

    pub fn state(name: &str, handlers: &str, enter: &[String], transitions: &[(&str, Vec<String>)]) -> String {
        let [i, u, x]: [&str; 3] = handlers.split('/').collect::<Vec<_>>().try_into().unwrap();
        let mut s = format!(
            "StateName : {name}\nStateInit : {i}\nStateUpdate : {u}\nStateExit : {x}\nStartConditionCount : {}\n{}UpdateConditionCount : 0\nExitConditionCount : 0\nTransitionCount : {}\n",
            enter.len(),
            enter.concat(),
            transitions.len()
        );
        for (n, (to, conds)) in transitions.iter().enumerate() {
            s += &format!(
                "TransitionName : {}\nTransitionStart : {name}\nTransitionEnd : {to}\nConditionCount : {}\n{}",
                n + 1,
                conds.len(),
                conds.concat()
            );
        }
        s
    }

    /// `croco1.ai` as the original files have it (`AI` 11).
    pub fn croco1() -> Machine {
        let afs = afs(&[
            ("GetCurrentTime", 0),
            ("IsAttackMe", 0),
            ("IsBattle", 0),
            ("GetEnergyPercent", 0),
            ("AttackUser", 2),
            ("GetCurrentUser", 0),
            ("IsFirstAttack", 0),
            ("SetStr", 1),
        ]);
        let test = |a: String, o: &str, b: String| cond(1, &[a, b, tok(9, o, 1, "0")]);
        let f = |name: &str| tok(4, name, 1, "0");
        let var = |name: &str, t: u8| tok(3, name, t, "0");
        let int = |v: i32| tok(2, &v.to_string(), 1, &v.to_string());
        let float = |v: &str| tok(2, v, 2, &format!("{:.6}", v.parse::<f32>().unwrap()));
        let param = || tok(5, "Param", 1, "1");
        let ai = format!(
            "BehaviorName : Croco1\nStateCount : 4\n{}{}{}{}StartState : init\n",
            state(
                "init",
                "Empty/Empty/Empty",
                &[],
                &[("idle", vec![cond(0, &[var("dwTime", 3), f("GetCurrentTime"), tok(8, "=", 1, "0")])])]
            ),
            state(
                "idle",
                "Empty/Empty/Empty",
                &[],
                &[
                    ("battle", vec![test(f("IsFirstAttack"), "==", int(1)), test(var("nCount", 1), "==", int(0))]),
                    ("battle", vec![test(f("IsAttackMe"), "==", int(1)), test(var("nCount", 1), "==", int(0))]),
                ]
            ),
            state(
                "battle",
                "Battle/Battle/Empty",
                &[],
                &[
                    ("BeStrong", vec![test(f("GetEnergyPercent"), "<=", float("0.5")), test(var("nCount", 1), "==", int(0))]),
                    ("init", vec![test(f("IsBattle"), "==", int(0))]),
                ]
            ),
            state(
                "BeStrong",
                "Empty/Empty/Empty",
                &[
                    cond(0, &[var("UserID", 3), f("GetCurrentUser"), tok(8, "=", 1, "0")]),
                    cond(0, &[f("AttackUser"), var("UserID", 3), param(), float("10.0"), param()]),
                    cond(0, &[var("nCount", 1), int(1), tok(8, "=", 1, "0")]),
                    cond(0, &[f("SetStr"), float("1.5"), param()]),
                ],
                &[("battle", vec![])]
            ),
        );
        Machine::parse(CROCO1_ALO, &afs, &ai, CROCO1_AH).unwrap()
    }

    /// A machine that speaks on entering: "talk" says line 1 to its target, then "shout" line 2 to the players within 200; it is
    /// back in "talk" only after a start-over.
    pub fn sayer() -> Machine {
        let afs = afs(&[("Say", 1), ("Say2", 2)]);
        let int = |v: i32| tok(2, &v.to_string(), 1, &v.to_string());
        let param = || tok(5, "Param", 1, "1");
        let f = |name: &str| tok(4, name, 1, "0");
        let ai = format!(
            "BehaviorName : Sayer\nStateCount : 2\n{}{}StartState : talk\n",
            state("talk", "Empty/Empty/Empty", &[cond(0, &[f("Say"), int(1), param()])], &[("shout", vec![])]),
            state("shout", "Empty/Empty/Empty", &[cond(0, &[f("Say2"), int(2), param(), int(200), param()])], &[]),
        );
        Machine::parse("LocalVariableCount : 0\n", &afs, &ai, "").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::samples::*;
    use super::*;

    #[derive(Default)]
    struct Probe {
        vars: Vec<Value>,
        calls: Vec<(Func, Vec<Value>)>,
        hp: f32,
        attacked: bool,
        time: u32,
    }

    impl Host for Probe {
        fn vars(&mut self) -> &mut Vec<Value> {
            &mut self.vars
        }

        fn call(&mut self, func: Func, args: &[Value]) -> Value {
            self.calls.push((func, args.to_vec()));
            match func {
                Func::GetCurrentTime => Value::dword(self.time),
                Func::GetEnergyPercent => Value::float(self.hp),
                Func::IsAttackMe => Value::int(i32::from(self.attacked)),
                Func::IsFirstAttack | Func::IsBattle => Value::int(0),
                Func::GetCurrentUser => Value::dword(77),
                _ => Value::void(),
            }
        }
    }

    #[test]
    fn names_hash_as_the_files_say() {
        assert_eq!(hash(b"Say"), 193_470_706);
        assert_eq!(hash(b"GetCurrentTime"), 914_508_983);
        assert_eq!(hash(b"IsAttackMe"), 2_757_518_987);
        assert_eq!(Func::from_id(3_347_793_440), Some(Func::GetEnergyPercent));
        assert_eq!(Func::from_id(hash(b"GetBloodType")), None);
    }

    #[test]
    fn croco1_parses_with_its_handlers_and_targets() {
        let m = croco1();
        assert_eq!((m.name.as_str(), m.states.len(), m.start), ("Croco1", 4, 0));
        assert_eq!(m.vars, [Kind::Int, Kind::Dword, Kind::Dword, Kind::Int, Kind::Dword]);
        let battle = &m.states[2];
        assert_eq!(battle.handlers, [Some(Handler::Battle), Some(Handler::Battle), None]);
        assert_eq!(battle.transitions.iter().map(|t| (t.target, t.tests)).collect::<Vec<_>>(), [(3, 2), (0, 1)]);
        assert_eq!(m.states[0].transitions[0].tests, 0, "an action is not a test");
        assert_eq!(m.states[3].on_enter[1].tokens[0], Token::Function { func: Some(Func::AttackUser), params: 2 });
    }

    #[test]
    fn actions_assign_and_tests_count() {
        let m = croco1();
        let mut host = Probe { vars: m.fresh_vars(), time: 12_345, ..Default::default() };
        // init -> idle: "dwTime = GetCurrentTime()", an action without a comparison.
        assert_eq!(evaluate(&m.states[0].transitions[0].conditions[0], &mut host), 0);
        assert_eq!(host.vars[1], Value::dword(12_345), "the variable keeps its type");
        // battle -> BeStrong: "GetEnergyPercent() <= 0.5" true at 0.5, false above.
        let t = &m.states[2].transitions[0];
        host.hp = 0.5;
        assert_eq!(t.conditions.iter().map(|c| evaluate(c, &mut host)).sum::<usize>(), 2);
        host.hp = 0.51;
        assert_eq!(evaluate(&t.conditions[0], &mut host), 0);
    }

    #[test]
    fn arguments_follow_the_function_with_their_own_types() {
        let m = croco1();
        let mut host = Probe { vars: m.fresh_vars(), ..Default::default() };
        for c in &m.states[3].on_enter {
            evaluate(c, &mut host);
        }
        let calls: Vec<_> = host.calls.iter().map(|(f, a)| (*f, a.clone())).collect();
        assert_eq!(calls[0], (Func::GetCurrentUser, vec![]));
        assert_eq!(calls[1], (Func::AttackUser, vec![Value::dword(77), Value::float(10.0)]));
        assert_eq!(calls[2], (Func::SetStr, vec![Value::float(1.5)]));
        assert_eq!(host.vars[0], Value::int(1), "nCount = 1");
    }

    #[test]
    fn operators_take_the_left_type() {
        let sub = |l, r| operate(op::MINUS, l, r);
        assert_eq!(sub(Value::dword(5), Value::dword(9)), Value::dword(u32::MAX - 3), "unsigned wrap");
        assert_eq!(sub(Value::int(7), Value::float(2.5)), Value::int(4), "trunc(4.5)");
        assert_eq!(sub(Value::float(7.0), Value::int(2)), Value::float(5.0));
        assert_eq!(operate(op::MUL, Value::int(3), Value::int(50_000)), Value::int(150_000));
        assert_eq!(operate(op::SUBSTITUTION, Value::int(0), Value::float(2.9)), Value::int(2));
        assert_eq!(operate(op::SUBSTITUTION, Value::float(0.0), Value::dword(u32::MAX)), Value::float(u32::MAX as f32));
        assert_eq!(sub(Value::void(), Value::int(1)), Value::void());
        assert_eq!(operate(op::DIV, Value::int(1), Value::int(0)), Value::int(0), "no crash");
    }

    #[test]
    fn comparisons_follow_the_original_cases() {
        let eq = |l, r| compare(op::EQUALS, l, r);
        assert!(compare(op::GREATER, Value::dword(6000), Value::int(5000)));
        assert!(!compare(op::GREATER, Value::dword(u32::MAX), Value::int(-1)), "unsigned: -1 is the largest");
        assert!(compare(op::LESS_EQUALS, Value::float(0.5), Value::float(0.5)));
        assert!(eq(Value::int(1), Value::float(1.0)));
        assert!(!eq(Value::int(5), Value::dword(5)), "a dword on the right never compares true");
        assert!(!eq(Value::void(), Value::int(0)), "nor a void on the left");
        assert!(eq(Value::int(0), Value::void()), "a void on the right reads as the int 0");
        assert!(compare(op::NOT_EQUALS, Value::dword(3), Value::int(0)));
    }

    #[test]
    fn transitions_to_unknown_states_do_not_load() {
        let ai = format!("BehaviorName : X\nStateCount : 1\n{}StartState : a\n", state("a", "Empty/Empty/Empty", &[], &[("b", vec![])]));
        assert!(Machine::parse("LocalVariableCount : 0\n", "SystemFunctionCount : 0\n", &ai, "").is_err());
    }
}
