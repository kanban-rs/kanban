---
bump: minor
---

domain: deletes `Model::columns()` and `Model::sprints()`, the collapsing accessors that silently flattened `NotLoaded`/`Missing`/`Failed` states into an empty slice. Every caller now goes through `columns_state()`/`sprints_state()` (or `.loaded_or_empty()` directly), matching the pattern already established for `boards()`, `all_cards()`, and `graph()`. A source-text guard test pins both accessors' absence. tui: the fallout from the deletion is mechanical, replacing every `X.columns()`/`X.sprints()` call site (all of them in test code) with `X.columns_state().loaded_or_empty()`/`X.sprints_state().loaded_or_empty()`, with no assertion's expected value changed.
