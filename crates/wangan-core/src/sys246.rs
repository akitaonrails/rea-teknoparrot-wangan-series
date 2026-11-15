//! Wangan Midnight R (Namco System 246) in pcsx2x6: the files the emulator
//! reads, made from the player's dump and board files.
//!
//! pcsx2x6 starts an arcade game from an `.acgame` file; with only the game
//! id set it looks for `<id>/<id>.chd`, `<id>/boot.elf` (the Proverb boot
//! loader, a pcsx2x6 project component) and the dongle as the memory card
//! `memcards/<id>.ps2` (pcsx2x6 `VMManager.cpp` 1345-1490). The BIOS is
//! named in `PCSX2.ini`, because the 2 MiB System 246 BIOS is below the
//! size pcsx2x6's folder scan accepts. Seen in the real game: a race driven
//! (docs/RUNTIME_NOTES.md).

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// pcsx2x6's id for Wangan Midnight R.
pub const GAME_ID: &str = "NM00005";

/// The MAME dongle files in `wanganmr.zip`: 16,384 NAND pages of 512
/// bytes, and 16 spare bytes per page.
pub const DONGLE_DATA: &str = "wmr1vera.ic002";
pub const DONGLE_SPARE: &str = "wmr1vera_spr.ic002";
const PAGES: usize = 16_384;
const PAGE: usize = 512;
const SPARE: usize = 16;
/// A raw 8 MB card with its spare area, as pcsx2x6 opens a `.ps2` file.
pub const CARD_SIZE: usize = PAGES * (PAGE + SPARE);

/// pcsx2x6's data folder in a runtime (it runs with `XDG_CONFIG_HOME` set
/// to `<runtime>/config`): `bios/`, `memcards/`, `inis/`.
pub fn data_dir(rt: &Path) -> PathBuf {
    rt.join("config").join("PCSX2x6")
}

/// The CHD in the dump, under its MAME name.
pub const CHD: &str = "wanganmr/wmr1-a.chd";

/// The raw memory card: each page followed by its spare bytes.
pub fn interleave(data: &[u8], spare: &[u8]) -> Option<Vec<u8>> {
    if data.len() != PAGES * PAGE || spare.len() != PAGES * SPARE {
        return None;
    }
    let mut out = Vec::with_capacity(CARD_SIZE);
    for (p, s) in data
        .as_chunks::<PAGE>()
        .0
        .iter()
        .zip(spare.as_chunks::<SPARE>().0)
    {
        out.extend_from_slice(p);
        out.extend_from_slice(s);
    }
    Some(out)
}

/// The dongle card from the dump's `wanganmr.zip`.
pub fn dongle_from_zip(zip: &Path) -> io::Result<Vec<u8>> {
    let mut z = zip::ZipArchive::new(fs::File::open(zip)?).map_err(io::Error::other)?;
    let mut read = |name: &str| -> io::Result<Vec<u8>> {
        let mut e = z.by_name(name).map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{name} is not in {}", zip.display()),
            )
        })?;
        let mut b = Vec::new();
        e.read_to_end(&mut b)?;
        Ok(b)
    };
    let data = read(DONGLE_DATA)?;
    let spare = read(DONGLE_SPARE)?;
    interleave(&data, &spare).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "the dongle files in wanganmr.zip have the wrong size",
        )
    })
}

/// The `.acgame` file: everything else is pcsx2x6's default.
pub fn acgame() -> String {
    format!("[game]\ngameid={GAME_ID}\n")
}

/// pcsx2x6's upscale factor for a window height: the PS2 draws about 360
/// to 480 lines, so 720p → 2, 1080p → 3, 1440p → 4, 2160p → 6
/// (TeknoParrot's mapping).
pub fn upscale_for(height: u32) -> u32 {
    ((height + 180) / 360).clamp(1, 8)
}

/// One action bound to a key and a pad control (SDL names; any pad, as
/// pcsx2x6 numbers them from 0).
const JVS_BINDINGS: &[(&str, &[&str])] = &[
    ("Coin1", &["Keyboard/5", "SDL-0/Back"]),
    ("P1_Start", &["Keyboard/1", "SDL-0/Start"]),
    ("P1_Up", &["Keyboard/W", "SDL-0/DPadUp"]),
    ("P1_Down", &["Keyboard/S", "SDL-0/DPadDown"]),
    ("P1_Service", &["Keyboard/F2"]),
    ("ToggleTestMode", &["Keyboard/F1"]),
    ("Racing_View", &["Keyboard/V", "SDL-0/Y"]),
    ("Racing_ShiftUp", &["Keyboard/E", "SDL-0/RightShoulder"]),
    ("Racing_ShiftDown", &["Keyboard/Q", "SDL-0/LeftShoulder"]),
    ("SteerLeft", &["Keyboard/Left", "SDL-0/-LeftX"]),
    ("SteerRight", &["Keyboard/Right", "SDL-0/+LeftX"]),
    ("Gas", &["Keyboard/Up", "SDL-0/+RightTrigger"]),
    ("Brake", &["Keyboard/Down", "SDL-0/+LeftTrigger"]),
];

/// `PCSX2.ini`, written before every start. `null_audio` is for test runs,
/// where no sound server is reachable (pcsx2x6 would show an error box).
pub fn pcsx2_ini(upscale: u32, null_audio: bool) -> String {
    let mut s = String::from(
        "; Written by the launcher before every start.\n\
         [UI]\nSettingsVersion = 1\nSetupWizardIncomplete = false\nConfirmShutdown = false\n\
         StartFullscreen = true\nHideMouseCursor = true\n\
         [Filenames]\nBIOS = r27v1602f.7d\n",
    );
    s += &format!("[EmuCore/GS]\nupscale_multiplier = {upscale}\n");
    if null_audio {
        s += "[SPU2/Output]\nBackend = Null\n";
    }
    s += "[Hotkeys]\nShutdownVM = Keyboard/Escape\n[JVS]\n";
    for (action, keys) in JVS_BINDINGS {
        for k in *keys {
            s += &format!("{action} = {k}\n");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn card_is_each_page_then_its_spare_bytes() {
        let data: Vec<u8> = (0..PAGES * PAGE).map(|i| (i / PAGE) as u8).collect();
        let spare = vec![0xEE; PAGES * SPARE];
        let card = interleave(&data, &spare).unwrap();
        assert_eq!(card.len(), 8_650_752);
        assert_eq!(&card[..PAGE], &data[..PAGE]);
        assert_eq!(&card[PAGE..PAGE + SPARE], &[0xEE; SPARE]);
        assert_eq!(card[PAGE + SPARE], 1, "second page follows the spare");
        assert!(interleave(&data[1..], &spare).is_none());
    }

    #[test]
    fn dongle_is_read_from_the_mame_zip() {
        let dir = tempfile::tempdir().unwrap();
        let zp = dir.path().join("wanganmr.zip");
        let mut z = zip::ZipWriter::new(fs::File::create(&zp).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file(DONGLE_DATA, o).unwrap();
        z.write_all(&vec![1; PAGES * PAGE]).unwrap();
        z.start_file(DONGLE_SPARE, o).unwrap();
        z.write_all(&vec![2; PAGES * SPARE]).unwrap();
        z.finish().unwrap();
        let card = dongle_from_zip(&zp).unwrap();
        assert_eq!(card.len(), CARD_SIZE);
        assert_eq!(card[PAGE], 2);

        let bad = dir.path().join("other.zip");
        zip::ZipWriter::new(fs::File::create(&bad).unwrap())
            .finish()
            .unwrap();
        assert!(dongle_from_zip(&bad).is_err());
    }

    #[test]
    fn upscale_follows_the_window_height() {
        let f: Vec<u32> = [480, 720, 1080, 1440, 2160, 9000]
            .iter()
            .map(|h| upscale_for(*h))
            .collect();
        assert_eq!(f, vec![1, 2, 3, 4, 6, 8]);
    }

    #[test]
    fn ini_names_the_bios_and_binds_keys_and_pad() {
        let ini = pcsx2_ini(6, false);
        assert!(ini.contains("BIOS = r27v1602f.7d\n"));
        assert!(ini.contains("upscale_multiplier = 6\n"));
        assert!(ini.contains("Gas = Keyboard/Up\nGas = SDL-0/+RightTrigger\n"));
        assert!(!ini.contains("Backend = Null"));
        assert!(pcsx2_ini(1, true).contains("[SPU2/Output]\nBackend = Null\n"));
    }
}
