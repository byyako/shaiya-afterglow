# Quality-of-life features: what the client allows

This page assesses which quality-of-life changes the US `game.exe` (build 2010-04-26, patch 108, 32-bit x86) allows, mostly through the
companion DLL (`fixes-dll.md`), with the client facts that decide each one and the cost. It was written from static analysis of
the client; none of the features is built or accepted in play. Work estimates are relative: **small** reuses a narrow established path;
**medium** adds a UI control or bounded protocol behaviour; **large** crosses native subsystems, persistence, replication or resource
lifecycles. They are not delivery times. Evidence labels: **observed** (read in the binary or data), **proposal** (engineering inference),
**unknown** (a trace or runtime check not resolved). Recovered pseudo-code is not original source; its signatures and stack arguments do not
establish a safe detour ABI.

## In short

- **Most proposals are feasible in the DLL,** sometimes with server work and assets. The strongest are inventory search, a sell button,
  sequential sort and stack consolidation, better tooltips, clearer cooldown and buff displays, selected-target HP visibility, and reuse of
  the native cloak, helmet and effect controls.
- **A third skill bar can reuse existing slot pages:** the native save path already handles five pages of ten general slots and four pages of
  six secondary slots, while the UI creates only two general widgets. The third widget is new UI and input work, not a dormant switch.
- **Ultrawide and 4K have a concrete obstacle:** the client recognises eight fixed resolution presets; an unrecognised size falls back to a
  preset state and a 1.25 aspect scalar, and a stock settings save can replace a custom mode with a preset. Support needs mode
  preservation, consistent projection and picking, readable UI scaling and matching hitboxes.
- **Wings, pets, costume overrides and custom titles are not established in this build:** hiding them depends on finding an existing
  rendering path; adding them is a separate DLL, server and asset project.
- **Server rules decide the rest:** remote selling, blacksmith and warehouse access, protected items, buyback and presets need explicit server
  entitlements and, for robust versions, new atomic operations or stable item identity.

## Feasibility matrix

| Proposal | Finding and DLL route | Server / assets | Work |
|---|---|---|---|
| Third visible skill bar | viable design on existing slot pages; the widget is new UI/input work | existing quick-slot persistence reusable with valid native bar and slot numbers; no new skill-use protocol in principle | large |
| FPS boost | targeted optimisations viable; no universal or measured gain (`graphics.md`) | client rendering and CPU; server latency is separate | medium/large |
| Faster loading | profiling, reuse or caching only where measurements justify | mostly client-side assets; world-entry server waits are separate | medium/large |
| Sell button near an NPC / anywhere | strong candidate using the existing sale requests | ordinary sales need no new protocol; a server may or may not require an NPC | medium |
| Blacksmith buttons near the NPC | strong candidate for repair, link, extract, enchant shortcuts, keeping a valid stock window context | existing handlers require the blacksmith in view (not a verified strict radius) | medium |
| Blacksmith anywhere | viable jointly; the stock NPC-dependent window lifecycle needs adapting | an explicit service entitlement, pricing and per-action validation | medium/large |
| Warehouse shortcut near the NPC / anywhere | viable; anywhere can reuse the recall-rune entitlement or a new permission | the entitlement must follow the window's closure lifecycle | medium |
| Toggle HP bars | selected-target draw branches mapped; overhead and party categories need their own traces | display-only hide is client-side | small/medium |
| Toggle cosmetic or skill effects | weapon and attachment gates mapped; per-owner and per-category filtering still unmapped | local; preserve meaningful combat cues | medium/large |
| Toggle cloak or helmet | confirmed native controls and actor rebuild paths | client-side | small/medium |
| Hide existing wings, pets, costumes, titles | conditional on proving a distinct draw path or actor category | local visibility must preserve actor and gameplay state | unresolved, potentially small to medium/large |
| Add wings, pets, costumes (transmog), custom titles | technically plausible custom attachment, follower, appearance or label systems | DLL + server state, ownership, persistence, replication + rig-compatible assets | medium (titles) to large |
| Ultrawide | concrete preset and aspect obstacles mapped; DLL fixes viable | client-only normally; projection, picking and all scenes must agree | medium/large |
| Readable 4K UI | new scaling, anchoring, font and hit-test work; resolution alone is insufficient | existing art gains no detail | large |
| Inventory search or highlight | strong local presentation candidate | none for read-only search | medium |
| Inventory sort | sequential acknowledged `0x0204` moves | whole-sort atomicity needs a new server operation | medium (sequence), larger (atomic) |
| Stack consolidation | compatible ordinary moves, respecting stack rules; recompute after replies | existing combination path | medium |
| Favourites | local per-type favourites or highlights | durable exact-copy bookmarks need stable instance identity | medium |
| Protected items | viable with server-enforced locks and DLL state and display | persist identity and enforce every destructive or transfer path | medium/large |
| Sell-junk preview | selection and value preview plus individual sales | the server sets prices; protected and batch semantics need support | medium |
| Buyback | new sales ledger and UI; no dormant stock implementation established | atomic sale/credit/ledger and buyback/debit/restore, exact metadata and expiry | large |
| Warehouse gold | dormant client request and answer paths exist, but the input dialog is unreachable (`packet-opportunities.md`) | a new persisted balance and transfer contract | medium/large |
| Numeric skill cooldowns | strong presentation candidate; readiness state and login delays identified | the server stays authoritative; shared cooldown semantics need validation | medium |
| Buff and debuff timers, warnings | viable; native duration field and warning conversion mapped | accurate modified or renewed durations may need extension; party refreshes need extra data | medium |
| Better tooltips, equipment comparison | shared renderer and item-instance data paths identified; no ready comparison switch | local information needs no protocol; custom server formulas need shared definitions | medium |
| Loot filters and highlighting | strong local presentation candidate | labels only need no server change | medium |
| Preferred or automatic pickup | viable with an explicit policy; distinguish the request queue from an authoritative automatic award | ownership, range, party and quest checks; reliable award and recovery | medium/large |
| Equipment presets | a local acknowledged move sequence is possible; a robust atomic set is joint work | stable per-instance identity, requirements and one transaction | large |
| Party and union readability | member and status data support layout and indicator improvements | extra roles, ready checks or accurate timers need a contract | medium |
| Event and boss notices | existing scoped notice and monster-speech routes usable | the server emits the events; basic notices need no DLL | small |
| Event or boss timer panel | viable custom panel | authoritative snapshots, deadlines, revisions and reconnect behaviour | medium/large |
| Target HP percentage, quick-slot swapping, skill-window lock, auto-dismount, resurrection-rune warning | already available as region features 26, 62, 45, 46 and 23 in the DLL's default set (`fixes-dll.md`) | live acceptance of each is separate | existing |

## Implementation boundaries

**Third bar.** The native serialiser (`0x4c5520`) emits general bars 0..4 with ten slots each, secondary bars 10..13 with six slots each, two
special bar-100 entries and a page-selection sentinel. Skill entries persist the stable **skill id**, and the restore remaps it to a
learned-skill runtime index; persisting that runtime index would be wrong. The `0x010B` receive callback is `0x567290`, reached through
scene vtable `0x6a5c20 + 0x200`. A server accepting up to 127 records does not make arbitrary bar and slot numbers safe: the native
restore function has unchecked index writes, including a wider secondary-bar test than the serialiser emits (`research-balance.md`
has the exact table limits). Use validated stock namespaces or intercept the extension before the native restore; do not invent a new bar
number. A bounded compatibility finding, not a completed security audit.

**Remote services and inventory.** Opening a window does not authorise an operation; remote selling, blacksmith and warehouse access each need
their own authorisation. Search and favourite-type presentation are local. Sort, stack and equipment actions must follow server
acknowledgments; a sequential operation can partly complete and must stop and reconcile after a timeout or a changed inventory. A
server batch can give atomic semantics, but that is a new operation. Client inventory records do not normally carry the persisted item id, so
exact-copy locks, robust presets and buyback need an identity design.

**Timers and social display.** Ordinary own-buff additions identify skill and level but omit the actual remaining time; login snapshots carry
remaining seconds. Party refreshes of the same skill and level may emit no new expiry information, and the native timing paths use
different units. A numeric timer must distinguish an authoritative remaining duration from an estimate derived from the data tables, and
must not show enemy buff state the server never supplied.

**Graphics and display.** Do not skip the whole target-frame routine to hide HP: it also clears invalid, dead and out-of-range targeting
state; suppress only the intended drawing branches. Native weapon effects have several feature gates, so one INI switch does not identify
all particles or cosmetic categories. Native cloak and helmet option changes rebuild actor components; direct global writes may not
reproduce that lifecycle. Preloading is synchronous current-map resource work, and archive lookup already uses hashes and shared
selected-entry state; a generic asynchronous loader is not justified by the evidence, so first separate I/O, decode and texture creation,
repetition and memory peaks (`graphics.md`). Existing mixed `Present` and loading diagnostics do not establish gameplay FPS, a speedup or a
whole loading-screen duration.

## Suggested order

1. **Smallest useful additions:** sell UI, inventory search and highlights, factual tooltip improvements, confirmed native visibility
   controls, and acceptance of the existing region features. Start display work by preserving custom modes and correcting validated aspect
   consumers.
2. **Broader client work:** sequential sorting and stacking, readable cooldown and buff timers, a third visible bar on existing slot
   pages, and consistent HUD, font and hit-test scaling.
3. **Joint server features:** protected items, explicit rules for remote blacksmith and warehouse use, warehouse gold, authoritative party and
   event timer information, and well-defined batch operations.
4. **Larger systems:** buyback, atomic equipment presets and new replicated cosmetic systems. Plan identity, persistence and assets before UI.

## Open

Separate cosmetic-system existence, safe detour ABIs, complete new-widget registration and all-scene UI scaling are only partly answered.
Negative name and path searches are bounded; they do not prove that an unnamed equivalent is absent. No live performance, visual or
multiplayer claim is made; each partly mapped feature needs its focused trace before a patch is built, and each built feature its own live
acceptance. Related open items: `re-open-items.md`.
