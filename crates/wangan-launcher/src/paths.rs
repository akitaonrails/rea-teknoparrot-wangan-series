//! Where the launcher keeps things (`rea_launcher::hub::HubPaths`).
//!
//! - data (`$WANGAN_DATA`, else on Linux `$XDG_DATA_HOME/rea-wangan` or
//!   `~/.local/share/rea-wangan`, on Windows `%LOCALAPPDATA%\rea-wangan`):
//!   `launcher.cfg`, `settings/`, `saves/` (one folder per game with cards
//!   and saves, mirrored with its runtime), `runtimes/` (one per game),
//!   `firmware/` (the board files the player added), `prefix/`
//!   (Wine, Linux only), `launcher.log` and each game's `<game>.log`;
//! - components (`$WANGAN_COMPONENTS`, else `components/` beside the
//!   program): `openparrot/`, `openparrot-6rr/`, `dxvk/`, `io/`, `wal/`,
//!   `pcsx2x6/`, `art/`, and on Linux the bundled `wine/`;
//! - Wine (Linux): `$WANGAN_WINE` (a Wine folder with `bin/wine`), else the
//!   package's `components/wine/`, else `wine` from PATH.

use std::path::{Path, PathBuf};

use rea_launcher::hub::{HubConfig, HubDirs, HubPaths, PathField};
use wangan_core::runtime::Components;

/// The folders' names.
pub fn dirs() -> HubDirs {
    HubDirs {
        env_prefix: "WANGAN",
        name: "rea-wangan",
        dev_assets: cfg!(debug_assertions)
            .then_some(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")),
    }
}

/// The paths `launcher.cfg` keeps besides the library folders: none (the
/// board files are copied into the data folder, `--add-files`).
pub const FIELDS: &[PathField] = &[];

/// `launcher.cfg`; a file that can't be read is an error.
pub fn config(p: &HubPaths) -> std::io::Result<HubConfig> {
    HubConfig::load(&p.config_file(), FIELDS)
}

/// The shared Wine prefix (Linux).
pub fn prefix(p: &HubPaths) -> PathBuf {
    p.data.join("prefix")
}

/// The player's backup-friendly card and save folder for `slug`.
pub fn saves(p: &HubPaths, slug: &str) -> PathBuf {
    wangan_core::saves::dir(&p.data, slug)
}

pub fn components(p: &HubPaths) -> Components {
    let c = &p.components;
    Components {
        openparrot: c.join("openparrot"),
        openparrot_6rr: c.join("openparrot-6rr"),
        dxvk: c.join("dxvk"),
        io_board: c.join("io"),
        wal: c.join("wal"),
        pcsx2x6: c.join("pcsx2x6"),
    }
}

/// The Wine folder to use (`bin/wine`, `bin/wineserver`); `None`: PATH.
pub fn wine(p: &HubPaths) -> Option<PathBuf> {
    pick_wine(
        std::env::var_os("WANGAN_WINE").map(PathBuf::from),
        &p.components,
    )
}

/// The Wine to run: `$WANGAN_WINE` when set (a folder with `bin/wine`),
/// else the package's `components/wine` when it has `bin/wine`, else none
/// (`wine` from PATH).
pub fn pick_wine(env: Option<PathBuf>, components: &Path) -> Option<PathBuf> {
    if let Some(dir) = env.filter(|d| !d.as_os_str().is_empty()) {
        return Some(dir);
    }
    let bundled = components.join("wine");
    bundled
        .join("bin")
        .join("wine")
        .is_file()
        .then_some(bundled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wine_is_the_override_then_the_bundled_one_then_path() {
        let t = tempfile::tempdir().unwrap();
        let comps = t.path().join("components");
        assert_eq!(pick_wine(None, &comps), None, "nothing bundled: PATH");
        std::fs::create_dir_all(comps.join("wine/bin")).unwrap();
        assert_eq!(
            pick_wine(None, &comps),
            None,
            "a folder without bin/wine is not a Wine"
        );
        std::fs::write(comps.join("wine/bin/wine"), b"").unwrap();
        assert_eq!(pick_wine(None, &comps), Some(comps.join("wine")));
        assert_eq!(
            pick_wine(Some("/opt/wine".into()), &comps),
            Some(PathBuf::from("/opt/wine"))
        );
        assert_eq!(pick_wine(Some("".into()), &comps), Some(comps.join("wine")));
    }

    #[test]
    fn the_library_that_0_2_0_saved_still_reads() {
        let c = HubConfig::parse("# note\nlibrary = /games/Jogos d'Ação\nother = 1\n", FIELDS);
        assert_eq!(c.libraries, [PathBuf::from("/games/Jogos d'Ação")]);
    }

    #[test]
    fn components_sit_in_the_components_folder() {
        let p = HubPaths {
            data: "/d".into(),
            components: "/pkg/components".into(),
            dev_assets: None,
        };
        let c = components(&p);
        assert_eq!(c.io_board, Path::new("/pkg/components/io"));
        assert_eq!(
            c.openparrot_6rr,
            Path::new("/pkg/components/openparrot-6rr")
        );
        assert_eq!(prefix(&p), Path::new("/d/prefix"));
    }
}
