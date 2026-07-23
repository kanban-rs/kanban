---
bump: patch
---

Fixed the TUI so viewing and restoring archived cards works on an archived
board, matching a live board. Previously, once you drilled into an archived
board, pressing `D` to view its archived cards silently did nothing, which
also made restore (`r`) and permanent-delete (`x`) for those cards
unreachable. `q`/`Q` (back out one level) and `e` (edit) were similarly dead
in that view despite being advertised in the footer/help. All four now work
exactly as they do on a live board.
