use std::path::PathBuf;

/// Canonical socket path for the local orbtd instance.
///
/// Priority: /run/user/<uid>/orbt.sock → $XDG_RUNTIME_DIR/orbt.sock
///           → $TMPDIR/orbit-<uid>.sock
pub fn default_socket_path() -> PathBuf {
    // libc::getuid() is unix-only; on other platforms fall back to env vars.
    #[cfg(unix)]
    let uid = unsafe { libc::getuid() };

    #[cfg(unix)]
    {
        let runtime = format!("/run/user/{uid}");
        let runtime_dir = std::path::Path::new(&runtime);
        if runtime_dir.exists() {
            return runtime_dir.join("orbt.sock");
        }
    }

    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = std::path::Path::new(&dir);
        if p.exists() {
            return p.join("orbt.sock");
        }
    }

    #[cfg(unix)]
    return std::env::temp_dir().join(format!("orbt-{uid}.sock"));

    #[cfg(not(unix))]
    return std::env::temp_dir().join("orbt.sock");
}
