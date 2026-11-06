//! Port of `packages/core/src/graphql/graphql-client.ts`.
//!
//! HTTP queries/mutations (through the stage 5 sender), schema introspection printed as SDL
//! (what `graphql:introspect` returned via `printSchema`) and subscriptions over the
//! `graphql-transport-ws` protocol, hand-implemented on tungstenite.

mod schema_printer;
mod subscription;
#[cfg(test)]
mod tests;

pub use schema_printer::print_introspection_schema;
pub use subscription::{ws_url_for, SubscriptionEvent, SubscriptionMessage, SubscriptionOptions};

use crate::http::{send_http_request, SendBody, SendOptions};
use crate::types::{HeaderValue, HttpMethod, RequestSettings};
use crate::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::oneshot;

/// The query `graphql-js` `getIntrospectionQuery()` builds with default options.
pub const INTROSPECTION_QUERY: &str = r#"
    query IntrospectionQuery {
      __schema {
        queryType { name }
        mutationType { name }
        subscriptionType { name }
        types {
          ...FullType
        }
        directives {
          name
          description
          locations
          args {
            ...InputValue
          }
        }
      }
    }

    fragment FullType on __Type {
      kind
      name
      description
      fields(includeDeprecated: true) {
        name
        description
        args {
          ...InputValue
        }
        type {
          ...TypeRef
        }
        isDeprecated
        deprecationReason
      }
      inputFields {
        ...InputValue
      }
      interfaces {
        ...TypeRef
      }
      enumValues(includeDeprecated: true) {
        name
        description
        isDeprecated
        deprecationReason
      }
      possibleTypes {
        ...TypeRef
      }
    }

    fragment InputValue on __InputValue {
      name
      description
      type { ...TypeRef }
      defaultValue
    }

    fragment TypeRef on __Type {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
                ofType {
                  kind
                  name
                  ofType {
                    kind
                    name
                  }
                }
              }
            }
          }
        }
      }
    }
"#;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQLRequestOptions {
    pub url: String,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
}

/// `data`/`errors`/`extensions` are passed through untouched (a JSON `null` `data` stays `null`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQLResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
    pub status: u16,
    /// Milliseconds.
    pub response_time: u64,
    /// Lower-cased names; repeated headers are joined with `, `.
    pub headers: IndexMap<String, String>,
}

/// HTTP GraphQL client with a per-URL introspection cache and one active subscription.
#[derive(Default)]
pub struct GraphQLClient {
    /// URL -> printed SDL (the cache ignores headers, like the TS one).
    schemas: Mutex<HashMap<String, String>>,
    subscription: Mutex<Option<ActiveSubscription>>,
}

struct ActiveSubscription {
    stop: oneshot::Sender<()>,
}

impl GraphQLClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// POSTs the request as JSON and parses the reply (any HTTP status).
    pub async fn send(&self, opts: &GraphQLRequestOptions) -> Result<GraphQLResponse> {
        send_request(opts).await
    }

    /// Introspects `url` and returns the schema as SDL; cached per URL.
    pub async fn introspect(
        &self,
        url: &str,
        headers: Option<IndexMap<String, String>>,
    ) -> Result<String> {
        if let Some(sdl) = self
            .schemas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(url)
        {
            return Ok(sdl.clone());
        }
        let response = send_request(&GraphQLRequestOptions {
            url: url.to_string(),
            query: INTROSPECTION_QUERY.to_string(),
            headers,
            ..Default::default()
        })
        .await?;

        if let Some(errors) = response
            .errors
            .as_ref()
            .and_then(Value::as_array)
            .filter(|e| !e.is_empty())
        {
            let msgs: Vec<&str> = errors
                .iter()
                .map(|e| {
                    e.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("undefined")
                })
                .collect();
            return Err(Error::Other(format!(
                "Introspection failed: {}",
                msgs.join(", ")
            )));
        }
        let data = match response.data {
            Some(d) if !d.is_null() => d,
            _ => return Err(Error::Other("Introspection returned no data".into())),
        };
        let sdl = print_introspection_schema(&data)?;
        self.schemas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(url.to_string(), sdl.clone());
        Ok(sdl)
    }

    /// Drops the cached schema of `url`, or of every URL when `None`.
    pub fn clear_schema_cache(&self, url: Option<&str>) {
        let mut schemas = self.schemas.lock().unwrap_or_else(|e| e.into_inner());
        match url {
            Some(u) => {
                schemas.remove(u);
            }
            None => schemas.clear(),
        }
    }

    /// Starts a subscription (must run inside a tokio runtime), replacing any previous one.
    /// Events go to `sink`; connection problems are reported as `SubscriptionEvent::Error`.
    pub fn subscribe(
        &self,
        opts: SubscriptionOptions,
        sink: impl Fn(SubscriptionEvent) + Send + Sync + 'static,
    ) {
        self.unsubscribe();
        let (stop_tx, stop_rx) = oneshot::channel();
        tokio::spawn(subscription::run(opts, sink, stop_rx));
        *self.subscription.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(ActiveSubscription { stop: stop_tx });
    }

    /// Stops the active subscription: the task tells the server (`complete`) and closes the socket.
    pub fn unsubscribe(&self) {
        let active = self
            .subscription
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(a) = active {
            let _ = a.stop.send(());
        }
    }

    /// Stops the subscription and clears the schema cache.
    pub fn dispose(&self) {
        self.unsubscribe();
        self.clear_schema_cache(None);
    }
}

impl Drop for GraphQLClient {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

async fn send_request(opts: &GraphQLRequestOptions) -> Result<GraphQLResponse> {
    let start = Instant::now();
    let mut body = serde_json::Map::new();
    body.insert("query".into(), Value::String(opts.query.clone()));
    if let Some(v) = &opts.variables {
        body.insert("variables".into(), v.clone());
    }
    if let Some(n) = &opts.operation_name {
        body.insert("operationName".into(), Value::String(n.clone()));
    }

    let mut headers: IndexMap<String, String> = IndexMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    headers.insert("Accept".into(), "application/json".into());
    for (k, v) in opts.headers.iter().flatten() {
        // Caller headers win, case-insensitively.
        headers.retain(|existing, _| !existing.eq_ignore_ascii_case(k));
        headers.insert(k.clone(), v.clone());
    }

    let response = send_http_request(SendOptions {
        method: HttpMethod::Post,
        url: opts.url.clone(),
        headers,
        body: Some(SendBody::Text(Value::Object(body).to_string())),
        // undici's `request` does not follow redirects.
        settings: RequestSettings {
            follow_redirects: Some(false),
            ..Default::default()
        },
    })
    .await?;
    let response_time = start.elapsed().as_millis() as u64;

    let headers = response
        .headers
        .iter()
        .map(|(k, v)| {
            let v = match v {
                HeaderValue::One(s) => s.clone(),
                HeaderValue::Many(items) => items.join(", "),
            };
            (k.clone(), v)
        })
        .collect();

    let (data, errors, extensions) = match serde_json::from_str::<Value>(&response.body_text) {
        Ok(Value::Object(mut o)) => (o.remove("data"), o.remove("errors"), o.remove("extensions")),
        _ => {
            let snippet: String = response.body_text.chars().take(200).collect();
            (
                None,
                Some(json!([{ "message": format!("Invalid JSON response: {snippet}") }])),
                None,
            )
        }
    };
    Ok(GraphQLResponse {
        data,
        errors,
        extensions,
        status: response.status,
        response_time,
        headers,
    })
}
