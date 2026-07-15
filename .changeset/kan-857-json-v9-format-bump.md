---
bump: patch
---

The JSON save format is now version 9, marking files as archived-board-capable. An older version of the app that predates board archiving will cleanly refuse to open a version-9 file instead of loading it and silently discarding archived boards on its next save. Existing files upgrade automatically on open, writing a one-time backup first.
