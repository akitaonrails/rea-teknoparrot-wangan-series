//! wangan-launcher: one launcher for the Wangan Midnight arcade series.
//! This file wires the command line (`rea_launcher::hub::cli`, plus
//! `--add-files`) to the window, the sessions and the messages; the
//! decisions live in `wangan-core`.

// a GUI program on Windows: no console window on a double click; commands
// reach the terminal that started them (console::attach_parent)
#![cfg_attr(windows, windows_subsystem = "windows")]

mod input;
mod paths;
mod session;
mod ui;

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rea_launcher::cfg::Value;
use rea_launcher::hub::cli::{
    self, BAD_COMMAND_LINE, CliSpec, Invocation, Mode, OK, REFUSED, TestOption,
};
use rea_launcher::hub::{HubConfig, HubPaths, PathKind, display_path, typed_path};
use rea_launcher::library::{Found, HashCache, Scan};
use rea_launcher::log::Log;
use rea_launcher::wheel::Script;
use rea_launcher_ui::report::{self, Link};
use rea_launcher_win32::report::{Audience, Mode as Says, Run};
use wangan_core::catalog::{self, GAMES, Game, Host, Support};
use wangan_core::firmware::{self, FileState};
use wangan_core::input::Button;
use wangan_core::settings::{self, Look, Settings};

use crate::session::{Display, Input};

/// The window's title; it differs from every game's (STANDARDS "One instance").
pub const WINDOW_TITLE: &str = "Wangan Midnight";
/// The single-instance lock's name.
const INSTANCE: &str = "rea-wangan-launcher";
const ISSUES: &str =
    "https://github.com/akitaonrails/rea-teknoparrot-wangan-series/issues/new/choose";

/// The games' ids, as `--play` takes them.
const SLUGS: [&str; GAMES.len()] = {
    let mut v = [""; GAMES.len()];
    let mut i = 0;
    while i < v.len() {
        v[i] = GAMES[i].slug;
        i += 1;
    }
    v
};

/// Whether game `id` takes option `key` here (the rows its window can change).
fn offers(id: &str, key: &str) -> bool {
    catalog::by_slug(id).is_some_and(|g| ui::form::offers(g, key, Host::current()))
}

/// What the command line knows.
const CLI: CliSpec = CliSpec {
    program: "wangan-launcher",
    about: "Wangan Midnight arcade series",
    games: &SLUGS,
    options: settings::OPTIONS,
    style: settings::STYLE,
    paths: paths::FIELDS,
    offers: Some(offers),
    test_options: &[TestOption {
        flag: "--input-script",
        help: "timed inputs, into the input block (not Wangan R)",
        refuses: Some(no_input_block),
    }],
};

/// Why game `id` can't take `--input-script`: Wangan R's emulator reads the
/// pad itself, not the launcher's input block.
fn no_input_block(id: &str) -> Option<&'static str> {
    (id == "wangan-r").then_some("Wangan Midnight R's emulator reads the pad itself")
}

/// `--add-files`, which the hub's command line doesn't have: a search, not
/// a setting.
const ADD_FILES: &str = "--add-files";

/// `--help`: the hub's usage and `--add-files`.
fn usage() -> String {
    format!(
        "{}\nBoard files:\n  wangan-launcher --add-files <path>  look in a folder or zip for the \
         files Wangan Midnight R and Maximum Tune need\n",
        cli::usage(&CLI)
    )
}

/// The path `--add-files` names, and the other arguments.
fn split_add_files(args: &[String]) -> Result<(Option<String>, Vec<String>), String> {
    let mut rest = Vec::new();
    let mut add = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.strip_prefix(ADD_FILES) {
            Some("") => {
                add = Some(
                    it.next()
                        .ok_or("--add-files needs a folder or zip")?
                        .clone(),
                )
            }
            Some(v) if v.starts_with('=') => add = Some(v[1..].to_string()),
            _ => rest.push(a.clone()),
        }
    }
    Ok((add, rest))
}

/// A game's settings ([`Settings::load`]: the first time, the recommended
/// look for `screen`); each line its file couldn't read goes to `warn`.
fn load_settings(
    paths: &HubPaths,
    game: &Game,
    screen: Option<(u32, u32)>,
    warn: impl Fn(&str),
) -> Result<Settings, String> {
    let file = paths.settings(game.slug);
    let (s, problems) = Settings::load(&file, game, screen)
        .map_err(|e| format!("couldn't read {}: {e}", display_path(&file)))?;
    problems.iter().for_each(|p| warn(p));
    Ok(s)
}

/// A warning of the command line, on stderr.
fn warn(text: &str) {
    eprintln!("warning: {text}");
}

/// The family's view of this run, for who sees its messages.
fn run_of(inv: &Invocation) -> Run {
    Run {
        command: inv.is_command(),
        no_window: inv.no_window(),
        settings_only: false,
        linux: inv.linux,
    }
}

fn main() -> ExitCode {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        rea_launcher_win32::console::attach_parent();
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = split_add_files(&args).and_then(|(add, rest)| {
        let inv = cli::parse(&CLI, rest)?;
        match (&add, &inv.mode) {
            (Some(_), Mode::Window | Mode::Configure) => Ok((add, inv)),
            (Some(_), _) => Err("--add-files goes alone (or with --library)".into()),
            (None, _) => Ok((add, inv)),
        }
    });
    let (add_files, inv) = match parsed {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}\n\n{}", usage());
            return ExitCode::from(BAD_COMMAND_LINE);
        }
    };
    if inv.mode == Mode::Help {
        print!("{}", usage());
        return ExitCode::from(OK);
    }
    let paths = HubPaths::discover(&paths::dirs());
    let log = Log::new(paths.log());
    // a search for board files is a command without the window: its output
    // is its result
    let run = Run {
        command: run_of(&inv).command || add_files.is_some(),
        no_window: run_of(&inv).no_window || add_files.is_some(),
        ..run_of(&inv)
    };
    let says = match Audience::of(run, false).mode() {
        Says::Command { interactive } => report::Mode::Command { interactive },
        Says::Play { interactive } => report::Mode::Play { interactive },
    };
    let fail = |text: &str| {
        let link = Link {
            label: "Report a problem",
            url: ISSUES,
        };
        rea_launcher_ui::hub::fatal(&ui::THEME, &log, text, Some(link), says);
    };
    // --list only reads: it works while the window is open; what writes
    // (a library folder, board files) takes the lock
    let writes = !inv.libraries.is_empty() || add_files.is_some();
    let _instance = if inv.mode == Mode::List && !writes {
        None
    } else {
        match rea_launcher::instance::single(INSTANCE) {
            Ok(guard) => Some(guard),
            Err(running) if inv.is_action() || writes => {
                fail(
                    "The Wangan Midnight launcher is already open. Close it first, then try again.",
                );
                return ExitCode::from(running.exit_code() as u8);
            }
            Err(_) => {
                eprintln!("The Wangan Midnight launcher is already open.");
                return ExitCode::from(OK);
            }
        }
    };
    match run_mode(&paths, &inv, add_files.as_deref()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            fail(&e);
            ExitCode::from(REFUSED)
        }
    }
}

fn run_mode(paths: &HubPaths, inv: &Invocation, add_files: Option<&str>) -> Result<u8, String> {
    let mut config = paths::config(paths)
        .map_err(|e| format!("couldn't read {}: {e}", display_path(&paths.config_file())))?;
    if !inv.libraries.is_empty() {
        configure(paths, &mut config, inv)?;
    }
    if let Some(src) = add_files {
        return add_board_files(paths, src);
    }
    match &inv.mode {
        Mode::Window => ui::run(paths.clone()).map(|()| OK),
        Mode::Help | Mode::Configure => Ok(OK),
        Mode::List => list(paths, &config),
        Mode::Check(game) => check(paths, &config, game.as_deref()),
        Mode::Set(slug) => {
            set(paths, game_of(slug), inv)?;
            Ok(OK)
        }
        Mode::Play(slug) => {
            let game = game_of(slug);
            set(paths, game, inv)?;
            play(paths, &config, game, inv)
        }
    }
}

/// A game the parser checked.
fn game_of(slug: &str) -> &'static Game {
    catalog::by_slug(slug).expect("checked by the parser")
}

/// The library folders of the command line saved in `launcher.cfg`.
fn configure(paths: &HubPaths, config: &mut HubConfig, inv: &Invocation) -> Result<(), String> {
    for dir in &inv.libraries {
        config.add_library(&typed_path(&dir.to_string_lossy(), PathKind::Folder, "")?);
    }
    config
        .save(&paths.config_file(), paths::FIELDS)
        .map_err(|e| format!("Could not save the settings: {e}"))
}

/// The options of the command line over `s`: a look first, so the flags
/// after it win (and make the picture custom: no look).
fn apply_options(inv: &Invocation, s: &mut Settings, game: &Game) {
    inv.apply_options(settings::OPTIONS, &["look"], |spec, v| match v {
        Value::Choice(key) if spec.key == "look" => {
            if let Some(look) = Look::from_key(key) {
                look.apply(s, game, cli_screen());
            }
        }
        v => {
            let _ = s.apply(spec.key, v);
            if settings::LOOK_KEYS.contains(&spec.key) {
                s.look = None;
            }
        }
    });
}

/// The screen a command-line run sees: Windows asks the system; on Linux
/// the program may run under gamescope or a helper, so it doesn't know.
fn cli_screen() -> Option<(u32, u32)> {
    #[cfg(windows)]
    return rea_launcher_win32::display::screen_size();
    #[cfg(not(windows))]
    None
}

/// The options of the command line saved into the game's settings (the
/// keys given, and those a look sets).
fn set(paths: &HubPaths, game: &Game, inv: &Invocation) -> Result<(), String> {
    if inv.options.is_empty() {
        return Ok(());
    }
    let mut s = load_settings(paths, game, cli_screen(), warn)?;
    apply_options(inv, &mut s, game);
    let mut keys: Vec<&str> = inv.options.iter().map(|(k, _)| *k).collect();
    if keys.contains(&"look") {
        // a look's values, or its line cleared when a flag made it custom
        keys.extend(settings::LOOK_KEYS);
    } else if keys.iter().any(|k| settings::LOOK_KEYS.contains(k)) {
        keys.push("look");
    }
    s.write(&paths.settings(game.slug), &keys)
        .map_err(|e| format!("Could not save the options: {e}"))
}

/// Looks in `src` for every board-file set and says what it found.
fn add_board_files(paths: &HubPaths, src: &str) -> Result<u8, String> {
    let src = rea_launcher::hub::expand_home(src);
    if !src.exists() {
        return Err(format!("{} is not a folder or file", display_path(&src)));
    }
    for set in [&firmware::SYS246, &firmware::CHIHIRO] {
        let r = firmware::import(set, &src, &set.dir(&paths.data))
            .map_err(|e| format!("{}: {e}", set.title))?;
        println!("{} ({} looked at)", set.title, r.looked_at);
        for f in set.files {
            let mark = if r.added.contains(&f.name) {
                "added"
            } else if r.missing.contains(&f.name) {
                "missing"
            } else {
                "already in"
            };
            println!("  {mark:<10} {}", f.name);
        }
    }
    Ok(OK)
}

/// The board files `game` still needs, by name.
fn missing_files(paths: &HubPaths, game: &Game) -> Vec<&'static str> {
    game.firmware
        .map(|set| {
            set.check(&set.dir(&paths.data))
                .into_iter()
                .filter(|(_, st)| *st != FileState::Present)
                .map(|(f, _)| f.name)
                .collect()
        })
        .unwrap_or_default()
}

/// What keeps `game` from starting here, its board files first.
fn state(paths: &HubPaths, game: &Game) -> Result<String, String> {
    let missing = missing_files(paths, game);
    if !missing.is_empty() {
        return Err(format!(
            "needs board files (--add-files): {}",
            missing.join(", ")
        ));
    }
    match game.support_on(Host::current()) {
        Support::Ready => Ok("ready".to_string()),
        Support::Unverified(note) => Ok(format!("starts; {note}")),
        Support::Blocked(why) => Err(format!("not yet: {why}")),
    }
}

/// The games in the library folders; what couldn't be read is said on stderr.
fn scan(config: &HubConfig) -> Result<Scan<Game>, String> {
    if config.libraries.is_empty() {
        return Err("No game folder yet: run with --library <folder>.".into());
    }
    let scan = catalog::library(HashCache::default()).scan(&config.libraries);
    for problem in &scan.problems {
        warn(&format!("couldn't read {problem}"));
    }
    Ok(scan)
}

fn list(paths: &HubPaths, config: &HubConfig) -> Result<u8, String> {
    for e in scan(config)?.entries {
        let dir = display_path(&e.dir);
        match &e.found {
            Found::Known { game, edition } => {
                let state = state(paths, game).unwrap_or_else(|e| e);
                println!(
                    "{:<8} {:<34} {:<20} {state}\n         {dir}",
                    game.slug, game.title, edition.label,
                );
            }
            Found::Unknown { file, digest } => {
                println!("unknown  {file} sha256 {digest}: report it at {ISSUES}\n         {dir}")
            }
        }
    }
    Ok(OK)
}

/// Whether each game (or the one named) could start now: found in the
/// library, a known edition, its board files in, supported here, its
/// components in place. The unknown builds reported are the named game's
/// only.
fn check(paths: &HubPaths, config: &HubConfig, only: Option<&str>) -> Result<u8, String> {
    let found = scan(config)?;
    let by_game = found.by_game();
    let asked = |g: &Game| only.is_none_or(|s| s == g.slug);
    let mut problems = 0;
    for game in GAMES.iter().filter(|g| asked(g)) {
        let verdict = match by_game.get(&game.id) {
            None => Err("not in your game folders".to_string()),
            Some(_) => state(paths, game).and_then(|s| {
                session::check_components(game, paths)
                    .map(|()| s)
                    .map_err(|e| format!("{e:#}"))
            }),
        };
        match verdict {
            Ok(s) => println!("{:<8} ok: {s}", game.slug),
            Err(e) => {
                problems += 1;
                println!("{:<8} {e}", game.slug);
            }
        }
    }
    for e in &found.entries {
        if let Found::Unknown { file, digest } = &e.found
            && GAMES.iter().any(|g| asked(g) && g.main_file == file)
        {
            problems += 1;
            println!(
                "unknown  {file} sha256 {digest} in {}: report it at {ISSUES}",
                display_path(&e.dir)
            );
        }
    }
    Ok(if problems == 0 { OK } else { REFUSED })
}

fn play(
    paths: &HubPaths,
    config: &HubConfig,
    game: &'static Game,
    inv: &Invocation,
) -> Result<u8, String> {
    if let Err(why) = state(paths, game) {
        return Err(format!("{} can't start: {why}", game.title));
    }
    let found = scan(config)?;
    let entry = found
        .by_game()
        .get(&game.id)
        .map(|e| (*e).clone())
        .ok_or_else(|| format!("{} is not in your game folders.", game.title))?;
    let settings = load_settings(paths, game, cli_screen(), warn)?;
    let display = if inv.test.headless {
        Display::Headless
    } else {
        Display::Screen
    };
    let prepared =
        session::prepare(paths, game, &entry, &settings).map_err(|e| format!("{e:#}"))?;
    // Ctrl+C or a TERM ends the game the way the window's Stop does
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(sig, Arc::clone(&stop));
    }
    if let Some(secs) = inv.test.seconds {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            stop.store(true, Ordering::SeqCst);
        });
    }
    let devices = (!inv.test.no_input && inv.test.script.is_none())
        .then(|| input::service(&input::bindings(paths, |w| Log::new(paths.log()).warn(w))));
    let feed = if let Some(f) = &inv.test.script {
        Input::Script(read_script(f)?)
    } else if let Some(i) = &devices {
        Input::Devices {
            tx: i.sender(),
            deadzone: settings.deadzone,
        }
    } else {
        Input::None
    };
    let status = session::run(
        paths,
        game,
        &prepared,
        display,
        &config.libraries,
        &feed,
        &stop,
    )
    .map_err(|e| format!("{e:#}"))?;
    eprintln!(
        "{} ended ({status}); log: {}",
        game.title,
        display_path(&session::session_log(paths, game))
    );
    Ok(OK)
}

/// A timed input script (`--input-script`).
fn read_script(file: &Path) -> Result<Script, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Script::parse::<Button>(&text).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher::gamescope::{Size, Upscaler};

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn parse_v(v: &[&str]) -> Invocation {
        cli::parse(&CLI, args(v)).unwrap()
    }

    fn paths_in(t: &Path) -> HubPaths {
        HubPaths {
            data: t.into(),
            components: t.join("c"),
            dev_assets: None,
        }
    }

    #[test]
    fn every_option_flag_reaches_its_settings_key() {
        let inv = parse_v(&[
            "--set",
            "mt3",
            "--render",
            "1920x1080",
            "--deadzone",
            "0.12",
        ]);
        let mut s = Settings::default();
        apply_options(&inv, &mut s, game_of("mt3"));
        assert_eq!(s.render, Some(Size::new(1920, 1080)));
        assert!((s.deadzone - 0.12).abs() < 1e-6);
        // the look and gamescope's options are Linux's
        if Host::current() == Host::Linux {
            let inv = parse_v(&[
                "--set",
                "mt3",
                "--render",
                "2560x1440",
                "--look",
                "original",
                "--output",
                "2560x1440",
                "--upscaler",
                "nis",
                "--sharpness",
                "3",
                "--fullscreen",
                "off",
            ]);
            apply_options(&inv, &mut s, game_of("mt3"));
            assert_eq!(s.look, None, "a picture flag after a look makes it custom");
            assert_eq!(
                s.render,
                Some(Size::new(2560, 1440)),
                "the flag wins over the look"
            );
            assert_eq!(s.output, Size::new(2560, 1440));
            assert_eq!(s.upscaler, Upscaler::Nis);
            assert_eq!(s.sharpness, 3);
            assert!(!s.fullscreen);
            let inv = parse_v(&["--set", "mt3", "--look", "steam_deck", "--deadzone", "0.2"]);
            apply_options(&inv, &mut s, game_of("mt3"));
            assert_eq!(
                s.look,
                Some(Look::SteamDeck),
                "the dead zone is no picture value"
            );
        }
    }

    #[test]
    fn options_a_game_ignores_are_refused() {
        let e = cli::parse(&CLI, args(&["--set", "mt5", "--render", "1920x1080"])).unwrap_err();
        assert!(e.contains("mt5 doesn't take --render"), "{e}");
        assert!(cli::parse(&CLI, args(&["--set", "mt3", "--render", "1920x1080"])).is_ok());
        let gamescope = cli::parse(&CLI, args(&["--play", "mt5", "--output", "1920x1080"]));
        assert_eq!(gamescope.is_err(), cfg!(windows), "{gamescope:?}");
        let look = cli::parse(&CLI, args(&["--set", "mt5", "--look", "original"]));
        assert_eq!(look.is_err(), cfg!(windows), "{look:?}");
    }

    #[test]
    fn wangan_r_takes_no_input_script() {
        let e = cli::parse(
            &CLI,
            args(&["--play", "wangan-r", "--input-script", "s.txt"]),
        );
        assert!(e.unwrap_err().contains("reads the pad itself"));
        assert!(cli::parse(&CLI, args(&["--play", "mt3", "--input-script", "s.txt"])).is_ok());
        assert!(usage().contains("timed inputs, into the input block (not Wangan R)"));
    }

    #[test]
    fn set_saves_the_flags_and_keeps_the_players_lines() {
        let t = tempfile::tempdir().unwrap();
        let p = paths_in(t.path());
        std::fs::create_dir_all(t.path().join("settings")).unwrap();
        std::fs::write(p.settings("mt3"), "# mine\nsharpness = 7\n").unwrap();
        let inv = parse_v(&["--set", "mt3", "--deadzone", "0.1"]);
        set(&p, game_of("mt3"), &inv).unwrap();
        let text = std::fs::read_to_string(p.settings("mt3")).unwrap();
        assert!(text.starts_with("# mine\nsharpness = 7\n"), "{text}");
        assert!(text.contains("\ndeadzone = 0.10\n"), "{text}");
        // only Windows' command line sees the screen and saves its look
        assert_eq!(text.contains("\nlook = "), cfg!(windows), "{text}");
    }

    #[test]
    fn add_files_is_taken_out_before_the_hub_parses_the_rest() {
        let (add, rest) =
            split_add_files(&args(&["--library", "/g", "--add-files", "/b"])).unwrap();
        assert_eq!(add.as_deref(), Some("/b"));
        assert_eq!(rest, ["--library", "/g"]);
        let (add, _) = split_add_files(&args(&["--add-files=/b"])).unwrap();
        assert_eq!(add.as_deref(), Some("/b"));
        assert!(split_add_files(&args(&["--add-files"])).is_err());
    }

    #[test]
    fn the_usage_lists_every_game_every_option_and_add_files() {
        let u = usage();
        for g in GAMES {
            assert!(u.contains(g.slug));
        }
        for spec in settings::OPTIONS {
            assert!(u.contains(&spec.flag_name(settings::STYLE)), "{}", spec.key);
        }
        assert!(u.contains("--add-files <path>"));
    }

    #[test]
    fn the_settings_heading_names_commands_the_usage_has() {
        let u = usage();
        for flag in ["--set <game>", "--play <game>"] {
            assert!(settings::HEADING.contains(flag), "{flag}");
            assert!(u.contains(flag), "{flag}");
        }
    }

    #[test]
    fn the_window_title_is_no_games_title() {
        for g in GAMES {
            assert_ne!(g.title, WINDOW_TITLE);
        }
    }
}
