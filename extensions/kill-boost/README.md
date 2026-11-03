# Kill Boost Stones

Three stones (items 100/212 to 214) whose buff (skill 200, levels 1 to 3) multiplies the player kills the wearer counts, 2x, 5x and 10x,
for one hour. The multipliers are the setting `kill_boost_skills` in `extension.toml`; the stones look like the Mini Sun EXP Stone.

    target/release/data-editor ext install extensions/kill-boost      # then restart the game server
    tools/setup.sh --extension extensions/kill-boost <PSM_Data> <PS_Game/Data>

The rows are `rows.toml`; its client fields (names, texts, the look-alike) go into the next client data patch
(`tools/sdata/client_patch.py`). Removing the extension is refused while players hold a stone or its buff
(`data-editor ext remove kill-boost`, `--force` deletes them). More: `docs/plans/ext-1-foundation.md` section 6.
