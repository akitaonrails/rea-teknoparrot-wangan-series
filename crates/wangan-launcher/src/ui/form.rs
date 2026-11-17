//! A game's settings page as data: its rows, what each shows, why one is
//! off, and how ◀ ▶ change the [`Settings`] behind it.

use std::path::Path;

use wangan_core::catalog::{Host, Route};
use wangan_core::launch::ES3B_SIZE;
use wangan_core::settings::{Settings, Size, Upscaler};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Output,
    Upscaler,
    Sharpness,
    Fullscreen,
    Render,
    Deadzone,
}

impl Row {
    pub const ALL: [Row; 6] = [
        Row::Output,
        Row::Upscaler,
        Row::Sharpness,
        Row::Fullscreen,
        Row::Render,
        Row::Deadzone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Row::Output => "Output size",
            Row::Upscaler => "Upscaler",
            Row::Sharpness => "Sharpness",
            Row::Fullscreen => "Fullscreen",
            Row::Render => "Game render size",
            Row::Deadzone => "Dead zone",
        }
    }

    /// The command-line option that sets this row (`--<key>`), as in the
    /// settings file.
    pub fn key(self) -> &'static str {
        match self {
            Row::Output => "output",
            Row::Upscaler => "upscaler",
            Row::Sharpness => "sharpness",
            Row::Fullscreen => "fullscreen",
            Row::Render => "render",
            Row::Deadzone => "deadzone",
        }
    }

    pub fn from_key(key: &str) -> Option<Row> {
        Row::ALL.into_iter().find(|r| r.key() == key)
    }

    pub fn description(self) -> &'static str {
        match self {
            Row::Output => "The size of the picture on your screen. Pick your screen's size.",
            Row::Upscaler => "How the game's picture is stretched to the output size.",
            Row::Sharpness => "How crisp the upscaler makes edges. 0 is the sharpest.",
            Row::Fullscreen => "Fill the whole screen, or play in a window.",
            Row::Render => {
                "The size the game draws at. Bigger is sharper and needs a stronger GPU."
            }
            Row::Deadzone => "How far a controller stick moves before the car starts to steer.",
        }
    }
}

/// Output sizes offered in the window; any other saved size is kept.
pub const OUTPUTS: [Size; 3] = [
    Size::new(1920, 1080),
    Size::new(2560, 1440),
    Size::new(3840, 2160),
];

/// Render sizes for N2 games (`None` = the output size).
pub const RENDERS: [Option<Size>; 5] = [
    None,
    Some(Size::new(1280, 720)),
    Some(Size::new(1920, 1080)),
    Some(Size::new(2560, 1440)),
    Some(Size::new(3840, 2160)),
];

const RENDER_FIXED: &str = "This game draws at its own size; the upscaler sizes it to your screen.";
const SHARPNESS_OFF: &str = "Only FSR 1 and NVIDIA Image Scaling use sharpness.";
/// gamescope's options on Windows, where the game sets its own screen mode.
pub const NO_GAMESCOPE: &str = "On Windows the game sets its own screen mode (full screen, 1360 × 768); this option is for gamescope on Linux.";

/// Why a row can't be used on `platform` at all, whatever the game.
pub fn platform_off(row: Row, platform: Host) -> Option<&'static str> {
    match (row, platform) {
        (Row::Output | Row::Upscaler | Row::Sharpness | Row::Fullscreen, Host::Windows) => {
            Some(NO_GAMESCOPE)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub settings: Settings,
    pub route: Route,
    pub platform: Host,
}

fn size_name(s: Size) -> String {
    let tag = match (s.w, s.h) {
        (1280, 720) => " (720p)",
        (1920, 1080) => " (1080p)",
        (2560, 1440) => " (1440p)",
        (3840, 2160) => " (4K)",
        _ => "",
    };
    format!("{} × {}{tag}", s.w, s.h)
}

fn area(s: &Size) -> u64 {
    u64::from(s.w) * u64::from(s.h)
}

/// The curated output sizes plus the current one, smallest first.
pub fn output_choices(current: Size) -> Vec<Size> {
    let mut v = OUTPUTS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v.sort_by_key(area);
    }
    v
}

/// The curated render sizes plus the current one.
pub fn render_choices(current: Option<Size>) -> Vec<Option<Size>> {
    let mut v = RENDERS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v[1..].sort_by_key(|s| s.as_ref().map(area));
    }
    v
}

/// The next item in `all` after `cur`, `d` steps away, clamped at the ends.
fn step<T: Copy + PartialEq>(all: &[T], cur: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    all[(i + d).clamp(0, all.len() as i32 - 1) as usize]
}

/// [`step`], wrapping around at the ends when `wrap`.
fn pick<T: Copy + PartialEq>(all: &[T], cur: T, d: i32, wrap: bool) -> T {
    let next = step(all, cur, d);
    match (wrap && next == cur, d > 0) {
        (false, _) => next,
        (true, true) => all[0],
        (true, false) => all[all.len() - 1],
    }
}

impl Form {
    pub fn new(settings: Settings, route: Route) -> Self {
        Self::on(settings, route, Host::current())
    }

    pub fn on(settings: Settings, route: Route, platform: Host) -> Self {
        Self {
            settings,
            route,
            platform,
        }
    }

    /// Why a row can't be changed for this game now, if it can't.
    pub fn disabled(&self, row: Row) -> Option<&'static str> {
        if let Some(why) = platform_off(row, self.platform) {
            return Some(why);
        }
        let n2 = self.route == Route::NativeN2;
        match row {
            Row::Sharpness if !matches!(self.settings.upscaler, Upscaler::Fsr | Upscaler::Nis) => {
                Some(SHARPNESS_OFF)
            }
            Row::Render if !n2 => Some(RENDER_FIXED),
            _ => None,
        }
    }

    pub fn value(&self, row: Row) -> String {
        let s = &self.settings;
        match row {
            Row::Output => size_name(s.output),
            Row::Upscaler => s.upscaler.label().to_string(),
            Row::Sharpness => s.sharpness.to_string(),
            Row::Fullscreen => if s.fullscreen { "On" } else { "Off" }.to_string(),
            Row::Render => match (self.route, s.render) {
                (Route::NativeN2, None) => format!("Same as output, {}", size_name(s.output)),
                (Route::NativeN2, Some(z)) => size_name(z),
                _ => size_name(ES3B_SIZE),
            },
            Row::Deadzone => format!("{:.0}%", s.deadzone * 100.0),
        }
    }

    /// Move a row's value `d` steps (◀ = -1, ▶ = +1, A = +1 with wrap for
    /// lists). Returns whether anything changed.
    pub fn adjust(&mut self, row: Row, d: i32, wrap: bool) -> bool {
        if self.disabled(row).is_some() || d == 0 {
            return false;
        }
        let before = self.settings.clone();
        let s = &mut self.settings;
        match row {
            Row::Output => s.output = pick(&output_choices(s.output), s.output, d, wrap),
            Row::Upscaler => s.upscaler = pick(&Upscaler::ALL, s.upscaler, d, wrap),
            Row::Fullscreen => s.fullscreen = !s.fullscreen,
            Row::Render => s.render = pick(&render_choices(s.render), s.render, d, wrap),
            Row::Sharpness => s.sharpness = (i32::from(s.sharpness) + d).clamp(0, 20) as u8,
            Row::Deadzone => {
                let pct = (s.deadzone * 100.0).round() as i32 + d;
                s.deadzone = pct.clamp(0, 30) as f32 / 100.0;
            }
        }
        self.settings != before
    }
}

/// Write `settings` into the file at `path`, keeping the player's comments
/// and unknown keys; atomically (temporary file, then rename).
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("cfg.tmp");
    std::fs::write(&tmp, settings.save(&existing))?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n2() -> Form {
        Form::on(Settings::default(), Route::NativeN2, Host::Linux)
    }

    #[test]
    fn windows_turns_the_gamescope_rows_off_with_a_reason() {
        let mut f = Form::on(Settings::default(), Route::Es3b, Host::Windows);
        for row in [Row::Output, Row::Upscaler, Row::Sharpness, Row::Fullscreen] {
            assert_eq!(f.disabled(row), Some(NO_GAMESCOPE), "{row:?}");
            assert!(!f.adjust(row, 1, true), "{row:?} must not change");
        }
        assert_eq!(f.settings, Settings::default());
        assert!(f.disabled(Row::Deadzone).is_none());
        assert!(f.adjust(Row::Deadzone, 1, false));
        // the same rows are on for Linux
        let l = Form::on(Settings::default(), Route::Es3b, Host::Linux);
        assert!(l.disabled(Row::Output).is_none() && l.disabled(Row::Fullscreen).is_none());
    }

    #[test]
    fn every_row_has_a_command_line_key() {
        for row in Row::ALL {
            assert_eq!(Row::from_key(row.key()), Some(row));
        }
        assert_eq!(Row::from_key("nonsense"), None);
    }

    #[test]
    fn output_steps_through_the_curated_sizes_and_keeps_a_custom_one() {
        let mut f = n2();
        assert_eq!(f.settings.output, Size::new(3840, 2160));
        assert!(!f.adjust(Row::Output, 1, false), "clamped at the end");
        assert!(f.adjust(Row::Output, -1, false));
        assert_eq!(f.settings.output, Size::new(2560, 1440));
        assert!(f.adjust(Row::Output, 1, true));
        assert!(f.adjust(Row::Output, 1, true), "A wraps around");
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
        assert_eq!(f.value(Row::Output), "3440 × 1440");
        f.adjust(Row::Output, 1, false);
        assert_eq!(f.settings.output, Size::new(3840, 2160));
    }

    #[test]
    fn sharpness_is_off_unless_fsr_or_nis() {
        let mut f = n2();
        assert!(f.disabled(Row::Sharpness).is_none());
        assert!(f.adjust(Row::Sharpness, 1, false));
        assert_eq!(f.settings.sharpness, 6);
        // FSR → NIS → Smooth
        f.adjust(Row::Upscaler, 1, false);
        assert_eq!(f.settings.upscaler, Upscaler::Nis);
        f.adjust(Row::Upscaler, 1, false);
        assert_eq!(f.settings.upscaler, Upscaler::Linear);
        assert_eq!(f.value(Row::Upscaler), "Smooth");
        assert!(f.disabled(Row::Sharpness).is_some());
        assert!(!f.adjust(Row::Sharpness, 1, false));
        assert_eq!(f.settings.sharpness, 6);
    }

    #[test]
    fn sharpness_and_dead_zone_clamp() {
        let mut f = n2();
        for _ in 0..40 {
            f.adjust(Row::Sharpness, 1, false);
            f.adjust(Row::Deadzone, 1, false);
        }
        assert_eq!(f.settings.sharpness, 20);
        assert!((f.settings.deadzone - 0.30).abs() < 1e-6);
        assert_eq!(f.value(Row::Deadzone), "30%");
        for _ in 0..40 {
            f.adjust(Row::Deadzone, -1, false);
        }
        assert_eq!(f.settings.deadzone, 0.0);
    }

    #[test]
    fn render_size_is_only_for_n2_games() {
        let mut f = n2();
        assert_eq!(f.value(Row::Render), "Same as output, 3840 × 2160 (4K)");
        f.adjust(Row::Render, 1, false);
        f.adjust(Row::Render, 1, false);
        assert_eq!(f.settings.render, Some(Size::new(1920, 1080)));

        let mut es3b = Form::on(Settings::default(), Route::Es3b, Host::Linux);
        assert_eq!(es3b.disabled(Row::Render), Some(RENDER_FIXED));
        assert_eq!(es3b.value(Row::Render), "1360 × 768");
        assert!(!es3b.adjust(Row::Render, 1, false));
        assert!(es3b.disabled(Row::Deadzone).is_none());
        assert!(es3b.adjust(Row::Fullscreen, 1, false));
        assert!(!es3b.settings.fullscreen);
    }

    #[test]
    fn every_row_has_words_and_disabled_rows_say_why() {
        for (route, platform) in [
            (Route::NativeN2, Host::Linux),
            (Route::Es3b, Host::Linux),
            (Route::Es3b, Host::Windows),
        ] {
            let f = Form::on(Settings::default(), route, platform);
            for row in Row::ALL {
                assert!(!row.label().is_empty() && row.description().ends_with('.'));
                assert!(!f.value(row).is_empty());
                if let Some(why) = f.disabled(row) {
                    assert!(why.ends_with('.'), "{why}");
                }
            }
        }
    }

    #[test]
    fn saving_keeps_notes_and_reads_back() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("settings/mt3.cfg");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# my notes\nfuture = 1\n").unwrap();
        let mut f = n2();
        f.adjust(Row::Output, -2, false);
        f.adjust(Row::Fullscreen, 1, false);
        save(&path, &f.settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# my notes\nfuture = 1\n"), "{text}");
        let (back, problems) = Settings::parse(&text);
        assert!(problems.is_empty());
        assert_eq!(back, f.settings);
        assert_eq!(back.output, Size::new(1920, 1080));
        assert!(!back.fullscreen);
        assert!(!path.with_extension("cfg.tmp").exists());
    }
}
