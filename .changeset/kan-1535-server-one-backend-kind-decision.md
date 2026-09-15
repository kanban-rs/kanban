---
bump: patch
---

the file watcher and the per-request Model reset now share one backend-kind decision derived from the registered store manager, so a JSON file at a .db name is watched for external writes instead of silently serving stale data
