//! The framed connection: AES-CTR in both directions once the keys are known, and on a game connection the server-to-client switch to
//! the fixed XOR table after `0x010B` (the last packet of the enter-world burst) and back to AES after `0x0107`, as `game.exe` does and
//! `crates/game-server/tests/common` copies it.

use std::fmt;
use std::time::Duration;

use shaiya_crypto::{CtrStream, SessionKeys, XOR_SEED, XorTable};
use shaiya_proto::{FrameDecoder, frame};
use shaiya_proto::{skill, world};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Why a connection ended, as the metrics count it (section 6: the server closed, a timeout, an error).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// The server closed the connection (or reset it).
    Closed,
    /// No packet within the time given.
    Timeout,
    /// Anything else: a bad frame, a write that failed.
    Error(String),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("server closed the connection"),
            Self::Timeout => f.write_str("no answer in time"),
            Self::Error(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for LinkError {}

pub struct Link {
    stream: TcpStream,
    decoder: FrameDecoder,
    send: Option<CtrStream>,
    recv: Option<CtrStream>,
    /// Whether `0x010B` and `0x0107` switch the receive cipher (a game connection; on the login connection `0x010B` is something else).
    world_cipher: bool,
    xor: Option<XorTable>,
}

impl Link {
    pub async fn connect(addr: &str) -> Result<Self, LinkError> {
        let stream = TcpStream::connect(addr).await.map_err(|e| LinkError::Error(format!("connecting to {addr}: {e}")))?;
        // A step or an attack is one small packet: never wait to batch it.
        let _ = stream.set_nodelay(true);
        Ok(Self { stream, decoder: FrameDecoder::new(), send: None, recv: None, world_cipher: false, xor: None })
    }

    /// AES-CTR both ways from the next packet on, with these keys' key and counter.
    pub fn encrypt(&mut self, keys: &SessionKeys) {
        self.send = Some(CtrStream::new(&keys.key, &keys.iv));
        self.recv = Some(CtrStream::new(&keys.key, &keys.iv));
    }

    /// Turns on the XOR switch of the game connection.
    pub fn enable_world_cipher(&mut self) {
        self.world_cipher = true;
    }

    /// The next packet, whole and decrypted (length field included). Cancel-safe: the only await is the socket's `read`, which loses no
    /// bytes when dropped, so it may sit in a `select!`.
    pub async fn next(&mut self) -> Result<Vec<u8>, LinkError> {
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(mut packet) = self.decoder.next_frame().map_err(|e| LinkError::Error(format!("{e:?}")))? {
                self.decrypt(&mut packet);
                return Ok(packet);
            }
            // A reset connection counts as closed.
            let n: usize = self.stream.read(&mut buffer).await.unwrap_or_default();
            if n == 0 {
                return Err(LinkError::Closed);
            }
            self.decoder.push(&buffer[..n]);
        }
    }

    /// `next` within `within`.
    pub async fn read(&mut self, within: Duration) -> Result<Vec<u8>, LinkError> {
        timeout(within, self.next()).await.map_err(|_| LinkError::Timeout)?
    }

    fn decrypt(&mut self, packet: &mut [u8]) {
        if let Some(table) = &self.xor {
            table.apply(&mut packet[2..]);
            if frame::opcode(packet) == world::OP_LOGOUT_TO_SELECT {
                self.xor = None;
            }
        } else if let Some(cipher) = &mut self.recv {
            cipher.apply(&mut packet[2..]);
            if self.world_cipher && frame::opcode(packet) == skill::OP_CHAR_QUICK {
                self.xor = Some(XorTable::new(&XOR_SEED));
            }
        }
    }

    pub async fn write(&mut self, mut packet: Vec<u8>) -> Result<(), LinkError> {
        if let Some(cipher) = &mut self.send {
            cipher.apply(&mut packet[2..]);
        }
        self.stream.write_all(&packet).await.map_err(|e| LinkError::Error(format!("write: {e}")))
    }

    /// Closes the sending side; the server sees the end of the connection.
    pub async fn close(&mut self) {
        let _ = self.stream.shutdown().await;
    }
}
