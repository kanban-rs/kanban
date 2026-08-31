---
bump: patch
---

kanban-mcp: card_crud tools resolve board/column/sprint/card names through the call-scoped Model instead of issuing a whole-collection backend read per call, mirroring the merged board.rs and card_batch.rs shape.
