---
bump: patch
---

persistence-sqlite: opening a database now repairs any card whose prefix namespace has no row, so upgrading a workspace where an archived card outlived its column no longer leaves an identifier pointing at a namespace the database does not know about.
