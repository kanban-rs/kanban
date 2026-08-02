---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). Fixes a critical data-loss bug: `kanban-server`, when backed by the JSON store, never wrote any board or column changes to disk — every mutation only ever existed in memory and was lost the moment the process exited, restarted, or crashed. Every write route now durably persists the change before reporting success back to the client.
