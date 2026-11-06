//! Scripts tab: pre-request / post-response editors with a snippet list.

use crate::highlight::Lang;
use crate::state::ScriptTab;
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{self, pos2, vec2, Align, Layout, Rect, Ui};
use nexus_core::types::NexusRequest;

const SNIPPETS: [(&str, &str); 7] = [
    ("Set env variable", "nx.environment.set(\"key\", \"value\");"),
    ("Get variable", "const val = nx.variables.get(\"key\");"),
    (
        "Test status 200",
        "nx.test(\"Status is 200\", () => {\n  nx.expect(nx.response.code).to.equal(200);\n});",
    ),
    (
        "Test has property",
        "nx.test(\"Has id\", () => {\n  nx.expect(nx.response.json()).to.have.property(\"id\");\n});",
    ),
    (
        "Test response time",
        "nx.test(\"Fast response\", () => {\n  nx.expect(nx.response.responseTime).to.be.below(500);\n});",
    ),
    ("Log response", "console.log(nx.response.json());"),
    (
        "Parse & store token",
        "const body = nx.response.json();\nnx.environment.set(\"token\", body.access_token);",
    ),
];

pub fn scripts_editor(ui: &mut Ui, req: &mut NexusRequest, tab: &mut ScriptTab) {
    let r = bar(ui, 33.0, bg_surface(), Some(border()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        for (t, l) in [
            (ScriptTab::Pre, "Pre-request"),
            (ScriptTab::Post, "Post-response"),
        ] {
            if underline_tab(ui, l, *tab == t, 33.0, None).clicked() {
                *tab = t;
            }
        }
    });
    let area = ui.available_rect_before_wrap();
    let snip_w = 170.0;
    let editor = Rect::from_min_max(area.min, pos2(area.right() - snip_w, area.bottom()));
    let side = Rect::from_min_max(pos2(area.right() - snip_w, area.top()), area.max);
    let (text, label) = match tab {
        ScriptTab::Pre => (&mut req.pre_request_script, "Pre-request script"),
        ScriptTab::Post => (&mut req.post_response_script, "Post-response script"),
    };
    let mut s = text.clone().unwrap_or_default();
    sub(ui, editor, Layout::top_down(Align::Min), |ui| {
        code_editor(ui, label, &mut s, Lang::JavaScript);
    });
    sub(ui, side, Layout::top_down(Align::Min), |ui| {
        ui.painter().rect_filled(side, 0, bg_surface());
        ui.painter().vline(
            side.left() + 0.5,
            side.y_range(),
            egui::Stroke::new(1.0, border()),
        );
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("SNIPPETS")
                    .font(font(11.0, "SemiBold"))
                    .color(fg_3()),
            );
        });
        egui::ScrollArea::vertical()
            .id_salt("snippets")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (name, code) in SNIPPETS {
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        let b = button(
                            ui,
                            name,
                            font(12.0, "Regular"),
                            (8.0, 4.0),
                            26.0,
                            &BtnStyle::flat(),
                        );
                        a11y_name(&b, &format!("Insert snippet: {name}"));
                        if b.clicked() {
                            if !s.is_empty() {
                                s.push('\n');
                            }
                            s.push_str(code);
                        }
                    });
                }
            });
    });
    if s != text.clone().unwrap_or_default() {
        *text = (!s.is_empty()).then_some(s);
    }
}
