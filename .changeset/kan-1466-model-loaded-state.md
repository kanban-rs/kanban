---
bump: minor
---

service: `Model` now implements `LoadedState` and `LoadedEntities` from `kanban_service::fetch_plan`, so a `resolve` call can read fetch status and loaded columns directly off the domain `Model` instead of only off the `resolve`-internal `Overlay`.
