# How game.exe talks to the servers

A walkthrough of the client's network layer in the order the code runs it: from the first `socket()` call to a packet showing up on screen.
Everything here was read statically in the Shaiya US client (`game.exe`, 2010-04-26, patch 108, SHA-256
`5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`). Nothing was run. Addresses are virtual addresses in that file.
"confirmed" means the function was read and the behaviour follows from its code; "inferred" means a reasonable reading that the code
does not state by itself.

## In short

- The client opens **two TCP connections one after the other**, never both at once: first to the **login server (port 30800)**, then,
  after you pick a server, to the **game server (port 30810)**. Both go through the same few functions and the same global cipher state.
- The socket is a plain blocking-connect Winsock socket driven by `WSAAsyncSelect`: a hidden window receives "data arrived" messages,
  so all receiving happens on the main thread inside the Windows message loop. There is no network thread.
- A packet on the wire is `u16 length` (counts itself) + `u16 opcode` + payload, little-endian. The length field is never encrypted.
- Right after connecting to the login server, the server sends its public RSA key (`0xA101`). The client invents a random 128-byte secret,
  RSA-encrypts it with that key and sends it back. Both sides derive the **AES-128 key and counter** from it with HMAC-SHA256.
- The cipher is **AES-128 in counter mode**, one continuous keystream per direction (not reset per packet). The game connection reuses
  the key and re-derives the counter from the login IV with one SHA-256.
- After the character is chosen, the **server-to-client direction switches to a second layer: an XOR table** (4096 bytes built by chaining
  SHA-256). The client never XORs what it sends.
- Receiving: accumulate bytes until a whole packet is there, copy it into a stream object, decrypt it, read the opcode, and call the one big
  dispatch function (`0x582e10`) that fans out to ~390 handlers. Handlers read fields one at a time; reading past the end zero-fills and logs
  "Packet Underflow".
- Sending: every sender builds `opcode + payload` and calls `0x585640`, which adds the length, encrypts and calls `send()`. If Winsock says
  "would block", the bytes go into a 16 KiB queue that is flushed on the next `FD_WRITE`.
- If the connection drops, the client shows a message box and **quits**. There is no automatic reconnect. The only "reconnect" is a
  server-ordered hop to another game server (`0x0111`).
- A keep-alive `0x0003` goes out every 2 minutes. There are no other timeouts on an open connection; the only timeouts are 30-second waits
  while connecting.

Related pages: [client-packets.md](client-packets.md) (receive and send tables, packet log), [client-packets.generated.md](client-packets.generated.md)
(every opcode's layout), [client-world-packets.md](client-world-packets.md) (what the world packets do).

## 1. The pieces and where they live

| What | Address | Notes |
|---|---|---|
| Frame function (called every iteration of the main loop) | `0x42cc10` | called from `WinMain` (`0x40b1a0`) at `0x40bf2a`; also sends the keep-alive (confirmed) |
| Start-up of the network object (`GameInit`) | `0x40a6f0` | connects to the login server, waits for the key packet (confirmed) |
| Network object | constructed by `0x40a4f0`, pointer stored at `0x2223818` and `0x222ea40` | 0x374 bytes; its vtable at `0x6A5C20` has 386 slots (confirmed) |
| Message window set-up | `0x5854b0` | creates a hidden window, subclasses it to `0x585280`, `WSAStartup(2.2)` (confirmed) |
| Connect | `0x578b10` | `socket`, `connect`, `WSAAsyncSelect` (confirmed) |
| Disconnect | `0x578c80` (full reset), `0x578ca0` (arg 6 = keep cipher state) | confirmed |
| Window procedure / receive | `0x585280` | `FD_READ`, `FD_WRITE`, `FD_CLOSE` and errors (confirmed) |
| Send | `0x585640` (`send(buf,len)`), `0x585520` (`send3(opcode,payload,len)`) | confirmed |
| Send-queue flush | `0x578bf0` | confirmed |
| Packet dump (log) | `0x5851b0` | only when bit `0x40` of the log mask `0x2232e14` is set (confirmed) |
| Dispatch | `0x582e10` | one function, one `switch` (confirmed) |
| Stream reads | `0x589d10` `read(dst,n)`, `0x589da0` `read_upto`, `0x589c90` constructor, `0x589cf0` end-of-packet hook | confirmed |
| Underflow report | `0x552c40` (vtable slot `+0x5fc`) | confirmed |
| Cipher front ends | `0x4010a0` encrypt, `0x4010e0` decrypt | confirmed |
| Cipher state switches | `0x401160` all off, `0x4013c0` AES on, `0x401570` XOR on, `0x4015a0` XOR off | confirmed |
| Handshake key derivation | `0x401180` | confirmed |
| Counter step for the game connection | `0x4013d0` | confirmed |
| Re-key | `0x401000` | confirmed |
| XOR table builder | `0x401520` | confirmed |

Global state used by the cipher (all confirmed from the references listed in the functions above):

| Address | Meaning |
|---|---|
| `0x2232a58` | the socket |
| `0x222ea48` | the hidden message window |
| `0x222aa38` / `0x222aa40` | receive accumulation: byte count (16-bit) and buffer |
| `0x2226a38` / `0x2232a60` | send queue: buffer and byte count |
| `0x2237700` / `0x2237701` / `0x2237702` | flags: **XOR layer on** / **encrypt on** / **decrypt (AES) on** |
| `0x22376f8` | cipher type chosen by the server in `0xA101` (0 = AES-CTR) |
| `0x22376fc` | XOR mode chosen by the server in the `0xA301` reply (2 = the XOR table is used) |
| `0x2237708` / `0x2237858` | the two AES contexts (key schedule, counter at `+0xF4`, keystream byte position at `+0x104`, keystream buffer at `+0x108`); counters therefore at `0x22377fc` / `0x223794c` |
| `0x22366d8` | saved copy of the login IV (16 bytes), used once for the game connection |
| `0x22366f8` | base of the 4096-byte XOR table (indexed `base + n + j`, see section 5.3) |
| `0x22379a4` | state of the client's random number generator (section 4) |
| `0x701970` | the login server's address as a text string (16-byte slot, filled from the region setting) |

## 2. Opening the socket

**The window.** `0x5854b0` creates a zero-size child window of class `BUTTON`, replaces its window procedure with `0x585280`
(`SetWindowLong(GWL_WNDPROC)`) and calls `WSAStartup(0x202)`. This hidden window is the target of every `WSAAsyncSelect` call
(confirmed). It is a cheap trick: no window class has to be registered.

**The connect** (`0x578b10`, confirmed):

```c
// params: dotted-decimal address string, port
sock = socket(AF_INET, SOCK_STREAM, 0);             // stored at 0x2232a58
addr = { AF_INET, htons(port), inet_addr(str) };    // dotted decimal only, no DNS
setsockopt(sock, SOL_SOCKET, SO_DONTLINGER, 0);     // option -0x81 = 0xFF7F
if (connect(sock, &addr, 16) == -1) return 2;       // BLOCKING connect
recvCount = 0;                                      // 0x222aa38
if (WSAAsyncSelect(sock, hwnd, 0x2B10, FD_READ|FD_CLOSE) == -1) return 3;   // 0x21
return 0;
```

Points worth noting:

- The connect itself is **blocking**. The window freezes until the server answers or Winsock gives up. `WSAAsyncSelect` is only
  armed afterwards, so there is no `FD_CONNECT` handling anywhere.
- Addresses are parsed with `inet_addr`, so host names do not work. The login address is a text string in the global `0x701970`,
  copied from a per-region table at start-up (see the region handling described in [client-packets.md](client-packets.md) and the login notes).
- The window message is the constant `0x2B10`, an application-chosen id used in all six places that call `WSAAsyncSelect` or test the message.

**Waiting for the server.** After connecting, the caller does not return to the main loop. It spins its own message loop
(`PeekMessage`/`GetMessage`/`DispatchMessage`) so that the window procedure can process incoming data, and polls a field of the network
object (`+0x1EC`) until a handler sets it. Each such wait has a **30-second limit** (`timeGetTime() - start > 30000`) after which a
message box is shown. Four places do this (confirmed): `GameInit` (`0x40a6f0`, waits for the key packet), the faction screen
(`0x47bb10`, same connect-and-wait; its failure texts are the literal `LoginServer Connet Error!!!` and `key Data Recv!!!`),
server selection (`0x4dca00`) and the game-connection step inside it.

## 3. Login connection, then game connection

| Step | Where | What happens |
|---|---|---|
| 1 | `GameInit` `0x40a6f0` | all cipher flags off (`0x401160`); connect to `0x701970:30800` (`0x7850`); wait for `+0x1EC != 0` |
| 2 | server sends `0xA101` | handler `0x57d3c0` → vtable `+0x218` = `0x55c650`: derive keys, **send the reply**, switch AES on, set `+0x1EC = 1` (section 4) |
| 3 | login screen | client sends `0xA102` (name and password, encrypted); reply handler `0x55c6c0` stores result, user id, the 16-byte token at `+0x342` and the privilege byte |
| 4 | server sends `0xA201` | server list (38-byte rows) |
| 5 | player picks a server | `0x4dca00` sends `0xA202` (`u8 server`, `u32 0x10042600` as the client version, a literal in the code) via `0x5868d0` and waits 30 s |
| 6 | accepted | close the login socket and **reset everything** (`0x578c80`), run the counter step `0x4013d0`, connect to the address from the reply on **port 30810** (`0x785A`) |
| 7 | connected | flags off (`0x401160`); **send `0xA301` in plain text** (`0x586910`: opcode, `u32 userId`, the 16-byte token = 22 bytes after the length, 24 on the wire); AES on (`0x4013c0`); wait 30 s for the reply |
| 8 | `0xA301` reply | handler `0x57d670` → `0x55ce50`; when `bySuccess == 0` it builds the XOR table (`0x401520`) and sets `+0x1EC` |
| 9 | game server sends character list etc. | `0x0101`, then after character select the "enter world" burst |
| 10 | `0x010B` (or `0x0113`) received | handler turns the XOR layer on (`0x401570`); every later server packet goes through the XOR table |
| 11 | `0x0107` received (back to the character screen) | `0x4015a0`: XOR off, AES decrypt back on |

All of the above is confirmed from the code. The ports are literals passed to `0x578b10` (`0x7850`, `0x785A`). Login-server connects
are in `0x40a6f0` and `0x47bb10`; game-server connects are in `0x4dca00` and in the main loop's state 6 (section 8).

## 4. The handshake and key exchange (`0xA101`)

The server's first packet carries a public RSA key. The wire layout (verified by the receive side, `0x57d3c0`):

```
u16 length  u16 0xA101
u8  mode            // goes to 0x22376f8 (cipher type; the reference server sends 0)
u8  exponentLen     // 64
u8  modulusLen      // 128
u8  exponent[64]
u8  modulus[128]    // total 197 bytes after the length field
```

The handler calls vtable slot `+0x218`, which is `0x55c650`:

```c
void on_A101(mode, expLen, modLen, exponent, modulus) {      // 0x55c650
    key128(out, &outLen, mode, exponent, expLen, modulus, modLen);   // 0x401180
    send_A101_reply(outLen, out);                                    // 0x586770: opcode, u8 len, 128 bytes
    enable_aes();                                                    // 0x4013c0: enc/dec flags on
    this->state = 1;                                                 // +0x1EC
}
```

`0x401180` does the work (confirmed from decompilation and disassembly):

```c
cipherType = mode;                                  // 0x22376f8
secret[128] = lcg_bytes(128);                       // 0x4046a0
secret[127] &= 0x7F;                                // make it smaller than the modulus
digest[32]  = HMAC_SHA256(key = secret[128],        // 0x4044a0 (key > 64 bytes is hashed first)
                          msg = modulus[modLen]);
aesKey = digest[0..16];  iv = digest[16..32];
if (mode == 0) {                                    // AES-CTR
    key_schedule(encCtx /*0x2237708*/, aesKey);     // 0x404770, 128-bit
    key_schedule(decCtx /*0x2237858*/, aesKey);
    savedIv /*0x22366d8*/ = iv;
    counter_enc /*0x22377fc*/ = counter_dec /*0x223794c*/ = iv;   // positions 0
} else { /* alternative stream cipher, see 5.2 */ }
reply = RSA_public(secret, exponent, modulus);      // big-number code at 0x4053e0 / 0x5718e0
```

- The **HMAC** is standard: ipad `0x36`, opad `0x5C`, 64-byte block, SHA-256 initial values `6a09e667 bb67ae85 …` (confirmed:
  the constants are in `0x4044a0`, and the key-over-64-bytes branch hashes the key first). The assignment "key = the secret, message = the
  server's modulus" follows the argument registers at `0x4011ee`–`0x4011f7` (`ECX = 0x80` is the key length) and agrees with the
  server-side analysis of the same handshake.
- The **secret is not strongly random**: `0x4046a0` is the Microsoft C `rand()` recurrence (`x = x*0x343FD + 0x269EC3`, 31 bits,
  each output built from `x / 0xFFFF`), and its state `0x22379a4` is set to `time(NULL)` by a start-up routine at `0x69ec30` (confirmed).
  So the key material has about as much entropy as the clock second. Worth knowing when you study the protocol; it is a property of the
  2010 design.
- The **RSA step** is plain textbook RSA with no padding. The exact modular exponentiation inside `0x4053e0` goes through a GMP-derived
  big-number library (`0x5714e0` init, `0x571720` free, `0x5718e0` operation); the operation's identity is *inferred* from its shape
  (init two numbers, load exponent and modulus, load the secret, call, copy out) and from the protocol, not from reading the
  library routine. The result is copied out as raw 32-bit words (little-endian integer), and the byte count of the result goes in front
  of it: `0x586770` always copies 128 bytes but sends the real length byte.
- The reply is sent **before** the encrypt flag is switched on, so it goes out in plain text (the flags were cleared by `0x401160`
  in `GameInit`; `0x585640` encrypts only if the flag `0x2237701` is set). From the next packet on, both directions are AES.

## 5. The cipher layers

### 5.1 The flags decide everything

```c
void encrypt(buf, n) {                       // 0x4010a0 (buf and context passed in registers)
    if (!encFlag /*0x2237701*/) return;      // off: send in clear
    if (cipherType == 0) aes_ctr(encCtx, buf, n);   // 0x4051b0
    else                 lcg_stream(buf, n);        // 0x401700
}
void decrypt(buf, n) {                       // 0x4010e0
    if (xorFlag /*0x2237700*/) {
        if (xorMode /*0x22376fc*/ == 2)
            for (j = 0; j < n; j++) buf[j] ^= xorTable[n + j];   // table base 0x22366f8
        // any other mode with the XOR flag on: the packet is left untouched
    } else if (decFlag /*0x2237702*/) {
        if (cipherType == 0) aes_ctr(decCtx, buf, n); else lcg_stream(buf, n);
    }
}
```

All confirmed from the two functions. The send side has **no XOR branch at all**: only the server-to-client direction gets the table.

The four switches:

| Function | Effect |
|---|---|
| `0x401160` | encrypt, decrypt and XOR all off (start of every connection, start of the `0xA301` plain-text step) |
| `0x4013c0` | encrypt and decrypt on |
| `0x401570` | XOR on; for XOR modes 0 or 1 it instead turns AES decrypt *off*; mode 3 does nothing |
| `0x4015a0` | XOR off; AES decrypt on unless the XOR mode is 0 |

### 5.2 AES in counter mode

`0x4051b0` (confirmed): the context keeps a 16-byte counter (at `+0xF4`), a 16-byte keystream block (`+0x108`) and a byte position
(`+0x104`). Leftover keystream is used first; then whole blocks (`0x404c80` encrypts the counter into the keystream block, XOR with the data);
then a trailing partial block generates one more keystream block and leaves the position in its middle. **The keystream continues across
packets**: there is no per-packet reset, so one lost or extra byte desynchronises the connection for good. The counter increment adds 1 to
the first 32-bit word and carries upward through the four words (little-endian, not the usual big-endian CTR). The key schedule
(`0x404770`, with S-box table at `0x6e1e08` and round constants at `0x6e360c`) is standard AES-128.

**The alternative stream cipher** (`cipherType != 0`, confirmed in code, never seen on the wire): `0x401700` XORs the data with a keystream
whose 16-byte blocks come from `0x4015c0`: four values, each built from two steps of the same `x*0x343FD + 0x269EC3` recurrence over an
8-word state seeded from the 32-byte HMAC digest. Whether any real server ever selected this is not known (inferred: a leftover for other
regional builds). The reference server sends mode 0.

### 5.3 The game connection's counters, and re-keying

- **Counter step `0x4013d0`** (run between the login socket closing and the game socket opening): both counters become the first 16 bytes of
  `SHA-256(savedIv)`, positions 0, key unchanged, then the saved IV is zeroed (confirmed shape; the SHA-256 call `0x404410` hashes 16 bytes).
  So the game connection uses the *same AES key* as the login connection with a new counter derived from the old IV.
- **Re-key `0x401000`**, called by the handler of `0x0111` (`0x579830`: `u32 address`, `bytes[32]`): key schedule from the first 16 bytes,
  both counters from the last 16 bytes, both flags on. See section 8 for the server-ordered hop it belongs to.
- **XOR table `0x401520(mode, seed16)`** (the `0xA301` reply, when `bySuccess == 0`): the table is built by 128 rounds of SHA-256 over 16 bytes,
  each 32-byte output appended; after the first (seeded from the 16 bytes in the reply), each round hashes the **last 16 bytes of the
  previous round's output** (the loop advances 0x20 and hashes at offset −0x10; confirmed). 128 × 32 = 4096 bytes. The decrypt loop indexes it
  as `table[n + j]` where `n` is the number of bytes after the length field (opcode included) and `j` the byte index, so the same
  four-kilobyte table is used with a different starting point per packet length. Mode 0 also turns the AES decrypt flag off.

## 6. Packet framing

```
 offset  size  field
 0       2     total length, little-endian, counts these two bytes (never encrypted)
 2       2     opcode      (encrypted)
 4       n     payload     (encrypted)
```

- **Sending** (`0x585640(buf, len)`, `buf` starts at the opcode, `len` = opcode + payload): copy `buf` behind a 2-byte slot, set the slot to
  `len + 2`, encrypt `len` bytes after it, `send()`. The packet dump (`SEND>>`) is written *before* encryption, so logs show the plain packet.
  `0x585520(opcode, payload, n)` is the same but writes the opcode itself; only the auction house uses it (confirmed).
- The sender's stack buffer holds at most 4090 payload bytes (`ushort local[2045]`), the receive side at most 8192 (`0x2000`). No sender
  checks it; the large packets (`0x0101` character list, `0x1C..` auction rows) stay far below.
- **Receiving** keeps the length field out of the stream: the object handed to the handlers starts at the opcode.

There is no padding, no checksum and no sequence number at this layer (confirmed: nothing in `0x585280` or `0x585640` adds one).

## 7. Receiving: from `recv()` to the handler

`0x585280` is the window procedure (confirmed):

```c
LRESULT wndproc(hwnd, msg, sock, lParam) {
    if (msg != 0x2B10) return DefWindowProc(...);
    event = LOWORD(lParam); error = HIWORD(lParam);
    if (error) { net->vt[+0x08](WSAGetLastError()); closesocket(sock); return; }
    if (event == FD_READ /*1*/) {
        n = recv(sock, tmp /*8192*/, 0x2000, 0);
        if (n == -1) { closesocket(sock); return; }
        memcpy(recvBuf /*0x222aa40*/ + recvCount /*0x222aa38*/, tmp, n); recvCount += n;
        while (recvCount >= 4 && u16(recvBuf) <= recvCount) {      // a whole packet is there
            len = u16(recvBuf);
            Stream s(recvBuf + 2, len - 2);       // 0x589c90: zero 0x2000 bytes, copy, cap at 0x2000
            decrypt(s.data, len - 2);             // 0x4010e0
            memmove(recvBuf, recvBuf + len, recvCount - len); recvCount -= len;
            s.read(&opcode, 2);                   // 0x589d10
            packet_dump("RECV>>", opcode, s.data, s.len);            // 0x5851b0
            dispatch(opcode, &s);                 // 0x582e10
            underflow_reset();                    // 0x589cf0
        }
    } else if (event == FD_WRITE /*2*/) flush_send_queue();          // 0x578bf0
    else if (event == FD_CLOSE /*0x20*/) net->vt[+0x0C]();           // connection lost
}
```

What this means in practice:

- **TCP reassembly** is the `while` loop: bytes are appended to the accumulation buffer, and as long as a complete packet (the declared length
  fits in what is buffered) is available it is processed. The tail of a partial packet is moved to the front and waits for the next `FD_READ`.
- **One packet = one decrypt = one dispatch.** Decryption happens per packet just before dispatch, so the cipher state advances exactly
  once per packet, in order. The server's XOR switch ("after `0x010B`") works because of this.
- **Sanity checks are minimal.** The only test is `length <= buffered`. There is no minimum-length check visible in this function (a declared
  length below 4 would be copied as a stream of `length − 2` bytes, and a value of 0 or 1 would wrap), and none for a length above the
  buffer's capacity (inferred: it is the server's job to send well-formed lengths). A server implementation should never send a length under 4.
- **Underflow.** `0x589d10` (`read`) first checks that `n` bytes are left. If not, it zero-fills the destination, calls vtable slot `+0x5FC`
  (`0x552c40`, `CNetWork::PacketError`) and returns what is left. The report is `Packet Underflow. Protocol : 0x%04X Size : %d Read %d`
  into the log; in error-report mode it also pops the "Shaiya Error Report-Packet Error" window. `0x552c40` remembers the stream it
  reported for in `0x2223820` and stays quiet for repeats; `0x589cf0` (run after every dispatch) calls it with a null stream, which clears
  that memory. So the report appears **once per packet**, not once per opcode.
- **Over-long packets** are not an error: unread bytes are simply dropped when the next packet starts.

### The dispatch function

`0x582e10(opcode, stream)` takes the 16-bit opcode and branches through the compiler's `switch` lowering: a **binary search of compares**
(`CMP EAX,0xd01 / JG …` at the top) ending in jump tables for dense ranges (for example `JMP [EAX*4 + 0x584f28]` for `0x0D02`–`0x0D38`).
Most cases are `push stream; call FUN_xxxxxxxx; add esp,4; ret`: a **reader function** that pulls the fields with `read()` and then calls a
slot of the network object's vtable (`0x6A5C20`) with the values as arguments. Examples:

| Opcode | Reader | Then calls | Notes |
|---|---|---|---|
| `0xA101` | `0x57d3c0` | slot `+0x218` = `0x55c650` | key exchange (section 4) |
| `0xA102` | `0x57d450` | slot `+0x21C` = `0x55c6c0` | login result |
| `0xA301` | `0x57d670` | slot `+0x234` = `0x55ce50` | game-connect reply, builds XOR table |
| `0x0111` | `0x579830` | slot `+0x44` = `0x54c910` | re-key and redirect (section 8) |
| `0x0505` | `0x57ae20` | slot `+0xEC` = `0x54e2c0` | recover HP/MP/SP (section 10) |
| `0x0002` | direct vtable call, slot `+0x10` (`0x54c7e0`) | | disconnect notice: shows sysmsg 10069 (a message box before the world is entered, a white notice line after), resets the main state to 0 and posts `WM_CLOSE` (confirmed, decompiled) |
| `0x0004` | reader `0x578ed0` | slot `+0x14` = `0x54c860` | disconnect notice: `u8` 0 shows sysmsg 10070 (this ID logged in elsewhere), any other value 10071 (system error); same box/notice rule, same reset and `WM_CLOSE` (confirmed, decompiled) |

The opcodes not in the `switch` end up in the **fallback** (vtable `+0x600` = `0x551030`), which handles the server-time packet `0x0114`
(sets a clock offset), the auction range `0x1Cxx` and the mail range `0x1Dxx`, and ignores everything else (confirmed). The generated table
[client-packets.generated.md](client-packets.generated.md) lists reader, layout and handler for every opcode.

Because the vtable is where the real logic hangs, "the handler of opcode X" usually means *two* functions: the reader in the `0x57xxxx`
range (the wire layout) and the logic slot in the `0x54xxxx`/`0x55xxxx` range (what the client does with it).

## 8. The send path, the queue, disconnects and timeouts

**Send queue** (`0x585640`, confirmed). `send()` is called directly on the socket. When it returns -1 and the error is
`WSAEWOULDBLOCK` (`0x2733`) or `997` (`0x3E5`, overlapped I/O pending), the *already encrypted* bytes are appended to the queue at
`0x2226a38`; if the queue passes **16384** bytes the client shows the message box `Send Error!!!` and empties it; otherwise it re-arms
`WSAAsyncSelect` with `FD_WRITE` added (`0x23`). On `FD_WRITE`, `0x578bf0` does one `send()` of the whole queue; on success it zeroes the
queue and goes back to `FD_READ|FD_CLOSE` only (`0x21`); on failure it shows `Send Error!!!----OnWrite()`.

Two consequences, both read from the code but not exercised (inferred): the queue flush does not look at how many bytes `send()` accepted, and
new packets are sent with `send()` first even while the queue is non-empty. In a congested connection that could reorder or lose bytes, which
with a continuous keystream means a permanent desync. In normal play the queue stays empty.

**Connection lost.** Both the error path and `FD_CLOSE` call into the network object's vtable (slots `+0x08` and `+0x0C`, `0x54c710` and `0x54c690`).
Both show the "connection with server has been disconnected" system message (id `0x2755` = 10069; the error variant adds the Winsock error
code; which of the two texts is used depends on a value returned by `0x429e20`, not traced), set a quit flag in the object (`+4`) and `PostMessage(WM_CLOSE)` to the main window. The client
exits. There is **no reconnect logic**: not for the login connection, not for the game connection (confirmed).

**Server-ordered hop (`0x0111`).** The packet carries a new address (u32, four bytes `a.b.c.d`) and 32 key bytes. The handler re-keys AES
(`0x401000`) and `0x54c910` stores the address and sets the main state to 6. The frame function (`0x42cc10`) then, in state 6
(confirmed from its decompilation):

1. closes the socket but keeps the cipher state (`0x578ca0(6)`),
2. sleeps 500 ms and then pumps messages for 5001 ms (`0x1389`),
3. connects to the new address on **30810**, switches all flags off, sends `0xA301` in plain text again with the stored user id and token,
4. turns AES on with the *new* key (`0x4013c0`).

This is the only automatic reconnection in the client. It looks like a hop between game servers (inferred; no packet source has been
observed).

**Keep-alive and timeouts.**

| Mechanism | Value | Where |
|---|---|---|
| `0x0003` ping, client → server (2 bytes: no payload) | every **120 000 ms**, checked once per frame | `0x42cc84` (timer `0x705088`), sender `0x585790` (confirmed) |
| second ping site | same 120 000 ms (timer `0x84d640`) inside a loop that sleeps 5 s per turn | `0x4b1260` (confirmed; the loop looks like a worker thread's, inferred) |
| connect waits | 30 000 ms | four places, section 2 |
| redirect delay | 500 ms + 5001 ms | `0x42cc10` state 6 |
| the portal re-send | 15 s | `0x443541` (see [client-packets.md](client-packets.md)) |

The client has **no receive timeout**. It never expects a ping from the server and never complains about silence on an open connection (confirmed:
no timer anywhere in `0x585280` or the dispatch path). Whether a server drops idle clients is the server's own policy.

## 9. Two things the client checks that are easy to miss

- `0x6f4604`, which the send and receive functions test first, is **not** a "connected" flag. It is a global initialised to 1 in `.data` and
  never written anywhere (confirmed: no write references). Treat it as a constant.
- The packet log (`SEND>>`, `RECV>>`) is written by `0x5851b0` only when bit `0x40` of `0x2232e14` is set; see
  [client-packets.md](client-packets.md) for the `gsConfig.cfg` that turns it on. `SEND>>` is logged before encryption, `RECV>>` after
  decryption, so the log is always readable.

## 10. Worked examples

### 10.1 One packet out: picking up a dropped item (`0x0205`)

1. **Trigger (inferred).** The player clicks a ground item; the click handling (not traced here) stores the item as the current target
   and puts the player's action state into "walk to item". Each frame the main loop `0x42cc10` calls `0x4216f0` (at `0x42eb5e`), which
   runs the action-state machine `0x43c000` (at `0x421727`).
2. **Arrival (confirmed).** When the player is close enough, the state machine calls `0x5859a0(groundItemId)` at `0x43fcd3` (the id is read from
   the target record at `[edi+4]`) and moves on to the next state (`[0x7050b0]`).
3. **Packet build (confirmed).** `0x5859a0` writes the opcode `0x0205` and the id into a stack buffer and calls `0x585640(buf, 6)`:

   ```
   buf (plain)        05 02 | 78 56 34 12          opcode 0x0205, ground item id 0x12345678
   ```
4. **Log (confirmed).** `SEND>> [0205/6] 05 02 78 56 34 12` if the packet log is on.
5. **Frame and encrypt (confirmed).** The length slot is `6 + 2 = 8`, so the buffer to send is `08 00 | 05 02 78 56 34 12`.
   `0x4010a0` encrypts the six bytes after the length using the next six bytes of the AES keystream:

   ```
   on the wire:  08 00 | (05 02 78 56 34 12) XOR keystream[p .. p+5]
   ```
6. **`send()`** pushes the 8 bytes to the socket. The keystream position `p` has moved six bytes (or the next block was generated).
7. The server decrypts with its own copy of the same keystream, sees opcode `0x0205` and a 4-byte id.

### 10.2 One packet in: HP/MP/SP recovery (`0x0505`)

1. **Bytes arrive.** Winsock posts `0x2B10` with `FD_READ`; `0x585280` calls `recv` and appends to the accumulation buffer. Suppose the 14 bytes
   below are there (plain form; on the wire everything after the length is XORed with the table because the player is in the world):

   ```
   0E 00 | 05 05 | 34 12 00 00 | 0A 00 | 00 00 | 05 00
   length  opcode  entity id    HP +10   MP +0   SP +5
   ```
2. **Whole packet?** `0x000E <= 14` buffered: yes. The 12 bytes after the length are copied into the stream object (`0x589c90`).
3. **Decrypt.** XOR flag on and mode 2: `buf[j] ^= table[12 + j]` for the 12 bytes (`0x4010e0`). The leftover buffer is shifted to the front.
4. **Opcode.** `read(&opcode, 2)` gives `0x0505`; `RECV>> [0505/12] …` is logged.
5. **Dispatch.** `0x582e10` branches on `0x0505` to the reader `0x57ae20`, which reads `u32 id`, `u16`, `u16`, `u16` and calls vtable slot `+0xEC`
   (`0x54e2c0`) with the four values.
6. **Effect (confirmed).** `0x54e2c0` finds the entity with `0x447880(id)` and adds the three signed 16-bit values to three 32-bit fields of
   the entity (`+0x130`, `+0x138`, `+0x140`), then clamps each at 0 from below (there is no upper clamp). With the HP, MP, SP order documented for
   the recovery packets in [client-world-packets.md](client-world-packets.md) those fields are HP, MP and SP (inferred from the order).
7. **To the screen (inferred).** The bars and numbers read those fields when the UI is drawn each frame; this step was not traced.
8. **End of packet.** `0x589cf0` clears the underflow marker, and the loop checks whether another complete packet is buffered.

## 11. How to find this yourself

| Question | Start here | What led to it |
|---|---|---|
| Where is the socket code? | the import table: `WSAStartup`, `WSAAsyncSelect`, `connect`, `send`, `recv` | cross-references from the imports lead to `0x5854b0`, `0x578b10`, `0x585280`, `0x585640` in a few hops |
| Which function is the receive entry? | the constant `0x2B10` | `scalar`-style search for that immediate finds exactly the six uses in the connect, send and window-procedure functions |
| The maintainer-visible error texts | the strings `LoginServer Connet Error!!!`, `key Data Recv!!!`, `Send Error!!!` | each has one reference: `0x47bb10`, `0x40a6f0`/`0x4dca00`, `0x585640` |
| The dispatcher | callers of the `RECV>>` dump (string `RECV>>`, `PacketDump`, `ProjectSNet.cpp`) | the function after it in `0x585280` is the dispatcher |
| Underflow | the string `Packet Underflow. Protocol` | `0x552c40`, and the vtable slot that points to it is `+0x5FC`, which `read()` calls |
| The cipher | the SHA-256 initial values `0x6a09e667`, `0xbb67ae85`; the AES S-box table (first bytes `63 7c 77 7b`) at `0x6e1e08`; the C `rand()` constants `0x343fd` and `0x269ec3` | each constant has a handful of users: `0x404410`/`0x4044a0` (hashing), `0x404770` (AES), `0x4046a0`/`0x4015c0` (random and the alternative stream cipher) |
| Which handler handles an opcode | the table in `client-packets.generated.md` or the dispatcher's compare tree | `DumpFunctions`/`DecompileAt` on the reader, then follow its single vtable call |
| What the vtable slot does | read the table at `0x6A5C20` as 32-bit pointers | `slot = vtable + offset`; slot `+0x218` is the 134th pointer |
| The `0xA101` key exchange | the string `GNU MP: Cannot allocate memory` | appears in the call tree of the `0xA101` handler (see `client-handler-hints.tsv`), which points to the big-number code |
| Who calls the cipher switches | cross-references to `0x401160`, `0x4013c0`, `0x401570`, `0x4015a0` | connect functions, the `0x010B` / `0x0113` / `0x0107` handlers, the `0x0111` redirect |

Practical notes from the analysis: many handlers are only reachable through pointers (vtable slots), so Ghidra does not list them until
"define missing code" has been run once over the pointer tables. And the decompiler hides register-passed arguments (`0x4010a0`/`0x4010e0` look
like they take only a length); the disassembly shows the buffer and context in `EAX`/`ECX`/`ESI`.

## 12. Confirmed and inferred at a glance

| Statement | Status |
|---|---|
| Ports 30800 and 30810; blocking `connect`; `WSAAsyncSelect(0x2B10, FD_READ\|FD_CLOSE)` | confirmed (`0x578b10`, `0x4dca00`) |
| Frame layout `u16 length (self included) + u16 opcode + payload` | confirmed (`0x585640`, `0x585280`) |
| `0xA101` flow: random 128-byte secret → RSA → HMAC-SHA256(secret, modulus) → key and IV | confirmed except the big-number routine, which is inferred to be modular exponentiation |
| Random numbers from the Microsoft C `rand()` recurrence seeded with `time()` | confirmed (`0x4046a0`, `0x69ec30`) |
| AES-128 CTR, continuous, little-endian counter | confirmed (`0x4051b0`) |
| Alternative stream cipher for `mode != 0` | confirmed in code; use on the wire unknown |
| XOR table layer, server → client only, chain of SHA-256 | confirmed (`0x4010e0`, `0x401520`) |
| `0x010B` / `0x0113` turn XOR on, `0x0107` off | confirmed (handlers) |
| `0xA301` is sent in clear | confirmed (`0x586910` sent between `0x401160` and `0x4013c0`) |
| 16 KiB send queue, flush on `FD_WRITE` | confirmed; queue-ordering risk inferred |
| No reconnect; message box then exit | confirmed (`0x54c690`, `0x54c710`) |
| `0x0111` hop to another game server | mechanism confirmed; its purpose inferred |
| 120-second `0x0003` ping, nothing else | confirmed for senders; "no other keep-alive" is a negative search over the send table |
| Click-to-packet chain for pickup, and the drawing of recovery bars | inferred, not traced |

### Corrections to earlier notes

Two statements elsewhere in these notes did not survive this reading:

- The flag at `0x6f4604` is a constant 1, not a "connected" flag (section 9).
- The underflow report is made once per packet, not once per opcode (section 7).

### Open questions

- ~~What the two disconnect opcodes `0x0002` and `0x0004` do~~ settled: see the table in section 7 and [re-open-items.md](re-open-items.md).
- Whether any server has ever selected the alternative stream cipher (`mode != 0`).
- The exact buffer capacity of the accumulation buffer at `0x222aa40` and what happens when it overflows (no check was found; not tested).
- The click handling and per-frame drawing steps of the worked examples.
