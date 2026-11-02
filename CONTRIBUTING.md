# Contributing

Thank you for looking. This project is a clean-room reimplementation of the Shaiya Episode 4.5 server, so a few rules matter more
than usual.

## The original comes first

The server's job is to behave as the original EP4.5 server did, minus its bugs (crashes, duplication holes, exploits). So:

- **A change in behaviour is a setting or an extension, not a new default.** Gameplay changes default to the original's behaviour;
  additions (new items, rules, content) are off by default. `docs/server/deviations.md` explains the three kinds and lists
  every place where the server differs from the original.
- **An analysis note comes before code.** The original's behaviour is read from its programs first and written up in
  `docs/original/<feature>.md` (packets, checks, numbers, and the addresses they come from, with the status words in
  `docs/README.md`). The code follows the note, and a deviation is written in the note with its reason. For a small fix, say in
  the pull request which note describes the behaviour, or add the missing paragraph to it.
- Do not guess at the original's rules; if you can't confirm one, say it is inferred.

## No original files

**Never post, attach or commit anything from the original game, client or server**: no executables, no data files (`*.SData`, maps,
`.saf` archives), no database backups, no decompiled source or large pieces of disassembly, no private server packs. This applies to
issues, pull requests and commits alike. Describe the location (a function name, an address, a packet) and the finding instead.
The importers read the operator's own copy, and `.gitignore` keeps `original/`, `reference/`, `data/` and `.env` out of the
repository; do not work around it.

## Working on the code

- Rust 2024, one workspace (`Cargo.toml`). Build and test as the README's "Development" section says:
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
  CI runs the same, plus `python3 tools/check_links.py`.
- **Tests with the change.** A rule gets a unit test with the numbers of its analysis note; a behaviour that crosses the network gets a
  test in the crate's `tests/`. The PostgreSQL tests use `DATABASE_URL` and skip themselves without it.
- **Docs with the change.** Settings go in `docs/server/settings.md`, a deviation in `docs/server/deviations.md`, what only a real client
  can show as an item in `docs/testing/live-test-checklist.md`. Run `python3 tools/check_links.py` after moving or renaming a file.
- Comments cite the analysis note by file name and section number; keep them current.
- Keep commits small and say why in the message. Don't commit secrets, local paths, addresses of your machines, or passwords.

## Reporting a bug

Open an issue with the version or commit, what you did, what you expected (and where the original does that: a note, a
packet, an address), what happened, and the relevant lines of `logs/game-server.log` or `logs/login-server.log`. For anything
that could be abused, use the private route in `SECURITY.md` instead.

## License

By contributing you agree that your contribution is released under the project's MIT license (`LICENSE`).
