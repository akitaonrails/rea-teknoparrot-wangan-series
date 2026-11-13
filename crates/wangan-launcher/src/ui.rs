//! The launcher window: a hub for the nine games, after Azimuth's Forza
//! hub, dressed as the Shuto expressway at night. Full screen, laid out for
//! 720 lines and zoomed to the window (it runs on 4K TVs), driven by the
//! mouse, the keyboard or a pad.

mod form;
mod model;
mod theme;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Mesh, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind, TextureHandle, Vec2, pos2, vec2,
};
use rea_launcher::wheel::{Action, Control, Kind};
use wangan_core::catalog::{GAMES, Game};
use wangan_core::input::{Button, default_pad_map};
use wangan_core::library::{self, Entry, Found, HashCache};

use crate::input::{Command, InputService, Snapshot};
use crate::log;
use crate::paths::{self, Paths};
use crate::session::{self, Display, Prepared};

use form::{Form, Row};
use model::{Effect, Focus, Input, Item, Model, Page, Tab, TileState, World};
use theme::{BASE, DISABLED, ERROR, INK, LINE, MUTED, PANEL, SIGN, SIGNAL, SODIUM, alpha, size};

const TITLE: &str = "Wangan Midnight";
const MARGIN: f32 = 48.0;
const BAR_H: f32 = 56.0;

pub fn run(paths: Paths) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_app_id("rea-wangan")
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(true),
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx, &paths.fonts());
            theme::install_style(&cc.egui_ctx);
            Ok(Box::new(App::new(paths)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("the window could not open: {e}"))
}

// ---------------------------------------------------------------- state

enum Stage {
    Preparing(Receiver<Result<Prepared, String>>),
    Playing(Receiver<Result<(), String>>),
}

struct Running {
    game: &'static Game,
    stage: Stage,
}

struct Message {
    title: String,
    body: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArtKind {
    Hero,
    Tile,
}

struct App {
    paths: Paths,
    library: Option<PathBuf>,
    input: InputService,
    snap: Snapshot,
    model: Model,
    /// The last scan; `None` while looking.
    entries: Option<Vec<Entry>>,
    scan: Option<Receiver<(Vec<Entry>, HashCache)>>,
    cache: Option<HashCache>,
    form: Option<(usize, Form)>,
    /// The last save: text, and whether it worked.
    note: Option<(String, bool)>,
    heroes: Vec<Option<TextureHandle>>,
    tiles: Vec<Option<TextureHandle>>,
    art: Receiver<(usize, ArtKind, egui::ColorImage)>,
    /// The hero picture being faded from, and when the fade started.
    fade: Option<(usize, f64)>,
    running: Option<Running>,
    message: Option<Message>,
    folder: String,
    folder_error: Option<String>,
    /// The View button (Y on a pad) last frame, for "Y Settings".
    last_view: bool,
}

impl App {
    fn new(paths: Paths) -> Self {
        // WANGAN_LIBRARY wins for this run (tests); never written
        let library = std::env::var_os("WANGAN_LIBRARY")
            .map(PathBuf::from)
            .or_else(|| paths::read_library(&paths.config()));
        let art = load_art(&paths);
        let mut app = Self {
            model: Model::new(GAMES.len(), library.is_some()),
            library,
            input: InputService::start(),
            snap: Snapshot::default(),
            entries: None,
            scan: None,
            cache: Some(HashCache::default()),
            form: None,
            note: None,
            heroes: vec![None; GAMES.len()],
            tiles: vec![None; GAMES.len()],
            art,
            fade: None,
            running: None,
            message: None,
            folder: String::new(),
            folder_error: None,
            last_view: false,
            paths,
        };
        app.start_scan();
        app
    }

    fn start_scan(&mut self) {
        let Some(root) = self.library.clone() else {
            return;
        };
        let mut cache = self.cache.take().unwrap_or_default();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("scan".into())
            .spawn(move || {
                let found = library::scan(&root, &mut cache);
                let _ = tx.send((found, cache));
            })
            .expect("spawn scan thread");
        self.entries = None;
        self.scan = Some(rx);
    }

    fn poll_scan(&mut self) {
        let Some(rx) = &self.scan else { return };
        match rx.try_recv() {
            Ok((found, cache)) => {
                for e in &found {
                    if let Found::Unknown { file, sha256 } = &e.found {
                        log::line(
                            &self.paths,
                            &format!(
                                "unknown build {file} sha256 {sha256} in {}",
                                e.dir.display()
                            ),
                        );
                    }
                }
                self.entries = Some(found);
                self.cache = Some(cache);
                self.scan = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.entries = Some(vec![]);
                self.scan = None;
            }
        }
    }

    fn poll_art(&mut self, ctx: &egui::Context) {
        for (i, kind, img) in self.art.try_iter() {
            let name = format!("{}-{}", GAMES[i].slug, kind as u8);
            let tex = ctx.load_texture(name, img, egui::TextureOptions::LINEAR);
            match kind {
                ArtKind::Hero => self.heroes[i] = Some(tex),
                ArtKind::Tile => self.tiles[i] = Some(tex),
            }
        }
    }

    fn entry(&self, i: usize) -> Option<&Entry> {
        self.entries
            .as_ref()?
            .iter()
            .find(|e| e.game_id() == Some(GAMES[i].id))
    }

    fn state(&self, i: usize) -> TileState {
        TileState::of(&GAMES[i], self.entries.is_some(), self.entry(i).is_some())
    }

    fn world(&self) -> World {
        World {
            playable: self.state(self.model.selected) == TileState::Ready,
            has_library: self.library.is_some(),
        }
    }

    fn ensure_form(&mut self) {
        let i = self.model.selected;
        if self.form.as_ref().map(|(at, _)| *at) != Some(i) {
            let g = &GAMES[i];
            self.form = Some((
                i,
                Form::new(crate::load_settings(&self.paths, g.slug), g.route),
            ));
            self.note = None;
        }
    }

    // ------------------------------------------------------------ input

    fn gather(&mut self, ctx: &egui::Context) -> Vec<Input> {
        let pads: Vec<Input> = self.input.nav.try_iter().map(Input::from_nav).collect();
        let view = self.snap.controls.view;
        let y = view && !self.last_view;
        self.last_view = view;
        if self.running.is_some() {
            return vec![];
        }
        let mut out = pads;
        if y && !self.snap.writing {
            out.push(Input::Settings);
        }
        let typing = self.model.page == Page::FirstRun && self.model.focus == Focus::Field;
        ctx.input(|i| {
            let keys: &[(egui::Key, Input)] = if typing {
                &[
                    (egui::Key::Enter, Input::Press),
                    (egui::Key::Escape, Input::Escape),
                    (egui::Key::ArrowDown, Input::Down),
                ]
            } else {
                &[
                    (egui::Key::ArrowUp, Input::Up),
                    (egui::Key::ArrowDown, Input::Down),
                    (egui::Key::ArrowLeft, Input::Left),
                    (egui::Key::ArrowRight, Input::Right),
                    (egui::Key::Enter, Input::Press),
                    (egui::Key::Space, Input::Press),
                    (egui::Key::Escape, Input::Escape),
                    (egui::Key::Backspace, Input::Back),
                    (egui::Key::PageUp, Input::PreviousTab),
                    (egui::Key::PageDown, Input::NextTab),
                ]
            };
            for (k, inp) in keys {
                if i.key_pressed(*k) {
                    out.push(*inp);
                }
            }
        });
        out
    }

    fn handle(&mut self, input: Input, ctx: &egui::Context) {
        if self.message.is_some() {
            if matches!(
                input,
                Input::Press | Input::Back | Input::Escape | Input::Start
            ) {
                self.message = None;
            }
            return;
        }
        let before = self.model.selected;
        let effect = self.model.input(input, self.world());
        self.apply(effect, ctx);
        if self.model.selected != before {
            self.selected_changed(before, ctx);
        }
    }

    fn selected_changed(&mut self, from: usize, ctx: &egui::Context) {
        self.fade = Some((from, ctx.input(|i| i.time)));
        let w = self.world();
        self.model.settle(w);
    }

    fn apply(&mut self, effect: Effect, ctx: &egui::Context) {
        match effect {
            Effect::None => {}
            Effect::Play => self.play(),
            Effect::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Effect::Adjust(row, d, wrap) => self.adjust(row, d, wrap),
            Effect::Rescan => self.start_scan(),
            Effect::SubmitFolder => self.submit_folder(),
            Effect::OpenedSettings => self.ensure_form(),
            Effect::OpenedFolder => {
                self.folder = self
                    .library
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.folder_error = None;
            }
        }
    }

    fn adjust(&mut self, row: Row, d: i32, wrap: bool) {
        self.ensure_form();
        let Some((i, f)) = self.form.as_mut() else {
            return;
        };
        if !f.adjust(row, d, wrap) {
            return;
        }
        let path = self.paths.settings(GAMES[*i].slug);
        self.note = Some(match form::save(&path, &f.settings) {
            Ok(()) => (
                "Saved. The game uses it the next time you press Play.".to_string(),
                true,
            ),
            Err(e) => (
                format!(
                    "Couldn't save to {}: {e}. Check that the folder can be written.",
                    path.display()
                ),
                false,
            ),
        });
    }

    fn submit_folder(&mut self) {
        let typed = self.folder.trim();
        let dir = match typed.strip_prefix("~/") {
            Some(rest) => std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(rest),
            None => PathBuf::from(typed),
        };
        if typed.is_empty() {
            self.folder_error = Some("Type the folder's path first.".into());
            return;
        }
        if !dir.is_dir() {
            self.folder_error = Some(format!(
                "{} isn't a folder this computer can open. Check the path and try again.",
                dir.display()
            ));
            return;
        }
        if let Err(e) = paths::write_library(&self.paths.config(), &dir) {
            self.folder_error = Some(format!(
                "Couldn't save the folder ({e}). Check that {} can be written.",
                self.paths.data.display()
            ));
            return;
        }
        log::line(&self.paths, &format!("library set to {}", dir.display()));
        self.library = Some(dir);
        self.folder_error = None;
        self.model.open(Page::Hub);
        self.start_scan();
    }

    // ------------------------------------------------------------ a game

    fn play(&mut self) {
        let i = self.model.selected;
        if self.running.is_some() || self.state(i) != TileState::Ready {
            return;
        }
        let Some(entry) = self.entry(i).cloned() else {
            return;
        };
        let game = &GAMES[i];
        let settings = crate::load_settings(&self.paths, game.slug);
        let paths = self.paths.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("prepare".into())
            .spawn(move || {
                let r =
                    session::prepare(&paths, game, &entry, &settings).map_err(|e| format!("{e:#}"));
                let _ = tx.send(r);
            })
            .expect("spawn prepare thread");
        self.running = Some(Running {
            game,
            stage: Stage::Preparing(rx),
        });
    }

    fn poll_running(&mut self, ctx: &egui::Context) {
        let Some(run) = self.running.as_mut() else {
            return;
        };
        let game = run.game;
        let log_hint = format!("Details are in {}.", self.paths.log().display());
        match &run.stage {
            Stage::Preparing(rx) => match rx.try_recv() {
                Ok(Ok(prepared)) => {
                    self.input.send(Command::Attach {
                        block: prepared.block.clone(),
                        cabinet: game.cabinet,
                    });
                    let paths = self.paths.clone();
                    let (tx, rx) = mpsc::channel();
                    std::thread::Builder::new()
                        .name("session".into())
                        .spawn(move || {
                            let r = session::run(&paths, game, &prepared, Display::Screen, |_| {})
                                .map(|_| ())
                                .map_err(|e| format!("{e:#}"));
                            let _ = tx.send(r);
                        })
                        .expect("spawn session thread");
                    run.stage = Stage::Playing(rx);
                }
                Ok(Err(e)) => {
                    self.running = None;
                    self.message = Some(Message {
                        title: format!("{} couldn't start", game.title),
                        body: format!(
                            "{}.\n\nCheck that the game folder is complete, then try again. {log_hint}",
                            sentence(&e)
                        ),
                    });
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.running = None,
            },
            Stage::Playing(rx) => {
                let r = match rx.try_recv() {
                    Ok(r) => r,
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => Err("the game ended unexpectedly".into()),
                };
                self.input.send(Command::Detach);
                self.running = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                if let Err(e) = r {
                    self.message = Some(Message {
                        title: format!("{} stopped with an error", game.title),
                        body: format!(
                            "{}.\n\nTry again. If it happens again: {log_hint}",
                            sentence(&e)
                        ),
                    });
                }
            }
        }
    }
}

/// An error message as a sentence: capital first letter, no final stop
/// (the caller adds one).
fn sentence(e: &str) -> String {
    let e = e.trim().trim_end_matches('.');
    let mut c = e.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "Something went wrong".into(),
    }
}

/// Decode the pictures off the window's thread: the tiles first, then the
/// big ones.
fn load_art(paths: &Paths) -> Receiver<(usize, ArtKind, egui::ColorImage)> {
    let (tx, rx) = mpsc::channel();
    let jobs: Vec<(usize, ArtKind, PathBuf)> = (0..GAMES.len())
        .map(|i| {
            (
                i,
                ArtKind::Tile,
                paths.art(&format!("{}-tile.jpg", GAMES[i].slug)),
            )
        })
        .chain((0..GAMES.len()).map(|i| {
            (
                i,
                ArtKind::Hero,
                paths.art(&format!("{}.jpg", GAMES[i].slug)),
            )
        }))
        .collect();
    std::thread::Builder::new()
        .name("art".into())
        .spawn(move || {
            for (i, kind, path) in jobs {
                let Ok(img) = image::open(&path) else {
                    continue;
                };
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let c = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                if tx.send((i, kind, c)).is_err() {
                    return;
                }
            }
        })
        .expect("spawn art thread");
    rx
}

// ---------------------------------------------------------------- frame

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // laid out for 720 lines, zoomed to the window
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let pixels = ctx.input(|i| i.viewport_rect().height() * i.pixels_per_point);
        let want = (pixels / native / theme::DESIGN_HEIGHT).clamp(0.75, 4.0);
        if (want - ctx.zoom_factor()).abs() > 0.001 {
            ctx.set_zoom_factor(want);
        }

        self.snap = self.input.snapshot();
        self.poll_scan();
        self.poll_art(&ctx);
        self.poll_running(&ctx);
        if self.model.page == Page::Settings {
            self.ensure_form();
        }

        // typed text first, so Enter in the same frame submits all of it
        if self.model.page == Page::FirstRun
            && self.model.focus == Focus::Field
            && self.message.is_none()
            && self.running.is_none()
        {
            let events = ctx.input(|i| i.events.clone());
            if edit_text(&mut self.folder, &events) {
                self.folder_error = None;
            }
        }
        for input in self.gather(&ctx) {
            self.handle(input, &ctx);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }

        let r = ui.max_rect();
        let p = ui.painter().clone();
        p.rect_filled(r, CornerRadius::ZERO, BASE);
        self.draw_hero(&p, r, &ctx);

        let mut clicks: Vec<Click> = Vec::new();
        match self.model.page {
            Page::Hub => self.draw_hub(ui, &p, r, &mut clicks),
            Page::Settings => self.draw_settings(ui, &p, r, &mut clicks),
            Page::Controls => self.draw_controls(&p, r),
            Page::FirstRun => self.draw_first_run(ui, &p, r, &mut clicks),
        }
        if self.model.page != Page::FirstRun {
            self.draw_top_bar(ui, &p, r, &mut clicks);
        } else {
            draw_brand_bar(&p, r);
        }
        self.draw_footer(&p, r);
        if let Some(run) = &self.running {
            draw_running(&p, r, run);
        }
        if self.message.is_some() {
            self.draw_message(ui, &p, r, &mut clicks);
        }

        if self.running.is_none() {
            for c in clicks {
                self.click(c, &ctx);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

/// A mouse click, turned into the same model inputs the pad sends.
enum Click {
    Focus(Focus, Option<Input>),
    Tile(usize, bool),
    Dismiss,
}

impl App {
    fn click(&mut self, c: Click, ctx: &egui::Context) {
        if self.message.is_some() && !matches!(c, Click::Dismiss) {
            return;
        }
        match c {
            Click::Dismiss => self.message = None,
            Click::Focus(f, input) => {
                self.model.focus = f;
                if let Some(i) = input {
                    self.handle(i, ctx);
                }
            }
            Click::Tile(i, double) => {
                let before = self.model.selected;
                self.model.select(i);
                self.model.focus = Focus::Tile;
                if before != i {
                    self.selected_changed(before, ctx);
                }
                if double {
                    self.handle(Input::Press, ctx);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- drawing helpers

fn text(
    p: &egui::Painter,
    pos: Pos2,
    align: Align2,
    s: &str,
    font: FontId,
    color: Color32,
) -> Rect {
    p.text(pos, align, s, font, color)
}

/// Text wrapped to `width`, top-left at `pos`; returns its rect.
fn wrapped(
    p: &egui::Painter,
    pos: Pos2,
    s: &str,
    font: FontId,
    color: Color32,
    width: f32,
) -> Rect {
    let g = p.layout(s.to_string(), font, color, width);
    let r = Rect::from_min_size(pos, g.size());
    p.galley(pos, g, color);
    r
}

fn text_width(p: &egui::Painter, s: &str, font: FontId) -> f32 {
    p.layout_no_wrap(s.to_string(), font, INK).size().x
}

fn gradient(p: &egui::Painter, r: Rect, [tl, tr, bl, br]: [Color32; 4]) {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), tl);
    m.colored_vertex(r.right_top(), tr);
    m.colored_vertex(r.left_bottom(), bl);
    m.colored_vertex(r.right_bottom(), br);
    m.add_triangle(0, 1, 2);
    m.add_triangle(1, 3, 2);
    p.add(Shape::mesh(m));
}

fn focus_ring(p: &egui::Painter, r: Rect, radius: u8) {
    p.rect_stroke(
        r.expand(4.0),
        CornerRadius::same(radius.saturating_add(4)),
        Stroke::new(3.0, SODIUM),
        StrokeKind::Outside,
    );
}

/// The part of a picture that covers `r`, centred.
fn cover_uv(img: Vec2, r: Rect) -> Rect {
    let ia = img.x / img.y.max(1.0);
    let ra = r.width() / r.height().max(1.0);
    if ra > ia {
        let h = ia / ra;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    } else {
        let w = ra / ia;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    }
}

fn picture(p: &egui::Painter, tex: &TextureHandle, r: Rect, tint: Color32) {
    p.image(tex.id(), r, cover_uv(tex.size_vec2(), r), tint);
}

/// A route shield, as on the Shuto expressway's signs: green, white rim,
/// flat top, a point at the bottom. Returns its rect.
fn shield(p: &egui::Painter, left_bottom: Pos2, label: &str, font_size: f32) -> Rect {
    let font = theme::display_semi(font_size);
    let w = text_width(p, label, font.clone()).max(font_size * 0.8) + font_size * 1.0;
    let h = font_size * 1.75;
    let r = Rect::from_min_size(pos2(left_bottom.x, left_bottom.y - h), vec2(w, h));
    let point = h * 0.22;
    let pts = |r: Rect| {
        vec![
            r.left_top(),
            r.right_top(),
            pos2(r.right(), r.bottom() - point),
            pos2(r.center().x, r.bottom()),
            pos2(r.left(), r.bottom() - point),
        ]
    };
    p.add(Shape::convex_polygon(pts(r), INK, Stroke::NONE));
    p.add(Shape::convex_polygon(
        pts(r.shrink(2.0)),
        SIGN,
        Stroke::NONE,
    ));
    text(
        p,
        pos2(r.center().x, r.top() + (h - point) / 2.0 + 1.0),
        Align2::CENTER_CENTER,
        label,
        font,
        INK,
    );
    r
}

const ARROWS: &str = "◀ ▶";

/// A key cap and what it does, for the footer; returns the next x.
fn hint(p: &egui::Painter, x: f32, cy: f32, key: &str, what: &str) -> f32 {
    let kf = theme::semibold(size::NOTE);
    // "◀ ▶" is drawn: the fonts have no such glyphs
    let arrows = key == ARROWS;
    let kw = if arrows {
        44.0
    } else {
        text_width(p, key, kf.clone()) + 14.0
    };
    let kr = Rect::from_center_size(pos2(x + kw / 2.0, cy), vec2(kw, 22.0));
    p.rect_stroke(
        kr,
        CornerRadius::same(4),
        Stroke::new(1.5, LINE),
        StrokeKind::Inside,
    );
    if arrows {
        let half = |c: Pos2, d: f32| {
            let s = 4.5;
            vec![
                pos2(c.x + d * s, c.y),
                pos2(c.x - d * s * 0.6, c.y - s),
                pos2(c.x - d * s * 0.6, c.y + s),
            ]
        };
        let c = kr.center();
        p.add(Shape::convex_polygon(
            half(pos2(c.x - 9.0, c.y), -1.0),
            INK,
            Stroke::NONE,
        ));
        p.add(Shape::convex_polygon(
            half(pos2(c.x + 9.0, c.y), 1.0),
            INK,
            Stroke::NONE,
        ));
    } else {
        text(p, kr.center(), Align2::CENTER_CENTER, key, kf, INK);
    }
    let r = text(
        p,
        pos2(kr.right() + 8.0, cy),
        Align2::LEFT_CENTER,
        what,
        theme::body(size::NOTE + 1.0),
        INK,
    );
    r.right() + 26.0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonStyle {
    Primary,
    Outline,
}

/// A button; returns its response.
#[allow(clippy::too_many_arguments)]
fn button(
    ui: &egui::Ui,
    p: &egui::Painter,
    r: Rect,
    id: &str,
    label: &str,
    style: ButtonStyle,
    enabled: bool,
    focused: bool,
) -> egui::Response {
    let resp = ui.interact(r, Id::unique(("button", id)), Sense::click());
    let hovered = enabled && resp.hovered();
    let radius = CornerRadius::same(6);
    let font = theme::display_semi(size::BUTTON);
    match (style, enabled) {
        (ButtonStyle::Primary, true) => {
            let fill = if hovered { theme::SODIUM_HOVER } else { SODIUM };
            p.rect_filled(r, radius, fill);
            text(p, r.center(), Align2::CENTER_CENTER, label, font, BASE);
        }
        (ButtonStyle::Outline, true) => {
            p.rect_filled(r, radius, alpha(BASE, 200));
            let stroke = if hovered { INK } else { LINE };
            p.rect_stroke(r, radius, Stroke::new(2.0, stroke), StrokeKind::Inside);
            text(p, r.center(), Align2::CENTER_CENTER, label, font, INK);
        }
        (_, false) => {
            p.rect_stroke(r, radius, Stroke::new(2.0, DISABLED), StrokeKind::Inside);
            text(p, r.center(), Align2::CENTER_CENTER, label, font, DISABLED);
        }
    }
    if focused {
        focus_ring(p, r, 6);
    }
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

fn card(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius::same(8), alpha(PANEL, 245));
    p.rect_stroke(
        r,
        CornerRadius::same(8),
        Stroke::new(1.0, alpha(LINE, 90)),
        StrokeKind::Inside,
    );
}

/// A card's heading on a sign-green plate, as an expressway direction sign.
fn card_heading(p: &egui::Painter, r: Rect, title: &str) -> f32 {
    let h = 40.0;
    let plate = Rect::from_min_size(r.min, vec2(r.width(), h));
    p.rect_filled(
        plate,
        CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        SIGN,
    );
    text(
        p,
        pos2(plate.left() + 20.0, plate.center().y),
        Align2::LEFT_CENTER,
        title,
        theme::semibold(size::LABEL),
        INK,
    );
    plate.bottom()
}

fn chip(p: &egui::Painter, pos: Pos2, label: &str, fill: Option<Color32>, color: Color32) -> Rect {
    let font = theme::semibold(size::BODY);
    let w = text_width(p, label, font.clone()) + 24.0;
    let r = Rect::from_min_size(pos, vec2(w, 28.0));
    match fill {
        Some(f) => {
            p.rect_filled(r, CornerRadius::same(4), f);
        }
        None => {
            p.rect_stroke(
                r,
                CornerRadius::same(4),
                Stroke::new(1.5, LINE),
                StrokeKind::Inside,
            );
        }
    }
    text(p, r.center(), Align2::CENTER_CENTER, label, font, color);
    r
}

/// A small padlock, drawn (the tile badge's icon).
fn padlock(p: &egui::Painter, c: Pos2, s: f32, color: Color32) {
    let body = Rect::from_center_size(pos2(c.x, c.y + s * 0.2), vec2(s, s * 0.7));
    p.rect_filled(body, CornerRadius::same(2), color);
    let r = s * 0.3;
    let top = pos2(c.x, body.top());
    let pts: Vec<Pos2> = (0..=12)
        .map(|k| {
            let a = std::f32::consts::PI * (k as f32 / 12.0);
            pos2(top.x - r * a.cos(), top.y - r * a.sin() * 1.1)
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(s * 0.14, color)));
}

fn draw_brand_bar(p: &egui::Painter, r: Rect) -> Rect {
    let bar = Rect::from_min_size(r.min, vec2(r.width(), BAR_H));
    p.rect_filled(bar, CornerRadius::ZERO, SIGN);
    p.rect_filled(
        Rect::from_min_size(pos2(bar.left(), bar.bottom() - 2.0), vec2(bar.width(), 2.0)),
        CornerRadius::ZERO,
        alpha(INK, 70),
    );
    let s = shield(
        p,
        pos2(bar.left() + MARGIN, bar.center().y + 14.0),
        "B",
        size::SHIELD,
    );
    text(
        p,
        pos2(s.right() + 12.0, bar.center().y),
        Align2::LEFT_CENTER,
        "Wangan Midnight",
        theme::display_semi(size::BRAND),
        INK,
    );
    bar
}

fn draw_running(p: &egui::Painter, r: Rect, run: &Running) {
    p.rect_filled(r, CornerRadius::ZERO, alpha(BASE, 248));
    let c = r.center();
    let (head, body) = match run.stage {
        Stage::Preparing(_) => (
            format!("Getting {} ready…", run.game.title),
            "This takes a few seconds the first time.".to_string(),
        ),
        Stage::Playing(_) => (
            format!("{} is running", run.game.title),
            "Close the game window or press Esc in it to come back.".to_string(),
        ),
    };
    shield(p, pos2(c.x - 30.0, c.y - 50.0), run.game.short, 20.0);
    text(
        p,
        pos2(c.x, c.y),
        Align2::CENTER_CENTER,
        &head,
        theme::display(size::PAGE_TITLE),
        INK,
    );
    text(
        p,
        pos2(c.x, c.y + 44.0),
        Align2::CENTER_CENTER,
        &body,
        theme::body(size::LABEL),
        MUTED,
    );
}

// ---------------------------------------------------------------- pages

impl App {
    fn draw_hero(&mut self, p: &egui::Painter, r: Rect, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let sel = self.model.selected;
        let t = match self.fade {
            Some((_, t0)) => ((now - t0) / 0.3).clamp(0.0, 1.0) as f32,
            None => 1.0,
        };
        if t >= 1.0 {
            self.fade = None;
        }
        if let Some((from, _)) = self.fade
            && let Some(tex) = &self.heroes[from]
        {
            picture(p, tex, r, Color32::WHITE);
        }
        if let Some(tex) = &self.heroes[sel] {
            picture(p, tex, r, Color32::WHITE.gamma_multiply(t));
        }
        let w = r.width();
        match self.model.page {
            Page::Hub => {
                // a solid backing behind the text column, fading to the picture
                let solid = alpha(BASE, theme::SCRIM);
                let clear = alpha(BASE, 0);
                let col = Rect::from_min_max(r.min, pos2(r.left() + w * 0.40, r.bottom()));
                p.rect_filled(col, CornerRadius::ZERO, solid);
                gradient(
                    p,
                    Rect::from_min_max(
                        pos2(col.right(), r.top()),
                        pos2(r.left() + w * 0.78, r.bottom()),
                    ),
                    [solid, clear, solid, clear],
                );
                let fade_top = r.top() + 470.0;
                let full = r.top() + 520.0;
                gradient(
                    p,
                    Rect::from_min_max(pos2(r.left(), fade_top), pos2(r.right(), full)),
                    [clear, clear, solid, solid],
                );
                p.rect_filled(
                    Rect::from_min_max(pos2(r.left(), full), r.max),
                    CornerRadius::ZERO,
                    solid,
                );
            }
            _ => {
                p.rect_filled(r, CornerRadius::ZERO, alpha(BASE, 225));
            }
        }
        if self.running.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn draw_top_bar(&mut self, ui: &egui::Ui, p: &egui::Painter, r: Rect, clicks: &mut Vec<Click>) {
        let bar = draw_brand_bar(p, r);
        let current = match self.model.page {
            Page::Controls => Tab::Controls,
            Page::Settings => Tab::Settings,
            _ => Tab::Games,
        };
        let mut x = bar.left() + 330.0;
        for tab in Tab::ALL {
            let font = theme::semibold(size::NAV);
            let tw = text_width(p, tab.label(), font.clone());
            let tr =
                Rect::from_center_size(pos2(x + tw / 2.0, bar.center().y), vec2(tw + 24.0, 38.0));
            let resp = ui
                .interact(tr, Id::unique(("tab", tab.label())), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            if resp.hovered() {
                p.rect_filled(tr, CornerRadius::same(4), alpha(BASE, 60));
            }
            text(
                p,
                tr.center(),
                Align2::CENTER_CENTER,
                tab.label(),
                font,
                INK,
            );
            if tab == current {
                p.rect_filled(
                    Rect::from_min_size(
                        pos2(tr.left() + 8.0, bar.bottom() - 6.0),
                        vec2(tr.width() - 16.0, 4.0),
                    ),
                    CornerRadius::same(1),
                    SODIUM,
                );
            }
            if self.model.focus == Focus::Tab(tab) {
                p.rect_stroke(
                    tr,
                    CornerRadius::same(4),
                    Stroke::new(3.0, SODIUM),
                    StrokeKind::Outside,
                );
            }
            if resp.clicked() {
                clicks.push(Click::Focus(Focus::Tab(tab), Some(Input::Press)));
            }
            x = tr.right() + 20.0;
        }
        // the controller that drives the car
        let (label, kind) = match (&self.snap.error, self.snap.driver) {
            (Some(_), _) => ("Controllers unavailable".to_string(), None),
            (None, Some(d)) => self
                .snap
                .devices
                .get(d)
                .map(|d| (d.name.clone(), Some(d.kind)))
                .unwrap_or(("No controller".into(), None)),
            (None, None) => ("No controller".into(), None),
        };
        let font = theme::body(size::BODY);
        let maxw = 300.0;
        let label = ellipsize(p, &label, font.clone(), maxw);
        let tr = text(
            p,
            pos2(bar.right() - MARGIN, bar.center().y),
            Align2::RIGHT_CENTER,
            &label,
            font,
            INK,
        );
        draw_device_glyph(p, pos2(tr.left() - 20.0, bar.center().y), kind);
    }

    fn draw_hub(&mut self, ui: &egui::Ui, p: &egui::Painter, r: Rect, clicks: &mut Vec<Click>) {
        let sel = self.model.selected;
        let g = &GAMES[sel];
        let state = self.state(sel);
        let x = r.left() + MARGIN;
        let colw = (r.width() * 0.40 - MARGIN).max(360.0);

        // title, anchored at its bottom
        let title_g = p.layout(
            g.title.to_string(),
            theme::display(size::TITLE),
            INK,
            colw + 80.0,
        );
        let th = title_g.size().y;
        let title_bottom = r.top() + 330.0;
        p.galley(pos2(x, title_bottom - th), title_g, INK);

        let mut line2 = format!("{} · {}", g.board, g.year);
        if let Some(Found::Known { edition, .. }) = self.entry(sel).map(|e| &e.found) {
            line2.push_str(&format!(" · {}", edition.label));
        }
        text(
            p,
            pos2(x + 2.0, title_bottom + 6.0),
            Align2::LEFT_TOP,
            &line2,
            theme::body(size::LABEL),
            MUTED,
        );

        let chip_y = r.top() + 372.0;
        let (fill, color) = match state {
            TileState::Ready => (Some(SIGN), INK),
            TileState::Blocked(_) => (Some(SIGNAL), INK),
            TileState::NotInLibrary => (None, INK),
            TileState::Looking => (None, MUTED),
        };
        chip(p, pos2(x, chip_y), state.chip(), fill, color);
        let reason = match state {
            TileState::Blocked(why) => why.to_string(),
            TileState::NotInLibrary => {
                "Put its folder in your games folder, then choose Rescan in Settings.".into()
            }
            TileState::Ready => self
                .entry(sel)
                .and_then(|e| e.dir.file_name())
                .map(|n| format!("Found in {}", n.to_string_lossy()))
                .unwrap_or_default(),
            TileState::Looking => String::new(),
        };
        if state == TileState::Ready {
            ellipsized_text(
                p,
                pos2(x, chip_y + 38.0),
                &reason,
                theme::body(size::BODY),
                MUTED,
                colw + 40.0,
            );
        } else {
            wrapped(
                p,
                pos2(x, chip_y + 38.0),
                &reason,
                theme::body(size::BODY),
                INK,
                colw + 40.0,
            );
        }

        let by = r.top() + 446.0;
        let play_r = Rect::from_min_size(pos2(x, by), vec2(170.0, 50.0));
        let set_r = Rect::from_min_size(pos2(play_r.right() + 16.0, by), vec2(170.0, 50.0));
        let playable = state == TileState::Ready;
        if button(
            ui,
            p,
            play_r,
            "play",
            "Play",
            ButtonStyle::Primary,
            playable,
            self.model.focus == Focus::Play,
        )
        .clicked()
            && playable
        {
            clicks.push(Click::Focus(Focus::Play, Some(Input::Press)));
        }
        if button(
            ui,
            p,
            set_r,
            "game-settings",
            "Settings",
            ButtonStyle::Outline,
            true,
            self.model.focus == Focus::GameSettings,
        )
        .clicked()
        {
            clicks.push(Click::Focus(Focus::GameSettings, Some(Input::Press)));
        }

        // the tile row
        let found = (0..GAMES.len())
            .filter(|i| self.entry(*i).is_some())
            .count();
        let row_label = match &self.entries {
            None => "Your games · looking…".to_string(),
            Some(_) => format!("Your games · {found} of {} found", GAMES.len()),
        };
        text(
            p,
            pos2(x, r.top() + 528.0),
            Align2::LEFT_BOTTOM,
            &row_label,
            theme::semibold(size::BODY),
            MUTED,
        );
        let gap = 12.0;
        let n = GAMES.len() as f32;
        let tw = (r.width() - 2.0 * MARGIN - gap * (n - 1.0)) / n;
        let th = tw * 9.0 / 16.0;
        let ty = r.top() + 540.0;
        for (i, game) in GAMES.iter().enumerate() {
            let tr = Rect::from_min_size(pos2(x + i as f32 * (tw + gap), ty), vec2(tw, th));
            let st = self.state(i);
            let dim = matches!(st, TileState::Blocked(_) | TileState::NotInLibrary);
            let resp = ui
                .interact(tr, Id::unique(("tile", i)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            let radius = CornerRadius::same(6);
            p.rect_filled(tr, radius, PANEL);
            if let Some(tex) = &self.tiles[i] {
                let tint = if dim {
                    Color32::from_gray(95)
                } else if resp.hovered() || i == sel {
                    Color32::WHITE
                } else {
                    Color32::from_gray(200)
                };
                p.image(tex.id(), tr, cover_uv(tex.size_vec2(), tr), tint);
            }
            p.rect_stroke(
                tr,
                radius,
                Stroke::new(1.0, alpha(LINE, 120)),
                StrokeKind::Inside,
            );
            shield(
                p,
                pos2(tr.left() + 6.0, tr.bottom() - 6.0),
                game.short,
                size::SHIELD,
            );
            if let Some(badge) = st.badge() {
                let bf = theme::semibold(size::BADGE);
                let bw = text_width(p, badge, bf.clone()) + 26.0;
                let br = Rect::from_min_size(
                    pos2(tr.right() - bw - 5.0, tr.top() + 5.0),
                    vec2(bw, 20.0),
                );
                p.rect_filled(br, CornerRadius::same(3), alpha(BASE, 235));
                if matches!(st, TileState::Blocked(_)) {
                    padlock(p, pos2(br.left() + 10.0, br.center().y - 1.0), 8.0, INK);
                } else {
                    p.circle_stroke(
                        pos2(br.left() + 10.0, br.center().y),
                        4.0,
                        Stroke::new(1.5, INK),
                    );
                    p.line_segment(
                        [
                            pos2(br.left() + 7.0, br.center().y + 3.0),
                            pos2(br.left() + 13.0, br.center().y - 3.0),
                        ],
                        Stroke::new(1.5, INK),
                    );
                }
                text(
                    p,
                    pos2(br.left() + 19.0, br.center().y),
                    Align2::LEFT_CENTER,
                    badge,
                    bf,
                    INK,
                );
            }
            if i == sel {
                p.rect_filled(
                    Rect::from_min_size(
                        pos2(tr.left() + tw * 0.3, tr.bottom() + 10.0),
                        vec2(tw * 0.4, 4.0),
                    ),
                    CornerRadius::same(2),
                    SODIUM,
                );
                if self.model.focus == Focus::Tile {
                    focus_ring(p, tr, 6);
                }
            }
            if resp.clicked() || resp.double_clicked() {
                clicks.push(Click::Tile(i, resp.double_clicked()));
            }
        }
    }

    fn draw_page_heading(&self, p: &egui::Painter, r: Rect, title: &str, sub: &str) {
        let x = r.left() + MARGIN;
        text(
            p,
            pos2(x, r.top() + BAR_H + 22.0),
            Align2::LEFT_TOP,
            title,
            theme::display(size::PAGE_TITLE),
            INK,
        );
        text(
            p,
            pos2(x, r.top() + BAR_H + 66.0),
            Align2::LEFT_TOP,
            sub,
            theme::body(size::BODY),
            MUTED,
        );
    }

    fn draw_settings(
        &mut self,
        ui: &egui::Ui,
        p: &egui::Painter,
        r: Rect,
        clicks: &mut Vec<Click>,
    ) {
        let sel = self.model.selected;
        let g = &GAMES[sel];
        self.draw_page_heading(
            p,
            r,
            &format!("{} settings", g.title),
            "Changes are saved as you make them and used the next time you press Play.",
        );
        let Some((_, form)) = &self.form else {
            return;
        };
        let top = r.top() + 162.0;
        let right_w = 340.0;
        let left = Rect::from_min_max(
            pos2(r.left() + MARGIN, top),
            pos2(
                r.right() - MARGIN - right_w - 24.0,
                top + 40.0 + 7.0 * 58.0 + 8.0,
            ),
        );
        card(p, left);
        let mut y = card_heading(p, left, "Picture and controls") + 4.0;
        for (idx, item) in self.model.items.iter().enumerate() {
            let Item::Option(row) = *item else { continue };
            let rr =
                Rect::from_min_size(pos2(left.left() + 4.0, y), vec2(left.width() - 8.0, 58.0));
            y += 58.0;
            let focused = self.model.focus == Focus::Item(idx);
            let off = form.disabled(row);
            let resp = ui
                .interact(rr, Id::unique(("row", idx)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            if focused {
                p.rect_filled(
                    rr,
                    CornerRadius::same(4),
                    Color32::from_rgb(0x1F, 0x2C, 0x3B),
                );
                p.rect_stroke(
                    rr,
                    CornerRadius::same(4),
                    Stroke::new(3.0, SODIUM),
                    StrokeKind::Inside,
                );
            } else if resp.hovered() {
                p.rect_filled(rr, CornerRadius::same(4), alpha(INK, 10));
            }
            let lx = rr.left() + 18.0;
            let label_color = if off.is_some() { MUTED } else { INK };
            text(
                p,
                pos2(lx, rr.top() + 9.0),
                Align2::LEFT_TOP,
                row.label(),
                theme::semibold(size::LABEL),
                label_color,
            );
            let desc = off.unwrap_or(row.description());
            let valw = 270.0;
            ellipsized_text(
                p,
                pos2(lx, rr.top() + 32.0),
                desc,
                theme::body(size::NOTE),
                MUTED,
                rr.width() - valw - 40.0,
            );
            // ◀ value ▶
            let vr = Rect::from_min_max(
                pos2(rr.right() - valw - 12.0, rr.top()),
                pos2(rr.right() - 12.0, rr.bottom()),
            );
            let vc = if off.is_some() { DISABLED } else { INK };
            text(
                p,
                vr.center(),
                Align2::CENTER_CENTER,
                &form.value(row),
                theme::semibold(size::LABEL),
                vc,
            );
            if off.is_none() {
                let ac = if focused { SODIUM } else { MUTED };
                let lr =
                    Rect::from_center_size(pos2(vr.left() + 12.0, vr.center().y), vec2(28.0, 40.0));
                let rr2 = Rect::from_center_size(
                    pos2(vr.right() - 12.0, vr.center().y),
                    vec2(28.0, 40.0),
                );
                arrow(p, lr.center(), -1.0, ac);
                arrow(p, rr2.center(), 1.0, ac);
                let l = ui.interact(lr, Id::unique(("left", idx)), Sense::click());
                let rt = ui.interact(rr2, Id::unique(("right", idx)), Sense::click());
                if l.clicked() {
                    clicks.push(Click::Focus(Focus::Item(idx), Some(Input::Left)));
                    continue;
                }
                if rt.clicked() {
                    clicks.push(Click::Focus(Focus::Item(idx), Some(Input::Right)));
                    continue;
                }
            } else {
                // "off" shown by shape too: a lock beside the value
                padlock(
                    p,
                    pos2(vr.right() - 12.0, vr.center().y - 1.0),
                    9.0,
                    DISABLED,
                );
            }
            if resp.clicked() {
                let input = if off.is_none() {
                    Some(Input::Press)
                } else {
                    None
                };
                clicks.push(Click::Focus(Focus::Item(idx), input));
            }
        }

        // the library card
        let rc = Rect::from_min_max(
            pos2(left.right() + 24.0, top),
            pos2(r.right() - MARGIN, left.bottom()),
        );
        card(p, rc);
        let mut y = card_heading(p, rc, "Your games folder") + 16.0;
        let ix = rc.left() + 20.0;
        let iw = rc.width() - 40.0;
        let folder = self
            .library
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Not chosen yet".into());
        y = wrapped(p, pos2(ix, y), &folder, theme::body(size::BODY), INK, iw).bottom() + 8.0;
        let status = match &self.entries {
            None => "Looking for your games…".to_string(),
            Some(e) => {
                let found = (0..GAMES.len())
                    .filter(|i| self.entry(*i).is_some())
                    .count();
                let unknown = e.iter().filter(|e| e.game_id().is_none()).count();
                let mut s = format!("{found} of {} games found.", GAMES.len());
                if unknown > 0 {
                    s.push_str(&format!(
                        " {unknown} folder{} holds a build this launcher doesn't know; launcher.log has its digest.",
                        if unknown == 1 { "" } else { "s" }
                    ));
                }
                s
            }
        };
        y = wrapped(p, pos2(ix, y), &status, theme::body(size::BODY), MUTED, iw).bottom() + 18.0;
        let n = self.model.items.len();
        let b1 = Rect::from_min_size(pos2(ix, y), vec2(iw, 46.0));
        let b2 = Rect::from_min_size(pos2(ix, y + 60.0), vec2(iw, 46.0));
        let scanning = self.scan.is_some();
        if button(
            ui,
            p,
            b1,
            "rescan",
            if scanning { "Looking…" } else { "Rescan" },
            ButtonStyle::Outline,
            !scanning,
            self.model.focus == Focus::Item(n - 2),
        )
        .clicked()
            && !scanning
        {
            clicks.push(Click::Focus(Focus::Item(n - 2), Some(Input::Press)));
        }
        if button(
            ui,
            p,
            b2,
            "folder",
            "Choose another folder",
            ButtonStyle::Outline,
            true,
            self.model.focus == Focus::Item(n - 1),
        )
        .clicked()
        {
            clicks.push(Click::Focus(Focus::Item(n - 1), Some(Input::Press)));
        }
        if let Some((note, ok)) = &self.note {
            let at = pos2(ix, b2.bottom() + 22.0);
            let color = if *ok { INK } else { ERROR };
            wrapped(p, at, note, theme::body(size::BODY), color, iw);
        }
    }

    fn draw_controls(&self, p: &egui::Painter, r: Rect) {
        self.draw_page_heading(
            p,
            r,
            "Controls",
            "Turn the wheel or move the stick, press the pedals and buttons: the bars should follow.",
        );
        let top = r.top() + 162.0;
        let bottom = r.bottom() - 62.0;
        let split = r.left() + MARGIN + (r.width() - 2.0 * MARGIN - 24.0) * 0.46;

        // devices
        let dev = Rect::from_min_max(pos2(r.left() + MARGIN, top), pos2(split, top + 196.0));
        card(p, dev);
        let mut y = card_heading(p, dev, "Your controllers") + 14.0;
        let ix = dev.left() + 20.0;
        let iw = dev.width() - 40.0;
        if let Some(e) = &self.snap.error {
            wrapped(
                p,
                pos2(ix, y),
                &format!(
                    "Controllers can't be read: {e}. Plug the controller in again, then restart the launcher."
                ),
                theme::body(size::BODY),
                ERROR,
                iw,
            );
        } else if self.snap.devices.is_empty() {
            wrapped(
                p,
                pos2(ix, y),
                "No controller found. Plug in a wheel or a pad; it shows up here.",
                theme::body(size::BODY),
                INK,
                iw,
            );
        }
        for (i, d) in self.snap.devices.iter().enumerate().take(4) {
            let kind = match d.kind {
                Kind::Wheel => "Wheel",
                Kind::Pad => "Pad",
                Kind::Other => "Other",
            };
            let k = chip(p, pos2(ix, y), kind, None, INK);
            let mut right = dev.right() - 20.0;
            if self.snap.driver == Some(i) {
                let label = "Drives the car";
                let w = text_width(p, label, theme::semibold(size::BODY)) + 24.0;
                chip(p, pos2(right - w, y), label, Some(SIGN), INK);
                right -= w + 10.0;
            }
            let name = ellipsize(
                p,
                &d.name,
                theme::body(size::BODY),
                right - k.right() - 14.0,
            );
            text(
                p,
                pos2(k.right() + 12.0, k.center().y),
                Align2::LEFT_CENTER,
                &name,
                theme::body(size::BODY),
                INK,
            );
            y += 38.0;
        }

        // default pad layout
        let lay = Rect::from_min_max(
            pos2(r.left() + MARGIN, dev.bottom() + 16.0),
            pos2(split, bottom),
        );
        card(p, lay);
        let mut y = card_heading(p, lay, "Pad layout") + 10.0;
        let map = default_pad_map();
        let mut rows: Vec<(String, String)> = vec![
            ("Steer".into(), "Left stick".into()),
            ("Gas".into(), map.describe(Action::Accelerate)),
            ("Brake".into(), map.describe(Action::Brake)),
        ];
        for b in Button::ALL {
            let keys = map.describe(Action::Press(*b));
            if keys != "-" {
                rows.push((b.label().to_string(), keys));
            }
        }
        let ix = lay.left() + 20.0;
        let colw = (lay.width() - 40.0) / 2.0;
        for (k, (what, keys)) in rows.iter().enumerate() {
            let col = (k / 6) as f32;
            let row = (k % 6) as f32;
            let at = pos2(ix + col * colw, y + row * 28.0);
            let kf = theme::semibold(size::NOTE);
            let kw = text_width(p, keys, kf.clone()) + 14.0;
            let kr = Rect::from_min_size(at, vec2(kw.max(36.0), 22.0));
            p.rect_stroke(
                kr,
                CornerRadius::same(4),
                Stroke::new(1.5, LINE),
                StrokeKind::Inside,
            );
            text(p, kr.center(), Align2::CENTER_CENTER, keys, kf, INK);
            ellipsized_text(
                p,
                pos2(kr.right() + 10.0, at.y + 2.0),
                what,
                theme::body(size::BODY),
                INK,
                colw - kr.width() - 18.0,
            );
        }
        y += 6.0 * 28.0 + 8.0;
        wrapped(
            p,
            pos2(ix, y),
            "A wheel steers with its wheel and pedals, and its paddles shift.",
            theme::body(size::NOTE),
            MUTED,
            lay.width() - 40.0,
        );

        // live check
        let live = Rect::from_min_max(pos2(split + 24.0, top), pos2(r.right() - MARGIN, bottom));
        card(p, live);
        let mut y = card_heading(p, live, "Live check") + 18.0;
        let c = &self.snap.controls;
        let ix = live.left() + 20.0;
        let iw = live.width() - 40.0;
        let label_w = 90.0;
        let num_w = 64.0;
        for (name, v, centred) in [
            ("Steering", c.wheel, true),
            ("Gas", c.gas, false),
            ("Brake", c.brake, false),
        ] {
            text(
                p,
                pos2(ix, y + 12.0),
                Align2::LEFT_CENTER,
                name,
                theme::semibold(size::LABEL),
                INK,
            );
            let br = Rect::from_min_size(pos2(ix + label_w, y), vec2(iw - label_w - num_w, 24.0));
            bar(p, br, v, centred);
            text(
                p,
                pos2(live.right() - 20.0, y + 12.0),
                Align2::RIGHT_CENTER,
                &format!("{:.0}%", v * 100.0),
                theme::body(size::BODY),
                INK,
            );
            y += 44.0;
        }
        text(
            p,
            pos2(ix, y + 22.0),
            Align2::LEFT_CENTER,
            "Gear",
            theme::semibold(size::LABEL),
            INK,
        );
        let gear = if c.gear == 0 {
            "N".to_string()
        } else {
            c.gear.to_string()
        };
        let gr = Rect::from_min_size(pos2(ix + label_w, y), vec2(56.0, 44.0));
        p.rect_stroke(
            gr,
            CornerRadius::same(4),
            Stroke::new(1.5, LINE),
            StrokeKind::Inside,
        );
        text(
            p,
            gr.center(),
            Align2::CENTER_CENTER,
            &gear,
            theme::display(30.0),
            SODIUM,
        );
        text(
            p,
            pos2(gr.right() + 14.0, gr.center().y),
            Align2::LEFT_CENTER,
            "Shift up and down to change it.",
            theme::body(size::NOTE),
            MUTED,
        );
        y += 64.0;
        text(
            p,
            pos2(ix, y),
            Align2::LEFT_TOP,
            "Buttons",
            theme::semibold(size::LABEL),
            INK,
        );
        y += 30.0;
        let pressed = [
            (Button::View, c.view),
            (Button::Intrude, c.intrude),
            (Button::Card, c.card),
            (Button::Enter, c.enter),
            (Button::Test, c.test),
            (Button::Service, c.service),
            (Button::MenuUp, c.menu_up),
            (Button::MenuDown, c.menu_down),
        ];
        let mut x = ix;
        for (b, on) in pressed {
            let label = short_button(b);
            let w = text_width(p, label, theme::semibold(size::BODY)) + 24.0;
            if x + w > live.right() - 20.0 {
                x = ix;
                y += 38.0;
            }
            if on {
                chip(p, pos2(x, y), label, Some(SODIUM), BASE);
            } else {
                chip(p, pos2(x, y), label, None, MUTED);
            }
            x += w + 10.0;
        }
        y += 44.0;
        wrapped(
            p,
            pos2(ix, y),
            "A pressed button turns solid.",
            theme::body(size::NOTE),
            MUTED,
            iw,
        );
    }

    fn draw_first_run(
        &mut self,
        ui: &mut egui::Ui,
        p: &egui::Painter,
        r: Rect,
        clicks: &mut Vec<Click>,
    ) {
        let w = 720.0_f32.min(r.width() - 2.0 * MARGIN);
        let x = r.left() + MARGIN + 40.0;
        let mut y = r.top() + 170.0;
        text(
            p,
            pos2(x, y),
            Align2::LEFT_TOP,
            "Choose your games folder",
            theme::display(48.0),
            INK,
        );
        y += 66.0;
        y = wrapped(
            p,
            pos2(x, y),
            "Type the path of the folder that holds your Wangan Midnight game folders. \
             The launcher looks inside it for each game and never changes what's there.",
            theme::body(size::LABEL),
            INK,
            w,
        )
        .bottom()
            + 24.0;
        let field = Rect::from_min_size(pos2(x, y), vec2(w, 48.0));
        let focused = self.model.focus == Focus::Field;
        p.rect_filled(field, CornerRadius::same(6), PANEL);
        let resp = ui
            .interact(field, Id::unique("folder-field"), Sense::click())
            .on_hover_cursor(CursorIcon::Text);
        if resp.clicked() {
            clicks.push(Click::Focus(Focus::Field, None));
        }
        // the text, its end kept in view, and a caret
        let font = theme::body(size::LABEL);
        let inner = field.shrink2(vec2(16.0, 0.0));
        if self.folder.is_empty() {
            text(
                p,
                inner.left_center(),
                Align2::LEFT_CENTER,
                "/path/to/your/games",
                font.clone(),
                MUTED,
            );
        }
        let g = p.layout_no_wrap(self.folder.clone(), font, INK);
        let tw = g.size().x;
        let tx = inner.left() - (tw - inner.width() + 6.0).max(0.0);
        p.with_clip_rect(inner)
            .galley(pos2(tx, inner.center().y - g.size().y / 2.0), g, INK);
        let blink = ui.input(|i| i.time).fract() < 0.6;
        if focused && blink {
            let cx = (tx + tw + 2.0).min(inner.right());
            p.line_segment(
                [
                    pos2(cx, inner.center().y - 11.0),
                    pos2(cx, inner.center().y + 11.0),
                ],
                Stroke::new(2.0, SODIUM),
            );
        }
        p.rect_stroke(
            field,
            CornerRadius::same(6),
            Stroke::new(2.0, if focused { INK } else { LINE }),
            StrokeKind::Inside,
        );
        if focused {
            focus_ring(p, field, 6);
        }
        y = field.bottom() + 12.0;
        let hint_text = "For example /mnt/games/TeknoParrot. Press Enter to use it.";
        text(
            p,
            pos2(x, y),
            Align2::LEFT_TOP,
            hint_text,
            theme::body(size::BODY),
            MUTED,
        );
        y += 30.0;
        if let Some(e) = &self.folder_error {
            y = wrapped(p, pos2(x, y), e, theme::body(size::BODY), ERROR, w).bottom() + 10.0;
        }
        y += 12.0;
        let br = Rect::from_min_size(pos2(x, y), vec2(220.0, 50.0));
        if button(
            ui,
            p,
            br,
            "use-folder",
            "Use this folder",
            ButtonStyle::Primary,
            true,
            self.model.focus == Focus::UseFolder,
        )
        .clicked()
        {
            clicks.push(Click::Focus(Focus::UseFolder, Some(Input::Press)));
        }
        if self.library.is_some() {
            let cr = Rect::from_min_size(pos2(br.right() + 16.0, y), vec2(150.0, 50.0));
            if button(
                ui,
                p,
                cr,
                "cancel-folder",
                "Cancel",
                ButtonStyle::Outline,
                true,
                false,
            )
            .clicked()
            {
                clicks.push(Click::Focus(Focus::UseFolder, Some(Input::Back)));
            }
        }
    }

    fn draw_footer(&self, p: &egui::Painter, r: Rect) {
        let cy = r.bottom() - 30.0;
        let x = r.left() + MARGIN;
        let hints: &[(&str, &str)] = match self.model.page {
            Page::Hub => &[
                ("A", "Play"),
                ("Y", "Settings"),
                ("B", "Back"),
                ("Start / Esc", "Quit"),
            ],
            Page::Settings => &[
                (ARROWS, "Change"),
                ("A", "Next choice"),
                ("B / Esc", "Back"),
                ("LB RB", "Switch page"),
            ],
            Page::Controls => &[("B / Esc", "Back"), ("LB RB", "Switch page")],
            Page::FirstRun if self.library.is_some() => {
                &[("Enter", "Use this folder"), ("Esc", "Cancel")]
            }
            Page::FirstRun => &[("Enter", "Use this folder"), ("Esc", "Quit")],
        };
        let mut hx = x;
        for (k, w) in hints {
            hx = hint(p, hx, cy, k, w);
        }
        text(
            p,
            pos2(r.right() - MARGIN, cy),
            Align2::RIGHT_CENTER,
            &format!("Version {}", env!("CARGO_PKG_VERSION")),
            theme::body(size::NOTE),
            MUTED,
        );
    }

    fn draw_message(&mut self, ui: &egui::Ui, p: &egui::Painter, r: Rect, clicks: &mut Vec<Click>) {
        let Some(m) = &self.message else { return };
        p.rect_filled(r, CornerRadius::ZERO, alpha(BASE, 200));
        let w = 620.0;
        let body = p.layout(m.body.clone(), theme::body(size::BODY), INK, w - 48.0);
        let h = 40.0 + 24.0 + body.size().y + 24.0 + 50.0 + 24.0;
        let cr = Rect::from_center_size(r.center(), vec2(w, h));
        card(p, cr);
        let y = card_heading(p, cr, &m.title) + 24.0;
        p.galley(pos2(cr.left() + 24.0, y), body.clone(), INK);
        let br = Rect::from_min_size(
            pos2(cr.left() + 24.0, y + body.size().y + 24.0),
            vec2(140.0, 50.0),
        );
        if button(ui, p, br, "dismiss", "OK", ButtonStyle::Primary, true, true).clicked() {
            clicks.push(Click::Dismiss);
        }
    }
}

/// Apply typed text, pastes and Backspace to a one-line field (no IME:
/// a folder path needs none, and an X server without an input method
/// would swallow the keys). Returns whether the text changed.
fn edit_text(s: &mut String, events: &[egui::Event]) -> bool {
    let before = s.clone();
    for e in events {
        match e {
            egui::Event::Text(t) | egui::Event::Paste(t) => {
                s.extend(t.chars().filter(|c| !c.is_control()));
            }
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                modifiers,
                ..
            } => {
                if modifiers.ctrl {
                    // a whole path part
                    let keep = s.trim_end_matches('/').rfind('/').map_or(0, |i| i + 1);
                    s.truncate(keep);
                } else {
                    s.pop();
                }
            }
            _ => {}
        }
    }
    *s != before
}

fn arrow(p: &egui::Painter, c: Pos2, dir: f32, color: Color32) {
    let s = 7.0;
    let pts = vec![
        pos2(c.x + dir * s, c.y),
        pos2(c.x - dir * s * 0.6, c.y - s),
        pos2(c.x - dir * s * 0.6, c.y + s),
    ];
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// A level bar: 0…1, or -1…1 from the centre.
fn bar(p: &egui::Painter, r: Rect, v: f32, centred: bool) {
    p.rect_filled(r, CornerRadius::same(4), BASE);
    p.rect_stroke(
        r,
        CornerRadius::same(4),
        Stroke::new(1.5, LINE),
        StrokeKind::Inside,
    );
    let inner = r.shrink(3.0);
    let fill = if centred {
        let c = inner.center().x;
        let x = c + v.clamp(-1.0, 1.0) * inner.width() / 2.0;
        Rect::from_min_max(pos2(c.min(x), inner.top()), pos2(c.max(x), inner.bottom()))
    } else {
        Rect::from_min_size(
            inner.min,
            vec2(inner.width() * v.clamp(0.0, 1.0), inner.height()),
        )
    };
    p.rect_filled(fill, CornerRadius::same(2), SODIUM);
    if centred {
        p.line_segment(
            [
                pos2(r.center().x, r.top() - 3.0),
                pos2(r.center().x, r.bottom() + 3.0),
            ],
            Stroke::new(2.0, INK),
        );
    }
}

fn short_button(b: Button) -> &'static str {
    match b {
        Button::Intrude => "Intrude",
        Button::Test => "Test",
        Button::MenuUp => "Menu up",
        Button::MenuDown => "Menu down",
        Button::Enter => "Enter",
        other => other.label(),
    }
}

fn draw_device_glyph(p: &egui::Painter, c: Pos2, kind: Option<Kind>) {
    let st = Stroke::new(2.0, INK);
    match kind {
        Some(Kind::Wheel) => {
            p.circle_stroke(c, 8.0, st);
            p.circle_filled(c, 2.5, INK);
            p.line_segment([pos2(c.x - 8.0, c.y), pos2(c.x + 8.0, c.y)], st);
        }
        Some(_) => {
            let r = Rect::from_center_size(c, vec2(20.0, 12.0));
            p.rect_stroke(r, CornerRadius::same(5), st, StrokeKind::Inside);
            p.circle_filled(pos2(c.x + 4.5, c.y), 1.6, INK);
            p.line_segment(
                [pos2(c.x - 7.0, c.y), pos2(c.x - 2.0, c.y)],
                Stroke::new(1.5, INK),
            );
        }
        None => {
            let r = Rect::from_center_size(c, vec2(20.0, 12.0));
            p.rect_stroke(
                r,
                CornerRadius::same(5),
                Stroke::new(2.0, MUTED),
                StrokeKind::Inside,
            );
            p.line_segment(
                [pos2(c.x - 10.0, c.y + 8.0), pos2(c.x + 10.0, c.y - 8.0)],
                Stroke::new(2.0, INK),
            );
        }
    }
}

fn ellipsize(p: &egui::Painter, s: &str, font: FontId, max: f32) -> String {
    if text_width(p, s, font.clone()) <= max {
        return s.to_string();
    }
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let t: String = chars.iter().collect::<String>() + "…";
        if text_width(p, &t, font.clone()) <= max {
            return t;
        }
    }
    "…".into()
}

fn ellipsized_text(p: &egui::Painter, pos: Pos2, s: &str, font: FontId, color: Color32, max: f32) {
    let t = ellipsize(p, s, font.clone(), max);
    text(p, pos, Align2::LEFT_TOP, &t, font, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_fit_crops_the_long_side_and_centres() {
        let wide = Rect::from_min_size(Pos2::ZERO, vec2(2000.0, 500.0));
        let uv = cover_uv(vec2(1920.0, 1080.0), wide);
        assert!((uv.width() - 1.0).abs() < 1e-6);
        assert!(uv.height() < 1.0 && (uv.center().y - 0.5).abs() < 1e-6);
        let tall = Rect::from_min_size(Pos2::ZERO, vec2(500.0, 1000.0));
        let uv = cover_uv(vec2(1920.0, 1080.0), tall);
        assert!((uv.height() - 1.0).abs() < 1e-6 && uv.width() < 1.0);
        let same = Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 720.0));
        assert!((cover_uv(vec2(1920.0, 1080.0), same).width() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_folder_field_takes_text_paste_and_backspace() {
        let key = |ctrl| egui::Event::Key {
            key: egui::Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl,
                ..Default::default()
            },
        };
        let mut s = String::new();
        assert!(edit_text(
            &mut s,
            &[
                egui::Event::Text("/mnt/".into()),
                egui::Event::Paste("games/Teknoparrot\n".into()),
            ]
        ));
        assert_eq!(s, "/mnt/games/Teknoparrot");
        edit_text(&mut s, &[key(false)]);
        assert_eq!(s, "/mnt/games/Teknoparro");
        edit_text(&mut s, &[key(true)]);
        assert_eq!(s, "/mnt/games/");
        edit_text(&mut s, &[key(true)]);
        assert_eq!(s, "/mnt/");
        assert!(!edit_text(&mut s, &[egui::Event::Copy]));
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            sentence("starting gamescope (is it installed?)."),
            "Starting gamescope (is it installed?)"
        );
        assert_eq!(sentence(""), "Something went wrong");
    }

    #[test]
    fn every_game_has_a_short_name_for_its_shield() {
        for g in GAMES {
            assert!(!g.short.is_empty() && g.short.len() <= 6, "{}", g.slug);
        }
    }
}
