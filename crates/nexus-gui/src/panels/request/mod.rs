//! The request builder: URL bar and the Params / Headers / Body / Auth / Scripts tabs.

mod auth;
mod body;
pub(crate) mod kv;
mod scripts;

use crate::app::NexusApp;
use crate::payload::{is_defined, resolve_request};
use crate::state::ReqTab;
use crate::theme::*;
use crate::url_sync::{effective_url, params_from_url, url_with_params};
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, text::LayoutJob, vec2, Align, Color32, ComboBox, FontFamily, FontId,
    Layout, Popup, PopupCloseBehavior, RichText, TextFormat, Ui, WidgetInfo,
};
use nexus_core::environments::VariableStore;
use nexus_core::types::{HttpMethod, NexusRequest};

const METHODS: [HttpMethod; 7] = [
    HttpMethod::Get,
    HttpMethod::Post,
    HttpMethod::Put,
    HttpMethod::Patch,
    HttpMethod::Delete,
    HttpMethod::Head,
    HttpMethod::Options,
];

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// URL text with `{{variables}}` coloured: green when they resolve, amber when they do not.
fn url_job(text: &str, store: &VariableStore, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let fmt = |color: Color32| TextFormat {
        font_id: mono(13.0),
        color,
        ..Default::default()
    };
    let mut rest = text;
    while !rest.is_empty() {
        let Some(start) = rest.find("{{") else {
            break;
        };
        let Some(len) = rest[start..].find("}}") else {
            break;
        };
        let end = start + len + 2;
        job.append(&rest[..start], 0.0, fmt(fg()));
        let name = &rest[start + 2..start + len];
        let color = if is_defined(store, name) {
            success()
        } else {
            warning()
        };
        job.append(&rest[start..end], 0.0, fmt(color));
        rest = &rest[end..];
    }
    job.append(rest, 0.0, fmt(fg()));
    job
}

/// The cURL command for `req` with variables resolved (params are already part of the URL).
fn curl_text(req: &NexusRequest, store: &VariableStore) -> String {
    let mut r = resolve_request(req, store);
    r.url = effective_url(&r.url, &r.params);
    r.params.clear();
    nexus_core::codegen::generate_curl(&r).unwrap_or_else(|e| format!("Cannot generate cURL: {e}"))
}

impl NexusApp {
    pub(crate) fn request_builder(&mut self, ui: &mut Ui) {
        ui.painter().rect_filled(ui.max_rect(), 0, bg_app());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut do_send = false;
        let mut do_cancel = false;
        let mut do_save = false;
        let mut save_to: Option<(String, String)> = None;
        let mut show_curl = false;
        let mut pick = None;
        let Some(idx) = self.tabs.active_index() else {
            return;
        };
        let before = self.tabs.tabs[idx].request.clone();
        let scopes = self.scopes_for(&self.tabs.tabs[idx]);
        let store = scopes.store();
        crate::widgets::set_variables(ui.ctx(), crate::editor::complete::var_items(&scopes));

        // URL bar
        let r = bar(ui, 58.0, bg_surface(), Some(border()));
        let inner = r.shrink2(vec2(12.0, 12.0)).translate(vec2(0.0, -0.5));
        let send_w = text_w(ui, "Send", font(13.0, "SemiBold")) + 40.0;
        let curl_w = text_w(ui, "cURL", font(12.0, "Medium")) + 24.0;
        let save_w = text_w(ui, "Save", font(12.0, "Medium")) + 24.0;
        let collection_choices: Vec<(String, String)> = self
            .collections
            .metas
            .iter()
            .map(|m| (m.name.clone(), crate::state::collections::dir_name(m)))
            .collect();
        let view = &mut self.view;
        sub(ui, inner, ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().interact_size.y = 34.0;
            let tab = &mut self.tabs.tabs[idx];
            let m = tab.request.method;
            let combo = ComboBox::from_id_salt("method")
                .width(95.0 - 24.0)
                .selected_text(
                    RichText::new(m.as_str())
                        .font(FontId::new(13.0, mono_bold()))
                        .color(method_color(m.as_str())),
                )
                .show_ui(ui, |ui| {
                    for mm in METHODS {
                        if ui
                            .selectable_label(
                                m == mm,
                                RichText::new(mm.as_str())
                                    .font(FontId::new(12.5, mono_bold()))
                                    .color(method_color(mm.as_str())),
                            )
                            .clicked()
                        {
                            tab.request.method = mm;
                        }
                    }
                });
            combo.response.widget_info(|| WidgetInfo {
                label: Some("HTTP method".into()),
                current_text_value: Some(m.as_str().into()),
                ..WidgetInfo::new(Role::ComboBox)
            });
            let w = ui.available_width() - (send_w + curl_w + save_w + 24.0);
            let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                let job = url_job(buf.as_str(), &store, wrap);
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let resp = field_with(
                ui,
                "Request URL",
                &mut tab.request.url,
                w,
                34.0,
                mono(13.0),
                "Enter request URL or paste cURL",
                bg_input(),
                true,
                Some(&mut layouter),
                false,
            );
            if std::mem::take(&mut view.focus_url) {
                resp.request_focus();
                if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
                    let n = tab.request.url.chars().count();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(n),
                    )));
                    st.store(ui.ctx(), resp.id);
                }
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                do_send = true;
            }
            // pasting a cURL command fills the whole request
            if resp.has_focus() {
                let pasted = ui.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        egui::Event::Paste(t) => Some(t.trim().to_string()),
                        _ => None,
                    })
                });
                if let Some(t) =
                    pasted.filter(|t| t.starts_with("curl ") || t.starts_with("curl\t"))
                {
                    if let Ok(p) = nexus_core::service::App::import_curl(&t) {
                        let r = &mut tab.request;
                        (r.method, r.url, r.headers, r.params, r.body) =
                            (p.method, p.url, p.headers, p.params, p.body);
                    }
                }
            }
            if tab.request.url.contains("{{") {
                let resolved = store.resolve(&tab.request.url);
                resp.on_hover_text(format!("Resolved: {resolved}"));
            }
            let loading = tab.loading;
            let (label, st) = if loading {
                (
                    "Cancel",
                    BtnStyle::primary(danger(), danger().gamma_multiply(0.85)),
                )
            } else {
                ("Send", BtnStyle::primary(accent(), accent_hover()))
            };
            if button(ui, label, font(13.0, "SemiBold"), (20.0, 7.0), 34.0, &st).clicked() {
                if loading {
                    do_cancel = true;
                } else {
                    do_send = true;
                }
            }
            if button(
                ui,
                "cURL",
                font(12.0, "Medium"),
                (12.0, 6.0),
                30.0,
                &BtnStyle::outline(),
            )
            .clicked()
            {
                show_curl = true;
            }
            let has_origin = tab.origin.is_some();
            let save = button(
                ui,
                "Save",
                font(12.0, "Medium"),
                (12.0, 6.0),
                30.0,
                &BtnStyle::outline_accent(),
            );
            if has_origin {
                if save.clicked() {
                    do_save = true;
                }
            } else {
                Popup::menu(&save)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_min_width(220.0);
                        if let Some(dir) = view.save_target.clone() {
                            ui.label("Request name");
                            let mut name = std::mem::take(&mut view.save_name);
                            let r = field(
                                ui,
                                "Request name",
                                &mut name,
                                200.0,
                                28.0,
                                mono(12.5),
                                "e.g. Get Users",
                                bg_input(),
                                true,
                            );
                            let enter =
                                r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            let ok = !name.trim().is_empty();
                            ui.horizontal(|ui| {
                                if ui.button("Back").clicked() {
                                    view.save_target = None;
                                }
                                if ui
                                    .add_enabled(ok, egui::Button::new("Save request"))
                                    .clicked()
                                    || (enter && ok)
                                {
                                    save_to = Some((dir.clone(), name.trim().to_string()));
                                    view.save_target = None;
                                    ui.close();
                                }
                            });
                            view.save_name = name;
                        } else {
                            if collection_choices.is_empty() {
                                ui.label("Create a collection first");
                            }
                            for (name, dir) in &collection_choices {
                                if ui.button(format!("Save to {name}")).clicked() {
                                    view.save_target = Some(dir.clone());
                                    view.save_name.clear();
                                }
                            }
                        }
                    });
            }
        });

        // sub tabs
        let tab = &self.tabs.tabs[idx];
        let pc = tab
            .request
            .params
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .count();
        let hc = tab
            .request
            .headers
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .count();
        let r = bar(ui, 33.0, bg_surface(), Some(border()));
        sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
            let count = |l: &str, n: usize| {
                if n > 0 {
                    format!("{l} ({n})")
                } else {
                    l.to_string()
                }
            };
            let items = [
                (ReqTab::Params, count("Params", pc)),
                (ReqTab::Headers, count("Headers", hc)),
                (ReqTab::Body, "Body".to_string()),
                (ReqTab::Auth, "Auth".to_string()),
                (ReqTab::Scripts, "Scripts".to_string()),
            ];
            for (t, l) in items {
                if underline_tab(ui, &l, self.view.req_tab == t, 33.0, None).clicked() {
                    self.view.req_tab = t;
                }
            }
        });

        // content: every sub-tab scrolls vertically when the pane is too short for it
        let area = ui.available_rect_before_wrap();
        let min_h = match self.view.req_tab {
            ReqTab::Params | ReqTab::Headers | ReqTab::Auth => 150.0,
            ReqTab::Body => 200.0,
            ReqTab::Scripts => 200.0,
        };
        let tab_id = self.tabs.tabs[idx].id.clone();
        sub(ui, area, Layout::top_down(Align::Min), |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("request_content", &tab_id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let h = area.height().max(min_h);
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(area.width(), h), egui::Sense::hover());
                    sub(ui, rect, Layout::top_down(Align::Min), |ui| {
                        let req = &mut self.tabs.tabs[idx].request;
                        match self.view.req_tab {
                            ReqTab::Params => {
                                kv::kv_table(ui, "params", "Parameter", &mut req.params)
                            }
                            ReqTab::Headers => {
                                kv::kv_table(ui, "headers", "Header", &mut req.headers)
                            }
                            ReqTab::Body => pick = body::body_editor(ui, &mut req.body),
                            ReqTab::Auth => {
                                auth::auth_editor(ui, &mut req.auth, &mut self.view.revealed)
                            }
                            ReqTab::Scripts => {
                                scripts::scripts_editor(ui, req, &mut self.view.script_tab)
                            }
                        }
                    });
                });
        });

        // URL <-> Params two-way sync
        let tab = &mut self.tabs.tabs[idx];
        let req = &mut tab.request;
        if req.url != before.url && req.params == before.params {
            req.params = params_from_url(&req.url, &req.params);
        } else if req.params != before.params && req.url == before.url {
            req.url = url_with_params(&req.url, &req.params);
        }
        if tab.request != before && !tab.dirty {
            tab.dirty = true;
            // the tab bar above already drew this frame
            ui.ctx().request_repaint();
        }

        if show_curl {
            let text = curl_text(&self.tabs.tabs[idx].request, &store);
            self.view.curl = Some(text);
        }
        self.curl_window(ui.ctx());

        if let Some(t) = pick {
            self.pick_file(t);
        }
        if do_save {
            self.save_active_tab();
            ui.ctx().request_repaint();
        }
        if let Some((d, name)) = save_to {
            if let Some(t) = self.tabs.active_mut() {
                t.request.name = name;
            }
            self.save_active_tab_to(&d);
            ui.ctx().request_repaint();
        }
        if do_cancel {
            self.cancel_active();
        }
        if do_send {
            self.send_active();
        }
    }

    /// The popup with the generated cURL command.
    fn curl_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.view.curl.clone() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        egui::Window::new("cURL command")
            .collapsible(false)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                let r = ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .font(mono(12.5))
                        .desired_width(f32::INFINITY)
                        .desired_rows(8),
                );
                a11y_name(&r, "cURL command");
                ui.horizontal(|ui| {
                    if ui.button("Copy cURL").clicked() {
                        ui.ctx().copy_text(text.clone());
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        // the text is read-only: edits made in the box are discarded
        if !open || close {
            self.view.curl = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::Scopes;
    use nexus_core::types::{Variable, VariableType};

    #[test]
    fn resolved_variables_are_green_and_unresolved_amber() {
        let scopes = Scopes {
            environment: vec![Variable {
                key: "host".into(),
                value: "h".into(),
                var_type: VariableType::String,
                enabled: true,
                description: None,
            }],
            ..Default::default()
        };
        let job = url_job("{{host}}/a/{{nope}}?x={{ $guid }}", &scopes.store(), 100.0);
        let colored: Vec<(&str, Color32)> = job
            .sections
            .iter()
            .map(|s| {
                (
                    &job.text[s.byte_range.start.0..s.byte_range.end.0],
                    s.format.color,
                )
            })
            .collect();
        assert!(colored.contains(&("{{host}}", success())));
        assert!(colored.contains(&("{{nope}}", warning())));
        assert!(colored.contains(&("{{ $guid }}", success())));
        assert!(colored.contains(&("/a/", fg())));
    }
}
