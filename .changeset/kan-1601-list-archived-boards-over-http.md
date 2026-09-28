---
bump: patch
---

backend-http: implement `list_archived_boards` over HTTP. The server already exposed `GET /v1/archived-boards`, but `HttpBackend` declined the read unconditionally; it now pages through that route with the existing `get_json_list` helper and converts each marker with a new `archived_board_from_response`. Unscoped `card list` over HTTP now progresses past the archived-board read instead of failing on it.
