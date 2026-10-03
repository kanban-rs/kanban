---
bump: minor
---

backend,backend-http: `RemoteSprintWrites` grows four required methods (`activate_sprint`, `complete_sprint`, `cancel_sprint`, `carry_over_sprint_cards`), and `HttpBackend` implements them against the flat `POST /v1/sprints/{id}/activate|complete|cancel|carry-over` routes added alongside the nested ones. `duration_days` travels through untouched as `Option<i32>` so the wire never claims a default the caller didn't ask for; the server still applies its own default of 14 when absent. Breaking for any external `RemoteSprintWrites` implementor.
