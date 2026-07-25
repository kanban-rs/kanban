---
bump: minor
---

**Breaking change for MCP clients**: all six MCP list tools (`list_boards`, `list_columns`, `list_sprints`, `list_card_parents`, `list_card_children`, and the already-paginated `list_cards`) now consistently return the same paginated envelope shape: `{ "items": [...], "total": N, "page": N, "page_size": N, "total_pages": N }`, with `page`/`page_size` request parameters on every one of them (default: page 1, page size 50).

Previously only `list_cards` used this shape. `list_boards` returned a bare array when no page parameters were supplied and the envelope only when they were — the same tool call could return two different shapes depending on input. `list_columns`, `list_sprints`, `list_card_parents`, and `list_card_children` always returned a bare array with no way to paginate at all.

Any client that read these tools' results as a bare JSON array needs to read `.items` instead, and should use `total` to detect when a result has more entries than fit on one page.
