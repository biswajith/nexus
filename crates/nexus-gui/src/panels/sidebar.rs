//! Left sidebar: Collections tree (with CRUD) and History list.

use crate::app::NexusApp;
use crate::state::collections::dir_name;
use crate::state::SidebarTab;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align2, CursorIcon, Popup, Rect, ScrollArea, Sense,
    TextEdit, Ui,
};
use nexus_core::collections::CollectionMeta;
use nexus_core::types::{create_default_request, CollectionItem, HistoryEntry, NexusRequest};

enum Action {
    ToggleCollection(CollectionMeta),
    ToggleFolder(String),
    Open(String, Box<NexusRequest>),
    AddRequest(CollectionMeta),
    SaveTabTo(String),
    Rename(CollectionMeta),
    Delete(CollectionMeta),
    DeleteRequest(String, String),
}

impl NexusApp {
    pub(crate) fn sidebar(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let r = bar(ui, 36.0, bg_surface(), Some(border()));
        sub(ui, r, ltr(), |ui| {
            let half = r.width() / 2.0;
            for (t, label) in [
                (SidebarTab::Collections, "Collections"),
                (SidebarTab::History, "History"),
            ] {
                if underline_tab(ui, label, self.ui.sidebar_tab == t, 36.0, Some(half)).clicked() {
                    self.ui.sidebar_tab = t;
                }
            }
        });
        match self.ui.sidebar_tab {
            SidebarTab::Collections => self.collection_tree(ui),
            SidebarTab::History => self.history_list(ui),
        }
    }

    fn collection_tree(&mut self, ui: &mut Ui) {
        let r = bar(ui, 35.0, bg_surface(), Some(border_subtle()));
        spaced_text(
            ui,
            pos2(r.left() + 12.0, r.center().y),
            "COLLECTIONS",
            font(11.0, "SemiBold"),
            fg_3(),
            0.66,
            false,
        );
        let plus = Rect::from_center_size(pos2(r.right() - 24.0, r.center().y), vec2(24.0, 24.0));
        let resp = ui
            .interact(plus, ui.scope_id().with("add-coll"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if resp.hovered() {
            ui.painter().rect_filled(plus, 4, hover_tint());
        }
        plus_icon(
            ui,
            plus.center(),
            5.0,
            if resp.hovered() { fg() } else { fg_3() },
        );
        if a11y(resp, Role::Button, "New collection").clicked() {
            self.collections.creating = Some(String::new());
        }

        let mut actions: Vec<Action> = Vec::new();

        // inline "new collection" input
        if let Some(buf) = self.collections.creating.as_mut() {
            match inline_input(ui, "Collection name", "Collection name", buf) {
                InlineResult::Commit => {
                    let name = std::mem::take(buf);
                    self.collections.creating = None;
                    self.collections.create(&self.app, &name);
                }
                InlineResult::Cancel => self.collections.creating = None,
                InlineResult::Editing => {}
            }
        }
        if let Some(e) = &self.collections.error {
            let id = ui.scope_id().with("coll-error");
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
            ui.painter().text(
                pos2(rect.left() + 12.0, rect.center().y),
                Align2::LEFT_CENTER,
                e,
                font(11.0, "Regular"),
                danger(),
            );
            let _ = id;
            a11y(resp, Role::Label, &format!("Error: {e}"));
        }
        if self.collections.metas.is_empty() && self.collections.creating.is_none() {
            let rect = ui.available_rect_before_wrap();
            let (_, resp) = ui.allocate_exact_size(vec2(rect.width(), 54.0), Sense::hover());
            ui.painter().text(
                pos2(rect.center().x, resp.rect.top() + 18.0),
                Align2::CENTER_CENTER,
                "No collections yet.",
                font(12.5, "Regular"),
                fg_3(),
            );
            ui.painter().text(
                pos2(rect.center().x, resp.rect.top() + 36.0),
                Align2::CENTER_CENTER,
                "Click + to create a collection.",
                font(11.5, "Regular"),
                fg_3(),
            );
            a11y(
                resp,
                Role::Label,
                "No collections yet. Click New collection to create one.",
            );
        }

        let active_tab = self.tabs.active_id().map(str::to_string);
        let metas = self.collections.metas.clone();
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for meta in &metas {
                    let dir = dir_name(meta);
                    let open = self.collections.expanded.contains(&meta.id);
                    let renaming = self
                        .collections
                        .renaming
                        .as_ref()
                        .is_some_and(|(id, _)| *id == meta.id);
                    if renaming {
                        let (_, buf) = self.collections.renaming.as_mut().expect("renaming");
                        match inline_input(ui, "Collection name", "Rename collection", buf) {
                            InlineResult::Commit => {
                                let name = std::mem::take(buf);
                                self.collections.renaming = None;
                                self.collections.rename(&self.app, &dir, &name);
                            }
                            InlineResult::Cancel => self.collections.renaming = None,
                            InlineResult::Editing => {}
                        }
                    } else {
                        let w = ui.available_width();
                        let (row, resp) = ui.allocate_exact_size(vec2(w, 38.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(row, 0, hover_tint());
                        }
                        chevron(ui, pos2(row.left() + 19.0, row.center().y), open, fg_3());
                        ui.painter().text(
                            pos2(row.left() + 34.0, row.center().y),
                            Align2::LEFT_CENTER,
                            &meta.name,
                            font(13.0, "Regular"),
                            fg(),
                        );
                        // "more actions" button
                        let mr = Rect::from_center_size(
                            pos2(row.right() - 20.0, row.center().y),
                            vec2(24.0, 24.0),
                        );
                        let mresp =
                            ui.interact(mr, ui.scope_id().with(("more", &meta.id)), Sense::click());
                        let show_more = resp.hovered() || mresp.hovered();
                        if show_more {
                            if mresp.hovered() {
                                ui.painter().rect_filled(mr, 4, hover_tint());
                            }
                            ui.painter().text(
                                mr.center(),
                                Align2::CENTER_CENTER,
                                "...",
                                font(13.0, "Bold"),
                                fg_2(),
                            );
                        }
                        let mresp = a11y(
                            mresp,
                            Role::Button,
                            &format!("More actions for {}", meta.name),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand);
                        let resp = a11y_expanded(resp, Role::TreeItem, &meta.name, open)
                            .on_hover_cursor(CursorIcon::PointingHand);
                        if resp.clicked() && !mresp.clicked() {
                            actions.push(Action::ToggleCollection(meta.clone()));
                        }
                        let has_tab = active_tab.is_some();
                        let menu = |ui: &mut Ui, actions: &mut Vec<Action>| {
                            if ui.button("Add Request").clicked() {
                                actions.push(Action::AddRequest(meta.clone()));
                            }
                            if has_tab && ui.button("Save Current Tab").clicked() {
                                actions.push(Action::SaveTabTo(dir.clone()));
                            }
                            if ui.button("Rename").clicked() {
                                actions.push(Action::Rename(meta.clone()));
                            }
                            if ui.button("Delete Collection").clicked() {
                                actions.push(Action::Delete(meta.clone()));
                            }
                        };
                        Popup::context_menu(&resp).show(|ui| menu(ui, &mut actions));
                        Popup::menu(&mresp).show(|ui| menu(ui, &mut actions));
                    }

                    if open {
                        // inline "new request" input
                        if self
                            .collections
                            .adding_request
                            .as_ref()
                            .is_some_and(|(d, _)| *d == dir)
                        {
                            let (_, buf) =
                                self.collections.adding_request.as_mut().expect("adding");
                            match inline_input(ui, "Request name", "New request name", buf) {
                                InlineResult::Commit => {
                                    let name = std::mem::take(buf);
                                    self.collections.adding_request = None;
                                    let mut req = create_default_request();
                                    req.name = if name.trim().is_empty() {
                                        "New Request".into()
                                    } else {
                                        name.trim().into()
                                    };
                                    if self.collections.add_request(&self.app, &dir, &req) {
                                        self.open_collection_request(&dir, &req);
                                    }
                                }
                                InlineResult::Cancel => self.collections.adding_request = None,
                                InlineResult::Editing => {}
                            }
                        }
                        if let Some(c) = self.collections.loaded.get(&dir) {
                            if c.items.is_empty() && self.collections.adding_request.is_none() {
                                let w = ui.available_width();
                                let (row, resp) =
                                    ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
                                ui.painter().text(
                                    pos2(row.left() + 28.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    "No requests",
                                    font(12.0, "Regular"),
                                    fg_3(),
                                );
                                a11y(resp, Role::Label, "No requests");
                            } else {
                                render_items(
                                    ui,
                                    &c.items,
                                    0,
                                    &dir,
                                    &self.collections.expanded,
                                    &self.tabs,
                                    &mut actions,
                                );
                            }
                        }
                        ui.add_space(4.0);
                    }
                    let w = ui.available_width();
                    let (b, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                    ui.painter().rect_filled(b, 0, border_subtle());
                }
            });

        for a in actions {
            match a {
                Action::ToggleCollection(m) => self.collections.toggle_collection(&self.app, &m),
                Action::ToggleFolder(id) => self.collections.toggle_folder(&id),
                Action::Open(dir, req) => self.open_collection_request(&dir, &req),
                Action::AddRequest(m) => {
                    if !self.collections.expanded.contains(&m.id) {
                        self.collections.toggle_collection(&self.app, &m);
                    }
                    self.collections.adding_request = Some((dir_name(&m), String::new()));
                }
                Action::SaveTabTo(dir) => self.save_active_tab_to(&dir),
                Action::Rename(m) => self.collections.renaming = Some((m.id.clone(), m.name)),
                Action::Delete(m) => {
                    let dir = dir_name(&m);
                    self.collections.delete(&self.app, &dir);
                    self.tabs.close_origin(&dir, None);
                }
                Action::DeleteRequest(dir, id) => {
                    self.collections.delete_request(&self.app, &dir, &id);
                    self.tabs.close_origin(&dir, Some(&id));
                }
            }
        }
    }

    fn history_list(&mut self, ui: &mut Ui) {
        let r = bar(ui, 35.0, bg_surface(), Some(border_subtle()));
        spaced_text(
            ui,
            pos2(r.left() + 12.0, r.center().y),
            "HISTORY",
            font(11.0, "SemiBold"),
            fg_3(),
            0.66,
            false,
        );
        let clear = Rect::from_center_size(pos2(r.right() - 36.0, r.center().y), vec2(52.0, 24.0));
        let resp = ui
            .interact(clear, ui.scope_id().with("clear-history"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if resp.hovered() {
            ui.painter().rect_filled(clear, 4, hover_tint());
        }
        ui.painter().text(
            clear.center(),
            Align2::CENTER_CENTER,
            "Clear",
            font(11.0, "Medium"),
            if resp.hovered() { fg() } else { fg_2() },
        );
        if a11y(resp, Role::Button, "Clear history").clicked() {
            self.clear_history();
        }

        let mut open: Option<HistoryEntry> = None;
        if self.history.entries.is_empty() {
            let rect = ui.available_rect_before_wrap();
            let (_, resp) = ui.allocate_exact_size(vec2(rect.width(), 40.0), Sense::hover());
            let text = if self.history.loading {
                "Loading history..."
            } else {
                "No request history yet."
            };
            ui.painter().text(
                pos2(rect.center().x, resp.rect.center().y),
                Align2::CENTER_CENTER,
                text,
                font(12.5, "Regular"),
                fg_3(),
            );
            a11y(resp, Role::Label, text);
        }
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, 46.0, self.history.entries.len(), |ui, range| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for h in &self.history.entries[range] {
                    let w = ui.available_width();
                    let (row, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(row, 0, hover_tint());
                    }
                    let p = ui.painter().with_clip_rect(row);
                    p.text(
                        pos2(row.left() + 12.0, row.top() + 15.0),
                        Align2::LEFT_CENTER,
                        &h.request.method,
                        font(10.0, "Bold"),
                        method_color(&h.request.method),
                    );
                    p.text(
                        pos2(row.left() + 66.0, row.top() + 15.0),
                        Align2::LEFT_CENTER,
                        &h.request.url,
                        font(12.0, "Regular"),
                        fg_2(),
                    );
                    let (bg, fg) = status_colors(h.response.status);
                    let pill = Rect::from_min_size(
                        pos2(row.left() + 66.0, row.top() + 25.0),
                        vec2(30.0, 15.0),
                    );
                    p.rect_filled(pill, 3, bg);
                    p.text(
                        pill.center(),
                        Align2::CENTER_CENTER,
                        h.response.status.to_string(),
                        font(10.0, "Bold"),
                        fg,
                    );
                    p.text(
                        pos2(pill.right() + 8.0, pill.center().y),
                        Align2::LEFT_CENTER,
                        format!(
                            "{}  {}",
                            fmt_dur(h.response.response_time),
                            fmt_time(h.timestamp)
                        ),
                        font(11.0, "Regular"),
                        fg_3(),
                    );
                    hline(
                        ui,
                        row.bottom() - 1.0,
                        row.left(),
                        row.right(),
                        border_subtle(),
                    );
                    let label = format!("{} {}", h.request.method, h.request.url);
                    let resp = a11y(resp, Role::Button, &label);
                    resp.ctx.accesskit_node_builder(resp.id, |n| {
                        n.set_description(format!("Status {}", h.response.status));
                    });
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        open = Some(h.clone());
                    }
                }
            });
        if let Some(e) = open {
            self.open_history_entry(&e);
        }
    }
}

fn fmt_time(ts_ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_millis_opt(ts_ms)
        .single()
        .map(|d| d.format("%H:%M").to_string())
        .unwrap_or_default()
}

enum InlineResult {
    Editing,
    Commit,
    Cancel,
}

/// A single-line input row (create / rename). Enter or losing focus commits, Escape cancels.
fn inline_input(ui: &mut Ui, hint: &str, label: &str, buf: &mut String) -> InlineResult {
    let w = ui.available_width();
    let (row, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
    let field = row.shrink2(vec2(10.0, 4.0));
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(field), |ui| {
            ui.add_sized(
                field.size(),
                TextEdit::singleline(buf)
                    .hint_text(hint)
                    .font(font(12.5, "Regular")),
            )
        })
        .inner;
    a11y_name(&resp, label);
    let id = resp.id;
    let inited = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    if !inited {
        resp.request_focus();
        ui.data_mut(|d| d.insert_temp(id, true));
    }
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let esc = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if esc {
        ui.data_mut(|d| d.remove_temp::<bool>(id));
        return InlineResult::Cancel;
    }
    if enter || (inited && resp.lost_focus()) {
        ui.data_mut(|d| d.remove_temp::<bool>(id));
        return if buf.trim().is_empty() {
            InlineResult::Cancel
        } else {
            InlineResult::Commit
        };
    }
    InlineResult::Editing
}

#[allow(clippy::too_many_arguments)]
fn render_items(
    ui: &mut Ui,
    items: &[CollectionItem],
    depth: usize,
    dir: &str,
    expanded: &std::collections::HashSet<String>,
    tabs: &crate::state::Tabs,
    actions: &mut Vec<Action>,
) {
    for item in items {
        let w = ui.available_width();
        let (row, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(row, 0, hover_tint());
        }
        let x0 = row.left() + 28.0 + 16.0 * depth as f32;
        match item {
            CollectionItem::Folder(f) => {
                let open = expanded.contains(&f.id);
                chevron(ui, pos2(x0 + 2.0, row.center().y), open, fg_3());
                ui.painter().text(
                    pos2(x0 + 22.0, row.center().y),
                    Align2::LEFT_CENTER,
                    &f.name,
                    font(12.0, "Medium"),
                    fg(),
                );
                let resp = a11y_expanded(resp, Role::TreeItem, &f.name, open)
                    .on_hover_cursor(CursorIcon::PointingHand);
                if resp.clicked() {
                    actions.push(Action::ToggleFolder(f.id.clone()));
                }
                if open {
                    render_items(ui, &f.items, depth + 1, dir, expanded, tabs, actions);
                }
            }
            CollectionItem::Request(r) => {
                let m = r.method.as_str();
                let is_open = tabs.active().is_some_and(|t| {
                    t.origin
                        .as_ref()
                        .is_some_and(|o| o.request_id == r.id && o.dir_name == dir)
                });
                if is_open {
                    ui.painter().rect_filled(row, 0, active_tint());
                }
                ui.painter().text(
                    pos2(x0, row.center().y),
                    Align2::LEFT_CENTER,
                    m,
                    font(10.0, "Bold"),
                    method_color(m),
                );
                ui.painter().text(
                    pos2(x0 + 56.0, row.center().y),
                    Align2::LEFT_CENTER,
                    &r.name,
                    font(12.0, "Regular"),
                    if resp.hovered() || is_open {
                        fg()
                    } else {
                        fg_2()
                    },
                );
                let resp = a11y(resp, Role::TreeItem, &r.name);
                resp.ctx.accesskit_node_builder(resp.id, |n| {
                    n.set_description(format!("{m} request"));
                });
                if resp.clicked() {
                    actions.push(Action::Open(dir.to_string(), Box::new(r.clone())));
                }
                Popup::context_menu(&resp).show(|ui| {
                    if ui.button("Delete Request").clicked() {
                        actions.push(Action::DeleteRequest(dir.to_string(), r.id.clone()));
                    }
                });
                let _ = resp.on_hover_cursor(CursorIcon::PointingHand);
            }
        }
    }
}
