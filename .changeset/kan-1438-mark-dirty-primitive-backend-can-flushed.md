---
bump: minor
---

backend,persistence-json,service: `KanbanBackend` gains a defaulted `mark_dirty()` method, so a backend can be marked dirty and later flushed without routing a fake empty snapshot through `apply_snapshot` just to trip the dirty flag. `JsonDataStore` overrides it to set its real dirty flag; `FaultInjectingBackend` delegates it to the wrapped backend.
