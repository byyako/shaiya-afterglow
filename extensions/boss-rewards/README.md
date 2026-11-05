# Boss and kill rewards

A script extension (`docs/server/scripts.md`). When a **boss of a boss group** dies, every player the kill was shared with (the killer, or
the party members near) gets `item` (`item_count` of it), the buff `buff` and `gold`, and `announce` tells everyone who defeated which
boss. A **player kill** (not a duel or a guild war) gives the players it was shared with `pvp_item` and `pvp_gold`, on `pvp_maps`.

Options (`[ext.boss-rewards]` in `config/game.toml`, or the data editor's `ext.boss-rewards.<key>`; `console <id> reload`): `item`,
`item_count`, `buff`, `gold`, `announce`, `pvp_item`, `pvp_gold`, `pvp_maps`. With everything at its default the extension gives
nothing but the announcement. An item that does not fit in the bags is not given.

    target/release/data-editor ext install extensions/boss-rewards
    target/release/console 1 reload
