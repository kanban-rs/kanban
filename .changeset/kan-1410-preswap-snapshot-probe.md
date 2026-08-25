---
bump: patch
---

tui: probe the post-swap snapshot read before installing the new storage backend. A failed read after a storage-location swap used to leave the app with no save worker and a dead completion receiver, silently breaking persistence until restart.
