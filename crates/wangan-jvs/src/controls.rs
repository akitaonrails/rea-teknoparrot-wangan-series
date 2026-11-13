//! The cabinet's controls, turned into the board's bytes.
//!
//! Every mapping here is TeknoParrotUI's for the Wangan profiles
//! (GameProfiles `WMMT5`…`WMMT6RR`, `InputListener*`, `DigitalHelper`,
//! `AnalogHelper`, `JvsHelper`):
//!
//! | Cabinet control | TeknoParrot mapping | Board bytes |
//! |---|---|---|
//! | Steering | `Analog0` | analog channel 0 |
//! | Gas | `Analog2` | analog channel 1 |
//! | Brake | `Analog4` | analog channel 2 |
//! | Test | `Test` | special bit 7 |
//! | Service | `Service1` | P1 byte 0, 0x40 |
//! | Test menu up / down / enter | `P1ButtonUp` / `Down` / `P1Button1` | P1 byte 0, 0x20 / 0x10 / 0x02 |
//! | View change (perspective) | `ExtensionOne2` | P1 byte 1, 0x02 |
//! | Intrude (interruption) | `ExtensionOne1` | P1 byte 1, 0x01 |
//! | Insert card (6, 6R, 6RR) | `ExtensionOne13` | P1 byte 2, 0x04 |
//! | Gear lever 1–6 | `Wmmt5GearChange*` | P1 byte 1, Button3–6 (H pattern) |

use crate::shared::Snapshot;

/// Which wheel range the game expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cabinet {
    /// Maximum Tune 5, 5DX, 5DX+, 6 (`NamcoWmmt5`): steering 0–255.
    Wmmt5,
    /// Maximum Tune 6R, 6RR (`NamcoWmmt6RR`): steering 20–236.
    Wmmt6RR,
}

impl Cabinet {
    fn wheel_range(self) -> (u8, u8) {
        match self {
            Cabinet::Wmmt5 => (0, 255),
            Cabinet::Wmmt6RR => (20, 236),
        }
    }
}

/// The player's controls at one moment, already filtered (dead zones,
/// curves and wheel calibration happen before this).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Controls {
    /// -1.0 full left … 1.0 full right.
    pub wheel: f32,
    /// 0.0 released … 1.0 floored.
    pub gas: f32,
    pub brake: f32,
    /// 0 neutral, 1–6.
    pub gear: u8,
    pub view: bool,
    pub intrude: bool,
    pub card: bool,
    pub test: bool,
    pub service: bool,
    pub menu_up: bool,
    pub menu_down: bool,
    pub enter: bool,
}

/// `JvsHelper.CalculateWheelPos`/`CalculateGasPos` on a 16-bit axis:
/// `raw / (65535 / (max - min)) + min`, integer division.
pub fn axis_byte(raw: u16, min: u8, max: u8) -> u8 {
    let span = (max - min) as u32;
    if span == 0 {
        return min;
    }
    let v = raw as u32 / (65535 / span) + min as u32;
    v.min(max as u32) as u8
}

fn to_raw_centered(v: f32) -> u16 {
    (((v.clamp(-1.0, 1.0) + 1.0) / 2.0) * 65535.0).round() as u16
}

fn to_raw(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0).round() as u16
}

/// Button3–6 for each gear, from `DigitalHelper.ChangeWmmt5Gear`.
pub fn gear_bits(gear: u8) -> u8 {
    const B3: u8 = 0x80;
    const B4: u8 = 0x40;
    const B5: u8 = 0x20;
    const B6: u8 = 0x10;
    match gear {
        1 => B3 | B5,
        2 => B4 | B5,
        3 => B3,
        4 => B4,
        5 => B3 | B6,
        6 => B4 | B6,
        _ => 0,
    }
}

impl Controls {
    pub fn to_snapshot(&self, cabinet: Cabinet, coins: [u16; 4], sequence: u32) -> Snapshot {
        let mut s = Snapshot {
            sequence,
            coins,
            ..Snapshot::default()
        };
        if self.test {
            s.special |= 0x80;
        }
        let p = &mut s.players[0];
        if self.service {
            p[0] |= 0x40;
        }
        if self.menu_up {
            p[0] |= 0x20;
        }
        if self.menu_down {
            p[0] |= 0x10;
        }
        if self.enter {
            p[0] |= 0x02;
        }
        p[1] |= gear_bits(self.gear);
        if self.view {
            p[1] |= 0x02;
        }
        if self.intrude {
            p[1] |= 0x01;
        }
        if self.card {
            p[2] |= 0x04;
        }
        let (lo, hi) = cabinet.wheel_range();
        s.analog[0] = axis_byte(to_raw_centered(self.wheel), lo, hi);
        s.analog[2] = axis_byte(to_raw(self.gas), 0, 255);
        s.analog[4] = axis_byte(to_raw(self.brake), 0, 255);
        s
    }
}

/// The sequential shifter the player uses with a pad or paddles, kept the
/// way TeknoParrotUI keeps it (`ChangeWmmt5GearUp/Down`: 0…6, starting in 1
/// at game start).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shifter {
    gear: u8,
}

impl Default for Shifter {
    fn default() -> Self {
        Self { gear: 1 }
    }
}

impl Shifter {
    pub fn gear(&self) -> u8 {
        self.gear
    }
    pub fn up(&mut self) {
        if self.gear < 6 {
            self.gear += 1;
        }
    }
    pub fn down(&mut self) {
        if self.gear > 0 {
            self.gear -= 1;
        }
    }
    pub fn set(&mut self, gear: u8) {
        self.gear = gear.min(6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_centre_and_ends_match_teknoparrot() {
        // DirectInput wheel centred at 32767 → 127 with 0..255.
        assert_eq!(axis_byte(32767, 0, 255), 127);
        assert_eq!(axis_byte(0, 0, 255), 0);
        assert_eq!(axis_byte(65535, 0, 255), 255);
        // 6R/6RR range 20..236: 65535/216 = 303.
        assert_eq!(axis_byte(32767, 20, 236), (20 + 32767 / 303) as u8);
        assert_eq!(axis_byte(65535, 20, 236), 236);
    }

    #[test]
    fn gears_follow_the_h_pattern() {
        assert_eq!(gear_bits(0), 0);
        assert_eq!(gear_bits(1), 0xA0);
        assert_eq!(gear_bits(2), 0x60);
        assert_eq!(gear_bits(3), 0x80);
        assert_eq!(gear_bits(4), 0x40);
        assert_eq!(gear_bits(5), 0x90);
        assert_eq!(gear_bits(6), 0x50);
    }

    #[test]
    fn controls_land_in_the_documented_bits() {
        let c = Controls {
            wheel: 1.0,
            gas: 1.0,
            brake: 0.0,
            gear: 3,
            view: true,
            intrude: true,
            card: true,
            test: true,
            service: true,
            menu_up: true,
            menu_down: false,
            enter: true,
        };
        let s = c.to_snapshot(Cabinet::Wmmt5, [0; 4], 1);
        assert_eq!(s.special, 0x80);
        assert_eq!(
            s.players[0],
            [0x40 | 0x20 | 0x02, 0x80 | 0x02 | 0x01, 0x04, 0]
        );
        assert_eq!(s.analog[0], 255);
        assert_eq!(s.analog[2], 255);
        assert_eq!(s.analog[4], 0);
        let six = c.to_snapshot(Cabinet::Wmmt6RR, [0; 4], 1);
        assert_eq!(six.analog[0], 236);
    }

    #[test]
    fn shifter_stays_between_neutral_and_sixth() {
        let mut s = Shifter::default();
        assert_eq!(s.gear(), 1);
        s.down();
        s.down();
        assert_eq!(s.gear(), 0);
        for _ in 0..10 {
            s.up();
        }
        assert_eq!(s.gear(), 6);
    }
}
