---
bump: patch
---

view: every call site that read a `Model`'s cards or resolved a card by id now goes through the load-state accessors, so "never loaded" is named instead of silently collapsing into "empty" or "no such card". The collapsing `Model::all_cards` and `Model::card_by_id` accessors are removed. Behaviour is unchanged everywhere.
