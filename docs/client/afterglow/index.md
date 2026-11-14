# The US client and Shaiya Afterglow

This section of the Shaiya Afterglow tab is written from the client side. The Client tab describes what the 2010 US `game.exe` does;
this section turns those facts into advice for a server, lists the opportunities the client already supports, and records what
Shaiya Afterglow acted on. It is kept apart from the Client tab so that the client pages stay plain descriptions of the client: they say
"a server" and link here when a sentence tells a server what to do.

## In short

- The **Client tab** says what the client does. This section says what a server should do about it.
- Four pages: a checklist of what a server must send, check or refuse; the opportunities the client supports with no client change;
  the status of each client finding on Shaiya Afterglow; and this overview.
- "A server" means any server for this client. Shaiya Afterglow's own status is given only where its notes or pages record it.
- Every item links the client fact it comes from, so you can check the claim before building on it.
- Status words are the Client tab's (**confirmed**, *inferred*, **live**, **open**) plus the server's own, always as **ref** with a link.

### Using this page

- **If you write a server:** start with [What a server must do for this client](server-checklist.md), then
  [Opportunities the client already supports](opportunities.md).
- **If you patch or mod the client:** the opportunities page shows which client patches a server would pair with
  (see also [Region features](../region-features.md)).
- **To check a claim yourself:** follow the "client fact" link on any item to the Client tab page and its addresses, and use
  [Tools and method](../tools-and-method.md).

## How to read this section

| Page | What it holds |
|---|---|
| [What a server must do for this client](server-checklist.md) | Everything that tells a server what to send, check, refuse or build, one section per Client tab page. Each item links its client fact and gives Shaiya Afterglow's status where it is recorded. |
| [Opportunities the client already supports](opportunities.md) | What a server can add with no client change, the extraction answer, the region-feature patches a server would pair with, and open questions for a server. |
| [Client findings: acted on and open](client-findings-status.md) | What the client side found, what Shaiya Afterglow built from it, what is still open, and the HP limit above 65,535. |

## Status words

| Word | Meaning |
|---|---|
| **confirmed**, **confirmed (disasm)**, **confirmed (file)** | Read in a decompile, in disassembly or data bytes, or in a file's bytes or headers. |
| **ref** | Taken from a page of this site or a named public source, with the link. |
| *inferred* | Follows from what was read, not stated by the code. |
| **live** | Needs a game session. |
| **open** | Answerable from the binary, not done. |
| **built** | Shaiya Afterglow implements it. Always **ref**, with a link to the server page that states it. |
| **not built** | Shaiya Afterglow does not implement it. **ref**, with a link. |
| **tested in play** | Checked in a game session on Shaiya Afterglow. **ref**, with a link. |
| **waiting for a test in play** | Built, and the check in a game session is still to be done. **ref**, with a link. |

Where a page says "Afterglow: not recorded", the server's notes do not say either way; that is not a statement that it is missing.

## What moved here

Each row is a section that held advice for a server on a Client tab page. The facts stayed on the client page; the advice moved to the
target. The checklist has one section per client page, named like the page in the Client tab's menu.

| Source page | Section | New place |
|---|---|---|
| [Every opcode against the original server](../packet-opportunities.md) | Opportunities without a client change | [Opportunities](opportunities.md#opportunities) |
| [Every opcode against the original server](../packet-opportunities.md) | The extraction answer (the advice) | [The extraction answer](opportunities.md#the-extraction-answer) |
| [Every opcode against the original server](../packet-opportunities.md) | Region features: what a client patch would unlock | [Region features](../region-features.md) (the whole text); the lines about a server are in [Region-feature patches a server can pair with](opportunities.md#region-feature-patches-a-server-can-pair-with) |
| [Every opcode against the original server](../packet-opportunities.md) | Open questions (server items) | [Open questions for a server](opportunities.md#open-questions-for-a-server) |
| [Client facts for server developers](../CLIENT-FACTS.md) | The "For a server" column of the comparison table | [Client facts](server-checklist.md#client-facts) |
| [Client facts for server developers](../CLIENT-FACTS.md) | Corrections for a server, System notes, Answers to server questions (advice sentences) | [Client facts](server-checklist.md#client-facts); the section is now "Facts that correct earlier assumptions" |
| [Client facts for server developers](../CLIENT-FACTS.md) | Watch: HP, MP and SP above 65,535 (bullets 2 to 4) | [HP, MP and SP above 65,535](client-findings-status.md#hp-mp-and-sp-above-65535) |
| [Packet handling](../client-packets.md) | Findings for a server | [Packet handling](server-checklist.md#packet-handling) |
| [Packet handling](../client-packets.md) | Chat markup crash (`<<NNNcCC`): symptom, cause and the check | [Chat markup crash](../client-packets.md#chat-markup-crash-nnnccc) (stays on the client page, written once) |
| [Social packets](../client-social-packets.md) | What the server must send (three sections) | [Social packets](server-checklist.md#social-packets); the client page keeps "What the original server sends, in order" |
| [Union parties](../union-parties.md) | Advice sentences in the summary and packet order | [Union parties](server-checklist.md#union-parties) |
| [Auction house](../auction-house.md) | For a server | [Auction house](server-checklist.md#auction-house) |
| [Personal shop](../personal-shop.md) | For a server | [Personal shop](server-checklist.md#personal-shop) |
| [Duels](../duels.md) | For a server | [Duels](server-checklist.md#duels) |
| [Guild wars](../guild-wars.md) | For a server | [Guild wars](server-checklist.md#guild-wars) |
| [Obelisks and roads](../obelisks.md) | For a server | [Obelisks and roads](server-checklist.md#obelisks-and-roads) |
| [Item mall](../item-mall.md) | For a server: a minimal mall | [Item mall](server-checklist.md#item-mall) |
| [Scripted monsters](../scripted-monsters.md) | For a server | [Scripted monsters](server-checklist.md#scripted-monsters) |
| [Operator chat copies (F502)](../operator-listen.md) | For a server | [Operator chat copies (F502)](server-checklist.md#operator-chat-copies-f502) |
| [Item.SData against the server](../item-sdata-vs-defs.md) | For a server | [Item.SData against the server](server-checklist.md#itemsdata-against-the-server) |
| [NpcQuest.SData against the server](../npcquest-sdata-vs-world.md) | For a server | [NpcQuest.SData against the server](server-checklist.md#npcquestsdata-against-the-server) |
| [Skill and Monster SData](../skill-monster-sdata-vs-defs.md) | For a server | [Skill and Monster SData](server-checklist.md#skill-and-monster-sdata) |
| [Generating the client data files](../sdata-generation.md) | Server-specific to-dos | [Generating the client data files](server-checklist.md#generating-the-client-data-files) |
| [The Priest packets 0x0F01 / 0x0F02](../research-priest-packets.md) | For a server | [The Priest packets 0x0F01 / 0x0F02](server-checklist.md#the-priest-packets-0x0f01-0x0f02) |
| [Extension features](../research-extension-features.md) | What a server can drive in the UI | [Extension features](server-checklist.md#extension-features); the client page keeps the facts as "What the client shows when a server sends it" |
| [Quality-of-life features](../qol-feasibility.md) | Suggested order | [Quality-of-life features](server-checklist.md#quality-of-life-features) |
| the server's own notes on the client | Server side: acted on; Open for the server | [Client findings: acted on and open](client-findings-status.md) |
