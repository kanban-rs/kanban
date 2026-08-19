---
bump: patch
---

view: `Model` now records whether each of its board, column, card, sprint and dependency-graph collections has actually been loaded, instead of representing "never fetched" and "fetched and empty" the same way. New `*_state()` accessors expose that distinction; every existing accessor keeps its exact signature and behaviour, so nothing a user sees changes yet.
