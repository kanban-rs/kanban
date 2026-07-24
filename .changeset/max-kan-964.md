---
bump: patch
---

Board, column, and card lists in the sortable/filtered views (and the SQLite backend's raw list queries) could still reshuffle nondeterministically when two entities shared the same position, or two cards additionally shared the same creation time — a gap left over from an earlier ordering fix that covered the in-memory store but not these paths. Boards, columns, and cards now break ties by creation time and then by id everywhere position ordering happens, matching the ordering the rest of the app already guarantees, so a duplicated position (for example after archiving a board and creating a new one) no longer produces an unstable order.
