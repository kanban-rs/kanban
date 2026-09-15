---
bump: patch
---

api, server: the client-id header name now lives in kanban-api (re-exported from kanban-server's old path) and Patch gains the reverse FieldUpdate conversion, so the HTTP client and the server share one constant and one bidirectional patch mapping
