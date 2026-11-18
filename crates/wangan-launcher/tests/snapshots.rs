//! Golden snapshots (rea-launcher-sdk docs/STANDARDS.md, "Golden
//! snapshots"): everything the launcher produces, compared with the files
//! in `tests/snapshots/`, so an SDK bump or a refactor can't change it
//! silently. A diff is reviewed like code: it must match an intended change
//! or an SDK Migration item.
//!
//! `REA_UPDATE_SNAPSHOTS=1 cargo test --test snapshots` rewrites them. The
//! window's own snapshots are in `src/ui/window_tests.rs`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use rea_launcher::gamescope::{Size, Upscaler};
use rea_launcher::linkfarm::{Item, Op};
use rea_launcher::wheel::Control as _;
use wangan_core::catalog::{self, GAMES};
use wangan_core::input::{Button, default_pad_map};
use wangan_core::launch::{self, Host};
use wangan_core::runtime::{self, Components};
use wangan_core::settings::Settings;
use wangan_jvs::board::{Board, NAMCO_WMMT};
use wangan_jvs::controls::{Cabinet, Controls};
use wangan_jvs::{chihiro, frame};

/// A path as the snapshot spells it on every system (Windows joins with `\`).
fn p(path: &Path) -> String {
    slashes(&path.display().to_string())
}

fn slashes(s: &str) -> String {
    s.replace('\\', "/")
}

/// The platform these tests run on, for snapshots that differ by platform.
fn host_name() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

// ---------------------------------------------------------------- the command line

/// A fixture: a data folder, no components, and a games folder holding
/// synthetic folders (never game files: their digests are unknown builds).
struct Fixture {
    t: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let t = tempfile::tempdir().unwrap();
        let games = t.path().join("games");
        for (dir, file, bytes) in [
            ("MT6 [TP]", "wmn6r.exe", &b"not the game"[..]),
            ("Maximum Tune 3", "main", b"\x7fELF not the game"),
            ("Wangan R", "wanganmr.zip", b"PK not the game"),
        ] {
            std::fs::create_dir_all(games.join(dir)).unwrap();
            std::fs::write(games.join(dir).join(file), bytes).unwrap();
        }
        std::fs::create_dir_all(games.join("empty")).unwrap();
        // board files of the right sizes but the wrong bytes
        let files = t.path().join("bios");
        std::fs::create_dir_all(files.join("sub")).unwrap();
        std::fs::write(files.join("r27v1602f.7d"), vec![0u8; 2_097_152]).unwrap();
        std::fs::write(files.join("sub/eeprom.bin"), vec![1u8; 8_192]).unwrap();
        std::fs::write(files.join("notes.txt"), b"hello").unwrap();
        Self { t }
    }

    fn root(&self) -> &Path {
        self.t.path()
    }

    fn data(&self) -> PathBuf {
        self.root().join("data")
    }

    /// Runs the launcher with `args`, with no display and a lock folder of
    /// its own. `--list` and `--check` get `--linux` (not shown: their
    /// output is read, so a refusal is never a message window, which would
    /// wait forever under Wine); a run without the window never shows one.
    fn run(&self, args: &[&str]) -> String {
        let run_dir = self.root().join("run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let command = args.iter().any(|a| matches!(*a, "--list" | "--check"));
        let out = Command::new(env!("CARGO_BIN_EXE_wangan-launcher"))
            .args(command.then_some("--linux"))
            .args(args)
            .env("WANGAN_DATA", self.data())
            .env("WANGAN_COMPONENTS", self.root().join("none"))
            .env("XDG_RUNTIME_DIR", run_dir)
            .env_remove("WANGAN_WINE")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .expect("run the launcher");
        let shown = args
            .iter()
            .map(|a| self.tidy(a))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "$ wangan-launcher {shown}\nexit {:?}\n-- stdout\n{}-- stderr\n{}\n",
            out.status.code(),
            self.tidy(&String::from_utf8_lossy(&out.stdout)),
            self.tidy(&String::from_utf8_lossy(&out.stderr)),
        )
    }

    /// The fixture's own folder as `<tmp>`, `/` for `\`.
    fn tidy(&self, s: &str) -> String {
        slashes(&s.replace(&self.root().display().to_string(), "<tmp>"))
    }

    fn arg(&self, rel: &str) -> String {
        self.root().join(rel).display().to_string()
    }

    fn file(&self, rel: &str) -> String {
        let text = std::fs::read_to_string(self.data().join(rel)).unwrap_or_default();
        format!("== <tmp>/data/{rel}\n{}\n", self.tidy(&text))
    }
}

#[test]
fn help() {
    let f = Fixture::new();
    let out = Command::new(env!("CARGO_BIN_EXE_wangan-launcher"))
        .arg("--help")
        .env("WANGAN_DATA", f.data())
        .output()
        .expect("run the launcher");
    assert!(out.status.success());
    rea_launcher::snapshot!("help.txt", String::from_utf8_lossy(&out.stdout));
}

/// Every mode of the command line on the fixture: what it prints, its exit
/// code and the files it writes. The window opens without arguments, so
/// that one is left to the window tests.
#[test]
fn every_command_line_mode() {
    let f = Fixture::new();
    let games = f.arg("games");
    let bios = f.arg("bios");
    let mut out = String::new();
    for args in [
        vec!["-h"],
        vec!["--nope"],
        vec!["--library"],
        vec!["--play"],
        vec!["--game"],
        vec!["--add-files"],
        vec!["--game", "mt5", "--output"],
        vec!["--set", "mt5"],
        vec!["--list", "--check"],
        vec!["--list"],
        vec!["--check"],
        vec!["--play", "mt5"],
        vec!["--library", "/no/such/folder"],
    ] {
        out += &f.run(&args);
    }
    // launcher.cfg keeps the player's other lines
    std::fs::create_dir_all(f.data()).unwrap();
    std::fs::write(
        f.data().join("launcher.cfg"),
        "# my note\nlibrary = /old/games\nfuture = 1\n",
    )
    .unwrap();
    out += &f.run(&["--library", &games]);
    out += &f.file("launcher.cfg");
    out += &f.run(&["--list"]);
    out += &f.run(&["--check"]);
    out += &f.run(&["--check", "mt6"]);
    out += &f.run(&["--check", "mt5"]);
    out += &f.run(&["--library", &games, "--list"]);
    out += &f.file("launcher.cfg");
    for args in [
        vec!["--add-files", "/no/such/folder"],
        vec!["--add-files", &bios],
        vec!["--play", "nope"],
        vec!["--play", "mt1"],
        vec!["--play", "mt5", "--no-input"],
        vec!["--play", "mt3", "--headless", "--no-input"],
        vec!["--play", "wangan-r", "--no-input"],
        vec!["--output", "1920x1080"],
        vec!["--game", "nope", "--deadzone", "0.1"],
        vec!["--game", "mt5", "--deadzone", "0.9"],
        vec!["--game", "mt5", "--render", "1x1"],
        vec!["--game", "mt5", "--upscaler", "bicubic"],
        vec!["--game", "mt5", "--fullscreen", "maybe"],
        vec!["--game", "mt5", "--sharpness", "21"],
    ] {
        out += &f.run(&args);
    }
    // the options land in the game's settings file, keeping its notes
    std::fs::create_dir_all(f.data().join("settings")).unwrap();
    std::fs::write(
        f.data().join("settings/mt3.cfg"),
        "# mine\nsharpness = 7\nfuture_key = yes\n",
    )
    .unwrap();
    out += &f.run(&[
        "--game",
        "mt3",
        "--render",
        "1920x1080",
        "--deadzone",
        "0.12",
    ]);
    out += &f.file("settings/mt3.cfg");
    out += &f.run(&[
        "--game",
        "mt3",
        "--output",
        "2560x1440",
        "--upscaler",
        "nis",
        "--sharpness",
        "3",
        "--fullscreen",
        "off",
    ]);
    out += &f.file("settings/mt3.cfg");
    out += &f.run(&["--game", "mt6rr", "--render", "native", "--deadzone", "0"]);
    out += &f.file("settings/mt6rr.cfg");
    // the window shows the render size off for this game: so does the command line
    out += &f.run(&["--game", "mt5", "--render", "1920x1080"]);
    out += &f.file("settings/mt5.cfg");
    out += &f.run(&["--set", "mt3", "--look", "steam_deck", "--deadzone=0.2"]);
    out += &f.file("settings/mt3.cfg");
    rea_launcher::snapshot!(&format!("cli-{}.txt", host_name()), out);
}

/// While another launcher holds the lock, `--list` still reads, but a
/// command that writes `launcher.cfg` is refused.
#[test]
#[cfg(unix)]
fn only_a_command_that_writes_runs_beside_an_open_launcher() {
    let f = Fixture::new();
    let lock = f.root().join("run/rea-launcher-rea-wangan-launcher.lock");
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let _held = rea_launcher::instance::single_at(&lock).expect("the lock");
    let games = f.arg("games");
    let list = f.run(&["--list"]);
    assert!(
        list.contains("exit Some(1)") && list.contains("No game folder yet"),
        "{list}"
    );
    let write = f.run(&["--list", "--library", &games]);
    assert!(write.contains("exit Some(5)"), "{write}");
    assert!(
        !f.data().join("launcher.cfg").exists(),
        "nothing written beside it"
    );
}

// ---------------------------------------------------------------- settings

#[test]
fn the_settings_file_written_from_defaults() {
    rea_launcher::snapshot!(
        "settings.cfg.txt",
        Settings::default().save("", &wangan_core::settings::ALL_KEYS)
    );
}

/// A player's file: comments, blank lines, unknown and old keys, a key
/// commented out, odd spacing, a bad value and a repeated key.
#[test]
fn a_settings_file_round_trip() {
    let text = "\
# my Wangan notes
output=2560X1440

  upscaler =  nis
fsr_sharpness = 3
# output = 1920x1080
sharpness = 50
render = native
deadzone = 0.1
no equals sign here
deadzone = 0.15
future_key = keep me
";
    let (mut settings, problems) = Settings::parse(text);
    // the window changes one row and saves it
    settings.deadzone = 0.2;
    let saved = settings.save(text, &["deadzone"]);
    let mut out = format!("== parsed\n{settings:#?}\n== problems\n");
    for problem in &problems {
        let _ = writeln!(out, "{problem}");
    }
    let _ = write!(out, "== saved\n{saved}");
    let (again, problems) = Settings::parse(&saved);
    assert_eq!(again, settings);
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert_eq!(
        again.save(&saved, &["deadzone"]),
        saved,
        "a second save changes nothing"
    );
    rea_launcher::snapshot!("settings-round-trip.txt", out);
}

// ---------------------------------------------------------------- launch plans

fn settings_cases() -> Vec<(&'static str, Settings)> {
    vec![
        ("defaults", Settings::default()),
        (
            "custom",
            Settings {
                look: None,
                render: Some(Size::new(1920, 1080)),
                output: Size::new(2560, 1440),
                upscaler: Upscaler::Nis,
                sharpness: 7,
                fullscreen: false,
                deadzone: 0.12,
            },
        ),
        (
            "smooth-720p",
            Settings {
                look: None,
                render: None,
                output: Size::new(1280, 720),
                upscaler: Upscaler::Linear,
                sharpness: 0,
                fullscreen: true,
                deadzone: 0.0,
            },
        ),
    ]
}

fn hosts() -> Vec<(&'static str, Host)> {
    vec![
        ("linux, wine from PATH", Host::system_wine()),
        (
            "linux, bundled wine",
            Host::Linux {
                wine: "/pkg/components/wine/bin/wine".into(),
                wine_env: vec![(
                    "LD_LIBRARY_PATH".into(),
                    "/pkg/components/wine/lib/ffmpeg:/usr/lib".into(),
                )],
            },
        ),
        ("windows", Host::Windows),
    ]
}

/// A file's text once; later plans writing the same text point back to it.
fn file_text(out: &mut String, seen: &mut Vec<(String, String)>, label: &str, text: &str) {
    if let Some((first, _)) = seen.iter().find(|(_, t)| t == text) {
        let _ = writeln!(out, "(same text as in {first})");
    } else {
        let _ = write!(out, "{}", slashes(text));
        seen.push((label.to_string(), text.to_string()));
    }
}

/// What each game writes and starts, for each host and a few settings:
/// files, helpers (the I/O board, the auth daemon), the game's command,
/// environment (the input block's path included), gamescope.
#[test]
fn every_launch_plan() {
    let mut out = String::new();
    let mut seen = Vec::new();
    for g in GAMES {
        let rt = PathBuf::from("/data/runtimes").join(g.slug);
        for (host_label, host) in hosts() {
            for (case, s) in settings_cases() {
                let label = format!("{} / {host_label} / {case}", g.slug);
                let _ = writeln!(out, "== {label}");
                let Some(plan) = launch::plan_on(g, &rt, Path::new("/data/prefix"), &s, &host)
                else {
                    let _ = writeln!(out, "no plan");
                    continue;
                };
                let _ = writeln!(out, "size {}", plan.game_size);
                let gamescope = plan.picture.map(|p| p.args(false).join(" "));
                let _ = writeln!(out, "gamescope {}", gamescope.unwrap_or_default());
                if let Some(prefix) = &plan.wine_prefix {
                    let _ = writeln!(out, "wine prefix {}", p(prefix));
                }
                for (i, helper) in plan.helpers.iter().enumerate() {
                    write_command(&mut out, &format!("helper {i}"), helper);
                }
                write_command(&mut out, "game", &plan.game);
                for (path, text) in &plan.files {
                    let _ = writeln!(out, "file {}", p(path));
                    file_text(&mut out, &mut seen, &label, text);
                }
            }
        }
    }
    rea_launcher::snapshot!("plans.txt", out);
}

fn write_command(out: &mut String, what: &str, c: &launch::Command) {
    let _ = writeln!(
        out,
        "{what}: {} {}",
        slashes(&c.program),
        slashes(&c.args.join(" "))
    );
    let _ = writeln!(out, "  cwd {}", p(&c.cwd));
    for (k, v) in &c.env {
        let _ = writeln!(out, "  env {k}={}", slashes(v));
    }
}

/// The paths a session derives from a runtime, and the System 246 files
/// for every window height step.
#[test]
fn runtime_paths_and_system_246_files() {
    let rt = Path::new("/data/runtimes/wangan-r");
    let mut out = String::new();
    let _ = writeln!(out, "block {}", p(&launch::block_path(rt)));
    let _ = writeln!(out, "acgame {}", p(&launch::sys246_acgame(rt)));
    let _ = writeln!(out, "ini {}", p(&launch::sys246_ini(rt)));
    let _ = writeln!(
        out,
        "wine path {}",
        launch::wine_path(Path::new("/data/x/y.exe"))
    );
    for h in [240, 480, 720, 768, 1080, 1440, 2160, 4320] {
        let _ = writeln!(
            out,
            "upscale for {h} lines: {}",
            wangan_core::sys246::upscale_for(h)
        );
    }
    let _ = write!(out, "== acgame\n{}", wangan_core::sys246::acgame());
    let _ = write!(
        out,
        "== PCSX2.ini, null audio\n{}",
        wangan_core::sys246::pcsx2_ini(3, true)
    );
    rea_launcher::snapshot!("sys246.txt", out);
}

// ---------------------------------------------------------------- runtime plans

fn item(name: &str, dir: bool, size: u64) -> Item {
    Item {
        name: name.into(),
        dir,
        size,
    }
}

/// A game folder's listing as each route's dumps look (names only, no
/// game files): what is linked, copied, refreshed and skipped.
fn listing(slug: &str) -> (Vec<Item>, Vec<Item>) {
    const BIG: u64 = 64 << 20;
    match slug {
        "mt3" | "mt3dxp" => (
            vec![
                item("config.toml", false, 200),
                item("data", true, 0),
                item("keyconfig.toml", false, 200),
                item("libso", true, 0),
                item("main", false, BIG),
                item("plugins", true, 0),
                item("readme.txt", false, 10),
                item("tmp", true, 0),
            ],
            vec![],
        ),
        "wangan-r" => (
            vec![item("wanganmr", true, 0), item("wanganmr.zip", false, 300)],
            vec![],
        ),
        _ => (
            vec![
                item("AMCUS", true, 0),
                item("BK_0001.dat", false, 4096),
                item("D3D9.dll", false, 100),
                item("TP", true, 0),
                item("[SYSTEM]", true, 0),
                item("bngrw.dll", false, 100),
                item("card.ini", false, 10),
                item("d3d11.dll", false, 100),
                item("data", true, 0),
                item("data_jp", true, 0),
                item("data_jp_ori", true, 0),
                item("html", true, 0),
                item("setup.ps1", false, 10),
                item("shot.jpg", false, 10),
                item("teknoparrot.ini", false, 10),
                item("wmn6r.exe", false, BIG),
            ],
            vec![
                item("network", true, 0),
                item("sound", true, 0),
                item("movie.bin", false, BIG),
            ],
        ),
    }
}

fn components() -> Components {
    let c = Path::new("/pkg/components");
    Components {
        openparrot: c.join("openparrot"),
        openparrot_6rr: c.join("openparrot-6rr"),
        dxvk: c.join("dxvk"),
        io_board: c.join("io"),
        wal: c.join("wal"),
        pcsx2x6: c.join("pcsx2x6"),
    }
}

fn write_op(out: &mut String, op: &Op) {
    let _ = match op {
        Op::Dir(d) => writeln!(out, "dir {}", p(d)),
        Op::Link { target, at } => writeln!(out, "link {} -> {}", p(at), p(target)),
        Op::CopyOnce { from, to } => writeln!(out, "copy once {} <- {}", p(to), p(from)),
        Op::TreeOnce { from, to } => writeln!(out, "tree once {} <- {}", p(to), p(from)),
        Op::Refresh { from, to, exec } => {
            let x = if *exec { " (exec)" } else { "" };
            writeln!(out, "refresh {} <- {}{x}", p(to), p(from))
        }
        other => writeln!(out, "{other:?}"),
    };
}

/// The runtime folder each game gets, on each platform.
#[test]
fn every_runtime_plan() {
    let mut out = String::new();
    for platform in [catalog::Host::Linux, catalog::Host::Windows] {
        for g in GAMES {
            let _ = writeln!(out, "== {} on {platform:?}", g.slug);
            let (items, jp) = listing(g.slug);
            let src = PathBuf::from("/games").join(g.slug);
            let rt = PathBuf::from("/data/runtimes").join(g.slug);
            let ls = |dir: &Path| {
                Ok(match dir.strip_prefix(&src).unwrap().to_str() {
                    Some("") => items.clone(),
                    Some("data_jp") => jp.clone(),
                    _ => vec![],
                })
            };
            let ops = runtime::plan(g, &src, &rt, &components(), platform, &ls).unwrap();
            for op in ops {
                write_op(&mut out, &op);
            }
        }
    }
    rea_launcher::snapshot!("runtime-plans.txt", out);
}

/// A 32-bit little-endian ELF shared object with a DT_SONAME, as small as
/// [`runtime::elf32_soname`] reads.
fn elf32_with_soname(soname: &str) -> Vec<u8> {
    let mut b = vec![0u8; 0x100];
    b[..4].copy_from_slice(b"\x7FELF");
    b[4] = 1; // ELFCLASS32
    b[5] = 1; // little-endian
    let put = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut b, 28, 0x34); // program headers
    b[42..44].copy_from_slice(&32u16.to_le_bytes());
    b[44..46].copy_from_slice(&2u16.to_le_bytes());
    // PT_LOAD over the whole file at address 0
    put(&mut b, 0x34, 1);
    put(&mut b, 0x34 + 16, 0x100);
    // PT_DYNAMIC at 0x80
    put(&mut b, 0x54, 2);
    put(&mut b, 0x54 + 4, 0x80);
    put(&mut b, 0x54 + 16, 24);
    // DT_STRTAB 0xC0, DT_SONAME at 1 in it, DT_NULL
    put(&mut b, 0x80, 5);
    put(&mut b, 0x84, 0xC0);
    put(&mut b, 0x88, 14);
    put(&mut b, 0x8C, 1);
    b[0xC1..0xC1 + soname.len()].copy_from_slice(soname.as_bytes());
    b
}

/// The `libso/` links of an N2 runtime from a synthetic folder: real ELF
/// libraries under their names and SONAMEs; stubs, empty files and the
/// dump's libstdc++/libz left out.
#[test]
fn the_n2_libso_plan() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("libso");
    std::fs::create_dir_all(&src).unwrap();
    for (name, bytes) in [
        (
            "libboost_thread.so.1.40.0",
            elf32_with_soname("libboost_thread.so.1.40.0"),
        ),
        (
            "libcrypto.so.0.9.8",
            elf32_with_soname("libcrypto.so.0.9.8"),
        ),
        ("libGLEW.so.1.5.1", elf32_with_soname("libGLEW.so.1.5")),
        ("libGLEW.so.1.5", Vec::new()),
        ("libfoo.so", b"INPUT(libfoo.so.1)".to_vec()),
        ("libnoname.so.2", {
            let mut b = elf32_with_soname("");
            b[0x88] = 0; // DT_SONAME → DT_NULL
            b
        }),
        ("libstdc++.so.6", elf32_with_soname("libstdc++.so.6")),
        ("libz.so.1", elf32_with_soname("libz.so.1")),
    ] {
        std::fs::write(src.join(name), bytes).unwrap();
    }
    let ops = runtime::plan_libso(&src, Path::new("/data/runtimes/mt3/libso")).unwrap();
    let mut out = String::new();
    for op in &ops {
        write_op(&mut out, op);
    }
    let out = out.replace(&p(t.path()), "<tmp>");
    rea_launcher::snapshot!("libso-plan.txt", out);
}

// ---------------------------------------------------------------- the input block and the board

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sample_controls() -> Vec<(&'static str, Controls)> {
    vec![
        ("released", Controls::default()),
        (
            "full left, gas, 3rd gear, view",
            Controls {
                wheel: -1.0,
                gas: 1.0,
                gear: 3,
                view: true,
                ..Controls::default()
            },
        ),
        (
            "half right, half brake, 6th, intrude, card",
            Controls {
                wheel: 0.5,
                brake: 0.5,
                gear: 6,
                intrude: true,
                card: true,
                ..Controls::default()
            },
        ),
        (
            "test menu",
            Controls {
                test: true,
                service: true,
                menu_up: true,
                menu_down: true,
                enter: true,
                ..Controls::default()
            },
        ),
    ]
}

/// The block the launcher writes and the I/O board (or the N2 loader, or
/// the Chihiro bridge) reads, byte for byte, and what the board answers a
/// game's poll with: the whole controls → bytes wiring as data.
#[test]
fn the_input_block_and_the_board() {
    let mut out = String::new();
    let fresh = wangan_jvs::shared::Snapshot::default().encode();
    let _ = writeln!(out, "fresh block\n  {}", hex(&fresh));
    // a poll: switches (2 players, 2 bytes), coins (2 slots), analogs (3)
    let poll = {
        let payload = [0x20, 0x02, 0x02, 0x21, 0x02, 0x22, 0x03];
        let len = payload.len() as u8 + 1;
        let mut v = vec![frame::SYNC, 0x01, len];
        v.extend_from_slice(&payload);
        v.push(frame::checksum(0x01, len, &payload));
        v
    };
    let _ = writeln!(out, "poll {}", hex(&poll));
    for cabinet in [Cabinet::Wmmt5, Cabinet::Wmmt6RR] {
        for (what, c) in sample_controls() {
            let s = c.to_snapshot(cabinet, [2, 0, 0, 0], 7);
            let mut board = Board::new(NAMCO_WMMT);
            let reply = board.reply(&poll, &s).reply.unwrap_or_default();
            let _ = writeln!(out, "== {cabinet:?}: {what}");
            let _ = writeln!(out, "  block {}", hex(&s.encode()));
            let _ = writeln!(out, "  board reply {}", hex(&reply));
            let _ = writeln!(
                out,
                "  chihiro control {:08x} analog {}",
                chihiro::control(&s),
                hex(&chihiro::analog(&s))
            );
        }
    }
    let _ = writeln!(out, "== default pad map");
    let map = default_pad_map();
    let _ = writeln!(out, "{map:#?}");
    let _ = writeln!(out, "== buttons");
    for b in Button::ALL {
        let _ = writeln!(out, "{} {:#06x} {}", b.key(), b.bit(), b.label());
    }
    rea_launcher::snapshot!("input-block.txt", out);
}
