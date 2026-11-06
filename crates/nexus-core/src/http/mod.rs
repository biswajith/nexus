//! Port of `packages/core/src/http`.

mod request_sender;
#[cfg(test)]
mod tests;

pub use request_sender::{send_http_request, strip_cross_origin_headers, SendBody, SendOptions};

use crate::auth::{resolve_auth, PreparedRequest};
use crate::types::{AuthConfig, AuthType, HttpMethod, NexusResponse, RequestSettings};
use crate::{Error, Result};
use indexmap::IndexMap;

/// Parses a (script-mutated) method name, case-insensitively.
pub fn parse_method(m: &str) -> Result<HttpMethod> {
    serde_json::from_value(serde_json::Value::String(m.to_ascii_uppercase()))
        .map_err(|_| Error::Other(format!("Invalid HTTP method: {m}")))
}

/// A response plus the request that was actually sent (after auth was applied).
#[derive(Debug, Clone)]
pub struct SentRequest {
    pub response: NexusResponse,
    pub url: String,
    pub headers: IndexMap<String, String>,
}

/// The shared "apply auth -> send" step of `http_send` and the collection runner. `none` /
/// `inherit` add nothing (inheritance is resolved by the caller).
pub async fn send_authenticated(
    auth: Option<&AuthConfig>,
    method: HttpMethod,
    url: String,
    headers: IndexMap<String, String>,
    body: Option<String>,
    settings: RequestSettings,
) -> Result<SentRequest> {
    send_authenticated_body(
        auth,
        method,
        url,
        headers,
        body.map(SendBody::Text),
        settings,
    )
    .await
}

/// [`send_authenticated`] with a text or binary body (multipart forms, file uploads).
pub async fn send_authenticated_body(
    auth: Option<&AuthConfig>,
    method: HttpMethod,
    mut url: String,
    mut headers: IndexMap<String, String>,
    body: Option<SendBody>,
    settings: RequestSettings,
) -> Result<SentRequest> {
    if let Some(auth) = auth.filter(|a| !matches!(a.auth_type, AuthType::None | AuthType::Inherit))
    {
        let prepared = resolve_auth(
            auth,
            &PreparedRequest {
                method: method.as_str().to_string(),
                url: url.clone(),
                headers: headers.clone(),
                body: body.clone(),
            },
        )?;
        url = prepared.url;
        headers = prepared.headers;
    }
    let response = send_http_request(SendOptions {
        method,
        url: url.clone(),
        headers: headers.clone(),
        body,
        settings,
    })
    .await?;
    Ok(SentRequest {
        response,
        url,
        headers,
    })
}
