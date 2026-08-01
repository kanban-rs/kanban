---
bump: patch
---

Correct the CLAUDE.md architecture reference: the dependency graph now shows the JSON and SQLite persistence crates depending on kanban-backend and kanban-backend-memory (they currently host their KanbanBackend adapters), both crate descriptions note that adapter placement, and the JSON envelope version is updated from V10 to the current V11.
