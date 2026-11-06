//! Design tokens (dark and light palettes) plus egui style tuning.

use eframe::egui::{
    self, epaint::Shadow, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Stroke, TextStyle,
};
use std::sync::Arc;

/// The colour tokens of one theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg_app: Color32,
    pub bg_surface: Color32,
    pub bg_sunken: Color32,
    pub bg_input: Color32,
    pub bg_input_hover: Color32,
    pub bg_elevated: Color32,
    pub border: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub fg: Color32,
    pub fg_2: Color32,
    pub fg_3: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Text/icons drawn on a solid accent fill.
    pub on_accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub syn_key: Color32,
    pub syn_str: Color32,
    pub syn_num: Color32,
    pub syn_lit: Color32,
    pub syn_punct: Color32,
}

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const DARK: Palette = Palette {
    dark: true,
    bg_app: rgb(0x1c1c1e),
    bg_surface: rgb(0x2c2c2e),
    bg_sunken: rgb(0x161618),
    bg_input: rgb(0x1c1c1e),
    bg_input_hover: rgb(0x232325),
    bg_elevated: rgb(0x3a3a3c),
    border: rgb(0x3a3a3c),
    border_subtle: rgb(0x2c2c2e),
    border_strong: rgb(0x4a4a4e),
    fg: rgb(0xf5f5f7),
    fg_2: rgb(0xa1a1a6),
    // The original #6e6e73 is only 3.1:1 on the app background; lightened to pass AA.
    fg_3: rgb(0x98989d),
    accent: rgb(0x5b9bf5),
    accent_hover: rgb(0x4a8df0),
    on_accent: rgb(0x101828),
    danger: rgb(0xf46969),
    success: rgb(0x22c55e),
    warning: rgb(0xf59e0b),
    syn_key: rgb(0x82aaff),
    syn_str: rgb(0x9ece6a),
    syn_num: rgb(0xff9e64),
    syn_lit: rgb(0xbb9af7),
    syn_punct: rgb(0x95959e),
};

pub const LIGHT: Palette = Palette {
    dark: false,
    bg_app: rgb(0xf5f5f7),
    bg_surface: rgb(0xffffff),
    bg_sunken: rgb(0xebebed),
    bg_input: rgb(0xffffff),
    bg_input_hover: rgb(0xf0f0f2),
    bg_elevated: rgb(0xffffff),
    border: rgb(0xd1d1d6),
    border_subtle: rgb(0xe5e5ea),
    border_strong: rgb(0xb0b0b6),
    fg: rgb(0x1c1c1e),
    fg_2: rgb(0x636366),
    // The original #8e8e93 is 3.3:1 on white; darkened to pass AA on every light surface.
    fg_3: rgb(0x67676c),
    accent: rgb(0x215ddc),
    accent_hover: rgb(0x1a4fc0),
    on_accent: rgb(0xffffff),
    // Text-capable variants of the light tokens (#dc2626, #16a34a, #d97706 are < 4.5:1).
    danger: rgb(0xc81e1e),
    success: rgb(0x137839),
    warning: rgb(0x9c5404),
    syn_key: rgb(0x1d4ed8),
    syn_str: rgb(0x166534),
    syn_num: rgb(0x9a3412),
    syn_lit: rgb(0x6d28d9),
    syn_punct: rgb(0x5a5a5f),
};

thread_local! {
    static PALETTE: std::cell::Cell<Palette> = const { std::cell::Cell::new(DARK) };
}

/// The active palette (per UI thread; the app sets it from the theme mode each frame).
pub fn pal() -> Palette {
    PALETTE.with(|p| p.get())
}

pub fn set_palette(p: Palette) {
    PALETTE.with(|c| c.set(p));
}

macro_rules! tokens {
    ($($name:ident),* $(,)?) => {
        $(
            #[inline]
            pub fn $name() -> Color32 {
                pal().$name
            }
        )*
    };
}
tokens!(
    bg_app,
    bg_surface,
    bg_sunken,
    bg_input,
    bg_input_hover,
    bg_elevated,
    border,
    border_subtle,
    border_strong,
    fg,
    fg_2,
    fg_3,
    accent,
    accent_hover,
    on_accent,
    danger,
    success,
    warning,
    syn_key,
    syn_str,
    syn_num,
    syn_lit,
    syn_punct,
);

/// Hover tint (`rgba(255,255,255,.06)` dark, `rgba(0,0,0,.04)` light).
pub fn hover_tint() -> Color32 {
    if pal().dark {
        Color32::from_white_alpha(15)
    } else {
        Color32::from_black_alpha(10)
    }
}
/// Active tint (`.10` dark, `.08` light).
pub fn active_tint() -> Color32 {
    if pal().dark {
        Color32::from_white_alpha(26)
    } else {
        Color32::from_black_alpha(20)
    }
}
pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// WCAG relative luminance.
pub fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

/// WCAG contrast ratio (1..21) between two opaque colours.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// White or near-black, whichever reads better on `bg`.
pub fn text_on(bg: Color32) -> Color32 {
    let dark = rgb(0x101828);
    if contrast(Color32::WHITE, bg) >= contrast(dark, bg) {
        Color32::WHITE
    } else {
        dark
    }
}

fn subtle(c: Color32) -> Color32 {
    let a = if pal().dark { 31 } else { 24 };
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
pub fn accent_subtle() -> Color32 {
    subtle(accent())
}
pub fn success_subtle() -> Color32 {
    subtle(success())
}
pub fn warning_subtle() -> Color32 {
    subtle(warning())
}
pub fn danger_subtle() -> Color32 {
    subtle(danger())
}
pub fn info_subtle() -> Color32 {
    subtle(accent())
}

pub fn method_color(m: &str) -> Color32 {
    // Light values, darkened where they fail 4.5:1 on white.
    let (d, l) = match m {
        "GET" => (0x22c55e, 0x137839),
        "POST" => (0xeab308, 0x8b6000),
        "PUT" => (0x5a97fa, 0x215ddc),
        "PATCH" => (0xbb76fc, 0x7c3aed),
        "DELETE" => (0xf46969, 0xc81e1e),
        "HEAD" => (0x888af7, 0x4f46e5),
        _ => (0xf066aa, 0xc01a66),
    };
    rgb(if pal().dark { d } else { l })
}

pub fn status_colors(status: u16) -> (Color32, Color32) {
    match status {
        200..=299 => (success_subtle(), success()),
        300..=399 => (info_subtle(), accent()),
        400..=499 => (warning_subtle(), warning()),
        500..=599 => (danger_subtle(), danger()),
        _ => (active_tint(), fg_2()),
    }
}

pub fn inter(weight: &str) -> FontFamily {
    FontFamily::Name(format!("Inter-{weight}").into())
}
pub fn mono_bold() -> FontFamily {
    FontFamily::Name("Mono-Bold".into())
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 6] = [
        (
            "Inter-Regular",
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
        ),
        (
            "Inter-Medium",
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
        ),
        (
            "Inter-SemiBold",
            include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
        ),
        (
            "Inter-Bold",
            include_bytes!("../assets/fonts/Inter-Bold.ttf"),
        ),
        (
            "JBM-Regular",
            include_bytes!("../assets/fonts/JetBrainsMonoNL-Regular.ttf"),
        ),
        (
            "JBM-Bold",
            include_bytes!("../assets/fonts/JetBrainsMonoNL-Bold.ttf"),
        ),
    ];
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mono_fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let with_fallback = |first: &str, fb: &Vec<String>| {
        let mut v = vec![first.to_owned()];
        v.extend(fb.iter().cloned());
        v
    };
    fonts.families.insert(
        FontFamily::Proportional,
        with_fallback("Inter-Regular", &fallback),
    );
    for w in ["Regular", "Medium", "SemiBold", "Bold"] {
        fonts
            .families
            .insert(inter(w), with_fallback(&format!("Inter-{w}"), &fallback));
    }
    fonts.families.insert(
        FontFamily::Monospace,
        with_fallback("JBM-Regular", &mono_fallback),
    );
    fonts
        .families
        .insert(mono_bold(), with_fallback("JBM-Bold", &mono_fallback));
    ctx.set_fonts(fonts);
}

/// How the theme is chosen (toolbar toggle, persisted in `ui-state.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
            ThemeMode::System => "System",
        }
    }
    /// Dark -> Light -> System -> Dark.
    pub fn next(self) -> Self {
        match self {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::System,
            ThemeMode::System => ThemeMode::Dark,
        }
    }
    /// Whether the dark palette applies; `system` is the OS preference when known.
    pub fn is_dark(self, system: Option<egui::Theme>) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => system.is_none_or(|t| t == egui::Theme::Dark),
        }
    }
}

/// Switches palette + egui style when the effective theme changed (`styled` remembers what the
/// context currently shows). Returns true on change.
pub fn sync_theme(ctx: &egui::Context, mode: ThemeMode, styled: &mut Option<bool>) -> bool {
    let dark = mode.is_dark(ctx.system_theme());
    if *styled == Some(dark) {
        return false;
    }
    set_palette(if dark { DARK } else { LIGHT });
    apply_style(ctx);
    *styled = Some(dark);
    true
}

pub fn apply_style(ctx: &egui::Context) {
    let dark = pal().dark;
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.global_style_mut(|style| {
        style.visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(12.5, inter("Medium"))),
            (TextStyle::Heading, FontId::new(16.0, inter("SemiBold"))),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
        ]
        .into();

        let sp = &mut style.spacing;
        sp.item_spacing = egui::vec2(8.0, 6.0);
        sp.button_padding = egui::vec2(10.0, 5.0);
        sp.interact_size = egui::vec2(28.0, 28.0);
        sp.combo_height = 280.0;
        sp.menu_margin = egui::Margin::symmetric(4, 4);
        sp.window_margin = egui::Margin::same(12);
        sp.icon_width = 15.0;
        sp.icon_width_inner = 9.0;
        sp.scroll = egui::style::ScrollStyle::floating();
        sp.scroll.bar_width = 8.0;
        sp.scroll.floating_width = 4.0;
        sp.scroll.floating_allocated_width = 0.0;
        // keep a faint handle visible at rest so clipped content is discoverable
        sp.scroll.dormant_handle_opacity = 0.45;
        sp.scroll.dormant_background_opacity = 0.08;

        let v = &mut style.visuals;
        v.dark_mode = dark;
        v.panel_fill = bg_app();
        v.window_fill = bg_elevated();
        v.window_stroke = Stroke::new(1.0, border());
        v.window_corner_radius = CornerRadius::same(8);
        v.menu_corner_radius = CornerRadius::same(6);
        v.extreme_bg_color = bg_input();
        v.faint_bg_color = bg_surface();
        v.code_bg_color = bg_sunken();
        v.override_text_color = None;
        v.hyperlink_color = accent();
        v.popup_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 110 } else { 50 }),
        };
        v.window_shadow = v.popup_shadow;
        v.selection.bg_fill =
            Color32::from_rgba_unmultiplied(accent().r(), accent().g(), accent().b(), 80);
        v.selection.stroke = Stroke::new(1.0, accent());
        v.text_cursor.stroke = Stroke::new(1.5, accent());
        v.interact_cursor = Some(egui::CursorIcon::PointingHand);

        let r = CornerRadius::same(6);
        v.widgets.noninteractive.bg_fill = bg_surface();
        v.widgets.noninteractive.weak_bg_fill = bg_surface();
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border());
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, fg());
        v.widgets.noninteractive.corner_radius = r;

        v.widgets.inactive.bg_fill = bg_input();
        v.widgets.inactive.weak_bg_fill = bg_input();
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, border());
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, fg());
        v.widgets.inactive.corner_radius = r;

        v.widgets.hovered.bg_fill = bg_input_hover();
        v.widgets.hovered.weak_bg_fill = bg_input_hover();
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, border_strong());
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, fg());
        v.widgets.hovered.corner_radius = r;
        v.widgets.hovered.expansion = 0.0;

        v.widgets.active.bg_fill = bg_input_hover();
        v.widgets.active.weak_bg_fill = bg_input_hover();
        v.widgets.active.bg_stroke = Stroke::new(1.0, accent());
        v.widgets.active.fg_stroke = Stroke::new(1.0, fg());
        v.widgets.active.corner_radius = r;
        v.widgets.active.expansion = 0.0;

        v.widgets.open.bg_fill = bg_input_hover();
        v.widgets.open.weak_bg_fill = bg_input_hover();
        v.widgets.open.bg_stroke = Stroke::new(1.0, border_strong());
        v.widgets.open.corner_radius = r;
    });
}

/// Installs fonts and the visual style (call once per `egui::Context`).
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    apply_style(ctx);
}

/// Changes whenever cached galleys (text layouts) must be thrown away because of a theme switch.
/// egui rebuilds its glyph atlas on the pass *after* the visuals change (text coverage differs
/// between dark and light), so layouts made in the switching pass are stale as well: the value
/// changes for two consecutive passes.
pub fn galley_epoch(ctx: &egui::Context) -> u64 {
    let id = egui::Id::unique("nexus_galley_epoch");
    let pass = ctx.cumulative_pass_nr();
    let dark = pal().dark;
    ctx.data_mut(|d| {
        let (mut last_dark, mut epoch, mut since) =
            d.get_temp::<(bool, u64, u64)>(id).unwrap_or((dark, 0, 0));
        if last_dark != dark {
            last_dark = dark;
            epoch += 1;
            since = pass;
        }
        d.insert_temp(id, (last_dark, epoch, since));
        epoch * 4 + pass.saturating_sub(since).min(2)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG AA (4.5:1) for every text colour on every surface it is drawn on.
    #[test]
    fn text_colours_meet_wcag_aa_in_both_themes() {
        for p in [DARK, LIGHT] {
            set_palette(p);
            let surfaces = [p.bg_app, p.bg_surface, p.bg_sunken, p.bg_input];
            let mut texts = vec![
                ("fg", p.fg),
                ("fg_2", p.fg_2),
                ("fg_3", p.fg_3),
                ("accent", p.accent),
                ("danger", p.danger),
                ("success", p.success),
                ("warning", p.warning),
            ];
            for m in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
                texts.push((m, method_color(m)));
            }
            for (n, c) in texts {
                for bg in surfaces {
                    let r = contrast(c, bg);
                    assert!(
                        r >= 4.5,
                        "{} {n} on {bg:?}: {r:.2}",
                        if p.dark { "dark" } else { "light" }
                    );
                }
            }
            for (n, c) in [
                ("key", p.syn_key),
                ("str", p.syn_str),
                ("num", p.syn_num),
                ("lit", p.syn_lit),
                ("punct", p.syn_punct),
            ] {
                let r = contrast(c, p.bg_sunken);
                assert!(r >= 4.5, "syntax {n}: {r:.2}");
            }
            assert!(contrast(p.on_accent, p.accent) >= 4.5);
            assert!(contrast(text_on(p.accent), p.accent) >= 4.5);
            assert!(contrast(text_on(p.danger), p.danger) >= 4.5);
        }
        set_palette(DARK);
    }
}
