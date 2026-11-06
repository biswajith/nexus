//! Bottom console panel: script logs and
//! HTTP/system entries with level chips, text filter, clear and auto-scroll.

use crate::app::NexusApp;
use crate::state::console::{level_label, source_label};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, Align2, Color32, FontFamily, FontId, Layout,
    ScrollArea, Sense, Ui,
};
use nexus_core::types::LogLevel;

const ROW_H: f32 = 20.0;

pub fn level_color(l: LogLevel) -> Color32 {
    match l {
        LogLevel::Info => Color32::from_rgb(0x3b, 0x82, 0xf6),
        LogLevel::Warn => Color32::from_rgb(0xf5, 0x9e, 0x0b),
        LogLevel::Error => Color32::from_rgb(0xef, 0x44, 0x44),
        LogLevel::Debug => Color32::from_rgb(0x8b, 0x5c, 0xf6),
    }
}

fn clock(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S%.3f")
                .to_string()
        })
        .unwrap_or_default()
}

impl NexusApp {
    pub(crate) fn console_panel(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0, bg_surface());
        hline(ui, full.top(), full.left(), full.right(), border());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let c = &mut self.console;

        let r = bar(ui, 36.0, bg_surface(), Some(border()));
        sub(ui, r.shrink2(vec2(10.0, 0.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new("Console")
                    .font(font(12.0, "SemiBold"))
                    .color(fg_2()),
            );
            ui.add_space(8.0);
            for l in [
                LogLevel::Info,
                LogLevel::Warn,
                LogLevel::Error,
                LogLevel::Debug,
            ] {
                let active = c.level == Some(l);
                let col = level_color(l);
                let st = BtnStyle {
                    fill: if active {
                        active_tint()
                    } else {
                        Color32::TRANSPARENT
                    },
                    fill_hover: if active { active_tint() } else { hover_tint() },
                    stroke: active.then_some(border()),
                    stroke_hover: active.then_some(border()),
                    text: col,
                    text_hover: col,
                    radius: 4,
                };
                if button_role(
                    ui,
                    level_label(l),
                    font(10.5, "Bold"),
                    (7.0, 2.0),
                    22.0,
                    &st,
                    Role::Button,
                    Some(active),
                )
                .clicked()
                {
                    c.level = if active { None } else { Some(l) };
                }
            }
            ui.add_space(6.0);
            field(
                ui,
                "Filter console",
                &mut c.search,
                240.0,
                24.0,
                font(12.0, "Regular"),
                "Filter console...",
                bg_input(),
                true,
            );
        });
        sub(
            ui,
            r.shrink2(vec2(10.0, 0.0)),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let clear = button(
                    ui,
                    "Clear",
                    font(11.0, "Medium"),
                    (10.0, 3.0),
                    24.0,
                    &BtnStyle::outline(),
                );
                a11y_name(&clear, "Clear console");
                if clear.clicked() {
                    c.clear();
                }
                let copy = button(
                    ui,
                    "Copy",
                    font(11.0, "Medium"),
                    (10.0, 3.0),
                    24.0,
                    &BtnStyle::outline(),
                )
                .on_hover_text("Copy the visible entries");
                a11y_name(&copy, "Copy console entries");
                if copy.clicked() {
                    let text: String = c
                        .visible()
                        .iter()
                        .map(|e| {
                            format!(
                                "{} {} {} {}\n",
                                clock(e.timestamp),
                                level_label(e.level),
                                source_label(e.source),
                                e.message
                            )
                        })
                        .collect();
                    ui.ctx().copy_text(text);
                }
                let mut auto = c.auto_scroll;
                if checkbox_labeled(ui, "Auto-scroll", &mut auto).changed() {
                    c.auto_scroll = auto;
                }
            },
        );

        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0, bg_sunken());
        let auto = c.auto_scroll;
        let empty_total = c.is_empty();
        let entries = c.visible();
        let n = entries.len();
        if n == 0 {
            let r = ui.allocate_rect(area, Sense::hover());
            let t = if empty_total {
                "Console is empty"
            } else {
                "No entries match the filter"
            };
            a11y(r, Role::Label, t);
            ui.painter().text(
                area.center_top() + vec2(0.0, 28.0),
                Align2::CENTER_CENTER,
                t,
                font(12.5, "Regular"),
                fg_3(),
            );
            return;
        }
        let mono = FontId::new(12.0, FontFamily::Monospace);
        ScrollArea::vertical()
            .id_salt("console_entries")
            .auto_shrink([false, false])
            .stick_to_bottom(auto)
            .show_rows(ui, ROW_H, n, |ui, range| {
                let w = ui.available_width();
                for i in range {
                    let e = &entries[i];
                    let (row, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
                    if resp.hovered() {
                        ui.painter().rect_filled(row, 0, hover_tint());
                    }
                    let p = ui.painter().with_clip_rect(row);
                    let y = row.center().y;
                    let col = level_color(e.level);
                    let mut x = row.left() + 12.0;
                    p.text(
                        pos2(x, y),
                        Align2::LEFT_CENTER,
                        clock(e.timestamp),
                        mono.clone(),
                        fg_3(),
                    );
                    x += 96.0;
                    p.text(
                        pos2(x, y),
                        Align2::LEFT_CENTER,
                        level_label(e.level),
                        FontId::new(11.0, mono_bold()),
                        col,
                    );
                    x += 44.0;
                    p.text(
                        pos2(x, y),
                        Align2::LEFT_CENTER,
                        source_label(e.source),
                        font(11.5, "Medium"),
                        fg_2(),
                    );
                    x += 54.0;
                    let first = e.message.lines().next().unwrap_or("");
                    let more = e.message.lines().count().saturating_sub(1);
                    let shown = if more > 0 {
                        format!("{first}  (+{more} lines)")
                    } else {
                        first.to_string()
                    };
                    p.text(pos2(x, y), Align2::LEFT_CENTER, shown, mono.clone(), fg());
                    a11y(
                        resp,
                        Role::ListItem,
                        &format!(
                            "{} {}: {}",
                            level_label(e.level),
                            source_label(e.source),
                            e.message
                        ),
                    );
                }
            });
    }
}

/// Painted checkbox with its text label to the right.
fn checkbox_labeled(ui: &mut Ui, label: &str, checked: &mut bool) -> egui::Response {
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font(11.5, "Regular"), fg_2());
    let lw = g.size().x;
    // right-to-left layout: text first, then the box to its left
    let (rect, _) = ui.allocate_exact_size(vec2(lw, 16.0), Sense::hover());
    ui.painter().galley(
        pos2(rect.left(), rect.center().y - g.size().y / 2.0),
        g,
        fg_2(),
    );
    checkbox(ui, label, checked)
}
