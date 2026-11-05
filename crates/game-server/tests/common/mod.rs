//! Helpers shared by the game server's socket tests: the test world, a server over `MemoryBackend` and the client side of the
//! connection. `tests/game_flow.rs` and `tests/fuzz_dispatch.rs` both `mod common;`.
#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use game_server::config::Config;
use game_server::creation::{BaseItemDef, ClassBase, CreationData, place_base_items};
use game_server::enter::ExpTable;
use game_server::inventory::{ItemDef, ItemDefs};
use game_server::memory::{MemoryBackend, StoredSession};
use game_server::mobs::{MobDef, MobDefs, SpawnArea};
use game_server::quests::{QuestDef, QuestDefs, QuestResult};
use game_server::server::{Context, GameData, Live, serve, start_friends, start_guilds};
use game_server::skills::{SkillDef, SkillDefs};
use game_server::world::{Gate, GateKeeper, MapNpcRow, Portal, World};
use sha2::{Digest, Sha256};
use shaiya_crypto::{CtrStream, SessionKeys, XOR_SEED, XorTable};
use shaiya_proto::game::{self as g, CreateRequest};
use shaiya_proto::world as w;
use shaiya_proto::{FrameDecoder, frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

pub const SERVER_ID: u8 = 1;
pub const LOCAL: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
pub const LOGIN_KEYS: SessionKeys = SessionKeys { key: [0x11; 16], iv: [0x22; 16] };

pub fn config(check_ip: bool) -> Config {
    Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        database_url: String::new(),
        server_id: SERVER_ID,
        check_ip,
        heartbeat_interval: Duration::from_millis(50),
        handshake_timeout: Duration::from_secs(5),
        idle_timeout: Duration::from_secs(30),
        delete_delay: Duration::from_secs(7200),
        logout_delay: Duration::from_millis(100),
        linger: Duration::from_millis(300),
        kick_notice: Duration::from_millis(400),
        town_delay: Duration::from_millis(300),
        handshake_wait: Duration::from_secs(1),
        trade_chat_min_level: 10,
        trade_chat_cooldown: Duration::from_millis(300),
        cross_faction_trade_maps: vec![42],
        market_maps: vec![35, 36, 42],
        guild_party_size: 7,
        kill_count: true,
        ranking_times: None,
        ranking_update_delay: Duration::from_secs(1800),
        smooth_chase: false,
        move_check: true,
        item_mall: true,
        game_log: true,
        game_chat_log: true,
        game_log_days: 0,
        max_connections: 50,
        max_per_ip: 100,
        connects_per_minute: 1000,
        write_timeout: Duration::from_secs(20),
        outbox_limit: 4 * 1024 * 1024,
        chat_per_sec: 0,
        chat_burst: 0,
        packets_per_sec: 0,
        db_pool: 24,
        db_acquire: Duration::from_secs(10),
        exp_rate: 1.0,
        kill_rate: 1,
        kill_boost_skills: Vec::new(),
        console_level: 1,
        scripts: true,
        script_memory_mb: 16,
        mini_game: true,
        slot_payout_percent: 220,
        card_win_chances: [420, 200, 130, 100, 80],
        config_path: None,
    }
}

/// Replaces the monsters of data that has not been shared yet.
pub fn set_mobs(data: &mut GameData, mobs: MobDefs) {
    Arc::get_mut(data.live.get_mut().unwrap()).unwrap().mobs = mobs;
}

pub fn data() -> GameData {
    data_with(false)
}

/// The test world, with the gambling house's NPC (type 5, object id 8) beside the human start when `gambling` is set.
pub fn data_with(gambling: bool) -> GameData {
    let b = |hp, sp, mp| ClassBase { hp, sp, mp };
    // Human fighter: a sword (type 5) and ten potions (type 25), as in defs.base_items_defs.
    let row = |item_type: u8, count| BaseItemDef {
        family: 0,
        job: 0,
        item_id: u32::from(item_type) * 1000 + 1,
        item_type,
        type_id: 1,
        count,
        quality: 1200,
    };
    let creation = CreationData {
        class_base: [b(205, 125, 50), b(250, 65, 65), b(170, 150, 60), b(175, 135, 70), b(150, 60, 170), b(160, 70, 150)],
        start: CreationData::original_start_positions(),
        base_items: place_base_items([row(5, 1), row(25, 10)]),
    };
    // Two merchants on map 1: one beside the human start (577.98, 1762.12), one far away.
    let merchant = |type_id, x, z| MapNpcRow { map: 1, kind: 1, type_id, first: Some([x, 78.0, z, 0.0]), walk: Vec::new() };
    let gatekeeper = |type_id, x, z| MapNpcRow { map: 1, kind: 2, type_id, first: Some([x, 78.0, z, 0.0]), walk: Vec::new() };
    // Object ids in this order: merchants 1 and 2; gatekeeper 3 beside the start, 4 at its first destination, 5 (Fury) next to 4;
    // warehouse keeper 6 (NPC type 6) and blacksmith 7 (NPC type 3) beside the start; gambling NPC 8 (type 5) beside it too.
    let mut world = World::build_with(
        [(1, 2048), (2, 2048), (3, 256), (35, 1024), (42, 256)],
        &std::collections::HashSet::from([(1, 1), (1, 2), (2, 1), (2, 2), (6, 1), (3, 1), (5, 1)]),
        [
            merchant(1, 590.0, 1770.0),
            merchant(2, 1500.0, 300.0),
            gatekeeper(1, 600.0, 1775.0),
            gatekeeper(1, 1010.0, 1000.0),
            gatekeeper(2, 1005.0, 1010.0),
            MapNpcRow { map: 1, kind: 6, type_id: 1, first: Some([595.0, 78.0, 1768.0, 0.0]), walk: Vec::new() },
            MapNpcRow { map: 1, kind: 3, type_id: 1, first: Some([592.0, 78.0, 1772.0, 0.0]), walk: Vec::new() },
            MapNpcRow { map: 1, kind: 5, type_id: 1, first: Some([594.0, 78.0, 1774.0, 0.0]), walk: Vec::new() },
        ],
        gambling,
    );
    let gate = |map, position, cost| Some(Gate { map, position, cost });
    let gates = [gate(1, [1000.0, 80.0, 1000.0], 100), gate(2, [165.25, 119.4, 397.89], 200), None];
    world.set_gatekeepers([(1, GateKeeper { country: 0, gates }), (2, GateKeeper { country: 1, gates })].into());
    // Portals beside the human start: 0 Light only, levels 1 to 20, to map 2; 1 on the same map; 2 to a party instance zone (map 3);
    // 3 Fury only.
    let portal = |country, to_map, to| Portal { position: [585.0, 78.0, 1765.0], country, level_min: 1, level_max: 20, to_map, to };
    world.set_portals(
        [(
            1,
            vec![
                portal(1, 2, [165.25, 118.5, 397.89]),
                portal(0, 1, [1200.0, 80.0, 1200.0]),
                portal(0, 3, [10.0, 0.0, 10.0]),
                portal(2, 2, [165.25, 118.5, 397.89]),
            ],
        )]
        .into(),
        [(3, game_server::world::create_type::PARTY)].into(),
    );
    // Both sell potions (index 0) and swords (index 1).
    let goods = vec![(25, 1), (5, 1)];
    world.set_shops([(1, goods.clone()), (2, goods)].into());
    let item = |item_type: u8, max_count, buy, sell| ItemDef {
        item_id: u32::from(item_type) * 1000 + 1,
        item_type,
        type_id: 1,
        max_count,
        quality: 1200,
        buy,
        sell,
        // Anyone may equip them.
        level: 1,
        country: 6,
        jobs: [true; 6],
        ..Default::default()
    };
    // The sword hits hard (effect 1, attack 200): one blow kills the monsters below.
    // Bound (reqog 1): it cannot be traded.
    let sword = ItemDef { effects: [200, 0, 0, 0], bound: true, room: 1, ..item(5, 1, 500, 100) };
    // The potions heal 50 HP (item rows keep HP, MP, SP), in reuse group 1 (15 s). A Stat Reset Stone (special 34) and a firecracker
    // (special 46) for level 10 and above.
    let potion = ItemDef { const_health: [50, 0, 0], reuse_group: 1, ..item(25, 100, 10, 2) };
    let stone = ItemDef { item_id: 100048, type_id: 48, special: 34, ..item(100, 1, 0, 0) };
    let firecracker = ItemDef { item_id: 100115, type_id: 115, special: 46, level: 10, ..item(100, 10, 0, 0) };
    // A Capital Recall Rune (special 47): Light to map 35.
    let rune = ItemDef { item_id: 100131, type_id: 131, special: 47, ..item(100, 10, 0, 0) };
    // The Appearance Change Scroll (special 44, stacks of 5) and the Sex Change Scroll (45).
    let appearance = ItemDef { item_id: 100070, type_id: 70, special: 44, ..item(100, 5, 0, 0) };
    let sex_change = ItemDef { item_id: 100071, type_id: 71, special: 45, ..item(100, 1, 0, 0) };
    // A mount (type 42): mode 1, range 1, 10 s before the next summon.
    let mount = ItemDef { item_id: 42001, grow: 1, range: 1, reuse_group: 10, ..item(42, 1, 0, 0) };
    // A weapon lapis of grade 31 (price 4095) and a cloak.
    let lapis = ItemDef { item_id: 30001, room: 1, reuse_group: 31, ..item(30, 10, 0, 0) };
    let cloak = ItemDef { item_id: 24001, ..item(24, 1, 0, 0) };
    // Message To Server (special 38), with no reuse time here so that only the armed check refuses a second one.
    let message = ItemDef { item_id: 100064, type_id: 64, special: 38, ..item(100, 1, 0, 0) };
    // A throwing item naming skill 7 (a 50-damage strike reaching 40) for `0x0559` kinds 1 and 2.
    let snowball = ItemDef { item_id: 44147, type_id: 147, range: 7, attack_time: 1, ..item(44, 10, 0, 0) };
    // The Name Change Stone (special 51), with no reuse time here so that only the once-per-entry check refuses a second one.
    let name_stone = ItemDef { item_id: 100136, type_id: 136, special: 51, ..item(100, 1, 0, 0) };
    let items = ItemDefs::new([
        potion,
        sword,
        stone,
        firecracker,
        rune,
        appearance,
        sex_change,
        mount,
        lapis,
        cloak,
        message,
        snowball,
        name_stone,
    ]);
    // Normal mode (grow 2 in defs.exp_defs): 100 to reach level 2.
    // Level 1 of the status table (HP, SP, MP), as in defs.status_defs.
    let status = game_server::stats::StatusTable::new(
        [[205, 125, 50], [250, 65, 65], [170, 150, 60], [175, 135, 70], [150, 60, 170], [160, 70, 150]]
            .into_iter()
            .enumerate()
            .map(|(job, row)| (job as u8, 1, row))
            .chain([(0, 2, [215, 130, 55])]),
    );
    // Monster 1 (HP 120, respawning at once): two in an area of one spot beside the human start, one far away.
    // It gives 100 experience: 144 for a level 1 killer, past the 100 of level 2.
    let mobs = MobDefs::new([MobDef { id: 1, level: 1, hp: 120, respawn: Duration::ZERO, exp: 100, country: 2, ..Default::default() }]);
    let area = |x: f32, z: f32, count| SpawnArea::new([x, 78.0, z, x + 0.5, 78.0, z + 0.5], vec![(1, count)]);
    world.set_mob_data(Default::default(), [(1, vec![area(600.0, 1780.0, 2), area(1500.0, 300.0, 1)])].into());
    let drops = game_server::drops::DropTables::new(&items);
    // Skills: 1 a passive (REC +10) at levels 1 (2 points) and 2 (level 5); 2 an active skill (2 points); 3 the bag passive (1 point);
    // 4 another (1 point).
    let skill = |id, level, points, required_level| SkillDef {
        id,
        level,
        country: 6,
        jobs: [true; 6],
        points,
        required_level,
        type_show: 1,
        type_effect: 1,
        ..Default::default()
    };
    // 5 and 6 (1 point each): 50 damage on one monster with a sword (real type 5), always hitting, for 10 SP; a 30 s cooldown. 6 takes
    // a second to cast.
    let strike = |id, ready_time| {
        let mut need_weapons = [false; 15];
        need_weapons[4] = true;
        SkillDef {
            target_type: 3,
            kind: 21,
            need_weapons,
            need_sp: 10,
            reset_time: 30,
            ready_time,
            damage: [50, 0, 0],
            success_type: 1,
            success_value: 100,
            ..skill(id, 1, 1, 1)
        }
    };
    let skills = SkillDefs::new([
        SkillDef { type_effect: 0, kind: 4, abilities: [(5, 10), (0, 0), (0, 0)], ..skill(1, 1, 2, 1) },
        SkillDef { type_effect: 0, kind: 4, abilities: [(5, 20), (0, 0), (0, 0)], ..skill(1, 2, 2, 5) },
        skill(2, 1, 2, 1),
        SkillDef { type_effect: 0, kind: 2, bags: 3, ..skill(3, 1, 1, 1) },
        skill(4, 1, 1, 1),
        strike(5, 0),
        strike(6, 4),
        SkillDef { attack_range: 40, ..strike(7, 0) },
    ]);
    // Quests at the merchant beside the start (type 1, type id 1): 1 kill one monster 1 and come back, for 50 experience (doubled),
    // 100 gold and a potion; 2 taken from any NPC, handed in at the gatekeeper (repeatable); 3 only after 1.
    let quest = |id| QuestDef {
        id,
        level_min: 1,
        level_max: 60,
        country: 6,
        jobs: [true; 6],
        start_type: 1,
        start_npc: (1, 1),
        ..Default::default()
    };
    let reward = QuestResult { exp: 50, money: 100, item: (25, 1), ..Default::default() };
    let quests = QuestDefs::new(vec![
        QuestDef {
            end_type: 2,
            end_npc: (1, 1),
            mob_kills: [(1, 1), (0, 0)],
            result_type: 1,
            results: [reward, QuestResult::default(), QuestResult::default()],
            ..quest(1)
        },
        QuestDef { start_type: 0, start_npc: (0, 0), end_type: 0, end_npc: (2, 1), repeatable: true, result_type: 1, ..quest(2) },
        QuestDef { prev: [1, 0, 0], end_type: 0, end_npc: (1, 1), result_type: 1, ..quest(3) },
        // 4 gives the stone (100/48) when it starts: for the operator's `F80B` and `F80C`.
        QuestDef { start_items: [(100, 48, 1), (0, 0, 0), (0, 0, 0)], end_type: 0, end_npc: (1, 1), result_type: 1, ..quest(4) },
    ]);
    GameData {
        creation,
        exp: ExpTable::from_rows([(1, 2, 100), (2, 2, 200)]),
        world,
        items,
        status,
        live: Live { mobs, drops }.shared(),
        skills,
        quests,
        kill_grades: Default::default(),
        products: products(),
    }
}

/// The item mall's products of these tests: 150 potions and a stone for 300 points, a stone for 50, one with an undefined item.
pub fn products() -> game_server::mall::Products {
    game_server::mall::Products::new([
        ("Potions".to_owned(), 300, vec![(25001, 150), (100048, 1)]),
        ("Stone".to_owned(), 50, vec![(100048, 1)]),
        ("Bad".to_owned(), 10, vec![(99001, 1)]),
    ])
}

pub fn session(identity: [u8; 16], server_id: Option<u8>, login_ip: IpAddr) -> StoredSession {
    StoredSession { identity, keys: LOGIN_KEYS, login_ip, server_id, in_game: false, privilege: 11, created: 1 }
}

pub async fn start(backend: Arc<MemoryBackend>, check_ip: bool) -> (SocketAddr, Arc<Context<Arc<MemoryBackend>>>) {
    start_with(backend, config(check_ip)).await
}

pub async fn start_with(backend: Arc<MemoryBackend>, config: Config) -> (SocketAddr, Arc<Context<Arc<MemoryBackend>>>) {
    start_with_data(backend, config, data()).await
}

/// A server whose world has the gambling house's NPC beside the human start.
pub async fn start_gambling(backend: Arc<MemoryBackend>) -> (SocketAddr, Arc<Context<Arc<MemoryBackend>>>) {
    start_with_data(backend, config(true), data_with(true)).await
}

pub async fn start_with_data(
    backend: Arc<MemoryBackend>,
    config: Config,
    data: GameData,
) -> (SocketAddr, Arc<Context<Arc<MemoryBackend>>>) {
    let mut ctx = Context::new(config, backend, data);
    // The log is kept in memory, for the tests that read it (`recorded`).
    ctx.set_game_log(game_server::game_log::GameLog::memory(true).0);
    let ctx = Arc::new(ctx);
    start_guilds(&ctx).await.unwrap();
    start_friends(&ctx);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, ctx.clone()));
    (addr, ctx)
}

/// The client side of the game connection, following the spec: plain-text handshake, then AES-CTR in both directions
/// with the login key and SHA-256(login IV)[0..16] as the starting counter; like game.exe, it decrypts server packets
/// with the XOR table instead of AES from the packet after `0x010B` on.
pub struct Client {
    pub stream: TcpStream,
    pub decoder: FrameDecoder,
    pub send: Option<CtrStream>,
    pub recv: Option<CtrStream>,
    pub xor: Option<XorTable>,
    /// The `0xB103` the handshake brought.
    pub billing: Vec<u8>,
}

impl Client {
    pub async fn connect(addr: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(addr).await.unwrap(),
            decoder: FrameDecoder::new(),
            send: None,
            recv: None,
            xor: None,
            billing: Vec::new(),
        }
    }

    /// A client whose receive buffer is as small as the kernel allows, so it can stop reading and have the server's writes wait soon.
    pub async fn connect_small(addr: SocketAddr) -> Self {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        socket.set_recv_buffer_size(4096).unwrap();
        Self {
            stream: socket.connect(addr).await.unwrap(),
            decoder: FrameDecoder::new(),
            send: None,
            recv: None,
            xor: None,
            billing: Vec::new(),
        }
    }

    pub fn start_encryption(&mut self) {
        let counter: [u8; 16] = Sha256::digest(LOGIN_KEYS.iv)[..16].try_into().unwrap();
        self.send = Some(CtrStream::new(&LOGIN_KEYS.key, &counter));
        self.recv = Some(CtrStream::new(&LOGIN_KEYS.key, &counter));
    }

    pub async fn write(&mut self, mut packet: Vec<u8>) {
        if let Some(cipher) = &mut self.send {
            cipher.apply(&mut packet[2..]);
        }
        self.stream.write_all(&packet).await.unwrap();
    }

    /// Next packet, or `None` when the server closed the connection.
    pub async fn read(&mut self) -> Option<Vec<u8>> {
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(mut packet) = self.decoder.next_frame().unwrap() {
                if let Some(table) = &self.xor {
                    table.apply(&mut packet[2..]);
                    // Like game.exe's 0x0107 handler: back to AES after it.
                    if frame::opcode(&packet) == w::OP_LOGOUT_TO_SELECT {
                        self.xor = None;
                    }
                } else if let Some(cipher) = &mut self.recv {
                    cipher.apply(&mut packet[2..]);
                    if frame::opcode(&packet) == w::OP_CHAR_QUICK {
                        self.xor = Some(XorTable::new(&XOR_SEED));
                    }
                }
                return Some(packet);
            }
            // A reset connection counts as closed.
            let n =
                timeout(Duration::from_secs(5), self.stream.read(&mut buffer)).await.expect("server answered in time").unwrap_or_default();
            if n == 0 {
                return None;
            }
            self.decoder.push(&buffer[..n]);
        }
    }

    /// Handshake and the character screen that follows: returns (country, max grow, list records).
    pub async fn enter(&mut self, user_id: u32, identity: [u8; 16]) -> (u8, u8, Vec<Vec<u8>>) {
        self.write(g::game_connect_request(user_id, &identity)).await;
        self.finish_enter().await
    }

    /// `enter` once the handshake has been sent.
    pub async fn finish_enter(&mut self) -> (u8, u8, Vec<Vec<u8>>) {
        self.start_encryption();
        let reply = self.read().await.expect("handshake reply");
        assert_eq!(frame::opcode(&reply), g::OP_GAME_CONNECT);
        assert_eq!(frame::payload(&reply), [&[0u8, 2][..], b"LENASOFTFOREVER!"].concat());
        let billing = self.read().await.unwrap();
        assert_eq!(frame::opcode(&billing), g::OP_BILLING_INFO);
        assert_eq!(frame::payload(&billing).len(), 5);
        self.billing = frame::payload(&billing).to_vec();
        let country = self.read().await.unwrap();
        assert_eq!(frame::opcode(&country), g::OP_CHAR_COUNTRY);
        let body = frame::payload(&country);
        let list = self.read_list().await;
        (body[0], body[1], list)
    }

    /// Five `0x0101` records.
    pub async fn read_list(&mut self) -> Vec<Vec<u8>> {
        let mut list = Vec::new();
        for _ in 0..5 {
            let packet = self.read().await.unwrap();
            assert_eq!(frame::opcode(&packet), g::OP_CHAR_LIST);
            list.push(frame::payload(&packet).to_vec());
        }
        list
    }
}

/// Waits until the account's connection has ended, which is after everything is saved (a character staying after a lost
/// connection included).
pub async fn wait_gone(backend: &MemoryBackend, user_id: u32, within: Duration) {
    timeout(within, async {
        while backend.session(user_id).is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the connection ended in time");
}

pub fn backend_with_session(identity: [u8; 16]) -> Arc<MemoryBackend> {
    let backend = Arc::new(MemoryBackend::default());
    backend.add_session(7, session(identity, Some(SERVER_ID), LOCAL));
    backend
}

pub fn create(slot: u8, family: u8, job: u8, name: &[u8]) -> Vec<u8> {
    g::char_create_request(&CreateRequest {
        slot,
        family,
        grow: 1,
        hair: 3,
        face: 4,
        size: 5,
        job,
        sex: 1,
        name: name.to_vec(),
        sent_len: 0,
    })
}

/// After the client's 0x0201: its own 0x0201 and 0x051C, those of the players in view, the NPCs in view, the weather, then the
/// player's HP, MP and SP (0x0521). Returns the 0x0201s and the 0x0E01s.
pub async fn read_zone_entry(client: &mut Client) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let (mut enters, mut npcs) = (Vec::new(), Vec::new());
    loop {
        let packet = client.read().await.expect("zone entry");
        match frame::opcode(&packet) {
            w::OP_USER_ENTER => enters.push(packet),
            w::OP_NPC_ENTER => npcs.push(packet),
            w::OP_MOB_ENTER => {}
            w::OP_SPEED => {}
            w::OP_WEATHER => {
                let health = client.read().await.expect("current HP, MP and SP");
                assert_eq!(frame::opcode(&health), shaiya_proto::combat::OP_RECOVER_SET);
                return (enters, npcs);
            }
            other => panic!("unexpected {other:#06x} in the zone entry"),
        }
    }
}

/// Selects the first character after the handshake and enters the world; returns its id.
pub async fn enter_world(client: &mut Client, user: u32, identity: [u8; 16], create_first: bool) -> u32 {
    client.enter(user, identity).await;
    if create_first {
        client.write(shaiya_proto::encode(g::OP_CHAR_COUNTRY, &[0])).await;
        client.write(create(0, 0, 0, b"Walker")).await;
        assert_eq!(client.read().await.unwrap(), g::char_create_response(0));
        client.read_list().await;
    }
    select_and_enter(client).await
}

/// From the character screen with the list received: select the character in slot 0, read the burst, send 0x0201.
pub async fn select_and_enter(client: &mut Client) -> u32 {
    let id = 1;
    client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
    while frame::opcode(&client.read().await.expect("burst")) != w::OP_CHAR_QUICK {}
    client.write(shaiya_proto::encode(w::OP_USER_ENTER, &[])).await;
    read_zone_entry(client).await;
    id
}

pub fn step(x: f32, z: f32) -> Vec<u8> {
    w::move_request(&w::Move { motion: 0, dir: 90, position: [x, 78.56, z] })
}
