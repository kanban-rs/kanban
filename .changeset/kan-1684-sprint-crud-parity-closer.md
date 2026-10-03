---
bump: patch
---

backend-http: adds behavioural and parity coverage for the sprint create/update/delete mutations `RemoteSprintWrites` already diverts to the server. `test_create_sprint_over_http_hits_the_sprint_route_and_returns_a_named_sprint` pins that the returned `Sprint` resolves its name against the board's `sprint_names` pool rather than an empty one; `test_update_sprint_over_http_sets_dates` and `test_delete_sprint_over_http_returns_the_server_invalidation` round out the family, and `test_remote_sprint_crud_leaves_graph_equal_to_local_{json,sqlite}` prove the counters and name pool match a local run on both backends. Test and docs only; `RemoteSprintWrites` and the flat sprint `DELETE` route landed in earlier children of KAN-1618.
