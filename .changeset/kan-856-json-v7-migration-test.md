---
bump: patch
---

Internal test coverage (no user-facing change): adds an end-to-end regression test that a legacy JSON file (format V7) migrates forward correctly and coexists with board archiving — archived cards keep their data with the board reference backfilled, the archived-boards collection defaults to empty, and archiving a board works on the migrated file.
