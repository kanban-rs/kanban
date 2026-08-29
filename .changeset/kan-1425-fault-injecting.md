---
bump: minor
---

service: add `FaultInjectingBackend` under the `test-helpers` feature, a wrapper over any `Arc<dyn KanbanBackend>` that makes named `DataStore` reads fail on demand and records every intercepted read in call order. It lets the cross-backend contract suite prove that a store error resolves to `Failed` rather than `Missing` on JSON and SQLite, not just in-memory, and lets downstream tests assert that a read did not happen at all. Every trait method delegates to the wrapped backend, including the defaulted ones that real backends override, so the wrapper stays transparent.
