---
bump: minor
---

cli,domain: breaking change to the CLI contract. `kanban column get`, `kanban column update`, `kanban column delete`, `kanban column reorder`, and the board-less column-name arm of `kanban card list` now require a `--board` when the column is given by a name; resolution by column UUID is unaffected. Columns belong to a board and column names are not unique across boards, so a board-less name lookup was ambiguous by construction and failed outright against a remote backend. `KanbanOperations::resolve_column_id_global` is removed; every name lookup now routes through the existing board-scoped `resolve_column_id`, which already works remotely. Removing a public trait's default method is breaking for out-of-tree implementors, hence `minor` under the pre-1.0 rule.
