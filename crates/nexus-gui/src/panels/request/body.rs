//! Body tab: mode selector and the editor for each mode.

use super::kv::{form_table, kv_table};
use crate::highlight::Lang;
use crate::runtime::PickTarget;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{self, accesskit::Role, pos2, vec2, Align, Align2, Layout, Sense, Ui};
use nexus_core::types::{BinaryBody, BodyMode, GraphqlBody, RequestBody};

const BODY_MODES: [(BodyMode, &str); 9] = [
    (BodyMode::None, "None"),
    (BodyMode::Json, "JSON"),
    (BodyMode::Xml, "XML"),
    (BodyMode::Text, "Text"),
    (BodyMode::Html, "HTML"),
    (BodyMode::FormData, "Form Data"),
    (BodyMode::XWwwFormUrlencoded, "URL Encoded"),
    (BodyMode::Binary, "Binary"),
    (BodyMode::Graphql, "GraphQL"),
];

/// Draws the body tab. Returns the file the user asked to browse for, if any.
pub fn body_editor(ui: &mut Ui, body: &mut RequestBody) -> Option<PickTarget> {
    let r = bar(ui, 35.0, bg_surface(), Some(border()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for (mode, label) in BODY_MODES {
            if radio(ui, body.mode == mode, label).clicked() {
                body.mode = mode;
            }
        }
    });
    if body.mode == BodyMode::Json {
        // right edge of the mode bar
        let at = egui::Rect::from_min_max(
            pos2(r.right() - 110.0, r.top() + 4.0),
            pos2(r.right() - 12.0, r.bottom() - 5.0),
        );
        sub(ui, at, ltr(), |ui| {
            let b = button(
                ui,
                "Format",
                font(12.0, "Medium"),
                (10.0, 4.0),
                24.0,
                &BtnStyle::outline(),
            );
            a11y_name(&b, "Format JSON");
            if b.clicked() {
                request_format(ui.ctx(), "Request body");
            }
        });
    }
    let mut pick = None;
    match body.mode {
        BodyMode::Json | BodyMode::Xml | BodyMode::Text | BodyMode::Html => {
            let lang = match body.mode {
                BodyMode::Json => Lang::Json,
                BodyMode::Xml => Lang::Xml,
                BodyMode::Html => Lang::Html,
                _ => Lang::Text,
            };
            let mut s = body.raw.clone().unwrap_or_default();
            code_editor(ui, "Request body", &mut s, lang);
            if s != body.raw.clone().unwrap_or_default() {
                body.raw = Some(s);
            }
        }
        BodyMode::None => hint(ui, "This request does not have a body"),
        BodyMode::XWwwFormUrlencoded => {
            let mut rows = body.urlencoded.take().unwrap_or_default();
            let orig = rows.clone();
            kv_table(ui, "urlencoded", "Field", &mut rows);
            body.urlencoded = (rows != orig || !rows.is_empty()).then_some(rows);
        }
        BodyMode::FormData => {
            let mut rows = body.form_data.take().unwrap_or_default();
            let orig = rows.clone();
            if let Some(i) = form_table(ui, "formdata", &mut rows) {
                pick = Some(PickTarget::FormRow(i));
            }
            body.form_data = (rows != orig || !rows.is_empty()).then_some(rows);
        }
        BodyMode::Binary => binary_editor(ui, body, &mut pick),
        BodyMode::Graphql => graphql_editor(ui, body),
    }
    pick
}

fn hint(ui: &mut Ui, text: &str) {
    let rect = ui.available_rect_before_wrap();
    let (_, resp) = ui.allocate_exact_size(vec2(rect.width(), 40.0), Sense::hover());
    a11y(resp, Role::Label, text);
    ui.painter().text(
        pos2(rect.left() + 12.0, rect.top() + 24.0),
        Align2::LEFT_CENTER,
        text,
        font(12.5, "Regular"),
        fg_3(),
    );
}

use crate::widgets::section_header as section_label;

fn binary_editor(ui: &mut Ui, body: &mut RequestBody, pick: &mut Option<PickTarget>) {
    let rect = ui.available_rect_before_wrap();
    sub(ui, rect.shrink(16.0), Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        let cur = body.binary.clone().unwrap_or(BinaryBody {
            path: String::new(),
            content_type: None,
        });
        let mut path = cur.path.clone();
        let mut ct = cur.content_type.clone().unwrap_or_default();
        ui.label(
            egui::RichText::new("FILE")
                .font(font(11.0, "SemiBold"))
                .color(fg_3()),
        );
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 90.0).min(520.0);
            field(
                ui,
                "Binary file path",
                &mut path,
                w,
                32.0,
                egui::FontId::new(12.5, egui::FontFamily::Monospace),
                "Path to the file to send as the body",
                bg_input(),
                true,
            );
            let b = button(
                ui,
                "Choose",
                font(12.0, "Medium"),
                (12.0, 6.0),
                32.0,
                &BtnStyle::outline(),
            );
            a11y_name(&b, "Choose binary file");
            if b.clicked() {
                *pick = Some(PickTarget::Binary);
            }
        });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("CONTENT TYPE")
                .font(font(11.0, "SemiBold"))
                .color(fg_3()),
        );
        field(
            ui,
            "Binary content type",
            &mut ct,
            ui.available_width().min(520.0),
            32.0,
            egui::FontId::new(12.5, egui::FontFamily::Monospace),
            "application/octet-stream",
            bg_input(),
            true,
        );
        if path != cur.path || ct != cur.content_type.unwrap_or_default() {
            body.binary = Some(BinaryBody {
                path,
                content_type: (!ct.is_empty()).then_some(ct),
            });
        }
    });
}

fn graphql_editor(ui: &mut Ui, body: &mut RequestBody) {
    let g = body.graphql.clone();
    let mut query = g.as_ref().map(|g| g.query.clone()).unwrap_or_default();
    let mut vars = g
        .as_ref()
        .and_then(|g| g.variables.clone())
        .unwrap_or_default();
    let h = ui.available_height();
    let q_h = (h * 0.55).floor();
    let area = ui.available_rect_before_wrap();
    let (top, bottom) = {
        let split = area.top() + q_h;
        (
            egui::Rect::from_min_max(area.min, pos2(area.right(), split)),
            egui::Rect::from_min_max(pos2(area.left(), split), area.max),
        )
    };
    sub(ui, top, Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        section_label(ui, "Query");
        code_editor(ui, "GraphQL query", &mut query, Lang::Graphql);
    });
    sub(ui, bottom, Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        section_label(ui, "Variables");
        code_editor(ui, "GraphQL variables", &mut vars, Lang::Json);
    });
    let old_vars = g
        .as_ref()
        .and_then(|g| g.variables.clone())
        .unwrap_or_default();
    if query != g.as_ref().map(|g| g.query.clone()).unwrap_or_default() || vars != old_vars {
        body.graphql = Some(GraphqlBody {
            query,
            variables: (!vars.is_empty()).then_some(vars),
        });
    }
}
