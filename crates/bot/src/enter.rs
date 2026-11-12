//! The game connection: handshake and character screen (as `game_probe`), then selecting a character and entering the world (as
//! `tests/common`: select, the burst up to `0x010B`, `0x0201`, the zone entry).

use std::time::Duration;

use anyhow::{Context, Result, bail};
use shaiya_crypto::{SessionKeys, game_session_keys};
use shaiya_proto::game::{self as g};
use shaiya_proto::{combat, frame, skill, world};

use crate::link::Link;
use crate::world::World;

/// How long each packet of the handshake, the burst and the zone entry may take.
const WAIT: Duration = Duration::from_secs(5);

/// A character of the list (`0x0101`): the fields of its 81-byte record the bot uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEntry {
    pub slot: u8,
    /// 0 for an empty slot.
    pub id: u32,
    pub level: u16,
    pub family: u8,
    pub grow: u8,
    pub job: u8,
    pub map: u16,
    pub hp: u16,
    pub name: String,
    pub delete_pending: bool,
}

impl ListEntry {
    pub fn parse(record: &[u8]) -> Result<Self> {
        if record.len() < 5 {
            bail!("a character record of {} bytes", record.len());
        }
        let id = u32::from_le_bytes(record[1..5].try_into()?);
        if id == 0 {
            return Ok(Self {
                slot: record[0],
                id: 0,
                level: 0,
                family: 0,
                grow: 0,
                job: 0,
                map: 0,
                hp: 0,
                name: String::new(),
                delete_pending: false,
            });
        }
        if record.len() < 74 {
            bail!("a character record of {} bytes", record.len());
        }
        let name = &record[54..73];
        Ok(Self {
            slot: record[0],
            id,
            level: u16::from_le_bytes([record[9], record[10]]),
            family: record[11],
            grow: record[12],
            job: record[16],
            map: u16::from_le_bytes([record[18], record[19]]),
            hp: u16::from_le_bytes([record[32], record[33]]),
            name: String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(19)]).into_owned(),
            delete_pending: record[73] != 0,
        })
    }
}

/// What the game server says between the handshake and the end of the character list.
pub struct CharacterScreen {
    pub success: u8,
    pub mode: u8,
    pub seed: String,
    pub billing: Vec<u8>,
    /// The account's faction (`g::COUNTRY_NONE`, 2, when it has not chosen).
    pub country: u8,
    pub max_grow: u8,
    /// The five raw `0x0101` records, in slot order.
    pub records: Vec<Vec<u8>>,
}

impl CharacterScreen {
    pub fn entries(&self) -> Result<Vec<ListEntry>> {
        self.records.iter().map(|r| ListEntry::parse(r)).collect()
    }

    /// The first character that exists and is not waiting to be deleted: the one a bot plays.
    pub fn first_character(&self) -> Result<ListEntry> {
        self.entries()?.into_iter().find(|e| e.id != 0 && !e.delete_pending).context("the account has no character")
    }
}

/// Connects to the game server and does the handshake; the keys are those of the login (`game_session_keys` derives the game
/// connection's counter from them). Returns the link, with the XOR switch on, and the character screen.
pub async fn connect_game(addr: &str, user_id: u32, identity: &[u8; 16], login_keys: &SessionKeys) -> Result<(Link, CharacterScreen)> {
    let mut link = Link::connect(addr).await?;
    link.write(g::game_connect_request(user_id, identity)).await?;
    link.encrypt(&game_session_keys(login_keys));
    link.enable_world_cipher();
    let reply = link.read(WAIT).await.context("handshake refused (no reply)")?;
    let body = frame::payload(&reply);
    if frame::opcode(&reply) != g::OP_GAME_CONNECT || body.len() < 2 {
        bail!("unexpected handshake reply {:#06x}", frame::opcode(&reply));
    }
    let (success, mode, seed) = (body[0], body[1], String::from_utf8_lossy(&body[2..]).into_owned());
    let billing = link.read(WAIT).await?;
    let country = link.read(WAIT).await?;
    let body = frame::payload(&country);
    if frame::opcode(&country) != g::OP_CHAR_COUNTRY || body.len() < 2 {
        bail!("unexpected {:#06x} where the faction was due", frame::opcode(&country));
    }
    let (country, max_grow) = (body[0], body[1]);
    let mut records = Vec::new();
    for _ in 0..g::SLOTS {
        let packet = link.read(WAIT).await?;
        if frame::opcode(&packet) != g::OP_CHAR_LIST {
            bail!("unexpected {:#06x} in the character list", frame::opcode(&packet));
        }
        records.push(frame::payload(&packet).to_vec());
    }
    Ok((link, CharacterScreen { success, mode, seed, billing: frame::payload(&billing).to_vec(), country, max_grow, records }))
}

/// Selects `character`, reads the burst up to `0x010B`, sends `0x0201` and reads the zone entry up to the player's own `0x0521`.
/// Everything read goes through the returned world, which also holds the players in view and the questions queued for them.
pub async fn enter_world(link: &mut Link, country: u8, character: &ListEntry) -> Result<World> {
    let mut world = World::new(character.id, country, character.job, character.level, character.map);
    link.write(g::char_id_request(g::OP_CHAR_SELECT, character.id)).await?;
    loop {
        let packet = link.read(WAIT).await.context("the burst after selecting the character")?;
        let opcode = frame::opcode(&packet);
        world.handle(&packet);
        if opcode == skill::OP_CHAR_QUICK {
            break;
        }
    }
    link.write(shaiya_proto::encode(world::OP_USER_ENTER, &[])).await?;
    // The zone entry: the player's own 0x0201 and 0x051C, those of the players and NPCs in view, the monsters, the weather, then the
    // player's HP, MP and SP.
    let mut weather = false;
    loop {
        let packet = link.read(WAIT).await.context("the zone entry")?;
        let opcode = frame::opcode(&packet);
        world.handle(&packet);
        match opcode {
            world::OP_WEATHER => weather = true,
            combat::OP_RECOVER_SET if weather => return Ok(world),
            _ => {}
        }
    }
}
