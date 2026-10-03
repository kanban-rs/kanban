---
bump: patch
---

cli,mcp,service: a sprint create/update/activate/complete/cancel that committed no longer reports an error when the follow-up name lookup fails. A sprint returned unnamed is re-read once so it keeps its name; only if that also fails is it reported with a null name and a warning on stderr. Adds `kanban_service::resolve_committed_sprint_name`.
