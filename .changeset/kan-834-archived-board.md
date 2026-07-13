---
bump: patch
---

Internal foundation for board archiving (no user-facing change): boards can now be represented as first-class archived records via the shared archival abstraction (`ArchivedBoard = Archived<Board>`), and snapshots gain a discrete `archived_boards` collection alongside `archived_cards`. Older data without it loads unchanged. Commands, persistence, and UI for board archiving arrive in later slices.
