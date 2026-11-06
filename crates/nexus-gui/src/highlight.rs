//! Small hand-written tokenizers (JSON, JS, GraphQL, XML/HTML) -> coloured byte spans / egui
//! `LayoutJob`s. No syntect: the binary stays small and the editor re-lexes only what changed.

use crate::theme::*;
use eframe::egui::{text::LayoutJob, Color32, FontFamily, FontId, TextFormat};

pub fn plain_job(
    text: &str,
    size: f32,
    line_height: f32,
    wrap_width: f32,
    color: Color32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::new(size, FontFamily::Monospace),
            color,
            line_height: Some(line_height),
            ..Default::default()
        },
    );
    job
}

/// Languages the code editor can colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Json,
    JavaScript,
    Graphql,
    Xml,
    Html,
    Text,
}

const JS_KEYWORDS: &[&str] = &[
    "const",
    "let",
    "var",
    "function",
    "return",
    "if",
    "else",
    "for",
    "while",
    "of",
    "in",
    "new",
    "try",
    "catch",
    "finally",
    "throw",
    "async",
    "await",
    "true",
    "false",
    "null",
    "undefined",
    "typeof",
    "break",
    "continue",
    "switch",
    "case",
    "default",
    "class",
    "this",
];
const GQL_KEYWORDS: &[&str] = &[
    "query",
    "mutation",
    "subscription",
    "fragment",
    "on",
    "type",
    "input",
    "enum",
    "interface",
    "union",
    "scalar",
    "schema",
    "extend",
    "directive",
    "true",
    "false",
    "null",
];

/// Coloured byte ranges of `text` for `lang` (the editor builds per-line jobs from these).
pub fn spans(lang: Lang, text: &str) -> Vec<Span> {
    match lang {
        Lang::Json => json_spans(text),
        Lang::Text => Vec::new(),
        Lang::JavaScript => script_spans(text, JS_KEYWORDS, false),
        Lang::Graphql => script_spans(text, GQL_KEYWORDS, true),
        Lang::Xml | Lang::Html => markup_spans(text),
    }
}

fn json_spans(text: &str) -> Vec<Span> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' && b[i] != b'\n' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i < b.len() && b[i] == b'"' {
                    i += 1;
                }
                i = i.min(b.len());
                let mut j = i;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                let c = if j < b.len() && b[j] == b':' {
                    syn_key()
                } else {
                    syn_str()
                };
                out.push((start, boundary(text, i), c));
            }
            b'-' | b'0'..=b'9' => {
                i += 1;
                while i < b.len() && matches!(b[i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
                {
                    i += 1;
                }
                out.push((start, i, syn_num()));
            }
            b't' | b'f' | b'n' => {
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                out.push((start, i, syn_lit()));
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => {
                i += 1;
                out.push((start, i, syn_punct()));
            }
            _ => i += 1,
        }
    }
    out
}

/// Layout job for `text` coloured as `lang`.
pub fn code_job(lang: Lang, text: &str, size: f32, line_height: f32, wrap: f32) -> LayoutJob {
    match lang {
        Lang::Json => spans_job(text, &json_spans(text), size, line_height, wrap),
        Lang::Text => plain_job(text, size, line_height, wrap, fg()),
        Lang::JavaScript => spans_job(
            text,
            &script_spans(text, JS_KEYWORDS, false),
            size,
            line_height,
            wrap,
        ),
        Lang::Graphql => spans_job(
            text,
            &script_spans(text, GQL_KEYWORDS, true),
            size,
            line_height,
            wrap,
        ),
        Lang::Xml | Lang::Html => spans_job(text, &markup_spans(text), size, line_height, wrap),
    }
}

/// `(start, end, colour)` byte range; spans are sorted and disjoint, gaps are plain text.
pub type Span = (usize, usize, Color32);

fn spans_job(text: &str, spans: &[Span], size: f32, line_height: f32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let font = FontId::new(size, FontFamily::Monospace);
    let mut at = 0;
    let push = |job: &mut LayoutJob, a: usize, b: usize, c: Color32| {
        if a < b {
            job.append(
                &text[a..b],
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: c,
                    line_height: Some(line_height),
                    ..Default::default()
                },
            );
        }
    };
    for &(a, b, c) in spans {
        push(&mut job, at, a, fg());
        push(&mut job, a, b, c);
        at = b;
    }
    push(&mut job, at, text.len(), fg());
    job
}

/// Snaps `i` forward to a char boundary.
fn boundary(text: &str, mut i: usize) -> usize {
    i = i.min(text.len());
    while !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn script_spans(text: &str, keywords: &[&str], hash_comments: bool) -> Vec<Span> {
    let b = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                spans.push((start, boundary(text, i), fg_3()));
            }
            b'#' if hash_comments => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                spans.push((start, boundary(text, i), fg_3()));
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
                spans.push((start, boundary(text, i), fg_3()));
            }
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = (i + 1).min(b.len());
                spans.push((start, boundary(text, i), syn_str()));
            }
            b'0'..=b'9' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') {
                    i += 1;
                }
                spans.push((start, i, syn_num()));
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$')
                {
                    i += 1;
                }
                if keywords.contains(&&text[start..i]) {
                    spans.push((start, i, syn_lit()));
                } else if &text[start..i] == "nx" {
                    spans.push((start, i, syn_key()));
                }
            }
            b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b':' => {
                i += 1;
                spans.push((start, i, syn_punct()));
            }
            _ => i += 1,
        }
    }
    spans
}

fn markup_spans(text: &str) -> Vec<Span> {
    let b = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let start = i;
        if text[i..].starts_with("<!--") {
            i = text[i..].find("-->").map_or(b.len(), |e| i + e + 3);
            spans.push((start, i, fg_3()));
            continue;
        }
        // tag: name coloured, quoted attribute values green
        i += 1;
        while i < b.len() && matches!(b[i], b'/' | b'?' | b'!') {
            i += 1;
        }
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
            i += 1;
        }
        spans.push((start, boundary(text, i), syn_key()));
        while i < b.len() && b[i] != b'>' {
            if b[i] == b'"' || b[i] == b'\'' {
                let q = b[i];
                let s = i;
                i += 1;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                i = (i + 1).min(b.len());
                spans.push((s, boundary(text, i), syn_str()));
            } else {
                i += 1;
            }
        }
        if i < b.len() {
            spans.push((i, i + 1, syn_key()));
            i += 1;
        }
    }
    spans.sort_by_key(|s| s.0);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_covers_the_whole_text_without_panicking() {
        let samples = [
            "const a = \"é\"; // c\nnx.test('x', () => {});/* b */ 12.5",
            "<a href=\"x\">é</a><!-- c --><b/>",
            "query { a(id: 1) } # c",
            "{\"k\": [1, true, null, \"é\"]}",
            "unterminated \"string <tag \"x",
        ];
        for lang in [
            Lang::Json,
            Lang::JavaScript,
            Lang::Graphql,
            Lang::Xml,
            Lang::Html,
            Lang::Text,
        ] {
            for s in samples {
                let job = code_job(lang, s, 13.0, 20.0, 400.0);
                assert_eq!(job.text, s, "{lang:?}");
            }
        }
    }
}
