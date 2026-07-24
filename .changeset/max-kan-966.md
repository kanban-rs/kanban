---
bump: patch
---

The JSON storage backend's V1-to-V2 and V8-to-V9 format migrations now write their results via the same atomic temp-file-and-rename pattern the rest of the migration chain already used. Previously these two steps wrote the migrated file in place, so a crash or power loss during either specific write could leave the board file truncated or corrupted. They now benefit from the same crash-safety guarantee as every other step in the chain.
