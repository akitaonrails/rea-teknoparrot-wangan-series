//! A game's settings page as data (the hub's [`HubForm`]): what each row
//! shows, why one is off, and how the arrows change the [`Settings`]
//! behind it. Labels and descriptions come from the option table
//! (`wangan_core::settings::OPTIONS`), which the hub draws the rows from,
//! so the window, `--help` and the settings file say the same thing. The
//! Look row sets the picture rows at once; changing one of those by hand
//! makes the look "Custom". A save writes only the rows the player changed.

use std::io;
use std::path::Path;

use rea_launcher::catalog::Host;
use rea_launcher::gamescope::{Size, Upscaler};
use rea_launcher::hub::{pick, size_name};
use rea_launcher_ui::hub::HubForm;
use wangan_core::catalog::Game;
use wangan_core::settings::{self, LOOK_KEYS, Look, Settings};

/// Output sizes offered in the window; any other saved size is kept.
const OUTPUTS: [Size; 3] = [
    Size::new(1920, 1080),
    Size::new(2560, 1440),
    Size::new(3840, 2160),
];

/// Render sizes for the N2 games (`None` = the output size).
const RENDERS: [Option<Size>; 5] = [
    None,
    Some(Size::new(1280, 720)),
    Some(Size::new(1920, 1080)),
    Some(Size::new(2560, 1440)),
    Some(Size::new(3840, 2160)),
];

/// The rows a look sets, besides itself: changing one by hand ends it.
const PICTURE_KEYS: [&str; 3] = ["render", "output", "upscaler"];

pub const RENDER_FIXED: &str =
    "This game draws at its own size; the upscaler sizes it to your screen.";
const SHARPNESS_OFF: &str = "Only FSR 1 and NVIDIA Image Scaling use sharpness.";
/// Why gamescope's rows (and the looks, which only set them) are off on
/// Windows, where the game sets its own screen mode.
pub const NO_GAMESCOPE: &str = "On Windows the game sets its own screen mode (full screen, 1360 × 768); this option is for gamescope on Linux.";

/// Whether `game` takes option `key` on `host` (the rows its window can
/// change; the command line refuses the others).
pub fn offers(game: &Game, key: &str, host: Host) -> bool {
    let helper = settings::OPTIONS.iter().any(|o| o.key == key && o.helper);
    match key {
        "render" if game.render_fixed() => false,
        "look" => host == Host::Linux,
        _ => !(helper && host == Host::Windows),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub settings: Settings,
    game: &'static Game,
    host: Host,
    /// The player's screen, for the looks.
    screen: Option<(u32, u32)>,
    /// The keys the player changed: the only ones a save writes.
    changed: Vec<&'static str>,
}

fn area(s: &Size) -> u64 {
    u64::from(s.w) * u64::from(s.h)
}

/// The curated output sizes plus the current one, smallest first.
fn output_choices(current: Size) -> Vec<Size> {
    let mut v = OUTPUTS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v.sort_by_key(area);
    }
    v
}

/// The curated render sizes plus the current one.
fn render_choices(current: Option<Size>) -> Vec<Option<Size>> {
    let mut v = RENDERS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v[1..].sort_by_key(|s| s.as_ref().map(area));
    }
    v
}

impl Form {
    pub fn new(settings: Settings, game: &'static Game, screen: Option<(u32, u32)>) -> Form {
        Form::on(settings, game, screen, Host::current())
    }

    pub fn on(
        settings: Settings,
        game: &'static Game,
        screen: Option<(u32, u32)>,
        host: Host,
    ) -> Form {
        Form {
            settings,
            game,
            host,
            screen,
            changed: Vec::new(),
        }
    }

    fn changed(&mut self, keys: &[&'static str]) {
        for key in keys {
            if !self.changed.contains(key) {
                self.changed.push(key);
            }
        }
    }
}

impl HubForm for Form {
    fn value(&self, key: &str) -> String {
        let s = &self.settings;
        match key {
            "look" => s.look.map_or("Custom", Look::label).to_string(),
            "render" => match s.render {
                _ if self.game.render_fixed() => size_name(self.game.native),
                None => format!("Same as output, {}", size_name(s.output)),
                Some(z) => size_name(z),
            },
            "output" => size_name(s.output),
            "upscaler" => s.upscaler.label().to_string(),
            "sharpness" => s.sharpness.to_string(),
            "fullscreen" => if s.fullscreen { "On" } else { "Off" }.to_string(),
            "deadzone" => format!("{:.0}%", s.deadzone * 100.0),
            _ => String::new(),
        }
    }

    fn disabled(&self, key: &str) -> Option<String> {
        if !offers(self.game, key, self.host) {
            let why = match key {
                "render" => RENDER_FIXED,
                _ => NO_GAMESCOPE,
            };
            return Some(why.into());
        }
        (key == "sharpness" && !self.settings.upscaler.sharpens()).then(|| SHARPNESS_OFF.into())
    }

    fn adjust(&mut self, key: &str, d: i32, wrap: bool) -> bool {
        let Some(spec) = settings::OPTIONS.iter().find(|o| o.key == key) else {
            return false;
        };
        if self.disabled(key).is_some() || d == 0 {
            return false;
        }
        let before = self.settings.clone();
        let s = &mut self.settings;
        match key {
            "look" => {
                let next = match s.look {
                    // from a custom picture, the one for this screen first
                    None => Look::recommended(self.screen),
                    Some(cur) => pick(&Look::ALL, cur, d, wrap),
                };
                next.apply(s, self.game, self.screen);
            }
            "render" => s.render = pick(&render_choices(s.render), s.render, d, wrap),
            "output" => s.output = pick(&output_choices(s.output), s.output, d, wrap),
            "upscaler" => s.upscaler = pick(&Upscaler::ALL, s.upscaler, d, wrap),
            "fullscreen" => s.fullscreen = !s.fullscreen,
            "sharpness" => s.sharpness = (i32::from(s.sharpness) + d).clamp(0, 20) as u8,
            "deadzone" => {
                let pct = (s.deadzone * 100.0).round() as i32 + d;
                s.deadzone = pct.clamp(0, 30) as f32 / 100.0;
            }
            _ => return false,
        }
        if self.settings == before {
            return false;
        }
        if spec.key == "look" {
            self.changed(&LOOK_KEYS);
        } else {
            self.changed(&[spec.key]);
            if PICTURE_KEYS.contains(&spec.key) && self.settings.look.is_some() {
                self.settings.look = None;
                self.changed(&["look"]);
            }
        }
        true
    }

    fn save(&self, file: &Path) -> io::Result<()> {
        self.settings.write(file, &self.changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wangan_core::catalog::{GameId, game};

    fn n2() -> Form {
        Form::on(Settings::default(), game(GameId::Mt3), None, Host::Linux)
    }

    #[test]
    fn windows_turns_the_gamescope_rows_and_the_looks_off_with_a_reason() {
        let mut f = Form::on(Settings::default(), game(GameId::Mt5), None, Host::Windows);
        for key in ["look", "output", "upscaler", "sharpness", "fullscreen"] {
            assert_eq!(f.disabled(key).as_deref(), Some(NO_GAMESCOPE), "{key}");
            assert!(!f.adjust(key, 1, false), "{key}");
        }
        assert!(
            f.adjust("deadzone", 1, false),
            "the dead zone works everywhere"
        );
        let l = Form::on(Settings::default(), game(GameId::Mt5), None, Host::Linux);
        assert_eq!(l.disabled("output"), None);
    }

    #[test]
    fn output_steps_through_the_curated_sizes_and_keeps_a_custom_one() {
        let mut f = n2();
        assert_eq!(f.settings.output, Size::new(3840, 2160));
        assert!(!f.adjust("output", 1, false), "clamped at the end");
        assert!(f.adjust("output", -1, false));
        assert_eq!(f.settings.output, Size::new(2560, 1440));
        assert!(f.adjust("output", 1, true));
        assert!(f.adjust("output", 1, true), "Enter wraps around");
        assert_eq!(f.settings.output, Size::new(1920, 1080));
        let custom = Size::new(3440, 1440);
        assert_eq!(
            output_choices(custom),
            vec![
                Size::new(1920, 1080),
                Size::new(2560, 1440),
                custom,
                Size::new(3840, 2160)
            ]
        );
        let mut f = n2();
        f.settings.output = custom;
        assert_eq!(f.value("output"), "3440 × 1440");
        f.adjust("output", 1, false);
        assert_eq!(f.settings.output, Size::new(3840, 2160));
    }

    #[test]
    fn sharpness_is_off_unless_fsr_or_nis() {
        let mut f = n2();
        assert!(f.disabled("sharpness").is_none());
        assert!(f.adjust("sharpness", 1, false));
        // FSR -> NIS -> Smooth
        f.adjust("upscaler", 1, false);
        f.adjust("upscaler", 1, false);
        assert_eq!(f.value("upscaler"), "Smooth");
        assert_eq!(f.disabled("sharpness").as_deref(), Some(SHARPNESS_OFF));
        assert!(!f.adjust("sharpness", 1, false));
        assert_eq!(f.settings.sharpness, 6);
    }

    #[test]
    fn sharpness_and_dead_zone_clamp() {
        let mut f = n2();
        for _ in 0..40 {
            f.adjust("sharpness", 1, false);
            f.adjust("deadzone", 1, false);
        }
        assert_eq!(f.settings.sharpness, 20);
        assert!((f.settings.deadzone - 0.30).abs() < 1e-6);
        assert_eq!(f.value("deadzone"), "30%");
        for _ in 0..40 {
            f.adjust("deadzone", -1, false);
        }
        assert_eq!(f.settings.deadzone, 0.0);
    }

    #[test]
    fn render_size_is_only_for_n2_games() {
        let mut f = n2();
        assert_eq!(f.value("render"), "Same as output, 3840 × 2160 (4K)");
        assert!(f.adjust("render", 1, false));
        assert_eq!(f.value("render"), "1280 × 720");
        let mut es3b = Form::on(Settings::default(), game(GameId::Mt6rr), None, Host::Linux);
        assert_eq!(es3b.disabled("render").as_deref(), Some(RENDER_FIXED));
        assert!(!es3b.adjust("render", 1, false));
        assert_eq!(es3b.value("render"), "1360 × 768");
    }

    #[test]
    fn a_look_sets_the_picture_and_a_hand_change_makes_it_custom() {
        let mt3 = game(GameId::Mt3);
        let mut f = Form::on(Settings::default(), mt3, Some((1280, 800)), Host::Linux);
        assert_eq!(f.value("look"), "Custom");
        assert!(f.adjust("look", 1, false), "from custom: the screen's look");
        assert_eq!(f.value("look"), "Steam Deck");
        assert_eq!(f.settings.output, Size::new(1280, 800));
        assert!(f.adjust("look", -1, false));
        assert_eq!(f.value("look"), "Recommended");
        f.adjust("upscaler", 1, false);
        assert_eq!(f.value("look"), "Custom");
        f.adjust("look", 1, false);
        f.adjust("deadzone", 1, false);
        assert_eq!(
            f.value("look"),
            "Steam Deck",
            "the dead zone is not a picture value"
        );
    }

    #[test]
    fn every_row_has_a_value_and_the_off_rows_say_why() {
        for host in [Host::Linux, Host::Windows] {
            for g in wangan_core::catalog::GAMES {
                let f = Form::on(Settings::default(), g, None, host);
                for o in settings::OPTIONS {
                    assert!(!f.value(o.key).is_empty(), "{}", o.key);
                    let off = f.disabled(o.key).is_some();
                    let taken = offers(g, o.key, host);
                    assert!(off || taken, "{} {} {host:?}: off, says why", g.slug, o.key);
                    // with FSR (the default) every row a game takes is on
                    assert!(!(off && taken), "{} {} {host:?}", g.slug, o.key);
                }
            }
        }
    }

    #[test]
    fn saving_writes_only_the_changed_rows_and_keeps_the_rest() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("settings/mt3.cfg");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = "# my notes\nfuture = 1\nfullscreen = maybe\n";
        std::fs::write(&path, text).unwrap();
        let mt3 = game(GameId::Mt3);
        let (s, _) = Settings::load(&path, mt3, None).unwrap();
        let mut f = Form::on(s, mt3, None, Host::Linux);
        // the Recommended look for an unknown screen (1080p), not saved
        assert_eq!(f.value("look"), "Recommended");
        f.adjust("output", 1, false);
        f.save(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(saved, format!("{text}output = 2560x1440\n"));
        // an unreadable file is left alone
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(f.save(&path).is_err());
    }
}
