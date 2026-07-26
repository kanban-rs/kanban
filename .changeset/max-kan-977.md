---
bump: patch
---

Internal test coverage (no user-facing change): adds regression guards for
three SQLite behaviors that were correct but relied on fragile, previously
untested invariants — foreign-key enforcement is properly restored after the
old-format migration step that temporarily disables it, the safety backup
taken before upgrading an older database file is now verified at every
upgrade step (not just the first one), and restoring a saved snapshot is
confirmed to roll back cleanly instead of leaving the database partially
wiped if something goes wrong partway through.
