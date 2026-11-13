//! The look: the Shuto expressway at night. Asphalt, sodium lamps and the
//! green-and-white of Japanese expressway signs; Barlow for the type.

use std::path::Path;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId};

/// Asphalt at night: the page.
pub const BASE: Color32 = Color32::from_rgb(0x0E, 0x16, 0x21);
/// Raised asphalt: cards on the settings and controls pages, text fields.
pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x21, 0x2E);
/// Control borders and dividers (3:1 on base and panel).
pub const LINE: Color32 = Color32::from_rgb(0x6B, 0x7A, 0x8C);
/// Sodium lamp: focus ring, Play, the active underline.
pub const SODIUM: Color32 = Color32::from_rgb(0xF2, 0xA3, 0x3A);
pub const SODIUM_HOVER: Color32 = Color32::from_rgb(0xF7, 0xB8, 0x5E);
/// Expressway sign green: the top bar, sign plates.
pub const SIGN: Color32 = Color32::from_rgb(0x0F, 0x6B, 0x4A);
/// Sign white: text.
pub const INK: Color32 = Color32::from_rgb(0xF4, 0xF7, 0xF2);
/// Lane grey: secondary text.
pub const MUTED: Color32 = Color32::from_rgb(0x8A, 0x97, 0xA6);
/// Signal red: the "Not supported yet" chip (white text on it).
pub const SIGNAL: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
/// Error text on asphalt (signal red itself is too dark for text there).
pub const ERROR: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x80);
/// Text and outlines of controls that can't change now.
pub const DISABLED: Color32 = Color32::from_rgb(0x5E, 0x6B, 0x7A);

/// How opaque the asphalt is behind text laid over the hero picture.
pub const SCRIM: u8 = 240;

/// Design canvas: everything is laid out for 720 lines and zoomed.
pub const DESIGN_HEIGHT: f32 = 720.0;

pub mod size {
    pub const TITLE: f32 = 58.0;
    pub const PAGE_TITLE: f32 = 36.0;
    pub const BRAND: f32 = 24.0;
    pub const NAV: f32 = 18.0;
    pub const BUTTON: f32 = 22.0;
    pub const BODY: f32 = 15.0;
    pub const LABEL: f32 = 17.0;
    pub const NOTE: f32 = 13.0;
    pub const SHIELD: f32 = 14.0;
    pub const BADGE: f32 = 11.0;
    #[cfg(test)]
    pub const ALL: [f32; 10] = [
        TITLE, PAGE_TITLE, BRAND, NAV, BUTTON, BODY, LABEL, NOTE, SHIELD, BADGE,
    ];
}

const DISPLAY: &str = "display";
const DISPLAY_SEMI: &str = "display-semibold";
const SEMIBOLD: &str = "semibold";

/// Barlow Condensed Bold: game titles.
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY.into()))
}
/// Barlow Condensed SemiBold: buttons, shields, the brand.
pub fn display_semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY_SEMI.into()))
}
/// Barlow SemiBold: labels, nav.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
/// Barlow Regular: body text.
pub fn body(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Load the Barlow files from `dir`; any missing face falls back to egui's
/// own fonts, so the window always has text.
pub fn install_fonts(ctx: &egui::Context, dir: &Path) {
    let mut defs = FontDefinitions::default();
    let fallback: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut load = |file: &str| -> Option<String> {
        let bytes = std::fs::read(dir.join(file)).ok()?;
        defs.font_data
            .insert(file.to_string(), FontData::from_owned(bytes).into());
        Some(file.to_string())
    };
    let regular = load("Barlow-Regular.ttf");
    let semi = load("Barlow-SemiBold.ttf");
    let cond_semi = load("BarlowCondensed-SemiBold.ttf");
    let cond_bold = load("BarlowCondensed-Bold.ttf");
    let chain = |first: &[&Option<String>]| -> Vec<String> {
        first
            .iter()
            .filter_map(|f| (*f).clone())
            .chain(fallback.iter().cloned())
            .collect()
    };
    defs.families
        .insert(FontFamily::Proportional, chain(&[&regular]));
    defs.families
        .insert(FontFamily::Name(SEMIBOLD.into()), chain(&[&semi, &regular]));
    defs.families.insert(
        FontFamily::Name(DISPLAY_SEMI.into()),
        chain(&[&cond_semi, &cond_bold, &semi]),
    );
    defs.families.insert(
        FontFamily::Name(DISPLAY.into()),
        chain(&[&cond_bold, &cond_semi, &semi]),
    );
    ctx.set_fonts(defs);
}

/// egui's own widgets (the folder text field) in the palette.
pub fn install_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.override_text_color = Some(INK);
        v.extreme_bg_color = PANEL;
        v.panel_fill = BASE;
        v.window_fill = PANEL;
        v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xF2, 0xA3, 0x3A, 110);
        v.selection.stroke = egui::Stroke::new(2.0, SODIUM);
        v.text_cursor.stroke = egui::Stroke::new(2.0, SODIUM);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.5, LINE);
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, INK);
        v.widgets.active.bg_stroke = egui::Stroke::new(2.0, SODIUM);
        s.text_styles
            .insert(egui::TextStyle::Body, body(size::LABEL));
        s.text_styles
            .insert(egui::TextStyle::Monospace, body(size::LABEL));
        s.spacing.item_spacing = egui::vec2(0.0, 0.0);
        s.spacing.window_margin = egui::Margin::ZERO;
    });
}

#[cfg(test)]
/// `fill` laid over `under` (fill is premultiplied, as egui keeps it).
pub fn over(fill: Color32, under: Color32) -> Color32 {
    let a = f32::from(fill.a()) / 255.0;
    let mix = |f: u8, b: u8| (f32::from(f) + (1.0 - a) * f32::from(b)).round() as u8;
    Color32::from_rgb(
        mix(fill.r(), under.r()),
        mix(fill.g(), under.g()),
        mix(fill.b(), under.b()),
    )
}

pub fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher_ui::theme::{Palette, contrast, tint};

    fn palette() -> Palette {
        Palette {
            base: BASE,
            panel: PANEL,
            line: LINE,
            ink: INK,
            muted: MUTED,
            action: SODIUM,
            action_hover: SODIUM_HOVER,
            on_action: BASE,
            select: SODIUM,
            on_select: BASE,
            error: ERROR,
            disabled_ink: DISABLED,
            disabled_line: DISABLED,
            focus: SODIUM,
            selection: tint(SODIUM, 128),
        }
    }

    #[test]
    fn the_palette_meets_wcag_aa() {
        let problems = palette().contrast_problems();
        assert!(problems.is_empty(), "{problems:#?}");
    }

    /// The pairs this window draws beyond the SDK's: sign plates, chips,
    /// and text over the hero picture (worst case: a white picture under
    /// the scrim).
    #[test]
    fn signs_chips_and_text_over_the_picture_are_readable() {
        let photo = over(alpha(BASE, SCRIM), Color32::WHITE);
        let checks = [
            ("white on the sign green", INK, SIGN, 4.5),
            ("focus ring on the sign green", SODIUM, SIGN, 3.0),
            ("white on the signal red chip", INK, SIGNAL, 4.5),
            ("dark text on Play", BASE, SODIUM, 4.5),
            ("white over the picture", INK, photo, 4.5),
            ("grey over the picture", MUTED, photo, 4.5),
            ("focus ring over the picture", SODIUM, photo, 3.0),
            ("error over the picture", ERROR, photo, 4.5),
            ("grey on a card", MUTED, PANEL, 4.5),
            ("disabled on a card", DISABLED, PANEL, 2.0),
        ];
        let failing: Vec<String> = checks
            .iter()
            .map(|(what, fg, bg, least)| (what, contrast(*fg, *bg), least))
            .filter(|(_, r, least)| r < *least)
            .map(|(what, r, least)| format!("{what}: {r:.2} < {least}"))
            .collect();
        assert!(failing.is_empty(), "{failing:#?}");
    }

    #[test]
    fn no_text_is_too_small() {
        for s in size::ALL {
            assert!(s >= 11.0, "{s}");
        }
        const { assert!(size::BODY >= 13.0 && size::NOTE >= 13.0 && size::LABEL >= 13.0) };
    }
}
