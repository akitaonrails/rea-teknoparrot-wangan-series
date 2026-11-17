//! One game run: prepare the runtime and the Wine prefix, start gamescope
//! with the helpers and the game, wait, and leave nothing behind.
//!
//! On Windows there is no Wine, prefix or gamescope: the helpers and the
//! game start directly, and after the game the board is told to quit and
//! anything still running from the runtime folder is stopped.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use rea_launcher::library::Entry;
use wangan_core::catalog::{Game, Route};
use wangan_core::launch::{self, Host, Plan};
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
pub fn prepare(
    paths: &Paths,
    game: &Game,
    entry: &Entry<Game>,
    settings: &Settings,
) -> Result<Prepared> {
    let rt = paths.runtime(game.slug);
    let items =
        runtime::list(&entry.dir).with_context(|| format!("reading {}", entry.dir.display()))?;
    let jp = entry.dir.join("data_jp");
    let jp_items = if jp.is_dir() {
        runtime::list(&jp)?
    } else {
        vec![]
    };
    if let wangan_core::catalog::Support::Blocked(why) =
        game.support_on(wangan_core::catalog::Host::current())
    {
        bail!("{} can't start here: {why}", game.title);
    }
    let comps = paths.runtime_components();
    let host = host(paths);
    check_components(game, &comps, &host)?;
    let ops = runtime::plan(game, &entry.dir, &items, &jp_items, &rt, &comps);
    runtime::apply(&ops).context("building the runtime folder")?;
    #[cfg(unix)]
    if game.route == Route::NativeN2 {
        runtime::apply(&runtime::plan_libso(
            &entry.dir.join("libso"),
            &rt.join("libso"),
        )?)?;
        link_nvidia_glx32(&rt.join("glfix"))?;
    }
    if game.route == Route::System246 {
        prepare_sys246(paths, game, entry, &rt)?;
    }
    let plan = launch::plan_on(game, &rt, &paths.prefix(), settings, &host)
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

/// Wangan R: the BIOS from the board files, and a fresh dongle card from
/// the dump's zip at every start (the game writes to the card while it
/// runs; the dump's copy is never touched).
fn prepare_sys246(paths: &Paths, game: &Game, entry: &Entry<Game>, rt: &Path) -> Result<()> {
    use wangan_core::sys246;
    let set = game
        .firmware
        .context("no board files are known for this game")?;
    let fw = set.dir(&paths.data);
    if !set.complete(&fw) {
        bail!(
            "the {} isn't added yet (Add files on the game's tile)",
            set.title
        );
    }
    let data = sys246::data_dir(rt);
    for f in set.files {
        // the emulator writes .nvm/.mec beside the BIOS: a copy in the runtime
        let to = data.join("bios").join(f.name);
        if fs::read(&to).map(|b| !f.matches(&b)).unwrap_or(true) {
            write_atomic(&to, &fs::read(fw.join(f.name))?)?;
        }
    }
    let zip = entry.dir.join(game.main_file);
    let card = sys246::dongle_from_zip(&zip)
        .with_context(|| format!("making the dongle card from {}", zip.display()))?;
    write_atomic(
        &data
            .join("memcards")
            .join(format!("{}.ps2", sys246::GAME_ID)),
        &card,
    )
}

fn op_dir<'a>(game: &Game, c: &'a runtime::Components) -> &'a Path {
    if game.id == wangan_core::catalog::GameId::Mt6rr {
        &c.openparrot_6rr
    } else {
        &c.openparrot
    }
}

/// How this launcher starts the Windows games: directly on Windows; on
/// Linux under the bundled (or chosen) Wine, else `wine` from PATH.
pub fn host(paths: &Paths) -> Host {
    if cfg!(windows) {
        return Host::Windows;
    }
    match &paths.wine {
        None => Host::system_wine(),
        Some(dir) => {
            let ffmpeg = dir.join("lib").join("ffmpeg");
            Host::Linux {
                wine: dir.join("bin").join("wine").to_string_lossy().into_owned(),
                wine_env: wine_lib_env(
                    ffmpeg.is_dir().then_some(ffmpeg.as_path()),
                    std::env::var("LD_LIBRARY_PATH").ok().as_deref(),
                ),
            }
        }
    }
}

/// The bundled Wine's FFmpeg (for `winedmo`, the movie demuxer: the
/// FFmpeg 4 libraries its build links against) goes first on the library
/// path of every Wine process.
pub fn wine_lib_env(ffmpeg: Option<&Path>, current: Option<&str>) -> Vec<(String, String)> {
    let Some(ff) = ffmpeg else {
        return vec![];
    };
    let mut v = ff.to_string_lossy().into_owned();
    if let Some(c) = current.filter(|c| !c.is_empty()) {
        v.push(':');
        v.push_str(c);
    }
    vec![("LD_LIBRARY_PATH".into(), v)]
}

fn wine_bin(paths: &Paths, name: &str) -> PathBuf {
    match &paths.wine {
        Some(dir) => dir.join("bin").join(name),
        None => PathBuf::from(name),
    }
}

fn check_components(game: &Game, c: &runtime::Components, host: &Host) -> Result<()> {
    let need: Vec<PathBuf> = match game.route {
        Route::Es3b => {
            let mut v = vec![
                op_dir(game, c).join("OpenParrot64.dll"),
                op_dir(game, c).join("OpenParrotLoader64.exe"),
                c.io_board.join("wangan-io.exe"),
            ];
            if matches!(host, Host::Linux { .. }) {
                v.push(c.dxvk.join("d3d11.dll"));
            }
            v
        }
        Route::NativeN2 => vec![c.wal.join("libwal_3dxp.so"), c.wal.join("libopenal.so")],
        Route::System246 => vec![
            c.pcsx2x6.join("app").join("AppRun"),
            c.pcsx2x6
                .join(wangan_core::sys246::GAME_ID)
                .join("boot.elf"),
        ],
        Route::Chihiro => vec![],
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
#[cfg(unix)]
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
pub fn prepare_prefix(paths: &Paths, wine_env: &[(String, String)]) -> Result<()> {
    let pfx = paths.prefix();
    let marker = pfx.join(".rea-wangan-prefix");
    if fs::read_to_string(&marker).ok().as_deref() == Some(PREFIX_VERSION) {
        return Ok(());
    }
    fs::create_dir_all(&pfx)?;
    log::line(paths, "creating the Wine prefix");
    let wine = |args: &[&str]| run_wine(paths, wine_env, &pfx, args);
    wine(&["wineboot", "-u"])?;
    wineserver_wait(paths, &pfx);
    // no crash dialog over the game
    wine(&[
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
    ])?;
    // movies: Wine's FFmpeg source (compressed WMV, as Windows) …
    wine(&[
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
    ])?;
    wineserver_wait(paths, &pfx);
    // … and a WMV decoder registered for MF RGB32 (docs/RUNTIME_NOTES.md)
    let reg = fs::read_to_string(pfx.join("system.reg")).unwrap_or_default();
    let current = wmv_output_types(&reg).unwrap_or_default();
    let fixed = with_mf_rgb32(&current);
    let hex: String = fixed.iter().map(|b| format!("{b:02x}")).collect();
    wine(&[
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
    ])?;
    wineserver_wait(paths, &pfx);
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

fn run_wine(paths: &Paths, env: &[(String, String)], pfx: &Path, args: &[&str]) -> Result<()> {
    let st = Command::new(wine_bin(paths, "wine"))
        .args(args)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .env("WINEPREFIX", pfx)
        .env("WINEDEBUG", "-all")
        // no Mono/Gecko install prompts: the games need neither
        .env("WINEDLLOVERRIDES", "mscoree,mshtml=")
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

fn wineserver_wait(paths: &Paths, pfx: &Path) {
    let _ = Command::new(wine_bin(paths, "wineserver"))
        .arg("-w")
        .env("WINEPREFIX", pfx)
        .status();
}

fn wineserver_kill(paths: &Paths, pfx: &Path) {
    let _ = Command::new(wine_bin(paths, "wineserver"))
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
    if p.plan.gamescope.is_empty() {
        return run_direct(paths, game, p, on_start);
    }
    if p.plan.wine_prefix.is_some() {
        let env = match host(paths) {
            Host::Linux { wine_env, .. } => wine_env,
            Host::Windows => vec![],
        };
        prepare_prefix(paths, &env)?;
    }
    if display == Display::Headless && game.route == Route::System246 {
        // test runs reach no sound server: pcsx2x6 would stop on an error box
        write_atomic(
            &launch::sys246_ini(&p.runtime),
            wangan_core::sys246::pcsx2_ini(
                wangan_core::sys246::upscale_for(p.plan.game_size.h),
                true,
            )
            .as_bytes(),
        )?;
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
    finish(paths, p);
    Ok(status)
}

/// A planned process as a direct child (Windows): its own folder, no
/// console window.
fn direct_command(c: &launch::Command) -> Command {
    let mut cmd = Command::new(&c.program);
    cmd.args(&c.args)
        .current_dir(&c.cwd)
        .envs(c.env.iter().map(|(k, v)| (k, v)));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Windows: the helpers (board, auth daemon), a pause after each as
/// TeknoParrotUI makes, then the loader, which waits for the game.
fn run_direct(
    paths: &Paths,
    game: &Game,
    p: &Prepared,
    on_start: impl FnOnce(u32),
) -> Result<std::process::ExitStatus> {
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.data.join(format!("{}.log", game.slug)))?;
    log::line(paths, &format!("starting {} (direct)", game.slug));
    let mut helpers = Vec::new();
    let started = (|| -> Result<std::process::Child> {
        for h in &p.plan.helpers {
            let mut c = direct_command(h);
            c.stdout(log_file.try_clone()?)
                .stderr(log_file.try_clone()?);
            helpers.push(
                rea_launcher_native::child::spawn_tied(&mut c)
                    .with_context(|| format!("starting {}", h.program))?,
            );
            std::thread::sleep(Duration::from_secs(2));
        }
        let mut c = direct_command(&p.plan.game);
        c.stdout(log_file.try_clone()?)
            .stderr(log_file.try_clone()?);
        rea_launcher_native::child::spawn_tied(&mut c)
            .with_context(|| format!("starting {}", p.plan.game.program))
    })();
    let mut child = match started {
        Ok(c) => c,
        Err(e) => {
            finish_direct(paths, p, &mut helpers);
            return Err(e);
        }
    };
    on_start(child.id());
    let status = child.wait()?;
    log::line(paths, &format!("{} ended: {status}", game.slug));
    finish_direct(paths, p, &mut helpers);
    Ok(status)
}

/// Tell the board to quit (the game has ended: the stop order of
/// [`finish`]), give the helpers a moment, then stop what is left: the
/// helpers, and whatever else runs from the runtime folder (the auth daemon
/// the loader started).
fn finish_direct(paths: &Paths, p: &Prepared, helpers: &mut [std::process::Child]) {
    signal_quit(&p.block);
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && helpers.iter_mut().any(|h| matches!(h.try_wait(), Ok(None)))
    {
        std::thread::sleep(Duration::from_millis(100));
    }
    for h in helpers.iter_mut() {
        if matches!(h.try_wait(), Ok(None)) {
            let _ = h.kill();
            let _ = h.wait();
        }
    }
    #[cfg(windows)]
    {
        let n = crate::winproc::kill_under(&p.runtime);
        if n > 0 {
            log::line(
                paths,
                &format!("stopped {n} leftover process(es) in the runtime"),
            );
        }
    }
    #[cfg(not(windows))]
    let _ = paths;
}

fn signal_quit(block: &Path) {
    if let Ok(mut f) = fs::OpenOptions::new().write(true).open(block) {
        use std::io::{Seek, SeekFrom};
        let _ = f
            .seek(SeekFrom::Start(QUIT as u64))
            .and_then(|_| f.write_all(&[1]));
    }
}

/// Game first, then the board, then Wine (stopping the board while the
/// game runs shows E0303 on the cabinet screen).
pub fn finish(paths: &Paths, p: &Prepared) {
    signal_quit(&p.block);
    if let Some(pfx) = &p.plan.wine_prefix {
        std::thread::sleep(Duration::from_millis(300));
        wineserver_kill(paths, pfx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_ffmpeg_goes_first_on_the_library_path() {
        let ff = Path::new("/pkg/components/wine/lib/ffmpeg");
        assert_eq!(wine_lib_env(None, Some("/x")), vec![]);
        assert_eq!(
            wine_lib_env(Some(ff), None),
            vec![(
                "LD_LIBRARY_PATH".into(),
                "/pkg/components/wine/lib/ffmpeg".into()
            )]
        );
        assert_eq!(
            wine_lib_env(Some(ff), Some("/usr/lib/x")),
            vec![(
                "LD_LIBRARY_PATH".into(),
                "/pkg/components/wine/lib/ffmpeg:/usr/lib/x".into()
            )]
        );
        assert_eq!(
            wine_lib_env(Some(ff), Some(""))[0].1,
            "/pkg/components/wine/lib/ffmpeg"
        );
    }

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
