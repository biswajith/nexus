//! The response viewer and the console, driven through role + label queries against a local
//! mock server.

mod common;

use common::*;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use std::time::{Duration, Instant};
use wiremock::ResponseTemplate;

fn json(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), "application/json")
}

fn setup(routes: Vec<(&str, ResponseTemplate)>) -> (tempfile::TempDir, H, Server) {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path());
    (dir, h, Server::routes(routes))
}

fn get(h: &mut H, url: &str) {
    type_into(h, "Request URL", url);
    send(h);
}

fn label_has(h: &H, text: &str) -> bool {
    h.query_by_label_contains(text).is_some()
}

#[test]
fn json_response_is_pretty_printed_with_status_and_size() {
    let body = r#"{"a":1,"b":[true,null],"name":"Ada"}"#;
    let (_d, mut h, s) = setup(vec![("/json", json(body))]);
    get(&mut h, &format!("{}/json", s.url()));

    assert!(has(&h, Role::Label, "Response status: 200 OK"));
    assert!(has(
        &h,
        Role::Label,
        &format!("Response size: {} B", body.len())
    ));
    assert!(h.query_by_label_contains("Response time: ").is_some());
    // pretty view: one line per token, 2-space indent, shown by line
    assert!(has(&h, Role::Document, "Response body"));
    assert!(has(&h, Role::Label, "  \"a\": 1,"));
    assert!(has(&h, Role::Label, "    true,"));
    assert!(!label_has(&h, "\"a\":1"));

    // raw view shows the body as received
    click_last(&mut h, Role::Tab, "Raw");
    assert!(has(&h, Role::Document, "Response body (raw)"));
    assert!(has(&h, Role::Label, body));
}

#[test]
fn headers_tab_lists_headers_sorted() {
    let (_d, mut h, s) = setup(vec![(
        "/h",
        json("{}")
            .insert_header("x-custom", "hello")
            .insert_header("x-alpha", "1, 2"),
    )]);
    get(&mut h, &format!("{}/h", s.url()));
    let n = h
        .state()
        .tabs
        .active()
        .unwrap()
        .response
        .as_ref()
        .unwrap()
        .headers
        .len();
    let label = format!("Headers ({n})");
    click_last(&mut h, Role::Tab, &label);
    assert!(has(&h, Role::Row, "x-custom: hello"));
    assert!(has(&h, Role::Row, "x-alpha: 1, 2"));
    assert!(has(&h, Role::Row, "content-type: application/json"));
    assert!(has(&h, Role::Button, "Copy headers"));
    let names: Vec<String> = h
        .state()
        .tabs
        .active()
        .unwrap()
        .response
        .as_ref()
        .unwrap()
        .headers
        .iter()
        .map(|(k, _)| k.clone())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

const SCRIPT: &str = "console.log('hello', 42); console.warn('careful'); console.error('broken'); \
nx.test('passes', () => { nx.expect(nx.response.code).to.equal(200); }); \
nx.test('fails', () => { nx.expect(nx.response.code).to.equal(500); });";

fn with_post_script(h: &mut H) {
    click(h, Role::Tab, "Scripts");
    click(h, Role::Tab, "Post-response");
    type_into_editor(h, "Post-response script", SCRIPT);
}

#[test]
fn passing_and_failing_nx_tests_appear_in_the_tests_tab() {
    let (_d, mut h, s) = setup(vec![("/t", json("{}"))]);
    type_into(&mut h, "Request URL", &format!("{}/t", s.url()));
    with_post_script(&mut h);
    send(&mut h);

    click_last(&mut h, Role::Tab, "Tests (1/2)");
    assert!(has(&h, Role::Label, "Tests 1 passed"));
    assert!(has(&h, Role::Label, "Tests 1 failed"));
    assert!(has(&h, Role::Label, "Tests 2 total"));
    assert!(has(&h, Role::ListItem, "Test passed: passes"));
    assert!(
        label_has(&h, "Test failed: fails: expected"),
        "the failing test shows its error"
    );
}

#[test]
fn script_console_output_is_filterable_in_the_console_panel() {
    let (_d, mut h, s) = setup(vec![("/c", json("{}"))]);
    type_into(&mut h, "Request URL", &format!("{}/c", s.url()));
    with_post_script(&mut h);
    send(&mut h);

    // closed by default
    assert!(!has(&h, Role::Button, "Clear console"));
    click(&mut h, Role::Button, "Console");
    assert!(has(&h, Role::ListItem, "INFO Script: hello 42"));
    assert!(has(&h, Role::ListItem, "WARN Script: careful"));
    assert!(has(&h, Role::ListItem, "ERR Script: broken"));
    // the request and the response are logged as HTTP entries
    assert!(h.query_by_label_contains("INFO HTTP: -> GET ").is_some());
    assert!(h.query_by_label_contains("INFO HTTP: <- 200 OK").is_some());

    // level chip
    click(&mut h, Role::Button, "ERR");
    assert!(has(&h, Role::ListItem, "ERR Script: broken"));
    assert!(!has(&h, Role::ListItem, "WARN Script: careful"));
    click(&mut h, Role::Button, "ERR"); // toggles off
    assert!(has(&h, Role::ListItem, "WARN Script: careful"));

    // text filter
    type_into(&mut h, "Filter console", "CAREF");
    assert!(has(&h, Role::ListItem, "WARN Script: careful"));
    assert!(!has(&h, Role::ListItem, "ERR Script: broken"));
    type_into(&mut h, "Filter console", "zzz");
    assert!(has(&h, Role::Label, "No entries match the filter"));

    // clear
    click(&mut h, Role::Button, "Clear console");
    assert!(h.state().console.is_empty());
}

#[test]
fn console_state_persists_across_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    click(&mut h, Role::Button, "Console");
    assert!(h.state().ui.console_open);
    h.state_mut().save_ui_state();
    drop(h);
    let h = harness(dir.path());
    assert!(h.state().ui.console_open);
    assert!(has(&h, Role::Button, "Clear console"));
}

#[test]
fn search_counts_matches_and_steps_through_them() {
    let (_d, mut h, s) = setup(vec![("/s", json(r#"{"name":"banana","other":"bandana"}"#))]);
    get(&mut h, &format!("{}/s", s.url()));
    h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    settle(&mut h);
    assert!(has(&h, Role::TextInput, "Search in response body"));
    type_into(&mut h, "Search in response body", "AN");
    // banana: 2, bandana: 2 (case-insensitive)
    assert!(has(&h, Role::Label, "Search matches 1 / 4"));
    click(&mut h, Role::Button, "Next match");
    assert!(has(&h, Role::Label, "Search matches 2 / 4"));
    click(&mut h, Role::Button, "Previous match");
    click(&mut h, Role::Button, "Previous match");
    assert!(has(&h, Role::Label, "Search matches 4 / 4"));
    type_into(&mut h, "Search in response body", "q");
    assert!(has(&h, Role::Label, "Search matches No results"));
    click(&mut h, Role::Button, "Close search");
    assert!(!has(&h, Role::TextInput, "Search in response body"));
}

#[test]
fn html_preview_goes_to_a_private_temp_file_and_the_opener() {
    let html = "<!doctype html><h1>Hello</h1><script>alert(1)</script>";
    let (dir, mut h, s) = setup(vec![(
        "/p",
        ResponseTemplate::new(200)
            .set_body_raw(html.as_bytes().to_vec(), "text/html; charset=utf-8"),
    )]);
    let (opener, seen) = recording_opener();
    h.state_mut().opener = opener;
    get(&mut h, &format!("{}/p", s.url()));
    // html is shown as source with highlighting first
    assert!(h.query_by_label_contains("<h1>Hello</h1>").is_some());
    click_last(&mut h, Role::Tab, "Preview");
    assert!(seen.lock().unwrap().is_empty(), "nothing opens by itself");
    click(&mut h, Role::Button, "Open preview in browser");

    let opened = seen.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    let path = &opened[0];
    assert!(path.starts_with(dir.path().join("tmp")));
    assert_eq!(path.extension().unwrap(), "html");
    let written = std::fs::read_to_string(path).unwrap();
    assert!(written.contains("<h1>Hello</h1>"));
    assert!(
        written.contains("script-src 'none'"),
        "scripts are blocked in the preview"
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(has(&h, Role::Label, "Opened in your browser"));
}

#[test]
fn visualizer_output_opens_in_the_browser() {
    let (dir, mut h, s) = setup(vec![("/v", json(r#"{"name":"Ada"}"#))]);
    let (opener, seen) = recording_opener();
    h.state_mut().opener = opener;
    type_into(&mut h, "Request URL", &format!("{}/v", s.url()));
    click(&mut h, Role::Tab, "Scripts");
    click(&mut h, Role::Tab, "Post-response");
    type_into_editor(
        &mut h,
        "Post-response script",
        "nx.visualizer.set('<h2>Hi {{name}}</h2>', nx.response.json());",
    );
    send(&mut h);
    click_last(&mut h, Role::Tab, "Visualize");
    click(&mut h, Role::Button, "Open visualization in browser");
    let opened = seen.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert!(opened[0].starts_with(dir.path().join("tmp")));
    assert!(std::fs::read_to_string(&opened[0])
        .unwrap()
        .contains("<h2>Hi Ada</h2>"));
}

#[test]
fn image_response_shows_a_summary_and_opens_externally() {
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(4, 3, image::Rgba([200, 30, 30, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let (_d, mut h, s) = setup(vec![(
        "/i",
        ResponseTemplate::new(200).set_body_raw(png.clone(), "image/png"),
    )]);
    let (opener, seen) = recording_opener();
    h.state_mut().opener = opener;
    get(&mut h, &format!("{}/i", s.url()));
    let summary = format!("Image: image/png, {} B", png.len());
    assert!(label_has(&h, &summary));
    assert!(
        h.query_by_label("Response image").is_some(),
        "image is drawn"
    );
    click(&mut h, Role::Button, "Open externally");
    let opened = seen.lock().unwrap().clone();
    assert_eq!(opened[0].extension().unwrap(), "png");
    assert_eq!(std::fs::read(&opened[0]).unwrap(), png);

    // Save writes the exact bytes
    let out = _d.path().join("out.png");
    let id = h.state().tabs.active_id().unwrap().to_string();
    h.state_mut().save_response_to(&id, &out);
    assert_eq!(std::fs::read(&out).unwrap(), png);
}

#[test]
fn saving_a_text_body_writes_the_shown_document() {
    let (d, mut h, s) = setup(vec![("/j", json(r#"{"k":1}"#))]);
    get(&mut h, &format!("{}/j", s.url()));
    let id = h.state().tabs.active_id().unwrap().to_string();
    let out = d.path().join("body.json");
    h.state_mut().save_response_to(&id, &out);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "{\n  \"k\": 1\n}");
    let notice = h
        .state()
        .tabs
        .active()
        .unwrap()
        .response
        .as_ref()
        .unwrap()
        .view
        .notice
        .clone();
    assert_eq!(notice, Some(format!("Saved to {}", out.display())));
}

#[test]
fn refused_connection_shows_the_error_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    // nothing listens on port 1
    type_into(&mut h, "Request URL", "http://127.0.0.1:1/x");
    send(&mut h);
    assert!(has(&h, Role::Label, "Response status: Error"));
    assert!(has(&h, Role::Label, "Error: Could not get a response"));
    assert!(label_has(&h, "Connection refused"));
    // no tab strip for a failed request
    assert!(h.query_by_role_and_label(Role::Tab, "Timing").is_none());
    click(&mut h, Role::Button, "Console");
    assert!(h
        .query_by_label_contains("ERR HTTP: <- Could not get a response")
        .is_some());
}

#[test]
fn cancelled_request_is_reported() {
    let s = Server::with_delay(Duration::from_secs(5));
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    type_into(&mut h, "Request URL", &s.url());
    click(&mut h, Role::Button, "Send");
    click(&mut h, Role::Button, "Cancel");
    assert!(has(&h, Role::Label, "Response status: Cancelled"));
    assert!(has(&h, Role::Label, "Error: Request cancelled"));
    assert!(has(&h, Role::Label, "The request was cancelled."));
}

#[test]
fn timing_tab_shows_every_phase() {
    let (_d, mut h, s) = setup(vec![("/t", json("{}"))]);
    get(&mut h, &format!("{}/t", s.url()));
    click_last(&mut h, Role::Tab, "Timing");
    for phase in ["DNS", "TCP", "TLS", "TTFB", "Download"] {
        assert!(
            h.query_by_label_contains(&format!("{phase}: ")).is_some(),
            "{phase}"
        );
    }
    assert!(h.query_by_label_contains("Total: ").is_some());
}

#[test]
fn big_body_renders_without_blocking() {
    // ~10 MB of JSON
    let items: Vec<String> = (0..125_000)
        .map(|i| {
            format!(
                r#"{{"id":{i},"name":"item-{i}","tags":["alpha","beta"],"price":{}.5,"ok":true}}"#,
                i % 97
            )
        })
        .collect();
    let body = format!("{{\"items\":[{}]}}", items.join(","));
    assert!(body.len() > 8_000_000, "{}", body.len());
    let (_d, mut h, s) = setup(vec![("/big", json(&body))]);
    type_into(&mut h, "Request URL", &format!("{}/big", s.url()));
    let t0 = Instant::now();
    click(&mut h, Role::Button, "Send");
    wait_response(&mut h);
    eprintln!("send + convert + first frames: {:?}", t0.elapsed());

    // first frame highlights the visible lines; later frames come from the cache
    h.run();
    let lines = h
        .state()
        .tabs
        .active()
        .unwrap()
        .response
        .as_ref()
        .unwrap()
        .pretty
        .line_count();
    assert!(lines > 900_000, "{lines} lines");
    let t = Instant::now();
    let n = 20;
    for _ in 0..n {
        h.step();
    }
    let per = t.elapsed() / n;
    eprintln!("{lines} lines: {per:?} per steady frame");
    assert!(per < Duration::from_millis(50), "steady frame took {per:?}");

    // the search jumps to a line far down; the viewer shows it
    h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    settle(&mut h);
    let t = Instant::now();
    type_into(&mut h, "Search in response body", "item-124999");
    eprintln!("search + jump over 10 MB: {:?}", t.elapsed());
    assert!(has(&h, Role::Label, "Search matches 1 / 1"));
    h.run();
    assert!(
        h.query_by_label_contains("\"name\": \"item-124999\"")
            .is_some(),
        "the matched line is on screen"
    );
    let t = Instant::now();
    for _ in 0..n {
        h.step();
    }
    let per = t.elapsed() / n;
    eprintln!("steady frame after the jump: {per:?}");
    assert!(
        per < Duration::from_millis(50),
        "frame after jump took {per:?}"
    );
    let _ = egui::Vec2::ZERO;
}

// ---- stage-2 follow-up: splitter + scrolling sub-tabs ----

#[test]
fn splitter_drags_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let before = h.state().ui.split;
    let r = h
        .get_by_role_and_label(Role::Splitter, "Resize request and response panes")
        .rect();
    let c = r.center();
    h.hover_at(c);
    h.run();
    h.drag_at(c);
    h.run();
    h.hover_at(c + egui::vec2(0.0, 120.0));
    h.run();
    h.drop_at(c + egui::vec2(0.0, 120.0));
    h.run();
    let after = h.state().ui.split;
    assert!(after > before + 0.05, "split {before} -> {after}");
    h.state_mut().save_ui_state();
    drop(h);
    let h = harness(dir.path());
    assert!((h.state().ui.split - after).abs() < 0.01);
}

#[test]
fn request_sub_tabs_scroll_so_every_field_is_reachable() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    click(&mut h, Role::Tab, "Auth");
    pick(&mut h, "Auth type", "OAuth 2.0");
    // the smallest request pane the splitter allows
    h.state_mut().ui.split = 0.0;
    settle(&mut h);
    let split_top = h
        .get_by_role_and_label(Role::Splitter, "Resize request and response panes")
        .rect()
        .top();
    let reachable = |h: &H| {
        let r = h.get_by_role_and_label(Role::TextInput, "Scope").rect();
        r.top() >= 0.0 && r.bottom() <= split_top
    };
    assert!(!reachable(&h), "Scope starts out of view");
    // wheel over the form until the last field shows
    let over = h
        .get_by_role_and_label(Role::TextInput, "Token Type")
        .rect()
        .center();
    for _ in 0..12 {
        h.hover_at(over);
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        settle(&mut h);
        if reachable(&h) {
            break;
        }
    }
    assert!(
        reachable(&h),
        "scrolling the Auth form brings Scope into view"
    );
}

// ---- accessibility inventory ----

#[test]
fn every_response_and_console_control_has_a_role_and_label() {
    let (_d, mut h, s) = setup(vec![("/a", json(r#"{"a":1}"#))]);
    type_into(&mut h, "Request URL", &format!("{}/a", s.url()));
    with_post_script(&mut h);
    send(&mut h);
    click(&mut h, Role::Button, "Console");
    h.key_press_modifiers(Modifiers::COMMAND, Key::F);
    settle(&mut h);
    for (role, label) in [
        (Role::Label, "Response status: 200 OK"),
        (Role::Tab, "Pretty"),
        (Role::Tab, "Raw"),
        (Role::Tab, "Preview"),
        (Role::Button, "Copy response body"),
        (Role::Button, "Save response body"),
        (Role::Button, "Search response body"),
        (Role::TextInput, "Search in response body"),
        (Role::Button, "Previous match"),
        (Role::Button, "Next match"),
        (Role::Button, "Close search"),
        (Role::Document, "Response body"),
        (Role::Button, "Console"),
        (Role::Button, "INFO"),
        (Role::Button, "WARN"),
        (Role::Button, "ERR"),
        (Role::Button, "DBG"),
        (Role::TextInput, "Filter console"),
        (Role::CheckBox, "Auto-scroll"),
        (Role::Button, "Copy console entries"),
        (Role::Button, "Clear console"),
    ] {
        assert!(has(&h, role, label), "{role:?} {label}");
    }
    // nothing interactive is anonymous
    use egui_kittest::kittest::NodeT;
    for n in h.get_all_by(|_| true) {
        let d = n.accesskit_node();
        let interactive = matches!(
            d.role(),
            Role::Button
                | Role::Tab
                | Role::CheckBox
                | Role::TextInput
                | Role::MultilineTextInput
                | Role::ComboBox
                | Role::RadioButton
        );
        if interactive {
            assert!(
                d.label().is_some_and(|l| !l.is_empty())
                    || d.value().is_some_and(|v| !v.is_empty()),
                "unnamed {:?}",
                d.role()
            );
        }
    }
}
