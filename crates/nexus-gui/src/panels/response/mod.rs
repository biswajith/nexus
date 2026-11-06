//! Response viewer: status bar, tab strip and the tab contents.

mod body;
mod parts;

use crate::app::NexusApp;
use crate::state::response::{ErrorKind, Resp};
use crate::state::RespTab;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{self, accesskit::Role, vec2, Align2, Sense, Ui};

/// Things the drawing code asks the app to do once the response borrow is released.
pub(crate) enum RespAction {
    Save,
    OpenPreview(String),
    OpenVisualizer(String),
    OpenExternal,
}

impl NexusApp {
    pub(crate) fn response_viewer(&mut self, ui: &mut Ui) {
        ui.painter().rect_filled(ui.max_rect(), 0, bg_app());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let Some(idx) = self.tabs.active_index() else {
            return;
        };
        let tab_id = self.tabs.tabs[idx].id.clone();
        // scroll areas and caches are per tab
        ui.push_id(tab_id, |ui| self.response_inner(ui, idx));
    }

    fn response_inner(&mut self, ui: &mut Ui, idx: usize) {
        let area = ui.max_rect();
        let tab = &mut self.tabs.tabs[idx];
        if tab.loading {
            ui.painter().text(
                area.center() + vec2(0.0, 14.0),
                Align2::CENTER_CENTER,
                "Sending request...",
                font(14.0, "Regular"),
                fg_3(),
            );
            let sp = egui::Spinner::new().size(22.0).color(accent());
            ui.put(
                egui::Rect::from_center_size(area.center() - vec2(0.0, 14.0), vec2(22.0, 22.0)),
                sp,
            );
            let r = ui.interact(area, ui.scope_id().with("loading"), Sense::hover());
            a11y(r, Role::Label, "Sending request");
            return;
        }
        let Some(resp) = tab.response.as_mut() else {
            let hint = |ui: &Ui, dy: f32, t: &str, size: f32| {
                ui.painter().text(
                    area.center() + vec2(0.0, dy),
                    Align2::CENTER_CENTER,
                    t,
                    font(size, "Regular"),
                    fg_3(),
                );
            };
            hint(ui, -8.0, "Click Send to get a response", 14.0);
            hint(ui, 12.0, "Or press Enter in the URL bar", 12.0);
            let r = ui.interact(area, ui.scope_id().with("empty"), Sense::hover());
            a11y(
                r,
                Role::Label,
                "No response yet. Click Send to get a response",
            );
            return;
        };

        status_bar(ui, resp);
        if let Some(e) = resp.error.clone() {
            parts::error_view(ui, e.title(), e.kind, &e.message);
            return;
        }

        // tab strip
        let tests_label = if resp.tests.is_empty() {
            "Tests".to_string()
        } else {
            let passed = resp.tests.iter().filter(|t| t.passed).count();
            format!("Tests ({}/{})", passed, resp.tests.len())
        };
        let mut items = vec![
            (RespTab::Body, "Body".to_string()),
            (
                RespTab::Headers,
                format!("Headers ({})", resp.headers.len()),
            ),
            (RespTab::Tests, tests_label),
            (RespTab::Timing, "Timing".to_string()),
        ];
        if resp.visualizer_html.is_some() {
            items.push((RespTab::Visualize, "Visualize".to_string()));
        }
        if !items.iter().any(|(t, _)| *t == self.view.resp_tab) {
            self.view.resp_tab = RespTab::Body;
        }
        let r = bar(ui, 33.0, bg_surface(), Some(border()));
        sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
            for (t, l) in &items {
                if underline_tab(ui, l, self.view.resp_tab == *t, 33.0, None).clicked() {
                    self.view.resp_tab = *t;
                }
            }
        });

        let mut acts: Vec<RespAction> = Vec::new();
        match self.view.resp_tab {
            RespTab::Body => body::body_tab(ui, resp, &mut acts),
            RespTab::Headers => parts::headers_tab(ui, resp),
            RespTab::Tests => parts::tests_tab(ui, &resp.tests),
            RespTab::Timing => parts::timing_tab(ui, &resp.timing),
            RespTab::Visualize => {
                if let Some(h) = &resp.visualizer_html {
                    parts::visualize_tab(ui, h, &mut acts);
                }
            }
        }
        for a in acts {
            self.run_resp_action(idx, a);
        }
    }

    fn run_resp_action(&mut self, idx: usize, a: RespAction) {
        let notice = match a {
            RespAction::Save => {
                self.save_response_dialog();
                None
            }
            RespAction::OpenPreview(html) => Some(
                self.open_temp(
                    "preview",
                    "html",
                    crate::preview::preview_html(&html).as_bytes(),
                )
                .map(|_| "Opened in your browser".to_string()),
            ),
            RespAction::OpenVisualizer(html) => Some(
                self.open_temp(
                    "visualize",
                    "html",
                    crate::preview::visualizer_html(&html).as_bytes(),
                )
                .map(|_| "Opened in your browser".to_string()),
            ),
            RespAction::OpenExternal => {
                let data = self.tabs.tabs[idx]
                    .response
                    .as_ref()
                    .and_then(|r| r.binary.as_ref())
                    .map(|b| (b.bytes.clone(), crate::preview::ext_for(&b.content_type)));
                data.map(|(bytes, ext)| {
                    self.open_temp("response", ext, &bytes)
                        .map(|_| "Opened with the default application".to_string())
                })
            }
        };
        if let (Some(n), Some(r)) = (notice, self.tabs.tabs[idx].response.as_mut()) {
            r.view.notice = Some(n.unwrap_or_else(|e| format!("Could not open: {e}")));
        }
    }
}

fn status_bar(ui: &mut Ui, resp: &Resp) {
    let r = bar(ui, 39.0, bg_surface(), Some(border()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let (label, (bg, fg)) = match &resp.error {
            Some(e) if e.kind == ErrorKind::Cancelled => {
                (e.pill().to_string(), (warning_subtle(), warning()))
            }
            Some(e) => (e.pill().to_string(), (danger_subtle(), danger())),
            None => (
                format!("{} {}", resp.status, resp.status_text),
                status_colors(resp.status),
            ),
        };
        let g = ui
            .painter()
            .layout_no_wrap(label.clone(), font(12.0, "Bold"), fg);
        let (pr, presp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 23.0), Sense::hover());
        a11y(presp, Role::Label, &format!("Response status: {label}"));
        ui.painter().rect_filled(pr, 4, bg);
        ui.painter().galley(pr.center() - g.size() / 2.0, g, fg);
        if resp.error.is_none() {
            let mono = egui::FontId::new(12.0, egui::FontFamily::Monospace);
            let meta = |ui: &mut Ui, what: &str, text: String| {
                labeled_text(ui, &text, &format!("{what}: {text}"), mono.clone(), fg_2());
            };
            meta(ui, "Response time", fmt_dur(resp.timing.total));
            meta(ui, "Response size", fmt_size(resp.size));
        }
    });
}
