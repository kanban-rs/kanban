---
bump: patch
---

Fixed the TUI archived-board drill-in so it behaves exactly like a live board.
After opening an archived board's card list, `Esc` (or `q`) now backs out to the
archived-boards list instead of leaving you stranded, and card actions such as
archive (`d`), set priority (`p`), open detail, and move now work on an archived
board's cards, matching a live board. Previously the drill-in used a
boards-list-only key dispatch that silently dropped both the back-out and every
card key, even though the help overlay advertised them.
