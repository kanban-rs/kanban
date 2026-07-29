---
bump: patch
---

Internal groundwork for the upcoming HTTP collaborative backend: `kanban-service` gained a `RemoteWrites` capability hook that lets a future backend delegate board/column/card create/update/delete directly to a remote server instead of applying them locally. This release has no user-visible effect — no backend uses the hook yet, and every existing local backend (JSON, SQLite, in-memory) behaves exactly as before.
