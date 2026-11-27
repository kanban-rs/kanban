---
bump: minor
---

Route the TUI's board-creation flow and card batch-archive flow through local/remote-aware service-layer methods instead of the local-only command batch: create-board now works over a remote backend (no more 'operation not supported by this backend'), and the local archive path now actually runs the CompactColumnPositions repack it was documented to (previously a silent no-op). Adds KanbanOperations::delete_cards and KanbanOperations::swap_column_positions, with local/remote parity tests (JSON + SQLite + mocked remote).
