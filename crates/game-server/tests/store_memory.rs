//! The memory store keeps the parity with `pg.rs` for the guild writes that are one transaction (`docs/server/audit.md` R16); the
//! same scenario as `tests/postgres.rs` runs here without a database.

use game_server::backend::{
    CharacterStore, CreateOutcome, GuildCreated, GuildStore, GuildWrite, ItemStore, NewCharacter, NewGuild, NewItem,
};
use game_server::inventory::SlotChange;
use game_server::memory::MemoryBackend;

fn hero(user_id: u32, slot: u8, name: &str) -> NewCharacter {
    NewCharacter {
        user_id,
        server_id: 1,
        slot,
        name: name.into(),
        family: 0,
        grow: 1,
        hair: 1,
        face: 2,
        size: 3,
        job: 0,
        sex: 0,
        level: 1,
        stat_point: 0,
        skill_point: 5,
        stats: [14, 12, 9, 8, 7, 15],
        health: [250, 85, 195],
        map: 1,
        dir: 0,
        position: [577.98, 78.56, 1762.12],
        money: 0,
        items: vec![NewItem { bag: 1, slot: 0, item_id: 25001, item_type: 25, type_id: 1, quality: 0, count: 10, kept: None }],
    }
}

#[tokio::test]
async fn an_etin_return_and_the_house_purchase_are_each_one_transaction_for_a_member_and_the_master() {
    let backend = MemoryBackend::default();
    let mut ids = Vec::new();
    for (user, name) in [(1, "Master"), (2, "Member"), (3, "Asker"), (4, "Alone")] {
        backend.choose_faction(user, 1, 0).await.unwrap();
        let CreateOutcome::Created(id) = backend.create(&hero(user, 0, name)).await.unwrap() else { panic!("not created") };
        ids.push(id);
    }
    let [master, member, asker, alone] = ids[..] else { unreachable!() };
    let new = NewGuild {
        server_id: 1,
        name: "Knights".into(),
        country: 0,
        remark: String::new(),
        members: vec![(master, 1), (member, 9)],
        payer: master,
        cost: 0,
    };
    let GuildCreated::Created(guild) = backend.create_guild(&new).await.unwrap() else { panic!("not created") };
    backend.write_guild(&GuildWrite::Add { guild, character: asker, rank: 0 }).await.unwrap();
    let etin =
        |slot| SlotChange::Insert(NewItem { bag: 1, slot, item_id: 100_001, item_type: 100, type_id: 1, quality: 0, count: 5, kept: None });
    for who in [member, asker, alone] {
        assert!(backend.trade(who, 0, &[etin(3)]).await.unwrap().is_some());
    }
    let house = || async { backend.load_guilds(1).await.unwrap().remove(0).house };
    let has_item = |who: u32| backend.stored(who).unwrap().1.iter().any(|i| (i.bag, i.slot) == (1, 3));
    let money = |who: u32| backend.stored(who).unwrap().0;

    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_some());
    assert_eq!((has_item(member), house().await.etin, house().await.etin_returns), (false, 50, 1));
    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert!(backend.guild_return_etin(asker, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert!(backend.guild_return_etin(alone, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert_eq!((has_item(asker), has_item(alone), house().await.etin, house().await.etin_returns), (true, true, 50, 1));

    for who in [master, member] {
        backend.set_money(who, 60_000_000);
    }
    assert!(!backend.buy_guild_house(member, guild, 50_000_000, 2000).await.unwrap(), "not the master");
    assert!(!backend.buy_guild_house(master, guild + 99, 50_000_000, 2000).await.unwrap(), "not that guild's master");
    backend.set_money(master, 49_999_999);
    assert!(!backend.buy_guild_house(master, guild, 50_000_000, 2000).await.unwrap(), "too little money");
    assert_eq!((money(master), money(member), house().await.bought), (49_999_999, 60_000_000, false));
    backend.set_money(master, 60_000_000);
    assert!(backend.buy_guild_house(master, guild, 50_000_000, 2000).await.unwrap());
    let bought = house().await;
    assert_eq!((money(master), bought.bought, bought.keep_etin, bought.etin), (10_000_000, true, 2000, 50));
    assert!(!backend.buy_guild_house(master, guild, 50_000_000, 2000).await.unwrap(), "bought already");
    assert_eq!(money(master), 10_000_000, "and nothing was paid again");

    backend.write_guild(&GuildWrite::Disband { guild }).await.unwrap();
    assert!(backend.trade(member, 0, &[etin(3)]).await.unwrap().is_some());
    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert!(has_item(member));
}

#[tokio::test]
async fn pvp_rewards_and_the_stat_reset_are_saved_with_their_steps() {
    use game_server::combat::Progress;
    let backend = MemoryBackend::default();
    backend.choose_faction(1, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(1, 0, "Climber")).await.unwrap() else { panic!("not created") };
    let play = || async { backend.load(1, 1, id).await.unwrap().unwrap() };
    let money = play().await.money;

    assert!(backend.claim_death_reward(id, 500, 0, 1).await.unwrap());
    let after = play().await;
    assert_eq!((after.money, after.kill_steps), (money + 500, [0, 1]));
    assert!(!backend.claim_death_reward(id, 500, 0, 1).await.unwrap(), "the same step again: nothing");
    assert_eq!(play().await.money, money + 500);
    backend.set_money(id, u32::MAX - 10);
    assert!(!backend.claim_death_reward(id, 11, 1, 2).await.unwrap(), "past 32 bits: nothing, the step is not taken");
    let after = play().await;
    assert_eq!((after.money, after.kill_steps), (u32::MAX - 10, [0, 1]));
    assert!(backend.claim_death_reward(id, 10, 1, 2).await.unwrap());
    assert_eq!((play().await.money, play().await.kill_steps), (u32::MAX, [0, 2]));

    let points = play().await.stat_point;
    assert!(backend.claim_kill_reward(id, points + 3, 0, 1).await.unwrap());
    let after = play().await;
    assert_eq!((after.stat_point, after.kill_steps), (points + 3, [1, 2]));
    assert!(!backend.claim_kill_reward(id, points + 6, 0, 1).await.unwrap());
    assert_eq!(play().await.stat_point, points + 3);

    let progress = Progress { level: 2, exp: 144, stat_point: 9, skill_point: 8, stats: [10, 10, 10, 10, 10, 10] };
    backend.save_stat_reset(id, &progress, [260, 90, 200], [0, 2]).await.unwrap();
    let after = play().await;
    assert_eq!((after.stat_point, after.stats, after.kill_steps, after.health), (9, [10; 6], [0, 2], [260, 90, 200]));
}
