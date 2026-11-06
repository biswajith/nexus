//! A finished request as the response viewer shows it: pre-computed text documents (line index,
//! pretty-printed JSON), decoded images and the per-response view state.

use eframe::egui::{self, ColorImage, TextureHandle};
use nexus_core::response::{detect_content_type, format_size};
use nexus_core::scripts::{LogEntry, TestResult};
use nexus_core::service::HttpSendResponse;
use nexus_core::types::{HeaderValue, ResponseFormat, ResponseTiming};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Display cap per line: a minified 10 MB body is one line, which must never reach the text
/// layouter in full. Copy and Save always use the complete text.
pub const MAX_LINE_CHARS: usize = 4000;

/// Text with a line index, so a viewer can slice out the visible lines in O(1).
pub struct TextDoc {
    pub text: String,
    /// Byte offset of the start of every line.
    starts: Vec<u32>,
    /// Longest line in columns (capped at [`MAX_LINE_CHARS`] + 1).
    pub max_cols: usize,
    lower: OnceLock<String>,
}

impl TextDoc {
    pub fn new(text: String) -> Self {
        let b = text.as_bytes();
        let mut starts = Vec::with_capacity(b.len() / 24 + 1);
        let mut max_cols = 0usize;
        if !b.is_empty() {
            starts.push(0u32);
        }
        let mut line_start = 0usize;
        for (i, &c) in b.iter().enumerate() {
            if c == b'\n' {
                max_cols = max_cols.max((i - line_start).min(MAX_LINE_CHARS + 1));
                line_start = i + 1;
                if line_start < b.len() {
                    starts.push(line_start as u32);
                }
            }
        }
        if line_start < b.len() {
            max_cols = max_cols.max((b.len() - line_start).min(MAX_LINE_CHARS + 1));
        }
        Self {
            text,
            starts,
            max_cols,
            lower: OnceLock::new(),
        }
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Line `i` without its line terminator.
    pub fn line(&self, i: usize) -> &str {
        let s = self.starts[i] as usize;
        let e = self
            .starts
            .get(i + 1)
            .map_or(self.text.len(), |&n| n as usize);
        self.text[s..e].trim_end_matches(['\n', '\r'])
    }

    /// Line `i` cut to [`MAX_LINE_CHARS`] characters; the flag says it was cut.
    pub fn display_line(&self, i: usize) -> (&str, bool) {
        let l = self.line(i);
        match l.char_indices().nth(MAX_LINE_CHARS) {
            Some((end, _)) => (&l[..end], true),
            None => (l, false),
        }
    }

    /// Index of the line holding byte `off`.
    pub fn line_of(&self, off: usize) -> usize {
        self.starts
            .partition_point(|&s| (s as usize) <= off)
            .saturating_sub(1)
    }

    pub fn line_start(&self, i: usize) -> usize {
        self.starts[i] as usize
    }

    /// ASCII-lowercased copy (byte offsets match `text`), built on first use.
    fn lowered(&self) -> &str {
        self.lower.get_or_init(|| self.text.to_ascii_lowercase())
    }

    /// Case-insensitive (ASCII) matches of `needle`, at most `limit`.
    pub fn find_all(&self, needle: &str, limit: usize) -> Vec<Match> {
        if needle.is_empty() || needle.contains('\n') {
            return Vec::new();
        }
        let n = needle.to_ascii_lowercase();
        let mut out = Vec::new();
        for (off, _) in self.lowered().match_indices(&n) {
            let line = self.line_of(off);
            out.push(Match {
                line: line as u32,
                start: (off - self.line_start(line)) as u32,
                len: n.len() as u32,
            });
            if out.len() >= limit {
                break;
            }
        }
        out
    }
}

/// A search hit: byte range within a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub line: u32,
    pub start: u32,
    pub len: u32,
}

pub const MAX_MATCHES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Network,
    Timeout,
    Cancelled,
    Script,
}

#[derive(Clone, Debug)]
pub struct RespError {
    pub kind: ErrorKind,
    pub message: String,
}

impl RespError {
    pub fn title(&self) -> &'static str {
        match self.kind {
            ErrorKind::Network => "Could not get a response",
            ErrorKind::Timeout => "Request timed out",
            ErrorKind::Cancelled => "Request cancelled",
            ErrorKind::Script => "Pre-request script failed",
        }
    }

    /// Short label for the status pill.
    pub fn pill(&self) -> &'static str {
        match self.kind {
            ErrorKind::Network => "Error",
            ErrorKind::Timeout => "Timeout",
            ErrorKind::Cancelled => "Cancelled",
            ErrorKind::Script => "Script error",
        }
    }
}

/// Raw bytes of a non-text body (image / pdf / binary) and, for images, the decoded pixels.
pub struct BinaryBody {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub image: Option<ColorImage>,
}

/// Which body view is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Pretty,
    Raw,
    Preview,
}

/// Per-response view state (search, texture, galley cache). Reset by every new response.
#[derive(Default)]
pub struct BodyView {
    pub mode: ViewMode,
    pub search_open: bool,
    pub search: String,
    pub focus_search: bool,
    pub matches: Vec<Match>,
    /// Query the current `matches` were computed for.
    pub match_key: Option<(String, ViewMode)>,
    pub current: usize,
    pub jump: bool,
    pub cache: LineCache,
    pub texture: Option<TextureHandle>,
    /// Transient one-line message under the toolbar ("Saved to ...", "Copied").
    pub notice: Option<String>,
}

/// Galleys of the lines drawn so far (highlighting happens once per line and response).
#[derive(Default)]
pub struct LineCache {
    pub ppp: f32,
    pub epoch: u64,
    pub galleys: HashMap<usize, Arc<egui::Galley>>,
}

impl LineCache {
    pub fn clear(&mut self) {
        self.galleys.clear();
    }
}

pub struct Resp {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub format: ResponseFormat,
    pub content_type: String,
    /// Body as received (lossy UTF-8).
    pub raw: Arc<TextDoc>,
    /// Pretty-printed JSON; the same document as `raw` for other formats.
    pub pretty: Arc<TextDoc>,
    pub binary: Option<BinaryBody>,
    pub timing: ResponseTiming,
    pub size: u64,
    pub tests: Vec<TestResult>,
    pub visualizer_html: Option<String>,
    pub error: Option<RespError>,
    pub view: BodyView,
}

impl Resp {
    fn empty() -> Self {
        let none = Arc::new(TextDoc::new(String::new()));
        Self {
            status: 0,
            status_text: String::new(),
            headers: Vec::new(),
            format: ResponseFormat::Text,
            content_type: String::new(),
            raw: none.clone(),
            pretty: none,
            binary: None,
            timing: ResponseTiming::default(),
            size: 0,
            tests: Vec::new(),
            visualizer_html: None,
            error: None,
            view: BodyView::default(),
        }
    }

    /// Converts a service response. Heavy (pretty-printing, indexing, image decoding): call off
    /// the UI thread. Also returns the script logs for the console.
    pub fn from_http(r: HttpSendResponse) -> (Self, Vec<LogEntry>) {
        let mut out = Self::empty();
        let logs = r.script_logs;
        let mut format = detect_content_type(&r.headers);
        let content_type = r
            .headers
            .get("content-type")
            .map(header_text)
            .unwrap_or_default();
        // SVG is an image to the header sniffer but text to us.
        if format == ResponseFormat::Image && content_type.to_ascii_lowercase().contains("svg") {
            format = ResponseFormat::Xml;
        }
        let mut headers: Vec<(String, String)> = r
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), header_text(v)))
            .collect();
        headers.sort_by_key(|(k, _)| k.to_lowercase());
        out.status = r.status;
        out.status_text = r.status_text.clone();
        out.headers = headers;
        out.format = format;
        out.content_type = content_type.clone();
        out.timing = r.timing;
        out.size = r.size;
        out.tests = r.test_results;
        out.visualizer_html = r.visualizer_html;

        if r.status == 0 {
            // pre-request script failure: the service puts the message in the body
            out.error = Some(RespError {
                kind: ErrorKind::Script,
                message: r.body_text,
            });
            return (out, logs);
        }

        match (format, r.body_bytes) {
            (ResponseFormat::Image | ResponseFormat::Pdf | ResponseFormat::Binary, Some(bytes)) => {
                let image = (format == ResponseFormat::Image)
                    .then(|| decode_image(&bytes))
                    .flatten();
                out.binary = Some(BinaryBody {
                    bytes,
                    content_type,
                    image,
                });
            }
            _ => {
                let pretty = match (format, &r.body_json) {
                    (ResponseFormat::Json, Some(v)) => serde_json::to_string_pretty(v).ok(),
                    _ => None,
                };
                let raw = Arc::new(TextDoc::new(r.body_text));
                out.pretty = match pretty {
                    Some(p) => Arc::new(TextDoc::new(p)),
                    None => raw.clone(),
                };
                out.raw = raw;
            }
        }
        (out, logs)
    }

    pub fn from_error(e: String) -> Self {
        let lower = e.to_lowercase();
        let kind = if lower.contains("timed out") || lower.contains("timeout") {
            ErrorKind::Timeout
        } else {
            ErrorKind::Network
        };
        Self::failed(kind, e)
    }

    pub fn cancelled() -> Self {
        Self::failed(ErrorKind::Cancelled, "The request was cancelled.".into())
    }

    fn failed(kind: ErrorKind, message: String) -> Self {
        let mut r = Self::empty();
        r.status_text = message.clone();
        r.error = Some(RespError { kind, message });
        r
    }

    pub fn is_binary(&self) -> bool {
        self.binary.is_some()
    }

    /// Document shown for the current view mode.
    pub fn doc(&self) -> &Arc<TextDoc> {
        if self.view.mode == ViewMode::Raw {
            &self.raw
        } else {
            &self.pretty
        }
    }

    /// One-line description for the console.
    pub fn summary(&self) -> String {
        match &self.error {
            Some(e) => format!("{}: {}", e.title(), first_line(&e.message)),
            None => format!(
                "{} {} ({}, {})",
                self.status,
                self.status_text,
                crate::widgets::fmt_dur(self.timing.total),
                format_size(self.size)
            ),
        }
    }
}

pub fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

fn header_text(v: &HeaderValue) -> String {
    match v {
        HeaderValue::One(s) => s.clone(),
        HeaderValue::Many(m) => m.join(", "),
    }
}

/// Decodes an image into pixels, unless it is too large to be worth a texture. Dimensions are read
/// from the header first and the decoder gets matching limits, so a small file claiming huge
/// dimensions (an image bomb) is refused before any pixel buffer is allocated.
pub(crate) fn decode_image(bytes: &[u8]) -> Option<ColorImage> {
    use image::{ImageReader, Limits};
    let reader = || {
        ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()
    };
    let (w, h) = reader()?.into_dimensions().ok()?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 || u64::from(w) * u64::from(h) > 16_000_000 {
        return None;
    }
    let mut r = reader()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    r.limits(limits);
    let rgba = r.decode().ok()?.to_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_image_refuses_header_claiming_huge_dimensions() {
        // PNG signature + IHDR claiming 60000x60000 (no pixel data follows).
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&60000u32.to_be_bytes());
        png.extend_from_slice(&60000u32.to_be_bytes());
        png.extend_from_slice(&[8, 0, 0, 0, 0]);
        png.extend_from_slice(&[0, 0, 0, 0]); // bogus CRC
        assert!(decode_image(&png).is_none());
    }

    #[test]
    fn text_doc_indexes_lines_and_finds_matches() {
        let d = TextDoc::new("ab\r\nAbc\n\nlast".into());
        assert_eq!(d.line_count(), 4);
        assert_eq!(d.line(0), "ab");
        assert_eq!(d.line(1), "Abc");
        assert_eq!(d.line(2), "");
        assert_eq!(d.line(3), "last");
        let m = d.find_all("AB", 10);
        assert_eq!(m.len(), 2);
        assert_eq!((m[1].line, m[1].start), (1, 0));
        assert_eq!(d.line_of(d.text.len() - 1), 3);
        assert!(d.find_all("", 10).is_empty());
    }

    #[test]
    fn trailing_newline_adds_no_line_and_long_lines_are_cut() {
        assert_eq!(TextDoc::new("a\nb\n".into()).line_count(), 2);
        assert_eq!(TextDoc::new(String::new()).line_count(), 0);
        let d = TextDoc::new("é".repeat(MAX_LINE_CHARS + 5));
        let (l, cut) = d.display_line(0);
        assert!(cut);
        assert_eq!(l.chars().count(), MAX_LINE_CHARS);
    }

    #[test]
    fn errors_are_classified() {
        assert_eq!(
            Resp::from_error("operation timed out".into())
                .error
                .unwrap()
                .kind,
            ErrorKind::Timeout
        );
        assert_eq!(
            Resp::from_error("connection refused".into())
                .error
                .unwrap()
                .kind,
            ErrorKind::Network
        );
        assert_eq!(Resp::cancelled().error.unwrap().kind, ErrorKind::Cancelled);
    }
}
