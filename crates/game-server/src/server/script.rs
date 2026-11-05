//! The server's side of the extensions' scripts (`docs/plans/ext-5-lua.md`): the world the API acts on (`ContextWorld`), loading and
//! starting the scripts, the `!word` commands, the events the connection raises (login, logout, zone entry and exit), the console's
//! `scripts`. The scripts themselves run on their own thread (`crate::script::runner`) or, for a point, on the asking task; nothing here
//! runs one while a lock is held (section 4.3).

use std::sync::Weak;

use super::*;
use crate::script::event::{Job, Payload, PlayerRef};
use crate::script::sources::{Resolution, ScriptSources, resolve_options};
use crate::script::world::{PlayerView, ScriptWorld};
use crate::script::{Event, ExtInput, LoadContext, ScriptSet};

/// A null-terminated chat text.
fn zeroed(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

/// The world of the scripts on this server. It holds the context weakly: the scripts are the context's, not the other way round.
pub struct ContextWorld<S> {
    ctx: Weak<Context<S>>,
}

impl<S: Backend> ScriptWorld for ContextWorld<S> {
    fn player(&self, id: u32) -> Option<PlayerView> {
        self.ctx.upgrade()?.zones.script_view(id)
    }

    fn find(&self, name: &str) -> Option<PlayerView> {
        self.ctx.upgrade()?.zones.script_find(name)
    }

    fn online(&self, max: usize) -> Vec<PlayerView> {
        self.ctx.upgrade().map(|c| c.zones.script_online(max)).unwrap_or_default()
    }

    fn online_count(&self) -> usize {
        self.ctx.upgrade().map_or(0, |c| c.zones.script_online_count())
    }

    fn party(&self, id: u32) -> Vec<u32> {
        self.ctx.upgrade().map(|c| c.zones.script_party(id)).unwrap_or_default()
    }

    fn whisper(&self, id: u32, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_to_char(id, &chat::whisper(false, SERVER_NAME, &zeroed(text)));
        }
    }

    fn notice_to(&self, id: u32, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_to_char(id, &ap::notice(crate::admin::NoticeKind::To.opcode(), &zeroed(text)));
        }
    }

    fn notice_all(&self, text: &str) {
        if let (Some(c), Some(packet)) = (self.ctx.upgrade(), ap::world_notice(text)) {
            c.zones.send_world(&packet);
        }
    }

    fn notice_faction(&self, country: u8, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_country(country, None, &ap::notice(crate::admin::NoticeKind::Country.opcode(), &zeroed(text)));
        }
    }

    fn notice_map(&self, map: u16, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_zone(map, &ap::notice(crate::admin::NoticeKind::Zone.opcode(), &zeroed(text)));
        }
    }

    fn banner(&self, name: &str, text: &str) {
        if let Some(c) = self.ctx.upgrade() {
            c.zones.send_all(&chat::by_name(chat::OP_ALL_SERVER, name.as_bytes(), &zeroed(text)));
        }
    }

    fn item_name(&self, item_type: u8, type_id: u8) -> Option<String> {
        let c = self.ctx.upgrade()?;
        c.names.items.get(&(item_type, type_id)).cloned()
    }

    fn mob_name(&self, mob_id: u16) -> Option<String> {
        let c = self.ctx.upgrade()?;
        c.names.mobs.get(&mob_id).cloned()
    }

    fn tell_staff(&self, text: &str) {
        let Some(c) = self.ctx.upgrade() else { return };
        for id in c.zones.script_staff_upto(c.config.console_level) {
            for piece in text.as_bytes().chunks(120) {
                c.zones.send_to_char(id, &chat::whisper(false, SERVER_NAME, &zeroed(&String::from_utf8_lossy(piece))));
            }
        }
    }

    fn commands(&self) -> Vec<(String, String, u8)> {
        let Some(c) = self.ctx.upgrade() else { return Vec::new() };
        let set = c.zones.ext_scripts();
        let mut out: Vec<(String, String, u8)> = set
            .extensions()
            .iter()
            .filter(|e| !e.is_disabled())
            .flat_map(|e| e.commands().into_iter().map(|(word, meta)| (word, meta.help, meta.staff)))
            .collect();
        out.sort();
        out
    }
}

impl<S: Backend> Context<S> {
    /// Binds the scripts' world to this context; the context must live in an `Arc` (called by `start_scripts`).
    pub fn attach_scripts(self: &Arc<Self>) {
        let _ = self.script_me.set(Arc::downgrade(self));
    }

    /// The world the scripts act on; a world with nobody in it until `attach_scripts` ran.
    pub fn script_world(&self) -> Arc<dyn ScriptWorld> {
        match self.script_me.get() {
            Some(weak) => Arc::new(ContextWorld { ctx: weak.clone() }),
            None => Arc::new(crate::script::world::NullWorld),
        }
    }

    /// Loads the scripts of `sources` with the options resolved in `resolution` (section 9.1): every enabled extension in a state of its
    /// own, an extension that does not load listed in the set's `failed`. CPU work: it runs the scripts' top levels.
    pub fn load_script_set(&self, sources: &ScriptSources, resolution: &Resolution) -> ScriptSet {
        let inputs: Vec<ExtInput> = resolution.inputs(sources);
        let weak = self.script_me.get().cloned();
        let exists: Arc<dyn Fn(u8, u8) -> bool + Send + Sync> = match weak {
            Some(weak) => Arc::new(move |t, i| weak.upgrade().is_some_and(|c| c.data.items.get(t, i).is_some())),
            None => Arc::new(|_, _| true),
        };
        ScriptSet::load(&inputs, LoadContext { item_exists: Some(exists), world: Some(self.script_world()), ..LoadContext::default() }, self.config.script_memory_mb)
    }

    /// Puts `set` in force and starts the script thread (once): the start of the server (`main`) and the tests call it.
    pub fn start_scripts(self: &Arc<Self>, set: ScriptSet) {
        self.attach_scripts();
        self.zones.start_script_thread();
        self.zones.set_ext_scripts(set);
    }
}

/// The scripts of the database and the file, loaded for the start of the server (section 9.1): an error in an option stops the start
/// (`Err`), an extension that does not load runs without scripts (a line in the log).
pub fn load_at_start<S: Backend>(ctx: &Arc<Context<S>>, sources: &ScriptSources) -> anyhow::Result<ScriptSet> {
    ctx.attach_scripts();
    let resolution = resolve_options(sources)?;
    for warning in &resolution.warnings {
        tracing::warn!("{warning}");
    }
    let set = ctx.load_script_set(sources, &resolution);
    for (name, why) in set.failed() {
        tracing::error!(extension = %name, "scripts not loaded: {why}");
    }
    for ext in set.extensions() {
        let s = ext.summary();
        tracing::info!(extension = %ext.name, handlers = s.handlers(), timers = s.timers, "scripts loaded");
    }
    Ok(set)
}

// ------------------------------------------------------------------------------------------------------------ the commands

/// `!word args` (`docs/plans/ext-5-lua.md` section 7.2): after an optional 8-byte colour marker (`<<NNNcCC`, which the client adds with
/// a chat colour), a `!`, a word of lower case letters, digits and `_` (any case is read as lower case), then a space or the end. The
/// text is cut at its first zero. `! name text` (a space after the `!`) is the client's whisper prefix, not a command.
pub fn parse_command(text: &[u8]) -> Option<(String, String)> {
    let text = text.split(|&b| b == 0).next().unwrap_or(text);
    let rest = match text.strip_prefix(b"<<") {
        Some(_) => {
            let marker = text.get(..8)?;
            let digits = |b: &[u8]| !b.is_empty() && b.iter().all(u8::is_ascii_digit);
            if !(digits(&marker[2..5]) && marker[5] == b'c' && digits(&marker[6..8])) {
                return None;
            }
            &text[8..]
        }
        None => text,
    };
    let rest = rest.strip_prefix(b"!")?;
    let end = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
    let word = &rest[..end];
    if word.is_empty() || !word.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_') {
        return None;
    }
    let word = String::from_utf8_lossy(word).to_ascii_lowercase();
    let args = String::from_utf8_lossy(&rest[end..]).trim().to_owned();
    Some((word, args))
}

/// Whether the command registered for staff levels `1..=staff` (0: everyone) may be used by a player whose staff level is `level` (0 for
/// a player, 1 to 10 for staff).
pub fn may_use(staff: u8, level: u8) -> bool {
    staff == 0 || (1..=staff).contains(&level)
}

/// A chat line that is a command a script registered: queued for the extension, and `true` (the line is not chat). Anything else,
/// an unregistered word, a staff command typed by a player, a disabled extension's, stays chat (`false`).
pub(super) fn command<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, text: &[u8]) -> bool {
    let set = ctx.zones.ext_scripts();
    if !set.has_commands() || !text.contains(&b'!') {
        return false;
    }
    let Some((word, args)) = parse_command(text) else { return false };
    let Some((ext, meta)) = set.command(&word) else { return false };
    let level = if state.staff { state.staff_level } else { 0 };
    if ext.is_disabled() || !may_use(meta.staff, level) {
        return false;
    }
    tracing::debug!(id = character.id, extension = %ext.name, word, "script command");
    ctx.zones.script_job(Job::Command { extension: ext.name.clone(), word, p: PlayerRef { id: character.id, name: character.name.clone() }, args });
    true
}

// ------------------------------------------------------------------------------------------------------------ the events

/// What the connection tells the scripts about itself: the character, which of its events were raised.
#[derive(Debug, Default)]
pub(super) struct Track {
    pub id: u32,
    pub name: String,
    /// `login` was raised (and `logout` is due).
    pub login: bool,
    /// The zone `map_enter` was raised for (and `map_leave` is due).
    pub map: Option<u16>,
}

impl Track {
    fn player(&self) -> PlayerRef {
        PlayerRef { id: self.id, name: self.name.clone() }
    }
}

/// The character entered a zone: `login` the first time, `map_enter` every time (section 5).
pub(super) fn entered<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    let map = state.map;
    if !state.script.login {
        state.script.login = true;
        let level = if state.staff { Some(state.staff_level) } else { None };
        ctx.zones.set_script_staff(state.script.id, level);
        ctx.zones.raise(Event::Login, || Payload::Login { p: state.script.player() });
    }
    state.script.map = Some(map);
    ctx.zones.raise(Event::MapEnter, || Payload::MapEnter { p: state.script.player(), map });
}

/// The character left a zone (`leave_zone`).
pub(super) fn left_zone<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    if let Some(map) = state.script.map.take() {
        ctx.zones.raise(Event::MapLeave, || Payload::MapLeave { p: state.script.player(), map });
    }
}

/// The character is out of the world: `map_leave` for the zone it was in, then `logout` (once, only after a `login`).
pub(super) fn gone<S: Backend>(ctx: &Context<S>, state: &mut WorldState) {
    left_zone(ctx, state);
    if std::mem::take(&mut state.script.login) {
        ctx.zones.set_script_staff(state.script.id, None);
        ctx.zones.raise(Event::Logout, || Payload::Logout { p: state.script.player() });
    }
}

// ------------------------------------------------------------------------------------------------------------ the console

/// `scripts`: one line per extension with scripts, `<name> <state> <handlers> timers <n> spawns <n> faults <n>`.
pub(super) fn answer<S>(ctx: &Context<S>) -> Vec<String> {
    let set = ctx.zones.ext_scripts();
    if set.is_empty() {
        return vec!["no scripts".to_owned()];
    }
    let mut out = Vec::new();
    for ext in set.extensions() {
        let s = ext.summary();
        let state = match ext.disabled_reason() {
            Some(reason) => format!("disabled ({reason})"),
            None => "running".to_owned(),
        };
        out.push(cut_line(format!("{} {state} {} handlers timers {} spawns {} faults {}", ext.name, s.handlers(), s.timers, ext.spawn_count(), ext.fault_count())));
    }
    for (name, why) in set.failed() {
        out.push(cut_line(format!("{name} not loaded ({why})")));
    }
    out
}

/// A console line cut to the whisper width.
fn cut_line(mut line: String) -> String {
    const WIDTH: usize = 120;
    if line.len() > WIDTH {
        let mut end = WIDTH - 1;
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        line.truncate(end);
        line.push('…');
    }
    line
}
