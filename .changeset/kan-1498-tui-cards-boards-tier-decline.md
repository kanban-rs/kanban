---
bump: patch
---

tui: the 19 remaining `loaded_or_empty()` reads on the cards/boards tiers in kanban-tui's handlers and app modules now handle `LoadState` explicitly. Acting handlers decline with a "not loaded yet" banner and leave state unchanged when their cards or boards tier is not loaded; per-frame and read-only search paths skip silently; the shared test fixture that seeds `active_board_id` now panics loudly instead of silently producing `None` if boards were never loaded.
