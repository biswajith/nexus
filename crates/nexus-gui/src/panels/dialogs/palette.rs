//! Command palette (`CommandPalette.tsx`): fuzzy search over requests, history, panels and
//! actions. Keyboard: type, Up/Down, Enter, Esc. A11y: a text input that points at the selected
//! option of a listbox.

use crate::app::NexusApp;
use crate::state::dialogs::Command;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align2, Frame, Id, Key, Margin, Modal, Modifiers, Rect,
    ScrollArea, Sense, Stroke, TextEdit, Ui,
};

const ROW_H: f32 = 32.0;
const WIDTH: f32 = 620.0;

impl NexusApp {
    /// Returns true when the palette should close.
    pub(super) fn palette_dialog(&mut self, ctx: &egui::Context) -> bool {
        let (up, down, pgup, pgdn, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::PageUp),
                i.consume_key(Modifiers::NONE, Key::PageDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        let pal_state = &mut self.dlg.palette;
        if up {
            pal_state.step(-1);
        }
        if down {
            pal_state.step(1);
        }
        if pgup {
            pal_state.step(-8);
        }
        if pgdn {
            pal_state.step(8);
        }
        let mut chosen: Option<usize> = enter.then_some(pal_state.selected);

        let frame = Frame::new()
            .fill(bg_surface())
            .stroke(Stroke::new(1.0, border_strong()))
            .corner_radius(10)
            .inner_margin(Margin::same(0))
            .shadow(ctx.global_style().visuals.popup_shadow);
        let area =
            Modal::default_area(Id::unique("palette")).anchor(Align2::CENTER_TOP, vec2(0.0, 90.0));
        let m = Modal::new(Id::unique("palette"))
            .area(area)
            .frame(frame)
            .backdrop_color(egui::Color32::from_black_alpha(if pal().dark {
                130
            } else {
                90
            }))
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                self.palette_body(ui, &mut chosen);
            });
        a11y(m.response.clone(), Role::Dialog, "Command palette");
        ctx.accesskit_node_builder(m.response.id, |n| n.set_modal());

        if let Some(i) = chosen {
            let st = &self.dlg.palette;
            if let Some(&ci) = st.results.get(i) {
                let action = st.commands[ci].action.clone();
                self.dialog = None;
                self.run_command(action);
                return false;
            }
        }
        m.should_close()
    }

    fn palette_body(&mut self, ui: &mut Ui, chosen: &mut Option<usize>) {
        // search row
        let (sr, _) = ui.allocate_exact_size(vec2(WIDTH, 48.0), Sense::hover());
        search_icon(ui, pos2(sr.left() + 24.0, sr.center().y), fg_3());
        let input_rect = Rect::from_min_max(
            pos2(sr.left() + 44.0, sr.top() + 4.0),
            pos2(sr.right() - 12.0, sr.bottom() - 4.0),
        );
        let st = &mut self.dlg.palette;
        let mut query = std::mem::take(&mut st.query);
        let resp = ui
            .scope_builder(egui::UiBuilder::new().max_rect(input_rect), |ui| {
                ui.add_sized(
                    input_rect.size(),
                    TextEdit::singleline(&mut query)
                        .font(font(14.0, "Regular"))
                        .hint_text(
                            egui::RichText::new("Type a command or search requests...")
                                .color(fg_3())
                                .font(font(14.0, "Regular")),
                        )
                        .frame(Frame::NONE)
                        .margin(Margin::symmetric(0, 10))
                        .align(egui::Align2::LEFT_CENTER)
                        .desired_width(f32::INFINITY),
                )
            })
            .inner;
        a11y_name(&resp, "Search commands");
        if !resp.has_focus() {
            resp.request_focus();
        }
        st.query = query;
        st.refilter();
        hline(ui, sr.bottom(), sr.left(), sr.right(), border());

        // results
        let n = st.results.len();
        let selected = st.selected.min(n.saturating_sub(1));
        let scroll_sel = std::mem::take(&mut st.scroll_to_selected);
        let option_id = |i: usize| Id::unique(("palette-option", i));
        if let Some(c) = st.results.get(selected) {
            let _ = c;
            ui.ctx().accesskit_node_builder(resp.id, |nb| {
                nb.set_active_descendant(option_id(selected).accesskit_id());
            });
        }
        let h = (n.max(1) as f32 * ROW_H).min(ROW_H * 11.0);
        let (lr, list_resp) = ui.allocate_exact_size(vec2(WIDTH, h + 8.0), Sense::hover());
        a11y(list_resp, Role::ListBox, "Results");
        let mut hovered: Option<usize> = None;
        let mut click: Option<usize> = None;
        let moved = ui.input(|i| i.pointer.delta() != vec2(0.0, 0.0));
        let mut cui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(lr.shrink2(vec2(6.0, 4.0)))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        if n == 0 {
            cui.painter().text(
                lr.center(),
                Align2::CENTER_CENTER,
                "No matching commands",
                font(13.0, "Regular"),
                fg_3(),
            );
            a11y(
                cui.allocate_rect(lr, Sense::hover()),
                Role::Label,
                "No matching commands",
            );
        }
        let st = &self.dlg.palette;
        ScrollArea::vertical()
            .id_salt("palette-scroll")
            .auto_shrink([false, false])
            .show(&mut cui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let mut last_cat = "";
                for (pos, &ci) in st.results.iter().enumerate() {
                    let c = &st.commands[ci];
                    let show_cat = st.query.trim().is_empty() && c.category != last_cat;
                    last_cat = c.category;
                    if show_cat {
                        let (r, _) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
                        ui.painter().text(
                            pos2(r.left() + 8.0, r.center().y + 2.0),
                            Align2::LEFT_CENTER,
                            c.category.to_uppercase(),
                            font(10.5, "SemiBold"),
                            fg_3(),
                        );
                    }
                    let w = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
                    let r = ui.interact(rect, option_id(pos), Sense::CLICK);
                    let sel = pos == selected;
                    paint_row(ui, rect, c, sel, r.hovered());
                    if r.hovered() && moved {
                        hovered = Some(pos);
                    }
                    if r.clicked() {
                        click = Some(pos);
                    }
                    if sel && scroll_sel {
                        r.scroll_to_me(None);
                    }
                    let r = a11y_selected(r, Role::ListBoxOption, &c.accessible(), sel);
                    ui.ctx().accesskit_node_builder(r.id, |nb| {
                        nb.set_position_in_set(pos + 1);
                        nb.set_size_of_set(n);
                    });
                }
            });
        if let Some(i) = hovered {
            self.dlg.palette.selected = i;
        }
        if let Some(i) = click {
            *chosen = Some(i);
        }
        // footer
        let (fr, _) = ui.allocate_exact_size(vec2(WIDTH, 28.0), Sense::hover());
        hline(ui, fr.top(), fr.left(), fr.right(), border());
        let hint = "Up/Down to move   Enter to run   Esc to close";
        ui.painter().text(
            pos2(fr.left() + 14.0, fr.center().y),
            Align2::LEFT_CENTER,
            hint,
            font(11.0, "Regular"),
            fg_3(),
        );
        let count = format!("{n} result{}", if n == 1 { "" } else { "s" });
        ui.painter().text(
            pos2(fr.right() - 14.0, fr.center().y),
            Align2::RIGHT_CENTER,
            &count,
            font(11.0, "Regular"),
            fg_3(),
        );
    }
}

fn paint_row(ui: &Ui, rect: Rect, c: &Command, selected: bool, hovered: bool) {
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 6, accent_subtle());
        p.rect_filled(
            Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(2.0, rect.height() - 12.0)),
            1,
            accent(),
        );
    } else if hovered {
        p.rect_filled(rect, 6, hover_tint());
    }
    let mut x = rect.left() + 12.0;
    let cy = rect.center().y;
    let label_color = fg();
    // requests: coloured method prefix
    let label = if c.category == "Request" || c.category == "History" {
        match c.label.split_once(' ') {
            Some((m, rest)) if m.chars().all(|ch| ch.is_ascii_uppercase()) => {
                let r = p.text(
                    pos2(x, cy),
                    Align2::LEFT_CENTER,
                    m,
                    egui::FontId::new(11.0, mono_bold()),
                    method_color(m),
                );
                x += r.width().max(44.0) + 8.0;
                rest.to_string()
            }
            _ => c.label.clone(),
        }
    } else {
        c.label.clone()
    };
    let right = rect.right() - 12.0;
    let mut right_x = right;
    if let Some(k) = &c.shortcut {
        let r = p.text(
            pos2(right_x, cy),
            Align2::RIGHT_CENTER,
            k,
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
            fg_2(),
        );
        right_x -= r.width() + 12.0;
    } else if !selected || c.detail.is_empty() {
        let r = p.text(
            pos2(right_x, cy),
            Align2::RIGHT_CENTER,
            c.category,
            font(11.0, "Regular"),
            fg_3(),
        );
        right_x -= r.width() + 12.0;
    }
    let clip = p.with_clip_rect(Rect::from_min_max(
        pos2(x, rect.top()),
        pos2(right_x, rect.bottom()),
    ));
    let r = clip.text(
        pos2(x, cy),
        Align2::LEFT_CENTER,
        &label,
        font(13.0, "Medium"),
        label_color,
    );
    if !c.detail.is_empty() {
        clip.text(
            pos2(x + r.width() + 10.0, cy),
            Align2::LEFT_CENTER,
            &c.detail,
            font(11.5, "Regular"),
            fg_3(),
        );
    }
}
