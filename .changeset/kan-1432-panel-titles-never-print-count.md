---
bump: minor
---

view,tui: `TasksPanelTitle.count` is now a `PanelCount` (`Known`/`NotLoaded`/
`Failed`) instead of a bare `usize`, so the tasks panel title never prints a
confident `(0)` for a card, column, or sprint tier that has not loaded or has
failed to load. `build_filter_title_parts` resolves active sprint filter
names through the board-scoped sprint tier so an active filter always shows a
label even when that tier is unloaded.
