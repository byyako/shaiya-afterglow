# Monster paths (A*)

This page covers how monsters in the EP4.5 game server `ps_game.exe` find their way round walls: the A* search, the
path a monster keeps (`CMob+0x7c`, an `AStar`), and how the five monster callers use it. It was read in the Ghidra decompiler and the
disassembly with the PDB names and types: `AStar::FindPath` 0x48eda0, `AStar::GetSuccessorList` 0x48ecf0, `AStar::PathToLine` 0x48f4a0,
`AStar::ResetPath` 0x48ec70, `AStar::SetPath` 0x4a6b20, `AStar::CursorNext` 0x4a6ae0, `AStarPriorityQueue::Push` 0x48e810 / `RecusFind`
0x48e7c0, `AStarMap::Add` 0x48ea80, `ChasePathList::AddPath` 0x48f660 / `GetNextPath` 0x48f700 / `Reset` 0x48f600, `CMap::IsBlockSafe`
0x41b420, `CBitArray::GetBit` 0x41b3a0, and the callers `CMob::Chase` 0x4a6de0, `CMob::ReturnHome` 0x4a7670, `CMob::ChaseExp` 0x4b6be0,
`CMob::WhileMove` 0x4b5790, `CMob::WhileReturnHome` 0x4b5c40. Tables: the neighbour offsets at 0x542dec (x) and 0x542dcc (z),
`g_ConstMobMoveSpeed` 0x542538.

This page completes chasing and going home on [Monsters fighting back](mob-fight.md#4-chasing) (a chase keeps the spot it set out
for; the chase path's newest cell is taken off when the monster sets out for it) and the Lua moves on
[Party instance zones](instances.md) (a Lua move goes back along the chase path or searches a way; `ChaseExp`'s full steps wait 400
ms less). Not covered: `CNpc::Chase` 0x4bca00 and `CNpc::ReturnHome` 0x4bcf30 (guard NPCs; see [Walking NPCs and guards](guards.md));
`CMob::ChaseStopAI` 0x4a6d10 has no caller.

## In short

- **Monsters walk straight when they can.** Only when a wall is in the way do they search for a route, square by square, inside a box
  around the start and the goal (at most 20 squares beyond either).
- **The search is not the textbook one**: it measures distance squared, so it heads straight for the goal and the route it finds is not
  always the shortest. If the monster and its target are on the same row or column, it cannot go round a wall across that line at all.
- **A chasing monster runs to where its target was**, then looks again. It keeps following an old route even after losing its target,
  until it next chases or goes home.
- **Going home** retraces the squares the monster ran through.

Status words: **confirmed** = read in the code (address given).

Cells are whole-number map squares; a cell is **blocked** when it is off the map or its bit is set (`CMap::IsBlockSafe`: bit
`z × size + x` of the map's block bits; see [Monsters](mobs.md#6-walking-about)).

## 1. The search

Confirmed, `AStar::FindPath(sx, sz, gx, gz, map)`.

1. **Start = goal**: true at once, the path untouched (no caller reaches this, below).
2. **The box**: `bx = min(|gx − sx| / 2, 20)`, `bz = min(|gz − sz| / 2, 20)` (integer halves); the box is
   `min(sx, gx) − bx … max(sx, gx) + bx` by `min(sz, gz) − bz … max(sz, gz) + bz`, the lower ends raised to 0, the upper ones lowered to
   `size − 1` (the map's cell count, both axes).
3. **Nodes** carry a cell, its **key** `x × 65536 + z`, `G`, `H`, `F = G + H` (all `f32`; the values are whole numbers), the node it was
   reached from, and the links of the tree it sits in. The start gets `G = 0`, `H = (sx − gx)² + (sz − gz)²`.
4. **Two trees**:
   - the **open tree**, ordered by `F`: a new node goes down **right when the node there has `F ≤` its own**, left otherwise
     (`AStarPriorityQueue::Push`); the next node to look at is the **leftmost** (taken out by joining its right subtree to its parent's
     left, or making it the root);
   - the **closed tree**, ordered by key: right when the key is **not below** the node's, left otherwise (`AStarMap::Add`); a look-up
     follows the same way down, testing each node it passes for an equal key.
   Removing a node from either tree cuts it from its parent (or empties the tree when it was the root), then puts its left subtree back,
   then its right one, each as a whole by its root's value (`Push` / `Add` of the subtree's root with its children attached). That keeps
   both trees in order (the subtrees land where the node was, the right one below the left one's rightmost node), so: **the open tree
   always gives the lowest `F`, the earliest put in of equal ones**, and the closed look-up always finds a cell that is there. A node
   is looked for in the open tree by visiting the whole tree (`RecusFind`); a cell is in each tree at most once.
5. The start goes into the open tree. Then, **until the open tree is empty** (→ **no path**, false):
   1. Take the leftmost node `c`. **When it is the goal cell**: the path is made (below), true.
   2. **Its neighbours** (`GetSuccessorList`), only when `c` lies **inside the box** (none otherwise): the eight cells at the offsets
      **(−1, −1), (0, −1), (1, −1), (1, 0), (1, 1), (0, 1), (−1, 1), (−1, 0)** in that order, each one on the map and not blocked (corners
      are not checked: a diagonal step between two blocked cells is allowed).
   3. For each neighbour `n` in that order: **`g = c.G + (n.x − c.x)² + (n.z − c.z)²`** (1 straight, 2 diagonal).
      - Already in the open tree with `G ≤ g`: `n` is dropped. With `G > g`: that node is removed from the open tree (item 4) and `n` goes
        on to the next test.
      - Found in the closed tree with `G ≤ g`: `n` is dropped. With `G > g`: that node is removed from the closed tree and `n` goes on.
      - `n` gets `G = g`, **`H = (n.x − gx)² + (n.z − gz)²`**, `F = G + H`, comes from `c`, and goes into the open tree.
   4. `c` goes into the closed tree.
   A node taken out of the closed tree is freed (`SSyncHeap<AStarNode>::Free`, a pool shared by every search) while the nodes reached
   from it still name it; the pool hands that memory to a later node, so a way that runs back through such a node is undefined in the
   original. The new server keeps the old node, so such a way leads back along the dearer route it was first reached by.
6. **The path**: from the goal back to the start along "comes from", then forwards. **Straight runs are cut to their ends**
   (`PathToLine`): a cell is left out when the step into it and the step out of it are the same. The start is not part of what the
   monster walks: **the path is the cells after the start**, each a turn of the way, the goal last.

The heuristic is the **squared** distance, so the search mostly heads straight for the goal, cells are often reached again more
cheaply (and looked at again), and the way found is not always the shortest. **Start and goal on one row or column** make the box one
cell wide that way: only that row is looked beyond, so a wall across it is never gone round (no way).

## 2. A monster's path

Confirmed, `CMob+0x7c`; `+0xa0` = it has one, `+0x8c` = the cell it walks to.

- **`SetPath(x, z)`**: the path is that one cell.
- **A search** that succeeds: the path is section 1's cells.
- **`CursorNext`**: the next cell; after the last one **the path is gone**.
- **`ResetPath`**: the path is gone.

While a monster has a path, chasing and going home **walk its cells and do not test the straight line again**. The path is gone only
where these functions say so (and when the monster is made, `CMob::Init`): **not when the target is dropped or changes** — a monster
that loses its target half way keeps walking towards that target's old spot when it next chases or goes home, and only then looks again.

**The chase path** (`ChasePathList`, `docs/original/mob-fight.md` section 4) is separate: the cells the monster ran through, which going
home walks back along.

## 3. Chasing

Confirmed, `CMob::Chase`; replaces `docs/original/mob-fight.md` section 4 step 5.

Steps 1 to 4 of that section stand (each way out of steps 2 and 4 **ends the path**). Then, with `m` the monster's truncated cell and `t`
the target's:

1. **Without a path**, `CMap::EnableLine(m, t)`:
   - **free**: `SetPath(t)`;
   - **blocked, `t` blocked**: as before — within 1 of the last free cell `f` (both axes): unfair (fair combat cleared), state 3, no
     target, the path ended; else `SetPath(f)`, and when the target is more than `bySize + entry 0's byRange + 0.3` from `f`: state 3, no
     target, the path ended;
   - **blocked, `t` free**: **the search** `FindPath(m, t)`; no way: **`CMob::StatusAttackStop`** (state 0, no target), the path ended.
2. **The cell walked to** `p` = the path's current cell; `v = p − m` (as floats), `L = |v|`, the step `s = g_ConstMobMoveSpeed[speed] +
   byChaseStep` (a whole number), and `ChasePathList::AddPath(m)` in every case below.
3. **`p` is not the path's last cell** (a turn of a searched way):
   - **`trunc(L) < s`**: `MoveMob` **onto `p`** (`(p.x, 0, p.z)`: the height becomes 0), the next chase time **now**
     (`trunc(L) / s × dwChaseTime`, an integer division), then **`CursorNext`**;
   - else **a full step** `position + v / L × s`, the next chase time `now + dwChaseTime`; the path stays.
4. **`p` is the last cell** (the target's spot when the path was set, the last free cell, or a searched way's end): the old step 5 with
   `p` in place of the target's cell: `g = L − (bySize + 0.4) − range`:
   - `g ≥ s`: a full step, next chase `now + dwChaseTime`; **the path stays**, so the next turns head for `p` again, not for where the
     target has gone;
   - else, `bySize + 0.6 − range ≥ L`: no move, next chase `now`, `CursorNext` (the path ends);
   - else: `MoveMob(position + v − v / L × (bySize + range + 0.4))`, next chase `now + trunc(g / s × dwChaseTime)`, `CursorNext`.

A chase whose target keeps moving therefore runs to the spot where the target was, then aims again: the in-range test of step 2 (with
the target's real position) still ends the chase as soon as the target is near.

All moves send `0x0603` with motion 1 (`CZone::MoveMob`, which refuses spots off the map).

## 4. Going home

Confirmed, `CMob::ReturnHome`; replaces `docs/original/mob-fight.md` section 5 step 3.

Steps 1 and 2 of that section stand (home when less than 10 from home, or less than 0.5 with a chase path left: the path ended too). At
move speed 255 nothing. Then, with `m` and the home's cell `h`:

1. **Without a path**, `EnableLine(m, h)`:
   - **free**: the cell walked to is `h` (**no path is set**: going home aims again each turn);
   - **blocked, `h` blocked**: when the last free cell is `m`, the home becomes where the monster stands; either way state 0, the chase
     path cleared, the path ended;
   - **blocked, `h` free, a chase path left**: its **newest cell is taken off** (`GetNextPath`) and `SetPath` to it;
   - **blocked, `h` free, no chase path**: **the search** `FindPath(m, h)`; no way: state 0 (a scripted monster: its machine reset), the
     home becomes where it stands, the path ended, the chase path cleared.
2. The cell walked to `p` (the path's current cell when there is one); `v = p − m`, `L = |v|`, `s` as for chasing, **`g = L − (bySize +
   1)`** — the same for every cell of a path:
   - `s ≤ g`: a full step `position + v / L × s` (motion 1), next chase `now + dwChaseTime`, the path stays;
   - else **onto `p`** (`(p.x, y, p.z)`, the height kept), next chase `now + trunc(g / s × dwChaseTime)`, **`CursorNext`** (nothing
     without a path).

## 5. Lua monsters

Confirmed, `CMob::WhileMove`, `WhileReturnHome`, `ChaseExp`; corrects `docs/original/instances.md` sections 12, 13.

- **`WhileReturnHome`** is section 4 exactly (the same tests, steps and times), with its own outcomes: home → `ResetAttack`, the damage
  record cleared, event 2; home blocked → event 2 (the home becoming the current spot when the last free cell is the monster's); no
  way → the home becomes the current spot, event 2.
- **`WhileMove`** (to the move spot `q`): arrival (less than 0.5 with a chase path left, else less than `5 + rand() % 5`, `rand()` drawn only
  then) as before. At move speed 255 nothing. Then section 4 item 1 **without the "`h` blocked" case**: free
  line → walk to `q`'s cell; blocked with a chase path → its newest cell, `SetPath`; blocked without → the search to `q`'s cell, **no
  way → the move ends** as on arrival (event 6, home = current spot, both paths cleared, then `ResetAttack` for move type 3 or
  `OnMoveEnd`). The steps are section 4 item 2's with **`byNormalStep` and `dwNormalTime`, motion 0**.
- **`ChaseExp`** is section 3 with its own ways of giving up (`docs/original/instances.md` section 13; each ends the path), and **every
  full step waits `dwChaseTime − 400`** (the waypoint's and the last cell's; the original's 32-bit sum, so a chase time below 400 means
  the next turn at once). Its in-range test ends the path as in `Chase`.

## 6. The new server

- **`crates/game-server/src/astar.rs`**: section 1 with the same order of neighbours, costs and tie order, so the same way is found;
  the trees are a heap (lowest `F`, then the order put in) and a grid over the box (open entry, closed node, blocked asked once per
  cell). A search that finds no way goes through its whole box: at most about 46,000 nodes (an 85 × 85 box), a few milliseconds.
- **The path** (`MobMap::paths`, by monster): the cells still to walk, the current first; none = no path. Ended where sections 3 to 5
  say, and when the monster leaves the map (death, removal). `fight::chase` and `fight::homeward` take it and the chase path; the Lua
  moves use `fight::homeward` and `fight::moving`.
- **Deviation** (approved):
  1. **A monster that reaches a turn of a searched way keeps its height** (the original sets it to 0; the client gets heights only in
     `0x0601`, so a player coming into view would see the monster at height 0 until it moves).
