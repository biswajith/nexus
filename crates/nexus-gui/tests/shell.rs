//! Drives the whole shell through accessible queries only (role + label), with a temp data dir.

use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use nexus_gui::{demo, NexusApp, Opts};

fn harness(home: &std::path::Path) -> Harness<'static, NexusApp> {
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

fn click(h: &mut Harness<'static, NexusApp>, role: Role, label: &str) {
    h.get_by_role_and_label(role, label).click();
    h.run();
}

fn type_into(h: &mut Harness<'static, NexusApp>, label: &str, text: &str) {
    let n = h.get_by_role_and_label(Role::TextInput, label);
    n.focus();
    h.run();
    h.get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    h.run();
}

#[test]
fn collection_request_tab_flow() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());

    click(&mut h, Role::Button, "New collection");
    type_into(&mut h, "Collection name", "Pets");
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    assert_eq!(h.state().collections.metas.len(), 1);
    assert!(h.query_by_role_and_label(Role::TreeItem, "Pets").is_some());

    // expand, add a request through the "more actions" menu
    click(&mut h, Role::TreeItem, "Pets");
    click(&mut h, Role::Button, "More actions for Pets");
    click(&mut h, Role::Button, "Add Request");
    type_into(&mut h, "New request name", "List pets");
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    h.run();

    // it opened in a tab
    assert!(h.query_by_role_and_label(Role::Tab, "List pets").is_some());
    assert!(h
        .query_by_role_and_label(Role::TreeItem, "List pets")
        .is_some());
    let tabs_before = h.state().tabs.tabs.len();

    // close it, then reopen from the tree
    click(&mut h, Role::Button, "Close List pets");
    assert!(h.query_by_role_and_label(Role::Tab, "List pets").is_none());
    assert_eq!(h.state().tabs.tabs.len(), tabs_before - 1);
    click(&mut h, Role::TreeItem, "List pets");
    assert!(h.query_by_role_and_label(Role::Tab, "List pets").is_some());

    // rename the collection and delete the request via context menus
    click(&mut h, Role::Button, "More actions for Pets");
    click(&mut h, Role::Button, "Rename");
    type_into(&mut h, "Rename collection", "2");
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    h.run();
    assert!(h.state().collections.metas[0].name.ends_with('2'));
    h.get_by_role_and_label(Role::TreeItem, "List pets")
        .click_secondary();
    h.run();
    click(&mut h, Role::Button, "Delete Request");
    assert!(h
        .query_by_role_and_label(Role::TreeItem, "List pets")
        .is_none());
    assert!(h.query_by_role_and_label(Role::Tab, "List pets").is_none());

    // delete the collection
    click(&mut h, Role::Button, "More actions for Pets2");
    click(&mut h, Role::Button, "Delete Collection");
    assert!(h.state().collections.metas.is_empty());
}

#[test]
fn environment_switch_persists_and_history_opens() {
    let dir = tempfile::tempdir().unwrap();
    demo::seed(dir.path()).unwrap();
    let mut h = harness(dir.path());

    click(&mut h, Role::ComboBox, "Select environment");
    click(&mut h, Role::Button, "Staging");
    assert_eq!(
        h.state().envs.name(h.state().ui.active_env_id.as_deref()),
        "Staging"
    );

    click(&mut h, Role::Tab, "History");
    h.run();
    h.run();
    let label = "GET https://api.example.com/users";
    click(&mut h, Role::Button, label);
    let t = h.state().tabs.active().unwrap();
    assert_eq!(t.request.url, "https://api.example.com/users");

    // mode tabs switch the central panel
    click(&mut h, Role::Tab, "Runner");
    assert!(h
        .query_by_role_and_label(Role::Button, "Run Collection")
        .is_some());

    // state is persisted and restored
    h.state_mut().save_ui_state();
    drop(h);
    let h2 = harness(dir.path());
    assert_eq!(
        h2.state().envs.name(h2.state().ui.active_env_id.as_deref()),
        "Staging"
    );
    assert!(h2
        .state()
        .tabs
        .tabs
        .iter()
        .any(|t| t.request.url == "https://api.example.com/users"));
}

#[test]
fn every_toolbar_and_tab_widget_has_a_role_and_label() {
    let dir = tempfile::tempdir().unwrap();
    demo::seed(dir.path()).unwrap();
    let h = harness(dir.path());
    for l in ["HTTP", "GraphQL", "WS", "MCP", "Runner"] {
        assert!(h.query_by_role_and_label(Role::Tab, l).is_some(), "{l}");
    }
    for (r, l) in [
        (Role::Button, "Toggle sidebar"),
        (Role::Button, "Send"),
        (Role::Button, "New request"),
        (Role::Button, "New collection"),
        (Role::ComboBox, "HTTP method"),
        (Role::TextInput, "Request URL"),
        (Role::Tab, "Collections"),
    ] {
        assert!(h.query_by_role_and_label(r, l).is_some(), "{l}");
    }
}
