//! wangan-launcher: one launcher for the Wangan Midnight arcade series.

mod input;
mod log;
mod paths;
mod session;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use wangan_core::catalog::{self, Support};
use wangan_core::library::{self, Found, HashCache};
use wangan_core::settings::Settings;

use crate::paths::Paths;
use crate::session::Display;

const USAGE: &str = "\
wangan-launcher — Wangan Midnight arcade series

  wangan-launcher                      open the launcher window
  wangan-launcher --library <folder>   remember where your games are
  wangan-launcher --list               list the games found and their state
  wangan-launcher --play <game>        start a game (mt3, mt3dxp, mt5, mt5dxp, mt6r, …)
  wangan-launcher --game <game> <options>   save options for a game without starting it
  Options (saved in that game's settings, same as the window):
      --output <WxH>                   screen size gamescope draws at, e.g. 3840x2160
      --upscaler <fsr|nis|linear|nearest>
      --sharpness <0-20>               FSR/NIS sharpness, 0 is sharpest
      --fullscreen <on|off>
      --render <WxH|native>            the game's own size (MT3, MT3DX+ only)
      --deadzone <0-0.3>               controller stick dead zone
      --headless                       run it in an invisible display (tests)
      --no-input                       leave the input block to another program (tests)
  wangan-launcher --help               this text
";

struct Args {
    library: Option<PathBuf>,
    list: bool,
    play: Option<String>,
    headless: bool,
    no_input: bool,
    game: Option<String>,
    /// `key = value` lines for the game's settings file.
    options: Vec<String>,
}

fn parse() -> Result<Option<Args>, String> {
    let mut a = Args {
        library: None,
        list: false,
        play: None,
        headless: false,
        no_input: false,
        game: None,
        options: vec![],
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--help" | "-h" => return Ok(None),
            "--library" => a.library = Some(it.next().ok_or("--library needs a folder")?.into()),
            "--list" => a.list = true,
            "--play" => a.play = Some(it.next().ok_or("--play needs a game")?),
            "--headless" => a.headless = true,
            "--no-input" => a.no_input = true,
            "--game" => a.game = Some(it.next().ok_or("--game needs a game")?),
            "--output" | "--upscaler" | "--sharpness" | "--fullscreen" | "--render"
            | "--deadzone" => {
                let key = &x[2..];
                let value = it.next().ok_or(format!("{x} needs a value"))?;
                let line = format!("{key} = {value}");
                let (_, problems) = Settings::parse(&line);
                if !problems.is_empty() {
                    return Err(format!("bad value for {x}: {value}"));
                }
                a.options.push(line);
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(Some(a))
}

pub fn load_settings(paths: &Paths, slug: &str) -> Settings {
    let text = std::fs::read_to_string(paths.settings(slug)).unwrap_or_default();
    let (s, problems) = Settings::parse(&text);
    for p in problems {
        log::line(paths, &format!("settings/{slug}.cfg: {p}"));
    }
    s
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
    let library = paths::read_library(&paths.config());
    if args.list {
        let Some(root) = library else {
            eprintln!("no library yet: run with --library <folder>");
            return ExitCode::FAILURE;
        };
        let found = library::scan(&root, &mut HashCache::default());
        for e in &found {
            match &e.found {
                Found::Known { game, edition } => {
                    let state = match game.support {
                        Support::Ready => "ready".to_string(),
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
                Found::Unknown { file, sha256 } => println!(
                    "unknown  {file} sha256 {sha256}\n         {}",
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
        if catalog::by_slug(slug).is_none() {
            eprintln!("unknown game {slug}");
            return ExitCode::from(2);
        }
        if let Err(e) = save_options(&paths, slug, &args.options) {
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

/// Apply `key = value` options to a game's settings file, keeping the rest.
pub fn save_options(paths: &Paths, slug: &str, options: &[String]) -> std::io::Result<()> {
    let file = paths.settings(slug);
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let (settings, _) = Settings::parse(&format!("{old}\n{}", options.join("\n")));
    if let Some(p) = file.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, settings.save(&old))?;
    std::fs::rename(tmp, file)
}

fn play_cli(paths: &Paths, library: Option<PathBuf>, slug: &str, args: &Args) -> ExitCode {
    let Some(game) = catalog::by_slug(slug) else {
        eprintln!("unknown game {slug}");
        return ExitCode::from(2);
    };
    if let Support::Blocked(why) = game.support {
        eprintln!("{} can't start yet: {why}", game.title);
        return ExitCode::FAILURE;
    }
    let Some(root) = library else {
        eprintln!("no library yet: run with --library <folder>");
        return ExitCode::FAILURE;
    };
    let found = library::scan(&root, &mut HashCache::default());
    let Some(entry) = library::by_game(&found).get(&game.id).map(|e| (*e).clone()) else {
        eprintln!("{} is not in {}", game.title, root.display());
        return ExitCode::FAILURE;
    };
    let settings = load_settings(paths, slug);
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
