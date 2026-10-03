---
bump: patch
---

backend-http: add HTTP behavioural, request-shape and JSON/SQLite parity coverage for the sprint lifecycle ops (`activate_sprint`, `complete_sprint`, `cancel_sprint`, `carry_over_sprint_cards`) that `RemoteSprintWrites` already diverts to the server, pinning that each returned `Sprint` resolves its name and that carry-over is one request. Docs: the README now lists carry-over as diverted and no longer says the lifecycle writes decline. Test and docs only.
