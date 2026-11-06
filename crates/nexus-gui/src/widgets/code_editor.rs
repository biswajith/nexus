//! The one place that draws an editable code area: syntax colours, line-number gutter, current
//! line and bracket highlight, auto-indent / auto-close / comment toggle, find, `{{variable}}`
//! and `nx.*` autocomplete.
//!
//! It is a custom widget (not `egui::TextEdit`) so it can lay out and highlight only the visible
//! lines: a 1 MB body costs the same per frame as a 1 KB one. The editing rules live in
//! [`crate::editor::model`], the completion sources in [`crate::editor::complete`].
//!
//! Accessibility: the editor is one multi-line text input with a label and description (the
//! gutter is painted only, not exposed); the completion list is a list box whose options are
//! list-box options, the highlighted one being the editor's active descendant.

use super::{a11y, bar, field, icon_button, labeled_text, ltr, sub, vchevron, x_icon};
use crate::editor::complete::{self, Item, ItemKind, Session, VarItem};
use crate::editor::model::{self, Model};
use crate::highlight::{spans, Lang, Span};
use crate::state::response::MAX_LINE_CHARS;
use crate::theme::*;
use eframe::egui::{
    self, accesskit::Role, pos2, text::CCursor, vec2, Align, Align2, Color32, Context, CursorIcon,
    EventFilter, FontFamily, FontId, Galley, Id, Key, Modifiers, Order, Rect, Response, ScrollArea,
    Sense, Stroke, StrokeKind, Ui,
};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex};

const FONT_SIZE: f32 = 13.0;
const ROW_H: f32 = 19.5;
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 8.0;
const TAB_W: usize = 4;
/// Above this the whole buffer is no longer lexed in one go: each visible line is lexed alone
/// (exact for JSON; block comments / template strings spanning lines are approximated).
const BIG_DOC: usize = 300_000;
const BRACKET_SCAN: usize = 20_000;
const MAX_MATCHES: usize = 10_000;
const POPUP_ROWS: usize = 8;
/// The a11y value is cut here (a screen reader gets the start of a huge buffer).
const A11Y_VALUE_MAX: usize = 100_000;

const DESCRIPTION: &str = "Code editor. Tab inserts indentation. Press Escape then Tab, or \
Control+M, to move focus out of the editor. Control+F finds, Control+Space opens suggestions.";

// ---- public API ----

/// A multi-line, syntax coloured editor filling the space left in `ui`. `label` is the
/// accessible name and the widget's identity. Returns the editor's response.
pub fn code_editor(ui: &mut Ui, label: &str, text: &mut String, lang: Lang) -> Response {
    let size = ui.available_size_before_wrap();
    let (rect, _) = ui.allocate_exact_size(vec2(size.x, size.y.max(60.0)), Sense::hover());
    ui.painter().rect_filled(rect, 0, bg_sunken());
    let id = ui.make_persistent_id(("code_editor", label));
    let shared = state_for(ui.ctx(), id);
    let mut st = shared.lock().unwrap_or_else(|e| e.into_inner());
    st.sync(text, lang);
    if take_flag(ui.ctx(), format_flag(label)) && lang == Lang::Json {
        st.format_json(text);
    }

    let mut area = rect;
    if st.find.is_some() {
        let h = 36.0;
        let bar_rect = Rect::from_min_size(rect.min, vec2(rect.width(), h));
        area = Rect::from_min_max(pos2(rect.left(), rect.top() + h), rect.max);
        sub(ui, bar_rect, egui::Layout::top_down(Align::Min), |ui| {
            find_bar(ui, &mut st, text);
        });
    }
    let resp = editor_area(ui, area, id, label, text, lang, &mut st);
    completion_popup(ui, id, &mut st, text, lang);
    if resp.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            0,
            Stroke::new(2.0, accent()),
            StrokeKind::Inside,
        );
    }
    resp
}

/// Variables offered after `{{`; call each frame before drawing editors.
pub fn set_variables(ctx: &Context, vars: Vec<VarItem>) {
    set_variables_shared(ctx, Arc::new(vars));
}

/// [`set_variables`] for a list that is kept between frames (no per-frame copy).
pub fn set_variables_shared(ctx: &Context, vars: Arc<Vec<VarItem>>) {
    ctx.data_mut(|d| d.insert_temp(Id::unique("nexus_editor_vars"), vars));
}

fn variables(ctx: &Context) -> Arc<Vec<VarItem>> {
    ctx.data(|d| d.get_temp::<Arc<Vec<VarItem>>>(Id::unique("nexus_editor_vars")))
        .unwrap_or_default()
}

fn format_flag(label: &str) -> Id {
    Id::unique(("nexus_editor_format", label))
}

/// Asks the JSON editor named `label` to pretty-print its buffer (undoable, ignored if the text
/// is not valid JSON).
pub fn request_format(ctx: &Context, label: &str) {
    ctx.data_mut(|d| d.insert_temp(format_flag(label), true));
}

/// Gives keyboard focus to the first editor drawn after this call (dev flag `--focus-editor`).
pub fn focus_next_editor(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(Id::unique("nexus_editor_focus"), true));
}

fn take_flag(ctx: &Context, id: Id) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(id)).unwrap_or(false)
}

// ---- state ----

struct Find {
    query: String,
    matches: Vec<Range<usize>>,
    /// (text revision, query) the matches were computed for.
    key: Option<(u64, String)>,
    current: usize,
    focus: bool,
    jump: bool,
}

/// `((revision, caret), matching pair)`.
type BracketCache = ((u64, usize), Option<(usize, usize)>);

struct State {
    model: Model,
    snapshot: String,
    lang: Lang,
    /// Whole-buffer spans for small buffers: (revision, spans).
    spans: Option<(u64, Vec<Span>)>,
    galleys: HashMap<usize, Arc<Galley>>,
    galley_rev: u64,
    galley_ppp: f32,
    galley_epoch: u64,
    /// Longest line in columns, for the scroll width: (revision, cols).
    width: (u64, usize),
    find: Option<Find>,
    find_focused: bool,
    complete: Option<Session>,
    complete_top: usize,
    popup_rect: Option<Rect>,
    /// Ctrl+M: Tab moves focus instead of indenting.
    tab_free: bool,
    /// Escape was pressed; a following Tab moves focus.
    esc_armed: bool,
    was_focused: bool,
    reveal: bool,
    refocus: bool,
    page_rows: usize,
    caret_screen: Option<Rect>,
    bracket: Option<BracketCache>,
    char_w: f32,
}

impl State {
    fn new() -> Self {
        Self {
            model: Model::new(""),
            snapshot: String::new(),
            lang: Lang::Text,
            spans: None,
            galleys: HashMap::new(),
            galley_rev: 0,
            galley_ppp: 0.0,
            galley_epoch: 0,
            width: (u64::MAX, 0),
            find: None,
            find_focused: false,
            complete: None,
            complete_top: 0,
            popup_rect: None,
            tab_free: false,
            esc_armed: false,
            was_focused: false,
            reveal: false,
            refocus: false,
            page_rows: 20,
            caret_screen: None,
            bracket: None,
            char_w: 8.0,
        }
    }

    /// Notices text that changed behind the editor's back (another tab, a snippet, Format).
    fn sync(&mut self, text: &str, lang: Lang) {
        if self.lang != lang {
            self.lang = lang;
            self.spans = None;
            self.galleys.clear();
        }
        if self.snapshot != text {
            self.snapshot = text.to_string();
            self.model.reset(text);
            self.complete = None;
            self.reveal = false;
        }
    }

    fn format_json(&mut self, text: &mut String) {
        let Some(pretty) = model::format_json(text) else {
            return;
        };
        if pretty != *text {
            let n = text.len();
            self.model
                .replace(text, 0..n, &pretty, model::Kind::Other, Some((0, 0)));
            self.snapshot.clone_from(text);
            self.reveal = true;
        }
    }

    fn big(&self) -> bool {
        self.snapshot.len() > BIG_DOC
    }

    fn ensure_caches(&mut self, ctx: &Context, text: &str) {
        let ppp = ctx.pixels_per_point();
        let rev = self.model.rev;
        let epoch = crate::theme::galley_epoch(ctx);
        if self.galley_epoch != epoch {
            // syntax colours are baked into the spans; galleys die with the glyph atlas
            self.galley_epoch = epoch;
            self.spans = None;
            self.galleys.clear();
        }
        if self.galley_rev != rev
            || (self.galley_ppp - ppp).abs() > f32::EPSILON
            || self.galleys.len() > 2500
        {
            self.galleys.clear();
            self.galley_rev = rev;
            self.galley_ppp = ppp;
        }
        if !self.big() && self.spans.as_ref().is_none_or(|(r, _)| *r != rev) {
            self.spans = Some((rev, spans(self.lang, text)));
        }
        if self.big() {
            self.spans = None;
        }
        if self.width.0 != rev {
            let mut cols = 0;
            for i in 0..self.model.line_count() {
                cols = cols.max(self.model.line_end(text, i) - self.model.line_start(i));
            }
            self.width = (rev, cols.min(MAX_LINE_CHARS + 20));
        }
    }

    fn line_galley(&mut self, ctx: &Context, text: &str, i: usize) -> Arc<Galley> {
        if let Some(g) = self.galleys.get(&i) {
            return g.clone();
        }
        let range = self.model.line_range(text, i);
        let (shown, cut) = cut_line(&text[range.clone()]);
        let local: Vec<Span> = match &self.spans {
            Some((_, all)) => clip_spans(all, range.start, range.start + shown.len()),
            None => spans(self.lang, shown),
        };
        let job = line_job(shown, &local, cut);
        let g = ctx.fonts_mut(|f| f.layout_job(job));
        self.galleys.insert(i, g.clone());
        g
    }
}

fn state_for(ctx: &Context, id: Id) -> Arc<Mutex<State>> {
    if let Some(s) = ctx.data(|d| d.get_temp::<Arc<Mutex<State>>>(id)) {
        return s;
    }
    let s = Arc::new(Mutex::new(State::new()));
    ctx.data_mut(|d| d.insert_temp(id, s.clone()));
    s
}

// ---- line layout ----

/// `line` cut to the display limit; the flag says it was cut.
fn cut_line(line: &str) -> (&str, bool) {
    match line.char_indices().nth(MAX_LINE_CHARS) {
        Some((i, _)) => (&line[..i], true),
        None => (line, false),
    }
}

/// Spans of `all` within `lo..hi`, shifted to start at `lo`.
fn clip_spans(all: &[Span], lo: usize, hi: usize) -> Vec<Span> {
    let first = all.partition_point(|s| s.1 <= lo);
    all[first..]
        .iter()
        .take_while(|s| s.0 < hi)
        .map(|&(a, b, c)| (a.max(lo) - lo, b.min(hi) - lo, c))
        .collect()
}

fn mono() -> FontId {
    FontId::new(FONT_SIZE, FontFamily::Monospace)
}

fn fmt(color: Color32) -> egui::TextFormat {
    egui::TextFormat {
        font_id: mono(),
        color,
        line_height: Some(ROW_H),
        ..Default::default()
    }
}

/// Tabs become spaces so one glyph is one column.
fn expand_tabs(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('\t') {
        s.replace('\t', &" ".repeat(TAB_W)).into()
    } else {
        s.into()
    }
}

fn line_job(line: &str, local: &[Span], cut: bool) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut at = 0;
    let push = |job: &mut egui::text::LayoutJob, a: usize, b: usize, c: Color32| {
        if a < b {
            job.append(&expand_tabs(&line[a..b]), 0.0, fmt(c));
        }
    };
    for &(a, b, c) in local {
        push(&mut job, at, a, fg());
        push(&mut job, a, b, c);
        at = b;
    }
    push(&mut job, at, line.len(), fg());
    if cut {
        job.append(" … (line cut)", 0.0, fmt(fg_3()));
    }
    job
}

/// Display column of byte `idx` in `line` (tabs are [`TAB_W`] wide), capped at the cut.
fn vcol(line: &str, idx: usize) -> usize {
    let mut col = 0;
    for (n, (i, c)) in line.char_indices().enumerate() {
        if i >= idx || n >= MAX_LINE_CHARS {
            break;
        }
        col += if c == '\t' { TAB_W } else { 1 };
    }
    col
}

/// Byte offset in `line` of display column `col` (nearest boundary).
fn byte_at_vcol(line: &str, col: usize) -> usize {
    let mut acc = 0;
    for (n, (i, c)) in line.char_indices().enumerate() {
        if n >= MAX_LINE_CHARS {
            return i;
        }
        let w = if c == '\t' { TAB_W } else { 1 };
        if col < acc + w {
            return if w > 1 && (col - acc) * 2 >= w {
                i + c.len_utf8()
            } else {
                i
            };
        }
        acc += w;
    }
    line.len()
}

// ---- the editing surface ----

#[allow(clippy::too_many_arguments)]
fn editor_area(
    ui: &mut Ui,
    area: Rect,
    id: Id,
    label: &str,
    text: &mut String,
    lang: Lang,
    st: &mut State,
) -> Response {
    let ctx = ui.ctx().clone();
    st.char_w = ctx.fonts_mut(|f| f.glyph_width(&mono(), '0'));
    let char_w = st.char_w;
    let mut out = None;
    sub(ui, area, egui::Layout::top_down(Align::Min), |ui| {
        ScrollArea::both()
            .id_salt(("code_editor_scroll", label))
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                st.ensure_caches(&ctx, text);
                let n = st.model.line_count();
                let digits = n.max(1).to_string().len().max(2);
                let gutter = digits as f32 * char_w + 20.0;
                let content_w = gutter + PAD_X + (st.width.1 as f32 + 4.0) * char_w + PAD_X;
                let content_h = n as f32 * ROW_H + 2.0 * PAD_Y + 3.0 * ROW_H;
                let (rect, _) = ui.allocate_exact_size(
                    vec2(
                        content_w.max(viewport.width()),
                        content_h.max(viewport.height()),
                    ),
                    Sense::hover(),
                );
                let resp = ui.interact(rect, id, Sense::click_and_drag());
                let text_x = rect.left() + gutter + PAD_X;
                st.page_rows = ((viewport.height() / ROW_H) as usize)
                    .saturating_sub(1)
                    .max(1);

                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::Text);
                }
                let before_rev = st.model.rev;
                let before_sel = (st.model.anchor, st.model.cursor);
                // ---- mouse ----
                mouse(ui, &resp, st, text, rect, text_x);
                // ---- keyboard ----
                let focused = resp.has_focus();
                if focused {
                    handle_events(ui, st, text, lang);
                }
                if st.was_focused && !focused {
                    let over = st
                        .popup_rect
                        .zip(ui.input(|i| i.pointer.latest_pos()))
                        .is_some_and(|(r, p)| r.contains(p));
                    if !over && !st.find_focused {
                        st.complete = None;
                    }
                    st.tab_free = false;
                    st.esc_armed = false;
                }
                st.was_focused = focused;
                if focused {
                    let filter = EventFilter {
                        tab: !(st.tab_free || st.esc_armed),
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    };
                    ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
                }
                if st.refocus || take_flag(&ctx, Id::unique("nexus_editor_focus")) {
                    resp.request_focus();
                    st.refocus = false;
                }
                if st.model.rev != before_rev {
                    st.snapshot.clone_from(text);
                    st.ensure_caches(&ctx, text);
                }
                if st.model.rev != before_rev || before_sel != (st.model.anchor, st.model.cursor) {
                    st.reveal = true;
                }
                paint(ui, st, text, rect, viewport, gutter, text_x, focused);

                // ---- accessibility ----
                let resp = a11y(resp, Role::MultilineTextInput, label);
                let value = a11y_value(text);
                let active = st.complete.as_ref().map(|s| option_id(id, s.selected));
                ctx.accesskit_node_builder(id, |nb| {
                    nb.set_description(DESCRIPTION);
                    nb.set_value(value);
                    if let Some(a) = active {
                        nb.set_active_descendant(a.accesskit_id());
                    }
                });
                if focused {
                    let caret = st.caret_screen.unwrap_or(area);
                    ctx.output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput {
                            purpose: egui::IMEPurpose::Normal,
                            rect: area,
                            cursor_rect: caret,
                            should_interrupt_composition: false,
                        });
                    });
                }
                out = Some(resp);
            });
        if st.tab_free || st.esc_armed {
            let r = area.shrink(8.0);
            ui.painter().text(
                r.right_bottom(),
                Align2::RIGHT_BOTTOM,
                "Tab moves focus",
                egui::FontId::new(11.0, FontFamily::Proportional),
                warning(),
            );
        }
    });
    out.expect("scroll area content ran")
}

fn a11y_value(text: &str) -> String {
    if text.len() <= A11Y_VALUE_MAX {
        return text.to_string();
    }
    let mut end = A11Y_VALUE_MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Byte offset under screen position `p`.
fn offset_at(
    ctx: &Context,
    st: &mut State,
    text: &str,
    rect: Rect,
    text_x: f32,
    p: egui::Pos2,
) -> usize {
    let n = st.model.line_count();
    let row = ((p.y - rect.top() - PAD_Y) / ROW_H).floor();
    let i = if row < 0.0 {
        0
    } else {
        (row as usize).min(n - 1)
    };
    let r = st.model.line_range(text, i);
    let g = st.line_galley(ctx, text, i);
    let c = g.cursor_from_pos(vec2(p.x - text_x, 0.0));
    r.start + byte_at_vcol(&text[r], c.index.0)
}

fn mouse(ui: &Ui, resp: &Response, st: &mut State, text: &str, rect: Rect, text_x: f32) {
    let ctx = ui.ctx();
    let Some(p) = ui.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let (pressed, shift) = ui.input(|i| (i.pointer.primary_pressed(), i.modifiers.shift));
    if pressed && resp.hovered() {
        resp.request_focus();
        let off = offset_at(ctx, st, text, rect, text_x, p);
        st.model.set_cursor(off, shift);
        st.complete = None;
        st.reveal = false;
    } else if resp.dragged_by(egui::PointerButton::Primary) {
        let off = offset_at(ctx, st, text, rect, text_x, p);
        let cur = st.model.cursor;
        if off != cur {
            st.model.set_cursor(off, true);
            st.reveal = true;
        }
    }
    if resp.double_clicked() {
        let off = offset_at(ctx, st, text, rect, text_x, p);
        st.model.select_word_at(text, off);
    } else if resp.triple_clicked() {
        let off = offset_at(ctx, st, text, rect, text_x, p);
        st.model.select_line_at(text, off);
    }
}

// ---- keyboard ----

fn handle_events(ui: &mut Ui, st: &mut State, text: &mut String, lang: Lang) {
    let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
    let ctx = ui.ctx().clone();
    let vars = variables(&ctx);
    let mut handled = vec![false; events.len()];
    for (k, ev) in events.iter().enumerate() {
        handled[k] = match ev {
            egui::Event::Text(s) => {
                st.esc_armed = false;
                type_text(st, text, lang, s, &vars);
                true
            }
            egui::Event::Paste(s) => {
                let s = s.replace("\r\n", "\n").replace('\r', "\n");
                let r = st.model.selection();
                st.model.replace(text, r, &s, model::Kind::Other, None);
                st.complete = None;
                true
            }
            egui::Event::Copy => {
                if st.model.has_selection() {
                    ctx.copy_text(st.model.selected(text).to_string());
                }
                true
            }
            egui::Event::Cut => {
                if st.model.has_selection() {
                    ctx.copy_text(st.model.selected(text).to_string());
                    let r = st.model.selection();
                    st.model.replace(text, r, "", model::Kind::Delete, None);
                }
                st.complete = None;
                true
            }
            egui::Event::Ime(egui::ImeEvent::Commit(s)) => {
                type_text(st, text, lang, s, &vars);
                true
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => key_pressed(st, text, lang, *key, *modifiers, &vars),
            _ => false,
        };
    }
    if handled.iter().any(|h| *h) {
        let mut idx = 0;
        ui.input_mut(|i| {
            i.events.retain(|_| {
                let keep = !handled.get(idx).copied().unwrap_or(false);
                idx += 1;
                keep
            })
        });
    }
}

fn type_text(st: &mut State, text: &mut String, lang: Lang, s: &str, vars: &[VarItem]) {
    if s.chars().count() > 256 {
        let r = st.model.selection();
        st.model.replace(text, r, s, model::Kind::Other, None);
        st.complete = None;
        return;
    }
    for c in s.chars() {
        match c {
            '\n' => {
                st.model.newline(text, lang);
                st.complete = None;
            }
            '\t' => st.model.tab(text, false),
            '\r' => {}
            c if c.is_control() => {}
            c => {
                let mut b = [0u8; 4];
                st.model.insert_text(text, lang, c.encode_utf8(&mut b));
                refresh_completion(st, text, lang, vars, false);
            }
        }
    }
}

/// Recomputes the popup after typing; opens it when a source applies.
fn refresh_completion(st: &mut State, text: &str, lang: Lang, vars: &[VarItem], explicit: bool) {
    let prev = st
        .complete
        .as_ref()
        .and_then(|s| s.items.get(s.selected))
        .map(|i| i.label.clone());
    st.complete = complete::query(lang, text, st.model.cursor, vars, explicit).map(|mut s| {
        if let Some(p) = prev {
            if let Some(i) = s.items.iter().position(|it| it.label == p) {
                s.selected = i;
            }
        }
        s
    });
    if st.complete.is_none() {
        st.complete_top = 0;
    }
}

fn accept_completion(st: &mut State, text: &mut String) {
    let Some(session) = st.complete.take() else {
        return;
    };
    let a = complete::apply(text, &session, st.model.cursor);
    st.model.replace(
        text,
        a.range,
        &a.text,
        model::Kind::Other,
        Some((a.caret, a.caret)),
    );
}

fn key_pressed(
    st: &mut State,
    text: &mut String,
    lang: Lang,
    key: Key,
    m: Modifiers,
    vars: &[VarItem],
) -> bool {
    let cmd = m.command;
    let shift = m.shift;
    if key != Key::Escape && !matches!(key, Key::Tab) {
        st.esc_armed = false;
    }
    // ---- completion popup owns a few keys ----
    if let Some(s) = st.complete.as_mut() {
        let len = s.items.len();
        match key {
            Key::ArrowDown if !cmd => {
                s.selected = (s.selected + 1) % len;
                return true;
            }
            Key::ArrowUp if !cmd => {
                s.selected = (s.selected + len - 1) % len;
                return true;
            }
            Key::Enter | Key::Tab if !cmd && !shift => {
                accept_completion(st, text);
                return true;
            }
            Key::Escape => {
                st.complete = None;
                return true;
            }
            _ => {}
        }
    }
    let rows = st.page_rows as isize;
    let before = st.model.rev;
    let mut moved = true;
    match key {
        Key::ArrowLeft => st.model.move_h(text, false, cmd, shift),
        Key::ArrowRight => st.model.move_h(text, true, cmd, shift),
        Key::ArrowUp => st.model.move_v(text, -1, shift),
        Key::ArrowDown => st.model.move_v(text, 1, shift),
        Key::PageUp => st.model.move_v(text, -rows, shift),
        Key::PageDown => st.model.move_v(text, rows, shift),
        Key::Home => st.model.home(text, cmd, shift),
        Key::End => st.model.end(text, cmd, shift),
        Key::A if cmd => st.model.select_all(text),
        Key::Backspace => st.model.backspace(text, lang, cmd),
        Key::Delete => st.model.delete(text, cmd),
        Key::Enter if !cmd => st.model.newline(text, lang),
        Key::Tab if !cmd => {
            if st.tab_free || st.esc_armed {
                return false;
            }
            st.model.tab(text, shift);
        }
        Key::Z if cmd && !shift => {
            st.model.undo(text);
        }
        Key::Z | Key::Y if cmd => {
            st.model.redo(text);
        }
        Key::Slash if cmd => st.model.toggle_comment(text, lang),
        Key::F if cmd && m.alt => return false,
        Key::F if cmd => {
            open_find(st, text);
            return true;
        }
        Key::F if m.alt && shift && lang == Lang::Json => st.format_json(text),
        Key::F3 => {
            if st.find.is_some() {
                find_step(st, text, !shift);
                return true;
            }
            return false;
        }
        Key::M if cmd => {
            st.tab_free = !st.tab_free;
            return true;
        }
        Key::Space if cmd => {
            refresh_completion(st, text, lang, vars, true);
            return true;
        }
        Key::Escape => {
            if st.find.is_some() {
                close_find(st);
            } else {
                st.esc_armed = true;
            }
            return true;
        }
        _ => moved = false,
    }
    if !moved {
        return false;
    }
    // popup follows typing (backspace/delete) but is dropped by any other command
    if matches!(key, Key::Backspace | Key::Delete) && st.model.rev != before {
        if st.complete.is_some() {
            refresh_completion(st, text, lang, vars, false);
        }
    } else {
        st.complete = None;
    }
    true
}

// ---- find ----

fn open_find(st: &mut State, text: &str) {
    let sel = st.model.selected(text);
    let preset =
        (!sel.is_empty() && sel.len() < 100 && !sel.contains('\n')).then(|| sel.to_string());
    match st.find.as_mut() {
        Some(f) => {
            f.focus = true;
            if let Some(p) = preset {
                f.query = p;
            }
        }
        None => {
            st.find = Some(Find {
                query: preset.unwrap_or_default(),
                matches: Vec::new(),
                key: None,
                current: 0,
                focus: true,
                jump: false,
            });
        }
    }
}

fn close_find(st: &mut State) {
    st.find = None;
    st.find_focused = false;
    st.refocus = true;
}

fn refresh_matches(st: &mut State, text: &str) {
    let rev = st.model.rev;
    let cursor = st.model.selection().start;
    let Some(f) = st.find.as_mut() else { return };
    let key = (rev, f.query.clone());
    if f.key.as_ref() == Some(&key) {
        return;
    }
    let query_changed = f.key.as_ref().is_none_or(|k| k.1 != f.query);
    f.matches = model::find_all(text, &f.query, MAX_MATCHES);
    f.current = f
        .matches
        .iter()
        .position(|m| m.start >= cursor)
        .unwrap_or(0);
    f.jump = query_changed && !f.matches.is_empty();
    f.key = Some(key);
}

fn find_step(st: &mut State, text: &str, forward: bool) {
    refresh_matches(st, text);
    let Some(f) = st.find.as_mut() else { return };
    let n = f.matches.len();
    if n == 0 {
        return;
    }
    f.current = if forward {
        (f.current + 1) % n
    } else {
        (f.current + n - 1) % n
    };
    f.jump = true;
}

/// Selects the current match once, scrolling it into view.
fn apply_find_jump(st: &mut State) {
    let Some(f) = st.find.as_mut() else { return };
    if !f.jump {
        return;
    }
    f.jump = false;
    if let Some(m) = f.matches.get(f.current).cloned() {
        st.model.select_range(m.start, m.end);
        st.reveal = true;
    }
}

fn find_bar(ui: &mut Ui, st: &mut State, text: &str) {
    let r = bar(ui, 36.0, bg_surface(), Some(border_subtle()));
    sub(ui, r.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let mut query = st
            .find
            .as_ref()
            .map(|f| f.query.clone())
            .unwrap_or_default();
        let f = field(
            ui,
            "Find in editor",
            &mut query,
            260.0,
            26.0,
            super::font(12.0, "Regular"),
            "Find...",
            bg_input(),
            true,
        );
        st.find_focused = f.has_focus();
        if let Some(find) = st.find.as_mut() {
            if find.focus {
                f.request_focus();
                find.focus = false;
            }
            find.query = query;
        }
        let (enter, shift, esc, f3) = ui.input(|i| {
            (
                i.key_pressed(Key::Enter),
                i.modifiers.shift,
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::F3),
            )
        });
        if f.lost_focus() && enter {
            refresh_matches(st, text);
            find_step(st, text, !shift);
            f.request_focus();
        }
        if f.has_focus() && f3 {
            find_step(st, text, !shift);
        }
        if f.has_focus() && esc {
            close_find(st);
            return;
        }
        refresh_matches(st, text);
        apply_find_jump(st);
        let Some(find) = st.find.as_ref() else { return };
        let n = find.matches.len();
        let count = if find.query.is_empty() {
            String::new()
        } else if n == 0 {
            "No results".to_string()
        } else if n >= MAX_MATCHES {
            format!("{} / {}+", find.current + 1, MAX_MATCHES)
        } else {
            format!("{} / {}", find.current + 1, n)
        };
        labeled_text(
            ui,
            &count,
            &format!("Editor matches {count}"),
            super::font(11.5, "Regular"),
            fg_2(),
        );
        if icon_button(ui, "Previous match in editor", 24.0, |ui, c, col| {
            vchevron(ui, c, true, col)
        })
        .clicked()
        {
            find_step(st, text, false);
        }
        if icon_button(ui, "Next match in editor", 24.0, |ui, c, col| {
            vchevron(ui, c, false, col)
        })
        .clicked()
        {
            find_step(st, text, true);
        }
        if icon_button(ui, "Close find in editor", 24.0, |ui, c, col| {
            x_icon(ui, c, 3.5, col)
        })
        .clicked()
        {
            close_find(st);
        }
    });
}

// ---- painting ----

fn sel_fill(focused: bool) -> Color32 {
    if focused {
        with_alpha(accent(), 85)
    } else {
        with_alpha(accent(), 45)
    }
}
fn match_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(0xf5, 0x9e, 0x0b, 70)
}
fn match_fill_current() -> Color32 {
    Color32::from_rgba_unmultiplied(0xf5, 0x9e, 0x0b, 190)
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &mut Ui,
    st: &mut State,
    text: &str,
    rect: Rect,
    viewport: Rect,
    gutter: f32,
    text_x: f32,
    focused: bool,
) {
    let ctx = ui.ctx().clone();
    let p = ui.painter().clone();
    let n = st.model.line_count();
    let first = (((viewport.min.y - PAD_Y) / ROW_H).floor().max(0.0) as usize).min(n - 1);
    let last = ((((viewport.max.y - PAD_Y) / ROW_H).ceil().max(0.0)) as usize + 1).min(n);
    let gx = rect.left() + viewport.min.x;
    let sel = st.model.selection();
    let cur_line = st.model.line_of(st.model.cursor);
    let bracket = bracket_pair(st, text);
    let find_owned = st.find.take();
    let find = find_owned.as_ref();
    let find_current = find.and_then(|f| f.matches.get(f.current).cloned());
    let width = rect.width().max(viewport.width());
    let ch_w = st.char_w;
    let mut caret: Option<Rect> = None;

    for i in first..last {
        let y = rect.top() + PAD_Y + i as f32 * ROW_H;
        let lr = st.model.line_range(text, i);
        let line = &text[lr.clone()];
        let g = st.line_galley(&ctx, text, i);
        let x_of = |byte: usize| -> f32 {
            let col = vcol(line, byte.saturating_sub(lr.start).min(line.len()));
            g.pos_from_cursor(CCursor::new(col)).min.x + text_x
        };
        if i == cur_line && focused && sel.is_empty() {
            p.rect_filled(
                Rect::from_min_size(pos2(rect.left(), y), vec2(width, ROW_H)),
                0,
                if pal().dark {
                    Color32::from_white_alpha(8)
                } else {
                    Color32::from_black_alpha(10)
                },
            );
        }
        // find matches
        if let Some(f) = find {
            let lo = f.matches.partition_point(|m| m.end <= lr.start);
            for m in f.matches[lo..].iter().take_while(|m| m.start <= lr.end) {
                if m.start < lr.start || m.end > lr.end {
                    continue;
                }
                let r = Rect::from_min_max(pos2(x_of(m.start), y), pos2(x_of(m.end), y + ROW_H));
                let cur = find_current.as_ref() == Some(m);
                p.rect_filled(
                    r.expand2(vec2(1.0, -1.0)),
                    2,
                    if cur {
                        match_fill_current()
                    } else {
                        match_fill()
                    },
                );
            }
        }
        // selection
        if !sel.is_empty() && sel.start <= lr.end && sel.end > lr.start {
            let a = sel.start.max(lr.start);
            let b = sel.end.min(lr.end);
            let mut x1 = x_of(b);
            if sel.end > lr.end {
                x1 += ch_w * 0.6;
            }
            p.rect_filled(
                Rect::from_min_max(pos2(x_of(a), y), pos2(x1.max(x_of(a) + 2.0), y + ROW_H)),
                0,
                sel_fill(focused),
            );
        }
        // matching brackets
        if let Some((a, b)) = bracket {
            for o in [a, b] {
                if o >= lr.start && o < lr.end {
                    let r = Rect::from_min_max(pos2(x_of(o), y), pos2(x_of(o + 1), y + ROW_H));
                    p.rect_filled(r, 2, accent_subtle());
                    p.rect_stroke(r, 2, Stroke::new(1.0, accent()), StrokeKind::Inside);
                }
            }
        }
        p.galley(pos2(text_x, y), g.clone(), fg());
        if st.model.cursor >= lr.start && st.model.cursor <= lr.end && i == cur_line {
            let cx = x_of(st.model.cursor);
            let r = Rect::from_min_size(pos2(cx, y), vec2(1.5, ROW_H));
            caret = Some(r);
            if focused {
                p.rect_filled(r, 0, fg());
            }
        }
    }
    // The caret can be far outside the painted rows (after Ctrl+End): place it from the model.
    if caret.is_none() {
        let i = cur_line;
        let lr = st.model.line_range(text, i);
        let line = &text[lr.clone()];
        let g = st.line_galley(&ctx, text, i);
        let col = vcol(line, st.model.cursor - lr.start);
        let x = g.pos_from_cursor(CCursor::new(col)).min.x + text_x;
        let y = rect.top() + PAD_Y + i as f32 * ROW_H;
        caret = Some(Rect::from_min_size(pos2(x, y), vec2(1.5, ROW_H)));
    }
    st.find = find_owned;
    st.caret_screen = caret;
    if st.reveal {
        st.reveal = false;
        if let Some(c) = caret {
            ui.scroll_to_rect(
                Rect::from_min_max(
                    pos2(c.left() - gutter - PAD_X - 10.0, c.top() - ROW_H),
                    pos2(c.right() + 48.0, c.bottom() + ROW_H),
                ),
                None,
            );
        }
    }
    // gutter: sticky while scrolling sideways, painted over the text
    let clip = ui.clip_rect();
    p.rect_filled(
        Rect::from_min_max(pos2(gx, clip.top()), pos2(gx + gutter, clip.bottom())),
        0,
        bg_sunken(),
    );
    for i in first..last {
        let y = rect.top() + PAD_Y + i as f32 * ROW_H;
        p.text(
            pos2(gx + gutter - 10.0, y + ROW_H / 2.0),
            Align2::RIGHT_CENTER,
            i + 1,
            FontId::new(11.0, FontFamily::Monospace),
            if i == cur_line && focused {
                fg_2()
            } else {
                fg_3()
            },
        );
    }
    p.vline(
        gx + gutter,
        clip.y_range(),
        Stroke::new(1.0, border_subtle()),
    );
}

/// The bracket pair around the caret, cached per (revision, caret).
fn bracket_pair(st: &mut State, text: &str) -> Option<(usize, usize)> {
    let key = (st.model.rev, st.model.cursor);
    if let Some((k, v)) = &st.bracket {
        if *k == key {
            return *v;
        }
    }
    let spans = st.spans.as_ref().map(|(_, s)| s);
    let skip = |o: usize| -> bool {
        spans.is_some_and(|s| {
            let i = s.partition_point(|sp| sp.1 <= o);
            s.get(i).is_some_and(|sp| {
                let json_key = sp.2 == syn_key() && text.as_bytes()[sp.0] == b'"';
                sp.0 <= o && (sp.2 == syn_str() || sp.2 == fg_3() || json_key)
            })
        })
    };
    let v = model::match_bracket(text, st.model.cursor, BRACKET_SCAN, &skip);
    st.bracket = Some((key, v));
    v
}

// ---- completion popup ----

fn option_id(editor: Id, i: usize) -> Id {
    editor.with(("option", i))
}

fn completion_popup(ui: &mut Ui, id: Id, st: &mut State, text: &mut String, lang: Lang) {
    let _ = lang;
    st.popup_rect = None;
    let (Some(session), Some(caret)) = (st.complete.clone(), st.caret_screen) else {
        return;
    };
    let ctx = ui.ctx().clone();
    let items = &session.items;
    let rows = items.len().min(POPUP_ROWS);
    // keep the selected row visible
    if session.selected < st.complete_top {
        st.complete_top = session.selected;
    }
    if session.selected >= st.complete_top + rows {
        st.complete_top = session.selected + 1 - rows;
    }
    st.complete_top = st.complete_top.min(items.len() - rows);
    let top = st.complete_top;
    let row_h = 22.0;
    let label_w = items
        .iter()
        .map(|i| i.label.chars().count())
        .max()
        .unwrap_or(10) as f32
        * st.char_w;
    let detail_w = items
        .iter()
        .map(|i| i.detail.chars().count())
        .max()
        .unwrap_or(0)
        .min(40) as f32
        * 6.2;
    let w = (28.0 + label_w + 24.0 + detail_w + 12.0).clamp(220.0, 560.0);
    let h = rows as f32 * row_h + 8.0;
    let screen = ctx.content_rect();
    let mut pos = pos2(caret.left(), caret.bottom() + 2.0);
    if pos.y + h > screen.bottom() {
        pos.y = (caret.top() - h - 2.0).max(screen.top());
    }
    pos.x = pos.x.min(screen.right() - w - 4.0).max(screen.left());
    let mut clicked = None;
    let area = egui::Area::new(id.with("popup"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(&ctx, |ui| {
            let (rect, list) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            a11y(list, Role::ListBox, "Suggestions");
            let p = ui.painter();
            p.rect_filled(rect, 6, bg_elevated());
            p.rect_stroke(
                rect,
                6,
                Stroke::new(1.0, border_strong()),
                StrokeKind::Inside,
            );
            for (k, item) in items.iter().enumerate().skip(top).take(rows) {
                let r = Rect::from_min_size(
                    pos2(
                        rect.left() + 4.0,
                        rect.top() + 4.0 + (k - top) as f32 * row_h,
                    ),
                    vec2(w - 8.0, row_h),
                );
                let rid = option_id(id, k);
                // not focusable: clicking an option must not take focus from the editor
                let resp = ui.interact(r, rid, Sense::CLICK);
                let selected = k == session.selected;
                if selected {
                    p.rect_filled(r, 4, accent_subtle());
                } else if resp.hovered() {
                    p.rect_filled(r, 4, hover_tint());
                }
                option_row(ui, r, item, selected);
                let name = if item.detail.is_empty() {
                    item.label.clone()
                } else {
                    format!("{}, {}", item.label, item.detail)
                };
                let resp = super::a11y_selected(resp, Role::ListBoxOption, &name, selected);
                ctx.accesskit_node_builder(rid, |nb| {
                    nb.set_position_in_set(k + 1);
                    nb.set_size_of_set(items.len());
                });
                if resp.clicked() {
                    clicked = Some(k);
                }
            }
        });
    st.popup_rect = Some(area.response.rect);
    if let Some(k) = clicked {
        if let Some(s) = st.complete.as_mut() {
            s.selected = k;
        }
        accept_completion(st, text);
        st.refocus = true;
        ctx.request_repaint();
    }
}

fn option_row(ui: &Ui, r: Rect, item: &Item, selected: bool) {
    let p = ui.painter();
    let glyph_col = match item.kind {
        ItemKind::Variable => warning(),
        ItemKind::Function => syn_key(),
        ItemKind::Property => syn_lit(),
    };
    p.text(
        pos2(r.left() + 14.0, r.center().y),
        Align2::CENTER_CENTER,
        item.kind.glyph(),
        FontId::new(10.5, FontFamily::Monospace),
        glyph_col,
    );
    p.text(
        pos2(r.left() + 28.0, r.center().y),
        Align2::LEFT_CENTER,
        &item.label,
        FontId::new(12.5, FontFamily::Monospace),
        if selected { fg() } else { fg_2() },
    );
    if !item.detail.is_empty() {
        p.text(
            pos2(r.right() - 8.0, r.center().y),
            Align2::RIGHT_CENTER,
            &item.detail,
            FontId::new(11.0, FontFamily::Proportional),
            fg_3(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_columns_round_trip() {
        let line = "\ta\tb";
        assert_eq!(vcol(line, 0), 0);
        assert_eq!(vcol(line, 1), TAB_W);
        assert_eq!(vcol(line, 2), TAB_W + 1);
        assert_eq!(vcol(line, 3), 2 * TAB_W + 1);
        assert_eq!(byte_at_vcol(line, 0), 0);
        assert_eq!(byte_at_vcol(line, TAB_W), 1);
        assert_eq!(byte_at_vcol(line, TAB_W + 1), 2);
        assert_eq!(byte_at_vcol(line, 999), line.len());
        // inside a tab: nearest side
        assert_eq!(byte_at_vcol(line, 1), 0);
        assert_eq!(byte_at_vcol(line, 3), 1);
    }

    #[test]
    fn long_lines_are_cut_for_display_only() {
        let line = "é".repeat(MAX_LINE_CHARS + 10);
        let (shown, cut) = cut_line(&line);
        assert!(cut);
        assert_eq!(shown.chars().count(), MAX_LINE_CHARS);
        assert!(!cut_line("short").1);
    }

    #[test]
    fn clip_spans_is_relative_to_the_line() {
        let all = vec![(0, 5, syn_key()), (8, 20, syn_str()), (30, 40, syn_num())];
        assert_eq!(clip_spans(&all, 10, 25), vec![(0, 10, syn_str())]);
        assert_eq!(
            clip_spans(&all, 3, 9),
            vec![(0, 2, syn_key()), (5, 6, syn_str())]
        );
        assert!(clip_spans(&all, 21, 29).is_empty());
    }
}
