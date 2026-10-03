---
bump: minor
---

domain: moving a card to a column on another board (single move, batch move, a card update that changes only `column_id`, or a restore redirected to another board's column) now drops its sprint binding, closing the sprint log entry, unless the sprint lives on the destination board. Undoing the move restores the binding and log exactly, and undoing a restore that was redirected to another board's column now restores the card's original column, board, position, sprint binding, and sprint log exactly too, instead of leaving the binding permanently lost. Breaking for callers that expected a moved card to keep its old board's sprint; a card update that moves a card to another board while resubmitting its current sprint is now refused with `SPRINT_BOARD_MISMATCH`.
