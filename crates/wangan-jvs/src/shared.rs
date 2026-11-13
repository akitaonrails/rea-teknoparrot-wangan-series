//! The 64-byte input block shared by the launcher (native, the only
//! writer) and the I/O board under Wine (the only reader).
//!
//! It lives in a plain file that both sides map: Wine maps a real file
//! with `mmap`, so a write from the native side is visible to the board
//! with no copy and no socket. Each field is a byte or a little-endian
//! integer that is meaningful on its own, so a read that races a write
//! sees at worst a one-frame mix of old and new controls.
//!
//! ```text
//! 0  magic  "WIO1"
//! 4  u32    sequence, bumped after every write (liveness)
//! 8  u8     special switches (bit 7 = Test)
//! 9  [u8;4] player 1 switch bytes, as the board sends them
//! 13 [u8;4] player 2 switch bytes
//! 17 [u16;4] coins inserted per slot since the launch
//! 25 [u8;32] analog bytes, two per channel (TeknoParrot's AnalogBytes)
//! 57 u8     quit: the launcher sets 1 when the session ends
//! 58 [u8;6] reserved, zero
//! ```

pub const SIZE: usize = 64;
pub const MAGIC: [u8; 4] = *b"WIO1";
/// Offset of the quit byte.
pub const QUIT: usize = 57;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub sequence: u32,
    pub special: u8,
    pub players: [[u8; 4]; 2],
    pub coins: [u16; 4],
    pub analog: [u8; 32],
    pub quit: bool,
}

impl Snapshot {
    /// Switch bytes of player `index` (0-based); zeros past player 2.
    pub fn player(&self, index: usize) -> [u8; 4] {
        self.players.get(index).copied().unwrap_or([0; 4])
    }

    pub fn encode(&self) -> [u8; SIZE] {
        let mut b = [0u8; SIZE];
        b[0..4].copy_from_slice(&MAGIC);
        b[4..8].copy_from_slice(&self.sequence.to_le_bytes());
        b[8] = self.special;
        b[9..13].copy_from_slice(&self.players[0]);
        b[13..17].copy_from_slice(&self.players[1]);
        for (i, c) in self.coins.iter().enumerate() {
            b[17 + i * 2..19 + i * 2].copy_from_slice(&c.to_le_bytes());
        }
        b[25..57].copy_from_slice(&self.analog);
        b[QUIT] = self.quit as u8;
        b
    }

    /// `None` when the block has not been written yet (no magic).
    pub fn decode(b: &[u8]) -> Option<Self> {
        if b.len() < SIZE || b[0..4] != MAGIC {
            return None;
        }
        let mut s = Snapshot {
            sequence: u32::from_le_bytes(b[4..8].try_into().ok()?),
            special: b[8],
            ..Snapshot::default()
        };
        s.players[0].copy_from_slice(&b[9..13]);
        s.players[1].copy_from_slice(&b[13..17]);
        for i in 0..4 {
            s.coins[i] = u16::from_le_bytes([b[17 + i * 2], b[18 + i * 2]]);
        }
        s.analog.copy_from_slice(&b[25..57]);
        s.quit = b[QUIT] != 0;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut s = Snapshot {
            sequence: 7,
            special: 0x80,
            ..Default::default()
        };
        s.players[0] = [1, 2, 3, 4];
        s.players[1] = [5, 6, 7, 8];
        s.coins = [1, 2, 300, 4];
        s.analog[0] = 0x7F;
        s.analog[31] = 9;
        s.quit = true;
        let b = s.encode();
        assert_eq!(Snapshot::decode(&b), Some(s));
    }

    #[test]
    fn unwritten_block_is_none() {
        assert_eq!(Snapshot::decode(&[0u8; SIZE]), None);
        assert_eq!(Snapshot::decode(&[0u8; 10]), None);
    }
}
