---
bump: minor
---

service: `KanbanContext` gains six `pub` inherent graph mutators
(`attach_children_impl`, `detach_children_impl`, `block_impl`, `unblock_impl`,
`relate_impl`, `dissociate_impl`) returning `KanbanResult<Invalidation>`, with
the `GraphOperations` trait impl reduced to thin discards over them.
`create_board_from_spec` now returns `(Board, Invalidation)` and
`create_or_replace_board` now returns `(BoardCreateOutcome, Invalidation)`;
the internal `create_board_from_spec_returning` helper is removed.
