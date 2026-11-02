//! Walks the login flow against a running login server and prints what comes back.
//!
//! usage: probe <host:port> <username> <password> [client_version]
//! Useful for checking a deployment; it speaks the same protocol as the game client.

use anyhow::{Result, bail};
use shaiya_crypto::{CtrStream, MODULUS_LEN, RsaKeyPair, derive_session_keys};
use shaiya_proto::login;
use shaiya_proto::{FrameDecoder, frame};
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
    async fn read(&mut self) -> Result<Vec<u8>> {
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(mut packet) = self.decoder.next_frame().map_err(|e| anyhow::anyhow!("{e:?}"))? {
                if let Some(cipher) = &mut self.recv {
                    cipher.apply(&mut packet[2..]);
                }
                return Ok(packet);
            }
            let n = timeout(Duration::from_secs(5), self.stream.read(&mut buffer)).await??;
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

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(addr), Some(user), Some(pass)) = (args.next(), args.next(), args.next()) else {
        bail!("usage: probe <host:port> <username> <password> [client_version]");
    };
    let version: u32 = args.next().map(|v| v.parse()).transpose()?.unwrap_or(108);

    let mut link = Link { stream: TcpStream::connect(&addr).await?, decoder: FrameDecoder::new(), send: None, recv: None };

    let hello = link.read().await?;
    let body = frame::payload(&hello);
    println!("server key: exponent {} bytes, modulus {} bytes", body[1], body[2]);
    let exponent: [u8; 64] = body[3..67].try_into()?;
    let modulus: [u8; MODULUS_LEN] = body[67..195].try_into()?;

    let mut secret = [0u8; MODULUS_LEN];
    rand::Rng::fill(&mut rand::rng(), &mut secret[..MODULUS_LEN - 2]);
    let ciphertext = RsaKeyPair::encrypt_le(&modulus, &exponent, &secret).expect("secret below modulus");
    link.write(login::handshake_response(&ciphertext)).await?;
    let keys = derive_session_keys(&secret, &modulus);
    link.send = Some(CtrStream::new(&keys.key, &keys.iv));
    link.recv = Some(CtrStream::new(&keys.key, &keys.iv));

    link.write(login::login_request(&user, &pass)).await?;
    let reply = link.read().await?;
    let body = frame::payload(&reply);
    println!("login status {} user id {} privilege {}", body[0], u32::from_le_bytes(body[1..5].try_into()?), body[5]);
    if body[0] != 0 {
        return Ok(());
    }

    let list = link.read().await?;
    let body = frame::payload(&list);
    let mut server_id = 0;
    for entry in body[1..].chunks(38).take(body[0] as usize) {
        let name_end = entry[6..].iter().position(|&b| b == 0).unwrap_or(32);
        println!(
            "server {} '{}' status {} players {}/{}",
            entry[0],
            String::from_utf8_lossy(&entry[6..6 + name_end]),
            entry[1],
            u16::from_le_bytes([entry[2], entry[3]]),
            u16::from_le_bytes([entry[4], entry[5]]),
        );
        server_id = entry[0];
    }

    link.write(login::select_server_request(server_id, version)).await?;
    let pick = link.read().await?;
    let body = frame::payload(&pick);
    println!("select server {server_id}: result {:#04x} address {}.{}.{}.{}", body[0], body[1], body[2], body[3], body[4]);
    Ok(())
}
