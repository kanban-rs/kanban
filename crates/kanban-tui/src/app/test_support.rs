//! Test-only helpers shared by `dialog_tests` and `lifecycle_tests`.
//!
//! `App::new_with_store` calls `kanban_service::config::load()`, which reads
//! the real `$HOME/.config/kanban/config.toml` unless `KANBAN_CONFIG`
//! overrides it. On any machine that has actually run `kanban` for real
//! (i.e. every developer's machine, just not a fresh CI runner), that file
//! can carry a genuine `storage_location`, which makes
//! `App::new_with_store(sm, None)` observe `has_data_file = true` instead of
//! the `false` these tests assume.

use std::ffi::OsString;
use std::sync::MutexGuard;

/// Held for the duration of a test that needs `kanban_service::config::load()`
/// to see an empty config regardless of what's on disk at the real location.
/// Pins `KANBAN_CONFIG` to a nonexistent path inside a private `TempDir` on
/// construction, and restores whatever `KANBAN_CONFIG` held before (or clears
/// it, if it was unset) on drop.
pub(in crate::app) struct IsolatedConfigGuard {
    _lock: MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
    prev: Option<OsString>,
}

impl Drop for IsolatedConfigGuard {
    fn drop(&mut self) {
        // SAFETY: `_lock` (crate::test_helpers::ENV_LOCK) is still held here —
        // it's a field on `self`, dropped only after this fn returns — so no
        // other test in this binary can be reading or writing any
        // environment variable concurrently.
        unsafe {
            match self.prev.take() {
                Some(prev) => std::env::set_var("KANBAN_CONFIG", prev),
                None => std::env::remove_var("KANBAN_CONFIG"),
            }
        }
    }
}

/// Acquire the process-wide env-isolation lock and pin `KANBAN_CONFIG` to
/// an empty, nonexistent path until the returned guard is dropped.
pub(in crate::app) fn isolated_config() -> IsolatedConfigGuard {
    let lock = crate::test_helpers::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var_os("KANBAN_CONFIG");
    isolated_config_holding(lock, prev)
}

/// Same as [`isolated_config`], but takes an already-held lock and the value
/// to restore on drop instead of acquiring/capturing them itself — lets a
/// caller that also needs to mutate `KANBAN_CONFIG` before overriding it
/// (see the unit test below) capture the *true* prior value ahead of its own
/// mutation, under one uninterrupted hold, rather than have the guard
/// capture what the caller just set.
fn isolated_config_holding(
    lock: MutexGuard<'static, ()>,
    prev: Option<OsString>,
) -> IsolatedConfigGuard {
    let dir = tempfile::TempDir::new().expect("failed to create isolated config tempdir");
    // SAFETY: serialized by crate::test_helpers::ENV_LOCK; no other test in
    // this binary can be reading or writing any environment variable
    // concurrently.
    unsafe {
        std::env::set_var("KANBAN_CONFIG", dir.path().join("config.toml"));
    }
    IsolatedConfigGuard {
        _lock: lock,
        _dir: dir,
        prev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_config_yields_default_appconfig_regardless_of_ambient_state() {
        let dir = tempfile::TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            "storage_location = \"/tmp/some-real-boards.json\"\nstorage_backend = \"json\"\n",
        )
        .unwrap();

        let lock = crate::test_helpers::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let true_prev = std::env::var_os("KANBAN_CONFIG");
        // SAFETY: serialized by crate::test_helpers::ENV_LOCK, held
        // continuously from here through `isolated_config_holding` below.
        unsafe {
            std::env::set_var("KANBAN_CONFIG", &config_path);
        }

        let _guard = isolated_config_holding(lock, true_prev);

        let config = kanban_service::config::load();

        assert_eq!(
            config.storage_location, None,
            "kanban_service::config::load() must return AppConfig::default() \
             while an IsolatedConfigGuard is held, not the storage_location \
             seeded at the ambient KANBAN_CONFIG path"
        );
        assert_eq!(config.storage_backend, None);
    }
}
