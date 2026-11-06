//! Port of `packages/core/src/visualizer/visualizer.ts`: a tiny Mustache-like HTML template
//! renderer (`{{x}}`, `{{{x}}}`, `{{#each}}`, `{{#if}}`, `{{#unless}}`) plus a dark-theme wrapper.

use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde_json::{Map, Value};

static EACH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{#each\s+([A-Za-z0-9_]+)\}\}([\s\S]*?)\{\{/each\}\}").expect("valid regex")
});
static IF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{#if\s+([A-Za-z0-9_]+)\}\}([\s\S]*?)\{\{/if\}\}").expect("valid regex")
});
static UNLESS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{#unless\s+([A-Za-z0-9_]+)\}\}([\s\S]*?)\{\{/unless\}\}").expect("valid regex")
});
// `.` in the JS regexes does not match line terminators.
static RAW_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\{([^\n\r\u{2028}\u{2029}]+?)\}\}\}").expect("valid regex"));
static ESCAPED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{([^\n\r\u{2028}\u{2029}]+?)\}\}").expect("valid regex"));

const WRAPPER_HEAD: &str = include_str!("wrapper_head.html");
const WRAPPER_TAIL: &str = "</body></html>";

#[derive(Debug, Clone, PartialEq)]
pub struct VisualizerConfig {
    pub template: String,
    pub data: Value,
}

/// Holds a template and data for rendering HTML snippets and full preview documents.
#[derive(Debug, Clone, Default)]
pub struct Visualizer {
    config: Option<VisualizerConfig>,
}

impl Visualizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, template: impl Into<String>, data: Value) {
        self.config = Some(VisualizerConfig {
            template: template.into(),
            data,
        });
    }

    pub fn config(&self) -> Option<&VisualizerConfig> {
        self.config.as_ref()
    }

    pub fn clear(&mut self) {
        self.config = None;
    }

    /// Renders the template against the data, or `None` when nothing is configured.
    pub fn render(&self) -> Option<String> {
        self.config
            .as_ref()
            .map(|c| render_template(&c.template, &c.data))
    }

    /// Like [`Visualizer::render`] but wrapped in a full dark-theme HTML document. Returns `None`
    /// when nothing is configured or the rendered body is empty (as in the TS `if (!body)`).
    pub fn render_with_wrapper(&self) -> Option<String> {
        let body = self.render().filter(|b| !b.is_empty())?;
        Some(format!("{WRAPPER_HEAD}{body}{WRAPPER_TAIL}"))
    }
}

/// Renders `template` using `context` as the root scope.
pub fn render_template(template: &str, context: &Value) -> String {
    process_template(template, context)
}

fn process_template(tmpl: &str, ctx: &Value) -> String {
    let result = EACH_RE.replace_all(tmpl, |caps: &Captures| {
        let Some(Value::Array(items)) = resolve(ctx, &caps[1]) else {
            return String::new();
        };
        items
            .iter()
            .enumerate()
            .map(|(index, item)| process_template(&caps[2], &item_context(ctx, item, index)))
            .collect::<String>()
    });
    let result = IF_RE.replace_all(&result, |caps: &Captures| {
        if is_truthy(resolve(ctx, &caps[1]).as_ref()) {
            process_template(&caps[2], ctx)
        } else {
            String::new()
        }
    });
    let result = UNLESS_RE.replace_all(&result, |caps: &Captures| {
        if is_truthy(resolve(ctx, &caps[1]).as_ref()) {
            String::new()
        } else {
            process_template(&caps[2], ctx)
        }
    });
    let result = RAW_RE.replace_all(&result, |caps: &Captures| {
        to_js_string(resolve(ctx, caps[1].trim()).as_ref())
    });
    let result = ESCAPED_RE.replace_all(&result, |caps: &Captures| {
        escape_html(&to_js_string(resolve(ctx, caps[1].trim()).as_ref()))
    });
    result.into_owned()
}

/// Scope for one `{{#each}}` iteration: parent scope, then the item's own keys (objects only),
/// then `@index` and `this` (and `.` for non-object items).
fn item_context(ctx: &Value, item: &Value, index: usize) -> Value {
    let mut scope: Map<String, Value> = ctx.as_object().cloned().unwrap_or_default();
    if let Value::Object(obj) = item {
        scope.extend(obj.clone());
    } else if let Value::Array(items) = item {
        // JS spreads arrays into index keys (and sets no `.`).
        scope.extend(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v.clone())),
        );
    } else {
        scope.insert(".".to_string(), item.clone());
    }
    scope.insert("@index".to_string(), Value::from(index));
    scope.insert("this".to_string(), item.clone());
    Value::Object(scope)
}

/// Walks a dotted path; `.` and `this` mean the current each-item. Array indexes work as path parts.
fn resolve(ctx: &Value, path: &str) -> Option<Value> {
    if path == "." || path == "this" {
        return match ctx.get("this") {
            Some(v) if !v.is_null() => Some(v.clone()),
            _ => Some(ctx.clone()),
        };
    }
    let mut current = ctx;
    for part in path.split('.') {
        current = match current {
            Value::Object(m) => m.get(part)?,
            Value::Array(a) => a.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current.clone())
}

/// `{{#if}}` truthiness: JS rules, except that empty arrays are falsy.
fn is_truthy(val: Option<&Value>) -> bool {
    match val {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(_)) => true,
    }
}

/// JS `String(val)` with null/undefined as the empty string.
fn to_js_string(val: Option<&Value>) -> String {
    match val {
        None | Some(Value::Null) => String::new(),
        Some(v) => js_string(v),
    }
}

fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.is_finite() && f.fract() == 0.0 && f.abs() < 1e21 => {
                format!("{f:.0}")
            }
            _ => n.to_string(),
        },
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// Escapes `&`, `<`, `>` and `"`.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests;
