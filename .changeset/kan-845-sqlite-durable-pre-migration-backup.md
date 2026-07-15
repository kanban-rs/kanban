---
bump: patch
---

The SQLite backend now writes a durable, transactionally-consistent backup
of the database (`<path>.v<from_version>.backup`, via `VACUUM INTO`) before
running an irreversible schema upgrade, so a user can roll back to the
previous binary version and recover their pre-upgrade data. The backup is
written atomically (to a scratch file, then renamed into place) so a crash
or disk-full mid-copy can never leave a truncated file mistaken for a valid
backup, and it is skipped if a backup for that source version already
exists. The naming convention matches the JSON backend's `.v{N}.backup`
scheme, though unlike the JSON backend's per-step backup it is kept
indefinitely rather than removed on success, since it exists to support
rollback across a binary downgrade rather than just crash recovery within
a single migration step.
