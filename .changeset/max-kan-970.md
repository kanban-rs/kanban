---
bump: patch
---

Fixed the TUI so viewing and restoring archived cards works on an archived
board, matching a live board. Previously, once you drilled into an archived
board, pressing `D` to view its archived cards silently did nothing, which
also made restore (`r`) and permanent-delete (`x`) for those cards
unreachable. `e` (edit) was similarly dead in that view despite being
advertised in the footer/help, and now works exactly as it does on a live
board.

`q`/`Q` in that view now correctly back out one level to the archived-boards
list, instead of silently doing nothing. This isn't identical to a live
board, where `q`/`Q` quits the application entirely — the drilled-in archived
view uses `q`/`Q` for back-navigation instead, matching how `Esc` already
behaved there.

Also fixed two related state-leak bugs surfaced while writing this: a stray
key sequence (e.g. `g` awaiting a second `g` for "jump to top") could survive
backing out with `q` and misfire on the next keystroke; and opening search
from the archived-cards view and closing it could strand you in the live
view with a stale reference to the archived board instead of returning you
to the archived-cards view.
