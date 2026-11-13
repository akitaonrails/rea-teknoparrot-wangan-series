//! `launcher.log`: one line per decision, with a timestamp.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::Paths;

pub fn line(paths: &Paths, text: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::create_dir_all(&paths.data);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log())
    {
        let _ = writeln!(f, "{secs} {text}");
    }
}
