//! Request tab bar and the HTTP panel frame (tab bar + request builder + response viewer).

use crate::app::NexusApp;
use crate::state::ui_state::{MAX_SPLIT, MIN_SPLIT};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, Align2, CursorIcon, Layout, Rect, ScrollArea, Sense,
    Ui,
};

/// Smallest request / response pane heights (points).
const MIN_REQ_H: f32 = 290.0;
const MIN_RESP_H: f32 = 140.0;

impl NexusApp {
    pub(crate) fn http_panel(&mut self, ui: &mut Ui) {
        self.tab_bar(ui);
        if self.tabs.active().is_none() {
            self.empty_state(ui);
            return;
        }
        let area = ui.available_rect_before_wrap();
        let div_h = 5.0;
        let usable = area.height() - div_h;
        // keep both panes usable: the builder needs room for its URL bar, tabs and some content
        let (lo, hi) = if usable > MIN_REQ_H + MIN_RESP_H {
            (MIN_REQ_H, usable - MIN_RESP_H)
        } else {
            (usable * 0.6, usable * 0.6)
        };
        let req_h = (usable * self.ui.split).clamp(lo, hi).round();
        let req_rect = Rect::from_min_size(area.min, vec2(area.width(), req_h));
        let div_rect = Rect::from_min_size(
            pos2(area.left(), req_rect.bottom()),
            vec2(area.width(), div_h),
        );
        let resp_rect = Rect::from_min_max(pos2(area.left(), div_rect.bottom()), area.max);
        sub(ui, req_rect, Layout::top_down(Align::Min), |ui| {
            self.request_builder(ui)
        });
        // draggable divider
        let id = ui.scope_id().with("divider");
        let dr = ui
            .interact(div_rect, id, Sense::click_and_drag())
            .on_hover_cursor(CursorIcon::ResizeVertical);
        if dr.dragged() {
            if let Some(p) = dr.interact_pointer_pos() {
                self.ui.split = ((p.y - area.top()) / usable).clamp(MIN_SPLIT, MAX_SPLIT);
            }
        }
        let active = dr.hovered() || dr.dragged();
        a11y(dr, Role::Splitter, "Resize request and response panes");
        ui.painter()
            .rect_filled(div_rect, 0, if active { accent() } else { bg_surface() });
        hline(
            ui,
            div_rect.top(),
            div_rect.left(),
            div_rect.right(),
            border(),
        );
        hline(
            ui,
            div_rect.bottom() - 1.0,
            div_rect.left(),
            div_rect.right(),
            border(),
        );
        ui.painter().rect_filled(
            Rect::from_center_size(div_rect.center(), vec2(36.0, 3.0)),
            2,
            if active {
                fg()
            } else {
                fg_3().gamma_multiply(0.6)
            },
        );
        sub(ui, resp_rect, Layout::top_down(Align::Min), |ui| {
            self.response_viewer(ui)
        });
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0, bg_app());
        ui.painter().text(
            area.center() - vec2(0.0, 24.0),
            Align2::CENTER_CENTER,
            "No request open",
            font(14.0, "Regular"),
            fg_3(),
        );
        let r = Rect::from_center_size(area.center() + vec2(0.0, 14.0), vec2(120.0, 30.0));
        sub(ui, r, ltr(), |ui| {
            if button(
                ui,
                "New request",
                font(12.0, "Medium"),
                (12.0, 6.0),
                30.0,
                &BtnStyle::outline_accent(),
            )
            .clicked()
            {
                self.tabs.open_blank();
            }
        });
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        let r = bar(ui, 38.0, bg_surface(), Some(border()));
        let inner = Rect::from_min_max(
            pos2(r.left() + 8.0, r.top() + 3.0),
            pos2(r.right() - 40.0, r.bottom() - 5.0),
        );
        let mut close: Option<String> = None;
        let mut select: Option<String> = None;
        let active = self.tabs.active_id().map(str::to_string);
        sub(ui, inner, ltr(), |ui| {
            ScrollArea::horizontal()
                .id_salt("tabs")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 1.0;
                        for t in &self.tabs.tabs {
                            let is_active = active.as_deref() == Some(t.id.as_str());
                            let method = t.request.method.as_str();
                            let label = t.label();
                            let mf = font(10.0, "Bold");
                            let nf = font(12.0, "Regular");
                            let mw = text_w(ui, method, mf.clone());
                            let nw = text_w(ui, &label, nf.clone()).min(180.0);
                            let dirty_w = if t.dirty { 12.0 } else { 0.0 };
                            let w = 12.0 + mw + 8.0 + nw + 8.0 + dirty_w + 16.0 + 12.0;
                            let (rect, resp) =
                                ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                            let xr = Rect::from_center_size(
                                pos2(rect.right() - 12.0 - 8.0, rect.center().y),
                                vec2(16.0, 16.0),
                            );
                            let xresp = ui.interact(
                                xr,
                                ui.scope_id().with(("tabx", &t.id)),
                                Sense::click(),
                            );
                            if is_active {
                                ui.painter().rect_filled(rect, 4, active_tint());
                            } else if resp.hovered() {
                                ui.painter().rect_filled(rect, 4, hover_tint());
                            }
                            let p = ui.painter().with_clip_rect(rect);
                            p.text(
                                pos2(rect.left() + 12.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                method,
                                mf,
                                method_color(method),
                            );
                            let name_clip = Rect::from_min_max(
                                pos2(rect.left() + 12.0 + mw + 8.0, rect.top()),
                                pos2(rect.left() + 12.0 + mw + 8.0 + nw, rect.bottom()),
                            );
                            ui.painter().with_clip_rect(name_clip).text(
                                name_clip.left_center(),
                                Align2::LEFT_CENTER,
                                &label,
                                nf,
                                if is_active || resp.hovered() {
                                    fg()
                                } else {
                                    fg_2()
                                },
                            );
                            if t.dirty {
                                p.text(
                                    pos2(name_clip.right() + 8.0, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    "●",
                                    font(9.0, "Regular"),
                                    fg_2(),
                                );
                            }
                            if xresp.hovered() {
                                p.rect_filled(xr, 3, danger_subtle());
                            }
                            x_icon(
                                ui,
                                xr.center(),
                                3.2,
                                if xresp.hovered() { danger() } else { fg_3() },
                            );
                            let close_clicked = xresp.clicked();
                            a11y(xresp, Role::Button, &format!("Close {}", t.request.name))
                                .on_hover_cursor(CursorIcon::PointingHand);
                            let resp = a11y_selected(resp, Role::Tab, &label, is_active)
                                .on_hover_text(&label)
                                .on_hover_cursor(CursorIcon::PointingHand);
                            if t.dirty {
                                resp.ctx.accesskit_node_builder(resp.id, |n| {
                                    n.set_description("Unsaved changes");
                                });
                            }
                            if close_clicked {
                                close = Some(t.id.clone());
                            } else if resp.clicked() {
                                select = Some(t.id.clone());
                            }
                        }
                    });
                });
        });
        let pr =
            Rect::from_center_size(pos2(r.right() - 22.0, r.center().y - 1.0), vec2(28.0, 28.0));
        let presp = ui
            .interact(pr, ui.scope_id().with("newtab"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if presp.hovered() {
            ui.painter().rect_filled(pr, 4, hover_tint());
        }
        plus_icon(
            ui,
            pr.center(),
            5.5,
            if presp.hovered() { fg() } else { fg_3() },
        );
        if a11y(presp, Role::Button, "New request").clicked() {
            self.tabs.open_blank();
        }
        if let Some(id) = select {
            self.tabs.set_active(&id);
        }
        if let Some(id) = close {
            self.tabs.close(&id);
        }
    }
}
