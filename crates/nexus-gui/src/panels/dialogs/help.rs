//! Keyboard shortcut overview.

use crate::app::NexusApp;
use crate::shortcuts::help_groups;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, vec2, Sense, Ui};

impl NexusApp {
    pub(super) fn help_dialog(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.y = 2.0;
        let cols = 2;
        let w = (ui.available_width() - 24.0) / cols as f32;
        let groups = help_groups();
        for pair in groups.chunks(cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 24.0;
                for (title, rows) in pair {
                    ui.vertical(|ui| {
                        ui.set_width(w);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        labeled_text(
                            ui,
                            &title.to_uppercase(),
                            title,
                            font(11.0, "SemiBold"),
                            fg_3(),
                        );
                        ui.add_space(2.0);
                        for (keys, what) in rows {
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                            ui.painter().text(
                                r.left_center(),
                                egui::Align2::LEFT_CENTER,
                                *what,
                                font(12.5, "Regular"),
                                fg(),
                            );
                            let kw = text_w(
                                ui,
                                keys,
                                egui::FontId::new(11.0, egui::FontFamily::Monospace),
                            );
                            let kr = egui::Rect::from_min_size(
                                egui::pos2(r.right() - kw - 12.0, r.top() + 2.0),
                                vec2(kw + 12.0, 18.0),
                            );
                            ui.painter().rect_filled(kr, 4, bg_sunken());
                            ui.painter().text(
                                kr.center(),
                                egui::Align2::CENTER_CENTER,
                                keys,
                                egui::FontId::new(11.0, egui::FontFamily::Monospace),
                                fg_2(),
                            );
                            a11y(resp, Role::ListItem, &format!("{what}: {keys}"));
                        }
                        ui.add_space(8.0);
                    });
                }
            });
        }
    }
}
