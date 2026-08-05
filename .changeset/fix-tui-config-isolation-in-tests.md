---
bump: patch
---

Test-only fix: `kanban-tui` tests that exercise `App::new_with_store(_, None)` (the "no data file" startup path) now isolate `KANBAN_CONFIG` instead of implicitly reading the real `$HOME/.config/kanban/config.toml`. Previously, on any machine with a real kanban config carrying a `storage_location` (i.e. any machine that has actually run kanban, not a fresh CI runner), 8 tests across `dialog_tests.rs`, `lifecycle_tests.rs`, and `backend_selection_tests.rs` would deterministically fail locally while passing in CI, because `has_data_file` came back `true` instead of the `false` these tests assume. No production behavior change.
