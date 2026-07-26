---
bump: patch
---

Fixed a bug where moving multiple selected cards to another column at once
(multi-select `H`/`L`) could silently place a card on top of an existing
card in the destination column instead of appending it after the others.
The moved card kept its old position from the source column rather than
getting a fresh position in the destination, so it would only avoid
colliding by coincidence. Moving several cards into the same column in one
action now gives each of them its own position, in order.
