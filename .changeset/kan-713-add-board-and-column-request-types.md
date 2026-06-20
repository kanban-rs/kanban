---
bump: patch
---

Added the v1 HTTP API foundation and the full board/column DTO surface to the kanban-service api module (`kanban_service::api`).

Foundation: a `Patch<T>` wire type implementing JSON Merge Patch (RFC 7386) for PATCH bodies (absent = no change, `null` = clear, value = set), a paginated list envelope (`Page<T>`, `PageParams`), and five new machine-readable `ErrorCode` variants (`BATCH_RESOLUTION_FAILED`, `CYCLE_DETECTED`, `SELF_REFERENCE`, `EDGE_NOT_FOUND`, `DUPLICATE_EDGE`).

Board/column: create (`CreateBoardRequest`/`CreateColumnRequest`), PATCH (`UpdateBoardRequest`/`UpdateColumnRequest`, now merge-patch), PUT full-replace (`ReplaceBoardRequest`/`ReplaceColumnRequest`), read responses (`BoardResponse`/`ColumnResponse`), and `ReorderColumnRequest`. Request types convert to the domain `BoardUpdate`/`ColumnUpdate` via exhaustive `From`/`TryFrom` conversions that exclude server-managed fields and validate at the boundary (e.g. non-negative `wip_limit`/`position`). `BoardResponse` omits internal allocation state (counters and sprint-name pools). These are wire types only; routing and handlers come with the server crate.
