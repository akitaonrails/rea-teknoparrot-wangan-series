//! One game run: prepare the runtime and the Wine prefix, start the game's
//! processes as one group tied to the launcher, feed its input, wait, and
//! leave nothing behind (`rea_launcher_native::{group, supervise}`).
//!
//! Linux: gamescope (and bubblewrap, binding the game folders read-only)
//! runs a small script that starts the helpers (the I/O board, the auth
//! daemon) and then the game, all under Wine where the game is a Windows
//! program. Windows: the helpers and the game start directly, each as a
//! group of its own. Either way the session ends in the order the cabinet
//! needs ([`Ending`]).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use rea_launcher::fsx;
use rea_launcher::gamescope::{CommandLine, Picture};
use rea_launcher::hub::HubPaths;
use rea_launcher::library::Entry;
use rea_launcher::linkfarm::{self, Occupied};
use rea_launcher::log::Log as LauncherLog;
use rea_launcher::session::{EARLY_END, early_end};
use rea_launcher::wheel::Script;
use rea_launcher_native::feeder::{self, Feeding, Scripted, Tuning};
use rea_launcher_native::group::{Group, Tag};
use rea_launcher_native::supervise::{self, Feed, Hold, Log, Options, Outcome, Watcher};
use wangan_core::catalog::{self, Game, Route, Support};
use wangan_core::launch::{self, Host, Plan};
use wangan_core::runtime;
use wangan_core::saves;
use wangan_core::settings::Settings;
use wangan_core::sys246;
use wangan_jvs::controls::Cabinet;
use wangan_jvs::shared::{QUIT, Snapshot};

use crate::input::{Block, BlockSink};
use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// On the player's screen.
    Screen,
    /// In a headless gamescope (tests, screenshots).
    Headless,
}

/// Bumped when the prefix setup changes, so old prefixes get the new steps.
const PREFIX_VERSION: &str = "1";

/// What marks the session's processes on Linux (its value: the runtime).
const TAG: &str = "WANGAN_SESSION";

/// How long the game, then what it leaves (the board, Wine), get to end
/// by themselves before the session is killed.
const GRACE: Duration = Duration::from_secs(5);

/// The pause after each helper, as TeknoParrotUI makes.
const HELPER_PAUSE: Duration = Duration::from_secs(2);

pub struct Prepared {
    pub plan: Plan,
    pub runtime: PathBuf,
    pub block: PathBuf,
}

/// What drives the game's input block.
pub enum Input {
    /// The input thread (pads and wheels through SDL); `deadzone` is the
    /// pad stick dead zone, 0–0.3.
    Devices {
        tx: Sender<feeder::Command>,
        deadzone: f32,
    },
    /// A timed script (headless tests).
    Script(Script),
    /// Nothing: the block stays released (another program may write it).
    None,
}

impl Input {
    /// The feed of one start and its sink, the input block (no feed:
    /// no sink).
    fn feed(&self, cabinet: Cabinet, block: &Path) -> Result<(Box<dyn Feed>, Option<Sink>)> {
        let sink = || -> Result<Option<Sink>> { Ok(Some(Box::new(BlockSink::open(block)?))) };
        Ok(match self {
            Input::Devices { tx, deadzone } => {
                let tuning = Tuning {
                    deadzone: *deadzone,
                    ..Tuning::default()
                };
                let feeding = Feeding::new(tx.clone(), Block::boxed(cabinet), tuning);
                (Box::new(feeding), sink()?)
            }
            Input::Script(script) => (
                Box::new(Scripted::new(Block::boxed(cabinet), script.clone())),
                sink()?,
            ),
            Input::None => (Box::new(Hold::default()), None),
        })
    }
}

type Sink = Box<dyn Write + Send>;

/// One line in `launcher.log`.
fn say_launcher(paths: &HubPaths, line: &str) {
    LauncherLog::new(paths.log()).line(line);
}

/// Build the runtime folder and write the game's config files.
pub fn prepare(
    paths: &HubPaths,
    game: &Game,
    entry: &Entry<Game>,
    settings: &Settings,
) -> Result<Prepared> {
    let rt = paths.runtime(game.slug);
    if let Support::Blocked(why) = game.support_on(catalog::Host::current()) {
        bail!("{} can't start here: {why}", game.title);
    }
    check_components(game, paths)?;
    let comps = paths::components(paths);
    let host = host(paths);
    if rt.is_dir() {
        for gone in linkfarm::dangling(&rt)? {
            linkfarm::remove_link(&gone)?;
        }
    }
    let ops = runtime::plan(
        game,
        &entry.dir,
        &rt,
        &comps,
        catalog::Host::current(),
        &linkfarm::list,
    )
    .with_context(|| format!("reading {}", entry.dir.display()))?;
    linkfarm::apply(&rt, &ops, Occupied::Keep).context("building the runtime folder")?;
    let saves_dir = paths::saves(paths, game.slug);
    let synced =
        saves::sync_before(game, &rt, &saves_dir).context("syncing card and save files")?;
    if !synced.is_empty() {
        say_launcher(
            paths,
            &format!(
                "saves ({}): -> runtime {:?}, -> saves {:?}",
                game.slug, synced.to_runtime, synced.to_saves
            ),
        );
    }
    #[cfg(unix)]
    if game.route == Route::NativeN2 {
        let libso = runtime::plan_libso(&entry.dir.join("libso"), &rt.join("libso"))?;
        linkfarm::apply(&rt, &libso, Occupied::Keep)?;
        link_nvidia_glx32(&rt.join("glfix"))?;
    }
    if game.route == Route::System246 {
        prepare_sys246(paths, game, entry, &rt)?;
    }
    let plan = launch::plan_on(game, &rt, &paths::prefix(paths), settings, &host)
        .context("this game has no launch route yet")?;
    for (path, text) in &plan.files {
        fsx::write_atomic(path, text)?;
    }
    let block = launch::block_path(&rt);
    // a fresh block: everything released, quit clear
    fsx::write_atomic(&block, Snapshot::default().encode())?;
    say_launcher(
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
fn prepare_sys246(paths: &HubPaths, game: &Game, entry: &Entry<Game>, rt: &Path) -> Result<()> {
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
            fsx::write_atomic(&to, fs::read(fw.join(f.name))?)?;
        }
    }
    let card_file = data
        .join("memcards")
        .join(format!("{}.ps2", sys246::GAME_ID));
    if !card_file.is_file() {
        let zip = entry.dir.join(game.main_file);
        let card = sys246::dongle_from_zip(&zip)
            .with_context(|| format!("making the dongle card from {}", zip.display()))?;
        fsx::write_atomic(&card_file, card)?;
    }
    Ok(())
}

/// How this launcher starts the Windows games: directly on Windows; on
/// Linux under the bundled (or chosen) Wine, else `wine` from PATH.
pub fn host(paths: &HubPaths) -> Host {
    if cfg!(windows) {
        return Host::Windows;
    }
    match &paths::wine(paths) {
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

fn wine_bin(paths: &HubPaths, name: &str) -> PathBuf {
    match &paths::wine(paths) {
        Some(dir) => dir.join("bin").join(name),
        None => PathBuf::from(name),
    }
}

/// Whether the launcher's components `game` needs are all in place.
pub fn check_components(game: &Game, paths: &HubPaths) -> Result<()> {
    let c = &paths::components(paths);
    let host = &host(paths);
    let need: Vec<PathBuf> = match game.route {
        Route::Es3b => {
            let mut v = vec![
                c.openparrot_for(game).join("OpenParrot64.dll"),
                c.openparrot_for(game).join("OpenParrotLoader64.exe"),
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
            c.pcsx2x6.join(sys246::GAME_ID).join("boot.elf"),
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

/// Create and fix up the shared Wine prefix once (per [`PREFIX_VERSION`]).
pub fn prepare_prefix(paths: &HubPaths, wine_env: &[(String, String)]) -> Result<()> {
    let pfx = paths::prefix(paths);
    let marker = pfx.join(".rea-wangan-prefix");
    if fs::read_to_string(&marker).ok().as_deref() == Some(PREFIX_VERSION) {
        return Ok(());
    }
    fs::create_dir_all(&pfx)?;
    say_launcher(paths, "creating the Wine prefix");
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

fn run_wine(paths: &HubPaths, env: &[(String, String)], pfx: &Path, args: &[&str]) -> Result<()> {
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

fn wineserver_wait(paths: &HubPaths, pfx: &Path) {
    let _ = Command::new(wine_bin(paths, "wineserver"))
        .arg("-w")
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
pub fn script_text(plan: &Plan) -> String {
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

/// The game's own log of one run (its output, the board's, the
/// session's `#` lines), started afresh at every start.
pub fn session_log(paths: &HubPaths, game: &Game) -> PathBuf {
    paths.data.join(format!("{}.log", game.slug))
}

/// Runs a prepared game until it ends or `stop` is set, then ends
/// everything the session started. A game that ended by itself at once
/// didn't start: that is an error, with the session log's reasons.
/// `libraries` are bound read-only around the game (Linux).
pub fn run(
    paths: &HubPaths,
    game: &Game,
    p: &Prepared,
    display: Display,
    libraries: &[PathBuf],
    input: &Input,
    stop: &AtomicBool,
) -> Result<ExitStatus> {
    let (mut feed, sink) = input.feed(game.cabinet, &p.block)?;
    let feed = feed.as_mut();
    let log_path = session_log(paths, game);
    let start = |path: &Path| {
        fs::File::create(path)?;
        fs::OpenOptions::new().append(true).open(path)
    };
    let file = start(&log_path).with_context(|| format!("starting {}", log_path.display()))?;
    let log: Log = Arc::new(Mutex::new(file.try_clone()?));
    say_launcher(paths, &format!("starting {} ({display:?})", game.slug));
    let outcome = match p.plan.picture {
        Some(picture) => {
            let session = Session {
                game,
                p,
                display,
                libraries,
            };
            run_in_gamescope(paths, &session, picture, feed, options(stop, &log, sink))?
        }
        None => run_direct(p, &file, feed, options(stop, &log, sink), &log)?,
    };
    say_launcher(paths, &format!("{} ended: {}", game.slug, outcome.status));
    let saves_dir = paths::saves(paths, game.slug);
    match saves::sync_after(game, &p.runtime, &saves_dir) {
        Ok(copied) if !copied.is_empty() => {
            say_launcher(paths, &format!("saved ({}): {:?}", game.slug, copied));
        }
        Ok(_) => {}
        Err(e) => {
            say_launcher(
                paths,
                &format!("warning: couldn't sync saves for {}: {e}", game.slug),
            );
        }
    }
    if let Some(why) = didnt_start(&outcome, &fs::read_to_string(&log_path).unwrap_or_default()) {
        bail!("{why}");
    }
    Ok(outcome.status)
}

/// Why a game that ended by itself at once didn't start (a missing
/// library, a refused program), from its session log.
fn didnt_start(o: &Outcome, log: &str) -> Option<String> {
    o.ended_early(EARLY_END)
        .then(|| early_end(o.ran, &o.status.to_string(), log))
        .flatten()
}

fn options<'a>(stop: &'a AtomicBool, log: &Log, sink: Option<Sink>) -> Options<'a> {
    Options {
        stop,
        log: log.clone(),
        grace: GRACE,
        sink,
    }
}

/// What a Linux run starts.
struct Session<'a> {
    game: &'a Game,
    p: &'a Prepared,
    display: Display,
    libraries: &'a [PathBuf],
}

/// Linux: gamescope runs the session script; everything it starts carries
/// the session's tag.
fn run_in_gamescope(
    paths: &HubPaths,
    s: &Session,
    picture: Picture,
    feed: &mut dyn Feed,
    opts: Options,
) -> Result<Outcome> {
    let Session {
        game,
        p,
        display,
        libraries,
    } = *s;
    let mut wine = None;
    if let Some(prefix) = &p.plan.wine_prefix {
        let env = match host(paths) {
            Host::Linux { wine_env, .. } => wine_env,
            Host::Windows => vec![],
        };
        prepare_prefix(paths, &env)?;
        wine = Some(WineServer {
            program: wine_bin(paths, "wineserver"),
            prefix: prefix.clone(),
        });
    }
    if display == Display::Headless && game.route == Route::System246 {
        // test runs reach no sound server: pcsx2x6 would stop on an error box
        let ini = sys246::pcsx2_ini(sys246::upscale_for(p.plan.game_size.h), true);
        fsx::write_atomic(&launch::sys246_ini(&p.runtime), ini)?;
    }
    let script = p.runtime.join("session.sh");
    fsx::write_atomic(&script, script_text(&p.plan))?;
    let argv = command_line(
        picture,
        display,
        which("bwrap").is_some().then_some(libraries),
        &writable(paths),
        &script,
    );
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let group = Group::spawn(&mut cmd, &Tag::new(TAG, p.runtime.to_string_lossy()))
        .context("starting gamescope")?;
    let mut ending = Ending::new(&p.block, wine);
    Ok(supervise::run(group, feed, &mut ending, opts)?)
}

/// What a session writes, bound back writable when a game folder holds it
/// (`--library ~`): the launcher's data folder, with the runtimes and the
/// Wine prefix inside.
fn writable(paths: &HubPaths) -> Vec<PathBuf> {
    vec![paths.data.clone()]
}

/// The command gamescope starts: bubblewrap with `read_only` bound
/// read-only where present (no loader can write to the player's dumps; an
/// unplugged drive is skipped) and `writable` bound back, then `sh script`.
fn command_line(
    picture: Picture,
    display: Display,
    read_only: Option<&[PathBuf]>,
    writable: &[PathBuf],
    script: &Path,
) -> Vec<String> {
    CommandLine {
        gamescope: picture.args(display == Display::Headless),
        homes: None,
        read_only: read_only.map(<[PathBuf]>::to_vec),
        writable: writable.to_vec(),
        cpus: None,
    }
    .argv(Path::new("sh"), &[script.to_string_lossy().into_owned()])
}

fn which(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(program))
            .find(|p| p.is_file())
    })
}

/// A planned process as Windows starts it: its own folder, its output in
/// the session log (`log`, opened to append).
fn direct_command(c: &launch::Command, log: &fs::File) -> Result<Command> {
    let mut cmd = Command::new(&c.program);
    cmd.args(&c.args)
        .current_dir(&c.cwd)
        .envs(c.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log.try_clone()?);
    Ok(cmd)
}

/// Windows: the helpers (board, auth daemon), a pause after each, then the
/// loader, which waits for the game. Each is a group of its own (a job);
/// the helpers end after the game ([`end_helpers`]).
fn run_direct(
    p: &Prepared,
    file: &fs::File,
    feed: &mut dyn Feed,
    opts: Options,
    log: &Log,
) -> Result<Outcome> {
    let tag = Tag::new(TAG, p.runtime.to_string_lossy());
    let mut helpers = Vec::new();
    for h in &p.plan.helpers {
        let mut c = direct_command(h, file)?;
        helpers
            .push(Group::spawn(&mut c, &tag).with_context(|| format!("starting {}", h.program))?);
        std::thread::sleep(HELPER_PAUSE);
    }
    let mut c = direct_command(&p.plan.game, file)?;
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let group =
        Group::spawn(&mut c, &tag).with_context(|| format!("starting {}", p.plan.game.program))?;
    let mut ending = Ending::new(&p.block, None);
    let outcome = supervise::run(group, feed, &mut ending, opts)?;
    ending.end();
    end_helpers(helpers, &p.runtime, log);
    Ok(outcome)
}

/// The helpers, told to quit by [`Ending`], get [`GRACE`] to do so; then
/// what is left of them is ended, and whatever else runs from the runtime
/// folder (the auth daemon the loader started, should it have left its
/// job).
fn end_helpers(helpers: Vec<Group>, rt: &Path, log: &Log) {
    let deadline = Instant::now() + GRACE;
    while Instant::now() < deadline && helpers.iter().any(Group::any_left) {
        std::thread::sleep(Duration::from_millis(100));
    }
    #[cfg(windows)]
    if let Some(first) = helpers.first() {
        let n = first.kill_under(rt);
        if n > 0 {
            say(
                log,
                &format!("# stopped {n} leftover process(es) in the runtime"),
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (rt, log);
    for h in helpers {
        let _ = h.finish();
    }
}

#[cfg(windows)]
fn say(log: &Log, line: &str) {
    if let Ok(mut f) = log.lock() {
        let _ = writeln!(f, "{line}");
    }
}

/// The Wine server of the session's prefix.
struct WineServer {
    program: PathBuf,
    prefix: PathBuf,
}

impl WineServer {
    /// Ends every Wine process of the prefix, and the server (which saves
    /// the prefix's registry as it goes).
    fn kill(&self) {
        let _ = Command::new(&self.program)
            .arg("-k")
            .env("WINEPREFIX", &self.prefix)
            .status();
    }
}

/// How a session ends, in the order the cabinet needs: the game first (it
/// ended, or was asked to), then the I/O board, told through the block's
/// quit byte (stopping the board while the game runs shows E0303 on the
/// cabinet's screen), then Wine's server for the prefix.
struct Ending<'a> {
    block: &'a Path,
    wine: Option<WineServer>,
    done: bool,
}

impl<'a> Ending<'a> {
    fn new(block: &'a Path, wine: Option<WineServer>) -> Ending<'a> {
        Ending {
            block,
            wine,
            done: false,
        }
    }

    /// Tells the board, then Wine, to end (once).
    fn end(&mut self) {
        if std::mem::replace(&mut self.done, true) {
            return;
        }
        signal_quit(self.block);
        if let Some(wine) = &self.wine {
            std::thread::sleep(Duration::from_millis(300));
            wine.kill();
        }
    }
}

impl Watcher for Ending<'_> {
    /// The game is asked to stop and gets [`GRACE`] to go; then (gone, or
    /// its polite stop failed) the board and Wine are told to end.
    fn ask_to_stop(&mut self, group: &Group) {
        group.terminate();
        let deadline = Instant::now() + GRACE;
        while Instant::now() < deadline && game_runs(group) {
            std::thread::sleep(Duration::from_millis(50));
        }
        self.end();
    }

    /// What the game leaves (the board, Wine) is told to end and gets the
    /// grace to do so, rather than being killed at once.
    fn saving(&mut self, _: &Group) -> bool {
        self.end();
        true
    }

    fn saves_after_exit(&self) -> bool {
        true
    }
}

/// Whether the game still runs: on Linux gamescope, which leads the session
/// and goes with the game's window (a leader that has exited is a zombie,
/// never a member); on Windows the loader, which waits for the game, or
/// anything else of its group.
#[cfg(target_os = "linux")]
fn game_runs(group: &Group) -> bool {
    group.members().iter().any(|m| m.pid == group.id())
}

#[cfg(windows)]
fn game_runs(group: &Group) -> bool {
    group.any_left()
}

/// Sets the block's quit byte: the I/O board ends on it.
fn signal_quit(block: &Path) {
    if let Ok(mut f) = fs::OpenOptions::new().write(true).open(block) {
        use std::io::{Seek, SeekFrom, Write};
        let _ = f
            .seek(SeekFrom::Start(QUIT as u64))
            .and_then(|_| f.write_all(&[1]));
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
            picture: None,
            wine_prefix: None,
            game_size: rea_launcher::gamescope::Size::new(1360, 768),
        };
        let s = script_text(&plan);
        assert!(
            s.contains("( cd '/rt/it'\\''s' && env 'A=x y' 'wine' 'io.exe' ) >/dev/null 2>&1 &"),
            "{s}"
        );
        assert!(
            s.contains("cd '/rt' && exec env 'wine' 'Z:\\a b\\g.exe'"),
            "{s}"
        );
    }

    #[test]
    fn the_board_is_told_to_quit_once_the_game_has_ended() {
        let t = tempfile::tempdir().unwrap();
        let block = t.path().join("input.block");
        fs::write(&block, Snapshot::default().encode()).unwrap();
        let mut ending = Ending::new(&block, None);
        assert_eq!(fs::read(&block).unwrap()[QUIT], 0);
        ending.end();
        assert_eq!(fs::read(&block).unwrap()[QUIT], 1);
        // what the input thread writes afterwards never clears it
        fs::write(&block, Snapshot::default().encode()).unwrap();
        ending.end();
        assert_eq!(fs::read(&block).unwrap()[QUIT], 0, "told once");
        assert!(ending.saves_after_exit());
    }

    #[test]
    fn gamescope_runs_the_script_inside_bubblewrap_and_headless_on_request() {
        let picture = Settings::default().picture(launch::ES3B_SIZE);
        let script = Path::new("/rt/mt6/session.sh");
        let libraries = [PathBuf::from("/games"), PathBuf::from("/more games")];
        let none: [PathBuf; 0] = [];
        let screen =
            command_line(picture, Display::Screen, Some(&libraries), &none, script).join(" ");
        assert!(
            screen.starts_with("gamescope -w 1360 -h 768 -W 3840 -H 2160 -r 60 -F fsr"),
            "{screen}"
        );
        assert!(
            screen.ends_with(
                " -f -- env ENABLE_GAMESCOPE_WSI=0 DISABLE_GAMESCOPE_WSI=1 bwrap --dev-bind / / \
                 --ro-bind-try /games /games --ro-bind-try /more games /more games sh \
                 /rt/mt6/session.sh"
            ),
            "{screen}"
        );
        let headless = command_line(picture, Display::Headless, None, &none, script).join(" ");
        assert!(
            headless.ends_with(
                "-F linear --backend headless -- env ENABLE_GAMESCOPE_WSI=0 \
                 DISABLE_GAMESCOPE_WSI=1 sh /rt/mt6/session.sh"
            ),
            "{headless}"
        );
    }

    /// `--library ~`: the launcher's own folder is inside a game folder
    /// bound read-only, and must stay writable; a library on a drive that
    /// isn't there doesn't stop the game.
    #[test]
    fn the_launchers_folder_stays_writable_inside_a_read_only_library() {
        let picture = Settings::default().picture(launch::ES3B_SIZE);
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let paths = HubPaths {
            data: home.join(".local/share/rea-wangan"),
            components: t.path().join("c"),
            dev_assets: None,
        };
        std::fs::create_dir_all(&paths.data).unwrap();
        let libraries = [home.clone(), t.path().join("unplugged")];
        let line = command_line(
            picture,
            Display::Headless,
            Some(&libraries),
            &writable(&paths),
            Path::new("/rt/session.sh"),
        )
        .join(" ");
        let (h, d, u) = (home.display(), paths.data.display(), libraries[1].display());
        assert!(
            line.contains(&format!(
                "bwrap --dev-bind / / --ro-bind-try {h} {h} --ro-bind-try {u} {u} --bind {d} {d} sh"
            )),
            "{line}"
        );
    }

    /// The same, run: bubblewrap starts with a missing folder and writes
    /// into the launcher's folder inside a read-only one.
    #[test]
    #[cfg(target_os = "linux")]
    fn bubblewrap_writes_the_launchers_folder_and_skips_a_missing_drive() {
        if which("bwrap").is_none() {
            return;
        }
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let paths = HubPaths {
            data: home.join("data"),
            components: t.path().join("c"),
            dev_assets: None,
        };
        std::fs::create_dir_all(&paths.data).unwrap();
        let line = CommandLine {
            gamescope: vec![],
            homes: None,
            read_only: Some(vec![home.clone(), t.path().join("unplugged")]),
            writable: writable(&paths),
            cpus: None,
        }
        .argv(
            Path::new("sh"),
            &["-c".into(), "echo ok > data/x && ! echo no > y".into()],
        );
        // past gamescope: bubblewrap and the shell
        let at = line.iter().position(|a| a == "bwrap").unwrap();
        let st = Command::new(&line[at])
            .args(&line[at + 1..])
            .current_dir(&home)
            .stderr(Stdio::null())
            .status()
            .unwrap();
        if !st.success() && !paths.data.join("x").exists() {
            // no user namespaces here: nothing to check
            return;
        }
        assert!(st.success());
        assert!(
            paths.data.join("x").is_file(),
            "the launcher's folder is written"
        );
        assert!(!home.join("y").exists(), "the game folder stays read-only");
    }

    #[test]
    fn a_game_that_ended_by_itself_at_once_didnt_start_and_says_why() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        let outcome = |secs, ended| Outcome {
            status: ExitStatus::from_raw(1),
            ran: Duration::from_secs(secs),
            ended,
            leftovers: 0,
        };
        let log = "# gamescope\nerror while loading shared libraries: libGL.so.1\n";
        let why = didnt_start(&outcome(2, supervise::Ended::Itself), log).unwrap();
        assert!(why.contains("closed by itself 2 s after starting"), "{why}");
        assert!(why.contains("libGL.so.1"), "{why}");
        assert_eq!(
            didnt_start(&outcome(600, supervise::Ended::Itself), log),
            None
        );
        assert_eq!(
            didnt_start(&outcome(2, supervise::Ended::Stopped), log),
            None
        );
    }

    /// The board is told to quit once the game has gone, not while it
    /// still runs (its cabinet would show E0303).
    #[test]
    #[cfg(target_os = "linux")]
    fn a_stop_tells_the_board_only_once_the_game_is_gone() {
        let t = tempfile::tempdir().unwrap();
        let block = t.path().join("input.block");
        fs::write(&block, Snapshot::default().encode()).unwrap();
        // a game that takes a second to go after the polite stop
        let mut game = Command::new("sh");
        game.args(["-c", "trap '' TERM; sleep 1"]);
        let tag = Tag::new(TAG, t.path().to_string_lossy());
        let group = Group::spawn(&mut game, &tag).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        let mut ending = Ending::new(&block, None);
        ending.ask_to_stop(&group);
        assert!(
            start.elapsed() >= Duration::from_millis(700),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(fs::read(&block).unwrap()[QUIT], 1);
    }
}
