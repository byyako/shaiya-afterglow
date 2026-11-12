//! `bot probe`: logs in through a running login server, selects a game server, connects to it and prints the character screen
//! (`game_probe` as it was).
//!
//! With `create`, it chooses the light faction if none is set and creates a human fighter in the first free slot. The game server is
//! reached at the address the login server returns, on port 30810. When the game server checks addresses, connect to the login server
//! through the same interface the game server is reached on.

use anyhow::{Context, Result, bail};
use shaiya_proto::frame;
use shaiya_proto::game::{self as g, CreateRequest};

use crate::enter::{ListEntry, connect_game};
use crate::login::login_and_select;

pub const USAGE: &str = "usage: bot probe <login host:port> <username> <password> [server id] [create <name> | delete <id> | restore <id>]";

fn show(record: &[u8]) {
    match ListEntry::parse(record) {
        Ok(e) if e.id == 0 => println!("  slot {}: empty", e.slot),
        Ok(_) => {
            // The fields the screen prints that the bot's own record does not keep.
            let name = &record[54..73];
            let name = String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(19)]);
            println!(
                "  slot {}: id {} '{name}' level {} race {} mode {} class {} map {} hp {}{}",
                record[0],
                u32::from_le_bytes(record[1..5].try_into().unwrap()),
                u16::from_le_bytes([record[9], record[10]]),
                record[11],
                record[12],
                record[16],
                u16::from_le_bytes([record[18], record[19]]),
                u16::from_le_bytes([record[32], record[33]]),
                if record[73] != 0 { " (deletion pending)" } else { "" }
            );
        }
        Err(error) => println!("  unreadable record: {error}"),
    }
}

/// `args` are those after `probe`.
pub async fn run(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        bail!("{USAGE}");
    }
    let server = args.get(3).map(|s| s.parse()).transpose()?;
    let action = args.get(4).map(String::as_str);
    let selected = login_and_select(&args[0], &args[1], &args[2], server).await?;
    println!("login: user id {}, privilege {}", selected.user_id, selected.privilege);
    for s in &selected.servers {
        println!("  server {} '{}' status {}", s.id, s.name, s.status);
    }
    println!("select server {}: game server at {}", selected.server_id, selected.game_address);

    let (mut game, screen) = connect_game(&selected.game_address, selected.user_id, &selected.identity, &selected.keys).await?;
    println!("game handshake: success {} mode {} seed {:?}", screen.success, screen.mode, screen.seed);
    println!("billing info: {:?}", screen.billing);
    println!("faction {} (2 = none), max mode {}", screen.country, screen.max_grow);
    for record in &screen.records {
        show(record);
    }
    let mut faction = screen.country;
    let wait = std::time::Duration::from_secs(5);

    match (action, args.get(5)) {
        (Some("create"), Some(name)) => {
            if faction == g::COUNTRY_NONE {
                game.write(frame::encode(g::OP_CHAR_COUNTRY, &[0])).await?;
                faction = 0;
                println!("chose faction {faction}");
            }
            let slot = screen.records.iter().find(|r| r[1..5] == [0, 0, 0, 0]).map(|r| r[0]).context("no free slot")?;
            let request = CreateRequest {
                slot,
                family: 0,
                grow: 1,
                hair: 0,
                face: 0,
                size: 0,
                job: 0,
                sex: 0,
                name: name.clone().into_bytes(),
                sent_len: 0,
            };
            game.write(g::char_create_request(&request)).await?;
            let result = game.read(wait).await?;
            println!("create: result {}", frame::payload(&result)[0]);
            if frame::payload(&result)[0] == 0 {
                for _ in 0..5 {
                    show(frame::payload(&game.read(wait).await?));
                }
            }
        }
        (Some(verb @ ("delete" | "restore")), Some(id)) => {
            let opcode = if verb == "delete" { g::OP_CHAR_DELETE } else { g::OP_CHAR_RESTORE };
            game.write(g::char_id_request(opcode, id.parse()?)).await?;
            match game.read(wait).await {
                Ok(reply) => println!("{verb}: {:?}", frame::payload(&reply)),
                Err(error) => println!("{verb}: no answer ({error})"),
            }
        }
        _ => {}
    }
    game.write(frame::encode(g::OP_CHAR_LEAVE, &[])).await?;
    Ok(())
}
