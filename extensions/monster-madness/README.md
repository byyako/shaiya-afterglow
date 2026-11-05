# Monster madness

A script extension (`docs/server/scripts.md`). A staff member of level 1 to 3 types `!madness <monster id> <count> <minutes>`: the monsters
appear around them (at most `max_alive` of the extension alive, a madness of at most `max_minutes`), the map is told, and the kills are
counted. The end is announced when the last falls or the time runs out; `!madness stop` removes them quietly. Their kills give
experience and drops as any monster's. With `item` set (`type/type_id`), using that item starts a madness of `item_mob` x `item_count`
for `item_minutes` around its user and uses one up.

Options (`[ext.monster-madness]` in `config/game.toml`, or the data editor's `ext.monster-madness.<key>`; `console <id> reload`):
`mobs` (the ids `!madness` may name; empty for any), `max_alive`, `max_minutes`, `item`, `item_mob`, `item_count`, `item_minutes`.

    target/release/data-editor ext install extensions/monster-madness
    target/release/console 1 reload
