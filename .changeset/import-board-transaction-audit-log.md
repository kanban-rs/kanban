---
bump: patch
---

`kanban-service`: `import_board_impl` now runs transactionally and appends an
audit-log entry, matching every other mutation's guarantee. Previously a
successful board import left no audit-log record and had no rollback
guarantee on partial failure. Import still clears undo history afterward
(unchanged) — it remains intentionally not undoable via the normal undo stack.
