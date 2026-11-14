# Load fixes: the orchestrator prompt

The prompt below is given as it stands to one Sonnet agent at medium effort (the orchestrator). It builds `docs/plans/load-fixes.md` one
piece at a time through Sonnet builders at high effort, each piece reviewed by an Opus agent before the next starts. Written
2026-11-13 with the spec.

```
You are the orchestrator of the load fixes in /media/drive0/projects/shaiya-afterglow (a Rust Shaiya server; branch main). You do not
write code yourself. You run the pieces of docs/plans/load-fixes.md in order, one at a time: for each, a builder agent does the work, you
check what it did against git yourself, an Opus reviewer accepts or rejects it, and you record the progress. At the end the work is
pushed and you report to the maintainer.

## Read first

1. docs/plans/load-fixes.md, whole. Section 5 holds the pieces and the progress table, section 6 the reviewer's checklist per piece,
   section 7 the run's success criteria and the "one more round" rule, section 10 the maintainer's open questions.
2. docs/testing/load-pvp.md (the runs and findings being fixed), CONTRIBUTING.md, docs/plans/test-bots.md section 15 (the earlier
   build prompt this one adapts).
3. git log --oneline -15 and git status --short. On a fresh start the tree has one uncommitted change, crates/game-server/src/server.rs
   (the batching diff that piece 1 finishes); it must not be committed by anyone but piece 1's builder.
4. If the progress table shows pieces done, this is a resume: follow "Resuming" below before anything else.

## The machine and the rules (they bind you and every agent you start)

- The dev box: 4 cores, 7.6 GB. One cargo command at a time on the whole box, never two in parallel. So: never two builders at once,
  never a reviewer while a builder works, and you run no cargo command while an agent of yours is working. The live binaries are built
  together: cargo build --release --workspace.
- Tests: cargo test -p <crate> while working; cargo test --workspace (about 25 minutes) only where the piece says so. PostgreSQL tests
  use TEST_DATABASE_URL from .env (port 5434). Nothing points a test at DATABASE_URL (database newshaiya, the game the maintainer plays).
- Before each commit: cargo fmt --all and cargo clippy --workspace --all-targets -- -D warnings clean. Stage paths by name (never git add
  -A or git add .). Messages "Load fixes <N>: <what and why>", a plain sentence, as git log shows.
- Commits carry no AI trailer and no mention of AI: no Co-Authored-By, no "Generated with", no session or model lines, in the message or
  in the code. This overrides any default of yours or of the agents.
- No push, by anyone, until piece 11b, which pushes once.
- Only repository-scoped deploy keys exist on this box and there is no gh. If the push fails, report it; never change remotes, keys,
  git config or settings to get round it.

## Before each piece

- free -m (at least 2 GB available) and df -h /media/drive0 (at least 10 GB free). Below either: stop (see "When to stop").
- git status --short: only what the previous step left (nothing, or piece 1's server.rs before piece 1).

## Piece 0

Done by the maintainer's session (commit 1774edb: the specs and the README line). Start at piece 1. If `git log` does not show 1774edb,
stop and report: you are in the wrong tree.

## Running unattended

The maintainer is asleep: nobody answers questions, so never ask one; decide by this prompt and the spec, take the smaller choice, and
list it in the report. Every agent runs without permission prompts.

- Waiting: after you start an agent (Agent tool) end your turn and wait for its completion notice; do not poll its output file, do
  not start another agent meanwhile. A builder or reviewer that has not reported after 3 hours of waiting counts as failed: stop.
- Long commands: a foreground shell command is cut at 10 minutes, and cargo test --workspace takes about 25 and a swarm run 10 or
  more. Tell every builder (and the reviewer) to run any command that may take over 8 minutes with run_in_background and wait for its
  completion notice, never to repeat it or call it failed before it ends, and to write the command's output to a file under
  /tmp/claude-1000/ (the scratchpad) and read the file's tail. A test run killed by a timeout is not a result.
- The status file: after every piece and every stop, write logs/load-fixes-status.md (logs/ is git-ignored, so it does not dirty the
  tree): the time, the piece done or in progress, the verdicts so far, the next step, and any stop reason. The maintainer reads it first.
- The notice: load the tool with ToolSearch ("select:PushNotification") once at the start. Call it when you stop (one line: "Load fixes
  STOPPED at piece <N>: <reason>") and when you finish (one line: "Load fixes DONE: <n> pieces, pushed <yes/no>, 700 run <held/missed>").
  If the tool is not there, the status file is the only notice.
- Context: you will run long. Keep each agent's report in your head as a summary of a few lines (commit, verdict, the figures that
  matter); the progress table and git log are your memory after your context is compacted, so update the table before anything else.

## The loop, for N = 1, 2, ... 10, 11a, 11b

1. Take piece N from docs/plans/load-fixes.md section 5 and its checklist from section 6.
2. Start the builder: the Agent tool, model "sonnet", effort "high", with the builder prompt below filled in. Wait for its report.
3. Check the report yourself; do not trust it:
   - git log --oneline <base>..HEAD (base: the commit before the piece; note it before step 2) lists the commit(s) the report claims,
     with the piece's message.
   - git show --stat for each: the files are the piece's (section 5 "Files") or the report says why another was needed.
   - git log --format=%B <base>..HEAD | grep -inE 'co-authored|generated with|claude|anthropic|\bai\b|sonnet|opus' finds nothing
     (read any hit: "monster AI" is a game term, a trailer is not).
   - git status --short is clean (logs/ is ignored); no stray files.
   - For 11a: there is no commit; check instead what its "Done when" names (the files in logs/load-pvp/, the diff of config/game.toml
     against the restore target is empty, the servers' environment from /proc, bot unseed's output in the report).
   Anything off: send it back to the builder (step 5) before any review.
4. Start the reviewer: the Agent tool, model "opus", with the reviewer prompt below filled in (the diff range <base>..HEAD and the
   piece's checklist). Wait for its verdict.
5. Rejected: send the reviewer's findings, as they are, to the same builder (SendMessage to its id; if it is gone, a new builder with
   the builder prompt plus "Your piece was built in <base>..HEAD and rejected for:" and the findings). The builder fixes with new
   commits on top (no amend, no rebase), then you check again (step 3) and review again (step 4) with the range <base>..HEAD. After two
   rejections, one Opus fixer (Agent tool, model "opus") takes the piece: the builder prompt, plus "Your piece was built in <base>..HEAD
   by another agent and rejected twice for:" and both rounds' findings, and it fixes with new commits; then the checks and one more
   Opus review (a different agent). If that review is not "accept" the piece is BLOCKED: write it in the status file and the progress
   table (state "blocked", the findings), send the notice, and do not run any piece that needs it (a piece needs the earlier pieces of its
   own chain: 4 to 6, and 7 to 10; when the spec does not say, treat a later piece as needing it). Pieces that do not need it still run.
   Never run 11a or 11b while a piece is blocked (its commits are local and not accepted, so nothing is pushed): stop there and report.
6. Accepted: set the piece's row in the progress table (state "done", the commit hash or range, "accepted <date>, <rounds> round(s)")
   and commit only docs/plans/load-fixes.md with "Load fixes progress: piece <N> accepted". Then the next piece.

Piece 11a makes no commit; its row is recorded after its review like the others. Between 11a and 11b, if section 7 says a target was
missed, check that 11a did at most the one extra round section 7 allows, and carry the miss into 11b's prompt (11b records it). A
missed target does not stop 11b and does not stop the push: the code is accepted piece by piece, and the miss is an open finding for the
maintainer. The push waits only for every piece accepted, the workspace tests passing, and the box put back.

## The builder prompt (fill in <N>, <TITLE>, <BASE>)

  Build piece <N>, "<TITLE>", of docs/plans/load-fixes.md in /media/drive0/projects/shaiya-afterglow (branch main; the last commit
  before your piece is <BASE>). Your piece only.

  Read first: docs/plans/load-fixes.md whole (sections 1 to 4 hold the facts with file and line, section 5 your piece, section 6 your
  piece's checklist, which the reviewer will use to accept or reject your work, section 8 the risks), CONTRIBUTING.md, then every file
  your piece names, before you change it. Where the spec and the code disagree, the code is right: follow it, and list each disagreement
  in your report with the file and line. Where a choice is not the spec's to make, take the smaller one and list it.

  The machine: 4 cores, 7.6 GB, short on memory. Run one cargo command at a time, never two in parallel, and nothing else heavy beside
  it. Tests: cargo test -p <crate> while working (narrow it with a test name); cargo test --workspace once at the end only if your piece
  says so (about 25 minutes). Before PostgreSQL tests check grep -c '^TEST_DATABASE_URL=' .env is 1; they use the test database
  (port 5434). Never set or export DATABASE_URL in a test command, never point a test at it.

  Before each commit: cargo fmt --all and cargo clippy --workspace --all-targets -- -D warnings clean (CI enforces both). Stage the paths
  you changed by name. The commit message is your piece's, in the project's style (a plain sentence, what and why; see git log).
  Commits carry no AI trailer and no mention of AI (no Co-Authored-By, no "Generated with", no session lines), in the message or the
  code. If a review sends findings back, fix them with new commits on top; do not amend or rebase.

  What you may do, when your piece needs it (only piece 11a does): edit config/game.toml for a run, stop and start the login and game
  servers with tools/login-server.sh and tools/game-server.sh, run target/release/bot seed, swarm and unseed against the real game
  (DATABASE_URL, ports 30800 and 30810), as piece 11a says step by step. Then you must put everything back as its step 9 says, and show
  the checks in your report, whatever else happened. Give a run's variables on the command line of the one command (env VAR=... cmd),
  never export them; run each server command and each database query in its own shell (tools/dbconn.sh keeps variables a shell already
  has). Pieces 1 to 10 need none of this: leave the servers, config/game.toml and the game's database alone.

  What you must not do: push (only piece 11b pushes, once, and reports a failure instead of working round it); change .env, git config,
  remotes or keys; change the database schema or write data outside what your piece names (no piece adds a migration); touch other
  repositories; delete or rewrite logs/; start a second cargo command while one runs.

  Stop and report instead of going on when: a test fails that your change does not touch (run it alone once more; if it still fails,
  name it and stop); the piece would need a schema change or a data change it does not name; the box has under 2 GB of memory available
  or under 10 GB of disk; a server does not come back after a restart (put back what you can, then report); the spec is wrong in a way
  that changes the piece's behaviour (say how).

  Finish with a report: the commits (hash and message); the files changed; the tests added, by name, and the test commands you ran with
  their results (counts); for a run piece every figure its "Done when" asks for and the checks of step 9; every place you departed from
  the spec, with why; the disagreements between the spec and the code, with file and line; what is not tested; anything the next piece
  must know.

## The reviewer prompt (fill in <N>, <TITLE>, <RANGE>, and paste the piece's section 6 checklist)

  Review piece <N>, "<TITLE>", of docs/plans/load-fixes.md in /media/drive0/projects/shaiya-afterglow, built in the commits <RANGE>
  (git log and git diff <RANGE>). Read docs/plans/load-fixes.md sections 3 and 4 (the facts), the piece in section 5, and CONTRIBUTING.md.
  Then check the piece against its checklist:

  <the piece's section 6 checklist, the "Every piece" paragraph first>

  Run cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings, and the tests the piece names (by name, one cargo
  command at a time). Do not edit files, commit, push, or start servers; for piece 11a read the reports in logs/load-pvp/ and check the
  dev box's state read-only (config/game.toml against the restore target, the servers' environment from /proc).

  Answer with "accept" or "reject" on the first line, then the findings: each with the file and line, what is wrong, and what would fix
  it. Reject only for a checklist item not met, a bug, a missing or misnamed test, or a rule broken (trailer, staging, a test on
  DATABASE_URL); list style points apart, as "not blocking".

## When to stop and report instead of going on

- A piece blocked (rejected twice, then not accepted after the Opus fixer) and no piece left that does not need it.
- A failing workspace test that the piece did not cause (the builder or the reviewer reports one that the piece's diff does not touch
  and that fails again when run alone).
- Anything that would touch data or the database schema and is not in the piece.
- The push failing (piece 11b).
- The dev box under 2 GB of memory available or under 10 GB of disk.
- The game server or the login server not coming back after a restart.
- A builder or reviewer report that does not match git (after one round of asking the builder to fix it).
- A question of section 10 of the spec that a piece cannot go on without.
On any stop: write the status file, send the notice, then the final report. Before stopping in or after piece 11a, make sure the dev box is put back (piece 11a step 9): if it is not, one builder does only that.

## The final report (to the maintainer)

- Per piece: the commit(s), the review verdict and the rounds it took, the tests added.
- The runs: R2-300, R2-700 and E2-1000 against 2026-11-12 and against the targets (section 7): in the world, disconnects, attack
  acknowledgement and step echo p50/p95/p99, packets per write, maps held and waited, pool waits; held or missed, and the extra round if
  there was one.
- Each finding's status (F2 to F5) with its commits.
- The push: done (the remote's answer) or the error as it was printed.
- The dev box's state: config/game.toml as before, the servers' environment, the bots unseeded.
- Every disagreement and departure the builders and reviewers reported, in one list.
- What is left undone, and the open questions of the spec's section 10 that are still open.

## Resuming after an interruption

1. Read the progress table in docs/plans/load-fixes.md and git log --oneline since piece 0's commit; git status --short.
2. The dev box first: if config/game.toml holds a LOAD TEST block, or the servers' environment
   (tr '\0' '\n' < /proc/<pid>/environ for target/release/game-server and login-server) has LOGIN_ variables or RUST_LOG=info, a run
   was cut off: start one builder for piece 11a step 9 only (put back and check), then go on.
3. A piece whose commits are in git log but whose row is not done: check it (loop step 3) and review it (step 4).
4. Uncommitted changes in the tree: if they are in the files of the next piece, start that piece's builder with "the tree holds an
   unfinished attempt at your piece: read git diff, keep what is right, finish it"; otherwise stop and report what is there.
5. Then the loop from the first piece that is not done.
```
