//! Auto-detect candidate ACP-capable agent binaries on `$PATH`.
//!
//! Pure logic, no spawning: a `which`-style scan over `$PATH` for a short list
//! of known agent CLIs. Kept dependency-free (a hand-rolled PATH split +
//! executable-bit check) matching the convention `ui/import.rs` already uses
//! for its own hand-rolled parsing rather than pulling in a crate for this.

use std::path::PathBuf;

/// Known ACP-capable / agent CLI names, in detection preference order.
pub fn candidate_names() -> &'static [&'static str] {
    &["claude-code-acp", "claude", "codex", "gemini"]
}

/// One agent binary found on `$PATH`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedAgent {
    pub name: String,
    pub path: PathBuf,
}

/// Scan every `$PATH` directory for each candidate name, first match wins per
/// name, in `candidate_names()` order. Empty if `$PATH` is unset or nothing
/// matches - an honest empty state, not a fabricated default.
pub fn scan_path() -> Vec<DetectedAgent> {
    let Some(path_var) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    candidate_names()
        .iter()
        .filter_map(|name| {
            dirs.iter().find_map(|dir| {
                let candidate = dir.join(name);
                is_executable_file(&candidate).then(|| DetectedAgent {
                    name: name.to_string(),
                    path: candidate,
                })
            })
        })
        .collect()
}

#[cfg(unix)]
fn is_executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &std::path::Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    // `$PATH` is process-global and cargo runs tests concurrently; serialize
    // the two tests that mutate it so they don't race each other.
    static PATH_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(b"#!/bin/sh\n").unwrap();
        let mut perms = f.metadata().unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn scan_path_finds_an_executable_candidate_in_a_scratch_path() {
        let _guard = PATH_ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("wb-agent-detect-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        make_executable(&dir.join("claude"));
        // A same-named file with no executable bit must not count.
        std::fs::write(dir.join("codex"), b"not executable").unwrap();

        let saved = std::env::var_os("PATH");
        // SAFETY: serialized by PATH_ENV_LOCK above; no other thread reads/
        // writes PATH concurrently with this test.
        unsafe { std::env::set_var("PATH", &dir) };
        let found = scan_path();
        if let Some(p) = saved {
            unsafe { std::env::set_var("PATH", p) };
        }
        std::fs::remove_dir_all(&dir).ok();

        assert!(found.iter().any(|a| a.name == "claude"));
        assert!(
            !found.iter().any(|a| a.name == "codex"),
            "non-executable file must not be detected"
        );
    }

    #[test]
    fn scan_path_is_empty_without_a_path_env() {
        let _guard = PATH_ENV_LOCK.lock().unwrap();
        let saved = std::env::var_os("PATH");
        // SAFETY: serialized by PATH_ENV_LOCK above.
        unsafe { std::env::remove_var("PATH") };
        let found = scan_path();
        if let Some(p) = saved {
            unsafe { std::env::set_var("PATH", p) };
        }
        assert!(found.is_empty());
    }

    #[test]
    fn candidate_names_lists_known_agent_clis() {
        assert!(candidate_names().contains(&"claude-code-acp"));
        assert!(candidate_names().contains(&"claude"));
    }
}
