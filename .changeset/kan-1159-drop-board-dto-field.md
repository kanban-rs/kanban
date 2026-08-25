---
bump: minor
---

BREAKING: remove completion_column_ids from the REST board DTOs (BoardResponse, UpdateBoardRequest, ReplaceBoardRequest); clients configuring board completion via the wire must migrate to per-column default_status. completion_column_ids never shipped in a released version, so no existing client sent it; a later fix (see the changeset for the singular `completion_column_id` removal) rejects it explicitly on write instead of ignoring it.
