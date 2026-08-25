---
bump: patch
---

domain: collapse the five duplicate-id validation loops in `ImportEntities::execute` into a single `reject_duplicate_ids` helper. No behavior change: same error type, message and check order for boards, columns, cards, archived cards, sprints and archived boards.
