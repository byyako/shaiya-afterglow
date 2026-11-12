# Welcome, adverts and chat commands

A script extension (`docs/server/scripts.md`). At login the player gets a whisper from `Server` and a notice with `text` (`{name}` is the
character's name). Every `advert_minutes` the next line of `adverts` is sent as a world notice, or as the centre-screen banner with
`advert_kind = "banner"` (how the banner reads in the client is open: `docs/testing/client-research-queue.md` entry 22).

Commands: `!help` (the commands the player may use, from every enabled extension), `!online` (the number of players), `!time` (the server's
local time).

Options (`[ext.welcome]` in `config/game.toml`, or the data editor's `ext.welcome.<key>`; `console <id> reload`): `text`, `adverts`,
`advert_minutes`, `advert_kind`.

    target/release/data-editor ext install extensions/welcome
    target/release/console 1 reload
