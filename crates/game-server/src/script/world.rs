//! The world a script acts on (`docs/plans/ext-5-lua.md` section 4.1): a trait object, so the script module needs no backend generics.
//! The server implements it for its `Context` (`server/script.rs`); `check`'s dry run and the pure tests use [`NullWorld`].

/// A player online on this server, as the reads of section 7.1 see it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerView {
    pub id: u32,
    pub name: String,
    pub level: u16,
    /// 0 light, 1 fury.
    pub country: u8,
    pub map: u16,
    pub position: [f32; 3],
    /// The number the client shows (the original's K1).
    pub kills: u32,
    pub job: u8,
    /// 0 for a player, else the staff level 1 to 10.
    pub staff: u8,
    pub guild: Option<u32>,
}

/// Everything the API reads or does outside its own state. Every method is called with no server lock held by the caller, and none
/// calls into a script (section 4.3).
pub trait ScriptWorld: Send + Sync {
    /// A player online here, by character id.
    fn player(&self, id: u32) -> Option<PlayerView>;
    /// A player online here by name, ignoring case.
    fn find(&self, name: &str) -> Option<PlayerView>;
    /// The players online here, at most `max`.
    fn online(&self, max: usize) -> Vec<PlayerView>;
    fn online_count(&self) -> usize;
    /// The ids of the party (or union) of `id` online here, itself included; empty when `id` is not online.
    fn party(&self, id: u32) -> Vec<u32>;
    /// A whisper from `Server` to one player.
    fn whisper(&self, id: u32, text: &str);
    /// The notice packets: to one player, everyone, a faction (0 light, 1 fury), one zone.
    fn notice_to(&self, id: u32, text: &str);
    fn notice_all(&self, text: &str);
    fn notice_faction(&self, country: u8, text: &str);
    fn notice_map(&self, map: u16, text: &str);
    /// The centre-screen line `0x1108` to everyone, from `name`.
    fn banner(&self, name: &str, text: &str);
    /// The names of definitions, when the server loaded them.
    fn item_name(&self, item_type: u8, type_id: u8) -> Option<String>;
    fn mob_name(&self, mob_id: u16) -> Option<String>;
    /// A line to each staff member online who may type console commands (a disabled extension).
    fn tell_staff(&self, text: &str);
    /// Leaves an order for the player's own connection; false when the player is not online here (nothing is left).
    fn order(&self, id: u32, queued: super::event::Queued) -> bool;
    /// Tells an extension's callback how an order came out (queued for the script thread, never run on the caller's stack).
    fn answer(&self, extension: &str, callback: u64, outcome: super::event::Outcome);
    /// `game.spawn`: monsters of `mob_id` near the spot, removed after `lifetime` seconds; their object ids (fewer than asked, or
    /// none, when the map, the spot or the extension's room says so).
    #[allow(clippy::too_many_arguments)]
    fn spawn(&self, extension: &str, mob_id: u16, count: usize, map: u16, x: f32, z: f32, radius: u32, lifetime: u32, exp: bool, drops: bool) -> Vec<u32>;
    /// The extension's monsters of `uids` taken away quietly / still alive; how many monsters it has alive.
    fn despawn(&self, extension: &str, uids: &[u32]) -> usize;
    fn alive(&self, extension: &str, uids: &[u32]) -> usize;
    fn spawn_count(&self, extension: &str) -> usize;
    /// `(word, help, staff)` of every command of every enabled extension, for a `!help`.
    fn commands(&self) -> Vec<(String, String, u8)>;
}

/// The world of a dry run and of the unit tests: nobody is online, nothing is sent.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullWorld;

impl ScriptWorld for NullWorld {
    fn player(&self, _: u32) -> Option<PlayerView> {
        None
    }
    fn find(&self, _: &str) -> Option<PlayerView> {
        None
    }
    fn online(&self, _: usize) -> Vec<PlayerView> {
        Vec::new()
    }
    fn online_count(&self) -> usize {
        0
    }
    fn party(&self, _: u32) -> Vec<u32> {
        Vec::new()
    }
    fn whisper(&self, _: u32, _: &str) {}
    fn notice_to(&self, _: u32, _: &str) {}
    fn notice_all(&self, _: &str) {}
    fn notice_faction(&self, _: u8, _: &str) {}
    fn notice_map(&self, _: u16, _: &str) {}
    fn banner(&self, _: &str, _: &str) {}
    fn item_name(&self, _: u8, _: u8) -> Option<String> {
        None
    }
    fn mob_name(&self, _: u16) -> Option<String> {
        None
    }
    fn tell_staff(&self, _: &str) {}
    fn spawn(&self, _: &str, _: u16, _: usize, _: u16, _: f32, _: f32, _: u32, _: u32, _: bool, _: bool) -> Vec<u32> {
        Vec::new()
    }
    fn despawn(&self, _: &str, _: &[u32]) -> usize {
        0
    }
    fn alive(&self, _: &str, _: &[u32]) -> usize {
        0
    }
    fn spawn_count(&self, _: &str) -> usize {
        0
    }
    fn order(&self, _: u32, _: super::event::Queued) -> bool {
        false
    }
    fn answer(&self, _: &str, _: u64, _: super::event::Outcome) {}
    fn commands(&self) -> Vec<(String, String, u8)> {
        Vec::new()
    }
}
