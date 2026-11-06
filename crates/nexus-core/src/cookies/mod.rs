//! Port of `packages/core/src/cookies`.

mod cookie_jar;
#[cfg(test)]
mod tests;

pub use cookie_jar::{CookieInfo, NexusCookieJar};
