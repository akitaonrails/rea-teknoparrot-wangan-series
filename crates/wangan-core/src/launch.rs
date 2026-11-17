//! What to write and what to run for a game, as plain data.
//!
//! [`plan`] turns a game, its runtime folder and the settings into the
//! files the launcher writes and the processes it starts, so tests can
//! check the whole wiring (setting → file / argument / environment)
//! without starting anything.

use std::path::{Path, PathBuf};

use crate::catalog::{Game, GameId, Route};
use rea_launcher::gamescope::Size;

use crate::settings::Settings;

/// A process to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Files written (replaced) before every launch.
    pub files: Vec<(PathBuf, String)>,
    /// Started first and kept running (the I/O board, the auth daemon).
    pub helpers: Vec<Command>,
    /// The game; the session lasts as long as it runs.
    pub game: Command,
    /// Arguments for gamescope, which wraps everything.
    pub gamescope: Vec<String>,
    /// The game runs under Wine in this prefix.
    pub wine_prefix: Option<PathBuf>,
    /// Native size the game draws at.
    pub game_size: Size,
}

/// How the launcher starts the Windows games on this system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    /// Linux: under Wine, inside gamescope. `wine` is the program to run
    /// (the bundled Wine's `bin/wine`, or `wine` from PATH); `wine_env` is
    /// added to every Wine process (the bundled Wine's library path).
    Linux {
        wine: String,
        wine_env: Vec<(String, String)>,
    },
    /// Windows: the I/O board and OpenParrot's loader run directly.
    Windows,
}

impl Host {
    /// Wine from PATH.
    pub fn system_wine() -> Self {
        Host::Linux {
            wine: "wine".into(),
            wine_env: vec![],
        }
    }
}

/// Where the input block lives inside a runtime.
pub fn block_path(rt: &Path) -> PathBuf {
    rt.join("input.block")
}

/// Wangan R's `.acgame` file in its runtime.
pub fn sys246_acgame(rt: &Path) -> PathBuf {
    rt.join("game")
        .join(format!("{}.acgame", crate::sys246::GAME_ID))
}

/// pcsx2x6's settings file in a Wangan R runtime.
pub fn sys246_ini(rt: &Path) -> PathBuf {
    crate::sys246::data_dir(rt).join("inis").join("PCSX2.ini")
}

/// `Z:\…` path as Wine sees a Unix path.
pub fn wine_path(p: &Path) -> String {
    format!("Z:{}", p.to_string_lossy().replace('/', "\\"))
}

fn teknoparrot_ini(id: GameId) -> Option<&'static str> {
    Some(match id {
        GameId::Mt5 => include_str!("../data/ini/WMMT5.ini"),
        GameId::Mt5dxp => include_str!("../data/ini/WMMT5DXPlus.ini"),
        GameId::Mt6 => include_str!("../data/ini/WMMT6.ini"),
        GameId::Mt6r => include_str!("../data/ini/WMMT6R.ini"),
        GameId::Mt6rr => include_str!("../data/ini/WMMT6RR.ini"),
        _ => return None,
    })
}

/// WanganArcadeLoader's `config.toml`. The dongle serial is the one the
/// loader ships in its own template.
pub fn wal_config(render: Size, deadzone: f32) -> String {
    format!(
        "# Written by the launcher before every start.\n\
         fullscreen = false\ninput_emu = true\ncard_emu = true\nblock_sudo = true\nignore_custom_ioctls = true\n\
         dongle = \"285013501138\"\ndeadzone = {deadzone:.2}\nwidth = {}\nheight = {}\n",
        render.w, render.h
    )
}

/// WanganArcadeLoader's `keyconfig.toml`: every key must be present (the
/// loader stops without the file). The launcher's input block drives the
/// cabinet; these bindings are the loader's own fallback (keyboard and an
/// SDL game controller) when no block is given.
pub const WAL_KEYCONFIG: &str = r#"# Written by the launcher before every start.
TEST = ["F1"]
SERVICE = ["F2"]
CARD_INSERT = ["C", "SDL_BACK"]
GEAR_NEXT = ["SDL_RSHOULDER"]
GEAR_PREVIOUS = ["SDL_LSHOULDER"]
GEAR_NEUTRAL = []
GEAR_FIRST = []
GEAR_SECOND = []
GEAR_THIRD = []
GEAR_FOURTH = []
GEAR_FIFTH = []
GEAR_SIXTH = []
GEAR_UP = []
GEAR_LEFT = []
GEAR_DOWN = []
GEAR_RIGHT = []
PERSPECTIVE = ["V", "SDL_Y"]
INTRUDE = ["I", "SDL_X"]
GAS = ["UPARROW", "SDL_RTRIGGER_DOWN"]
BRAKE = ["DOWNARROW", "SDL_LTRIGGER_DOWN"]
WHEEL_LEFT = ["LEFTARROW", "SDL_LSTICK_LEFT"]
WHEEL_RIGHT = ["RIGHTARROW", "SDL_LSTICK_RIGHT"]
"#;

fn es3b_exe(id: GameId) -> &'static str {
    match id {
        GameId::Mt5 | GameId::Mt5dxp => "wmn5r.exe",
        _ => "wmn6r.exe",
    }
}

/// ES3B native size (all five games render 1360×768).
pub const ES3B_SIZE: Size = Size::new(1360, 768);

/// The ES3B processes as Windows runs them: the board, the auth daemon
/// through the loader (MT6, 6RR), then the game through the loader. Full
/// paths, because Windows looks for a program beside the launcher, not in
/// the runtime. No gamescope: the game sets its own screen mode.
fn plan_es3b_windows(game: &Game, rt: &Path) -> Option<Plan> {
    let run = |program: &str, args: Vec<String>| Command {
        program: native_path(&rt.join(program)),
        args,
        env: vec![],
        cwd: rt.to_path_buf(),
    };
    let mut helpers = vec![run(
        "wangan-io.exe",
        vec![
            "--block".into(),
            native_path(&block_path(rt)),
            "--log".into(),
            native_path(&rt.join("wangan-io.log")),
        ],
    )];
    if matches!(game.id, GameId::Mt6 | GameId::Mt6rr) {
        helpers.push(run(
            "OpenParrotLoader64.exe",
            vec![
                "OpenParrot64".into(),
                native_path(&rt.join("AMCUS").join("AMAuthd.exe")),
            ],
        ));
    }
    Some(Plan {
        files: vec![(
            rt.join("teknoparrot.ini"),
            teknoparrot_ini(game.id)?.to_string(),
        )],
        helpers,
        game: run(
            "OpenParrotLoader64.exe",
            vec![
                "OpenParrot64".into(),
                native_path(&rt.join(es3b_exe(game.id))),
            ],
        ),
        gamescope: vec![],
        wine_prefix: None,
        game_size: ES3B_SIZE,
    })
}

fn native_path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// [`plan_on`] with Wine from PATH (Linux).
pub fn plan(game: &Game, rt: &Path, prefix: &Path, s: &Settings) -> Option<Plan> {
    plan_on(game, rt, prefix, s, &Host::system_wine())
}

pub fn plan_on(game: &Game, rt: &Path, prefix: &Path, s: &Settings, host: &Host) -> Option<Plan> {
    let (wine, extra_env) = match host {
        Host::Linux { wine, wine_env } => (wine.as_str(), wine_env.as_slice()),
        Host::Windows => {
            return match game.route {
                Route::Es3b => plan_es3b_windows(game, rt),
                // Linux programs (catalog::N2_LINUX_ONLY), and the blocked routes
                _ => None,
            };
        }
    };
    match game.route {
        Route::Es3b => {
            let exe = es3b_exe(game.id);
            let mut wine_env = vec![
                (
                    "WINEPREFIX".to_string(),
                    prefix.to_string_lossy().into_owned(),
                ),
                ("WINEDEBUG".to_string(), "-all".to_string()),
                ("WINEDLLOVERRIDES".to_string(), "d3d11,dxgi=n,b".to_string()),
            ];
            wine_env.extend(extra_env.iter().cloned());
            let mut helpers = vec![Command {
                program: wine.into(),
                args: vec![
                    "wangan-io.exe".into(),
                    "--block".into(),
                    wine_path(&block_path(rt)),
                    "--log".into(),
                    wine_path(&rt.join("wangan-io.log")),
                ],
                env: wine_env.clone(),
                cwd: rt.to_path_buf(),
            }];
            // the auth daemon first, as TeknoParrot does for 6; 6RR needs it too (docs/RUNTIME_NOTES.md)
            if matches!(game.id, GameId::Mt6 | GameId::Mt6rr) {
                helpers.push(Command {
                    program: wine.into(),
                    args: vec![
                        "OpenParrotLoader64.exe".into(),
                        "OpenParrot64".into(),
                        wine_path(&rt.join("AMCUS/AMAuthd.exe")),
                    ],
                    env: wine_env.clone(),
                    cwd: rt.to_path_buf(),
                });
            }
            Some(Plan {
                files: vec![(
                    rt.join("teknoparrot.ini"),
                    teknoparrot_ini(game.id)?.to_string(),
                )],
                helpers,
                game: Command {
                    program: wine.into(),
                    args: vec![
                        "OpenParrotLoader64.exe".into(),
                        "OpenParrot64".into(),
                        wine_path(&rt.join(exe)),
                    ],
                    env: wine_env,
                    cwd: rt.to_path_buf(),
                },
                gamescope: s.picture(ES3B_SIZE).args(false),
                wine_prefix: Some(prefix.to_path_buf()),
                game_size: ES3B_SIZE,
            })
        }
        Route::NativeN2 => {
            // unset: draw at the output size (WanganArcadeLoader renders
            // MT3/3DX+ natively at any size, 4K seen in both games)
            let render = s.render.unwrap_or(s.output);
            let ld = format!(
                "{}:{}:{}",
                rt.join("glfix").display(),
                rt.display(),
                rt.join("libso").display()
            );
            Some(Plan {
                files: vec![
                    (rt.join("config.toml"), wal_config(render, s.deadzone)),
                    (rt.join("keyconfig.toml"), WAL_KEYCONFIG.to_string()),
                ],
                helpers: vec![],
                game: Command {
                    program: rt.join("main").to_string_lossy().into_owned(),
                    args: vec![],
                    env: vec![
                        ("LC_ALL".into(), "C".into()),
                        ("LD_LIBRARY_PATH".into(), ld),
                        ("LD_PRELOAD".into(), "libwal_3dxp.so".into()),
                        (
                            "WANGAN_INPUT_BLOCK".into(),
                            block_path(rt).to_string_lossy().into_owned(),
                        ),
                    ],
                    cwd: rt.to_path_buf(),
                },
                gamescope: s.picture(render).args(false),
                wine_prefix: None,
                game_size: render,
            })
        }
        Route::System246 => {
            // pcsx2x6 fills the window; it draws at the upscale the window
            // height asks for
            let window = s.render.unwrap_or(s.output);
            let data = rt.join("config");
            Some(Plan {
                files: vec![
                    (sys246_acgame(rt), crate::sys246::acgame()),
                    (
                        sys246_ini(rt),
                        crate::sys246::pcsx2_ini(crate::sys246::upscale_for(window.h), false),
                    ),
                ],
                helpers: vec![],
                game: Command {
                    program: rt.join("pcsx2x6/AppRun").to_string_lossy().into_owned(),
                    args: vec![
                        "-nogui".into(),
                        "-batch".into(),
                        "-fullscreen".into(),
                        sys246_acgame(rt).to_string_lossy().into_owned(),
                    ],
                    env: vec![
                        (
                            "XDG_CONFIG_HOME".into(),
                            data.to_string_lossy().into_owned(),
                        ),
                        // into gamescope's X server, never the desktop's Wayland
                        ("QT_QPA_PLATFORM".into(), "xcb".into()),
                    ],
                    cwd: rt.to_path_buf(),
                },
                gamescope: s.picture(window).args(false),
                wine_prefix: None,
                game_size: window,
            })
        }
        Route::Chihiro => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::game;
    use rea_launcher::gamescope::Upscaler;

    #[test]
    fn wangan_r_runs_pcsx2x6_on_its_acgame_upscaled_for_the_window() {
        let s = Settings {
            output: Size::new(3840, 2160),
            ..Settings::default()
        };
        let rt = Path::new("/rt/wangan-r");
        let p = plan(game(GameId::WanganR), rt, Path::new("/pfx"), &s).unwrap();
        let at = |f: &str| rt.join(f).to_string_lossy().into_owned();
        assert_eq!(p.game.program, at("pcsx2x6/AppRun"));
        assert_eq!(
            *p.game.args.last().unwrap(),
            sys246_acgame(rt).to_string_lossy()
        );
        assert!(
            p.game
                .env
                .contains(&("XDG_CONFIG_HOME".into(), at("config")))
        );
        assert_eq!(p.game_size, Size::new(3840, 2160));
        assert_eq!(p.files[0], (sys246_acgame(rt), crate::sys246::acgame()));
        assert_eq!(p.files[1].0, sys246_ini(rt));
        assert!(p.files[1].1.contains("upscale_multiplier = 6\n"));
        assert!(p.wine_prefix.is_none() && p.helpers.is_empty());
        assert!(
            plan_on(
                game(GameId::WanganR),
                rt,
                Path::new("/pfx"),
                &s,
                &Host::Windows
            )
            .is_none()
        );
    }

    #[test]
    fn es3b_plan_runs_board_then_loader_with_dxvk() {
        let s = Settings::default();
        let p = plan(
            game(GameId::Mt5dxp),
            Path::new("/rt/mt5dxp"),
            Path::new("/pfx"),
            &s,
        )
        .unwrap();
        assert_eq!(p.helpers.len(), 1);
        assert_eq!(p.helpers[0].args[0], "wangan-io.exe");
        assert_eq!(p.helpers[0].args[2], r"Z:\rt\mt5dxp\input.block");
        assert_eq!(
            p.game.args,
            [
                "OpenParrotLoader64.exe",
                "OpenParrot64",
                r"Z:\rt\mt5dxp\wmn5r.exe"
            ]
        );
        assert!(
            p.game
                .env
                .contains(&("WINEDLLOVERRIDES".into(), "d3d11,dxgi=n,b".into()))
        );
        assert!(
            p.files[0]
                .1
                .contains("[General]\nInput API=DirectInput\nFreePlay=1")
        );
        assert_eq!(&p.gamescope[..4], ["-w", "1360", "-h", "768"]);
        assert!(p.gamescope.windows(2).any(|w| w == ["-F", "fsr"]));
    }

    #[test]
    fn six_rr_also_starts_amauthd_first() {
        let p = plan(
            game(GameId::Mt6rr),
            Path::new("/rt/mt6rr"),
            Path::new("/pfx"),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(p.helpers.len(), 2);
        assert!(p.helpers[1].args[2].ends_with(r"AMCUS\AMAuthd.exe"));
        assert!(p.files[0].1.contains("[Resolution]\nRES=1360x768"));
    }

    #[test]
    fn mt6_starts_amauthd_first() {
        let p = plan(
            game(GameId::Mt6),
            Path::new("/rt/mt6"),
            Path::new("/pfx"),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(p.helpers.len(), 2);
        assert!(p.helpers[1].args[2].ends_with(r"AMCUS\AMAuthd.exe"));
        assert!(p.files[0].1.contains("Banapass Connection=0"));
    }

    #[test]
    fn n2_render_size_left_unset_follows_the_output() {
        let s = Settings {
            render: None,
            output: Size::new(2560, 1440),
            ..Settings::default()
        };
        let p = plan(
            game(GameId::Mt3),
            Path::new("/rt/mt3"),
            Path::new("/pfx"),
            &s,
        )
        .unwrap();
        assert!(p.files[0].1.contains("width = 2560\nheight = 1440"));
        assert_eq!(p.game_size, Size::new(2560, 1440));
    }

    #[test]
    fn n2_plan_writes_render_size_and_preloads_the_loader() {
        let s = Settings {
            render: Some(Size::new(3840, 2160)),
            upscaler: Upscaler::Linear,
            fullscreen: false,
            ..Settings::default()
        };
        let p = plan(
            game(GameId::Mt3dxp),
            Path::new("/rt/mt3dxp"),
            Path::new("/pfx"),
            &s,
        )
        .unwrap();
        assert!(p.files[0].1.contains("width = 3840\nheight = 2160"));
        // the loader parses the whole file or nothing: every field it requires must be there
        for k in [
            "fullscreen",
            "input_emu",
            "card_emu",
            "block_sudo",
            "ignore_custom_ioctls",
            "dongle",
            "deadzone",
        ] {
            assert!(
                p.files[0]
                    .1
                    .lines()
                    .any(|l| l.starts_with(&format!("{k} = "))),
                "{k}"
            );
        }
        // the loader unwraps keyconfig.toml at start: it must always be written, every key present
        let keys = &p.files[1].1;
        assert!(p.files[1].0.ends_with("keyconfig.toml"));
        for k in [
            "TEST",
            "SERVICE",
            "CARD_INSERT",
            "GEAR_NEXT",
            "GEAR_PREVIOUS",
            "GEAR_NEUTRAL",
            "GEAR_FIRST",
            "GEAR_SECOND",
            "GEAR_THIRD",
            "GEAR_FOURTH",
            "GEAR_FIFTH",
            "GEAR_SIXTH",
            "GEAR_UP",
            "GEAR_LEFT",
            "GEAR_DOWN",
            "GEAR_RIGHT",
            "PERSPECTIVE",
            "INTRUDE",
            "GAS",
            "BRAKE",
            "WHEEL_LEFT",
            "WHEEL_RIGHT",
        ] {
            assert!(
                keys.lines().any(|l| l.starts_with(&format!("{k} = "))),
                "{k}"
            );
        }
        let rt = Path::new("/rt/mt3dxp");
        assert_eq!(p.game.program, rt.join("main").to_string_lossy());
        assert!(
            p.game
                .env
                .contains(&("LD_PRELOAD".into(), "libwal_3dxp.so".into()))
        );
        assert!(p.game.env.contains(&(
            "WANGAN_INPUT_BLOCK".into(),
            rt.join("input.block").to_string_lossy().into_owned()
        )));
        assert!(!p.gamescope.contains(&"-f".to_string()));
        assert!(!p.gamescope.contains(&"--sharpness".to_string()));
        assert!(p.wine_prefix.is_none());
    }

    #[test]
    fn bundled_wine_runs_every_wine_process_with_its_environment() {
        let host = Host::Linux {
            wine: "/pkg/components/wine/bin/wine".into(),
            wine_env: vec![(
                "LD_LIBRARY_PATH".into(),
                "/pkg/components/wine/lib/ffmpeg".into(),
            )],
        };
        let p = plan_on(
            game(GameId::Mt6),
            Path::new("/rt/mt6"),
            Path::new("/pfx"),
            &Settings::default(),
            &host,
        )
        .unwrap();
        for c in p.helpers.iter().chain([&p.game]) {
            assert_eq!(c.program, "/pkg/components/wine/bin/wine");
            assert!(c.env.contains(&(
                "LD_LIBRARY_PATH".into(),
                "/pkg/components/wine/lib/ffmpeg".into()
            )));
            assert!(c.env.iter().any(|(k, _)| k == "WINEPREFIX"));
        }
    }

    #[test]
    fn windows_runs_the_board_and_the_loader_directly() {
        let rt = Path::new("/rt/mt6rr");
        let at = |f: &str| rt.join(f).to_string_lossy().into_owned();
        let p = plan_on(
            game(GameId::Mt6rr),
            rt,
            Path::new("/pfx"),
            &Settings::default(),
            &Host::Windows,
        )
        .unwrap();
        // board first, then the auth daemon, both by full path, no Wine
        assert_eq!(p.helpers.len(), 2);
        assert_eq!(p.helpers[0].program, at("wangan-io.exe"));
        assert_eq!(
            p.helpers[0].args,
            [
                "--block".to_string(),
                at("input.block"),
                "--log".to_string(),
                at("wangan-io.log")
            ]
        );
        assert_eq!(p.helpers[1].program, at("OpenParrotLoader64.exe"));
        assert!(p.helpers[1].args[1].ends_with("AMAuthd.exe"));
        assert_eq!(p.game.program, at("OpenParrotLoader64.exe"));
        assert_eq!(p.game.args, ["OpenParrot64".to_string(), at("wmn6r.exe")]);
        for c in p.helpers.iter().chain([&p.game]) {
            assert!(c.env.is_empty(), "no Wine variables: {:?}", c.env);
            assert_eq!(c.cwd, rt);
        }
        assert!(p.gamescope.is_empty());
        assert!(p.wine_prefix.is_none());
        assert!(p.files[0].0.ends_with("teknoparrot.ini"));
        assert!(p.files[0].1.contains("[Resolution]\nRES=1360x768"));
    }

    #[test]
    fn windows_has_no_n2_or_wangan_r_plan() {
        for id in [GameId::Mt3, GameId::Mt3dxp, GameId::WanganR] {
            assert!(
                plan_on(
                    game(id),
                    Path::new("/rt"),
                    Path::new("/pfx"),
                    &Settings::default(),
                    &Host::Windows
                )
                .is_none()
            );
        }
        let mt5 = plan_on(
            game(GameId::Mt5),
            Path::new("/rt"),
            Path::new("/pfx"),
            &Settings::default(),
            &Host::Windows,
        )
        .unwrap();
        assert_eq!(mt5.helpers.len(), 1);
        assert!(mt5.game.args[1].ends_with("wmn5r.exe"));
    }

    #[test]
    fn blocked_routes_have_no_plan() {
        assert!(
            plan(
                game(GameId::Mt1),
                Path::new("/r"),
                Path::new("/p"),
                &Settings::default()
            )
            .is_none()
        );
    }
}
