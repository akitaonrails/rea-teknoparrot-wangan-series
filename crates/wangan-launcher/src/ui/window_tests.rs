//! The window, headless (egui_kittest, no GPU): each page's AccessKit tree
//! and painted text as golden snapshots, and the keyboard, pad and mouse
//! driving the real frame. The window paints its own controls, so the
//! AccessKit nodes carry no names; the dump names each one by the egui id
//! the frame gives it.
//!
//! The snapshots are in `tests/snapshots/window/<linux|windows>/` (which
//! games can start, and which options are on, differ by platform).
//! `REA_UPDATE_SNAPSHOTS=1 cargo test window_tests` rewrites them.

use std::any::Any;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use egui::accesskit::{NodeId, Toggled};
use rea_launcher::pad::Nav;
use rea_launcher_ui::testing::kittest::NodeT as _;
use rea_launcher_ui::testing::{Harness, Node, Queryable as _};
use wangan_core::settings::Settings;
use wangan_jvs::controls::Controls;

use super::*;
use crate::input::DeviceInfo;

const SIZES: [Vec2; 2] = [vec2(1280.0, 720.0), vec2(1280.0, 800.0)];
const SCREEN: Vec2 = SIZES[0];
/// The library folder the window shows (never scanned: the tests set what
/// it found).
const LIBRARY: &str = "/srv/games/Wangan";

fn platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

fn index(slug: &str) -> usize {
    GAMES.iter().position(|g| g.slug == slug).expect(slug)
}

/// A library entry for `slug` (its first edition), in a folder named
/// after the game.
fn entry(slug: &str) -> Entry<Game> {
    let game = &GAMES[index(slug)];
    Entry {
        dir: Path::new(LIBRARY).join(format!("{} [TP]", game.title)),
        file: Path::new(LIBRARY)
            .join(format!("{} [TP]", game.title))
            .join(game.main_file),
        found: Found::Known {
            game,
            edition: &game.editions[0],
        },
    }
}

fn unknown_entry() -> Entry<Game> {
    Entry {
        dir: Path::new(LIBRARY).join("Something else"),
        file: Path::new(LIBRARY).join("Something else/wmn6r.exe"),
        found: Found::Unknown {
            file: "wmn6r.exe".into(),
            digest: "00".repeat(32),
        },
    }
}

struct Window {
    h: Harness<'static, App>,
    nav: Sender<Nav>,
    pads: Arc<Mutex<Snapshot>>,
    paths: Paths,
    /// The window asked to close (quit).
    closed: bool,
    /// Kept alive while the window runs (a channel's other end).
    keep: Vec<Box<dyn Any>>,
    t: tempfile::TempDir,
}

impl Window {
    /// A window at `size` with no library yet (the setup page), no
    /// controllers and a data folder of its own.
    fn new(size: Vec2) -> Self {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths {
            data: t.path().join("data"),
            components: t.path().join("components"),
            wine: None,
        };
        let fonts = paths.fonts();
        let (input, nav, pads) = InputService::fake();
        let app = App::with_input(paths.clone(), input);
        let mut frame = eframe::Frame::_new_kittest();
        let h = Harness::builder()
            .with_size(size)
            // the real frame rate: a double click is two clicks within 0.3 s
            .with_step_dt(1.0 / 60.0)
            .with_theme(egui::Theme::Dark)
            .with_context_setup(move |ctx| {
                theme::install_fonts(ctx, &fonts);
                theme::install_style(ctx);
            })
            .build_ui_state(
                move |ui, app: &mut App| eframe::App::ui(app, ui, &mut frame),
                app,
            );
        let mut w = Self {
            h,
            nav,
            pads,
            paths,
            closed: false,
            keep: vec![],
            t,
        };
        w.steps(3);
        w
    }

    /// A window on the hub whose library holds `found`.
    fn with_games(size: Vec2, found: &[&str]) -> Self {
        let mut w = Self::new(size);
        let app = w.h.state_mut();
        app.library = Some(LIBRARY.into());
        app.entries = Some(found.iter().map(|s| entry(s)).collect());
        app.model = Model::new(GAMES.len(), true);
        w.steps(3);
        w
    }

    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.h.step();
            self.closed |= self.h.output().viewport_output.values().any(|v| {
                v.commands
                    .iter()
                    .any(|c| matches!(c, egui::ViewportCommand::Close))
            });
        }
    }

    fn app(&self) -> &App {
        self.h.state()
    }

    fn at(&self) -> (Page, Focus) {
        (self.app().model.page, self.app().model.focus)
    }

    fn selected(&self) -> &'static str {
        GAMES[self.app().model.selected].slug
    }

    /// Select a game as a click on its tile does.
    fn select(&mut self, slug: &str) {
        self.click(Id::unique(("tile", index(slug))));
        assert_eq!(self.selected(), slug);
    }

    /// Press and release `key`, a frame each (the harness runs one frame per
    /// queued event, and only the last frame's output can be read).
    fn key(&mut self, key: egui::Key) {
        self.h.key_down(key);
        self.steps(1);
        self.h.key_up(key);
        self.steps(1);
    }

    fn keys(&mut self, key: egui::Key, times: usize) {
        for _ in 0..times {
            self.key(key);
        }
    }

    fn type_text(&mut self, text: &str) {
        self.h.event(egui::Event::Text(text.into()));
        self.steps(2);
    }

    /// One menu move from a pad, as the input thread's PadNav sends it.
    fn pad(&mut self, nav: Nav) {
        self.nav.send(nav).unwrap();
        self.steps(2);
    }

    fn set_pads(&mut self, pads: Snapshot) {
        *self.pads.lock().unwrap() = pads;
        self.steps(2);
    }

    /// Click the control the frame gave `id`.
    fn click(&mut self, id: Id) {
        self.node(id).click();
        self.steps(3);
    }

    fn node(&self, id: Id) -> Node<'_> {
        let want = id.accesskit_id();
        self.h.get_by(move |n| n.locate().0 == want)
    }

    fn has(&self, id: Id) -> bool {
        let want = id.accesskit_id();
        self.h.query_by(move |n| n.locate().0 == want).is_some()
    }

    /// Whether the window paints `words` somewhere.
    fn says(&self, words: &str) -> bool {
        painted(&self.h).iter().any(|(_, t)| t.contains(words))
    }

    fn settings_file(&self, slug: &str) -> Option<String> {
        std::fs::read_to_string(self.paths.settings(slug)).ok()
    }

    fn wait(&mut self, what: &str, done: impl Fn(&App) -> bool) {
        for _ in 0..500 {
            self.steps(1);
            if done(self.app()) {
                self.steps(2);
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {what}");
    }
}

// ---------------------------------------------------------------- the dump

/// Every control the frame makes, by the source of its egui id.
fn id_names() -> Vec<(NodeId, String)> {
    let mut v: Vec<(NodeId, String)> = [
        "play",
        "game-settings",
        "rescan",
        "folder",
        "use-folder",
        "cancel-folder",
        "look-for-files",
        "files-back",
        "dismiss",
    ]
    .iter()
    .map(|b| {
        (
            Id::unique(("button", *b)).accesskit_id(),
            format!("button {b}"),
        )
    })
    .collect();
    for tab in Tab::ALL {
        v.push((
            Id::unique(("tab", tab.label())).accesskit_id(),
            format!("tab {}", tab.label()),
        ));
    }
    for (i, g) in GAMES.iter().enumerate() {
        v.push((
            Id::unique(("tile", i)).accesskit_id(),
            format!("tile {}", g.slug),
        ));
    }
    for i in 0..model::settings_items().len() {
        for part in ["row", "left", "right"] {
            v.push((Id::unique((part, i)).accesskit_id(), format!("{part} {i}")));
        }
    }
    v.push((
        Id::unique("folder-field").accesskit_id(),
        "folder-field".into(),
    ));
    v
}

fn rounded(r: egui::accesskit::Rect) -> String {
    format!(
        "{},{} {},{}",
        r.x0.round(),
        r.y0.round(),
        r.x1.round(),
        r.y1.round()
    )
}

fn accesskit_tree(h: &Harness<'_, App>) -> String {
    fn walk(n: &Node<'_>, depth: usize, names: &[(NodeId, String)], out: &mut String) {
        let a = n.accesskit_node();
        let id = a.locate().0;
        let _ = write!(out, "{}{:?}", "  ".repeat(depth), a.role());
        if let Some((_, name)) = names.iter().find(|(n, _)| *n == id) {
            let _ = write!(out, " [{name}]");
        }
        for (what, text) in [
            ("label", a.label()),
            ("value", a.value()),
            ("description", a.description()),
        ] {
            if let Some(t) = text {
                let _ = write!(out, " {what}={t:?}");
            }
        }
        if a.is_disabled() {
            out.push_str(" disabled");
        }
        if let Some(t) = a.toggled() {
            let _ = write!(out, " toggled={}", t == Toggled::True);
        }
        if let Some(b) = a.bounding_box() {
            let _ = write!(out, " @ {}", rounded(b));
        }
        out.push('\n');
        for c in n.children() {
            walk(&c, depth + 1, names, out);
        }
    }
    let mut out = String::new();
    walk(&h.root(), 0, &id_names(), &mut out);
    out
}

/// The text the last frame painted, with its position (points, rounded).
fn painted(h: &Harness<'_, App>) -> Vec<(Pos2, String)> {
    fn collect(shape: &Shape, out: &mut Vec<(Pos2, String)>) {
        match shape {
            Shape::Text(t) if !t.galley.text().is_empty() => {
                out.push((t.pos, t.galley.text().to_string()));
            }
            Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        collect(&clipped.shape, &mut out);
    }
    out
}

fn dump(h: &Harness<'_, App>) -> String {
    let mut out = String::from("-- accesskit\n");
    out += &accesskit_tree(h);
    out += "-- painted text\n";
    for (at, text) in painted(h) {
        let _ = writeln!(out, "{},{} {text:?}", at.x.round(), at.y.round());
    }
    out
}

/// A page in each window size, against `window/<platform>/<name>.txt`.
fn golden(name: &str, open: impl Fn(Vec2) -> Window) {
    let mut out = String::new();
    for size in SIZES {
        let w = open(size);
        let _ = writeln!(out, "== {}x{}", size.x, size.y);
        out += &dump(&w.h);
    }
    rea_launcher::snapshot!(&format!("window/{}/{name}.txt", platform()), out);
}

// ---------------------------------------------------------------- every page

#[test]
fn page_setup() {
    golden("setup", Window::new);
}

#[test]
fn page_setup_from_settings() {
    golden("setup-from-settings", |size| {
        let mut w = Window::with_games(size, &[]);
        w.key(egui::Key::PageUp);
        w.keys(egui::Key::ArrowDown, 7);
        w.key(egui::Key::Enter);
        assert_eq!(w.at(), (Page::FirstRun, Focus::Field));
        w
    });
}

#[test]
fn page_hub_looking() {
    golden("hub-looking", |size| {
        let mut w = Window::with_games(size, &[]);
        w.h.state_mut().entries = None;
        w.steps(2);
        w
    });
}

#[test]
fn page_hub_not_found() {
    golden("hub-not-found", |size| Window::with_games(size, &[]));
}

#[test]
fn page_hub_needs_files() {
    golden("hub-needs-files", |size| {
        Window::with_games(size, &["wangan-r"])
    });
}

#[test]
fn page_hub_ready() {
    golden("hub-ready", |size| {
        let mut w = Window::with_games(size, &["mt3", "mt5", "mt6rr"]);
        w.select("mt5");
        w
    });
}

#[test]
fn page_hub_locked() {
    golden("hub-locked", |size| {
        let mut w = Window::with_games(size, &["mt1"]);
        w.select("mt1");
        w
    });
}

#[test]
fn page_settings_n2() {
    golden("settings-n2", |size| {
        let mut w = Window::with_games(size, &["mt3"]);
        w.h.state_mut()
            .entries
            .as_mut()
            .unwrap()
            .push(unknown_entry());
        w.select("mt3");
        w.key(egui::Key::PageUp);
        assert_eq!(w.at(), (Page::Settings, Focus::Item(0)));
        w
    });
}

#[test]
fn page_settings_es3b_after_a_change() {
    golden("settings-es3b-saved", |size| {
        let mut w = Window::with_games(size, &["mt6rr"]);
        w.select("mt6rr");
        w.key(egui::Key::PageUp);
        w.keys(egui::Key::ArrowDown, 5);
        w.key(egui::Key::ArrowRight);
        assert!(w.settings_file("mt6rr").is_some());
        w
    });
}

#[test]
fn page_controls_without_controllers() {
    golden("controls-none", |size| {
        let mut w = Window::with_games(size, &[]);
        w.key(egui::Key::PageDown);
        assert_eq!(w.at().0, Page::Controls);
        w
    });
}

fn two_controllers() -> Snapshot {
    Snapshot {
        devices: vec![
            DeviceInfo {
                name: "Logitech G29 Driving Force Racing Wheel".into(),
                kind: Kind::Wheel,
                haptic: true,
            },
            DeviceInfo {
                name: "Xbox Wireless Controller".into(),
                kind: Kind::Pad,
                haptic: false,
            },
        ],
        driver: Some(0),
        controls: Controls {
            wheel: -0.25,
            gas: 0.5,
            brake: 0.1,
            gear: 3,
            intrude: true,
            ..Controls::default()
        },
        writing: false,
        error: None,
    }
}

#[test]
fn page_controls_with_controllers() {
    golden("controls-two", |size| {
        let mut w = Window::with_games(size, &[]);
        w.set_pads(two_controllers());
        w.key(egui::Key::PageDown);
        w
    });
}

#[test]
fn page_controls_unavailable() {
    golden("controls-unavailable", |size| {
        let mut w = Window::with_games(size, &[]);
        w.set_pads(Snapshot {
            error: Some("controllers unavailable: no SDL".into()),
            ..Snapshot::default()
        });
        w.key(egui::Key::PageDown);
        w
    });
}

#[test]
fn page_add_files() {
    golden("add-files", |size| {
        let mut w = Window::with_games(size, &["wangan-r"]);
        w.key(egui::Key::Enter);
        assert_eq!(w.at(), (Page::AddFiles, Focus::Field));
        // it offers ~/Downloads when that folder exists: the same text everywhere
        w.h.state_mut().folder = "/srv/bios".into();
        w.steps(2);
        w
    });
}

#[test]
fn page_message() {
    golden("message", |size| {
        let mut w = Window::with_games(size, &["mt5"]);
        w.h.state_mut().message = Some(Message {
            title: "Wangan Midnight Maximum Tune 5 couldn't start".into(),
            body: "Reading /srv/games/Wangan/MT5: no such folder.\n\nCheck that the game \
                   folder is complete, then try again. Details are in /data/launcher.log."
                .into(),
        });
        w.steps(2);
        w
    });
}

#[test]
fn page_running() {
    golden("running", |size| {
        let mut w = Window::with_games(size, &["mt5"]);
        let (tx, rx) = mpsc::channel::<Result<Prepared, String>>();
        w.keep.push(Box::new(tx));
        w.h.state_mut().running = Some(Running {
            game: &GAMES[index("mt5")],
            stage: Stage::Preparing(rx),
        });
        w.steps(2);
        w
    });
}

// ---------------------------------------------------------------- driving it

#[test]
fn the_keyboard_moves_through_the_hub_and_the_top_bar() {
    let mut w = Window::with_games(SCREEN, &["mt5"]);
    assert_eq!(w.at(), (Page::Hub, Focus::Tile));
    w.key(egui::Key::ArrowRight);
    assert_eq!(w.selected(), GAMES[1].slug);
    w.keys(egui::Key::ArrowRight, 20);
    assert_eq!(w.selected(), GAMES[GAMES.len() - 1].slug, "clamped");
    w.select("mt5");
    w.key(egui::Key::ArrowUp);
    assert_eq!(w.at(), (Page::Hub, Focus::Play));
    w.key(egui::Key::ArrowRight);
    assert_eq!(w.at(), (Page::Hub, Focus::GameSettings));
    w.key(egui::Key::ArrowUp);
    assert_eq!(w.at(), (Page::Hub, Focus::Tab(Tab::Games)));
    w.key(egui::Key::ArrowRight);
    w.key(egui::Key::Space);
    assert_eq!(w.at(), (Page::Controls, Focus::Tab(Tab::Controls)));
    w.key(egui::Key::PageDown);
    assert_eq!(w.at(), (Page::Settings, Focus::Item(0)));
    w.key(egui::Key::PageDown);
    assert_eq!(w.at(), (Page::Hub, Focus::Tile));
    w.key(egui::Key::PageUp);
    assert_eq!(w.at().0, Page::Settings);
    w.key(egui::Key::Backspace);
    assert_eq!(w.at(), (Page::Hub, Focus::Tile));
    assert!(!w.closed);
    w.key(egui::Key::Backspace);
    assert!(!w.closed, "B never quits");
    w.key(egui::Key::Escape);
    assert!(w.closed, "Esc on the hub quits");
}

#[test]
fn a_pad_drives_every_page() {
    let mut w = Window::with_games(SCREEN, &["mt3", "wangan-r"]);
    w.pad(Nav::Right);
    assert_eq!(w.selected(), GAMES[1].slug);
    w.pad(Nav::Left);
    assert_eq!(w.selected(), "wangan-r");
    // Wangan R waits for its board files: A opens the add-files page
    assert!(w.says("Add files"));
    w.pad(Nav::Press);
    assert_eq!(w.at(), (Page::AddFiles, Focus::Field));
    w.pad(Nav::Down);
    assert_eq!(w.at(), (Page::AddFiles, Focus::UseFolder));
    w.pad(Nav::Back);
    assert_eq!(w.at(), (Page::Hub, Focus::Play));
    w.pad(Nav::Up);
    assert_eq!(w.at(), (Page::Hub, Focus::Tab(Tab::Games)));
    w.pad(Nav::Right);
    w.pad(Nav::Press);
    assert_eq!(w.at(), (Page::Controls, Focus::Tab(Tab::Controls)));
    w.pad(Nav::NextTab);
    assert_eq!(w.at(), (Page::Settings, Focus::Item(0)));
    w.pad(Nav::PreviousTab);
    assert_eq!(w.at().0, Page::Controls);
    w.pad(Nav::Back);
    assert_eq!(w.at(), (Page::Hub, Focus::Tile));
    // Y (the view button) opens the selected game's settings
    let mut pads = Snapshot::default();
    pads.controls.view = true;
    w.set_pads(pads);
    assert_eq!(w.at(), (Page::Settings, Focus::Item(0)));
    w.set_pads(Snapshot::default());
    // A on a settings row moves its value on: the dead zone, on everywhere
    for _ in 0..5 {
        w.pad(Nav::Down);
    }
    assert!(w.says("5%"));
    w.pad(Nav::Press);
    assert!(w.says("6%"));
    w.pad(Nav::Play);
    assert_eq!(w.at().0, Page::Hub, "Start goes back to the hub");
    assert!(!w.closed);
    w.pad(Nav::Play);
    assert!(w.closed, "Start on the hub quits");
}

#[test]
fn the_mouse_clicks_tabs_tiles_buttons_and_arrows() {
    let mut w = Window::with_games(SCREEN, &["mt3", "mt5"]);
    w.click(Id::unique(("tab", "Controls")));
    assert_eq!(w.at().0, Page::Controls);
    w.click(Id::unique(("tab", "Games")));
    assert_eq!(w.at().0, Page::Hub);
    w.select("mt3");
    w.click(Id::unique(("button", "game-settings")));
    assert_eq!(w.at(), (Page::Settings, Focus::Item(0)));
    // the dead zone row's ▶
    w.click(Id::unique(("right", 5)));
    assert_eq!(w.at(), (Page::Settings, Focus::Item(5)));
    let (s, _) = Settings::parse(&w.settings_file("mt3").unwrap());
    assert!((s.deadzone - 0.06).abs() < 1e-6, "{s:?}");
    assert!(w.says("Saved. The game uses it the next time you press Play."));
    // a row's body: A (the next choice)
    w.click(Id::unique(("row", 3)));
    assert_eq!(w.at(), (Page::Settings, Focus::Item(3)));
    w.click(Id::unique(("button", "folder")));
    assert_eq!(w.at(), (Page::FirstRun, Focus::Field));
    assert_eq!(w.app().folder, LIBRARY);
    w.click(Id::unique(("button", "cancel-folder")));
    assert_eq!(w.at(), (Page::Settings, Focus::Item(7)));
    w.click(Id::unique(("tab", "Games")));
    // a double click on a tile presses it: Play (half a second after the
    // last click, or egui counts a triple click)
    w.steps(30);
    let tile = Id::unique(("tile", index("mt5")));
    w.node(tile).click();
    w.steps(1);
    w.node(tile).click();
    w.steps(2);
    assert!(w.app().running.is_some() || w.app().message.is_some());
}

/// Each option the window changes is in the game's settings file as the
/// command line writes it, and a row that is off changes nothing.
#[test]
fn every_option_is_saved_like_the_command_line_saves_it() {
    let mut w = Window::with_games(SCREEN, &["mt3", "mt5"]);
    w.select("mt3");
    w.key(egui::Key::PageUp);
    let mut mirror = Form::new(Settings::default(), GAMES[index("mt3")].route);
    let mut changed = 0;
    for (i, row) in Row::ALL.into_iter().enumerate() {
        // the default output is the largest: ◀ is the change
        let (key, d) = if row == Row::Output {
            (egui::Key::ArrowLeft, -1)
        } else {
            (egui::Key::ArrowRight, 1)
        };
        assert_eq!(w.at(), (Page::Settings, Focus::Item(i)));
        w.key(key);
        if mirror.adjust(row, d, false) {
            changed += 1;
            let (saved, problems) = Settings::parse(&w.settings_file("mt3").unwrap());
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(saved, mirror.settings, "{row:?}");
        } else {
            assert_eq!(
                w.settings_file("mt3").map(|t| Settings::parse(&t).0),
                (changed > 0).then(|| mirror.settings.clone()),
                "{row:?} is off here and saves nothing"
            );
        }
        w.key(egui::Key::ArrowDown);
    }
    assert!(
        changed >= 2,
        "the dead zone and the render size change everywhere"
    );

    let s = &mirror.settings;
    let render = s.render.map_or("native".into(), |z| z.to_string());
    let cli = tempfile::tempdir().unwrap();
    let cli_paths = Paths {
        data: cli.path().to_path_buf(),
        ..w.paths.clone()
    };
    let options: Vec<(String, String)> = [
        ("output", s.output.to_string()),
        ("upscaler", s.upscaler.key().to_string()),
        ("sharpness", s.sharpness.to_string()),
        ("fullscreen", s.fullscreen.to_string()),
        ("render", render),
        ("deadzone", s.deadzone.to_string()),
    ]
    .map(|(k, v)| (k.to_string(), v))
    .into();
    let mt3 = wangan_core::catalog::by_slug("mt3").unwrap();
    crate::save_options(&cli_paths, mt3, &options).unwrap();
    assert_eq!(
        w.settings_file("mt3").unwrap(),
        std::fs::read_to_string(cli_paths.settings("mt3")).unwrap()
    );

    // MT5 draws at its own size: its render row is off and saves nothing
    w.key(egui::Key::Escape);
    w.select("mt5");
    w.key(egui::Key::PageUp);
    w.keys(egui::Key::ArrowDown, 4);
    w.key(egui::Key::ArrowRight);
    assert!(w.settings_file("mt5").is_none());
    assert!(w.says("This game draws at its own size"));
}

#[test]
fn the_setup_page_takes_a_typed_folder() {
    let mut w = Window::new(SCREEN);
    assert_eq!(w.at(), (Page::FirstRun, Focus::Field));
    w.key(egui::Key::Enter);
    assert_eq!(
        w.app().folder_error.as_deref(),
        Some("Type the folder's path first.")
    );
    w.type_text("/no/such/folder");
    assert_eq!(w.app().folder_error, None, "typing clears the error");
    w.key(egui::Key::Enter);
    assert!(w.says("isn't a folder this computer can open"));
    // Backspace takes a character, Ctrl+Backspace a path part
    w.key(egui::Key::Backspace);
    assert_eq!(w.app().folder, "/no/such/folde");
    w.h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Backspace);
    w.steps(2);
    assert_eq!(w.app().folder, "/no/such/");
    w.h.state_mut().folder.clear();
    let games = w.t.path().join("games");
    std::fs::create_dir_all(&games).unwrap();
    w.type_text(&games.display().to_string());
    w.key(egui::Key::Enter);
    assert_eq!(w.at(), (Page::Hub, Focus::Tile));
    assert_eq!(paths::read_library(&w.paths.config()), Some(games));
    w.wait("the scan", |a| a.entries.is_some());
    assert!(w.says(&format!("Your games · 0 of {} found", GAMES.len())));
}

#[test]
fn the_setup_page_without_a_library_quits_on_escape() {
    let mut w = Window::new(SCREEN);
    w.key(egui::Key::PageDown);
    assert_eq!(w.at().0, Page::FirstRun, "no top bar yet");
    w.key(egui::Key::Escape);
    assert!(w.closed);
}

#[test]
fn add_files_searches_the_typed_folder() {
    let mut w = Window::with_games(SCREEN, &["wangan-r"]);
    let bios = w.t.path().join("bios");
    std::fs::create_dir_all(&bios).unwrap();
    std::fs::write(bios.join("r27v1602f.7d"), vec![0u8; 2_097_152]).unwrap();
    w.click(Id::unique(("button", "play")));
    assert_eq!(w.at(), (Page::AddFiles, Focus::Field));
    w.h.state_mut().folder.clear();
    w.type_text(&bios.display().to_string());
    w.key(egui::Key::Enter);
    let (note, done) = w.app().files_note.clone().unwrap();
    assert!(!done);
    assert!(
        note.starts_with("None of the missing files were found in")
            && note.ends_with(
                "(looked at 1 file of the right size). Choose the folder or zip that has them."
            ),
        "{note}"
    );
    assert!(w.says("Missing"));
    w.click(Id::unique(("button", "files-back")));
    assert_eq!(w.at(), (Page::Hub, Focus::Play));
    assert!(w.says("Needs board files"));
}

#[test]
fn a_game_that_cannot_start_says_why_and_any_press_dismisses_it() {
    let mut w = Window::with_games(SCREEN, &["mt5"]);
    w.select("mt5");
    w.key(egui::Key::Enter);
    w.wait("the error", |a| a.message.is_some());
    assert!(w.says("Wangan Midnight Maximum Tune 5 couldn't start"));
    assert!(w.has(Id::unique(("button", "dismiss"))));
    w.key(egui::Key::ArrowRight);
    assert!(
        w.app().message.is_some(),
        "only A, B, Esc or Start dismiss it"
    );
    assert_eq!(w.selected(), "mt5");
    w.pad(Nav::Press);
    assert!(w.app().message.is_none());
    assert!(!w.closed);
}

#[test]
fn rescan_reads_the_folder_again() {
    let mut w = Window::with_games(SCREEN, &["mt5"]);
    w.key(egui::Key::PageUp);
    assert!(w.says(LIBRARY));
    assert!(w.says(&format!("1 of {} games found.", GAMES.len())));
    w.click(Id::unique(("button", "rescan")));
    w.wait("the rescan", |a| a.scan.is_none());
    assert!(w.says(&format!("0 of {} games found.", GAMES.len())));
}
