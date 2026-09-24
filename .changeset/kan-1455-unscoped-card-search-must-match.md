---
bump: minor
---

domain,service: an unscoped `CardListFilter` search (no `board_id`) now filters cards by their own board instead of silently admitting every card. `filter_and_sort_cards` and `count_filtered_cards` gain a `boards: &[Board]` lookup slice, positioned after `board: Option<&Board>`, that each card's own board is resolved from when the search predicate runs; a card whose board is not present in the slice is excluded rather than admitted. The service tier gathers that boards slice, plus the cross-board sprint set, on the unscoped search path. Because this changes the signature of two public functions re-exported from `kanban-domain`, it takes a `minor` bump under the pre-1.0 policy.
