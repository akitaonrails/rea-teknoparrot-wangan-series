//! Per-game settings, stored as `key = value` lines in
//! `<data>/settings/<slug>.cfg`.
//!
//! [`OPTIONS`] names every setting once (`rea_launcher::cfg` option specs):
//! its key, its flag, the values it takes, and the label and description
//! the window and `--help` show. Each key keeps the format earlier versions
//! wrote (`fullscreen = true`, `deadzone = 0.05`, `render = native`). A
//! save writes only the keys that changed, so the player's comments,
//! unknown keys and any line the launcher couldn't read stay as they were;
//! an unreadable file is never written over.

use std::io;
use std::path::Path;

use rea_launcher::cfg::{self, KeyStyle, Kind, OptionSpec, Rejected, Resolution, Value};
use rea_launcher::gamescope::{self, Crt, SIZE_MAX, SIZE_MIN, Size, UPSCALERS, Upscaler, fit};
use rea_launcher::{display, fsx};

use crate::catalog::Game;

/// Flags are the keys: `--render` is `render`.
pub const STYLE: KeyStyle = KeyStyle::Underscore;

/// A look: a named set of picture values (the render size, the size on
/// the screen, the upscaler).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Look {
    /// The game as it draws by itself: its own size, smooth scaling.
    Original,
    /// Drawn at the player's screen size in the game's shape, FSR to fill
    /// the rest.
    Recommended,
    /// The Steam Deck's 1280×800 screen.
    SteamDeck,
}

pub const LOOKS: &[(&str, &str)] = &[
    ("original", "Original"),
    ("recommended", "Recommended"),
    ("steam_deck", "Steam Deck"),
];

/// The keys a look sets.
pub const LOOK_KEYS: [&str; 4] = ["look", "render", "output", "upscaler"];

/// Every settings key.
pub const ALL_KEYS: [&str; 7] = [
    "look",
    "output",
    "upscaler",
    "sharpness",
    "fullscreen",
    "render",
    "deadzone",
];

impl Look {
    pub const ALL: [Look; 3] = [Look::Original, Look::Recommended, Look::SteamDeck];

    pub fn key(self) -> &'static str {
        LOOKS[self as usize].0
    }

    pub fn label(self) -> &'static str {
        LOOKS[self as usize].1
    }

    pub fn from_key(key: &str) -> Option<Look> {
        Look::ALL.into_iter().find(|l| l.key() == key)
    }

    /// The look for `screen` (the monitor in pixels, when known): Steam
    /// Deck on a Deck's screen.
    pub fn recommended(screen: Option<(u32, u32)>) -> Look {
        if display::is_steam_deck(screen) {
            Look::SteamDeck
        } else {
            Look::Recommended
        }
    }

    /// Sets this look's values in `s` for `game` on `screen` (1920×1080
    /// when unknown). A game that draws at its own size keeps no render
    /// size.
    pub fn apply(self, s: &mut Settings, game: &Game, screen: Option<(u32, u32)>) {
        let screen = screen
            .and_then(display::landscape)
            .map_or(Size::new(1920, 1080), |(w, h)| Size::new(w, h));
        let native = game.native;
        let (render, output, upscaler) = match self {
            Look::Original => (native, screen, Upscaler::Linear),
            Look::Recommended => (fit(native, screen), screen, Upscaler::Fsr),
            Look::SteamDeck => {
                let (w, h) = display::STEAM_DECK_SCREEN;
                let deck = Size::new(w, h);
                (fit(native, deck), deck, Upscaler::Linear)
            }
        };
        s.render = (!game.render_fixed()).then_some(render);
        s.output = output;
        s.upscaler = upscaler;
        s.look = Some(self);
    }
}

const NATIVE: &cfg::Aliases = &[("native", Resolution::Original)];

/// Every per-game option: settings key, flag and window row.
pub const OPTIONS: &[OptionSpec] = &[
    OptionSpec::new(
        "look",
        Kind::Choice(LOOKS),
        "Look",
        "Sets the picture at once: the game's own, sharper for your screen, or for the \
         Steam Deck. Options set after it win.",
    ),
    OptionSpec::new(
        "output",
        Kind::Size {
            min: SIZE_MIN,
            max: SIZE_MAX,
            aliases: cfg::SIZES_ONLY,
        },
        "Output size",
        "The size of the picture on your screen. Pick your screen's size.",
    )
    .with_example("3840x2160")
    .helper_owned(),
    OptionSpec::new(
        "upscaler",
        Kind::Choice(UPSCALERS),
        "Upscaler",
        "How the game's picture is stretched to the output size.",
    )
    .helper_owned(),
    OptionSpec::new(
        "sharpness",
        Kind::Range { min: 0, max: 20 },
        "Sharpness",
        "How crisp the upscaler makes edges. 0 is the sharpest.",
    )
    .helper_owned(),
    OptionSpec::new(
        "fullscreen",
        Kind::Toggle,
        "Fullscreen",
        "Fill the whole screen, or play in a window.",
    )
    .helper_owned(),
    OptionSpec::new(
        "render",
        Kind::Size {
            min: SIZE_MIN,
            max: SIZE_MAX,
            aliases: NATIVE,
        },
        "Game render size",
        "The size the game draws at. Bigger is sharper and needs a stronger GPU.",
    )
    .with_placeholder("<WxH|native>")
    .with_example("3840x2160"),
    OptionSpec::new(
        "deadzone",
        Kind::Decimal {
            min: 0,
            max: 30,
            places: 2,
        },
        "Dead zone",
        "How far a controller stick moves before the car starts to steer.",
    )
    .with_example("0.05"),
];

/// Comment lines at the top of a new settings file.
pub const HEADING: &str = "Settings for one game of the Wangan Midnight launcher.\n\
    Every key is also a command-line option: render = 2560x1440 is \
    --set <game> --render 2560x1440 (or --play <game> --render 2560x1440).";

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The look last chosen (its values are in the fields below); `None`
    /// once a picture value was changed by hand.
    pub look: Option<Look>,
    /// The game's own render size (N2 games only: the loader patches it);
    /// `None`: the output size.
    pub render: Option<Size>,
    /// What gamescope shows on the screen.
    pub output: Size,
    pub upscaler: Upscaler,
    /// 0–20, gamescope's FSR/NIS sharpness (0 = sharpest).
    pub sharpness: u8,
    pub fullscreen: bool,
    /// Pad stick dead zone around the centre, 0.0–0.3 (all games).
    pub deadzone: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            look: None,
            render: None,
            output: Size::new(3840, 2160),
            upscaler: Upscaler::Fsr,
            sharpness: 5,
            fullscreen: true,
            deadzone: 0.05,
        }
    }
}

impl Settings {
    /// Sets one key from a value its row checked (the file, the command
    /// line and the window all come through here).
    pub fn apply(&mut self, key: &str, value: Value) -> Result<(), Rejected> {
        match (key, value) {
            ("look", Value::Choice(v)) => self.look = Look::from_key(v),
            ("render", Value::Size(Resolution::Size(w, h))) => self.render = Some(Size::new(w, h)),
            ("render", Value::Size(_)) => self.render = None,
            ("output", Value::Size(Resolution::Size(w, h))) => self.output = Size::new(w, h),
            ("upscaler", Value::Choice(v)) => {
                self.upscaler = Upscaler::parse(v).ok_or(Rejected::Unknown)?
            }
            ("sharpness", Value::Number(n)) => self.sharpness = n as u8,
            ("fullscreen", Value::Toggle(b)) => self.fullscreen = b,
            ("deadzone", Value::Decimal(d)) => self.deadzone = d.as_f32(),
            _ => return Err(Rejected::Unknown),
        }
        Ok(())
    }

    /// Sets `key` from its text, checked against its row.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Rejected> {
        cfg::spec::dispatch(OPTIONS, key, value, &mut |k, v| self.apply(k, v))
    }

    /// Reads a settings file's text; a bad value keeps the default and is
    /// reported.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut s = Settings::default();
        let problems = cfg::spec::parse_file(text, "settings", OPTIONS, |k, v| s.apply(k, v));
        (s, problems)
    }

    /// The file value of `key`, in the format earlier versions wrote
    /// (`look`: empty when there is none).
    pub fn value(&self, key: &str) -> String {
        match key {
            "look" => self.look.map_or("", Look::key).into(),
            "render" => self.render.map_or("native".into(), |z| z.to_string()),
            "output" => self.output.to_string(),
            "upscaler" => self.upscaler.key().into(),
            "sharpness" => self.sharpness.to_string(),
            "fullscreen" => self.fullscreen.to_string(),
            "deadzone" => format!("{:.2}", self.deadzone),
            _ => unreachable!("unknown settings key {key}"),
        }
    }

    /// `existing` with `keys` set to these settings' values, every other
    /// line as it was (no look: its line is cleared); a new file starts
    /// from the commented template.
    pub fn save(&self, existing: &str, keys: &[&str]) -> String {
        let mut out = if existing.trim().is_empty() {
            template()
        } else {
            existing.to_string()
        };
        for spec in OPTIONS.iter().filter(|s| keys.contains(&s.key)) {
            out = match self.value(spec.key) {
                v if v.is_empty() => cfg::clear_key(&out, spec.key),
                v => cfg::spec::set_key(&out, spec, &v),
            };
        }
        out
    }

    /// The settings in `file` for `game`, and what couldn't be read in it;
    /// an unreadable file is an error. While no picture was chosen (no
    /// look, render size, output size or upscaler in the file) the
    /// recommended look for `screen` is used; it is saved only when the
    /// screen is known, so Play or `--play` before the settings page ever
    /// opened uses it, and a run that can't see the screen (the Linux
    /// command line) leaves the choice to the window.
    pub fn load(
        file: &Path,
        game: &Game,
        screen: Option<(u32, u32)>,
    ) -> io::Result<(Settings, Vec<String>)> {
        let text = cfg::read_file(file)?.unwrap_or_default();
        let (mut s, mut problems) = Settings::parse(&text);
        if cfg::entries(&text).any(|e| LOOK_KEYS.contains(&e.key)) {
            return Ok((s, problems));
        }
        Look::recommended(screen).apply(&mut s, game, screen);
        if screen.is_some()
            && let Err(e) = s.write(file, &LOOK_KEYS)
        {
            problems.push(format!(
                "couldn't save the recommended look to {}: {e}",
                file.display()
            ));
        }
        Ok((s, problems))
    }

    /// Writes `keys` into `file` ([`Settings::save`]) as a whole-file swap;
    /// a file that can't be read is left alone and the error returned.
    pub fn write(&self, file: &Path, keys: &[&str]) -> io::Result<()> {
        let existing = cfg::read_file(file)?.unwrap_or_default();
        fsx::write_atomic(file, self.save(&existing, keys))
    }

    /// The picture gamescope shows of a game drawn at `game`.
    pub fn picture(&self, game: Size) -> gamescope::Picture {
        gamescope::Picture {
            game,
            output: self.output,
            upscaler: self.upscaler,
            sharpness: self.sharpness,
            fullscreen: self.fullscreen,
            crt: Crt::Off,
        }
    }
}

/// The commented file a new settings file starts from.
pub fn template() -> String {
    cfg::spec::template(HEADING, OPTIONS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};

    #[test]
    fn round_trip_keeps_comments_and_unknown_keys() {
        let text = "# my notes\nupscaler = nis\nfuture_key = 7\noutput=2560x1440\n";
        let (s, problems) = Settings::parse(text);
        assert_eq!(problems.len(), 1, "only the unknown key: {problems:?}");
        assert_eq!(s.upscaler, Upscaler::Nis);
        assert_eq!(s.output, Size::new(2560, 1440));
        let saved = s.save(text, &["render", "output"]);
        assert_eq!(
            saved,
            "# my notes\nupscaler = nis\nfuture_key = 7\noutput = 2560x1440\nrender = native\n"
        );
        assert_eq!(Settings::parse(&saved).0, s);
    }

    #[test]
    fn bad_values_fall_back_and_are_reported() {
        let (s, p) =
            Settings::parse("output = 99x99\nsharpness = 50\nfullscreen = maybe\nnonsense");
        assert_eq!(s, Settings::default());
        // a line without `=` is no setting: kept, not reported
        assert_eq!(p.len(), 3, "{p:?}");
    }

    #[test]
    fn a_bad_value_stays_as_the_player_wrote_it_unless_set() {
        let text = "fullscreen = off # trailing\ndeadzone = 0.123\n";
        let (mut s, problems) = Settings::parse(text);
        assert_eq!(problems.len(), 1, "{problems:?}");
        s.sharpness = 3;
        assert_eq!(
            s.save(text, &["sharpness"]),
            "fullscreen = off # trailing\ndeadzone = 0.123\nsharpness = 3\n",
            "neither the unread line nor 0.123's precision is rewritten"
        );
        s.fullscreen = false;
        assert!(
            s.save(text, &["fullscreen"])
                .starts_with("fullscreen = false\n")
        );
    }

    #[test]
    fn each_key_keeps_its_stored_format() {
        let mut s = Settings::default();
        for (key, value) in [
            ("fullscreen", "off"),
            ("deadzone", "0.1"),
            ("render", "NATIVE"),
            ("upscaler", "NIS"),
        ] {
            s.set(key, value).unwrap();
        }
        assert_eq!(s.value("fullscreen"), "false");
        assert_eq!(s.value("deadzone"), "0.10");
        assert_eq!(s.value("render"), "native");
        assert_eq!(s.value("upscaler"), "nis");
        assert!(s.set("deadzone", "0.5").is_err());
    }

    #[test]
    fn a_new_file_is_the_template_with_every_key() {
        let mut s = Settings::default();
        Look::Original.apply(&mut s, game(GameId::Mt3), None);
        let text = s.save("", &ALL_KEYS);
        assert!(text.starts_with("# Settings for one game"), "{text}");
        for spec in OPTIONS {
            assert!(
                text.contains(&format!("\n{} = {}\n", spec.key, s.value(spec.key))),
                "{}",
                spec.key
            );
        }
        assert_eq!(Settings::parse(&text), (s, vec![]));
    }

    #[test]
    fn an_unreadable_file_is_not_written_over() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("mt3.cfg");
        std::fs::create_dir(&file).unwrap();
        assert!(Settings::load(&file, game(GameId::Mt3), None).is_err());
        assert!(Settings::default().write(&file, &["render"]).is_err());
        assert!(file.is_dir());
    }

    #[test]
    fn looks_set_the_picture_for_the_screen() {
        let mt3 = game(GameId::Mt3);
        let mut s = Settings::default();
        Look::Recommended.apply(&mut s, mt3, Some((3840, 2160)));
        assert_eq!(
            (s.render, s.output, s.upscaler),
            (
                Some(Size::new(3840, 2160)),
                Size::new(3840, 2160),
                Upscaler::Fsr
            ),
            "the N2 games draw at 4K on a 4K screen (docs/RUNTIME_NOTES.md)"
        );
        Look::SteamDeck.apply(&mut s, mt3, Some((3840, 2160)));
        assert_eq!(
            (s.render, s.output, s.upscaler),
            (
                Some(Size::new(1280, 720)),
                Size::new(1280, 800),
                Upscaler::Linear
            )
        );
        Look::Original.apply(&mut s, mt3, Some((2560, 1440)));
        assert_eq!(
            (s.render, s.output, s.upscaler),
            (
                Some(Size::new(1280, 720)),
                Size::new(2560, 1440),
                Upscaler::Linear
            ),
            "WanganArcadeLoader's own size, smooth"
        );
        assert_eq!(s.look, Some(Look::Original));
        Look::Recommended.apply(&mut s, game(GameId::Mt6), Some((2560, 1440)));
        assert_eq!(s.render, None, "an ES3B game draws at its own size");
        assert_eq!(Look::recommended(Some((800, 1280))), Look::SteamDeck);
        assert_eq!(Look::recommended(None), Look::Recommended);
    }

    #[test]
    fn a_look_given_up_is_cleared_from_the_file() {
        let mut s = Settings::default();
        Look::Original.apply(&mut s, game(GameId::Mt3), None);
        let text = s.save("", &ALL_KEYS);
        assert!(text.contains("\nlook = original\n"), "{text}");
        s.look = None;
        let text = s.save(&text, &LOOK_KEYS);
        assert!(!text.contains("\nlook ="), "{text}");
        assert_eq!(Settings::parse(&text).0.look, None);
    }

    #[test]
    fn the_first_load_saves_the_recommended_look_for_a_known_screen() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("settings/mt3.cfg");
        let (s, problems) = Settings::load(&file, game(GameId::Mt3), Some((1280, 800))).unwrap();
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(s.look, Some(Look::SteamDeck));
        let (again, _) = Settings::load(&file, game(GameId::Mt3), Some((3840, 2160))).unwrap();
        assert_eq!(
            again, s,
            "a second load reads the file, whatever the screen"
        );
    }

    #[test]
    fn with_no_screen_known_the_look_is_this_runs_only() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("settings/mt3.cfg");
        let mt3 = game(GameId::Mt3);
        let (s, _) = Settings::load(&file, mt3, None).unwrap();
        assert_eq!(s.look, Some(Look::Recommended));
        assert_eq!(
            s.output,
            Size::new(1920, 1080),
            "a 1080p screen for this run"
        );
        assert!(!file.exists(), "nothing saved: the window decides later");
        // a file the command line wrote without picture keys: still undecided
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "# mine\ndeadzone = 0.10\n").unwrap();
        let (s, _) = Settings::load(&file, mt3, Some((2560, 1440))).unwrap();
        assert_eq!(
            (s.look, s.output),
            (Some(Look::Recommended), Size::new(2560, 1440))
        );
        assert!((s.deadzone - 0.10).abs() < 1e-6);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# mine\ndeadzone = 0.10\n"), "{text}");
        assert!(text.contains("look = recommended\n"), "{text}");
        // a picture chosen by hand is never replaced by a look
        std::fs::write(&file, "output = 3840x2160\n").unwrap();
        let (s, _) = Settings::load(&file, mt3, Some((1280, 800))).unwrap();
        assert_eq!((s.look, s.output), (None, Size::new(3840, 2160)));
    }

    #[test]
    fn a_new_file_gets_the_keys_given_and_the_rest_commented() {
        let s = Settings {
            deadzone: 0.2,
            ..Settings::default()
        };
        let text = s.save("", &["deadzone"]);
        assert!(text.starts_with("# Settings for one game"), "{text}");
        assert!(text.contains("\ndeadzone = 0.20\n"), "{text}");
        assert!(text.contains("\n#output ="), "{text}");
        assert!(!text.contains("\noutput ="), "{text}");
    }

    #[test]
    fn the_template_lists_every_option_commented() {
        let keys: Vec<&str> = OPTIONS.iter().map(|o| o.key).collect();
        assert_eq!(keys, ALL_KEYS);
        let t = template();
        for spec in OPTIONS {
            assert!(t.contains(&format!("#{} =", spec.key)), "{}", spec.key);
        }
        assert_eq!(Settings::parse(&t).0, Settings::default());
    }
}
