//! The JVS I/O board that the Wangan Midnight Maximum Tune 5–6RR cabinets
//! talk to, rebuilt for the launcher.
//!
//! On a cabinet the game reads its wheel, pedals and buttons from a Namco
//! JVS I/O board on a serial port. OpenParrot redirects that port to the
//! named pipe `\\.\pipe\teknoparrot_jvs` and expects someone to answer;
//! TeknoParrotUI used to. This crate holds everything about that answer
//! that can be tested without Windows:
//!
//! - [`frame`]: the serial framing (sync byte, escapes, checksum);
//! - [`board`]: the replies, a port of TeknoParrotUI's
//!   `JvsPackageEmulator` in its Namco mode (GPL-3.0, see `THIRD_PARTY.md`);
//! - [`controls`]: what the player does (steer, pedals, gears, buttons)
//!   turned into the board's switch and analog bytes, per game;
//! - [`shared`]: the 64-byte block through which the launcher, a native
//!   program, hands those bytes to the board running under Wine.

pub mod board;
pub mod chihiro;
pub mod controls;
pub mod frame;
pub mod shared;
