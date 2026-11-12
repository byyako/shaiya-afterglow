# Kill streaks and bounties

A script extension (`docs/server/scripts.md`). A **streak** is the player kills a player makes in a row without dying. The streak
lengths in `announce_at` are announced to everyone. From `bounty_from` kills the server puts a **bounty** on the streaker that grows by
`bounty_gold` for each kill (at most `bounty_cap`); players can add gold to anyone's bounty with `!bounty <name> <gold>` (the gold is
taken from them at once and never refunded). Whoever kills a player with a bounty takes all of it, and the streak ends.

| command | does |
|---|---|
| `!streak` | your streak, your best, the bounty on you |
| `!bounty <name> <gold>` | add gold to a player's bounty (at least `player_bounty_min`) |
| `!bounties` | the five largest bounties of the players online |

Options (`[ext.kill-streak]` in `config/game.toml`, or the data editor's `ext.kill-streak.<key>`; `console <id> reload` puts them in
force): `maps` (empty: everywhere), `announce_at`, `bounty_from`, `bounty_gold`, `bounty_cap`, `player_bounty_min`, `repeat_secs` (a kill
of the same victim within this time does not count). Stored values: per character `streak`, `best`, `last_victim`, `last_at`; the
extension's own `bounty.<character id>`. Removing the extension drops them.

    target/release/data-editor ext install extensions/kill-streak
    target/release/console 1 reload
