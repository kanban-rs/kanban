---
bump: patch
---

Card counts and lists no longer include archived cards. Sprint Detail's
"Cards Assigned" count, Board Detail's per-sprint and per-column counts,
the Carry-Over Sprint popup's card count (and the check for whether that
popup should open at all), and a completed sprint's Uncompleted task
panel could previously keep including an archived card forever, since
archiving keeps a card's sprint and column assignment intact rather than
clearing it. All of these now reflect only live cards.
