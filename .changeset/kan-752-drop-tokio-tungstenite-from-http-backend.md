---
bump: patch
---

Dropped the `tokio-tungstenite` dependency from `kanban-http-backend`. The collaborative transport is server-sent events (SSE), not WebSocket, so the dependency was unused.
