---
bump: minor
---

kanban-domain, kanban-tui: replace the collapsing `Model::live_boards()` with `Model::live_boards_state()`, which reports `NotLoaded`/`Failed` instead of flattening every non-loaded state into an empty iterator. Every kanban-tui call site now distinguishes "not loaded yet" from "loaded and genuinely empty".
