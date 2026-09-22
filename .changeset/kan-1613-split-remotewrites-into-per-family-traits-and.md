---
bump: patch
---

backend,backend-http: split RemoteWrites into per-family traits (RemoteBoardWrites, RemoteCardWrites, RemoteBatchWrites, RemoteSprintWrites, RemoteGraphWrites) as independently optional seams on KanbanBackend. The existing RemoteWrites trait is unchanged and HttpBackend still returns Some only for the board and card families it already covers, so this is a no-behavior-change refactor.
