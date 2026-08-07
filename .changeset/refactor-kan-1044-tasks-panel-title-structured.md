---
bump: patch
---

view: `panel_titles` now returns a structured `TasksPanelTitle { kind, count, filters }` and bare filter labels instead of a formatted string with the terminal-only `[2]` panel hotkey baked in, so a non-terminal frontend can title its own panels. `kanban-tui` renders them; terminal titles are unchanged.
