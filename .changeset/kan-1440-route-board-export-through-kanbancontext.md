---
bump: minor
---

tui: adds `TuiContext::export_all_boards`, a pass-through to `KanbanContext::export_all_boards`, and points board export (single-board, export-all, and auto-save) at it instead of building the export from a raw `DataStore::snapshot` call.
