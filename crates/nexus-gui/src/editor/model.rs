//! The text-editing model behind the code editor: selection, line index, undo/redo and every
//! editing command (auto-indent, bracket pairing, indentation, comment toggling, word motion).
//! Pure `String` + byte offsets, no egui, so it is unit tested directly.

use crate::highlight::Lang;
use std::ops::Range;

pub const INDENT: &str = "  ";
const UNDO_LIMIT: usize = 500;
const MERGE_LIMIT: usize = 4000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Typing: consecutive inserts merge into one undo step.
    Type,
    /// Backspace/delete: consecutive deletions merge.
    Delete,
    Other,
}

#[derive(Clone, Debug)]
struct Change {
    at: usize,
    removed: String,
    inserted: String,
    /// (anchor, cursor) before and after.
    before: (usize, usize),
    after: (usize, usize),
    kind: Kind,
}

#[derive(Default)]
pub struct Model {
    pub anchor: usize,
    pub cursor: usize,
    lines: Vec<usize>,
    undo: Vec<Change>,
    redo: Vec<Change>,
    /// Bumped on every text change.
    pub rev: u64,
    /// Column (in chars) vertical motion tries to keep.
    goal_col: Option<usize>,
}

/// Which characters are auto-closed in `lang`: `(open, close)`.
pub fn pairs(lang: Lang) -> &'static [(char, char)] {
    match lang {
        Lang::Json | Lang::Graphql => &[('(', ')'), ('[', ']'), ('{', '}'), ('"', '"')],
        Lang::JavaScript => &[
            ('(', ')'),
            ('[', ']'),
            ('{', '}'),
            ('"', '"'),
            ('\'', '\''),
            ('`', '`'),
        ],
        Lang::Xml | Lang::Html | Lang::Text => &[],
    }
}

/// The line-comment token for `lang`, if it has one.
pub fn comment_token(lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::JavaScript => Some("//"),
        Lang::Graphql => Some("#"),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punct,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

pub fn is_word_char(c: char) -> bool {
    class(c) == Class::Word
}

impl Model {
    pub fn new(text: &str) -> Self {
        let mut m = Self::default();
        m.rebuild(text);
        m
    }

    /// The text changed behind the model's back (new tab, snippet inserted, ...).
    pub fn reset(&mut self, text: &str) {
        self.rebuild(text);
        self.rev += 1;
        self.undo.clear();
        self.redo.clear();
        self.anchor = floor_boundary(text, self.anchor.min(text.len()));
        self.cursor = floor_boundary(text, self.cursor.min(text.len()));
        self.goal_col = None;
    }

    fn rebuild(&mut self, text: &str) {
        self.lines.clear();
        self.lines.push(0);
        self.lines
            .extend(text.match_indices('\n').map(|(i, _)| i + 1));
    }

    // ---- lines ----

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
    pub fn line_start(&self, i: usize) -> usize {
        self.lines[i.min(self.lines.len() - 1)]
    }
    /// End of line `i` (before its `\n`).
    pub fn line_end(&self, text: &str, i: usize) -> usize {
        if i + 1 < self.lines.len() {
            self.lines[i + 1] - 1
        } else {
            text.len()
        }
    }
    pub fn line_range(&self, text: &str, i: usize) -> Range<usize> {
        self.line_start(i)..self.line_end(text, i)
    }
    pub fn line_of(&self, off: usize) -> usize {
        self.lines.partition_point(|&s| s <= off).saturating_sub(1)
    }
    pub fn line_text<'a>(&self, text: &'a str, i: usize) -> &'a str {
        &text[self.line_range(text, i)]
    }

    // ---- selection ----

    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }
    pub fn has_selection(&self) -> bool {
        self.anchor != self.cursor
    }
    pub fn selected<'a>(&self, text: &'a str) -> &'a str {
        &text[self.selection()]
    }
    pub fn select_all(&mut self, text: &str) {
        self.anchor = 0;
        self.cursor = text.len();
        self.goal_col = None;
    }
    /// Moves the caret (and the anchor unless `extend`).
    pub fn set_cursor(&mut self, pos: usize, extend: bool) {
        self.cursor = pos;
        if !extend {
            self.anchor = pos;
        }
        self.goal_col = None;
    }
    pub fn select_range(&mut self, a: usize, b: usize) {
        self.anchor = a;
        self.cursor = b;
        self.goal_col = None;
    }

    /// Column of `off` in chars from its line start.
    pub fn col_of(&self, text: &str, off: usize) -> usize {
        let ls = self.line_start(self.line_of(off));
        text[ls..off].chars().count()
    }

    // ---- motion ----

    pub fn move_h(&mut self, text: &str, forward: bool, word: bool, extend: bool) {
        if self.has_selection() && !extend {
            let r = self.selection();
            self.set_cursor(if forward { r.end } else { r.start }, false);
            return;
        }
        let to = if word {
            if forward {
                word_right(text, self.cursor)
            } else {
                word_left(text, self.cursor)
            }
        } else if forward {
            next_boundary(text, self.cursor)
        } else {
            prev_boundary(text, self.cursor)
        };
        self.set_cursor(to, extend);
    }

    pub fn move_v(&mut self, text: &str, lines: isize, extend: bool) {
        let goal = self
            .goal_col
            .unwrap_or_else(|| self.col_of(text, self.cursor));
        let cur = self.line_of(self.cursor) as isize;
        let target = cur + lines;
        let to = if target < 0 {
            0
        } else if target as usize >= self.line_count() {
            text.len()
        } else {
            let r = self.line_range(text, target as usize);
            let line = &text[r.clone()];
            let byte = line.char_indices().nth(goal).map_or(line.len(), |(i, _)| i);
            r.start + byte
        };
        self.cursor = to;
        if !extend {
            self.anchor = to;
        }
        self.goal_col = Some(goal);
    }

    /// Home: first non-blank, then column 0 on the second press.
    pub fn home(&mut self, text: &str, doc: bool, extend: bool) {
        if doc {
            return self.set_cursor(0, extend);
        }
        let i = self.line_of(self.cursor);
        let r = self.line_range(text, i);
        let first = r.start
            + text[r.clone()]
                .find(|c: char| !c.is_whitespace() || c == '\n')
                .unwrap_or(r.len());
        self.set_cursor(if self.cursor == first { r.start } else { first }, extend);
    }

    pub fn end(&mut self, text: &str, doc: bool, extend: bool) {
        if doc {
            return self.set_cursor(text.len(), extend);
        }
        let i = self.line_of(self.cursor);
        self.set_cursor(self.line_end(text, i), extend);
    }

    /// Selects the word (or the run of punctuation / blanks) around `pos`.
    pub fn select_word_at(&mut self, text: &str, pos: usize) {
        let pos = pos.min(text.len());
        let before = text[..pos].chars().next_back();
        let after = text[pos..].chars().next();
        let probe = after.or(before);
        let Some(c) = probe else {
            return self.set_cursor(pos, false);
        };
        let cl = class(if after.is_some_and(|a| class(a) != Class::Space) {
            after.unwrap()
        } else {
            before.unwrap_or(c)
        });
        let mut s = pos;
        while let Some(ch) = text[..s].chars().next_back() {
            if class(ch) != cl {
                break;
            }
            s -= ch.len_utf8();
        }
        let mut e = pos;
        while let Some(ch) = text[e..].chars().next() {
            if class(ch) != cl {
                break;
            }
            e += ch.len_utf8();
        }
        self.select_range(s, e);
    }

    pub fn select_line_at(&mut self, text: &str, pos: usize) {
        let i = self.line_of(pos);
        let r = self.line_range(text, i);
        let end = if r.end < text.len() { r.end + 1 } else { r.end };
        self.select_range(r.start, end);
    }

    // ---- primitive edit ----

    /// Replaces `range` with `ins`; the caret lands after the insertion unless `sel_after`
    /// says otherwise.
    pub fn replace(
        &mut self,
        text: &mut String,
        range: Range<usize>,
        ins: &str,
        kind: Kind,
        sel_after: Option<(usize, usize)>,
    ) {
        let before = (self.anchor, self.cursor);
        let removed: String = text[range.clone()].to_string();
        text.replace_range(range.clone(), ins);
        let end = range.start + ins.len();
        let after = sel_after.unwrap_or((end, end));
        self.anchor = after.0;
        self.cursor = after.1;
        self.goal_col = None;
        self.rebuild(text);
        self.rev += 1;
        self.redo.clear();
        let change = Change {
            at: range.start,
            removed,
            inserted: ins.to_string(),
            before,
            after,
            kind,
        };
        self.push_change(change);
    }

    fn push_change(&mut self, c: Change) {
        if let Some(last) = self.undo.last_mut() {
            if c.kind == last.kind && c.kind != Kind::Other {
                match c.kind {
                    Kind::Type
                        if last.removed.is_empty()
                            && c.removed.is_empty()
                            && last.at + last.inserted.len() == c.at
                            && !c.inserted.contains('\n')
                            && last.inserted.len() < MERGE_LIMIT =>
                    {
                        last.inserted.push_str(&c.inserted);
                        last.after = c.after;
                        return;
                    }
                    Kind::Delete if last.inserted.is_empty() && c.inserted.is_empty() => {
                        if c.at + c.removed.len() == last.at {
                            // backspace run
                            last.removed.insert_str(0, &c.removed);
                            last.at = c.at;
                            last.after = c.after;
                            return;
                        } else if c.at == last.at {
                            // forward-delete run
                            last.removed.push_str(&c.removed);
                            last.after = c.after;
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.undo.push(c);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self, text: &mut String) -> bool {
        let Some(c) = self.undo.pop() else {
            return false;
        };
        text.replace_range(c.at..c.at + c.inserted.len(), &c.removed);
        (self.anchor, self.cursor) = c.before;
        self.rebuild(text);
        self.rev += 1;
        self.goal_col = None;
        self.redo.push(c);
        true
    }

    pub fn redo(&mut self, text: &mut String) -> bool {
        let Some(c) = self.redo.pop() else {
            return false;
        };
        text.replace_range(c.at..c.at + c.removed.len(), &c.inserted);
        (self.anchor, self.cursor) = c.after;
        self.rebuild(text);
        self.rev += 1;
        self.goal_col = None;
        self.undo.push(c);
        true
    }

    // ---- commands ----

    /// Types `s` (a pasted/IME string goes in verbatim, a single char gets the bracket logic).
    pub fn insert_text(&mut self, text: &mut String, lang: Lang, s: &str) {
        let mut it = s.chars();
        let single = match (it.next(), it.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        };
        let Some(c) = single else {
            let r = self.selection();
            return self.replace(text, r, s, Kind::Other, None);
        };
        let table = pairs(lang);
        let r = self.selection();
        let next = text[r.end..].chars().next();
        let prev = text[..r.start].chars().next_back();
        // wrap a selection in the pair
        if self.has_selection() {
            if let Some(&(o, cl)) = table.iter().find(|(o, _)| *o == c) {
                let inner = text[r.clone()].to_string();
                let ins = format!("{o}{inner}{cl}");
                let a = r.start + o.len_utf8();
                return self.replace(text, r, &ins, Kind::Other, Some((a, a + inner.len())));
            }
        }
        // typing the closer in front of an identical one just steps over it
        if next == Some(c) && table.iter().any(|(_, cl)| *cl == c) && !self.has_selection() {
            return self.set_cursor(self.cursor + c.len_utf8(), false);
        }
        if let Some(&(o, cl)) = table.iter().find(|(o, _)| *o == c) {
            let quote = o == cl;
            let next_ok = next.is_none_or(|n| n.is_whitespace() || ")]}:;>,".contains(n));
            let prev_ok = !quote || !prev.is_some_and(|p| is_word_char(p) || p == o);
            if next_ok && prev_ok {
                let ins = format!("{o}{cl}");
                let at = r.start + o.len_utf8();
                return self.replace(text, r, &ins, Kind::Other, Some((at, at)));
            }
        }
        let mut buf = [0u8; 4];
        self.replace(text, r, c.encode_utf8(&mut buf), Kind::Type, None);
    }

    /// Enter: keeps the indentation and opens a block between `{}` / `[]` / `()` / tags.
    pub fn newline(&mut self, text: &mut String, lang: Lang) {
        let r = self.selection();
        let li = self.line_of(r.start);
        let ls = self.line_start(li);
        let line_before = &text[ls..r.start];
        let indent: String = line_before
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let before = text[..r.start].chars().next_back();
        let after = text[r.end..].chars().next();
        let opens = match (before, after) {
            (Some(b @ ('{' | '[' | '(')), a) => Some((b, a)),
            (Some('>'), Some('<')) if matches!(lang, Lang::Xml | Lang::Html) => {
                Some(('>', Some('<')))
            }
            _ => None,
        };
        if let Some((b, a)) = opens {
            let closes = match b {
                '{' => a == Some('}'),
                '[' => a == Some(']'),
                '(' => a == Some(')'),
                _ => text[r.end..].starts_with("</"),
            };
            let inner = format!("\n{indent}{INDENT}");
            if closes {
                let ins = format!("{inner}\n{indent}");
                let at = r.start + inner.len();
                return self.replace(text, r, &ins, Kind::Other, Some((at, at)));
            }
            if b != '>' {
                return self.replace(text, r, &inner, Kind::Other, None);
            }
        }
        let ins = format!("\n{indent}");
        self.replace(text, r, &ins, Kind::Other, None);
    }

    /// Tab / Shift+Tab. A selection over several lines indents the lines; otherwise Tab
    /// inserts two spaces and Shift+Tab outdents the current line.
    pub fn tab(&mut self, text: &mut String, shift: bool) {
        let r = self.selection();
        let multi = text[r.clone()].contains('\n');
        if !shift && !multi {
            return self.replace(text, r, INDENT, Kind::Other, None);
        }
        let (first, last) = self.touched_lines(text);
        let mut edits = Vec::new();
        for i in first..=last {
            let ls = self.line_start(i);
            let line = self.line_text(text, i);
            if shift {
                let k = if line.starts_with('\t') {
                    1
                } else {
                    line.chars()
                        .take(INDENT.len())
                        .take_while(|c| *c == ' ')
                        .count()
                };
                if k > 0 {
                    edits.push((ls, k, String::new()));
                }
            } else if !line.is_empty() {
                edits.push((ls, 0, INDENT.to_string()));
            }
        }
        self.apply_edits(text, edits);
    }

    /// Lines a block command applies to: the selection's lines, minus a last line the selection
    /// only reaches the start of.
    fn touched_lines(&self, text: &str) -> (usize, usize) {
        let r = self.selection();
        let first = self.line_of(r.start);
        let mut last = self.line_of(r.end);
        if last > first && r.end == self.line_start(last) && r.end <= text.len() {
            last -= 1;
        }
        (first, last)
    }

    /// Applies non-overlapping `(pos, removed_len, inserted)` edits as one undo step and maps
    /// the selection through them.
    pub fn apply_edits(&mut self, text: &mut String, mut edits: Vec<(usize, usize, String)>) {
        if edits.is_empty() {
            return;
        }
        edits.sort_by_key(|e| e.0);
        let lo = edits[0].0;
        let hi = edits.iter().map(|e| e.0 + e.1).max().unwrap_or(lo);
        let mut region = String::new();
        let mut at = lo;
        for (pos, rem, ins) in &edits {
            region.push_str(&text[at..*pos]);
            region.push_str(ins);
            at = pos + rem;
        }
        region.push_str(&text[at..hi]);
        // `after`: an insertion exactly at `o` pushes `o` past it (a selection start stays put so
        // the indentation is part of the selection).
        let map = |o: usize, after: bool| -> usize {
            let mut delta: isize = 0;
            for (pos, rem, ins) in &edits {
                if o >= pos + rem && (*rem > 0 || o > *pos || (o == *pos && after)) {
                    delta += ins.len() as isize - *rem as isize;
                } else if o > *pos && o < pos + rem {
                    return (*pos as isize + delta) as usize;
                }
            }
            (o as isize + delta) as usize
        };
        let sel = if self.anchor <= self.cursor {
            let collapsed = self.anchor == self.cursor;
            (map(self.anchor, collapsed), map(self.cursor, true))
        } else {
            (map(self.anchor, true), map(self.cursor, false))
        };
        self.replace(text, lo..hi, &region, Kind::Other, Some(sel));
    }

    /// Ctrl+/: toggles the line-comment token on the selected lines.
    pub fn toggle_comment(&mut self, text: &mut String, lang: Lang) {
        let Some(tok) = comment_token(lang) else {
            return;
        };
        let (first, last) = self.touched_lines(text);
        let lines: Vec<(usize, &str)> = (first..=last)
            .map(|i| (self.line_start(i), self.line_text(text, i)))
            .collect();
        let indent_of = |l: &str| l.len() - l.trim_start_matches([' ', '\t']).len();
        let mut targets: Vec<&(usize, &str)> =
            lines.iter().filter(|(_, l)| !l.trim().is_empty()).collect();
        if targets.is_empty() {
            targets = lines.iter().collect();
        }
        let all_commented = targets
            .iter()
            .all(|(_, l)| l[indent_of(l)..].starts_with(tok));
        let mut edits = Vec::new();
        if all_commented {
            for (ls, l) in &targets {
                let at = indent_of(l);
                let mut n = tok.len();
                if l[at + n..].starts_with(' ') {
                    n += 1;
                }
                edits.push((ls + at, n, String::new()));
            }
        } else {
            let col = targets.iter().map(|(_, l)| indent_of(l)).min().unwrap_or(0);
            for (ls, _) in &targets {
                edits.push((ls + col, 0, format!("{tok} ")));
            }
        }
        self.apply_edits(text, edits);
    }

    pub fn backspace(&mut self, text: &mut String, lang: Lang, word: bool) {
        if self.has_selection() {
            let r = self.selection();
            return self.replace(text, r, "", Kind::Delete, None);
        }
        let c = self.cursor;
        if c == 0 {
            return;
        }
        // delete an empty pair like `()` / `""` in one go
        if !word {
            let p = text[..c].chars().next_back();
            let n = text[c..].chars().next();
            if let (Some(p), Some(n)) = (p, n) {
                if pairs(lang).contains(&(p, n)) {
                    let range = c - p.len_utf8()..c + n.len_utf8();
                    return self.replace(text, range, "", Kind::Delete, None);
                }
            }
            // in leading whitespace, drop one indent unit
            let ls = self.line_start(self.line_of(c));
            let lead = &text[ls..c];
            if lead.len() >= INDENT.len()
                && lead.chars().all(|ch| ch == ' ')
                && lead.len().is_multiple_of(INDENT.len())
            {
                return self.replace(text, c - INDENT.len()..c, "", Kind::Delete, None);
            }
        }
        let to = if word {
            word_left(text, c)
        } else {
            prev_boundary(text, c)
        };
        self.replace(text, to..c, "", Kind::Delete, None);
    }

    pub fn delete(&mut self, text: &mut String, word: bool) {
        if self.has_selection() {
            let r = self.selection();
            return self.replace(text, r, "", Kind::Delete, None);
        }
        let c = self.cursor;
        if c >= text.len() {
            return;
        }
        let to = if word {
            word_right(text, c)
        } else {
            next_boundary(text, c)
        };
        self.replace(text, c..to, "", Kind::Delete, None);
    }
}

fn floor_boundary(text: &str, mut i: usize) -> usize {
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn prev_boundary(text: &str, i: usize) -> usize {
    text[..i]
        .chars()
        .next_back()
        .map_or(0, |c| i - c.len_utf8())
}

pub fn next_boundary(text: &str, i: usize) -> usize {
    text[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

pub fn word_left(text: &str, i: usize) -> usize {
    let mut p = i;
    while let Some(c) = text[..p].chars().next_back() {
        if class(c) != Class::Space {
            break;
        }
        p -= c.len_utf8();
    }
    let Some(first) = text[..p].chars().next_back() else {
        return p;
    };
    let cl = class(first);
    while let Some(c) = text[..p].chars().next_back() {
        if class(c) != cl {
            break;
        }
        p -= c.len_utf8();
    }
    p
}

pub fn word_right(text: &str, i: usize) -> usize {
    let mut p = i;
    if let Some(first) = text[p..].chars().next() {
        let cl = class(first);
        if cl != Class::Space {
            while let Some(c) = text[p..].chars().next() {
                if class(c) != cl {
                    break;
                }
                p += c.len_utf8();
            }
        }
    }
    while let Some(c) = text[p..].chars().next() {
        if class(c) != Class::Space {
            break;
        }
        p += c.len_utf8();
    }
    p
}

/// The bracket pairing with the one at `pos` (or just before it), scanning at most `limit`
/// bytes. `skip(offset)` says an offset is inside a string/comment and must be ignored.
/// Returns `(this, other)` byte offsets of the two brackets.
pub fn match_bracket(
    text: &str,
    pos: usize,
    limit: usize,
    skip: &dyn Fn(usize) -> bool,
) -> Option<(usize, usize)> {
    let b = text.as_bytes();
    let at = |i: usize| b.get(i).copied();
    let is_br = |c: u8| b"()[]{}".contains(&c);
    let here = if at(pos).is_some_and(is_br) && !skip(pos) {
        pos
    } else if pos > 0 && at(pos - 1).is_some_and(is_br) && !skip(pos - 1) {
        pos - 1
    } else {
        return None;
    };
    let c = b[here];
    let (open, close, forward) = match c {
        b'(' => (b'(', b')', true),
        b'[' => (b'[', b']', true),
        b'{' => (b'{', b'}', true),
        b')' => (b'(', b')', false),
        b']' => (b'[', b']', false),
        _ => (b'{', b'}', false),
    };
    let mut depth = 0i32;
    if forward {
        let end = (here + limit).min(b.len());
        #[allow(clippy::needless_range_loop)]
        for i in here..end {
            if skip(i) {
                continue;
            }
            if b[i] == open {
                depth += 1;
            } else if b[i] == close {
                depth -= 1;
                if depth == 0 {
                    return Some((here, i));
                }
            }
        }
    } else {
        let start = here.saturating_sub(limit);
        for i in (start..=here).rev() {
            if skip(i) {
                continue;
            }
            if b[i] == close {
                depth += 1;
            } else if b[i] == open {
                depth -= 1;
                if depth == 0 {
                    return Some((here, i));
                }
            }
        }
    }
    None
}

/// Case-insensitive (ASCII) non-overlapping matches of `needle` as byte ranges.
pub fn find_all(text: &str, needle: &str, limit: usize) -> Vec<Range<usize>> {
    if needle.is_empty() {
        return Vec::new();
    }
    let n = needle.to_ascii_lowercase();
    text.to_ascii_lowercase()
        .match_indices(&n)
        .take(limit)
        .map(|(i, m)| i..i + m.len())
        .collect()
}

/// Pretty-prints JSON with two-space indentation; `None` if `text` is not valid JSON.
pub fn format_json(text: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    serde_json::to_string_pretty(&v).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ed {
        m: Model,
        t: String,
        lang: Lang,
    }

    impl Ed {
        /// `|` marks the caret.
        fn new(lang: Lang, src: &str) -> Self {
            let pos = src.find('|').unwrap_or(0);
            let t = src.replacen('|', "", 1);
            let mut m = Model::new(&t);
            m.set_cursor(pos, false);
            Self { m, t, lang }
        }
        fn type_str(&mut self, s: &str) {
            for c in s.chars() {
                if c == '\n' {
                    self.m.newline(&mut self.t, self.lang);
                } else {
                    let mut b = [0; 4];
                    self.m
                        .insert_text(&mut self.t, self.lang, c.encode_utf8(&mut b));
                }
            }
        }
        fn show(&self) -> String {
            let r = self.m.selection();
            if r.is_empty() {
                format!("{}|{}", &self.t[..r.start], &self.t[r.start..])
            } else {
                format!(
                    "{}[{}]{}",
                    &self.t[..r.start],
                    &self.t[r.clone()],
                    &self.t[r.end..]
                )
            }
        }
    }

    #[test]
    fn auto_close_and_overtype() {
        let mut e = Ed::new(Lang::Json, "|");
        e.type_str("{");
        assert_eq!(e.show(), "{|}");
        e.type_str("\"a");
        assert_eq!(e.show(), "{\"a|\"}");
        e.type_str("\"");
        assert_eq!(e.show(), "{\"a\"|}");
        e.type_str(":[");
        assert_eq!(e.show(), "{\"a\":[|]}");
        e.type_str("]");
        assert_eq!(e.show(), "{\"a\":[]|}");
    }

    #[test]
    fn quote_not_closed_after_a_word_and_not_in_text() {
        let mut e = Ed::new(Lang::JavaScript, "don|");
        e.type_str("'");
        assert_eq!(e.show(), "don'|");
        let mut e = Ed::new(Lang::Text, "|");
        e.type_str("(\"");
        assert_eq!(e.show(), "(\"|");
        // no pair when a word follows
        let mut e = Ed::new(Lang::Json, "|abc");
        e.type_str("{");
        assert_eq!(e.show(), "{|abc");
    }

    #[test]
    fn typing_an_open_bracket_wraps_the_selection() {
        let mut e = Ed::new(Lang::Json, "x|");
        e.m.select_range(0, 1);
        e.type_str("[");
        assert_eq!(e.t, "[x]");
    }

    #[test]
    fn backspace_removes_an_empty_pair_and_indent_units() {
        let mut e = Ed::new(Lang::Json, "(|)");
        e.m.backspace(&mut e.t, e.lang, false);
        assert_eq!(e.show(), "|");
        let mut e = Ed::new(Lang::Json, "    |x");
        e.m.backspace(&mut e.t, e.lang, false);
        assert_eq!(e.show(), "  |x");
        let mut e = Ed::new(Lang::Json, "ab cd|");
        e.m.backspace(&mut e.t, e.lang, true);
        assert_eq!(e.show(), "ab |");
    }

    #[test]
    fn enter_keeps_indent_and_opens_blocks() {
        let mut e = Ed::new(Lang::Json, "  foo|");
        e.type_str("\n");
        assert_eq!(e.show(), "  foo\n  |");
        let mut e = Ed::new(Lang::JavaScript, "if (x) {|}");
        e.type_str("\n");
        assert_eq!(e.show(), "if (x) {\n  |\n}");
        let mut e = Ed::new(Lang::JavaScript, "  f({|})");
        e.type_str("\n");
        assert_eq!(e.show(), "  f({\n    |\n  })");
        let mut e = Ed::new(Lang::Json, "[|");
        e.type_str("\n");
        assert_eq!(e.show(), "[\n  |");
        let mut e = Ed::new(Lang::Xml, "<a>|</a>");
        e.type_str("\n");
        assert_eq!(e.show(), "<a>\n  |\n</a>");
    }

    #[test]
    fn tab_inserts_two_spaces_and_indents_blocks() {
        let mut e = Ed::new(Lang::Json, "a|");
        e.m.tab(&mut e.t, false);
        assert_eq!(e.show(), "a  |");
        let mut e = Ed::new(Lang::Json, "a\n\nb");
        e.m.select_all(&e.t);
        e.m.tab(&mut e.t, false);
        assert_eq!(e.t, "  a\n\n  b");
        e.m.tab(&mut e.t, true);
        assert_eq!(e.t, "a\n\nb");
        // outdent is a no-op when there is nothing to remove, and removes at most one unit
        e.m.tab(&mut e.t, true);
        assert_eq!(e.t, "a\n\nb");
        let mut e = Ed::new(Lang::Json, "    x|");
        e.m.tab(&mut e.t, true);
        assert_eq!(e.show(), "  x|");
    }

    #[test]
    fn block_indent_keeps_the_selection_over_the_lines() {
        let mut e = Ed::new(Lang::Json, "a\nb\nc");
        e.m.select_range(0, 3); // "a\nb"
        e.m.tab(&mut e.t, false);
        assert_eq!(e.t, "  a\n  b\nc");
        assert_eq!(e.m.selected(&e.t), "  a\n  b");
        // selection ending at the start of a line leaves that line alone
        let mut e = Ed::new(Lang::Json, "a\nb\nc");
        e.m.select_range(0, 4);
        e.m.tab(&mut e.t, false);
        assert_eq!(e.t, "  a\n  b\nc");
    }

    #[test]
    fn comment_toggle() {
        let mut e = Ed::new(Lang::JavaScript, "a();\n  b();|");
        e.m.select_all(&e.t);
        e.m.toggle_comment(&mut e.t, e.lang);
        assert_eq!(e.t, "// a();\n//   b();");
        e.m.toggle_comment(&mut e.t, e.lang);
        assert_eq!(e.t, "a();\n  b();");
        let mut e = Ed::new(Lang::JavaScript, "  x|\n");
        e.m.toggle_comment(&mut e.t, e.lang);
        assert_eq!(e.show(), "  // x|\n");
        // json has no comments
        let mut e = Ed::new(Lang::Json, "x|");
        e.m.toggle_comment(&mut e.t, e.lang);
        assert_eq!(e.t, "x");
        let mut e = Ed::new(Lang::Graphql, "q|");
        e.m.toggle_comment(&mut e.t, e.lang);
        assert_eq!(e.t, "# q");
    }

    #[test]
    fn undo_redo_merges_typing() {
        let mut e = Ed::new(Lang::Text, "|");
        e.type_str("hello");
        e.type_str(" ");
        e.type_str("x");
        assert_eq!(e.t, "hello x");
        assert!(e.m.undo(&mut e.t));
        assert_eq!(e.t, "");
        assert!(!e.m.undo(&mut e.t));
        assert!(e.m.redo(&mut e.t));
        assert_eq!(e.show(), "hello x|");
        // a newline starts a new step
        let mut e = Ed::new(Lang::Text, "|");
        e.type_str("a\nb");
        e.m.undo(&mut e.t);
        assert_eq!(e.t, "a\n");
        e.m.undo(&mut e.t);
        e.m.undo(&mut e.t);
        assert_eq!(e.t, "");
    }

    #[test]
    fn backspace_runs_undo_together() {
        let mut e = Ed::new(Lang::Text, "abcd|");
        for _ in 0..3 {
            e.m.backspace(&mut e.t, e.lang, false);
        }
        assert_eq!(e.t, "a");
        e.m.undo(&mut e.t);
        assert_eq!(e.show(), "abcd|");
    }

    #[test]
    fn vertical_motion_keeps_the_column() {
        let mut e = Ed::new(Lang::Text, "abcdef\nab\nabcdef|");
        e.m.move_v(&e.t, -1, false);
        assert_eq!(e.show(), "abcdef\nab|\nabcdef");
        e.m.move_v(&e.t, -1, false);
        assert_eq!(e.show(), "abcdef|\nab\nabcdef");
        e.m.move_v(&e.t, 5, false);
        assert_eq!(e.m.cursor, e.t.len());
        e.m.move_v(&e.t, -10, false);
        assert_eq!(e.m.cursor, 0);
    }

    #[test]
    fn word_motion_and_selection() {
        let t = "foo.bar  baz";
        assert_eq!(word_right(t, 0), 3);
        assert_eq!(word_right(t, 3), 4);
        assert_eq!(word_right(t, 4), 9);
        assert_eq!(word_left(t, 12), 9);
        assert_eq!(word_left(t, 9), 4);
        let mut m = Model::new(t);
        m.select_word_at(t, 5);
        assert_eq!(&t[m.selection()], "bar");
        m.select_word_at(t, 3);
        assert_eq!(&t[m.selection()], ".");
    }

    #[test]
    fn home_toggles_between_indent_and_column_zero() {
        let mut e = Ed::new(Lang::Text, "  ab|");
        e.m.home(&e.t, false, false);
        assert_eq!(e.m.cursor, 2);
        e.m.home(&e.t, false, false);
        assert_eq!(e.m.cursor, 0);
    }

    #[test]
    fn multibyte_text_is_safe() {
        let mut e = Ed::new(Lang::Json, "\"é|\"");
        e.m.backspace(&mut e.t, e.lang, false);
        assert_eq!(e.t, "\"\"");
        e.type_str("日本");
        e.m.move_h(&e.t, false, false, false);
        assert_eq!(e.show(), "\"日|本\"");
    }

    #[test]
    fn bracket_matching() {
        let t = "{ \"a\": [1, (2)] }";
        let none = |_: usize| false;
        assert_eq!(match_bracket(t, 0, 1000, &none), Some((0, t.len() - 1)));
        assert_eq!(match_bracket(t, 1, 1000, &none), Some((0, t.len() - 1)));
        assert_eq!(
            match_bracket(t, t.len(), 1000, &none),
            Some((t.len() - 1, 0))
        );
        let t = "(\")\" )";
        let in_str = |i: usize| i == 1 || i == 2 || i == 3;
        assert_eq!(match_bracket(t, 0, 1000, &in_str), Some((0, 5)));
        assert_eq!(match_bracket("(", 0, 1000, &none), None);
        assert_eq!(match_bracket("abc", 1, 1000, &none), None);
    }

    #[test]
    fn find_is_ascii_case_insensitive() {
        assert_eq!(find_all("Foo foo FOO", "foo", 10), vec![0..3, 4..7, 8..11]);
        assert_eq!(find_all("aaa", "a", 2).len(), 2);
        assert!(find_all("abc", "", 5).is_empty());
    }

    #[test]
    fn json_format() {
        assert_eq!(
            format_json("{\"a\":[1,2],\"b\":{}}").unwrap(),
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {}\n}"
        );
        assert!(format_json("{oops").is_none());
    }
}
