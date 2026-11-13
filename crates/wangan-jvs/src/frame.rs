//! JVS serial framing.
//!
//! A packet is `E0 node len payload… sum`, where `len` counts the payload
//! plus the checksum byte and `sum = node + len + Σpayload (mod 256)`.
//! Inside a packet `E0` and `D0` are sent as `D0 DF` and `D0 CF` (the byte
//! after `D0` is the real byte minus one).

pub const SYNC: u8 = 0xE0;
pub const MARK: u8 = 0xD0;

/// Checksum of a packet body as JVS defines it.
pub fn checksum(node: u8, len: u8, payload: &[u8]) -> u8 {
    payload
        .iter()
        .fold(node.wrapping_add(len), |acc, b| acc.wrapping_add(*b))
}

/// Escape a packet for the wire: the leading sync byte as is, then every
/// `E0`/`D0` as `D0 (b-1)`.
pub fn escape(packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packet.len() + 4);
    let mut iter = packet.iter();
    if let Some(first) = iter.next() {
        out.push(*first);
    }
    for &b in iter {
        if b == SYNC || b == MARK {
            out.push(MARK);
            out.push(b.wrapping_sub(1));
        } else {
            out.push(b);
        }
    }
    out
}

/// Turns the raw byte stream from the pipe back into packets
/// (`E0 node len payload… sum`, unescaped). Bytes before a sync byte are
/// dropped, as TeknoParrotUI does; an escape split across two reads is kept.
#[derive(Debug, Default)]
pub struct Assembler {
    escape_pending: bool,
    buf: Vec<u8>,
}

impl Assembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed bytes read from the pipe; returns every packet they complete.
    pub fn push(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        for &raw in data {
            let (byte, is_sync) = if self.escape_pending {
                self.escape_pending = false;
                (raw.wrapping_add(1), false)
            } else if raw == MARK {
                self.escape_pending = true;
                continue;
            } else {
                (raw, raw == SYNC)
            };
            if self.buf.is_empty() {
                if is_sync {
                    self.buf.push(byte);
                }
                continue;
            }
            if is_sync {
                // A new packet starts before the old one finished: start over.
                self.buf.clear();
                self.buf.push(byte);
                continue;
            }
            self.buf.push(byte);
            if self.buf.len() >= 3 {
                let want = 3 + self.buf[2] as usize;
                if self.buf.len() == want {
                    packets.push(std::mem::take(&mut self.buf));
                }
            }
        }
        packets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_round_trip_through_the_assembler() {
        let payload = [0x20, 0xE0, 0xD0, 0x01];
        let len = payload.len() as u8 + 1;
        let mut packet = vec![SYNC, 0x01, len];
        packet.extend_from_slice(&payload);
        packet.push(checksum(0x01, len, &payload));
        let wire = escape(&packet);
        assert_eq!(&wire[..4], &[SYNC, 0x01, len, 0x20]);
        assert_eq!(&wire[4..8], &[MARK, 0xDF, MARK, 0xCF]);
        let mut asm = Assembler::new();
        // split inside the escape sequence
        let mut got = asm.push(&wire[..5]);
        assert!(got.is_empty());
        got = asm.push(&wire[5..]);
        assert_eq!(got, vec![packet]);
    }

    #[test]
    fn garbage_before_sync_is_dropped_and_a_restart_wins() {
        let mut asm = Assembler::new();
        let got = asm.push(&[0x11, 0x22, SYNC, 0xFF, 0x03, SYNC, 0xFF, 0x02, 0xF0, 0xF1]);
        assert_eq!(got, vec![vec![SYNC, 0xFF, 0x02, 0xF0, 0xF1]]);
    }

    #[test]
    fn checksum_matches_the_reset_packet() {
        // E0 FF 03 F0 D9 CB is the broadcast reset every JVS master sends.
        assert_eq!(checksum(0xFF, 0x03, &[0xF0, 0xD9]), 0xCB);
    }
}
