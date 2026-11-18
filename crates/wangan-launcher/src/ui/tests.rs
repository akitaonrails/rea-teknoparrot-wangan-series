//! The window over the real backend, headless (`rea_launcher_ui::hub::testing`):
//! what the Wangan games add to the hub (their board files, rows, sessions,
//! controls), the keyboard, pad and mouse driving it, and golden AccessKit
//! dumps of every page in `tests/snapshots/window/<linux|windows>/` (which
//! games can start, and which options are on, differ by system).
//! `REA_UPDATE_SNAPSHOTS=1 cargo test ui::tests` rewrites them.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui::accesskit::Role;
use eframe::egui::{self, Vec2};
use rea_launcher::hub::{Focus, Page, Tab, cli};
use rea_launcher::library::Entry;
use rea_launcher::pad;
use rea_launcher::wheel::{Controls, Kind};
use rea_launcher_native::feeder::DeviceInfo;
use rea_launcher_ui::hub::HubApp;
use rea_launcher_ui::hub::testing::{self, SIZE_720, SIZE_800, says};
use rea_launcher_ui::testing::{Harness, NodeT, Queryable};

use super::*;
use wangan_core::settings;

/// The tests' controllers, scan, next session and extra-page start.
#[derive(Default)]
pub(crate) struct Fake {
    pub snapshot: Option<Snapshot>,
    pub session: Option<Session>,
    pub scanner: RefCell<Option<Scanner<Scan<Game>>>>,
}

type Hub = Harness<'static, HubApp<Wangan>>;

/// The library folder the window shows (never scanned: the tests set what
/// it found).
const LIBRARY: &str = "/srv/games/Wangan";

fn platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

fn paths_in(t: &Path) -> HubPaths {
    HubPaths {
        data: t.join("data"),
        components: t.join("components"),
        dev_assets: None,
    }
}

fn index(slug: &str) -> usize {
    GAMES.iter().position(|g| g.slug == slug).expect(slug)
}

/// A library entry for `slug` (its first edition), in a folder named after
/// the game.
fn entry(slug: &str) -> Entry<Game> {
    let game = &GAMES[index(slug)];
    let dir = Path::new(LIBRARY).join(format!("{} [TP]", game.title));
    Entry {
        file: dir.join(game.main_file),
        dir,
        found: Known::Known {
            game,
            edition: &game.editions[0],
        },
    }
}

fn unknown_entry() -> Entry<Game> {
    let dir = Path::new(LIBRARY).join("Something else");
    Entry {
        file: dir.join("wmn6r.exe"),
        dir,
        found: Known::Unknown {
            file: "wmn6r.exe".into(),
            digest: "00".repeat(32),
        },
    }
}

fn library(found: &[&str]) -> Scan<Game> {
    Scan {
        entries: found.iter().map(|s| entry(s)).collect(),
        problems: Vec::new(),
    }
}

/// A window whose `launcher.cfg` is `config` (none: the first run), its
/// first scan done.
fn window(t: &Path, config: Option<&str>, size: Vec2) -> Hub {
    let paths = paths_in(t);
    if let Some(text) = config {
        std::fs::create_dir_all(&paths.data).unwrap();
        std::fs::write(paths.config_file(), text).unwrap();
    }
    let backend = Wangan::new(paths.clone(), None);
    let mut h = testing::harness(spec(paths), backend, size);
    testing::wait(&mut h, "the scan", |a| !a.scanning());
    h
}

/// A hub at `size` whose library holds `found`.
fn hub(t: &Path, size: Vec2, found: &[&str]) -> Hub {
    let mut h = window(t, Some(&format!("library = {LIBRARY}\n")), size);
    h.state_mut().set_library(library(found));
    h.run_steps(3);
    h
}

fn at(h: &Hub) -> (Page, Focus) {
    (h.state().model().page, h.state().model().focus)
}

fn selected(h: &Hub) -> &'static str {
    GAMES[h.state().model().selected].slug
}

fn select(h: &mut Hub, slug: &str) {
    let title = catalog::by_slug(slug).unwrap().title;
    h.get_by_role_and_label(Role::RadioButton, title).click();
    h.run_steps(3);
    assert_eq!(selected(h), slug);
}

fn click_button(h: &mut Hub, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    h.run_steps(3);
}

fn open_tab(h: &mut Hub, tab: Tab) {
    h.get_by_role_and_label(Role::Tab, tab.label()).click();
    h.run_steps(3);
}

/// Press and release `key`, a frame each.
fn key(h: &mut Hub, key: egui::Key) {
    h.key_press(key);
    h.run_steps(2);
}

fn keys(h: &mut Hub, k: egui::Key, times: usize) {
    for _ in 0..times {
        key(h, k);
    }
}

/// The pad's menu buttons down now (XInput bits), for two frames.
fn pad(h: &mut Hub, bits: u16) {
    h.state_mut().backend_mut().fake.snapshot = Some(Snapshot {
        nav: bits,
        ..Snapshot::default()
    });
    h.run_steps(2);
}

/// One press of a pad button: down, then up.
fn press(h: &mut Hub, bits: u16) {
    pad(h, bits);
    pad(h, 0);
}

fn closed(h: &Hub) -> bool {
    testing::commands(h)
        .iter()
        .any(|c| matches!(c, egui::ViewportCommand::Close))
}

/// Whether the frame that reads `key` going down closes the window.
fn key_closes(h: &mut Hub, key: egui::Key) -> bool {
    h.key_down(key);
    h.run_steps(1);
    let closes = closed(h);
    h.key_up(key);
    h.run_steps(1);
    closes
}

/// Whether the frame that reads pad buttons `bits` closes the window.
fn pad_closes(h: &mut Hub, bits: u16) -> bool {
    h.state_mut().backend_mut().fake.snapshot = Some(Snapshot {
        nav: bits,
        ..Snapshot::default()
    });
    h.run_steps(1);
    closed(h)
}

/// The value a settings row shows.
fn row(h: &Hub, label: &str) -> String {
    h.get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// Why a row is off, as its description says.
fn row_reason(h: &Hub, label: &str) -> Option<String> {
    let n = h.get_by_role_and_label(Role::Button, label);
    let a = n.accesskit_node();
    a.is_disabled().then(|| a.description().unwrap_or_default())
}

fn settings_file(t: &Path, slug: &str) -> Option<String> {
    std::fs::read_to_string(paths_in(t).settings(slug)).ok()
}

/// Frames for 300 ms: the session thread's news reaches the window.
fn settle(h: &mut Hub) {
    for _ in 0..30 {
        h.run_steps(1);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The dump with the test's own folder shown as `/data` (as the dump spells
/// it: Debug turns a Windows `\` into `\\`).
fn dump(h: &Hub, t: &Path) -> String {
    let data = format!("{:?}", display_path(&paths_in(t).data));
    testing::dump(h).replace(data.trim_matches('"'), "/data")
}

/// A page in each window size, against `window/<system>/<name>.txt`.
fn golden(name: &str, open: impl Fn(&Path, Vec2) -> Hub) {
    let mut out = String::new();
    for size in [SIZE_720, SIZE_800] {
        let t = tempfile::tempdir().unwrap();
        let h = open(t.path(), size);
        out.push_str(&format!("== {}x{}\n", size.x, size.y));
        out.push_str(&dump(&h, t.path()));
    }
    rea_launcher::snapshot!(&format!("window/{}/{name}.txt", platform()), out);
}

// ------------------------------------------------------------ every page

#[test]
fn page_setup() {
    golden("setup", |t, size| window(t, None, size));
}

#[test]
fn page_setup_from_settings() {
    golden("setup-from-settings", |t, size| {
        let mut h = hub(t, size, &[]);
        open_tab(&mut h, Tab::Settings);
        click_button(&mut h, "Game folders");
        assert_eq!(at(&h), (Page::Setup, Focus::Field(0)));
        h
    });
}

#[test]
fn page_hub_looking() {
    golden("hub-looking", |t, size| {
        let paths = paths_in(t);
        std::fs::create_dir_all(&paths.data).unwrap();
        std::fs::write(paths.config_file(), format!("library = {LIBRARY}\n")).unwrap();
        let backend = Wangan::new(paths.clone(), None);
        // a scan that never ends while the page is dumped
        let (_tx, rx) = mpsc::channel::<()>();
        *backend.fake.scanner.borrow_mut() = Some(Box::new(move |_: &[PathBuf]| {
            let _ = rx.recv();
            library(&[])
        }));
        let mut h = testing::harness(spec(paths), backend, size);
        h.run_steps(3);
        assert!(h.state().scanning());
        h
    });
}

#[test]
fn page_hub_not_found() {
    golden("hub-not-found", |t, size| hub(t, size, &[]));
}

#[test]
fn page_hub_needs_files() {
    golden("hub-needs-files", |t, size| hub(t, size, &["wangan-r"]));
}

#[test]
fn page_hub_ready() {
    golden("hub-ready", |t, size| {
        let mut h = hub(t, size, &["mt3", "mt5", "mt6rr"]);
        select(&mut h, "mt5");
        h
    });
}

#[test]
fn page_hub_locked() {
    golden("hub-locked", |t, size| {
        let mut h = hub(t, size, &["mt1"]);
        select(&mut h, "mt1");
        h
    });
}

#[test]
fn page_settings_n2() {
    golden("settings-n2", |t, size| {
        let mut h = hub(t, size, &["mt3"]);
        let mut scan = library(&["mt3"]);
        scan.entries.push(unknown_entry());
        h.state_mut().set_library(scan);
        select(&mut h, "mt3");
        open_tab(&mut h, Tab::Settings);
        assert_eq!(at(&h), (Page::Settings, Focus::Item(0)));
        h
    });
}

#[test]
fn page_settings_es3b_after_a_change() {
    golden("settings-es3b-saved", |t, size| {
        let mut h = hub(t, size, &["mt6rr"]);
        select(&mut h, "mt6rr");
        open_tab(&mut h, Tab::Settings);
        click_button(&mut h, "Dead zone: next");
        assert!(
            settings_file(t, "mt6rr")
                .unwrap()
                .contains("deadzone = 0.06")
        );
        h
    });
}

#[test]
fn page_controls_without_controllers() {
    golden("controls-none", |t, size| {
        let mut h = hub(t, size, &[]);
        key(&mut h, egui::Key::PageDown);
        assert_eq!(at(&h).0, Page::Controls);
        h
    });
}

fn two_controllers() -> Snapshot {
    let mut controls = Controls {
        steer: -0.25,
        accel: 0.5,
        brake: 0.1,
        buttons: 0,
    };
    controls.press(Button::Intrude);
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
        controls,
        ..Snapshot::default()
    }
}

#[test]
fn page_controls_with_controllers() {
    golden("controls-two", |t, size| {
        let mut h = hub(t, size, &[]);
        key(&mut h, egui::Key::PageDown);
        shift_up_twice(&mut h);
        h.state_mut().backend_mut().fake.snapshot = Some(two_controllers());
        h.run_steps(2);
        h
    });
}

/// Two shift-up presses, a frame each and released between.
fn shift_up_twice(h: &mut Hub) {
    for buttons in [Button::GearUp.bit(), 0, Button::GearUp.bit(), 0] {
        let mut snap = two_controllers();
        snap.controls.buttons = buttons;
        h.state_mut().backend_mut().fake.snapshot = Some(snap);
        h.run_steps(1);
    }
}

#[test]
fn the_controls_page_shows_the_gear_the_paddles_chose() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &[]);
    open_tab(&mut h, Tab::Controls);
    assert!(says(&h, "Gear: 1"), "first gear at the start");
    shift_up_twice(&mut h);
    assert!(says(&h, "Gear: 3"));
}

#[test]
fn page_controls_unavailable() {
    golden("controls-unavailable", |t, size| {
        let mut h = hub(t, size, &[]);
        h.state_mut().backend_mut().fake.snapshot = Some(Snapshot {
            error: Some("controllers unavailable: no SDL".into()),
            ..Snapshot::default()
        });
        key(&mut h, egui::Key::PageDown);
        h
    });
}

/// Empties the focused text field, a character at a time.
fn clear_field(h: &mut Hub) {
    for _ in 0..40 {
        h.key_press(egui::Key::Backspace);
    }
    h.run_steps(1);
}

/// Clears the extra page's field and types `text` (it offers ~/Downloads
/// when that folder exists: the same text everywhere).
fn type_in_extra(h: &mut Hub, text: &str) {
    clear_field(h);
    h.event(egui::Event::Text(text.into()));
    h.run_steps(2);
}

#[test]
fn page_add_files() {
    golden("add-files", |t, size| {
        let mut h = hub(t, size, &["wangan-r"]);
        key(&mut h, egui::Key::Enter);
        assert_eq!(at(&h), (Page::Extra, Focus::Field(0)));
        type_in_extra(&mut h, "/srv/bios");
        h
    });
}

#[test]
fn page_message() {
    golden("message", |t, size| {
        let mut h = hub(t, size, &["mt5"]);
        select(&mut h, "mt5");
        h.state_mut().backend_mut().fake.session = Some(Box::new(|_: &Reporter| {
            Err("reading /srv/games/Wangan/MT5: no such folder".into())
        }));
        click_button(&mut h, "Play");
        testing::wait(&mut h, "the message", |a| a.message().is_some());
        h
    });
}

#[test]
fn page_running() {
    golden("running", |t, size| {
        let mut h = hub(t, size, &["mt5"]);
        select(&mut h, "mt5");
        let (tx, rx) = mpsc::channel::<()>();
        h.state_mut().backend_mut().fake.session = Some(Box::new(move |r: &Reporter| {
            r.playing();
            let _ = rx.recv();
            Ok(())
        }));
        click_button(&mut h, "Play");
        settle(&mut h);
        assert!(h.state().is_running());
        // the session ends with the test; the dump shows it running
        drop(tx);
        h
    });
}

// ------------------------------------------------------------ driving it

#[test]
fn the_keyboard_moves_through_the_hub_and_the_top_bar() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt5"]);
    assert_eq!(at(&h), (Page::Hub, Focus::Tile));
    key(&mut h, egui::Key::ArrowRight);
    assert_eq!(selected(&h), GAMES[1].slug);
    keys(&mut h, egui::Key::ArrowRight, 20);
    assert_eq!(selected(&h), GAMES[GAMES.len() - 1].slug, "clamped");
    select(&mut h, "mt5");
    key(&mut h, egui::Key::ArrowUp);
    assert_eq!(at(&h), (Page::Hub, Focus::Play));
    key(&mut h, egui::Key::ArrowRight);
    assert_eq!(at(&h), (Page::Hub, Focus::GameSettings));
    key(&mut h, egui::Key::ArrowUp);
    assert_eq!(at(&h), (Page::Hub, Focus::Tab(Tab::Games)));
    key(&mut h, egui::Key::ArrowRight);
    key(&mut h, egui::Key::Space);
    assert_eq!(at(&h), (Page::Controls, Focus::Tab(Tab::Controls)));
    key(&mut h, egui::Key::PageDown);
    assert_eq!(at(&h), (Page::Settings, Focus::Item(0)));
    key(&mut h, egui::Key::PageDown);
    assert_eq!(at(&h), (Page::Hub, Focus::Tile));
    key(&mut h, egui::Key::PageUp);
    assert_eq!(at(&h).0, Page::Settings);
    key(&mut h, egui::Key::Backspace);
    assert_eq!(at(&h), (Page::Hub, Focus::Tile));
    assert!(!key_closes(&mut h, egui::Key::Backspace), "B never quits");
    assert!(
        key_closes(&mut h, egui::Key::Escape),
        "Esc on the hub quits"
    );
}

#[test]
fn a_pad_drives_every_page() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt3", "wangan-r"]);
    press(&mut h, pad::DPAD_RIGHT);
    assert_eq!(selected(&h), GAMES[1].slug);
    press(&mut h, pad::DPAD_LEFT);
    assert_eq!(selected(&h), "wangan-r");
    // Wangan R waits for its board files: A opens the add-files page
    assert!(says(&h, "Add files"));
    press(&mut h, pad::A);
    assert_eq!(at(&h), (Page::Extra, Focus::Field(0)));
    press(&mut h, pad::DPAD_DOWN);
    assert_eq!(at(&h), (Page::Extra, Focus::Submit(0)));
    press(&mut h, pad::B);
    assert_eq!(at(&h), (Page::Hub, Focus::Play));
    press(&mut h, pad::DPAD_UP);
    assert_eq!(at(&h), (Page::Hub, Focus::Tab(Tab::Games)));
    press(&mut h, pad::DPAD_RIGHT);
    press(&mut h, pad::A);
    assert_eq!(at(&h), (Page::Controls, Focus::Tab(Tab::Controls)));
    press(&mut h, pad::RIGHT_SHOULDER);
    assert_eq!(at(&h), (Page::Settings, Focus::Item(0)));
    press(&mut h, pad::LEFT_SHOULDER);
    assert_eq!(at(&h).0, Page::Controls);
    press(&mut h, pad::B);
    assert_eq!(at(&h), (Page::Hub, Focus::Tile));
    // Y (the View button) opens the selected game's settings
    press(&mut h, pad::Y);
    assert_eq!(at(&h), (Page::Settings, Focus::Item(0)));
    // A on a settings row moves its value on: the dead zone, on everywhere
    let rows = settings::OPTIONS.len();
    for _ in 0..rows - 1 {
        press(&mut h, pad::DPAD_DOWN);
    }
    assert_eq!(row(&h, "Dead zone"), "5%");
    press(&mut h, pad::A);
    assert_eq!(row(&h, "Dead zone"), "6%");
    assert!(!pad_closes(&mut h, pad::START));
    assert_eq!(at(&h).0, Page::Hub, "Start goes back to the hub");
    pad(&mut h, 0);
    assert!(pad_closes(&mut h, pad::START), "Start on the hub quits");
}

#[test]
fn a_held_direction_repeats_and_a_running_game_keeps_the_pad() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt5"]);
    pad(&mut h, pad::DPAD_RIGHT);
    assert_eq!(h.state().model().selected, 1, "held: one step at first");
    std::thread::sleep(Duration::from_millis(450));
    pad(&mut h, pad::DPAD_RIGHT);
    assert_eq!(h.state().model().selected, 2, "then it repeats");
    pad(&mut h, 0);
    select(&mut h, "mt5");
    let (tx, rx) = mpsc::channel::<()>();
    h.state_mut().backend_mut().fake.session = Some(Box::new(move |r: &Reporter| {
        r.playing();
        let _ = rx.recv();
        Ok(())
    }));
    click_button(&mut h, "Play");
    settle(&mut h);
    press(&mut h, pad::DPAD_RIGHT);
    assert_eq!(selected(&h), "mt5", "the game's presses never move the hub");
    tx.send(()).unwrap();
    testing::wait(&mut h, "the game's end", |a| !a.is_running());
}

#[test]
fn the_mouse_clicks_tabs_tiles_buttons_and_arrows() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt3", "mt5"]);
    open_tab(&mut h, Tab::Controls);
    assert_eq!(at(&h).0, Page::Controls);
    open_tab(&mut h, Tab::Games);
    assert_eq!(at(&h).0, Page::Hub);
    select(&mut h, "mt3");
    click_button(&mut h, "Settings");
    assert_eq!(at(&h), (Page::Settings, Focus::Item(0)));
    click_button(&mut h, "Dead zone: next");
    let (s, _) = Settings::parse(&settings_file(t.path(), "mt3").unwrap());
    assert!((s.deadzone - 0.06).abs() < 1e-6, "{s:?}");
    assert!(says(
        &h,
        "Saved. The game uses it the next time you press Play."
    ));
    click_button(&mut h, "Game folders");
    assert_eq!(at(&h).0, Page::Setup);
    assert!(says(&h, &display_path(Path::new(LIBRARY))));
}

#[test]
fn a_double_click_counts_on_one_tile_only() {
    let t = tempfile::tempdir().unwrap();
    let paths = paths_in(t.path());
    std::fs::create_dir_all(&paths.data).unwrap();
    std::fs::write(paths.config_file(), format!("library = {LIBRARY}\n")).unwrap();
    let mut backend = Wangan::new(paths.clone(), None);
    let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = std::sync::Arc::clone(&started);
    backend.fake.session = Some(Box::new(move |_: &Reporter| {
        seen.store(true, Ordering::SeqCst);
        Ok(())
    }));
    // the real frame rate: a double click is two clicks within 0.4 s
    let mut h: Hub = Harness::builder()
        .with_size(SIZE_720)
        .with_step_dt(1.0 / 60.0)
        .with_theme(egui::Theme::Dark)
        .with_context_setup(|ctx| hub::install(ctx, &THEME))
        .build_ui_state(
            |ui, app: &mut HubApp<Wangan>| app.frame(ui),
            HubApp::new(spec(paths), backend),
        );
    testing::wait(&mut h, "the scan", |a| !a.scanning());
    h.state_mut().set_library(library(&["mt3", "mt5"]));
    h.run_steps(3);
    let mt5 = catalog::by_slug("mt5").unwrap().title;
    // a tab, then the tile twice at once: the tab's click is not the tile's
    h.get_by_role_and_label(Role::Tab, "Games").click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::RadioButton, mt5).click();
    h.run_steps(1);
    h.get_by_role_and_label(Role::RadioButton, mt5).click();
    h.run_steps(2);
    testing::wait(&mut h, "the session", |_| started.load(Ordering::SeqCst));
}

/// Each option the window changes is in the game's settings file as the
/// command line writes it.
#[test]
fn every_option_is_saved_like_the_command_line_saves_it() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt3"]);
    select(&mut h, "mt3");
    open_tab(&mut h, Tab::Settings);
    let window = paths_in(t.path()).settings("mt3");
    let mut given: Vec<(&str, String)> = Vec::new();
    for spec in settings::OPTIONS {
        if row_reason(&h, spec.label).is_some() {
            continue;
        }
        // the default output is the largest: previous is the change
        let way = if spec.key == "output" {
            "previous"
        } else {
            "next"
        };
        click_button(&mut h, &format!("{}: {way}", spec.label));
        let (s, _) = Settings::parse(&std::fs::read_to_string(&window).unwrap());
        given.push((spec.key, s.value(spec.key)));
    }
    let (s, _) = Settings::parse(&std::fs::read_to_string(&window).unwrap());
    // a hand change after the look ended it: the command line gives no look
    given.retain(|(k, v)| *k != "look" || s.look.is_some() && !v.is_empty());
    let mut flags = vec!["--set".to_string(), "mt3".into()];
    for (k, v) in given {
        flags.push(format!("--{k}"));
        flags.push(v);
    }
    let cli = tempfile::tempdir().unwrap();
    let cli_paths = paths_in(cli.path());
    let inv = cli::parse(&crate::CLI, flags.clone()).unwrap();
    crate::set(&cli_paths, catalog::by_slug("mt3").unwrap(), &inv).unwrap();
    let (by_cli, _) = Settings::parse(&std::fs::read_to_string(cli_paths.settings("mt3")).unwrap());
    for pair in flags[2..].chunks(2) {
        let key = pair[0].trim_start_matches("--");
        assert_eq!(by_cli.value(key), s.value(key), "{key} of {flags:?}");
    }
}

#[test]
fn rows_a_game_ignores_are_off_and_say_their_whole_reason() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt5"]);
    select(&mut h, "mt5");
    open_tab(&mut h, Tab::Settings);
    assert_eq!(
        row_reason(&h, "Game render size").as_deref(),
        Some(form::RENDER_FIXED)
    );
    for label in ["Output size", "Upscaler", "Fullscreen", "Look"] {
        let reason = row_reason(&h, label);
        assert_eq!(
            reason.as_deref(),
            cfg!(windows).then_some(form::NO_GAMESCOPE),
            "{label}: the whole reason, never cut"
        );
    }
}

#[test]
fn every_control_has_an_accessible_name() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt3", "wangan-r"]);
    let check = |h: &Hub, page: &str| {
        for n in h.root().children_recursive() {
            let a = n.accesskit_node();
            let named = a.label().is_some_and(|l| !l.is_empty());
            match a.role() {
                Role::Button | Role::RadioButton | Role::Tab | Role::TextInput => {
                    assert!(named, "{page}: a {:?} without a name", a.role())
                }
                Role::Unknown => panic!("{page}: an unnamed control"),
                _ => {}
            }
        }
    };
    check(&h, "hub");
    open_tab(&mut h, Tab::Controls);
    check(&h, "controls");
    open_tab(&mut h, Tab::Settings);
    check(&h, "settings");
    open_tab(&mut h, Tab::Games);
    select(&mut h, "wangan-r");
    click_button(&mut h, "Add files");
    check(&h, "add files");
}

#[test]
fn every_control_fits_the_window_on_every_page() {
    for size in [SIZE_720, SIZE_800] {
        let t = tempfile::tempdir().unwrap();
        let mut h = hub(t.path(), size, &["mt3", "wangan-r"]);
        let check = |h: &mut Hub, page: &str| {
            h.run_steps(3);
            let misfits = testing::misfits(h);
            assert!(misfits.is_empty(), "{page} at {size:?}: {misfits:#?}");
        };
        check(&mut h, "hub");
        open_tab(&mut h, Tab::Controls);
        check(&mut h, "controls");
        open_tab(&mut h, Tab::Settings);
        check(&mut h, "settings");
        click_button(&mut h, "Game folders");
        check(&mut h, "setup");
    }
}

#[test]
fn a_game_seen_for_the_first_time_gets_the_recommended_look_saved() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt3"]);
    h.state_mut().set_screen(Some((1280, 800)));
    select(&mut h, "mt3");
    open_tab(&mut h, Tab::Settings);
    assert_eq!(row(&h, "Look"), "Steam Deck");
    let (s, _) = Settings::parse(&settings_file(t.path(), "mt3").unwrap());
    assert_eq!(s.look, Some(settings::Look::SteamDeck));
}

#[test]
fn the_setup_page_takes_a_typed_folder() {
    let t = tempfile::tempdir().unwrap();
    let mut h = window(t.path(), None, SIZE_720);
    assert_eq!(at(&h), (Page::Setup, Focus::Field(0)));
    key(&mut h, egui::Key::Enter);
    assert!(says(&h, "Type the folder's path first."));
    h.event(egui::Event::Text("/no/such/folder".into()));
    h.run_steps(2);
    key(&mut h, egui::Key::Enter);
    assert!(says(&h, "isn't a folder this computer can open"));
    clear_field(&mut h);
    let games = t.path().join("games");
    std::fs::create_dir_all(&games).unwrap();
    h.event(egui::Event::Text(games.display().to_string()));
    h.run_steps(2);
    key(&mut h, egui::Key::Enter);
    let config = paths::config(&paths_in(t.path())).unwrap();
    assert_eq!(config.libraries, [games]);
    click_button(&mut h, "Done");
    assert_eq!(at(&h), (Page::Hub, Focus::Tile));
    testing::wait(&mut h, "the scan", |a| !a.scanning());
    assert!(says(&h, &format!("0 of {} found", GAMES.len())));
}

#[test]
fn the_setup_page_without_a_library_quits_on_escape() {
    let t = tempfile::tempdir().unwrap();
    let mut h = window(t.path(), None, SIZE_720);
    key(&mut h, egui::Key::PageDown);
    assert_eq!(at(&h).0, Page::Setup, "no top bar yet");
    assert!(key_closes(&mut h, egui::Key::Escape));
}

#[test]
fn add_files_searches_the_typed_folder_once_per_press() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["wangan-r"]);
    let bios = t.path().join("bios");
    std::fs::create_dir_all(&bios).unwrap();
    std::fs::write(bios.join("r27v1602f.7d"), vec![0u8; 2_097_152]).unwrap();
    click_button(&mut h, "Add files");
    assert_eq!(at(&h), (Page::Extra, Focus::Field(0)));
    type_in_extra(&mut h, &bios.display().to_string());
    key(&mut h, egui::Key::Enter);
    assert!(says(&h, "None of the missing files were found in"));
    assert!(says(
        &h,
        "(looked at 1 file of the right size). Choose the folder or zip that has them."
    ));
    assert!(says(&h, "r27v1602f.7d  ·  2 MB: Missing"));
    click_button(&mut h, "Back");
    assert_eq!(at(&h), (Page::Hub, Focus::Play));
    assert!(says(&h, "Needs board files"));
}

#[test]
fn a_game_whose_board_files_are_in_but_whose_emulator_is_pending_is_locked() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt1"]);
    h.state_mut().backend_mut().files_ok[index("mt1")] = true;
    select(&mut h, "mt1");
    assert!(says(&h, catalog::EMULATOR_PENDING));
    assert!(says(&h, "Not supported yet"));
}

#[test]
fn a_game_that_cannot_start_says_why_and_any_press_dismisses_it() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt5"]);
    select(&mut h, "mt5");
    key(&mut h, egui::Key::Enter);
    testing::wait(&mut h, "the error", |a| a.message().is_some());
    let m = h.state().message().unwrap();
    assert_eq!(m.title, "Wangan Midnight Maximum Tune 5 couldn't start");
    assert!(m.body.contains("components"), "{}", m.body);
    let log = std::fs::read_to_string(paths_in(t.path()).log()).unwrap();
    assert!(
        log.contains("components"),
        "every error is in launcher.log: {log}"
    );
    key(&mut h, egui::Key::ArrowRight);
    assert!(
        h.state().message().is_some(),
        "only A, B, Esc or Start dismiss it"
    );
    press(&mut h, pad::A);
    assert!(h.state().message().is_none());
    assert!(!closed(&h));
}

#[test]
fn closing_the_window_while_a_game_runs_stops_the_game_first() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path(), SIZE_720, &["mt5"]);
    select(&mut h, "mt5");
    let (told, stopped) = mpsc::channel();
    h.state_mut().backend_mut().fake.session = Some(Box::new(move |r: &Reporter| {
        r.playing();
        let stop = r.stop_flag();
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = told.send(());
        Ok(())
    }));
    click_button(&mut h, "Play");
    settle(&mut h);
    testing::request_close(&mut h);
    h.run_steps(1);
    assert!(
        testing::commands(&h)
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::CancelClose)),
        "the window stays until the game has stopped"
    );
    stopped.recv_timeout(Duration::from_secs(5)).unwrap();
    testing::wait(&mut h, "the close", |a| !a.is_running());
}

#[test]
fn rescan_reads_the_folders_again() {
    let t = tempfile::tempdir().unwrap();
    let games = t.path().join("games");
    std::fs::create_dir_all(games.join("MT6 [TP]")).unwrap();
    let mut h = window(
        t.path(),
        Some(&format!("library = {}\n", games.display())),
        SIZE_720,
    );
    open_tab(&mut h, Tab::Settings);
    assert!(says(&h, &format!("0 of {} games found", GAMES.len())));
    std::fs::write(games.join("MT6 [TP]/wmn6r.exe"), b"not the game").unwrap();
    click_button(&mut h, "Rescan");
    testing::wait(&mut h, "the scan", |a| !a.scanning());
    assert!(says(&h, "doesn't know"), "an unknown build is reported");
}

#[test]
fn the_controls_page_shows_the_pad_layout_and_the_live_buttons() {
    let v = controls_view(&two_controllers(), 4);
    assert_eq!(v.lines, [("Gear".to_string(), "4".to_string())]);
    assert_eq!(v.devices[0].name, "Logitech G29 Driving Force Racing Wheel");
    assert!(
        v.layout.contains(&("View change".into(), "Y".into())),
        "{:?}",
        v.layout
    );
    assert!(v.buttons.iter().any(|b| b.key == "intrude" && b.pressed));
    assert!(v.buttons.iter().any(|b| b.key == "view" && !b.pressed));
}

#[test]
fn the_window_is_not_named_like_a_game_and_its_colours_are_readable() {
    assert!(GAMES.iter().all(|g| g.title != THEME.title));
    let problems = THEME.contrast_problems();
    assert!(problems.is_empty(), "{problems:#?}");
}
