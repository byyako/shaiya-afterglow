//! The gambling house's rules (`docs/original/mini-game.md` sections 3 to 5): the slot machine and the card double-up as one state
//! machine per player (`CMiniGame`, `CUser::PacketMiniGame`). Pure: the money and the random source come in, the packets, the money
//! changes and the game log records come out; `server/mini_game.rs` does the rest.

use shaiya_proto::mini_game as mp;

/// The tables' bet per line (`g_MiniGameNpc`, section 3).
pub const TABLE_BETS: [u32; 7] = [100, 300, 500, 1000, 1500, 2000, 3000];

/// The most lines to bet on.
pub const MAX_LINES: u8 = 8;

/// The symbols as `(multiplier, weight)` (`g_MiniGameCard`, section 4 step 2); the weights add up to 226.
const SYMBOLS: [(u32, u32); 22] = [
    (2, 20),
    (2, 20),
    (2, 20),
    (3, 15),
    (3, 15),
    (3, 15),
    (4, 12),
    (4, 12),
    (4, 12),
    (5, 10),
    (5, 10),
    (5, 10),
    (6, 8),
    (6, 8),
    (6, 8),
    (7, 6),
    (7, 6),
    (7, 6),
    (8, 4),
    (8, 4),
    (9, 3),
    (10, 2),
];

/// The weights' total.
const SYMBOL_WEIGHTS: u32 = 226;

/// Where each class ends among 10 000 (section 4 step 1): 60 %, 20 %, 10 %, 4 %, 3 %, 2 %, 0.8 %, 0.19 %, 0.01 %.
const CLASS_ENDS: [u32; 9] = [6000, 8000, 9000, 9400, 9700, 9900, 9980, 9999, 10000];

/// The lines, in the order the first `lines` of them count (`g_MiniGameSlotLine`, section 4 step 5).
pub const LINES: [[usize; 3]; 8] = [[3, 4, 5], [0, 1, 2], [6, 7, 8], [2, 4, 6], [0, 4, 8], [2, 5, 8], [1, 4, 7], [0, 3, 6]];

/// The patterns of a losing spin's cells (`g_NoBingoSet`, section 4.1): bit `k` set = cell `k` shows a low symbol (0 to 10).
const NO_BINGO: [u16; 32] = [
    0x18c, 0x11c, 0x09c, 0x19c, 0x162, 0x152, 0x072, 0x172, 0x14a, 0x16a, 0x11a, 0x15a, 0x10e, 0x18e, 0x14e, 0x11e, 0x0e1, 0x0b1, 0x071,
    0x0f1, 0x0a5, 0x0e5, 0x095, 0x0b5, 0x08d, 0x18d, 0x0ad, 0x09d, 0x063, 0x163, 0x0e3, 0x073,
];

/// The cells of a win (`g_BingoSet`, section 4.1): bit `k` set = cell `k` shows the winning symbol. Class `c` uses
/// `START[c]..START[c] + COUNT[c]`; the last entry (all cells) is class 8's and unused.
const BINGO: [u16; 282] = [
    0x1c0, 0x1e0, 0x1d0, 0x1f0, 0x1c8, 0x1e8, 0x1d8, 0x038, 0x138, 0x0b8, 0x1b8, 0x078, 0x178, 0x0f8, 0x1c4, 0x124, 0x1a4, 0x164, 0x054,
    0x154, 0x0d4, 0x134, 0x1b4, 0x074, 0x0f4, 0x1cc, 0x12c, 0x1ac, 0x16c, 0x05c, 0x15c, 0x0dc, 0x03c, 0x0bc, 0x1c2, 0x1e2, 0x092, 0x192,
    0x0d2, 0x0b2, 0x1b2, 0x0f2, 0x1ca, 0x1ea, 0x09a, 0x19a, 0x0da, 0x03a, 0x13a, 0x07a, 0x17a, 0x1c6, 0x126, 0x1a6, 0x166, 0x096, 0x196,
    0x056, 0x156, 0x136, 0x0b6, 0x076, 0x1ce, 0x12e, 0x1ae, 0x16e, 0x09e, 0x19e, 0x05e, 0x15e, 0x03e, 0x1c1, 0x1e1, 0x111, 0x191, 0x151,
    0x131, 0x1b1, 0x171, 0x049, 0x149, 0x0c9, 0x069, 0x169, 0x0e9, 0x119, 0x199, 0x059, 0x0d9, 0x039, 0x0b9, 0x1c5, 0x125, 0x1a5, 0x165,
    0x115, 0x195, 0x055, 0x0d5, 0x075, 0x0f5, 0x04d, 0x14d, 0x0cd, 0x12d, 0x1ad, 0x06d, 0x0ed, 0x11d, 0x19d, 0x03d, 0x0bd, 0x1c3, 0x1e3,
    0x113, 0x093, 0x153, 0x0d3, 0x133, 0x0b3, 0x173, 0x0f3, 0x04b, 0x14b, 0x0cb, 0x06b, 0x16b, 0x0eb, 0x11b, 0x09b, 0x05b, 0x03b, 0x007,
    0x107, 0x087, 0x187, 0x047, 0x147, 0x0c7, 0x027, 0x0a7, 0x067, 0x0e7, 0x017, 0x037, 0x00f, 0x10f, 0x08f, 0x18f, 0x02f, 0x0af, 0x01f,
    0x1f8, 0x1e4, 0x1d4, 0x174, 0x1ec, 0x1dc, 0x13c, 0x1bc, 0x07c, 0x0fc, 0x1d2, 0x1f2, 0x1da, 0x0ba, 0x1ba, 0x0fa, 0x1e6, 0x0d6, 0x1b6,
    0x176, 0x0f6, 0x1ee, 0x0de, 0x13e, 0x0be, 0x07e, 0x1d1, 0x1f1, 0x1c9, 0x1e9, 0x159, 0x139, 0x1b9, 0x079, 0x0f9, 0x1e5, 0x155, 0x135,
    0x1b5, 0x1cd, 0x16d, 0x05d, 0x0dd, 0x193, 0x1b3, 0x1cb, 0x1eb, 0x19b, 0x15b, 0x0db, 0x13b, 0x0bb, 0x07b, 0x1c7, 0x127, 0x1a7, 0x167,
    0x117, 0x097, 0x057, 0x0b7, 0x077, 0x04f, 0x14f, 0x0cf, 0x12f, 0x1af, 0x06f, 0x0ef, 0x11f, 0x09f, 0x03f, 0x1f4, 0x17c, 0x1fa, 0x1d6,
    0x1de, 0x1be, 0x17e, 0x0fe, 0x1d9, 0x179, 0x1d5, 0x175, 0x1ed, 0x15d, 0x13d, 0x1bd, 0x07d, 0x0fd, 0x1d3, 0x1f3, 0x1bb, 0x17b, 0x0fb,
    0x1e7, 0x197, 0x157, 0x0d7, 0x137, 0x0f7, 0x1cf, 0x16f, 0x19f, 0x05f, 0x0bf, 0x1fc, 0x1f6, 0x1f9, 0x1f5, 0x1dd, 0x1db, 0x1b7, 0x177,
    0x1ef, 0x15f, 0x0df, 0x13f, 0x07f, 0x1fe, 0x17d, 0x1fb, 0x1d7, 0x1bf, 0x0ff, 0x1fd, 0x1f7, 0x1df, 0x17f, 0x1ff,
];

/// Where each class's patterns start in `BINGO`, and how many there are (classes 1 to 6; index 0 is unused).
const START: [usize; 7] = [0, 0, 152, 224, 258, 271, 277];
const COUNT: [usize; 7] = [0, 152, 72, 34, 13, 6, 4];

/// The money a player may hold (`CUser::ChkAddMoney`): adds stop here.
fn add(money: &mut u32, amount: u32) {
    *money = money.saturating_add(amount);
}

/// The odds in force (`GAME_SLOT_PAYOUT_PERCENT`, `GAME_CARD_WIN_CHANCES`, `docs/original/mini-game.md` decision 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Odds {
    /// Each winning line pays `multiplier × the table's bet × payout_percent / 100`; 100 is the original.
    pub payout_percent: u32,
    /// The chance in per mille that the card falls on the guessed side, by round (the original `[400, 200, 100, 50, 10]`).
    pub chances: [u32; 5],
}

impl Odds {
    /// The original's.
    pub const ORIGINAL: Self = Self { payout_percent: 100, chances: [400, 200, 100, 50, 10] };
}

/// The game's state (`CMiniGame +0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// At the table: betting, spinning.
    Slot,
    /// A win on the table: take it, or double up.
    SlotWon,
    /// The card game, waiting for the guess.
    Card,
    /// A card won: go on or stop.
    CardWon,
}

/// A record for the game log (section 7); the connection adds who and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    /// `0x0D03`: the money after a change, and the change (signed).
    Money { after: u32, change: i64 },
    /// `0x0D04`: lines bet, the bet, lines won, the winnings.
    Slot { lines: u8, bet: u32, won: u8, winnings: u32 },
    /// `0x0D05`: the rounds won (counting this one when it won), won or lost, the stake or what came back.
    Card { round: u8, won: bool, amount: u32 },
    /// `0x0D02`: the money on standing up.
    Exit { money: u32 },
}

/// What a request did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The packet to answer with, if any (a request in the wrong state gets none).
    pub answer: Option<Vec<u8>>,
    /// The money changed: save it.
    pub money_changed: bool,
    pub records: Vec<Record>,
}

/// What standing up hands back (`CMiniGame::GameExit`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stood {
    pub money_changed: bool,
    /// The money handed back, then `Record::Exit`.
    pub records: Vec<Record>,
}

/// One player's game (`CMiniGame`): `bet` is the slot's bet, then the card's stake; `pending` the slot's winnings, or a re-bet
/// already paid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiniGame {
    state: State,
    table: u8,
    bet: u32,
    lines: u8,
    pending: u32,
    round: u8,
    dealer: u8,
}

impl MiniGame {
    /// Sitting down (`CMiniGame::GameEnter`): `None` for a table above 6.
    pub fn sit(table: u8) -> Option<Self> {
        (usize::from(table) < TABLE_BETS.len()).then_some(Self {
            state: State::Slot,
            table,
            bet: 0,
            lines: 0,
            pending: 0,
            round: 0,
            dealer: 0,
        })
    }

    pub fn table(&self) -> u8 {
        self.table
    }

    /// The table's bet per line.
    pub fn table_bet(&self) -> u32 {
        TABLE_BETS[usize::from(self.table)]
    }

    fn reset_bet(&mut self) {
        self.bet = 0;
        self.lines = 0;
        self.pending = 0;
    }

    /// `0x3102` (section 4): one more line, or the re-bet handed back. Nothing in another state.
    pub fn bet_line(&mut self, money: &mut u32) -> Outcome {
        if self.state != State::Slot {
            return Outcome::default();
        }
        let result = if self.pending == 0 {
            let per_line = self.table_bet();
            if self.lines < MAX_LINES && *money >= per_line {
                self.bet += per_line;
                *money -= per_line;
                self.lines += 1;
                mp::BET_LINE
            } else {
                mp::BET_REFUSED
            }
        } else {
            add(money, self.bet);
            self.reset_bet();
            mp::BET_BACK
        };
        Outcome { answer: Some(mp::bet(result, self.bet, *money)), money_changed: result != mp::BET_REFUSED, records: Vec::new() }
    }

    /// `0x3103` (section 4): the class, the symbol, the cells, the lines that pay. Without a bet the answer is the refusal.
    pub fn spin(&mut self, money: &mut u32, odds: &Odds, rand: &mut impl FnMut(u32) -> u32) -> Outcome {
        if self.state != State::Slot || self.lines == 0 {
            return Outcome { answer: Some(mp::spin(false, [0; 9], false, 0, false)), ..Outcome::default() };
        }
        let cells = draw_cells(rand);
        let (won, winnings) = pay(&cells, self.lines, self.table_bet(), odds);
        let records = vec![
            Record::Slot { lines: self.lines, bet: self.bet, won, winnings },
            Record::Money { after: *money, change: -i64::from(self.bet) },
        ];
        let (win, short, money_changed) = if won == 0 {
            // The automatic re-bet: the next spin is paid at once, if the money allows it.
            if *money >= self.bet {
                *money -= self.bet;
                self.pending = 1;
                (false, false, true)
            } else {
                self.reset_bet();
                (false, true, false)
            }
        } else {
            self.pending = winnings;
            self.state = State::SlotWon;
            (true, u64::from(*money) + u64::from(winnings) < u64::from(self.bet), false)
        };
        Outcome { answer: Some(mp::spin(true, cells, win, winnings, short)), money_changed, records }
    }

    /// `0x3105` (section 4): the win to the money, and the same bet again when the money allows it.
    pub fn again(&mut self, money: &mut u32) -> Outcome {
        if self.state != State::SlotWon {
            return Outcome::default();
        }
        add(money, self.pending);
        let records = vec![Record::Money { after: *money, change: i64::from(self.pending) }];
        if *money >= self.bet {
            *money -= self.bet;
        } else {
            self.reset_bet();
        }
        self.state = State::Slot;
        Outcome { answer: Some(mp::again()), money_changed: true, records }
    }

    /// `0x3104` (section 5): the win becomes the stake; the dealer's first card.
    pub fn double_up(&mut self, rand: &mut impl FnMut(u32) -> u32) -> Outcome {
        if self.state != State::SlotWon {
            return Outcome::default();
        }
        self.bet = self.pending;
        self.round = 0;
        self.state = State::Card;
        self.dealer = deal(rand);
        Outcome { answer: Some(mp::dealer(mp::OP_DOUBLE, self.dealer)), ..Outcome::default() }
    }

    /// `0x3107` (section 5): the guess against the dealer's card.
    pub fn guess(&mut self, high: bool, money: &mut u32, odds: &Odds, rand: &mut impl FnMut(u32) -> u32) -> Outcome {
        if self.state != State::Card {
            return Outcome::default();
        }
        let d = u32::from(self.dealer);
        let side_guessed = rand(1000) < odds.chances[usize::from(self.round)];
        // The card lands on the guessed side, or on the other one.
        let upper = side_guessed == high;
        let card = (if upper { rand(22 - d) + d } else { rand(d + 1) }) as u8;
        let won = side_guessed && u32::from(card) != d;
        if won {
            self.round += 1;
            self.bet = self.bet.saturating_mul(u32::from(self.round) * 2);
            let mut records = vec![Record::Card { round: self.round, won: true, amount: self.bet }];
            if self.round == 5 {
                // The fifth win ends the game: the stake is paid out.
                add(money, self.bet);
                records.push(Record::Money { after: *money, change: i64::from(self.bet) });
                let stake = self.bet;
                self.reset_bet();
                self.state = State::Slot;
                return Outcome { answer: Some(mp::guessed(card, mp::GUESS_WON_LAST, stake)), money_changed: true, records };
            }
            self.state = State::CardWon;
            Outcome { answer: Some(mp::guessed(card, mp::GUESS_WON, self.bet)), money_changed: false, records }
        } else {
            // A fifth of the stake comes back (`(float)stake × 0.2` cut).
            let back = (self.bet as f32 * 0.2f32) as u32;
            add(money, back);
            let records = vec![
                Record::Card { round: self.round, won: false, amount: back },
                Record::Money { after: *money, change: i64::from(back) },
            ];
            self.reset_bet();
            self.state = State::Slot;
            Outcome { answer: Some(mp::guessed(card, mp::GUESS_LOST, back)), money_changed: true, records }
        }
    }

    /// `0x3108` (section 5): the next dealer's card.
    pub fn next_card(&mut self, rand: &mut impl FnMut(u32) -> u32) -> Outcome {
        if self.state != State::CardWon {
            return Outcome::default();
        }
        self.dealer = deal(rand);
        self.state = State::Card;
        Outcome { answer: Some(mp::dealer(mp::OP_NEXT, self.dealer)), ..Outcome::default() }
    }

    /// `0x3109` (section 5): the stake to the money.
    pub fn stop(&mut self, money: &mut u32) -> Outcome {
        if self.state != State::CardWon {
            return Outcome::default();
        }
        add(money, self.bet);
        let records = vec![Record::Money { after: *money, change: i64::from(self.bet) }];
        self.reset_bet();
        self.state = State::Slot;
        Outcome { answer: Some(mp::stopped(*money)), money_changed: true, records }
    }

    /// Standing up, or any leave (`CMiniGame::GameExit`, section 3): what is on the table goes back to the money.
    pub fn stand(&self, money: &mut u32) -> Stood {
        let (back, record) = match self.state {
            State::Slot => (self.bet, false),
            State::SlotWon => (self.pending, true),
            State::Card | State::CardWon => (self.bet, true),
        };
        add(money, back);
        let mut records = Vec::new();
        if record {
            records.push(Record::Money { after: *money, change: i64::from(back) });
        }
        records.push(Record::Exit { money: *money });
        Stood { money_changed: back > 0, records }
    }
}

/// A dealer's card, 4 to 17 (section 5).
fn deal(rand: &mut impl FnMut(u32) -> u32) -> u8 {
    (rand(14) + 4) as u8
}

/// The symbol of a win by a draw in `0..226` (section 4 step 2).
fn symbol(draw: u32) -> u8 {
    let mut total = 0;
    for (i, (_, weight)) in SYMBOLS.iter().enumerate() {
        total += weight;
        if draw < total {
            return i as u8;
        }
    }
    (SYMBOLS.len() - 1) as u8
}

/// The nine cells of a spin (section 4 steps 1 to 4): the class, the winning symbol, the cells that cannot form a line, and the
/// pattern that does.
fn draw_cells(rand: &mut impl FnMut(u32) -> u32) -> [u8; 9] {
    let r = rand(10000);
    let class = CLASS_ENDS.iter().position(|&end| r < end).unwrap_or(CLASS_ENDS.len() - 1);
    // The pattern table has no class 7.
    let class = if class == 7 { 6 } else { class };
    let s = if class == 0 { 0 } else { symbol(rand(SYMBOL_WEIGHTS)) };
    if class == 8 {
        return [s; 9];
    }
    let mixed = NO_BINGO[rand(32) as usize];
    let mut cells = [0u8; 9];
    for (k, cell) in cells.iter_mut().enumerate() {
        *cell = if mixed >> k & 1 == 1 {
            let v = rand(11) as u8;
            if v == s { (v + 1) % 11 } else { v }
        } else {
            let v = rand(11) as u8 + 11;
            if v == s { (v - 10) % 11 + 11 } else { v }
        };
    }
    if class > 0 {
        let winning = BINGO[rand(COUNT[class] as u32) as usize + START[class]];
        for (k, cell) in cells.iter_mut().enumerate() {
            if winning >> k & 1 == 1 {
                *cell = s;
            }
        }
    }
    cells
}

/// The lines that pay among the first `lines`, and what they pay (section 4 step 5 and decision 2).
fn pay(cells: &[u8; 9], lines: u8, table_bet: u32, odds: &Odds) -> (u8, u32) {
    let mut won = 0;
    let mut winnings = 0u32;
    for line in LINES.iter().take(usize::from(lines)) {
        let [a, b, c] = line.map(|k| cells[k]);
        if a == b && b == c {
            won += 1;
            let (multiplier, _) = SYMBOLS[usize::from(a)];
            let paid = u64::from(multiplier) * u64::from(table_bet) * u64::from(odds.payout_percent) / 100;
            winnings = winnings.saturating_add(u32::try_from(paid).unwrap_or(u32::MAX));
        }
    }
    (won, winnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A scripted random source: each call takes the next number, which must fit the range asked for.
    fn script(draws: &[u32]) -> impl FnMut(u32) -> u32 {
        let mut draws: VecDeque<u32> = draws.iter().copied().collect();
        move |n| {
            let v = draws.pop_front().expect("the script ran out");
            assert!(v < n, "draw {v} for a range of {n}");
            v
        }
    }

    /// A deterministic spread of numbers for the sweeps.
    fn sweep(seed: u32) -> impl FnMut(u32) -> u32 {
        let mut x = seed.wrapping_mul(2654435761).wrapping_add(12345);
        move |n| {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            (x >> 8) % n
        }
    }

    /// The lines of the 8 whose three cells lie all inside `bits`.
    fn lines_inside(bits: u16) -> usize {
        LINES.iter().filter(|line| line.iter().all(|&k| bits >> k & 1 == 1)).count()
    }

    fn lines_equal(cells: &[u8; 9]) -> usize {
        LINES.iter().filter(|line| cells[line[0]] == cells[line[1]] && cells[line[1]] == cells[line[2]]).count()
    }

    #[test]
    fn the_tables_are_the_originals() {
        assert_eq!(SYMBOLS.iter().map(|s| s.1).sum::<u32>(), SYMBOL_WEIGHTS);
        assert_eq!(SYMBOLS.len(), 22);
        assert_eq!((symbol(0), symbol(19), symbol(20), symbol(59), symbol(60), symbol(223), symbol(224)), (0, 0, 1, 2, 3, 20, 21));
        assert_eq!(BINGO.len(), 282);
        assert_eq!(START[6] + COUNT[6], 281, "class 6 ends before the all-cells entry");
        assert_eq!(BINGO[281], 0x1ff);
        // The classes' chances: 60 %, 20 %, 10 %, 4 %, 3 %, 2 %, 0.8 %, 0.19 %, 0.01 %.
        let mut before = 0;
        let shares: Vec<u32> = CLASS_ENDS
            .iter()
            .map(|&e| {
                let share = e - before;
                before = e;
                share
            })
            .collect();
        assert_eq!(shares, [6000, 2000, 1000, 400, 300, 200, 80, 19, 1]);
    }

    #[test]
    fn every_winning_pattern_forms_exactly_its_class_in_lines() {
        for class in 1..=6 {
            for (k, &pattern) in BINGO.iter().enumerate().skip(START[class]).take(COUNT[class]) {
                assert_eq!(lines_inside(pattern), class, "pattern {k} of class {class}");
            }
        }
        assert_eq!(lines_inside(BINGO[281]), 8);
        // No losing pattern puts a whole line in one half: no line can form with cells that never equal the symbol.
        for (p, &mixed) in NO_BINGO.iter().enumerate() {
            assert_eq!(lines_inside(mixed), 0, "pattern {p} has a line of low symbols");
            assert_eq!(lines_inside(!mixed & 0x1ff), 0, "pattern {p} has a line of high symbols");
        }
    }

    #[test]
    fn a_losing_spin_forms_no_line_and_never_shows_the_symbol() {
        // Class 0 for the draws below 6000, whatever the pattern and the cell draws.
        for seed in 0..400 {
            let mut inner = sweep(seed);
            let mut first = true;
            let mut rand = |n: u32| if std::mem::take(&mut first) { inner(6000) } else { inner(n) };
            let cells = draw_cells(&mut rand);
            assert_eq!(lines_equal(&cells), 0, "{cells:?}");
            assert!(cells.iter().all(|&c| c != 0), "symbol 0 never shows on a losing spin: {cells:?}");
        }
        // The cell rules at their edges: a draw equal to the symbol moves up one; in the low half 10 wraps to 0, in the high half 21 to 11.
        // Pattern 0 (0x18c): cells 2, 3, 7 and 8 are low, the others high; the first winning pattern fills cells 6, 7 and 8.
        let cells = draw_cells(&mut script(&[6000, 152, 0, 0, 0, 10, 10, 0, 0, 0, 10, 10, 0]));
        assert_eq!(cells, [11, 11, 0, 0, 11, 11, 10, 10, 10], "symbol 10: a low draw of 10 is 11, wrapped to 0");
        let cells = draw_cells(&mut script(&[6000, 225, 0, 10, 10, 10, 10, 10, 10, 10, 10, 10, 0]));
        assert_eq!(cells, [11, 11, 10, 10, 11, 11, 21, 21, 21], "symbol 21: a high draw of 21 is 22, wrapped to 11");
    }

    #[test]
    fn each_class_pays_its_lines_and_class_seven_becomes_six() {
        // class draw, symbol draw (0: symbol 0, multiplier 2), the pattern (0), nine cell draws, the winning pattern.
        let spin_cells = |class: u32, k: u32| {
            let mut draws = vec![class, 0, 0];
            draws.extend([5; 9]);
            draws.push(k);
            draw_cells(&mut script(&draws))
        };
        // Class 1: the first pattern fills the middle row (cells 6, 7, 8 per the table bits: 0x1c0).
        let c1 = spin_cells(6000, 0);
        assert_eq!(lines_equal(&c1), 1);
        assert_eq!(lines_equal(&spin_cells(8000, 0)), 2);
        assert_eq!(lines_equal(&spin_cells(9000, 0)), 3);
        assert_eq!(lines_equal(&spin_cells(9400, 0)), 4);
        assert_eq!(lines_equal(&spin_cells(9700, 0)), 5);
        assert_eq!(lines_equal(&spin_cells(9900, 0)), 6);
        assert_eq!(lines_equal(&spin_cells(9980, 0)), 6, "class 7 is class 6");
        // Class 8 (draws 9999): all nine cells, no pattern drawn.
        let all = draw_cells(&mut script(&[9999, 60]));
        assert_eq!(all, [3; 9], "a symbol draw of 60 is symbol 3");
        assert_eq!(lines_equal(&all), 8);
    }

    #[test]
    fn only_the_first_lines_pay_and_the_payout_follows_the_setting() {
        // The middle row (3, 4, 5) and the top row (0, 1, 2) of symbol 3 (multiplier 3).
        let cells = [3, 3, 3, 3, 3, 3, 12, 14, 16];
        assert_eq!(pay(&cells, 1, 100, &Odds::ORIGINAL), (1, 300), "the middle row only");
        assert_eq!(pay(&cells, 2, 100, &Odds::ORIGINAL), (2, 600));
        assert_eq!(pay(&cells, 8, 100, &Odds::ORIGINAL), (2, 600));
        let more = Odds { payout_percent: 220, ..Odds::ORIGINAL };
        assert_eq!(pay(&cells, 2, 100, &more), (2, 1320), "3 × 100 × 2.2 = 660 per line");
        assert_eq!(pay(&cells, 1, 300, &more), (1, 1980));
        // Not among the counted lines: the bottom row (6, 7, 8) only.
        let bottom = [12, 14, 16, 13, 15, 17, 1, 1, 1];
        assert_eq!(pay(&bottom, 2, 100, &Odds::ORIGINAL), (0, 0));
        assert_eq!(pay(&bottom, 3, 100, &Odds::ORIGINAL), (1, 200));
    }

    #[test]
    fn sitting_down_and_betting_lines() {
        assert_eq!(MiniGame::sit(7), None);
        let mut game = MiniGame::sit(1).unwrap();
        assert_eq!(game.table_bet(), 300);
        let mut money = 2600;
        for lines in 1..=8 {
            let out = game.bet_line(&mut money);
            assert_eq!(out.answer, Some(mp::bet(mp::BET_LINE, 300 * lines, 2600 - 300 * lines)));
            assert!(out.money_changed);
        }
        assert_eq!(money, 200);
        let out = game.bet_line(&mut money);
        assert_eq!((out.answer, out.money_changed), (Some(mp::bet(mp::BET_REFUSED, 2400, 200)), false), "a ninth line");
        let mut poor = MiniGame::sit(0).unwrap();
        let mut money = 99;
        assert_eq!(poor.bet_line(&mut money).answer, Some(mp::bet(mp::BET_REFUSED, 0, 99)), "too little money");
        // Wrong state: no answer at all.
        let mut won = MiniGame::sit(0).unwrap();
        won.state = State::SlotWon;
        assert_eq!(won.bet_line(&mut money), Outcome::default());
    }

    /// Draws that make a spin of class `class_draw` with the given symbol draw, pattern 0 and the first winning pattern.
    fn draws(class_draw: u32, symbol_draw: u32) -> Vec<u32> {
        let mut d = vec![class_draw, symbol_draw, 0];
        d.extend([5; 9]);
        d.push(0);
        d
    }

    #[test]
    fn a_losing_spin_pays_the_next_bet_at_once_when_it_can() {
        let mut game = MiniGame::sit(0).unwrap();
        let mut money = 1000;
        game.bet_line(&mut money);
        game.bet_line(&mut money);
        assert_eq!((game.bet, money), (200, 800));
        // Class 0 (a draw below 6000): no win; the symbol draw is not made.
        let mut d = vec![100, 0];
        d.extend([5; 9]);
        let out = game.spin(&mut money, &Odds::ORIGINAL, &mut script(&d));
        let cells = shaiya_proto::frame::payload(out.answer.as_ref().unwrap());
        assert_eq!((cells[0], cells[10], &cells[11..15], cells[15]), (1, 0, &[0, 0, 0, 0][..], 0), "ok, not won, no winnings, not short");
        assert_eq!((money, game.pending, game.state), (600, 1, State::Slot), "the next spin is paid");
        assert_eq!(
            out.records,
            [Record::Slot { lines: 2, bet: 200, won: 0, winnings: 0 }, Record::Money { after: 800, change: -200 }],
            "the record names the money before the re-bet"
        );
        assert!(out.money_changed);
        // Taking the re-bet back is a bet again (result 2).
        let back = game.bet_line(&mut money);
        assert_eq!(back.answer, Some(mp::bet(mp::BET_BACK, 0, 800)));
        assert_eq!((game.bet, game.lines, game.pending), (0, 0, 0));
        // Without the money for the next spin: the bet is gone and `short` says so.
        let mut game = MiniGame::sit(0).unwrap();
        let mut money = 100;
        game.bet_line(&mut money);
        let mut d = vec![100, 0];
        d.extend([5; 9]);
        let out = game.spin(&mut money, &Odds::ORIGINAL, &mut script(&d));
        let p = shaiya_proto::frame::payload(out.answer.as_ref().unwrap()).to_vec();
        assert_eq!((p[0], p[10], p[15]), (1, 0, 1));
        assert_eq!((game.bet, game.lines, game.pending, money, out.money_changed), (0, 0, 0, 0, false));
    }

    #[test]
    fn a_win_waits_on_the_table_until_it_is_taken_or_doubled() {
        let mut game = MiniGame::sit(0).unwrap();
        let mut money = 500;
        game.bet_line(&mut money);
        // Class 1 (6000), symbol 0 (multiplier 2): the pattern 0 is cells 6, 7, 8 (the bottom row) with one line bet: not counted.
        let out = game.spin(&mut money, &Odds::ORIGINAL, &mut script(&draws(6000, 0)));
        assert_eq!(shaiya_proto::frame::payload(out.answer.as_ref().unwrap())[10], 0, "the bottom row is the third line");
        // Three lines bet: it counts. Start again.
        let mut game = MiniGame::sit(0).unwrap();
        let mut money = 500;
        for _ in 0..3 {
            game.bet_line(&mut money);
        }
        let out = game.spin(&mut money, &Odds::ORIGINAL, &mut script(&draws(6000, 0)));
        let p = shaiya_proto::frame::payload(out.answer.as_ref().unwrap()).to_vec();
        assert_eq!((p[0], p[10], u32::from_le_bytes(p[11..15].try_into().unwrap()), p[15]), (1, 1, 200, 0));
        assert_eq!((game.state, game.pending, money), (State::SlotWon, 200, 200), "not in the money yet");
        assert_eq!(out.records, [Record::Slot { lines: 3, bet: 300, won: 1, winnings: 200 }, Record::Money { after: 200, change: -300 }]);
        // `again`: the win to the money, then the same bet again (200 + 200 = 400 ≥ 300).
        let mut taken = game.clone();
        let mut m = money;
        let out = taken.again(&mut m);
        assert_eq!(out.answer, Some(mp::again()));
        assert_eq!((m, taken.state, taken.pending, taken.bet), (100, State::Slot, 200, 300), "pending keeps a non-zero value");
        assert_eq!(out.records, [Record::Money { after: 400, change: 200 }]);
        // Too poor for the same bet: the bet is gone.
        let mut poor = game.clone();
        poor.bet = 1000;
        let mut m = 0;
        poor.again(&mut m);
        assert_eq!((m, poor.bet, poor.lines, poor.pending, poor.state), (200, 0, 0, 0, State::Slot));
        // The wrong state gets nothing.
        assert_eq!(taken.again(&mut m), Outcome::default());
        // `short`: the money plus the winnings cannot pay the bet again.
        let mut short = MiniGame::sit(0).unwrap();
        let mut m = 300;
        for _ in 0..3 {
            short.bet_line(&mut m);
        }
        let out = short.spin(&mut m, &Odds::ORIGINAL, &mut script(&draws(6000, 0)));
        assert_eq!(shaiya_proto::frame::payload(out.answer.as_ref().unwrap())[15], 1, "0 + 200 < 300");
    }

    #[test]
    fn the_card_game_doubles_the_stake_round_by_round() {
        let mut game = MiniGame::sit(0).unwrap();
        game.state = State::SlotWon;
        game.pending = 100;
        let mut money = 0;
        // The dealer shows 10 (draw 6 + 4).
        let out = game.double_up(&mut script(&[6]));
        assert_eq!((out.answer, game.state, game.bet, game.dealer), (Some(mp::dealer(mp::OP_DOUBLE, 10)), State::Card, 100, 10));
        let odds = Odds::ORIGINAL;
        // Round 1 (chance 400): a draw of 0 falls on the guessed side; high: 15 = rand(12) 5 + 10.
        let out = game.guess(true, &mut money, &odds, &mut script(&[0, 5]));
        assert_eq!(out.answer, Some(mp::guessed(15, mp::GUESS_WON, 200)));
        assert_eq!((game.state, game.bet, game.round), (State::CardWon, 200, 1));
        assert_eq!(out.records, [Record::Card { round: 1, won: true, amount: 200 }]);
        // Next card, then round 2 (chance 200): low wins with a card of 3.
        assert_eq!(game.next_card(&mut script(&[0])).answer, Some(mp::dealer(mp::OP_NEXT, 4)));
        let out = game.guess(false, &mut money, &odds, &mut script(&[199, 0]));
        assert_eq!(out.answer, Some(mp::guessed(0, mp::GUESS_WON, 200 * 4)), "199 < 200: the side guessed; low: rand(5) = 0");
        assert_eq!((game.bet, game.round), (800, 2));
        // The stake is taken: stop.
        let mut stopping = game.clone();
        let mut m = 5;
        let out = stopping.stop(&mut m);
        assert_eq!((out.answer, m, stopping.state, stopping.bet), (Some(mp::stopped(805)), 805, State::Slot, 0));
        assert_eq!(out.records, [Record::Money { after: 805, change: 800 }]);
        assert_eq!(stopping.stop(&mut m), Outcome::default(), "wrong state");
        // Rounds 3 to 5: the stake ends at 3840 times the first.
        game.next_card(&mut script(&[0]));
        let mut last = Outcome::default();
        for round in 3..=5 {
            last = game.guess(true, &mut money, &odds, &mut script(&[0, 1]));
            if round < 5 {
                game.next_card(&mut script(&[0]));
            }
        }
        assert_eq!(last.answer, Some(mp::guessed(5, mp::GUESS_WON_LAST, 100 * 3840)));
        assert_eq!((money, game.state, game.bet, game.lines, game.pending), (384_000, State::Slot, 0, 0, 0), "paid out at the fifth win");
        assert_eq!(last.records.last(), Some(&Record::Money { after: 384_000, change: 384_000 }));
    }

    #[test]
    fn a_lost_guess_gives_back_a_fifth_and_a_tie_loses() {
        let odds = Odds::ORIGINAL;
        let start = || {
            let mut g = MiniGame::sit(0).unwrap();
            g.state = State::Card;
            g.bet = 1000;
            g.dealer = 10;
            g
        };
        // The card falls on the other side: high guessed, the chance missed (draw 400 is not below 400), a card at or below the dealer's.
        let mut game = start();
        let mut money = 0;
        let out = game.guess(true, &mut money, &odds, &mut script(&[400, 3]));
        assert_eq!(out.answer, Some(mp::guessed(3, mp::GUESS_LOST, 200)));
        assert_eq!((money, game.state, game.bet), (200, State::Slot, 0));
        assert_eq!(out.records, [Record::Card { round: 0, won: false, amount: 200 }, Record::Money { after: 200, change: 200 }]);
        // A tie: the chance hit, the card equals the dealer's (rand(12) = 0 → 10): lost.
        let mut game = start();
        let mut money = 0;
        let out = game.guess(true, &mut money, &odds, &mut script(&[0, 0]));
        assert_eq!(out.answer, Some(mp::guessed(10, mp::GUESS_LOST, 200)));
        // Low guessed and the chance missed: the card lands high.
        let mut game = start();
        let out = game.guess(false, &mut money, &odds, &mut script(&[999, 4]));
        assert_eq!(out.answer, Some(mp::guessed(14, mp::GUESS_LOST, 200)), "rand(12) = 4 → 14");
        // The fifth is cut as a float: 55 × 0.2 = 11.
        let mut game = start();
        game.bet = 55;
        assert_eq!(game.guess(true, &mut money, &odds, &mut script(&[999, 0])).answer, Some(mp::guessed(0, mp::GUESS_LOST, 11)));
        // The chances are the setting's: round 1 at 420.
        let better = Odds { chances: [420, 200, 130, 100, 80], ..Odds::ORIGINAL };
        let mut game = start();
        assert_eq!(game.guess(true, &mut money, &better, &mut script(&[419, 1])).answer, Some(mp::guessed(11, mp::GUESS_WON, 2000)));
        // The wrong state is silent.
        let mut idle = MiniGame::sit(0).unwrap();
        assert_eq!(idle.guess(true, &mut money, &odds, &mut script(&[])), Outcome::default());
        assert_eq!(idle.next_card(&mut script(&[])), Outcome::default());
        assert_eq!(idle.double_up(&mut script(&[])), Outcome::default());
    }

    #[test]
    fn standing_up_hands_back_what_is_on_the_table() {
        let mut money = 50;
        // At the table with a bet (a re-bet included).
        let mut game = MiniGame::sit(0).unwrap();
        game.bet = 300;
        let stood = game.stand(&mut money);
        assert_eq!((money, stood.money_changed, stood.records), (350, true, vec![Record::Exit { money: 350 }]));
        // A win on the table.
        let mut game = MiniGame::sit(0).unwrap();
        game.state = State::SlotWon;
        game.pending = 700;
        game.bet = 100;
        let stood = game.stand(&mut money);
        assert_eq!(money, 1050);
        assert_eq!(stood.records, [Record::Money { after: 1050, change: 700 }, Record::Exit { money: 1050 }]);
        // In the card game: the stake.
        for state in [State::Card, State::CardWon] {
            let mut game = MiniGame::sit(0).unwrap();
            game.state = state;
            game.bet = 400;
            let mut m = 0;
            assert_eq!(game.stand(&mut m).records, [Record::Money { after: 400, change: 400 }, Record::Exit { money: 400 }]);
        }
        // Nothing on the table: only the exit record, nothing to save.
        let mut m = 7;
        let stood = MiniGame::sit(0).unwrap().stand(&mut m);
        assert_eq!((m, stood.money_changed, stood.records), (7, false, vec![Record::Exit { money: 7 }]));
        // Money stops at the top.
        let mut game = MiniGame::sit(0).unwrap();
        game.bet = 100;
        let mut m = u32::MAX - 10;
        game.stand(&mut m);
        assert_eq!(m, u32::MAX);
    }

    #[test]
    fn a_spin_without_a_bet_is_refused() {
        let mut game = MiniGame::sit(0).unwrap();
        let mut money = 100;
        let out = game.spin(&mut money, &Odds::ORIGINAL, &mut script(&[]));
        assert_eq!(out.answer, Some(mp::spin(false, [0; 9], false, 0, false)));
        assert!(!out.money_changed && out.records.is_empty());
        game.state = State::Card;
        assert_eq!(game.spin(&mut money, &Odds::ORIGINAL, &mut script(&[])).answer, Some(mp::spin(false, [0; 9], false, 0, false)));
    }

    /// The client's gambling window as research entry 6 reads it (`docs/original/mini-game.md` section 13): its own wallet, taken
    /// from the answers only at `0x3102` result 1 (and `0x3106`), otherwise worked out from the bet, the amounts and the `short` flag.
    #[derive(Default)]
    struct ClientWindow {
        wallet: u32,
        bet: u32,
        winnings: u32,
        short: bool,
        stake: u32,
        /// The card round, counted by the client (1 at `0x3104`); 0 at the slots.
        round: u8,
    }

    impl ClientWindow {
        /// After a losing spin, or `0x3105`: the same bet again, paid from the wallet, unless `short` says it could not be.
        fn re_bet(&mut self) {
            if self.short {
                self.bet = 0;
            } else {
                self.wallet -= self.bet;
            }
        }

        fn answer(&mut self, packet: &[u8]) {
            let p = shaiya_proto::frame::payload(packet);
            let u32_at = |at: usize| u32::from_le_bytes(p[at..at + 4].try_into().unwrap());
            match shaiya_proto::frame::opcode(packet) {
                mp::OP_BET => match p[0] {
                    mp::BET_LINE => (self.bet, self.wallet) = (u32_at(1), u32_at(5)),
                    // The answer's fields are not read here: the bet the window holds goes back.
                    mp::BET_BACK => (self.wallet, self.bet) = (self.wallet + self.bet, 0),
                    _ => {}
                },
                mp::OP_SPIN => {
                    assert_eq!(p[0], 1);
                    assert!(p[1..10].iter().all(|&c| c <= 21), "faces 0 to 21 only (22 is the back): {:?}", &p[1..10]);
                    self.short = p[15] != 0;
                    if p[10] != 0 {
                        self.winnings = u32_at(11);
                    } else {
                        self.re_bet();
                    }
                }
                // No payload read.
                mp::OP_AGAIN => {
                    self.wallet += self.winnings;
                    self.re_bet();
                }
                mp::OP_DOUBLE => {
                    assert!((4..=17).contains(&p[0]));
                    (self.stake, self.round) = (self.winnings, 1);
                }
                mp::OP_GUESS => {
                    assert!(p[0] <= 21);
                    let amount = u32_at(2);
                    if p[1] == 0 {
                        self.wallet += amount;
                        self.leave_cards();
                    } else if self.round == 5 {
                        // The fifth round by the client's own count; result 2 is not looked at.
                        self.wallet += amount;
                        self.leave_cards();
                    } else {
                        self.stake = amount;
                    }
                }
                mp::OP_NEXT => self.round += 1,
                // The money field is not read: the stake the window holds is added.
                mp::OP_STOP => {
                    self.wallet += self.stake;
                    self.leave_cards();
                }
                other => panic!("unexpected answer {other:#06x}"),
            }
        }

        /// Back at the slots after the card game. Whether the window clears its slot bet here is not read (live test 52); the
        /// server's bet is 0, and so is the window's in this model.
        fn leave_cards(&mut self) {
            (self.round, self.stake, self.bet) = (0, 0, 0);
        }
    }

    #[test]
    fn the_clients_wallet_follows_the_servers_money() {
        let fair = Odds { payout_percent: 220, chances: [420, 200, 130, 100, 80] };
        // Every card on the guessed side (only ties lose): the card game often runs to its fifth round.
        let certain = Odds { payout_percent: 220, chances: [1000; 5] };
        for seed in 0..400 {
            let mut rand = sweep(seed);
            let mut choose = sweep(seed + 10_000);
            let (odds, table) = if seed % 2 == 0 { (fair, seed % 7) } else { (certain, seed % 3) };
            let mut game = MiniGame::sit(table as u8).unwrap();
            let mut money = 20_000 + seed * 37;
            let mut client = ClientWindow { wallet: money, ..ClientWindow::default() };
            for step in 0..200 {
                // What the window lets the player press in its current view.
                let out = match game.state {
                    State::Slot if client.bet == 0 || (game.lines < MAX_LINES && choose(3) == 0) => game.bet_line(&mut money),
                    State::Slot => game.spin(&mut money, &odds, &mut rand),
                    State::SlotWon if choose(2) == 0 => game.again(&mut money),
                    State::SlotWon => game.double_up(&mut rand),
                    State::Card => game.guess(choose(2) == 0, &mut money, &odds, &mut rand),
                    State::CardWon if choose(3) == 0 => game.stop(&mut money),
                    State::CardWon => game.next_card(&mut rand),
                };
                client.answer(out.answer.as_deref().expect("every press the window allows is answered"));
                assert_eq!(client.wallet, money, "seed {seed}, step {step}: the wallet shown and the server's money");
                if game.state == State::Slot {
                    assert_eq!(client.bet, game.bet, "seed {seed}, step {step}: the bet shown");
                }
            }
            // Standing up: `0x3106` carries the money, which the window takes as it is.
            let before = money;
            game.stand(&mut money);
            let on_table = match game.state {
                State::Slot => client.bet,
                State::SlotWon => client.winnings,
                State::Card | State::CardWon => client.stake,
            };
            assert_eq!(money, before + on_table, "seed {seed}: standing up hands back what the window shows on the table");
        }
    }

    #[test]
    fn the_settings_give_the_stated_return() {
        // Over the classes' chances, the lines their patterns put among the counted ones and the mean multiplier 930/226, the original
        // returns about 41 % of the bet (section 4); 220 percent about 90 %.
        let mean = |percent: u32, lines: u8| -> f64 {
            let mut total = 0.0;
            let mut before = 0;
            for (class, &end) in CLASS_ENDS.iter().enumerate() {
                let chance = f64::from(end - before) / 10000.0;
                before = end;
                if class == 0 {
                    continue;
                }
                let class = if class == 7 { 6 } else { class };
                let (start, count) = if class == 8 { (281, 1) } else { (START[class], COUNT[class]) };
                let counted: f64 = (start..start + count)
                    .map(|k| LINES.iter().take(usize::from(lines)).filter(|l| l.iter().all(|&c| BINGO[k] >> c & 1 == 1)).count() as f64)
                    .sum::<f64>()
                    / count as f64;
                total += chance * counted * (930.0 / 226.0) * f64::from(percent) / 100.0;
            }
            total / f64::from(lines)
        };
        for lines in 1..=8 {
            let original = mean(100, lines);
            assert!((0.40..=0.42).contains(&original), "{lines} lines: {original}");
            let fairer = mean(220, lines);
            assert!((0.88..=0.93).contains(&fairer), "{lines} lines: {fairer}");
        }
    }
}
