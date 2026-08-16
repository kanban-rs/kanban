---
bump: patch
---

Internal maintenance with no user-visible effect: bumps the `rust-overlay` flake input so `nix develop` resolves stable Rust 1.97.1 instead of 1.93.1 (needed by an in-flight branch depending on a crate with a newer MSRV). The newer clippy this pulls in flags a handful of pre-existing patterns (`manual_checked_ops`, `unnecessary_sort_by`, `collapsible_match`) across `kanban-core`, `kanban-domain`, `kanban-backend-memory`, and `kanban-tui`; all are fixed mechanically with no behavior change.
