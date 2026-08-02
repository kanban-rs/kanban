---
bump: patch
---

The JSON file backend now supports archived boards: archiving a board persists it to the on-disk file and it reloads correctly, with the board's columns and cards kept in place. Older JSON files that predate this feature load unchanged. No file-format migration is needed.
