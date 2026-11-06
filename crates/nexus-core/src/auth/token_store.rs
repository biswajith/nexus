use crate::fs::{write_private_file, WriteMode};
use crate::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An OAuth2 token as persisted in `~/.nexus/tokens.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredToken {
    pub access_token: String,
    pub token_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    /// Epoch milliseconds.
    pub expires_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// In-memory map of tokens by key, with optional JSON file persistence (mode 0600).
#[derive(Debug, Default)]
pub struct TokenStore {
    tokens: IndexMap<String, StoredToken>,
    file_path: Option<PathBuf>,
}

impl TokenStore {
    /// Pass `file_path` to load and persist tokens as JSON on disk.
    pub fn new(file_path: Option<PathBuf>) -> Self {
        Self {
            tokens: IndexMap::new(),
            file_path,
        }
    }

    /// Reads tokens from the backing file; a missing or invalid file gives an empty store.
    pub fn load(&mut self) {
        let Some(path) = &self.file_path else { return };
        self.tokens = std::fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
    }

    pub fn get_token(&self, key: &str) -> Option<StoredToken> {
        self.tokens.get(key).cloned()
    }

    /// Expired, or within 30 seconds of expiring.
    pub fn is_expired(&self, token: &StoredToken) -> bool {
        chrono::Utc::now().timestamp_millis() >= token.expires_at - 30_000
    }

    pub fn save_token(&mut self, key: &str, token: StoredToken) -> Result<()> {
        self.tokens.insert(key.to_string(), token);
        self.persist()
    }

    pub fn remove_token(&mut self, key: &str) -> Result<()> {
        self.tokens.shift_remove(key);
        self.persist()
    }

    pub fn clear(&mut self) -> Result<()> {
        self.tokens.clear();
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        let Some(path) = &self.file_path else {
            return Ok(());
        };
        write_private_file(
            path,
            &serde_json::to_string_pretty(&self.tokens)?,
            WriteMode::Replace,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
