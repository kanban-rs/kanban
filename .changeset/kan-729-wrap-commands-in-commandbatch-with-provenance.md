---
bump: patch
---

Internal infrastructure change with no user-visible behaviour difference. Every command executed by the tool now carries a correlation ID and client identity through the audit log. This prepares the persistence layer for the upcoming HTTP collaborative backend, where mutations from multiple clients need to be attributable in the command log and real-time event stream.
