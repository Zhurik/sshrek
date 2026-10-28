//! Where the config and the history live.
//!
//! Each location comes in two halves: a `resolve_*` function holding the rules
//! and a wrapper that feeds it the environment. The rules are then testable
//! without touching the environment of the running process.

use std::path::{Path, PathBuf};

pub const LOG_FILE: &str = "sessions.jsonl";

fn non_empty(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

pub fn resolve_ssh_config(env_val: Option<&str>, home: &Path) -> PathBuf {
    match non_empty(env_val) {
        Some(v) => PathBuf::from(v),
        None => home.join(".ssh").join("config"),
    }
}

pub fn resolve_state_dir(sshrek_dir: Option<&str>, xdg: Option<&str>, home: &Path) -> PathBuf {
    if let Some(v) = non_empty(sshrek_dir) {
        return PathBuf::from(v);
    }
    if let Some(v) = non_empty(xdg) {
        return PathBuf::from(v).join("sshrek");
    }
    home.join(".local").join("state").join("sshrek")
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn ssh_config() -> PathBuf {
    let env_val = std::env::var("SSHREK_SSH_CONFIG").ok();
    resolve_ssh_config(env_val.as_deref(), &home())
}

pub fn state_dir() -> PathBuf {
    let own = std::env::var("SSHREK_STATE_DIR").ok();
    let xdg = std::env::var("XDG_STATE_HOME").ok();
    resolve_state_dir(own.as_deref(), xdg.as_deref(), &home())
}

pub fn sessions_log() -> PathBuf {
    state_dir().join(LOG_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_config_defaults_under_home() {
        let p = resolve_ssh_config(None, Path::new("/home/z"));
        assert_eq!(p, PathBuf::from("/home/z/.ssh/config"));
    }

    #[test]
    fn ssh_config_env_wins() {
        let p = resolve_ssh_config(Some("/tmp/cfg"), Path::new("/home/z"));
        assert_eq!(p, PathBuf::from("/tmp/cfg"));
    }

    #[test]
    fn empty_env_is_ignored() {
        let p = resolve_ssh_config(Some("   "), Path::new("/home/z"));
        assert_eq!(p, PathBuf::from("/home/z/.ssh/config"));
    }

    #[test]
    fn state_dir_prefers_own_env_then_xdg_then_home() {
        assert_eq!(
            resolve_state_dir(Some("/tmp/s"), Some("/xdg"), Path::new("/home/z")),
            PathBuf::from("/tmp/s")
        );
        assert_eq!(
            resolve_state_dir(None, Some("/xdg"), Path::new("/home/z")),
            PathBuf::from("/xdg/sshrek")
        );
        assert_eq!(
            resolve_state_dir(None, None, Path::new("/home/z")),
            PathBuf::from("/home/z/.local/state/sshrek")
        );
    }
}
