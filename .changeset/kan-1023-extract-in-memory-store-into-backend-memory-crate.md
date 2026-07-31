---
bump: patch
---

Internal restructuring with no user-visible effect: the in-memory store now lives in its own `kanban-backend-memory` crate instead of inside `kanban-domain`. It becomes an ordinary backend alongside JSON, SQLite, and HTTP, so an application can choose to run with no storage backend, with one, or with a combination, rather than always inheriting an in-memory implementation from the domain layer. `kanban-domain` is now purely entities, commands, and trait definitions. All existing behaviour, on-disk formats, and command-line usage are unchanged.
