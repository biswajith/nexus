//! Two-way sync between the URL's query string and the Params table.
//!
//! Editing the URL rebuilds the enabled params; editing the params rebuilds the URL's query.
//! Disabled params live only in the table. `{{variable}}` tokens are never percent-encoded so
//! they still resolve at send time.

use nexus_core::types::KeyValuePair;

/// Splits `url` into (everything before `?`, query without `?`, fragment including `#`).
pub fn split(url: &str) -> (&str, Option<&str>, &str) {
    let (rest, frag) = match url.find('#') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };
    match rest.find('?') {
        Some(i) => (&rest[..i], Some(&rest[i + 1..]), frag),
        None => (rest, None, frag),
    }
}

/// Percent-decodes `s` (`+` stays a plus); malformed escapes are kept verbatim.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/// `encodeURIComponent`, except `{{variable}}` tokens are copied verbatim.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(start) = rest.find("{{") {
            if let Some(len) = rest[start..].find("}}") {
                out.push_str(&encode_plain(&rest[..start]));
                out.push_str(&rest[start..start + len + 2]);
                rest = &rest[start + len + 2..];
                continue;
            }
        }
        out.push_str(&encode_plain(rest));
        break;
    }
    out
}

fn encode_plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decoded `(key, value)` pairs of a query string (empty segments skipped).
pub fn parse_query(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|s| !s.is_empty())
        .map(|s| match s.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(s), String::new()),
        })
        .collect()
}

/// A param that takes part in the query string: enabled with a key.
pub fn is_query_param(p: &KeyValuePair) -> bool {
    p.enabled && !p.key.is_empty()
}

pub fn enabled_pairs(params: &[KeyValuePair]) -> Vec<(String, String)> {
    params
        .iter()
        .filter(|p| is_query_param(p))
        .map(|p| (p.key.clone(), p.value.clone()))
        .collect()
}

/// The URL with its query replaced by the enabled params (fragment kept).
pub fn url_with_params(url: &str, params: &[KeyValuePair]) -> String {
    let (base, _, frag) = split(url);
    let pairs = enabled_pairs(params);
    let mut out = base.to_string();
    if !pairs.is_empty() {
        out.push('?');
        let q: Vec<String> = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
            .collect();
        out.push_str(&q.join("&"));
    }
    out.push_str(frag);
    out
}

/// Whether the URL's query already equals the enabled params (the synced state).
pub fn query_matches(url: &str, params: &[KeyValuePair]) -> bool {
    let q = split(url).1.map(parse_query).unwrap_or_default();
    q == enabled_pairs(params)
}

/// Params after the URL was edited: enabled params follow the query, disabled and blank rows stay.
pub fn params_from_url(url: &str, existing: &[KeyValuePair]) -> Vec<KeyValuePair> {
    let parsed = split(url).1.map(parse_query).unwrap_or_default();
    if parsed == enabled_pairs(existing) {
        return existing.to_vec();
    }
    let mut out: Vec<KeyValuePair> = Vec::new();
    let mut it = parsed.into_iter();
    let mut insert_at = None;
    for row in existing {
        if is_query_param(row) {
            if let Some((k, v)) = it.next() {
                out.push(KeyValuePair {
                    key: k,
                    value: v,
                    enabled: true,
                    description: row.description.clone(),
                });
                insert_at = Some(out.len());
            }
        } else {
            out.push(row.clone());
        }
    }
    let at = insert_at.unwrap_or_else(|| {
        // No enabled row consumed: put new params before trailing blank rows.
        out.iter()
            .rposition(|r| !(r.key.is_empty() && r.value.is_empty() && r.enabled))
            .map_or(0, |i| i + 1)
    });
    let extra: Vec<KeyValuePair> = it
        .map(|(k, v)| KeyValuePair {
            key: k,
            value: v,
            enabled: true,
            description: None,
        })
        .collect();
    out.splice(at..at, extra);
    out
}

/// The URL that is sent: the URL as is when its query mirrors the params, else (requests saved
/// by older Nexus versions keep the query only in the params) the enabled params appended.
pub fn effective_url(url: &str, params: &[KeyValuePair]) -> String {
    if enabled_pairs(params).is_empty() || query_matches(url, params) {
        return url.to_string();
    }
    let (base, query, frag) = split(url);
    let mut out = base.to_string();
    out.push('?');
    let mut parts: Vec<String> = Vec::new();
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        parts.push(q.to_string());
    }
    for (k, v) in enabled_pairs(params) {
        parts.push(format!("{}={}", encode_component(&k), encode_component(&v)));
    }
    out.push_str(&parts.join("&"));
    out.push_str(frag);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(k: &str, v: &str, enabled: bool) -> KeyValuePair {
        KeyValuePair {
            key: k.into(),
            value: v.into(),
            enabled,
            description: None,
        }
    }

    #[test]
    fn splits_and_parses() {
        assert_eq!(
            split("http://h/p?a=1&b=2#x"),
            ("http://h/p", Some("a=1&b=2"), "#x")
        );
        assert_eq!(split("http://h/p"), ("http://h/p", None, ""));
        assert_eq!(
            parse_query("a=1&b=%20x&c&&d=%zz"),
            vec![
                ("a".into(), "1".into()),
                ("b".into(), " x".into()),
                ("c".into(), "".into()),
                ("d".into(), "%zz".into())
            ]
        );
    }

    #[test]
    fn variables_are_not_encoded() {
        assert_eq!(encode_component("a b{{x y}}&"), "a%20b{{x y}}%26");
        assert_eq!(encode_component("{{open"), "%7B%7Bopen");
    }

    #[test]
    fn params_edit_rewrites_the_query_keeping_fragment() {
        let p = vec![
            kv("a", "1 2", true),
            kv("off", "x", false),
            kv("", "", true),
        ];
        assert_eq!(
            url_with_params("http://h/p?old=1#f", &p),
            "http://h/p?a=1%202#f"
        );
        assert_eq!(url_with_params("http://h/p", &[]), "http://h/p");
    }

    #[test]
    fn url_edit_updates_enabled_params_and_keeps_disabled() {
        let mut d = kv("a", "1", true);
        d.description = Some("desc".into());
        let existing = vec![d, kv("off", "x", false), kv("", "", true)];
        let out = params_from_url("http://h?a=2&b=3", &existing);
        let flat: Vec<_> = out
            .iter()
            .map(|p| (p.key.as_str(), p.value.as_str(), p.enabled))
            .collect();
        assert_eq!(
            flat,
            vec![
                ("a", "2", true),
                ("b", "3", true),
                ("off", "x", false),
                ("", "", true)
            ]
        );
        assert_eq!(out[0].description.as_deref(), Some("desc"));
        // removing the query drops the enabled rows only
        let out = params_from_url("http://h", &out);
        let flat: Vec<_> = out.iter().map(|p| (p.key.as_str(), p.enabled)).collect();
        assert_eq!(flat, vec![("off", false), ("", true)]);
        // unchanged query leaves the table alone
        assert_eq!(
            params_from_url("http://h?a=1", &[kv("a", "1", true)]),
            vec![kv("a", "1", true)]
        );
    }

    #[test]
    fn effective_url_appends_only_unsynced_params() {
        let p = vec![kv("a", "1", true)];
        assert_eq!(effective_url("http://h?a=1", &p), "http://h?a=1");
        assert_eq!(effective_url("http://h", &p), "http://h?a=1");
        assert_eq!(effective_url("http://h?x=9#f", &p), "http://h?x=9&a=1#f");
        assert_eq!(
            effective_url("http://h?x=9", &[kv("a", "1", false)]),
            "http://h?x=9"
        );
    }
}
