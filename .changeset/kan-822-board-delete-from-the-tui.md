---
bump: minor
---

Added the ability to delete a board (project) directly from the TUI. Until now the only way to remove a board was the `kanban board delete` CLI command; there was no keybinding in the interface. Press `d` on the boards panel to delete the selected board, with a confirmation dialog that shows what will be removed (columns, tasks, archived tasks, and sprints) for a non-empty board, or a lighter prompt for an empty one. Deletion reuses the existing undoable cascade, so removing a board and everything in it can be undone with `u`, and the confirmation can be dismissed with `q`, `n`, or `Esc`.
