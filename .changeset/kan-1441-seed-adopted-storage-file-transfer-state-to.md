---
bump: minor
---

kanban-tui: route the storage-adoption dialog's `adopt_storage_file` through the new `TuiContext::transfer_state_to` instead of a whole-store `snapshot`/`apply_snapshot` round trip, replace the post-seed readability probe with `backend.list_boards()`, and explicitly `mark_dirty()` the seeded backend so the queued flush writes to disk even for an empty workspace.
