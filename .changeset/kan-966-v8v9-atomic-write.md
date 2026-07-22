---
bump: patch
---

Hardened the JSON V8 to V9 migration to write atomically (temp file plus rename),
matching every other migration step. Previously it wrote the upgraded file in
place, so a crash mid-write could truncate it; recovery still needed the backup.
Now a crash leaves the original file intact.
