use super::*;

fn ct(value: &str) -> IndexMap<String, HeaderValue> {
    [(
        "content-type".to_string(),
        HeaderValue::One(value.to_string()),
    )]
    .into_iter()
    .collect()
}

#[test]
fn detects_content_types() {
    use ResponseFormat::*;
    let cases = [
        ("application/json", Json),
        ("application/json; charset=utf-8", Json),
        ("text/json", Json),
        ("text/html", Html),
        ("text/html; charset=utf-8", Html),
        ("application/xml", Xml),
        ("text/xml", Xml),
        ("image/png", Image),
        ("image/jpeg", Image),
        ("image/svg+xml", Xml),
        ("application/pdf", Pdf),
        ("application/octet-stream", Binary),
        ("text/plain", Text),
        ("application/yaml", Text),
        ("APPLICATION/JSON", Json),
        ("Text/HTML", Html),
    ];
    for (header, expected) in cases {
        assert_eq!(detect_content_type(&ct(header)), expected, "{header}");
    }
}

#[test]
fn missing_content_type_is_text() {
    assert_eq!(detect_content_type(&IndexMap::new()), ResponseFormat::Text);
}

#[test]
fn array_header_values_are_joined() {
    let headers: IndexMap<String, HeaderValue> = [(
        "content-type".to_string(),
        HeaderValue::Many(vec!["application/json".into(), "text/plain".into()]),
    )]
    .into_iter()
    .collect();
    assert_eq!(detect_content_type(&headers), ResponseFormat::Json);
}

#[test]
fn format_json_pretty_prints() {
    let r = format_json(r#"{"a":1,"b":[2,3]}"#);
    assert_eq!(r.error, None);
    assert_eq!(
        r.formatted,
        "{\n  \"a\": 1,\n  \"b\": [\n    2,\n    3\n  ]\n}"
    );
}

#[test]
fn format_json_invalid_returns_original_with_error() {
    let r = format_json("not-json");
    assert_eq!(r.formatted, "not-json");
    assert_eq!(r.error.as_deref(), Some("Invalid JSON"));
}

#[test]
fn format_json_empty_containers() {
    assert_eq!(
        format_json("{}"),
        FormattedJson {
            formatted: "{}".into(),
            error: None
        }
    );
    assert_eq!(
        format_json("[]"),
        FormattedJson {
            formatted: "[]".into(),
            error: None
        }
    );
}

#[test]
fn format_json_empty_string_is_invalid() {
    let r = format_json("");
    assert_eq!(r.formatted, "");
    assert_eq!(r.error.as_deref(), Some("Invalid JSON"));
}

#[test]
fn format_json_nested_indentation() {
    let r = format_json(r#"{"user":{"name":"Alice","tags":["admin","dev"]}}"#);
    assert_eq!(r.error, None);
    assert!(r.formatted.contains("  \"user\""));
    assert!(r.formatted.contains("    \"name\""));
}

#[test]
fn format_size_units() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1023), "1023 B");
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1536), "1.5 KB");
    assert_eq!(format_size(1024 * 1023), "1023.0 KB");
    assert_eq!(format_size(1024 * 1024), "1.00 MB");
    assert_eq!(format_size((1024.0 * 1024.0 * 5.25) as u64), "5.25 MB");
    assert_eq!(format_size(2048), "2.0 KB");
    assert_eq!(format_size(1_048_576), "1.00 MB");
}

#[test]
fn format_duration_values() {
    assert_eq!(format_duration(0.0), "0 ms");
    assert_eq!(format_duration(42.0), "42 ms");
    assert_eq!(format_duration(999.0), "999 ms");
    assert_eq!(format_duration(42.7), "43 ms");
    assert_eq!(format_duration(0.4), "0 ms");
    assert_eq!(format_duration(1000.0), "1.00 s");
    assert_eq!(format_duration(1234.0), "1.23 s");
    assert_eq!(format_duration(65000.0), "65.00 s");
}

fn timing() -> ResponseTiming {
    ResponseTiming {
        dns: 10.0,
        tcp: 20.0,
        tls: 15.0,
        ttfb: 50.0,
        download: 5.0,
        total: 100.0,
    }
}

#[test]
fn waterfall_has_five_ordered_segments() {
    let s = build_timing_waterfall(&timing());
    let labels: Vec<_> = s.iter().map(|s| s.label).collect();
    assert_eq!(labels, ["DNS", "TCP", "TLS", "TTFB", "Download"]);
}

#[test]
fn waterfall_cumulative_offsets_and_durations() {
    let s = build_timing_waterfall(&timing());
    let starts: Vec<_> = s.iter().map(|s| s.start_ms).collect();
    let durations: Vec<_> = s.iter().map(|s| s.duration_ms).collect();
    assert_eq!(starts, [0.0, 10.0, 30.0, 45.0, 95.0]);
    assert_eq!(durations, [10.0, 20.0, 15.0, 50.0, 5.0]);
}

#[test]
fn waterfall_colors_are_distinct() {
    let s = build_timing_waterfall(&timing());
    let colors: std::collections::HashSet<_> = s.iter().map(|s| s.color).collect();
    assert_eq!(colors.len(), 5);
}

#[test]
fn waterfall_zero_duration_phases() {
    let zero = ResponseTiming {
        dns: 0.0,
        tcp: 0.0,
        tls: 0.0,
        ttfb: 100.0,
        download: 0.0,
        total: 100.0,
    };
    let s = build_timing_waterfall(&zero);
    assert_eq!((s[0].start_ms, s[0].duration_ms), (0.0, 0.0));
    assert_eq!((s[3].start_ms, s[3].duration_ms), (0.0, 100.0));
    assert_eq!(s[4].start_ms, 100.0);
}

#[test]
fn status_colors() {
    for s in [200, 201, 204, 299] {
        assert_eq!(get_status_color(s), "positive");
    }
    for s in [301, 304, 399] {
        assert_eq!(get_status_color(s), "informative");
    }
    for s in [400, 404, 422, 499] {
        assert_eq!(get_status_color(s), "notice");
    }
    for s in [500, 502, 503] {
        assert_eq!(get_status_color(s), "negative");
    }
    for s in [100, 101, 199] {
        assert_eq!(get_status_color(s), "neutral");
    }
}
