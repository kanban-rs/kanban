---
bump: patch
---

Internal infrastructure change with no user-visible behaviour difference. Every command batch executed by the tool now carries a correlation ID, a session ID, and app-surface attribution (which app issued it) through the audit log. Client identity is populated when commands are routed through the upcoming HTTP collaborative backend; locally it is left unset. This prepares the persistence layer for multi-client attribution in the command log and real-time event stream.
