//! A light parser for the SDL that `graphql_introspect` returns: enough structure (types, their
//! fields / values) to browse and search the schema. It understands what the core printer emits.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Object,
    Interface,
    Input,
    Enum,
    Union,
    Scalar,
    Directive,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Object => "type",
            Kind::Interface => "interface",
            Kind::Input => "input",
            Kind::Enum => "enum",
            Kind::Union => "union",
            Kind::Scalar => "scalar",
            Kind::Directive => "directive",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    /// `(id: ID!, first: Int)` as printed, empty when the field has no arguments.
    pub args: String,
    /// Result type (or enum value suffix such as a directive).
    pub ty: String,
    pub description: String,
    pub deprecated: bool,
}

impl Field {
    /// `user(id: ID!): User`
    pub fn signature(&self) -> String {
        if self.ty.is_empty() {
            format!("{}{}", self.name, self.args)
        } else {
            format!("{}{}: {}", self.name, self.args, self.ty)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDef {
    pub kind: Kind,
    pub name: String,
    pub description: String,
    /// `implements A & B` / union members, as printed.
    pub detail: String,
    pub fields: Vec<Field>,
    /// The type's SDL block.
    pub sdl: String,
}

#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub types: Vec<TypeDef>,
    /// Names of the root types: query, mutation, subscription.
    pub roots: Vec<String>,
}

impl Schema {
    /// Types whose name or one of whose fields contains `q` (case-insensitive); all when empty.
    pub fn search(&self, q: &str) -> Vec<usize> {
        let q = q.trim().to_lowercase();
        self.types
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                q.is_empty()
                    || t.name.to_lowercase().contains(&q)
                    || t.fields.iter().any(|f| f.name.to_lowercase().contains(&q))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Every distinct field / argument / type name, for autocomplete.
    pub fn words(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in &self.types {
            if t.kind != Kind::Directive {
                out.push(t.name.clone());
            }
            if matches!(t.kind, Kind::Object | Kind::Interface | Kind::Input) {
                out.extend(t.fields.iter().map(|f| f.name.clone()));
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Parses printed SDL. Unknown constructs are skipped.
pub fn parse_sdl(sdl: &str) -> Schema {
    let mut schema = Schema::default();
    let lines: Vec<&str> = sdl.lines().collect();
    let mut i = 0;
    let mut desc = String::new();
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        // top-level description (`"""text"""` or a multi-line block)
        if line.starts_with("\"\"\"") {
            let (text, next) = read_description(&lines, i);
            desc = text;
            i = next;
            continue;
        }
        let start = i;
        let header = t.trim_end_matches('{').trim();
        let (kind, rest) = match header.split_once(' ') {
            Some(("type", r)) => (Some(Kind::Object), r),
            Some(("interface", r)) => (Some(Kind::Interface), r),
            Some(("input", r)) => (Some(Kind::Input), r),
            Some(("enum", r)) => (Some(Kind::Enum), r),
            Some(("union", r)) => (Some(Kind::Union), r),
            Some(("scalar", r)) => (Some(Kind::Scalar), r),
            Some(("directive", r)) => (Some(Kind::Directive), r),
            _ => (None, ""),
        };
        let has_body = t.ends_with('{');
        i += 1;
        let mut body: Vec<&str> = Vec::new();
        if has_body {
            while i < lines.len() && lines[i] != "}" {
                body.push(lines[i]);
                i += 1;
            }
            i += 1; // the closing brace
        }
        let Some(kind) = kind else {
            if header == "schema" {
                for b in &body {
                    if let Some((_, n)) = b.trim().split_once(':') {
                        schema.roots.push(n.trim().to_string());
                    }
                }
            }
            desc.clear();
            continue;
        };
        let sdl_block = lines[start..i.min(lines.len())].join("\n");
        let (name, detail) = split_header(kind, rest);
        let fields = match kind {
            Kind::Object | Kind::Interface | Kind::Input => parse_fields(&body),
            Kind::Enum => parse_enum(&body),
            _ => Vec::new(),
        };
        schema.types.push(TypeDef {
            kind,
            name,
            description: std::mem::take(&mut desc),
            detail,
            fields,
            sdl: sdl_block,
        });
    }
    // root types: default names when there was no `schema { }` block
    if schema.roots.is_empty() {
        for n in ["Query", "Mutation", "Subscription"] {
            if schema.types.iter().any(|t| t.name == n) {
                schema.roots.push(n.to_string());
            }
        }
    }
    schema
}

fn split_header(kind: Kind, rest: &str) -> (String, String) {
    match kind {
        Kind::Union => match rest.split_once('=') {
            Some((n, m)) => (n.trim().to_string(), m.trim().to_string()),
            None => (rest.trim().to_string(), String::new()),
        },
        Kind::Directive => {
            let name = rest
                .trim_start_matches('@')
                .split(|c: char| c == '(' || c.is_whitespace())
                .next()
                .unwrap_or("");
            (format!("@{name}"), rest.to_string())
        }
        _ => match rest.split_once(' ') {
            Some((n, d)) => (n.to_string(), d.trim().to_string()),
            None => (rest.trim().to_string(), String::new()),
        },
    }
}

/// Reads a description starting at `lines[i]`; returns the text and the next line index.
fn read_description(lines: &[&str], i: usize) -> (String, usize) {
    let first = lines[i].trim();
    let rest = &first[3..];
    if let Some(end) = rest.find("\"\"\"") {
        return (rest[..end].to_string(), i + 1);
    }
    let mut out: Vec<String> = Vec::new();
    if !rest.trim().is_empty() {
        out.push(rest.trim().to_string());
    }
    let mut j = i + 1;
    while j < lines.len() {
        let t = lines[j].trim();
        if t == "\"\"\"" {
            return (out.join("\n"), j + 1);
        }
        out.push(t.to_string());
        j += 1;
    }
    (out.join("\n"), j)
}

fn parse_fields(body: &[&str]) -> Vec<Field> {
    let mut out = Vec::new();
    let mut desc = String::new();
    let mut i = 0;
    while i < body.len() {
        let t = body[i].trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        if t.starts_with("\"\"\"") {
            let lines: Vec<&str> = body.iter().map(|l| l.trim()).collect();
            let (d, next) = read_description(&lines, i);
            desc = d;
            i = next;
            continue;
        }
        // a field whose arguments span several lines: `name(` ... `): Type`
        let mut text = t.to_string();
        if t.ends_with('(') {
            let mut depth = 1;
            i += 1;
            while i < body.len() && depth > 0 {
                let l = body[i].trim();
                if l.starts_with("\"\"\"") {
                    // skip argument descriptions
                    let lines: Vec<&str> = body.iter().map(|l| l.trim()).collect();
                    let (_, next) = read_description(&lines, i);
                    i = next;
                    continue;
                }
                if l.starts_with(')') {
                    depth -= 1;
                }
                if !l.is_empty() {
                    if depth > 0 {
                        text.push_str(if text.ends_with('(') { "" } else { ", " });
                    }
                    text.push_str(l);
                }
                i += 1;
            }
        } else {
            i += 1;
        }
        let deprecated = text.contains("@deprecated");
        let text = text.split(" @deprecated").next().unwrap_or("").to_string();
        let (head, ty) = split_field(&text);
        let (name, args) = match head.find('(') {
            Some(p) => (head[..p].to_string(), head[p..].to_string()),
            None => (head.to_string(), String::new()),
        };
        out.push(Field {
            name,
            args,
            ty,
            description: std::mem::take(&mut desc),
            deprecated,
        });
    }
    out
}

/// Splits `name(args): Type = default` at the colon that is not inside the parentheses.
fn split_field(text: &str) -> (String, String) {
    let mut depth = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ':' if depth == 0 => {
                return (
                    text[..i].trim().to_string(),
                    text[i + 1..].trim().to_string(),
                )
            }
            _ => {}
        }
    }
    (text.trim().to_string(), String::new())
}

fn parse_enum(body: &[&str]) -> Vec<Field> {
    body.iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('"'))
        .map(|l| Field {
            name: l.split(' ').next().unwrap_or("").to_string(),
            deprecated: l.contains("@deprecated"),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SDL: &str = r#""""Requires a role"""
directive @auth(role: Role = ADMIN) on FIELD_DEFINITION | OBJECT

type Query {
  user(id: ID!): User
  users: [User!]!
}

"""A person"""
type User implements Node {
  id: ID!

  """The display name"""
  name: String
  legacy: String @deprecated(reason: "Use name")
}

interface Node {
  id: ID!
}

enum Role {
  ADMIN
  GUEST @deprecated
}

input Filter {
  limit: Int = 10
}

union Result = User | Query

scalar DateTime

"""
a long description
"""
type Subscription {
  tick: Int
  big(
    """first"""
    a: Int

    """
    line1
    line2
    """
    b: Int
  ): Int
}"#;

    #[test]
    fn parses_types_fields_and_descriptions() {
        let s = parse_sdl(SDL);
        let names: Vec<_> = s.types.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "@auth",
                "Query",
                "User",
                "Node",
                "Role",
                "Filter",
                "Result",
                "DateTime",
                "Subscription"
            ]
        );
        let q = &s.types[1];
        assert_eq!(q.kind, Kind::Object);
        assert_eq!(q.fields[0].signature(), "user(id: ID!): User");
        assert_eq!(q.fields[1].signature(), "users: [User!]!");
        let u = &s.types[2];
        assert_eq!(u.detail, "implements Node");
        assert_eq!(u.description, "A person");
        assert_eq!(u.fields[1].description, "The display name");
        assert!(u.fields[2].deprecated);
        assert_eq!(u.fields[2].ty, "String");
        assert_eq!(s.types[4].fields.len(), 2);
        assert!(s.types[4].fields[1].deprecated);
        assert_eq!(s.types[5].fields[0].ty, "Int = 10");
        assert_eq!(s.types[6].detail, "User | Query");
        assert_eq!(s.types[7].kind, Kind::Scalar);
        let sub = &s.types[8];
        assert_eq!(sub.description, "a long description");
        assert_eq!(sub.fields[1].name, "big");
        assert_eq!(sub.fields[1].args, "(a: Int, b: Int)");
        assert_eq!(sub.fields[1].ty, "Int");
        assert_eq!(s.roots, ["Query", "Subscription"]);
        assert!(u.sdl.starts_with("type User implements Node {"));
        assert!(u.sdl.ends_with('}'));
    }

    #[test]
    fn search_and_words() {
        let s = parse_sdl(SDL);
        let hits: Vec<_> = s
            .search("legacy")
            .into_iter()
            .map(|i| s.types[i].name.as_str())
            .collect();
        assert_eq!(hits, ["User"]);
        assert_eq!(s.search("").len(), s.types.len());
        let w = s.words();
        assert!(w.contains(&"users".to_string()));
        assert!(w.contains(&"tick".to_string()));
        assert!(!w.contains(&"@auth".to_string()));
    }

    #[test]
    fn schema_block_sets_roots() {
        let s = parse_sdl("schema {\n  query: RootQuery\n  mutation: RootMutation\n}\n\ntype RootQuery {\n  a: Int\n}");
        assert_eq!(s.roots, ["RootQuery", "RootMutation"]);
        assert_eq!(s.types.len(), 1);
    }
}
