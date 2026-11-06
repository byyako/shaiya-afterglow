# docs/guide: the player guide

A plain-language guide to the game's mechanics for players, published on the docs site as the "Player guide" tab (`site/mkdocs.yml`;
the tab's landing page is `site/content/guide.md`). It describes the original EP4.5 server and the US client; each page ends with what
Shaiya Afterglow does differently. The reference notes it draws on are in `docs/original/` (with addresses) and `docs/client/`; the plan
is `docs/plans/player-guide.md`.

| page | what |
|---|---|
| [`character-numbers.md`](character-numbers.md) | the six stats, points, HP/SP/MP, the fighting numbers, equipment, lapis and enchants, buffs and passives, speeds, recovery, modes, the faction kill-count bonus, the character window; worked example A |
| [`hitting.md`](hitting.md) | the timeline, hit chance, damage on monsters, skills, monsters' blows, player against player, experience; worked examples B, C, D |
| [`elements-defences.md`](elements-defences.md) | elements, defence and magic defence, magic resistance, debuff resistance, absorption, blocks and reflection, lapis option skills; worked example E |

The worked examples were computed by hand and checked with `tools/balance/` (`formulas.py`, `expect.py`); their builds are model strings
there, so a change of a formula can be rechecked.
