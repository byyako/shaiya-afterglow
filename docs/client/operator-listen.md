# Operator chat copies (`0xF502`)

This page describes a packet that does not exist in the original game: `0xF502`, which lets a server show an operator a copy of other
players' chat. The stock client drops it; the companion DLL (`fixes-dll.md`) adds a handler. It describes the US `game.exe` (build
2010-04-26, patch 108). Status words: **confirmed** (read in the binary, with address), **confirmed (disasm)**, [*inferred*](glossary.md#inferred), [**live**](glossary.md#live).

## In short

- **What it is:** a new server-to-client packet that carries a sender name and one line of text. The client prints it in the operator
  chat list as `[Listen] Name: [a,b]text`.
- **The client only displays it.** It sends and echoes nothing; the server alone decides who may listen and which operator gets
  copies.
- **An unmodified client silently drops it**, so sending it to a stock client is harmless.
- **The line is made safe before it is shown:** `<` becomes `[` and control bytes become spaces, so a name or message cannot feed the
  old renderer's markup parser.
- **Not yet seen in game:** colour, tabs, wrapping and an end-to-end watch command are *live* checks.

### Using this page

- **If you write a server:** the wire contract gives the packet layout; the checklist is [What a server must do for this client](afterglow/server-checklist.md#operator-chat-copies-f502).
- **If you use a DLL like this:** the handler is on by default (`listen_chat` in its settings file, see [the companion DLL](fixes-dll.md#settings)); a received copy appears in the operator chat list as `[Listen] Name: [a,b]text`.
- **To check a claim yourself:** "How the client receives it" has the addresses.

## Wire contract

Sizes include the opcode and exclude the transport's length prefix. Little endian:

| Packet offset | Field |
|---|---|
| +0 | `u16 0xF502` |
| +2 | `char name[21]`, sender's character name, normally NUL-padded |
| +23 | `u8 len`, exact number of following bytes |
| +24 | `u8 text[len]`, normally `[a,b]` followed by the copied line |

The packet size is exactly **24 + len** (len 0..255). A zero terminator may be included in `len` but is not required. The display
works on a counted final NUL and at most 255 bytes of prefix, text and NUL together. No extra bytes are accepted. Empty text, or a
`len`/packet-size mismatch, is consumed without displaying a line. Names are bounded at 21 bytes; text stops at its first zero.

The layout follows the original server's listen-chat sender (`ps_game.exe 0x4702f0`). The meaning of `a` and `b` in the prefix is
read on the server side: `a` is the kind of the line and `b` a whisper's target (**ref** [Listening, `FA0B` and `FA0C`](../original/gm-commands.md#84-listening-fa0b-fa0c)).
The client shows the prefix verbatim, so the original numbers work without any client change.

## Display

`[Listen] Name: [a,b]text` in the existing GM chat category `0x10`, in the chat log's GM/system list, with no centre-screen notice.
For literal display every `<` becomes `[`, ASCII control bytes become spaces, other bytes are kept, and the line is at most 255 bytes
plus a NUL (an overlong line ends with `...`). This protects the old renderer's unchecked `<<NNNcCC` colour parser, including markup in
a name. The server's wire text is not rewritten by the handler.

## How the client receives it (confirmed)

- The dispatcher `0x582e10` has no `0xF502` case and sends it to the network vtable's `+0x600` slot (`0x6a6220`).
- That slot holds `0x551030`, which handles `0x0114` and forwards `0x1Cxx`/`0x1Dxx`. The DLL replaces the slot only when its four bytes
  match, handles `0xF502`, and forwards every other opcode to the original method with the original `thiscall` arguments.
- Packet object constructor `0x589c90`: 8192 data bytes, size at `+0x2000`, read pointer `+0x2004`, write pointer `+0x2008`. The
  receive loop (`0x585280` / `0x585390`) consumes the two-byte opcode before calling `0x582e10`. The handler validates size and cursor
  before forming the remaining slice, and consumes the recognised packet even if it is malformed.
- Chat renderer `0x41e2d0`, `thiscall(chat-object, category, char*)`; the chat object pointer is at `0x7013d8` (also loaded by
  `0x41e91c`). Existing GM chat at `0x554d1b` uses category `0x10`. The renderer adds centre-screen entries only for categories
  8/18/1B/1C/1D/1E/20, not `0x10`. `0x41ddc0` stores chat lines in 260-byte entries, hence the formatter's smaller bound.

The setting is `[fixes] listen_chat` (default on); with 0 the original fallback is kept.

## Checks that were run

- Six standalone parser tests: NUL conventions, short headers and length mismatch, bounded names, markup and controls, maximum-length
  truncation, ANSI bytes.
- The DLL's full unit test set passed offline; this is test evidence, not an in-game test.
- **confirmed (disasm)** on the built DLL: i386 PE; the `0xF502` branch; non-`0xF502` opcodes tail-jump to `0x551030`; the display
  path loads `0x7013d8`, pushes the text and `0x10` and calls `0x41e2d0`; the hook's return cleans its two stack arguments.

## For server developers

What a server should do with these facts: [What a server must do for this client](afterglow/server-checklist.md#operator-chat-copies-f502).

## Open

**Live:** a watch session across ordinary, whisper, party and guild chat; the literal prefix, name and line; placement in the GM
chat; that no more copies arrive after the watch stops; and that auctions, mail and server time keep working through the forwarded
fallback.
