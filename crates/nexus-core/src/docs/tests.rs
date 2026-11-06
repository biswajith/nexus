use super::*;
use crate::types::*;

fn request() -> NexusRequest {
    NexusRequest {
        name: "Get Users".into(),
        url: "https://api.example.com/users".into(),
        ..create_default_request()
    }
}

fn with(f: impl FnOnce(&mut NexusRequest)) -> NexusRequest {
    let mut r = request();
    f(&mut r);
    r
}

fn folder(name: &str, items: Vec<CollectionItem>) -> NexusFolder {
    NexusFolder {
        id: "folder-1".into(),
        name: name.into(),
        description: None,
        items,
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        item_order: None,
    }
}

fn collection(items: Vec<CollectionItem>) -> NexusCollection {
    NexusCollection {
        id: "coll-1".into(),
        name: "My API".into(),
        description: None,
        version: None,
        variables: vec![],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        items,
        item_order: None,
    }
}

fn req(r: NexusRequest) -> CollectionItem {
    CollectionItem::Request(r)
}

fn fld(f: NexusFolder) -> CollectionItem {
    CollectionItem::Folder(f)
}

fn kv(key: &str, value: &str, enabled: bool, description: Option<&str>) -> KeyValuePair {
    KeyValuePair {
        key: key.into(),
        value: value.into(),
        enabled,
        description: description.map(str::to_string),
    }
}

fn base_url_var(value: &str) -> Variable {
    Variable {
        key: "base_url".into(),
        value: value.into(),
        var_type: VariableType::String,
        enabled: true,
        description: None,
    }
}

fn md(c: &NexusCollection) -> String {
    generate_markdown(c).unwrap()
}

fn html(c: &NexusCollection) -> String {
    generate_html(c).unwrap()
}

// ---- markdown ----

#[test]
fn md_title() {
    let c = NexusCollection {
        name: "Pet Store API".into(),
        ..collection(vec![])
    };
    assert!(md(&c).contains("# Pet Store API"));
}

#[test]
fn md_description() {
    let c = NexusCollection {
        description: Some("A sample API for pets".into()),
        ..collection(vec![])
    };
    assert!(md(&c).contains("A sample API for pets"));
}

#[test]
fn md_base_url() {
    let c = NexusCollection {
        variables: vec![base_url_var("https://api.pets.com")],
        ..collection(vec![])
    };
    assert!(md(&c).contains("**Base URL:** `https://api.pets.com`"));
    assert!(!md(&collection(vec![])).contains("Base URL"));
}

#[test]
fn md_groups_top_level_requests() {
    let c = collection(vec![req(NexusRequest {
        name: "Health Check".into(),
        ..request()
    })]);
    let out = md(&c);
    assert!(out.contains("## Requests"));
    assert!(out.contains("### GET Health Check"));
}

#[test]
fn md_groups_folder_items() {
    let mut f = folder(
        "Users",
        vec![req(NexusRequest {
            name: "List Users".into(),
            url: "/users".into(),
            ..request()
        })],
    );
    f.description = Some("User management".into());
    let out = md(&collection(vec![fld(f)]));
    assert!(out.contains("## Users"));
    assert!(out.contains("User management"));
    assert!(out.contains("### GET List Users"));
}

#[test]
fn md_table_of_contents() {
    let f = folder(
        "Auth",
        vec![req(NexusRequest {
            name: "Login".into(),
            method: HttpMethod::Post,
            url: "/login".into(),
            ..request()
        })],
    );
    let out = md(&collection(vec![fld(f)]));
    assert!(out.contains("## Table of Contents"));
    assert!(out.contains("- [Auth](#auth)"));
    assert!(
        out.contains("- [POST Login](#post-login)")
            || out.contains("  - [POST Login](#post-login)")
    );
}

#[test]
fn md_headers_table_for_enabled_headers() {
    let r = with(|r| {
        r.headers = vec![
            kv("Authorization", "Bearer token", true, Some("Auth header")),
            kv("X-Disabled", "nope", false, None),
        ]
    });
    let out = md(&collection(vec![req(r)]));
    assert!(out.contains("**Headers:**"));
    assert!(out.contains("`Authorization`"));
    assert!(out.contains("Auth header"));
    assert!(!out.contains("X-Disabled"));
}

#[test]
fn md_query_parameters_table() {
    let r = with(|r| {
        r.params = vec![
            kv("page", "1", true, Some("Page number")),
            kv("skip", "0", false, None),
        ]
    });
    let out = md(&collection(vec![req(r)]));
    assert!(out.contains("**Query Parameters:**"));
    assert!(out.contains("`page`"));
    assert!(out.contains("Page number"));
    assert!(!out.contains("skip"));
}

#[test]
fn md_json_body_example() {
    let r = with(|r| {
        r.method = HttpMethod::Post;
        r.body = RequestBody {
            mode: BodyMode::Json,
            raw: Some(r#"{"name":"Alice"}"#.into()),
            ..Default::default()
        };
    });
    let out = md(&collection(vec![req(r)]));
    assert!(out.contains("**Body** (`json`)"));
    assert!(out.contains(r#"{"name":"Alice"}"#));
}

#[test]
fn md_omits_body_for_mode_none() {
    assert!(!md(&collection(vec![req(request())])).contains("**Body**"));
}

#[test]
fn md_graphql_query_as_body_example() {
    let r = with(|r| {
        r.method = HttpMethod::Post;
        r.body = RequestBody {
            mode: BodyMode::Graphql,
            graphql: Some(GraphqlBody {
                query: "{ users { id } }".into(),
                variables: None,
            }),
            ..Default::default()
        };
    });
    assert!(md(&collection(vec![req(r)])).contains("{ users { id } }"));
}

#[test]
fn md_curl_snippet() {
    let out = md(&collection(vec![req(request())]));
    assert!(out.contains("**cURL:**"));
    assert!(out.contains("curl"));
}

#[test]
fn md_flattens_nested_folders() {
    let inner = folder(
        "Nested",
        vec![req(NexusRequest {
            name: "Deep Endpoint".into(),
            url: "/deep".into(),
            ..request()
        })],
    );
    let out = md(&collection(vec![fld(folder("V2", vec![fld(inner)]))]));
    assert!(out.contains("## V2"));
    assert!(out.contains("### GET Deep Endpoint"));
}

#[test]
fn md_footer() {
    let c = NexusCollection {
        name: "Awesome API".into(),
        ..collection(vec![])
    };
    assert!(md(&c).contains("*Generated from \"Awesome API\" by Nexus*"));
}

#[test]
fn md_empty_collection() {
    let out = md(&collection(vec![]));
    assert!(out.contains("# My API"));
    assert!(!out.contains("## Requests"));
}

#[test]
fn md_invalid_graphql_variables_surface_as_error() {
    let r = with(|r| {
        r.body = RequestBody {
            mode: BodyMode::Graphql,
            graphql: Some(GraphqlBody {
                query: "{ a }".into(),
                variables: Some("{nope".into()),
            }),
            ..Default::default()
        };
    });
    assert!(generate_markdown(&collection(vec![req(r)])).is_err());
}

// ---- html ----

#[test]
fn html_full_document() {
    let out = html(&collection(vec![]));
    assert!(out.contains("<!DOCTYPE html>"));
    assert!(out.contains("</html>"));
}

#[test]
fn html_name_in_title_and_h1() {
    let c = NexusCollection {
        name: "Pet Store".into(),
        ..collection(vec![])
    };
    let out = html(&c);
    assert!(out.contains("<title>Pet Store \u{2014} API Documentation</title>"));
    assert!(out.contains("<h1>Pet Store</h1>"));
}

#[test]
fn html_escapes_collection_name() {
    let c = NexusCollection {
        name: "A & B <script>".into(),
        ..collection(vec![])
    };
    let out = html(&c);
    assert!(out.contains("A &amp; B &lt;script&gt;"));
    assert!(!out.contains("<script>"));
}

#[test]
fn html_escapes_endpoint_content() {
    let r = with(|r| {
        r.name = "<img src=x onerror=1>".into();
        r.url = "https://x.test/?a=\"b\"&c=<d>".into();
        r.headers = vec![kv("<h>", "\"v\"", true, Some("<b>desc</b>"))];
        r.body = RequestBody {
            mode: BodyMode::Text,
            raw: Some("<script>alert(1)</script>".into()),
            ..Default::default()
        };
    });
    let c = NexusCollection {
        description: Some("<i>d</i>".into()),
        variables: vec![base_url_var("<base>")],
        ..collection(vec![req(r)])
    };
    let out = html(&c);
    assert!(!out.contains("<script>"));
    assert!(!out.contains("<img"));
    assert!(!out.contains("<i>d</i>"));
    assert!(!out.contains("<b>desc</b>"));
    assert!(!out.contains("<base>"));
    assert!(out.contains("&lt;img src=x onerror=1&gt;"));
    assert!(out.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
}

#[test]
fn html_description() {
    let c = NexusCollection {
        description: Some("My awesome API".into()),
        ..collection(vec![])
    };
    assert!(html(&c).contains("My awesome API"));
}

#[test]
fn html_base_url() {
    let c = NexusCollection {
        variables: vec![base_url_var("https://api.test.com")],
        ..collection(vec![])
    };
    let out = html(&c);
    assert!(out.contains("https://api.test.com"));
    assert!(out.contains("base-url"));
}

#[test]
fn html_method_badge() {
    let r = with(|r| {
        r.method = HttpMethod::Post;
        r.name = "Create User".into();
    });
    let out = html(&collection(vec![req(r)]));
    assert!(out.contains("method-POST"));
    assert!(out.contains("POST"));
}

#[test]
fn html_headers_table() {
    let r = with(|r| r.headers = vec![kv("Accept", "application/json", true, None)]);
    let out = html(&collection(vec![req(r)]));
    assert!(out.contains("Headers"));
    assert!(out.contains("Accept"));
}

#[test]
fn html_query_params_table() {
    let r = with(|r| r.params = vec![kv("limit", "10", true, None)]);
    let out = html(&collection(vec![req(r)]));
    assert!(out.contains("Query Parameters"));
    assert!(out.contains("limit"));
}

#[test]
fn html_body_example() {
    let r = with(|r| {
        r.method = HttpMethod::Post;
        r.body = RequestBody {
            mode: BodyMode::Json,
            raw: Some(r#"{"key":"val"}"#.into()),
            ..Default::default()
        };
    });
    let out = html(&collection(vec![req(r)]));
    assert!(out.contains("Body (json)"));
    assert!(out.contains("{&quot;key&quot;:&quot;val&quot;}"));
}

#[test]
fn html_curl_section() {
    let out = html(&collection(vec![req(request())]));
    assert!(out.contains("cURL"));
    assert!(out.contains("curl"));
}

#[test]
fn html_footer() {
    let c = NexusCollection {
        name: "Test API".into(),
        ..collection(vec![])
    };
    assert!(html(&c).contains("Generated from \"Test API\" by Nexus"));
}

#[test]
fn html_table_of_contents_links() {
    let f = folder(
        "Auth",
        vec![req(NexusRequest {
            name: "Login".into(),
            method: HttpMethod::Post,
            url: "/login".into(),
            ..request()
        })],
    );
    let out = html(&collection(vec![fld(f)]));
    assert!(out.contains("Endpoints"));
    assert!(out.contains("href=\"#post-login\""));
    assert!(out.contains("id=\"post-login\""));
}

#[test]
fn html_empty_collection() {
    let out = html(&collection(vec![]));
    assert!(out.contains("<h1>My API</h1>"));
    assert!(out.contains("</html>"));
}

#[test]
fn slugify_collapses_and_trims() {
    assert_eq!(slugify("POST-Create  User!"), "post-create-user");
    assert_eq!(slugify("--A--"), "a");
}
