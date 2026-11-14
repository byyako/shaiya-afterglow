# plans: what is planned and what was planned

The live plan is [`roadmap.md`](roadmap.md): what is next, what is frozen, and what is done. The other files are the plans behind it.

- [`roadmap.md`](roadmap.md): the live plan.
- [`release.md`](release.md): the plan for the open-source repository, step by step.
- [`extensions.md`](extensions.md): the design of the extensions (rules and pets) and their work, chunk by chunk.
- [`ext-1-foundation.md`](ext-1-foundation.md): the spec and build plan of the first extensions chunk (reconciled with 0.2.0 on 2026-11-02; not built).
- [`ext-2-rules.md`](ext-2-rules.md): the spec and build plan of the second extensions chunk, rules and hooks (specified and built 2026-11-04).
- [`ext-3-effects.md`](ext-3-effects.md): the spec and build plan of the third extensions chunk, item effects and skill abilities (specified and built 2026-11-05).
- [`ext-5-lua.md`](ext-5-lua.md): the spec and build plan of the fifth extensions chunk, Lua scripts (specified 2026-11-04; built 2026-11-04).
- [`player-guide.md`](player-guide.md): the spec and build plan of the docs site's player guide, starting with the stat system (specified and built 2026-11-05).
- [`enchant-table.md`](enchant-table.md): the enchant rates and add values from the operator's `Data/ItemEnchant.ini`, as the original loads them (decided 2026-11-05; built 2026-11-06).
- [`episode-adoptions.md`](episode-adoptions.md): what the EP3 and EP5.4 server builds suggest for the new server (bug fixes, features, structure), ranked; not on the roadmap yet (written 2026-11-09).
- [`extensions-split.md`](extensions-split.md): the extensions move to their own repository; the server keeps the extension system and the engine work (specified and built 2026-11-12; the new repository is local, not yet on GitHub).
- [`test-bots.md`](test-bots.md): the test bots, first as a 200 to 300 player PvP battle to load the server and the client (specified 2026-11-12; parts 1 to 4 built; the runs at 300, 700 and 1000 bots done on 2026-11-12 and 2026-11-14, bots only, written up in [`../testing/load-pvp.md`](../testing/load-pvp.md); the runs with the real client are still to do).
- [`load-fixes.md`](load-fixes.md): what the PvP load runs found (write batching, the session lockout, the database pool at entry, the `maps` lock), in pieces, with the prompt that builds them one at a time ([`load-fixes-prompt.md`](load-fixes-prompt.md)) (specified 2026-11-13; built 2026-11-14).
- [`ideas.md`](ideas.md): ideas and discussions that are parked, with what each would need.
- [`balance/`](balance/README.md): the balance analysis and its plan (parked; it would come as extension work).
- [`done/`](done/README.md): the plans of what has been built, kept as the record of the design.
