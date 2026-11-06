//! SDL printer for an introspection result (`{"__schema": ...}`), following `graphql-js`
//! 16's `printSchema(buildClientSchema(data))`: directives first, then types in
//! introspection order, built-in scalars/directives and `__` types omitted.

use crate::{Error, Result};
use serde_json::Value;

const SPECIFIED_SCALARS: &[&str] = &["Int", "Float", "String", "Boolean", "ID"];
const SPECIFIED_DIRECTIVES: &[&str] = &["skip", "include", "deprecated", "specifiedBy"];
const DEFAULT_DEPRECATION_REASON: &str = "No longer supported";

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn arr_of<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Prints the schema in an introspection result (the `data` of the introspection query).
pub fn print_introspection_schema(data: &Value) -> Result<String> {
    let schema = data
        .get("__schema")
        .filter(|s| s.is_object())
        .ok_or_else(|| {
            Error::Other("Invalid or incomplete introspection result: missing __schema".into())
        })?;

    let mut blocks: Vec<String> = Vec::new();
    if let Some(def) = print_schema_definition(schema) {
        blocks.push(def);
    }
    for d in arr_of(schema, "directives") {
        if str_of(d, "name").is_some_and(|n| !SPECIFIED_DIRECTIVES.contains(&n)) {
            blocks.push(print_directive(d));
        }
    }
    for t in arr_of(schema, "types") {
        let Some(name) = str_of(t, "name") else {
            continue;
        };
        if name.starts_with("__") || SPECIFIED_SCALARS.contains(&name) {
            continue;
        }
        blocks.push(print_type(t));
    }
    Ok(blocks.join("\n\n"))
}

fn root_name<'a>(schema: &'a Value, key: &str) -> Option<&'a str> {
    schema.get(key).and_then(|t| str_of(t, "name"))
}

fn print_schema_definition(schema: &Value) -> Option<String> {
    let query = root_name(schema, "queryType");
    let mutation = root_name(schema, "mutationType");
    let subscription = root_name(schema, "subscriptionType");
    let common = query.is_none_or(|n| n == "Query")
        && mutation.is_none_or(|n| n == "Mutation")
        && subscription.is_none_or(|n| n == "Subscription");
    if common {
        return None;
    }
    let mut out = String::from("schema {");
    for (op, name) in [
        ("query", query),
        ("mutation", mutation),
        ("subscription", subscription),
    ] {
        if let Some(n) = name {
            out.push_str(&format!("\n  {op}: {n}"));
        }
    }
    out.push_str("\n}");
    Some(out)
}

fn print_directive(d: &Value) -> String {
    let locations: Vec<&str> = arr_of(d, "locations")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    format!(
        "{}directive @{}{} on {}",
        print_description(d, "", true),
        str_of(d, "name").unwrap_or(""),
        print_args(arr_of(d, "args"), ""),
        locations.join(" | ")
    )
}

fn print_type(t: &Value) -> String {
    let name = str_of(t, "name").unwrap_or("");
    let desc = print_description(t, "", true);
    match str_of(t, "kind").unwrap_or("") {
        "SCALAR" => format!("{desc}scalar {name}"),
        "UNION" => {
            let members: Vec<String> = arr_of(t, "possibleTypes")
                .iter()
                .map(print_type_ref)
                .collect();
            if members.is_empty() {
                format!("{desc}union {name}")
            } else {
                format!("{desc}union {name} = {}", members.join(" | "))
            }
        }
        "ENUM" => {
            let values: Vec<String> = arr_of(t, "enumValues")
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    format!(
                        "{}  {}{}",
                        print_description(v, "  ", i == 0),
                        str_of(v, "name").unwrap_or(""),
                        print_deprecated(v)
                    )
                })
                .collect();
            format!("{desc}enum {name}{}", print_block(&values))
        }
        "INPUT_OBJECT" => {
            let fields: Vec<String> = arr_of(t, "inputFields")
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    format!(
                        "{}  {}",
                        print_description(f, "  ", i == 0),
                        print_input_value(f)
                    )
                })
                .collect();
            format!("{desc}input {name}{}", print_block(&fields))
        }
        kind => {
            let keyword = if kind == "INTERFACE" {
                "interface"
            } else {
                "type"
            };
            let interfaces: Vec<String> =
                arr_of(t, "interfaces").iter().map(print_type_ref).collect();
            let implements = if interfaces.is_empty() {
                String::new()
            } else {
                format!(" implements {}", interfaces.join(" & "))
            };
            let fields: Vec<String> = arr_of(t, "fields")
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    format!(
                        "{}  {}{}: {}{}",
                        print_description(f, "  ", i == 0),
                        str_of(f, "name").unwrap_or(""),
                        print_args(arr_of(f, "args"), "  "),
                        f.get("type").map(print_type_ref).unwrap_or_default(),
                        print_deprecated(f)
                    )
                })
                .collect();
            format!("{desc}{keyword} {name}{implements}{}", print_block(&fields))
        }
    }
}

fn print_block(items: &[String]) -> String {
    if items.is_empty() {
        String::new()
    } else {
        format!(" {{\n{}\n}}", items.join("\n"))
    }
}

fn print_args(args: &[Value], indentation: &str) -> String {
    if args.is_empty() {
        return String::new();
    }
    if args
        .iter()
        .all(|a| str_of(a, "description").is_none_or(str::is_empty))
    {
        let inline: Vec<String> = args.iter().map(print_input_value).collect();
        return format!("({})", inline.join(", "));
    }
    let lines: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                "{}  {indentation}{}",
                print_description(a, &format!("  {indentation}"), i == 0),
                print_input_value(a)
            )
        })
        .collect();
    format!("(\n{}\n{indentation})", lines.join("\n"))
}

fn print_input_value(v: &Value) -> String {
    let mut s = format!(
        "{}: {}",
        str_of(v, "name").unwrap_or(""),
        v.get("type").map(print_type_ref).unwrap_or_default()
    );
    if let Some(d) = str_of(v, "defaultValue") {
        s.push_str(&format!(" = {d}"));
    }
    s
}

fn print_type_ref(t: &Value) -> String {
    match str_of(t, "kind") {
        Some("NON_NULL") => format!(
            "{}!",
            t.get("ofType").map(print_type_ref).unwrap_or_default()
        ),
        Some("LIST") => format!(
            "[{}]",
            t.get("ofType").map(print_type_ref).unwrap_or_default()
        ),
        _ => str_of(t, "name").unwrap_or("").to_string(),
    }
}

fn print_deprecated(v: &Value) -> String {
    if v.get("isDeprecated").and_then(Value::as_bool) != Some(true) {
        return String::new();
    }
    match str_of(v, "deprecationReason") {
        None | Some(DEFAULT_DEPRECATION_REASON) => " @deprecated".to_string(),
        Some(reason) => format!(
            " @deprecated(reason: {})",
            serde_json::to_string(reason).unwrap_or_default()
        ),
    }
}

/// `printDescription`: a block string, preceded by a blank line unless first in its block.
fn print_description(def: &Value, indentation: &str, first_in_block: bool) -> String {
    let Some(desc) = str_of(def, "description").filter(|d| !d.is_empty()) else {
        return String::new();
    };
    let prefix = if !indentation.is_empty() && !first_in_block {
        "\n"
    } else {
        ""
    };
    let block = print_block_string(desc).replace('\n', &format!("\n{indentation}"));
    format!("{prefix}{indentation}{block}\n")
}

fn is_ws(c: Option<char>) -> bool {
    matches!(c, Some(' ') | Some('\t'))
}

/// `printBlockString(value)` from graphql-js 16 (not minimized).
fn print_block_string(value: &str) -> String {
    let escaped = value.replace("\"\"\"", "\\\"\"\"");
    let normalized = escaped.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let single_line = lines.len() == 1;
    let force_leading_newline = lines.len() > 1
        && lines[1..]
            .iter()
            .all(|l| l.is_empty() || is_ws(l.chars().next()));
    let trailing_triple = escaped.ends_with("\\\"\"\"");
    let trailing_quote = value.ends_with('"') && !trailing_triple;
    let trailing_slash = value.ends_with('\\');
    let force_trailing_newline = trailing_quote || trailing_slash;
    let multiple = !single_line
        || value.chars().count() > 70
        || force_trailing_newline
        || force_leading_newline
        || trailing_triple;
    let skip_leading_newline = single_line && is_ws(value.chars().next());

    let mut result = String::new();
    if (multiple && !skip_leading_newline) || force_leading_newline {
        result.push('\n');
    }
    result.push_str(&escaped);
    if multiple || force_trailing_newline {
        result.push('\n');
    }
    format!("\"\"\"{result}\"\"\"")
}
