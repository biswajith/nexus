//! On-disk environments: `~/.nexus/environments/<id>.json` (mode 0600). In the TS this lived in
//! `ipc-handlers.ts` (`saveEnvironment`, `saveImportedEnvironment`, `environments:*`).

use crate::fs::{ensure_private_dir, is_safe_file_id, write_private_file, WriteMode};
use crate::types::NexusEnvironment;
use crate::{Error, Result};
use std::io;
use std::path::PathBuf;

pub struct EnvironmentStore {
    dir: PathBuf,
}

impl EnvironmentStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// All environments; files that cannot be parsed are skipped. Sorted by file name.
    pub fn list(&self) -> Result<Vec<NexusEnvironment>> {
        ensure_private_dir(&self.dir)?;
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        Ok(files
            .iter()
            .filter_map(|f| serde_json::from_str(&std::fs::read_to_string(f).ok()?).ok())
            .collect())
    }

    fn save(&self, env: &NexusEnvironment, mode: WriteMode) -> Result<()> {
        if !is_safe_file_id(&env.id) {
            return Err(Error::Other("Invalid environment id".into()));
        }
        ensure_private_dir(&self.dir)?;
        write_private_file(
            &self.dir.join(format!("{}.json", env.id)),
            &serde_json::to_string_pretty(env)?,
            mode,
        )?;
        Ok(())
    }

    /// `environments:create` and `environments:update`: create or replace `<id>.json`.
    pub fn save_environment(&self, env: &NexusEnvironment) -> Result<()> {
        self.save(env, WriteMode::Replace)
    }

    /// Saves an imported environment without touching existing ones. The id comes from the
    /// imported file, so it is replaced with a fresh `env_<uuid>` when it is not a safe file name
    /// or is already taken. Returns the saved environment (possibly with a new id).
    pub fn save_imported(&self, env: &NexusEnvironment) -> Result<NexusEnvironment> {
        let mut candidate = if is_safe_file_id(&env.id) {
            env.clone()
        } else {
            NexusEnvironment {
                id: fresh_id(),
                ..env.clone()
            }
        };
        loop {
            match self.save(&candidate, WriteMode::CreateNew) {
                Ok(()) => return Ok(candidate),
                Err(Error::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => {
                    candidate = NexusEnvironment {
                        id: fresh_id(),
                        ..env.clone()
                    };
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Deletes `<id>.json`; a missing file is not an error.
    pub fn delete(&self, env_id: &str) -> Result<()> {
        if !is_safe_file_id(env_id) {
            return Err(Error::Other("Invalid environment id".into()));
        }
        match std::fs::remove_file(self.dir.join(format!("{env_id}.json"))) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

fn fresh_id() -> String {
    format!("env_{}", uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests;
