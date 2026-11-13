//! Where the launcher keeps things.
//!
//! - data (`$WANGAN_DATA`, else `$XDG_DATA_HOME/rea-wangan`, else
//!   `~/.local/share/rea-wangan`): `launcher.cfg`, `settings/`, `runtimes/`
//!   (one per game, saves included), `prefix/` (Wine), `launcher.log`;
//! - components (`$WANGAN_COMPONENTS`, else `components/` beside the
//!   program): `openparrot/`, `dxvk/`, `io/`, `wal/`, `art/`, `fonts/`.

use std::path::{Path, PathBuf};

use wangan_core::runtime::Components;

#[derive(Debug, Clone)]
pub struct Paths {
    pub data: PathBuf,
    pub components: PathBuf,
}

impl Paths {
    pub fn discover() -> Self {
        let data = std::env::var_os("WANGAN_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let base = std::env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .unwrap_or_else(|| home().join(".local/share"));
                base.join("rea-wangan")
            });
        let components = std::env::var_os("WANGAN_COMPONENTS")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|e| e.parent().map(|p| p.join("components")))
                    .unwrap_or_else(|| PathBuf::from("components"))
            });
        Self { data, components }
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
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
