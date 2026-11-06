//! An extension's options (`docs/plans/ext-5-lua.md` sections 3.1 and 3.4): the specs of the manifest's `[options.<key>]`, the
//! validation of a value against one, and the Lua table `game.options` made from the values in force. Pure: no state, no database.

use anyhow::{Result, bail};
use serde_json::{Map, Value};

/// The most options an extension may declare.
pub const MAX_OPTIONS: usize = 64;
/// The longest string option (bytes) and the most entries of a list option.
pub const MAX_STRING: usize = 1000;
pub const MAX_LIST: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptType {
    String,
    Integer,
    Float,
    Boolean,
    Maps,
    Strings,
    Integers,
}

impl OptType {
    pub const NAMES: [&'static str; 7] = ["string", "integer", "float", "boolean", "maps", "strings", "integers"];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "string" => Self::String,
            "integer" => Self::Integer,
            "float" => Self::Float,
            "boolean" => Self::Boolean,
            "maps" => Self::Maps,
            "strings" => Self::Strings,
            "integers" => Self::Integers,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// One option as the manifest declares it.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionSpec {
    pub ty: OptType,
    pub default: Value,
    pub doc: String,
    pub min: Option<Value>,
    pub max: Option<Value>,
}

/// An option key: `^[a-z][a-z0-9_]{0,39}$`.
pub fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    key.len() <= 40
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn printable(text: &str) -> bool {
    text.bytes().all(|b| (0x20..=0x7e).contains(&b))
}

impl OptionSpec {
    /// Reads `{ type, default, doc, min, max }` (the manifest table as JSON); the default is validated against the rest.
    pub fn parse(spec: &Value) -> Result<Self> {
        let Some(map) = spec.as_object() else { bail!("an option is a table with type, default and doc") };
        for key in map.keys() {
            if !["type", "default", "doc", "min", "max"].contains(&key.as_str()) {
                bail!("unknown key {key:?} (an option has type, default, doc, min and max)");
            }
        }
        let ty = match map.get("type") {
            Some(Value::String(name)) => {
                OptType::from_name(name).ok_or_else(|| anyhow::anyhow!("type {name:?} is not one of {}", OptType::NAMES.join(", ")))?
            }
            Some(_) => bail!("type must be a string"),
            None => bail!("type is required"),
        };
        let doc = match map.get("doc") {
            Some(Value::String(doc)) if !doc.trim().is_empty() && doc.len() <= 300 && !doc.contains('\n') => doc.clone(),
            Some(Value::String(_)) => bail!("doc must be one line of 1 to 300 bytes"),
            Some(_) => bail!("doc must be a string"),
            None => bail!("doc is required"),
        };
        let Some(default) = map.get("default").cloned() else { bail!("default is required") };
        let numeric = matches!(ty, OptType::Integer | OptType::Float);
        let bound = |key: &str| -> Result<Option<Value>> {
            let Some(v) = map.get(key) else { return Ok(None) };
            if !numeric {
                bail!("{key} applies to integer and float options only");
            }
            let ok = match ty {
                OptType::Integer => v.is_i64() || v.is_u64(),
                _ => v.is_number(),
            };
            if !ok {
                bail!("{key} must be {} number", if ty == OptType::Integer { "a whole" } else { "a" });
            }
            Ok(Some(v.clone()))
        };
        let (min, max) = (bound("min")?, bound("max")?);
        if let (Some(a), Some(b)) = (&min, &max)
            && a.as_f64() > b.as_f64()
        {
            bail!("min {a} is above max {b}");
        }
        let spec = Self { ty, default, doc, min, max };
        spec.check(&spec.default).map_err(|e| anyhow::anyhow!("default: {e}"))?;
        Ok(spec)
    }

    /// The spec back as JSON (what `content.ext_options.spec` keeps).
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("type".into(), self.ty.name().into());
        m.insert("default".into(), self.default.clone());
        m.insert("doc".into(), self.doc.clone().into());
        if let Some(v) = &self.min {
            m.insert("min".into(), v.clone());
        }
        if let Some(v) = &self.max {
            m.insert("max".into(), v.clone());
        }
        Value::Object(m)
    }

    /// Whether `value` is a value of this option: its type, then `min` and `max`.
    pub fn check(&self, value: &Value) -> Result<()> {
        match self.ty {
            OptType::String => self.string(value).map(drop),
            OptType::Boolean => match value {
                Value::Bool(_) => Ok(()),
                other => bail!("{} is not a boolean", show(other)),
            },
            OptType::Integer => match value {
                Value::Number(n) if n.is_i64() => self.range(n.as_i64().unwrap() as f64, value),
                other => bail!("{} is not a whole number", show(other)),
            },
            OptType::Float => match value.as_f64() {
                Some(f) if f.is_finite() => self.range(f, value),
                _ => bail!("{} is not a number", show(value)),
            },
            OptType::Strings => {
                for v in self.list(value)? {
                    self.string(v)?;
                }
                Ok(())
            }
            OptType::Integers => {
                for v in self.list(value)? {
                    if !v.as_i64().is_some() {
                        bail!("{} is not a whole number", show(v));
                    }
                }
                Ok(())
            }
            OptType::Maps => {
                for v in self.list(value)? {
                    map_entry(v)?;
                }
                Ok(())
            }
        }
    }

    fn range(&self, n: f64, shown: &Value) -> Result<()> {
        if let Some(min) = self.min.as_ref().and_then(Value::as_f64)
            && n < min
        {
            bail!("{} is below {}", show(shown), self.min.as_ref().unwrap());
        }
        if let Some(max) = self.max.as_ref().and_then(Value::as_f64)
            && n > max
        {
            bail!("{} is above {}", show(shown), self.max.as_ref().unwrap());
        }
        Ok(())
    }

    fn string<'a>(&self, value: &'a Value) -> Result<&'a str> {
        let Value::String(s) = value else { bail!("{} is not a string", show(value)) };
        if s.len() > MAX_STRING {
            bail!("a string of {} bytes (at most {MAX_STRING})", s.len());
        }
        if !printable(s) {
            bail!("{s:?} holds a character that is not printable ASCII");
        }
        Ok(s)
    }

    fn list<'a>(&self, value: &'a Value) -> Result<&'a Vec<Value>> {
        let Value::Array(list) = value else { bail!("{} is not a list", show(value)) };
        if list.len() > MAX_LIST {
            bail!("a list of {} entries (at most {MAX_LIST})", list.len());
        }
        Ok(list)
    }
}

fn show(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 60 { format!("{}…", s.chars().take(57).collect::<String>()) } else { s }
}

/// A `maps` entry: a map id 0 to 65535 or a range `"a-b"`; the ids it names.
fn map_entry(v: &Value) -> Result<(u32, u32)> {
    match v {
        Value::Number(n) => match n.as_u64() {
            Some(id) if id <= 65535 => Ok((id as u32, id as u32)),
            _ => bail!("{n} is not a map id (0 to 65535)"),
        },
        Value::String(s) => {
            let parse = |t: &str| t.trim().parse::<u32>().ok().filter(|n| *n <= 65535);
            match s.split_once('-').and_then(|(a, b)| Some((parse(a)?, parse(b)?))) {
                Some((a, b)) if a <= b => Ok((a, b)),
                _ => bail!("{s:?} is not a map range (\"a-b\", ids 0 to 65535, a not above b)"),
            }
        }
        other => bail!("{} is not a map id or range", show(other)),
    }
}

/// Whether `value` is valid for the option declared by `spec` (a manifest option as JSON): the checker the data editor and the
/// config file use (section 3.4).
pub fn check_option(spec: &Value, value: &Value) -> Result<()> {
    OptionSpec::parse(spec)?.check(value)
}

/// An option with the value in force.
#[derive(Clone, Debug)]
pub struct ResolvedOption {
    pub key: String,
    pub spec: OptionSpec,
    pub value: Value,
}

/// The options at their defaults (what `check` and the dry run use).
pub fn defaults(specs: &[(String, OptionSpec)]) -> Vec<ResolvedOption> {
    specs.iter().map(|(key, spec)| ResolvedOption { key: key.clone(), spec: spec.clone(), value: spec.default.clone() }).collect()
}

/// `game.options`: a read-only table, `maps` as a set (`options.maps[35] == true`), lists as arrays.
pub(crate) fn lua_table(lua: &mlua::Lua, options: &[ResolvedOption]) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    for o in options {
        let v = match (o.spec.ty, &o.value) {
            (OptType::Maps, Value::Array(entries)) => {
                let set = lua.create_table()?;
                for e in entries {
                    if let Ok((a, b)) = map_entry(e) {
                        for id in a..=b {
                            set.raw_set(id, true)?;
                        }
                    }
                }
                mlua::Value::Table(set)
            }
            (_, v) => json_to_lua(lua, v)?,
        };
        table.raw_set(o.key.as_str(), v)?;
    }
    readonly(lua, table)
}

fn json_to_lua(lua: &mlua::Lua, v: &Value) -> mlua::Result<mlua::Value> {
    Ok(match v {
        Value::Null => mlua::Value::Nil,
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => mlua::Value::Number(i as f64),
            None => mlua::Value::Number(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => mlua::Value::String(lua.create_string(s)?),
        Value::Array(a) => {
            let t = lua.create_table()?;
            for (i, e) in a.iter().enumerate() {
                t.raw_set(i + 1, json_to_lua(lua, e)?)?;
            }
            mlua::Value::Table(t)
        }
        Value::Object(_) => mlua::Value::Nil,
    })
}

/// A proxy table over `inner` that refuses writes.
fn readonly(lua: &mlua::Lua, inner: mlua::Table) -> mlua::Result<mlua::Table> {
    let proxy = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.raw_set("__index", inner.clone())?;
    meta.raw_set(
        "__newindex",
        lua.create_function(|_, _: mlua::MultiValue| -> mlua::Result<()> { Err(mlua::Error::runtime("game.options is read-only")) })?,
    )?;
    meta.raw_set("__metatable", false)?;
    // `pairs` and `next` over the proxy would see nothing, so scripts walking the options use the keys they know; `ipairs` of a
    // list option works through `__index`.
    proxy.set_metatable(Some(meta))?;
    Ok(proxy)
}
