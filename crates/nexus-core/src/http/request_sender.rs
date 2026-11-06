use crate::types::{HeaderValue, HttpMethod, NexusResponse, RequestSettings, ResponseTiming};
use crate::{Error, Result};
use indexmap::IndexMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue as ReqHeaderValue};
use reqwest::{Method, StatusCode};
use std::time::{Duration, Instant};
use url::Url;

/// Request body as the sender sees it: the UI already turned every body mode into text, and the
/// runner may hand over raw bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum SendBody {
    Text(String),
    Bytes(Vec<u8>),
}

impl SendBody {
    fn into_bytes(self) -> Vec<u8> {
        match self {
            SendBody::Text(s) => s.into_bytes(),
            SendBody::Bytes(b) => b,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SendOptions {
    pub method: HttpMethod,
    pub url: String,
    pub headers: IndexMap<String, String>,
    pub body: Option<SendBody>,
    pub settings: RequestSettings,
}

/// Request headers that carry no credentials and are kept on a cross-origin redirect.
const CROSS_ORIGIN_SAFE_HEADERS: &[&str] = &[
    "accept",
    "accept-encoding",
    "accept-language",
    "cache-control",
    "content-language",
    "content-type",
    "pragma",
    "user-agent",
];

/// Removes every header not on the cross-origin allowlist (case-insensitive).
///
/// Redirects are followed by hand so credentials can be dropped when a redirect leaves the
/// original origin: Nexus sends credentials in arbitrary headers (API-key auth,
/// `x-amz-security-token`, user-defined headers), not only `Authorization`/`Cookie`.
pub fn strip_cross_origin_headers(headers: &IndexMap<String, String>) -> IndexMap<String, String> {
    headers
        .iter()
        .filter(|(k, _)| CROSS_ORIGIN_SAFE_HEADERS.contains(&k.to_ascii_lowercase().as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn build_header_map(headers: &IndexMap<String, String>) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for (k, v) in headers {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| Error::Other(format!("Invalid header name \"{k}\": {e}")))?;
        let value = ReqHeaderValue::from_str(v)
            .map_err(|e| Error::Other(format!("Invalid value for header \"{k}\": {e}")))?;
        map.insert(name, value);
    }
    Ok(map)
}

/// Sends an HTTP request, records timing, and parses the response body (including JSON when valid).
///
/// Redirects are followed manually (see [`strip_cross_origin_headers`]); `followRedirects: false`
/// means 0 redirects, otherwise `maxRedirects` (default 10). The timeout (default 30 s) covers the
/// whole exchange including body download. `rejectUnauthorized: false` disables TLS certificate
/// verification for this request only. Dropping the returned future cancels the request.
/// DNS/TCP/TLS timings are always 0 (as in the TS).
pub async fn send_http_request(opts: SendOptions) -> Result<NexusResponse> {
    let timeout = Duration::from_millis(opts.settings.timeout.unwrap_or(30_000));
    match tokio::time::timeout(timeout, send_inner(opts)).await {
        Ok(r) => r,
        Err(_) => Err(Error::Other("Request timed out".to_string())),
    }
}

async fn send_inner(opts: SendOptions) -> Result<NexusResponse> {
    let start = Instant::now();
    let max_redirects = if opts.settings.follow_redirects == Some(false) {
        0
    } else {
        opts.settings.max_redirects.unwrap_or(10)
    };

    // Per-request client: redirects are manual, and TLS verification is a per-request setting.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(opts.settings.reject_unauthorized == Some(false))
        .build()
        .map_err(|e| Error::Other(e.to_string()))?;

    let mut url = Url::parse(&opts.url).map_err(|e| Error::Other(format!("Invalid URL: {e}")))?;
    let mut method = Method::from_bytes(opts.method.as_str().as_bytes())
        .map_err(|e| Error::Other(e.to_string()))?;
    let mut headers = opts.headers;
    let mut body: Option<Vec<u8>> = opts.body.map(SendBody::into_bytes);

    let ttfb_start = Instant::now();
    let mut response = dispatch(&client, &method, &url, &headers, &body).await?;

    for _ in 0..max_redirects {
        let status = response.status();
        let target = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let Some(target) = target else { break };
        if !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
            break;
        }

        // Drop the redirect body; ignore errors, the connection is not reused anyway.
        drop(response);
        let next = url
            .join(&target)
            .map_err(|e| Error::Other(format!("Invalid redirect location: {e}")))?;
        if next.origin() != url.origin() {
            headers = strip_cross_origin_headers(&headers);
        }
        // Same rules as fetch: 303 always becomes GET; 301/302 turn a POST into GET.
        let to_get = if status.as_u16() == 303 {
            method != Method::HEAD
        } else {
            status.as_u16() <= 302 && method == Method::POST
        };
        if to_get {
            method = Method::GET;
            body = None;
            headers.retain(|k, _| {
                let k = k.to_ascii_lowercase();
                k != "content-type" && k != "content-length"
            });
        }
        url = next;
        response = dispatch(&client, &method, &url, &headers, &body).await?;
    }

    let ttfb = ttfb_start.elapsed().as_secs_f64() * 1000.0;

    let status = response.status();
    let mut out_headers: IndexMap<String, HeaderValue> = IndexMap::new();
    for name in response.headers().keys() {
        let key = name.as_str().to_ascii_lowercase();
        let values: Vec<String> = response
            .headers()
            .get_all(name)
            .iter()
            .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
            .collect();
        // Like undici: `set-cookie` is always an array, other repeated headers are comma-joined.
        let value = if key == "set-cookie" {
            HeaderValue::Many(values)
        } else {
            HeaderValue::One(values.join(", "))
        };
        out_headers.insert(key, value);
    }

    let download_start = Instant::now();
    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| Error::Other(e.to_string()))?
        .to_vec();
    let download = download_start.elapsed().as_secs_f64() * 1000.0;
    let total = start.elapsed().as_secs_f64() * 1000.0;

    let body_text = String::from_utf8_lossy(&body_bytes).into_owned();
    let body_json = serde_json::from_str(&body_text).ok();

    Ok(NexusResponse {
        status: status.as_u16(),
        status_text: get_status_text(status),
        headers: out_headers,
        size: body_bytes.len() as u64,
        body: body_bytes,
        body_text,
        body_json,
        timing: ResponseTiming {
            dns: 0.0,
            tcp: 0.0,
            tls: 0.0,
            ttfb,
            download,
            total,
        },
    })
}

async fn dispatch(
    client: &reqwest::Client,
    method: &Method,
    url: &Url,
    headers: &IndexMap<String, String>,
    body: &Option<Vec<u8>>,
) -> Result<reqwest::Response> {
    let mut req = client
        .request(method.clone(), url.clone())
        .headers(build_header_map(headers)?);
    if let Some(b) = body {
        req = req.body(b.clone());
    }
    req.send()
        .await
        .map_err(|e| Error::Other(error_chain(&e.without_url())))
}

/// Short reason phrase for the status codes the TS knew about, `""` otherwise.
fn get_status_text(code: StatusCode) -> String {
    match code.as_u16() {
        200 | 201 | 204 | 301 | 302 | 304 | 400 | 401 | 403 | 404 | 405 | 409 | 422 | 429 | 500
        | 502 | 503 | 504 => code.canonical_reason().unwrap_or("").to_string(),
        _ => String::new(),
    }
}

/// `error sending request: connection refused: ...` instead of reqwest's bare top-level message.
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        source = s.source();
    }
    msg
}
