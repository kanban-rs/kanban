---
bump: patch
---

service: `create_sprint_from_spec` diverts to `RemoteSprintWrites::create_sprint` before the board FK read when the backend exposes sprint writes, so sprint creation over the HTTP backend no longer hits the local fence. `auto_consume_name` with no explicit name declines loudly (`create_sprint.auto_consume_name over HTTP`) instead of silently changing which pooled name gets consumed.
