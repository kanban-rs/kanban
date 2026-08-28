---
bump: minor
---

service: undo and redo now derive and record which entities they invalidated, the same way a forward execution does, so a cache attached to the context cannot serve stale entities after an undo; the value is readable via ctx.last_invalidation() and is None until a batch has committed.
