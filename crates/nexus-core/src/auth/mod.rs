//! Port of `packages/core/src/auth`: the request auth handlers and the OAuth2 token store.
//!
//! The TS auth module has no OAuth2 grant flows (no authorization-code/PKCE/client-credentials
//! exchange) and no Hawk handler (`hawk` is a type-only auth kind that behaves like `none`);
//! `oauth2` just sends a static `accessToken`. This port matches that.

mod handler;
#[cfg(test)]
mod handler_tests;
mod token_store;

pub use handler::{apply_auth, effective_auth, resolve_auth, AuthResult, PreparedRequest};
pub use token_store::{StoredToken, TokenStore};
