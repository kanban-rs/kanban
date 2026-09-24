---
bump: minor
---

domain: adds an `UndoOperations` trait (`undo`, `redo`, `can_undo`, `can_redo`, no default bodies, object-safe) over the per-session undo/redo capability.
service: `KanbanContext`'s undo/redo/can_undo/can_redo move from an inherent impl onto `impl UndoOperations for KanbanContext`; behaviour is unchanged.
tui: `TuiContext` implements `UndoOperations` directly, now returning the full `Option<Invalidation>` instead of a bool, and still queues a save flush only when something was actually undone or redone.
mcp: `McpContext` implements `UndoOperations` by delegating to the inner context; `tool_undo`/`tool_redo` are unchanged.
cli: `CliContext` implements `UndoOperations` by declining undo and redo with `KanbanError::unsupported`, since the CLI opens a fresh context per invocation.
server: `Session` implements `UndoOperations` by declining undo and redo with `KanbanError::unsupported`, since a session is shared across clients and must not silently inherit the inner context's undo history through `Deref`.
