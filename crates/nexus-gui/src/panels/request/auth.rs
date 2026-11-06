//! Auth tab. Field names on `AuthConfig.extra` are the keys nexus-core's auth handlers read
//! (`token`, `prefix`, `username`, `accessToken`, `addTo`, ...) so collections stay compatible.

use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, vec2, Align, ComboBox, FontFamily, FontId, Layout, RichText, Ui,
    WidgetInfo,
};
use nexus_core::types::{AuthConfig, AuthType};
use std::collections::HashSet;

const AUTH_TYPES: [(AuthType, &str); 8] = [
    (AuthType::Inherit, "Inherit from parent"),
    (AuthType::None, "No Auth"),
    (AuthType::Bearer, "Bearer Token"),
    (AuthType::Basic, "Basic Auth"),
    (AuthType::ApiKey, "API Key"),
    (AuthType::Oauth2, "OAuth 2.0"),
    (AuthType::Digest, "Digest Auth"),
    (AuthType::AwsSigV4, "AWS Signature v4"),
];

/// (key on `extra`, label, placeholder / default shown when unset, secret)
type Field = (&'static str, &'static str, &'static str, bool);

const BEARER: &[Field] = &[
    ("token", "Token", "{{auth_token}}", false),
    ("prefix", "Prefix", "Bearer", false),
];
const BASIC: &[Field] = &[
    ("username", "Username", "username", false),
    ("password", "Password", "password", true),
];
const API_KEY: &[Field] = &[
    ("key", "Key", "X-API-Key", false),
    ("value", "Value", "your-api-key", true),
];
const OAUTH_TOKEN: &[Field] = &[
    ("accessToken", "Access Token", "Paste or fetch token", true),
    ("tokenType", "Token Type", "Bearer", false),
];
const OAUTH_CONFIG: &[Field] = &[
    (
        "authUrl",
        "Auth URL",
        "https://auth.example.com/authorize",
        false,
    ),
    (
        "tokenUrl",
        "Token URL",
        "https://auth.example.com/token",
        false,
    ),
    ("clientId", "Client ID", "client-id", false),
    ("clientSecret", "Client Secret", "client-secret", true),
    ("scope", "Scope", "read write", false),
    (
        "redirectUri",
        "Redirect URI",
        "http://localhost:5556/callback",
        false,
    ),
];
const DIGEST: &[Field] = &[
    ("username", "Username", "username", false),
    ("password", "Password", "password", true),
    ("realm", "Realm", "realm", false),
    ("nonce", "Nonce", "nonce", false),
    ("algorithm", "Algorithm", "MD5", false),
    ("qop", "QOP", "auth", false),
    ("opaque", "Opaque", "opaque", false),
];
const AWS: &[Field] = &[
    ("accessKey", "Access Key", "AKIAIOSFODNN7EXAMPLE", false),
    (
        "secretKey",
        "Secret Key",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        true,
    ),
    ("region", "Region", "us-east-1", false),
    ("service", "Service", "execute-api", false),
    (
        "sessionToken",
        "Session Token",
        "optional session token",
        true,
    ),
];

fn caption(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(font(11.0, "SemiBold"))
            .color(fg_3()),
    );
}

fn auth_string(auth: &AuthConfig, key: &str) -> Option<String> {
    auth.extra.get(key).map(|v| match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

fn field_row(ui: &mut Ui, auth: &mut AuthConfig, f: &Field, revealed: &mut HashSet<String>) {
    let (key, label, default, secret) = *f;
    // an unset field shows its default but only stores what is typed
    let stored = auth_string(auth, key);
    let mut v = stored.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (lr, _) = ui.allocate_exact_size(vec2(110.0, 32.0), egui::Sense::hover());
        ui.painter().text(
            lr.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            font(12.0, "Medium"),
            fg_2(),
        );
        let shown = !secret || revealed.contains(key);
        let w = (ui.available_width() - if secret { 66.0 } else { 0.0 }).clamp(120.0, 520.0);
        let resp = field_with(
            ui,
            label,
            &mut v,
            w,
            32.0,
            FontId::new(12.5, FontFamily::Monospace),
            default,
            bg_input(),
            true,
            None,
            !shown,
        );
        let _ = resp;
        if secret {
            let name = format!("{} {label}", if shown { "Hide" } else { "Show" });
            let b = button(
                ui,
                if shown { "Hide" } else { "Show" },
                font(12.0, "Medium"),
                (10.0, 6.0),
                32.0,
                &BtnStyle::outline(),
            );
            a11y_name(&b, &name);
            if b.clicked() && !revealed.remove(key) {
                revealed.insert(key.to_string());
            }
        }
    });
    if Some(&v) != stored.as_ref() && !(stored.is_none() && v.is_empty()) {
        auth.extra.insert(key.to_string(), v.into());
    }
}

fn add_to(ui: &mut Ui, auth: &mut AuthConfig) {
    let cur = auth_string(auth, "addTo").unwrap_or_else(|| "header".into());
    ui.horizontal(|ui| {
        let (lr, _) = ui.allocate_exact_size(vec2(110.0, 32.0), egui::Sense::hover());
        ui.painter().text(
            lr.left_center(),
            egui::Align2::LEFT_CENTER,
            "Add to",
            font(12.0, "Medium"),
            fg_2(),
        );
        let text = if cur == "query" {
            "Query Params"
        } else {
            "Header"
        };
        let combo = ComboBox::from_id_salt("auth_add_to")
            .width(180.0)
            .selected_text(RichText::new(text).font(font(12.5, "Regular")))
            .show_ui(ui, |ui| {
                for (v, l) in [("header", "Header"), ("query", "Query Params")] {
                    if ui.selectable_label(cur == v, l).clicked() && cur != v {
                        auth.extra.insert("addTo".into(), v.into());
                    }
                }
            });
        combo.response.widget_info(|| WidgetInfo {
            label: Some("Add to".into()),
            current_text_value: Some(text.into()),
            ..WidgetInfo::new(Role::ComboBox)
        });
    });
}

fn note(ui: &mut Ui, text: &str) {
    let (_, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), egui::Sense::hover());
    a11y(resp.clone(), Role::Label, text);
    ui.painter().text(
        resp.rect.left_center(),
        egui::Align2::LEFT_CENTER,
        text,
        font(12.5, "Regular"),
        fg_3(),
    );
}

pub fn auth_editor(ui: &mut Ui, auth: &mut AuthConfig, revealed: &mut HashSet<String>) {
    let rect = ui.available_rect_before_wrap();
    sub(ui, rect.shrink(16.0), Layout::top_down(Align::Min), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("auth_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                caption(ui, "TYPE");
                let cur = AUTH_TYPES
                    .iter()
                    .find(|(t, _)| *t == auth.auth_type)
                    .map_or("Other", |(_, l)| *l);
                let combo = ComboBox::from_id_salt("authtype")
                    .width(220.0)
                    .selected_text(RichText::new(cur).font(font(12.5, "Regular")))
                    .show_ui(ui, |ui| {
                        for (t, l) in AUTH_TYPES {
                            // switching type keeps the other fields
                            if ui.selectable_label(auth.auth_type == t, l).clicked() {
                                auth.auth_type = t;
                            }
                        }
                    });
                combo.response.widget_info(|| WidgetInfo {
                    label: Some("Auth type".into()),
                    current_text_value: Some(cur.into()),
                    ..WidgetInfo::new(Role::ComboBox)
                });
                ui.add_space(4.0);
                match auth.auth_type {
                    AuthType::Inherit => note(
                        ui,
                        "This request will use the auth configuration from its parent folder or collection.",
                    ),
                    AuthType::None => note(ui, "This request does not use any authentication."),
                    AuthType::Bearer => {
                        for f in BEARER {
                            field_row(ui, auth, f, revealed);
                        }
                    }
                    AuthType::Basic => {
                        for f in BASIC {
                            field_row(ui, auth, f, revealed);
                        }
                    }
                    AuthType::ApiKey => {
                        for f in API_KEY {
                            field_row(ui, auth, f, revealed);
                        }
                        add_to(ui, auth);
                    }
                    AuthType::Oauth2 => {
                        for f in OAUTH_TOKEN {
                            field_row(ui, auth, f, revealed);
                        }
                        add_to(ui, auth);
                        ui.add_space(6.0);
                        caption(ui, "TOKEN CONFIGURATION");
                        for f in OAUTH_CONFIG {
                            field_row(ui, auth, f, revealed);
                        }
                    }
                    AuthType::Digest => {
                        for f in DIGEST {
                            field_row(ui, auth, f, revealed);
                        }
                    }
                    AuthType::AwsSigV4 => {
                        for f in AWS {
                            field_row(ui, auth, f, revealed);
                        }
                    }
                    AuthType::Hawk => note(ui, "Hawk authentication is not supported."),
                }
            });
    });
}
