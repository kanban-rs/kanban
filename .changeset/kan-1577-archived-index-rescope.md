---
bump: patch
---

service: `archived_card_index` now takes an optional board scope, so a board-scoped card listing reads only that board's archival markers instead of the workspace-global collection. Board-scoped `card list` now works over a remote (HTTP) locator, which previously failed with `Unsupported: list_archived_cards`. Unscoped listings keep the global read and its decline. Local backends are unchanged: the board-scoped read is the same subset the archival stamping already consulted.
