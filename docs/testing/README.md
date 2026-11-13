# testing: what has to be walked with a real client

The automated tests cover the rules and the packets; what a real client shows still has to be walked by hand. Each item of the
checklist is a thing to do and what should happen; the results are marked on the checklist page, which keeps them in
`logs/live-test-results.json`.

- [`live-test-checklist.md`](live-test-checklist.md): every live test, by section. The page is built from it (`tools/checklist_page.py`)
  and served by `tools/checklist-server.sh` (port 30880).
- [`live-test-solo.md`](live-test-solo.md): which items one tester can do alone, and the sittings that group them.
- [`smoke-list.md`](smoke-list.md): the short list for before opening to players; it is section 0 of the checklist, copied there by
  `tools/smoke-list-sync.sh`, so edit it here.
- [`client-research-queue.md`](client-research-queue.md): questions the server's notes left open that only the client or a live session
  can answer, and where each answer is.
- [`load-pvp.md`](load-pvp.md): the PvP swarm runs (the test bots, `docs/plans/test-bots.md`): the settings for a run, the steps, and
  each run's results.
