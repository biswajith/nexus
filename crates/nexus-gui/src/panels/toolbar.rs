use crate::app::NexusApp;
use crate::state::dialogs::Dialog;
use crate::state::Panel;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Rect, RichText, Sense, Shape, Ui, WidgetInfo,
};

impl NexusApp {
    pub(crate) fn title_bar(&mut self, ui: &mut Ui) {
        let r = bar(ui, 38.0, bg_sunken(), Some(border()));
        spaced_text(
            ui,
            r.center(),
            "NEXUS",
            font(12.0, "SemiBold"),
            fg_3(),
            1.0,
            true,
        );
        let id = ui.scope_id().with("title");
        a11y(ui.interact(r, id, Sense::hover()), Role::Heading, "Nexus");
    }

    pub(crate) fn toolbar(&mut self, ui: &mut Ui) {
        let r = bar(ui, 42.0, bg_surface(), Some(border()));
        let env_cx = r.left() + r.width() * 0.5;
        let left = Rect::from_min_max(
            pos2(r.left() + 12.0, r.top()),
            pos2(env_cx - 150.0, r.bottom()),
        );
        sub(ui, left, ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let collapsed = self.ui.sidebar_collapsed;
            if icon_button(ui, "Toggle sidebar", 28.0, |ui, c, col| {
                if collapsed {
                    plus_icon(ui, c, 4.0, col)
                } else {
                    x_icon(ui, c, 4.0, col)
                }
            })
            .clicked()
            {
                self.ui.sidebar_collapsed = !collapsed;
            }
            // mode tabs
            let idx = ui.painter().add(Shape::Noop);
            let grp = ui.scope(|ui| {
                ui.add_space(2.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                for p in Panel::ALL {
                    let active = self.ui.active_panel == p;
                    let st = if active {
                        BtnStyle {
                            fill: accent(),
                            fill_hover: accent_hover(),
                            stroke: None,
                            stroke_hover: None,
                            text: text_on(accent()),
                            text_hover: text_on(accent_hover()),
                            radius: 4,
                        }
                    } else {
                        BtnStyle::flat()
                    };
                    if button_role(
                        ui,
                        p.label(),
                        font(12.0, "Medium"),
                        (10.0, 4.0),
                        24.0,
                        &st,
                        Role::Tab,
                        Some(active),
                    )
                    .clicked()
                    {
                        self.ui.active_panel = p;
                    }
                }
                ui.add_space(2.0);
            });
            let g = Rect::from_center_size(
                grp.response.rect.center(),
                vec2(grp.response.rect.width(), 28.0),
            );
            ui.painter().set(idx, Shape::rect_filled(g, 6, bg_sunken()));
        });

        // right side: Import, Docs | Theme, Console
        let right = Rect::from_min_max(
            pos2(env_cx + 145.0, r.top()),
            pos2(r.right() - 12.0, r.bottom()),
        );
        sub(
            ui,
            right,
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let open = self.ui.console_open;
                let st = if open {
                    BtnStyle {
                        fill: active_tint(),
                        fill_hover: active_tint(),
                        stroke: None,
                        stroke_hover: None,
                        text: fg(),
                        text_hover: fg(),
                        radius: 4,
                    }
                } else {
                    BtnStyle::flat()
                };
                if button_role(
                    ui,
                    "Console",
                    font(12.0, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &st,
                    Role::Button,
                    Some(open),
                )
                .on_hover_text("Toggle console (Ctrl+J)")
                .clicked()
                {
                    self.toggle_console();
                }
                let mode = self.ui.theme;
                if button_named(
                    ui,
                    &format!("Theme: {}", mode.label()),
                    &format!("Theme: {}", mode.label()),
                    &BtnStyle::flat(),
                    26.0,
                )
                .on_hover_text(format!("Theme: {} (click to change)", mode.label()))
                .clicked()
                {
                    self.cycle_theme();
                }
                ui.add_space(6.0);
                if button(
                    ui,
                    "Docs",
                    font(12.0, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &BtnStyle::flat(),
                )
                .on_hover_text("Generate API documentation")
                .clicked()
                {
                    self.open_dialog(Dialog::Docs);
                }
                if button(
                    ui,
                    "Import",
                    font(12.0, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &BtnStyle::flat(),
                )
                .on_hover_text("Import / export collections")
                .clicked()
                {
                    self.open_dialog(Dialog::Import);
                }
            },
        );

        // environment switcher
        let cr = Rect::from_center_size(pos2(env_cx, r.center().y), vec2(290.0, 32.0));
        sub(ui, cr, ltr(), |ui| {
            ui.spacing_mut().interact_size.y = 28.0;
            ui.spacing_mut().item_spacing.x = 10.0;
            let current = self.envs.name(self.ui.active_env_id.as_deref()).to_string();
            let mut chosen: Option<Option<String>> = None;
            let combo = egui::ComboBox::from_id_salt("env")
                .width(190.0)
                .selected_text(
                    RichText::new(&current)
                        .font(font(12.0, "Regular"))
                        .color(fg()),
                )
                .show_ui(ui, |ui| {
                    let none = self.ui.active_env_id.is_none();
                    if ui
                        .selectable_label(
                            none,
                            RichText::new("No Environment").font(font(12.0, "Regular")),
                        )
                        .clicked()
                    {
                        chosen = Some(None);
                    }
                    for e in &self.envs.environments {
                        let sel = self.ui.active_env_id.as_deref() == Some(e.id.as_str());
                        if ui
                            .selectable_label(
                                sel,
                                RichText::new(&e.name).font(font(12.0, "Regular")),
                            )
                            .clicked()
                        {
                            chosen = Some(Some(e.id.clone()));
                        }
                    }
                });
            combo.response.widget_info(|| WidgetInfo {
                label: Some("Select environment".into()),
                current_text_value: Some(current.clone()),
                ..WidgetInfo::new(Role::ComboBox)
            });
            if std::mem::take(&mut self.view.focus_env) {
                combo.response.request_focus();
            }
            if let Some(c) = chosen {
                self.set_env(c);
            }
            if button(
                ui,
                "Manage",
                font(12.0, "Medium"),
                (10.0, 4.0),
                26.0,
                &BtnStyle::outline(),
            )
            .on_hover_text("Manage environments")
            .clicked()
            {
                self.open_dialog(Dialog::Envs);
            }
        });
    }
}
