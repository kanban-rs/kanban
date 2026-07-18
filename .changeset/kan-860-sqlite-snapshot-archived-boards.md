---
bump: patch
---

Fixes data loss where exporting or snapshotting a SQLite database silently dropped every archived board. Archived boards are now included in a SQLite snapshot and restored on import, so exporting a SQLite board to JSON (or copying between backends) preserves them.
