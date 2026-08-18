---
bump: patch
---

persistence-sqlite, persistence-json: upgrading a workspace now carries every sprint-naming namespace's counter forward, not only the one its board currently resolves to. Workspaces that used `sprint create --prefix`, renamed a board's sprint prefix, or configured a default sprint prefix previously lost those counters on upgrade and could re-issue a sprint identifier a live sprint already held; they repair themselves on the next open.
