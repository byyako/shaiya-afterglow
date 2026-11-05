//! What an event carries to the script thread (`docs/plans/ext-5-lua.md` sections 4.2, 5 and 7.3): plain data, taken where the event was
//! raised, so the thread never reads a zone to build it.

/// A player named by an event: the handle a script gets is made of these two.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRef {
    pub id: u32,
    pub name: String,
}

/// `mob` of the `kill` and `boss_kill` events (section 7.3).
#[derive(Clone, Debug, PartialEq)]
pub struct MobInfo {
    /// The monster definition's id.
    pub id: u16,
    /// The monster's object id.
    pub uid: u32,
    pub name: String,
    pub level: u16,
    pub map: u16,
    pub x: f32,
    pub z: f32,
    pub boss: bool,
    /// The extension that spawned it (`game.spawn`), if one did.
    pub spawned: Option<String>,
}

/// Who killed a player (`death`'s `by` and `killer`).
#[derive(Clone, Debug, PartialEq)]
pub enum Killer {
    Player(PlayerRef),
    /// A monster, by its object id.
    Monster(u32),
    /// An NPC (a guard), by its object id.
    Npc(u32),
}

/// The arguments of one event.
#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    Login { p: PlayerRef },
    Logout { p: PlayerRef },
    MapEnter { p: PlayerRef, map: u16 },
    MapLeave { p: PlayerRef, map: u16 },
    /// `kill` and `boss_kill`: the player of most damage, the monster, the players the kill was shared with.
    Kill { p: PlayerRef, mob: MobInfo, near: Vec<PlayerRef> },
    PvpKill { killer: PlayerRef, victim: PlayerRef, last_hit: Option<PlayerRef>, near: Vec<PlayerRef> },
    Death { victim: PlayerRef, killer: Killer, pvp: bool },
}

/// One unit of work for the script thread (section 4.2).
#[derive(Clone, Debug)]
pub enum Job {
    /// The handlers of every enabled extension that registered for the event, in install order.
    Event { event: super::Event, payload: Payload },
    /// A player's `!word args`, for the extension that registered the word.
    Command { extension: String, word: String, p: PlayerRef, args: String },
    /// The set of scripts was swapped: look at the timers again.
    Wake,
}
