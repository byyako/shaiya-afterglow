# Kill Boost Stones

Three stones (items 100/212 to 214) whose buff (skill 200, levels 1 to 3) multiplies the player kills the wearer counts, 2x, 5x and 10x,
for one hour. The multipliers are `kill_boost` abilities on the skill rows (`rows.toml`); the stones look like the Mini Sun EXP Stone.

    target/release/data-editor ext install extensions/kill-boost      # then restart the game server
    tools/setup.sh --extension extensions/kill-boost <PSM_Data> <PS_Game/Data>

The rows are `rows.toml`; its client fields (names, texts, the look-alike) go into the next client data patch
(`tools/sdata/client_patch.py`). Removing the extension is refused while players hold a stone or its buff
(`data-editor ext remove kill-boost`, `--force` deletes them). More: `docs/plans/ext-1-foundation.md` section 6.

## Version 1.1.0: the multipliers are abilities

Version 1.0.0 set the setting `kill_boost_skills = "200/1:2,200/2:5,200/3:10"`. From 1.1.0 each skill row has an ability instead, the
registry of `docs/server/effects.md` (`docs/plans/ext-3-effects.md` section 5.4):

    abilities = [{ type = "kill_boost", times = 2 }]      # 5 and 10 on levels 2 and 3

What the players see and what the kills count does not change: the `defs` rows and the client rows are the same as in 1.0.0, so the client
patch (109) is unchanged. The ability is server-only data in `content.ext_skill_abilities`, read when the game server starts.

To move a server where 1.0.0 is installed:

    target/release/data-editor ext upgrade extensions/kill-boost      # then restart the game server

The upgrade replaces the extension's setting default with the abilities. It is optional: until it is done the setting keeps working (the
setting is a shorthand that gives the same abilities when the server starts), and nothing breaks without it. Disabled, the extension's
abilities are not loaded: the stones and buffs stay and count once, as before.
