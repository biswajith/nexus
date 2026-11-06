//! Read-only, virtualised code viewer: only the visible lines are highlighted and laid out
//! (their galleys are cached), so a 10 MB body costs the same per frame as a 10 KB one.

use super::a11y;
use crate::highlight::{code_job, Lang};
use crate::state::response::{LineCache, Match, TextDoc, MAX_LINE_CHARS};
use crate::theme::*;
use eframe::egui::{
    self, accesskit::Role, pos2, text::CCursor, vec2, Align, Color32, FontFamily, FontId, Rect,
    ScrollArea, Sense, Ui,
};

const FONT_SIZE: f32 = 13.0;
pub const ROW_H: f32 = 19.5;
const PAD: f32 = 12.0;

/// What to highlight and where to scroll.
pub struct ViewOpts<'a> {
    /// Accessible name of the whole viewer.
    pub label: &'a str,
    pub lang: Lang,
    pub matches: &'a [Match],
    /// Index into `matches` of the current hit.
    pub current: Option<usize>,
    /// Scroll the current hit into view this frame.
    pub jump: bool,
}

fn match_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(0xf5, 0x9e, 0x0b, 70)
}
fn match_fill_current() -> Color32 {
    Color32::from_rgba_unmultiplied(0xf5, 0x9e, 0x0b, 190)
}

/// Draws `doc` filling the space left in `ui`.
pub fn code_view(ui: &mut Ui, doc: &TextDoc, cache: &mut LineCache, o: &ViewOpts) {
    let area = ui.available_rect_before_wrap();
    ui.painter().rect_filled(area, 0, bg_sunken());
    let mono = FontId::new(FONT_SIZE, FontFamily::Monospace);
    let ppp = ui.ctx().pixels_per_point();
    let epoch = crate::theme::galley_epoch(ui.ctx());
    if (cache.ppp - ppp).abs() > f32::EPSILON || cache.galleys.len() > 3000 || cache.epoch != epoch
    {
        cache.clear();
        cache.ppp = ppp;
        cache.epoch = epoch;
    }
    let char_w = ui.fonts_mut(|f| f.glyph_width(&mono, '0'));
    let n = doc.line_count();
    let digits = n.max(1).to_string().len().max(2);
    let gutter = digits as f32 * char_w + 20.0;
    let content_w = gutter + PAD + (doc.max_cols as f32 + 2.0) * char_w + PAD;
    let content_h = n as f32 * ROW_H + 2.0 * PAD;

    let hit = o.current.and_then(|c| o.matches.get(c)).copied();

    ScrollArea::both()
        .id_salt(("code_view", o.label))
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            let (rect, resp) = ui.allocate_exact_size(
                vec2(content_w.max(viewport.width()), content_h),
                Sense::hover(),
            );
            a11y(resp, Role::Document, o.label);
            if n == 0 {
                ui.painter().text(
                    rect.left_top() + vec2(PAD, PAD),
                    egui::Align2::LEFT_TOP,
                    "(empty body)",
                    FontId::new(12.5, FontFamily::Proportional),
                    fg_3(),
                );
                return;
            }
            let first = (((viewport.min.y - PAD) / ROW_H).floor().max(0.0) as usize).min(n - 1);
            let last = ((((viewport.max.y - PAD) / ROW_H).ceil().max(0.0)) as usize + 1).min(n);
            // gutter background strip stays put while scrolling horizontally
            let gx = rect.left() + viewport.min.x;
            let p = ui.painter().clone();
            p.rect_filled(
                Rect::from_min_max(
                    pos2(gx, ui.clip_rect().top()),
                    pos2(gx + gutter, ui.clip_rect().bottom()),
                ),
                0,
                bg_sunken(),
            );
            for i in first..last {
                let y = rect.top() + PAD + i as f32 * ROW_H;
                let galley = cache
                    .galleys
                    .entry(i)
                    .or_insert_with(|| {
                        let (text, cut) = doc.display_line(i);
                        let mut job = code_job(o.lang, text, FONT_SIZE, ROW_H, f32::INFINITY);
                        if cut {
                            job.append(
                                " … (line cut)",
                                0.0,
                                egui::TextFormat {
                                    font_id: mono.clone(),
                                    color: fg_3(),
                                    line_height: Some(ROW_H),
                                    ..Default::default()
                                },
                            );
                        }
                        ui.fonts_mut(|f| f.layout_job(job))
                    })
                    .clone();
                let x = rect.left() + gutter + PAD;
                // search hits on this line
                let lo = o.matches.partition_point(|m| (m.line as usize) < i);
                for m in o.matches[lo..].iter().take_while(|m| m.line as usize == i) {
                    let line = doc.line(i);
                    let (s, e) = (m.start as usize, (m.start + m.len) as usize);
                    let cs = line[..s].chars().count();
                    if cs >= MAX_LINE_CHARS {
                        continue;
                    }
                    let ce = (cs + line[s..e].chars().count()).min(MAX_LINE_CHARS);
                    let x0 = galley.pos_from_cursor(CCursor::new(cs)).min.x;
                    let x1 = galley.pos_from_cursor(CCursor::new(ce)).min.x;
                    let r = Rect::from_min_max(pos2(x + x0, y), pos2(x + x1, y + ROW_H));
                    let is_cur = hit == Some(*m);
                    p.rect_filled(
                        r.expand2(vec2(1.0, -1.0)),
                        2,
                        if is_cur {
                            match_fill_current()
                        } else {
                            match_fill()
                        },
                    );
                    if is_cur && o.jump {
                        ui.scroll_to_rect(r.expand2(vec2(40.0, 0.0)), None);
                    }
                }
                p.galley(pos2(x, y), galley, fg());
                // line number (right aligned in the gutter)
                p.text(
                    pos2(gx + gutter - 10.0, y + ROW_H / 2.0),
                    egui::Align2::RIGHT_CENTER,
                    i + 1,
                    FontId::new(11.0, FontFamily::Monospace),
                    fg_3(),
                );
                {
                    let row = Rect::from_min_size(pos2(x, y), vec2(content_w - gutter, ROW_H));
                    let line = doc.display_line(i).0;
                    let rr = ui.interact(row, ui.scope_id().with(("line", i)), Sense::hover());
                    a11y(rr, Role::Label, if line.is_empty() { " " } else { line });
                }
            }
            p.vline(
                gx + gutter,
                ui.clip_rect().y_range(),
                egui::Stroke::new(1.0, border_subtle()),
            );
            if o.jump {
                if let Some(m) = hit {
                    let y = rect.top() + PAD + m.line as f32 * ROW_H;
                    let line_rect = Rect::from_min_size(pos2(rect.left(), y), vec2(1.0, ROW_H));
                    let in_view = (m.line as usize) > first && (m.line as usize) + 1 < last;
                    if !in_view {
                        ui.scroll_to_rect(line_rect, Some(Align::Center));
                    }
                }
            }
        });
}
