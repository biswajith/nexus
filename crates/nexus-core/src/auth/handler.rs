use crate::http::SendBody;
use crate::types::{AuthConfig, AuthType};
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use indexmap::IndexMap;
use md5::Md5;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRequest {
    pub method: String,
    pub url: String,
    pub headers: IndexMap<String, String>,
    pub body: Option<SendBody>,
}

/// What an auth handler contributes: headers to merge and query params to set. Empty = no-op.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AuthResult {
    pub headers: IndexMap<String, String>,
    pub params: IndexMap<String, String>,
}

impl AuthResult {
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty() && self.params.is_empty()
    }

    fn header(name: &str, value: String) -> Self {
        Self {
            headers: IndexMap::from([(name.to_string(), value)]),
            params: IndexMap::new(),
        }
    }

    fn param(name: &str, value: String) -> Self {
        Self {
            headers: IndexMap::new(),
            params: IndexMap::from([(name.to_string(), value)]),
        }
    }
}

/// `String(config[key] ?? '')`: missing/null is `None`, other scalars are stringified like JS.
fn cfg(config: &AuthConfig, key: &str) -> Option<String> {
    match config.extra.get(key)? {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn cfg_or(config: &AuthConfig, key: &str, default: &str) -> String {
    cfg(config, key).unwrap_or_else(|| default.to_string())
}

/// Computes the headers / query params for `config` against `request`. Unknown, `none`,
/// `inherit` and `hawk` configs, and configs missing required fields, give an empty result.
pub fn apply_auth(config: &AuthConfig, request: &PreparedRequest) -> Result<AuthResult> {
    apply_auth_at(config, request, Utc::now(), &random_cnonce())
}

/// [`apply_auth`] with the clock and the digest client nonce injected (for tests).
pub(crate) fn apply_auth_at(
    config: &AuthConfig,
    request: &PreparedRequest,
    now: DateTime<Utc>,
    cnonce: &str,
) -> Result<AuthResult> {
    match config.auth_type {
        AuthType::None | AuthType::Inherit | AuthType::Hawk => Ok(AuthResult::default()),
        AuthType::ApiKey => {
            let key = cfg_or(config, "key", "");
            let value = cfg_or(config, "value", "");
            if key.is_empty() || value.is_empty() {
                return Ok(AuthResult::default());
            }
            Ok(if cfg_or(config, "addTo", "header") == "query" {
                AuthResult::param(&key, value)
            } else {
                AuthResult::header(&key, value)
            })
        }
        AuthType::Bearer => {
            let token = cfg_or(config, "token", "");
            if token.is_empty() {
                return Ok(AuthResult::default());
            }
            let prefix = cfg_or(config, "prefix", "Bearer");
            Ok(AuthResult::header(
                "Authorization",
                format!("{prefix} {token}"),
            ))
        }
        AuthType::Basic => {
            use base64::Engine;
            let creds = format!(
                "{}:{}",
                cfg_or(config, "username", ""),
                cfg_or(config, "password", "")
            );
            let encoded = base64::engine::general_purpose::STANDARD.encode(creds);
            Ok(AuthResult::header(
                "Authorization",
                format!("Basic {encoded}"),
            ))
        }
        AuthType::Oauth2 => {
            let access_token = cfg_or(config, "accessToken", "");
            if access_token.is_empty() {
                return Ok(AuthResult::default());
            }
            Ok(if cfg_or(config, "addTo", "header") == "query" {
                AuthResult::param("access_token", access_token)
            } else {
                let token_type = cfg_or(config, "tokenType", "Bearer");
                AuthResult::header("Authorization", format!("{token_type} {access_token}"))
            })
        }
        AuthType::Digest => apply_digest(config, request, cnonce),
        AuthType::AwsSigV4 => apply_sig_v4(config, request, now),
    }
}

fn parse_url(url: &str) -> Result<Url> {
    Url::parse(url).map_err(|e| Error::Other(format!("Invalid URL: {e}")))
}

fn random_cnonce() -> String {
    hex::encode(&uuid::Uuid::new_v4().as_bytes()[..8])
}

fn md5_hex(data: &str) -> String {
    hex::encode(Md5::digest(data.as_bytes()))
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn apply_digest(
    config: &AuthConfig,
    request: &PreparedRequest,
    cnonce: &str,
) -> Result<AuthResult> {
    let username = cfg_or(config, "username", "");
    let password = cfg_or(config, "password", "");
    let realm = cfg_or(config, "realm", "");
    let nonce = cfg_or(config, "nonce", "");
    let qop = cfg_or(config, "qop", "auth");
    let algorithm = cfg_or(config, "algorithm", "MD5");
    let opaque = cfg_or(config, "opaque", "");

    if username.is_empty() || nonce.is_empty() {
        return Ok(AuthResult::default());
    }

    let url = parse_url(&request.url)?;
    let uri = format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    let nc = "00000001";

    let sha256 = algorithm.to_uppercase().contains("SHA-256");
    let hash = |data: &str| {
        if sha256 {
            sha256_hex(data.as_bytes())
        } else {
            md5_hex(data)
        }
    };

    let ha1 = hash(&format!("{username}:{realm}:{password}"));
    let ha2 = hash(&format!("{}:{uri}", request.method));

    let response_hash = if qop == "auth" || qop == "auth-int" {
        hash(&format!("{ha1}:{nonce}:{nc}:{cnonce}:{qop}:{ha2}"))
    } else {
        hash(&format!("{ha1}:{nonce}:{ha2}"))
    };

    let mut header = format!(
        "Digest username=\"{username}\", realm=\"{realm}\", nonce=\"{nonce}\", uri=\"{uri}\", response=\"{response_hash}\", algorithm={algorithm}"
    );
    if !qop.is_empty() {
        header.push_str(&format!(", qop={qop}, nc={nc}, cnonce=\"{cnonce}\""));
    }
    if !opaque.is_empty() {
        header.push_str(&format!(", opaque=\"{opaque}\""));
    }
    Ok(AuthResult::header("Authorization", header))
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// AWS Signature Version 4 exactly as the TS computed it: only `host`, `x-amz-date` and
/// `x-amz-security-token` are signed, and the canonical query string is the URL's query
/// re-serialized as form-urlencoded in its original order (not sorted per the AWS spec).
fn apply_sig_v4(
    config: &AuthConfig,
    request: &PreparedRequest,
    now: DateTime<Utc>,
) -> Result<AuthResult> {
    let access_key = cfg_or(config, "accessKey", "");
    let secret_key = cfg_or(config, "secretKey", "");
    let region = cfg_or(config, "region", "us-east-1");
    let service = cfg_or(config, "service", "execute-api");
    let session_token = cfg(config, "sessionToken").filter(|s| !s.is_empty());

    if access_key.is_empty() || secret_key.is_empty() {
        return Ok(AuthResult::default());
    }

    let url = parse_url(&request.url)?;
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = &amz_date[..8];

    let host = match (url.host_str(), url.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        _ => return Err(Error::Other("Invalid URL: no host".to_string())),
    };
    let mut headers: IndexMap<String, String> = IndexMap::new();
    headers.insert("host".into(), host);
    headers.insert("x-amz-date".into(), amz_date.clone());
    if let Some(t) = session_token {
        headers.insert("x-amz-security-token".into(), t);
    }

    let mut keys: Vec<&String> = headers.keys().collect();
    keys.sort();
    let signed_headers = keys
        .iter()
        .map(|k| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical_headers: String = keys
        .iter()
        .map(|k| format!("{k}:{}\n", headers[*k].trim()))
        .collect();

    let body_hash = match &request.body {
        None => sha256_hex(b""),
        Some(SendBody::Text(s)) => sha256_hex(s.as_bytes()),
        Some(SendBody::Bytes(b)) => sha256_hex(String::from_utf8_lossy(b).as_bytes()),
    };

    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(url.query_pairs())
        .finish();
    let path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    let canonical_request = [
        request.method.to_uppercase().as_str(),
        path,
        &query,
        &canonical_headers,
        &signed_headers,
        &body_hash,
    ]
    .join("\n");

    let credential_scope = format!("{date_stamp}/{region}/{service}/aws4_request");
    let string_to_sign = [
        "AWS4-HMAC-SHA256",
        &amz_date,
        &credential_scope,
        &sha256_hex(canonical_request.as_bytes()),
    ]
    .join("\n");

    let k_date = hmac(format!("AWS4{secret_key}").as_bytes(), date_stamp);
    let k_region = hmac(&k_date, &region);
    let k_service = hmac(&k_region, &service);
    let k_signing = hmac(&k_service, "aws4_request");
    let signature = hex::encode(hmac(&k_signing, &string_to_sign));

    headers.insert(
        "Authorization".into(),
        format!(
            "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
        ),
    );
    Ok(AuthResult {
        headers,
        params: IndexMap::new(),
    })
}

/// Applies the handler for `config.auth_type` and returns a new request with the auth headers
/// merged in and the auth query params `set` on the URL (replacing existing params of that name).
/// `none`/`inherit` return the request unchanged.
pub fn resolve_auth(config: &AuthConfig, request: &PreparedRequest) -> Result<PreparedRequest> {
    if matches!(config.auth_type, AuthType::None | AuthType::Inherit) {
        return Ok(request.clone());
    }
    let result = apply_auth(config, request)?;

    let mut headers = request.headers.clone();
    for (k, v) in result.headers {
        headers.insert(k, v);
    }

    let mut updated_url = request.url.clone();
    if !result.params.is_empty() {
        let mut url = parse_url(&request.url)?;
        let mut pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        for (k, v) in result.params {
            // URLSearchParams.set: first match gets the value, later duplicates are removed.
            match pairs.iter().position(|(pk, _)| *pk == k) {
                Some(i) => {
                    pairs[i].1 = v;
                    let mut seen = 0;
                    pairs.retain(|(pk, _)| {
                        if *pk == k {
                            seen += 1;
                            seen == 1
                        } else {
                            true
                        }
                    });
                }
                None => pairs.push((k, v)),
            }
        }
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&pairs)
            .finish();
        url.set_query(Some(&query));
        updated_url = url.to_string();
    }

    Ok(PreparedRequest {
        url: updated_url,
        headers,
        ..request.clone()
    })
}

/// Resolves `inherit` auth: the request's own config unless it is `inherit`, in which case the
/// nearest ancestor with a non-`inherit` config wins. `ancestors` is ordered innermost folder
/// first, collection last; `None` (no auth set) counts as inherit. Falls back to `none`.
/// (The TS runner only looked at the collection; folder auth is honoured here too.)
pub fn effective_auth(request_auth: &AuthConfig, ancestors: &[Option<&AuthConfig>]) -> AuthConfig {
    if request_auth.auth_type != AuthType::Inherit {
        return request_auth.clone();
    }
    ancestors
        .iter()
        .flatten()
        .find(|a| a.auth_type != AuthType::Inherit)
        .map(|a| (*a).clone())
        .unwrap_or_else(|| AuthConfig::new(AuthType::None))
}
