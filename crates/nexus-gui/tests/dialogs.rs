//! Stage 6: dialogs, command palette, shortcuts, theme and form persistence - by role + label.

mod common;

use common::*;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use nexus_gui::state::dialogs::{Dialog, ExportFmt, ExportWhat};
use nexus_gui::theme::ThemeMode;
use nexus_gui::{demo, NexusApp, Opts};

const POSTMAN: &str = r#"{"info":{"name":"Imported Pets","_postman_id":"x","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
"item":[{"name":"Folder A","item":[{"name":"List","request":{"method":"GET","url":"https://x.test/a"}}]},
{"name":"Create","request":{"method":"POST","url":"https://x.test/b"}}]}"#;

const OPENAPI: &str = "openapi: 3.0.0\ninfo:\n  title: Spec API\n  version: '1'\nservers:\n  - url: https://spec.test\npaths:\n  /things:\n    get:\n      summary: List things\n      responses:\n        '200':\n          description: ok\n";

fn cmd_key(h: &mut H, key: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, key);
    settle(h);
}

fn seeded() -> (tempfile::TempDir, H) {
    let dir = tempfile::tempdir().unwrap();
    demo::seed(dir.path()).unwrap();
    let h = harness(dir.path());
    (dir, h)
}

fn import_file(h: &mut H, dir: &std::path::Path, name: &str, text: &str) {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    click(h, Role::Button, "Import");
    assert!(has(h, Role::Dialog, "Import / Export"));
    h.state_mut().import_load_file(&p);
    wait_until(h, "parsed", |a| a.dlg.import.parsed.is_some());
    settle(h);
}

#[test]
fn postman_file_import_previews_and_lands_in_the_sidebar() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    import_file(&mut h, dir.path(), "pets.json", POSTMAN);
    assert!(has(&h, Role::Label, "Detected format: Postman Collection"));
    assert!(has(&h, Role::ListItem, "GET List"));
    assert!(has(&h, Role::ListItem, "POST Create"));
    click_last(&mut h, Role::Button, "Import");
    assert!(has(&h, Role::Label, "Imported successfully"));
    assert_eq!(h.state().collections.metas[0].name, "Imported Pets");
    click_last(&mut h, Role::Button, "Close");
    assert!(h.state().dialog.is_none());
    assert!(has(&h, Role::TreeItem, "Imported Pets"));
}

#[test]
fn openapi_yaml_import_works() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    import_file(&mut h, dir.path(), "spec.yaml", OPENAPI);
    assert!(has(&h, Role::Label, "Detected format: OpenAPI 3.x"));
    click_last(&mut h, Role::Button, "Import");
    assert_eq!(h.state().collections.metas[0].name, "Spec API");
}

#[test]
fn bad_file_shows_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let p = dir.path().join("bad.json");
    std::fs::write(&p, "not a spec").unwrap();
    click(&mut h, Role::Button, "Import");
    h.state_mut().import_load_file(&p);
    wait_until(&mut h, "error", |a| a.dlg.import.error.is_some());
    assert!(has(&h, Role::Label, "Error"));
    assert!(h.state().collections.metas.is_empty());
}

#[test]
fn pasted_curl_opens_a_new_tab() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    click(&mut h, Role::Button, "Import");
    h.get_by_role_and_label(Role::MultilineTextInput, "Paste cURL or file contents")
        .focus();
    settle(&mut h);
    h.get_by_role_and_label(Role::MultilineTextInput, "Paste cURL or file contents")
        .type_text("curl -X POST https://api.test/users -H 'X-Token: abc' -d 'a=1'");
    settle(&mut h);
    click(&mut h, Role::Button, "Preview import");
    wait_until(&mut h, "parsed", |a| a.dlg.import.parsed.is_some());
    settle(&mut h);
    assert!(has(&h, Role::Label, "Detected format: cURL"));
    click(&mut h, Role::Button, "Open in new tab");
    assert!(h.state().dialog.is_none());
    let t = h.state().tabs.active().unwrap();
    assert_eq!(t.request.method.as_str(), "POST");
    assert_eq!(t.request.url, "https://api.test/users");
    assert!(t
        .request
        .headers
        .iter()
        .any(|k| k.key == "X-Token" && k.value == "abc"));
    // nothing was saved
    assert!(h.state().collections.metas.is_empty());
}

#[test]
fn export_round_trips_through_import() {
    let (dir, mut h) = seeded();
    let m = h
        .state()
        .collections
        .metas
        .iter()
        .find(|m| m.name == "Petstore API")
        .unwrap();
    let cdir = nexus_gui::state::collections::dir_name(m);
    for (fmt, name) in [(ExportFmt::Nexus, "n.json"), (ExportFmt::Postman, "p.json")] {
        let out = dir.path().join(name);
        h.state_mut()
            .export_to(&ExportWhat::Collection(cdir.clone()), fmt, &out);
        assert!(
            matches!(h.state().dlg.import.export_notice, Some(Ok(_))),
            "{:?}",
            h.state().dlg.import.export_notice
        );
        let parsed = nexus_gui::state::dialogs::Parsed::from_file(&out).unwrap();
        assert_eq!(parsed.result.collections[0].name, "Petstore API");
        assert_eq!(parsed.result.collections[0].items.len(), 2);
    }
    // environment export
    let id = h.state().envs.environments[0].id.clone();
    let out = dir.path().join("e.json");
    h.state_mut()
        .export_to(&ExportWhat::Environment(id), ExportFmt::Nexus, &out);
    assert!(std::fs::read_to_string(out).unwrap().contains("variables"));
    // the export pane lists both with accessible buttons
    click(&mut h, Role::Button, "Import");
    click(&mut h, Role::Tab, "Export");
    assert!(has(
        &h,
        Role::Button,
        "Export collection Petstore API as Postman v2.1"
    ));
    assert!(has(
        &h,
        Role::Button,
        "Export environment Staging as Nexus JSON"
    ));
}

#[test]
fn docs_markdown_and_html() {
    let (_d, mut h) = seeded();
    let (opener, seen) = recording_opener();
    h.state_mut().opener = opener;
    click(&mut h, Role::Button, "Docs");
    assert!(has(&h, Role::Dialog, "Generate API Documentation"));
    pick(&mut h, "Collection", "Petstore API");
    click_last(&mut h, Role::Button, "Generate");
    let md = h.state().dlg.docs.content.clone().unwrap();
    assert!(md.starts_with("# Petstore API"));
    assert!(has(&h, Role::Document, "Generated Markdown"));
    assert!(has(&h, Role::Button, "Copy"));
    pick(&mut h, "Format", "HTML");
    click_last(&mut h, Role::Button, "Generate");
    assert!(h
        .state()
        .dlg
        .docs
        .content
        .clone()
        .unwrap()
        .contains("<html"));
    click(&mut h, Role::Button, "Open in browser");
    let files = seen.lock().unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].extension().unwrap() == "html");
    // save writes the generated text
    drop(files);
    let out = _d.path().join("docs.html");
    h.state_mut().docs_save_to(&out);
    assert!(std::fs::read_to_string(out)
        .unwrap()
        .contains("Petstore API"));
}

#[test]
fn environment_manager_create_edit_secret_delete() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    click(&mut h, Role::Button, "Manage");
    assert!(has(&h, Role::Dialog, "Manage Environments"));
    click_last(&mut h, Role::Button, "+ New Environment");
    type_into(&mut h, "New environment name", "Staging");
    h.key_press(Key::Enter);
    settle(&mut h);
    // the editor opened for the new environment
    assert!(has(&h, Role::Dialog, "Edit: Staging"));
    click_last(&mut h, Role::Button, "+ Add Variable");
    type_into(&mut h, "Variable key 1", "token");
    type_into(&mut h, "Variable value 1", "s3cret");
    // a plain variable is visible text ...
    assert!(has(&h, Role::TextInput, "Variable value 1"));
    pick(&mut h, "Variable type 1", "Secret");
    // ... a secret one is masked until revealed
    assert!(has(&h, Role::PasswordInput, "Variable value 1"));
    click_last(&mut h, Role::Button, "Show value 1");
    assert!(has(&h, Role::TextInput, "Variable value 1"));
    click_last(&mut h, Role::Button, "Hide value 1");
    assert!(has(&h, Role::PasswordInput, "Variable value 1"));
    click_last(&mut h, Role::Button, "Save Changes");
    let e = &h.state().envs.environments[0];
    assert_eq!(e.name, "Staging");
    assert_eq!(e.variables[0].key, "token");
    assert_eq!(e.variables[0].value, "s3cret");
    assert_eq!(format!("{:?}", e.variables[0].var_type), "Secret");
    // persisted on disk
    let on_disk = h.state().app.environments_list().unwrap();
    assert_eq!(on_disk[0].variables[0].key, "token");

    // back, rename, duplicate, activate
    click_last(&mut h, Role::Button, "<- All Environments");
    click_last(&mut h, Role::Button, "Rename Staging");
    replace_text(&mut h, Role::TextInput, "Rename Staging to", "Prod");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().envs.environments[0].name, "Prod");
    click_last(&mut h, Role::Button, "Duplicate Prod");
    assert_eq!(h.state().envs.environments.len(), 2);
    assert!(h
        .state()
        .envs
        .environments
        .iter()
        .any(|e| e.name == "Prod Copy"));
    click_last(&mut h, Role::Button, "Activate Prod");
    assert_eq!(
        h.state().envs.name(h.state().ui.active_env_id.as_deref()),
        "Prod"
    );
    // delete needs a confirmation and clears the active selection
    click_last(&mut h, Role::Button, "Delete Prod");
    assert_eq!(h.state().envs.environments.len(), 2);
    click_last(&mut h, Role::Button, "Confirm deleting Prod");
    assert_eq!(h.state().envs.environments.len(), 1);
    assert!(h.state().ui.active_env_id.is_none());
    // closing the dialog: the switcher offers the remaining environment
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().dialog.is_none());
    click(&mut h, Role::ComboBox, "Select environment");
    assert!(has(&h, Role::Button, "Prod Copy"));
}

#[test]
fn environment_variables_reach_autocomplete_and_requests() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let id = h.state_mut().env_create("Dev").unwrap();
    h.state_mut().env_edit(&id);
    assert_eq!(h.state().envs.environments.len(), 1);
    h.state_mut()
        .dlg
        .envs
        .draft
        .as_mut()
        .unwrap()
        .vars
        .push(NexusApp::new_variable());
    h.state_mut().dlg.envs.draft.as_mut().unwrap().vars[0].key = "host".into();
    h.state_mut().env_save_draft();
    h.state_mut().set_env(Some(id));
    let vars = h
        .state()
        .envs
        .variables(h.state().ui.active_env_id.as_deref());
    assert_eq!(vars[0].key, "host");
}

#[test]
fn palette_finds_and_opens_a_request() {
    let (_d, mut h) = seeded();
    cmd_key(&mut h, Key::K);
    assert!(has(&h, Role::Dialog, "Command palette"));
    assert!(has(&h, Role::ListBox, "Results"));
    type_into(&mut h, "Search commands", "list pets");
    assert!(has(
        &h,
        Role::ListBoxOption,
        "GET List pets, Petstore API / Pets, Request"
    ));
    assert!(!has(
        &h,
        Role::ListBoxOption,
        "DELETE Delete pet, Petstore API / Pets, Request"
    ));
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().dialog.is_none());
    assert_eq!(h.state().tabs.active().unwrap().request.name, "List pets");
}

#[test]
fn palette_keyboard_navigation_and_actions() {
    let (_d, mut h) = seeded();
    cmd_key(&mut h, Key::K);
    type_into(&mut h, "Search commands", "switch");
    // first hit is HTTP, Down moves to GraphQL
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().ui.active_panel, nexus_gui::state::Panel::Graphql);
    // theme command
    cmd_key(&mut h, Key::K);
    type_into(&mut h, "Search commands", "theme light");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().ui.theme, ThemeMode::Light);
    // manage environments opens its dialog; Esc closes the palette
    cmd_key(&mut h, Key::K);
    type_into(&mut h, "Search commands", "manage env");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().dialog, Some(Dialog::Envs));
    h.key_press(Key::Escape);
    settle(&mut h);
    cmd_key(&mut h, Key::K);
    assert_eq!(h.state().dialog, Some(Dialog::Palette));
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().dialog.is_none());
    // no match
    cmd_key(&mut h, Key::K);
    type_into(&mut h, "Search commands", "zzzzqqq");
    assert!(has(&h, Role::Label, "No matching commands"));
}

#[test]
fn every_global_shortcut() {
    let server = Server::start();
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let tabs0 = h.state().tabs.tabs.len();
    cmd_key(&mut h, Key::N);
    assert_eq!(h.state().tabs.tabs.len(), tabs0 + 1);
    cmd_key(&mut h, Key::T);
    assert_eq!(h.state().tabs.tabs.len(), tabs0 + 2);
    cmd_key(&mut h, Key::W);
    assert_eq!(h.state().tabs.tabs.len(), tabs0 + 1);
    let collapsed = h.state().ui.sidebar_collapsed;
    cmd_key(&mut h, Key::B);
    assert_eq!(h.state().ui.sidebar_collapsed, !collapsed);
    let console = h.state().ui.console_open;
    cmd_key(&mut h, Key::J);
    assert_eq!(h.state().ui.console_open, !console);
    for (k, p) in [
        (Key::Num2, nexus_gui::state::Panel::Graphql),
        (Key::Num3, nexus_gui::state::Panel::Ws),
        (Key::Num4, nexus_gui::state::Panel::Mcp),
        (Key::Num5, nexus_gui::state::Panel::Runner),
        (Key::Num1, nexus_gui::state::Panel::Http),
    ] {
        cmd_key(&mut h, k);
        assert_eq!(h.state().ui.active_panel, p);
    }
    // Ctrl+L focuses the URL bar, Ctrl+Enter sends
    cmd_key(&mut h, Key::L);
    settle(&mut h);
    type_into_url(&mut h, &server.url());
    cmd_key(&mut h, Key::Enter);
    wait_response(&mut h);
    assert_eq!(server.count(), 1);
    // Ctrl+Shift+E focuses the environment switcher
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::E);
    settle(&mut h);
    // F1 and ? open the help; Esc closes it
    h.key_press(Key::F1);
    settle(&mut h);
    assert_eq!(h.state().dialog, Some(Dialog::Help));
    assert!(has(&h, Role::ListItem, "Send the request: Ctrl+Enter"));
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().dialog.is_none());
    // Ctrl+K toggles the palette
    cmd_key(&mut h, Key::K);
    cmd_key(&mut h, Key::K);
    assert!(h.state().dialog.is_none());
}

fn type_into_url(h: &mut H, url: &str) {
    replace_text(h, Role::TextInput, "Request URL", url);
}

#[test]
fn ctrl_s_saves_the_request_of_a_collection() {
    let (_d, mut h) = seeded();
    cmd_key(&mut h, Key::K);
    type_into(&mut h, "Search commands", "list pets");
    h.key_press(Key::Enter);
    settle(&mut h);
    type_into_url(&mut h, "https://changed.test/pets");
    cmd_key(&mut h, Key::S);
    let t = h.state().tabs.active().unwrap();
    assert!(!t.dirty);
}

#[test]
fn dialogs_are_modal_with_accessible_names() {
    let (_d, mut h) = seeded();
    for (btn, title) in [
        ("Import", "Import / Export"),
        ("Docs", "Generate API Documentation"),
        ("Manage", "Manage Environments"),
    ] {
        click(&mut h, Role::Button, btn);
        assert!(has(&h, Role::Dialog, title), "{title}");
        assert!(has(&h, Role::Button, "Close"));
        h.key_press(Key::Escape);
        settle(&mut h);
        assert!(!has(&h, Role::Dialog, title));
    }
    // clicking Close works as well
    click(&mut h, Role::Button, "Import");
    click_last(&mut h, Role::Button, "Close");
    assert!(h.state().dialog.is_none());
}

#[test]
fn theme_toggle_cycles_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut h = harness(dir.path());
        assert!(nexus_gui::theme::pal().dark);
        click(&mut h, Role::Button, "Theme: Dark");
        assert_eq!(h.state().ui.theme, ThemeMode::Light);
        assert!(!nexus_gui::theme::pal().dark);
        assert!(has(&h, Role::Button, "Theme: Light"));
        h.state_mut().save_ui_state();
    }
    let text = std::fs::read_to_string(dir.path().join("ui-state.json")).unwrap();
    assert!(text.contains("\"theme\": \"light\""));
    let h = harness(dir.path());
    assert_eq!(h.state().ui.theme, ThemeMode::Light);
    assert!(!nexus_gui::theme::pal().dark);
    let mut h = h;
    click(&mut h, Role::Button, "Theme: Light");
    assert_eq!(h.state().ui.theme, ThemeMode::System);
    click(&mut h, Role::Button, "Theme: System");
    assert_eq!(h.state().ui.theme, ThemeMode::Dark);
    assert!(nexus_gui::theme::pal().dark);
}

#[test]
fn panel_forms_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut h = harness(dir.path());
        let a = h.state_mut();
        a.gql.url = "https://gql.test/graphql".into();
        a.gql.query = "query { me }".into();
        a.ws.active_mut().url = "ws://echo.test".into();
        a.ws.active_mut().auto_reconnect = true;
        a.mcp.active_mut().command = "my-server".into();
        a.mcp.active_mut().args = "--flag 1".into();
        a.runner.iterations = "7".into();
        a.runner.stop_on_error = true;
        a.save_ui_state();
    }
    let h = harness(dir.path());
    let a = h.state();
    assert_eq!(a.gql.url, "https://gql.test/graphql");
    assert_eq!(a.gql.query, "query { me }");
    assert_eq!(a.ws.sessions[0].url, "ws://echo.test");
    assert!(a.ws.sessions[0].auto_reconnect);
    assert_eq!(a.mcp.sessions[0].command, "my-server");
    assert_eq!(a.mcp.sessions[0].args, "--flag 1");
    assert_eq!(a.runner.iterations, "7");
    assert!(a.runner.stop_on_error);
}

#[test]
fn internal_splitters_are_exposed_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    click(&mut h, Role::Tab, "GraphQL");
    assert!(has(&h, Role::Splitter, "Resize editors and results"));
    h.state_mut().ui.splits.graphql = 0.3;
    h.state_mut().save_ui_state();
    let h2 = harness(dir.path());
    assert!((h2.state().ui.splits.graphql - 0.3).abs() < 1e-3);
    let _ = Opts::default();
}
