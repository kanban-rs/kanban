---
bump: minor
---

backend-http,server: `RemoteCardWrites` grows three required methods (`move_card`, `assign_card_to_sprint`, `unassign_card_from_sprint`), and `HttpBackend` diverts card move and sprint assign/unassign to the new `POST /v1/cards/{id}/move`, `POST /v1/cards/{id}/assign-sprint` and `POST /v1/cards/{id}/unassign-sprint` routes instead of the local command-execute-then-log path. Breaking for any external `RemoteCardWrites` implementor; additive server API surface. Parity tests prove the diverted ops leave the store graph identical to a local run on both the JSON and SQLite backends.
