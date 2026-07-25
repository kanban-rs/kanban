---
bump: patch
---

Card counts no longer include archived cards. Sprint Detail's "Cards
Assigned" count, Board Detail's per-sprint and per-column counts, and the
Carry-Over Sprint popup's card count could previously stay inflated
forever after a card was archived, since archiving keeps a card's sprint
and column assignment intact rather than clearing it. All four now count
only live cards.
