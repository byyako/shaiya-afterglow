//! Operator (GM) commands, the pure rules (`docs/original/gm-commands.md`, `docs/plans/done/gm-commands.md`): who may send what, the staff
//! status written to the account, what `/set` allows and changes, which maps a GM may move into, the faction call and the bans.

use std::time::{Duration, Instant};

use shaiya_proto::admin as ap;

use crate::combat::{class_stat, max_level};
use crate::enter::ExpTable;

/// What to do with an `Fxxx` packet from a connection of a given staff level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Carry the command out.
    Allowed,
    /// Answer `F501 opcode` (deviation 1: the original closes the connection).
    Refused,
    /// Close the connection: a player sent a staff opcode, as the original does.
    Close,
}

/// The opcodes this server carries out (the original's cases plus deviations 14 to 17).
fn is_built(opcode: u16) -> bool {
    use ap::*;
    matches!(
        opcode,
        OP_CHAT_NORMAL
            | OP_CHAT_WHISPER
            | OP_CHAT_TRADE
            | OP_CHAT_GUILD
            | OP_CHAT_PARTY
            | OP_CHAT_UNION
            | OP_SET_LEVEL
            | OP_CLEAR_LEVEL
            | OP_SET
            | OP_MAKE_ITEM
            | OP_RECALL_ITEM
            | OP_MOB_MAKE
            | OP_MOB_REMOVE
            | OP_SERVER_ON
            | OP_SERVER_OFF
            | OP_SHUTDOWN
            | OP_ACCOUNT_KICK
            | OP_ACCOUNT_BAN
            | OP_ACCOUNT_UNBAN
            | OP_NPC_MAKE
            | OP_NPC_REMOVE
            | OP_REMOVE_TARGETED
            | OP_BAN
            | OP_UNBAN
            | OP_QUEST_ON
            | OP_QUEST_OFF
            | OP_EVENT_ADD
            | OP_EVENT_REMOVE
            | OP_INVISIBLE
            | OP_VISIBLE
            | OP_UNTOUCHABLE
            | OP_TOUCHABLE
            | OP_WARNING
            | OP_WATCH
            | OP_MOVE_TO_PLAYER
            | OP_MOVE_TO_SPOT
            | OP_LISTEN
            | OP_LISTEN_OFF
            | OP_SILENCE
            | OP_SILENCE_OFF
            | OP_STOP
            | OP_STOP_OFF
            | OP_MOVE_BIND
            | OP_MOVE_GUILD
            | OP_GUILD_GRADE
            | OP_GUILD_RANK
            | OP_GUILD_POINTS
            | OP_PVP_ON
            | OP_PVP_OFF
            | OP_CALL
            | OP_UQ_GET
            | OP_UQ_ANSWER
            | OP_UQ_ANSWER_TO
            | OP_UQ_COUNT
            | OP_CURE
            | OP_CLEAR_BAGS
            | OP_CLEAR_EQUIPMENT
            | OP_KICK
            | OP_QUEST_ADD
            | OP_QUEST_REMOVE
            | OP_HEAL
            | OP_CURE_ALL
            | OP_QUIRY
            | OP_SUMMON
            | OP_SUMMON_TO
            | OP_NOTICE_STAFF
            | OP_NOTICE_VIEW
            | OP_NOTICE_TO
            | OP_NOTICE_ZONE
            | OP_NOTICE_COUNTRY
            | OP_NOTICE_ALL
            | OP_SUMMON_PARTY
            | OP_AUCTION_RECALL
            | OP_AUCTION_SEARCH
    )
}

/// The staff level a GM listening to a player's chat needs (`docs/original/gm-commands.md` section 8.4): above it the copies stop and
/// the link is dropped (`CUser::PacketChatE` 0x4704bc), as `FA0B` needs the `FAxx` family's level anyway.
pub const LISTEN_LEVEL_MAX: u8 = 5;

/// What a listening GM is told a copied line was (`CUser::SendListenChat`'s first text, at each call in `PacketChatE` and
/// `PacketAdminChat`), with the chat log's type of the same line (`SendChatLogA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heard {
    /// `0x1101`, `F101`.
    Normal,
    /// `0x1102`, `F102`: with the target's name.
    Whisper,
    /// `0x1103`, `F103`: the faction's (the GM's: everyone's) trade channel, which the original calls `ALL`.
    All,
    /// `0x1104`, `F104`, in a guild.
    Guild,
    /// `0x1105`, `F105`, in a party; `0x1112`, `F10A`, the union's leaders.
    Party,
    /// `0x1107`.
    Shout,
    /// `0x1108`, and a normal line with a Message To Server armed: everyone on the server.
    Trade,
    /// `0x1111`, the faction on the map.
    Zone,
}

impl Heard {
    /// The prefix's first text. `ZONE` is sent with 5 bytes by the original, its zero inside the prefix, so that the client's copy
    /// ends there; here its 4 letters only (deviation 22).
    pub fn kind(self) -> &'static [u8] {
        match self {
            Heard::Normal => b"NORMAL",
            Heard::Whisper => b"TO",
            Heard::All => b"ALL",
            Heard::Guild => b"GUILD",
            Heard::Party => b"PARTY",
            Heard::Shout => b"SHOUT",
            Heard::Trade => b"TRADE",
            Heard::Zone => b"ZONE",
        }
    }

    /// The chat log's type: 1 normal, 2 whisper, 3 guild, 4 party, 5 to all, 6 shout, 7 zone.
    pub fn chat_type(self) -> u8 {
        match self {
            Heard::Normal => 1,
            Heard::Whisper => 2,
            Heard::Guild => 3,
            Heard::Party => 4,
            Heard::All | Heard::Trade => 5,
            Heard::Shout => 6,
            Heard::Zone => 7,
        }
    }
}

/// `level` is the connection's staff level now (1 to 10 staff, 11 a player).
pub fn check(level: u8, opcode: u16) -> Check {
    if level > crate::enter::STAFF_PRIVILEGE_MAX {
        return Check::Close;
    }
    if !is_built(opcode) {
        return Check::Refused;
    }
    // `/pvpon` and `/pvpoff` are open to every staff level (deviation 15): the client sends them with no check of its own.
    if opcode == ap::OP_PVP_ON || opcode == ap::OP_PVP_OFF {
        return Check::Allowed;
    }
    match ap::family(opcode) {
        Some(limit) if level <= limit => Check::Allowed,
        _ => Check::Refused,
    }
}

/// The account `status` a staff level is saved as (`usp_Save_User_Status_E`): `level * 16` for levels 1 to 5, else 0.
pub fn status_of(level: u8) -> i16 {
    if (1..=5).contains(&level) { i16::from(level) * 16 } else { 0 }
}

/// Whether a GM of `gm_level` may empty, kick or change the quests of a character whose staff level is `target_level` (11 for a
/// player), deviation 7 of part 2: staff of the GM's level or higher are spared, the GM itself is not.
pub fn may_touch(gm_level: u8, target_level: u8, itself: bool) -> bool {
    itself || target_level > gm_level
}

/// A question's time as `GetSystemTime` packs the local time (`docs/original/gm-commands-2.md` section 3.1): `((((((year - 16) * 16 +
/// month) * 32 + day) * 32 + hour) * 64 + minute) * 64 + second` in 32 bits.
pub fn packed_time(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> u32 {
    shaiya_proto::world::pack_time(year, month, day, hour, minute, second)
}

/// The packed local time of a unix time (`packed_time` of the local clock).
pub fn packed_from_unix(unix: i64) -> u32 {
    use chrono::{Datelike, TimeZone, Timelike};
    match chrono::Local.timestamp_opt(unix, 0).single() {
        Some(t) => {
            packed_time(t.year().clamp(0, 9999) as u16, t.month() as u8, t.day() as u8, t.hour() as u8, t.minute() as u8, t.second() as u8)
        }
        None => 0,
    }
}

/// The result whisper of `/event on|off` (`F80D`, `F80E`; section 4.5 of part 2): `cure ok ->Name`, or `... failed ->Name`, with its zero.
pub fn cure_line(opcode: u16, ok: bool, name: &[u8]) -> Vec<u8> {
    let what = match (opcode == ap::OP_CURE_ALL, ok) {
        (false, true) => "cure ok ->",
        (false, false) => "cure failed ->",
        (true, true) => "cure, buff,debuff clear ok ->",
        (true, false) => "cure, buff, debuff clear failed ->",
    };
    let mut line = what.as_bytes().to_vec();
    line.extend_from_slice(name);
    line.push(0);
    line
}

/// An answer to a player as whisper lines (deviation 2 of part 2): the text up to its zero cut into pieces of at most 120 bytes, each
/// with a zero added. Nothing for an empty text.
pub fn answer_lines(text: &[u8]) -> Vec<Vec<u8>> {
    let text = text.split(|&b| b == 0).next().unwrap_or_default();
    text.chunks(ANSWER_LINE)
        .map(|piece| {
            let mut line = piece.to_vec();
            line.push(0);
            line
        })
        .collect()
}

/// The most text bytes of one answer line.
const ANSWER_LINE: usize = 120;

/// The five notices a GM may repeat (`stAdminNotice`, section 5.4 of part 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// `F907` to those in view.
    View,
    /// `F908` to one player.
    To,
    /// `F909` to the zone.
    Zone,
    /// `F90A` to a country.
    Country,
    /// `F90B` to everyone.
    All,
}

impl NoticeKind {
    pub fn of_opcode(opcode: u16) -> Option<Self> {
        match opcode {
            ap::OP_NOTICE_VIEW => Some(Self::View),
            ap::OP_NOTICE_TO => Some(Self::To),
            ap::OP_NOTICE_ZONE => Some(Self::Zone),
            ap::OP_NOTICE_COUNTRY => Some(Self::Country),
            ap::OP_NOTICE_ALL => Some(Self::All),
            _ => None,
        }
    }

    pub fn opcode(self) -> u16 {
        match self {
            Self::View => ap::OP_NOTICE_VIEW,
            Self::To => ap::OP_NOTICE_TO,
            Self::Zone => ap::OP_NOTICE_ZONE,
            Self::Country => ap::OP_NOTICE_COUNTRY,
            Self::All => ap::OP_NOTICE_ALL,
        }
    }

    fn slot(self) -> usize {
        self as usize
    }
}

/// One repeating notice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repeat {
    pub due: Instant,
    pub period: Duration,
    /// The text with its zero.
    pub text: Vec<u8>,
    /// `To`: the character id; `Country`: the country; else 0.
    pub target: u32,
}

/// A GM connection's repeating notices, one slot per kind.
#[derive(Debug, Default)]
pub struct Notices([Option<Repeat>; 5]);

impl Notices {
    /// Starts the repeat of `kind`, replacing the one it had. A period of 0 stops it instead (a repeat of 0 seconds is never stored).
    pub fn set(&mut self, kind: NoticeKind, repeat: Repeat) {
        self.0[kind.slot()] = (!repeat.period.is_zero()).then_some(repeat);
    }

    pub fn stop(&mut self, kind: NoticeKind) {
        self.0[kind.slot()] = None;
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// When the first repeat falls due.
    pub fn next_due(&self) -> Option<Instant> {
        self.0.iter().flatten().map(|r| r.due).min()
    }

    /// The repeats due at `now`, earliest first, each moved to `now + period` (a late pass does not catch up).
    pub fn due(&mut self, now: Instant) -> Vec<(NoticeKind, Repeat)> {
        let kinds = [NoticeKind::View, NoticeKind::To, NoticeKind::Zone, NoticeKind::Country, NoticeKind::All];
        let mut out = Vec::new();
        for kind in kinds {
            if let Some(repeat) = &mut self.0[kind.slot()]
                && repeat.due <= now
            {
                out.push((kind, repeat.clone()));
                repeat.due = now + repeat.period;
            }
        }
        out.sort_by_key(|(_, r)| r.due);
        out
    }

    /// The text and target of a kind's repeat, if one is set.
    pub fn get(&self, kind: NoticeKind) -> Option<&Repeat> {
        self.0[kind.slot()].as_ref()
    }
}

/// The levels `/apower` may give (deviation 2): 1 to 10 staff, 11 a player.
pub fn may_set_level(level: u8) -> bool {
    (1..=11).contains(&level)
}

/// Whether a GM of `gm_level` may ban or unban an account whose `status` is given (deviation 14): an account that is staff of the GM's
/// level or higher is refused (staff level = bits 4 to 6 of the status, 0 for none).
pub fn may_ban(gm_level: u8, status: i16) -> bool {
    let staff = ((status >> 4) & 7) as u8;
    staff == 0 || staff > gm_level
}

/// What `/set` can change, numbered as the client sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Grow,
    Level,
    Money,
    StatPoint,
    SkillPoint,
    Str,
    Dex,
    Rec,
    Int,
    Luc,
    Wis,
    Honor,
    Vg,
    Cg,
    Og,
    Ig,
    Exp,
    Kills,
    Deaths,
}

impl Stat {
    pub fn from_u8(n: u8) -> Option<Self> {
        use Stat::*;
        [Grow, Level, Money, StatPoint, SkillPoint, Str, Dex, Rec, Int, Luc, Wis, Honor, Vg, Cg, Og, Ig, Exp, Kills, Deaths]
            .get(usize::from(n))
            .copied()
    }

    /// The position in the stored stat order (str, dex, rec, int, wis, luc) of one of the six base stats.
    pub fn base_index(self) -> Option<usize> {
        match self {
            Stat::Str => Some(0),
            Stat::Dex => Some(1),
            Stat::Rec => Some(2),
            Stat::Int => Some(3),
            Stat::Wis => Some(4),
            Stat::Luc => Some(5),
            _ => None,
        }
    }
}

/// The `/set` number of the stat at a stored position (the other way round of `Stat::base_index`).
fn number_of_base(index: usize) -> u8 {
    [5, 6, 7, 8, 10, 9][index]
}

/// The most a point stat or a base stat may be set to, and VG (`0x6FFF`).
pub const SET_MAX: u32 = 0x6FFF;

/// What `/set` reads and writes of a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub job: u8,
    pub grow: u8,
    pub level: u16,
    pub exp: u32,
    pub money: u32,
    pub stat_point: u16,
    pub skill_point: u16,
    /// STR, DEX, REC, INT, WIS, LUC as stored.
    pub stats: [u16; 6],
    pub honor: u16,
    pub vg: i16,
    pub cg: u8,
    pub og: u8,
    pub ig: u8,
    pub kills: u32,
    pub deaths: u32,
}

/// A `/set` that was accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetResult {
    pub sheet: Sheet,
    /// The stored value, which the target is told.
    pub value: u32,
    /// The level-up announcements are due (a level, a grow or an exp that moved the level).
    pub announce: bool,
    /// `F701` packets for the target before its own (`stat`, `value`): exp and the job's main stat for a level, the level for an exp.
    pub extras: Vec<(u8, u32)>,
}

/// `/set` (`docs/original/gm-commands.md` section 7.1 with deviations 3 and 4). `None` when the value is refused.
pub fn apply_set(sheet: &Sheet, stat: Stat, value: u32, exp: &ExpTable, kill_cap: u32) -> Option<SetResult> {
    let mut s = *sheet;
    let mut stored = value;
    let mut announce = false;
    let mut extras = Vec::new();
    match stat {
        Stat::Grow => {
            if value > 3 || (value == 0 && s.level > 30) {
                return None;
            }
            s.grow = value as u8;
            announce = true;
        }
        Stat::Level => {
            if value == 0 || value > 60 || (s.grow == 0 && value > 30) || value == u32::from(s.level) {
                return None;
            }
            s.level = value as u16;
            s.exp = exp.total(s.grow, s.level - 1);
            announce = true;
        }
        Stat::Money => s.money = value,
        Stat::StatPoint | Stat::SkillPoint => {
            if value > SET_MAX {
                return None;
            }
            if stat == Stat::StatPoint { s.stat_point = value as u16 } else { s.skill_point = value as u16 }
        }
        Stat::Str | Stat::Dex | Stat::Rec | Stat::Int | Stat::Luc | Stat::Wis => {
            let index = stat.base_index()?;
            if value > SET_MAX || value == u32::from(s.stats[index]) {
                return None;
            }
            s.stats[index] = value as u16;
        }
        Stat::Honor => {
            if value > 0xFFFF {
                return None;
            }
            s.honor = value as u16;
        }
        Stat::Vg => {
            if value > SET_MAX {
                return None;
            }
            s.vg = value as i16;
        }
        Stat::Cg | Stat::Og | Stat::Ig => {
            if value > 255 {
                return None;
            }
            match stat {
                Stat::Cg => s.cg = value as u8,
                Stat::Og => s.og = value as u8,
                _ => s.ig = value as u8,
            }
        }
        Stat::Exp => {
            let top = max_level(s.grow);
            if value > exp.total(s.grow, top) {
                return None;
            }
            // The first level below the highest whose end is past the value, else the highest level (deviation 4).
            let level = (1..top).find(|&n| value < exp.total(s.grow, n)).unwrap_or(top);
            s.exp = value;
            if level != s.level {
                s.level = level;
                announce = true;
                extras.push((1, u32::from(level)));
            }
        }
        Stat::Kills | Stat::Deaths => {
            stored = value.min(kill_cap);
            if stat == Stat::Kills { s.kills = stored } else { s.deaths = stored }
        }
    }
    if stat == Stat::Level {
        // The exp of the level's start, then the job's main stat as the target has it now (deviation 3).
        extras.push((16, s.exp));
        if let Some(index) = class_stat(s.job) {
            extras.push((number_of_base(index), u32::from(s.stats[index])));
        }
    }
    Some(SetResult { sheet: s, value: stored, announce, extras })
}

/// Maps a GM may be moved into by `FA12`: party maps (3), ranking maps (5) and house maps (6).
pub fn gm_zone_allowed(create_type: u8) -> bool {
    matches!(create_type, 3 | 5 | 6)
}

/// A staff member's own switches (`+0x5774`, `+0x5775`, and `pvp` of deviation 15): staff enter with all three off, players have them on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub visible: bool,
    pub attackable: bool,
    pub pvp: bool,
}

impl Flags {
    pub fn on_entering(staff: bool) -> Self {
        Self { visible: !staff, attackable: !staff, pvp: !staff }
    }
}

/// `FA09` reaches only a player on a map without a create type.
pub fn amove_allowed(create_type: u8) -> bool {
    create_type == 0
}

/// `FA18` (`CZone::UpdateAllCall`): of the players `(id, level, country, position)` on the GM's map, the ones to move to the GM, each with
/// a delay of `1000 + rand % 10000` ms: not staff (level 11 or more), not the GM, of that country and farther than 10 (x and z).
pub fn call_targets(
    gm: (u32, [f32; 3]),
    country: u8,
    players: &[(u32, u8, u8, [f32; 3])],
    mut rand: impl FnMut() -> u32,
) -> Vec<(u32, Duration)> {
    players
        .iter()
        .filter(|&&(id, level, c, at)| {
            id != gm.0 && level > crate::enter::STAFF_PRIVILEGE_MAX && c == country && crate::zone::distance(at, gm.1) > 10.0
        })
        .map(|&(id, ..)| (id, Duration::from_millis(1000 + u64::from(rand() % 10000))))
        .collect()
}

/// When a chat or move ban ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Until {
    Forever,
    At(Instant),
}

impl Until {
    /// Whether the ban is over at `now` (`Forever` never).
    pub fn passed(self, now: Instant) -> bool {
        matches!(self, Until::At(end) if end <= now)
    }
}

/// `FA0D` / `FA0F`: 0 minutes is for ever.
pub fn ban_until(minutes: u16, now: Instant) -> Until {
    if minutes == 0 { Until::Forever } else { Until::At(now + Duration::from_secs(u64::from(minutes) * 60)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Totals of 100 per level in every mode.
    fn table() -> ExpTable {
        ExpTable::from_rows((1..=60).flat_map(|level| (1..=4).map(move |grow| (level, grow, 100))))
    }

    fn sheet() -> Sheet {
        Sheet {
            job: 2,
            grow: 1,
            level: 30,
            exp: 2900,
            money: 5,
            stat_point: 10,
            skill_point: 4,
            stats: [20, 30, 40, 50, 60, 70],
            honor: 1,
            vg: 2,
            cg: 3,
            og: 4,
            ig: 5,
            kills: 6,
            deaths: 7,
        }
    }

    fn set(stat: Stat, value: u32) -> Option<SetResult> {
        apply_set(&sheet(), stat, value, &table(), 1000)
    }

    #[test]
    fn who_may_send_what() {
        assert_eq!(check(11, ap::OP_INVISIBLE), Check::Close);
        assert_eq!(check(11, 0xF3FF), Check::Close);
        assert_eq!(check(2, ap::OP_SET_LEVEL), Check::Refused);
        assert_eq!(check(1, ap::OP_SET_LEVEL), Check::Allowed);
        assert_eq!(check(5, ap::OP_MAKE_ITEM), Check::Refused);
        assert_eq!(check(2, ap::OP_MAKE_ITEM), Check::Allowed);
        assert_eq!(check(5, ap::OP_INVISIBLE), Check::Allowed);
        assert_eq!(check(6, ap::OP_INVISIBLE), Check::Refused);
        // Deviation 15: the pvp switch is for every staff level.
        assert_eq!((check(5, ap::OP_PVP_ON), check(9, ap::OP_PVP_OFF)), (Check::Allowed, Check::Allowed));
        // Deviation 14: the bans follow their families' limits.
        assert_eq!((check(2, ap::OP_ACCOUNT_BAN), check(3, ap::OP_ACCOUNT_UNBAN)), (Check::Allowed, Check::Refused));
        assert_eq!((check(3, ap::OP_BAN), check(4, ap::OP_UNBAN)), (Check::Allowed, Check::Refused));
        assert_eq!((check(3, ap::OP_QUEST_OFF), check(3, ap::OP_EVENT_ADD)), (Check::Allowed, Check::Allowed));
        // Not built, or not defined: answered, not closed (deviation 1).
        for op in
            [0xFA03, 0xFA04, 0xF205, 0xF800, 0xF807, 0xF808, 0xF811, 0xF902, 0xF903, 0xF90D, 0xF106, 0xF109, 0xF10B, 0xF301, 0xF501, 0xFB01]
        {
            assert_eq!(check(1, op), Check::Refused, "{op:04X}");
        }
        for op in [0xF101, 0xF102, 0xF103, 0xF104, 0xF105, 0xF10A] {
            assert_eq!(check(10, op), Check::Allowed, "{op:04X}");
        }
        // The auction house's commands (`docs/original/auction.md` section 12) follow the FAxx family's limit.
        for op in [ap::OP_AUCTION_RECALL, ap::OP_AUCTION_SEARCH] {
            assert_eq!(check(ap::family(op).unwrap(), op), Check::Allowed, "{op:04X}");
        }
    }

    #[test]
    fn part_two_commands_follow_their_families() {
        for op in [0xF201, 0xF202, 0xF203, 0xF204] {
            assert_eq!((check(10, op), check(11, op)), (Check::Allowed, Check::Close), "{op:04X}");
        }
        for op in [0xF801, 0xF802, 0xF803, 0xF804, 0xF80B, 0xF80C, 0xF80D, 0xF80E] {
            assert_eq!((check(3, op), check(4, op)), (Check::Allowed, Check::Refused), "{op:04X}");
        }
        for op in [0xF901, 0xF904, 0xF905, 0xF906, 0xF907, 0xF908, 0xF909, 0xF90A, 0xF90B, 0xF90C] {
            assert_eq!((check(4, op), check(5, op)), (Check::Allowed, Check::Refused), "{op:04X}");
        }
        // 0x1201 is a player's opcode: not an `Fxxx` at all.
        assert_eq!(check(1, ap::OP_USER_QUESTION), Check::Refused);
    }

    #[test]
    fn staff_of_the_same_or_a_higher_rank_are_spared() {
        assert!(!may_touch(2, 1, false) && !may_touch(2, 2, false));
        assert!(may_touch(2, 3, false) && may_touch(2, 11, false));
        assert!(may_touch(2, 1, true), "the GM itself");
    }

    #[test]
    fn the_question_time_is_packed_like_the_original() {
        // (2009 - 16) = 1993 keeps its low 6 bits, 9; 9 << 26 = 603979776, month 7 << 22 = 29360128, day 15 << 17 = 1966080,
        // hour 12 << 12 = 49152, minute 34 << 6 = 2176, second 56: 635357368.
        assert_eq!(packed_time(2009, 7, 15, 12, 34, 56), 635_357_368);
    }

    #[test]
    fn the_event_whispers_name_the_result_and_the_target() {
        assert_eq!(cure_line(ap::OP_HEAL, true, b"Bob"), b"cure ok ->Bob\0");
        assert_eq!(cure_line(ap::OP_HEAL, false, b"Bob"), b"cure failed ->Bob\0");
        assert_eq!(cure_line(ap::OP_CURE_ALL, true, b"Bob"), b"cure, buff,debuff clear ok ->Bob\0");
        assert_eq!(cure_line(ap::OP_CURE_ALL, false, b"Bob"), b"cure, buff, debuff clear failed ->Bob\0");
    }

    #[test]
    fn answers_are_cut_into_lines_of_120_bytes() {
        assert!(answer_lines(b"").is_empty() && answer_lines(b"\0").is_empty());
        assert_eq!(answer_lines(b"hello\0"), vec![b"hello\0".to_vec()]);
        let long = [b'a'; 250];
        let lines = answer_lines(&long);
        assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [121, 121, 11]);
        assert!(lines.iter().all(|l| l.last() == Some(&0)));
        assert_eq!(answer_lines(&[b'b'; 120]).len(), 1);
    }

    #[test]
    fn repeats_fall_due_in_order_without_catching_up() {
        let t = Instant::now();
        let repeat = |secs: u64, due_in: u64| Repeat {
            due: t + Duration::from_secs(due_in),
            period: Duration::from_secs(secs),
            text: b"hi\0".to_vec(),
            target: 0,
        };
        let mut n = Notices::default();
        assert!(n.next_due().is_none() && n.is_empty());
        n.set(NoticeKind::Zone, repeat(10, 10));
        n.set(NoticeKind::View, repeat(5, 5));
        n.set(NoticeKind::All, repeat(0, 1));
        assert_eq!(n.next_due(), Some(t + Duration::from_secs(5)));
        assert!(n.get(NoticeKind::All).is_none(), "a repeat of 0 seconds is never stored");
        assert!(n.due(t + Duration::from_secs(4)).is_empty());
        // A pass 30 s late: both are due once, and each is next due a period after the pass.
        let due = n.due(t + Duration::from_secs(30));
        assert_eq!(due.iter().map(|d| d.0).collect::<Vec<_>>(), [NoticeKind::View, NoticeKind::Zone]);
        assert_eq!(n.next_due(), Some(t + Duration::from_secs(35)));
        assert!(n.due(t + Duration::from_secs(31)).is_empty(), "no catch-up burst");
        n.stop(NoticeKind::View);
        assert_eq!(n.next_due(), Some(t + Duration::from_secs(40)));
        n.stop(NoticeKind::Zone);
        assert!(n.is_empty());
        assert_eq!(NoticeKind::of_opcode(ap::OP_NOTICE_COUNTRY), Some(NoticeKind::Country));
        assert_eq!(NoticeKind::To.opcode(), ap::OP_NOTICE_TO);
        assert_eq!(NoticeKind::of_opcode(ap::OP_NOTICE_STAFF), None);
    }

    #[test]
    fn staff_levels_become_account_status() {
        assert_eq!((status_of(1), status_of(5), status_of(6), status_of(11)), (16, 80, 0, 0));
        assert!(may_set_level(1) && may_set_level(11) && !may_set_level(0) && !may_set_level(12));
    }

    #[test]
    fn a_ban_spares_staff_of_the_gms_level_or_higher() {
        assert!(!may_ban(2, 16) && !may_ban(2, 32));
        assert!(may_ban(2, 48) && may_ban(2, 0) && may_ban(2, 2));
    }

    #[test]
    fn set_refuses_what_the_table_refuses() {
        assert!(set(Stat::Grow, 4).is_none());
        assert!(apply_set(&Sheet { level: 31, ..sheet() }, Stat::Grow, 0, &table(), 0).is_none(), "easy mode only up to level 30");
        assert!(set(Stat::Grow, 0).is_some());
        assert!(set(Stat::Level, 61).is_none());
        assert!(set(Stat::Level, 30).is_none(), "the same level");
        assert!(apply_set(&Sheet { grow: 0, ..sheet() }, Stat::Level, 31, &table(), 0).is_none());
        assert!(set(Stat::StatPoint, 0x7000).is_none() && set(Stat::SkillPoint, 0x7000).is_none());
        assert!(set(Stat::Str, 0x7000).is_none());
        assert!(set(Stat::Str, 20).is_none(), "the same stat");
        assert!(set(Stat::Honor, 0x10000).is_none() && set(Stat::Vg, 0x7000).is_none());
        assert!(set(Stat::Cg, 256).is_none() && set(Stat::Og, 256).is_none() && set(Stat::Ig, 256).is_none());
        assert!(set(Stat::Exp, table().total(1, 60) + 1).is_none());
        assert!(apply_set(&Sheet { grow: 0, ..sheet() }, Stat::Exp, table().total(0, 30) + 1, &table(), 0).is_none());
    }

    #[test]
    fn a_level_sets_the_levels_start_and_keeps_the_points() {
        let r = set(Stat::Level, 35).unwrap();
        assert_eq!((r.sheet.level, r.sheet.exp, r.value, r.announce), (35, 3400, 35, true));
        assert_eq!((r.sheet.stat_point, r.sheet.skill_point, r.sheet.stats), (10, 4, [20, 30, 40, 50, 60, 70]));
        // Job 2 (ranger): DEX, number 6, with the target's own value.
        assert_eq!(r.extras, vec![(16, 3400), (6, 30)]);
    }

    #[test]
    fn exp_sets_the_level_that_holds_it() {
        let r = set(Stat::Exp, 3950).unwrap();
        assert_eq!((r.sheet.level, r.sheet.exp, r.announce, r.extras.clone()), (40, 3950, true, vec![(1, 40)]));
        let r = set(Stat::Exp, 2950).unwrap();
        assert!(!r.announce && r.extras.is_empty(), "inside the level it already has");
        // Past level 60's start: level 60, not the grow number.
        let r = set(Stat::Exp, 5950).unwrap();
        assert_eq!((r.sheet.level, r.sheet.exp), (60, 5950));
        let r = apply_set(&Sheet { grow: 0, ..sheet() }, Stat::Exp, 2950, &table(), 0).unwrap();
        assert_eq!(r.sheet.level, 30);
    }

    #[test]
    fn the_main_stat_is_the_targets_job() {
        let jobs = [(0, 5, 20), (1, 7, 40), (2, 6, 30), (3, 9, 70), (4, 8, 50), (5, 10, 60)];
        for (job, number, value) in jobs {
            let r = apply_set(&Sheet { job, ..sheet() }, Stat::Level, 35, &table(), 0).unwrap();
            assert_eq!(r.extras[1], (number, value), "job {job}");
        }
    }

    #[test]
    fn stats_use_the_set_numbering() {
        let r = set(Stat::Luc, 9).unwrap();
        assert_eq!(r.sheet.stats, [20, 30, 40, 50, 60, 9]);
        let r = set(Stat::Wis, 9).unwrap();
        assert_eq!(r.sheet.stats, [20, 30, 40, 50, 9, 70]);
        assert_eq!(Stat::from_u8(9), Some(Stat::Luc));
        assert_eq!(Stat::from_u8(19), None);
    }

    #[test]
    fn kills_and_deaths_are_clamped_and_other_values_land() {
        assert_eq!(set(Stat::Kills, 5000).unwrap().value, 1000);
        assert_eq!(set(Stat::Deaths, 9).unwrap().sheet.deaths, 9);
        assert_eq!(set(Stat::Money, 123456).unwrap().sheet.money, 123456);
        assert_eq!(set(Stat::Cg, 255).unwrap().sheet.cg, 255);
        assert_eq!(set(Stat::Grow, 3).unwrap().sheet.grow, 3);
    }

    #[test]
    fn maps_for_moves() {
        assert!(gm_zone_allowed(3) && gm_zone_allowed(5) && gm_zone_allowed(6));
        assert!(!gm_zone_allowed(0) && !gm_zone_allowed(1) && !gm_zone_allowed(4));
        assert!(amove_allowed(0) && !amove_allowed(3));
    }

    #[test]
    fn bans_end() {
        let now = Instant::now();
        assert_eq!(ban_until(0, now), Until::Forever);
        assert!(!Until::Forever.passed(now + Duration::from_secs(1_000_000)));
        let end = ban_until(2, now);
        assert!(!end.passed(now + Duration::from_secs(119)) && end.passed(now + Duration::from_secs(120)));
    }

    #[test]
    fn the_call_skips_staff_the_gm_the_other_country_and_anyone_near() {
        let gm = (1, [100.0, 0.0, 100.0]);
        let players = [
            (1, 11, 0, [500.0, 0.0, 500.0]), // the GM
            (2, 11, 0, [500.0, 0.0, 500.0]), // called
            (3, 5, 0, [500.0, 0.0, 500.0]),  // staff
            (4, 11, 1, [500.0, 0.0, 500.0]), // the other country
            (5, 11, 0, [105.0, 0.0, 108.0]), // within 10
            (6, 11, 0, [111.0, 0.0, 100.0]), // farther in x
            (7, 11, 0, [100.0, 0.0, 89.0]),  // farther in z
        ];
        let mut n = 0;
        let called = call_targets(gm, 0, &players, || {
            n += 1;
            n * 7
        });
        assert_eq!(called.iter().map(|c| c.0).collect::<Vec<_>>(), vec![2, 6, 7]);
        assert_eq!(called[0].1, Duration::from_millis(1007));
        assert!(called.iter().all(|c| c.1 >= Duration::from_secs(1) && c.1 < Duration::from_secs(11)));
    }
}
