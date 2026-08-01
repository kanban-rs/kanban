---
bump: patch
---

Removes the `json` feature from `kanban-service`. `sqlite` is now the crate's
only optional persistence feature; `kanban-persistence-json` and
`kanban-backend-memory` moved from optional production dependencies to
unconditional dev-dependencies.

This is internal groundwork with no user-visible behavior change:
`kanban-service`'s production surface no longer depends on the JSON
persistence crate directly. `kanban-cli` and `kanban-mcp` already gate their
own `kanban-persistence-json` dependency independently, so nothing changes
for consumers of those binaries. Storage formats, commands, and CLI/MCP/TUI
behavior are unchanged.

Completes the KAN-1027 split (final child, after KAN-1032 and KAN-1033).
