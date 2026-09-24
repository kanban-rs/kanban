---
bump: patch
---

Moving a card to another column now adopts that column's default_status regardless of the card's current status, instead of only when it is Todo. A Blocked card is exempt and keeps its status on moves between non-completion columns.
