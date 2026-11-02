//! Friends, the block list and the party search (`docs/original/friends.md`): every player in the world with its two lists, the
//! waiting friend requests and the party search registrations. The friend packets go from here to the players' outboxes; the zone
//! says who is on which map and who is in a party (`crate::zone::friends`). Every change to the database goes, in order, to the writer
//! task (`run_writer`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use shaiya_proto::chat;
use shaiya_proto::friend::{self as fp, BlockUnit, FoundUnit, FriendUnit};
use tokio::sync::mpsc;

use crate::backend::{FriendStore, FriendWrite, StoredFriend};
use crate::zone::Outbox;

/// Friends and blocked players a character keeps at most (`CFriend`, 100 of each).
pub const MAX_LIST: usize = 100;
/// How long a friend request waits for its answer (`m_dwTimeOverFriend`).
pub const ANSWER_TIME: Duration = Duration::from_secs(30);
/// The longest memo (the column's size).
pub const MAX_MEMO: usize = 50;
/// The party search's level window, either way.
pub const SEARCH_LEVELS: u16 = 5;

/// A memo that may be stored and shown (deviation 3): printable text whose colour markers are safe for the client.
pub fn valid_memo(memo: &[u8]) -> bool {
    memo.len() <= MAX_MEMO && memo.iter().all(|b| (0x20..0x7F).contains(b)) && chat::markup_is_safe(memo)
}

/// A friend or blocked player in a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u32,
    pub name: Vec<u8>,
    pub job: u8,
    pub memo: Vec<u8>,
}

impl From<&StoredFriend> for Entry {
    fn from(f: &StoredFriend) -> Self {
        Self { id: f.id, name: f.name.clone().into_bytes(), job: f.job, memo: f.memo.clone().into_bytes() }
    }
}

/// A player in the world.
#[derive(Clone, Debug)]
pub struct Player {
    pub outbox: Outbox,
    pub name: Vec<u8>,
    pub country: u8,
    pub job: u8,
    pub level: u16,
    pub friends: Vec<Entry>,
    pub blocks: Vec<Entry>,
    /// Who asked this player to be friends, and until when the request waits (`m_dwFriendReqID`).
    pub asked_by: Option<(u32, Instant)>,
    /// Registered for the party search (`m_bPartyFindReg`).
    pub searching: bool,
}

impl Player {
    /// A player entering the world, with its stored lists.
    pub fn new(outbox: Outbox, name: Vec<u8>, country: u8, job: u8, level: u16, friends: &[StoredFriend], blocks: &[StoredFriend]) -> Self {
        Self {
            outbox,
            name,
            country,
            job,
            level,
            friends: friends.iter().map(Entry::from).collect(),
            blocks: blocks.iter().map(Entry::from).collect(),
            asked_by: None,
            searching: false,
        }
    }

    fn send(&self, packet: &[u8]) {
        // A closed outbox means that connection is ending; it leaves by itself.
        let _ = self.outbox.send(packet.to_vec());
    }

    pub fn has_friend(&self, id: u32) -> bool {
        self.friends.iter().any(|f| f.id == id)
    }

    fn has_friend_named(&self, name: &[u8]) -> bool {
        self.friends.iter().any(|f| f.name.eq_ignore_ascii_case(name))
    }

    pub fn has_blocked(&self, id: u32) -> bool {
        self.blocks.iter().any(|b| b.id == id)
    }

    fn entry(&self, me: u32) -> Entry {
        Entry { id: me, name: self.name.clone(), job: self.job, memo: Vec::new() }
    }
}

/// A queued friend write.
pub enum Queued {
    Write(FriendWrite),
    /// Answered once everything queued before it is written (the shutdown waits for it).
    Flush(tokio::sync::oneshot::Sender<()>),
}

/// Writes the friend changes in the order they were made, as long as the server runs. A failed write is tried again after 0.2 s, 1 s
/// and 5 s, then logged with the write at error level (`docs/server/audit.md` R16).
pub async fn run_writer<S: FriendStore>(store: &S, queue: mpsc::UnboundedReceiver<Queued>) {
    run_writer_with(store, queue, &crate::supervise::WRITE_RETRIES).await;
}

async fn run_writer_with<S: FriendStore>(store: &S, mut queue: mpsc::UnboundedReceiver<Queued>, retries: &[Duration]) {
    while let Some(item) = queue.recv().await {
        match item {
            Queued::Write(write) => match crate::supervise::retried(retries, || store.write_friends(&write)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::error!(%error, ?write, "could not write a friend change; given up"),
                Err(panic) => tracing::error!(%panic, ?write, "writing a friend change panicked"),
            },
            Queued::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

#[derive(Default)]
pub struct Friends {
    online: HashMap<u32, Player>,
    writer: Option<mpsc::UnboundedSender<Queued>>,
}

impl Friends {
    pub fn new(writer: mpsc::UnboundedSender<Queued>) -> Self {
        Self { online: HashMap::new(), writer: Some(writer) }
    }

    fn write(&self, write: FriendWrite) {
        if let Some(writer) = &self.writer {
            let _ = writer.send(Queued::Write(write));
        }
    }

    /// Asks the writer to answer once what is queued now is written; `None` without a writer.
    pub fn flush(&self) -> Option<tokio::sync::oneshot::Receiver<()>> {
        let (done, written) = tokio::sync::oneshot::channel();
        self.writer.as_ref()?.send(Queued::Flush(done)).ok()?;
        Some(written)
    }

    fn send(&self, id: u32, packet: &[u8]) {
        if let Some(p) = self.online.get(&id) {
            p.send(packet);
        }
    }

    fn by_name(&self, name: &[u8]) -> Option<u32> {
        self.online.iter().find(|(_, p)| p.name.eq_ignore_ascii_case(name)).map(|(&id, _)| id)
    }

    pub fn player(&self, id: u32) -> Option<&Player> {
        self.online.get(&id)
    }

    /// The lists of the enter-world burst (section 6): `0x2201`, a friend online when it is in the world and lists the player, then
    /// `0x2208`; each only when not empty.
    pub fn burst(&self, me: u32, friends: &[StoredFriend], blocks: &[StoredFriend]) -> Vec<Vec<u8>> {
        let friends: Vec<FriendUnit> = friends
            .iter()
            .map(|f| FriendUnit {
                id: f.id,
                job: f.job,
                online: self.online.get(&f.id).is_some_and(|p| p.has_friend(me)),
                name: f.name.clone().into_bytes(),
                memo: f.memo.clone().into_bytes(),
            })
            .collect();
        let blocks: Vec<BlockUnit> =
            blocks.iter().map(|b| BlockUnit { id: b.id, name: b.name.clone().into_bytes(), memo: b.memo.clone().into_bytes() }).collect();
        let mut packets = fp::friends(&friends);
        packets.extend(fp::blocks(&blocks));
        packets
    }

    /// Entering the world, right after the burst (`CUser::InitFriend`): every friend in the player's list who is in the world gets
    /// `0x2207` (the player, online).
    pub fn connect(&mut self, me: u32, player: Player) {
        let online = fp::online(me, true);
        for f in &player.friends {
            self.send(f.id, &online);
        }
        self.online.insert(me, player);
    }

    /// Leaving the world (`CUser::ReleaseFriend`, `CUser::LeaveZone`): a waiting request is refused (to the asker if it is on the
    /// player's map), and every friend in the player's list who is in the world gets `0x2207` (the player, offline).
    pub fn disconnect(&mut self, me: u32, here: impl Fn(u32) -> bool) {
        self.left_map(me, here);
        let Some(player) = self.online.remove(&me) else { return };
        let offline = fp::online(me, false);
        for f in &player.friends {
            self.send(f.id, &offline);
        }
    }

    /// Leaving a map (`CUser::LeaveZone`): a waiting request is refused, `0x2203` 0 to the asker if it is on that map.
    pub fn left_map(&mut self, me: u32, here: impl Fn(u32) -> bool) {
        let Some((asker, _)) = self.online.get_mut(&me).and_then(|p| p.asked_by.take()) else { return };
        if here(asker) {
            self.send(asker, &fp::answer(fp::REFUSED));
        }
    }

    pub fn level_up(&mut self, me: u32, level: u16) {
        if let Some(p) = self.online.get_mut(&me) {
            p.level = level;
        }
    }

    /// Whether `by` has blocked `who` (deviation 5).
    pub fn blocked(&self, by: u32, who: u32) -> bool {
        self.online.get(&by).is_some_and(|p| p.has_blocked(who))
    }

    /// `me` adds `other` to its list (if missing and there is room), with the `0x2204` to `me`; whether it is in the list now.
    fn add(&mut self, me: u32, other: u32) -> bool {
        let Some(entry) = self.online.get(&other).map(|o| o.entry(other)) else { return false };
        let Some(p) = self.online.get_mut(&me) else { return false };
        if !p.has_friend(other) {
            if p.friends.len() >= MAX_LIST {
                return false;
            }
            p.send(&fp::friend_added(other, entry.job, &entry.name));
            p.friends.push(entry);
        }
        true
    }

    /// `0x2202` (section 3): ask the player named `name`; `here` says whether a player is on the asker's map.
    pub fn request(&mut self, me: u32, name: &[u8], here: impl Fn(u32) -> bool, now: Instant) {
        let Some(asker) = self.online.get(&me) else { return };
        let target = self.by_name(name).filter(|&t| {
            let p = &self.online[&t];
            !asker.has_friend_named(name)
                && here(t)
                && !asker.has_friend(t)
                && t != me
                && p.country == asker.country
                // A request past its time is gone, silently (`CUser::UpdateTimeover`).
                && p.asked_by.is_none_or(|(_, until)| now >= until)
                // Deviation 1: full lists refuse; deviation 5: a player who blocked the asker cannot be asked.
                && asker.friends.len() < MAX_LIST
                && !p.has_blocked(me)
        });
        let Some(target) = target else {
            asker.send(&fp::answer(fp::CANNOT_ASK));
            return;
        };
        let asked = &self.online[&target];
        if asked.has_friend(me) {
            // Already a friend the other way: added at once, and the asker shows as online to the one asked.
            self.send(me, &fp::answer(fp::DONE));
            self.add(me, target);
            self.write(FriendWrite::Friends(vec![(me, target)]));
            self.send(target, &fp::online(me, true));
            return;
        }
        if asked.friends.len() >= MAX_LIST {
            self.send(me, &fp::answer(fp::CANNOT_ASK));
            return;
        }
        let asker_name = self.online[&me].name.clone();
        let asked = self.online.get_mut(&target).expect("found above");
        asked.asked_by = Some((me, now + ANSWER_TIME));
        asked.send(&fp::request(&asker_name));
    }

    /// `0x2203` (section 4): the answer to the waiting request; `here` says whether a player is on the answerer's map.
    pub fn answer(&mut self, me: u32, yes: bool, here: impl Fn(u32) -> bool, now: Instant) {
        let Some((asker, until)) = self.online.get_mut(&me).and_then(|p| p.asked_by.take()) else { return };
        if now >= until || !self.online.contains_key(&asker) || !here(asker) {
            return;
        }
        let (mine, theirs) = (&self.online[&me], &self.online[&asker]);
        let room = |p: &Player, other: u32| p.has_friend(other) || p.friends.len() < MAX_LIST;
        if !yes || mine.has_friend(asker) || !room(mine, asker) || !room(theirs, me) {
            self.send(asker, &fp::answer(fp::REFUSED));
            return;
        }
        // Deviation 4: only what is missing is added, on either side.
        self.write(FriendWrite::Friends(vec![(me, asker), (asker, me)]));
        self.send(asker, &fp::answer(fp::DONE));
        self.add(asker, me);
        self.add(me, asker);
    }

    /// `0x2205` (section 5): one-sided; the removed friend sees the player offline from now on.
    pub fn remove(&mut self, me: u32, id: u32) {
        let Some(p) = self.online.get_mut(&me) else { return };
        let Some(at) = p.friends.iter().position(|f| f.id == id) else { return };
        p.friends.remove(at);
        p.send(&fp::friend_removed(id));
        self.write(FriendWrite::RemoveFriend { character: me, friend: id });
        self.send(id, &fp::online(me, false));
    }

    /// `0x2206` (section 5, deviation 3).
    pub fn friend_memo(&mut self, me: u32, id: u32, memo: &[u8]) {
        if !valid_memo(memo) {
            return;
        }
        let Some(p) = self.online.get_mut(&me) else { return };
        let Some(f) = p.friends.iter_mut().find(|f| f.id == id) else { return };
        let changed = f.memo != memo;
        f.memo = memo.to_vec();
        p.send(&fp::friend_memo(id, memo));
        // The echo goes out either way; a memo that changes nothing is not written (`docs/server/audit.md` R4).
        if !changed {
            return;
        }
        self.write(FriendWrite::FriendMemo { character: me, friend: id, memo: String::from_utf8_lossy(memo).into_owned() });
    }

    /// `0x2209` (section 7): the player named `name`, in the world on any map.
    pub fn block(&mut self, me: u32, name: &[u8]) {
        let Some(p) = self.online.get(&me) else { return };
        let target = self.by_name(name).filter(|&t| {
            !p.blocks.iter().any(|b| b.name.eq_ignore_ascii_case(name))
                && t != me
                && self.online[&t].country == p.country
                && !p.has_blocked(t)
                && p.blocks.len() < MAX_LIST
        });
        let Some(target) = target else {
            p.send(&fp::block_result(fp::BLOCK_FAILED));
            return;
        };
        let entry = self.online[&target].entry(target);
        let p = self.online.get_mut(&me).expect("found above");
        p.send(&fp::block_result(fp::BLOCKED));
        p.send(&fp::block_added(target, &entry.name));
        p.blocks.push(entry);
        self.write(FriendWrite::Block { character: me, blocked: target });
    }

    /// `0x220A` (section 7).
    pub fn unblock(&mut self, me: u32, id: u32) {
        let Some(p) = self.online.get_mut(&me) else { return };
        let Some(at) = p.blocks.iter().position(|b| b.id == id) else { return };
        p.blocks.remove(at);
        p.send(&fp::block_removed(id));
        self.write(FriendWrite::RemoveBlock { character: me, blocked: id });
    }

    /// `0x220B` (section 7, deviation 3).
    pub fn block_memo(&mut self, me: u32, id: u32, memo: &[u8]) {
        if !valid_memo(memo) {
            return;
        }
        let Some(p) = self.online.get_mut(&me) else { return };
        let Some(b) = p.blocks.iter_mut().find(|b| b.id == id) else { return };
        let changed = b.memo != memo;
        b.memo = memo.to_vec();
        p.send(&fp::block_memo(id, memo));
        if !changed {
            return;
        }
        self.write(FriendWrite::BlockMemo { character: me, blocked: id, memo: String::from_utf8_lossy(memo).into_owned() });
    }

    /// `0x220C` (section 8): registered unless in a party; then up to 30 registered players among `on_map` (the players on its map), of
    /// its faction and within 5 levels.
    pub fn party_search(&mut self, me: u32, in_party: bool, on_map: &[u32]) {
        let Some(p) = self.online.get_mut(&me) else { return };
        if !in_party {
            p.searching = true;
        }
        p.send(&fp::party_search_registered());
        let (country, level) = (p.country, p.level);
        let found: Vec<FoundUnit> = on_map
            .iter()
            .filter(|&&id| id != me)
            .filter_map(|id| self.online.get(id))
            .filter(|o| o.searching && o.country == country && o.level.abs_diff(level) <= SEARCH_LEVELS)
            .take(fp::MAX_FOUND)
            .map(|o| FoundUnit { level: o.level, job: o.job, name: o.name.clone() })
            .collect();
        if let Some(packet) = fp::party_found(&found) {
            self.send(me, &packet);
        }
    }

    /// Joining a party ends the party search registration.
    pub fn party_joined(&mut self, me: u32) {
        if let Some(p) = self.online.get_mut(&me) {
            p.searching = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shaiya_proto::frame::{opcode, payload};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

    struct Setup {
        friends: Friends,
        boxes: HashMap<u32, crate::outbox::OutboxInbox>,
        writes: UnboundedReceiver<Queued>,
    }

    fn stored(id: u32, name: &str) -> StoredFriend {
        StoredFriend { id, name: name.into(), job: 1, memo: String::new() }
    }

    impl Setup {
        fn new() -> Self {
            let (writer, writes) = unbounded_channel();
            Self { friends: Friends::new(writer), boxes: HashMap::new(), writes }
        }

        fn join(&mut self, id: u32, name: &str, country: u8, friends: &[StoredFriend]) {
            let (outbox, inbox) = crate::outbox::channel(usize::MAX);
            self.boxes.insert(id, inbox);
            self.friends.connect(id, Player::new(outbox, name.as_bytes().to_vec(), country, 2, 30, friends, &[]));
        }

        /// The opcodes (and first payload byte) each player got since last asked.
        fn got(&mut self, id: u32) -> Vec<(u16, u8)> {
            let inbox = self.boxes.get_mut(&id).unwrap();
            std::iter::from_fn(|| inbox.try_recv().ok()).map(|p| (opcode(&p), payload(&p).first().copied().unwrap_or(0))).collect()
        }

        fn writes(&mut self) -> Vec<FriendWrite> {
            std::iter::from_fn(|| self.writes.try_recv().ok())
                .filter_map(|q| if let Queued::Write(w) = q { Some(w) } else { None })
                .collect()
        }
    }

    struct Flaky {
        fails: std::sync::atomic::AtomicUsize,
        written: std::sync::Mutex<Vec<FriendWrite>>,
    }

    impl FriendStore for Flaky {
        async fn write_friends(&self, write: &FriendWrite) -> anyhow::Result<()> {
            use std::sync::atomic::Ordering::SeqCst;
            if self.fails.load(SeqCst) > 0 {
                self.fails.fetch_sub(1, SeqCst);
                anyhow::bail!("database down");
            }
            self.written.lock().unwrap().push(write.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_failed_friend_write_is_tried_again_and_a_flush_waits_for_the_queue() {
        let store = Flaky { fails: 2.into(), written: Default::default() };
        let (writer, queue) = unbounded_channel();
        let friends = Friends::new(writer);
        friends.write(FriendWrite::Block { character: 1, blocked: 2 });
        let written = friends.flush().expect("a writer");
        friends.write(FriendWrite::RemoveBlock { character: 1, blocked: 2 });
        drop(friends);
        run_writer_with(&store, queue, &[Duration::from_millis(1); 3]).await;
        assert!(written.await.is_ok());
        assert_eq!(
            *store.written.lock().unwrap(),
            [FriendWrite::Block { character: 1, blocked: 2 }, FriendWrite::RemoveBlock { character: 1, blocked: 2 }],
            "in order, the first after two failures"
        );
    }

    const EVERYWHERE: fn(u32) -> bool = |_| true;

    #[test]
    fn asking_and_answering_yes_makes_friends_both_ways() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[]);
        s.join(2, "Bob", 0, &[]);
        let now = Instant::now();
        s.friends.request(1, b"bob", EVERYWHERE, now);
        assert_eq!(s.got(1), []);
        assert_eq!(s.got(2), [(fp::OP_REQUEST, b'A')]);
        s.friends.answer(2, true, EVERYWHERE, now);
        assert_eq!(s.got(1), [(fp::OP_ANSWER, fp::DONE), (fp::OP_FRIEND_ADD, 2)]);
        assert_eq!(s.got(2), [(fp::OP_FRIEND_ADD, 1)]);
        assert_eq!(s.writes(), [FriendWrite::Friends(vec![(2, 1), (1, 2)])]);
        assert!(s.friends.player(1).unwrap().has_friend(2) && s.friends.player(2).unwrap().has_friend(1));
        // A second answer has no request to answer.
        s.friends.answer(2, true, EVERYWHERE, now);
        assert_eq!(s.got(1), []);
    }

    #[test]
    fn requests_fail_for_the_other_faction_another_map_oneself_or_someone_already_asked() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[]);
        s.join(2, "Bob", 1, &[]);
        s.join(3, "Cid", 0, &[]);
        s.join(4, "Dee", 0, &[]);
        let now = Instant::now();
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.request(1, b"Ann", EVERYWHERE, now);
        s.friends.request(1, b"Nobody", EVERYWHERE, now);
        s.friends.request(1, b"Cid", |id| id != 3, now);
        assert_eq!(s.got(1), [(fp::OP_ANSWER, fp::CANNOT_ASK); 4]);
        s.friends.request(1, b"Cid", EVERYWHERE, now);
        s.friends.request(4, b"Cid", EVERYWHERE, now);
        assert_eq!(s.got(4), [(fp::OP_ANSWER, fp::CANNOT_ASK)], "Cid already has a request waiting");
        assert_eq!(s.got(3), [(fp::OP_REQUEST, b'A')]);
    }

    #[test]
    fn a_no_an_asker_gone_from_the_map_and_time_running_out() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[]);
        s.join(2, "Bob", 0, &[]);
        let now = Instant::now();
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.answer(2, false, EVERYWHERE, now);
        assert_eq!(s.got(1), [(fp::OP_ANSWER, fp::REFUSED)]);
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.answer(2, true, |_| false, now);
        assert_eq!(s.got(1), [], "the asker left the map: nothing at all");
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.answer(2, true, EVERYWHERE, now + ANSWER_TIME);
        assert_eq!(s.got(1), [], "expired silently");
        s.join(3, "Cid", 0, &[]);
        s.got(2);
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.request(3, b"Bob", EVERYWHERE, now + ANSWER_TIME / 2);
        assert_eq!(s.got(3), [(fp::OP_ANSWER, fp::CANNOT_ASK)], "Ann's request still waits");
        s.friends.request(3, b"Bob", EVERYWHERE, now + ANSWER_TIME);
        assert_eq!(s.got(2), [(fp::OP_REQUEST, b'A'), (fp::OP_REQUEST, b'C')], "Ann's request ran out");
        s.friends.answer(2, false, EVERYWHERE, now + ANSWER_TIME);
        assert_eq!(s.got(3), [(fp::OP_ANSWER, fp::REFUSED)]);
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.left_map(2, EVERYWHERE);
        assert_eq!(s.got(1), [(fp::OP_ANSWER, fp::REFUSED)], "the one asked left the map");
        assert!(s.writes().is_empty());
    }

    #[test]
    fn asking_someone_who_already_lists_you_adds_at_once() {
        let mut s = Setup::new();
        s.join(2, "Bob", 0, &[stored(1, "Ann")]);
        s.join(1, "Ann", 0, &[]);
        assert_eq!(s.got(2), [], "Ann's list was empty: no online notice");
        s.friends.request(1, b"Bob", EVERYWHERE, Instant::now());
        assert_eq!(s.got(1), [(fp::OP_ANSWER, fp::DONE), (fp::OP_FRIEND_ADD, 2)]);
        assert_eq!(s.got(2), [(fp::OP_ONLINE, 1)]);
        assert_eq!(s.writes(), [FriendWrite::Friends(vec![(1, 2)])]);
    }

    #[test]
    fn online_shows_only_both_ways() {
        let mut s = Setup::new();
        s.join(2, "Bob", 0, &[stored(1, "Ann")]);
        s.join(3, "Cid", 0, &[]);
        let list = s.friends.burst(1, &[stored(2, "Bob"), stored(3, "Cid"), stored(4, "Dee")], &[stored(5, "Eve")]);
        assert_eq!(list.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [fp::OP_FRIENDS, fp::OP_BLOCKS]);
        // Each friend: id, job, online...
        let online: Vec<u8> = payload(&list[0])[1..].chunks(28).map(|unit| unit[5]).collect();
        assert_eq!(online, [1, 0, 0], "Bob lists Ann; Cid does not; Dee is away");
        s.join(1, "Ann", 0, &[stored(2, "Bob"), stored(3, "Cid")]);
        assert_eq!(s.got(2), [(fp::OP_ONLINE, 1)]);
        assert_eq!(s.got(3), [(fp::OP_ONLINE, 1)], "sent to everyone listed; the client ignores ids it does not list");
        s.friends.disconnect(1, EVERYWHERE);
        assert_eq!(s.got(2), [(fp::OP_ONLINE, 1)]);
        assert!(s.friends.burst(9, &[], &[]).is_empty());
    }

    #[test]
    fn removing_is_one_sided_and_the_other_sees_you_go_offline() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[stored(2, "Bob")]);
        s.join(2, "Bob", 0, &[stored(1, "Ann")]);
        s.got(1);
        s.friends.remove(1, 2);
        assert_eq!(s.got(1), [(fp::OP_FRIEND_REMOVE, 2)]);
        assert_eq!(s.got(2), [(fp::OP_ONLINE, 1)]);
        assert!(s.friends.player(2).unwrap().has_friend(1));
        s.friends.remove(1, 2);
        assert_eq!(s.got(1), []);
        assert_eq!(s.writes(), [FriendWrite::RemoveFriend { character: 1, friend: 2 }]);
    }

    #[test]
    fn full_lists_refuse() {
        let mut s = Setup::new();
        let many: Vec<_> = (100..200).map(|i| stored(i, "X")).collect();
        s.join(1, "Ann", 0, &many);
        s.join(2, "Bob", 0, &[]);
        s.friends.request(1, b"Bob", EVERYWHERE, Instant::now());
        assert_eq!(s.got(1), [(fp::OP_ANSWER, fp::CANNOT_ASK)]);
        s.friends.request(2, b"Ann", EVERYWHERE, Instant::now());
        assert_eq!(s.got(2), [(fp::OP_ANSWER, fp::CANNOT_ASK)], "the one asked is full");
    }

    #[test]
    fn crossed_requests_do_not_add_twice() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[]);
        s.join(2, "Bob", 0, &[]);
        let now = Instant::now();
        s.friends.request(1, b"Bob", EVERYWHERE, now);
        s.friends.request(2, b"Ann", EVERYWHERE, now);
        s.friends.answer(2, true, EVERYWHERE, now);
        s.friends.answer(1, true, EVERYWHERE, now);
        assert_eq!(s.friends.player(1).unwrap().friends.len(), 1);
        assert_eq!(s.friends.player(2).unwrap().friends.len(), 1);
        s.got(1);
        assert_eq!(s.got(2).last(), Some(&(fp::OP_ANSWER, fp::REFUSED)), "Ann already has Bob");
    }

    #[test]
    fn memos_are_checked_and_echoed() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[stored(2, "Bob")]);
        s.friends.friend_memo(1, 2, b"<<999c01x");
        s.friends.friend_memo(1, 2, &[b'a'; 51]);
        s.friends.friend_memo(1, 3, b"nobody");
        assert_eq!(s.got(1), []);
        s.friends.friend_memo(1, 2, b"Bob's alt");
        assert_eq!(s.got(1), [(fp::OP_FRIEND_MEMO, 2)]);
        assert_eq!(s.friends.player(1).unwrap().friends[0].memo, b"Bob's alt");
        assert_eq!(s.writes(), [FriendWrite::FriendMemo { character: 1, friend: 2, memo: "Bob's alt".into() }]);
    }

    #[test]
    fn a_memo_that_changes_nothing_is_echoed_but_not_written() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[stored(2, "Bob")]);
        s.friends.friend_memo(1, 2, b"same");
        assert_eq!(s.writes().len(), 1);
        s.got(1);
        s.friends.friend_memo(1, 2, b"same");
        assert_eq!(s.got(1), [(fp::OP_FRIEND_MEMO, 2)], "echoed");
        assert_eq!(s.writes(), [], "not written again");
    }

    #[test]
    fn blocking_needs_the_player_in_the_world_and_of_the_faction() {
        let mut s = Setup::new();
        s.join(1, "Ann", 0, &[]);
        s.join(2, "Bob", 0, &[]);
        s.join(3, "Cid", 1, &[]);
        s.friends.block(1, b"Cid");
        s.friends.block(1, b"Nobody");
        s.friends.block(1, b"Ann");
        assert_eq!(s.got(1), [(fp::OP_BLOCK_RESULT, fp::BLOCK_FAILED); 3]);
        s.friends.block(1, b"bob");
        assert_eq!(s.got(1), [(fp::OP_BLOCK_RESULT, fp::BLOCKED), (fp::OP_BLOCK_ADD, 2)]);
        assert!(s.friends.blocked(1, 2) && !s.friends.blocked(2, 1));
        s.friends.block(1, b"Bob");
        assert_eq!(s.got(1), [(fp::OP_BLOCK_RESULT, fp::BLOCK_FAILED)], "already blocked");
        // A blocked player cannot ask to be friends.
        s.friends.request(2, b"Ann", EVERYWHERE, Instant::now());
        assert_eq!(s.got(2), [(fp::OP_ANSWER, fp::CANNOT_ASK)]);
        s.friends.block_memo(1, 2, b"rude");
        s.friends.unblock(1, 2);
        assert_eq!(s.got(1), [(fp::OP_BLOCK_MEMO, 2), (fp::OP_BLOCK_REMOVE, 2)]);
        assert!(!s.friends.blocked(1, 2));
        assert_eq!(
            s.writes(),
            [
                FriendWrite::Block { character: 1, blocked: 2 },
                FriendWrite::BlockMemo { character: 1, blocked: 2, memo: "rude".into() },
                FriendWrite::RemoveBlock { character: 1, blocked: 2 }
            ]
        );
    }

    #[test]
    fn the_party_search_lists_registered_players_of_the_map_within_five_levels() {
        let mut s = Setup::new();
        for (id, name, country) in [(1, "Ann", 0), (2, "Bob", 0), (3, "Cid", 0), (4, "Dee", 1), (5, "Eve", 0)] {
            s.join(id, name, country, &[]);
        }
        s.friends.level_up(3, 36);
        // Nobody registered yet: registered, nothing found.
        s.friends.party_search(1, false, &[1, 2, 3, 4, 5]);
        assert_eq!(s.got(1), [(fp::OP_PARTY_SEARCH, 1)]);
        for id in [2, 3, 4, 5] {
            s.friends.party_search(id, false, &[]);
        }
        s.friends.party_joined(5);
        s.friends.party_search(1, true, &[1, 2, 3, 4, 5]);
        let found = s.got(1);
        assert_eq!(
            found,
            [(fp::OP_PARTY_SEARCH, 1), (fp::OP_PARTY_FOUND, 1)],
            "Bob only: Cid is 6 levels up, Dee of the other faction, Eve in a party"
        );
        // In a party: answered but not registered.
        s.friends.party_search(5, true, &[]);
        s.friends.party_search(2, false, &[1, 5]);
        assert_eq!(
            s.got(2),
            [(fp::OP_PARTY_SEARCH, 1), (fp::OP_PARTY_SEARCH, 1), (fp::OP_PARTY_FOUND, 1)],
            "Ann is still registered, Eve not"
        );
    }
}
