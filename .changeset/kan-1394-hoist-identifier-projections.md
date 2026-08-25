---
bump: patch
---

domain: hoist the per-card column/board/sprint projections and the column-to-board index out of `find_cards_by_identifier`'s filter loop, building them once instead of on every card. No behavior change: match order, plurality, prefix normalization, and the has-board guard are all pinned by the existing and new regression tests.
