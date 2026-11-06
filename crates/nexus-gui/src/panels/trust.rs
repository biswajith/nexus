//! Banner shown on untrusted (imported / synced) collections, with the "Trust" action.

use crate::app::NexusApp;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{vec2, Align, Align2, Layout, Ui};

impl NexusApp {
    /// Draws the banner when the collection in `dir` is untrusted; trusts it on click.
    pub(crate) fn untrusted_banner(&mut self, ui: &mut Ui, dir: &str) {
        if !self.collections.is_untrusted(dir) {
            return;
        }
        let scripts = self
            .collections
            .loaded
            .get(dir)
            .map_or(0, nexus_core::trust::script_count);
        let text = if self.collections.changed.contains(dir) {
            format!("Scripts or requests changed since you trusted this collection: its {scripts} script(s) are disabled and secret variables are withheld.")
        } else {
            format!("Untrusted collection (imported or synced): its {scripts} script(s) are disabled and secret variables are withheld.")
        };
        let r = bar(ui, 38.0, warning_subtle(), Some(border()));
        let btn_w = text_w(ui, "Trust this collection", font(12.0, "Medium")) + 24.0;
        sub(
            ui,
            r.shrink2(vec2(12.0, 0.0)),
            Layout::right_to_left(Align::Center),
            |ui| {
                let trust = button(
                    ui,
                    "Trust this collection",
                    font(12.0, "Medium"),
                    (12.0, 6.0),
                    28.0,
                    &BtnStyle::outline_accent(),
                );
                if trust.clicked() {
                    self.collections.trust(&self.app, dir);
                }
            },
        );
        ui.painter().text(
            vec2(r.left() + 12.0, r.center().y).to_pos2(),
            Align2::LEFT_CENTER,
            fit(ui, &text, r.width() - btn_w - 36.0),
            font(12.0, "Regular"),
            warning(),
        );
    }
}

/// `text` shortened with an ellipsis to fit `max_w`.
fn fit(ui: &Ui, text: &str, max_w: f32) -> String {
    if text_w(ui, text, font(12.0, "Regular")) <= max_w {
        return text.to_string();
    }
    let mut t: String = text.to_string();
    while t.pop().is_some() && text_w(ui, &format!("{t}..."), font(12.0, "Regular")) > max_w {}
    format!("{t}...")
}
