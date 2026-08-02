---
bump: patch
---

**BREAKING:** `McpServer::register_backend` and `CliApp::register_backend` now take two
arguments, `(Box<dyn StoreFactory>, Box<dyn KanbanBackendFactory>)`, instead of one. A
backend registered through the old single-argument signature could pass `has_backends()`
while still being unreachable from `make_backend`, since only the store-level registry was
populated. The new signature makes that failure mode structurally impossible: registering a
backend now always wires both dispatch paths together. Both types also gain a `backends()`
accessor mirroring the existing `registry()`.

Otherwise this is internal groundwork with no other user-visible effect: `StoreManager`
now dispatches `make_backend` purely through an injected `KanbanBackendRegistry` instead of
consulting `#[cfg(feature = ...)]` blocks internally, and each application (`kanban-cli`,
`kanban-mcp`, `kanban-tui`, `kanban-server`) assembles its own pair of registries rather than
asking `kanban-service` for a pre-populated one. Storage formats, commands, and CLI/MCP/TUI
behavior are unchanged.
