---
bump: patch
---

Add tests pinning six already-shipped remote path behaviours: SSE CRLF tolerance and multi-data-line joining, graph tier invalidate and refetch over HttpBackend, an MCP graph-backed tool over an http locator, the startup scope's flat-tier avoidance over HttpBackend, and non-None optionals surviving the create wire in write parity. No production behaviour changes.
