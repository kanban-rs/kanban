---
bump: major
---

Remove completion_column_ids from the REST board DTOs (BoardResponse, UpdateBoardRequest, ReplaceBoardRequest); clients configuring board completion via the wire must migrate to per-column default_status. Legacy request bodies that still send the key are ignored, not rejected.
