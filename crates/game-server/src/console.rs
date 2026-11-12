//! The operator console's commands, parsed (`docs/original/operator-console.md` sections 4 to 6): the words of section 5, their help
//! texts and the shutdown countdown's timing. Pure; `server/console.rs` runs them, for level-1 staff typing `.command` in the game
//! and for the `console` tool through `NOTIFY`.

use std::time::Duration;

/// Who is kicked, and how they are named (`/kickun`, `/kickuid`, `/kickcn`, `/kickcid`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KickBy {
    User(String),
    UserId(u32),
    Char(String),
    CharId(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// `help`, or `help <command>`.
    Help(Option<String>),
    Info,
    Notice(String),
    /// `quit [minutes]`.
    Quit(Option<u32>),
    QuitCancel,
    /// `restart [minutes]`: the `quit` countdown, then the process ends with `RESTART_STATUS` so that its supervisor starts it again.
    Restart(Option<u32>),
    /// `reload`: the monsters and drop tables, and the rules, read again (`docs/plans/done/data-editing.md` section 7,
    /// `docs/plans/ext-2-rules.md` section 4.6).
    Reload,
    /// `rules`: the rules in force with how often each matched (`docs/plans/ext-2-rules.md` section 4.7).
    Rules,
    /// `scripts`: the extensions' scripts in force with their state (`docs/plans/ext-5-lua.md` section 9.3).
    Scripts,
    AllOut,
    Close,
    Open,
    MaxUsers(u32),
    Kick(KickBy),
    Exp(f64),
    ExpOff,
    KillRate(u32),
    /// `switches::SHOP_ON`, `SHOP_OFF` or `SHOP_SILENT`.
    Shop(u8),
    KillCount(bool),
    Zones,
    Map(u16),
    /// `stats`: the figures of the `stats` line over the time since the last one.
    Stats,
}

/// A command's help: the word, its usage, what it does in one sentence, an example (without the dot).
struct Entry {
    word: &'static str,
    usage: &'static str,
    sentence: &'static str,
    example: &'static str,
}

const TABLE: [Entry; 20] = [
    Entry { word: "help", usage: "help [command]", sentence: "lists the commands, or tells how one is used.", example: "help kick" },
    Entry {
        word: "info",
        usage: "info",
        sentence: "shows the players, whether logins are open, the uptime and the switches.",
        example: "info",
    },
    Entry {
        word: "notice",
        usage: "notice <text>",
        sentence: "shows a notice to every player in the world.",
        example: "notice Maintenance in 10 minutes",
    },
    Entry {
        word: "quit",
        usage: "quit [minutes] | quit cancel",
        sentence: "counts down to a shutdown (at least 30 seconds) with notices, or cancels the count.",
        example: "quit 5",
    },
    Entry {
        word: "restart",
        usage: "restart [minutes]",
        sentence: "counts down as quit does, then ends with status 75 so that it is started again (quit cancel stops it).",
        example: "restart 5",
    },
    Entry {
        word: "reload",
        usage: "reload",
        sentence: "reloads the monsters, the drops and the rules: drops, gold, monster stats and rules change at once.",
        example: "reload",
    },
    Entry {
        word: "rules",
        usage: "rules",
        sentence: "lists the rules in force: name, type, where from, and how often each matched since the start or the last reload.",
        example: "rules",
    },
    Entry {
        word: "scripts",
        usage: "scripts",
        sentence: "lists the extensions' scripts: running, disabled after errors or not loaded, with handlers, timers and errors.",
        example: "scripts",
    },
    Entry { word: "allout", usage: "allout", sentence: "disconnects every player, saved; the server keeps running.", example: "allout" },
    Entry { word: "close", usage: "close", sentence: "stops new logins to this server (as /server off).", example: "close" },
    Entry { word: "open", usage: "open", sentence: "lets players log in again (as /server on).", example: "open" },
    Entry {
        word: "maxusers",
        usage: "maxusers <0-29999>",
        sentence: "refuses new connections while that many are open; those open stay.",
        example: "maxusers 200",
    },
    Entry {
        word: "kick",
        usage: "kick user <account> | userid <n> | char <name> | charid <n>",
        sentence: "disconnects an account or a character, saved.",
        example: "kick char ProbeHero",
    },
    Entry {
        word: "exp",
        usage: "exp <factor> | exp off",
        sentence: "multiplies the experience monsters give; 0 or more, 1 = normal.",
        example: "exp 1.5",
    },
    Entry {
        word: "killrate",
        usage: "killrate <1-100>",
        sentence: "sets how many kills each counted player kill adds.",
        example: "killrate 2",
    },
    Entry {
        word: "shop",
        usage: "shop on | off | silent",
        sentence: "opens the item mall, closes it with the usual failure, or makes it answer nothing.",
        example: "shop off",
    },
    Entry {
        word: "killcount",
        usage: "killcount on | off",
        sentence: "turns the factions' kill counts on or off.",
        example: "killcount off",
    },
    Entry { word: "zones", usage: "zones", sentence: "counts the instance zones now open.", example: "zones" },
    Entry { word: "map", usage: "map <id>", sentence: "shows the players and monsters in each zone of a map.", example: "map 1" },
    Entry {
        word: "stats",
        usage: "stats",
        sentence: "shows traffic, outboxes, the monster pass, lock waits, saves and memory since the last stats line or call.",
        example: "stats",
    },
];

/// The exit status of a `restart` (`quit` ends with 0): `docs/server/deploy.md`'s service units and `tools/game-server.sh` start the server
/// again after it, and after any other failure but not after a clean stop.
pub const RESTART_STATUS: i32 = 75;

/// The shutdown countdown's first notice (`/quitt`, section 1.1; the original's text in English).
pub const QUIT_SHORTLY: &str = "The server will shut down shortly. Please log out.";
/// The notice when a countdown is cancelled (an addition).
pub const QUIT_CANCELLED: &str = "The server shutdown was cancelled.";
/// The most minutes a countdown may take.
pub const QUIT_MAX_MINUTES: u32 = 1440;

/// A countdown notice with the seconds left (`/quitt`).
pub fn quit_notice(seconds: u64) -> String {
    format!("The server will shut down in {seconds} seconds. Please log out.")
}

/// How long a countdown takes: the minutes in seconds, at least 30 s (`/quitt`: `atoi × 60000`, at least 30000 ms).
pub fn quit_duration(minutes: Option<u32>) -> Duration {
    Duration::from_secs(u64::from(minutes.unwrap_or(0)) * 60).max(Duration::from_secs(30))
}

/// The wait after a countdown notice with `left` to go: 1 s while 10 s or less are left, 5 s while 30 s or less, else 10 s.
pub fn next_notice(left: Duration) -> Duration {
    Duration::from_secs(match left.as_secs_f64() {
        l if l <= 10.0 => 1,
        l if l <= 30.0 => 5,
        _ => 10,
    })
}

/// Every command with its arguments, one line each (`help`).
pub fn help() -> Vec<String> {
    TABLE.iter().map(|e| e.usage.to_owned()).collect()
}

/// One command's usage, what it does and an example (`help <command>`, and the answer to wrong arguments); `None` for an unknown word.
pub fn help_for(word: &str) -> Option<Vec<String>> {
    let entry = TABLE.iter().find(|e| e.word.eq_ignore_ascii_case(word))?;
    Some(vec![format!("usage: {}", entry.usage), format!("{} {}", entry.word, entry.sentence), format!("e.g. .{}", entry.example)])
}

/// The answer to wrong arguments: the command's help, the lines joined by a newline.
fn wrong(word: &str) -> String {
    help_for(word).map(|lines| lines.join("\n")).unwrap_or_default()
}

/// The text after a leading chat colour marker (`<<NNNcCC`, `docs/client/gm-and-commands.md` section 5a) and the terminating zero,
/// when it starts with a dot: the in-game way to type a command. `None` for any other line.
pub fn typed_command(text: &[u8]) -> Option<String> {
    let text = text.strip_suffix(&[0]).unwrap_or(text);
    let text = match text {
        [b'<', b'<', _, _, _, b'c', _, _, rest @ ..] => rest,
        _ => text,
    };
    let rest = text.strip_prefix(b".")?;
    Some(String::from_utf8_lossy(rest).trim().to_owned())
}

/// A command line (a leading dot is dropped) as a command, or the answer to give instead: `unknown command <word>; try help`, or a
/// known word's help when its arguments are wrong. Nothing is changed then.
pub fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim().trim_start_matches('.').trim();
    let (word, rest) = line.split_once(char::is_whitespace).map_or((line, ""), |(w, r)| (w, r.trim()));
    let lower = word.to_ascii_lowercase();
    let mut args = rest.split_whitespace();
    let (first, second, extra) = (args.next(), args.next(), args.next());
    let bad = || Err(wrong(&lower));
    match lower.as_str() {
        "help" | "?" if extra.is_none() && second.is_none() => Ok(Command::Help(first.map(str::to_owned))),
        "info" if rest.is_empty() => Ok(Command::Info),
        "notice" if !rest.is_empty() => Ok(Command::Notice(rest.to_owned())),
        "quit" => match (first, second) {
            (None, _) => Ok(Command::Quit(None)),
            (Some(w), None) if w.eq_ignore_ascii_case("cancel") => Ok(Command::QuitCancel),
            (Some(minutes), None) => match minutes.parse::<u32>() {
                Ok(m) if (1..=QUIT_MAX_MINUTES).contains(&m) => Ok(Command::Quit(Some(m))),
                _ => bad(),
            },
            _ => bad(),
        },
        "restart" => match (first, second) {
            (None, _) => Ok(Command::Restart(None)),
            (Some(minutes), None) => match minutes.parse::<u32>() {
                Ok(m) if (1..=QUIT_MAX_MINUTES).contains(&m) => Ok(Command::Restart(Some(m))),
                _ => bad(),
            },
            _ => bad(),
        },
        "reload" if rest.is_empty() => Ok(Command::Reload),
        "rules" if rest.is_empty() => Ok(Command::Rules),
        "scripts" if rest.is_empty() => Ok(Command::Scripts),
        "allout" if rest.is_empty() => Ok(Command::AllOut),
        "close" if rest.is_empty() => Ok(Command::Close),
        "open" if rest.is_empty() => Ok(Command::Open),
        "maxusers" => match first.map(str::parse::<u32>) {
            Some(Ok(n)) if n < 30000 && second.is_none() => Ok(Command::MaxUsers(n)),
            _ => bad(),
        },
        "kick" => match (first.map(str::to_ascii_lowercase).as_deref(), second, extra) {
            (Some("user"), Some(name), None) => Ok(Command::Kick(KickBy::User(name.to_owned()))),
            (Some("char"), Some(name), None) => Ok(Command::Kick(KickBy::Char(name.to_owned()))),
            (Some("userid"), Some(n), None) => match n.parse::<u32>() {
                Ok(n) if n > 0 => Ok(Command::Kick(KickBy::UserId(n))),
                _ => bad(),
            },
            (Some("charid"), Some(n), None) => match n.parse::<u32>() {
                Ok(n) if n > 0 => Ok(Command::Kick(KickBy::CharId(n))),
                _ => bad(),
            },
            _ => bad(),
        },
        "exp" => match (first, second) {
            (Some(w), None) if w.eq_ignore_ascii_case("off") => Ok(Command::ExpOff),
            (Some(f), None) => match f.parse::<f64>() {
                Ok(f) if f.is_finite() && f >= 0.0 => Ok(Command::Exp(f)),
                _ => bad(),
            },
            _ => bad(),
        },
        "killrate" => match first.map(str::parse::<u32>) {
            Some(Ok(n)) if (1..=100).contains(&n) && second.is_none() => Ok(Command::KillRate(n)),
            _ => bad(),
        },
        "shop" => match (first.map(str::to_ascii_lowercase).as_deref(), second) {
            (Some("on"), None) => Ok(Command::Shop(crate::switches::SHOP_ON)),
            (Some("off"), None) => Ok(Command::Shop(crate::switches::SHOP_OFF)),
            (Some("silent"), None) => Ok(Command::Shop(crate::switches::SHOP_SILENT)),
            _ => bad(),
        },
        "killcount" => match (first.map(str::to_ascii_lowercase).as_deref(), second) {
            (Some("on"), None) => Ok(Command::KillCount(true)),
            (Some("off"), None) => Ok(Command::KillCount(false)),
            _ => bad(),
        },
        "zones" if rest.is_empty() => Ok(Command::Zones),
        "stats" if rest.is_empty() => Ok(Command::Stats),
        "map" => match first.map(str::parse::<u16>) {
            Some(Ok(map)) if second.is_none() => Ok(Command::Map(map)),
            _ => bad(),
        },
        "" => Err("try help".to_owned()),
        _ if help_for(&lower).is_some() => bad(),
        _ => Err(format!("unknown command {word}; try help")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::switches::{SHOP_OFF, SHOP_ON, SHOP_SILENT};

    #[test]
    fn every_command_parses_with_or_without_the_dot() {
        let cases = [
            ("help", Command::Help(None)),
            (".help kick", Command::Help(Some("kick".into()))),
            ("info", Command::Info),
            (".NOTICE  Maintenance in 5 minutes ", Command::Notice("Maintenance in 5 minutes".into())),
            ("quit", Command::Quit(None)),
            (".quit 5", Command::Quit(Some(5))),
            (".quit cancel", Command::QuitCancel),
            ("restart", Command::Restart(None)),
            (".restart 15", Command::Restart(Some(15))),
            ("reload", Command::Reload),
            (".rules", Command::Rules),
            ("scripts", Command::Scripts),
            ("allout", Command::AllOut),
            ("close", Command::Close),
            ("open", Command::Open),
            ("maxusers 0", Command::MaxUsers(0)),
            ("maxusers 29999", Command::MaxUsers(29999)),
            ("kick user Bob", Command::Kick(KickBy::User("Bob".into()))),
            ("kick userid 7", Command::Kick(KickBy::UserId(7))),
            ("kick char ProbeHero", Command::Kick(KickBy::Char("ProbeHero".into()))),
            ("kick charid 12", Command::Kick(KickBy::CharId(12))),
            ("exp 2", Command::Exp(2.0)),
            ("exp 0", Command::Exp(0.0)),
            ("exp off", Command::ExpOff),
            ("killrate 1", Command::KillRate(1)),
            ("killrate 100", Command::KillRate(100)),
            ("shop on", Command::Shop(SHOP_ON)),
            ("shop off", Command::Shop(SHOP_OFF)),
            ("shop silent", Command::Shop(SHOP_SILENT)),
            ("killcount on", Command::KillCount(true)),
            ("killcount off", Command::KillCount(false)),
            ("zones", Command::Zones),
            ("map 1", Command::Map(1)),
            (".stats", Command::Stats),
        ];
        for (line, command) in cases {
            assert_eq!(parse(line), Ok(command), "{line}");
        }
    }

    #[test]
    fn wrong_words_and_arguments_change_nothing() {
        assert_eq!(parse(".foo bar"), Err("unknown command foo; try help".into()));
        assert_eq!(parse(""), Err("try help".into()));
        assert_eq!(parse("."), Err("try help".into()));
        for line in [
            "exp -1",
            "exp nan",
            "exp",
            "exp 1 2",
            "killrate 0",
            "killrate 101",
            "killrate x",
            "maxusers 30000",
            "maxusers",
            "kick",
            "kick user",
            "kick userid 0",
            "kick charid x",
            "kick foo bar",
            "shop",
            "shop maybe",
            "killcount",
            "map",
            "map 70000",
            "quit 0",
            "quit 1441",
            "quit x",
            "quit 1 2",
            "restart 0",
            "restart 1441",
            "restart x",
            "restart 1 2",
            "restart cancel",
            "reload now",
            "rules now",
            "notice",
            "info now",
            "stats now",
            "close now",
            "help a b",
        ] {
            let word = line.split(' ').next().unwrap();
            assert_eq!(parse(line), Err(wrong(word)), "{line}");
            assert!(
                parse(line).unwrap_err().contains(&format!("usage: {}", TABLE.iter().find(|e| e.word == word).unwrap().usage)),
                "{line}"
            );
        }
    }

    #[test]
    fn help_lists_every_command_and_explains_each() {
        let all = help();
        assert_eq!(all.len(), 20);
        assert!(all.contains(&"exp <factor> | exp off".to_owned()));
        let lines = help_for("exp").unwrap();
        assert_eq!(lines[0], "usage: exp <factor> | exp off");
        assert!(lines[1].starts_with("exp multiplies") && lines[2] == "e.g. .exp 1.5");
        assert_eq!(help_for("KICK"), help_for("kick"));
        assert_eq!(help_for("nothing"), None);
        // Every example is itself a valid command, every text fits a whisper line.
        for entry in &TABLE {
            assert!(parse(entry.example).is_ok(), "{}", entry.example);
            for line in help_for(entry.word).unwrap() {
                assert!(line.len() <= 120, "{line}");
            }
        }
    }

    #[test]
    fn the_in_game_line_starts_with_a_dot_after_a_colour_marker() {
        assert_eq!(typed_command(b".info\0").as_deref(), Some("info"));
        assert_eq!(typed_command(b".kick char X").as_deref(), Some("kick char X"));
        assert_eq!(typed_command(b"<<005c03.info\0").as_deref(), Some("info"), "a chat colour marker is skipped");
        assert_eq!(typed_command(b"hello\0"), None);
        assert_eq!(typed_command(b"<<005c03hello\0"), None);
        assert_eq!(typed_command(b"\0"), None);
    }

    #[test]
    fn the_countdown_keeps_the_originals_timing() {
        assert_eq!(quit_duration(None), Duration::from_secs(30), "at least 30 s");
        assert_eq!(quit_duration(Some(1)), Duration::from_secs(60));
        assert_eq!(quit_duration(Some(5)), Duration::from_secs(300));
        let secs = |s| next_notice(Duration::from_secs(s)).as_secs();
        assert_eq!((secs(300), secs(31), secs(30), secs(11), secs(10), secs(1)), (10, 10, 5, 5, 1, 1));
        assert_eq!(quit_notice(25), "The server will shut down in 25 seconds. Please log out.");
    }
}
