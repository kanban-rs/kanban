---
bump: patch
---

service: `activate_sprint_impl`, `complete_sprint_impl`, `cancel_sprint_impl` and `carry_over_sprint_cards_impl` divert to `RemoteSprintWrites` as their first statement when the backend supports it, and decline with a per-op `unsupported` error over `HttpBackend` without one. `carry_over_sprint_cards_impl` now issues a single request over HTTP instead of two sprint reads plus a full live-card walk.
