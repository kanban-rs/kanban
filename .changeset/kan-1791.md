---
bump: minor
---

Cards now support a free-form list of labels on create and update, queryable through the MCP `list_cards` labels filter and the `kanban card list --labels` CLI flag. A global label registry is also added: each label has a unique, case-sensitive name plus optional color and description; `kanban label list|get|upsert|delete` and the MCP tools `list_labels`, `get_label`, `upsert_label`, `delete_label` manage it end to end. SQLite database schema is bumped to v15, adding a `labels` table and a `card_labels` join table (both with cascade-delete foreign keys).

Breaking: `Snapshot` gains a `labels: Vec<Label>` field, so all external full-struct literal initializers must be updated; `KanbanOperations` gains six label methods (`list_labels`, `get_label`, `upsert_label`, `delete_label`, `assign_label_to_cards`, `remove_label_from_cards`) and `DataStore` gains matching trait methods — downstream implementors of either trait need to add them.
