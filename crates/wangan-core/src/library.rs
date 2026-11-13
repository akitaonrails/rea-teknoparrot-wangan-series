//! Finding the player's games in a library folder.
//!
//! The library is a folder of game folders (any names). Each folder whose
//! main file matches a known edition becomes an [`Entry`]; a known file
//! name with an unknown digest is reported, never launched.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::catalog::{self, Edition, Game, GameId};

#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Known {
        game: &'static Game,
        edition: &'static Edition,
    },
    /// A file named like a game's main file, with a digest we don't know.
    Unknown { file: String, sha256: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub dir: PathBuf,
    pub found: Found,
}

impl Entry {
    pub fn game_id(&self) -> Option<GameId> {
        match self.found {
            Found::Known { game, .. } => Some(game.id),
            Found::Unknown { .. } => None,
        }
    }
}

/// Digests by (path, size, mtime), so a rescan doesn't read 30 MB files again.
#[derive(Debug, Default, Clone)]
pub struct HashCache {
    map: HashMap<PathBuf, (u64, Option<SystemTime>, String)>,
}

impl HashCache {
    pub fn digest(&mut self, path: &Path) -> std::io::Result<String> {
        let meta = fs::metadata(path)?;
        let key = (meta.len(), meta.modified().ok());
        if let Some((len, mt, sha)) = self.map.get(path)
            && (*len, *mt) == key
        {
            return Ok(sha.clone());
        }
        let sha = sha256_file(path)?;
        self.map
            .insert(path.to_path_buf(), (key.0, key.1, sha.clone()));
        Ok(sha)
    }
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn main_file_names() -> Vec<&'static str> {
    let mut names: Vec<&str> = catalog::GAMES.iter().map(|g| g.main_file).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Look at `dir` itself: is it a game folder?
pub fn probe_dir(dir: &Path, cache: &mut HashCache) -> Option<Entry> {
    for name in main_file_names() {
        let file = dir.join(name);
        if !file.is_file() {
            continue;
        }
        let Ok(sha) = cache.digest(&file) else {
            continue;
        };
        let found = match catalog::identify(&sha) {
            Some((game, edition)) if game.main_file == name => Found::Known { game, edition },
            _ => Found::Unknown {
                file: name.to_string(),
                sha256: sha,
            },
        };
        return Some(Entry {
            dir: dir.to_path_buf(),
            found,
        });
    }
    None
}

/// Every game folder directly inside `root` (and `root` itself), sorted by
/// catalog order, unknown builds last.
pub fn scan(root: &Path, cache: &mut HashCache) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(e) = probe_dir(root, cache) {
        out.push(e);
    }
    if let Ok(rd) = fs::read_dir(root) {
        let mut dirs: Vec<PathBuf> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for d in dirs {
            if let Some(e) = probe_dir(&d, cache) {
                out.push(e);
            }
        }
    }
    out.sort_by_key(|e| (e.game_id().is_none(), e.game_id()));
    out
}

/// The first folder found for each game (a library may hold duplicates).
pub fn by_game(entries: &[Entry]) -> HashMap<GameId, &Entry> {
    let mut m = HashMap::new();
    for e in entries {
        if let Some(id) = e.game_id() {
            m.entry(id).or_insert(e);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_digest_is_reported_with_its_hash() {
        let t = tempfile::tempdir().unwrap();
        let g = t.path().join("Some Game [TP]");
        fs::create_dir(&g).unwrap();
        fs::write(g.join("wmn6r.exe"), b"not the game").unwrap();
        fs::create_dir(t.path().join("empty")).unwrap();
        let mut cache = HashCache::default();
        let found = scan(t.path(), &mut cache);
        assert_eq!(found.len(), 1);
        match &found[0].found {
            Found::Unknown { file, sha256 } => {
                assert_eq!(file, "wmn6r.exe");
                assert_eq!(sha256, &sha256_file(&g.join("wmn6r.exe")).unwrap());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cache_rehashes_only_when_the_file_changes() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("main");
        fs::write(&f, b"one").unwrap();
        let mut c = HashCache::default();
        let a = c.digest(&f).unwrap();
        assert_eq!(c.digest(&f).unwrap(), a);
        fs::write(&f, b"three").unwrap();
        assert_ne!(c.digest(&f).unwrap(), a);
    }

    /// The owner's real library, when present (`WANGAN_GAMES_DIR=… cargo test -- --ignored`).
    #[test]
    #[ignore]
    fn real_library_finds_all_nine() {
        let Ok(root) = std::env::var("WANGAN_GAMES_DIR") else {
            return;
        };
        let mut c = HashCache::default();
        let found = scan(Path::new(&root), &mut c);
        let ids = by_game(&found);
        assert_eq!(ids.len(), 9, "{found:#?}");
    }
}
