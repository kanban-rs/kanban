---
bump: patch
---

Archiving a board that is already archived (MCP `archive_board`, CLI `board archive`, `POST /v1/boards/{id}/archive`) no longer resets its `archived_at`, and undoing it no longer un-archives the board and its contents.
