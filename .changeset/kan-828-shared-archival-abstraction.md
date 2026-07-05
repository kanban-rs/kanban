---
bump: patch
---

Internal foundation with no user-visible behaviour change. Adds a bounded shared archival abstraction to the domain layer: an `ArchiveMetadata` envelope and a thin `ArchivedEntity` trait (stable id + metadata), implemented first by `ArchivedCard`. Also adds a `KanbanError::unsupported` affordance (a `DomainError::Unsupported` variant) that later slices use as the default body for backend methods not yet implemented, so trait extensions compile cleanly. This is the groundwork the archived-cards retrofit and board archival build on; nothing changes for users yet.
