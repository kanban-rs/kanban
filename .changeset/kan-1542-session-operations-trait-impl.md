---
bump: minor
---

server: Session now implements KanbanOperations and GraphOperations itself, with mutators routed through state::mutate/state::mutate_unit; the server's read helpers take &Session instead of &KanbanContext. No route, request or response changes.
