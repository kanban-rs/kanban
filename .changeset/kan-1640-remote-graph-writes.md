---
bump: minor
---

backend,backend-http: `RemoteGraphWrites` grows six required methods (`attach_children`, `detach_children`, `block`, `unblock`, `relate`, `dissociate`), and `HttpBackend` implements them against the graph mutation routes, turning on the `remote_graph_writes()` accessor. Breaking: `RemoteGraphWrites` gains six required methods, so any external `impl RemoteGraphWrites for X {}` stops compiling. No in-tree backend implemented it before.
