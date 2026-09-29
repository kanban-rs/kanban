---
bump: minor
---

backend,backend-http: `RemoteCardWrites` grows three required methods (`move_card`, `assign_card_to_sprint`, `unassign_card_from_sprint`), and `HttpBackend` implements them against the card move and sprint assignment routes. Breaking: `RemoteCardWrites` gains three required methods, so any external `impl RemoteCardWrites for X {}` stops compiling.
