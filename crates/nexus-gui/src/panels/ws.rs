//! WebSocket client: connections with URL,
//! headers and subprotocols, a message composer and a filterable message log.

use crate::app::NexusApp;
use crate::highlight::Lang;
use crate::runtime::Msg;
use crate::state::ws::{DirFilter, NoteKind, WsRow, WsSession, WsTab};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, pos2, vec2, Align, Color32, FontId, Layout, Margin, Rect, ScrollArea, Sense, Ui,
};
use nexus_core::websocket::{WsConnectOptions, WsConnectionStatus, WsDirection, WsMessageType};

const ROW_H: f32 = 28.0;

fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

fn status_style(s: &WsSession) -> (&'static str, Color32) {
    match s.status {
        WsConnectionStatus::Idle => ("Not connected", fg_3()),
        WsConnectionStatus::Connecting if s.reconnecting => ("Reconnecting", warning()),
        WsConnectionStatus::Connecting => ("Connecting", warning()),
        WsConnectionStatus::Open => ("Connected", success()),
        WsConnectionStatus::Closing => ("Closing", warning()),
        WsConnectionStatus::Closed => ("Disconnected", fg_2()),
        WsConnectionStatus::Error => ("Error", danger()),
    }
}

enum Action {
    Connect,
    Disconnect,
    Send,
    Clear,
    AddSession,
    Select(usize),
    Close(usize),
}

impl NexusApp {
    /// Connects the active session (any earlier connection of the session is dropped).
    pub fn ws_connect(&mut self) {
        let idx = self.ws.active.min(self.ws.sessions.len() - 1);
        let s = &mut self.ws.sessions[idx];
        if s.url.trim().is_empty() || s.is_live() {
            return;
        }
        let headers: indexmap::IndexMap<String, String> = s
            .headers
            .iter()
            .filter(|h| h.enabled && !h.key.trim().is_empty())
            .map(|h| (h.key.trim().to_string(), h.value.clone()))
            .collect();
        let protocols: Vec<String> = s
            .protocols
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        let opts = WsConnectOptions {
            url: s.url.trim().to_string(),
            protocols: (!protocols.is_empty()).then_some(protocols),
            headers: (!headers.is_empty()).then_some(headers),
            auto_reconnect: s.auto_reconnect.then_some(true),
            ..Default::default()
        };
        if let Some(old) = s.conn.take() {
            self.app.ws_disconnect(&old);
        }
        let conn = format!("{}-{}", s.id, uuid::Uuid::new_v4());
        s.conn = Some(conn.clone());
        s.clear();
        s.error = None;
        s.reconnecting = false;
        s.status = WsConnectionStatus::Connecting;
        let notifier = self.rt.notifier();
        let _guard = self.rt.handle().enter();
        let res = self.app.ws_connect(&conn, opts, move |id, event| {
            notifier.send(Msg::Ws {
                conn: id.to_string(),
                event,
            })
        });
        if let Err(e) = res {
            s.status = WsConnectionStatus::Error;
            s.error = Some(e.to_string());
            s.note(NoteKind::Error, format!("Error: {e}"));
        }
    }

    pub fn ws_disconnect(&mut self) {
        let idx = self.ws.active.min(self.ws.sessions.len() - 1);
        if let Some(conn) = self.ws.sessions[idx].conn.clone() {
            self.app.ws_disconnect(&conn);
        }
    }

    /// Sends the composer text of the active session.
    pub fn ws_send(&mut self) {
        let idx = self.ws.active.min(self.ws.sessions.len() - 1);
        let s = &mut self.ws.sessions[idx];
        if s.draft.trim().is_empty() || !s.is_open() {
            return;
        }
        let Some(conn) = s.conn.clone() else { return };
        match self.app.ws_send(&conn, &s.draft) {
            Ok(m) => {
                s.push(WsRow::Message(m));
                s.draft.clear();
            }
            Err(e) => s.note(NoteKind::Error, format!("Send failed: {e}")),
        }
    }

    fn ws_close_session(&mut self, i: usize) {
        if self.ws.sessions.len() <= 1 || i >= self.ws.sessions.len() {
            return;
        }
        if let Some(conn) = self.ws.sessions[i].conn.take() {
            self.app.ws_disconnect(&conn);
        }
        self.ws.sessions.remove(i);
        if self.ws.active >= i && self.ws.active > 0 {
            self.ws.active -= 1;
        }
    }

    pub(crate) fn ws_panel(&mut self, ui: &mut Ui) {
        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0, bg_app());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut action: Option<Action> = None;
        self.ws.active = self.ws.active.min(self.ws.sessions.len() - 1);

        // session tabs
        {
            let r = bar(ui, 36.0, bg_sunken(), Some(border()));
            let active = self.ws.active;
            let many = self.ws.sessions.len() > 1;
            let items: Vec<(String, Color32)> = self
                .ws
                .sessions
                .iter()
                .map(|s| (s.name.clone(), status_style(s).1))
                .collect();
            sub(ui, r.shrink2(vec2(8.0, 0.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, (name, color)) in items.iter().enumerate() {
                    status_dot(ui, *color, &format!("{name} status"));
                    if underline_tab(ui, name, i == active, 36.0, None).clicked() {
                        action = Some(Action::Select(i));
                    }
                    if many
                        && icon_button(ui, &format!("Close {name}"), 22.0, |ui, c, col| {
                            x_icon(ui, c, 3.0, col)
                        })
                        .clicked()
                    {
                        action = Some(Action::Close(i));
                    }
                    ui.add_space(6.0);
                }
                if icon_button(ui, "New connection", 26.0, |ui, c, col| {
                    plus_icon(ui, c, 4.5, col)
                })
                .clicked()
                {
                    action = Some(Action::AddSession);
                }
            });
        }

        let idx = self.ws.active;
        let (live, open, can_connect) = {
            let s = &self.ws.sessions[idx];
            (s.is_live(), s.is_open(), !s.url.trim().is_empty())
        };
        // connection bar
        {
            let r = bar(ui, 58.0, bg_surface(), Some(border()));
            let s = &mut self.ws.sessions[idx];
            let (label, color) = status_style(s);
            let btn_w = 118.0;
            sub(ui, r.shrink2(vec2(12.0, 12.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                pill(
                    ui,
                    label,
                    color,
                    color.gamma_multiply(0.18),
                    &format!("Connection status: {label}"),
                );
                let w = ui.available_width() - btn_w - 10.0;
                ui.add_enabled_ui(!live, |ui| {
                    let resp = field(
                        ui,
                        "WebSocket URL",
                        &mut s.url,
                        w,
                        34.0,
                        mono(13.0),
                        "ws://localhost:8080",
                        bg_input(),
                        true,
                    );
                    if resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && can_connect
                    {
                        action = Some(Action::Connect);
                    }
                });
                if live {
                    if button(
                        ui,
                        "Disconnect",
                        font(13.0, "SemiBold"),
                        (20.0, 7.0),
                        34.0,
                        &BtnStyle::primary(danger(), danger().gamma_multiply(0.85)),
                    )
                    .clicked()
                    {
                        action = Some(Action::Disconnect);
                    }
                } else {
                    ui.add_enabled_ui(can_connect, |ui| {
                        let st = if can_connect {
                            BtnStyle::primary(accent(), accent_hover())
                        } else {
                            BtnStyle::primary(border(), border())
                        };
                        if button(
                            ui,
                            "Connect",
                            font(13.0, "SemiBold"),
                            (20.0, 7.0),
                            34.0,
                            &st,
                        )
                        .clicked()
                            && can_connect
                        {
                            action = Some(Action::Connect);
                        }
                    });
                }
            });
        }

        // sub tabs
        {
            let s = &mut self.ws.sessions[idx];
            let n_headers = s.headers.iter().filter(|h| !h.key.is_empty()).count();
            let items = [
                (WsTab::Messages, "Messages".to_string()),
                (
                    WsTab::Headers,
                    if n_headers > 0 {
                        format!("Headers ({n_headers})")
                    } else {
                        "Headers".to_string()
                    },
                ),
            ];
            if let Some(t) = tab_strip(ui, &items, s.tab) {
                s.tab = t;
            }
        }

        let tab = self.ws.sessions[idx].tab;
        match tab {
            WsTab::Headers => {
                let s = &mut self.ws.sessions[idx];
                ui.add_enabled_ui(!live, |ui| {
                    let top = bar(ui, 46.0, bg_app(), Some(border_subtle()));
                    sub(ui, top.shrink2(vec2(12.0, 8.0)), ltr(), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        field_label(ui, "Subprotocols");
                        field(
                            ui,
                            "Subprotocols",
                            &mut s.protocols,
                            320.0,
                            30.0,
                            mono(12.0),
                            "chat, v1.json (comma separated)",
                            bg_input(),
                            true,
                        );
                        ui.add_space(16.0);
                        checkbox(ui, "Auto-reconnect", &mut s.auto_reconnect);
                        field_label(ui, "Auto-reconnect");
                    });
                    crate::panels::request::kv::kv_table(
                        ui,
                        "ws-headers",
                        "Header",
                        &mut s.headers,
                    );
                });
            }
            WsTab::Messages => self.ws_messages(ui, &mut action),
        }

        match action {
            Some(Action::Connect) => self.ws_connect(),
            Some(Action::Disconnect) => self.ws_disconnect(),
            Some(Action::Send) => self.ws_send(),
            Some(Action::Clear) => self.ws.sessions[idx].clear(),
            Some(Action::AddSession) => self.ws.add(),
            Some(Action::Select(i)) => self.ws.active = i,
            Some(Action::Close(i)) => self.ws_close_session(i),
            None => {}
        }
        let _ = open;
    }

    fn ws_messages(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        let idx = self.ws.active;
        let s = &mut self.ws.sessions[idx];
        let open = s.is_open();

        // toolbar
        let r = bar(ui, 42.0, bg_app(), Some(border_subtle()));
        sub(ui, r.shrink2(vec2(12.0, 6.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            field(
                ui,
                "Filter messages",
                &mut s.filter,
                220.0,
                28.0,
                mono(12.0),
                "Filter messages",
                bg_input(),
                true,
            );
            for (f, l) in [
                (DirFilter::All, "All"),
                (DirFilter::Sent, "Sent"),
                (DirFilter::Received, "Received"),
            ] {
                if radio(ui, s.dir_filter == f, l).clicked() {
                    s.dir_filter = f;
                }
            }
            ui.add_space(8.0);
            let counts = format!("{} sent / {} received", s.sent_count(), s.received_count());
            labeled_text(ui, &counts, &counts, font(11.5, "Regular"), fg_3());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(
                    ui,
                    "Clear messages",
                    font(11.5, "Medium"),
                    (10.0, 4.0),
                    26.0,
                    &BtnStyle::outline(),
                )
                .clicked()
                {
                    *action = Some(Action::Clear);
                }
                field_label(ui, "Auto-scroll");
                checkbox(ui, "Auto-scroll", &mut s.auto_scroll);
            });
        });

        // composer height / detail height
        let area = ui.available_rect_before_wrap();
        let composer_h = 150.0;
        let upper = Rect::from_min_max(
            area.min,
            pos2(area.max.x, (area.max.y - composer_h).max(area.min.y + 60.0)),
        );
        let (list_rect, detail_rect) = if s.selected.is_some() {
            split(
                ui,
                upper,
                false,
                &mut self.ui.splits.ws,
                60.0,
                80.0,
                "Resize message list and detail",
            )
        } else {
            (upper, Rect::from_min_max(upper.max, upper.max))
        };
        let composer_rect = Rect::from_min_max(pos2(area.min.x, upper.max.y), area.max);
        let _ = ui.allocate_rect(area, Sense::hover());

        // message list
        let visible = s.visible();
        let mut clicked: Option<String> = None;
        let connected = open;
        sub(ui, list_rect, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if visible.is_empty() {
                let msg = if !s.rows.is_empty() {
                    "No messages match the filter"
                } else if connected {
                    "Waiting for messages..."
                } else {
                    "Connect to a WebSocket server to start"
                };
                note(ui, msg, msg);
                return;
            }
            ScrollArea::vertical()
                .id_salt(("ws-messages", &s.id))
                .auto_shrink([false, false])
                .stick_to_bottom(s.auto_scroll)
                .show_rows(ui, ROW_H, visible.len(), |ui, range| {
                    for vi in range {
                        let row = &s.rows[visible[vi]];
                        match row {
                            WsRow::Message(m) => {
                                let dir = if m.direction == WsDirection::Sent {
                                    "Sent"
                                } else {
                                    "Received"
                                };
                                let preview = one_line(&m.data);
                                let label = format!("{dir} {}: {preview}", clock(m.timestamp));
                                let sel = s.selected.as_deref() == Some(m.id.as_str());
                                let resp = list_row(ui, ROW_H, sel, &label, |ui, rect| {
                                    let rect = rect.shrink2(vec2(12.0, 0.0));
                                    let (arrow, col) = if m.direction == WsDirection::Sent {
                                        ("\u{2192}", accent())
                                    } else {
                                        ("\u{2190}", success())
                                    };
                                    let mut x = row_text(
                                        ui,
                                        rect,
                                        rect.left(),
                                        arrow,
                                        font(14.0, "Bold"),
                                        col,
                                    ) + 10.0;
                                    x = row_text(
                                        ui,
                                        rect,
                                        x,
                                        &clock(m.timestamp),
                                        mono(11.5),
                                        fg_3(),
                                    ) + 12.0;
                                    let mut rx = rect.right();
                                    rx = row_text_right(
                                        ui,
                                        rect,
                                        rx,
                                        &fmt_size(m.size),
                                        mono(11.5),
                                        fg_3(),
                                    ) - 12.0;
                                    if m.kind == WsMessageType::Binary {
                                        rx = row_text_right(
                                            ui,
                                            rect,
                                            rx,
                                            "binary",
                                            font(11.0, "SemiBold"),
                                            warning(),
                                        ) - 12.0;
                                    }
                                    let clip = Rect::from_min_max(rect.min, pos2(rx, rect.max.y));
                                    row_text(ui, clip, x, &preview, mono(12.5), fg());
                                });
                                if resp.clicked() {
                                    clicked = Some(m.id.clone());
                                }
                            }
                            WsRow::Note { at, kind, text } => {
                                let col = match kind {
                                    NoteKind::Info => success(),
                                    NoteKind::Warn => warning(),
                                    NoteKind::Error => danger(),
                                };
                                let label = format!("Connection event: {text}");
                                list_row(ui, ROW_H, false, &label, |ui, rect| {
                                    let rect = rect.shrink2(vec2(12.0, 0.0));
                                    let x = row_text(
                                        ui,
                                        rect,
                                        rect.left(),
                                        "\u{25CF}",
                                        font(10.0, "Regular"),
                                        col,
                                    ) + 10.0;
                                    let x = row_text(ui, rect, x, &clock(*at), mono(11.5), fg_3())
                                        + 12.0;
                                    row_text(ui, rect, x, text, font(12.0, "Medium"), col);
                                });
                            }
                        }
                    }
                });
        });
        if let Some(id) = clicked {
            s.selected = if s.selected.as_deref() == Some(id.as_str()) {
                None
            } else {
                Some(id)
            };
        }

        // detail of the selected message
        if let Some(sel) = s.selected.clone() {
            let found = s.rows.iter().find_map(|r| match r {
                WsRow::Message(m) if m.id == sel => Some(m.clone()),
                _ => None,
            });
            match found {
                Some(m) => {
                    if s.detail_for.as_deref() != Some(sel.as_str()) {
                        let text = serde_json::from_str::<serde_json::Value>(&m.data)
                            .ok()
                            .filter(|v| v.is_object() || v.is_array())
                            .map_or_else(|| m.data.clone(), |v| pretty(&v));
                        s.detail.set(text);
                        s.detail_for = Some(sel);
                    }
                    sub(ui, detail_rect, Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        let h = bar(ui, 26.0, bg_surface(), Some(border()));
                        let dir = if m.direction == WsDirection::Sent {
                            "Sent"
                        } else {
                            "Received"
                        };
                        sub(ui, h.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                            let t = format!(
                                "{dir} message - {} - {}",
                                fmt_size(m.size),
                                if m.kind == WsMessageType::Binary {
                                    "binary (hex)"
                                } else {
                                    "text"
                                }
                            );
                            labeled_text(ui, &t, &t, font(11.5, "Medium"), fg_2());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if button(
                                    ui,
                                    "Copy message",
                                    font(11.0, "Medium"),
                                    (8.0, 2.0),
                                    20.0,
                                    &BtnStyle::outline(),
                                )
                                .clicked()
                                {
                                    ui.ctx().copy_text(m.data.clone());
                                }
                            });
                        });
                        let lang = if s.detail.text().starts_with(['{', '[']) {
                            Lang::Json
                        } else {
                            Lang::Text
                        };
                        s.detail.show(ui, "Selected message", lang);
                    });
                }
                None => s.selected = None,
            }
        }

        // composer
        sub(ui, composer_rect, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.painter().rect_filled(composer_rect, 0, bg_surface());
            hline(
                ui,
                composer_rect.top(),
                composer_rect.left(),
                composer_rect.right(),
                border(),
            );
            let head = Rect::from_min_size(composer_rect.min, vec2(composer_rect.width(), 34.0));
            sub(ui, head.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                field_label(ui, "Message");
                let hint = if open {
                    "Ctrl+Enter sends"
                } else {
                    "Connect first"
                };
                labeled_text(ui, hint, hint, font(11.0, "Regular"), fg_3());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can = open && !s.draft.trim().is_empty();
                    ui.add_enabled_ui(can, |ui| {
                        let st = if can {
                            BtnStyle::primary(accent(), accent_hover())
                        } else {
                            BtnStyle::primary(border(), border())
                        };
                        if button(ui, "Send", font(12.5, "SemiBold"), (18.0, 5.0), 26.0, &st)
                            .clicked()
                            && can
                        {
                            *action = Some(Action::Send);
                        }
                    });
                });
            });
            let ed = Rect::from_min_max(
                pos2(composer_rect.left(), composer_rect.top() + 34.0),
                composer_rect.max,
            );
            sub(ui, ed, Layout::top_down(Align::Min), |ui| {
                let t = s.draft.trim_start();
                let lang = if t.starts_with('{') || t.starts_with('[') {
                    Lang::Json
                } else {
                    Lang::Text
                };
                let resp = code_editor(ui, "Message to send", &mut s.draft, lang);
                if resp.has_focus()
                    && open
                    && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
                {
                    *action = Some(Action::Send);
                }
            });
        });
        let _ = Margin::ZERO;
    }
}

/// First line of `s`, cut at 300 characters.
fn one_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("");
    let more = s.contains('\n');
    let mut out: String = line.chars().take(300).collect();
    if more || line.chars().count() > 300 {
        out.push('\u{2026}');
    }
    out
}
