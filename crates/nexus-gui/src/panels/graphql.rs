//! GraphQL client: endpoint + headers, query and
//! variables editors, response viewer, schema browser and subscriptions.

use crate::app::NexusApp;
use crate::highlight::Lang;
use crate::runtime::{GqlMsg, Msg};
use crate::state::gql_schema::Kind;
use crate::state::graphql::{EditorTab, ResultTab, SchemaView};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, FontId, Layout, Margin, Rect, RichText, ScrollArea,
    Sense, Ui,
};
use nexus_core::graphql::{GraphQLRequestOptions, SubscriptionEvent, SubscriptionOptions};

const ROW_H: f32 = 26.0;

fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

#[derive(PartialEq)]
enum Action {
    Send,
    Schema,
    Subscribe,
    Unsubscribe,
    ClearSub,
}

impl NexusApp {
    pub fn gql_send(&mut self) {
        let g = &mut self.gql;
        if g.url.trim().is_empty() || g.sending {
            return;
        }
        let variables = match g.variables_json() {
            Ok(v) => v,
            Err(e) => {
                g.result = Some(crate::state::graphql::GqlResult::failed(e));
                g.tab = ResultTab::Response;
                return;
            }
        };
        let opts = GraphQLRequestOptions {
            url: g.url.trim().to_string(),
            query: g.query.clone(),
            variables: Some(variables),
            operation_name: None,
            headers: g.headers_map(),
        };
        g.sending = true;
        g.result = None;
        g.tab = ResultTab::Response;
        let app = self.app.clone();
        self.rt.spawn(async move {
            Msg::Gql(GqlMsg::Response(
                app.graphql_send(&opts).await.map_err(|e| e.to_string()),
            ))
        });
    }

    pub fn gql_introspect(&mut self) {
        let g = &mut self.gql;
        if g.url.trim().is_empty() || g.schema_loading {
            return;
        }
        g.schema_loading = true;
        g.schema_error = None;
        g.tab = ResultTab::Schema;
        let url = g.url.trim().to_string();
        let headers = g.headers_map();
        let app = self.app.clone();
        self.rt.spawn(async move {
            // headers are not part of the core's schema cache key: always ask the server
            app.graphql.clear_schema_cache(Some(&url));
            let result = app
                .graphql_introspect(&url, headers)
                .await
                .map_err(|e| e.to_string());
            Msg::Gql(GqlMsg::Schema { url, result })
        });
    }

    pub fn gql_subscribe(&mut self) {
        let g = &mut self.gql;
        if g.url.trim().is_empty() {
            return;
        }
        let variables = match g.variables_json() {
            Ok(v) => v,
            Err(e) => {
                g.tab = ResultTab::Subscription;
                g.add_sub(chrono::Utc::now().timestamp_millis(), e, true);
                return;
            }
        };
        let opts = SubscriptionOptions {
            url: g.url.trim().to_string(),
            query: g.query.clone(),
            variables: Some(variables),
            operation_name: None,
            headers: g.headers_map(),
        };
        g.sub_events.clear();
        g.sub_count = 0;
        g.sub_selected = None;
        g.sub_view_for = None;
        g.sub_active = true;
        g.sub_notice = None;
        g.tab = ResultTab::Subscription;
        let notifier = self.rt.notifier();
        let _guard = self.rt.handle().enter();
        self.app.graphql_subscribe(opts, move |ev| {
            notifier.send(Msg::Gql(GqlMsg::Sub(ev)));
        });
    }

    pub fn gql_unsubscribe(&mut self) {
        self.app.graphql_unsubscribe();
        self.gql.sub_active = false;
    }

    pub(crate) fn on_gql(&mut self, m: GqlMsg) {
        let g = &mut self.gql;
        match m {
            GqlMsg::Response(r) => {
                g.sending = false;
                g.result = Some(match r {
                    Ok(resp) => crate::state::graphql::GqlResult::from_response(&resp),
                    Err(e) => crate::state::graphql::GqlResult::failed(e),
                });
            }
            GqlMsg::Schema { url, result } => {
                g.schema_loading = false;
                match result {
                    Ok(sdl) => {
                        g.schema_error = None;
                        g.set_schema(url, sdl);
                    }
                    Err(e) => g.schema_error = Some(e),
                }
            }
            GqlMsg::Sub(ev) => match ev {
                SubscriptionEvent::Data(m) => g.push_sub(m),
                SubscriptionEvent::Error(e) => {
                    g.add_sub(chrono::Utc::now().timestamp_millis(), e, true);
                }
                SubscriptionEvent::Complete { message_count } => {
                    g.sub_active = false;
                    g.sub_notice = Some(format!(
                        "Subscription completed after {message_count} {}",
                        if message_count == 1 {
                            "message"
                        } else {
                            "messages"
                        }
                    ));
                }
            },
        }
    }

    pub(crate) fn graphql_panel(&mut self, ui: &mut Ui) {
        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0, bg_app());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut action: Option<Action> = None;
        let g = &mut self.gql;
        set_variables_shared(ui.ctx(), g.words.clone());

        // endpoint bar
        let r = bar(ui, 58.0, bg_surface(), Some(border()));
        let can = !g.url.trim().is_empty();
        let send_w = text_w(ui, "Sending...", font(13.0, "SemiBold")) + 40.0;
        let schema_w = text_w(ui, "Schema", font(12.0, "Medium")) + 28.0;
        let sub_w = text_w(ui, "Subscribe", font(12.0, "Medium")) + 28.0;
        sub(ui, r.shrink2(vec2(12.0, 12.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let w = ui.available_width() - (send_w + schema_w + sub_w + 24.0);
            let resp = field(
                ui,
                "GraphQL endpoint URL",
                &mut g.url,
                w,
                34.0,
                mono(13.0),
                "https://api.example.com/graphql",
                bg_input(),
                true,
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can {
                action = Some(Action::Send);
            }
            let dim = |ui: &mut Ui, enabled: bool, f: &mut dyn FnMut(&mut Ui)| {
                ui.add_enabled_ui(enabled, |ui| f(ui));
            };
            dim(ui, can && !g.sending, &mut |ui| {
                let label = if g.sending { "Sending..." } else { "Send" };
                let st = if can && !g.sending {
                    BtnStyle::primary(accent(), accent_hover())
                } else {
                    BtnStyle::primary(border(), border())
                };
                if button(ui, label, font(13.0, "SemiBold"), (20.0, 7.0), 34.0, &st).clicked() {
                    action = Some(Action::Send);
                }
            });
            dim(ui, can && !g.schema_loading, &mut |ui| {
                let label = if g.schema_loading {
                    "Loading..."
                } else {
                    "Schema"
                };
                if button(
                    ui,
                    label,
                    font(12.0, "Medium"),
                    (14.0, 7.0),
                    34.0,
                    &BtnStyle::outline(),
                )
                .on_hover_text("Introspect the endpoint's schema")
                .clicked()
                {
                    action = Some(Action::Schema);
                }
            });
            if g.sub_active {
                if button(
                    ui,
                    "Stop",
                    font(12.0, "Medium"),
                    (14.0, 7.0),
                    34.0,
                    &BtnStyle::primary(danger(), danger().gamma_multiply(0.85)),
                )
                .clicked()
                {
                    action = Some(Action::Unsubscribe);
                }
            } else {
                dim(ui, can, &mut |ui| {
                    if button(
                        ui,
                        "Subscribe",
                        font(12.0, "Medium"),
                        (14.0, 7.0),
                        34.0,
                        &BtnStyle::outline(),
                    )
                    .on_hover_text("Run the query as a subscription (graphql-transport-ws)")
                    .clicked()
                    {
                        action = Some(Action::Subscribe);
                    }
                });
            }
        });

        let body = ui.available_rect_before_wrap();
        let _ = ui.allocate_rect(body, Sense::hover());
        let (left, right) = split(
            ui,
            body,
            true,
            &mut self.ui.splits.graphql,
            320.0,
            240.0,
            "Resize editors and results",
        );
        sub(ui, left, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            self.gql_editors(ui);
        });
        ui.painter().vline(
            left.max.x - 0.5,
            body.y_range(),
            egui::Stroke::new(1.0, border()),
        );
        sub(ui, right, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            self.gql_results(ui, &mut action);
        });

        match action {
            Some(Action::Send) => self.gql_send(),
            Some(Action::Schema) => self.gql_introspect(),
            Some(Action::Subscribe) => self.gql_subscribe(),
            Some(Action::Unsubscribe) => self.gql_unsubscribe(),
            Some(Action::ClearSub) => {
                self.gql.sub_events.clear();
                self.gql.sub_selected = None;
                self.gql.sub_view_for = None;
                self.gql.sub_notice = None;
            }
            None => {}
        }
    }

    fn gql_editors(&mut self, ui: &mut Ui) {
        let g = &mut self.gql;
        let area = ui.available_rect_before_wrap();
        let bottom_h = (area.height() * 0.38).clamp(150.0, 320.0);
        let top = Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - bottom_h));
        let bottom = Rect::from_min_max(pos2(area.min.x, area.max.y - bottom_h), area.max);
        let _ = ui.allocate_rect(area, Sense::hover());
        sub(ui, top, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            section_header(ui, "Query");
            code_editor(ui, "GraphQL query", &mut g.query, Lang::Graphql);
        });
        sub(ui, bottom, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let n = g.headers.iter().filter(|h| !h.key.is_empty()).count();
            let items = [
                (EditorTab::Variables, "Variables".to_string()),
                (
                    EditorTab::Headers,
                    if n > 0 {
                        format!("Headers ({n})")
                    } else {
                        "Headers".to_string()
                    },
                ),
            ];
            if let Some(t) = tab_strip(ui, &items, g.editor_tab) {
                g.editor_tab = t;
            }
            match g.editor_tab {
                EditorTab::Variables => {
                    code_editor(ui, "GraphQL variables", &mut g.variables, Lang::Json);
                }
                EditorTab::Headers => {
                    crate::panels::request::kv::kv_table(
                        ui,
                        "gql-headers",
                        "Header",
                        &mut g.headers,
                    );
                }
            }
        });
    }

    fn gql_results(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        let g = &mut self.gql;
        let items = [
            (ResultTab::Response, "Response".to_string()),
            (ResultTab::Schema, "Schema".to_string()),
            (
                ResultTab::Subscription,
                if g.sub_active {
                    "Subscription \u{25CF}".to_string()
                } else {
                    "Subscription".to_string()
                },
            ),
        ];
        if let Some(t) = tab_strip(ui, &items, g.tab) {
            g.tab = t;
        }
        match g.tab {
            ResultTab::Response => self.gql_response(ui),
            ResultTab::Schema => self.gql_schema(ui),
            ResultTab::Subscription => self.gql_subscription(ui, action),
        }
    }

    fn gql_response(&mut self, ui: &mut Ui) {
        let g = &mut self.gql;
        if g.sending {
            note(ui, "Sending query", "Sending query...");
            return;
        }
        let Some(res) = g.result.as_mut() else {
            note(
                ui,
                "Send a query to see the response",
                "Send a query to see the response",
            );
            return;
        };
        // status bar at the bottom
        let area = ui.available_rect_before_wrap();
        let status_h = 36.0;
        let main = Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - status_h));
        let status = Rect::from_min_max(pos2(area.min.x, area.max.y - status_h), area.max);
        let _ = ui.allocate_rect(area, Sense::hover());
        sub(ui, main, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if !res.errors.is_empty() {
                egui::Frame::NONE
                    .fill(danger_subtle())
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 4.0;
                        for e in &res.errors {
                            let resp = ui.add(
                                egui::Label::new(RichText::new(e).font(mono(12.0)).color(danger()))
                                    .wrap()
                                    .selectable(true),
                            );
                            a11y(resp, Role::Label, &format!("GraphQL error: {e}"));
                        }
                    });
            }
            if res.has_data {
                res.body.show(ui, "GraphQL response data", Lang::Json);
            } else if res.errors.is_empty() {
                note(ui, "The response has no data", "The response has no data");
            }
        });
        ui.painter().rect_filled(status, 0, bg_surface());
        hline(ui, status.top(), status.left(), status.right(), border());
        sub(ui, status.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if res.failure.is_some() {
                pill(
                    ui,
                    "Failed",
                    danger(),
                    danger_subtle(),
                    "Response status: Failed",
                );
            } else {
                let (bg, fg) = status_colors(res.status);
                let t = res.status.to_string();
                pill(ui, &t, fg, bg, &format!("Response status: {t}"));
                let t = fmt_dur(res.ms as f64);
                labeled_text(ui, &t, &format!("Response time: {t}"), mono(12.0), fg_2());
            }
        });
    }

    fn gql_schema(&mut self, ui: &mut Ui) {
        let g = &mut self.gql;
        if g.schema_loading {
            note(ui, "Loading schema", "Loading schema...");
            return;
        }
        if let Some(e) = &g.schema_error {
            let t = format!("Introspection failed: {e}");
            let area = ui.available_rect_before_wrap();
            let r = ui.allocate_rect(area, Sense::hover());
            a11y(r, Role::Label, &t);
            ui.painter().text(
                area.center_top() + vec2(0.0, 40.0),
                egui::Align2::CENTER_CENTER,
                &t,
                font(12.5, "Regular"),
                danger(),
            );
            return;
        }
        let Some(schema) = g.schema.as_ref() else {
            note(
                ui,
                "Click Schema to introspect",
                "Click \"Schema\" to introspect the endpoint",
            );
            return;
        };
        // toolbar
        let r = bar(ui, 42.0, bg_app(), Some(border_subtle()));
        sub(ui, r.shrink2(vec2(12.0, 6.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if g.schema_view == SchemaView::Browser {
                field(
                    ui,
                    "Search schema",
                    &mut g.schema_search,
                    220.0,
                    28.0,
                    mono(12.0),
                    "Search types and fields",
                    bg_input(),
                    true,
                );
            }
            if radio(ui, g.schema_view == SchemaView::Browser, "Browser").clicked() {
                g.schema_view = SchemaView::Browser;
            }
            if radio(ui, g.schema_view == SchemaView::Sdl, "SDL").clicked() {
                g.schema_view = SchemaView::Sdl;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(
                    ui,
                    "Copy SDL",
                    font(11.5, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &BtnStyle::outline(),
                )
                .clicked()
                {
                    ui.ctx().copy_text(g.sdl.text().to_string());
                }
            });
        });
        if g.schema_view == SchemaView::Sdl {
            g.sdl.show(ui, "Schema SDL", Lang::Graphql);
            return;
        }
        let hits = schema.search(&g.schema_search);
        let q = g.schema_search.trim().to_lowercase();
        let area = ui.available_rect_before_wrap();
        let _ = ui.allocate_rect(area, Sense::hover());
        let list_w = (area.width() * 0.38).clamp(200.0, 360.0);
        let list = Rect::from_min_max(area.min, pos2(area.min.x + list_w, area.max.y));
        let detail = Rect::from_min_max(pos2(list.max.x, area.min.y), area.max);
        let mut picked: Option<usize> = None;
        sub(ui, list, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if hits.is_empty() {
                note(ui, "No types match", "No types match");
                return;
            }
            ScrollArea::vertical()
                .id_salt("gql-types")
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, hits.len(), |ui, range| {
                    for hi in range {
                        let ti = hits[hi];
                        let t = &schema.types[ti];
                        let label = format!("{} {}", t.kind.label(), t.name);
                        let root = schema.roots.contains(&t.name);
                        let resp = list_row(
                            ui,
                            ROW_H,
                            g.selected_type == Some(ti),
                            &label,
                            |ui, rect| {
                                let rect = rect.shrink2(vec2(12.0, 0.0));
                                let kind_col = match t.kind {
                                    Kind::Object => accent(),
                                    Kind::Interface => syn_lit(),
                                    Kind::Input => warning(),
                                    Kind::Enum => syn_num(),
                                    Kind::Union => syn_lit(),
                                    Kind::Scalar => success(),
                                    Kind::Directive => fg_3(),
                                };
                                let x = row_text(
                                    ui,
                                    rect,
                                    rect.left(),
                                    t.kind.label(),
                                    font(10.5, "SemiBold"),
                                    kind_col,
                                );
                                let x = x.max(rect.left() + 56.0);
                                let end = row_text(
                                    ui,
                                    rect,
                                    x,
                                    &t.name,
                                    mono(12.5),
                                    if root { fg() } else { fg_2() },
                                );
                                if root {
                                    row_text(
                                        ui,
                                        rect,
                                        end + 8.0,
                                        "root",
                                        font(10.0, "Medium"),
                                        fg_3(),
                                    );
                                }
                            },
                        );
                        if resp.clicked() {
                            picked = Some(ti);
                        }
                    }
                });
        });
        ui.painter().vline(
            list.max.x - 0.5,
            area.y_range(),
            egui::Stroke::new(1.0, border_subtle()),
        );
        if let Some(ti) = picked {
            g.selected_type = Some(ti);
        }
        sub(ui, detail, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let Some(ti) = g.selected_type.filter(|i| *i < schema.types.len()) else {
                note(ui, "Select a type", "Select a type to see its fields");
                return;
            };
            let t = &schema.types[ti];
            let h = bar(ui, 54.0, bg_surface(), Some(border_subtle()));
            sub(
                ui,
                h.shrink2(vec2(12.0, 0.0)),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.add_space(8.0);
                    let title = format!("{} {}", t.kind.label(), t.name);
                    labeled_text(ui, &title, &title, FontId::new(14.0, mono_bold()), fg());
                    if !t.detail.is_empty() && t.kind != Kind::Directive {
                        ui.add_space(2.0);
                        labeled_text(ui, &t.detail, &t.detail, mono(11.5), fg_3());
                    }
                },
            );
            if !t.description.is_empty() {
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            egui::Label::new(
                                RichText::new(&t.description)
                                    .font(font(12.0, "Regular"))
                                    .color(fg_2()),
                            )
                            .wrap(),
                        );
                    });
            }
            let type_matches = q.is_empty() || t.name.to_lowercase().contains(&q);
            let fields: Vec<usize> = t
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| type_matches || f.name.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
            if t.fields.is_empty() {
                // scalar / union / directive: show the SDL
                if g.type_view_for != Some(ti) {
                    g.type_view.set(t.sdl.clone());
                    g.type_view_for = Some(ti);
                }
                g.type_view.show(ui, "Type definition", Lang::Graphql);
                return;
            }
            ScrollArea::vertical()
                .id_salt(("gql-fields", ti))
                .auto_shrink([false, false])
                .show_rows(ui, 38.0, fields.len(), |ui, range| {
                    for fi in range {
                        let f = &t.fields[fields[fi]];
                        let label = format!("Field {}", f.signature());
                        list_row(ui, 38.0, false, &label, |ui, rect| {
                            let rect = rect.shrink2(vec2(12.0, 0.0));
                            let top = Rect::from_min_size(rect.min, vec2(rect.width(), 22.0));
                            let mut x = row_text(
                                ui,
                                top,
                                rect.left(),
                                &f.name,
                                FontId::new(12.5, mono_bold()),
                                if f.deprecated { fg_3() } else { syn_key() },
                            );
                            if !f.args.is_empty() {
                                x = row_text(ui, top, x, &f.args, mono(12.0), fg_2());
                            }
                            if !f.ty.is_empty() {
                                x = row_text(ui, top, x, ": ", mono(12.0), fg_3());
                                x = row_text(ui, top, x, &f.ty, mono(12.0), syn_str());
                            }
                            if f.deprecated {
                                row_text(
                                    ui,
                                    top,
                                    x + 8.0,
                                    "deprecated",
                                    font(10.5, "SemiBold"),
                                    warning(),
                                );
                            }
                            if !f.description.is_empty() {
                                let bot = Rect::from_min_max(
                                    pos2(rect.left(), rect.top() + 20.0),
                                    rect.max,
                                );
                                row_text(
                                    ui,
                                    bot,
                                    rect.left(),
                                    &f.description.replace('\n', " "),
                                    font(11.5, "Regular"),
                                    fg_3(),
                                );
                            }
                        });
                    }
                });
        });
    }

    fn gql_subscription(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        let g = &mut self.gql;
        let r = bar(ui, 38.0, bg_app(), Some(border_subtle()));
        sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if g.sub_active {
                pill(
                    ui,
                    "Listening",
                    success(),
                    success_subtle(),
                    "Subscription status: Listening",
                );
            } else {
                pill(
                    ui,
                    "Stopped",
                    fg_2(),
                    active_tint(),
                    "Subscription status: Stopped",
                );
            }
            let t = format!(
                "{} {}",
                g.sub_events.len(),
                if g.sub_events.len() == 1 {
                    "event"
                } else {
                    "events"
                }
            );
            labeled_text(ui, &t, &t, font(11.5, "Regular"), fg_3());
            if let Some(n) = &g.sub_notice {
                labeled_text(ui, n, n, font(11.5, "Regular"), fg_2());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(
                    ui,
                    "Clear events",
                    font(11.5, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &BtnStyle::outline(),
                )
                .clicked()
                {
                    *action = Some(Action::ClearSub);
                }
            });
        });
        if g.sub_events.is_empty() {
            let (l, t) = if g.sub_active {
                (
                    "Waiting for subscription data",
                    "Waiting for subscription data...",
                )
            } else {
                (
                    "Subscribe to see real-time data",
                    "Subscribe to see real-time data",
                )
            };
            note(ui, l, t);
            return;
        }
        let area = ui.available_rect_before_wrap();
        let _ = ui.allocate_rect(area, Sense::hover());
        let detail_h = if g.sub_selected.is_some() {
            area.height() * 0.45
        } else {
            0.0
        };
        let list = Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - detail_h));
        let detail = Rect::from_min_max(pos2(area.min.x, list.max.y), area.max);
        let mut clicked = None;
        sub(ui, list, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ScrollArea::vertical()
                .id_salt("gql-sub-events")
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show_rows(ui, ROW_H, g.sub_events.len(), |ui, range| {
                    for i in range {
                        let e = &g.sub_events[i];
                        let preview = e.text.split_whitespace().collect::<Vec<_>>().join(" ");
                        let label = format!(
                            "{} {}: {}",
                            if e.is_error { "Error" } else { "Event" },
                            e.n,
                            preview
                        );
                        let resp =
                            list_row(ui, ROW_H, g.sub_selected == Some(i), &label, |ui, rect| {
                                let rect = rect.shrink2(vec2(12.0, 0.0));
                                let x = row_text(
                                    ui,
                                    rect,
                                    rect.left(),
                                    &format!("#{}", e.n),
                                    mono(11.5),
                                    fg_3(),
                                ) + 10.0;
                                let x = row_text(
                                    ui,
                                    rect,
                                    x.max(rect.left() + 40.0),
                                    &clock(e.at),
                                    mono(11.5),
                                    fg_3(),
                                ) + 12.0;
                                row_text(
                                    ui,
                                    rect,
                                    x,
                                    &preview,
                                    mono(12.5),
                                    if e.is_error { danger() } else { fg() },
                                );
                            });
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                    }
                });
        });
        if let Some(i) = clicked {
            g.sub_selected = if g.sub_selected == Some(i) {
                None
            } else {
                Some(i)
            };
        }
        if let Some(i) = g.sub_selected.filter(|i| *i < g.sub_events.len()) {
            if g.sub_view_for != Some(i) {
                g.sub_view.set(g.sub_events[i].text.clone());
                g.sub_view_for = Some(i);
            }
            sub(ui, detail, Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                hline(ui, detail.top(), detail.left(), detail.right(), border());
                g.sub_view
                    .show(ui, "Selected subscription event", Lang::Json);
            });
        }
    }
}
