---
bump: patch
---

api: `SprintResponse` no longer requires the owning `Board` to construct — `SprintResponse::new(sprint, resolved_name)` takes an already-resolved name, so a sprint read never forces a second board fetch just to project it onto the wire
