---
bump: patch
---

domain: documentation only. The `spawns` relation is described as a directed acyclic graph rather than a hierarchy, and the doc comments on `SpawnsEdge` and `CardEdgeType::Spawns` now state that a card may have more than one parent. No behaviour changed; the code already supported this and the prose did not say so, which led a reviewer to report the absence of a single-parent check as a defect.
