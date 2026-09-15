---
bump: patch
---

Switching storage location mid-session, or adopting a new storage file, now re-points live change detection at the new location instead of leaving it on the old one, so a switch to a remote server starts receiving other clients' changes without a restart.
