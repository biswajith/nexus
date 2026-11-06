//! Reusable key/value table (params, headers, urlencoded) and the form-data table.
//!
//! Every table ends with a blank row that is not stored until something is typed in it, so there
//! is always a row to type into. Each control is labelled `"{what} {n} key"` etc. for assistive
//! technology and tests.

use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Color32, ComboBox, RichText, ScrollArea, Sense, Ui,
    WidgetInfo,
};
use nexus_core::types::{FormDataEntry, FormDataType, KeyValuePair};

fn mono(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Monospace)
}

fn blank_kv() -> KeyValuePair {
    KeyValuePair {
        key: String::new(),
        value: String::new(),
        enabled: true,
        description: None,
    }
}

fn is_blank_kv(r: &KeyValuePair) -> bool {
    r.enabled && r.key.is_empty() && r.value.is_empty() && r.description.is_none()
}

fn blank_form() -> FormDataEntry {
    FormDataEntry {
        key: String::new(),
        value: String::new(),
        entry_type: FormDataType::Text,
        enabled: true,
        description: None,
    }
}

fn is_blank_form(r: &FormDataEntry) -> bool {
    r.enabled
        && r.key.is_empty()
        && r.value.is_empty()
        && r.entry_type == FormDataType::Text
        && r.description.is_none()
}

fn header_row(ui: &mut Ui, w: f32, cols: &[(&str, f32)]) {
    let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let hr = hr.translate(vec2(12.0, 0.0));
    let mut x = hr.left() + 28.0 + 8.0;
    for (l, cw) in cols {
        spaced_text(
            ui,
            pos2(x, hr.center().y),
            l,
            font(11.0, "SemiBold"),
            fg_3(),
            0.66,
            false,
        );
        x += cw + 8.0;
    }
    hline(ui, hr.bottom() - 1.0, hr.left(), hr.right(), border());
}

fn add_button(ui: &mut Ui, label: &str) -> bool {
    let mut clicked = false;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        clicked = button(
            ui,
            label,
            font(12.0, "Medium"),
            (8.0, 4.0),
            26.0,
            &BtnStyle {
                fill: Color32::TRANSPARENT,
                fill_hover: accent_subtle(),
                stroke: None,
                stroke_hover: None,
                text: accent(),
                text_hover: accent(),
                radius: 4,
            },
        )
        .clicked();
    });
    clicked
}

fn remove_button(ui: &mut Ui, label: &str) -> bool {
    icon_button(ui, label, 24.0, |ui, c, col| x_icon(ui, c, 3.5, col)).clicked()
}

/// Key / value / description table. `what` names the rows ("Parameter", "Header", "Field").
pub fn kv_table(ui: &mut Ui, id: &str, what: &str, rows: &mut Vec<KeyValuePair>) {
    let area = ui.available_rect_before_wrap();
    let virtual_row = rows.last().is_none_or(|r| !is_blank_kv(r));
    if virtual_row {
        rows.push(blank_kv());
    }
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            let w = area.width() - 24.0;
            let col = ((w - 28.0 - 28.0 - 32.0) / 3.0).floor();
            ui.add_space(8.0);
            header_row(ui, w, &[("KEY", col), ("VALUE", col), ("DESCRIPTION", col)]);
            let mut remove: Option<usize> = None;
            let last = rows.len() - 1;
            #[allow(clippy::needless_range_loop)]
            for i in 0..rows.len() {
                let (rr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                let rr = rr.translate(vec2(12.0, 0.0));
                if ui.rect_contains_pointer(rr) {
                    ui.painter().rect_filled(rr, 0, hover_tint());
                }
                let n = i + 1;
                let row = &mut rows[i];
                let is_virtual = virtual_row && i == last;
                sub(ui, rr, ltr(), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.push_id((id, i), |ui| {
                        checkbox(ui, &format!("{what} {n} enabled"), &mut row.enabled);
                        ui.add_space(5.0);
                        field(
                            ui,
                            &format!("{what} {n} key"),
                            &mut row.key,
                            col,
                            26.0,
                            mono(12.0),
                            "Key",
                            bg_input(),
                            false,
                        );
                        field(
                            ui,
                            &format!("{what} {n} value"),
                            &mut row.value,
                            col,
                            26.0,
                            mono(12.0),
                            "Value",
                            bg_input(),
                            false,
                        );
                        let mut d = row.description.clone().unwrap_or_default();
                        field(
                            ui,
                            &format!("{what} {n} description"),
                            &mut d,
                            col,
                            26.0,
                            mono(12.0),
                            "Description",
                            bg_input(),
                            false,
                        );
                        if d != row.description.clone().unwrap_or_default() {
                            row.description = (!d.is_empty()).then_some(d);
                        }
                        if is_virtual {
                            ui.add_space(24.0);
                        } else if remove_button(ui, &format!("Remove {what} {n}")) {
                            remove = Some(i);
                        }
                    });
                });
                hline(
                    ui,
                    rr.bottom() - 1.0,
                    rr.left(),
                    rr.right(),
                    border_subtle(),
                );
            }
            if let Some(i) = remove {
                rows.remove(i);
            }
            // keep the virtual blank row out of the data until something is typed in it
            if virtual_row && rows.last().is_some_and(is_blank_kv) {
                rows.pop();
            }
            if add_button(ui, &format!("Add {what}")) {
                rows.push(blank_kv());
            }
        });
}

/// Form-data table: enabled, key, Text/File, value (text or file path + Browse), remove.
/// Returns the row whose Browse button was clicked.
pub fn form_table(ui: &mut Ui, id: &str, rows: &mut Vec<FormDataEntry>) -> Option<usize> {
    let what = "Form field";
    let area = ui.available_rect_before_wrap();
    let virtual_row = rows.last().is_none_or(|r| !is_blank_form(r));
    if virtual_row {
        rows.push(blank_form());
    }
    let mut browse = None;
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            let w = area.width() - 24.0;
            let type_w = 78.0;
            let browse_w = text_w(ui, "Browse", font(12.0, "Medium")) + 20.0;
            let rest = w - 28.0 - 28.0 - 32.0 - type_w;
            let key_w = (rest * 0.3).floor();
            let desc_w = (rest * 0.25).floor();
            let val_w = rest - key_w - desc_w;
            ui.add_space(8.0);
            header_row(
                ui,
                w,
                &[
                    ("KEY", key_w),
                    ("TYPE", type_w),
                    ("VALUE", val_w),
                    ("DESCRIPTION", desc_w),
                ],
            );
            let mut remove: Option<usize> = None;
            let last = rows.len() - 1;
            #[allow(clippy::needless_range_loop)]
            for i in 0..rows.len() {
                let (rr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                let rr = rr.translate(vec2(12.0, 0.0));
                if ui.rect_contains_pointer(rr) {
                    ui.painter().rect_filled(rr, 0, hover_tint());
                }
                let n = i + 1;
                let row = &mut rows[i];
                let is_virtual = virtual_row && i == last;
                sub(ui, rr, ltr(), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.push_id((id, i), |ui| {
                        checkbox(ui, &format!("{what} {n} enabled"), &mut row.enabled);
                        ui.add_space(5.0);
                        field(
                            ui,
                            &format!("{what} {n} key"),
                            &mut row.key,
                            key_w,
                            26.0,
                            mono(12.0),
                            "Key",
                            bg_input(),
                            false,
                        );
                        let ty = if row.entry_type == FormDataType::File {
                            "File"
                        } else {
                            "Text"
                        };
                        let combo = ComboBox::from_id_salt(("form_type", i))
                            .width(type_w - 12.0)
                            .selected_text(RichText::new(ty).font(font(12.0, "Regular")))
                            .show_ui(ui, |ui| {
                                for (t, l) in
                                    [(FormDataType::Text, "Text"), (FormDataType::File, "File")]
                                {
                                    if ui.selectable_label(row.entry_type == t, l).clicked()
                                        && row.entry_type != t
                                    {
                                        row.entry_type = t;
                                        row.value.clear();
                                    }
                                }
                            });
                        combo.response.widget_info(|| WidgetInfo {
                            label: Some(format!("{what} {n} type")),
                            current_text_value: Some(ty.into()),
                            ..WidgetInfo::new(Role::ComboBox)
                        });
                        if row.entry_type == FormDataType::File {
                            field(
                                ui,
                                &format!("{what} {n} file"),
                                &mut row.value,
                                val_w - browse_w - 8.0,
                                26.0,
                                mono(12.0),
                                "Path to file",
                                bg_input(),
                                false,
                            );
                            let b = button(
                                ui,
                                "Browse",
                                font(12.0, "Medium"),
                                (10.0, 4.0),
                                26.0,
                                &BtnStyle::outline(),
                            );
                            a11y_name(&b, &format!("Browse file for {what} {n}"));
                            if b.clicked() {
                                browse = Some(i);
                            }
                        } else {
                            field(
                                ui,
                                &format!("{what} {n} value"),
                                &mut row.value,
                                val_w,
                                26.0,
                                mono(12.0),
                                "Value",
                                bg_input(),
                                false,
                            );
                        }
                        let mut d = row.description.clone().unwrap_or_default();
                        field(
                            ui,
                            &format!("{what} {n} description"),
                            &mut d,
                            desc_w,
                            26.0,
                            mono(12.0),
                            "Description",
                            bg_input(),
                            false,
                        );
                        if d != row.description.clone().unwrap_or_default() {
                            row.description = (!d.is_empty()).then_some(d);
                        }
                        if is_virtual {
                            ui.add_space(24.0);
                        } else if remove_button(ui, &format!("Remove {what} {n}")) {
                            remove = Some(i);
                        }
                    });
                });
                hline(
                    ui,
                    rr.bottom() - 1.0,
                    rr.left(),
                    rr.right(),
                    border_subtle(),
                );
            }
            if let Some(i) = remove {
                rows.remove(i);
                browse = None;
            }
            if virtual_row && rows.last().is_some_and(is_blank_form) {
                rows.pop();
            }
            if add_button(ui, &format!("Add {what}")) {
                rows.push(blank_form());
            }
        });
    // a browse click on the virtual row needs it to exist
    if let Some(i) = browse {
        if i >= rows.len() {
            rows.push(blank_form());
            rows[i].entry_type = FormDataType::File;
        }
    }
    browse
}
