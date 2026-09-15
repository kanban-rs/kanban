---
bump: patch
---

service: the per-session undo stack is now capped at 100 entries, dropping the oldest batch when it overflows. kanban-server holds one process-wide context and never drains the stack, so every HTTP mutation previously retained a command batch plus its captured inverse state for the life of the process. Interactive undo is unaffected: the TUI applies one entry per keypress, so 100 levels is effectively unlimited for a session.
