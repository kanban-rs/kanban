---
bump: patch
---

service: `archive_cards_detailed`, `move_cards_detailed` and `assign_cards_to_sprint_detailed` divert to `RemoteBatchWrites` when the backend has one, matching the non-detailed batch ops. A batch write transport error, or a remote backend with no batch-write support, now fails every input id with the per-op message instead of falling through to the local command path.
