//! The code editor driven through the accessibility tree: typing, indentation, auto-close,
//! comment toggle, autocomplete popups, find, Format and a 1 MB body.

mod common;

use common::*;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use nexus_core::types::{NexusEnvironment, Variable, VariableType};
use std::time::Instant;

const BODY: &str = "Request body";

fn fresh() -> (tempfile::TempDir, H) {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path());
    (dir, h)
}

fn json_body(h: &mut H) {
    click(h, Role::Tab, "Body");
    click(h, Role::RadioButton, "JSON");
    focus(h, BODY);
}

fn focus(h: &mut H, label: &str) {
    h.get_by_role_and_label(Role::MultilineTextInput, label)
        .focus();
    settle(h);
}

fn key(h: &mut H, _label: &str, key: Key) {
    h.key_press(key);
    settle(h);
}

fn key_mod(h: &mut H, _label: &str, m: Modifiers, key: Key) {
    h.key_press_modifiers(m, key);
    settle(h);
}

fn body(h: &H) -> String {
    h.state()
        .tabs
        .active()
        .unwrap()
        .request
        .body
        .raw
        .clone()
        .unwrap_or_default()
}

fn pre_script(h: &H) -> String {
    h.state()
        .tabs
        .active()
        .unwrap()
        .request
        .pre_request_script
        .clone()
        .unwrap_or_default()
}

fn typed(h: &mut H, label: &str, s: &str) {
    for c in s.chars() {
        let mut b = [0; 4];
        let part: &str = c.encode_utf8(&mut b);
        if c == '\n' {
            key(h, label, Key::Enter);
        } else {
            h.get_by_role_and_label(Role::MultilineTextInput, label)
                .type_text(part);
            settle(h);
        }
    }
}

fn var(k: &str, v: &str) -> Variable {
    Variable {
        key: k.into(),
        value: v.into(),
        var_type: VariableType::String,
        enabled: true,
        description: None,
    }
}

#[test]
fn tab_indents_and_shift_tab_outdents() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    typed(&mut h, BODY, "a");
    key(&mut h, BODY, Key::Tab);
    assert_eq!(body(&h), "a  ");
    // shift+tab outdents the line's leading blanks
    key(&mut h, BODY, Key::Home);
    key(&mut h, BODY, Key::Tab);
    assert_eq!(body(&h), "  a  ");
    key_mod(&mut h, BODY, Modifiers::SHIFT, Key::Tab);
    assert_eq!(body(&h), "a  ");
    key(&mut h, BODY, Key::Delete);
    key(&mut h, BODY, Key::End);
    key(&mut h, BODY, Key::Backspace);
    key(&mut h, BODY, Key::Backspace);
    assert_eq!(body(&h), "");
    typed(&mut h, BODY, "a");
    // block indent over a multi-line selection
    typed(&mut h, BODY, "\nb");
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::A);
    key(&mut h, BODY, Key::Tab);
    assert_eq!(body(&h), "  a\n  b");
    key_mod(&mut h, BODY, Modifiers::SHIFT, Key::Tab);
    assert_eq!(body(&h), "a\nb");
}

#[test]
fn tab_does_not_move_focus_until_escaped() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    key(&mut h, BODY, Key::Tab);
    assert!(h
        .get_by_role_and_label(Role::MultilineTextInput, BODY)
        .is_focused());
    // Escape, then Tab: focus leaves the editor and nothing is inserted
    key(&mut h, BODY, Key::Escape);
    key(&mut h, BODY, Key::Tab);
    settle(&mut h);
    assert!(!h
        .get_by_role_and_label(Role::MultilineTextInput, BODY)
        .is_focused());
    assert_eq!(body(&h), "  ");
    // the focus-trap escape is documented in the accessible description
    let desc = h
        .get_by_role_and_label(Role::MultilineTextInput, BODY)
        .accesskit_node()
        .data()
        .description()
        .unwrap_or_default()
        .to_string();
    assert!(
        desc.contains("Escape then Tab") && desc.contains("Control+M"),
        "{desc}"
    );
}

#[test]
fn enter_auto_indents_and_brackets_close() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    typed(&mut h, BODY, "{");
    assert_eq!(body(&h), "{}");
    key(&mut h, BODY, Key::Enter);
    assert_eq!(body(&h), "{\n  \n}");
    typed(&mut h, BODY, "\"a\": [");
    assert_eq!(body(&h), "{\n  \"a\": []\n}");
    typed(&mut h, BODY, "1]");
    assert_eq!(body(&h), "{\n  \"a\": [1]\n}");
    // Enter keeps the indentation of the current line
    key(&mut h, BODY, Key::End);
    key(&mut h, BODY, Key::Enter);
    assert!(
        body(&h).starts_with("{\n  \"a\": [1]\n  \n}"),
        "{:?}",
        body(&h)
    );
}

#[test]
fn undo_and_redo() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    typed(&mut h, BODY, "hello");
    key(&mut h, BODY, Key::Enter);
    typed(&mut h, BODY, "x");
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::Z);
    assert_eq!(body(&h), "hello\n");
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::Z);
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::Z);
    assert_eq!(body(&h), "");
    key_mod(&mut h, BODY, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(body(&h), "hello");
}

#[test]
fn ctrl_slash_toggles_comments_in_scripts() {
    let (_d, mut h) = fresh();
    click(&mut h, Role::Tab, "Scripts");
    let label = "Pre-request script";
    focus(&mut h, label);
    typed(&mut h, label, "a();");
    key_mod(&mut h, label, Modifiers::COMMAND, Key::Slash);
    assert_eq!(pre_script(&h), "// a();");
    key_mod(&mut h, label, Modifiers::COMMAND, Key::Slash);
    assert_eq!(pre_script(&h), "a();");
}

#[test]
fn brace_brace_offers_variables_and_inserts_the_chosen_one() {
    let (_d, mut h) = fresh();
    h.state_mut().envs.environments.push(NexusEnvironment {
        id: "e1".into(),
        name: "Dev".into(),
        variables: vec![var("baseUrl", "http://x"), var("token", "abc")],
    });
    h.state_mut().ui.active_env_id = Some("e1".into());
    settle(&mut h);
    json_body(&mut h);
    typed(&mut h, BODY, "{{");
    // list box with env variables first, then the dynamic ones
    assert!(has(&h, Role::ListBox, "Suggestions"));
    assert!(has(
        &h,
        Role::ListBoxOption,
        "baseUrl, environment - http://x"
    ));
    assert!(has(&h, Role::ListBoxOption, "token, environment - abc"));
    assert!(h
        .query_by_role_and_label(Role::ListBoxOption, "$guid, dynamic")
        .is_some());
    // the editor points at the highlighted option
    let ed = h.get_by_role_and_label(Role::MultilineTextInput, BODY);
    assert!(ed.accesskit_node().data().active_descendant().is_some());
    // filter, move to the second match, accept with Enter
    typed(&mut h, BODY, "t");
    key(&mut h, BODY, Key::Enter);
    assert_eq!(body(&h), "{{token}}");
    assert!(!has(&h, Role::ListBox, "Suggestions"));
}

#[test]
fn variable_completion_by_clicking_an_option_and_escape_closes() {
    let (_d, mut h) = fresh();
    h.state_mut().envs.environments.push(NexusEnvironment {
        id: "e1".into(),
        name: "Dev".into(),
        variables: vec![var("baseUrl", "http://x")],
    });
    h.state_mut().ui.active_env_id = Some("e1".into());
    json_body(&mut h);
    typed(&mut h, BODY, "{{ba");
    h.get_by_role_and_label(Role::ListBoxOption, "baseUrl, environment - http://x")
        .click();
    settle(&mut h);
    assert_eq!(body(&h), "{{baseUrl}}");
    // Escape just closes the popup
    typed(&mut h, BODY, "{{");
    assert!(has(&h, Role::ListBox, "Suggestions"));
    key(&mut h, BODY, Key::Escape);
    assert!(!has(&h, Role::ListBox, "Suggestions"));
}

#[test]
fn nx_members_complete_in_scripts_only() {
    let (_d, mut h) = fresh();
    click(&mut h, Role::Tab, "Scripts");
    let label = "Pre-request script";
    focus(&mut h, label);
    typed(&mut h, label, "nx.environment.se");
    assert!(has(
        &h,
        Role::ListBoxOption,
        "nx.environment.set, Set environment variable"
    ));
    key(&mut h, label, Key::Enter);
    assert_eq!(pre_script(&h), "nx.environment.set");
    // arrow keys pick another member
    typed(&mut h, label, "\nnx.environment.");
    key(&mut h, label, Key::ArrowDown);
    key(&mut h, label, Key::Enter);
    assert_eq!(pre_script(&h), "nx.environment.set\nnx.environment.set");
    // a template with the caret inside the quotes
    typed(&mut h, label, "\nnx.te");
    key(&mut h, label, Key::Tab);
    assert!(
        pre_script(&h).ends_with("nx.test(\"\", () => {\n  \n});"),
        "{:?}",
        pre_script(&h)
    );
    typed(&mut h, label, "ok");
    assert!(pre_script(&h).contains("nx.test(\"ok\""));
    // not in a JSON body
    click(&mut h, Role::Tab, "Body");
    click(&mut h, Role::RadioButton, "JSON");
    focus(&mut h, BODY);
    typed(&mut h, BODY, "nx.env");
    assert!(!has(&h, Role::ListBox, "Suggestions"));
}

#[test]
fn expect_chain_completes() {
    let (_d, mut h) = fresh();
    click(&mut h, Role::Tab, "Scripts");
    let label = "Post-response script";
    click(&mut h, Role::Tab, "Post-response");
    focus(&mut h, label);
    typed(&mut h, label, "nx.expect(1).to.equ");
    key(&mut h, label, Key::Enter);
    let s = h
        .state()
        .tabs
        .active()
        .unwrap()
        .request
        .post_response_script
        .clone()
        .unwrap();
    assert_eq!(s, "nx.expect(1).to.equal()");
}

#[test]
fn find_in_editor_counts_and_steps() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    h.state_mut().tabs.active_mut().unwrap().request.body.raw =
        Some("alpha\nbeta alpha\nALPHA".into());
    settle(&mut h);
    focus(&mut h, BODY);
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::F);
    settle(&mut h);
    assert!(has(&h, Role::TextInput, "Find in editor"));
    type_into(&mut h, "Find in editor", "alpha");
    assert!(has(&h, Role::Label, "Editor matches 1 / 3"), "count label");
    click(&mut h, Role::Button, "Next match in editor");
    assert!(has(&h, Role::Label, "Editor matches 2 / 3"));
    click(&mut h, Role::Button, "Previous match in editor");
    click(&mut h, Role::Button, "Previous match in editor");
    assert!(has(&h, Role::Label, "Editor matches 3 / 3"));
    // the match is selected in the editor: typing replaces it
    focus(&mut h, BODY);
    typed(&mut h, BODY, "Z");
    assert_eq!(body(&h), "alpha\nbeta alpha\nZ");
    // closing the bar
    click(&mut h, Role::Button, "Close find in editor");
    assert!(!has(&h, Role::TextInput, "Find in editor"));
}

#[test]
fn format_json_button_pretty_prints() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    h.state_mut().tabs.active_mut().unwrap().request.body.raw =
        Some("{\"a\":[1,2],\"b\":{}}".into());
    settle(&mut h);
    click(&mut h, Role::Button, "Format JSON");
    settle(&mut h);
    assert_eq!(
        body(&h),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {}\n}"
    );
    // invalid JSON is left alone
    h.state_mut().tabs.active_mut().unwrap().request.body.raw = Some("{oops".into());
    settle(&mut h);
    click(&mut h, Role::Button, "Format JSON");
    assert_eq!(body(&h), "{oops");
}

#[test]
fn one_megabyte_body_stays_responsive() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    let item =
        "  {\"id\": 1, \"name\": \"User number\", \"tags\": [\"a\", \"b\"], \"ok\": true},\n";
    let mut big = String::from("[\n");
    while big.len() < 1_000_000 {
        big.push_str(item);
    }
    big.push_str("  null\n]");
    h.state_mut().tabs.active_mut().unwrap().request.body.raw = Some(big.clone());
    settle(&mut h);
    // steady frames
    let t0 = Instant::now();
    for _ in 0..20 {
        h.step();
    }
    let steady = t0.elapsed() / 20;
    // keystrokes in the middle of the file
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::Home);
    key_mod(&mut h, BODY, Modifiers::COMMAND, Key::End);
    let t1 = Instant::now();
    h.get_by_role_and_label(Role::MultilineTextInput, BODY)
        .type_text("x");
    h.step();
    let typing = t1.elapsed();
    eprintln!("1 MB body (debug build): steady frame {steady:?}, keystroke frame {typing:?}");
    assert!(body(&h).len() > big.len(), "typed into the big body");
    // generous bounds for an unoptimised test build
    assert!(steady.as_millis() < 60, "steady frame {steady:?}");
    assert!(typing.as_millis() < 400, "typing {typing:?}");
}

#[test]
fn editors_expose_label_value_and_hide_the_gutter() {
    let (_d, mut h) = fresh();
    json_body(&mut h);
    typed(&mut h, BODY, "{\"k\":1}");
    let v = h
        .get_by_role_and_label(Role::MultilineTextInput, BODY)
        .value()
        .unwrap();
    assert_eq!(v, "{\"k\":1}");
    // line numbers are painted, not exposed
    assert!(h.query_all_by_label("1").next().is_none());
}
