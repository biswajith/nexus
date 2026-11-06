//! Body tab: Pretty / Raw / Preview, search, copy, save, and the binary summary.

use super::parts::external_panel;
use super::RespAction;
use crate::highlight::Lang;
use crate::state::response::{Resp, ViewMode, MAX_MATCHES};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{self, accesskit::Role, pos2, vec2, Align, Color32, Key, Layout, Modifiers, Ui};
use nexus_core::response::format_size;
use nexus_core::types::ResponseFormat;

pub fn body_tab(ui: &mut Ui, resp: &mut Resp, acts: &mut Vec<RespAction>) {
    if resp.is_binary() {
        binary_view(ui, resp, acts);
        return;
    }
    // Ctrl+F opens the search (only while the body is on screen)
    if resp.view.mode != ViewMode::Preview
        && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F))
    {
        resp.view.search_open = true;
        resp.view.focus_search = true;
    }
    let is_html = resp.format == ResponseFormat::Html;
    toolbar(ui, resp, acts);
    if resp.view.search_open && resp.view.mode != ViewMode::Preview {
        search_bar(ui, resp);
    }
    if let Some(n) = resp.view.notice.clone() {
        let r = bar(ui, 24.0, bg_surface(), Some(border_subtle()));
        ui.painter().text(
            pos2(r.left() + 12.0, r.center().y),
            egui::Align2::LEFT_CENTER,
            &n,
            font(11.5, "Regular"),
            fg_2(),
        );
        let id = ui.scope_id().with("notice");
        a11y(ui.interact(r, id, egui::Sense::hover()), Role::Label, &n);
    }

    match resp.view.mode {
        ViewMode::Preview if is_html => external_panel(
            ui,
            "HTML preview",
            "The page opens in your browser with scripts disabled: there is no embedded web view.",
            "Open preview in browser",
            acts,
            RespAction::OpenPreview(resp.raw.text.clone()),
        ),
        mode => {
            refresh_matches(resp);
            let raw = mode == ViewMode::Raw;
            let doc = if raw { &resp.raw } else { &resp.pretty };
            let lang = match resp.format {
                ResponseFormat::Json => Lang::Json,
                ResponseFormat::Xml => Lang::Xml,
                ResponseFormat::Html => Lang::Html,
                _ => Lang::Text,
            };
            let label = if raw {
                "Response body (raw)"
            } else {
                "Response body"
            };
            let search_on = resp.view.search_open;
            let matches: &[_] = if search_on { &resp.view.matches } else { &[] };
            code_view(
                ui,
                doc,
                &mut resp.view.cache,
                &ViewOpts {
                    label,
                    lang,
                    matches,
                    current: (search_on && !matches.is_empty()).then_some(resp.view.current),
                    jump: resp.view.jump,
                },
            );
            resp.view.jump = false;
        }
    }
}

fn toolbar(ui: &mut Ui, resp: &mut Resp, acts: &mut Vec<RespAction>) {
    let r = bar(ui, 35.0, bg_surface(), Some(border_subtle()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (m, l) in [
            (ViewMode::Pretty, "Pretty"),
            (ViewMode::Raw, "Raw"),
            (ViewMode::Preview, "Preview"),
        ] {
            let active = resp.view.mode == m;
            let st = BtnStyle {
                fill: if active {
                    active_tint()
                } else {
                    Color32::TRANSPARENT
                },
                fill_hover: if active { active_tint() } else { hover_tint() },
                stroke: None,
                stroke_hover: None,
                text: if active { fg() } else { fg_2() },
                text_hover: fg(),
                radius: 4,
            };
            let f = font(11.0, if active { "SemiBold" } else { "Medium" });
            if button_role(ui, l, f, (10.0, 3.0), 22.0, &st, Role::Tab, Some(active)).clicked()
                && !active
            {
                resp.view.mode = m;
                resp.view.cache.clear();
                resp.view.match_key = None;
            }
        }
    });
    sub(
        ui,
        r.shrink2(vec2(12.0, 0.0)),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let copy = button(
                ui,
                "Copy",
                font(11.0, "Medium"),
                (10.0, 3.0),
                24.0,
                &BtnStyle::outline(),
            )
            .on_hover_text("Copy the body");
            a11y_name(&copy, "Copy response body");
            if copy.clicked() {
                let text = if resp.view.mode == ViewMode::Raw {
                    resp.raw.text.clone()
                } else {
                    resp.pretty.text.clone()
                };
                ui.ctx().copy_text(text);
                resp.view.notice = Some("Copied to the clipboard".into());
            }
            let save = button(
                ui,
                "Save",
                font(11.0, "Medium"),
                (10.0, 3.0),
                24.0,
                &BtnStyle::outline(),
            )
            .on_hover_text("Save the body to a file");
            a11y_name(&save, "Save response body");
            if save.clicked() {
                acts.push(RespAction::Save);
            }
            if resp.view.mode != ViewMode::Preview {
                let open = resp.view.search_open;
                let b = icon_button(ui, "Search response body", 26.0, |ui, c, col| {
                    search_icon(ui, c, if open { accent() } else { col })
                })
                .on_hover_text("Search (Ctrl+F)");
                if b.clicked() {
                    resp.view.search_open = !open;
                    resp.view.focus_search = !open;
                    if open {
                        close_search(resp);
                    }
                }
            }
        },
    );
}

fn close_search(resp: &mut Resp) {
    let v = &mut resp.view;
    v.search_open = false;
    v.search.clear();
    v.matches.clear();
    v.match_key = None;
    v.current = 0;
}

fn step(resp: &mut Resp, forward: bool) {
    let v = &mut resp.view;
    let n = v.matches.len();
    if n == 0 {
        return;
    }
    v.current = if forward {
        (v.current + 1) % n
    } else {
        (v.current + n - 1) % n
    };
    v.jump = true;
}

fn search_bar(ui: &mut Ui, resp: &mut Resp) {
    let r = bar(ui, 38.0, bg_surface(), Some(border_subtle()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let f = field(
            ui,
            "Search in response body",
            &mut resp.view.search,
            260.0,
            26.0,
            font(12.0, "Regular"),
            "Search...",
            bg_input(),
            true,
        );
        if resp.view.focus_search {
            f.request_focus();
            resp.view.focus_search = false;
        }
        let (enter, shift, esc) = ui.input(|i| {
            (
                i.key_pressed(Key::Enter),
                i.modifiers.shift,
                i.key_pressed(Key::Escape),
            )
        });
        if f.lost_focus() && enter {
            step(resp, !shift);
            f.request_focus();
        }
        if f.has_focus() && esc {
            close_search(resp);
            return;
        }
        refresh_matches(resp);
        let n = resp.view.matches.len();
        let count = if resp.view.search.is_empty() {
            String::new()
        } else if n == 0 {
            "No results".to_string()
        } else if n >= MAX_MATCHES {
            format!("{} / {}+", resp.view.current + 1, MAX_MATCHES)
        } else {
            format!("{} / {}", resp.view.current + 1, n)
        };
        labeled_text(
            ui,
            &count,
            &format!("Search matches {count}"),
            font(11.5, "Regular"),
            fg_2(),
        );
        if icon_button(ui, "Previous match", 24.0, |ui, c, col| {
            vchevron(ui, c, true, col)
        })
        .clicked()
        {
            step(resp, false);
        }
        if icon_button(ui, "Next match", 24.0, |ui, c, col| {
            vchevron(ui, c, false, col)
        })
        .clicked()
        {
            step(resp, true);
        }
        if icon_button(ui, "Close search", 24.0, |ui, c, col| {
            x_icon(ui, c, 3.5, col)
        })
        .clicked()
        {
            close_search(resp);
        }
    });
}

/// Recomputes the matches when the query or the shown document changed.
fn refresh_matches(resp: &mut Resp) {
    let v = &mut resp.view;
    if !v.search_open || v.search.is_empty() {
        v.matches.clear();
        v.match_key = None;
        v.current = 0;
        return;
    }
    let key = (v.search.clone(), v.mode);
    if v.match_key.as_ref() != Some(&key) {
        let doc = if v.mode == ViewMode::Raw {
            &resp.raw
        } else {
            &resp.pretty
        };
        v.matches = doc.find_all(&v.search, MAX_MATCHES);
        v.current = 0;
        v.jump = !v.matches.is_empty();
        v.match_key = Some(key);
    }
}

fn binary_view(ui: &mut Ui, resp: &mut Resp, acts: &mut Vec<RespAction>) {
    let Some(b) = resp.binary.as_mut() else {
        return;
    };
    let kind = match resp.format {
        ResponseFormat::Image => "Image",
        ResponseFormat::Pdf => "PDF document",
        _ => "Binary data",
    };
    let ct = if b.content_type.is_empty() {
        "unknown type".to_string()
    } else {
        b.content_type.clone()
    };
    let summary = format!("{kind}: {ct}, {}", format_size(b.bytes.len() as u64));
    let r = bar(ui, 35.0, bg_surface(), Some(border_subtle()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        labeled_text(ui, &summary, &summary, font(12.0, "Medium"), fg_2());
    });
    sub(
        ui,
        r.shrink2(vec2(12.0, 0.0)),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let save = button(
                ui,
                "Save",
                font(11.0, "Medium"),
                (10.0, 3.0),
                24.0,
                &BtnStyle::outline(),
            );
            a11y_name(&save, "Save response body");
            if save.clicked() {
                acts.push(RespAction::Save);
            }
            if button(
                ui,
                "Open externally",
                font(11.0, "Medium"),
                (10.0, 3.0),
                24.0,
                &BtnStyle::outline(),
            )
            .clicked()
            {
                acts.push(RespAction::OpenExternal);
            }
        },
    );
    if let Some(n) = resp.view.notice.clone() {
        let r = bar(ui, 24.0, bg_surface(), Some(border_subtle()));
        ui.painter().text(
            pos2(r.left() + 12.0, r.center().y),
            egui::Align2::LEFT_CENTER,
            &n,
            font(11.5, "Regular"),
            fg_2(),
        );
    }
    let area = ui.available_rect_before_wrap();
    ui.painter().rect_filled(area, 0, bg_sunken());
    if resp.view.texture.is_none() {
        if let Some(img) = b.image.take() {
            resp.view.texture = Some(ui.ctx().load_texture(
                "response-image",
                img,
                egui::TextureOptions::LINEAR,
            ));
        }
    }
    let Some(tex) = &resp.view.texture else {
        let note = if resp.format == ResponseFormat::Image {
            "This image cannot be shown here. Use Open externally or Save."
        } else {
            "This response is not text. Use Open externally or Save."
        };
        external_note(ui, area, note);
        return;
    };
    let avail = area.shrink(16.0);
    let t = tex.size_vec2();
    let scale = (avail.width() / t.x).min(avail.height() / t.y).min(1.0);
    let size = t * scale;
    let rect = egui::Rect::from_min_size(avail.min, size);
    let img = egui::Image::new((tex.id(), size)).alt_text("Response image");
    ui.put(rect, img);
}

fn external_note(ui: &mut Ui, area: egui::Rect, text: &str) {
    let r = ui.allocate_rect(area, egui::Sense::hover());
    a11y(r, Role::Label, text);
    ui.painter().text(
        area.center_top() + vec2(0.0, 48.0),
        egui::Align2::CENTER_CENTER,
        text,
        font(12.5, "Regular"),
        fg_3(),
    );
}
