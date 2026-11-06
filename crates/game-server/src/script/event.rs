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
    Login {
        p: PlayerRef,
    },
    Logout {
        p: PlayerRef,
    },
    MapEnter {
        p: PlayerRef,
        map: u16,
    },
    MapLeave {
        p: PlayerRef,
        map: u16,
    },
    /// `kill` and `boss_kill`: the player of most damage, the monster, the players the kill was shared with.
    Kill {
        p: PlayerRef,
        mob: MobInfo,
        near: Vec<PlayerRef>,
    },
    PvpKill {
        killer: PlayerRef,
        victim: PlayerRef,
        last_hit: Option<PlayerRef>,
        near: Vec<PlayerRef>,
    },
    Death {
        victim: PlayerRef,
        killer: Killer,
        pvp: bool,
    },
}

/// What a script does to a player (section 4.4): carried out by that player's own connection.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    GiveItem { item_type: u8, type_id: u8, count: u8 },
    TakeItem { item_type: u8, type_id: u8, count: u8 },
    GiveGold(u32),
    TakeGold(u32),
    AddBuff { skill: u16, level: u8 },
    Teleport { map: u16, position: [f32; 3] },
}

/// An order waiting for its connection: for which extension, and the callback to tell the result to.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
    pub extension: String,
    pub callback: Option<u64>,
    pub order: Order,
}

/// What an order came to, as the callback gets it: a count (items given, gold given) or whether it was done.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Count(u32),
    Done(bool),
}

/// One unit of work for the script thread (section 4.2).
#[derive(Clone, Debug)]
pub enum Job {
    /// The handlers of every enabled extension that registered for the event, in install order.
    Event { event: super::Event, payload: Payload },
    /// A player's `!word args`, for the extension that registered the word.
    Command { extension: String, word: String, p: PlayerRef, args: String },
    /// An order a connection carried out (or could not): the callback of the extension that gave it.
    OrderDone { extension: String, callback: u64, outcome: Outcome },
    /// The character's stored values leave the cache (after its `logout` handlers ran): `session` is the login they belong to.
    Forget { id: u32, session: u64 },
    /// The set of scripts was swapped: look at the timers again.
    Wake,
}
