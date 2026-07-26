---
bump: patch
---

The board delete/archive confirmation dialog's task count no longer
double-counts archived cards against the separate archived-task figure.
This was the last of a small cluster of display counts (see the previous
release's card-count fixes) that read from an internal collection without
excluding archived cards.
