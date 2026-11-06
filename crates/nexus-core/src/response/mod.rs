//! Port of `packages/core/src/response/response-formatter.ts`.

use indexmap::IndexMap;
use serde::Serialize;

use crate::types::{HeaderValue, ResponseFormat, ResponseTiming};

/// Infers a display format from the `content-type` header. The header name must already be
/// lower-case (as the HTTP client produces it); array values are joined with `,` like JS `String()`.
pub fn detect_content_type(headers: &IndexMap<String, HeaderValue>) -> ResponseFormat {
    let ct = match headers.get("content-type") {
        Some(HeaderValue::One(s)) => s.to_lowercase(),
        Some(HeaderValue::Many(v)) => v.join(",").to_lowercase(),
        None => String::new(),
    };
    if ct.contains("json") {
        ResponseFormat::Json
    } else if ct.contains("html") {
        ResponseFormat::Html
    } else if ct.contains("xml") {
        ResponseFormat::Xml
    } else if ct.contains("image/") {
        ResponseFormat::Image
    } else if ct.contains("pdf") {
        ResponseFormat::Pdf
    } else if ct.contains("octet-stream") {
        ResponseFormat::Binary
    } else {
        ResponseFormat::Text
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedJson {
    pub formatted: String,
    pub error: Option<String>,
}

/// Pretty-prints JSON with 2-space indentation; on parse failure returns the input and an error.
pub fn format_json(raw: &str) -> FormattedJson {
    match serde_json::from_str::<serde_json::Value>(raw)
        .and_then(|v| serde_json::to_string_pretty(&v))
    {
        Ok(formatted) => FormattedJson {
            formatted,
            error: None,
        },
        Err(_) => FormattedJson {
            formatted: raw.to_string(),
            error: Some("Invalid JSON".to_string()),
        },
    }
}

/// `512 B`, `1.5 KB`, `5.25 MB`.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// `42 ms` or `1.23 s`.
pub fn format_duration(ms: f64) -> String {
    if ms < 1000.0 {
        // JS Math.round rounds half up.
        format!("{} ms", (ms + 0.5).floor())
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingWaterfallSegment {
    pub label: &'static str,
    pub start_ms: f64,
    pub duration_ms: f64,
    pub color: &'static str,
}

/// Ordered waterfall segments (DNS, TCP, TLS, TTFB, Download) with cumulative start offsets.
pub fn build_timing_waterfall(timing: &ResponseTiming) -> Vec<TimingWaterfallSegment> {
    let steps = [
        ("DNS", timing.dns, "#8b5cf6"),
        ("TCP", timing.tcp, "#3b82f6"),
        ("TLS", timing.tls, "#f59e0b"),
        ("TTFB", timing.ttfb, "#10b981"),
        ("Download", timing.download, "#6366f1"),
    ];
    let mut offset = 0.0;
    steps
        .into_iter()
        .map(|(label, duration, color)| {
            let seg = TimingWaterfallSegment {
                label,
                start_ms: offset,
                duration_ms: duration,
                color,
            };
            offset += duration;
            seg
        })
        .collect()
}

/// Maps an HTTP status code to a semantic color token.
pub fn get_status_color(status: u16) -> &'static str {
    match status {
        200..=299 => "positive",
        300..=399 => "informative",
        400..=499 => "notice",
        500.. => "negative",
        _ => "neutral",
    }
}

#[cfg(test)]
mod tests;
