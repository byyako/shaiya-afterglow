use std::net::Ipv4Addr;
use std::time::Duration;

use login_server::backend::ServerInfo;
use login_server::limits::LimitConfig;
use shaiya_proto::frame;
use shaiya_proto::login::{self, select_result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

mod common;
use common::*;

#[tokio::test]
async fn login_server_list_and_server_selection() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;

    let reply = c.login("alice", "secret").await;
    assert_eq!(frame::opcode(&reply), login::OP_LOGIN);
    assert_eq!(reply.len(), 2 + 24);
    let body = frame::payload(&reply);
    assert_eq!(body[0], 0, "status success");
    assert_eq!(u32::from_le_bytes(body[1..5].try_into().unwrap()), 1, "user id");
    assert_eq!(body[5], 11, "ordinary players get privilege 11");
    assert_ne!(body[6..22], [0u8; 16], "session identity is random");

    let list = c.read().await.expect("server list follows the login reply");
    assert_eq!(frame::opcode(&list), login::OP_SERVER_LIST);
    let body = frame::payload(&list);
    assert_eq!(body[0], 1);
    assert_eq!(body[1], 1, "server id");
    assert_eq!(body[2], 0, "status normal");
    assert_eq!(u16::from_le_bytes(body[3..5].try_into().unwrap()), 0, "players");
    assert_eq!(u16::from_le_bytes(body[5..7].try_into().unwrap()), 500, "capacity");
    assert_eq!(&body[7..18], b"Test Server");
    assert_eq!(body.len(), 1 + 38);

    c.send_encrypted(login::select_server_request(1, 108)).await;
    let pick = c.read().await.unwrap();
    assert_eq!(frame::opcode(&pick), login::OP_SELECT_SERVER);
    assert_eq!(frame::payload(&pick), [select_result::SUCCESS, 10, 1, 2, 3]);
}

#[tokio::test]
async fn gm_privilege_is_sent_to_the_client() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    let reply = c.login("GMBob", "gmpass").await; // account names are case-insensitive
    assert_eq!(frame::payload(&reply)[5], 3);
}

#[tokio::test]
async fn wrong_password_and_unknown_user_look_the_same_and_allow_retry() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;

    let wrong = c.login("alice", "nope").await;
    let unknown = c.login("nobody", "secret").await;
    assert_eq!(frame::payload(&wrong), frame::payload(&unknown));
    assert_eq!(frame::payload(&wrong)[0], 3);
    assert_eq!(frame::payload(&wrong)[1..], [0u8; 21], "no data leaks after the status");

    let ok = c.login("alice", "secret").await;
    assert_eq!(frame::payload(&ok)[0], 0, "same connection can retry");
}

#[tokio::test]
async fn ten_failed_logins_drop_the_connection() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    for _ in 0..10 {
        let reply = c.login("alice", "wrong").await;
        assert_eq!(frame::payload(&reply)[0], 3);
    }
    c.expect_closed().await;
}

#[tokio::test]
async fn a_password_is_read_to_sixteen_characters_and_a_username_to_twelve() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    // The stored password is 6 characters, so extra characters make it wrong.
    let reply = c.login("alice", "secretXXXXXXXXXXXX").await;
    assert_eq!(frame::payload(&reply)[0], 3);
    // A 16-character password works whole, and a 17th character typed is dropped by the client (here: by the builder).
    assert_eq!(frame::payload(&c.login("longpw", "abcdefghijklmnop").await)[0], 0);
}

#[tokio::test]
async fn a_sixteen_character_password_with_no_zero_after_it_is_read_whole() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    // The client's 16 bytes with nothing after them, as a 16-character password goes out.
    let mut body = vec![0u8; 32 + 16];
    body[..6].copy_from_slice(b"longpw");
    body[32..].copy_from_slice(b"abcdefghijklmnop");
    c.send_encrypted(frame::encode(login::OP_LOGIN, &body)).await;
    let reply = c.read().await.expect("login reply");
    assert_eq!(frame::payload(&reply)[0], 0);
}

#[tokio::test]
async fn login_before_encryption_is_refused() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.send_plain(login::login_request("alice", "secret")).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn short_key_reply_is_refused() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    // The original read 128 bytes even when fewer were sent; here it is rejected outright.
    let mut payload = vec![0x80];
    payload.extend_from_slice(&[1u8; 64]);
    c.send_plain(frame::encode(login::OP_HANDSHAKE, &payload)).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn garbage_ciphertext_is_refused() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.send_plain(login::handshake_response(&[0xff; 128])).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn selecting_a_server_before_login_is_refused() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.send_encrypted(login::select_server_request(1, 108)).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn second_key_exchange_is_refused() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.send_encrypted(login::handshake_response(&[1; 128])).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn server_selection_checks_version_and_id() {
    let addr = start(Some(108)).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.login("alice", "secret").await;
    c.read().await.unwrap(); // server list

    c.send_encrypted(login::select_server_request(1, 99)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap()), [select_result::VERSION_MISMATCH, 0, 0, 0, 0]);

    c.send_encrypted(login::select_server_request(9, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap()), [select_result::CANNOT_CONNECT, 0, 0, 0, 0]);

    c.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::SUCCESS);
}

#[tokio::test]
async fn pings_are_ignored_and_unknown_opcodes_disconnect() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.send_plain(frame::encode(login::OP_PING, &[])).await;
    c.handshake().await;
    assert_eq!(frame::payload(&c.login("alice", "secret").await)[0], 0);
    c.read().await.unwrap();
    c.send_encrypted(frame::encode(0x7777, &[])).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn packets_split_across_tcp_writes_are_reassembled() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    let mut packet = login::login_request("alice", "secret");
    c.send.as_mut().unwrap().apply(&mut packet[2..]);
    for chunk in packet.chunks(7) {
        c.stream.write_all(chunk).await.unwrap();
        c.stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let reply = c.read().await.unwrap();
    assert_eq!(frame::payload(&reply)[0], 0);
}

#[tokio::test]
async fn quit_notification_closes_the_connection_quietly() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.send_encrypted(frame::encode(login::OP_LOGIN_TERMINATE, &[])).await;
    c.expect_closed().await;
}

#[tokio::test]
async fn a_closed_game_server_is_listed_closed_and_cannot_be_selected() {
    let b = backend(None);
    b.set_server(ServerInfo { open: false, ..game_server(1, "Test Server", GAME_IP, None) });
    let addr = serve_with(test_config(None), b).await;

    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.login("alice", "secret").await;
    let list = c.read().await.unwrap();
    let body = frame::payload(&list);
    assert_eq!(body[2], login::SERVER_STATUS_CLOSED);
    assert_eq!(u16::from_le_bytes(body[3..5].try_into().unwrap()), 0, "players");
    assert_eq!(u16::from_le_bytes(body[5..7].try_into().unwrap()), login::SERVER_CLOSED_CAPACITY);

    c.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap()), [select_result::CANNOT_CONNECT, 0, 0, 0, 0]);
}

#[tokio::test]
async fn staff_levels_come_from_the_status_value() {
    let addr = start(None).await;
    for (user, pass, privilege) in [("boss", "bosspass", 1), ("gmbob", "gmpass", 3), ("alice", "secret", 11)] {
        let mut c = Client::connect(addr).await;
        c.handshake().await;
        let reply = c.login(user, pass).await;
        assert_eq!(frame::payload(&reply)[0], 0, "{user} can log in");
        assert_eq!(frame::payload(&reply)[5], privilege, "{user}");
    }
}

#[tokio::test]
async fn blocked_accounts_get_their_message_only_with_the_right_password() {
    let addr = start(None).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;

    let wrong = c.login("banned", "nope").await;
    assert_eq!(frame::payload(&wrong)[0], 3, "wrong password looks like any other failure");

    let blocked = c.login("banned", "banpass").await;
    assert_eq!(frame::payload(&blocked)[0], 4, "status 2 is shown as 'cannot log into the game'");
    assert_eq!(frame::payload(&blocked)[1..], [0u8; 21]);

    let leaving = c.login("leaving", "leavepass").await;
    assert_eq!(frame::payload(&leaving)[0], 6, "withdrawal");
}

#[tokio::test]
async fn version_checking_is_off_by_default_and_enforced_when_on() {
    // Off: the server has a version configured but checking is disabled, so any client passes.
    let mut config = test_config(Some(108));
    config.check_versions = false;
    let addr = serve_with(config, backend(Some(108))).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.login("alice", "secret").await;
    c.read().await.unwrap();
    c.send_encrypted(login::select_server_request(1, 0x10042600)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::SUCCESS);

    // On: 0xFFFFFFFF from the client is still a wildcard.
    let addr = start(Some(108)).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.login("alice", "secret").await;
    c.read().await.unwrap();
    c.send_encrypted(login::select_server_request(1, 0x10042600)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::VERSION_MISMATCH);
    c.send_encrypted(login::select_server_request(1, u32::MAX)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::SUCCESS);
}

#[tokio::test]
async fn repeated_failures_lock_the_address_out_across_connections() {
    let mut config = test_config(None);
    config.limits = LimitConfig { failure_limit: 4, ..LimitConfig::default() };
    let addr = start_with(config).await;

    // Two connections of two failures each reach the limit of four.
    for _ in 0..2 {
        let mut c = Client::connect(addr).await;
        c.handshake().await;
        c.login("alice", "wrong").await;
        c.login("alice", "wrong").await;
    }
    // The fourth failure closed the connection it came from; new connections from this address are refused,
    // even with the right password.
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buffer = [0u8; 16];
    let read = timeout(Duration::from_secs(5), stream.read(&mut buffer)).await.expect("closed promptly");
    assert!(matches!(read, Ok(0) | Err(_)), "a locked-out address gets no key packet");
}

#[tokio::test]
async fn one_address_cannot_hold_unlimited_connections() {
    let mut config = test_config(None);
    config.limits = LimitConfig { max_connections: 3, ..LimitConfig::default() };
    let addr = start_with(config).await;

    let mut open = Vec::new();
    for _ in 0..3 {
        open.push(Client::connect(addr).await);
    }
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buffer = [0u8; 16];
    let read = timeout(Duration::from_secs(5), stream.read(&mut buffer)).await.expect("closed promptly");
    assert!(matches!(read, Ok(0) | Err(_)), "the fourth connection is dropped");

    drop(open.pop());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _again = Client::connect(addr).await; // a slot is free again
}

#[tokio::test]
async fn login_stores_a_session_with_the_negotiated_keys() {
    let b = backend(None);
    let addr = serve_with(test_config(None), b.clone()).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    let reply = c.login("alice", "secret").await;
    let user_id = u32::from_le_bytes(frame::payload(&reply)[1..5].try_into().unwrap());
    let identity: [u8; 16] = frame::payload(&reply)[6..22].try_into().unwrap();
    c.read().await.unwrap(); // server list

    let session = b.session(user_id).expect("a session exists after login");
    assert_eq!(session.identity, identity, "the identity the client holds");
    let keys = c.keys.unwrap();
    assert_eq!(session.keys.key, keys.key, "the game server will need the same AES key");
    assert_eq!(session.keys.iv, keys.iv);
    assert_eq!(session.server_id, None, "no server chosen yet");

    c.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::SUCCESS);
    assert_eq!(b.session(user_id).unwrap().server_id, Some(1), "the choice is recorded for the game server");
}

#[tokio::test]
async fn a_newer_login_replaces_the_session_and_the_older_one_cannot_select() {
    let b = backend(None);
    let addr = serve_with(test_config(None), b.clone()).await;

    let mut first = Client::connect(addr).await;
    first.handshake().await;
    let first_reply = first.login("alice", "secret").await;
    let user_id = u32::from_le_bytes(frame::payload(&first_reply)[1..5].try_into().unwrap());
    first.read().await.unwrap();

    let mut second = Client::connect(addr).await;
    second.handshake().await;
    let second_reply = second.login("alice", "secret").await;
    second.read().await.unwrap();
    let newest: [u8; 16] = frame::payload(&second_reply)[6..22].try_into().unwrap();
    assert_eq!(b.session(user_id).unwrap().identity, newest);

    // The first connection no longer owns the session, so it cannot claim a game server.
    first.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&first.read().await.unwrap())[0], select_result::CANNOT_CONNECT);
    assert_eq!(b.session(user_id).unwrap().server_id, None);

    second.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&second.read().await.unwrap())[0], select_result::SUCCESS);
    assert_eq!(b.session(user_id).unwrap().server_id, Some(1));
}

#[tokio::test]
async fn a_full_server_refuses_players_but_lets_staff_in() {
    let b = backend(None);
    b.set_server(ServerInfo { players: 500, ..game_server(1, "Test Server", GAME_IP, None) });
    let addr = serve_with(test_config(None), b).await;

    let mut player = Client::connect(addr).await;
    player.handshake().await;
    player.login("alice", "secret").await;
    let list = player.read().await.unwrap();
    assert_eq!(u16::from_le_bytes(frame::payload(&list)[3..5].try_into().unwrap()), 500, "players are shown");
    player.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&player.read().await.unwrap())[0], select_result::SERVER_FULL);

    let mut staff = Client::connect(addr).await;
    staff.handshake().await;
    staff.login("gmbob", "gmpass").await;
    staff.read().await.unwrap();
    staff.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&staff.read().await.unwrap())[0], select_result::SUCCESS, "staff bypass the limit");
}

#[tokio::test]
async fn several_game_servers_are_listed_each_with_its_own_address() {
    let b = backend(None);
    let second_ip = Ipv4Addr::new(10, 1, 2, 4);
    b.set_server(ServerInfo { players: 123, ..game_server(2, "Second", second_ip, None) });
    b.set_server(ServerInfo { open: false, ..game_server(3, "Maintenance", Ipv4Addr::new(10, 1, 2, 5), None) });
    let addr = serve_with(test_config(None), b).await;

    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.login("alice", "secret").await;
    let list = c.read().await.unwrap();
    let body = frame::payload(&list);
    assert_eq!(body[0], 3, "three servers");
    let entry = |n: usize| &body[1 + n * 38..1 + (n + 1) * 38];
    assert_eq!((entry(0)[0], entry(0)[1]), (1, login::SERVER_STATUS_NORMAL));
    assert_eq!((entry(1)[0], entry(1)[1]), (2, login::SERVER_STATUS_NORMAL));
    assert_eq!(u16::from_le_bytes(entry(1)[2..4].try_into().unwrap()), 123, "live player count");
    assert_eq!((entry(2)[0], entry(2)[1]), (3, login::SERVER_STATUS_CLOSED));
    assert_eq!(&entry(1)[6..12], b"Second");

    c.send_encrypted(login::select_server_request(2, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap()), [select_result::SUCCESS, 10, 1, 2, 4], "the second server's own address");
    c.send_encrypted(login::select_server_request(3, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::CANNOT_CONNECT);
}

#[tokio::test]
async fn a_player_who_types_slowly_is_not_disconnected() {
    // The client connects at start-up and waits at the login screen; it must not be cut off while the player types.
    let mut config = test_config(None);
    config.idle_timeout = Duration::from_secs(5);
    let addr = serve_with(config, backend(None)).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(frame::payload(&c.login("alice", "secret").await)[0], 0, "login still works after a pause");
    c.read().await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    c.send_encrypted(login::select_server_request(1, 108)).await;
    assert_eq!(frame::payload(&c.read().await.unwrap())[0], select_result::SUCCESS, "and so does the server choice");
}

#[tokio::test]
async fn silent_connections_are_still_dropped() {
    let mut config = test_config(None);
    config.key_timeout = Duration::from_millis(300);
    config.idle_timeout = Duration::from_millis(600);
    let addr = serve_with(config, backend(None)).await;

    // Never answers the key exchange: dropped quickly.
    let mut silent = Client::connect(addr).await;
    silent.expect_closed().await;

    // Answers the key exchange, then goes quiet: dropped after the (longer) idle time.
    let mut idle = Client::connect(addr).await;
    idle.handshake().await;
    let started = std::time::Instant::now();
    idle.expect_closed().await;
    assert!(started.elapsed() >= Duration::from_millis(500), "not before the idle timeout");
}

// ---- Audit 2026-10, part 1: R10 ----

/// Pings (`ping`) every 200 ms until the server closes the connection; how long that took. Fails if it lasts past `limit`.
async fn ping_until_closed(c: &mut Client, encrypted: bool, limit: Duration) -> Duration {
    let started = std::time::Instant::now();
    let mut buffer = [0u8; 256];
    loop {
        let ping = shaiya_proto::encode(login::OP_PING, &[]);
        let sent = if encrypted {
            let mut packet = ping;
            c.send.as_mut().unwrap().apply(&mut packet[2..]);
            c.stream.write_all(&packet).await
        } else {
            c.stream.write_all(&ping).await
        };
        if sent.is_err() {
            return started.elapsed();
        }
        if let Ok(read) = timeout(Duration::from_millis(200), c.stream.read(&mut buffer)).await
            && matches!(read, Ok(0) | Err(_))
        {
            return started.elapsed();
        }
        assert!(started.elapsed() < limit, "still open after {limit:?}");
    }
}

#[tokio::test]
async fn pings_do_not_extend_the_key_exchange_deadline() {
    let mut config = test_config(None);
    config.key_timeout = Duration::from_secs(1);
    let addr = start_with(config).await;
    let mut c = Client::connect(addr).await;
    let closed = ping_until_closed(&mut c, false, Duration::from_secs(4)).await;
    assert!(closed >= Duration::from_millis(800) && closed < Duration::from_millis(2000), "{closed:?}");
}

#[tokio::test]
async fn pings_do_not_extend_the_time_from_the_key_exchange_to_the_login() {
    let mut config = test_config(None);
    config.idle_timeout = Duration::from_secs(1);
    let addr = start_with(config).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    let closed = ping_until_closed(&mut c, true, Duration::from_secs(4)).await;
    assert!(closed >= Duration::from_millis(800) && closed < Duration::from_millis(2000), "{closed:?}");
}

#[tokio::test]
async fn after_the_login_any_packet_restarts_the_idle_time() {
    let mut config = test_config(None);
    config.idle_timeout = Duration::from_secs(1);
    let addr = start_with(config).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    c.login("alice", "secret").await;
    c.read().await.expect("server list");
    // Pings for three idle times keep the connection; silence ends it.
    for _ in 0..15 {
        let mut ping = shaiya_proto::encode(login::OP_PING, &[]);
        c.send.as_mut().unwrap().apply(&mut ping[2..]);
        c.stream.write_all(&ping).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    c.send_encrypted(login::select_server_request(1, u32::MAX)).await;
    assert_eq!(frame::opcode(&c.read().await.unwrap()), login::OP_SELECT_SERVER, "still connected after 3 s of pings");
    assert!(c.read().await.is_none(), "closed after a silent idle time");
}

#[tokio::test]
async fn new_connections_are_limited_per_address_and_minute() {
    let mut config = test_config(None);
    config.limits = LimitConfig { connects_per_minute: 3, ..LimitConfig::default() };
    let addr = start_with(config).await;
    for _ in 0..3 {
        drop(Client::connect(addr).await);
    }
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buffer = [0u8; 16];
    let read = timeout(Duration::from_secs(5), stream.read(&mut buffer)).await.expect("closed promptly");
    assert!(matches!(read, Ok(0) | Err(_)), "the fourth in a minute gets no key packet");
}

#[tokio::test]
async fn a_flood_of_key_exchanges_is_decrypted_off_the_async_threads() {
    // Sixteen clients exchange keys at once and log in: the decrypts queue behind the small semaphore and all succeed.
    let addr = start(None).await;
    let mut tasks = Vec::new();
    for _ in 0..16 {
        tasks.push(tokio::spawn(async move {
            let mut c = Client::connect(addr).await;
            c.handshake().await;
            frame::payload(&c.login("alice", "secret").await)[0]
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap(), 0);
    }
}

fn bad_credentials(reply: &[u8]) -> bool {
    frame::payload(reply)[0] == login::LoginStatus::BadCredentials as u8
}

#[tokio::test]
async fn five_wrong_passwords_wait_that_address_out_for_that_account_only() {
    let addr = start(None).await;
    // Five wrong passwords on two connections (the connection cap of failures is 10).
    for tries in [3, 2] {
        let mut c = Client::connect(addr).await;
        c.handshake().await;
        for _ in 0..tries {
            assert!(bad_credentials(&c.login("alice", "wrong").await));
        }
    }
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    assert!(bad_credentials(&c.login("alice", "secret").await), "the right password is refused during the wait");
    // Another account on the same address is not affected, and its login does not lift alice's wait.
    assert_eq!(frame::payload(&c.login("gmbob", "gmpass").await)[0], 0, "gmbob logs in at once");
    let mut again = Client::connect(addr).await;
    again.handshake().await;
    assert!(bad_credentials(&again.login("alice", "secret").await), "gmbob's login did not clear alice's count");
}

#[tokio::test]
async fn an_account_is_blocked_from_every_address_after_enough_wrong_passwords() {
    let mut config = test_config(None);
    config.limits = LimitConfig { pair_failure_limit: 100, account_failure_limit: 4, ..LimitConfig::default() };
    let addr = start_with(config).await;
    let mut c = Client::connect(addr).await;
    c.handshake().await;
    for _ in 0..4 {
        assert!(bad_credentials(&c.login("alice", "wrong").await));
    }
    assert!(bad_credentials(&c.login("alice", "secret").await), "blocked, though the address itself has not been counted out");
    assert_eq!(frame::payload(&c.login("boss", "bosspass").await)[0], 0, "other accounts are untouched");
}
