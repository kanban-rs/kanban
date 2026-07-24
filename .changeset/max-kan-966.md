---
bump: patch
---

The JSON storage backend's V8-to-V9 format migration (adding archived-board support) now writes its result via the same atomic temp-file-and-rename pattern every other migration step already used. Previously this one step wrote the migrated file in place, so a crash or power loss during that specific write could leave the board file truncated or corrupted. It now benefits from the same crash-safety guarantee as the rest of the migration chain.
