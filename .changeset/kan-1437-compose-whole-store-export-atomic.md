---
bump: minor
---

service: route the whole-store export branch of `export_board(None)` through `read_full_snapshot` instead of `DataStore::snapshot`, and add a new `KanbanContext::export_all_boards()` that composes an `AllBoardsExport` directly, matching the two-step snapshot-then-convert flow callers previously had to do by hand.
