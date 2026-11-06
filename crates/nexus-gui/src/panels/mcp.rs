//! MCP tester: connect to a server over stdio or HTTP,
//! browse tools / resources / prompts and call tools with arguments built from their schemas.

use crate::app::NexusApp;
use crate::highlight::Lang;
use crate::runtime::{McpMsg, McpSnapshot, Msg};
use crate::state::mcp::{
    build_args, parse_env, parse_prompts, parse_resources, parse_tools, split_args, ArgType,
    McpSession, McpStatus, ResultItem, ServerTab, ToolResult,
};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, Color32, ColorImage, FontId, Layout, Margin, Rect,
    RichText, ScrollArea, Sense, Ui,
};
use nexus_core::mcp::{McpConnectConfig, McpTransportType};
use serde_json::Value;

const ROW_H: f32 = 44.0;

fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

enum Action {
    Connect,
    Disconnect,
    Execute,
    AddSession,
    Select(usize),
    Close(usize),
}

fn status_style(s: &McpSession) -> (&'static str, Color32) {
    match s.status {
        McpStatus::Idle => ("Not connected", fg_3()),
        McpStatus::Connecting => ("Connecting", warning()),
        McpStatus::Connected => ("Connected", success()),
        McpStatus::Error => ("Error", danger()),
    }
}

impl NexusApp {
    pub fn mcp_connect(&mut self) {
        let idx = self.mcp.active.min(self.mcp.sessions.len() - 1);
        let s = &mut self.mcp.sessions[idx];
        if !s.can_connect() || matches!(s.status, McpStatus::Connecting | McpStatus::Connected) {
            return;
        }
        let config = match s.transport {
            McpTransportType::Stdio => McpConnectConfig {
                transport: McpTransportType::Stdio,
                command: Some(s.command.trim().to_string()),
                args: Some(split_args(&s.args)),
                env: Some(parse_env(&s.env)).filter(|e| !e.is_empty()),
                url: None,
            },
            McpTransportType::Http => McpConnectConfig {
                transport: McpTransportType::Http,
                command: None,
                args: None,
                env: None,
                url: Some(s.url.trim().to_string()),
            },
        };
        s.clear_server();
        s.history.clear();
        s.error = None;
        s.status = McpStatus::Connecting;
        let sid = s.id.clone();
        let app = self.app.clone();
        self.rt.spawn(async move {
            let result = async {
                let info = app
                    .mcp_connect(&sid, &config)
                    .await
                    .map_err(|e| e.to_string())?;
                let caps = info.get("capabilities").cloned().unwrap_or(Value::Null);
                let list = |v: Option<Value>| -> Vec<Value> {
                    v.and_then(|v| v.as_array().cloned()).unwrap_or_default()
                };
                // tools always, the others when advertised
                let tools = list(app.mcp_list_tools(&sid).await.ok());
                let resources = if caps.get("resources").is_some_and(|c| !c.is_null()) {
                    list(app.mcp_list_resources(&sid).await.ok())
                } else {
                    Vec::new()
                };
                let prompts = if caps.get("prompts").is_some_and(|c| !c.is_null()) {
                    list(app.mcp_list_prompts(&sid).await.ok())
                } else {
                    Vec::new()
                };
                Ok::<_, String>(Box::new(McpSnapshot {
                    info,
                    tools,
                    resources,
                    prompts,
                }))
            }
            .await;
            Msg::Mcp(McpMsg::Connected { sid, result })
        });
    }

    pub fn mcp_disconnect(&mut self) {
        let idx = self.mcp.active.min(self.mcp.sessions.len() - 1);
        let s = &mut self.mcp.sessions[idx];
        s.status = McpStatus::Idle;
        s.clear_server();
        let id = s.id.clone();
        let app = self.app.clone();
        self.rt.handle().spawn(async move {
            app.mcp_disconnect(&id).await;
        });
    }

    /// Calls the selected tool of the active session with the form's arguments.
    pub fn mcp_execute(&mut self) {
        let idx = self.mcp.active.min(self.mcp.sessions.len() - 1);
        let s = &mut self.mcp.sessions[idx];
        if s.executing || !s.connected() {
            return;
        }
        let Some(tool) = s
            .selected_tool
            .and_then(|i| s.tools.get(i))
            .map(|t| t.name.clone())
        else {
            return;
        };
        let args = match build_args(&s.fields) {
            Ok(a) => a,
            Err(e) => {
                s.form_error = Some(e);
                return;
            }
        };
        s.form_error = None;
        s.executing = true;
        s.result = None;
        let sid = s.id.clone();
        let app = self.app.clone();
        self.rt.spawn(async move {
            let t0 = std::time::Instant::now();
            let arg_value = Value::Object(args.clone());
            let result = app
                .mcp_call_tool(&sid, &tool, Some(args))
                .await
                .map_err(|e| e.to_string());
            Msg::Mcp(McpMsg::Called {
                sid,
                tool,
                args: arg_value,
                ms: t0.elapsed().as_secs_f64() * 1000.0,
                result,
            })
        });
    }

    pub(crate) fn on_mcp(&mut self, m: McpMsg) {
        match m {
            McpMsg::Connected { sid, result } => {
                let Some(s) = self.mcp.by_id(&sid) else {
                    // the session was closed while connecting
                    let app = self.app.clone();
                    self.rt
                        .handle()
                        .spawn(async move { app.mcp_disconnect(&sid).await });
                    return;
                };
                if s.status != McpStatus::Connecting {
                    return;
                }
                match result {
                    Ok(snap) => {
                        let info = &snap.info;
                        let si = info.get("serverInfo");
                        s.server_name = si
                            .and_then(|v| v.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or("MCP server")
                            .to_string();
                        s.server_version = si
                            .and_then(|v| v.get("version"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let cap = |k: &str| {
                            info.get("capabilities")
                                .and_then(|c| c.get(k))
                                .is_some_and(|c| !c.is_null())
                        };
                        s.caps = [cap("tools"), cap("resources"), cap("prompts")];
                        s.tools = parse_tools(&snap.tools);
                        s.resources = parse_resources(&snap.resources);
                        s.prompts = parse_prompts(&snap.prompts);
                        s.status = McpStatus::Connected;
                        s.tab = ServerTab::Tools;
                    }
                    Err(e) => {
                        s.status = McpStatus::Error;
                        s.error = Some(e);
                    }
                }
            }
            McpMsg::Called {
                sid,
                tool,
                args,
                ms,
                result,
            } => {
                let Some(s) = self.mcp.by_id(&sid) else {
                    return;
                };
                s.executing = false;
                let r = match result {
                    Ok(v) => ToolResult::from_value(&v),
                    Err(e) => ToolResult::error(e),
                };
                s.result_view.set(r.text.clone());
                // history keeps its own copy of the result
                let keep = ToolResult {
                    is_error: r.is_error,
                    items: r
                        .items
                        .iter()
                        .map(|i| match i {
                            ResultItem::Text(t) => ResultItem::Text(t.clone()),
                            ResultItem::Other(t) => ResultItem::Other(t.clone()),
                            ResultItem::Image { mime, data, .. } => ResultItem::Image {
                                mime: mime.clone(),
                                data: data.clone(),
                                texture: None,
                                broken: false,
                            },
                        })
                        .collect(),
                    structured: r.structured.clone(),
                    text: r.text.clone(),
                };
                s.result = Some(r);
                s.history.insert(
                    0,
                    crate::state::mcp::CallEntry {
                        tool,
                        args,
                        at: chrono::Utc::now().timestamp_millis(),
                        ms,
                        result: keep,
                    },
                );
                s.history.truncate(100);
            }
        }
    }

    fn mcp_close_session(&mut self, i: usize) {
        if self.mcp.sessions.len() <= 1 || i >= self.mcp.sessions.len() {
            return;
        }
        let s = self.mcp.sessions.remove(i);
        if self.mcp.active >= i && self.mcp.active > 0 {
            self.mcp.active -= 1;
        }
        if matches!(s.status, McpStatus::Connected | McpStatus::Connecting) {
            let app = self.app.clone();
            self.rt
                .handle()
                .spawn(async move { app.mcp_disconnect(&s.id).await });
        }
    }

    pub(crate) fn mcp_panel(&mut self, ui: &mut Ui) {
        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0, bg_app());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut action: Option<Action> = None;
        self.mcp.active = self.mcp.active.min(self.mcp.sessions.len() - 1);

        // session tabs
        {
            let r = bar(ui, 36.0, bg_sunken(), Some(border()));
            let active = self.mcp.active;
            let many = self.mcp.sessions.len() > 1;
            let items: Vec<(String, Color32)> = self
                .mcp
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
                if icon_button(ui, "New server connection", 26.0, |ui, c, col| {
                    plus_icon(ui, c, 4.5, col)
                })
                .clicked()
                {
                    action = Some(Action::AddSession);
                }
            });
        }

        let idx = self.mcp.active;
        self.mcp_connection_bar(ui, idx, &mut action);
        let s = &mut self.mcp.sessions[idx];
        if let Some(e) = s.error.clone() {
            let r = bar(ui, 34.0, danger_subtle(), None);
            sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let t = format!("Error: {e}");
                labeled_text(ui, &first_line(&t), &t, font(12.0, "Regular"), danger());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, "Dismiss error", 22.0, |ui, c, col| {
                        x_icon(ui, c, 3.0, col)
                    })
                    .clicked()
                    {
                        s.error = None;
                    }
                });
            });
        }
        let s = &mut self.mcp.sessions[idx];
        if s.connected() {
            let r = bar(ui, 34.0, bg_app(), Some(border_subtle()));
            sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let n = s.server_name.clone();
                labeled_text(
                    ui,
                    &n,
                    &format!("Server name: {n}"),
                    font(13.0, "SemiBold"),
                    fg(),
                );
                if !s.server_version.is_empty() {
                    let v = format!("v{}", s.server_version);
                    labeled_text(ui, &v, &format!("Server version: {v}"), mono(12.0), fg_3());
                }
                for (on, name) in s.caps.iter().zip(["Tools", "Resources", "Prompts"]) {
                    if *on {
                        pill(
                            ui,
                            name,
                            accent(),
                            accent_subtle(),
                            &format!("Capability: {name}"),
                        );
                    }
                }
            });
            self.mcp_main(ui, idx, &mut action);
        } else if s.status == McpStatus::Connecting {
            note(ui, "Connecting to the server", "Connecting...");
        } else {
            note(
                ui,
                "Connect to an MCP server to discover and test tools",
                "Connect to an MCP server to discover and test tools\nstdio (local commands) and HTTP (remote servers) - no LLM needed",
            );
        }

        match action {
            Some(Action::Connect) => self.mcp_connect(),
            Some(Action::Disconnect) => self.mcp_disconnect(),
            Some(Action::Execute) => self.mcp_execute(),
            Some(Action::AddSession) => self.mcp.add(),
            Some(Action::Select(i)) => self.mcp.active = i,
            Some(Action::Close(i)) => self.mcp_close_session(i),
            None => {}
        }
    }

    fn mcp_connection_bar(&mut self, ui: &mut Ui, idx: usize, action: &mut Option<Action>) {
        let s = &mut self.mcp.sessions[idx];
        let connected = s.connected();
        let connecting = s.status == McpStatus::Connecting;
        let can = s.can_connect();
        let r = bar(ui, 58.0, bg_surface(), Some(border()));
        let (label, color) = status_style(s);
        let btn_w = 118.0;
        let stdio = s.transport == McpTransportType::Stdio;
        sub(ui, r.shrink2(vec2(12.0, 12.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().interact_size.y = 34.0;
            pill(
                ui,
                label,
                color,
                color.gamma_multiply(0.18),
                &format!("Connection status: {label}"),
            );
            ui.add_enabled_ui(!connected && !connecting, |ui| {
                let cur = if stdio { "stdio" } else { "HTTP" };
                if let Some(i) = combo_pick(
                    ui,
                    "mcp-transport",
                    "Transport",
                    90.0,
                    cur,
                    &["stdio".to_string(), "HTTP".to_string()],
                ) {
                    s.transport = if i == 0 {
                        McpTransportType::Stdio
                    } else {
                        McpTransportType::Http
                    };
                }
                let avail = ui.available_width() - btn_w - 8.0;
                if stdio {
                    let cw = (avail * 0.34).floor();
                    let aw = (avail * 0.30).floor();
                    let ew = avail - cw - aw - 16.0;
                    for (label, text, w, hint) in [
                        (
                            "Command",
                            &mut s.command,
                            cw,
                            "Command (e.g. npx, node, python)",
                        ),
                        ("Arguments", &mut s.args, aw, "Args (space-separated)"),
                        ("Environment", &mut s.env, ew, "KEY=value KEY2=value2"),
                    ] {
                        let resp =
                            field(ui, label, text, w, 34.0, mono(12.5), hint, bg_input(), true);
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can
                        {
                            *action = Some(Action::Connect);
                        }
                    }
                } else {
                    let resp = field(
                        ui,
                        "Server URL",
                        &mut s.url,
                        avail,
                        34.0,
                        mono(13.0),
                        "http://localhost:3000/mcp",
                        bg_input(),
                        true,
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can {
                        *action = Some(Action::Connect);
                    }
                }
            });
            if connected {
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
                    *action = Some(Action::Disconnect);
                }
            } else {
                let enabled = can && !connecting;
                ui.add_enabled_ui(enabled, |ui| {
                    let st = if enabled {
                        BtnStyle::primary(accent(), accent_hover())
                    } else {
                        BtnStyle::primary(border(), border())
                    };
                    let l = if connecting {
                        "Connecting..."
                    } else {
                        "Connect"
                    };
                    if button(ui, l, font(13.0, "SemiBold"), (20.0, 7.0), 34.0, &st).clicked()
                        && enabled
                    {
                        *action = Some(Action::Connect);
                    }
                });
            }
        });
    }

    fn mcp_main(&mut self, ui: &mut Ui, idx: usize, action: &mut Option<Action>) {
        let area = ui.available_rect_before_wrap();
        let _ = ui.allocate_rect(area, Sense::hover());
        let (list, right) = split(
            ui,
            area,
            true,
            &mut self.ui.splits.mcp,
            240.0,
            320.0,
            "Resize list and details",
        );
        let s = &mut self.mcp.sessions[idx];
        sub(ui, list, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            mcp_list(ui, s);
        });
        ui.painter().vline(
            list.max.x - 0.5,
            area.y_range(),
            egui::Stroke::new(1.0, border()),
        );
        sub(ui, right, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            match s.tab {
                ServerTab::Tools => mcp_tester(ui, s, action),
                ServerTab::Resources => mcp_resource_detail(ui, s),
                ServerTab::Prompts => mcp_prompt_detail(ui, s),
            }
        });
    }
}

fn first_line(s: &str) -> String {
    let l = s.lines().next().unwrap_or("");
    let mut out: String = l.chars().take(240).collect();
    if s.lines().count() > 1 || l.chars().count() > 240 {
        out.push('\u{2026}');
    }
    out
}

fn mcp_list(ui: &mut Ui, s: &mut McpSession) {
    let items = [
        (ServerTab::Tools, format!("Tools ({})", s.tools.len())),
        (
            ServerTab::Resources,
            format!("Resources ({})", s.resources.len()),
        ),
        (ServerTab::Prompts, format!("Prompts ({})", s.prompts.len())),
    ];
    // tabs are narrow here: draw them compactly
    let r = bar(ui, 33.0, bg_surface(), Some(border()));
    sub(ui, r, ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let w = r.width() / 3.0;
        for (t, l) in &items {
            if underline_tab(ui, l, s.tab == *t, 33.0, Some(w)).clicked() {
                s.tab = *t;
            }
        }
    });
    match s.tab {
        ServerTab::Tools => {
            let fr = bar(ui, 40.0, bg_app(), Some(border_subtle()));
            sub(ui, fr.shrink2(vec2(10.0, 6.0)), ltr(), |ui| {
                let w = ui.available_width();
                field(
                    ui,
                    "Filter tools",
                    &mut s.filter,
                    w,
                    28.0,
                    mono(12.0),
                    "Filter tools",
                    bg_input(),
                    true,
                );
            });
            let vis = s.visible_tools();
            if vis.is_empty() {
                let t = if s.tools.is_empty() {
                    "No tools available"
                } else {
                    "No tools match the filter"
                };
                note(ui, t, t);
                return;
            }
            let mut pick = None;
            ScrollArea::vertical()
                .id_salt("mcp-tools")
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, vis.len(), |ui, range| {
                    for vi in range {
                        let t = &s.tools[vis[vi]];
                        let label = format!("Tool {}", t.name);
                        let resp = list_row(
                            ui,
                            ROW_H,
                            s.selected_tool == Some(vis[vi]),
                            &label,
                            |ui, rect| {
                                let rect = rect.shrink2(vec2(12.0, 0.0));
                                let top = Rect::from_min_size(rect.min, vec2(rect.width(), 24.0));
                                row_text(
                                    ui,
                                    top,
                                    rect.left(),
                                    &t.name,
                                    FontId::new(12.5, mono_bold()),
                                    fg(),
                                );
                                let bot = Rect::from_min_max(
                                    pos2(rect.left(), rect.top() + 20.0),
                                    rect.max,
                                );
                                row_text(
                                    ui,
                                    bot,
                                    rect.left(),
                                    &t.description.replace('\n', " "),
                                    font(11.5, "Regular"),
                                    fg_3(),
                                );
                            },
                        );
                        if resp.clicked() {
                            pick = Some(vis[vi]);
                        }
                    }
                });
            if let Some(i) = pick {
                s.select_tool(i);
            }
        }
        ServerTab::Resources => {
            if s.resources.is_empty() {
                note(ui, "No resources available", "No resources available");
                return;
            }
            let mut pick = None;
            ScrollArea::vertical()
                .id_salt("mcp-resources")
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, s.resources.len(), |ui, range| {
                    for i in range {
                        let r = &s.resources[i];
                        let label = format!("Resource {}", r.name);
                        let resp = list_row(
                            ui,
                            ROW_H,
                            s.selected_resource == Some(i),
                            &label,
                            |ui, rect| {
                                let rect = rect.shrink2(vec2(12.0, 0.0));
                                let top = Rect::from_min_size(rect.min, vec2(rect.width(), 24.0));
                                row_text(
                                    ui,
                                    top,
                                    rect.left(),
                                    &r.name,
                                    font(12.5, "SemiBold"),
                                    fg(),
                                );
                                let bot = Rect::from_min_max(
                                    pos2(rect.left(), rect.top() + 20.0),
                                    rect.max,
                                );
                                row_text(ui, bot, rect.left(), &r.uri, mono(11.0), fg_3());
                            },
                        );
                        if resp.clicked() {
                            pick = Some(i);
                        }
                    }
                });
            if let Some(i) = pick {
                s.selected_resource = Some(i);
                let r = &s.resources[i];
                s.detail.set(pretty(&serde_json::json!({
                    "uri": r.uri, "name": r.name, "description": r.description, "mimeType": r.mime
                })));
            }
        }
        ServerTab::Prompts => {
            if s.prompts.is_empty() {
                note(ui, "No prompts available", "No prompts available");
                return;
            }
            let mut pick = None;
            ScrollArea::vertical()
                .id_salt("mcp-prompts")
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, s.prompts.len(), |ui, range| {
                    for i in range {
                        let p = &s.prompts[i];
                        let label = format!("Prompt {}", p.name);
                        let resp = list_row(
                            ui,
                            ROW_H,
                            s.selected_prompt == Some(i),
                            &label,
                            |ui, rect| {
                                let rect = rect.shrink2(vec2(12.0, 0.0));
                                let top = Rect::from_min_size(rect.min, vec2(rect.width(), 24.0));
                                row_text(
                                    ui,
                                    top,
                                    rect.left(),
                                    &p.name,
                                    FontId::new(12.5, mono_bold()),
                                    fg(),
                                );
                                let bot = Rect::from_min_max(
                                    pos2(rect.left(), rect.top() + 20.0),
                                    rect.max,
                                );
                                row_text(
                                    ui,
                                    bot,
                                    rect.left(),
                                    &p.description.replace('\n', " "),
                                    font(11.5, "Regular"),
                                    fg_3(),
                                );
                            },
                        );
                        if resp.clicked() {
                            pick = Some(i);
                        }
                    }
                });
            if let Some(i) = pick {
                s.selected_prompt = Some(i);
                let p = &s.prompts[i];
                s.detail.set(pretty(&serde_json::json!({
                    "name": p.name,
                    "description": p.description,
                    "arguments": p.arguments.iter().map(|(n, d, r)| serde_json::json!({"name": n, "description": d, "required": r})).collect::<Vec<_>>()
                })));
            }
        }
    }
}

fn mcp_resource_detail(ui: &mut Ui, s: &mut McpSession) {
    match s.selected_resource {
        Some(_) => {
            section_header(ui, "Resource");
            s.detail.show(ui, "Resource details", Lang::Json);
        }
        None => note(
            ui,
            "Select a resource",
            "Select a resource to see its details",
        ),
    }
}

fn mcp_prompt_detail(ui: &mut Ui, s: &mut McpSession) {
    match s.selected_prompt {
        Some(_) => {
            section_header(ui, "Prompt");
            s.detail.show(ui, "Prompt details", Lang::Json);
        }
        None => note(ui, "Select a prompt", "Select a prompt to see its details"),
    }
}

fn mcp_tester(ui: &mut Ui, s: &mut McpSession, action: &mut Option<Action>) {
    let Some(ti) = s.selected_tool.filter(|i| *i < s.tools.len()) else {
        note(
            ui,
            "Select a tool from the list to test it",
            "Select a tool from the list to test it\nCall any tool with custom arguments - no LLM needed",
        );
        return;
    };
    let tool = s.tools[ti].clone();
    let area = ui.available_rect_before_wrap();
    let _ = ui.allocate_rect(area, Sense::hover());
    let history_h = if s.history.is_empty() { 0.0 } else { 120.0 };
    let main = Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - history_h));
    let hist = Rect::from_min_max(pos2(area.min.x, main.max.y), area.max);
    let has_result = s.result.is_some() || s.executing;
    let form_h = if has_result {
        (main.height() * 0.45).max(150.0)
    } else {
        main.height()
    };
    let form = Rect::from_min_max(
        main.min,
        pos2(main.max.x, (main.min.y + form_h).min(main.max.y)),
    );
    let result = Rect::from_min_max(pos2(main.min.x, form.max.y), main.max);

    // header + form
    sub(ui, form, Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let h = bar(ui, 44.0, bg_surface(), Some(border()));
        sub(ui, h.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
            labeled_text(
                ui,
                &tool.name,
                &format!("Selected tool: {}", tool.name),
                FontId::new(14.0, mono_bold()),
                fg(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if s.executing { "Running..." } else { "Execute" };
                ui.add_enabled_ui(!s.executing, |ui| {
                    if button(
                        ui,
                        label,
                        font(12.5, "SemiBold"),
                        (18.0, 5.0),
                        28.0,
                        &BtnStyle::primary(accent(), accent_hover()),
                    )
                    .clicked()
                    {
                        *action = Some(Action::Execute);
                    }
                });
            });
        });
        ScrollArea::vertical()
            .id_salt(("mcp-form", &s.id, &tool.name))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 8.0;
                        if !tool.description.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&tool.description)
                                        .font(font(12.5, "Regular"))
                                        .color(fg_2()),
                                )
                                .wrap(),
                            );
                        }
                        if s.fields.is_empty() {
                            labeled_text(
                                ui,
                                "This tool takes no arguments",
                                "This tool takes no arguments",
                                font(12.0, "Regular"),
                                fg_3(),
                            );
                        } else {
                            labeled_text(
                                ui,
                                "ARGUMENTS",
                                "Arguments",
                                font(11.0, "SemiBold"),
                                fg_3(),
                            );
                        }
                        let w = ui.available_width();
                        for f in s.fields.iter_mut() {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let name = if f.required {
                                    format!("{} *", f.key)
                                } else {
                                    f.key.clone()
                                };
                                labeled_text(
                                    ui,
                                    &name,
                                    &format!("Argument {}", f.key),
                                    FontId::new(12.5, mono_bold()),
                                    fg(),
                                );
                                labeled_text(
                                    ui,
                                    &f.schema_type,
                                    &format!("{} type", f.key),
                                    font(11.0, "Regular"),
                                    fg_3(),
                                );
                            });
                            if !f.description.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&f.description)
                                            .font(font(11.5, "Regular"))
                                            .color(fg_3()),
                                    )
                                    .wrap(),
                                );
                            }
                            let label = format!("Argument {} value", f.key);
                            if !f.choices.is_empty() || f.ty == ArgType::Boolean {
                                let mut opts = vec!["(unset)".to_string()];
                                if f.ty == ArgType::Boolean {
                                    opts.push("true".into());
                                    opts.push("false".into());
                                } else {
                                    opts.extend(f.choices.iter().cloned());
                                }
                                let cur = if f.value.is_empty() {
                                    "(unset)".to_string()
                                } else {
                                    f.value.clone()
                                };
                                if let Some(i) = combo_pick(
                                    ui,
                                    &format!("arg-{}", f.key),
                                    &label,
                                    220.0,
                                    &cur,
                                    &opts,
                                ) {
                                    f.value = if i == 0 {
                                        String::new()
                                    } else {
                                        opts[i].clone()
                                    };
                                }
                            } else if f.ty == ArgType::Json {
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(w, 90.0), Sense::hover());
                                sub(ui, rect, Layout::top_down(Align::Min), |ui| {
                                    code_editor(ui, &label, &mut f.value, Lang::Json);
                                });
                            } else {
                                field(
                                    ui,
                                    &label,
                                    &mut f.value,
                                    w,
                                    30.0,
                                    mono(12.5),
                                    f.ty.label(),
                                    bg_input(),
                                    true,
                                );
                            }
                            ui.add_space(4.0);
                        }
                        if let Some(e) = &s.form_error {
                            labeled_text(
                                ui,
                                e,
                                &format!("Argument error: {e}"),
                                font(12.0, "Regular"),
                                danger(),
                            );
                        }
                    });
            });
    });

    // result
    if has_result {
        sub(ui, result, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if s.executing {
                note(ui, "Calling the tool", "Running...");
                return;
            }
            let Some(r) = s.result.as_mut() else { return };
            let h = bar(ui, 34.0, bg_surface(), Some(border()));
            sub(ui, h.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if r.is_error {
                    pill(ui, "Error", danger(), danger_subtle(), "Tool result: Error");
                } else {
                    pill(
                        ui,
                        "Success",
                        success(),
                        success_subtle(),
                        "Tool result: Success",
                    );
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(
                        ui,
                        "Copy result",
                        font(11.5, "Medium"),
                        (10.0, 4.0),
                        24.0,
                        &BtnStyle::outline(),
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(r.text.clone());
                    }
                });
            });
            for item in r.items.iter_mut() {
                if let ResultItem::Image {
                    mime,
                    data,
                    texture,
                    broken,
                } = item
                {
                    if texture.is_none() && !*broken {
                        match decode_image(data) {
                            Some(img) => {
                                *texture = Some(ui.ctx().load_texture(
                                    "mcp-image",
                                    img,
                                    Default::default(),
                                ))
                            }
                            None => *broken = true,
                        }
                    }
                    match texture {
                        Some(t) => {
                            let size = t.size_vec2();
                            let k = (200.0 / size.y).min(1.0);
                            let resp = ui.add(egui::Image::new(&*t).fit_to_exact_size(size * k));
                            a11y(resp, Role::Image, &format!("Tool result image ({mime})"));
                        }
                        None => {
                            labeled_text(
                                ui,
                                &format!("Image ({mime}) could not be decoded"),
                                &format!("Image ({mime}) could not be decoded"),
                                font(12.0, "Regular"),
                                warning(),
                            );
                        }
                    }
                }
            }
            let t = s.result_view.text();
            let lang = if t.trim_start().starts_with('{') || t.trim_start().starts_with('[') {
                Lang::Json
            } else {
                Lang::Text
            };
            if let Some(st) = &r.structured {
                let _ = st;
            }
            s.result_view.show(ui, "Tool result", lang);
        });
    }

    // call history
    if !s.history.is_empty() {
        sub(ui, hist, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let h = bar(ui, 26.0, bg_surface(), Some(border_subtle()));
            let mut clear = false;
            sub(ui, h.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                labeled_text(
                    ui,
                    "CALL HISTORY",
                    "Call history",
                    font(11.0, "SemiBold"),
                    fg_3(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(
                        ui,
                        "Clear history",
                        font(11.0, "Medium"),
                        (8.0, 2.0),
                        20.0,
                        &BtnStyle::flat(),
                    )
                    .clicked()
                    {
                        clear = true;
                    }
                });
            });
            let mut show: Option<usize> = None;
            ScrollArea::vertical()
                .id_salt("mcp-history")
                .auto_shrink([false, false])
                .show_rows(ui, 26.0, s.history.len(), |ui, range| {
                    for i in range {
                        let e = &s.history[i];
                        let args = e.args.to_string();
                        let label = format!(
                            "Call {}{}",
                            e.tool,
                            if e.result.is_error { " (error)" } else { "" }
                        );
                        let resp = list_row(ui, 26.0, false, &label, |ui, rect| {
                            let rect = rect.shrink2(vec2(12.0, 0.0));
                            let mut x = row_text(
                                ui,
                                rect,
                                rect.left(),
                                &e.tool,
                                FontId::new(12.0, mono_bold()),
                                if e.result.is_error { danger() } else { fg() },
                            ) + 10.0;
                            let rx = row_text_right(
                                ui,
                                rect,
                                rect.right(),
                                &format!("{} ms   {}", e.ms.round(), clock(e.at)),
                                mono(11.0),
                                fg_3(),
                            );
                            let clip = Rect::from_min_max(rect.min, pos2(rx - 10.0, rect.max.y));
                            x = row_text(ui, clip, x, &args, mono(11.0), fg_3());
                            let _ = x;
                        });
                        if resp.clicked() {
                            show = Some(i);
                        }
                    }
                });
            if clear {
                s.history.clear();
            }
            if let Some(i) = show {
                let e = &s.history[i];
                let r = ToolResult {
                    is_error: e.result.is_error,
                    items: Vec::new(),
                    structured: e.result.structured.clone(),
                    text: e.result.text.clone(),
                };
                s.set_result(r);
            }
        });
    }
}

fn decode_image(b64: &str) -> Option<ColorImage> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    if size[0] * size[1] > 16_000_000 {
        return None;
    }
    Some(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}
