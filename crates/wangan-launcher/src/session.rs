//! One game run: prepare the runtime and the Wine prefix, start gamescope
//! with the helpers and the game, wait, and leave nothing behind.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use wangan_core::catalog::{Game, Route};
use wangan_core::launch::{self, Plan};
use wangan_core::library::Entry;
use wangan_core::runtime;
use wangan_core::settings::Settings;
use wangan_jvs::shared::{QUIT, Snapshot};

use crate::log;
use crate::paths::Paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// On the player's screen.
    Screen,
    /// In a headless gamescope (tests, screenshots).
    Headless,
}

/// Bumped when the prefix setup changes, so old prefixes get the new steps.
const PREFIX_VERSION: &str = "1";

pub struct Prepared {
    pub plan: Plan,
    pub runtime: PathBuf,
    pub block: PathBuf,
}

/// Build the runtime folder and write the game's config files.
pub fn prepare(paths: &Paths, game: &Game, entry: &Entry, settings: &Settings) -> Result<Prepared> {
    let rt = paths.runtime(game.slug);
    let items =
        runtime::list(&entry.dir).with_context(|| format!("reading {}", entry.dir.display()))?;
    let jp = entry.dir.join("data_jp");
    let jp_items = if jp.is_dir() {
        runtime::list(&jp)?
    } else {
        vec![]
    };
    let comps = paths.runtime_components();
    check_components(game, &comps)?;
    let ops = runtime::plan(game, &entry.dir, &items, &jp_items, &rt, &comps);
    runtime::apply(&ops).context("building the runtime folder")?;
    if game.route == Route::NativeN2 {
        runtime::apply(&runtime::plan_libso(
            &entry.dir.join("libso"),
            &rt.join("libso"),
        )?)?;
        link_nvidia_glx32(&rt.join("glfix"))?;
    }
    let plan = launch::plan(game, &rt, &paths.prefix(), settings)
        .context("this game has no launch route yet")?;
    for (path, text) in &plan.files {
        write_atomic(path, text.as_bytes())?;
    }
    let block = launch::block_path(&rt);
    // a fresh block: everything released, quit clear
    write_atomic(&block, &Snapshot::default().encode())?;
    log::line(
        paths,
        &format!("prepared {} in {}", game.slug, rt.display()),
    );
    Ok(Prepared {
        plan,
        runtime: rt,
        block,
    })
}

fn op_dir<'a>(game: &Game, c: &'a runtime::Components) -> &'a Path {
    if game.id == wangan_core::catalog::GameId::Mt6rr {
        &c.openparrot_6rr
    } else {
        &c.openparrot
    }
}

fn check_components(game: &Game, c: &runtime::Components) -> Result<()> {
    let need: Vec<PathBuf> = match game.route {
        Route::Es3b => vec![
            op_dir(game, c).join("OpenParrot64.dll"),
            op_dir(game, c).join("OpenParrotLoader64.exe"),
            c.dxvk.join("d3d11.dll"),
            c.io_board.join("wangan-io.exe"),
        ],
        Route::NativeN2 => vec![c.wal.join("libwal_3dxp.so"), c.wal.join("libopenal.so")],
        _ => vec![],
    };
    let missing: Vec<String> = need
        .iter()
        .filter(|p| !p.is_file())
        .map(|p| p.display().to_string())
        .collect();
    if !missing.is_empty() {
        bail!(
            "the launcher's components are incomplete; missing: {}",
            missing.join(", ")
        );
    }
    Ok(())
}

/// glvnd looks for `libGLX_nvidia.so.0`; some 32-bit driver installs only
/// carry the versioned file (docs/RUNTIME_NOTES.md). Nothing to do without
/// a 32-bit NVIDIA driver.
fn link_nvidia_glx32(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    let Ok(rd) = fs::read_dir("/usr/lib32") else {
        return Ok(());
    };
    let mut found: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            n.starts_with("libGLX_nvidia.so.") && n != "libGLX_nvidia.so.0"
        })
        .collect();
    found.sort();
    if let Some(target) = found.last() {
        let at = dir.join("libGLX_nvidia.so.0");
        if fs::read_link(&at).ok().as_ref() != Some(target) {
            let _ = fs::remove_file(&at);
            std::os::unix::fs::symlink(target, &at)?;
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("rea-tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Create and fix up the shared Wine prefix once (per [`PREFIX_VERSION`]).
pub fn prepare_prefix(paths: &Paths) -> Result<()> {
    let pfx = paths.prefix();
    let marker = pfx.join(".rea-wangan-prefix");
    if fs::read_to_string(&marker).ok().as_deref() == Some(PREFIX_VERSION) {
        return Ok(());
    }
    fs::create_dir_all(&pfx)?;
    log::line(paths, "creating the Wine prefix");
    wine(&pfx, &["wineboot", "-u"])?;
    wineserver_wait(&pfx);
    // no crash dialog over the game
    wine(
        &pfx,
        &[
            "reg",
            "add",
            r"HKCU\Software\Wine\WineDbg",
            "/v",
            "ShowCrashDialog",
            "/t",
            "REG_DWORD",
            "/d",
            "0",
            "/f",
        ],
    )?;
    // movies: Wine's FFmpeg source (compressed WMV, as Windows) …
    wine(
        &pfx,
        &[
            "reg",
            "add",
            r"HKCU\Software\Wine\MediaFoundation",
            "/v",
            "DisableGstByteStreamHandler",
            "/t",
            "REG_DWORD",
            "/d",
            "1",
            "/f",
        ],
    )?;
    wineserver_wait(&pfx);
    // … and a WMV decoder registered for MF RGB32 (docs/RUNTIME_NOTES.md)
    let reg = fs::read_to_string(pfx.join("system.reg")).unwrap_or_default();
    let current = wmv_output_types(&reg).unwrap_or_default();
    let fixed = with_mf_rgb32(&current);
    let hex: String = fixed.iter().map(|b| format!("{b:02x}")).collect();
    wine(
        &pfx,
        &[
            "reg",
            "add",
            WMV_MFT_KEY,
            "/v",
            "OutputTypes",
            "/t",
            "REG_BINARY",
            "/d",
            &hex,
            "/f",
        ],
    )?;
    wineserver_wait(&pfx);
    fs::write(&marker, PREFIX_VERSION)?;
    Ok(())
}

const WMV_MFT_KEY: &str =
    r"HKLM\Software\Classes\MediaFoundation\Transforms\82d353df-90bd-4382-8bc2-3f6192b76e34";

fn guid_le(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[..4].copy_from_slice(&d1.to_le_bytes());
    g[4..6].copy_from_slice(&d2.to_le_bytes());
    g[6..8].copy_from_slice(&d3.to_le_bytes());
    g[8..].copy_from_slice(&d4);
    g
}

const MF_BASE: [u8; 8] = [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71];

/// `OutputTypes` of the WMV decoder MFT with `{MFMediaType_Video, MFVideoFormat_RGB32}` added once.
pub fn with_mf_rgb32(current: &[u8]) -> Vec<u8> {
    let video = guid_le(0x73646976, 0x0000, 0x0010, MF_BASE);
    let rgb32 = guid_le(0x00000016, 0x0000, 0x0010, MF_BASE);
    let mut pair = video.to_vec();
    pair.extend_from_slice(&rgb32);
    let mut out = current.to_vec();
    if !out.chunks(32).any(|c| c == pair.as_slice()) {
        out.extend_from_slice(&pair);
    }
    out
}

/// The current `OutputTypes` bytes from a Wine `system.reg`.
pub fn wmv_output_types(reg: &str) -> Option<Vec<u8>> {
    let head =
        r"[Software\\Classes\\MediaFoundation\\Transforms\\82d353df-90bd-4382-8bc2-3f6192b76e34]";
    let start = reg.find(head)?;
    let block = &reg[start..];
    let block = &block[..block.find("\n\n").unwrap_or(block.len())];
    let at = block.find("\"OutputTypes\"=hex:")? + "\"OutputTypes\"=hex:".len();
    let mut bytes = Vec::new();
    for line in block[at..].lines() {
        let l = line.trim();
        let more = l.ends_with('\\');
        for tok in l
            .trim_end_matches('\\')
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            bytes.push(u8::from_str_radix(tok, 16).ok()?);
        }
        if !more {
            break;
        }
    }
    Some(bytes)
}

fn wine(pfx: &Path, args: &[&str]) -> Result<()> {
    let st = Command::new("wine")
        .args(args)
        .env("WINEPREFIX", pfx)
        .env("WINEDEBUG", "-all")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("starting wine (is Wine installed?)")?;
    if !st.success() {
        bail!("wine {} failed: {st}", args.join(" "));
    }
    Ok(())
}

fn wineserver_wait(pfx: &Path) {
    let _ = Command::new("wineserver")
        .arg("-w")
        .env("WINEPREFIX", pfx)
        .status();
}

fn wineserver_kill(pfx: &Path) {
    let _ = Command::new("wineserver")
        .arg("-k")
        .env("WINEPREFIX", pfx)
        .status();
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn sh_command(c: &launch::Command) -> String {
    let mut s = format!("cd {} && env", sh_quote(&c.cwd.to_string_lossy()));
    for (k, v) in &c.env {
        s.push(' ');
        s.push_str(&sh_quote(&format!("{k}={v}")));
    }
    s.push(' ');
    s.push_str(&sh_quote(&c.program));
    for a in &c.args {
        s.push(' ');
        s.push_str(&sh_quote(a));
    }
    s
}

/// The shell script gamescope runs: helpers in the background (a short
/// pause each, as TeknoParrotUI waits a second per helper), then the game.
pub fn script(plan: &Plan) -> String {
    let mut s = String::from("set -u\n");
    for h in &plan.helpers {
        s.push_str(&format!(
            "( {} ) >/dev/null 2>&1 &\nsleep 2\n",
            sh_command(h)
        ));
    }
    s.push_str(&format!(
        "{}\n",
        sh_command(&plan.game).replacen(" && env", " && exec env", 1)
    ));
    s
}

pub fn gamescope_command(plan: &Plan, display: Display, rt: &Path) -> Result<Command> {
    let script_path = rt.join("session.sh");
    write_atomic(&script_path, script(plan).as_bytes())?;
    let mut c = Command::new("gamescope");
    if display == Display::Headless {
        c.args(["--backend", "headless"]);
        c.args(plan.gamescope.iter().filter(|a| a.as_str() != "-f"));
    } else {
        c.args(&plan.gamescope);
    }
    c.arg("--").arg("sh").arg(&script_path);
    Ok(c)
}

/// Run a prepared game to the end. `on_start` gets the gamescope pid.
pub fn run(
    paths: &Paths,
    game: &Game,
    p: &Prepared,
    display: Display,
    on_start: impl FnOnce(u32),
) -> Result<std::process::ExitStatus> {
    if p.plan.wine_prefix.is_some() {
        prepare_prefix(paths)?;
    }
    let mut cmd = gamescope_command(&p.plan, display, &p.runtime)?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.data.join(format!("{}.log", game.slug)))?;
    cmd.stdout(log_file.try_clone()?).stderr(log_file);
    log::line(paths, &format!("starting {} ({:?})", game.slug, display));
    let mut child = rea_launcher_native::child::spawn_tied(&mut cmd)
        .context("starting gamescope (is it installed?)")?;
    on_start(child.id());
    let status = child.wait()?;
    log::line(paths, &format!("{} ended: {status}", game.slug));
    finish(p);
    Ok(status)
}

/// Game first, then the board, then Wine (stopping the board while the
/// game runs shows E0303 on the cabinet screen).
pub fn finish(p: &Prepared) {
    if let Ok(mut f) = fs::OpenOptions::new().write(true).open(&p.block) {
        use std::io::{Seek, SeekFrom};
        let _ = f
            .seek(SeekFrom::Start(QUIT as u64))
            .and_then(|_| f.write_all(&[1]));
    }
    if let Some(pfx) = &p.plan.wine_prefix {
        std::thread::sleep(std::time::Duration::from_millis(300));
        wineserver_kill(pfx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb32_is_added_once() {
        let once = with_mf_rgb32(&[]);
        assert_eq!(once.len(), 32);
        assert_eq!(with_mf_rgb32(&once), once);
        // MFVideoFormat_RGB32 = {00000016-0000-0010-8000-00AA00389B71}
        assert_eq!(&once[16..20], &[0x16, 0, 0, 0]);
    }

    #[test]
    fn output_types_are_read_from_system_reg() {
        let reg = "[Software\\\\Classes\\\\MediaFoundation\\\\Transforms\\\\82d353df-90bd-4382-8bc2-3f6192b76e34] 1\n@=\"WMVideo Decoder MFT\"\n\"InputTypes\"=hex:01,02\n\"OutputTypes\"=hex:0a,0b,\\\n  0c\n\"Attributes\"=hex:ff\n\n[Other]\n";
        assert_eq!(wmv_output_types(reg), Some(vec![0x0a, 0x0b, 0x0c]));
        assert_eq!(wmv_output_types("nothing"), None);
    }

    #[test]
    fn script_starts_helpers_then_execs_the_game_with_quoting() {
        let plan = Plan {
            files: vec![],
            helpers: vec![launch::Command {
                program: "wine".into(),
                args: vec!["io.exe".into()],
                env: vec![("A".into(), "x y".into())],
                cwd: "/rt/it's".into(),
            }],
            game: launch::Command {
                program: "wine".into(),
                args: vec![r"Z:\a b\g.exe".into()],
                env: vec![],
                cwd: "/rt".into(),
            },
            gamescope: vec![],
            wine_prefix: None,
            game_size: wangan_core::settings::Size::new(1360, 768),
        };
        let s = script(&plan);
        assert!(
            s.contains("( cd '/rt/it'\\''s' && env 'A=x y' 'wine' 'io.exe' ) >/dev/null 2>&1 &"),
            "{s}"
        );
        assert!(
            s.contains("cd '/rt' && exec env 'wine' 'Z:\\a b\\g.exe'"),
            "{s}"
        );
    }
}
