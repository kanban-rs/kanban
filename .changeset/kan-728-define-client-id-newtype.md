---
bump: patch
---

Added a `ClientId` newtype to `kanban-core`. This is an internal infrastructure type with no user-visible behaviour change. It provides typed identity for connected clients and will be used by the upcoming HTTP collaborative backend to attribute mutations in the audit log and real-time event stream.
