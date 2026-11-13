//! The cabinet's buttons, the default pad layout, and the step from the
//! SDK's device-independent controls to the I/O board's controls.

use rea_launcher::wheel::{Action, Control, Controls as Read, PadInput, PadMap};
use wangan_jvs::controls::{Controls, Shifter};

/// Every button a Maximum Tune cabinet has (besides the pedals and wheel),
/// plus the sequential shift a pad or paddles use instead of the H lever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Button {
    GearUp,
    GearDown,
    View,
    Intrude,
    Card,
    Test,
    Service,
    MenuUp,
    MenuDown,
    Enter,
    /// A coin (6RR plays on credits).
    Coin,
}

impl Control for Button {
    const ALL: &'static [Self] = &[
        Button::GearUp,
        Button::GearDown,
        Button::View,
        Button::Intrude,
        Button::Card,
        Button::Test,
        Button::Service,
        Button::MenuUp,
        Button::MenuDown,
        Button::Enter,
        Button::Coin,
    ];

    fn key(self) -> &'static str {
        match self {
            Button::GearUp => "gear_up",
            Button::GearDown => "gear_down",
            Button::View => "view",
            Button::Intrude => "intrude",
            Button::Card => "card",
            Button::Test => "test",
            Button::Service => "service",
            Button::MenuUp => "menu_up",
            Button::MenuDown => "menu_down",
            Button::Enter => "enter",
            Button::Coin => "coin",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Button::GearUp => "Shift up",
            Button::GearDown => "Shift down",
            Button::View => "View change",
            Button::Intrude => "Intrude (accept a challenger)",
            Button::Card => "Insert card",
            Button::Test => "Operator test menu",
            Button::Service => "Service",
            Button::MenuUp => "Test menu up",
            Button::MenuDown => "Test menu down",
            Button::Enter => "Test menu enter",
            Button::Coin => "Insert coin",
        }
    }

    fn bit(self) -> u32 {
        1 << Self::ALL.iter().position(|&b| b == self).expect("in ALL")
    }
}

/// The pad layout a player gets before changing anything: triggers for the
/// pedals, the left stick steers (the SDK's pad reader), shoulders shift.
pub fn default_pad_map() -> PadMap<Button> {
    PadMap::with(&[
        (Action::Accelerate, &[PadInput::RightTrigger]),
        (Action::Brake, &[PadInput::LeftTrigger]),
        (Action::Press(Button::GearUp), &[PadInput::RightShoulder]),
        (Action::Press(Button::GearDown), &[PadInput::LeftShoulder]),
        (Action::Press(Button::View), &[PadInput::Y]),
        (Action::Press(Button::Intrude), &[PadInput::X]),
        (Action::Press(Button::Card), &[PadInput::Back]),
        (Action::Press(Button::Enter), &[PadInput::A]),
        (Action::Press(Button::Coin), &[PadInput::Start]),
    ])
}

/// Turns one frame of SDK controls into the board's controls, keeping the
/// gear between frames: a shift happens on the press, not while held.
#[derive(Debug, Default, Clone)]
pub struct Translator {
    shifter: Shifter,
    last: u32,
    coins: u16,
}

impl Translator {
    pub fn gear(&self) -> u8 {
        self.shifter.gear()
    }

    /// Coins inserted since the game started (slot 1).
    pub fn coins(&self) -> u16 {
        self.coins
    }

    pub fn step(&mut self, r: &Read) -> Controls {
        let pressed = |b: Button| r.buttons & b.bit() != 0;
        let edge = |b: Button| pressed(b) && self.last & b.bit() == 0;
        if edge(Button::GearUp) {
            self.shifter.up();
        }
        if edge(Button::GearDown) {
            self.shifter.down();
        }
        if edge(Button::Coin) {
            self.coins = self.coins.wrapping_add(1);
        }
        self.last = r.buttons;
        Controls {
            wheel: r.steer,
            gas: r.accel,
            brake: r.brake,
            gear: self.shifter.gear(),
            view: pressed(Button::View),
            intrude: pressed(Button::Intrude),
            card: pressed(Button::Card),
            test: pressed(Button::Test),
            service: pressed(Button::Service),
            menu_up: pressed(Button::MenuUp),
            menu_down: pressed(Button::MenuDown),
            enter: pressed(Button::Enter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher::wheel::{PadSettings, PadState, read_pad};

    #[test]
    fn bits_are_distinct() {
        let mut seen = 0u32;
        for b in Button::ALL {
            assert_eq!(seen & b.bit(), 0);
            seen |= b.bit();
        }
    }

    #[test]
    fn a_coin_counts_once_per_press() {
        let mut t = Translator::default();
        let coin = Read {
            buttons: Button::Coin.bit(),
            ..Read::default()
        };
        t.step(&coin);
        t.step(&coin);
        t.step(&Read::default());
        t.step(&coin);
        assert_eq!(t.coins(), 2);
    }

    #[test]
    fn shifting_happens_on_the_press_only() {
        let mut t = Translator::default();
        let up = Read {
            buttons: Button::GearUp.bit(),
            ..Read::default()
        };
        let none = Read::default();
        t.step(&up);
        t.step(&up); // held: no second shift
        assert_eq!(t.gear(), 2);
        t.step(&none);
        t.step(&up);
        assert_eq!(t.gear(), 3);
    }

    #[test]
    fn a_pad_drives_wheel_pedals_and_view_through_the_default_map() {
        let settings = PadSettings::new(default_pad_map());
        let pad = PadState {
            left_x: 32767,
            right_trigger: 32767,
            y: true,
            ..PadState::default()
        };
        let read = read_pad(&settings, &pad, None);
        let c = Translator::default().step(&read);
        assert!(c.wheel > 0.9, "{c:?}");
        assert!(c.gas > 0.9);
        assert!(c.view);
        assert!(!c.intrude);
    }
}
