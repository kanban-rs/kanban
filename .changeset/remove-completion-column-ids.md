---
bump: minor
---

Completion is now determined solely by `column.default_status == Done`. `Board.completion_column_ids` and its storage (the JSON envelope field and the SQLite `board_completion_columns` table, schema v9) are removed; migrations continue to read the historical shape.
