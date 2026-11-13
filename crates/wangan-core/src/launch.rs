//! What to write and what to run for a game, as plain data.
//!
//! [`plan`] turns a game, its runtime folder and the settings into the
//! files the launcher writes and the processes it starts, so tests can
//! check the whole wiring (setting → file / argument / environment)
//! without starting anything.

use std::path::{Path, PathBuf};

use crate::catalog::{Game, GameId, Route};
use crate::settings::{Settings, Size};

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

/// Where the input block lives inside a runtime.
pub fn block_path(rt: &Path) -> PathBuf {
    rt.join("input.block")
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

fn gamescope_args(s: &Settings, game: Size) -> Vec<String> {
    let mut a = vec![
        "-w".into(),
        game.w.to_string(),
        "-h".into(),
        game.h.to_string(),
        "-W".into(),
        s.output.w.to_string(),
        "-H".into(),
        s.output.h.to_string(),
        "-r".into(),
        "60".into(),
        "-F".into(),
        s.upscaler.key().into(),
    ];
    if matches!(
        s.upscaler,
        crate::settings::Upscaler::Fsr | crate::settings::Upscaler::Nis
    ) {
        a.push("--sharpness".into());
        a.push(s.sharpness.to_string());
    }
    if s.fullscreen {
        a.push("-f".into());
    }
    a
}

/// ES3B native size (all five games render 1360×768).
pub const ES3B_SIZE: Size = Size::new(1360, 768);

pub fn plan(game: &Game, rt: &Path, prefix: &Path, s: &Settings) -> Option<Plan> {
    match game.route {
        Route::Es3b => {
            let exe = match game.id {
                GameId::Mt5 | GameId::Mt5dxp => "wmn5r.exe",
                _ => "wmn6r.exe",
            };
            let wine_env = vec![
                (
                    "WINEPREFIX".to_string(),
                    prefix.to_string_lossy().into_owned(),
                ),
                ("WINEDEBUG".to_string(), "-all".to_string()),
                ("WINEDLLOVERRIDES".to_string(), "d3d11,dxgi=n,b".to_string()),
            ];
            let mut helpers = vec![Command {
                program: "wine".into(),
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
            if game.id == GameId::Mt6 {
                helpers.push(Command {
                    program: "wine".into(),
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
                    program: "wine".into(),
                    args: vec![
                        "OpenParrotLoader64.exe".into(),
                        "OpenParrot64".into(),
                        wine_path(&rt.join(exe)),
                    ],
                    env: wine_env,
                    cwd: rt.to_path_buf(),
                },
                gamescope: gamescope_args(s, ES3B_SIZE),
                wine_prefix: Some(prefix.to_path_buf()),
                game_size: ES3B_SIZE,
            })
        }
        Route::NativeN2 => {
            let render = s.render.unwrap_or(Size::new(1280, 720));
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
                gamescope: gamescope_args(s, render),
                wine_prefix: None,
                game_size: render,
            })
        }
        Route::Chihiro | Route::System246 => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::game;
    use crate::settings::Upscaler;

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
        for k in ["fullscreen", "input_emu", "card_emu", "block_sudo", "ignore_custom_ioctls", "dongle", "deadzone"] {
            assert!(p.files[0].1.lines().any(|l| l.starts_with(&format!("{k} = "))), "{k}");
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
        assert_eq!(p.game.program, "/rt/mt3dxp/main");
        assert!(
            p.game
                .env
                .contains(&("LD_PRELOAD".into(), "libwal_3dxp.so".into()))
        );
        assert!(
            p.game
                .env
                .contains(&("WANGAN_INPUT_BLOCK".into(), "/rt/mt3dxp/input.block".into()))
        );
        assert!(!p.gamescope.contains(&"-f".to_string()));
        assert!(!p.gamescope.contains(&"--sharpness".to_string()));
        assert!(p.wine_prefix.is_none());
    }

    #[test]
    fn blocked_routes_have_no_plan() {
        assert!(
            plan(
                game(GameId::WanganR),
                Path::new("/r"),
                Path::new("/p"),
                &Settings::default()
            )
            .is_none()
        );
    }
}
