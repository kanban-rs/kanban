---
bump: minor
---

Add the `search.cardlist.jumpto` configuration field (`"current"`, `"global"`, or `"both"`; absent means off). Additive — no behavior change yet. This value is consumed by the Enter-jump (KAN-1807) and Esc-restore (KAN-1808) features.

Example (`~/.config/kanban/config.toml`):

```toml
[search.cardlist]
jumpto = "current"
```
