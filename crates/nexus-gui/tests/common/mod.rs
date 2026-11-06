//! Shared kittest harness: the whole app on a temp data dir, driven by role + label only.
#![allow(dead_code)]

pub mod servers;

use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use nexus_gui::{NexusApp, Opts};
use std::time::{Duration, Instant};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

pub type H = Harness<'static, NexusApp>;

pub fn harness(home: &std::path::Path) -> H {
    let app = NexusApp::new(home.to_path_buf(), Opts::default()).unwrap();
    let mut themed = false;
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut NexusApp| {
                if !themed {
                    nexus_gui::theme::install(ui.ctx());
                    themed = true;
                }
                app.show(ui)
            },
            app,
        );
    h.run();
    h
}

/// Runs frames until the UI settles; a UI that keeps repainting (in-flight send) is fine.
pub fn settle(h: &mut H) {
    let _ = h.run_ok();
}

pub fn click(h: &mut H, role: Role, label: &str) {
    // request and response panels both have tabs named "Body"/"Headers": the request one is first
    h.get_all_by_role_and_label(role, label)
        .next()
        .unwrap_or_else(|| panic!("no {role:?} named {label:?}"))
        .click();
    settle(h);
}

pub fn has(h: &H, role: Role, label: &str) -> bool {
    h.query_by_role_and_label(role, label).is_some()
}

/// Focuses a text input and types `text` into it.
pub fn type_into(h: &mut H, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).focus();
    settle(h);
    h.get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    settle(h);
}

/// Like [`type_into`] for the multi-line code editors.
pub fn type_into_editor(h: &mut H, label: &str, text: &str) {
    h.get_by_role_and_label(Role::MultilineTextInput, label)
        .focus();
    settle(h);
    h.get_by_role_and_label(Role::MultilineTextInput, label)
        .type_text(text);
    settle(h);
}

/// Like [`type_into`] for masked (secret) fields.
pub fn type_into_secret(h: &mut H, label: &str, text: &str) {
    h.get_by_role_and_label(Role::PasswordInput, label).focus();
    settle(h);
    h.get_by_role_and_label(Role::PasswordInput, label)
        .type_text(text);
    settle(h);
}

pub fn value_of(h: &H, label: &str) -> String {
    h.get_by_role_and_label(Role::TextInput, label)
        .value()
        .unwrap_or_default()
}

/// Picks `item` from the combo box named `combo`.
pub fn pick(h: &mut H, combo: &str, item: &str) {
    click(h, Role::ComboBox, combo);
    click(h, Role::Button, item);
}

/// Waits (pumping frames) until the active tab has a response.
pub fn wait_response(h: &mut H) {
    let t0 = Instant::now();
    loop {
        let _ = h.run_ok();
        if h.state()
            .tabs
            .active()
            .is_some_and(|t| t.response.is_some())
        {
            return;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "no response");
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub fn send(h: &mut H) {
    click(h, Role::Button, "Send");
    wait_response(h);
}

pub struct Server {
    pub rt: tokio::runtime::Runtime,
    pub mock: MockServer,
}

impl Server {
    pub fn start() -> Self {
        Self::with_delay(Duration::ZERO)
    }

    pub fn with_delay(delay: Duration) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mock = rt.block_on(async {
            let s = MockServer::start().await;
            Mock::given(any())
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_string("{\"ok\":true}")
                        .set_delay(delay),
                )
                .mount(&s)
                .await;
            s
        });
        Self { rt, mock }
    }

    pub fn url(&self) -> String {
        self.mock.uri()
    }

    /// The last request the server received.
    pub fn last(&self) -> wiremock::Request {
        self.rt
            .block_on(self.mock.received_requests())
            .unwrap()
            .pop()
            .expect("server received a request")
    }

    pub fn count(&self) -> usize {
        self.rt
            .block_on(self.mock.received_requests())
            .unwrap()
            .len()
    }
}

pub fn header(r: &wiremock::Request, name: &str) -> Option<String> {
    r.headers.get(name).map(|v| v.to_str().unwrap().to_string())
}

impl Server {
    /// A server answering exactly these paths (anything else: 404).
    pub fn routes(routes: Vec<(&str, ResponseTemplate)>) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mock = rt.block_on(async {
            let s = MockServer::start().await;
            for (p, t) in routes {
                Mock::given(wiremock::matchers::path(p))
                    .respond_with(t)
                    .mount(&s)
                    .await;
            }
            s
        });
        Self { rt, mock }
    }
}

/// Opener that records instead of launching a browser.
pub fn recording_opener() -> (
    nexus_gui::preview::Opener,
    std::sync::Arc<std::sync::Mutex<Vec<std::path::PathBuf>>>,
) {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let opener: nexus_gui::preview::Opener = std::sync::Arc::new(move |p| {
        s2.lock().unwrap().push(p.to_path_buf());
        Ok(())
    });
    (opener, seen)
}

/// Clicks the last node with this role and label (response tabs share names with request tabs).
pub fn click_last(h: &mut H, role: Role, label: &str) {
    h.get_all_by_role_and_label(role, label)
        .last()
        .unwrap_or_else(|| panic!("no {role:?} named {label:?}"))
        .click();
    settle(h);
}

/// Pumps frames until `cond` holds (panics after 20 s).
pub fn wait_until(h: &mut H, what: &str, cond: impl Fn(&NexusApp) -> bool) {
    let t0 = Instant::now();
    loop {
        let _ = h.run_ok();
        if cond(h.state()) {
            return;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out: {what}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Replaces the content of a text input: focus, select all, type.
pub fn replace_text(h: &mut H, role: Role, label: &str, text: &str) {
    h.get_by_role_and_label(role, label).focus();
    settle(h);
    h.key_press_modifiers(eframe::egui::Modifiers::COMMAND, eframe::egui::Key::A);
    settle(h);
    if text.is_empty() {
        h.key_press(eframe::egui::Key::Backspace);
    } else {
        h.get_by_role_and_label(role, label).type_text(text);
    }
    settle(h);
}

/// A tokio runtime that keeps local servers alive for the length of a test.
pub fn server_rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
