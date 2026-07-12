---
bump: patch
---

Board, column, and card lists now have a deterministic, stable order. Entities that share the same `position` (for example legacy boards that predate the `position` field and default to `0`) previously fell back to hash-map iteration order, so the projects list and other views could reshuffle on every launch. Ordering now breaks ties by creation time and then by id, giving a total order that is stable across runs. A single shared helper owns this ordering policy so every read path stays consistent.
