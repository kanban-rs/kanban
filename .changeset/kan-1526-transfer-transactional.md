---
bump: minor
---

service: `KanbanContext::transfer_state_to` now takes the target as `&dyn KanbanBackend` instead of `&dyn DataStore` and runs the whole snapshot write inside `target.with_transaction(..)`, so a mid-write referential-integrity failure rolls the target back to unchanged instead of leaving it half-populated. tui: `TuiContext::transfer_state_to` takes the same new parameter type; its one caller in `App::adopt_storage_file` now passes the backend directly.
