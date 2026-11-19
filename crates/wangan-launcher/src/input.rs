//! What this launcher hands the SDK's input thread
//! (`rea_launcher_native::feeder`): the cabinet's controls as the game's
//! input block ([`Block`], a [`Wire`]) and the block file itself
//! ([`BlockSink`], the feed's sink).
//!
//! The block is a 64-byte file both sides map (`wangan_jvs::shared`): the
//! I/O board under Wine and the N2 loader read it, the launcher writes it
//! in place. The feeder's wire is text, so each frame travels as one line
//! of hex that the sink turns back into bytes at the block's start. The
//! sequence number changes every frame, so the feeder writes every tick
//! (4 ms), and the board sees a live block.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use rea_launcher::hub::HubPaths;
use rea_launcher::hub::bindings::Bindings;
use rea_launcher_native::feeder::{Frame, InputService, Wire};
use wangan_core::input::{Button, Translator, default_pad_map};
use wangan_jvs::controls::Cabinet;
use wangan_jvs::shared::QUIT;

/// The player's pad layout: the cabinet's, with the changes saved in the
/// data folder's `controls.cfg`. A file that can't be read leaves the
/// cabinet's layout, and `warn` hears why.
pub fn bindings(paths: &HubPaths, warn: impl FnOnce(&str)) -> Bindings<Button> {
    let file = paths.controls_file();
    Bindings::load(&file, default_pad_map()).unwrap_or_else(|e| {
        warn(&format!(
            "{}: {e}; the default pad layout is used",
            file.display()
        ));
        Bindings::new(default_pad_map())
    })
}

/// The input thread with the player's pad layout `bindings`; a wheel's
/// paddles shift.
pub fn service(bindings: &Bindings<Button>) -> InputService {
    InputService::start(
        bindings.map().clone(),
        Some((Button::GearUp, Button::GearDown)),
    )
}

/// A game's input block: the cabinet's controls (the gear kept between
/// frames, coins counted), all but the quit byte, which is the session's.
pub struct Block {
    cabinet: Cabinet,
    translator: Translator,
    sequence: u32,
}

impl Block {
    pub fn boxed(cabinet: Cabinet) -> Box<dyn Wire> {
        Box::new(Block {
            cabinet,
            translator: Translator::default(),
            sequence: 0,
        })
    }
}

impl Wire for Block {
    fn encode(&mut self, frame: &Frame) -> String {
        let controls = self.translator.step(&frame.controls);
        self.sequence = self.sequence.wrapping_add(1);
        let coins = [self.translator.coins(), 0, 0, 0];
        let bytes = controls
            .to_snapshot(self.cabinet, coins, self.sequence)
            .encode();
        let mut line: String = bytes[..QUIT].iter().map(|b| format!("{b:02x}")).collect();
        line.push('\n');
        line
    }
}

/// The block file as the feed's sink: each line [`Block`] writes, put in
/// place at the start of the file (the board or the loader maps it).
pub struct BlockSink {
    file: File,
    line: Vec<u8>,
}

impl BlockSink {
    pub fn open(block: &Path) -> io::Result<BlockSink> {
        Ok(BlockSink {
            file: OpenOptions::new().write(true).open(block)?,
            line: Vec::new(),
        })
    }

    fn put(&self, hex: &[u8]) -> io::Result<()> {
        let bytes = decode(hex).ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        write_front(&self.file, &bytes)
    }
}

impl Write for BlockSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            if b == b'\n' {
                let line = std::mem::take(&mut self.line);
                self.put(&line)?;
            } else {
                self.line.push(b);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The bytes of a hex line; `None` when it isn't one.
fn decode(hex: &[u8]) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    hex.chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// Writes `b` at the start of the block, in place.
fn write_front(f: &File, b: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        f.write_all_at(b, 0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut done = 0;
        while done < b.len() {
            match f.seek_write(&b[done..], done as u64)? {
                0 => return Err(io::ErrorKind::WriteZero.into()),
                n => done += n,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher::wheel::{Control, Controls};
    use wangan_jvs::shared::Snapshot;

    fn frame(buttons: u32, steer: f32) -> Frame {
        Frame {
            controls: Controls {
                steer,
                buttons,
                ..Controls::default()
            },
            ..Frame::default()
        }
    }

    #[test]
    fn the_block_is_written_in_place_and_the_quit_byte_left_alone() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("input.block");
        let fresh = Snapshot {
            quit: true,
            ..Snapshot::default()
        };
        std::fs::write(&path, fresh.encode()).unwrap();
        let mut wire = Block::boxed(Cabinet::Wmmt5);
        let mut sink = BlockSink::open(&path).unwrap();
        // a line may reach the sink in pieces
        let line = wire.encode(&frame(Button::GearUp.bit(), -1.0));
        let (a, b) = line.as_bytes().split_at(7);
        sink.write_all(a).unwrap();
        sink.write_all(b).unwrap();
        let read = Snapshot::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(read.sequence, 1);
        assert!(read.quit, "the quit byte is the session's");
        let line = wire.encode(&frame(Button::Coin.bit(), 0.0));
        sink.write_all(line.as_bytes()).unwrap();
        let read = Snapshot::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((read.sequence, read.coins[0]), (2, 1));
        assert_eq!(
            std::fs::read(&path).unwrap().len(),
            wangan_jvs::shared::SIZE
        );
    }

    #[test]
    fn every_frame_is_a_new_line_so_the_board_sees_it_live() {
        let mut wire = Block::boxed(Cabinet::Wmmt6RR);
        let still = frame(0, 0.0);
        assert_ne!(wire.encode(&still), wire.encode(&still));
    }

    #[test]
    fn a_line_that_is_no_block_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("input.block");
        std::fs::write(&path, [0u8; 64]).unwrap();
        let mut sink = BlockSink::open(&path).unwrap();
        assert!(sink.write_all(b"zz\n").is_err());
        assert!(sink.write_all(b"abc\n").is_err());
    }
}
