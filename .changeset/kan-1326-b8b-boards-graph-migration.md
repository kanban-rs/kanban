---
bump: patch
---

view: every call site that read a `Model`'s boards, resolved a board by id, or read the dependency graph now goes through the load-state accessors, so "never loaded" is named instead of silently collapsing into "empty". The collapsing `Model::boards`, `Model::board_by_id` and `Model::graph` accessors are removed. Behaviour is unchanged everywhere.
