---
bump: patch
---

Repoint doc comments left over from PR #650, which renamed `Model::boards()`, `Model::board_by_id()` and `Model::graph()` to `boards_state()`, `board_by_id_state()` and `graph_state()` but left several comments in `kanban-view` and `kanban-tui` still naming the deleted accessors. No executable code changed.
