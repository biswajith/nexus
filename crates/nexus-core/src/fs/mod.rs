//! Owner-only file helpers (port of `packages/core/src/fs/private-fs.ts`).
//!
//! `~/.nexus` holds tokens, secrets and history, so every file is 0600 and every directory 0700.
//! The mode passed at creation only applies to new files, so the mode is also set explicitly to
//! tighten files left behind by older versions.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Mode for files that may hold credentials: owner read/write only.
pub const PRIVATE_FILE_MODE: u32 = 0o600;

/// Mode for directories that hold such files: owner only.
pub const PRIVATE_DIR_MODE: u32 = 0o700;

/// How [`write_private_file`] opens its target (the `'w' | 'wx' | 'a'` open flags of the TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Create or truncate.
    Replace,
    /// Create; fail with `AlreadyExists` if the file exists.
    CreateNew,
    /// Create or append.
    Append,
}

/// Creates `dir` (and parents) owner-only, and tightens the leaf if it already existed.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
}

/// Creates one new directory (not recursive) with mode 0700. Fails with `AlreadyExists` if present.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new().mode(PRIVATE_DIR_MODE).create(dir)
}

/// Writes `data` to `file` with mode 0600.
pub fn write_private_file(file: &Path, data: &str, mode: WriteMode) -> io::Result<()> {
    write_private_bytes(file, data.as_bytes(), mode)
}

/// Like [`write_private_file`] for binary data.
pub fn write_private_bytes(file: &Path, data: &[u8], mode: WriteMode) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).mode(PRIVATE_FILE_MODE);
    match mode {
        WriteMode::Replace => opts.create(true).truncate(true),
        WriteMode::CreateNew => opts.create_new(true),
        WriteMode::Append => opts.create(true).append(true),
    };
    let mut handle = opts.open(file)?;
    handle.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    handle.write_all(data)
}

/// Appends `data` to `file`, creating it with mode 0600.
pub fn append_private_file(file: &Path, data: &str) -> io::Result<()> {
    write_private_file(file, data, WriteMode::Append)
}

/// Whether `id` can be used as a file name as-is: 1-128 of letters, digits, `_` and `-`.
/// IDs of imported records come from the imported file, so anything else could contain
/// path separators or `..`.
pub fn is_safe_file_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests;
