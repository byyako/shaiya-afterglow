//! In-memory backend for tests and quick experiments.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

use anyhow::{Result, bail};
use shaiya_crypto::SessionKeys;

use crate::accounts::{AccountStore, AuthResult, MemoryAccountStore};
use crate::backend::{NewSession, ServerDirectory, ServerInfo, SessionStore};

#[derive(Clone, Debug)]
pub struct StoredSession {
    pub identity: [u8; 16],
    pub keys: SessionKeys,
    pub ip: IpAddr,
    pub server_id: Option<u8>,
}

#[derive(Default)]
pub struct MemoryBackend {
    pub accounts: MemoryAccountStore,
    servers: Mutex<Vec<ServerInfo>>,
    sessions: Mutex<HashMap<u32, StoredSession>>,
}

impl MemoryBackend {
    pub fn set_server(&self, info: ServerInfo) {
        let mut servers = self.servers.lock().unwrap();
        servers.retain(|s| s.id != info.id);
        servers.push(info);
        servers.sort_by_key(|s| s.id);
    }

    pub fn session(&self, user_id: u32) -> Option<StoredSession> {
        self.sessions.lock().unwrap().get(&user_id).cloned()
    }
}

impl AccountStore for MemoryBackend {
    fn authenticate(&self, username: &str, password: &str, ip: IpAddr) -> impl Future<Output = Result<AuthResult>> + Send {
        self.accounts.authenticate(username, password, ip)
    }
}

impl ServerDirectory for MemoryBackend {
    async fn servers(&self) -> Result<Vec<ServerInfo>> {
        Ok(self.servers.lock().unwrap().clone())
    }

    async fn server(&self, id: u8) -> Result<Option<ServerInfo>> {
        Ok(self.servers.lock().unwrap().iter().find(|s| s.id == id).cloned())
    }
}

impl SessionStore for MemoryBackend {
    async fn create(&self, session: NewSession) -> Result<()> {
        self.sessions
            .lock()
            .unwrap()
            .insert(session.user_id, StoredSession { identity: session.identity, keys: session.keys, ip: session.ip, server_id: None });
        Ok(())
    }

    async fn bind_server(&self, user_id: u32, identity: [u8; 16], server_id: u8) -> Result<()> {
        match self.sessions.lock().unwrap().get_mut(&user_id) {
            Some(session) if session.identity == identity => {
                session.server_id = Some(server_id);
                Ok(())
            }
            _ => bail!("no session to bind (replaced by a newer login?)"),
        }
    }
}
