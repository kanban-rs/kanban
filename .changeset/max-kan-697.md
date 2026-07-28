---
bump: patch
---

Internal groundwork: `kanban-service` now has the scaffolding for an HTTP
client backend (`HttpBackend`), which will let the TUI, CLI, and MCP server
connect to a shared `kanban-server` instance over the network instead of only
a local file. This release only adds the module skeleton — it isn't wired
into any user-facing locator or connection flow yet, so there is no visible
behavior change.
