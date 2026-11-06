//! Smaller response tabs: error state, headers, tests, timing, visualizer.

use super::RespAction;
use crate::state::response::{ErrorKind, Resp};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText,
    ScrollArea, Sense, Stroke, Ui,
};
use nexus_core::response::build_timing_waterfall;
use nexus_core::scripts::TestResult;
use nexus_core::types::ResponseTiming;

fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

fn centered_note(ui: &mut Ui, label: &str, text: &str) {
    let area = ui.available_rect_before_wrap();
    let r = ui.allocate_rect(area, Sense::hover());
    a11y(r, Role::Label, label);
    ui.painter().text(
        area.center_top() + vec2(0.0, 48.0),
        Align2::CENTER_CENTER,
        text,
        font(12.5, "Regular"),
        fg_3(),
    );
}

// ---- error state ----

pub fn error_view(ui: &mut Ui, title: &str, kind: ErrorKind, message: &str) {
    let area = ui.available_rect_before_wrap();
    let (color, hint) = match kind {
        ErrorKind::Network => (
            danger(),
            "Check the URL, your network connection and any proxy settings, then send again.",
        ),
        ErrorKind::Timeout => (
            warning(),
            "The server did not answer in time. Try again, or check that it is reachable.",
        ),
        ErrorKind::Cancelled => (fg_2(), "Click Send to run it again."),
        ErrorKind::Script => (
            danger(),
            "Fix the pre-request script on the Scripts tab and send again.",
        ),
    };
    let w = (area.width() - 48.0).clamp(200.0, 640.0);
    let card = Rect::from_center_size(
        pos2(area.center().x, area.top() + 56.0 + 70.0),
        vec2(w, 140.0),
    );
    let _ = ui.allocate_rect(area, Sense::hover());
    sub(ui, card, Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        labeled_text(
            ui,
            title,
            &format!("Error: {title}"),
            font(15.0, "SemiBold"),
            color,
        );
        ui.label(
            RichText::new(hint)
                .font(font(12.5, "Regular"))
                .color(fg_2()),
        );
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(bg_sunken())
            .stroke(Stroke::new(1.0, border()))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(w - 22.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(message)
                            .font(mono(12.0))
                            .color(danger().gamma_multiply(0.9)),
                    )
                    .selectable(true),
                );
            });
    });
}

// ---- headers ----

pub fn headers_tab(ui: &mut Ui, resp: &Resp) {
    let r = bar(ui, 35.0, bg_surface(), Some(border_subtle()));
    sub(
        ui,
        r.shrink2(vec2(12.0, 0.0)),
        Layout::right_to_left(Align::Center),
        |ui| {
            let b = button(
                ui,
                "Copy headers",
                font(11.0, "Medium"),
                (10.0, 3.0),
                24.0,
                &BtnStyle::outline(),
            );
            if b.clicked() {
                let text: String = resp
                    .headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}\n"))
                    .collect();
                ui.ctx().copy_text(text);
            }
        },
    );
    let area = ui.available_rect_before_wrap();
    if resp.headers.is_empty() {
        centered_note(ui, "No headers in response", "No headers in response");
        return;
    }
    ScrollArea::vertical()
        .id_salt("resp_headers")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let w = area.width() - 24.0;
            let name_w = (w * 0.3).clamp(140.0, 260.0);
            ui.add_space(8.0);
            // header row
            let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
            let hr = hr.translate(vec2(12.0, 0.0));
            for (x, t) in [
                (hr.left() + 8.0, "NAME"),
                (hr.left() + name_w + 8.0, "VALUE"),
            ] {
                ui.painter().text(
                    pos2(x, hr.center().y),
                    Align2::LEFT_CENTER,
                    t,
                    font(10.5, "SemiBold"),
                    fg_3(),
                );
            }
            hline(ui, hr.bottom() - 1.0, hr.left(), hr.right(), border());
            for (k, v) in &resp.headers {
                let vg =
                    ui.painter()
                        .layout(v.clone(), mono(12.0), fg(), (w - name_w - 70.0).max(80.0));
                let h = (vg.size().y + 12.0).max(28.0);
                let (row, rresp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                let row = row.translate(vec2(12.0, 0.0));
                let hov = ui.rect_contains_pointer(row);
                if hov {
                    ui.painter().rect_filled(row, 4, hover_tint());
                }
                a11y(rresp, Role::Row, &format!("{k}: {v}"));
                ui.painter().text(
                    pos2(row.left() + 8.0, row.top() + 14.0),
                    Align2::LEFT_CENTER,
                    k,
                    FontId::new(12.0, mono_bold()),
                    accent(),
                );
                ui.painter()
                    .galley(pos2(row.left() + name_w + 8.0, row.top() + 6.0), vg, fg());
                hline(
                    ui,
                    row.bottom() - 1.0,
                    row.left(),
                    row.right(),
                    border_subtle(),
                );
                if hov {
                    let br = Rect::from_center_size(
                        pos2(row.right() - 30.0, row.top() + 14.0),
                        vec2(46.0, 20.0),
                    );
                    let id = ui.scope_id().with(("copy_header", k));
                    let resp = ui.interact(br, id, Sense::click());
                    ui.painter().rect_filled(
                        br,
                        4,
                        if resp.hovered() {
                            active_tint()
                        } else {
                            bg_elevated()
                        },
                    );
                    ui.painter().text(
                        br.center(),
                        Align2::CENTER_CENTER,
                        "Copy",
                        font(11.0, "Medium"),
                        fg(),
                    );
                    if a11y(resp, Role::Button, &format!("Copy header {k}")).clicked() {
                        ui.ctx().copy_text(v.clone());
                    }
                }
            }
        });
}

// ---- tests ----

pub fn tests_tab(ui: &mut Ui, tests: &[TestResult]) {
    if tests.is_empty() {
        centered_note(
            ui,
            "No test results. Add assertions in your post-response script using nx.test().",
            "No test results. Add assertions in your post-response script using nx.test().",
        );
        return;
    }
    let passed = tests.iter().filter(|t| t.passed).count();
    let failed = tests.len() - passed;
    let r = bar(ui, 40.0, bg_surface(), Some(border_subtle()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for (ok, n, label) in [(true, passed, "passed"), (false, failed, "failed")] {
            let text = format!("{n} {label}");
            let g = ui.painter().layout_no_wrap(
                text.clone(),
                font(12.5, "Medium"),
                if ok { success() } else { danger() },
            );
            let (rect, resp) =
                ui.allocate_exact_size(vec2(g.size().x + 20.0, 20.0), Sense::hover());
            let c = if ok { success() } else { danger() };
            if ok {
                check_icon(ui, pos2(rect.left() + 6.0, rect.center().y), 4.0, c);
            } else {
                x_icon(ui, pos2(rect.left() + 6.0, rect.center().y), 3.4, c);
            }
            ui.painter().galley(
                pos2(rect.left() + 18.0, rect.center().y - g.size().y / 2.0),
                g,
                c,
            );
            a11y(resp, Role::Label, &format!("Tests {text}"));
        }
        let total = format!("{} total", tests.len());
        labeled_text(
            ui,
            &total,
            &format!("Tests {total}"),
            font(12.0, "Regular"),
            fg_3(),
        );
    });
    ScrollArea::vertical()
        .id_salt("resp_tests")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let w = ui.available_width() - 24.0;
            ui.add_space(8.0);
            for t in tests {
                let eg = t.error.as_ref().map(|e| {
                    ui.painter()
                        .layout(e.clone(), mono(12.0), danger(), (w - 110.0).max(80.0))
                });
                let h = 30.0 + eg.as_ref().map_or(0.0, |g| g.size().y + 2.0);
                let (row, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                let row = row.translate(vec2(12.0, 0.0));
                let (c, bg) = if t.passed {
                    (success(), success_subtle().gamma_multiply(0.5))
                } else {
                    (danger(), danger_subtle().gamma_multiply(0.5))
                };
                ui.painter().rect_filled(row.shrink2(vec2(0.0, 1.0)), 4, bg);
                let ic = pos2(row.left() + 16.0, row.top() + 15.0);
                if t.passed {
                    check_icon(ui, ic, 4.5, c);
                } else {
                    x_icon(ui, ic, 3.8, c);
                }
                ui.painter().text(
                    pos2(row.left() + 34.0, row.top() + 15.0),
                    Align2::LEFT_CENTER,
                    &t.name,
                    font(12.5, "Medium"),
                    fg(),
                );
                ui.painter().text(
                    pos2(row.right() - 12.0, row.top() + 15.0),
                    Align2::RIGHT_CENTER,
                    test_duration(t.duration),
                    mono(11.0),
                    fg_3(),
                );
                let label = match &t.error {
                    Some(e) => format!("Test failed: {}: {e}", t.name),
                    None if t.passed => format!("Test passed: {}", t.name),
                    None => format!("Test failed: {}", t.name),
                };
                if let Some(g) = eg {
                    ui.painter()
                        .galley(pos2(row.left() + 34.0, row.top() + 28.0), g, danger());
                }
                a11y(resp, Role::ListItem, &label);
                ui.add_space(2.0);
            }
        });
}

// ---- timing ----

fn hex(s: &str) -> Color32 {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x888888);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn timing_tab(ui: &mut Ui, timing: &ResponseTiming) {
    let area = ui.available_rect_before_wrap();
    let _ = ui.allocate_rect(area, Sense::hover());
    let x0 = area.left() + 16.0;
    let mut y = area.top() + 16.0;
    // total row
    let total = fmt_dur(timing.total);
    let ttl = ui
        .painter()
        .layout_no_wrap("Total".into(), font(14.0, "SemiBold"), fg_2());
    ui.painter()
        .galley(pos2(x0, y + 28.0 - ttl.size().y), ttl.clone(), fg_2());
    ui.painter().text(
        pos2(x0 + ttl.size().x + 12.0, y + 31.0),
        Align2::LEFT_BOTTOM,
        &total,
        FontId::new(22.0, mono_bold()),
        accent(),
    );
    let tr = Rect::from_min_size(pos2(x0, y), vec2(area.width() - 32.0, 36.0));
    let resp = ui.interact(tr, ui.scope_id().with("timing_total"), Sense::hover());
    a11y(resp, Role::ListItem, &format!("Total: {total}"));
    y += 42.0;
    hline(ui, y, x0, area.right() - 16.0, border());
    y += 16.0;

    let max_ms = if timing.total > 0.0 {
        timing.total
    } else {
        1.0
    };
    let label_w = 80.0;
    let value_w = 76.0;
    let track_x = x0 + label_w + 12.0;
    let track_w = (area.right() - 16.0 - value_w - 12.0 - track_x).max(40.0);
    for seg in build_timing_waterfall(timing) {
        let track = Rect::from_min_size(pos2(track_x, y), vec2(track_w, 22.0));
        ui.painter().rect_filled(track, 4, bg_sunken());
        let left = (seg.start_ms / max_ms).clamp(0.0, 1.0) as f32 * track_w;
        let width = ((seg.duration_ms / max_ms * 100.0).max(1.0) / 100.0) as f32 * track_w;
        let bar = Rect::from_min_size(
            pos2(track_x + left, y),
            vec2(width.min(track_w - left).max(2.0), 22.0),
        );
        ui.painter().rect_filled(bar, 4, hex(seg.color));
        ui.painter().text(
            pos2(x0 + label_w, y + 11.0),
            Align2::RIGHT_CENTER,
            seg.label,
            font(12.5, "SemiBold"),
            fg_2(),
        );
        let value = fmt_dur(seg.duration_ms);
        ui.painter().text(
            pos2(track.right() + 12.0, y + 11.0),
            Align2::LEFT_CENTER,
            &value,
            mono(12.0),
            fg_2(),
        );
        let row = Rect::from_min_max(pos2(x0, y), pos2(area.right() - 16.0, y + 22.0));
        let resp = ui.interact(
            row,
            ui.scope_id().with(("timing", seg.label)),
            Sense::hover(),
        );
        a11y(resp, Role::ListItem, &format!("{}: {value}", seg.label));
        y += 32.0;
    }
}

// ---- visualizer ----

pub fn visualize_tab(ui: &mut Ui, html: &str, acts: &mut Vec<RespAction>) {
    external_panel(
        ui,
        "Visualization",
        "The visualization is rendered in your browser: there is no embedded web view.",
        "Open visualization in browser",
        acts,
        RespAction::OpenVisualizer(html.to_string()),
    );
}

/// Centered note + one button (preview, visualizer, binary bodies).
pub fn external_panel(
    ui: &mut Ui,
    title: &str,
    note: &str,
    button_label: &str,
    acts: &mut Vec<RespAction>,
    action: RespAction,
) {
    let area = ui.available_rect_before_wrap();
    ui.painter().rect_filled(area, 0, bg_sunken());
    let _ = ui.allocate_rect(area, Sense::hover());
    let c = pos2(area.center().x, area.top() + 70.0);
    let r = Rect::from_center_size(c, vec2((area.width() - 40.0).min(560.0), 120.0));
    sub(ui, r, Layout::top_down(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(
            RichText::new(title)
                .font(font(14.0, "SemiBold"))
                .color(fg()),
        );
        ui.label(
            RichText::new(note)
                .font(font(12.5, "Regular"))
                .color(fg_2()),
        );
        ui.add_space(6.0);
        if button(
            ui,
            button_label,
            font(12.0, "Medium"),
            (14.0, 6.0),
            30.0,
            &BtnStyle::outline_accent(),
        )
        .clicked()
        {
            acts.push(action);
        }
    });
}

/// `12.3ms`; sub-0.1 ms tests read `<0.1ms` rather than a misleading `0.0ms`.
fn test_duration(ms: f64) -> String {
    if ms > 0.0 && ms < 0.05 {
        "<0.1ms".to_string()
    } else {
        format!("{ms:.1}ms")
    }
}

#[cfg(test)]
mod duration_tests {
    use super::test_duration;

    #[test]
    fn formats_fast_tests() {
        assert_eq!(test_duration(0.0), "0.0ms");
        assert_eq!(test_duration(0.02), "<0.1ms");
        assert_eq!(test_duration(0.34), "0.3ms");
        assert_eq!(test_duration(12.34), "12.3ms");
    }
}
