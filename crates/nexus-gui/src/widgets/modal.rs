//! Modal dialogs: a backdrop that blocks the rest of the UI, focus kept inside (egui's `Modal`),
//! Esc / backdrop click to close, a header with a Close button.

use super::*;
use eframe::egui::{Frame, Id, Margin, Modal};

/// Shows a modal dialog titled `title` (also its accessible name). Returns true when it should
/// close (Esc, backdrop click or the Close button).
pub fn modal(ctx: &egui::Context, title: &str, width: f32, content: impl FnOnce(&mut Ui)) -> bool {
    let frame = Frame::new()
        .fill(bg_surface())
        .stroke(Stroke::new(1.0, border_strong()))
        .corner_radius(10)
        .inner_margin(Margin::same(0))
        .shadow(ctx.global_style().visuals.popup_shadow);
    let mut close = false;
    let m = Modal::new(Id::unique(("dialog", title)))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(if pal().dark { 130 } else { 90 }))
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            // header
            let (hr, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
            hline(ui, hr.bottom() - 1.0, hr.left(), hr.right(), border());
            let t = ui
                .painter()
                .layout_no_wrap(title.to_string(), font(14.0, "SemiBold"), fg());
            let ty = hr.center().y - t.size().y / 2.0;
            let tr = Rect::from_min_size(pos2(hr.left() + 16.0, ty), t.size());
            ui.painter().galley(tr.min, t, fg());
            let id = ui.scope_id().with(("title", title));
            a11y(ui.interact(tr, id, Sense::hover()), Role::Heading, title);
            let cr =
                Rect::from_center_size(pos2(hr.right() - 24.0, hr.center().y), vec2(28.0, 28.0));
            sub(ui, cr, ltr(), |ui| {
                if icon_button(ui, "Close", 28.0, |ui, c, col| x_icon(ui, c, 5.0, col)).clicked() {
                    close = true;
                }
            });
            // body
            Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(width - 32.0);
                content(ui);
            });
        });
    a11y(m.response.clone(), Role::Dialog, title);
    m.response
        .ctx
        .accesskit_node_builder(m.response.id, |n| n.set_modal());
    close || m.should_close()
}
