---
bump: patch
---

domain,api: `EntityIds`/`EntityIdsDto` gain `card_columns` (per-card set of invalidated `cards_by_column` scopes) and `archival_changed` (reserved, unset in this slice). Additive only: `merge` unions `card_columns`, `is_empty` is unaffected, and both new fields default to empty/false so existing producers and older wire peers are unchanged.
