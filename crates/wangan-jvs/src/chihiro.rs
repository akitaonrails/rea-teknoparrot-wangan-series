//! Maximum Tune 1 (Sega Chihiro) in the TeknoParrot fork of Cxbx-Reloaded:
//! the emulator builds its JVS board inside and reads the controls from
//! the `TeknoParrot_JvsState` section (`src/devices/chihiro/JvsIo.cpp`,
//! `JvsIo::Update`), so there the board is only a translator from the
//! launcher's block to that section:
//!
//! ```text
//! 8   u32  control bits (Test, Start, Service, directions, Button1-8)
//! 12  [u8;4] analog bytes (Maximum Tune: steering, gas, brake, unused)
//! 32  u32  coin switch, edge-triggered: one press = one coin
//! ```
//!
//! The control bits map one-to-one onto the JVS switch bytes the launcher
//! already writes (`JvsIo.h`, `GetByte0`/`GetByte1`), so the controls of
//! the Maximum Tune 5 cabinet carry over unchanged: gears on Button3-6,
//! view change on byte 1 bit 1, intrude on byte 1 bit 0.

use crate::shared::Snapshot;

/// Offsets in `TeknoParrot_JvsState`.
pub const CONTROL: usize = 8;
pub const ANALOG: usize = 12;
pub const COIN: usize = 32;

/// (snapshot byte, bit) → control bit, for player 1 and the special byte.
const SPECIAL: &[(u8, u32)] = &[(0x80, 0x01)]; // Test
const P1_BYTE0: &[(u8, u32)] = &[
    (0x80, 0x02),   // Start
    (0x40, 0x40),   // Service
    (0x20, 0x800),  // Up
    (0x10, 0x2000), // Down
    (0x08, 0x400),  // Left
    (0x04, 0x1000), // Right
    (0x02, 0x04),   // Button1
    (0x01, 0x20),   // Button2
];
const P1_BYTE1: &[(u8, u32)] = &[
    (0x80, 0x200),     // Button3
    (0x40, 0x80000),   // Button4
    (0x20, 0x100000),  // Button5
    (0x10, 0x200000),  // Button6
    (0x02, 0x400000),  // byte 1 bit 1 (view change)
    (0x01, 0x8000000), // byte 1 bit 0 (intrude)
];

fn bits(byte: u8, table: &[(u8, u32)]) -> u32 {
    table
        .iter()
        .filter(|(b, _)| byte & b != 0)
        .fold(0, |acc, (_, c)| acc | c)
}

/// The control word for a snapshot (player 1 and Test; Maximum Tune 1 is
/// a one-player cabinet).
pub fn control(s: &Snapshot) -> u32 {
    let p = s.player(0);
    bits(s.special, SPECIAL) | bits(p[0], P1_BYTE0) | bits(p[1], P1_BYTE1)
}

/// Steering, gas and brake: the launcher's analog channels 0, 1 and 2
/// (`controls`, TeknoParrot's `Analog0/2/4`), one byte each.
pub fn analog(s: &Snapshot) -> [u8; 4] {
    [s.analog[0], s.analog[2], s.analog[4], 0]
}

/// Turns the launcher's running coin count into presses of the coin
/// switch, which the emulator counts on the rising edge.
#[derive(Debug, Default)]
pub struct CoinSwitch {
    delivered: u16,
    /// When the current press or release started, in ms.
    since: Option<(bool, u64)>,
}

/// How long the switch is held down, and then up, per coin.
pub const PRESS_MS: u64 = 80;

impl CoinSwitch {
    /// Whether the switch is down at `now_ms`, for `inserted` coins so far.
    pub fn down(&mut self, inserted: u16, now_ms: u64) -> bool {
        if inserted < self.delivered {
            // the launcher restarted its count
            self.delivered = inserted;
            self.since = None;
        }
        match self.since {
            None if inserted > self.delivered => {
                self.since = Some((true, now_ms));
                true
            }
            None => false,
            Some((true, t)) if now_ms - t >= PRESS_MS => {
                self.delivered += 1;
                self.since = Some((false, now_ms));
                false
            }
            Some((false, t)) if now_ms - t >= PRESS_MS => {
                self.since = None;
                self.down(inserted, now_ms)
            }
            Some((pressed, _)) => pressed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{Cabinet, Controls};

    #[test]
    fn gears_view_and_intrude_reach_the_fork_buttons() {
        let c = Controls {
            gear: 1,
            view: true,
            intrude: true,
            test: true,
            ..Controls::default()
        };
        let s = c.to_snapshot(Cabinet::Wmmt5, [0; 4], 1);
        // gear 1 = Button3 | Button5
        assert_eq!(control(&s), 0x01 | 0x200 | 0x100000 | 0x400000 | 0x8000000);
    }

    #[test]
    fn start_and_menu_buttons() {
        let mut s = Snapshot::default();
        s.players[0] = [0x80 | 0x20 | 0x02, 0, 0, 0];
        assert_eq!(control(&s), 0x02 | 0x800 | 0x04);
    }

    #[test]
    fn analog_is_steer_gas_brake() {
        let c = Controls {
            wheel: -1.0,
            gas: 1.0,
            brake: 0.0,
            ..Controls::default()
        };
        let s = c.to_snapshot(Cabinet::Wmmt5, [0; 4], 1);
        assert_eq!(analog(&s), [0, 255, 0, 0]);
    }

    #[test]
    fn each_coin_is_one_press_then_release() {
        let mut c = CoinSwitch::default();
        assert!(!c.down(0, 0));
        // two coins at once: down, up, down, up
        let mut edges = 0;
        let mut was = false;
        for t in (0..1000).step_by(10) {
            let d = c.down(2, t);
            if d && !was {
                edges += 1;
            }
            was = d;
        }
        assert_eq!(edges, 2);
        assert!(!c.down(2, 2000));
        // a restarted count does not replay old coins
        assert!(!c.down(0, 2010));
        assert!(c.down(1, 2020));
    }
}
