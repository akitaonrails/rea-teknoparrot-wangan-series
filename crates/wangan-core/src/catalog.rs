//! The nine games, what runs each one, and the builds we know.
//!
//! An edition is recognised by the SHA-256 of its main file. Only builds
//! listed here are launched; anything else is reported with its digest so
//! the player can file it (STANDARDS: "Exact binary identity").

use rea_launcher::catalog::{self, Edition, Title};
pub use rea_launcher::catalog::{Host, Support};
use rea_launcher::gamescope::Size;
use rea_launcher::library::{HashCache, Library};
use wangan_jvs::controls::Cabinet;

use crate::firmware::{CHIHIRO, FirmwareSet, SYS246};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameId {
    WanganR,
    Mt1,
    Mt3,
    Mt3dxp,
    Mt5,
    Mt5dxp,
    Mt6,
    Mt6r,
    Mt6rr,
}

/// How a game is run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Namco N2 Linux binary, natively, with WanganArcadeLoader preloaded.
    NativeN2,
    /// Namco ES3B Windows build under Wine with OpenParrot and our I/O board.
    Es3b,
    /// Sega Chihiro build in Cxbx-Reloaded under Wine.
    Chihiro,
    /// Namco System 246 in pcsx2x6.
    System246,
}

/// A game whose board files are in, but whose emulator the launcher
/// doesn't start yet on Linux (under Wine).
pub const EMULATOR_PENDING: &str = "Your board files are in. Under Wine the Chihiro emulator freezes in most runs of this game, so Play stays off for now.";

/// Why Chihiro games (`mt1`) stay blocked on Windows: no Chihiro emulator is
/// bundled for Windows in this launcher yet (never blame Wine on Windows).
pub const EMULATOR_PENDING_WINDOWS: &str = "Your board files are in. No Chihiro emulator is bundled for this game on Windows yet, so Play stays off for now.";

/// Why Wangan R is not offered on Windows yet.
pub const SYS246_LINUX_ONLY: &str =
    "Wangan Midnight R has only been set up and played in the Linux build of the launcher so far.";

/// Why the N2 games are not offered on Windows.
pub const N2_LINUX_ONLY: &str = "Maximum Tune 3 and 3DX+ are Linux programs (Namco N2 board); they run only in the Linux build of the launcher.";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Game {
    pub id: GameId,
    /// Stable id for files and the command line.
    pub slug: &'static str,
    pub title: &'static str,
    /// Short name on a tile.
    pub short: &'static str,
    pub year: u16,
    pub board: &'static str,
    /// Short trivia/context shown over the top-right quarter of the hero art.
    pub blurb: &'static str,
    pub route: Route,
    /// The file that identifies the edition, relative to the game folder.
    pub main_file: &'static str,
    pub editions: &'static [Edition],
    /// The size the game draws at when nothing changes it (the N2 games:
    /// WanganArcadeLoader's own; the others always draw at it).
    pub native: Size,
    pub cabinet: Cabinet,
    /// Board firmware the player must add (not part of the game dump).
    pub firmware: Option<&'static FirmwareSet>,
    pub support: Support,
}

impl Game {
    /// Whether the game can start on `host`, and why not.
    pub fn support_on(&self, host: Host) -> Support {
        match (self.support, self.route, host) {
            (Support::Ready, Route::NativeN2, Host::Windows) => Support::Blocked(N2_LINUX_ONLY),
            (Support::Ready, Route::System246, Host::Windows) => {
                Support::Blocked(SYS246_LINUX_ONLY)
            }
            (Support::Blocked(_), Route::Chihiro, Host::Windows) => {
                Support::Blocked(EMULATOR_PENDING_WINDOWS)
            }
            (s, _, _) => s,
        }
    }

    /// Whether the game draws at its own size whatever the settings say
    /// (only WanganArcadeLoader takes a render size).
    pub fn render_fixed(&self) -> bool {
        self.route != Route::NativeN2
    }
}

impl Title for Game {
    type Id = GameId;

    fn id(&self) -> GameId {
        self.id
    }

    fn main_file(&self) -> &'static str {
        self.main_file
    }

    fn editions(&self) -> &'static [Edition] {
        self.editions
    }
}

pub const GAMES: &[Game] = &[
    Game {
        id: GameId::WanganR,
        slug: "wangan-r",
        title: "Wangan Midnight R",
        short: "R",
        year: 2002,
        board: "Namco System 246",
        blurb: "Arcade follow-up to 2001's Wangan Midnight on PS2-based System 246 hardware — the last entry before Maximum Tune began.",
        route: Route::System246,
        main_file: "wanganmr.zip",
        editions: &[Edition::sha256(
            "4d78fe15299df5a10f63e697bbe1a620abe7080e69f8972e7315f1e3bff13ca8",
            "WMR1 Ver. A",
        )],
        native: Size::new(640, 480),
        cabinet: Cabinet::Wmmt5,
        firmware: Some(&SYS246),
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt1,
        slug: "mt1",
        title: "Wangan Midnight Maximum Tune",
        short: "MT",
        year: 2004,
        board: "Sega Chihiro",
        blurb: "The first Maximum Tune, built with Genki on Xbox-based Chihiro hardware. Introduced Story Mode, 800 HP tuning and magnetic cards.",
        route: Route::Chihiro,
        main_file: "V307.xbe",
        editions: &[Edition::sha256(
            "2d74371e5a0bb83a6b5185350792f7e3a4242076b62306fab103936ddd2dd760",
            "3.07 Rev B (Export)",
        )],
        native: Size::new(640, 480),
        cabinet: Cabinet::Wmmt5,
        firmware: Some(&CHIHIRO),
        support: Support::Blocked(EMULATOR_PENDING),
    },
    Game {
        id: GameId::Mt3,
        slug: "mt3",
        title: "Wangan Midnight Maximum Tune 3",
        short: "MT3",
        year: 2007,
        board: "Namco N2",
        blurb: "Moved the series to Linux-based N2 cabinets with widescreen 720p, Ghost Battles and the 6-speed gearbox.",
        route: Route::NativeN2,
        main_file: "main",
        editions: &[Edition::sha256(
            "0574cf963d8dd94788f247e65cc4da7e47f4b366588d1309fe869f4264a26475",
            "Export",
        )],
        native: Size::new(1280, 720),
        cabinet: Cabinet::Wmmt5,
        firmware: None,
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt3dxp,
        slug: "mt3dxp",
        title: "Wangan Midnight Maximum Tune 3DX+",
        short: "MT3DX+",
        year: 2010,
        board: "Namco N2",
        blurb: "Final N2 release and last magnetic-card cabinet. Added the R35 GT-R, Fukuoka Urban Expressway and expanded Ghost Battles.",
        route: Route::NativeN2,
        main_file: "main",
        editions: &[Edition::sha256(
            "3dc7cc6174806fe4ea6687625c233ed5468e0225e3bcce15e225b25b2934be5b",
            "Export",
        )],
        native: Size::new(1280, 720),
        cabinet: Cabinet::Wmmt5,
        firmware: None,
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt5,
        slug: "mt5",
        title: "Wangan Midnight Maximum Tune 5",
        short: "MT5",
        year: 2014,
        board: "Namco ES3B",
        blurb: "Rebuilt on Windows-based ES3B hardware with Banapassport IC cards, 60-chapter Story Mode, Extreme VS and Mt. Taikan.",
        route: Route::Es3b,
        main_file: "wmn5r.exe",
        editions: &[Edition::sha256(
            "4701c88d3598d56c31055fdaba98ec1c17e3455e96ad701abebc65eba6105c3e",
            "5.00 (Japan)",
        )],
        native: Size::new(1360, 768),
        cabinet: Cabinet::Wmmt5,
        firmware: None,
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt5dxp,
        slug: "mt5dxp",
        title: "Wangan Midnight Maximum Tune 5DX+",
        short: "MT5DX+",
        year: 2017,
        board: "Namco ES3B",
        blurb: "Final 5-series update: added Hiroshima Expressway, Lamborghini and Honda (NSX, S2000), and tuning up to 830 HP.",
        route: Route::Es3b,
        main_file: "wmn5r.exe",
        editions: &[Edition::sha256(
            "738e88b4d03ef6ab84208464d470e69557a8591cf5a293570badb1dfdcbc3b2a",
            "Export",
        )],
        native: Size::new(1360, 768),
        cabinet: Cabinet::Wmmt5,
        firmware: None,
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt6,
        slug: "mt6",
        title: "Wangan Midnight Maximum Tune 6",
        short: "MT6",
        year: 2018,
        board: "Namco ES3B",
        blurb: "Major overhaul: 100-chapter Story Mode, 840 HP cap, 4-player Ghost Battles, Porsche joining the roster, and a new terminal system.",
        route: Route::Es3b,
        main_file: "wmn6r.exe",
        editions: &[Edition::sha256(
            "92f02199a44fa65a35af3ed162b5ce5477cfc8b2e3a13ccc95936356680f1479",
            "1.03.04 (Export)",
        )],
        native: Size::new(1360, 768),
        cabinet: Cabinet::Wmmt5,
        firmware: None,
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt6r,
        slug: "mt6r",
        title: "Wangan Midnight Maximum Tune 6R",
        short: "MT6R",
        year: 2020,
        board: "Namco ES3B",
        blurb: "Revision of 6 that refreshed the roster (GR Supra, Silvia Spec-R), rebalanced physics and tuning, and updated the soundtrack.",
        route: Route::Es3b,
        main_file: "wmn6r.exe",
        editions: &[Edition::sha256(
            "bf9fded65f5d029deaed5c3b5d06e8035dae70eee2e03c51e3adc2b1444ae2be",
            "Japan",
        )],
        native: Size::new(1360, 768),
        cabinet: Cabinet::Wmmt6RR,
        firmware: None,
        support: Support::Ready,
    },
    Game {
        id: GameId::Mt6rr,
        slug: "mt6rr",
        title: "Wangan Midnight Maximum Tune 6RR",
        short: "MT6RR",
        year: 2022,
        board: "Namco ES3B",
        blurb: "Latest 6-series update over 6R: adds the Bingo Challenge, new Dress-Up parts, Time Attack rankings, and modern cars like the GR Yaris.",
        route: Route::Es3b,
        main_file: "wmn6r.exe",
        editions: &[Edition::sha256(
            "9203e373b73a522c3ca213df9add0e3d4cce3f81aaba8c48ce5e46c132932d4b",
            "3.05.03 (Japan)",
        )],
        native: Size::new(1360, 768),
        cabinet: Cabinet::Wmmt6RR,
        firmware: None,
        support: Support::Ready,
    },
];

pub fn game(id: GameId) -> &'static Game {
    catalog::by_id(GAMES, id).expect("every id is in GAMES")
}

pub fn by_slug(slug: &str) -> Option<&'static Game> {
    GAMES.iter().find(|g| g.slug == slug)
}

/// The library scan over these games, sharing `cache` with every other scan.
pub fn library(cache: HashCache) -> Library<Game> {
    Library::new(GAMES).with_cache(cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_slugs_and_digests_are_unique() {
        assert_eq!(catalog::check_table(GAMES), Ok(()));
        let slugs: HashSet<_> = GAMES.iter().map(|g| g.slug).collect();
        assert_eq!(slugs.len(), GAMES.len());
    }

    #[test]
    fn identify_finds_the_right_game() {
        let (g, e) = catalog::identify(
            GAMES,
            "3DC7CC6174806FE4EA6687625C233ED5468E0225E3BCCE15E225B25B2934BE5B",
        )
        .unwrap();
        assert_eq!(g.id, GameId::Mt3dxp);
        assert_eq!(e.label, "Export");
        assert!(catalog::identify(GAMES, "00").is_none());
    }

    #[test]
    fn six_r_family_uses_the_narrow_wheel_range() {
        assert_eq!(game(GameId::Mt6r).cabinet, Cabinet::Wmmt6RR);
        assert_eq!(game(GameId::Mt6rr).cabinet, Cabinet::Wmmt6RR);
        assert_eq!(game(GameId::Mt5dxp).cabinet, Cabinet::Wmmt5);
    }

    #[test]
    fn n2_games_are_linux_only() {
        let ready_on = |h: Host| -> Vec<&str> {
            GAMES
                .iter()
                .filter(|g| g.support_on(h) == Support::Ready)
                .map(|g| g.slug)
                .collect()
        };
        assert_eq!(
            ready_on(Host::Windows),
            ["mt5", "mt5dxp", "mt6", "mt6r", "mt6rr"]
        );
        assert_eq!(
            game(GameId::Mt3).support_on(Host::Windows),
            Support::Blocked(N2_LINUX_ONLY)
        );
        assert_eq!(game(GameId::Mt3dxp).support_on(Host::Linux), Support::Ready);
        // mt1 is blocked on both platforms, with a Windows-specific reason (never blaming Wine on Windows)
        assert_eq!(
            game(GameId::Mt1).support_on(Host::Linux),
            Support::Blocked(EMULATOR_PENDING)
        );
        assert_eq!(
            game(GameId::Mt1).support_on(Host::Windows),
            Support::Blocked(EMULATOR_PENDING_WINDOWS)
        );
    }

    #[test]
    fn only_verified_games_are_ready() {
        let ready: Vec<_> = GAMES
            .iter()
            .filter(|g| g.support == Support::Ready)
            .map(|g| g.slug)
            .collect();
        assert_eq!(
            ready,
            [
                "wangan-r", "mt3", "mt3dxp", "mt5", "mt5dxp", "mt6", "mt6r", "mt6rr"
            ]
        );
    }

    #[test]
    fn only_the_n2_games_take_a_render_size() {
        let takes: Vec<_> = GAMES
            .iter()
            .filter(|g| !g.render_fixed())
            .map(|g| g.slug)
            .collect();
        assert_eq!(takes, ["mt3", "mt3dxp"]);
    }

    #[test]
    fn a_stray_main_file_never_hides_the_game_beside_it() {
        let t = tempfile::tempdir().unwrap();
        let g = t.path().join("MT6 [TP]");
        std::fs::create_dir(&g).unwrap();
        // an N2 main file left over in an ES3B folder, and the ES3B program
        std::fs::write(g.join("main"), b"not the game").unwrap();
        std::fs::write(g.join("wmn6r.exe"), b"not the game either").unwrap();
        let scan = library(HashCache::default()).scan(&[t.path().to_path_buf()]);
        let files: Vec<_> = scan
            .entries
            .iter()
            .map(|e| e.file.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, ["main", "wmn6r.exe"], "both are reported");
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
    }

    /// The owner's real library, when present (`WANGAN_GAMES_DIR=… cargo test -- --ignored`).
    #[test]
    #[ignore]
    fn real_library_finds_all_nine() {
        let Ok(root) = std::env::var("WANGAN_GAMES_DIR") else {
            return;
        };
        let scan = library(HashCache::default()).scan(&[root.into()]);
        assert_eq!(scan.by_game().len(), 9, "{:#?}", scan.entries);
    }
}
