use std::path::PathBuf;

/// Canonical socket path for the local orbitd instance.
///
/// Priority: /run/user/<uid>/orbit.sock → $XDG_RUNTIME_DIR/orbit.sock
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
            return runtime_dir.join("orbit.sock");
        }
    }

    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = std::path::Path::new(&dir);
        if p.exists() {
            return p.join("orbit.sock");
        }
    }

    #[cfg(unix)]
    return std::env::temp_dir().join(format!("orbit-{uid}.sock"));

    #[cfg(not(unix))]
    return std::env::temp_dir().join("orbit.sock");
}
