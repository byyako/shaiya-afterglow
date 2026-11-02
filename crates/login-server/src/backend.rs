//! What the login server needs from its surroundings, as three small traits.
//! `PgBackend` implements them on PostgreSQL, `MemoryBackend` in memory for tests.

use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use shaiya_crypto::SessionKeys;

use crate::accounts::AccountStore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerInfo {
    pub id: u8,
    pub name: String,
    /// The one IPv4 address clients are sent to; the client always uses port 30810.
    pub address: Ipv4Addr,
    pub capacity: u16,
    pub players: u16,
    /// Client version required when version checking is on; `None` accepts any.
    pub version: Option<u32>,
    /// Enabled by the operator and heartbeating recently.
    pub open: bool,
}

pub trait ServerDirectory: Send + Sync + 'static {
    fn servers(&self) -> impl Future<Output = Result<Vec<ServerInfo>>> + Send;
    fn server(&self, id: u8) -> impl Future<Output = Result<Option<ServerInfo>>> + Send;
}

pub struct NewSession {
    pub user_id: u32,
    pub identity: [u8; 16],
    pub keys: SessionKeys,
    pub ip: IpAddr,
}

pub trait SessionStore: Send + Sync + 'static {
    /// Starts the account's session, replacing any earlier one.
    fn create(&self, session: NewSession) -> impl Future<Output = Result<()>> + Send;
    /// Records which game server the account chose and gives the client time to connect to it.
    fn bind_server(&self, user_id: u32, identity: [u8; 16], server_id: u8) -> impl Future<Output = Result<()>> + Send;
}

pub trait Backend: AccountStore + ServerDirectory + SessionStore {}
impl<T: AccountStore + ServerDirectory + SessionStore> Backend for T {}

/// How long a session lives after login and after choosing a server, until the game server takes over.
pub const SESSION_LOGIN_SECS: f64 = 300.0;
pub const SESSION_SELECT_SECS: f64 = 120.0;

// A shared backend works wherever an owned one does, so tests and processes can keep a handle to it.
impl<T: AccountStore> AccountStore for std::sync::Arc<T> {
    fn authenticate(&self, username: &str, password: &str, ip: IpAddr) -> impl Future<Output = Result<crate::accounts::AuthResult>> + Send {
        (**self).authenticate(username, password, ip)
    }
}

impl<T: ServerDirectory> ServerDirectory for std::sync::Arc<T> {
    fn servers(&self) -> impl Future<Output = Result<Vec<ServerInfo>>> + Send {
        (**self).servers()
    }
    fn server(&self, id: u8) -> impl Future<Output = Result<Option<ServerInfo>>> + Send {
        (**self).server(id)
    }
}

impl<T: SessionStore> SessionStore for std::sync::Arc<T> {
    fn create(&self, session: NewSession) -> impl Future<Output = Result<()>> + Send {
        (**self).create(session)
    }
    fn bind_server(&self, user_id: u32, identity: [u8; 16], server_id: u8) -> impl Future<Output = Result<()>> + Send {
        (**self).bind_server(user_id, identity, server_id)
    }
}
