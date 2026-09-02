---
bump: minor
---

service: create_card_from_spec, create_or_replace_card, create_column_from_spec, create_or_replace_column, create_sprint_from_spec and create_or_replace_sprint on `KanbanContext` now return `KanbanResult<(T, Invalidation)>` instead of `KanbanResult<T>`, mirroring the existing create_board_from_spec / create_or_replace_board precedent. Every caller in kanban-mcp, kanban-server and kanban-cli discards the invalidation explicitly for now.
