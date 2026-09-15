---
bump: minor
---

server: `Session` no longer retains a `kanban_domain::Model` between requests; every read handler builds its own for the duration of its own request. Removes the public `Session::model` field, `AppState::with_reset` and `AppState::reset_model_per_request`. `state::mutate` now returns `KanbanResult<(T, Invalidation)>` and `state::mutate_unit` returns `KanbanResult<Invalidation>`, instead of applying the Invalidation to a retained Model themselves.
