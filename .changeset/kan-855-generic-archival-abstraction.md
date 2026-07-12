---
bump: patch
---

Internal refactor (no user-facing change): archival is now a single reusable abstraction. Archived records are modeled generically as an entity plus its shared archive metadata and an entity-specific restore context, so archiving any entity is a matter of implementing one small trait rather than hand-rolling a bespoke type. Archived cards move onto this abstraction; already-saved archived-card data keeps loading unchanged.
