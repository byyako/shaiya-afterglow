//! The points (`docs/plans/ext-5-lua.md` section 6): places where the core asks the scripts, after the core's checks and the rules,
//! and the answers it takes. A point runs on the asking connection's task, synchronously, within a point's limit
//! ([`Limit::POINT`]), and **never while a zone lock, the context's lock or any other mutex is held** (section 4.3): every caller
//! says where it asks and that it holds none. A handler that returns nothing changes nothing; a wrong answer (a factor of 200, a
//! string from `exp`) changes nothing and counts as a fault (section 8.3).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use mlua::{Lua, MultiValue, Value};

use super::{ExtScript, Limit, Point, ScriptSet};
use crate::rules::Refusal;

/// The ids of the refusals scripts make, apart from the rules' (the whisper limit is per id): one per extension and point, new at
/// each load.
static NEXT_REFUSAL: AtomicU32 = AtomicU32::new(1_000_000_000);

/// A refusal's whisper text: printable ASCII, at most 100 bytes, markup the clients can show; otherwise the refusal is silent.
fn refusal_text(text: &Value) -> Option<Arc<str>> {
    let Value::String(s) = text else { return None };
    let bytes = s.as_bytes();
    (!bytes.is_empty() && bytes.len() <= 100 && bytes.iter().all(|b| (0x20..=0x7e).contains(b)) && shaiya_proto::chat::markup_is_safe(&bytes))
        .then(|| Arc::from(String::from_utf8_lossy(&bytes).as_ref()))
}

impl ExtScript {
    /// The id of this extension's refusals at `point`.
    fn refusal_id(&self, point: Point) -> u32 {
        self.refusal_ids[point as usize]
    }

    pub(super) fn fresh_refusal_ids() -> [u32; 8] {
        let first = NEXT_REFUSAL.fetch_add(8, Ordering::Relaxed);
        std::array::from_fn(|i| first + i as u32)
    }

    /// Asks this extension's handler for `point`; the values it returned, `None` when there is no handler, the extension is
    /// disabled, or the call failed (a fault, counted).
    fn ask(&self, point: Point, build: &dyn Fn(&Lua) -> mlua::Result<MultiValue>) -> Option<Vec<Value>> {
        match self.call_built::<MultiValue>(point.name(), Limit::POINT, build) {
            Ok(Some(values)) => Some(values.into_iter().collect()),
            _ => None,
        }
    }

    /// An answer that is not what the point wants: a fault, the answer ignored.
    fn wrong(&self, point: Point, what: &str) {
        self.fault(point.name(), &mlua::Error::runtime(format!("wrong answer: {what}")));
    }
}

fn describe(v: &Value) -> String {
    match v {
        Value::String(s) => format!("the string {:?}", s.to_string_lossy()),
        Value::Number(n) => format!("the number {n}"),
        Value::Integer(n) => format!("the number {n}"),
        Value::Boolean(b) => format!("{b}"),
        other => format!("a {}", other.type_name()),
    }
}

impl ScriptSet {
    /// A refusal point (`trade`, `personal_shop`, `trade_chat`, `chat`): the first extension in install order that returns
    /// `false[, text]` ends the asking (section 6.2). `None`: allowed.
    pub fn refuse(&self, point: Point, build: &dyn Fn(&Lua) -> mlua::Result<MultiValue>) -> Option<Refusal> {
        for ext in self.for_point(point) {
            let Some(answer) = ext.ask(point, build) else { continue };
            match answer.first() {
                None | Some(Value::Nil) | Some(Value::Boolean(true)) => {}
                Some(Value::Boolean(false)) => {
                    let text = answer.get(1).and_then(refusal_text);
                    tracing::debug!(extension = %ext.name, point = point.name(), "refused by a script");
                    return Some(Refusal { rule: ext.refusal_id(point), name: Arc::from(format!("{}/{}", ext.name, point.name())), text });
                }
                Some(other) => ext.wrong(point, &format!("{} where true, false or nothing is wanted", describe(other))),
            }
        }
        None
    }

    /// A factor point (`exp`): each extension is asked with the number so far (`build` gets it) and answers a factor 0 to 100; the
    /// answers multiply. 1 when nobody answered.
    pub fn factor(&self, point: Point, start: f64, build: &dyn Fn(&Lua, f64) -> mlua::Result<MultiValue>) -> f64 {
        let mut factor = 1.0;
        for ext in self.for_point(point) {
            let Some(answer) = ext.ask(point, &|lua| build(lua, start * factor)) else { continue };
            match answer.first() {
                None | Some(Value::Nil) => {}
                Some(Value::Number(n)) if n.is_finite() && (0.0..=100.0).contains(n) => factor *= n,
                Some(Value::Integer(n)) if (0..=100).contains(n) => factor *= *n as f64,
                Some(other) => ext.wrong(point, &format!("{} where a factor from 0 to 100 is wanted", describe(other))),
            }
        }
        factor
    }

    /// The `pvp_kills` point: as [`ScriptSet::factor`] but whole numbers 0 to 100; the product is the multiplier of the counted kill.
    pub fn times(&self, start: u32, build: &dyn Fn(&Lua, u32) -> mlua::Result<MultiValue>) -> u32 {
        let mut times = 1u32;
        for ext in self.for_point(Point::PvpKills) {
            let Some(answer) = ext.ask(Point::PvpKills, &|lua| build(lua, start.saturating_mul(times))) else { continue };
            match answer.first() {
                None | Some(Value::Nil) => {}
                Some(Value::Integer(n)) if (0..=100).contains(n) => times = times.saturating_mul(*n as u32),
                Some(Value::Number(n)) if n.fract() == 0.0 && (0.0..=100.0).contains(n) => times = times.saturating_mul(*n as u32),
                Some(other) => ext.wrong(Point::PvpKills, &format!("{} where a whole number from 0 to 100 is wanted", describe(other))),
            }
        }
        times
    }

    /// The `pvp_death` point: false from any extension means the death is not counted. True: counted.
    pub fn death_counts(&self, build: &dyn Fn(&Lua) -> mlua::Result<MultiValue>) -> bool {
        let mut counts = true;
        for ext in self.for_point(Point::PvpDeath) {
            let Some(answer) = ext.ask(Point::PvpDeath, build) else { continue };
            match answer.first() {
                None | Some(Value::Nil) | Some(Value::Boolean(true)) => {}
                Some(Value::Boolean(false)) => counts = false,
                Some(other) => ext.wrong(Point::PvpDeath, &format!("{} where true, false or nothing is wanted", describe(other))),
            }
        }
        counts
    }

    /// The `enchant` point: each extension is asked with the jump so far and may answer a whole number 1 to 19; the last answer
    /// stands.
    pub fn jump(&self, start: u8, build: &dyn Fn(&Lua, u8) -> mlua::Result<MultiValue>) -> u8 {
        let mut jump = start;
        for ext in self.for_point(Point::Enchant) {
            let Some(answer) = ext.ask(Point::Enchant, &|lua| build(lua, jump)) else { continue };
            match answer.first() {
                None | Some(Value::Nil) => {}
                Some(Value::Integer(n)) if (1..=19).contains(n) => jump = *n as u8,
                Some(Value::Number(n)) if n.fract() == 0.0 && (1.0..=19.0).contains(n) => jump = *n as u8,
                Some(other) => ext.wrong(Point::Enchant, &format!("{} where a jump from 1 to 19 is wanted", describe(other))),
            }
        }
        jump
    }
}

/// What the owner of an item answers when it is used (section 6.1).
#[derive(Clone, Debug, PartialEq)]
pub enum ItemUse {
    /// `nil` (or a wrong answer): the core's own use.
    Core,
    /// `"use"`: one is used up and the use answered done.
    Use,
    /// `"keep"`: done, nothing used up.
    Keep,
    /// `false[, text]`: refused, `use_failed`.
    Refused(Option<Arc<str>>),
}

impl ExtScript {
    /// Asks this extension about the use of an item it registered.
    pub fn item_use(&self, item_type: u8, type_id: u8, build: &dyn Fn(&Lua) -> mlua::Result<MultiValue>) -> ItemUse {
        if self.is_disabled() {
            return ItemUse::Core;
        }
        let answer = self.run("item_use", Limit::POINT, |lua| {
            let Some(f) = super::api::item_fn(lua, item_type, type_id) else { return Ok(Vec::new()) };
            Ok(f.call::<MultiValue>(build(lua)?)?.into_iter().collect::<Vec<Value>>())
        });
        let Ok(answer) = answer else { return ItemUse::Core };
        match answer.first() {
            None | Some(Value::Nil) => ItemUse::Core,
            Some(Value::String(s)) if s.as_bytes() == b"use" => ItemUse::Use,
            Some(Value::String(s)) if s.as_bytes() == b"keep" => ItemUse::Keep,
            Some(Value::Boolean(false)) => ItemUse::Refused(answer.get(1).and_then(refusal_text)),
            Some(other) => {
                self.fault("item_use", &mlua::Error::runtime(format!("wrong answer: {} where \"use\", \"keep\", false or nothing is wanted", describe(other))));
                ItemUse::Core
            }
        }
    }

    /// The id of this extension's item refusals (the whisper limit).
    pub fn item_refusal_id(&self) -> u32 {
        self.refusal_ids[0]
    }
}
