use super::*;
use crate::types::{HistoryRequest, HistoryResponse, HttpMethod};
use indexmap::IndexMap;
use std::fs;

fn ts(s: &str) -> i64 {
    parse_date(s).unwrap().timestamp_millis()
}

fn entry(id: &str, timestamp: &str, method: &str, url: &str, status: u16) -> HistoryEntry {
    HistoryEntry {
        id: id.into(),
        timestamp: ts(timestamp),
        request: HistoryRequest {
            method: method.into(),
            url: url.into(),
            headers: IndexMap::new(),
            body: None,
        },
        response: HistoryResponse {
            status,
            status_text: "OK".into(),
            response_time: 42.0,
            size: 1024,
        },
        collection_id: None,
        request_id: None,
    }
}

fn simple(id: &str, timestamp: &str) -> HistoryEntry {
    entry(id, timestamp, "GET", "https://api.example.com/users", 200)
}

fn setup() -> (tempfile::TempDir, HistoryManager) {
    let tmp = tempfile::tempdir().unwrap();
    let m = HistoryManager::new(tmp.path());
    (tmp, m)
}

fn ids(v: &[HistoryEntry]) -> Vec<&str> {
    v.iter().map(|e| e.id.as_str()).collect()
}

mod log {
    use super::*;

    #[test]
    fn creates_the_base_directory_if_it_does_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("deep/nested/history");
        HistoryManager::new(&nested)
            .log(&simple("e1", "2025-06-15T10:00:00Z"))
            .unwrap();
        assert!(nested.is_dir());
    }

    #[test]
    fn appends_an_entry_to_a_dated_jsonl_file() {
        let (tmp, m) = setup();
        m.log(&simple("e1", "2025-06-15T10:00:00Z")).unwrap();
        let content = fs::read_to_string(tmp.path().join("2025-06-15.jsonl")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["id"], "e1");
    }

    #[test]
    fn groups_entries_by_calendar_day() {
        let (tmp, m) = setup();
        m.log(&simple("e1", "2025-06-15T08:00:00Z")).unwrap();
        m.log(&simple("e2", "2025-06-15T20:00:00Z")).unwrap();
        m.log(&simple("e3", "2025-06-16T05:00:00Z")).unwrap();
        let day15 = fs::read_to_string(tmp.path().join("2025-06-15.jsonl")).unwrap();
        assert_eq!(day15.trim().split('\n').count(), 2);
        let day16 = fs::read_to_string(tmp.path().join("2025-06-16.jsonl")).unwrap();
        assert_eq!(day16.trim().split('\n').count(), 1);
    }

    #[test]
    fn round_trips_a_full_entry_through_log_query() {
        let (_t, m) = setup();
        let mut e = simple("round-trip-1", "2025-06-15T10:00:00Z");
        e.collection_id = Some("col-1".into());
        e.request_id = Some("req-1".into());
        e.request.body = Some("{\"key\":\"value\"}".into());
        m.log(&e).unwrap();
        assert_eq!(m.query(&HistoryFilter::default()).unwrap(), vec![e]);
    }

    #[test]
    fn writes_camel_case_json_like_the_ts() {
        let (tmp, m) = setup();
        let mut e = simple("e1", "2025-06-15T10:00:00Z");
        e.collection_id = Some("c".into());
        m.log(&e).unwrap();
        let line = fs::read_to_string(tmp.path().join("2025-06-15.jsonl")).unwrap();
        assert!(line.contains("\"statusText\":\"OK\"") && line.contains("\"responseTime\":42"));
        assert!(line.contains("\"collectionId\":\"c\"") && !line.contains("requestId"));
    }
}

mod query {
    use super::*;

    fn seeded() -> (tempfile::TempDir, HistoryManager) {
        let (t, m) = setup();
        m.log(&entry(
            "e1",
            "2025-06-14T10:00:00Z",
            "GET",
            "https://api.example.com/users",
            200,
        ))
        .unwrap();
        m.log(&entry(
            "e2",
            "2025-06-15T12:00:00Z",
            "POST",
            "https://api.example.com/users",
            201,
        ))
        .unwrap();
        m.log(&entry(
            "e3",
            "2025-06-15T14:00:00Z",
            "GET",
            "https://api.example.com/products",
            404,
        ))
        .unwrap();
        m.log(&entry(
            "e4",
            "2025-06-16T09:00:00Z",
            "DELETE",
            "https://api.example.com/users/1",
            204,
        ))
        .unwrap();
        (t, m)
    }

    fn q(m: &HistoryManager, f: HistoryFilter) -> Vec<HistoryEntry> {
        m.query(&f).unwrap()
    }

    #[test]
    fn returns_all_entries_without_filters() {
        let (_t, m) = seeded();
        assert_eq!(q(&m, HistoryFilter::default()).len(), 4);
    }

    #[test]
    fn returns_entries_newest_first() {
        let (_t, m) = seeded();
        let r = q(&m, HistoryFilter::default());
        assert_eq!(r[0].id, "e4");
        assert_eq!(ids(&r), ["e4", "e3", "e2", "e1"]);
    }

    #[test]
    fn filters_by_method() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                method: Some(HttpMethod::Get),
                ..Default::default()
            },
        );
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|e| e.request.method == "GET"));
    }

    #[test]
    fn filters_by_url_pattern() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                url_pattern: Some("products".into()),
                ..Default::default()
            },
        );
        assert_eq!(ids(&r), ["e3"]);
    }

    #[test]
    fn filters_by_status_code() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                status_code: Some(404),
                ..Default::default()
            },
        );
        assert_eq!(ids(&r), ["e3"]);
    }

    #[test]
    fn filters_by_start_date() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                start_date: Some(ts("2025-06-15")),
                ..Default::default()
            },
        );
        let got = ids(&r);
        assert!(!got.contains(&"e1") && got.contains(&"e2"));
    }

    #[test]
    fn filters_by_end_date() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                end_date: Some(ts("2025-06-15")),
                ..Default::default()
            },
        );
        let got = ids(&r);
        assert!(!got.contains(&"e4") && got.contains(&"e2"));
    }

    #[test]
    fn filters_by_date_range() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                start_date: Some(ts("2025-06-15")),
                end_date: Some(ts("2025-06-15")),
                ..Default::default()
            },
        );
        assert_eq!(ids(&r), ["e3", "e2"]);
    }

    #[test]
    fn combines_method_and_url_pattern() {
        let (_t, m) = seeded();
        let r = q(
            &m,
            HistoryFilter {
                method: Some(HttpMethod::Get),
                url_pattern: Some("users".into()),
                ..Default::default()
            },
        );
        assert_eq!(ids(&r), ["e1"]);
    }

    #[test]
    fn applies_limit_offset_and_both() {
        let (_t, m) = seeded();
        let all = q(&m, HistoryFilter::default());
        assert_eq!(
            q(
                &m,
                HistoryFilter {
                    limit: Some(2),
                    ..Default::default()
                }
            )
            .len(),
            2
        );
        assert_eq!(
            q(
                &m,
                HistoryFilter {
                    offset: Some(2),
                    ..Default::default()
                }
            ),
            all[2..].to_vec()
        );
        let page = q(
            &m,
            HistoryFilter {
                limit: Some(1),
                offset: Some(1),
                ..Default::default()
            },
        );
        assert_eq!(page, vec![all[1].clone()]);
    }

    #[test]
    fn default_limit_is_100() {
        let (_t, m) = setup();
        for i in 0..120 {
            m.log(&simple(&format!("e{i}"), "2025-06-15T10:00:00Z"))
                .unwrap();
        }
        assert_eq!(q(&m, HistoryFilter::default()).len(), 100);
    }

    #[test]
    fn returns_empty_when_nothing_matches() {
        let (_t, m) = seeded();
        assert!(q(
            &m,
            HistoryFilter {
                method: Some(HttpMethod::Patch),
                ..Default::default()
            }
        )
        .is_empty());
    }

    #[test]
    fn returns_empty_for_a_fresh_directory() {
        let (_t, m) = setup();
        assert!(q(&m, HistoryFilter::default()).is_empty());
    }

    #[test]
    fn skips_malformed_json_lines() {
        let (t, m) = seeded();
        fs::write(t.path().join("2025-06-17.jsonl"), "not-json\n{\"bad json\n").unwrap();
        let r = q(&m, HistoryFilter::default());
        assert_eq!(r.len(), 4);
    }
}

mod get_entry {
    use super::*;

    #[test]
    fn retrieves_an_entry_by_id() {
        let (_t, m) = setup();
        m.log(&simple("find-me", "2025-06-15T10:00:00Z")).unwrap();
        m.log(&simple("not-me", "2025-06-15T11:00:00Z")).unwrap();
        assert_eq!(m.get_entry("find-me").unwrap().unwrap().id, "find-me");
    }

    #[test]
    fn searches_across_multiple_day_files() {
        let (_t, m) = setup();
        m.log(&simple("day1", "2025-06-14T10:00:00Z")).unwrap();
        m.log(&simple("day2", "2025-06-16T10:00:00Z")).unwrap();
        assert!(m.get_entry("day1").unwrap().is_some());
        assert!(m.get_entry("day2").unwrap().is_some());
    }

    #[test]
    fn returns_none_for_unknown_id_or_no_files() {
        let (_t, m) = setup();
        assert!(m.get_entry("anything").unwrap().is_none());
        m.log(&simple("exists", "2025-06-15T10:00:00Z")).unwrap();
        assert!(m.get_entry("nope").unwrap().is_none());
    }

    #[test]
    fn skips_malformed_lines() {
        let (t, m) = setup();
        let good = serde_json::to_string(&simple("good", "2025-06-15T10:00:00Z")).unwrap();
        fs::write(
            t.path().join("2025-06-15.jsonl"),
            format!("bad-json\n{good}\n"),
        )
        .unwrap();
        assert!(m.get_entry("good").unwrap().is_some());
    }
}

mod clear {
    use super::*;

    fn jsonl(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|f| f.ends_with(".jsonl"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn removes_all_jsonl_files_without_a_date() {
        let (t, m) = setup();
        m.log(&simple("e1", "2025-06-14T10:00:00Z")).unwrap();
        m.log(&simple("e2", "2025-06-15T10:00:00Z")).unwrap();
        m.clear(None).unwrap();
        assert!(jsonl(t.path()).is_empty());
    }

    #[test]
    fn only_removes_files_strictly_before_the_date() {
        let (t, m) = setup();
        for (id, day) in [("e1", "13"), ("e2", "14"), ("e3", "15")] {
            m.log(&simple(id, &format!("2025-06-{day}T10:00:00Z")))
                .unwrap();
        }
        m.clear(Some(parse_date("2025-06-14").unwrap())).unwrap();
        assert_eq!(jsonl(t.path()), ["2025-06-14.jsonl", "2025-06-15.jsonl"]);
    }

    #[test]
    fn leaves_non_jsonl_files_untouched() {
        let (t, m) = setup();
        m.log(&simple("e1", "2025-06-15T10:00:00Z")).unwrap();
        fs::write(t.path().join("metadata.json"), "{}").unwrap();
        m.clear(None).unwrap();
        assert!(t.path().join("metadata.json").exists());
    }

    #[test]
    fn does_not_fail_on_an_empty_directory() {
        let (_t, m) = setup();
        m.clear(None).unwrap();
    }

    #[test]
    fn entries_are_gone_after_a_full_clear() {
        let (_t, m) = setup();
        m.log(&simple("e1", "2025-06-15T10:00:00Z")).unwrap();
        m.clear(None).unwrap();
        assert!(m.query(&HistoryFilter::default()).unwrap().is_empty());
        assert!(m.get_entry("e1").unwrap().is_none());
    }
}

mod persistence {
    use super::*;

    #[test]
    fn entries_survive_a_new_manager_on_the_same_directory() {
        let (t, m) = setup();
        m.log(&simple("persist-1", "2025-06-15T10:00:00Z")).unwrap();
        let r = HistoryManager::new(t.path())
            .query(&HistoryFilter::default())
            .unwrap();
        assert_eq!(ids(&r), ["persist-1"]);
    }

    #[test]
    fn preserves_all_fields() {
        let (_t, m) = setup();
        let mut e = simple("full-entry", "2025-06-15T10:00:00Z");
        e.collection_id = Some("col-abc".into());
        e.request_id = Some("req-xyz".into());
        e.request = HistoryRequest {
            method: "POST".into(),
            url: "https://api.example.com/submit".into(),
            headers: [
                ("content-type", "application/json"),
                ("authorization", "Bearer tok"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            body: Some("{\"name\":\"test\"}".into()),
        };
        e.response = HistoryResponse {
            status: 201,
            status_text: "Created".into(),
            response_time: 98.0,
            size: 512,
        };
        m.log(&e).unwrap();
        assert_eq!(m.get_entry("full-entry").unwrap(), Some(e));
    }
}

#[test]
fn parse_date_accepts_iso_forms_and_rejects_garbage() {
    assert_eq!(
        parse_date("2025-06-14").unwrap().timestamp_millis(),
        1_749_859_200_000
    );
    assert_eq!(
        parse_date("2025-06-14T00:00:00.000Z")
            .unwrap()
            .timestamp_millis(),
        1_749_859_200_000
    );
    assert!(parse_date("yesterday").is_err());
}
