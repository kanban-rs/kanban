---
bump: minor
---

service,tui: the in-app configuration editor shows and edits background_highlight (#RRGGBB, or empty for the default); an invalid value is rejected with an error and nothing is saved, and a valid one applies as soon as the editor closes (GH #918). Breaking (kanban-service library API): AppConfigDto gains a background_highlight field. Adds App::config_editor_buffer and App::finish_config_edit.
