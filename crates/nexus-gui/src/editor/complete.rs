//! Autocomplete sources: `{{variable}}` names everywhere, the `nx.*` script API in JavaScript.
//! Pure functions from (text, caret) to a list of items; the widget draws and applies them.

use crate::highlight::Lang;
use crate::payload::Scopes;
use nexus_core::environments::DYNAMIC_KEYS;
use nexus_core::types::VariableType;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Variable,
    Function,
    Property,
}

impl ItemKind {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Variable => "{}",
            Self::Function => "f",
            Self::Property => "p",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub kind: ItemKind,
    /// What replaces the typed word; `$0` marks where the caret goes. `None` = the label.
    pub apply: Option<String>,
}

/// A variable the user can reference with `{{name}}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarItem {
    pub name: String,
    /// `environment` / `collection` / `global`.
    pub scope: &'static str,
    pub value: String,
}

/// Enabled variables of the three scopes, innermost first, shadowed names dropped.
pub fn var_items(scopes: &Scopes) -> Vec<VarItem> {
    let mut out: Vec<VarItem> = Vec::new();
    for (scope, vars) in [
        ("environment", &scopes.environment),
        ("collection", &scopes.collection),
        ("global", &scopes.global),
    ] {
        for v in vars.iter().filter(|v| v.enabled && !v.key.is_empty()) {
            if !out.iter().any(|o| o.name == v.key) {
                out.push(VarItem {
                    name: v.key.clone(),
                    scope,
                    value: if v.var_type == VariableType::Secret {
                        "********".to_string()
                    } else {
                        v.value.clone()
                    },
                });
            }
        }
    }
    out
}

/// An open completion list.
#[derive(Clone, Debug)]
pub struct Session {
    /// Start of the text the chosen item replaces (it ends at the caret).
    pub from: usize,
    pub items: Vec<Item>,
    pub selected: usize,
    /// The popup was asked for with Ctrl+Space.
    pub explicit: bool,
}

/// Result of accepting an item: the replacement and where the caret ends up.
#[derive(Debug, PartialEq, Eq)]
pub struct Applied {
    pub range: std::ops::Range<usize>,
    pub text: String,
    /// Caret offset in the new text (absolute).
    pub caret: usize,
}

const MAX_ITEMS: usize = 60;

fn nx(label: &str, kind: ItemKind, info: &str) -> Item {
    Item {
        label: label.to_string(),
        detail: info.to_string(),
        kind,
        apply: None,
    }
}

fn nx_apply(label: &str, kind: ItemKind, info: &str, apply: &str) -> Item {
    Item {
        apply: Some(apply.to_string()),
        ..nx(label, kind, info)
    }
}

/// The `nx.*` script API, as the previous editor listed it (plus the members
/// the sandbox runtime has that it missed).
pub fn nx_items() -> Vec<Item> {
    use ItemKind::{Function as F, Property as P};
    vec![
        nx("nx.environment.get", F, "Get environment variable"),
        nx("nx.environment.set", F, "Set environment variable"),
        nx("nx.environment.unset", F, "Remove environment variable"),
        nx("nx.environment.toObject", F, "All environment variables"),
        nx("nx.collectionVariables.get", F, "Get collection variable"),
        nx("nx.collectionVariables.set", F, "Set collection variable"),
        nx(
            "nx.collectionVariables.unset",
            F,
            "Remove collection variable",
        ),
        nx("nx.globals.get", F, "Get global variable"),
        nx("nx.globals.set", F, "Set global variable"),
        nx("nx.globals.unset", F, "Remove global variable"),
        nx("nx.variables.get", F, "Get variable from any scope"),
        nx("nx.variables.set", F, "Set environment variable"),
        nx(
            "nx.variables.replaceIn",
            F,
            "Resolve {{variables}} in a string",
        ),
        nx("nx.request.url", P, "Request URL (mutable)"),
        nx("nx.request.method", P, "HTTP method (mutable)"),
        nx("nx.request.headers", P, "Request headers (mutable)"),
        nx("nx.request.body", P, "Request body (mutable)"),
        nx("nx.response.code", P, "HTTP status code"),
        nx("nx.response.status", P, "Status text"),
        nx("nx.response.headers", P, "Response headers"),
        nx("nx.response.responseTime", P, "Response time in ms"),
        nx("nx.response.json()", F, "Parse response body as JSON"),
        nx("nx.response.text()", F, "Response body as text"),
        nx_apply(
            "nx.test",
            F,
            "Define a test assertion",
            "nx.test(\"$0\", () => {\n  \n});",
        ),
        nx_apply("nx.expect", F, "Create an expectation", "nx.expect($0)"),
        nx_apply(
            "nx.visualizer.set",
            F,
            "Render a template with data in the Visualize tab",
            "nx.visualizer.set($0)",
        ),
        nx(
            "nx.execution.setNextRequest",
            F,
            "Set next request in runner",
        ),
        nx("nx.execution.skipRequest", F, "Skip current request"),
        nx("console.log", F, "Log to console"),
        nx("console.info", F, "Log info to console"),
        nx("console.warn", F, "Log a warning to console"),
        nx("console.error", F, "Log an error to console"),
    ]
}

/// Members chainable after `nx.expect(value)`.
pub fn expect_items() -> Vec<Item> {
    use ItemKind::{Function as F, Property as P};
    let words = [
        ("to", P),
        ("be", P),
        ("have", P),
        ("not", P),
        ("a", P),
        ("an", P),
        ("true", P),
        ("false", P),
        ("null", P),
        ("undefined", P),
        ("empty", P),
        ("ok", P),
    ];
    let mut v: Vec<Item> = words
        .iter()
        .map(|(l, k)| nx(l, *k, "Assertion chain"))
        .collect();
    for (l, info) in [
        ("equal", "Strict / deep equality"),
        ("eql", "Deep equality"),
        ("include", "Contains item or substring"),
        ("contain", "Contains item or substring"),
        ("property", "Has property (optionally with value)"),
        ("length", "Has length"),
        ("above", "Greater than"),
        ("below", "Less than"),
        ("least", "Greater than or equal"),
        ("most", "Less than or equal"),
        ("match", "Matches a regular expression"),
        ("oneOf", "Is one of the list"),
        ("matchSchema", "Validates against a JSON Schema"),
    ] {
        v.push(nx_apply(l, F, info, &format!("{l}($0)")));
    }
    v
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '$' | '.' | '-')
}

/// Candidates for the caret at `cursor`. `explicit` (Ctrl+Space) opens the list on an empty word.
pub fn query(
    lang: Lang,
    text: &str,
    cursor: usize,
    vars: &[VarItem],
    explicit: bool,
) -> Option<Session> {
    let before = &text[..cursor];
    // ---- {{variable ----
    let word_start = before.rfind(|c: char| !is_ident(c)).map_or(0, |i| {
        i + before[i..].chars().next().map_or(1, char::len_utf8)
    });
    let prefix = &before[word_start..];
    if before[..word_start].ends_with("{{") {
        let items = variable_items(vars, prefix);
        if items.is_empty() {
            return None;
        }
        return Some(Session {
            from: word_start,
            items,
            selected: 0,
            explicit,
        });
    }
    if lang == Lang::Graphql {
        return schema_words(before, vars, explicit);
    }
    if lang != Lang::JavaScript {
        return None;
    }
    // ---- nx.* in scripts ----
    let js_start = before
        .rfind(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '$' | '.')))
        .map_or(0, |i| {
            i + before[i..].chars().next().map_or(1, char::len_utf8)
        });
    let word = &before[js_start..];
    // `nx.expect(x).to.` -> chain members
    if word.starts_with('.') && before[..js_start].ends_with(')') {
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        if before[line_start..js_start].contains("expect(") {
            let seg_start = js_start + word.rfind('.').unwrap_or(0) + 1;
            let typed = &before[seg_start..];
            let items: Vec<Item> = expect_items()
                .into_iter()
                .filter(|i| i.label.to_lowercase().starts_with(&typed.to_lowercase()))
                .collect();
            if items.is_empty() {
                return None;
            }
            return Some(Session {
                from: seg_start,
                items,
                selected: 0,
                explicit,
            });
        }
        return None;
    }
    if word.is_empty() && !explicit {
        return None;
    }
    if word.chars().count() < 2 && !explicit {
        return None;
    }
    let lw = word.to_lowercase();
    let items: Vec<Item> = nx_items()
        .into_iter()
        .filter(|i| i.label.to_lowercase().starts_with(&lw))
        .collect();
    // an exact, complete match needs no popup
    if items.is_empty() || (items.len() == 1 && items[0].label == word && !explicit) {
        return None;
    }
    Some(Session {
        from: js_start,
        items,
        selected: 0,
        explicit,
    })
}

/// Scope of the [`VarItem`]s that carry GraphQL schema names (types, fields) instead of variables.
pub const SCHEMA_SCOPE: &str = "schema";

/// GraphQL: type / field names of the introspected schema that start with the word at the caret.
fn schema_words(before: &str, vars: &[VarItem], explicit: bool) -> Option<Session> {
    let start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map_or(0, |i| {
            i + before[i..].chars().next().map_or(1, char::len_utf8)
        });
    let word = &before[start..];
    if word.chars().count() < 2 && !explicit {
        return None;
    }
    let lw = word.to_lowercase();
    let items: Vec<Item> = vars
        .iter()
        .filter(|v| v.scope == SCHEMA_SCOPE && v.name.to_lowercase().starts_with(&lw))
        .take(MAX_ITEMS)
        .map(|v| Item {
            label: v.name.clone(),
            detail: "schema".to_string(),
            kind: ItemKind::Property,
            apply: None,
        })
        .collect();
    if items.is_empty() || (items.len() == 1 && items[0].label == word && !explicit) {
        return None;
    }
    Some(Session {
        from: start,
        items,
        selected: 0,
        explicit,
    })
}

/// Variables whose name starts with `prefix` (then those containing it), then `$dynamic` names.
fn variable_items(vars: &[VarItem], prefix: &str) -> Vec<Item> {
    let p = prefix.to_lowercase();
    let mut starts = Vec::new();
    let mut contains = Vec::new();
    let named = vars.iter().filter(|v| v.scope != SCHEMA_SCOPE).map(|v| {
        (
            v.name.as_str(),
            Item {
                label: v.name.clone(),
                detail: format!("{} - {}", v.scope, shorten(&v.value)),
                kind: ItemKind::Variable,
                apply: None,
            },
        )
    });
    let dynamic = DYNAMIC_KEYS.iter().map(|k| {
        (
            *k,
            Item {
                label: (*k).to_string(),
                detail: "dynamic".to_string(),
                kind: ItemKind::Variable,
                apply: None,
            },
        )
    });
    for (name, item) in named.chain(dynamic) {
        let n = name.to_lowercase();
        if n.starts_with(&p) {
            starts.push(item);
        } else if n.contains(&p) {
            contains.push(item);
        }
    }
    starts.extend(contains);
    starts.truncate(MAX_ITEMS);
    starts
}

fn shorten(v: &str) -> String {
    let v: String = v.chars().take(24).collect();
    v.replace('\n', " ")
}

/// What accepting `item` does to `text` (caret at `cursor`).
pub fn apply(text: &str, session: &Session, cursor: usize) -> Applied {
    let item = &session.items[session.selected.min(session.items.len() - 1)];
    let from = session.from;
    let src = item.apply.clone().unwrap_or_else(|| item.label.clone());
    let (mut ins, caret_in) = match src.find("$0") {
        Some(i) => (src.replacen("$0", "", 1), i),
        None => (src.clone(), src.len()),
    };
    let mut caret = from + caret_in;
    let mut end = cursor;
    if item.kind == ItemKind::Variable {
        // close the braces unless they are already there
        if text[cursor..].starts_with("}}") {
            end = cursor;
            caret = from + ins.len() + 2;
        } else {
            ins.push_str("}}");
            caret = from + ins.len();
        }
    }
    Applied {
        range: from..end,
        text: ins,
        caret,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::types::Variable;

    fn var(k: &str, v: &str) -> Variable {
        Variable {
            key: k.into(),
            value: v.into(),
            var_type: VariableType::String,
            enabled: true,
            description: None,
        }
    }

    fn vars() -> Vec<VarItem> {
        var_items(&Scopes {
            environment: vec![var("baseUrl", "http://x"), var("token", "abc")],
            collection: vec![var("apiVersion", "v2"), var("token", "shadowed")],
            global: vec![var("who", "me")],
        })
    }

    fn labels(s: &Session) -> Vec<&str> {
        s.items.iter().map(|i| i.label.as_str()).collect()
    }

    #[test]
    fn variables_are_listed_innermost_first_without_shadowed() {
        let v = vars();
        let names: Vec<_> = v.iter().map(|v| (v.name.as_str(), v.scope)).collect();
        assert_eq!(
            names,
            [
                ("baseUrl", "environment"),
                ("token", "environment"),
                ("apiVersion", "collection"),
                ("who", "global")
            ]
        );
        assert_eq!(v[1].value, "abc");
    }

    #[test]
    fn disabled_variables_are_not_offered() {
        let mut off = var("secret", "x");
        off.enabled = false;
        let v = var_items(&Scopes {
            environment: vec![off],
            ..Default::default()
        });
        assert!(v.is_empty());
    }

    #[test]
    fn brace_brace_offers_variables_then_dynamic() {
        let v = vars();
        let t = "{\"u\": \"{{";
        let s = query(Lang::Json, t, t.len(), &v, false).unwrap();
        assert_eq!(&labels(&s)[..4], ["baseUrl", "token", "apiVersion", "who"]);
        assert!(labels(&s).contains(&"$guid"));
        assert_eq!(s.from, t.len());
        let t = "{{ba";
        let s = query(Lang::Text, t, t.len(), &v, false).unwrap();
        assert_eq!(labels(&s)[0], "baseUrl");
        assert_eq!(s.from, 2);
        let s = query(Lang::Text, "{{$ran", 6, &v, false).unwrap();
        assert!(labels(&s).iter().all(|l| l.starts_with("$ran")));
        assert!(query(Lang::Text, "{{zzzz", 6, &v, false).is_none());
        assert!(query(Lang::Text, "plain", 5, &v, false).is_none());
    }

    #[test]
    fn applying_a_variable_closes_the_braces_once() {
        let v = vars();
        let t = "GET {{ba";
        let s = query(Lang::Text, t, t.len(), &v, false).unwrap();
        let a = apply(t, &s, t.len());
        assert_eq!(a.text, "baseUrl}}");
        assert_eq!(a.caret, 6 + "baseUrl}}".len());
        // braces already present (auto-closed): not doubled, caret moves past them
        let t = "{{ba}}";
        let s = query(Lang::Json, t, 4, &v, false).unwrap();
        let a = apply(t, &s, 4);
        assert_eq!(a.text, "baseUrl");
        assert_eq!(a.range, 2..4);
        assert_eq!(a.caret, 2 + 7 + 2);
    }

    #[test]
    fn nx_members_in_scripts_only() {
        let t = "nx.";
        let s = query(Lang::JavaScript, t, 3, &[], false).unwrap();
        assert!(labels(&s).iter().all(|l| l.starts_with("nx.")));
        assert!(labels(&s).contains(&"nx.response.json()"));
        assert!(query(Lang::Json, t, 3, &[], false).is_none());
        let t = "const x = nx.env";
        let s = query(Lang::JavaScript, t, t.len(), &[], false).unwrap();
        assert_eq!(
            labels(&s),
            [
                "nx.environment.get",
                "nx.environment.set",
                "nx.environment.unset",
                "nx.environment.toObject"
            ]
        );
        assert_eq!(s.from, 10);
        // single letters stay quiet unless asked for
        assert!(query(Lang::JavaScript, "n", 1, &[], false).is_none());
        assert!(query(Lang::JavaScript, "n", 1, &[], true).is_some());
        // a complete member needs no popup
        assert!(query(Lang::JavaScript, "nx.test", 7, &[], false).is_none());
        assert!(query(Lang::JavaScript, "nx.response.code", 16, &[], false).is_none());
    }

    #[test]
    fn nx_apply_places_the_caret() {
        let t = "nx.te";
        let s = query(Lang::JavaScript, t, t.len(), &[], false).unwrap();
        let a = apply(t, &s, t.len());
        assert_eq!(a.text, "nx.test(\"\", () => {\n  \n});");
        assert_eq!(a.caret, "nx.test(\"".len());
        let t = "nx.response.j";
        let s = query(Lang::JavaScript, t, t.len(), &[], false).unwrap();
        let a = apply(t, &s, t.len());
        assert_eq!(a.text, "nx.response.json()");
        assert_eq!(a.caret, a.text.len());
    }

    #[test]
    fn expect_chain_members() {
        let t = "nx.expect(x).to.equ";
        let s = query(Lang::JavaScript, t, t.len(), &[], false).unwrap();
        assert_eq!(labels(&s), ["equal"]);
        assert_eq!(&t[s.from..], "equ");
        let a = apply(t, &s, t.len());
        assert_eq!(a.text, "equal()");
        assert_eq!(a.caret, s.from + "equal(".len());
        let t = "nx.expect(nx.response.code).";
        let s = query(Lang::JavaScript, t, t.len(), &[], true).unwrap();
        assert!(labels(&s).contains(&"to"));
    }

    #[test]
    fn graphql_offers_schema_names() {
        let words: Vec<VarItem> = ["users", "userById", "User"]
            .iter()
            .map(|n| VarItem {
                name: n.to_string(),
                scope: SCHEMA_SCOPE,
                value: String::new(),
            })
            .collect();
        let t = "query { us";
        let s = query(Lang::Graphql, t, t.len(), &words, false).unwrap();
        let labels: Vec<_> = s.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["users", "userById", "User"]);
        assert_eq!(s.from, t.len() - 2);
        // one character is too little, unless asked for
        assert!(query(Lang::Graphql, "query { u", 9, &words, false).is_none());
        assert!(query(Lang::Graphql, "query { u", 9, &words, true).is_some());
        // schema words never show up after `{{`, and other languages ignore them
        let braces = query(Lang::Graphql, "{{us", 4, &words, false);
        assert!(braces.is_none_or(|s| s.items.iter().all(|i| i.detail == "dynamic")));
        assert!(query(Lang::Json, t, t.len(), &words, false).is_none());
    }
}
