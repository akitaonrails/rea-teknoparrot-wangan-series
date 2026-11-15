//! The nine games, what runs each one, and the builds we know.
//!
//! An edition is recognised by the SHA-256 of its main file. Only builds
//! listed here are launched; anything else is reported with its digest so
//! the player can file it (STANDARDS: "Exact binary identity").

use crate::firmware::{CHIHIRO, FirmwareSet, SYS246};
use wangan_jvs::controls::Cabinet;

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

/// Whether the launcher can start a game today, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Verified in the real game (a race played), see docs/RUNTIME_NOTES.md.
    Ready,
    /// Known not to work yet; the text says why, for the player.
    Blocked(&'static str),
}

/// The system the launcher runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Wine and gamescope for the Windows games, the N2 games natively.
    Linux,
    /// The Windows games directly; no N2 games.
    Windows,
}

impl Platform {
    /// The platform this build of the launcher is for.
    pub const HOST: Platform = if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    };
}

/// A game whose board files are in, but whose emulator the launcher
/// doesn't start yet.
pub const EMULATOR_PENDING: &str = "Your board files are in. Starting this game from the launcher hasn't been tested yet, so Play stays off for now.";

/// Why Wangan R is not offered on Windows yet.
pub const SYS246_LINUX_ONLY: &str =
    "Wangan Midnight R has only been set up and played in the Linux build of the launcher so far.";

/// Why the N2 games are not offered on Windows.
pub const N2_LINUX_ONLY: &str = "Maximum Tune 3 and 3DX+ are Linux programs (Namco N2 board); they run only in the Linux build of the launcher.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edition {
    pub sha256: &'static str,
    pub label: &'static str,
}

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
    pub route: Route,
    /// The file that identifies the edition, relative to the game folder.
    pub main_file: &'static str,
    pub editions: &'static [Edition],
    pub cabinet: Cabinet,
    /// Board firmware the player must add (not part of the game dump).
    pub firmware: Option<&'static FirmwareSet>,
    pub support: Support,
}

impl Game {
    /// Whether the game can start on `platform`, and why not.
    pub fn support_on(&self, platform: Platform) -> Support {
        match (self.support, self.route, platform) {
            (Support::Ready, Route::NativeN2, Platform::Windows) => Support::Blocked(N2_LINUX_ONLY),
            (Support::Ready, Route::System246, Platform::Windows) => {
                Support::Blocked(SYS246_LINUX_ONLY)
            }
            (s, _, _) => s,
        }
    }

    /// [`Game::support_on`] for the platform this launcher was built for.
    pub fn support_here(&self) -> Support {
        self.support_on(Platform::HOST)
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
        route: Route::System246,
        main_file: "wanganmr.zip",
        editions: &[Edition {
            sha256: "4d78fe15299df5a10f63e697bbe1a620abe7080e69f8972e7315f1e3bff13ca8",
            label: "WMR1 Ver. A",
        }],
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
        route: Route::Chihiro,
        main_file: "V307.xbe",
        editions: &[Edition {
            sha256: "2d74371e5a0bb83a6b5185350792f7e3a4242076b62306fab103936ddd2dd760",
            label: "3.07 Rev B (Export)",
        }],
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
        route: Route::NativeN2,
        main_file: "main",
        editions: &[Edition {
            sha256: "0574cf963d8dd94788f247e65cc4da7e47f4b366588d1309fe869f4264a26475",
            label: "Export",
        }],
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
        route: Route::NativeN2,
        main_file: "main",
        editions: &[Edition {
            sha256: "3dc7cc6174806fe4ea6687625c233ed5468e0225e3bcce15e225b25b2934be5b",
            label: "Export",
        }],
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
        route: Route::Es3b,
        main_file: "wmn5r.exe",
        editions: &[Edition {
            sha256: "4701c88d3598d56c31055fdaba98ec1c17e3455e96ad701abebc65eba6105c3e",
            label: "5.00 (Japan)",
        }],
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
        route: Route::Es3b,
        main_file: "wmn5r.exe",
        editions: &[Edition {
            sha256: "738e88b4d03ef6ab84208464d470e69557a8591cf5a293570badb1dfdcbc3b2a",
            label: "Export",
        }],
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
        route: Route::Es3b,
        main_file: "wmn6r.exe",
        editions: &[Edition {
            sha256: "92f02199a44fa65a35af3ed162b5ce5477cfc8b2e3a13ccc95936356680f1479",
            label: "1.03.04 (Export)",
        }],
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
        route: Route::Es3b,
        main_file: "wmn6r.exe",
        editions: &[Edition {
            sha256: "bf9fded65f5d029deaed5c3b5d06e8035dae70eee2e03c51e3adc2b1444ae2be",
            label: "Japan",
        }],
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
        route: Route::Es3b,
        main_file: "wmn6r.exe",
        editions: &[Edition {
            sha256: "9203e373b73a522c3ca213df9add0e3d4cce3f81aaba8c48ce5e46c132932d4b",
            label: "3.05.03 (Japan)",
        }],
        cabinet: Cabinet::Wmmt6RR,
        firmware: None,
        support: Support::Ready,
    },
];

pub fn game(id: GameId) -> &'static Game {
    GAMES
        .iter()
        .find(|g| g.id == id)
        .expect("every id is in GAMES")
}

pub fn by_slug(slug: &str) -> Option<&'static Game> {
    GAMES.iter().find(|g| g.slug == slug)
}

/// Every game whose main file has this name (several share `main` or `wmn6r.exe`).
pub fn by_main_file(name: &str) -> impl Iterator<Item = &'static Game> + '_ {
    GAMES.iter().filter(move |g| g.main_file == name)
}

/// The game and edition a main file's digest belongs to.
pub fn identify(sha256: &str) -> Option<(&'static Game, &'static Edition)> {
    GAMES.iter().find_map(|g| {
        g.editions
            .iter()
            .find(|e| e.sha256.eq_ignore_ascii_case(sha256))
            .map(|e| (g, e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_slugs_and_digests_are_unique() {
        let ids: HashSet<_> = GAMES.iter().map(|g| g.id).collect();
        let slugs: HashSet<_> = GAMES.iter().map(|g| g.slug).collect();
        let shas: HashSet<_> = GAMES
            .iter()
            .flat_map(|g| g.editions.iter().map(|e| e.sha256))
            .collect();
        assert_eq!(ids.len(), GAMES.len());
        assert_eq!(slugs.len(), GAMES.len());
        assert_eq!(
            shas.len(),
            GAMES.iter().map(|g| g.editions.len()).sum::<usize>()
        );
        for s in &shas {
            assert_eq!(s.len(), 64);
            assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn identify_finds_the_right_game() {
        let (g, e) =
            identify("3DC7CC6174806FE4EA6687625C233ED5468E0225E3BCCE15E225B25B2934BE5B").unwrap();
        assert_eq!(g.id, GameId::Mt3dxp);
        assert_eq!(e.label, "Export");
        assert!(identify("00").is_none());
    }

    #[test]
    fn six_r_family_uses_the_narrow_wheel_range() {
        assert_eq!(game(GameId::Mt6r).cabinet, Cabinet::Wmmt6RR);
        assert_eq!(game(GameId::Mt6rr).cabinet, Cabinet::Wmmt6RR);
        assert_eq!(game(GameId::Mt5dxp).cabinet, Cabinet::Wmmt5);
    }

    #[test]
    fn n2_games_are_linux_only() {
        let ready_on = |p: Platform| -> Vec<&str> {
            GAMES
                .iter()
                .filter(|g| g.support_on(p) == Support::Ready)
                .map(|g| g.slug)
                .collect()
        };
        assert_eq!(
            ready_on(Platform::Windows),
            ["mt5", "mt5dxp", "mt6", "mt6r", "mt6rr"]
        );
        assert_eq!(
            game(GameId::Mt3).support_on(Platform::Windows),
            Support::Blocked(N2_LINUX_ONLY)
        );
        assert_eq!(
            game(GameId::Mt3dxp).support_on(Platform::Linux),
            Support::Ready
        );
        // a game blocked everywhere keeps its own reason
        assert_eq!(
            game(GameId::Mt1).support_on(Platform::Windows),
            game(GameId::Mt1).support
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
}
