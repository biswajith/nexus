//! Port of `packages/core/src/docs/doc-generator.ts`: Markdown and HTML API documentation for a
//! collection. All collection-derived text is HTML-escaped in the HTML output.

use crate::codegen::generate_curl;
use crate::types::{CollectionItem, NexusCollection, NexusRequest};
use crate::Result;

const STYLE: &str = include_str!("style.html");

struct DocGroup {
    name: String,
    description: Option<String>,
    endpoints: Vec<DocEndpoint>,
}

struct DocField {
    key: String,
    value: String,
    description: Option<String>,
}

struct DocEndpoint {
    name: String,
    method: &'static str,
    url: String,
    headers: Vec<DocField>,
    params: Vec<DocField>,
    body_mode: String,
    body_example: Option<String>,
    curl: String,
}

/// Renders the collection as Markdown: table of contents, grouped endpoints and cURL examples.
pub fn generate_markdown(collection: &NexusCollection) -> Result<String> {
    let groups = extract_groups(collection)?;
    let mut lines: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| lines.extend(items.iter().map(|s| s.to_string()));

    push(&[&format!("# {}", collection.name)]);
    if let Some(desc) = collection.description.as_deref().filter(|d| !d.is_empty()) {
        push(&["", desc]);
    }
    if let Some(base) = base_url(collection).filter(|b| !b.is_empty()) {
        push(&["", &format!("**Base URL:** `{base}`")]);
    }

    push(&["", "---", "", "## Table of Contents", ""]);
    for group in &groups {
        push(&[&format!("- [{}](#{})", group.name, slugify(&group.name))]);
        for ep in &group.endpoints {
            push(&[&format!(
                "  - [{} {}](#{})",
                ep.method,
                ep.name,
                slugify(&format!("{}-{}", ep.method, ep.name))
            )]);
        }
    }

    for group in &groups {
        push(&["", "---", "", &format!("## {}", group.name)]);
        if let Some(desc) = group.description.as_deref().filter(|d| !d.is_empty()) {
            push(&["", desc]);
        }
        for ep in &group.endpoints {
            push(&["", &format!("### {} {}", ep.method, ep.name), ""]);
            push(&[&format!("`{} {}`", ep.method, ep.url)]);

            if !ep.headers.is_empty() {
                push(&[
                    "",
                    "**Headers:**",
                    "",
                    "| Header | Value | Description |",
                    "|--------|-------|-------------|",
                ]);
                for h in &ep.headers {
                    push(&[&format!(
                        "| `{}` | `{}` | {} |",
                        h.key,
                        h.value,
                        h.description.as_deref().unwrap_or("")
                    )]);
                }
            }
            if !ep.params.is_empty() {
                push(&[
                    "",
                    "**Query Parameters:**",
                    "",
                    "| Parameter | Value | Description |",
                    "|-----------|-------|-------------|",
                ]);
                for p in &ep.params {
                    push(&[&format!(
                        "| `{}` | `{}` | {} |",
                        p.key,
                        p.value,
                        p.description.as_deref().unwrap_or("")
                    )]);
                }
            }
            if let Some(example) = body_example(ep) {
                push(&[
                    "",
                    &format!("**Body** (`{}`):", ep.body_mode),
                    "",
                    "```json",
                    example,
                    "```",
                ]);
            }
            push(&["", "**cURL:**", "", "```bash", &ep.curl, "```"]);
        }
    }

    push(&[
        "",
        "---",
        "",
        &format!("*Generated from \"{}\" by Nexus*", collection.name),
    ]);
    Ok(lines.join("\n"))
}

/// Renders the collection as a single-page styled HTML document.
pub fn generate_html(collection: &NexusCollection) -> Result<String> {
    let groups = extract_groups(collection)?;
    let name = esc(&collection.name);
    let mut out = String::new();

    out.push_str(&format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"UTF-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n  <title>{name} \u{2014} API Documentation</title>\n"
    ));
    out.push_str(STYLE);
    out.push_str("</head>\n<body>\n  <div class=\"container\">\n");
    out.push_str(&format!("    <h1>{name}</h1>\n"));
    out.push_str("    ");
    if let Some(desc) = collection.description.as_deref().filter(|d| !d.is_empty()) {
        out.push_str(&format!("<p>{}</p>", esc(desc)));
    }
    out.push('\n');
    out.push_str("    ");
    if let Some(base) = base_url(collection) {
        out.push_str(&format!("<div class=\"base-url\">{}</div>", esc(base)));
    }
    out.push_str("\n\n    <div class=\"toc\">\n      <strong>Endpoints</strong>\n      <ul>\n");
    let toc: Vec<String> = groups
        .iter()
        .map(|g| {
            let items: String = g
                .endpoints
                .iter()
                .map(|ep| {
                    format!(
                        "<li><a href=\"#{}\"><span class=\"method-badge method-{}\">{}</span>{}</a></li>",
                        slugify(&format!("{}-{}", ep.method, ep.name)),
                        ep.method,
                        ep.method,
                        esc(&ep.name)
                    )
                })
                .collect();
            format!("        <li><strong>{}</strong><ul>{items}</ul></li>", esc(&g.name))
        })
        .collect();
    out.push_str(&toc.join("\n"));
    out.push_str("\n      </ul>\n    </div>\n\n");

    for g in &groups {
        out.push_str(&format!("\n    <h2>{}</h2>\n    ", esc(&g.name)));
        if let Some(desc) = g.description.as_deref().filter(|d| !d.is_empty()) {
            out.push_str(&format!("<p>{}</p>", esc(desc)));
        }
        out.push('\n');
        let endpoints: Vec<String> = g.endpoints.iter().map(endpoint_html).collect();
        out.push_str(&endpoints.join("\n"));
    }

    out.push_str(&format!("\n\n    <div class=\"footer\">Generated from \"{name}\" by Nexus</div>\n  </div>\n</body>\n</html>"));
    Ok(out)
}

fn endpoint_html(ep: &DocEndpoint) -> String {
    let mut s = format!(
        "\n    <div class=\"endpoint\" id=\"{}\">\n      <h3><span class=\"method-badge method-{}\">{}</span><span class=\"url\">{}</span></h3>\n",
        slugify(&format!("{}-{}", ep.method, ep.name)),
        ep.method,
        ep.method,
        esc(&ep.url)
    );
    s.push_str("      ");
    if !ep.headers.is_empty() {
        s.push_str(&table_html("Headers", "Header", &ep.headers));
    }
    s.push_str("\n      ");
    if !ep.params.is_empty() {
        s.push_str(&table_html("Query Parameters", "Parameter", &ep.params));
    }
    s.push_str("\n      ");
    if let Some(example) = body_example(ep) {
        s.push_str(&format!(
            "<div class=\"label\">Body ({})</div><pre><code>{}</code></pre>",
            esc(&ep.body_mode),
            esc(example)
        ));
    }
    s.push_str(&format!(
        "\n      <div class=\"label\">cURL</div><pre><code>{}</code></pre>\n    </div>",
        esc(&ep.curl)
    ));
    s
}

fn table_html(label: &str, first_col: &str, rows: &[DocField]) -> String {
    let rows: String = rows
        .iter()
        .map(|r| {
            format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>",
                esc(&r.key),
                esc(&r.value),
                esc(r.description.as_deref().unwrap_or(""))
            )
        })
        .collect();
    format!("<div class=\"label\">{label}</div><table><tr><th>{first_col}</th><th>Value</th><th>Description</th></tr>{rows}</table>")
}

fn body_example(ep: &DocEndpoint) -> Option<&str> {
    if ep.body_mode == "none" {
        return None;
    }
    ep.body_example.as_deref().filter(|e| !e.is_empty())
}

/// The first `base_url` variable, regardless of its enabled flag (as in the TS code).
fn base_url(collection: &NexusCollection) -> Option<&str> {
    collection
        .variables
        .iter()
        .find(|v| v.key == "base_url")
        .map(|v| v.value.as_str())
}

/// Folder-backed groups, then a final "Requests" group for ungrouped top-level requests.
fn extract_groups(collection: &NexusCollection) -> Result<Vec<DocGroup>> {
    let mut groups = Vec::new();
    let mut ungrouped = Vec::new();
    for item in &collection.items {
        match item {
            CollectionItem::Folder(folder) => groups.push(DocGroup {
                name: folder.name.clone(),
                description: folder.description.clone(),
                endpoints: flatten_endpoints(&folder.items)?,
            }),
            CollectionItem::Request(req) => ungrouped.push(to_endpoint(req)?),
        }
    }
    if !ungrouped.is_empty() {
        groups.push(DocGroup {
            name: "Requests".to_string(),
            description: None,
            endpoints: ungrouped,
        });
    }
    Ok(groups)
}

fn flatten_endpoints(items: &[CollectionItem]) -> Result<Vec<DocEndpoint>> {
    let mut result = Vec::new();
    for item in items {
        match item {
            CollectionItem::Folder(f) => result.extend(flatten_endpoints(&f.items)?),
            CollectionItem::Request(r) => result.push(to_endpoint(r)?),
        }
    }
    Ok(result)
}

fn to_endpoint(req: &NexusRequest) -> Result<DocEndpoint> {
    let fields = |list: &[crate::types::KeyValuePair]| {
        list.iter()
            .filter(|kv| kv.enabled)
            .map(|kv| DocField {
                key: kv.key.clone(),
                value: kv.value.clone(),
                description: kv.description.clone(),
            })
            .collect()
    };
    Ok(DocEndpoint {
        name: req.name.clone(),
        method: req.method.as_str(),
        url: req.url.clone(),
        headers: fields(&req.headers),
        params: fields(&req.params),
        body_mode: serde_json::to_value(req.body.mode)?
            .as_str()
            .unwrap_or("none")
            .to_string(),
        body_example: req
            .body
            .raw
            .clone()
            .or_else(|| req.body.graphql.as_ref().map(|g| g.query.clone())),
        curl: generate_curl(req)?,
    })
}

/// Lowercases and replaces non-alphanumeric runs with `-`, trimming leading/trailing `-`.
fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            pending_dash = false;
        } else if !pending_dash {
            out.push('-');
            pending_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Escapes `&`, `<`, `>` and `"`.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
