---
bump: patch
---

mcp: retarget card_batch.rs's name-resolution call sites (tool_archive_cards,
tool_move_cards, tool_assign_cards_to_sprint, tool_assign_card_to_sprint) from
the McpResolve shim to a call-scoped Model built through ToolScope, and fix
resolve_cards to require the card-list tier lazily so an all-uuid batch no
longer trips an unfetched-tier error. A failed or unfetched column/sprint
collection read now surfaces an error naming "columns of the board" or
"sprints of the board" instead of a raw backend error. Singular card
identifier resolution (tool_unassign_card_from_sprint, and the card lookup
inside tool_assign_card_to_sprint) is left on the indexed
KanbanOperations::resolve_card_id path rather than moved to the Model, since
that path is already an indexed lookup and migrating it would trade an
indexed read for a full collection scan with no correctness gain.
card_relations.rs is unchanged: it has zero McpResolve shim call sites and its
graph reads already surface a failed read as an error.
