---
bump: minor
---

Adds `kanban-web`, a server-rendered web-client prototype built on [Topcoat](https://github.com/tokio-rs/topcoat) (tokio-rs): a single read-only page listing boards, columns, and cards straight off `KanbanContext`, with no REST/DTO layer in between. It can run against a local JSON file (`KANBAN_FILE`, matching `kanban-cli`'s convention) or a remote `kanban-server` instance (`KANBAN_SERVER_URL`), via a new real read path in `kanban-backend-http`'s `HttpBackend`: `get_board`, `list_boards`, `get_column`, `list_columns_by_board`, and `list_cards_by_column` now issue real HTTP requests instead of returning `Unsupported`. `HttpBackend`'s write methods and `get_card` remain unimplemented.
