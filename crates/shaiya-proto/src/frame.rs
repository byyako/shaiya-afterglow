/// Length field plus opcode.
pub const HEADER_LEN: usize = 4;
/// Largest packet the original server buffers (2044 bytes of command data after the opcode).
pub const MAX_PACKET_LEN: usize = 2048;

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    /// The length field is smaller than the header or larger than `MAX_PACKET_LEN`.
    BadLength(u16),
}

/// Builds a plaintext packet. A payload over `MAX_PACKET_LEN - HEADER_LEN` bytes is a bug in the caller: a debug build stops on it; a
/// release build logs it at error and cuts the payload, since a panic here would be inside a zone lock (`docs/server/audit.md` R22).
pub fn encode(opcode: u16, payload: &[u8]) -> Vec<u8> {
    debug_assert!(HEADER_LEN + payload.len() <= MAX_PACKET_LEN, "packet too large: {}", HEADER_LEN + payload.len());
    let payload = if HEADER_LEN + payload.len() > MAX_PACKET_LEN {
        tracing::error!(
            opcode = format_args!("{opcode:#06x}"),
            length = HEADER_LEN + payload.len(),
            "packet too large, cut to {MAX_PACKET_LEN} bytes"
        );
        &payload[..MAX_PACKET_LEN - HEADER_LEN]
    } else {
        payload
    };
    let length = HEADER_LEN + payload.len();
    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(&(length as u16).to_le_bytes());
    out.extend_from_slice(&opcode.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Splits a TCP byte stream into packets. Only the length field is read, so it works
/// before and after encryption; the caller decrypts each returned packet in order.
#[derive(Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Returns the next complete packet (length field included), `Ok(None)` if more
    /// bytes are needed, or an error if the stream is not valid and should be dropped.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        if self.buffer.len() < 2 {
            return Ok(None);
        }
        let length = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
        if (length as usize) < HEADER_LEN || length as usize > MAX_PACKET_LEN {
            return Err(FrameError::BadLength(length));
        }
        if self.buffer.len() < length as usize {
            return Ok(None);
        }
        let rest = self.buffer.split_off(length as usize);
        Ok(Some(std::mem::replace(&mut self.buffer, rest)))
    }
}

/// Opcode of a packet returned by `FrameDecoder` (after decryption, if any).
pub fn opcode(packet: &[u8]) -> u16 {
    u16::from_le_bytes([packet[2], packet[3]])
}

/// Payload of a packet returned by `FrameDecoder`.
pub fn payload(packet: &[u8]) -> &[u8] {
    &packet[HEADER_LEN..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_prefixes_total_length() {
        assert_eq!(encode(0xA101, &[1, 2, 3]), [7, 0, 0x01, 0xA1, 1, 2, 3]);
    }

    #[test]
    fn the_largest_payload_fits() {
        let p = encode(0xA101, &[7; MAX_PACKET_LEN - HEADER_LEN]);
        assert_eq!(p.len(), MAX_PACKET_LEN);
        assert_eq!(u16::from_le_bytes([p[0], p[1]]) as usize, MAX_PACKET_LEN);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "packet too large")]
    fn an_oversize_payload_stops_a_debug_build() {
        encode(0xA101, &[0; MAX_PACKET_LEN]);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn an_oversize_payload_is_cut_in_a_release_build() {
        let p = encode(0xA101, &[0; MAX_PACKET_LEN]);
        assert_eq!(p.len(), MAX_PACKET_LEN);
        let mut dec = FrameDecoder::new();
        dec.push(&p);
        assert_eq!(dec.next_frame(), Ok(Some(p)), "the cut packet is a valid frame");
    }

    #[test]
    fn decoder_waits_for_whole_packets_and_splits_batches() {
        let mut stream = encode(0x0003, &[]);
        stream.extend(encode(0xA102, &[9; 10]));
        let mut dec = FrameDecoder::new();

        dec.push(&stream[..5]);
        assert_eq!(dec.next_frame(), Ok(Some(encode(0x0003, &[]))));
        assert_eq!(dec.next_frame(), Ok(None));

        dec.push(&stream[5..]);
        let p = dec.next_frame().unwrap().unwrap();
        assert_eq!(opcode(&p), 0xA102);
        assert_eq!(payload(&p), [9; 10]);
        assert_eq!(dec.next_frame(), Ok(None));
    }

    #[test]
    fn decoder_rejects_bad_lengths() {
        let mut dec = FrameDecoder::new();
        dec.push(&[3, 0]);
        assert_eq!(dec.next_frame(), Err(FrameError::BadLength(3)));

        let mut dec = FrameDecoder::new();
        dec.push(&(MAX_PACKET_LEN as u16 + 1).to_le_bytes());
        assert!(dec.next_frame().is_err());
    }
}
