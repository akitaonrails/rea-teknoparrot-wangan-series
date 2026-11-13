//! The nine games, what runs each one, and the builds we know.
//!
//! An edition is recognised by the SHA-256 of its main file. Only builds
//! listed here are launched; anything else is reported with its digest so
//! the player can file it (STANDARDS: "Exact binary identity").

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
    pub support: Support,
}

const ES3B_BLOCKED_BUILD: &str =
    "Needs the OpenParrot build with this launcher's fix; not shipped yet.";

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
        support: Support::Blocked("Needs a Namco System 246 BIOS, which is not in your library."),
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
        support: Support::Blocked(
            "Needs the four Chihiro media-board files (ic10_g24lc64.bin, pc20_g24lc64.bin, ic11_24lc024.bin, fpr21042_m29w160et.bin), which are not in your library.",
        ),
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
        support: Support::Blocked(ES3B_BLOCKED_BUILD),
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
        support: Support::Blocked(ES3B_BLOCKED_BUILD),
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
    fn only_verified_games_are_ready() {
        let ready: Vec<_> = GAMES
            .iter()
            .filter(|g| g.support == Support::Ready)
            .map(|g| g.slug)
            .collect();
        assert_eq!(ready, ["mt3", "mt3dxp", "mt5", "mt5dxp", "mt6r"]);
    }
}
