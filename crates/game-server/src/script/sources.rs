//! What the database and the config file hold for scripts (`docs/plans/ext-5-lua.md` sections 3.4 and 9.1): the enabled extensions'
//! scripts and option specs, the data editor's option values, and the layers laid over each other. Pure: [`ScriptSources`] is read by
//! `config::read_script_sources` (PostgreSQL) or set by a test (the memory backend).

use anyhow::{Result, bail};
use serde_json::Value;

use super::options::{OptionSpec, ResolvedOption};
use super::{ExtInput, options};
use crate::config::{FileOption, Layer};

/// One enabled extension with scripts, as `content.ext_scripts` and `content.ext_options` hold it.
#[derive(Clone, Debug, Default)]
pub struct ScriptExtension {
    pub name: String,
    /// `(path, source)` in the manifest's order.
    pub scripts: Vec<(String, String)>,
    /// `(key, spec)` by key.
    pub options: Vec<(String, OptionSpec)>,
    /// The values the extension stored for itself (`content.script_globals`), `(key, value)`; read at start and at `reload`.
    pub globals: Vec<(String, Value)>,
}

/// The database's side of the scripts: read at start and at `reload`.
#[derive(Clone, Debug, Default)]
pub struct ScriptSources {
    /// The enabled extensions with scripts, in install order.
    pub extensions: Vec<ScriptExtension>,
    /// The data editor's option values: `(ext.<extension>.<option>, the value as JSON text, change set)`.
    pub editor: Vec<(String, String, Option<i32>)>,
    /// Every installed extension and whether it is enabled (for the warnings of a layer that names one that does not count).
    pub installed: Vec<(String, bool)>,
    /// The config file's `[ext.<name>]` tables (read by `config::read_file`).
    pub file: Option<FileOptions>,
}

/// The file's side of the options: where they were read from, and each with its line.
#[derive(Clone, Debug, Default)]
pub struct FileOptions {
    pub path: String,
    pub options: Vec<FileOption>,
}

/// One extension's options after the layers: each with the layer its value came from.
#[derive(Clone, Debug)]
pub struct ExtOptions {
    pub extension: String,
    pub options: Vec<(ResolvedOption, Layer)>,
}

/// The result of [`resolve_options`].
#[derive(Clone, Debug, Default)]
pub struct Resolution {
    pub extensions: Vec<ExtOptions>,
    /// Lines for the log: a layer names an extension or option that does not count.
    pub warnings: Vec<String>,
}

fn why_not(sources: &ScriptSources, extension: &str) -> String {
    match sources.installed.iter().find(|(n, _)| n == extension) {
        None => format!("the extension {extension} is not installed"),
        Some((_, false)) => format!("the extension {extension} is disabled"),
        Some(_) => format!("the extension {extension} has no scripts"),
    }
}

fn unknown_option(e: &ExtOptions, key: &str) -> String {
    let known: Vec<&str> = e.options.iter().map(|(o, _)| o.key.as_str()).collect();
    if known.is_empty() {
        format!("the extension {} has no options", e.extension)
    } else {
        format!("the extension {} has no option {key:?} (its options: {})", e.extension, known.join(", "))
    }
}

/// Lays the config file's `[ext.<name>]` and the data editor's `ext.<name>.<key>` over the manifests' defaults (section 3.4), checking
/// each value against its spec. An unknown option in the file, or a value out of its spec in either layer, is an error naming the place;
/// a layer for an extension that does not count (not installed, disabled, no scripts) is a warning, and so is an editor value for an
/// option the extension no longer has (an upgrade took it away).
pub fn resolve_options(sources: &ScriptSources) -> Result<Resolution> {
    let mut out = Resolution::default();
    for e in &sources.extensions {
        out.extensions.push(ExtOptions {
            extension: e.name.clone(),
            options: e
                .options
                .iter()
                .map(|(key, spec)| (ResolvedOption { key: key.clone(), spec: spec.clone(), value: spec.default.clone() }, Layer::Default))
                .collect(),
        });
    }
    if let Some(file) = &sources.file {
        for fo in &file.options {
            let place = format!("{} line {}", file.path, fo.line);
            let Some(ext) = out.extensions.iter_mut().find(|e| e.extension == fo.extension) else {
                out.warnings.push(format!("{place}: [ext.{}] is ignored: {}", fo.extension, why_not(sources, &fo.extension)));
                continue;
            };
            let Some(slot) = ext.options.iter().position(|(o, _)| o.key == fo.key) else {
                bail!("{place}: ext.{}.{}: {}", fo.extension, fo.key, unknown_option(ext, &fo.key));
            };
            let o = &mut ext.options[slot];
            o.0.spec.check(&fo.value).map_err(|e| anyhow::anyhow!("{place}: ext.{}.{}: {e:#}", fo.extension, fo.key))?;
            o.0.value = fo.value.clone();
            o.1 = Layer::File;
        }
    }
    for (name, text, change_set) in &sources.editor {
        let place = match change_set {
            Some(n) => format!("data editor (change set {n})"),
            None => "data editor".to_owned(),
        };
        let Some((extension, key)) = name.strip_prefix("ext.").and_then(|r| r.split_once('.')) else {
            bail!("{place}: {name}: not an extension option (ext.<extension>.<option>)");
        };
        let Some(ext) = out.extensions.iter_mut().find(|e| e.extension == extension) else {
            out.warnings.push(format!("{place}: {name} is ignored: {}", why_not(sources, extension)));
            continue;
        };
        let Some(slot) = ext.options.iter().position(|(o, _)| o.key == key) else {
            out.warnings.push(format!("{place}: {name} is ignored: {}", unknown_option(ext, key)));
            continue;
        };
        let value: Value =
            serde_json::from_str(text).map_err(|e| anyhow::anyhow!("{place}: {name}: the value {text:?} is not JSON ({e})"))?;
        let o = &mut ext.options[slot];
        o.0.spec.check(&value).map_err(|e| anyhow::anyhow!("{place}: {name}: {e:#}"))?;
        o.0.value = value;
        o.1 = Layer::Editor(*change_set);
    }
    Ok(out)
}

impl Resolution {
    /// The loader's input for every extension, in install order.
    pub fn inputs(&self, sources: &ScriptSources) -> Vec<ExtInput> {
        sources
            .extensions
            .iter()
            .map(|e| ExtInput {
                name: e.name.clone(),
                scripts: e.scripts.clone(),
                options: self
                    .extensions
                    .iter()
                    .find(|x| x.extension == e.name)
                    .map(|x| x.options.iter().map(|(o, _)| o.clone()).collect())
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// The options as `game-server config` prints them, after the settings: a table per extension, each value with its layer.
    pub fn show(&self) -> String {
        let mut out = String::new();
        for e in &self.extensions {
            if e.options.is_empty() {
                continue;
            }
            out.push_str(&format!("\n[ext.{}]\n", e.extension));
            for (o, layer) in &e.options {
                let comment = match layer {
                    Layer::Default => "extension default".to_owned(),
                    other => other.to_string(),
                };
                out.push_str(&format!("{:<44} # {comment}\n", format!("{} = {}", o.key, toml_text(&o.value))));
            }
        }
        out
    }
}

/// A JSON value as TOML text (printable ASCII only, as options are).
fn toml_text(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s:?}"),
        Value::Array(items) => format!("[{}]", items.iter().map(toml_text).collect::<Vec<_>>().join(", ")),
        other => other.to_string(),
    }
}

/// The commented example `game-server config --example` ends with (an extension's options in the file).
pub const EXAMPLE: &str = r#"# An enabled extension's options (its README lists them; `game-server config` shows each with where its value comes from):
# [ext.kill-streak]
# bounty_gold = 100000
# announce_at = [5, 10, 15]
"#;

/// Whether `value` is valid for the option declared by `spec`; for the data editor (`options::check_option` on JSON).
pub fn check_text(spec: &Value, text: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(text).map_err(|e| anyhow::anyhow!("{text:?} is not JSON ({e})"))?;
    options::check_option(spec, &value)?;
    Ok(value)
}
