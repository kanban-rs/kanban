---
bump: minor
---

server: the four collection endpoints (GET /v1/boards, /v1/boards/{id}/columns, /v1/boards/{id}/cards and /v1/boards/{id}/sprints) now accept ?page= and ?page_size= and return a { items, total, page, page_size, total_pages } envelope instead of a bare JSON array, so a client can page through a large board and still know how many rows exist; page_size defaults to 50 and is capped at 500, a page or page_size of 0 is a 422, and a page past the end is a 200 with an empty items list rather than an error.
