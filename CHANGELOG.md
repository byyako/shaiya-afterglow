# Changelog

## 0.1.0

The first release: a Shaiya Episode 4.5 server in Rust with PostgreSQL, for the unmodified US client (patch 108).

**Included**
- The login server and the game server, with every game system of the original EP4.5 server built from its analysis notes
  (`docs/original/`): login and the server list, the character screen, the world, items and shops, combat and skills, monsters and
  their AI, parties, trade, guilds and guild houses, quests, PvP, duels and guild wars, obelisks, instance zones, personal shops and the
  auction house, the item mall, the gambling house, the game and chat log, the operator (GM) commands and the operator console.
- The operator's tools: the console, the data editor (a web page with a drawn map, history and undo), the server list, account and
  item mall tools, and the import scripts for the original's data (`tools/`, described in `tools/README.md`).
- A patch kit for the original `ps_game.exe` (`tools/server-patches/`).
- Setup from a fresh clone with `tools/setup.sh` and `compose.yaml`; CI with format, lint, tests and a link check.
- The analysis notes, the research on the client, the plans and the live-test checklist (`docs/`).

**Behaviour**
- The defaults are the original's. Fixes to the original's crashes, duplication holes, exploits and limits are on; gameplay changes
  are settings (`docs/server/settings.md`); additions are off (`docs/server/deviations.md`).
- Not built, by decision: the second game server for map 200 and the original console's live reloads beyond `reload`.

**Known limits**
- Part of the live-test checklist has not been walked with a real client yet (`docs/testing/live-test-checklist.md`).
