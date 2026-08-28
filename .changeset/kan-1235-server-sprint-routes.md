---
bump: minor
---

server: sprints are now reachable over HTTP with list, get, create, replace, update and delete routes, both board-nested and via the flat /v1/sprints/{id} alias; listing the sprints of a board that does not exist returns 404 rather than an empty list.
