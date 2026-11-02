# Shaiya internals

Notes from reverse engineering the Shaiya Episode 4.5 server programs (`ps_login.exe`, `ps_game.exe` and their neighbours) and the US client `game.exe`: how they talk to
the client and to each other, what each packet carries, and what the game rules actually are, with the addresses and structures they were found at.

The notes come from static analysis of the shipped binaries and their debug symbols. Where a point has been checked against the real client it
says so; where it is an inference it says that too.

## Where to start

- **How the client and server talk:** [Protocol notes](original/protocol-notes.md), then the login server ([`ps_login`](original/ps_login.md)) and the game
  server ([`ps_game`](original/ps_game.md)): the handshake, the encryption and the character screen.
- **A game system:** use the tabs above, or the search box. Each page covers one system from the packets in, through the checks the original
  server makes, to the packets out.
- **The client side:** the [game.exe pages](client.md): how the client talks to the servers, its engine and game state, every packet it handles, the data formats, and a patch kit for your own `game.exe` (US build, patch level 108).
- **Running a server:** the [server pages](server.md): NewShaiya, the open-source server built from these notes; how to run your own,
  its architecture, every setting, and every place it differs from the original.
- **Bugs in the original:** [Known bugs and fixes](original/community-fixes.md), and a [patch kit](server-patches.md) that fixes many of them in
  the EP4.5 `ps_game.exe`.

## Reading the pages

- Packet opcodes are written as `0x0702`; the first byte pair is the family (`0x07xx` is shops).
- Addresses such as `CUser::PacketShop 0x46b55a` are in the EP4.5 `ps_game.exe` the notes were built from.
- Every page starts with **In short**, a plain-language summary of the system, then the technical detail.
- "Confirmed" means read in the binary; "inferred" means it follows from what was read but was not checked directly.

!!! note "A player guide is planned"
    A plain-language guide to the game's mechanics will be added next to this reference.

!!! note "The open-source server"
    A server built from this research is open source: [shaiya-afterglow on GitHub](https://github.com/byyako/shaiya-afterglow) (Rust and PostgreSQL,
    MIT license). You bring your own copy of the original server files. The [server pages](server.md) describe it.

## In memory

!!! quote "In memory of Michael (SafeBett)"
    Michael is the reason I started learning to program, and he was part of the Shaiya community that this work comes from. His
    persistence and kindness kept me going. Life got busy and I could not always be there, but he never stopped checking in on me or
    caring how I was, and I tried to help him whenever I got the chance. This work is for him.

    So long, mate. See you on the other side.

## About this site

Independent research. No game files, client or server code, or data tables are distributed. The downloads are two patch kits: scripts and the lists of bytes they change, applied to your own copy of `ps_game.exe` or `game.exe`. Shaiya and its trademarks belong to their owners.
