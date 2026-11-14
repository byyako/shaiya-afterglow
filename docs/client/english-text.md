# The English text pass

This page describes how the US client's system-message table, `data/sysmsg-uni.txt`, was proofread and rewritten into plain English without
breaking the client's placeholder system: the file format, the tags and `%` signs that must survive, the checks that enforce it, what was
deliberately left alone, and a few before-and-after examples. It describes the 108 client (build 2010-04-26). The full table of changes is
game text and is not published. Status words: **confirmed** (read in the binary or checked by the tool), *inferred*.

## In short

- **790 of the 1,386 messages in the [sysmsg](glossary.md#sysmsg) table were changed;** the rest were already fine or must not change.
- **Each message is a line `id<TAB><TAB>"text"`** in a UTF-16LE file with a byte-order mark and CRLF line ends.
- **The client fills in placeholders** (`<p>`, `<t>`, `<v>`, `<b>`, `<i>`, …), colours (`{c3}…{/c}`) and passes the text through `printf`,
  so a rewrite must keep exactly the same tags and the same number of `%` signs; a tool refuses any line that does not.
- **Left alone on purpose:** slash-command words the client matches, labels it joins to a value, chat formats, texts never shown,
  monster shouts, and map and NPC names.
- **The change is a data file:** it ships through the updater (`updater.md`), not through `game.exe` or the DLL.

### Using this page

- **If you patch or mod the client:** edit the text file only with the checking tool of the client repository, which refuses a line whose tags or `%` signs changed; ship the file through [the updater](updater.md).
- **If you write a server:** a server picks messages by id, so the ids and their placeholders are the contract; the words can change freely.
- **To check a claim yourself:** the loader is at `0x41df80`; see [tools-and-method.md](tools-and-method.md).

## The file and the placeholders

`sysmsg-uni.txt` is loaded once at start-up (`0x41df80`): the two-byte byte-order mark is skipped and the file is read as UTF-16 lines of up
to 2048 characters, each converted to the region's ANSI code page (`re-engine.md`). A line is an id, whitespace and the text in double quotes.
The text can contain:

- **fill-in tags** the client replaces from fixed buffers: `<p>` (a player), `<t>` (a target), `<v>`, `<b>`, `<i>` (an item), `<r>`, `<s>`,
  `<hp>`, `<zz>` and others, depending on the message;
- **colour spans** `{c3}…{/c}`;
- **`printf` conversions:** the text goes through a format call, so a literal percent sign is written `%%`, and for texts that pass twice
  `%%%%`;
- **leading and trailing spaces** the client relies on where it joins a text to a value.

A missing id prints "GetMsg Error <id>"; 1,386 ids are used, the highest 10123.

## Method

A tab-separated table (`id<TAB>new text`) holds only the changed messages. A small tool applies it to the original file and:

- **refuses a replacement whose tags, colour spans or number of `%` signs differ** from the line it replaces;
- keeps the original's leading and trailing spaces and its line layout, so untouched lines stay byte-identical;
- writes a review table (before and after, per id) for proofreading;
- has a `check` mode that finds broken lines in any file.

Edits are made to the table, never to the generated file.

## What was left alone

- **Slash-command words** the client matches against typed text (for example ids 5076–5081, 5114–5123, 5335, 6543, 6605–6608).
- **Labels the client joins to a value,** where the original spaces matter.
- **Lines with the particle tag `<is>`** (6515, 6534).
- **Chat formats** (353–357, 370, 481, 482, 6606): the client builds chat lines from them.
- **GameGuard and billing texts,** which are never shown.
- **Monster shouts, map names and NPC names.**

Several messages are not in this file at all: the GM question window's labels and the Priest window's buttons are Korean literals inside
`game.exe`, handled by the DLL (`fixes-dll.md`).

## Examples

| Before | After |
|---|---|
| `<p>got <v> damage from a trap` | `<p> took <v> damage from a trap` |
| `<p> is killed by <t>` | `<p> was killed by <t>` |
| `Leveled up to Lv<v>` | `Reached level <v>` |
| `Recharge must be completed before next use` | `Wait for the recharge to finish before using it again` |
| `Cannot use the item due to its limitation` | `You do not meet this item's requirements` |
| `<p> requested a transaction` | `<p> wants to trade with you` |
| `Transaction has been completed successfully` | `Trade completed` |
| `<p> requested battle` | `<p> challenged you to a duel` |
| `Cannot trade the quest items` | `Quest items cannot be traded` |
| `<b> got 'perfect party' bonus EXP.` | `Gained <b> bonus EXP for a perfect party.` |

## Checks and limits

The tool validates every replaced line against its original; the whole table applies cleanly. A message edit is not tested in play line
by line: whether each message appears where the new wording fits is a **live** question, and a message whose original was ambiguous about
who the actor is (for example 204 "`<p>` invited") was rewritten from the call sites that fill the tags, where those were read.
