//! The user questions (`/request GM`, `docs/original/gm-commands-2.md` section 3 with deviations 1 to 4): the queue the staff take
//! them from, the pure rules. The database copy is `AdminStore`'s; the queue holds what is open.

use std::collections::{BTreeMap, HashMap};

/// The most unanswered questions one character may have (deviation 4).
pub const PER_CHARACTER: usize = 3;

/// One question. `id` is the table's id: it grows with the age, so it is also the queue order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub id: u32,
    pub character_id: u32,
    pub name: Vec<u8>,
    /// The text with its zero.
    pub text: Vec<u8>,
    /// The packed local time of asking (`admin::packed_time`).
    pub time: u32,
}

/// What `/UQGET` gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Taken {
    /// The GM held a question already: the same one again, with the `n` of `F201` (the questions still waiting, plus one).
    Again(Question, u16),
    /// The oldest waiting question, now the GM's, with the `n` of `F201` (the waiting count before it was taken).
    New(Question, u16),
    Empty,
}

/// Every open question of the server: waiting in order, and those the staff hold.
#[derive(Debug, Default)]
pub struct QuestionQueue {
    waiting: BTreeMap<u32, Question>,
    in_process: HashMap<u32, Question>,
    /// GM character id -> the id of the question it holds.
    held: HashMap<u32, u32>,
}

impl QuestionQueue {
    /// The open questions of a restart, all waiting.
    pub fn load(questions: impl IntoIterator<Item = Question>) -> Self {
        Self { waiting: questions.into_iter().map(|q| (q.id, q)).collect(), ..Self::default() }
    }

    fn unanswered_of(&self, character: u32) -> usize {
        self.waiting.values().chain(self.in_process.values()).filter(|q| q.character_id == character).count()
    }

    /// Whether the character may ask another question now.
    pub fn may_ask(&self, character: u32) -> bool {
        self.unanswered_of(character) < PER_CHARACTER
    }

    /// Queues a question at the back. `false` when its character already has three unanswered ones.
    pub fn ask(&mut self, question: Question) -> bool {
        if !self.may_ask(question.character_id) {
            return false;
        }
        self.waiting.insert(question.id, question);
        true
    }

    /// `/UQGET` of the GM `gm`.
    pub fn take(&mut self, gm: u32) -> Taken {
        let waiting = self.waiting.len();
        if let Some(question) = self.held.get(&gm).and_then(|id| self.in_process.get(id)) {
            return Taken::Again(question.clone(), waiting as u16 + 1);
        }
        let Some((id, question)) = self.waiting.pop_first() else { return Taken::Empty };
        self.in_process.insert(id, question.clone());
        self.held.insert(gm, id);
        Taken::New(question, waiting as u16)
    }

    /// `F202` / `F203` of `gm` for question `id`: the GM's own hold ends whatever the id, then the question leaves the queue for good
    /// (any GM's) and every hold on it ends (deviation 3). `None` for a question nobody holds.
    pub fn answer(&mut self, gm: u32, id: u32) -> Option<Question> {
        self.held.remove(&gm);
        let question = self.in_process.remove(&id)?;
        self.held.retain(|_, held| *held != id);
        Some(question)
    }

    /// A GM leaving the world: its question goes back to the front of the waiting list (its id is older than the rest). Returns the id.
    pub fn give_back(&mut self, gm: u32) -> Option<u32> {
        let id = self.held.remove(&gm)?;
        let question = self.in_process.remove(&id)?;
        self.waiting.insert(id, question);
        Some(id)
    }

    pub fn waiting_count(&self) -> usize {
        self.waiting.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(id: u32, character: u32) -> Question {
        Question { id, character_id: character, name: b"Asker".to_vec(), text: b"where?\0".to_vec(), time: 1 }
    }

    #[test]
    fn the_oldest_question_is_taken_first_and_the_count_is_the_one_before() {
        let mut queue = QuestionQueue::default();
        assert_eq!(queue.take(1), Taken::Empty);
        assert!(queue.ask(question(10, 1)) && queue.ask(question(11, 2)));
        assert_eq!(queue.waiting_count(), 2);
        assert_eq!(queue.take(1), Taken::New(question(10, 1), 2));
        assert_eq!(queue.waiting_count(), 1);
    }

    #[test]
    fn a_fourth_question_of_one_character_is_refused() {
        let mut queue = QuestionQueue::default();
        assert!((1..=3).all(|id| queue.ask(question(id, 7))));
        assert!(!queue.may_ask(7) && !queue.ask(question(4, 7)));
        assert!(queue.ask(question(5, 8)), "another character");
        // A question in process still counts; an answered one does not.
        assert!(matches!(queue.take(9), Taken::New(..)));
        assert!(!queue.ask(question(6, 7)));
        queue.answer(9, 1);
        assert!(queue.ask(question(7, 7)));
        assert_eq!(queue.waiting_count(), 4);
    }

    #[test]
    fn taking_twice_gives_the_same_question() {
        let mut queue = QuestionQueue::load([question(10, 1), question(11, 2)]);
        assert_eq!(queue.take(5), Taken::New(question(10, 1), 2));
        assert_eq!(queue.take(5), Taken::Again(question(10, 1), 2), "one waits, plus the held one");
        assert_eq!(queue.take(6), Taken::New(question(11, 2), 1), "another GM gets the next");
    }

    #[test]
    fn an_answer_frees_every_gm_holding_the_question() {
        let mut queue = QuestionQueue::load([question(10, 1), question(11, 2)]);
        queue.take(5);
        // GM 6 has no hold, but answers GM 5's question (spec 3.4).
        assert_eq!(queue.answer(6, 10), Some(question(10, 1)));
        assert_eq!(queue.take(5), Taken::New(question(11, 2), 1), "GM 5 holds nothing now");
        assert_eq!(queue.answer(5, 10), None, "already answered");
        assert_eq!(queue.take(5), Taken::Empty, "the hold went with that answer, whatever the id");
    }

    #[test]
    fn a_gm_leaving_puts_its_question_first_again() {
        let mut queue = QuestionQueue::load([question(10, 1), question(11, 2)]);
        queue.take(5);
        assert_eq!(queue.give_back(6), None);
        assert_eq!(queue.give_back(5), Some(10));
        assert_eq!(queue.waiting_count(), 2);
        assert_eq!(queue.take(7), Taken::New(question(10, 1), 2), "the next take gets it");
        assert_eq!(queue.give_back(5), None, "the hold is gone");
    }
}
