use serde_json::json;

use super::*;

fn viz(template: &str, data: Value) -> Visualizer {
    let mut v = Visualizer::new();
    v.set(template, data);
    v
}

fn render(template: &str, data: Value) -> String {
    viz(template, data).render().unwrap()
}

#[test]
fn starts_with_no_config() {
    assert!(Visualizer::new().config().is_none());
}

#[test]
fn stores_template_and_data() {
    let v = viz("<h1>{{title}}</h1>", json!({"title": "Hello"}));
    let cfg = v.config().unwrap();
    assert_eq!(cfg.template, "<h1>{{title}}</h1>");
    assert_eq!(cfg.data, json!({"title": "Hello"}));
}

#[test]
fn second_set_overwrites() {
    let mut v = viz("first", json!({"a": 1}));
    v.set("second", json!({"b": 2}));
    assert_eq!(v.config().unwrap().template, "second");
}

#[test]
fn clear_removes_config() {
    let mut v = viz("tmpl", json!({}));
    v.clear();
    assert!(v.config().is_none());
    assert!(v.render().is_none());
}

#[test]
fn render_is_none_without_config() {
    assert!(Visualizer::new().render().is_none());
}

#[test]
fn interpolates_and_escapes() {
    assert_eq!(
        render("<p>{{name}}</p>", json!({"name": "Alice & Bob"})),
        "<p>Alice &amp; Bob</p>"
    );
}

#[test]
fn triple_braces_are_raw() {
    assert_eq!(
        render("<div>{{{html}}}</div>", json!({"html": "<b>bold</b>"})),
        "<div><b>bold</b></div>"
    );
}

#[test]
fn escapes_angle_brackets() {
    assert_eq!(
        render("{{val}}", json!({"val": "<script>alert(1)</script>"})),
        "&lt;script&gt;alert(1)&lt;/script&gt;"
    );
}

#[test]
fn escapes_quotes() {
    assert_eq!(
        render("attr=\"{{val}}\"", json!({"val": "say \"hello\""})),
        "attr=\"say &quot;hello&quot;\""
    );
}

#[test]
fn missing_and_null_values_render_empty() {
    assert_eq!(render("Hello {{missing}}!", json!({})), "Hello !");
    assert_eq!(render("{{a}}{{b}}", json!({"a": null})), "");
}

#[test]
fn numbers_and_bools_stringify_like_js() {
    assert_eq!(
        render(
            "{{n}} {{f}} {{b}} {{x}}",
            json!({"n": 3, "f": 2.5, "b": true, "x": 1.0})
        ),
        "3 2.5 true 1"
    );
}

#[test]
fn each_over_objects() {
    let out = render(
        "{{#each items}}<li>{{name}}</li>{{/each}}",
        json!({"items": [{"name": "A"}, {"name": "B"}]}),
    );
    assert_eq!(out, "<li>A</li><li>B</li>");
}

#[test]
fn each_provides_index() {
    assert_eq!(
        render(
            "{{#each items}}{{@index}}{{/each}}",
            json!({"items": ["a", "b", "c"]})
        ),
        "012"
    );
}

#[test]
fn each_primitives_via_this() {
    assert_eq!(
        render(
            "{{#each tags}}{{this}},{{/each}}",
            json!({"tags": ["x", "y", "z"]})
        ),
        "x,y,z,"
    );
}

#[test]
fn each_over_non_array_missing_or_empty_renders_nothing() {
    assert_eq!(
        render("{{#each notArr}}item{{/each}}", json!({"notArr": "string"})),
        ""
    );
    assert_eq!(render("{{#each missing}}item{{/each}}", json!({})), "");
    assert_eq!(
        render("{{#each items}}item{{/each}}", json!({"items": []})),
        ""
    );
}

#[test]
fn if_blocks() {
    assert_eq!(
        render("{{#if show}}visible{{/if}}", json!({"show": true})),
        "visible"
    );
    assert_eq!(
        render("{{#if show}}visible{{/if}}", json!({"show": false})),
        ""
    );
    assert_eq!(
        render("{{#if items}}has items{{/if}}", json!({"items": [1]})),
        "has items"
    );
    assert_eq!(
        render("{{#if items}}has items{{/if}}", json!({"items": []})),
        ""
    );
    assert_eq!(render("{{#if missing}}visible{{/if}}", json!({})), "");
}

#[test]
fn unless_blocks() {
    assert_eq!(
        render(
            "{{#unless hidden}}shown{{/unless}}",
            json!({"hidden": false})
        ),
        "shown"
    );
    assert_eq!(
        render(
            "{{#unless hidden}}shown{{/unless}}",
            json!({"hidden": true})
        ),
        ""
    );
}

#[test]
fn dotted_paths() {
    assert_eq!(
        render("{{user.name}}", json!({"user": {"name": "Eve"}})),
        "Eve"
    );
    assert_eq!(render("{{a.b.c}}", json!({"a": {"b": null}})), "");
}

#[test]
fn wrapper_none_without_config() {
    assert!(Visualizer::new().render_with_wrapper().is_none());
}

#[test]
fn wrapper_produces_full_document() {
    let html = viz("<p>Hello</p>", json!({}))
        .render_with_wrapper()
        .unwrap();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<body><p>Hello</p></body>"));
    assert!(html.contains("</html>"));
}

#[test]
fn wrapper_has_dark_theme_css() {
    let html = viz("test", json!({})).render_with_wrapper().unwrap();
    assert!(html.contains("background: #0d1117"));
}

#[test]
fn wrapper_renders_template() {
    let html = viz("<span>{{name}}</span>", json!({"name": "Test"}))
        .render_with_wrapper()
        .unwrap();
    assert!(html.contains("<span>Test</span>"));
}

#[test]
fn each_and_if_compose() {
    let out = render(
        "{{#each rows}}{{#if active}}{{name}}|{{/if}}{{/each}}",
        json!({"rows": [{"name": "A", "active": true}, {"name": "B", "active": false}, {"name": "C", "active": true}]}),
    );
    assert_eq!(out, "A|C|");
}

#[test]
fn static_text_is_unchanged() {
    assert_eq!(
        render("<h1>Static Content</h1>", json!({})),
        "<h1>Static Content</h1>"
    );
}

#[test]
fn substituted_data_is_never_rescanned_as_template() {
    let data = json!({
        "payload": "<script>alert(1)</script>",
        "name": "{{{payload}}}",
        "items": [{ "name": "{{{payload}}}{{#if payload}}X{{/if}}" }],
        "flag": true
    });
    for tmpl in [
        "{{name}}",
        "{{#each items}}{{name}}{{/each}}",
        "{{#if flag}}{{name}}{{/if}}",
        "{{#unless nope}}{{name}}{{/unless}}",
    ] {
        let out = render(tmpl, data.clone());
        assert!(!out.contains("<script>"), "{tmpl} -> {out}");
        assert!(out.contains("{{{payload}}}"), "{tmpl} -> {out}");
    }
    assert_eq!(
        render("{{payload}}", data),
        "&lt;script&gt;alert(1)&lt;/script&gt;"
    );
}
