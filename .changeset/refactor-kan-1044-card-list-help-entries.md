---
bump: patch
---

view: replace `CardListComponentConfig::help_text()` with `help_entries()`, which returns structured `CardListHelpEntry { action, label }` values instead of a joined vim key-chord string, so a non-terminal frontend can render its own affordances. `kanban-tui` derives its own keyboard-chord hints and joins them back into the same footer text; terminal output is unchanged.
