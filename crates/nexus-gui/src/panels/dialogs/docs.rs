//! API documentation dialog (`DocsModal.tsx`): Markdown in a read-only viewer, HTML via browser.

use super::{card, para};
use crate::app::NexusApp;
use crate::highlight::Lang;
use crate::state::dialogs::DocFormat;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, vec2, Align, Layout, Sense, Ui};

impl NexusApp {
    pub(super) fn docs_dialog(&mut self, ui: &mut Ui) {
        let cols: Vec<(String, String)> = self
            .collections
            .metas
            .iter()
            .map(|m| (m.name.clone(), crate::state::collections::dir_name(m)))
            .collect();
        if cols.is_empty() {
            para(
                ui,
                "No collections. Import or create a collection first.",
                12.5,
                fg_2(),
            );
            return;
        }
        let names: Vec<String> = cols.iter().map(|c| c.0.clone()).collect();
        let cur = self
            .dlg
            .docs
            .dir
            .as_ref()
            .and_then(|d| cols.iter().find(|c| &c.1 == d))
            .map_or("Select collection...", |c| c.0.as_str())
            .to_string();
        let mut generate = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            field_label(ui, "Collection");
            if let Some(i) = combo_pick(ui, "docs-collection", "Collection", 240.0, &cur, &names) {
                self.dlg.docs.dir = Some(cols[i].1.clone());
                self.dlg.docs.content = None;
            }
            field_label(ui, "Format");
            let fmts = ["Markdown".to_string(), "HTML".to_string()];
            if let Some(i) = combo_pick(
                ui,
                "docs-format",
                "Format",
                130.0,
                self.dlg.docs.format.label(),
                &fmts,
            ) {
                self.dlg.docs.format = if i == 0 {
                    DocFormat::Markdown
                } else {
                    DocFormat::Html
                };
                self.dlg.docs.content = None;
            }
            if button(
                ui,
                "Generate",
                font(12.0, "Medium"),
                (14.0, 6.0),
                28.0,
                &BtnStyle::primary(accent(), accent_hover()),
            )
            .clicked()
            {
                generate = true;
            }
        });
        if generate {
            self.docs_generate();
        }
        if let Some(e) = self.dlg.docs.error.clone() {
            card(ui, "Error", danger(), &[e]);
        }
        let Some(content) = self.dlg.docs.content.clone() else {
            let (r, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
            ui.painter().rect_filled(r, 6, bg_sunken());
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "Select a collection and click \"Generate\" to create documentation",
                font(12.5, "Regular"),
                fg_3(),
            );
            a11y(resp, Role::Label, "No documentation generated yet");
            return;
        };
        let html = self.dlg.docs.format == DocFormat::Html;
        let mut copy = false;
        let mut save = false;
        let mut browser = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let o = BtnStyle::outline();
            if html
                && button(
                    ui,
                    "Open in browser",
                    font(12.0, "Medium"),
                    (12.0, 5.0),
                    26.0,
                    &o,
                )
                .clicked()
            {
                browser = true;
            }
            if button(ui, "Copy", font(12.0, "Medium"), (12.0, 5.0), 26.0, &o).clicked() {
                copy = true;
            }
            if button(ui, "Save...", font(12.0, "Medium"), (12.0, 5.0), 26.0, &o).clicked() {
                save = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let n = content.lines().count();
                let t = format!("{} lines, {}", n, fmt_size(content.len() as u64));
                labeled_text(ui, &t, &t, font(11.5, "Regular"), fg_3());
            });
        });
        if copy {
            ui.ctx().copy_text(content);
            self.dlg.docs.notice = Some("Copied to the clipboard".into());
        }
        if save {
            self.docs_save_dialog();
        }
        if browser {
            self.docs_open_browser();
        }
        if let Some(n) = self.dlg.docs.notice.clone() {
            labeled_text(ui, &n, &n, font(12.0, "Regular"), success());
        }
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 340.0), Sense::hover());
        let lang = if html { Lang::Html } else { Lang::Text };
        let label = if html {
            "Generated HTML"
        } else {
            "Generated Markdown"
        };
        let view = &mut self.dlg.docs.view;
        sub(ui, rect, Layout::top_down(Align::Min), |ui| {
            view.show(ui, label, lang)
        });
    }
}
