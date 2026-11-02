use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use rand::RngCore;
use shaiya_crypto::{CtrStream, SessionKeys, derive_session_keys};
use shaiya_proto::login::{
    self, LoginStatus, PRIVILEGE_PLAYER, SERVER_CLOSED_CAPACITY, SERVER_STATUS_CLOSED, SERVER_STATUS_NORMAL, ServerEntry, select_result,
};
use shaiya_proto::{FrameDecoder, frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

use crate::accounts::AuthResult;
use crate::backend::{Backend, NewSession};
use crate::config::{Config, version_allowed};
use crate::keys::KeyPool;
use crate::limits::{ConnectionGuard, IpLimits};

/// The original dropped a connection after this many failed logins.
const MAX_FAILED_LOGINS: u32 = 10;

/// Key-exchange decryptions running at once. Each is a full 1024-bit modular exponentiation, about a millisecond, which does not
/// belong on the async threads; a flood of connections queues here instead of using every core.
const RSA_SLOTS: usize = 4;

pub struct Context<S> {
    pub config: Config,
    pub keys: KeyPool,
    pub store: S,
    pub connections: Arc<Semaphore>,
    pub limits: Arc<IpLimits>,
    rsa: Arc<Semaphore>,
}

impl<S> Context<S> {
    pub fn new(config: Config, keys: KeyPool, store: S) -> Self {
        let connections = Arc::new(Semaphore::new(config.max_connections));
        let limits = IpLimits::new(config.limits.clone());
        Self { config, keys, store, connections, limits, rsa: Arc::new(Semaphore::new(RSA_SLOTS)) }
    }
}

pub async fn serve<S: Backend>(listener: TcpListener, ctx: Arc<Context<S>>) -> Result<()> {
    let mut failures = 0u32;
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => {
                failures = 0;
                accepted
            }
            Err(error) if listener_is_gone(&error) => return Err(error.into()),
            // Out of file descriptors or memory, or a connection that reset before it was taken: go on after a wait
            // (`docs/server/audit.md` R6).
            Err(error) => {
                failures = failures.saturating_add(1);
                let wait = accept_backoff(failures);
                tracing::warn!(%error, ?wait, "cannot accept a connection");
                tokio::time::sleep(wait).await;
                continue;
            }
        };
        let Ok(permit) = ctx.connections.clone().try_acquire_owned() else {
            tracing::warn!(%peer, "connection limit reached, dropping");
            continue;
        };
        let guard = match ctx.limits.try_connect(peer.ip()) {
            Ok(guard) => guard,
            Err(reason) => {
                tracing::debug!(%peer, ?reason, "connection refused");
                continue;
            }
        };
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _guard: ConnectionGuard = guard;
            match handle(stream, peer, &ctx).await {
                Ok(()) => tracing::debug!(%peer, "closed"),
                Err(error) => tracing::info!(%peer, %error, "dropped"),
            }
        });
    }
}

/// How long to wait after the `failures`th accept error in a row: 100 ms, doubling to 1 s.
fn accept_backoff(failures: u32) -> Duration {
    Duration::from_millis(100 << failures.saturating_sub(1).min(4)).min(Duration::from_secs(1))
}

/// An accept error that means the listening socket itself is gone (EBADF, EINVAL, ENOTSOCK), which no wait mends.
fn listener_is_gone(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(9 | 22 | 88))
}

enum Phase {
    /// Public key sent, waiting for the client's encrypted secret.
    AwaitKey,
    /// Encryption active, waiting for credentials.
    AwaitLogin { failed: u32 },
    /// Logged in, server list sent, waiting for a server choice.
    Authenticated { user_id: u32, privilege: u8, identity: [u8; 16] },
}

struct Connection {
    stream: TcpStream,
    decoder: FrameDecoder,
    recv: Option<CtrStream>,
    send: Option<CtrStream>,
}

impl Connection {
    async fn send(&mut self, mut packet: Vec<u8>) -> Result<()> {
        if let Some(cipher) = &mut self.send {
            cipher.apply(&mut packet[2..]);
        }
        self.stream.write_all(&packet).await?;
        Ok(())
    }

    /// Next decrypted packet, or `None` when the client closed the connection. The wait ends at `until` however many bytes arrive.
    async fn next_packet(&mut self, until: Instant) -> Result<Option<Vec<u8>>> {
        let mut buffer = [0u8; 2048];
        loop {
            match self.decoder.next_frame() {
                Ok(Some(mut packet)) => {
                    if let Some(cipher) = &mut self.recv {
                        cipher.apply(&mut packet[2..]);
                    }
                    return Ok(Some(packet));
                }
                Ok(None) => {}
                Err(error) => bail!("invalid packet: {error:?}"),
            }
            let read = tokio::time::timeout_at(until.into(), self.stream.read(&mut buffer)).await??;
            if read == 0 {
                return Ok(None);
            }
            self.decoder.push(&buffer[..read]);
        }
    }
}

async fn handle<S: Backend>(stream: TcpStream, peer: SocketAddr, ctx: &Context<S>) -> Result<()> {
    stream.set_nodelay(true)?;
    let mut conn = Connection { stream, decoder: FrameDecoder::new(), recv: None, send: None };
    let accepted = Instant::now();
    let mut keyed = accepted;

    let key = ctx.keys.pick();
    conn.send(login::handshake_request(&key.modulus_le(), &key.public_exponent_le())).await?;
    let mut phase = Phase::AwaitKey;
    let mut session_keys: Option<SessionKeys> = None;

    loop {
        // The client does the key exchange the moment it connects, so waiting for it is short; counted from the accept. After that
        // it sits at the login screen while the player types, counted from the key exchange; pings extend neither (R10). Once
        // logged in, it sits at the server list, and any packet restarts the idle time.
        let until = match phase {
            Phase::AwaitKey => accepted + ctx.config.key_timeout,
            Phase::AwaitLogin { .. } => keyed + ctx.config.idle_timeout,
            Phase::Authenticated { .. } => Instant::now() + ctx.config.idle_timeout,
        };
        let Some(packet) = conn.next_packet(until).await? else { break };
        let payload = frame::payload(&packet);
        match (frame::opcode(&packet), &mut phase) {
            (login::OP_PING, _) => {}

            (login::OP_LOGIN_TERMINATE, _) => return Ok(()),

            (login::OP_HANDSHAKE, Phase::AwaitKey) => {
                let Some(ciphertext) = login::parse_handshake_response(payload) else {
                    bail!("malformed key reply");
                };
                // Off the async threads, a few at a time, and not past the key exchange's own deadline.
                let decrypting = async {
                    let _slot = ctx.rsa.acquire().await?;
                    let key = key.clone();
                    Ok::<_, anyhow::Error>(tokio::task::spawn_blocking(move || key.decrypt_le(&ciphertext)).await?)
                };
                let Some(secret) = tokio::time::timeout_at((accepted + ctx.config.key_timeout).into(), decrypting)
                    .await
                    .map_err(|_| anyhow::anyhow!("the key exchange took longer than {:?}", ctx.config.key_timeout))??
                else {
                    bail!("key reply not below modulus");
                };
                keyed = Instant::now();
                let session = derive_session_keys(&secret, &key.modulus_le());
                conn.recv = Some(CtrStream::new(&session.key, &session.iv));
                conn.send = Some(CtrStream::new(&session.key, &session.iv));
                session_keys = Some(session);
                phase = Phase::AwaitLogin { failed: 0 };
                tracing::debug!(%peer, "encryption established");
            }

            (login::OP_LOGIN, Phase::AwaitLogin { failed }) => {
                let Some(request) = login::parse_login_request(payload) else {
                    bail!("malformed login");
                };
                let credentials = String::from_utf8(request.username).ok().zip(String::from_utf8(request.password).ok());
                // The account's name as the limits key it; a name that is not text is only counted against the address.
                let account = credentials.as_ref().map(|c| c.0.to_lowercase());
                // An account in its block is answered "wrong password" from the first byte, without hashing (R7).
                let blocked = account.as_deref().is_some_and(|a| ctx.limits.login_blocked(a, peer.ip()));
                let result = match &credentials {
                    _ if blocked => Some(AuthResult::UnknownAccount),
                    Some((user, pass)) => match ctx.store.authenticate(user, pass, peer.ip()).await {
                        Ok(result) => Some(result),
                        Err(error) => {
                            tracing::error!(%error, "account lookup failed");
                            None
                        }
                    },
                    None => Some(AuthResult::UnknownAccount),
                };
                let name = credentials.as_ref().map(|c| c.0.as_str()).unwrap_or("<non-ascii>");

                match result {
                    Some(AuthResult::Accepted { user_id, privilege }) => {
                        let mut identity = [0u8; 16];
                        rand::rng().fill_bytes(&mut identity);
                        let keys = session_keys.expect("login is only accepted after the key exchange");
                        let prepared = async {
                            ctx.store.create(NewSession { user_id, identity, keys, ip: peer.ip() }).await?;
                            ctx.store.servers().await
                        };
                        match prepared.await {
                            Ok(servers) => {
                                tracing::info!(%peer, user = name, user_id, privilege, "login accepted");
                                if let Some(account) = &account {
                                    ctx.limits.record_success(account, peer.ip());
                                }
                                conn.send(login::login_response(LoginStatus::Success, user_id, privilege, &identity)).await?;
                                let entries: Vec<_> = servers
                                    .iter()
                                    .map(|s| ServerEntry {
                                        id: s.id,
                                        status: if s.open { SERVER_STATUS_NORMAL } else { SERVER_STATUS_CLOSED },
                                        players: if s.open { s.players } else { 0 },
                                        capacity: if s.open { s.capacity } else { SERVER_CLOSED_CAPACITY },
                                        name: s.name.clone(),
                                    })
                                    .collect();
                                for packet in login::server_list(&entries) {
                                    conn.send(packet).await?;
                                }
                                phase = Phase::Authenticated { user_id, privilege, identity };
                            }
                            Err(error) => {
                                tracing::error!(%error, "could not start the session");
                                conn.send(login::login_response(LoginStatus::NotAllowed, 0, 0, &[0; 16])).await?;
                            }
                        }
                    }
                    other => {
                        // Unknown account and wrong password get the same answer; only a right password
                        // reveals the account's status.
                        let (status, guessing) = match other {
                            Some(AuthResult::Refused(status)) => (status, false),
                            Some(_) => (LoginStatus::BadCredentials, true),
                            None => (LoginStatus::NotAllowed, false),
                        };
                        tracing::info!(%peer, user = name, ?status, "login refused");
                        conn.send(login::login_response(status, 0, 0, &[0; 16])).await?;
                        *failed += 1;
                        if *failed >= MAX_FAILED_LOGINS {
                            bail!("too many failed logins on this connection");
                        }
                        if guessing
                            && !blocked
                            && let Some(account) = &account
                        {
                            ctx.limits.record_account_failure(account, peer.ip());
                        }
                        if guessing && ctx.limits.record_failure(peer.ip()) {
                            tracing::warn!(%peer, "too many failed logins, address locked out");
                            bail!("too many failed logins from this address");
                        }
                    }
                }
            }

            (login::OP_SELECT_SERVER, Phase::Authenticated { user_id, privilege, identity }) => {
                let Some((id, version)) = login::parse_select_server(payload) else {
                    bail!("malformed server selection");
                };
                tracing::info!(%peer, server = id, client_version = version, "server selected");
                let server = match ctx.store.server(id).await {
                    Ok(server) => server,
                    Err(error) => {
                        tracing::error!(%error, "server lookup failed");
                        None
                    }
                };
                let staff = *privilege < PRIVILEGE_PLAYER;
                // Same order as the original: connectable, then full (staff may enter a full server), then version.
                let refusal = match &server {
                    None => Some(select_result::CANNOT_CONNECT),
                    Some(s) if !s.open => Some(select_result::CANNOT_CONNECT),
                    Some(s) if s.players >= s.capacity && !staff => Some(select_result::SERVER_FULL),
                    Some(s) if !version_allowed(ctx.config.check_versions, s.version, version) => Some(select_result::VERSION_MISMATCH),
                    Some(_) => None,
                };
                let response = match (refusal, server) {
                    (Some(code), _) => login::select_server_response(code, [0; 4]),
                    (None, Some(server)) => match ctx.store.bind_server(*user_id, *identity, id).await {
                        Ok(()) => login::select_server_response(select_result::SUCCESS, server.address.octets()),
                        Err(error) => {
                            tracing::warn!(%error, "could not bind the session to the server");
                            login::select_server_response(select_result::CANNOT_CONNECT, [0; 4])
                        }
                    },
                    (None, None) => unreachable!("a missing server is always refused"),
                };
                conn.send(response).await?;
            }

            (opcode, _) => bail!("unexpected opcode {opcode:#06x} in this state"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_errors_back_off_and_only_a_gone_listener_ends_serve() {
        let waits: Vec<_> = (1..=7).map(|n| accept_backoff(n).as_millis()).collect();
        assert_eq!(waits, [100, 200, 400, 800, 1000, 1000, 1000]);
        let os = |code| std::io::Error::from_raw_os_error(code);
        assert!(listener_is_gone(&os(9)) && listener_is_gone(&os(88)) && listener_is_gone(&os(22)));
        // EMFILE, ENFILE, ENOMEM, ENOBUFS, ECONNABORTED.
        assert!([24, 23, 12, 105, 103].into_iter().all(|code| !listener_is_gone(&os(code))));
    }
}
