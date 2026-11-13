//! Per-game settings, stored as `key = value` lines in
//! `<data>/settings/<slug>.cfg`. Unknown keys and comments are kept when the
//! launcher rewrites the file, so a player's notes survive.

use std::fmt;

/// How gamescope scales the game's picture to the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upscaler {
    /// Bilinear, gamescope's default.
    Linear,
    /// AMD FidelityFX Super Resolution 1 (spatial; any GPU).
    Fsr,
    /// NVIDIA Image Scaling (spatial; any GPU).
    Nis,
    /// Sharp pixels.
    Nearest,
}

impl Upscaler {
    pub const ALL: [Upscaler; 4] = [
        Upscaler::Fsr,
        Upscaler::Nis,
        Upscaler::Linear,
        Upscaler::Nearest,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Upscaler::Linear => "linear",
            Upscaler::Fsr => "fsr",
            Upscaler::Nis => "nis",
            Upscaler::Nearest => "nearest",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Upscaler::Linear => "Smooth",
            Upscaler::Fsr => "FSR 1 (sharp upscale)",
            Upscaler::Nis => "NVIDIA Image Scaling",
            Upscaler::Nearest => "Sharp pixels",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|u| u.key() == s)
    }
}

/// A width × height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl Size {
    pub const fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }
    pub fn parse(s: &str) -> Option<Self> {
        let (w, h) = s.trim().split_once(['x', 'X'])?;
        let (w, h) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
        (w >= 320 && h >= 240 && w <= 15360 && h <= 8640).then_some(Self { w, h })
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.w, self.h)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The game's own render size (N2 games only: the loader patches it).
    pub render: Option<Size>,
    /// What gamescope shows on the screen.
    pub output: Size,
    pub upscaler: Upscaler,
    /// 0–20, gamescope's FSR/NIS sharpness (0 = sharpest).
    pub sharpness: u8,
    pub fullscreen: bool,
    /// Stick/wheel dead zone around the centre, 0.0–0.3.
    pub deadzone: f32,
    /// 1.0 = linear; above 1 softens the centre for pads.
    pub steer_curve: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render: None,
            output: Size::new(3840, 2160),
            upscaler: Upscaler::Fsr,
            sharpness: 5,
            fullscreen: true,
            deadzone: 0.05,
            steer_curve: 1.4,
        }
    }
}

const KEYS: &[&str] = &[
    "render",
    "output",
    "upscaler",
    "sharpness",
    "fullscreen",
    "deadzone",
    "steer_curve",
];

impl Settings {
    /// Read a settings file; bad values fall back to the default and are reported.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut s = Settings::default();
        let mut problems = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                problems.push(format!("line {}: no '='", n + 1));
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            let ok = match k {
                "render" => {
                    if v == "native" {
                        s.render = None;
                        true
                    } else {
                        Size::parse(v).map(|z| s.render = Some(z)).is_some()
                    }
                }
                "output" => Size::parse(v).map(|z| s.output = z).is_some(),
                "upscaler" => Upscaler::parse(v).map(|u| s.upscaler = u).is_some(),
                "sharpness" => v
                    .parse::<u8>()
                    .ok()
                    .filter(|x| *x <= 20)
                    .map(|x| s.sharpness = x)
                    .is_some(),
                "fullscreen" => parse_bool(v).map(|b| s.fullscreen = b).is_some(),
                "deadzone" => v
                    .parse::<f32>()
                    .ok()
                    .filter(|x| (0.0..=0.3).contains(x))
                    .map(|x| s.deadzone = x)
                    .is_some(),
                "steer_curve" => v
                    .parse::<f32>()
                    .ok()
                    .filter(|x| (1.0..=3.0).contains(x))
                    .map(|x| s.steer_curve = x)
                    .is_some(),
                _ => true, // unknown keys are kept, not an error
            };
            if !ok {
                problems.push(format!("line {}: bad value for {k}: {v}", n + 1));
            }
        }
        (s, problems)
    }

    fn value(&self, key: &str) -> String {
        match key {
            "render" => self.render.map_or("native".into(), |z| z.to_string()),
            "output" => self.output.to_string(),
            "upscaler" => self.upscaler.key().into(),
            "sharpness" => self.sharpness.to_string(),
            "fullscreen" => self.fullscreen.to_string(),
            "deadzone" => format!("{:.2}", self.deadzone),
            "steer_curve" => format!("{:.2}", self.steer_curve),
            _ => unreachable!(),
        }
    }

    /// Rewrite `existing` with this settings' values, keeping comments and
    /// unknown keys in place and appending missing keys.
    pub fn save(&self, existing: &str) -> String {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for line in existing.lines() {
            let key = line
                .split_once('=')
                .map(|(k, _)| k.trim())
                .filter(|k| KEYS.contains(k));
            match key {
                Some(k) if !line.trim_start().starts_with('#') => {
                    seen.push(k.to_string());
                    out.push(format!("{k} = {}", self.value(k)));
                }
                _ => out.push(line.to_string()),
            }
        }
        for k in KEYS {
            if !seen.iter().any(|s| s == k) {
                out.push(format!("{k} = {}", self.value(k)));
            }
        }
        out.join("\n") + "\n"
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Pad steering: dead zone, then a power curve, sign kept.
pub fn shape_steer(x: f32, deadzone: f32, curve: f32) -> f32 {
    let a = x.abs();
    if a <= deadzone {
        return 0.0;
    }
    let t = ((a - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    t.powf(curve).copysign(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_comments_and_unknown_keys() {
        let text = "# my notes\nupscaler = nis\nfuture_key = 7\noutput=2560x1440\n";
        let (s, problems) = Settings::parse(text);
        assert!(problems.is_empty());
        assert_eq!(s.upscaler, Upscaler::Nis);
        assert_eq!(s.output, Size::new(2560, 1440));
        let saved = s.save(text);
        assert!(
            saved.starts_with("# my notes\nupscaler = nis\nfuture_key = 7\noutput = 2560x1440\n")
        );
        assert!(saved.contains("render = native"));
        assert_eq!(Settings::parse(&saved).0, s);
    }

    #[test]
    fn bad_values_fall_back_and_are_reported() {
        let (s, p) =
            Settings::parse("output = 99x99\nsharpness = 50\nfullscreen = maybe\nnonsense");
        assert_eq!(s, Settings::default());
        assert_eq!(p.len(), 4, "{p:?}");
    }

    #[test]
    fn steer_shape_has_dead_zone_and_keeps_ends() {
        assert_eq!(shape_steer(0.04, 0.05, 1.4), 0.0);
        assert_eq!(shape_steer(1.0, 0.05, 1.4), 1.0);
        assert_eq!(shape_steer(-1.0, 0.05, 1.4), -1.0);
        let half = shape_steer(0.5, 0.05, 1.4);
        assert!(half > 0.0 && half < 0.5);
    }
}
