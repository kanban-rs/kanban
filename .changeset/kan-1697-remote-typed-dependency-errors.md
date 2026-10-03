---
bump: patch
---

api: against a remote server, cycle / self-reference / duplicate-edge / missing-edge rejections now come back as the typed `DependencyError`, so `relation add`/`remove` (CLI) and the card-parent MCP tools show the same card-naming hints as locally.
