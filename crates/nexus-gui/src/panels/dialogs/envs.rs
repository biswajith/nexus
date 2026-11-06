//! Environment manager (`EnvironmentManager.tsx`): list, create, rename, duplicate, delete,
//! activate and edit variables.

use super::{card, para};
use crate::app::NexusApp;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{self, accesskit::Role, pos2, vec2, Align, Layout, Rect, ScrollArea, Sense, Ui};
use nexus_core::types::VariableType;

impl NexusApp {
    pub(super) fn envs_dialog(&mut self, ui: &mut Ui) {
        if let Some(e) = self.dlg.envs.error.clone() {
            card(ui, "Error", danger(), &[e]);
        }
        if self.dlg.envs.draft.is_some() {
            self.env_editor(ui);
        } else {
            self.env_list(ui);
        }
    }

    fn env_list(&mut self, ui: &mut Ui) {
        let count = self.envs.environments.len();
        let mut start_create = false;
        ui.horizontal(|ui| {
            let t = format!("{count} environment{}", if count == 1 { "" } else { "s" });
            labeled_text(ui, &t, &t, font(12.0, "Regular"), fg_2());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(
                    ui,
                    "+ New Environment",
                    font(12.0, "Medium"),
                    (12.0, 5.0),
                    28.0,
                    &BtnStyle::primary(accent(), accent_hover()),
                )
                .clicked()
                {
                    start_create = true;
                }
            });
        });
        if start_create {
            self.dlg.envs.creating = Some(String::new());
        }
        if let Some(mut name) = self.dlg.envs.creating.take() {
            let mut keep = true;
            let mut commit = false;
            ui.horizontal(|ui| {
                let w = ui.available_width() - 160.0;
                let r = field(
                    ui,
                    "New environment name",
                    &mut name,
                    w,
                    30.0,
                    font(12.5, "Regular"),
                    "Environment name",
                    bg_input(),
                    true,
                );
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    commit = true;
                }
                if button(
                    ui,
                    "Create",
                    font(12.0, "Medium"),
                    (12.0, 5.0),
                    28.0,
                    &BtnStyle::outline(),
                )
                .clicked()
                {
                    commit = true;
                }
                if button(
                    ui,
                    "Cancel",
                    font(12.0, "Medium"),
                    (12.0, 5.0),
                    28.0,
                    &BtnStyle::flat(),
                )
                .clicked()
                {
                    keep = false;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
            if commit {
                if let Some(id) = self.env_create(&name) {
                    self.env_edit(&id);
                }
            } else if keep {
                self.dlg.envs.creating = Some(name);
            }
        }
        if count == 0 && self.dlg.envs.creating.is_none() {
            ui.add_space(12.0);
            para(ui, "No environments yet.", 13.0, fg());
            para(
                ui,
                "Create one to start using variables like {{base_url}}.",
                12.0,
                fg_2(),
            );
            return;
        }
        let envs: Vec<(String, String, usize)> = self
            .envs
            .environments
            .iter()
            .map(|e| (e.id.clone(), e.name.clone(), e.variables.len()))
            .collect();
        let active = self.ui.active_env_id.clone();
        let mut edit: Option<String> = None;
        let mut dup: Option<String> = None;
        let mut activate: Option<Option<String>> = None;
        let mut delete: Option<String> = None;
        let mut rename_commit: Option<(String, String)> = None;
        ScrollArea::vertical()
            .id_salt("env-list")
            .min_scrolled_height((envs.len() as f32 * 50.0).min(420.0))
            .max_height((envs.len() as f32 * 50.0).min(420.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (id, name, nvars) in &envs {
                    let is_active = active.as_deref() == Some(id.as_str());
                    let w = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(vec2(w, 44.0), Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        6,
                        if is_active {
                            accent_subtle()
                        } else {
                            bg_input()
                        },
                    );
                    ui.painter().rect_stroke(
                        rect.shrink(0.5),
                        6,
                        egui::Stroke::new(1.0, if is_active { accent() } else { border_subtle() }),
                        egui::StrokeKind::Inside,
                    );
                    let inner = Rect::from_min_max(
                        pos2(rect.left() + 12.0, rect.top()),
                        pos2(rect.right() - 8.0, rect.bottom()),
                    );
                    sub(ui, inner, Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let o = BtnStyle::outline();
                        if self.dlg.envs.confirm_delete.as_deref() == Some(id.as_str()) {
                            if button_named(
                                ui,
                                "Cancel",
                                &format!("Cancel deleting {name}"),
                                &BtnStyle::flat(),
                                26.0,
                            )
                            .clicked()
                            {
                                self.dlg.envs.confirm_delete = None;
                            }
                            if button_named(
                                ui,
                                "Confirm delete",
                                &format!("Confirm deleting {name}"),
                                &BtnStyle::primary(danger(), danger().gamma_multiply(0.85)),
                                26.0,
                            )
                            .clicked()
                            {
                                delete = Some(id.clone());
                            }
                        } else {
                            if button_named(ui, "Delete", &format!("Delete {name}"), &o, 26.0)
                                .clicked()
                            {
                                self.dlg.envs.confirm_delete = Some(id.clone());
                            }
                            if button_named(ui, "Duplicate", &format!("Duplicate {name}"), &o, 26.0)
                                .clicked()
                            {
                                dup = Some(id.clone());
                            }
                            if button_named(ui, "Rename", &format!("Rename {name}"), &o, 26.0)
                                .clicked()
                            {
                                self.dlg.envs.renaming = Some((id.clone(), name.clone()));
                            }
                            if is_active {
                                if button_named(
                                    ui,
                                    "Deactivate",
                                    &format!("Deactivate {name}"),
                                    &o,
                                    26.0,
                                )
                                .clicked()
                                {
                                    activate = Some(None);
                                }
                            } else if button_named(
                                ui,
                                "Activate",
                                &format!("Activate {name}"),
                                &o,
                                26.0,
                            )
                            .clicked()
                            {
                                activate = Some(Some(id.clone()));
                            }
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let renaming = self
                                .dlg
                                .envs
                                .renaming
                                .as_ref()
                                .filter(|(rid, _)| rid == id)
                                .map(|(_, b)| b.clone());
                            if let Some(mut buf) = renaming {
                                let r = field(
                                    ui,
                                    &format!("Rename {name} to"),
                                    &mut buf,
                                    220.0,
                                    28.0,
                                    font(12.5, "Regular"),
                                    "Name",
                                    bg_input(),
                                    true,
                                );
                                if !r.has_focus() && !r.lost_focus() {
                                    r.request_focus();
                                }
                                let enter =
                                    r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                if enter {
                                    rename_commit = Some((id.clone(), buf.clone()));
                                    self.dlg.envs.renaming = None;
                                } else if esc {
                                    self.dlg.envs.renaming = None;
                                } else {
                                    self.dlg.envs.renaming = Some((id.clone(), buf));
                                }
                            } else {
                                let r = button_named(
                                    ui,
                                    name,
                                    &format!("Edit {name}"),
                                    &BtnStyle::flat_text(),
                                    28.0,
                                );
                                if r.clicked() {
                                    edit = Some(id.clone());
                                }
                                let t = format!(
                                    "{nvars} variable{}",
                                    if *nvars == 1 { "" } else { "s" }
                                );
                                labeled_text(
                                    ui,
                                    &t,
                                    &format!("{name}: {t}"),
                                    font(11.5, "Regular"),
                                    fg_3(),
                                );
                                if is_active {
                                    pill(
                                        ui,
                                        "Active",
                                        success(),
                                        success_subtle(),
                                        &format!("{name} is active"),
                                    );
                                }
                            }
                        });
                    });
                }
            });
        if let Some((id, n)) = rename_commit {
            self.env_rename(&id, &n);
        }
        if let Some(id) = edit {
            self.env_edit(&id);
        }
        if let Some(id) = dup {
            self.env_duplicate(&id);
        }
        if let Some(a) = activate {
            self.set_env(a);
        }
        if let Some(id) = delete {
            self.dlg.envs.confirm_delete = None;
            self.env_delete(&id);
        }
    }

    fn env_editor(&mut self, ui: &mut Ui) {
        let mut back = false;
        let mut save = false;
        let mut add = false;
        let mut remove: Option<usize> = None;
        let mut toggle_reveal: Option<usize> = None;
        let revealed = self.dlg.envs.revealed.clone();
        let Some(d) = self.dlg.envs.draft.as_mut() else {
            return;
        };
        ui.horizontal(|ui| {
            if button(
                ui,
                "<- All Environments",
                font(12.0, "Medium"),
                (10.0, 5.0),
                28.0,
                &BtnStyle::flat(),
            )
            .clicked()
            {
                back = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let st = if d.dirty {
                    BtnStyle::primary(accent(), accent_hover())
                } else {
                    BtnStyle::outline()
                };
                if button(
                    ui,
                    "Save Changes",
                    font(12.0, "Medium"),
                    (12.0, 5.0),
                    28.0,
                    &st,
                )
                .clicked()
                    && d.dirty
                {
                    save = true;
                }
                if d.dirty {
                    labeled_text(
                        ui,
                        "Unsaved changes",
                        "Unsaved changes",
                        font(11.5, "Regular"),
                        warning(),
                    );
                }
            });
        });
        if let Some(e) = &d.error {
            card(ui, "Cannot save", danger(), std::slice::from_ref(e));
        }
        ui.horizontal(|ui| {
            field_label(ui, "Name");
            let w = ui.available_width();
            let r = field(
                ui,
                "Environment name",
                &mut d.name,
                w,
                30.0,
                font(12.5, "Regular"),
                "Environment name",
                bg_input(),
                true,
            );
            if r.changed() {
                d.dirty = true;
            }
        });
        ui.horizontal(|ui| {
            labeled_text(ui, "VARIABLES", "Variables", font(11.0, "SemiBold"), fg_3());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(
                    ui,
                    "+ Add Variable",
                    font(12.0, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &BtnStyle::outline(),
                )
                .clicked()
                {
                    add = true;
                }
            });
        });
        if d.vars.is_empty() {
            para(
                ui,
                "No variables defined. Click \"+ Add Variable\" to create one.",
                12.0,
                fg_2(),
            );
        }
        ScrollArea::vertical()
            .id_salt("env-vars")
            .min_scrolled_height((d.vars.len() as f32 * 40.0 + 12.0).min(380.0))
            .max_height((d.vars.len() as f32 * 40.0 + 12.0).min(380.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for (i, v) in d.vars.iter_mut().enumerate() {
                    let n = i + 1;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if checkbox(ui, &format!("Enable variable {n}"), &mut v.enabled).changed() {
                            d.dirty = true;
                        }
                        let total = ui.available_width();
                        let key_w = (total * 0.30).clamp(120.0, 220.0);
                        let rest = total - key_w - 90.0 - 28.0 - 60.0 - 24.0;
                        let secret = v.var_type == VariableType::Secret;
                        if field(
                            ui,
                            &format!("Variable key {n}"),
                            &mut v.key,
                            key_w,
                            28.0,
                            egui::FontId::new(12.5, egui::FontFamily::Monospace),
                            "Key",
                            bg_input(),
                            true,
                        )
                        .changed()
                        {
                            d.dirty = true;
                        }
                        let hide = secret && !revealed.contains(&i);
                        if field_with(
                            ui,
                            &format!("Variable value {n}"),
                            &mut v.value,
                            rest.max(120.0),
                            28.0,
                            egui::FontId::new(12.5, egui::FontFamily::Monospace),
                            "Value",
                            bg_input(),
                            true,
                            None,
                            hide,
                        )
                        .changed()
                        {
                            d.dirty = true;
                        }
                        let cur = if secret { "Secret" } else { "String" };
                        let types = ["String".to_string(), "Secret".to_string()];
                        if let Some(k) = combo_pick(
                            ui,
                            &format!("vtype{i}"),
                            &format!("Variable type {n}"),
                            90.0,
                            cur,
                            &types,
                        ) {
                            v.var_type = if k == 1 {
                                VariableType::Secret
                            } else {
                                VariableType::String
                            };
                            d.dirty = true;
                        }
                        if secret {
                            let (txt, acc) = if hide {
                                ("Show", format!("Show value {n}"))
                            } else {
                                ("Hide", format!("Hide value {n}"))
                            };
                            if button_named(ui, txt, &acc, &BtnStyle::flat(), 26.0).clicked() {
                                toggle_reveal = Some(i);
                            }
                        } else {
                            ui.add_space(52.0);
                        }
                        if icon_button(ui, &format!("Remove variable {n}"), 26.0, |ui, c, col| {
                            x_icon(ui, c, 4.0, col)
                        })
                        .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                }
            });
        if add {
            d.vars.push(NexusApp::new_variable());
            d.dirty = true;
        }
        if let Some(i) = remove {
            d.vars.remove(i);
            d.dirty = true;
            self.dlg.envs.revealed.clear();
        }
        if let Some(i) = toggle_reveal {
            if !self.dlg.envs.revealed.remove(&i) {
                self.dlg.envs.revealed.insert(i);
            }
        }
        if save {
            self.env_save_draft();
        }
        if back {
            self.dlg.envs.draft = None;
            self.dlg.envs.revealed.clear();
        }
        let _ = Role::Button;
    }
}
