---
bump: minor
---

service: add `KanbanContext::transfer_state_to`, copying the whole workspace onto another `DataStore` by composing `read_full_snapshot`/`write_full_snapshot`. Upserts into the target rather than clearing it first, and runs no FK repair.
