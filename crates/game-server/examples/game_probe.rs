//! Logs in through a running login server, selects a game server, connects to it and prints the character screen.
//!
//! usage: game_probe <login host:port> <username> <password> [server id] [create <name> | delete <id> | restore <id>]
//!
//! With `create`, it chooses the light faction if none is set and creates a human fighter in the first free slot.
//! The game server is reached at the address the login server returns, on port 30810. When the game server checks
//! addresses, connect to the login server through the same interface the game server is reached on.

use anyhow::{Context, Result, bail};
use shaiya_crypto::{CtrStream, MODULUS_LEN, RsaKeyPair, SessionKeys, derive_session_keys, game_session_keys};
use shaiya_proto::game::{self as g, CreateRequest};
use shaiya_proto::{FrameDecoder, frame, login};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

struct Link {
    stream: TcpStream,
    decoder: FrameDecoder,
    send: Option<CtrStream>,
    recv: Option<CtrStream>,
}

impl Link {
    async fn connect(addr: &str) -> Result<Self> {
        Ok(Self { stream: TcpStream::connect(addr).await?, decoder: FrameDecoder::new(), send: None, recv: None })
    }

    fn encrypt(&mut self, keys: &SessionKeys) {
        self.send = Some(CtrStream::new(&keys.key, &keys.iv));
        self.recv = Some(CtrStream::new(&keys.key, &keys.iv));
    }

    async fn read(&mut self) -> Result<Vec<u8>> {
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(mut packet) = self.decoder.next_frame().map_err(|e| anyhow::anyhow!("{e:?}"))? {
                if let Some(cipher) = &mut self.recv {
                    cipher.apply(&mut packet[2..]);
                }
                return Ok(packet);
            }
            let n = timeout(Duration::from_secs(5), self.stream.read(&mut buffer)).await.context("no answer within 5 s")??;
            if n == 0 {
                bail!("server closed the connection");
            }
            self.decoder.push(&buffer[..n]);
        }
    }

    async fn write(&mut self, mut packet: Vec<u8>) -> Result<()> {
        if let Some(cipher) = &mut self.send {
            cipher.apply(&mut packet[2..]);
        }
        self.stream.write_all(&packet).await?;
        Ok(())
    }
}

/// Returns (user id, identity, login keys, game server address).
async fn login_and_select(addr: &str, user: &str, pass: &str, server: Option<u8>) -> Result<(u32, [u8; 16], SessionKeys, String)> {
    let mut link = Link::connect(addr).await?;
    let hello = link.read().await?;
    let body = frame::payload(&hello);
    let exponent: [u8; 64] = body[3..67].try_into()?;
    let modulus: [u8; MODULUS_LEN] = body[67..195].try_into()?;
    let mut secret = [0u8; MODULUS_LEN];
    rand::Rng::fill(&mut rand::rng(), &mut secret[..MODULUS_LEN - 2]);
    let ciphertext = RsaKeyPair::encrypt_le(&modulus, &exponent, &secret).expect("secret below modulus");
    link.write(login::handshake_response(&ciphertext)).await?;
    let keys = derive_session_keys(&secret, &modulus);
    link.encrypt(&keys);

    link.write(login::login_request(user, pass)).await?;
    let reply = link.read().await?;
    let body = frame::payload(&reply);
    if body[0] != 0 {
        bail!("login refused with status {}", body[0]);
    }
    let user_id = u32::from_le_bytes(body[1..5].try_into()?);
    let identity: [u8; 16] = body[6..22].try_into()?;
    println!("login: user id {user_id}, privilege {}", body[5]);

    let list = link.read().await?;
    let body = frame::payload(&list);
    let mut chosen = None;
    for entry in body[1..].chunks(38).take(body[0] as usize) {
        let name_end = entry[6..].iter().position(|&b| b == 0).unwrap_or(32);
        println!("  server {} '{}' status {}", entry[0], String::from_utf8_lossy(&entry[6..6 + name_end]), entry[1]);
        if server.is_none_or(|s| s == entry[0]) && chosen.is_none() {
            chosen = Some(entry[0]);
        }
    }
    let server_id = chosen.context("no such game server in the list")?;
    link.write(login::select_server_request(server_id, 0x10042600)).await?;
    let pick = link.read().await?;
    let body = frame::payload(&pick);
    if body[0] != 0 {
        bail!("server select refused with result {:#04x}", body[0]);
    }
    let address = format!("{}.{}.{}.{}:30810", body[1], body[2], body[3], body[4]);
    println!("select server {server_id}: game server at {address}");
    Ok((user_id, identity, keys, address))
}

fn show(record: &[u8]) {
    let id = u32::from_le_bytes(record[1..5].try_into().unwrap());
    if id == 0 {
        println!("  slot {}: empty", record[0]);
        return;
    }
    let name = &record[54..73];
    let name = String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(19)]);
    let level = u16::from_le_bytes([record[9], record[10]]);
    let map = u16::from_le_bytes([record[18], record[19]]);
    let hp = u16::from_le_bytes([record[32], record[33]]);
    println!(
        "  slot {}: id {id} '{name}' level {level} race {} mode {} class {} map {map} hp {hp}{}",
        record[0],
        record[11],
        record[12],
        record[16],
        if record[73] != 0 { " (deletion pending)" } else { "" }
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        bail!("usage: game_probe <login host:port> <username> <password> [server id] [create <name> | delete <id> | restore <id>]");
    }
    let server = args.get(3).map(|s| s.parse()).transpose()?;
    let action = args.get(4).map(String::as_str);
    let (user_id, identity, login_keys, address) = login_and_select(&args[0], &args[1], &args[2], server).await?;

    let mut game = Link::connect(&address).await?;
    game.write(g::game_connect_request(user_id, &identity)).await?;
    game.encrypt(&game_session_keys(&login_keys));
    let reply = game.read().await.context("handshake refused (no reply)")?;
    let body = frame::payload(&reply);
    println!("game handshake: success {} mode {} seed {:?}", body[0], body[1], String::from_utf8_lossy(&body[2..]));
    let billing = game.read().await?;
    println!("billing info: {:?}", frame::payload(&billing));
    let country = game.read().await?;
    let body = frame::payload(&country);
    println!("faction {} (2 = none), max mode {}", body[0], body[1]);
    let mut faction = body[0];
    let mut records = Vec::new();
    for _ in 0..5 {
        let packet = game.read().await?;
        records.push(frame::payload(&packet).to_vec());
        show(frame::payload(&packet));
    }

    match (action, args.get(5)) {
        (Some("create"), Some(name)) => {
            if faction == g::COUNTRY_NONE {
                game.write(frame::encode(g::OP_CHAR_COUNTRY, &[0])).await?;
                faction = 0;
                println!("chose faction {faction}");
            }
            let slot = records.iter().find(|r| r[1..5] == [0, 0, 0, 0]).map(|r| r[0]).context("no free slot")?;
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
            let result = game.read().await?;
            println!("create: result {}", frame::payload(&result)[0]);
            if frame::payload(&result)[0] == 0 {
                for _ in 0..5 {
                    show(frame::payload(&game.read().await?));
                }
            }
        }
        (Some(verb @ ("delete" | "restore")), Some(id)) => {
            let opcode = if verb == "delete" { g::OP_CHAR_DELETE } else { g::OP_CHAR_RESTORE };
            game.write(g::char_id_request(opcode, id.parse()?)).await?;
            match game.read().await {
                Ok(reply) => println!("{verb}: {:?}", frame::payload(&reply)),
                Err(error) => println!("{verb}: no answer ({error})"),
            }
        }
        _ => {}
    }
    game.write(frame::encode(g::OP_CHAR_LEAVE, &[])).await?;
    Ok(())
}
