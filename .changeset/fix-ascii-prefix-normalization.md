---
bump: patch
---

domain: prefix normalisation now folds ASCII case only, matching the `COLLATE NOCASE` comparison SQLite uses for the `cards.prefix_ref` foreign key. Unicode folding desynced the stored row name from what SQLite could match, so a board with a non-ASCII card prefix (for example `ÖST`) could not store a card at all on the SQLite backend while JSON accepted it. Non-ASCII prefixes now round-trip on every backend, pinned by a new cross-backend contract test.
