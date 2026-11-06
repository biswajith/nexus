use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;

fn counter() -> (Arc<AtomicUsize>, impl Fn() + Send + Sync + 'static) {
    let c = Arc::new(AtomicUsize::new(0));
    let c2 = c.clone();
    (c, move || {
        c2.fetch_add(1, Ordering::SeqCst);
    })
}

fn populated() -> NexusConsole {
    let mut c = NexusConsole::new();
    c.info(LogSource::Http, "HTTP request sent", None, Some("req-1"));
    c.warn(
        LogSource::Script,
        "Deprecated API used",
        None,
        Some("req-1"),
    );
    c.error(LogSource::System, "Connection failed", None, Some("req-2"));
    c.debug(
        LogSource::Variable,
        "Resolved variable {{host}}",
        None,
        Some("req-2"),
    );
    c
}

#[test]
fn stores_entry_with_all_fields() {
    let mut c = NexusConsole::new();
    c.log(
        LogLevel::Info,
        LogSource::Http,
        "Request sent",
        Some(json!({"url": "/api"})),
        Some("req-1"),
    );
    let entries = c.get_all();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].level, LogLevel::Info);
    assert_eq!(entries[0].source, LogSource::Http);
    assert_eq!(entries[0].message, "Request sent");
    assert_eq!(entries[0].data, Some(json!({"url": "/api"})));
    assert_eq!(entries[0].request_id.as_deref(), Some("req-1"));
}

#[test]
fn assigns_unique_ids() {
    let mut c = NexusConsole::new();
    c.log(LogLevel::Info, LogSource::System, "msg1", None, None);
    c.log(LogLevel::Info, LogSource::System, "msg2", None, None);
    let e = c.get_all();
    assert_ne!(e[0].id, e[1].id);
}

#[test]
fn assigns_timestamp() {
    let mut c = NexusConsole::new();
    let before = now_ms();
    c.log(LogLevel::Info, LogSource::System, "msg", None, None);
    let after = now_ms();
    let ts = c.get_all()[0].timestamp;
    assert!(ts >= before && ts <= after);
}

#[test]
fn evicts_oldest_beyond_max_entries() {
    let mut c = NexusConsole::new();
    for i in 0..10_001 {
        c.log(
            LogLevel::Info,
            LogSource::System,
            format!("msg-{i}"),
            None,
            None,
        );
    }
    let e = c.get_all();
    assert_eq!(e.len(), 10_000);
    assert_eq!(e[0].message, "msg-1");
    assert_eq!(e[e.len() - 1].message, "msg-10000");
}

#[test]
fn notifies_entry_handlers() {
    let mut c = NexusConsole::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    c.on_entry(move |e| seen2.lock().unwrap().push((e.level, e.message.clone())));
    c.log(
        LogLevel::Warn,
        LogSource::Script,
        "test warning",
        None,
        None,
    );
    assert_eq!(
        *seen.lock().unwrap(),
        vec![(LogLevel::Warn, "test warning".to_string())]
    );
}

#[test]
fn convenience_methods_set_level() {
    let mut c = NexusConsole::new();
    c.info(LogSource::Http, "a", None, None);
    c.warn(LogSource::Script, "b", None, None);
    c.error(LogSource::System, "c", None, None);
    c.debug(LogSource::Variable, "d", None, None);
    let levels: Vec<_> = c.get_all().iter().map(|e| e.level).collect();
    assert_eq!(
        levels,
        [
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
            LogLevel::Debug
        ]
    );
}

#[test]
fn convenience_methods_pass_data_and_request_id() {
    let mut c = NexusConsole::new();
    c.info(
        LogSource::Http,
        "msg",
        Some(json!({"detail": 1})),
        Some("req-42"),
    );
    let e = &c.get_all()[0];
    assert_eq!(e.data, Some(json!({"detail": 1})));
    assert_eq!(e.request_id.as_deref(), Some("req-42"));
}

#[test]
fn query_without_filters_returns_all() {
    assert_eq!(populated().query(&ConsoleFilter::default()).len(), 4);
}

#[test]
fn query_by_level() {
    let r = populated().query(&ConsoleFilter {
        level: Some(LogLevel::Warn),
        ..Default::default()
    });
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].message, "Deprecated API used");
}

#[test]
fn query_by_source() {
    let r = populated().query(&ConsoleFilter {
        source: Some(LogSource::Http),
        ..Default::default()
    });
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].message, "HTTP request sent");
}

#[test]
fn query_by_request_id() {
    let r = populated().query(&ConsoleFilter {
        request_id: Some("req-2".into()),
        ..Default::default()
    });
    assert_eq!(r.len(), 2);
}

#[test]
fn query_by_search_is_case_insensitive() {
    let r = populated().query(&ConsoleFilter {
        search: Some("connection".into()),
        ..Default::default()
    });
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].message, "Connection failed");
}

#[test]
fn query_combines_filters() {
    let r = populated().query(&ConsoleFilter {
        level: Some(LogLevel::Debug),
        request_id: Some("req-2".into()),
        ..Default::default()
    });
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].source, LogSource::Variable);
}

#[test]
fn query_with_no_matches_is_empty() {
    let r = populated().query(&ConsoleFilter {
        level: Some(LogLevel::Info),
        source: Some(LogSource::Script),
        ..Default::default()
    });
    assert!(r.is_empty());
}

#[test]
fn get_all_returns_a_copy() {
    let mut c = NexusConsole::new();
    c.info(LogSource::System, "msg", None, None);
    let mut all = c.get_all();
    all.clear();
    assert_eq!(c.get_all().len(), 1);
}

#[test]
fn clear_removes_entries_and_notifies() {
    let mut c = NexusConsole::new();
    c.info(LogSource::System, "msg1", None, None);
    c.info(LogSource::System, "msg2", None, None);
    let (count, handler) = counter();
    c.on_clear(handler);
    c.clear();
    assert!(c.get_all().is_empty());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn multiple_entry_handlers_and_unsubscribe() {
    let mut c = NexusConsole::new();
    let (n1, h1) = counter();
    let (n2, h2) = counter();
    let id1 = c.on_entry(move |_| h1());
    c.on_entry(move |_| h2());

    c.info(LogSource::System, "first", None, None);
    assert_eq!(
        (n1.load(Ordering::SeqCst), n2.load(Ordering::SeqCst)),
        (1, 1)
    );

    c.remove_handler(id1);
    c.info(LogSource::System, "second", None, None);
    assert_eq!(
        (n1.load(Ordering::SeqCst), n2.load(Ordering::SeqCst)),
        (1, 2)
    );
}

#[test]
fn multiple_clear_handlers_and_unsubscribe() {
    let mut c = NexusConsole::new();
    let (n1, h1) = counter();
    let (n2, h2) = counter();
    let id1 = c.on_clear(h1);
    c.on_clear(h2);

    c.clear();
    assert_eq!(
        (n1.load(Ordering::SeqCst), n2.load(Ordering::SeqCst)),
        (1, 1)
    );

    c.remove_handler(id1);
    c.clear();
    assert_eq!(
        (n1.load(Ordering::SeqCst), n2.load(Ordering::SeqCst)),
        (1, 2)
    );
}
