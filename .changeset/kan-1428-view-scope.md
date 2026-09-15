---
bump: minor
---

tui: adds `ViewScope`, kanban-tui's first production `kanban_service::FetchPlan` driver, and `App::view_scope`, which maps the current `AppMode`/`DialogMode`/`SelectionHub` state onto a `FetchRound` covering both the flat tiers the renderer reads and the by-parent tiers the handlers read.
