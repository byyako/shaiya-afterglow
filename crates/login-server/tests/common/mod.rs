//! Helpers shared by the login server's socket tests: a server over `MemoryBackend` and the client side of the connection.
//! `tests/login_flow.rs` and `tests/fuzz_login.rs` both `mod common;`.
#![allow(dead_code)]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use login_server::backend::ServerInfo;
use login_server::config::Config;
use login_server::keys::KeyPool;
use login_server::limits::LimitConfig;
use login_server::memory::MemoryBackend;
use login_server::server::{Context, serve};
use shaiya_crypto::{CtrStream, MODULUS_LEN, RsaKeyPair, SessionKeys, derive_session_keys};
use shaiya_proto::login;
use shaiya_proto::{FrameDecoder, frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

pub const GAME_IP: Ipv4Addr = Ipv4Addr::new(10, 1, 2, 3);

pub fn test_config(server_version: Option<u32>) -> Config {
    Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        database_url: String::new(),
        heartbeat_timeout: Duration::from_secs(15),
        key_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(600),
        check_versions: server_version.is_some(),
        limits: LimitConfig::default(),
        key_pool_size: 2,
        max_connections: 50,
    }
}

pub fn game_server(id: u8, name: &str, address: Ipv4Addr, version: Option<u32>) -> ServerInfo {
    ServerInfo { id, name: name.into(), address, capacity: 500, players: 0, version, open: true }
}

pub fn backend(server_version: Option<u32>) -> Arc<MemoryBackend> {
    let backend = Arc::new(MemoryBackend::default());
    let accounts = &backend.accounts;
    accounts.add("alice", "secret", 0).unwrap();
    accounts.add("gmbob", "gmpass", 48).unwrap(); // staff level 3
    accounts.add("boss", "bosspass", 16).unwrap(); // full operator
    accounts.add("banned", "banpass", 2).unwrap(); // blocked from the game
    accounts.add("leaving", "leavepass", 0x80).unwrap(); // withdrawal
    accounts.add("longpw", "abcdefghijklmnop", 0).unwrap(); // 16 characters, all the client sends
    backend.set_server(game_server(1, "Test Server", GAME_IP, server_version));
    backend
}

pub async fn serve_with(config: Config, backend: Arc<MemoryBackend>) -> SocketAddr {
    serve_with_keys(config, backend, KeyPool::generate(2)).await
}

/// `serve_with` with keys made before (generation is slow, so a file that starts many servers makes one pool).
pub async fn serve_with_keys(config: Config, backend: Arc<MemoryBackend>, keys: KeyPool) -> SocketAddr {
    let ctx = Arc::new(Context::new(config, keys, backend));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx));
    addr
}

pub async fn start_with(config: Config) -> SocketAddr {
    let version = config.check_versions.then_some(108);
    serve_with(config, backend(version)).await
}

pub async fn start(server_version: Option<u32>) -> SocketAddr {
    serve_with(test_config(server_version), backend(server_version)).await
}

/// A client written from the protocol notes, independent of the server's connection code.
pub struct Client {
    pub stream: TcpStream,
    pub decoder: FrameDecoder,
    pub send: Option<CtrStream>,
    pub recv: Option<CtrStream>,
    pub modulus: [u8; MODULUS_LEN],
    pub exponent: [u8; 64],
    pub keys: Option<SessionKeys>,
}

impl Client {
    pub async fn connect(addr: SocketAddr) -> Self {
        Self::try_connect(addr).await.expect("server sends the public key first")
    }

    /// `connect`, or `None` when the server closes the connection without sending its key (a refused address).
    pub async fn try_connect(addr: SocketAddr) -> Option<Self> {
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut client =
            Self { stream, decoder: FrameDecoder::new(), send: None, recv: None, modulus: [0; 128], exponent: [0; 64], keys: None };
        let hello = client.read().await?;
        assert_eq!(frame::opcode(&hello), login::OP_HANDSHAKE);
        let body = frame::payload(&hello);
        assert_eq!(body[..3], [0, 64, 128]);
        client.exponent.copy_from_slice(&body[3..67]);
        client.modulus.copy_from_slice(&body[67..195]);
        Some(client)
    }

    pub async fn handshake(&mut self) {
        let mut secret = [0u8; MODULUS_LEN];
        rand::Rng::fill(&mut rand::rng(), &mut secret[..MODULUS_LEN - 2]);
        let ciphertext = RsaKeyPair::encrypt_le(&self.modulus, &self.exponent, &secret).unwrap();
        self.send_plain(login::handshake_response(&ciphertext)).await;
        let keys = derive_session_keys(&secret, &self.modulus);
        self.send = Some(CtrStream::new(&keys.key, &keys.iv));
        self.recv = Some(CtrStream::new(&keys.key, &keys.iv));
        self.keys = Some(keys);
    }

    pub async fn send_plain(&mut self, packet: Vec<u8>) {
        self.stream.write_all(&packet).await.unwrap();
    }

    pub async fn send_encrypted(&mut self, mut packet: Vec<u8>) {
        self.send.as_mut().unwrap().apply(&mut packet[2..]);
        self.stream.write_all(&packet).await.unwrap();
    }

    /// Next packet, decrypted once encryption is on. `None` if the server closed the connection.
    pub async fn read(&mut self) -> Option<Vec<u8>> {
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(mut packet) = self.decoder.next_frame().unwrap() {
                if let Some(cipher) = &mut self.recv {
                    cipher.apply(&mut packet[2..]);
                }
                return Some(packet);
            }
            let n = timeout(Duration::from_secs(5), self.stream.read(&mut buffer)).await.expect("server answered in time").unwrap_or(0);
            if n == 0 {
                return None;
            }
            self.decoder.push(&buffer[..n]);
        }
    }

    pub async fn login(&mut self, user: &str, pass: &str) -> Vec<u8> {
        self.send_encrypted(login::login_request(user, pass)).await;
        self.read().await.expect("login reply")
    }

    pub async fn expect_closed(&mut self) {
        assert!(self.read().await.is_none(), "server should have closed the connection");
    }
}
