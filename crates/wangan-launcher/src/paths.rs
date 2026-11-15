//! Where the launcher keeps things.
//!
//! - data (`$WANGAN_DATA`, else on Linux `$XDG_DATA_HOME/rea-wangan` or
//!   `~/.local/share/rea-wangan`, on Windows `%LOCALAPPDATA%\rea-wangan`):
//!   `launcher.cfg`, `settings/`, `runtimes/` (one per game, saves
//!   included), `prefix/` (Wine, Linux only), `launcher.log`;
//! - components (`$WANGAN_COMPONENTS`, else `components/` beside the
//!   program): `openparrot/`, `openparrot-6rr/`, `dxvk/`, `io/`, `wal/`, `pcsx2x6/`,
//!   `art/`, `fonts/`, and on Linux the bundled `wine/`;
//! - Wine (Linux): `$WANGAN_WINE` (a Wine folder with `bin/wine`), else the
//!   package's `components/wine/`, else `wine` from PATH.

use std::path::{Path, PathBuf};

use wangan_core::runtime::Components;

#[derive(Debug, Clone)]
pub struct Paths {
    pub data: PathBuf,
    pub components: PathBuf,
    /// The Wine folder to use (`bin/wine`, `bin/wineserver`); `None`: PATH.
    pub wine: Option<PathBuf>,
}

impl Paths {
    pub fn discover() -> Self {
        let data = std::env::var_os("WANGAN_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(default_data);
        let components = std::env::var_os("WANGAN_COMPONENTS")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|e| e.parent().map(|p| p.join("components")))
                    .unwrap_or_else(|| PathBuf::from("components"))
            });
        let wine = pick_wine(
            std::env::var_os("WANGAN_WINE").map(PathBuf::from),
            &components,
        );
        Self {
            data,
            components,
            wine,
        }
    }

    pub fn config(&self) -> PathBuf {
        self.data.join("launcher.cfg")
    }
    pub fn settings(&self, slug: &str) -> PathBuf {
        self.data.join("settings").join(format!("{slug}.cfg"))
    }
    pub fn runtime(&self, slug: &str) -> PathBuf {
        self.data.join("runtimes").join(slug)
    }
    pub fn prefix(&self) -> PathBuf {
        self.data.join("prefix")
    }
    pub fn log(&self) -> PathBuf {
        self.data.join("launcher.log")
    }
    pub fn art(&self, name: &str) -> PathBuf {
        self.components.join("art").join(name)
    }
    pub fn fonts(&self) -> PathBuf {
        self.components.join("fonts")
    }
    pub fn runtime_components(&self) -> Components {
        Components {
            openparrot: self.components.join("openparrot"),
            openparrot_6rr: self.components.join("openparrot-6rr"),
            dxvk: self.components.join("dxvk"),
            io_board: self.components.join("io"),
            wal: self.components.join("wal"),
            pcsx2x6: self.components.join("pcsx2x6"),
        }
    }
}

#[cfg(not(windows))]
fn default_data() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"));
    base.join("rea-wangan")
}

#[cfg(windows)]
fn default_data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join("AppData").join("Local"))
        .join("rea-wangan")
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

#[cfg(not(windows))]
fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `launcher.cfg`: `library = <folder>` (more keys may follow).
pub fn read_library(cfg: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(cfg).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "library" && !v.trim().is_empty()).then(|| PathBuf::from(v.trim()))
    })
}

pub fn write_library(cfg: &Path, dir: &Path) -> std::io::Result<()> {
    let old = std::fs::read_to_string(cfg).unwrap_or_default();
    let mut lines: Vec<String> = old
        .lines()
        .filter(|l| l.split_once('=').map(|(k, _)| k.trim()) != Some("library"))
        .map(String::from)
        .collect();
    lines.insert(0, format!("library = {}", dir.display()));
    if let Some(p) = cfg.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = cfg.with_extension("tmp");
    std::fs::write(&tmp, lines.join("\n") + "\n")?;
    std::fs::rename(tmp, cfg)
}

#[cfg(windows)]
fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
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
    fn library_round_trip_keeps_other_lines() {
        let t = tempfile::tempdir().unwrap();
        let cfg = t.path().join("a/launcher.cfg");
        assert_eq!(read_library(&cfg), None);
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, "# note\nlibrary = /old\nother = 1\n").unwrap();
        write_library(&cfg, Path::new("/games/Jogos d'Ação")).unwrap();
        assert_eq!(
            read_library(&cfg),
            Some(PathBuf::from("/games/Jogos d'Ação"))
        );
        let text = std::fs::read_to_string(&cfg).unwrap();
        assert!(text.contains("# note") && text.contains("other = 1") && !text.contains("/old"));
    }
}
