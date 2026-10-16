---
bump: minor
---

backend-http,backend,domain,api: a client no longer refuses an older kanban server or one that reports no version; HttpBackend::probe() logs a one-time compatibility warning (CLI/MCP stderr, TUI F12 log) and opens, and writes degrade per operation. Breaking: KanbanError::UnsupportedServerVersion is removed. Adds kanban_backend::CompatibilityNotice and KanbanBackend::compatibility_notice().
