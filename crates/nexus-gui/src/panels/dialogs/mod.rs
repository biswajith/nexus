//! Modal dialogs: Import/Export, Docs, Environment manager, command palette, shortcut help.

mod docs;
mod envs;
mod help;
mod import;
mod palette;

use crate::app::NexusApp;
use crate::state::dialogs::Dialog;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{self, vec2, Color32, RichText, Ui};

/// Wrapped paragraph text.
pub(super) fn para(ui: &mut Ui, text: &str, size: f32, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text).font(font(size, "Regular")).color(color)).wrap());
}

/// An error / warning card with a coloured left edge.
pub(super) fn card(ui: &mut Ui, title: &str, color: Color32, lines: &[String]) {
    let w = ui.available_width();
    let r = ui
        .scope(|ui| {
            ui.set_width(w);
            egui::Frame::new()
                .fill(bg_sunken())
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(w - 24.0);
                    ui.spacing_mut().item_spacing.y = 3.0;
                    labeled_text(ui, title, title, font(12.0, "SemiBold"), color);
                    for l in lines {
                        para(ui, l, 12.0, fg_2());
                    }
                });
        })
        .response;
    let rect = r.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
        2,
        color,
    );
}

impl NexusApp {
    /// Draws the open dialog (if any) on top of the app.
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(d) = self.dialog else {
            return;
        };
        let close = match d {
            Dialog::Import => modal(ctx, "Import / Export", 680.0, |ui| self.import_dialog(ui)),
            Dialog::Docs => modal(ctx, "Generate API Documentation", 760.0, |ui| {
                self.docs_dialog(ui)
            }),
            Dialog::Envs => {
                let title = match &self.dlg.envs.draft {
                    Some(d) => format!("Edit: {}", d.name),
                    None => "Manage Environments".to_string(),
                };
                modal(ctx, &title, 720.0, |ui| self.envs_dialog(ui))
            }
            Dialog::Palette => self.palette_dialog(ctx),
            Dialog::Help => modal(ctx, "Keyboard Shortcuts", 560.0, |ui| self.help_dialog(ui)),
        };
        if close && self.dialog == Some(d) {
            self.close_dialog();
        }
    }
}
