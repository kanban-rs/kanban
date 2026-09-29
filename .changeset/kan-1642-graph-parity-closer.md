---
bump: patch
---

backend-http: `write_parity.rs`'s `GraphSnapshot` now compares each spawns/blocks/relates edge's archival state, not just its endpoints, and adds `test_remote_graph_mutations_leave_graph_equal_to_local_{json,sqlite}` (the six graph edge mutations over HTTP, whole-store parity against a local run) and `test_attach_no_children_over_http_returns_the_same_invalidation_as_local` (an empty `attach_children` invalidates the same thing over HTTP as it does locally, since the server is the reference). Test-only; no src item changes shape.
