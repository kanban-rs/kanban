---
bump: minor
---

service,backend-http,server: divert board archive and restore to the remote when a board is backed by a server. `archive_board_impl`/`restore_board_impl` check `remote_board_writes()` before any local read and, when present, call it instead of mutating local state directly. `HttpBackend` implements `RemoteBoardWrites::archive_board`/`restore_board` over `POST /v1/boards/{id}/archive` and `/restore`, and the two server route handlers now return `MutationResponse<BoardResponse>` so the invalidation travels on the wire (additive, since `MutationResponse` flattens `BoardResponse`).
