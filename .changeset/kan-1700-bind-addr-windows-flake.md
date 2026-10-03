---
bump: patch
---

server (tests only): the bind_addr integration tests re-probe a fresh port and retry up to three times when the spawned server fails to report its address, and include the server's stderr in the failure message.
