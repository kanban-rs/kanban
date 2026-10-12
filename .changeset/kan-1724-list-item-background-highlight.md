---
bump: minor
---

tui: the projects list, help popup, parents/children boxes, import list and sort pickers use background_highlight. Breaking (kanban-tui library API): ListItemConfig gains a public highlight_bg field and ListItemConfig::new takes the highlight colour, Default and theme::selected_item are removed, render_relationship_section and render_selection_popup_with_lines take a highlight_bg parameter.
