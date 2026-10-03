---
bump: patch
---

service: `update_sprint_impl` and `delete_sprint_impl` divert to `RemoteSprintWrites` when the backend supports it, matching the diversion already in place for sprint create. The diverted update returns the server's `Sprint` directly rather than re-reading it locally.
