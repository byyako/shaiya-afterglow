//! Values a script stores (`docs/plans/ext-5-lua.md` sections 7.5 and 7.6): per character and per extension (`p:get`, `p:set`), and the
//! extension's own (`game.get`, `game.set`). They live in memory: a character's are loaded at the login, the extension's at the start and
//! at `reload`; a `set` changes the cache at once and queues a write that a writer task coalesces by key and sends every 2 seconds.
//! A value is never read back from the database while it is cached.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use serde_json::Value;

/// Keys per character per extension, and keys in an extension's own (section 7.5).
pub const MAX_CHAR_KEYS: usize = 200;
pub const MAX_GLOBAL_KEYS: usize = 10_000;
/// The longest string value, bytes.
pub const MAX_TEXT: usize = 1000;

/// A key: `^[a-z0-9_.:-]{1,64}$`.
pub fn valid_key(key: &str) -> bool {
    (1..=64).contains(&key.len())
        && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

/// A value a script may store: a boolean, a finite number, or a string of at most [`MAX_TEXT`] bytes. Whole numbers are kept as
/// integers so that they read back as written.
pub fn check_value(value: &Value) -> Result<(), String> {
    match value {
        Value::Bool(_) => Ok(()),
        Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => Ok(()),
        Value::String(s) if s.len() <= MAX_TEXT => Ok(()),
        Value::String(s) => Err(format!("a string of {} bytes (at most {MAX_TEXT})", s.len())),
        _ => Err("only booleans, numbers and strings are stored".to_owned()),
    }
}

/// One change to the database: a value set (`Some`) or deleted (`None`). `character` is `None` for the extension's own values.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptWrite {
    pub extension: String,
    pub character: Option<u32>,
    pub key: String,
    pub value: Option<Value>,
}

type Slot = (String, Option<u32>, String);

#[derive(Default)]
struct Entry {
    session: u64,
    /// `(extension, key)` → value.
    values: HashMap<(String, String), Value>,
}

#[derive(Default)]
struct Inner {
    chars: HashMap<u32, Entry>,
    next_gen: u64,
    /// The extension's own values.
    globals: HashMap<String, HashMap<String, Value>>,
    /// Writes not sent yet, by what they change: the last one stands.
    pending: BTreeMap<Slot, Option<Value>>,
}

/// The cache and the write-behind queue, shared by every extension's state.
#[derive(Default)]
pub struct Store {
    inner: Mutex<Inner>,
}

impl Store {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A character's values come in at its login: the rows the database has, with the writes not sent yet over them. Returns the
    /// generation to hand back to [`Store::end_char`].
    pub fn begin_char(&self, id: u32, rows: Vec<(String, String, Value)>) -> u64 {
        let mut inner = self.lock();
        inner.next_gen += 1;
        let session = inner.next_gen;
        let mut entry = Entry { session, values: rows.into_iter().map(|(e, k, v)| ((e, k), v)).collect() };
        for ((extension, character, key), value) in &inner.pending {
            if *character == Some(id) {
                match value {
                    Some(v) => entry.values.insert((extension.clone(), key.clone()), v.clone()),
                    None => entry.values.remove(&(extension.clone(), key.clone())),
                };
            }
        }
        inner.chars.insert(id, entry);
        session
    }

    /// The character left: its values go from the cache unless it has logged in again since (`session` is the login's). Writes not sent
    /// yet stay queued.
    pub fn end_char(&self, id: u32, session: u64) {
        let mut inner = self.lock();
        if inner.chars.get(&id).is_some_and(|e| e.session == session) {
            inner.chars.remove(&id);
        }
    }

    pub fn is_loaded(&self, id: u32) -> bool {
        self.lock().chars.contains_key(&id)
    }

    pub fn get(&self, extension: &str, id: u32, key: &str) -> Option<Value> {
        self.lock().chars.get(&id)?.values.get(&(extension.to_owned(), key.to_owned())).cloned()
    }

    /// Sets (or, with `None`, deletes) a character's value. An error is the Lua error's text.
    pub fn set(&self, extension: &str, id: u32, key: &str, value: Option<Value>) -> Result<(), String> {
        if !valid_key(key) {
            return Err(format!("{key:?} is not a key (^[a-z0-9_.:-]{{1,64}}$)"));
        }
        if let Some(v) = &value {
            check_value(v)?;
        }
        let mut inner = self.lock();
        let Some(entry) = inner.chars.get_mut(&id) else {
            return Err("the player is not online here: its values are not loaded".to_owned());
        };
        let slot = (extension.to_owned(), key.to_owned());
        match &value {
            Some(v) => {
                if !entry.values.contains_key(&slot) && entry.values.keys().filter(|(e, _)| e == extension).count() >= MAX_CHAR_KEYS {
                    return Err(format!("more than {MAX_CHAR_KEYS} keys for one character"));
                }
                entry.values.insert(slot, v.clone());
            }
            None => {
                entry.values.remove(&slot);
            }
        }
        inner.pending.insert((extension.to_owned(), Some(id), key.to_owned()), value);
        Ok(())
    }

    /// An extension's own values come in at the start and at `reload`, with the writes not sent yet over them.
    pub fn set_globals(&self, extension: &str, rows: Vec<(String, Value)>) {
        let mut inner = self.lock();
        let mut map: HashMap<String, Value> = rows.into_iter().collect();
        for ((e, character, key), value) in &inner.pending {
            if e == extension && character.is_none() {
                match value {
                    Some(v) => map.insert(key.clone(), v.clone()),
                    None => map.remove(key),
                };
            }
        }
        inner.globals.insert(extension.to_owned(), map);
    }

    /// Forgets the globals of extensions that are not in `keep` (removed or disabled).
    pub fn retain_globals(&self, keep: &[String]) {
        self.lock().globals.retain(|e, _| keep.contains(e));
    }

    pub fn get_global(&self, extension: &str, key: &str) -> Option<Value> {
        self.lock().globals.get(extension)?.get(key).cloned()
    }

    pub fn set_global(&self, extension: &str, key: &str, value: Option<Value>) -> Result<(), String> {
        if !valid_key(key) {
            return Err(format!("{key:?} is not a key (^[a-z0-9_.:-]{{1,64}}$)"));
        }
        if let Some(v) = &value {
            check_value(v)?;
        }
        let mut inner = self.lock();
        let map = inner.globals.entry(extension.to_owned()).or_default();
        match &value {
            Some(v) => {
                if !map.contains_key(key) && map.len() >= MAX_GLOBAL_KEYS {
                    return Err(format!("more than {MAX_GLOBAL_KEYS} keys for the extension"));
                }
                map.insert(key.to_owned(), v.clone());
            }
            None => {
                map.remove(key);
            }
        }
        inner.pending.insert((extension.to_owned(), None, key.to_owned()), value);
        Ok(())
    }

    /// Takes the writes not sent yet (the writer task's round).
    pub fn take_pending(&self) -> Vec<ScriptWrite> {
        std::mem::take(&mut self.lock().pending)
            .into_iter()
            .map(|((extension, character, key), value)| ScriptWrite { extension, character, key, value })
            .collect()
    }

    /// Puts back writes that did not go through; a newer write of the same key stands.
    pub fn requeue(&self, batch: Vec<ScriptWrite>) {
        let mut inner = self.lock();
        for w in batch {
            inner.pending.entry((w.extension, w.character, w.key)).or_insert(w.value);
        }
    }

    pub fn pending_len(&self) -> usize {
        self.lock().pending.len()
    }
}

/// A Lua value as a stored one; `Ok(None)` for `nil` (a delete).
pub fn from_lua(value: &mlua::Value) -> Result<Option<Value>, String> {
    Ok(Some(match value {
        mlua::Value::Nil => return Ok(None),
        mlua::Value::Boolean(b) => Value::Bool(*b),
        mlua::Value::Integer(i) => Value::from(*i as i64),
        mlua::Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 => Value::from(*n as i64),
        mlua::Value::Number(n) if n.is_finite() => serde_json::Number::from_f64(*n).map(Value::Number).ok_or("not a finite number")?,
        mlua::Value::Number(_) => return Err("a number that is not finite is not stored".to_owned()),
        mlua::Value::String(s) => Value::String(s.to_str().map_err(|_| "a string that is not text is not stored".to_owned())?.to_string()),
        other => return Err(format!("a {} is not stored (booleans, numbers and strings are)", other.type_name())),
    }))
}

/// A stored value as a Lua one.
pub fn to_lua(lua: &mlua::Lua, value: &Value) -> mlua::Result<mlua::Value> {
    Ok(match value {
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Number(n) => mlua::Value::Number(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => mlua::Value::String(lua.create_string(s)?),
        _ => mlua::Value::Nil,
    })
}
