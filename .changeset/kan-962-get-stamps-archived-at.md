---
bump: patch
---

Fixed single-entity reads returning archived entities as if they were live.
`card get` and `board get` (CLI and MCP, by UUID or identifier) now stamp the
archival marker's `archived_at`, so an archived card or board is clearly marked
instead of being indistinguishable from a live one. This matches what
`card list --archived` already did and keeps the get and list views consistent.
