---
bump: patch
---

Board and column lists in the sortable/filtered views (and the SQLite backend's raw list queries) could still reshuffle nondeterministically when two boards or two columns shared the same position — a gap left over from an earlier ordering fix that covered the in-memory store but not these paths. Boards, columns, and the SQLite board/column list queries now break ties by creation time and then by id, matching the ordering the rest of the app already guarantees, so a duplicated position (for example after archiving a board and creating a new one) no longer produces an unstable order.
