//! Where the focus is and what each input does, without a window: pages,
//! the top navigation, the hub's buttons and tiles, the settings rows and
//! the first-run folder field. The window draws this and acts on the
//! [`Effect`]s it returns.

use wangan_core::catalog::{Game, Support};

use super::form::Row;

/// One menu input, from the pad, the keyboard or a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    /// A, Enter.
    Press,
    /// B, Back.
    Back,
    /// Esc: back, or quit on the hub.
    Escape,
    /// Start: quit on the hub, else back to the hub.
    Start,
    /// Y: the selected game's settings.
    Settings,
    PreviousTab,
    NextTab,
}

impl Input {
    pub fn from_nav(n: rea_launcher::pad::Nav) -> Self {
        use rea_launcher::pad::Nav;
        match n {
            Nav::Up => Input::Up,
            Nav::Down => Input::Down,
            Nav::Left => Input::Left,
            Nav::Right => Input::Right,
            Nav::Press => Input::Press,
            Nav::Back => Input::Back,
            Nav::Play => Input::Start,
            Nav::PreviousTab => Input::PreviousTab,
            Nav::NextTab => Input::NextTab,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    FirstRun,
    Hub,
    Controls,
    Settings,
}

/// The top bar's sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    Controls,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Games, Tab::Controls, Tab::Settings];
    pub fn label(self) -> &'static str {
        match self {
            Tab::Games => "Games",
            Tab::Controls => "Controls",
            Tab::Settings => "Settings",
        }
    }
    pub fn page(self) -> Page {
        match self {
            Tab::Games => Page::Hub,
            Tab::Controls => Page::Controls,
            Tab::Settings => Page::Settings,
        }
    }
    fn of(page: Page) -> Option<Tab> {
        match page {
            Page::Hub => Some(Tab::Games),
            Page::Controls => Some(Tab::Controls),
            Page::Settings => Some(Tab::Settings),
            Page::FirstRun => None,
        }
    }
    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// The settings page's items, top to bottom: the game's options, then the
/// library actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Option(Row),
    Rescan,
    ChangeFolder,
}

pub fn settings_items() -> Vec<Item> {
    Row::ALL
        .iter()
        .map(|r| Item::Option(*r))
        .chain([Item::Rescan, Item::ChangeFolder])
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tab(Tab),
    Play,
    GameSettings,
    Tile,
    Item(usize),
    Field,
    UseFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Start the selected game.
    Play,
    Quit,
    /// Change a setting by -1/+1; `wrap` when it came from A.
    Adjust(Row, i32, bool),
    Rescan,
    /// Accept the folder typed on the first-run page.
    SubmitFolder,
    /// The settings page was opened (load the selected game's form).
    OpenedSettings,
    /// The folder page was opened from settings.
    OpenedFolder,
}

/// What the model needs to know about the world to move the focus.
#[derive(Debug, Clone, Copy)]
pub struct World {
    /// The selected game can start now.
    pub playable: bool,
    /// A library folder is set (the folder page can be left).
    pub has_library: bool,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub page: Page,
    pub focus: Focus,
    /// Index into `catalog::GAMES`.
    pub selected: usize,
    pub games: usize,
    pub items: Vec<Item>,
}

impl Model {
    pub fn new(games: usize, has_library: bool) -> Self {
        let mut m = Self {
            page: Page::Hub,
            focus: Focus::Tile,
            selected: 0,
            games,
            items: settings_items(),
        };
        if !has_library {
            m.open(Page::FirstRun);
        }
        m
    }

    /// Go to a page with its natural first focus.
    pub fn open(&mut self, page: Page) -> Effect {
        self.page = page;
        self.focus = match page {
            Page::Hub => Focus::Tile,
            Page::Controls => Focus::Tab(Tab::Controls),
            Page::Settings => Focus::Item(0),
            Page::FirstRun => Focus::Field,
        };
        match page {
            Page::Settings => Effect::OpenedSettings,
            _ => Effect::None,
        }
    }

    fn hub_button(w: World) -> Focus {
        if w.playable {
            Focus::Play
        } else {
            Focus::GameSettings
        }
    }

    pub fn select(&mut self, i: usize) {
        self.selected = i.min(self.games.saturating_sub(1));
    }

    pub fn input(&mut self, input: Input, w: World) -> Effect {
        // shared by every page with a top bar
        if self.page != Page::FirstRun {
            match input {
                Input::PreviousTab | Input::NextTab => {
                    let cur = Tab::of(self.page).unwrap_or(Tab::Games).index() as i32;
                    let d = if input == Input::NextTab { 1 } else { -1 };
                    let n = (cur + d).rem_euclid(Tab::ALL.len() as i32) as usize;
                    return self.open(Tab::ALL[n].page());
                }
                Input::Settings if self.page == Page::Hub => return self.open(Page::Settings),
                Input::Start | Input::Escape if self.page == Page::Hub => return Effect::Quit,
                Input::Start | Input::Escape | Input::Back if self.page != Page::Hub => {
                    return self.open(Page::Hub);
                }
                _ => {}
            }
            if let Focus::Tab(t) = self.focus {
                return self.on_tab(t, input, w);
            }
        }
        match self.page {
            Page::Hub => self.on_hub(input, w),
            Page::Settings => self.on_settings(input),
            Page::Controls => Effect::None,
            Page::FirstRun => self.on_first_run(input, w),
        }
    }

    fn on_tab(&mut self, t: Tab, input: Input, w: World) -> Effect {
        let i = t.index();
        match input {
            Input::Left if i > 0 => self.focus = Focus::Tab(Tab::ALL[i - 1]),
            Input::Right if i + 1 < Tab::ALL.len() => self.focus = Focus::Tab(Tab::ALL[i + 1]),
            Input::Press => {
                if self.page == t.page() {
                    return self.input(Input::Down, w);
                }
                let e = self.open(t.page());
                if t == Tab::Controls {
                    self.focus = Focus::Tab(Tab::Controls);
                }
                return e;
            }
            Input::Down => {
                self.focus = match self.page {
                    Page::Hub => Self::hub_button(w),
                    Page::Settings => Focus::Item(0),
                    _ => self.focus,
                }
            }
            Input::Back => self.focus = Focus::Tile,
            _ => {}
        }
        Effect::None
    }

    fn on_hub(&mut self, input: Input, w: World) -> Effect {
        match (self.focus, input) {
            (Focus::Tile, Input::Left) => self.select(self.selected.saturating_sub(1)),
            (Focus::Tile, Input::Right) => self.select(self.selected + 1),
            (Focus::Tile, Input::Up) => self.focus = Self::hub_button(w),
            (Focus::Tile, Input::Press) if w.playable => return Effect::Play,
            (Focus::Tile, Input::Press) => self.focus = Focus::GameSettings,
            (Focus::Play | Focus::GameSettings, Input::Down) => self.focus = Focus::Tile,
            (Focus::Play | Focus::GameSettings, Input::Up) => self.focus = Focus::Tab(Tab::Games),
            (Focus::Play | Focus::GameSettings, Input::Back) => self.focus = Focus::Tile,
            (Focus::Play, Input::Right) => self.focus = Focus::GameSettings,
            (Focus::GameSettings, Input::Left) if w.playable => self.focus = Focus::Play,
            (Focus::Play, Input::Press) if w.playable => return Effect::Play,
            (Focus::GameSettings, Input::Press) => return self.open(Page::Settings),
            _ => {}
        }
        // the selection may have made Play unavailable: not focused then
        Effect::None
    }

    /// After the selection or the library changed: keep focus off a Play
    /// that can't be pressed.
    pub fn settle(&mut self, w: World) {
        if self.focus == Focus::Play && !w.playable {
            self.focus = Focus::GameSettings;
        }
    }

    fn on_settings(&mut self, input: Input) -> Effect {
        let Focus::Item(i) = self.focus else {
            self.focus = Focus::Item(0);
            return Effect::None;
        };
        let item = self.items[i.min(self.items.len() - 1)];
        match (item, input) {
            (_, Input::Up) if i == 0 => self.focus = Focus::Tab(Tab::Settings),
            (_, Input::Up) => self.focus = Focus::Item(i - 1),
            (_, Input::Down) if i + 1 < self.items.len() => self.focus = Focus::Item(i + 1),
            (Item::Option(r), Input::Left) => return Effect::Adjust(r, -1, false),
            (Item::Option(r), Input::Right) => return Effect::Adjust(r, 1, false),
            (Item::Option(r), Input::Press) => return Effect::Adjust(r, 1, true),
            (Item::Rescan, Input::Press) => return Effect::Rescan,
            (Item::ChangeFolder, Input::Press) => {
                self.open(Page::FirstRun);
                return Effect::OpenedFolder;
            }
            _ => {}
        }
        Effect::None
    }

    fn on_first_run(&mut self, input: Input, w: World) -> Effect {
        match (self.focus, input) {
            (Focus::Field | Focus::UseFolder, Input::Press) => Effect::SubmitFolder,
            (Focus::Field, Input::Down | Input::Right) => {
                self.focus = Focus::UseFolder;
                Effect::None
            }
            (Focus::UseFolder, Input::Up | Input::Left) => {
                self.focus = Focus::Field;
                Effect::None
            }
            (_, Input::Back | Input::Escape) if w.has_library => {
                self.open(Page::Settings);
                self.focus = Focus::Item(self.items.len() - 1);
                Effect::OpenedSettings
            }
            (_, Input::Start | Input::Escape) if !w.has_library => Effect::Quit,
            _ => Effect::None,
        }
    }
}

/// What a tile and the hero say about a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileState {
    /// The library is still being read.
    Looking,
    Ready,
    NotInLibrary,
    Blocked(&'static str),
}

impl TileState {
    pub fn of(game: &Game, scanned: bool, in_library: bool) -> Self {
        match (game.support_here(), scanned, in_library) {
            (Support::Blocked(why), _, _) => TileState::Blocked(why),
            (_, false, _) => TileState::Looking,
            (_, true, false) => TileState::NotInLibrary,
            (Support::Ready, true, true) => TileState::Ready,
        }
    }

    pub fn chip(self) -> &'static str {
        match self {
            TileState::Looking => "Looking for your games…",
            TileState::Ready => "Ready",
            TileState::NotInLibrary => "Not in your library",
            TileState::Blocked(_) => "Not supported yet",
        }
    }

    /// The tile's small badge (shape and word, never colour alone).
    pub fn badge(self) -> Option<&'static str> {
        match self {
            TileState::NotInLibrary => Some("Not found"),
            TileState::Blocked(_) => Some("Locked"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wangan_core::catalog::{self, GAMES};

    const READY: World = World {
        playable: true,
        has_library: true,
    };
    const NOT_READY: World = World {
        playable: false,
        has_library: true,
    };

    #[test]
    fn tiles_move_left_right_and_clamp() {
        let mut m = Model::new(GAMES.len(), true);
        assert_eq!((m.page, m.focus), (Page::Hub, Focus::Tile));
        m.input(Input::Left, READY);
        assert_eq!(m.selected, 0);
        for _ in 0..20 {
            m.input(Input::Right, READY);
        }
        assert_eq!(m.selected, GAMES.len() - 1);
    }

    #[test]
    fn up_goes_tiles_to_buttons_to_top_bar_and_back_down() {
        let mut m = Model::new(GAMES.len(), true);
        m.input(Input::Up, READY);
        assert_eq!(m.focus, Focus::Play);
        m.input(Input::Right, READY);
        assert_eq!(m.focus, Focus::GameSettings);
        m.input(Input::Up, READY);
        assert_eq!(m.focus, Focus::Tab(Tab::Games));
        m.input(Input::Right, READY);
        assert_eq!(m.focus, Focus::Tab(Tab::Controls));
        m.input(Input::Back, READY);
        assert_eq!(m.focus, Focus::Tile);
        m.input(Input::Up, READY);
        m.input(Input::Down, READY);
        assert_eq!(m.focus, Focus::Tile);
    }

    #[test]
    fn play_only_when_playable() {
        let mut m = Model::new(GAMES.len(), true);
        assert_eq!(m.input(Input::Press, READY), Effect::Play);
        assert_eq!(m.input(Input::Press, NOT_READY), Effect::None);
        assert_eq!(
            m.focus,
            Focus::GameSettings,
            "A on a locked game points at Settings"
        );
        m.input(Input::Down, NOT_READY);
        m.input(Input::Up, NOT_READY);
        assert_eq!(m.focus, Focus::GameSettings, "Play is skipped");
        m.input(Input::Left, NOT_READY);
        assert_eq!(m.focus, Focus::GameSettings);
        m.focus = Focus::Play;
        m.settle(NOT_READY);
        assert_eq!(m.focus, Focus::GameSettings);
    }

    #[test]
    fn quit_only_from_the_hub_and_back_returns_there() {
        let mut m = Model::new(GAMES.len(), true);
        assert_eq!(m.input(Input::Settings, READY), Effect::OpenedSettings);
        assert_eq!(m.page, Page::Settings);
        assert_eq!(m.input(Input::Escape, READY), Effect::None);
        assert_eq!(m.page, Page::Hub);
        assert_eq!(m.input(Input::Back, READY), Effect::None, "B never quits");
        assert_eq!(m.input(Input::Start, READY), Effect::Quit);
        assert_eq!(m.input(Input::Escape, READY), Effect::Quit);
    }

    #[test]
    fn shoulders_cycle_the_top_bar_pages() {
        let mut m = Model::new(GAMES.len(), true);
        m.input(Input::NextTab, READY);
        assert_eq!(m.page, Page::Controls);
        assert_eq!(m.input(Input::NextTab, READY), Effect::OpenedSettings);
        m.input(Input::NextTab, READY);
        assert_eq!(m.page, Page::Hub);
        m.input(Input::PreviousTab, READY);
        assert_eq!(m.page, Page::Settings);
    }

    #[test]
    fn top_bar_opens_pages() {
        let mut m = Model::new(GAMES.len(), true);
        m.focus = Focus::Tab(Tab::Controls);
        m.input(Input::Press, READY);
        assert_eq!(
            (m.page, m.focus),
            (Page::Controls, Focus::Tab(Tab::Controls))
        );
        m.input(Input::Right, READY);
        assert_eq!(m.input(Input::Press, READY), Effect::OpenedSettings);
        assert_eq!((m.page, m.focus), (Page::Settings, Focus::Item(0)));
    }

    #[test]
    fn settings_rows_adjust_and_end_in_library_actions() {
        let mut m = Model::new(GAMES.len(), true);
        m.open(Page::Settings);
        assert_eq!(
            m.input(Input::Right, READY),
            Effect::Adjust(Row::Output, 1, false)
        );
        assert_eq!(
            m.input(Input::Left, READY),
            Effect::Adjust(Row::Output, -1, false)
        );
        assert_eq!(
            m.input(Input::Press, READY),
            Effect::Adjust(Row::Output, 1, true)
        );
        m.input(Input::Up, READY);
        assert_eq!(m.focus, Focus::Tab(Tab::Settings));
        m.input(Input::Down, READY);
        for _ in 0..30 {
            m.input(Input::Down, READY);
        }
        assert_eq!(m.focus, Focus::Item(m.items.len() - 1));
        assert_eq!(m.items[m.items.len() - 2], Item::Rescan);
        m.input(Input::Up, READY);
        assert_eq!(m.input(Input::Press, READY), Effect::Rescan);
        m.input(Input::Down, READY);
        assert_eq!(m.input(Input::Press, READY), Effect::OpenedFolder);
        assert_eq!((m.page, m.focus), (Page::FirstRun, Focus::Field));
        assert_eq!(m.input(Input::Back, READY), Effect::OpenedSettings);
        assert_eq!(m.page, Page::Settings);
    }

    #[test]
    fn first_run_needs_a_folder_before_anything_else() {
        let none = World {
            playable: false,
            has_library: false,
        };
        let mut m = Model::new(GAMES.len(), false);
        assert_eq!((m.page, m.focus), (Page::FirstRun, Focus::Field));
        assert_eq!(m.input(Input::NextTab, none), Effect::None);
        assert_eq!(m.page, Page::FirstRun);
        assert_eq!(m.input(Input::Back, none), Effect::None);
        assert_eq!(m.input(Input::Press, none), Effect::SubmitFolder);
        m.input(Input::Down, none);
        assert_eq!(m.focus, Focus::UseFolder);
        assert_eq!(m.input(Input::Press, none), Effect::SubmitFolder);
        assert_eq!(m.input(Input::Escape, none), Effect::Quit);
    }

    #[test]
    fn tile_state_from_catalog_and_library() {
        let mt5 = catalog::by_slug("mt5").unwrap();
        let blocked = catalog::by_slug("wangan-r").unwrap();
        assert_eq!(TileState::of(mt5, false, false), TileState::Looking);
        assert_eq!(TileState::of(mt5, true, true), TileState::Ready);
        assert_eq!(TileState::of(mt5, true, false), TileState::NotInLibrary);
        // the N2 games are Linux programs: locked, with the reason, on Windows
        let mt3 = catalog::by_slug("mt3").unwrap();
        assert_eq!(
            TileState::of(mt3, true, true),
            if cfg!(windows) {
                TileState::Blocked(catalog::N2_LINUX_ONLY)
            } else {
                TileState::Ready
            }
        );
        assert!(matches!(
            TileState::of(blocked, true, true),
            TileState::Blocked(_)
        ));
        assert!(matches!(
            TileState::of(blocked, false, false),
            TileState::Blocked(_)
        ));
        assert_eq!(TileState::NotInLibrary.badge(), Some("Not found"));
        assert_eq!(TileState::Ready.badge(), None);
    }
}
