---
bump: minor
---

Add the `search.cardlist.jumpto` configuration field (`"current"`, `"global"`, or `"both"`; absent means off). Additive — no behavior change yet. This value is consumed by the Enter-jump (KAN-1807) and Esc-restore (KAN-1808) features.

Also expose that field in the TUI Configuration settings panel as a selectable, editable "Search Jumpto" row, and refactor the settings-panel row model to an identity-based descriptor (`ConfigRowId`/`ConfigRow`/`config_panel_rows`) so adding a row no longer requires keeping hardcoded indices in sync across the view, the nav handlers, and the item count. This fixes a latent settings-navigation bug where the override + storage-configured mode reported 10 selectable rows but only 8 were navigable.

Example (`~/.config/kanban/config.toml`):

```toml
[search.cardlist]
jumpto = "current"
```
