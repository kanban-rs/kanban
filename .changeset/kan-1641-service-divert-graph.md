---
bump: patch
---

service: the six graph mutators (`attach_children_impl`, `detach_children_impl`, `block_impl`, `unblock_impl`, `relate_impl`, `dissociate_impl`) now divert to `KanbanBackend::remote_graph_writes()` as their first statement, before `require_card_exists`/`edge_born_archived` run any client read. Over `HttpBackend` each graph op is now one request instead of up to `2 + 3N` client reads followed by the blanket fence.
