---
bump: patch
---

Re-archiving an already-archived card in a batch (CLI `card archive`, `POST /v1/cards/batch/archive`, MCP `archive_cards` over HTTP) now reports it as archived instead of not found. A failed remote batch (`archive_cards`, `move_cards`, `assign_cards_to_sprint`) now invalidates the whole client cache instead of nothing, since the server may have applied the batch before the connection dropped.
