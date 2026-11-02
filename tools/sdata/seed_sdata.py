#!/usr/bin/env python3
"""Decrypt or encrypt a SEED-encrypted SData file (EP4-7 header, 64 bytes) with its checksum.

usage: seed_sdata.py <Parsec SEEDConstants.cs> decrypt <encrypted.SData> <decrypted out>
       seed_sdata.py <Parsec SEEDConstants.cs> encrypt <decrypted> <encrypted.SData out>

The SEED tables, key and checksum table are read at run time from Parsec's SEEDConstants.cs (Parsec, github.com/matigramirez/Parsec), not
copied here. Checked against Parsec's ItemEp6.enc.SData / ItemEp6.dec.SData pair (byte-identical), and the client's Item.SData
decrypts and encrypts back to the same bytes (docs/client/sdata-generation.md).
"""
import re, struct, sys

SIGNATURE = b'0001CBCEBC5B2784D3FC9A2A9DB84D1C3FEB6E99'
HEADER = 64

def load_consts(cs_path):
    text = open(cs_path, encoding='utf-8-sig').read()
    def block(name):
        start = text.index(name)
        body = text[text.index('{', start):text.index('};', start)]
        return [int(v, 16) if v.lower().startswith('0x') else int(v) for v in re.findall(r'0x[0-9a-fA-F]+|\b\d+\b', body)]
    ss = block('uint[,] SS')
    assert len(ss) == 1024, len(ss)
    key = block('byte[] Key')
    cks = block('ChecksumTable')
    return [ss[i*256:(i+1)*256] for i in range(4)], bytes(key), bytes(cks)

M = 0xffffffff
def rol(x, n): return ((x << n) | (x >> (32 - n))) & M
def swap(v): return (rol(v, 8) & 0x00ff00ff) | (rol(v, 24) & 0xff00ff00)

def checksum(data, consts):
    cks = consts[2]
    c = M
    for b in data:
        c = (c >> 8) ^ swap(struct.unpack_from('>I', cks, ((c & 255) ^ b) * 4)[0])
    return ~c & M

def crypt(data, consts, offsets):
    """SEED on 16-byte blocks: round keys 30 down to 0 decrypt, 0 up to 30 encrypt."""
    SS, KEY, _ = consts
    G = lambda v: SS[0][v & 255] ^ SS[1][(v >> 8) & 255] ^ SS[2][(v >> 16) & 255] ^ SS[3][v >> 24]
    ks = [swap(struct.unpack_from('>I', KEY, i*4)[0]) for i in range(len(KEY)//4)]
    def rnd(L0, L1, R0, R1, o):
        T0 = R0 ^ ks[o]; T1 = R1 ^ ks[o+1]
        T1 ^= T0; T1 = G(T1); T0 = (T0 + T1) & M; T0 = G(T0); T1 = (T1 + T0) & M; T1 = G(T1); T0 = (T0 + T1) & M
        return L0 ^ T0, L1 ^ T1
    out = bytearray()
    for i in range(0, len(data), 16):
        L0, L1, R0, R1 = struct.unpack_from('>IIII', data, i)
        for r, o in enumerate(offsets):
            if r % 2 == 0: L0, L1 = rnd(L0, L1, R0, R1, o)
            else: R0, R1 = rnd(R0, R1, L0, L1, o)
        out += struct.pack('>IIII', R0, R1, L0, L1)
    return bytes(out)

def decrypt(buf, consts, check=True):
    assert buf[:40] == SIGNATURE, 'not encrypted'
    sum_, real = struct.unpack_from('<II', buf, 40)
    assert sum_ != 0, 'binary (EP8) header not handled'
    assert (len(buf) - HEADER) % 16 == 0
    out = crypt(buf[HEADER:], consts, range(30, -1, -2))[:real]
    if check:
        assert checksum(out, consts) == sum_, 'checksum mismatch'
    return out

def encrypt(data, consts):
    """Header: signature, checksum, real size, 16 zero bytes; then the data padded with zeros to 16 bytes."""
    padded = data + bytes(-len(data) % 16)
    header = SIGNATURE + struct.pack('<II', checksum(data, consts), len(data)) + bytes(16)
    return header + crypt(padded, consts, range(0, 31, 2))

if __name__ == '__main__':
    consts = load_consts(sys.argv[1])
    data = open(sys.argv[3], 'rb').read()
    out = {'decrypt': decrypt, 'encrypt': encrypt}[sys.argv[2]](data, consts)
    open(sys.argv[4], 'wb').write(out)
    print('ok', len(out))
