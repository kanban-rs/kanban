---
bump: minor
---

api: `ChangeEventFrame` gains an additive `invalidation: Option<InvalidationDto>` field naming a mutation's full blast radius (every board/column/card/sprint id it touched), alongside the existing single `entity_type`/`entity_id`/`kind`. Adds `EntityIdsDto`/`InvalidationDto` as new public exports and `ChangeEventFrame::with_invalidation`.

server: `AppState::broadcast_change` and `AppState::persist_and_broadcast` take a new required `&Invalidation` parameter, the value the mutation's seam returned; `broadcast_unscoped_change` now emits `InvalidationDto::All`. Every write route threads the invalidation from its mutation through to the broadcast, so a subscriber can invalidate exactly what changed instead of falling back to a whole-cache refresh.
