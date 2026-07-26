---
bump: patch
---

Hardened the board view's column ordering so it can no longer disagree with
itself in edge cases (two columns ending up sharing the same internal
position value, which can happen after deleting and recreating columns).
Column list rendering, the move-column-up/down commands, the rename/delete
column dialogs, and the board detail view now all resolve column order the
same consistent way instead of each computing it separately.

