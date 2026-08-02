---
bump: patch
---

Added three new crate stubs to the workspace -- `kanban-api`, `kanban-server`, and `kanban-http-backend` -- and registered the shared HTTP dependencies (`axum`, `reqwest`, `tower`, `tower-http`, `tokio-tungstenite`, `prometheus`) in the workspace manifest. These are empty scaffolds only; no features are available yet. This prepares the workspace for the HTTP collaborative backend implementation (KAN-684).
