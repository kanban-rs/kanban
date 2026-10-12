---
bump: minor
---

tui: sprint pickers, the carry-over dialog and the parents/children popup use background_highlight; a source guard forbids hard-coded selection backgrounds. Breaking (kanban-tui library API): render_entry_line, SprintPickerView::render/render_with_cursor and SprintPicker::render take a highlight_bg parameter.
