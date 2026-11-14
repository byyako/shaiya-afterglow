# Words used on these pages

This page explains the words that come back on many pages of the client notes. Each entry is one to three sentences, with a link to the
page that explains the word best. The notes describe the US `game.exe` (build of 2010-04-26, patch level 108) unless a page says otherwise.

## In short

- **The pages use a small set of words again and again;** this is the one place that defines them.
- **Terms about the network** are opcode, packet, handler and dispatcher.
- **Terms about the files** are archive, SData, sysmsg, file offset and VA.
- **Terms about how sure a finding is** are confirmed, confirmed (file), inferred, live, open, ref, unverified and decompile or disassembly.
- **Terms about versions** are episode, patch level and server pack.

### Using this page

- **If you are new here:** read the entries once, then follow the link of any word you meet on another page.
- **To check a claim yourself:** start with [Decompile and disassembly](#decompile-and-disassembly), [VA](#va) and
  [File offset](#file-offset), then open [tools-and-method.md](tools-and-method.md).

## Archive

The client keeps nearly all its files in two big files: `data.sah` (the index) and `data.saf` (the contents laid end to end). A file is
found by name through the index. Layout and lookup: [data-archive.md](data-archive.md).

## Confirmed

A finding is **confirmed** when it was read in the code of the named binary, with the function and address given. Weaker words
(*inferred*, **live**, **open**) are listed on [afterglow/index.md](afterglow/index.md#status-words).

## Confirmed (file)

**Confirmed (file)** is the same word for a fact read from a file's bytes or headers (size, link date, imported libraries) and not from
code. It says what the file contains, not what it does. See [afterglow/index.md](afterglow/index.md#status-words).

## Decompile and disassembly

A disassembly lists the machine instructions of a function. A decompile is a C-like text that a tool (here Ghidra) rebuilds from them;
it is easier to read but can be wrong, so important points are checked against the instructions. See
[tools-and-method.md](tools-and-method.md#disassemblers-and-decompilers).

## Dispatcher

The dispatcher is the function that reads the opcode of an incoming packet and passes it to the right handler. In this client it is a
large switch with 393 cases. See [re-network.md](re-network.md).

## Episode

An episode is a major release of Shaiya (EP1 to EP8 and later), each with new maps, systems and a new client. The notes are about
episode 4.5; the other episodes are compared in [episodes/index.md](episodes/index.md).

## File offset

A file offset is a position counted in bytes from the start of a file on disk. It is not the same as a [VA](#va): the two differ by a
fixed amount per section of the program. Both appear in [formats.md](formats.md) and [tools-and-method.md](tools-and-method.md).

## Handler

A handler is the client function that deals with one kind of packet: it reads the fields and updates the game state or the screen.
Handlers are listed per opcode in [client-packets.md](client-packets.md).

## Inferred

*Inferred* means a conclusion that follows from what was read but that the code does not state outright. It is weaker than
[confirmed](#confirmed); the open ones are triaged in [re-open-items.md](re-open-items.md).

## Live

**Live** marks a fact that can only be learned with a running game, so static reading cannot settle it. The list is in
[re-open-items.md](re-open-items.md#needs-the-client-running-live).

## Opcode

An opcode is the 2-byte number at the start of a packet that says what the packet is, for example `0x0802`. The client's table of them
is [client-packets.md](client-packets.md).

## Open

**Open** marks a question that the binary can answer but that has not been traced yet. The list is in
[re-open-items.md](re-open-items.md).

## Packet

A packet is one message between the client and a server, made of an opcode followed by fields. Layouts are on
[client-packets.md](client-packets.md), [client-world-packets.md](client-world-packets.md) and
[client-social-packets.md](client-social-packets.md).

## Patch level

The patch level is the number of the last official patch applied to a client; this client is at 108. The patches and what they changed
are listed in the patch-level list of [episodes/us-patches.md](episodes/us-patches.md).

## Ref

**Ref** marks a statement taken from another page of this site or from a named public source, with the link beside it, and not checked
in the client here. See [afterglow/index.md](afterglow/index.md#status-words).

## Region feature

A region feature is a client behaviour that is switched on or off by the client's region setting (for example the US build against the
Korean or other builds). The list, with the state for region 6, is in [region-features.md](region-features.md).

## SData

SData files are the client's data tables (items, skills, monsters, NPCs and quests, and more). They are stored encrypted in the
archive, and the server keeps its own copy of the same data. See [data-archive.md](data-archive.md) and
[sdata-generation.md](sdata-generation.md).

## Server pack

A server pack is a bundle of server programs and databases for one episode, as shared in the community. Some are studied for comparison;
which ones is shown in [episodes/index.md](episodes/index.md).

## Sysmsg

The sysmsg table holds the numbered system messages the client shows (errors, notices, chat lines), which a server selects by id.
The English pass over it is described in [english-text.md](english-text.md).

## Unverified

*Unverified* means a claim that rests on one source that cannot be checked here, for example the "EP4" label a public collection puts on
a file. See [episodes/index.md](episodes/index.md).

## VA

A VA (virtual address) is a position in `game.exe` as it sits in memory when it runs, for example `0x582e10`. The pages give VAs so you
can jump to the same place in a disassembler; the matching [file offset](#file-offset) is a different number.
