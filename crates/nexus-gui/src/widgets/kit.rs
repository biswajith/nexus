//! Small pieces shared by the Runner / WebSocket / GraphQL / MCP panels: combo boxes, pills,
//! status dots, list rows, progress bar, a read-only text view and a tab strip.

use super::*;
use crate::highlight::Lang;
use crate::state::response::{LineCache, TextDoc};
use eframe::egui::{Align2, ComboBox, RichText};

/// A combo box labelled `label` (for assistive technology); returns the index of a newly picked
/// item.
pub fn combo_pick(
    ui: &mut Ui,
    salt: &str,
    label: &str,
    width: f32,
    current: &str,
    items: &[String],
) -> Option<usize> {
    let mut chosen = None;
    let combo = ComboBox::from_id_salt(salt)
        .width(width - 24.0)
        .selected_text(
            RichText::new(current)
                .font(font(12.5, "Regular"))
                .color(fg()),
        )
        .show_ui(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                if ui
                    .selectable_label(
                        item == current,
                        RichText::new(item).font(font(12.5, "Regular")),
                    )
                    .clicked()
                {
                    chosen = Some(i);
                }
            }
        });
    combo.response.widget_info(|| WidgetInfo {
        label: Some(label.into()),
        current_text_value: Some(current.into()),
        ..WidgetInfo::new(Role::ComboBox)
    });
    chosen
}

/// Upper-case section caption bar.
pub fn section_header(ui: &mut Ui, text: &str) {
    let r = bar(ui, 26.0, bg_surface(), Some(border_subtle()));
    spaced_text(
        ui,
        pos2(r.left() + 12.0, r.center().y),
        &text.to_uppercase(),
        font(11.0, "SemiBold"),
        fg_3(),
        0.5,
        false,
    );
    let id = ui.scope_id().with(("section", text));
    a11y(ui.interact(r, id, Sense::hover()), Role::Heading, text);
}

/// Small caption in front of a control.
pub fn field_label(ui: &mut Ui, text: &str) {
    labeled_text(ui, text, text, font(11.5, "Medium"), fg_2());
}

/// A rounded text pill; `accessible` is what assistive technology reads.
pub fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32, accessible: &str) -> Response {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), font(11.5, "SemiBold"), fg);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 16.0, 20.0), Sense::hover());
    ui.painter().rect_filled(r, 4, bg);
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
    a11y(resp, Role::Label, accessible)
}

/// A coloured status dot.
pub fn status_dot(ui: &mut Ui, color: Color32, accessible: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 4.5, color);
    a11y(resp, Role::Label, accessible)
}

/// Centered hint in the space left in `ui`.
pub fn note(ui: &mut Ui, label: &str, text: &str) {
    let area = ui.available_rect_before_wrap();
    let r = ui.allocate_rect(area, Sense::hover());
    a11y(r, Role::Label, label);
    ui.painter().text(
        area.center(),
        Align2::CENTER_CENTER,
        text,
        font(13.0, "Regular"),
        fg_3(),
    );
}

/// Horizontal progress bar showing `frac` (0..=1).
pub fn progress_bar(ui: &mut Ui, label: &str, frac: f32, width: f32) -> Response {
    let frac = frac.clamp(0.0, 1.0);
    let (r, resp) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    ui.painter().rect_filled(r, 3, bg_sunken());
    let mut fill = r;
    fill.set_width((r.width() * frac).max(if frac > 0.0 { 6.0 } else { 0.0 }));
    ui.painter().rect_filled(fill, 3, accent());
    resp.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, label));
    resp.ctx.accesskit_node_builder(resp.id, |n| {
        n.set_min_numeric_value(0.0);
        n.set_max_numeric_value(100.0);
        n.set_numeric_value((frac * 100.0).round() as f64);
    });
    resp
}

/// A full-width selectable list row of height `h`. `paint` draws the content into the row rect.
pub fn list_row(
    ui: &mut Ui,
    h: f32,
    selected: bool,
    label: &str,
    paint: impl FnOnce(&Ui, Rect),
) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0, accent_subtle());
        p.rect_filled(
            Rect::from_min_size(rect.min, vec2(2.0, rect.height())),
            0,
            accent(),
        );
    } else if resp.hovered() {
        p.rect_filled(rect, 0, hover_tint());
    }
    paint(ui, rect);
    hline(
        ui,
        rect.bottom() - 1.0,
        rect.left(),
        rect.right(),
        border_subtle(),
    );
    resp.widget_info(|| WidgetInfo::selected(Role::ListItem, true, selected, label));
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Draws `text` left-aligned at `x` (centred vertically in `rect`), clipped to `rect`; returns
/// the right edge of the text.
pub fn row_text(ui: &Ui, rect: Rect, x: f32, text: &str, f: FontId, color: Color32) -> f32 {
    let p = ui.painter().with_clip_rect(rect);
    let g = p.layout_no_wrap(text.to_string(), f, color);
    let w = g.size().x;
    p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color);
    x + w
}

/// Like [`row_text`] but right-aligned to `x_right`; returns the left edge.
pub fn row_text_right(
    ui: &Ui,
    rect: Rect,
    x_right: f32,
    text: &str,
    f: FontId,
    color: Color32,
) -> f32 {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), f.clone(), color);
    let x = x_right - g.size().x;
    row_text(ui, rect, x, text, f, color);
    x
}

/// Underlined tab strip in a bar; returns the newly selected item.
pub fn tab_strip<T: Copy + PartialEq>(ui: &mut Ui, items: &[(T, String)], current: T) -> Option<T> {
    let mut picked = None;
    let r = bar(ui, 33.0, bg_surface(), Some(border()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        for (t, l) in items {
            if underline_tab(ui, l, current == *t, 33.0, None).clicked() {
                picked = Some(*t);
            }
        }
    });
    picked
}

/// A read-only, virtualised, highlighted text view that owns its document and line cache.
#[derive(Default)]
pub struct TextView {
    doc: Option<TextDoc>,
    cache: LineCache,
}

impl TextView {
    pub fn new(text: String) -> Self {
        let mut v = Self::default();
        v.set(text);
        v
    }

    pub fn set(&mut self, text: String) {
        self.doc = Some(TextDoc::new(text));
        self.cache.clear();
    }

    pub fn text(&self) -> &str {
        self.doc.as_ref().map_or("", |d| d.text.as_str())
    }

    pub fn show(&mut self, ui: &mut Ui, label: &str, lang: Lang) {
        let doc = self.doc.get_or_insert_with(|| TextDoc::new(String::new()));
        code_view(
            ui,
            doc,
            &mut self.cache,
            &ViewOpts {
                label,
                lang,
                matches: &[],
                current: None,
                jump: false,
            },
        );
    }
}

/// Pretty-printed JSON text of a value.
pub fn pretty(v: &serde_json::Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// `HH:MM:SS` local time of an epoch-milliseconds timestamp.
pub fn clock(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

/// A draggable splitter dividing `area` into two panes: side by side (`vertical_bar`) or one
/// above the other. `frac` is the first pane's share of the area and is updated by dragging
/// (keyboard: arrow keys while focused). Returns the two pane rects. `Role::Splitter`.
pub fn split(
    ui: &mut Ui,
    area: Rect,
    side_by_side: bool,
    frac: &mut f32,
    min_a: f32,
    min_b: f32,
    label: &str,
) -> (Rect, Rect) {
    let total = if side_by_side {
        area.width()
    } else {
        area.height()
    };
    let size_a = if total < min_a + min_b {
        total * 0.5
    } else {
        (total * *frac).clamp(min_a, total - min_b)
    }
    .round();
    let (a, b, bar) = if side_by_side {
        let x = area.left() + size_a;
        (
            Rect::from_min_max(area.min, pos2(x, area.max.y)),
            Rect::from_min_max(pos2(x, area.min.y), area.max),
            Rect::from_min_max(pos2(x - 3.0, area.min.y), pos2(x + 3.0, area.max.y)),
        )
    } else {
        let y = area.top() + size_a;
        (
            Rect::from_min_max(area.min, pos2(area.max.x, y)),
            Rect::from_min_max(pos2(area.min.x, y), area.max),
            Rect::from_min_max(pos2(area.min.x, y - 3.0), pos2(area.max.x, y + 3.0)),
        )
    };
    let id = ui.scope_id().with(("split", label));
    let resp = ui
        .interact(bar, id, Sense::click_and_drag())
        .on_hover_cursor(if side_by_side {
            CursorIcon::ResizeHorizontal
        } else {
            CursorIcon::ResizeVertical
        });
    if resp.dragged() {
        if let Some(p) = resp.interact_pointer_pos() {
            let v = if side_by_side {
                (p.x - area.left()) / total
            } else {
                (p.y - area.top()) / total
            };
            *frac = v.clamp(0.15, 0.85);
        }
    }
    if resp.has_focus() {
        let step = ui.input(|i| {
            (i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowDown)) as i32
                - (i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowUp)) as i32
        });
        if step != 0 {
            *frac = (*frac + step as f32 * 0.02).clamp(0.15, 0.85);
        }
    }
    if resp.hovered() || resp.dragged() {
        let r = if side_by_side {
            Rect::from_center_size(bar.center(), vec2(2.0, bar.height()))
        } else {
            Rect::from_center_size(bar.center(), vec2(bar.width(), 2.0))
        };
        ui.painter().rect_filled(r, 0, accent());
    }
    a11y(resp, Role::Splitter, label);
    (a, b)
}
