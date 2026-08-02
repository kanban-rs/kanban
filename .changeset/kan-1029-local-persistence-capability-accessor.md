---
bump: patch
---

Internal groundwork with no user-visible effect: `KanbanBackend::persistence_metadata()` is
replaced by a `local_persistence()` capability accessor returning `Option<&dyn
LocalPersistence>`, mirroring the existing `remote_writes()` pattern. Only the JSON and SQLite
backends implement `LocalPersistence`; the in-memory and HTTP backends simply have no
capability to return, instead of being forced to answer a question they cannot meaningfully
answer. The TUI's F12 diagnostics panel still shows the writer stamp exactly as before;
behaviour, storage formats, and commands are unchanged.

Completes the KAN-1021 epic.
