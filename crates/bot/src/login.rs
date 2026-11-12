//! The login connection as `game_probe` made it: key exchange, login, the server list, the server's selection, which answers with
//! the game server's address.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use shaiya_crypto::{MODULUS_LEN, RsaKeyPair, SessionKeys, derive_session_keys};
use shaiya_proto::{frame, login};

use crate::link::Link;

/// The client version the login server checks at server selection.
pub const CLIENT_VERSION: u32 = 0x1004_2600;
/// The port the client always connects to the game server on.
pub const GAME_PORT: u16 = 30810;
/// How long each answer of the login server may take.
const WAIT: Duration = Duration::from_secs(5);

/// One line of the server list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerLine {
    pub id: u8,
    pub name: String,
    pub status: u8,
}

/// What a login gives: the account, the identity and keys the game server is told of, and where it is.
pub struct Selected {
    pub user_id: u32,
    pub privilege: u8,
    pub identity: [u8; 16],
    pub keys: SessionKeys,
    pub servers: Vec<ServerLine>,
    pub server_id: u8,
    /// `ip:30810`, the address the login server returned.
    pub game_address: String,
}

/// Logs in at `addr` and selects the game server `server` (the first of the list when `None`).
pub async fn login_and_select(addr: &str, user: &str, pass: &str, server: Option<u8>) -> Result<Selected> {
    let mut link = Link::connect(addr).await?;
    let hello = link.read(WAIT).await.context("login server hello")?;
    let body = frame::payload(&hello);
    if body.len() < 195 {
        bail!("the login server's hello is {} bytes", body.len());
    }
    let exponent: [u8; 64] = body[3..67].try_into()?;
    let modulus: [u8; MODULUS_LEN] = body[67..195].try_into()?;
    let mut secret = [0u8; MODULUS_LEN];
    rand::Rng::fill(&mut rand::rng(), &mut secret[..MODULUS_LEN - 2]);
    let ciphertext = RsaKeyPair::encrypt_le(&modulus, &exponent, &secret).context("the secret does not fit the key")?;
    link.write(login::handshake_response(&ciphertext)).await?;
    let keys = derive_session_keys(&secret, &modulus);
    link.encrypt(&keys);

    link.write(login::login_request(user, pass)).await?;
    let reply = link.read(WAIT).await.context("login answer")?;
    let body = frame::payload(&reply);
    if body.len() < 22 {
        bail!("the login answer is {} bytes", body.len());
    }
    if body[0] != 0 {
        bail!("login refused with status {}", body[0]);
    }
    let user_id = u32::from_le_bytes(body[1..5].try_into()?);
    let privilege = body[5];
    let identity: [u8; 16] = body[6..22].try_into()?;

    let list = link.read(WAIT).await.context("server list")?;
    let body = frame::payload(&list);
    let mut servers = Vec::new();
    let mut chosen = None;
    for entry in body.get(1..).unwrap_or_default().as_chunks::<38>().0.iter().take(usize::from(body.first().copied().unwrap_or(0))) {
        let name_end = entry[6..].iter().position(|&b| b == 0).unwrap_or(32);
        servers.push(ServerLine { id: entry[0], name: String::from_utf8_lossy(&entry[6..6 + name_end]).into_owned(), status: entry[1] });
        if server.is_none_or(|s| s == entry[0]) && chosen.is_none() {
            chosen = Some(entry[0]);
        }
    }
    let server_id = chosen.context("no such game server in the list")?;
    link.write(login::select_server_request(server_id, CLIENT_VERSION)).await?;
    let pick = link.read(WAIT).await.context("server select answer")?;
    let body = frame::payload(&pick);
    if body.len() < 5 {
        bail!("the server select answer is {} bytes", body.len());
    }
    if body[0] != 0 {
        bail!("server select refused with result {:#04x}", body[0]);
    }
    let game_address = format!("{}.{}.{}.{}:{GAME_PORT}", body[1], body[2], body[3], body[4]);
    Ok(Selected { user_id, privilege, identity, keys, servers, server_id, game_address })
}
