use crate::fs::{write_private_file, WriteMode};
use crate::types::HeaderValue;
use crate::Result;
use cookie_store::CookieStore;
use indexmap::IndexMap;
use std::path::PathBuf;
use url::Url;

/// A cookie as reported by the jar (what tough-cookie's `Cookie` exposed: key/value/domain/path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieInfo {
    pub key: String,
    pub value: String,
    pub domain: String,
    pub path: String,
}

/// A `cookie_store` jar with optional JSON persistence (file mode 0600) for HTTP requests and
/// responses. Persisted JSON uses `cookie_store`'s format, not tough-cookie's.
#[derive(Debug, Default)]
pub struct NexusCookieJar {
    store: CookieStore,
    persist_path: Option<PathBuf>,
}

fn parse_url(url: &str) -> Result<Url> {
    Url::parse(url).map_err(|e| crate::Error::Other(format!("Invalid URL: {e}")))
}

impl NexusCookieJar {
    /// With `persist_path` set, cookies can be loaded from and saved to that file.
    pub fn new(persist_path: Option<PathBuf>) -> Self {
        Self {
            store: CookieStore::default(),
            persist_path,
        }
    }

    /// Loads the jar from `persist_path`; a missing, unreadable or invalid file gives an empty jar.
    /// No-op without a path.
    pub fn load(&mut self) {
        let Some(path) = &self.persist_path else {
            return;
        };
        self.store = std::fs::File::open(path)
            .ok()
            .and_then(|f| cookie_store::serde::json::load_all(std::io::BufReader::new(f)).ok())
            .unwrap_or_default();
    }

    /// Parses and stores a raw `Set-Cookie` string for `url`, then persists. An unparsable or
    /// rejected cookie (e.g. wrong domain) is ignored, like tough-cookie with `ignoreError`.
    pub fn set_cookie(&mut self, raw_cookie: &str, url: &str) -> Result<()> {
        let url = parse_url(url)?;
        let _ = self.store.parse(raw_cookie, &url);
        self.persist()
    }

    /// All cookies that apply to `url`.
    pub fn get_cookies(&self, url: &str) -> Result<Vec<CookieInfo>> {
        let url = parse_url(url)?;
        Ok(self.store.matches(&url).into_iter().map(info).collect())
    }

    /// `Cookie` header value for a request to `url` (`a=1; b=2`), empty if none.
    pub fn get_cookie_string(&self, url: &str) -> Result<String> {
        let url = parse_url(url)?;
        Ok(self
            .store
            .get_request_values(&url)
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; "))
    }

    /// Every cookie in the jar.
    pub fn get_all_cookies(&self) -> Vec<CookieInfo> {
        self.store.iter_any().map(info).collect()
    }

    /// Removes the cookie identified by domain, path and name, then persists.
    pub fn remove_cookie(&mut self, domain: &str, path: &str, key: &str) -> Result<()> {
        self.store.remove(domain, path, key);
        self.persist()
    }

    pub fn remove_all_cookies(&mut self) -> Result<()> {
        self.store.clear();
        self.persist()
    }

    /// Sets a `cookie` header on `headers` for `url` from the matching cookies (none: untouched).
    pub fn apply_to_request(
        &self,
        url: &str,
        headers: &mut IndexMap<String, String>,
    ) -> Result<()> {
        let cookie_string = self.get_cookie_string(url)?;
        if !cookie_string.is_empty() {
            headers.insert("cookie".to_string(), cookie_string);
        }
        Ok(())
    }

    /// Stores every `set-cookie` header of a response (string or array) for `url`.
    pub fn process_response(
        &mut self,
        url: &str,
        response_headers: &IndexMap<String, HeaderValue>,
    ) -> Result<()> {
        let Some(set_cookies) = response_headers.get("set-cookie") else {
            return Ok(());
        };
        let raws: Vec<&str> = match set_cookies {
            HeaderValue::One(s) => vec![s.as_str()],
            HeaderValue::Many(v) => v.iter().map(String::as_str).collect(),
        };
        for raw in raws {
            self.set_cookie(raw, url)?;
        }
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        let mut buf = Vec::new();
        cookie_store::serde::json::save_incl_expired_and_nonpersistent(&self.store, &mut buf)
            .map_err(|e| crate::Error::Other(format!("Failed to serialize cookies: {e}")))?;
        let json = String::from_utf8(buf).map_err(|e| crate::Error::Other(e.to_string()))?;
        write_private_file(path, &json, WriteMode::Replace)?;
        Ok(())
    }
}

fn info(c: &cookie_store::Cookie<'static>) -> CookieInfo {
    CookieInfo {
        key: c.name().to_string(),
        value: c.value().to_string(),
        domain: c
            .domain
            .as_cow()
            .map(|d| d.into_owned())
            .unwrap_or_default(),
        path: c.path.as_ref().to_string(),
    }
}
