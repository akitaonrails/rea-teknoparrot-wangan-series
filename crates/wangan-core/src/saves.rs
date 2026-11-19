//! Per-game card and save mirroring between `<data>/saves/<game>/` (clean,
//! backup-friendly, player-owned) and the game's private runtime folder
//! (`rea_launcher::saves::Mirror`).
//!
//! Each route keeps only its real card/save/cabinet files in `saves/<game>/`,
//! never DLLs, symlinks, logs, or launcher config:
//!
//! - **Namco N2** (MT3, MT3DX+): magnetic card files (`card.bin`, `card*.bin`)
//!   in the runtime root.
//! - **Namco ES3B** (MT5, MT5DX+, MT6, MT6R, MT6RR): `card.ini` (seeded once
//!   with a stable Banapass card ID if absent), OpenParrot's local save files
//!   (`openprogress.sav`, `mileage.sav`, `stars.bin`, `Setting*.bin`,
//!   `card_*.bin`), cabinet settings/backup files (`F*.lua.gz`, `G*.lua.gz`,
//!   `BK_*.dat`), and the `TP/` and `TeknoParrot/` save folders.
//! - **Namco System 246** (Wangan Midnight R): `memcards/NM00005.ps2` and
//!   BIOS NVRAM (`bios/*.nvm`, `bios/*.mec`) — never the BIOS ROM itself.
//!
//! Mirroring uses recorded nanosecond state (`FirstRun::SavesWin`): restoring
//! an older backup into `saves/<game>/` wins over the runtime copy, and a
//! crash before exit is recovered on the next launch. Nothing is ever deleted.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rea_launcher::fsx;
use rea_launcher::paths::find_ignoring_case;
use rea_launcher::saves::{FirstRun, Mirror, Moved};

use crate::catalog::{Game, Route};
use crate::sys246;

/// Default `card.ini` for OpenBanapass (`bngrw.dll`), seeded once when
/// neither the saves folder nor the runtime already has one.
pub const DEFAULT_CARD_INI: &str = "[card]\n\
chipId=30303030303030303030303030303031\n\
accessCode=30000000000000000001\n";

/// The player's backup-friendly saves folder for `game` (`<data>/saves/<slug>`).
pub fn dir(data: &Path, slug: &str) -> PathBuf {
    data.join("saves").join(slug)
}

/// What was synced before a launch.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub to_runtime: Vec<String>,
    pub to_saves: Vec<String>,
}

impl Report {
    pub fn is_empty(&self) -> bool {
        self.to_runtime.is_empty() && self.to_saves.is_empty()
    }

    fn extend(&mut self, prefix: &str, m: Moved) {
        let tag = |rel: String| {
            if prefix.is_empty() {
                rel
            } else {
                format!("{prefix}/{rel}")
            }
        };
        self.to_runtime.extend(m.to_runtime.into_iter().map(tag));
        self.to_saves.extend(m.to_saves.into_iter().map(tag));
    }
}

/// Brings `<data>/saves/<slug>/` and `rt` in step before the game starts.
pub fn sync_before(game: &Game, rt: &Path, saves: &Path) -> io::Result<Report> {
    let mut report = Report::default();
    match game.route {
        Route::NativeN2 => {
            let state = rt.join(".saves-state");
            let m = Mirror::folder(rt, saves, &state, FirstRun::SavesWin)
                .scan(&[""])
                .only(&is_n2_save)
                .before()?;
            report.extend("", m);
        }
        Route::Es3b => {
            seed_card_ini(rt, saves)?;
            let state_root = rt.join(".saves-state-root");
            let m = Mirror::folder(rt, saves, &state_root, FirstRun::SavesWin)
                .scan(&[""])
                .only(&is_es3b_root_save)
                .before()?;
            report.extend("", m);
            for sub in ["TP", "TeknoParrot"] {
                let rt_sub = rt.join(sub);
                let sv_sub = saves.join(sub);
                if has_any_file(&rt_sub) || has_any_file(&sv_sub) {
                    let state = rt.join(format!(".saves-state-{}", sub.to_ascii_lowercase()));
                    let m =
                        Mirror::folder(&rt_sub, &sv_sub, &state, FirstRun::SavesWin).before()?;
                    report.extend(sub, m);
                }
            }
        }
        Route::System246 => {
            let data = sys246::data_dir(rt);
            let mc_rt = data.join("memcards");
            let mc_sv = saves.join("memcards");
            if has_any_file(&mc_rt) || has_any_file(&mc_sv) {
                let state = rt.join(".saves-state-memcards");
                let m = Mirror::folder(&mc_rt, &mc_sv, &state, FirstRun::SavesWin)
                    .scan(&[""])
                    .only(&is_sys246_card)
                    .before()?;
                report.extend("memcards", m);
            }
            let bios_rt = data.join("bios");
            let bios_sv = saves.join("bios");
            if has_any_file(&bios_rt) || has_any_file(&bios_sv) {
                let state = rt.join(".saves-state-bios");
                let m = Mirror::folder(&bios_rt, &bios_sv, &state, FirstRun::SavesWin)
                    .scan(&[""])
                    .only(&is_sys246_nvram)
                    .before()?;
                report.extend("bios", m);
            }
        }
        Route::Chihiro => {}
    }
    Ok(report)
}

/// Copies any card/save files the game created or updated in `rt` back to
/// `<data>/saves/<slug>/` after the game exits.
pub fn sync_after(game: &Game, rt: &Path, saves: &Path) -> io::Result<Vec<String>> {
    let mut copied = Vec::new();
    match game.route {
        Route::NativeN2 => {
            let state = rt.join(".saves-state");
            copied.extend(
                Mirror::folder(rt, saves, &state, FirstRun::SavesWin)
                    .scan(&[""])
                    .only(&is_n2_save)
                    .after()?,
            );
        }
        Route::Es3b => {
            let state_root = rt.join(".saves-state-root");
            copied.extend(
                Mirror::folder(rt, saves, &state_root, FirstRun::SavesWin)
                    .scan(&[""])
                    .only(&is_es3b_root_save)
                    .after()?,
            );
            for sub in ["TP", "TeknoParrot"] {
                let rt_sub = rt.join(sub);
                let sv_sub = saves.join(sub);
                if has_any_file(&rt_sub) || has_any_file(&sv_sub) {
                    let state = rt.join(format!(".saves-state-{}", sub.to_ascii_lowercase()));
                    let files =
                        Mirror::folder(&rt_sub, &sv_sub, &state, FirstRun::SavesWin).after()?;
                    copied.extend(files.into_iter().map(|r| format!("{sub}/{r}")));
                }
            }
        }
        Route::System246 => {
            let data = sys246::data_dir(rt);
            let mc_rt = data.join("memcards");
            let mc_sv = saves.join("memcards");
            if has_any_file(&mc_rt) || has_any_file(&mc_sv) {
                let state = rt.join(".saves-state-memcards");
                let files = Mirror::folder(&mc_rt, &mc_sv, &state, FirstRun::SavesWin)
                    .scan(&[""])
                    .only(&is_sys246_card)
                    .after()?;
                copied.extend(files.into_iter().map(|r| format!("memcards/{r}")));
            }
            let bios_rt = data.join("bios");
            let bios_sv = saves.join("bios");
            if has_any_file(&bios_rt) || has_any_file(&bios_sv) {
                let state = rt.join(".saves-state-bios");
                let files = Mirror::folder(&bios_rt, &bios_sv, &state, FirstRun::SavesWin)
                    .scan(&[""])
                    .only(&is_sys246_nvram)
                    .after()?;
                copied.extend(files.into_iter().map(|r| format!("bios/{r}")));
            }
        }
        Route::Chihiro => {}
    }
    Ok(copied)
}

/// Seeds `saves/card.ini` once when neither `saves` nor `rt` has one yet.
fn seed_card_ini(rt: &Path, saves: &Path) -> io::Result<()> {
    if find_ignoring_case(saves, "card.ini").is_some()
        || find_ignoring_case(rt, "card.ini").is_some()
    {
        return Ok(());
    }
    fsx::write_atomic(&saves.join("card.ini"), DEFAULT_CARD_INI)
}

/// Magnetic card files written by WanganArcadeLoader (`card.bin`, `card*.bin`).
pub fn is_n2_save(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower.starts_with("card") && lower.ends_with(".bin")
}

/// Player card, progress, and cabinet settings files in an ES3B runtime root.
pub fn is_es3b_root_save(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "card.ini"
            | "openprogress.sav"
            | "mileage.sav"
            | "stars.bin"
            | "setting1.bin"
            | "setting2.bin"
    ) || (lower.starts_with("card_") && lower.ends_with(".bin"))
        || ((lower.starts_with('f') || lower.starts_with('g')) && lower.ends_with(".lua.gz"))
        || (lower.starts_with("bk_") && lower.ends_with(".dat"))
}

/// System 246 dongle/memory card files (`*.ps2`).
pub fn is_sys246_card(rel: &str) -> bool {
    rel.to_ascii_lowercase().ends_with(".ps2")
}

/// System 246 BIOS NVRAM sidecar files written by pcsx2x6 (`*.nvm`, `*.mec`),
/// never the 2 MiB BIOS ROM itself.
pub fn is_sys246_nvram(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower.ends_with(".nvm") || lower.ends_with(".mec")
}

fn has_any_file(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() || (p.is_dir() && has_any_file(&p)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};
    use std::time::{Duration, SystemTime};

    fn put(path: &Path, bytes: &[u8], secs_ago: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(secs_ago))
            .unwrap();
    }

    #[test]
    fn n2_mirrors_only_card_files_and_honours_older_backups() {
        let t = tempfile::tempdir().unwrap();
        let rt = t.path().join("rt");
        let sv = dir(t.path(), "mt3dxp");
        fs::create_dir_all(&rt).unwrap();
        let g = game(GameId::Mt3dxp);

        // Runtime has non-save files plus a newly created card.bin
        put(&rt.join("main"), b"elf", 10);
        put(&rt.join("config.toml"), b"cfg", 10);
        put(&rt.join("card.bin"), b"gtr-r35", 10);

        let before = sync_before(g, &rt, &sv).unwrap();
        assert_eq!(before.to_saves, ["card.bin"]);
        assert_eq!(fs::read(sv.join("card.bin")).unwrap(), b"gtr-r35");
        assert!(!sv.join("main").exists(), "non-save files stay out");
        assert!(!sv.join("config.toml").exists());

        // Game updates the card during play
        put(&rt.join("card.bin"), b"gtr-r35-tuned", 0);
        assert_eq!(sync_after(g, &rt, &sv).unwrap(), ["card.bin"]);
        assert_eq!(fs::read(sv.join("card.bin")).unwrap(), b"gtr-r35-tuned");

        // Player restores an older backup into saves/mt3dxp/
        put(&sv.join("card.bin"), b"backup-rx7", 5000);
        let restored = sync_before(g, &rt, &sv).unwrap();
        assert_eq!(restored.to_runtime, ["card.bin"]);
        assert_eq!(fs::read(rt.join("card.bin")).unwrap(), b"backup-rx7");
    }

    #[test]
    fn es3b_seeds_card_ini_once_and_mirrors_saves_and_tp_folder() {
        let t = tempfile::tempdir().unwrap();
        let rt = t.path().join("rt");
        let sv = dir(t.path(), "mt5dxp");
        fs::create_dir_all(&rt).unwrap();
        let g = game(GameId::Mt5dxp);

        put(&rt.join("OpenParrot64.dll"), b"dll", 10);
        put(&rt.join("teknoparrot.ini"), b"ini", 10);

        let before = sync_before(g, &rt, &sv).unwrap();
        assert_eq!(before.to_runtime, ["card.ini"]);
        assert_eq!(
            fs::read_to_string(rt.join("card.ini")).unwrap(),
            DEFAULT_CARD_INI
        );
        assert!(!sv.join("OpenParrot64.dll").exists());
        assert!(!sv.join("teknoparrot.ini").exists());

        // Game writes progress, a car card, cabinet settings, and a TP/ file
        put(&rt.join("openprogress.sav"), b"prog", 0);
        put(&rt.join("card_abc_1.bin"), b"car1", 0);
        put(&rt.join("Fsetting.lua.gz"), b"gz", 0);
        put(&rt.join("TP/save.dat"), b"tp", 0);

        let after = sync_after(g, &rt, &sv).unwrap();
        for f in [
            "Fsetting.lua.gz",
            "card_abc_1.bin",
            "openprogress.sav",
            "TP/save.dat",
        ] {
            assert!(after.iter().any(|x| x == f), "missing {f} in {after:?}");
        }
        assert_eq!(fs::read(sv.join("TP/save.dat")).unwrap(), b"tp");

        // Player customizes card.ini: next sync_before never overwrites it
        put(&sv.join("card.ini"), b"[card]\nchipId=CUSTOM\n", 100);
        let next = sync_before(g, &rt, &sv).unwrap();
        assert_eq!(next.to_runtime, ["card.ini"]);
        assert_eq!(
            fs::read_to_string(rt.join("card.ini")).unwrap(),
            "[card]\nchipId=CUSTOM\n"
        );
    }

    #[test]
    fn sys246_mirrors_card_and_nvram_but_never_the_bios_rom() {
        let t = tempfile::tempdir().unwrap();
        let rt = t.path().join("rt");
        let sv = dir(t.path(), "wangan-r");
        let data = sys246::data_dir(&rt);
        let g = game(GameId::WanganR);

        put(&data.join("bios/r27v1602f.7d"), b"bios-rom", 10);
        put(&data.join("bios/r27v1602f.nvm"), b"nvram-1", 10);
        put(&data.join("memcards/NM00005.ps2"), b"card-1", 10);

        sync_before(g, &rt, &sv).unwrap();
        assert!(sv.join("memcards/NM00005.ps2").is_file());
        assert!(sv.join("bios/r27v1602f.nvm").is_file());
        assert!(!sv.join("bios/r27v1602f.7d").exists(), "BIOS ROM stays out");

        put(&data.join("memcards/NM00005.ps2"), b"card-2", 0);
        assert_eq!(sync_after(g, &rt, &sv).unwrap(), ["memcards/NM00005.ps2"]);
        assert_eq!(
            fs::read(sv.join("memcards/NM00005.ps2")).unwrap(),
            b"card-2"
        );
    }

    #[test]
    fn crash_before_sync_after_is_recovered_on_next_launch() {
        let t = tempfile::tempdir().unwrap();
        let rt = t.path().join("rt");
        let sv = dir(t.path(), "mt3");
        let g = game(GameId::Mt3);

        put(&sv.join("card.bin"), b"v1", 100);
        sync_before(g, &rt, &sv).unwrap();
        // Game saved v2, then crashed before sync_after ran
        put(&rt.join("card.bin"), b"v2", 10);
        let recovered = sync_before(g, &rt, &sv).unwrap();
        assert_eq!(recovered.to_saves, ["card.bin"]);
        assert_eq!(fs::read(sv.join("card.bin")).unwrap(), b"v2");
    }
}
