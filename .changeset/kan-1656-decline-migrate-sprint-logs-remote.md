---
bump: patch
---

service,tui: against a remote kanban server, sprint-log migration now declines immediately with KanbanError::unsupported("migrate_sprint_logs") instead of pulling every card, sprint and board over the wire to compute a backfill that only the store owner should run. The TUI log line for a declined migration no longer claims the decline was a read.
