---
bump: patch
---

The view-model layer can now apply a per-entity resolve result and mark a failed fetch's scope, so a refresh that only fetched cards no longer discards the loaded boards, columns and sprints, and a backend read failure marks the affected panel instead of leaving it silently stale.
