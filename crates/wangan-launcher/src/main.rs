//! wangan-launcher: one launcher for the Wangan Midnight arcade series.

mod input;
mod log;
mod paths;
mod session;
mod ui;
mod winproc;

use std::path::PathBuf;
use std::process::ExitCode;

use rea_launcher::library::{Found, HashCache};
use wangan_core::catalog::{self, Game, Host, Support};
use wangan_core::firmware::{self, FileState};
use wangan_core::settings::Settings;

use crate::paths::Paths;
use crate::session::Display;

const USAGE: &str = "\
wangan-launcher — Wangan Midnight arcade series

  wangan-launcher                      open the launcher window
  wangan-launcher --library <folder>   remember where your games are
  wangan-launcher --list               list the games found and their state
  wangan-launcher --add-files <path>   look in a folder or zip for the board files
                                       Wangan Midnight R and Maximum Tune need
  wangan-launcher --play <game>        start a game (mt3, mt3dxp, mt5, mt5dxp, mt6r, …)
  wangan-launcher --game <game> <options>   save options for a game without starting it
  Options (saved in that game's settings, same as the window):
      --output <WxH>                   screen size gamescope draws at, e.g. 3840x2160 (Linux)
      --upscaler <fsr|nis|linear|nearest>   (Linux)
      --sharpness <0-20>               FSR/NIS sharpness, 0 is sharpest (Linux)
      --fullscreen <on|off>            (Linux)
      --render <WxH|native>            the game's own size (MT3, MT3DX+ only)
      --deadzone <0-0.3>               controller stick dead zone
      --headless                       run it in an invisible display (tests; Linux)
      --no-input                       leave the input block to another program (tests)
  wangan-launcher --help               this text
";

struct Args {
    library: Option<PathBuf>,
    list: bool,
    add_files: Option<PathBuf>,
    play: Option<String>,
    headless: bool,
    no_input: bool,
    game: Option<String>,
    /// `(key, value)` for the game's settings file.
    options: Vec<(String, String)>,
}

fn parse() -> Result<Option<Args>, String> {
    parse_args(std::env::args().skip(1), Host::current())
}

/// The command line; options the window shows as off on this platform are
/// refused here with the same reason (window and CLI in parity).
fn parse_args(
    mut it: impl Iterator<Item = String>,
    platform: Host,
) -> Result<Option<Args>, String> {
    let mut a = Args {
        library: None,
        list: false,
        add_files: None,
        play: None,
        headless: false,
        no_input: false,
        game: None,
        options: vec![],
    };
    while let Some(x) = it.next() {
        match x.as_str() {
            "--help" | "-h" => return Ok(None),
            "--library" => a.library = Some(it.next().ok_or("--library needs a folder")?.into()),
            "--list" => a.list = true,
            "--add-files" => {
                a.add_files = Some(it.next().ok_or("--add-files needs a folder or zip")?.into())
            }
            "--play" => a.play = Some(it.next().ok_or("--play needs a game")?),
            "--headless" if platform == Host::Windows => {
                return Err(format!("--headless: {}", ui::form::NO_GAMESCOPE));
            }
            "--headless" => a.headless = true,
            "--no-input" => a.no_input = true,
            "--game" => a.game = Some(it.next().ok_or("--game needs a game")?),
            "--output" | "--upscaler" | "--sharpness" | "--fullscreen" | "--render"
            | "--deadzone" => {
                let key = &x[2..];
                if let Some(why) = ui::form::Row::from_key(key)
                    .and_then(|row| ui::form::platform_off(row, platform))
                {
                    return Err(format!("{x}: {why}"));
                }
                let value = it.next().ok_or(format!("{x} needs a value"))?;
                if Settings::default().set(key, &value).is_err() {
                    return Err(format!("bad value for {x}: {value}"));
                }
                a.options.push((key.to_string(), value));
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(Some(a))
}

/// A game's settings; the lines its file couldn't read go to `launcher.log`.
pub fn load_settings(paths: &Paths, game: &Game) -> Result<Settings, String> {
    let file = paths.settings(game.slug);
    let text = rea_launcher::cfg::read_file(&file)
        .map_err(|e| format!("couldn't read {}: {e}", file.display()))?;
    let (s, problems) = Settings::parse(&text.unwrap_or_default());
    for p in problems {
        log::line(paths, &format!("settings/{}.cfg: {p}", game.slug));
    }
    Ok(s)
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(Some(a)) => a,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let paths = Paths::discover();
    if let Some(dir) = &args.library {
        if !dir.is_dir() {
            eprintln!("{} is not a folder", dir.display());
            return ExitCode::from(2);
        }
        if let Err(e) = paths::write_library(&paths.config(), dir) {
            eprintln!("could not save the library folder: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Some(src) = &args.add_files {
        return add_files_cli(&paths, src);
    }
    let library = paths::read_library(&paths.config());
    if args.list {
        let Some(root) = library else {
            eprintln!("no library yet: run with --library <folder>");
            return ExitCode::FAILURE;
        };
        let scan = catalog::library(HashCache::default()).scan(&[root]);
        for problem in &scan.problems {
            eprintln!("warning: couldn't read {problem}");
        }
        for e in &scan.entries {
            match &e.found {
                Found::Known { game, edition } => {
                    let missing: Vec<&str> = game
                        .firmware
                        .map(|set| {
                            set.check(&set.dir(&paths.data))
                                .into_iter()
                                .filter(|(_, st)| *st != FileState::Present)
                                .map(|(f, _)| f.name)
                                .collect()
                        })
                        .unwrap_or_default();
                    let state = match game.support_on(Host::current()) {
                        _ if !missing.is_empty() => {
                            format!("needs board files (--add-files): {}", missing.join(", "))
                        }
                        Support::Ready => "ready".to_string(),
                        Support::Unverified(note) => format!("starts; {note}"),
                        Support::Blocked(why) => format!("not yet: {why}"),
                    };
                    println!(
                        "{:<8} {:<34} {:<20} {state}\n         {}",
                        game.slug,
                        game.title,
                        edition.label,
                        e.dir.display()
                    );
                }
                Found::Unknown { file, digest } => println!(
                    "unknown  {file} sha256 {digest}\n         {}",
                    e.dir.display()
                ),
            }
        }
        return ExitCode::SUCCESS;
    }
    if !args.options.is_empty() {
        let Some(slug) = args.play.as_ref().or(args.game.as_ref()) else {
            eprintln!("options need --play <game> or --game <game>\n\n{USAGE}");
            return ExitCode::from(2);
        };
        let Some(game) = catalog::by_slug(slug) else {
            eprintln!("unknown game {slug}");
            return ExitCode::from(2);
        };
        if let Err(e) = save_options(&paths, game, &args.options) {
            eprintln!("could not save the options: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Some(slug) = &args.play {
        return play_cli(&paths, library, slug, &args);
    }
    if args.library.is_some() || args.game.is_some() {
        return ExitCode::SUCCESS;
    }
    match ui::run(paths) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Look in `src` for every board-file set and report each one.
fn add_files_cli(paths: &Paths, src: &std::path::Path) -> ExitCode {
    if !src.exists() {
        eprintln!("{} is not a folder or file", src.display());
        return ExitCode::from(2);
    }
    let mut ok = true;
    for set in [&firmware::SYS246, &firmware::CHIHIRO] {
        match firmware::import(set, src, &set.dir(&paths.data)) {
            Ok(r) => {
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
            Err(e) => {
                eprintln!("{}: {e}", set.title);
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Apply `(key, value)` options to a game's settings file, keeping the rest.
pub fn save_options(
    paths: &Paths,
    game: &Game,
    options: &[(String, String)],
) -> Result<(), String> {
    let mut s = load_settings(paths, game)?;
    for (k, v) in options {
        s.set(k, v)
            .map_err(|_| format!("bad value for --{k}: {v}"))?;
    }
    let keys: Vec<&str> = options.iter().map(|(k, _)| k.as_str()).collect();
    s.write(&paths.settings(game.slug), &keys)
        .map_err(|e| e.to_string())
}

fn play_cli(paths: &Paths, library: Option<PathBuf>, slug: &str, args: &Args) -> ExitCode {
    let Some(game) = catalog::by_slug(slug) else {
        eprintln!("unknown game {slug}");
        return ExitCode::from(2);
    };
    if let Support::Blocked(why) = game.support_on(Host::current()) {
        eprintln!("{} can't start: {why}", game.title);
        return ExitCode::FAILURE;
    }
    let Some(root) = library else {
        eprintln!("no library yet: run with --library <folder>");
        return ExitCode::FAILURE;
    };
    let found = catalog::library(HashCache::default()).scan(&[root.clone()]);
    let Some(entry) = found.by_game().get(&game.id).map(|e| (*e).clone()) else {
        eprintln!("{} is not in {}", game.title, root.display());
        return ExitCode::FAILURE;
    };
    let settings = match load_settings(paths, game) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let prepared = match session::prepare(paths, game, &entry, &settings) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e:#}");
            return ExitCode::FAILURE;
        }
    };
    let input = (!args.no_input).then(input::InputService::start);
    if let Some(i) = &input {
        i.send(input::Command::Attach {
            block: prepared.block.clone(),
            cabinet: game.cabinet,
            deadzone: settings.deadzone,
        });
    }
    let display = if args.headless {
        Display::Headless
    } else {
        Display::Screen
    };
    let r = session::run(paths, game, &prepared, display, |pid| {
        println!("gamescope pid {pid}")
    });
    if let Some(i) = &input {
        i.send(input::Command::Detach);
    }
    match r {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> impl Iterator<Item = String> {
        s.split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn gamescope_options_are_refused_on_windows_with_the_windows_reason() {
        for opt in [
            "--output 1920x1080",
            "--upscaler fsr",
            "--sharpness 3",
            "--fullscreen on",
            "--headless",
        ] {
            let line = format!("--play mt5 {opt}");
            let e = parse_args(args(&line), Host::Windows).err().expect(opt);
            assert!(e.ends_with(ui::form::NO_GAMESCOPE), "{e}");
            assert!(parse_args(args(&line), Host::Linux).is_ok(), "{opt}");
        }
        let a = parse_args(args("--game mt5 --deadzone 0.1"), Host::Windows)
            .unwrap()
            .unwrap();
        assert_eq!(a.options, [("deadzone".to_string(), "0.1".to_string())]);
    }
}
