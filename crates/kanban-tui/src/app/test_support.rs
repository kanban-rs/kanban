//! Test-only helpers shared by `dialog_tests` and `lifecycle_tests`.
//!
//! `App::new_with_store` calls `kanban_service::config::load()`, which reads
//! the real `$HOME/.config/kanban/config.toml` unless `KANBAN_CONFIG`
//! overrides it. On any machine that has actually run `kanban` for real
//! (i.e. every developer's machine, just not a fresh CI runner), that file
//! can carry a genuine `storage_location`, which makes
//! `App::new_with_store(sm, None)` observe `has_data_file = true` instead of
//! the `false` these tests assume — not flaky, but silently
//! environment-dependent: it passes in CI and fails locally for anyone with
//! a real kanban config.

use std::sync::{Mutex, MutexGuard};

/// `KANBAN_CONFIG` is process-global; every test in this binary that relies
/// on `App::new_with_store` reading a clean config must serialize on this
/// lock, matching the existing `CWD_LOCK` pattern in
/// `tests/backend_selection_tests.rs` for the same class of problem.
static CONFIG_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Held for the duration of a test that needs `kanban_service::config::load()`
/// to see an empty config regardless of what's on disk at the real location.
/// Pins `KANBAN_CONFIG` to a nonexistent path inside a private `TempDir` on
/// construction, and clears it again on drop.
pub(in crate::app) struct IsolatedConfigGuard {
    _lock: MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
}

impl Drop for IsolatedConfigGuard {
    fn drop(&mut self) {
        // SAFETY: guarded by CONFIG_ENV_LOCK for as long as `self` is alive;
        // no other test in this binary can be reading or writing
        // KANBAN_CONFIG concurrently.
        unsafe {
            std::env::remove_var("KANBAN_CONFIG");
        }
    }
}

/// Acquire the process-wide config-isolation lock and pin `KANBAN_CONFIG` to
/// an empty, nonexistent path until the returned guard is dropped.
pub(in crate::app) fn isolated_config() -> IsolatedConfigGuard {
    let lock = CONFIG_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::TempDir::new().expect("failed to create isolated config tempdir");
    // SAFETY: serialized by CONFIG_ENV_LOCK; no other test in this binary
    // can be reading or writing KANBAN_CONFIG concurrently.
    unsafe {
        std::env::set_var("KANBAN_CONFIG", dir.path().join("config.toml"));
    }
    IsolatedConfigGuard {
        _lock: lock,
        _dir: dir,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_config_yields_default_appconfig_regardless_of_ambient_state() {
        let _guard = isolated_config();

        let config = kanban_service::config::load();

        assert_eq!(
            config.storage_location, None,
            "kanban_service::config::load() must return AppConfig::default() \
             while an IsolatedConfigGuard is held, not whatever is on disk at \
             the real KANBAN_CONFIG/$HOME/.config/kanban/config.toml location"
        );
        assert_eq!(config.storage_backend, None);
    }
}
