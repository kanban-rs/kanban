---
bump: patch
---

persistence-sqlite: opening a database now also raises a naming prefix's counter when a card already carries a higher number, so the next card created in that namespace cannot re-use an identifier a live card already holds.
