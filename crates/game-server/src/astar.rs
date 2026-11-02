//! The way a monster searches when its straight line is blocked (`AStar::FindPath`, `docs/original/pathing.md` section 1).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::fight::Terrain;

/// The box around the start and the goal reaches at most this far beyond them on each axis.
const BOX: i32 = 20;
/// The neighbours of a cell, in the order they are tried (the tables at 0x542dec and 0x542dcc).
const NEIGHBOURS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0)];

struct Node {
    cell: (i32, i32),
    g: f32,
    /// The node it was reached from.
    from: Option<usize>,
}

fn squared(a: (i32, i32), b: (i32, i32)) -> f32 {
    let (dx, dz) = ((a.0 - b.0) as f32, (a.1 - b.1) as f32);
    dx * dx + dz * dz
}

/// What the search knows of each cell of its box (and the ring around it, where neighbours of its edge lie): whether it is blocked
/// (asked once), the open entry (its place in the queue and node) and the closed node.
#[derive(Clone, Copy, Default)]
struct Slot {
    blocked: Option<bool>,
    open: Option<(u64, usize)>,
    closed: Option<usize>,
}

/// The open tree as its order keeps it: lowest `F` first, the earliest put in of equal ones (`F` is never negative, so its bits
/// order as it does). Entries taken out of the tree stay in the heap and are passed over.
struct Open {
    heap: BinaryHeap<Reverse<(u32, u64, usize)>>,
    count: u64,
}

impl Open {
    fn push(&mut self, slot: &mut Slot, f: f32, n: usize) {
        self.count += 1;
        self.heap.push(Reverse((f.to_bits(), self.count, n)));
        slot.open = Some((self.count, n));
    }
}

/// A way from `start` to `goal` over free cells: the cells after the start, each where the way turns, the goal last. `None` when
/// the search runs out of cells inside its box. The start and the goal the same: an empty way.
pub fn find_path(start: (i32, i32), goal: (i32, i32), terrain: &impl Terrain) -> Option<Vec<(i32, i32)>> {
    if start == goal {
        return Some(Vec::new());
    }
    let (bx, bz) = (((goal.0 - start.0).abs() / 2).min(BOX), ((goal.1 - start.1).abs() / 2).min(BOX));
    // The lower ends raised to 0 and the upper ones lowered to the map's last cell change nothing: such cells are blocked.
    let (x0, x1) = (start.0.min(goal.0) - bx, start.0.max(goal.0) + bx);
    let (z0, z1) = (start.1.min(goal.1) - bz, start.1.max(goal.1) + bz);
    let width = (x1 - x0 + 3) as usize;
    let mut slots = vec![Slot::default(); width * (z1 - z0 + 3) as usize];
    let at = |cell: (i32, i32)| (cell.1 - z0 + 1) as usize * width + (cell.0 - x0 + 1) as usize;
    // Nodes taken out of a tree stay here: what was reached from them still leads back through them.
    let mut nodes = vec![Node { cell: start, g: 0.0, from: None }];
    let mut open = Open { heap: BinaryHeap::new(), count: 0 };
    open.push(&mut slots[at(start)], squared(start, goal), 0);
    while let Some(Reverse((_, seq, c))) = open.heap.pop() {
        let here = at(nodes[c].cell);
        if slots[here].open != Some((seq, c)) {
            continue;
        }
        slots[here].open = None;
        if nodes[c].cell == goal {
            return Some(way(&nodes, c));
        }
        let (cx, cz) = nodes[c].cell;
        if (x0..=x1).contains(&cx) && (z0..=z1).contains(&cz) {
            for (dx, dz) in NEIGHBOURS {
                let cell = (cx + dx, cz + dz);
                let slot = &mut slots[at(cell)];
                if *slot.blocked.get_or_insert_with(|| terrain.blocked(cell)) {
                    continue;
                }
                let g = nodes[c].g + (dx * dx + dz * dz) as f32;
                if let Some((_, o)) = slot.open {
                    if nodes[o].g <= g {
                        continue;
                    }
                    slot.open = None;
                }
                if let Some(o) = slot.closed {
                    if nodes[o].g <= g {
                        continue;
                    }
                    slot.closed = None;
                }
                nodes.push(Node { cell, g, from: Some(c) });
                open.push(slot, g + squared(cell, goal), nodes.len() - 1);
            }
        }
        slots[here].closed = Some(c);
    }
    None
}

/// The cells from the start to `end`, straight runs cut to their ends (`AStar::PathToLine`), the start left out.
fn way(nodes: &[Node], end: usize) -> Vec<(i32, i32)> {
    let mut cells = vec![nodes[end].cell];
    let mut at = nodes[end].from;
    while let Some(n) = at {
        cells.push(nodes[n].cell);
        at = nodes[n].from;
    }
    cells.reverse();
    let step = |a: (i32, i32), b: (i32, i32)| (b.0 - a.0, b.1 - a.1);
    (1..cells.len())
        .filter(|&i| i + 1 == cells.len() || step(cells[i - 1], cells[i]) != step(cells[i], cells[i + 1]))
        .map(|i| cells[i])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map of `size` cells a side with the listed cells blocked.
    struct Grid {
        size: i32,
        walls: Vec<(i32, i32)>,
    }

    impl Terrain for Grid {
        fn line(&self, _: (i32, i32), _: (i32, i32)) -> Result<(), (i32, i32)> {
            unreachable!("the search does not test lines")
        }
        fn blocked(&self, at: (i32, i32)) -> bool {
            !(0..self.size).contains(&at.0) || !(0..self.size).contains(&at.1) || self.walls.contains(&at)
        }
    }

    #[test]
    fn open_ground_is_one_straight_run() {
        let open = Grid { size: 64, walls: vec![] };
        assert_eq!(find_path((10, 10), (20, 10), &open), Some(vec![(20, 10)]));
        assert_eq!(find_path((10, 10), (15, 15), &open), Some(vec![(15, 15)]), "a diagonal run");
        assert_eq!(find_path((10, 10), (10, 10), &open), Some(vec![]));
        // Squared distance pulls the search onto the diagonal; on equal F the neighbour tried first wins: straight before diagonal.
        assert_eq!(find_path((10, 10), (20, 13), &open), Some(vec![(12, 12), (19, 12), (20, 13)]));
    }

    /// Every run is straight along one of the eight directions and crosses no blocked cell, and the way ends at the goal.
    fn walkable(start: (i32, i32), goal: (i32, i32), way: &[(i32, i32)], grid: &Grid) {
        assert_eq!(way.last(), Some(&goal));
        let mut at = start;
        for &next in way {
            let (dx, dz) = (next.0 - at.0, next.1 - at.1);
            assert!(dx == 0 || dz == 0 || dx.abs() == dz.abs(), "{at:?} -> {next:?}");
            let n = dx.abs().max(dz.abs());
            for i in 1..=n {
                assert!(!grid.blocked((at.0 + dx / n * i, at.1 + dz / n * i)), "{at:?} -> {next:?}");
            }
            at = next;
        }
    }

    #[test]
    fn a_wall_is_gone_round() {
        // A wall across z = 20 from x = 5 to x = 18 with a gap at x = 16; from (10, 10) to (14, 30) the box is x 8..16, z 0..40, so
        // the wall's ends lie outside it and the way goes through the gap.
        let wall = Grid { size: 64, walls: (5..=18).filter(|x| *x != 16).map(|x| (x, 20)).collect() };
        let way = find_path((10, 10), (14, 30), &wall).expect("a way through the gap");
        walkable((10, 10), (14, 30), &way, &wall);
        assert!(way.contains(&(16, 20)), "{way:?}");
    }

    #[test]
    fn no_way_inside_the_box() {
        // The goal walled in on all eight sides.
        let walls = NEIGHBOURS.iter().map(|(dx, dz)| (30 + dx, 30 + dz)).collect();
        assert_eq!(find_path((20, 30), (30, 30), &Grid { size: 64, walls }), None);
        // Start and goal on one row: the box is that row, and a wall across it cannot be gone round, however short.
        let post = Grid { size: 64, walls: vec![(12, 10)] };
        assert_eq!(find_path((10, 10), (14, 10), &post), None);
        // Two rows apart, the box is z 9..13: round the post.
        let way = find_path((10, 10), (14, 12), &post).expect("round the post");
        walkable((10, 10), (14, 12), &way, &post);
    }

    #[test]
    fn diagonals_slip_between_blocked_corners() {
        let corners = Grid { size: 64, walls: vec![(11, 10), (10, 11)] };
        let blocked_line = Grid { size: 64, walls: vec![(11, 10), (10, 11), (11, 11)] };
        assert_eq!(find_path((10, 10), (12, 12), &corners), Some(vec![(12, 12)]));
        assert_ne!(find_path((10, 10), (12, 12), &blocked_line), Some(vec![(12, 12)]));
    }

    /// The cells round (145, 145) blocked: no way in.
    struct Ring;
    impl Terrain for Ring {
        fn line(&self, _: (i32, i32), _: (i32, i32)) -> Result<(), (i32, i32)> {
            unreachable!()
        }
        fn blocked(&self, at: (i32, i32)) -> bool {
            !(0..256).contains(&at.0) || !(0..256).contains(&at.1) || ((at.0 - 145).abs().max((at.1 - 145).abs()) == 1)
        }
    }

    /// A search that finds no way goes through its whole box (about 46,000 nodes for an 85 × 85 box, cells being reached again
    /// more cheaply); `cargo test --release -- --ignored --nocapture` shows how long that takes.
    #[test]
    #[ignore]
    fn worst_case_time() {
        let t = std::time::Instant::now();
        for _ in 0..10 {
            assert_eq!(find_path((100, 100), (145, 145), &Ring), None);
        }
        eprintln!("10 failed searches: {:?}", t.elapsed());
    }
}
