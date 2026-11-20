//! The launcher window: the SDK's hub (`rea_launcher_ui::hub`) for the nine
//! games, after Azimuth's Forza hub, dressed as the Shuto expressway at
//! night. This file is what the hub needs to know about the Wangan games
//! ([`Wangan`], a `HubBackend`): the table, the scan, each game's settings
//! ([`form::Form`]), its session, the controllers, and the board files a
//! game still needs (the hub's extra page).

pub(crate) mod form;

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::Color32;
use rea_launcher::catalog::{Host, Support};
use rea_launcher::cfg::OptionSpec;
use rea_launcher::hub::bindings::Bindings;
use rea_launcher::hub::{HubConfig, HubPaths, Input, Need, display_path, expand_home};
use rea_launcher::library::{Found as Known, HashCache, Scan};
use rea_launcher::log::Log;
use rea_launcher::pad::PadNav;
use rea_launcher::wheel::{Control, PadInput};
use rea_launcher_native::feeder::{Command, InputService, Snapshot};
use rea_launcher_ui::hub::{
    self, ButtonView, Check, ControlsView, Device, ExtraPage, Found, HubBackend, HubGame,
    HubLayout, HubSpec, HubTheme, HubWords, Mark, Note, Reporter, Scanner, Session,
};
use wangan_core::catalog::{self, GAMES, Game};
use wangan_core::firmware::{self, FileState, FirmwareSet};
use wangan_core::input::{Button, Translator};
use wangan_core::settings::Settings;

use crate::paths;
use crate::session::{self, Display};

use form::Form;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The Shuto expressway at night: asphalt, sodium lamps and the
/// green-and-white of Japanese expressway signs.
pub const THEME: HubTheme = HubTheme {
    base: rgb(0x0E1621),
    panel: rgb(0x16212E),
    line: rgb(0x6B7A8C),
    lamp: rgb(0xF2A33A),
    lamp_hover: rgb(0xF7B85E),
    sign: rgb(0x0F6B4A),
    ink: rgb(0xF4F7F2),
    muted: rgb(0x8A97A6),
    signal: rgb(0xB3261E),
    error: rgb(0xFF8A80),
    disabled: rgb(0x5E6B7A),
    focus_fill: rgb(0x1F2C3B),
    scrim: 240,
    brand: "B",
    title: crate::WINDOW_TITLE,
    app_id: "rea-wangan",
};

/// The words that differ from the other hubs.
const WORDS: HubWords = HubWords {
    setup_intro: "Type the path of a folder that holds your Wangan Midnight game folders and \
         add it. The launcher looks inside for each game and never changes what's there.",
    folders_button: "Game folders",
    not_here: form::NO_GAMESCOPE,
};

/// What the hub is, besides its backend.
pub fn spec(paths: HubPaths) -> HubSpec {
    HubSpec {
        theme: THEME,
        // nine tiles share the width; seven rows fit at 58 points
        layout: HubLayout::DEFAULT,
        words: WORDS,
        fields: paths::FIELDS,
        paths,
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Opens the window.
pub fn run(paths: HubPaths) -> Result<(), String> {
    let log = Log::new(paths.log());
    let bindings = crate::input::bindings(&paths, |w| log.warn(w));
    let service = crate::input::service(&bindings);
    let mut backend = Wangan::new(paths.clone(), Some(service));
    backend.bindings = bindings;
    hub::run(spec(paths), backend)
}

/// A game of the table as the hub shows it.
const fn hub_game(g: &Game) -> HubGame {
    HubGame {
        slug: g.slug,
        title: g.title,
        short: g.short,
        board: g.board,
        year: g.year,
    }
}

static HUB_GAMES: [HubGame; GAMES.len()] = {
    let mut v = [hub_game(&GAMES[0]); GAMES.len()];
    let mut i = 1;
    while i < v.len() {
        v[i] = hub_game(&GAMES[i]);
        i += 1;
    }
    v
};

/// The Wangan games, for the hub.
pub struct Wangan {
    paths: HubPaths,
    log: Log,
    /// Each game's support on this computer (the tests set others).
    supports: Vec<Support>,
    /// Per game: its board files are in (or it needs none). Hashed when
    /// the window opens and after each search, never per frame.
    files_ok: Vec<bool>,
    /// Shared by every scan: a rescan during a scan reuses its digests.
    cache: HashCache,
    /// Owns SDL; `None` in the headless tests.
    input: Option<InputService>,
    /// The pads' menu buttons held at the last frame (a held direction
    /// repeats, as in 0.2.0).
    pad: PadNav,
    /// The cabinet's gear, for the Controls page.
    shifter: RefCell<Translator>,
    /// The player's pad layout (`controls.cfg`).
    bindings: Bindings<Button>,
    /// The tests' controllers, scan and next session.
    #[cfg(test)]
    fake: tests::Fake,
}

impl Wangan {
    pub fn new(paths: HubPaths, input: Option<InputService>) -> Wangan {
        let mut w = Wangan {
            log: Log::new(paths.log()),
            supports: GAMES
                .iter()
                .map(|g| g.support_on(Host::current()))
                .collect(),
            files_ok: vec![true; GAMES.len()],
            cache: HashCache::default(),
            input,
            pad: PadNav::with_repeat(Duration::from_millis(400), Duration::from_millis(120)),
            shifter: RefCell::default(),
            bindings: Bindings::new(wangan_core::input::default_pad_map()),
            #[cfg(test)]
            fake: tests::Fake::default(),
            paths,
        };
        w.check_files();
        w
    }

    /// Which games have their board files.
    fn check_files(&mut self) {
        for (i, g) in GAMES.iter().enumerate() {
            self.files_ok[i] = g
                .firmware
                .is_none_or(|set| set.complete(&set.dir(&self.paths.data)));
        }
    }

    /// The board files game `i` still waits for.
    fn missing_files(&self, i: usize) -> Option<&'static FirmwareSet> {
        GAMES[i].firmware.filter(|_| !self.files_ok[i])
    }

    /// Game `i`'s settings ([`crate::load_settings`]); the lines its file
    /// couldn't read go to `launcher.log`.
    fn settings(&self, i: usize, screen: Option<(u32, u32)>) -> Result<Settings, String> {
        crate::load_settings(&self.paths, &GAMES[i], screen, |w| self.log.warn(w))
    }

    /// Hands the changed pad layout to the input thread; what the
    /// Controls page says (a save that failed also goes to `launcher.log`).
    fn bindings_changed(&self, done: Result<String, String>) -> Note {
        if let Some(i) = &self.input {
            i.set_pad_map(self.bindings.map().clone());
        }
        match done {
            Ok(said) => Note::ok(said),
            Err(e) => {
                self.log.error(&e);
                Note::error(e)
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        #[cfg(test)]
        if let Some(s) = &self.fake.snapshot {
            return s.clone();
        }
        self.input
            .as_ref()
            .map(InputService::snapshot)
            .unwrap_or_default()
    }

    /// The session of game `i`, found in `library`, to run on the session thread.
    fn real_session(
        &self,
        i: usize,
        library: &Scan<Game>,
        config: &HubConfig,
        screen: Option<(u32, u32)>,
    ) -> Result<Session, String> {
        let game = &GAMES[i];
        let entry = library
            .by_game()
            .get(&game.id)
            .map(|e| (*e).clone())
            .ok_or_else(|| format!("{} is not in your game folders", game.title))?;
        let settings = self.settings(i, screen)?;
        let paths = self.paths.clone();
        let libraries = config.libraries.clone();
        let input = match &self.input {
            Some(service) => session::Input::Devices {
                tx: service.sender(),
                deadzone: settings.deadzone,
            },
            None => session::Input::None,
        };
        Ok(Box::new(move |reporter: &Reporter| {
            let prepared =
                session::prepare(&paths, game, &entry, &settings).map_err(|e| format!("{e:#}"))?;
            reporter.playing();
            let stop = reporter.stop_flag();
            session::run(
                &paths,
                game,
                &prepared,
                Display::Screen,
                &libraries,
                &input,
                &stop,
            )
            .map(|_| ())
            .map_err(|e| format!("{e:#}"))
        }))
    }
}

/// A button's name on the live check (the full one is in the pad layout).
fn short_label(b: Button) -> &'static str {
    match b {
        Button::GearUp => "Shift up",
        Button::GearDown => "Shift down",
        Button::View => "View change",
        Button::Intrude => "Intrude",
        Button::Card => "Insert card",
        Button::Test => "Test",
        Button::Service => "Service",
        Button::MenuUp => "Menu up",
        Button::MenuDown => "Menu down",
        Button::Enter => "Enter",
        Button::Coin => "Insert coin",
    }
}

/// What the Controls page shows of the input thread's `snap`, with the
/// cabinet's current `gear` and the player's pad layout.
fn controls_view(snap: &Snapshot, gear: u8, bindings: &Bindings<Button>) -> ControlsView {
    let c = snap.controls;
    ControlsView {
        error: snap.error.clone(),
        devices: snap
            .devices
            .iter()
            .map(|d| Device {
                name: d.name.clone(),
                kind: d.kind,
            })
            .collect(),
        driver: snap.driver,
        layout: bindings.layout(),
        layout_note: "A wheel steers with its wheel and pedals, and its paddles shift.".into(),
        steer: c.steer,
        accel: c.accel,
        brake: c.brake,
        buttons: Button::ALL
            .iter()
            .map(|b| ButtonView {
                key: b.key(),
                label: short_label(*b).to_string(),
                pressed: c.buttons & b.bit() != 0,
            })
            .collect(),
        feeding: snap.feeding,
        lines: vec![("Gear".into(), gear.to_string())],
    }
}

/// A size as the checklist says it.
fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{} MB", b >> 20),
        b if b >= 1 << 10 => format!("{} KB", b >> 10),
        b => format!("{b} bytes"),
    }
}

/// What a search for the board files found, as the player reads it.
fn search_note(set: &FirmwareSet, src: &std::path::Path, r: &firmware::Report) -> Note {
    if r.missing.is_empty() {
        return Note::ok(format!("All {} files are in.", set.files.len()));
    }
    if r.added.is_empty() {
        return Note::error(format!(
            "None of the missing files were found in {} (looked at {} file{} of the right \
             size). Choose the folder or zip that has them.",
            display_path(src),
            r.looked_at,
            if r.looked_at == 1 { "" } else { "s" }
        ));
    }
    Note::error(format!(
        "Added {}. Still missing: {}.",
        r.added.join(", "),
        r.missing.join(", ")
    ))
}

impl HubBackend for Wangan {
    type Library = Scan<Game>;
    type Form = Form;

    fn games(&self) -> &[HubGame] {
        &HUB_GAMES
    }

    fn options(&self) -> &'static [OptionSpec] {
        wangan_core::settings::OPTIONS
    }

    /// A game waiting for its board files offers to add them first,
    /// whatever else keeps it from starting (Maximum Tune's emulator, the
    /// Windows build): the files come before the rest, as in 0.2.0.
    fn support(&self, game: usize) -> Support {
        match self.missing_files(game) {
            Some(_) => Support::Ready,
            None => self.supports[game],
        }
    }

    fn need(&self, game: usize) -> Option<Need> {
        self.missing_files(game).map(|set| Need {
            chip: "Needs board files",
            badge: "Add files",
            action: "Add files",
            reason: format!("Also needs the {}.", set.title),
        })
    }

    fn blurb(&self, game: usize) -> Option<&str> {
        Some(GAMES[game].blurb)
    }

    fn scanner(&self) -> Scanner<Scan<Game>> {
        #[cfg(test)]
        if let Some(s) = self.fake.scanner.borrow_mut().take() {
            return s;
        }
        let library = catalog::library(self.cache.clone());
        Box::new(move |roots: &[PathBuf]| library.scan(roots))
    }

    fn found(library: &Scan<Game>, game: usize) -> Option<Found> {
        let e = *library.by_game().get(&GAMES[game].id)?;
        let edition = match &e.found {
            Known::Known { edition, .. } => Some(edition.label.to_string()),
            Known::Unknown { .. } => None,
        };
        Some(Found {
            dir: e.dir.clone(),
            edition,
        })
    }

    fn unknown(library: &Scan<Game>) -> Vec<String> {
        let unreadable = library
            .problems
            .iter()
            .map(|p| format!("couldn't read {p}"));
        let unknown = library.entries.iter().filter_map(|e| match &e.found {
            Known::Unknown { file, digest } => Some(format!(
                "unknown build {file} sha256 {digest} in {}",
                display_path(&e.dir)
            )),
            Known::Known { .. } => None,
        });
        unreadable.chain(unknown).collect()
    }

    fn form(&mut self, game: usize, screen: Option<(u32, u32)>) -> Form {
        let settings = self.settings(game, screen).unwrap_or_else(|e| {
            self.log.error(&e);
            Settings::default()
        });
        Form::new(settings, &GAMES[game], screen)
    }

    fn session(
        &mut self,
        game: usize,
        library: &Scan<Game>,
        config: &HubConfig,
        screen: Option<(u32, u32)>,
    ) -> Result<Session, String> {
        #[cfg(test)]
        if let Some(s) = self.fake.session.take() {
            return Ok(s);
        }
        self.real_session(game, library, config, screen)
    }

    fn game_log(&self, game: usize) -> Option<PathBuf> {
        Some(session::session_log(&self.paths, &GAMES[game]))
    }

    fn ended(&mut self) {
        if let Some(i) = &self.input {
            let _ = i.sender().send(Command::Detach);
        }
    }

    /// The gear follows the shift presses the window sees, as the
    /// cabinet's lever would.
    fn controls(&self) -> ControlsView {
        let snap = self.snapshot();
        let gear = self.shifter.borrow_mut().step(&snap.controls).gear;
        controls_view(&snap, gear, &self.bindings)
    }

    fn pad(&mut self) -> Vec<Input> {
        let bits = self.snapshot().nav;
        Input::from_pad(&mut self.pad, bits)
    }

    fn pad_now(&self) -> Option<(u16, Option<PadInput>)> {
        let snap = self.snapshot();
        Some((snap.nav, snap.held))
    }

    fn rebindable(&self) -> Vec<usize> {
        self.bindings.rebindable()
    }

    fn rebind(&mut self, row: usize, input: PadInput) -> Note {
        let file = self.paths.controls_file();
        let done = self.bindings.rebind_saved(row, input, &file);
        self.bindings_changed(done)
    }

    fn reset_bindings(&mut self) -> Note {
        let file = self.paths.controls_file();
        let done = self.bindings.reset_saved(&file);
        self.bindings_changed(done)
    }

    fn extra(&self, game: usize) -> Option<ExtraPage> {
        let g = &GAMES[game];
        let set = g.firmware?;
        let checklist = set
            .check(&set.dir(&self.paths.data))
            .into_iter()
            .map(|(f, state)| {
                let (mark, word) = match state {
                    FileState::Present => (Mark::Good, "In"),
                    FileState::Missing => (Mark::Missing, "Missing"),
                    FileState::Wrong => (Mark::Bad, "Wrong file"),
                };
                Check {
                    mark,
                    word,
                    text: format!("{}  ·  {}", f.name, human_size(f.size)),
                }
            })
            .collect();
        let downloads = expand_home("~/Downloads");
        Some(ExtraPage {
            title: format!("Add the {}", set.title),
            intro: format!(
                "{} needs these files from the original board. Type the folder or zip \
                 where you keep them: the launcher looks inside (zips and subfolders too), \
                 keeps only exact matches whatever their names, and copies them to its own \
                 folder.",
                g.title
            ),
            checklist,
            label: "Folder or zip with the board files",
            placeholder: "/path/to/your/files",
            hint: "A folder, a zip, or ~/Downloads. Press Enter to look.",
            button: "Look for the files",
            start: if downloads.is_dir() {
                "~/Downloads".into()
            } else {
                String::new()
            },
        })
    }

    fn submit_extra(&mut self, game: usize, typed: &str) -> Note {
        let Some(set) = GAMES[game].firmware else {
            return Note::error("This game needs no board files.");
        };
        if typed.trim().is_empty() {
            return Note::error("Type the folder's path first.");
        }
        let src = expand_home(typed);
        if !src.exists() {
            return Note::error(format!(
                "{} isn't a folder or file this computer can open. Check the path and try \
                 again.",
                display_path(&src)
            ));
        }
        let dest = set.dir(&self.paths.data);
        let note = match firmware::import(set, &src, &dest) {
            Ok(r) => {
                self.log.line(&format!(
                    "board files ({}) from {}: added {:?}, missing {:?}, {} looked at",
                    set.id,
                    display_path(&src),
                    r.added,
                    r.missing,
                    r.looked_at
                ));
                search_note(set, &src, &r)
            }
            Err(e) => Note::error(format!(
                "Couldn't finish the search: {e}. Check that {} can be written.",
                display_path(&dest)
            )),
        };
        self.check_files();
        note
    }
}

#[cfg(test)]
mod tests;
