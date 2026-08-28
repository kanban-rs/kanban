---
bump: minor
---

persistence-json: keep the pre-migration `.v{N}.backup` after a successful migration instead of deleting it. A migrated file cannot be opened by an older binary, so the backup is the rollback artifact for the whole binary-downgrade window, matching the SQLite backend's existing policy. Delete the backup manually once you are confident you will not downgrade.
