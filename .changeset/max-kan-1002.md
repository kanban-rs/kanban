---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). `kanban-server`, when backed by the JSON store, now detects and reloads when the underlying file is changed by another process (the TUI, CLI, or MCP server) — previously it only ever read the file once at startup and could go stale until restarted.

Also fixes two bugs in the shared `FileWatcher` component (also used by the TUI): starting a watch on a locator whose file doesn't exist yet no longer fails, and starting a watch now waits until the OS-level watch is actually armed before returning, closing a race where a write landing immediately after startup could be missed.
