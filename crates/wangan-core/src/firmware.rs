//! Firmware the player supplies for the boards whose emulators need it:
//! the Namco System 246 BIOS (Wangan Midnight R) and the Sega Chihiro
//! media-board files (Maximum Tune). The launcher never downloads them; it
//! looks through a folder the player chooses (and the zip files in it),
//! keeps the files whose size and SHA-1 match MAME's records, and copies
//! them under their MAME names into its own data folder.
//!
//! Sizes, CRC32 and SHA-1 are MAME's (`namcops2.cpp` 962-963 for
//! `r27v1602f.7d`, `chihiro.cpp` 1977-1988 for the media board, MAME
//! c1335fd); MAME publishes no SHA-256 for them.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};

#[derive(Debug, PartialEq, Eq)]
pub struct FirmwareFile {
    /// MAME's file name; the name it is stored under.
    pub name: &'static str,
    pub size: u64,
    pub crc32: u32,
    pub sha1: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FirmwareSet {
    /// Folder under the launcher's `firmware/`.
    pub id: &'static str,
    /// What the player is asked for.
    pub title: &'static str,
    pub files: &'static [FirmwareFile],
}

pub static SYS246: FirmwareSet = FirmwareSet {
    id: "sys246",
    title: "Namco System 246 BIOS",
    files: &[FirmwareFile {
        name: "r27v1602f.7d",
        size: 2_097_152,
        crc32: 0x2b2e41a2,
        sha1: "f0a74bbcaf801f3fd0b7002ebd0118564aae3528",
    }],
};

pub static CHIHIRO: FirmwareSet = FirmwareSet {
    id: "chihiro",
    title: "Sega Chihiro media-board files",
    files: &[
        FirmwareFile {
            name: "fpr21042_m29w160et.bin",
            size: 2_097_152,
            crc32: 0xa4fcab0b,
            sha1: "a13cf9c5cdfe8605d82150b7573652f419b30197",
        },
        FirmwareFile {
            name: "ic10_g24lc64.bin",
            size: 8_192,
            crc32: 0xcfc5e06f,
            sha1: "3ababd4334d8d57abb22dd98bd2d347df39648d9",
        },
        FirmwareFile {
            name: "ic11_24lc024.bin",
            size: 128,
            crc32: 0x8dc8374e,
            sha1: "cc03a0650bfac4bf6cb66e414bbef121cba53efe",
        },
        FirmwareFile {
            name: "pc20_g24lc64.bin",
            size: 8_192,
            crc32: 0x7742ab62,
            sha1: "82dad6e2a75bab4a4840dc6939462f1fb9b95101",
        },
    ],
};

/// A stored file's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Present,
    Missing,
    /// There, but not the expected file (size or SHA-1 differ).
    Wrong,
}

fn sha1_hex(bytes: &[u8]) -> String {
    let d = Sha1::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

impl FirmwareFile {
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.size && sha1_hex(bytes) == self.sha1
    }
}

impl FirmwareSet {
    /// Where the set is kept: `<data>/firmware/<id>/`.
    pub fn dir(&self, data: &Path) -> PathBuf {
        data.join("firmware").join(self.id)
    }

    /// Each file's state in `dir`.
    pub fn check(&self, dir: &Path) -> Vec<(&'static FirmwareFile, FileState)> {
        self.files
            .iter()
            .map(|f| {
                let state = match fs::read(dir.join(f.name)) {
                    Ok(b) if f.matches(&b) => FileState::Present,
                    Ok(_) => FileState::Wrong,
                    Err(_) => FileState::Missing,
                };
                (f, state)
            })
            .collect()
    }

    pub fn complete(&self, dir: &Path) -> bool {
        self.check(dir)
            .iter()
            .all(|(_, s)| *s == FileState::Present)
    }

    fn wanted(&self, size: u64) -> bool {
        self.files.iter().any(|f| f.size == size)
    }
}

/// What a search found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Newly stored, by MAME name.
    pub added: Vec<&'static str>,
    /// Still not found anywhere (and not stored before).
    pub missing: Vec<&'static str>,
    /// Files and zip entries looked at.
    pub looked_at: usize,
}

/// How deep the search goes below the chosen folder.
const MAX_DEPTH: usize = 3;

/// Look through `src` (a folder, a zip, or one file) for the set's files
/// and store each match in `dest` under its MAME name. Files already
/// stored are kept. Only files whose size is one the set needs are read.
pub fn import(set: &FirmwareSet, src: &Path, dest: &Path) -> io::Result<Report> {
    let mut report = Report::default();
    let mut have: Vec<bool> = set
        .check(dest)
        .iter()
        .map(|(_, s)| *s == FileState::Present)
        .collect();
    let mut found = |bytes: &[u8], report: &mut Report| -> io::Result<()> {
        for (i, f) in set.files.iter().enumerate() {
            if !have[i] && f.matches(bytes) {
                fs::create_dir_all(dest)?;
                let tmp = dest.join(format!(".{}.part", f.name));
                fs::write(&tmp, bytes)?;
                fs::rename(&tmp, dest.join(f.name))?;
                have[i] = true;
                report.added.push(f.name);
            }
        }
        Ok(())
    };
    walk(set, src, 0, &mut report, &mut found)?;
    report.missing = set
        .files
        .iter()
        .zip(&have)
        .filter(|(_, h)| !**h)
        .map(|(f, _)| f.name)
        .collect();
    Ok(report)
}

fn is_zip(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == *b"PK\x03\x04"
}

fn walk(
    set: &FirmwareSet,
    path: &Path,
    depth: usize,
    report: &mut Report,
    found: &mut dyn FnMut(&[u8], &mut Report) -> io::Result<()>,
) -> io::Result<()> {
    let meta = fs::metadata(path)?;
    if meta.is_dir() {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let mut entries: Vec<_> = fs::read_dir(path)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            // unreadable entries are skipped, not fatal
            let _ = walk(set, &e.path(), depth + 1, report, found);
        }
        return Ok(());
    }
    if !meta.is_file() {
        return Ok(());
    }
    if set.wanted(meta.len()) {
        report.looked_at += 1;
        found(&fs::read(path)?, report)?;
    }
    if is_zip(path) {
        let mut zip = zip::ZipArchive::new(fs::File::open(path)?).map_err(io::Error::other)?;
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else {
                continue;
            };
            if !entry.is_file() || !set.wanted(entry.size()) {
                continue;
            }
            report.looked_at += 1;
            // read no more than the size the entry declares: a hostile
            // header cannot make this hold more than one wanted file
            let size = entry.size();
            let mut bytes = Vec::with_capacity(size as usize);
            if (&mut entry).take(size).read_to_end(&mut bytes).is_ok() {
                found(&bytes, report)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A set made from known bytes, so the tests need no real firmware.
    fn test_set() -> &'static FirmwareSet {
        let a = vec![1u8; 64];
        let b = vec![2u8; 32];
        let files = vec![
            FirmwareFile {
                name: "a.bin",
                size: 64,
                crc32: 0,
                sha1: Box::leak(sha1_hex(&a).into_boxed_str()),
            },
            FirmwareFile {
                name: "b.bin",
                size: 32,
                crc32: 0,
                sha1: Box::leak(sha1_hex(&b).into_boxed_str()),
            },
        ];
        Box::leak(Box::new(FirmwareSet {
            id: "t",
            title: "test",
            files: Box::leak(files.into_boxed_slice()),
        }))
    }

    #[test]
    fn finds_files_by_content_under_any_name_and_in_zips() {
        let set = test_set();
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        fs::create_dir_all(src.path().join("x/y")).unwrap();
        fs::write(src.path().join("x/y/renamed.rom"), vec![1u8; 64]).unwrap();
        // same size, wrong content: not taken
        fs::write(src.path().join("decoy.bin"), vec![9u8; 32]).unwrap();
        let mut z = zip::ZipWriter::new(fs::File::create(src.path().join("pack.zip")).unwrap());
        z.start_file("inner/b.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(&[2u8; 32]).unwrap();
        z.finish().unwrap();

        assert!(!set.complete(dest.path()));
        let r = import(set, src.path(), dest.path()).unwrap();
        assert_eq!(r.added, vec!["b.bin", "a.bin"]);
        assert!(r.missing.is_empty());
        assert!(set.complete(dest.path()));
        assert_eq!(fs::read(dest.path().join("a.bin")).unwrap(), vec![1u8; 64]);
    }

    #[test]
    fn reports_what_is_still_missing_and_keeps_what_was_stored() {
        let set = test_set();
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        fs::write(src.path().join("a"), vec![1u8; 64]).unwrap();
        let r = import(set, src.path(), dest.path()).unwrap();
        assert_eq!((r.added, r.missing), (vec!["a.bin"], vec!["b.bin"]));

        let empty = tempfile::tempdir().unwrap();
        let r = import(set, empty.path(), dest.path()).unwrap();
        assert!(r.added.is_empty(), "already stored: not added again");
        assert_eq!(r.missing, vec!["b.bin"]);
        let states: Vec<_> = set.check(dest.path()).into_iter().map(|(_, s)| s).collect();
        assert_eq!(states, vec![FileState::Present, FileState::Missing]);

        fs::write(dest.path().join("b.bin"), vec![0u8; 32]).unwrap();
        assert_eq!(set.check(dest.path())[1].1, FileState::Wrong);
    }

    #[test]
    fn mame_records_are_well_formed() {
        for set in [&SYS246, &CHIHIRO] {
            for f in set.files {
                assert_eq!(f.sha1.len(), 40, "{}", f.name);
                assert!(f.sha1.bytes().all(|c| c.is_ascii_hexdigit()));
            }
        }
    }
}
