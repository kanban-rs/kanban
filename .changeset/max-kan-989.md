---
bump: patch
---

Fixed a bug where moving cards between columns without also changing their
status (for example the bulk multi-select "move left/right" action in the
TUI) could silently exceed a column's WIP limit, and could leave a card
pointing at the wrong board after a cross-board move. Both of these are now
enforced the same way a direct drag-and-drop move already enforces them, so
the two paths behave consistently.
