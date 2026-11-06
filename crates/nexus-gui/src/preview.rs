//! HTML preview, visualizer output and "open externally": the content goes to a private temp
//! file under `<home>/tmp` and is opened by the system (the browser for `.html`). No webview.

use nexus_core::fs::{ensure_private_dir, write_private_bytes, WriteMode};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Opens a file with the system handler. Injectable so tests do not launch a browser.
pub type Opener = Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

/// The real opener (`xdg-open` & co., detached).
pub fn system_opener() -> Opener {
    Arc::new(|p| open::that_detached(p).map_err(|e| e.to_string()))
}

const TTL: Duration = Duration::from_secs(24 * 3600);

fn tmp_dir(home: &Path) -> PathBuf {
    home.join("tmp")
}

/// Writes `bytes` to a fresh owner-only file `<prefix>-<uuid>.<ext>` and returns its path.
/// Files older than a day are removed on the way.
pub fn write_temp(home: &Path, prefix: &str, ext: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let dir = tmp_dir(home);
    ensure_private_dir(&dir)?;
    sweep(&dir);
    let ext: String = ext
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect();
    let name = format!("{prefix}-{}.{ext}", uuid::Uuid::new_v4().simple());
    let path = dir.join(name);
    write_private_bytes(&path, bytes, WriteMode::CreateNew)?;
    Ok(path)
}

fn sweep(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > TTL);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Response HTML for the browser: scripts, forms and network calls are blocked (an embedded
/// preview was a fully sandboxed iframe, and a `file://` page must not run response code).
pub fn preview_html(html: &str) -> String {
    format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'none'; connect-src 'none'; form-action 'none'; object-src 'none'; frame-src 'none'\">{html}"
    )
}

/// Visualizer output: its own scripts may run, but cannot talk to the network or submit forms.
pub fn visualizer_html(html: &str) -> String {
    format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"connect-src 'none'; form-action 'none'; object-src 'none'; frame-src 'none'\">{html}"
    )
}

/// File extension for a binary response's content type.
pub fn ext_for(content_type: &str) -> &'static str {
    let ct = content_type.to_ascii_lowercase();
    let ct = ct.split(';').next().unwrap_or("").trim();
    match ct {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "application/json" => "json",
        "text/html" => "html",
        "text/plain" => "txt",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn temp_files_are_private_and_unique() {
        let d = tempfile::tempdir().unwrap();
        let a = write_temp(d.path(), "preview", "html", b"<p>x</p>").unwrap();
        let b = write_temp(d.path(), "preview", "html", b"<p>x</p>").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read(&a).unwrap(), b"<p>x</p>");
        let mode = std::fs::metadata(&a).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dmode = std::fs::metadata(d.path().join("tmp"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dmode, 0o700);
    }

    #[test]
    fn extension_is_sanitised() {
        let d = tempfile::tempdir().unwrap();
        let p = write_temp(d.path(), "x", "../../etc", b"").unwrap();
        assert!(p.starts_with(d.path().join("tmp")));
    }

    #[test]
    fn preview_blocks_scripts() {
        assert!(preview_html("<b>x</b>").contains("script-src 'none'"));
        assert!(!visualizer_html("<b>x</b>").contains("script-src"));
    }
}
