---
bump: minor
---

backend: `RemoteSprintWrites` grows three required methods (`create_sprint`, `update_sprint`, `delete_sprint`). No in-tree implementor exists yet, so this is additive in practice; breaking for any external implementor of the trait.
