//! Fuzz harness for the wire layer (`docs/server/audit.md` section 5.1): every client-to-server decoder on hostile bytes, round trips
//! through the builders, the frame layer, the encoders at their largest input and the text helpers.

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use shaiya_proto::frame::{self, FrameDecoder, FrameError, HEADER_LEN, MAX_PACKET_LEN};
use shaiya_proto::*;

/// Failing cases go to `proptest-regressions/decoders.txt`: the default place is found from `lib.rs`, which a test file has no sibling of.
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct("proptest-regressions/decoders.txt"))),
        ..ProptestConfig::default()
    }
}

/// Random bytes: short, around the length `n` a decoder accepts (the middle arm reaches the accepting path), or a whole packet.
fn bytes(n: usize) -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![vec(any::<u8>(), 0..64), vec(any::<u8>(), n.saturating_sub(1)..=n + 1), vec(any::<u8>(), 0..=2044)]
}

/// One property per decoder: it returns, whatever it is given.
macro_rules! no_panic {
    ($($name:ident: $decoder:path, $n:expr;)*) => {$(
        proptest! {
            #![proptest_config(config())]
            #[test]
            fn $name(input in bytes($n)) {
                let _ = $decoder(&input);
            }
        }
    )*};
}

no_panic! {
    login_handshake: login::parse_handshake_response, 129;
    login_request_decoder: login::parse_login_request, 48;
    login_select_server: login::parse_select_server, 5;

    game_connect: game::parse_game_connect, 20;
    game_char_create: game::parse_char_create, 29;
    game_char_id: game::parse_char_id, 4;
    game_char_country: game::parse_char_country, 1;
    game_char_rename: game::parse_char_rename, 25;
    game_clean_name: game::clean_name, 21;

    world_move: world::parse_move, 15;
    world_npc_use: world::parse_npc_use, 4;
    world_motion: world::parse_motion, 1;
    world_item_move: world::parse_item_move, 4;
    world_portal: world::parse_portal, 2;
    world_status_up: world::parse_status_up, 12;
    world_gatekeeper: world::parse_gatekeeper, 5;
    world_enchant_level: world::enchant_level, 20;

    skill_learn: skill::parse_learn, 3;
    skill_use: skill::parse_use, 5;
    skill_quick: skill::parse_quick, 11;

    combat_attack: combat::parse_attack, 4;
    combat_rebirth: combat::parse_rebirth_request, 1;

    chat_message: chat::parse_message, 20;
    chat_whisper: chat::parse_whisper, 40;
    chat_markup_is_safe: chat::markup_is_safe, 40;

    party_union_set: party::parse_union_set, 6;
    party_union_move: party::parse_union_move, 8;
    party_id: party::parse_id, 4;
    party_answer: party::parse_answer, 1;
    party_name: party::parse_name, 21;

    trade_id: trade::parse_id, 4;
    trade_byte: trade::parse_byte, 1;
    trade_offer: trade::parse_offer, 4;
    trade_money: trade::parse_money, 4;

    guild_npc_upgrade: guild::parse_npc_upgrade, 3;
    guild_create: guild::parse_create, 90;
    guild_remark: guild::parse_remark, 65;
    guild_yes: guild::parse_yes, 1;
    guild_id: guild::parse_id, 4;
    guild_flag_and_id: guild::parse_flag_and_id, 5;

    friend_name: friend::parse_name, 21;
    friend_yes: friend::parse_yes, 1;
    friend_id: friend::parse_id, 4;
    friend_memo: friend::parse_memo, 40;

    quest_request: quest::parse_request, 6;
    quest_choice: quest::parse_choice, 3;
    quest_id: quest::parse_quest_id, 2;

    shop_buy: shop::parse_buy, 6;
    shop_sell: shop::parse_sell, 3;

    ground_pick_up: ground::parse_pick_up, 4;
    ground_drop: ground::parse_drop, 3;

    item_party_call_answer: item::parse_party_call_answer, 1;
    item_repair: item::parse_repair, 2;
    item_save_point: item::parse_save_point, 12;
    item_body_change: item::parse_body_change, 6;

    gem_link: gem::parse_link, 6;
    gem_extract: gem::parse_extract, 6;
    gem_badge: gem::parse_badge, 8;
    gem_place: gem::parse_place, 2;
    gem_enchant: gem::parse_enchant, 7;
    gem_compose: gem::parse_compose, 4;
    gem_remake: gem::parse_remake, 6;

    market_search: market::parse_search, 37;
    market_same_item: market::parse_same_item, 2;
    market_page: market::parse_page, 1;
    market_register: market::parse_register, 12;
    market_id: market::parse_id, 4;
    market_bid: market::parse_bid, 8;

    my_shop_start: my_shop::parse_start, 40;
    my_shop_add: my_shop::parse_add, 7;
    my_shop_set_price: my_shop::parse_set_price, 5;
    my_shop_buy: my_shop::parse_buy, 2;

    point_buy: point::parse_buy, 21;
    point_gift: point::parse_gift, 42;
    point_slot: point::parse_slot, 1;

    vehicle_ask: vehicle::parse_ask, 4;
    vehicle_answer: vehicle::parse_answer, 1;

    mini_game_sit: mini_game::parse_sit, 1;
    mini_game_guess: mini_game::parse_guess, 1;

    admin_user_question: admin::parse_user_question, 40;
    admin_set_level: admin::parse_set_level, 22;
    admin_name: admin::parse_name, 21;
    admin_name_field: admin::name_field, 25;
    admin_set: admin::parse_set, 26;
    admin_make_item: admin::parse_make_item, 3;
    admin_recall_item: admin::parse_recall_item, 23;
    admin_mob: admin::parse_mob, 3;
    admin_mob_id: admin::parse_mob_id, 4;
    admin_npc: admin::parse_npc, 4;
    admin_ban: admin::parse_ban, 23;
    admin_quest: admin::parse_quest, 2;
    admin_name_quest: admin::parse_name_quest, 23;
    admin_event: admin::parse_event, 42;
    admin_summon_to: admin::parse_summon_to, 31;
    admin_notice: admin::parse_notice, 40;
    admin_repeat_notice: admin::parse_repeat_notice, 40;
    admin_notice_to: admin::parse_notice_to, 60;
    admin_notice_country: admin::parse_notice_country, 40;
    admin_named_text: admin::parse_named_text, 40;
    admin_go: admin::parse_go, 10;
    admin_map: admin::parse_map, 2;
    admin_guild_go: admin::parse_guild_go, 35;
    admin_guild_grade: admin::parse_guild_grade, 47;
    admin_guild_rank: admin::parse_guild_rank, 26;
    admin_guild_points: admin::parse_guild_points, 29;
    admin_call: admin::parse_call, 1;
    admin_auction_recall: admin::parse_auction_recall, 25;
    admin_uq_answer: admin::parse_uq_answer, 30;
    admin_uq_answer_to: admin::parse_uq_answer_to, 50;
}

proptest! {
    #![proptest_config(config())]
    /// `parse_use` takes the opcode too: the three use requests, and any other.
    #[test]
    fn item_use(opcode in prop_oneof![Just(0x050Au16), Just(0x0557), Just(0x0559), any::<u16>()], input in bytes(7)) {
        let _ = item::parse_use(opcode, &input);
    }

    #[test]
    fn admin_family(opcode in any::<u16>()) {
        let _ = admin::family(opcode);
    }

    #[test]
    fn frame_opcode_and_payload(packet in vec(any::<u8>(), HEADER_LEN..=MAX_PACKET_LEN)) {
        prop_assert_eq!(frame::payload(&packet).len(), packet.len() - HEADER_LEN);
        let _ = frame::opcode(&packet);
    }
}

// Text helpers on any bytes (the decoders above reach most; these are the ones the handlers call directly).
proptest! {
    #![proptest_config(config())]
    #[test]
    fn markup_check_ignores_what_follows_the_zero(text in vec(any::<u8>(), 0..200), tail in vec(any::<u8>(), 0..50)) {
        let mut joined = text.iter().copied().filter(|&b| b != 0).collect::<Vec<_>>();
        let safe = chat::markup_is_safe(&joined);
        joined.push(0);
        joined.extend(tail);
        prop_assert_eq!(chat::markup_is_safe(&joined), safe);
    }

    #[test]
    fn names_are_cut_and_clean(field in vec(any::<u8>(), 0..64)) {
        let name = game::clean_name(&field);
        prop_assert!(name.len() <= game::NAME_MAX_LEN);
        prop_assert!(!name.contains(&0) && !name.contains(&b' '));
        let name = admin::name_field(&field);
        prop_assert!(name.len() < field.len().max(1));
        prop_assert!(!name.contains(&0));
    }
}

/// Finite floats only: a coordinate that is not a finite number is refused by the decoder (R21).
fn finite() -> impl Strategy<Value = f32> {
    any::<f32>().prop_filter("finite", |v| v.is_finite())
}

/// A username or password as the client can type it: no zero byte, which ends the field.
fn credential(max: usize) -> impl Strategy<Value = String> {
    vec(1u8..=0x7e, 0..=max).prop_map(|b| String::from_utf8(b).unwrap())
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn round_trip_login(user in credential(login::CREDENTIAL_MAX_LEN), pass in credential(login::PASSWORD_MAX_LEN)) {
        let p = login::login_request(&user, &pass);
        let r = login::parse_login_request(frame::payload(&p)).unwrap();
        prop_assert_eq!((r.username, r.password), (user.into_bytes(), pass.into_bytes()));
    }

    #[test]
    fn round_trip_handshake(cipher in vec(any::<u8>(), 128)) {
        let cipher: [u8; 128] = cipher.try_into().unwrap();
        let p = login::handshake_response(&cipher);
        prop_assert_eq!(login::parse_handshake_response(frame::payload(&p)), Some(cipher));
    }

    #[test]
    fn round_trip_select_server(id in any::<u8>(), version in any::<u32>()) {
        let p = login::select_server_request(id, version);
        prop_assert_eq!(login::parse_select_server(frame::payload(&p)), Some((id, version)));
    }

    #[test]
    fn round_trip_game_connect(user in any::<u32>(), identity in any::<[u8; 16]>()) {
        let p = game::game_connect_request(user, &identity);
        prop_assert_eq!(game::parse_game_connect(frame::payload(&p)), Some((user, identity)));
    }

    #[test]
    fn round_trip_char_create(
        fields in any::<[u8; 8]>(),
        name in vec(prop_oneof![0x21u8..=0x7e, 0x80u8..=0xff], 0..=game::NAME_MAX_LEN),
    ) {
        let [slot, family, grow, hair, face, size, job, sex] = fields;
        let r = game::CreateRequest { slot, family, grow, hair, face, size, job, sex, name, sent_len: 8 + game::NAME_FIELD_LEN };
        let p = game::char_create_request(&r);
        prop_assert_eq!(game::parse_char_create(frame::payload(&p)), Some(r));
    }

    #[test]
    fn round_trip_char_id(opcode in any::<u16>(), id in any::<u32>()) {
        let p = game::char_id_request(opcode, id);
        prop_assert_eq!((frame::opcode(&p), game::parse_char_id(frame::payload(&p))), (opcode, Some(id)));
    }

    #[test]
    fn round_trip_move(motion in any::<u8>(), dir in any::<u16>(), x in finite(), y in finite(), z in finite()) {
        let m = world::Move { motion, dir, position: [x, y, z] };
        let p = world::move_request(&m);
        prop_assert_eq!(world::parse_move(frame::payload(&p)), Some(m));
    }

    #[test]
    fn a_move_with_a_non_finite_coordinate_is_refused(which in 0usize..3, bad in prop_oneof![Just(f32::NAN), Just(f32::INFINITY), Just(f32::NEG_INFINITY)]) {
        let mut position = [1.0f32, 2.0, 3.0];
        position[which] = bad;
        let p = world::move_request(&world::Move { motion: 1, dir: 0, position });
        prop_assert_eq!(world::parse_move(frame::payload(&p)), None);
    }

    #[test]
    fn round_trip_item_move(from in any::<(u8, u8)>(), to in any::<(u8, u8)>()) {
        let p = world::item_move_request(from, to);
        prop_assert_eq!(world::parse_item_move(frame::payload(&p)), Some((from, to)));
    }

    #[test]
    fn round_trip_portal(portal in any::<u8>()) {
        let p = world::portal_request(portal);
        prop_assert_eq!(world::parse_portal(frame::payload(&p)), Some(portal));
    }

    #[test]
    fn round_trip_gatekeeper(npc in any::<u32>(), destination in any::<u8>()) {
        let p = world::gatekeeper_request(npc, destination);
        prop_assert_eq!(world::parse_gatekeeper(frame::payload(&p)), Some((npc, destination)));
    }

    #[test]
    fn round_trip_learn(skill_id in any::<u16>(), level in any::<u8>()) {
        let p = skill::learn_request(skill_id, level);
        prop_assert_eq!(skill::parse_learn(frame::payload(&p)), Some((skill_id, level)));
    }

    #[test]
    fn round_trip_skill_use(opcode in prop_oneof![Just(0x0511u16), Just(0x0517)], number in any::<u8>(), target in any::<u32>()) {
        let p = skill::use_request(opcode, number, target);
        prop_assert_eq!(skill::parse_use(frame::payload(&p)), Some((number, target)));
    }

    #[test]
    fn round_trip_buy(npc_id in any::<u32>(), index in any::<u8>(), count in any::<u8>()) {
        let r = shop::BuyRequest { npc_id, index, count };
        let p = shop::buy_request(&r);
        prop_assert_eq!(shop::parse_buy(frame::payload(&p)), Some(r));
    }

    #[test]
    fn round_trip_sell(bag in any::<u8>(), slot in any::<u8>(), count in any::<u8>()) {
        let p = shop::sell_request(bag, slot, count);
        prop_assert_eq!(shop::parse_sell(frame::payload(&p)), Some((bag, slot, count)));
    }

    #[test]
    fn round_trip_quick(slots in vec((any::<u8>(), any::<u8>(), any::<u8>(), any::<u16>()), 0..skill::MAX_QUICK_SLOTS)) {
        let slots: Vec<_> = slots.into_iter().map(|(bar, slot, bag, number)| skill::QuickSlot { bar, slot, bag, number }).collect();
        let p = skill::char_quick(&slots);
        prop_assert_eq!(skill::parse_quick(frame::payload(&p)), Some(slots));
    }

    /// The count byte is below 128 and the length must match: anything else is refused, never read past the end.
    #[test]
    fn quick_with_a_wrong_count_is_refused(count in any::<u8>(), body in vec(any::<u8>(), 0..700)) {
        let mut payload = vec![count];
        payload.extend(&body);
        let ok = usize::from(count) < skill::MAX_QUICK_SLOTS && body.len() == usize::from(count) * 5;
        prop_assert_eq!(skill::parse_quick(&payload).is_some(), ok);
    }
}

// The frame layer.

/// The stream cut into chunks of the given sizes (the last chunk takes the rest).
fn chunks<'a>(stream: &'a [u8], sizes: &[usize]) -> Vec<&'a [u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    for &size in sizes {
        let end = (at + size).min(stream.len());
        out.push(&stream[at..end]);
        at = end;
    }
    out.push(&stream[at..]);
    out
}

/// A stream that is mostly valid frames with some noise, so the interesting paths (several frames per chunk, a bad length in the
/// middle) are reached more often than with pure noise.
fn stream() -> impl Strategy<Value = Vec<u8>> {
    let frame = (any::<u16>(), vec(any::<u8>(), 0..300)).prop_map(|(op, p)| frame::encode(op, &p));
    let piece = prop_oneof![4 => frame, 1 => vec(any::<u8>(), 0..8)];
    prop_oneof![vec(piece, 0..12).prop_map(|v| v.concat()), vec(any::<u8>(), 0..3000)]
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn the_frame_decoder_holds_for_any_stream_in_any_chunks(stream in stream(), sizes in vec(0usize..700, 0..12)) {
        let mut decoder = FrameDecoder::new();
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let mut failed = false;
        for chunk in chunks(&stream, &sizes) {
            decoder.push(chunk);
            loop {
                match decoder.next_frame() {
                    Ok(Some(f)) => {
                        prop_assert!(!failed, "a frame after an error");
                        prop_assert!((HEADER_LEN..=MAX_PACKET_LEN).contains(&f.len()));
                        prop_assert_eq!(usize::from(u16::from_le_bytes([f[0], f[1]])), f.len());
                        frames.push(f);
                    }
                    Ok(None) => break,
                    Err(FrameError::BadLength(n)) => {
                        prop_assert!(usize::from(n) < HEADER_LEN || usize::from(n) > MAX_PACKET_LEN);
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                break;
            }
        }
        // The frames, joined, are a prefix of the input.
        let joined = frames.concat();
        prop_assert!(stream.starts_with(&joined));
    }

    /// The same stream gives the same frames however it is cut.
    #[test]
    fn chunking_does_not_change_the_frames(stream in stream(), sizes in vec(1usize..400, 1..12)) {
        let drain = |chunks: Vec<&[u8]>| {
            let mut decoder = FrameDecoder::new();
            let mut frames = Vec::new();
            for chunk in chunks {
                decoder.push(chunk);
                loop {
                    match decoder.next_frame() {
                        Ok(Some(f)) => frames.push(f),
                        Ok(None) => break,
                        Err(e) => return (frames, Some(e)),
                    }
                }
            }
            (frames, None)
        };
        prop_assert_eq!(drain(chunks(&stream, &sizes)), drain(vec![&stream[..]]));
    }

    #[test]
    fn encode_comes_back_whole(opcode in any::<u16>(), payload in vec(any::<u8>(), 0..=MAX_PACKET_LEN - HEADER_LEN)) {
        let packet = encode(opcode, &payload);
        prop_assert_eq!(packet.len(), HEADER_LEN + payload.len());
        let mut decoder = FrameDecoder::new();
        decoder.push(&packet);
        prop_assert_eq!(decoder.next_frame(), Ok(Some(packet.clone())));
        prop_assert_eq!(decoder.next_frame(), Ok(None));
        prop_assert_eq!((frame::opcode(&packet), frame::payload(&packet)), (opcode, &payload[..]));
    }
}

// A release build cuts an oversize payload and logs it; a debug build stops on it by design, so only release runs this.
#[cfg(not(debug_assertions))]
proptest! {
    #![proptest_config(config())]
    #[test]
    fn an_oversize_payload_is_cut_not_a_panic(payload in vec(any::<u8>(), MAX_PACKET_LEN - HEADER_LEN + 1..4000)) {
        prop_assert_eq!(encode(0x0501, &payload).len(), MAX_PACKET_LEN);
    }
}

// The encoders at their largest input: whatever a unit holds, a packet stays a valid frame of at most 2,048 bytes.

fn text(max: usize) -> impl Strategy<Value = Vec<u8>> {
    vec(any::<u8>(), 0..=max)
}

fn whole(packet: &[u8]) -> Result<(), TestCaseError> {
    prop_assert!((HEADER_LEN..=MAX_PACKET_LEN).contains(&packet.len()), "{} bytes", packet.len());
    prop_assert_eq!(usize::from(u16::from_le_bytes([packet[0], packet[1]])), packet.len());
    Ok(())
}

fn whole_all(packets: &[Vec<u8>]) -> Result<(), TestCaseError> {
    packets.iter().try_for_each(|p| whole(p))
}

fn party_unit() -> impl Strategy<Value = party::MemberUnit> {
    (
        (any::<u32>(), text(60), any::<u16>(), any::<u8>()),
        (any::<[u16; 3]>(), any::<[u16; 3]>(), any::<u16>()),
        vec(any::<(u16, u8, u32)>(), 0..=party::MAX_UNIT_BUFFS + 10),
    )
        .prop_map(|((id, name, level, job), (max, health, map), buffs)| party::MemberUnit {
            id,
            name,
            level,
            job,
            max,
            health,
            map,
            position: [1.0, 2.0, 3.0],
            buffs,
        })
}

fn item_row() -> impl Strategy<Value = market::ItemRow> {
    (any::<(u8, u8, u16)>(), any::<[u8; 6]>(), any::<u8>(), text(60)).prop_map(
        |((item_type, type_id, quality), gems, count, craft_name)| market::ItemRow { item_type, type_id, quality, gems, count, craft_name },
    )
}

fn market_row() -> impl Strategy<Value = market::MarketRow> {
    (any::<(u32, u32, u32, u32)>(), item_row()).prop_map(|((id, price, direct, end), item)| market::MarketRow {
        id,
        price,
        direct,
        end,
        item,
    })
}

fn end_item() -> impl Strategy<Value = market::EndItem> {
    (any::<(u32, u8, u32)>(), item_row()).prop_map(|((id, result, deadline), item)| market::EndItem { id, result, deadline, item })
}

fn end_money() -> impl Strategy<Value = market::EndMoney> {
    (any::<(u32, u32, u8, u32, u32, u32)>(), item_row()).prop_map(|((money_id, market_id, result, return_money, money, deadline), item)| {
        market::EndMoney { money_id, market_id, result, return_money, money, deadline, item }
    })
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn the_party_list_with_six_others_and_the_most_buffs(units in vec(party_unit(), 0..=6)) {
        whole(&party::list(0, &units))?;
        for unit in &units {
            whole(&party::enter(unit))?;
        }
    }

    #[test]
    fn the_union_lists(units in vec((any::<u8>(), party_unit()), 0..40), rule in any::<u32>()) {
        let settings = party::UnionSettings { leader_slot: 0, sub_leader_slot: 1, rule, auto_join: true };
        whole_all(&party::union_lists(settings, &units))?;
        for (slot, unit) in &units {
            whole(&party::union_enter(*slot, unit))?;
        }
    }

    #[test]
    fn the_friend_and_block_lists(
        friends in vec((any::<(u32, u8, bool)>(), text(60), text(255)), 0..60),
        blocks in vec((any::<u32>(), text(60), text(255)), 0..60),
    ) {
        let friends: Vec<_> = friends.into_iter().map(|((id, job, online), name, memo)| friend::FriendUnit { id, job, online, name, memo }).collect();
        let blocks: Vec<_> = blocks.into_iter().map(|(id, name, memo)| friend::BlockUnit { id, name, memo }).collect();
        whole_all(&friend::friends(&friends))?;
        whole_all(&friend::blocks(&blocks))?;
        let found: Vec<_> = friends.iter().take(friend::MAX_FOUND).map(|f| friend::FoundUnit { level: 1, job: f.job, name: f.name.clone() }).collect();
        if let Some(p) = friend::party_found(&found) {
            whole(&p)?;
        }
    }

    #[test]
    fn the_guild_lists(
        guilds in vec((any::<(u32, u8, u32)>(), text(80), text(80), text(200)), 0..50),
        members in vec((any::<(u32, u8, u16, u8)>(), text(60)), 0..120),
    ) {
        let guilds: Vec<_> = guilds
            .into_iter()
            .map(|((id, ranking, points), name, master, remark)| guild::GuildUnit { id, name, master, remark, ranking, points })
            .collect();
        let members: Vec<_> = members
            .into_iter()
            .map(|((id, rank, level, job), name)| guild::MemberUnit { id, rank, level, job, name })
            .collect();
        let requests: Vec<_> = members.iter().map(|m| guild::RequestUnit { id: m.id, level: m.level, job: m.job, name: m.name.clone() }).collect();
        whole_all(&guild::guild_list(&guilds))?;
        let (online, offline) = members.split_at(members.len() / 2);
        whole_all(&guild::member_list(online, offline))?;
        whole_all(&guild::request_list(&requests))?;
        if let Some(g) = guilds.first() {
            whole(&guild::guild_added(g))?;
        }
    }

    #[test]
    fn the_guild_ranking_and_levels(
        places in vec(any::<(u32, u32, u8)>(), 0..200),
        slots in vec(any::<(u16, u8, u8)>(), 0..200),
        remark in text(300),
    ) {
        let places = &places[..places.len().min(150)];
        whole(&guild::ranking(places))?;
        if let Some(p) = guild::npc_levels(&slots) {
            whole(&p)?;
        }
        whole(&guild::remark(true, &remark))?;
    }

    /// The market's lists: the search and the interest list are one packet each, so their callers cut to a page (`PAGE + 1` rows at most).
    #[test]
    fn the_market_lists(
        found in vec(market_row(), 0..=18),
        sells in vec((market_row(), any::<u32>()), 0..80),
        ends in vec(end_item(), 0..90),
        moneys in vec(end_money(), 0..80),
    ) {
        whole(&market::search(market::OP_PAGE, true, true, &found))?;
        whole(&market::interest_list(&found[..found.len().min(10)]))?;
        let sells: Vec<_> = sells.into_iter().map(|(row, deposit)| market::SellRow { row, deposit }).collect();
        whole_all(&market::sell_list(&sells))?;
        whole_all(&market::bid_list(&found))?;
        whole_all(&market::end_items(&ends))?;
        whole_all(&market::end_moneys(&moneys))?;
    }

    #[test]
    fn the_skill_and_quest_lists(
        skills in vec(any::<(u16, u8, u8, u32)>(), 0..=300),
        active in vec(any::<(u16, u16, [u8; 3])>(), 0..600),
        finished in vec(any::<(u16, bool)>(), 0..900),
    ) {
        let skills: Vec<_> = skills.into_iter().map(|(skill_id, level, number, delay)| skill::SkillEntry { skill_id, level, number, delay }).collect();
        whole(&skill::char_skills(&skills))?;
        let active: Vec<_> = active.into_iter().map(|(quest_id, minutes_left, counts)| quest::ActiveEntry { quest_id, minutes_left, counts }).collect();
        whole_all(&quest::active_list(&active))?;
        whole_all(&quest::finished_list(&finished))?;
    }

    #[test]
    fn the_world_notice(text in "\\PC{0,2100}") {
        match admin::world_notice(&text) {
            Some(p) => whole(&p)?,
            None => prop_assert!(text.is_empty() || text.len() > admin::MAX_WORLD_NOTICE),
        }
    }
}
