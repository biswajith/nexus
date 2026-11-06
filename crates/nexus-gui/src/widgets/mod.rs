//! Hand-painted widgets: egui's stock ones don't match the design, so these draw directly.

mod code_editor;
mod code_view;
mod kit;
mod modal;
pub use code_editor::{
    code_editor, focus_next_editor, request_format, set_variables, set_variables_shared,
};
pub use code_view::{code_view, ViewOpts};
pub use kit::*;
pub use modal::modal;

use crate::theme::*;
use eframe::egui::accesskit::Role;
use eframe::egui::WidgetInfo;
use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, CursorIcon, FontId, Margin, Pos2, Rect, Response,
    Sense, Shape, Stroke, StrokeKind, TextEdit, Ui,
};

/// Exposes a custom-painted widget to assistive technology: `role` + accessible `label`.
/// Returns the response so calls can be chained.
pub fn a11y(resp: Response, role: Role, label: &str) -> Response {
    resp.widget_info(|| WidgetInfo::labeled(role, true, label));
    resp
}

/// Like [`a11y`], additionally exposing the selected state (tabs, radio buttons).
pub fn a11y_selected(resp: Response, role: Role, label: &str, selected: bool) -> Response {
    resp.widget_info(|| WidgetInfo::labeled(role, true, label));
    resp.ctx.accesskit_node_builder(resp.id, |n| {
        n.set_selected(selected);
    });
    resp
}

/// Like [`a11y`], additionally exposing the expanded state (tree rows).
pub fn a11y_expanded(resp: Response, role: Role, label: &str, expanded: bool) -> Response {
    resp.widget_info(|| WidgetInfo::labeled(role, true, label));
    resp.ctx.accesskit_node_builder(resp.id, |n| {
        n.set_expanded(expanded);
    });
    resp
}

/// Sets the accessible name of a stock egui widget that has none (text edits).
pub fn a11y_name(resp: &Response, label: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| {
        n.set_label(label);
    });
}

/// Plain text whose accessible name differs from what is drawn (e.g. `Response size: 12 B` for
/// `12 B`). Assistive technology reads `accessible`.
pub fn labeled_text(
    ui: &mut Ui,
    visible: &str,
    accessible: &str,
    f: FontId,
    color: Color32,
) -> Response {
    let g = ui.painter().layout_no_wrap(visible.to_string(), f, color);
    let (rect, resp) = ui.allocate_exact_size(g.size().max(vec2(1.0, 1.0)), Sense::hover());
    ui.painter().galley(rect.min, g, color);
    a11y(resp, Role::Label, accessible)
}

pub fn font(size: f32, weight: &str) -> FontId {
    FontId::new(size, inter(weight))
}

/// Draws text with CSS-like letter spacing; returns the total width.
pub fn spaced_text(
    ui: &Ui,
    pos: Pos2,
    text: &str,
    f: FontId,
    color: Color32,
    spacing: f32,
    anchor_center: bool,
) {
    let p = ui.painter();
    let galleys: Vec<_> = text
        .chars()
        .map(|c| p.layout_no_wrap(c.to_string(), f.clone(), color))
        .collect();
    let total: f32 = galleys.iter().map(|g| g.size().x + spacing).sum::<f32>() - spacing;
    let mut x = if anchor_center {
        pos.x - total / 2.0
    } else {
        pos.x
    };
    for g in galleys {
        let w = g.size().x;
        p.galley(pos2(x, pos.y - g.size().y / 2.0), g, color);
        x += w + spacing;
    }
}

pub struct BtnStyle {
    pub fill: Color32,
    pub fill_hover: Color32,
    pub stroke: Option<Color32>,
    pub stroke_hover: Option<Color32>,
    pub text: Color32,
    pub text_hover: Color32,
    pub radius: u8,
}

impl BtnStyle {
    pub fn flat() -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            fill_hover: hover_tint(),
            stroke: None,
            stroke_hover: None,
            text: fg_2(),
            text_hover: fg(),
            radius: 4,
        }
    }
    /// Borderless button with primary-coloured text (list entries that open something).
    pub fn flat_text() -> Self {
        Self {
            text: fg(),
            ..Self::flat()
        }
    }
    pub fn outline() -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            fill_hover: hover_tint(),
            stroke: Some(border()),
            stroke_hover: Some(border_strong()),
            text: fg_2(),
            text_hover: fg(),
            radius: 6,
        }
    }
    pub fn outline_accent() -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            fill_hover: hover_tint(),
            stroke: Some(accent()),
            stroke_hover: Some(accent()),
            text: accent(),
            text_hover: accent(),
            radius: 6,
        }
    }
    pub fn primary(bg: Color32, hover: Color32) -> Self {
        Self {
            fill: bg,
            fill_hover: hover,
            stroke: None,
            stroke_hover: None,
            text: text_on(bg),
            text_hover: text_on(hover),
            radius: 6,
        }
    }
}

pub fn button(
    ui: &mut Ui,
    label: &str,
    f: FontId,
    pad: Vec2x,
    min_h: f32,
    st: &BtnStyle,
) -> Response {
    button_role(ui, label, f, pad, min_h, st, Role::Button, None)
}

/// A painted button with an explicit AccessKit role (e.g. `Tab`) and optional selected state.
#[allow(clippy::too_many_arguments)]
pub fn button_role(
    ui: &mut Ui,
    label: &str,
    f: FontId,
    pad: Vec2x,
    min_h: f32,
    st: &BtnStyle,
    role: Role,
    selected: Option<bool>,
) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), f, st.text);
    let size = vec2(
        galley.size().x + pad.0 * 2.0,
        min_h.max(galley.size().y + pad.1 * 2.0),
    );
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hov = resp.hovered();
    let p = ui.painter();
    let r = CornerRadius::same(st.radius);
    p.rect_filled(rect, r, if hov { st.fill_hover } else { st.fill });
    let sc = if hov { st.stroke_hover } else { st.stroke };
    if let Some(c) = sc {
        p.rect_stroke(rect.shrink(0.5), r, Stroke::new(1.0, c), StrokeKind::Inside);
    }
    let color = if hov { st.text_hover } else { st.text };
    p.galley(rect.center() - galley.size() / 2.0, galley, color);
    let resp = match selected {
        Some(s) => a11y_selected(resp, role, label, s),
        None => a11y(resp, role, label),
    };
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// (horizontal, vertical) padding.
pub type Vec2x = (f32, f32);

/// Tab with a 2px accent underline (sidebar, request and response tabs).
pub fn underline_tab(
    ui: &mut Ui,
    label: &str,
    active: bool,
    height: f32,
    stretch: Option<f32>,
) -> Response {
    let color = if active { accent() } else { fg_2() };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font(12.0, "Medium"), color);
    let w = stretch.unwrap_or(galley.size().x + 24.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::click());
    let hov = resp.hovered();
    let p = ui.painter();
    if hov {
        p.rect_filled(rect, 0.0, hover_tint());
    }
    let color = if active {
        accent()
    } else if hov {
        fg()
    } else {
        fg_2()
    };
    p.galley(
        rect.center() - galley.size() / 2.0 - vec2(0.0, 1.0),
        galley.clone(),
        color,
    );
    // re-layout in final colour (galleys are colour-baked)
    if active {
        p.rect_filled(
            Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom()),
            0.0,
            accent(),
        );
    }
    a11y_selected(resp, Role::Tab, label, active).on_hover_cursor(CursorIcon::PointingHand)
}

pub fn hline(ui: &Ui, y: f32, x0: f32, x1: f32, color: Color32) {
    ui.painter()
        .hline(x0..=x1, y + 0.5, Stroke::new(1.0, color));
}

pub fn chevron(ui: &Ui, center: Pos2, open: bool, color: Color32) {
    let s = 3.2;
    let pts = if open {
        vec![
            center + vec2(-s, -s * 0.55),
            center + vec2(s, -s * 0.55),
            center + vec2(0.0, s * 0.75),
        ]
    } else {
        vec![
            center + vec2(-s * 0.55, -s),
            center + vec2(-s * 0.55, s),
            center + vec2(s * 0.75, 0.0),
        ]
    };
    ui.painter()
        .add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

pub fn x_icon(ui: &Ui, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    ui.painter()
        .line_segment([c + vec2(-s, -s), c + vec2(s, s)], st);
    ui.painter()
        .line_segment([c + vec2(-s, s), c + vec2(s, -s)], st);
}

pub fn check_icon(ui: &Ui, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.7, color);
    let p = ui.painter();
    p.line_segment([c + vec2(-s, 0.0), c + vec2(-s * 0.3, s * 0.7)], st);
    p.line_segment([c + vec2(-s * 0.3, s * 0.7), c + vec2(s, -s * 0.7)], st);
}

/// Magnifying glass.
pub fn search_icon(ui: &Ui, c: Pos2, color: Color32) {
    let c = c - vec2(1.0, 1.0);
    let st = Stroke::new(1.4, color);
    ui.painter().circle_stroke(c, 4.5, st);
    ui.painter()
        .line_segment([c + vec2(3.3, 3.3), c + vec2(6.5, 6.5)], st);
}

/// Up (`up = true`) or down chevron.
pub fn vchevron(ui: &Ui, c: Pos2, up: bool, color: Color32) {
    let d = if up { -1.0 } else { 1.0 };
    let st = Stroke::new(1.5, color);
    let p = ui.painter();
    p.line_segment([c + vec2(-4.0, -2.0 * d), c + vec2(0.0, 2.0 * d)], st);
    p.line_segment([c + vec2(0.0, 2.0 * d), c + vec2(4.0, -2.0 * d)], st);
}

pub fn plus_icon(ui: &Ui, c: Pos2, s: f32, color: Color32) {
    let st = Stroke::new(1.4, color);
    ui.painter()
        .line_segment([c + vec2(-s, 0.0), c + vec2(s, 0.0)], st);
    ui.painter()
        .line_segment([c + vec2(0.0, -s), c + vec2(0.0, s)], st);
}

pub fn icon_button(
    ui: &mut Ui,
    label: &str,
    size: f32,
    draw: impl FnOnce(&Ui, Pos2, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hov = resp.hovered();
    if hov {
        ui.painter().rect_filled(rect, 4, hover_tint());
    }
    draw(ui, rect.center(), if hov { fg() } else { fg_2() });
    a11y(resp, Role::Button, label)
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(label)
}

pub fn checkbox(ui: &mut Ui, label: &str, checked: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::click());
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let p = ui.painter();
    if *checked {
        p.rect_filled(rect, 3, accent());
        let c = rect.center();
        let st = Stroke::new(1.7, on_accent());
        p.line_segment([c + vec2(-3.6, 0.2), c + vec2(-1.1, 2.9)], st);
        p.line_segment([c + vec2(-1.1, 2.9), c + vec2(3.8, -2.7)], st);
    } else {
        p.rect_filled(rect, 3, bg_input());
        p.rect_stroke(
            rect.shrink(0.5),
            3,
            Stroke::new(
                1.0,
                if resp.hovered() {
                    fg_3()
                } else {
                    border_strong()
                },
            ),
            StrokeKind::Inside,
        );
    }
    resp.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, *checked, label));
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn radio(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let color = if selected { accent() } else { fg_2() };
    let f = font(12.0, if selected { "SemiBold" } else { "Regular" });
    let galley = ui.painter().layout_no_wrap(label.to_string(), f, color);
    let (rect, resp) =
        ui.allocate_exact_size(vec2(14.0 + 5.0 + galley.size().x, 20.0), Sense::click());
    let p = ui.painter();
    let c = pos2(rect.left() + 7.0, rect.center().y);
    if selected {
        p.circle_filled(c, 6.5, accent());
        p.circle_filled(c, 2.6, on_accent());
    } else {
        p.circle_filled(c, 6.5, bg_input());
        p.circle_stroke(
            c,
            6.0,
            Stroke::new(
                1.0,
                if resp.hovered() {
                    fg_3()
                } else {
                    border_strong()
                },
            ),
        );
    }
    let col = if resp.hovered() && !selected {
        fg()
    } else {
        color
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font(12.0, if selected { "SemiBold" } else { "Regular" }),
        col,
    );
    ui.painter().galley(
        pos2(rect.left() + 19.0, rect.center().y - galley.size().y / 2.0),
        galley,
        col,
    );
    resp.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, selected, label));
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A single-line text field with the app look (border, accent focus ring, hover tint).
#[allow(clippy::too_many_arguments)]
pub fn field(
    ui: &mut Ui,
    label: &str,
    text: &mut String,
    width: f32,
    height: f32,
    f: FontId,
    hint: &str,
    bg: Color32,
    bordered: bool,
) -> Response {
    field_with(
        ui, label, text, width, height, f, hint, bg, bordered, None, false,
    )
}

/// Layouter hook of a text edit (custom colouring).
pub type Layouter<'a> =
    &'a mut dyn FnMut(&Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley>;

/// [`field`] with an optional layouter and password masking.
#[allow(clippy::too_many_arguments)]
pub fn field_with(
    ui: &mut Ui,
    label: &str,
    text: &mut String,
    width: f32,
    height: f32,
    f: FontId,
    hint: &str,
    bg: Color32,
    bordered: bool,
    layouter: Option<Layouter<'_>>,
    password: bool,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let hovered = ui.rect_contains_pointer(rect);
    let r = CornerRadius::same(6);
    let fill = if bordered {
        bg
    } else if hovered {
        hover_tint()
    } else {
        Color32::TRANSPARENT
    };
    let bg_idx = ui.painter().add(Shape::Noop);
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            let mut te = TextEdit::singleline(text)
                .font(f.clone())
                .hint_text(egui::RichText::new(hint).color(fg_3()).font(f))
                .frame(
                    egui::Frame::NONE
                        .inner_margin(Margin::symmetric(if bordered { 12 } else { 8 }, 0)),
                )
                .password(password)
                .align(egui::Align2::LEFT_CENTER);
            if let Some(l) = layouter {
                te = te.layouter(l);
            }
            ui.add_sized(rect.size(), te)
        })
        .inner;
    a11y_name(&resp, label);
    let focused = resp.has_focus();
    let p = ui.painter();
    // draw background *behind* the text by using a lower layer shape
    p.set(
        bg_idx,
        Shape::rect_filled(
            rect,
            r,
            if focused && bordered {
                bg_input_hover()
            } else {
                fill
            },
        ),
    );
    if bordered || focused {
        let c = if focused {
            accent()
        } else if hovered {
            border_strong()
        } else {
            border()
        };
        p.rect_stroke(rect.shrink(0.5), r, Stroke::new(1.0, c), StrokeKind::Inside);
    }
    resp
}

// ---- layout helpers ----

/// Runs `add` in a sub-ui covering exactly `rect`.
pub fn sub(ui: &mut Ui, rect: Rect, layout: egui::Layout, add: impl FnOnce(&mut Ui)) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(layout), |ui| {
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        add(ui);
    });
}

pub fn ltr() -> egui::Layout {
    egui::Layout::left_to_right(egui::Align::Center)
}

/// A full-width bar of height `h` with optional bottom border; returns its rect.
pub fn bar(ui: &mut Ui, h: f32, fill: Color32, border: Option<Color32>) -> Rect {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter().rect_filled(rect, 0, fill);
    if let Some(b) = border {
        hline(ui, rect.bottom() - 1.0, rect.left(), rect.right(), b);
    }
    rect
}

pub fn text_w(ui: &Ui, t: &str, f: FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(t.to_string(), f, Color32::WHITE)
        .size()
        .x
}

/// `42 ms` / `1.23 s` (format-utils `formatDuration`).
pub fn fmt_dur(ms: f64) -> String {
    nexus_core::response::format_duration(ms)
}

/// `512 B` / `1.5 KB` / `5.25 MB` (format-utils `formatSize`).
pub fn fmt_size(b: u64) -> String {
    nexus_core::response::format_size(b)
}

/// A small outline button whose visible text is short while assistive technology gets the
/// longer `accessible` name ("Delete" / "Delete environment Staging").
pub fn button_named(
    ui: &mut Ui,
    visible: &str,
    accessible: &str,
    st: &BtnStyle,
    min_h: f32,
) -> Response {
    let r = button(ui, visible, font(12.0, "Medium"), (10.0, 4.0), min_h, st);
    a11y(r, Role::Button, accessible)
}
