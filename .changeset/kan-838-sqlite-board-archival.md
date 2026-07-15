---
bump: patch
---

Board archiving now works on the SQLite backend. Archiving a board persists it as archived, keeps its columns and cards in place, and reloads correctly; restore and permanent-delete (with undo) all behave the same as on the other backends. The database schema version is raised so an older version of the app safely refuses to open a database that uses this feature instead of misreading it; upgrading writes a one-time backup first.
