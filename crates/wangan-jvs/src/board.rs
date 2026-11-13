//! The I/O board's replies.
//!
//! A port of TeknoParrotUI 1.0.0.2296's `JvsPackageEmulator` in the mode
//! its Namco profiles use (`Namco = true`, no Taito, single board), read
//! from the decompiled program and from ProjectAsakura's GPL-3.0 fork of
//! its source. Where TeknoParrotUI throws (a missing argument byte, an
//! unknown Namco `0x70` sub-command, more than two players) the C# side
//! caught the exception and sent nothing; [`Board::reply`] returns `None`
//! in exactly those cases.

use crate::frame::{SYNC, checksum, escape};
use crate::shared::Snapshot;

/// Identity and feature numbers the board reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardConfig {
    /// Answer to command 0x10, NUL included.
    pub identifier: &'static str,
    pub command_revision: u8,
    pub jvs_version: u8,
    pub comm_version: u8,
    /// Switches per player reported by command 0x14.
    pub switch_count: u8,
}

/// The board TeknoParrotUI emulates for `NamcoWmmt5` (Maximum Tune 5,
/// 5DX, 5DX+, 6) and `NamcoWmmt6RR` (6R, 6RR): the Namco JVS I/O of
/// Mario Kart Arcade GP DX, `JVSIdentifiers.NBGI_MarioKart3`.
pub const NAMCO_WMMT: BoardConfig = BoardConfig {
    identifier: "NBGI.;NA-JV;Ver6.01;JPN,MK3100-1-NA-APR0-A01\0",
    command_revision: 0x31,
    jvs_version: 0x31,
    comm_version: 0x31,
    switch_count: 0x18,
};

/// What a packet did besides answering.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Bytes to write back to the pipe (escaped), if any.
    pub reply: Option<Vec<u8>>,
    /// New value for the "addressed" flag in `TeknoParrot_JvsState`.
    pub addressed: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Board {
    config: BoardConfig,
    /// Coins taken by the game per slot (commands 0x30/0x31).
    taken: [u32; 4],
}

struct Step {
    consumed: usize,
    bytes: Vec<u8>,
}

impl Board {
    pub fn new(config: BoardConfig) -> Self {
        Self {
            config,
            taken: [0; 4],
        }
    }

    pub fn config(&self) -> &BoardConfig {
        &self.config
    }

    /// Coins the game sees in `slot` (0-based) for the player's total.
    pub fn coins(&self, slot: usize, inserted: u16) -> u16 {
        (inserted as u32).saturating_sub(self.taken[slot]) as u16
    }

    /// Answer one unescaped packet (`E0 node len payload… sum`).
    pub fn reply(&mut self, packet: &[u8], input: &Snapshot) -> Outcome {
        let mut out = Outcome::default();
        if packet.len() <= 3 {
            return out;
        }
        let node = packet[1];
        if node != 0xFF && node > 0x01 {
            return out;
        }
        // TeknoParrotUI's special case for a Type X probe; its counter test
        // can never be true, so the answer is always a single 0x00.
        if packet.len() >= 7 && packet[..7] == [0xE0, 0x00, 0x04, 0x01, 0x01, 0x08, 0x0E] {
            out.reply = Some(craft(&[0x00]));
            return out;
        }
        if packet[3] == 0xF0 {
            out.addressed = Some(false);
            return out;
        }
        let commands = &packet[3..packet.len() - 1];
        let size = (packet[2] as usize).saturating_sub(1);
        let mut rest = commands;
        let mut done = 0usize;
        let mut body = Vec::new();
        let mut multi = false;
        while done < size {
            let Some(step) = self.parse(rest, multi, node, input, &mut out) else {
                return Outcome {
                    reply: None,
                    addressed: out.addressed,
                };
            };
            let take = step.consumed.min(rest.len());
            rest = &rest[take..];
            done += step.consumed;
            body.extend_from_slice(&step.bytes);
            multi = true;
            if step.consumed == 0 {
                return Outcome {
                    reply: None,
                    addressed: out.addressed,
                };
            }
        }
        out.reply = Some(craft(&body));
        out
    }

    fn parse(
        &mut self,
        b: &[u8],
        multi: bool,
        node: u8,
        input: &Snapshot,
        out: &mut Outcome,
    ) -> Option<Step> {
        let cmd = *b.first()?;
        let report = |bytes: &[u8]| -> Vec<u8> {
            let mut v = Vec::with_capacity(bytes.len() + 1);
            if multi {
                v.push(0x01);
            }
            v.extend_from_slice(bytes);
            v
        };
        let step = |consumed: usize, bytes: Vec<u8>| Some(Step { consumed, bytes });
        match cmd {
            0xF1 => {
                out.addressed = Some(true);
                let target = *b.get(1)?;
                if matches!(target, 0x01 | 0x02 | 0x07) {
                    step(2, vec![])
                } else {
                    None
                }
            }
            // The Taito handlers come first in TeknoParrotUI's switch, so a
            // Namco game gets these answers too.
            0x01 => step(2, vec![0x01, 0x01]),
            0x03 => step(2, vec![0x01]),
            0x04 => step(1, vec![]),
            0x05 => step(3, vec![]),
            0x25 => step(2, vec![]),
            0x65 => step(
                2,
                if multi {
                    vec![0x01, 0x00, 0x00]
                } else {
                    vec![0x00, 0x00]
                },
            ),
            0x6A => step(9, vec![0x01]),
            0x6B | 0x6D | 0x23 => step(2, vec![]),
            0x10 => step(1, self.config.identifier.as_bytes().to_vec()),
            0x11 => step(1, report(&[self.config.command_revision])),
            0x12 => step(1, report(&[self.config.jvs_version])),
            0x13 => step(1, report(&[self.config.comm_version])),
            0x14 => step(
                1,
                report(&[
                    0x01,
                    0x02,
                    self.config.switch_count,
                    0x00, // switches: 2 players
                    0x02,
                    0x02,
                    0x00,
                    0x00, // coin slots: 2
                    0x03,
                    0x08,
                    0x0A,
                    0x00, // analogs: 8 channels, 10 bits
                    0x12,
                    0x14, // general purpose outputs: 20
                    0x00,
                    0x00,
                    0x00, // end
                ]),
            ),
            0x15 => {
                let id_len = b.iter().take_while(|&&x| x != 0).count();
                step(id_len + 1, vec![0x01, 0x01, 0x05])
            }
            0x20 => {
                let players = *b.get(1)?;
                let per = *b.get(2)?;
                if players > 2 {
                    return None;
                }
                let base = if node == 2 { 2 } else { 0 };
                let mut v = report(&[input.special]);
                for p in 0..players as usize {
                    let sw = input.player(base + p);
                    for i in 0..per as usize {
                        v.push(sw.get(i).copied().unwrap_or(0));
                    }
                }
                step(3, v)
            }
            0x21 => {
                let slots = *b.get(1)? as usize;
                let mut v = report(&[]);
                for slot in 0..slots {
                    let inserted = *input.coins.get(slot)?;
                    let n = self.coins(slot, inserted);
                    v.push((n >> 8) as u8);
                    v.push(n as u8);
                }
                step(2, v)
            }
            0x22 => {
                let channels = if b.len() == 1 { 8 } else { b[1] as usize };
                let mut v = report(&[]);
                for ch in 0..channels {
                    // TeknoParrotUI reads the second board's bytes for any
                    // node other than 1; there is no second board here.
                    let (hi, lo) = if node == 1 {
                        (*input.analog.get(ch * 2)?, *input.analog.get(ch * 2 + 1)?)
                    } else {
                        if ch * 2 + 1 >= input.analog.len() {
                            return None;
                        }
                        (0, 0)
                    };
                    v.push(hi);
                    v.push(lo);
                }
                step(2, v)
            }
            0x26 => {
                let n = *b.get(1)? as usize;
                let mut v = report(&[]);
                v.extend(std::iter::repeat_n(0u8, n));
                step(2, v)
            }
            0x2E => step(2, report(&[0x00, 0x00, 0x00, 0x00])),
            0x2F => step(1, vec![0x00; 5]),
            0x30 | 0x31 => {
                let slot = *b.get(1)? as usize;
                let count = u16::from_be_bytes([*b.get(2)?, *b.get(3)?]) as u32;
                let idx = slot.checked_sub(1)?;
                let inserted = *input.coins.get(idx)? as u32;
                let taken = self.taken.get_mut(idx)?;
                *taken = (*taken + count).min(inserted.max(*taken));
                step(4, report(&[]))
            }
            0x32 | 0x34 => {
                let n = *b.get(1)? as usize;
                if cmd == 0x32 && b.len() < 2 + n {
                    return None; // BlockCopy past the end throws
                }
                step(n + 2, report(&[]))
            }
            0x33 => {
                let n = *b.get(1)? as usize;
                step(n * 2 + 2, report(&[]))
            }
            0x36 => step(4, report(&[])),
            0x37 => step(3, report(&[])),
            0x70 => {
                let sub = *b.get(1)?;
                match sub {
                    0x18 => step(b.len(), if multi { vec![0x01, 0x01] } else { vec![0x01] }),
                    0x05 => step(
                        *b.get(2)? as usize,
                        if multi { vec![0x01, 0x01] } else { vec![0x01] },
                    ),
                    0x03 => step(4, if multi { vec![0x01, 0x00] } else { vec![0x00] }),
                    0x15 | 0x16 => step(4, if multi { vec![0x01, 0x01] } else { vec![0x01] }),
                    _ => None,
                }
            }
            0x78..=0x80 => step(15, vec![0x01]),
            _ => step(1, vec![]),
        }
    }
}

/// `CraftJvsPackageWithStatusAndReport(0, bytes)`: status 1, report 1.
fn craft(body: &[u8]) -> Vec<u8> {
    let mut payload = vec![0x01, 0x01];
    payload.extend_from_slice(body);
    let len = (payload.len() + 1) as u8;
    let mut packet = vec![SYNC, 0x00, len];
    packet.extend_from_slice(&payload);
    packet.push(checksum(0x00, len, &payload));
    escape(&packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Assembler;

    fn packet(node: u8, cmds: &[u8]) -> Vec<u8> {
        let len = cmds.len() as u8 + 1;
        let mut p = vec![SYNC, node, len];
        p.extend_from_slice(cmds);
        p.push(checksum(node, len, cmds));
        p
    }

    fn body(reply: &[u8]) -> Vec<u8> {
        let mut asm = Assembler::new();
        let got = asm.push(reply);
        assert_eq!(got.len(), 1, "one reply packet");
        let p = &got[0];
        assert_eq!(&p[..2], &[SYNC, 0x00]);
        assert_eq!(p[2] as usize, p.len() - 3);
        let payload = &p[3..p.len() - 1];
        assert_eq!(*p.last().unwrap(), checksum(0, p[2], payload));
        assert_eq!(&payload[..2], &[0x01, 0x01], "status and report");
        payload[2..].to_vec()
    }

    #[test]
    fn reset_clears_addressed_and_says_nothing() {
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(&packet(0xFF, &[0xF0, 0xD9]), &Snapshot::default());
        assert_eq!(
            o,
            Outcome {
                reply: None,
                addressed: Some(false)
            }
        );
    }

    #[test]
    fn address_sets_addressed_and_answers_empty() {
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(&packet(0xFF, &[0xF1, 0x01]), &Snapshot::default());
        assert_eq!(o.addressed, Some(true));
        assert_eq!(body(&o.reply.unwrap()), Vec::<u8>::new());
    }

    #[test]
    fn identity_commands_in_one_packet() {
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(
            &packet(0x01, &[0x10, 0x11, 0x12, 0x13]),
            &Snapshot::default(),
        );
        let mut want = NAMCO_WMMT.identifier.as_bytes().to_vec();
        want.extend_from_slice(&[0x01, 0x31, 0x01, 0x31, 0x01, 0x31]);
        assert_eq!(body(&o.reply.unwrap()), want);
    }

    #[test]
    fn features_report_24_switches_8_analogs() {
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(&packet(0x01, &[0x14]), &Snapshot::default());
        assert_eq!(
            body(&o.reply.unwrap()),
            vec![1, 2, 24, 0, 2, 2, 0, 0, 3, 8, 10, 0, 0x12, 0x14, 0, 0, 0]
        );
    }

    #[test]
    fn switches_coins_and_analogs_come_from_the_snapshot() {
        let mut s = Snapshot {
            special: 0x80,
            ..Snapshot::default()
        };
        s.players[0] = [0x40, 0x02, 0x00, 0x00];
        s.coins[0] = 3;
        s.analog[0] = 0x7F; // wheel
        s.analog[2] = 0xFF; // gas
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(
            &packet(0x01, &[0x20, 0x02, 0x02, 0x21, 0x02, 0x22, 0x03]),
            &s,
        );
        assert_eq!(
            body(&o.reply.unwrap()),
            vec![
                0x80, 0x40, 0x02, 0x00, 0x00, // switches: special, P1 x2, P2 x2
                0x01, 0x00, 0x03, 0x00, 0x00, // coins
                0x01, 0x7F, 0x00, 0xFF, 0x00, 0x00, 0x00, // 3 analog channels
            ]
        );
    }

    #[test]
    fn coin_decrease_is_remembered_and_never_negative() {
        let mut s = Snapshot::default();
        s.coins[0] = 2;
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(&packet(0x01, &[0x30, 0x01, 0x00, 0x05]), &s);
        assert!(o.reply.is_some());
        assert_eq!(b.coins(0, s.coins[0]), 0);
        // two more coins after the game took everything
        assert_eq!(b.coins(0, 4), 2);
    }

    #[test]
    fn unknown_namco_custom_subcommand_is_dropped_like_teknoparrot() {
        let mut b = Board::new(NAMCO_WMMT);
        assert_eq!(
            b.reply(&packet(0x01, &[0x70, 0x99, 0x00]), &Snapshot::default())
                .reply,
            None
        );
        let o = b.reply(
            &packet(0x01, &[0x70, 0x18, 0xAA, 0xBB]),
            &Snapshot::default(),
        );
        assert_eq!(body(&o.reply.unwrap()), vec![0x01]);
    }

    #[test]
    fn unknown_command_is_skipped_one_byte() {
        let mut b = Board::new(NAMCO_WMMT);
        let o = b.reply(&packet(0x01, &[0x99, 0x11]), &Snapshot::default());
        assert_eq!(body(&o.reply.unwrap()), vec![0x01, 0x31]);
    }

    #[test]
    fn other_nodes_are_ignored() {
        let mut b = Board::new(NAMCO_WMMT);
        assert_eq!(
            b.reply(&packet(0x02, &[0x10]), &Snapshot::default()),
            Outcome::default()
        );
    }

    #[test]
    fn truncated_arguments_drop_the_packet() {
        let mut b = Board::new(NAMCO_WMMT);
        assert_eq!(
            b.reply(&packet(0x01, &[0x20, 0x01]), &Snapshot::default())
                .reply,
            None
        );
    }
}
