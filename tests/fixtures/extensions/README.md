# Extension fixtures

Frozen copies of extensions that this repository used to carry, read by the engine's tests (`crates/data-editor/tests/scripts.rs`,
`effects.rs`, `crates/game-server/tests/game_flow.rs`). They test the **extension system** (the manifest, rows, item effects, skill
abilities and the Lua API) through real folders.

The extensions themselves live in their own repository, `byyako/shaiya-afterglow-extensions` (`docs/plans/extensions-split.md`), and
are installed from there. Do not add content here: a new extension goes to that repository. When an engine change alters what one of
these folders must say, change the fixture here and the real extension there.
