---
bump: patch
---

persistence-sqlite: SQLite workspaces now refuse, at the database level, to delete or rename a prefix any card was numbered under, and refuse to store a card naming a prefix that has no row. Databases are upgraded to schema 13 on open, and a `.v12.backup` is kept before the upgrade.
