//! Chat packets (`0x11xx`), as `CUser::PacketChatE` in `ps_game.exe` reads and builds them (`docs/original/world.md` section 6a).
//! The text is passed on as the client sent it, terminating zero included.

use crate::frame::encode;

pub const OP_NORMAL: u16 = 0x1101;
pub const OP_WHISPER: u16 = 0x1102;
/// The client's trade chat (`$`, "[Trade]"); the server sends it to the sender's faction (`CWorld::PSendAllCountry`).
pub const OP_TRADE: u16 = 0x1103;
pub const OP_GUILD: u16 = 0x1104;
pub const OP_PARTY: u16 = 0x1105;
/// Whisper target not found (or of the other faction).
pub const OP_WHISPER_FAILED: u16 = 0x1106;
pub const OP_SHOUT: u16 = 0x1107;
/// "[chatting all members]": everyone on the server, with a Message To Server item armed (item special 38); this client never sends it.
pub const OP_ALL_SERVER: u16 = 0x1108;
pub const OP_ZONE: u16 = 0x1111;
pub const OP_PARTY_LEADER: u16 = 0x1112;
/// The staff's union line (`u32 id, u8 len, text`): the client shows it to anyone as "[Union] <text>" (message 6606), while it ignores
/// `0x1112` from the server; the new server sends a union leader's `0x1112` as this (`docs/original/union.md` deviation 7).
pub const OP_UNION_NOTICE: u16 = 0xF10A;

/// Longest text the original accepts (`u8 len`, zero included); longer closes the connection.
pub const MAX_TEXT: usize = 128;
pub(crate) const NAME_LEN: usize = 21;

#[derive(Debug, PartialEq, Eq)]
pub enum Text<'a> {
    Ok(&'a [u8]),
    /// `len` below 2: ignored.
    Empty,
    /// `len` above 128, or fewer bytes than announced: the original closes the connection.
    Invalid,
}

/// `u8 len` and `len` bytes of text.
fn text(body: &[u8]) -> Text<'_> {
    let Some((&len, rest)) = body.split_first() else { return Text::Invalid };
    let len = usize::from(len);
    if len > MAX_TEXT || rest.len() < len {
        Text::Invalid
    } else if len < 2 {
        Text::Empty
    } else {
        Text::Ok(&rest[..len])
    }
}

/// Every kind but the whisper: `u8 len, text`.
/// Whether the text's colour and item-link markers are ones game.exe can display (`docs/client/client-packets.md`, "Chat markup
/// crash"). The client renders `<<` + 3 digits (a length) + `c` + 2 digits (a colour) as a marker for the next bytes; a marker
/// whose length runs past the text crashes every client that shows the line. Each `<<` must be such a marker, with a length of at
/// least 1 and at most what follows it. The terminating zero does not count.
pub fn markup_is_safe(text: &[u8]) -> bool {
    let text = text.split(|&b| b == 0).next().unwrap_or(text);
    let digits = |bytes: &[u8]| -> Option<usize> {
        bytes.iter().try_fold(0usize, |n, b| b.is_ascii_digit().then(|| n * 10 + usize::from(b - b'0')))
    };
    let mut at = 0;
    while let Some(found) = text[at..].windows(2).position(|w| w == b"<<") {
        let start = at + found;
        let Some(marker) = text.get(start..start + 8) else { return false };
        let length = digits(&marker[2..5]);
        let colour = digits(&marker[6..8]);
        let rest = text.len() - (start + 8);
        match (length, marker[5], colour) {
            (Some(length), b'c', Some(_)) if (1..=rest).contains(&length) => at = start + 8,
            _ => return false,
        }
    }
    true
}

pub fn parse_message(payload: &[u8]) -> Text<'_> {
    text(payload)
}

/// `0x1102`: `char target[21], u8 len, text`. The target name is cut at its first zero (the original forces the last byte to 0).
pub fn parse_whisper(payload: &[u8]) -> Option<(Vec<u8>, Text<'_>)> {
    let name = payload.get(..NAME_LEN)?;
    let name = name[..NAME_LEN - 1].iter().copied().take_while(|&b| b != 0).collect();
    Some((name, text(&payload[NAME_LEN..])))
}

pub(crate) fn name_field(name: &[u8]) -> [u8; NAME_LEN] {
    let mut field = [0u8; NAME_LEN];
    let len = name.len().min(NAME_LEN - 1);
    field[..len].copy_from_slice(&name[..len]);
    field
}

/// Normal, shout and party chat: `u32 char id, u8 len, text`.
pub fn by_id(opcode: u16, char_id: u32, text: &[u8]) -> Vec<u8> {
    let mut p = char_id.to_le_bytes().to_vec();
    p.push(text.len() as u8);
    p.extend_from_slice(text);
    encode(opcode, &p)
}

/// All, trade and zone chat: `char sender[21], u8 len, text`.
pub fn by_name(opcode: u16, sender: &[u8], text: &[u8]) -> Vec<u8> {
    let mut p = name_field(sender).to_vec();
    p.push(text.len() as u8);
    p.extend_from_slice(text);
    encode(opcode, &p)
}

/// `0x1102` from the server: `u8 0` and the sender's name to the target, `u8 1` and the target's name back to the sender.
pub fn whisper(to_sender: bool, name: &[u8], text: &[u8]) -> Vec<u8> {
    whisper_op(OP_WHISPER, to_sender, name, text)
}

/// The whisper packet under another opcode (a GM's `0xF102`, `docs/original/gm-commands.md` section 5).
pub fn whisper_op(opcode: u16, to_sender: bool, name: &[u8], text: &[u8]) -> Vec<u8> {
    let mut p = vec![u8::from(to_sender)];
    p.extend_from_slice(&name_field(name));
    p.push(text.len() as u8);
    p.extend_from_slice(text);
    encode(opcode, &p)
}

/// A monster's line (`TP_CHAT_MONSTER`): `u8 kind` (0 to its target, 1 to the players around), `u32 monster id`, `u8 len`, the
/// text with its zero (`len` counts it, as the client reads it; the original sends no zero, `docs/original/scripted-mobs.md`
/// deviation 2).
pub const OP_MONSTER: u16 = 0x1109;

pub fn monster_say(kind: u8, mob: u32, text: &[u8]) -> Vec<u8> {
    let text = &text[..text.len().min(MAX_TEXT - 1)];
    let mut p = vec![kind];
    p.extend_from_slice(&mob.to_le_bytes());
    p.push(text.len() as u8 + 1);
    p.extend_from_slice(text);
    p.push(0);
    encode(OP_MONSTER, &p)
}

/// A Lua monster's line (`CMob::LuaSay`, `docs/original/instances.md` section 15): kind 1, the monster, `len = strlen`, and the text
/// with its zero in a 128-byte buffer, 136 bytes in all. `None` unless the text has 1 to 127 bytes.
pub fn monster_say_lua(mob: u32, text: &[u8]) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() >= MAX_TEXT {
        return None;
    }
    let mut p = vec![1];
    p.extend_from_slice(&mob.to_le_bytes());
    p.push(text.len() as u8);
    let mut buffer = [0u8; MAX_TEXT];
    buffer[..text.len()].copy_from_slice(text);
    p.extend_from_slice(&buffer);
    Some(encode(OP_MONSTER, &p))
}

/// `0x110A` (`CMob::LuaSayByIndex`): the monster and a `sysmsg` line number, which the client prints as "<monster> : <line>".
pub const OP_MONSTER_INDEX: u16 = 0x110A;

pub fn monster_say_index(mob: u32, index: u16) -> Vec<u8> {
    let mut p = mob.to_le_bytes().to_vec();
    p.extend_from_slice(&index.to_le_bytes());
    encode(OP_MONSTER_INDEX, &p)
}

/// `0x110B` (`CMob::LuaSayByVoice`): the monster and a sound file name in 32 bytes (at most 31 kept, then zeros), which the client
/// plays from `data/sound` at the monster.
pub const OP_MONSTER_VOICE: u16 = 0x110B;

pub fn monster_voice(mob: u32, file: &[u8]) -> Vec<u8> {
    let mut p = mob.to_le_bytes().to_vec();
    let mut name = [0u8; 32];
    let kept = file.iter().position(|&b| b == 0).unwrap_or(file.len()).min(31);
    name[..kept].copy_from_slice(&file[..kept]);
    p.extend_from_slice(&name);
    encode(OP_MONSTER_VOICE, &p)
}

/// `0x1106 u8 1`: nobody of that name (in the sender's faction) is online.
pub fn whisper_failed() -> Vec<u8> {
    encode(OP_WHISPER_FAILED, &[1])
}

#[cfg(test)]
mod tests {
    #[test]
    fn lua_monster_lines_have_the_original_sizes() {
        // After the frame's 2-byte length: 136, 8 and 38 bytes from the opcode on, as the original sends them.
        let say = super::monster_say_lua(7, b"Hi").unwrap();
        assert_eq!((say.len() - 2, &say[2..11]), (136, &[0x09, 0x11, 1, 7, 0, 0, 0, 2, b'H'][..]));
        assert!(super::monster_say_lua(7, b"").is_none() && super::monster_say_lua(7, &[b'a'; 128]).is_none());
        assert_eq!(super::monster_say_index(7, 6401)[2..], [0x0A, 0x11, 7, 0, 0, 0, 0x01, 0x19]);
        let voice = super::monster_voice(7, b"boss.wav");
        assert_eq!((voice.len() - 2, &voice[8..16], voice[16]), (38, &b"boss.wav"[..], 0));
    }

    #[test]
    fn markup_that_would_crash_clients_is_refused() {
        assert!(markup_is_safe(b"hello\0"));
        // An item link as the client builds it: <<NNNcCC then the NNN bytes of the name.
        assert!(markup_is_safe(b"look <<005c03Sword!\0"));
        assert!(markup_is_safe(b"<<005c03Sword\0"), "exactly as long as the rest");
        assert!(!markup_is_safe(b"<<999c01\0"), "the length runs past the text");
        assert!(!markup_is_safe(b"<<005c01\0hello"), "bytes after the first zero do not count");
        assert!(!markup_is_safe(b"<<005c01\0hello\0"));
        assert!(markup_is_safe(b"hi\0<<999c01"), "a marker after the zero is never shown");
        assert!(!markup_is_safe(b"<<006c03Sword\0"));
        assert!(!markup_is_safe(b"<<000c03Sword\0"), "length 0");
        assert!(!markup_is_safe(b"<<05c03Sword\0"), "not 3 digits");
        assert!(!markup_is_safe(b"<<005x03Sword\0"));
        assert!(!markup_is_safe(b"<<005c3xSword\0"));
        assert!(!markup_is_safe(b"a <<\0"), "cut short");
        assert!(!markup_is_safe(b"<<001c01a <<999c01\0"), "every marker counts");
    }

    use super::*;
    use crate::frame::payload;

    #[test]
    fn requests_parse_like_the_live_client_sent_them() {
        // Seen live: 0x1101 04 "hey\0", and a whisper to ProbeHero.
        assert_eq!(parse_message(&[4, b'h', b'e', b'y', 0]), Text::Ok(b"hey\0"));
        let mut whisper = b"ProbeHero".to_vec();
        whisper.resize(21, 0);
        whisper.extend_from_slice(&[3, b'e', b'r', 0]);
        assert_eq!(parse_whisper(&whisper), Some((b"ProbeHero".to_vec(), Text::Ok(b"er\0"))));
        assert_eq!(parse_message(&[1, 0]), Text::Empty);
        assert_eq!(parse_message(&[129]), Text::Invalid);
        assert_eq!(parse_message(&[5, b'a']), Text::Invalid, "shorter than announced");
        assert_eq!(parse_whisper(&[0; 10]), None);
    }

    #[test]
    fn replies_have_the_original_layouts() {
        let normal = by_id(OP_NORMAL, 7, b"hey\0");
        assert_eq!(payload(&normal), [7, 0, 0, 0, 4, b'h', b'e', b'y', 0]);
        let all = by_name(OP_TRADE, b"Hero", b"hi\0");
        assert_eq!(payload(&all).len(), 21 + 1 + 3);
        assert_eq!(&payload(&all)[..5], b"Hero\0");
        assert_eq!(payload(&all)[21], 3);
        let everyone = by_name(OP_ALL_SERVER, b"TestRanger", b"hello\0");
        assert_eq!(everyone.len(), 2 + 6 + 24, "len + 24 bytes with the opcode (TP_CHAT_SENDALL)");
        assert_eq!(&payload(&everyone)[..10], b"TestRanger");
        assert!(payload(&everyone)[10..21].iter().all(|&b| b == 0), "the name padded with zeros");
        assert_eq!(&payload(&everyone)[21..], b"\x06hello\0");
        let w = whisper(true, b"Elf", b"yo\0");
        assert_eq!(payload(&w)[0], 1);
        assert_eq!(&payload(&w)[1..4], b"Elf");
        assert_eq!(payload(&w)[22], 3);
        assert_eq!(w.len(), 2 + 0x19 + 3, "len + 0x19 bytes with the opcode, as the original");
        assert_eq!(payload(&whisper_failed()), [1]);
    }
}
