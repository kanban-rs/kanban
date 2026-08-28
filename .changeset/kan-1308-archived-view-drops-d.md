---
bump: patch
---

tui: Pressing `d` in the archived cards view no longer fires the red archive flash on a card that is already archived. The key is now dropped in that view, matching the footer, which already omitted it; previously the stray archive also pushed an undo entry, so a following `u` would unarchive the card.
